//! Command-line parsing for settings-spike. Same rules as saver-spike, but
//! only the settings forms matter here. No platform dependencies, so it is
//! unit tested on any host.

/// Where the owner window of the settings window comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// No owner: a plain top-level window (no argument).
    None,
    /// `/c` without an HWND: use the foreground window, if it is valid.
    Foreground,
    /// `/c:1234` or `/c 1234`: use this window handle, if it is valid.
    Explicit(usize),
}

/// What the process was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Show the settings window (no argument or `/c`).
    Settings(Owner),
    /// `/s` or `/p`: not a settings mode. The spike exits quietly without
    /// showing anything, so that selecting it as the screensaver is harmless.
    NotSettings,
    /// Unknown arguments: handled like no argument (W-8), but logged.
    Unrecognized,
}

/// Parses the arguments that follow the program name.
pub fn parse<S: AsRef<str>>(args: &[S]) -> Mode {
    let Some(first) = args.first() else {
        return Mode::Settings(Owner::None);
    };
    let first = first.as_ref().to_ascii_lowercase();
    let Some(body) = first.strip_prefix(['/', '-']) else {
        return Mode::Unrecognized;
    };
    let (command, inline_value) = match body.split_once(':') {
        Some((command, value)) => (command, Some(value)),
        None => (body, None),
    };
    match command {
        "c" => {
            let hwnd = parse_hwnd(inline_value.or_else(|| args.get(1).map(AsRef::as_ref)));
            Mode::Settings(hwnd.map_or(Owner::Foreground, Owner::Explicit))
        }
        "s" | "p" => Mode::NotSettings,
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

    #[test]
    fn no_argument_means_no_owner() {
        assert_eq!(parse::<&str>(&[]), Mode::Settings(Owner::None));
    }

    #[test]
    fn c_without_hwnd_uses_the_foreground_window() {
        for args in [&["/c"][..], &["-C"], &["/c", "abc"], &["/c:"], &["/c:-1"]] {
            assert_eq!(parse(args), Mode::Settings(Owner::Foreground), "{args:?}");
        }
    }

    #[test]
    fn c_with_hwnd() {
        let expected = Mode::Settings(Owner::Explicit(1234));
        assert_eq!(parse(&["/c:1234"]), expected);
        assert_eq!(parse(&["/C", "1234"]), expected);
        assert_eq!(parse(&["-c:1234", "extra"]), expected);
    }

    #[test]
    fn screensaver_modes_are_not_settings() {
        for args in [&["/s"][..], &["/S"], &["-p", "5"], &["/p:5"]] {
            assert_eq!(parse(args), Mode::NotSettings, "{args:?}");
        }
    }

    #[test]
    fn everything_else_is_unrecognized() {
        for args in [&["foo"][..], &["--c"], &["/x"], &["--settings"], &[""]] {
            assert_eq!(parse(args), Mode::Unrecognized, "{args:?}");
        }
    }
}
