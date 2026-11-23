use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::core::BOOL;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

const HOTKEY_ID: i32 = 0x4a415641_u32 as i32;
const HOTKEY_VK: u32 = 0x4a;

pub struct Safety {
    stopped: AtomicBool,
    delay_ms: u64,
    log_path: PathBuf,
    entries: Mutex<VecDeque<String>>,
}

static SAFETY: OnceLock<Safety> = OnceLock::new();

pub fn state() -> &'static Safety {
    SAFETY.get_or_init(|| {
        let delay_ms = std::env::var("JARVAS_ACTION_DELAY_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(150);
        let log_path = std::env::var_os("JARVAS_LOG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|path| path.parent().map(|dir| dir.join("jarvas.log")))
                    .unwrap_or_else(|| PathBuf::from("jarvas.log"))
            });
        Safety {
            stopped: AtomicBool::new(false),
            delay_ms,
            log_path,
            entries: Mutex::new(VecDeque::new()),
        }
    })
}

pub fn init_panic_hotkey() {
    let safety = state();
    log("system", "starting; panic hotkey is Ctrl+Shift+J");
    std::thread::spawn(move || {
        if let Err(error) = unsafe {
            RegisterHotKey(
                None,
                HOTKEY_ID,
                MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                HOTKEY_VK,
            )
        } {
            eprintln!("[JARVAS] could not register panic hotkey Ctrl+Shift+J: {error}");
            return;
        }
        let mut message = MSG::default();
        loop {
            let result: BOOL = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if result.0 <= 0 {
                break;
            }
            if message.message == WM_HOTKEY {
                safety.stopped.store(true, Ordering::SeqCst);
                eprintln!("[JARVAS] PANIC HOTKEY PRESSED - all further actions are blocked");
                let _ =
                    safety.append_log("PANIC", "Ctrl+Shift+J pressed; all further actions blocked");
            }
        }
    });
}

pub fn ensure_running() -> Result<(), String> {
    if state().stopped.load(Ordering::SeqCst) {
        Err("EMERGENCY STOP ACTIVE: Ctrl+Shift+J was pressed. All actions are blocked until JARVAS is restarted.".to_string())
    } else {
        Ok(())
    }
}

pub fn delay() -> Result<(), String> {
    let steps = (state().delay_ms / 10).max(1) as usize;
    for _ in 0..steps {
        ensure_running()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    ensure_running()
}

pub fn log(action: &str, detail: &str) {
    let safety = state();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let line = format!("{millis} | {action} | {detail}");
    eprintln!("[JARVAS] {line}");
    let _ = safety.append_log(action, detail);
}

impl Safety {
    fn append_log(&self, action: &str, detail: &str) -> std::io::Result<()> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or(0);
        let line = format!("{millis} | {action} | {detail}");
        if let Ok(mut entries) = self.entries.lock() {
            entries.push_back(line.clone());
            while entries.len() > 200 {
                entries.pop_front();
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "{line}")
    }
}

pub fn action_log(limit: usize) -> String {
    let entries = state().entries.lock().expect("action log lock poisoned");
    let skip = entries.len().saturating_sub(limit);
    let lines: Vec<&String> = entries.iter().skip(skip).collect();
    if lines.is_empty() {
        "No actions logged yet.".to_string()
    } else {
        lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn require_confirm(confirm: bool, action: &str) -> Result<(), String> {
    if confirm {
        Ok(())
    } else {
        Err(format!("'{action}' is destructive and requires confirm=true. Set confirm=true in the tool call to proceed."))
    }
}
