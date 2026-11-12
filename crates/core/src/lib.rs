//! Platform-independent logic for the Flying Microwave screensaver.
//!
//! This crate must not depend on any OS or GPU API (ADR-0009).

#![forbid(unsafe_code)]

pub mod clock;
pub mod config;
pub mod exit_policy;
pub mod flight;
pub mod frame_loop;
pub mod monitors;
pub mod pacing;
pub mod rng;
pub mod sim;
pub mod space;
