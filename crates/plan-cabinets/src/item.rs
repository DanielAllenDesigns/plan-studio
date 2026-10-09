//! Per-item settings of a cabinet face item (reference manual pp. 675 to 685):
//! the Door, Drawer and Side Panel Face Item Specification dialogs and the
//! Cabinet Shelf Specification. An item that has none of these set is stored
//! as the bare [`crate::FaceItem`]; one the user customised is wrapped in
//! [`crate::FaceItem::Custom`] together with its [`ItemProps`].

use serde::{Deserialize, Serialize};

use crate::cabinet::{DoorStyle, DrawerStyle};
use crate::dress::shelf_count;

/// How deep a shelf is (Cabinet Shelf Specification, Depth).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ShelfDepth {
    /// The full depth of the opening.
    #[default]
    Full,
    /// Half the depth of the opening.
    Half,
    /// A depth entered in inches.
    Specify(f64),
}

impl ShelfDepth {
    /// The depth of a shelf in an opening `opening` deep, inches.
    pub fn inches(self, opening: f64) -> f64 {
        match self {
            ShelfDepth::Full => opening,
            ShelfDepth::Half => opening / 2.0,
            ShelfDepth::Specify(d) => d.clamp(0.0, opening),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ShelfDepth::Full => "Full",
            ShelfDepth::Half => "Half",
            ShelfDepth::Specify(_) => "Specify",
        }
    }
}

/// Thickness of an automatic shelf, inches.
pub const SHELF_THICKNESS: f64 = 0.75;

/// One manually specified shelf, storage item or organiser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shelf {
    /// The shelf rolls out when the door is shown open (standard cabinets).
    pub rollout: bool,
    /// How far it rolls out, inches.
    pub rollout_amount: f64,
    /// Thickness or height of the shelf or storage item, inches.
    pub thickness: f64,
    /// Distance from the top of the shelf below (or the bottom of the
    /// opening) to the bottom of this one, inches.
    pub spacing: f64,
    pub depth: ShelfDepth,
    /// A shelf, storage or organisation object picked from the Library.
    pub library: String,
}

impl Default for Shelf {
    fn default() -> Self {
        Self {
            rollout: false,
            rollout_amount: 12.0,
            thickness: SHELF_THICKNESS,
            spacing: 12.0,
            depth: ShelfDepth::Full,
            library: String::new(),
        }
    }
}

/// The shelves of a Door or Opening face item (Cabinet Shelf Specification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ShelfSpec {
    /// Manual (the shelves below) instead of Automatic shelving.
    pub manual: bool,
    /// The manual shelves, bottom to top.
    pub shelves: Vec<Shelf>,
    /// Spacing between the manual shelves stays equal (Equal Spacing).
    pub equal_spacing: bool,
}

/// A shelf placed in an opening: its bottom and top above the opening's
/// bottom, its depth and roll-out.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelfPlacement {
    pub z0: f64,
    pub z1: f64,
    pub depth: ShelfDepth,
    pub rollout: Option<f64>,
    pub library: String,
}

impl ShelfSpec {
    /// A manual spec of `n` equally spaced default shelves.
    pub fn manual_of(n: usize) -> Self {
        Self {
            manual: true,
            shelves: vec![Shelf::default(); n],
            equal_spacing: true,
        }
    }

    /// Number of shelves in an opening `h` inches tall.
    pub fn count(&self, h: f64) -> usize {
        if self.manual {
            self.shelves.len()
        } else {
            shelf_count(h)
        }
    }

    /// Re-spaces every manual shelf evenly through an opening `h` tall.
    pub fn equalize(&mut self, h: f64) {
        let n = self.shelves.len();
        if n == 0 {
            return;
        }
        let total: f64 = self.shelves.iter().map(|s| s.thickness).sum();
        let gap = ((h - total) / (n as f64 + 1.0)).max(0.0);
        for s in &mut self.shelves {
            s.spacing = gap;
        }
        self.equal_spacing = true;
    }

    /// The shelves in an opening `h` tall, bottom to top. Automatic shelves
    /// are evenly spaced; manual ones follow their spacings and any that no
    /// longer fit the opening (it was resized) are left out.
    pub fn place(&self, h: f64) -> Vec<ShelfPlacement> {
        if !self.manual {
            let n = shelf_count(h);
            return (1..=n)
                .map(|k| {
                    let c = h * k as f64 / (n + 1) as f64;
                    ShelfPlacement {
                        z0: c - SHELF_THICKNESS / 2.0,
                        z1: c + SHELF_THICKNESS / 2.0,
                        depth: ShelfDepth::Full,
                        rollout: None,
                        library: String::new(),
                    }
                })
                .collect();
        }
        let mut out = Vec::new();
        let mut z = 0.0;
        for s in &self.shelves {
            let thick = s.thickness.max(0.0);
            let (z0, z1) = if self.equal_spacing {
                let total: f64 = self.shelves.iter().map(|x| x.thickness).sum();
                let gap = ((h - total) / (self.shelves.len() as f64 + 1.0)).max(0.0);
                (z + gap, z + gap + thick)
            } else {
                (z + s.spacing, z + s.spacing + thick)
            };
            if z1 > h + 1e-9 {
                break;
            }
            out.push(ShelfPlacement {
                z0,
                z1,
                depth: s.depth,
                rollout: s.rollout.then_some(s.rollout_amount),
                library: s.library.clone(),
            });
            z = z1;
        }
        out
    }
}

/// Hardware Size/Orientation (reference manual p. 685): the size and angle
/// of a library handle or hinge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HardwareSize {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub retain_aspect: bool,
    /// Rotation about the axis out of the front, degrees.
    pub angle: f64,
}

impl Default for HardwareSize {
    fn default() -> Self {
        Self {
            width: 4.0,
            height: 0.75,
            depth: 1.0,
            retain_aspect: true,
            angle: 0.0,
        }
    }
}

impl HardwareSize {
    /// Resizes keeping the aspect ratio when asked to: `which` is 0, 1 or 2
    /// for width, height or depth.
    pub fn set(&mut self, which: usize, value: f64) {
        let value = value.max(0.01);
        let old = [self.width, self.height, self.depth][which];
        let k = if old > 1e-9 { value / old } else { 1.0 };
        if self.retain_aspect {
            self.width *= k;
            self.height *= k;
            self.depth *= k;
        } else {
            match which {
                0 => self.width = value,
                1 => self.height = value,
                _ => self.depth = value,
            }
        }
    }

    /// Adds 90 degrees (or subtracts them) to the angle, keeping it in
    /// `[0, 360)`.
    pub fn rotate(&mut self, delta: f64) {
        self.angle = (self.angle + delta).rem_euclid(360.0);
    }
}

/// The order door back inserts are placed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InsertOrder {
    #[default]
    TopToBottom,
    BottomToTop,
}

impl InsertOrder {
    pub fn name(self) -> &'static str {
        match self {
            InsertOrder::TopToBottom => "Top to Bottom",
            InsertOrder::BottomToTop => "Bottom to Top",
        }
    }
}

/// Everything a Door, Drawer, Panel or Appliance Face Item Specification
/// can set on one item. Fields left at their defaults follow the cabinet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ItemProps {
    /// A door or panel style of its own (a secondary style: glass doors over
    /// solid ones). `None` follows the cabinet's Door/Drawer panel.
    pub door: Option<DoorStyle>,
    /// A drawer style of its own.
    pub drawer: Option<DrawerStyle>,
    /// Size and angle of a library handle on this item.
    pub hardware: Option<HardwareSize>,
    /// Shelves of a door or opening.
    pub shelves: ShelfSpec,
    /// Left, Right, Top and Bottom overlap, inches (frameless traditional
    /// overlay only); `None` follows the cabinet.
    pub overlap: Option<[f64; 4]>,
    /// How far a drawer (or roll-out) is shown open in plan and 3D, percent;
    /// `None` follows the cabinet's Show Open.
    pub percent_open: Option<f64>,
    /// Swing angle of a door shown open, degrees (0 closed, 180 wide open);
    /// `None` follows the cabinet's Show Open.
    pub swing_angle: Option<f64>,
    /// Lock from Auto Resize: the item keeps its size when the cabinet is
    /// resized, an item is added or Equalize is used.
    pub locked: bool,
    /// Door back inserts: library objects on the back of the door.
    pub inserts: Vec<String>,
    pub insert_order: InsertOrder,
    /// Drawer Box/Pullout insert from the Library.
    pub drawer_box: String,
    /// Reverse Appliance: mirror the appliance left to right.
    pub reverse: bool,
    /// The appliance or fixture picked from the Library for an Appliance item.
    pub library: String,
}

impl ItemProps {
    /// True when nothing is set (the item needs no wrapper).
    pub fn is_default(&self) -> bool {
        *self == ItemProps::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_shelves_follow_the_opening_height() {
        let auto = ShelfSpec::default();
        assert_eq!(auto.count(30.0), 2);
        assert_eq!(auto.count(10.0), 0);
        let placed = auto.place(30.0);
        assert_eq!(placed.len(), 2);
        assert!(placed[0].z1 < placed[1].z0);
        assert!(placed.iter().all(|s| s.z0 > 0.0 && s.z1 < 30.0));
    }

    #[test]
    fn manual_shelves_stack_by_spacing_and_equalize() {
        let mut spec = ShelfSpec::manual_of(3);
        spec.equal_spacing = false;
        spec.shelves[0].spacing = 4.0;
        spec.shelves[1].spacing = 6.0;
        spec.shelves[2].spacing = 8.0;
        let p = spec.place(40.0);
        assert_eq!(p.len(), 3);
        assert!((p[0].z0 - 4.0).abs() < 1e-9);
        assert!((p[1].z0 - (4.75 + 6.0)).abs() < 1e-9);
        assert!((p[2].z0 - (4.75 + 6.0 + 0.75 + 8.0)).abs() < 1e-9);
        spec.equalize(40.0);
        let q = spec.place(40.0);
        let gap = (40.0 - 3.0 * 0.75) / 4.0;
        assert!((q[0].z0 - gap).abs() < 1e-9);
        assert!((q[2].z1 + gap - 40.0).abs() < 1e-9);
        // A shelf that no longer fits an opening that shrank is left out.
        spec.equal_spacing = false;
        assert!(spec.place(10.0).len() < 3);
    }

    #[test]
    fn rollout_and_depth_are_carried_to_the_placement() {
        let mut spec = ShelfSpec::manual_of(1);
        spec.shelves[0].rollout = true;
        spec.shelves[0].rollout_amount = 9.0;
        spec.shelves[0].depth = ShelfDepth::Specify(10.0);
        let p = spec.place(30.0);
        assert_eq!(p[0].rollout, Some(9.0));
        assert!((p[0].depth.inches(21.0) - 10.0).abs() < 1e-9);
        assert!((ShelfDepth::Half.inches(21.0) - 10.5).abs() < 1e-9);
        assert!((ShelfDepth::Specify(40.0).inches(21.0) - 21.0).abs() < 1e-9);
    }

    #[test]
    fn hardware_resize_keeps_the_aspect_ratio_when_asked() {
        let mut h = HardwareSize {
            width: 4.0,
            height: 1.0,
            depth: 2.0,
            retain_aspect: true,
            angle: 0.0,
        };
        h.set(0, 8.0);
        assert!((h.height - 2.0).abs() < 1e-9 && (h.depth - 4.0).abs() < 1e-9);
        h.retain_aspect = false;
        h.set(1, 1.0);
        assert!((h.width - 8.0).abs() < 1e-9 && (h.height - 1.0).abs() < 1e-9);
        h.rotate(-90.0);
        assert!((h.angle - 270.0).abs() < 1e-9);
        h.rotate(90.0);
        assert!(h.angle.abs() < 1e-9);
    }

    #[test]
    fn default_props_are_default_and_round_trip() {
        let p = ItemProps::default();
        assert!(p.is_default());
        let q = ItemProps {
            locked: true,
            percent_open: Some(50.0),
            shelves: ShelfSpec::manual_of(2),
            ..Default::default()
        };
        assert!(!q.is_default());
        let back: ItemProps = serde_json::from_str(&serde_json::to_string(&q).unwrap()).unwrap();
        assert_eq!(back, q);
        let old: ItemProps = serde_json::from_str("{}").unwrap();
        assert!(old.is_default());
    }
}
