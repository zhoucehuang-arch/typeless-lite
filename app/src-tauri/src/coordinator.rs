use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use parking_lot::Mutex;
use std::sync::mpsc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::asr::sherpa::{SherpaAsr, SherpaRuntime};
use crate::asr::RawTranscript;
use crate::hotkey::{HotkeyEvent, HotkeyMonitor};
use crate::insertion::TextInserter;
use crate::persistence::{
    apply_corrections, CorrectionStore, DictionaryStore, HistoryStore, PreferencesStore, StyleStore,
};
use crate::polish::polish_text;
use crate::recorder::{AudioConsumer, Recorder};
use crate::types::{CapsulePayload, CapsuleState, DictationSession, HotkeyMode, InsertStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Listening,
    Processing,
}

struct Session {
    id: String,
    started_at: Instant,
    recorder: Option<Recorder>,
    asr: Arc<SherpaAsr>,
}

pub struct Coordinator {
    app: AppHandle,
    prefs: PreferencesStore,
    history: HistoryStore,
    dictionary: DictionaryStore,
    corrections: CorrectionStore,
    styles: StyleStore,
    sherpa_runtime: Arc<SherpaRuntime>,
    phase: Mutex<Phase>,
    session: Mutex<Option<Session>>,
    hotkey_monitor: Mutex<Option<HotkeyMonitor>>,
}

impl Coordinator {
    pub fn new(app: AppHandle) -> anyhow::Result<Self> {
        Ok(Self {
            app,
            prefs: PreferencesStore::new()?,
            history: HistoryStore::new()?,
            dictionary: DictionaryStore::new()?,
            corrections: CorrectionStore::new()?,
            styles: StyleStore::new()?,
            sherpa_runtime: Arc::new(SherpaRuntime::new()),
            phase: Mutex::new(Phase::Idle),
            session: Mutex::new(None),
            hotkey_monitor: Mutex::new(None),
        })
    }

    pub fn prefs(&self) -> &PreferencesStore {
        &self.prefs
    }

    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    pub fn dictionary(&self) -> &DictionaryStore {
        &self.dictionary
    }

    pub fn corrections(&self) -> &CorrectionStore {
        &self.corrections
    }

    pub fn styles(&self) -> &StyleStore {
        &self.styles
    }

    pub fn install_hotkey(self: &Arc<Self>) -> Result<(), String> {
        let prefs = self.prefs.get();
        let (tx, rx) = mpsc::channel();
        let monitor = HotkeyMonitor::start(&prefs.hotkey, tx)?;
        *self.hotkey_monitor.lock() = Some(monitor);
        let coord = Arc::clone(self);
        std::thread::Builder::new()
            .name("typeless-hotkey-bridge".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    let coord = Arc::clone(&coord);
                    tauri::async_runtime::spawn(async move {
                        coord.handle_hotkey_event(event).await;
                    });
                }
            })
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn refresh_hotkey(self: &Arc<Self>) -> Result<(), String> {
        self.hotkey_monitor.lock().take();
        self.install_hotkey()
    }

    pub fn set_shortcut_recording_active(self: &Arc<Self>, active: bool) -> Result<(), String> {
        if active {
            self.hotkey_monitor.lock().take();
            Ok(())
        } else if self.hotkey_monitor.lock().is_none() {
            self.install_hotkey()
        } else {
            Ok(())
        }
    }

    pub fn reset_preferences(self: &Arc<Self>) -> Result<(), String> {
        let prefs = crate::persistence::reset_preferences_file().map_err(|err| err.to_string())?;
        self.prefs.set(prefs).map_err(|err| err.to_string())?;
        self.refresh_hotkey()
    }

    async fn handle_hotkey_event(self: Arc<Self>, event: HotkeyEvent) {
        let prefs = self.prefs.get();
        match (prefs.hotkey_mode, event) {
            (HotkeyMode::Hold, HotkeyEvent::Pressed) => {
                let _ = self.start_dictation().await;
            }
            (HotkeyMode::Hold, HotkeyEvent::Released) => {
                let _ = self.stop_dictation().await;
            }
            (HotkeyMode::Toggle, HotkeyEvent::Pressed) => {
                let phase = *self.phase.lock();
                if phase == Phase::Idle {
                    let _ = self.start_dictation().await;
                } else if phase == Phase::Listening {
                    let _ = self.stop_dictation().await;
                }
            }
            (HotkeyMode::Toggle, HotkeyEvent::Released) => {}
        }
    }

    pub async fn start_dictation(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut phase = self.phase.lock();
            if *phase != Phase::Idle {
                return Ok(());
            }
            *phase = Phase::Listening;
        }
        let prefs = self.prefs.get();
        let id = Uuid::new_v4().to_string();
        let asr = Arc::new(SherpaAsr::new(
            Arc::clone(&self.sherpa_runtime),
            prefs.sherpa_model.clone(),
            prefs.sherpa_language_hint.clone(),
        ));
        let level_coord = Arc::clone(self);
        let started_at = Instant::now();
        let level_handler: Arc<dyn Fn(f32) + Send + Sync> = Arc::new(move |level| {
            let elapsed = started_at.elapsed().as_millis() as u64;
            level_coord.emit_capsule(CapsuleState::Recording, level, elapsed, None, None);
        });
        let consumer: Arc<dyn AudioConsumer> = asr.clone();
        let (recorder, runtime_errors) = match Recorder::start(
            prefs.microphone_device_name.clone(),
            consumer,
            level_handler,
        ) {
            Ok(value) => value,
            Err(err) => {
                *self.phase.lock() = Phase::Idle;
                self.emit_capsule(
                    CapsuleState::Error,
                    0.0,
                    0,
                    Some(format!("录音启动失败: {err}")),
                    None,
                );
                return Err(err.to_string());
            }
        };
        *self.session.lock() = Some(Session {
            id,
            started_at,
            recorder: Some(recorder),
            asr,
        });
        let coord = Arc::clone(self);
        std::thread::spawn(move || {
            if let Ok(err) = runtime_errors.recv() {
                coord.abort_with_error(format!("录音中断: {err}"));
            }
        });
        self.emit_capsule(CapsuleState::Recording, 0.0, 0, None, None);
        Ok(())
    }

    pub async fn stop_dictation(self: &Arc<Self>) -> Result<(), String> {
        {
            let mut phase = self.phase.lock();
            if *phase != Phase::Listening {
                return Ok(());
            }
            *phase = Phase::Processing;
        }
        let mut session = match self.session.lock().take() {
            Some(session) => session,
            None => {
                *self.phase.lock() = Phase::Idle;
                return Ok(());
            }
        };
        let elapsed = session.started_at.elapsed().as_millis() as u64;
        self.emit_capsule(CapsuleState::Transcribing, 0.0, elapsed, None, None);
        if let Some(recorder) = session.recorder.take() {
            recorder.stop();
        }
        let raw = match session.asr.transcribe().await {
            Ok(raw) => raw,
            Err(err) => {
                *self.phase.lock() = Phase::Idle;
                self.emit_capsule(
                    CapsuleState::Error,
                    0.0,
                    elapsed,
                    Some(format!("识别失败: {err}")),
                    None,
                );
                return Err(err.to_string());
            }
        };
        self.finish_session(session.id, raw, elapsed).await
    }

    pub fn cancel_dictation(&self) {
        let session = self.session.lock().take();
        if let Some(mut session) = session {
            if let Some(recorder) = session.recorder.take() {
                recorder.stop();
            }
        }
        *self.phase.lock() = Phase::Idle;
        self.emit_capsule(CapsuleState::Cancelled, 0.0, 0, None, None);
    }

    async fn finish_session(&self, session_id: String, mut raw: RawTranscript, elapsed: u64) -> Result<(), String> {
        if raw.text.trim().is_empty() {
            let prefs = self.prefs.get();
            let mode = self
                .styles
                .get(&prefs.active_style_id)
                .map(|style| style.mode)
                .unwrap_or_default();
            let history = DictationSession {
                id: session_id,
                created_at: Utc::now().to_rfc3339(),
                raw_transcript: raw.text,
                final_text: String::new(),
                mode,
                insert_status: InsertStatus::Failed,
                error_code: Some("emptyTranscript".into()),
                duration_ms: raw.duration_ms,
                dictionary_hit_count: 0,
            };
            let _ = self.history.append(history, prefs.history_max_entries);
            *self.phase.lock() = Phase::Idle;
            self.emit_capsule(CapsuleState::Error, 0.0, elapsed, Some("没有识别到语音".into()), None);
            return Err("empty transcript".into());
        }

        let prefs = self.prefs.get();
        let corrections = self.corrections.list().unwrap_or_default();
        raw.text = apply_corrections(&raw.text, &corrections);
        let style = self
            .styles
            .get(&prefs.active_style_id)
            .map_err(|err| err.to_string())?;
        let hotwords = self.dictionary.enabled_phrases().unwrap_or_default();
        self.emit_capsule(CapsuleState::Polishing, 0.0, elapsed, None, None);
        let (mut final_text, polish_failed) = match polish_text(&raw.text, &style, &prefs, &hotwords).await {
            Ok(text) => (text, false),
            Err(err) => {
                log::warn!("[polish] failed: {err}");
                (raw.text.clone(), true)
            }
        };
        final_text = apply_corrections(&final_text, &corrections);
        let status = TextInserter::insert(&final_text, prefs.restore_clipboard_after_paste);
        let hits = self.dictionary.record_hits(&final_text).unwrap_or(0);
        let error_code = if polish_failed {
            Some("polishFailed".into())
        } else if status == InsertStatus::Failed {
            Some("insertFailed".into())
        } else {
            None
        };
        let history = DictationSession {
            id: session_id,
            created_at: Utc::now().to_rfc3339(),
            raw_transcript: raw.text,
            final_text: final_text.clone(),
            mode: style.mode,
            insert_status: status,
            error_code,
            duration_ms: raw.duration_ms,
            dictionary_hit_count: hits,
        };
        self.history
            .append(history, prefs.history_max_entries)
            .map_err(|err| err.to_string())?;
        *self.phase.lock() = Phase::Idle;
        let message = if polish_failed {
            Some("润色失败，已插入原文".into())
        } else if status == InsertStatus::CopiedFallback {
            Some("已复制，请手动粘贴".into())
        } else {
            None
        };
        self.emit_capsule(
            CapsuleState::Done,
            0.0,
            elapsed,
            message,
            Some(final_text.chars().count().min(u32::MAX as usize) as u32),
        );
        Ok(())
    }

    fn abort_with_error(&self, message: String) {
        self.cancel_dictation();
        self.emit_capsule(CapsuleState::Error, 0.0, 0, Some(message), None);
    }

    fn emit_capsule(
        &self,
        state: CapsuleState,
        level: f32,
        elapsed_ms: u64,
        message: Option<String>,
        inserted_chars: Option<u32>,
    ) {
        let payload = CapsulePayload {
            state,
            level,
            elapsed_ms,
            message,
            inserted_chars,
        };
        let _ = self.app.emit("capsule", payload);
    }
}
