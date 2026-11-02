//! The Windows part of the smoke test: starting the executable, sending
//! input, making a parent window for the preview and finding the message box.

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use flying_microwave_smoke::Progress;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_SPACE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows, GetClassNameW,
    GetSystemMetrics, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, MSG, PM_REMOVE,
    PeekMessageW, PostMessageW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, TranslateMessage,
    WM_CLOSE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

/// Limit for the first frame. GPU setup itself gives up after 15 seconds
/// (plan 0001, D-27), so a longer wait would only hide a hang.
const READY_LIMIT: Duration = Duration::from_secs(30);
/// Input must not fall inside the 500 ms grace period of the exit policy.
const AFTER_READY: Duration = Duration::from_secs(1);
/// N-02 of plan 0001: from the input to the end of the process.
const EXIT_AFTER_INPUT: Duration = Duration::from_millis(200);
/// F-02 of plan 0003: the preview checks its parent periodically.
const EXIT_AFTER_PARENT_GONE: Duration = Duration::from_secs(2);
const DIALOG_LIMIT: Duration = Duration::from_secs(15);
const EXIT_AFTER_DIALOG: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(2);

/// One check: its name and the function that runs it.
type Case = (&'static str, fn(&Path) -> Result<(), String>);

/// Runs every case and returns the process exit code (0 when all passed).
pub fn run(exe: &Path) -> i32 {
    let cases: [Case; 3] = [
        ("F-01 /s ends on a key press", screensaver_ends_on_key),
        ("F-02 /p ends with its parent", preview_ends_with_parent),
        (
            "F-03 unknown argument shows the settings message",
            unknown_argument_shows_message,
        ),
    ];
    let mut failed = 0;
    for (name, case) in cases {
        match case(exe) {
            Ok(()) => println!("PASS {name}"),
            Err(e) => {
                println!("FAIL {name}: {e}");
                failed += 1;
            }
        }
    }
    println!("{} passed, {failed} failed", cases.len() - failed);
    i32::from(failed > 0)
}

/// A started executable that is killed if the case ends while it still runs
/// (F-04: no process is left behind).
struct Target {
    child: Child,
}

impl Target {
    fn start(exe: &Path, args: &[&str]) -> Result<Self, String> {
        // A fresh log, so that only this run's lines are read.
        let _ = std::fs::remove_file(log_path());
        let child = Command::new(exe)
            .args(args)
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", exe.display()))?;
        Ok(Target { child })
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn progress(&self) -> Progress {
        let log = std::fs::read_to_string(log_path()).unwrap_or_default();
        Progress::read(&log, self.pid())
    }

    /// The exit code once the process has ended.
    fn exited(&mut self) -> Result<Option<i32>, String> {
        match self.child.try_wait() {
            Ok(status) => Ok(status.map(|s| s.code().unwrap_or(-1))),
            Err(e) => Err(format!("could not query the process: {e}")),
        }
    }

    /// Waits for the process to end, pumping this thread's messages.
    fn wait_exit(&mut self, limit: Duration) -> Result<(i32, Duration), String> {
        let start = Instant::now();
        loop {
            if let Some(code) = self.exited()? {
                return Ok((code, start.elapsed()));
            }
            if start.elapsed() > limit {
                return Err(format!(
                    "still running {} ms after the limit",
                    limit.as_millis()
                ));
            }
            pump_messages();
            std::thread::sleep(POLL);
        }
    }

    /// Waits until `ready` holds for the log, failing if the process ends first.
    fn wait_ready(&mut self, ready: fn(&Progress) -> bool) -> Result<Progress, String> {
        let start = Instant::now();
        loop {
            let progress = self.progress();
            if ready(&progress) {
                return Ok(progress);
            }
            if let Some(code) = self.exited()? {
                return Err(format!(
                    "ended (exit code {code}) before its first frame: {progress:?}"
                ));
            }
            if start.elapsed() > READY_LIMIT {
                return Err(format!(
                    "no first frame within {} s: {progress:?}",
                    READY_LIMIT.as_secs()
                ));
            }
            pump_messages();
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            println!("  killing process {} that was left running", self.pid());
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("flying-microwave.log")
}

fn report(progress: &Progress) {
    println!(
        "  adapter: {}; started to first frame: {}",
        progress.adapter.as_deref().unwrap_or("not logged"),
        progress
            .startup_ms()
            .map_or("not logged".to_owned(), |ms| format!("{ms} ms")),
    );
}

fn screensaver_ends_on_key(exe: &Path) -> Result<(), String> {
    let mut target = Target::start(exe, &["/s"])?;
    let progress = target.wait_ready(Progress::screensaver_ready)?;
    report(&progress);
    std::thread::sleep(AFTER_READY);
    if let Some(code) = target.exited()? {
        return Err(format!("ended (exit code {code}) before any input"));
    }
    send_key(VK_SPACE)?;
    let (code, elapsed) = target.wait_exit(Duration::from_secs(5))?;
    println!("  input to exit: {} ms", elapsed.as_millis());
    if code != 0 {
        return Err(format!("exit code {code}"));
    }
    if elapsed > EXIT_AFTER_INPUT {
        return Err(format!(
            "took {} ms after the key press (limit {} ms)",
            elapsed.as_millis(),
            EXIT_AFTER_INPUT.as_millis()
        ));
    }
    Ok(())
}

fn preview_ends_with_parent(exe: &Path) -> Result<(), String> {
    let parent = ParentWindow::new()?;
    let hwnd_arg = (parent.0 as usize).to_string();
    let mut target = Target::start(exe, &["/p", &hwnd_arg])?;
    let progress = target.wait_ready(Progress::preview_ready)?;
    report(&progress);
    drop(parent);
    let (code, elapsed) = target.wait_exit(EXIT_AFTER_PARENT_GONE)?;
    println!("  parent destroyed to exit: {} ms", elapsed.as_millis());
    if code != 0 {
        return Err(format!("exit code {code}"));
    }
    Ok(())
}

fn unknown_argument_shows_message(exe: &Path) -> Result<(), String> {
    let mut target = Target::start(exe, &["/x"])?;
    let start = Instant::now();
    let dialog = loop {
        let windows = windows_of(target.pid());
        if let Some(full) = windows.iter().find(|w| w.covers_screen) {
            return Err(format!("made a full-screen window ({})", full.class));
        }
        if let Some(dialog) = windows.iter().find(|w| w.class == "#32770") {
            break dialog.hwnd;
        }
        if let Some(code) = target.exited()? {
            return Err(format!("ended (exit code {code}) without a message box"));
        }
        if start.elapsed() > DIALOG_LIMIT {
            return Err(format!(
                "no message box within {} s",
                DIALOG_LIMIT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // SAFETY: posting a message to a window handle is safe even if the
    // window is gone by now; the call then just fails.
    unsafe { PostMessageW(dialog, WM_CLOSE, 0, 0) };
    let (code, _) = target.wait_exit(EXIT_AFTER_DIALOG)?;
    if code != 0 {
        return Err(format!("exit code {code}"));
    }
    Ok(())
}

/// Presses and releases one key, as a user would.
fn send_key(vk: u16) -> Result<(), String> {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(0), key(KEYEVENTF_KEYUP)];
    // SAFETY: the array outlives the call and its length and element size are passed.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!("SendInput sent {sent} of {} events", inputs.len()))
    }
}

/// A plain top-level window that stands in for the Settings dialog's
/// preview frame. Dropping it destroys the window.
struct ParentWindow(HWND);

impl ParentWindow {
    fn new() -> Result<Self, String> {
        let class: Vec<u16> = "FlyingMicrowaveSmokeParent\0".encode_utf16().collect();
        // SAFETY: the class name outlives both calls; a zero result is
        // handled, and registering the same class twice fails harmlessly.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(parent_proc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..Default::default()
            };
            RegisterClassW(&wc);
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                320,
                240,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err("CreateWindowExW failed for the parent window".to_owned());
        }
        Ok(ParentWindow(hwnd))
    }
}

impl Drop for ParentWindow {
    fn drop(&mut self) {
        // SAFETY: the window was created by this thread and is destroyed once.
        unsafe { DestroyWindow(self.0) };
    }
}

unsafe extern "system" fn parent_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: forwards the arguments the system passed in.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Handles the messages of this thread, so that the parent window answers
/// the messages its cross-process child window sends.
fn pump_messages() {
    let mut msg = MSG::default();
    // SAFETY: msg is a valid buffer; a null window means all of this thread's windows.
    unsafe {
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

struct TopWindow {
    hwnd: HWND,
    class: String,
    covers_screen: bool,
}

/// The visible top-level windows of process `pid`.
fn windows_of(pid: u32) -> Vec<TopWindow> {
    struct Search {
        pid: u32,
        found: Vec<TopWindow>,
    }
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: lparam is the &mut Search passed to EnumWindows below,
        // which is alive for the whole enumeration.
        let search = unsafe { &mut *(lparam as *mut Search) };
        let mut owner = 0;
        // SAFETY: hwnd comes from EnumWindows; the out pointers are valid.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == search.pid && IsWindowVisible(hwnd) != 0 {
                let mut name = [0u16; 64];
                let len = GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32);
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect);
                let covers_screen = rect.right - rect.left >= GetSystemMetrics(SM_CXSCREEN)
                    && rect.bottom - rect.top >= GetSystemMetrics(SM_CYSCREEN);
                search.found.push(TopWindow {
                    hwnd,
                    class: String::from_utf16_lossy(&name[..len.max(0) as usize]),
                    covers_screen,
                });
            }
        }
        1
    }
    let mut search = Search {
        pid,
        found: Vec::new(),
    };
    // SAFETY: the callback only uses lparam as the Search above, during this call.
    unsafe { EnumWindows(Some(each), &mut search as *mut Search as LPARAM) };
    search.found
}
