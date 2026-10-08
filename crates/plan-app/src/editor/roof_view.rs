//! Roof planes: storage, automatic building, editing math, plan display and
//! the 3D meshes (`docs/parity/roofs.md`).
//!
//! # Storage (temporary)
//!
//! `plan-core` has no roof field yet, so roof planes ride in the opaque
//! `Floor.cad` list of the floor they sit on, which is saved, loaded and
//! undone with the plan like everything else:
//!
//! * layer `"Roof Planes, Data"` (hidden): one `CadItem::Text` per plane whose
//!   text is `RFP1:` + the JSON of a [`RoofPlaneRecord`] (the object id is the
//!   plane id), and one `RFS1:` + JSON of the [`RoofSettings`] of the last
//!   Build Roof;
//! * the plane's own layer (`"Roof Planes"`): a closed `CadItem::Polyline` of
//!   the plane outline, so DXF export and other CAD consumers see the roof.
//!
//! The data items are the source of truth; the polylines are rewritten from
//! them on every [`store`].
//!
//! TODO: replace with a `roofs: Vec<RoofPlaneRecord>` field on `plan_core::Floor`
//! (and `roof: RoofSettings` on `Project`), then delete the CAD-slot code.
//!
//! Coordinates in a record follow `plan-roof`: `[x, elevation, -plan_y]`.

use super::{Camera, EditorContext};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Stroke};
use plan_core::cad::{CadItem, CadObject};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::{
    dist_to_segment, point_in_polygon, polygon_area, polygon_centroid, Point,
};
use plan_core::{Floor, Id, Layer, PlanDefaults, Project, Wall, WallKind};
use plan_roof::{build_roof, footprint_from_walls, EdgeKind, EdgeRoof};
use serde_json::{json, Value};

pub const LAYER_PLANES: &str = "Roof Planes";
pub const LAYER_DATA: &str = "Roof Planes, Data";
const PLANE_TAG: &str = "RFP1:";
const SETTINGS_TAG: &str = "RFS1:";
/// Footprint / wall matching tolerance, inches.
const TOL: f64 = 0.5;
/// Default skylight size (RF-43), inches: width, length along the slope.
pub const SKYLIGHT_SIZE: (f64, f64) = (24.0, 48.0);
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

macro_rules! field {
    ($v:expr, $k:literal, $t:ty) => {
        $v.get($k)
            .and_then(|x| serde_json::from_value::<$t>(x.clone()).ok())
    };
}

// ===================================================================
// Records
// ===================================================================

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
    /// One rectangular/polygonal hole (RF-42).
    pub hole: Option<Vec<Point>>,
    /// `(center, width, length)` of each skylight (RF-43).
    pub skylights: Vec<(Point, f64, f64)>,
    /// Horizontal overhang beyond the wall face, inches (read-only display).
    pub overhang: f64,
    pub label: String,
    pub material: String,
    pub layer: String,
    pub ridge_caps: bool,
    pub gutters: bool,
    /// Id of the derived outline polyline in `Floor.cad` (0 = none yet).
    pub outline_id: Id,
}

impl RoofPlaneRecord {
    pub fn new(id: Id, polygon3d: Vec<[f64; 3]>, pitch: f64, baseline: (Point, Point)) -> Self {
        Self {
            id,
            polygon3d,
            pitch,
            baseline,
            auto: false,
            hole: None,
            skylights: Vec::new(),
            overhang: 0.0,
            label: String::new(),
            material: ROOF_MATERIALS[0].to_string(),
            layer: LAYER_PLANES.to_string(),
            ridge_caps: false,
            gutters: false,
            outline_id: 0,
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
        if let Some(h) = &mut self.hole {
            for p in h.iter_mut() {
                *p = p.add(d);
            }
        }
        for s in &mut self.skylights {
            s.0 = s.0.add(d);
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

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "polygon3d": self.polygon3d,
            "pitch": self.pitch,
            "baseline": [self.baseline.0, self.baseline.1],
            "auto": self.auto,
            "hole": self.hole,
            "skylights": self.skylights,
            "overhang": self.overhang,
            "label": self.label,
            "material": self.material,
            "layer": self.layer,
            "ridge_caps": self.ridge_caps,
            "gutters": self.gutters,
            "outline_id": self.outline_id,
        })
    }

    pub fn from_json(v: &Value) -> Option<Self> {
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
        r.hole = field!(v, "hole", Vec<Point>);
        r.skylights = field!(v, "skylights", Vec<(Point, f64, f64)>).unwrap_or_default();
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
        r.outline_id = field!(v, "outline_id", Id).unwrap_or(0);
        Some(r)
    }
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

/// The Build Roof dialog's settings (RF-2), kept with the roof.
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
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
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
        })
    }

    pub fn from_json(v: &Value, defaults: &RoofSettings) -> Self {
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
        }
    }
}

/// Everything stored for one floor's roof.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoofSet {
    /// `None` until Build Roof ran (or after Delete Roof Planes).
    pub settings: Option<RoofSettings>,
    pub planes: Vec<RoofPlaneRecord>,
}

impl RoofSet {
    pub fn plane(&self, id: Id) -> Option<&RoofPlaneRecord> {
        self.planes.iter().find(|p| p.id == id)
    }

    pub fn plane_mut(&mut self, id: Id) -> Option<&mut RoofPlaneRecord> {
        self.planes.iter_mut().find(|p| p.id == id)
    }

    /// Topmost (last drawn) plane containing `p`.
    pub fn plane_at(&self, p: Point) -> Option<Id> {
        self.planes
            .iter()
            .rev()
            .find(|r| r.contains(p))
            .map(|r| r.id)
    }
}

// ===================================================================
// Storage in Floor.cad
// ===================================================================

fn data_text<'a>(c: &'a CadObject, tag: &str) -> Option<&'a str> {
    if c.layer != LAYER_DATA {
        return None;
    }
    match &c.item {
        CadItem::Text { text, .. } => text.strip_prefix(tag),
        _ => None,
    }
}

/// Reads the roof stored on `floor`.
pub fn load(floor: &Floor) -> RoofSet {
    let base = RoofSettings::fallback();
    let mut set = RoofSet::default();
    for c in &floor.cad {
        if let Some(json) = data_text(c, PLANE_TAG) {
            if let Some(mut r) = serde_json::from_str::<Value>(json)
                .ok()
                .and_then(|v| RoofPlaneRecord::from_json(&v))
            {
                r.id = c.id;
                set.planes.push(r);
            }
        } else if let Some(json) = data_text(c, SETTINGS_TAG) {
            if let Ok(v) = serde_json::from_str::<Value>(json) {
                set.settings = Some(RoofSettings::from_json(&v, &base));
            }
        }
    }
    set
}

/// Writes `set` back as the floor's roof, replacing what was stored.
pub fn store(project: &mut Project, fi: usize, set: &mut RoofSet) {
    if project.layers.get(LAYER_DATA).is_none() {
        let mut l = Layer::new(LAYER_DATA, [128, 0, 128], 13);
        l.display = false;
        project.layers.add(l);
    }
    let mut cad = std::mem::take(&mut project.floors[fi].cad);
    let old_outlines: Vec<Id> = cad
        .iter()
        .filter_map(|c| data_text(c, PLANE_TAG))
        .filter_map(|j| serde_json::from_str::<Value>(j).ok())
        .filter_map(|v| field!(v, "outline_id", Id))
        .collect();
    let settings_id = cad
        .iter()
        .find(|c| data_text(c, SETTINGS_TAG).is_some())
        .map(|c| c.id);
    cad.retain(|c| c.layer != LAYER_DATA && !old_outlines.contains(&c.id));
    for r in &mut set.planes {
        if r.outline_id == 0 {
            r.outline_id = project.alloc_id();
        }
        let poly = r.plan_polygon();
        cad.push(CadObject {
            id: r.outline_id,
            layer: r.layer.clone(),
            item: CadItem::Polyline {
                points: poly.clone(),
                closed: true,
            },
        });
        cad.push(CadObject {
            id: r.id,
            layer: LAYER_DATA.to_string(),
            item: CadItem::Text {
                pos: poly.first().copied().unwrap_or(Point::ZERO),
                text: format!("{PLANE_TAG}{}", r.to_json()),
                height: 1.0,
                angle: 0.0,
            },
        });
    }
    if let Some(s) = &set.settings {
        let id = settings_id.unwrap_or_else(|| project.alloc_id());
        cad.push(CadObject {
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
    project.floors[fi].cad = cad;
}

/// Does plane `id` exist on `floor`?
pub fn exists(floor: &Floor, id: Id) -> bool {
    floor
        .cad
        .iter()
        .any(|c| c.id == id && data_text(c, PLANE_TAG).is_some())
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
    h
}

fn exterior_walls(floor: &Floor) -> Vec<Wall> {
    floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior)
        .cloned()
        .collect()
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
    roof: EdgeRoof,
    /// Overhang from the wall face, as shown to the user.
    face_overhang: f64,
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

fn edge_plans(walls: &[Wall], fp: &[Point], s: &RoofSettings) -> Vec<EdgePlan> {
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
            let kind = match kind {
                RoofWallKind::FullGable => EdgeKind::Gable,
                RoofWallKind::HighShedGable => EdgeKind::Shed,
                _ => EdgeKind::Hip,
            };
            EdgePlan {
                roof: EdgeRoof {
                    pitch_in_12: pitch,
                    kind,
                    // plan-roof measures from the centerline; RF-10 wants the
                    // wall face.
                    overhang: over + thick * 0.5,
                },
                face_overhang: over,
            }
        })
        .collect()
}

/// Footprint edge index and walls of the edge `wall_id` lies on.
fn edge_of_wall(floor: &Floor, wall_id: Id) -> Option<Vec<Id>> {
    let walls = exterior_walls(floor);
    let fp = footprint_from_walls(&walls, TOL)?;
    let n = fp.len();
    for i in 0..n {
        let on = walls_on_edge(&walls, fp[i], fp[(i + 1) % n]);
        if on.iter().any(|&k| walls[k].id == wall_id) {
            return Some(on.iter().map(|&k| walls[k].id).collect());
        }
    }
    None
}

/// Gable/Roof Line on a wall (RF-18, RF-20): flips every wall along its
/// footprint edge between Hip and Full Gable. Returns the new kind.
pub fn toggle_gable(project: &mut Project, fi: usize, wall_id: Id) -> Option<RoofWallKind> {
    let floor = &project.floors[fi];
    let ids = edge_of_wall(floor, wall_id).unwrap_or_else(|| vec![wall_id]);
    let current = floor.wall(wall_id)?.roof.kind;
    let new = if current == RoofWallKind::FullGable {
        RoofWallKind::Hip
    } else {
        RoofWallKind::FullGable
    };
    for id in ids {
        if let Some(w) = project.floors[fi].wall_mut(id) {
            w.roof.kind = new;
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
    let walls = exterior_walls(floor);
    let fp = footprint_from_walls(&walls, TOL)
        .ok_or_else(|| "The exterior walls do not enclose an area".to_string())?;
    let plans = edge_plans(&walls, &fp, s);
    let edges: Vec<EdgeRoof> = plans.iter().map(|e| e.roof).collect();
    let top = walls.iter().map(|w| w.height).fold(0.0, f64::max);
    let baseline = floor.elevation + top + s.raise_off_plate;
    let roof = build_roof(&fp, &edges, baseline);
    let mut out = Vec::new();
    for pl in roof.planes {
        let mut r = RoofPlaneRecord::new(0, pl.polygon3d, pl.pitch_in_12, pl.baseline);
        r.auto = true;
        r.overhang = plans
            .get(pl.source_edge)
            .map_or(s.overhang, |e| e.face_overhang);
        r.material = s.material.clone();
        out.push(r);
    }
    for r in &mut out {
        r.id = project.alloc_id();
    }
    Ok((out, roof.approximate))
}

/// Copies holes, skylights and per-plane options from the old automatic plane
/// with the same baseline onto the rebuilt one.
fn carry_over(old: &[RoofPlaneRecord], new: &mut [RoofPlaneRecord]) {
    for n in new.iter_mut() {
        let nd = n.baseline.1.sub(n.baseline.0).normalized();
        let nm = Point::lerp(n.baseline.0, n.baseline.1, 0.5);
        let hit = old.iter().find(|o| {
            let od = o.baseline.1.sub(o.baseline.0).normalized();
            let om = Point::lerp(o.baseline.0, o.baseline.1, 0.5);
            om.dist(nm) < 12.0 && od.dot(nd) > 0.99
        });
        if let Some(o) = hit {
            n.hole = o.hole.clone();
            n.skylights = o.skylights.clone();
            n.label = o.label.clone();
            n.material = o.material.clone();
            n.layer = o.layer.clone();
            n.ridge_caps = o.ridge_caps;
            n.gutters = o.gutters;
            n.outline_id = o.outline_id;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuildReport {
    pub floor: usize,
    pub planes: usize,
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
                carry_over(&old_auto, &mut planes);
                approximate = approx;
                built = planes.len();
                set.planes.extend(planes);
            }
            Err(e) if !clear_on_error => return Err(e),
            Err(_) => {}
        }
    }
    settings.signature = wall_signature(&project.floors[fi]);
    set.settings = Some(settings);
    store(project, fi, &mut set);
    Ok(BuildReport {
        floor: fi,
        planes: built,
        approximate,
    })
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

/// Delete Roof Planes (RF-40): removes every plane and the roof settings of
/// floor `fi` without rebuilding. Returns how many planes went.
pub fn delete_all(project: &mut Project, fi: usize) -> usize {
    let mut set = load(&project.floors[fi]);
    let n = set.planes.len();
    set.planes.clear();
    set.settings = None;
    store(project, fi, &mut set);
    n
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

/// The roof planes of the current floor on layer "Roof Planes": outline with
/// eaves heavy and ridge/hip/valley lines (edges two planes share) solid,
/// holes, skylights, and at each centroid the pitch label with a slope arrow
/// (pointing down the slope).
pub fn draw_roofs(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let set = load(cx.floor());
    if set.planes.is_empty() {
        return;
    }
    let edges_of = |r: &RoofPlaneRecord| -> Vec<(Point, Point)> {
        let p = r.plan_polygon();
        (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])).collect()
    };
    for (k, r) in set.planes.iter().enumerate() {
        if !cx.layers().is_visible(&r.layer) {
            continue;
        }
        let color = layer_color(cx, &r.layer);
        let poly = r.plan_polygon();
        let n = poly.len();
        let min_h = r
            .polygon3d
            .iter()
            .map(|v| v[1])
            .fold(f64::INFINITY, f64::min);
        for i in 0..n {
            let j = (i + 1) % n;
            let eave =
                (r.polygon3d[i][1] - min_h).abs() < 0.5 && (r.polygon3d[j][1] - min_h).abs() < 0.5;
            let shared = set.planes.iter().enumerate().any(|(o, other)| {
                o != k
                    && edges_of(other)
                        .iter()
                        .any(|e| same_edge(*e, (poly[i], poly[j])))
            });
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
        if let Some(h) = &r.hole {
            let pts: Vec<Pos2> = h.iter().map(|p| cam.world_to_screen(*p)).collect();
            painter.add(egui::Shape::closed_line(
                pts.clone(),
                Stroke::new(1.5_f32, color),
            ));
            if pts.len() >= 4 {
                painter.line_segment([pts[0], pts[2]], Stroke::new(1.0_f32, color));
                painter.line_segment([pts[1], pts[3]], Stroke::new(1.0_f32, color));
            }
        }
        let up = r.up_slope();
        let across = up.perp();
        for (c, w, l) in &r.skylights {
            let corner = |sw: f64, sl: f64| {
                cam.world_to_screen(
                    c.add(across.scale(sw * w * 0.5))
                        .add(up.scale(sl * l * 0.5)),
                )
            };
            let q = [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ];
            painter.add(egui::Shape::closed_line(
                q.to_vec(),
                Stroke::new(1.5_f32, color),
            ));
            painter.line_segment([q[0], q[2]], Stroke::new(1.0_f32, color));
            painter.line_segment([q[1], q[3]], Stroke::new(1.0_f32, color));
        }
        let c = r.centroid();
        let at = cam.world_to_screen(c);
        if !cam.rect.expand(40.0).contains(at) {
            continue;
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
        let mut text = r.pitch_label();
        if !r.label.is_empty() {
            text = format!("{} {text}", r.label);
        }
        painter.text(
            at + egui::Vec2::new(0.0, -14.0),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(11.0),
            color,
        );
    }
}

// ===================================================================
// 3D meshes
// ===================================================================

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [a[0] / l, a[1] / l, a[2] / l]
    }
}

struct Tris {
    vertices: Vec<plan_3d::Vertex>,
    indices: Vec<u32>,
}

impl Tris {
    /// One triangle, wound so its front faces `toward` (a direction).
    fn tri(&mut self, a: [f64; 3], b: [f64; 3], c: [f64; 3], toward: [f64; 3]) {
        let mut n = cross3(sub3(b, a), sub3(c, a));
        let (mut b, mut c) = (b, c);
        if n[0] * toward[0] + n[1] * toward[1] + n[2] * toward[2] < 0.0 {
            std::mem::swap(&mut b, &mut c);
            n = [-n[0], -n[1], -n[2]];
        }
        let n = unit3(n);
        let base = self.vertices.len() as u32;
        for p in [a, b, c] {
            self.vertices.push(plan_3d::Vertex {
                position: [p[0] as f32, p[1] as f32, p[2] as f32],
                normal: [n[0] as f32, n[1] as f32, n[2] as f32],
                uv: [(p[0] / 12.0) as f32, (p[2] / 12.0) as f32],
            });
        }
        self.indices.extend([base, base + 1, base + 2]);
    }
}

/// One mesh per roof plane on every floor: the sloped top triangulated by ear
/// clipping, a 1" slab underneath (bottom face and edge walls), material
/// `Roof`. Holes are not cut yet. The 3D view builder appends these to the
/// scene of `plan_3d::build_scene`.
pub fn roof_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let mut out = Vec::new();
    for floor in &project.floors {
        for r in load(floor).planes {
            let plan = r.plan_polygon();
            if plan.len() < 3 {
                continue;
            }
            let tris = plan_3d::triangulate::ear_clip(&plan);
            let up = [0.0, 1.0, 0.0];
            let down = [0.0, -1.0, 0.0];
            let drop = |p: [f64; 3]| [p[0], p[1] - SLAB_THICKNESS, p[2]];
            let mut t = Tris {
                vertices: Vec::new(),
                indices: Vec::new(),
            };
            for [i, j, k] in &tris {
                let (a, b, c) = (r.polygon3d[*i], r.polygon3d[*j], r.polygon3d[*k]);
                t.tri(a, b, c, up);
                t.tri(drop(a), drop(b), drop(c), down);
            }
            let ccw = polygon_area(&plan) >= 0.0;
            let n = plan.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let (a, b) = (r.polygon3d[i], r.polygon3d[j]);
                // Outward in plan is to the right of a CCW edge: (dy, -dx),
                // which is (dy, 0, dx) in scene space.
                let (dx, dy) = (plan[j].x - plan[i].x, plan[j].y - plan[i].y);
                let s = if ccw { 1.0 } else { -1.0 };
                let out_dir = [dy * s, 0.0, dx * s];
                t.tri(a, b, drop(b), out_dir);
                t.tri(a, drop(b), drop(a), out_dir);
            }
            out.push(plan_3d::Mesh {
                vertices: t.vertices,
                indices: t.indices,
                material: plan_3d::Material::Roof,
                object_id: Some(r.id),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

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
        let d = plan_defaults::embedded();
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
        let d = plan_defaults::embedded();
        let mut p = rect_project(480.0, 288.0);
        rebuild(&mut p, 0, RoofSettings::from_defaults(&d), false).unwrap();
        let mut set = load(&p.floors[0]);
        set.planes[0].hole = Some(rect_polygon(Point::new(0.0, 0.0), Point::new(10.0, 10.0)));
        set.planes[0]
            .skylights
            .push((Point::new(5.0, 5.0), 24.0, 48.0));
        set.planes[1].label = "Main".into();
        store(&mut p, 0, &mut set);
        let text = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&text).unwrap();
        let again = load(&back.floors[0]);
        assert_eq!(again, load(&p.floors[0]));
        assert_eq!(again.planes.len(), 4);
        assert_eq!(again.planes[1].label, "Main");
        assert_eq!(again.planes[0].skylights.len(), 1);
        // The data layer is hidden, the outline polylines are on "Roof Planes".
        assert!(!back.layers.is_visible(LAYER_DATA));
        let outlines = back.floors[0]
            .cad
            .iter()
            .filter(|c| c.layer == LAYER_PLANES)
            .count();
        assert_eq!(outlines, 4);
        // Storing twice does not duplicate anything.
        let mut p2 = back;
        let mut s2 = load(&p2.floors[0]);
        store(&mut p2, 0, &mut s2);
        assert_eq!(p2.floors[0].cad.len(), 4 + 4 + 1);
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
        let d = plan_defaults::embedded();
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
    fn delete_all_clears_planes_and_settings() {
        let d = plan_defaults::embedded();
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
}
