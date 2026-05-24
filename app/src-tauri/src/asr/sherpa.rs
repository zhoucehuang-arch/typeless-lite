use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::asr::RawTranscript;
use crate::persistence;
use crate::recorder::AudioConsumer;
use crate::types::{SherpaDefaultModelStatus, SherpaModelFileStatus, SherpaModelInfo};

#[cfg(target_os = "windows")]
use sherpa_onnx::{
    OfflineParaformerModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, OfflineWhisperModelConfig,
};

pub const DEFAULT_MODEL_ALIAS: &str = "sense-voice-small-zh";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum ModelFamily {
    SenseVoice,
    Paraformer,
    Whisper,
}

#[derive(Debug, Clone, Copy)]
struct ModelSpec {
    alias: &'static str,
    display_name: &'static str,
    family: ModelFamily,
    languages: &'static [&'static str],
    files: &'static [&'static str],
    hf_repo: Option<&'static str>,
}

const MODELS: &[ModelSpec] = &[
    ModelSpec {
        alias: "sense-voice-small-zh",
        display_name: "SenseVoice Small (zh/en/ja/ko/yue)",
        family: ModelFamily::SenseVoice,
        languages: &["zh", "en", "ja", "ko", "yue"],
        files: &["model.int8.onnx", "tokens.txt"],
        hf_repo: Some("csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"),
    },
    ModelSpec {
        alias: "paraformer-zh",
        display_name: "Paraformer (zh)",
        family: ModelFamily::Paraformer,
        languages: &["zh"],
        files: &["model.int8.onnx", "tokens.txt"],
        hf_repo: None,
    },
    ModelSpec {
        alias: "whisper-small-multi",
        display_name: "Whisper Small (multilingual)",
        family: ModelFamily::Whisper,
        languages: &["multi"],
        files: &["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt"],
        hf_repo: None,
    },
];

const HF_BASE_URL: &str = "https://huggingface.co";
static DEFAULT_DOWNLOAD_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn catalog() -> Vec<SherpaModelInfo> {
    MODELS
        .iter()
        .map(|model| SherpaModelInfo {
            alias: model.alias.into(),
            display_name: model.display_name.into(),
            languages: model.languages.iter().map(|value| value.to_string()).collect(),
            cached: model_cached(model.alias),
        })
        .collect()
}

pub fn model_dir(alias: &str) -> Result<PathBuf> {
    ensure_known(alias)?;
    Ok(persistence::sherpa_models_root()?.join(alias))
}

pub fn default_model_status() -> Result<SherpaDefaultModelStatus> {
    let spec = spec_for(DEFAULT_MODEL_ALIAS)?;
    let dir = model_dir(spec.alias)?;
    let mut downloaded_bytes = 0u64;
    let mut files = Vec::new();
    for file in spec.files {
        let path = dir.join(file);
        let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        downloaded_bytes = downloaded_bytes.saturating_add(bytes);
        files.push(SherpaModelFileStatus {
            name: (*file).to_string(),
            present: path.exists(),
            bytes,
        });
    }
    Ok(SherpaDefaultModelStatus {
        alias: spec.alias.to_string(),
        display_name: spec.display_name.to_string(),
        cached: model_cached(spec.alias),
        directory: dir.display().to_string(),
        files,
        downloaded_bytes,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    alias: String,
    file: String,
    downloaded_bytes: u64,
    total_bytes: u64,
    done: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    lfs: Option<HfLfsInfo>,
}

#[derive(Debug, Deserialize)]
struct HfLfsInfo {
    #[serde(default)]
    size: Option<u64>,
}

pub async fn prepare_default_model(app: AppHandle) -> Result<SherpaDefaultModelStatus> {
    let spec = spec_for(DEFAULT_MODEL_ALIAS)?;
    if model_cached(spec.alias) {
        let status = default_model_status()?;
        emit_download_progress(
            &app,
            spec.alias,
            "",
            status.downloaded_bytes,
            status.downloaded_bytes,
            true,
            None,
        );
        return Ok(status);
    }
    if DEFAULT_DOWNLOAD_ACTIVE.swap(true, Ordering::SeqCst) {
        return default_model_status();
    }
    let result = download_model_files(app.clone(), spec)
        .await
        .and_then(|_| default_model_status());
    DEFAULT_DOWNLOAD_ACTIVE.store(false, Ordering::SeqCst);
    if let Err(error) = &result {
        emit_download_progress(&app, spec.alias, "", 0, 0, true, Some(error.to_string()));
    }
    result
}

pub fn model_cached(alias: &str) -> bool {
    let Some(spec) = MODELS.iter().find(|model| model.alias == alias) else {
        return false;
    };
    let Ok(dir) = model_dir(alias) else {
        return false;
    };
    spec.files.iter().all(|file| dir.join(file).exists())
}

pub struct SherpaAsr {
    runtime: Arc<SherpaRuntime>,
    model_alias: String,
    language_hint: Option<String>,
    buffer: Mutex<Vec<u8>>,
}

impl SherpaAsr {
    pub fn new(runtime: Arc<SherpaRuntime>, model_alias: String, language_hint: Option<String>) -> Self {
        Self {
            runtime,
            model_alias,
            language_hint: language_hint
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty()),
            buffer: Mutex::new(Vec::new()),
        }
    }

    pub async fn transcribe(&self) -> Result<RawTranscript> {
        let pcm = self.buffer.lock().clone();
        self.buffer.lock().clear();
        let duration_ms = (pcm.len() as u64 / 2) * 1000 / 16_000;
        if pcm.is_empty() {
            return Ok(RawTranscript {
                text: String::new(),
                duration_ms,
            });
        }
        let text = self
            .runtime
            .transcribe_pcm(&self.model_alias, pcm, self.language_hint.clone())
            .await?;
        Ok(RawTranscript {
            text: text.trim().to_string(),
            duration_ms,
        })
    }
}

impl AudioConsumer for SherpaAsr {
    fn consume_pcm_chunk(&self, pcm: &[u8]) {
        self.buffer.lock().extend_from_slice(pcm);
    }
}

#[derive(Default)]
pub struct SherpaRuntime {
    #[cfg(target_os = "windows")]
    loaded: Mutex<Option<LoadedModel>>,
}

#[cfg(target_os = "windows")]
struct LoadedModel {
    alias: String,
    recognizer: Arc<OfflineRecognizer>,
}

impl SherpaRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn preload(&self, alias: &str) -> Result<()> {
        ensure_known(alias)?;
        #[cfg(not(target_os = "windows"))]
        {
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            let _ = self.ensure_loaded(alias).await?;
            Ok(())
        }
    }

    pub async fn transcribe_pcm(
        &self,
        alias: &str,
        pcm: Vec<u8>,
        language_hint: Option<String>,
    ) -> Result<String> {
        ensure_known(alias)?;
        #[cfg(not(target_os = "windows"))]
        {
            let _ = pcm;
            let _ = language_hint;
            anyhow::bail!("本地 sherpa-onnx ASR 首版仅支持 Windows");
        }

        #[cfg(target_os = "windows")]
        {
            let loaded = self.ensure_loaded(alias).await?;
            transcribe_loaded_model(loaded, pcm, language_hint).await
        }
    }

    #[cfg(target_os = "windows")]
    async fn ensure_loaded(&self, alias: &str) -> Result<Arc<OfflineRecognizer>> {
        if let Some(loaded) = self
            .loaded
            .lock()
            .as_ref()
            .filter(|loaded| loaded.alias == alias)
        {
            return Ok(Arc::clone(&loaded.recognizer));
        }
        let alias_owned = alias.to_string();
        let dir = model_dir(alias)?;
        let spec = spec_for(alias)?;
        ensure_files(spec, &dir)?;
        let recognizer = tokio::task::spawn_blocking(move || create_recognizer(&alias_owned, &dir))
            .await
            .map_err(|err| anyhow::anyhow!("sherpa-onnx load join failed: {err}"))??;
        let recognizer = Arc::new(recognizer);
        *self.loaded.lock() = Some(LoadedModel {
            alias: alias.into(),
            recognizer: Arc::clone(&recognizer),
        });
        Ok(recognizer)
    }
}

fn ensure_known(alias: &str) -> Result<()> {
    if MODELS.iter().any(|model| model.alias == alias) {
        Ok(())
    } else {
        anyhow::bail!("unknown sherpa-onnx model alias: {alias}");
    }
}

fn spec_for(alias: &str) -> Result<&'static ModelSpec> {
    MODELS
        .iter()
        .find(|model| model.alias == alias)
        .context("unknown sherpa model")
}

async fn download_model_files(app: AppHandle, spec: &'static ModelSpec) -> Result<()> {
    let repo = spec
        .hf_repo
        .ok_or_else(|| anyhow::anyhow!("模型 {} 暂未配置自动下载源", spec.alias))?;
    let dir = model_dir(spec.alias)?;
    tokio::fs::create_dir_all(&dir).await?;
    let client = reqwest::Client::builder()
        .user_agent("TypelessLite/0.1")
        .build()?;
    let sizes = fetch_hf_file_sizes(&client, repo).await.unwrap_or_default();
    let total_bytes = spec
        .files
        .iter()
        .map(|file| {
            sizes
                .iter()
                .find(|(path, _)| path.as_str() == *file)
                .map(|(_, size)| *size)
                .unwrap_or(0)
        })
        .sum::<u64>();
    let mut completed_bytes = 0u64;
    for file in spec.files {
        let dest = dir.join(file);
        if dest.exists() {
            completed_bytes = completed_bytes
                .saturating_add(std::fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(0));
            emit_download_progress(&app, spec.alias, file, completed_bytes, total_bytes, false, None);
            continue;
        }
        let expected = sizes
            .iter()
            .find(|(path, _)| path.as_str() == *file)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        download_one_file(
            &client,
            repo,
            file,
            &dest,
            &app,
            spec.alias,
            completed_bytes,
            total_bytes,
            expected,
        )
        .await?;
        completed_bytes = completed_bytes
            .saturating_add(std::fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(expected));
    }
    emit_download_progress(&app, spec.alias, "", completed_bytes, total_bytes, true, None);
    Ok(())
}

async fn fetch_hf_file_sizes(client: &reqwest::Client, repo: &str) -> Result<Vec<(String, u64)>> {
    let url = format!("{HF_BASE_URL}/api/models/{repo}/tree/main");
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("HuggingFace 模型清单请求失败: HTTP {}", response.status().as_u16());
    }
    let entries: Vec<HfTreeEntry> = response.json().await?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry.entry_type == "file")
        .filter_map(|entry| {
            let size = entry.lfs.and_then(|lfs| lfs.size).or(entry.size).unwrap_or(0);
            if size > 0 {
                Some((entry.path, size))
            } else {
                None
            }
        })
        .collect())
}

async fn download_one_file(
    client: &reqwest::Client,
    repo: &str,
    file: &str,
    dest: &Path,
    app: &AppHandle,
    alias: &str,
    completed_before: u64,
    total_bytes: u64,
    expected_bytes: u64,
) -> Result<()> {
    let url = format!("{HF_BASE_URL}/{repo}/resolve/main/{file}");
    let partial = dest.with_extension("partial");
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("下载 ASR 模型文件 {file} 失败: HTTP {}", response.status().as_u16());
    }
    let file_total = response.content_length().unwrap_or(expected_bytes);
    let total = if total_bytes > 0 {
        total_bytes
    } else {
        completed_before.saturating_add(file_total)
    };
    let mut stream = response.bytes_stream();
    let mut output = tokio::fs::File::create(&partial).await?;
    let mut downloaded = 0u64;
    emit_download_progress(app, alias, file, completed_before, total, false, None);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        output.write_all(&chunk).await?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        emit_download_progress(
            app,
            alias,
            file,
            completed_before.saturating_add(downloaded),
            total,
            false,
            None,
        );
    }
    output.flush().await?;
    drop(output);
    tokio::fs::rename(&partial, dest).await.or_else(|err| {
        let _ = std::fs::remove_file(&partial);
        Err(err)
    })?;
    Ok(())
}

fn emit_download_progress(
    app: &AppHandle,
    alias: &str,
    file: &str,
    downloaded_bytes: u64,
    total_bytes: u64,
    done: bool,
    error: Option<String>,
) {
    let _ = app.emit(
        "sherpa-download-progress",
        DownloadProgress {
            alias: alias.to_string(),
            file: file.to_string(),
            downloaded_bytes,
            total_bytes,
            done,
            error,
        },
    );
}

#[cfg(target_os = "windows")]
fn ensure_files(spec: &ModelSpec, dir: &Path) -> Result<()> {
    for file in spec.files {
        if !dir.join(file).exists() {
            anyhow::bail!(
                "缺少本地 ASR 模型文件 {}，请放到 {}",
                file,
                dir.display()
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn create_recognizer(alias: &str, dir: &Path) -> Result<OfflineRecognizer> {
    let spec = spec_for(alias)?;
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.num_threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(1, 4) as i32)
        .unwrap_or(2);
    config.model_config.provider = Some("cpu".into());
    match spec.family {
        ModelFamily::SenseVoice => {
            config.model_config.tokens = Some(path_string(&dir.join("tokens.txt"))?);
            config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(path_string(&dir.join("model.int8.onnx"))?),
                language: Some("auto".into()),
                use_itn: true,
            };
        }
        ModelFamily::Paraformer => {
            config.model_config.tokens = Some(path_string(&dir.join("tokens.txt"))?);
            config.model_config.paraformer = OfflineParaformerModelConfig {
                model: Some(path_string(&dir.join("model.int8.onnx"))?),
            };
        }
        ModelFamily::Whisper => {
            config.model_config.tokens = Some(path_string(&dir.join("tokens.txt"))?);
            config.model_config.whisper = OfflineWhisperModelConfig {
                encoder: Some(path_string(&dir.join("encoder.int8.onnx"))?),
                decoder: Some(path_string(&dir.join("decoder.int8.onnx"))?),
                language: Some("auto".into()),
                task: Some("transcribe".into()),
                tail_paddings: 0,
                enable_token_timestamps: false,
                enable_segment_timestamps: false,
            };
        }
    }
    OfflineRecognizer::create(&config)
        .ok_or_else(|| anyhow::anyhow!("create sherpa-onnx offline recognizer failed"))
}

#[cfg(target_os = "windows")]
fn path_string(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))?
        .to_string())
}

#[cfg(target_os = "windows")]
async fn transcribe_loaded_model(
    recognizer: Arc<OfflineRecognizer>,
    pcm: Vec<u8>,
    language_hint: Option<String>,
) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let samples = pcm_s16le_to_f32(&pcm)?;
        let stream = recognizer.create_stream();
        if let Some(language) = language_hint.as_deref().filter(|value| !value.is_empty()) {
            if stream.has_option("language") {
                stream.set_option("language", language);
            }
        }
        stream.accept_waveform(16_000, &samples);
        recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| anyhow::anyhow!("sherpa-onnx returned no result"))?;
        Ok(result.text)
    })
    .await
    .map_err(|err| anyhow::anyhow!("sherpa-onnx transcribe join failed: {err}"))?
}

#[cfg(target_os = "windows")]
fn pcm_s16le_to_f32(pcm: &[u8]) -> Result<Vec<f32>> {
    if pcm.len() % 2 != 0 {
        anyhow::bail!("PCM buffer length is not aligned to i16 samples");
    }
    Ok(pcm
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect())
}
