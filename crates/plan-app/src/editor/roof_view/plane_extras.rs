//! Roof plane extras (Round 16 brief 18b): the heights a plane shows in its
//! specification, its plan styles and report, and the commands that place a
//! plane against another one (Move to be Coplanar, Place Roof Plane
//! Intersection Point, Make Parallel / Perpendicular, Display on Floor Above
//! / Below). The plan-roof arithmetic is in `plan_roof::placement` and
//! `plan_roof::heights`.

use super::*;
use plan_roof::{
    edge_length, perimeter, plan_length_for_entry, snap_to_wall_surface, turn_to_align,
    vertical_structure_depth, HeightLock, LengthEntry, PlaneHeights, WallSurface,
};

// ===================================================================
// Plan styles (Line Style, Fill Style and Arrow panels)
// ===================================================================

/// How the inside of a plane is filled in plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillKind {
    #[default]
    None,
    Solid,
    Hatch,
}

impl FillKind {
    pub const ALL: [FillKind; 3] = [FillKind::None, FillKind::Solid, FillKind::Hatch];

    pub fn label(self) -> &'static str {
        match self {
            FillKind::None => "No Fill",
            FillKind::Solid => "Solid",
            FillKind::Hatch => "Hatch",
        }
    }
}

/// Fill Style panel (RF-89).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaneFill {
    pub kind: FillKind,
    pub color: [u8; 3],
    /// 0 to 255.
    pub opacity: u8,
    /// Distance between hatch lines, inches.
    pub spacing: f64,
}

impl Default for PlaneFill {
    fn default() -> Self {
        Self {
            kind: FillKind::None,
            color: [200, 120, 80],
            opacity: 70,
            spacing: 12.0,
        }
    }
}

/// Arrow panel (RF-91): the slope arrow drawn at the plane's centre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlopeArrow {
    pub show: bool,
    /// Length in plan, inches; 0 draws it a fixed size on the screen.
    pub length: f64,
    /// Own color; `None` follows the layer.
    pub color: Option<[u8; 3]>,
    /// Show the pitch (and label) text beside the arrow.
    pub show_text: bool,
}

impl Default for SlopeArrow {
    fn default() -> Self {
        Self {
            show: true,
            length: 0.0,
            color: None,
            show_text: true,
        }
    }
}

/// Line Style, Fill Style and Arrow of a plane in plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaneStyle {
    /// Own line color; `None` follows the plane's layer.
    pub line_color: Option<[u8; 3]>,
    /// Multiplies the usual line weights.
    pub line_weight: f64,
    pub dash: LineStyle,
    pub fill: PlaneFill,
    pub arrow: SlopeArrow,
}

impl Default for PlaneStyle {
    fn default() -> Self {
        Self {
            line_color: None,
            line_weight: 1.0,
            dash: LineStyle::Solid,
            fill: PlaneFill::default(),
            arrow: SlopeArrow::default(),
        }
    }
}

// ===================================================================
// Heights and the Polyline report
// ===================================================================

/// The Polyline panel's numbers (RF-92; manual p. 834).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaneReport {
    /// Perimeter in plan and along the slope.
    pub perimeter_projected: f64,
    pub perimeter_actual: f64,
    /// Sloped surface area and its footprint in plan, square inches.
    pub area_surface: f64,
    pub area_projected: f64,
    /// Surface area less the holes (what is framed and roofed).
    pub area_framing: f64,
    /// Roof structure volume, cubic inches: framing area times thickness.
    pub volume: f64,
}

impl RoofPlaneRecord {
    /// Plan distance from the baseline to the plane's far side.
    pub fn run_depth(&self) -> f64 {
        let (a, _) = self.baseline;
        let up = self.up_slope();
        self.plan_polygon()
            .iter()
            .map(|p| p.sub(a).dot(up))
            .fold(0.0, f64::max)
    }

    /// The four heights and what they hang on, for a structure `thickness`
    /// thick measured square to the slope. The baseline is the plane's top
    /// surface over the outside face of the wall; the record's own
    /// [`baseline_height`](Self::baseline_height) is the eave tip, which
    /// sits `overhang` further out and lower.
    pub fn plane_heights(&self, thickness: f64) -> PlaneHeights {
        let k = self.pitch / 12.0;
        let baseline = self.baseline_height() + self.overhang * k;
        let plate_top = self
            .plate_top
            .unwrap_or(baseline - vertical_structure_depth(thickness, self.pitch));
        PlaneHeights {
            baseline,
            pitch: self.pitch,
            run: (self.run_depth() - self.overhang).max(0.0),
            overhang: self.overhang,
            thickness,
            plate_top,
            plate_width: self.plate_width,
        }
    }

    /// Takes the pitch, eave height and plate from `h`, which came from
    /// [`plane_heights`](Self::plane_heights) through an edit.
    pub fn apply_heights(&mut self, h: &PlaneHeights) {
        if (h.pitch - self.pitch).abs() > 1e-9 {
            self.set_pitch(h.pitch);
        }
        self.set_baseline_height(h.fascia_top());
        self.plate_top = Some(h.plate_top);
    }

    /// Perimeter, areas and volume for a structure `thickness` thick.
    pub fn report(&self, thickness: f64) -> PlaneReport {
        let (perimeter_projected, perimeter_actual) = perimeter(&self.polygon3d);
        let area_surface = self.area();
        let area_projected = self.to_roof_plane(0).projected_area().abs();
        let k = self.pitch / 12.0;
        let holes: f64 = self
            .holes
            .iter()
            .map(|h| plan_core::geometry::polygon_area(&h.outline).abs())
            .sum::<f64>()
            * (1.0 + k * k).sqrt();
        let area_framing = (area_surface - holes).max(0.0);
        PlaneReport {
            perimeter_projected,
            perimeter_actual,
            area_surface,
            area_projected,
            area_framing,
            volume: area_framing * thickness,
        }
    }

    /// Plan and actual length of edge `i`.
    pub fn edge_lengths(&self, i: usize) -> Option<(f64, f64)> {
        let n = self.polygon3d.len();
        (i < n).then(|| edge_length(self.polygon3d[i], self.polygon3d[(i + 1) % n]))
    }

    /// Sets the length of edge `i` to `typed`, read as projected or actual
    /// (RF-97). Vertex `i` stays; the next vertex moves along the edge and the
    /// plane stays planar. Returns whether anything changed.
    pub fn set_edge_length(&mut self, i: usize, typed: f64, entry: LengthEntry) -> bool {
        let n = self.polygon3d.len();
        if i >= n || typed <= 0.0 {
            return false;
        }
        let (a3, b3) = (self.polygon3d[i], self.polygon3d[(i + 1) % n]);
        let plan = plan_length_for_entry(a3, b3, typed, entry);
        let a = Point::new(a3[0], -a3[2]);
        let b = Point::new(b3[0], -b3[2]);
        let d = b.sub(a);
        if d.length() < 1e-9 || (plan - d.length()).abs() < 1e-9 {
            return false;
        }
        self.move_vertex((i + 1) % n, a.add(d.normalized().scale(plan)));
        true
    }
}

// ===================================================================
// Placing planes against each other
// ===================================================================

/// Move to be Coplanar (RF-113): raises or lowers plane `moving` of floor
/// `fi` into the plane of `target`. The moved plane becomes manual.
pub fn move_coplanar(
    project: &mut Project,
    fi: usize,
    moving: Id,
    target: Id,
) -> Result<f64, String> {
    let mut set = load(&project.floors[fi]);
    let (m, t) = match (set.plane(moving), set.plane(target)) {
        (Some(m), Some(t)) => (m.to_roof_plane(0), t.to_roof_plane(0)),
        _ => return Err("pick two roof planes".into()),
    };
    let shift = plan_roof::coplanar_shift(&m, &t).map_err(|e| e.to_string())?;
    if shift.abs() < 1e-6 {
        return Err("the planes are coplanar already".into());
    }
    if let Some(r) = set.plane_mut(moving) {
        let eave = r.baseline_height();
        r.set_baseline_height(eave + shift);
        r.auto = false;
    }
    store(project, fi, &mut set);
    Ok(shift)
}

/// Place Roof Plane Intersection Point (RF-111): drops a temporary CAD point
/// where the extended edge `edge` of plane `a` meets the plane of `b`.
/// Returns the point and its elevation.
pub fn place_intersection_point(
    project: &mut Project,
    fi: usize,
    a: Id,
    edge: usize,
    b: Id,
) -> Result<(Point, f64), String> {
    let set = load(&project.floors[fi]);
    let (pa, pb) = match (set.plane(a), set.plane(b)) {
        (Some(x), Some(y)) => (x.to_roof_plane(0), y.to_roof_plane(0)),
        _ => return Err("pick two roof planes".into()),
    };
    let q = plan_roof::edge_plane_point(&pa, edge, &pb)
        .ok_or_else(|| "the edge runs parallel to that plane".to_string())?;
    let at = Point::new(q[0], -q[2]);
    if project.layers.get(TEMP_POINT_LAYER).is_none() {
        project
            .layers
            .add(Layer::new(TEMP_POINT_LAYER, [200, 0, 200], 13));
    }
    for item in crate::tools::cad::point_items(at, false) {
        project.add_cad(fi, TEMP_POINT_LAYER, item);
    }
    Ok((at, q[1]))
}

/// Layer of Place Point's temporary points (the CAD tools' own layer).
pub const TEMP_POINT_LAYER: &str = "CAD, Temporary Points";

/// Make Parallel / Make Perpendicular for a plane: turns plane `id` about the
/// middle of its baseline until the baseline is parallel (or square) to
/// `reference`, a direction in plan. Returns the turn in degrees.
pub fn align_plane(
    project: &mut Project,
    fi: usize,
    id: Id,
    reference: Point,
    perpendicular: bool,
) -> Result<f64, String> {
    let mut set = load(&project.floors[fi]);
    let r = set.plane_mut(id).ok_or("pick a roof plane")?;
    let (a, b) = r.baseline;
    let turn = turn_to_align(b.sub(a), reference, perpendicular);
    if turn.abs() < 1e-9 {
        return Err(if perpendicular {
            "the baseline is square to it already".into()
        } else {
            "the baseline is parallel to it already".into()
        });
    }
    r.rotate(turn, Point::lerp(a, b, 0.5));
    store(project, fi, &mut set);
    Ok(turn.to_degrees())
}

/// The direction of the straight thing nearest `at` that a plane can be made
/// parallel to: a wall, an edge of another roof plane (not `except`) or a CAD
/// line, within `tol` inches.
pub fn reference_direction(
    floor: &Floor,
    except: Option<Id>,
    at: Point,
    tol: f64,
) -> Option<Point> {
    let mut best: Option<(f64, Point)> = None;
    let mut consider = |d: f64, a: Point, b: Point| {
        if d <= tol && b.sub(a).length() > 1e-9 && best.is_none_or(|(d0, _)| d < d0) {
            best = Some((d, b.sub(a).normalized()));
        }
    };
    for w in &floor.walls {
        consider(
            dist_to_segment(at, w.start, w.end) - w.thickness * 0.5,
            w.start,
            w.end,
        );
    }
    for c in &floor.cad {
        if let CadItem::Line { a, b } = c.item {
            consider(dist_to_segment(at, a, b), a, b);
        }
    }
    for r in &load(floor).planes {
        if Some(r.id) == except {
            continue;
        }
        let poly = r.plan_polygon();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            consider(dist_to_segment(at, a, b), a, b);
        }
    }
    best.map(|(_, d)| d)
}

/// Display on Floor Above / Below (RF-96): moves the display of plane `id`
/// from floor `fi` to floor `fi + delta`. Its heights are absolute, so the
/// 3D roof does not change; the plane becomes manual (a rebuild would not
/// know where it went). Returns the floor index it landed on.
pub fn move_display(project: &mut Project, fi: usize, id: Id, delta: isize) -> Result<usize, String> {
    let to = fi as isize + delta;
    if to < 0 || to as usize >= project.floors.len() {
        return Err(if delta > 0 {
            "there is no floor above".into()
        } else {
            "there is no floor below".into()
        });
    }
    let to = to as usize;
    let mut from = load(&project.floors[fi]);
    if from.dormers.iter().any(|d| d.main == id) {
        return Err("a plane with a dormer stays with it".into());
    }
    let Some(k) = from.planes.iter().position(|p| p.id == id) else {
        return Err("pick a roof plane".into());
    };
    let mut rec = from.planes.remove(k);
    rec.auto = false;
    let mut dest = load(&project.floors[to]);
    dest.planes.push(rec);
    store(project, fi, &mut from);
    store(project, to, &mut dest);
    Ok(to)
}

// ===================================================================
// Wall surfaces (Use Special Snapping)
// ===================================================================

/// The outside surface line of every exterior wall of `floor`. "Outside" is
/// away from the middle of the exterior walls (a wall drawn the other way
/// round still has its outside out); with fewer than three walls the wall's
/// own exterior side counts.
pub fn wall_surfaces(floor: &Floor) -> Vec<WallSurface> {
    let walls: Vec<&Wall> = floor
        .walls
        .iter()
        .filter(|w| w.kind == WallKind::Exterior && w.length() > 1.0)
        .collect();
    let centre = (walls.len() >= 3).then(|| {
        let sum = walls
            .iter()
            .fold(Point::ZERO, |s, w| s.add(Point::lerp(w.start, w.end, 0.5)));
        sum.scale(1.0 / walls.len() as f64)
    });
    walls
        .iter()
        .map(|w| {
            let mut n = w.exterior_normal();
            if let Some(c) = centre {
                if Point::lerp(w.start, w.end, 0.5).sub(c).dot(n) < 0.0 {
                    n = n.scale(-1.0);
                }
            }
            let off = n.scale(w.thickness * 0.5);
            WallSurface {
                a: w.start.add(off),
                b: w.end.add(off),
            }
        })
        .collect()
}

/// Baseline `a -> b` with both ends moved onto the outside of a parallel wall
/// when one is close.
pub fn snap_baseline(floor: &Floor, a: Point, b: Point) -> (Point, Point) {
    snap_to_wall_surface(a, b, &wall_surfaces(floor))
}

/// Snaps edge `i` of plane `rec` to the outside of a nearby parallel wall
/// (both its vertices move onto the wall line). Returns whether it moved.
pub fn snap_edge_to_walls(rec: &mut RoofPlaneRecord, floor: &Floor, i: usize) -> bool {
    let n = rec.polygon3d.len();
    if !rec.special_snapping || i >= n {
        return false;
    }
    let poly = rec.plan_polygon();
    let (a, b) = (poly[i], poly[(i + 1) % n]);
    let (sa, sb) = snap_baseline(floor, a, b);
    if sa.dist(a) < 1e-9 && sb.dist(b) < 1e-9 {
        return false;
    }
    rec.move_vertex(i, sa);
    rec.move_vertex((i + 1) % n, sb);
    true
}

// ===================================================================
// Edit toolbar commands
// ===================================================================

pub const CMD_COPLANAR: &str = "roof.coplanar";
pub const CMD_INTERSECTION: &str = "roof.intersection";
pub const CMD_PARALLEL: &str = "roof.plane_parallel";
pub const CMD_PERPENDICULAR: &str = "roof.plane_perpendicular";
pub const CMD_FLOOR_ABOVE: &str = "roof.display_above";
pub const CMD_FLOOR_BELOW: &str = "roof.display_below";

/// `(command, label)` of the edit buttons a selected roof plane offers.
pub const PLANE_COMMANDS: [(&str, &str); 6] = [
    (CMD_COPLANAR, "Move to be Coplanar"),
    (CMD_INTERSECTION, "Place Roof Plane Intersection Point"),
    (CMD_PARALLEL, "Make Parallel"),
    (CMD_PERPENDICULAR, "Make Perpendicular"),
    (CMD_FLOOR_ABOVE, "Display on Floor Above"),
    (CMD_FLOOR_BELOW, "Display on Floor Below"),
];

/// Runs a plane command on the selected roof plane. Returns whether `id` was
/// one of them. The picking commands hand over to the roof tool, which asks
/// for the second object.
pub fn run_plane_command(cx: &mut EditorContext, id: &str) -> bool {
    use crate::tools::roof::RoofMode;
    use crate::tools::ToolId;
    let Some((_, label)) = PLANE_COMMANDS.iter().find(|(c, _)| *c == id) else {
        return false;
    };
    let mode = match id {
        CMD_COPLANAR => Some(RoofMode::Coplanar),
        CMD_INTERSECTION => Some(RoofMode::IntersectionPoint),
        CMD_PARALLEL => Some(RoofMode::MakeParallel),
        CMD_PERPENDICULAR => Some(RoofMode::MakePerpendicular),
        _ => None,
    };
    if let Some(m) = mode {
        cx.requests
            .push(crate::editor::EditorRequest::SetTool(ToolId::RoofVariant(m)));
        return true;
    }
    let Some(ObjectRef::RoofPlane(plane)) = cx.selection.single() else {
        cx.status = format!("{label}: select a roof plane first");
        return true;
    };
    let delta = if id == CMD_FLOOR_ABOVE { 1 } else { -1 };
    let fi = cx.floor;
    cx.begin_change(label);
    match move_display(&mut cx.project, fi, plane, delta) {
        Ok(to) => {
            cx.mark_dirty();
            cx.selection.clear();
            cx.status = format!("Roof plane now shows on {}", cx.project.floors[to].name);
        }
        Err(e) => {
            cx.cancel_change();
            cx.status = format!("{label}: {e}");
        }
    }
    true
}

/// Height lock radio labels in the order the specification lists them.
pub const LOCKS: [(HeightLock, &str); 4] = [
    (HeightLock::RidgeTop, "Ridge Top Height"),
    (HeightLock::Baseline, "Baseline Height"),
    (HeightLock::FasciaTop, "Fascia Top Height"),
    (HeightLock::TopOfPlate, "Top of Plate"),
];

// ===================================================================
// Hatching
// ===================================================================

/// Hatch lines at 45 degrees, `spacing` inches apart, clipped to `poly`.
pub fn hatch_segments(poly: &[Point], spacing: f64) -> Vec<(Point, Point)> {
    let spacing = spacing.max(1.0);
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    // Lines x - y = c; c runs over the polygon's range of x - y.
    let cs: Vec<f64> = poly.iter().map(|p| p.x - p.y).collect();
    let lo = cs.iter().cloned().fold(f64::MAX, f64::min);
    let hi = cs.iter().cloned().fold(f64::MIN, f64::max);
    let mut out = Vec::new();
    let mut c = (lo / spacing).ceil() * spacing;
    // At most a few thousand lines, whatever the spacing.
    let mut guard = 0;
    while c <= hi && guard < 4000 {
        guard += 1;
        // Where y = x - c crosses each edge (as a parameter along the line).
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (fa, fb) = (a.x - a.y - c, b.x - b.y - c);
            if (fa > 0.0) != (fb > 0.0) {
                let t = fa / (fa - fb);
                xs.push(a.x + (b.x - a.x) * t);
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            out.push((
                Point::new(pair[0], pair[0] - c),
                Point::new(pair[1], pair[1] - c),
            ));
        }
        c += spacing;
    }
    out
}

// ===================================================================
// Wall Roof panel helpers
// ===================================================================

/// Does an attic wall stand above wall `id` of floor `fi` (Combine with Above
/// Wall is only offered then, manual p. 429)? An attic wall on the next floor
/// that runs along the wall within its thickness counts.
pub fn attic_wall_above(project: &Project, fi: usize, id: Id) -> bool {
    let (Some(floor), Some(above)) = (project.floors.get(fi), project.floors.get(fi + 1)) else {
        return false;
    };
    let Some(w) = floor.wall(id) else {
        return false;
    };
    let dir = w.direction();
    above.walls.iter().filter(|a| a.flags.attic).any(|a| {
        let mid = Point::lerp(a.start, a.end, 0.5);
        dist_to_segment(mid, w.start, w.end) <= w.thickness.max(a.thickness) * 0.5 + TOL
            && a.direction().cross(dir).abs() < 0.02
    })
}

// ===================================================================
// Residential-template behaviour: a roof when a room closes (RF-164)
// ===================================================================

thread_local! {
    /// The wall signature of each floor at the last attempt, so a roof the
    /// user deleted (or undid) does not come straight back: it is only built
    /// again after the walls change.
    static CLOSED_SEEN: std::cell::RefCell<std::collections::HashMap<usize, u64>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// With the Preferences switch on: when the exterior walls of the current
/// floor close and the floor has no roof, builds one with the Roof Defaults.
/// It makes no undo step of its own (like Auto Rebuild, it follows the edit
/// that closed the room). Returns whether a roof was built.
pub fn build_when_room_closes(cx: &mut EditorContext) -> bool {
    let fi = cx.floor;
    let set = load(&cx.project.floors[fi]);
    if set.settings.is_some() || !set.planes.is_empty() {
        return false;
    }
    let walls = exterior_walls(&cx.project.floors[fi]);
    if footprint_from_walls(&walls, TOL).is_none() {
        CLOSED_SEEN.with(|m| m.borrow_mut().remove(&fi));
        return false;
    }
    let sig = project_signature(&cx.project, fi);
    let fresh = CLOSED_SEEN.with(|m| m.borrow_mut().insert(fi, sig) != Some(sig));
    if !fresh {
        return false;
    }
    let s = RoofSettings::from_defaults(&cx.defaults);
    if rebuild(&mut cx.project, fi, s, false).is_ok() {
        cx.mark_dirty();
        cx.status = "Roof built over the closed walls".into();
        return true;
    }
    false
}
