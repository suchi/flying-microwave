//! saver-spike: throwaway technical spike for the Flying Microwave screensaver.
//!
//! It covers spikes S1 (preview child window), S3 (what Windows passes as
//! arguments), S4 (multi-monitor fullscreen and focus events), S6 (GPU-less
//! environments) and the CI smoke-test feasibility check. See spikes/README.md
//! for the manual procedure and docs/research/windows-screensaver.md for the
//! questions being answered. This is not product code.

// Why: no console window; the log file is the only output (see log.rs).
#![cfg_attr(windows, windows_subsystem = "windows")]
// On non-Windows hosts only the platform independent modules (args, policy,
// log) are compiled into use, and only for their unit tests.
#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_macros))]

mod args;
mod log;
mod policy;

#[cfg(windows)]
mod gpu;
#[cfg(windows)]
mod preview;
#[cfg(windows)]
mod saver;
#[cfg(windows)]
mod selftest;
#[cfg(windows)]
mod win;

#[cfg(not(windows))]
fn main() {
    println!("Windows only");
}

#[cfg(windows)]
fn main() {
    let code = windows_main::run();
    std::process::exit(code);
}

#[cfg(windows)]
mod windows_main {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use crate::args::{self, Mode};
    use crate::log::{self, log};
    use crate::win::{self, MessageIcon};
    use crate::{gpu, preview, saver, selftest};

    /// How long to wait for the registry probe before exiting anyway.
    const REGISTRY_PROBE_WAIT: Duration = Duration::from_secs(3);

    pub fn run() -> i32 {
        // SEC-1: the very first thing, before anything can load a DLL by bare
        // name. The result is logged as soon as the logger exists.
        let dll_search_path = win::restrict_dll_search_path();

        log::init("saver-spike");
        log::install_panic_hook();
        log!("=== saver-spike {} started ===", env!("CARGO_PKG_VERSION"));
        match dll_search_path {
            Ok(()) => log!("SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32): succeeded"),
            Err(code) => log!(
                "SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32): FAILED, GetLastError={code}"
            ),
        }

        // S3: record exactly what Windows passed. args_os() cannot panic on
        // unpaired surrogates, unlike args().
        let raw_args: Vec<OsString> = std::env::args_os().collect();
        log!("std::env::args: {raw_args:?}");
        log!("std::env::current_exe: {:?}", std::env::current_exe());
        let registry_probe = start_registry_probe();

        let text_args: Vec<String> = raw_args
            .iter()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let mode = args::parse(&text_args);
        log!("decision: mode = {mode:?}");

        let options = read_flags();
        let code = match mode {
            Mode::Screensaver => saver::run(options),
            Mode::Preview { hwnd } => preview::run(options, hwnd),
            Mode::Settings { hwnd } => {
                log!("settings mode (hwnd argument {hwnd:?}): showing the message box");
                let result = win::message_box(
                    "Settings are tested by settings-spike.",
                    "saver-spike",
                    MessageIcon::Information,
                );
                log!("MessageBoxW result: {result:?}");
                0
            }
            Mode::SelfTest => selftest::run(),
            Mode::Unrecognized => {
                log!("unrecognized arguments: showing the message box");
                let result = win::message_box(
                    "saver-spike: unrecognized arguments (see log).",
                    "saver-spike",
                    MessageIcon::Warning,
                );
                log!("MessageBoxW result: {result:?}");
                0
            }
        };

        // Give the probe a moment to finish so that its output reaches the log.
        let _ = registry_probe.recv_timeout(REGISTRY_PROBE_WAIT);
        log!("exiting with code {code}");
        code
    }

    /// Reads `force-fallback.flag` and `backend-vulkan.flag` next to the
    /// executable. Files are used (not command-line options) so that the flags
    /// also work when Windows starts the .scr itself.
    fn read_flags() -> gpu::Options {
        let dir: Option<PathBuf> = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from));
        let flag = |name: &str| -> bool {
            let Some(dir) = &dir else {
                log!("flag {name}: executable directory unknown, treated as absent");
                return false;
            };
            let path = dir.join(name);
            let present = path.exists();
            log!(
                "flag file {path:?}: {}",
                if present { "present" } else { "absent" }
            );
            present
        };
        let options = gpu::Options {
            force_fallback: flag("force-fallback.flag"),
            vulkan: flag("backend-vulkan.flag"),
        };
        log!("gpu options from flag files: {options:?}");
        options
    }

    /// S3: runs `reg query "HKCR\scrfile\shell" /s` on a background thread and
    /// logs its output line by line. The returned receiver fires when done.
    fn start_registry_probe() -> mpsc::Receiver<()> {
        let (done, wait) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("registry-probe".to_owned())
            .spawn(move || {
                run_registry_probe();
                let _ = done.send(());
            });
        if let Err(e) = spawned {
            log!("registry probe: could not start the thread: {e}");
        }
        wait
    }

    fn run_registry_probe() {
        use std::os::windows::process::CommandExt;
        /// CREATE_NO_WINDOW: do not flash a console window for reg.exe.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        // Why: a bare "reg" would be searched for in the application's own
        // directory first, where a planted reg.exe could sit next to the .scr.
        // The path is built from GetSystemDirectoryW and not from the inherited
        // %SystemRoot%, which any parent process could have changed.
        let system_dir = match win::system_directory() {
            Ok(dir) => dir,
            Err(code) => {
                log!(
                    "registry probe: GetSystemDirectoryW FAILED, GetLastError={code}; \
                     skipping the probe"
                );
                return;
            }
        };
        let reg = system_dir.join("reg.exe");
        log!("registry probe: running {reg:?} query HKCR\\scrfile\\shell /s");
        let output = Command::new(&reg)
            .args(["query", r"HKCR\scrfile\shell", "/s"])
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match output {
            Ok(output) => {
                log!("registry probe: exit status {}", output.status);
                // Known limitation: reg.exe writes to the pipe (stdout and
                // stderr) in the OEM code page (for example CP932 on Japanese
                // Windows), not UTF-8, so non-ASCII text such as "(既定)" for
                // the default value name is garbled by from_utf8_lossy. The
                // spike does not decode it; the verbs and commands it is after
                // are ASCII.
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    log!("reg: {line}");
                }
                for line in String::from_utf8_lossy(&output.stderr).lines() {
                    log!("reg (stderr): {line}");
                }
            }
            Err(e) => log!("registry probe: could not run reg.exe: {e}"),
        }
    }
}
