//! Helpers for the frame diagnostics in the screensaver log.

/// How many events of each kind are logged per window.
const LOGGED: u64 = 3;

/// Whether the event with the 1-based number `count` is one of the first few
/// that are logged, so that a per-frame event cannot flood the log.
pub fn first_few(count: u64) -> bool {
    (1..=LOGGED).contains(&count)
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
}
