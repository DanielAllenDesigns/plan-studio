//! Roof heights and eave alignment (manual pp. 829, 830, 840, 841, 844;
//! RF-73..RF-77, RF-103..RF-105, RF-112, Round 16 brief 18).
//!
//! Two parts, both pure arithmetic:
//!
//! * [`HeightSettings`]: the Build Roof "Roof Height" switches (framing
//!   method, Heel Height, Automatic Birdsmouth Cut, Raise Off Plate /
//!   Birdsmouth Cut, Same Roof Height at Exterior Walls, Same Height Eaves,
//!   Allow Low Roof Planes) and the overhang and baseline rules they imply.
//! * [`PlaneHeights`]: the four heights a Roof Plane Specification shows
//!   (Ridge Top, Baseline, Fascia Top, Top of Plate) and how a pitch or
//!   height edit moves them under each lock.
//!
//! Lengths are inches, pitch is rise per 12 of run.

use serde::{Deserialize, Serialize};

/// Overhangs below this are treated as none.
const EPS: f64 = 1e-6;
/// A raise of at least this much lifts a rafter clear of the plate, so there
/// is no birdsmouth (manual p. 841: 1/16 inch).
pub const NO_BIRDSMOUTH_RAISE: f64 = 1.0 / 16.0;

/// Slope as a fraction (rise over run).
fn slope(pitch_in_12: f64) -> f64 {
    pitch_in_12 / 12.0
}

/// Length along the slope of a run measured in plan (projected length).
pub fn actual_length(projected: f64, pitch_in_12: f64) -> f64 {
    projected * (1.0 + slope(pitch_in_12).powi(2)).sqrt()
}

/// Plan length of an edge measured along the slope.
pub fn projected_length(actual: f64, pitch_in_12: f64) -> f64 {
    actual / (1.0 + slope(pitch_in_12).powi(2)).sqrt()
}

/// Vertical depth of a structure of perpendicular `thickness` at `pitch`.
pub fn vertical_structure_depth(thickness: f64, pitch_in_12: f64) -> f64 {
    actual_length(thickness, pitch_in_12)
}

/// Birdsmouth cut (vertical depth) for a seat (horizontal depth) at `pitch`.
pub fn birdsmouth_cut_for_seat(seat: f64, pitch_in_12: f64) -> f64 {
    seat * slope(pitch_in_12)
}

/// Birdsmouth seat (horizontal depth) for a vertical `cut` at `pitch`; zero
/// for a flat roof.
pub fn birdsmouth_seat_for_cut(cut: f64, pitch_in_12: f64) -> f64 {
    let k = slope(pitch_in_12);
    if k.abs() < EPS {
        0.0
    } else {
        cut / k
    }
}

/// How the roof is framed; sets which Roof Height fields apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoofFraming {
    #[default]
    Rafters,
    Trusses,
}

/// The Build Roof dialog's Roof Height group (manual pp. 829 and 830).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeightSettings {
    /// Framing Method: Heel Height needs trusses, the birdsmouth rafters.
    pub framing: RoofFraming,
    /// Heel Height: how far trusses are raised off the top plates for an
    /// energy heel. Only counts with trusses.
    pub heel_height: f64,
    /// Automatic Birdsmouth Cut: the cut follows the pitch and the seat is
    /// the plate. Off, [`HeightSettings::birdsmouth_cut`] is used.
    pub auto_birdsmouth: bool,
    /// Raise Off Plate / Birdsmouth Cut when the automatic cut is off:
    /// positive raises the roof (attic knee walls), negative sinks it into
    /// a birdsmouth of that depth. Rafters only.
    pub birdsmouth_cut: f64,
    /// Same Roof Height at Exterior Walls: bearing walls stay the same
    /// height and overhangs change so the eaves meet.
    pub same_roof_height: bool,
    /// Same Height Eaves: every plane's eave is at the height of a plane
    /// with the default pitch and overhang; planes are raised or lowered.
    pub same_height_eaves: bool,
    /// Allow Low Roof Planes. Turn off only when an upper floor overhangs
    /// the roof below. Stored; the roof builder does not yet differ
    /// (DECISIONS RH4).
    pub allow_low_planes: bool,
}

impl Default for HeightSettings {
    fn default() -> Self {
        Self {
            framing: RoofFraming::Rafters,
            heel_height: 0.0,
            auto_birdsmouth: true,
            birdsmouth_cut: 0.0,
            same_roof_height: true,
            same_height_eaves: false,
            allow_low_planes: true,
        }
    }
}

impl HeightSettings {
    /// What this group adds to the roof's height over the top plates:
    /// the Heel Height for trusses, the Raise Off Plate / Birdsmouth Cut
    /// for rafters whose automatic cut is off.
    pub fn plate_lift(&self) -> f64 {
        match self.framing {
            RoofFraming::Trusses => self.heel_height.max(0.0),
            RoofFraming::Rafters if !self.auto_birdsmouth => self.birdsmouth_cut,
            RoofFraming::Rafters => 0.0,
        }
    }

    /// Does a rafter at `raise` over the plate (Raise/Lower All Roof Planes
    /// plus [`HeightSettings::plate_lift`]) have a birdsmouth? Trusses and a
    /// raise of 1/16 inch or more have none (manual p. 841).
    pub fn has_birdsmouth(&self, raise: f64) -> bool {
        self.framing == RoofFraming::Rafters && raise < NO_BIRDSMOUTH_RAISE
    }

    /// True when planes are placed so every eave meets at one height rather
    /// than every bearing wall at one height: Same Height Eaves, or neither
    /// switch (the builder always joins neighbouring planes, DECISIONS RH2).
    pub fn eaves_at_default_height(&self) -> bool {
        self.same_height_eaves || !self.same_roof_height
    }

    /// The overhang (horizontal, from the wall face) a plane of `pitch` gets
    /// so its eave meets one built with the defaults, or `overhang` when
    /// the switches leave it alone. `independent` planes do not meet a
    /// plane of another pitch; they keep their overhang unless both
    /// switches are on (manual p. 844).
    pub fn eave_overhang(
        &self,
        default_pitch: f64,
        default_overhang: f64,
        pitch: f64,
        overhang: f64,
        independent: bool,
    ) -> f64 {
        let aligns = self.same_roof_height && (!independent || self.same_height_eaves);
        if !aligns || pitch.abs() < EPS || default_pitch.abs() < EPS {
            return overhang;
        }
        default_overhang * default_pitch / pitch
    }
}

/// For each edge: does its plane meet no plane of another pitch? `pitches`
/// has one entry per footprint edge, `None` for an edge with no plane of its
/// own (gable, high shed). An edge is independent when every plane of the
/// roof has its pitch.
pub fn independent_edges(pitches: &[Option<f64>]) -> Vec<bool> {
    pitches
        .iter()
        .map(|p| match p {
            None => true,
            Some(p) => pitches.iter().flatten().all(|q| (q - p).abs() < 1e-6),
        })
        .collect()
}

/// The four heights of a roof plane and the facts they hang on. Heights are
/// elevations; `run` is the plan distance from the baseline to the ridge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneHeights {
    /// Elevation of the plane's top surface on its baseline.
    pub baseline: f64,
    pub pitch: f64,
    /// Plan distance from the baseline up the slope to the ridge.
    pub run: f64,
    /// Plan distance from the baseline out to the fascia.
    pub overhang: f64,
    /// Structure thickness measured square to the slope.
    pub thickness: f64,
    /// Top of the bearing wall's top plate.
    pub plate_top: f64,
    /// Width of the plate, the birdsmouth seat of the automatic cut.
    pub plate_width: f64,
}

/// Which height a pitch change keeps fixed, or a height edit sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightLock {
    RidgeTop,
    Baseline,
    FasciaTop,
    TopOfPlate,
}

impl PlaneHeights {
    pub fn vertical_depth(&self) -> f64 {
        vertical_structure_depth(self.thickness, self.pitch)
    }

    pub fn ridge_top(&self) -> f64 {
        self.baseline + self.run * slope(self.pitch)
    }

    pub fn fascia_top(&self) -> f64 {
        self.baseline - self.overhang * slope(self.pitch)
    }

    /// Elevation of the structure's underside on the baseline.
    pub fn underside_at_baseline(&self) -> f64 {
        self.baseline - self.vertical_depth()
    }

    /// Birdsmouth depth: how far the underside at the wall face is below
    /// the plate top. Negative is a raise off the plate.
    pub fn birdsmouth_depth(&self) -> f64 {
        self.plate_top - self.underside_at_baseline()
    }

    /// Horizontal seat of the birdsmouth.
    pub fn birdsmouth_seat(&self) -> f64 {
        birdsmouth_seat_for_cut(self.birdsmouth_depth(), self.pitch)
    }

    /// The height under `lock`.
    pub fn height(&self, lock: HeightLock) -> f64 {
        match lock {
            HeightLock::RidgeTop => self.ridge_top(),
            HeightLock::Baseline => self.baseline,
            HeightLock::FasciaTop => self.fascia_top(),
            HeightLock::TopOfPlate => self.plate_top,
        }
    }

    /// The plane raised or lowered with its pitch locked so that `lock`
    /// reads `value`. The Top of Plate entry is the plate the plane bears
    /// on, so setting it leaves the plane where it is and the birdsmouth
    /// depth takes the change.
    pub fn with_height(&self, lock: HeightLock, value: f64) -> Self {
        let k = slope(self.pitch);
        let mut out = *self;
        match lock {
            HeightLock::RidgeTop => out.baseline = value - self.run * k,
            HeightLock::Baseline => out.baseline = value,
            HeightLock::FasciaTop => out.baseline = value + self.overhang * k,
            HeightLock::TopOfPlate => out.plate_top = value,
        }
        out
    }

    /// The plane after its pitch changes to `new_pitch` while `lock` stays
    /// fixed (manual p. 840). The plane pivots about the locked height's
    /// plan position: the ridge, the baseline, the fascia, or for Top of
    /// Plate the inside top edge of the plate with the automatic birdsmouth
    /// and the outside top edge without it.
    pub fn with_pitch(&self, new_pitch: f64, lock: HeightLock, auto_birdsmouth: bool) -> Self {
        let k1 = slope(new_pitch);
        let mut out = *self;
        out.pitch = new_pitch;
        out.baseline = match lock {
            HeightLock::RidgeTop => self.ridge_top() - self.run * k1,
            HeightLock::Baseline => self.baseline,
            HeightLock::FasciaTop => self.fascia_top() + self.overhang * k1,
            HeightLock::TopOfPlate => {
                let v1 = out.vertical_depth();
                if auto_birdsmouth {
                    // The underside rests on the plate's inside edge.
                    self.plate_top + v1 - self.plate_width * k1
                } else {
                    // The underside stays where it meets the outside edge.
                    self.underside_at_baseline() + v1
                }
            }
        };
        out
    }
}

/// Eave-tip elevation of a plane of `pitch` and `overhang` (from the
/// footprint line) whose structure of `thickness` sits on `plate`, the same
/// rule [`crate::plate_baseline`] applies to the first plain hip edge, here
/// for a chosen reference plane (the default pitch and overhang, for Same
/// Height Eaves).
pub fn seated_eave_elevation(plate: f64, thickness: f64, pitch: f64, overhang: f64) -> f64 {
    let k = slope(pitch.max(0.0));
    plate + thickness.max(0.0) * (1.0 + k * k).sqrt() - overhang * k
}
