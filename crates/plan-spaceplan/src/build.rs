//! "Build House": convert room boxes into walls, doors, windows and room names.

use crate::boxes::{Rect, RoomBox};
use plan_core::{
    detect_rooms, Floor, Id, OpeningKind, Point, Project, WallKind, DEFAULT_CEILING_HEIGHT,
    DEFAULT_EXTERIOR_THICKNESS, DEFAULT_INTERIOR_THICKNESS,
};

/// Coordinates closer than this (inches) are the same coordinate.
const COORD_TOL: f64 = 0.01;
/// Interior segments shorter than this get no door.
const MIN_DOOR_SEGMENT: f64 = 40.0;
/// Width of the door between the garage and the house, inches.
const GARAGE_DOOR_WIDTH: f64 = 36.0;
/// Width of the front door, inches.
const FRONT_DOOR_WIDTH: f64 = 36.0;
/// Clear distance between a window and a wall corner, inches.
const WINDOW_CORNER_CLEARANCE: f64 = 24.0;
/// Tolerance for room detection, inches.
const ROOM_TOL: f64 = 0.5;

/// Options for [`build_house`]. Lengths in inches.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildOptions {
    pub exterior_thickness: f64,
    pub interior_thickness: f64,
    pub wall_height: f64,
    /// Interior door width (the garage door is always 36").
    pub door_width: f64,
    pub window_width: f64,
    /// Center-to-center distance between windows on one wall.
    pub window_spacing: f64,
    /// Also put windows in baths.
    pub bath_window: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            exterior_thickness: DEFAULT_EXTERIOR_THICKNESS,
            interior_thickness: DEFAULT_INTERIOR_THICKNESS,
            wall_height: DEFAULT_CEILING_HEIGHT,
            door_width: 32.0,
            window_width: 36.0,
            window_spacing: 96.0,
            bath_window: true,
        }
    }
}

/// What [`build_house`] created.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildReport {
    pub walls: Vec<Id>,
    pub doors: Vec<Id>,
    pub windows: Vec<Id>,
    /// Detected rooms that received the name of their box.
    pub rooms_named: usize,
}

/// A stretch of a [`BuiltWall`] with a constant set of adjoining boxes.
struct Run {
    /// Start/end along the wall axis (absolute coordinate), inches.
    s: f64,
    e: f64,
    /// Indices into the floor's solid boxes (1 = exterior, 2 = shared).
    owners: Vec<usize>,
}

struct BuiltWall {
    id: Id,
    horizontal: bool,
    /// Fixed coordinate of the centerline.
    line: f64,
    /// Absolute coordinate of the wall start along its axis.
    start: f64,
    runs: Vec<Run>,
}

/// A wall being assembled from consecutive [`Seg`]s of one class.
struct Pending {
    class: WallKind,
    horizontal: bool,
    /// Index of the fixed coordinate.
    line: usize,
    /// Coordinate indices along the axis the wall spans.
    lo: usize,
    hi: usize,
    runs: Vec<Run>,
}

/// One grid-cell-sized piece of a box edge (between neighboring coordinates).
struct Seg {
    horizontal: bool,
    line: usize,
    lo: usize,
    owners: Vec<usize>,
}

/// Convert room boxes to walls on `project`, floor by floor.
///
/// 1. Every box edge is split at all box corner coordinates of its floor (so
///    edges of neighbors line up exactly).
/// 2. A piece shared by two boxes becomes an **interior** wall; a piece with a
///    box on one side only is an **exterior** wall.
/// 3. Collinear touching pieces of the same class are merged into one wall
///    (wall centerlines run along the box edges).
/// 4. One door is placed, centered, on every shared stretch of 40" or more
///    (36" for the garage, else `door_width`); windows go on the exterior
///    stretches of bedrooms, living, dining and kitchen (and baths when
///    `bath_window`) every `window_spacing`, at least 24" from corners; an
///    Entry box additionally gets a 36" front door, preferably on the side
///    facing a porch.
/// 5. Rooms are detected and named after the box containing their centroid.
///
/// Deck and Porch boxes are outdoor space: they get no walls (their shared
/// edge with the house is an exterior wall). Floors missing from `project`
/// are added.
pub fn build_house(project: &mut Project, boxes: &[RoomBox], opts: &BuildOptions) -> BuildReport {
    let mut report = BuildReport::default();
    let mut floors: Vec<usize> = boxes.iter().map(|b| b.floor).collect();
    floors.sort_unstable();
    floors.dedup();
    for f in floors {
        build_floor(project, f, boxes, opts, &mut report);
    }
    report
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix} Floor")
}

fn ensure_floor(project: &mut Project, index: usize) {
    while project.floors.len() <= index {
        let elevation = project
            .floors
            .last()
            .map_or(0.0, |f| f.elevation + f.ceiling_height + 12.0);
        let name = ordinal(project.floors.len() + 1);
        project.floors.push(Floor::new(name, elevation));
    }
}

fn cluster(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for x in v {
        if out.last().is_none_or(|l| x - l > COORD_TOL) {
            out.push(x);
        }
    }
    out
}

fn index_of(vals: &[f64], x: f64) -> usize {
    vals.iter()
        .position(|v| (v - x).abs() <= COORD_TOL)
        .unwrap_or(0)
}

fn is_outdoor(b: &RoomBox) -> bool {
    matches!(b.room_type.as_str(), "Deck" | "Porch")
}

fn wants_window(room_type: &str, bath_window: bool) -> bool {
    match room_type {
        "Master Bedroom" | "Bedroom" | "Living" | "Dining" | "Kitchen" => true,
        "Bath" | "Master Bath" | "Powder Room" => bath_window,
        _ => false,
    }
}

/// Split all box edges into [`Seg`]s using coordinate-index arithmetic, so
/// shared edges are matched exactly.
fn edge_segments(solid: &[&RoomBox], xs: &[f64], ys: &[f64]) -> Vec<Seg> {
    // (x0, x1, y0, y1) as indices into xs / ys.
    let idx: Vec<[usize; 4]> = solid
        .iter()
        .map(|b| {
            [
                index_of(xs, b.rect.0.x),
                index_of(xs, b.rect.1.x),
                index_of(ys, b.rect.0.y),
                index_of(ys, b.rect.1.y),
            ]
        })
        .collect();
    let mut segs = Vec::new();
    for j in 0..ys.len() {
        for i in 0..xs.len().saturating_sub(1) {
            let owners: Vec<usize> = (0..idx.len())
                .filter(|&k| {
                    let [x0, x1, y0, y1] = idx[k];
                    (y0 == j || y1 == j) && x0 <= i && i < x1
                })
                .collect();
            if !owners.is_empty() {
                segs.push(Seg {
                    horizontal: true,
                    line: j,
                    lo: i,
                    owners,
                });
            }
        }
    }
    for i in 0..xs.len() {
        for j in 0..ys.len().saturating_sub(1) {
            let owners: Vec<usize> = (0..idx.len())
                .filter(|&k| {
                    let [x0, x1, y0, y1] = idx[k];
                    (x0 == i || x1 == i) && y0 <= j && j < y1
                })
                .collect();
            if !owners.is_empty() {
                segs.push(Seg {
                    horizontal: false,
                    line: i,
                    lo: j,
                    owners,
                });
            }
        }
    }
    segs
}

fn class_of(owners: &[usize]) -> WallKind {
    if owners.len() >= 2 {
        WallKind::Interior
    } else {
        WallKind::Exterior
    }
}

fn set_opening_width(project: &mut Project, floor: usize, id: Id, width: f64) {
    if let Some(o) = project.floors[floor]
        .openings
        .iter_mut()
        .find(|o| o.id == id)
    {
        o.width = width;
    }
}

fn place(
    project: &mut Project,
    floor: usize,
    wall: &BuiltWall,
    center: f64,
    kind: OpeningKind,
    width: f64,
) -> Option<Id> {
    let id = project.add_opening(floor, wall.id, center - wall.start, kind)?;
    set_opening_width(project, floor, id, width);
    Some(id)
}

/// True when `rect` has a side lying along the wall stretch `[s, e]` at `line`.
fn faces(rect: &Rect, horizontal: bool, line: f64, s: f64, e: f64) -> bool {
    let (fixed_lo, fixed_hi, lo, hi) = if horizontal {
        (rect.0.y, rect.1.y, rect.0.x, rect.1.x)
    } else {
        (rect.0.x, rect.1.x, rect.0.y, rect.1.y)
    };
    ((fixed_lo - line).abs() < COORD_TOL || (fixed_hi - line).abs() < COORD_TOL)
        && hi.min(e) - lo.max(s) > COORD_TOL
}

fn build_floor(
    project: &mut Project,
    floor: usize,
    boxes: &[RoomBox],
    opts: &BuildOptions,
    report: &mut BuildReport,
) {
    ensure_floor(project, floor);
    let solid: Vec<&RoomBox> = boxes
        .iter()
        .filter(|b| b.floor == floor && !is_outdoor(b))
        .collect();
    if solid.is_empty() {
        return;
    }
    let porches: Vec<&RoomBox> = boxes
        .iter()
        .filter(|b| b.floor == floor && b.room_type == "Porch")
        .collect();
    let xs = cluster(
        solid
            .iter()
            .flat_map(|b| [b.rect.0.x, b.rect.1.x])
            .collect(),
    );
    let ys = cluster(
        solid
            .iter()
            .flat_map(|b| [b.rect.0.y, b.rect.1.y])
            .collect(),
    );

    // Steps 1-3: classify the pieces and merge them into walls.
    let segs = edge_segments(&solid, &xs, &ys);
    let mut pending: Vec<Pending> = Vec::new();
    for seg in &segs {
        let class = class_of(&seg.owners);
        let coords = if seg.horizontal { &xs } else { &ys };
        let (s, e) = (coords[seg.lo], coords[seg.lo + 1]);
        let continues = pending.last().is_some_and(|p| {
            p.class == class
                && p.line == seg.line
                && p.hi == seg.lo
                && p.horizontal == seg.horizontal
        });
        if !continues {
            pending.push(Pending {
                class,
                horizontal: seg.horizontal,
                line: seg.line,
                lo: seg.lo,
                hi: seg.lo,
                runs: Vec::new(),
            });
        }
        if let Some(p) = pending.last_mut() {
            p.hi = seg.lo + 1;
            match p.runs.last_mut() {
                Some(r) if r.owners == seg.owners => r.e = e,
                _ => p.runs.push(Run {
                    s,
                    e,
                    owners: seg.owners.clone(),
                }),
            }
        }
    }
    let mut walls: Vec<BuiltWall> = Vec::with_capacity(pending.len());
    for p in pending {
        let (coords, line_coords) = if p.horizontal { (&xs, &ys) } else { (&ys, &xs) };
        let (start, end, line) = (coords[p.lo], coords[p.hi], line_coords[p.line]);
        let (a, b) = if p.horizontal {
            (Point::new(start, line), Point::new(end, line))
        } else {
            (Point::new(line, start), Point::new(line, end))
        };
        let thickness = match p.class {
            WallKind::Exterior => opts.exterior_thickness,
            WallKind::Interior => opts.interior_thickness,
        };
        let id = project.add_wall(floor, a, b, thickness, opts.wall_height, p.class);
        walls.push(BuiltWall {
            id,
            horizontal: p.horizontal,
            line,
            start,
            runs: p.runs,
        });
    }
    report.walls.extend(walls.iter().map(|w| w.id));

    // Step 4a: doors between rooms.
    for w in &walls {
        for run in w.runs.iter().filter(|r| r.owners.len() >= 2) {
            if run.e - run.s < MIN_DOOR_SEGMENT {
                continue;
            }
            let garage = run.owners.iter().any(|&k| solid[k].room_type == "Garage");
            let width = if garage {
                GARAGE_DOOR_WIDTH
            } else {
                opts.door_width
            };
            let center = (run.s + run.e) * 0.5;
            if let Some(id) = place(project, floor, w, center, OpeningKind::Door, width) {
                report.doors.push(id);
            }
        }
    }

    // Step 4b: a front door per Entry box, then windows.
    for (k, b) in solid.iter().enumerate() {
        if b.room_type != "Entry" {
            continue;
        }
        let mut best: Option<(bool, f64, usize, usize)> = None;
        for (wi, w) in walls.iter().enumerate() {
            for (ri, run) in w.runs.iter().enumerate() {
                let len = run.e - run.s;
                if run.owners != [k] || len < MIN_DOOR_SEGMENT {
                    continue;
                }
                let porch = porches
                    .iter()
                    .any(|p| faces(&p.rect, w.horizontal, w.line, run.s, run.e));
                if best.is_none_or(|(bp, bl, _, _)| (porch, len) > (bp, bl)) {
                    best = Some((porch, len, wi, ri));
                }
            }
        }
        if let Some((_, _, wi, ri)) = best {
            let w = &walls[wi];
            let run = &w.runs[ri];
            let center = (run.s + run.e) * 0.5;
            if let Some(id) = place(
                project,
                floor,
                w,
                center,
                OpeningKind::Door,
                FRONT_DOOR_WIDTH,
            ) {
                report.doors.push(id);
            }
        }
    }
    for w in &walls {
        for run in w.runs.iter().filter(|r| r.owners.len() == 1) {
            let room_type = &solid[run.owners[0]].room_type;
            if !wants_window(room_type, opts.bath_window) {
                continue;
            }
            for center in window_centers(run.s, run.e, opts) {
                if let Some(id) = place(
                    project,
                    floor,
                    w,
                    center,
                    OpeningKind::Window,
                    opts.window_width,
                ) {
                    report.windows.push(id);
                }
            }
        }
    }

    // Step 5: name the detected rooms after their boxes.
    let rooms = detect_rooms(&project.floors[floor].walls, ROOM_TOL);
    for room in &rooms {
        let c = room.centroid;
        let hit = solid.iter().find(|b| {
            c.x >= b.rect.0.x && c.x <= b.rect.1.x && c.y >= b.rect.0.y && c.y <= b.rect.1.y
        });
        if let Some(b) = hit {
            project.set_room_name(floor, c, b.name.clone(), b.room_type.clone(), &rooms);
            report.rooms_named += 1;
        }
    }
}

/// Window centers (absolute coordinates) for an exterior stretch `[s, e]`:
/// as many as fit at `window_spacing`, kept 24" from the ends and centered.
fn window_centers(s: f64, e: f64, opts: &BuildOptions) -> Vec<f64> {
    let avail = e - s - 2.0 * WINDOW_CORNER_CLEARANCE;
    if avail < opts.window_width {
        return Vec::new();
    }
    let n = 1 + ((avail - opts.window_width) / opts.window_spacing).floor() as usize;
    let span = (n - 1) as f64 * opts.window_spacing + opts.window_width;
    let first = s + WINDOW_CORNER_CLEARANCE + (avail - span) * 0.5 + opts.window_width * 0.5;
    (0..n)
        .map(|i| first + i as f64 * opts.window_spacing)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::{dist_to_segment, point_in_polygon};

    fn bx(id: Id, name: &str, ty: &str, x: f64, y: f64, w: f64, h: f64) -> RoomBox {
        RoomBox::new(id, name, ty, Point::new(x, y), (w, h), 0)
    }

    #[test]
    fn two_adjacent_boxes_make_five_walls_one_door_two_rooms() {
        let boxes = vec![
            bx(1, "Living Room", "Living", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Kitchen", "Kitchen", 144.0, 0.0, 144.0, 144.0),
        ];
        let mut p = Project::new("t");
        let r = build_house(&mut p, &boxes, &BuildOptions::default());
        let f = &p.floors[0];
        assert_eq!(r.walls.len(), 5);
        let ext = f
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior)
            .count();
        assert_eq!(ext, 4);
        assert_eq!(r.doors.len(), 1);
        let door = f.openings.iter().find(|o| o.id == r.doors[0]).unwrap();
        let wall = f.wall(door.wall_id).unwrap();
        assert_eq!(wall.kind, WallKind::Interior);
        assert!((wall.length() - 144.0).abs() < 1e-9);
        assert!((door.center_offset - 72.0).abs() < 1e-9);
        assert!((door.width - 32.0).abs() < 1e-9);
        assert!(!r.windows.is_empty());

        let rooms = detect_rooms(&f.walls, 0.5);
        assert_eq!(rooms.len(), 2);
        assert_eq!(r.rooms_named, 2);
        for (b, name) in [(&boxes[0], "Living Room"), (&boxes[1], "Kitchen")] {
            let n = f
                .room_names
                .iter()
                .find(|n| {
                    point_in_polygon(
                        n.anchor,
                        &rooms
                            .iter()
                            .find(|r| point_in_polygon(b.center(), &r.polygon))
                            .unwrap()
                            .polygon,
                    )
                })
                .unwrap();
            assert_eq!(n.name, name);
            assert_eq!(n.room_type, b.room_type);
        }
    }

    #[test]
    fn l_shape_gives_three_connected_rooms() {
        let boxes = vec![
            bx(1, "A", "Living", 0.0, 0.0, 144.0, 144.0),
            bx(2, "B", "Kitchen", 144.0, 0.0, 144.0, 144.0),
            bx(3, "C", "Dining", 0.0, 144.0, 144.0, 120.0),
        ];
        let mut p = Project::new("t");
        let r = build_house(&mut p, &boxes, &BuildOptions::default());
        let f = &p.floors[0];
        assert_eq!(detect_rooms(&f.walls, 0.5).len(), 3);
        assert_eq!(r.rooms_named, 3);
        assert_eq!(r.doors.len(), 2);
        for w in &f.walls {
            for end in [w.start, w.end] {
                let touches = f
                    .walls
                    .iter()
                    .filter(|o| o.id != w.id)
                    .any(|o| dist_to_segment(end, o.start, o.end) <= 0.5);
                assert!(touches, "dangling end {end:?} of wall {}", w.id);
            }
        }
    }

    #[test]
    fn short_shared_wall_gets_no_door_and_garage_door_is_36() {
        let boxes = vec![
            bx(1, "Living", "Living", 0.0, 0.0, 144.0, 144.0),
            bx(2, "Closet", "Hall", 144.0, 0.0, 36.0, 30.0), // 30" with living, 36" with garage
            bx(3, "Garage", "Garage", 0.0, -264.0, 264.0, 264.0), // shares 144"
        ];
        let mut p = Project::new("t");
        let r = build_house(&mut p, &boxes, &BuildOptions::default());
        assert_eq!(r.doors.len(), 1);
        let d = p.floors[0]
            .openings
            .iter()
            .find(|o| o.id == r.doors[0])
            .unwrap();
        assert!((d.width - 36.0).abs() < 1e-9);
    }

    #[test]
    fn entry_gets_front_door_facing_porch_and_decks_get_no_walls() {
        let boxes = vec![
            bx(1, "Entry", "Entry", 0.0, 0.0, 96.0, 96.0),
            bx(2, "Living Room", "Living", 96.0, 0.0, 192.0, 192.0),
            bx(3, "Porch", "Porch", 0.0, -96.0, 168.0, 96.0),
            bx(4, "Deck", "Deck", 288.0, 0.0, 144.0, 192.0),
        ];
        let mut p = Project::new("t");
        let r = build_house(&mut p, &boxes, &BuildOptions::default());
        let f = &p.floors[0];
        // Porch and deck get no walls: everything stays inside the entry + living footprint.
        assert!(f
            .walls
            .iter()
            .all(|w| w.start.x >= -1e-9 && w.end.x <= 288.0 + 1e-9 && w.start.y >= -1e-9));
        assert_eq!(r.doors.len(), 2); // entry-living + front door
        let front = f
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .find(|o| f.wall(o.wall_id).unwrap().kind == WallKind::Exterior)
            .unwrap();
        let wall = f.wall(front.wall_id).unwrap();
        assert!((wall.start.y - 0.0).abs() < 1e-9 && (wall.end.y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn windows_keep_clear_of_corners_and_follow_spacing() {
        let boxes = vec![bx(
            1,
            "Master Bedroom",
            "Master Bedroom",
            0.0,
            0.0,
            300.0,
            144.0,
        )];
        let opts = BuildOptions::default();
        let mut p = Project::new("t");
        let r = build_house(&mut p, &boxes, &opts);
        let f = &p.floors[0];
        assert!(!r.windows.is_empty());
        for o in f.openings.iter().filter(|o| o.kind == OpeningKind::Window) {
            let len = f.wall(o.wall_id).unwrap().length();
            assert!(o.start_offset() >= 24.0 - 1e-9, "{o:?}");
            assert!(o.end_offset() <= len - 24.0 + 1e-9, "{o:?}");
        }
        // 300" wall: avail 252, n = 1 + floor((252-36)/96) = 3 windows.
        let long: Vec<_> = f.openings.iter().filter(|o| o.width == 36.0).collect();
        assert!(long.len() >= 3);
    }

    #[test]
    fn upper_floor_boxes_create_the_floor() {
        let mut up = bx(1, "Bedroom", "Bedroom", 0.0, 0.0, 144.0, 144.0);
        up.floor = 1;
        let mut p = Project::new("t");
        let r = build_house(&mut p, &[up], &BuildOptions::default());
        assert_eq!(p.floors.len(), 2);
        assert_eq!(p.floors[1].walls.len(), 4);
        assert_eq!(p.floors[1].name, "2nd Floor");
        assert_eq!(r.rooms_named, 1);
    }
}
