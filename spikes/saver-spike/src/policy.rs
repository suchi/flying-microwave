//! Small pure helpers shared by the modes. No platform dependencies, so they
//! are unit tested on any host.

use std::f64::consts::TAU;
use std::time::Duration;

/// Grace period after the windows appear during which input is logged but
/// does not end the screensaver (the key release of the Enter key that started
/// a preview, a cursor jitter caused by window creation, and so on).
pub const GRACE_PERIOD: Duration = Duration::from_millis(500);

/// Physical pixels the cursor must travel from its position at startup
/// before the screensaver exits.
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

#[cfg(test)]
mod tests {
    use super::*;

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
