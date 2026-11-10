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
        // Why one write: lines may come from other threads too (the GPU
        // limit, the display state), and one append of the whole line keeps
        // them from mixing; writeln! writes the parts one by one.
        let text = format!("+{elapsed}ms pid={} {line}\n", std::process::id());
        let _ = file.write_all(text.as_bytes());
    }
}

/// Forwards warnings and errors of the `log` crate (used by wgpu) to the
/// diagnostic log. Lines start with `[LEVEL target]`.
pub fn forward_log_crate() {
    struct Forward;
    impl ::log_crate::Log for Forward {
        fn enabled(&self, metadata: &::log_crate::Metadata<'_>) -> bool {
            metadata.level() <= ::log_crate::Level::Warn
        }
        fn log(&self, record: &::log_crate::Record<'_>) {
            if self.enabled(record.metadata()) {
                write(&format!(
                    "[{} {}] {}",
                    record.level(),
                    record.target(),
                    record.args()
                ));
            }
        }
        fn flush(&self) {}
    }
    static FORWARD: Forward = Forward;
    if ::log_crate::set_logger(&FORWARD).is_ok() {
        ::log_crate::set_max_level(::log_crate::LevelFilter::Warn);
    }
}

/// `log!("...", args)`: formats and writes one line to the diagnostic log.
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write(&format!($($arg)*))
    };
}
pub(crate) use log;
