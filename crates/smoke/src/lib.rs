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
    /// The first `config:` line without its prefix: how the settings were
    /// read (plan 0004, F-06). Later `config:` lines name the items that
    /// fell back.
    pub config: Option<String>,
    /// Why the process decided to end: the screensaver's `exit:` line or
    /// the preview's `preview exit:` line, without the prefix.
    pub exit_reason: Option<String>,
    /// The summary line of each window, in log order.
    pub summaries: Vec<WindowSummary>,
}

/// The counts of one window from its summary line
/// (`window N: R RedrawRequested received, F frames, ...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSummary {
    pub window: u32,
    pub received: u64,
    pub frames: u64,
}

impl WindowSummary {
    fn parse(message: &str) -> Option<Self> {
        let rest = message.strip_prefix("window ")?;
        let (window, rest) = rest.split_once(": ")?;
        let (received, rest) = rest.split_once(" RedrawRequested received, ")?;
        let (frames, _) = rest.split_once(" frames, ")?;
        Some(Self {
            window: window.parse().ok()?,
            received: received.parse().ok()?,
            frames: frames.parse().ok()?,
        })
    }
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
            } else if let Some(n) = message
                .strip_prefix("monitors: ")
                .and_then(|n| n.trim().parse().ok())
            {
                progress.monitors = Some(n);
            } else if message.ends_with("first frame presented") {
                progress.first_frames_ms.push(ms);
            } else if let Some(summary) = WindowSummary::parse(message) {
                progress.summaries.push(summary);
            } else if let Some(adapter) = message.strip_prefix("adapter: ") {
                progress.adapter.get_or_insert_with(|| adapter.to_owned());
            } else if let Some(config) = message.strip_prefix("config: ") {
                progress.config.get_or_insert_with(|| config.to_owned());
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

    /// The summary of window `window`, if its line was logged.
    pub fn summary_of(&self, window: u32) -> Option<&WindowSummary> {
        self.summaries.iter().find(|s| s.window == window)
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

/// Whether the sprites, not just the background, are drawn: the log of one
/// process (from [`lines_of`]) has the `sprite atlas:` line and neither a
/// fallback to the background nor a wgpu error. Returns the atlas line.
///
/// Why: the app keeps running on the background alone when the sprite
/// pipeline or the atlas cannot be made, so the other checks would pass
/// (plan 0004, D-6: the pre-release exists to show the sprites).
pub fn sprites_drawn(lines: &[&str]) -> Result<String, String> {
    let messages: Vec<&str> = lines
        .iter()
        .filter_map(|line| split_line(line))
        .filter_map(|(_, rest)| rest.split_once(' ').map(|(_, message)| message))
        .collect();
    if let Some(bad) = messages
        .iter()
        .find(|m| m.contains("drawing the background only") || m.starts_with("wgpu error"))
    {
        return Err(format!("the sprites are not drawn: {bad}"));
    }
    // Why: a wgpu error spans several lines, so a fallback message may sit
    // on a continuation line that lines_of drops. Every surface (one per
    // `surface configured:` line) must have made its atlas.
    let surfaces = messages
        .iter()
        .filter(|m| m.starts_with("surface configured:"))
        .count();
    let atlases: Vec<&str> = messages
        .iter()
        .copied()
        .filter(|m| m.starts_with("sprite atlas:"))
        .collect();
    match atlases.first() {
        Some(first) if atlases.len() >= surfaces => Ok((*first).to_owned()),
        Some(_) => Err(format!(
            "the sprites are not drawn: {} sprite atlases for {surfaces} surfaces",
            atlases.len()
        )),
        None => Err("the sprite atlas was not logged".to_owned()),
    }
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

    #[test]
    fn sprites_are_drawn_when_the_atlas_was_made_without_errors() {
        let lines = [
            "+90ms pid=7 surface configured: 1024x768 Bgra8Unorm Fifo",
            "+120ms pid=7 sprite atlas: 512x128 in 3 ms",
            "+200ms pid=7 window 0: first frame presented",
        ];
        assert_eq!(
            sprites_drawn(&lines),
            Ok("sprite atlas: 512x128 in 3 ms".to_owned())
        );
    }

    #[test]
    fn falling_back_to_the_background_is_a_failure() {
        // The screensaver keeps running on the background alone, so the
        // other checks pass; this one must not.
        let fallback = [
            "+120ms pid=7 sprite pipeline: shader failed: drawing the background only",
            "+200ms pid=7 window 0: first frame presented",
        ];
        assert!(sprites_drawn(&fallback).is_err());
        let gpu_error = [
            "+120ms pid=7 sprite atlas: 512x128 in 3 ms",
            "+130ms pid=7 wgpu error: Validation Error",
        ];
        assert!(sprites_drawn(&gpu_error).is_err());
        assert!(sprites_drawn(&["+1ms pid=7 window 0: first frame presented"]).is_err());
    }

    #[test]
    fn one_window_without_its_atlas_is_a_failure() {
        // Review: a wgpu error spans several lines, so the fallback message
        // of a second window may sit on a line without the `+Nms pid=` prefix,
        // which lines_of drops. The count of atlases still tells.
        let lines = [
            "+90ms pid=7 surface configured: 1920x1080 Bgra8Unorm Fifo",
            "+95ms pid=7 sprite atlas: 512x128 in 3 ms",
            "+100ms pid=7 surface configured: 3840x2160 Bgra8Unorm Fifo",
            "+130ms pid=7 window 0: first frame presented",
        ];
        assert!(sprites_drawn(&lines).is_err());
        let both = [
            "+90ms pid=7 surface configured: 1920x1080 Bgra8Unorm Fifo",
            "+95ms pid=7 sprite atlas: 512x128 in 3 ms",
            "+100ms pid=7 surface configured: 3840x2160 Bgra8Unorm Fifo",
            "+105ms pid=7 sprite atlas: 1024x256 in 6 ms",
        ];
        assert!(sprites_drawn(&both).is_ok());
    }

    const LOG: &str = "\
+0ms pid=10 Flying Microwave 0.1.0 started: Screensaver
+1ms pid=11 Flying Microwave 0.1.0 started: Preview { parent: 5 }
+20ms pid=10 monitors: 2
+21ms pid=10 monitor 0: position Physical { x: 0, y: 0 } size (1920, 1080) scale 1
+30ms pid=10 config: not valid TOML; using the defaults (folder from FLYING_MICROWAVE_CONFIG_DIR) in 0.4 ms (waited 0 ms after the windows)
+31ms pid=10 config: flight.count: out of range; clamped
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
    fn the_first_config_line_tells_how_the_settings_were_read() {
        assert_eq!(
            Progress::read(LOG, 10).config.as_deref(),
            Some(
                "not valid TOML; using the defaults (folder from FLYING_MICROWAVE_CONFIG_DIR) \
                 in 0.4 ms (waited 0 ms after the windows)"
            )
        );
        assert_eq!(Progress::read(LOG, 11).config, None);
    }

    #[test]
    fn the_screensaver_is_ready_when_every_monitor_has_a_frame() {
        let partial = "+0ms pid=1 monitors: 2\n+5ms pid=1 window 0: first frame presented\n";
        assert!(!Progress::read(partial, 1).screensaver_ready());
        assert!(Progress::read(LOG, 10).screensaver_ready());
    }

    #[test]
    fn a_monitors_line_without_a_count_keeps_the_count() {
        // Review: a line such as "monitors: 1 left out, ..." must not clear
        // the monitor count, which would make one frame look like enough.
        let log = "+0ms pid=1 monitors: 2\n+1ms pid=1 monitors: 1 left out, their area is unknown\n+5ms pid=1 window 0: first frame presented\n";
        assert!(!Progress::read(log, 1).screensaver_ready());
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

    #[test]
    fn the_summary_line_gives_the_counts_of_each_window() {
        let log = "\
+900ms pid=1 window 0: 12 RedrawRequested received, 10 frames, 1 paced, 0 blank, 1 skipped (Outdated 1, Lost 0, Timeout 0, Occluded 0, Validation 0)
+901ms pid=1 window 1: 0 RedrawRequested received, 0 frames, 0 paced, 0 blank, 0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0)
+902ms pid=2 window 0: 99 RedrawRequested received, 99 frames, 0 paced, 0 blank, 0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0)
";
        let p = Progress::read(log, 1);
        assert_eq!(
            p.summary_of(0),
            Some(&WindowSummary {
                window: 0,
                received: 12,
                frames: 10
            })
        );
        assert_eq!(p.summary_of(1).map(|s| s.frames), Some(0));
        assert_eq!(p.summary_of(2), None);
        assert_eq!(Progress::read(log, 3).summary_of(0), None);
    }

    #[test]
    fn other_window_lines_are_not_summaries() {
        let log = "\
+1ms pid=1 window 0: first frame presented
+2ms pid=1 window 0: RedrawRequested received (#1)
+3ms pid=1 window 0: frame skipped: Occluded
+4ms pid=1 window 0: closed, its monitor is gone
";
        let p = Progress::read(log, 1);
        assert!(p.summaries.is_empty());
        assert_eq!(p.first_frames_ms.len(), 1);
    }
}
