//! Command-line parsing for the spike (spike S3 records what Windows really passes).
//!
//! The rules follow docs/design/architecture.md, but this spike is deliberately
//! simple: only the first argument selects the mode. This module has no
//! platform dependencies, so it is unit tested on any host.

/// What the process was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `/s`: run the screensaver on every monitor.
    Screensaver,
    /// `/p <HWND>`: render into a child window of the given window.
    /// `None` means the HWND was missing or not an unsigned decimal number.
    Preview { hwnd: Option<usize> },
    /// `/c` (optionally with an HWND): the settings dialog is tested elsewhere.
    Settings { hwnd: Option<usize> },
    /// `--self-test`: render a few frames with the software adapter and exit.
    SelfTest,
    /// Anything else, including no argument at all.
    Unrecognized,
}

/// Parses the arguments that follow the program name.
pub fn parse<S: AsRef<str>>(args: &[S]) -> Mode {
    let Some(first) = args.first() else {
        return Mode::Unrecognized;
    };
    let first = first.as_ref().to_ascii_lowercase();

    // The only long option. It is matched exactly so that `--s` can never be
    // mistaken for `-s`.
    if first == "--self-test" {
        return Mode::SelfTest;
    }

    // Accept `/x` and `-x`, optionally followed by `:value`.
    let Some(body) = first.strip_prefix(['/', '-']) else {
        return Mode::Unrecognized;
    };
    let (command, inline_value) = match body.split_once(':') {
        Some((command, value)) => (command, Some(value)),
        None => (body, None),
    };
    // The HWND is either glued on after ':' or the next argument.
    let hwnd = parse_hwnd(inline_value.or_else(|| args.get(1).map(AsRef::as_ref)));

    match (command, inline_value) {
        // Strict on purpose: `/s:1` is not a form Windows is documented to use,
        // and guessing wrong would cover the screen by mistake.
        ("s", None) => Mode::Screensaver,
        ("p", _) => Mode::Preview { hwnd },
        ("c", _) => Mode::Settings { hwnd },
        _ => Mode::Unrecognized,
    }
}

/// Parses an unsigned decimal window handle. Signs, spaces and hex are rejected.
fn parse_hwnd(text: Option<&str>) -> Option<usize> {
    let text = text?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Mode {
        parse(args)
    }

    #[test]
    fn no_argument_is_unrecognized() {
        assert_eq!(p(&[]), Mode::Unrecognized);
    }

    #[test]
    fn screensaver_forms() {
        for form in ["/s", "/S", "-s", "-S"] {
            assert_eq!(p(&[form]), Mode::Screensaver, "{form}");
        }
        // Extra arguments after the first one are ignored.
        assert_eq!(p(&["/s", "extra"]), Mode::Screensaver);
    }

    #[test]
    fn screensaver_is_strict() {
        for form in ["s", "--s", "/s:1", "/ss", "foo", "/x", "", "/", "-"] {
            assert_eq!(p(&[form]), Mode::Unrecognized, "{form}");
        }
        // A later `/s` must never start the screensaver.
        assert_eq!(p(&["foo", "/s"]), Mode::Unrecognized);
    }

    #[test]
    fn preview_forms() {
        let ok = Mode::Preview { hwnd: Some(1234) };
        assert_eq!(p(&["/p", "1234"]), ok);
        assert_eq!(p(&["/P", "1234"]), ok);
        assert_eq!(p(&["-p:1234"]), ok);
        assert_eq!(p(&["/p:1234", "extra"]), ok);
    }

    #[test]
    fn preview_with_bad_hwnd_keeps_the_mode() {
        let bad = Mode::Preview { hwnd: None };
        for args in [
            &["/p"][..],
            &["/p", "abc"],
            &["/p", "-5"],
            &["/p", "+5"],
            &["/p", "0x10"],
            &["/p:"],
            &["/p", "99999999999999999999999999"],
        ] {
            assert_eq!(p(args), bad, "{args:?}");
        }
    }

    #[test]
    fn settings_forms() {
        assert_eq!(p(&["/c"]), Mode::Settings { hwnd: None });
        assert_eq!(p(&["/c:77"]), Mode::Settings { hwnd: Some(77) });
        assert_eq!(p(&["/C", "77"]), Mode::Settings { hwnd: Some(77) });
        assert_eq!(p(&["-c", "oops"]), Mode::Settings { hwnd: None });
    }

    #[test]
    fn self_test_is_exact() {
        assert_eq!(p(&["--self-test"]), Mode::SelfTest);
        assert_eq!(p(&["--SELF-TEST"]), Mode::SelfTest);
        assert_eq!(p(&["--self-test-x"]), Mode::Unrecognized);
        assert_eq!(p(&["/self-test"]), Mode::Unrecognized);
    }
}
