use crate::platform::ToolResult;
use crate::safety;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};

pub fn get_clipboard() -> ToolResult {
    unsafe {
        OpenClipboard(None)
            .map_err(|error| format!("Could not open the clipboard: {error}. Another app may be using it."))?;
        let data = GetClipboardData(CF_UNICODETEXT.0 as u32);
        let _ = CloseClipboard();
        let handle: HANDLE = data.map_err(|_| "Clipboard does not contain text. Copy some text first or call set_clipboard.".to_string())?;
        let pointer = GlobalLock(HGLOBAL(handle.0));
        if pointer.is_null() {
            return Err("Could not read clipboard memory.".to_string());
        }
        let words = pointer as *const u16;
        let mut length = 0usize;
        while length < 1_000_000 && *words.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(words, length));
        let _ = GlobalUnlock(HGLOBAL(handle.0));
        Ok(text)
    }
}

pub fn set_clipboard(text: &str) -> ToolResult {
    safety::delay()?;
    unsafe {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes = wide.len() * std::mem::size_of::<u16>();
        let handle = GlobalAlloc(GHND, bytes)
            .map_err(|error| format!("Could not allocate clipboard memory: {error}"))?;
        let pointer = GlobalLock(handle);
        if pointer.is_null() {
            let _ = GlobalFree(Some(handle));
            return Err("Could not lock clipboard memory.".to_string());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer as *mut u16, wide.len());
        let _ = GlobalUnlock(handle);

        OpenClipboard(None).map_err(|error| {
            let _ = GlobalFree(Some(handle));
            format!("Could not open the clipboard: {error}. Another app may be using it.")
        })?;
        let _ = EmptyClipboard();
        if let Err(error) = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(handle.0))) {
            let _ = CloseClipboard();
            let _ = GlobalFree(Some(handle));
            return Err(format!("Could not write text to the clipboard: {error}"));
        }
        let _ = CloseClipboard();
    }
    safety::log("set_clipboard", &format!("chars={}", text.chars().count()));
    Ok(format!("Clipboard set to {} character(s).", text.chars().count()))
}
