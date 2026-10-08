//! The cabinet data model, Chief-style defaults, labels and wall runs.

use plan_core::geometry::Point;
use plan_core::Id;
use serde::{Deserialize, Serialize};

use crate::face::FaceLayout;

/// What kind of cabinet this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CabinetKind {
    Base,
    Wall,
    FullHeight,
    Soffit,
    Shelf,
    Partition,
}

/// Countertop slab sitting on top of the cabinet (inside its height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Countertop {
    pub thickness: f64,
    pub overhang_front: f64,
    pub overhang_sides: f64,
    pub overhang_back: f64,
}

impl Default for Countertop {
    /// Chief's 1 1/2" top with a 1" front overhang. Sides and back are flush
    /// (Chief's dialog defaults to 1" all round, which would collide in a run).
    fn default() -> Self {
        Self {
            thickness: 1.5,
            overhang_front: 1.0,
            overhang_sides: 0.0,
            overhang_back: 0.0,
        }
    }
}

/// Backsplash standing on the countertop along the back edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Backsplash {
    pub height: f64,
    pub thickness: f64,
}

/// Recessed toe kick under a base cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToeKick {
    pub height: f64,
    /// How far the kick is set back from the front of the cabinet.
    pub depth: f64,
}

impl Default for ToeKick {
    /// Chief's 4" high, 3" deep kick.
    fn default() -> Self {
        Self {
            height: 4.0,
            depth: 3.0,
        }
    }
}

/// Handle (pull/knob) style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleStyle {
    None,
    Knob,
    Pull,
}

/// Door panel and handle settings (Chief's Door/Drawer tab).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoorStyle {
    pub name: String,
    pub thickness: f64,
    pub glass: bool,
    pub handle: HandleStyle,
    /// Handle distance from the top (base/full height) or bottom (wall) of the door.
    pub handle_from_top: f64,
    /// Handle distance from the free edge of the door.
    pub handle_from_edge: f64,
}

impl Default for DoorStyle {
    fn default() -> Self {
        Self {
            name: "Lincoln Door".to_string(),
            thickness: 0.75,
            glass: false,
            handle: HandleStyle::Knob,
            handle_from_top: 1.375,
            handle_from_edge: 1.375,
        }
    }
}

/// Drawer front and handle settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawerStyle {
    pub name: String,
    pub thickness: f64,
    pub handle: HandleStyle,
}

impl Default for DrawerStyle {
    fn default() -> Self {
        Self {
            name: "Lincoln Flat Panel Drawer".to_string(),
            thickness: 0.75,
            handle: HandleStyle::Knob,
        }
    }
}

/// How fronts sit relative to the opening (Chief's Door/Drawer Overlay).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Overlay {
    /// Fronts cover the box; `reveal` is the gap between neighbours.
    Full { reveal: f64 },
    /// Fronts overlap the opening by `overlap` on every side.
    Traditional { overlap: f64 },
    /// Fronts sit inside the opening with `clearance` all round.
    Inset { clearance: f64 },
}

impl Default for Overlay {
    /// Chief's default: full overlay with a 1/16" reveal.
    fn default() -> Self {
        Overlay::Full { reveal: 0.0625 }
    }
}

/// A parametric cabinet. See the crate docs for the local frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cabinet {
    pub id: Id,
    pub kind: CabinetKind,
    /// Plan position of the local origin (back-left corner).
    pub position: Point,
    /// Counter-clockwise rotation in radians. At `0` the back faces -Y and the
    /// front faces +Y.
    pub angle: f64,
    pub width: f64,
    pub depth: f64,
    /// Overall height, including the countertop.
    pub height: f64,
    /// Bottom of the cabinet above the floor (0 for base, 54 for wall).
    pub elevation: f64,
    pub countertop: Option<Countertop>,
    pub backsplash: Option<Backsplash>,
    pub toe_kick: Option<ToeKick>,
    pub face: FaceLayout,
    pub door_style: DoorStyle,
    pub drawer_style: DrawerStyle,
    pub overlay: Overlay,
    /// Face-frame (stiles and rails) construction rather than frameless.
    pub framed: bool,
    /// Optional label override; empty means [`auto_label`].
    pub label: String,
}

impl Cabinet {
    fn blank(kind: CabinetKind, width: f64, depth: f64, height: f64, elevation: f64) -> Self {
        Self {
            id: 0,
            kind,
            position: Point::ZERO,
            angle: 0.0,
            width,
            depth,
            height,
            elevation,
            countertop: None,
            backsplash: None,
            toe_kick: None,
            face: FaceLayout::empty(),
            door_style: DoorStyle::default(),
            drawer_style: DrawerStyle::default(),
            overlay: Overlay::default(),
            framed: true,
            label: String::new(),
        }
    }

    /// A Chief default base cabinet: 24" deep, 36" high including a 1 1/2"
    /// top, 4" x 3" toe kick, drawer over door face.
    pub fn base(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::Base, width, 24.0, 36.0, 0.0);
        c.countertop = Some(Countertop::default());
        c.toe_kick = Some(ToeKick::default());
        c.face = FaceLayout::base_default(c.face_height());
        c
    }

    /// A Chief default wall cabinet: 12" deep, 30" high at 54".
    pub fn wall(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::Wall, width, 12.0, 30.0, 54.0);
        c.face = FaceLayout::wall_default(c.face_height());
        c
    }

    /// A Chief default full-height (pantry) cabinet: 24" deep, 84" high.
    pub fn full_height(width: f64) -> Self {
        let mut c = Self::blank(CabinetKind::FullHeight, width, 24.0, 84.0, 0.0);
        c.toe_kick = Some(ToeKick::default());
        c.face = FaceLayout::full_height_default(c.face_height());
        c
    }

    /// A base cabinet with a sink face (B36-SB style label).
    pub fn sink_base(width: f64) -> Self {
        let mut c = Self::base(width);
        c.face = FaceLayout::sink_base();
        c
    }

    /// A cabinet of any kind with sensible defaults: base, wall and full
    /// height use their Chief defaults; the rest are bare boxes (soffit 12x12
    /// at 84", shelf 12" deep at 48", partition 24" deep x 36" high).
    pub fn new(kind: CabinetKind, width: f64) -> Self {
        match kind {
            CabinetKind::Base => Self::base(width),
            CabinetKind::Wall => Self::wall(width),
            CabinetKind::FullHeight => Self::full_height(width),
            CabinetKind::Soffit => Self::blank(kind, width, 12.0, 12.0, 84.0),
            CabinetKind::Shelf => Self::blank(kind, width, 12.0, 0.75, 48.0),
            CabinetKind::Partition => Self::blank(kind, width, 24.0, 36.0, 0.0),
        }
    }

    /// Height available to the face: above the toe kick, below the countertop.
    pub fn face_height(&self) -> f64 {
        let toe = self.toe_kick.map_or(0.0, |t| t.height);
        let top = self.countertop.map_or(0.0, |c| c.thickness);
        (self.height - toe - top).max(0.0)
    }

    /// Map a local-frame point (inches) to plan coordinates.
    pub fn to_plan(&self, local: Point) -> Point {
        let (s, c) = self.angle.sin_cos();
        Point::new(
            self.position.x + local.x * c - local.y * s,
            self.position.y + local.x * s + local.y * c,
        )
    }

    /// The box footprint in plan, counter-clockwise from the back-left corner.
    pub fn corners(&self) -> [Point; 4] {
        [
            self.to_plan(Point::new(0.0, 0.0)),
            self.to_plan(Point::new(self.width, 0.0)),
            self.to_plan(Point::new(self.width, self.depth)),
            self.to_plan(Point::new(0.0, self.depth)),
        ]
    }
}

/// Format a dimension as a whole number, or with up to two trimmed decimals.
fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Industry-style label: `B24`, `B36-SB` (sink base), `W3030` (width then
/// height), `FH2484`, `SO..`, `SH..`, `PT..`.
pub fn auto_label(cabinet: &Cabinet) -> String {
    let w = num(cabinet.width);
    match cabinet.kind {
        CabinetKind::Base if cabinet.face.has_appliance("Sink") => format!("B{w}-SB"),
        CabinetKind::Base => format!("B{w}"),
        CabinetKind::Wall => format!("W{w}{}", num(cabinet.height)),
        CabinetKind::FullHeight => format!("FH{w}{}", num(cabinet.height)),
        CabinetKind::Soffit => format!("SO{w}"),
        CabinetKind::Shelf => format!("SH{w}"),
        CabinetKind::Partition => format!("PT{w}"),
    }
}

/// Place a row of cabinets along a wall on its +side (the left of the
/// `wall_start` to `wall_end` direction), backs against the wall face.
///
/// `wall_thickness` is the wall's full thickness and the start/end points are
/// its centreline, so the backs sit `wall_thickness / 2` off the centreline.
/// Cabinets are laid end to end from `wall_start`; the run is not clipped to
/// the wall length. Ids are left at `0` for the caller to assign.
pub fn run_along_wall(
    wall_start: Point,
    wall_end: Point,
    wall_thickness: f64,
    widths: &[f64],
    kind: CabinetKind,
) -> Vec<Cabinet> {
    let dir = wall_end.sub(wall_start).normalized();
    let normal = dir.perp();
    let angle = if dir == Point::ZERO { 0.0 } else { dir.angle() };
    let origin = wall_start.add(normal.scale(wall_thickness / 2.0));
    let mut offset = 0.0;
    widths
        .iter()
        .map(|&w| {
            let mut c = Cabinet::new(kind, w);
            c.position = origin.add(dir.scale(offset));
            c.angle = angle;
            offset += w;
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_defaults_match_chief() {
        let c = Cabinet::base(24.0);
        assert_eq!(
            (c.width, c.depth, c.height, c.elevation),
            (24.0, 24.0, 36.0, 0.0)
        );
        assert_eq!(
            c.toe_kick,
            Some(ToeKick {
                height: 4.0,
                depth: 3.0
            })
        );
        let top = c.countertop.unwrap();
        assert_eq!((top.thickness, top.overhang_front), (1.5, 1.0));
        // Box height under the 1.5" top.
        assert_eq!(c.height - top.thickness, 34.5);
        assert_eq!(c.overlay, Overlay::Full { reveal: 0.0625 });
        assert!(c.framed);
    }

    #[test]
    fn wall_and_full_height_defaults() {
        let w = Cabinet::wall(30.0);
        assert_eq!((w.depth, w.height, w.elevation), (12.0, 30.0, 54.0));
        assert!(w.countertop.is_none() && w.toe_kick.is_none());
        let f = Cabinet::full_height(24.0);
        assert_eq!((f.depth, f.height), (24.0, 84.0));
        assert!(f.face.resolve(f.face_height(), 21.0).is_ok());
    }

    #[test]
    fn labels() {
        assert_eq!(auto_label(&Cabinet::base(36.0)), "B36");
        assert_eq!(auto_label(&Cabinet::sink_base(36.0)), "B36-SB");
        assert_eq!(auto_label(&Cabinet::wall(30.0)), "W3030");
        assert_eq!(auto_label(&Cabinet::full_height(24.0)), "FH2484");
        assert_eq!(auto_label(&Cabinet::base(37.5)), "B37.5");
    }

    #[test]
    fn run_of_three_covers_84_without_overlap() {
        let run = run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            &[24.0, 36.0, 24.0],
            CabinetKind::Base,
        );
        assert_eq!(run.len(), 3);
        // Backs on the +Y wall face (centreline + 3").
        assert!(run
            .iter()
            .all(|c| (c.position.y - 3.0).abs() < 1e-9 && c.angle == 0.0));
        let spans: Vec<(f64, f64)> = run
            .iter()
            .map(|c| (c.position.x, c.position.x + c.width))
            .collect();
        for pair in spans.windows(2) {
            assert!((pair[0].1 - pair[1].0).abs() < 1e-9, "gap or overlap");
        }
        assert_eq!(spans[0].0, 0.0);
        assert!((spans[2].1 - 84.0).abs() < 1e-9);
    }

    #[test]
    fn run_on_vertical_wall_faces_plus_side() {
        // Wall runs +Y; its +side (left) is -X, so fronts must face -X.
        let run = run_along_wall(
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            4.0,
            &[24.0],
            CabinetKind::Base,
        );
        let c = &run[0];
        let front = c.to_plan(Point::new(0.0, 1.0)).sub(c.position);
        assert!((front.x + 1.0).abs() < 1e-9 && front.y.abs() < 1e-9);
        assert!((c.position.x + 2.0).abs() < 1e-9);
    }

    #[test]
    fn serde_round_trip() {
        let mut c = Cabinet::sink_base(36.0);
        c.angle = 0.5;
        c.position = Point::new(12.0, -3.5);
        c.overlay = Overlay::Inset { clearance: 0.0625 };
        c.label = "X".into();
        let json = serde_json::to_string(&c).unwrap();
        let back: Cabinet = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
