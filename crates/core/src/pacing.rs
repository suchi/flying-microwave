//! Frame rate limit (`display.max_fps`; docs/design/settings.md).
//!
//! Presentation stays vertically synchronized; the limit only skips frames.
//! The caller asks [`FramePacer::poll`] before drawing and, when told to
//! wait, wakes its event loop at the returned time. Times are passed in, as
//! in [`crate::exit_policy`], so the rules can be tested without a clock.

use std::time::Duration;

/// What to do when a frame could be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// Draw now.
    Draw,
    /// Do not draw before this time.
    WaitUntil(Duration),
}

/// Spaces frames at most `max_fps` per second.
#[derive(Debug, Clone)]
pub struct FramePacer {
    interval: Option<Duration>,
    next_due: Option<Duration>,
}

impl FramePacer {
    /// Creates a pacer for the limit `max_fps` (0: no limit) on a monitor
    /// that refreshes `refresh_millihertz` times per thousand seconds, if
    /// known. A limit at or above the refresh rate needs no pacing: the
    /// synchronized presentation already keeps to it.
    pub fn new(max_fps: u32, refresh_millihertz: Option<u32>) -> Self {
        let refresh = refresh_millihertz.filter(|&r| r > 0);
        let needed =
            max_fps > 0 && refresh.is_none_or(|r| u64::from(max_fps) * 1000 < u64::from(r));
        Self {
            interval: needed.then(|| Duration::from_secs(1) / max_fps),
            next_due: None,
        }
    }

    /// Whether frames are skipped at all.
    pub fn limits(&self) -> bool {
        self.interval.is_some()
    }

    /// Called whenever a frame could be drawn; `now` is the time since any
    /// fixed start, the same for every call.
    pub fn poll(&mut self, now: Duration) -> Pace {
        let Some(interval) = self.interval else {
            return Pace::Draw;
        };
        match self.next_due {
            Some(due) if now < due => Pace::WaitUntil(due),
            Some(due) => {
                // Keep to the schedule, so waking late does not lower the
                // rate; after a stall, start a new schedule instead of
                // drawing a burst of frames.
                let next = due + interval;
                self.next_due = Some(if next > now { next } else { now + interval });
                Pace::Draw
            }
            None => {
                self.next_due = Some(now + interval);
                Pace::Draw
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn no_limit_always_draws() {
        let mut pacer = FramePacer::new(0, Some(60_000));
        assert!(!pacer.limits());
        for t in [0, 1, 2, 3] {
            assert_eq!(pacer.poll(ms(t)), Pace::Draw);
        }
    }

    #[test]
    fn a_limit_at_or_above_the_refresh_rate_does_not_skip_frames() {
        assert!(!FramePacer::new(60, Some(60_000)).limits());
        assert!(!FramePacer::new(120, Some(59_940)).limits());
        assert!(!FramePacer::new(60, Some(59_950)).limits());
    }

    #[test]
    fn a_limit_below_the_refresh_rate_skips_frames() {
        assert!(FramePacer::new(60, Some(144_000)).limits());
        assert!(FramePacer::new(30, Some(60_000)).limits());
    }

    #[test]
    fn an_unknown_refresh_rate_keeps_the_limit() {
        assert!(FramePacer::new(60, None).limits());
        assert!(FramePacer::new(60, Some(0)).limits());
    }

    #[test]
    fn frames_are_spaced_by_the_interval() {
        let mut pacer = FramePacer::new(50, None);
        assert_eq!(pacer.poll(ms(1000)), Pace::Draw);
        assert_eq!(pacer.poll(ms(1007)), Pace::WaitUntil(ms(1020)));
        assert_eq!(pacer.poll(ms(1019)), Pace::WaitUntil(ms(1020)));
        assert_eq!(pacer.poll(ms(1020)), Pace::Draw);
        assert_eq!(pacer.poll(ms(1021)), Pace::WaitUntil(ms(1040)));
    }

    #[test]
    fn a_late_frame_does_not_shift_the_schedule() {
        // Waking a little late (the next vertical blank) must not lower the
        // average rate below the limit.
        let mut pacer = FramePacer::new(50, None);
        assert_eq!(pacer.poll(ms(0)), Pace::Draw);
        assert_eq!(pacer.poll(ms(27)), Pace::Draw);
        assert_eq!(pacer.poll(ms(28)), Pace::WaitUntil(ms(40)));
        assert_eq!(pacer.poll(ms(40)), Pace::Draw);
    }

    #[test]
    fn after_a_long_pause_the_schedule_starts_again() {
        // No burst of frames to catch up after a stall.
        let mut pacer = FramePacer::new(50, None);
        assert_eq!(pacer.poll(ms(0)), Pace::Draw);
        assert_eq!(pacer.poll(ms(500)), Pace::Draw);
        assert_eq!(pacer.poll(ms(501)), Pace::WaitUntil(ms(520)));
    }

    #[test]
    fn thirty_frames_a_second_over_one_second() {
        let mut pacer = FramePacer::new(30, Some(60_000));
        // A 60 Hz display offers a frame every 16.67 ms.
        let drawn = (0..60u64)
            .map(|i| Duration::from_micros(i * 16_667))
            .filter(|&t| pacer.poll(t) == Pace::Draw)
            .count();
        assert_eq!(drawn, 30);
    }
}
