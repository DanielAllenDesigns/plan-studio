//! The take-off engine behind the Materials List: the quantities of a plan
//! with the objects that make them, and the filters that pick the objects
//! (all floors, a floor, an area, a room, a selection).
//!
//! Every row remembers its [`Source`]s, the objects (and the share of the
//! quantity each gave) that add up to it. That is what lets a row be expanded
//! into its objects, found in the plan, narrowed to a polyline or a selection,
//! priced per object and cut by floor and category.

use super::{
    framing_members, layer_line, layered_type, size_key, size_text, Component, MasterList,
    MaterialLine, BUNDLES_PER_SQUARE, SHEET_SQ_FT, STUD_SPACING,
};
use crate::schedule::room_name;
use crate::schedule_kinds::entries;
use plan_cabinets::{auto_label, Cabinet};
use plan_core::foundation::FoundationLayer;
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::materials_data::{FramingStyle, IncludedObjects};
use plan_core::props::PropKey;
use plan_core::schedules::ScheduleKind;
use plan_core::units::fmt_ft_in;
use plan_core::{detect_rooms, Floor, OpeningKind, Project, Room, WallKind};
use plan_electrical::ElectricalLayer;
use plan_framing::{FramingMember, MaterialList, Member};
use std::collections::{BTreeMap, BTreeSet};

mod platform_lines;

// ------------------------------------------------------------------ sources --

/// One object's share of a row: `qty` in the row's pre-rounding unit (square
/// feet for sheets, squares for roofing...).
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub floor: usize,
    /// The object key (`wall:12`, `door:5`, `room:0:120,84` ...).
    pub key: String,
    pub qty: f64,
}

/// How a row's quantity follows from the sum of its sources' shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// The sum as it is.
    Sum,
    /// Rounded up to a whole number.
    Ceil,
    /// Rounded up to hundredths (cubic yards, squares).
    Hundredths,
    /// 4x8 sheets: the sum is square feet, rounded up to whole sheets.
    Sheets,
}

impl Rule {
    /// The row quantity for a summed share.
    pub fn apply(self, sum: f64) -> f64 {
        match self {
            Rule::Sum => sum,
            Rule::Ceil => (sum - 1e-9).ceil().max(0.0),
            Rule::Hundredths => ((sum * 100.0) - 1e-7).ceil().max(0.0) / 100.0,
            Rule::Sheets => (sum / SHEET_SQ_FT - 1e-9).ceil().max(0.0),
        }
    }
}

/// A row of the take-off with the objects behind it.
#[derive(Debug, Clone)]
pub struct Raw {
    pub line: MaterialLine,
    pub sources: Vec<Source>,
    pub rule: Rule,
}

impl Raw {
    /// Sets the quantity from the sources again (after some were dropped or
    /// changed).
    pub fn recompute(&mut self) {
        let sum: f64 = self.sources.iter().map(|s| s.qty).sum();
        self.line.net = self.rule.apply(sum);
        self.line.quantity = self.line.net;
    }
}

#[derive(Default)]
struct Acc(BTreeMap<(usize, String), f64>);

impl Acc {
    fn add(&mut self, floor: usize, key: &str, v: f64) {
        *self.0.entry((floor, key.to_string())).or_default() += v;
    }

    fn total(&self) -> f64 {
        self.0.values().sum()
    }

    fn sources(&self) -> Vec<Source> {
        self.0
            .iter()
            .map(|((floor, key), qty)| Source {
                floor: *floor,
                key: key.clone(),
                qty: *qty,
            })
            .collect()
    }
}

// ------------------------------------------------------------------ filters --

/// Where an object sits, plan inches.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
    pub center: Point,
}

impl Bounds {
    pub fn of_points(pts: &[Point]) -> Self {
        let mut lo = Point::new(f64::MAX, f64::MAX);
        let mut hi = Point::new(f64::MIN, f64::MIN);
        for p in pts {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        if pts.is_empty() {
            return Self::at(Point::ZERO);
        }
        Self {
            min: lo,
            max: hi,
            center: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
        }
    }

    pub fn at(p: Point) -> Self {
        Self {
            min: p,
            max: p,
            center: p,
        }
    }

    fn corners(&self) -> [Point; 4] {
        [
            self.min,
            Point::new(self.max.x, self.min.y),
            self.max,
            Point::new(self.min.x, self.max.y),
        ]
    }
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

fn inside_area(p: Point, polygon: &[Point], holes: &[Vec<Point>]) -> bool {
    point_in_polygon(p, polygon) && !holes.iter().any(|h| point_in_polygon(p, h))
}

/// Does the bounding box touch the polygon (corner inside, vertex inside the
/// box, or edges crossing)?
fn box_touches(b: &Bounds, polygon: &[Point], holes: &[Vec<Point>]) -> bool {
    if b.corners().iter().any(|c| inside_area(*c, polygon, holes))
        || inside_area(b.center, polygon, holes)
    {
        return true;
    }
    if polygon
        .iter()
        .any(|p| p.x >= b.min.x && p.x <= b.max.x && p.y >= b.min.y && p.y <= b.max.y)
    {
        return true;
    }
    let c = b.corners();
    for i in 0..4 {
        for j in 0..polygon.len() {
            if segments_cross(
                c[i],
                c[(i + 1) % 4],
                polygon[j],
                polygon[(j + 1) % polygon.len()],
            ) {
                return true;
            }
        }
    }
    false
}

/// Which objects a take-off counts.
#[derive(Debug, Clone, Default)]
pub enum Filter {
    /// Everything on the floors asked for.
    #[default]
    All,
    /// Calculate Materials From Selection.
    Selection(BTreeSet<(usize, String)>),
    /// Calculate From Area (a Materials List Polyline).
    Area {
        polygon: Vec<Point>,
        holes: Vec<Vec<Point>>,
        mode: IncludedObjects,
    },
    /// Calculate Materials in Room: objects whose centre is in the room, its
    /// floor and ceiling finishes and the wall surfaces facing it.
    Room {
        floor: usize,
        key: String,
        polygon: Vec<Point>,
    },
}

impl Filter {
    /// Does the object count? `bounds` is only asked for when the filter is
    /// by position.
    pub fn counts(&self, floor: usize, key: &str, bounds: impl FnOnce() -> Bounds) -> bool {
        match self {
            Filter::All => true,
            Filter::Selection(set) => set.contains(&(floor, key.to_string())),
            Filter::Area {
                polygon,
                holes,
                mode,
            } => {
                let b = bounds();
                match mode {
                    IncludedObjects::ByCenter => inside_area(b.center, polygon, holes),
                    IncludedObjects::Contained => {
                        b.corners().iter().all(|c| inside_area(*c, polygon, holes))
                    }
                    IncludedObjects::Intersected => box_touches(&b, polygon, holes),
                }
            }
            Filter::Room {
                floor: f,
                key: k,
                polygon,
            } => {
                // The room itself, and what is in it. The walls around it are
                // not counted (their surfaces are, see `room_wall_finish`).
                if floor != *f {
                    return false;
                }
                if key == k {
                    return true;
                }
                if key.starts_with("wall:") || key.starts_with("room:") {
                    return false;
                }
                point_in_polygon(bounds().center, polygon)
            }
        }
    }

    /// An opening counts in a room when its centre is inside or within
    /// `slack` inches of the room's outline (it sits in a wall of the room).
    fn counts_opening(&self, floor: usize, key: &str, center: Point, slack: f64) -> bool {
        match self {
            Filter::Room {
                floor: f, polygon, ..
            } => {
                floor == *f
                    && (point_in_polygon(center, polygon)
                        || (0..polygon.len()).any(|i| {
                            plan_core::geometry::dist_to_segment(
                                center,
                                polygon[i],
                                polygon[(i + 1) % polygon.len()],
                            ) <= slack
                        }))
            }
            _ => self.counts(floor, key, || Bounds::at(center)),
        }
    }

    /// Are the wall surfaces facing room `key` counted (Room scope, or a
    /// selected room)?
    fn room_wall_finish(&self, floor: usize, key: &str) -> bool {
        match self {
            Filter::Room {
                floor: f, key: k, ..
            } => *f == floor && k == key,
            Filter::Selection(set) => set.contains(&(floor, key.to_string())),
            _ => false,
        }
    }

    /// Does the site (soil cut and fill) count?
    fn counts_site(&self) -> bool {
        match self {
            Filter::All => true,
            Filter::Selection(set) => set.iter().any(|(_, k)| k == "terrain"),
            _ => false,
        }
    }

    /// Is the filter a plain "everything"?
    pub fn is_all(&self) -> bool {
        matches!(self, Filter::All)
    }
}

/// What the engine needs beyond the project.
#[derive(Debug, Clone)]
pub struct Options {
    pub filter: Filter,
    pub framing: FramingStyle,
    /// Soil and plants belong to every floor of an All Floors list.
    pub site_everywhere: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            filter: Filter::All,
            framing: FramingStyle::BuyList,
            site_everywhere: false,
        }
    }
}

// ------------------------------------------------------------ object bounds --

fn wall_bounds(w: &plan_core::Wall) -> Bounds {
    let t = w.thickness * 0.5;
    let (a, b) = (w.start, w.end);
    Bounds {
        min: Point::new(a.x.min(b.x) - t, a.y.min(b.y) - t),
        max: Point::new(a.x.max(b.x) + t, a.y.max(b.y) + t),
        center: Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5),
    }
}

fn opening_center(f: &Floor, o: &plan_core::Opening) -> Point {
    match f.wall(o.wall_id) {
        Some(w) => {
            let len = w.length().max(1e-9);
            let t = o.center_offset / len;
            Point::new(
                w.start.x + (w.end.x - w.start.x) * t,
                w.start.y + (w.end.y - w.start.y) * t,
            )
        }
        None => Point::ZERO,
    }
}

fn cabinet_bounds(c: &Cabinet) -> Bounds {
    let (s, co) = c.angle.sin_cos();
    let at =
        |x: f64, y: f64| Point::new(c.position.x + x * co - y * s, c.position.y + x * s + y * co);
    Bounds::of_points(&[
        at(0.0, 0.0),
        at(c.width, 0.0),
        at(c.width, c.depth),
        at(0.0, c.depth),
    ])
}

fn pad_bounds(center: Point, half: f64) -> Bounds {
    Bounds {
        min: Point::new(center.x - half, center.y - half),
        max: Point::new(center.x + half, center.y + half),
        center,
    }
}

/// One roof plane of a floor with its plan footprint.
struct RoofPlane {
    key: String,
    area_in2: f64,
    eave_in: f64,
    gutters: bool,
    bounds: Bounds,
}

fn roof_planes(f: &Floor) -> Vec<RoofPlane> {
    f.roofs
        .iter()
        .filter(|v| v.get("kind").and_then(|k| k.as_str()) == Some("plane"))
        .enumerate()
        .filter_map(|(n, v)| {
            let poly: Vec<[f64; 3]> = serde_json::from_value(v.get("polygon3d")?.clone()).ok()?;
            if poly.len() < 3 {
                return None;
            }
            // Newell vector: half its length is the true area.
            let mut s = [0.0; 3];
            for i in 0..poly.len() {
                let (c, d) = (poly[i], poly[(i + 1) % poly.len()]);
                s[0] += (c[1] - d[1]) * (c[2] + d[2]);
                s[1] += (c[2] - d[2]) * (c[0] + d[0]);
                s[2] += (c[0] - d[0]) * (c[1] + d[1]);
            }
            let area = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * 0.5;
            let eave = {
                let (a, b) = (poly[0], poly[1]);
                ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            };
            let gutters = v.get("gutters").and_then(|g| g.as_bool()).unwrap_or(false);
            let id = v
                .get("id")
                .and_then(|i| i.as_u64())
                .map_or_else(|| format!("n{n}"), |i| i.to_string());
            // The 3D frame has Z = -plan y.
            let plan: Vec<Point> = poly.iter().map(|p| Point::new(p[0], -p[2])).collect();
            Some(RoofPlane {
                key: format!("roof:{id}"),
                area_in2: area,
                eave_in: eave,
                gutters,
                bounds: Bounds::of_points(&plan),
            })
        })
        .collect()
}

fn member_bounds(m: &Member) -> Bounds {
    let a = m.transform.origin;
    let d = m.transform.axis_x;
    let b = [
        a[0] + d[0] * m.length,
        a[1] + d[1] * m.length,
        a[2] + d[2] * m.length,
    ];
    Bounds::of_points(&[Point::new(a[0], -a[2]), Point::new(b[0], -b[2])])
}

/// `room:<floor>:<x>,<y>` for a detected room (the Property Manager's key).
pub fn room_key(floor: usize, room: &Room) -> String {
    PropKey::room(floor, room.centroid).0
}

// ----------------------------------------------------------------- take-off --

#[derive(Default)]
struct Takeoff {
    studs: [Acc; 2],
    plate_lf: [Acc; 2],
    drywall: Acc,
    exterior: Acc,
    sheathing_extra: Acc,
    siding_extra: Acc,
    layers: BTreeMap<(String, String, String), Acc>,
    doors: BTreeMap<(i64, i64), Acc>,
    windows: BTreeMap<(i64, i64), Acc>,
}

#[allow(clippy::too_many_arguments)]
fn emit(
    out: &mut Vec<Raw>,
    category: &str,
    key: &str,
    item: impl Into<String>,
    size: &str,
    unit: &str,
    acc: &Acc,
    rule: Rule,
) {
    let mut line = MaterialLine::new(category, key, item, size, rule.apply(acc.total()), unit);
    line.net = line.quantity;
    out.push(Raw {
        line,
        sources: acc.sources(),
        rule,
    });
}

/// The take-off of `floors` for the objects `opts.filter` counts, with the
/// objects behind every row. No waste, no prices, no ids.
pub fn take_off_raw(
    project: &Project,
    floors: &[usize],
    active_rooms: Option<(usize, &[Room])>,
    master: &MasterList,
    opts: &Options,
) -> Vec<Raw> {
    let floors: Vec<usize> = floors
        .iter()
        .copied()
        .filter(|f| *f < project.floors.len())
        .collect();
    let multi = floors.len() > 1;
    let filter = &opts.filter;
    let mut t = Takeoff::default();
    let mut out: Vec<Raw> = Vec::new();
    let mut room_lines: Vec<Raw> = Vec::new();
    // (floor, framing source key, member) of the framing to count.
    let mut framing_auto: Vec<(usize, String, Member)> = Vec::new();
    let mut framing_manual: Vec<(usize, String, FramingMember)> = Vec::new();

    for &fi in &floors {
        let f = &project.floors[fi];
        let (auto, manual) = framing_members(f);
        let framed = !auto.is_empty() || !manual.is_empty();
        for (n, m) in auto.into_iter().enumerate() {
            let (key, wall_ok) = match m.wall_id {
                Some(w) => {
                    let wk = PropKey::wall(w).0;
                    let ok = f
                        .wall(w)
                        .is_some_and(|wall| filter.counts(fi, &wk, || wall_bounds(wall)));
                    (wk, ok)
                }
                None => {
                    let k = format!("framing:auto:{fi}:{n}");
                    let ok = filter.counts(fi, &k, || member_bounds(&m));
                    (k, ok)
                }
            };
            if wall_ok {
                framing_auto.push((fi, key, m));
            }
        }
        for m in manual {
            let k = PropKey::framing(m.id).0;
            if filter.counts(fi, &k, || Bounds::of_points(&[m.start, m.end])) {
                framing_manual.push((fi, k, m));
            }
        }

        for w in &f.walls {
            let wk = PropKey::wall(w.id).0;
            let counted = filter.counts(fi, &wk, || wall_bounds(w));
            let len = w.length();
            let openings: Vec<_> = f.openings_on(w.id).collect();
            let k = usize::from(w.kind == WallKind::Interior);
            let layered = layered_type(project, w);
            if counted {
                if !framed {
                    t.studs[k].add(
                        fi,
                        &wk,
                        (len / STUD_SPACING).ceil() + 1.0 + 4.0 * openings.len() as f64,
                    );
                    t.plate_lf[k].add(fi, &wk, len / 12.0 * 3.0);
                }
                let opening_sq_ft: f64 = openings.iter().map(|o| o.width * o.height / 144.0).sum();
                let net = (len * w.height / 144.0 - opening_sq_ft).max(0.0);
                if let Some(def) = layered {
                    for l in def.layers.iter().filter(|l| !l.is_main) {
                        match super::component_of(&l.name, &l.material) {
                            Component::Skip => {}
                            Component::Sheathing => t.sheathing_extra.add(fi, &wk, net),
                            Component::Drywall => t.drywall.add(fi, &wk, net),
                            Component::Siding => t.siding_extra.add(fi, &wk, net),
                            Component::Other(category) => t
                                .layers
                                .entry((category.to_string(), l.name.clone(), l.material.clone()))
                                .or_default()
                                .add(fi, &wk, net),
                        }
                    }
                } else {
                    match w.kind {
                        WallKind::Exterior => {
                            t.exterior.add(fi, &wk, net);
                            t.drywall.add(fi, &wk, net);
                        }
                        WallKind::Interior => t.drywall.add(fi, &wk, 2.0 * net),
                    }
                }
            }
            for o in openings {
                let ok = match o.kind {
                    OpeningKind::Door => PropKey::door(o.id).0,
                    OpeningKind::Window => PropKey::window(o.id).0,
                };
                let c = opening_center(f, o);
                if !filter.counts_opening(fi, &ok, c, w.thickness * 0.5 + 2.0) {
                    continue;
                }
                let key = size_key(o.width, o.height);
                let map = match o.kind {
                    OpeningKind::Door => &mut t.doors,
                    OpeningKind::Window => &mut t.windows,
                };
                map.entry(key).or_default().add(fi, &ok, 1.0);
            }
        }

        let detected;
        let rooms: &[Room] = match active_rooms {
            Some((a, r)) if a == fi => r,
            _ => {
                detected = detect_rooms(&f.walls, 1.0);
                &detected
            }
        };
        for r in rooms {
            let rk = room_key(fi, r);
            if !filter.counts(fi, &rk, || Bounds::of_points(&r.polygon)) {
                continue;
            }
            let name = room_name(f, r);
            let name = if multi {
                format!("{} - {name}", f.name)
            } else {
                name
            };
            let area = r.area_sq_ft();
            let mut a = Acc::default();
            a.add(fi, &rk, area);
            let mut floor_line = Vec::new();
            // A layered platform lists its layers (Subfloor, Flooring,
            // Framing, Wallboard); the others keep the two rows they had.
            platform_lines::emit_room(&mut floor_line, f, r, &name, &a);
            room_lines.extend(floor_line);
            if filter.room_wall_finish(fi, &rk) {
                let sq_ft = room_wall_finish_sq_ft(f, r);
                t.drywall.add(fi, &rk, sq_ft);
            }
        }
    }

    // ---- Foundation ----
    let (mut slab, mut pad, mut pier) = (Acc::default(), Acc::default(), Acc::default());
    for &fi in &floors {
        let layer = FoundationLayer::load(&project.floors[fi]);
        let holes: Vec<_> = layer.holes.iter().collect();
        for s in &layer.slabs {
            let k = format!("foundation:{}", s.id);
            if filter.counts(fi, &k, || Bounds::of_points(&s.outline)) {
                slab.add(fi, &k, s.concrete_cu_yd(&holes));
            }
        }
        for p in &layer.pads {
            let k = format!("foundation:{}", p.id);
            if filter.counts(fi, &k, || pad_bounds(p.center, p.size * 0.5)) {
                pad.add(fi, &k, p.concrete_cu_yd());
            }
        }
        for p in &layer.piers {
            let k = format!("foundation:{}", p.id);
            if filter.counts(fi, &k, || pad_bounds(p.center, p.diameter * 0.5)) {
                pier.add(fi, &k, p.concrete_cu_yd());
            }
        }
    }
    for (key, item, acc) in [
        ("Slab concrete", "Slab and footing concrete", &slab),
        ("Pad concrete", "Pad concrete", &pad),
        ("Pier concrete", "Pier concrete", &pier),
    ] {
        if acc.total() > 0.0 {
            emit(
                &mut out,
                "Foundation",
                key,
                item,
                "",
                "cu yd",
                acc,
                Rule::Hundredths,
            );
        }
    }

    // ---- Framing ----
    let names = ["2x6 Stud @ 16\" o.c.", "2x4 Stud @ 16\" o.c."];
    let stud_keys = ["2x6 stud", "2x4 stud"];
    let plates = [
        "2x6 Plate (1 bottom + 2 top)",
        "2x4 Plate (1 bottom + 2 top)",
    ];
    let plate_keys = ["2x6 plate", "2x4 plate"];
    for k in 0..2 {
        if t.studs[k].total() > 0.0 {
            emit(
                &mut out,
                "Framing",
                stud_keys[k],
                names[k],
                if k == 0 { "2x6" } else { "2x4" },
                "ea",
                &t.studs[k],
                Rule::Sum,
            );
        }
    }
    for k in 0..2 {
        if t.plate_lf[k].total() > 0.0 {
            emit(
                &mut out,
                "Framing",
                plate_keys[k],
                plates[k],
                if k == 0 { "2x6" } else { "2x4" },
                "lf",
                &t.plate_lf[k],
                Rule::Ceil,
            );
        }
    }
    if !framing_auto.is_empty() || !framing_manual.is_empty() {
        framing_rows(
            &mut out,
            &framing_auto,
            &framing_manual,
            master,
            opts.framing,
        );
    }
    {
        let mut acc = Acc::default();
        for (fl, k, v) in t.exterior.0.iter().map(|((f, k), v)| (f, k, v)) {
            acc.add(*fl, k, *v);
        }
        for ((fl, k), v) in &t.sheathing_extra.0 {
            acc.add(*fl, k, *v);
        }
        if acc.total() > 0.0 {
            emit(
                &mut out,
                "Framing",
                "Wall sheathing 7/16\" OSB 4x8 sheet",
                "Wall sheathing 7/16\" OSB 4x8 sheet",
                "4x8",
                "sheet",
                &acc,
                Rule::Sheets,
            );
        }
    }
    for ((category, name, material), acc) in &t.layers {
        if category == "Framing" && acc.total() > 0.0 {
            out.push(layer_raw(category, name, material, acc));
        }
    }

    // ---- Roofing ----
    let (mut area, mut squares, mut bundles, mut eave, mut gutter) = (
        Acc::default(),
        Acc::default(),
        Acc::default(),
        Acc::default(),
        Acc::default(),
    );
    for &fi in &floors {
        for p in roof_planes(&project.floors[fi]) {
            if !filter.counts(fi, &p.key, || p.bounds) {
                continue;
            }
            let sq_ft = p.area_in2 / 144.0;
            area.add(fi, &p.key, sq_ft);
            squares.add(fi, &p.key, sq_ft / 100.0);
            bundles.add(fi, &p.key, sq_ft / 100.0 * BUNDLES_PER_SQUARE);
            eave.add(fi, &p.key, p.eave_in / 12.0);
            if p.gutters {
                gutter.add(fi, &p.key, p.eave_in / 12.0);
            }
        }
    }
    if area.total() > 0.0 {
        emit(
            &mut out,
            "Roofing",
            "Roof area",
            "Roof area",
            "",
            "sq ft",
            &area,
            Rule::Ceil,
        );
        emit(
            &mut out,
            "Roofing",
            "Roofing squares",
            "Roofing (100 sq ft squares)",
            "",
            "sq",
            &squares,
            Rule::Hundredths,
        );
        emit(
            &mut out,
            "Roofing",
            "Shingle bundles",
            "Shingles (3 bundles per square)",
            "",
            "bundle",
            &bundles,
            Rule::Ceil,
        );
        emit(
            &mut out,
            "Roofing",
            "Underlayment",
            "Roof underlayment",
            "",
            "sq",
            &squares,
            Rule::Hundredths,
        );
        emit(
            &mut out,
            "Roofing",
            "Drip edge",
            "Drip edge along eaves",
            "",
            "lf",
            &eave,
            Rule::Ceil,
        );
        if gutter.total() > 0.0 {
            emit(
                &mut out,
                "Roofing",
                "Gutters",
                "Gutters",
                "",
                "lf",
                &gutter,
                Rule::Ceil,
            );
        }
    }

    // ---- Siding ----
    {
        let mut acc = Acc::default();
        for ((fl, k), v) in &t.exterior.0 {
            acc.add(*fl, k, *v);
        }
        for ((fl, k), v) in &t.siding_extra.0 {
            acc.add(*fl, k, *v);
        }
        if acc.total() > 0.0 {
            emit(
                &mut out,
                "Siding",
                "Siding",
                "Siding",
                "",
                "sq ft",
                &acc,
                Rule::Ceil,
            );
        }
    }
    for ((category, name, material), acc) in &t.layers {
        if category == "Siding" && acc.total() > 0.0 {
            out.push(layer_raw(category, name, material, acc));
        }
    }

    // ---- Windows, Doors ----
    for (k, acc) in &t.windows {
        let text = size_text(*k);
        emit(
            &mut out,
            "Windows",
            &format!("Window {text}"),
            format!("Window {text}"),
            &text,
            "ea",
            acc,
            Rule::Sum,
        );
    }
    for (k, acc) in &t.doors {
        let text = size_text(*k);
        emit(
            &mut out,
            "Doors",
            &format!("Door {text}"),
            format!("Door {text}"),
            &text,
            "ea",
            acc,
            Rule::Sum,
        );
    }

    // ---- Cabinets (by label), countertop ----
    let mut cabinets: BTreeMap<(String, String), Acc> = BTreeMap::new();
    let mut counter = Acc::default();
    for &fi in &floors {
        for v in &project.floors[fi].cabinets {
            let Ok(c) = serde_json::from_value::<Cabinet>(v.clone()) else {
                continue;
            };
            let ck = PropKey::cabinet(c.id).0;
            if !filter.counts(fi, &ck, || cabinet_bounds(&c)) {
                continue;
            }
            let label = if c.label.trim().is_empty() {
                auto_label(&c)
            } else {
                c.label.clone()
            };
            let size = format!(
                "{} x {} x {}",
                fmt_ft_in(c.width),
                fmt_ft_in(c.depth),
                fmt_ft_in(c.height)
            );
            cabinets.entry((label, size)).or_default().add(fi, &ck, 1.0);
            if c.countertop.is_some() {
                counter.add(fi, &ck, c.width * c.depth / 144.0);
            }
        }
    }
    for ((label, size), acc) in &cabinets {
        emit(
            &mut out,
            "Cabinets",
            &format!("Cabinet {label}"),
            format!("Cabinet {label}"),
            size,
            "ea",
            acc,
            Rule::Sum,
        );
    }
    if counter.total() > 0.0 {
        emit(
            &mut out,
            "Cabinets",
            "Countertop",
            "Countertop",
            "",
            "sq ft",
            &counter,
            Rule::Ceil,
        );
    }

    // ---- Electrical ----
    let mut devices: BTreeMap<String, Acc> = BTreeMap::new();
    let mut rope_lights = Acc::default();
    for &fi in &floors {
        let Some(v) = project.floors[fi].electrical.as_ref() else {
            continue;
        };
        if let Ok(layer) = serde_json::from_value::<ElectricalLayer>(v.clone()) {
            for d in &layer.devices {
                let dk = PropKey::device(fi, d.id).0;
                if filter.counts(fi, &dk, || Bounds::at(d.position)) {
                    devices
                        .entry(d.kind.name().to_string())
                        .or_default()
                        .add(fi, &dk, 1.0);
                }
            }
            // Rope lights are listed by length (E-22).
            for r in &layer.ropes {
                let key = format!("rope:{fi}:{}", r.id);
                if filter.counts(fi, &key, || Bounds::of_points(&r.points)) {
                    rope_lights.add(fi, &key, r.length() / 12.0);
                }
            }
        }
    }
    for (name, acc) in &devices {
        emit(
            &mut out,
            "Electrical",
            name,
            name.clone(),
            "",
            "ea",
            acc,
            Rule::Sum,
        );
    }
    if rope_lights.total() > 0.0 {
        emit(
            &mut out,
            "Electrical",
            "Rope Light",
            "Rope Light",
            "",
            "lf",
            &rope_lights,
            Rule::Sum,
        );
    }

    // ---- Fixtures, Landscaping ----
    let listed = |kind: ScheduleKind| -> BTreeMap<(String, String), Acc> {
        let mut m: BTreeMap<(String, String), Acc> = BTreeMap::new();
        for e in entries(project, kind, None) {
            let in_floor =
                floors.contains(&e.floor) || (kind == ScheduleKind::Plant && opts.site_everywhere);
            if !in_floor {
                continue;
            }
            let key = if e.id != 0 {
                PropKey::symbol(e.id).0
            } else {
                format!("plant:{},{}", e.position.x.round(), e.position.y.round())
            };
            if !filter.counts(e.floor, &key, || Bounds::at(e.position)) {
                continue;
            }
            m.entry((e.name.clone(), e.size.clone()))
                .or_default()
                .add(e.floor, &key, 1.0);
        }
        m
    };
    for ((name, size), acc) in &listed(ScheduleKind::Fixture) {
        emit(
            &mut out,
            "Fixtures",
            name,
            name.clone(),
            size,
            "ea",
            acc,
            Rule::Sum,
        );
    }
    for ((name, size), acc) in &listed(ScheduleKind::Plant) {
        emit(
            &mut out,
            "Landscaping",
            name,
            name.clone(),
            size,
            "ea",
            acc,
            Rule::Sum,
        );
    }
    // The site's cut and fill (graded pads), in cubic yards.
    if (floors.contains(&0) || opts.site_everywhere) && filter.counts_site() {
        let (cut, fill) = crate::terrain_report::soil_yards(project);
        for (item, qty) in [
            ("Soil cut (excavation)", cut),
            ("Soil fill (backfill)", fill),
        ] {
            if qty > 0.0 {
                let mut a = Acc::default();
                a.add(0, "terrain", qty);
                emit(
                    &mut out,
                    "Landscaping",
                    item,
                    item,
                    "",
                    "cu yd",
                    &a,
                    Rule::Sum,
                );
            }
        }
    }

    // ---- Interior Finishes ----
    if t.drywall.total() > 0.0 {
        emit(
            &mut out,
            "Interior Finishes",
            "Wall drywall 1/2\" 4x8 sheet",
            "Wall drywall 1/2\" 4x8 sheet",
            "4x8",
            "sheet",
            &t.drywall,
            Rule::Sheets,
        );
    }
    for ((category, name, material), acc) in &t.layers {
        if category == "Interior Finishes" && acc.total() > 0.0 {
            out.push(layer_raw(category, name, material, acc));
        }
    }
    // ---- Layered material regions, 3D solids, compound solids and blocks ----
    // (`plan_core::material_region::r15_takeoff`). A by-position filter has
    // no box for these rows, so they count for All and Selection only.
    for &fi in &floors {
        if !matches!(filter, Filter::All | Filter::Selection(_)) {
            continue;
        }
        for row in plan_core::material_region::r15_takeoff(project, fi) {
            if !filter.counts(fi, &row.key, || Bounds::of_points(&[])) {
                continue;
            }
            let mut a = Acc::default();
            a.add(fi, &row.key, row.qty);
            emit(
                &mut out,
                &row.category,
                &row.item,
                row.item.clone(),
                "",
                row.unit,
                &a,
                Rule::Sum,
            );
        }
    }
    out.extend(room_lines);
    out
}

/// A wall-type layer measured in square feet.
fn layer_raw(category: &str, name: &str, material: &str, acc: &Acc) -> Raw {
    Raw {
        line: layer_line(category, name, material, acc.total()),
        sources: acc.sources(),
        rule: Rule::Ceil,
    }
}

/// Wall surface facing `room`: each edge of its inner outline times the height
/// of the wall it runs along, less the openings in that stretch.
fn room_wall_finish_sq_ft(f: &Floor, room: &Room) -> f64 {
    let poly = &room.polygon;
    let inner = if room.inner_polygon.len() == poly.len() {
        &room.inner_polygon
    } else {
        poly
    };
    let mut total = 0.0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let mid = Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        let Some(w) = f
            .walls
            .iter()
            .filter(|w| {
                plan_core::geometry::dist_to_segment(mid, w.start, w.end) <= w.thickness * 0.5 + 1.0
            })
            .min_by(|x, y| {
                let dx = plan_core::geometry::dist_to_segment(mid, x.start, x.end);
                let dy = plan_core::geometry::dist_to_segment(mid, y.start, y.end);
                dx.partial_cmp(&dy).unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            continue;
        };
        let edge = inner[i].dist(inner[(i + 1) % inner.len()]);
        // The stretch of the wall this edge covers, measured from its start.
        let wl = w.length().max(1e-9);
        let proj = |p: Point| {
            ((p.x - w.start.x) * (w.end.x - w.start.x) + (p.y - w.start.y) * (w.end.y - w.start.y))
                / wl
        };
        let (t0, t1) = {
            let (p, q) = (proj(a), proj(b));
            (p.min(q), p.max(q))
        };
        let holes: f64 = f
            .openings_on(w.id)
            .filter(|o| o.center_offset >= t0 - 1.0 && o.center_offset <= t1 + 1.0)
            .map(|o| o.width * o.height / 144.0)
            .sum();
        total += (edge * w.height / 144.0 - holes).max(0.0);
    }
    total
}

/// The framing rows: lumber by stock length (Buy List), by cut length (Cut
/// List) or in linear feet, and the board feet.
fn framing_rows(
    out: &mut Vec<Raw>,
    auto: &[(usize, String, Member)],
    manual: &[(usize, String, FramingMember)],
    master: &MasterList,
    style: FramingStyle,
) {
    // (size, stock feet) -> pieces; (size, cut) -> pieces; size -> inches.
    let mut stock: BTreeMap<(String, u32), Acc> = BTreeMap::new();
    let mut cuts: BTreeMap<(String, i64), Acc> = BTreeMap::new();
    let mut linear: BTreeMap<String, Acc> = BTreeMap::new();
    let mut bf = Acc::default();
    let mut add_rows = |fi: usize, key: &str, list: MaterialList| {
        for r in &list.rows {
            let (l, per_cut) = master.stock_pieces(r.length_in);
            stock
                .entry((r.size.clone(), l))
                .or_default()
                .add(fi, key, f64::from(r.qty * per_cut));
            cuts.entry((r.size.clone(), (r.length_in * 16.0).round() as i64))
                .or_default()
                .add(fi, key, f64::from(r.qty));
            linear.entry(r.size.clone()).or_default().add(
                fi,
                key,
                f64::from(r.qty) * r.length_in / 12.0,
            );
        }
        bf.add(fi, key, list.total_board_feet());
    };
    for (fi, key, m) in auto {
        add_rows(
            *fi,
            key,
            MaterialList::from_members(std::slice::from_ref(m), &[]),
        );
    }
    for (fi, key, m) in manual {
        add_rows(
            *fi,
            key,
            MaterialList::from_members(&[], std::slice::from_ref(m)),
        );
    }
    match style {
        FramingStyle::BuyList => {
            for ((size, l), acc) in &stock {
                emit(
                    out,
                    "Framing",
                    &format!("{size} lumber"),
                    format!("{size} x {l}' lumber"),
                    &format!("{size} x {l}'"),
                    "ea",
                    acc,
                    Rule::Sum,
                );
            }
        }
        FramingStyle::CutList => {
            for ((size, sixteenths), acc) in &cuts {
                let len = *sixteenths as f64 / 16.0;
                emit(
                    out,
                    "Framing",
                    &format!("{size} cut {}", fmt_ft_in(len)),
                    format!("{size} cut to {}", fmt_ft_in(len)),
                    &format!("{size} x {}", fmt_ft_in(len)),
                    "ea",
                    acc,
                    Rule::Sum,
                );
            }
        }
        FramingStyle::LinearFeet => {
            for (size, acc) in &linear {
                emit(
                    out,
                    "Framing",
                    &format!("{size} linear feet"),
                    format!("{size} lumber, linear feet"),
                    size,
                    "lf",
                    acc,
                    Rule::Ceil,
                );
            }
        }
    }
    emit(
        out,
        "Framing",
        "board feet",
        "Framing lumber, board feet",
        "",
        "bf",
        &bf,
        Rule::Ceil,
    );
}
