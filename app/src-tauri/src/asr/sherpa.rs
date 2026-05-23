use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::Serialize;

use crate::asr::RawTranscript;
use crate::persistence;
use crate::recorder::AudioConsumer;
use crate::types::SherpaModelInfo;

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
}

const MODELS: &[ModelSpec] = &[
    ModelSpec {
        alias: "sense-voice-small-zh",
        display_name: "SenseVoice Small (zh/en/ja/ko/yue)",
        family: ModelFamily::SenseVoice,
        languages: &["zh", "en", "ja", "ko", "yue"],
        files: &["model.int8.onnx", "tokens.txt"],
    },
    ModelSpec {
        alias: "paraformer-zh",
        display_name: "Paraformer (zh)",
        family: ModelFamily::Paraformer,
        languages: &["zh"],
        files: &["model.int8.onnx", "tokens.txt"],
    },
    ModelSpec {
        alias: "whisper-small-multi",
        display_name: "Whisper Small (multilingual)",
        family: ModelFamily::Whisper,
        languages: &["multi"],
        files: &["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt"],
    },
];

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
