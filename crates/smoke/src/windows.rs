//! The Windows part of the smoke test: starting the executable, sending
//! input, making a parent window for the preview and finding the settings
//! window.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use flying_microwave_smoke::{Progress, lines_of, meshes_drawn};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, IsWindowEnabled, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput, VK_SPACE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows, GA_ROOT,
    GW_OWNER, GetAncestor, GetClassNameW, GetCursorPos, GetForegroundWindow, GetSystemMetrics,
    GetWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, MSG,
    PM_REMOVE, PeekMessageW, PostMessageW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SetCursorPos,
    TranslateMessage, WM_CLOSE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WindowFromPoint,
};

/// Limit for the first frame. GPU setup itself gives up after 15 seconds
/// (plan 0001, D-27), so a longer wait would only hide a hang.
const READY_LIMIT: Duration = Duration::from_secs(30);
/// A key press ends the screensaver even inside the 500 ms grace period, but
/// a focus loss ends it only after the grace period; waiting longer than that
/// before the input makes such an early end show up as its own failure.
const AFTER_READY: Duration = Duration::from_secs(1);
/// N-02 of plan 0001: from the input until the screen is gone, that is, no
/// visible top-level window of the process is left (plan 0003, D-5).
const GONE_AFTER_INPUT: Duration = Duration::from_millis(200);
/// The process itself may take longer to release the GPU (plan 0003, D-5).
const EXIT_AFTER_INPUT: Duration = Duration::from_secs(2);
/// F-02 of plan 0003: the preview checks its parent periodically.
const EXIT_AFTER_PARENT_GONE: Duration = Duration::from_secs(2);
/// Limit for the settings window to show. Its GPU setup gives up after 15
/// seconds and shows a message box (plan 0004, D-17), so, as READY_LIMIT, a
/// longer wait would only hide a hang (plan 0005, D-1).
const SETTINGS_LIMIT: Duration = Duration::from_secs(30);
const EXIT_AFTER_CLOSE: Duration = Duration::from_secs(10);
/// Limit from the settings window being found to its owner being disabled:
/// the window may be shown before the app is made and the owner attached
/// (crates/app/src/settings.rs, `attach_owner`).
const OWNER_DISABLED_LIMIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(2);

/// The title of the settings window (crates/app/src/settings.rs).
const SETTINGS_TITLE: &str = "Flying Microwave Settings";
/// The variable that moves the settings file (crates/app/src/storage.rs).
const CONFIG_DIR_ENV: &str = "FLYING_MICROWAVE_CONFIG_DIR";
/// A settings file that is not TOML (and not UTF-8).
const BROKEN_SETTINGS: &[u8] = b"\xff\xfe[flight\ncount = = 3\n";

/// One check: its name and the function that runs it.
type Case = (&'static str, fn(&Env) -> Result<(), String>);

/// What every case needs.
struct Env<'a> {
    exe: &'a Path,
    /// The tested processes get this folder as TMP and TEMP, so that their
    /// log is apart from the user's own `%TEMP%\flying-microwave.log`.
    log_dir: &'a Path,
    /// The tested processes get this folder as [`CONFIG_DIR_ENV`], so that
    /// they never read or write the user's own settings (plan 0004, E-1).
    config_dir: &'a Path,
}

/// Runs every case and returns the process exit code (0 when all passed).
pub fn run(exe: &Path) -> i32 {
    let cases: [Case; 5] = [
        ("F-01 /s ends on user input", screensaver_ends_on_input),
        ("F-02 /p ends with its parent", preview_ends_with_parent),
        (
            "F-03 unknown argument shows the settings window",
            unknown_argument_shows_settings,
        ),
        (
            "F-01 /s ends on user input with a broken settings file",
            screensaver_runs_with_a_broken_settings_file,
        ),
        (
            "F-03 /c:<HWND> disables its owner until the settings window closes",
            settings_disables_its_owner,
        ),
    ];
    let log_dir =
        std::env::temp_dir().join(format!("flying-microwave-smoke-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        println!("FAIL could not create {}: {e}", log_dir.display());
        return 1;
    }
    println!("log folder of the tested processes: {}", log_dir.display());
    let config_dir = log_dir.join("config");
    let mut failed = 0;
    for (name, case) in cases {
        match case(&Env {
            exe,
            log_dir: &log_dir,
            config_dir: &config_dir,
        }) {
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
    log: PathBuf,
}

impl Target {
    /// Starts the executable with `args`, and with `settings` as the
    /// contents of its settings file (no file when `None`).
    fn start(env: &Env, args: &[&str], settings: Option<&[u8]>) -> Result<Self, String> {
        let log = env.log_dir.join("flying-microwave.log");
        // A fresh log, so that only this run's lines are read.
        let _ = std::fs::remove_file(&log);
        // A fresh settings folder, so that no case sees another case's file.
        let _ = std::fs::remove_dir_all(env.config_dir);
        std::fs::create_dir_all(env.config_dir)
            .map_err(|e| format!("could not create {}: {e}", env.config_dir.display()))?;
        if let Some(settings) = settings {
            let path = env.config_dir.join("config.toml");
            std::fs::write(&path, settings)
                .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        }
        // Why both: std::env::temp_dir (GetTempPath2W) reads TMP first, then TEMP.
        let child = Command::new(env.exe)
            .args(args)
            .env("TMP", env.log_dir)
            .env("TEMP", env.log_dir)
            .env(CONFIG_DIR_ENV, env.config_dir)
            .spawn()
            .map_err(|e| format!("could not start {}: {e}", env.exe.display()))?;
        Ok(Target { child, log })
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// Prints the process's own log lines, which tell why a case failed.
    fn print_log(&self) {
        let text = self.log_text();
        println!("  log of process {}:", self.pid());
        for line in lines_of(&text, self.pid()) {
            println!("    {line}");
        }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn progress(&self) -> Progress {
        Progress::read(&self.log_text(), self.pid())
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
                return Err(format!("still running after {} ms", limit.as_millis()));
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

/// Fails unless the microwaves are drawn (not only the background), and
/// prints the atlas line (its time is part of the startup, plan 0001 N-01).
fn expect_meshes(target: &Target) -> Result<(), String> {
    let text = target.log_text();
    let atlas = meshes_drawn(&lines_of(&text, target.pid()))?;
    println!("  {atlas}");
    Ok(())
}

fn report(progress: &Progress) {
    // The facts a failure would depend on, printed even when the case passes.
    println!(
        "  adapter: {}; started to first frame: {}",
        progress.adapter.as_deref().unwrap_or("not logged"),
        progress
            .startup_ms()
            .map_or("not logged".to_owned(), |ms| format!("{ms} ms")),
    );
}

/// Runs `body` on a started target and prints the target's log if it fails.
fn with_target(
    env: &Env,
    args: &[&str],
    settings: Option<&[u8]>,
    body: impl FnOnce(&mut Target) -> Result<(), String>,
) -> Result<(), String> {
    let mut target = Target::start(env, args, settings)?;
    let result = body(&mut target);
    if result.is_err() {
        target.print_log();
    }
    result
}

fn expect_reason(progress: &Progress, expected: &[&str]) -> Result<(), String> {
    let reason = progress.exit_reason.as_deref().unwrap_or("not logged");
    println!("  exit reason: {reason}");
    if expected.iter().any(|e| reason.starts_with(e)) {
        Ok(())
    } else {
        Err(format!("ended for another reason: {reason}"))
    }
}

/// Checks the `config:` line: the settings came from the test folder and
/// were read as `read_as` says (plan 0004, F-06).
fn expect_config(progress: &Progress, read_as: &str) -> Result<(), String> {
    let config = progress.config.as_deref().unwrap_or("not logged");
    println!("  settings: {config}");
    if !config.contains(CONFIG_DIR_ENV) {
        return Err(format!(
            "the settings were not read from {CONFIG_DIR_ENV}: {config}"
        ));
    }
    if !config.starts_with(read_as) {
        return Err(format!(
            "the settings were read as \"{config}\", not \"{read_as}\""
        ));
    }
    Ok(())
}

fn screensaver_ends_on_input(env: &Env) -> Result<(), String> {
    screensaver_ends_on_input_with(env, None, "file not found")
}

/// F-06 of plan 0004: a broken settings file must not stop `/s`.
fn screensaver_runs_with_a_broken_settings_file(env: &Env) -> Result<(), String> {
    screensaver_ends_on_input_with(env, Some(BROKEN_SETTINGS), "not valid TOML")
}

fn screensaver_ends_on_input_with(
    env: &Env,
    settings: Option<&[u8]>,
    read_as: &str,
) -> Result<(), String> {
    // Why before the start: a click is sent where the cursor is, and moving
    // the cursor after the grace period would end the screensaver by itself.
    let (cx, cy) = screen_center();
    // SAFETY: SetCursorPos takes plain coordinates; a failure only leaves
    // the cursor where it was, which the check before the click reports.
    unsafe { SetCursorPos(cx, cy) };
    with_target(env, &["/s"], settings, |target| {
        let progress = target.wait_ready(Progress::screensaver_ready)?;
        report(&progress);
        expect_meshes(target)?;
        expect_config(&progress, read_as)?;
        // Whether the key press can reach the window depends on this.
        for line in lines_of(&target.log_text(), target.pid()) {
            if line.contains("SetForegroundWindow") {
                println!("  {line}");
            }
        }
        std::thread::sleep(AFTER_READY);
        if let Some(code) = target.exited()? {
            return Err(format!("ended (exit code {code}) before any input"));
        }
        // Why only full-screen windows: winit also keeps a visible 0x0 window
        // for its own messages until the process ends (winit 0.30.13,
        // create_event_target_window), which is not part of the screen.
        let shown = || windows_of(target.pid()).iter().any(|w| w.covers_screen);
        if !shown() {
            return Err("no full-screen window is shown before the input".to_owned());
        }
        // Why the click: a key press goes to the foreground window, and on
        // the windows-11-arm runner SetForegroundWindow returned false (the
        // cause is assumed to be the foreground lock). A click goes to the
        // window under the cursor (plan 0003, D-7). The click is allowed only
        // when the product tried and Windows refused, so that losing the
        // call in the product still fails the test.
        let (fg_pid, fg_class) = foreground_owner();
        println!("  foreground window: process {fg_pid}, class {fg_class}");
        let input = if fg_pid == target.pid() {
            send_key(VK_SPACE)?;
            Input::Key
        } else {
            let refused = lines_of(&target.log_text(), target.pid())
                .iter()
                .any(|line| line.ends_with("SetForegroundWindow: false"));
            if !refused {
                return Err(
                    "not in the foreground, and the log has no \"SetForegroundWindow: false\""
                        .to_owned(),
                );
            }
            let under = window_under_cursor();
            if !windows_of(target.pid())
                .iter()
                .any(|w| w.covers_screen && w.hwnd == under)
            {
                return Err(
                    "not in the foreground, and the cursor is not over its full-screen window"
                        .to_owned(),
                );
            }
            // GitHub Actions shows this line as a warning in the run summary.
            println!(
                "::warning::F-01: SetForegroundWindow was refused; tested with a click instead of a key"
            );
            send_click()?;
            Input::Click
        };
        println!("  input: {}", input.name());
        let start = Instant::now();
        while shown() {
            if start.elapsed() > EXIT_AFTER_INPUT {
                return Err(format!(
                    "the screen is still shown {} ms after the {}",
                    EXIT_AFTER_INPUT.as_millis(),
                    input.name()
                ));
            }
            std::thread::sleep(POLL);
        }
        let gone = start.elapsed();
        let (code, _) = target.wait_exit(EXIT_AFTER_INPUT.saturating_sub(gone))?;
        println!(
            "  input to screen gone: {} ms; to process exit: {} ms",
            gone.as_millis(),
            start.elapsed().as_millis()
        );
        let reason = expect_reason(&target.progress(), &[input.exit_reason()]);
        if code != 0 {
            return Err(format!("exit code {code}"));
        }
        reason?;
        // Why: the summary line was never written once (issue #50), and a
        // window that drew nothing must not pass.
        let progress = target.progress();
        match progress.summary_of(0) {
            Some(s) if s.frames >= 1 && s.is_consistent() => {
                // Incident 0028: where the frames wait, shown in every run.
                let timings = s
                    .timings
                    .map(|(wait, present, draw)| {
                        format!(
                            "; longest surface wait {wait} ms, longest present {present} ms, \
                             longest draw {draw} ms"
                        )
                    })
                    .unwrap_or_default();
                println!(
                    "  window 0: {} draws attempted, {} frames, {} paced, {} blank, {} skipped{timings}",
                    s.attempts, s.frames, s.paced, s.blank, s.skipped
                );
            }
            Some(s) => {
                return Err(format!(
                    "window 0 summary: {} draws attempted, {} frames, {} paced, {} blank, {} skipped \
                     (expected at least 1 frame, and attempts = frames + paced + blank + skipped)",
                    s.attempts, s.frames, s.paced, s.blank, s.skipped
                ));
            }
            None => return Err("the log has no summary line for window 0".to_owned()),
        }
        if gone > GONE_AFTER_INPUT {
            return Err(format!(
                "the screen was gone {} ms after the {} (limit {} ms)",
                gone.as_millis(),
                input.name(),
                GONE_AFTER_INPUT.as_millis()
            ));
        }
        Ok(())
    })
}

fn preview_ends_with_parent(env: &Env) -> Result<(), String> {
    let parent = ParentWindow::new()?;
    let hwnd_arg = (parent.0 as usize).to_string();
    let mut parent = Some(parent);
    with_target(env, &["/p", &hwnd_arg], None, |target| {
        let progress = target.wait_ready(Progress::preview_ready)?;
        report(&progress);
        expect_meshes(target)?;
        drop(parent.take());
        let (code, elapsed) = target.wait_exit(EXIT_AFTER_PARENT_GONE)?;
        println!("  parent destroyed to exit: {} ms", elapsed.as_millis());
        let reason = expect_reason(&target.progress(), &["WindowDestroyed", "ParentGone"]);
        if code != 0 {
            return Err(format!("exit code {code}"));
        }
        reason
    })
}

fn unknown_argument_shows_settings(env: &Env) -> Result<(), String> {
    with_target(env, &["/x"], None, |target| {
        let window = wait_settings_window(target)?;
        expect_config(&target.progress(), "file not found")?;
        close_settings(target, window)
    })
}

/// Plan 0005, task 12 (plan 0001, Phase 3, task 8): with `/c:<HWND>`, the
/// settings window is owned by that window (F-04 of plan 0001), which is
/// disabled while it is open and enabled again when it closes.
fn settings_disables_its_owner(env: &Env) -> Result<(), String> {
    let owner = ParentWindow::new()?;
    let owner_arg = format!("/c:{}", owner.0 as usize);
    with_target(env, &[&owner_arg], None, |target| {
        let window = wait_settings_window(target)?;
        let start = Instant::now();
        // SAFETY: the handle is only read; a window that is gone makes the
        // call return 0.
        while unsafe { IsWindowEnabled(owner.0) } != 0 {
            if start.elapsed() > OWNER_DISABLED_LIMIT {
                return Err(format!(
                    "the owner is still enabled {} ms after the settings window was found",
                    OWNER_DISABLED_LIMIT.as_millis()
                ));
            }
            // The owner's thread answers the messages that disabling sends.
            pump_messages();
            std::thread::sleep(POLL);
        }
        println!(
            "  owner disabled {} ms after the settings window was found",
            start.elapsed().as_millis()
        );
        // The app sets the owner right before disabling it
        // (crates/app/src/settings.rs, `attach_owner`), so it is set by now.
        // SAFETY: both handles are only read; a window that is gone makes
        // the call return null, which fails the check.
        let owned_by = unsafe { GetWindow(window, GW_OWNER) };
        if owned_by != owner.0 {
            return Err(format!(
                "the settings window is owned by {owned_by:?}, not by {:?}",
                owner.0
            ));
        }
        close_settings(target, window)?;
        // SAFETY: as for the wait above.
        if unsafe { IsWindowEnabled(owner.0) } == 0 {
            return Err("the owner is still disabled after the settings window closed".to_owned());
        }
        println!("  owner enabled again after the settings window closed");
        Ok(())
    })
}

/// Waits for the settings window of `target` and returns it. Fails on a
/// full-screen window, a message box, an early exit or the time limit.
fn wait_settings_window(target: &mut Target) -> Result<HWND, String> {
    let start = Instant::now();
    let window = loop {
        let windows = windows_of(target.pid());
        if let Some(full) = windows.iter().find(|w| w.covers_screen) {
            return Err(format!("made a full-screen window ({})", full.class));
        }
        if let Some(settings) = windows.iter().find(|w| w.title == SETTINGS_TITLE) {
            break settings.hwnd;
        }
        // The message box that tells the settings window could not open
        // (plan 0004, E-4 and D-17). Both cases end with exit code 7, so
        // the log line tells a reached limit apart (plan 0005, D-1).
        if let Some(dialog) = windows.iter().find(|w| w.class == "#32770") {
            // SAFETY: as for the settings window below.
            unsafe { PostMessageW(dialog.hwnd, WM_CLOSE, 0, 0) };
            return Err(format!(
                "a message box (\"{}\") was shown instead of the settings window{}",
                dialog.title,
                timed_out(target)
            ));
        }
        if let Some(code) = target.exited()? {
            return Err(format!(
                "ended (exit code {code}) without a settings window{}",
                timed_out(target)
            ));
        }
        if start.elapsed() > SETTINGS_LIMIT {
            return Err(format!(
                "no settings window within {} s",
                SETTINGS_LIMIT.as_secs()
            ));
        }
        // A window owned by this thread (with `/c:<HWND>`) must answer the
        // messages the settings window sends it while it is set up.
        pump_messages();
        std::thread::sleep(Duration::from_millis(50));
    };
    println!(
        "  settings window shown after {} ms",
        start.elapsed().as_millis()
    );
    Ok(window)
}

/// Closes the settings window and expects the process to end with code 0.
fn close_settings(target: &mut Target, window: HWND) -> Result<(), String> {
    // SAFETY: posting a message to a window handle is safe even if the
    // window is gone by now; the call then just fails.
    unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
    let (code, elapsed) = target.wait_exit(EXIT_AFTER_CLOSE)?;
    println!("  close to process exit: {} ms", elapsed.as_millis());
    if code != 0 {
        return Err(format!("exit code {code}"));
    }
    Ok(())
}

/// Says so when the GPU initialization limit was reached.
fn timed_out(target: &Target) -> &'static str {
    if target.progress().gpu_init_timed_out {
        ": the GPU initialization limit was reached"
    } else {
        ""
    }
}

/// The user input that F-01 sends.
#[derive(Clone, Copy)]
enum Input {
    Key,
    Click,
}

impl Input {
    fn name(self) -> &'static str {
        match self {
            Input::Key => "key press",
            Input::Click => "click",
        }
    }

    /// The exit reason the screensaver logs for this input.
    fn exit_reason(self) -> &'static str {
        match self {
            Input::Key => "KeyPressed { synthetic: false }",
            Input::Click => "MouseButtonPressed",
        }
    }
}

/// The process id and class name of the foreground window (0 and "none"
/// when there is none).
fn foreground_owner() -> (u32, String) {
    let mut owner = 0;
    let mut name = [0u16; 64];
    // SAFETY: a null foreground window is checked before use; owner and
    // name are local buffers that outlive the calls, with their sizes passed.
    let len = unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (0, "none".to_owned());
        }
        GetWindowThreadProcessId(hwnd, &mut owner);
        GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32)
    };
    (
        owner,
        String::from_utf16_lossy(&name[..len.max(0) as usize]),
    )
}

/// The center of the primary monitor.
fn screen_center() -> (i32, i32) {
    // SAFETY: GetSystemMetrics takes a constant and returns a number.
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN) / 2,
            GetSystemMetrics(SM_CYSCREEN) / 2,
        )
    }
}

/// The top-level window under the cursor (null if it cannot be found).
fn window_under_cursor() -> HWND {
    let mut point = POINT::default();
    // SAFETY: point is a local buffer; a failed GetCursorPos returns null.
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return std::ptr::null_mut();
        }
        let hwnd = WindowFromPoint(point);
        if hwnd.is_null() {
            hwnd
        } else {
            GetAncestor(hwnd, GA_ROOT)
        }
    }
}

/// Presses and releases the left mouse button where the cursor is, without
/// moving it (a move could end the screensaver for another reason).
fn send_click() -> Result<(), String> {
    let button = |flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    send_inputs(&[button(MOUSEEVENTF_LEFTDOWN), button(MOUSEEVENTF_LEFTUP)])
}

fn send_inputs(inputs: &[INPUT]) -> Result<(), String> {
    // SAFETY: the slice outlives the call and its length and element size are passed.
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
    send_inputs(&[key(0), key(KEYEVENTF_KEYUP)])
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
    title: String,
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
                let mut title = [0u16; 128];
                let title_len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect);
                let covers_screen = rect.right - rect.left >= GetSystemMetrics(SM_CXSCREEN)
                    && rect.bottom - rect.top >= GetSystemMetrics(SM_CYSCREEN);
                search.found.push(TopWindow {
                    hwnd,
                    class: String::from_utf16_lossy(&name[..len.max(0) as usize]),
                    title: String::from_utf16_lossy(&title[..title_len.max(0) as usize]),
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
