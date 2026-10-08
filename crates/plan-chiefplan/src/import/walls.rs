//! Walls (class 6).
//!
//! A wall is a class 6 object that owns its parts as child objects:
//!
//! | Child | Meaning | Confidence |
//! |---|---|---|
//! | class 31 (straight) | the reference line: `f64` x, y, dx, dy, length at `+0x32/+0x3a/+0x42/+0x4a/+0x52` | High |
//! | class 40 (curved) | the same chord fields, then the arc: centre x, y at `+0x60/+0x68`, radius `+0x70`, angle at the start point `+0x78` and at the end point `+0x80` (radians) | Medium (one wall: a quarter circle of radius 24.27" matching the project PDF) |
//! | class 4 (24 bytes) | a point `f64` x, y at `+8/+16`: wall corners and break points | Medium |
//! | class 215 | a full copy of the wall type, only for the first wall that uses it | High |
//! | class 9 / 10 | doors / windows hosted in the wall (see `openings`) | High |
//! | class 120 | material list components of the wall's parts | High (class), none (contents) |
//! | class 68, 79 | framing members and framing assemblies | Medium |
//! | class 81 | connection records (two object ids and a GUID) | Low |
//!
//! The wall type is referenced by a serial id: somewhere in the wall's own
//! bytes (outside its children) sits `01 <u32 id> 00 x 12`, and the id is the
//! serial id that precedes a class 215 object (`01 <u32 id> 00 00 00 00` right
//! before its `CD`). All 63 walls of the first floor of one project resolve
//! to a type this way, 65 of 65 on its foundation, and the thicknesses match
//! the outlines in the project's PDF (6.5" `Interior-6`, 16.5"
//! `Brick-6_2, dbl`).
//!
//! The top of the wall is stored as an absolute elevation twice in a row
//! (start and end of a sloped top) a few hundred bytes after the line child:
//! 121.125 on a first floor with a 10' ceiling, 247.0 (= 137.875 + 109.125) on
//! the second floor. Foundation walls store no such pair, and attic floors
//! store values below their own elevation; both fall back to the floor's
//! ceiling value.
//!
//! The reference line is the main layer's inside face plus 0.25" for
//! exterior walls (the main layer lies to the right of the line, the outer
//! layers further right), and the centre of the main layer for symmetric
//! walls. [`plan_wall`] turns that into a centreline and an exterior side.

use super::floors::{ChiefFloor, WALL_TYPE};
use super::tree::{cstring_at, f64_at, fin, gaps, on_grid, u32_at, ObjectTree};
use crate::bridge::wall_type_def;
use crate::decode::{decode_materials, decode_wall_types, RawObject, TemplateWallType};
use plan_core::geometry::Point;
use plan_core::model::{Wall, WallKind};
use plan_core::walls::{FenceStyle, Side, WallClass, WallCurve};
use std::collections::HashMap;

/// Wall class id.
pub const WALL: u8 = 6;
/// Offset of the line fields inside a line object.
const LINE_X: usize = 0x32;
/// Bytes of a wall scanned for the top elevation, after its line child.
const TOP_WINDOW: usize = 0x500;

/// One decoded wall, in Chief's plan coordinates (inches).
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefWall {
    pub node: usize,
    /// Start and end of the reference line.
    pub start: (f64, f64),
    pub end: (f64, f64),
    /// Serial id of the wall type, when found.
    pub type_id: Option<u32>,
    /// Absolute elevation of the top, when found.
    pub top: Option<f64>,
    /// No line child: the wall was reduced to its first and last point.
    pub approximated: bool,
    /// Signed sagitta of a curved wall (positive toward the left of
    /// start-to-end), from a class 40 line object.
    pub bulge: Option<f64>,
}

/// The wall types a file defines, with their serial ids.
#[derive(Debug, Clone, Default)]
pub struct WallTypes {
    /// Every decoded type, in file order (names may repeat across copies).
    pub types: Vec<TemplateWallType>,
    /// Serial id -> index into `types`.
    pub by_id: HashMap<u32, usize>,
}

impl WallTypes {
    pub fn get(&self, id: u32) -> Option<&TemplateWallType> {
        self.by_id.get(&id).map(|&i| &self.types[i])
    }

    pub fn by_name(&self, name: &str) -> Option<&TemplateWallType> {
        self.types.iter().find(|t| t.name == name)
    }
}

fn raw_of(bytes: &[u8], tree: &ObjectTree, i: usize) -> RawObject {
    let n = tree.node(i);
    RawObject {
        class: n.class,
        version: n.version,
        flag: bytes.get(n.marker.wrapping_sub(1)).copied().unwrap_or(0),
        marker: n.marker,
        size_pos: n.marker + 4,
        end: n.end,
    }
}

/// Bytes per layer record of a class 215 object saved by X17 (518 in X18).
const X17_STRIDE: usize = 377;
/// Bytes after the records of an X17 wall type object.
const X17_TAIL: usize = 51;

/// X17 wall types store, per layer, the distance from the exterior face to
/// the layer's *start* at `+0` (a cumulative value: 0, 4.0, 5.0, 5.01, 5.51,
/// 11.01 for `Brick-6`) and the material id at `+8`; the main flag sits at
/// `+0x15` as in X18. The last record only holds the total, and some types
/// carry a zero-thickness layer (material 88) at either end, which is
/// dropped. The object is recognised by `size = 8 + string + count * 377 +
/// 51` (checked on every class 215 object of one X17 project).
fn decode_wall_type_x17(
    b: &[u8],
    r: &RawObject,
    materials: &[crate::decode::TemplateMaterial],
) -> Option<TemplateWallType> {
    let p = r.marker + 8;
    let (name, name_end) = cstring_at(b, p, 160)?;
    let count = u32_at(b, name_end)? as usize;
    let first = name_end + 4;
    if !(2..=40).contains(&count) || r.end != first + count * X17_STRIDE + X17_TAIL {
        return None;
    }
    let pos: Vec<f64> = (0..count)
        .map(|k| fin(b, first + k * X17_STRIDE))
        .collect::<Option<_>>()?;
    let mut details = Vec::new();
    for k in 0..count - 1 {
        let at = first + k * X17_STRIDE;
        let t = pos[k + 1] - pos[k];
        if !(0.0..=1000.0).contains(&t) {
            return None;
        }
        if t < 1e-3 {
            continue;
        }
        let id = u32_at(b, at + 8)?;
        let material = materials
            .iter()
            .find(|m| m.id == id && !m.name.is_empty())
            .map_or_else(|| format!("material #{id}"), |m| m.name.clone());
        details.push(crate::decode::TemplateWallLayer {
            material,
            material_id: id,
            thickness_in: (t * 1e6).round() / 1e6,
            is_main: *b.get(at + 0x15)? == 1,
            is_framing: *b.get(at + 0x16)? == 1,
            is_gap: false,
            spacing_in: 0.0,
        });
    }
    let total = details.iter().map(|l| l.thickness_in).sum::<f64>();
    Some(TemplateWallType {
        name,
        layers: details
            .iter()
            .map(|l| (l.material.clone(), l.thickness_in, l.is_main))
            .collect(),
        layer_details: details,
        total_thickness_in: (total * 1e6).round() / 1e6,
        offset: r.marker as u64,
    })
}

/// Collects every class 215 object, decodes it (X18 layout, then X17) and
/// reads its serial id.
pub fn collect_wall_types(bytes: &[u8], tree: &ObjectTree) -> WallTypes {
    let mats: Vec<RawObject> = tree
        .of_kind(57, 0)
        .map(|i| raw_of(bytes, tree, i))
        .collect();
    let materials = decode_materials(bytes, &mats);
    let defs: Vec<RawObject> = tree
        .of_kind(WALL_TYPE, 0)
        .map(|i| raw_of(bytes, tree, i))
        .collect();
    let mut types = decode_wall_types(bytes, &defs, &materials);
    let known: std::collections::HashSet<u64> = types.iter().map(|t| t.offset).collect();
    for r in &defs {
        if !known.contains(&(r.marker as u64)) {
            if let Some(t) = decode_wall_type_x17(bytes, r, &materials) {
                types.push(t);
            }
        }
    }
    let mut by_id = HashMap::new();
    for (k, t) in types.iter().enumerate() {
        let m = t.offset as usize;
        if m >= 9 && bytes[m - 9] == 1 && bytes[m - 4..m] == [0, 0, 0, 0] {
            if let Some(id) = u32_at(bytes, m - 8) {
                by_id.entry(id).or_insert(k);
            }
        }
    }
    WallTypes { types, by_id }
}

fn read_line(b: &[u8], marker: usize) -> Option<((f64, f64), (f64, f64))> {
    let x = fin(b, marker + LINE_X)?;
    let y = fin(b, marker + LINE_X + 8)?;
    let dx = fin(b, marker + LINE_X + 16)?;
    let dy = fin(b, marker + LINE_X + 24)?;
    let len = fin(b, marker + LINE_X + 32)?;
    let unit = (dx * dx + dy * dy).sqrt();
    if (unit - 1.0).abs() > 1e-3 || !(0.01..=100_000.0).contains(&len) {
        return None;
    }
    if x.abs() > 1.0e6 || y.abs() > 1.0e6 {
        return None;
    }
    Some(((x, y), (x + dx * len, y + dy * len)))
}

/// The signed sagitta of the arc stored after the chord fields of a class 40
/// object: centre (`+0x60`, `+0x68`), radius `+0x70`, angles at the start and
/// end points `+0x78`, `+0x80`. The arc is taken as the minor one (sweep
/// under 180 degrees), which is what the one curved wall seen is.
fn read_arc(b: &[u8], marker: usize, start: (f64, f64), end: (f64, f64)) -> Option<f64> {
    let cx = fin(b, marker + 0x60)?;
    let cy = fin(b, marker + 0x68)?;
    let r = fin(b, marker + 0x70)?;
    let a0 = fin(b, marker + 0x78)?;
    let a1 = fin(b, marker + 0x80)?;
    let chord = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt();
    if !(0.5..=100_000.0).contains(&r) || r < chord / 2.0 - 1e-6 || chord < 1e-6 {
        return None;
    }
    // The stored centre must be at distance r from both ends.
    let near = |p: (f64, f64)| {
        (((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() - r).abs() < 0.05 * r.max(1.0)
    };
    if !near(start) || !near(end) {
        return None;
    }
    let sagitta = r - (r * r - chord * chord / 4.0).max(0.0).sqrt();
    // Left normal of the chord; the apex lies opposite the centre.
    let (nx, ny) = (-(end.1 - start.1) / chord, (end.0 - start.0) / chord);
    let mid = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    let side = (cx - mid.0) * nx + (cy - mid.1) * ny;
    let sign = if side.abs() > 1e-6 {
        if side < 0.0 {
            1.0
        } else {
            -1.0
        }
    } else {
        // A semicircle: a clockwise sweep (angle falls) bulges left.
        let mut sweep = a1 - a0;
        while sweep > std::f64::consts::PI {
            sweep -= 2.0 * std::f64::consts::PI;
        }
        while sweep <= -std::f64::consts::PI {
            sweep += 2.0 * std::f64::consts::PI;
        }
        if sweep < 0.0 {
            1.0
        } else {
            -1.0
        }
    };
    (sagitta > 1e-6).then_some(sign * sagitta)
}

fn read_point(b: &[u8], marker: usize) -> Option<(f64, f64)> {
    let x = fin(b, marker + 8)?;
    let y = fin(b, marker + 16)?;
    (x.abs() < 1.0e6 && y.abs() < 1.0e6).then_some((x, y))
}

/// Finds the wall type id in the wall's own bytes.
fn find_type_id(bytes: &[u8], tree: &ObjectTree, node: usize, types: &WallTypes) -> Option<u32> {
    let n = tree.node(node);
    let spans = tree.child_spans(node);
    for (a, e) in gaps(n.marker, n.end, &spans) {
        let mut pos = a;
        while pos + 17 <= e {
            if bytes[pos] == 1 && bytes[pos + 5..pos + 17].iter().all(|&c| c == 0) {
                if let Some(id) = u32_at(bytes, pos + 1) {
                    if types.by_id.contains_key(&id) {
                        return Some(id);
                    }
                }
            }
            pos += 1;
        }
    }
    None
}

/// The absolute top elevation: the largest value stored twice in a row in the
/// window after the line child.
fn find_top(bytes: &[u8], tree: &ObjectTree, node: usize, after: usize) -> Option<f64> {
    let n = tree.node(node);
    let stop = tree
        .children(node)
        .iter()
        .map(|&c| tree.node(c))
        .find(|c| c.marker > after && c.class == 120)
        .map_or(n.end, |c| c.marker);
    let end = stop.min(after + TOP_WINDOW).min(n.end);
    let mut best: Option<f64> = None;
    let mut o = after;
    while o + 16 <= end {
        if let (Some(a), Some(b)) = (f64_at(bytes, o), f64_at(bytes, o + 8)) {
            if a.is_finite() && a == b && (24.0..=600.0).contains(&a) && on_grid(a, 8.0) {
                best = Some(best.map_or(a, |m: f64| m.max(a)));
            }
        }
        o += 1;
    }
    best
}

/// Decodes the wall at tree index `node`.
pub fn decode_wall(
    bytes: &[u8],
    tree: &ObjectTree,
    node: usize,
    types: &WallTypes,
) -> Option<ChiefWall> {
    let line_child = tree.first_child_of(node, &[31, 40]);
    let mut bulge = None;
    let (start, end, after, approximated) = match line_child.and_then(|c| {
        let m = tree.node(c).marker;
        read_line(bytes, m).map(|l| (l, tree.node(c).end, c))
    }) {
        Some(((s, e), after, c)) => {
            if tree.node(c).class == 40 {
                bulge = read_arc(bytes, tree.node(c).marker, s, e);
            }
            (s, e, after, false)
        }
        None => {
            let pts: Vec<(f64, f64)> = tree
                .children(node)
                .iter()
                .filter(|&&c| tree.node(c).class == 4)
                .filter_map(|&c| read_point(bytes, tree.node(c).marker))
                .collect();
            if pts.len() < 2 || pts.first() == pts.last() {
                return None;
            }
            let after = tree.node(node).marker + 0x1c9;
            (pts[0], *pts.last()?, after, true)
        }
    };
    Some(ChiefWall {
        node,
        start,
        end,
        type_id: find_type_id(bytes, tree, node, types),
        top: find_top(bytes, tree, node, after),
        approximated,
        bulge,
    })
}

/// How a wall type lays out around its main layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stack {
    pub total: f64,
    /// Thickness outside the main layer(s) (layers are listed exterior first).
    pub outside: f64,
    /// Thickness of the main layer span.
    pub main: f64,
    /// Thickness inside the main layer(s).
    pub inside: f64,
}

impl Stack {
    pub fn of(t: &TemplateWallType) -> Option<Stack> {
        let total: f64 = t.layers.iter().map(|l| l.1).sum();
        if total <= 0.0 {
            return None;
        }
        let first = t.layers.iter().position(|l| l.2);
        let last = t.layers.iter().rposition(|l| l.2);
        let (outside, main, inside) = match (first, last) {
            (Some(f), Some(l)) => {
                let outside: f64 = t.layers[..f].iter().map(|x| x.1).sum();
                let main: f64 = t.layers[f..=l].iter().map(|x| x.1).sum();
                (outside, main, total - outside - main)
            }
            _ => (0.0, total, 0.0),
        };
        Some(Stack {
            total,
            outside,
            main,
            inside,
        })
    }

    pub fn symmetric(&self) -> bool {
        (self.outside - self.inside).abs() < 0.02
    }

    /// Distance from the reference line to the centre of the whole wall, to
    /// the right of the line (positive) for an asymmetric wall; zero for a
    /// symmetric one.
    pub fn centre_shift_right(&self) -> f64 {
        if self.symmetric() {
            0.0
        } else {
            (self.main / 2.0 - REF_INSET) + (self.outside - self.inside) / 2.0
        }
    }
}

/// Distance of the reference line inside the main layer's inside face on
/// exterior walls, inches (measured from the project PDF outlines).
pub const REF_INSET: f64 = 0.25;

/// What a wall type name implies about the wall.
#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    pub class: WallClass,
    pub layer: &'static str,
    pub invisible: bool,
}

/// Wall class and layer from the type name and whether the wall stands on
/// the foundation floor (Medium confidence: names only).
pub fn role_of(type_name: &str, on_foundation_floor: bool) -> Role {
    let n = type_name.to_lowercase();
    let standard = |layer| Role {
        class: WallClass::Standard,
        layer,
        invisible: false,
    };
    if n.contains("room divider") {
        return Role {
            class: WallClass::RoomDivider,
            layer: "Walls, Invisible",
            invisible: true,
        };
    }
    if n.contains("railing") && n.contains("deck") {
        return Role {
            class: WallClass::DeckRailing,
            layer: "Walls, Railings",
            invisible: false,
        };
    }
    if n.contains("fence") || n.contains("fencing") {
        return Role {
            class: WallClass::Fencing {
                style: FenceStyle::Picket,
            },
            layer: "Walls, Railings",
            invisible: false,
        };
    }
    if n.contains("railing") {
        return Role {
            class: WallClass::Railing,
            layer: "Walls, Railings",
            invisible: false,
        };
    }
    if n.starts_with("glass") && !n.contains("block") {
        return Role {
            class: WallClass::Glass,
            layer: "Walls, Normal",
            invisible: false,
        };
    }
    if on_foundation_floor
        && [
            "stem wall",
            "footing",
            "foundation",
            "cmu",
            "concrete",
            "icf",
        ]
        .iter()
        .any(|k| n.contains(k))
    {
        return Role {
            class: WallClass::Foundation,
            layer: "Walls, Foundation",
            invisible: false,
        };
    }
    standard("Walls, Normal")
}

/// Converts a decoded wall into a Plan Studio wall. Returns the wall and the
/// resolved type name (when known).
pub fn plan_wall(
    w: &ChiefWall,
    floor: &ChiefFloor,
    floor_index: usize,
    types: &WallTypes,
) -> (Wall, Option<String>) {
    let ty = w.type_id.and_then(|id| types.get(id));
    let stack = ty.and_then(Stack::of);
    let (sx, sy) = w.start;
    let (ex, ey) = w.end;
    let len = ((ex - sx).powi(2) + (ey - sy).powi(2)).sqrt().max(1e-9);
    let (ux, uy) = ((ex - sx) / len, (ey - sy) / len);
    // Left normal is (-uy, ux); the main layer of an asymmetric wall lies to
    // the right.
    let shift = stack.map_or(0.0, |s| s.centre_shift_right());
    let (cx, cy) = (uy * shift, -ux * shift);
    let thickness = stack.map_or(5.5, |s| s.total);
    let asymmetric = stack.is_some_and(|s| !s.symmetric());
    let height = match w.top {
        Some(top) if (24.0..=300.0).contains(&(top - floor.elevation)) => top - floor.elevation,
        _ => floor.ceiling,
    };
    let def_kind = ty.map(|t| wall_type_def(t).kind);
    let kind = if asymmetric {
        WallKind::Exterior
    } else {
        def_kind.unwrap_or(WallKind::Interior)
    };
    let mut wall = Wall::new(
        Point::new(sx + cx, sy + cy),
        Point::new(ex + cx, ey + cy),
        thickness,
        height,
        kind,
    );
    let role = ty.map_or_else(
        || role_of("", floor_index == 0),
        |t| role_of(&t.name, floor_index == 0 && floor.elevation < -1.0),
    );
    wall.layer = role.layer.to_string();
    wall.wall_type = ty.map(|t| t.name.clone());
    if asymmetric {
        wall.exterior_side = Side::Right;
    }
    if let Some(b) = w.bulge {
        wall.curve = Some(WallCurve { bulge: b });
    }
    if !role.class.is_standard() {
        wall.set_class(role.class);
    }
    if role.invisible {
        wall.flags.invisible = true;
    }
    (wall, ty.map(|t| t.name.clone()))
}

/// Largest distance a wall end may move to meet another wall, inches.
const MAX_SNAP: f64 = 12.0;

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Intersection of the infinite lines `p + t u` and `q + s v`.
fn line_intersection(p: Point, u: Point, q: Point, v: Point) -> Option<Point> {
    let d = cross(u, v);
    if d.abs() < 1e-9 {
        return None;
    }
    let t = cross(q.sub(p), v) / d;
    Some(p.add(u.scale(t)))
}

/// Moves one end of `walls[i]` to `to`, keeping the openings' distances from
/// the wall start valid (a start that moves along the wall shifts them).
fn move_end(
    walls: &mut [Wall],
    openings: &mut [plan_core::model::Opening],
    i: usize,
    start: bool,
    to: Point,
) {
    let id = walls[i].id;
    if start {
        let dir = walls[i].direction();
        let delta = to.sub(walls[i].start);
        let along = delta.x * dir.x + delta.y * dir.y;
        walls[i].start = to;
        for o in openings.iter_mut().filter(|o| o.wall_id == id) {
            o.center_offset -= along;
        }
    } else {
        walls[i].end = to;
    }
}

/// Whether moving one end of `w` to `to` keeps the wall at least an inch long
/// and pointing the same way (a pilaster stub must not collapse or flip).
fn can_move(w: &Wall, start: bool, to: Point) -> bool {
    let (fixed, old_moving) = if start {
        (w.end, w.start)
    } else {
        (w.start, w.end)
    };
    let new = to.sub(fixed);
    let old = old_moving.sub(fixed);
    new.x * old.x + new.y * old.y > 0.0 && new.x.hypot(new.y) >= 1.0
}

/// Reconnects the walls of one floor after their reference lines were turned
/// into centrelines.
///
/// Chief joins walls at their reference lines, which are not the walls'
/// centres (an exterior wall's centre is 5" to the right of its line), so
/// right after the conversion an interior wall stops 5" short of the exterior
/// wall's centreline and two exterior walls meet 5" apart at a corner. This
/// moves wall ends along their own line, by at most [`MAX_SNAP`], so that
///
/// 1. two ends within [`MAX_SNAP`] of each other (of non-parallel walls)
///    meet at the intersection of the two centrelines, and
/// 2. an end that stops inside another wall's thickness reaches that wall's
///    centreline.
///
/// Returns the number of ends moved.
pub fn heal_joins(walls: &mut [Wall], openings: &mut [plan_core::model::Opening]) -> usize {
    let straight: Vec<usize> = (0..walls.len())
        .filter(|&i| walls[i].curve.is_none() && walls[i].length() > 1.0)
        .collect();
    let mut moved: Vec<[bool; 2]> = vec![[false; 2]; walls.len()];
    let mut count = 0;
    // Pass 1: end to end.
    let ends = |w: &Wall, start: bool| if start { w.start } else { w.end };
    for (ai, &i) in straight.iter().enumerate() {
        for start_i in [true, false] {
            if moved[i][usize::from(start_i)] {
                continue;
            }
            let ei = ends(&walls[i], start_i);
            let mut best: Option<(f64, usize, bool, Point)> = None;
            for &j in &straight[ai + 1..] {
                for start_j in [true, false] {
                    if moved[j][usize::from(start_j)] {
                        continue;
                    }
                    let ej = ends(&walls[j], start_j);
                    let gap = ei.dist(ej);
                    if !(0.05..=MAX_SNAP).contains(&gap) {
                        continue;
                    }
                    let (ui, uj) = (walls[i].direction(), walls[j].direction());
                    if cross(ui, uj).abs() < 0.2 {
                        continue;
                    }
                    let Some(x) = line_intersection(walls[i].start, ui, walls[j].start, uj) else {
                        continue;
                    };
                    if x.dist(ei) > MAX_SNAP
                        || x.dist(ej) > MAX_SNAP
                        || !can_move(&walls[i], start_i, x)
                        || !can_move(&walls[j], start_j, x)
                    {
                        continue;
                    }
                    if best.is_none_or(|b| gap < b.0) {
                        best = Some((gap, j, start_j, x));
                    }
                }
            }
            if let Some((_, j, start_j, x)) = best {
                move_end(walls, openings, i, start_i, x);
                move_end(walls, openings, j, start_j, x);
                moved[i][usize::from(start_i)] = true;
                moved[j][usize::from(start_j)] = true;
                count += 2;
            }
        }
    }
    // Pass 2: an end inside another wall's thickness.
    for &i in &straight {
        for start in [true, false] {
            if moved[i][usize::from(start)] {
                continue;
            }
            let e = ends(&walls[i], start);
            let ui = walls[i].direction();
            let mut best: Option<(f64, Point)> = None;
            for &j in &straight {
                if j == i {
                    continue;
                }
                let v = &walls[j];
                let uj = v.direction();
                if cross(ui, uj).abs() < 0.2 {
                    continue;
                }
                let rel = e.sub(v.start);
                let along = rel.x * uj.x + rel.y * uj.y;
                let side = cross(uj, rel).abs();
                if !(-1.0..=v.length() + 1.0).contains(&along)
                    || side < 0.25
                    || side > v.thickness / 2.0 + 1.0
                {
                    continue;
                }
                let Some(x) = line_intersection(walls[i].start, ui, v.start, uj) else {
                    continue;
                };
                if x.dist(e) <= MAX_SNAP
                    && can_move(&walls[i], start, x)
                    && best.is_none_or(|b| side < b.0)
                {
                    best = Some((side, x));
                }
            }
            if let Some((_, x)) = best {
                move_end(walls, openings, i, start, x);
                moved[i][usize::from(start)] = true;
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::floors::tests::floor_obj;
    use crate::import::tree::testutil::*;

    /// A class 215 wall type object: name, count, `layers` as
    /// `(thickness, material id, main)` plus the sentinel record.
    pub fn wall_type_obj(name: &str, layers: &[(f64, u32, bool)]) -> Vec<u8> {
        let mut payload = cstr(name);
        payload.extend_from_slice(&((layers.len() + 1) as u32).to_le_bytes());
        for &(t, id, main) in layers {
            let mut rec = vec![0u8; 518];
            rec[..8].copy_from_slice(&t.to_le_bytes());
            rec[8..12].copy_from_slice(&id.to_le_bytes());
            rec[0x15] = u8::from(main);
            payload.extend_from_slice(&rec);
        }
        payload.extend_from_slice(&[0u8; 518]);
        obj(WALL_TYPE, 0, &payload)
    }

    /// A line child (class 31) with the five line `f64`s.
    pub fn line_obj(x: f64, y: f64, dx: f64, dy: f64, len: f64) -> Vec<u8> {
        sized(31, 0, 96, |b| {
            put_f64(b, LINE_X, x);
            put_f64(b, LINE_X + 8, y);
            put_f64(b, LINE_X + 16, dx);
            put_f64(b, LINE_X + 24, dy);
            put_f64(b, LINE_X + 32, len);
        })
    }

    /// A class 40 line object: the chord fields plus centre, radius and the
    /// angles at the two ends.
    pub fn arc_obj(chord: (f64, f64, f64, f64, f64), arc: (f64, f64, f64, f64, f64)) -> Vec<u8> {
        sized(40, 0, 148, |b| {
            put_f64(b, LINE_X, chord.0);
            put_f64(b, LINE_X + 8, chord.1);
            put_f64(b, LINE_X + 16, chord.2);
            put_f64(b, LINE_X + 24, chord.3);
            put_f64(b, LINE_X + 32, chord.4);
            put_f64(b, 0x60, arc.0);
            put_f64(b, 0x68, arc.1);
            put_f64(b, 0x70, arc.2);
            put_f64(b, 0x78, arc.3);
            put_f64(b, 0x80, arc.4);
        })
    }

    pub fn point_obj(x: f64, y: f64) -> Vec<u8> {
        sized(4, 0, 24, |b| {
            put_f64(b, 8, x);
            put_f64(b, 16, y);
        })
    }

    /// A wall: header, the line child, a gap holding `top` (twice) and the
    /// type reference, then `extra` children.
    pub fn wall_obj(
        line: Vec<u8>,
        type_id: Option<u32>,
        top: Option<f64>,
        extra: &[Vec<u8>],
    ) -> Vec<u8> {
        let gap = 0x400usize;
        let kids: usize = extra.iter().map(|c| c.len() - 1).sum();
        let total = 0x169 + (line.len() - 1) + gap + kids;
        sized(WALL, 0, total, |b| {
            let l = &line[1..];
            b[0x169..0x169 + l.len()].copy_from_slice(l);
            let g = 0x169 + l.len();
            if let Some(t) = top {
                put_f64(b, g + 0x10, t);
                put_f64(b, g + 0x18, t);
            }
            if let Some(id) = type_id {
                b[g + 0x100] = 1;
                put_u32(b, g + 0x101, id);
            }
            let mut at = g + gap;
            for c in extra {
                b[at..at + c.len() - 1].copy_from_slice(&c[1..]);
                at += c.len() - 1;
            }
        })
    }

    fn types_from(body: &[u8]) -> (ObjectTree, WallTypes) {
        let tree = ObjectTree::build(body);
        let types = collect_wall_types(body, &tree);
        (tree, types)
    }

    #[test]
    fn decodes_type_ids_and_stacks() {
        let mut body = Vec::new();
        body.extend(with_id(
            581,
            wall_type_obj(
                "Brick-6",
                &[
                    (4.0, 1, false),
                    (1.0, 2, false),
                    (5.5, 3, true),
                    (0.5, 4, false),
                ],
            ),
        ));
        body.extend(with_id(
            351,
            wall_type_obj(
                "Interior-6",
                &[(0.5, 4, false), (5.5, 3, true), (0.5, 4, false)],
            ),
        ));
        let (_, types) = types_from(&body);
        assert_eq!(types.types.len(), 2);
        assert_eq!(types.get(581).unwrap().name, "Brick-6");
        assert_eq!(types.get(351).unwrap().name, "Interior-6");
        assert!(types.by_name("Interior-6").is_some());
        let brick = Stack::of(types.get(581).unwrap()).unwrap();
        assert!((brick.total - 11.0).abs() < 1e-9);
        assert!(!brick.symmetric());
        assert!((brick.centre_shift_right() - (2.5 + (5.0 - 0.5) / 2.0)).abs() < 1e-9);
        let int = Stack::of(types.get(351).unwrap()).unwrap();
        assert!(int.symmetric());
        assert_eq!(int.centre_shift_right(), 0.0);
    }

    /// An X17-layout wall type: cumulative start positions, 377-byte records.
    fn wall_type_obj_x17(name: &str, layers: &[(f64, u32, bool)]) -> Vec<u8> {
        let count = layers.len() + 2;
        let mut payload = cstr(name);
        // `count` = layers + one zero-thickness layer + the final record.
        payload.extend_from_slice(&(count as u32).to_le_bytes());
        let mut pos = 0.0;
        let mut recs: Vec<(f64, u32, bool)> = Vec::new();
        for &(t, id, main) in layers {
            recs.push((pos, id, main));
            pos += t;
        }
        recs.push((pos, 88, false)); // zero-thickness layer
        recs.push((pos, 35, false)); // final record
        for (p, id, main) in recs {
            let mut rec = vec![0u8; X17_STRIDE];
            rec[..8].copy_from_slice(&p.to_le_bytes());
            rec[8..12].copy_from_slice(&id.to_le_bytes());
            rec[0x15] = u8::from(main);
            payload.extend_from_slice(&rec);
        }
        payload.extend_from_slice(&[0u8; X17_TAIL]);
        obj(WALL_TYPE, 0, &payload)
    }

    #[test]
    fn decodes_x17_wall_types() {
        let mut body = Vec::new();
        body.extend(with_id(
            7,
            wall_type_obj_x17(
                "Brick-6",
                &[
                    (4.0, 79, false),
                    (1.0, 71, false),
                    (5.5, 77, true),
                    (0.5, 35, false),
                ],
            ),
        ));
        let tree = ObjectTree::build(&body);
        let types = collect_wall_types(&body, &tree);
        let t = types.get(7).expect("x17 type by id");
        assert_eq!(t.name, "Brick-6");
        assert_eq!(t.layers.len(), 4);
        assert!((t.total_thickness_in - 11.0).abs() < 1e-9);
        assert_eq!(t.layers.iter().position(|l| l.2), Some(2));
        // The same object under the X18 stride would not decode.
        assert!(Stack::of(t).unwrap().main == 5.5);
        // A single-layer type has only the layer and the final record.
        let mut b2 = Vec::new();
        let mut payload = cstr("Frame-3 1/2");
        payload.extend_from_slice(&2u32.to_le_bytes());
        for (p, id, main) in [(0.0f64, 134u32, true), (3.5, 35, false)] {
            let mut rec = vec![0u8; X17_STRIDE];
            rec[..8].copy_from_slice(&p.to_le_bytes());
            rec[8..12].copy_from_slice(&id.to_le_bytes());
            rec[0x15] = u8::from(main);
            payload.extend_from_slice(&rec);
        }
        payload.extend_from_slice(&[0u8; X17_TAIL]);
        b2.extend(with_id(9, obj(WALL_TYPE, 0, &payload)));
        let tree2 = ObjectTree::build(&b2);
        let t2 = collect_wall_types(&b2, &tree2);
        let f = t2.get(9).unwrap();
        assert_eq!(f.layers.len(), 1);
        assert!((f.total_thickness_in - 3.5).abs() < 1e-9 && f.layers[0].2);
    }

    #[test]
    fn decodes_a_wall_line_type_and_top() {
        let mut body = Vec::new();
        body.extend(with_id(
            351,
            wall_type_obj(
                "Interior-6",
                &[(0.5, 4, false), (5.5, 3, true), (0.5, 4, false)],
            ),
        ));
        let wall = wall_obj(
            line_obj(100.0, 200.0, 0.0, 1.0, 144.5),
            Some(351),
            Some(121.125),
            &[point_obj(100.0, 200.0), point_obj(100.0, 344.5)],
        );
        body.extend(wall);
        let (tree, types) = types_from(&body);
        let node = tree.of_kind(WALL, 0).next().unwrap();
        let w = decode_wall(&body, &tree, node, &types).unwrap();
        assert_eq!(w.start, (100.0, 200.0));
        assert_eq!(w.end, (100.0, 344.5));
        assert_eq!(w.type_id, Some(351));
        assert_eq!(w.top, Some(121.125));
        assert!(!w.approximated);

        let floor = ChiefFloor {
            node: 0,
            elevation: 0.0,
            ceiling: 121.125,
            third: 0.0,
            found: true,
        };
        let (pw, name) = plan_wall(&w, &floor, 1, &types);
        assert_eq!(name.as_deref(), Some("Interior-6"));
        assert_eq!(pw.wall_type.as_deref(), Some("Interior-6"));
        assert!((pw.thickness - 6.5).abs() < 1e-9);
        assert!((pw.height - 121.125).abs() < 1e-9);
        assert!((pw.start.x - 100.0).abs() < 1e-9 && (pw.end.y - 344.5).abs() < 1e-9);
        assert_eq!(pw.kind, WallKind::Interior);
        assert_eq!(pw.layer, "Walls, Normal");
    }

    #[test]
    fn exterior_wall_is_shifted_to_the_right_of_its_line() {
        let mut body = Vec::new();
        body.extend(with_id(
            581,
            wall_type_obj(
                "Brick-6",
                &[
                    (4.0, 1, false),
                    (1.0, 2, false),
                    (5.5, 3, true),
                    (0.5, 4, false),
                ],
            ),
        ));
        // A wall going +x: its right side is -y.
        body.extend(wall_obj(
            line_obj(0.0, 0.0, 1.0, 0.0, 100.0),
            Some(581),
            Some(121.125),
            &[],
        ));
        let (tree, types) = types_from(&body);
        let node = tree.of_kind(WALL, 0).next().unwrap();
        let w = decode_wall(&body, &tree, node, &types).unwrap();
        let floor = ChiefFloor {
            node: 0,
            elevation: 0.0,
            ceiling: 121.125,
            third: 0.0,
            found: true,
        };
        let (pw, _) = plan_wall(&w, &floor, 1, &types);
        assert_eq!(pw.kind, WallKind::Exterior);
        assert_eq!(pw.exterior_side, Side::Right);
        assert!(
            (pw.start.y - (-4.5 / 2.0 - 2.5)).abs() < 1e-9,
            "{}",
            pw.start.y
        );
        assert!((pw.thickness - 11.0).abs() < 1e-9);
    }

    #[test]
    fn curved_wall_from_a_class_40_arc() {
        use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};
        let r: f64 = 24.27;
        let chord = r * 2f64.sqrt();
        // The quarter circle seen in a real project: centre (663.86, 667.86),
        // from angle pi to pi/2, chord running up-right at 45 degrees.
        let arc = arc_obj(
            (639.59, 667.86, FRAC_1_SQRT_2, FRAC_1_SQRT_2, chord),
            (663.86, 667.86, r, PI, FRAC_PI_2),
        );
        let wall = wall_obj(arc, None, None, &[]);
        let types = WallTypes::default();
        let tree = ObjectTree::build(&wall);
        let node = tree.of_kind(WALL, 0).next().unwrap();
        let w = decode_wall(&wall, &tree, node, &types).unwrap();
        let b = w.bulge.expect("bulge");
        let expected = r - (r * r - chord * chord / 4.0).sqrt();
        assert!((b - expected).abs() < 1e-6 && b > 0.0, "{b} vs {expected}");
        let floor = ChiefFloor {
            node: 0,
            elevation: 0.0,
            ceiling: 108.0,
            third: 0.0,
            found: true,
        };
        let (pw, _) = plan_wall(&w, &floor, 1, &types);
        let curve = pw.curve.expect("curve");
        assert!((curve.bulge - expected).abs() < 1e-6);
        // The arc through start, apex and end has the stored radius.
        let (_, radius) = curve.arc_center_radius(pw.start, pw.end).unwrap();
        assert!((radius - r).abs() < 0.05, "{radius}");

        // The mirrored arc (centre on the other side) bulges the other way.
        let arc2 = arc_obj(
            (639.59, 667.86, FRAC_1_SQRT_2, FRAC_1_SQRT_2, chord),
            (639.59, 692.13, r, -FRAC_PI_2, 0.0),
        );
        let wall2 = wall_obj(arc2, None, None, &[]);
        let tree2 = ObjectTree::build(&wall2);
        let n2 = tree2.of_kind(WALL, 0).next().unwrap();
        let w2 = decode_wall(&wall2, &tree2, n2, &types).unwrap();
        assert!(w2.bulge.unwrap() < 0.0);
        // A centre that is not r from the ends is rejected: straight wall.
        let bad = arc_obj(
            (639.59, 667.86, FRAC_1_SQRT_2, FRAC_1_SQRT_2, chord),
            (0.0, 0.0, r, PI, FRAC_PI_2),
        );
        let wall3 = wall_obj(bad, None, None, &[]);
        let tree3 = ObjectTree::build(&wall3);
        let n3 = tree3.of_kind(WALL, 0).next().unwrap();
        assert!(decode_wall(&wall3, &tree3, n3, &types)
            .unwrap()
            .bulge
            .is_none());
    }

    fn mk(start: (f64, f64), end: (f64, f64), thick: f64, id: u64) -> Wall {
        let mut w = Wall::new(
            Point::new(start.0, start.1),
            Point::new(end.0, end.1),
            thick,
            108.0,
            WallKind::Interior,
        );
        w.id = id;
        w
    }

    #[test]
    fn heal_joins_closes_corners_and_tees_and_keeps_openings() {
        // Two exterior walls whose centrelines were shifted 5" to the right
        // of Chief's reference lines (which met at (100, 0)), an interior
        // wall ending at the reference line of the first, and one starting
        // there with a window 30" from its start.
        let mut walls = vec![
            mk((0.0, -5.0), (100.0, -5.0), 11.0, 1),
            mk((105.0, 0.0), (105.0, 100.0), 11.0, 2),
            mk((50.0, 100.0), (50.0, 0.0), 6.5, 3),
            mk((70.0, 0.0), (70.0, 80.0), 6.5, 4),
        ];
        let mut op = plan_core::model::Opening::default_window(1, 4, 30.0);
        op.wall_id = 4;
        let mut openings = vec![op];
        let n = heal_joins(&mut walls, &mut openings);
        assert_eq!(n, 4, "two ends of the corner and two tee ends");
        // The corner is at the intersection of the centrelines.
        assert!(walls[0].end.dist(Point::new(105.0, -5.0)) < 1e-9);
        assert!(walls[1].start.dist(Point::new(105.0, -5.0)) < 1e-9);
        // The interior walls reach the first wall's centreline.
        assert!(walls[2].end.dist(Point::new(50.0, -5.0)) < 1e-9);
        assert!(walls[3].start.dist(Point::new(70.0, -5.0)) < 1e-9);
        // The window stays where it was in the plan (moved with the start).
        assert!((openings[0].center_offset - 35.0).abs() < 1e-9);
        let w = &walls[3];
        let c = w.point_at(openings[0].center_offset);
        assert!((c.y - 30.0).abs() < 1e-9, "{c:?}");
        // Healing twice changes nothing.
        assert_eq!(heal_joins(&mut walls, &mut openings), 0);
        // Parallel walls and distant ends are left alone.
        let mut far = vec![
            mk((0.0, 0.0), (100.0, 0.0), 6.5, 1),
            mk((0.0, 20.0), (100.0, 20.0), 6.5, 2),
            mk((300.0, 0.0), (300.0, 100.0), 6.5, 3),
        ];
        assert_eq!(heal_joins(&mut far, &mut []), 0);
    }

    #[test]
    fn missing_pieces_fall_back_sensibly() {
        // No line child: two points are used; no type; no top.
        let body = wall_obj_without_line();
        let tree = ObjectTree::build(&body);
        let types = WallTypes::default();
        let node = tree.of_kind(WALL, 0).next().unwrap();
        let w = decode_wall(&body, &tree, node, &types).unwrap();
        assert!(w.approximated);
        assert_eq!(w.type_id, None);
        assert_eq!(w.top, None);
        let floor = ChiefFloor {
            node: 0,
            elevation: 137.875,
            ceiling: 109.125,
            third: 0.0,
            found: true,
        };
        let (pw, name) = plan_wall(&w, &floor, 2, &types);
        assert!(name.is_none());
        assert_eq!(pw.height, 109.125);
        assert_eq!(pw.thickness, 5.5);
        // A line with a zero direction is rejected.
        let bad = wall_obj(line_obj(0.0, 0.0, 0.0, 0.0, 10.0), None, None, &[]);
        let tree = ObjectTree::build(&bad);
        let n = tree.of_kind(WALL, 0).next().unwrap();
        assert!(decode_wall(&bad, &tree, n, &types).is_none());
    }

    fn wall_obj_without_line() -> Vec<u8> {
        let pts = [point_obj(10.0, 10.0), point_obj(210.0, 10.0)];
        let total = 0x200 + pts.iter().map(|p| p.len() - 1).sum::<usize>();
        sized(WALL, 0, total, |b| {
            let mut at = 0x200;
            for p in &pts {
                b[at..at + p.len() - 1].copy_from_slice(&p[1..]);
                at += p.len() - 1;
            }
        })
    }

    #[test]
    fn roles_follow_type_names() {
        assert_eq!(
            role_of("Room Divider_1\"", false).class,
            WallClass::RoomDivider
        );
        assert!(role_of("Room Divider_1\"", false).invisible);
        assert_eq!(role_of("Interior Railing", false).class, WallClass::Railing);
        assert_eq!(
            role_of("Deck Railing/Fence", false).class,
            WallClass::DeckRailing
        );
        assert!(matches!(
            role_of("Railing/Fence", false).class,
            WallClass::Fencing { .. }
        ));
        assert_eq!(role_of("Glass Shower", false).class, WallClass::Glass);
        assert_eq!(role_of("Glass Block", false).class, WallClass::Standard);
        assert_eq!(
            role_of("10\" Concrete Stem Wall", true).class,
            WallClass::Foundation
        );
        assert_eq!(
            role_of("10\" Concrete Stem Wall", false).class,
            WallClass::Standard
        );
        assert_eq!(role_of("Interior-6", false).layer, "Walls, Normal");
    }

    #[test]
    fn floors_walls_nest_in_a_floor() {
        let wall = wall_obj(line_obj(0.0, 0.0, 1.0, 0.0, 50.0), None, None, &[]);
        let body = floor_obj(0.0, 121.125, &[wall]);
        let tree = ObjectTree::build(&body);
        let w = tree.of_kind(WALL, 0).next().unwrap();
        assert!(tree.ancestor_of_class(w, 30).is_some());
    }
}
