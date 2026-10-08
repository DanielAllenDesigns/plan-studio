//! Code minimums: the one list of numbers the plan is held to.
//!
//! [`CodeMinimums`] is derived from the plan's [`CheckSettings`] (the
//! jurisdiction preset plus whatever limits were edited), so Plan Check, the
//! specification dialogs (which warn when a field is under or over the
//! limit), the tools (which start from legal values) and the plan defaults
//! (which are seeded from them) all read the same figures. A few limits that
//! have no field in [`CheckOptions`] (receptacle spacing, alarm placement,
//! the span tables) are fixed per code edition here; a plan can amend any
//! numeric one by name with [`CodeMinimums::with_overrides`] (stored with the
//! plan like the settings, see [`CodeMinimums::load`]).
//!
//! All lengths are inches, areas square feet, unless the field says
//! otherwise. Each field names the code section it comes from.

use std::collections::BTreeMap;

use plan_core::{Opening, OpeningStyle, Project};
use serde::{Deserialize, Serialize};

use crate::settings::{read_entry, write_entry};
use crate::{CheckOptions, CheckSettings};

const OVERRIDES_KEY: &str = "plancheck.minimum_overrides";

/// The limits a plan is checked against, in one place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeMinimums {
    /// The preset (or `Custom`) these came from.
    pub jurisdiction: String,
    /// Code edition (IRC year).
    pub code_year: u32,

    // ----- stairs (R311.7) -----
    /// Tallest riser (R311.7.5.1).
    pub stair_riser_max: f64,
    /// Shallowest tread (R311.7.5.2).
    pub stair_tread_min: f64,
    /// Narrowest stair (R311.7.1).
    pub stair_width_min: f64,
    /// Lowest headroom (R311.7.2).
    pub stair_headroom_min: f64,
    /// Shallowest landing (R311.7.6).
    pub stair_landing_min: f64,
    /// Most rise between landings (R311.7.3).
    pub stair_flight_rise_max: f64,

    // ----- rails and guards (R311.7.8, R312) -----
    /// Lowest handrail (R311.7.8.1).
    pub handrail_min: f64,
    /// Highest handrail (R311.7.8.1).
    pub handrail_max: f64,
    /// Flights with this many risers or more need a handrail (R311.7.8).
    pub handrail_risers: u32,
    /// Lowest guard (R312.1.2).
    pub guard_height: f64,
    /// Lowest guard on the open side of a stair (R312.1.2 exception).
    pub stair_guard_height: f64,
    /// Drop above which a walking surface needs a guard (R312.1.1).
    pub guard_drop: f64,
    /// Diameter of the sphere that must not pass through a guard (R312.1.3).
    pub guard_sphere: f64,

    // ----- egress window (R310.2) -----
    /// Smallest net clear opening, sq ft (R310.2.1).
    pub egress_min_area: f64,
    /// The same on the grade floor.
    pub egress_min_area_grade: f64,
    /// Narrowest net clear opening.
    pub egress_min_width: f64,
    /// Lowest net clear opening.
    pub egress_min_height: f64,
    /// Highest sill above the floor (R310.2.2).
    pub egress_max_sill: f64,

    // ----- egress door (R311.2) -----
    /// Clear width of the required egress door.
    pub egress_door_clear: f64,
    /// Height of the required egress door.
    pub egress_door_height: f64,
    /// Depth of the landing outside an exterior door (R311.3).
    pub exit_landing: f64,
    /// Narrowest bathroom door.
    pub bath_door_min: f64,
    /// Narrowest hall (R311.6).
    pub hall_width_min: f64,

    // ----- rooms (R304, R305, R303) -----
    /// Smallest habitable room, sq ft (R304.1).
    pub room_area_min: f64,
    /// Smallest horizontal dimension of a habitable room (R304.2).
    pub room_dim_min: f64,
    /// Lowest ceiling (R305.1).
    pub ceiling_min: f64,
    /// Lowest ceiling of a bath or laundry (R305.1 exception).
    pub ceiling_min_bath: f64,
    /// Glazing as a share of the floor area (R303.1).
    pub glazing_light_ratio: f64,
    /// Openable glazing as a share of the floor area (R303.1).
    pub glazing_vent_ratio: f64,

    // ----- bath (R307, P2705, P2708) -----
    /// Water closet: centerline to a side wall or fixture.
    pub toilet_side_clear: f64,
    /// Water closet: clear space in front.
    pub toilet_front_clear: f64,
    /// Clear space in front of a shower or tub entrance.
    pub shower_front_clear: f64,
    /// Shower and tub: smallest side.
    pub shower_min_dim: f64,
    /// Shower: smallest floor area, sq in.
    pub shower_min_area: f64,

    // ----- garage (R302.5, R302.6) -----
    /// Narrowest door from the garage to the house (R302.5.1).
    pub garage_door_min: f64,
    /// Thinnest gypsum board on the garage side of the house wall (R302.6).
    pub garage_gypsum_min: f64,
    /// Gypsum (Type X) where a habitable room is above the garage (R302.6).
    pub garage_gypsum_habitable_above: f64,

    // ----- foundation (R403) -----
    /// Frost line depth below finished grade (R403.1.4.1).
    pub frost_depth: f64,
    /// How far the first floor sits above finished grade.
    pub grade_below_floor: f64,
    /// Thinnest footing (R403.1.1).
    pub footing_min_thickness: f64,

    // ----- receptacles (NEC 210.52, IRC E3901) -----
    /// Wall length one receptacle serves: none farther apart than this.
    pub receptacle_max_spacing: f64,
    /// How far along a wall space one receptacle reaches.
    pub receptacle_reach: f64,
    /// Narrowest wall space that needs a receptacle (beside a door, say).
    pub receptacle_wall_space_min: f64,
    /// Kitchen counter: farthest apart (210.52(C)).
    pub counter_receptacle_spacing: f64,
    /// Kitchen counter: no point farther than this from a receptacle.
    pub counter_receptacle_reach: f64,
    /// Kitchen counter: narrowest wall space that needs one.
    pub counter_wall_space_min: f64,

    // ----- alarms (R314, R315) -----
    /// A smoke alarm in each bedroom (R314.3).
    pub smoke_alarm_in_bedrooms: bool,
    /// A smoke alarm outside each sleeping area.
    pub smoke_alarm_outside_sleeping: bool,
    /// A smoke alarm on every level.
    pub smoke_alarm_each_level: bool,
    /// A CO alarm outside each sleeping area (R315.3).
    pub co_alarm_outside_sleeping: bool,
    /// A CO alarm with an attached garage (R315.2).
    pub co_alarm_attached_garage: bool,

    // ----- framing (R502, R802) -----
    /// Widest joist or rafter spacing the span tables cover.
    pub framing_spacing_max: f64,
    /// Spacing the span tables are written for (16" on centre).
    pub framing_table_spacing: f64,

    // ----- roof (R905) -----
    /// Shallowest pitch for shingles, rise per 12 (R905.2.2).
    pub roof_pitch_min: f64,
    /// Pitch under which double underlayment is needed (R905.1.1).
    pub roof_pitch_underlay: f64,
}

impl Default for CodeMinimums {
    fn default() -> Self {
        Self::from_options(
            "IRC 2021 residential",
            &CheckOptions::default(),
            &BTreeMap::new(),
        )
    }
}

impl CodeMinimums {
    /// The minimums of a plan's Plan Check settings.
    pub fn from_settings(s: &CheckSettings) -> Self {
        Self::from_options(&s.jurisdiction, &s.options, &BTreeMap::new())
    }

    /// The minimums of a named preset of [`crate::JURISDICTIONS`]; a name
    /// that is not a preset (`Custom`) gets the 2021 IRC figures under that
    /// name.
    pub fn for_jurisdiction(name: &str) -> Self {
        match CheckSettings::preset(name) {
            Some(s) => Self::from_settings(&s),
            None => Self {
                jurisdiction: name.to_string(),
                ..Self::default()
            },
        }
    }

    /// The minimums stored with a plan: its Plan Check settings and the
    /// overrides saved by [`CodeMinimums::store_overrides`].
    pub fn load(project: &Project) -> Self {
        let s = CheckSettings::load(project);
        Self::from_options(&s.jurisdiction, &s.options, &Self::load_overrides(project))
    }

    /// The named amendments stored with the plan (empty when there are none).
    pub fn load_overrides(project: &Project) -> BTreeMap<String, f64> {
        read_entry(project, OVERRIDES_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Keep named amendments with the plan (an empty map clears them).
    pub fn store_overrides(project: &mut Project, overrides: &BTreeMap<String, f64>) {
        let value = (!overrides.is_empty())
            .then(|| serde_json::to_string(overrides).ok())
            .flatten();
        write_entry(project, OVERRIDES_KEY, value);
    }

    fn from_options(name: &str, o: &CheckOptions, overrides: &BTreeMap<String, f64>) -> Self {
        let m = Self {
            jurisdiction: name.to_string(),
            code_year: o.code_year,
            stair_riser_max: o.riser_max,
            stair_tread_min: o.tread_min,
            stair_width_min: o.stair_min_width,
            stair_headroom_min: o.headroom,
            stair_landing_min: o.landing_min,
            stair_flight_rise_max: 147.0,
            handrail_min: o.stair_guard_height,
            handrail_max: o.handrail_max,
            handrail_risers: o.handrail_risers,
            guard_height: o.guard_height,
            stair_guard_height: o.stair_guard_height,
            guard_drop: o.guard_drop,
            guard_sphere: o.guard_sphere,
            egress_min_area: o.egress_min_area,
            egress_min_area_grade: o.egress_min_area.min(5.0),
            egress_min_width: o.egress_min_w,
            egress_min_height: o.egress_min_h,
            egress_max_sill: o.egress_max_sill,
            egress_door_clear: o.min_door_width,
            egress_door_height: o.entry_door_height,
            exit_landing: o.exit_landing,
            bath_door_min: o.bath_door,
            hall_width_min: o.min_hall_width,
            room_area_min: o.min_room_area,
            room_dim_min: o.min_room_dim,
            ceiling_min: o.min_ceiling,
            ceiling_min_bath: o.min_ceiling_bath,
            glazing_light_ratio: 0.08,
            glazing_vent_ratio: o.vent_ratio,
            toilet_side_clear: o.wc_side_clear,
            toilet_front_clear: o.wc_front_clear,
            shower_front_clear: o.shower_front_clear,
            shower_min_dim: o.shower_min_dim,
            shower_min_area: o.shower_min_area,
            garage_door_min: o.garage_house_door_min,
            garage_gypsum_min: o.garage_gypsum_min,
            garage_gypsum_habitable_above: 0.625,
            frost_depth: o.frost_depth,
            grade_below_floor: o.grade_below_floor,
            footing_min_thickness: o.footing_min_thickness,
            receptacle_max_spacing: 144.0,
            receptacle_reach: 72.0,
            receptacle_wall_space_min: 24.0,
            counter_receptacle_spacing: 48.0,
            counter_receptacle_reach: 24.0,
            counter_wall_space_min: 12.0,
            smoke_alarm_in_bedrooms: true,
            smoke_alarm_outside_sleeping: true,
            smoke_alarm_each_level: true,
            co_alarm_outside_sleeping: true,
            co_alarm_attached_garage: true,
            framing_spacing_max: 24.0,
            framing_table_spacing: 16.0,
            roof_pitch_min: o.roof_pitch_min,
            roof_pitch_underlay: o.roof_pitch_underlay,
        };
        m.with_overrides(overrides)
    }

    /// Applies named amendments (see [`CodeMinimums::numeric_keys`]); keys
    /// that are not numeric fields are ignored.
    pub fn with_overrides(mut self, overrides: &BTreeMap<String, f64>) -> Self {
        for (k, v) in overrides {
            if v.is_finite() {
                self.set(k, *v);
            }
        }
        self
    }

    /// Short name for the status bar: `IRC 2021`, `IRC 2018 (Georgia)`,
    /// `IRC 2021 (Custom)`.
    pub fn label(&self) -> String {
        let region = if self.jurisdiction.starts_with("Georgia") {
            Some("Georgia")
        } else if self.jurisdiction == "Custom" {
            Some("Custom")
        } else {
            None
        };
        match region {
            Some(r) => format!("IRC {} ({r})", self.code_year),
            None => format!("IRC {}", self.code_year),
        }
    }

    /// Leaf width of the required egress door (clear width plus 4").
    pub fn egress_door_width(&self) -> f64 {
        self.egress_door_clear + 4.0
    }

    /// Footing width for `stories` storeys on 2000 psf soil (Table R403.1(1)).
    pub fn footing_width(&self, stories: usize) -> f64 {
        match stories {
            0 | 1 => 12.0,
            2 => 15.0,
            _ => 18.0,
        }
    }

    /// Header depth over an opening `width` inches wide (R602.7 /
    /// Table R602.7(1), doubled 2x headers, 30 psf snow, 28' span): 2x6 to
    /// 3', 2x8 to 4', 2x10 to 6', 2x12 beyond.
    pub fn header_depth(&self, width: f64) -> f64 {
        if width <= 36.0 {
            5.5
        } else if width <= 48.0 {
            7.25
        } else if width <= 72.0 {
            9.25
        } else {
            11.25
        }
    }

    /// Longest unsupported joist span by joist depth (R502.3.1, 40 psf live
    /// load, 16" on centre, Douglas fir-larch No. 2).
    pub fn joist_span(&self, depth: f64) -> f64 {
        if depth < 6.5 {
            117.0
        } else if depth < 8.5 {
            154.0
        } else if depth < 10.5 {
            195.0
        } else {
            236.0
        }
    }

    /// Longest rafter run (plan projection) by rafter depth: a simplified
    /// conservative table (R802.4.1: 16" on centre, 20 psf roof live load,
    /// 10 psf dead load, ceiling joists tied at the plates). Not a
    /// structural design.
    pub fn rafter_span(&self, depth: f64) -> f64 {
        if depth < 4.5 {
            72.0
        } else if depth < 6.5 {
            120.0
        } else if depth < 8.5 {
            158.0
        } else if depth < 10.5 {
            202.0
        } else {
            244.0
        }
    }

    /// The shallowest joist (nominal depth, inches) that spans `span`, or
    /// `None` when none of the table does.
    pub fn joist_depth_for(&self, span: f64) -> Option<f64> {
        [5.5, 7.25, 9.25, 11.25]
            .into_iter()
            .find(|d| self.joist_span(*d) + 1e-6 >= span)
    }

    /// The shallowest rafter that runs `run`, or `None`.
    pub fn rafter_depth_for(&self, run: f64) -> Option<f64> {
        [3.5, 5.5, 7.25, 9.25, 11.25]
            .into_iter()
            .find(|d| self.rafter_span(*d) + 1e-6 >= run)
    }

    /// Footing width and thickness a Build Foundation dialog should offer for
    /// `stories` storeys: the table width and at least the minimum thickness
    /// (the current choice is kept when it is already legal).
    pub fn footing_for(&self, stories: usize, width: f64, thickness: f64) -> (f64, f64) {
        (
            width.max(self.footing_width(stories)),
            thickness.max(self.footing_min_thickness),
        )
    }

    /// Footing depth to ask for: the underside must reach the frost depth
    /// below grade. `elevation` is the foundation floor's elevation relative
    /// to the first floor, `thickness` the footing's thickness; returns the
    /// thickness that puts the bottom at the frost line (at least the
    /// footing minimum).
    pub fn footing_thickness_for_frost(&self, elevation: f64, thickness: f64) -> f64 {
        let deepest_ok = -self.grade_below_floor - self.frost_depth;
        let bottom = elevation - thickness;
        let t = if bottom > deepest_ok {
            thickness + (bottom - deepest_ok)
        } else {
            thickness
        };
        t.max(self.footing_min_thickness)
    }

    /// Net clear opening of a window when its sash is open, `(width,
    /// height)`; `None` for windows that cannot be opened (fixed windows,
    /// niches). A sliding window opens half its width.
    pub fn window_net_clear(op: &Opening) -> Option<(f64, f64)> {
        match op.style {
            OpeningStyle::Fixed | OpeningStyle::WallNiche | OpeningStyle::PassThrough => None,
            OpeningStyle::SlidingWindow => Some((op.width * 0.5, op.height)),
            _ => Some((op.width, op.height)),
        }
    }

    /// What a window in a sleeping room lacks to be an escape opening, one
    /// sentence per shortfall (empty when it passes). `grade_floor` uses the
    /// smaller grade-floor area.
    pub fn egress_shortfalls(&self, op: &Opening, grade_floor: bool) -> Vec<String> {
        let Some((w, h)) = Self::window_net_clear(op) else {
            return vec!["cannot be opened (a fixed window is not an escape opening)".to_string()];
        };
        let min_area = if grade_floor {
            self.egress_min_area_grade
        } else {
            self.egress_min_area
        };
        let mut why = Vec::new();
        let area = w * h / 144.0;
        if area < min_area - 1e-6 {
            why.push(format!(
                "has {area:.1} sq ft net clear, needs {min_area:.1}"
            ));
        }
        if w < self.egress_min_width - 1e-6 {
            why.push(format!(
                "is {w:.0}\" wide net clear, needs {:.0}\"",
                self.egress_min_width
            ));
        }
        if h < self.egress_min_height - 1e-6 {
            why.push(format!(
                "is {h:.0}\" high net clear, needs {:.0}\"",
                self.egress_min_height
            ));
        }
        if op.sill_height > self.egress_max_sill + 1e-6 {
            why.push(format!(
                "has a {:.0}\" sill, at most {:.0}\" allowed",
                op.sill_height, self.egress_max_sill
            ));
        }
        why
    }

    /// Is a room of this type a habitable room (R304)? The same test Plan
    /// Check uses; the type is a room type or name such as `"Master Bedroom"`.
    pub fn is_habitable(room_type: &str) -> bool {
        crate::ctx::is_habitable(&room_type.to_lowercase())
    }

    /// Is a room of this type a sleeping room (a bedroom)?
    pub fn is_sleeping(room_type: &str) -> bool {
        crate::ctx::is_bedroom(&room_type.to_lowercase())
    }

    /// The lowest ceiling a room of this type may have (R305.1): the full
    /// minimum for habitable rooms and halls, the bath minimum for baths and
    /// laundries, `None` for rooms the rule does not look at.
    pub fn ceiling_min_for(&self, room_type: &str) -> Option<f64> {
        let t = room_type.to_lowercase();
        if crate::ctx::is_habitable(&t) || crate::ctx::is_hall(&t) {
            Some(self.ceiling_min)
        } else if crate::ctx::is_bath(&t) || t.contains("laundry") {
            Some(self.ceiling_min_bath)
        } else {
            None
        }
    }

    /// The names [`CodeMinimums::get`] and [`CodeMinimums::set`] accept.
    pub fn numeric_keys() -> &'static [&'static str] {
        NUMERIC_KEYS
    }

    /// A numeric field by name.
    pub fn get(&self, key: &str) -> Option<f64> {
        let mut copy = self.clone();
        copy.numeric_mut(key).map(|v| *v)
    }

    /// Sets a numeric field by name; false for a name that is not one.
    pub fn set(&mut self, key: &str, value: f64) -> bool {
        match self.numeric_mut(key) {
            Some(v) => {
                *v = value;
                true
            }
            None => false,
        }
    }

    fn numeric_mut(&mut self, key: &str) -> Option<&mut f64> {
        Some(match key {
            "stair_riser_max" => &mut self.stair_riser_max,
            "stair_tread_min" => &mut self.stair_tread_min,
            "stair_width_min" => &mut self.stair_width_min,
            "stair_headroom_min" => &mut self.stair_headroom_min,
            "stair_landing_min" => &mut self.stair_landing_min,
            "stair_flight_rise_max" => &mut self.stair_flight_rise_max,
            "handrail_min" => &mut self.handrail_min,
            "handrail_max" => &mut self.handrail_max,
            "guard_height" => &mut self.guard_height,
            "stair_guard_height" => &mut self.stair_guard_height,
            "guard_drop" => &mut self.guard_drop,
            "guard_sphere" => &mut self.guard_sphere,
            "egress_min_area" => &mut self.egress_min_area,
            "egress_min_area_grade" => &mut self.egress_min_area_grade,
            "egress_min_width" => &mut self.egress_min_width,
            "egress_min_height" => &mut self.egress_min_height,
            "egress_max_sill" => &mut self.egress_max_sill,
            "egress_door_clear" => &mut self.egress_door_clear,
            "egress_door_height" => &mut self.egress_door_height,
            "exit_landing" => &mut self.exit_landing,
            "bath_door_min" => &mut self.bath_door_min,
            "hall_width_min" => &mut self.hall_width_min,
            "room_area_min" => &mut self.room_area_min,
            "room_dim_min" => &mut self.room_dim_min,
            "ceiling_min" => &mut self.ceiling_min,
            "ceiling_min_bath" => &mut self.ceiling_min_bath,
            "glazing_light_ratio" => &mut self.glazing_light_ratio,
            "glazing_vent_ratio" => &mut self.glazing_vent_ratio,
            "toilet_side_clear" => &mut self.toilet_side_clear,
            "toilet_front_clear" => &mut self.toilet_front_clear,
            "shower_front_clear" => &mut self.shower_front_clear,
            "shower_min_dim" => &mut self.shower_min_dim,
            "shower_min_area" => &mut self.shower_min_area,
            "garage_door_min" => &mut self.garage_door_min,
            "garage_gypsum_min" => &mut self.garage_gypsum_min,
            "garage_gypsum_habitable_above" => &mut self.garage_gypsum_habitable_above,
            "frost_depth" => &mut self.frost_depth,
            "grade_below_floor" => &mut self.grade_below_floor,
            "footing_min_thickness" => &mut self.footing_min_thickness,
            "receptacle_max_spacing" => &mut self.receptacle_max_spacing,
            "receptacle_reach" => &mut self.receptacle_reach,
            "receptacle_wall_space_min" => &mut self.receptacle_wall_space_min,
            "counter_receptacle_spacing" => &mut self.counter_receptacle_spacing,
            "counter_receptacle_reach" => &mut self.counter_receptacle_reach,
            "counter_wall_space_min" => &mut self.counter_wall_space_min,
            "framing_spacing_max" => &mut self.framing_spacing_max,
            "framing_table_spacing" => &mut self.framing_table_spacing,
            "roof_pitch_min" => &mut self.roof_pitch_min,
            "roof_pitch_underlay" => &mut self.roof_pitch_underlay,
            _ => return None,
        })
    }
}

const NUMERIC_KEYS: &[&str] = &[
    "stair_riser_max",
    "stair_tread_min",
    "stair_width_min",
    "stair_headroom_min",
    "stair_landing_min",
    "stair_flight_rise_max",
    "handrail_min",
    "handrail_max",
    "guard_height",
    "stair_guard_height",
    "guard_drop",
    "guard_sphere",
    "egress_min_area",
    "egress_min_area_grade",
    "egress_min_width",
    "egress_min_height",
    "egress_max_sill",
    "egress_door_clear",
    "egress_door_height",
    "exit_landing",
    "bath_door_min",
    "hall_width_min",
    "room_area_min",
    "room_dim_min",
    "ceiling_min",
    "ceiling_min_bath",
    "glazing_light_ratio",
    "glazing_vent_ratio",
    "toilet_side_clear",
    "toilet_front_clear",
    "shower_front_clear",
    "shower_min_dim",
    "shower_min_area",
    "garage_door_min",
    "garage_gypsum_min",
    "garage_gypsum_habitable_above",
    "frost_depth",
    "grade_below_floor",
    "footing_min_thickness",
    "receptacle_max_spacing",
    "receptacle_reach",
    "receptacle_wall_space_min",
    "counter_receptacle_spacing",
    "counter_receptacle_reach",
    "counter_wall_space_min",
    "framing_spacing_max",
    "framing_table_spacing",
    "roof_pitch_min",
    "roof_pitch_underlay",
];

impl From<&CheckSettings> for CodeMinimums {
    fn from(s: &CheckSettings) -> Self {
        Self::from_settings(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CheckSettings, JURISDICTIONS};

    #[test]
    fn the_default_is_the_2021_irc() {
        let m = CodeMinimums::default();
        assert_eq!(m.jurisdiction, "IRC 2021 residential");
        assert_eq!(m.code_year, 2021);
        assert_eq!(m.stair_riser_max, 7.75);
        assert_eq!(m.stair_tread_min, 10.0);
        assert_eq!(m.stair_width_min, 36.0);
        assert_eq!(m.stair_headroom_min, 80.0);
        assert_eq!((m.handrail_min, m.handrail_max), (34.0, 38.0));
        assert_eq!(m.guard_height, 36.0);
        assert_eq!(m.guard_sphere, 4.0);
        assert_eq!(m.egress_min_area, 5.7);
        assert_eq!(m.egress_max_sill, 44.0);
        assert_eq!(m.egress_door_width(), 36.0);
        assert_eq!(m.egress_door_height, 80.0);
        assert_eq!(m.ceiling_min, 84.0);
        assert_eq!(m.garage_gypsum_habitable_above, 0.625);
        assert_eq!(m.label(), "IRC 2021");
    }

    #[test]
    fn every_option_the_rules_read_reaches_the_minimums() {
        let from = CodeMinimums::from(&CheckSettings::irc_2021());
        assert_eq!(from, CodeMinimums::default());
        let mut s = CheckSettings::irc_2021();
        s.options.riser_max = 7.0;
        s.options.guard_height = 42.0;
        s.options.frost_depth = 24.0;
        s.name_from_limits();
        let m = CodeMinimums::from(&s);
        assert_eq!(m.stair_riser_max, 7.0);
        assert_eq!(m.guard_height, 42.0);
        assert_eq!(m.frost_depth, 24.0);
        assert_eq!(m.jurisdiction, "Custom");
        assert_eq!(m.label(), "IRC 2021 (Custom)");
    }

    #[test]
    fn each_jurisdiction_has_its_own_minimums() {
        let irc = CodeMinimums::for_jurisdiction(JURISDICTIONS[0]);
        let ga = CodeMinimums::for_jurisdiction(JURISDICTIONS[1]);
        let custom = CodeMinimums::for_jurisdiction(JURISDICTIONS[2]);
        assert_eq!(irc.code_year, 2021);
        assert_eq!(ga.code_year, 2018);
        assert_eq!(ga.label(), "IRC 2018 (Georgia)");
        assert_eq!(ga.jurisdiction, "Georgia 2020");
        assert_eq!(ga.frost_depth, 12.0);
        // Limits equal except for the edition.
        assert_eq!(ga.stair_riser_max, irc.stair_riser_max);
        assert_ne!(irc, ga);
        assert_eq!(custom.jurisdiction, "Custom");
        assert_eq!(custom.stair_riser_max, irc.stair_riser_max);
        // A name that is not a preset also gets the 2021 figures.
        assert_eq!(
            CodeMinimums::for_jurisdiction("Atlanta").jurisdiction,
            "Atlanta"
        );
    }

    #[test]
    fn a_plans_settings_and_amendments_are_stored_with_it() {
        let mut p = Project::new("t");
        assert_eq!(CodeMinimums::load(&p), CodeMinimums::default());
        let mut s = CheckSettings::irc_2021();
        s.options.riser_max = 7.5;
        s.name_from_limits();
        s.store(&mut p);
        assert_eq!(CodeMinimums::load(&p).stair_riser_max, 7.5);
        let mut o = BTreeMap::new();
        o.insert("guard_height".to_string(), 42.0);
        o.insert("not_a_field".to_string(), 1.0);
        CodeMinimums::store_overrides(&mut p, &o);
        let m = CodeMinimums::load(&p);
        assert_eq!(m.guard_height, 42.0);
        assert_eq!(m.stair_riser_max, 7.5);
        CodeMinimums::store_overrides(&mut p, &BTreeMap::new());
        assert_eq!(CodeMinimums::load(&p).guard_height, 36.0);
    }

    #[test]
    fn named_fields_read_and_write() {
        let mut m = CodeMinimums::default();
        for k in CodeMinimums::numeric_keys() {
            assert!(m.get(k).is_some(), "{k}");
        }
        assert_eq!(m.get("stair_tread_min"), Some(10.0));
        assert!(m.set("stair_tread_min", 11.0));
        assert_eq!(m.stair_tread_min, 11.0);
        assert!(!m.set("bogus", 1.0));
        assert_eq!(m.get("bogus"), None);
    }

    #[test]
    fn the_tables_match_the_code() {
        let m = CodeMinimums::default();
        assert_eq!(
            (m.footing_width(1), m.footing_width(2), m.footing_width(3)),
            (12.0, 15.0, 18.0)
        );
        assert_eq!(m.header_depth(30.0), 5.5);
        assert_eq!(m.header_depth(48.0), 7.25);
        assert_eq!(m.header_depth(60.0), 9.25);
        assert_eq!(m.header_depth(90.0), 11.25);
        assert_eq!(m.joist_span(7.25), 154.0);
        assert_eq!(m.rafter_span(5.5), 120.0);
        assert_eq!(m.joist_depth_for(150.0), Some(7.25));
        assert_eq!(m.joist_depth_for(300.0), None);
        assert_eq!(m.rafter_depth_for(60.0), Some(3.5));
        assert_eq!(m.rafter_depth_for(250.0), None);
    }

    #[test]
    fn footings_are_sized_from_the_table_and_the_frost_line() {
        let m = CodeMinimums::default();
        assert_eq!(m.footing_for(2, 12.0, 4.0), (15.0, 6.0));
        assert_eq!(m.footing_for(1, 16.0, 8.0), (16.0, 8.0));
        // Grade 6" below the floor plus 12" of frost: the bottom must be at
        // -18" or lower. A foundation floor at -10" with a 6" footing is not.
        assert_eq!(m.footing_thickness_for_frost(-10.0, 6.0), 8.0);
        assert_eq!(m.footing_thickness_for_frost(0.0, 6.0), 18.0);
        assert_eq!(m.footing_thickness_for_frost(-20.0, 8.0), 8.0);
    }

    #[test]
    fn room_types_decide_which_limits_apply() {
        let m = CodeMinimums::default();
        assert!(CodeMinimums::is_habitable("Master Bedroom"));
        assert!(CodeMinimums::is_habitable("Kitchen"));
        assert!(!CodeMinimums::is_habitable("Garage"));
        assert!(CodeMinimums::is_sleeping("Bedroom #2"));
        assert!(!CodeMinimums::is_sleeping("Master Bath"));
        assert_eq!(m.ceiling_min_for("Living"), Some(84.0));
        assert_eq!(m.ceiling_min_for("Hall"), Some(84.0));
        assert_eq!(m.ceiling_min_for("Master Bath"), Some(80.0));
        assert_eq!(m.ceiling_min_for("Laundry"), Some(80.0));
        assert_eq!(m.ceiling_min_for("Garage"), None);
    }

    #[test]
    fn egress_shortfalls_name_what_is_missing() {
        use plan_core::{Opening, OpeningKind};
        let m = CodeMinimums::default();
        let mut w = Opening::new(1, 0.0, OpeningKind::Window, 36.0, 60.0, 36.0);
        w.style = OpeningStyle::Casement;
        assert!(m.egress_shortfalls(&w, false).is_empty());
        w.height = 20.0;
        w.sill_height = 50.0;
        let why = m.egress_shortfalls(&w, false);
        assert_eq!(why.len(), 3, "{why:?}");
        w.style = OpeningStyle::Fixed;
        assert!(m.egress_shortfalls(&w, false)[0].contains("cannot be opened"));
    }
}
