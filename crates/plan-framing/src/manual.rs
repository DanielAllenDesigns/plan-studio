//! Manually placed framing members (Chief's General, Floor/Ceiling and Roof
//! Framing tools) as pure data, with defaults, oriented 3D boxes, takeoff and
//! a material list.
//!
//! A [`FramingMember`] is described in plan: `start`/`end` points, a bottom
//! elevation, a section (`width` x `depth`) and an optional roll. The 3D frame
//! matches `plan-3d`: X right, Y up, Z = -plan y.

use crate::lumber::{format_inches, Lumber};
use crate::member::{add, cross, scale, Member, MemberKind as AutoKind, Transform3, Vec3};
use crate::takeoff::Takeoff;
use crate::truss::{Truss, TrussSpec, TrussType};
use plan_3d::Mesh;
use plan_core::{Id, Point};
use serde::{Deserialize, Serialize};

/// Subfloor thickness between joist tops and the finished floor.
pub(crate) const SUBFLOOR: f64 = 0.75;
const EPS: f64 = 1e-9;
const UP: Vec3 = [0.0, 1.0, 0.0];

/// Actual (dressed) size of a nominal dimension.
fn dress(nominal: u32) -> f64 {
    match nominal {
        1 => 0.75,
        2 => 1.5,
        3 => 2.5,
        4 => 3.5,
        5 => 4.5,
        6 => 5.5,
        8 => 7.25,
        10 => 9.25,
        12 => 11.25,
        14 => 13.25,
        16 => 15.25,
        n => f64::from(n) - 0.5,
    }
}

/// What a member is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FramingMaterial {
    Lumber,
    Steel,
    Glulam,
    Lvl,
    Psl,
}

impl FramingMaterial {
    pub fn name(&self) -> &'static str {
        match self {
            FramingMaterial::Lumber => "Lumber",
            FramingMaterial::Steel => "Steel",
            FramingMaterial::Glulam => "Glulam",
            FramingMaterial::Lvl => "LVL",
            FramingMaterial::Psl => "PSL",
        }
    }
}

/// A framing section: dimensional lumber by nominal size, or an engineered
/// beam by actual width x depth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LumberSize {
    /// Nominal thickness x depth, e.g. `Dim { thickness: 2, depth: 10 }`.
    Dim { thickness: u32, depth: u32 },
    /// Glued-laminated beam, actual inches.
    Glulam { width: f64, depth: f64 },
    /// Laminated veneer lumber, actual inches.
    Lvl { width: f64, depth: f64 },
}

impl LumberSize {
    pub const TWO_BY_FOUR: LumberSize = LumberSize::dim(2, 4);
    pub const TWO_BY_SIX: LumberSize = LumberSize::dim(2, 6);
    pub const TWO_BY_EIGHT: LumberSize = LumberSize::dim(2, 8);
    pub const TWO_BY_TEN: LumberSize = LumberSize::dim(2, 10);
    pub const TWO_BY_TWELVE: LumberSize = LumberSize::dim(2, 12);
    pub const FOUR_BY_FOUR: LumberSize = LumberSize::dim(4, 4);
    pub const FOUR_BY_TEN: LumberSize = LumberSize::dim(4, 10);
    pub const SIX_BY_SIX: LumberSize = LumberSize::dim(6, 6);

    pub const fn dim(thickness: u32, depth: u32) -> Self {
        LumberSize::Dim { thickness, depth }
    }

    /// Nominal thickness: the "2" of a 2x10, the width rounded for an engineered beam.
    pub fn nominal_thickness(&self) -> u32 {
        match *self {
            LumberSize::Dim { thickness, .. } => thickness,
            LumberSize::Glulam { width, .. } | LumberSize::Lvl { width, .. } => {
                width.round().max(1.0) as u32
            }
        }
    }

    /// Nominal depth: the "10" of a 2x10, the depth rounded for an engineered beam.
    pub fn nominal_depth(&self) -> u32 {
        match *self {
            LumberSize::Dim { depth, .. } => depth,
            LumberSize::Glulam { depth, .. } | LumberSize::Lvl { depth, .. } => {
                depth.round().max(1.0) as u32
            }
        }
    }

    /// Actual section width (the smaller, "thickness" side).
    pub fn width(&self) -> f64 {
        match *self {
            LumberSize::Dim { thickness, .. } => dress(thickness),
            LumberSize::Glulam { width, .. } | LumberSize::Lvl { width, .. } => width,
        }
    }

    /// Actual section depth.
    pub fn depth(&self) -> f64 {
        match *self {
            LumberSize::Dim { depth, .. } => dress(depth),
            LumberSize::Glulam { depth, .. } | LumberSize::Lvl { depth, .. } => depth,
        }
    }

    /// Cross-section area used for board feet: nominal for dimensional lumber
    /// (the lumber-yard convention), actual for engineered beams.
    pub fn board_area(&self) -> f64 {
        match *self {
            LumberSize::Dim { thickness, depth } => f64::from(thickness * depth),
            _ => self.width() * self.depth(),
        }
    }

    pub fn material(&self) -> FramingMaterial {
        match self {
            LumberSize::Dim { .. } => FramingMaterial::Lumber,
            LumberSize::Glulam { .. } => FramingMaterial::Glulam,
            LumberSize::Lvl { .. } => FramingMaterial::Lvl,
        }
    }

    /// `"2x10"`, `"GLB 5 1/8x12"`, `"LVL 1 3/4x11 7/8"`.
    pub fn name(&self) -> String {
        match self {
            LumberSize::Dim { thickness, depth } => format!("{thickness}x{depth}"),
            LumberSize::Glulam { width, depth } => {
                format!("GLB {}x{}", format_inches(*width), format_inches(*depth))
            }
            LumberSize::Lvl { width, depth } => {
                format!("LVL {}x{}", format_inches(*width), format_inches(*depth))
            }
        }
    }

    /// The same section as an automatic-framing [`Lumber`].
    pub fn to_lumber(&self) -> Lumber {
        Lumber {
            thickness: self.width(),
            depth: self.depth(),
        }
    }
}

/// The manually placed member types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemberKind {
    GeneralFraming,
    Post,
    PostWithFooting,
    Blocking,
    Joist,
    JoistBlocking,
    FloorCeilingBeam,
    FloorCeilingTruss,
    /// A bearing line is layout data, not lumber: it has no boxes or takeoff.
    BearingLine,
    Rafter,
    RoofBeam,
    RoofBlocking,
    RoofPurlin,
    RoofTruss,
    GirderTruss,
    /// A truss base segment is layout data, not lumber.
    TrussBase,
}

impl MemberKind {
    pub fn name(&self) -> &'static str {
        match self {
            MemberKind::GeneralFraming => "framing",
            MemberKind::Post => "post",
            MemberKind::PostWithFooting => "post with footing",
            MemberKind::Blocking => "blocking",
            MemberKind::Joist => "joist",
            MemberKind::JoistBlocking => "joist blocking",
            MemberKind::FloorCeilingBeam => "floor/ceiling beam",
            MemberKind::FloorCeilingTruss => "floor/ceiling truss",
            MemberKind::BearingLine => "bearing line",
            MemberKind::Rafter => "rafter",
            MemberKind::RoofBeam => "roof beam",
            MemberKind::RoofBlocking => "roof blocking",
            MemberKind::RoofPurlin => "roof purlin",
            MemberKind::RoofTruss => "roof truss",
            MemberKind::GirderTruss => "girder truss",
            MemberKind::TrussBase => "truss base",
        }
    }

    /// Posts stand on end: `start == end` and the length is the height.
    pub fn is_vertical(&self) -> bool {
        matches!(self, MemberKind::Post | MemberKind::PostWithFooting)
    }

    pub fn is_truss(&self) -> bool {
        matches!(
            self,
            MemberKind::FloorCeilingTruss | MemberKind::RoofTruss | MemberKind::GirderTruss
        )
    }

    /// Whether the member is built from material (false for layout lines).
    pub fn is_physical(&self) -> bool {
        !matches!(self, MemberKind::BearingLine | MemberKind::TrussBase)
    }

    /// Chief-style default section: joists 2x10, rafters 2x8, posts 4x4,
    /// beams 4x10, trusses 2x4.
    pub fn default_lumber(&self) -> LumberSize {
        match self {
            MemberKind::GeneralFraming | MemberKind::Blocking => LumberSize::TWO_BY_FOUR,
            MemberKind::Post | MemberKind::PostWithFooting => LumberSize::FOUR_BY_FOUR,
            MemberKind::Joist | MemberKind::JoistBlocking => LumberSize::TWO_BY_TEN,
            MemberKind::FloorCeilingBeam | MemberKind::RoofBeam => LumberSize::FOUR_BY_TEN,
            MemberKind::Rafter | MemberKind::RoofBlocking => LumberSize::TWO_BY_EIGHT,
            MemberKind::RoofPurlin => LumberSize::TWO_BY_SIX,
            MemberKind::FloorCeilingTruss
            | MemberKind::RoofTruss
            | MemberKind::GirderTruss
            | MemberKind::BearingLine
            | MemberKind::TrussBase => LumberSize::TWO_BY_FOUR,
        }
    }

    /// Default on-centre spacing for auto layout, inches (`0.0` = not laid out).
    pub fn default_spacing(&self) -> f64 {
        match self {
            MemberKind::Joist | MemberKind::JoistBlocking | MemberKind::GeneralFraming => 16.0,
            MemberKind::Rafter
            | MemberKind::FloorCeilingTruss
            | MemberKind::RoofTruss
            | MemberKind::GirderTruss => 24.0,
            _ => 0.0,
        }
    }

    /// Default post height, inches (8').
    fn default_height(&self) -> f64 {
        if self.is_vertical() {
            96.0
        } else {
            0.0
        }
    }
}

/// Footing under a post.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FootingSpec {
    /// Side of the square footing.
    pub size: f64,
    pub thickness: f64,
}

impl Default for FootingSpec {
    /// 24" square, 12" thick.
    fn default() -> Self {
        Self {
            size: 24.0,
            thickness: 12.0,
        }
    }
}

/// A square concrete footing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Footing {
    pub center: Point,
    pub size: f64,
    pub thickness: f64,
    /// Top of the footing.
    pub elevation: f64,
}

impl Footing {
    pub fn bottom_elevation(&self) -> f64 {
        self.elevation - self.thickness
    }

    /// Concrete volume, cubic yards.
    pub fn volume_cubic_yards(&self) -> f64 {
        self.size * self.size * self.thickness / 46_656.0
    }

    pub fn to_box(&self) -> OrientedBox {
        OrientedBox::from_axes(
            [
                self.center.x,
                self.elevation - self.thickness / 2.0,
                -self.center.y,
            ],
            [self.thickness, self.size, self.size],
            [UP, [1.0, 0.0, 0.0], cross(UP, [1.0, 0.0, 0.0])],
        )
    }
}

/// A box in the 3D frame: `size` is the extent along each of `axes`
/// (`[length, depth, width]`), the axes are unit and right-handed
/// (`axes[2] = axes[0] x axes[1]`), and `rotation` is `[yaw, pitch, roll]` in
/// radians derived from them (yaw about +Y from +X toward -Z, pitch up from
/// horizontal, roll about the length axis).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrientedBox {
    pub center: Vec3,
    pub size: Vec3,
    pub axes: [Vec3; 3],
    pub rotation: Vec3,
}

impl OrientedBox {
    /// Build a box, deriving `rotation` from the axes.
    pub fn from_axes(center: Vec3, size: Vec3, axes: [Vec3; 3]) -> Self {
        let a = axes[0];
        let horizontal = a[0].hypot(a[2]);
        let (yaw, pitch, up0) = if horizontal < 1e-9 {
            // Standing on end: the heading comes from the depth axis.
            (
                (-axes[1][2]).atan2(axes[1][0]),
                std::f64::consts::FRAC_PI_2,
                [-axes[1][0], 0.0, -axes[1][2]],
            )
        } else {
            let (hx, hz) = (a[0] / horizontal, a[2] / horizontal);
            let s = a[1];
            let c = horizontal;
            ((-a[2]).atan2(a[0]), s.atan2(c), [-hx * s, c, -hz * s])
        };
        let z0 = cross(a, up0);
        let roll = if horizontal < 1e-9 {
            0.0
        } else {
            let y = axes[1];
            (y[0] * z0[0] + y[1] * z0[1] + y[2] * z0[2])
                .atan2(y[0] * up0[0] + y[1] * up0[1] + y[2] * up0[2])
        };
        Self {
            center,
            size,
            axes,
            rotation: [yaw, pitch, roll],
        }
    }

    /// Box volume, cubic inches.
    pub fn volume(&self) -> f64 {
        self.size[0] * self.size[1] * self.size[2]
    }

    /// A 24-vertex box mesh (the same one [`Member::mesh`] builds).
    pub fn mesh(&self, object_id: Option<Id>) -> Mesh {
        let origin = add(self.center, scale(self.axes[0], -self.size[0] / 2.0));
        Member::new(
            AutoKind::Blocking,
            Lumber {
                thickness: self.size[2],
                depth: self.size[1],
            },
            self.size[0],
            Transform3 {
                origin,
                axis_x: self.axes[0],
                axis_y: self.axes[1],
            },
            object_id,
        )
        .mesh()
    }
}

/// The Rotate option of a framing member in a wall (manual p. 929): which
/// way the member lies in the wall's framing layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlatTo {
    /// The thickness spans the depth of the framing layer.
    #[default]
    None,
    /// Turned 90 degrees and aligned with the inside of the framing layer.
    Inside,
    /// Turned 90 degrees and aligned with the outside of the framing layer.
    Outside,
}

impl FlatTo {
    pub const ALL: [FlatTo; 3] = [FlatTo::None, FlatTo::Inside, FlatTo::Outside];

    pub fn name(self) -> &'static str {
        match self {
            FlatTo::None => "None",
            FlatTo::Inside => "Flat to Inside",
            FlatTo::Outside => "Flat to Outside",
        }
    }
}

/// The shape of a decorative end profile (manual p. 930).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EndShape {
    #[default]
    Square,
    /// The two edges of the end cut off at 45 degrees.
    Chamfer,
    /// The end rounded off over its full depth.
    Round,
    /// The end cut away on a slope on its top edge.
    Taper,
}

impl EndShape {
    pub const ALL: [EndShape; 4] = [
        EndShape::Square,
        EndShape::Chamfer,
        EndShape::Round,
        EndShape::Taper,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EndShape::Square => "Square",
            EndShape::Chamfer => "Chamfer",
            EndShape::Round => "Round",
            EndShape::Taper => "Taper",
        }
    }
}

/// The End Profile of one end of a joist, beam, rafter or General Framing
/// member: a shape and how far it cuts in.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EndProfile {
    pub shape: EndShape,
    /// How far along the member the profile reaches, inches.
    pub size: f64,
}

/// A manually placed framing member.
///
/// * Linear members run `start` -> `end` in plan; `rise` raises the end above
///   the start (rafters, purlins on a slope). `elevation_bottom` is the
///   underside at `start`.
/// * Posts have `start == end`; `height` is their length and `rotation` turns
///   the section in plan.
/// * `rotation` on linear members rolls the section about its axis (degrees;
///   `0` = on edge, `90` = laid flat).
/// * Truss kinds run bearing to bearing; `truss` holds the geometry inputs
///   (its span is replaced by the member length when generating).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramingMember {
    pub id: Id,
    pub kind: MemberKind,
    pub start: Point,
    pub end: Point,
    pub elevation_bottom: f64,
    /// Section depth (vertical for a member on edge).
    pub depth: f64,
    /// Section width (the thin side; all plies together for a girder).
    pub width: f64,
    /// Degrees; see the type docs.
    pub rotation: f64,
    /// End elevation minus start elevation (linear members).
    pub rise: f64,
    /// Length of a post.
    pub height: f64,
    pub lumber: LumberSize,
    pub material: FramingMaterial,
    /// Plies side by side (girder trusses, built-up beams).
    pub plies: u32,
    pub truss: Option<TrussSpec>,
    /// Footing under a [`MemberKind::PostWithFooting`].
    pub footing_spec: FootingSpec,
    pub label: String,
    pub layer_name: String,
    /// Bearing Beam (floor/ceiling beams): joists run across it and lap or
    /// butt over it, or hang on its sides when it stands 1" above them.
    #[serde(default)]
    pub bearing_beam: bool,
    /// Rotate: Flat to Inside or Outside of a wall's framing layer.
    #[serde(default)]
    pub flat: FlatTo,
    /// Show Cross: a post (or vertical member) draws as a cross box in plan.
    #[serde(default = "yes")]
    pub show_cross: bool,
    /// Counted as treated lumber in schedules and the Materials List.
    #[serde(default)]
    pub treated: bool,
    /// Show Multi-Ply Lines between the plies of a beam or post.
    #[serde(default)]
    pub show_ply_lines: bool,
    /// End Profile at the start and at the end.
    #[serde(default)]
    pub end_profile: [EndProfile; 2],
    /// A label the user typed (Label panel); empty uses the automatic label.
    #[serde(default)]
    pub custom_label: String,
    /// The fill in plan view (Fill Style panel); `None` follows the layer.
    #[serde(default)]
    pub fill: Option<plan_core::fill_styles::FillStyle>,
    /// The fill in a Wall Detail (Fill Style panel).
    #[serde(default)]
    pub detail_fill: Option<plan_core::fill_styles::FillStyle>,
    /// This end of the member was joined to another member with Join and Lap
    /// Ends (`Lap`) or Join and Mitre Ends (`Mitre`): start, end.
    #[serde(default)]
    pub joint: [JoinKind; 2],
    /// The Default Framing Member this member was given by Apply Framing
    /// Default Properties (empty: none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub member_def: String,
    /// Its Framing Type, by name (empty: the type of its Role's default).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub framing_type: String,
    /// Its Role when it is not the Role of its kind (lists only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::catalog::Role>,
}

fn yes() -> bool {
    true
}

/// How an end of a horizontal member was joined to another (manual p. 926).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum JoinKind {
    #[default]
    Free,
    /// The member butts against the other, which laps over its end.
    Butt,
    /// The other member laps this one's end.
    Lap,
    /// Both ends are cut to the angle between them.
    Mitre,
}

impl FramingMember {
    /// A member of `kind` with Chief-style defaults for its section, material
    /// and (for trusses) geometry.
    pub fn new(id: Id, kind: MemberKind, start: Point, end: Point) -> Self {
        let lumber = kind.default_lumber();
        let plies = if kind == MemberKind::GirderTruss {
            3
        } else {
            1
        };
        let truss = kind.is_truss().then(|| {
            let mut s = TrussSpec::new(TrussType::Fink, start.dist(end), 6.0);
            s.plies = plies;
            s
        });
        let mut m = Self {
            id,
            kind,
            start,
            end,
            elevation_bottom: 0.0,
            depth: lumber.depth(),
            width: lumber.width() * f64::from(plies),
            rotation: 0.0,
            rise: 0.0,
            height: kind.default_height(),
            lumber,
            material: lumber.material(),
            plies,
            truss,
            footing_spec: FootingSpec::default(),
            label: String::new(),
            layer_name: "Framing".into(),
            bearing_beam: false,
            flat: FlatTo::None,
            show_cross: true,
            treated: false,
            show_ply_lines: false,
            end_profile: [EndProfile::default(); 2],
            custom_label: String::new(),
            fill: None,
            detail_fill: None,
            joint: [JoinKind::Free; 2],
            member_def: String::new(),
            framing_type: String::new(),
            role: None,
        };
        m.label = m.default_label();
        m
    }

    /// Replace the section, keeping depth, width, material and label in step.
    pub fn with_lumber(mut self, lumber: LumberSize) -> Self {
        self.lumber = lumber;
        self.depth = lumber.depth();
        self.width = lumber.width() * f64::from(self.plies.max(1));
        self.material = lumber.material();
        self.label = self.default_label();
        self
    }

    pub fn at_elevation(mut self, elevation_bottom: f64) -> Self {
        self.elevation_bottom = elevation_bottom;
        self
    }

    fn default_label(&self) -> String {
        format!("{} {}", self.lumber.name(), self.kind.name())
    }

    /// Whether the member can be broken or joined: a straight horizontal or
    /// sloping piece, not a post or a truss.
    pub fn is_linear(&self) -> bool {
        self.kind.is_physical()
            && !self.kind.is_vertical()
            && !self.kind.is_truss()
            && self.plan_length() > 1e-6
    }

    /// Add Break (manual p. 290): the member cut in two at `at` (projected onto
    /// the member's line, at least 1" from each end). The first piece keeps
    /// this member's start, the second ends where this one does and gets
    /// `new_id`; the rise is shared out in proportion to length. `None` for a
    /// member that is not linear or a break too near an end.
    pub fn break_at(&self, at: Point, new_id: Id) -> Option<(FramingMember, FramingMember)> {
        if !self.is_linear() {
            return None;
        }
        let run = self.plan_length();
        let dir = (self.end - self.start).normalized();
        let s = (at - self.start).dot(dir);
        if s < 1.0 || s > run - 1.0 {
            return None;
        }
        let cut = self.start + dir * s;
        let rise_at = self.rise * s / run;
        let mut a = self.clone();
        a.end = cut;
        a.rise = rise_at;
        a.joint[1] = JoinKind::Free;
        let mut b = self.clone();
        b.id = new_id;
        b.start = cut;
        b.elevation_bottom += rise_at;
        b.rise = self.rise - rise_at;
        b.joint[0] = JoinKind::Free;
        Some((a, b))
    }

    /// The point where this member's line meets `other`'s, in plan; `None`
    /// for parallel members.
    pub fn crossing(&self, other: &FramingMember) -> Option<Point> {
        let (d1, d2) = (self.end - self.start, other.end - other.start);
        let den = d1.cross(d2);
        if den.abs() < 1e-9 {
            return None;
        }
        let t = (other.start - self.start).cross(d2) / den;
        Some(self.start + d1 * t)
    }

    /// Join and Lap Ends (manual p. 926): this member butts against `other`,
    /// which laps over its end. The end of this member nearest the crossing
    /// stops at the near face of `other`; `other` is extended, if need be, to
    /// cover the end. Returns false for parallel or non-linear members.
    pub fn join_lap(&mut self, other: &mut FramingMember) -> bool {
        if !self.is_linear() || !other.is_linear() {
            return false;
        }
        let Some(x) = self.crossing(other) else {
            return false;
        };
        let at_end = self.end.dist(x) < self.start.dist(x);
        let dir = (self.end - self.start).normalized();
        // Stop short of the other member's near face.
        let back = other.width / 2.0;
        let to = if at_end {
            x - dir * back
        } else {
            x + dir * back
        };
        if at_end {
            self.end = to;
            self.joint[1] = JoinKind::Butt;
        } else {
            self.start = to;
            self.joint[0] = JoinKind::Butt;
        }
        // The other member must reach across this one's width.
        let od = (other.end - other.start).normalized();
        let reach = self.width / 2.0;
        let (ps, pe) = ((x - other.start).dot(od), (x - other.end).dot(od));
        if ps < reach {
            other.start = other.start - od * (reach - ps);
            other.joint[0] = JoinKind::Lap;
        } else if -pe < reach {
            other.end = other.end + od * (reach + pe);
            other.joint[1] = JoinKind::Lap;
        } else if ps.abs() <= pe.abs() {
            other.joint[0] = JoinKind::Lap;
        } else {
            other.joint[1] = JoinKind::Lap;
        }
        true
    }

    /// Join and Mitre Ends: both members' nearest ends are brought to the
    /// crossing of their centre lines and marked mitred. Returns false for
    /// parallel or non-linear members.
    pub fn join_mitre(&mut self, other: &mut FramingMember) -> bool {
        if !self.is_linear() || !other.is_linear() {
            return false;
        }
        let Some(x) = self.crossing(other) else {
            return false;
        };
        for m in [&mut *self, &mut *other] {
            if m.end.dist(x) < m.start.dist(x) {
                m.end = x;
                m.joint[1] = JoinKind::Mitre;
            } else {
                m.start = x;
                m.joint[0] = JoinKind::Mitre;
            }
        }
        true
    }

    /// Horizontal length from `start` to `end`.
    pub fn plan_length(&self) -> f64 {
        self.start.dist(self.end)
    }

    /// Cut length: the post height, or the sloped length of a linear member.
    pub fn length(&self) -> f64 {
        if self.kind.is_vertical() {
            self.height
        } else {
            self.plan_length().hypot(self.rise)
        }
    }

    /// The footing under a post with footing, centred on the post and topped
    /// by the post's bottom.
    pub fn footing(&self) -> Option<Footing> {
        (self.kind == MemberKind::PostWithFooting).then_some(Footing {
            center: self.start,
            size: self.footing_spec.size,
            thickness: self.footing_spec.thickness,
            elevation: self.elevation_bottom,
        })
    }

    /// Oriented boxes for the 3D view: one for lumber members, the footing
    /// as well for a post with footing, one per chord and web for a truss with
    /// geometry, none for layout lines.
    pub fn to_boxes(&self) -> Vec<OrientedBox> {
        if !self.kind.is_physical() {
            return Vec::new();
        }
        if self.kind.is_vertical() {
            let r = self.rotation.to_radians();
            let ay = [r.cos(), 0.0, -r.sin()];
            let post = OrientedBox::from_axes(
                [
                    self.start.x,
                    self.elevation_bottom + self.height / 2.0,
                    -self.start.y,
                ],
                [self.height, self.depth, self.width],
                [UP, ay, cross(UP, ay)],
            );
            let mut out = vec![post];
            out.extend(self.footing().map(|f| f.to_box()));
            return out;
        }
        let run = self.plan_length();
        if run < EPS && self.rise.abs() < EPS {
            return Vec::new();
        }
        if self.kind.is_truss() {
            if let Some(spec) = &self.truss {
                let mut s = spec.clone();
                s.span = run;
                let truss = Truss::generate(&s);
                if !truss.members.is_empty() {
                    return truss.to_boxes(
                        self.start,
                        self.end - self.start,
                        self.elevation_bottom,
                    );
                }
            }
        }
        let len3 = run.hypot(self.rise);
        let (h, c, s) = if run < EPS {
            ([1.0, 0.0, 0.0], 0.0, self.rise.signum())
        } else {
            let d = (self.end - self.start).normalized();
            ([d.x, 0.0, -d.y], run / len3, self.rise / len3)
        };
        let ax = [h[0] * c, s, h[2] * c];
        let ay0 = [-h[0] * s, c, -h[2] * s];
        let az0 = cross(ax, ay0);
        let (sr, cr) = self.rotation.to_radians().sin_cos();
        let ay = add(scale(ay0, cr), scale(az0, sr));
        let az = add(scale(ay0, -sr), scale(az0, cr));
        let half_up = (self.depth * cr.abs() + self.width * sr.abs()) / 2.0;
        let start_c: Vec3 = [self.start.x, self.elevation_bottom + half_up, -self.start.y];
        vec![OrientedBox::from_axes(
            add(start_c, scale(ax, len3 / 2.0)),
            [len3, self.depth, self.width],
            [ax, ay, az],
        )]
    }

    /// Individual cut pieces for takeoff (a truss yields each chord and web,
    /// times its plies).
    pub(crate) fn pieces(&self) -> Vec<Piece> {
        if !self.kind.is_physical() {
            return Vec::new();
        }
        let material = self.material;
        if self.kind.is_truss() {
            if let Some(spec) = &self.truss {
                let mut s = spec.clone();
                s.span = self.plan_length();
                let truss = Truss::generate(&s);
                let plies = self.plies.max(1);
                let mut out = Vec::new();
                for m in &truss.members {
                    let size = LumberSize::dim(2, m.lumber.nominal_depth());
                    for _ in 0..plies {
                        out.push(Piece {
                            size: size.name(),
                            material: FramingMaterial::Lumber,
                            kind: m.role.name(),
                            length: m.length(),
                            board_feet: size.board_area() * m.length() / 144.0,
                        });
                    }
                }
                return out;
            }
        }
        let n = if self.kind.is_truss() {
            1
        } else {
            self.plies.max(1)
        };
        let len = self.length();
        (0..n)
            .map(|_| Piece {
                size: self.lumber.name(),
                material,
                kind: self.kind.name(),
                length: len,
                board_feet: self.lumber.board_area() * len / 144.0,
            })
            .collect()
    }
}

/// A girder truss: `spec` built from `plies` boards side by side, bearing at
/// `start` and `end`.
pub fn girder_truss(
    id: Id,
    start: Point,
    end: Point,
    mut spec: TrussSpec,
    plies: u32,
) -> FramingMember {
    spec.plies = plies.max(1);
    spec.span = start.dist(end);
    let mut m = FramingMember::new(id, MemberKind::GirderTruss, start, end);
    m.plies = spec.plies;
    m.width = spec.thickness();
    m.truss = Some(spec);
    m
}

/// A post and its footing, centred on one another: the footing top meets the
/// post bottom at `elevation`.
pub fn post_with_footing(
    id: Id,
    center: Point,
    elevation: f64,
    height: f64,
    footing: FootingSpec,
) -> (FramingMember, Footing) {
    let mut post = FramingMember::new(id, MemberKind::PostWithFooting, center, center);
    post.elevation_bottom = elevation;
    post.height = height;
    post.footing_spec = footing;
    let f = post.footing().expect("a post with footing has a footing");
    (post, f)
}

/// One cut piece in a takeoff.
pub(crate) struct Piece {
    pub size: String,
    pub material: FramingMaterial,
    pub kind: &'static str,
    pub length: f64,
    pub board_feet: f64,
}

/// `(thickness, depth)` sort key from a size name such as `"2x10"`; engineered
/// sizes sort after dimensional lumber.
fn size_key(name: &str) -> (u32, u32, String) {
    let mut it = name.split('x');
    match (
        it.next().and_then(|a| a.parse::<u32>().ok()),
        it.next().and_then(|b| b.parse::<u32>().ok()),
    ) {
        (Some(t), Some(d)) => (t, d, String::new()),
        _ => (u32::MAX, 0, name.to_string()),
    }
}

fn merge_linear(into: &mut Vec<(String, f64)>, name: &str, feet: f64) {
    match into.iter_mut().find(|(n, _)| n == name) {
        Some((_, f)) => *f += feet,
        None => into.push((name.to_string(), feet)),
    }
}

fn merge_lines(into: &mut Vec<(String, u32)>, name: String, n: u32) {
    match into.iter_mut().find(|(l, _)| *l == name) {
        Some((_, c)) => *c += n,
        None => into.push((name, n)),
    }
}

/// Takeoff of manual members: counts per cut, board feet and linear feet per
/// size. A truss also adds a count line such as `"Fink truss 288\" span"`.
pub fn manual_takeoff(members: &[FramingMember]) -> Takeoff {
    combined_takeoff(&[], members)
}

/// Takeoff of automatic members plus manual members, merged.
pub fn combined_takeoff(auto: &[Member], manual: &[FramingMember]) -> Takeoff {
    let mut t = crate::takeoff::takeoff(auto);
    for m in manual {
        if let (true, Some(spec)) = (m.kind.is_truss(), &m.truss) {
            let plies = if m.plies > 1 {
                format!(" x{} ply", m.plies)
            } else {
                String::new()
            };
            merge_lines(
                &mut t.lines,
                format!(
                    "{} truss {}\" span{plies}",
                    spec.kind.name(),
                    format_inches(m.plan_length())
                ),
                1,
            );
        }
        for p in m.pieces() {
            merge_lines(
                &mut t.lines,
                format!("{} x {}\" {}", p.size, format_inches(p.length), p.kind),
                1,
            );
            t.board_feet += p.board_feet;
            merge_linear(&mut t.linear_feet_by_size, &p.size, p.length / 12.0);
            crate::takeoff::add_cut(&mut t.cuts, p.kind, &p.size, p.length, 1);
        }
    }
    t.linear_feet_by_size
        .sort_by_key(|(name, _)| size_key(name));
    crate::takeoff::sort_cuts(&mut t.cuts);
    t
}

/// One line of the material list: a size and cut length with its count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialRow {
    pub size: String,
    pub material: FramingMaterial,
    pub length_in: f64,
    pub qty: u32,
    pub board_feet: f64,
}

/// Piece counts by lumber size and length.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MaterialList {
    pub rows: Vec<MaterialRow>,
}

fn feet_inches(inches: f64) -> String {
    let sixteenths = (inches * 16.0).round() as i64;
    let (ft, rest) = (sixteenths.div_euclid(192), sixteenths.rem_euclid(192));
    format!("{ft}'-{}\"", format_inches(rest as f64 / 16.0))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

impl MaterialList {
    /// Group manual members (with automatic ones, if given) by size, material
    /// and cut length to 1/16".
    pub fn from_members(auto: &[Member], manual: &[FramingMember]) -> Self {
        let mut rows: Vec<MaterialRow> = Vec::new();
        let mut add_piece = |size: String, material: FramingMaterial, length: f64, bf: f64| {
            let key = (length * 16.0).round();
            match rows.iter_mut().find(|r| {
                r.size == size && r.material == material && (r.length_in * 16.0).round() == key
            }) {
                Some(r) => {
                    r.qty += 1;
                    r.board_feet += bf;
                }
                None => rows.push(MaterialRow {
                    size,
                    material,
                    length_in: length,
                    qty: 1,
                    board_feet: bf,
                }),
            }
        };
        for m in auto {
            let area = f64::from(m.lumber.nominal_thickness() * m.lumber.nominal_depth());
            add_piece(
                m.lumber.nominal_name(),
                FramingMaterial::Lumber,
                m.length,
                area * m.length / 144.0,
            );
        }
        for m in manual {
            for p in m.pieces() {
                add_piece(p.size, p.material, p.length, p.board_feet);
            }
        }
        rows.sort_by(|a, b| {
            size_key(&a.size)
                .cmp(&size_key(&b.size))
                .then(a.length_in.total_cmp(&b.length_in))
        });
        Self { rows }
    }

    pub fn total_pieces(&self) -> u32 {
        self.rows.iter().map(|r| r.qty).sum()
    }

    pub fn total_board_feet(&self) -> f64 {
        self.rows.iter().map(|r| r.board_feet).sum()
    }

    /// `(size, pieces, board feet)` per lumber size, in size order.
    pub fn by_size(&self) -> Vec<(String, u32, f64)> {
        let mut out: Vec<(String, u32, f64)> = Vec::new();
        for r in &self.rows {
            match out.iter_mut().find(|(s, _, _)| *s == r.size) {
                Some((_, q, bf)) => {
                    *q += r.qty;
                    *bf += r.board_feet;
                }
                None => out.push((r.size.clone(), r.qty, r.board_feet)),
            }
        }
        out
    }

    /// CSV with a header and a final `Total` row.
    pub fn to_csv(&self) -> String {
        let mut s = String::from("Size,Material,Length,Length (in),Qty,Board Feet\n");
        for r in &self.rows {
            s.push_str(&format!(
                "{},{},{},{:.2},{},{:.2}\n",
                csv_field(&r.size),
                r.material.name(),
                csv_field(&feet_inches(r.length_in)),
                r.length_in,
                r.qty,
                r.board_feet
            ));
        }
        s.push_str(&format!(
            "Total,,,,{},{:.2}\n",
            self.total_pieces(),
            self.total_board_feet()
        ));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joist(id: Id, len: f64) -> FramingMember {
        FramingMember::new(
            id,
            MemberKind::Joist,
            Point::new(0.0, f64::from(id as u32) * 16.0),
            Point::new(len, f64::from(id as u32) * 16.0),
        )
    }

    #[test]
    fn chief_defaults_per_kind() {
        let p = Point::ZERO;
        let q = Point::new(100.0, 0.0);
        let j = FramingMember::new(1, MemberKind::Joist, p, q);
        assert_eq!(j.lumber, LumberSize::TWO_BY_TEN);
        assert!((j.depth - 9.25).abs() < 1e-9 && (j.width - 1.5).abs() < 1e-9);
        assert_eq!(MemberKind::Joist.default_spacing(), 16.0);
        let r = FramingMember::new(2, MemberKind::Rafter, p, q);
        assert_eq!(r.lumber.name(), "2x8");
        assert_eq!(MemberKind::Rafter.default_spacing(), 24.0);
        let post = FramingMember::new(3, MemberKind::Post, p, p);
        assert_eq!(post.lumber.name(), "4x4");
        assert!((post.width - 3.5).abs() < 1e-9);
        assert_eq!(post.height, 96.0);
        let beam = FramingMember::new(4, MemberKind::FloorCeilingBeam, p, q);
        assert_eq!(beam.lumber.name(), "4x10");
        let girder = FramingMember::new(5, MemberKind::GirderTruss, p, q);
        assert_eq!(girder.plies, 3);
        assert!((girder.width - 4.5).abs() < 1e-9);
        assert_eq!(j.layer_name, "Framing");
    }

    #[test]
    fn lumber_sizes_name_and_dress() {
        assert_eq!(LumberSize::dim(2, 12).depth(), 11.25);
        assert_eq!(LumberSize::SIX_BY_SIX.width(), 5.5);
        let g = LumberSize::Glulam {
            width: 5.125,
            depth: 12.0,
        };
        assert_eq!(g.name(), "GLB 5 1/8x12");
        assert_eq!(g.material(), FramingMaterial::Glulam);
        let l = LumberSize::Lvl {
            width: 1.75,
            depth: 11.875,
        };
        assert_eq!(l.name(), "LVL 1 3/4x11 7/8");
        assert!((l.board_area() - 1.75 * 11.875).abs() < 1e-9);
    }

    #[test]
    fn post_with_footing_is_centered_under_the_post() {
        let (post, f) = post_with_footing(
            9,
            Point::new(60.0, 40.0),
            -12.0,
            100.0,
            FootingSpec {
                size: 30.0,
                thickness: 10.0,
            },
        );
        assert_eq!(f.center, post.start);
        assert!((f.elevation - post.elevation_bottom).abs() < 1e-9);
        assert!((f.bottom_elevation() - -22.0).abs() < 1e-9);
        assert!((f.volume_cubic_yards() - 9000.0 / 46_656.0).abs() < 1e-12);
        let boxes = post.to_boxes();
        assert_eq!(boxes.len(), 2);
        let (pb, fb) = (boxes[0], boxes[1]);
        assert!((pb.center[0] - fb.center[0]).abs() < 1e-9);
        assert!((pb.center[2] - fb.center[2]).abs() < 1e-9);
        // Footing sits directly below the post.
        assert!((fb.center[1] + fb.size[0] / 2.0 - (pb.center[1] - pb.size[0] / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn joist_box_matches_the_member_geometry() {
        let m = joist(1, 144.0).at_elevation(90.0);
        let b = m.to_boxes();
        assert_eq!(b.len(), 1);
        let b = b[0];
        assert!((b.size[0] - 144.0).abs() < 1e-9);
        assert!((b.center[0] - 72.0).abs() < 1e-9);
        assert!((b.center[1] - (90.0 + 9.25 / 2.0)).abs() < 1e-9);
        assert!((b.center[2] + 16.0).abs() < 1e-9);
        assert!((b.rotation[0]).abs() < 1e-9 && b.rotation[1].abs() < 1e-9);
        assert!((b.axes[2][2].abs() - 1.0).abs() < 1e-9);
        assert_eq!(b.mesh(Some(1)).triangle_count(), 12);
    }

    #[test]
    fn sloped_rafter_box_has_pitch_and_flat_roll_swaps_the_section() {
        let mut r = FramingMember::new(
            1,
            MemberKind::Rafter,
            Point::new(0.0, 0.0),
            Point::new(144.0, 0.0),
        );
        r.rise = 72.0;
        let b = r.to_boxes()[0];
        assert!((b.size[0] - 144.0f64.hypot(72.0)).abs() < 1e-9);
        assert!((b.rotation[1] - 0.5f64.atan()).abs() < 1e-9);
        assert!((b.axes[0][1] - 72.0 / 144.0f64.hypot(72.0)).abs() < 1e-9);
        let mut flat = joist(2, 100.0);
        flat.rotation = 90.0;
        let fb = flat.to_boxes()[0];
        assert!((fb.axes[1][1]).abs() < 1e-9, "depth axis lies flat");
        assert!((fb.rotation[2].abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        for k in 0..3 {
            let a = fb.axes[k];
            assert!((a[0] * a[0] + a[1] * a[1] + a[2] * a[2] - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn post_box_is_vertical_and_rotates_in_plan() {
        let mut p = FramingMember::new(
            1,
            MemberKind::Post,
            Point::new(10.0, 20.0),
            Point::new(10.0, 20.0),
        );
        p.rotation = 90.0;
        let b = p.to_boxes()[0];
        assert_eq!(b.axes[0], [0.0, 1.0, 0.0]);
        assert!((b.center[1] - 48.0).abs() < 1e-9);
        assert!((b.rotation[1] - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!((b.axes[1][2] + 1.0).abs() < 1e-9);
    }

    #[test]
    fn girder_truss_has_plies_and_ply_width() {
        let spec = TrussSpec::new(TrussType::Howe, 0.0, 8.0);
        let g = girder_truss(1, Point::ZERO, Point::new(240.0, 0.0), spec, 2);
        assert_eq!((g.plies, g.kind), (2, MemberKind::GirderTruss));
        assert!((g.width - 3.0).abs() < 1e-9);
        let boxes = g.to_boxes();
        assert_eq!(boxes.len(), 11);
        assert!(boxes.iter().all(|b| (b.size[2] - 3.0).abs() < 1e-9));
    }

    #[test]
    fn layout_lines_have_no_boxes_or_takeoff() {
        let b = FramingMember::new(
            1,
            MemberKind::BearingLine,
            Point::ZERO,
            Point::new(100.0, 0.0),
        );
        assert!(b.to_boxes().is_empty());
        assert!(manual_takeoff(&[b]).lines.is_empty());
    }

    #[test]
    fn ten_2x10x12_is_two_hundred_board_feet() {
        let ms: Vec<FramingMember> = (0..10).map(|i| joist(i, 144.0)).collect();
        let t = manual_takeoff(&ms);
        assert!((t.board_feet - 200.0).abs() < 1e-9);
        assert_eq!(t.lines, [("2x10 x 144\" joist".to_string(), 10)]);
        assert_eq!(t.linear_feet_by_size, [("2x10".to_string(), 120.0)]);
        let list = MaterialList::from_members(&[], &ms);
        assert_eq!(list.rows.len(), 1);
        assert_eq!(list.total_pieces(), 10);
        assert!((list.total_board_feet() - 200.0).abs() < 1e-9);
        assert_eq!(list.by_size(), [("2x10".to_string(), 10, 200.0)]);
        let csv = list.to_csv();
        assert!(csv.starts_with("Size,Material,Length,Length (in),Qty,Board Feet\n"));
        assert!(
            csv.contains("2x10,Lumber,\"12'-0\"\"\",144.00,10,200.00\n"),
            "{csv}"
        );
        assert!(csv.ends_with("Total,,,,10,200.00\n"));
    }

    #[test]
    fn engineered_beams_use_actual_area_and_plies_multiply() {
        let mut beam =
            FramingMember::new(1, MemberKind::RoofBeam, Point::ZERO, Point::new(240.0, 0.0))
                .with_lumber(LumberSize::Lvl {
                    width: 1.75,
                    depth: 11.875,
                });
        assert_eq!(beam.material, FramingMaterial::Lvl);
        let t = manual_takeoff(std::slice::from_ref(&beam));
        assert!((t.board_feet - 1.75 * 11.875 * 240.0 / 144.0).abs() < 1e-9);
        beam.plies = 2;
        let t2 = manual_takeoff(&[beam]);
        assert!((t2.board_feet - 2.0 * t.board_feet).abs() < 1e-9);
    }

    #[test]
    fn truss_members_are_taken_off_per_chord_and_web_and_merge_with_auto() {
        let t = FramingMember::new(
            1,
            MemberKind::GirderTruss,
            Point::ZERO,
            Point::new(288.0, 0.0),
        );
        let to = manual_takeoff(std::slice::from_ref(&t));
        assert!(to
            .lines
            .iter()
            .any(|(l, n)| l == "Fink truss 288\" span x3 ply" && *n == 1));
        let truss = Truss::generate(
            &t.truss
                .clone()
                .map(|mut s| {
                    s.span = 288.0;
                    s
                })
                .unwrap(),
        );
        let inches: f64 = truss.members.iter().map(|m| m.length()).sum::<f64>() * 3.0;
        assert!((to.board_feet - 8.0 * inches / 144.0).abs() < 1e-6);
        // Merged with automatic framing.
        let tf = Transform3 {
            origin: [0.0; 3],
            axis_x: [1.0, 0.0, 0.0],
            axis_y: [0.0, 1.0, 0.0],
        };
        let auto = Member::new(AutoKind::Stud, Lumber::two_by(3.5), 96.0, tf, None);
        let both = combined_takeoff(&[auto], &[t]);
        assert!((both.board_feet - (to.board_feet + 16.0 / 3.0)).abs() < 1e-6);
        assert_eq!(both.linear_feet_by_size.len(), 1);
    }

    // ----- Round 16: break, joins -----

    #[test]
    fn add_break_cuts_a_member_in_two_and_shares_the_rise() {
        let mut m = joist(1, 120.0);
        m.rise = 12.0;
        m.elevation_bottom = 100.0;
        let (a, b) = m.break_at(Point::new(30.0, 5.0), 9).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(b.id, 9);
        assert!((a.plan_length() - 30.0).abs() < 1e-9);
        assert!((b.plan_length() - 90.0).abs() < 1e-9);
        assert!((a.end.x - b.start.x).abs() < 1e-9 && (a.end.y - 16.0).abs() < 1e-9);
        assert!((a.rise - 3.0).abs() < 1e-9 && (b.rise - 9.0).abs() < 1e-9);
        assert!((b.elevation_bottom - 103.0).abs() < 1e-9);
        // Too near an end, or not a linear member.
        assert!(m.break_at(Point::new(0.5, 0.0), 9).is_none());
        let post = FramingMember::new(2, MemberKind::Post, Point::ZERO, Point::ZERO);
        assert!(post.break_at(Point::ZERO, 9).is_none());
    }

    #[test]
    fn join_and_lap_butts_the_first_member_against_the_second() {
        let mut a = joist(1, 100.0);
        // The second member crosses the first's end at x = 98 and is long enough.
        let mut b = FramingMember::new(2, MemberKind::Joist, Point::new(98.0, -50.0), Point::new(98.0, 50.0));
        assert!(a.join_lap(&mut b));
        // `a` stops at b's near face (half its width short of the crossing).
        assert!((a.end.x - (98.0 - b.width / 2.0)).abs() < 1e-9, "{}", a.end.x);
        assert_eq!(a.joint[1], JoinKind::Butt);
        // `b` laps over the end of `a` and keeps its length: it already covers it.
        assert!(b.joint.contains(&JoinKind::Lap));
        assert!((b.plan_length() - 100.0).abs() < 1e-9);
        // Parallel members cannot be joined.
        let mut c = joist(3, 50.0);
        let mut d = joist(4, 50.0);
        d.start.y = 10.0;
        d.end.y = 10.0;
        assert!(!c.join_lap(&mut d));
        assert!(!c.join_mitre(&mut d));
    }

    #[test]
    fn join_and_mitre_brings_both_ends_to_the_crossing() {
        let mut a = FramingMember::new(1, MemberKind::GeneralFraming, Point::new(0.0, 0.0), Point::new(90.0, 0.0));
        let mut b = FramingMember::new(2, MemberKind::GeneralFraming, Point::new(100.0, 10.0), Point::new(100.0, 90.0));
        assert!(a.join_mitre(&mut b));
        assert!((a.end.x - 100.0).abs() < 1e-9 && a.end.y.abs() < 1e-9);
        assert!((b.start.y).abs() < 1e-9 && (b.start.x - 100.0).abs() < 1e-9);
        assert_eq!(a.joint[1], JoinKind::Mitre);
        assert_eq!(b.joint[0], JoinKind::Mitre);
    }

    #[test]
    fn old_members_read_with_the_new_options_off() {
        let m = joist(1, 100.0);
        let mut v = serde_json::to_value(&m).unwrap();
        let o = v.as_object_mut().unwrap();
        for k in [
            "bearing_beam",
            "flat",
            "show_cross",
            "treated",
            "show_ply_lines",
            "end_profile",
            "custom_label",
            "fill",
            "detail_fill",
            "joint",
        ] {
            o.remove(k);
        }
        let back: FramingMember = serde_json::from_value(v).unwrap();
        assert_eq!(back, m);
        assert!(back.show_cross && !back.bearing_beam && back.flat == FlatTo::None);
        assert_eq!(FlatTo::ALL.len(), 3);
        assert_eq!(EndShape::ALL.len(), 4);
    }
}
