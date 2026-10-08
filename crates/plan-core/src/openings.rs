//! Door and window extensions for Chief parity (`docs/parity/doors-windows.md`).
//!
//! * DW-31..DW-33: swing side and hinge end are independent
//!   (`Opening::swing_flipped` = swings to the other side of the wall,
//!   `Opening::hinge_at_end` = hinge on the wall-end jamb).
//! * DW-38..DW-58: the opening style ([`OpeningStyle`]).
//! * DW-59..DW-63: plan label, size shorthand and schedule number.
//! * DW-31..DW-37 editing: [`Project::flip_swing`], [`Project::flip_hinge`],
//!   [`Project::slide_opening`].

use crate::geometry::Point;
use crate::model::{Id, Opening, OpeningKind, Project};
use serde::{Deserialize, Serialize};

/// Minimum clear distance between an opening jamb and a wall end or another
/// opening (same as `Project::add_opening`).
const OPENING_MARGIN: f64 = 2.0;

/// Drawing/behaviour style of an opening (DW-38..DW-58).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OpeningStyle {
    #[default]
    Hinged,
    Sliding,
    Pocket,
    Bifold,
    Garage,
    Doorway,
    Barn,
    Shower,
    Fixed,
    Window,
    BayWindow,
    BowWindow,
    BoxWindow,
    PassThrough,
    WallNiche,
}

impl OpeningStyle {
    /// The style a new opening of `kind` starts with.
    pub fn default_for(kind: OpeningKind) -> OpeningStyle {
        match kind {
            OpeningKind::Door => OpeningStyle::Hinged,
            OpeningKind::Window => OpeningStyle::Window,
        }
    }

    /// Whether this style belongs to doors (as opposed to windows/niches).
    pub fn is_door_style(self) -> bool {
        matches!(
            self,
            OpeningStyle::Hinged
                | OpeningStyle::Sliding
                | OpeningStyle::Pocket
                | OpeningStyle::Bifold
                | OpeningStyle::Garage
                | OpeningStyle::Doorway
                | OpeningStyle::Barn
                | OpeningStyle::Shower
        )
    }
}

/// Casing around an opening (DW defaults: width, depth, reveal), inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Casing {
    pub width: f64,
    pub depth: f64,
    pub reveal: f64,
}

impl Default for Casing {
    fn default() -> Self {
        Self {
            width: 3.5,
            depth: 0.75,
            reveal: 0.25,
        }
    }
}

pub(crate) fn default_lites() -> (u32, u32) {
    (1, 1)
}

/// On-disk shape of [`Opening`]; fills `style` from the kind when absent so
/// old files get `Window` for windows, not the door default.
#[derive(Deserialize)]
pub(crate) struct OpeningDe {
    id: Id,
    wall_id: Id,
    center_offset: f64,
    width: f64,
    height: f64,
    sill_height: f64,
    kind: OpeningKind,
    #[serde(default)]
    swing_flipped: bool,
    #[serde(default)]
    hinge_at_end: bool,
    #[serde(default)]
    style: Option<OpeningStyle>,
    #[serde(default)]
    label_override: Option<String>,
    #[serde(default)]
    schedule_number: Option<String>,
    #[serde(default)]
    casing: Option<Casing>,
    #[serde(default = "default_lites")]
    lites: (u32, u32),
    #[serde(default)]
    egress: bool,
    #[serde(default)]
    tempered: bool,
    #[serde(default)]
    extras: crate::extras::OpeningExtras,
}

impl From<OpeningDe> for Opening {
    fn from(d: OpeningDe) -> Opening {
        Opening {
            id: d.id,
            wall_id: d.wall_id,
            center_offset: d.center_offset,
            width: d.width,
            height: d.height,
            sill_height: d.sill_height,
            kind: d.kind,
            swing_flipped: d.swing_flipped,
            hinge_at_end: d.hinge_at_end,
            style: d.style.unwrap_or_else(|| OpeningStyle::default_for(d.kind)),
            label_override: d.label_override,
            schedule_number: d.schedule_number,
            casing: d.casing,
            lites: d.lites,
            egress: d.egress,
            tempered: d.tempered,
            extras: d.extras,
        }
    }
}

/// Feet and inches concatenated, rounding down to a whole inch (DW-59):
/// 36" is `30`, 80" is `68`, 32" is `28`.
pub fn size_shorthand(inches: f64) -> String {
    let total = (inches + 1e-6).floor().max(0.0) as i64;
    format!("{}{}", total / 12, total % 12)
}

impl Opening {
    pub fn new(
        wall_id: Id,
        center_offset: f64,
        kind: OpeningKind,
        width: f64,
        height: f64,
        sill_height: f64,
    ) -> Opening {
        Opening {
            id: 0,
            wall_id,
            center_offset,
            width,
            height,
            sill_height,
            kind,
            swing_flipped: false,
            hinge_at_end: false,
            style: OpeningStyle::default_for(kind),
            label_override: None,
            schedule_number: None,
            casing: None,
            lites: default_lites(),
            egress: false,
            tempered: false,
            extras: crate::extras::OpeningExtras::default(),
        }
    }

    /// Chief's automatic label (DW-59): width then height as concatenated
    /// feet-and-inches digits, e.g. `3068` for a 36" x 80" door.
    pub fn auto_label(&self) -> String {
        format!(
            "{}{}",
            size_shorthand(self.width),
            size_shorthand(self.height)
        )
    }

    /// The label to draw: the user's override (DW-62) or the automatic one.
    pub fn label(&self) -> String {
        self.label_override
            .clone()
            .unwrap_or_else(|| self.auto_label())
    }
}

impl Default for Opening {
    fn default() -> Self {
        Opening::new(0, 0.0, OpeningKind::Door, 36.0, 80.0, 0.0)
    }
}

/// Clamp `center` like `Project::add_opening`: the jambs stay at least the
/// margin away from the wall ends. `None` if the wall is too short.
pub fn clamp_opening_center(wall_len: f64, width: f64, center: f64) -> Option<f64> {
    if wall_len < width + 2.0 * OPENING_MARGIN {
        return None;
    }
    let half = width * 0.5;
    Some(center.clamp(half + OPENING_MARGIN, wall_len - half - OPENING_MARGIN))
}

/// Door placement defaults from the pointer (DW-8, DW-76): returns
/// `(swing_flipped, hinge_at_end)`. The door swings toward the side of the
/// wall the pointer is on (`swing_flipped` is false for the wall's left/normal
/// side, true for the right; a pointer on the centerline keeps the left), and
/// the hinge goes to the jamb nearer the closer wall end.
pub fn door_defaults_for_pointer(
    wall: &crate::model::Wall,
    pointer: Point,
    center_offset: f64,
) -> (bool, bool) {
    let n = wall.normal();
    let rel = pointer.sub(wall.start);
    let side = rel.x * n.x + rel.y * n.y;
    (side < 0.0, center_offset > wall.length() * 0.5)
}

impl Project {
    /// Reverse Swing (DW-32): the door opens to the other side of the wall,
    /// hinge unchanged. Returns `false` for an unknown opening.
    pub fn flip_swing(&mut self, floor: usize, id: Id) -> bool {
        match self.floors[floor].openings.iter_mut().find(|o| o.id == id) {
            Some(o) => {
                o.swing_flipped = !o.swing_flipped;
                true
            }
            None => false,
        }
    }

    /// Flip Hinge (DW-32): move the hinge to the other jamb, swing side
    /// unchanged. Returns `false` for an unknown opening.
    pub fn flip_hinge(&mut self, floor: usize, id: Id) -> bool {
        match self.floors[floor].openings.iter_mut().find(|o| o.id == id) {
            Some(o) => {
                o.hinge_at_end = !o.hinge_at_end;
                true
            }
            None => false,
        }
    }

    /// Slide an opening along its wall to `new_center`, clamped like
    /// [`Project::add_opening`]. Returns `false` (and changes nothing) if the
    /// opening is unknown, the wall is too short, or the new spot overlaps
    /// another opening.
    pub fn slide_opening(&mut self, floor: usize, id: Id, new_center: f64) -> bool {
        let f = &mut self.floors[floor];
        let Some(cur) = f.openings.iter().find(|o| o.id == id).cloned() else {
            return false;
        };
        let Some(wall_len) = f.wall(cur.wall_id).map(|w| w.length()) else {
            return false;
        };
        let Some(center) = clamp_opening_center(wall_len, cur.width, new_center) else {
            return false;
        };
        let probe = Opening {
            center_offset: center,
            ..cur.clone()
        };
        let overlaps = f.openings.iter().any(|o| {
            o.id != id
                && o.wall_id == cur.wall_id
                && probe.start_offset() < o.end_offset() + OPENING_MARGIN
                && probe.end_offset() > o.start_offset() - OPENING_MARGIN
        });
        if overlaps {
            return false;
        }
        if let Some(o) = f.openings.iter_mut().find(|o| o.id == id) {
            o.center_offset = center;
        }
        true
    }

    /// The plan position of the opening center on its wall centerline.
    pub fn opening_position(&self, floor: usize, id: Id) -> Option<Point> {
        let f = &self.floors[floor];
        let o = f.openings.iter().find(|o| o.id == id)?;
        Some(f.wall(o.wall_id)?.point_at(o.center_offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WallKind, DEFAULT_CEILING_HEIGHT};

    fn proj() -> (Project, Id) {
        let mut p = Project::new("o");
        let w = p.add_wall(
            0,
            Point::ZERO,
            Point::new(200.0, 0.0),
            4.5,
            DEFAULT_CEILING_HEIGHT,
            WallKind::Interior,
        );
        (p, w)
    }

    #[test]
    fn auto_label_is_chief_shorthand() {
        let d = Opening::default_door(1, 1, 50.0);
        assert_eq!(d.auto_label(), "3068");
        let o = Opening::new(1, 0.0, OpeningKind::Door, 32.0, 80.0, 0.0);
        assert_eq!(o.auto_label(), "2868");
        let w = Opening::default_window(2, 1, 50.0);
        assert_eq!(w.auto_label(), "3050");
        let g = Opening::new(1, 0.0, OpeningKind::Door, 192.0, 84.0, 0.0);
        assert_eq!(g.auto_label(), "16070");
        // Fractions round down to the next-lower inch.
        let f = Opening::new(1, 0.0, OpeningKind::Door, 35.75, 79.9, 0.0);
        assert_eq!(f.auto_label(), "21167");
        let mut l = Opening::default();
        assert_eq!(l.label(), "3068");
        l.label_override = Some("A".into());
        assert_eq!(l.label(), "A");
        assert_eq!(l.auto_label(), "3068");
    }

    #[test]
    fn style_defaults_by_kind() {
        assert_eq!(Opening::default_door(1, 1, 0.0).style, OpeningStyle::Hinged);
        assert_eq!(
            Opening::default_window(1, 1, 0.0).style,
            OpeningStyle::Window
        );
        assert!(OpeningStyle::Pocket.is_door_style());
        assert!(!OpeningStyle::BayWindow.is_door_style());
        let d = Opening::default();
        assert_eq!((d.lites, d.egress, d.tempered), ((1, 1), false, false));
    }

    #[test]
    fn old_json_gets_kind_based_style() {
        let w: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":60.0,
                "sill_height":24.0,"kind":"Window","swing_flipped":false}"#,
        )
        .unwrap();
        assert_eq!(w.style, OpeningStyle::Window);
        assert!(!w.hinge_at_end && w.casing.is_none());
        assert_eq!(w.lites, (1, 1));
        let d: Opening = serde_json::from_str(
            r#"{"id":1,"wall_id":2,"center_offset":50.0,"width":36.0,"height":80.0,
                "sill_height":0.0,"kind":"Door","swing_flipped":true}"#,
        )
        .unwrap();
        assert_eq!(d.style, OpeningStyle::Hinged);
        assert!(d.swing_flipped);
        // And a full round trip keeps the new fields.
        let x = Opening {
            hinge_at_end: true,
            casing: Some(Casing::default()),
            schedule_number: Some("1".into()),
            ..Opening::default()
        };
        let back: Opening = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert!(back.hinge_at_end);
        assert_eq!(back.casing, Some(Casing::default()));
        assert_eq!(back.schedule_number.as_deref(), Some("1"));
    }

    #[test]
    fn flips_are_independent() {
        let (mut p, w) = proj();
        let d = p.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        assert!(p.flip_swing(0, d));
        let o = &p.floors[0].openings[0];
        assert!(o.swing_flipped && !o.hinge_at_end);
        assert!(p.flip_hinge(0, d));
        let o = &p.floors[0].openings[0];
        assert!(o.swing_flipped && o.hinge_at_end);
        assert!(p.flip_swing(0, d));
        assert!(!p.floors[0].openings[0].swing_flipped);
        assert!(!p.flip_swing(0, 999) && !p.flip_hinge(0, 999));
    }

    #[test]
    fn slide_clamps_and_rejects_overlap() {
        let (mut p, w) = proj();
        let a = p.add_opening(0, w, 40.0, OpeningKind::Door).unwrap();
        let b = p.add_opening(0, w, 150.0, OpeningKind::Door).unwrap();
        // Clamped to half-width + margin from the start.
        assert!(p.slide_opening(0, a, -50.0));
        let oa = p.floors[0].openings.iter().find(|o| o.id == a).unwrap();
        assert!((oa.center_offset - 20.0).abs() < 1e-9);
        // Onto the other door: refused, unchanged.
        assert!(!p.slide_opening(0, a, 150.0));
        let oa = p.floors[0].openings.iter().find(|o| o.id == a).unwrap();
        assert!((oa.center_offset - 20.0).abs() < 1e-9);
        // Sliding to its own current spot is fine; past the end clamps.
        assert!(p.slide_opening(0, b, 1000.0));
        let ob = p.floors[0].openings.iter().find(|o| o.id == b).unwrap();
        assert!((ob.center_offset - 180.0).abs() < 1e-9);
        assert!(!p.slide_opening(0, 999, 10.0));
        assert!(p.opening_position(0, b).is_some());
    }
}
