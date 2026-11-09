//! Reading and writing the settings file (plan 0004, F-01 and F-02).
//!
//! The file is `%APPDATA%\FlyingMicrowave\config.toml` (plan 0004, D-1).
//! Validation is [`flying_microwave_core::config`]'s job; this module only
//! finds the file, limits how much of it is read, and replaces it safely.
//! Everything except the lookup of the Roaming AppData folder is plain
//! `std`, so it is tested on any host.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flying_microwave_core::config::{Loaded, MAX_FILE_BYTES, Source};

/// Environment variable that moves the settings file to another folder.
///
/// Why: decided in ADR-0024 (E-1). Tests (the smoke
/// test, local runs) use it so that they never touch the user's own file.
/// An empty value counts as not set. Any other value must be an absolute
/// path (on Windows, one that starts with a drive letter); when it is not,
/// finding the file fails instead of falling back to the user's own file.
pub const DIR_ENV: &str = "FLYING_MICROWAVE_CONFIG_DIR";
/// Folder under Roaming AppData.
const APP_DIR: &str = "FlyingMicrowave";
const FILE_NAME: &str = "config.toml";

/// Where the settings file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Full path of `config.toml`.
    pub path: PathBuf,
    /// Whether [`DIR_ENV`] chose the folder.
    pub from_env: bool,
}

/// Whether `dir` is an absolute path that [`DIR_ENV`] may name.
///
/// On Windows only a drive path (`C:\...` or `\\?\C:\...`) counts: a UNC path would make the
/// screensaver use the network (SEC-5), and a device path (`\\.\pipe\...`)
/// could block a read (review L-4).
// Why: decided in ADR-0024 (E-1).
fn is_local_absolute(dir: &Path) -> bool {
    if !dir.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(
            dir.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Finds the settings file from the value of [`DIR_ENV`] and the Roaming
/// AppData folder (looked up only when needed).
pub fn resolve(
    env_dir: Option<&OsStr>,
    roaming: impl FnOnce() -> Option<PathBuf>,
) -> Result<Location, String> {
    if let Some(value) = env_dir.filter(|value| !value.is_empty()) {
        let dir = Path::new(value);
        // Why: a rejected value fails instead of falling back, so that a
        // test or local run never reads or replaces the user's own file
        // (review L-B).
        if !is_local_absolute(dir) {
            return Err(format!(
                "{DIR_ENV} is not an absolute path that starts with a drive letter"
            ));
        }
        return Ok(Location {
            path: dir.join(FILE_NAME),
            from_env: true,
        });
    }
    let roaming = roaming().ok_or("the Roaming AppData folder could not be found")?;
    Ok(Location {
        path: roaming.join(APP_DIR).join(FILE_NAME),
        from_env: false,
    })
}

/// The settings file of this user, or why it cannot be found.
pub fn location() -> Result<Location, String> {
    resolve(std::env::var_os(DIR_ENV).as_deref(), roaming_app_data)
}

#[cfg(windows)]
fn roaming_app_data() -> Option<PathBuf> {
    crate::win::roaming_app_data()
}

#[cfg(not(windows))]
fn roaming_app_data() -> Option<PathBuf> {
    // Where Linux and macOS keep the file is decided with those ports
    // (plan 0004, D-1).
    None
}

/// Reads and validates the settings file. Never fails: problems fall back
/// to the defaults as the validation rules say.
pub fn read(path: &Path) -> Loaded {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Loaded::missing(),
        Err(e) => return Loaded::unreadable(e.to_string()),
    };
    // One byte more than the limit is enough to tell that it is too large,
    // without reading a huge file (SEC-4).
    let mut bytes = Vec::new();
    match file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes) {
        Ok(_) => Loaded::from_bytes(&bytes),
        Err(e) => Loaded::unreadable(e.to_string()),
    }
}

/// Replaces the settings file with `text`: the text goes to a temporary
/// file in the same folder first, which then replaces the file, so that
/// `/s` never reads a half-written file (plan 0004, F-02).
pub fn write(path: &Path, text: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no folder"))?;
    std::fs::create_dir_all(dir)?;
    // The process id keeps two settings windows from sharing one temporary file.
    let temporary = dir.join(format!("{FILE_NAME}.{}.tmp", std::process::id()));
    let result = write_new(&temporary, text).and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Writes `text` to a new file and waits until it is on the disk, so that
/// the replacement never exposes an incomplete file after a power loss.
fn write_new(path: &Path, text: &str) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// The settings read at startup, with what the diagnostic log needs.
pub struct Startup {
    /// The validated settings.
    pub loaded: Loaded,
    /// Where the file is, or why that is unknown.
    pub location: Result<Location, String>,
    /// How long finding and reading took.
    pub elapsed: Duration,
}

impl Startup {
    /// Finds and reads the settings file.
    pub fn load() -> Self {
        let start = Instant::now();
        let location = location();
        let loaded = match &location {
            Ok(location) => read(&location.path),
            // Why: decided in ADR-0024 (E-6): without a
            // location, the defaults are used.
            Err(reason) => Loaded::unreadable(format!("no location: {reason}")),
        };
        Self {
            loaded,
            location,
            elapsed: start.elapsed(),
        }
    }

    /// One line for the diagnostic log. It names neither the path (which
    /// contains the user name) nor any value from the file.
    pub fn summary(&self) -> String {
        let source = match &self.loaded.source {
            Source::Missing => "file not found; using the defaults".to_owned(),
            Source::Unreadable(reason) => format!("unreadable ({reason}); using the defaults"),
            Source::TooLarge => "file too large; using the defaults".to_owned(),
            Source::NotToml => "not valid TOML; using the defaults".to_owned(),
            Source::File => format!("read, {} items fell back", self.loaded.notes.len()),
        };
        let place = match &self.location {
            Ok(location) if location.from_env => format!("folder from {DIR_ENV}"),
            Ok(_) => "standard location".to_owned(),
            Err(_) => "no location".to_owned(),
        };
        let newer = self
            .loaded
            .newer_schema
            .map_or(String::new(), |v| format!(", newer schema_version {v}"));
        format!(
            "config: {source} ({place}{newer}) in {:.1} ms",
            self.elapsed.as_secs_f64() * 1000.0
        )
    }
}

/// How long after the overwrite warning appears a second OK is not taken
/// as the confirmation: longer than the Windows double-click time (500 ms by
/// default), so that a double click on OK does not save unread (review L-A).
pub const CONFIRM_DELAY: Duration = Duration::from_millis(700);

/// Whether an OK that comes `since_warning` after the warning confirms it.
pub fn second_ok_confirms(since_warning: Duration) -> bool {
    since_warning >= CONFIRM_DELAY
}

/// The warning to show before OK replaces the settings file, if any.
///
/// The form shows the defaults when the file could not be read, so a save
/// would silently replace a file the user may want to keep (one with a typo,
/// or one on a share that is unreachable for a moment). A newer file loses
/// the items this version does not know (plan 0004, F-08).
// Why: decided in ADR-0024 (E-3): warn, then save on
// the second OK.
pub fn overwrite_warning(source: &Source, newer_schema: Option<i64>) -> Option<String> {
    let reason = match source {
        Source::Missing | Source::File => None,
        Source::Unreadable(reason) => Some(format!("could not be read ({reason})")),
        Source::TooLarge => Some("is too large to read".to_owned()),
        Source::NotToml => Some("is not a valid settings file".to_owned()),
    };
    let what = match (reason, newer_schema) {
        (Some(reason), _) => format!(
            "The settings file {reason}, so the defaults are shown. Saving replaces the file \
             with these values."
        ),
        (None, Some(version)) => format!(
            "This settings file is from a newer version (schema_version {version}). Saving \
             rewrites it in the format of this version, and settings this version does not \
             know are lost."
        ),
        (None, None) => return None,
    };
    Some(format!(
        "{what} Click Replace to save, or Cancel to keep the file."
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn saving_over_a_file_that_was_read_or_missing_needs_no_warning() {
        assert_eq!(overwrite_warning(&Source::File, None), None);
        assert_eq!(overwrite_warning(&Source::Missing, None), None);
    }

    #[test]
    fn saving_over_a_file_that_could_not_be_read_is_warned_about() {
        // Review M-1: OK must not silently replace a file the user may want
        // to keep, such as one with a typo or one on an unreachable share.
        for source in [
            Source::NotToml,
            Source::TooLarge,
            Source::Unreadable("access denied".to_owned()),
        ] {
            let warning = overwrite_warning(&source, None).expect("a warning");
            assert!(warning.contains("Click Replace to save"), "{warning}");
            assert!(warning.contains("Cancel"), "{warning}");
        }
        let warning =
            overwrite_warning(&Source::Unreadable("access denied".to_owned()), None).unwrap();
        assert!(warning.contains("access denied"), "{warning}");
    }

    #[test]
    fn a_double_click_does_not_confirm_the_warning() {
        // Review L-A: the second click of a double click comes well within
        // the Windows double-click time (500 ms by default).
        assert!(!second_ok_confirms(Duration::from_millis(0)));
        assert!(!second_ok_confirms(Duration::from_millis(500)));
        assert!(second_ok_confirms(CONFIRM_DELAY));
        assert!(second_ok_confirms(Duration::from_secs(5)));
    }

    #[test]
    fn saving_over_a_newer_file_is_warned_about() {
        let warning = overwrite_warning(&Source::File, Some(3)).expect("a warning");
        assert!(warning.contains("schema_version 3"), "{warning}");
        assert!(warning.contains("Click Replace to save"), "{warning}");
    }

    use super::*;

    use flying_microwave_core::config::Config;

    /// A fresh, empty folder for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flying-microwave-storage-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test folder");
        dir
    }

    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn roaming() -> Option<PathBuf> {
        Some(absolute("roaming"))
    }

    // Location (plan 0004, D-1 and E-1).

    #[test]
    fn the_file_is_under_roaming_app_data() {
        let location = resolve(None, roaming).expect("a location");
        assert_eq!(
            location.path,
            absolute("roaming")
                .join("FlyingMicrowave")
                .join("config.toml")
        );
        assert!(!location.from_env);
    }

    #[test]
    fn an_absolute_folder_in_the_environment_is_used_as_it_is() {
        let dir = absolute("smoke-config");
        let location =
            resolve(Some(dir.as_os_str()), || panic!("not looked up")).expect("a location");
        assert_eq!(location.path, dir.join("config.toml"));
        assert!(location.from_env);
    }

    #[test]
    fn a_relative_folder_in_the_environment_is_rejected() {
        // Review L-B: rejected instead of ignored, so that the user's own
        // file is never used in its place.
        for value in ["config", "./config", "..", "C:config"] {
            assert!(
                resolve(Some(OsStr::new(value)), roaming).is_err(),
                "value {value:?}"
            );
        }
        let location = resolve(Some(OsStr::new("")), roaming).expect("a location");
        assert!(location.path.starts_with(absolute("roaming")));
    }

    #[test]
    fn without_roaming_app_data_there_is_no_location() {
        assert!(resolve(None, || None).is_err());
        assert!(resolve(Some(OsStr::new("relative")), || None).is_err());
    }

    #[test]
    fn a_rejected_folder_does_not_fall_back_to_the_real_file() {
        // Review L-B: a test or local run that names a folder must never
        // read or replace the user's own file instead.
        let roaming = || Some(PathBuf::from("/home/user/AppData/Roaming"));
        let error = resolve(Some(OsStr::new("relative")), roaming).expect_err("rejected");
        assert!(error.contains(DIR_ENV), "{error}");
    }

    #[test]
    fn an_empty_folder_value_counts_as_not_set() {
        let roaming = || Some(PathBuf::from("/home/user/AppData/Roaming"));
        let location = resolve(Some(OsStr::new("")), roaming).expect("a location");
        assert!(!location.from_env);
    }

    #[cfg(windows)]
    #[test]
    fn only_a_drive_path_may_name_the_folder() {
        // Review L-4: a UNC or device path is ignored (SEC-5).
        for value in [
            r"\\server\share\fm",
            r"\\.\pipe\fm",
            r"\\?\UNC\server\share",
        ] {
            assert!(
                resolve(Some(OsStr::new(value)), || None).is_err(),
                "{value}"
            );
        }
        let location = resolve(Some(OsStr::new(r"C:\fm")), || None).expect("a location");
        assert!(location.from_env);
        // The form std::fs::canonicalize returns is a local disk too.
        let location = resolve(Some(OsStr::new(r"\\?\C:\fm")), || None).expect("a location");
        assert!(location.from_env);
    }

    // Reading.

    #[test]
    fn a_missing_file_reads_as_missing() {
        let dir = scratch("missing");
        assert_eq!(read(&dir.join("config.toml")).source, Source::Missing);
    }

    #[test]
    fn a_valid_file_is_read() {
        let dir = scratch("valid");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[flight]\ncount = 7\n").unwrap();
        let loaded = read(&path);
        assert_eq!(loaded.source, Source::File);
        assert_eq!(loaded.config.flight.count, 7);
    }

    #[test]
    fn a_broken_file_reads_as_the_defaults() {
        let dir = scratch("broken");
        let path = dir.join("config.toml");
        std::fs::write(&path, b"\xff\xfe[flight\ncount = = 7").unwrap();
        let loaded = read(&path);
        assert_eq!(loaded.source, Source::NotToml);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn a_file_at_the_size_limit_is_read_and_one_byte_more_is_not() {
        let dir = scratch("limit");
        let path = dir.join("config.toml");
        let mut text = b"[flight]\ncount = 7\n".to_vec();
        text.resize(MAX_FILE_BYTES, b'\n');
        std::fs::write(&path, &text).unwrap();
        assert_eq!(read(&path).config.flight.count, 7);
        text.push(b'\n');
        std::fs::write(&path, &text).unwrap();
        let loaded = read(&path);
        assert_eq!(loaded.source, Source::TooLarge);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn a_folder_in_place_of_the_file_reads_as_the_defaults() {
        let dir = scratch("folder");
        let path = dir.join("config.toml");
        std::fs::create_dir(&path).unwrap();
        let loaded = read(&path);
        assert!(
            matches!(loaded.source, Source::Unreadable(_)),
            "{:?}",
            loaded.source
        );
        assert_eq!(loaded.config, Config::default());
    }

    // Writing.

    fn leftovers(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name != "config.toml")
            .collect()
    }

    #[test]
    fn writing_creates_the_folder_and_the_file() {
        let dir = scratch("create").join("FlyingMicrowave");
        let path = dir.join("config.toml");
        write(&path, "[flight]\ncount = 9\n").expect("write");
        assert_eq!(read(&path).config.flight.count, 9);
        assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    }

    #[test]
    fn writing_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = scratch("replace");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[flight]\ncount = 3\n# a comment\n").unwrap();
        write(&path, "[flight]\ncount = 4\n").expect("write");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[flight]\ncount = 4\n"
        );
        assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    }

    #[test]
    fn a_failed_replacement_keeps_nothing_half_written() {
        let dir = scratch("fail");
        let path = dir.join("config.toml");
        // A non-empty folder where the file should be cannot be replaced.
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), "x").unwrap();
        assert!(write(&path, "[flight]\ncount = 4\n").is_err());
        assert!(path.join("keep").exists());
        assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    }

    // The log line.

    #[test]
    fn the_summary_names_neither_the_path_nor_values() {
        let dir = scratch("summary");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[flight]\ncount = \"secret\"\n").unwrap();
        let startup = Startup {
            loaded: read(&path),
            location: Ok(Location {
                path: path.clone(),
                from_env: true,
            }),
            elapsed: Duration::from_micros(1500),
        };
        let line = startup.summary();
        assert!(
            line.starts_with("config: read, 1 items fell back"),
            "{line}"
        );
        assert!(line.contains(DIR_ENV), "{line}");
        assert!(!line.contains("secret"), "{line}");
        assert!(!line.contains(&*dir.to_string_lossy()), "{line}");
    }
}
