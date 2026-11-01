//! A small diagnostic log in the temporary folder
//! (`%TEMP%\flying-microwave.log` on Windows).
//!
//! A screensaver has no console, so this file is the only way to see why a
//! run on real hardware failed. It records only startup facts, GPU setup
//! results and the exit reason; never user data. Writing is best effort:
//! errors are ignored.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

/// The log is started afresh when it grows beyond this size.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// Path of the log file.
pub fn path() -> PathBuf {
    std::env::temp_dir().join("flying-microwave.log")
}

/// Appends one line, prefixed with the milliseconds since the first call and the process id.
pub fn write(line: &str) {
    let elapsed = start().elapsed().as_millis();
    let path = path();
    let too_big = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES);
    let file = OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path);
    if let Ok(mut file) = file {
        let _ = writeln!(file, "+{elapsed}ms pid={} {line}", std::process::id());
    }
}

/// `log!("...", args)`: formats and writes one line to the diagnostic log.
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write(&format!($($arg)*))
    };
}
pub(crate) use log;
