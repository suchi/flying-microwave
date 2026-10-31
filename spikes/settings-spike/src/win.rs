//! Thin wrappers over the few Win32 calls the spike needs. All `unsafe` is
//! confined to this module.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{GetLastError, HWND, SetLastError};
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GW_OWNER, GWLP_HWNDPARENT, GetForegroundWindow, GetWindow, IsWindow, MB_ICONERROR, MB_OK,
    MessageBoxW, SetForegroundWindow, SetWindowLongPtrW,
};

/// Window handle type, re-exported so that callers need no `windows_sys` import.
pub type Hwnd = HWND;

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

/// Numeric value of an HWND, for logging and for storing in atomics.
pub fn hwnd_to_usize(hwnd: HWND) -> usize {
    hwnd.expose_provenance()
}

/// Returns true if `hwnd` currently identifies an existing window.
pub fn is_window(hwnd: HWND) -> bool {
    // SAFETY: IsWindow accepts any value, including stale or null handles.
    unsafe { IsWindow(hwnd) != 0 }
}

/// The current foreground window (null if there is none).
pub fn foreground_window() -> HWND {
    // SAFETY: no preconditions.
    unsafe { GetForegroundWindow() }
}

/// Makes `owner` the owner of the top-level window `window`
/// (`SetWindowLongPtrW(GWLP_HWNDPARENT)`). Returns the previous owner value.
pub fn set_owner(window: HWND, owner: HWND) -> Result<usize, u32> {
    // SAFETY: both handles are plain values that Windows validates. The last
    // error is cleared first because a previous value of 0 is also returned
    // on success (see the SetWindowLongPtr documentation).
    unsafe {
        SetLastError(0);
        let previous =
            SetWindowLongPtrW(window, GWLP_HWNDPARENT, owner.expose_provenance() as isize);
        if previous == 0 {
            let error = GetLastError();
            if error != 0 {
                return Err(error);
            }
        }
        Ok(previous as usize)
    }
}

/// The owner of a window as reported by `GetWindow(GW_OWNER)` (null if none).
pub fn owner_of(window: HWND) -> HWND {
    // SAFETY: GetWindow validates the handle.
    unsafe { GetWindow(window, GW_OWNER) }
}

/// `EnableWindow`. Returns true if the window was disabled before the call.
pub fn enable_window(window: HWND, enable: bool) -> bool {
    // SAFETY: EnableWindow validates the handle.
    unsafe { EnableWindow(window, i32::from(enable)) != 0 }
}

/// Whether the window is currently enabled.
pub fn is_window_enabled(window: HWND) -> bool {
    // SAFETY: IsWindowEnabled validates the handle.
    unsafe { IsWindowEnabled(window) != 0 }
}

/// `SetForegroundWindow`; Windows may refuse (returns false).
pub fn set_foreground(window: HWND) -> bool {
    // SAFETY: SetForegroundWindow validates the handle.
    unsafe { SetForegroundWindow(window) != 0 }
}

/// Last Win32 error of the calling thread.
pub fn last_error() -> u32 {
    // SAFETY: no preconditions.
    unsafe { GetLastError() }
}

/// Shows a modal error box (only used in the /c style modes, never in /s or /p).
pub fn error_box(text: &str, caption: &str) {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: both buffers are NUL-terminated UTF-16 and outlive the call; a
    // null owner window is allowed.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
