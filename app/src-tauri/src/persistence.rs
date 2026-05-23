use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde::Serialize;
use uuid::Uuid;

use crate::types::{
    builtin_styles, CorrectionRule, DictationSession, DictionaryEntry, LocalDataFileStatus,
    LocalDataStatus, Preferences, StyleProfile,
};

const HISTORY_FILE: &str = "history.json";
const PREFERENCES_FILE: &str = "preferences.json";
const DICTIONARY_FILE: &str = "dictionary.json";
const CORRECTIONS_FILE: &str = "correction-rules.json";
const STYLES_FILE: &str = "styles.json";

pub fn data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").context("APPDATA not set")?;
        Ok(PathBuf::from(appdata).join("TypelessLite"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("TypelessLite"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Ok(PathBuf::from(xdg).join("TypelessLite"));
            }
        }
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".local/share/TypelessLite"))
    }
}

pub fn sherpa_models_root() -> Result<PathBuf> {
    let dir = data_dir()?.join("models").join("sherpa-onnx");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn local_data_status(llm_api_key_configured: bool) -> Result<LocalDataStatus> {
    let dir = data_dir()?;
    let files = data_file_names()
        .iter()
        .map(|name| file_status(&dir, name))
        .collect::<Result<Vec<_>>>()?;
    let llm_api_key_found_in_json = files_with_json_secrets(&dir)?.into_iter().any(|found| found);
    Ok(LocalDataStatus {
        data_dir: dir.display().to_string(),
        files,
        llm_api_key_configured,
        llm_api_key_found_in_json,
    })
}

pub fn reset_preferences_file() -> Result<Preferences> {
    let prefs = Preferences::default();
    write_json(&data_dir()?.join(PREFERENCES_FILE), &prefs)?;
    Ok(prefs)
}

pub fn reset_history_file() -> Result<()> {
    write_json(&data_dir()?.join(HISTORY_FILE), &Vec::<DictationSession>::new())
}

pub fn reset_dictionary_files() -> Result<()> {
    write_json(&data_dir()?.join(DICTIONARY_FILE), &Vec::<DictionaryEntry>::new())?;
    write_json(&data_dir()?.join(CORRECTIONS_FILE), &Vec::<CorrectionRule>::new())
}

pub fn reset_styles_file() -> Result<()> {
    write_json(&data_dir()?.join(STYLES_FILE), &builtin_styles())
}

fn data_file_names() -> [&'static str; 5] {
    [
        PREFERENCES_FILE,
        HISTORY_FILE,
        DICTIONARY_FILE,
        CORRECTIONS_FILE,
        STYLES_FILE,
    ]
}

fn file_status(dir: &Path, name: &str) -> Result<LocalDataFileStatus> {
    let path = dir.join(name);
    let exists = path.exists();
    let bytes = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
    let records = if exists {
        json_record_count(&path).ok()
    } else {
        None
    };
    Ok(LocalDataFileStatus {
        name: name.to_string(),
        path: path.display().to_string(),
        exists,
        bytes,
        records,
    })
}

fn json_record_count(path: &Path) -> Result<u64> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() {
        return Ok(0);
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    Ok(match value {
        Value::Array(items) => items.len() as u64,
        Value::Object(items) => items.len() as u64,
        Value::Null => 0,
        _ => 1,
    })
}

fn files_with_json_secrets(dir: &Path) -> Result<Vec<bool>> {
    data_file_names()
        .iter()
        .map(|name| {
            let path = dir.join(name);
            if !path.exists() {
                return Ok(false);
            }
            let text = fs::read_to_string(path)?;
            let lower = text.to_ascii_lowercase();
            Ok(lower.contains("api_key")
                || lower.contains("apikey")
                || lower.contains("authorization")
                || lower.contains("bearer "))
        })
        .collect()
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!("{name}.tmp-{}", Uuid::new_v4().simple()));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path).or_else(|err| {
        let _ = fs::remove_file(&tmp);
        Err(err)
    })?;
    Ok(())
}

fn read_or_default<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() {
        return Ok(T::default());
    }
    serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write(path, &bytes)
}

pub struct PreferencesStore {
    path: PathBuf,
    current: Mutex<Preferences>,
}

impl PreferencesStore {
    pub fn new() -> Result<Self> {
        let path = data_dir()?.join(PREFERENCES_FILE);
        let current = read_or_default(&path)?;
        Ok(Self {
            path,
            current: Mutex::new(current),
        })
    }

    pub fn get(&self) -> Preferences {
        self.current.lock().clone()
    }

    pub fn set(&self, prefs: Preferences) -> Result<()> {
        write_json(&self.path, &prefs)?;
        *self.current.lock() = prefs;
        Ok(())
    }
}

pub struct HistoryStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl HistoryStore {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: data_dir()?.join(HISTORY_FILE),
            lock: Mutex::new(()),
        })
    }

    pub fn list(&self) -> Result<Vec<DictationSession>> {
        let _guard = self.lock.lock();
        read_or_default(&self.path)
    }

    pub fn append(&self, session: DictationSession, max_entries: u32) -> Result<()> {
        let _guard = self.lock.lock();
        let mut sessions: Vec<DictationSession> = read_or_default(&self.path)?;
        sessions.insert(0, session);
        let cap = max_entries.clamp(5, 200) as usize;
        if sessions.len() > cap {
            sessions.truncate(cap);
        }
        write_json(&self.path, &sessions)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let _guard = self.lock.lock();
        let mut sessions: Vec<DictationSession> = read_or_default(&self.path)?;
        sessions.retain(|session| session.id != id);
        write_json(&self.path, &sessions)
    }

    pub fn clear(&self) -> Result<()> {
        let _guard = self.lock.lock();
        write_json(&self.path, &Vec::<DictationSession>::new())
    }
}

pub struct DictionaryStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl DictionaryStore {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: data_dir()?.join(DICTIONARY_FILE),
            lock: Mutex::new(()),
        })
    }

    pub fn list(&self) -> Result<Vec<DictionaryEntry>> {
        let _guard = self.lock.lock();
        read_or_default(&self.path)
    }

    pub fn enabled_phrases(&self) -> Result<Vec<String>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.phrase)
            .collect())
    }

    pub fn add(&self, phrase: String, note: Option<String>) -> Result<DictionaryEntry> {
        let phrase = phrase.trim().to_string();
        anyhow::ensure!(!phrase.is_empty(), "词条不能为空");
        let _guard = self.lock.lock();
        let mut entries: Vec<DictionaryEntry> = read_or_default(&self.path)?;
        if let Some(existing) = entries
            .iter()
            .find(|entry| entry.phrase.eq_ignore_ascii_case(&phrase))
        {
            return Ok(existing.clone());
        }
        let entry = DictionaryEntry {
            id: Uuid::new_v4().to_string(),
            phrase,
            note,
            enabled: true,
            hits: 0,
            created_at: Utc::now().to_rfc3339(),
        };
        entries.insert(0, entry.clone());
        write_json(&self.path, &entries)?;
        Ok(entry)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let _guard = self.lock.lock();
        let mut entries: Vec<DictionaryEntry> = read_or_default(&self.path)?;
        entries.retain(|entry| entry.id != id);
        write_json(&self.path, &entries)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let _guard = self.lock.lock();
        let mut entries: Vec<DictionaryEntry> = read_or_default(&self.path)?;
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.enabled = enabled;
        }
        write_json(&self.path, &entries)
    }

    pub fn record_hits(&self, text: &str) -> Result<u64> {
        let _guard = self.lock.lock();
        let mut entries: Vec<DictionaryEntry> = read_or_default(&self.path)?;
        let mut total = 0u64;
        for entry in entries.iter_mut().filter(|entry| entry.enabled) {
            let count = text.matches(&entry.phrase).count() as u64;
            if count > 0 {
                entry.hits = entry.hits.saturating_add(count);
                total = total.saturating_add(count);
            }
        }
        if total > 0 {
            write_json(&self.path, &entries)?;
        }
        Ok(total)
    }
}

pub struct CorrectionStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl CorrectionStore {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: data_dir()?.join(CORRECTIONS_FILE),
            lock: Mutex::new(()),
        })
    }

    pub fn list(&self) -> Result<Vec<CorrectionRule>> {
        let _guard = self.lock.lock();
        read_or_default(&self.path)
    }

    pub fn add(&self, pattern: String, replacement: String) -> Result<CorrectionRule> {
        let pattern = pattern.trim().to_string();
        anyhow::ensure!(!pattern.is_empty(), "纠错词不能为空");
        let _guard = self.lock.lock();
        let mut rules: Vec<CorrectionRule> = read_or_default(&self.path)?;
        let rule = CorrectionRule {
            id: Uuid::new_v4().to_string(),
            pattern,
            replacement: replacement.trim().to_string(),
            enabled: true,
            created_at: Utc::now().to_rfc3339(),
        };
        rules.insert(0, rule.clone());
        write_json(&self.path, &rules)?;
        Ok(rule)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let _guard = self.lock.lock();
        let mut rules: Vec<CorrectionRule> = read_or_default(&self.path)?;
        rules.retain(|rule| rule.id != id);
        write_json(&self.path, &rules)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let _guard = self.lock.lock();
        let mut rules: Vec<CorrectionRule> = read_or_default(&self.path)?;
        if let Some(rule) = rules.iter_mut().find(|rule| rule.id == id) {
            rule.enabled = enabled;
        }
        write_json(&self.path, &rules)
    }
}

pub fn apply_corrections(text: &str, rules: &[CorrectionRule]) -> String {
    rules
        .iter()
        .filter(|rule| rule.enabled && !rule.pattern.is_empty())
        .fold(text.to_string(), |acc, rule| {
            acc.replace(&rule.pattern, &rule.replacement)
        })
}

pub struct StyleStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl StyleStore {
    pub fn new() -> Result<Self> {
        let store = Self {
            path: data_dir()?.join(STYLES_FILE),
            lock: Mutex::new(()),
        };
        store.ensure_defaults()?;
        Ok(store)
    }

    pub fn list(&self) -> Result<Vec<StyleProfile>> {
        let _guard = self.lock.lock();
        read_or_default(&self.path)
    }

    pub fn get(&self, id: &str) -> Result<StyleProfile> {
        let styles = self.list()?;
        styles
            .iter()
            .find(|style| style.id == id)
            .cloned()
            .or_else(|| styles.into_iter().find(|style| style.id == "builtin.light"))
            .context("no style profiles available")
    }

    pub fn save(&self, mut style: StyleProfile) -> Result<StyleProfile> {
        let _guard = self.lock.lock();
        let mut styles: Vec<StyleProfile> = read_or_default(&self.path)?;
        style.updated_at = Utc::now().to_rfc3339();
        if let Some(existing) = styles.iter_mut().find(|item| item.id == style.id) {
            *existing = style.clone();
        } else {
            styles.push(style.clone());
        }
        write_json(&self.path, &styles)?;
        Ok(style)
    }

    pub fn reset_builtin(&self, id: &str) -> Result<StyleProfile> {
        let builtin = builtin_styles()
            .into_iter()
            .find(|style| style.id == id)
            .context("unknown builtin style")?;
        self.save(builtin)
    }

    fn ensure_defaults(&self) -> Result<()> {
        let _guard = self.lock.lock();
        let mut styles: Vec<StyleProfile> = read_or_default(&self.path)?;
        let mut changed = false;
        for builtin in builtin_styles() {
            if !styles.iter().any(|style| style.id == builtin.id) {
                styles.push(builtin);
                changed = true;
            }
        }
        if changed || !self.path.exists() {
            write_json(&self.path, &styles)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DictationSession, InsertStatus, PolishMode};

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "typeless-lite-test-{name}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_json_roundtrip_preserves_preferences() {
        let dir = temp_path("preferences");
        let path = dir.join("preferences.json");
        let prefs = Preferences {
            hotkey: "Ctrl+Alt+Space".into(),
            ..Preferences::default()
        };

        write_json(&path, &prefs).unwrap();
        let restored: Preferences = read_or_default(&path).unwrap();

        assert_eq!(restored.hotkey, "Ctrl+Alt+Space");
        assert!(!fs::read_to_string(path).unwrap().contains("api_key"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn history_append_trims_to_configured_cap() {
        let dir = temp_path("history");
        let store = HistoryStore {
            path: dir.join("history.json"),
            lock: Mutex::new(()),
        };

        for index in 0..8 {
            store.append(history_item(index), 5).unwrap();
        }

        let items = store.list().unwrap();
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].raw_transcript, "raw 7");
        assert_eq!(items[4].raw_transcript, "raw 3");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dictionary_and_corrections_can_reset() {
        let dir = temp_path("dictionary");
        let entries_path = dir.join("dictionary.json");
        let rules_path = dir.join("correction-rules.json");
        write_json(
            &entries_path,
            &vec![DictionaryEntry {
                id: "entry".into(),
                phrase: "Typeless".into(),
                note: None,
                enabled: true,
                hits: 0,
                created_at: "now".into(),
            }],
        )
        .unwrap();
        write_json(
            &rules_path,
            &vec![CorrectionRule {
                id: "rule".into(),
                pattern: "Open Less".into(),
                replacement: "OpenLess".into(),
                enabled: true,
                created_at: "now".into(),
            }],
        )
        .unwrap();

        write_json(&entries_path, &Vec::<DictionaryEntry>::new()).unwrap();
        write_json(&rules_path, &Vec::<CorrectionRule>::new()).unwrap();

        let entries: Vec<DictionaryEntry> = read_or_default(&entries_path).unwrap();
        let rules: Vec<CorrectionRule> = read_or_default(&rules_path).unwrap();
        assert!(entries.is_empty());
        assert!(rules.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn json_secret_scan_detects_api_key_like_fields() {
        let dir = temp_path("secrets");
        write_json(&dir.join("preferences.json"), &Preferences::default()).unwrap();
        fs::write(dir.join("history.json"), r#"[{"api_key":"leaked"}]"#).unwrap();

        let findings = files_with_json_secrets(&dir).unwrap();
        assert!(findings.into_iter().any(|found| found));
        fs::remove_dir_all(dir).unwrap();
    }

    fn history_item(index: usize) -> DictationSession {
        DictationSession {
            id: index.to_string(),
            created_at: "2026-05-23T00:00:00Z".into(),
            raw_transcript: format!("raw {index}"),
            final_text: format!("final {index}"),
            mode: PolishMode::Light,
            insert_status: InsertStatus::Inserted,
            error_code: None,
            duration_ms: 100,
            dictionary_hit_count: 0,
        }
    }
}
