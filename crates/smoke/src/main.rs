//! Smoke test of the built screensaver (docs/plans/0003-v0.1.1-smoke-test.md).
//!
//! Usage: `flying-microwave-smoke <path to FlyingMicrowave.scr or flying-microwave.exe>`
//!
//! Runs the real executable in each mode and checks that it starts and ends
//! as the screensaver protocol requires (F-01 to F-05 of plan 0003). Every
//! wait has a limit, and a process left running is killed and reported.
//! Messages are in English, like the other CI output of the product.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    let Some(exe) = std::env::args_os().nth(1) else {
        eprintln!("usage: flying-microwave-smoke <path to the executable>");
        std::process::exit(2);
    };
    std::process::exit(windows::run(std::path::Path::new(&exe)));
}

#[cfg(not(windows))]
fn main() {
    // The screensaver is Windows only (ADR-0022); this build exists so the
    // workspace can be checked and tested on Linux.
    eprintln!("the smoke test runs on Windows only");
    std::process::exit(2);
}
