//! Helpers for the frame diagnostics in the screensaver log.

/// How many events of each kind are logged per window.
const LOGGED: u64 = 3;

/// Whether the event with the 1-based number `count` is one of the first few
/// that are logged, so that a per-frame event cannot flood the log.
pub fn first_few(count: u64) -> bool {
    (1..=LOGGED).contains(&count)
}

/// Why `Target::draw` did not draw (the surface texture was not available).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    Outdated,
    Lost,
    Timeout,
    Occluded,
    Validation,
}

/// Counts of one window, for the summary line in the log (issue #50).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FrameStats {
    received: u64,
    frames: u64,
    paced: u64,
    blank: u64,
    outdated: u64,
    lost: u64,
    timeout: u64,
    occluded: u64,
    validation: u64,
}

impl FrameStats {
    /// Records a `RedrawRequested` event. Returns how many have been received.
    pub fn received(&mut self) -> u64 {
        self.received += 1;
        self.received
    }

    /// Records a drawn frame.
    pub fn drawn(&mut self) {
        self.frames += 1;
    }

    /// Records a frame that `Pace::WaitUntil` held back.
    pub fn paced(&mut self) {
        self.paced += 1;
    }

    /// Records a redraw that left the window black (no target or no GPU).
    pub fn blank(&mut self) {
        self.blank += 1;
    }

    /// Records a skipped frame. Returns how many frames were skipped for
    /// this reason, so that each reason is logged on its own.
    pub fn skipped(&mut self, skip: Skip) -> u64 {
        let count = match skip {
            Skip::Outdated => &mut self.outdated,
            Skip::Lost => &mut self.lost,
            Skip::Timeout => &mut self.timeout,
            Skip::Occluded => &mut self.occluded,
            Skip::Validation => &mut self.validation,
        };
        *count += 1;
        *count
    }

    fn skipped_total(&self) -> u64 {
        self.outdated + self.lost + self.timeout + self.occluded + self.validation
    }

    /// The summary line of window `index`, written when it ends.
    pub fn summary(&self, index: usize) -> String {
        format!(
            "window {index}: {} RedrawRequested received, {} frames, {} paced, {} blank, \
             {} skipped (Outdated {}, Lost {}, Timeout {}, Occluded {}, Validation {})",
            self.received,
            self.frames,
            self.paced,
            self.blank,
            self.skipped_total(),
            self.outdated,
            self.lost,
            self.timeout,
            self.occluded,
            self.validation
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "window 2: 0 RedrawRequested received, 0 frames, 0 paced, 0 blank, \
             0 skipped (Outdated 0, Lost 0, Timeout 0, Occluded 0, Validation 0)"
        );
    }

    #[test]
    fn the_summary_lists_every_count() {
        let mut s = FrameStats::default();
        for _ in 0..9 {
            s.received();
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
        assert_eq!(
            s.summary(0),
            "window 0: 9 RedrawRequested received, 2 frames, 1 paced, 2 blank, \
             4 skipped (Outdated 1, Lost 0, Timeout 0, Occluded 2, Validation 1)"
        );
    }

    #[test]
    fn received_returns_its_running_count() {
        let mut s = FrameStats::default();
        assert_eq!(s.received(), 1);
        assert_eq!(s.received(), 2);
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
    }
}
