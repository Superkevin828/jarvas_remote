use crate::platform::ToolResult;
use crate::safety;
use std::ffi::c_void;
use std::path::Path;
use std::process::Command;
use windows::core::{w, PCWSTR};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow,
    IsWindowVisible, MoveWindow, PostMessageW, SetForegroundWindow, ShowWindow, SHOW_WINDOW_CMD,
    SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
};

#[derive(Clone, Debug)]
struct WindowInfo {
    handle: u64,
    title: String,
    class: String,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

pub fn list_windows() -> ToolResult {
    let mut found: Vec<WindowInfo> = Vec::new();
    if let Err(error) = unsafe { EnumWindows(Some(enum_window), LPARAM(&mut found as *mut Vec<WindowInfo> as isize)) } {
        return Err(format!("Could not enumerate windows: {error}"));
    }
    let items: Vec<String> = found
        .iter()
        .map(|window| {
            format!(
                "{{\"handle\":{}, \"handle_hex\":\"0x{:X}\", \"title\":{}, \"class\":{}, \"left\":{}, \"top\":{}, \"width\":{}, \"height\":{}}}",
                window.handle,
                window.handle,
                serde_json::to_string(&window.title).unwrap_or_default(),
                serde_json::to_string(&window.class).unwrap_or_default(),
                window.left,
                window.top,
                window.width,
                window.height
            )
        })
        .collect();
    Ok(format!(
        "{{\"count\":{}, \"windows\":[{}]}}",
        items.len(),
        items.join(", ")
    ))
}

unsafe extern "system" fn enum_window(handle: HWND, data: LPARAM) -> BOOL {
    let windows = &mut *(data.0 as *mut Vec<WindowInfo>);
    if !IsWindowVisible(handle).as_bool() {
        return TRUE;
    }
    let length = GetWindowTextLengthW(handle);
    if length <= 0 {
        return TRUE;
    }
    let mut title = vec![0_u16; length as usize + 1];
    let read = GetWindowTextW(handle, &mut title);
    if read <= 0 {
        return TRUE;
    }
    let mut class = vec![0_u16; 256];
    let class_read = GetClassNameW(handle, &mut class);
    let mut rect = RECT::default();
    let _ = GetWindowRect(handle, &mut rect);
    windows.push(WindowInfo {
        handle: handle.0 as u64,
        title: String::from_utf16_lossy(&title[..read as usize]),
        class: if class_read > 0 {
            String::from_utf16_lossy(&class[..class_read as usize])
        } else {
            String::new()
        },
        left: rect.left,
        top: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    });
    TRUE
}

pub fn focus_window(window_handle: Option<u64>, title_contains: Option<&str>) -> ToolResult {
    safety::delay()?;
    let handle = resolve(window_handle, title_contains)?;
    unsafe {
        let _ = ShowWindow(handle, SW_RESTORE);
        if !SetForegroundWindow(handle).as_bool() {
            let _ = crate::platform::input::tap_alt();
            let _ = SetForegroundWindow(handle);
        }
    }
    let description = describe(handle)?;
    safety::log("focus_window", &description);
    Ok(format!("Focused window: {description}"))
}

pub fn close_window(window_handle: Option<u64>, title_contains: Option<&str>, confirm: bool) -> ToolResult {
    safety::delay()?;
    safety::require_confirm(confirm, "close_window")?;
    let handle = resolve(window_handle, title_contains)?;
    let description = describe(handle)?;
    unsafe { PostMessageW(Some(handle), WM_CLOSE, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("Could not send close request to '{description}': {error}"))?;
    safety::log("close_window", &description);
    Ok(format!("Requested close of window: {description}"))
}

pub fn minimize(window_handle: Option<u64>, title_contains: Option<&str>) -> ToolResult {
    safety::delay()?;
    let handle = resolve(window_handle, title_contains)?;
    unsafe { ShowWindow(handle, SW_MINIMIZE); }
    let description = describe(handle)?;
    safety::log("minimize", &description);
    Ok(format!("Minimized window: {description}"))
}

pub fn maximize(window_handle: Option<u64>, title_contains: Option<&str>) -> ToolResult {
    safety::delay()?;
    let handle = resolve(window_handle, title_contains)?;
    unsafe { ShowWindow(handle, SW_MAXIMIZE); }
    let description = describe(handle)?;
    safety::log("maximize", &description);
    Ok(format!("Maximized window: {description}"))
}

pub fn restore_window(window_handle: Option<u64>, title_contains: Option<&str>) -> ToolResult {
    safety::delay()?;
    let handle = resolve(window_handle, title_contains)?;
    unsafe { ShowWindow(handle, SW_RESTORE); }
    let description = describe(handle)?;
    safety::log("restore_window", &description);
    Ok(format!("Restored window: {description}"))
}

pub fn move_window(
    window_handle: Option<u64>,
    title_contains: Option<&str>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> ToolResult {
    safety::delay()?;
    if width <= 0 || height <= 0 {
        return Err("Width and height must be positive numbers.".to_string());
    }
    let handle = resolve(window_handle, title_contains)?;
    unsafe { MoveWindow(handle, x, y, width, height, true) }
        .map_err(|error| format!("Could not move/resize window: {error}"))?;
    let detail = format!("handle=0x{:X}, x={x}, y={y}, width={width}, height={height}", handle.0 as u64);
    safety::log("move_window", &detail);
    Ok(format!("Moved/resized window ({detail})."))
}

pub fn launch_app(name_or_path: &str, args: Option<&str>) -> ToolResult {
    safety::delay()?;
    let expanded = expand_environment(name_or_path);
    let arguments = split_arguments(args);
    let trimmed = expanded.trim();
    if trimmed.is_empty() {
        return Err("name_or_path must not be empty.".to_string());
    }

    if Path::new(trimmed).exists() {
        if is_executable(trimmed) {
            Command::new(trimmed)
                .args(&arguments)
                .spawn()
                .map_err(|error| {
                    format!("Found '{trimmed}' but could not start it: {error}. Check that it is a valid Windows executable.")
                })?;
        } else {
            shell_open(trimmed, &arguments.join(" "))?;
        }
    } else if trimmed.contains('\\') || trimmed.contains('/') {
        return Err(format!(
            "No file exists at '{trimmed}'. Check the path, or pass just an app name such as 'notepad' or 'calc'."
        ));
    } else {
        let spawned = Command::new(trimmed).args(&arguments).spawn();
        if spawned.is_err() {
            shell_open(trimmed, &arguments.join(" "))?;
        }
    }

    safety::log("launch_app", &format!("name_or_path={trimmed}, args={:?}", arguments));
    Ok(format!("Launched '{trimmed}'."))
}

pub fn close_app(process_name: &str, confirm: bool) -> ToolResult {
    safety::delay()?;
    safety::require_confirm(confirm, "close_app")?;
    let target = process_name.trim().trim_end_matches(".exe").to_ascii_lowercase();
    if target.is_empty() {
        return Err("process_name must not be empty.".to_string());
    }
    let pids = process_ids(&target)?;
    let mut stopped = 0usize;
    for pid in pids {
        if pid == std::process::id() {
            continue;
        }
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                if TerminateProcess(handle, 1).is_ok() {
                    stopped += 1;
                }
            }
        }
    }
    safety::log("close_app", &format!("process={process_name}, stopped={stopped}"));
    if stopped == 0 {
        Err(format!(
            "No running process named '{process_name}' was found. Use list_windows to find the app, or check the process name (for example 'notepad.exe')."
        ))
    } else {
        Ok(format!("Closed {stopped} process(es) named '{process_name}'."))
    }
}

fn process_ids(target: &str) -> Result<Vec<u32>, String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|error| format!("Could not read running processes: {error}"))?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut result = Vec::new();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_err() {
            return Err("Could not read the first running process.".to_string());
        }
        loop {
            let name = String::from_utf16_lossy(&entry.szExeFile)
                .trim_end_matches('\0')
                .to_ascii_lowercase();
            if name.trim_end_matches(".exe") == target {
                result.push(entry.th32ProcessID);
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
    }
    Ok(result)
}

fn shell_open(file: &str, parameters: &str) -> Result<(), String> {
    let file_wide: Vec<u16> = format!("{file}\0").encode_utf16().collect();
    let parameters_wide: Vec<u16> = format!("{parameters}\0").encode_utf16().collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR::from_raw(file_wide.as_ptr()),
            PCWSTR::from_raw(parameters_wide.as_ptr()),
            PCWSTR::null(),
            SHOW_WINDOW_CMD(5),
        )
    };
    if result.0 as usize > 32 {
        Ok(())
    } else {
        Err(format!(
            "Could not open '{file}' (Windows error code {}). Check the name/path and try a full path.",
            result.0 as usize
        ))
    }
}

fn is_executable(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("exe") | Some("com") | Some("bat") | Some("cmd") | Some("ps1")
    )
}

fn expand_environment(value: &str) -> String {
    let mut result = value.to_string();
    for (key, replacement) in [
        ("USERPROFILE", std::env::var("USERPROFILE").unwrap_or_default()),
        ("LOCALAPPDATA", std::env::var("LOCALAPPDATA").unwrap_or_default()),
        ("APPDATA", std::env::var("APPDATA").unwrap_or_default()),
        ("TEMP", std::env::var("TEMP").unwrap_or_default()),
        ("ProgramFiles", std::env::var("ProgramFiles").unwrap_or_default()),
    ] {
        result = result.replace(&format!("%{key}%"), &replacement);
    }
    result
}

fn split_arguments(args: Option<&str>) -> Vec<String> {
    args.unwrap_or("")
        .split_whitespace()
        .map(|item| item.to_string())
        .collect()
}

fn resolve(window_handle: Option<u64>, title_contains: Option<&str>) -> Result<HWND, String> {
    if let Some(handle) = window_handle {
        let hwnd = HWND(handle as usize as *mut c_void);
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(format!("Window handle 0x{handle:X} is not a valid window. Call list_windows for current handles."));
        }
        return Ok(hwnd);
    }
    if let Some(fragment) = title_contains {
        let windows = list_windows()?;
        let value: serde_json::Value = serde_json::from_str(&windows)
            .map_err(|error| format!("Could not parse window list: {error}"))?;
        let needle = fragment.to_ascii_lowercase();
        if let Some(items) = value["windows"].as_array() {
            for item in items {
                let title = item["title"].as_str().unwrap_or("").to_ascii_lowercase();
                if title.contains(&needle) {
                    let handle = item["handle"].as_u64().unwrap_or(0);
                    return Ok(HWND(handle as usize as *mut c_void));
                }
            }
        }
        return Err(format!(
            "No visible window title contains '{fragment}'. Call list_windows to see exact titles, then pass a title fragment or window_handle."
        ));
    }
    Err("Provide either window_handle or title_contains to identify the window.".to_string())
}

fn describe(handle: HWND) -> Result<String, String> {
    let length = unsafe { GetWindowTextLengthW(handle) };
    if length <= 0 {
        return Ok(format!("handle=0x{:X} (untitled)", handle.0 as u64));
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    let read = unsafe { GetWindowTextW(handle, &mut buffer) };
    Ok(format!(
        "handle=0x{:X}, title='{}'",
        handle.0 as u64,
        String::from_utf16_lossy(&buffer[..read.max(0) as usize])
    ))
}
