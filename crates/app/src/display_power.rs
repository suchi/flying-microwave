//! Whether the display is on (plan 0001, N-14; plan 0005, F-09 and D-8).
//!
//! Windows reports the display state of the session as a power setting
//! (`GUID_SESSION_DISPLAY_STATUS`). The value is a `MONITOR_DISPLAY_STATE`:
//! 0 off, 1 on, 2 dimmed ("Power Setting GUIDs", Microsoft Learn). Drawing
//! stops only while the display is off; a dimmed display is still seen. The
//! registration is in [`crate::win`]; this part has no Windows dependency,
//! so it is tested on every host.

use std::sync::atomic::{AtomicBool, Ordering};

/// The display state as last reported. True until a report says otherwise,
/// so that drawing goes on as before when the registration fails.
static DISPLAY_ON: AtomicBool = AtomicBool::new(true);

/// Whether the display is on (or dimmed), as last reported.
pub fn is_on() -> bool {
    DISPLAY_ON.load(Ordering::SeqCst)
}

/// Records a report: `data` is the `Data` of a `POWERBROADCAST_SETTING`
/// for `GUID_SESSION_DISPLAY_STATUS`. A report that is too short is ignored.
pub fn report(data: &[u8]) {
    if let Some(on) = display_on(data) {
        DISPLAY_ON.store(on, Ordering::SeqCst);
    }
}

/// Reads a `MONITOR_DISPLAY_STATE` (a little-endian DWORD): false only for
/// off (0). None when there are fewer than four bytes.
fn display_on(data: &[u8]) -> Option<bool> {
    let bytes: [u8; 4] = data.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_off_stops_drawing() {
        assert_eq!(display_on(&0u32.to_le_bytes()), Some(false));
        assert_eq!(display_on(&1u32.to_le_bytes()), Some(true));
        // Dimmed: the screen is still seen.
        assert_eq!(display_on(&2u32.to_le_bytes()), Some(true));
    }

    #[test]
    fn a_short_report_is_ignored() {
        assert_eq!(display_on(&[]), None);
        assert_eq!(display_on(&[0, 0, 0]), None);
    }

    #[test]
    fn extra_bytes_are_not_read() {
        assert_eq!(display_on(&[0, 0, 0, 0, 1]), Some(false));
    }
}
