//! Flying Microwave screensaver executable.

mod cli;
// Used by the Windows renderer; tested on every host.
#[cfg_attr(not(windows), allow(dead_code))]
mod layout;

use std::ffi::OsString;

use cli::{Mode, Owner};

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let exe_extension = std::env::current_exe()
        .ok()
        .and_then(|path| path.extension().map(|ext| ext.to_os_string()));

    let mode = cli::parse(&args, exe_extension.as_deref());

    // The modes are implemented in later Phase 1 tasks
    // (docs/plans/0001-initial-implementation-plan.md, Phase 1).
    let what = match mode {
        Mode::Screensaver => "screensaver".to_owned(),
        Mode::Preview { parent } => format!("preview of window {parent}"),
        Mode::Settings { owner: Owner::None } => "settings without an owner window".to_owned(),
        Mode::Settings {
            owner: Owner::Foreground,
        } => "settings owned by the foreground window".to_owned(),
        Mode::Settings {
            owner: Owner::Window(handle),
        } => format!("settings owned by window {handle}"),
        Mode::Standalone { fullscreen: false } => "standalone window".to_owned(),
        Mode::Standalone { fullscreen: true } => "standalone fullscreen".to_owned(),
    };
    eprintln!("{what} is not implemented yet");
}
