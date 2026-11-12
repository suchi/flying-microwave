//! The LED clock on the microwaves (plan 0005, F-06 and D-16): which
//! segments of the seven-segment display are lit for a local time.
//!
//! The renderer draws every segment unlit and lights the ones whose bit is
//! set in the mask, so the display needs no new texture when the time
//! changes. The local time is passed in, so the display is tested with any
//! time of day.

/// `clock.format`: how the hours are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockFormat {
    /// `"24h"`: 00:00 to 23:59, with a leading zero.
    #[default]
    H24,
    /// `"12h"`: 1:00 to 12:59, without a leading zero; afternoon is shown by
    /// the PM dot.
    H12,
}

impl ClockFormat {
    /// The value in the settings file.
    pub fn name(self) -> &'static str {
        match self {
            ClockFormat::H24 => "24h",
            ClockFormat::H12 => "12h",
        }
    }

    /// The format of a value in the settings file; values are exact.
    pub fn from_name(name: &str) -> Option<Self> {
        [ClockFormat::H24, ClockFormat::H12]
            .into_iter()
            .find(|f| f.name() == name)
    }
}

/// A local time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
    /// 0 to 59 (a leap second, 60, is accepted).
    pub second: u8,
}

/// Number of digits on the display.
pub const DIGITS: u32 = 4;
/// Segments of one digit: a (top), b (upper right), c (lower right),
/// d (bottom), e (lower left), f (upper left), g (middle).
pub const SEGMENTS: u32 = 7;
/// Bit of the colon (both dots) in the mask.
pub const COLON_BIT: u32 = DIGITS * SEGMENTS;
/// Bit of the PM dot in the mask.
pub const PM_BIT: u32 = COLON_BIT + 1;
// The mask is one u32 (and one WGSL u32).
const _: () = assert!(PM_BIT < 32);

/// Bit of segment `segment` (0 to 6, a to g) of digit `digit` (0 to 3, from
/// the left).
pub const fn segment_bit(digit: u32, segment: u32) -> u32 {
    digit * SEGMENTS + segment
}

/// Lit segments of each digit, bit 0 = a .. bit 6 = g.
const DIGIT_SEGMENTS: [u8; 10] = [
    0b011_1111, // 0: a b c d e f
    0b000_0110, // 1: b c
    0b101_1011, // 2: a b d e g
    0b100_1111, // 3: a b c d g
    0b110_0110, // 4: b c f g
    0b110_1101, // 5: a c d f g
    0b111_1101, // 6: a c d e f g
    0b000_0111, // 7: a b c
    0b111_1111, // 8: all
    0b110_1111, // 9: a b c d f g
];

/// The lit segments for `time`, or nothing lit when the clock is off or the
/// time is unknown (all segments are then drawn unlit).
///
/// The colon is lit during even seconds, so it blinks once a second (below
/// the three flashes a second of guideline G-3).
pub fn led_mask(time: Option<LocalTime>, format: ClockFormat, enabled: bool) -> u32 {
    let Some(t) = time.filter(|_| enabled) else {
        return 0;
    };
    if t.hour > 23 || t.minute > 59 || t.second > 60 {
        return 0;
    }
    let (hour, pm) = match format {
        ClockFormat::H24 => (t.hour, false),
        ClockFormat::H12 => (
            if t.hour % 12 == 0 { 12 } else { t.hour % 12 },
            t.hour >= 12,
        ),
    };
    let tens = match (format, hour / 10) {
        // 12 hours: no leading zero.
        (ClockFormat::H12, 0) => None,
        (_, d) => Some(d),
    };
    let shown = [
        tens,
        Some(hour % 10),
        Some(t.minute / 10),
        Some(t.minute % 10),
    ];
    let mut mask = 0;
    for (digit, value) in (0..).zip(shown) {
        if let Some(v) = value {
            mask |= u32::from(DIGIT_SEGMENTS[usize::from(v)]) << segment_bit(digit, 0);
        }
    }
    if t.second % 2 == 0 {
        mask |= 1 << COLON_BIT;
    }
    if pm {
        mask |= 1 << PM_BIT;
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u8, minute: u8, second: u8) -> Option<LocalTime> {
        Some(LocalTime {
            hour,
            minute,
            second,
        })
    }

    /// The digit shown at position `digit`, or `None` when it is blank.
    fn shown(mask: u32, digit: u32) -> Option<u8> {
        let bits = (mask >> segment_bit(digit, 0)) & 0x7f;
        if bits == 0 {
            return None;
        }
        let found = DIGIT_SEGMENTS.iter().position(|&s| u32::from(s) == bits);
        Some(found.expect("a digit pattern") as u8)
    }

    fn digits(mask: u32) -> [Option<u8>; 4] {
        [0, 1, 2, 3].map(|d| shown(mask, d))
    }

    #[test]
    fn digit_patterns_are_distinct_and_seven_bits() {
        for (i, a) in DIGIT_SEGMENTS.iter().enumerate() {
            assert!(*a < 0x80);
            for b in &DIGIT_SEGMENTS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn twenty_four_hours_have_leading_zeros() {
        let m = led_mask(at(9, 5, 0), ClockFormat::H24, true);
        assert_eq!(digits(m), [Some(0), Some(9), Some(0), Some(5)]);
        assert_eq!(m & (1 << PM_BIT), 0);
        let m = led_mask(at(23, 59, 0), ClockFormat::H24, true);
        assert_eq!(digits(m), [Some(2), Some(3), Some(5), Some(9)]);
        assert_eq!(m & (1 << PM_BIT), 0, "no PM dot in 24 hours");
        let m = led_mask(at(0, 0, 0), ClockFormat::H24, true);
        assert_eq!(digits(m), [Some(0), Some(0), Some(0), Some(0)]);
    }

    #[test]
    fn twelve_hours_drop_the_leading_zero_and_show_pm() {
        let cases = [
            (0, [Some(1), Some(2)], false),
            (9, [None, Some(9)], false),
            (11, [Some(1), Some(1)], false),
            (12, [Some(1), Some(2)], true),
            (13, [None, Some(1)], true),
            (23, [Some(1), Some(1)], true),
        ];
        for (hour, hours, pm) in cases {
            let m = led_mask(at(hour, 7, 0), ClockFormat::H12, true);
            assert_eq!(digits(m)[..2], hours, "hour {hour}");
            assert_eq!(digits(m)[2..], [Some(0), Some(7)], "hour {hour}");
            assert_eq!(m & (1 << PM_BIT) != 0, pm, "hour {hour}");
        }
    }

    #[test]
    fn colon_blinks_once_a_second() {
        let lit = |s| led_mask(at(10, 30, s), ClockFormat::H24, true) & (1 << COLON_BIT) != 0;
        assert!(lit(0));
        assert!(!lit(1));
        assert!(lit(58));
        assert!(!lit(59));
        // The digits do not change within the minute.
        let a = led_mask(at(10, 30, 0), ClockFormat::H24, true) & !(1 << COLON_BIT);
        let b = led_mask(at(10, 30, 59), ClockFormat::H24, true) & !(1 << COLON_BIT);
        assert_eq!(a, b);
    }

    #[test]
    fn nothing_is_lit_when_off_or_unknown() {
        assert_eq!(led_mask(at(12, 34, 0), ClockFormat::H24, false), 0);
        assert_eq!(led_mask(None, ClockFormat::H24, true), 0);
        assert_eq!(led_mask(None, ClockFormat::H12, false), 0);
    }

    #[test]
    fn out_of_range_times_light_nothing() {
        // A time the system should never give is not drawn as nonsense.
        assert_eq!(led_mask(at(24, 0, 0), ClockFormat::H24, true), 0);
        assert_eq!(led_mask(at(12, 60, 0), ClockFormat::H24, true), 0);
        // A leap second keeps the display.
        let m = led_mask(at(23, 59, 60), ClockFormat::H24, true);
        assert_eq!(digits(m), [Some(2), Some(3), Some(5), Some(9)]);
    }

    #[test]
    fn format_names_are_exact() {
        assert_eq!(ClockFormat::from_name("24h"), Some(ClockFormat::H24));
        assert_eq!(ClockFormat::from_name("12h"), Some(ClockFormat::H12));
        assert_eq!(ClockFormat::from_name("12H"), None);
        assert_eq!(ClockFormat::default(), ClockFormat::H24);
    }
}
