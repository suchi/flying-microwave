//! Reading the diagnostic log of the screensaver for the smoke test
//! (docs/plans/0003-v0.1.1-smoke-test.md).
//!
//! The log is shared by every process that ran, so only the lines of one
//! process id are read. This part has no Windows dependency, so it is tested
//! on any host.

/// What one process has written to the log so far.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Progress {
    /// Milliseconds of the `started` line.
    pub started_ms: Option<u64>,
    /// Number of monitors the screensaver saw (`monitors: N`).
    pub monitors: Option<u32>,
    /// Milliseconds of each `first frame presented` line, in log order.
    pub first_frames_ms: Vec<u64>,
    /// The `adapter:` line without its prefix.
    pub adapter: Option<String>,
    /// Why the process decided to end: the screensaver's `exit:` line or
    /// the preview's `preview exit:` line, without the prefix.
    pub exit_reason: Option<String>,
}

impl Progress {
    /// Reads the lines of process `pid` from the log text.
    pub fn read(log: &str, pid: u32) -> Self {
        let mut progress = Progress::default();
        let pid_field = format!("pid={pid}");
        for line in log.lines() {
            let Some((ms, rest)) = split_line(line) else {
                continue;
            };
            let Some((field, message)) = rest.split_once(' ') else {
                continue;
            };
            if field != pid_field {
                continue;
            }
            if message.starts_with("Flying Microwave ") && message.contains(" started: ") {
                progress.started_ms.get_or_insert(ms);
            } else if let Some(n) = message.strip_prefix("monitors: ") {
                progress.monitors = n.trim().parse().ok();
            } else if message.ends_with("first frame presented") {
                progress.first_frames_ms.push(ms);
            } else if let Some(adapter) = message.strip_prefix("adapter: ") {
                progress.adapter.get_or_insert_with(|| adapter.to_owned());
            } else if let Some(reason) = message
                .strip_prefix("exit: ")
                .or_else(|| message.strip_prefix("preview exit: "))
            {
                progress
                    .exit_reason
                    .get_or_insert_with(|| reason.to_owned());
            }
        }
        progress
    }

    /// Whether every full-screen window has shown its first frame. Without
    /// monitor information the screensaver makes one window.
    pub fn screensaver_ready(&self) -> bool {
        let expected = self.monitors.unwrap_or(0).max(1) as usize;
        self.first_frames_ms.len() >= expected
    }

    /// Whether the preview has shown its first frame.
    pub fn preview_ready(&self) -> bool {
        !self.first_frames_ms.is_empty()
    }

    /// Milliseconds from the `started` line to the last first frame (the
    /// measure of N-01 in plan 0001).
    pub fn startup_ms(&self) -> Option<u64> {
        Some(self.first_frames_ms.iter().max()? - self.started_ms?)
    }
}

/// The lines of process `pid`, to show when a case fails.
pub fn lines_of(log: &str, pid: u32) -> Vec<&str> {
    let pid_field = format!("pid={pid}");
    log.lines()
        .filter(|line| {
            split_line(line)
                .and_then(|(_, rest)| rest.split_once(' '))
                .is_some_and(|(field, _)| field == pid_field)
        })
        .collect()
}

/// Splits `+123ms rest` into `(123, "rest")`.
fn split_line(line: &str) -> Option<(u64, &str)> {
    let rest = line.strip_prefix('+')?;
    let (ms, rest) = rest.split_once("ms ")?;
    Some((ms.parse().ok()?, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
+0ms pid=10 Flying Microwave 0.1.0 started: Screensaver
+1ms pid=11 Flying Microwave 0.1.0 started: Preview { parent: 5 }
+20ms pid=10 monitors: 2
+21ms pid=10 monitor 0: position Physical { x: 0, y: 0 } size (1920, 1080) scale 1
+300ms pid=10 adapter: Microsoft Basic Render Driver (Cpu, Dx12, driver 10.0)
+410ms pid=10 window 0: first frame presented
+415ms pid=11 preview: first frame presented
+430ms pid=10 window 1: first frame presented
";

    #[test]
    fn reads_only_the_lines_of_the_given_process() {
        let p = Progress::read(LOG, 10);
        assert_eq!(p.started_ms, Some(0));
        assert_eq!(p.monitors, Some(2));
        assert_eq!(p.first_frames_ms, vec![410, 430]);
        assert_eq!(
            p.adapter.as_deref(),
            Some("Microsoft Basic Render Driver (Cpu, Dx12, driver 10.0)")
        );
    }

    #[test]
    fn the_screensaver_is_ready_when_every_monitor_has_a_frame() {
        let partial = "+0ms pid=1 monitors: 2\n+5ms pid=1 window 0: first frame presented\n";
        assert!(!Progress::read(partial, 1).screensaver_ready());
        assert!(Progress::read(LOG, 10).screensaver_ready());
    }

    #[test]
    fn without_monitor_information_one_window_is_expected() {
        let log = "+0ms pid=1 monitors: 0\n+5ms pid=1 window 0: first frame presented\n";
        assert!(Progress::read(log, 1).screensaver_ready());
        assert!(!Progress::read("+0ms pid=1 monitors: 0\n", 1).screensaver_ready());
    }

    #[test]
    fn the_preview_is_ready_after_its_first_frame() {
        assert!(Progress::read(LOG, 11).preview_ready());
        assert!(
            !Progress::read(
                "+0ms pid=11 preview: 5 is not a valid window; exiting\n",
                11
            )
            .preview_ready()
        );
    }

    #[test]
    fn startup_time_runs_from_started_to_the_last_first_frame() {
        assert_eq!(Progress::read(LOG, 10).startup_ms(), Some(430));
        assert_eq!(
            Progress::read("+3ms pid=1 monitors: 1\n", 1).startup_ms(),
            None
        );
    }

    #[test]
    fn the_exit_reason_of_either_mode_is_read() {
        let log = "+0ms pid=1 exit: KeyPressed { synthetic: false }\n\
                   +1ms pid=1 screensaver ended: KeyPressed { synthetic: false }\n\
                   +2ms pid=2 preview exit: WindowDestroyed (exit code 0)\n";
        assert_eq!(
            Progress::read(log, 1).exit_reason.as_deref(),
            Some("KeyPressed { synthetic: false }")
        );
        assert_eq!(
            Progress::read(log, 2).exit_reason.as_deref(),
            Some("WindowDestroyed (exit code 0)")
        );
        assert_eq!(Progress::read(LOG, 10).exit_reason, None);
    }

    #[test]
    fn the_lines_of_one_process_are_kept_for_a_failure_report() {
        let lines = lines_of(LOG, 11);
        assert_eq!(
            lines,
            vec![
                "+1ms pid=11 Flying Microwave 0.1.0 started: Preview { parent: 5 }",
                "+415ms pid=11 preview: first frame presented",
            ]
        );
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let log = "garbage\n+x ms pid=1 monitors: 3\n+5ms\n+6ms pid=1 monitors: 1\n";
        assert_eq!(Progress::read(log, 1).monitors, Some(1));
    }
}
