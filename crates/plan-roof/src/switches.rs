//! The Build Roof switches beyond the pitch and overhang defaults (manual pp.
//! 826 to 829; RF-66..RF-72): what a rebuild keeps, whether the roof is
//! built from the walls or from Roof Baseline Polylines, how pitch is shown,
//! and how curved walls are roofed.

use serde::{Deserialize, Serialize};
use std::cell::Cell;

/// Segment Angle at Curved Wall: smallest and largest increment, degrees
/// (manual p. 826).
pub const SEGMENT_ANGLE_RANGE: (f64, f64) = (6.0, 90.0);
/// Segment angle a new roof starts with, degrees.
pub const DEFAULT_SEGMENT_ANGLE: f64 = 15.0;
/// Minimum Alcove Size a new roof starts with, inches.
pub const DEFAULT_MIN_ALCOVE: f64 = 24.0;
/// Pitch in Degrees accepts values between these, degrees (manual p. 829).
pub const PITCH_DEGREES_RANGE: (f64, f64) = (-89.0, 89.0);

/// The Build Roof dialog's switches that are not the pitch and overhang
/// defaults. Kept with the roof settings; all of them load with the stock
/// values when a stored roof lacks them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildSwitches {
    /// Retain Manually Drawn Roof Planes: a rebuild keeps planes drawn by
    /// hand. Off, a rebuild replaces them.
    pub retain_manual: bool,
    /// Retain Edited Automatic Roof Planes: a rebuild keeps automatic planes
    /// the user moved or re-pitched. Off, a rebuild replaces them.
    pub retain_edited: bool,
    /// Make Roof Baseline Polylines: delete the roof and make baseline
    /// polylines from the exterior walls instead of planes.
    pub make_baselines: bool,
    /// Use Existing Roof Baselines: build the planes from the baseline
    /// polylines instead of the exterior walls.
    pub use_existing_baselines: bool,
    /// Pitch in Degrees: dialogs and plane labels show degrees.
    pub pitch_in_degrees: bool,
    /// Segment Angle at Curved Wall, degrees (6 to 90).
    pub segment_angle: f64,
    /// Concave curved walls are roofed section by section only when the
    /// sections' baselines are longer than this, inches.
    pub min_alcove: f64,
    /// Show All Ridges: a line along each hip between the roof planes over a
    /// curved wall shows in vector views (3D Display; on by default).
    pub show_all_ridges: bool,
}

impl Default for BuildSwitches {
    fn default() -> Self {
        Self {
            // A rebuild has always kept both kinds (RF-6, RF-37).
            retain_manual: true,
            retain_edited: true,
            make_baselines: false,
            use_existing_baselines: false,
            pitch_in_degrees: false,
            segment_angle: DEFAULT_SEGMENT_ANGLE,
            min_alcove: DEFAULT_MIN_ALCOVE,
            show_all_ridges: true,
        }
    }
}

impl BuildSwitches {
    /// The segment angle limited to 6 through 90 degrees.
    pub fn segment_angle_clamped(&self) -> f64 {
        clamp_segment_angle(self.segment_angle)
    }

    /// These switches with every value brought into its legal range. The
    /// two baseline switches exclude Build Roof Planes in the dialog; here
    /// Make Roof Baseline Polylines wins when both are set.
    pub fn normalized(&self) -> Self {
        Self {
            segment_angle: self.segment_angle_clamped(),
            min_alcove: self.min_alcove.max(0.0),
            use_existing_baselines: self.use_existing_baselines && !self.make_baselines,
            ..self.clone()
        }
    }
}

/// `angle` limited to the Segment Angle at Curved Wall range.
pub fn clamp_segment_angle(angle: f64) -> f64 {
    if angle.is_finite() {
        angle.clamp(SEGMENT_ANGLE_RANGE.0, SEGMENT_ANGLE_RANGE.1)
    } else {
        DEFAULT_SEGMENT_ANGLE
    }
}

/// Pitch as rise per 12 to degrees: 12 in 12 is 45 degrees.
pub fn pitch_to_degrees(rise_in_12: f64) -> f64 {
    (rise_in_12 / 12.0).atan().to_degrees()
}

/// Degrees to rise per 12, after limiting `degrees` to -89 through 89.
pub fn degrees_to_pitch(degrees: f64) -> f64 {
    let d = degrees.clamp(PITCH_DEGREES_RANGE.0, PITCH_DEGREES_RANGE.1);
    12.0 * d.to_radians().tan()
}

thread_local! {
    static PITCH_DEGREES: Cell<bool> = const { Cell::new(false) };
}

/// Does the open plan show pitch in degrees (Pitch in Degrees)? The
/// application sets it when it loads a roof's settings; dialogs and labels
/// read it. Per thread, so tests do not disturb each other.
pub fn pitch_display_degrees() -> bool {
    PITCH_DEGREES.with(Cell::get)
}

/// Sets what [`pitch_display_degrees`] reports.
pub fn set_pitch_display_degrees(on: bool) {
    PITCH_DEGREES.with(|c| c.set(on));
}

/// A pitch as dialogs and labels show it: `8:12`, or `33.7°` with Pitch in
/// Degrees.
pub fn pitch_text(rise_in_12: f64) -> String {
    if pitch_display_degrees() {
        let d = pitch_to_degrees(rise_in_12);
        if (d - d.round()).abs() < 0.05 {
            format!("{}\u{b0}", d.round() as i64)
        } else {
            format!("{d:.1}\u{b0}")
        }
    } else if (rise_in_12 - rise_in_12.round()).abs() < 0.05 {
        format!("{}:12", rise_in_12.round() as i64)
    } else {
        format!("{rise_in_12:.1}:12")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_segment_angle_stays_between_6_and_90_degrees() {
        assert_eq!(clamp_segment_angle(2.0), 6.0);
        assert_eq!(clamp_segment_angle(120.0), 90.0);
        assert_eq!(clamp_segment_angle(30.0), 30.0);
        assert_eq!(clamp_segment_angle(f64::NAN), DEFAULT_SEGMENT_ANGLE);
    }

    #[test]
    fn pitch_converts_both_ways_and_limits_to_89_degrees() {
        assert!((pitch_to_degrees(12.0) - 45.0).abs() < 1e-9);
        assert!((degrees_to_pitch(45.0) - 12.0).abs() < 1e-9);
        // 8 in 12 is 33.6901 degrees (manual p. 857).
        assert!((pitch_to_degrees(8.0) - 33.690_067).abs() < 1e-5);
        let steep = degrees_to_pitch(100.0);
        assert!((pitch_to_degrees(steep) - 89.0).abs() < 1e-9);
        assert!(degrees_to_pitch(-100.0) < 0.0);
    }

    #[test]
    fn the_label_follows_the_degrees_switch_on_this_thread() {
        assert_eq!(pitch_text(8.0), "8:12");
        set_pitch_display_degrees(true);
        assert_eq!(pitch_text(12.0), "45\u{b0}");
        assert_eq!(pitch_text(8.0), "33.7\u{b0}");
        set_pitch_display_degrees(false);
        assert_eq!(pitch_text(8.5), "8.5:12");
    }

    #[test]
    fn make_baselines_excludes_using_them() {
        let s = BuildSwitches {
            make_baselines: true,
            use_existing_baselines: true,
            segment_angle: 1.0,
            ..BuildSwitches::default()
        };
        let n = s.normalized();
        assert!(!n.use_existing_baselines);
        assert_eq!(n.segment_angle, 6.0);
    }

    #[test]
    fn a_stored_roof_without_the_switches_keeps_both_retain_boxes_on() {
        let s: BuildSwitches = serde_json::from_str("{}").unwrap();
        assert!(s.retain_manual && s.retain_edited);
        assert_eq!(s.segment_angle, DEFAULT_SEGMENT_ANGLE);
    }
}
