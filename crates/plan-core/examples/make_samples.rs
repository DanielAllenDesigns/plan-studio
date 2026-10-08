//! Builds the three sample projects shipped in `samples/` and writes them as
//! `.psplan` JSON (the format `Project::to_json` produces).
//!
//! Run from the workspace root:
//!
//! ```text
//! cargo run -p plan-core --example make_samples
//! cargo run -p plan-core --example make_samples -- --large-only
//! ```
//!
//! A fourth file, `large-house.psplan`, is the performance benchmark house
//! (about 600 walls on three floors; see `docs/performance.md`). It is only
//! checked for loading and round-tripping.
//!
//! Every file is read back with `Project::from_json`, re-serialized (must be
//! byte-identical) and checked: room counts, every room named, wall endpoints
//! on the 6" grid, exterior walls facing outward, openings clear of each other
//! and the wall ends, and placed symbols inside their rooms without overlaps.
//! A summary table is printed at the end.
//!
//! All coordinates below are in feet (x east, y north, front of the house at
//! y = 0); `pt` converts them to the model's inches.

#![allow(clippy::too_many_arguments)] // builder helpers take plain coordinates

use plan_core::geometry::point_in_polygon;
use plan_core::{
    auto_exterior_dimensions, detect_rooms, FloorKind, FoundationKind, Id, OpeningKind,
    OpeningStyle, PlacedSymbol, PlanDefaults, Point, Project, Room, WallKind,
};
use std::path::PathBuf;

type P = (f64, f64);
/// A wall named by its two centerline end points (feet), in either order.
type WallRef = (P, P);

fn pt(p: P) -> Point {
    Point::new(p.0 * 12.0, p.1 * 12.0)
}

/// Opening jambs stay at least this far from wall ends and each other.
const MARGIN: f64 = 2.0;

struct S {
    p: Project,
    d: PlanDefaults,
    ext_ty: &'static str,
    int_ty: &'static str,
}

impl S {
    fn new(name: &str, ext_ty: &'static str) -> S {
        let d = PlanDefaults::chief_x18_daniel();
        let p = Project::from_defaults(name, &d);
        S {
            p,
            d,
            ext_ty,
            int_ty: "Interior-4",
        }
    }

    // ---------------------------------------------------------------- walls

    fn wall(&mut self, fl: usize, a: P, b: P, kind: WallKind) -> Id {
        let (ty, height) = match kind {
            WallKind::Exterior => (self.ext_ty, self.d.exterior_wall.height),
            WallKind::Interior => (self.int_ty, self.d.interior_wall.height),
        };
        let thickness = self
            .p
            .wall_type_def(ty)
            .unwrap_or_else(|| panic!("wall type {ty} missing"))
            .thickness();
        let id = self.p.add_wall(fl, pt(a), pt(b), thickness, height, kind);
        self.p.floors[fl].wall_mut(id).unwrap().wall_type = Some(ty.to_string());
        id
    }

    /// A closed exterior loop. Points are listed clockwise so the default
    /// `exterior_side` (left of start-to-end) faces outward.
    fn ext_loop(&mut self, fl: usize, pts: &[P]) {
        for i in 0..pts.len() {
            self.wall(fl, pts[i], pts[(i + 1) % pts.len()], WallKind::Exterior);
        }
    }

    fn int(&mut self, fl: usize, a: P, b: P) {
        self.wall(fl, a, b, WallKind::Interior);
    }

    fn find_wall(&self, fl: usize, w: WallRef) -> Id {
        let (a, b) = (pt(w.0), pt(w.1));
        self.p.floors[fl]
            .walls
            .iter()
            .find(|x| {
                (x.start.dist(a) < 0.01 && x.end.dist(b) < 0.01)
                    || (x.start.dist(b) < 0.01 && x.end.dist(a) < 0.01)
            })
            .map(|x| x.id)
            .unwrap_or_else(|| panic!("no wall {w:?} on floor {fl}"))
    }

    // ------------------------------------------------------------- openings

    fn place(
        &mut self,
        fl: usize,
        wall: WallRef,
        at: P,
        kind: OpeningKind,
        width: f64,
        height: f64,
        sill: f64,
    ) -> Id {
        let wid = self.find_wall(fl, wall);
        let w = self.p.floors[fl].wall(wid).unwrap().clone();
        let off = w.start.dist(pt(at));
        assert!(
            w.point_at(off).dist(pt(at)) < 0.01,
            "{at:?} is not on wall {wall:?}"
        );
        let id = self
            .p
            .add_opening(fl, wid, off, kind)
            .unwrap_or_else(|| panic!("cannot place opening at {at:?} on {wall:?}"));
        let o = self.p.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.center_offset = off;
        o.width = width;
        o.height = height;
        o.sill_height = sill;
        id
    }

    /// A door. `toward` is a point on the side the leaf swings to, and
    /// `hinge_near` a point the hinge jamb should be closest to (usually the
    /// nearest perpendicular wall, so the open leaf rests against it).
    fn door(
        &mut self,
        fl: usize,
        wall: WallRef,
        at: P,
        toward: P,
        hinge_near: P,
        width: f64,
        style: OpeningStyle,
    ) -> Id {
        let height = if style == OpeningStyle::Garage {
            84.0
        } else {
            96.0
        };
        let id = self.place(fl, wall, at, OpeningKind::Door, width, height, 0.0);
        let wid = self.find_wall(fl, wall);
        let w = self.p.floors[fl].wall(wid).unwrap().clone();
        let flipped = pt(toward).sub(w.start).dot(w.normal()) < 0.0;
        let off = w.start.dist(pt(at));
        let start_jamb = w.point_at(off - width * 0.5);
        let end_jamb = w.point_at(off + width * 0.5);
        let h = pt(hinge_near);
        let hinge_at_end = h.dist(end_jamb) < h.dist(start_jamb);
        let o = self.p.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.swing_flipped = flipped;
        o.hinge_at_end = hinge_at_end;
        o.style = style;
        id
    }

    fn hinged(&mut self, fl: usize, wall: WallRef, at: P, toward: P, hinge_near: P, width: f64) {
        self.door(
            fl,
            wall,
            at,
            toward,
            hinge_near,
            width,
            OpeningStyle::Hinged,
        );
    }

    /// A cased opening between two rooms (no leaf).
    fn cased(&mut self, fl: usize, wall: WallRef, at: P, width: f64) {
        self.door(fl, wall, at, at, at, width, OpeningStyle::Doorway);
    }

    fn pocket(&mut self, fl: usize, wall: WallRef, at: P, width: f64) {
        self.door(fl, wall, at, at, at, width, OpeningStyle::Pocket);
    }

    fn bifold(&mut self, fl: usize, wall: WallRef, at: P, width: f64) {
        self.door(fl, wall, at, at, at, width, OpeningStyle::Bifold);
    }

    /// An exterior entry door, sized from the plan's exterior door defaults.
    fn entry(&mut self, fl: usize, wall: WallRef, at: P, toward: P, hinge_near: P) {
        let w = self.d.exterior_door.width;
        self.hinged(fl, wall, at, toward, hinge_near, w);
    }

    fn window(
        &mut self,
        fl: usize,
        wall: WallRef,
        at: P,
        width: f64,
        height: f64,
        sill: f64,
        egress: bool,
        tempered: bool,
    ) {
        let id = self.place(fl, wall, at, OpeningKind::Window, width, height, sill);
        let o = self.p.floors[fl]
            .openings
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap();
        o.egress = egress;
        o.tempered = tempered;
    }

    /// A bedroom egress window: 36" x 48" or larger, sill at 36".
    fn egress(&mut self, fl: usize, wall: WallRef, at: P, width: f64, height: f64) {
        self.window(fl, wall, at, width, height, 36.0, true, false);
    }

    // -------------------------------------------------------------- symbols

    /// Place a library symbol near a wall (`pos` in feet, within a few inches
    /// of the wall face) and snap it flush to that wall.
    fn symbol(&mut self, fl: usize, catalog_id: &str, label: &str, pos: P, size: (f64, f64, f64)) {
        let mut s = PlacedSymbol::new(catalog_id, pt(pos), size.0, size.1, size.2);
        s.label = label.to_string();
        let walls = self.p.floors[fl].walls.clone();
        assert!(
            s.auto_rotate_to_wall(&walls),
            "{label} at {pos:?} is not near a wall"
        );
        self.p.add_symbol(fl, s);
    }

    fn toilet(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.plumbing.toilet_elongated",
            "Toilet",
            pos,
            (20.0, 28.0, 30.0),
        );
    }
    fn vanity(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.plumbing.vanity_sink_30",
            "Vanity",
            pos,
            (30.0, 21.0, 34.0),
        );
    }
    fn pedestal(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.plumbing.pedestal_sink",
            "Pedestal Sink",
            pos,
            (20.0, 18.0, 34.0),
        );
    }
    fn tub(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.plumbing.bathtub_60x30",
            "Tub",
            pos,
            (60.0, 30.0, 20.0),
        );
    }
    fn shower(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.plumbing.shower_36x36",
            "Shower",
            pos,
            (36.0, 36.0, 80.0),
        );
    }
    fn range(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.appliances.range_30",
            "Range",
            pos,
            (30.0, 26.0, 36.0),
        );
    }
    fn fridge(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.appliances.refrigerator_36x30",
            "Refrigerator",
            pos,
            (36.0, 30.0, 70.0),
        );
    }
    fn washer(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.appliances.washer_27",
            "Washer",
            pos,
            (27.0, 28.0, 38.0),
        );
    }
    fn dryer(&mut self, fl: usize, pos: P) {
        self.symbol(
            fl,
            "core.appliances.dryer_27",
            "Dryer",
            pos,
            (27.0, 28.0, 38.0),
        );
    }
    fn kitchen_sink(&mut self, fl: usize, pos: P) {
        let mut s = PlacedSymbol::new(
            "core.plumbing.kitchen_sink_double",
            pt(pos),
            33.0,
            22.0,
            8.0,
        );
        s.label = "Kitchen Sink".into();
        s.elevation = 28.0;
        let walls = self.p.floors[fl].walls.clone();
        assert!(
            s.auto_rotate_to_wall(&walls),
            "kitchen sink not near a wall"
        );
        self.p.add_symbol(fl, s);
    }

    /// Stairs placeholder: a straight run, back-center at `back_center`
    /// (inches), width along +x and run along +y. The real stair object is
    /// owned by plan-stairs; this is only a footprint marker.
    fn stairs_placeholder(&mut self, fl: usize, back_center: Point, label: &str) {
        let mut s = PlacedSymbol::new(
            "core.stairs.placeholder_straight",
            back_center,
            42.0,
            132.0,
            109.125,
        );
        s.label = label.to_string();
        self.p.add_symbol(fl, s);
    }

    // ---------------------------------------------------------------- rooms

    fn rooms(&self, fl: usize) -> Vec<Room> {
        detect_rooms(&self.p.floors[fl].walls, 0.5)
    }

    /// Name every detected room: `(anchor inside the room, name, room type)`.
    fn name_rooms(&mut self, fl: usize, names: &[(P, &str, &str)]) {
        let rooms = self.rooms(fl);
        assert_eq!(
            rooms.len(),
            names.len(),
            "floor {fl}: {} rooms detected, {} names given",
            rooms.len(),
            names.len()
        );
        for (anchor, name, ty) in names {
            assert!(
                self.d.room_type(ty).is_some(),
                "room type {ty:?} is not in the plan defaults"
            );
            self.p.set_room_name(fl, pt(*anchor), *name, *ty, &rooms);
        }
        assert_eq!(self.p.floors[fl].room_names.len(), rooms.len());
        for r in &rooms {
            let n = self.p.floors[fl]
                .room_names
                .iter()
                .filter(|n| point_in_polygon(n.anchor, &r.polygon))
                .count();
            assert_eq!(n, 1, "floor {fl}: a room does not hold exactly one name");
        }
    }

    /// Automatic exterior dimensions on `fl`.
    fn auto_dims(&mut self, fl: usize) {
        let off = self.d.dimensions.auto_exterior_offset;
        let dims = auto_exterior_dimensions(&self.p.floors[fl].walls, off);
        assert!(!dims.is_empty(), "no automatic dimensions on floor {fl}");
        for dim in dims {
            self.p.add_dimension(fl, dim);
        }
    }
}

// ------------------------------------------------------------------ checks

struct Stats {
    floor: String,
    rooms: usize,
    interior_sf: f64,
    standard_sf: f64,
    doors: usize,
    windows: usize,
    symbols: usize,
}

fn aabb(pts: &[Point]) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x0, y0, x1, y1)
}

/// Structural checks on every floor; returns per-floor statistics.
fn check(p: &Project, expected_rooms: &[usize]) -> Vec<Stats> {
    assert_eq!(p.floors.len(), expected_rooms.len(), "floor count");
    let mut out = Vec::new();
    for (fi, f) in p.floors.iter().enumerate() {
        let rooms = detect_rooms(&f.walls, 0.5);
        assert_eq!(
            rooms.len(),
            expected_rooms[fi],
            "{}: expected {} rooms, found {}",
            f.name,
            expected_rooms[fi],
            rooms.len()
        );
        if f.kind == FloorKind::Normal {
            assert_eq!(f.room_names.len(), rooms.len(), "{}: unnamed rooms", f.name);
        }
        // 6" grid.
        for w in &f.walls {
            for v in [w.start.x, w.start.y, w.end.x, w.end.y] {
                assert!(
                    (v / 6.0 - (v / 6.0).round()).abs() < 1e-9,
                    "{}: wall endpoint {v} is off the 6\" grid",
                    f.name
                );
            }
        }
        // Exterior walls face outward: the point just outside is in no room.
        for w in f.walls.iter().filter(|w| w.kind == WallKind::Exterior) {
            let mid = w.point_at(w.length() * 0.5);
            let out_pt = mid + w.exterior_normal() * (w.thickness + 6.0);
            assert!(
                !rooms.iter().any(|r| point_in_polygon(out_pt, &r.polygon)),
                "{}: exterior wall {} faces inward",
                f.name,
                w.id
            );
        }
        // Openings sit inside their wall and clear of each other.
        for w in &f.walls {
            let mut on: Vec<_> = f.openings_on(w.id).collect();
            on.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
            for o in &on {
                assert!(
                    o.start_offset() >= MARGIN - 1e-6
                        && o.end_offset() <= w.length() - MARGIN + 1e-6,
                    "{}: opening {} runs past the wall ends",
                    f.name,
                    o.id
                );
            }
            for pair in on.windows(2) {
                assert!(
                    pair[0].end_offset() + MARGIN <= pair[1].start_offset() + 1e-6,
                    "{}: openings {} and {} overlap",
                    f.name,
                    pair[0].id,
                    pair[1].id
                );
            }
        }
        // Symbols: inside a room, not overlapping each other.
        let boxes: Vec<_> = f
            .symbols
            .iter()
            .map(|s| (s, aabb(&s.footprint())))
            .collect();
        for (s, _) in &boxes {
            if f.kind != FloorKind::Normal {
                continue;
            }
            let fp = s.footprint();
            let c = Point::new(
                fp.iter().map(|q| q.x).sum::<f64>() / 4.0,
                fp.iter().map(|q| q.y).sum::<f64>() / 4.0,
            );
            let inside = rooms.iter().any(|r| {
                let poly = if r.inner_polygon.is_empty() {
                    &r.polygon
                } else {
                    &r.inner_polygon
                };
                // Corners nudged 0.5" toward the centre so a symbol that sits
                // flush against a wall face still counts as inside.
                fp.iter().all(|q| {
                    let dir = c.sub(*q).normalized();
                    point_in_polygon(*q + dir * 0.5, poly)
                })
            });
            assert!(
                inside,
                "{}: symbol {} ({}) leaves its room",
                f.name, s.id, s.label
            );
        }
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                let (a, b) = (boxes[i].1, boxes[j].1);
                let overlap =
                    a.0 < b.2 - 0.01 && b.0 < a.2 - 0.01 && a.1 < b.3 - 0.01 && b.1 < a.3 - 0.01;
                assert!(
                    !overlap,
                    "{}: symbols {} and {} overlap",
                    f.name, boxes[i].0.label, boxes[j].0.label
                );
            }
        }
        out.push(Stats {
            floor: f.name.clone(),
            rooms: rooms.len(),
            interior_sf: rooms.iter().map(|r| r.interior_area_sq_ft()).sum(),
            standard_sf: rooms.iter().map(|r| r.standard_area_sq_ft()).sum(),
            doors: f
                .openings
                .iter()
                .filter(|o| o.kind == OpeningKind::Door)
                .count(),
            windows: f
                .openings
                .iter()
                .filter(|o| o.kind == OpeningKind::Window)
                .count(),
            symbols: f.symbols.len(),
        });
    }
    out
}

/// Write, read back, and prove the file round-trips.
fn save(
    dir: &std::path::Path,
    file: &str,
    p: &Project,
    expected_rooms: &[usize],
) -> (usize, Vec<Stats>) {
    let json = p.to_json().expect("serialize");
    let path = dir.join(file);
    let _ = std::fs::remove_file(&path); // replace rather than overwrite in place
    std::fs::write(&path, &json).expect("write sample");
    let text = std::fs::read_to_string(&path).expect("read sample");
    let back = Project::from_json(&text).expect("sample loads with Project::from_json");
    assert_eq!(
        back.to_json().expect("re-serialize"),
        json,
        "{file} does not round-trip"
    );
    let stats = check(&back, expected_rooms);
    (text.len(), stats)
}

// ------------------------------------------------------------------ samples

/// 3 bed / 2 bath one-story ranch: 48' x 32' house with a 22' x 22' attached
/// 2-car garage on the east end.
fn ranch() -> Project {
    let mut s = S::new("Ranch 3-Bed (sample)", "Stucco-6");
    let f = 0;

    // Exterior shell, clockwise from the SW corner.
    s.ext_loop(
        f,
        &[
            (0.0, 0.0),
            (0.0, 32.0),
            (48.0, 32.0),
            (48.0, 22.0),
            (70.0, 22.0),
            (70.0, 0.0),
            (48.0, 0.0),
        ],
    );
    // House/garage party wall.
    s.int(f, (48.0, 0.0), (48.0, 22.0));
    // Bedroom wing south wall (north face of the hall) and hall south wall.
    s.int(f, (0.0, 19.0), (48.0, 19.0));
    s.int(f, (0.0, 15.0), (33.0, 15.0));
    s.int(f, (33.0, 15.0), (33.0, 19.0));
    // Living | Dining | Kitchen.
    s.int(f, (18.0, 0.0), (18.0, 15.0));
    s.int(f, (28.0, 0.0), (28.0, 15.0));
    // Kitchen | Laundry and Master | Master Bath share one wall.
    s.int(f, (41.0, 0.0), (41.0, 32.0));
    // Bedroom wing partitions.
    s.int(f, (11.0, 19.0), (11.0, 32.0));
    s.int(f, (17.0, 19.0), (17.0, 32.0));
    s.int(f, (27.0, 19.0), (27.0, 32.0));

    let south: WallRef = ((48.0, 0.0), (0.0, 0.0));
    let west: WallRef = ((0.0, 0.0), (0.0, 32.0));
    let north: WallRef = ((0.0, 32.0), (48.0, 32.0));
    let bed_wall: WallRef = ((0.0, 19.0), (48.0, 19.0));
    let hall_south: WallRef = ((0.0, 15.0), (33.0, 15.0));
    let x41: WallRef = ((41.0, 0.0), (41.0, 32.0));
    let party: WallRef = ((48.0, 0.0), (48.0, 22.0));

    // Exterior doors.
    s.entry(f, south, (14.0, 0.0), (14.0, 3.0), (18.0, 0.0));
    s.door(
        f,
        ((70.0, 0.0), (48.0, 0.0)),
        (59.0, 0.0),
        (59.0, 5.0),
        (59.0, 0.0),
        192.0,
        OpeningStyle::Garage,
    );
    s.entry(
        f,
        ((70.0, 22.0), (70.0, 0.0)),
        (70.0, 11.0),
        (67.0, 11.0),
        (70.0, 22.0),
    );
    // Garage to Laundry.
    s.hinged(f, party, (48.0, 6.0), (45.0, 6.0), (48.0, 0.0), 36.0);
    // Bedroom wing doors off the hall (all swing into the room).
    s.hinged(f, bed_wall, (8.5, 19.0), (8.5, 22.0), (11.0, 19.0), 30.0);
    s.pocket(f, bed_wall, (14.0, 19.0), 28.0);
    s.hinged(f, bed_wall, (24.5, 19.0), (24.5, 22.0), (27.0, 19.0), 30.0);
    s.hinged(f, bed_wall, (30.0, 19.0), (30.0, 22.0), (27.0, 19.0), 30.0);
    // Master Bath and Laundry.
    s.hinged(f, x41, (41.0, 26.0), (44.0, 26.0), (41.0, 32.0), 30.0);
    s.hinged(f, x41, (41.0, 9.0), (44.0, 9.0), (41.0, 0.0), 30.0);
    // Cased openings through the living spaces.
    s.cased(f, hall_south, (9.0, 15.0), 72.0);
    s.cased(f, hall_south, (23.0, 15.0), 48.0);
    s.cased(f, ((18.0, 0.0), (18.0, 15.0)), (18.0, 8.0), 72.0);
    s.cased(f, ((28.0, 0.0), (28.0, 15.0)), (28.0, 8.0), 60.0);

    // Windows. South.
    s.window(f, south, (5.0, 0.0), 72.0, 60.0, 24.0, false, false);
    s.window(f, south, (23.0, 0.0), 48.0, 48.0, 30.0, false, false);
    s.window(f, south, (34.5, 0.0), 48.0, 36.0, 42.0, false, false);
    s.window(f, south, (44.5, 0.0), 24.0, 36.0, 48.0, false, false);
    // West.
    s.window(f, west, (0.0, 8.0), 48.0, 60.0, 24.0, false, false);
    s.egress(f, west, (0.0, 25.5), 36.0, 48.0);
    // North.
    s.window(f, north, (5.5, 32.0), 48.0, 48.0, 30.0, false, false);
    s.window(f, north, (14.0, 32.0), 24.0, 36.0, 60.0, false, true);
    s.egress(f, north, (22.0, 32.0), 36.0, 48.0);
    s.egress(f, north, (34.0, 32.0), 60.0, 48.0);
    s.window(f, north, (44.5, 32.0), 24.0, 36.0, 60.0, false, true);

    // Fixtures. Bath.
    s.tub(f, (14.0, 31.5));
    s.toilet(f, (11.5, 24.0));
    s.vanity(f, (16.5, 22.0));
    // Master Bath.
    s.shower(f, (46.0, 31.5));
    s.toilet(f, (47.5, 25.0));
    s.vanity(f, (43.25, 19.5));
    s.vanity(f, (45.9, 19.5));
    // Kitchen.
    s.range(f, (36.5, 18.5));
    s.fridge(f, (40.5, 17.0));
    // Laundry.
    s.washer(f, (47.5, 14.0));
    s.dryer(f, (47.5, 16.5));
    // Garage.
    s.symbol(
        f,
        "core.bathkitchen.water_heater_24",
        "Water Heater",
        (69.5, 19.0),
        (24.0, 24.0, 60.0),
    );

    s.name_rooms(
        f,
        &[
            ((8.0, 8.0), "Living", "Living"),
            ((23.0, 8.0), "Dining", "Dining"),
            ((34.0, 8.0), "Kitchen", "Kitchen"),
            ((44.5, 10.0), "Laundry", "Laundry"),
            ((16.0, 17.0), "Hall", "Hall"),
            ((5.5, 25.0), "Bedroom 3", "Bedroom #3"),
            ((14.0, 25.0), "Bath", "Bath"),
            ((22.0, 25.0), "Bedroom 2", "Bedroom #2"),
            ((34.0, 25.0), "Master Bedroom", "Master Bedroom"),
            ((44.5, 25.0), "Master Bath", "Master Bath"),
            ((59.0, 11.0), "2-Car Garage", "Garage"),
        ],
    );
    s.auto_dims(f);
    s.p
}

/// Two-story center-hall colonial: 40' x 32', stem-wall foundation, four
/// bedrooms up. Floors end up as Foundation, 1st Floor, 2nd Floor.
fn colonial() -> Project {
    let mut s = S::new("Two-Story Colonial (sample)", "Siding-6");
    let g = 0; // ground floor while building

    let corners = [(0.0, 0.0), (0.0, 32.0), (40.0, 32.0), (40.0, 0.0)];
    s.ext_loop(g, &corners);
    // Hall walls (full depth) and cross walls.
    s.int(g, (16.0, 0.0), (16.0, 32.0));
    s.int(g, (24.0, 0.0), (24.0, 32.0));
    s.int(g, (0.0, 16.0), (16.0, 16.0));
    s.int(g, (24.0, 16.0), (40.0, 16.0));
    s.int(g, (16.0, 26.0), (24.0, 26.0));

    let south: WallRef = ((40.0, 0.0), (0.0, 0.0));
    let west: WallRef = ((0.0, 0.0), (0.0, 32.0));
    let north: WallRef = ((0.0, 32.0), (40.0, 32.0));
    let east: WallRef = ((40.0, 32.0), (40.0, 0.0));
    let x16: WallRef = ((16.0, 0.0), (16.0, 32.0));
    let x24: WallRef = ((24.0, 0.0), (24.0, 32.0));

    // 1st floor openings.
    s.entry(g, south, (20.0, 0.0), (20.0, 3.0), (16.0, 0.0));
    s.entry(g, north, (36.0, 32.0), (36.0, 29.0), (40.0, 32.0));
    s.hinged(
        g,
        ((16.0, 26.0), (24.0, 26.0)),
        (20.0, 26.0),
        (20.0, 29.0),
        (16.0, 26.0),
        28.0,
    );
    s.cased(g, x16, (16.0, 4.0), 48.0);
    s.cased(g, x16, (16.0, 23.0), 48.0);
    s.cased(g, x24, (24.0, 8.0), 72.0);
    s.cased(g, x24, (24.0, 22.0), 60.0);
    s.cased(g, ((0.0, 16.0), (16.0, 16.0)), (8.0, 16.0), 60.0);
    s.cased(g, ((24.0, 16.0), (40.0, 16.0)), (32.0, 16.0), 60.0);
    // 1st floor windows.
    for x in [29.0, 35.0, 11.0, 5.0] {
        s.window(g, south, (x, 0.0), 36.0, 60.0, 24.0, false, false);
    }
    s.window(g, west, (0.0, 8.0), 36.0, 60.0, 24.0, false, false);
    s.window(g, west, (0.0, 24.0), 36.0, 60.0, 24.0, false, false);
    s.window(g, north, (5.0, 32.0), 36.0, 60.0, 24.0, false, false);
    s.window(g, north, (11.0, 32.0), 36.0, 60.0, 24.0, false, false);
    s.window(g, north, (29.0, 32.0), 36.0, 48.0, 36.0, false, false);
    s.window(g, north, (20.0, 32.0), 18.0, 24.0, 60.0, false, true);
    s.window(g, east, (40.0, 8.0), 36.0, 60.0, 24.0, false, false);
    s.window(g, east, (40.0, 20.0), 36.0, 48.0, 36.0, false, false);
    // 1st floor fixtures.
    s.toilet(g, (22.0, 31.5));
    s.pedestal(g, (16.5, 29.0));
    s.range(g, (32.0, 31.5));
    s.fridge(g, (39.5, 27.0));
    s.kitchen_sink(g, (26.0, 31.5));
    // Stairs up the west side of the hall.
    s.stairs_placeholder(
        g,
        Point::new(16.0 * 12.0 + 2.25 + 21.0, 8.0 * 12.0),
        "Stairs Up (placeholder)",
    );
    s.name_rooms(
        g,
        &[
            ((8.0, 8.0), "Living", "Living"),
            ((8.0, 24.0), "Family Room", "Family Room"),
            ((32.0, 8.0), "Dining Room", "Dining Room"),
            ((32.0, 24.0), "Kitchen", "Kitchen"),
            ((20.0, 13.0), "Center Hall", "Hall"),
            ((20.0, 29.0), "Powder Room", "Powder Room"),
        ],
    );
    s.auto_dims(g);

    // 2nd floor: copy the exterior shell, then lay out the bedrooms.
    let u = s.p.build_new_floor(true);
    assert_eq!(u, 1);
    s.p.floors[u].openings.clear(); // re-added below for the new room layout
    s.int(u, (16.0, 0.0), (16.0, 32.0));
    s.int(u, (24.0, 0.0), (24.0, 32.0));
    // West wing: Master Bedroom / Master Bath + Closet / Bedroom 4.
    s.int(u, (0.0, 14.0), (16.0, 14.0));
    s.int(u, (0.0, 22.0), (16.0, 22.0));
    s.int(u, (9.0, 14.0), (9.0, 22.0));
    // East wing: Bedroom 2 / Laundry + Jack-and-Jill Bath / Bedroom 3.
    s.int(u, (24.0, 12.0), (40.0, 12.0));
    s.int(u, (24.0, 20.0), (40.0, 20.0));
    s.int(u, (32.0, 12.0), (32.0, 20.0));

    // Doors (all swing into the room they serve).
    s.hinged(u, x16, (16.0, 4.0), (13.0, 4.0), (16.0, 0.0), 30.0);
    s.hinged(u, x16, (16.0, 27.0), (13.0, 27.0), (16.0, 32.0), 30.0);
    s.hinged(
        u,
        ((0.0, 14.0), (16.0, 14.0)),
        (4.5, 14.0),
        (4.5, 17.0),
        (0.0, 14.0),
        28.0,
    );
    s.bifold(u, ((0.0, 14.0), (16.0, 14.0)), (12.5, 14.0), 60.0);
    s.hinged(u, x24, (24.0, 5.0), (27.0, 5.0), (24.0, 0.0), 30.0);
    s.hinged(u, x24, (24.0, 16.0), (27.0, 16.0), (24.0, 12.0), 30.0);
    s.hinged(u, x24, (24.0, 26.0), (27.0, 26.0), (24.0, 32.0), 30.0);
    s.hinged(
        u,
        ((24.0, 12.0), (40.0, 12.0)),
        (36.0, 12.0),
        (36.0, 15.0),
        (32.0, 12.0),
        28.0,
    );
    s.hinged(
        u,
        ((24.0, 20.0), (40.0, 20.0)),
        (36.0, 20.0),
        (36.0, 17.0),
        (32.0, 20.0),
        28.0,
    );
    // Windows (egress in every bedroom).
    s.egress(u, south, (5.0, 0.0), 36.0, 60.0);
    s.egress(u, south, (11.0, 0.0), 36.0, 60.0);
    s.window(u, south, (20.0, 0.0), 36.0, 60.0, 24.0, false, false);
    s.egress(u, south, (29.0, 0.0), 36.0, 60.0);
    s.egress(u, south, (35.0, 0.0), 36.0, 60.0);
    s.egress(u, west, (0.0, 8.0), 36.0, 60.0);
    s.window(u, west, (0.0, 18.0), 24.0, 36.0, 60.0, false, true);
    s.egress(u, west, (0.0, 27.0), 36.0, 60.0);
    s.egress(u, north, (5.0, 32.0), 36.0, 60.0);
    s.egress(u, north, (11.0, 32.0), 36.0, 60.0);
    s.window(u, north, (20.0, 32.0), 36.0, 48.0, 36.0, false, false);
    s.egress(u, north, (29.0, 32.0), 36.0, 60.0);
    s.egress(u, north, (35.0, 32.0), 36.0, 60.0);
    s.egress(u, east, (40.0, 6.0), 36.0, 60.0);
    s.window(u, east, (40.0, 16.0), 24.0, 36.0, 60.0, false, true);
    s.egress(u, east, (40.0, 26.0), 36.0, 60.0);
    // Fixtures. Master Bath.
    s.shower(u, (0.5, 19.0));
    s.toilet(u, (6.0, 21.5));
    s.vanity(u, (8.5, 19.0));
    // Laundry.
    s.washer(u, (31.5, 14.5));
    s.dryer(u, (31.5, 17.25));
    // Jack-and-Jill Bath.
    s.tub(u, (39.5, 15.5));
    s.toilet(u, (32.5, 14.0));
    s.vanity(u, (32.5, 18.0));
    s.stairs_placeholder(
        u,
        Point::new(16.0 * 12.0 + 2.25 + 21.0, 8.0 * 12.0),
        "Stairs (placeholder)",
    );
    s.name_rooms(
        u,
        &[
            ((8.0, 7.0), "Master Bedroom", "Master Bedroom"),
            ((4.5, 18.0), "Master Bath", "Master Bath"),
            ((12.5, 18.0), "Master Closet", "Closet"),
            ((8.0, 27.0), "Bedroom 4", "Bedroom #4"),
            ((20.0, 16.0), "Upstairs Hall", "Hall"),
            ((32.0, 6.0), "Bedroom 2", "Bedroom #2"),
            ((28.0, 16.0), "Laundry", "Laundry"),
            ((36.0, 16.0), "Jack-and-Jill Bath", "Bath"),
            ((32.0, 26.0), "Bedroom 3", "Bedroom #3"),
        ],
    );
    s.auto_dims(u);

    // Stem-wall foundation under the exterior walls (inserts the floor at 0).
    let h = s.d.foundation_wall.height;
    let fnd = s.p.build_foundation(FoundationKind::StemWall { height: h });
    assert_eq!(fnd, 0);
    s.name_rooms(0, &[((20.0, 16.0), "Crawl Space", "Crawl Space")]);
    s.p
}

/// 400 sq ft studio ADU: kitchenette along the west wall, bath in the NE
/// corner, closet in the NW corner.
fn adu() -> Project {
    let mut s = S::new("Studio ADU (sample)", "Siding-6");
    let f = 0;
    let (w, h) = (19.5, 19.5);
    s.ext_loop(f, &[(0.0, 0.0), (0.0, h), (w, h), (w, 0.0)]);
    // Bath, NE corner.
    s.int(f, (11.5, 11.5), (11.5, h));
    s.int(f, (11.5, 11.5), (w, 11.5));
    // Closet, NW corner.
    s.int(f, (4.0, 14.0), (4.0, h));
    s.int(f, (0.0, 14.0), (4.0, 14.0));

    let south: WallRef = ((w, 0.0), (0.0, 0.0));
    let west: WallRef = ((0.0, 0.0), (0.0, h));
    let north: WallRef = ((0.0, h), (w, h));
    let east: WallRef = ((w, h), (w, 0.0));

    s.entry(f, south, (14.0, 0.0), (14.0, 3.0), (18.0, 0.0));
    s.hinged(
        f,
        ((11.5, 11.5), (11.5, h)),
        (11.5, 15.0),
        (14.0, 15.0),
        (11.5, h),
        28.0,
    );
    s.bifold(f, ((0.0, 14.0), (4.0, 14.0)), (2.0, 14.0), 36.0);
    s.window(f, south, (6.5, 0.0), 36.0, 48.0, 30.0, false, false);
    s.window(f, west, (0.0, 10.5), 36.0, 36.0, 42.0, false, false);
    s.egress(f, east, (w, 5.5), 36.0, 48.0);
    s.window(f, north, (8.0, h), 36.0, 48.0, 30.0, false, false);
    s.window(f, north, (15.5, h), 24.0, 36.0, 60.0, false, true);

    // Kitchenette along the west wall (south to north).
    s.fridge(f, (0.5, 3.7));
    s.range(f, (0.5, 7.25));
    s.kitchen_sink(f, (0.5, 10.5));
    // Bath.
    s.shower(f, (17.68, 12.0));
    s.toilet(f, (w - 0.5, 16.0));
    s.vanity(f, (13.5, h - 0.5));

    s.name_rooms(
        f,
        &[
            ((8.0, 6.0), "Studio", "Great Room"),
            ((15.5, 15.5), "Bath", "Bath"),
            ((2.0, 17.0), "Closet", "Closet"),
        ],
    );
    s.auto_dims(f);
    s.p
}

// ------------------------------------------------------------ large house

/// Grid of the large benchmark house: 14 x 7 cells of 10', three floors.
const LARGE_COLS: usize = 14;
const LARGE_ROWS: usize = 7;
const LARGE_CELL: f64 = 10.0;

/// A deterministic estate-sized plan for performance work
/// (`docs/performance.md`): about 600 walls, 150 openings, 60 cabinets, 20
/// roof planes, 40 dimensions, 200 CAD items, 10 schedules, material regions
/// and hatches, and a terrain with 50 elevation lines, over three floors.
/// There is no randomness: every value comes from loop indices.
fn large_house() -> Project {
    use plan_core::cad::{CadAttrs, FillAttr};
    use plan_core::details::{DetailsLayer, MaterialRegion, WallHatch};
    use plan_core::schedules::{ensure_layer, FloorScope, Schedule, ScheduleKind, ScheduleLayer};
    use plan_core::{CadItem, Dimension, DimensionKind};
    use serde_json::json;

    let mut s = S::new("Large House (benchmark sample)", "Siding-6");
    s.p.build_new_floor(false);
    s.p.build_new_floor(false);
    assert_eq!(s.p.floors.len(), 3);
    let (cols, rows) = (LARGE_COLS, LARGE_ROWS);
    let c = LARGE_CELL;

    // ---- walls and openings: every grid edge is one wall of 10'.
    for fl in 0..3 {
        let mut interior_idx = 0usize;
        let mut ext_idx = 0usize;
        // Horizontal edges, west to east. The north edge runs west to east,
        // the south edge east to west (clockwise, so left of travel faces out).
        for r in 0..=rows {
            for k in 0..cols {
                let (x0, x1, y) = (k as f64 * c, (k + 1) as f64 * c, r as f64 * c);
                let exterior = r == 0 || r == rows;
                if exterior {
                    let (a, b) = if r == rows {
                        ((x0, y), (x1, y))
                    } else {
                        ((x1, y), (x0, y))
                    };
                    let id = s.wall(fl, a, b, WallKind::Exterior);
                    ext_idx += 1;
                    let kind = if r == 0 && k == 3 && fl == 0 {
                        OpeningKind::Door
                    } else {
                        OpeningKind::Window
                    };
                    if !ext_idx.is_multiple_of(3) || kind == OpeningKind::Door {
                        s.p.add_opening(fl, id, 60.0, kind);
                    }
                } else if (r * 7 + k * 3) % 9 != 0 {
                    let id = s.wall(fl, (x0, y), (x1, y), WallKind::Interior);
                    interior_idx += 1;
                    if interior_idx % 7 == 1 {
                        s.p.add_opening(fl, id, 60.0, OpeningKind::Door);
                    }
                }
            }
        }
        // Vertical edges: the west edge runs south to north, the east edge
        // north to south.
        for k in 0..=cols {
            for r in 0..rows {
                let (y0, y1, x) = (r as f64 * c, (r + 1) as f64 * c, k as f64 * c);
                let exterior = k == 0 || k == cols;
                if exterior {
                    let (a, b) = if k == 0 {
                        ((x, y0), (x, y1))
                    } else {
                        ((x, y1), (x, y0))
                    };
                    let id = s.wall(fl, a, b, WallKind::Exterior);
                    ext_idx += 1;
                    if !ext_idx.is_multiple_of(3) {
                        s.p.add_opening(fl, id, 60.0, OpeningKind::Window);
                    }
                } else if (r * 5 + k * 2) % 9 != 0 {
                    let id = s.wall(fl, (x, y0), (x, y1), WallKind::Interior);
                    interior_idx += 1;
                    if interior_idx % 7 == 1 {
                        s.p.add_opening(fl, id, 60.0, OpeningKind::Door);
                    }
                }
            }
        }
    }

    // ---- room names: every detected room, so room schedules have rows.
    for fl in 0..3 {
        let rooms = s.rooms(fl);
        for (i, r) in rooms.iter().enumerate() {
            let (name, ty) = if i % 5 == 0 {
                (format!("Bedroom {}", i + 1), "Bedroom")
            } else {
                (format!("Room {}", i + 1), "Great Room")
            };
            assert!(s.d.room_type(ty).is_some(), "room type {ty}");
            s.p.set_room_name(fl, r.centroid, name, ty, &rooms);
        }
    }

    // ---- dimensions: 40 manual strings on the ground floor.
    let w_in = cols as f64 * c * 12.0;
    let h_in = rows as f64 * c * 12.0;
    for k in 0..cols {
        let x = k as f64 * c * 12.0;
        let d = |y: f64, off: f64| {
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::new(x, y),
                Point::new(x + c * 12.0, y),
                off,
            )
        };
        s.p.add_dimension(0, d(0.0, -48.0));
        s.p.add_dimension(0, d(h_in, 48.0));
    }
    for k in 0..rows {
        let y = k as f64 * c * 12.0;
        let west = Dimension::new(
            0,
            DimensionKind::Manual,
            Point::new(0.0, y),
            Point::new(0.0, y + c * 12.0),
            48.0,
        );
        s.p.add_dimension(0, west);
    }
    for k in 0..5 {
        let y = k as f64 * c * 12.0;
        let east = Dimension::new(
            0,
            DimensionKind::Manual,
            Point::new(w_in, y),
            Point::new(w_in, y + c * 12.0),
            -48.0,
        );
        s.p.add_dimension(0, east);
    }

    // ---- CAD: 120 items on floor 0, 50 on floor 1, 30 on floor 2.
    for (fl, n) in [(0usize, 120usize), (1, 50), (2, 30)] {
        for i in 0..n {
            let x = (i % 20) as f64 * 84.0 + 12.0;
            let y = (i / 20) as f64 * 96.0 + 12.0;
            let item = match i % 5 {
                0 => CadItem::Line {
                    a: Point::new(x, y),
                    b: Point::new(x + 60.0, y + 30.0),
                },
                1 => CadItem::Circle {
                    center: Point::new(x + 20.0, y + 20.0),
                    radius: 14.0,
                },
                2 => CadItem::Arc {
                    center: Point::new(x + 20.0, y + 20.0),
                    radius: 22.0,
                    start_angle: 0.0,
                    end_angle: 2.2,
                },
                3 => CadItem::Polyline {
                    points: vec![
                        Point::new(x, y),
                        Point::new(x + 40.0, y),
                        Point::new(x + 40.0, y + 30.0),
                        Point::new(x, y + 30.0),
                    ],
                    closed: true,
                },
                _ => CadItem::Text {
                    pos: Point::new(x, y),
                    text: format!("Note {fl}-{i}"),
                    height: 4.0,
                    angle: 0.0,
                },
            };
            let id = s.p.add_cad(fl, "CAD, Default", item);
            // Every third closed shape carries a fill or hatch attribute.
            if i % 5 == 3 && i % 3 == 0 {
                let mut a = CadAttrs::new(id);
                a.fill = Some(FillAttr {
                    pattern: if i % 2 == 0 {
                        String::new()
                    } else {
                        "Lines".into()
                    },
                    ..FillAttr::default()
                });
                s.p.floors[fl].cad_attrs.push(a);
            }
        }
    }

    // ---- material regions on floor 0, wall hatches on floors 0 and 1.
    for fl in 0..2 {
        let mut layer = DetailsLayer::default();
        for k in 0..8usize {
            let x0 = (k % 4) as f64 * 360.0 + 12.0;
            let y0 = (k / 4) as f64 * 360.0 + 12.0;
            let id = s.p.alloc_id();
            layer.regions.push(MaterialRegion {
                id,
                outline: vec![
                    Point::new(x0, y0),
                    Point::new(x0 + 336.0, y0),
                    Point::new(x0 + 336.0, y0 + 336.0),
                    Point::new(x0, y0 + 336.0),
                ],
                material: [
                    "Ceramic Tile 12x12",
                    "Porcelain Tile 24x24",
                    "Walnut Flooring",
                ][k % 3]
                    .to_string(),
                ..MaterialRegion::default()
            });
        }
        let ext: Vec<Id> = s.p.floors[fl]
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior)
            .map(|w| w.id)
            .step_by(2)
            .take(24)
            .collect();
        for wid in ext {
            let id = s.p.alloc_id();
            layer.hatches.push(WallHatch {
                id,
                wall_id: wid,
                ..WallHatch::default()
            });
        }
        layer.store(&mut s.p.floors[fl]);
    }

    // ---- cabinets: 30 + 20 + 10 base cabinets in runs of 10.
    for (fl, n) in [(0usize, 30usize), (1, 20), (2, 10)] {
        for i in 0..n {
            let id = s.p.alloc_id();
            let x = 12.0 + (i % 10) as f64 * 26.0 + (i / 10) as f64 * 300.0;
            let y = 12.0 + (i / 10) as f64 * 150.0;
            s.p.floors[fl].cabinets.push(cabinet_json(id, x, y, 24.0));
        }
    }

    // ---- roof: 20 planes (two slopes, ten bays) on the top floor.
    {
        let top = &s.p.floors[2];
        let e = top.elevation + top.ceiling_height;
        let half = h_in * 0.5;
        let rise = half * 6.0 / 12.0;
        let bay = w_in / 10.0;
        let mut items = Vec::new();
        for k in 0..10usize {
            let (x0, x1) = (k as f64 * bay, (k + 1) as f64 * bay);
            for north in [false, true] {
                let id = s.p.alloc_id();
                let (ye, ym) = if north { (h_in, half) } else { (0.0, half) };
                let (a, b) = if north { (x1, x0) } else { (x0, x1) };
                items.push(json!({
                    "kind": "plane",
                    "id": id,
                    "polygon3d": [
                        [a, e, -ye], [b, e, -ye], [b, e + rise, -ym], [a, e + rise, -ym]
                    ],
                    "pitch": 6.0,
                    "baseline": [{"x": a, "y": ye}, {"x": b, "y": ye}],
                    "auto": false,
                    "holes": [],
                    "source": null,
                    "overhang": 0.0,
                    "label": "",
                    "material": "Asphalt Shingles",
                    "layer": "Roof Planes",
                    "ridge_caps": true,
                    "gutters": false,
                }));
            }
        }
        s.p.floors[2].roofs = items;
    }

    // ---- schedules: 10 tables outside the house.
    {
        let plan: [(usize, &[ScheduleKind]); 3] = [
            (
                0,
                &[
                    ScheduleKind::Door,
                    ScheduleKind::Window,
                    ScheduleKind::Room,
                    ScheduleKind::Wall,
                    ScheduleKind::Cabinet,
                    ScheduleKind::General,
                ],
            ),
            (1, &[ScheduleKind::Door, ScheduleKind::Window]),
            (2, &[ScheduleKind::Room, ScheduleKind::Door]),
        ];
        let mut n = 0usize;
        for (fl, kinds) in plan {
            let mut layer = ScheduleLayer::default();
            for kind in kinds {
                let at = Point::new(
                    -1400.0 + (n % 5) as f64 * 300.0,
                    -120.0 - (n / 5) as f64 * 1800.0,
                );
                let mut sch = Schedule::new(*kind, at);
                sch.id = s.p.alloc_id();
                // The ground-floor door and window schedules list every floor.
                if fl == 0 && matches!(kind, ScheduleKind::Door | ScheduleKind::Window) {
                    sch.floor_scope = FloorScope::All;
                }
                layer.add(sch);
                n += 1;
            }
            layer.store(&mut s.p.floors[fl]);
        }
        assert_eq!(n, 10);
        ensure_layer(&mut s.p.layers);
    }

    // ---- terrain: 50 elevation lines across a 240' x 160' lot.
    {
        let lines: Vec<_> = (0..50usize)
            .map(|i| {
                let y = -1100.0 + i as f64 * 70.0;
                let z = (-60.0 + (i as f64 * 0.7).sin() * 40.0 + i as f64 * 2.0).round();
                let pts: Vec<_> = (0..8usize)
                    .map(|j| {
                        let x = -900.0 + j as f64 * 380.0;
                        json!({"x": x, "y": (y + (j as f64 * 0.9).sin() * 30.0).round()})
                    })
                    .collect();
                json!({"points": pts, "z": z})
            })
            .collect();
        s.p.terrain = Some(json!({
            "terrain": {
                "perimeter": [
                    {"x": -1000.0, "y": -1200.0}, {"x": 2700.0, "y": -1200.0},
                    {"x": 2700.0, "y": 2300.0}, {"x": -1000.0, "y": 2300.0}
                ],
                "elevation_lines": lines,
                "building_pad_elevation": 0.0,
                "grid_spacing": 120.0,
            },
            "contour_interval": 24.0,
            "built": true,
            "layer": "Terrain",
        }));
    }
    s.p
}

/// A base cabinet as the `plan-cabinets` JSON (kept in the typed slot
/// `Floor.cabinets`; `plan-app` has a test that parses it).
fn cabinet_json(id: Id, x: f64, y: f64, width: f64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "kind": "Base",
        "position": {"x": x, "y": y}, "angle": 0.0,
        "width": width, "depth": 24.0, "height": 36.0, "elevation": 0.0,
        "countertop": {"thickness": 1.5, "overhang_front": 1.0, "overhang_sides": 0.0, "overhang_back": 0.0},
        "backsplash": null,
        "toe_kick": {"height": 4.0, "depth": 3.0},
        "face": {
            "items": [
                {"Separation": {"height": 1.5}}, {"Drawer": {"height": 6.0}},
                {"Separation": {"height": 1.5}}, {"DoorAuto": {"height": 0.0}},
                {"Separation": {"height": 1.5}}
            ],
            "frame_width": 1.5
        },
        "door_style": {
            "name": "Lincoln Door", "thickness": 0.75, "glass": false, "handle": "Knob",
            "handle_from_top": 1.375, "handle_from_edge": 1.375, "profile": "Slab",
            "frame_width": 2.25, "handle_centered": false, "hinge": "Hidden", "hinge_from_edge": 3.0
        },
        "drawer_style": {
            "name": "Lincoln Flat Panel Drawer", "thickness": 0.75, "handle": "Knob",
            "profile": "Slab", "handle_centered": true
        },
        "overlay": {"Full": {"reveal": 0.0625}},
        "framed": true, "label": "",
        "corner": null, "blind": null, "custom": null, "cutouts": [], "appliance": null,
        "moldings": [],
        "materials": {
            "carcass": "Default", "door": "Default", "drawer": "Default", "countertop": "Default",
            "backsplash": "Default", "toe_kick": "Default", "molding": "Default"
        },
        "indicators": false
    })
}

/// Write the large sample; it only has to load and round-trip (the strict
/// `check` above is for the hand-laid houses).
fn save_plain(dir: &std::path::Path, file: &str, p: &Project) -> usize {
    let json = p.to_json().expect("serialize");
    let path = dir.join(file);
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, &json).expect("write sample");
    let text = std::fs::read_to_string(&path).expect("read sample");
    let back = Project::from_json(&text).expect("sample loads with Project::from_json");
    let again = back.to_json().expect("re-serialize");
    if again != json {
        let at = again
            .bytes()
            .zip(json.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let ctx = |t: &str| t[at.saturating_sub(120)..(at + 120).min(t.len())].to_string();
        panic!(
            "{file} does not round-trip at byte {at}:\n{}\n---\n{}",
            ctx(&json),
            ctx(&again)
        );
    }
    text.len()
}

/// Builds `large-house.psplan` and prints its object counts.
fn write_large(dir: &std::path::Path) {
    let big = large_house();
    let bytes = save_plain(dir, "large-house.psplan", &big);
    let count = |f: &dyn Fn(&plan_core::Floor) -> usize| big.floors.iter().map(f).sum::<usize>();
    println!(
        "\nlarge-house.psplan: {} bytes, {} walls, {} openings, {} cabinets, {} dimensions, {} CAD items",
        bytes,
        count(&|f| f.walls.len()),
        count(&|f| f.openings.len()),
        count(&|f| f.cabinets.len()),
        count(&|f| f.dimensions.len()),
        count(&|f| f.cad.len()),
    );
}

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    std::fs::create_dir_all(&dir).expect("create samples/");
    let dir = dir.canonicalize().unwrap_or(dir);
    // `--large-only` regenerates just the benchmark house.
    if std::env::args().any(|a| a == "--large-only") {
        write_large(&dir);
        return;
    }

    // (file, project, expected rooms per floor bottom to top)
    let jobs: Vec<(&str, Project, Vec<usize>)> = vec![
        ("ranch-3bed.psplan", ranch(), vec![11]),
        ("two-story-colonial.psplan", colonial(), vec![1, 6, 9]),
        ("studio-adu.psplan", adu(), vec![3]),
    ];

    println!(
        "{:<26} {:<12} {:>5} {:>9} {:>9} {:>5} {:>7} {:>7} {:>8}",
        "sample", "floor", "rooms", "interior", "outside", "doors", "windows", "symbols", "bytes"
    );
    for (file, p, expect) in &jobs {
        let (bytes, stats) = save(&dir, file, p, expect);
        for (i, st) in stats.iter().enumerate() {
            println!(
                "{:<26} {:<12} {:>5} {:>7.0}sf {:>7.0}sf {:>5} {:>7} {:>7} {:>8}",
                if i == 0 { *file } else { "" },
                st.floor,
                st.rooms,
                st.interior_sf,
                st.standard_sf,
                st.doors,
                st.windows,
                st.symbols,
                if i == 0 {
                    bytes.to_string()
                } else {
                    String::new()
                },
            );
        }
    }

    // Per-room listing for the single-story samples.
    for (file, p, _) in &jobs {
        println!("\n{file}");
        for f in &p.floors {
            let rooms = detect_rooms(&f.walls, 0.5);
            for r in &rooms {
                let name = f
                    .room_names
                    .iter()
                    .find(|n| point_in_polygon(n.anchor, &r.polygon))
                    .map_or("?", |n| n.name.as_str());
                println!(
                    "  {:<12} {:<22} {:>6.0} sf interior",
                    f.name,
                    name,
                    r.interior_area_sq_ft()
                );
            }
        }
    }
    write_large(&dir);
    println!("\nwrote {} files to {}", jobs.len() + 1, dir.display());
}
