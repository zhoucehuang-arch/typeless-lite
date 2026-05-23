use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::types::{
    builtin_styles, CorrectionRule, DictationSession, DictionaryEntry, Preferences, StyleProfile,
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
