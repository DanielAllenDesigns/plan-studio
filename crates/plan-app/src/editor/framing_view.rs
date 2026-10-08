//! Build Framing and manually placed framing: wall, floor and roof framing from
//! `plan-framing`, stored on the floor and drawn in plan.
//!
//! # Storage
//!
//! Everything lives in the typed `Floor.framing` slot (a list of JSON values),
//! so it is saved with the plan and undone with it. (The roofs keep their
//! records in `Floor.roofs`; see `roof_view`.)
//!
//! * Automatic framing is one serialized [`Member`] per value, replaced by
//!   every Build Framing.
//! * The manual section is a list of single-key tagged [`Record`] values mixed
//!   into the same slot: members placed by the framing tools
//!   ([`Record::Manual`]), the layout lines that steer Build Framing (Joist
//!   Direction, Roof Truss Direction, Bearing Line, Framing Reference Marker,
//!   Truss Base) and [`Record::Built`] members that Build Framing makes from
//!   those lines (directed joists, bearing beams, laid-out trusses). Build
//!   Framing and Delete Framing leave `Manual` members and layout lines alone.
//!   A value that is not a record is automatic framing, so old files and old
//!   readers (which skip what they do not parse) keep working.
//!
//! # What is built
//!
//! * **Build Framing**: the active floor. Every wall except invisible, room
//!   divider and railing walls is framed with its openings
//!   ([`plan_framing::frame_wall_with_marker`]; a Framing Reference Marker
//!   within [`MARKER_REACH`] of a wall anchors its studs); every room gets a
//!   floor platform ([`plan_framing::frame_floor`], joists across the short
//!   side, or [`plan_framing::frame_floor_directed`] when a Joist Direction,
//!   Bearing Line or Reference Marker lies in the room); and when roof planes
//!   are stored on the floor they are framed ([`plan_framing::frame_roof`]).
//!   Each Truss Base is filled with trusses ([`plan_framing::layout_trusses`]).
//! * **Build All Framing**: the same for every floor.
//!
//! Automatic members are drawn on layer `"Framing"`; manual members on
//! Chief's framing layers (`"Framing, Floor Joists"`, `"Framing, Rafters"`,
//! `"Framing, Posts"`, `"Framing, Beams"`, `"Framing, Trusses"`), as the plan
//! outline of each piece of lumber. 3D meshes of the manual members come from
//! [`manual_framing_meshes`]; DXF export adds them from [`floor_dxf`].

use super::{Camera, EditorContext, ObjectRef};
use crate::editor::roof_view;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape, Stroke};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::{detect_rooms, DxfExport, Floor, FloorKind, Id, Layer, Opening, Project, Room};
use plan_framing::{
    frame_floor, frame_floor_directed, frame_roof, frame_wall_with_marker, layout_trusses,
    manual_takeoff, roof_framing_takeoff, BearingLine, FramingDefaults, FramingMember,
    JoistDirection, JoistDirectionLine, ManualMemberKind, MaterialList, Member, MemberKind,
    ReferenceMarker, RoofFramingDefaults, RoofTrussDirection, Takeoff, TrussBase, TrussSpec,
    TrussType,
};
use plan_roof::{Roof, RoofPlane, DEFAULT_FASCIA_HEIGHT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// The layer framing is drawn on.
pub const LAYER: &str = "Framing";

/// How many members a build made, by group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildSummary {
    pub walls: usize,
    pub floor: usize,
    pub roof: usize,
}

impl BuildSummary {
    pub fn total(&self) -> usize {
        self.walls + self.floor + self.roof
    }

    fn add(&mut self, o: BuildSummary) {
        self.walls += o.walls;
        self.floor += o.floor;
        self.roof += o.roof;
    }
}

/// The automatic members stored on `floor` (values that are not manual
/// [`Record`]s; ones that do not parse are skipped).
pub fn load(floor: &Floor) -> Vec<Member> {
    floor
        .framing
        .iter()
        .filter(|v| !is_record(v))
        .filter_map(|v| serde_json::from_value::<Member>(v.clone()).ok())
        .collect()
}

/// The roof stored on `floor`, rebuilt as a `plan_roof::Roof`; `None` when it
/// has no planes.
pub fn roof_of(floor: &Floor) -> Option<Roof> {
    let set = roof_view::load(floor);
    if set.planes.is_empty() {
        return None;
    }
    let baseline = set.planes[0].baseline_height();
    let planes = set
        .planes
        .iter()
        .enumerate()
        .map(|(i, p)| RoofPlane {
            polygon3d: p.polygon3d.clone(),
            pitch_in_12: p.pitch,
            baseline: p.baseline,
            source_edge: i,
        })
        .collect();
    Some(Roof {
        planes,
        fascia_height: DEFAULT_FASCIA_HEIGHT,
        baseline_elevation: baseline,
        approximate: false,
    })
}

/// What Build Framing makes for one floor.
#[derive(Clone, Debug, Default)]
pub struct FloorBuild {
    /// Automatic members (walls, undirected floors, roofs).
    pub members: Vec<Member>,
    /// Members made from layout lines (directed joists, beams, trusses); their
    /// ids are 0 until [`build`] stores them.
    pub built: Vec<FramingMember>,
    pub summary: BuildSummary,
}

/// Reach of a Framing Reference Marker to a wall's centre line, inches.
pub const MARKER_REACH: f64 = 24.0;

/// The layout lines of a floor, grouped for the generators.
struct Layout {
    directions: Vec<JoistDirectionLine>,
    truss_directions: Vec<RoofTrussDirection>,
    bearing: Vec<BearingLine>,
    markers: Vec<ReferenceMarker>,
    bases: Vec<TrussBase>,
}

impl Layout {
    fn from_records(records: &[Record]) -> Self {
        let mut l = Layout {
            directions: Vec::new(),
            truss_directions: Vec::new(),
            bearing: Vec::new(),
            markers: Vec::new(),
            bases: Vec::new(),
        };
        for r in records {
            match r {
                Record::JoistDirection { dir, .. } => l.directions.push(*dir),
                Record::TrussDirection { dir, .. } => l.truss_directions.push(*dir),
                Record::BearingLine { line, .. } => l.bearing.push(*line),
                Record::Marker { marker, .. } => l.markers.push(*marker),
                Record::TrussBase { base, .. } => l.bases.push(base.clone()),
                Record::Manual(_) | Record::Built(_) => {}
            }
        }
        l
    }

    fn marker_near_segment(&self, a: Point, b: Point) -> Option<ReferenceMarker> {
        self.markers
            .iter()
            .find(|m| dist_to_segment(m.point, a, b) <= MARKER_REACH)
            .copied()
    }

    fn marker_in(&self, poly: &[Point]) -> Option<ReferenceMarker> {
        self.markers
            .iter()
            .find(|m| point_in_polygon(m.point, poly))
            .copied()
    }

    fn direction_in(&self, poly: &[Point]) -> Option<JoistDirectionLine> {
        self.directions
            .iter()
            .find(|d| point_in_polygon(Point::lerp(d.line.0, d.line.1, 0.5), poly))
            .copied()
    }

    fn bearing_in(&self, poly: &[Point]) -> Vec<BearingLine> {
        self.bearing
            .iter()
            .filter(|b| {
                [b.line.0, b.line.1, Point::lerp(b.line.0, b.line.1, 0.5)]
                    .iter()
                    .any(|p| point_in_polygon(*p, poly))
            })
            .copied()
            .collect()
    }
}

/// Frames floor `fi` of `project`: the walls, the floor platforms, the roof
/// stored on that floor (with `with_roof`) and the trusses of its Truss
/// Bases, honoring Joist Direction, Bearing Line, Reference Marker and Truss
/// Base lines.
pub fn frame_floor_all(project: &Project, fi: usize, with_roof: bool) -> FloorBuild {
    let floor = &project.floors[fi];
    let d = FramingDefaults::default();
    let layout = Layout::from_records(&load_records(floor));
    let mut out = FloorBuild::default();

    for wall in floor
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
    {
        let openings: Vec<&Opening> = floor.openings_on(wall.id).collect();
        let marker = layout.marker_near_segment(wall.start, wall.end);
        let m = frame_wall_with_marker(wall, &openings, floor.elevation, &d, marker.as_ref());
        out.summary.walls += m.len();
        out.members.extend(m);
    }
    if floor.kind != FloorKind::Foundation {
        for room in detect_rooms(&floor.walls, 0.5) {
            if room.area_sq_ft() < 1.0 {
                continue;
            }
            let dir = layout.direction_in(&room.polygon);
            let bearing = layout.bearing_in(&room.polygon);
            let marker = layout.marker_in(&room.polygon);
            if dir.is_none() && bearing.is_empty() && marker.is_none() {
                let m = frame_floor(&room, floor.elevation, &d, JoistDirection::Auto);
                out.summary.floor += m.len();
                out.members.extend(m);
            } else {
                let m = frame_floor_directed(
                    &room,
                    floor.elevation,
                    &d,
                    dir.as_ref(),
                    &bearing,
                    marker.as_ref(),
                    0,
                );
                out.summary.floor += m.len();
                out.built.extend(m);
            }
        }
    }
    if with_roof {
        if let Some(roof) = roof_of(floor) {
            let m = frame_roof(&roof, &RoofFramingDefaults::default());
            out.summary.roof += m.len();
            out.members.extend(m);
        }
        for base in &layout.bases {
            let m = truss_layout(base, &layout);
            out.summary.roof += m.len();
            out.built.extend(m);
        }
    }
    out
}

/// Trusses over one base: its Roof Truss Direction (the one whose middle lies
/// in the base, else spaced 24" across the longer side) and Reference Marker.
fn truss_layout(base: &TrussBase, layout: &Layout) -> Vec<FramingMember> {
    let dir = layout
        .truss_directions
        .iter()
        .find(|d| point_in_polygon(Point::lerp(d.line.0, d.line.1, 0.5), &base.points))
        .copied()
        .unwrap_or_else(|| {
            let (lo, hi) = bounds(&base.points);
            let along_x = hi.x - lo.x >= hi.y - lo.y;
            let (a, b) = if along_x {
                (lo, Point::new(hi.x, lo.y))
            } else {
                (lo, Point::new(lo.x, hi.y))
            };
            RoofTrussDirection::new((a, b), 0.0)
        });
    let marker = layout.marker_in(&base.points);
    let spec = TrussSpec::new(TrussType::Fink, 0.0, 6.0);
    layout_trusses(base, &dir, &spec, marker.as_ref(), 0)
}

fn bounds(pts: &[Point]) -> (Point, Point) {
    let mut lo = pts.first().copied().unwrap_or(Point::ZERO);
    let mut hi = lo;
    for p in pts {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (lo, hi)
}

/// [`frame_floor_all`] as the automatic members plus the summary (the
/// layout-made members count in the summary but are not returned).
pub fn frame_floor_members(
    project: &Project,
    fi: usize,
    with_roof: bool,
) -> (Vec<Member>, BuildSummary) {
    let b = frame_floor_all(project, fi, with_roof);
    (b.members, b.summary)
}

/// Build Framing (active floor) or Build All Framing (every floor) as one
/// undo step. Turns the Framing layer on so the result shows. Manual members
/// and layout lines stay; members an earlier build made from them are
/// replaced.
pub fn build(cx: &mut EditorContext, all_floors: bool) -> BuildSummary {
    cx.begin_change(if all_floors {
        "Build All Framing"
    } else {
        "Build Framing"
    });
    let floors: Vec<usize> = if all_floors {
        (0..cx.project.floors.len()).collect()
    } else {
        vec![cx.floor]
    };
    let mut total = BuildSummary::default();
    let mut built_layers: Vec<String> = Vec::new();
    for fi in floors {
        let mut fb = frame_floor_all(&cx.project, fi, true);
        for m in &mut fb.built {
            m.id = cx.project.alloc_id();
            m.layer_name = layer_for(m.kind).to_string();
        }
        built_layers.extend(fb.built.iter().map(|m| m.layer_name.clone()));
        store_auto(&mut cx.project.floors[fi], &fb.members, fb.built);
        total.add(fb.summary);
    }
    ensure_layer(&mut cx.project);
    ensure_manual_layers(&mut cx.project);
    for layer in built_layers {
        show_layer(&mut cx.project, &layer);
    }
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "Built framing: {} wall, {} floor and {} roof members",
        total.walls, total.floor, total.roof
    );
    total
}

/// Removes the built framing of the active floor (one undo step); members and
/// layout lines placed by hand stay. Returns how many members went.
pub fn clear(cx: &mut EditorContext) -> usize {
    let built = load_records(cx.floor())
        .iter()
        .filter(|r| matches!(r, Record::Built(_)))
        .count();
    let n = load(cx.floor()).len() + built;
    if n == 0 {
        cx.status = "There is no framing to delete".into();
        return 0;
    }
    cx.begin_change("Delete Framing");
    store_auto(cx.floor_mut(), &[], Vec::new());
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Deleted {n} framing members");
    n
}

/// Makes sure the Framing layer exists and is shown.
fn ensure_layer(project: &mut Project) {
    match project.layers.get_mut(LAYER) {
        Some(l) => l.display = true,
        None => {
            project.layers.add(Layer::new(LAYER, [180, 140, 60], 18));
        }
    }
}

// ----- takeoff -----

/// The framing members of the active floor (`all_floors` false) or of the
/// whole plan.
pub fn members_for(project: &Project, floor: usize, all_floors: bool) -> Vec<Member> {
    if all_floors {
        project.floors.iter().flat_map(load).collect()
    } else {
        load(&project.floors[floor])
    }
}

pub fn takeoff_of(members: &[Member]) -> Takeoff {
    roof_framing_takeoff(members)
}

/// Lumber list as table rows `(columns, rows)`: the cut lines, then the
/// totals.
pub fn takeoff_table(members: &[Member]) -> (Vec<String>, Vec<Vec<String>>) {
    table_of(&takeoff_of(members))
}

/// The table rows of a [`Takeoff`].
pub fn table_of(t: &Takeoff) -> (Vec<String>, Vec<Vec<String>>) {
    let columns = vec!["Item".to_string(), "Qty".to_string()];
    let mut rows: Vec<Vec<String>> = t
        .lines
        .iter()
        .map(|(name, n)| vec![name.clone(), n.to_string()])
        .collect();
    for (size, lf) in &t.linear_feet_by_size {
        rows.push(vec![
            format!("Total {size} (linear ft)"),
            format!("{lf:.1}"),
        ]);
    }
    rows.push(vec![
        "Total board feet".to_string(),
        format!("{:.1}", t.board_feet),
    ]);
    (columns, rows)
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The takeoff as CSV (`Item,Qty` rows).
pub fn takeoff_csv(members: &[Member]) -> String {
    csv_of(takeoff_table(members))
}

fn csv_of((cols, rows): (Vec<String>, Vec<Vec<String>>)) -> String {
    let mut out = cols.join(",");
    out.push('\n');
    for r in rows {
        out.push_str(&r.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

// ----- drawing -----

/// Plan outline of a member: the convex hull of its eight corners seen from
/// above (3D `[x, y, z]` is plan `(x, -z)`).
pub fn plan_outline(m: &Member) -> Vec<Point> {
    let pts: Vec<Point> = m
        .corners()
        .iter()
        .map(|c| Point::new(c[0], -c[2]))
        .collect();
    convex_hull(pts)
}

/// Andrew's monotone chain; collinear points are dropped.
fn convex_hull(mut pts: Vec<Point>) -> Vec<Point> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup_by(|a, b| a.dist(*b) < 1e-6);
    if pts.len() < 3 {
        return pts;
    }
    let cross =
        |o: Point, a: Point, b: Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut hull: Vec<Point> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Point>> = if pass == 0 {
            Box::new(pts.iter())
        } else {
            Box::new(pts.iter().rev())
        };
        for &p in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 1e-9
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// Draws the active floor's framing: the built members when the Framing layer
/// is shown, then the manual members and layout lines on their own layers.
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    draw_built(cx, painter, cam);
    draw_manual(cx, painter, cam);
}

/// A built member's plan outline and its extent.
struct Hull {
    pts: Vec<Point>,
    lo: Point,
    hi: Point,
}

/// What drawing the framing of the active floor derives from the stored data:
/// the outline of every built member and the manual records. Kept until the
/// editor context signals a change (`EditorContext::cache_key`) or the floor
/// switches; a frame that changed nothing only draws.
struct DrawCache {
    key: (u64, u64),
    floor: usize,
    hulls: Option<Rc<Vec<Hull>>>,
    records: Option<Rc<Vec<Record>>>,
}

thread_local! {
    static DRAW_CACHE: RefCell<Option<DrawCache>> = const { RefCell::new(None) };
}

fn with_draw_cache<R>(cx: &EditorContext, f: impl FnOnce(&mut DrawCache) -> R) -> R {
    DRAW_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let key = cx.cache_key();
        if c.as_ref()
            .is_none_or(|x| x.key != key || x.floor != cx.floor)
        {
            *c = Some(DrawCache {
                key,
                floor: cx.floor,
                hulls: None,
                records: None,
            });
        }
        f(c.as_mut().expect("just filled"))
    })
}

fn built_hulls(cx: &EditorContext) -> Rc<Vec<Hull>> {
    if let Some(h) = with_draw_cache(cx, |c| c.hulls.clone()) {
        return h;
    }
    let hulls: Rc<Vec<Hull>> = Rc::new(
        cx.framing
            .iter()
            .map(|m| {
                let pts = plan_outline(m);
                let (mut lo, mut hi) = (
                    Point::new(f64::MAX, f64::MAX),
                    Point::new(f64::MIN, f64::MIN),
                );
                for p in &pts {
                    lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
                    hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
                }
                Hull { pts, lo, hi }
            })
            .collect(),
    );
    with_draw_cache(cx, |c| c.hulls = Some(hulls.clone()));
    hulls
}

fn manual_records(cx: &EditorContext) -> Rc<Vec<Record>> {
    if let Some(r) = with_draw_cache(cx, |c| c.records.clone()) {
        return r;
    }
    let records = Rc::new(load_records(cx.floor()));
    with_draw_cache(cx, |c| c.records = Some(records.clone()));
    records
}

fn draw_built(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if cx.framing.is_empty() || !cx.layers().is_visible(LAYER) {
        return;
    }
    let [r, g, b] = cx.layers().get(LAYER).map_or([180, 140, 60], |l| l.color);
    let fill = Color32::from_rgba_unmultiplied(r, g, b, 70);
    let edge = Stroke::new(0.75_f32, Color32::from_rgb(r, g, b));
    let view = cam.rect.expand(2.0);
    for h in built_hulls(cx).iter() {
        // Members outside the canvas draw nothing visible.
        let (a, c) = (cam.world_to_screen(h.lo), cam.world_to_screen(h.hi));
        if !view.intersects(egui::Rect::from_two_pos(a, c)) {
            continue;
        }
        let pts: Vec<Pos2> = h.pts.iter().map(|p| cam.world_to_screen(*p)).collect();
        match pts.len() {
            0 | 1 => {}
            2 => {
                painter.line_segment([pts[0], pts[1]], edge);
            }
            _ => {
                painter.add(Shape::convex_polygon(pts, fill, edge));
            }
        }
    }
}

/// A one-line description of a member kind count, for the status bar.
pub fn count_kind(members: &[Member], kind: MemberKind) -> usize {
    members.iter().filter(|m| m.kind == kind).count()
}

// ===================================================================
// Manual framing
// ===================================================================

/// Layers of Chief's framing tools.
pub const LAYER_JOISTS: &str = "Framing, Floor Joists";
pub const LAYER_RAFTERS: &str = "Framing, Rafters";
pub const LAYER_POSTS: &str = "Framing, Posts";
pub const LAYER_BEAMS: &str = "Framing, Beams";
pub const LAYER_TRUSSES: &str = "Framing, Trusses";

const MANUAL_LAYERS: [(&str, [u8; 3]); 5] = [
    (LAYER_JOISTS, [200, 150, 70]),
    (LAYER_RAFTERS, [170, 120, 60]),
    (LAYER_POSTS, [120, 90, 50]),
    (LAYER_BEAMS, [150, 100, 40]),
    (LAYER_TRUSSES, [190, 110, 80]),
];

/// Subfloor thickness between the joist tops and the finished floor.
const SUBFLOOR: f64 = 0.75;
/// Depth of a floor/ceiling truss, inches.
const FLOOR_TRUSS_DEPTH: f64 = 12.0;
/// Wall height assumed on a floor without walls, inches.
const DEFAULT_WALL_HEIGHT: f64 = 109.125;
/// Shortest line member the tools keep, inches.
pub const MIN_MEMBER: f64 = 3.0;

/// The layer a kind of member is drawn on.
pub fn layer_for(kind: ManualMemberKind) -> &'static str {
    use ManualMemberKind as K;
    match kind {
        K::Joist | K::JoistBlocking | K::BearingLine => LAYER_JOISTS,
        K::Rafter | K::RoofBlocking | K::RoofPurlin => LAYER_RAFTERS,
        K::Post | K::PostWithFooting => LAYER_POSTS,
        K::FloorCeilingBeam | K::RoofBeam => LAYER_BEAMS,
        K::FloorCeilingTruss | K::RoofTruss | K::GirderTruss | K::TrussBase => LAYER_TRUSSES,
        K::GeneralFraming | K::Blocking => LAYER,
    }
}

/// One manual framing object stored in `Floor.framing` (see the module docs).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Record {
    /// A member placed by a framing tool.
    Manual(FramingMember),
    /// A member Build Framing made from layout lines; replaced by every build.
    Built(FramingMember),
    /// Joist Direction: joists are spaced along the line and run across it.
    JoistDirection {
        id: Id,
        dir: JoistDirectionLine,
    },
    /// Roof Truss Direction.
    TrussDirection {
        id: Id,
        dir: RoofTrussDirection,
    },
    BearingLine {
        id: Id,
        line: BearingLine,
    },
    Marker {
        id: Id,
        marker: ReferenceMarker,
    },
    TrussBase {
        id: Id,
        base: TrussBase,
    },
}

const RECORD_KEYS: [&str; 7] = [
    "Manual",
    "Built",
    "JoistDirection",
    "TrussDirection",
    "BearingLine",
    "Marker",
    "TrussBase",
];

/// Whether a stored value is a manual [`Record`] (a one-key object with a
/// record's name) rather than an automatic member.
fn is_record(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == 1 && o.keys().all(|k| RECORD_KEYS.contains(&k.as_str())))
}

impl Record {
    pub fn id(&self) -> Id {
        match self {
            Record::Manual(m) | Record::Built(m) => m.id,
            Record::JoistDirection { id, .. }
            | Record::TrussDirection { id, .. }
            | Record::BearingLine { id, .. }
            | Record::Marker { id, .. }
            | Record::TrussBase { id, .. } => *id,
        }
    }

    pub fn member(&self) -> Option<&FramingMember> {
        match self {
            Record::Manual(m) | Record::Built(m) => Some(m),
            _ => None,
        }
    }

    /// The layer the record is drawn on.
    pub fn layer(&self) -> &str {
        match self {
            Record::Manual(m) | Record::Built(m) => &m.layer_name,
            Record::JoistDirection { .. } | Record::BearingLine { .. } => LAYER_JOISTS,
            Record::TrussDirection { .. } | Record::TrussBase { .. } => LAYER_TRUSSES,
            Record::Marker { .. } => LAYER,
        }
    }

    /// Name for the status bar.
    pub fn name(&self) -> String {
        match self {
            Record::Manual(m) | Record::Built(m) => {
                let mut k = m.kind.name().to_string();
                if let Some(c) = k.get_mut(..1) {
                    c.make_ascii_uppercase();
                }
                k
            }
            Record::JoistDirection { .. } => "Joist Direction".into(),
            Record::TrussDirection { .. } => "Roof Truss Direction".into(),
            Record::BearingLine { .. } => "Bearing Line".into(),
            Record::Marker { .. } => "Framing Reference Marker".into(),
            Record::TrussBase { .. } => "Truss Base".into(),
        }
    }

    fn translate(&mut self, d: Point) {
        match self {
            Record::Manual(m) | Record::Built(m) => {
                m.start = m.start + d;
                m.end = m.end + d;
            }
            Record::JoistDirection { dir, .. } => dir.line = (dir.line.0 + d, dir.line.1 + d),
            Record::TrussDirection { dir, .. } => dir.line = (dir.line.0 + d, dir.line.1 + d),
            Record::BearingLine { line, .. } => line.line = (line.line.0 + d, line.line.1 + d),
            Record::Marker { marker, .. } => marker.point = marker.point + d,
            Record::TrussBase { base, .. } => {
                for p in &mut base.points {
                    *p = *p + d;
                }
            }
        }
    }

    /// The points that bound the record in plan (box selection).
    pub fn extent(&self) -> Vec<Point> {
        match self {
            Record::Manual(m) | Record::Built(m) => {
                let o = member_outline(m);
                if o.is_empty() {
                    vec![m.start, m.end]
                } else {
                    o
                }
            }
            Record::JoistDirection { dir, .. } => vec![dir.line.0, dir.line.1],
            Record::TrussDirection { dir, .. } => vec![dir.line.0, dir.line.1],
            Record::BearingLine { line, .. } => vec![line.line.0, line.line.1],
            Record::Marker { marker, .. } => vec![marker.point],
            Record::TrussBase { base, .. } => base.points.clone(),
        }
    }

    /// The two end points of a record drawn as a line: members that run from
    /// start to end (not posts) and the direction and bearing lines.
    pub fn line_ends(&self) -> Option<(Point, Point)> {
        match self {
            Record::Manual(m) | Record::Built(m)
                if m.kind.is_physical() && !m.kind.is_vertical() =>
            {
                Some((m.start, m.end))
            }
            Record::JoistDirection { dir, .. } => Some(dir.line),
            Record::TrussDirection { dir, .. } => Some(dir.line),
            Record::BearingLine { line, .. } => Some(line.line),
            _ => None,
        }
    }

    /// Moves one end of a line record to `to` (`at_end`: the second point).
    /// Returns whether the record has ends to move.
    fn set_end(&mut self, at_end: bool, to: Point) -> bool {
        let slot = match self {
            Record::Manual(m) | Record::Built(m)
                if m.kind.is_physical() && !m.kind.is_vertical() =>
            {
                if at_end {
                    &mut m.end
                } else {
                    &mut m.start
                }
            }
            Record::JoistDirection { dir, .. } => {
                if at_end {
                    &mut dir.line.1
                } else {
                    &mut dir.line.0
                }
            }
            Record::TrussDirection { dir, .. } => {
                if at_end {
                    &mut dir.line.1
                } else {
                    &mut dir.line.0
                }
            }
            Record::BearingLine { line, .. } => {
                if at_end {
                    &mut line.line.1
                } else {
                    &mut line.line.0
                }
            }
            _ => return false,
        };
        *slot = to;
        true
    }

    /// Whether `p` is on the record, within `tol` inches.
    pub fn hit(&self, p: Point, tol: f64) -> bool {
        match self {
            Record::Manual(m) | Record::Built(m) => member_hit(m, p, tol),
            Record::JoistDirection { dir, .. } => dist_to_segment(p, dir.line.0, dir.line.1) <= tol,
            Record::TrussDirection { dir, .. } => dist_to_segment(p, dir.line.0, dir.line.1) <= tol,
            Record::BearingLine { line, .. } => dist_to_segment(p, line.line.0, line.line.1) <= tol,
            Record::Marker { marker, .. } => marker.point.dist(p) <= tol * 2.0,
            Record::TrussBase { base, .. } => {
                let n = base.points.len();
                (0..n).any(|i| dist_to_segment(p, base.points[i], base.points[(i + 1) % n]) <= tol)
            }
        }
    }
}

/// The manual records stored on `floor`, in storage order. Values that do not
/// parse are skipped.
pub fn load_records(floor: &Floor) -> Vec<Record> {
    floor
        .framing
        .iter()
        .filter(|v| is_record(v))
        .filter_map(|v| serde_json::from_value::<Record>(v.clone()).ok())
        .collect()
}

/// Replaces the manual records of `floor`, keeping the automatic members.
pub fn store_records(floor: &mut Floor, records: &[Record]) {
    let mut values: Vec<Value> = floor
        .framing
        .iter()
        .filter(|v| !is_record(v))
        .cloned()
        .collect();
    values.extend(records.iter().filter_map(|r| serde_json::to_value(r).ok()));
    floor.framing = values;
}

/// Replaces the automatic members and the members built from layout lines,
/// keeping `Manual` members and the layout lines.
fn store_auto(floor: &mut Floor, members: &[Member], built: Vec<FramingMember>) {
    let mut records: Vec<Record> = load_records(floor)
        .into_iter()
        .filter(|r| !matches!(r, Record::Built(_)))
        .collect();
    records.extend(built.into_iter().map(Record::Built));
    let mut values: Vec<Value> = members
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();
    values.extend(records.iter().filter_map(|r| serde_json::to_value(r).ok()));
    floor.framing = values;
}

/// Every placed or built member on `floor`.
pub fn manual_members(floor: &Floor) -> Vec<FramingMember> {
    load_records(floor)
        .into_iter()
        .filter_map(|r| match r {
            Record::Manual(m) | Record::Built(m) => Some(m),
            _ => None,
        })
        .collect()
}

/// The manual record `id` on `floor`.
pub fn find(floor: &Floor, id: Id) -> Option<Record> {
    load_records(floor).into_iter().find(|r| r.id() == id)
}

/// Makes sure the five manual framing layers exist (a hidden one stays hidden).
pub fn ensure_manual_layers(project: &mut Project) {
    for (name, color) in MANUAL_LAYERS {
        project.layers.add(Layer::new(name, color, 18));
    }
}

/// Shows layer `name` so what was just placed or built is visible.
fn show_layer(project: &mut Project, name: &str) {
    project.layers.set_display(name, true);
}

// ----- creating members -----

fn wall_height(floor: &Floor) -> f64 {
    floor
        .walls
        .iter()
        .map(|w| w.height)
        .fold(0.0, f64::max)
        .max(0.0)
        .max(if floor.walls.is_empty() {
            DEFAULT_WALL_HEIGHT
        } else {
            0.0
        })
}

/// Elevation of the top of the wall plates on `floor`, inches.
pub fn top_plate(floor: &Floor) -> f64 {
    floor.elevation + wall_height(floor)
}

/// A new member of `kind` from `start` to `end` with Chief's defaults: its
/// layer, and an elevation that puts floor members under the subfloor, posts
/// on the floor and roof members on the wall plates. Rafters rise 6:12 and a
/// floor/ceiling truss is a flat 12" truss. The id is 0 until stored.
pub fn new_member(
    floor: &Floor,
    kind: ManualMemberKind,
    start: Point,
    end: Point,
) -> FramingMember {
    use ManualMemberKind as K;
    let mut m = FramingMember::new(0, kind, start, end);
    m.layer_name = layer_for(kind).to_string();
    m.elevation_bottom = match kind {
        K::Joist | K::JoistBlocking | K::FloorCeilingBeam => floor.elevation - SUBFLOOR - m.depth,
        K::FloorCeilingTruss => floor.elevation - SUBFLOOR - FLOOR_TRUSS_DEPTH,
        K::Rafter
        | K::RoofBeam
        | K::RoofBlocking
        | K::RoofPurlin
        | K::RoofTruss
        | K::GirderTruss => top_plate(floor),
        _ => floor.elevation,
    };
    match kind {
        K::Rafter => m.rise = m.plan_length() * 0.5,
        K::FloorCeilingTruss => {
            if let Some(t) = &mut m.truss {
                t.pitch = 0.0;
                t.heel_height = FLOOR_TRUSS_DEPTH;
                t.overhang = 0.0;
            }
        }
        K::Post | K::PostWithFooting if !floor.walls.is_empty() => m.height = wall_height(floor),
        _ => {}
    }
    m
}

/// Default on-centre spacing of a direction line, inches.
pub fn default_spacing(truss: bool) -> f64 {
    if truss {
        ManualMemberKind::RoofTruss.default_spacing()
    } else {
        ManualMemberKind::Joist.default_spacing()
    }
}

// ----- editing -----

/// Stores a new record built from the id it gets (one undo step named
/// `label`). Returns the id.
pub fn add_record(cx: &mut EditorContext, label: &str, make: impl FnOnce(Id) -> Record) -> Id {
    cx.begin_change(label);
    let id = cx.project.alloc_id();
    let mut records = load_records(cx.floor());
    records.push(make(id));
    let layer = records.last().map(|r| r.layer().to_string());
    store_records(cx.floor_mut(), &records);
    ensure_manual_layers(&mut cx.project);
    if let Some(layer) = layer {
        show_layer(&mut cx.project, &layer);
    }
    cx.mark_dirty();
    cx.refresh();
    id
}

/// Deletes the records `ids` (one undo step). Returns how many went.
pub fn delete_records(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let mut records = load_records(cx.floor());
    let before = records.len();
    records.retain(|r| !ids.contains(&r.id()));
    let n = before - records.len();
    if n == 0 {
        return 0;
    }
    cx.begin_change("Delete Framing");
    store_records(cx.floor_mut(), &records);
    cx.selection
        .items
        .retain(|o| !matches!(o, ObjectRef::Framing(i) if ids.contains(i)));
    cx.mark_dirty();
    cx.refresh();
    n
}

/// Moves records by `delta` (a drag shares one undo step through the merged
/// change; call `cx.end_merge()` when it ends).
pub fn move_records(cx: &mut EditorContext, ids: &[Id], delta: Point) -> usize {
    if delta.length() < 1e-9 {
        return 0;
    }
    let mut records = load_records(cx.floor());
    let mut n = 0;
    for r in records.iter_mut().filter(|r| ids.contains(&r.id())) {
        n += 1;
        r.translate(delta);
    }
    if n == 0 {
        return 0;
    }
    cx.begin_change_merged("Move Framing");
    store_records(cx.floor_mut(), &records);
    cx.mark_dirty();
    cx.refresh();
    n
}

/// Translates the records `ids` of `floor` by `d` without opening an undo
/// step (group drags and nudges inside the caller's step). Returns how many
/// moved.
pub fn translate_in(floor: &mut Floor, ids: &[Id], d: Point) -> usize {
    let mut records = load_records(floor);
    let mut n = 0;
    for r in records.iter_mut().filter(|r| ids.contains(&r.id())) {
        r.translate(d);
        n += 1;
    }
    if n > 0 {
        store_records(floor, &records);
    }
    n
}

/// Moves one end of the line record `id` of `floor` to `to` (an end handle
/// drag; the caller owns the undo step). A moved member becomes a manual
/// one, so a rebuild keeps it.
pub fn move_end_in(floor: &mut Floor, id: Id, at_end: bool, to: Point) -> bool {
    let mut records = load_records(floor);
    let Some(slot) = records.iter_mut().find(|r| r.id() == id) else {
        return false;
    };
    if !slot.set_end(at_end, to) {
        return false;
    }
    let promoted = match &*slot {
        Record::Built(m) => Some(Record::Manual(m.clone())),
        _ => None,
    };
    if let Some(p) = promoted {
        *slot = p;
    }
    store_records(floor, &records);
    true
}

/// Moves corner `i` of the Truss Base `id` of `floor` to `to` (a vertex
/// handle drag; the caller owns the undo step).
pub fn move_vertex_in(floor: &mut Floor, id: Id, i: usize, to: Point) -> bool {
    let mut records = load_records(floor);
    let moved = records
        .iter_mut()
        .find(|r| r.id() == id)
        .is_some_and(|r| match r {
            Record::TrussBase { base, .. } => base.points.get_mut(i).map(|v| *v = to).is_some(),
            _ => false,
        });
    if moved {
        store_records(floor, &records);
    }
    moved
}

/// Applies the Framing Member Specification: the draft replaces the member
/// with its id and becomes a manual member (a rebuild no longer discards it).
pub fn apply_edit(cx: &mut EditorContext, draft: FramingMember) -> bool {
    let mut records = load_records(cx.floor());
    let Some(slot) = records
        .iter_mut()
        .find(|r| r.member().is_some_and(|m| m.id == draft.id))
    else {
        return false;
    };
    if matches!(slot, Record::Manual(m) if *m == draft) {
        return false;
    }
    cx.begin_change("Framing Member Specification");
    let layer = draft.layer_name.clone();
    *slot = Record::Manual(draft);
    store_records(cx.floor_mut(), &records);
    ensure_manual_layers(&mut cx.project);
    show_layer(&mut cx.project, &layer);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// The topmost record under `p` (the last stored one wins).
pub fn pick(floor: &Floor, p: Point, tol: f64) -> Option<Id> {
    load_records(floor)
        .iter()
        .rev()
        .find(|r| r.hit(p, tol))
        .map(Record::id)
}

// ----- selection (`ObjectRef::Framing` in `cx.selection`) -----

/// The selected manual records.
pub fn selected(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Framing(id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// Replaces the selection with these records.
pub fn select(cx: &mut EditorContext, ids: Vec<Id>) {
    cx.selection.items = ids.into_iter().map(ObjectRef::Framing).collect();
}

// ----- geometry -----

/// Plan outline of a member: the box (or, for a truss, the span times its
/// ply thickness). Empty for layout lines and zero-size members.
pub fn member_outline(m: &FramingMember) -> Vec<Point> {
    if !m.kind.is_physical() {
        return Vec::new();
    }
    if m.kind.is_truss() {
        let run = m.plan_length();
        if run < 1e-6 {
            return Vec::new();
        }
        let n = (m.end - m.start).normalized().perp() * (m.width / 2.0);
        return vec![m.start + n, m.end + n, m.end - n, m.start - n];
    }
    let boxes = m.to_boxes();
    let Some(b) = boxes.first() else {
        return Vec::new();
    };
    let mut pts = Vec::with_capacity(8);
    for sx in [-0.5, 0.5] {
        for sy in [-0.5, 0.5] {
            for sz in [-0.5, 0.5] {
                let mut c = b.center;
                for (k, s) in [sx, sy, sz].into_iter().enumerate() {
                    for (i, ci) in c.iter_mut().enumerate() {
                        *ci += b.axes[k][i] * b.size[k] * s;
                    }
                }
                pts.push(Point::new(c[0], -c[2]));
            }
        }
    }
    convex_hull(pts)
}

/// Plan square of a post's footing.
pub fn footing_outline(m: &FramingMember) -> Vec<Point> {
    m.footing().map_or_else(Vec::new, |f| {
        let h = f.size / 2.0;
        let c = f.center;
        vec![
            Point::new(c.x - h, c.y - h),
            Point::new(c.x + h, c.y - h),
            Point::new(c.x + h, c.y + h),
            Point::new(c.x - h, c.y + h),
        ]
    })
}

fn member_hit(m: &FramingMember, p: Point, tol: f64) -> bool {
    let o = member_outline(m);
    if o.len() >= 3 {
        let n = o.len();
        return point_in_polygon(p, &o)
            || (0..n).any(|i| dist_to_segment(p, o[i], o[(i + 1) % n]) <= tol);
    }
    dist_to_segment(p, m.start, m.end) <= tol
}

// ----- takeoff -----

/// Manual and built members of the active floor (`all_floors` false) or of the
/// whole plan.
pub fn manual_for(project: &Project, floor: usize, all_floors: bool) -> Vec<FramingMember> {
    if all_floors {
        project.floors.iter().flat_map(manual_members).collect()
    } else {
        manual_members(&project.floors[floor])
    }
}

/// The takeoff window's data: automatic and manual members together.
#[derive(Clone, Debug, Default)]
pub struct TakeoffData {
    /// Automatic plus manual member count.
    pub members: usize,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The table as `Item,Qty` CSV.
    pub csv: String,
    /// Piece counts by size and length ([`MaterialList::to_csv`]).
    pub material_csv: String,
}

fn merge_takeoff(mut a: Takeoff, b: Takeoff) -> Takeoff {
    for (name, n) in b.lines {
        match a.lines.iter_mut().find(|(l, _)| *l == name) {
            Some((_, c)) => *c += n,
            None => a.lines.push((name, n)),
        }
    }
    a.board_feet += b.board_feet;
    for (size, lf) in b.linear_feet_by_size {
        match a.linear_feet_by_size.iter_mut().find(|(s, _)| *s == size) {
            Some((_, f)) => *f += lf,
            None => a.linear_feet_by_size.push((size, lf)),
        }
    }
    let key = |name: &str| {
        let mut it = name.split('x').map(|n| n.parse::<u32>().ok());
        match (it.next().flatten(), it.next().flatten()) {
            (Some(t), Some(d)) => (t, d),
            _ => (u32::MAX, 0),
        }
    };
    a.linear_feet_by_size.sort_by_key(|(n, _)| key(n));
    a
}

/// The takeoff of the automatic and the manual members.
pub fn takeoff_data(project: &Project, floor: usize, all_floors: bool) -> TakeoffData {
    let auto = members_for(project, floor, all_floors);
    let manual = manual_for(project, floor, all_floors);
    let t = merge_takeoff(takeoff_of(&auto), manual_takeoff(&manual));
    let (columns, rows) = table_of(&t);
    TakeoffData {
        members: auto.len() + manual.len(),
        csv: csv_of((columns.clone(), rows.clone())),
        columns,
        rows,
        material_csv: MaterialList::from_members(&auto, &manual).to_csv(),
    }
}

// ----- 3D and DXF -----

/// Meshes of every manual and built member on every floor, for the 3D view
/// (boxes per lumber member, chords and webs per truss, footings under posts).
/// Members on a hidden layer are left out.
pub fn manual_framing_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let mut meshes = Vec::new();
    for floor in &project.floors {
        for m in manual_members(floor) {
            if !project.layers.is_visible(&m.layer_name) {
                continue;
            }
            meshes.extend(m.to_boxes().iter().map(|b| b.mesh(Some(m.id))));
        }
    }
    meshes
}

/// The plan outlines of the roof planes stored on `floor`.
pub fn roof_plane_outlines(floor: &Floor) -> Vec<Vec<Point>> {
    roof_view::load(floor)
        .planes
        .iter()
        .map(|p| p.plan_polygon())
        .collect()
}

/// File > Export > DXF for `floor`: the plan plus the roof plane outlines on
/// layer "Roof Planes" and the manual framing (member outlines, direction and
/// bearing lines, truss bases) on layer "Framing".
pub fn floor_dxf(project: &Project, floor: usize, rooms: &[Room]) -> String {
    let fl = &project.floors[floor];
    let mut closed: Vec<Vec<Point>> = Vec::new();
    let mut open: Vec<Vec<Point>> = Vec::new();
    for r in load_records(fl) {
        match &r {
            Record::Manual(m) | Record::Built(m) => {
                closed.push(member_outline(m));
                closed.push(footing_outline(m));
            }
            Record::JoistDirection { dir, .. } => open.push(vec![dir.line.0, dir.line.1]),
            Record::TrussDirection { dir, .. } => open.push(vec![dir.line.0, dir.line.1]),
            Record::BearingLine { line, .. } => open.push(vec![line.line.0, line.line.1]),
            Record::TrussBase { base, .. } => closed.push(base.points.clone()),
            Record::Marker { .. } => {}
        }
    }
    DxfExport::new(project, floor, rooms)
        .with_extra_polylines("Roof Planes", roof_plane_outlines(fl))
        .with_extra_polylines("Framing", closed)
        .with_extra_open_polylines("Framing", open)
        .write()
}

// ----- drawing -----

fn layer_rgb(cx: &EditorContext, name: &str) -> [u8; 3] {
    cx.layers().get(name).map_or_else(
        || {
            MANUAL_LAYERS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or([180, 140, 60], |(_, c)| *c)
        },
        |l| l.color,
    )
}

/// Draws the manual members and layout lines of the active floor.
pub fn draw_manual(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let records = manual_records(cx);
    if records.is_empty() {
        return;
    }
    let picked = selected(cx);
    for r in records.iter() {
        if !cx.layers().is_visible(r.layer()) {
            continue;
        }
        let [cr, cg, cb] = layer_rgb(cx, r.layer());
        let color = Color32::from_rgb(cr, cg, cb);
        let stroke = if picked.contains(&r.id()) {
            Stroke::new(2.0_f32, cx.palette.selection)
        } else {
            Stroke::new(0.75_f32, color)
        };
        draw_record(painter, cam, r, color, stroke);
    }
}

fn screen(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

/// Paints one member the way the plan shows it (also used for the tool's
/// ghost).
pub fn paint_member(
    painter: &egui::Painter,
    cam: &Camera,
    m: &FramingMember,
    color: Color32,
    stroke: Stroke,
) {
    let fill = color.gamma_multiply(0.3);
    let foot = screen(cam, &footing_outline(m));
    if foot.len() >= 3 {
        painter.add(Shape::closed_line(
            foot,
            Stroke::new(0.75_f32, color.gamma_multiply(0.7)),
        ));
    }
    let pts = screen(cam, &member_outline(m));
    match pts.len() {
        0 | 1 => {}
        2 => {
            painter.line_segment([pts[0], pts[1]], stroke);
        }
        _ => {
            painter.add(Shape::convex_polygon(pts.clone(), fill, stroke));
            if m.kind.is_vertical() {
                painter.line_segment([pts[0], pts[2.min(pts.len() - 1)]], stroke);
                if pts.len() >= 4 {
                    painter.line_segment([pts[1], pts[3]], stroke);
                }
            } else if m.kind.is_truss() {
                painter.line_segment(
                    [cam.world_to_screen(m.start), cam.world_to_screen(m.end)],
                    Stroke::new(0.5_f32, stroke.color),
                );
            }
        }
    }
}

fn arrow_head(painter: &egui::Painter, tip: Pos2, from: Pos2, color: Color32) {
    let d = tip - from;
    if d.length() < 1.0 {
        return;
    }
    let d = d.normalized();
    let n = egui::vec2(-d.y, d.x);
    let base = tip - d * 9.0;
    painter.add(Shape::convex_polygon(
        vec![tip, base + n * 4.0, base - n * 4.0],
        color,
        Stroke::NONE,
    ));
}

fn dashed(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    painter.extend(Shape::dashed_line(&[a, b], stroke, 8.0, 4.0));
}

/// Length of the direction arrows, inches.
const ARROW_LEN: f64 = 48.0;

fn draw_direction(
    painter: &egui::Painter,
    cam: &Camera,
    line: (Point, Point),
    run: Point,
    spacing: f64,
    color: Color32,
    stroke: Stroke,
) {
    let (a, b) = (cam.world_to_screen(line.0), cam.world_to_screen(line.1));
    dashed(painter, a, b, stroke);
    let mid = Point::lerp(line.0, line.1, 0.5);
    let (t0, t1) = (
        cam.world_to_screen(mid - run * (ARROW_LEN / 2.0)),
        cam.world_to_screen(mid + run * (ARROW_LEN / 2.0)),
    );
    painter.line_segment([t0, t1], stroke);
    arrow_head(painter, t1, t0, color);
    arrow_head(painter, t0, t1, color);
    let text = if spacing > 0.0 {
        format!("{spacing:.0}\" o.c.")
    } else {
        "o.c.".to_string()
    };
    painter.text(
        cam.world_to_screen(mid) + egui::vec2(6.0, -6.0),
        Align2::LEFT_BOTTOM,
        text,
        FontId::proportional(10.0),
        color,
    );
}

fn draw_record(painter: &egui::Painter, cam: &Camera, r: &Record, color: Color32, stroke: Stroke) {
    match r {
        Record::Manual(m) | Record::Built(m) => paint_member(painter, cam, m, color, stroke),
        Record::JoistDirection { dir, .. } => draw_direction(
            painter,
            cam,
            dir.line,
            dir.run(),
            dir.spacing.max(0.0),
            color,
            stroke,
        ),
        Record::TrussDirection { dir, .. } => draw_direction(
            painter,
            cam,
            dir.line,
            dir.run(),
            dir.spacing.max(0.0),
            color,
            stroke,
        ),
        Record::BearingLine { line, .. } => {
            let (a, b) = (
                cam.world_to_screen(line.line.0),
                cam.world_to_screen(line.line.1),
            );
            painter.line_segment([a, b], Stroke::new(stroke.width + 1.5, stroke.color));
            painter.text(
                a.lerp(b, 0.5) + egui::vec2(6.0, -6.0),
                Align2::LEFT_BOTTOM,
                "BEARING",
                FontId::proportional(10.0),
                color,
            );
        }
        Record::Marker { marker, .. } => {
            let c = cam.world_to_screen(marker.point);
            painter.circle_stroke(c, 6.0, stroke);
            let a = marker.angle.to_radians();
            let t = egui::vec2(a.cos() as f32, -a.sin() as f32) * 10.0;
            painter.line_segment([c - t, c + t], stroke);
            painter.line_segment(
                [c - egui::vec2(-t.y, t.x), c + egui::vec2(-t.y, t.x)],
                stroke,
            );
        }
        Record::TrussBase { base, .. } => {
            let pts = screen(cam, &base.points);
            if pts.len() >= 2 {
                for i in 0..pts.len() {
                    dashed(painter, pts[i], pts[(i + 1) % pts.len()], stroke);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{OpeningKind, WallKind};

    fn house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        let mut ids = Vec::new();
        for i in 0..4 {
            ids.push(cx.project.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.5,
                109.125,
                WallKind::Exterior,
            ));
        }
        cx.project
            .add_opening(0, ids[0], 100.0, OpeningKind::Door)
            .unwrap();
        cx.refresh();
        cx
    }

    #[test]
    fn build_frames_walls_and_the_floor_and_undoes() {
        let mut cx = house();
        let s = build(&mut cx, false);
        assert!(s.walls > 40, "{s:?}");
        assert!(s.floor > 10, "{s:?}");
        assert_eq!(s.roof, 0, "no roof planes yet");
        assert_eq!(cx.floor().framing.len(), s.total());
        assert_eq!(cx.framing.len(), s.total(), "the context caches them");
        assert!(count_kind(&cx.framing, MemberKind::Stud) > 10);
        assert!(count_kind(&cx.framing, MemberKind::Header) >= 1);
        assert_eq!(cx.undo_label(), Some("Build Framing"));
        cx.undo();
        assert!(cx.floor().framing.is_empty());
        assert!(cx.framing.is_empty());
        cx.redo();
        assert_eq!(cx.framing.len(), s.total());
    }

    #[test]
    fn roof_planes_are_framed_and_other_floors_wait_for_build_all() {
        let mut cx = house();
        let settings = roof_view::RoofSettings::from_defaults(&cx.defaults);
        roof_view::rebuild(&mut cx.project, 0, settings, false).unwrap();
        assert!(roof_of(cx.floor()).is_some());
        let s = build(&mut cx, false);
        assert!(s.roof > 10, "{s:?}");

        let mut cx = house();
        cx.project.build_new_floor(true);
        let one = build(&mut cx, false);
        assert!(cx.project.floors[1].framing.is_empty());
        let all = build(&mut cx, true);
        assert!(!cx.project.floors[1].framing.is_empty());
        assert!(all.total() > one.total());
    }

    #[test]
    fn build_shows_the_framing_layer_and_clear_deletes() {
        let mut cx = house();
        cx.project.layers.get_mut(LAYER).unwrap().display = false;
        build(&mut cx, false);
        assert!(cx.layers().is_visible(LAYER));
        assert!(clear(&mut cx) > 0);
        assert!(cx.floor().framing.is_empty());
        assert_eq!(cx.undo_label(), Some("Delete Framing"));
        assert_eq!(clear(&mut cx), 0);
    }

    #[test]
    fn invisible_and_divider_walls_are_not_framed() {
        let mut cx = house();
        let (all, _) = frame_floor_members(&cx.project, 0, false);
        let id = cx.project.floors[0].walls[1].id;
        cx.project.floors[0].wall_mut(id).unwrap().flags.invisible = true;
        let (fewer, _) = frame_floor_members(&cx.project, 0, false);
        assert!(fewer.len() < all.len());
    }

    #[test]
    fn takeoff_rows_and_csv() {
        let mut cx = house();
        build(&mut cx, false);
        let members = members_for(&cx.project, 0, false);
        let (cols, rows) = takeoff_table(&members);
        assert_eq!(cols, vec!["Item", "Qty"]);
        assert!(rows.iter().any(|r| r[0].contains("stud")));
        assert_eq!(rows.last().unwrap()[0], "Total board feet");
        let csv = takeoff_csv(&members);
        assert!(csv.starts_with("Item,Qty\n"));
        assert!(csv.lines().count() == rows.len() + 1);
        // Quotes and commas are escaped.
        assert_eq!(csv_cell("2x4 x 8', \"stud\""), "\"2x4 x 8', \"\"stud\"\"\"");
    }

    #[test]
    fn a_stud_outline_is_its_footprint_and_the_plan_draws() {
        let mut cx = house();
        build(&mut cx, false);
        let stud = cx
            .framing
            .iter()
            .find(|m| m.kind == MemberKind::Stud)
            .unwrap();
        let hull = plan_outline(stud);
        assert_eq!(hull.len(), 4);
        let (mut lo, mut hi) = (hull[0], hull[0]);
        for p in &hull {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let (w, h) = (hi.x - lo.x, hi.y - lo.y);
        let (a, b) = (w.min(h), w.max(h));
        assert!(
            (a - 1.5).abs() < 1e-6 && (b - 5.5).abs() < 0.01,
            "{a} x {b}"
        );

        let egui_ctx = egui::Context::default();
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(egui::vec2(400.0, 300.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                draw(&cx, &painter, &cam);
            });
        });
    }

    // ----- manual framing -----

    fn place(cx: &mut EditorContext, kind: ManualMemberKind, a: Point, b: Point) -> Id {
        let m = new_member(cx.floor(), kind, a, b);
        add_record(cx, "Place", |id| Record::Manual(FramingMember { id, ..m }))
    }

    fn built_of(cx: &EditorContext, kind: ManualMemberKind) -> Vec<FramingMember> {
        load_records(cx.floor())
            .into_iter()
            .filter_map(|r| match r {
                Record::Built(m) if m.kind == kind => Some(m),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn manual_members_share_the_slot_with_built_framing() {
        let mut cx = house();
        let joist = place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(10.0, 20.0),
            Point::new(130.0, 20.0),
        );
        let post = place(
            &mut cx,
            ManualMemberKind::PostWithFooting,
            Point::new(60.0, 60.0),
            Point::new(60.0, 60.0),
        );
        assert_eq!(load_records(cx.floor()).len(), 2);
        assert!(cx.framing.is_empty(), "records are not automatic members");
        assert!(cx.layers().is_visible(LAYER_POSTS));

        let s = build(&mut cx, false);
        assert_eq!(
            cx.framing.len(),
            s.total(),
            "no layout lines: all automatic"
        );
        assert_eq!(
            load_records(cx.floor()).len(),
            2,
            "manual members survive a build"
        );
        assert_eq!(
            cx.floor().framing.len(),
            s.total() + 2,
            "one slot, automatic members plus two records"
        );
        // Delete Framing removes what Build made and nothing else.
        assert_eq!(clear(&mut cx), s.total());
        assert_eq!(load_records(cx.floor()).len(), 2);
        assert_eq!(clear(&mut cx), 0);
        assert!(find(cx.floor(), joist).is_some() && find(cx.floor(), post).is_some());

        // Moving, picking and deleting.
        assert_eq!(move_records(&mut cx, &[joist], Point::new(0.0, 5.0)), 1);
        let Some(Record::Manual(m)) = find(cx.floor(), joist) else {
            panic!()
        };
        assert_eq!((m.start.y, m.end.y), (25.0, 25.0));
        assert_eq!(pick(cx.floor(), Point::new(70.0, 25.5), 2.0), Some(joist));
        assert_eq!(pick(cx.floor(), Point::new(60.0, 60.0), 2.0), Some(post));
        assert_eq!(pick(cx.floor(), Point::new(200.0, 150.0), 2.0), None);
        assert_eq!(delete_records(&mut cx, &[joist]), 1);
        assert!(find(cx.floor(), joist).is_none());
        cx.undo();
        assert!(find(cx.floor(), joist).is_some());
    }

    #[test]
    fn old_files_and_foreign_values_survive_the_slot_helpers() {
        let mut cx = house();
        build(&mut cx, false);
        let autos = cx.floor().framing.len();
        let before: Vec<Value> = cx.floor().framing.clone();
        // A file with no records loads unchanged and a store keeps every value.
        assert!(load_records(cx.floor()).is_empty());
        store_records(cx.floor_mut(), &[]);
        assert_eq!(cx.floor().framing, before);
        // A value that is neither a member nor a record is kept, not loaded.
        cx.floor_mut()
            .framing
            .push(serde_json::json!({"future": 1}));
        assert_eq!(load(cx.floor()).len(), autos);
        place(
            &mut cx,
            ManualMemberKind::Blocking,
            Point::new(0.0, 0.0),
            Point::new(14.5, 0.0),
        );
        assert_eq!(cx.floor().framing.len(), autos + 2);
        assert_eq!(load(cx.floor()).len(), autos);
    }

    #[test]
    fn a_joist_direction_line_rotates_the_floor_framing() {
        let mut cx = house();
        // No layout lines: joists across the short side, running along y.
        let plain = build(&mut cx, false);
        let joists = |cx: &EditorContext| -> Vec<plan_framing::Member> {
            cx.framing
                .iter()
                .filter(|m| m.kind == MemberKind::Joist)
                .cloned()
                .collect()
        };
        let before = joists(&cx);
        assert!(before.len() > 5);
        assert!(
            before.iter().all(|m| m.transform.axis_x[2].abs() > 0.99),
            "along y"
        );

        // A direction line along y spaces the joists along y: they run along x.
        add_record(&mut cx, "Dir", |id| Record::JoistDirection {
            id,
            dir: JoistDirectionLine::new((Point::new(100.0, 40.0), Point::new(100.0, 120.0)), 12.0),
        });
        let directed = build(&mut cx, false);
        assert!(joists(&cx).is_empty(), "the room is framed from the line");
        let built = built_of(&cx, ManualMemberKind::Joist);
        assert!(built.len() > 10, "{}", built.len());
        for m in &built {
            assert!((m.start.y - m.end.y).abs() < 1e-6, "{m:?}");
            assert!((m.end.x - m.start.x).abs() > 1.0);
            assert_eq!(m.layer_name, LAYER_JOISTS);
            assert!(m.id > 0, "ids come from the project");
        }
        let (a, b) = (
            built
                .iter()
                .map(|m| m.start.y)
                .fold(f64::INFINITY, f64::min),
            built.iter().map(|m| m.start.y).fold(0.0, f64::max),
        );
        assert!(b - a > 150.0, "spaced along y");
        assert!(directed.walls == plain.walls && directed.roof == plain.roof);
        assert_eq!(directed.floor, built.len());
        // 12" spacing is closer than the default 16".
        let ys: Vec<f64> = {
            let mut v: Vec<f64> = built.iter().map(|m| m.start.y).collect();
            v.sort_by(f64::total_cmp);
            v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            v
        };
        assert!(ys.windows(2).all(|w| w[1] - w[0] <= 12.0 + 1e-6));

        // Rebuilding replaces the built members (no duplicates).
        build(&mut cx, false);
        assert_eq!(built_of(&cx, ManualMemberKind::Joist).len(), built.len());
        // Undo restores the previous build.
        cx.undo();
        assert_eq!(built_of(&cx, ManualMemberKind::Joist).len(), built.len());
    }

    #[test]
    fn bearing_lines_split_joists_and_markers_move_the_stud_grid() {
        let mut cx = house();
        add_record(&mut cx, "Dir", |id| Record::JoistDirection {
            id,
            dir: JoistDirectionLine::new((Point::new(100.0, 40.0), Point::new(100.0, 120.0)), 16.0),
        });
        build(&mut cx, false);
        let n0 = built_of(&cx, ManualMemberKind::Joist).len();
        add_record(&mut cx, "Bearing", |id| Record::BearingLine {
            id,
            line: BearingLine {
                line: (Point::new(120.0, 5.0), Point::new(120.0, 185.0)),
            },
        });
        build(&mut cx, false);
        assert!(
            built_of(&cx, ManualMemberKind::Joist).len() > n0,
            "joists split"
        );
        let beams = built_of(&cx, ManualMemberKind::FloorCeilingBeam);
        assert_eq!(beams.len(), 1);
        assert!((beams[0].start.x - 120.0).abs() < 1e-6);

        // A marker near the bottom wall re-anchors its studs.
        let studs = |cx: &EditorContext| -> Vec<i64> {
            let wall = cx.floor().walls[0].id;
            let mut v: Vec<i64> = cx
                .framing
                .iter()
                .filter(|m| m.kind == MemberKind::Stud && m.wall_id == Some(wall))
                .map(|m| (m.transform.origin[0] * 100.0).round() as i64)
                .collect();
            v.sort_unstable();
            v
        };
        let plain = studs(&cx);
        assert!(!plain.is_empty());
        add_record(&mut cx, "Marker", |id| Record::Marker {
            id,
            marker: ReferenceMarker {
                point: Point::new(23.0, 8.0),
                angle: 0.0,
            },
        });
        build(&mut cx, false);
        let anchored = studs(&cx);
        assert_ne!(plain, anchored, "the marker moved the grid");
    }

    #[test]
    fn a_truss_base_and_direction_lay_out_roof_trusses() {
        let mut cx = house();
        let rect = vec![
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        add_record(&mut cx, "Base", |id| Record::TrussBase {
            id,
            base: TrussBase::new(rect, 109.125),
        });
        let s = build(&mut cx, false);
        let trusses = built_of(&cx, ManualMemberKind::RoofTruss);
        assert!(trusses.len() >= 8, "{}", trusses.len());
        assert_eq!(s.roof, trusses.len());
        // Default direction: spaced along the long side, spanning the short one.
        assert!(trusses
            .iter()
            .all(|t| (t.plan_length() - 192.0).abs() < 1e-6));
        // A direction line along y spaces them along y at 48": they run along x.
        add_record(&mut cx, "TDir", |id| Record::TrussDirection {
            id,
            dir: RoofTrussDirection::new((Point::new(120.0, 20.0), Point::new(120.0, 100.0)), 48.0),
        });
        build(&mut cx, false);
        let t2 = built_of(&cx, ManualMemberKind::RoofTruss);
        assert!(!t2.is_empty() && t2.len() < trusses.len());
        assert!(t2.iter().all(|t| (t.plan_length() - 240.0).abs() < 1e-6));
    }

    #[test]
    fn takeoff_and_material_list_include_manual_members() {
        let mut cx = house();
        let none = takeoff_data(&cx.project, 0, false);
        assert_eq!(none.members, 0);
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
        );
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(0.0, 16.0),
            Point::new(120.0, 16.0),
        );
        place(
            &mut cx,
            ManualMemberKind::Post,
            Point::new(5.0, 5.0),
            Point::new(5.0, 5.0),
        );
        let t = takeoff_data(&cx.project, 0, false);
        assert_eq!(t.members, 3);
        let qty = |name: &str| t.rows.iter().find(|r| r[0] == name).map(|r| r[1].clone());
        assert_eq!(qty("2x10 x 120\" joist").as_deref(), Some("2"));
        assert!(t.rows.iter().any(|r| r[0].contains("post")));
        assert_eq!(t.rows.last().unwrap()[0], "Total board feet");
        assert!(t.csv.starts_with("Item,Qty\n") && t.csv.contains("joist"));
        assert!(t.material_csv.starts_with("Size,Material,Length"));
        assert!(t.material_csv.contains("2x10"));
        assert!(t.material_csv.lines().last().unwrap().starts_with("Total"));

        // With automatic framing too, the totals add up.
        build(&mut cx, false);
        let both = takeoff_data(&cx.project, 0, false);
        assert_eq!(both.members, cx.framing.len() + 3);
        let bf = |t: &TakeoffData| -> f64 { t.rows.last().unwrap()[1].parse().unwrap() };
        assert!(bf(&both) > bf(&t));
        assert_eq!(
            takeoff_data(&cx.project, 0, true).members,
            both.members,
            "one floor"
        );
    }

    #[test]
    fn dxf_carries_roof_planes_and_manual_framing() {
        let mut cx = house();
        let settings = roof_view::RoofSettings::from_defaults(&cx.defaults);
        roof_view::rebuild(&mut cx.project, 0, settings, false).unwrap();
        let planes = roof_plane_outlines(cx.floor());
        assert!(!planes.is_empty());
        let base = plan_core::write_dxf(&cx.project, 0, &[]);
        let count = |s: &str| s.lines().filter(|l| *l == "POLYLINE").count();

        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(10.0, 10.0),
            Point::new(130.0, 10.0),
        );
        add_record(&mut cx, "Bearing", |id| Record::BearingLine {
            id,
            line: BearingLine {
                line: (Point::new(20.0, 0.0), Point::new(20.0, 90.0)),
            },
        });
        let dxf = floor_dxf(&cx.project, 0, &[]);
        assert_eq!(count(&dxf), count(&base) + planes.len() + 2);
        let lines: Vec<&str> = dxf.lines().collect();
        for layer in ["Roof Planes", "Framing"] {
            assert!(
                lines.chunks(2).any(|c| c[0] == "8" && c[1] == layer),
                "{layer} is used by an entity"
            );
            assert!(
                lines.chunks(2).any(|c| c[0] == "2" && c[1] == layer),
                "{layer} is in the layer table"
            );
        }
    }

    #[test]
    fn manual_members_make_3d_meshes_unless_their_layer_is_hidden() {
        let mut cx = house();
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
        );
        place(
            &mut cx,
            ManualMemberKind::PostWithFooting,
            Point::new(60.0, 60.0),
            Point::new(60.0, 60.0),
        );
        place(
            &mut cx,
            ManualMemberKind::RoofTruss,
            Point::new(0.0, 100.0),
            Point::new(288.0, 100.0),
        );
        let meshes = manual_framing_meshes(&cx.project);
        // Joist (1), post and footing (2), a Fink truss (9 members).
        assert_eq!(meshes.len(), 1 + 2 + 9);
        assert!(meshes.iter().all(|m| m.triangle_count() == 12));
        assert!(meshes.iter().all(|m| m.object_id.is_some()));
        cx.project.layers.set_display(LAYER_TRUSSES, false);
        assert_eq!(manual_framing_meshes(&cx.project).len(), 3);
        // Layout lines have no lumber.
        add_record(&mut cx, "Bearing", |id| Record::BearingLine {
            id,
            line: BearingLine {
                line: (Point::ZERO, Point::new(50.0, 0.0)),
            },
        });
        assert_eq!(manual_framing_meshes(&cx.project).len(), 3);
    }

    #[test]
    fn the_specification_edit_replaces_the_member_and_keeps_a_built_one() {
        let mut cx = house();
        let id = place(
            &mut cx,
            ManualMemberKind::Rafter,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        );
        let Some(Record::Manual(mut m)) = find(cx.floor(), id) else {
            panic!()
        };
        m = m.with_lumber(plan_framing::LumberSize::TWO_BY_TWELVE);
        assert!(apply_edit(&mut cx, m.clone()));
        assert!(!apply_edit(&mut cx, m.clone()), "no change, no undo step");
        let Some(Record::Manual(back)) = find(cx.floor(), id) else {
            panic!()
        };
        assert_eq!(back.lumber, plan_framing::LumberSize::TWO_BY_TWELVE);
        assert_eq!(cx.undo_label(), Some("Framing Member Specification"));
        assert!(!apply_edit(&mut cx, FramingMember { id: 99_999, ..m }));
    }
}
