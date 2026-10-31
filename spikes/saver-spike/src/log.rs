//! Minimal file logger shared by the spike modes.
//!
//! Every line is appended to `%TEMP%\flying-microwave-spike.log` as
//! `<unix ms> +<ms since start> pid=<pid> <tag>: <message>`. The file is opened
//! per line and written with a single call, so the main thread, the watchdog
//! thread and the `reg query` thread can log without interleaving mid-line.
//!
//! The binaries use the Windows GUI subsystem, so there is no console: the log
//! file is the only output. All failures here are ignored on purpose, because
//! a screensaver must never fail (or show a dialog) because of logging.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct State {
    tag: &'static str,
    started: Instant,
    path: PathBuf,
}

static STATE: OnceLock<State> = OnceLock::new();

/// Name of the log file inside the temp directory (shared by both spikes).
pub const FILE_NAME: &str = "flying-microwave-spike.log";

/// Must be called once at the start of `main`. `tag` identifies the program.
pub fn init(tag: &'static str) {
    let _ = STATE.set(State {
        tag,
        started: Instant::now(),
        path: std::env::temp_dir().join(FILE_NAME),
    });
}

/// Appends one line to the log file.
pub fn write(message: &str) {
    let Some(state) = STATE.get() else {
        return;
    };
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let line = format!(
        "{unix_ms} +{}ms pid={} {}: {message}\n",
        state.started.elapsed().as_millis(),
        std::process::id(),
        state.tag,
    );
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// `log!("format {}", args)` appends a formatted line to the log file.
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write(&format!($($arg)*))
    };
}
pub(crate) use log;

/// Logs panics (the GUI subsystem would otherwise swallow them silently).
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        write(&format!("PANIC: {info}"));
    }));
}
