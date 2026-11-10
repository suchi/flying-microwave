//! Small Win32 helpers. Every `unsafe` block calls one documented Win32 API
//! with arguments that are valid for the duration of the call.

use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, POINT, RECT, SetLastError,
};
use windows_sys::Win32::Graphics::Gdi::{
    BLACK_BRUSH, FillRect, GetDC, GetMonitorInfoW, GetStockObject, MONITORINFO, ReleaseDC,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};
use windows_sys::Win32::System::Power::{
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, POWERBROADCAST_SETTING,
    PowerSettingRegisterNotification, PowerSettingUnregisterNotification,
};
use windows_sys::Win32::System::SystemServices::GUID_SESSION_DISPLAY_STATUS;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, TerminateProcess};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Fonts, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_CALLBACK, GWLP_HWNDPARENT, GetClientRect, GetCursorPos, GetForegroundWindow,
    IsWindow, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
    PBT_POWERSETTINGCHANGE, SetForegroundWindow, SetWindowLongPtrW,
};
use windows_sys::core::{GUID, PWSTR};
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use flying_microwave_core::monitors::Area;

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

/// The area of a monitor on the desktop, in physical pixels; `None` when
/// Windows does not know the monitor (it was just removed).
///
/// Why not `MonitorHandle::size`: winit 0.30.13 unwraps the result of
/// `GetMonitorInfoW` there (platform_impl/windows/monitor.rs), which panics
/// when the monitor is gone.
pub fn monitor_area(monitor: &MonitorHandle) -> Option<Area> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    // Why the cast: winit declares `HMONITOR` as `isize`, windows-sys as a pointer.
    let hmonitor = monitor.hmonitor() as windows_sys::Win32::Graphics::Gdi::HMONITOR;
    // SAFETY: `info` is a valid, writable MONITORINFO with `cbSize` set; a
    // monitor handle that is no longer valid only makes the call fail.
    let ok = unsafe { GetMonitorInfoW(hmonitor, &mut info) };
    if ok == 0 {
        return None;
    }
    let rect = info.rcMonitor;
    let width = u32::try_from(rect.right.checked_sub(rect.left)?).ok()?;
    let height = u32::try_from(rect.bottom.checked_sub(rect.top)?).ok()?;
    Some(Area {
        x: rect.left,
        y: rect.top,
        width,
        height,
    })
}

/// The window handle passed on the command line (`/p <HWND>`).
///
/// Why the cast: Windows passes the handle as a decimal number; HWND values
/// fit in a pointer, and anything else fails the `is_window` check.
pub fn hwnd_from_arg(value: u64) -> HWND {
    value as usize as HWND
}

/// SEC-3: whether `hwnd` is currently a valid window. A handle from the
/// command line must pass this check before it is used, and may stop being
/// valid at any time afterwards.
pub fn is_window(hwnd: HWND) -> bool {
    // SAFETY: IsWindow accepts any value and only reads it.
    unsafe { IsWindow(hwnd) != 0 }
}

/// Size of the client area of `hwnd` in physical pixels.
pub fn client_size(hwnd: HWND) -> Option<(u32, u32)> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `rect` is a valid, writable RECT; an invalid `hwnd` makes the
    // call fail, which is returned as None.
    let ok = unsafe { GetClientRect(hwnd, &mut rect) };
    if ok == 0 {
        return None;
    }
    let width = u32::try_from(rect.right - rect.left).ok()?;
    let height = u32::try_from(rect.bottom - rect.top).ok()?;
    Some((width, height))
}

/// Shows an information message box without an owner window.
pub fn message_box(text: &str, caption: &str) {
    show_message_box(text, caption, MB_OK | MB_ICONINFORMATION);
}

/// Shows an error message box without an owner window. Only for the
/// settings mode: `/s` and `/p` never show a dialog.
pub fn error_box(text: &str, caption: &str) {
    show_message_box(text, caption, MB_OK | MB_ICONERROR);
}

fn show_message_box(text: &str, caption: &str, style: MESSAGEBOX_STYLE) {
    let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), style);
    }
}

/// The current foreground window (null when there is none).
pub fn foreground_window() -> HWND {
    // SAFETY: takes no arguments.
    unsafe { GetForegroundWindow() }
}

/// Makes `owner` the owner of the top-level window `window`
/// (`SetWindowLongPtrW(GWLP_HWNDPARENT)`, as in spike S2).
pub fn set_owner(window: HWND, owner: HWND) -> Result<(), u32> {
    // SAFETY: Windows validates both handles. The last error is cleared
    // first, because a previous value of 0 is also returned on success.
    unsafe {
        SetLastError(0);
        if SetWindowLongPtrW(window, GWLP_HWNDPARENT, owner as isize) == 0 {
            let error = GetLastError();
            if error != 0 {
                return Err(error);
            }
        }
    }
    Ok(())
}

/// `EnableWindow`: returns whether the window was disabled before the call.
pub fn enable_window(hwnd: HWND, enable: bool) -> bool {
    // SAFETY: EnableWindow validates the handle.
    unsafe { EnableWindow(hwnd, i32::from(enable)) != 0 }
}

/// Whether the window accepts input.
pub fn is_window_enabled(hwnd: HWND) -> bool {
    // SAFETY: IsWindowEnabled validates the handle.
    unsafe { IsWindowEnabled(hwnd) != 0 }
}

/// The Roaming AppData folder of the current user (`%APPDATA%`), from
/// `SHGetKnownFolderPath` (plan 0004, D-1).
pub fn roaming_app_data() -> Option<PathBuf> {
    known_folder(&FOLDERID_RoamingAppData)
}

/// The system fonts folder (`%WINDIR%\Fonts`).
pub fn fonts_folder() -> Option<PathBuf> {
    known_folder(&FOLDERID_Fonts)
}

/// N-14: the registration for the display state of the session. Each report
/// goes to [`crate::display_power::report`]; dropping this unregisters.
pub struct DisplayWatch {
    handle: HPOWERNOTIFY,
    // Why kept: the parameters are passed by pointer at registration, so
    // they live as long as the registration does.
    _params: Box<DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS>,
}

/// Registers for `GUID_SESSION_DISPLAY_STATUS` with a callback, which needs
/// no window ("PowerSettingRegisterNotification", Microsoft Learn). Windows
/// calls it with the current state right after registering. Returns the
/// Win32 error code on failure; drawing then goes on as before.
pub fn watch_display() -> Result<DisplayWatch, u32> {
    let params = Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
        Callback: Some(display_callback),
        Context: std::ptr::null_mut(),
    });
    let mut handle: *mut c_void = std::ptr::null_mut();
    // SAFETY: the GUID is a constant, `params` points to valid parameters
    // that outlive the registration (kept in DisplayWatch), and `handle` is
    // a writable pointer.
    let error = unsafe {
        PowerSettingRegisterNotification(
            &GUID_SESSION_DISPLAY_STATUS,
            DEVICE_NOTIFY_CALLBACK,
            &*params as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS as HANDLE,
            &mut handle,
        )
    };
    if error != 0 {
        return Err(error);
    }
    Ok(DisplayWatch {
        handle: handle as HPOWERNOTIFY,
        _params: params,
    })
}

impl Drop for DisplayWatch {
    fn drop(&mut self) {
        // SAFETY: the handle came from PowerSettingRegisterNotification and
        // is unregistered once, before the parameters are freed.
        unsafe { PowerSettingUnregisterNotification(self.handle) };
    }
}

/// The callback of [`watch_display`]. The documentation does not say on
/// which thread it runs, so it passes the state only through an atomic
/// ([`crate::display_power`]). It also says only that `setting` depends on
/// the type, so it is read as a `POWERBROADCAST_SETTING` only for a power
/// setting change of the display state, and at most the four bytes of a
/// DWORD. Other reports are logged the first few times, so that a hardware
/// check can tell them from no report at all.
unsafe extern "system" fn display_callback(
    _context: *const c_void,
    kind: u32,
    setting: *const c_void,
) -> u32 {
    if kind != PBT_POWERSETTINGCHANGE || setting.is_null() {
        log_unexpected(&format!(
            "type {kind:#x}, setting null {}",
            setting.is_null()
        ));
        return 0;
    }
    let setting = setting as *const POWERBROADCAST_SETTING;
    // SAFETY: for a power setting change, `setting` points to a
    // POWERBROADCAST_SETTING; its fields are read without making a reference,
    // because the data follows the struct and may be longer than it.
    let (guid, length) = unsafe {
        (
            std::ptr::read_unaligned(std::ptr::addr_of!((*setting).PowerSetting)),
            std::ptr::read_unaligned(std::ptr::addr_of!((*setting).DataLength)),
        )
    };
    if !same_guid(&guid, &GUID_SESSION_DISPLAY_STATUS) {
        log_unexpected(&format!("power setting {:08x}", guid.data1));
        return 0;
    }
    let length = (length as usize).min(4);
    // SAFETY: the data has `DataLength` bytes after the GUID and the length;
    // at most four of them are read.
    let data = unsafe {
        std::slice::from_raw_parts(std::ptr::addr_of!((*setting).Data) as *const u8, length)
    };
    crate::display_power::report(data);
    crate::log::write(&format!(
        "display state reported: {data:?} (on: {})",
        crate::display_power::is_on()
    ));
    0
}

/// Logs a report the display callback does not use, the first few times.
fn log_unexpected(what: &str) {
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    if crate::diag::first_few(count) {
        crate::log::write(&format!("display state: unexpected report ({what})"));
    }
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

/// The path of a known folder, or `None` when Windows cannot give it.
fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut raw: PWSTR = std::ptr::null_mut();
    // SAFETY: `id` is a valid GUID and `raw` a writable pointer; a null
    // token means the current user.
    let result =
        unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT as u32, std::ptr::null_mut(), &mut raw) };
    let path = (result >= 0 && !raw.is_null()).then(|| {
        // SAFETY: on success `raw` is a NUL-terminated UTF-16 string that
        // stays valid until it is freed below.
        let wide = unsafe {
            let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
            std::slice::from_raw_parts(raw, len)
        };
        PathBuf::from(OsString::from_wide(wide))
    });
    // SAFETY: the documentation of SHGetKnownFolderPath requires freeing the
    // buffer whether the call succeeded or not; freeing null does nothing.
    unsafe { CoTaskMemFree(raw as *const c_void) };
    path
}
