//! What the flight simulation shares with the rest of the program: the fixed
//! time step, the tunable parameters and the number of body colors
//! (docs/design/architecture.md).
//!
//! The 3D flight itself is [`crate::flight`] (ADR-0026). The 2D simulation
//! of v0.2.0 (ADR-0023) was removed when the renderer moved to 3D (plan
//! 0005, task 6).

use std::time::Duration;

/// Length of one simulation step (about 1/120 s).
pub const TIME_STEP: Duration = Duration::from_nanos(8_333_333);

/// Longest elapsed time accepted by one `flight::World::advance` call.
///
/// Longer gaps (a suspended machine, a debugger stop) are clamped so the
/// simulation never has to catch up with a huge number of steps.
pub const MAX_ELAPSED: Duration = Duration::from_millis(250);

/// Number of body colors. `flight::FlyerInstance::color` is below this value.
pub const BODY_COLORS: u8 = 4;

/// Tunable simulation parameters.
///
/// `speed` and `scale` are multipliers. Callers pass finite, positive values;
/// validating them is the job of `core::config`, so this module does not
/// check them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimParams {
    /// Number of flyers.
    pub count: u32,
    /// Speed multiplier (1.0 is the default speed).
    pub speed: f32,
    /// Size multiplier (1.0 is the default size).
    pub scale: f32,
    /// Number of food items flying besides the `count` microwaves.
    pub food_count: u32,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            count: 12,
            speed: 1.0,
            scale: 1.0,
            food_count: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_neutral() {
        let p = SimParams::default();
        assert_eq!(p.count, 12);
        assert_eq!(p.speed, 1.0);
        assert_eq!(p.scale, 1.0);
        assert_eq!(p.food_count, 4);
    }
}
