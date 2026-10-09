//! Placed library symbols and opaque cabinet/stair slots
//! (`docs/parity/cabinets-stairs-framing-terrain-library.md`, CB-43..CB-50
//! placement of plants/symbols, CB-55/CB-56 click-to-place with auto-rotate).
//!
//! `plan-cabinets` and `plan-stairs` depend on this crate, so their objects are
//! stored on the floor as JSON values and converted with generic accessors.

use crate::geometry::{dist_to_segment, project_on_segment, Point};
use crate::model::{Floor, Id, Project, Wall};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Auto-rotate snaps to a wall when the symbol is within this distance of its
/// face, inches (CB-56).
pub const AUTO_ROTATE_SNAP: f64 = 6.0;

/// A library item placed in the plan. `position` is the **back-center** of the
/// symbol footprint (the library's origin convention); local +Y is the front.
/// `angle` is in **degrees**, counter-clockwise: at 0 the symbol's front
/// faces +Y and its width runs along +X.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedSymbol {
    pub id: Id,
    /// Library catalog id.
    pub catalog_id: String,
    pub position: Point,
    pub angle: f64,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    /// Height of the symbol's bottom above the floor, inches.
    pub elevation: f64,
    /// Mirrored left-right.
    pub flip: bool,
    pub label: String,
    /// Layer name (see `LayerSet`).
    pub layer: String,
    /// Makes the symbol a picture (Create Image / Billboard Image); see
    /// [`crate::images`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<crate::images::ImageSpec>,
    /// Makes the symbol the record of a distributed-object path or region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution: Option<crate::images::Distribution>,
    /// The distribution record this symbol is a generated copy of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<Id>,
    /// A 3D Solid Feature: the library object is used as a solid. Affects
    /// the 3D view only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solid: bool,
    /// How the object appears in the schedules (Library Object
    /// Specification > Schedule); `None` follows its library category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<SymbolSchedule>,
    /// Library-specific choices of the Options tab (`door_style`,
    /// `cabinet_door`, `hardware`: the library object's name). Empty by
    /// default.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub options: std::collections::BTreeMap<String, String>,
}

/// The Schedule tab of a placed library object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolSchedule {
    /// Leave the object out of every schedule.
    pub exclude: bool,
    /// The schedule it belongs to (`Fixture`, `Furniture`, `Plant`,
    /// `Appliance`); empty follows the library category.
    pub category: String,
    /// The Mark column.
    pub mark: String,
    pub manufacturer: String,
    pub model: String,
    pub note: String,
}

impl SymbolSchedule {
    /// True when nothing differs from the defaults (the object then needs no
    /// record).
    pub fn is_default(&self) -> bool {
        *self == SymbolSchedule::default()
    }
}

impl PlacedSymbol {
    /// A symbol at `position` (id assigned by [`Project::add_symbol`]).
    pub fn new(
        catalog_id: impl Into<String>,
        position: Point,
        width: f64,
        depth: f64,
        height: f64,
    ) -> Self {
        Self {
            id: 0,
            catalog_id: catalog_id.into(),
            position,
            angle: 0.0,
            width,
            depth,
            height,
            elevation: 0.0,
            flip: false,
            label: String::new(),
            layer: "CAD, Default".to_string(),
            image: None,
            distribution: None,
            owner: None,
            solid: false,
            schedule: None,
            options: std::collections::BTreeMap::new(),
        }
    }

    /// The footprint rectangle (back-left, back-right, front-right,
    /// front-left) after rotation.
    pub fn footprint(&self) -> [Point; 4] {
        let a = self.angle.to_radians();
        let u = Point::new(a.cos(), a.sin());
        let v = u.perp();
        let hw = self.width * 0.5;
        let p = self.position;
        [
            p - u * hw,
            p + u * hw,
            p + u * hw + v * self.depth,
            p - u * hw + v * self.depth,
        ]
    }

    /// Snap to the nearest wall face (CB-56): when the back-center is within
    /// [`AUTO_ROTATE_SNAP`] of a visible wall's face, rotate the symbol to the
    /// wall's direction with its front facing away from the wall, and place
    /// the back-center on that face. Returns whether it snapped.
    pub fn auto_rotate_to_wall(&mut self, walls: &[Wall]) -> bool {
        let mut best: Option<(f64, &Wall)> = None;
        for w in walls
            .iter()
            .filter(|w| !w.flags.invisible && w.length() > 0.0)
        {
            let d = (dist_to_segment(self.position, w.start, w.end) - w.thickness * 0.5).max(0.0);
            if d <= AUTO_ROTATE_SNAP && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, w));
            }
        }
        let Some((_, w)) = best else {
            return false;
        };
        let (_, q) = project_on_segment(self.position, w.start, w.end);
        let side = if self.position.sub(q).dot(w.normal()) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let dir_deg = w.direction().angle().to_degrees();
        self.angle = if side > 0.0 { dir_deg } else { dir_deg + 180.0 };
        self.angle = self.angle.rem_euclid(360.0);
        self.position = q + w.normal() * (side * w.thickness * 0.5);
        true
    }
}

impl Floor {
    /// The floor's cabinets as typed objects.
    pub fn cabinets_as<T: DeserializeOwned>(&self) -> Result<Vec<T>, serde_json::Error> {
        self.cabinets
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect()
    }

    /// Replace the floor's cabinets with `items`.
    pub fn set_cabinets<T: Serialize>(&mut self, items: &[T]) -> Result<(), serde_json::Error> {
        self.cabinets = items
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    /// The floor's stairs as typed objects.
    pub fn stairs_as<T: DeserializeOwned>(&self) -> Result<Vec<T>, serde_json::Error> {
        self.stairs
            .iter()
            .cloned()
            .map(serde_json::from_value)
            .collect()
    }

    /// Replace the floor's stairs with `items`.
    pub fn set_stairs<T: Serialize>(&mut self, items: &[T]) -> Result<(), serde_json::Error> {
        self.stairs = items
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    pub fn symbol(&self, id: Id) -> Option<&PlacedSymbol> {
        self.symbols.iter().find(|s| s.id == id)
    }
}

impl Project {
    /// Add a symbol (its `id` is replaced with a fresh one) and return the id.
    pub fn add_symbol(&mut self, floor: usize, mut symbol: PlacedSymbol) -> Id {
        let id = self.alloc_id();
        symbol.id = id;
        // A pasted fireplace brings its specification along.
        let record = crate::fireplace::take_carried_record(&mut symbol, id);
        self.floors[floor].symbols.push(symbol);
        if let Some(fp) = record {
            self.floors[floor].set_fireplace(fp);
        }
        id
    }

    /// Remove a symbol; returns whether it existed.
    pub fn remove_symbol(&mut self, floor: usize, id: Id) -> bool {
        let f = &mut self.floors[floor];
        let n = f.symbols.len();
        // A distribution record takes its generated copies with it.
        f.symbols.retain(|s| s.id != id && s.owner != Some(id));
        f.symbols.len() != n
    }

    /// Move a symbol to `position`; returns `false` if it does not exist.
    pub fn move_symbol(&mut self, floor: usize, id: Id, position: Point) -> bool {
        match self.floors[floor].symbols.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                s.position = position;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WallKind;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Cab {
        w: f64,
        name: String,
    }

    #[test]
    fn symbol_crud() {
        let mut p = Project::new("s");
        let id = p.add_symbol(
            0,
            PlacedSymbol::new("sofa", Point::new(10.0, 10.0), 84.0, 36.0, 32.0),
        );
        assert_eq!(p.floors[0].symbol(id).unwrap().catalog_id, "sofa");
        assert!(p.move_symbol(0, id, Point::new(50.0, 60.0)));
        assert_eq!(p.floors[0].symbols[0].position, Point::new(50.0, 60.0));
        assert!(!p.move_symbol(0, 999, Point::ZERO));
        assert!(p.remove_symbol(0, id));
        assert!(!p.remove_symbol(0, id));
    }

    #[test]
    fn typed_cabinet_and_stair_slots() {
        let mut p = Project::new("c");
        let cabs = vec![
            Cab {
                w: 24.0,
                name: "B24".into(),
            },
            Cab {
                w: 36.0,
                name: "B36".into(),
            },
        ];
        p.floors[0].set_cabinets(&cabs).unwrap();
        assert_eq!(p.floors[0].cabinets.len(), 2);
        assert_eq!(p.floors[0].cabinets_as::<Cab>().unwrap(), cabs);
        // Wrong type reports an error rather than panicking.
        assert!(p.floors[0].cabinets_as::<Vec<u8>>().is_err());
        p.floors[0].set_stairs(&[1.5f64, 2.5]).unwrap();
        assert_eq!(p.floors[0].stairs_as::<f64>().unwrap(), vec![1.5, 2.5]);
        // Survives a project round trip.
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(q.floors[0].cabinets_as::<Cab>().unwrap(), cabs);
    }

    fn wall() -> Wall {
        Wall {
            id: 1,
            ..Wall::new(
                Point::new(0.0, 0.0),
                Point::new(200.0, 0.0),
                4.5,
                96.0,
                WallKind::Interior,
            )
        }
    }

    #[test]
    fn auto_rotate_snaps_to_wall_face() {
        let walls = [wall()];
        // Above the wall (left side), 4" off the face: faces +y, angle 0.
        let mut s = PlacedSymbol::new("b", Point::new(100.0, 6.25), 24.0, 24.0, 34.0);
        s.angle = 37.0;
        assert!(s.auto_rotate_to_wall(&walls));
        assert!((s.angle - 0.0).abs() < 1e-9);
        assert!((s.position.y - 2.25).abs() < 1e-9 && (s.position.x - 100.0).abs() < 1e-9);
        // Below the wall: faces -y, angle 180, back on the lower face.
        let mut s = PlacedSymbol::new("b", Point::new(50.0, -5.0), 24.0, 24.0, 34.0);
        assert!(s.auto_rotate_to_wall(&walls));
        assert!((s.angle - 180.0).abs() < 1e-9);
        assert!((s.position.y + 2.25).abs() < 1e-9);
        let fp = s.footprint();
        assert!(
            fp[2].y < -2.25 - 20.0,
            "front must extend away from the wall"
        );
        // Too far: untouched.
        let mut s = PlacedSymbol::new("b", Point::new(50.0, 40.0), 24.0, 24.0, 34.0);
        s.angle = 12.0;
        assert!(!s.auto_rotate_to_wall(&walls));
        assert_eq!(s.angle, 12.0);
        // Invisible walls are ignored.
        let mut w = wall();
        w.flags.invisible = true;
        let mut s = PlacedSymbol::new("b", Point::new(50.0, 3.0), 24.0, 24.0, 34.0);
        assert!(!s.auto_rotate_to_wall(&[w]));
    }

    #[test]
    fn schedule_and_options_round_trip_and_older_plans_still_load() {
        let mut s = PlacedSymbol::new("a", Point::new(1.0, 2.0), 24.0, 24.0, 34.0);
        // Nothing set: the two slots are not written at all.
        let plain = serde_json::to_string(&s).unwrap();
        assert!(!plain.contains("schedule") && !plain.contains("options"));
        s.schedule = Some(SymbolSchedule {
            exclude: false,
            category: "Appliance".into(),
            mark: "A1".into(),
            ..SymbolSchedule::default()
        });
        s.options.insert("hardware".into(), "Matte Black".into());
        let back: PlacedSymbol = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert!(!back.schedule.as_ref().unwrap().is_default());
        assert!(SymbolSchedule::default().is_default());
        // A symbol saved before these slots existed loads with them empty.
        let back: PlacedSymbol = serde_json::from_str(&plain).unwrap();
        assert!(back.schedule.is_none() && back.options.is_empty());
    }
}
