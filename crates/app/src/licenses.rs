//! The third-party license notices shown on the About tab (plan 0004,
//! F-05 and D-12; ADR-0011).
//!
//! The release puts `THIRD-PARTY-LICENSES.txt` next to the executable, so
//! the text is read from there. Everything here is plain `std`, so it is
//! tested on any host.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Name of the file that comes with the release, next to the executable.
pub const FILE_NAME: &str = "THIRD-PARTY-LICENSES.txt";

/// Largest file that is read, in bytes.
///
/// Why: the file of v0.2.0 is about 250 KB; 2 MiB leaves room for many more
/// dependencies, while a file that someone replaced with something huge is
/// neither read into memory nor laid out by the settings window.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Why the notices could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// The path of the executable is unknown.
    NoProgramPath(String),
    /// There is no such file next to the executable.
    NotFound,
    /// The file is larger than [`MAX_FILE_BYTES`].
    TooLarge,
    /// The file is not UTF-8 text.
    NotUtf8,
    /// Opening or reading failed.
    Io(String),
}

impl fmt::Display for Unreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgramPath(reason) => write!(f, "the program path is unknown: {reason}"),
            Self::NotFound => f.write_str("not found"),
            Self::TooLarge => write!(f, "larger than {MAX_FILE_BYTES} bytes"),
            Self::NotUtf8 => f.write_str("not UTF-8 text"),
            Self::Io(reason) => f.write_str(reason),
        }
    }
}

/// Where the notices are for the executable at `exe`: the file
/// [`FILE_NAME`] in the same folder.
///
/// Only an absolute path counts: the folder of a relative one would be the
/// current folder, which is not where the release put the file.
pub fn path_next_to(exe: &Path) -> Option<PathBuf> {
    if !exe.is_absolute() {
        return None;
    }
    exe.parent().map(|dir| dir.join(FILE_NAME))
}

/// Reads the notices from `path`.
///
/// Text that is not UTF-8 is not shown at all, rather than shown with
/// replacement characters: the release writes UTF-8, so anything else is
/// not the file the release put there.
pub fn read(path: &Path) -> Result<String, Unreadable> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Unreadable::NotFound),
        Err(e) => return Err(Unreadable::Io(e.to_string())),
    };
    // One byte more than the limit is enough to tell that it is too large,
    // without reading a huge file.
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Unreadable::Io(e.to_string()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(Unreadable::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| Unreadable::NotUtf8)
}

/// Reads the notices next to the executable, given the result of
/// [`std::env::current_exe`].
///
/// The path is built only from the folder of the executable and
/// [`FILE_NAME`]; no environment variable or argument takes part.
pub fn load_from(exe: io::Result<PathBuf>) -> Result<String, Unreadable> {
    let exe = exe.map_err(|e| Unreadable::NoProgramPath(e.to_string()))?;
    let path = path_next_to(&exe)
        .ok_or_else(|| Unreadable::NoProgramPath("not an absolute path".to_owned()))?;
    read(&path)
}

/// Reads the notices next to this executable.
pub fn load() -> Result<String, Unreadable> {
    load_from(std::env::current_exe())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flying-microwave-licenses-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test folder");
        dir
    }

    // Where the file is.

    #[test]
    fn the_file_is_next_to_the_executable() {
        let exe = std::env::temp_dir().join("app").join("FlyingMicrowave.scr");
        assert_eq!(
            path_next_to(&exe),
            Some(std::env::temp_dir().join("app").join(FILE_NAME))
        );
    }

    #[test]
    fn a_path_without_a_folder_has_no_file_next_to_it() {
        assert_eq!(path_next_to(Path::new("")), None);
    }

    #[test]
    fn a_relative_program_path_is_not_used() {
        // It would read from the current folder.
        assert_eq!(path_next_to(Path::new("FlyingMicrowave.scr")), None);
        assert_eq!(path_next_to(Path::new("app/FlyingMicrowave.scr")), None);
    }

    #[test]
    fn without_the_program_path_nothing_is_read() {
        let failed = Err(io::Error::other("access denied"));
        assert_eq!(
            load_from(failed),
            Err(Unreadable::NoProgramPath("access denied".to_owned()))
        );
        assert!(matches!(
            load_from(Ok(PathBuf::from("FlyingMicrowave.scr"))),
            Err(Unreadable::NoProgramPath(_))
        ));
    }

    #[test]
    fn the_file_next_to_the_program_is_loaded() {
        let dir = scratch("load");
        std::fs::write(dir.join(FILE_NAME), "MIT License\n").unwrap();
        assert_eq!(
            load_from(Ok(dir.join("FlyingMicrowave.scr"))).as_deref(),
            Ok("MIT License\n")
        );
        let empty = scratch("load-missing");
        assert_eq!(
            load_from(Ok(empty.join("FlyingMicrowave.scr"))),
            Err(Unreadable::NotFound)
        );
    }

    // Reading.

    #[test]
    fn a_text_file_is_read_as_it_is() {
        let dir = scratch("text");
        let path = dir.join(FILE_NAME);
        let text = "Third-party software\r\n\tMIT License\r\n\u{a9} 2026 Someone\n";
        std::fs::write(&path, text).unwrap();
        assert_eq!(read(&path).as_deref(), Ok(text));
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let dir = scratch("missing");
        assert_eq!(read(&dir.join(FILE_NAME)), Err(Unreadable::NotFound));
    }

    #[test]
    fn a_file_at_the_size_limit_is_read_and_one_byte_more_is_not() {
        let dir = scratch("limit");
        let path = dir.join(FILE_NAME);
        let mut bytes = vec![b'x'; MAX_FILE_BYTES as usize];
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read(&path).map(|text| text.len()), Ok(bytes.len()));
        bytes.push(b'x');
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read(&path), Err(Unreadable::TooLarge));
    }

    #[test]
    fn a_file_that_is_not_utf8_is_not_shown() {
        let dir = scratch("latin1");
        let path = dir.join(FILE_NAME);
        // "(c) 2026" with the copyright sign in Latin-1.
        std::fs::write(&path, b"\xa9 2026 Someone\n").unwrap();
        assert_eq!(read(&path), Err(Unreadable::NotUtf8));
    }

    #[test]
    fn a_folder_in_place_of_the_file_is_unreadable() {
        let dir = scratch("folder");
        let path = dir.join(FILE_NAME);
        std::fs::create_dir(&path).unwrap();
        let result = read(&path);
        assert!(matches!(result, Err(Unreadable::Io(_))), "{result:?}");
    }

    #[test]
    fn every_reason_reads_as_a_phrase_for_the_log() {
        let reasons = [
            (
                Unreadable::NoProgramPath("access denied".to_owned()),
                "the program path is unknown: access denied",
            ),
            (Unreadable::NotFound, "not found"),
            (Unreadable::TooLarge, "larger than 2097152 bytes"),
            (Unreadable::NotUtf8, "not UTF-8 text"),
            (Unreadable::Io("access denied".to_owned()), "access denied"),
        ];
        for (reason, text) in reasons {
            assert_eq!(reason.to_string(), text);
        }
    }
}
