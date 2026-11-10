//! Decides how the event loop waits after one pass (ADR-0025).
//!
//! Pure so that the decision is tested on any host; `saver.rs` is Windows
//! only and converts [`Flow`] to winit's `ControlFlow`.

/// How the event loop waits before the next pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow<T> {
    /// Do not wait.
    Poll,
    /// Wait until the given time, or until an event arrives.
    WaitUntil(T),
    /// Wait for an event, with no deadline.
    Wait,
}

/// Chooses how to wait.
///
/// Why `Poll` when exiting: winit 0.30 on Windows waits with the control flow
/// set in `about_to_wait` before it checks whether to exit, so `Wait` would
/// hold the process until some message arrives.
pub fn next_flow<T: Ord + Copy>(
    exiting: bool,
    presented: bool,
    wake_times: impl IntoIterator<Item = T>,
) -> Flow<T> {
    if exiting || presented {
        return Flow::Poll;
    }
    wake_times
        .into_iter()
        .min()
        .map_or(Flow::Wait, Flow::WaitUntil)
}

/// The times to wake for after one pass: the fixed ones (the end of the
/// grace period, the next monitor check) and, while drawing, the retry and
/// paced-frame times of the windows. While drawing is paused (the display is
/// off, plan 0001 N-14), those are left out: they may be past, and waking
/// for them would spin.
pub fn wake_times<T>(
    fixed: impl IntoIterator<Item = T>,
    retries: impl IntoIterator<Item = T>,
    drawing: bool,
) -> Vec<T> {
    let retries = retries.into_iter().filter(|_| drawing);
    fixed.into_iter().chain(retries).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exiting_polls_whatever_else() {
        assert_eq!(next_flow(true, false, [0u32; 0]), Flow::Poll);
        assert_eq!(next_flow(true, true, [5u32]), Flow::Poll);
        assert_eq!(next_flow(true, false, [5u32, 3]), Flow::Poll);
    }

    #[test]
    fn presented_polls() {
        assert_eq!(next_flow(false, true, [5u32]), Flow::Poll);
        assert_eq!(next_flow(false, true, [0u32; 0]), Flow::Poll);
    }

    #[test]
    fn waits_until_the_earliest_time() {
        assert_eq!(next_flow(false, false, [7u32, 3, 9]), Flow::WaitUntil(3));
        assert_eq!(next_flow(false, false, [4u32]), Flow::WaitUntil(4));
    }

    #[test]
    fn waits_without_deadline_when_no_times() {
        assert_eq!(next_flow(false, false, [0u32; 0]), Flow::Wait);
    }

    #[test]
    fn retries_count_while_drawing() {
        assert_eq!(wake_times([10u32], [2, 4], true), [10, 2, 4]);
    }

    #[test]
    fn paused_drawing_waits_only_for_the_fixed_times() {
        assert_eq!(wake_times([10u32, 20], [2, 4], false), [10, 20]);
        // A retry time in the past (2, before "now" 5) must not make the
        // loop wake at once: it waits for the monitor check at 10.
        assert_eq!(
            next_flow(false, false, wake_times([10u32], [2], false)),
            Flow::WaitUntil(10)
        );
        assert_eq!(wake_times([0u32; 0], [2], false), [0u32; 0]);
    }
}
