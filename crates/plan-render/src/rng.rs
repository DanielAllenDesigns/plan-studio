//! PCG32 random numbers with deterministic per-sample seeding.

const MULT: u64 = 6_364_136_223_846_793_005;
const INC: u64 = 1_442_695_040_888_963_407;

/// SplitMix64 finaliser, used to decorrelate (seed, pixel, sample) triples.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// PCG-XSH-RR 64/32 generator.
#[derive(Clone, Debug)]
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut rng = Rng { state: 0 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    /// Generator for one pixel sample; independent of thread scheduling.
    pub fn for_sample(seed: u64, pixel: u64, sample: u32) -> Rng {
        let h = splitmix64(seed ^ splitmix64(pixel));
        Rng::new(splitmix64(
            h ^ (u64::from(sample) << 20) ^ u64::from(sample),
        ))
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULT).wrapping_add(INC);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform float in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_are_in_unit_interval_and_repeatable() {
        let mut a = Rng::for_sample(7, 3, 9);
        let mut b = Rng::for_sample(7, 3, 9);
        for _ in 0..1000 {
            let x = a.next_f32();
            assert!((0.0..1.0).contains(&x));
            assert_eq!(x, b.next_f32());
        }
        assert_ne!(
            Rng::for_sample(7, 3, 9).next_u32(),
            Rng::for_sample(7, 3, 10).next_u32()
        );
    }
}
