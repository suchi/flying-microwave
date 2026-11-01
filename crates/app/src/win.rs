//! Small Win32 helpers. Every `unsafe` block calls one documented Win32 API
//! with arguments that are valid for the duration of the call.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, POINT, RECT,
};
use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, FillRect, GetDC, GetStockObject, ReleaseDC};
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, TerminateProcess};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, MB_ICONINFORMATION, MB_OK, MessageBoxW, SetForegroundWindow,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// SEC-1 (docs/design/architecture.md): load DLLs only from System32, so a
/// DLL planted next to the .scr is never picked up. Called first in `main`.
pub fn restrict_dll_search_path() -> Result<(), u32> {
    // SAFETY: takes a flags value only.
    let ok = unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if ok != 0 {
        Ok(())
    } else {
        // SAFETY: reads the calling thread's last error.
        Err(unsafe { GetLastError() })
    }
}

/// Holds the single-instance mutex for the lifetime of the process.
pub struct SingleInstance {
    _handle: HANDLE,
}

/// Returns `None` when another `/s` instance of this user session is running.
/// If the mutex cannot be created at all, the screensaver still runs.
pub fn single_instance() -> Option<SingleInstance> {
    let name: Vec<u16> = "Local\\FlyingMicrowave.Screensaver"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    // SAFETY: reads the calling thread's last error, set by CreateMutexW.
    let already = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if already {
        None
    } else {
        Some(SingleInstance { _handle: handle })
    }
}

/// Ends the process at once (used by the GPU initialization watchdog, when the
/// main thread may be stuck inside a driver call).
pub fn terminate_now(code: u32) -> ! {
    // SAFETY: terminates the current process; the pseudo handle is always valid.
    unsafe {
        TerminateProcess(GetCurrentProcess(), code);
    }
    std::process::exit(code as i32)
}

/// Cursor position in screen coordinates (physical pixels).
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is a valid, writable POINT.
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x, point.y))
}

/// Brings the window to the foreground so that it receives keyboard input.
///
/// Why: winit's `focus_window` injects an extended Alt key press with
/// SendInput when the window is not in the foreground, and that key press
/// ended the screensaver at once (docs/research/spike-results.md).
pub fn set_foreground(hwnd: HWND) -> bool {
    // SAFETY: `hwnd` comes from a live winit window.
    unsafe { SetForegroundWindow(hwnd) != 0 }
}

/// The Win32 window handle of a winit window.
pub fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut c_void),
        _ => None,
    }
}

/// W-9: paints the window black with GDI before any GPU work, so the screen
/// is covered even if the GPU never draws.
pub fn fill_black(hwnd: HWND) -> bool {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `hwnd` comes from a live winit window; the DC is released
    // before returning and the stock brush needs no cleanup.
    unsafe {
        if GetClientRect(hwnd, &mut rect) == 0 {
            return false;
        }
        let hdc = GetDC(hwnd);
        if hdc.is_null() {
            return false;
        }
        let filled = FillRect(hdc, &rect, GetStockObject(BLACK_BRUSH));
        ReleaseDC(hwnd, hdc);
        filled != 0
    }
}

/// Shows an information message box without an owner window.
pub fn message_box(text: &str, caption: &str) {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
