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

use super::{Camera, EditorContext};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::defaults::{RoofDetailDefaults, RoofWallKind};
use plan_core::geometry::{dist_to_segment, point_in_polygon, polygon_centroid, Point};
use plan_core::{
    detect_rooms, Floor, Id, Layer, LineStyle, PlanDefaults, Project, Room, Wall, WallKind,
};
use plan_roof::{
    apply_gable_line, auto_dormer, build_roof_at_plate, build_roof_with_specs,
    ceiling_planes_for_vaulted_room, flat_roof_plane, footprint_from_walls, join_planes,
    roof_plane_with_holes, roof_return_at, CeilingPlane, Dormer, DormerSpec, EdgeRoofSpec,
    ReturnKind, ReturnSpec, Roof, RoofHole, RoofPlane, SkylightSpec,
};
use serde_json::{json, Value};

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
}

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

/// `8:12`, or `8.5:12` for fractional pitches.
pub fn pitch_label(pitch: f64) -> String {
    if (pitch - pitch.round()).abs() < 0.05 {
        format!("{}:12", pitch.round() as i64)
    } else {
        format!("{pitch:.1}:12")
    }
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
    pub material: String,
    /// [`wall_signature`] at the last build (Auto Rebuild compares it).
    pub signature: u64,
    /// Per-edge overrides set in the Roof Plane Specification.
    pub edge_specs: Vec<EdgeSpec>,
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
            material: ROOF_MATERIALS[0].to_string(),
            signature: 0,
            edge_specs: Vec::new(),
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
            material: ROOF_MATERIALS[0].to_string(),
            signature: 0,
            edge_specs: Vec::new(),
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
            material: field!(v, "material", String).unwrap_or_else(|| defaults.material.clone()),
            signature: field!(v, "signature", u64).unwrap_or(0),
            edge_specs: field!(v, "edge_specs", Vec<Value>)
                .map(|e| e.iter().filter_map(EdgeSpec::from_json).collect())
                .unwrap_or_default(),
            // A roof stored before the detail existed keeps its baseline.
            detail: field!(v, "detail", RoofDetailDefaults).unwrap_or_else(|| RoofDetailDefaults {
                baseline_at_plate: false,
                ..defaults.detail.clone()
            }),
        }
    }
}

impl RoofSettings {
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
                    g.roof_planes
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
                    g.roof_planes.iter().map(RoofPlane::plan_polygon).collect(),
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
            Some("settings") => set.settings = Some(RoofSettings::from_json(v, &base)),
            _ => {}
        }
    }
    if let Some(s) = &set.settings {
        for r in &mut set.planes {
            if let Some(edge) = r.source {
                r.edge = s.override_of(edge).unwrap_or_default();
            }
        }
    }
    set
}

/// Writes `set` back as the floor's roof, replacing what was stored.
pub fn store(project: &mut Project, fi: usize, set: &mut RoofSet) {
    let mut items: Vec<Value> = set.planes.iter().map(RoofPlaneRecord::to_json).collect();
    items.extend(set.ceilings.iter().map(CeilingRecord::to_json));
    items.extend(set.dormers.iter().map(DormerRecord::to_json));
    if let Some(s) = &set.settings {
        items.push(s.to_json());
    }
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

/// `eave_height` is the eave's height above the floor, which turns a wall's
/// "Starts at Height" (above the floor) into a rise over the eave.
fn edge_plans(walls: &[Wall], fp: &[Point], s: &RoofSettings, eave_height: f64) -> Vec<EdgePlan> {
    let n = fp.len();
    (0..n)
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
                // The plane keeps sloping below its eave (RF-24).
                extend_slope_downward: (kind == RoofWallKind::ExtendSlopeDownward).then_some(
                    primary
                        .and_then(|w| w.roof.extend_drop)
                        .filter(|d| *d > 0.0)
                        .unwrap_or(EXTEND_SLOPE_DROP),
                ),
                dutch_gable: kind == RoofWallKind::DutchGable,
                ..EdgeRoofSpec::default()
            };
            // The second pitch and the height it starts at (RF-25); a
            // Dutch gable starts at the same height when one is given.
            if let Some((rise, start)) = primary.and_then(|w| w.roof.upper_pitch) {
                let over_eave = start - eave_height;
                if over_eave > 0.0 {
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
        .collect()
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

/// New automatic planes over floor `fi`, and whether plan-roof had to
/// approximate. Ids are allocated from `project`.
fn make_auto_planes(
    project: &mut Project,
    fi: usize,
    s: &RoofSettings,
) -> Result<(Vec<RoofPlaneRecord>, bool), String> {
    let floor = &project.floors[fi];
    let rooms = detect_rooms(&floor.walls, TOL);
    // A floor whose every room is flat or roofless has no pitched roof.
    let pitched = rooms.is_empty()
        || rooms
            .iter()
            .any(|r| room_roof(floor, r) == RoomRoof::Pitched);
    let mut out = Vec::new();
    let mut approximate = false;
    if pitched {
        let walls = exterior_walls(floor);
        let fp = footprint_from_walls(&walls, TOL)
            .ok_or_else(|| "The exterior walls do not enclose an area".to_string())?;
        let top = walls.iter().map(|w| w.height).fold(0.0, f64::max);
        let plans = edge_plans(&walls, &fp, s, top + s.raise_off_plate);
        let specs: Vec<EdgeRoofSpec> = plans.iter().map(|e| e.spec).collect();
        let plate = floor.elevation + top + s.raise_off_plate;
        // With the baseline rule on the structure sits on the top plate at
        // the wall; without it the eave tip is at plate height.
        let roof = if s.detail.baseline_at_plate {
            build_roof_at_plate(&fp, &specs, plate, s.detail.thickness)
        } else {
            build_roof_with_specs(&fp, &specs, plate)
        };
        approximate = roof.approximate;
        let n = fp.len();
        let returns = auto_returns(&roof.planes, &plans);
        for pl in roof.planes {
            let mut r = RoofPlaneRecord::new(0, pl.polygon3d, pl.pitch_in_12, pl.baseline);
            r.auto = true;
            r.source = Some((fp[pl.source_edge % n], fp[(pl.source_edge + 1) % n]));
            r.overhang = plans
                .get(pl.source_edge)
                .map_or(s.overhang, |e| e.face_overhang);
            r.material = s.material.clone();
            out.push(r);
        }
        for (src, ret) in returns {
            let mut r = RoofPlaneRecord::new(0, ret.polygon3d, ret.pitch_in_12, ret.baseline);
            r.auto = true;
            r.material = s.material.clone();
            r.overhang = plans.get(src).map_or(s.overhang, |e| e.face_overhang);
            out.push(r);
        }
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
                if let Some(pl) = flat_roof_plane(&room.polygon, height) {
                    let mut r = RoofPlaneRecord::new(0, pl.polygon3d, 0.0, pl.baseline);
                    r.auto = true;
                    r.material = s.material.clone();
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
        }
    }
    for r in &mut out {
        r.id = project.alloc_id();
    }
    Ok((out, approximate))
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
    set.planes.retain(|p| !p.auto);
    let mut approximate = false;
    let mut built = 0;
    if settings.build_planes {
        match make_auto_planes(project, fi, &settings) {
            Ok((mut planes, approx)) => {
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
    settings.signature = wall_signature(&project.floors[fi]);
    set.settings = Some(settings);
    store(project, fi, &mut set);
    Ok(BuildReport {
        floor: fi,
        planes: built,
        ceilings,
        approximate,
    })
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
        if target == fi && wall_signature(&cx.project.floors[fi]) == s.signature {
            continue;
        }
        if target != fi {
            // The roof moved to another floor: drop this floor's automatic
            // planes and settings, keep the manual ones.
            let mut old = set;
            old.planes.retain(|p| !p.auto);
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
    let mut spec = DormerSpec::default();
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
        let height = dw.height.min(rec.spec.wall_height).max(1.0);
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
    if a == b {
        return Err("Pick a different plane to join to".into());
    }
    let mut set = load(&project.floors[fi]);
    let pa = set.plane(a).ok_or("The first roof plane is gone")?;
    let pb = set.plane(b).ok_or("The second roof plane is gone")?;
    let joined = join_planes(&pa.to_roof_plane(0), edge, &pb.to_roof_plane(0))
        .ok_or("These planes cannot be joined along that edge (parallel planes?)")?;
    let rec = set.plane_mut(a).ok_or("The first roof plane is gone")?;
    rec.polygon3d = joined.polygon3d;
    rec.baseline = joined.baseline;
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
) {
    let n = poly.len();
    let min_h = heights.iter().copied().fold(f64::INFINITY, f64::min);
    for i in 0..n {
        let j = (i + 1) % n;
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
        painter.line_segment(
            [cam.world_to_screen(poly[i]), cam.world_to_screen(poly[j])],
            Stroke::new(width as f32, color),
        );
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
) {
    let at = cam.world_to_screen(c);
    if !cam.rect.expand(40.0).contains(at) {
        return;
    }
    // Slope arrow: down the slope, 26 px long.
    let len = 26.0 / cam.px_per_in.max(1e-6);
    let tail = cam.world_to_screen(c.add(up.scale(len * 0.5)));
    let tip = cam.world_to_screen(c.sub(up.scale(len * 0.5)));
    let stroke = Stroke::new(1.25_f32, color);
    painter.line_segment([tail, tip], stroke);
    let dir = (tip - tail).normalized();
    let side = egui::Vec2::new(-dir.y, dir.x);
    for s in [-1.0_f32, 1.0] {
        painter.line_segment([tip, tip - dir * 6.0 + side * (3.5 * s)], stroke);
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
    let set = load(cx.floor());
    if set.planes.is_empty() && set.ceilings.is_empty() {
        return;
    }
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
        draw_plane_outline(painter, cam, &poly, &heights, &others, color);
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
        draw_slope_label(painter, cam, r.centroid(), r.up_slope(), text, color);
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
            .roof_planes
            .iter()
            .map(|p| poly_edges(&p.plan_polygon()))
            .collect();
        for (k, pl) in g.roof_planes.iter().enumerate() {
            let heights: Vec<f64> = pl.polygon3d.iter().map(|v| v[1]).collect();
            let others: Vec<Vec<(Point, Point)>> = planes
                .iter()
                .enumerate()
                .filter(|(o, _)| *o != k)
                .map(|(_, e)| e.clone())
                .collect();
            draw_plane_outline(painter, cam, &pl.plan_polygon(), &heights, &others, color);
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
    floor_roof_meshes_in(floor, None)
}

/// [`floor_roof_meshes`] with the planes cut where they butt a taller wall:
/// `cover` is the floor's part of `plan_3d::RoofCover`, whose planes are
/// already trimmed (a plane trimmed away entirely is left out).
fn floor_roof_meshes_in(floor: &Floor, cover: Option<&plan_3d::FloorCover>) -> Vec<plan_3d::Mesh> {
    let set = load(floor);
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
    for r in &set.planes {
        let mut plane = r.to_roof_plane(0);
        if let Some(c) = cover {
            // The cover holds the plane after butting walls trimmed it.
            match c.eaves.iter().find(|e| e.id == Some(r.id)) {
                Some(e) => plane.polygon3d = e.plane.polygon3d.clone(),
                None => continue,
            }
        }
        let mut holes = r.roof_holes();
        holes.extend(
            dormers
                .iter()
                .filter(|(d, _)| d.main == r.id && !d.floating)
                .map(|(_, g)| g.hole_in_main_roof.clone()),
        );
        let poly = roof_plane_with_holes(&plane, &holes);
        out.extend(tagged(plan_3d::roof_plane_meshes(&poly, thickness), r.id));
    }
    for c in &set.ceilings {
        out.extend(tagged(plan_3d::ceiling_plane_meshes(&c.to_plane()), c.id));
    }
    for (d, g) in &dormers {
        out.extend(tagged(
            plan_3d::dormer_meshes(g, DORMER_WALL_THICKNESS, SLAB_THICKNESS),
            d.id,
        ));
    }
    out
}

/// [`floor_roof_meshes`] of every floor, with each plane trimmed at the face
/// of any taller wall it butts (RF-13).
pub fn roof_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let cover = plan_3d::RoofCover::from_project(project);
    project
        .floors
        .iter()
        .enumerate()
        .flat_map(|(i, f)| floor_roof_meshes_in(f, cover.floor(i)))
        .collect()
}

/// Eave detail of every roof plane (RF-14, RF-15): fascia and soffit along
/// the eaves, rake boards and soffit on gable ends, optional frieze and ridge
/// caps, and the flashing line where a lower roof butts a wall. Each mesh is
/// tagged with its roof plane's id. Kept apart from [`roof_meshes`] so the
/// plane slabs stay one mesh per plane.
pub fn roof_detail_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    plan_3d::RoofCover::from_project(project).eave_meshes()
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
}
