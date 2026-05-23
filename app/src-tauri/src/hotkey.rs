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
    backend: Option<HotkeyBackend>,
}

enum HotkeyBackend {
    Global(RegisteredHotkey),
    #[cfg(target_os = "windows")]
    WindowsModifier(windows_modifier::WindowsModifierMonitor),
}

impl HotkeyMonitor {
    pub fn start(binding: &str, tx: Sender<HotkeyEvent>) -> Result<Self, String> {
        if let Some(trigger) = parse_modifier_only_binding(binding) {
            #[cfg(target_os = "windows")]
            {
                return Ok(Self {
                    backend: Some(HotkeyBackend::WindowsModifier(
                        windows_modifier::WindowsModifierMonitor::start(trigger, tx)?,
                    )),
                });
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = trigger;
            }
        }

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
            backend: Some(HotkeyBackend::Global(registered)),
        })
    }
}

impl Drop for HotkeyMonitor {
    fn drop(&mut self) {
        self.backend.take();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierTrigger {
    AltRight,
    AltLeft,
    ControlRight,
    ControlLeft,
    ShiftRight,
    ShiftLeft,
    MetaRight,
    MetaLeft,
}

fn parse_modifier_only_binding(raw: &str) -> Option<ModifierTrigger> {
    let mut parts = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let only = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    parse_modifier_trigger(only)
}

fn parse_modifier_trigger(raw: &str) -> Option<ModifierTrigger> {
    match normalize_key_name(raw).as_str() {
        "altright" | "rightalt" | "rightoption" | "rightopt" => Some(ModifierTrigger::AltRight),
        "altleft" | "leftalt" | "leftoption" | "leftopt" => Some(ModifierTrigger::AltLeft),
        "controlright" | "rightcontrol" | "ctrlright" | "rightctrl" => {
            Some(ModifierTrigger::ControlRight)
        }
        "controlleft" | "leftcontrol" | "ctrlleft" | "leftctrl" => {
            Some(ModifierTrigger::ControlLeft)
        }
        "shiftright" | "rightshift" => Some(ModifierTrigger::ShiftRight),
        "shiftleft" | "leftshift" => Some(ModifierTrigger::ShiftLeft),
        "metaright" | "rightmeta" | "rightwin" | "winright" | "rightcommand" | "rightcmd" => {
            Some(ModifierTrigger::MetaRight)
        }
        "metaleft" | "leftmeta" | "leftwin" | "winleft" | "leftcommand" | "leftcmd" => {
            Some(ModifierTrigger::MetaLeft)
        }
        _ => None,
    }
}

fn normalize_key_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_'))
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_key(raw: &str) -> Result<Code, String> {
    if raw.chars().count() == 1 {
        if let Some(code) = char_to_code(raw.chars().next().unwrap()) {
            return Ok(code);
        }
    }
    let normalized = normalize_key_name(raw);
    let key = match normalized.as_str() {
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

#[cfg(target_os = "windows")]
mod windows_modifier {
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    use std::sync::mpsc::{self, Sender};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG,
        WH_KEYBOARD_LL, WM_QUIT,
    };

    use super::{HotkeyEvent, ModifierTrigger};

    const WM_KEYDOWN: usize = 0x0100;
    const WM_KEYUP: usize = 0x0101;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const WM_SYSKEYUP: usize = 0x0105;

    const VK_LSHIFT: u32 = 0xA0;
    const VK_RSHIFT: u32 = 0xA1;
    const VK_LCONTROL: u32 = 0xA2;
    const VK_RCONTROL: u32 = 0xA3;
    const VK_LMENU: u32 = 0xA4;
    const VK_RMENU: u32 = 0xA5;
    const VK_LWIN: u32 = 0x5B;
    const VK_RWIN: u32 = 0x5C;
    const LLKHF_INJECTED: u32 = 0x0000_0010;

    static HOOK_CONTEXT: AtomicPtr<CallbackContext> = AtomicPtr::new(std::ptr::null_mut());

    pub struct WindowsModifierMonitor {
        thread_id: u32,
        join: Option<JoinHandle<()>>,
    }

    impl WindowsModifierMonitor {
        pub fn start(trigger: ModifierTrigger, tx: Sender<HotkeyEvent>) -> Result<Self, String> {
            let (status_tx, status_rx) = mpsc::channel::<Result<u32, String>>();
            let join = std::thread::Builder::new()
                .name("typeless-hotkey-win-modifier-hook".into())
                .spawn(move || run_hook_loop(trigger, tx, status_tx))
                .map_err(|err| err.to_string())?;
            let thread_id = match status_rx.recv_timeout(Duration::from_secs(3)) {
                Ok(Ok(thread_id)) => thread_id,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err("Windows 热键监听启动超时".into()),
            };
            Ok(Self {
                thread_id,
                join: Some(join),
            })
        }
    }

    impl Drop for WindowsModifierMonitor {
        fn drop(&mut self) {
            unsafe {
                if let Err(err) = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0))
                {
                    log::warn!("[hotkey] Windows modifier hook quit failed: {err}");
                }
            }
            if let Some(join) = self.join.take() {
                if let Err(err) = join.join() {
                    log::warn!("[hotkey] Windows modifier hook join failed: {err:?}");
                }
            }
        }
    }

    struct CallbackContext {
        trigger: ModifierTrigger,
        held: AtomicBool,
        tx: Sender<HotkeyEvent>,
        hook: std::sync::Mutex<Option<HHOOK>>,
    }

    unsafe impl Send for CallbackContext {}
    unsafe impl Sync for CallbackContext {}

    fn run_hook_loop(
        trigger: ModifierTrigger,
        tx: Sender<HotkeyEvent>,
        status_tx: Sender<Result<u32, String>>,
    ) {
        let thread_id = unsafe { GetCurrentThreadId() };
        let context = Box::into_raw(Box::new(CallbackContext {
            trigger,
            held: AtomicBool::new(false),
            tx,
            hook: std::sync::Mutex::new(None),
        }));
        HOOK_CONTEXT.store(context, Ordering::SeqCst);

        unsafe {
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0) {
                Ok(hook) => {
                    *(*context).hook.lock().unwrap() = Some(hook);
                    let _ = status_tx.send(Ok(thread_id));
                }
                Err(err) => {
                    HOOK_CONTEXT.store(std::ptr::null_mut(), Ordering::SeqCst);
                    let _ = Box::from_raw(context);
                    let _ = status_tx.send(Err(format!("Windows 右 Alt 热键监听安装失败: {err}")));
                    return;
                }
            }

            let mut message = MSG::default();
            loop {
                let result = GetMessageW(&mut message, None, 0, 0).0;
                if result <= 0 {
                    break;
                }
                let _ = TranslateMessage(&message);
                let _ = DispatchMessageW(&message);
            }

            if let Some(hook) = (*context).hook.lock().unwrap().take() {
                let _ = UnhookWindowsHookEx(hook);
            }
            HOOK_CONTEXT.store(std::ptr::null_mut(), Ordering::SeqCst);
            let _ = Box::from_raw(context);
        }
    }

    unsafe extern "system" fn low_level_keyboard_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 && lparam.0 != 0 {
            if let Some(ctx) = callback_context() {
                let keyboard = *(lparam.0 as *const KBDLLHOOKSTRUCT);
                if keyboard.flags.0 & LLKHF_INJECTED == 0
                    && dispatch_keyboard_event(ctx, keyboard.vkCode, wparam.0)
                {
                    return LRESULT(1);
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    unsafe fn callback_context<'a>() -> Option<&'a CallbackContext> {
        let ptr = HOOK_CONTEXT.load(Ordering::SeqCst);
        if ptr.is_null() {
            None
        } else {
            Some(&*ptr)
        }
    }

    fn dispatch_keyboard_event(ctx: &CallbackContext, vk_code: u32, message: usize) -> bool {
        if vk_code != trigger_to_vk_code(ctx.trigger) {
            return false;
        }

        match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let was_held = ctx.held.swap(true, Ordering::SeqCst);
                if !was_held {
                    let _ = ctx.tx.send(HotkeyEvent::Pressed);
                }
                true
            }
            WM_KEYUP | WM_SYSKEYUP => {
                let was_held = ctx.held.swap(false, Ordering::SeqCst);
                if was_held {
                    let _ = ctx.tx.send(HotkeyEvent::Released);
                }
                true
            }
            _ => false,
        }
    }

    fn trigger_to_vk_code(trigger: ModifierTrigger) -> u32 {
        match trigger {
            ModifierTrigger::AltRight => VK_RMENU,
            ModifierTrigger::AltLeft => VK_LMENU,
            ModifierTrigger::ControlRight => VK_RCONTROL,
            ModifierTrigger::ControlLeft => VK_LCONTROL,
            ModifierTrigger::ShiftRight => VK_RSHIFT,
            ModifierTrigger::ShiftLeft => VK_LSHIFT,
            ModifierTrigger::MetaRight => VK_RWIN,
            ModifierTrigger::MetaLeft => VK_LWIN,
        }
    }
}
