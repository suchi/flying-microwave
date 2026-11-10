//! The limit on GPU initialization (plan 0001, D-27; plan 0005, D-1).
//!
//! A hung driver call blocks the thread that made it, so a timer thread
//! ends the process instead. The timer and the code that finishes the
//! initialization race: exactly one of them wins, because the state moves
//! only once, from armed to fired or to disarmed. This part has no Windows
//! dependency, so it is tested on every host; the effects (the log line,
//! the owner, the message box, the exit) are passed in by the callers.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

const ARMED: u8 = 0;
const FIRED: u8 = 1;
const DISARMED: u8 = 2;

/// The result of disarming a [`Limit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disarm {
    /// The limit had not been reached (or was already disarmed): go on.
    InTime,
    /// The limit was reached first; the timer thread ends the process.
    TooLate,
}

/// A limit that fires once unless it is disarmed first. Clones share it.
#[derive(Debug, Clone)]
pub struct Limit {
    state: Arc<AtomicU8>,
}

impl Limit {
    /// An armed limit without a timer.
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(ARMED)),
        }
    }

    /// Fires the limit if it is still armed; true if this call fired it.
    pub fn fire(&self) -> bool {
        self.state
            .compare_exchange(ARMED, FIRED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Disarms the limit if it is still armed, and tells whether it fired.
    pub fn disarm(&self) -> Disarm {
        match self
            .state
            .compare_exchange(ARMED, DISARMED, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) | Err(DISARMED) => Disarm::InTime,
            Err(_) => Disarm::TooLate,
        }
    }

    /// Starts a thread that fires the limit after `timeout` and, if that
    /// fired it, runs `on_fire`.
    pub fn start(
        timeout: Duration,
        on_fire: impl FnOnce() + Send + 'static,
    ) -> std::io::Result<Self> {
        let limit = Self::new();
        let timer = limit.clone();
        // A thread, because a hung driver call also blocks the event loop.
        std::thread::Builder::new()
            .name("gpu-init-watchdog".to_owned())
            .spawn(move || {
                std::thread::sleep(timeout);
                if timer.fire() {
                    on_fire();
                }
            })?;
        Ok(limit)
    }
}

/// What the settings window does when the limit is reached, called in the
/// order of plan 0005, D-1 (2) by [`settings_timed_out`].
pub trait SettingsTimeout {
    /// Writes the line that the smoke test reads (D-1 (4)).
    fn log(&mut self);
    /// Enables the owner again, if this process disabled it.
    fn release_owner(&mut self);
    /// Tells the user that the window could not be opened.
    fn show_message(&mut self);
    /// Ends the process with exit code 7.
    fn exit(&mut self);
}

/// Runs the effects of a reached limit in the settings window. The log line
/// comes before the message box, so that the smoke test can read it as soon
/// as it sees the box.
pub fn settings_timed_out(effects: &mut impl SettingsTimeout) {
    effects.log();
    effects.release_owner();
    effects.show_message();
    effects.exit();
}

/// What the settings window does after `eframe::run_native` has returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterRun {
    /// Go on as before: close normally, or show the failure.
    GoOn,
    /// The limit was reached: do not return, and let the timer thread show
    /// its message box and end the process. Returning would end the process
    /// and take that message box with it (plan 0005, D-1 (3)).
    WaitForExit,
}

/// Decides [`AfterRun`] from disarming the limit after `run_native`.
pub fn after_run(disarm: Disarm) -> AfterRun {
    match disarm {
        Disarm::InTime => AfterRun::GoOn,
        Disarm::TooLate => AfterRun::WaitForExit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_disarmed_limit_does_not_fire() {
        let limit = Limit::new();
        assert_eq!(limit.disarm(), Disarm::InTime);
        assert!(!limit.fire());
        // Disarming again changes nothing.
        assert_eq!(limit.disarm(), Disarm::InTime);
    }

    #[test]
    fn a_fired_limit_cannot_be_disarmed_in_time() {
        let limit = Limit::new();
        assert!(limit.fire());
        // It fires only once.
        assert!(!limit.fire());
        assert_eq!(limit.disarm(), Disarm::TooLate);
        assert_eq!(limit.disarm(), Disarm::TooLate);
    }

    #[test]
    fn clones_share_the_state() {
        let limit = Limit::new();
        let other = limit.clone();
        assert!(other.fire());
        assert_eq!(limit.disarm(), Disarm::TooLate);
    }

    #[test]
    fn the_timer_fires_and_runs_the_effects_once() {
        let (sent, received) = mpsc::channel();
        let limit =
            Limit::start(Duration::from_millis(10), move || sent.send(()).unwrap()).unwrap();
        assert!(received.recv_timeout(Duration::from_secs(5)).is_ok());
        assert_eq!(limit.disarm(), Disarm::TooLate);
    }

    #[test]
    fn a_limit_disarmed_in_time_runs_no_effects() {
        // Why 60 s: the test must not depend on how fast this thread runs; a
        // short timer could fire first on a busy runner.
        let (sent, received) = mpsc::channel::<()>();
        let limit = Limit::start(Duration::from_secs(60), move || sent.send(()).unwrap()).unwrap();
        assert_eq!(limit.disarm(), Disarm::InTime);
        // The timer can no longer fire, so its effects cannot run.
        assert!(!limit.fire());
        assert_eq!(
            received.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
    }

    #[derive(Default)]
    struct Recorder(Vec<&'static str>);

    impl SettingsTimeout for Recorder {
        fn log(&mut self) {
            self.0.push("log");
        }
        fn release_owner(&mut self) {
            self.0.push("release owner");
        }
        fn show_message(&mut self) {
            self.0.push("message box");
        }
        fn exit(&mut self) {
            self.0.push("exit");
        }
    }

    #[test]
    fn the_settings_timeout_logs_first_and_exits_last() {
        let mut recorder = Recorder::default();
        settings_timed_out(&mut recorder);
        assert_eq!(recorder.0, ["log", "release owner", "message box", "exit"]);
    }

    #[test]
    fn after_a_reached_limit_the_main_thread_does_not_return() {
        assert_eq!(after_run(Disarm::InTime), AfterRun::GoOn);
        assert_eq!(after_run(Disarm::TooLate), AfterRun::WaitForExit);
    }
}
