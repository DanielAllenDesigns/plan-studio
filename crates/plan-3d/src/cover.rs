//! The roof as the walls see it: roof and ceiling planes above each floor,
//! the top profile they give a wall (gable triangles, walls clipped under
//! hips and sheds, interior walls rising to a vaulted ceiling), butting roofs
//! trimmed at a taller wall, and the attic walls generated above them.
//!
//! A [`RoofCover`] is plain input to the scene builder. [`RoofCover::from_project`]
//! reads it from the roof records the editor stores in `Floor.roofs`
//! (`"kind": "plane"` and `"kind": "ceiling"` entries); a caller that holds the
//! planes some other way passes them to [`RoofCover::new`].

use crate::clip::{
    lower_envelope, merge, profile_from_runs, trim_polygon3, upper_envelope, Piece, TopProfile,
};
use crate::eave::{EaveOverrides, EavePlane};
use crate::mesh::{Material, Mesh};
use crate::wall::build_panel;
use plan_core::defaults::{EaveCut, RoofDetailDefaults, RoofWallKind};
use plan_core::geometry::{dist_to_segment, point_in_polygon, segment_intersection};
use plan_core::{Floor, Id, Point, Project, Wall, WallKind};
use plan_roof::{CeilingPlane, RoofPlane};
use serde_json::Value;

/// Tolerance, inches.
const EPS: f64 = 1e-6;
/// A plane whose underside is not at least this far above a wall's bottom does
/// not shape that wall (a dormer wall stands on the surface).
const MIN_CLEARANCE: f64 = 1.0;
/// Walls this much taller than a whole plane butt it instead of being cut.
const BUTT_MARGIN: f64 = 1.0;
/// A wall must run this far through a plane to butt it, inches.
const BUTT_MIN_RUN: f64 = 6.0;
/// The plane stops this far short of the wall face, inches.
const TRIM_GAP: f64 = 0.5;
/// An attic wall is generated where the roof is this far below the wall.
const ATTIC_MIN_GAP: f64 = 0.5;

/// A roof plane under a wall's bottom must be this close to count as the
/// wall's footing, inches below the wall bottom.
const MAX_BOTTOM_DROP: f64 = 96.0;
/// A bottom cut never rises within this of the wall's top, inches.
const MIN_STANDING: f64 = 2.0;

/// Roof detail switches and sizes (defaults match Chief's stock roof; see
/// [`RoofDetailDefaults`], the Default Settings page they come from).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoofDetail {
    /// Roof structure plus surface, thick along the plane normal, inches.
    pub thickness: f64,
    /// How the end of the structure is cut at an eave.
    pub eave_cut: EaveCut,
    /// Fascia boards on the eaves.
    pub fascia: bool,
    /// Fascia board height, inches.
    pub fascia_height: f64,
    /// Fascia board thickness, inches.
    pub fascia_thickness: f64,
    /// Soffit under the eave and the rake.
    pub soffit: bool,
    /// The eave soffit follows the roof slope instead of lying level.
    pub sloped_soffit: bool,
    /// Rake fascia on gable ends.
    pub rake_fascia: bool,
    /// A frieze board on the wall between the plate and the roof underside.
    pub frieze: bool,
    /// Draw ridge and hip caps on planes with their Ridge Caps option on.
    pub ridge_caps: bool,
    /// Gutters along the eaves.
    pub gutters: bool,
    /// Gutter size, inches.
    pub gutter_size: f64,
    /// A flashing strip along the line where a lower roof butts a wall.
    pub flashing: bool,
    /// Exposed rafter tails under the eaves (instead of a soffit).
    pub rafter_tails: bool,
    /// Rafter spacing on center, width and depth, inches.
    pub rafter_spacing: f64,
    pub rafter_width: f64,
    pub rafter_depth: f64,
    /// Generate attic walls (Defaults: Auto Attic Walls).
    pub auto_attic_walls: bool,
    /// A roof plane under a wall cuts the wall's bottom (Roof Cuts Wall at
    /// Bottom).
    pub roof_cuts_wall_at_bottom: bool,
}

impl RoofDetail {
    /// The detail Default Settings > Roof Defaults describes.
    pub fn from_defaults(d: &RoofDetailDefaults) -> Self {
        Self {
            thickness: d.thickness,
            eave_cut: d.eave_cut,
            fascia: d.fascia,
            fascia_height: d.fascia_height,
            fascia_thickness: d.fascia_thickness,
            soffit: d.soffit,
            sloped_soffit: d.sloped_soffit,
            rake_fascia: d.rake_fascia,
            frieze: d.frieze,
            ridge_caps: d.ridge_caps,
            gutters: d.gutters,
            gutter_size: d.gutter_size,
            flashing: d.flashing,
            rafter_tails: d.rafter_tails,
            rafter_spacing: d.rafter_spacing,
            rafter_width: d.rafter_width,
            rafter_depth: d.rafter_depth,
            auto_attic_walls: d.auto_attic_walls,
            roof_cuts_wall_at_bottom: d.roof_cuts_wall_at_bottom,
        }
    }
}

impl Default for RoofDetail {
    fn default() -> Self {
        Self::from_defaults(&RoofDetailDefaults::default())
    }
}

/// Wall type names the roof detail adds (empty keeps a wall's own type).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoofTypes {
    /// Attic walls and the part of a wall above its plate.
    pub attic: String,
    /// The part of a wall below a butting roof.
    pub lower: String,
}

impl RoofTypes {
    /// The wall types of Default Settings > Roof Defaults.
    pub fn from_defaults(d: &RoofDetailDefaults) -> Self {
        Self {
            attic: d.attic_wall_type.clone(),
            lower: d.lower_wall_type.clone(),
        }
    }
}

/// Where a lower roof plane butts a tall wall: along the wall's face the
/// plane's surface runs from scene elevation `h0` at `s0` to `h1` at `s1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Butt {
    pub wall: Id,
    pub s0: f64,
    pub s1: f64,
    pub h0: f64,
    pub h1: f64,
}

/// A sloped surface over a plan outline, `elevation = c + gx*x + gy*y`.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    outline: Vec<Point>,
    c: f64,
    gx: f64,
    gy: f64,
    /// Also sample beside the wall's centerline (ceilings are outlined at the
    /// room's inner faces, so a partition's centerline lies outside them).
    wide: bool,
}

impl Surface {
    fn from_fn(outline: Vec<Point>, f: impl Fn(Point) -> f64, wide: bool) -> Self {
        let c = f(Point::ZERO);
        Self {
            outline,
            c,
            gx: f(Point::new(1.0, 0.0)) - c,
            gy: f(Point::new(0.0, 1.0)) - c,
            wide,
        }
    }

    /// The underside of a roof plane of the given thickness.
    pub fn roof_underside(plane: &RoofPlane, thickness: f64) -> Option<Self> {
        plane.underside_at(Point::ZERO, thickness)?;
        Some(Self::from_fn(
            plane.plan_polygon(),
            |p| plane.underside_at(p, thickness).unwrap_or(0.0),
            false,
        ))
    }

    /// The visible (top) surface of a roof plane.
    pub fn roof_top(plane: &RoofPlane) -> Option<Self> {
        plane.height_at(Point::ZERO)?;
        Some(Self::from_fn(
            plane.plan_polygon(),
            |p| plane.height_at(p).unwrap_or(0.0),
            false,
        ))
    }

    /// The visible surface of a ceiling plane.
    pub fn ceiling(plane: &CeilingPlane) -> Self {
        Self::from_fn(plane.outline.clone(), |p| plane.height_at(p), true)
    }

    /// Elevation at plan point `p`.
    pub fn height_at(&self, p: Point) -> f64 {
        self.c + self.gx * p.x + self.gy * p.y
    }

    /// The stretches `(s0, s1)` of the wall `a -> b` (`s` from `a`) under the
    /// surface's outline, merged.
    fn covered(&self, a: Point, b: Point, half: f64) -> Vec<(f64, f64)> {
        let len = a.dist(b);
        if len <= EPS || self.outline.len() < 3 {
            return Vec::new();
        }
        let n = b.sub(a).normalized().perp();
        let offsets: &[f64] = if self.wide {
            &[0.0, half + 0.75, -(half + 0.75)]
        } else {
            &[0.0]
        };
        let mut cuts = vec![0.0, len];
        for &off in offsets {
            let (p, q) = (a.add(n.scale(off)), b.add(n.scale(off)));
            let m = self.outline.len();
            for i in 0..m {
                if let Some((t, _)) =
                    segment_intersection(p, q, self.outline[i], self.outline[(i + 1) % m])
                {
                    cuts.push(t * len);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-7);
        let dir = b.sub(a).normalized();
        let mut out: Vec<(f64, f64)> = Vec::new();
        for w in cuts.windows(2) {
            let mid = a.add(dir.scale((w[0] + w[1]) * 0.5));
            let inside = offsets
                .iter()
                .any(|&off| point_in_polygon(mid.add(n.scale(off)), &self.outline));
            if !inside {
                continue;
            }
            match out.last_mut() {
                Some(last) if (last.1 - w[0]).abs() < 1e-7 => last.1 = w[1],
                _ => out.push((w[0], w[1])),
            }
        }
        out
    }
}

/// A wall the roof planes can butt: one rising above the plane.
struct TallWall<'a> {
    wall: &'a Wall,
    /// Scene elevation of the wall's bottom and top.
    bottom: f64,
    top: f64,
}

/// An attic wall: a panel on a wall's line, `(s, elevation)` polygon.
#[derive(Debug, Clone)]
pub struct AtticPanel {
    pub wall: Wall,
    pub polygon: Vec<(f64, f64)>,
}

/// A wall's bottom cut by roof planes below it: `profile` is the bottom
/// edge measured from `drop` inches under the wall's own bottom, so every
/// height is positive.
#[derive(Debug, Clone, PartialEq)]
pub struct BottomCut {
    /// How far the wall reaches below its bottom, inches.
    pub drop: f64,
    pub profile: TopProfile,
}

/// What a wall's material split follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitKind {
    /// The roof line of a butting plane: below it the lower wall type.
    Butt,
    /// The plate: above it the attic wall type.
    Plate,
}

/// The roof planes and ceiling planes above one floor.
#[derive(Debug, Clone, Default)]
pub struct FloorCover {
    /// Roof planes, trimmed where they butt a taller wall.
    pub eaves: Vec<EavePlane>,
    roofs: Vec<Surface>,
    /// The visible surface of every plane before it was trimmed: what a
    /// wall above can stand on.
    tops: Vec<Surface>,
    ceilings: Vec<Surface>,
    /// The detail this floor's roof is drawn with (its stored settings).
    pub detail: RoofDetail,
    /// Wall types the roof detail names.
    pub types: RoofTypes,
    /// Attic walls generated above lower roofs beside taller walls.
    pub attic: Vec<AtticPanel>,
    /// Footprint edges the Roof Plane Specification made gable ends: the
    /// walls along them rise to the roof like Full Gable walls.
    gable_edges: Vec<(Point, Point)>,
}

/// A surface shaping one line along a wall.
struct Cand<'a> {
    surface: &'a Surface,
    /// The surface sets the top (rather than only limiting it).
    drives: bool,
    /// Stretches of the line under the surface.
    cov: Vec<(f64, f64)>,
    /// Where the line starts (the wall's start, or shifted to a face).
    origin: Point,
}

/// Which way a wall's top is driven.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drive {
    /// Cut by the roof, never raised.
    Clip,
    /// Rises to the underside of the roof (gable, shed, knee walls).
    Roof,
    /// Rises to the ceiling planes (interior walls) and is cut by the roof.
    Ceiling,
}

fn drive_of(wall: &Wall) -> Drive {
    match wall.roof.kind {
        RoofWallKind::FullGable
        | RoofWallKind::DutchGable
        | RoofWallKind::HighShedGable
        | RoofWallKind::KneeWall => Drive::Roof,
        _ if wall.kind == WallKind::Interior => Drive::Ceiling,
        _ => Drive::Clip,
    }
}

impl FloorCover {
    /// Is there nothing above this floor?
    pub fn is_empty(&self) -> bool {
        self.roofs.is_empty() && self.ceilings.is_empty()
    }

    /// Does `wall` lie along a footprint edge made a gable end by the roof
    /// settings?
    fn on_gable_edge(&self, wall: &Wall) -> bool {
        let mid = Point::lerp(wall.start, wall.end, 0.5);
        let dir = wall.direction();
        self.gable_edges.iter().any(|&(a, b)| {
            dist_to_segment(mid, a, b) <= wall.thickness * 0.5 + 1.0
                && b.sub(a).normalized().cross(dir).abs() < 0.02
        })
    }

    /// The top of `wall` (standing at `bottom` scene elevation, `nominal`
    /// inches tall) where the roof and ceiling planes shape it, or `None` for
    /// a level top at `nominal`.
    ///
    /// Every wall is cut by the roof planes above it; a gable, high-shed or
    /// knee wall rises to the plane; an interior wall rises to the ceiling
    /// planes over it. Planes below the wall's bottom are ignored.
    pub fn wall_top(&self, wall: &Wall, bottom: f64, nominal: f64) -> Option<TopProfile> {
        let mut drive = drive_of(wall);
        if drive == Drive::Clip && wall.kind == WallKind::Exterior && self.on_gable_edge(wall) {
            drive = Drive::Roof;
        }
        self.wall_top_driven(wall, bottom, nominal, drive)
    }

    /// The top of a half, pony, foundation or curved-facet wall (RF-16): cut
    /// by the roof like a standard wall, but never raised to it (a half wall
    /// stays a half wall under the gable).
    pub fn wall_top_clipped(&self, wall: &Wall, bottom: f64, nominal: f64) -> Option<TopProfile> {
        self.wall_top_driven(wall, bottom, nominal, Drive::Clip)
    }

    fn wall_top_driven(
        &self,
        wall: &Wall,
        bottom: f64,
        nominal: f64,
        drive: Drive,
    ) -> Option<TopProfile> {
        let len = wall.length();
        if self.is_empty() || len <= EPS || nominal <= EPS {
            return None;
        }
        let half = wall.thickness * 0.5;
        let (a, b) = (wall.start, wall.end);
        let n = wall.normal();
        let mut cands: Vec<Cand> = Vec::new();
        // A roof plane cuts the wall's faces as well as its centerline: the
        // top is the lowest of the three, so no corner pokes through.
        let offsets: &[f64] = if half > EPS {
            &[0.0, 1.0, -1.0]
        } else {
            &[0.0]
        };
        for s in &self.roofs {
            for k in offsets {
                let shift = n.scale(k * half);
                let cov = s.covered(a.add(shift), b.add(shift), 0.0);
                if !cov.is_empty() {
                    cands.push(Cand {
                        surface: s,
                        drives: drive == Drive::Roof,
                        cov,
                        origin: a.add(shift),
                    });
                }
            }
        }
        if drive == Drive::Ceiling {
            for s in &self.ceilings {
                let cov = s.covered(a, b, half);
                if !cov.is_empty() {
                    cands.push(Cand {
                        surface: s,
                        drives: true,
                        cov,
                        origin: a,
                    });
                }
            }
        }
        if cands.is_empty() {
            return None;
        }
        let mut cuts = vec![0.0, len];
        for c in &cands {
            cuts.extend(c.cov.iter().flat_map(|c| [c.0, c.1]));
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-7);
        let dir = wall.direction();
        let height_at =
            |s: &Surface, o: Point, t: f64| (s.height_at(o.add(dir.scale(t))) - bottom).max(0.0);
        let mut pieces: Vec<Piece> = Vec::new();
        for w in cuts.windows(2) {
            let (x, y) = (w[0], w[1]);
            if y - x <= EPS {
                continue;
            }
            let mid = (x + y) * 0.5;
            let mut lines = Vec::new();
            let mut driven = false;
            for c in &cands {
                let (surf, drives, cov, origin) = (c.surface, c.drives, &c.cov, c.origin);
                if !cov.iter().any(|c| c.0 <= mid && mid <= c.1) {
                    continue;
                }
                let (hx, hy) = (height_at(surf, origin, x), height_at(surf, origin, y));
                if hx.max(hy) <= MIN_CLEARANCE {
                    continue;
                }
                driven |= drives;
                lines.push((hx, hy));
            }
            if !driven {
                lines.push((nominal, nominal));
            }
            pieces.extend(lower_envelope(x, y, &lines));
        }
        let top = TopProfile {
            pieces: merge(pieces),
        };
        (!top.pieces.is_empty() && !top.is_flat_at(nominal)).then_some(top)
    }
}

/// The roof of a whole project, floor by floor.
#[derive(Debug, Clone, Default)]
pub struct RoofCover {
    pub floors: Vec<FloorCover>,
    pub detail: RoofDetail,
    /// Where lower roof planes butt taller walls.
    pub butts: Vec<Butt>,
}

/// Roof planes and ceiling planes of one floor, as stored.
#[derive(Debug, Clone, Default)]
pub struct FloorRoofInput {
    pub planes: Vec<EavePlane>,
    pub ceilings: Vec<CeilingPlane>,
    /// Footprint edges that are gable ends by the roof settings.
    pub gable_edges: Vec<(Point, Point)>,
    /// The detail stored with the floor's roof settings; `None` follows the
    /// cover's.
    pub detail: Option<RoofDetail>,
    /// The wall types stored with the roof settings.
    pub types: Option<RoofTypes>,
}

fn field<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Option<T> {
    serde_json::from_value(v.get(key)?.clone()).ok()
}

/// Roof and ceiling planes in the `Floor.roofs` records of `floor`.
pub fn read_floor_roof(floor: &Floor) -> FloorRoofInput {
    let mut out = FloorRoofInput::default();
    for v in &floor.roofs {
        match v.get("kind").and_then(Value::as_str) {
            Some("plane") => {
                let (Some(poly), Some(base)) = (
                    field::<Vec<[f64; 3]>>(v, "polygon3d").filter(|p| p.len() >= 3),
                    field::<Vec<Point>>(v, "baseline").filter(|b| b.len() >= 2),
                ) else {
                    continue;
                };
                out.planes.push(EavePlane {
                    plane: RoofPlane {
                        polygon3d: poly,
                        pitch_in_12: field(v, "pitch").unwrap_or(0.0),
                        baseline: (base[0], base[1]),
                        source_edge: 0,
                    },
                    overhang: field(v, "overhang").unwrap_or(0.0),
                    ridge_caps: field(v, "ridge_caps").unwrap_or(false),
                    cuts: Vec::new(),
                    id: field(v, "id"),
                    opts: {
                        let mut o: EaveOverrides = field(v, "eave").unwrap_or_default();
                        if o.gutters.is_none() && field::<bool>(v, "gutters").unwrap_or(false) {
                            o.gutters = Some(true);
                        }
                        o
                    },
                    skip: Vec::new(),
                });
            }
            Some("ceiling") => {
                let (Some(outline), Some(base)) = (
                    field::<Vec<Point>>(v, "outline").filter(|o| o.len() >= 3),
                    field::<Vec<Point>>(v, "baseline").filter(|b| b.len() >= 2),
                ) else {
                    continue;
                };
                out.ceilings.push(CeilingPlane {
                    outline,
                    baseline: (base[0], base[1]),
                    pitch_in_12: field(v, "pitch").unwrap_or(0.0),
                    height_at_baseline: field(v, "height").unwrap_or(0.0),
                    thickness: field(v, "thickness").unwrap_or(0.0),
                });
            }
            Some("settings") => {
                if let Some(d) = field::<RoofDetailDefaults>(v, "detail") {
                    out.detail = Some(RoofDetail::from_defaults(&d));
                    out.types = Some(RoofTypes::from_defaults(&d));
                }
                for e in field::<Vec<Value>>(v, "edge_specs").unwrap_or_default() {
                    if field::<bool>(&e, "gable").unwrap_or(false) {
                        if let (Some(a), Some(b)) = (field(&e, "a"), field(&e, "b")) {
                            out.gable_edges.push((a, b));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

impl RoofCover {
    /// No roof at all: walls keep their level tops.
    pub fn none() -> Self {
        Self::default()
    }

    /// The cover stored in the project's roof records, with stock detail.
    pub fn from_project(project: &Project) -> Self {
        Self::from_project_with(project, RoofDetail::default())
    }

    /// [`RoofCover::from_project`] with explicit detail.
    pub fn from_project_with(project: &Project, detail: RoofDetail) -> Self {
        let input: Vec<FloorRoofInput> = project.floors.iter().map(read_floor_roof).collect();
        Self::new(project, input, detail)
    }

    /// The cover of `project` given each floor's planes (index-aligned with
    /// `project.floors`; missing floors have no roof). Roof planes are trimmed
    /// where they run into a wall that rises above them, and the attic walls
    /// that leaves are worked out.
    pub fn new(project: &Project, input: Vec<FloorRoofInput>, detail: RoofDetail) -> Self {
        let mut floors = Vec::with_capacity(project.floors.len());
        let mut butts = Vec::new();
        let mut input = input.into_iter();
        for fi in 0..project.floors.len() {
            let inp = input.next().unwrap_or_default();
            if inp.planes.is_empty() && inp.ceilings.is_empty() {
                floors.push(FloorCover::default());
                continue;
            }
            let floor_detail = inp.detail.unwrap_or(detail);
            let tall = tall_walls(project, fi);
            let mut eaves = Vec::new();
            let mut attic = Vec::new();
            let mut tops = Vec::new();
            for mut plane in inp.planes {
                tops.extend(Surface::roof_top(&plane.plane));
                if trim_plane(&mut plane, &tall, &mut attic, &mut butts) {
                    eaves.push(plane);
                }
            }
            let roofs = eaves
                .iter()
                .filter_map(|e| Surface::roof_underside(&e.plane, floor_detail.thickness))
                .collect();
            let ceilings = inp.ceilings.iter().map(Surface::ceiling).collect();
            floors.push(FloorCover {
                eaves,
                roofs,
                tops,
                ceilings,
                detail: floor_detail,
                types: inp.types.unwrap_or_default(),
                attic,
                gable_edges: inp.gable_edges,
            });
        }
        Self {
            floors,
            detail,
            butts,
        }
    }

    /// The cover above floor `fi`.
    pub fn floor(&self, fi: usize) -> Option<&FloorCover> {
        self.floors.get(fi)
    }

    /// Meshes of the generated attic walls ([`Material::WallExterior`], no
    /// object id: they are not in the model). Floors whose
    /// [`RoofDetail::auto_attic_walls`] is off make none.
    pub fn attic_meshes(&self) -> Vec<Mesh> {
        self.attic_meshes_typed(&|_| None)
    }

    /// [`RoofCover::attic_meshes`] where the attic wall type of the floor's
    /// roof detail maps to a material through `material_of` (a type name to
    /// the material of its exterior layer).
    pub fn attic_meshes_typed(&self, material_of: &dyn Fn(&str) -> Option<Material>) -> Vec<Mesh> {
        self.floors
            .iter()
            .filter(|f| f.detail.auto_attic_walls)
            .flat_map(|f| {
                let material = if f.types.attic.is_empty() {
                    None
                } else {
                    material_of(&f.types.attic)
                }
                .unwrap_or(Material::WallExterior);
                f.attic
                    .iter()
                    .flat_map(move |p| build_panel(&p.wall, &p.polygon, material))
            })
            .collect()
    }

    /// Soffit, fascia, rake boards, frieze, ridge caps, rafter tails, gutters
    /// and flashing of every roof plane (see [`crate::eave_detail_meshes`]).
    pub fn eave_meshes(&self) -> Vec<Mesh> {
        self.floors
            .iter()
            .flat_map(|f| crate::eave::eave_detail_meshes(&f.eaves, &f.detail))
            .collect()
    }

    /// The wall types the roof detail names (those of the first floor
    /// whose roof settings name any).
    pub fn types(&self) -> RoofTypes {
        self.floors
            .iter()
            .map(|f| &f.types)
            .find(|t| **t != RoofTypes::default())
            .cloned()
            .unwrap_or_default()
    }

    /// How the roof planes below floor `fi` cut the bottom of `wall` (Roof
    /// Cuts Wall at Bottom). `bottom` is the wall's scene elevation,
    /// `nominal` its height.
    pub fn wall_bottom(
        &self,
        fi: usize,
        wall: &Wall,
        bottom: f64,
        nominal: f64,
    ) -> Option<BottomCut> {
        bottom_cut(self.floors.get(..fi)?, wall, bottom, nominal)
    }

    /// Where the exterior face of `wall` changes wall type: along the roof
    /// line of a plane that butts it ([`SplitKind::Butt`]), else at the
    /// plate when the wall rises above it ([`SplitKind::Plate`]). Heights are
    /// above the wall's bottom (scene elevation `bottom`); `top` is the
    /// wall's top profile.
    pub fn wall_split(
        &self,
        wall: &Wall,
        bottom: f64,
        top: Option<&TopProfile>,
    ) -> Option<(TopProfile, SplitKind)> {
        let len = wall.length();
        if len <= EPS {
            return None;
        }
        let runs: Vec<_> = self
            .butts
            .iter()
            .filter(|b| b.wall == wall.id)
            .map(|b| (b.s0, b.s1, b.h0 - bottom, b.h1 - bottom))
            .collect();
        if !runs.is_empty() {
            return Some((profile_from_runs(len, &runs), SplitKind::Butt));
        }
        let above = top.is_some_and(|t| t.max_height() > wall.height + 0.5);
        above.then(|| (TopProfile::flat(len, wall.height), SplitKind::Plate))
    }
}

/// [`RoofCover::wall_bottom`] over the covers of the floors under the wall.
pub fn bottom_cut(
    below: &[FloorCover],
    wall: &Wall,
    bottom: f64,
    nominal: f64,
) -> Option<BottomCut> {
    let len = wall.length();
    if len <= EPS || nominal <= MIN_STANDING {
        return None;
    }
    let (a, b) = (wall.start, wall.end);
    // The wall reaches down to a roof below its bottom only where no attic
    // wall fills that gap already.
    type Cand<'a> = (&'a Surface, Vec<(f64, f64)>, bool);
    let cands: Vec<Cand> = below
        .iter()
        .filter(|f| f.detail.roof_cuts_wall_at_bottom)
        .flat_map(|f| f.tops.iter().map(move |s| (s, !f.detail.auto_attic_walls)))
        .filter_map(|(s, reach)| {
            let cov = s.covered(a, b, 0.0);
            (!cov.is_empty()).then_some((s, cov, reach))
        })
        .collect();
    if cands.is_empty() {
        return None;
    }
    let mut cuts = vec![0.0, len];
    for (_, cov, _) in &cands {
        cuts.extend(cov.iter().flat_map(|c| [c.0, c.1]));
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-7);
    let dir = wall.direction();
    let rel = |s: &Surface, t: f64| s.height_at(a.add(dir.scale(t))) - bottom;
    let mut pieces: Vec<Piece> = Vec::new();
    for w in cuts.windows(2) {
        let (x, y) = (w[0], w[1]);
        if y - x <= EPS {
            continue;
        }
        let mid = (x + y) * 0.5;
        let mut lines: Vec<(f64, f64)> = Vec::new();
        for (s, cov, reach) in &cands {
            if !cov.iter().any(|c| c.0 <= mid && mid <= c.1) {
                continue;
            }
            let l = (rel(s, x), rel(s, y));
            // A roof far below the wall is not its footing.
            if l.0.max(l.1) >= -MAX_BOTTOM_DROP {
                lines.push(l);
            }
            if !reach {
                lines.push((0.0, 0.0));
            }
        }
        if lines.is_empty() {
            lines.push((0.0, 0.0));
        }
        pieces.extend(upper_envelope(x, y, &lines));
    }
    let profile = TopProfile {
        pieces: merge(pieces),
    }
    .clamped(nominal - MIN_STANDING);
    if profile.pieces.is_empty() {
        return None;
    }
    let drop = (-profile.min_height()).clamp(0.0, MAX_BOTTOM_DROP);
    if drop <= EPS
        && profile
            .pieces
            .iter()
            .all(|p| p.h0.abs() < 1e-4 && p.h1.abs() < 1e-4)
    {
        return None;
    }
    Some(BottomCut {
        drop,
        profile: profile.raised(drop),
    })
}

/// Walls of floors `fi` and above that can rise through a plane.
fn tall_walls(project: &Project, fi: usize) -> Vec<TallWall<'_>> {
    project.floors[fi..]
        .iter()
        .flat_map(|floor| {
            floor
                .walls
                .iter()
                .filter(|w| {
                    !w.flags.invisible
                        && !w.flags.railing
                        && w.class.is_standard()
                        && !w.is_curved()
                        && w.height > EPS
                        && w.length() > EPS
                })
                .map(move |w| TallWall {
                    wall: w,
                    bottom: floor.elevation + w.bottom_offset,
                    top: floor.elevation + w.bottom_offset + w.height,
                })
        })
        .collect()
}

/// Trims `plane` at every wall that rises above it and records the attic
/// walls that opens. Returns `false` when nothing of the plane is left.
fn trim_plane(
    plane: &mut EavePlane,
    tall: &[TallWall],
    attic: &mut Vec<AtticPanel>,
    butts: &mut Vec<Butt>,
) -> bool {
    let max_y = plane
        .plane
        .polygon3d
        .iter()
        .fold(f64::NEG_INFINITY, |m, p| m.max(p[1]));
    let start = plane
        .plane
        .baseline
        .0
        .add(plane.plane.baseline.1)
        .scale(0.5);
    for t in tall {
        if t.top <= max_y + BUTT_MARGIN {
            continue;
        }
        let w = t.wall;
        let poly = Surface {
            outline: plane.plane.plan_polygon(),
            c: 0.0,
            gx: 0.0,
            gy: 0.0,
            wide: false,
        };
        let run: f64 = poly
            .covered(w.start, w.end, 0.0)
            .iter()
            .map(|c| c.1 - c.0)
            .sum();
        if run < BUTT_MIN_RUN {
            continue;
        }
        // Keep the side the eave is on.
        let n = w.normal();
        let side = n.dot(start.sub(w.start));
        let half = w.thickness * 0.5;
        if side.abs() <= half + 0.5 {
            continue;
        }
        let keep = n.scale(side.signum());
        // Attic wall: where the roof along the wall's face is below its bottom.
        let face_a = w.start.add(keep.scale(half));
        let face_b = w.end.add(keep.scale(half));
        for (s0, s1) in poly.covered(face_a, face_b, 0.0) {
            let dir = w.direction();
            let h = |s: f64| {
                plane
                    .plane
                    .height_at(face_a.add(dir.scale(s)))
                    .unwrap_or(t.bottom)
            };
            let (h0, h1) = (h(s0), h(s1));
            butts.push(Butt {
                wall: w.id,
                s0,
                s1,
                h0,
                h1,
            });
            if h0.min(h1) >= t.bottom - ATTIC_MIN_GAP {
                continue;
            }
            let ceil = t.bottom.max(h0).max(h1) + 1.0;
            let region = [(s0, h0), (s1, h1), (s1, ceil), (s0, ceil)];
            // Keep the part at or below the wall's bottom.
            let piece = crate::clip::clip_half_plane(&region, 0.0, -1.0, t.bottom);
            if piece.len() >= 3 && crate::clip::area(&piece).abs() > 1.0 {
                attic.push(AtticPanel {
                    wall: w.clone(),
                    polygon: piece,
                });
            }
        }
        let origin = (w.start.x, w.start.y);
        let (kept, cut) = trim_polygon3(
            &plane.plane.polygon3d,
            origin,
            (keep.x, keep.y),
            // The wall's face on the eave side, less a hair so the wall's own
            // face line is not under the plane.
            half + TRIM_GAP,
        );
        if kept.len() < 3 {
            return false;
        }
        plane.plane.polygon3d = kept;
        if let Some(c) = cut {
            plane.cuts.push(c);
        }
    }
    true
}
