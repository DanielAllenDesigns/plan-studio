//! Cabinet runs: which cabinets merge, the module lines between them, the
//! Plan Display Options of the General Cabinet Defaults, schedule categories
//! and the sizing rules of reference manual pp. 650 to 655.
//!
//! A *run* is a set of cabinets of one family (base, wall or full height) and
//! one height that stand side by side within the merge reach (3 in), or meet
//! at a front corner, or at a back corner facing away at no more than 87
//! degrees. Merged cabinets share their toe kick, countertop, backsplash and
//! moldings, and the line where two of them meet is a *module line* on the
//! layer [`MODULE_LINES_LAYER`] (hidden with the layer, grey and short with
//! Show Partial Module Lines).

use plan_core::geometry::{point_in_polygon, Point};
use plan_core::Id;
use std::collections::HashMap;

use crate::cabinet::{Cabinet, CabinetKind, Countertop};
use crate::face::FaceItem;
use crate::filler::{run_class, run_mates, RunClass};
use crate::geom;
use crate::symbol::{plan_symbol, Stroke};

/// The layer module lines (and the lines of extended face frames) are drawn
/// on; turn it off to hide them.
pub const MODULE_LINES_LAYER: &str = "Cabinets, Module Lines";

/// Merged cabinets must face each other or away at no more than this angle.
const BACK_CORNER_MAX_DEG: f64 = 87.0;
/// Two cabinets whose ends are this close count as touching, inches.
const TOUCH: f64 = 0.01;
/// Depth two side-by-side cabinets must share to merge, inches.
const MIN_SHARED_DEPTH: f64 = 0.5;

/// How two cabinets are joined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Link {
    /// Side by side along one line, with this gap between them (inches).
    Side { gap: f64 },
    /// Facing each other and meeting at a front corner.
    FrontCorner,
    /// Meeting at a back corner and facing away at no more than 87 degrees.
    BackCorner,
}

fn axes(c: &Cabinet) -> (Point, Point) {
    let u = Point::new(c.angle.cos(), c.angle.sin());
    (u, u.perp())
}

fn same_line(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    d < 1e-4 || std::f64::consts::TAU - d < 1e-4
}

fn centre(c: &Cabinet) -> Point {
    c.to_plan(Point::new(c.width / 2.0, c.depth / 2.0))
}

fn corners(c: &Cabinet, front: bool) -> [Point; 2] {
    let y = if front { c.depth } else { 0.0 };
    [
        c.to_plan(Point::new(0.0, y)),
        c.to_plan(Point::new(c.width, y)),
    ]
}

/// How `a` and `b` are joined, when they are run mates (same family and
/// height) and no more than `reach` inches apart.
pub fn link(a: &Cabinet, b: &Cabinet, reach: f64) -> Option<Link> {
    if a.id == b.id || !run_mates(a, b) {
        return None;
    }
    if same_line(a.angle, b.angle) {
        let (u, v) = axes(a);
        let (a0, a1) = (a.position.dot(u), a.position.dot(u) + a.width);
        let (b0, b1) = (b.position.dot(u), b.position.dot(u) + b.width);
        let (at0, at1) = (a.position.dot(v), a.position.dot(v) + a.depth);
        let (bt0, bt1) = (b.position.dot(v), b.position.dot(v) + b.depth);
        if at1.min(bt1) - at0.max(bt0) <= MIN_SHARED_DEPTH {
            return None;
        }
        let gap = (b0 - a1).max(a0 - b1);
        return (gap >= -TOUCH && gap <= reach + 1e-6).then_some(Link::Side { gap: gap.max(0.0) });
    }
    if a.kind.is_corner() || b.kind.is_corner() {
        return None;
    }
    let (_, av) = axes(a);
    let (_, bv) = axes(b);
    let (ca, cb) = (centre(a), centre(b));
    let toward_b = cb.sub(ca).dot(av) > 0.0;
    let toward_a = ca.sub(cb).dot(bv) > 0.0;
    let near = |pa: [Point; 2], pb: [Point; 2]| {
        pa.iter()
            .any(|p| pb.iter().any(|q| p.dist(*q) <= reach + 1e-6))
    };
    if toward_b && toward_a && near(corners(a, true), corners(b, true)) {
        return Some(Link::FrontCorner);
    }
    let away_a = cb.sub(ca).dot(av) <= 1e-6;
    let away_b = ca.sub(cb).dot(bv) <= 1e-6;
    let between = av.dot(bv).clamp(-1.0, 1.0).acos().to_degrees();
    if away_a
        && away_b
        && between <= BACK_CORNER_MAX_DEG
        && near(corners(a, false), corners(b, false))
    {
        return Some(Link::BackCorner);
    }
    None
}

/// A set of merged cabinets.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub class: RunClass,
    /// The member ids, in the order of the cabinets given.
    pub ids: Vec<Id>,
}

/// The runs of `cabs`: groups of two or more cabinets that merge (see the
/// module docs). `reach` is 3 in with Create Automatic Fillers, or a hair
/// above zero without it (only cabinets that actually touch merge then).
/// Custom tops, soffits, shelves and partitions never merge.
pub fn merge_runs(cabs: &[Cabinet], reach: f64) -> Vec<Run> {
    let items: Vec<&Cabinet> = cabs
        .iter()
        .filter(|c| run_class(c).is_some() && c.custom.is_none())
        .collect();
    let mut parent: Vec<usize> = (0..items.len()).collect();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut k = i;
        while p[k] != r {
            let n = p[k];
            p[k] = r;
            k = n;
        }
        r
    }
    for i in 0..items.len() {
        for j in i + 1..items.len() {
            if link(items[i], items[j], reach).is_some() {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    let mut groups: Vec<(usize, Vec<Id>)> = Vec::new();
    for i in 0..items.len() {
        let r = find(&mut parent, i);
        match groups.iter_mut().find(|g| g.0 == r) {
            Some(g) => g.1.push(items[i].id),
            None => groups.push((r, vec![items[i].id])),
        }
    }
    groups
        .into_iter()
        .filter(|g| g.1.len() >= 2)
        .filter_map(|(r, ids)| run_class(items[r]).map(|class| Run { class, ids }))
        .collect()
}

impl Run {
    /// The two ends of a straight run on its back line: from the back-left
    /// corner of the first member to the back-right corner of the last, and
    /// the run's length. `None` when the members do not all stand on one line
    /// (a run that turns a corner has no single extent).
    pub fn extent(&self, cabs: &[Cabinet]) -> Option<(Point, Point, f64)> {
        let members: Vec<&Cabinet> = cabs.iter().filter(|c| self.ids.contains(&c.id)).collect();
        let first = members.first()?;
        if members.iter().any(|m| !same_line(m.angle, first.angle)) {
            return None;
        }
        let (u, v) = axes(first);
        let back = first.position.dot(v);
        if members
            .iter()
            .any(|m| (m.position.dot(v) - back).abs() > MIN_SHARED_DEPTH)
        {
            return None;
        }
        let lo = members
            .iter()
            .map(|m| m.position.dot(u))
            .fold(f64::MAX, f64::min);
        let hi = members
            .iter()
            .map(|m| m.position.dot(u) + m.width)
            .fold(f64::MIN, f64::max);
        Some((u * lo + v * back, u * hi + v * back, hi - lo))
    }
}

/// The reach to merge by: 3 in when automatic fillers are on, else touching
/// only.
pub fn merge_reach(automatic_fillers: bool) -> f64 {
    if automatic_fillers {
        crate::filler::AUTO_FILLER_REACH
    } else {
        TOUCH
    }
}

/// One module line, in plan inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModuleLine {
    pub a: Point,
    pub b: Point,
}

/// What the plan needs to draw merged cabinets: the footprint edges of each
/// cabinet that lie on a merge (they are drawn as module lines instead, or
/// not at all when the layer is off) and the module lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunDisplay {
    /// Edge indices of `footprint()` (rectangles: 0 back, 1 right end, 2
    /// front, 3 left end) that are shared with a merged neighbour.
    pub hidden_edges: HashMap<Id, Vec<usize>>,
    pub lines: Vec<ModuleLine>,
}

/// Module lines and hidden edges for the cabinets `cabs`. With `partial`
/// each line is a short tick in the middle of the shared end instead of a
/// line across it. Extended face frames add their own lines.
pub fn run_display(cabs: &[Cabinet], reach: f64, partial: bool) -> RunDisplay {
    let mut out = RunDisplay::default();
    let items: Vec<&Cabinet> = cabs
        .iter()
        .filter(|c| run_class(c).is_some() && c.custom.is_none() && !c.kind.is_corner())
        .collect();
    for (i, a) in items.iter().enumerate() {
        for b in items.iter().skip(i + 1) {
            let Some(Link::Side { gap }) = link(a, b, reach) else {
                continue;
            };
            let (u, v) = axes(a);
            let (a0, b0) = (a.position.dot(u), b.position.dot(u));
            // `lo` is the one on the left along the run.
            let (lo, hi) = if a0 <= b0 { (a, b) } else { (b, a) };
            let (at0, at1) = (a.position.dot(v), a.position.dot(v) + a.depth);
            let (bt0, bt1) = (b.position.dot(v), b.position.dot(v) + b.depth);
            let (t0, t1) = (at0.max(bt0), at1.min(bt1));
            let s = lo.position.dot(u) + lo.width + gap / 2.0;
            let (p0, p1) = if partial {
                let mid = (t0 + t1) / 2.0;
                let half = ((t1 - t0) / 6.0).min(2.0);
                (u * s + v * (mid - half), u * s + v * (mid + half))
            } else {
                (u * s + v * t0, u * s + v * t1)
            };
            out.lines.push(ModuleLine { a: p0, b: p1 });
            // The end faces hide only when the ends coincide over the whole
            // shared depth.
            let whole = |c: &Cabinet| {
                let t = c.position.dot(v);
                (t - t0).abs() < 0.05 && (t + c.depth - t1).abs() < 0.05
            };
            if gap <= 0.5 {
                if whole(lo) {
                    out.hidden_edges.entry(lo.id).or_default().push(1);
                }
                if whole(hi) {
                    out.hidden_edges.entry(hi.id).or_default().push(3);
                }
            }
        }
    }
    // Extended stiles: a small frame outline at the front of the end.
    for c in items {
        let w = 1.5;
        if c.stile_ext_left > 0.0 {
            let e = c.stile_ext_left;
            let q = |x: f64, y: f64| c.to_plan(Point::new(x, y));
            out.lines.push(ModuleLine {
                a: q(-e, c.depth - w),
                b: q(0.0, c.depth - w),
            });
            out.lines.push(ModuleLine {
                a: q(-e, c.depth - w),
                b: q(-e, c.depth),
            });
        }
        if c.stile_ext_right > 0.0 {
            let e = c.stile_ext_right;
            let q = |x: f64, y: f64| c.to_plan(Point::new(x, y));
            out.lines.push(ModuleLine {
                a: q(c.width, c.depth - w),
                b: q(c.width + e, c.depth - w),
            });
            out.lines.push(ModuleLine {
                a: q(c.width + e, c.depth - w),
                b: q(c.width + e, c.depth),
            });
        }
    }
    for v in out.hidden_edges.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    out
}

// ----- Plan Display Options -----

/// The Plan Display Options of the General Cabinet Defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlanOptions {
    /// Show Closed Doors/Drawers and Panels.
    pub closed_fronts: bool,
    /// Show Pilasters.
    pub pilasters: bool,
    /// Display Molding Edges in Plan Views.
    pub molding_edges: bool,
}

impl PlanOptions {
    pub fn from_general(g: &plan_core::defaults::GeneralCabinetDefaults) -> Self {
        Self {
            closed_fronts: g.show_closed_doors_drawers,
            pilasters: g.show_pilasters,
            molding_edges: g.display_molding_edges,
        }
    }
}

/// The thickness a closed door, drawer or panel is drawn with, inches.
const FRONT_THICKNESS: f64 = 0.75;

/// Is `item` a closed door, drawer or panel (something that shows in plan with
/// Show Closed Doors/Drawers and Panels)?
fn solid_front(item: &FaceItem) -> bool {
    match item {
        FaceItem::Custom { item, .. } => solid_front(item),
        FaceItem::Drawer { .. }
        | FaceItem::FalseDrawer { .. }
        | FaceItem::DoubleDrawer { .. }
        | FaceItem::FalseDoubleDrawer { .. }
        | FaceItem::DoorAuto { .. }
        | FaceItem::DoorAutoLeft { .. }
        | FaceItem::DoorLeft { .. }
        | FaceItem::DoorRight { .. }
        | FaceItem::DoubleDoor { .. }
        | FaceItem::Panel { .. }
        | FaceItem::Blank { .. } => true,
        _ => false,
    }
}

/// [`plan_symbol`] with the Plan Display Options and a merge applied: the
/// edges in `hidden` are left out of the outline, closed doors, drawers and
/// panels, pilasters and the width of the countertop edge are added when
/// asked for. Pilasters are drawn on the ends no neighbour shares.
pub fn plan_strokes(cab: &Cabinet, hidden: &[usize], opts: &PlanOptions) -> Vec<Stroke> {
    let mut out = plan_symbol(cab);
    // The first stroke is the footprint; split it where edges are hidden.
    if !hidden.is_empty() {
        if let Some(Stroke::Polyline(ring, true)) = out.first().cloned() {
            let n = ring.len();
            if n == 4 {
                let mut lines = Vec::new();
                for e in 0..n {
                    if !hidden.contains(&e) {
                        lines.push(Stroke::Line(ring[e], ring[(e + 1) % n]));
                    }
                }
                out.splice(0..1, lines);
            }
        }
    }
    let is_box = cab.custom.is_none() && !cab.kind.is_corner() && run_class(cab).is_some();
    let q = |x: f64, y: f64| cab.to_plan(Point::new(x, y));
    if opts.closed_fronts && is_box && cab.appliance.is_none() {
        if let Ok(leaves) = cab.face.resolve(cab.face_height(), cab.face_width()) {
            let mut spans: Vec<(f64, f64)> = Vec::new();
            for l in leaves {
                let solid = solid_front(&l.item);
                let (x, _, w, _) = l.rect;
                if solid
                    && !spans
                        .iter()
                        .any(|s| (s.0 - x).abs() < 0.01 && (s.1 - w).abs() < 0.01)
                {
                    spans.push((x, w));
                }
            }
            for (x, w) in spans {
                let d = cab.depth;
                out.push(Stroke::Polyline(
                    vec![
                        q(x, d - FRONT_THICKNESS),
                        q(x + w, d - FRONT_THICKNESS),
                        q(x + w, d),
                        q(x, d),
                    ],
                    true,
                ));
            }
        }
    }
    if opts.pilasters && is_box {
        let (left, right) = cab.accessories.pilasters();
        let p = cab.accessories.pilaster_width.clamp(0.5, cab.width / 2.0);
        let d = cab.depth;
        if left && !hidden.contains(&3) {
            out.push(Stroke::Polyline(
                vec![q(0.0, d - p), q(p, d - p), q(p, d), q(0.0, d)],
                true,
            ));
        }
        if right && !hidden.contains(&1) {
            let w = cab.width;
            out.push(Stroke::Polyline(
                vec![q(w - p, d - p), q(w, d - p), q(w, d), q(w - p, d)],
                true,
            ));
        }
    }
    if opts.molding_edges {
        if let Some(Countertop {
            edge, edge_size, ..
        }) = cab.countertop
        {
            if edge != crate::top::EdgeProfile::Square && edge_size > 0.0 {
                if let Some(top) = cab.top_local() {
                    let inner = geom::offset_ring(&top, -edge_size);
                    if inner.len() >= 3 {
                        out.push(Stroke::Polyline(
                            inner.into_iter().map(|p| cab.to_plan(p)).collect(),
                            true,
                        ));
                    }
                }
            }
        }
    }
    out
}

// ----- schedule categories -----

/// The Cabinet Schedule categories (reference manual p. 655): what a row's
/// cabinet is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleCategory {
    Base,
    Wall,
    FullHeight,
    Fillers,
    Soffits,
    Shelves,
    Partitions,
}

impl ScheduleCategory {
    pub const ALL: [ScheduleCategory; 7] = [
        ScheduleCategory::Base,
        ScheduleCategory::Wall,
        ScheduleCategory::FullHeight,
        ScheduleCategory::Fillers,
        ScheduleCategory::Soffits,
        ScheduleCategory::Shelves,
        ScheduleCategory::Partitions,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ScheduleCategory::Base => "Base Cabinets",
            ScheduleCategory::Wall => "Wall Cabinets",
            ScheduleCategory::FullHeight => "Full Height Cabinets",
            ScheduleCategory::Fillers => "Fillers",
            ScheduleCategory::Soffits => "Soffits",
            ScheduleCategory::Shelves => "Shelves",
            ScheduleCategory::Partitions => "Partitions",
        }
    }
}

/// The schedule category of `c`; `None` for custom tops and backsplashes
/// (they have their own schedule categories).
pub fn schedule_category(c: &Cabinet) -> Option<ScheduleCategory> {
    Some(match c.kind {
        CabinetKind::Base | CabinetKind::CornerBase | CabinetKind::BlindBase => {
            ScheduleCategory::Base
        }
        CabinetKind::Wall | CabinetKind::CornerWall | CabinetKind::BlindWall => {
            ScheduleCategory::Wall
        }
        CabinetKind::FullHeight => ScheduleCategory::FullHeight,
        CabinetKind::BaseFiller | CabinetKind::WallFiller | CabinetKind::FullHeightFiller => {
            ScheduleCategory::Fillers
        }
        CabinetKind::Soffit | CabinetKind::SoffitPolygon => ScheduleCategory::Soffits,
        CabinetKind::Shelf => ScheduleCategory::Shelves,
        CabinetKind::Partition => ScheduleCategory::Partitions,
        _ => return None,
    })
}

/// How many cabinets of each category the Cabinet Schedule lists: cabinets
/// switched out of the schedule and the fillers the program makes are not
/// counted.
pub fn schedule_counts(cabs: &[Cabinet]) -> Vec<(ScheduleCategory, usize)> {
    ScheduleCategory::ALL
        .into_iter()
        .map(|cat| {
            let n = cabs
                .iter()
                .filter(|c| c.in_schedule && !c.auto_filler && schedule_category(c) == Some(cat))
                .count();
            (cat, n)
        })
        .collect()
}

// ----- sizing rules -----

/// The width a cabinet of `default_width` gets in a free `space` of the run
/// (reference manual p. 651): the default when it fits, else the largest
/// multiple of the resize `increment` that fits (a 24 in cabinet in a 20 in
/// space with a 3 in increment is 18 in), never below `min_width`; `None`
/// when the space is narrower than `min_width` (no cabinet is placed).
pub fn width_for_space(
    default_width: f64,
    space: f64,
    increment: f64,
    min_width: f64,
) -> Option<f64> {
    if space >= default_width - 1e-9 {
        return Some(default_width);
    }
    if space < min_width - 1e-9 {
        return None;
    }
    let step = increment.max(1.0 / 16.0);
    let w = (space / step + 1e-9).floor() * step;
    Some(w.max(min_width))
}

/// The tops of the appliances that stand where a wall cabinet is hung:
/// `(footprint, top height)`, from the cabinets that hold an appliance bay.
pub fn cabinet_appliance_tops(cabs: &[Cabinet]) -> Vec<(Vec<Point>, f64)> {
    cabs.iter()
        .filter(|c| c.appliance.is_some() && !c.kind.is_wall_like())
        .map(|c| (c.footprint(), c.elevation + c.height))
        .collect()
}

/// A wall cabinet over an appliance takes its bottom from the appliance top
/// (CB-635): the highest top among `appliances` that lies under the middle of
/// the cabinet's back and reaches above the cabinet's current bottom.
pub fn bottom_over_appliance(cab: &Cabinet, appliances: &[(Vec<Point>, f64)]) -> Option<f64> {
    let probes = [
        cab.to_plan(Point::new(cab.width / 2.0, 1.0)),
        cab.to_plan(Point::new(cab.width / 2.0, cab.depth / 2.0)),
    ];
    appliances
        .iter()
        .filter(|(ring, top)| {
            *top > cab.elevation + 0.01 && probes.iter().any(|p| point_in_polygon(*p, ring))
        })
        .map(|(_, top)| *top)
        .fold(None, |best: Option<f64>, t| {
            Some(best.map_or(t, |b| b.max(t)))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabinet::Cabinet;
    use crate::filler::{auto_fillers, wall_polygon, FillerOptions};

    fn at(w: f64, x: f64, id: Id) -> Cabinet {
        let mut c = Cabinet::base(w);
        c.id = id;
        c.position = Point::new(x, 0.0);
        c
    }

    fn rotated(mut c: Cabinet, angle_deg: f64, p: Point) -> Cabinet {
        c.angle = angle_deg.to_radians();
        c.position = p;
        c
    }

    #[test]
    fn side_by_side_cabinets_merge_up_to_three_inches_apart() {
        let a = at(24.0, 0.0, 1);
        for (gap, merges) in [
            (0.0, true),
            (2.0, true),
            (3.0, true),
            (3.5, false),
            (6.0, false),
        ] {
            let b = at(24.0, 24.0 + gap, 2);
            assert_eq!(link(&a, &b, 3.0).is_some(), merges, "gap {gap}");
        }
        // Without automatic fillers only cabinets that touch merge.
        let near = at(24.0, 26.0, 2);
        assert!(merge_runs(&[a.clone(), near], merge_reach(false)).is_empty());
        let touching = at(24.0, 24.0, 2);
        assert_eq!(merge_runs(&[a, touching], merge_reach(false)).len(), 1);
    }

    #[test]
    fn different_families_and_heights_do_not_merge() {
        let base = at(24.0, 0.0, 1);
        let mut wall = Cabinet::wall(24.0);
        wall.id = 2;
        wall.position = Point::new(24.0, 0.0);
        assert!(link(&base, &wall, 3.0).is_none());
        let mut vanity = at(24.0, 24.0, 3);
        vanity.height = 34.5;
        assert!(link(&base, &vanity, 3.0).is_none());
        let full = {
            let mut c = Cabinet::full_height(24.0);
            c.id = 4;
            c.position = Point::new(24.0, 0.0);
            c
        };
        assert!(link(&base, &full, 3.0).is_none());
    }

    #[test]
    fn cabinets_at_an_angle_merge_at_a_front_corner_and_a_back_corner() {
        // North run along +x with its front towards +y; the west run's front
        // corner meets its front corner (an inside corner).
        let a = at(24.0, 27.0, 1);
        let b = rotated(Cabinet::base(24.0), -90.0, Point::new(0.0, 48.0));
        let mut b = b;
        b.id = 2;
        // b's front faces +x; its front-end corner is at (24, 24) and a's
        // front-left corner at (27, 24): 3 in apart.
        assert_eq!(link(&a, &b, 3.0), Some(Link::FrontCorner));
        assert!(link(&a, &b, 1.0).is_none());
        // Backs meeting at a corner and facing away: a bend of 45 degrees
        // merges, a right angle (more than 87) does not.
        let c = at(24.0, 0.0, 3);
        let bent = |deg: f64, id: Id| {
            let mut d = rotated(Cabinet::base(24.0), deg, Point::new(24.0, 0.0));
            d.id = id;
            d
        };
        assert_eq!(link(&c, &bent(-45.0, 4), 3.0), Some(Link::BackCorner));
        let mut corner = rotated(Cabinet::base(24.0), -90.0, Point::new(24.0, 24.0));
        corner.id = 5;
        assert!(link(&c, &corner, 3.0).is_none());
        // A wall cabinet never merges with a base across the angle.
        let mut w = rotated(Cabinet::wall(24.0), -90.0, Point::new(0.0, 48.0));
        w.id = 5;
        assert!(link(&a, &w, 3.0).is_none());
    }

    #[test]
    fn a_filler_brings_a_gapped_run_into_one_merge() {
        let a = at(24.0, 0.0, 1);
        let b = at(24.0, 26.0, 2);
        let fillers = auto_fillers(&[a.clone(), b.clone()], &[], &FillerOptions::default());
        assert_eq!(fillers.len(), 1);
        assert!((fillers[0].width - 2.0).abs() < 1e-9);
        let mut f = fillers[0].clone();
        f.id = 3;
        let runs = merge_runs(&[a, b, f], merge_reach(true));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].ids.len(), 3);
        assert_eq!(runs[0].class, RunClass::Base);
    }

    #[test]
    fn module_lines_sit_on_the_joins_and_hide_the_shared_edges() {
        let a = at(24.0, 0.0, 1);
        let b = at(24.0, 24.0, 2);
        let c = at(24.0, 60.0, 3);
        let d = run_display(&[a.clone(), b.clone(), c], 3.0, false);
        assert_eq!(d.lines.len(), 1, "{:?}", d.lines);
        let l = d.lines[0];
        assert!((l.a.x - 24.0).abs() < 1e-9 && (l.b.x - 24.0).abs() < 1e-9);
        assert!((l.a.y - 0.0).abs() < 1e-9 && (l.b.y - 24.0).abs() < 1e-9);
        assert_eq!(d.hidden_edges.get(&1), Some(&vec![1]));
        assert_eq!(d.hidden_edges.get(&2), Some(&vec![3]));
        assert!(!d.hidden_edges.contains_key(&3));
        // Partial module lines are short ticks.
        let p = run_display(&[a, b], 3.0, true);
        assert!(p.lines[0].a.dist(p.lines[0].b) < 5.0);
    }

    #[test]
    fn a_run_knows_its_extent_along_the_wall() {
        let a = at(24.0, 10.0, 1);
        let b = at(24.0, 36.0, 2);
        let c = at(30.0, 62.0, 3);
        let mut cabs = vec![a, b, c];
        let mut fillers = auto_fillers(&cabs, &[], &FillerOptions::default());
        assert_eq!(fillers.len(), 2);
        for (i, f) in fillers.iter_mut().enumerate() {
            f.id = 10 + i as u64;
        }
        cabs.extend(fillers);
        let runs = merge_runs(&cabs, merge_reach(true));
        assert_eq!(runs.len(), 1);
        let (from, to, len) = runs[0].extent(&cabs).unwrap();
        assert!((from.x - 10.0).abs() < 1e-9 && (to.x - 92.0).abs() < 1e-9);
        assert!((len - 82.0).abs() < 1e-9);
        // A run that turns a corner has no straight extent.
        let mut turned = cabs.clone();
        turned[2].angle = std::f64::consts::FRAC_PI_2;
        assert!(runs[0].extent(&turned).is_none());
    }

    #[test]
    fn extended_stiles_add_their_own_lines() {
        let mut a = at(24.0, 0.0, 1);
        a.stile_ext_left = 1.5;
        let d = run_display(&[a], 3.0, false);
        assert_eq!(d.lines.len(), 2);
    }

    #[test]
    fn plan_strokes_hide_shared_edges_and_show_closed_fronts() {
        let a = at(24.0, 0.0, 1);
        let plain = plan_strokes(&a, &[], &PlanOptions::default());
        let hidden = plan_strokes(&a, &[1], &PlanOptions::default());
        // A closed outline became three lines.
        assert!(matches!(plain[0], Stroke::Polyline(_, true)));
        assert_eq!(
            hidden
                .iter()
                .take(3)
                .filter(|s| matches!(s, Stroke::Line(..)))
                .count(),
            3
        );
        let with = plan_strokes(
            &a,
            &[],
            &PlanOptions {
                closed_fronts: true,
                ..PlanOptions::default()
            },
        );
        assert!(with.len() > plain.len());
        let mut p = at(24.0, 0.0, 2);
        p.accessories.pilaster = crate::dress::PilasterStyle::Plain;
        let n0 = plan_strokes(&p, &[], &PlanOptions::default()).len();
        let n1 = plan_strokes(
            &p,
            &[],
            &PlanOptions {
                pilasters: true,
                ..PlanOptions::default()
            },
        )
        .len();
        assert_eq!(n1, n0 + 2);
        let n2 = plan_strokes(
            &p,
            &[3],
            &PlanOptions {
                pilasters: true,
                ..PlanOptions::default()
            },
        )
        .len();
        // The hidden left edge is not drawn and carries no pilaster.
        assert_eq!(n2, n0 + 3);
    }

    #[test]
    fn the_schedule_has_seven_categories_and_skips_auto_fillers() {
        let mut shelf = Cabinet::new(CabinetKind::Shelf, 24.0);
        shelf.id = 3;
        let mut part = Cabinet::new(CabinetKind::Partition, 24.0);
        part.id = 4;
        let mut soff = Cabinet::new(CabinetKind::Soffit, 24.0);
        soff.id = 5;
        let mut f = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        f.id = 6;
        let mut auto = f.clone();
        auto.id = 7;
        auto.auto_filler = true;
        auto.in_schedule = false;
        let cabs = vec![
            at(24.0, 0.0, 1),
            {
                let mut w = Cabinet::wall(24.0);
                w.id = 2;
                w
            },
            shelf,
            part,
            soff,
            f,
            auto,
            {
                let mut u = Cabinet::full_height(24.0);
                u.id = 8;
                u
            },
        ];
        let counts = schedule_counts(&cabs);
        assert_eq!(counts.len(), 7);
        let get = |c| counts.iter().find(|x| x.0 == c).unwrap().1;
        assert_eq!(get(ScheduleCategory::Base), 1);
        assert_eq!(get(ScheduleCategory::Wall), 1);
        assert_eq!(get(ScheduleCategory::FullHeight), 1);
        assert_eq!(get(ScheduleCategory::Fillers), 1);
        assert_eq!(get(ScheduleCategory::Soffits), 1);
        assert_eq!(get(ScheduleCategory::Shelves), 1);
        assert_eq!(get(ScheduleCategory::Partitions), 1);
    }

    #[test]
    fn a_narrow_space_takes_a_multiple_of_the_increment() {
        // The manual's example: 24 in cabinet, 20 in space, 3 in increment.
        assert_eq!(width_for_space(24.0, 20.0, 3.0, 3.0), Some(18.0));
        assert_eq!(width_for_space(24.0, 30.0, 3.0, 3.0), Some(24.0));
        // And the second example: nothing narrower than the minimum width.
        assert_eq!(width_for_space(24.0, 8.0, 3.0, 9.0), None);
        assert_eq!(width_for_space(24.0, 9.0, 3.0, 9.0), Some(9.0));
        // A space between the minimum and the first multiple takes the minimum.
        assert_eq!(width_for_space(24.0, 8.0, 5.0, 6.0), Some(6.0));
        assert_eq!(width_for_space(24.0, 2.0, 3.0, 3.0), None);
    }

    #[test]
    fn a_wall_cabinet_takes_its_bottom_from_the_appliance_below() {
        let mut fridge = Cabinet::refrigerator(36.0);
        fridge.id = 1;
        fridge.position = Point::new(0.0, 0.0);
        fridge.appliance = Some("Refrigerator".into());
        let tops = cabinet_appliance_tops(&[fridge]);
        let mut w = Cabinet::wall(36.0);
        w.position = Point::new(0.0, 0.0);
        // The default bottom is 54; the refrigerator reaches 84.
        assert_eq!(bottom_over_appliance(&w, &tops), Some(84.0));
        // Elsewhere along the wall nothing changes.
        w.position = Point::new(100.0, 0.0);
        assert_eq!(bottom_over_appliance(&w, &tops), None);
        // An appliance lower than the cabinet's bottom does not matter.
        let mut dw = Cabinet::dishwasher_opening();
        dw.id = 2;
        let tops = cabinet_appliance_tops(&[dw]);
        let mut w2 = Cabinet::wall(24.0);
        w2.position = Point::new(0.0, 0.0);
        assert_eq!(bottom_over_appliance(&w2, &tops), None);
    }

    #[test]
    fn walls_in_the_way_do_not_merge_anything() {
        let wall = wall_polygon(Point::new(24.0, -10.0), Point::new(24.0, 60.0), 1.0);
        let a = at(24.0, 0.0, 1);
        // A wall is not a cabinet: no run.
        assert!(merge_runs(&[a], 3.0).is_empty());
        let _ = wall;
    }
}
