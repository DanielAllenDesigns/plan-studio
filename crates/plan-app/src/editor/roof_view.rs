//! Roof planes: storage, automatic building, editing math, plan display and
//! the 3D meshes (`docs/parity/roofs.md`).
//!
//! # Storage
//!
//! A floor's roof lives in `Floor.roofs`, saved, loaded and undone with the
//! plan: one JSON object per entry, tagged by `"kind"`: `"plane"` for a
//! [`RoofPlaneRecord`] (its `id` is the plane id) and `"settings"` for the
//! [`RoofSettings`] of the last Build Roof. Two more kinds share the slot:
//! `"ceiling"` ([`CeilingRecord`], a vaulted ceiling plane on layer
//! "Ceiling Planes") and `"dormer"` ([`DormerRecord`]: the main plane, the
//! `plan_roof::DormerSpec`; its planes, walls and the hole in the main roof
//! are regenerated from those, so a dormer follows its plane). Planes are
//! picked and drawn straight from the records; there are no outline
//! polylines in `Floor.cad`.
//!
//! Holes and skylights are [`HoleRecord`]s on the plane that carries them.
//! `RoofSettings::edge_specs` keeps the per-edge pitch / overhang / gable
//! overrides the Roof Plane Specification sets for Build Roof.
//!
//! Older files kept the roof as hidden CAD text records (`RFP1:` / `RFS1:` on
//! the layer `"Roof Planes, Data"`, plus an outline polyline per plane);
//! [`migrate_legacy`] converts them once when a project is loaded.
//!
//! Coordinates in a record follow `plan-roof`: `[x, elevation, -plan_y]`.

use super::selection::ObjectRef;
use super::{Camera, EditAction, EditActionKind, EditorContext};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::defaults::{RoofDetailDefaults, RoofWallKind};
use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_centroid, Point};
use plan_core::{
    detect_rooms, Floor, Id, Layer, LineStyle, PlanDefaults, Project, Room, Wall, WallKind,
};
use plan_roof::{
    apply_gable_line, auto_dormer, build_roof_at_plate_with_faces, build_roof_with_faces,
    ceiling_planes_for_vaulted_room, flat_roof_plane_with_overhang, footprint_from_walls,
    join_planes, plate_baseline, roof_plane_with_holes, roof_return_at, CeilingPlane, Dormer,
    DormerSpec, EdgeRoofSpec, ReturnKind, ReturnSpec, Roof, RoofHole, RoofPlane, SkylightSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod plane_extras;
pub use plane_extras::{
    align_plane, attic_wall_above, build_when_room_closes, move_coplanar, move_display, place_intersection_point, reference_direction,
    run_plane_command, snap_baseline, snap_edge_to_walls, FillKind, PlaneFill, PlaneStyle, SlopeArrow, LOCKS,
    PLANE_COMMANDS,
};

pub const LAYER_PLANES: &str = "Roof Planes";
/// Layer of the vaulted ceiling planes.
pub const LAYER_CEILING: &str = "Ceiling Planes";
/// Thickness of a ceiling plane (structure above the finished surface), inches.
pub const CEILING_THICKNESS: f64 = 9.0;
/// Length of the roof returns Build Roof makes for walls with Auto Roof
/// Return, inches (the Roof Return tool uses the same).
pub const AUTO_RETURN_LENGTH: f64 = 24.0;
/// How far an Extend Slope Downward edge continues below its eave, inches
/// (vertical drop). Chief reaches down to the wall below; plan-roof needs a
/// fixed drop because the walls below are not known to Build Roof.
pub const EXTEND_SLOPE_DROP: f64 = 24.0;
/// Overhang a new dormer starts with, inches.
pub const DORMER_OVERHANG: f64 = 12.0;
/// Thickness of the dormer walls in the 3D meshes, inches.
pub const DORMER_WALL_THICKNESS: f64 = 4.5;
/// Hidden layer of the legacy CAD-record storage (older files only).
pub const LAYER_DATA: &str = "Roof Planes, Data";
const PLANE_TAG: &str = "RFP1:";
const SETTINGS_TAG: &str = "RFS1:";
/// Footprint / wall matching tolerance, inches.
const TOL: f64 = 0.5;
/// Default skylight size (RF-43), inches: width, length along the slope.
pub const SKYLIGHT_SIZE: (f64, f64) = (24.0, 48.0);
/// A hole must have at least this much plan clearance from its plane's edge.
const HOLE_MARGIN: f64 = 0.5;
/// Thickness of the plane slab in the 3D meshes, inches.
pub const SLAB_THICKNESS: f64 = 1.0;

/// Roofing choices of the Materials pages.
pub const ROOF_MATERIALS: [&str; 5] = [
    "Asphalt Shingles",
    "Concrete Tile",
    "Standing Seam Metal",
    "Wood Shakes",
    "Slate",
];

// ===================================================================
// Records
// ===================================================================

macro_rules! field {
    ($v:expr, $k:literal, $t:ty) => {
        $v.get($k)
            .and_then(|x| serde_json::from_value::<$t>(x.clone()).ok())
    };
}

/// A hole through a roof plane: a plain opening (RF-42) or a skylight on a
/// curb (RF-43).
#[derive(Clone, Debug, PartialEq)]
pub struct HoleRecord {
    /// Plan outline, inches.
    pub outline: Vec<Point>,
    /// Skylight construction; `None` is a plain hole.
    pub skylight: Option<SkylightSpec>,
}

impl HoleRecord {
    pub fn hole(outline: Vec<Point>) -> Self {
        Self {
            outline,
            skylight: None,
        }
    }

    pub fn skylight(outline: Vec<Point>) -> Self {
        Self {
            outline,
            skylight: Some(SkylightSpec::default()),
        }
    }

    pub fn is_skylight(&self) -> bool {
        self.skylight.is_some()
    }

    /// Plan extents `(width, height)` of the outline's bounding box.
    pub fn size(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for p in &self.outline {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        if self.outline.is_empty() {
            (0.0, 0.0)
        } else {
            (hi.x - lo.x, hi.y - lo.y)
        }
    }

    pub fn translate(&mut self, d: Point) {
        for p in &mut self.outline {
            *p = p.add(d);
        }
    }

    /// Turns the outline by `angle` radians about `pivot`.
    pub fn rotate(&mut self, angle: f64, pivot: Point) {
        for p in &mut self.outline {
            *p = rotate_about(*p, angle, pivot);
        }
    }

    /// The `plan-roof` hole.
    pub fn to_roof_hole(&self) -> RoofHole {
        match self.skylight {
            Some(spec) => RoofHole::skylight(self.outline.clone(), spec),
            None => RoofHole::hole(self.outline.clone()),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "outline": self.outline, "skylight": self.skylight })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let outline = field!(v, "outline", Vec<Point>)?;
        (outline.len() >= 3).then(|| Self {
            outline,
            skylight: field!(v, "skylight", SkylightSpec),
        })
    }
}

/// `p` turned by `angle` radians (counter-clockwise) about `pivot`.
pub fn rotate_about(p: Point, angle: f64, pivot: Point) -> Point {
    let (s, c) = angle.sin_cos();
    let d = p.sub(pivot);
    pivot.add(Point::new(d.x * c - d.y * s, d.x * s + d.y * c))
}

/// How far up the slope from the centre the pitch arrow's handle sits, inches.
pub const PITCH_ARROW_REACH: f64 = 24.0;
/// How far below the eave the rotate handle sits, inches.
pub const ROTATE_REACH: f64 = 30.0;
/// Rise in 12 gained per inch the pitch arrow is dragged up the slope.
pub const PITCH_PER_INCH: f64 = 0.25;
/// Edges shorter than this get no edge handle, inches.
const MIN_EDGE_HANDLE: f64 = 12.0;

/// The edit handles of a selected roof plane (RF-38).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaneHandle {
    /// A corner: drags it in plan, the plane keeps its pitch.
    Vertex(usize),
    /// The middle of edge `i` (vertex `i` to `i + 1`): drags the whole edge
    /// square to itself.
    Edge(usize),
    /// The pitch arrow: dragging it up the slope steepens the plane.
    Pitch,
    /// The knob below the eave: turns the plane about its centre.
    Rotate,
}

impl RoofPlaneRecord {
    /// The handles of the plane with their plan positions: corners, the
    /// middle of every edge long enough, the pitch arrow up the slope from the
    /// centre and the rotate knob below the eave.
    pub fn handles(&self) -> Vec<(PlaneHandle, Point)> {
        let poly = self.plan_polygon();
        let n = poly.len();
        let mut out: Vec<(PlaneHandle, Point)> = poly
            .iter()
            .enumerate()
            .map(|(i, &q)| (PlaneHandle::Vertex(i), q))
            .collect();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if a.dist(b) >= MIN_EDGE_HANDLE {
                out.push((PlaneHandle::Edge(i), Point::lerp(a, b, 0.5)));
            }
        }
        let c = self.centroid();
        out.push((
            PlaneHandle::Pitch,
            c.add(self.up_slope().scale(PITCH_ARROW_REACH)),
        ));
        let (a, b) = self.baseline;
        out.push((
            PlaneHandle::Rotate,
            Point::lerp(a, b, 0.5).sub(self.up_slope().scale(ROTATE_REACH)),
        ));
        out
    }

    /// The handle at `p` within `tol`, corners first.
    pub fn handle_at(&self, p: Point, tol: f64) -> Option<(PlaneHandle, Point)> {
        self.handles()
            .into_iter()
            .filter(|(_, q)| q.dist(p) <= tol)
            .min_by(|x, y| {
                let rank = |h: PlaneHandle| match h {
                    PlaneHandle::Vertex(_) => 0,
                    PlaneHandle::Pitch | PlaneHandle::Rotate => 1,
                    PlaneHandle::Edge(_) => 2,
                };
                (rank(x.0), x.1.dist(p))
                    .partial_cmp(&(rank(y.0), y.1.dist(p)))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Steepens the plane by `along` inches of drag up the slope (negative
    /// flattens it): [`PITCH_PER_INCH`] in 12 per inch, in quarters, between
    /// 1/4:12 and 24:12. The plane becomes manual (RF-37).
    pub fn drag_pitch(&mut self, along: f64) {
        let target = (self.pitch + along * PITCH_PER_INCH).clamp(0.25, 24.0);
        self.set_pitch((target * 4.0).round() / 4.0);
        self.auto = false;
    }

    /// Turns the plane (outline, baseline, holes, source edge) by `angle`
    /// radians about `pivot`; the heights follow the turned baseline.
    pub fn rotate(&mut self, angle: f64, pivot: Point) {
        for v in &mut self.polygon3d {
            let q = rotate_about(Point::new(v[0], -v[2]), angle, pivot);
            v[0] = q.x;
            v[2] = -q.y;
        }
        self.baseline = (
            rotate_about(self.baseline.0, angle, pivot),
            rotate_about(self.baseline.1, angle, pivot),
        );
        for h in &mut self.holes {
            h.rotate(angle, pivot);
        }
        if let Some((a, b)) = &mut self.source {
            *a = rotate_about(*a, angle, pivot);
            *b = rotate_about(*b, angle, pivot);
        }
        self.auto = false;
        self.resolve_heights();
    }

    /// Moves edge `i` (vertex `i` to `i + 1`) by `by` inches square to
    /// itself, positive toward the right of the edge's direction (away from
    /// a counter-clockwise plane's inside). The plane stays planar and its
    /// pitch is kept; it becomes manual. Moving the baseline edge moves the
    /// eave.
    pub fn move_edge(&mut self, i: usize, by: f64) {
        let n = self.polygon3d.len();
        if i >= n {
            return;
        }
        let poly = self.plan_polygon();
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let d = b.sub(a).normalized();
        let shift = Point::new(d.y, -d.x).scale(by);
        for k in [i, (i + 1) % n] {
            let q = poly[k].add(shift);
            self.polygon3d[k][0] = q.x;
            self.polygon3d[k][2] = -q.y;
            if k == 0 {
                self.baseline.0 = q;
            } else if k == 1 {
                self.baseline.1 = q;
            }
        }
        self.auto = false;
        self.resolve_heights();
    }
}

/// A handle drag on a plane, worked out from the plane as it was when the
/// handle was grabbed (RF-38).
#[derive(Clone, Debug, PartialEq)]
pub struct HandleDrag {
    /// The plane after the drag.
    pub record: RoofPlaneRecord,
    /// The undo label of the drag.
    pub label: &'static str,
    /// The readout shown while dragging.
    pub readout: String,
}

/// Drags `handle` of the plane `orig` from `start` to `to`. Corners follow
/// the pointer, the pitch arrow trades slope for distance up the slope, the
/// rotate knob turns the plane about its centre in whole degrees and an edge
/// handle moves the edge square to itself. `fmt` writes a length for the
/// readout.
pub fn drag_handle(
    orig: &RoofPlaneRecord,
    handle: PlaneHandle,
    start: Point,
    to: Point,
    fmt: &dyn Fn(f64) -> String,
) -> HandleDrag {
    let mut rec = orig.clone();
    let drag = to.sub(start);
    let (label, readout) = match handle {
        PlaneHandle::Vertex(i) => {
            rec.move_vertex(i, to);
            ("Reshape Roof Plane", String::new())
        }
        PlaneHandle::Pitch => {
            rec.drag_pitch(drag.dot(orig.up_slope()));
            ("Change Roof Pitch", format!("Pitch: {}", rec.pitch_label()))
        }
        PlaneHandle::Rotate => {
            let pivot = orig.centroid();
            let a0 = start.sub(pivot).angle();
            let a1 = to.sub(pivot).angle();
            let turn = (a1 - a0).to_degrees().round().to_radians();
            rec.rotate(turn, pivot);
            (
                "Rotate Roof Plane",
                format!("Rotation: {:.0} degrees", turn.to_degrees()),
            )
        }
        PlaneHandle::Edge(i) => {
            let poly = orig.plan_polygon();
            let n = poly.len();
            let d = poly[(i + 1) % n].sub(poly[i % n]).normalized();
            let by = drag.dot(Point::new(d.y, -d.x));
            rec.move_edge(i, by);
            ("Move Roof Edge", format!("Edge moved {}", fmt(by)))
        }
    };
    HandleDrag {
        record: rec,
        label,
        readout,
    }
}

/// Applies a handle drag to plane `id` of floor `fi` of `project`, working
/// from the plane as it is in `project` (the Select tool restores the
/// original project before every step, so this is the plane at the press).
/// Returns the drag, or `None` when the plane is gone.
pub fn apply_handle_drag(
    project: &mut Project,
    fi: usize,
    id: Id,
    handle: PlaneHandle,
    start: Point,
    to: Point,
) -> Option<HandleDrag> {
    let mut set = load(&project.floors[fi]);
    let orig = set.plane(id)?.clone();
    let drag = drag_handle(&orig, handle, start, to, &|v| format!("{v:.1}\""));
    *set.plane_mut(id)? = drag.record.clone();
    store(project, fi, &mut set);
    Some(drag)
}

/// Per-edge overrides of Build Roof (the Roof Plane Specification's edge
/// fields), feeding `plan_roof::EdgeRoofSpec`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeOverride {
    /// Rise per 12 instead of the wall's.
    pub pitch: Option<f64>,
    /// Overhang from the wall face instead of the wall's.
    pub overhang: Option<f64>,
    /// A gable end: no plane rises from the edge.
    pub gable: bool,
}

impl EdgeOverride {
    pub fn is_default(&self) -> bool {
        self.pitch.is_none() && self.overhang.is_none() && !self.gable
    }
}

/// An override and the footprint edge it applies to.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSpec {
    pub edge: (Point, Point),
    pub over: EdgeOverride,
}

impl EdgeSpec {
    fn to_json(&self) -> Value {
        json!({
            "a": self.edge.0,
            "b": self.edge.1,
            "pitch": self.over.pitch,
            "overhang": self.over.overhang,
            "gable": self.over.gable,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            edge: (field!(v, "a", Point)?, field!(v, "b", Point)?),
            over: EdgeOverride {
                pitch: field!(v, "pitch", f64),
                overhang: field!(v, "overhang", f64),
                gable: field!(v, "gable", bool).unwrap_or(false),
            },
        })
    }
}

/// How a roof plane is framed (Roof Plane Specification > Structure).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RoofFraming {
    #[default]
    Rafters,
    Trusses,
}

impl RoofFraming {
    pub const ALL: [RoofFraming; 2] = [RoofFraming::Rafters, RoofFraming::Trusses];

    pub fn label(self) -> &'static str {
        match self {
            RoofFraming::Rafters => "Rafters",
            RoofFraming::Trusses => "Trusses",
        }
    }
}

/// How the ceiling under a roof plane is framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CeilingFraming {
    /// Joists across the walls under the roof.
    #[default]
    CeilingJoists,
    /// The rafters are the ceiling: a vaulted (cathedral) ceiling.
    Vaulted,
    /// The bottom chords of the trusses.
    TrussBottomChords,
}

impl CeilingFraming {
    pub const ALL: [CeilingFraming; 3] = [
        CeilingFraming::CeilingJoists,
        CeilingFraming::Vaulted,
        CeilingFraming::TrussBottomChords,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CeilingFraming::CeilingJoists => "Ceiling Joists",
            CeilingFraming::Vaulted => "Vaulted (rafters show)",
            CeilingFraming::TrussBottomChords => "Truss Bottom Chords",
        }
    }
}

/// The structure of one roof plane, set by Structure > Define (RF-36): the
/// framing, member size and spacing, and the layers over it. A plane without
/// one follows Roof Defaults. The thickness and rafter size reach the 3D roof
/// through [`plan_3d::EaveOverrides`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofStructure {
    pub framing: RoofFraming,
    /// Rafter (or top chord) width and depth, inches.
    pub member_width: f64,
    pub member_depth: f64,
    /// On-center spacing, inches.
    pub spacing: f64,
    /// Sheathing and roofing over the framing, inches.
    pub sheathing: f64,
    pub roofing: f64,
    pub ceiling: CeilingFraming,
}

impl Default for RoofStructure {
    /// 2x6 rafters at 24" with 1/2" sheathing, like the Roof Defaults.
    fn default() -> Self {
        Self {
            framing: RoofFraming::Rafters,
            member_width: 1.5,
            member_depth: 5.5,
            spacing: 24.0,
            sheathing: 0.5,
            roofing: 0.0,
            ceiling: CeilingFraming::CeilingJoists,
        }
    }
}

impl RoofStructure {
    /// The structure Roof Defaults describe.
    pub fn from_detail(d: &RoofDetailDefaults) -> Self {
        let sheathing = 0.5_f64.min(d.thickness.max(0.0));
        Self {
            framing: RoofFraming::Rafters,
            member_width: d.rafter_width,
            member_depth: d.rafter_depth,
            spacing: d.rafter_spacing,
            sheathing,
            roofing: (d.thickness - d.rafter_depth - sheathing).max(0.0),
            ceiling: CeilingFraming::CeilingJoists,
        }
    }

    /// Thickness of the plane along its normal, inches.
    pub fn thickness(&self) -> f64 {
        self.member_depth + self.sheathing + self.roofing
    }

    /// Smallest sizes the Define dialog accepts.
    pub fn error(&self) -> Option<String> {
        if self.member_width <= 0.0 || self.member_depth <= 0.0 {
            Some("The framing members need a positive width and depth".into())
        } else if self.spacing < 1.0 {
            Some("Spacing must be at least 1 inch".into())
        } else if self.sheathing < 0.0 || self.roofing < 0.0 {
            Some("Layer thicknesses cannot be negative".into())
        } else {
            None
        }
    }
}

/// One roof plane (RF-36).
#[derive(Clone, Debug, PartialEq)]
pub struct RoofPlaneRecord {
    pub id: Id,
    /// `[x, elevation, -plan_y]` vertices; edge `0 -> 1` is the eave baseline.
    pub polygon3d: Vec<[f64; 3]>,
    /// Rise per 12 of run.
    pub pitch: f64,
    pub baseline: (Point, Point),
    /// Built by Build Roof and not edited by hand (RF-6, RF-37).
    pub auto: bool,
    /// Holes and skylights through the plane (RF-42, RF-43).
    pub holes: Vec<HoleRecord>,
    /// Horizontal overhang beyond the wall face, inches (read-only display).
    pub overhang: f64,
    pub label: String,
    pub material: String,
    pub layer: String,
    pub ridge_caps: bool,
    pub gutters: bool,
    /// This plane's own eave choices (cut, rafter tails, fascia, soffit,
    /// frieze, gutters); unset ones follow the roof's detail.
    pub eave: plan_3d::EaveOverrides,
    /// The footprint edge an automatic plane rises from (what Build Roof
    /// overrides are keyed by).
    pub source: Option<(Point, Point)>,
    /// The Build Roof override of [`source`](Self::source). Read from the
    /// roof settings on load; the dialog edits it and
    /// [`apply_plane_edit`] writes it back.
    pub edge: EdgeOverride,
    /// This plane's own structure (Structure > Define); `None` follows Roof
    /// Defaults.
    pub structure: Option<RoofStructure>,
    /// The layered Roof Surface, Roof Structure and Roof Ceiling Finish
    /// definitions of this plane (Structure panel, Round 16); `structure`
    /// holds the numbers the builders read from them.
    pub layers: Option<plan_core::assemblies::RoofLayers>,
    /// The per-edge record of a plane imported from a Chief plan (`role`,
    /// `joined`, `overhangs` per outline edge), kept as read so editing the
    /// plane does not lose it.
    pub chief_edges: Option<Value>,
    /// Curved Roof (RF-61): the plane's section across the slope is an arc;
    /// `None` is an ordinary flat plane.
    pub curved: Option<plan_roof::CurvedSpec>,
    /// Top of the top plate the baseline bears on, elevation (brief 18b);
    /// `None` for a hand-drawn plane, whose plate is then taken to sit with
    /// no birdsmouth.
    pub plate_top: Option<f64>,
    /// Width of that plate, the birdsmouth seat of the automatic cut.
    pub plate_width: f64,
    /// Plan Line Style, Fill Style and slope Arrow of the plane (RF-88,
    /// RF-89, RF-91).
    pub style: PlaneStyle,
    /// List the plane in the roof schedule (RF-101).
    pub in_schedule: bool,
    /// Use Special Snapping: edges snap to the outside of a parallel wall
    /// (RF-107, RF-119).
    pub special_snapping: bool,
}

/// Plate width a plane starts with, inches (a 2x4 plate).
pub const DEFAULT_PLATE_WIDTH: f64 = 3.5;

impl RoofPlaneRecord {
    pub fn new(id: Id, polygon3d: Vec<[f64; 3]>, pitch: f64, baseline: (Point, Point)) -> Self {
        Self {
            id,
            polygon3d,
            pitch,
            baseline,
            auto: false,
            holes: Vec::new(),
            overhang: 0.0,
            label: String::new(),
            material: ROOF_MATERIALS[0].to_string(),
            layer: LAYER_PLANES.to_string(),
            ridge_caps: false,
            gutters: false,
            eave: plan_3d::EaveOverrides::default(),
            source: None,
            edge: EdgeOverride::default(),
            structure: None,
            layers: None,
            chief_edges: None,
            curved: None,
            plate_top: None,
            plate_width: DEFAULT_PLATE_WIDTH,
            style: PlaneStyle::default(),
            in_schedule: true,
            special_snapping: true,
        }
    }

    /// Gives the plane layered Roof Surface, Roof Structure and Roof Ceiling
    /// Finish definitions (or, with `None`, hands it back to Roof Defaults).
    /// The numbers the 3D roof and the framing read follow the layers;
    /// `base` supplies what layers do not say (the ceiling framing, member
    /// sizes when the structure has no Framing layer).
    pub fn set_layers(
        &mut self,
        layers: Option<plan_core::assemblies::RoofLayers>,
        base: RoofStructure,
    ) {
        match &layers {
            Some(l) => {
                let n = l.numbers();
                let mut st = base;
                st.framing = if n.trusses {
                    RoofFraming::Trusses
                } else {
                    RoofFraming::Rafters
                };
                st.member_width = n.member_width.unwrap_or(base.member_width);
                st.member_depth = n.member_depth.unwrap_or(base.member_depth);
                st.spacing = n.spacing.unwrap_or(base.spacing);
                st.sheathing = n.sheathing;
                st.roofing = n.roofing;
                self.set_structure(Some(st));
            }
            None => self.set_structure(None),
        }
        self.layers = layers;
    }

    /// Gives the plane its own structure (or, with `None`, hands it back to
    /// Roof Defaults); the 3D thickness and rafter choices follow.
    pub fn set_structure(&mut self, structure: Option<RoofStructure>) {
        self.layers = None;
        self.structure = structure;
        match structure {
            Some(s) => {
                self.eave.thickness = Some(s.thickness());
                self.eave.rafter_spacing = Some(s.spacing);
                self.eave.rafter_width = Some(s.member_width);
                self.eave.rafter_depth = Some(s.member_depth);
            }
            None => {
                self.eave.thickness = None;
                self.eave.rafter_spacing = None;
                self.eave.rafter_width = None;
                self.eave.rafter_depth = None;
            }
        }
    }

    pub fn plan_polygon(&self) -> Vec<Point> {
        self.polygon3d
            .iter()
            .map(|p| Point::new(p[0], -p[2]))
            .collect()
    }

    /// Elevation of the eave (vertex 0), inches.
    pub fn baseline_height(&self) -> f64 {
        self.polygon3d.first().map_or(0.0, |p| p[1])
    }

    pub fn centroid(&self) -> Point {
        polygon_centroid(&self.plan_polygon())
    }

    pub fn contains(&self, p: Point) -> bool {
        point_in_polygon(p, &self.plan_polygon())
    }

    /// Unit plan direction in which the plane rises.
    pub fn up_slope(&self) -> Point {
        let (a, b) = self.baseline;
        let d = b.sub(a);
        if d.length() < 1e-9 {
            return Point::new(0.0, 1.0);
        }
        let n = d.normalized().perp();
        if self.centroid().sub(a).dot(n) < 0.0 {
            n.scale(-1.0)
        } else {
            n
        }
    }

    /// Sets every vertex height from the baseline and pitch, keeping the plane
    /// planar (RF-38: vertex drags keep the plane's pitch).
    pub fn resolve_heights(&mut self) {
        let base = self.baseline_height();
        let a = self.baseline.0;
        let up = self.up_slope();
        let k = self.pitch / 12.0;
        for v in &mut self.polygon3d {
            let p = Point::new(v[0], -v[2]);
            v[1] = base + p.sub(a).dot(up).max(0.0) * k;
        }
    }

    pub fn set_pitch(&mut self, pitch: f64) {
        self.pitch = pitch.max(0.01);
        self.resolve_heights();
    }

    pub fn set_baseline_height(&mut self, h: f64) {
        let d = h - self.baseline_height();
        for v in &mut self.polygon3d {
            v[1] += d;
        }
    }

    /// Moves vertex `i` in plan; the plane stays planar (RF-38). Editing makes
    /// the plane manual (RF-37).
    pub fn move_vertex(&mut self, i: usize, to: Point) {
        if i >= self.polygon3d.len() {
            return;
        }
        self.polygon3d[i][0] = to.x;
        self.polygon3d[i][2] = -to.y;
        if i == 0 {
            self.baseline.0 = to;
        } else if i == 1 {
            self.baseline.1 = to;
        }
        self.auto = false;
        self.resolve_heights();
    }

    /// Translates the plane in plan (RF-38 move handle).
    pub fn translate(&mut self, d: Point) {
        for v in &mut self.polygon3d {
            v[0] += d.x;
            v[2] -= d.y;
        }
        self.baseline = (self.baseline.0.add(d), self.baseline.1.add(d));
        for h in &mut self.holes {
            h.translate(d);
        }
        if let Some((a, b)) = &mut self.source {
            *a = a.add(d);
            *b = b.add(d);
        }
        self.auto = false;
    }

    /// Sloped surface area, square inches.
    pub fn area(&self) -> f64 {
        newell_area(&self.polygon3d)
    }

    pub fn pitch_label(&self) -> String {
        pitch_label(self.pitch)
    }

    #[cfg(test)]
    pub fn to_json_for_test(&self) -> Value {
        self.to_json()
    }

    #[cfg(test)]
    pub fn from_json_for_test(v: &Value) -> Option<Self> {
        Self::from_json(v)
    }

    /// The `Floor.roofs` entry of this plane.
    fn to_json(&self) -> Value {
        let mut v = json!({
            "kind": "plane",
            "id": self.id,
            "polygon3d": self.polygon3d,
            "pitch": self.pitch,
            "baseline": [self.baseline.0, self.baseline.1],
            "auto": self.auto,
            "holes": self.holes.iter().map(HoleRecord::to_json).collect::<Vec<_>>(),
            "source": self.source.map(|(a, b)| [a, b]),
            "overhang": self.overhang,
            "label": self.label,
            "material": self.material,
            "layer": self.layer,
            "ridge_caps": self.ridge_caps,
            "gutters": self.gutters,
        });
        if !self.eave.is_default() {
            if let (Value::Object(m), Ok(e)) = (&mut v, serde_json::to_value(self.eave)) {
                m.insert("eave".into(), e);
            }
        }
        if let (Value::Object(m), Some(st)) = (&mut v, self.structure) {
            if let Ok(e) = serde_json::to_value(st) {
                m.insert("structure".into(), e);
            }
        }
        if let (Value::Object(m), Some(l)) = (&mut v, &self.layers) {
            if let Ok(e) = serde_json::to_value(l) {
                m.insert("layers".into(), e);
            }
        }
        if let (Value::Object(m), Some(edges)) = (&mut v, &self.chief_edges) {
            m.insert("chief_edges".into(), edges.clone());
        }
        if let (Value::Object(m), Some(c)) = (&mut v, self.curved) {
            if let Ok(e) = serde_json::to_value(c) {
                m.insert("curved".into(), e);
            }
        }
        if let Value::Object(m) = &mut v {
            if let Some(t) = self.plate_top {
                m.insert("plate_top".into(), json!(t));
                m.insert("plate_width".into(), json!(self.plate_width));
            }
            if self.style != PlaneStyle::default() {
                if let Ok(e) = serde_json::to_value(&self.style) {
                    m.insert("style".into(), e);
                }
            }
            if !self.in_schedule {
                m.insert("in_schedule".into(), json!(false));
            }
            if !self.special_snapping {
                m.insert("special_snapping".into(), json!(false));
            }
        }
        v
    }

    /// A plane from a `Floor.roofs` entry or a legacy record; `None` when
    /// the geometry is missing or degenerate.
    fn from_json(v: &Value) -> Option<Self> {
        let polygon3d = field!(v, "polygon3d", Vec<[f64; 3]>)?;
        if polygon3d.len() < 3 {
            return None;
        }
        let base = field!(v, "baseline", Vec<Point>)?;
        let mut r = Self::new(
            field!(v, "id", Id)?,
            polygon3d,
            field!(v, "pitch", f64)?,
            (*base.first()?, *base.get(1)?),
        );
        r.auto = field!(v, "auto", bool).unwrap_or(false);
        r.holes = field!(v, "holes", Vec<Value>)
            .map(|hs| hs.iter().filter_map(HoleRecord::from_json).collect())
            .unwrap_or_default();
        r.source = field!(v, "source", Vec<Point>).and_then(|e| Some((*e.first()?, *e.get(1)?)));
        r.read_legacy_holes(v);
        r.overhang = field!(v, "overhang", f64).unwrap_or(0.0);
        r.label = field!(v, "label", String).unwrap_or_default();
        if let Some(m) = field!(v, "material", String) {
            r.material = m;
        }
        if let Some(l) = field!(v, "layer", String) {
            r.layer = l;
        }
        r.ridge_caps = field!(v, "ridge_caps", bool).unwrap_or(false);
        r.gutters = field!(v, "gutters", bool).unwrap_or(false);
        r.eave = field!(v, "eave", plan_3d::EaveOverrides).unwrap_or_default();
        r.structure = field!(v, "structure", RoofStructure);
        r.layers = field!(v, "layers", plan_core::assemblies::RoofLayers);
        r.chief_edges = v.get("chief_edges").filter(|e| e.is_array()).cloned();
        r.curved = field!(v, "curved", plan_roof::CurvedSpec);
        r.plate_top = field!(v, "plate_top", f64);
        r.plate_width = field!(v, "plate_width", f64).unwrap_or(DEFAULT_PLATE_WIDTH);
        r.style = field!(v, "style", PlaneStyle).unwrap_or_default();
        r.in_schedule = field!(v, "in_schedule", bool).unwrap_or(true);
        r.special_snapping = field!(v, "special_snapping", bool).unwrap_or(true);
        Some(r)
    }

    /// Files written before `holes`: one `hole` outline and `skylights` as
    /// `(center, width, length)` along the slope.
    fn read_legacy_holes(&mut self, v: &Value) {
        if let Some(h) = field!(v, "hole", Vec<Point>).filter(|h| h.len() >= 3) {
            self.holes.push(HoleRecord::hole(h));
        }
        let up = self.up_slope();
        let across = up.perp();
        for (c, w, l) in field!(v, "skylights", Vec<(Point, f64, f64)>).unwrap_or_default() {
            self.holes
                .push(HoleRecord::skylight(oriented_rect(c, w, l, across, up)));
        }
    }

    /// The plane as a `plan-roof` plane; `source_edge` tags it.
    pub fn to_roof_plane(&self, source_edge: usize) -> RoofPlane {
        RoofPlane {
            polygon3d: self.polygon3d.clone(),
            pitch_in_12: self.pitch,
            baseline: self.baseline,
            source_edge,
        }
    }

    /// The holes as `plan-roof` holes.
    pub fn roof_holes(&self) -> Vec<RoofHole> {
        self.holes.iter().map(HoleRecord::to_roof_hole).collect()
    }

    /// Does `outline` lie inside the plane with a margin, so `plan-roof`
    /// will cut it?
    pub fn encloses(&self, outline: &[Point]) -> bool {
        let poly = self.plan_polygon();
        let n = poly.len();
        outline.len() >= 3
            && outline.iter().all(|q| {
                point_in_polygon(*q, &poly)
                    && (0..n).all(|i| dist_to_segment(*q, poly[i], poly[(i + 1) % n]) > HOLE_MARGIN)
            })
    }
}

/// The plan rectangle `width` (along `across`) by `length` (along `up`)
/// centered on `c`.
pub fn oriented_rect(c: Point, width: f64, length: f64, across: Point, up: Point) -> Vec<Point> {
    let corner = |sw: f64, sl: f64| {
        c.add(across.scale(sw * width * 0.5))
            .add(up.scale(sl * length * 0.5))
    };
    vec![
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ]
}

/// `8:12`, or `8.5:12` for fractional pitches; `33.7°` with Pitch in
/// Degrees.
pub fn pitch_label(pitch: f64) -> String {
    // Degrees with Pitch in Degrees on (RF-72), else `x:12`.
    plan_roof::pitch_text(pitch)
}

fn newell_area(poly: &[[f64; 3]]) -> f64 {
    let n = poly.len();
    let mut s = [0.0; 3];
    for i in 0..n {
        let (c, d) = (poly[i], poly[(i + 1) % n]);
        s[0] += (c[1] - d[1]) * (c[2] + d[2]);
        s[1] += (c[2] - d[2]) * (c[0] + d[0]);
        s[2] += (c[0] - d[0]) * (c[1] + d[1]);
    }
    (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * 0.5
}

/// The Build Roof dialog's settings (RF-2), kept with the roof. Fields a
/// stored roof lacks load with Chief's stock values ([`RoofSettings::fallback`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RoofSettings {
    pub build_planes: bool,
    pub auto_rebuild: bool,
    /// Rise per 12 for walls without their own pitch.
    pub pitch: f64,
    /// Overhang from the wall face for walls without their own.
    pub overhang: f64,
    pub ignore_top_floor: bool,
    pub raise_off_plate: f64,
    pub build_ceiling_planes: bool,
    pub build_framing: bool,
    /// One-shot for the Build Roof dialog's "Build attic floor" check box
    /// (R-68): OK also calls `Project::build_attic_floor`. Never stored.
    pub build_attic_floor: bool,
    pub material: String,
    /// [`wall_signature`] at the last build (Auto Rebuild compares it).
    pub signature: u64,
    /// Per-edge overrides set in the Roof Plane Specification.
    pub edge_specs: Vec<EdgeSpec>,
    /// Build Roof's retain / baseline / pitch-in-degrees / curved-wall
    /// switches (RF-66..RF-72).
    pub switches: plan_roof::BuildSwitches,
    /// Build Roof's Roof Height group: framing method, Heel Height,
    /// birdsmouth, Same Roof Height at Exterior Walls, Same Height Eaves,
    /// Allow Low Roof Planes (brief 18).
    pub heights: plan_roof::HeightSettings,
    /// Roof detail: eave cut, fascia, soffit, rafter tails, attic walls and
    /// the baseline rule (Default Settings > Roof Defaults), kept with the
    /// roof so the 3D view needs no defaults.
    pub detail: RoofDetailDefaults,
}

impl RoofSettings {
    /// Chief's defaults: the exterior wall roof defaults (8:12, 16").
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            build_planes: true,
            auto_rebuild: true,
            pitch: d.exterior_wall.roof.pitch_in_12,
            overhang: d.exterior_wall.roof.overhang,
            ignore_top_floor: false,
            raise_off_plate: 0.0,
            build_ceiling_planes: false,
            build_framing: false,
            build_attic_floor: false,
            material: ROOF_MATERIALS[0].to_string(),
            signature: 0,
            edge_specs: Vec::new(),
            switches: plan_roof::BuildSwitches::default(),
            heights: plan_roof::HeightSettings::default(),
            detail: d.roof_detail.clone(),
        }
    }

    /// Chief's stock values (8:12, 16"), for fields a stored roof lacks.
    pub fn fallback() -> Self {
        Self {
            build_planes: true,
            auto_rebuild: true,
            pitch: 8.0,
            overhang: 16.0,
            ignore_top_floor: false,
            raise_off_plate: 0.0,
            build_ceiling_planes: false,
            build_framing: false,
            build_attic_floor: false,
            material: ROOF_MATERIALS[0].to_string(),
            signature: 0,
            edge_specs: Vec::new(),
            switches: plan_roof::BuildSwitches::default(),
            heights: plan_roof::HeightSettings::default(),
            detail: RoofDetailDefaults::default(),
        }
    }

    /// The `Floor.roofs` entry of these settings.
    fn to_json(&self) -> Value {
        json!({
            "kind": "settings",
            "build_planes": self.build_planes,
            "auto_rebuild": self.auto_rebuild,
            "pitch": self.pitch,
            "overhang": self.overhang,
            "ignore_top_floor": self.ignore_top_floor,
            "raise_off_plate": self.raise_off_plate,
            "build_ceiling_planes": self.build_ceiling_planes,
            "build_framing": self.build_framing,
            "material": self.material,
            "signature": self.signature,
            "edge_specs": self.edge_specs.iter().map(EdgeSpec::to_json).collect::<Vec<_>>(),
            "switches": self.switches,
            "heights": self.heights,
            "detail": self.detail,
        })
    }

    /// Settings from a stored entry; missing fields take `defaults`' values.
    fn from_json(v: &Value, defaults: &RoofSettings) -> Self {
        Self {
            build_planes: field!(v, "build_planes", bool).unwrap_or(defaults.build_planes),
            auto_rebuild: field!(v, "auto_rebuild", bool).unwrap_or(defaults.auto_rebuild),
            pitch: field!(v, "pitch", f64).unwrap_or(defaults.pitch),
            overhang: field!(v, "overhang", f64).unwrap_or(defaults.overhang),
            ignore_top_floor: field!(v, "ignore_top_floor", bool)
                .unwrap_or(defaults.ignore_top_floor),
            raise_off_plate: field!(v, "raise_off_plate", f64).unwrap_or(0.0),
            build_ceiling_planes: field!(v, "build_ceiling_planes", bool).unwrap_or(false),
            build_framing: field!(v, "build_framing", bool).unwrap_or(false),
            build_attic_floor: false,
            material: field!(v, "material", String).unwrap_or_else(|| defaults.material.clone()),
            signature: field!(v, "signature", u64).unwrap_or(0),
            edge_specs: field!(v, "edge_specs", Vec<Value>)
                .map(|e| e.iter().filter_map(EdgeSpec::from_json).collect())
                .unwrap_or_default(),
            switches: field!(v, "switches", plan_roof::BuildSwitches).unwrap_or_default(),
            heights: field!(v, "heights", plan_roof::HeightSettings).unwrap_or_default(),
            // A roof stored before the detail existed keeps its baseline.
            detail: field!(v, "detail", RoofDetailDefaults).unwrap_or_else(|| RoofDetailDefaults {
                baseline_at_plate: false,
                ..defaults.detail.clone()
            }),
        }
    }
}

impl RoofSettings {
    /// Inches the roof is lifted off the top plates: Raise/Lower All Roof
    /// Planes plus the Heel Height (trusses) or the manual Raise Off Plate /
    /// Birdsmouth Cut (rafters, automatic cut off).
    pub fn plate_raise(&self) -> f64 {
        self.raise_off_plate + self.heights.plate_lift()
    }

    /// The override stored for footprint edge `edge`, if any.
    pub fn override_of(&self, edge: (Point, Point)) -> Option<EdgeOverride> {
        self.edge_specs
            .iter()
            .find(|e| same_edge(e.edge, edge))
            .map(|e| e.over)
    }

    /// Sets (or, when `over` is the default, clears) the override of `edge`.
    pub fn set_override(&mut self, edge: (Point, Point), over: EdgeOverride) {
        self.edge_specs.retain(|e| !same_edge(e.edge, edge));
        if !over.is_default() {
            self.edge_specs.push(EdgeSpec { edge, over });
        }
    }
}

/// A vaulted ceiling plane (RF-45): `plan_roof::CeilingPlane` data on layer
/// "Ceiling Planes".
#[derive(Clone, Debug, PartialEq)]
pub struct CeilingRecord {
    pub id: Id,
    /// Plan outline, inches.
    pub outline: Vec<Point>,
    /// The plane rises toward the left of `baseline.0 -> baseline.1`.
    pub baseline: (Point, Point),
    pub pitch: f64,
    /// Scene elevation at the baseline, inches.
    pub height_at_baseline: f64,
    pub thickness: f64,
    pub layer: String,
    pub line_style: LineStyle,
    /// Made by Build Ceiling Planes and replaced by the next Build Roof.
    pub auto: bool,
}

impl CeilingRecord {
    /// A record for `plane` (Build Ceiling Planes makes these).
    pub fn from_plane(id: Id, plane: &CeilingPlane, auto: bool) -> Self {
        Self {
            id,
            outline: plane.outline.clone(),
            baseline: plane.baseline,
            pitch: plane.pitch_in_12,
            height_at_baseline: plane.height_at_baseline,
            thickness: plane.thickness,
            layer: LAYER_CEILING.to_string(),
            line_style: LineStyle::Dashed,
            auto,
        }
    }

    pub fn to_plane(&self) -> CeilingPlane {
        CeilingPlane {
            outline: self.outline.clone(),
            baseline: self.baseline,
            pitch_in_12: self.pitch,
            height_at_baseline: self.height_at_baseline,
            thickness: self.thickness,
        }
    }

    pub fn contains(&self, p: Point) -> bool {
        point_in_polygon(p, &self.outline)
    }

    pub fn translate(&mut self, d: Point) {
        for p in &mut self.outline {
            *p = p.add(d);
        }
        self.baseline = (self.baseline.0.add(d), self.baseline.1.add(d));
    }

    fn to_json(&self) -> Value {
        json!({
            "kind": "ceiling",
            "id": self.id,
            "outline": self.outline,
            "baseline": [self.baseline.0, self.baseline.1],
            "pitch": self.pitch,
            "height": self.height_at_baseline,
            "thickness": self.thickness,
            "layer": self.layer,
            "line_style": self.line_style,
            "auto": self.auto,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let outline = field!(v, "outline", Vec<Point>).filter(|o| o.len() >= 3)?;
        let base = field!(v, "baseline", Vec<Point>)?;
        Some(Self {
            id: field!(v, "id", Id)?,
            outline,
            baseline: (*base.first()?, *base.get(1)?),
            pitch: field!(v, "pitch", f64)?,
            height_at_baseline: field!(v, "height", f64)?,
            thickness: field!(v, "thickness", f64).unwrap_or(CEILING_THICKNESS),
            layer: field!(v, "layer", String).unwrap_or_else(|| LAYER_CEILING.to_string()),
            line_style: field!(v, "line_style", LineStyle).unwrap_or(LineStyle::Dashed),
            auto: field!(v, "auto", bool).unwrap_or(false),
        })
    }
}

/// An Auto Dormer (RF-48): the main plane and the dormer dimensions. The
/// dormer's planes, walls and the hole in the main roof come from
/// `plan_roof::auto_dormer`, see [`dormer_geometry`].
#[derive(Clone, Debug, PartialEq)]
pub struct DormerRecord {
    pub id: Id,
    /// The roof plane the dormer stands on.
    pub main: Id,
    pub spec: DormerSpec,
    pub layer: String,
    /// Auto Floating Dormer (RF-49): the dormer sits on the roof plane
    /// without cutting a hole through it.
    pub floating: bool,
}

impl DormerRecord {
    fn to_json(&self) -> Value {
        json!({
            "kind": "dormer",
            "id": self.id,
            "main": self.main,
            "spec": self.spec,
            "layer": self.layer,
            "floating": self.floating,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: field!(v, "id", Id)?,
            main: field!(v, "main", Id)?,
            spec: field!(v, "spec", DormerSpec)?,
            layer: field!(v, "layer", String).unwrap_or_else(|| LAYER_PLANES.to_string()),
            floating: field!(v, "floating", bool).unwrap_or(false),
        })
    }
}

/// Everything stored for one floor's roof.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoofSet {
    /// `None` until Build Roof ran (or after Delete Roof Planes).
    pub settings: Option<RoofSettings>,
    pub planes: Vec<RoofPlaneRecord>,
    pub ceilings: Vec<CeilingRecord>,
    pub dormers: Vec<DormerRecord>,
    /// The vertical faces Build Roof makes under Dutch gables (outward
    /// roof-space polygons, see `plan_roof::build_roof_with_faces`). They are
    /// not roof planes: not selectable, replaced by every rebuild.
    pub faces: Vec<Vec<[f64; 3]>>,
}

/// What an id of the roof slot names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Plane,
    Ceiling,
    Dormer,
}

impl RoofSet {
    pub fn plane(&self, id: Id) -> Option<&RoofPlaneRecord> {
        self.planes.iter().find(|p| p.id == id)
    }

    pub fn plane_mut(&mut self, id: Id) -> Option<&mut RoofPlaneRecord> {
        self.planes.iter_mut().find(|p| p.id == id)
    }

    pub fn ceiling(&self, id: Id) -> Option<&CeilingRecord> {
        self.ceilings.iter().find(|c| c.id == id)
    }

    pub fn dormer(&self, id: Id) -> Option<&DormerRecord> {
        self.dormers.iter().find(|d| d.id == id)
    }

    /// Which kind of record `id` is.
    pub fn kind_of(&self, id: Id) -> Option<RecordKind> {
        if self.plane(id).is_some() {
            Some(RecordKind::Plane)
        } else if self.ceiling(id).is_some() {
            Some(RecordKind::Ceiling)
        } else if self.dormer(id).is_some() {
            Some(RecordKind::Dormer)
        } else {
            None
        }
    }

    /// Topmost (last drawn) plane containing `p`.
    pub fn plane_at(&self, p: Point) -> Option<Id> {
        self.planes
            .iter()
            .rev()
            .find(|r| r.contains(p))
            .map(|r| r.id)
    }

    /// The dormer whose roof or front wall covers `p`.
    pub fn dormer_at(&self, p: Point) -> Option<Id> {
        self.dormers
            .iter()
            .rev()
            .find(|d| {
                dormer_geometry(self, d).is_some_and(|g| {
                    g.overhang_planes
                        .iter()
                        .any(|r| point_in_polygon(p, &r.plan_polygon()))
                })
            })
            .map(|d| d.id)
    }

    /// The topmost dormer, plane or ceiling plane under `p`, in that order.
    pub fn record_at(&self, p: Point) -> Option<Id> {
        self.dormer_at(p).or_else(|| self.plane_at(p)).or_else(|| {
            self.ceilings
                .iter()
                .rev()
                .find(|c| c.contains(p))
                .map(|c| c.id)
        })
    }

    /// Layer of record `id`.
    pub fn layer_of(&self, id: Id) -> Option<String> {
        self.plane(id)
            .map(|p| p.layer.clone())
            .or_else(|| self.ceiling(id).map(|c| c.layer.clone()))
            .or_else(|| self.dormer(id).map(|d| d.layer.clone()))
    }

    /// Plan polygons that stand for each record, for picking: a plane's
    /// outline, a ceiling plane's outline, a dormer's roof planes.
    pub fn pick_polys(&self) -> Vec<(Id, String, Vec<Vec<Point>>)> {
        let mut out: Vec<(Id, String, Vec<Vec<Point>>)> = self
            .planes
            .iter()
            .map(|r| (r.id, r.layer.clone(), vec![r.plan_polygon()]))
            .collect();
        out.extend(
            self.ceilings
                .iter()
                .map(|c| (c.id, c.layer.clone(), vec![c.outline.clone()])),
        );
        for d in &self.dormers {
            if let Some(g) = dormer_geometry(self, d) {
                out.push((
                    d.id,
                    d.layer.clone(),
                    g.overhang_planes
                        .iter()
                        .map(RoofPlane::plan_polygon)
                        .collect(),
                ));
            }
        }
        out
    }
}

/// The dormer of `rec` rebuilt on its main plane; `None` when the plane is
/// gone or the dormer no longer fits it.
pub fn dormer_geometry(set: &RoofSet, rec: &DormerRecord) -> Option<Dormer> {
    auto_dormer(&set.plane(rec.main)?.to_roof_plane(0), rec.spec)
}

// ===================================================================
// Storage in Floor.roofs
// ===================================================================

/// Reads the roof stored on `floor`. Entries that do not parse are skipped.
pub fn load(floor: &Floor) -> RoofSet {
    let base = RoofSettings::fallback();
    let mut set = RoofSet::default();
    for v in &floor.roofs {
        match v.get("kind").and_then(Value::as_str) {
            Some("plane") => set.planes.extend(RoofPlaneRecord::from_json(v)),
            Some("ceiling") => set.ceilings.extend(CeilingRecord::from_json(v)),
            Some("dormer") => set.dormers.extend(DormerRecord::from_json(v)),
            Some("face") => {
                if let Some(poly) = field!(v, "polygon3d", Vec<[f64; 3]>).filter(|p| p.len() >= 3) {
                    set.faces.push(poly);
                }
            }
            Some("settings") => set.settings = Some(RoofSettings::from_json(v, &base)),
            _ => {}
        }
    }
    if let Some(s) = &set.settings {
        // Dialogs and labels show degrees with Pitch in Degrees (RF-72).
        plan_roof::set_pitch_display_degrees(s.switches.pitch_in_degrees);
        for r in &mut set.planes {
            if let Some(edge) = r.source {
                r.edge = s.override_of(edge).unwrap_or_default();
            }
        }
    }
    set
}

/// Does [`load`] understand roof record `v`? Records it does not are opaque
/// and pass through [`store`] untouched.
fn readable(v: &Value) -> bool {
    match v.get("kind").and_then(Value::as_str) {
        Some("plane") => RoofPlaneRecord::from_json(v).is_some(),
        Some("ceiling") => CeilingRecord::from_json(v).is_some(),
        Some("dormer") => DormerRecord::from_json(v).is_some(),
        Some("face") => field!(v, "polygon3d", Vec<[f64; 3]>).is_some_and(|p| p.len() >= 3),
        Some("settings") => true,
        _ => false,
    }
}

/// Writes `set` back as the floor's roof, replacing what was stored. Records
/// this build cannot read stay as they are.
pub fn store(project: &mut Project, fi: usize, set: &mut RoofSet) {
    let mut items: Vec<Value> = set.planes.iter().map(RoofPlaneRecord::to_json).collect();
    items.extend(set.ceilings.iter().map(CeilingRecord::to_json));
    items.extend(set.dormers.iter().map(DormerRecord::to_json));
    items.extend(
        set.faces
            .iter()
            .map(|p| json!({"kind": "face", "polygon3d": p})),
    );
    if let Some(s) = &set.settings {
        items.push(s.to_json());
    }
    // A record this build cannot read (a newer build's kind) is written back
    // untouched after the edited ones (QA-29).
    items.extend(
        project.floors[fi]
            .roofs
            .iter()
            .filter(|v| !readable(v))
            .cloned(),
    );
    if !set.ceilings.is_empty() {
        project
            .layers
            .add(Layer::new(LAYER_CEILING, [96, 96, 160], 18));
    }
    // Plain data always serializes; on the impossible error the old roof stays.
    let _ = project.floors[fi].set_roofs(&items);
}

/// Does record `id` (a plane, ceiling plane or dormer) exist on `floor`?
pub fn exists(floor: &Floor, id: Id) -> bool {
    floor.roofs.iter().any(|v| {
        matches!(
            v.get("kind").and_then(Value::as_str),
            Some("plane" | "ceiling" | "dormer")
        ) && v.get("id").and_then(Value::as_u64) == Some(id)
    })
}

// ----- migration of the old CAD-record storage -----

fn data_text<'a>(c: &'a CadObject, tag: &str) -> Option<&'a str> {
    if c.layer != LAYER_DATA {
        return None;
    }
    match &c.item {
        CadItem::Text { text, .. } => text.strip_prefix(tag),
        _ => None,
    }
}

/// The roof stored the old way on `floor`: `RFP1:` / `RFS1:` text records on
/// the hidden data layer (the plane id is the CAD object id). Also returns the
/// ids of the outline polylines that went with the planes.
fn load_legacy(floor: &Floor) -> (RoofSet, Vec<Id>) {
    let mut set = RoofSet::default();
    let mut outlines = Vec::new();
    for c in &floor.cad {
        if let Some(json) = data_text(c, PLANE_TAG) {
            if let Some((mut r, outline)) = serde_json::from_str::<Value>(json)
                .ok()
                .and_then(|v| Some((RoofPlaneRecord::from_json(&v)?, field!(v, "outline_id", Id))))
            {
                r.id = c.id;
                outlines.extend(outline.filter(|o| *o != 0));
                set.planes.push(r);
            }
        } else if let Some(json) = data_text(c, SETTINGS_TAG) {
            if let Ok(v) = serde_json::from_str::<Value>(json) {
                set.settings = Some(RoofSettings::from_json(&v, &RoofSettings::fallback()));
            }
        }
    }
    (set, outlines)
}

/// Project-load step "Migrate roof storage" (no undo entry): on every floor
/// whose `roofs` slot is empty, moves the legacy CAD records into it, deletes
/// the data records and the plane outline polylines from `Floor.cad`, and
/// drops the hidden "Roof Planes, Data" layer. Returns whether anything moved.
pub fn migrate_legacy(project: &mut Project) -> bool {
    let mut changed = false;
    for fi in 0..project.floors.len() {
        if !project.floors[fi].roofs.is_empty() {
            continue;
        }
        let (mut set, outlines) = load_legacy(&project.floors[fi]);
        if set.planes.is_empty() && set.settings.is_none() {
            continue;
        }
        store(project, fi, &mut set);
        project.floors[fi]
            .cad
            .retain(|c| c.layer != LAYER_DATA && !outlines.contains(&c.id));
        changed = true;
    }
    super::site_view::drop_data_layer(project, LAYER_DATA);
    // The slab record kept the same way.
    changed |= plan_core::foundation::migrate_legacy(project);
    // Lights kept as hidden text records move into `Project::lights`.
    changed |= plan_core::camera::migrate_legacy(project);
    // CAD attributes, blocks, text macros and note types kept on "CAD, Data".
    changed |= plan_core::cad::migrate_legacy(project);
    changed
}

// ===================================================================
// Building (RF-1..RF-20)
// ===================================================================

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// Hash of everything about the exterior walls that shapes the roof.
pub fn wall_signature(floor: &Floor) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for w in floor.walls.iter().filter(|w| w.kind == WallKind::Exterior) {
        for v in [
            w.start.x,
            w.start.y,
            w.end.x,
            w.end.y,
            w.thickness,
            w.height,
        ] {
            fnv(&mut h, &v.to_bits().to_le_bytes());
        }
        fnv(&mut h, &w.id.to_le_bytes());
        fnv(&mut h, format!("{:?}", w.roof).as_bytes());
        // A curved wall is roofed in sections (RF-66).
        if let Some(c) = w.curve {
            fnv(&mut h, &c.bulge.to_bits().to_le_bytes());
        }
    }
    // A Roof Group splits the roof into buildings (R-114).
    for n in floor.room_names.iter().filter(|n| n.roof_group != 0) {
        fnv(&mut h, &n.anchor.x.to_bits().to_le_bytes());
        fnv(&mut h, &n.anchor.y.to_bits().to_le_bytes());
        fnv(&mut h, &n.roof_group.to_le_bytes());
    }
    // Roof Over This Room and Flat Roof Over This Room change the roof too.
    for n in &floor.room_names {
        if let Some(m) = n.misc.as_ref().filter(|m| !m.roof_over || m.flat_roof) {
            fnv(&mut h, &n.anchor.x.to_bits().to_le_bytes());
            fnv(&mut h, &n.anchor.y.to_bits().to_le_bytes());
            fnv(&mut h, &[u8::from(m.roof_over), u8::from(m.flat_roof)]);
        }
    }
    h
}

/// What Build Roof does over a room (R-30, R-40).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRoof {
    /// The pitched roof of the footprint covers it.
    Pitched,
    /// "Roof Over This Room" is off: nothing is built over it.
    None,
    /// "Flat Roof Over This Room": a level plane at its ceiling.
    Flat,
}

/// How Build Roof treats `room` of `floor`, from its name entry.
pub fn room_roof(floor: &Floor, room: &Room) -> RoomRoof {
    match room
        .name_entry(&floor.room_names)
        .and_then(|n| n.misc.as_ref())
    {
        Some(m) if !m.roof_over => RoomRoof::None,
        Some(m) if m.flat_roof => RoomRoof::Flat,
        _ => RoomRoof::Pitched,
    }
}

/// The rooms of `rooms` that lie along `w` (their centerline polygon has an
/// edge under the wall's middle).
fn rooms_beside<'a>(w: &Wall, rooms: &'a [Room]) -> Vec<&'a Room> {
    let mid = Point::lerp(w.start, w.end, 0.5);
    let dir = w.direction();
    rooms
        .iter()
        .filter(|r| {
            let n = r.polygon.len();
            (0..n).any(|i| {
                let (a, b) = (r.polygon[i], r.polygon[(i + 1) % n]);
                dist_to_segment(mid, a, b) <= w.thickness * 0.5 + TOL
                    && b.sub(a).normalized().cross(dir).abs() < 0.02
            })
        })
        .collect()
}

/// The walls that shape the roof's footprint: the exterior walls, except
/// knee walls (RF-23, they stand under a roof plane and make none of their
/// own) and the walls that only bound rooms the pitched roof skips (Roof
/// Over This Room off, Flat Roof). A partition between a roofed and a skipped
/// room takes the place of the skipped room's walls as the roof's edge. If
/// that leaves no closed footprint, the whole exterior is used.
fn exterior_walls(floor: &Floor) -> Vec<Wall> {
    with_unit_extensions(floor, plain_exterior_walls(floor))
}

/// The walls with the main roof brought down over every bay, box or bow
/// window that asks for Extend Existing Roof Over (manual p. 637).
fn with_unit_extensions(floor: &Floor, walls: Vec<Wall>) -> Vec<Wall> {
    if !floor
        .openings
        .iter()
        .any(|o| o.style.projects() && o.extras.spec.bay.extends_main_roof())
    {
        return walls;
    }
    let rooms = detect_rooms(&floor.walls, TOL);
    floor.extend_roof_over_units(walls, &|w| plan_core::exterior_sign(w, &rooms))
}

fn plain_exterior_walls(floor: &Floor) -> Vec<Wall> {
    let all: Vec<Wall> = floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .cloned()
        .collect();
    let valid = |ws: &[Wall]| ws.len() >= 3 && footprint_from_walls(ws, TOL).is_some();
    let roofing: Vec<Wall> = all
        .iter()
        .filter(|w| w.roof.kind != RoofWallKind::KneeWall)
        .cloned()
        .collect();
    let rooms = detect_rooms(&floor.walls, TOL);
    if rooms
        .iter()
        .any(|r| room_roof(floor, r) != RoomRoof::Pitched)
    {
        let skipped_side = |w: &Wall| {
            let beside = rooms_beside(w, &rooms);
            let skipped = beside
                .iter()
                .filter(|r| room_roof(floor, r) != RoomRoof::Pitched)
                .count();
            (beside.len(), skipped)
        };
        let mut kept: Vec<Wall> = Vec::new();
        for w in &roofing {
            let (n, skipped) = skipped_side(w);
            if !(n > 0 && skipped == n) {
                kept.push(w.clone());
            }
        }
        for w in floor.walls.iter().filter(|w| w.kind == WallKind::Interior) {
            let (n, skipped) = skipped_side(w);
            if skipped > 0 && skipped < n {
                kept.push(w.clone());
            }
        }
        if valid(&kept) {
            return kept;
        }
    }
    if valid(&roofing) {
        roofing
    } else {
        all
    }
}

/// The floor a roof is built over: the highest with exterior walls, or the one
/// below it with "Ignore Top Floor" (RF-2). `fallback` when no floor has any.
pub fn build_floor(project: &Project, ignore_top: bool, fallback: usize) -> usize {
    let with_walls: Vec<usize> = (0..project.floors.len())
        .filter(|&i| {
            project.floors[i]
                .walls
                .iter()
                .any(|w| w.kind == WallKind::Exterior)
        })
        .collect();
    match with_walls.len() {
        0 => fallback,
        1 => with_walls[0],
        n => with_walls[if ignore_top { n - 2 } else { n - 1 }],
    }
}

struct EdgePlan {
    spec: EdgeRoofSpec,
    /// Overhang from the wall face, as shown to the user.
    face_overhang: f64,
    /// A wall on the edge has Auto Roof Return on (RF-27).
    auto_return: bool,
    /// Length of those returns, inches.
    return_length: f64,
}

/// Indices of the walls lying along the footprint edge `a -> b`.
fn walls_on_edge(walls: &[Wall], a: Point, b: Point) -> Vec<usize> {
    let dir = b.sub(a).normalized();
    walls
        .iter()
        .enumerate()
        .filter(|(_, w)| {
            let mid = Point::lerp(w.start, w.end, 0.5);
            dist_to_segment(mid, a, b) <= w.thickness.max(1.0) * 0.5 + TOL
                && w.direction().cross(dir).abs() < 0.02
        })
        .map(|(i, _)| i)
        .collect()
}

/// What the surroundings of a roof add to its edges: wings and upper floors
/// that an edge butts against, and the wall an Extend Slope Downward edge
/// reaches down to.
struct EdgeCtx<'a> {
    /// Does the edge `a -> b` butt a taller wall or roof (a lower wing against
    /// the main house)? Such an edge is a high shed side (RF-13).
    butts: &'a dyn Fn(Point, Point) -> bool,
    /// How far the plane of edge `a -> b` reaches down to the wall below
    /// (RF-24), inches; `None` when no wall is below it.
    below: &'a dyn Fn(Point, Point) -> Option<f64>,
}

/// An [`EdgeCtx`] with no neighbours.
fn alone() -> EdgeCtx<'static> {
    EdgeCtx {
        butts: &|_, _| false,
        below: &|_, _| None,
    }
}

/// `eave_height` is the eave's height above the floor, which turns a wall's
/// "Starts at Height" (above the floor) into a rise over the eave.
fn edge_plans(
    walls: &[Wall],
    fp: &[Point],
    s: &RoofSettings,
    eave_height: f64,
    ctx: &EdgeCtx,
) -> Vec<EdgePlan> {
    let n = fp.len();
    let mut plans: Vec<EdgePlan> = (0..n)
        .map(|i| {
            let (a, b) = (fp[i], fp[(i + 1) % n]);
            let on = walls_on_edge(walls, a, b);
            // The longest wall on the edge speaks for it.
            let primary = on
                .iter()
                .map(|&k| &walls[k])
                .max_by(|x, y| x.length().total_cmp(&y.length()));
            let (kind, pitch, over, thick) = match primary {
                Some(w) => (
                    w.roof.kind,
                    w.roof.pitch_in_12.unwrap_or(s.pitch),
                    w.roof.overhang.unwrap_or(s.overhang),
                    w.thickness,
                ),
                None => (RoofWallKind::Hip, s.pitch, s.overhang, 0.0),
            };
            let mut spec = EdgeRoofSpec {
                pitch,
                // plan-roof measures from the centerline; RF-10 wants the
                // wall face.
                overhang: over + thick * 0.5,
                full_gable_wall: kind == RoofWallKind::FullGable,
                high_shed_gable: kind == RoofWallKind::HighShedGable,
                // The plane keeps sloping below its eave (RF-24): by the
                // wall's own drop, else down to the wall below, else a
                // fixed drop.
                extend_slope_downward: (kind == RoofWallKind::ExtendSlopeDownward).then(|| {
                    primary
                        .and_then(|w| w.roof.extend_drop)
                        .filter(|d| *d > 0.0)
                        .or_else(|| (ctx.below)(a, b).filter(|d| *d > 0.5))
                        .unwrap_or(EXTEND_SLOPE_DROP)
                }),
                dutch_gable: kind == RoofWallKind::DutchGable,
                ..EdgeRoofSpec::default()
            };
            // The second pitch and the height it starts at (RF-25); a
            // Dutch gable starts at the same height when one is given.
            if let Some((rise, start)) = primary.and_then(|w| w.roof.upper_pitch) {
                let over_eave = start - eave_height;
                if kind == RoofWallKind::FullGable {
                    // A gable wall has no plane to break: the second pitch
                    // is the hip that clips its peak (a half hip), starting
                    // at the given height.
                    spec.half_hip_pitch = Some(if rise > 0.0 { rise } else { s.pitch });
                    spec.half_hip_rise = (over_eave > 0.0).then_some(over_eave);
                } else if over_eave > 0.0 {
                    spec.break_rise = Some(over_eave);
                    if kind != RoofWallKind::DutchGable && rise > 0.0 {
                        spec.upper_pitch = Some(rise);
                    }
                }
            }
            let auto_return = on.iter().any(|&k| walls[k].roof.auto_roof_return);
            let return_length = on
                .iter()
                .filter_map(|&k| walls[k].roof.return_length)
                .find(|l| *l > 0.0)
                .unwrap_or(AUTO_RETURN_LENGTH);
            let mut face_overhang = over;
            // An edge against a taller wall is where a lower wing's roof
            // rises to the wall: no plane of its own, no overhang (RF-13).
            // An explicit gable wall stays a gable.
            if (ctx.butts)(a, b) && !spec.full_gable_wall && !spec.dutch_gable {
                spec.high_shed_gable = true;
                spec.upper_pitch = None;
                spec.break_rise = None;
                spec.extend_slope_downward = None;
                face_overhang = 0.0;
            }
            // The Roof Plane Specification's overrides win over the wall.
            if let Some(o) = s.override_of((a, b)) {
                if let Some(p) = o.pitch {
                    spec.pitch = p;
                }
                if let Some(v) = o.overhang {
                    spec.overhang = v + thick * 0.5;
                    face_overhang = v;
                }
                if o.gable {
                    spec.gable = true;
                    spec.high_shed_gable = false;
                }
            }
            EdgePlan {
                spec,
                face_overhang,
                auto_return,
                return_length,
            }
        })
        .collect();
    align_eaves(&mut plans, fp, s);
    plans
}

/// Does this edge have a plane of its own that slopes (not a gable end or
/// a high shed side)?
fn edge_slopes(p: &EdgePlan) -> bool {
    !(p.spec.high_shed_gable || p.spec.gable || p.spec.full_gable_wall)
}

/// Same Roof Height at Exterior Walls / Same Height Eaves (manual pp. 830
/// and 844): changes the overhang of planes whose pitch differs from the
/// default so their eaves meet the default plane's. An overhang typed in the
/// Roof Plane Specification is the user's own and stays.
fn align_eaves(plans: &mut [EdgePlan], fp: &[Point], s: &RoofSettings) {
    let n = fp.len();
    let pitches: Vec<Option<f64>> = plans
        .iter()
        .map(|p| edge_slopes(p).then_some(p.spec.pitch))
        .collect();
    let independent = plan_roof::independent_edges(&pitches);
    for (i, p) in plans.iter_mut().enumerate() {
        let edge = (fp[i], fp[(i + 1) % n]);
        if !edge_slopes(p) || s.override_of(edge).is_some_and(|o| o.overhang.is_some()) {
            continue;
        }
        // plan-roof measures from the centerline: keep the half thickness.
        let half = p.spec.overhang - p.face_overhang;
        let face = s.heights.eave_overhang(
            s.pitch,
            s.overhang,
            p.spec.pitch,
            p.face_overhang,
            independent[i],
        );
        p.face_overhang = face;
        p.spec.overhang = face + half;
    }
}

/// The eave-tip elevation of a roof whose eaves all meet the height of a
/// plane with the default pitch and overhang (Same Height Eaves), or `None`
/// when the roof is placed by its first hip edge instead.
fn default_eave_baseline(s: &RoofSettings, plans: &[EdgePlan], plate: f64) -> Option<f64> {
    if !s.heights.eaves_at_default_height() {
        return None;
    }
    let first = plans.iter().find(|p| edge_slopes(p))?;
    let half = first.spec.overhang - first.face_overhang;
    Some(plan_roof::seated_eave_elevation(
        plate,
        s.detail.thickness,
        s.pitch,
        s.overhang + half,
    ))
}

/// The footprint edge `wall_id` lies on, and the ids of the walls along it.
fn edge_of_wall(floor: &Floor, wall_id: Id) -> Option<((Point, Point), Vec<Id>)> {
    let walls = exterior_walls(floor);
    let fp = footprint_from_walls(&walls, TOL)?;
    let n = fp.len();
    for i in 0..n {
        let edge = (fp[i], fp[(i + 1) % n]);
        let on = walls_on_edge(&walls, edge.0, edge.1);
        if on.iter().any(|&k| walls[k].id == wall_id) {
            return Some((edge, on.iter().map(|&k| walls[k].id).collect()));
        }
    }
    None
}

/// Does the Build Roof override of footprint edge `edge` make it a gable?
fn edge_override_is_gable(project: &Project, edge: (Point, Point)) -> bool {
    project.floors.iter().any(|f| {
        load(f)
            .settings
            .and_then(|s| s.override_of(edge))
            .is_some_and(|o| o.gable)
    })
}

/// Gable/Roof Line on a wall (RF-18, RF-20): flips every wall along its
/// footprint edge between Hip and Full Gable. A gable that comes from the
/// roof settings' edge override (set from an eave) counts as a gable: the
/// override is cleared and the walls become Hip. Returns the new kind.
pub fn toggle_gable(project: &mut Project, fi: usize, wall_id: Id) -> Option<RoofWallKind> {
    let floor = &project.floors[fi];
    let (edge, ids) = match edge_of_wall(floor, wall_id) {
        Some((e, ids)) => (Some(e), ids),
        None => (None, vec![wall_id]),
    };
    let current = floor.wall(wall_id)?.roof.kind;
    let overridden = edge.is_some_and(|e| edge_override_is_gable(project, e));
    let new = if current == RoofWallKind::FullGable || overridden {
        RoofWallKind::Hip
    } else {
        RoofWallKind::FullGable
    };
    for id in ids {
        if let Some(w) = project.floors[fi].wall_mut(id) {
            w.roof.kind = new;
        }
    }
    if let (Some(edge), true) = (edge, overridden) {
        for f in 0..project.floors.len() {
            let mut set = load(&project.floors[f]);
            let Some(s) = set.settings.as_mut() else {
                continue;
            };
            if let Some(mut o) = s.override_of(edge).filter(|o| o.gable) {
                o.gable = false;
                s.set_override(edge, o);
                // The walls did not change: make Auto Rebuild notice.
                s.signature = 0;
                store(project, f, &mut set);
            }
        }
    }
    Some(new)
}

// ===================================================================
// Roof styles and wall directives (RF-3, RF-4)
// ===================================================================

/// The roof style presets of Build Roof (Chief's "Roof Styles" buttons). A
/// style is a shortcut: it writes the Roof tab of the exterior walls, and
/// Build Roof builds from those (RF-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoofStyle {
    Hip,
    Gable,
    Shed,
    Gambrel,
    DutchGable,
    HalfHip,
}

/// Lower (steep) and upper (shallow) pitch of a gambrel preset, rise per 12.
pub const GAMBREL_PITCHES: (f64, f64) = (18.0, 6.0);

impl RoofStyle {
    pub const ALL: [RoofStyle; 6] = [
        RoofStyle::Hip,
        RoofStyle::Gable,
        RoofStyle::Shed,
        RoofStyle::Gambrel,
        RoofStyle::DutchGable,
        RoofStyle::HalfHip,
    ];

    /// Chief's names for the presets.
    pub fn label(self) -> &'static str {
        match self {
            RoofStyle::Hip => "Hip",
            RoofStyle::Gable => "Gable",
            RoofStyle::Shed => "Shed",
            RoofStyle::Gambrel => "Gambrel",
            RoofStyle::DutchGable => "Dutch Gable",
            RoofStyle::HalfHip => "Half Hip",
        }
    }
}

/// How a footprint edge lies against the ridge a style puts along the
/// footprint's longest edge.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeRole {
    /// Parallel to the ridge: an eave.
    Eave,
    /// Across the ridge: an end.
    End,
    Other,
}

/// A wall's new directive from a style: id, kind, pitch and upper pitch.
type StyleWrite = (Id, RoofWallKind, Option<f64>, Option<(f64, f64)>);

/// The directive a style gives an edge role: kind, pitch and upper pitch.
fn style_directive(
    style: RoofStyle,
    role: EdgeRole,
    first_eave: bool,
    pitch: f64,
    eave_top: f64,
    half_span: f64,
) -> (RoofWallKind, Option<f64>, Option<(f64, f64)>) {
    use RoofWallKind::*;
    match (style, role) {
        (RoofStyle::Hip, _) | (_, EdgeRole::Other) => (Hip, None, None),
        (RoofStyle::Gable, EdgeRole::End) => (FullGable, None, None),
        (RoofStyle::Gable, EdgeRole::Eave) => (Hip, None, None),
        // One long wall is the high side; the ends are gables.
        (RoofStyle::Shed, EdgeRole::End) => (FullGable, None, None),
        (RoofStyle::Shed, EdgeRole::Eave) => {
            if first_eave {
                (HighShedGable, None, None)
            } else {
                (Hip, None, None)
            }
        }
        (RoofStyle::Gambrel, EdgeRole::End) => (FullGable, None, None),
        (RoofStyle::Gambrel, EdgeRole::Eave) => {
            let (lower, upper) = GAMBREL_PITCHES;
            // The break is half way in from the eave.
            let rise = lower / 12.0 * half_span * 0.5;
            (Hip, Some(lower), Some((upper, eave_top + rise)))
        }
        (RoofStyle::DutchGable, EdgeRole::End) => (DutchGable, None, None),
        (RoofStyle::DutchGable, EdgeRole::Eave) => (Hip, None, None),
        (RoofStyle::HalfHip, EdgeRole::End) => {
            // The hip starts 60% of the way up the gable.
            let ridge_rise = pitch / 12.0 * half_span;
            (
                FullGable,
                None,
                Some((
                    pitch,
                    eave_top + ridge_rise * plan_roof::DEFAULT_CLIP_FRACTION,
                )),
            )
        }
        (RoofStyle::HalfHip, EdgeRole::Eave) => (Hip, None, None),
    }
}

/// Applies a roof style (RF-3, RF-4) to floor `fi`: writes the roof directive
/// of every exterior wall along the footprint. `pitch` is the roof's pitch
/// (the dialog's default). Walls keep their own pitch and overhang, except a
/// gambrel's eaves, which take its steep lower pitch. Returns how many walls
/// were written.
pub fn apply_style(
    project: &mut Project,
    fi: usize,
    style: RoofStyle,
    pitch: f64,
) -> Result<usize, String> {
    let floor = &project.floors[fi];
    let walls = exterior_walls(floor);
    let fp = footprint_from_walls(&walls, TOL)
        .ok_or_else(|| "The exterior walls do not enclose an area".to_string())?;
    let n = fp.len();
    // The ridge runs along the longest edge.
    let longest = (0..n)
        .max_by(|&a, &b| {
            let la = fp[(a + 1) % n].sub(fp[a]).length();
            let lb = fp[(b + 1) % n].sub(fp[b]).length();
            la.total_cmp(&lb)
        })
        .unwrap_or(0);
    let ridge = fp[(longest + 1) % n].sub(fp[longest]).normalized();
    let across = fp
        .iter()
        .map(|p| p.dot(ridge.perp()))
        .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let half_span = (across.1 - across.0) * 0.5;
    let eave_top = walls.iter().map(|w| w.height).fold(0.0, f64::max);
    let role_of = |i: usize| {
        let dir = fp[(i + 1) % n].sub(fp[i]).normalized();
        if dir.cross(ridge).abs() < 0.1 {
            EdgeRole::Eave
        } else if dir.dot(ridge).abs() < 0.1 {
            EdgeRole::End
        } else {
            EdgeRole::Other
        }
    };
    // The shed's high side is the eave edge furthest back (smallest y).
    let high_edge = (0..n)
        .filter(|&i| role_of(i) == EdgeRole::Eave)
        .min_by(|&i, &j| {
            let y = |k: usize| Point::lerp(fp[k], fp[(k + 1) % n], 0.5).y;
            y(i).total_cmp(&y(j))
        });
    let mut changed = 0;
    let mut writes: Vec<StyleWrite> = Vec::new();
    for i in 0..n {
        let (a, b) = (fp[i], fp[(i + 1) % n]);
        let (kind, wall_pitch, upper) = style_directive(
            style,
            role_of(i),
            high_edge == Some(i),
            pitch,
            eave_top,
            half_span,
        );
        for k in walls_on_edge(&walls, a, b) {
            writes.push((walls[k].id, kind, wall_pitch, upper));
        }
    }
    for (id, kind, wall_pitch, upper) in writes {
        if let Some(w) = project.floors[fi].wall_mut(id) {
            w.roof.kind = kind;
            w.roof.upper_pitch = upper;
            match wall_pitch {
                Some(p) => w.roof.pitch_in_12 = Some(p),
                // Another style clears the steep pitch a gambrel gave.
                None if w.roof.pitch_in_12 == Some(GAMBREL_PITCHES.0) => w.roof.pitch_in_12 = None,
                None => {}
            }
            changed += 1;
        }
    }
    Ok(changed)
}

/// Sets the roof directive of the walls `ids` (the per-wall Roof Options of
/// the Wall Specification, applied from the selection, RF-4): the whole
/// footprint edge each lies on follows, so the wall that speaks for the edge
/// is the one changed. A Knee Wall is the wall itself only. Returns how many
/// walls changed.
pub fn set_walls_roof_kind(
    project: &mut Project,
    fi: usize,
    ids: &[Id],
    kind: RoofWallKind,
) -> usize {
    let mut targets: Vec<Id> = Vec::new();
    for &id in ids {
        if project.floors[fi].wall(id).is_none() {
            continue;
        }
        let along = if kind == RoofWallKind::KneeWall {
            None
        } else {
            edge_of_wall(&project.floors[fi], id).map(|(_, ids)| ids)
        };
        for t in along.unwrap_or_else(|| vec![id]) {
            if !targets.contains(&t) {
                targets.push(t);
            }
        }
    }
    let mut n = 0;
    for id in targets {
        if let Some(w) = project.floors[fi].wall_mut(id) {
            if w.roof.kind != kind {
                w.roof.kind = kind;
                n += 1;
            }
        }
    }
    n
}

/// Context-menu commands that set the roof directive of the selected walls.
pub const WALL_ROOF_COMMANDS: [(&str, &str, RoofWallKind); 5] = [
    ("roof.wall.hip", "Hip Wall", RoofWallKind::Hip),
    (
        "roof.wall.gable",
        "Full Gable Wall",
        RoofWallKind::FullGable,
    ),
    (
        "roof.wall.shed",
        "High Shed/Gable Wall",
        RoofWallKind::HighShedGable,
    ),
    ("roof.wall.knee", "Knee Wall", RoofWallKind::KneeWall),
    (
        "roof.wall.dutch",
        "Dutch Gable Wall",
        RoofWallKind::DutchGable,
    ),
];

/// The Edit toolbar buttons that set the roof directive of the selected
/// exterior walls (RF-4); none unless the selection is walls with an exterior
/// one among them.
pub fn wall_edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let sel = &cx.selection.items;
    let walls: Vec<Id> = sel
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    if walls.is_empty()
        || walls.len() != sel.len()
        || !walls.iter().any(|id| {
            cx.floor()
                .wall(*id)
                .is_some_and(|w| w.kind == WallKind::Exterior)
        })
    {
        return Vec::new();
    }
    WALL_ROOF_COMMANDS
        .iter()
        .map(|(id, label, _)| {
            EditAction::new(EditActionKind::Custom {
                id,
                label,
                icon: "",
            })
        })
        .collect()
}

/// Runs a roof command of [`WALL_ROOF_COMMANDS`] on the selected walls: one
/// undo step, and the roof follows (Auto Rebuild). Returns whether `id` was
/// one of them.
pub fn run_wall_command(cx: &mut EditorContext, id: &str) -> bool {
    let Some((_, label, kind)) = WALL_ROOF_COMMANDS.iter().find(|(c, _, _)| *c == id) else {
        return false;
    };
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Wall(id) => Some(*id),
            _ => None,
        })
        .collect();
    let fl = cx.floor;
    cx.begin_change(label);
    let n = set_walls_roof_kind(&mut cx.project, fl, &ids, *kind);
    if n == 0 {
        cx.cancel_change();
        cx.status = format!("Already a {label}");
        return true;
    }
    cx.mark_dirty();
    auto_rebuild(cx);
    cx.status = format!("{label}: {n} wall{}", if n == 1 { "" } else { "s" });
    true
}

// ===================================================================
// Roof levels: wings with their own plate heights (RF-9)
// ===================================================================

/// Exterior walls whose heights differ by less than this share a roof level,
/// inches.
const LEVEL_TOL: f64 = 1.0;
/// How far outside an edge to look for the building next to it, inches.
const BUTT_PROBE: f64 = 8.0;
/// A room is under the floor above when this share of it is. A room only
/// partly under an upper floor is roofed by nobody (the lower roof would have
/// to wrap the upper floor), as before wings existed.
const COVERED_SHARE: f64 = 0.05;

/// One roof of its own: a footprint with the plate height of its walls.
#[derive(Clone, Debug)]
struct Region {
    /// Index of the floor the walls stand on.
    floor: usize,
    /// Counter-clockwise outline along the wall centerlines.
    fp: Vec<Point>,
    /// Wall height above the floor, inches.
    top: f64,
}

/// Planes, faces and the "approximate" flag of one or more roofs.
#[derive(Default)]
struct RegionRoof {
    planes: Vec<RoofPlaneRecord>,
    approximate: bool,
    faces: Vec<Vec<[f64; 3]>>,
}

impl RegionRoof {
    fn add(&mut self, other: RegionRoof) {
        self.planes.extend(other.planes);
        self.approximate |= other.approximate;
        self.faces.extend(other.faces);
    }
}

/// Curved walls replaced by the straight sections an automatic roof is built
/// over: the Segment Angle at Curved Wall and the Minimum Alcove Size of the
/// Build Roof switches (RF-66, RF-67).
fn curved_sections(walls: Vec<Wall>, s: &RoofSettings) -> Vec<Wall> {
    plan_roof::flatten_curved_walls(&walls, s.switches.segment_angle, s.switches.min_alcove)
}

/// The exterior walls that make planes: all but the knee walls (RF-23).
fn roofing_walls(floor: &Floor) -> Vec<Wall> {
    let walls: Vec<Wall> = floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior && w.roof.kind != RoofWallKind::KneeWall)
        .cloned()
        .collect();
    with_unit_extensions(floor, walls)
}

/// Does the room's outline run along wall `w`?
fn room_runs_along(room: &Room, w: &Wall) -> bool {
    let n = room.polygon.len();
    let dir = w.direction();
    let wall_mid = Point::lerp(w.start, w.end, 0.5);
    (0..n).any(|i| {
        let (a, b) = (room.polygon[i], room.polygon[(i + 1) % n]);
        if b.sub(a).normalized().cross(dir).abs() >= 0.02 {
            return false;
        }
        let reach = w.thickness * 0.5 + TOL;
        dist_to_segment(Point::lerp(a, b, 0.5), w.start, w.end) <= reach
            || dist_to_segment(wall_mid, a, b) <= reach
    })
}

/// The plate height of a room: its lowest exterior wall (a shared wall drawn
/// at the main house's height does not lift a wing's roof). `None` for a room
/// with no exterior wall.
fn room_level(floor: &Floor, room: &Room) -> Option<f64> {
    floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior && w.roof.kind != RoofWallKind::KneeWall)
        // A deck's railing or a half wall is not a plate.
        .filter(|w| !w.is_deck_edge && !w.flags.half_wall)
        .filter(|w| room_runs_along(room, w))
        .map(|w| w.height)
        .reduce(f64::min)
}

/// Rooms grouped into the sets that are roofed together, tallest roof first.
/// Rooms in the default Roof Group (0) are grouped by plate height (DECISIONS
/// 118); rooms of any other Roof Group form one building each, whatever their
/// plate heights (R-114, `plan_roof::assign_roof_groups`). Rooms with no
/// exterior wall join the tallest group.
fn level_groups(floor: &Floor, rooms: &[&Room]) -> Vec<(f64, Vec<Room>)> {
    let tallest = floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior && !w.is_deck_edge && !w.flags.half_wall)
        .map(|w| w.height)
        .fold(0.0, f64::max);
    let grouped: Vec<plan_roof::GroupedRoom> = rooms
        .iter()
        .map(|r| plan_roof::GroupedRoom {
            group: r.name_entry(&floor.room_names).map_or(0, |n| n.roof_group),
            level: room_level(floor, r),
        })
        .collect();
    plan_roof::assign_roof_groups(&grouped, tallest, LEVEL_TOL)
        .into_iter()
        .map(|a| {
            let group: Vec<Room> = a.rooms.iter().map(|&i| rooms[i].clone()).collect();
            (a.level, group)
        })
        .collect()
}

/// The wing groups of the build floor, when its pitched rooms stand at more
/// than one plate height; `None` when one footprint roofs the floor.
fn floor_levels(floor: &Floor, rooms: &[Room]) -> Option<Vec<(f64, Vec<Room>)>> {
    if rooms.is_empty()
        || rooms
            .iter()
            .any(|r| room_roof(floor, r) != RoomRoof::Pitched)
    {
        return None;
    }
    let refs: Vec<&Room> = rooms.iter().collect();
    let groups = level_groups(floor, &refs);
    (groups.len() > 1).then_some(groups)
}

/// Outer boundaries of the union of `rooms`, one per connected piece.
fn rooms_footprints(rooms: &[Room]) -> Vec<Vec<Point>> {
    let mut left: Vec<&Room> = rooms.iter().collect();
    let mut out = Vec::new();
    while !left.is_empty() {
        let walls: Vec<Wall> = left
            .iter()
            .flat_map(|r| {
                let n = r.polygon.len();
                (0..n).map(move |i| {
                    Wall::new(
                        r.polygon[i],
                        r.polygon[(i + 1) % n],
                        6.0,
                        96.0,
                        WallKind::Exterior,
                    )
                })
            })
            .collect();
        let Some(fp) = footprint_from_walls(&walls, TOL) else {
            break;
        };
        let before = left.len();
        left.retain(|r| {
            let inside = point_in_polygon(r.centroid, &fp)
                || r.polygon
                    .iter()
                    .all(|p| point_in_polygon(*p, &fp) || on_outline(*p, &fp));
            !inside
        });
        out.push(fp);
        if left.len() == before {
            break;
        }
    }
    out
}

/// Is `p` on the outline of `poly`?
fn on_outline(p: Point, poly: &[Point]) -> bool {
    let n = poly.len();
    (0..n).any(|i| dist_to_segment(p, poly[i], poly[(i + 1) % n]) <= TOL)
}

/// The regions of the build floor's wing groups.
fn level_regions(project: &Project, fi: usize, groups: &[(f64, Vec<Room>)]) -> Vec<Region> {
    let _ = project;
    groups
        .iter()
        .flat_map(|(level, rooms)| {
            rooms_footprints(rooms).into_iter().map(|fp| Region {
                floor: fi,
                fp,
                top: *level,
            })
        })
        .collect()
}

/// Outlines of the exterior walls of floors `from + 1 ..= to` that carry a
/// roof of their own (not foundations).
fn upper_footprints(project: &Project, from: usize, to: usize) -> Vec<Vec<Point>> {
    (from + 1..=to.min(project.floors.len().saturating_sub(1)))
        .filter_map(|h| {
            let f = &project.floors[h];
            if f.kind == plan_core::FloorKind::Foundation {
                return None;
            }
            let walls: Vec<Wall> = f
                .walls
                .iter()
                .filter(|w| w.kind == WallKind::Exterior)
                .cloned()
                .collect();
            footprint_from_walls(&walls, TOL)
        })
        .collect()
}

/// The share of `room` that lies under any of the `uppers` outlines.
fn covered_share(room: &Room, uppers: &[Vec<Point>]) -> f64 {
    let (mut lo, mut hi) = (room.polygon[0], room.polygon[0]);
    for p in &room.polygon {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    const N: usize = 9;
    let (mut inside, mut covered) = (0, 0);
    for i in 0..N {
        for j in 0..N {
            let p = Point::new(
                lo.x + (hi.x - lo.x) * (i as f64 + 0.5) / N as f64,
                lo.y + (hi.y - lo.y) * (j as f64 + 0.5) / N as f64,
            );
            if point_in_polygon(p, &room.polygon) {
                inside += 1;
                if uppers.iter().any(|u| point_in_polygon(p, u)) {
                    covered += 1;
                }
            }
        }
    }
    if inside == 0 {
        f64::from(u8::from(
            uppers.iter().any(|u| point_in_polygon(room.centroid, u)),
        ))
    } else {
        f64::from(covered) / f64::from(inside)
    }
}

/// The wing regions of floor `g` under the build floor `fi`: its pitched
/// rooms the floors above do not cover, by plate height (RF-9, a one-story
/// garage beside a two-story house).
fn wing_regions(project: &Project, fi: usize, g: usize) -> Vec<Region> {
    let floor = &project.floors[g];
    if g >= fi || floor.kind != plan_core::FloorKind::Normal || floor.elevation < 0.0 {
        return Vec::new();
    }
    let uppers = upper_footprints(project, g, fi);
    if uppers.is_empty() || !floor.walls.iter().any(|w| w.kind == WallKind::Exterior) {
        return Vec::new();
    }
    let rooms = detect_rooms(&floor.walls, TOL);
    let exposed: Vec<&Room> = rooms
        .iter()
        .filter(|r| room_roof(floor, r) == RoomRoof::Pitched)
        .filter(|r| r.polygon.len() >= 3 && covered_share(r, &uppers) < COVERED_SHARE)
        .collect();
    if exposed.is_empty() {
        return Vec::new();
    }
    level_groups(floor, &exposed)
        .iter()
        .flat_map(|(level, rooms)| {
            rooms_footprints(rooms).into_iter().map(|fp| Region {
                floor: g,
                fp,
                top: *level,
            })
        })
        .collect()
}

/// Does edge `a -> b` of region `me` butt a taller wall: an upper floor's
/// outline, or a wing of the same floor with a higher plate (RF-13)?
fn edge_butts(
    project: &Project,
    me: &Region,
    regions: &[Region],
    uppers: &[Vec<Point>],
    a: Point,
    b: Point,
) -> bool {
    let d = b.sub(a);
    if d.length() < 1.0 {
        return false;
    }
    let dir = d.normalized();
    let out = Point::new(dir.y, -dir.x);
    let plate = |r: &Region| project.floors[r.floor].elevation + r.top;
    [0.25, 0.5, 0.75].iter().any(|t| {
        let q = Point::lerp(a, b, *t).add(out.scale(BUTT_PROBE));
        uppers.iter().any(|u| point_in_polygon(q, u))
            || regions.iter().any(|o| {
                o.floor == me.floor
                    && plate(o) > plate(me) + LEVEL_TOL
                    && point_in_polygon(q, &o.fp)
            })
    })
}

/// How far a plane on edge `a -> b` reaches down to the wall below it: the
/// eave (absolute height `eave`) down to the top of the nearest lower floor's
/// exterior wall within 6 feet of the edge (RF-24).
fn wall_below_drop(project: &Project, fi: usize, eave: f64, a: Point, b: Point) -> Option<f64> {
    let d = b.sub(a);
    if d.length() < 1.0 {
        return None;
    }
    let dir = d.normalized();
    let mid = Point::lerp(a, b, 0.5);
    for g in (0..fi).rev() {
        let fl = &project.floors[g];
        if fl.kind == plan_core::FloorKind::Foundation {
            continue;
        }
        let best = fl
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior && w.roof.kind != RoofWallKind::KneeWall)
            .filter(|w| w.direction().cross(dir).abs() < 0.1)
            .map(|w| {
                (
                    dist_to_segment(mid, w.start, w.end),
                    fl.elevation + w.height,
                )
            })
            .filter(|(dist, top)| *dist <= 72.0 && *top < eave - 0.5)
            .min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((_, top)) = best {
            return Some((eave - top).min(240.0));
        }
    }
    None
}

/// The planes (without ids), roof returns and Dutch faces of the roof over
/// `fp`, whose walls stand `top` inches high on floor `fi`. `butts` tells
/// which edges meet a taller wall.
fn region_roof(
    project: &Project,
    fi: usize,
    walls: &[Wall],
    fp: &[Point],
    top: f64,
    s: &RoofSettings,
    butts: &dyn Fn(Point, Point) -> bool,
) -> Result<RegionRoof, String> {
    let floor = &project.floors[fi];
    let plate = floor.elevation + top + s.plate_raise();
    let below = |a: Point, b: Point| wall_below_drop(project, fi, plate, a, b);
    let ctx = EdgeCtx {
        butts,
        below: &below,
    };
    let plans = edge_plans(walls, fp, s, top + s.plate_raise(), &ctx);
    let specs: Vec<EdgeRoofSpec> = plans.iter().map(|e| e.spec).collect();
    // With the baseline rule on the structure sits on the top plate at
    // the wall; without it the eave tip is at plate height.
    let (roof, faces) = if s.detail.baseline_at_plate {
        match default_eave_baseline(s, &plans, plate) {
            Some(eave) => build_roof_with_faces(fp, &specs, eave),
            None => build_roof_at_plate_with_faces(fp, &specs, plate, s.detail.thickness),
        }
    } else {
        build_roof_with_faces(fp, &specs, plate)
    };
    let n = fp.len();
    let returns = auto_returns(&roof.planes, &plans);
    let mut out = Vec::new();
    for pl in roof.planes {
        let mut r = RoofPlaneRecord::new(0, pl.polygon3d, pl.pitch_in_12, pl.baseline);
        r.auto = true;
        r.source = Some((fp[pl.source_edge % n], fp[(pl.source_edge + 1) % n]));
        r.overhang = plans
            .get(pl.source_edge)
            .map_or(s.overhang, |e| e.face_overhang);
        r.material = s.material.clone();
        // The plate the plane bears on, for the Roof Plane Specification's
        // Top of Plate and birdsmouth read-outs (brief 18b).
        r.plate_top = Some(plate);
        let on = walls_on_edge(walls, fp[pl.source_edge % n], fp[(pl.source_edge + 1) % n]);
        if let Some(w) = on.first().map(|&k| &walls[k]) {
            r.plate_width = w.thickness.max(1.0);
        }
        // Include Frieze off on every wall of the edge keeps the frieze off
        // this plane (manual p. 428).
        if !on.is_empty() && on.iter().all(|&k| !walls[k].roof.include_frieze) {
            r.eave.frieze = Some(false);
        }
        out.push(r);
    }
    for (src, ret) in returns {
        let mut r = RoofPlaneRecord::new(0, ret.polygon3d, ret.pitch_in_12, ret.baseline);
        r.auto = true;
        r.material = s.material.clone();
        r.overhang = plans.get(src).map_or(s.overhang, |e| e.face_overhang);
        out.push(r);
    }
    Ok(RegionRoof {
        planes: out,
        approximate: roof.approximate,
        faces,
    })
}

/// The wings of the floors below the build floor `fi`: for each lower floor
/// with a part the floors above do not cover, the roofs over that part.
fn make_wing_planes(
    project: &Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<Vec<(usize, RegionRoof)>, String> {
    let uppers_of = |g: usize| upper_footprints(project, g, fi);
    let mut out = Vec::new();
    for g in 0..fi {
        let regions = wing_regions(project, fi, g);
        if regions.is_empty() {
            continue;
        }
        let uppers = uppers_of(g);
        let walls = curved_sections(roofing_walls(&project.floors[g]), s);
        let mut all = RegionRoof::default();
        for r in &regions {
            let butts = |a: Point, b: Point| edge_butts(project, r, &regions, &uppers, a, b);
            all.add(region_roof(project, g, &walls, &r.fp, r.top, s, &butts)?);
        }
        out.push((g, all));
    }
    Ok(out)
}

/// One roof's footprint with the directives of its walls, for Make Roof
/// Baseline Polylines (RF-62): what a baseline polyline is made from.
pub(crate) struct BaselineSource {
    /// The floor the walls stand on (and the polyline goes on).
    pub floor: usize,
    /// Counter-clockwise outline along the wall centerlines.
    pub fp: Vec<Point>,
    /// The roof directive of each edge.
    pub specs: Vec<EdgeRoofSpec>,
    /// Half the thickness of the wall along each edge, inches.
    pub half: Vec<f64>,
    /// Eave elevation above the floor datum of `floor`, inches.
    pub height: f64,
}

/// The footprint, directives and eave height of one roof region.
fn region_source(
    project: &Project,
    fi: usize,
    walls: &[Wall],
    fp: &[Point],
    top: f64,
    s: &RoofSettings,
    butts: &dyn Fn(Point, Point) -> bool,
) -> BaselineSource {
    let floor = &project.floors[fi];
    let plate = floor.elevation + top + s.plate_raise();
    let below = |a: Point, b: Point| wall_below_drop(project, fi, plate, a, b);
    let ctx = EdgeCtx {
        butts,
        below: &below,
    };
    let plans = edge_plans(walls, fp, s, top + s.plate_raise(), &ctx);
    let specs: Vec<EdgeRoofSpec> = plans.iter().map(|e| e.spec).collect();
    let eave = if s.detail.baseline_at_plate {
        default_eave_baseline(s, &plans, plate)
            .unwrap_or_else(|| plate_baseline(&specs, plate, s.detail.thickness))
    } else {
        plate
    };
    let n = fp.len();
    let half = (0..n)
        .map(|i| {
            walls_on_edge(walls, fp[i], fp[(i + 1) % n])
                .iter()
                .map(|&k| &walls[k])
                .max_by(|x, y| x.length().total_cmp(&y.length()))
                .map_or(0.0, |w| w.thickness * 0.5)
        })
        .collect();
    BaselineSource {
        floor: fi,
        fp: fp.to_vec(),
        specs,
        half,
        height: eave - floor.elevation,
    }
}

/// The roofs the walls would get from Build Roof, as sources for baseline
/// polylines: the build floor's (one per wing region) and the wings on the
/// floors below. Mirrors [`make_auto_planes`] and [`make_wing_planes`].
pub(crate) fn baseline_sources(
    project: &Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<Vec<BaselineSource>, String> {
    let floor = &project.floors[fi];
    let rooms = detect_rooms(&floor.walls, TOL);
    let pitched = rooms.is_empty()
        || rooms
            .iter()
            .any(|r| room_roof(floor, r) == RoomRoof::Pitched);
    let mut out = Vec::new();
    if pitched {
        match floor_levels(floor, &rooms) {
            Some(groups) => {
                let regions = level_regions(project, fi, &groups);
                let uppers = upper_footprints(project, fi, fi);
                let walls = curved_sections(roofing_walls(floor), s);
                for r in &regions {
                    let butts =
                        |a: Point, b: Point| edge_butts(project, r, &regions, &uppers, a, b);
                    out.push(region_source(project, fi, &walls, &r.fp, r.top, s, &butts));
                }
            }
            None => {
                let walls = curved_sections(exterior_walls(floor), s);
                let fp = footprint_from_walls(&walls, TOL)
                    .ok_or_else(|| "The exterior walls do not enclose an area".to_string())?;
                let top = walls.iter().map(|w| w.height).fold(0.0, f64::max);
                out.push(region_source(project, fi, &walls, &fp, top, s, &|_, _| {
                    false
                }));
            }
        }
    }
    for g in 0..fi {
        let regions = wing_regions(project, fi, g);
        if regions.is_empty() {
            continue;
        }
        let uppers = upper_footprints(project, g, fi);
        let walls = curved_sections(roofing_walls(&project.floors[g]), s);
        for r in &regions {
            let butts = |a: Point, b: Point| edge_butts(project, r, &regions, &uppers, a, b);
            out.push(region_source(project, g, &walls, &r.fp, r.top, s, &butts));
        }
    }
    if out.is_empty() {
        return Err("The exterior walls do not enclose an area".to_string());
    }
    Ok(out)
}

/// The wings of the floors below `fi` built from their Roof Baseline
/// Polylines (the baselines version of [`make_wing_planes`]).
fn baseline_wings(
    project: &Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<Vec<(usize, RegionRoof)>, String> {
    let mut out = Vec::new();
    for g in 0..fi {
        let (planes, approximate, faces) =
            crate::tools::roof_baseline::planes_on_floor(project, g, s)?;
        if planes.is_empty() && faces.is_empty() {
            continue;
        }
        out.push((
            g,
            RegionRoof {
                planes,
                approximate,
                faces,
            },
        ));
    }
    Ok(out)
}

/// Hash of what shapes the roof built over floor `fi`: its exterior walls,
/// the walls of the floors below (their uncovered rooms get wings) and, with
/// wings on one floor, its partitions.
pub fn project_signature(project: &Project, fi: usize) -> u64 {
    let mut h = wall_signature(&project.floors[fi]);
    let all_walls = |h: &mut u64, f: &Floor| {
        for w in &f.walls {
            for v in [
                w.start.x,
                w.start.y,
                w.end.x,
                w.end.y,
                w.thickness,
                w.height,
            ] {
                fnv(h, &v.to_bits().to_le_bytes());
            }
            fnv(h, format!("{:?}{:?}", w.kind, w.roof).as_bytes());
        }
        for n in &f.room_names {
            if n.roof_group != 0 {
                fnv(h, &n.anchor.x.to_bits().to_le_bytes());
                fnv(h, &n.anchor.y.to_bits().to_le_bytes());
                fnv(h, &n.roof_group.to_le_bytes());
            }
            if let Some(m) = n.misc.as_ref().filter(|m| !m.roof_over || m.flat_roof) {
                fnv(h, &n.anchor.x.to_bits().to_le_bytes());
                fnv(h, &n.anchor.y.to_bits().to_le_bytes());
                fnv(h, &[u8::from(m.roof_over), u8::from(m.flat_roof)]);
            }
        }
    };
    // Partitions only matter on a floor whose walls stand at several heights.
    let top = &project.floors[fi];
    let mut heights: Vec<i64> = top
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .map(|w| w.height.round() as i64)
        .collect();
    heights.sort_unstable();
    heights.dedup();
    if heights.len() > 1 || top.room_names.iter().any(|n| n.roof_group != 0) {
        all_walls(&mut h, top);
    }
    for g in 0..fi {
        let f = &project.floors[g];
        h = h.rotate_left(7);
        fnv(&mut h, &f.elevation.to_bits().to_le_bytes());
        all_walls(&mut h, f);
    }
    h
}

/// New automatic planes over floor `fi`, and whether plan-roof had to
/// approximate. Ids are allocated from `project`.
fn make_auto_planes(
    project: &mut Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<AutoRoof, String> {
    // Use Existing Roof Baselines (RF-71): the planes come from the Roof
    // Baseline Polylines instead of the walls.
    if s.switches.use_existing_baselines {
        let (planes, approximate, faces) =
            crate::tools::roof_baseline::baseline_planes(project, fi, s)?;
        return Ok(AutoRoof {
            planes,
            approximate,
            faces,
        });
    }
    let floor = &project.floors[fi];
    let rooms = detect_rooms(&floor.walls, TOL);
    // A floor whose every room is flat or roofless has no pitched roof.
    let pitched = rooms.is_empty()
        || rooms
            .iter()
            .any(|r| room_roof(floor, r) == RoomRoof::Pitched);
    let mut out = Vec::new();
    let mut approximate = false;
    let mut faces: Vec<Vec<[f64; 3]>> = Vec::new();
    if pitched {
        let built = match floor_levels(floor, &rooms) {
            // Wings at different plate heights each get their own roof.
            Some(groups) => {
                let regions = level_regions(project, fi, &groups);
                let mut all = RegionRoof::default();
                let uppers = upper_footprints(project, fi, fi);
                for r in &regions {
                    let butts =
                        |a: Point, b: Point| edge_butts(project, r, &regions, &uppers, a, b);
                    let walls = curved_sections(roofing_walls(floor), s);
                    let one = region_roof(project, fi, &walls, &r.fp, r.top, s, &butts)?;
                    all.add(one);
                }
                if regions.is_empty() {
                    return Err("The exterior walls do not enclose an area".to_string());
                }
                all
            }
            None => {
                let walls = curved_sections(exterior_walls(floor), s);
                let fp = footprint_from_walls(&walls, TOL)
                    .ok_or_else(|| "The exterior walls do not enclose an area".to_string())?;
                let top = walls.iter().map(|w| w.height).fold(0.0, f64::max);
                region_roof(project, fi, &walls, &fp, top, s, &|_, _| false)?
            }
        };
        approximate = built.approximate;
        faces = built.faces;
        out = built.planes;
    }
    // Rooms with Roof Over off get a hole in the plane over them; rooms with
    // a Flat Roof get a level plane at their ceiling.
    let mut holes: Vec<Vec<Point>> = Vec::new();
    for room in &rooms {
        match room_roof(floor, room) {
            RoomRoof::Pitched => {}
            RoomRoof::None => holes.push(if room.inner_polygon.len() >= 3 {
                room.inner_polygon.clone()
            } else {
                room.polygon.clone()
            }),
            RoomRoof::Flat => {
                let height = plan_3d::room_ceiling_top(floor, room);
                // The exterior edges of the room overhang like the pitched
                // roof's eaves; a partition against the pitched roof does not.
                let n = room.polygon.len();
                let mut ring = room.polygon.clone();
                let mut over: Vec<f64> = (0..n)
                    .map(|i| {
                        let (a, b) = (room.polygon[i], room.polygon[(i + 1) % n]);
                        let (mid, dir) = (Point::lerp(a, b, 0.5), b.sub(a).normalized());
                        floor
                            .walls
                            .iter()
                            .filter(|w| w.kind == WallKind::Exterior)
                            .find(|w| {
                                dist_to_segment(mid, w.start, w.end)
                                    <= w.thickness.max(1.0) * 0.5 + TOL
                                    && w.direction().cross(dir).abs() < 0.02
                            })
                            .map_or(0.0, |w| {
                                w.thickness * 0.5 + w.roof.overhang.unwrap_or(s.overhang)
                            })
                    })
                    .collect();
                // The baseline (first edge) is an overhanging one.
                let first = (0..n).max_by(|&x, &y| over[x].total_cmp(&over[y]).then(y.cmp(&x)));
                if let Some(k) = first.filter(|_| n > 0) {
                    ring.rotate_left(k);
                    over.rotate_left(k);
                }
                if let Some(pl) = flat_roof_plane_with_overhang(&ring, height, &over) {
                    let mut r = RoofPlaneRecord::new(0, pl.polygon3d, 0.0, pl.baseline);
                    r.auto = true;
                    r.material = s.material.clone();
                    r.overhang = s.overhang;
                    out.push(r);
                }
            }
        }
    }
    for outline in holes {
        if let Some(r) = out
            .iter_mut()
            .find(|r| r.source.is_some() && r.encloses(&outline))
        {
            r.holes.push(HoleRecord::hole(outline));
            continue;
        }
        // A roofless room that crosses a ridge, hip or valley: one hole piece
        // in every plane it reaches.
        let hit: Vec<usize> = (0..out.len())
            .filter(|&i| out[i].source.is_some())
            .collect();
        let roofs: Vec<RoofPlane> = hit.iter().map(|&i| out[i].to_roof_plane(0)).collect();
        for (k, piece) in plan_roof::hole_pieces(&roofs, &outline) {
            out[hit[k]].holes.push(HoleRecord::hole(piece));
        }
    }
    for r in &mut out {
        r.id = project.alloc_id();
    }
    Ok(AutoRoof {
        planes: out,
        approximate,
        faces,
    })
}

/// What Build Roof made of the walls: the planes, whether plan-roof had to
/// approximate, and the Dutch gable faces.
struct AutoRoof {
    planes: Vec<RoofPlaneRecord>,
    approximate: bool,
    faces: Vec<Vec<[f64; 3]>>,
}

/// The roof returns of walls with Auto Roof Return (RF-27): at each gable end
/// the planes of the two neighbouring edges wrap the corner with a full
/// return of the wall's Auto Roof Return length ([`AUTO_RETURN_LENGTH`] unless
/// the wall gives one). Returns `(source edge, return plane)`.
/// A return belongs to the roof it was made with: it is an automatic plane
/// without a `source` edge.
fn auto_returns(planes: &[RoofPlane], plans: &[EdgePlan]) -> Vec<(usize, RoofPlane)> {
    let n = plans.len();
    let mut out = Vec::new();
    for (i, plan) in plans.iter().enumerate() {
        let gable = plan.spec.gable || plan.spec.full_gable_wall;
        if !plan.auto_return || !gable {
            continue;
        }
        let spec = ReturnSpec {
            kind: ReturnKind::Full,
            length: plan.return_length,
        };
        // The plane before the gable edge ends at its corner, the plane
        // after it starts there.
        for (src, at_start) in [((i + n - 1) % n, false), ((i + 1) % n, true)] {
            for pl in planes.iter().filter(|p| p.source_edge == src) {
                if let Some(r) = roof_return_at(pl, 0, at_start, spec) {
                    out.push((src, r.plane));
                }
            }
        }
    }
    out
}

/// Copies holes, skylights and per-plane options from the old automatic plane
/// with the same baseline onto the rebuilt one. Returns `(old id, new id)` of
/// every plane that was matched.
fn carry_over(old: &[RoofPlaneRecord], new: &mut [RoofPlaneRecord]) -> Vec<(Id, Id)> {
    let mut map = Vec::new();
    for n in new.iter_mut().filter(|n| n.source.is_some()) {
        let nd = n.baseline.1.sub(n.baseline.0).normalized();
        let nm = Point::lerp(n.baseline.0, n.baseline.1, 0.5);
        let hit = old.iter().filter(|o| o.source.is_some()).find(|o| {
            let od = o.baseline.1.sub(o.baseline.0).normalized();
            let om = Point::lerp(o.baseline.0, o.baseline.1, 0.5);
            om.dist(nm) < 12.0 && od.dot(nd) > 0.99
        });
        if let Some(o) = hit {
            n.holes = o.holes.clone();
            n.label = o.label.clone();
            n.material = o.material.clone();
            n.layer = o.layer.clone();
            n.ridge_caps = o.ridge_caps;
            n.gutters = o.gutters;
            n.eave = o.eave;
            n.structure = o.structure;
            n.layers = o.layers.clone();
            map.push((o.id, n.id));
        }
    }
    map
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildReport {
    pub floor: usize,
    pub planes: usize,
    /// Vaulted ceiling planes made by Build Ceiling Planes (RF-46).
    pub ceilings: usize,
    pub approximate: bool,
}

/// Build Roof (RF-1..RF-8): replaces the automatic planes of floor `fi`
/// (manual planes stay, RF-6) and stores `settings` with the wall signature.
/// `clear_on_error`: when the walls enclose nothing, remove the old automatic
/// planes instead of failing (Auto Rebuild).
pub fn rebuild(
    project: &mut Project,
    fi: usize,
    mut settings: RoofSettings,
    clear_on_error: bool,
) -> Result<BuildReport, String> {
    let mut set = load(&project.floors[fi]);
    let old_auto: Vec<RoofPlaneRecord> = set.planes.iter().filter(|p| p.auto).cloned().collect();
    // Retain Manually Drawn / Edited Automatic Roof Planes (RF-68, RF-71).
    set.planes
        .retain(|p| crate::tools::roof_baseline::retained(p, &settings.switches));
    // Make Roof Baseline Polylines is a one-shot: it replaces the roof with
    // polylines and is not kept (Auto Rebuild must not remake them).
    let make_baselines = std::mem::take(&mut settings.switches.make_baselines);
    let mut approximate = false;
    let mut built = 0;
    if make_baselines {
        set.faces.clear();
        crate::tools::roof_baseline::make_from_walls(project, fi, &settings)?;
    } else if settings.build_planes {
        match make_auto_planes(project, fi, &settings) {
            Ok(AutoRoof {
                mut planes,
                approximate: approx,
                faces,
            }) => {
                set.faces = faces;
                // A new plane where a retained one stands is dropped.
                planes = crate::tools::roof_baseline::drop_replaced_records(&set.planes, planes);
                let moved = carry_over(&old_auto, &mut planes);
                // Dormers follow their (rebuilt) plane.
                for d in &mut set.dormers {
                    if let Some((_, new)) = moved.iter().find(|(old, _)| *old == d.main) {
                        d.main = *new;
                    }
                }
                approximate = approx;
                built = planes.len();
                set.planes.extend(planes);
            }
            Err(e) if !clear_on_error => return Err(e),
            Err(_) => {}
        }
    }
    // Build Ceiling Planes (RF-46) replaces the ceilings it made before.
    set.ceilings.retain(|c| !c.auto);
    let mut ceilings = 0;
    if settings.build_ceiling_planes {
        let planes: Vec<RoofPlaneRecord> = set
            .planes
            .iter()
            .filter(|p| !(p.auto && p.source.is_none()))
            .cloned()
            .collect();
        for plane in vaulted_ceilings(project, fi, &planes) {
            let id = project.alloc_id();
            set.ceilings
                .push(CeilingRecord::from_plane(id, &plane, true));
            ceilings += 1;
        }
    }
    settings.signature = project_signature(project, fi);
    set.settings = Some(settings.clone());
    store(project, fi, &mut set);
    // The stored Gable/Roof Lines of the build floor cut gables into the
    // planes just stored (RF-44).
    if settings.build_planes && !make_baselines {
        crate::tools::gable_line::apply_stored(project, fi);
    }
    // The wings on the floors below (RF-9); from the Roof Baseline Polylines of
    // those floors when the roof is built from baselines.
    let wings = if make_baselines || !settings.build_planes {
        Vec::new()
    } else if settings.switches.use_existing_baselines {
        baseline_wings(project, fi, &settings)?
    } else {
        make_wing_planes(project, fi, &settings)?
    };
    for g in 0..fi {
        let new = wings.iter().find(|(f, _)| *f == g).map(|(_, w)| w);
        let (n, c, approx) = rebuild_wing_floor(project, g, &settings, new);
        built += n;
        ceilings += c;
        approximate |= approx;
    }
    // An attic floor follows the walls the roof was just built over (R-68).
    project.refresh_attic_floor();
    // The automatic roof trim follows the roof just built.
    if !make_baselines {
        for g in 0..=fi {
            crate::tools::roof_trim::regenerate(project, g);
        }
    }
    Ok(BuildReport {
        floor: fi,
        planes: built,
        ceilings,
        approximate,
    })
}

/// Replaces the automatic planes of the lower floor `g` with `new` (its wings
/// under the roof built over a higher floor), keeping its manual planes.
/// Returns `(planes, ceiling planes, approximate)`.
fn rebuild_wing_floor(
    project: &mut Project,
    g: usize,
    settings: &RoofSettings,
    new: Option<&RegionRoof>,
) -> (usize, usize, bool) {
    let mut set = load(&project.floors[g]);
    let had_auto = set.planes.iter().any(|p| p.auto) || set.ceilings.iter().any(|c| c.auto);
    if new.is_none() && !had_auto && set.settings.is_none() && set.faces.is_empty() {
        return (0, 0, false);
    }
    let old_auto: Vec<RoofPlaneRecord> = set.planes.iter().filter(|p| p.auto).cloned().collect();
    set.planes
        .retain(|p| crate::tools::roof_baseline::retained(p, &settings.switches));
    set.ceilings.retain(|c| !c.auto);
    set.faces.clear();
    // A lower floor takes its roof from the build floor's settings.
    set.settings = None;
    let mut planes_made = 0;
    let mut approximate = false;
    if let Some(w) = new {
        let mut planes = w.planes.clone();
        for r in &mut planes {
            r.id = project.alloc_id();
        }
        planes = crate::tools::roof_baseline::drop_replaced_records(&set.planes, planes);
        let moved = carry_over(&old_auto, &mut planes);
        for d in &mut set.dormers {
            if let Some((_, to)) = moved.iter().find(|(old, _)| *old == d.main) {
                d.main = *to;
            }
        }
        planes_made = planes.len();
        approximate = w.approximate;
        set.faces = w.faces.clone();
        set.planes.extend(planes);
    }
    let mut ceilings = 0;
    if settings.build_ceiling_planes {
        let planes: Vec<RoofPlaneRecord> = set
            .planes
            .iter()
            .filter(|p| !(p.auto && p.source.is_none()))
            .cloned()
            .collect();
        for plane in vaulted_ceilings(project, g, &planes) {
            let id = project.alloc_id();
            set.ceilings
                .push(CeilingRecord::from_plane(id, &plane, true));
            ceilings += 1;
        }
    }
    store(project, g, &mut set);
    (planes_made, ceilings, approximate)
}

/// Build Ceiling Planes (RF-46): for every room of floor `fi` whose
/// "Ceiling Over This Room" is off, the ceiling planes that follow `planes`
/// (`plan_roof::ceiling_planes_for_vaulted_room`).
pub fn vaulted_ceilings(
    project: &Project,
    fi: usize,
    planes: &[RoofPlaneRecord],
) -> Vec<CeilingPlane> {
    let floor = &project.floors[fi];
    let roof_planes: Vec<RoofPlane> = planes
        .iter()
        .enumerate()
        .map(|(k, r)| r.to_roof_plane(k))
        .collect();
    let mut out = Vec::new();
    for room in detect_rooms(&floor.walls, 0.5) {
        let vaulted = room
            .name_entry(&floor.room_names)
            .is_some_and(|n| !n.has_ceiling);
        if !vaulted {
            continue;
        }
        let poly = if room.inner_polygon.len() >= 3 {
            &room.inner_polygon
        } else {
            &room.polygon
        };
        out.extend(ceiling_planes_for_vaulted_room(
            poly,
            &roof_planes,
            CEILING_THICKNESS,
        ));
    }
    out
}

/// Auto Rebuild Roofs (RF-6): rebuilds the automatic planes of every floor
/// whose roof is flagged auto and whose walls changed since the last build.
/// Returns whether anything was rebuilt. Call it after wall edits (the Roof
/// tool does on its events; the shell may call it once a frame).
pub fn auto_rebuild(cx: &mut EditorContext) -> bool {
    let mut changed = false;
    for fi in 0..cx.project.floors.len() {
        let set = load(&cx.project.floors[fi]);
        let Some(s) = set.settings.clone() else {
            continue;
        };
        if !s.auto_rebuild {
            continue;
        }
        let target = build_floor(&cx.project, s.ignore_top_floor, fi);
        if target == fi && project_signature(&cx.project, fi) == s.signature {
            continue;
        }
        if target != fi {
            // The roof moved to another floor: drop this floor's automatic
            // planes and settings, keep the manual ones.
            let mut old = set;
            old.planes.retain(|p| !p.auto);
            old.faces.clear();
            old.settings = None;
            store(&mut cx.project, fi, &mut old);
        }
        if rebuild(&mut cx.project, target, s, true).is_ok() {
            changed = true;
        }
    }
    if changed {
        cx.mark_dirty();
    }
    changed
}

/// Gives the roof settings of every floor `detail`, so the 3D roof follows
/// the Roof Defaults (the baseline rule only matters at the next Build Roof).
/// Returns how many floors had settings.
pub fn apply_detail(project: &mut Project, detail: &RoofDetailDefaults) -> usize {
    let mut n = 0;
    for fi in 0..project.floors.len() {
        let mut set = load(&project.floors[fi]);
        if let Some(s) = set.settings.as_mut() {
            s.detail = detail.clone();
            n += 1;
            store(project, fi, &mut set);
        }
    }
    n
}

/// Delete Roof Planes (RF-40): removes every plane and the roof settings of
/// floor `fi` without rebuilding. Returns how many planes went.
pub fn delete_all(project: &mut Project, fi: usize) -> usize {
    let mut set = load(&project.floors[fi]);
    let n = set.planes.len();
    set.planes.clear();
    // Dormers stand on planes: they go with them.
    set.dormers.clear();
    set.faces.clear();
    set.settings = None;
    store(project, fi, &mut set);
    n
}

/// Delete Ceiling Planes: removes every ceiling plane of floor `fi`. Returns
/// how many went.
pub fn delete_ceilings(project: &mut Project, fi: usize) -> usize {
    let mut set = load(&project.floors[fi]);
    let n = set.ceilings.len();
    if n > 0 {
        set.ceilings.clear();
        store(project, fi, &mut set);
    }
    n
}

/// Removes the records `ids` (planes, ceiling planes, dormers; the dormers of
/// a removed plane go with it) from floor `fi`. Returns how many records went.
pub fn delete_records(project: &mut Project, fi: usize, ids: &[Id]) -> usize {
    let mut set = load(&project.floors[fi]);
    let before = set.planes.len() + set.ceilings.len() + set.dormers.len();
    set.planes.retain(|p| !ids.contains(&p.id));
    set.ceilings.retain(|c| !ids.contains(&c.id));
    set.dormers
        .retain(|d| !ids.contains(&d.id) && !ids.contains(&d.main));
    let gone = before - (set.planes.len() + set.ceilings.len() + set.dormers.len());
    if gone > 0 {
        store(project, fi, &mut set);
    }
    gone
}

/// Translates record `id` by `d` in plan: a plane or ceiling plane moves, a
/// dormer slides on its plane (its position follows the eave and slope
/// directions). Returns whether the record exists.
pub fn translate_record(set: &mut RoofSet, id: Id, d: Point) -> bool {
    if let Some(r) = set.plane_mut(id) {
        r.translate(d);
        return true;
    }
    if let Some(c) = set.ceilings.iter_mut().find(|c| c.id == id) {
        c.translate(d);
        return true;
    }
    let Some(i) = set.dormers.iter().position(|x| x.id == id) else {
        return false;
    };
    if let Some(main) = set.plane(set.dormers[i].main) {
        let (a, b) = main.baseline;
        let along = b.sub(a).normalized();
        let up = main.up_slope();
        let spec = &mut set.dormers[i].spec;
        spec.position_along_eave += d.dot(along);
        spec.setback_from_eave = (spec.setback_from_eave + d.dot(up)).max(0.0);
    }
    true
}

/// Slides the dormer `id` by `delta` along its plane while the pointer is at
/// `pointer` (RF-51). A dormer that would no longer fit its plane stays where
/// it was; with the pointer over another plane it moves onto that plane,
/// centered under the pointer. Returns whether it moved.
pub fn slide_dormer(set: &mut RoofSet, id: Id, delta: Point, pointer: Point) -> bool {
    let Some(i) = set.dormers.iter().position(|d| d.id == id) else {
        return false;
    };
    let backup = set.dormers[i].clone();
    let over = set.plane_at(pointer);
    if let Some(other) = over.filter(|o| *o != backup.main) {
        // Onto another plane, keeping the size, centered at the pointer; a
        // dormer too deep for the plane there is brought closer to the eave.
        let mut spec = backup.spec;
        if let Some(p) = set.plane(other) {
            let (a, b) = p.baseline;
            spec.position_along_eave = pointer.sub(a).dot(b.sub(a).normalized());
            let wanted = pointer.sub(a).dot(p.up_slope()).max(12.0);
            for setback in [wanted, 36.0, 24.0, 12.0] {
                spec.setback_from_eave = setback.min(wanted.max(12.0));
                if auto_dormer(&p.to_roof_plane(0), spec).is_some() {
                    set.dormers[i].main = other;
                    set.dormers[i].spec = spec;
                    return true;
                }
            }
        }
        return false;
    }
    translate_record(set, id, delta);
    if dormer_geometry(set, &set.dormers[i].clone()).is_none() {
        set.dormers[i] = backup;
        return false;
    }
    true
}

// ===================================================================
// Manual planes (RF-35)
// ===================================================================

/// Baseline and `[x, elevation, -y]` vertices of a manual plane.
pub type ManualGeometry = ((Point, Point), Vec<[f64; 3]>);

/// The rectangle plane for a baseline `a -> b` and a click `toward` on the
/// side the plane rises to, at eave elevation `elev` and `pitch`. Vertex 0..1
/// is the baseline, counter-clockwise in plan. `None` for a click on the
/// baseline.
pub fn manual_plane_geometry(
    a: Point,
    b: Point,
    toward: Point,
    elev: f64,
    pitch: f64,
) -> Option<ManualGeometry> {
    let d = b.sub(a);
    if d.length() < 1e-6 {
        return None;
    }
    let mut n = d.normalized().perp();
    let mut depth = toward.sub(a).dot(n);
    let (mut a, mut b) = (a, b);
    if depth < 0.0 {
        std::mem::swap(&mut a, &mut b);
        n = n.scale(-1.0);
        depth = -depth;
    }
    if depth < 1.0 {
        return None;
    }
    let rise = depth * pitch / 12.0;
    let (c, e) = (b.add(n.scale(depth)), a.add(n.scale(depth)));
    let v = |p: Point, h: f64| [p.x, h, -p.y];
    Some((
        (a, b),
        vec![v(a, elev), v(b, elev), v(c, elev + rise), v(e, elev + rise)],
    ))
}

/// Axis-aligned rectangle through two corners, counter-clockwise.
pub fn rect_polygon(a: Point, b: Point) -> Vec<Point> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

/// Applies the dialog's edited copy `new` to `old` (RF-36). Pitch and
/// baseline-height edits make the plane manual (RF-37).
pub fn apply_edits(old: &mut RoofPlaneRecord, new: &RoofPlaneRecord) {
    let was_auto = old.auto;
    if (new.pitch - old.pitch).abs() > 1e-9 {
        old.set_pitch(new.pitch);
        old.auto = false;
    }
    if (new.baseline_height() - old.baseline_height()).abs() > 1e-9 {
        old.set_baseline_height(new.baseline_height());
        old.auto = false;
    }
    old.label = new.label.clone();
    old.material = new.material.clone();
    old.layer = new.layer.clone();
    old.ridge_caps = new.ridge_caps;
    old.gutters = new.gutters;
    old.eave = new.eave;
    old.structure = new.structure;
    old.layers = new.layers.clone();
    old.plate_top = new.plate_top;
    old.plate_width = new.plate_width;
    old.style = new.style.clone();
    old.in_schedule = new.in_schedule;
    old.special_snapping = new.special_snapping;
    // Mark as Edited: a plane built by Build Roof is kept through rebuilds
    // once marked; a built plane unmarked again is rebuilt (hand-drawn
    // planes have nothing to be rebuilt from and stay).
    if new.auto != was_auto && (was_auto || old.source.is_some()) {
        old.auto = new.auto;
    }
    // Curved Roof (RF-61); the curve follows the plane's (new) pitch.
    let curved = new.curved.map(|c| c.retarget(old.pitch));
    if curved != old.curved {
        old.curved = curved;
        old.auto = false;
    }
    // The dialog removes holes and edits skylight constructions.
    old.holes = new.holes.clone();
}

/// Applies the Roof Plane Specification's edited copy `new` to plane
/// `new.id` of floor `fi` (RF-36). The per-edge fields go to the roof
/// settings; when they changed the automatic roof is rebuilt. Returns whether
/// the plane exists.
pub fn apply_plane_edit(project: &mut Project, fi: usize, new: &RoofPlaneRecord) -> bool {
    let mut set = load(&project.floors[fi]);
    let Some(old) = set.plane_mut(new.id) else {
        return false;
    };
    let edge_changed = old.edge != new.edge;
    let source = old.source;
    apply_edits(old, new);
    if edge_changed {
        if let (Some(edge), Some(s)) = (source, set.settings.as_mut()) {
            s.set_override(edge, new.edge);
        }
    }
    store(project, fi, &mut set);
    if edge_changed {
        if let Some(s) = set.settings.clone() {
            // A failed rebuild (no closed walls) leaves the roof as it was.
            let _ = rebuild(project, fi, s, false);
        }
    }
    true
}

/// What Edit All Roof Planes changes on every plane at once (RF-39): each
/// field is `None` to leave the planes as they are. An eave choice is
/// `Some(None)` to hand it back to Roof Defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllPlanesEdit {
    /// Rise per 12.
    pub pitch: Option<f64>,
    /// Overhang from the wall face (automatic planes).
    pub overhang: Option<f64>,
    pub material: Option<String>,
    pub layer: Option<String>,
    pub ridge_caps: Option<bool>,
    pub eave_cut: Option<Option<plan_core::defaults::EaveCut>>,
    pub rafter_tails: Option<Option<bool>>,
    pub fascia: Option<Option<bool>>,
    pub soffit: Option<Option<bool>>,
    pub frieze: Option<Option<bool>>,
    pub gutters: Option<Option<bool>>,
    pub structure: Option<Option<RoofStructure>>,
}

impl AllPlanesEdit {
    /// Does it change nothing?
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Edit All Roof Planes (RF-39): applies `edit` to every roof plane of floor
/// `fi`. A pitch or overhang on an automatic plane goes to the edge it rises
/// from, and the roof is rebuilt so the hips and ridges follow; a manual
/// plane is changed in place. Returns how many planes changed.
pub fn apply_all(project: &mut Project, fi: usize, edit: &AllPlanesEdit) -> usize {
    let mut set = load(&project.floors[fi]);
    let mut settings = set.settings.take();
    let mut rebuild_needed = false;
    let mut changed = 0;
    for r in &mut set.planes {
        let before = r.clone();
        if let Some(m) = &edit.material {
            r.material = m.clone();
        }
        if let Some(l) = &edit.layer {
            r.layer = l.clone();
        }
        if let Some(v) = edit.ridge_caps {
            r.ridge_caps = v;
        }
        if let Some(v) = edit.eave_cut {
            r.eave.eave_cut = v;
        }
        if let Some(v) = edit.rafter_tails {
            r.eave.rafter_tails = v;
        }
        if let Some(v) = edit.fascia {
            r.eave.fascia = v;
        }
        if let Some(v) = edit.soffit {
            r.eave.soffit = v;
        }
        if let Some(v) = edit.frieze {
            r.eave.frieze = v;
        }
        if let Some(v) = edit.gutters {
            r.eave.gutters = v;
            r.gutters = v == Some(true);
        }
        if let Some(st) = edit.structure {
            r.set_structure(st);
        }
        let edge = r.source.filter(|_| r.auto);
        if let Some(p) = edit.pitch {
            match (edge, settings.as_mut()) {
                (Some(edge), Some(s)) => {
                    let mut o = s.override_of(edge).unwrap_or_default();
                    o.pitch = Some(p);
                    s.set_override(edge, o);
                    rebuild_needed = true;
                }
                _ => {
                    r.set_pitch(p);
                    r.auto = false;
                }
            }
        }
        if let (Some(v), Some(edge), Some(s)) = (edit.overhang, edge, settings.as_mut()) {
            let mut o = s.override_of(edge).unwrap_or_default();
            o.overhang = Some(v);
            s.set_override(edge, o);
            rebuild_needed = true;
        }
        if *r != before {
            changed += 1;
        }
    }
    if rebuild_needed {
        changed = changed.max(set.planes.iter().filter(|p| p.auto).count());
    }
    set.settings = settings.clone();
    store(project, fi, &mut set);
    if rebuild_needed {
        if let Some(s) = settings {
            // A failed rebuild (no closed walls) leaves the roof as it was.
            let _ = rebuild(project, fi, s, false);
        }
    }
    changed
}

// ===================================================================
// Roof features: holes, skylights, ceiling planes, dormers, gable lines,
// returns (RF-27, RF-42..RF-51)
// ===================================================================

/// Roof Hole / Skylight (RF-42, RF-43): the plan rectangle through `a` and
/// `b` becomes a hole (or skylight) of the plane under its center. Returns the
/// plane's id.
pub fn add_hole(
    project: &mut Project,
    fi: usize,
    a: Point,
    b: Point,
    skylight: bool,
) -> Result<Id, String> {
    add_hole_outline(project, fi, rect_polygon(a, b), skylight)
}

/// A polygon hole or skylight (RF-42): the outline may have any shape, as
/// long as it does not cross itself. Inside one plane it is a hole of that
/// plane; across a ridge, hip or valley it is cut into one piece per plane
/// (`plan_roof::hole_pieces`). Returns the id of the first plane that got it.
pub fn add_hole_polygon(
    project: &mut Project,
    fi: usize,
    outline: Vec<Point>,
    skylight: bool,
) -> Result<Id, String> {
    let name = if skylight { "skylight" } else { "hole" };
    if outline.len() < 3 {
        return Err(format!("A {name} needs at least three corners"));
    }
    if polygon_self_intersects(&outline) {
        return Err(format!("The {name} outline crosses itself"));
    }
    if polygon_centroid_area(&outline) < MIN_HOLE_AREA {
        return Err(format!("The {name} is too small"));
    }
    match add_hole_outline(project, fi, outline.clone(), skylight) {
        Ok(id) => Ok(id),
        Err(first) => {
            let mut set = load(&project.floors[fi]);
            let planes: Vec<RoofPlane> = set
                .planes
                .iter()
                .enumerate()
                .map(|(k, r)| r.to_roof_plane(k))
                .collect();
            let pieces = plan_roof::hole_pieces(&planes, &outline);
            // A hole straddling the plane boundary needs pieces in two
            // planes at least; one piece is the same refusal as before.
            if pieces.len() < 2 {
                return Err(first);
            }
            let mut first_id = None;
            for (k, piece) in pieces {
                let rec = &mut set.planes[k];
                rec.holes.push(if skylight {
                    HoleRecord::skylight(piece)
                } else {
                    HoleRecord::hole(piece)
                });
                first_id.get_or_insert(rec.id);
            }
            store(project, fi, &mut set);
            first_id.ok_or(first)
        }
    }
}

/// Smallest area of a hole polygon, square inches.
const MIN_HOLE_AREA: f64 = 36.0;

fn polygon_centroid_area(p: &[Point]) -> f64 {
    plan_core::geometry::polygon_area(p).abs()
}

/// Do two edges of the closed polygon `p` cross (not just meet at a corner)?
pub fn polygon_self_intersects(p: &[Point]) -> bool {
    let n = p.len();
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (p[j], p[(j + 1) % n]);
            let side = |o: Point, u: Point, v: Point| (u.sub(o)).cross(v.sub(o));
            let (d1, d2) = (side(a, b, c), side(a, b, d));
            let (d3, d4) = (side(c, d, a), side(c, d, b));
            if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
                return true;
            }
        }
    }
    false
}

fn add_hole_outline(
    project: &mut Project,
    fi: usize,
    outline: Vec<Point>,
    skylight: bool,
) -> Result<Id, String> {
    let name = if skylight { "skylight" } else { "hole" };
    let mut set = load(&project.floors[fi]);
    let center = polygon_centroid(&outline);
    let id = set
        .plane_at(center)
        .ok_or_else(|| format!("Draw the {name} inside a roof plane"))?;
    let plane = set.plane_mut(id).ok_or("The roof plane is gone")?;
    if !plane.encloses(&outline) {
        return Err(format!(
            "The {name} must lie completely inside one roof plane"
        ));
    }
    plane.holes.push(if skylight {
        HoleRecord::skylight(outline)
    } else {
        HoleRecord::hole(outline)
    });
    store(project, fi, &mut set);
    Ok(id)
}

/// A default-size skylight (24" x 48", long side along the slope) centered at
/// `at` on the plane under it.
pub fn place_skylight(project: &mut Project, fi: usize, at: Point) -> Result<Id, String> {
    let set = load(&project.floors[fi]);
    let id = set
        .plane_at(at)
        .ok_or("Click inside a roof plane".to_string())?;
    let up = set.plane(id).map_or(Point::new(0.0, 1.0), |p| p.up_slope());
    let outline = oriented_rect(at, SKYLIGHT_SIZE.0, SKYLIGHT_SIZE.1, up.perp(), up);
    add_hole_outline(project, fi, outline, true)
}

/// Ceiling Plane (RF-45): a rectangle on baseline `a -> b` rising toward
/// `toward`, `height` above the floor-0 datum at the baseline.
pub fn add_ceiling(
    project: &mut Project,
    fi: usize,
    (a, b, toward): (Point, Point, Point),
    height: f64,
    pitch: f64,
) -> Result<Id, String> {
    let (baseline, poly) = manual_plane_geometry(a, b, toward, height, pitch)
        .ok_or("Click away from the baseline, on the side the ceiling rises to".to_string())?;
    let outline: Vec<Point> = poly.iter().map(|v| Point::new(v[0], -v[2])).collect();
    let mut set = load(&project.floors[fi]);
    let id = project.alloc_id();
    set.ceilings.push(CeilingRecord {
        id,
        outline,
        baseline,
        pitch,
        height_at_baseline: height,
        thickness: CEILING_THICKNESS,
        layer: LAYER_CEILING.to_string(),
        line_style: LineStyle::Dashed,
        auto: false,
    });
    store(project, fi, &mut set);
    Ok(id)
}

/// Auto Dormer (RF-48): stores a dormer of `spec` on plane `main` of floor
/// `fi`, or, with `edit`, replaces the spec of that dormer (it keeps being
/// floating or not). Fails when the dormer does not fit the plane. Returns
/// the dormer's id.
pub fn apply_dormer(
    project: &mut Project,
    fi: usize,
    main: Id,
    edit: Option<Id>,
    spec: DormerSpec,
) -> Result<Id, String> {
    apply_dormer_as(project, fi, main, edit, spec, None)
}

/// Auto Floating Dormer (RF-49): like [`apply_dormer`], but the dormer is
/// floating: it does not cut a hole in the roof plane under it (and its
/// walls do not pierce it).
pub fn apply_floating_dormer(
    project: &mut Project,
    fi: usize,
    main: Id,
    edit: Option<Id>,
    spec: DormerSpec,
) -> Result<Id, String> {
    apply_dormer_as(project, fi, main, edit, spec, Some(true))
}

/// `floating`: `None` keeps the flag of the dormer being edited (a new one
/// is not floating).
fn apply_dormer_as(
    project: &mut Project,
    fi: usize,
    main: Id,
    edit: Option<Id>,
    spec: DormerSpec,
    floating: Option<bool>,
) -> Result<Id, String> {
    let mut set = load(&project.floors[fi]);
    let plane = set.plane(main).ok_or("The roof plane is gone")?;
    if auto_dormer(&plane.to_roof_plane(0), spec).is_none() {
        return Err("The dormer does not fit on that roof plane".into());
    }
    let existing = edit.filter(|id| set.dormer(*id).is_some());
    let id = match existing {
        Some(id) => {
            if let Some(d) = set.dormers.iter_mut().find(|d| d.id == id) {
                d.spec = spec;
                d.main = main;
                if let Some(f) = floating {
                    d.floating = f;
                }
            }
            id
        }
        None => {
            let id = project.alloc_id();
            set.dormers.push(DormerRecord {
                id,
                main,
                spec,
                layer: LAYER_PLANES.to_string(),
                floating: floating.unwrap_or(false),
            });
            id
        }
    };
    store(project, fi, &mut set);
    Ok(id)
}

/// The dormer spec a click at `at` on plane `main` starts from: centered at
/// the click along the eave and `at`'s distance up the slope as the setback.
pub fn dormer_spec_at(set: &RoofSet, main: Id, at: Point) -> DormerSpec {
    let mut spec = DormerSpec {
        overhang: DORMER_OVERHANG,
        ..DormerSpec::default()
    };
    if let Some(p) = set.plane(main) {
        let (a, b) = p.baseline;
        spec.position_along_eave = at.sub(a).dot(b.sub(a).normalized());
        spec.setback_from_eave = at.sub(a).dot(p.up_slope()).max(12.0);
    }
    spec
}

/// What Explode Dormer made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exploded {
    /// Plain roof plane records.
    pub planes: usize,
    /// Real walls on the floor the roof sits on.
    pub walls: usize,
}

/// Explode Dormer (RF-51): the dormer's roof planes become plain plane
/// records, its footprint a plain hole in the main plane, and its front and
/// cheek walls real walls of the roof's floor. A dormer wall starts at the
/// roof surface under it (`Wall::bottom_offset`, measured from that floor's
/// elevation) and rises the dormer's wall height; it takes the default
/// exterior wall type and thickness of `defaults`, with its outer face on the
/// dormer footprint. A window in the front wall becomes a window opening.
pub fn explode_dormer_record(
    project: &mut Project,
    fi: usize,
    id: Id,
    defaults: &PlanDefaults,
) -> Result<Exploded, String> {
    let mut set = load(&project.floors[fi]);
    let rec = set.dormer(id).cloned().ok_or("That is not a dormer")?;
    let geom = dormer_geometry(&set, &rec).ok_or("The dormer no longer fits its roof plane")?;
    let exploded = plan_roof::explode_dormer(&geom);
    let (material, layer) = set
        .plane(rec.main)
        .map(|p| (p.material.clone(), p.layer.clone()))
        .unwrap_or_else(|| (ROOF_MATERIALS[0].to_string(), LAYER_PLANES.to_string()));
    let mut planes = 0;
    for pl in exploded.roof_planes {
        let mut r = RoofPlaneRecord::new(
            project.alloc_id(),
            pl.polygon3d,
            pl.pitch_in_12,
            pl.baseline,
        );
        r.material = material.clone();
        r.layer = layer.clone();
        set.planes.push(r);
        planes += 1;
    }
    // A floating dormer never cut the roof under it.
    if let (Some(main), false) = (set.plane_mut(rec.main), rec.floating) {
        main.holes.push(HoleRecord::hole(exploded.hole.outline));
    }
    set.dormers.retain(|d| d.id != id);
    store(project, fi, &mut set);

    // The walls: the front wall first, then the cheek walls.
    let thickness = defaults.exterior_thickness();
    let type_name = defaults.exterior_wall.wall_type.clone();
    if let Some(def) = defaults.wall_type(&type_name) {
        if project.wall_type_def(&type_name).is_none() {
            project.register_wall_type(def.clone());
        }
    }
    let typed = defaults.wall_type(&type_name).is_some();
    let floor_elevation = project.floors[fi].elevation;
    let mut front_wall = None;
    let mut walls = 0;
    for (i, dw) in exploded.walls.iter().enumerate() {
        // The footprint is the outer face: the centerline sits half a wall
        // thickness inside it.
        let out = Point::new(dw.normal[0], -dw.normal[2]);
        let inward = out * (-thickness * 0.5);
        // The front wall of a gable dormer is the gable end: it stands to the
        // ridge and, as a Full Gable wall, rises to the underside of the
        // dormer roof planes, which gives the gable triangle above the wall.
        let gable_front = i == 0 && rec.spec.kind == plan_roof::DormerKind::Gable;
        let height = if gable_front {
            dw.height
        } else {
            dw.height.min(rec.spec.wall_height)
        }
        .max(1.0);
        let wid = project.add_wall(
            fi,
            dw.start + inward,
            dw.end + inward,
            thickness,
            height,
            WallKind::Exterior,
        );
        if let Some(w) = project.floors[fi].wall_mut(wid) {
            w.bottom_offset = dw.base_elevation - floor_elevation;
            if gable_front {
                w.roof.kind = RoofWallKind::FullGable;
            }
            if typed {
                w.wall_type = Some(type_name.clone());
            }
            if w.normal().dot(out) < 0.0 {
                w.exterior_side = plan_core::walls::Side::Right;
            }
        }
        if i == 0 {
            front_wall = Some(wid);
        }
        walls += 1;
    }
    if let (Some(wid), Some(win)) = (front_wall, exploded.window_opening) {
        let center = project.floors[fi]
            .wall(wid)
            .map_or(0.0, |w| w.length() * 0.5);
        if let Some(oid) = project.add_opening(fi, wid, center, plan_core::OpeningKind::Window) {
            let bottom = project.floors[fi]
                .wall(wid)
                .map_or(0.0, |w| w.bottom_offset);
            if let Some(o) = project.floors[fi].openings.iter_mut().find(|o| o.id == oid) {
                o.width = win.width;
                o.height = win.height;
                // Sills are measured from the floor, like every opening.
                o.sill_height = bottom + win.sill_height;
            }
        }
    }
    Ok(Exploded { planes, walls })
}

/// The roof plane edge nearest `p` within `tol`: `(plane id, edge index)`,
/// edge `i` running from vertex `i` to vertex `i + 1`. `only` restricts the
/// search to one plane.
pub fn edge_near(set: &RoofSet, p: Point, tol: f64, only: Option<Id>) -> Option<(Id, usize)> {
    let mut best: Option<(f64, Id, usize)> = None;
    for r in set
        .planes
        .iter()
        .filter(|r| only.is_none_or(|id| id == r.id))
    {
        let poly = r.plan_polygon();
        let n = poly.len();
        for i in 0..n {
            let d = dist_to_segment(p, poly[i], poly[(i + 1) % n]);
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, r.id, i));
            }
        }
    }
    best.map(|(_, id, i)| (id, i))
}

/// Join Roof Planes (RF-41): the edge `edge` of plane `a` is extended or
/// trimmed to the line where plane `a` meets plane `b`
/// (`plan_roof::join_planes`). The joined plane becomes a manual plane; its
/// holes that no longer fit are dropped.
pub fn join_planes_record(
    project: &mut Project,
    fi: usize,
    a: Id,
    edge: usize,
    b: Id,
) -> Result<(), String> {
    join_planes_record_locked(project, fi, a, edge, b, plan_roof::JoinLock::Radius)
}

/// [`join_planes_record`] with the choice of the Join Curved Roof Plane
/// dialog: a curved plane keeps its radius (the default) or its angle at the
/// ridge when its ridge edge moves to meet the other plane (RF-61).
pub fn join_planes_record_locked(
    project: &mut Project,
    fi: usize,
    a: Id,
    edge: usize,
    b: Id,
    lock: plan_roof::JoinLock,
) -> Result<(), String> {
    if a == b {
        return Err("Pick a different plane to join to".into());
    }
    let mut set = load(&project.floors[fi]);
    let pa = set.plane(a).ok_or("The first roof plane is gone")?;
    let pb = set.plane(b).ok_or("The second roof plane is gone")?;
    let joined = join_planes(&pa.to_roof_plane(0), edge, &pb.to_roof_plane(0))
        .ok_or("These planes cannot be joined along that edge (parallel planes?)")?;
    // A curved plane spans a new run now: the curve follows (RF-61).
    let curved = match pa.curved.filter(|c| !c.is_straight()) {
        Some(c) => {
            let old_run = plan_roof::plane_run(&pa.to_roof_plane(0));
            let run = plan_roof::plane_run(&joined);
            Some(
                c.after_join(old_run, run, run * pa.pitch / 12.0, lock)
                    .ok_or(
                        "The radius is too short for the joined edge: lock the angle at the ridge",
                    )?,
            )
        }
        None => pa.curved,
    };
    let rec = set.plane_mut(a).ok_or("The first roof plane is gone")?;
    rec.polygon3d = joined.polygon3d;
    rec.baseline = joined.baseline;
    rec.curved = curved;
    rec.auto = false;
    let kept: Vec<HoleRecord> = rec
        .holes
        .iter()
        .filter(|h| rec.encloses(&h.outline))
        .cloned()
        .collect();
    rec.holes = kept;
    store(project, fi, &mut set);
    Ok(())
}

/// Ceiling Plane Specification OK: height, pitch, thickness, line style and
/// layer of the ceiling plane `new.id` (its outline and baseline stay). The
/// plane becomes a manual one, so Build Ceiling Planes keeps it. Returns
/// whether the ceiling plane exists.
pub fn apply_ceiling_edit(project: &mut Project, fi: usize, new: &CeilingRecord) -> bool {
    let mut set = load(&project.floors[fi]);
    let Some(c) = set.ceilings.iter_mut().find(|c| c.id == new.id) else {
        return false;
    };
    c.height_at_baseline = new.height_at_baseline;
    c.pitch = new.pitch.max(0.0);
    c.thickness = new.thickness.max(0.0);
    c.line_style = new.line_style;
    c.layer = new.layer.clone();
    c.auto = false;
    store(project, fi, &mut set);
    true
}

/// The plane whose eave edge (polygon edge `0 -> 1`) passes within `tol` of
/// `p`.
pub fn eave_near(set: &RoofSet, p: Point, tol: f64) -> Option<Id> {
    set.planes
        .iter()
        .map(|r| (dist_to_segment(p, r.baseline.0, r.baseline.1), r.id))
        .filter(|(d, _)| *d <= tol)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// The eave corner within `tol` of `p`: the plane and whether it is the
/// start (vertex 0) of the eave.
pub fn eave_corner_near(set: &RoofSet, p: Point, tol: f64) -> Option<(Id, bool)> {
    let mut best: Option<(f64, Id, bool)> = None;
    for r in &set.planes {
        for (pt, at_start) in [(r.baseline.0, true), (r.baseline.1, false)] {
            let d = pt.dist(p);
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, r.id, at_start));
            }
        }
    }
    best.map(|(_, id, at_start)| (id, at_start))
}

/// Auto Roof Return (RF-27): a return of `spec` at the eave corner of plane
/// `id` (`at_start`: its first eave vertex). The return becomes a plane
/// record. Returns the new plane's id.
pub fn add_return(
    project: &mut Project,
    fi: usize,
    id: Id,
    at_start: bool,
    spec: ReturnSpec,
) -> Result<Id, String> {
    let mut set = load(&project.floors[fi]);
    let plane = set.plane(id).ok_or("The roof plane is gone")?;
    let ret = roof_return_at(&plane.to_roof_plane(0), 0, at_start, spec)
        .ok_or("A roof return does not fit at that corner")?;
    let mut r = RoofPlaneRecord::new(
        project.alloc_id(),
        ret.plane.polygon3d,
        ret.plane.pitch_in_12,
        ret.plane.baseline,
    );
    r.material = plane.material.clone();
    r.layer = plane.layer.clone();
    let new_id = r.id;
    set.planes.push(r);
    store(project, fi, &mut set);
    Ok(new_id)
}

/// Gable/Roof Line on an automatic plane (RF-44): the edge it rises from
/// becomes a gable end in the roof settings, and the roof is rebuilt. The plane
/// is gone afterwards.
pub fn set_edge_gable(project: &mut Project, fi: usize, id: Id) -> Result<(), String> {
    let set = load(&project.floors[fi]);
    let plane = set.plane(id).ok_or("The roof plane is gone")?;
    let edge = plane
        .source
        .filter(|_| plane.auto)
        .ok_or("That plane is not an automatic one")?;
    let mut settings = set
        .settings
        .clone()
        .ok_or("No roof has been built yet: use Build Roof")?;
    let mut over = settings.override_of(edge).unwrap_or_default();
    over.gable = true;
    settings.set_override(edge, over);
    rebuild(project, fi, settings, false).map(|_| ())
}

/// Gable/Roof Line on manual planes (RF-44): `plan_roof::apply_gable_line`
/// rebuilds the roof formed by the manual planes with the eave of plane `id`
/// as a gable end. The planes must form one closed ring or a chain with one
/// straight gap. Returns the number of manual planes afterwards.
pub fn gable_line_manual(project: &mut Project, fi: usize, id: Id) -> Result<usize, String> {
    let mut set = load(&project.floors[fi]);
    let manual: Vec<RoofPlaneRecord> = set.planes.iter().filter(|p| !p.auto).cloned().collect();
    let target = manual
        .iter()
        .position(|p| p.id == id)
        .ok_or("That plane is not a manual one")?;
    let roof = Roof {
        planes: manual
            .iter()
            .enumerate()
            .map(|(k, r)| r.to_roof_plane(k))
            .collect(),
        fascia_height: plan_roof::DEFAULT_FASCIA_HEIGHT,
        baseline_elevation: manual[target].baseline_height(),
        approximate: false,
    };
    let rebuilt = apply_gable_line(&roof, target).ok_or(
        "The manual roof planes do not form a closed roof: build the roof with Build Roof instead",
    )?;
    set.planes.retain(|p| p.auto);
    for np in rebuilt.planes {
        let Some(old) = manual.get(np.source_edge) else {
            continue;
        };
        let mut r = RoofPlaneRecord::new(old.id, np.polygon3d, np.pitch_in_12, np.baseline);
        r.material = old.material.clone();
        r.layer = old.layer.clone();
        r.label = old.label.clone();
        r.ridge_caps = old.ridge_caps;
        r.gutters = old.gutters;
        r.overhang = old.overhang;
        let holes = old
            .holes
            .iter()
            .filter(|h| r.encloses(&h.outline))
            .cloned()
            .collect();
        r.holes = holes;
        set.planes.push(r);
    }
    // Dormers of the plane that went are orphaned: drop them.
    let live: Vec<Id> = set.planes.iter().map(|p| p.id).collect();
    set.dormers.retain(|d| live.contains(&d.main));
    let n = set.planes.iter().filter(|p| !p.auto).count();
    store(project, fi, &mut set);
    Ok(n)
}

// ===================================================================
// Plan display (RF-58, RF-59)
// ===================================================================

fn layer_color(cx: &EditorContext, name: &str) -> Color32 {
    cx.layers()
        .get(name)
        .map_or(Color32::from_rgb(128, 0, 128), |l| {
            Color32::from_rgb(l.color[0], l.color[1], l.color[2])
        })
}

fn same_edge(a: (Point, Point), b: (Point, Point)) -> bool {
    let near = |p: Point, q: Point| p.dist(q) < 1.0;
    (near(a.0, b.0) && near(a.1, b.1)) || (near(a.0, b.1) && near(a.1, b.0))
}

/// A dashed closed outline.
fn dashed_outline(painter: &egui::Painter, pts: &[Pos2], stroke: Stroke) {
    if pts.len() < 2 {
        return;
    }
    let mut ring = pts.to_vec();
    ring.push(pts[0]);
    painter.extend(egui::Shape::dashed_line(&ring, stroke, 6.0, 4.0));
}

/// A closed outline in `style`.
fn styled_outline(painter: &egui::Painter, pts: &[Pos2], stroke: Stroke, style: LineStyle) {
    if pts.len() < 2 {
        return;
    }
    let mut ring = pts.to_vec();
    ring.push(pts[0]);
    match style {
        LineStyle::Solid => {
            painter.add(egui::Shape::line(ring, stroke));
        }
        LineStyle::Dashed => painter.extend(egui::Shape::dashed_line(&ring, stroke, 6.0, 4.0)),
        LineStyle::Dotted => painter.extend(egui::Shape::dashed_line(&ring, stroke, 2.0, 4.0)),
        LineStyle::DashDot => painter.extend(egui::Shape::dashed_line(&ring, stroke, 10.0, 6.0)),
    }
}

/// The outline of one roof plane polygon: eaves heavy, edges two planes share
/// medium, the rest light.
fn draw_plane_outline(
    painter: &egui::Painter,
    cam: &Camera,
    poly: &[Point],
    heights: &[f64],
    others: &[Vec<(Point, Point)>],
    color: Color32,
    hidden: &[(Point, Point)],
    style: &PlaneStyle,
) {
    let n = poly.len();
    draw_plane_fill(painter, cam, poly, &style.fill);
    let min_h = heights.iter().copied().fold(f64::INFINITY, f64::min);
    for i in 0..n {
        let j = (i + 1) % n;
        // Show All Ridges off hides the hips over a curved wall (RF-82).
        if hidden.iter().any(|e| same_edge(*e, (poly[i], poly[j]))) {
            continue;
        }
        let eave = (heights[i] - min_h).abs() < 0.5 && (heights[j] - min_h).abs() < 0.5;
        let shared = others
            .iter()
            .any(|edges| edges.iter().any(|e| same_edge(*e, (poly[i], poly[j]))));
        let width = if eave {
            2.5
        } else if shared {
            1.5
        } else {
            1.0
        };
        let stroke = Stroke::new((width * style.line_weight.clamp(0.25, 8.0)) as f32, color);
        let seg = [cam.world_to_screen(poly[i]), cam.world_to_screen(poly[j])];
        match style.dash {
            LineStyle::Solid => {
                painter.line_segment(seg, stroke);
            }
            LineStyle::Dashed => painter.extend(egui::Shape::dashed_line(&seg, stroke, 6.0, 4.0)),
            LineStyle::Dotted => painter.extend(egui::Shape::dashed_line(&seg, stroke, 2.0, 4.0)),
            LineStyle::DashDot => {
                painter.extend(egui::Shape::dashed_line(&seg, stroke, 10.0, 6.0))
            }
        }
    }
}

/// The Fill Style panel's fill: a solid wash or hatch lines inside the plane.
fn draw_plane_fill(painter: &egui::Painter, cam: &Camera, poly: &[Point], fill: &PlaneFill) {
    if poly.len() < 3 {
        return;
    }
    let [r, g, b] = fill.color;
    match fill.kind {
        FillKind::None => {}
        FillKind::Solid => {
            let pts: Vec<Pos2> = poly.iter().map(|p| cam.world_to_screen(*p)).collect();
            painter.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgba_unmultiplied(r, g, b, fill.opacity),
                Stroke::NONE,
            ));
        }
        FillKind::Hatch => {
            let stroke = Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(r, g, b, fill.opacity.max(60)),
            );
            for (a, c) in plane_extras::hatch_segments(poly, fill.spacing) {
                painter.line_segment([cam.world_to_screen(a), cam.world_to_screen(c)], stroke);
            }
        }
    }
}

fn poly_edges(p: &[Point]) -> Vec<(Point, Point)> {
    (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])).collect()
}

/// A slope arrow (pointing down the slope) and the label text at `c`.
fn draw_slope_label(
    painter: &egui::Painter,
    cam: &Camera,
    c: Point,
    up: Point,
    text: String,
    color: Color32,
    arrow: &SlopeArrow,
) {
    let at = cam.world_to_screen(c);
    if !cam.rect.expand(40.0).contains(at) {
        return;
    }
    let color = arrow
        .color
        .map_or(color, |[r, g, b]| Color32::from_rgb(r, g, b));
    if arrow.show {
        // Slope arrow: down the slope, 26 px long unless it has a length.
        let len = if arrow.length > 0.0 {
            arrow.length
        } else {
            26.0 / cam.px_per_in.max(1e-6)
        };
        let tail = cam.world_to_screen(c.add(up.scale(len * 0.5)));
        let tip = cam.world_to_screen(c.sub(up.scale(len * 0.5)));
        let stroke = Stroke::new(1.25_f32, color);
        painter.line_segment([tail, tip], stroke);
        let dir = (tip - tail).normalized();
        let side = egui::Vec2::new(-dir.y, dir.x);
        for s in [-1.0_f32, 1.0] {
            painter.line_segment([tip, tip - dir * 6.0 + side * (3.5 * s)], stroke);
        }
    }
    if !arrow.show_text {
        return;
    }
    painter.text(
        at + egui::Vec2::new(0.0, -14.0),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0),
        color,
    );
}

/// The roof planes of the current floor on layer "Roof Planes": outline with
/// eaves heavy and ridge/hip/valley lines (edges two planes share) solid,
/// holes (dashed), skylights (outline with diagonals), dormers (their planes
/// and the front wall), ceiling planes (layer "Ceiling Planes", dashed), and
/// at each centroid the pitch label with a slope arrow (pointing down the
/// slope).
pub fn draw_roofs(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    // Decking, level changes, fireplaces and chimney chases draw after the
    // walls (`render::draw_plan`).
    // Roof Baseline Polylines and their directive letters (RF-62).
    crate::tools::roof_baseline::draw_baselines(cx, painter, cam);
    let set = load(cx.floor());
    if set.planes.is_empty() && set.ceilings.is_empty() {
        return;
    }
    // Show All Ridges off: the hips between the sections of a curved wall's
    // roof are left out (RF-82).
    let hidden = match &set.settings {
        Some(s) if !s.switches.show_all_ridges => {
            crate::tools::roof_baseline::facet_hips(&set.planes, s.switches.segment_angle_clamped())
        }
        _ => Vec::new(),
    };
    let edges: Vec<Vec<(Point, Point)>> = set
        .planes
        .iter()
        .map(|r| poly_edges(&r.plan_polygon()))
        .collect();
    for (k, r) in set.planes.iter().enumerate() {
        if !cx.layers().is_visible(&r.layer) {
            continue;
        }
        let color = layer_color(cx, &r.layer);
        let poly = r.plan_polygon();
        let heights: Vec<f64> = r.polygon3d.iter().map(|v| v[1]).collect();
        let others: Vec<Vec<(Point, Point)>> = edges
            .iter()
            .enumerate()
            .filter(|(o, _)| *o != k)
            .map(|(_, e)| e.clone())
            .collect();
        let color = r
            .style
            .line_color
            .map_or(color, |[cr, cg, cb]| Color32::from_rgb(cr, cg, cb));
        draw_plane_outline(
            painter, cam, &poly, &heights, &others, color, &hidden, &r.style,
        );
        for h in &r.holes {
            let pts: Vec<Pos2> = h.outline.iter().map(|p| cam.world_to_screen(*p)).collect();
            if h.is_skylight() {
                painter.add(egui::Shape::closed_line(
                    pts.clone(),
                    Stroke::new(1.5_f32, color),
                ));
                if pts.len() >= 4 {
                    painter.line_segment([pts[0], pts[2]], Stroke::new(1.0_f32, color));
                    painter.line_segment([pts[1], pts[3]], Stroke::new(1.0_f32, color));
                }
            } else {
                dashed_outline(painter, &pts, Stroke::new(1.5_f32, color));
            }
        }
        let mut text = r.pitch_label();
        if !r.label.is_empty() {
            text = format!("{} {text}", r.label);
        }
        draw_slope_label(
            painter,
            cam,
            r.centroid(),
            r.up_slope(),
            text,
            color,
            &r.style.arrow,
        );
    }
    for c in &set.ceilings {
        if !cx.layers().is_visible(&c.layer) {
            continue;
        }
        let color = layer_color(cx, &c.layer);
        let pts: Vec<Pos2> = c.outline.iter().map(|p| cam.world_to_screen(*p)).collect();
        styled_outline(painter, &pts, Stroke::new(1.5_f32, color), c.line_style);
        let (a, b) = c.baseline;
        let up = b.sub(a).normalized().perp();
        draw_slope_label(
            painter,
            cam,
            polygon_centroid(&c.outline),
            up,
            format!("Ceiling {}", pitch_label(c.pitch)),
            color,
            &SlopeArrow::default(),
        );
    }
    for d in &set.dormers {
        if !cx.layers().is_visible(&d.layer) {
            continue;
        }
        let Some(g) = dormer_geometry(&set, d) else {
            continue;
        };
        let color = layer_color(cx, &d.layer);
        let planes: Vec<Vec<(Point, Point)>> = g
            .overhang_planes
            .iter()
            .map(|p| poly_edges(&p.plan_polygon()))
            .collect();
        for (k, pl) in g.overhang_planes.iter().enumerate() {
            let heights: Vec<f64> = pl.polygon3d.iter().map(|v| v[1]).collect();
            let others: Vec<Vec<(Point, Point)>> = planes
                .iter()
                .enumerate()
                .filter(|(o, _)| *o != k)
                .map(|(_, e)| e.clone())
                .collect();
            draw_plane_outline(
                painter,
                cam,
                &pl.plan_polygon(),
                &heights,
                &others,
                color,
                &[],
                &PlaneStyle::default(),
            );
        }
        let w = &g.front_wall;
        painter.line_segment(
            [cam.world_to_screen(w.start), cam.world_to_screen(w.end)],
            Stroke::new(2.0_f32, color),
        );
    }
}

// ===================================================================
// 3D meshes
// ===================================================================

/// Marks every mesh as belonging to record `id`.
fn tagged(mut meshes: Vec<plan_3d::Mesh>, id: Id) -> Vec<plan_3d::Mesh> {
    for m in &mut meshes {
        m.object_id = Some(id);
    }
    meshes
}

/// The meshes of one floor's roof: each plane as a slab with its holes cut
/// (skylight curb, frame and glass on top), the ceiling planes, and the
/// dormers (walls and roof planes; their footprint is cut from the main
/// plane). The 3D view builder appends these to the scene of
/// `plan_3d::build_scene`. Planes are not trimmed at walls here (see
/// [`roof_meshes`], which knows the other floors' walls).
pub fn floor_roof_meshes(floor: &Floor) -> Vec<plan_3d::Mesh> {
    floor_roof_meshes_in(floor, None, &[])
}

/// [`floor_roof_meshes`] with the planes cut where they butt a taller wall:
/// `cover` is the floor's part of `plan_3d::RoofCover`, whose planes are
/// already trimmed (a plane trimmed away entirely is left out). `chimneys`
/// are the plan outlines of the project's chimneys: each is cut through the
/// planes it crosses, a piece per plane (CB-87).
fn floor_roof_meshes_in(
    floor: &Floor,
    cover: Option<&plan_3d::FloorCover>,
    chimneys: &[Vec<Point>],
) -> Vec<plan_3d::Mesh> {
    let set = load(floor);
    let chimney_pieces: Vec<(usize, Vec<Point>)> = if chimneys.is_empty() {
        Vec::new()
    } else {
        let flat: Vec<RoofPlane> = set.planes.iter().map(|r| r.to_roof_plane(0)).collect();
        chimneys
            .iter()
            .flat_map(|o| plan_roof::hole_pieces(&flat, o))
            .collect()
    };
    let dormers: Vec<(&DormerRecord, Dormer)> = set
        .dormers
        .iter()
        .filter_map(|d| Some((d, dormer_geometry(&set, d)?)))
        .collect();
    let thickness = cover.map_or_else(
        || plan_3d::RoofDetail::default().thickness,
        |c| c.detail.thickness,
    );
    let mut out = Vec::new();
    for (k, r) in set.planes.iter().enumerate() {
        let mut plane = r.to_roof_plane(0);
        if let Some(c) = cover {
            // The cover holds the plane after butting walls trimmed it.
            match c.eaves.iter().find(|e| e.id == Some(r.id)) {
                Some(e) => plane.polygon3d = e.plane.polygon3d.clone(),
                None => continue,
            }
        }
        // A curved plane (RF-61) is drawn as the flat facets of its arc;
        // holes do not cut a curved plane.
        if let Some(curve) = r.curved.filter(|c| !c.is_straight()) {
            let own = r.eave.thickness.unwrap_or(thickness);
            for facet in plan_roof::curved_facets(&plane, &curve) {
                let poly = roof_plane_with_holes(&facet, &[]);
                out.extend(tagged(plan_3d::roof_plane_meshes(&poly, own), r.id));
            }
            continue;
        }
        let mut holes = r.roof_holes();
        holes.extend(
            dormers
                .iter()
                .filter(|(d, _)| d.main == r.id && !d.floating)
                .map(|(_, g)| g.hole_in_main_roof.clone()),
        );
        holes.extend(
            chimney_pieces
                .iter()
                .filter(|(i, _)| *i == k)
                .map(|(_, piece)| RoofHole::hole(piece.clone())),
        );
        let poly = roof_plane_with_holes(&plane, &holes);
        let own = r.eave.thickness.unwrap_or(thickness);
        out.extend(tagged(plan_3d::roof_plane_meshes(&poly, own), r.id));
    }
    // Ceiling planes that meet along an edge (the ridge of a vaulted ceiling)
    // are mitred.
    let ceiling_planes: Vec<CeilingPlane> =
        set.ceilings.iter().map(CeilingRecord::to_plane).collect();
    for (c, plane) in set.ceilings.iter().zip(&ceiling_planes) {
        out.extend(tagged(
            plan_3d::ceiling_plane_meshes_joined(plane, &ceiling_planes),
            c.id,
        ));
    }
    for (d, g) in &dormers {
        out.extend(tagged(
            plan_3d::dormer_meshes(g, DORMER_WALL_THICKNESS, SLAB_THICKNESS),
            d.id,
        ));
    }
    // The wall faces under Dutch gables.
    for face in &set.faces {
        out.extend(plan_3d::gable_face_meshes(face, DORMER_WALL_THICKNESS));
    }
    out
}

/// [`floor_roof_meshes`] of every floor, with each plane trimmed at the face
/// of any taller wall it butts (RF-13).
pub fn roof_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let cover = plan_3d::RoofCover::from_project(project);
    // The roof is cut around every chimney of the project, whichever floor
    // it stands on.
    let chimneys: Vec<Vec<Point>> = super::fireplace_view::chimney_holes(project)
        .into_iter()
        .map(|(_, outline)| outline)
        .collect();
    project
        .floors
        .iter()
        .enumerate()
        .flat_map(|(i, f)| floor_roof_meshes_in(f, cover.floor(i), &chimneys))
        .collect()
}

/// Eave detail of every roof plane (RF-14, RF-15): fascia and soffit along
/// the eaves, rake boards and soffit on gable ends, optional frieze and ridge
/// caps, and the flashing line where a lower roof butts a wall. Each mesh is
/// tagged with its roof plane's id. Kept apart from [`roof_meshes`] so the
/// plane slabs stay one mesh per plane.
pub fn roof_detail_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let cover = plan_3d::RoofCover::from_project(project);
    let mut out = cover.eave_meshes();
    // The dormers' own eaves and rakes (a dormer overhang has fascia and
    // soffit like any roof plane).
    for (i, floor) in project.floors.iter().enumerate() {
        let set = load(floor);
        let detail = cover.floor(i).map_or(cover.detail, |c| c.detail);
        for d in &set.dormers {
            if let Some(g) = dormer_geometry(&set, d) {
                out.extend(tagged(plan_3d::dormer_eave_meshes(&g, &detail), d.id));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults with the eave-tip baseline rule (the tip at plate height),
    /// which the older tests below measure against; the plate rule has its
    /// own tests.
    fn eave_tip_defaults() -> PlanDefaults {
        let mut d = plan_defaults::embedded();
        d.roof_detail.baseline_at_plate = false;
        d
    }
    use crate::plan_defaults;
    use plan_core::geometry::polygon_area;

    fn rect_project(w: f64, d: f64) -> Project {
        let mut p = Project::new("t");
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, d),
            Point::new(0.0, d),
        ];
        for i in 0..4 {
            p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        p
    }

    #[test]
    fn build_rectangle_gives_four_planes_at_wall_top() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        let s = RoofSettings::from_defaults(&d);
        let rep = rebuild(&mut p, 0, s, false).unwrap();
        assert_eq!(rep.planes, 4);
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4);
        assert!(set.planes.iter().all(|r| r.auto && r.pitch == 8.0));
        // Eave at wall top (floor 0 + 109").
        assert!((set.planes[0].baseline_height() - 109.0).abs() < 1e-6);
        // Overhang 16" past the wall face: 3" half thickness + 16".
        let xs: Vec<f64> = set
            .planes
            .iter()
            .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
            .collect();
        assert!((xs.iter().cloned().fold(f64::INFINITY, f64::min) + 19.0).abs() < 1e-6);
    }

    #[test]
    fn store_load_round_trip_through_json() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let mut set = load(&p.floors[0]);
        set.planes[0].holes.push(HoleRecord::hole(rect_polygon(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
        )));
        set.planes[0].holes.push(HoleRecord::skylight(rect_polygon(
            Point::new(20.0, 20.0),
            Point::new(44.0, 68.0),
        )));
        set.planes[1].label = "Main".into();
        store(&mut p, 0, &mut set);
        let text = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&text).unwrap();
        let again = load(&back.floors[0]);
        assert_eq!(again, load(&p.floors[0]));
        assert_eq!(again.planes.len(), 4);
        assert_eq!(again.planes[1].label, "Main");
        assert_eq!(again.planes[0].holes.len(), 2);
        assert!(again.planes[0].holes[1].is_skylight());
        // The roof lives in the typed slot; nothing is left in the CAD list.
        assert_eq!(
            back.floors[0].roofs.len(),
            4 + 1,
            "four planes and the settings"
        );
        assert!(back.floors[0].cad.is_empty());
        assert!(back.layers.get(LAYER_DATA).is_none());
        assert_eq!(
            back.floors[0].roofs_as::<Value>().unwrap().len(),
            back.floors[0].roofs.len()
        );
        // Storing twice does not duplicate anything.
        let mut p2 = back;
        let mut s2 = load(&p2.floors[0]);
        store(&mut p2, 0, &mut s2);
        assert_eq!(p2.floors[0].roofs.len(), 5);
        assert!(exists(&p2.floors[0], again.planes[2].id));
        assert!(!exists(&p2.floors[0], 9999));
    }

    /// A plane as the old storage wrote it (with its outline id).
    fn legacy_plane_json(r: &RoofPlaneRecord, outline_id: Id) -> Value {
        let mut v = r.to_json();
        let o = v.as_object_mut().unwrap();
        o.remove("kind");
        o.insert("outline_id".into(), json!(outline_id));
        v
    }

    /// The pre-typed-slot storage: `RFP1:` / `RFS1:` records on the hidden
    /// layer and an outline polyline per plane.
    fn write_legacy(project: &mut Project, fi: usize, set: &RoofSet) {
        let mut l = plan_core::Layer::new(LAYER_DATA, [128, 0, 128], 13);
        l.display = false;
        project.layers.add(l);
        for r in &set.planes {
            let outline = project.alloc_id();
            project.floors[fi].cad.push(CadObject {
                id: outline,
                layer: r.layer.clone(),
                item: CadItem::Polyline {
                    points: r.plan_polygon(),
                    closed: true,
                },
            });
            project.floors[fi].cad.push(CadObject {
                id: r.id,
                layer: LAYER_DATA.to_string(),
                item: CadItem::Text {
                    pos: Point::ZERO,
                    text: format!("{PLANE_TAG}{}", legacy_plane_json(r, outline)),
                    height: 1.0,
                    angle: 0.0,
                },
            });
        }
        if let Some(s) = &set.settings {
            let id = project.alloc_id();
            project.floors[fi].cad.push(CadObject {
                id,
                layer: LAYER_DATA.to_string(),
                item: CadItem::Text {
                    pos: Point::ZERO,
                    text: format!("{SETTINGS_TAG}{}", s.to_json()),
                    height: 1.0,
                    angle: 0.0,
                },
            });
        }
    }

    #[test]
    fn legacy_cad_records_migrate_into_the_roofs_slot() {
        let d = eave_tip_defaults();
        let mut built = rect_project(480.0, 288.0);
        rebuild(&mut built, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let mut set = load(&built.floors[0]);
        set.planes[1].label = "Kept".into();
        set.planes[0].holes.push(HoleRecord::skylight(rect_polygon(
            Point::new(20.0, 20.0),
            Point::new(44.0, 68.0),
        )));

        // An old file: the same walls, the roof as CAD text records.
        let mut old = rect_project(480.0, 288.0);
        old.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        write_legacy(&mut old, 0, &set);
        assert!(old.floors[0].roofs.is_empty());
        assert_eq!(old.floors[0].cad.len(), 1 + 4 * 2 + 1);
        // Through a JSON round trip, as when the file is opened.
        let mut loaded = Project::from_json(&old.to_json().unwrap()).unwrap();
        assert!(site_view_migrate(&mut loaded));

        let got = load(&loaded.floors[0]);
        assert_eq!(got, set);
        assert_eq!(loaded.floors[0].roofs.len(), 5);
        // Only the unrelated CAD line is left, and the data layer is gone.
        assert_eq!(loaded.floors[0].cad.len(), 1);
        assert!(loaded.layers.get(LAYER_DATA).is_none());
        assert!(loaded.layers.get(LAYER_PLANES).is_some());
        // Planes keep their ids, so a selection by id still resolves.
        assert!(exists(&loaded.floors[0], set.planes[3].id));
        // Migrating again is a no-op.
        assert!(!site_view_migrate(&mut loaded));
        assert_eq!(load(&loaded.floors[0]), set);
    }

    fn site_view_migrate(p: &mut Project) -> bool {
        crate::editor::site_view::migrate_legacy_storage(p)
    }

    #[test]
    fn the_load_migration_moves_legacy_lights_into_the_typed_slot() {
        use plan_core::camera::LIGHTS_LAYER;
        let mut p = Project::new("old lights");
        let record = |id: Id, text: String| CadObject {
            id,
            layer: LIGHTS_LAYER.to_string(),
            item: CadItem::Text {
                pos: Point::ZERO,
                text,
                height: 1.0,
                angle: 0.0,
            },
        };
        p.floors[0].cad.push(record(
            41,
            r#"plan-light:{"name":"Hall","position":{"x":30.0,"y":40.0},"height":80.0,"intensity":2.0,"color":[255,255,255],"enabled":true,"cast_shadows":false}"#.into(),
        ));
        p.floors[0].cad.push(record(
            42,
            r#"plan-lightset:{"use_electrical":false}"#.into(),
        ));
        assert!(p.lights.is_empty());
        // The same chain EditorContext runs on load.
        assert!(crate::editor::site_view::migrate_legacy_storage(&mut p));
        assert_eq!(p.lights().len(), 1);
        let l = p.light(41).expect("the id carries over");
        assert_eq!(
            (l.floor, l.name.as_str(), l.cast_shadows),
            (0, "Hall", false)
        );
        assert!(!p.light_settings().use_electrical);
        assert!(p.floors[0].cad.is_empty());
        // Saved and loaded again it stays put and migrates nothing more.
        let mut back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.lights().len(), 1);
        assert!(!crate::editor::site_view::migrate_legacy_storage(&mut back));
    }

    #[test]
    fn migration_leaves_a_filled_roofs_slot_alone() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let typed = load(&p.floors[0]);
        let mut other = typed.clone();
        other.planes.truncate(1);
        write_legacy(&mut p, 0, &other);
        assert!(!migrate_legacy(&mut p));
        assert_eq!(load(&p.floors[0]), typed);
    }

    #[test]
    fn manual_geometry_rises_at_pitch_and_is_ccw() {
        let (base, poly) = manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(60.0, 60.0),
            100.0,
            8.0,
        )
        .unwrap();
        assert_eq!(base.0, Point::new(0.0, 0.0));
        assert_eq!(poly[2], [120.0, 140.0, -60.0]);
        // Clicking below the baseline flips the baseline direction.
        let (base, poly) = manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(60.0, -60.0),
            100.0,
            8.0,
        )
        .unwrap();
        assert_eq!(base.0, Point::new(120.0, 0.0));
        let plan: Vec<Point> = poly.iter().map(|v| Point::new(v[0], -v[2])).collect();
        assert!(polygon_area(&plan) > 0.0);
        assert!(manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(60.0, 0.2),
            0.0,
            8.0
        )
        .is_none());
    }

    #[test]
    fn vertex_move_keeps_plane_planar() {
        let (base, poly) = manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(60.0, 60.0),
            100.0,
            12.0,
        )
        .unwrap();
        let mut r = RoofPlaneRecord::new(1, poly, 12.0, base);
        r.auto = true;
        r.move_vertex(2, Point::new(120.0, 90.0));
        assert!(!r.auto);
        assert!((r.polygon3d[2][1] - 190.0).abs() < 1e-9);
        // Moving a baseline vertex keeps the eave height.
        r.move_vertex(0, Point::new(-10.0, 0.0));
        assert_eq!(r.polygon3d[0][1], 100.0);
        assert_eq!(r.baseline.0, Point::new(-10.0, 0.0));
    }

    fn square_plane() -> RoofPlaneRecord {
        let (base, poly) = manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            Point::new(60.0, 60.0),
            100.0,
            12.0,
        )
        .unwrap();
        let mut r = RoofPlaneRecord::new(1, poly, 12.0, base);
        r.auto = true;
        r
    }

    fn assert_planar(r: &RoofPlaneRecord) {
        let plane = r.to_roof_plane(0);
        let n = plane.normal();
        for v in &r.polygon3d {
            let d = [
                v[0] - r.polygon3d[0][0],
                v[1] - r.polygon3d[0][1],
                v[2] - r.polygon3d[0][2],
            ];
            assert!((d[0] * n[0] + d[1] * n[1] + d[2] * n[2]).abs() < 1e-6);
        }
        assert!(n[1] > 0.0);
    }

    #[test]
    fn a_plane_has_vertex_edge_pitch_and_rotate_handles() {
        let r = square_plane();
        let hs = r.handles();
        let count = |f: fn(&PlaneHandle) -> bool| hs.iter().filter(|(h, _)| f(h)).count();
        assert_eq!(count(|h| matches!(h, PlaneHandle::Vertex(_))), 4);
        assert_eq!(count(|h| matches!(h, PlaneHandle::Edge(_))), 4);
        assert_eq!(count(|h| matches!(h, PlaneHandle::Pitch)), 1);
        assert_eq!(count(|h| matches!(h, PlaneHandle::Rotate)), 1);
        let at = |h: PlaneHandle| hs.iter().find(|(k, _)| *k == h).unwrap().1;
        // Edge handles are the edge middles, the pitch arrow is up the slope
        // from the centre, the rotate knob is below the eave.
        assert_eq!(at(PlaneHandle::Edge(0)), Point::new(60.0, 0.0));
        let up = r.up_slope();
        assert!(
            (at(PlaneHandle::Pitch).sub(r.centroid()).dot(up) - PITCH_ARROW_REACH).abs() < 1e-9
        );
        assert!(at(PlaneHandle::Rotate).y < 0.0 && (at(PlaneHandle::Rotate).x - 60.0).abs() < 1e-9);
        // The nearest handle wins; corners beat edges.
        assert_eq!(
            r.handle_at(Point::new(1.0, 1.0), 6.0).map(|h| h.0),
            Some(PlaneHandle::Vertex(0))
        );
        assert_eq!(
            r.handle_at(Point::new(61.0, 1.0), 6.0).map(|h| h.0),
            Some(PlaneHandle::Edge(0))
        );
        assert_eq!(r.handle_at(Point::new(60.0, 30.0), 4.0), None);
        // Short edges get none.
        let mut thin = r.clone();
        thin.move_vertex(1, Point::new(5.0, 0.0));
        assert!(
            thin.handles()
                .iter()
                .filter(|(h, _)| matches!(h, PlaneHandle::Edge(_)))
                .count()
                < 4
        );
    }

    #[test]
    fn dragging_the_pitch_arrow_changes_the_pitch_in_quarters() {
        let mut r = square_plane();
        assert_eq!(r.pitch, 12.0);
        r.drag_pitch(8.0);
        assert!(!r.auto);
        assert_eq!(r.pitch, 14.0);
        // The heights follow: the far edge is 60" of run at 14:12.
        assert!((r.polygon3d[2][1] - (100.0 + 60.0 * 14.0 / 12.0)).abs() < 1e-9);
        assert_planar(&r);
        r.drag_pitch(-1.1);
        assert_eq!(r.pitch, 13.75);
        r.drag_pitch(-1000.0);
        assert_eq!(r.pitch, 0.25);
        r.drag_pitch(1000.0);
        assert_eq!(r.pitch, 24.0);
        assert_eq!(r.baseline_height(), 100.0, "the eave stays");
    }

    #[test]
    fn rotating_a_plane_turns_it_about_its_centre_and_keeps_its_pitch() {
        let mut r = square_plane();
        r.holes.push(HoleRecord::hole(rect_polygon(
            Point::new(50.0, 20.0),
            Point::new(70.0, 40.0),
        )));
        let c = r.centroid();
        let area = polygon_area(&r.plan_polygon()).abs();
        r.rotate(std::f64::consts::FRAC_PI_2, c);
        assert!(!r.auto);
        // Same shape and size, still about the same centre.
        assert!((polygon_area(&r.plan_polygon()).abs() - area).abs() < 1e-6);
        assert!(r.centroid().dist(c) < 1e-6);
        // The baseline was along +x; a quarter turn puts it along +y.
        let d = r.baseline.1.sub(r.baseline.0);
        assert!(d.x.abs() < 1e-9 && d.y > 100.0, "{d:?}");
        // The plane rises the other way now and keeps 12:12 and its eave.
        assert_planar(&r);
        assert_eq!(r.baseline_height(), 100.0);
        let slope = r.up_slope();
        assert!(slope.x.abs() > 0.99, "{slope:?}");
        assert!((r.to_roof_plane(0).pitch_in_12 - 12.0).abs() < 1e-9);
        // The hole turned with it and is still inside.
        assert!(r.encloses(&r.holes[0].outline));
        // Four quarter turns bring everything back.
        let orig = square_plane();
        for _ in 0..3 {
            r.rotate(std::f64::consts::FRAC_PI_2, c);
        }
        for (a, b) in r.plan_polygon().iter().zip(orig.plan_polygon()) {
            assert!(a.dist(b) < 1e-6);
        }
    }

    #[test]
    fn moving_an_edge_keeps_the_plane_planar_and_its_pitch() {
        let mut r = square_plane();
        // The top edge (vertex 2 to 3) 20" further out (to the right of
        // its direction, which is up the screen for a plane rising in +y).
        let top = r.plan_polygon();
        let edge = (0..4)
            .find(|&i| (top[i].y - 60.0).abs() < 1e-9 && (top[(i + 1) % 4].y - 60.0).abs() < 1e-9)
            .unwrap();
        r.move_edge(edge, 20.0);
        let moved = r.plan_polygon();
        assert!(
            (moved[edge].y - 80.0).abs() < 1e-9 && (moved[(edge + 1) % 4].y - 80.0).abs() < 1e-9
        );
        assert!(!r.auto);
        assert_planar(&r);
        assert!(
            (r.polygon3d[edge][1] - (100.0 + 80.0)).abs() < 1e-9,
            "12:12 over 80\""
        );
        // The eave edge moves the eave line (its elevation stays).
        let mut e = square_plane();
        e.move_edge(0, -10.0);
        assert_eq!(e.baseline.0, Point::new(0.0, 10.0));
        assert_eq!(e.baseline_height(), 100.0);
        assert_planar(&e);
        // Out of range does nothing.
        let before = e.clone();
        e.move_edge(9, 5.0);
        assert_eq!(e, before);
    }

    #[test]
    fn meshes_cover_each_plane() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let meshes = roof_meshes(&p);
        assert_eq!(meshes.len(), 4);
        for m in &meshes {
            assert_eq!(m.material, plan_3d::Material::Roof);
            // A triangle: top + bottom + three side quads.
            assert!(m.triangle_count() >= 2 + 3 * 2);
            assert!(m.object_id.is_some());
        }
        // The top faces up.
        let m = &meshes[0];
        assert!(m.vertices[0].normal[1] > 0.0);
        assert!(m.vertices[3].normal[1] < 0.0);
    }

    #[test]
    fn a_built_roof_gets_eave_detail_tagged_with_its_planes() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let ids: Vec<Id> = load(&p.floors[0]).planes.iter().map(|r| r.id).collect();
        let detail = roof_detail_meshes(&p);
        assert!(!detail.is_empty());
        assert!(detail
            .iter()
            .all(|m| m.object_id.is_some_and(|id| ids.contains(&id))));
        // Fascia and soffit are trim; the slabs stay one mesh per plane.
        assert!(detail.iter().any(|m| m.material == plan_3d::Material::Trim));
        assert_eq!(roof_meshes(&p).len(), 4);
    }

    #[test]
    fn a_gable_end_wall_rises_to_the_roof_in_the_scene() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        let wall = p.floors[0].walls[1].id;
        toggle_gable(&mut p, 0, wall).unwrap();
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let scene = plan_3d::build_scene(&p);
        let top = |id: Id| {
            scene
                .meshes
                .iter()
                .filter(|m| m.object_id == Some(id))
                .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
                .fold(f32::MIN, f32::max)
        };
        let eave_wall = p.floors[0].walls[0].id;
        assert!(
            (top(eave_wall) - 109.0).abs() < 1e-3,
            "eave wall stays at the plate"
        );
        assert!(
            top(wall) > 109.0 + 60.0,
            "gable wall reaches the ridge: {}",
            top(wall)
        );
    }

    #[test]
    fn delete_all_clears_planes_and_settings() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        assert_eq!(delete_all(&mut p, 0), 4);
        let set = load(&p.floors[0]);
        assert!(set.planes.is_empty() && set.settings.is_none());
        assert!(p.floors[0].cad.is_empty());
    }

    #[test]
    fn build_floor_picks_top_or_next() {
        let mut p = rect_project(480.0, 288.0);
        assert_eq!(build_floor(&p, false, 0), 0);
        p.floors.push(Floor::new("2nd", 109.0));
        assert_eq!(build_floor(&p, false, 0), 0);
        p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        assert_eq!(build_floor(&p, false, 0), 1);
        assert_eq!(build_floor(&p, true, 0), 0);
    }

    // ----- Join, ceilings, directives -----

    #[test]
    fn join_planes_record_extends_a_plane_to_the_ridge() {
        let mut p = rect_project(480.0, 360.0);
        let (ba, poly_a) = manual_plane_geometry(
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(240.0, 100.0),
            100.0,
            8.0,
        )
        .unwrap();
        let (bb, poly_b) = manual_plane_geometry(
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
            Point::new(240.0, 260.0),
            100.0,
            8.0,
        )
        .unwrap();
        let (ia, ib) = (p.alloc_id(), p.alloc_id());
        let mut set = RoofSet::default();
        set.planes.push(RoofPlaneRecord::new(ia, poly_a, 8.0, ba));
        set.planes
            .push(RoofPlaneRecord::new(ib, poly_b.clone(), 8.0, bb));
        store(&mut p, 0, &mut set);
        // The pick finds the top edge of the first plane.
        let set = load(&p.floors[0]);
        assert_eq!(
            edge_near(&set, Point::new(240.0, 101.0), 4.0, None),
            Some((ia, 2))
        );
        assert_eq!(edge_near(&set, Point::new(240.0, 50.0), 4.0, None), None);
        join_planes_record(&mut p, 0, ia, 2, ib).unwrap();
        let set = load(&p.floors[0]);
        let a = set.plane(ia).unwrap();
        for i in [2, 3] {
            assert!((a.plan_polygon()[i].y - 180.0).abs() < 1e-6);
            assert!((a.polygon3d[i][1] - 220.0).abs() < 1e-6);
        }
        assert!(!a.auto);
        // The second plane is untouched; bad joins are refused.
        assert_eq!(set.plane(ib).unwrap().polygon3d, poly_b);
        assert!(join_planes_record(&mut p, 0, ia, 2, ia).is_err());
        assert!(join_planes_record(&mut p, 0, ia, 2, 999).is_err());
        assert!(join_planes_record(&mut p, 0, ia, 9, ib).is_err());
    }

    fn vaulted_house() -> Project {
        let mut p = rect_project(480.0, 288.0);
        let mut name = plan_core::RoomName::new(Point::new(240.0, 144.0), "Great Room", "Living");
        name.has_ceiling = false;
        p.floors[0].room_names.push(name);
        p
    }

    #[test]
    fn build_ceiling_planes_follows_the_roof_for_rooms_without_a_ceiling() {
        let d = eave_tip_defaults();
        let mut s = RoofSettings::from_defaults(&d);
        s.build_ceiling_planes = true;
        let mut p = vaulted_house();
        let rep = rebuild(&mut p, 0, s.clone(), false).unwrap();
        assert_eq!(rep.planes, 4);
        assert_eq!(rep.ceilings, 4);
        let set = load(&p.floors[0]);
        assert_eq!(set.ceilings.len(), 4);
        assert!(set
            .ceilings
            .iter()
            .all(|c| c.auto && c.layer == LAYER_CEILING));
        // Each follows its roof plane's pitch and covers the room together.
        assert!(set.ceilings.iter().all(|c| (c.pitch - 8.0).abs() < 1e-9));
        let area: f64 = set
            .ceilings
            .iter()
            .map(|c| polygon_area(&c.outline).abs())
            .sum();
        let room = detect_rooms(&p.floors[0].walls, 0.5)[0]
            .inner_polygon
            .clone();
        assert!((area - polygon_area(&room).abs()).abs() < 1.0, "{area}");
        // Planes that meet along the hips are mitred: every slab is meshed
        // with finite corners and its structure above the underside.
        let meshes = floor_roof_meshes(&p.floors[0]);
        for c in &set.ceilings {
            let mine: Vec<&plan_3d::Mesh> = meshes
                .iter()
                .filter(|m| m.object_id == Some(c.id))
                .collect();
            assert!(mine
                .iter()
                .any(|m| m.material == plan_3d::CEILING_FRAMING_MATERIAL));
            assert!(mine
                .iter()
                .flat_map(|m| m.vertices.iter())
                .all(|v| v.position.iter().all(|x| x.is_finite())));
        }
        // The ceilings sit under the roof surface.
        let c = &set.ceilings[0];
        let mid = polygon_centroid(&c.outline);
        let roof_h = set
            .planes
            .iter()
            .filter_map(|r| r.to_roof_plane(0).height_at(mid))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(c.to_plane().height_at(mid) < roof_h);
        // A rebuild replaces them instead of piling up; manual ones stay.
        let manual = add_ceiling(
            &mut p,
            0,
            (
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(50.0, 50.0),
            ),
            100.0,
            4.0,
        )
        .unwrap();
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        assert_eq!(load(&p.floors[0]).ceilings.len(), 5);
        // Unchecking removes only the automatic ones.
        s.build_ceiling_planes = false;
        let rep = rebuild(&mut p, 0, s, false).unwrap();
        assert_eq!(rep.ceilings, 0);
        let set = load(&p.floors[0]);
        assert_eq!(set.ceilings.len(), 1);
        assert_eq!(set.ceilings[0].id, manual);
        // The settings and the ceiling record round-trip through the file.
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(load(&back.floors[0]), set);
    }

    #[test]
    fn rooms_with_a_ceiling_get_no_ceiling_planes() {
        let d = eave_tip_defaults();
        let mut s = RoofSettings::from_defaults(&d);
        s.build_ceiling_planes = true;
        let mut p = rect_project(480.0, 288.0);
        let rep = rebuild(&mut p, 0, s, false).unwrap();
        assert_eq!(rep.ceilings, 0);
        assert!(load(&p.floors[0]).ceilings.is_empty());
    }

    #[test]
    fn ceiling_specification_edits_height_pitch_thickness_and_style() {
        let mut p = rect_project(480.0, 288.0);
        let id = add_ceiling(
            &mut p,
            0,
            (
                Point::new(0.0, 0.0),
                Point::new(240.0, 0.0),
                Point::new(120.0, 90.0),
            ),
            100.0,
            4.0,
        )
        .unwrap();
        let mut c = load(&p.floors[0]).ceilings[0].clone();
        c.height_at_baseline = 120.0;
        c.pitch = 6.0;
        c.thickness = 5.5;
        c.line_style = LineStyle::Dotted;
        c.layer = "Walls, Normal".into();
        c.outline.clear(); // the outline is not editable here
        assert!(apply_ceiling_edit(&mut p, 0, &c));
        let got = load(&p.floors[0]).ceilings[0].clone();
        assert_eq!(got.id, id);
        assert_eq!(got.outline.len(), 4);
        assert_eq!(
            (
                got.height_at_baseline,
                got.pitch,
                got.thickness,
                got.line_style
            ),
            (120.0, 6.0, 5.5, LineStyle::Dotted)
        );
        assert_eq!(got.layer, "Walls, Normal");
        assert!(!got.auto);
        let mut gone = got;
        gone.id = 9999;
        assert!(!apply_ceiling_edit(&mut p, 0, &gone));
    }

    #[test]
    fn extend_slope_downward_walls_lower_their_plane() {
        let d = eave_tip_defaults();
        let s = RoofSettings::from_defaults(&d);
        let mut p = rect_project(480.0, 288.0);
        let lowest = |p: &Project| {
            load(&p.floors[0])
                .planes
                .iter()
                .flat_map(|r| r.polygon3d.iter().map(|v| v[1]))
                .fold(f64::INFINITY, f64::min)
        };
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        assert!((lowest(&p) - 109.0).abs() < 1e-6);
        p.floors[0].walls[0].roof.kind = RoofWallKind::ExtendSlopeDownward;
        rebuild(&mut p, 0, s, false).unwrap();
        assert!(
            (lowest(&p) - (109.0 - EXTEND_SLOPE_DROP)).abs() < 1e-6,
            "{}",
            lowest(&p)
        );
        // Only the plane of that wall went down, still 8:12.
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4);
        assert!(set.planes.iter().all(|r| (r.pitch - 8.0).abs() < 1e-9));
    }

    #[test]
    fn auto_roof_return_wraps_the_corners_of_a_gable_end() {
        let d = eave_tip_defaults();
        let s = RoofSettings::from_defaults(&d);
        let mut p = rect_project(480.0, 288.0);
        // East and west walls are gable ends; only the east one returns.
        for i in [1, 3] {
            p.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
        }
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        let plain = load(&p.floors[0]);
        assert_eq!(plain.planes.len(), 2);
        let max_x = |set: &RoofSet| {
            set.planes
                .iter()
                .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
                .fold(f64::NEG_INFINITY, f64::max)
        };
        p.floors[0].walls[1].roof.auto_roof_return = true;
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4, "two planes and a return at each eave");
        let returns: Vec<&RoofPlaneRecord> =
            set.planes.iter().filter(|r| r.source.is_none()).collect();
        assert_eq!(returns.len(), 2);
        assert!(returns.iter().all(|r| r.auto && r.pitch > 0.0));
        // The returns reach past the old east edge, by their length.
        assert!((max_x(&set) - max_x(&plain) - AUTO_RETURN_LENGTH).abs() < 1e-6);
        // Rebuilding again does not pile returns up.
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        assert_eq!(load(&p.floors[0]).planes.len(), 4);
        // Without the flag they go away again.
        p.floors[0].walls[1].roof.auto_roof_return = false;
        rebuild(&mut p, 0, s, false).unwrap();
        assert_eq!(load(&p.floors[0]).planes.len(), 2);
    }

    /// Highest roof vertex of floor 0, inches.
    fn roof_peak(p: &Project) -> f64 {
        load(&p.floors[0])
            .planes
            .iter()
            .flat_map(|r| r.polygon3d.iter().map(|v| v[1]))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn gable_house() -> Project {
        let mut p = rect_project(480.0, 288.0);
        // Walls 1 and 3 (east and west) are the gable ends.
        for i in [1, 3] {
            p.floors[0].walls[i].roof.kind = RoofWallKind::FullGable;
        }
        p
    }

    #[test]
    fn upper_pitch_directives_make_a_gambrel() {
        let d = eave_tip_defaults();
        let mut s = RoofSettings::from_defaults(&d);
        s.overhang = 0.0;
        let mut p = gable_house();
        for i in [0, 2] {
            let w = &mut p.floors[0].walls[i];
            w.roof.overhang = Some(0.0);
            w.roof.pitch_in_12 = Some(6.0);
        }
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        let plain = roof_peak(&p);
        assert_eq!(load(&p.floors[0]).planes.len(), 2);
        // Steeper above 36" over the eave (the wall is 109" high).
        for i in [0, 2] {
            p.floors[0].walls[i].roof.upper_pitch = Some((24.0, 109.0 + 36.0));
        }
        rebuild(&mut p, 0, s, false).unwrap();
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4, "a lower and an upper plane per side");
        assert!(roof_peak(&p) > plain + 50.0, "{} vs {plain}", roof_peak(&p));
        assert_eq!(set.planes.iter().filter(|r| r.pitch == 24.0).count(), 2);
    }

    #[test]
    fn dutch_gable_walls_cut_the_end_hips_short() {
        let d = eave_tip_defaults();
        let mut s = RoofSettings::from_defaults(&d);
        s.overhang = 0.0;
        let mut p = rect_project(480.0, 288.0);
        for w in &mut p.floors[0].walls {
            w.roof.overhang = Some(0.0);
        }
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        let hip_peak = roof_peak(&p);
        let ridge_x = |p: &Project| {
            let hi = roof_peak(p);
            let xs: Vec<f64> = load(&p.floors[0])
                .planes
                .iter()
                .flat_map(|r| r.polygon3d.iter())
                .filter(|v| (v[1] - hi).abs() < 1e-6)
                .map(|v| v[0])
                .collect();
            xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min)
        };
        let hip_ridge = ridge_x(&p);
        for i in [1, 3] {
            p.floors[0].walls[i].roof.kind = RoofWallKind::DutchGable;
        }
        rebuild(&mut p, 0, s, false).unwrap();
        assert!((roof_peak(&p) - hip_peak).abs() < 1e-6);
        assert!(
            ridge_x(&p) > hip_ridge + 100.0,
            "{} vs {hip_ridge}",
            ridge_x(&p)
        );
        // The vertical face under each short gable is stored and meshed.
        let set = load(&p.floors[0]);
        assert_eq!(set.faces.len(), 2);
        let face_tris = |p: &Project| -> usize {
            floor_roof_meshes(&p.floors[0])
                .iter()
                .filter(|m| m.object_id.is_none() && m.material == plan_3d::Material::Stucco)
                .map(plan_3d::Mesh::triangle_count)
                .sum()
        };
        assert!(face_tris(&p) > 0, "the faces are in the roof meshes");
        // It survives a save, and goes with a rebuild without the directive.
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(load(&back.floors[0]).faces, set.faces);
        for i in [1, 3] {
            p.floors[0].walls[i].roof.kind = RoofWallKind::Hip;
        }
        let s = RoofSettings::from_defaults(&d);
        rebuild(&mut p, 0, s, false).unwrap();
        assert!(load(&p.floors[0]).faces.is_empty());
        assert_eq!(face_tris(&p), 0);
        delete_all(&mut p, 0);
        assert!(load(&p.floors[0]).faces.is_empty());
    }

    #[test]
    fn a_walls_return_length_and_extend_drop_are_used() {
        let d = eave_tip_defaults();
        let s = RoofSettings::from_defaults(&d);
        let mut p = gable_house();
        p.floors[0].walls[1].roof.auto_roof_return = true;
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        let max_x = |p: &Project| {
            load(&p.floors[0])
                .planes
                .iter()
                .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let short = max_x(&p);
        p.floors[0].walls[1].roof.return_length = Some(48.0);
        rebuild(&mut p, 0, s.clone(), false).unwrap();
        assert!((max_x(&p) - short - (48.0 - AUTO_RETURN_LENGTH)).abs() < 1e-6);

        let mut p = rect_project(480.0, 288.0);
        p.floors[0].walls[0].roof.kind = RoofWallKind::ExtendSlopeDownward;
        p.floors[0].walls[0].roof.extend_drop = Some(40.0);
        rebuild(&mut p, 0, s, false).unwrap();
        let lowest = load(&p.floors[0])
            .planes
            .iter()
            .flat_map(|r| r.polygon3d.iter().map(|v| v[1]))
            .fold(f64::INFINITY, f64::min);
        assert!((lowest - (109.0 - 40.0)).abs() < 1e-6, "{lowest}");
    }

    #[test]
    fn knee_walls_make_no_roof_plane_of_their_own() {
        let mut p = rect_project(480.0, 288.0);
        // A knee wall crossing the house is not part of the outline anyway;
        // a wall that would be part of it is.
        let id = p.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(480.0, 100.0),
            6.0,
            48.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(id).unwrap().roof.kind = RoofWallKind::KneeWall;
        assert_eq!(exterior_walls(&p.floors[0]).len(), 4);
        // Without it the other walls would not close: keep every wall.
        p.floors[0].walls[0].roof.kind = RoofWallKind::KneeWall;
        assert_eq!(exterior_walls(&p.floors[0]).len(), 5);
    }

    // ----- roof detail, Roof Over / Flat Roof rooms (RF-14, RF-15, R-30) -----

    fn deck_misc(roof_over: bool, flat_roof: bool) -> plan_core::extras::RoomMisc {
        plan_core::extras::RoomMisc {
            roof_over,
            flat_roof,
            ..plan_core::extras::RoomMisc::default()
        }
    }

    /// A 480 x 288 house whose east 120" is a separate room behind a
    /// partition; the exterior walls are split at the partition like the
    /// editor splits them.
    fn house_with_east_room(east: plan_core::extras::RoomMisc) -> Project {
        let mut p = Project::new("t");
        let seg = |p: &mut Project, a: (f64, f64), b: (f64, f64), kind| {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                109.0,
                kind,
            );
        };
        use WallKind::{Exterior, Interior};
        seg(&mut p, (0.0, 0.0), (360.0, 0.0), Exterior);
        seg(&mut p, (360.0, 0.0), (480.0, 0.0), Exterior);
        seg(&mut p, (480.0, 0.0), (480.0, 288.0), Exterior);
        seg(&mut p, (480.0, 288.0), (360.0, 288.0), Exterior);
        seg(&mut p, (360.0, 288.0), (0.0, 288.0), Exterior);
        seg(&mut p, (0.0, 288.0), (0.0, 0.0), Exterior);
        seg(&mut p, (360.0, 0.0), (360.0, 288.0), Interior);
        let mut main = plan_core::RoomName::new(Point::new(180.0, 144.0), "Living", "Living");
        main.misc = None;
        let mut deck = plan_core::RoomName::new(Point::new(420.0, 144.0), "Deck", "Deck");
        deck.misc = Some(east);
        p.floors[0].room_names.push(main);
        p.floors[0].room_names.push(deck);
        p
    }

    fn plan_min_y(p: &Project) -> f64 {
        load(&p.floors[0])
            .planes
            .iter()
            .flat_map(|r| r.polygon3d.iter().map(|v| -v[2]))
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn extend_existing_roof_over_brings_the_main_roof_down_over_a_bay() {
        use plan_core::openings::bay::BayUnit;
        use plan_core::OpeningStyle;
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        let wall = p.floors[0].walls[0].id;
        let id = p
            .add_opening(0, wall, 240.0, plan_core::OpeningKind::Window)
            .unwrap();
        {
            let o = p.floors[0].openings.iter_mut().find(|o| o.id == id).unwrap();
            o.style = OpeningStyle::BayWindow;
            o.width = 50.0;
            o.extras.spec.bay = BayUnit::for_style(OpeningStyle::BayWindow);
        }
        let mut plain = p.clone();
        rebuild(&mut plain, 0, RoofSettings::from_defaults(&d), false).unwrap();
        // The unit keeps its own hip: the main roof is the rectangle.
        let flat = plan_min_y(&plain);
        p.floors[0].openings[0].extras.spec.bay.roof.extend_existing = true;
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        assert!(
            plan_min_y(&p) < flat - 6.0,
            "the roof follows the unit out: {} against {flat}",
            plan_min_y(&p)
        );
        // A lowered ceiling spoils it: the unit builds its own lower hip.
        let mut low = p.clone();
        low.floors[0].openings[0].extras.spec.bay.lowered_ceiling = Some(Default::default());
        rebuild(&mut low, 0, RoofSettings::from_defaults(&d), false).unwrap();
        assert!((plan_min_y(&low) - flat).abs() < 1.0);
    }

    fn plan_x_range(p: &Project) -> (f64, f64) {
        let xs: Vec<f64> = load(&p.floors[0])
            .planes
            .iter()
            .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
            .collect();
        (
            xs.iter().copied().fold(f64::INFINITY, f64::min),
            xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    #[test]
    fn build_roof_skips_a_room_with_roof_over_off() {
        let d = eave_tip_defaults();
        // With the room roofed the roof covers the whole 480".
        let mut roofed = house_with_east_room(deck_misc(true, false));
        rebuild(&mut roofed, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let (_, east) = plan_x_range(&roofed);
        assert!(east > 480.0, "roof reaches {east}");
        // Roof Over off: the roof stops at the partition (plus its overhang).
        let mut open = house_with_east_room(deck_misc(false, false));
        rebuild(&mut open, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let (west, east) = plan_x_range(&open);
        assert!(east < 400.0, "roof stops near the partition, not at {east}");
        assert!(west < 0.0, "the roofed side keeps its overhang");
        assert_eq!(
            room_roof(
                &open.floors[0],
                &plan_core::detect_rooms(&open.floors[0].walls, 0.5)[1]
            ),
            RoomRoof::None
        );
        // Changing the flag changes the wall signature (Auto Rebuild reruns).
        assert_ne!(
            wall_signature(&roofed.floors[0]),
            wall_signature(&open.floors[0])
        );
    }

    #[test]
    fn a_roofless_room_inside_one_plane_gets_a_hole() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        let (a, b) = (Point::new(210.0, 20.0), Point::new(270.0, 60.0));
        let c = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 4.5, 109.0, WallKind::Interior);
        }
        let mut well = plan_core::RoomName::new(Point::new(240.0, 40.0), "Light well", "Courtyard");
        well.misc = Some(deck_misc(false, false));
        p.floors[0].room_names.push(well);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let holes: usize = load(&p.floors[0])
            .planes
            .iter()
            .map(|r| r.holes.len())
            .sum();
        assert_eq!(holes, 1, "one hole where the room is");
    }

    /// Is the point `p` (plan) under a roof triangle facing up?
    fn roof_covers(p: &Project, at: Point) -> bool {
        roof_meshes(p).iter().any(|m| {
            m.indices.chunks(3).any(|t| {
                let v = |i: u32| m.vertices[i as usize];
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                let plan = |q: plan_3d::Vertex| {
                    Point::new(f64::from(q.position[0]), -f64::from(q.position[2]))
                };
                a.normal[1] > 0.5 && point_in_polygon(at, &[plan(a), plan(b), plan(c)])
            })
        })
    }

    #[test]
    fn a_roofless_room_across_the_ridge_gets_a_hole_in_each_plane() {
        let d = eave_tip_defaults();
        let mut p = rect_project(480.0, 288.0);
        // 60 x 90 inches, straddling the ridge at y = 144.
        let (a, b) = (Point::new(210.0, 100.0), Point::new(270.0, 190.0));
        let c = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
        for i in 0..4 {
            p.add_wall(0, c[i], c[(i + 1) % 4], 4.5, 109.0, WallKind::Interior);
        }
        let mut well =
            plan_core::RoomName::new(Point::new(240.0, 145.0), "Light well", "Courtyard");
        well.misc = Some(deck_misc(false, false));
        p.floors[0].room_names.push(well);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let set = load(&p.floors[0]);
        let with_holes = set.planes.iter().filter(|r| !r.holes.is_empty()).count();
        assert_eq!(with_holes, 2, "the south and the north plane");
        // The roof is open over both halves of the room and closed beside it.
        assert!(!roof_covers(&p, Point::new(240.0, 120.0)));
        assert!(!roof_covers(&p, Point::new(240.0, 170.0)));
        assert!(roof_covers(&p, Point::new(240.0, 60.0)));
        assert!(roof_covers(&p, Point::new(240.0, 230.0)));
    }

    #[test]
    fn a_flat_roof_overhangs_its_exterior_edges_but_not_the_partition() {
        let d = eave_tip_defaults();
        let mut p = house_with_east_room(deck_misc(true, true));
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let set = load(&p.floors[0]);
        let flat = set.planes.iter().find(|r| r.pitch == 0.0).unwrap();
        let pts = flat.plan_polygon();
        let range = |f: fn(&Point) -> f64| {
            (
                pts.iter().map(f).fold(f64::MAX, f64::min),
                pts.iter().map(f).fold(f64::MIN, f64::max),
            )
        };
        // 3" half wall + 16" overhang past the centerlines on the three
        // exterior sides; the partition at x = 360 stays put.
        let (x0, x1) = range(|p| p.x);
        let (y0, y1) = range(|p| p.y);
        assert!((x0 - 360.0).abs() < 1e-6, "{x0}");
        assert!((x1 - 499.0).abs() < 1e-6, "{x1}");
        assert!((y0 + 19.0).abs() < 1e-6, "{y0}");
        assert!((y1 - 307.0).abs() < 1e-6, "{y1}");
        // The eave (baseline) is an overhanging edge.
        let (a, b) = flat.baseline;
        assert!(a.x > 360.0 + 1.0 || b.x > 360.0 + 1.0 || (a.y + 19.0).abs() < 1e-6);
        assert_eq!(flat.overhang, d.exterior_wall.roof.overhang);
    }

    #[test]
    fn a_flat_roof_room_gets_a_level_plane_at_its_ceiling() {
        let d = eave_tip_defaults();
        let mut p = house_with_east_room(deck_misc(true, true));
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let set = load(&p.floors[0]);
        let flat: Vec<&RoofPlaneRecord> = set.planes.iter().filter(|r| r.pitch == 0.0).collect();
        assert_eq!(flat.len(), 1, "one flat plane");
        let rooms = plan_core::detect_rooms(&p.floors[0].walls, 0.5);
        let east = rooms
            .iter()
            .find(|r| {
                r.name_entry(&p.floors[0].room_names)
                    .is_some_and(|n| n.name == "Deck")
            })
            .unwrap();
        let top = plan_3d::room_ceiling_top(&p.floors[0], east);
        assert!(flat[0].polygon3d.iter().all(|v| (v[1] - top).abs() < 1e-9));
        assert!(flat[0].auto && flat[0].source.is_none());
        // The rest of the house keeps its pitched roof, stopping at the partition.
        assert_eq!(set.planes.len(), 5, "four pitched planes and the flat one");
        let (_, east_x) = plan_x_range(&p);
        assert!(
            east_x >= 480.0 - 1e-6,
            "the flat plane covers the room: {east_x}"
        );
        // The flat plane is level in 3D too: its meshes are one slab.
        assert!(!roof_meshes(&p).is_empty());
    }

    #[test]
    fn the_baseline_at_the_plate_seats_the_roof_on_the_walls() {
        let mut p = rect_project(480.0, 288.0);
        let d = plan_defaults::embedded();
        assert!(
            d.roof_detail.baseline_at_plate,
            "Daniel's template turns it on"
        );
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let set = load(&p.floors[0]);
        let south = set
            .planes
            .iter()
            .find(|r| r.baseline.0.y == r.baseline.1.y && r.baseline.0.y < 0.0)
            .unwrap();
        let plane = south.to_roof_plane(0);
        let under = plane
            .underside_at(Point::new(240.0, 0.0), d.roof_detail.thickness)
            .unwrap();
        assert!(
            (under - 109.0).abs() < 1e-6,
            "underside at the wall: {under}"
        );
        // And the eave tip hangs below the plate.
        assert!(south.baseline_height() < 109.0);
        // A project built before the rule keeps its baseline.
        let old = serde_json::json!({"kind": "settings", "pitch": 8.0, "overhang": 16.0});
        let s = RoofSettings::from_json(&old, &RoofSettings::from_defaults(&d));
        assert!(!s.detail.baseline_at_plate);
    }

    #[test]
    fn the_roof_detail_and_a_planes_eave_choices_round_trip_and_reach_3d() {
        use plan_core::defaults::EaveCut;
        let mut d = plan_defaults::embedded();
        d.roof_detail.eave_cut = EaveCut::Square;
        d.roof_detail.rafter_tails = true;
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let mut set = load(&p.floors[0]);
        assert_eq!(
            set.settings.as_ref().unwrap().detail.eave_cut,
            EaveCut::Square
        );
        set.planes[0].eave.eave_cut = Some(EaveCut::Level);
        set.planes[0].eave.gutters = Some(true);
        store(&mut p, 0, &mut set);
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        let again = load(&back.floors[0]);
        assert_eq!(again.planes[0].eave.eave_cut, Some(EaveCut::Level));
        assert_eq!(again.planes[1].eave, plan_3d::EaveOverrides::default());
        assert!(again.settings.unwrap().detail.rafter_tails);
        let cover = plan_3d::RoofCover::from_project(&back);
        let first = &cover.floor(0).unwrap().eaves[0];
        assert_eq!(first.opts.eave_cut, Some(EaveCut::Level));
        assert!(cover.floor(0).unwrap().detail.rafter_tails);
        assert!(!roof_detail_meshes(&back).is_empty());
        // The Roof Plane Specification's eave choices apply to the record.
        let mut edited = again.planes[1].clone();
        edited.eave.rafter_tails = Some(false);
        let mut target = back;
        assert!(apply_plane_edit(&mut target, 0, &edited));
        assert_eq!(
            load(&target.floors[0]).planes[1].eave.rafter_tails,
            Some(false)
        );
    }

    // ----- round 14: styles, wings, handles, structure, holes -----

    fn style_settings() -> RoofSettings {
        let mut s = RoofSettings::from_defaults(&eave_tip_defaults());
        s.overhang = 0.0;
        s
    }

    fn kinds(p: &Project) -> Vec<RoofWallKind> {
        p.floors[0].walls.iter().map(|w| w.roof.kind).collect()
    }

    #[test]
    fn the_hip_style_writes_hip_walls_and_builds_four_planes() {
        let mut p = gable_house();
        assert_eq!(apply_style(&mut p, 0, RoofStyle::Hip, 8.0).unwrap(), 4);
        assert!(kinds(&p).iter().all(|k| *k == RoofWallKind::Hip));
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        assert_eq!(load(&p.floors[0]).planes.len(), 4);
    }

    #[test]
    fn the_gable_style_makes_the_walls_across_the_ridge_gables() {
        let mut p = rect_project(480.0, 288.0);
        apply_style(&mut p, 0, RoofStyle::Gable, 8.0).unwrap();
        // The ridge runs along the long (south and north) walls.
        assert_eq!(
            kinds(&p),
            vec![
                RoofWallKind::Hip,
                RoofWallKind::FullGable,
                RoofWallKind::Hip,
                RoofWallKind::FullGable
            ]
        );
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        assert_eq!(load(&p.floors[0]).planes.len(), 2);
    }

    #[test]
    fn the_shed_style_has_one_high_wall_and_one_plane() {
        let mut p = rect_project(480.0, 288.0);
        apply_style(&mut p, 0, RoofStyle::Shed, 4.0).unwrap();
        let k = kinds(&p);
        assert_eq!(k[0], RoofWallKind::HighShedGable, "the back wall is high");
        assert_eq!(k[1], RoofWallKind::FullGable);
        assert_eq!(k[2], RoofWallKind::Hip);
        assert_eq!(k[3], RoofWallKind::FullGable);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 1);
        // It rises toward the high wall.
        let r = &set.planes[0];
        assert!(r.up_slope().y < -0.99, "rises toward y = 0");
    }

    #[test]
    fn the_gambrel_style_breaks_each_long_side_into_two_pitches() {
        let mut p = rect_project(480.0, 288.0);
        apply_style(&mut p, 0, RoofStyle::Gambrel, 8.0).unwrap();
        assert_eq!(
            p.floors[0].walls[0].roof.pitch_in_12,
            Some(GAMBREL_PITCHES.0)
        );
        assert!(p.floors[0].walls[0].roof.upper_pitch.is_some());
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4, "a steep and a shallow plane per side");
        let mut pitches: Vec<f64> = set.planes.iter().map(|r| r.pitch).collect();
        pitches.sort_by(f64::total_cmp);
        assert_eq!(pitches, vec![6.0, 6.0, 18.0, 18.0]);
        // Hip clears the steep pitch again.
        apply_style(&mut p, 0, RoofStyle::Hip, 8.0).unwrap();
        assert_eq!(p.floors[0].walls[0].roof.pitch_in_12, None);
        assert_eq!(p.floors[0].walls[0].roof.upper_pitch, None);
    }

    #[test]
    fn the_dutch_gable_style_keeps_a_gable_face_above_the_end_hips() {
        let mut p = rect_project(480.0, 288.0);
        apply_style(&mut p, 0, RoofStyle::DutchGable, 8.0).unwrap();
        assert_eq!(p.floors[0].walls[1].roof.kind, RoofWallKind::DutchGable);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        assert!(
            !set.faces.is_empty(),
            "the vertical face of the Dutch gable"
        );
        assert!(set.planes.len() >= 4);
    }

    #[test]
    fn the_half_hip_style_clips_the_gable_peaks() {
        let mut p = rect_project(480.0, 288.0);
        apply_style(&mut p, 0, RoofStyle::HalfHip, 8.0).unwrap();
        let end = &p.floors[0].walls[1].roof;
        assert_eq!(end.kind, RoofWallKind::FullGable);
        assert!(end.upper_pitch.is_some(), "Starts at Height of the hip");
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        // Two long planes and a small hip at each end.
        assert_eq!(set.planes.len(), 4);
        let plain = {
            let mut q = rect_project(480.0, 288.0);
            apply_style(&mut q, 0, RoofStyle::Gable, 8.0).unwrap();
            rebuild(&mut q, 0, style_settings(), false).unwrap();
            roof_peak(&q)
        };
        assert!((roof_peak(&p) - plain).abs() < 1e-6, "same ridge height");
    }

    #[test]
    fn setting_a_walls_roof_kind_reaches_the_whole_edge_and_knee_walls_stay_single() {
        let mut p = rect_project(480.0, 288.0);
        // Split the east wall in two.
        let east = p.floors[0].walls[1].id;
        let id2 = p.alloc_id();
        let mut half = p.floors[0].walls[1].clone();
        half.id = id2;
        half.start = Point::new(480.0, 144.0);
        p.floors[0].walls[1].end = Point::new(480.0, 144.0);
        half.end = Point::new(480.0, 288.0);
        p.floors[0].walls.push(half);
        let n = set_walls_roof_kind(&mut p, 0, &[east], RoofWallKind::FullGable);
        assert_eq!(n, 2, "both walls of the east edge");
        assert_eq!(
            set_walls_roof_kind(&mut p, 0, &[east], RoofWallKind::FullGable),
            0,
            "already so"
        );
        let n = set_walls_roof_kind(&mut p, 0, &[east], RoofWallKind::KneeWall);
        assert_eq!(n, 1, "a knee wall is the wall itself");
    }

    // ----- wings -----

    /// A 40' x 24' house with 109" walls and a 20' x 24' garage on its east
    /// side with `garage_h` walls, on floor 0. The shared wall is exterior
    /// when `shared` is.
    fn house_with_garage(garage_h: f64, shared: WallKind) -> Project {
        let mut p = Project::new("t");
        let box_ = |p: &mut Project, x0: f64, x1: f64, h: f64, skip_west: bool| {
            let pts = [
                Point::new(x0, 0.0),
                Point::new(x1, 0.0),
                Point::new(x1, 288.0),
                Point::new(x0, 288.0),
            ];
            for i in 0..4 {
                if skip_west && i == 3 {
                    continue;
                }
                p.add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, h, WallKind::Exterior);
            }
        };
        box_(&mut p, 0.0, 480.0, 109.0, false);
        // The garage shares the main house's east wall.
        box_(&mut p, 480.0, 720.0, garage_h, true);
        let east = p.floors[0]
            .walls
            .iter_mut()
            .find(|w| (w.start.x - 480.0).abs() < 1e-6 && (w.end.x - 480.0).abs() < 1e-6)
            .unwrap();
        east.kind = shared;
        p
    }

    fn planes_over(p: &Project, fi: usize, at: Point) -> Vec<RoofPlaneRecord> {
        load(&p.floors[fi])
            .planes
            .into_iter()
            .filter(|r| r.contains(at))
            .collect()
    }

    #[test]
    fn a_lower_garage_gets_its_own_roof_at_its_own_plate_height() {
        let mut p = house_with_garage(96.0, WallKind::Interior);
        let rep = rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        assert_eq!(rep.planes, set.planes.len());
        // Main house: four hips at 109"; garage: planes at 96".
        let eaves: Vec<f64> = set.planes.iter().map(|r| r.baseline_height()).collect();
        assert_eq!(
            eaves.iter().filter(|e| (**e - 109.0).abs() < 1e-6).count(),
            4
        );
        let garage: Vec<&RoofPlaneRecord> = set
            .planes
            .iter()
            .filter(|r| (r.baseline_height() - 96.0).abs() < 1e-6)
            .collect();
        assert!(garage.len() >= 2, "{} garage planes", garage.len());
        // The garage roof stays east of the main house's wall line (it
        // butts the wall: no plane rises from that edge, no overhang).
        for r in &garage {
            assert!(
                r.plan_polygon().iter().all(|q| q.x >= 480.0 - 1e-6),
                "{:?}",
                r.plan_polygon()
            );
        }
        // Over the garage's middle there is exactly one plane.
        assert_eq!(planes_over(&p, 0, Point::new(600.0, 144.0)).len(), 1);
    }

    #[test]
    fn a_house_at_one_height_builds_as_before() {
        let mut p = house_with_garage(109.0, WallKind::Interior);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        // One footprint, one hip roof over both (6 exterior walls on its
        // edge: 4 planes).
        assert_eq!(set.planes.len(), 4);
    }

    #[test]
    fn the_shared_wall_drawn_at_the_main_height_does_not_lift_the_garage() {
        let mut p = house_with_garage(96.0, WallKind::Exterior);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let set = load(&p.floors[0]);
        assert!(set
            .planes
            .iter()
            .any(|r| (r.baseline_height() - 96.0).abs() < 1e-6));
    }

    /// The house with a garage wing on floor 0 and the second floor over the
    /// main house only.
    fn two_story_with_garage() -> Project {
        let mut p = house_with_garage(96.0, WallKind::Exterior);
        p.floors.push(Floor::new("2nd Floor", 109.0));
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ];
        for i in 0..4 {
            p.add_wall(1, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        p
    }

    #[test]
    fn a_one_story_garage_beside_a_two_story_house_gets_a_roof_on_the_first_floor() {
        let mut p = two_story_with_garage();
        let rep = rebuild(&mut p, 1, style_settings(), false).unwrap();
        // The main roof is over floor 1 as ever.
        let top = load(&p.floors[1]);
        assert_eq!(top.planes.len(), 4);
        // The garage's roof is on floor 0, at the floor-0 wall height.
        let low = load(&p.floors[0]);
        assert!(low.planes.len() >= 2);
        assert!(low.planes.iter().all(|r| r.auto));
        assert!(low
            .planes
            .iter()
            .all(|r| (r.baseline_height() - 96.0).abs() < 1e-6));
        assert!(
            low.settings.is_none(),
            "wings keep no settings of their own"
        );
        // Nothing roofs the part of floor 0 under the second floor.
        assert!(planes_over(&p, 0, Point::new(240.0, 144.0)).is_empty());
        assert_eq!(rep.planes, top.planes.len() + low.planes.len());
        // The wing butts the second floor wall: no overhang on that side.
        assert!(low
            .planes
            .iter()
            .all(|r| r.plan_polygon().iter().all(|q| q.x >= 480.0 - 1e-6)));
    }

    #[test]
    fn a_partly_covered_room_is_not_roofed_on_the_floor_below() {
        // The 2nd floor covers a quarter of the first floor's only room.
        let mut p = rect_project(480.0, 288.0);
        p.floors.push(Floor::new("2nd Floor", 109.0));
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        for i in 0..4 {
            p.add_wall(1, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        rebuild(&mut p, 1, style_settings(), false).unwrap();
        assert!(load(&p.floors[0]).planes.is_empty());
    }

    #[test]
    fn wings_follow_the_walls_of_the_floors_below_in_auto_rebuild() {
        let mut p = two_story_with_garage();
        let s = style_settings();
        rebuild(&mut p, 1, s, false).unwrap();
        let stored = load(&p.floors[1]).settings.unwrap();
        assert_eq!(stored.signature, project_signature(&p, 1));
        // Raising the garage walls changes the signature even though the
        // roof's own floor did not change.
        for w in &mut p.floors[0].walls {
            if w.start.x > 480.0 || w.end.x > 480.0 {
                w.height = 100.0;
            }
        }
        assert_ne!(stored.signature, project_signature(&p, 1));
    }

    #[test]
    fn rebuilding_replaces_the_wings_and_keeps_manual_planes_below() {
        let mut p = two_story_with_garage();
        rebuild(&mut p, 1, style_settings(), false).unwrap();
        let n = load(&p.floors[0]).planes.len();
        let mut set = load(&p.floors[0]);
        let id = p.alloc_id();
        let mut manual = set.planes[0].clone();
        manual.id = id;
        manual.auto = false;
        // Stands clear of the rebuilt planes: one lying on a rebuilt plane
        // replaces it (the Retain switches, DECISIONS RB8).
        for v in &mut manual.polygon3d {
            v[0] += 5000.0;
        }
        set.planes.push(manual);
        store(&mut p, 0, &mut set);
        rebuild(&mut p, 1, style_settings(), false).unwrap();
        let low = load(&p.floors[0]);
        assert_eq!(low.planes.len(), n + 1);
        assert_eq!(low.planes.iter().filter(|r| !r.auto).count(), 1);
    }

    #[test]
    fn a_wall_below_gives_extend_slope_downward_its_length() {
        // The second floor stands back from the first floor's walls.
        let mut p = rect_project(480.0, 288.0);
        p.floors.push(Floor::new("2nd Floor", 109.0));
        let pts = [
            Point::new(60.0, 60.0),
            Point::new(420.0, 60.0),
            Point::new(420.0, 228.0),
            Point::new(60.0, 228.0),
        ];
        for i in 0..4 {
            let w = p.add_wall(1, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
            if i == 0 {
                p.floors[1].wall_mut(w).unwrap().roof.kind = RoofWallKind::ExtendSlopeDownward;
            }
        }
        rebuild(&mut p, 1, style_settings(), false).unwrap();
        let set = load(&p.floors[1]);
        let south = set
            .planes
            .iter()
            .find(|r| {
                r.source
                    .is_some_and(|(a, _)| (a.y - 60.0).abs() < 1.0 && (a.x - 60.0).abs() < 1.0)
            })
            .expect("the south plane");
        let lowest = south
            .polygon3d
            .iter()
            .map(|v| v[1])
            .fold(f64::INFINITY, f64::min);
        // Eave 218", the wall below tops out at 109".
        assert!((lowest - 109.0).abs() < 1e-6, "{lowest}");
        // An explicit drop still wins.
        for w in &mut p.floors[1].walls {
            if w.roof.kind == RoofWallKind::ExtendSlopeDownward {
                w.roof.extend_drop = Some(12.0);
            }
        }
        rebuild(&mut p, 1, style_settings(), false).unwrap();
        let set = load(&p.floors[1]);
        let south = set
            .planes
            .iter()
            .find(|r| {
                r.source
                    .is_some_and(|(a, _)| (a.y - 60.0).abs() < 1.0 && (a.x - 60.0).abs() < 1.0)
            })
            .unwrap();
        let lowest = south
            .polygon3d
            .iter()
            .map(|v| v[1])
            .fold(f64::INFINITY, f64::min);
        assert!((lowest - 206.0).abs() < 1e-6, "{lowest}");
    }

    // ----- handles, edit all, structure, holes, dormers -----

    #[test]
    fn dragging_handles_through_the_project_matches_the_edit_mode_math() {
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let rec = load(&p.floors[0]).planes[0].clone();
        let (_, pitch_at) = rec
            .handles()
            .into_iter()
            .find(|(h, _)| *h == PlaneHandle::Pitch)
            .unwrap();
        let to = pitch_at.add(rec.up_slope().scale(8.0));
        let d = apply_handle_drag(&mut p, 0, rec.id, PlaneHandle::Pitch, pitch_at, to).unwrap();
        assert_eq!(d.label, "Change Roof Pitch");
        assert!((d.record.pitch - 10.0).abs() < 1e-9, "8 + 8 * 0.25");
        let after = load(&p.floors[0]);
        assert!((after.plane(rec.id).unwrap().pitch - 10.0).abs() < 1e-9);
        assert!(!after.plane(rec.id).unwrap().auto);
    }

    #[test]
    fn edit_all_planes_changes_material_layer_and_eave_choices_everywhere() {
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let edit = AllPlanesEdit {
            material: Some("Standing Seam Metal".into()),
            ridge_caps: Some(true),
            fascia: Some(Some(false)),
            gutters: Some(Some(true)),
            ..AllPlanesEdit::default()
        };
        assert_eq!(apply_all(&mut p, 0, &edit), 4);
        for r in load(&p.floors[0]).planes {
            assert_eq!(r.material, "Standing Seam Metal");
            assert!(r.ridge_caps && r.gutters);
            assert_eq!(r.eave.fascia, Some(false));
            assert_eq!(r.eave.gutters, Some(true));
        }
        assert_eq!(apply_all(&mut p, 0, &AllPlanesEdit::default()), 0);
    }

    #[test]
    fn edit_all_planes_pitch_rebuilds_the_automatic_roof_so_hips_follow() {
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let before = roof_peak(&p);
        let edit = AllPlanesEdit {
            pitch: Some(12.0),
            material: Some("Slate".into()),
            ..AllPlanesEdit::default()
        };
        apply_all(&mut p, 0, &edit);
        let set = load(&p.floors[0]);
        assert_eq!(set.planes.len(), 4);
        assert!(set.planes.iter().all(|r| r.auto && r.pitch == 12.0));
        assert!(roof_peak(&p) > before + 40.0);
        // The other choice survives the rebuild.
        assert!(set.planes.iter().all(|r| r.material == "Slate"));
        // Manual planes change in place.
        let mut set = load(&p.floors[0]);
        set.planes.retain(|r| !r.auto);
        let (base, poly) = manual_plane_geometry(
            Point::new(0.0, 400.0),
            Point::new(100.0, 400.0),
            Point::new(50.0, 450.0),
            100.0,
            4.0,
        )
        .unwrap();
        let id = p.alloc_id();
        let mut m = RoofPlaneRecord::new(id, poly, 4.0, base);
        m.auto = false;
        let mut set2 = load(&p.floors[0]);
        set2.planes.push(m);
        store(&mut p, 0, &mut set2);
        apply_all(
            &mut p,
            0,
            &AllPlanesEdit {
                pitch: Some(6.0),
                ..AllPlanesEdit::default()
            },
        );
        let again = load(&p.floors[0]);
        assert_eq!(again.plane(id).unwrap().pitch, 6.0);
    }

    #[test]
    fn an_imported_planes_chief_edges_survive_storing_and_editing() {
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let mut set = load(&p.floors[0]);
        let edges = json!([{"role": "eave", "joined": false, "overhangs": true}]);
        set.planes[0].chief_edges = Some(edges.clone());
        let id = set.planes[0].id;
        store(&mut p, 0, &mut set);
        let mut again = load(&p.floors[0]);
        assert_eq!(again.plane(id).unwrap().chief_edges, Some(edges.clone()));
        // Editing the plane (its label) writes the record back with them.
        again.plane_mut(id).unwrap().label = "Garage".into();
        store(&mut p, 0, &mut again);
        let third = load(&p.floors[0]);
        assert_eq!(third.plane(id).unwrap().chief_edges, Some(edges));
        assert_eq!(third.plane(id).unwrap().label, "Garage");
    }

    #[test]
    fn a_planes_own_structure_sets_its_thickness_and_rafters_and_round_trips() {
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let mut set = load(&p.floors[0]);
        let st = RoofStructure {
            framing: RoofFraming::Trusses,
            member_width: 1.5,
            member_depth: 9.25,
            spacing: 16.0,
            sheathing: 0.5,
            roofing: 0.75,
            ceiling: CeilingFraming::TrussBottomChords,
        };
        set.planes[0].set_structure(Some(st));
        let id = set.planes[0].id;
        assert!((st.thickness() - 10.5).abs() < 1e-9);
        assert_eq!(set.planes[0].eave.thickness, Some(10.5));
        assert_eq!(set.planes[0].eave.rafter_spacing, Some(16.0));
        store(&mut p, 0, &mut set);
        let again = load(&p.floors[0]);
        assert_eq!(again.plane(id).unwrap().structure, Some(st));
        // A rebuild keeps it.
        rebuild(&mut p, 0, style_settings(), false).unwrap();
        let rebuilt = load(&p.floors[0]);
        assert_eq!(
            rebuilt
                .planes
                .iter()
                .filter(|r| r.structure == Some(st))
                .count(),
            1
        );
        // The slab is thicker than the others in 3D: its meshes hang lower.
        let drop = |rec: &RoofPlaneRecord| {
            let ms = tagged(
                plan_3d::roof_plane_meshes(
                    &roof_plane_with_holes(&rec.to_roof_plane(0), &[]),
                    rec.eave.thickness.unwrap_or(6.0),
                ),
                rec.id,
            );
            ms.iter()
                .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
                .fold(f32::MAX, f32::min)
        };
        let thick = rebuilt
            .planes
            .iter()
            .find(|r| r.structure.is_some())
            .unwrap();
        let thin = rebuilt
            .planes
            .iter()
            .find(|r| r.structure.is_none())
            .unwrap();
        assert!(drop(thick) < drop(thin) - 2.0);
        // Back to Roof Defaults.
        let mut set = rebuilt;
        let k = set
            .planes
            .iter()
            .position(|r| r.structure.is_some())
            .unwrap();
        set.planes[k].set_structure(None);
        assert_eq!(set.planes[k].eave, plan_3d::EaveOverrides::default());
    }

    #[test]
    fn a_polygon_hole_inside_a_plane_and_across_a_ridge() {
        let mut p = rect_project(480.0, 288.0);
        let s = style_settings();
        let mut g = gable_house();
        rebuild(&mut g, 0, s.clone(), false).unwrap();
        // L-shaped hole inside the south plane.
        let l = vec![
            Point::new(100.0, 40.0),
            Point::new(160.0, 40.0),
            Point::new(160.0, 80.0),
            Point::new(130.0, 80.0),
            Point::new(130.0, 110.0),
            Point::new(100.0, 110.0),
        ];
        let id = add_hole_polygon(&mut g, 0, l.clone(), false).unwrap();
        let rec = load(&g.floors[0]).plane(id).unwrap().clone();
        assert_eq!(rec.holes.len(), 1);
        assert_eq!(rec.holes[0].outline.len(), 6);
        // The 3D roof is open there and closed in the notch.
        assert!(!roof_covers(&g, Point::new(110.0, 50.0)));
        assert!(roof_covers(&g, Point::new(150.0, 100.0)));
        // A bow tie crosses itself and is refused.
        let bow = vec![
            Point::new(100.0, 40.0),
            Point::new(160.0, 100.0),
            Point::new(160.0, 40.0),
            Point::new(100.0, 100.0),
        ];
        assert!(add_hole_polygon(&mut g, 0, bow, false).is_err());
        // A polygon across the ridge is cut into a piece per plane.
        let mut h = gable_house();
        rebuild(&mut h, 0, s, false).unwrap();
        let across = vec![
            Point::new(200.0, 110.0),
            Point::new(300.0, 110.0),
            Point::new(300.0, 180.0),
            Point::new(250.0, 200.0),
            Point::new(200.0, 180.0),
        ];
        add_hole_polygon(&mut h, 0, across, false).unwrap();
        let holes: usize = load(&h.floors[0])
            .planes
            .iter()
            .map(|r| r.holes.len())
            .sum();
        assert!(holes >= 2, "{holes} hole pieces");
        assert!(!roof_covers(&h, Point::new(250.0, 130.0)));
        assert!(!roof_covers(&h, Point::new(250.0, 160.0)));
        let _ = &mut p;
    }

    #[test]
    fn polygons_that_cross_themselves_are_found() {
        let sq = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        assert!(!polygon_self_intersects(&sq));
        let bow = [sq[0], sq[2], sq[1], sq[3]];
        assert!(polygon_self_intersects(&bow));
    }

    #[test]
    fn a_dormer_slides_onto_the_other_plane_when_dragged_across_the_ridge() {
        let mut g = gable_house();
        rebuild(&mut g, 0, style_settings(), false).unwrap();
        let mut set = load(&g.floors[0]);
        let south = set.planes.iter().find(|r| r.up_slope().y > 0.9).unwrap().id;
        let north = set
            .planes
            .iter()
            .find(|r| r.up_slope().y < -0.9)
            .unwrap()
            .id;
        let spec = dormer_spec_at(&set, south, Point::new(240.0, 20.0));
        let d = apply_dormer(&mut g, 0, south, None, spec).unwrap();
        set = load(&g.floors[0]);
        assert_eq!(set.dormer(d).unwrap().main, south);
        // A small step stays on the plane.
        assert!(slide_dormer(
            &mut set,
            d,
            Point::new(20.0, 0.0),
            Point::new(260.0, 70.0)
        ));
        assert_eq!(set.dormer(d).unwrap().main, south);
        // The pointer over the north plane takes the dormer there.
        assert!(slide_dormer(
            &mut set,
            d,
            Point::ZERO,
            Point::new(260.0, 285.0)
        ));
        assert_eq!(set.dormer(d).unwrap().main, north);
        // Off the roof it stays.
        assert!(!slide_dormer(
            &mut set,
            d,
            Point::new(0.0, 5000.0),
            Point::new(260.0, 5000.0)
        ));
        assert_eq!(set.dormer(d).unwrap().main, north);
    }
}
