//! Command-line mode selection.
//!
//! Windows launches a screensaver with `/s` (run), `/p <hwnd>` (preview in the
//! small Settings window) or `/c[:<hwnd>]` (configure), and with no arguments
//! when the user opens the file directly. This module maps the arguments to a
//! [`Mode`] without touching the system, so it can be tested on any platform.

use std::ffi::{OsStr, OsString};

/// The window that owns the settings dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// No owner window.
    None,
    /// The foreground window at the time the dialog opens (plain `/c`).
    Foreground,
    /// An explicit window handle, not yet checked to be a real window.
    Window(u64),
}

/// What the program should do for the given command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run full screen as the screensaver.
    Screensaver,
    /// Render into the preview window with the given handle.
    Preview {
        /// Handle of the parent window, not yet checked to be a real window.
        parent: u64,
    },
    /// Show the settings dialog.
    Settings {
        /// Window that owns the dialog.
        owner: Owner,
    },
    /// Run as an ordinary program, outside the screensaver protocol.
    Standalone {
        /// Whether to cover the whole screen instead of using a window.
        fullscreen: bool,
    },
}

/// The safe fallback: showing a dialog can never cover the screen by mistake.
const SETTINGS: Mode = Mode::Settings { owner: Owner::None };

/// Selects the mode from the command line.
///
/// `args` excludes the program name. `exe_extension` is the extension of the
/// running executable (from `std::env::current_exe()`), without the dot.
///
/// Only the first argument decides the mode. Later arguments are ignored,
/// except where `/p` and `/c` read the second one as a window handle.
/// Anything unrecognized yields the settings dialog, never the screensaver.
pub fn parse(args: &[OsString], exe_extension: Option<&OsStr>) -> Mode {
    let Some(first) = args.first() else {
        return default_mode(exe_extension);
    };
    let Some(first) = first.to_str() else {
        return SETTINGS;
    };

    match first {
        "--window" => return Mode::Standalone { fullscreen: false },
        "--fullscreen" => return Mode::Standalone { fullscreen: true },
        "--settings" => return SETTINGS,
        _ => {}
    }

    let Some(body) = first.strip_prefix(['/', '-']) else {
        return SETTINGS;
    };
    if body.starts_with('-') {
        return SETTINGS;
    }

    let (command, inline) = match body.split_once(':') {
        Some((command, value)) => (command, Some(value)),
        None => (body, None),
    };
    // An inline value (even an empty one) wins over the second argument.
    let handle_text = inline.or_else(|| args.get(1).and_then(|arg| arg.to_str()));

    match command.to_ascii_lowercase().as_str() {
        // Strict on purpose: guessing wrong here would cover the screen.
        "s" if inline.is_none() => Mode::Screensaver,
        "p" => match handle_text.and_then(parse_handle) {
            Some(parent) => Mode::Preview { parent },
            None => SETTINGS,
        },
        "c" => Mode::Settings {
            owner: match handle_text.and_then(parse_handle) {
                Some(handle) => Owner::Window(handle),
                None => Owner::Foreground,
            },
        },
        _ => SETTINGS,
    }
}

/// Mode for a launch without arguments, decided by the executable's extension.
fn default_mode(exe_extension: Option<&OsStr>) -> Mode {
    match exe_extension {
        Some(ext) if ext.eq_ignore_ascii_case("exe") => Mode::Standalone { fullscreen: false },
        _ => SETTINGS,
    }
}

/// Parses a window handle: non-empty, ASCII digits only, and fits in `u64`.
///
/// Whether the handle names a real window is checked elsewhere.
fn parse_handle(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mode = Mode::Settings { owner: Owner::None };
    const FOREGROUND: Mode = Mode::Settings {
        owner: Owner::Foreground,
    };

    fn window(handle: u64) -> Mode {
        Mode::Settings {
            owner: Owner::Window(handle),
        }
    }

    fn preview(parent: u64) -> Mode {
        Mode::Preview { parent }
    }

    fn standalone(fullscreen: bool) -> Mode {
        Mode::Standalone { fullscreen }
    }

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    /// Parses with the extension `scr`, the normal way Windows runs a screensaver.
    fn parse_scr(args: &[&str]) -> Mode {
        parse(&os_args(args), Some(OsStr::new("scr")))
    }

    fn parse_with_ext(args: &[&str], ext: &str) -> Mode {
        parse(&os_args(args), Some(OsStr::new(ext)))
    }

    #[cfg(unix)]
    fn non_utf8() -> OsString {
        std::os::unix::ffi::OsStringExt::from_vec(vec![0xff])
    }

    #[cfg(windows)]
    fn non_utf8() -> OsString {
        std::os::windows::ffi::OsStringExt::from_wide(&[0xD800])
    }

    // Rule 1: no arguments depend on the executable's extension.

    #[test]
    fn no_args_scr_extension_opens_settings() {
        for ext in ["scr", "SCR", "Scr"] {
            assert_eq!(parse_with_ext(&[], ext), NONE, "extension {ext:?}");
        }
    }

    #[test]
    fn no_args_exe_extension_runs_standalone_window() {
        for ext in ["exe", "EXE", "Exe"] {
            assert_eq!(
                parse_with_ext(&[], ext),
                standalone(false),
                "extension {ext:?}"
            );
        }
    }

    #[test]
    fn no_args_other_or_missing_extension_opens_settings() {
        for ext in ["bin", "", "scr2", "exe "] {
            assert_eq!(parse_with_ext(&[], ext), NONE, "extension {ext:?}");
        }
        assert_eq!(parse(&[], None), NONE);
    }

    // Rule 2: a first argument that is not UTF-8 is never guessed at.

    #[cfg(any(unix, windows))]
    #[test]
    fn non_utf8_first_argument_opens_settings() {
        let scr = Some(OsStr::new("scr"));
        let exe = Some(OsStr::new("exe"));
        assert_eq!(parse(&[non_utf8()], scr), NONE);
        assert_eq!(parse(&[non_utf8()], exe), NONE);
        assert_eq!(parse(&[non_utf8(), OsString::from("/s")], scr), NONE);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn non_utf8_window_handle_is_invalid() {
        let scr = Some(OsStr::new("scr"));
        assert_eq!(parse(&[OsString::from("/p"), non_utf8()], scr), NONE);
        assert_eq!(parse(&[OsString::from("/c"), non_utf8()], scr), FOREGROUND);
    }

    // Rule 3: exact, case-sensitive long options, whatever the extension.

    #[test]
    fn long_options_select_the_mode_regardless_of_extension() {
        for ext in ["scr", "exe"] {
            assert_eq!(parse_with_ext(&["--window"], ext), standalone(false));
            assert_eq!(parse_with_ext(&["--fullscreen"], ext), standalone(true));
            assert_eq!(parse_with_ext(&["--settings"], ext), NONE);
        }
        assert_eq!(parse(&os_args(&["--window"]), None), standalone(false));
        assert_eq!(parse(&os_args(&["--fullscreen"]), None), standalone(true));
    }

    #[test]
    fn long_options_ignore_later_arguments() {
        assert_eq!(parse_scr(&["--window", "/s"]), standalone(false));
        assert_eq!(parse_scr(&["--fullscreen", "/p", "1234"]), standalone(true));
        assert_eq!(parse_scr(&["--settings", "/s"]), NONE);
    }

    #[test]
    fn long_options_are_case_sensitive() {
        for arg in ["--Window", "--WINDOW", "--Fullscreen", "--Settings"] {
            assert_eq!(parse_scr(&[arg]), NONE, "argument {arg:?}");
        }
    }

    // Rule 4: a long option is never mistaken for a short one.

    #[test]
    fn unknown_double_dash_arguments_open_settings() {
        for arg in [
            "--s",
            "--S",
            "--p",
            "--c",
            "--setting",
            "--",
            "--s:1",
            "--p:1234",
        ] {
            assert_eq!(parse_scr(&[arg]), NONE, "argument {arg:?}");
        }
        assert_eq!(parse_scr(&["--p", "1234"]), NONE);
        assert_eq!(parse_scr(&["--c", "1234"]), NONE);
        assert_eq!(parse_with_ext(&["--s"], "exe"), NONE);
    }

    #[test]
    fn s_starts_the_screensaver() {
        for arg in ["/s", "/S", "-s", "-S"] {
            assert_eq!(parse_scr(&[arg]), Mode::Screensaver, "argument {arg:?}");
        }
    }

    #[test]
    fn s_starts_the_screensaver_regardless_of_extension() {
        assert_eq!(parse_with_ext(&["/s"], "exe"), Mode::Screensaver);
        assert_eq!(parse(&os_args(&["/s"]), None), Mode::Screensaver);
    }

    #[test]
    fn s_ignores_later_arguments() {
        assert_eq!(parse_scr(&["/s", "extra"]), Mode::Screensaver);
        assert_eq!(parse_scr(&["/s", "/p", "1234"]), Mode::Screensaver);
    }

    #[test]
    fn s_with_an_inline_value_opens_settings() {
        for arg in ["/s:1", "/s:", "-s:1", "/S:abc"] {
            assert_eq!(parse_scr(&[arg]), NONE, "argument {arg:?}");
        }
    }

    #[test]
    fn s_without_a_prefix_opens_settings() {
        assert_eq!(parse_scr(&["s"]), NONE);
        assert_eq!(parse_scr(&["S"]), NONE);
    }

    #[test]
    fn p_opens_the_preview_with_the_window_handle() {
        let cases = [
            (vec!["/p", "1234"], 1234),
            (vec!["/p:1234"], 1234),
            (vec!["/P:1234"], 1234),
            (vec!["/P", "1234"], 1234),
            (vec!["-p", "1234"], 1234),
            (vec!["-P:1234"], 1234),
            (vec!["/p", "1234", "extra"], 1234),
            (vec!["/p", "0"], 0),
            (vec!["/p", "18446744073709551615"], u64::MAX),
        ];
        for (args, parent) in cases {
            assert_eq!(parse_scr(&args), preview(parent), "arguments {args:?}");
        }
    }

    #[test]
    fn p_prefers_the_inline_value_over_the_second_argument() {
        assert_eq!(parse_scr(&["/p:1", "2"]), preview(1));
        assert_eq!(parse_scr(&["/p:", "1234"]), NONE);
    }

    #[test]
    fn p_with_an_invalid_window_handle_opens_settings() {
        let cases: [&[&str]; 14] = [
            &["/p"],
            &["/p:"],
            &["/p", ""],
            &["/p", "abc"],
            &["/p", "-5"],
            &["/p", "+5"],
            &["/p", "12 34"],
            &["/p", " 1234"],
            &["/p", "0x10"],
            &["/p", "1.5"],
            &["/p", "18446744073709551616"],
            &["/p", "99999999999999999999"],
            &["/p", "\u{ff11}\u{ff12}"],
            &["/p:abc"],
        ];
        for args in cases {
            assert_eq!(parse_scr(args), NONE, "arguments {args:?}");
        }
    }

    #[test]
    fn c_without_a_valid_window_handle_uses_the_foreground_window() {
        let cases: [&[&str]; 8] = [
            &["/c"],
            &["/C"],
            &["-c"],
            &["/c", "abc"],
            &["/c", ""],
            &["/c:"],
            &["/c:abc"],
            &["/c", "99999999999999999999"],
        ];
        for args in cases {
            assert_eq!(parse_scr(args), FOREGROUND, "arguments {args:?}");
        }
    }

    #[test]
    fn c_with_a_valid_window_handle_uses_that_window() {
        let cases = [
            (vec!["/c:1234"], 1234),
            (vec!["/C:1234"], 1234),
            (vec!["/c", "1234"], 1234),
            (vec!["-c", "1234"], 1234),
            (vec!["/c", "1234", "extra"], 1234),
            (vec!["/c", "0"], 0),
        ];
        for (args, handle) in cases {
            assert_eq!(parse_scr(&args), window(handle), "arguments {args:?}");
        }
    }

    #[test]
    fn unknown_commands_open_settings() {
        for arg in [
            "/",
            "-",
            "/x",
            "/sp",
            "/ps",
            "/setup",
            "/?",
            "/\u{ff53}",
            "-1",
        ] {
            assert_eq!(parse_scr(&[arg]), NONE, "argument {arg:?}");
        }
        assert_eq!(parse_with_ext(&["/x"], "exe"), NONE);
    }

    // Rule 5: only the first argument decides.

    #[test]
    fn a_later_s_does_not_start_the_screensaver() {
        assert_eq!(parse_scr(&["foo /s"]), NONE);
        assert_eq!(parse_scr(&["foo", "/s"]), NONE);
        assert_eq!(parse_scr(&["", "/s"]), NONE);
        assert_eq!(parse_scr(&["/x", "/s"]), NONE);
        assert_eq!(parse_with_ext(&["foo", "/s"], "exe"), NONE);
    }

    #[test]
    fn an_empty_first_argument_opens_settings() {
        assert_eq!(parse_scr(&[""]), NONE);
        assert_eq!(parse_with_ext(&[""], "exe"), NONE);
        assert_eq!(parse(&os_args(&[""]), None), NONE);
    }

    #[test]
    fn a_leading_space_is_not_trimmed() {
        assert_eq!(parse_scr(&[" /s"]), NONE);
        assert_eq!(parse_scr(&["/s "]), NONE);
    }
}
