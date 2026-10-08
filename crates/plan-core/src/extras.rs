//! Typed, serde-default extras that dialogs used to keep per-session or stuff
//! into `floor.cad` text records: room specification options, per-opening and
//! per-wall dialog values, and cross-section lines. Every field defaults, so
//! older files load unchanged.
//!
//! Also the typed accessors for the opaque `roofs`, `electrical`, `framing`
//! (per floor) and `terrain` (per project) slots, shaped like
//! [`Floor::cabinets_as`](crate::model::Floor::cabinets_as).

use crate::geometry::Point;
use crate::model::{Floor, Project};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Which area figure a room label shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AreaKind {
    /// Clear area inside the finished walls.
    #[default]
    Interior,
    /// Area measured to the standard (framing) boundary.
    Standard,
    /// Area measured to wall centerlines.
    Centerline,
}

/// Fill drawn inside a room in plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomFill {
    pub color: [u8; 3],
    pub pattern: String,
    pub alpha: f32,
}

/// What a room's plan label shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomLabelOptions {
    pub show_name: bool,
    pub show_area: bool,
    pub show_dimensions: bool,
    pub area_kind: AreaKind,
}

impl Default for RoomLabelOptions {
    fn default() -> Self {
        Self {
            show_name: true,
            show_area: true,
            show_dimensions: false,
            area_kind: AreaKind::Interior,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoldingKind {
    Base,
    Crown,
    Chair,
}

/// A molding run applied around a room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoldingRef {
    pub kind: MoldingKind,
    pub profile: String,
    /// Molding height, inches.
    pub height: f64,
}

/// Door/window dialog values that used to live only in the dialog session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningExtras {
    pub style_name: Option<String>,
    pub thickness: Option<f64>,
    pub swing_angle_deg: Option<f64>,
    pub jamb_width: Option<f64>,
    pub frame_width: Option<f64>,
    pub sash_width: Option<f64>,
    pub show_open_in_plan: bool,
}

impl Default for OpeningExtras {
    fn default() -> Self {
        Self {
            style_name: None,
            thickness: None,
            swing_angle_deg: None,
            jamb_width: None,
            frame_width: None,
            sash_width: None,
            show_open_in_plan: true,
        }
    }
}

/// Wall dialog values that used to live only in the dialog session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallExtras {
    pub label_text: Option<String>,
    pub display_label: bool,
    pub last_wall_type: Option<String>,
}

impl Default for WallExtras {
    fn default() -> Self {
        Self {
            label_text: None,
            display_label: true,
            last_wall_type: None,
        }
    }
}

/// A cross-section cut line carried by a camera object.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SectionLine {
    pub a: Point,
    pub b: Point,
    /// Back clip distance behind the cut, inches (`None` = unlimited).
    #[serde(default)]
    pub back_clip: Option<f64>,
}

fn vec_as<T: DeserializeOwned>(v: &[serde_json::Value]) -> Result<Vec<T>, serde_json::Error> {
    v.iter().cloned().map(serde_json::from_value).collect()
}

fn vec_set<T: Serialize>(items: &[T]) -> Result<Vec<serde_json::Value>, serde_json::Error> {
    items.iter().map(serde_json::to_value).collect()
}

impl Floor {
    /// The floor's roofs as typed objects.
    pub fn roofs_as<T: DeserializeOwned>(&self) -> Result<Vec<T>, serde_json::Error> {
        vec_as(&self.roofs)
    }

    /// Replace the floor's roofs with `items`.
    pub fn set_roofs<T: Serialize>(&mut self, items: &[T]) -> Result<(), serde_json::Error> {
        self.roofs = vec_set(items)?;
        Ok(())
    }

    /// The floor's framing objects as typed objects.
    pub fn framing_as<T: DeserializeOwned>(&self) -> Result<Vec<T>, serde_json::Error> {
        vec_as(&self.framing)
    }

    /// Replace the floor's framing objects with `items`.
    pub fn set_framing<T: Serialize>(&mut self, items: &[T]) -> Result<(), serde_json::Error> {
        self.framing = vec_set(items)?;
        Ok(())
    }

    /// The floor's electrical data as a typed object (`None` when unset).
    pub fn electrical_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.electrical
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
    }

    /// Replace the floor's electrical data.
    pub fn set_electrical<T: Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.electrical = Some(serde_json::to_value(value)?);
        Ok(())
    }
}

impl Project {
    /// The project's terrain as a typed object (`None` when unset).
    pub fn terrain_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.terrain
            .as_ref()
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
    }

    /// Replace the project's terrain.
    pub fn set_terrain<T: Serialize>(&mut self, value: &T) -> Result<(), serde_json::Error> {
        self.terrain = Some(serde_json::to_value(value)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraKind, CameraObject};
    use crate::model::{Opening, RoomName, Wall, WallKind};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        a: u32,
        b: String,
    }

    fn samples() -> Vec<Sample> {
        vec![
            Sample {
                a: 1,
                b: "x".into(),
            },
            Sample {
                a: 2,
                b: "y".into(),
            },
        ]
    }

    #[test]
    fn slot_accessors_round_trip() {
        let mut p = Project::new("t");
        let f = &mut p.floors[0];
        f.set_roofs(&samples()).unwrap();
        f.set_framing(&samples()).unwrap();
        f.set_electrical(&samples()[0]).unwrap();
        p.set_terrain(&samples()[1]).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let q: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(q.floors[0].roofs_as::<Sample>().unwrap(), samples());
        assert_eq!(q.floors[0].framing_as::<Sample>().unwrap(), samples());
        assert_eq!(
            q.floors[0].electrical_as::<Sample>().unwrap(),
            Some(Sample {
                a: 1,
                b: "x".into()
            })
        );
        assert_eq!(
            q.terrain_as::<Sample>().unwrap(),
            Some(Sample {
                a: 2,
                b: "y".into()
            })
        );
        assert!(q.floors[0].roofs_as::<Vec<u8>>().is_err());
        assert!(Project::new("e").terrain_as::<Sample>().unwrap().is_none());
        assert!(q.floors[0].electrical_as::<Vec<u8>>().is_err());
    }

    #[test]
    fn old_project_json_loads_without_new_slots() {
        let mut v = serde_json::to_value(Project::new("old")).unwrap();
        v.as_object_mut().unwrap().remove("terrain");
        let f = v["floors"][0].as_object_mut().unwrap();
        for k in ["roofs", "electrical", "framing"] {
            f.remove(k);
        }
        let p: Project = serde_json::from_value(v).unwrap();
        assert!(p.terrain.is_none());
        assert!(p.floors[0].roofs.is_empty());
        assert!(p.floors[0].electrical.is_none());
        assert!(p.floors[0].framing.is_empty());
    }

    #[test]
    fn room_name_extras_round_trip_and_old_json_loads() {
        let mut r = RoomName::new(Point::new(1.0, 2.0), "Kitchen", "Kitchen");
        r.conditioned = Some(false);
        r.stem_wall_height = Some(24.0);
        r.fill_style = Some(RoomFill {
            color: [10, 20, 30],
            pattern: "Hatch".into(),
            alpha: 0.5,
        });
        r.label.show_dimensions = true;
        r.label.area_kind = AreaKind::Centerline;
        r.moldings = vec![MoldingRef {
            kind: MoldingKind::Crown,
            profile: "Cove".into(),
            height: 4.5,
        }];
        let back: RoomName = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);

        let old = r#"{"anchor":{"x":0.0,"y":0.0},"name":"A","room_type":"B"}"#;
        let o: RoomName = serde_json::from_str(old).unwrap();
        assert_eq!(o.conditioned, None);
        assert_eq!(o.stem_wall_height, None);
        assert_eq!(o.rough_ceiling, None);
        assert_eq!(o.fill_style, None);
        assert_eq!(o.label, RoomLabelOptions::default());
        assert!(o.label.show_name && o.label.show_area && !o.label.show_dimensions);
        assert_eq!(o.label.area_kind, AreaKind::Interior);
        assert!(o.moldings.is_empty());
    }

    #[test]
    fn opening_extras_round_trip_and_default() {
        let mut o = Opening::default_door(3, 1, 40.0);
        assert_eq!(o.extras, OpeningExtras::default());
        assert!(o.extras.show_open_in_plan);
        o.extras.style_name = Some("Shaker".into());
        o.extras.swing_angle_deg = Some(90.0);
        o.extras.show_open_in_plan = false;
        let json = serde_json::to_string(&o).unwrap();
        let back: Opening = serde_json::from_str(&json).unwrap();
        assert_eq!(back.extras, o.extras);

        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("extras");
        let old: Opening = serde_json::from_value(v).unwrap();
        assert_eq!(old.extras, OpeningExtras::default());
    }

    #[test]
    fn wall_extras_round_trip_and_default() {
        let mut w = Wall::new(
            Point::ZERO,
            Point::new(100.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(w.extras, WallExtras::default());
        assert!(w.extras.display_label);
        w.extras.label_text = Some("Bearing".into());
        w.extras.display_label = false;
        w.extras.last_wall_type = Some("Interior-4".into());
        let json = serde_json::to_string(&w).unwrap();
        let back: Wall = serde_json::from_str(&json).unwrap();
        assert_eq!(back.extras, w.extras);

        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("extras");
        let old: Wall = serde_json::from_value(v).unwrap();
        assert_eq!(old.extras, WallExtras::default());
    }

    #[test]
    fn camera_section_round_trip_and_default() {
        let mut c = CameraObject::new(CameraKind::FullCamera, Point::ZERO, 0.0, "Sec", 0);
        assert!(c.section.is_none());
        c.section = Some(SectionLine {
            a: Point::new(0.0, 0.0),
            b: Point::new(120.0, 0.0),
            back_clip: Some(60.0),
        });
        let json = serde_json::to_string(&c).unwrap();
        let back: CameraObject = serde_json::from_str(&json).unwrap();
        assert_eq!(back.section, c.section);

        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("section");
        let old: CameraObject = serde_json::from_value(v).unwrap();
        assert!(old.section.is_none());
    }
}
