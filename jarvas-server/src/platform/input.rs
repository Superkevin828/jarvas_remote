use crate::platform::ToolResult;
use crate::safety;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY, VK_ADD,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_ESCAPE,
    VK_F1, VK_HOME, VK_INSERT, VK_LEFT, VK_LWIN, VK_MENU, VK_MULTIPLY, VK_NEXT, VK_NUMLOCK,
    VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_COMMA,
    VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS, VK_PAUSE, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SCROLL,
    VK_SHIFT, VK_SNAPSHOT, VK_SPACE, VK_SUBTRACT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

pub fn move_mouse(x: i32, y: i32) -> ToolResult {
    safety::delay()?;
    unsafe { SetCursorPos(x, y) }
        .map_err(|error| format!("Could not move cursor to ({x}, {y}): {error}"))?;
    safety::log("move_mouse", &format!("x={x}, y={y}"));
    Ok(format!("Cursor moved to ({x}, {y})."))
}

pub fn get_cursor_position() -> ToolResult {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }
        .map_err(|error| format!("Could not read cursor position: {error}"))?;
    Ok(format!("{{\"x\":{}, \"y\":{}}}", point.x, point.y))
}

pub fn click(button: &str, double: bool, x: Option<i32>, y: Option<i32>) -> ToolResult {
    safety::delay()?;
    let button = button.to_ascii_lowercase();
    let (down, up, label) = match button.as_str() {
        "left" | "primary" => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, "left"),
        "right" | "secondary" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, "right"),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, "middle"),
        _ => {
            return Err(format!(
                "Unknown button '{button}'. Use 'left', 'right', or 'middle'."
            ))
        }
    };
    if let (Some(x), Some(y)) = (x, y) {
        unsafe { SetCursorPos(x, y) }.map_err(|error| format!("Could not move cursor: {error}"))?;
    }
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok();
    let times = if double { 2 } else { 1 };
    for index in 0..times {
        if index > 0 {
            std::thread::sleep(std::time::Duration::from_millis(70));
        }
        send_mouse(down, 0, 0, 0)?;
        send_mouse(up, 0, 0, 0)?;
    }
    let detail = format!(
        "{label}{} click at ({}, {})",
        if double { " double" } else { "" },
        point.x,
        point.y
    );
    safety::log("click", &detail);
    Ok(format!("Performed {detail}."))
}

pub fn drag(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> ToolResult {
    safety::delay()?;
    unsafe { SetCursorPos(from_x, from_y) }
        .map_err(|error| format!("Could not start drag: {error}"))?;
    send_mouse(MOUSEEVENTF_LEFTDOWN, 0, 0, 0)?;
    let steps = 24_i32;
    for step in 1..=steps {
        safety::ensure_running()?;
        let x = from_x + ((to_x - from_x) * step) / steps;
        let y = from_y + ((to_y - from_y) * step) / steps;
        if let Err(error) = unsafe { SetCursorPos(x, y) } {
            let _ = send_mouse(MOUSEEVENTF_LEFTUP, 0, 0, 0);
            return Err(format!("Drag interrupted at step {step}: {error}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    send_mouse(MOUSEEVENTF_LEFTUP, 0, 0, 0)?;
    let detail = format!("({from_x},{from_y}) -> ({to_x},{to_y})");
    safety::log("drag", &detail);
    Ok(format!("Dragged {detail}."))
}

pub fn scroll(amount: i32, x: Option<i32>, y: Option<i32>) -> ToolResult {
    safety::delay()?;
    if let (Some(x), Some(y)) = (x, y) {
        unsafe { SetCursorPos(x, y) }.map_err(|error| format!("Could not move cursor: {error}"))?;
    }
    if amount == 0 {
        return Err("Scroll amount must be non-zero (positive = up, negative = down).".to_string());
    }
    let notches = amount.clamp(-20, 20);
    send_mouse(MOUSEEVENTF_WHEEL, 0, 0, (notches * 120) as u32)?;
    safety::log("scroll", &format!("notches={notches}"));
    Ok(format!("Scrolled {notches} notch(es) (positive is up)."))
}

pub fn type_text(text: &str) -> ToolResult {
    safety::delay()?;
    if text.is_empty() {
        return Err("Text must not be empty.".to_string());
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    for chunk in units.chunks(64) {
        safety::ensure_running()?;
        let mut inputs = Vec::with_capacity(chunk.len() * 2);
        for &unit in chunk {
            inputs.push(keyboard_unicode_input(unit, false));
            inputs.push(keyboard_unicode_input(unit, true));
        }
        send_inputs(&inputs, "typing text")?;
    }
    safety::log(
        "type_text",
        &format!(
            "chars={}, utf16_units={}",
            text.chars().count(),
            units.len()
        ),
    );
    Ok(format!("Typed {} character(s).", text.chars().count()))
}

pub fn press_key(key: &str) -> ToolResult {
    safety::delay()?;
    let virtual_key = virtual_key(key)?;
    tap(virtual_key)?;
    safety::log("press_key", key);
    Ok(format!("Pressed '{key}'."))
}

pub fn tap_alt() -> Result<(), String> {
    tap(VK_MENU)
}

pub fn hotkey(combo: &str) -> ToolResult {
    safety::delay()?;
    let parts: Vec<&str> = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return Err(
            "Hotkey needs at least one modifier and one key, e.g. 'Ctrl+C', 'Alt+Tab', 'Win+R'."
                .to_string(),
        );
    }
    let mut modifiers: Vec<VIRTUAL_KEY> = Vec::new();
    let mut main_keys: Vec<VIRTUAL_KEY> = Vec::new();
    for part in &parts {
        let key = virtual_key(part)?;
        if is_modifier(key) {
            modifiers.push(key);
        } else {
            main_keys.push(key);
        }
    }
    if modifiers.is_empty() {
        return Err(
            "No modifier found. Include Ctrl, Alt, Shift, or Win (e.g. 'Ctrl+S').".to_string(),
        );
    }
    if main_keys.len() != 1 {
        return Err(
            "A hotkey must contain exactly one non-modifier key (e.g. 'Ctrl+Shift+S').".to_string(),
        );
    }
    for &modifier in &modifiers {
        key_down(modifier)?;
    }
    for &key in &main_keys {
        key_down(key)?;
    }
    for &key in main_keys.iter().rev() {
        key_up(key)?;
    }
    for &modifier in modifiers.iter().rev() {
        key_up(modifier)?;
    }
    safety::log("hotkey", combo);
    Ok(format!("Pressed hotkey '{combo}'."))
}

fn send_mouse(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, data: u32) -> Result<(), String> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    send_inputs(std::slice::from_ref(&input), "mouse input")
}

fn keyboard_unicode_input(scan: u16, up: bool) -> INPUT {
    let flags = if up {
        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
    } else {
        KEYEVENTF_UNICODE
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_input(virtual_key: VIRTUAL_KEY, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(virtual_key.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    let flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_down(virtual_key: VIRTUAL_KEY) -> Result<(), String> {
    send_inputs(
        std::slice::from_ref(&key_input(virtual_key, false)),
        "key down",
    )
}

fn key_up(virtual_key: VIRTUAL_KEY) -> Result<(), String> {
    send_inputs(
        std::slice::from_ref(&key_input(virtual_key, true)),
        "key up",
    )
}

fn tap(virtual_key: VIRTUAL_KEY) -> Result<(), String> {
    let inputs = [key_input(virtual_key, false), key_input(virtual_key, true)];
    send_inputs(&inputs, "key press")
}

fn send_inputs(inputs: &[INPUT], what: &str) -> Result<(), String> {
    let size = std::mem::size_of::<INPUT>() as i32;
    let sent = unsafe { SendInput(inputs, size) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!("Windows rejected {what}: SentInput inserted {sent} of {} events. Another program may be blocking input.", inputs.len()))
    }
}

fn is_modifier(key: VIRTUAL_KEY) -> bool {
    matches!(key, VK_CONTROL | VK_SHIFT | VK_MENU | VK_LWIN)
}

fn virtual_key(name: &str) -> Result<VIRTUAL_KEY, String> {
    let normalized = name.trim().to_ascii_lowercase();
    let key = match normalized.as_str() {
        "ctrl" | "control" | "controlbreak" => VIRTUAL_KEY(VK_CONTROL.0),
        "alt" | "menu" => VIRTUAL_KEY(VK_MENU.0),
        "shift" => VIRTUAL_KEY(VK_SHIFT.0),
        "win" | "windows" | "super" | "meta" => VIRTUAL_KEY(VK_LWIN.0),
        "enter" | "return" => VK_RETURN,
        "esc" | "escape" => VK_ESCAPE,
        "tab" => VK_TAB,
        "space" | "spacebar" => VK_SPACE,
        "backspace" => VK_BACK,
        "delete" | "del" => VK_DELETE,
        "insert" | "ins" => VK_INSERT,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" | "pgup" => VK_PRIOR,
        "pagedown" | "pgdn" => VK_NEXT,
        "up" | "arrowup" => VK_UP,
        "down" | "arrowdown" => VK_DOWN,
        "left" | "arrowleft" => VK_LEFT,
        "right" | "arrowright" => VK_RIGHT,
        "capslock" | "caps" => VK_CAPITAL,
        "numlock" => VK_NUMLOCK,
        "scrolllock" => VK_SCROLL,
        "printscreen" | "prtsc" => VK_SNAPSHOT,
        "pause" | "break" => VK_PAUSE,
        "add" | "numpadadd" => VK_ADD,
        "subtract" | "numpadsubtract" => VK_SUBTRACT,
        "multiply" | "numpadmultiply" => VK_MULTIPLY,
        "divide" | "numpaddivide" => VK_DIVIDE,
        "decimal" | "numpaddecimal" => VK_DECIMAL,
        "." | "period" => VK_OEM_PERIOD,
        "," | "comma" => VK_OEM_COMMA,
        "-" | "minus" => VK_OEM_MINUS,
        "=" | "equals" | "plus" => VK_OEM_PLUS,
        "/" | "slash" => VK_OEM_2,
        "\\" | "backslash" => VK_OEM_5,
        ";" | "semicolon" => VK_OEM_1,
        "'" | "quote" => VK_OEM_7,
        "[" | "openbracket" => VK_OEM_4,
        "]" | "closebracket" => VK_OEM_6,
        "`" | "backtick" => VK_OEM_3,
        _ => {
            if normalized.len() == 1 {
                let byte = normalized.as_bytes()[0];
                if byte.is_ascii_alphanumeric() {
                    VIRTUAL_KEY(byte.to_ascii_uppercase() as u16)
                } else {
                    return Err(format!("Unsupported key '{name}'. Use names like 'enter', 'esc', 'space', 'f1', 'a', or 'ctrl'."));
                }
            } else if let Some(number) = normalized.strip_prefix('f') {
                let function_number = number
                    .parse::<u16>()
                    .map_err(|_| format!("Unknown key '{name}'."))?;
                if !(1..=12).contains(&function_number) {
                    return Err(format!("Function key must be F1-F12, got '{name}'."));
                }
                VIRTUAL_KEY(VK_F1.0 + function_number - 1)
            } else {
                return Err(format!("Unsupported key '{name}'. Use names like 'enter', 'esc', 'space', 'f1', 'a', or 'ctrl'."));
            }
        }
    };
    Ok(key)
}
