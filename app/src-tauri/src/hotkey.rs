use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;

#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

static RUNTIME: OnceCell<Arc<GlobalHotkeyRuntime>> = OnceCell::new();

pub struct GlobalHotkeyRuntime {
    manager: GlobalHotKeyManager,
    routes: Mutex<HashMap<u32, Sender<GlobalHotKeyEvent>>>,
    shutdown: AtomicBool,
}

unsafe impl Send for GlobalHotkeyRuntime {}
unsafe impl Sync for GlobalHotkeyRuntime {}

struct RegisteredHotkey {
    runtime: Arc<GlobalHotkeyRuntime>,
    hotkey: HotKey,
}

impl Drop for RegisteredHotkey {
    fn drop(&mut self) {
        self.runtime.routes.lock().remove(&self.hotkey.id());
        if let Err(err) = self.runtime.manager.unregister(self.hotkey) {
            log::warn!("[hotkey] unregister failed: {err}");
        }
    }
}

impl GlobalHotkeyRuntime {
    fn shared() -> Result<Arc<Self>, String> {
        RUNTIME
            .get_or_try_init(|| {
                let manager = GlobalHotKeyManager::new().map_err(|err| err.to_string())?;
                let runtime = Arc::new(Self {
                    manager,
                    routes: Mutex::new(HashMap::new()),
                    shutdown: AtomicBool::new(false),
                });
                start_dispatcher(Arc::clone(&runtime));
                Ok(runtime)
            })
            .cloned()
    }

    fn register(
        self: &Arc<Self>,
        hotkey: HotKey,
    ) -> Result<(RegisteredHotkey, Receiver<GlobalHotKeyEvent>), String> {
        self.manager.register(hotkey).map_err(|err| err.to_string())?;
        let (tx, rx) = mpsc::channel();
        self.routes.lock().insert(hotkey.id(), tx);
        Ok((
            RegisteredHotkey {
                runtime: Arc::clone(self),
                hotkey,
            },
            rx,
        ))
    }
}

fn start_dispatcher(runtime: Arc<GlobalHotkeyRuntime>) {
    std::thread::Builder::new()
        .name("typeless-global-hotkey-dispatch".into())
        .spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while !runtime.shutdown.load(Ordering::SeqCst) {
                let Ok(event) = receiver.recv_timeout(Duration::from_millis(250)) else {
                    continue;
                };
                let route = runtime.routes.lock().get(&event.id()).cloned();
                if let Some(tx) = route {
                    let _ = tx.send(event);
                }
            }
        })
        .expect("spawn global hotkey dispatcher");
}

pub struct HotkeyMonitor {
    registered: Option<RegisteredHotkey>,
}

impl HotkeyMonitor {
    pub fn start(binding: &str, tx: Sender<HotkeyEvent>) -> Result<Self, String> {
        let runtime = GlobalHotkeyRuntime::shared()?;
        let hotkey = parse_hotkey(binding)?;
        let hotkey_id = hotkey.id();
        let (registered, rx) = runtime.register(hotkey)?;
        std::thread::Builder::new()
            .name("typeless-hotkey-forward".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    if event.id() != hotkey_id {
                        continue;
                    }
                    let next = match event.state() {
                        HotKeyState::Pressed => HotkeyEvent::Pressed,
                        HotKeyState::Released => HotkeyEvent::Released,
                    };
                    if tx.send(next).is_err() {
                        break;
                    }
                }
            })
            .map_err(|err| err.to_string())?;
        Ok(Self {
            registered: Some(registered),
        })
    }
}

impl Drop for HotkeyMonitor {
    fn drop(&mut self) {
        self.registered.take();
    }
}

pub fn validate_hotkey_binding(raw: &str) -> Result<(), String> {
    parse_hotkey(raw).map(|_| ())
}

fn parse_hotkey(raw: &str) -> Result<HotKey, String> {
    let parts: Vec<String> = raw
        .split('+')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("快捷键不能为空".into());
    }
    let primary = parts.last().cloned().unwrap_or_default();
    let mut mods = Modifiers::empty();
    for modifier in &parts[..parts.len().saturating_sub(1)] {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "alt" | "option" => mods |= Modifiers::ALT,
            "cmd" | "command" | "meta" | "super" | "win" => mods |= Modifiers::SUPER,
            other => return Err(format!("不支持的修饰键: {other}")),
        }
    }
    let code = parse_key(&primary)?;
    let mods = if mods.is_empty() { None } else { Some(mods) };
    Ok(HotKey::new(mods, code))
}

fn parse_key(raw: &str) -> Result<Code, String> {
    if raw.chars().count() == 1 {
        if let Some(code) = char_to_code(raw.chars().next().unwrap()) {
            return Ok(code);
        }
    }
    let key = match raw.to_ascii_lowercase().as_str() {
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "esc" | "escape" => Code::Escape,
        "backspace" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        other => return Err(format!("不支持的主键: {other}")),
    };
    Ok(key)
}

fn char_to_code(ch: char) -> Option<Code> {
    Some(match ch.to_ascii_uppercase() {
        'A' => Code::KeyA,
        'B' => Code::KeyB,
        'C' => Code::KeyC,
        'D' => Code::KeyD,
        'E' => Code::KeyE,
        'F' => Code::KeyF,
        'G' => Code::KeyG,
        'H' => Code::KeyH,
        'I' => Code::KeyI,
        'J' => Code::KeyJ,
        'K' => Code::KeyK,
        'L' => Code::KeyL,
        'M' => Code::KeyM,
        'N' => Code::KeyN,
        'O' => Code::KeyO,
        'P' => Code::KeyP,
        'Q' => Code::KeyQ,
        'R' => Code::KeyR,
        'S' => Code::KeyS,
        'T' => Code::KeyT,
        'U' => Code::KeyU,
        'V' => Code::KeyV,
        'W' => Code::KeyW,
        'X' => Code::KeyX,
        'Y' => Code::KeyY,
        'Z' => Code::KeyZ,
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        _ => return None,
    })
}
