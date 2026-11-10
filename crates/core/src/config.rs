//! The settings model, its defaults and the validation rules
//! (docs/design/settings.md; plan 0004, F-01).
//!
//! Reading never fails: whatever the file holds, the result is a usable
//! [`Config`], and each problem only falls back for the item it concerns.
//! Every row of the validation table in the settings specification is a test
//! below. File access is not done here (`storage` in the app crate does it).

use std::ops::RangeInclusive;

use serde::Serialize;
use toml::{Table, Value};

use crate::sim::SimParams;

/// The `schema_version` this version writes.
pub const SCHEMA_VERSION: i64 = 1;
/// Files larger than this are not read at all (SEC-4).
pub const MAX_FILE_BYTES: usize = 64 * 1024;

/// `flight.count`.
pub const COUNT_RANGE: RangeInclusive<u32> = 1..=50;
/// `flight.speed`.
pub const SPEED_RANGE: RangeInclusive<f64> = 0.25..=3.0;
/// `flight.scale`.
pub const SCALE_RANGE: RangeInclusive<f64> = 0.5..=2.0;

/// Every setting of v0.2.0.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Config {
    /// The `[flight]` table.
    pub flight: Flight,
    /// The `[display]` table.
    pub display: Display,
    /// The `[input]` table.
    pub input: Input,
}

/// The `[flight]` table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    /// `flight.count`: number of microwaves, in [`COUNT_RANGE`].
    pub count: u32,
    /// `flight.speed`: speed multiplier, finite and in [`SPEED_RANGE`].
    pub speed: f64,
    /// `flight.scale`: size multiplier, finite and in [`SCALE_RANGE`].
    pub scale: f64,
}

impl Default for Flight {
    fn default() -> Self {
        Self {
            count: 12,
            speed: 1.0,
            scale: 1.0,
        }
    }
}

/// The `[display]` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Display {
    /// `display.max_fps`.
    pub max_fps: MaxFps,
    /// `display.monitors`.
    pub monitors: Monitors,
}

/// The `[input]` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Input {
    /// `input.exit_sensitivity`.
    pub exit_sensitivity: ExitSensitivity,
}

/// `display.max_fps`: the frame rate limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaxFps {
    /// `0`: as fast as the monitor refreshes.
    MatchMonitor,
    /// `30`.
    Fps30,
    /// `60`.
    #[default]
    Fps60,
    /// `120`.
    Fps120,
}

impl MaxFps {
    /// Every choice, in the order the settings window lists them.
    pub const ALL: [MaxFps; 4] = [
        MaxFps::MatchMonitor,
        MaxFps::Fps30,
        MaxFps::Fps60,
        MaxFps::Fps120,
    ];

    /// The value in the file (0 means "match the monitor").
    pub fn value(self) -> u32 {
        match self {
            MaxFps::MatchMonitor => 0,
            MaxFps::Fps30 => 30,
            MaxFps::Fps60 => 60,
            MaxFps::Fps120 => 120,
        }
    }

    fn from_value(value: i64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| i64::from(m.value()) == value)
    }
}

/// `display.monitors`: where the animation is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Monitors {
    /// `"all"`.
    #[default]
    All,
    /// `"primary"`: the other monitors are only covered in black.
    Primary,
}

impl Monitors {
    /// The value in the file.
    pub fn name(self) -> &'static str {
        match self {
            Monitors::All => "all",
            Monitors::Primary => "primary",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [Monitors::All, Monitors::Primary]
            .into_iter()
            .find(|m| m.name() == name)
    }
}

/// `input.exit_sensitivity`: how far the cursor must move to end `/s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExitSensitivity {
    /// `"low"`.
    Low,
    /// `"medium"`.
    #[default]
    Medium,
    /// `"high"`.
    High,
}

impl ExitSensitivity {
    /// Every choice, from the least to the most sensitive.
    pub const ALL: [ExitSensitivity; 3] = [
        ExitSensitivity::Low,
        ExitSensitivity::Medium,
        ExitSensitivity::High,
    ];

    /// The value in the file.
    pub fn name(self) -> &'static str {
        match self {
            ExitSensitivity::Low => "low",
            ExitSensitivity::Medium => "medium",
            ExitSensitivity::High => "high",
        }
    }

    /// The cursor travel that ends the screensaver, in physical pixels
    /// (values of the settings specification, checked on hardware in plan
    /// 0004, D-16).
    pub fn threshold_px(self) -> u32 {
        match self {
            ExitSensitivity::Low => 48,
            ExitSensitivity::Medium => 16,
            ExitSensitivity::High => 4,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

impl Config {
    /// The simulation parameters for these settings.
    pub fn sim_params(&self) -> SimParams {
        SimParams {
            count: self.flight.count,
            speed: self.flight.speed as f32,
            scale: self.flight.scale as f32,
        }
    }
}

/// Where the settings came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The file does not exist: every item is its default.
    Missing,
    /// The file could not be read (the reason is from the system).
    Unreadable(String),
    /// The file is larger than [`MAX_FILE_BYTES`] and was not read.
    TooLarge,
    /// The file is not valid TOML (or not UTF-8).
    NotToml,
    /// The file was read; [`Loaded::notes`] lists the items that fell back.
    File,
}

/// The settings that were read, and what happened while reading them.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The settings to use.
    pub config: Config,
    /// Where they came from.
    pub source: Source,
    /// The file's `schema_version` when it is newer than [`SCHEMA_VERSION`]:
    /// writing back must be confirmed by the user first.
    pub newer_schema: Option<i64>,
    /// One line per item that was clamped or fell back, for the diagnostic
    /// log. Lines name the key, never the value (which could be anything).
    pub notes: Vec<String>,
}

impl Loaded {
    fn defaults(source: Source) -> Self {
        Self {
            config: Config::default(),
            source,
            newer_schema: None,
            notes: Vec::new(),
        }
    }

    /// The result when the file does not exist.
    pub fn missing() -> Self {
        Self::defaults(Source::Missing)
    }

    /// The result when the file exists but cannot be read.
    pub fn unreadable(reason: impl Into<String>) -> Self {
        Self::defaults(Source::Unreadable(reason.into()))
    }

    /// Validates the contents of a settings file.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() > MAX_FILE_BYTES {
            return Self::defaults(Source::TooLarge);
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return Self::defaults(Source::NotToml);
        };
        let Ok(table) = text.parse::<Table>() else {
            return Self::defaults(Source::NotToml);
        };
        let mut reader = Reader { notes: Vec::new() };
        let newer_schema = reader.schema_version(table.get("schema_version"));
        let flight = reader.table(&table, "flight");
        let display = reader.table(&table, "display");
        let input = reader.table(&table, "input");
        let defaults = Config::default();
        let config = Config {
            flight: Flight {
                count: reader.integer(flight, "flight.count", COUNT_RANGE, defaults.flight.count),
                speed: reader.float(flight, "flight.speed", SPEED_RANGE, defaults.flight.speed),
                scale: reader.float(flight, "flight.scale", SCALE_RANGE, defaults.flight.scale),
            },
            display: Display {
                max_fps: reader.choice(
                    display,
                    "display.max_fps",
                    |v| v.as_integer().and_then(MaxFps::from_value),
                    defaults.display.max_fps,
                ),
                monitors: reader.choice(
                    display,
                    "display.monitors",
                    |v| v.as_str().and_then(Monitors::from_name),
                    defaults.display.monitors,
                ),
            },
            input: Input {
                exit_sensitivity: reader.choice(
                    input,
                    "input.exit_sensitivity",
                    |v| v.as_str().and_then(ExitSensitivity::from_name),
                    defaults.input.exit_sensitivity,
                ),
            },
        };
        Self {
            config,
            source: Source::File,
            newer_schema,
            notes: reader.notes,
        }
    }
}

/// Reads the items of a parsed file and records every fallback.
struct Reader {
    notes: Vec<String>,
}

/// The last part of a dotted key (`flight.count` to `count`).
fn leaf(key: &str) -> &str {
    key.rsplit('.').next().unwrap_or(key)
}

impl Reader {
    fn note(&mut self, key: &str, what: &str) {
        self.notes.push(format!("{key}: {what}"));
    }

    /// Returns the version when it is newer than [`SCHEMA_VERSION`].
    fn schema_version(&mut self, value: Option<&Value>) -> Option<i64> {
        match value {
            None => None,
            Some(Value::Integer(v)) if *v > SCHEMA_VERSION => Some(*v),
            Some(Value::Integer(v)) if *v >= 1 => None,
            // Why: decided in ADR-0024 (E-3): a
            // non-integer or non-positive version is read as the current one.
            Some(_) => {
                self.note("schema_version", "not a positive integer; read as 1");
                None
            }
        }
    }

    /// The table `name`, or `None` when it is missing or not a table.
    fn table<'a>(&mut self, root: &'a Table, name: &str) -> Option<&'a Table> {
        match root.get(name) {
            None => None,
            Some(Value::Table(table)) => Some(table),
            Some(_) => {
                self.note(name, "not a table; using the defaults");
                None
            }
        }
    }

    fn integer(
        &mut self,
        table: Option<&Table>,
        key: &str,
        range: RangeInclusive<u32>,
        default: u32,
    ) -> u32 {
        match table.and_then(|t| t.get(leaf(key))) {
            None => default,
            Some(Value::Integer(v)) => {
                let clamped = (*v).clamp(i64::from(*range.start()), i64::from(*range.end()));
                if clamped != *v {
                    self.note(key, "out of range; clamped");
                }
                // Always succeeds: the range is within u32.
                u32::try_from(clamped).unwrap_or(default)
            }
            Some(_) => {
                self.note(key, "not an integer; using the default");
                default
            }
        }
    }

    fn float(
        &mut self,
        table: Option<&Table>,
        key: &str,
        range: RangeInclusive<f64>,
        default: f64,
    ) -> f64 {
        let value = match table.and_then(|t| t.get(leaf(key))) {
            None => return default,
            Some(Value::Float(v)) => *v,
            // An integer is accepted where a float is expected.
            Some(Value::Integer(v)) => *v as f64,
            Some(_) => {
                self.note(key, "not a number; using the default");
                return default;
            }
        };
        // Checked before clamping: a NaN would stay NaN.
        if !value.is_finite() {
            self.note(key, "not finite; using the default");
            return default;
        }
        let clamped = value.clamp(*range.start(), *range.end());
        if clamped != value {
            self.note(key, "out of range; clamped");
        }
        clamped
    }

    fn choice<T>(
        &mut self,
        table: Option<&Table>,
        key: &str,
        read: impl FnOnce(&Value) -> Option<T>,
        default: T,
    ) -> T {
        let Some(value) = table.and_then(|t| t.get(leaf(key))) else {
            return default;
        };
        read(value).unwrap_or_else(|| {
            self.note(key, "not one of the choices; using the default");
            default
        })
    }
}

/// The layout of a written file (the field order is the order in the file).
#[derive(Serialize)]
struct FileV1 {
    schema_version: i64,
    flight: FlightV1,
    display: DisplayV1,
    input: InputV1,
}

#[derive(Serialize)]
struct FlightV1 {
    count: u32,
    speed: f64,
    scale: f64,
}

#[derive(Serialize)]
struct DisplayV1 {
    max_fps: u32,
    monitors: &'static str,
}

#[derive(Serialize)]
struct InputV1 {
    exit_sensitivity: &'static str,
}

/// The text of a settings file for `config`.
///
/// Why: decided in ADR-0024 (E-3). Only the known items
/// are written, with [`SCHEMA_VERSION`]; unknown keys and comments of the
/// file being replaced are lost.
pub fn to_toml(config: &Config) -> String {
    let file = FileV1 {
        schema_version: SCHEMA_VERSION,
        flight: FlightV1 {
            count: config.flight.count,
            speed: config.flight.speed,
            scale: config.flight.scale,
        },
        display: DisplayV1 {
            max_fps: config.display.max_fps.value(),
            monitors: config.display.monitors.name(),
        },
        input: InputV1 {
            exit_sensitivity: config.input.exit_sensitivity.name(),
        },
    };
    // Why the fallback: serializing plain numbers and strings does not fail,
    // and an empty file would still read back as the defaults.
    toml::to_string(&file).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str) -> Loaded {
        Loaded::from_bytes(text.as_bytes())
    }

    fn config(text: &str) -> Config {
        load(text).config
    }

    fn flight(text: &str) -> Flight {
        config(text).flight
    }

    // The defaults are the ones of the settings specification.

    #[test]
    fn defaults_match_the_specification() {
        let c = Config::default();
        assert_eq!(c.flight.count, 12);
        assert_eq!(c.flight.speed, 1.0);
        assert_eq!(c.flight.scale, 1.0);
        assert_eq!(c.display.max_fps.value(), 60);
        assert_eq!(c.display.monitors, Monitors::All);
        assert_eq!(c.input.exit_sensitivity, ExitSensitivity::Medium);
    }

    // Row: the file does not exist.

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let loaded = Loaded::missing();
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.source, Source::Missing);
        assert_eq!(loaded.newer_schema, None);
    }

    #[test]
    fn an_unreadable_file_gives_the_defaults() {
        let loaded = Loaded::unreadable("access denied");
        assert_eq!(loaded.config, Config::default());
        assert_eq!(
            loaded.source,
            Source::Unreadable("access denied".to_owned())
        );
    }

    // Row: the file is too large.

    #[test]
    fn a_file_over_the_limit_is_not_read() {
        let mut text = b"[flight]\ncount = 3\n".to_vec();
        text.resize(MAX_FILE_BYTES + 1, b' ');
        let loaded = Loaded::from_bytes(&text);
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.source, Source::TooLarge);
    }

    #[test]
    fn a_file_at_the_limit_is_read() {
        let mut text = b"[flight]\ncount = 3\n".to_vec();
        text.resize(MAX_FILE_BYTES, b' ');
        let loaded = Loaded::from_bytes(&text);
        assert_eq!(loaded.source, Source::File);
        assert_eq!(loaded.config.flight.count, 3);
    }

    // Row: the file is not TOML.

    #[test]
    fn a_file_that_is_not_toml_gives_the_defaults() {
        for text in [
            "[flight\ncount = 3",
            "count = = 3",
            "\u{0}\u{1}",
            "[flight]\ncount = 3\ncount = 4",
        ] {
            let loaded = load(text);
            assert_eq!(loaded.config, Config::default(), "text {text:?}");
            assert_eq!(loaded.source, Source::NotToml, "text {text:?}");
        }
    }

    #[test]
    fn a_file_that_is_not_utf8_gives_the_defaults() {
        let loaded = Loaded::from_bytes(b"[flight]\ncount = 3 # \xff\n");
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.source, Source::NotToml);
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        let loaded = load("");
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.source, Source::File);
        assert!(loaded.notes.is_empty());
    }

    #[test]
    fn valid_values_are_used() {
        let c = config(
            "schema_version = 1\n\
             [flight]\ncount = 30\nspeed = 2.5\nscale = 0.75\n\
             [display]\nmax_fps = 0\nmonitors = \"primary\"\n\
             [input]\nexit_sensitivity = \"high\"\n",
        );
        assert_eq!(
            c,
            Config {
                flight: Flight {
                    count: 30,
                    speed: 2.5,
                    scale: 0.75
                },
                display: Display {
                    max_fps: MaxFps::MatchMonitor,
                    monitors: Monitors::Primary
                },
                input: Input {
                    exit_sensitivity: ExitSensitivity::High
                },
            }
        );
    }

    #[test]
    fn every_choice_is_read() {
        for (value, fps) in [
            (0, MaxFps::MatchMonitor),
            (30, MaxFps::Fps30),
            (60, MaxFps::Fps60),
            (120, MaxFps::Fps120),
        ] {
            assert_eq!(
                config(&format!("[display]\nmax_fps = {value}"))
                    .display
                    .max_fps,
                fps
            );
        }
        for s in ExitSensitivity::ALL {
            let text = format!("[input]\nexit_sensitivity = \"{}\"", s.name());
            assert_eq!(config(&text).input.exit_sensitivity, s);
        }
    }

    // Row: out-of-range numbers are clamped.

    #[test]
    fn out_of_range_numbers_are_clamped() {
        assert_eq!(flight("[flight]\ncount = 0").count, 1);
        assert_eq!(flight("[flight]\ncount = -5").count, 1);
        assert_eq!(flight("[flight]\ncount = 51").count, 50);
        assert_eq!(flight("[flight]\ncount = 9223372036854775807").count, 50);
        assert_eq!(flight("[flight]\nspeed = 0.1").speed, 0.25);
        assert_eq!(flight("[flight]\nspeed = 3.5").speed, 3.0);
        assert_eq!(flight("[flight]\nspeed = -1e300").speed, 0.25);
        assert_eq!(flight("[flight]\nscale = 0.0").scale, 0.5);
        assert_eq!(flight("[flight]\nscale = 1e300").scale, 2.0);
    }

    #[test]
    fn a_clamped_item_is_noted_by_its_key() {
        let loaded = load("[flight]\ncount = 99");
        assert_eq!(loaded.notes.len(), 1);
        assert!(
            loaded.notes[0].starts_with("flight.count"),
            "{:?}",
            loaded.notes
        );
    }

    // Row: non-finite floats fall back for that item only.

    #[test]
    fn non_finite_floats_fall_back_for_that_item_only() {
        for value in ["nan", "+nan", "-nan", "inf", "+inf", "-inf"] {
            let f = flight(&format!(
                "[flight]\ncount = 7\nspeed = {value}\nscale = {value}"
            ));
            assert_eq!(f.speed, 1.0, "value {value}");
            assert_eq!(f.scale, 1.0, "value {value}");
            assert_eq!(f.count, 7, "value {value}");
        }
    }

    // Row: an integer in a float item is accepted.

    #[test]
    fn an_integer_in_a_float_item_is_accepted() {
        assert_eq!(flight("[flight]\nspeed = 2").speed, 2.0);
        assert_eq!(flight("[flight]\nscale = 1").scale, 1.0);
        assert_eq!(flight("[flight]\nscale = 5").scale, 2.0);
        assert_eq!(flight("[flight]\nspeed = 0").speed, 0.25);
    }

    // Row: a float in an integer item falls back for that item.

    #[test]
    fn a_float_in_an_integer_item_falls_back() {
        let f = flight("[flight]\ncount = 3.0\nspeed = 2.0");
        assert_eq!(f.count, 12);
        assert_eq!(f.speed, 2.0);
        assert_eq!(
            config("[display]\nmax_fps = 30.0").display.max_fps,
            MaxFps::Fps60
        );
    }

    // Row: values that are not a choice, and other type mismatches.

    #[test]
    fn values_that_are_not_a_choice_fall_back() {
        assert_eq!(
            config("[display]\nmax_fps = 45").display.max_fps,
            MaxFps::Fps60
        );
        assert_eq!(
            config("[display]\nmax_fps = -60").display.max_fps,
            MaxFps::Fps60
        );
        assert_eq!(
            config("[display]\nmonitors = \"left\"").display.monitors,
            Monitors::All
        );
        // Choices are exact: case and spaces matter.
        assert_eq!(
            config("[display]\nmonitors = \"Primary\"").display.monitors,
            Monitors::All
        );
        assert_eq!(
            config("[input]\nexit_sensitivity = \"high \"")
                .input
                .exit_sensitivity,
            ExitSensitivity::Medium
        );
    }

    #[test]
    fn other_type_mismatches_fall_back() {
        assert_eq!(flight("[flight]\ncount = \"12\"").count, 12);
        assert_eq!(flight("[flight]\ncount = \"3\"").count, 12);
        assert_eq!(flight("[flight]\nspeed = true").speed, 1.0);
        assert_eq!(flight("[flight]\nscale = [1.5]").scale, 1.0);
        assert_eq!(
            config("[display]\nmax_fps = \"30\"").display.max_fps,
            MaxFps::Fps60
        );
        assert_eq!(
            config("[display]\nmonitors = 1").display.monitors,
            Monitors::All
        );
        assert_eq!(
            config("[input]\nexit_sensitivity = { level = \"high\" }")
                .input
                .exit_sensitivity,
            ExitSensitivity::Medium
        );
        assert_eq!(flight("[flight]\ncount = 1979-05-27").count, 12);
    }

    #[test]
    fn a_table_of_the_wrong_type_falls_back_for_its_items() {
        let c = config("flight = 3\ndisplay = \"all\"\n[input]\nexit_sensitivity = \"low\"");
        assert_eq!(c.flight, Flight::default());
        assert_eq!(c.display, Display::default());
        assert_eq!(c.input.exit_sensitivity, ExitSensitivity::Low);
    }

    #[test]
    fn one_bad_item_keeps_the_others() {
        let c = config(
            "[flight]\ncount = \"many\"\nspeed = 1.5\nscale = nan\n\
             [display]\nmax_fps = 120\nmonitors = 2\n",
        );
        assert_eq!(c.flight.count, 12);
        assert_eq!(c.flight.speed, 1.5);
        assert_eq!(c.flight.scale, 1.0);
        assert_eq!(c.display.max_fps, MaxFps::Fps120);
        assert_eq!(c.display.monitors, Monitors::All);
    }

    // Row: unknown keys are ignored.

    #[test]
    fn unknown_keys_are_ignored() {
        let loaded = load(
            "theme = \"night\"\n\
             [flight]\ncount = 5\nfood_count = 4\n\
             [clock]\nenabled = true\n",
        );
        assert_eq!(loaded.source, Source::File);
        assert_eq!(loaded.config.flight.count, 5);
        assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
    }

    // Row: a newer schema_version.

    #[test]
    fn a_newer_schema_uses_the_readable_items_and_asks_before_writing() {
        let loaded = load("schema_version = 2\n[flight]\ncount = 9\nwings = 2\n");
        assert_eq!(loaded.config.flight.count, 9);
        assert_eq!(loaded.newer_schema, Some(2));
    }

    #[test]
    fn the_current_or_no_schema_version_needs_no_confirmation() {
        assert_eq!(load("schema_version = 1").newer_schema, None);
        assert_eq!(load("[flight]\ncount = 3").newer_schema, None);
    }

    #[test]
    fn an_invalid_schema_version_is_read_as_the_current_one() {
        // Why: decided in ADR-0024 (E-3).
        for text in [
            "schema_version = 0",
            "schema_version = -1",
            "schema_version = \"2\"",
            "schema_version = 2.0",
        ] {
            let loaded = load(&format!("{text}\n[flight]\ncount = 3"));
            assert_eq!(loaded.newer_schema, None, "{text}");
            assert_eq!(loaded.config.flight.count, 3, "{text}");
        }
    }

    // Writing.

    #[test]
    fn the_defaults_are_written_as_in_the_specification() {
        assert_eq!(
            to_toml(&Config::default()),
            "schema_version = 1\n\
             \n\
             [flight]\n\
             count = 12\n\
             speed = 1.0\n\
             scale = 1.0\n\
             \n\
             [display]\n\
             max_fps = 60\n\
             monitors = \"all\"\n\
             \n\
             [input]\n\
             exit_sensitivity = \"medium\"\n"
        );
    }

    #[test]
    fn written_settings_read_back_unchanged() {
        let c = Config {
            flight: Flight {
                count: 50,
                speed: 0.25,
                scale: 1.35,
            },
            display: Display {
                max_fps: MaxFps::MatchMonitor,
                monitors: Monitors::Primary,
            },
            input: Input {
                exit_sensitivity: ExitSensitivity::Low,
            },
        };
        let loaded = load(&to_toml(&c));
        assert_eq!(loaded.config, c);
        assert_eq!(loaded.source, Source::File);
        assert!(loaded.notes.is_empty(), "{:?}", loaded.notes);
    }

    // Uses of the settings.

    #[test]
    fn sensitivity_maps_to_the_specified_thresholds() {
        assert_eq!(ExitSensitivity::Low.threshold_px(), 48);
        assert_eq!(ExitSensitivity::Medium.threshold_px(), 16);
        assert_eq!(ExitSensitivity::High.threshold_px(), 4);
    }

    #[test]
    fn flight_settings_become_the_simulation_parameters() {
        let c = config("[flight]\ncount = 20\nspeed = 2.5\nscale = 0.5");
        assert_eq!(
            c.sim_params(),
            SimParams {
                count: 20,
                speed: 2.5,
                scale: 0.5
            }
        );
        assert_eq!(Config::default().sim_params(), SimParams::default());
    }
}
