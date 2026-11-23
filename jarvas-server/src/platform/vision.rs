use crate::safety;
use image::ImageEncoder;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowRect, IsIconic, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

pub struct Screenshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub saved_to: Option<String>,
}

pub fn get_screen_size() -> Result<String, String> {
    unsafe {
        Ok(format!(
            "{{\"width\":{}, \"height\":{}, \"virtual_left\":{}, \"virtual_top\":{}, \"virtual_width\":{}, \"virtual_height\":{}}}",
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN)
        ))
    }
}

pub fn capture(window_handle: Option<u64>, path: Option<&str>) -> Result<Screenshot, String> {
    safety::delay()?;
    let (left, top, width, height, hwnd) = match window_handle {
        Some(handle) => {
            let handle = HWND(handle as usize as *mut c_void);
            let mut rect = RECT::default();
            unsafe { GetWindowRect(handle, &mut rect) }.map_err(|error| {
                format!(
                    "Window 0x{:X} has no valid bounds: {error}",
                    handle.0 as u64
                )
            })?;
            if unsafe { IsIconic(handle) }.as_bool() {
                return Err(format!(
                    "Window 0x{:X} is minimized; restore it before taking a screenshot.",
                    handle.0 as u64
                ));
            }
            (
                rect.left,
                rect.top,
                (rect.right - rect.left).max(1),
                (rect.bottom - rect.top).max(1),
                Some(handle),
            )
        }
        None => unsafe {
            (
                0,
                0,
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
                None,
            )
        },
    };
    if width <= 0 || height <= 0 {
        return Err(
            "Screen capture area is empty. Call get_screen_size to verify the display.".to_string(),
        );
    }

    let png = unsafe { grab(left, top, width, height) }?;
    let saved_to = match path {
        Some(path) => {
            std::fs::write(path, &png)
                .map_err(|error| format!("Could not save screenshot to '{path}': {error}"))?;
            Some(path.to_string())
        }
        None => None,
    };
    let detail = format!(
        "width={width}, height={height}, target={}",
        hwnd.map(|handle| format!("window 0x{:X}", handle.0 as u64))
            .unwrap_or_else(|| "full screen".to_string())
    );
    safety::log("screenshot", &detail);
    Ok(Screenshot {
        png,
        width: width as u32,
        height: height as u32,
        saved_to,
    })
}

unsafe fn grab(left: i32, top: i32, width: i32, height: i32) -> Result<Vec<u8>, String> {
    let screen_dc = GetDC(None);
    if screen_dc.is_invalid() {
        return Err("Could not access the screen device context.".to_string());
    }
    let memory_dc = CreateCompatibleDC(Some(screen_dc));
    if memory_dc.is_invalid() {
        ReleaseDC(None, screen_dc);
        return Err("Could not create a compatible drawing context.".to_string());
    }
    let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
    if bitmap.is_invalid() {
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);
        return Err("Could not create a compatible bitmap.".to_string());
    }
    let old_bitmap = SelectObject(memory_dc, bitmap.into());

    BitBlt(
        memory_dc,
        0,
        0,
        width,
        height,
        Some(screen_dc),
        left,
        top,
        SRCCOPY,
    )
    .map_err(|error| format!("Could not capture the screen: {error}"))?;

    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let rows = GetDIBits(
        memory_dc,
        bitmap,
        0,
        height as u32,
        Some(pixels.as_mut_ptr() as *mut c_void),
        &mut info,
        DIB_RGB_COLORS,
    );
    let _ = SelectObject(memory_dc, old_bitmap);
    let _ = DeleteObject(bitmap.into());
    let _ = DeleteDC(memory_dc);
    ReleaseDC(None, screen_dc);
    if rows == 0 {
        return Err("Could not read captured pixels from Windows.".to_string());
    }

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &pixels,
            width as u32,
            height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("Could not encode screenshot as PNG: {error}"))?;
    Ok(png)
}
