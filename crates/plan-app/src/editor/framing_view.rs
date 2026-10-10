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
//!   on the floor anchors its studs, per floor: see [`reference_marker`]); every room gets a
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

pub mod commands;
pub mod details;
pub mod selected;
pub mod trusses;

pub use commands::{cmd, edit_actions, run_command};
pub use details::{
    configs as truss_configs_of, detail_wall, find_config, in_wall_detail, open_truss_detail,
    refresh_details, wall_details,
};
pub use selected::{build_parents, build_selected, Outcome};
pub use trusses::{LAYER_FLOOR_TRUSS_LABELS, LAYER_ROOF_TRUSS_LABELS};

use super::{Camera, EditorContext, ObjectRef};
use crate::editor::roof_view;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape, Stroke};
use plan_core::foundation::{FoundationLayer, PlatformKind};
use plan_core::geometry::{dist_to_segment, point_in_polygon, Point};
use plan_core::{
    detect_rooms, DxfExport, Floor, FloorKind, Id, Layer, Opening, Project, Room, Wall,
};
use plan_framing::span::{check_spans, SpanIssue};
use plan_framing::{
    fingerprint, frame_floor_supported, frame_roof_eaves, frame_wall_with, group_of,
    group_of_manual, layout_trusses, manual_takeoff, merge_groups, merge_rebuild,
    roof_framing_takeoff, BearingLine, BearingMode, BuildOptions, EaveSpec, FramingDefaults,
    FramingMember, Group, JoistDirectionLine, ManualMemberKind, MaterialList, Member, MemberKind,
    ReferenceMarker, RoofFramingDefaults, RoofTrussDirection, TailCut, Takeoff, TrussBase,
    TrussDefaults, TrussSpec, WallJoints, TRAY_MEMBER_FLAG,
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
    /// Ceiling joists (the Ceiling group of Build Framing).
    pub ceiling: usize,
}

impl BuildSummary {
    pub fn total(&self) -> usize {
        self.walls + self.floor + self.roof + self.ceiling
    }

    fn add(&mut self, o: BuildSummary) {
        self.walls += o.walls;
        self.floor += o.floor;
        self.roof += o.roof;
        self.ceiling += o.ceiling;
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

// ----- Framing Defaults -----

/// Key of the single-key object that stores the framing defaults in the
/// first floor's `framing` slot. It is not a [`Record`] and not a member, so
/// every reader that does not know it skips it.
const SETTINGS_KEY: &str = "FramingSettings";

/// The framing defaults of the plan (Default Settings > Framing): the wall
/// and floor assembly and the roof framing, one size and spacing per member
/// kind. A plan that never opened the page uses [`FramingSettings::default`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FramingSettings {
    /// Walls and floors ([`FramingDefaults::house`]: corner and tee studs and
    /// 48" wall blocking on).
    pub walls: FramingDefaults,
    pub roof: RoofFramingDefaults,
    /// The Build Framing dialog: which groups to build, Retain Framing,
    /// auto rebuild, the Posts and Trusses tabs.
    pub build: BuildOptions,
    /// Set by the Build Framing dialog: [`set_settings`] builds after storing
    /// (`Some(true)` every floor, `Some(false)` the active floor). Never saved.
    #[serde(skip)]
    pub build_on_ok: Option<bool>,
    /// The names of the building floors, in floor order, for the floor choices
    /// of Build Framing Once. Filled by [`settings`]; never saved.
    #[serde(skip)]
    pub floor_names: Vec<String>,
}

/// Settings are equal when what is saved is: the floor names are only for the
/// dialog's floor choices.
impl PartialEq for FramingSettings {
    fn eq(&self, o: &Self) -> bool {
        self.walls == o.walls
            && self.roof == o.roof
            && self.build == o.build
            && self.build_on_ok == o.build_on_ok
    }
}

impl Default for FramingSettings {
    fn default() -> Self {
        Self {
            walls: FramingDefaults::house(),
            roof: RoofFramingDefaults::default(),
            build: BuildOptions::default(),
            build_on_ok: None,
            floor_names: Vec::new(),
        }
    }
}

/// The framing defaults stored in `project` (the defaults when none).
pub fn settings(project: &Project) -> FramingSettings {
    project
        .floors
        .first()
        .and_then(|f| {
            f.framing
                .iter()
                .find_map(|v| v.as_object()?.get(SETTINGS_KEY).cloned())
        })
        .and_then(|v| serde_json::from_value::<FramingSettings>(v).ok())
        .map(|mut st| {
            st.floor_names = project
                .floors
                .iter()
                .take_while(|f| !f.is_cad_detail())
                .map(|f| f.name.clone())
                .collect();
            st
        })
        .unwrap_or_else(|| FramingSettings {
            floor_names: project
                .floors
                .iter()
                .take_while(|f| !f.is_cad_detail())
                .map(|f| f.name.clone())
                .collect(),
            ..FramingSettings::default()
        })
}

/// Is `v` the stored settings object?
fn is_settings(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == 1 && o.contains_key(SETTINGS_KEY))
}

/// Stores the framing defaults as one undo step. Existing framing stays as
/// built until the next Build Framing.
pub fn set_settings(cx: &mut EditorContext, mut new: FramingSettings) {
    // The Build Framing dialog: store its options and build, as one step.
    if let Some(all) = new.build_on_ok.take() {
        build_with(cx, all, Some(new));
        return;
    }
    if settings(&cx.project) == new {
        return;
    }
    cx.begin_change("Framing Defaults");
    store_settings(&mut cx.project, &new);
    cx.mark_dirty();
    cx.status = "Framing defaults saved; Build Framing applies them".into();
}

/// Writes the settings into the first floor's `framing` slot, with no undo
/// step of its own (a build or an auto rebuild records its fingerprints
/// inside the step it belongs to).
fn store_settings(project: &mut Project, new: &FramingSettings) {
    let floor = &mut project.floors[0];
    floor.framing.retain(|v| !is_settings(v));
    if let Ok(v) = serde_json::to_value(new) {
        let mut o = serde_json::Map::new();
        o.insert(SETTINGS_KEY.to_string(), v);
        floor.framing.push(Value::Object(o));
    }
}

/// The Floor Hole outlines of `floor`'s platform (stairwells and hand-made
/// holes in the floor framing).
pub fn floor_holes(floor: &Floor) -> Vec<Vec<Point>> {
    floor
        .foundation_as::<FoundationLayer>()
        .ok()
        .flatten()
        .map(|l| l.platform_hole_outlines(PlatformKind::Floor))
        .unwrap_or_default()
}

/// The eave of every roof plane stored on `floor`, in plane order: its
/// horizontal overhang and the tail cut of its Eave settings (the plane's own
/// choice, else the roof detail's, else plumb).
pub fn eave_specs(floor: &Floor) -> Vec<EaveSpec> {
    let set = roof_view::load(floor);
    let roof_cut = set
        .settings
        .as_ref()
        .map_or(plan_core::defaults::EaveCut::Plumb, |s| s.detail.eave_cut);
    set.planes
        .iter()
        .map(|p| {
            let cut = p.eave.eave_cut.unwrap_or(roof_cut);
            EaveSpec {
                overhang: p.overhang,
                cut: Some(match cut {
                    plan_core::defaults::EaveCut::Plumb => TailCut::Plumb,
                    plan_core::defaults::EaveCut::Level => TailCut::Level,
                    plan_core::defaults::EaveCut::Square => TailCut::Square,
                }),
            }
        })
        .collect()
}

/// A region of the plan whose framing a build leaves alone: a room whose
/// floor and ceiling framing is retained, a tray ceiling, a roof plane. Old
/// members of `groups` whose centre lies in `polygon` stay as they are, and
/// the build makes none there.
#[derive(Clone, Debug, PartialEq)]
pub struct Protected {
    pub groups: Vec<Group>,
    pub polygon: Vec<Point>,
}

/// What Build Framing makes for one floor.
#[derive(Clone, Debug, Default)]
pub struct FloorBuild {
    /// Automatic members (walls, undirected floors, ceilings, roofs).
    pub members: Vec<Member>,
    /// Members made from layout lines (directed joists, beams, trusses); their
    /// ids are 0 until [`build`] stores them.
    pub built: Vec<FramingMember>,
    pub summary: BuildSummary,
    /// Joists and rafters that carry more than their size allows
    /// ([`plan_framing::span`]), as sentences for the build report.
    pub warnings: Vec<String>,
    /// Regions Retain Framing protects.
    pub protected: Vec<Protected>,
}

/// Plan position of the middle of an automatic member.
pub fn member_center(m: &Member) -> Point {
    let c = [
        m.transform.origin[0] + m.transform.axis_x[0] * m.length / 2.0,
        m.transform.origin[2] + m.transform.axis_x[2] * m.length / 2.0,
    ];
    Point::new(c[0], -c[1])
}

fn is_protected(group: Group, at: Point, protected: &[Protected]) -> bool {
    protected
        .iter()
        .any(|p| p.groups.contains(&group) && point_in_polygon(at, &p.polygon))
}

/// The layout lines of a floor, grouped for the generators.
struct Layout {
    directions: Vec<JoistDirectionLine>,
    truss_directions: Vec<RoofTrussDirection>,
    bearing: Vec<BearingLine>,
    bases: Vec<TrussBase>,
    /// The manual floor and ceiling beams of the floor.
    beams: Vec<FramingMember>,
}

impl Layout {
    fn from_records(records: &[Record]) -> Self {
        let mut l = Layout {
            directions: Vec::new(),
            truss_directions: Vec::new(),
            bearing: Vec::new(),
            bases: Vec::new(),
            beams: Vec::new(),
        };
        for r in records {
            match r {
                Record::JoistDirection { dir, .. } => l.directions.push(dir.clone()),
                Record::TrussDirection { dir, .. } => l.truss_directions.push(dir.clone()),
                Record::BearingLine { line, .. } => l.bearing.push(*line),
                Record::TrussBase { base, .. } => l.bases.push(base.clone()),
                Record::Manual(m) if m.kind == ManualMemberKind::FloorCeilingBeam => {
                    l.beams.push(m.clone())
                }
                Record::Marker { .. } | Record::Manual(_) | Record::Built(_) => {}
            }
        }
        l
    }

    fn direction_in(&self, poly: &[Point]) -> Option<JoistDirectionLine> {
        self.directions
            .iter()
            .find(|d| point_in_polygon(Point::lerp(d.line.0, d.line.1, 0.5), poly))
            .cloned()
    }

    /// The Bearing Lines drawn through the platform and the Bearing Beams
    /// that cross it.
    fn bearing_in(&self, poly: &[Point], joist_bottom: f64) -> Vec<BearingLine> {
        let touches = |a: Point, b: Point| {
            [a, b, Point::lerp(a, b, 0.5)]
                .iter()
                .any(|p| point_in_polygon(*p, poly))
        };
        let mut out: Vec<BearingLine> = self
            .bearing
            .iter()
            .filter(|b| touches(b.line.0, b.line.1))
            .copied()
            .collect();
        for beam in self.beams.iter().filter(|b| b.bearing_beam) {
            if !touches(beam.start, beam.end) {
                continue;
            }
            // A beam that stands 1" or more above the joists' bottoms holds
            // them by its sides.
            let top = beam.elevation_bottom + beam.depth;
            let hang = top - joist_bottom >= 1.0;
            out.push(BearingLine::beam((beam.start, beam.end), beam.width, hang));
        }
        out
    }
}

/// The Framing Reference Marker of floor `fi`: the first one drawn on the
/// floor, else the first one drawn on the first floor (a single marker for the
/// whole model sits there). Markers made later are ignored (manual p. 919).
pub fn reference_marker(project: &Project, fi: usize) -> Option<ReferenceMarker> {
    let first = |floor: &Floor| {
        load_records(floor).into_iter().find_map(|r| match r {
            Record::Marker { marker, .. } => Some(marker),
            _ => None,
        })
    };
    project.floors.get(fi).and_then(first).or_else(|| {
        project
            .floors
            .first()
            .filter(|f| !f.is_cad_detail())
            .and_then(first)
    })
}

/// The marker floor `fi`'s layout starts from, or `None` when its Use Framing
/// Reference is off or there is no marker.
pub fn reference_point(project: &Project, fi: usize) -> Option<Point> {
    settings(project)
        .build
        .uses_reference(fi)
        .then(|| reference_marker(project, fi))
        .flatten()
        .map(|m| m.point)
}

/// The outlines of the Open Below rooms of `above` (rooms with no floor
/// platform): the floor under them has no ceiling there.
fn open_below_outlines(above: Option<&Floor>) -> Vec<Vec<Point>> {
    let Some(above) = above else {
        return Vec::new();
    };
    if above.room_names.iter().all(|n| n.has_floor) {
        return Vec::new();
    }
    detect_rooms(&above.walls, 0.5)
        .into_iter()
        .filter(|r| {
            r.name_entry(&above.room_names)
                .is_some_and(|n| !n.has_floor)
        })
        .map(|r| r.polygon)
        .collect()
}

/// The Framing Group of a room: the group of its name entry, 0 when unnamed.
fn group_of_room(floor: &Floor, room: &Room) -> u32 {
    room.name_entry(&floor.room_names)
        .map_or(0, |n| n.options.framing_group)
}

/// The interior walls that stop a platform: a wall with rooms of different
/// Framing Groups on its two sides, which splits the platform there.
fn group_dividers(floor: &Floor, rooms: &[Room]) -> Vec<Id> {
    let side_group = |p: Point| {
        rooms
            .iter()
            .find(|r| point_in_polygon(p, &r.polygon))
            .map(|r| group_of_room(floor, r))
    };
    floor
        .walls
        .iter()
        .filter(|w| w.kind != plan_core::WallKind::Exterior)
        .filter(|w| {
            let mid = Point::lerp(w.start, w.end, 0.5);
            let n = w.normal() * (w.thickness / 2.0 + 4.0);
            match (side_group(mid + n), side_group(mid - n)) {
                (Some(a), Some(b)) => a != b,
                _ => false,
            }
        })
        .map(|w| w.id)
        .collect()
}

/// The rooms whose platforms are framed on `floor`: every room, or with
/// [`BearingMode::ExteriorAndBearingLines`] the rooms the exterior walls
/// alone enclose (interior partitions do not carry joists), cut by the walls
/// that separate rooms of different Framing Groups.
fn platform_rooms(floor: &Floor, d: &FramingDefaults) -> Vec<Room> {
    if d.bearing == BearingMode::ExteriorAndBearingLines {
        let every = detect_rooms(&floor.walls, 0.5);
        let dividers = group_dividers(floor, &every);
        let platform_walls: Vec<Wall> = floor
            .walls
            .iter()
            .filter(|w| w.kind == plan_core::WallKind::Exterior || dividers.contains(&w.id))
            .cloned()
            .collect();
        let rooms = detect_rooms(&platform_walls, 0.5);
        if !rooms.is_empty() {
            return rooms;
        }
    }
    detect_rooms(&floor.walls, 0.5)
}

/// The interior Bearing Walls of `floor` as Bearing Lines through their
/// centres, for the platforms that cross them. Only the mode where interior
/// walls do not already split the platform needs them.
fn bearing_wall_lines(floor: &Floor, d: &FramingDefaults) -> Vec<BearingLine> {
    if d.bearing != BearingMode::ExteriorAndBearingLines {
        return Vec::new();
    }
    floor
        .walls
        .iter()
        .filter(|w| w.spec.structure.bearing_wall && w.kind != plan_core::WallKind::Exterior)
        .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
        .map(|w| {
            let dir = w.direction();
            BearingLine::wall((w.start - dir * 12.0, w.end + dir * 12.0))
        })
        .collect()
}

/// Whether the room entry under `poly` says to retain its floor and ceiling
/// framing.
fn room_retained(floor: &Floor, poly: &[Point]) -> bool {
    floor
        .room_names
        .iter()
        .any(|n| n.options.retain_framing && point_in_polygon(n.anchor, poly))
}

/// Frames floor `fi` of `project`: the walls, the floor platforms, the
/// ceilings, the roof stored on that floor (with `with_roof`) and the trusses
/// of its Truss Bases, honoring Joist Direction, Bearing Line, Reference
/// Marker and Truss Base lines and the Build Framing options. A group that is
/// off or retained is not built (a build keeps what that group has), nor is a
/// wall whose framing is retained. Rooms with no floor platform (Open Below,
/// decks) get no floor joists, and rooms with no ceiling, or open to the
/// floor above, get no ceiling joists.
pub fn frame_floor_all(project: &Project, fi: usize, with_roof: bool) -> FloorBuild {
    let st = settings(project);
    frame_floor_all_with(project, fi, with_roof, &st, &st.build)
}

/// [`frame_floor_all`] with the options a Build Framing Once resolved to for
/// this floor (`opts`: the groups it makes) instead of the saved ones.
pub fn frame_floor_all_with(
    project: &Project,
    fi: usize,
    with_roof: bool,
    st: &FramingSettings,
    opts: &BuildOptions,
) -> FloorBuild {
    let floor = &project.floors[fi];
    let d = &st.walls;
    let mut roof_d = st.roof.clone();
    let detail = &opts.detail;
    let layout = Layout::from_records(&load_records(floor));
    let reference = opts
        .uses_reference(fi)
        .then(|| reference_marker(project, fi))
        .flatten();
    let mut out = FloorBuild::default();

    let framed: Vec<Wall> = floor
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
        .cloned()
        .collect();
    for wall in framed.iter().filter(|w| opts.builds_wall(w.id)) {
        let openings: Vec<&Opening> = floor.openings_on(wall.id).collect();
        let origin = reference.map(|m| (m.point - wall.start).dot(wall.direction()));
        let joints = WallJoints::of(wall, &framed);
        let m = frame_wall_with(
            wall,
            &openings,
            floor.elevation,
            d,
            origin,
            &joints,
            detail,
            wall.spec.structure.bearing_wall,
        );
        out.summary.walls += m.len();
        out.members.extend(m);
    }
    if !opts.group_frozen(Group::Wall) {
        posts_under_beams(project, fi, &framed, opts, &mut out);
    }
    let floor_group = !opts.group_frozen(Group::Floor);
    let ceiling_group = !opts.group_frozen(Group::Ceiling)
        && !roof_d.ceiling_joists
        && !roof_d.trusses
        && floor.kind != FloorKind::Foundation;
    if (floor_group || ceiling_group) && floor.kind != FloorKind::Foundation {
        let open = open_below_outlines(project.floors.get(fi + 1));
        let plate = floor.elevation + wall_height(floor);
        let wall_bearing = bearing_wall_lines(floor, d);
        let joist_bottom = floor.elevation - SUBFLOOR - d.joist_size.depth;
        for room in platform_rooms(floor, d) {
            if room.area_sq_ft() < 1.0 {
                continue;
            }
            // Retain Floor/Ceiling Framing on the room: nothing is built there
            // and what stands stays.
            if room_retained(floor, &room.polygon) {
                out.protected.push(Protected {
                    groups: vec![Group::Floor, Group::Ceiling],
                    polygon: room.polygon.clone(),
                });
                continue;
            }
            let named = room.name_entry(&floor.room_names);
            if floor_group && named.is_none_or(|n| n.has_floor) {
                let mut dir = layout.direction_in(&room.polygon);
                let mut bearing = layout.bearing_in(&room.polygon, joist_bottom);
                bearing.extend(wall_bearing.iter().filter(|b| {
                    [b.line.0, b.line.1, Point::lerp(b.line.0, b.line.1, 0.5)]
                        .iter()
                        .any(|p| point_in_polygon(*p, &room.polygon))
                }));
                // Without a Joist Direction line the joists run across the
                // longest bearing wall or beam.
                if dir.is_none() {
                    if let Some(b) = bearing
                        .iter()
                        .max_by(|a, b| a.line.0.dist(a.line.1).total_cmp(&b.line.0.dist(b.line.1)))
                    {
                        dir = Some(JoistDirectionLine::new(b.line, 0.0));
                    }
                }
                let marker_point = reference.map(|m| m.point);
                if dir.is_none() && bearing.is_empty() {
                    // Stairwells in this floor's platform get headers and trimmers.
                    let holes: Vec<Vec<Point>> = floor_holes(floor)
                        .into_iter()
                        .filter(|h| h.iter().all(|p| point_in_polygon(*p, &room.polygon)))
                        .collect();
                    let m = plan_framing::frame_floor_ref(
                        &room,
                        floor.elevation,
                        d,
                        d.joist_direction,
                        &holes,
                        detail,
                        false,
                        marker_point,
                    );
                    out.summary.floor += m.len();
                    out.members.extend(m);
                } else {
                    let m = frame_floor_supported(
                        &room,
                        floor.elevation,
                        d,
                        dir.as_ref(),
                        &bearing,
                        reference.as_ref(),
                        detail.splice,
                        0,
                    );
                    out.summary.floor += m.len();
                    out.built.extend(m);
                }
            }
            let covered = open
                .iter()
                .any(|h| plan_core::rooms::polygon_inside(&room.polygon, h, 1.0));
            if ceiling_group && named.is_none_or(|n| n.has_ceiling) && !covered {
                let m = plan_framing::frame_ceiling_ref(
                    &room,
                    plate,
                    d,
                    detail,
                    reference.map(|m| m.point),
                );
                out.summary.ceiling += m.len();
                out.members.extend(m);
            }
        }
        // Tray ceilings: the side walls and joists of every tray that has no
        // Caution and does not retain its framing.
        if ceiling_group {
            for g in plan_3d::tray::floor_trays(floor) {
                let Some(rec) = floor.trays.get(g.id).cloned() else {
                    continue;
                };
                if rec.retain_framing {
                    out.protected.push(Protected {
                        groups: vec![Group::Ceiling],
                        polygon: g.outer.clone(),
                    });
                    continue;
                }
                let mut m = plan_framing::frame_tray_ceiling(&g, &rec, floor.elevation, d);
                for x in &mut m {
                    x.wall_id = Some(TRAY_MEMBER_FLAG | g.id);
                }
                out.summary.ceiling += m.len();
                out.members.extend(m);
            }
        }
    }
    if with_roof && !opts.group_frozen(Group::Roof) {
        roof_d.reference = opts
            .roof_reference
            .then(|| reference_marker(project, fi))
            .flatten()
            .map(|m| m.point);
        if let Some(roof) = roof_of(floor) {
            // Retain Framing on a roof plane keeps the framing over it.
            for p in roof_view::load(floor)
                .planes
                .iter()
                .filter(|p| opts.retain_planes.contains(&p.id))
            {
                out.protected.push(Protected {
                    groups: vec![Group::Roof],
                    polygon: p
                        .polygon3d
                        .iter()
                        .map(|v| Point::new(v[0], -v[2]))
                        .collect(),
                });
            }
            let m = frame_roof_eaves(&roof, &roof_d, &eave_specs(floor));
            out.summary.roof += m.len();
            out.members.extend(m);
        }
        for base in &layout.bases {
            let m = truss_layout(base, &layout, &opts.trusses, roof_d.reference);
            out.summary.roof += m.len();
            out.built.extend(m);
        }
    }
    out.warnings = check_spans(&out.members, d, &roof_d)
        .iter()
        .map(SpanIssue::message)
        .collect();
    out
}

/// Posts under the Floor/Ceiling Beams that cross the walls of floor `fi`: the
/// beams drawn on the floor above (they sit in its platform) and the ceiling
/// beams of this floor. A post stands where a beam crosses a wall, as tall as
/// the wall's studs, and the wall's studs that it stands among are left out.
fn posts_under_beams(
    project: &Project,
    fi: usize,
    walls: &[Wall],
    opts: &BuildOptions,
    out: &mut FloorBuild,
) {
    let floor = &project.floors[fi];
    let mut beams: Vec<FramingMember> = Vec::new();
    if let Some(above) = project.floors.get(fi + 1).filter(|f| !f.is_cad_detail()) {
        beams.extend(
            manual_members(above)
                .into_iter()
                .filter(|m| m.kind == ManualMemberKind::FloorCeilingBeam),
        );
    }
    let plate = top_plate(floor);
    beams.extend(
        manual_members(floor)
            .into_iter()
            .filter(|m| m.kind == ManualMemberKind::FloorCeilingBeam)
            .filter(|m| m.elevation_bottom + m.depth >= plate - 6.0),
    );
    if beams.is_empty() {
        return;
    }
    let p = &opts.posts;
    for wall in walls.iter().filter(|w| opts.builds_wall(w.id)) {
        let (a, b) = (wall.start, wall.end);
        for beam in &beams {
            let Some(at) = segment_crossing(a, b, beam.start, beam.end) else {
                continue;
            };
            let mut post = new_member(floor, ManualMemberKind::Post, at, at);
            post.lumber = p.size;
            post.depth = p.size.depth();
            post.width = p.size.width();
            post.material = p.material;
            post.height = (wall.height - 4.5).max(12.0);
            post.elevation_bottom = floor.elevation + 1.5;
            // The studs the post stands among are not made.
            let s = (at - a).dot(wall.direction());
            let reach = (post.width + 1.5) / 2.0 + 0.01;
            out.members.retain(|m| {
                !(m.wall_id == Some(wall.id)
                    && m.kind == MemberKind::Stud
                    && ((m.transform.origin[0] - a.x) * wall.direction().x
                        - (m.transform.origin[2] + a.y) * wall.direction().y
                        - s)
                        .abs()
                        < reach)
            });
            out.built.push(post);
            out.summary.walls += 1;
        }
    }
}

/// Where segments `a`-`b` and `c`-`d` cross, if they do.
fn segment_crossing(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    let (r, s) = (b - a, d - c);
    let den = r.cross(s);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = (c - a).cross(s) / den;
    let u = (c - a).cross(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}

/// Trusses over one base: its Roof Truss Direction (the one whose middle lies
/// in the base, else spaced across the longer side) and Reference Marker,
/// of the Trusses tab's type, pitch, heel, overhang and spacing.
fn truss_layout(
    base: &TrussBase,
    layout: &Layout,
    tr: &TrussDefaults,
    reference: Option<Point>,
) -> Vec<FramingMember> {
    let dir = layout
        .truss_directions
        .iter()
        .find(|d| point_in_polygon(Point::lerp(d.line.0, d.line.1, 0.5), &base.points))
        .cloned()
        .unwrap_or_else(|| {
            let (lo, hi) = bounds(&base.points);
            let along_x = hi.x - lo.x >= hi.y - lo.y;
            let (a, b) = if along_x {
                (lo, Point::new(hi.x, lo.y))
            } else {
                (lo, Point::new(lo.x, hi.y))
            };
            RoofTrussDirection::new((a, b), tr.spacing)
        });
    let marker = reference.map(|point| ReferenceMarker { point, angle: 0.0 });
    let mut spec = TrussSpec::new(tr.kind, 0.0, tr.pitch);
    spec.heel_height = tr.heel_height;
    spec.overhang = tr.overhang;
    // The Roof Truss Direction's Specification sizes the trusses it covers.
    let thickness = spec.chord.thickness;
    let depth = |v: f64| {
        (v > 0.0).then_some(plan_framing::Lumber {
            thickness,
            depth: v,
        })
    };
    if let Some(l) = depth(dir.top_chord_depth) {
        spec.chord = l;
    }
    if dir.bottom_chord_depth > 0.0 {
        spec.bottom_chord_depth = dir.bottom_chord_depth;
    }
    if let Some(l) = depth(dir.web_depth) {
        spec.web = l;
    }
    spec.max_span_top = dir.max_span;
    spec.max_span_bottom = dir.max_span;
    spec.require_kingpost = dir.require_kingpost;
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
/// undo step, with the options of the Build Framing dialog: the groups that
/// are off or retained keep their members, retained walls keep theirs, and
/// everything else is replaced. Turns the framing layers on when the options
/// say so (the layers are off in a new plan, like Chief's). Manual members and
/// layout lines stay; members an earlier build made from them are replaced.
/// The status line reports spans the lumber does not carry.
pub fn build(cx: &mut EditorContext, all_floors: bool) -> BuildSummary {
    build_with(cx, all_floors, None)
}

/// Stores a floor's build: the new automatic members merged with the old ones
/// (what the options keep and the protected regions stay), the members made
/// from layout lines with new ids, and the layers they land on. Returns the
/// layers the new members are on.
fn apply_floor_build(
    project: &mut Project,
    fi: usize,
    mut fb: FloorBuild,
    opts: &BuildOptions,
    only: Option<&[Group]>,
) -> Vec<String> {
    for m in &mut fb.built {
        m.id = project.alloc_id();
        m.layer_name = layer_for(m.kind).to_string();
    }
    let layers: Vec<String> = fb.built.iter().map(|m| m.layer_name.clone()).collect();
    let old = load(&project.floors[fi]);
    let (kept, rest): (Vec<Member>, Vec<Member>) = old
        .into_iter()
        .partition(|m| is_protected(group_of(m), member_center(m), &fb.protected));
    let fresh: Vec<Member> = fb
        .members
        .into_iter()
        .filter(|m| !is_protected(group_of(m), member_center(m), &fb.protected))
        .collect();
    let mut members = match only {
        Some(due) => merge_groups(rest, fresh, opts, due),
        None => merge_rebuild(rest, fresh, opts),
    };
    members.extend(kept);
    let built: Vec<FramingMember> = fb
        .built
        .into_iter()
        .filter(|m| {
            let at = Point::lerp(m.start, m.end, 0.5);
            !is_protected(group_of_manual(m.kind), at, &fb.protected)
        })
        .collect();
    store_auto(
        &mut project.floors[fi],
        &members,
        built,
        opts,
        only,
        &fb.protected,
    );
    restamp(project, fi);
    layers
}

/// New members take the shape of their Framing Type (a plan that never saved
/// Framing Types keeps plain boxes).
pub(crate) fn restamp(project: &mut Project, fi: usize) {
    if plan_framing::catalog::project_has_catalog(project) {
        let catalog = plan_framing::catalog::of_project(project);
        plan_framing::catalog::stamp_floor(&mut project.floors[fi], &catalog);
    }
}

/// [`build`] with new settings (the Build Framing dialog's OK) stored in the
/// same undo step.
fn build_with(
    cx: &mut EditorContext,
    all_floors: bool,
    new: Option<FramingSettings>,
) -> BuildSummary {
    cx.begin_change(if all_floors {
        "Build All Framing"
    } else {
        "Build Framing"
    });
    if let Some(n) = &new {
        store_settings(&mut cx.project, n);
    }
    let active = cx.floor;
    let mut st = settings(&cx.project);
    let stored = st.clone();
    let floors: Vec<usize> = (0..cx.project.floors.len())
        .filter(|fi| !cx.project.floors[*fi].is_cad_detail())
        .filter(|fi| st.build.once_for(*fi, active, all_floors).build.any())
        .collect();
    let mut total = BuildSummary::default();
    let mut warnings: Vec<String> = Vec::new();
    let mut built_layers: Vec<String> = Vec::new();
    for fi in floors {
        let opts = st.build.once_for(fi, active, all_floors);
        let fb = frame_floor_all_with(&cx.project, fi, true, &st, &opts);
        total.add(fb.summary);
        warnings.extend(fb.warnings.iter().cloned());
        built_layers.extend(apply_floor_build(&mut cx.project, fi, fb, &opts, None));
        // The fingerprints only matter to Auto Rebuild; a plan that does not use
        // it keeps no settings object in its framing slot.
        if st.build.auto_rebuild.any() {
            st.build.record_built(fi, group_prints(&cx.project, fi));
        }
    }
    if st != stored {
        store_settings(&mut cx.project, &st);
    }
    ensure_layer(&mut cx.project, st.build.show_layers);
    ensure_manual_layers(&mut cx.project);
    if st.build.show_layers {
        for layer in built_layers {
            show_layer(&mut cx.project, &layer);
        }
    }
    refresh_details(cx);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "Built framing: {} wall, {} floor, {} ceiling and {} roof members",
        total.walls, total.floor, total.ceiling, total.roof
    );
    if let Some(w) = warnings.first() {
        cx.status.push_str(&format!(
            ". Check spans: {w}{}",
            if warnings.len() > 1 { " and more" } else { "" }
        ));
    }
    total
}

// ----- auto rebuild and retain -----

/// A fingerprint of the inputs of each group of floor `fi`, in
/// [`Group::ALL`] order (floor, ceiling, wall, roof): what the group's
/// framing is built from. Floor and ceiling framing follow the walls, the
/// room names and the layout lines (and the floor's holes); wall framing the
/// walls and their openings and the reference markers; roof framing the roof
/// planes and the truss layout lines.
pub fn group_prints(project: &Project, fi: usize) -> [u64; 4] {
    let floor = &project.floors[fi];
    let records = load_records(floor);
    let lines = |keep: fn(&Record) -> bool| -> String {
        records
            .iter()
            .filter(|r| keep(r))
            .map(|r| format!("{r:?}"))
            .collect()
    };
    let walls = format!("{:?}", floor.walls);
    let openings = format!("{:?}", floor.openings);
    let above = project
        .floors
        .get(fi + 1)
        .map(|f| format!("{:?}{:?}", f.walls, f.room_names))
        .unwrap_or_default();
    let floor_lines = lines(|r| {
        matches!(
            r,
            Record::JoistDirection { .. } | Record::BearingLine { .. } | Record::Marker { .. }
        )
    });
    let truss_lines = lines(|r| {
        matches!(
            r,
            Record::TrussDirection { .. } | Record::TrussBase { .. } | Record::Marker { .. }
        )
    });
    let rooms = format!("{:?}{:?}", floor.room_names, floor_holes(floor));
    let roof = format!("{:?}", roof_view::load(floor).planes);
    let print = |parts: &[&str]| fingerprint(&parts.join("|"));
    let mut out = [0; 4];
    for g in Group::ALL {
        out[group_index(g)] = match g {
            Group::Floor => print(&[&walls, &rooms, &floor_lines]),
            Group::Ceiling => print(&[&walls, &rooms, &floor_lines, &above]),
            Group::Wall => print(&[&walls, &openings, &floor_lines]),
            Group::Roof => print(&[&roof, &truss_lines]),
        };
    }
    out
}

fn group_index(g: Group) -> usize {
    Group::ALL.iter().position(|x| *x == g).unwrap_or(0)
}

thread_local! {
    /// The editor revision auto rebuild last looked at.
    static AUTO_CHECKED: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}

/// Auto Rebuild Framing: rebuilds the groups whose Build Framing option
/// "Auto rebuild" is on and whose inputs changed since the last build or
/// rebuild (see [`group_prints`]). Only floors that were built are touched.
/// Retained groups and retained walls keep their members. Returns whether
/// anything was rebuilt. The shell may call it once a frame; like
/// `roof_view::auto_rebuild` it makes no undo step of its own, so the
/// rebuild is undone together with the edit that caused it.
pub fn auto_rebuild(cx: &mut EditorContext) -> bool {
    let mut st = settings(&cx.project);
    if !st.build.auto_rebuild.any() {
        return false;
    }
    let key = cx.cache_key();
    if AUTO_CHECKED.with(|c| c.replace(key)) == key {
        return false;
    }
    let mut changed = false;
    for fi in 0..cx.project.floors.len() {
        let now = group_prints(&cx.project, fi);
        let due = st.build.due(fi, now);
        if due.is_empty() {
            continue;
        }
        let fb = frame_floor_all_with(&cx.project, fi, true, &st, &st.build);
        apply_floor_build(&mut cx.project, fi, fb, &st.build, Some(&due));
        for g in &due {
            st.build.record_group(fi, *g, now[group_index(*g)]);
        }
        changed = true;
    }
    if changed {
        store_settings(&mut cx.project, &st);
        ensure_manual_layers(&mut cx.project);
        cx.mark_dirty();
        cx.refresh();
        cx.status = "Framing rebuilt".into();
    }
    changed
}

/// Retain Wall Framing on the walls `ids` (or off): the next Build Framing
/// and every auto rebuild leave those walls' framing as it is, so edits made
/// to it survive. One undo step. Returns how many walls changed.
pub fn set_walls_retained(cx: &mut EditorContext, ids: &[Id], retained: bool) -> usize {
    let mut st = settings(&cx.project);
    let before = st.build.retain_walls.clone();
    for id in ids {
        st.build.set_wall_retained(*id, retained);
    }
    let n = ids
        .iter()
        .filter(|id| before.contains(id) != retained)
        .count();
    if n == 0 {
        return 0;
    }
    cx.begin_change("Retain Wall Framing");
    store_settings(&mut cx.project, &st);
    cx.mark_dirty();
    cx.status = if retained {
        format!("Retaining the framing of {n} wall(s)")
    } else {
        format!("{n} wall(s) rebuild with Build Framing again")
    };
    n
}

/// Retain Wall Framing on the walls `ids` with no undo step of its own: the
/// Wall Specification calls it inside its own step. Returns how many walls
/// changed.
pub fn retain_walls_in(project: &mut Project, ids: &[Id], retained: bool) -> usize {
    let mut st = settings(project);
    let n = ids
        .iter()
        .filter(|id| st.build.retain_walls.contains(id) != retained)
        .count();
    if n == 0 {
        return 0;
    }
    for id in ids {
        st.build.set_wall_retained(*id, retained);
    }
    store_settings(project, &st);
    n
}

/// Whether Build Framing leaves wall `id`'s framing alone.
pub fn wall_retained(project: &Project, id: Id) -> bool {
    settings(project).build.retain_walls.contains(&id)
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
    store_auto(cx.floor_mut(), &[], Vec::new(), &everything(), None, &[]);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Deleted {n} framing members");
    n
}

/// Options under which nothing is kept: every group is built and none is
/// retained (Delete Framing).
fn everything() -> BuildOptions {
    BuildOptions {
        build: plan_framing::GroupFlags {
            floor: true,
            ceiling: true,
            wall: true,
            roof: true,
        },
        ..BuildOptions::default()
    }
}

/// Makes sure the Framing layer exists; with `show` it is turned on, and a
/// layer made here starts hidden otherwise (framing layers are off until
/// something uses them, like Chief's).
fn ensure_layer(project: &mut Project, show: bool) {
    match project.layers.get_mut(LAYER) {
        Some(l) => {
            if show {
                l.display = true;
            }
        }
        None => {
            let mut l = Layer::new(LAYER, [180, 140, 60], 18);
            l.display = show;
            project.layers.add(l);
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

/// The framing schedule of a [`Takeoff`]: one row per member type, size and
/// cut length with its count, linear feet and board feet, then a total.
pub fn cut_table(t: &Takeoff) -> (Vec<String>, Vec<Vec<String>>) {
    let columns: Vec<String> = [
        "Member",
        "Size",
        "Cut Length",
        "Qty",
        "Linear ft",
        "Board ft",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();
    let mut rows: Vec<Vec<String>> = t
        .cuts
        .iter()
        .map(|c| {
            vec![
                c.member.clone(),
                c.size.clone(),
                format!("{}\"", plan_framing::format_inches(c.cut)),
                c.count.to_string(),
                format!("{:.1}", c.linear_feet()),
                format!("{:.1}", c.board_feet()),
            ]
        })
        .collect();
    let (qty, lf, bf) = t.cuts.iter().fold((0, 0.0, 0.0), |(q, l, b), c| {
        (q + c.count, l + c.linear_feet(), b + c.board_feet())
    });
    rows.push(vec![
        "Total".into(),
        String::new(),
        String::new(),
        qty.to_string(),
        format!("{lf:.1}"),
        format!("{bf:.1}"),
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
    style: PlanStyle,
}

/// How a built member is drawn in plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanStyle {
    /// Dash and gap lengths in screen points; `None` is a solid outline.
    pub dash: Option<(f32, f32)>,
    /// Alpha of the fill (0 draws the outline alone).
    pub fill_alpha: u8,
    /// Outline width in points.
    pub width: f32,
    /// Drawn as a cross box (vertical members in plan).
    pub cross: bool,
}

/// Chief's plan line styles for built framing, by member kind: the walls'
/// plates, studs and headers are solid and filled; floor joists, rims and
/// blocking solid with a light fill; ceiling joists dashed; rafters, ridge,
/// hips, valleys and fascia long-dashed (they lie above the plan); truss
/// chords long-dashed and webs dotted. Wall blocking is drawn lighter. (Chief's
/// exact dash pattern is not documented; DECISIONS 114, verify in Chief.)
pub fn plan_style(m: &Member) -> PlanStyle {
    use MemberKind as K;
    let solid = |fill_alpha, width| PlanStyle {
        dash: None,
        fill_alpha,
        width,
        cross: false,
    };
    let vertical = m.transform.axis_x[1].abs() > 0.99;
    match m.kind {
        K::Stud | K::KingStud | K::TrimmerStud | K::CrippleStud | K::CornerStud | K::TeeStud => {
            PlanStyle {
                cross: vertical,
                ..solid(70, 0.75)
            }
        }
        K::TopPlate | K::BottomPlate | K::Header | K::Sill => solid(70, 0.75),
        K::Blocking if m.wall_id.is_some() => solid(35, 0.5),
        K::Blocking | K::Joist | K::RimJoist | K::TrimmerJoist | K::HeaderJoist | K::Ledger => {
            solid(40, 0.75)
        }
        K::CeilingJoist => PlanStyle {
            dash: Some((6.0, 4.0)),
            fill_alpha: 0,
            width: 0.75,
            cross: false,
        },
        K::Rafter
        | K::Ridge
        | K::Hip
        | K::Valley
        | K::Fascia
        | K::CollarTie
        | K::TrussTopChord
        | K::TrussBottomChord => PlanStyle {
            dash: Some((10.0, 5.0)),
            fill_alpha: 0,
            width: 0.75,
            cross: false,
        },
        K::TrussWeb => PlanStyle {
            dash: Some((2.0, 3.0)),
            fill_alpha: 0,
            width: 0.5,
            cross: false,
        },
    }
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
    /// The TR-X / FTR-X labels of the plan's trusses.
    truss_labels: Option<Rc<Vec<(Id, String)>>>,
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
                truss_labels: None,
            });
        }
        f(c.as_mut().expect("just filled"))
    })
}

fn built_hulls(cx: &EditorContext) -> Rc<Vec<Hull>> {
    if let Some(h) = with_draw_cache(cx, |c| c.hulls.clone()) {
        return h;
    }
    let show_cross = settings(&cx.project).build.detail.show_cross;
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
                let mut style = plan_style(m);
                style.cross &= show_cross;
                Hull { pts, lo, hi, style }
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

/// The automatic label of every truss in the plan, kept until the next edit.
fn truss_label_cache(cx: &EditorContext) -> Rc<Vec<(Id, String)>> {
    if let Some(l) = with_draw_cache(cx, |c| c.truss_labels.clone()) {
        return l;
    }
    let labels = Rc::new(trusses::truss_labels(&cx.project));
    with_draw_cache(cx, |c| c.truss_labels = Some(labels.clone()));
    labels
}

fn draw_built(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if cx.framing.is_empty() || !cx.layers().is_visible(LAYER) {
        return;
    }
    let [r, g, b] = cx.layers().get(LAYER).map_or([180, 140, 60], |l| l.color);
    let view = cam.rect.expand(2.0);
    for h in built_hulls(cx).iter() {
        // Members outside the canvas draw nothing visible.
        let (a, c) = (cam.world_to_screen(h.lo), cam.world_to_screen(h.hi));
        if !view.intersects(egui::Rect::from_two_pos(a, c)) {
            continue;
        }
        let pts: Vec<Pos2> = h.pts.iter().map(|p| cam.world_to_screen(*p)).collect();
        let edge = Stroke::new(h.style.width, Color32::from_rgb(r, g, b));
        let fill = Color32::from_rgba_unmultiplied(r, g, b, h.style.fill_alpha);
        match pts.len() {
            0 | 1 => {}
            2 => {
                painter.line_segment([pts[0], pts[1]], edge);
            }
            _ => match h.style.dash {
                None => {
                    painter.add(Shape::convex_polygon(pts.clone(), fill, edge));
                    // Studs and other vertical members are cross boxes.
                    if h.style.cross && pts.len() >= 3 {
                        painter.line_segment([pts[0], pts[pts.len() / 2]], edge);
                        if pts.len() >= 4 {
                            painter.line_segment(
                                [pts[1], pts[(pts.len() * 3 / 4).min(pts.len() - 1)]],
                                edge,
                            );
                        }
                    }
                }
                Some((dash, gap)) => {
                    let mut ring = pts;
                    ring.push(ring[0]);
                    painter.extend(Shape::dashed_line(&ring, edge, dash, gap));
                }
            },
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

/// Layer of the labels of framing members.
pub const LAYER_LABELS: &str = "Framing, Labels";

const MANUAL_LAYERS: [(&str, [u8; 3]); 8] = [
    (LAYER_LABELS, [90, 60, 30]),
    (LAYER_JOISTS, [200, 150, 70]),
    (LAYER_RAFTERS, [170, 120, 60]),
    (LAYER_POSTS, [120, 90, 50]),
    (LAYER_BEAMS, [150, 100, 40]),
    (LAYER_TRUSSES, [190, 110, 80]),
    (LAYER_ROOF_TRUSS_LABELS, [120, 60, 40]),
    (LAYER_FLOOR_TRUSS_LABELS, [120, 60, 40]),
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
fn store_auto(
    floor: &mut Floor,
    members: &[Member],
    built: Vec<FramingMember>,
    opts: &BuildOptions,
    only: Option<&[Group]>,
    protected: &[Protected],
) {
    let kept = |m: &FramingMember| {
        let g = group_of_manual(m.kind);
        // The groups outside an auto rebuild, and frozen groups, keep theirs;
        // so do the members in a region Retain Framing protects.
        opts.group_frozen(g)
            || only.is_some_and(|o| !o.contains(&g))
            || is_protected(g, Point::lerp(m.start, m.end, 0.5), protected)
    };
    let mut records: Vec<Record> = load_records(floor)
        .into_iter()
        .filter(|r| match r {
            Record::Built(m) => kept(m),
            _ => true,
        })
        .collect();
    records.extend(built.into_iter().filter(|m| !kept(m)).map(Record::Built));
    let mut values: Vec<Value> = members
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();
    values.extend(records.iter().filter_map(|r| serde_json::to_value(r).ok()));
    // The stored framing defaults are neither members nor records.
    values.extend(floor.framing.iter().filter(|v| is_settings(v)).cloned());
    // Nor are the Framing Types, Default Framing Members and reporting
    // defaults (plan_framing::catalog).
    values.extend(
        floor
            .framing
            .iter()
            .filter(|v| plan_framing::catalog::is_catalog(v))
            .cloned(),
    );
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

/// Every placed or built member on every building floor (not the details), in
/// floor order: the order the trusses among them were made.
pub fn all_manual_in_plan(project: &Project) -> Vec<FramingMember> {
    project
        .floors
        .iter()
        .filter(|f| !f.is_cad_detail())
        .flat_map(manual_members)
        .collect()
}

/// The manual record `id` on `floor`.
pub fn find(floor: &Floor, id: Id) -> Option<Record> {
    load_records(floor).into_iter().find(|r| r.id() == id)
}

/// Makes sure the five manual framing layers exist. A layer made here starts
/// hidden, like Chief's framing layers; placing or building a member turns on
/// the layer it lands on ([`show_layer`]). A hidden one stays hidden.
pub fn ensure_manual_layers(project: &mut Project) {
    for (name, color) in MANUAL_LAYERS {
        let mut l = Layer::new(name, color, 18);
        l.display = false;
        project.layers.add(l);
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

/// [`new_member`] with the Posts tab of Build Framing applied: a new post takes
/// the size and material set there, and becomes a post with footing when the
/// tab asks for footings.
pub fn new_member_in(
    cx: &EditorContext,
    kind: ManualMemberKind,
    start: Point,
    end: Point,
) -> FramingMember {
    use ManualMemberKind as K;
    let is_post = matches!(kind, K::Post | K::PostWithFooting);
    let kind = if kind == K::Post && settings(&cx.project).build.posts.footing {
        K::PostWithFooting
    } else {
        kind
    };
    let mut m = new_member(cx.floor(), kind, start, end);
    if is_post {
        let p = settings(&cx.project).build.posts;
        m.lumber = p.size;
        m.depth = p.size.depth();
        m.width = p.size.width();
        m.material = p.material;
    }
    // The Manual Framing Defaults (Default Settings > Framing > Manual
    // Framing) set the section, plies and construction of a new member.
    if plan_framing::catalog::project_has_catalog(&cx.project) {
        let catalog = plan_framing::catalog::of_project(&cx.project);
        catalog.manual.apply_new(&mut m);
        if let Some(def) = catalog
            .manual
            .section_for(m.kind)
            .map(|s| s.construction.clone())
        {
            catalog.apply_def_to_manual(&def, &mut m);
        }
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
    let truss = records
        .last()
        .and_then(|r| r.member().map(|m| (m.kind.is_truss(), m.kind)));
    store_records(cx.floor_mut(), &records);
    ensure_manual_layers(&mut cx.project);
    if let Some(layer) = layer {
        show_layer(&mut cx.project, &layer);
    }
    if let Some((true, kind)) = truss {
        // A new truss takes the shape of the roof it stands under, is
        // labelled, and joins the Truss Detail.
        let fl = cx.floor;
        trusses::conform_moved(&mut cx.project, fl, &[id]);
        show_layer(&mut cx.project, trusses::label_layer(kind));
        details::refresh_truss_detail_in(&mut cx.project);
    }
    cx.mark_dirty();
    cx.refresh();
    id
}

/// Deletes the records `ids` (one undo step). Returns how many went.
pub fn delete_records(cx: &mut EditorContext, ids: &[Id]) -> usize {
    let mut records = load_records(cx.floor());
    let before = records.len();
    let truss = records
        .iter()
        .any(|r| ids.contains(&r.id()) && r.member().is_some_and(|m| m.kind.is_truss()));
    records.retain(|r| !ids.contains(&r.id()));
    let n = before - records.len();
    if n == 0 {
        return 0;
    }
    cx.begin_change("Delete Framing");
    store_records(cx.floor_mut(), &records);
    if truss {
        details::refresh_truss_detail_in(&mut cx.project);
    }
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
    let truss = records
        .iter()
        .any(|r| ids.contains(&r.id()) && r.member().is_some_and(|m| m.kind.is_truss()));
    cx.begin_change_merged("Move Framing");
    store_records(cx.floor_mut(), &records);
    if truss {
        // A roof truss that moves takes the roof where it lands, unless locked.
        let fl = cx.floor;
        trusses::conform_moved(&mut cx.project, fl, ids);
        details::refresh_truss_detail_in(&mut cx.project);
    }
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
    apply_edit_keep(cx, draft, false)
}

/// [`apply_edit`] where `keep_built` leaves a member Build Framing laid out
/// as one (the truss's Automatically Generated Truss box stayed checked): the
/// next build replaces it.
pub fn apply_edit_keep(cx: &mut EditorContext, draft: FramingMember, keep_built: bool) -> bool {
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
    let mut draft = draft;
    // Force Truss Rebuild in the specification: made again for where it stands.
    if draft.truss.as_ref().is_some_and(|t| t.force_rebuild) {
        let roof = roof_of(cx.floor());
        if let Some(t) = draft.truss.as_mut() {
            t.force_rebuild = false;
        }
        trusses::conform(&mut draft, roof.as_ref());
    }
    let truss = draft.kind.is_truss();
    *slot = if keep_built && matches!(slot, Record::Built(_)) {
        Record::Built(draft)
    } else {
        Record::Manual(draft)
    };
    store_records(cx.floor_mut(), &records);
    ensure_manual_layers(&mut cx.project);
    show_layer(&mut cx.project, &layer);
    if truss {
        details::refresh_truss_detail_in(&mut cx.project);
    }
    cx.mark_dirty();
    cx.refresh();
    true
}

/// Replaces the layout record with the id of `rec` (a Joist Direction Line,
/// a Roof Truss Direction Line ...) by `rec`, one undo step named `label`.
/// Returns whether it changed.
pub fn apply_record_edit(cx: &mut EditorContext, label: &str, rec: Record) -> bool {
    let mut records = load_records(cx.floor());
    let Some(slot) = records.iter_mut().find(|r| r.id() == rec.id()) else {
        return false;
    };
    if *slot == rec {
        return false;
    }
    cx.begin_change(label);
    *slot = rec;
    store_records(cx.floor_mut(), &records);
    cx.mark_dirty();
    cx.refresh();
    true
}

/// The macros the Label panel of a framing member offers (`%name%`).
pub const LABEL_MACROS: &[(&str, &str)] = &[
    ("nominal_size", "The nominal size, such as 2x10"),
    ("size", "The actual section, such as 1 1/2 x 9 1/4"),
    ("width", "The width of the section"),
    ("depth", "The depth of the section"),
    ("length", "The cut length"),
    ("type", "The member type"),
    ("automatic_label", "The cut-list label"),
    ("plies", "How many boards make the member"),
];

/// A framing member's label with its macros expanded: the label the user
/// specified, empty when there is none.
pub fn label_text(m: &FramingMember) -> String {
    if m.custom_label.is_empty() {
        return String::new();
    }
    let fmt = |v: f64| plan_framing::format_inches(v);
    let size = format!(
        "{} x {}",
        fmt(m.width / f64::from(m.plies.max(1))),
        fmt(m.depth)
    );
    let mut out = m.custom_label.clone();
    for (name, value) in [
        ("nominal_size", m.lumber.name()),
        ("size", size),
        ("width", format!("{}\"", fmt(m.width))),
        ("depth", format!("{}\"", fmt(m.depth))),
        ("length", format!("{}\"", fmt(m.length()))),
        ("type", m.kind.name().to_string()),
        ("automatic_label", m.label.clone()),
        ("plies", m.plies.max(1).to_string()),
    ] {
        out = out.replace(&format!("%{name}%"), &value);
    }
    out
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
    /// The framing schedule: pieces by member type, size and cut length
    /// (columns Member, Size, Cut Length, Qty, Linear ft, Board ft).
    pub cut_columns: Vec<String>,
    pub cut_rows: Vec<Vec<String>>,
    /// The schedule as CSV.
    pub cut_csv: String,
}

fn merge_takeoff(mut a: Takeoff, b: Takeoff) -> Takeoff {
    for (name, n) in b.lines {
        match a.lines.iter_mut().find(|(l, _)| *l == name) {
            Some((_, c)) => *c += n,
            None => a.lines.push((name, n)),
        }
    }
    a.board_feet += b.board_feet;
    for c in b.cuts {
        let key = |x: f64| (x * 16.0).round() as i64;
        match a
            .cuts
            .iter_mut()
            .find(|l| l.member == c.member && l.size == c.size && key(l.cut) == key(c.cut))
        {
            Some(l) => l.count += c.count,
            None => a.cuts.push(c),
        }
    }
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
    // A plan that saved Framing Types names them in the cut schedule.
    let auto_takeoff = if plan_framing::catalog::project_has_catalog(project) {
        plan_framing::typed_takeoff(&auto, &plan_framing::catalog::of_project(project))
    } else {
        takeoff_of(&auto)
    };
    let t = merge_takeoff(auto_takeoff, manual_takeoff(&manual));
    let (columns, rows) = table_of(&t);
    let (cut_columns, cut_rows) = cut_table(&t);
    TakeoffData {
        members: auto.len() + manual.len(),
        csv: csv_of((columns.clone(), rows.clone())),
        columns,
        rows,
        material_csv: MaterialList::from_members(&auto, &manual).to_csv(),
        cut_csv: csv_of((cut_columns.clone(), cut_rows.clone())),
        cut_columns,
        cut_rows,
    }
}

// ----- 3D and DXF -----

/// Meshes of every framing member on every floor, for the 3D view: the
/// automatic wall, floor and roof members (one box per piece of lumber; a
/// rafter with its tail cut and birdsmouth), then the manual and built members
/// (boxes per lumber member, chords and webs per truss, footings under posts).
/// Members on a hidden layer are left out.
pub fn manual_framing_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let mut meshes = Vec::new();
    if project.layers.is_visible(LAYER) {
        for floor in &project.floors {
            meshes.extend(load(floor).iter().map(Member::mesh));
        }
    }
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

/// Wall surface materials: a wall mesh of one of these hides the studs in an
/// elevation.
const WALL_SURFACES: [plan_3d::Material; 7] = [
    plan_3d::Material::WallExterior,
    plan_3d::Material::WallInterior,
    plan_3d::Material::Stucco,
    plan_3d::Material::Siding,
    plan_3d::Material::Brick,
    plan_3d::Material::Stone,
    plan_3d::Material::Concrete,
];

/// The scene an elevation or section is drawn from. Outside the Framing
/// Overview it is `plan_3d::build_scene`. In the overview the wall surfaces
/// are left out and every wall's framing (plates, studs, kings, trimmers,
/// headers, sills, cripples, blocking) is added, so the elevation shows the
/// framing of the wall instead of its skin.
pub fn elevation_scene(project: &Project) -> plan_3d::Scene {
    let mut scene = plan_3d::build_scene(project);
    if !in_overview(project) {
        return scene;
    }
    let walls: std::collections::HashSet<Id> = project
        .floors
        .iter()
        .flat_map(|f| f.walls.iter().map(|w| w.id))
        .collect();
    scene.meshes.retain(|m| {
        !(m.object_id.is_some_and(|id| walls.contains(&id)) && WALL_SURFACES.contains(&m.material))
    });
    scene.meshes.extend(wall_framing_meshes(project));
    scene
}

/// The meshes of the automatic wall members of every floor (the members that
/// belong to a wall), when the view shows the Framing layer.
pub fn wall_framing_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    if !project.view_layers().is_visible(LAYER) {
        return Vec::new();
    }
    project
        .floors
        .iter()
        .flat_map(load)
        .filter(|m| m.wall_id.is_some())
        .map(|m| m.mesh())
        .collect()
}

/// The scene of the Framing Overview camera (a Perspective Overview of the
/// framing): every automatic and manual framing member of every floor, and
/// nothing else, so the walls, floors and roof surfaces do not hide them. It
/// ignores the framing layers' display (they are off in a new plan): the
/// camera exists to show them.
pub fn overview_scene(project: &Project) -> plan_3d::Scene {
    let mut scene = plan_3d::Scene::default();
    for floor in &project.floors {
        scene.meshes.extend(load(floor).iter().map(Member::mesh));
        for m in manual_members(floor) {
            scene
                .meshes
                .extend(m.to_boxes().iter().map(|b| b.mesh(Some(m.id))));
        }
    }
    scene
}

// ----- Wall Detail -----

/// The framing elevation of one wall with its dimensions (Chief's Wall
/// Detail): the strokes and dimensions of `plan_framing::{wall_detail,
/// wall_detail_dims}` in the wall's elevation frame, X along the wall from its
/// start and Y up from the bottom plate.
#[derive(Clone, Debug, PartialEq)]
pub struct WallDetail {
    pub wall: Id,
    pub length: f64,
    pub height: f64,
    pub strokes: Vec<plan_framing::Stroke>,
    pub dims: Vec<plan_framing::DetailDim>,
}

/// The wall detail of wall `wall` on floor `fi`, from its stored framing;
/// `None` when the wall has no framing yet (Build Framing first).
pub fn wall_detail_of(project: &Project, fi: usize, wall: Id) -> Option<WallDetail> {
    let floor = project.floors.get(fi)?;
    let w = floor.walls.iter().find(|w| w.id == wall)?;
    let members = load(floor);
    if !members.iter().any(|m| m.wall_id == Some(wall)) {
        return None;
    }
    Some(WallDetail {
        wall,
        length: w.length(),
        height: w.height,
        strokes: plan_framing::wall_detail(w, &members),
        dims: plan_framing::wall_detail_dims(w, &members),
    })
}

/// Draws a wall detail in `rect`, scaled to fit with room for the
/// dimensions: members as outlines, labels, and each dimension as a line with
/// end ticks and its text.
pub fn paint_wall_detail(painter: &egui::Painter, rect: egui::Rect, d: &WallDetail, ink: Color32) {
    use plan_framing::Stroke as S;
    const MARGIN: f64 = 36.0;
    let (w, h) = (d.length.max(1.0), d.height.max(1.0));
    let scale = ((f64::from(rect.width()) - 2.0 * MARGIN) / w)
        .min((f64::from(rect.height()) - 2.0 * MARGIN) / h)
        .max(0.01);
    let at = |p: Point| {
        Pos2::new(
            rect.min.x + MARGIN as f32 + (p.x * scale) as f32,
            rect.max.y - MARGIN as f32 - (p.y * scale) as f32,
        )
    };
    let line = Stroke::new(0.75_f32, ink);
    for s in &d.strokes {
        match s {
            S::Line(a, b) => {
                painter.line_segment([at(*a), at(*b)], line);
            }
            S::Rect { min, max } => {
                painter.rect_stroke(
                    egui::Rect::from_two_pos(at(*min), at(*max)),
                    0.0,
                    line,
                    egui::StrokeKind::Middle,
                );
            }
            S::Text { pos, text, height } => {
                painter.text(
                    at(*pos),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(((height * scale) as f32).clamp(6.0, 11.0)),
                    ink,
                );
            }
        }
    }
    let dim = Stroke::new(1.0_f32, Color32::from_rgb(180, 0, 0));
    for dm in &d.dims {
        let horizontal = (dm.b.y - dm.a.y).abs() < (dm.b.x - dm.a.x).abs();
        let off = if horizontal {
            Point::new(0.0, dm.offset)
        } else {
            Point::new(dm.offset, 0.0)
        };
        let (a, b) = (at(dm.a + off), at(dm.b + off));
        painter.line_segment([a, b], dim);
        for p in [a, b] {
            let tick = if horizontal {
                egui::vec2(0.0, 3.0)
            } else {
                egui::vec2(3.0, 0.0)
            };
            painter.line_segment([p - tick, p + tick], dim);
        }
        painter.text(
            a + (b - a) * 0.5,
            if horizontal {
                Align2::CENTER_BOTTOM
            } else {
                Align2::RIGHT_CENTER
            },
            &dm.text,
            FontId::proportional(9.0),
            Color32::from_rgb(180, 0, 0),
        );
    }
}

// ----- Framing Overview -----

/// Name of the saved plan view, and of its layer set, that shows the framing
/// alone.
pub const OVERVIEW: &str = "Framing Overview";

/// Is `name` one of the framing layers (`"Framing"`, `"Framing, Floor
/// Joists"`, ...)?
pub fn is_framing_layer(name: &str) -> bool {
    name == LAYER || name.starts_with("Framing, ")
}

/// The Framing Overview layer set: the framing layers shown, every other layer
/// hidden.
pub fn overview_layer_set(layers: &plan_core::LayerSet) -> plan_core::layer_sets::LayerSetDef {
    let mut set = plan_core::layer_sets::LayerSetDef::new(OVERVIEW);
    for l in &layers.layers {
        set.states.push(plan_core::layer_sets::LayerState::new(
            l.name.clone(),
            is_framing_layer(&l.name),
            false,
        ));
    }
    set
}

/// Adds the Framing Overview layer set and saved plan view when the plan has
/// none, and makes sure the framing layers exist. Returns whether anything
/// was added. The set is rebuilt when layers were added since, so a layer
/// made later never leaks into the overview.
pub fn install_overview(project: &mut Project) -> bool {
    ensure_layer(project, true);
    ensure_manual_layers(project);
    let set = overview_layer_set(&project.layers);
    let mut added = false;
    match project.layer_sets.get_mut(OVERVIEW) {
        Some(existing) => *existing = set,
        None => {
            project.layer_sets.add_set(set);
            added = true;
        }
    }
    if project.plan_view(OVERVIEW).is_none() {
        project
            .plan_views
            .push(plan_core::SavedPlanView::new(OVERVIEW, OVERVIEW));
        added = true;
    }
    added
}

/// Is the Framing Overview the active plan view?
pub fn in_overview(project: &Project) -> bool {
    project.active_plan_view == OVERVIEW
}

/// Switches the plan to the Framing Overview of the active floor: the plan
/// shows the framing of that floor and nothing else. Builds the framing first
/// when the floor has none. One undo step.
pub fn activate_overview(cx: &mut EditorContext) {
    cx.begin_change("Framing Overview");
    if install_overview(&mut cx.project) {
        cx.status = "Framing Overview added to the plan views".into();
    }
    if load(cx.floor()).is_empty() && manual_members(cx.floor()).is_empty() {
        let fi = cx.floor;
        let fb = frame_floor_all(&cx.project, fi, true);
        let opts = settings(&cx.project).build;
        apply_floor_build(&mut cx.project, fi, fb, &opts, None);
    }
    cx.project.activate_plan_view(OVERVIEW);
    cx.mark_dirty();
    cx.refresh();
}

/// Back from the Framing Overview to the ordinary floor plan view.
pub fn leave_overview(cx: &mut EditorContext) {
    if !in_overview(&cx.project) {
        return;
    }
    cx.begin_change("Floor Plan View");
    cx.project
        .activate_plan_view(plan_core::layer_sets::DEFAULT_PLAN_VIEW_NAME);
    cx.mark_dirty();
    cx.refresh();
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
    floor_dxf_with(project, floor, rooms, plan_core::DxfAnnotation::default())
}

/// [`floor_dxf`] with the annotation sizing of the dimension set and the
/// sheet scale (text heights from the styles, printed sizes at the scale).
pub fn floor_dxf_with(
    project: &Project,
    floor: usize,
    rooms: &[Room],
    annotation: plan_core::DxfAnnotation,
) -> String {
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
        .with_annotation(annotation)
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
        if let Some(m) = r.member() {
            draw_member_texts(cx, painter, cam, m, picked.contains(&m.id), color);
        }
    }
}

/// Whether a selected member shows the S and E marks at its ends (the Start
/// and End Indicators of Preferences > Edit).
static START_END_INDICATORS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Turns the Start and End Indicators on or off (Preferences > Edit).
pub fn set_start_end_indicators(on: bool) {
    START_END_INDICATORS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The text a member draws on the plan: its S and E marks when selected, the
/// label the user specified, and a truss's TR-X label.
fn draw_member_texts(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    m: &FramingMember,
    selected: bool,
    color: Color32,
) {
    let mid = Point::lerp(m.start, m.end, 0.5);
    if selected
        && m.kind.is_physical()
        && !m.kind.is_vertical()
        && START_END_INDICATORS.load(std::sync::atomic::Ordering::Relaxed)
    {
        let size = 6.0;
        for (stroke, at) in plan_framing::start_end_marks(m.start, m.end, size)
            .into_iter()
            .map(|s| match s {
                plan_framing::Stroke::Text { pos, text, .. } => (text, pos),
                _ => (String::new(), Point::ZERO),
            })
        {
            painter.text(
                cam.world_to_screen(at),
                Align2::CENTER_CENTER,
                stroke,
                FontId::proportional(11.0),
                color,
            );
        }
    }
    if m.kind.is_truss() {
        let layer = trusses::label_layer(m.kind);
        if cx.layers().is_visible(layer) {
            let text = if m.custom_label.trim().is_empty() {
                truss_label_cache(cx)
                    .iter()
                    .find(|(id, _)| *id == m.id)
                    .map(|(_, l)| l.clone())
            } else {
                Some(label_text(m))
            };
            if let Some(t) = text {
                painter.text(
                    cam.world_to_screen(mid),
                    Align2::CENTER_CENTER,
                    t,
                    FontId::proportional(12.0),
                    color,
                );
            }
        }
    } else if !m.custom_label.is_empty() && cx.layers().is_visible(LAYER_LABELS) {
        painter.text(
            cam.world_to_screen(mid),
            Align2::CENTER_CENTER,
            label_text(m),
            FontId::proportional(11.0),
            color,
        );
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
            line: BearingLine::new((Point::new(120.0, 5.0), Point::new(120.0, 185.0))),
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
            line: BearingLine::new((Point::new(20.0, 0.0), Point::new(20.0, 90.0))),
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
            line: BearingLine::new((Point::ZERO, Point::new(50.0, 0.0))),
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

    // ----- Framing Defaults, wall details, stairwells, eaves, takeoff, overview -----

    #[test]
    fn build_backs_the_corners_and_blocks_the_walls_by_default() {
        let mut cx = house();
        build(&mut cx, false);
        // Each of the four walls ends at a corner on both ends.
        assert_eq!(count_kind(&cx.framing, MemberKind::CornerStud), 8);
        assert!(count_kind(&cx.framing, MemberKind::Blocking) > 20);
        // No partitions in this plan, so no tee backing.
        assert_eq!(count_kind(&cx.framing, MemberKind::TeeStud), 0);
        // A partition butting into the south wall backs it on both sides.
        let mut cx = house();
        cx.project.add_wall(
            0,
            Point::new(160.0, 0.0),
            Point::new(160.0, 192.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        cx.refresh();
        build(&mut cx, false);
        // The partition's two ends meet the south and north walls.
        assert_eq!(count_kind(&cx.framing, MemberKind::TeeStud), 4);
    }

    #[test]
    fn framing_defaults_are_stored_undone_saved_and_used() {
        let mut cx = house();
        assert_eq!(settings(&cx.project), FramingSettings::default());
        assert_eq!(settings(&cx.project).walls, FramingDefaults::house());
        let mut s = settings(&cx.project);
        s.walls.stud_spacing = 24.0;
        s.walls.corner_studs = 0;
        s.walls.wall_blocking = false;
        s.walls.header_table[2].lumber = plan_framing::TWO_BY_TWELVE;
        set_settings(&mut cx, s.clone());
        assert_eq!(cx.undo_label(), Some("Framing Defaults"));
        assert_eq!(settings(&cx.project), s);
        // Saved with the plan.
        let back = Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(settings(&back), s);
        // Setting the same value again is not an undo step.
        let before = cx.can_undo();
        set_settings(&mut cx, s.clone());
        assert_eq!(cx.can_undo(), before);

        let plain = {
            let mut c2 = house();
            build(&mut c2, false);
            c2.framing.len()
        };
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::CornerStud), 0);
        assert_eq!(count_kind(&cx.framing, MemberKind::Blocking), 0);
        assert!(
            cx.framing.len() < plain,
            "24\" studs and no blocking: fewer pieces"
        );
        // Delete Framing keeps the defaults, and so does a rebuild.
        clear(&mut cx);
        assert_eq!(settings(&cx.project), s);
        build(&mut cx, false);
        assert_eq!(settings(&cx.project), s);
        // The stored object is not a member or a record.
        assert_eq!(load(cx.floor()).len(), cx.framing.len());
        assert!(load_records(cx.floor()).is_empty());
        // Undo to the first step takes the defaults back.
        while cx.can_undo() {
            cx.undo();
        }
        assert_eq!(settings(&cx.project), FramingSettings::default());
    }

    #[test]
    fn a_stairwell_in_the_floor_platform_is_framed_with_headers_and_trimmers() {
        let mut cx = house();
        let mut layer = FoundationLayer::default();
        layer
            .platform_holes
            .push(plan_core::foundation::PlatformHole::new(
                5,
                vec![
                    Point::new(100.0, 60.0),
                    Point::new(136.0, 60.0),
                    Point::new(136.0, 120.0),
                    Point::new(100.0, 120.0),
                ],
                PlatformKind::Floor,
            ));
        cx.project.floors[0].set_foundation(&layer).unwrap();
        assert_eq!(floor_holes(cx.floor()).len(), 1);
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::TrimmerJoist), 4);
        assert_eq!(count_kind(&cx.framing, MemberKind::HeaderJoist), 4);
        // Without the hole there are none.
        let mut cx = house();
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::TrimmerJoist), 0);
    }

    #[test]
    fn rim_joists_are_built_with_the_floor() {
        let mut cx = house();
        build(&mut cx, false);
        assert!(count_kind(&cx.framing, MemberKind::RimJoist) >= 2);
        let mut s = settings(&cx.project);
        s.walls.rim_joist = false;
        set_settings(&mut cx, s);
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::RimJoist), 0);
    }

    #[test]
    fn roof_rafters_take_the_planes_overhang_and_eave_cut() {
        use plan_core::defaults::EaveCut;
        let mut cx = house();
        let settings_ = roof_view::RoofSettings::from_defaults(&cx.defaults);
        roof_view::rebuild(&mut cx.project, 0, settings_, false).unwrap();
        let specs = eave_specs(cx.floor());
        assert!(!specs.is_empty());
        let overhang = roof_view::load(cx.floor()).planes[0].overhang;
        assert_eq!(specs[0].overhang, overhang);
        assert_eq!(specs[0].cut, Some(TailCut::Plumb), "the roof detail's cut");
        build(&mut cx, false);
        let rafters: Vec<&Member> = cx
            .framing
            .iter()
            .filter(|m| m.kind == MemberKind::Rafter && !m.cuts.is_empty())
            .collect();
        assert!(rafters.len() > 10);
        assert!(rafters.iter().all(|r| r.cuts.tail == Some(TailCut::Plumb)));
        assert!(rafters.iter().all(|r| r.cuts.birdsmouth.is_some()));
        // A plane's own eave choice wins.
        let mut set = roof_view::load(cx.floor());
        for p in &mut set.planes {
            p.eave.eave_cut = Some(EaveCut::Level);
        }
        roof_view::store(&mut cx.project, 0, &mut set);
        assert!(eave_specs(cx.floor())
            .iter()
            .all(|e| e.cut == Some(TailCut::Level)));
        build(&mut cx, false);
        assert!(cx
            .framing
            .iter()
            .filter(|m| m.kind == MemberKind::Rafter && !m.cuts.is_empty())
            .all(|r| r.cuts.tail == Some(TailCut::Level)));
    }

    #[test]
    fn the_cut_table_lists_each_member_type_size_and_cut_length() {
        let mut cx = house();
        build(&mut cx, false);
        let data = takeoff_data(&cx.project, 0, false);
        assert_eq!(
            data.cut_columns,
            [
                "Member",
                "Size",
                "Cut Length",
                "Qty",
                "Linear ft",
                "Board ft"
            ]
        );
        let studs: Vec<_> = data.cut_rows.iter().filter(|r| r[0] == "stud").collect();
        assert!(!studs.is_empty());
        // Studs are 2x6 cut to 104 5/8".
        assert!(studs.iter().any(|r| r[1] == "2x6" && r[2] == "104 5/8\""));
        let kinds: Vec<&str> = data.cut_rows.iter().map(|r| r[0].as_str()).collect();
        for want in [
            "top plate",
            "bottom plate",
            "header",
            "joist",
            "rim joist",
            "corner stud",
            "blocking",
        ] {
            assert!(kinds.contains(&want), "{want} missing: {kinds:?}");
        }
        // Members of one type sit together and the table ends with a total.
        let first_stud = kinds.iter().position(|k| *k == "stud").unwrap();
        let last_stud = kinds.iter().rposition(|k| *k == "stud").unwrap();
        assert!(kinds[first_stud..=last_stud].iter().all(|k| *k == "stud"));
        let total = data.cut_rows.last().unwrap();
        assert_eq!(total[0], "Total");
        let qty: u32 = data.cut_rows[..data.cut_rows.len() - 1]
            .iter()
            .map(|r| r[3].parse::<u32>().unwrap())
            .sum();
        assert_eq!(total[3], qty.to_string());
        assert_eq!(qty as usize, data.members);
        // Linear feet = qty x cut length; the CSV has the same rows.
        let r = studs.iter().find(|r| r[1] == "2x6").unwrap();
        let (n, lf): (f64, f64) = (r[3].parse().unwrap(), r[4].parse().unwrap());
        assert!((lf - n * 104.625 / 12.0).abs() < 0.06, "{lf}");
        assert_eq!(data.cut_csv.lines().count(), data.cut_rows.len() + 1);
        assert!(data
            .cut_csv
            .starts_with("Member,Size,Cut Length,Qty,Linear ft,Board ft\n"));
        // Manual members join the table.
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(10.0, 10.0),
            Point::new(130.0, 10.0),
        );
        let more = takeoff_data(&cx.project, 0, false);
        assert_eq!(more.members, data.members + 1);
        assert!(more
            .cut_rows
            .iter()
            .any(|r| r[0] == "joist" && r[2] == "120\""));
    }

    #[test]
    fn the_framing_overview_shows_only_the_framing_layers() {
        let mut cx = house();
        build(&mut cx, false);
        assert!(cx.layers().is_visible("Walls, Normal") || cx.layers().is_visible("Walls"));
        assert!(!in_overview(&cx.project));
        activate_overview(&mut cx);
        assert!(in_overview(&cx.project));
        assert_eq!(cx.undo_label(), Some("Framing Overview"));
        // Framing layers show; the walls, rooms, doors and the rest are hidden.
        let layers = cx.layers().clone();
        let shown: Vec<&str> = layers
            .layers
            .iter()
            .filter(|l| l.display)
            .map(|l| l.name.as_str())
            .collect();
        assert!(!shown.is_empty());
        assert!(shown.iter().all(|n| is_framing_layer(n)), "{shown:?}");
        assert!(shown.contains(&"Framing"));
        for hidden in ["Doors", "Rooms", "Dimensions, Manual"] {
            if layers.get(hidden).is_some() {
                assert!(!layers.is_visible(hidden), "{hidden}");
            }
        }
        // The base layers are untouched, so leaving brings the plan back.
        assert!(cx.project.layers.is_visible("Doors"));
        leave_overview(&mut cx);
        assert!(!in_overview(&cx.project));
        assert!(cx.layers().is_visible("Doors"));
        // Activating again reuses the set; a layer made since stays out of it.
        cx.project.layers.add(Layer::new("Extra", [0, 0, 0], 18));
        activate_overview(&mut cx);
        assert!(!cx.layers().is_visible("Extra"));
        assert_eq!(
            cx.project
                .layer_sets
                .names()
                .iter()
                .filter(|n| **n == OVERVIEW)
                .count(),
            1
        );
        // An unframed floor is framed on the way in.
        let mut cx = house();
        activate_overview(&mut cx);
        assert!(!cx.framing.is_empty());
        // The manual framing layers are in the overview too.
        ensure_manual_layers(&mut cx.project);
        install_overview(&mut cx.project);
        cx.refresh();
        assert!(cx.layers().is_visible(LAYER_JOISTS));
    }

    #[test]
    fn the_elevation_scene_swaps_wall_skins_for_wall_framing_in_the_overview() {
        let mut cx = house();
        build(&mut cx, false);
        let plain = elevation_scene(&cx.project);
        let base = plan_3d::build_scene(&cx.project);
        assert_eq!(
            plain.meshes.len(),
            base.meshes.len(),
            "no change outside the overview"
        );
        let wall_ids: Vec<Id> = cx.floor().walls.iter().map(|w| w.id).collect();
        let skin = |m: &plan_3d::Mesh| {
            WALL_SURFACES.contains(&m.material)
                && m.object_id.is_some_and(|id| wall_ids.contains(&id))
        };
        assert!(base.meshes.iter().any(skin));
        activate_overview(&mut cx);
        let scene = elevation_scene(&cx.project);
        assert!(!scene.meshes.iter().any(skin), "the wall skins are gone");
        let studs = scene
            .meshes
            .iter()
            .filter(|m| m.material == plan_3d::Material::Framing)
            .count();
        let walls = load(cx.floor())
            .iter()
            .filter(|m| m.wall_id.is_some())
            .count();
        assert_eq!(studs, walls);
        assert!(studs > 40);
        // The meshes are the members' own boxes: the first stud's bounds match.
        let m = load(cx.floor())
            .into_iter()
            .find(|m| m.kind == MemberKind::Stud)
            .unwrap();
        let (lo, hi) = m.mesh().bounds().unwrap();
        assert!(hi[1] > lo[1]);
        // The 3D view gets the automatic members too.
        let all = manual_framing_meshes(&cx.project);
        assert!(all.len() >= load(cx.floor()).len());
        cx.project.layers.get_mut(LAYER).unwrap().display = false;
        assert!(manual_framing_meshes(&cx.project).len() < all.len());
    }

    // ----- Round 14: Build Framing options -----

    fn members_of_wall(cx: &EditorContext, wall: Id) -> Vec<Member> {
        cx.framing
            .iter()
            .filter(|m| m.wall_id == Some(wall))
            .cloned()
            .collect()
    }

    fn with_build(cx: &mut EditorContext, edit: impl FnOnce(&mut FramingSettings)) {
        let mut st = settings(&cx.project);
        edit(&mut st);
        set_settings(cx, st);
    }

    #[test]
    fn a_retained_wall_keeps_its_framing_through_a_rebuild() {
        let mut cx = house();
        build(&mut cx, false);
        let ids: Vec<Id> = cx.floor().walls.iter().map(|w| w.id).collect();
        let kept = members_of_wall(&cx, ids[1]);
        assert!(!kept.is_empty());
        assert_eq!(set_walls_retained(&mut cx, &[ids[1]], true), 1);
        assert_eq!(cx.undo_label(), Some("Retain Wall Framing"));
        assert_eq!(
            set_walls_retained(&mut cx, &[ids[1]], true),
            0,
            "already retained"
        );
        assert!(wall_retained(&cx.project, ids[1]));
        // Lower every wall and rebuild: the retained wall keeps its tall studs.
        for w in &mut cx.project.floors[0].walls {
            w.height = 96.0;
        }
        build(&mut cx, false);
        assert_eq!(members_of_wall(&cx, ids[1]), kept);
        fn studs(cx: &EditorContext, id: Id) -> f64 {
            members_of_wall(cx, id)
                .iter()
                .filter(|m| m.kind == MemberKind::Stud)
                .map(|m| m.length)
                .fold(0.0, f64::max)
        }
        assert!(studs(&cx, ids[2]) < 95.0, "the others were rebuilt at 96\"");
        assert!(studs(&cx, ids[1]) > 100.0);
        // Not retained any more: the next build frames it again.
        set_walls_retained(&mut cx, &[ids[1]], false);
        build(&mut cx, false);
        assert!(studs(&cx, ids[1]) < 95.0);
    }

    #[test]
    fn a_group_that_is_off_or_retained_keeps_what_it_has() {
        let mut cx = house();
        build(&mut cx, false);
        let joists = count_kind(&cx.framing, MemberKind::Joist);
        assert!(joists > 5);
        // Floor framing off: a tighter spacing is not applied.
        with_build(&mut cx, |st| {
            st.walls.joist_spacing = 8.0;
            st.build.build.floor = false;
        });
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::Joist), joists);
        // On again: rebuilt at 8".
        with_build(&mut cx, |st| st.build.build.floor = true);
        build(&mut cx, false);
        let tight = count_kind(&cx.framing, MemberKind::Joist);
        assert!(tight > joists);
        // Retained: a wider spacing is not applied either, the walls still rebuild.
        with_build(&mut cx, |st| {
            st.walls.joist_spacing = 24.0;
            st.build.retain.floor = true;
        });
        let walls = count_kind(&cx.framing, MemberKind::Stud);
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::Joist), tight);
        assert_eq!(count_kind(&cx.framing, MemberKind::Stud), walls);
    }

    #[test]
    fn the_build_dialog_settings_and_the_build_are_one_undo_step() {
        let mut cx = house();
        let mut st = settings(&cx.project);
        st.build.auto_rebuild.wall = true;
        st.build_on_ok = Some(false);
        let before = cx.action_history().0.len();
        set_settings(&mut cx, st);
        assert_eq!(cx.action_history().0.len(), before + 1);
        assert_eq!(cx.undo_label(), Some("Build Framing"));
        assert!(!cx.framing.is_empty());
        assert!(settings(&cx.project).build.auto_rebuild.wall);
        assert_eq!(settings(&cx.project).build_on_ok, None, "never saved");
        cx.undo();
        assert!(cx.floor().framing.is_empty());
        assert!(!settings(&cx.project).build.auto_rebuild.wall);
    }

    #[test]
    fn the_ceiling_group_frames_ceiling_joists_on_the_plates() {
        let mut cx = house();
        build(&mut cx, false);
        assert_eq!(
            count_kind(&cx.framing, MemberKind::CeilingJoist),
            0,
            "off by default"
        );
        with_build(&mut cx, |st| st.build.build.ceiling = true);
        let s = build(&mut cx, false);
        let n = count_kind(&cx.framing, MemberKind::CeilingJoist);
        assert!(n > 5 && s.ceiling == n, "{s:?}");
        for m in cx
            .framing
            .iter()
            .filter(|m| m.kind == MemberKind::CeilingJoist)
        {
            let bottom = m.transform.origin[1] - m.lumber.depth / 2.0;
            assert!((bottom - 109.125).abs() < 1e-6, "{bottom}");
        }
        // Trusses bring their own bottom chords.
        with_build(&mut cx, |st| st.roof.trusses = true);
        build(&mut cx, false);
        assert_eq!(count_kind(&cx.framing, MemberKind::CeilingJoist), 0);
    }

    #[test]
    fn open_below_rooms_get_no_floor_joists_and_the_rooms_under_them_no_ceiling() {
        let mut cx = house();
        with_build(&mut cx, |st| st.build.build.ceiling = true);
        cx.project.build_new_floor(true);
        let both = frame_floor_all(&cx.project, 0, false);
        assert!(both.summary.ceiling > 0 && both.summary.floor > 0);
        let upper = frame_floor_all(&cx.project, 1, false);
        assert!(upper.summary.floor > 0, "{:?}", upper.summary);
        let mut open = plan_core::RoomName::new(Point::new(120.0, 96.0), "Foyer", "Foyer");
        open.has_floor = false;
        cx.project.floors[1].room_names.push(open);
        let after_up = frame_floor_all(&cx.project, 1, false);
        assert_eq!(after_up.summary.floor, 0, "no floor platform to frame");
        let after_down = frame_floor_all(&cx.project, 0, false);
        assert_eq!(
            after_down.summary.ceiling, 0,
            "the room under it is open to it"
        );
        assert!(after_down.summary.floor > 0);
        // A room with no ceiling of its own gets none either.
        let mut cx = house();
        with_build(&mut cx, |st| st.build.build.ceiling = true);
        let mut deck = plan_core::RoomName::new(Point::new(120.0, 96.0), "Deck", "Deck");
        deck.has_ceiling = false;
        cx.project.floors[0].room_names.push(deck);
        assert_eq!(frame_floor_all(&cx.project, 0, false).summary.ceiling, 0);
    }

    #[test]
    fn bearing_mode_lets_joists_run_across_interior_partitions() {
        let mut cx = house();
        cx.project.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 192.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        cx.refresh();
        let longest = |cx: &EditorContext| {
            frame_floor_all(&cx.project, 0, false)
                .members
                .iter()
                .filter(|m| m.kind == MemberKind::Joist)
                .map(|m| m.length)
                .fold(0.0, f64::max)
        };
        let every = longest(&cx);
        assert!(every < 125.0, "two 120\" rooms: {every}");
        with_build(&mut cx, |st| {
            st.walls.bearing = BearingMode::ExteriorAndBearingLines;
        });
        let exterior = longest(&cx);
        assert!(exterior > 180.0, "one 240 x 192 platform: {exterior}");
    }

    #[test]
    fn the_joist_direction_and_a_double_rim_come_from_the_build_options() {
        let mut cx = house();
        let run = |cx: &EditorContext| {
            frame_floor_all(&cx.project, 0, false)
                .members
                .into_iter()
                .filter(|m| m.kind == MemberKind::Joist)
                .collect::<Vec<_>>()
        };
        // 240 x 192: Auto spans the short side, 192.
        assert!(run(&cx).iter().all(|m| m.length > 180.0));
        with_build(&mut cx, |st| {
            st.walls.joist_direction = plan_framing::JoistDirection::AlongX
        });
        assert!(run(&cx).iter().all(|m| m.length > 230.0));
        let rims = |cx: &EditorContext| {
            frame_floor_all(&cx.project, 0, false)
                .members
                .iter()
                .filter(|m| m.kind == MemberKind::RimJoist)
                .count()
        };
        let single = rims(&cx);
        with_build(&mut cx, |st| st.walls.rim_plies = 2);
        assert_eq!(rims(&cx), single * 2);
    }

    #[test]
    fn auto_rebuild_follows_edits_for_automatic_groups_only() {
        let mut cx = house();
        with_build(&mut cx, |st| st.build.auto_rebuild.wall = true);
        // Nothing is built yet: nothing to rebuild.
        cx.refresh();
        assert!(!auto_rebuild(&mut cx));
        build(&mut cx, false);
        cx.refresh();
        assert!(!auto_rebuild(&mut cx), "the build is current");
        let joists = count_kind(&cx.framing, MemberKind::Joist);
        let ids: Vec<Id> = cx.floor().walls.iter().map(|w| w.id).collect();
        for w in &mut cx.project.floors[0].walls {
            w.height = 96.0;
        }
        cx.mark_dirty();
        cx.refresh();
        assert!(auto_rebuild(&mut cx));
        let stud = members_of_wall(&cx, ids[2])
            .iter()
            .filter(|m| m.kind == MemberKind::Stud)
            .map(|m| m.length)
            .fold(0.0, f64::max);
        assert!(stud < 95.0);
        assert_eq!(
            count_kind(&cx.framing, MemberKind::Joist),
            joists,
            "floors are not automatic"
        );
        cx.refresh();
        assert!(!auto_rebuild(&mut cx), "up to date again");
        // Retained walls stay put even when due.
        set_walls_retained(&mut cx, &[ids[2]], true);
        for w in &mut cx.project.floors[0].walls {
            w.height = 100.0;
        }
        cx.mark_dirty();
        cx.refresh();
        assert!(auto_rebuild(&mut cx));
        let after = members_of_wall(&cx, ids[2])
            .iter()
            .filter(|m| m.kind == MemberKind::Stud)
            .map(|m| m.length)
            .fold(0.0, f64::max);
        assert_eq!(after, stud);
    }

    #[test]
    fn framing_layers_start_off_and_a_build_turns_on_what_it_uses() {
        let mut project = Project::new("t");
        ensure_manual_layers(&mut project);
        for (name, _) in MANUAL_LAYERS {
            assert!(!project.layers.is_visible(name), "{name}");
        }
        let mut cx = house();
        ensure_manual_layers(&mut cx.project);
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        );
        assert!(cx.layers().is_visible(LAYER_JOISTS));
        assert!(!cx.layers().is_visible(LAYER_RAFTERS));
        // Show layers off: a build leaves the Framing layer as it is.
        cx.project.layers.get_mut(LAYER).unwrap().display = false;
        with_build(&mut cx, |st| st.build.show_layers = false);
        build(&mut cx, false);
        assert!(!cx.layers().is_visible(LAYER));
    }

    #[test]
    fn plan_styles_follow_the_member_kind() {
        let mut cx = house();
        with_build(&mut cx, |st| st.build.build.ceiling = true);
        let roof = roof_view::RoofSettings::from_defaults(&cx.defaults);
        roof_view::rebuild(&mut cx.project, 0, roof, false).unwrap();
        build(&mut cx, false);
        let style_of = |k: MemberKind| {
            cx.framing
                .iter()
                .find(|m| m.kind == k)
                .map(plan_style)
                .unwrap_or_else(|| panic!("no {k:?}"))
        };
        let stud = style_of(MemberKind::Stud);
        assert!(stud.dash.is_none() && stud.fill_alpha > 0);
        let joist = style_of(MemberKind::Joist);
        assert!(joist.dash.is_none());
        let ceiling = style_of(MemberKind::CeilingJoist);
        assert!(ceiling.dash.is_some() && ceiling.fill_alpha == 0);
        let rafter = style_of(MemberKind::Rafter);
        assert!(rafter
            .dash
            .is_some_and(|(d, _)| d > ceiling.dash.unwrap().0));
        let blocking = cx
            .framing
            .iter()
            .find(|m| m.kind == MemberKind::Blocking && m.wall_id.is_some())
            .map(plan_style)
            .unwrap();
        assert!(blocking.fill_alpha < stud.fill_alpha);
    }

    #[test]
    fn spans_the_lumber_cannot_carry_are_reported_in_the_build() {
        let mut cx = house();
        with_build(&mut cx, |st| {
            st.walls.joist_size = plan_framing::TWO_BY_TWELVE
        });
        let fine = frame_floor_all(&cx.project, 0, false);
        assert!(fine.warnings.is_empty(), "{:?}", fine.warnings);
        with_build(&mut cx, |st| st.walls.joist_size = plan_framing::TWO_BY_SIX);
        let fb = frame_floor_all(&cx.project, 0, false);
        assert_eq!(fb.warnings.len(), 1, "{:?}", fb.warnings);
        assert!(fb.warnings[0].contains("2x6"), "{}", fb.warnings[0]);
        build(&mut cx, false);
        assert!(cx.status.contains("Check spans"), "{}", cx.status);
    }

    #[test]
    fn new_posts_take_the_posts_tab_and_trusses_the_trusses_tab() {
        let mut cx = house();
        let p = Point::new(10.0, 10.0);
        assert_eq!(
            new_member_in(&cx, ManualMemberKind::Post, p, p).kind,
            ManualMemberKind::Post
        );
        with_build(&mut cx, |st| {
            st.build.posts.size = plan_framing::LumberSize::SIX_BY_SIX;
            st.build.posts.footing = true;
        });
        let post = new_member_in(&cx, ManualMemberKind::Post, p, p);
        assert_eq!(post.kind, ManualMemberKind::PostWithFooting);
        assert_eq!(post.lumber, plan_framing::LumberSize::SIX_BY_SIX);
        assert!((post.width - 5.5).abs() < 1e-9);
        // Trusses over a base: type and spacing.
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
        build(&mut cx, false);
        let fink = built_of(&cx, ManualMemberKind::RoofTruss);
        assert!(fink
            .iter()
            .all(|t| t.truss.as_ref().unwrap().kind == plan_framing::TrussType::Fink));
        with_build(&mut cx, |st| {
            st.build.trusses.kind = plan_framing::TrussType::Howe;
            st.build.trusses.spacing = 48.0;
            st.build.trusses.overhang = 18.0;
        });
        build(&mut cx, false);
        let howe = built_of(&cx, ManualMemberKind::RoofTruss);
        assert!(
            !howe.is_empty() && howe.len() < fink.len(),
            "{} vs {}",
            howe.len(),
            fink.len()
        );
        let spec = howe[0].truss.as_ref().unwrap();
        assert_eq!(spec.kind, plan_framing::TrussType::Howe);
        assert_eq!(spec.overhang, 18.0);
    }

    #[test]
    fn the_overview_scene_is_the_framing_alone_even_with_the_layers_off() {
        let mut cx = house();
        place(
            &mut cx,
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
        );
        build(&mut cx, false);
        for name in [LAYER, LAYER_JOISTS] {
            cx.project.layers.get_mut(name).unwrap().display = false;
        }
        let scene = overview_scene(&cx.project);
        assert!(scene.meshes.len() > cx.framing.len());
        assert!(scene
            .meshes
            .iter()
            .all(|m| m.material == plan_3d::Material::Framing));
        let full = plan_3d::build_scene(&cx.project);
        assert!(full
            .meshes
            .iter()
            .any(|m| m.material != plan_3d::Material::Framing));
    }

    #[test]
    fn a_wall_detail_carries_dimensions_and_paints() {
        let mut cx = house();
        build(&mut cx, false);
        let walls: Vec<(Id, usize)> = cx
            .floor()
            .walls
            .iter()
            .map(|w| (w.id, cx.floor().openings_on(w.id).count()))
            .collect();
        let (door_wall, _) = *walls.iter().find(|(_, n)| *n > 0).unwrap();
        let (plain_wall, _) = *walls.iter().find(|(_, n)| *n == 0).unwrap();
        let d = wall_detail_of(&cx.project, 0, door_wall).unwrap();
        assert!(d
            .dims
            .iter()
            .any(|x| x.kind == plan_framing::DimKind::RoughWidth));
        assert!(!d.strokes.is_empty());
        let p = wall_detail_of(&cx.project, 0, plain_wall).unwrap();
        assert!(!p
            .dims
            .iter()
            .any(|x| x.kind == plan_framing::DimKind::RoughWidth));
        assert!(p
            .dims
            .iter()
            .any(|x| x.kind == plan_framing::DimKind::StudSpacing));
        // No framing, no detail.
        clear(&mut cx);
        assert!(wall_detail_of(&cx.project, 0, door_wall).is_none());
        // It paints.
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                paint_wall_detail(ui.painter(), ui.max_rect(), &d, Color32::BLACK);
            });
        });
        assert!(out.shapes.len() > 1);
    }
}
