//! HDR to display conversion.

use serde::{Deserialize, Serialize};

/// Tone-mapping operator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMap {
    /// Narkowicz ACES filmic fit.
    #[default]
    Aces,
    /// `x / (1 + x)`.
    Reinhard,
    /// Clamp to `[0, 1]`.
    Linear,
}

impl ToneMap {
    fn curve(self, x: f32) -> f32 {
        match self {
            ToneMap::Aces => (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14),
            ToneMap::Reinhard => x / (1.0 + x),
            ToneMap::Linear => x,
        }
        .clamp(0.0, 1.0)
    }

    /// Map linear radiance (scaled by `exposure`) to display-linear `[0, 1]`.
    ///
    /// NaN and negative inputs map to `0`; infinities saturate.
    pub fn apply(self, rgb: [f32; 3], exposure: f32) -> [f32; 3] {
        rgb.map(|c| {
            let x = c * exposure;
            if x.is_nan() {
                0.0
            } else {
                self.curve(x.clamp(0.0, 1.0e6))
            }
        })
    }
}

/// Linear `[0, 1]` to an sRGB-encoded byte.
pub fn srgb_byte(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let encoded = if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_stay_in_display_range() {
        let inputs = [
            f32::NAN,
            f32::NEG_INFINITY,
            -5.0,
            0.0,
            0.18,
            1.0,
            50.0,
            f32::INFINITY,
        ];
        for tm in [ToneMap::Aces, ToneMap::Reinhard, ToneMap::Linear] {
            for &x in &inputs {
                for exposure in [0.0, 1.0, 8.0] {
                    for v in tm.apply([x, x, x], exposure) {
                        assert!((0.0..=1.0).contains(&v), "{tm:?}({x}) = {v}");
                        let b = srgb_byte(v);
                        assert!(u32::from(b) <= 255);
                    }
                }
            }
        }
        assert_eq!(srgb_byte(0.0), 0);
        assert_eq!(srgb_byte(1.0), 255);
        assert_eq!(srgb_byte(f32::NAN.max(2.0)), 255);
    }

    #[test]
    fn curves_are_monotonic() {
        for tm in [ToneMap::Aces, ToneMap::Reinhard, ToneMap::Linear] {
            let mut last = -1.0;
            for i in 0..200 {
                let v = tm.apply([i as f32 * 0.05; 3], 1.0)[0];
                assert!(v >= last);
                last = v;
            }
        }
    }
}
