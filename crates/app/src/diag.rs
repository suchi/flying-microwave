//! Helpers for the frame diagnostics in the screensaver log.

use std::time::Duration;

/// How many events of each kind are logged per window.
const LOGGED: u64 = 3;

/// Whether the event with the 1-based number `count` is one of the first few
/// that are logged, so that a per-frame event cannot flood the log.
pub fn first_few(count: u64) -> bool {
    (1..=LOGGED).contains(&count)
}

/// A `get_current_texture` call that takes longer than this is logged.
///
/// Why: incident 0035 and plan 0005, D-24 (6): the exit time of `/s` was
/// near its limit, and the log did not show whether the surface texture
/// call was the slow part. 100 ms is well above a frame and well below the
/// waits that hold the input.
pub const SLOW_ACQUIRE: Duration = Duration::from_millis(100);
/// How many slow `get_current_texture` calls are logged per window.
///
/// Why: as `SLOW_ACQUIRE`; a window that is slow on every frame must not
/// flood the log.
pub const SLOW_ACQUIRE_LOGGED: u32 = 5;

/// Whether a `get_current_texture` call that took `took` is logged, when
/// `logged` slow calls of the window have been logged already.
pub fn log_slow_acquire(took: Duration, logged: u32) -> bool {
    took > SLOW_ACQUIRE && logged < SLOW_ACQUIRE_LOGGED
}

/// A time in milliseconds with one decimal, for the log.
pub fn ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1000.0
}

/// How long after the exit decision the cleanup started, for the cleanup
/// lines; `None` when the loop ended without an exit decision.
pub fn after_exit_decision(since: Option<Duration>) -> String {
    match since {
        Some(time) => format!("{:.1} ms after the exit decision", ms(time)),
        None => "no exit decision".to_owned(),
    }
}

/// Why `Target::draw` did not draw (the surface texture was not available).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    Outdated,
    Lost,
    /// wgpu gave up waiting for the surface texture.
    Timeout,
    /// The swap chain could not take another frame yet; the draw did not
    /// wait for it (ADR-0028).
    NotReady,
    Occluded,
    Validation,
}

/// Counts of one window, for the summary line in the log (issue #50).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FrameStats {
    attempts: u64,
    frames: u64,
    paced: u64,
    blank: u64,
    outdated: u64,
    lost: u64,
    timeout: u64,
    not_ready: u64,
    occluded: u64,
    validation: u64,
    /// Frames that were presented although the swap chain had not
    /// signalled (ADR-0028); also counted in `frames`.
    without_signal: u64,
    /// The longest wait for the surface texture, the longest present and
    /// the longest whole draw (incident 0028: the log has no line for the
    /// later frames).
    longest_wait: Duration,
    longest_present: Duration,
    longest_draw: Duration,
}

impl FrameStats {
    /// Records an attempt to draw. Returns how many have been made. Every
    /// attempt ends as exactly one of drawn, paced, blank or skipped.
    pub fn attempted(&mut self) -> u64 {
        self.attempts += 1;
        self.attempts
    }

    /// Records a drawn frame.
    pub fn drawn(&mut self) {
        self.frames += 1;
    }

    /// Records a frame that `Pace::WaitUntil` held back.
    pub fn paced(&mut self) {
        self.paced += 1;
    }

    /// Records a frame that was presented without the swap chain signal,
    /// after `pacing::SIGNAL_WAIT_LIMIT` (ADR-0028).
    pub fn drawn_without_signal(&mut self) {
        self.without_signal += 1;
    }

    /// Records an attempt that left the window black (no target or no GPU).
    pub fn blank(&mut self) {
        self.blank += 1;
    }

    /// Records how long one draw waited for the surface texture, took to
    /// present and took in all, keeping the longest of each. Draws of the
    /// attempts that the log already shows with their times (`first_few`)
    /// are left out, so that the values are those of the later draws, which
    /// the log does not show (plan 0005, D-19).
    pub fn timed(&mut self, wait: Duration, present: Duration, draw: Duration) {
        if first_few(self.attempts) {
            return;
        }
        self.longest_wait = self.longest_wait.max(wait);
        self.longest_present = self.longest_present.max(present);
        self.longest_draw = self.longest_draw.max(draw);
    }

    /// Records a skipped frame. Returns how many frames were skipped for
    /// this reason, so that each reason is logged on its own.
    pub fn skipped(&mut self, skip: Skip) -> u64 {
        let count = match skip {
            Skip::Outdated => &mut self.outdated,
            Skip::Lost => &mut self.lost,
            Skip::Timeout => &mut self.timeout,
            Skip::NotReady => &mut self.not_ready,
            Skip::Occluded => &mut self.occluded,
            Skip::Validation => &mut self.validation,
        };
        *count += 1;
        *count
    }

    fn skipped_total(&self) -> u64 {
        self.outdated + self.lost + self.timeout + self.not_ready + self.occluded + self.validation
    }

    /// The summary line of window `index`, written when it ends.
    pub fn summary(&self, index: usize) -> String {
        format!(
            "window {index}: {} draws attempted, {} frames, {} paced, {} blank, \
             {} skipped (Outdated {}, Lost {}, Timeout {}, NotReady {}, Occluded {}, \
             Validation {}); {} drawn without signal; \
             longest surface wait {} ms, longest present {} ms, longest draw {} ms",
            self.attempts,
            self.frames,
            self.paced,
            self.blank,
            self.skipped_total(),
            self.outdated,
            self.lost,
            self.timeout,
            self.not_ready,
            self.occluded,
            self.validation,
            self.without_signal,
            self.longest_wait.as_millis(),
            self.longest_present.as_millis(),
            self.longest_draw.as_millis()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_surface_texture_call_over_100_ms_is_logged() {
        assert!(log_slow_acquire(Duration::from_millis(101), 0));
        assert!(log_slow_acquire(Duration::from_millis(312), 4));
    }

    #[test]
    fn a_surface_texture_call_of_100_ms_or_less_is_not_logged() {
        assert!(!log_slow_acquire(Duration::from_millis(100), 0));
        assert!(!log_slow_acquire(Duration::ZERO, 0));
    }

    #[test]
    fn only_the_first_five_slow_surface_texture_calls_are_logged() {
        let slow = Duration::from_secs(1);
        assert!(log_slow_acquire(slow, 4));
        assert!(!log_slow_acquire(slow, 5));
        assert!(!log_slow_acquire(slow, u32::MAX));
    }

    #[test]
    fn the_time_since_the_exit_decision_is_worded_for_the_cleanup_lines() {
        assert_eq!(
            after_exit_decision(Some(Duration::from_micros(2340))),
            "2.3 ms after the exit decision"
        );
        assert_eq!(after_exit_decision(None), "no exit decision");
    }

    #[test]
    fn times_are_given_in_milliseconds() {
        assert!((ms(Duration::from_micros(1500)) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn the_first_three_events_are_logged() {
        assert!(first_few(1));
        assert!(first_few(2));
        assert!(first_few(3));
    }

    #[test]
    fn later_events_are_not_logged() {
        assert!(!first_few(4));
        assert!(!first_few(1000));
        assert!(!first_few(u64::MAX));
    }

    #[test]
    fn zero_is_not_an_event_number() {
        assert!(!first_few(0));
    }

    #[test]
    fn a_new_window_has_an_all_zero_summary() {
        assert_eq!(
            FrameStats::default().summary(2),
            "window 2: 0 draws attempted, 0 frames, 0 paced, 0 blank, \
             0 skipped (Outdated 0, Lost 0, Timeout 0, NotReady 0, Occluded 0, Validation 0); \
             0 drawn without signal; \
             longest surface wait 0 ms, longest present 0 ms, longest draw 0 ms"
        );
    }

    #[test]
    fn the_summary_lists_every_count() {
        let mut s = FrameStats::default();
        for _ in 0..10 {
            s.attempted();
        }
        s.drawn();
        s.drawn();
        s.paced();
        s.blank();
        s.blank();
        s.skipped(Skip::Outdated);
        s.skipped(Skip::Occluded);
        s.skipped(Skip::Occluded);
        s.skipped(Skip::Validation);
        s.skipped(Skip::NotReady);
        s.drawn_without_signal();
        s.timed(
            Duration::from_millis(12),
            Duration::from_millis(3),
            Duration::from_millis(20),
        );
        assert_eq!(
            s.summary(0),
            "window 0: 10 draws attempted, 2 frames, 1 paced, 2 blank, \
             5 skipped (Outdated 1, Lost 0, Timeout 0, NotReady 1, Occluded 2, Validation 1); \
             1 drawn without signal; \
             longest surface wait 12 ms, longest present 3 ms, longest draw 20 ms"
        );
    }

    #[test]
    fn the_logged_attempts_are_left_out_of_the_longest() {
        // The first few attempts are in the log with their times; the
        // longest values are for the later ones, which are not.
        let mut s = FrameStats::default();
        let ms = Duration::from_millis;
        for _ in 0..LOGGED {
            s.attempted();
            s.timed(ms(900), ms(900), ms(900));
        }
        assert!(
            s.summary(0)
                .ends_with("; longest surface wait 0 ms, longest present 0 ms, longest draw 0 ms"),
            "{}",
            s.summary(0)
        );
        s.attempted();
        s.timed(ms(40), ms(2), ms(45));
        assert!(
            s.summary(0).ends_with(
                "; longest surface wait 40 ms, longest present 2 ms, longest draw 45 ms"
            ),
            "{}",
            s.summary(0)
        );
    }

    #[test]
    fn the_longest_wait_and_present_are_kept_apart() {
        // Incident 0028: where a frame waits is not in the log after the
        // first few attempts, so the summary keeps the longest of each.
        let mut s = FrameStats::default();
        let ms = Duration::from_millis;
        s.timed(ms(5), ms(40), ms(60));
        s.timed(ms(700), ms(2), ms(705));
        s.timed(ms(30), ms(9), ms(900));
        assert!(
            s.summary(0).ends_with(
                "; longest surface wait 700 ms, longest present 40 ms, longest draw 900 ms"
            ),
            "{}",
            s.summary(0)
        );
    }

    #[test]
    fn attempted_returns_its_running_count() {
        let mut s = FrameStats::default();
        assert_eq!(s.attempted(), 1);
        assert_eq!(s.attempted(), 2);
    }

    #[test]
    fn skips_are_counted_per_reason() {
        let mut s = FrameStats::default();
        assert_eq!(s.skipped(Skip::Lost), 1);
        assert_eq!(s.skipped(Skip::Timeout), 1);
        assert_eq!(s.skipped(Skip::Lost), 2);
        assert_eq!(s.skipped(Skip::Outdated), 1);
        assert_eq!(s.skipped(Skip::Occluded), 1);
        assert_eq!(s.skipped(Skip::Validation), 1);
        assert_eq!(s.skipped(Skip::NotReady), 1);
        assert_eq!(s.skipped(Skip::NotReady), 2);
        assert_eq!(s.skipped(Skip::Timeout), 2);
    }
}
