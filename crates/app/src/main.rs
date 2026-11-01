//! Flying Microwave screensaver executable.

// A screensaver must not open a console window.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod cli;
// Used by the Windows renderer; tested on every host.
#[cfg_attr(not(windows), allow(dead_code))]
mod layout;
#[cfg(windows)]
mod log;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod saver;
#[cfg(windows)]
mod win;

use std::ffi::OsString;

use cli::Mode;

fn main() {
    #[cfg(windows)]
    if let Err(code) = win::restrict_dll_search_path() {
        // SEC-1: keep going, as the system default search order still applies.
        log::write(&format!("SetDefaultDllDirectories failed: error {code}"));
    }

    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let exe_extension = std::env::current_exe()
        .ok()
        .and_then(|path| path.extension().map(|ext| ext.to_os_string()));
    let mode = cli::parse(&args, exe_extension.as_deref());

    std::process::exit(run(mode));
}

#[cfg(windows)]
fn run(mode: Mode) -> i32 {
    log::write(&format!(
        "Flying Microwave {} started: {mode:?}",
        env!("CARGO_PKG_VERSION")
    ));
    match mode {
        Mode::Screensaver => {
            let Some(_instance) = win::single_instance() else {
                log::write("another screensaver instance is running; exiting");
                return 0;
            };
            saver::run()
        }
        // The preview is the next step of Phase 1 (task 7). Until then the
        // preview area stays as the system draws it.
        Mode::Preview { .. } => 0,
        // UI labels are English (AGENTS.md, language rules).
        Mode::Settings { .. } => {
            win::message_box(
                "Settings will be added in a future version.",
                "Flying Microwave",
            );
            0
        }
        Mode::Standalone { .. } => {
            win::message_box(
                "Running Flying Microwave as an app is not available yet.\n\
                 Install FlyingMicrowave.scr as a screensaver to use it.",
                "Flying Microwave",
            );
            0
        }
    }
}

#[cfg(not(windows))]
fn run(mode: Mode) -> i32 {
    // The screensaver is Windows only (ADR-0006); this build exists so the
    // workspace can be checked and tested on Linux.
    eprintln!("{mode:?}: Flying Microwave runs on Windows only");
    0
}
