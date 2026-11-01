//! Small pure helpers shared by the modes. No platform dependencies, so they
//! are unit tested on any host.

use std::f64::consts::TAU;
use std::time::{Duration, Instant};

/// Grace period after the windows appear during which only cursor movement and
/// focus changes are ignored (a cursor jitter caused by window creation, focus
/// moving from window to window, and so on). Every other exit trigger (key
/// press, mouse button press, wheel, touch, close request) ends the
/// screensaver immediately, even during this period (docs/design/architecture.md,
/// section "入力と終了判定").
pub const GRACE_PERIOD: Duration = Duration::from_millis(500);

/// Physical pixels the cursor must travel from the reference position (the
/// position when the grace period ended) before the screensaver exits.
pub const CURSOR_EXIT_DISTANCE: i32 = 16;

/// Length of one full color cycle. Guideline G-3 asks for no flashing, so the
/// color drifts slowly: at most about 0.014 per second per channel.
pub const COLOR_CYCLE_SECONDS: f64 = 12.0;

/// Returns true when `now` is at least `threshold` physical pixels away from
/// `start` (Euclidean distance, screen coordinates).
pub fn cursor_moved_enough(start: (i32, i32), now: (i32, i32), threshold: i32) -> bool {
    // i128: the squared distance of two arbitrary i32 points overflows i64.
    let dx = i128::from(now.0) - i128::from(start.0);
    let dy = i128::from(now.1) - i128::from(start.1);
    let limit = i128::from(threshold);
    dx * dx + dy * dy >= limit * limit
}

/// Returns true if a keyboard event ends the screensaver: only a press that is
/// not synthetic. Releases can belong to the key that started a preview, and
/// winit generates synthetic events for keys that are held while the focus
/// moves between windows. Not subject to the grace period.
pub fn key_ends_screensaver(pressed: bool, synthetic: bool) -> bool {
    pressed && !synthetic
}

/// What [`ExitPolicy::cursor_moved`] decided about one cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDecision {
    /// The grace period is running: the movement is ignored and the position
    /// became the new reference.
    IgnoredInGrace,
    /// No reference position was known, so this position became the reference.
    ReferenceSet,
    /// The cursor is less than the threshold away from the reference.
    BelowThreshold,
    /// The cursor is at least the threshold away from `from`: exit.
    Exit { from: (i32, i32) },
}

/// Cursor and grace period part of the `/s` exit rule. Pure state: the caller
/// passes the current time and the cursor position (screen coordinates), so it
/// is unit tested on any host.
///
/// Rules (docs/design/architecture.md): cursor movement during the grace
/// period is ignored and re-bases the reference position; the reference is
/// re-taken once more when the grace period ends; afterwards a movement of at
/// least [`CURSOR_EXIT_DISTANCE`] from the reference ends the screensaver.
#[derive(Debug)]
pub struct ExitPolicy {
    reference: Option<(i32, i32)>,
    grace_until: Option<Instant>,
    /// True from `start_grace` until the end of the grace period was handled.
    grace_end_pending: bool,
}

impl ExitPolicy {
    /// `reference` is the cursor position at startup (`None` if unknown). No
    /// grace period is running until [`ExitPolicy::start_grace`] is called.
    pub fn new(reference: Option<(i32, i32)>) -> Self {
        Self {
            reference,
            grace_until: None,
            grace_end_pending: false,
        }
    }

    /// Starts the grace period at `now`.
    pub fn start_grace(&mut self, now: Instant, grace: Duration) {
        self.grace_until = Some(now + grace);
        self.grace_end_pending = true;
    }

    /// The reference position the cursor distance is measured from.
    pub fn reference(&self) -> Option<(i32, i32)> {
        self.reference
    }

    /// True while the grace period is running.
    pub fn in_grace(&self, now: Instant) -> bool {
        self.grace_until.is_some_and(|until| now < until)
    }

    /// When the end of the grace period has to be handled (so that the caller
    /// can wake up then), or `None` if there is nothing pending.
    pub fn pending_grace_end(&self) -> Option<Instant> {
        self.grace_until.filter(|_| self.grace_end_pending)
    }

    /// True if the grace period is over but [`ExitPolicy::finish_grace`] has
    /// not been called yet.
    pub fn grace_end_due(&self, now: Instant) -> bool {
        self.pending_grace_end().is_some_and(|until| now >= until)
    }

    /// Handles the end of the grace period: the reference is re-taken from the
    /// current cursor position (kept unchanged if the position is unknown).
    pub fn finish_grace(&mut self, cursor: Option<(i32, i32)>) {
        self.grace_end_pending = false;
        if cursor.is_some() {
            self.reference = cursor;
        }
    }

    /// Judges a cursor movement. `cursor` is the screen position from
    /// `GetCursorPos`, not the window relative one.
    pub fn cursor_moved(&mut self, now: Instant, cursor: (i32, i32)) -> CursorDecision {
        if self.in_grace(now) {
            self.reference = Some(cursor);
            return CursorDecision::IgnoredInGrace;
        }
        // Why: if this event is the first one after the grace period and
        // arrives before `finish_grace` ran, the reference stays what the last
        // movement during the grace period set. Re-taking it here would swallow
        // this very movement.
        self.grace_end_pending = false;
        match self.reference {
            None => {
                self.reference = Some(cursor);
                CursorDecision::ReferenceSet
            }
            Some(from) if cursor_moved_enough(from, cursor, CURSOR_EXIT_DISTANCE) => {
                CursorDecision::Exit { from }
            }
            Some(_) => CursorDecision::BelowThreshold,
        }
    }
}

/// A slowly changing, always dark RGB color (each channel stays within
/// 0.05..=0.10), periodic with [`COLOR_CYCLE_SECONDS`].
pub fn dark_color(elapsed: Duration) -> [f64; 3] {
    const BASE: f64 = 0.05;
    const SWING: f64 = 0.05;
    let phase = TAU * (elapsed.as_secs_f64() % COLOR_CYCLE_SECONDS) / COLOR_CYCLE_SECONDS;
    // The three channels are offset by a third of a cycle.
    let channel = |index: f64| BASE + SWING * 0.5 * (1.0 + (phase + index * TAU / 3.0).sin());
    [channel(0.0), channel(1.0), channel(2.0)]
}

/// Color of the marker square, as 8-bit RGB. A medium, warm color: bright
/// enough to see against the dark background, but the element is small
/// (guideline G-5) and it never flashes (G-3).
pub const MARKER_RGB: [u8; 3] = [150, 110, 60];

/// Horizontal and vertical periods of the marker movement. Different periods
/// make the square wander over the whole area instead of along one line, so
/// no pixel stays lit (guideline G-4).
const MARKER_PERIODS_SECONDS: (f64, f64) = (23.0, 17.0);

/// Edge length of the marker square: an eighth of the shorter edge of the
/// area, at least 4 pixels, and never larger than the area itself.
pub fn marker_side(width: u32, height: u32) -> u32 {
    let shorter = width.min(height);
    (shorter / 8).max(4).min(shorter)
}

/// Top-left corner of the marker square at `elapsed`. The square starts at
/// the center and moves smoothly, always fully inside the area.
pub fn marker_origin(elapsed: Duration, width: u32, height: u32, side: u32) -> (u32, u32) {
    let t = elapsed.as_secs_f64();
    let along = |range: u32, period: f64| {
        let fraction = 0.5 + 0.5 * (TAU * t / period).sin();
        // `fraction` is within 0..=1, so the result stays within 0..=range.
        (f64::from(range) * fraction).round() as u32
    };
    (
        along(width.saturating_sub(side), MARKER_PERIODS_SECONDS.0),
        along(height.saturating_sub(side), MARKER_PERIODS_SECONDS.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_side_is_an_eighth_of_the_shorter_edge() {
        assert_eq!(marker_side(152, 112), 14);
        assert_eq!(marker_side(1600, 1200), 150);
        assert_eq!(marker_side(1280, 1024), 128);
    }

    #[test]
    fn marker_side_never_exceeds_the_area() {
        assert_eq!(marker_side(40, 30), 4);
        assert_eq!(marker_side(3, 3), 3);
        assert_eq!(marker_side(1, 500), 1);
    }

    #[test]
    fn marker_starts_at_the_center() {
        assert_eq!(marker_origin(Duration::ZERO, 1600, 1200, 150), (725, 525));
    }

    #[test]
    fn marker_stays_inside_the_area_and_moves() {
        let (width, height, side) = (152, 112, 14);
        let mut seen = std::collections::HashSet::new();
        for tenth in 0..1200 {
            let (x, y) = marker_origin(Duration::from_millis(tenth * 100), width, height, side);
            assert!(
                x + side <= width && y + side <= height,
                "({x}, {y}) at {tenth}"
            );
            seen.insert((x, y));
        }
        assert!(
            seen.len() > 100,
            "the marker barely moved: {} positions",
            seen.len()
        );
    }

    #[test]
    fn cursor_distance_threshold() {
        let start = (100, 100);
        assert!(!cursor_moved_enough(start, (100, 100), 16));
        assert!(!cursor_moved_enough(start, (115, 100), 16));
        assert!(cursor_moved_enough(start, (116, 100), 16));
        assert!(cursor_moved_enough(start, (84, 100), 16));
        assert!(cursor_moved_enough(start, (100, 116), 16));
        // Diagonal: sqrt(12^2 + 12^2) is about 16.97.
        assert!(cursor_moved_enough(start, (112, 112), 16));
        assert!(!cursor_moved_enough(start, (111, 111), 16));
    }

    #[test]
    fn cursor_distance_handles_negative_and_large_coordinates() {
        // Secondary monitors to the left of or above the primary one have
        // negative coordinates.
        assert!(cursor_moved_enough((-1920, 0), (-1900, 0), 16));
        assert!(cursor_moved_enough((i32::MIN, 0), (i32::MAX, 0), 16));
    }

    const GRACE: Duration = Duration::from_millis(500);

    fn ms(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    #[test]
    fn cursor_movement_in_grace_rebases_the_reference() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(Some((0, 0)));
        policy.start_grace(t0, GRACE);
        // A big jump during the grace period is ignored and becomes the reference.
        assert_eq!(
            policy.cursor_moved(ms(t0, 100), (1000, 1000)),
            CursorDecision::IgnoredInGrace
        );
        assert_eq!(policy.reference(), Some((1000, 1000)));
        // Last instant of the grace period: still ignored.
        assert_eq!(
            policy.cursor_moved(ms(t0, 499), (1005, 1000)),
            CursorDecision::IgnoredInGrace
        );
        assert_eq!(policy.reference(), Some((1005, 1000)));
        // After the grace period the distance is measured from the last position.
        assert_eq!(
            policy.cursor_moved(ms(t0, 500), (1020, 1000)),
            CursorDecision::BelowThreshold
        );
        assert_eq!(
            policy.cursor_moved(ms(t0, 501), (1021, 1000)),
            CursorDecision::Exit { from: (1005, 1000) }
        );
    }

    #[test]
    fn grace_end_retakes_the_reference() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(Some((0, 0)));
        policy.start_grace(t0, GRACE);
        assert_eq!(policy.pending_grace_end(), Some(ms(t0, 500)));
        assert!(!policy.grace_end_due(ms(t0, 499)));
        assert!(policy.grace_end_due(ms(t0, 500)));

        // The cursor was moved without any event; the reference is re-taken
        // from the position at the end of the grace period.
        policy.finish_grace(Some((200, 200)));
        assert_eq!(policy.reference(), Some((200, 200)));
        assert_eq!(policy.pending_grace_end(), None);
        assert!(!policy.grace_end_due(ms(t0, 600)));

        assert_eq!(
            policy.cursor_moved(ms(t0, 600), (210, 200)),
            CursorDecision::BelowThreshold
        );
        assert_eq!(
            policy.cursor_moved(ms(t0, 601), (216, 200)),
            CursorDecision::Exit { from: (200, 200) }
        );
    }

    #[test]
    fn finish_grace_with_unknown_cursor_keeps_the_reference() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(Some((5, 5)));
        policy.start_grace(t0, GRACE);
        policy.finish_grace(None);
        assert_eq!(policy.reference(), Some((5, 5)));
        assert_eq!(policy.pending_grace_end(), None);
    }

    #[test]
    fn first_move_after_grace_does_not_rebase_the_reference() {
        // The event arrives before the end of the grace period was handled:
        // it is judged against the reference set during the grace period.
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(Some((0, 0)));
        policy.start_grace(t0, GRACE);
        assert_eq!(
            policy.cursor_moved(ms(t0, 10), (100, 100)),
            CursorDecision::IgnoredInGrace
        );
        assert!(policy.grace_end_due(ms(t0, 600)));
        assert_eq!(
            policy.cursor_moved(ms(t0, 600), (130, 100)),
            CursorDecision::Exit { from: (100, 100) }
        );
        // The end of the grace period no longer needs handling afterwards.
        assert!(!policy.grace_end_due(ms(t0, 601)));
    }

    #[test]
    fn unknown_reference_is_set_by_the_first_move() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(None);
        assert_eq!(
            policy.cursor_moved(t0, (50, 50)),
            CursorDecision::ReferenceSet
        );
        assert_eq!(policy.reference(), Some((50, 50)));
        assert_eq!(
            policy.cursor_moved(t0, (60, 50)),
            CursorDecision::BelowThreshold
        );
        assert_eq!(
            policy.cursor_moved(t0, (66, 50)),
            CursorDecision::Exit { from: (50, 50) }
        );
    }

    #[test]
    fn unknown_reference_during_grace_is_rebased_too() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(None);
        policy.start_grace(t0, GRACE);
        assert_eq!(
            policy.cursor_moved(ms(t0, 1), (10, 10)),
            CursorDecision::IgnoredInGrace
        );
        assert_eq!(policy.reference(), Some((10, 10)));
    }

    #[test]
    fn without_a_grace_period_movement_counts_at_once() {
        let t0 = Instant::now();
        let mut policy = ExitPolicy::new(Some((0, 0)));
        assert!(!policy.in_grace(t0));
        assert_eq!(policy.pending_grace_end(), None);
        assert_eq!(
            policy.cursor_moved(t0, (16, 0)),
            CursorDecision::Exit { from: (0, 0) }
        );
    }

    #[test]
    fn only_non_synthetic_key_presses_end_the_screensaver() {
        assert!(key_ends_screensaver(true, false));
        // Release (the Enter key that started a preview may be released late).
        assert!(!key_ends_screensaver(false, false));
        // Synthetic events winit generates when the focus moves between windows.
        assert!(!key_ends_screensaver(true, true));
        assert!(!key_ends_screensaver(false, true));
    }

    #[test]
    fn color_is_dark_and_periodic() {
        for ms in (0..40_000).step_by(37) {
            let color = dark_color(Duration::from_millis(ms));
            for channel in color {
                assert!((0.05 - 1e-9..=0.10 + 1e-9).contains(&channel), "{channel}");
            }
        }
        let a = dark_color(Duration::from_millis(1_234));
        let b = dark_color(Duration::from_millis(1_234 + 12_000));
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 1e-9);
        }
    }

    #[test]
    fn color_changes_slowly() {
        // Never more than 0.03 per second per channel (the real maximum is about
        // 0.014), i.e. no flashing.
        let step = Duration::from_millis(100);
        let mut previous = dark_color(Duration::ZERO);
        for i in 1..=400 {
            let current = dark_color(step * i);
            for (p, c) in previous.iter().zip(current.iter()) {
                assert!((p - c).abs() < 0.003, "{p} -> {c}");
            }
            previous = current;
        }
    }
}
