//! Thin wrappers over the few Win32 calls the spike needs. All `unsafe` is
//! confined to this module.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{GetLastError, HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, FillRect, GetDC, GetStockObject, ReleaseDC};
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, IsWindow, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MessageBoxW,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Which icon a message box gets.
#[derive(Clone, Copy)]
pub enum MessageIcon {
    Information,
    Warning,
}

/// SEC-1: restrict the default DLL search path to System32.
///
/// Returns `Ok(())` or the Win32 error code. It only affects DLLs loaded after
/// the call (for example the ones wgpu loads by bare name), not the implicit
/// imports resolved before `main`.
pub fn restrict_dll_search_path() -> Result<(), u32> {
    // SAFETY: plain call with a valid flag constant; no pointers involved.
    let ok = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if ok != 0 {
        Ok(())
    } else {
        // SAFETY: GetLastError has no preconditions.
        Err(unsafe { GetLastError() })
    }
}

/// Converts the unsigned decimal handle from the command line into an HWND.
pub fn hwnd_from_usize(value: usize) -> HWND {
    std::ptr::with_exposed_provenance_mut::<c_void>(value)
}

/// Returns true if `hwnd` currently identifies an existing window.
pub fn is_window(hwnd: HWND) -> bool {
    // SAFETY: IsWindow accepts any value, including stale or null handles.
    unsafe { IsWindow(hwnd) != 0 }
}

/// Client rectangle size `(width, height)` of a window, in the units Windows
/// reports to this (per-monitor DPI aware) process.
pub fn client_size(hwnd: HWND) -> Result<(i32, i32), u32> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `rect` is a valid, writable RECT for the duration of the call.
    let ok = unsafe { GetClientRect(hwnd, &mut rect) };
    if ok != 0 {
        Ok((rect.right - rect.left, rect.bottom - rect.top))
    } else {
        // SAFETY: GetLastError has no preconditions.
        Err(unsafe { GetLastError() })
    }
}

/// Cursor position in screen coordinates (physical pixels for a DPI aware process).
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is a valid, writable POINT for the duration of the call.
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x, point.y))
}

/// Returns the HWND of a winit window (Win32 only).
pub fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

/// Paints the whole client area of a window opaque black with GDI.
///
/// This covers the screen before (or without) any GPU work (requirement W-9).
/// Returns a short description of the outcome for the log.
pub fn fill_black(hwnd: HWND) -> &'static str {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `rect` is a valid, writable RECT; the DC obtained from GetDC is
    // released before returning; the stock brush must not be deleted.
    unsafe {
        if GetClientRect(hwnd, &mut rect) == 0 {
            return "GetClientRect failed";
        }
        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return "GetDC failed";
        }
        let brush = GetStockObject(BLACK_BRUSH);
        let filled = FillRect(hdc, &rect, brush);
        ReleaseDC(hwnd, hdc);
        if filled != 0 {
            "filled black"
        } else {
            "FillRect failed"
        }
    }
}

/// Shows a modal message box (only used for /c and unrecognized arguments,
/// never in /s or /p). Returns the button id or the Win32 error code.
pub fn message_box(text: &str, caption: &str, icon: MessageIcon) -> Result<i32, u32> {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
    let icon = match icon {
        MessageIcon::Information => MB_ICONINFORMATION,
        MessageIcon::Warning => MB_ICONWARNING,
    };
    // SAFETY: both buffers are NUL-terminated UTF-16 and outlive the call; a
    // null owner window is allowed.
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | icon,
        )
    };
    if result != 0 {
        Ok(result)
    } else {
        // SAFETY: GetLastError has no preconditions.
        Err(unsafe { GetLastError() })
    }
}
