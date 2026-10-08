//! Build Framing: wall, floor and roof framing from `plan-framing`, stored on
//! the floor and drawn in plan.
//!
//! # Storage
//!
//! The members of a floor live in the typed `Floor.framing` slot (one
//! serialized [`Member`] each), so they are saved with the plan and undone
//! with it. (The roofs keep their records in a hidden CAD layer because the
//! model had no roof slot when they were written; the framing slot already
//! existed, so no data layer is needed here.)
//!
//! # What is built
//!
//! * **Build Framing**: the active floor. Every wall except invisible, room
//!   divider and railing walls is framed with its openings
//!   ([`plan_framing::frame_wall`]); every room gets a floor platform
//!   ([`plan_framing::frame_floor`], joists across the short side); and when
//!   roof planes are stored on the floor they are framed
//!   ([`plan_framing::frame_roof`]).
//! * **Build All Framing**: the same for every floor.
//!
//! Members are drawn on layer `"Framing"` when that layer is visible, as the
//! plan outline of each piece of lumber.

use super::{Camera, EditorContext};
use crate::editor::roof_view;
use eframe::egui::{self, Color32, Pos2, Shape, Stroke};
use plan_core::geometry::Point;
use plan_core::{detect_rooms, Floor, FloorKind, Layer, Opening, Project};
use plan_framing::{
    frame_floor, frame_roof, frame_wall, roof_framing_takeoff, FramingDefaults, JoistDirection,
    Member, MemberKind, RoofFramingDefaults, Takeoff,
};
use plan_roof::{Roof, RoofPlane, DEFAULT_FASCIA_HEIGHT};

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

/// The members stored on `floor` (empty when there are none or the slot does
/// not parse).
pub fn load(floor: &Floor) -> Vec<Member> {
    if floor.framing.is_empty() {
        return Vec::new();
    }
    floor.framing_as::<Member>().unwrap_or_default()
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

/// Frames floor `fi` of `project`: the walls, the floor platforms and, with
/// `with_roof`, the roof stored on that floor. Returns the members and what
/// was built.
pub fn frame_floor_members(
    project: &Project,
    fi: usize,
    with_roof: bool,
) -> (Vec<Member>, BuildSummary) {
    let floor = &project.floors[fi];
    let d = FramingDefaults::default();
    let mut members = Vec::new();
    let mut summary = BuildSummary::default();

    for wall in floor
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && !w.flags.room_divider && !w.flags.railing)
    {
        let openings: Vec<&Opening> = floor.openings_on(wall.id).collect();
        let m = frame_wall(wall, &openings, floor.elevation, &d);
        summary.walls += m.len();
        members.extend(m);
    }
    if floor.kind != FloorKind::Foundation {
        for room in detect_rooms(&floor.walls, 0.5) {
            if room.area_sq_ft() < 1.0 {
                continue;
            }
            let m = frame_floor(&room, floor.elevation, &d, JoistDirection::Auto);
            summary.floor += m.len();
            members.extend(m);
        }
    }
    if with_roof {
        if let Some(roof) = roof_of(floor) {
            let m = frame_roof(&roof, &RoofFramingDefaults::default());
            summary.roof += m.len();
            members.extend(m);
        }
    }
    (members, summary)
}

/// Build Framing (active floor) or Build All Framing (every floor) as one
/// undo step. Turns the Framing layer on so the result shows.
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
    for fi in floors {
        let (members, summary) = frame_floor_members(&cx.project, fi, true);
        let _ = cx.project.floors[fi].set_framing(&members);
        total.add(summary);
    }
    ensure_layer(&mut cx.project);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!(
        "Built framing: {} wall, {} floor and {} roof members",
        total.walls, total.floor, total.roof
    );
    total
}

/// Removes the framing of the active floor (one undo step). Returns how many
/// members went.
pub fn clear(cx: &mut EditorContext) -> usize {
    let n = cx.floor().framing.len();
    if n == 0 {
        cx.status = "There is no framing to delete".into();
        return 0;
    }
    cx.begin_change("Delete Framing");
    cx.floor_mut().framing.clear();
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
    let t = takeoff_of(members);
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
    let (cols, rows) = takeoff_table(members);
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

/// Draws the active floor's framing when the Framing layer is shown.
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if cx.framing.is_empty() || !cx.layers().is_visible(LAYER) {
        return;
    }
    let [r, g, b] = cx.layers().get(LAYER).map_or([180, 140, 60], |l| l.color);
    let fill = Color32::from_rgba_unmultiplied(r, g, b, 70);
    let edge = Stroke::new(0.75_f32, Color32::from_rgb(r, g, b));
    for m in &cx.framing {
        let hull = plan_outline(m);
        let pts: Vec<Pos2> = hull.iter().map(|p| cam.world_to_screen(*p)).collect();
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
}
