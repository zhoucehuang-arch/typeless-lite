use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::types::InsertStatus;

const RESTORE_DELAY: Duration = Duration::from_millis(750);

static NEXT_RESTORE_ID: AtomicU64 = AtomicU64::new(1);
static PENDING_RESTORE: Lazy<Mutex<Option<PendingRestore>>> = Lazy::new(|| Mutex::new(None));

#[derive(Clone)]
struct PendingRestore {
    restore_id: u64,
    original: Option<String>,
}

pub struct TextInserter;

impl TextInserter {
    pub fn insert(text: &str, restore_clipboard: bool) -> InsertStatus {
        if text.trim().is_empty() {
            return InsertStatus::Failed;
        }
        let plan = match write_clipboard(text) {
            Ok(plan) => plan,
            Err(err) => {
                log::error!("[insertion] clipboard write failed: {err}");
                return InsertStatus::Failed;
            }
        };
        if simulate_paste().is_err() {
            return InsertStatus::CopiedFallback;
        }
        if restore_clipboard {
            schedule_restore(plan);
        }
        InsertStatus::Inserted
    }
}

struct ClipboardPlan {
    inserted: String,
    previous: Option<String>,
}

fn write_clipboard(text: &str) -> Result<ClipboardPlan, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;
    let previous = clipboard.get_text().ok();
    clipboard.set_text(text.to_string()).map_err(|err| err.to_string())?;
    Ok(ClipboardPlan {
        inserted: text.to_string(),
        previous,
    })
}

fn schedule_restore(plan: ClipboardPlan) {
    let restore_id = NEXT_RESTORE_ID.fetch_add(1, Ordering::SeqCst);
    let original = {
        let mut pending = PENDING_RESTORE.lock();
        let original = pending
            .as_ref()
            .map(|existing| existing.original.clone())
            .unwrap_or(plan.previous);
        *pending = Some(PendingRestore {
            restore_id,
            original: original.clone(),
        });
        original
    };
    std::thread::spawn(move || {
        std::thread::sleep(RESTORE_DELAY);
        if !matches!(
            PENDING_RESTORE.lock().as_ref(),
            Some(pending) if pending.restore_id == restore_id
        ) {
            return;
        }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if clipboard.get_text().ok().as_deref() == Some(plan.inserted.as_str()) {
                if let Some(original) = original {
                    let _ = clipboard.set_text(original);
                }
            }
        }
        let mut pending = PENDING_RESTORE.lock();
        if matches!(pending.as_ref(), Some(item) if item.restore_id == restore_id) {
            pending.take();
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|err| err.to_string())?;
    enigo.key(Key::Control, Direction::Press).map_err(|err| err.to_string())?;
    let click = enigo.key(Key::Unicode('v'), Direction::Click).map_err(|err| err.to_string());
    let release = enigo.key(Key::Control, Direction::Release).map_err(|err| err.to_string());
    click.and(release)
}

#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|err| err.to_string())?;
    enigo.key(Key::Meta, Direction::Press).map_err(|err| err.to_string())?;
    let click = enigo.key(Key::Unicode('v'), Direction::Click).map_err(|err| err.to_string());
    let release = enigo.key(Key::Meta, Direction::Release).map_err(|err| err.to_string());
    click.and(release)
}
