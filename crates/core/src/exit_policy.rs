//! Decides when the screensaver or preview should exit (docs/design/architecture.md).
//!
//! The policy is a pure state machine: the caller translates platform events
//! into [`Event`]s and passes the current time in, so the rules can be tested
//! without a window system or a clock.

use std::collections::HashSet;
use std::time::Duration;

/// How the program was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Full-screen screensaver: any real user input ends it.
    Screensaver,
    /// Small preview inside the settings dialog: it ignores all input and only
    /// ends when its window or its parent goes away.
    Preview,
}

/// Identifies one of our windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub u64);

/// A platform event, reduced to what the exit decision needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A key was pressed.
    KeyPressed {
        /// winit's `is_synthetic`: the event was generated for a key that was
        /// already held while focus moved to one of our windows, not typed.
        /// Key repeat and injected input are not synthetic.
        synthetic: bool,
    },
    /// A key was released.
    KeyReleased {
        /// winit's `is_synthetic`, as for [`Event::KeyPressed`].
        synthetic: bool,
    },
    /// A mouse button was pressed.
    MouseButtonPressed,
    /// The mouse wheel was turned.
    MouseWheel,
    /// A touch or pen contact.
    Touch,
    /// The cursor moved, in screen coordinates (physical pixels).
    CursorMoved {
        /// Horizontal position.
        x: i32,
        /// Vertical position.
        y: i32,
    },
    /// One of our windows gained or lost keyboard focus.
    FocusChanged {
        /// The window whose focus changed.
        window: WindowId,
        /// Whether the window now has focus.
        focused: bool,
    },
    /// The end of one batch of events (winit's `about_to_wait`).
    EventsProcessed,
    /// The user or the system asked a window to close.
    CloseRequested,
    /// A window was destroyed.
    WindowDestroyed,
    /// Preview only: the parent window is no longer valid.
    ParentGone,
}

/// The outcome of handling one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Keep running.
    Continue,
    /// Shut down.
    Exit,
}

/// Tracks input and focus state and decides when to exit.
#[derive(Debug, Clone)]
pub struct ExitPolicy {
    mode: Mode,
    threshold_px: u32,
    grace: Duration,
    origin: Option<(i32, i32)>,
    focused: HashSet<WindowId>,
    had_focus: bool,
}

impl ExitPolicy {
    /// Default length of the grace period (see [`ExitPolicy::handle`] for when it starts).
    pub const DEFAULT_GRACE: Duration = Duration::from_millis(500);
    /// Default cursor travel, in physical pixels, that ends the screensaver.
    pub const DEFAULT_THRESHOLD_PX: u32 = 16;

    /// Creates a policy for `mode`.
    ///
    /// The screensaver exits when the cursor moves at least `threshold_px`
    /// pixels away from its reference position; a threshold of 0 is raised to 1.
    /// Cursor movement and focus changes during the first `grace` only move the
    /// reference position or are recorded (the window system may report a
    /// stray move or focus change when the windows appear).
    pub fn new(mode: Mode, threshold_px: u32, grace: Duration) -> Self {
        Self {
            mode,
            threshold_px: threshold_px.max(1),
            grace,
            origin: None,
            focused: HashSet::new(),
            had_focus: false,
        }
    }

    /// Handles one event and returns whether to exit.
    ///
    /// `now` is the monotonic time since the grace period started. The caller
    /// starts it once every window has been created and GPU initialization
    /// has finished or failed: events queued while those ran are delivered
    /// afterwards and must still fall inside the grace period. The caller also
    /// wakes its event loop when the grace period ends, so that
    /// [`Event::EventsProcessed`] checks a focus loss from the grace period
    /// even if no further event arrives.
    pub fn handle(&mut self, event: Event, now: Duration) -> Decision {
        match (self.mode, event) {
            (_, Event::CloseRequested | Event::WindowDestroyed) => Decision::Exit,
            (Mode::Preview, Event::ParentGone) => Decision::Exit,
            // The preview shows inside the settings dialog and ignores all input.
            (Mode::Preview, _) => Decision::Continue,
            (Mode::Screensaver, event) => self.handle_screensaver(event, now),
        }
    }

    fn handle_screensaver(&mut self, event: Event, now: Duration) -> Decision {
        match event {
            Event::KeyPressed { synthetic: false }
            | Event::MouseButtonPressed
            | Event::MouseWheel
            | Event::Touch => Decision::Exit,
            Event::CursorMoved { x, y } => self.handle_cursor(x, y, now),
            Event::FocusChanged { window, focused } => {
                if focused {
                    self.focused.insert(window);
                    self.had_focus = true;
                } else {
                    self.focused.remove(&window);
                }
                // Focus moving from one of our windows to another arrives as "lost"
                // then "gained" within one batch, so only the end of a batch decides.
                Decision::Continue
            }
            Event::EventsProcessed => {
                if now >= self.grace && self.had_focus && self.focused.is_empty() {
                    Decision::Exit
                } else {
                    Decision::Continue
                }
            }
            // A synthetic key press is not user input, and a key release can arrive
            // right after the "Preview" button was pressed with Enter.
            Event::KeyPressed { synthetic: true } | Event::KeyReleased { .. } => Decision::Continue,
            // Already handled for both modes.
            Event::CloseRequested | Event::WindowDestroyed => Decision::Exit,
            // Only the preview has a parent window.
            Event::ParentGone => Decision::Continue,
        }
    }

    fn handle_cursor(&mut self, x: i32, y: i32, now: Duration) -> Decision {
        let origin = match self.origin {
            // Only the first position and positions inside the grace period move the
            // reference; afterwards it stays put so slow drift adds up.
            Some(origin) if now >= self.grace => origin,
            _ => {
                self.origin = Some((x, y));
                return Decision::Continue;
            }
        };
        // i32 differences fit in i64, and their squares fit in u128.
        let dx = u128::from((i64::from(x) - i64::from(origin.0)).unsigned_abs());
        let dy = u128::from((i64::from(y) - i64::from(origin.1)).unsigned_abs());
        let threshold = u128::from(self.threshold_px);
        if dx * dx + dy * dy >= threshold * threshold {
            Decision::Exit
        } else {
            Decision::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Decision::{Continue, Exit};

    const GRACE: Duration = ExitPolicy::DEFAULT_GRACE;
    /// A time after the grace period.
    const LATER: Duration = Duration::from_secs(1);
    /// A time inside the grace period.
    const EARLY: Duration = Duration::from_millis(100);

    const A: WindowId = WindowId(1);
    const B: WindowId = WindowId(2);

    fn screensaver() -> ExitPolicy {
        ExitPolicy::new(
            Mode::Screensaver,
            ExitPolicy::DEFAULT_THRESHOLD_PX,
            ExitPolicy::DEFAULT_GRACE,
        )
    }

    fn preview() -> ExitPolicy {
        ExitPolicy::new(
            Mode::Preview,
            ExitPolicy::DEFAULT_THRESHOLD_PX,
            ExitPolicy::DEFAULT_GRACE,
        )
    }

    fn moved(x: i32, y: i32) -> Event {
        Event::CursorMoved { x, y }
    }

    fn focus(window: WindowId, focused: bool) -> Event {
        Event::FocusChanged { window, focused }
    }

    /// A screensaver whose cursor reference is (100, 100), set after the grace period.
    fn screensaver_with_origin() -> ExitPolicy {
        let mut p = screensaver();
        assert_eq!(p.handle(moved(100, 100), LATER), Continue);
        p
    }

    #[test]
    fn defaults_are_500ms_and_16px() {
        assert_eq!(ExitPolicy::DEFAULT_GRACE, Duration::from_millis(500));
        assert_eq!(ExitPolicy::DEFAULT_THRESHOLD_PX, 16);
    }

    // Both modes.

    #[test]
    fn close_requested_exits_in_both_modes() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(screensaver().handle(Event::CloseRequested, now), Exit);
            assert_eq!(preview().handle(Event::CloseRequested, now), Exit);
        }
    }

    #[test]
    fn window_destroyed_exits_in_both_modes() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(screensaver().handle(Event::WindowDestroyed, now), Exit);
            assert_eq!(preview().handle(Event::WindowDestroyed, now), Exit);
        }
    }

    // Preview.

    #[test]
    fn preview_exits_when_parent_is_gone() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(preview().handle(Event::ParentGone, now), Exit);
        }
    }

    #[test]
    fn preview_ignores_all_input() {
        let events = [
            Event::KeyPressed { synthetic: false },
            Event::KeyPressed { synthetic: true },
            Event::KeyReleased { synthetic: false },
            Event::KeyReleased { synthetic: true },
            Event::MouseButtonPressed,
            Event::MouseWheel,
            Event::Touch,
            moved(0, 0),
            moved(5000, 5000),
            moved(i32::MIN, i32::MAX),
            focus(A, true),
            focus(A, false),
            Event::EventsProcessed,
        ];
        for now in [Duration::ZERO, EARLY, LATER] {
            let mut p = preview();
            for event in events {
                assert_eq!(p.handle(event, now), Continue, "{event:?} at {now:?}");
            }
        }
    }

    // Screensaver: keys, buttons, wheel, touch.

    #[test]
    fn real_key_press_exits_even_within_grace() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(
                screensaver().handle(Event::KeyPressed { synthetic: false }, now),
                Exit
            );
        }
    }

    #[test]
    fn synthetic_key_press_is_ignored() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(
                screensaver().handle(Event::KeyPressed { synthetic: true }, now),
                Continue
            );
        }
    }

    #[test]
    fn key_release_is_ignored() {
        // A release can arrive right after the "Preview" button was pressed with Enter.
        for synthetic in [false, true] {
            for now in [Duration::ZERO, EARLY, LATER] {
                assert_eq!(
                    screensaver().handle(Event::KeyReleased { synthetic }, now),
                    Continue
                );
            }
        }
    }

    #[test]
    fn mouse_button_wheel_and_touch_exit_even_within_grace() {
        for event in [Event::MouseButtonPressed, Event::MouseWheel, Event::Touch] {
            for now in [Duration::ZERO, EARLY, LATER] {
                assert_eq!(screensaver().handle(event, now), Exit, "{event:?}");
            }
        }
    }

    #[test]
    fn parent_gone_does_not_apply_to_the_screensaver() {
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(screensaver().handle(Event::ParentGone, now), Continue);
        }
    }

    // Screensaver: cursor movement.

    #[test]
    fn first_cursor_position_becomes_the_origin() {
        let mut p = screensaver();
        // Far from any earlier position, but there is none yet.
        assert_eq!(p.handle(moved(5000, -5000), LATER), Continue);
        assert_eq!(p.handle(moved(5000, -5000), LATER), Continue);
        assert_eq!(p.handle(moved(5016, -5000), LATER), Exit);
    }

    #[test]
    fn threshold_boundaries_with_16px() {
        let cases = [
            ((12, 12), Exit),     // 16.97 px
            ((11, 11), Continue), // 15.56 px
            ((16, 0), Exit),
            ((15, 0), Continue),
            ((0, 16), Exit),
            ((0, 15), Continue),
            ((-16, 0), Exit),
            ((-15, 0), Continue),
            ((-12, -12), Exit),
            ((-11, -11), Continue),
            ((0, 0), Continue),
        ];
        for ((dx, dy), want) in cases {
            let mut p = screensaver_with_origin();
            assert_eq!(
                p.handle(moved(100 + dx, 100 + dy), LATER),
                want,
                "({dx}, {dy})"
            );
        }
    }

    #[test]
    fn slow_drift_adds_up_because_the_origin_stays_put() {
        let mut p = screensaver_with_origin();
        for x in [105, 110, 115] {
            assert_eq!(p.handle(moved(x, 100), LATER), Continue, "x = {x}");
        }
        assert_eq!(p.handle(moved(116, 100), LATER), Exit);
    }

    #[test]
    fn moving_back_toward_the_origin_does_not_move_it() {
        let mut p = screensaver_with_origin();
        assert_eq!(p.handle(moved(110, 100), LATER), Continue);
        assert_eq!(p.handle(moved(100, 100), LATER), Continue);
        assert_eq!(p.handle(moved(116, 100), LATER), Exit);
    }

    #[test]
    fn cursor_moves_within_grace_only_rebase_the_origin() {
        let mut p = screensaver();
        assert_eq!(p.handle(moved(0, 0), Duration::ZERO), Continue);
        // A big jump inside the grace period does not exit; it becomes the origin.
        assert_eq!(p.handle(moved(1000, 1000), EARLY), Continue);
        assert_eq!(
            p.handle(moved(2000, 2000), GRACE - Duration::from_nanos(1)),
            Continue
        );
        // After the grace period the distance is measured from the last rebase.
        assert_eq!(p.handle(moved(2010, 2000), GRACE), Continue);
        assert_eq!(p.handle(moved(2016, 2000), GRACE), Exit);
    }

    #[test]
    fn cursor_move_exactly_at_grace_end_counts() {
        let mut p = screensaver();
        assert_eq!(p.handle(moved(0, 0), Duration::ZERO), Continue);
        assert_eq!(p.handle(moved(100, 0), GRACE), Exit);
    }

    #[test]
    fn extreme_coordinates_do_not_overflow() {
        // Any two different values here are at least 1000 apart, far above the threshold.
        let corners = [i32::MIN, -1000, 0, 1000, i32::MAX];
        for ox in corners {
            for oy in corners {
                for x in corners {
                    for y in corners {
                        let mut p = screensaver();
                        assert_eq!(p.handle(moved(ox, oy), LATER), Continue);
                        let moved_far = (ox, oy) != (x, y);
                        let want = if moved_far { Exit } else { Continue };
                        assert_eq!(
                            p.handle(moved(x, y), LATER),
                            want,
                            "({ox}, {oy}) -> ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn extreme_threshold_and_distance_compare_exactly() {
        let mut p = ExitPolicy::new(Mode::Screensaver, u32::MAX, GRACE);
        assert_eq!(p.handle(moved(i32::MIN, 0), LATER), Continue);
        // The distance is exactly u32::MAX here.
        assert_eq!(p.handle(moved(i32::MAX - 1, 0), LATER), Continue);
        assert_eq!(p.handle(moved(i32::MAX, 0), LATER), Exit);
    }

    #[test]
    fn threshold_zero_is_raised_to_one() {
        let mut p = ExitPolicy::new(Mode::Screensaver, 0, GRACE);
        assert_eq!(p.handle(moved(50, 50), LATER), Continue);
        assert_eq!(p.handle(moved(50, 50), LATER), Continue);
        assert_eq!(p.handle(moved(51, 50), LATER), Exit);
    }

    // Screensaver: focus.

    #[test]
    fn focus_events_never_decide_by_themselves() {
        let mut p = screensaver();
        assert_eq!(p.handle(focus(A, true), LATER), Continue);
        assert_eq!(p.handle(focus(A, false), LATER), Continue);
    }

    #[test]
    fn focus_moving_between_our_windows_in_one_batch_continues() {
        let mut p = screensaver();
        assert_eq!(p.handle(focus(A, true), LATER), Continue);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Continue);
        // One batch: A loses focus, then B gains it.
        assert_eq!(p.handle(focus(A, false), LATER), Continue);
        assert_eq!(p.handle(focus(B, true), LATER), Continue);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Continue);
    }

    #[test]
    fn losing_focus_on_one_of_several_focused_windows_continues() {
        let mut p = screensaver();
        p.handle(focus(A, true), LATER);
        p.handle(focus(B, true), LATER);
        p.handle(focus(A, false), LATER);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Continue);
        p.handle(focus(B, false), LATER);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Exit);
    }

    #[test]
    fn all_windows_unfocused_exits_at_events_processed_after_grace() {
        let mut p = screensaver();
        p.handle(focus(A, true), LATER);
        p.handle(focus(A, false), LATER);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Exit);
    }

    #[test]
    fn focus_lost_within_grace_exits_at_first_events_processed_after_grace() {
        let mut p = screensaver();
        p.handle(focus(A, true), Duration::ZERO);
        p.handle(focus(A, false), EARLY);
        assert_eq!(p.handle(Event::EventsProcessed, EARLY), Continue);
        assert_eq!(
            p.handle(Event::EventsProcessed, GRACE - Duration::from_nanos(1)),
            Continue
        );
        assert_eq!(p.handle(Event::EventsProcessed, GRACE), Exit);
    }

    #[test]
    fn focus_regained_within_grace_does_not_exit() {
        let mut p = screensaver();
        p.handle(focus(A, true), Duration::ZERO);
        p.handle(focus(A, false), EARLY);
        p.handle(focus(A, true), EARLY);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Continue);
    }

    #[test]
    fn never_focused_never_exits_for_focus() {
        let mut p = screensaver();
        for now in [Duration::ZERO, EARLY, LATER, Duration::from_secs(3600)] {
            assert_eq!(p.handle(Event::EventsProcessed, now), Continue);
        }
        // Losing focus on a window that never had it does not count as having had focus.
        p.handle(focus(A, false), LATER);
        assert_eq!(p.handle(Event::EventsProcessed, LATER), Continue);
    }

    #[test]
    fn events_processed_with_focus_continues() {
        let mut p = screensaver();
        p.handle(focus(A, true), Duration::ZERO);
        for now in [Duration::ZERO, EARLY, LATER] {
            assert_eq!(p.handle(Event::EventsProcessed, now), Continue);
        }
    }
}
