//! Compares two lists of monitors (plan 0004, F-07).
//!
//! `/s` polls the monitor list while it runs and compares it with the
//! monitors its windows cover, to find the monitors that were added, moved or
//! resized, or removed. The comparison has no platform dependency, so it is
//! tested on every host; the monitor handle is a type parameter. The helpers
//! after `compare` decide which windows follow from the result.

/// The area of a monitor on the desktop, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// What happened to one monitor between the old and the new list. `old` and
/// `new` are positions in those lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// The same monitor with the same area. Its handle may be another one.
    Kept {
        /// Position in the old list.
        old: usize,
        /// Position in the new list.
        new: usize,
    },
    /// The same monitor with another position or size.
    Moved {
        /// Position in the old list.
        old: usize,
        /// Position in the new list.
        new: usize,
    },
    /// A monitor that is only in the new list.
    Added {
        /// Position in the new list.
        new: usize,
    },
    /// A monitor that is only in the old list.
    Removed {
        /// Position in the old list.
        old: usize,
    },
}

/// Compares the monitors in `old` with those in `new`.
///
/// A monitor is the same when its handle is equal. A monitor whose handle
/// is not in the other list is still the same when an unmatched monitor of
/// the other list has exactly its area (the handle was replaced). The result
/// lists the old monitors in order, then the added ones in order.
///
/// Returns `None` when `new` is empty: the caller then keeps everything as
/// it is, since losing every monitor at once is more likely a failed query
/// than a real change, and closing every window would uncover the screen.
pub fn compare<K: PartialEq>(old: &[(K, Area)], new: &[(K, Area)]) -> Option<Vec<Change>> {
    if new.is_empty() {
        return None;
    }
    let mut matched: Vec<Option<usize>> = vec![None; old.len()];
    let mut used = vec![false; new.len()];
    // First by handle, then the rest by area.
    for (i, (key, _)) in old.iter().enumerate() {
        if let Some(j) = (0..new.len()).find(|&j| !used[j] && new[j].0 == *key) {
            matched[i] = Some(j);
            used[j] = true;
        }
    }
    for (i, (_, area)) in old.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        if let Some(j) = (0..new.len()).find(|&j| !used[j] && new[j].1 == *area) {
            matched[i] = Some(j);
            used[j] = true;
        }
    }
    let mut changes: Vec<Change> = matched
        .iter()
        .enumerate()
        .map(|(i, m)| match *m {
            Some(j) if new[j].1 == old[i].1 => Change::Kept { old: i, new: j },
            Some(j) => Change::Moved { old: i, new: j },
            None => Change::Removed { old: i },
        })
        .collect();
    changes.extend(
        used.iter()
            .enumerate()
            .filter(|(_, used)| !**used)
            .map(|(j, _)| Change::Added { new: j }),
    );
    Some(changes)
}

/// The position of the window that covers each known monitor, or `None` when
/// no window does (it could not be made). `window_monitors` holds the monitor
/// of each window, `None` for a window not tied to a monitor.
pub fn windows_of_known<K: PartialEq>(
    known: &[(K, Area)],
    window_monitors: &[Option<&K>],
) -> Vec<Option<usize>> {
    known
        .iter()
        .map(|(key, _)| window_monitors.iter().position(|m| *m == Some(key)))
        .collect()
}

/// Which windows animate. Without `only_primary`, all of them. With it, those
/// on the primary monitor (`on_primary`, one flag per window); when none is
/// there, the first window, so that the animation is shown somewhere.
pub fn animated(only_primary: bool, on_primary: &[bool]) -> Vec<bool> {
    if !only_primary {
        return vec![true; on_primary.len()];
    }
    if on_primary.iter().any(|&p| p) {
        return on_primary.to_vec();
    }
    (0..on_primary.len()).map(|i| i == 0).collect()
}

/// The positions of the windows to close, out of `positions` (the windows
/// whose monitors are gone): sorted, without duplicates and positions beyond
/// `window_count`. Empty when closing them would leave no window.
pub fn closable(positions: &[usize], window_count: usize) -> Vec<usize> {
    let mut positions: Vec<usize> = positions
        .iter()
        .copied()
        .filter(|&p| p < window_count)
        .collect();
    positions.sort_unstable();
    positions.dedup();
    if positions.len() >= window_count {
        positions.clear();
    }
    positions
}

/// The first window that is not closing: the one to bring to the foreground
/// before the others close.
pub fn foreground_after_close(closing: &[usize], window_count: usize) -> Option<usize> {
    (0..window_count).find(|p| !closing.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Change::{Added, Kept, Moved, Removed};

    const LEFT: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const RIGHT: Area = Area {
        x: 1920,
        y: 0,
        width: 2560,
        height: 1440,
    };

    #[test]
    fn the_same_monitors_are_kept() {
        let list = [(1, LEFT), (2, RIGHT)];
        assert_eq!(
            compare(&list, &list),
            Some(vec![Kept { old: 0, new: 0 }, Kept { old: 1, new: 1 }])
        );
    }

    #[test]
    fn the_same_monitors_in_another_order_are_kept() {
        let old = [(1, LEFT), (2, RIGHT)];
        let new = [(2, RIGHT), (1, LEFT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Kept { old: 0, new: 1 }, Kept { old: 1, new: 0 }])
        );
    }

    #[test]
    fn a_new_monitor_is_added() {
        let old = [(1, LEFT)];
        let new = [(1, LEFT), (2, RIGHT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Kept { old: 0, new: 0 }, Added { new: 1 }])
        );
    }

    #[test]
    fn every_monitor_is_added_when_none_was_known() {
        let old: [(u32, Area); 0] = [];
        let new = [(1, LEFT), (2, RIGHT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Added { new: 0 }, Added { new: 1 }])
        );
    }

    #[test]
    fn a_missing_monitor_is_removed() {
        let old = [(1, LEFT), (2, RIGHT)];
        let new = [(1, LEFT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Kept { old: 0, new: 0 }, Removed { old: 1 }])
        );
    }

    #[test]
    fn a_monitor_with_another_position_or_size_is_moved() {
        let old = [(1, LEFT), (2, RIGHT)];
        let resized = Area {
            width: 1280,
            height: 720,
            ..LEFT
        };
        let moved = Area { x: -2560, ..RIGHT };
        let new = [(1, resized), (2, moved)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Moved { old: 0, new: 0 }, Moved { old: 1, new: 1 }])
        );
    }

    #[test]
    fn an_empty_list_changes_nothing() {
        let old = [(1, LEFT), (2, RIGHT)];
        let new: [(u32, Area); 0] = [];
        assert_eq!(compare(&old, &new), None);
    }

    #[test]
    fn a_new_handle_with_the_same_area_is_the_same_monitor() {
        let old = [(1, LEFT), (2, RIGHT)];
        let new = [(1, LEFT), (3, RIGHT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Kept { old: 0, new: 0 }, Kept { old: 1, new: 1 }])
        );
    }

    #[test]
    fn a_new_handle_with_another_area_is_another_monitor() {
        let old = [(1, LEFT), (2, RIGHT)];
        let other = Area { y: 1080, ..RIGHT };
        let new = [(1, LEFT), (3, other)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![
                Kept { old: 0, new: 0 },
                Removed { old: 1 },
                Added { new: 1 }
            ])
        );
    }

    #[test]
    fn a_handle_match_wins_over_an_area_match() {
        // Monitor 1 moved to where monitor 2 was, and monitor 2 is gone.
        let old = [(1, LEFT), (2, RIGHT)];
        let new = [(1, RIGHT)];
        assert_eq!(
            compare(&old, &new),
            Some(vec![Moved { old: 0, new: 0 }, Removed { old: 1 }])
        );
    }

    #[test]
    fn each_known_monitor_finds_its_window() {
        let known = [(1, LEFT), (2, RIGHT)];
        let windows = [Some(&2), Some(&1)];
        assert_eq!(windows_of_known(&known, &windows), vec![Some(1), Some(0)]);
    }

    #[test]
    fn a_known_monitor_without_a_window_has_none() {
        let known = [(1, LEFT), (2, RIGHT)];
        let windows = [Some(&1), None];
        assert_eq!(windows_of_known(&known, &windows), vec![Some(0), None]);
    }

    #[test]
    fn the_first_window_of_a_monitor_is_taken() {
        let known = [(1, LEFT)];
        let windows = [Some(&1), Some(&1)];
        assert_eq!(windows_of_known(&known, &windows), vec![Some(0)]);
    }

    #[test]
    fn every_window_animates_unless_only_the_primary_does() {
        assert_eq!(animated(false, &[false, true, false]), vec![true; 3]);
    }

    #[test]
    fn only_the_windows_on_the_primary_monitor_animate() {
        assert_eq!(
            animated(true, &[false, true, false]),
            vec![false, true, false]
        );
    }

    #[test]
    fn the_first_window_animates_when_none_is_on_the_primary_monitor() {
        assert_eq!(
            animated(true, &[false, false, false]),
            vec![true, false, false]
        );
    }

    #[test]
    fn no_windows_animate_nothing() {
        assert_eq!(animated(true, &[]), Vec::<bool>::new());
    }

    #[test]
    fn closable_sorts_and_removes_duplicates_and_positions_out_of_range() {
        assert_eq!(closable(&[3, 1, 1, 7], 4), vec![1, 3]);
    }

    #[test]
    fn nothing_is_closable_when_no_window_would_remain() {
        assert_eq!(closable(&[1, 0, 2], 3), Vec::<usize>::new());
        assert_eq!(closable(&[0, 0, 5], 1), Vec::<usize>::new());
    }

    #[test]
    fn nothing_is_closable_without_positions() {
        assert_eq!(closable(&[], 3), Vec::<usize>::new());
        assert_eq!(closable(&[9], 3), Vec::<usize>::new());
    }

    #[test]
    fn the_foreground_goes_to_the_first_window_that_stays() {
        assert_eq!(foreground_after_close(&[0, 1], 4), Some(2));
        assert_eq!(foreground_after_close(&[1], 3), Some(0));
        assert_eq!(foreground_after_close(&[0, 1, 2], 3), None);
    }
}
