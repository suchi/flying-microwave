//! Small deterministic pseudo-random number generator (SplitMix64).
//!
//! The generator is not cryptographically secure. It only drives the visual
//! variation of the simulation, and the same seed always produces the same
//! sequence on every platform.

/// SplitMix64 pseudo-random number generator.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a generator from `seed`. Any seed value is valid.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 random bits (SplitMix64).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Returns a uniformly distributed value in `[0, 1)`.
    ///
    /// Only the top 24 bits are used, so every result is exactly
    /// representable as an `f32` and the upper bound is never reached.
    pub fn next_f32(&mut self) -> f32 {
        // 2^-24: a 24-bit integer times this is exact in f32.
        const SCALE: f32 = 1.0 / 16_777_216.0;
        (self.next_u64() >> 40) as f32 * SCALE
    }

    /// Returns a uniformly distributed value in `[low, high)`.
    ///
    /// `low` must not be greater than `high`; both must be finite.
    pub fn range_f32(&mut self, low: f32, high: f32) -> f32 {
        debug_assert!(low <= high, "empty range: {low} > {high}");
        low + (high - low) * self.next_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_splitmix64_reference_vector() {
        // Reference values from the SplitMix64 reference implementation, as used by
        // the test in rand_xoshiro's splitmix64.rs.
        let mut rng = Rng::new(1_477_776_061_723_855_037);
        let expected: [u64; 5] = [
            1_985_237_415_132_408_290,
            2_979_275_885_539_914_483,
            13_511_426_838_097_143_398,
            8_488_337_342_461_049_707,
            15_141_737_807_933_549_159,
        ];
        for want in expected {
            assert_eq!(rng.next_u64(), want);
        }
    }

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn clone_continues_independently_from_same_state() {
        let mut a = Rng::new(7);
        a.next_u64();
        let mut b = a.clone();
        assert_eq!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn next_f32_uses_top_24_bits() {
        let mut reference = Rng::new(99);
        let mut rng = Rng::new(99);
        for _ in 0..100 {
            let bits = reference.next_u64() >> 40;
            assert_eq!(rng.next_f32(), bits as f32 * (1.0 / 16_777_216.0));
        }
    }

    #[test]
    fn next_f32_stays_in_unit_interval() {
        let mut rng = Rng::new(0);
        for _ in 0..100_000 {
            let x = rng.next_f32();
            assert!((0.0..1.0).contains(&x), "out of range: {x}");
        }
    }

    #[test]
    fn range_f32_stays_in_range() {
        let mut rng = Rng::new(0);
        for _ in 0..100_000 {
            let x = rng.range_f32(-2.5, 3.5);
            assert!((-2.5..3.5).contains(&x), "out of range: {x}");
        }
    }

    #[test]
    fn range_f32_with_equal_bounds_returns_that_value() {
        let mut rng = Rng::new(5);
        assert_eq!(rng.range_f32(1.5, 1.5), 1.5);
    }
}
