//! Platform-independent logic for the Flying Microwave screensaver.
//!
//! This crate must not depend on any OS or GPU API (ADR-0009).

#![forbid(unsafe_code)]

pub mod rng;
pub mod sim;
