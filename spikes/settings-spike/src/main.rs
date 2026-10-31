//! settings-spike: throwaway technical spike S2 for the Flying Microwave
//! screensaver: can an eframe (egui) window be made modal to the owner window
//! that Windows passes with `/c`, using only Win32 calls after creation?
//! See spikes/README.md for the manual procedure. This is not product code.

// Why: no console window; the log file is the only output (see log.rs).
#![cfg_attr(windows, windows_subsystem = "windows")]
// On non-Windows hosts only the platform independent modules (args, log) are
// compiled into use, and only for the unit tests.
#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_macros))]

mod args;
mod log;

#[cfg(windows)]
mod app;
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

    use eframe::egui;

    use crate::app::{self, SettingsApp};
    use crate::args::{self, Mode, Owner};
    use crate::log::{self, log};
    use crate::win;

    const EXIT_OK: i32 = 0;
    const EXIT_START_FAILED: i32 = 1;

    pub fn run() -> i32 {
        // SEC-1: the very first thing, before anything can load a DLL by bare
        // name. The result is logged as soon as the logger exists.
        let dll_search_path = win::restrict_dll_search_path();

        log::init("settings-spike");
        // A crash must not leave the owner dialog disabled.
        std::panic::set_hook(Box::new(|info| {
            log!("PANIC: {info}");
            app::release_owner("panic");
        }));
        log!(
            "=== settings-spike {} started ===",
            env!("CARGO_PKG_VERSION")
        );
        match dll_search_path {
            Ok(()) => log!("SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32): succeeded"),
            Err(code) => log!(
                "SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32): FAILED, GetLastError={code}"
            ),
        }

        let raw_args: Vec<OsString> = std::env::args_os().collect();
        log!("std::env::args: {raw_args:?}");
        log!("std::env::current_exe: {:?}", std::env::current_exe());
        let text_args: Vec<String> = raw_args
            .iter()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let mode = args::parse(&text_args);
        log!("decision: mode = {mode:?}");

        let owner = match mode {
            Mode::NotSettings => {
                log!("not a settings mode (/s or /p): exiting quietly without a window");
                return EXIT_OK;
            }
            Mode::Unrecognized => {
                log!("unrecognized arguments: treated like no argument (settings, no owner)");
                None
            }
            Mode::Settings(owner) => resolve_owner(owner),
        };
        if let Some(owner) = owner {
            app::set_owner_candidate(win::hwnd_to_usize(owner));
        }

        let code = run_window();

        // Backup in case the window ended without going through the UI paths.
        app::release_owner("after run_native returned");
        log!("exiting with code {code}");
        code
    }

    /// Resolves the owner request to a validated window handle (SEC-3).
    fn resolve_owner(request: Owner) -> Option<win::Hwnd> {
        let candidate = match request {
            Owner::None => {
                log!("owner: none requested (no argument)");
                return None;
            }
            Owner::Explicit(value) => {
                log!("owner: HWND {value} from the command line");
                win::hwnd_from_usize(value)
            }
            Owner::Foreground => {
                let hwnd = win::foreground_window();
                log!(
                    "owner: no HWND on the command line; GetForegroundWindow() = {}",
                    win::hwnd_to_usize(hwnd)
                );
                hwnd
            }
        };
        if candidate.is_null() || !win::is_window(candidate) {
            log!("owner: candidate is not a valid window (IsWindow = FALSE); no owner");
            None
        } else {
            log!("owner: candidate is a valid window");
            Some(candidate)
        }
    }

    fn run_window() -> i32 {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Flying Microwave settings (spike)")
                .with_inner_size([420.0, 170.0])
                .with_resizable(false)
                .with_minimize_button(false)
                .with_maximize_button(false)
                // Why: the default egui icon is not ours to ship.
                .with_icon(egui::IconData::default()),
            renderer: eframe::Renderer::Wgpu,
            centered: true,
            // Why: system_fonts is not enabled, so there is nothing to fall back to.
            system_font_fallback: false,
            wgpu_options: app::wgpu_configuration(),
            ..Default::default()
        };
        log!("starting eframe::run_native");
        let result = eframe::run_native(
            "settings-spike",
            options,
            Box::new(|cc| Ok(Box::new(SettingsApp::create(cc)))),
        );
        match result {
            Ok(()) => {
                log!("eframe::run_native returned Ok");
                EXIT_OK
            }
            Err(e) => {
                log!("eframe::run_native FAILED: {e}");
                // The settings window is an interactive mode, so a dialog is fine here.
                win::error_box(
                    "settings-spike: could not open the window (see log).",
                    "settings-spike",
                );
                EXIT_START_FAILED
            }
        }
    }
}
