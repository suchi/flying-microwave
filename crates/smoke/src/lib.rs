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
    /// Whether the limit on GPU initialization was reached (a line that
    /// starts with `GPU initialization timed out`).
    pub gpu_init_timed_out: bool,
    /// The first `windows hidden in` line without its prefix: how long
    /// hiding the windows took when the exit was decided (plan 0005, D-24).
    pub windows_hidden: Option<String>,
    /// The first `exit cleanup:` line without its prefix (`/s`).
    pub exit_cleanup: Option<String>,
    /// The first `preview cleanup:` line without its prefix (`/p`).
    pub preview_cleanup: Option<String>,
    /// Each line that tells a `get_current_texture` call took long (for
    /// example `window 0: surface texture took 312 ms (draw attempt #7)`).
    pub slow_surface_textures: Vec<String>,
}

/// The counts of one window from its summary line
/// (`window N: A draws attempted, F frames, P paced, B blank, S skipped ...`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSummary {
    pub window: u32,
    pub attempts: u64,
    pub frames: u64,
    pub paced: u64,
    pub blank: u64,
    pub skipped: u64,
    /// The longest wait for the surface texture, the longest present and
    /// the longest whole draw, in ms, when the line has them (only products
    /// that write them; incident 0028).
    pub timings: Option<(u64, u64, u64)>,
}

impl WindowSummary {
    /// Whether every attempt is accounted for by exactly one outcome.
    pub fn is_consistent(&self) -> bool {
        self.attempts == self.frames + self.paced + self.blank + self.skipped
    }
}

impl WindowSummary {
    fn parse(message: &str) -> Option<Self> {
        let rest = message.strip_prefix("window ")?;
        let (window, rest) = rest.split_once(": ")?;
        let (attempts, rest) = rest.split_once(" draws attempted, ")?;
        let (frames, rest) = rest.split_once(" frames, ")?;
        let (paced, rest) = rest.split_once(" paced, ")?;
        let (blank, rest) = rest.split_once(" blank, ")?;
        let (skipped, rest) = rest.split_once(" skipped")?;
        let timings = rest
            .split_once("; longest surface wait ")
            .and_then(|(_, rest)| rest.split_once(" ms, longest present "))
            .and_then(|(wait, rest)| {
                let (present, rest) = rest.split_once(" ms, longest draw ")?;
                let draw = rest.strip_suffix(" ms")?;
                Some((
                    wait.parse().ok()?,
                    present.parse().ok()?,
                    draw.parse().ok()?,
                ))
            });
        Some(Self {
            window: window.parse().ok()?,
            attempts: attempts.parse().ok()?,
            frames: frames.parse().ok()?,
            paced: paced.parse().ok()?,
            blank: blank.parse().ok()?,
            skipped: skipped.parse().ok()?,
            timings,
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
            } else if message.starts_with("GPU initialization timed out") {
                progress.gpu_init_timed_out = true;
            } else if let Some(hidden) = message.strip_prefix("windows hidden in ") {
                progress
                    .windows_hidden
                    .get_or_insert_with(|| hidden.to_owned());
            } else if let Some(cleanup) = message.strip_prefix("exit cleanup: ") {
                progress
                    .exit_cleanup
                    .get_or_insert_with(|| cleanup.to_owned());
            } else if let Some(cleanup) = message.strip_prefix("preview cleanup: ") {
                progress
                    .preview_cleanup
                    .get_or_insert_with(|| cleanup.to_owned());
            } else if message.contains(": surface texture took ") {
                progress.slow_surface_textures.push(message.to_owned());
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

    /// The lines that show where the time of an exit went, for the report of
    /// a failed case: the time to hide the windows, the cleanup and the slow
    /// surface texture calls. Only what the log has.
    pub fn exit_diagnostics(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(hidden) = &self.windows_hidden {
            lines.push(format!("windows hidden in {hidden}"));
        }
        if let Some(cleanup) = &self.exit_cleanup {
            lines.push(format!("exit cleanup: {cleanup}"));
        }
        if let Some(cleanup) = &self.preview_cleanup {
            lines.push(format!("preview cleanup: {cleanup}"));
        }
        lines.extend(self.slow_surface_textures.iter().cloned());
        lines
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

/// One time that a case measured against a limit of the smoke test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timing {
    /// The name of the case (`run`'s `cases`).
    pub case: &'static str,
    /// What was measured.
    pub name: &'static str,
    pub value_ms: u64,
    pub limit_ms: u64,
}

impl Timing {
    /// Whether the value is over half of the limit. Why half (decision of
    /// the maintainer, plan 0005, D-23 (1)): a check that passes near its
    /// limit fails some other time (incidents 0033 and 0035). The check
    /// itself still fails only over the limit.
    pub fn is_over_half(&self) -> bool {
        self.value_ms.saturating_mul(2) > self.limit_ms
    }

    /// The GitHub Actions warning line, when the value is over half of the
    /// limit.
    pub fn warning_line(&self) -> Option<String> {
        self.is_over_half().then(|| {
            format!(
                "::warning::{}: {} {} ms is over half of the limit {} ms",
                self.case, self.name, self.value_ms, self.limit_ms
            )
        })
    }

    /// The line that every recorded time gets at the end of the run.
    pub fn line(&self) -> String {
        format!(
            "timing: {}: {} {} ms (limit {} ms)",
            self.case, self.name, self.value_ms, self.limit_ms
        )
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

/// Whether the microwaves, not just the background, are drawn: the log of
/// one process (from [`lines_of`]) has the `face atlas:` line, one
/// `mesh pipelines ready:` line for every configured surface, and neither a
/// fallback to the background nor a wgpu error. Returns the atlas line.
///
/// Why: the app keeps running on the background alone when the atlas or the
/// pipelines cannot be made, so the other checks would pass (plan 0004,
/// D-6: the pre-release exists to show the microwaves). The atlas is made
/// once and shared by every window (plan 0005, D-13).
pub fn meshes_drawn(lines: &[&str]) -> Result<String, String> {
    let messages: Vec<&str> = lines
        .iter()
        .filter_map(|line| split_line(line))
        .filter_map(|(_, rest)| rest.split_once(' ').map(|(_, message)| message))
        .collect();
    if let Some(bad) = messages
        .iter()
        .find(|m| m.contains("drawing the background only") || m.starts_with("wgpu error"))
    {
        return Err(format!("the microwaves are not drawn: {bad}"));
    }
    let atlas = messages
        .iter()
        .find(|m| m.starts_with("face atlas:"))
        .ok_or("the face atlas was not logged")?;
    // Why: a wgpu error spans several lines, so a fallback message may sit
    // on a continuation line that lines_of drops. Every surface (one per
    // `surface configured:` line) must have made its pipelines.
    let count = |prefix: &str| messages.iter().filter(|m| m.starts_with(prefix)).count();
    let surfaces = count("surface configured:");
    let ready = count("mesh pipelines ready:");
    if ready < surfaces.max(1) {
        return Err(format!(
            "the microwaves are not drawn: mesh pipelines for {ready} of {surfaces} surfaces"
        ));
    }
    Ok((*atlas).to_owned())
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

    const ATLAS: &str =
        "face atlas: 4096x1632, 5 levels, anisotropy 8, rasterized in 50 ms, ready in 90 ms";

    #[test]
    fn meshes_are_drawn_when_the_atlas_and_pipelines_were_made_without_errors() {
        let atlas = format!("+120ms pid=7 {ATLAS}");
        let lines = [
            "+90ms pid=7 surface configured: 1024x768 Bgra8Unorm Fifo",
            atlas.as_str(),
            "+125ms pid=7 mesh pipelines ready: 1024x768",
            "+200ms pid=7 window 0: first frame presented",
        ];
        assert_eq!(meshes_drawn(&lines), Ok(ATLAS.to_owned()));
    }

    #[test]
    fn falling_back_to_the_background_is_a_failure() {
        // The screensaver keeps running on the background alone, so the
        // other checks pass; this one must not.
        let fallback = [
            "+120ms pid=7 creating the face atlas and the meshes failed: out of memory: drawing the background only",
            "+200ms pid=7 window 0: first frame presented",
        ];
        assert!(meshes_drawn(&fallback).is_err());
        let atlas = format!("+120ms pid=7 {ATLAS}");
        let gpu_error = [
            atlas.as_str(),
            "+125ms pid=7 mesh pipelines ready: 1024x768",
            "+130ms pid=7 wgpu error: Validation Error",
        ];
        assert!(meshes_drawn(&gpu_error).is_err());
        assert!(meshes_drawn(&["+1ms pid=7 window 0: first frame presented"]).is_err());
        // An atlas without any pipeline draws nothing.
        assert!(meshes_drawn(&[atlas.as_str()]).is_err());
    }

    #[test]
    fn one_window_without_its_pipelines_is_a_failure() {
        // Review: a wgpu error spans several lines, so the fallback message
        // of a second window may sit on a line without the `+Nms pid=` prefix,
        // which lines_of drops. The count of pipelines still tells.
        let atlas = format!("+95ms pid=7 {ATLAS}");
        let lines = [
            "+90ms pid=7 surface configured: 1920x1080 Bgra8Unorm Fifo",
            atlas.as_str(),
            "+96ms pid=7 mesh pipelines ready: 1920x1080",
            "+100ms pid=7 surface configured: 3840x2160 Bgra8Unorm Fifo",
            "+130ms pid=7 window 0: first frame presented",
        ];
        assert!(meshes_drawn(&lines).is_err());
        // One shared atlas serves both windows.
        let both = [
            "+90ms pid=7 surface configured: 1920x1080 Bgra8Unorm Fifo",
            atlas.as_str(),
            "+96ms pid=7 mesh pipelines ready: 1920x1080",
            "+100ms pid=7 surface configured: 3840x2160 Bgra8Unorm Fifo",
            "+105ms pid=7 mesh pipelines ready: 3840x2160",
        ];
        assert_eq!(meshes_drawn(&both), Ok(ATLAS.to_owned()));
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
+900ms pid=1 window 0: 12 draws attempted, 10 frames, 1 paced, 0 blank, 1 skipped (Outdated 1, Lost 0, Timeout 0, Occluded 0, Validation 0)
+901ms pid=1 window 1: 0 draws attempted, 0 frames, 0 paced, 0 blank, 0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0)
+902ms pid=2 window 0: 99 draws attempted, 99 frames, 0 paced, 0 blank, 0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0)
";
        let p = Progress::read(log, 1);
        assert_eq!(
            p.summary_of(0),
            Some(&WindowSummary {
                window: 0,
                attempts: 12,
                frames: 10,
                paced: 1,
                blank: 0,
                skipped: 1,
                timings: None
            })
        );
        assert!(p.summary_of(0).is_some_and(WindowSummary::is_consistent));
        assert_eq!(p.summary_of(1).map(|s| s.frames), Some(0));
        assert_eq!(p.summary_of(2), None);
        assert_eq!(Progress::read(log, 3).summary_of(0), None);
    }

    #[test]
    fn the_summary_line_gives_the_longest_wait_and_present() {
        // Incident 0028: products that measure them append them to the line.
        let log = "\
+900ms pid=1 window 0: 5 draws attempted, 4 frames, 1 paced, 0 blank, 0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0); longest surface wait 512 ms, longest present 3 ms, longest draw 530 ms
";
        let s = Progress::read(log, 1).summary_of(0).copied();
        assert_eq!(s.map(|s| s.frames), Some(4));
        assert_eq!(s.and_then(|s| s.timings), Some((512, 3, 530)));
    }

    #[test]
    fn the_summary_line_with_the_swap_chain_counts_is_read() {
        // ADR-0028: NotReady skips and draws without the signal.
        let log = "\
+900ms pid=1 window 0: 23 draws attempted, 4 frames, 1 paced, 0 blank, 18 skipped (Outdated 0, Lost 0, Timeout 0, NotReady 18, Occluded 0, Validation 0); 2 drawn without signal; longest surface wait 0 ms, longest present 1 ms, longest draw 4 ms
";
        let s = Progress::read(log, 1).summary_of(0).copied();
        assert_eq!(s.map(|s| s.skipped), Some(18));
        assert!(s.is_some_and(|s| s.is_consistent()));
        assert_eq!(s.and_then(|s| s.timings), Some((0, 1, 4)));
    }

    #[test]
    fn other_window_lines_are_not_summaries() {
        let log = "\
+1ms pid=1 window 0: first frame presented
+2ms pid=1 window 0: draw attempted (#1)
+3ms pid=1 window 0: frame skipped: Occluded
+4ms pid=1 window 0: closed, its monitor is gone
";
        let p = Progress::read(log, 1);
        assert!(p.summaries.is_empty());
        assert_eq!(p.first_frames_ms.len(), 1);
    }

    #[test]
    fn the_exit_lines_are_read_for_the_report_of_a_failure() {
        let log = "\
+500ms pid=1 window 0: surface texture took 312 ms (draw attempt #7)
+900ms pid=1 exit: KeyPressed { synthetic: false }
+901ms pid=1 windows hidden in 4.2 ms
+950ms pid=1 exit cleanup: 5.1 ms after the exit decision; summaries 0.1 ms, windows and surfaces 30.0 ms, GPU 12.5 ms, instance 1.0 ms
+960ms pid=2 preview cleanup: 0.2 ms after the exit decision; surface 1.0 ms, GPU 2.0 ms, window 3.0 ms
+970ms pid=2 preview: surface texture took 150 ms (draw attempt #2)
+980ms pid=3 exit cleanup: no exit decision; summaries 0.1 ms, windows and surfaces 0.1 ms, GPU 0.1 ms, instance 0.1 ms
";
        let p = Progress::read(log, 1);
        assert_eq!(p.windows_hidden.as_deref(), Some("4.2 ms"));
        assert_eq!(
            p.exit_cleanup.as_deref(),
            Some(
                "5.1 ms after the exit decision; summaries 0.1 ms, windows and surfaces 30.0 ms, \
                 GPU 12.5 ms, instance 1.0 ms"
            )
        );
        assert_eq!(p.preview_cleanup, None);
        assert_eq!(
            p.slow_surface_textures,
            vec!["window 0: surface texture took 312 ms (draw attempt #7)"]
        );
        assert_eq!(p.exit_diagnostics().len(), 3);
        let preview = Progress::read(log, 2);
        assert!(preview.preview_cleanup.is_some());
        assert_eq!(
            preview.slow_surface_textures,
            vec!["preview: surface texture took 150 ms (draw attempt #2)"]
        );
        assert!(
            Progress::read(log, 3)
                .exit_cleanup
                .is_some_and(|c| c.starts_with("no exit decision"))
        );
        assert!(Progress::read(log, 4).exit_diagnostics().is_empty());
    }

    #[test]
    fn a_time_over_half_of_its_limit_is_warned_about() {
        let timing = |value_ms| Timing {
            case: "F-01 /s ends on user input",
            name: "input to screen gone",
            value_ms,
            limit_ms: 200,
        };
        assert!(!timing(100).is_over_half());
        assert!(timing(101).is_over_half());
        assert_eq!(timing(100).warning_line(), None);
        assert_eq!(
            timing(150).warning_line().as_deref(),
            Some(
                "::warning::F-01 /s ends on user input: input to screen gone 150 ms \
                 is over half of the limit 200 ms"
            )
        );
        // Over the limit is also over half.
        assert!(timing(500).warning_line().is_some());
    }

    #[test]
    fn every_time_gets_a_timing_line() {
        let timing = Timing {
            case: "F-02 /p ends with its parent",
            name: "parent destroyed to exit",
            value_ms: 40,
            limit_ms: 2000,
        };
        assert_eq!(
            timing.line(),
            "timing: F-02 /p ends with its parent: parent destroyed to exit 40 ms (limit 2000 ms)"
        );
    }

    #[test]
    fn the_gpu_initialization_limit_is_read_from_its_line() {
        // Plan 0005, D-1 (4): the settings window shows a message box both
        // when the limit is reached and when the setup fails, and ends with
        // exit code 7 either way, so only the log line tells them apart.
        let log = "\
+3330ms pid=1 adapter: Microsoft Basic Render Driver (Cpu, Dx12, driver 10.0.26100.33648)
+15138ms pid=1 GPU initialization timed out; exiting
+15002ms pid=2 GPU initialization timed out; the settings window could not be opened
+400ms pid=3 settings: the window could not be opened: app creation error
";
        assert!(Progress::read(log, 1).gpu_init_timed_out);
        assert!(Progress::read(log, 2).gpu_init_timed_out);
        assert!(!Progress::read(log, 3).gpu_init_timed_out);
        assert!(!Progress::read(log, 4).gpu_init_timed_out);
    }
}
