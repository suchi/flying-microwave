//! Flyer simulation with a fixed time step (docs/design/architecture.md).
//!
//! Coordinates: the screen height is 1.0 and the screen width is `aspect`
//! (width / height). The origin is the top-left corner, x grows to the right
//! and y grows downward. Each flyer is a square of side `size`, positioned by
//! its center.
//!
//! The world advances in fixed steps of [`TIME_STEP`]. Rendering interpolates
//! between the previous and the current step, so motion stays smooth at any
//! display refresh rate.

use std::time::Duration;

use crate::rng::Rng;

/// Length of one simulation step (about 1/120 s).
pub const TIME_STEP: Duration = Duration::from_nanos(8_333_333);

/// Longest elapsed time accepted by one [`World::advance`] call.
///
/// Longer gaps (a suspended machine, a debugger stop) are clamped so the
/// simulation never has to catch up with a huge number of steps.
pub const MAX_ELAPSED: Duration = Duration::from_millis(250);

/// Flyer side length in screen heights at `scale` 1.0.
const BASE_SIZE: f32 = 0.12;
/// Flyer speed in screen heights per second at `speed` 1.0.
const BASE_SPEED: f32 = 0.15;
/// Range of the random per-flyer speed factor.
const SPEED_FACTOR_RANGE: (f32, f32) = (0.8, 1.2);
/// Flight direction (left 2, down 1) normalized: (-2, 1) / sqrt(5).
const DIRECTION: (f32, f32) = (-0.894_427_2, 0.447_213_6);
/// Range of the random per-flyer door flap rate (flaps per second).
///
/// Opening the door shows the lit oven, so a flap may count as a flash.
/// Guideline G-3 allows at most three flashes a second; the rate stays well
/// below that (docs/design/visual-design.md).
const FLAP_RATE_RANGE: (f32, f32) = (1.2, 1.8);

/// Number of body colors. [`SpriteInstance::color`] is below this value.
pub const BODY_COLORS: u8 = 4;

/// Tunable simulation parameters.
///
/// `speed` and `scale` are multipliers. Callers pass finite, positive values;
/// validating them is the job of a later phase (`core::config`), so this
/// module does not check them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimParams {
    /// Number of flyers.
    pub count: u32,
    /// Speed multiplier (1.0 is the default speed).
    pub speed: f32,
    /// Size multiplier (1.0 is the default size).
    pub scale: f32,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            count: 12,
            speed: 1.0,
            scale: 1.0,
        }
    }
}

/// Where to draw one flyer, in the coordinates described in the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInstance {
    /// Center x position (screen heights from the left edge).
    pub x: f32,
    /// Center y position (screen heights from the top edge).
    pub y: f32,
    /// Side length of the square sprite (screen heights).
    pub size: f32,
    /// Door flap phase in `[0, 1)`: 0 is the start of a flap cycle.
    pub flap: f32,
    /// Body color, below [`BODY_COLORS`].
    pub color: u8,
}

/// One flyer: the previous and the current step position and flap phase,
/// its velocity, flap rate and body color.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flyer {
    prev_x: f32,
    prev_y: f32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    prev_flap: f32,
    /// Flap phase in `[0, 1)`.
    flap: f32,
    /// Flaps per second.
    flap_rate: f32,
    color: u8,
}

/// The simulated world: a set of flyers that cross the screen.
#[derive(Debug, Clone)]
pub struct World {
    rng: Rng,
    aspect: f32,
    size: f32,
    speed: f32,
    flyers: Vec<Flyer>,
    accumulator: Duration,
}

impl World {
    /// Creates a world of `params.count` flyers spread over the screen.
    ///
    /// `aspect` is the screen width divided by its height. A value that is not
    /// finite or not positive (for example from a minimized window with zero
    /// height) is replaced by 1.0.
    pub fn new(seed: u64, aspect: f32, params: SimParams) -> Self {
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let size = BASE_SIZE * params.scale;
        let speed = BASE_SPEED * params.speed;
        let mut rng = Rng::new(seed);
        let flyers = (0..params.count)
            .map(|_| {
                let x = rng.range_f32(0.0, aspect + size);
                let y = rng.range_f32(-size, 1.0);
                let (vx, vy) = random_velocity(&mut rng, speed);
                let (flap, flap_rate, color) = random_looks(&mut rng);
                Flyer {
                    prev_x: x,
                    prev_y: y,
                    x,
                    y,
                    vx,
                    vy,
                    prev_flap: flap,
                    flap,
                    flap_rate,
                    color,
                }
            })
            .collect();
        Self {
            rng,
            aspect,
            size,
            speed,
            flyers,
            accumulator: Duration::ZERO,
        }
    }

    /// Advances the simulation by `elapsed` wall-clock time.
    ///
    /// `elapsed` is clamped to [`MAX_ELAPSED`]. The simulation runs a whole
    /// number of [`TIME_STEP`]s; the remainder is kept for the next call.
    pub fn advance(&mut self, elapsed: Duration) {
        self.accumulator += elapsed.min(MAX_ELAPSED);
        while self.accumulator >= TIME_STEP {
            self.step();
            self.accumulator -= TIME_STEP;
        }
    }

    /// Returns the sprites to draw, interpolated between the last two steps.
    pub fn instances(&self) -> Vec<SpriteInstance> {
        let alpha = self.alpha();
        self.flyers
            .iter()
            .map(|f| SpriteInstance {
                x: f.prev_x + (f.x - f.prev_x) * alpha,
                y: f.prev_y + (f.y - f.prev_y) * alpha,
                size: self.size,
                flap: wrap_phase(f.prev_flap + forward(f.prev_flap, f.flap) * alpha),
                color: f.color,
            })
            .collect()
    }

    /// Interpolation factor in `[0, 1)`: how far the accumulator is into the next step.
    fn alpha(&self) -> f32 {
        (self.accumulator.as_secs_f64() / TIME_STEP.as_secs_f64()) as f32
    }

    /// Runs one fixed step: moves every flyer and respawns those that left the screen.
    fn step(&mut self) {
        let dt = TIME_STEP.as_secs_f32();
        let half = self.size / 2.0;
        for f in &mut self.flyers {
            f.prev_x = f.x;
            f.prev_y = f.y;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.prev_flap = f.flap;
            f.flap = wrap_phase(f.flap + f.flap_rate * dt);
            // Flyers leave through the left and bottom edges.
            if f.x + half < 0.0 || f.y - half > 1.0 {
                respawn(f, &mut self.rng, self.aspect, half, self.speed);
            }
        }
    }
}

/// Draws a velocity along [`DIRECTION`] with a random speed factor.
///
/// `speed` is the base speed in screen heights per second, already scaled by
/// the speed parameter.
fn random_velocity(rng: &mut Rng, speed: f32) -> (f32, f32) {
    let factor = rng.range_f32(SPEED_FACTOR_RANGE.0, SPEED_FACTOR_RANGE.1);
    let v = speed * factor;
    (DIRECTION.0 * v, DIRECTION.1 * v)
}

/// Draws the flap phase, the flap rate and the body color of a flyer.
fn random_looks(rng: &mut Rng) -> (f32, f32, u8) {
    let flap = rng.next_f32();
    let rate = rng.range_f32(FLAP_RATE_RANGE.0, FLAP_RATE_RANGE.1);
    // The top bits, as in `Rng::next_f32`; BODY_COLORS is far below 2^32.
    let color = ((rng.next_u64() >> 32) % u64::from(BODY_COLORS)) as u8;
    (flap, rate, color)
}

/// Wraps a phase into `[0, 1)`.
fn wrap_phase(phase: f32) -> f32 {
    let wrapped = phase - phase.floor();
    // Why: a tiny negative phase gives 1.0 after rounding; keep it below 1.
    if wrapped >= 1.0 { 0.0 } else { wrapped }
}

/// How far the phase moved forward from `from` to `to`, across the wrap.
fn forward(from: f32, to: f32) -> f32 {
    if to >= from {
        to - from
    } else {
        to + 1.0 - from
    }
}

/// Moves `flyer` to a new entry point on the top or right edge with a fresh
/// velocity.
///
/// `half` is half the flyer size. A sprite touches the screen while its center
/// is within x in [-half, aspect + half] and y in [-half, 1 + half]. The entry
/// line runs along the top of that area from x = -half to the corner
/// (aspect + half, -half), then down its right side to y = 1 + half. The two
/// parts meet at that corner, so every track whose sprite can touch the screen
/// is fed, corners included, and no empty band appears. The entry point is
/// drawn along the combined line, each part weighted by the flyer's motion
/// across it: the top part is crossed at rate 1 (down) and the right part at
/// rate 2 (left). This keeps the flyer density uniform over the screen.
fn respawn(flyer: &mut Flyer, rng: &mut Rng, aspect: f32, half: f32, speed: f32) {
    let top = aspect + 2.0 * half;
    let right = 1.0 + 2.0 * half;
    let u = rng.range_f32(0.0, top + 2.0 * right);
    let (x, y) = if u < top {
        (u - half, -half)
    } else {
        (aspect + half, (u - top) / 2.0 - half)
    };
    let (vx, vy) = random_velocity(rng, speed);
    let (flap, flap_rate, color) = random_looks(rng);
    *flyer = Flyer {
        // The previous position equals the new one so that interpolation
        // never draws a streak across the screen.
        prev_x: x,
        prev_y: y,
        x,
        y,
        vx,
        vy,
        prev_flap: flap,
        flap,
        flap_rate,
        color,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;
    const EPS: f32 = 1e-4;

    fn params(count: u32) -> SimParams {
        SimParams {
            count,
            ..SimParams::default()
        }
    }

    fn world(seed: u64) -> World {
        World::new(seed, ASPECT, SimParams::default())
    }

    fn speed_of(flyer: &Flyer) -> f32 {
        flyer.vx.hypot(flyer.vy)
    }

    /// A flyer at (x, y) with no streak and the velocity of the given speed factor.
    fn flyer_at(x: f32, y: f32, speed_factor: f32) -> Flyer {
        let speed = BASE_SPEED * speed_factor;
        Flyer {
            prev_x: x,
            prev_y: y,
            x,
            y,
            vx: DIRECTION.0 * speed,
            vy: DIRECTION.1 * speed,
            prev_flap: 0.0,
            flap: 0.0,
            flap_rate: 1.5,
            color: 0,
        }
    }

    /// A one-flyer world whose only flyer is `flyer`.
    fn world_with(seed: u64, flyer: Flyer) -> World {
        let mut w = World::new(seed, ASPECT, params(1));
        w.flyers[0] = flyer;
        w
    }

    #[test]
    fn default_params_are_neutral() {
        let p = SimParams::default();
        assert_eq!(p.count, 12);
        assert_eq!(p.speed, 1.0);
        assert_eq!(p.scale, 1.0);
    }

    #[test]
    fn direction_is_normalized_left_two_down_one() {
        let (dx, dy) = DIRECTION;
        assert!((dx.hypot(dy) - 1.0).abs() < 1e-6);
        assert!((dx / dy + 2.0).abs() < 1e-6);
    }

    #[test]
    fn count_gives_that_many_instances() {
        for count in [1, 5, 12, 100] {
            let w = World::new(1, ASPECT, params(count));
            assert_eq!(w.instances().len(), count as usize);
        }
    }

    #[test]
    fn count_zero_gives_no_instances() {
        let mut w = World::new(1, ASPECT, params(0));
        assert!(w.instances().is_empty());
        w.advance(Duration::from_secs(1));
        assert!(w.instances().is_empty());
    }

    #[test]
    fn initial_flyers_start_in_spawn_area_without_streak() {
        let w = World::new(3, ASPECT, params(200));
        for f in &w.flyers {
            assert!((0.0..ASPECT + w.size).contains(&f.x), "x = {}", f.x);
            assert!((-w.size..1.0).contains(&f.y), "y = {}", f.y);
            assert_eq!((f.prev_x, f.prev_y), (f.x, f.y));
        }
        // With a fresh accumulator the instance is exactly the position.
        for (f, i) in w.flyers.iter().zip(w.instances()) {
            assert_eq!((i.x, i.y), (f.x, f.y));
        }
    }

    #[test]
    fn velocity_points_left_and_down_within_speed_range() {
        let w = World::new(5, ASPECT, params(200));
        for f in &w.flyers {
            assert!(f.vx < 0.0 && f.vy > 0.0);
            assert!((f.vx / f.vy + 2.0).abs() < 1e-4);
            let s = speed_of(f);
            assert!((0.12 - 1e-6..=0.18 + 1e-6).contains(&s), "speed = {s}");
        }
    }

    #[test]
    fn params_scale_size_and_speed() {
        let p = SimParams {
            count: 50,
            speed: 3.0,
            scale: 2.0,
        };
        let w = World::new(5, ASPECT, p);
        for i in w.instances() {
            assert!((i.size - 0.24).abs() < 1e-6, "size = {}", i.size);
        }
        for f in &w.flyers {
            let s = speed_of(f);
            assert!((0.36 - 1e-5..=0.54 + 1e-5).contains(&s), "speed = {s}");
        }
    }

    #[test]
    fn advance_moves_every_flyer_left_and_down() {
        let mut w = world(7);
        let before = w.instances();
        w.advance(Duration::from_millis(100));
        let after = w.instances();

        // Why no flyer can respawn within 0.1 s of the initial area: the initial
        // x is at least 0, so x + size/2 starts at 0.06 or more, while the
        // fastest flyer covers at most 0.15 * 1.2 * 0.894 * 0.1 = 0.0161 in x.
        // The initial y is below 1.0, so y - size/2 starts below 0.94, and the
        // fastest flyer covers at most 0.15 * 1.2 * 0.447 * 0.1 = 0.0081 in y.
        // Neither reaches the "fully outside" test, so every flyer keeps its
        // identity and the displacement is a pure straight-line move.
        assert_eq!(before.len(), after.len());
        for (b, a) in before.iter().zip(&after) {
            let dx = a.x - b.x;
            let dy = a.y - b.y;
            assert!(dx < 0.0, "dx = {dx}");
            assert!(dy > 0.0, "dy = {dy}");
            assert!((dx / dy + 2.0).abs() < 1e-2, "dx/dy = {}", dx / dy);
        }
    }

    #[test]
    fn flyers_stay_on_screen_for_a_minute() {
        let mut w = world(11);
        let half = w.size / 2.0;
        let chunk = Duration::from_secs_f64(1.0 / 60.0);
        for n in 0..3600 {
            w.advance(chunk);
            let instances = w.instances();
            assert_eq!(instances.len(), 12);
            // Flyers start up to one size outside the right and top edges, so the
            // bounds are only guaranteed once the initial flyers have flown in.
            if n >= 120 {
                for i in &instances {
                    assert!(
                        (-half - EPS..=ASPECT + half + EPS).contains(&i.x),
                        "x = {}",
                        i.x
                    );
                    assert!(
                        (-half - EPS..=1.0 + half + EPS).contains(&i.y),
                        "y = {}",
                        i.y
                    );
                }
            }
        }
    }

    #[test]
    fn same_seed_and_same_advances_give_identical_instances() {
        let mut a = world(21);
        let mut b = world(21);
        for ms in [3, 16, 40, 7, 250, 1, 90] {
            a.advance(Duration::from_millis(ms));
            b.advance(Duration::from_millis(ms));
            assert_eq!(a.instances(), b.instances());
        }
    }

    #[test]
    fn different_seeds_give_different_instances() {
        let a = world(1);
        let b = world(2);
        assert_ne!(a.instances(), b.instances());
    }

    #[test]
    fn elapsed_is_clamped_to_max_elapsed() {
        let mut long = world(9);
        let mut max = world(9);
        long.advance(Duration::from_secs(10));
        max.advance(MAX_ELAPSED);
        assert_eq!(long.flyers, max.flyers);
        assert_eq!(long.accumulator, max.accumulator);
        assert_eq!(long.rng.next_u64(), max.rng.next_u64());
        assert_eq!(long.instances(), max.instances());
    }

    #[test]
    fn instances_interpolate_halfway_after_one_and_a_half_steps() {
        let mut w = world(13);
        w.advance(TIME_STEP + TIME_STEP / 2);
        let instances = w.instances();
        assert_eq!(instances.len(), w.flyers.len());
        for (f, i) in w.flyers.iter().zip(&instances) {
            // One step ran, so the previous and current positions differ.
            assert_ne!((f.prev_x, f.prev_y), (f.x, f.y));
            assert!((i.x - (f.prev_x + f.x) / 2.0).abs() < 1e-6);
            assert!((i.y - (f.prev_y + f.y) / 2.0).abs() < 1e-6);
            assert_eq!(i.size, w.size);
        }
    }

    #[test]
    fn interpolation_factor_stays_below_one() {
        let mut w = world(13);
        w.advance(TIME_STEP - Duration::from_nanos(1));
        assert!(w.accumulator < TIME_STEP);
        let alpha = w.alpha();
        assert!((0.0..1.0).contains(&alpha), "alpha = {alpha}");
        assert!(alpha > 0.999);
    }

    #[test]
    fn advance_runs_exactly_three_steps_for_three_time_steps() {
        let mut w = world(17);
        let start = w.flyers.clone();
        w.advance(TIME_STEP * 3);
        assert_eq!(w.accumulator, Duration::ZERO);
        let dt = TIME_STEP.as_secs_f32();
        for (s, f) in start.iter().zip(&w.flyers) {
            // No respawn within three steps of the initial area, so the move is linear.
            assert!((f.x - (s.x + 3.0 * s.vx * dt)).abs() < 1e-6);
            assert!((f.y - (s.y + 3.0 * s.vy * dt)).abs() < 1e-6);
            assert!((f.prev_x - (s.x + 2.0 * s.vx * dt)).abs() < 1e-6);
            assert!((f.prev_y - (s.y + 2.0 * s.vy * dt)).abs() < 1e-6);
        }
    }

    #[test]
    fn remainder_below_one_step_is_carried_over() {
        let mut w = world(17);
        let start = w.flyers.clone();
        w.advance(TIME_STEP - Duration::from_nanos(1));
        assert_eq!(w.flyers, start);
        assert_eq!(w.accumulator, TIME_STEP - Duration::from_nanos(1));
        w.advance(Duration::from_nanos(6));
        assert_eq!(w.accumulator, Duration::from_nanos(5));
        assert_ne!(w.flyers, start);
    }

    #[test]
    fn short_advances_accumulate_into_steps() {
        let mut w = world(19);
        w.advance(Duration::from_millis(5));
        w.advance(Duration::from_millis(5));
        // 10 ms is one 8.333333 ms step plus a remainder.
        assert_eq!(w.accumulator, Duration::from_millis(10) - TIME_STEP);
    }

    #[test]
    fn flyer_that_is_not_fully_outside_is_kept() {
        let half = BASE_SIZE / 2.0;
        // Still overlaps the left edge by 0.01 after the step (a step moves < 0.0016).
        let mut w = world_with(1, flyer_at(-half + 0.01, 0.5, 1.0));
        let before = w.flyers[0];
        w.advance(TIME_STEP);
        let f = w.flyers[0];
        let dt = TIME_STEP.as_secs_f32();
        assert_eq!((f.prev_x, f.prev_y), (before.x, before.y));
        assert!((f.x - (before.x + before.vx * dt)).abs() < 1e-7);
        assert_eq!((f.vx, f.vy), (before.vx, before.vy));
    }

    #[test]
    fn respawn_enters_at_top_or_right_without_streak() {
        let half = BASE_SIZE / 2.0;
        let (mut tops, mut rights) = (0, 0);
        for seed in 0..64 {
            // Just inside the left edge: the first step takes it fully outside.
            // The starting factor 0.5 is below the respawn range 0.8..1.2, so an
            // in-range speed afterwards proves that a new factor was drawn.
            let exits = [
                flyer_at(-half + 1e-4, 0.5, 0.5),
                // Just inside the bottom edge.
                flyer_at(0.5, 1.0 + half - 1e-4, 0.5),
            ];
            for exiting in exits {
                for quarter in 0..4 {
                    let mut w = world_with(seed, exiting);
                    // What the respawn must draw, in order: u, then the speed factor.
                    let mut probe = w.rng.clone();
                    let top = ASPECT + 2.0 * half;
                    let u = probe.range_f32(0.0, top + 2.0 * (1.0 + 2.0 * half));
                    let factor = probe.range_f32(0.8, 1.2);

                    // One step runs, leaving quarter/4 of a step in the accumulator.
                    w.advance(TIME_STEP + TIME_STEP * quarter / 4);

                    let f = w.flyers[0];
                    let (want_x, want_y) = if u < top {
                        tops += 1;
                        (u - half, -half)
                    } else {
                        rights += 1;
                        (ASPECT + half, (u - top) / 2.0 - half)
                    };
                    assert_eq!((f.x, f.y), (want_x, want_y));
                    assert_eq!((f.prev_x, f.prev_y), (f.x, f.y), "no streak");
                    assert!((f.vx - DIRECTION.0 * BASE_SPEED * factor).abs() < 1e-7);
                    assert!((f.vy - DIRECTION.1 * BASE_SPEED * factor).abs() < 1e-7);

                    // The drawn instance is the spawn point at every interpolation factor.
                    let i = w.instances()[0];
                    assert_eq!((i.x, i.y), (f.x, f.y));
                }
            }
        }
        assert!(tops > 0 && rights > 0, "tops = {tops}, rights = {rights}");
    }

    #[test]
    fn respawn_tracks_cover_the_screen_without_gaps_at_uniform_density() {
        // Along the flight direction (-2, 1) the value y + x / 2 stays constant,
        // so it names the track a flyer follows. Entry points must cover every
        // track from the top-left to the bottom-right corner evenly; a gap would
        // leave an empty band crossing the screen.
        let half = BASE_SIZE / 2.0;
        let mut rng = Rng::new(7);
        let mut tracks: Vec<f32> = (0..20_000)
            .map(|_| {
                let mut f = flyer_at(0.0, 0.0, 1.0);
                respawn(&mut f, &mut rng, ASPECT, half, BASE_SPEED);
                f.y + f.x / 2.0
            })
            .collect();
        tracks.sort_by(f32::total_cmp);

        let widest_gap = tracks.windows(2).map(|w| w[1] - w[0]).fold(0.0, f32::max);
        assert!(widest_gap < 0.01, "widest gap between tracks: {widest_gap}");

        // The tracks span from the top-left corner to the bottom-right corner.
        let (first, last) = (tracks[0], tracks[tracks.len() - 1]);
        // Every track whose sprite can touch the screen: centers range over
        // x in [-half, ASPECT + half] and y in [-half, 1 + half].
        let (low, high) = (-1.5 * half, 1.0 + half + (ASPECT + half) / 2.0);
        assert!(
            (first - low).abs() < 0.01,
            "first track {first}, expected {low}"
        );
        assert!(
            (last - high).abs() < 0.01,
            "last track {last}, expected {high}"
        );

        // Uniform density: every tenth of the span gets about a tenth of the flyers.
        let mut bins = [0_u32; 10];
        for t in &tracks {
            let i = (((t - low) / (high - low)) * 10.0) as usize;
            bins[i.min(9)] += 1;
        }
        for (i, &n) in bins.iter().enumerate() {
            assert!(
                (1800..=2200).contains(&n),
                "bin {i} has {n} of 20000: {bins:?}"
            );
        }
    }

    #[test]
    fn invalid_aspect_gives_finite_instances() {
        for aspect in [0.0, -1.0, -0.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut w = World::new(23, aspect, SimParams::default());
            assert_eq!(w.aspect, 1.0, "aspect = {aspect}");
            for _ in 0..200 {
                w.advance(Duration::from_millis(16));
            }
            for i in w.instances() {
                assert!(i.x.is_finite() && i.y.is_finite() && i.size.is_finite());
            }
        }
    }

    #[test]
    fn instances_have_a_flap_phase_and_a_body_color() {
        let mut w = World::new(29, ASPECT, params(200));
        for _ in 0..50 {
            w.advance(Duration::from_millis(37));
            for i in w.instances() {
                assert!((0.0..1.0).contains(&i.flap), "flap = {}", i.flap);
                assert!(i.color < BODY_COLORS, "color = {}", i.color);
            }
        }
    }

    #[test]
    fn every_body_color_is_used() {
        let w = World::new(31, ASPECT, params(200));
        let mut seen = [false; BODY_COLORS as usize];
        for i in w.instances() {
            seen[usize::from(i.color)] = true;
        }
        assert!(seen.iter().all(|&s| s), "colors seen: {seen:?}");
    }

    #[test]
    fn flap_rate_stays_well_below_three_per_second() {
        // Guideline G-3: nothing may flash more than three times a second.
        // Opening the door shows the lit oven, so one flap may count as one flash.
        let w = World::new(37, ASPECT, params(500));
        for f in &w.flyers {
            assert!(
                (FLAP_RATE_RANGE.0..FLAP_RATE_RANGE.1).contains(&f.flap_rate),
                "rate = {}",
                f.flap_rate
            );
        }
        assert!(FLAP_RATE_RANGE.0 > 0.0 && FLAP_RATE_RANGE.1 <= 2.0);
    }

    #[test]
    fn flap_advances_by_its_rate_and_wraps() {
        let mut f = flyer_at(0.5, 0.5, 1.0);
        f.flap_rate = 1.5;
        f.prev_flap = 0.995;
        f.flap = 0.995;
        let mut w = world_with(1, f);
        w.advance(TIME_STEP);
        let f = w.flyers[0];
        let dt = TIME_STEP.as_secs_f32();
        assert_eq!(f.prev_flap, 0.995);
        // 0.995 + 1.5 * 0.00833 = 1.0075, wrapped to 0.0075.
        assert!(
            (f.flap - (0.995 + 1.5 * dt - 1.0)).abs() < 1e-6,
            "{}",
            f.flap
        );
    }

    #[test]
    fn flap_interpolates_across_the_wrap() {
        let mut f = flyer_at(0.5, 0.5, 1.0);
        f.flap_rate = 1.5;
        f.prev_flap = 0.99;
        f.flap = 0.99;
        let mut w = world_with(1, f);
        // One step from 0.99 to about 0.0025, then half a step more.
        w.advance(TIME_STEP + TIME_STEP / 2);
        let f = w.flyers[0];
        let i = w.instances()[0];
        let halfway = f.prev_flap + (f.flap + 1.0 - f.prev_flap) / 2.0;
        let want = halfway - halfway.floor();
        assert!(
            (i.flap - want).abs() < 1e-4,
            "flap = {}, want {want}",
            i.flap
        );
        assert!((0.0..1.0).contains(&i.flap));
    }

    #[test]
    fn respawn_resets_the_flap_without_a_jump() {
        let half = BASE_SIZE / 2.0;
        for seed in 0..32 {
            let mut w = world_with(seed, flyer_at(-half + 1e-4, 0.5, 0.5));
            w.advance(TIME_STEP + TIME_STEP / 2);
            let f = w.flyers[0];
            assert_eq!(f.prev_flap, f.flap, "no jump after a respawn");
            assert_eq!(w.instances()[0].flap, f.flap);
            assert!(f.color < BODY_COLORS);
        }
    }

    #[test]
    fn valid_aspect_is_kept() {
        assert_eq!(World::new(1, ASPECT, params(1)).aspect, ASPECT);
    }
}
