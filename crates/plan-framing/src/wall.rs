//! Wall framing: plates, studs, and the king/trimmer/header/sill/cripple
//! assembly around each opening.

use crate::defaults::FramingDefaults;
use crate::lumber::{Lumber, TWO_BY_THICKNESS};
use crate::member::{add, scale, Member, MemberKind, Transform3, Vec3};
use plan_core::geometry::project_on_segment;
use plan_core::{Opening, Wall};

/// Thickness of every plate, header ply and sill (a flat 2x).
const PLATE: f64 = TWO_BY_THICKNESS;
const EPS: f64 = 1e-6;
const UP: Vec3 = [0.0, 1.0, 0.0];

/// A wall's local frame in 3D: `s` runs along the wall from its start.
struct WallFrame {
    start: Vec3,
    /// Unit direction start -> end.
    dir: Vec3,
    /// Unit direction across the wall thickness.
    thick: Vec3,
    wall_id: u64,
}

impl WallFrame {
    fn new(wall: &Wall) -> Self {
        let (d, n) = (wall.direction(), wall.normal());
        Self {
            start: [wall.start.x, 0.0, -wall.start.y],
            dir: [d.x, 0.0, -d.y],
            thick: [n.x, 0.0, -n.y],
            wall_id: wall.id,
        }
    }

    fn point(&self, s: f64, y: f64) -> Vec3 {
        let p = add(self.start, scale(self.dir, s));
        [p[0], y, p[2]]
    }

    fn member(&self, kind: MemberKind, lumber: Lumber, length: f64, t: Transform3) -> Member {
        Member::new(kind, lumber, length, t, Some(self.wall_id))
    }

    /// Plate or sill: runs along the wall, depth across the wall, thickness vertical.
    fn flat(&self, kind: MemberKind, lumber: Lumber, s0: f64, length: f64, y_mid: f64) -> Member {
        let t = Transform3 {
            origin: self.point(s0, y_mid),
            axis_x: self.dir,
            axis_y: self.thick,
        };
        self.member(kind, lumber, length, t)
    }

    /// Vertical stud-like member whose left edge sits at `s_left`.
    fn vertical(&self, kind: MemberKind, lumber: Lumber, s_left: f64, y0: f64, len: f64) -> Member {
        let t = Transform3 {
            origin: self.point(s_left + lumber.thickness / 2.0, y0),
            axis_x: UP,
            axis_y: self.thick,
        };
        self.member(kind, lumber, len, t)
    }

    /// Header ply: runs along the wall, depth vertical, `offset` across the wall.
    fn on_edge(&self, lumber: Lumber, s0: f64, length: f64, y_mid: f64, offset: f64) -> Member {
        let origin = add(self.point(s0, y_mid), scale(self.thick, offset));
        let t = Transform3 {
            origin,
            axis_x: self.dir,
            axis_y: UP,
        };
        self.member(MemberKind::Header, lumber, length, t)
    }
}

/// A partition that butts into the side of a wall (a T intersection).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tee {
    /// Where the partition's centerline meets the wall, along the wall from its start.
    pub offset: f64,
    /// Thickness of the partition.
    pub thickness: f64,
}

/// How a wall meets the walls around it, for the corner and tee studs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WallJoints {
    /// Another wall ends at the wall's start (not as a straight continuation).
    pub start_corner: bool,
    /// Another wall ends at the wall's end.
    pub end_corner: bool,
    pub tees: Vec<Tee>,
}

impl WallJoints {
    /// The joints of `wall` among `walls` (the wall itself is skipped).
    /// Walls that continue it in a straight line make neither a corner nor a tee.
    pub fn of(wall: &Wall, walls: &[Wall]) -> Self {
        let mut j = WallJoints::default();
        let len = wall.length();
        if len < 1.0 {
            return j;
        }
        let dir = wall.direction();
        for w in walls.iter().filter(|w| w.id != wall.id && w.length() > 1.0) {
            let tol = (wall.thickness.max(w.thickness)).max(1.0);
            let straight = w.direction().cross(dir).abs() < 0.05;
            for end in [w.start, w.end] {
                let (t, on) = project_on_segment(end, wall.start, wall.end);
                if end.dist(on) > tol * 0.75 {
                    continue;
                }
                let along = t * len;
                if along <= tol {
                    j.start_corner |= !straight;
                } else if along >= len - tol {
                    j.end_corner |= !straight;
                } else if !straight {
                    j.tees.push(Tee {
                        offset: along,
                        thickness: w.thickness,
                    });
                }
            }
        }
        j.tees.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        j.tees.dedup_by(|a, b| (a.offset - b.offset).abs() < 1.0);
        j
    }
}

/// Frame one wall the way Chief's "Build Framing" does.
///
/// `floor_elevation` is the bottom of the wall (underside of the bottom
/// plate); opening `sill_height` and `height` are measured from it.
/// `openings` must be the openings hosted by `wall`. The framing centreline
/// sits on the wall centreline. Stud positions are measured from the wall
/// start to each stud's near edge, so studs sit at 0, 16, 32, ... plus an end
/// stud flush with the far end.
///
/// Studs inside an opening's king-to-king zone are dropped. Adjacent openings
/// whose zones overlap are not combined, and corner/intersection framing is
/// not generated.
pub fn frame_wall(
    wall: &Wall,
    openings: &[&Opening],
    floor_elevation: f64,
    d: &FramingDefaults,
) -> Vec<Member> {
    frame_wall_grid(wall, openings, floor_elevation, d, None)
}

/// [`frame_wall`] with the stud layout grid anchored at `grid_origin`, the
/// distance along the wall from its start to the near edge of a grid stud
/// (a Framing Reference Marker). Grid studs run both ways from it; the wall's
/// first and last studs are always present.
pub(crate) fn frame_wall_grid(
    wall: &Wall,
    openings: &[&Opening],
    floor_elevation: f64,
    d: &FramingDefaults,
    grid_origin: Option<f64>,
) -> Vec<Member> {
    frame_wall_joined(
        wall,
        openings,
        floor_elevation,
        d,
        grid_origin,
        &WallJoints::default(),
    )
}

/// [`frame_wall`] with the stud grid anchored at `grid_origin` and the wall's
/// `joints` known: with `d.corner_studs` extra studs beside the end stud of a
/// corner end, with `d.tee_studs` backing studs on each side of every
/// partition that butts in, and with `d.wall_blocking` a row of horizontal
/// blocking between the studs every `d.wall_blocking_spacing` inches up the
/// wall. Studs the extras land on are dropped, as are extras inside an
/// opening's king-to-king zone.
pub fn frame_wall_joined(
    wall: &Wall,
    openings: &[&Opening],
    floor_elevation: f64,
    d: &FramingDefaults,
    grid_origin: Option<f64>,
    joints: &WallJoints,
) -> Vec<Member> {
    let f = WallFrame::new(wall);
    let lumber = d.stud_size_for(wall);
    let t = lumber.thickness;
    let len = wall.length();
    let plates_bottom = f64::from(d.bottom_plates) * PLATE;
    let plates_top = f64::from(d.top_plates) * PLATE;
    let stud_len = wall.height - plates_bottom - plates_top;
    let y_bot = floor_elevation + plates_bottom;
    let y_top = floor_elevation + wall.height - plates_top;

    let mut out = Vec::new();
    for i in 0..d.bottom_plates {
        let y = floor_elevation + PLATE * (f64::from(i) + 0.5);
        out.push(f.flat(MemberKind::BottomPlate, lumber, 0.0, len, y));
    }
    for i in 0..d.top_plates {
        let y = y_top + PLATE * (f64::from(i) + 0.5);
        out.push(f.flat(MemberKind::TopPlate, lumber, 0.0, len, y));
    }
    if stud_len <= EPS || len <= EPS {
        return out;
    }

    // King-to-king zone of every opening, as (start, end) along the wall.
    let side = f64::from(d.trimmers + d.king_studs) * t;
    let zones: Vec<(f64, f64)> = openings
        .iter()
        .map(|o| (o.start_offset() - side, o.end_offset() + side))
        .collect();
    let in_zone = |left: f64| {
        zones
            .iter()
            .any(|&(a, b)| left < b - EPS && left + t > a + EPS)
    };

    // Common studs on the layout grid, then the end stud.
    let step = d.stud_spacing.max(t);
    let end_left = (len - t).max(0.0);
    let origin = grid_origin.unwrap_or(0.0);
    let first_k = ((-origin) / step - EPS).ceil() as i64;
    let mut lefts: Vec<f64> = (first_k..)
        .map(|k| origin + k as f64 * step)
        .take_while(|&g| g <= end_left - t + EPS)
        .filter(|&g| !in_zone(g))
        .collect();
    if grid_origin.is_some() && lefts.first().is_none_or(|&g| g > EPS) && !in_zone(0.0) {
        lefts.insert(0, 0.0);
    }
    if end_left > EPS && !in_zone(end_left) {
        lefts.push(end_left);
    }
    // Corner studs beside a corner end's end stud, backing studs around tees.
    let mut extras: Vec<(f64, MemberKind)> = Vec::new();
    for k in 0..d.corner_studs {
        let off = (f64::from(k) + 1.0) * t;
        if joints.start_corner {
            extras.push((off, MemberKind::CornerStud));
        }
        if joints.end_corner {
            extras.push((end_left - off, MemberKind::CornerStud));
        }
    }
    for tee in &joints.tees {
        for k in 0..d.tee_studs {
            let off = f64::from(k) * t;
            extras.push((
                tee.offset - tee.thickness / 2.0 - t - off,
                MemberKind::TeeStud,
            ));
            extras.push((tee.offset + tee.thickness / 2.0 + off, MemberKind::TeeStud));
        }
    }
    extras.retain(|&(left, _)| left >= -EPS && left + t <= len + EPS && !in_zone(left));
    let overlaps = |a: f64, b: f64| (a - b).abs() < t - EPS;
    let is_end = |left: f64| left <= EPS || (left - end_left).abs() <= EPS;
    lefts.retain(|&l| is_end(l) || !extras.iter().any(|&(e, _)| overlaps(l, e)));
    let mut studs: Vec<(f64, MemberKind)> =
        lefts.into_iter().map(|l| (l, MemberKind::Stud)).collect();
    // Extras that fall on each other (tees close to a corner) keep the first.
    for e in extras {
        if !studs.iter().any(|&(l, _)| overlaps(l, e.0)) {
            studs.push(e);
        }
    }
    studs.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(left, kind) in &studs {
        out.push(f.vertical(kind, lumber, left, y_bot, stud_len));
    }

    for o in openings {
        frame_opening(&f, o, lumber, d, (y_bot, y_top), (len, stud_len), &mut out);
    }

    if d.wall_blocking && d.wall_blocking_spacing > 1.0 {
        blocking(
            &f,
            wall,
            lumber,
            d,
            (floor_elevation, y_top),
            &zones,
            &mut out,
        );
    }
    out
}

/// Rows of horizontal blocking between the full-height studs, at multiples of
/// `d.wall_blocking_spacing` above the bottom of the wall that clear the top
/// plate. Gaps over an opening are left out.
fn blocking(
    f: &WallFrame,
    wall: &Wall,
    lumber: Lumber,
    d: &FramingDefaults,
    (y_base, y_top): (f64, f64),
    zones: &[(f64, f64)],
    out: &mut Vec<Member>,
) {
    let t = lumber.thickness;
    let start = f.start;
    // Left edges along the wall of the full-height verticals.
    let mut lefts: Vec<f64> = out
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MemberKind::Stud
                    | MemberKind::CornerStud
                    | MemberKind::TeeStud
                    | MemberKind::KingStud
            )
        })
        .map(|m| {
            let rel = [
                m.transform.origin[0] - start[0],
                0.0,
                m.transform.origin[2] - start[2],
            ];
            crate::member::dot(rel, f.dir) - t / 2.0
        })
        .collect();
    lefts.sort_by(f64::total_cmp);
    lefts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let gaps: Vec<(f64, f64)> = lefts
        .windows(2)
        .map(|w| (w[0] + t, w[1]))
        .filter(|&(a, b)| b - a > 1.0)
        .filter(|&(a, b)| {
            let mid = (a + b) * 0.5;
            !zones.iter().any(|&(za, zb)| mid > za && mid < zb)
        })
        .collect();
    let mut k = 1.0;
    loop {
        let h = k * d.wall_blocking_spacing;
        let y = y_base + h;
        // The flat 2x must fit under the top plate.
        if y + PLATE / 2.0 > y_top - EPS || h > wall.height {
            break;
        }
        for &(a, b) in &gaps {
            out.push(f.flat(MemberKind::Blocking, lumber, a, b - a, y));
        }
        k += 1.0;
    }
}

/// Kings, trimmers, header, cripples and (for windows) the sill of one opening.
fn frame_opening(
    f: &WallFrame,
    o: &Opening,
    lumber: Lumber,
    d: &FramingDefaults,
    (y_bot, y_top): (f64, f64),
    (wall_len, stud_len): (f64, f64),
    out: &mut Vec<Member>,
) {
    let t = lumber.thickness;
    let (a, b) = (o.start_offset(), o.end_offset());
    let floor = y_bot - PLATE * f64::from(d.bottom_plates);
    let open_bot = floor + o.sill_height;
    let open_top = open_bot + o.height;
    // Vertical members that would hang off either wall end are dropped.
    let push_if_fits = |out: &mut Vec<Member>, kind: MemberKind, left: f64, y0: f64, len: f64| {
        if left >= -EPS && left + t <= wall_len + EPS && len > 0.5 {
            out.push(f.vertical(kind, lumber, left, y0, len));
        }
    };

    let (nt, nk) = (d.trimmers, d.king_studs);
    for i in 0..nk {
        let off = (f64::from(nt) + f64::from(i)) * t;
        push_if_fits(out, MemberKind::KingStud, a - off - t, y_bot, stud_len);
        push_if_fits(out, MemberKind::KingStud, b + off, y_bot, stud_len);
    }
    for i in 0..nt {
        let off = f64::from(i) * t;
        let trimmer_len = open_top - y_bot;
        push_if_fits(
            out,
            MemberKind::TrimmerStud,
            a - off - t,
            y_bot,
            trimmer_len,
        );
        push_if_fits(out, MemberKind::TrimmerStud, b + off, y_bot, trimmer_len);
    }

    // Header between the kings, resting on the trimmers.
    let h_start = a - f64::from(nt) * t;
    let h_len = o.width + 2.0 * f64::from(nt) * t;
    let mut header_top = open_top;
    if d.header_plies > 0 {
        // A header that would poke through the top plates is shortened in depth.
        let depth = d.header_depth_for(o.width).min(y_top - open_top);
        if depth > 0.5 {
            header_top = open_top + depth;
            let ply = Lumber::two_by(depth);
            let plies = f64::from(d.header_plies);
            for p in 0..d.header_plies {
                let offset = (f64::from(p) - (plies - 1.0) / 2.0) * PLATE;
                let y_mid = open_top + depth / 2.0;
                out.push(f.on_edge(ply, h_start, h_len, y_mid, offset));
            }
        }
    }

    // Cripples above the header: one over each trimmer, the rest on the grid.
    let step = d.cripple_spacing.max(t);
    let gap = y_top - header_top;
    if gap > 0.5 {
        let (lo, hi) = (h_start, h_start + h_len - t);
        let mut lefts = vec![lo];
        lefts.extend(grid(step, lo + t, hi - t));
        if hi > lo + EPS {
            lefts.push(hi);
        }
        for left in lefts {
            push_if_fits(out, MemberKind::CrippleStud, left, header_top, gap);
        }
    }

    // Windows: sill plate and cripples below it.
    if o.sill_height > 0.0 && open_bot - PLATE >= y_bot - EPS {
        let sill_bot = open_bot - PLATE;
        out.push(f.flat(MemberKind::Sill, lumber, a, o.width, sill_bot + PLATE / 2.0));
        let below = sill_bot - y_bot;
        if below > 0.5 {
            for left in grid(step, a, b - t) {
                push_if_fits(out, MemberKind::CrippleStud, left, y_bot, below);
            }
        }
    }
}

/// Multiples of `step` within `[lo, hi]` (left edges that fit).
fn grid(step: f64, lo: f64, hi: f64) -> impl Iterator<Item = f64> {
    let first = (lo / step - EPS).ceil().max(0.0) as i64;
    (first..)
        .map(move |k| k as f64 * step)
        .take_while(move |&g| g <= hi + EPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{Point, WallKind};

    fn wall() -> Wall {
        Wall {
            id: 7,
            start: Point::new(0.0, 0.0),
            end: Point::new(120.0, 0.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        }
    }

    fn of(m: &[Member], k: MemberKind) -> Vec<&Member> {
        m.iter().filter(|x| x.kind == k).collect()
    }

    /// Left edge along the wall (plan x for this wall) of a vertical member.
    fn left_edge(m: &Member) -> f64 {
        m.transform.origin[0] - m.lumber.thickness / 2.0
    }

    #[test]
    fn plain_wall() {
        let m = frame_wall(&wall(), &[], 0.0, &FramingDefaults::default());
        assert_eq!(of(&m, MemberKind::TopPlate).len(), 2);
        assert_eq!(of(&m, MemberKind::BottomPlate).len(), 1);
        for p in m
            .iter()
            .filter(|x| matches!(x.kind, MemberKind::TopPlate | MemberKind::BottomPlate))
        {
            assert_eq!(p.length, 120.0);
            assert_eq!(p.lumber.nominal_name(), "2x6");
        }
        let studs = of(&m, MemberKind::Stud);
        let lefts: Vec<f64> = studs.iter().map(|s| left_edge(s)).collect();
        assert_eq!(
            lefts,
            [0.0, 16.0, 32.0, 48.0, 64.0, 80.0, 96.0, 112.0, 118.5]
        );
        for s in &studs {
            assert!((s.length - 104.625).abs() < 1e-9);
            assert_eq!(s.label, "2x6 x 104 5/8");
            assert_eq!(s.transform.origin[1], 1.5);
        }
        assert_eq!(m.len(), 3 + 9);
        // Top plates sit under the wall top.
        let top: Vec<f64> = of(&m, MemberKind::TopPlate)
            .iter()
            .map(|p| p.transform.origin[1])
            .collect();
        assert_eq!(top, [106.125 + 0.75, 106.125 + 2.25]);
    }

    #[test]
    fn door_assembly() {
        let w = wall();
        let door = Opening::default_door(1, w.id, 60.0);
        let m = frame_wall(&w, &[&door], 0.0, &FramingDefaults::default());
        assert_eq!(of(&m, MemberKind::KingStud).len(), 2);
        let trimmers = of(&m, MemberKind::TrimmerStud);
        assert_eq!(trimmers.len(), 2);
        assert!((trimmers[0].length - 78.5).abs() < 1e-9);
        let headers = of(&m, MemberKind::Header);
        assert_eq!(headers.len(), 2);
        for h in &headers {
            assert_eq!(h.length, 39.0);
            assert_eq!(h.lumber.nominal_name(), "2x6");
        }
        // Plies stack across the wall, centred on the centreline.
        let z: Vec<f64> = headers.iter().map(|h| h.transform.origin[2]).collect();
        assert!((z[0] + z[1]).abs() < 1e-9 && ((z[0] - z[1]).abs() - 1.5).abs() < 1e-9);
        let cripples = of(&m, MemberKind::CrippleStud);
        assert_eq!(cripples.len(), 4);
        assert!(cripples
            .iter()
            .all(|c| (c.transform.origin[1] - 85.5).abs() < 1e-9));
        assert!(of(&m, MemberKind::Sill).is_empty());
        // Common studs avoid the opening zone [39, 81].
        for s in of(&m, MemberKind::Stud) {
            let l = left_edge(s);
            assert!(l + 1.5 <= 39.0 + 1e-9 || l >= 81.0 - 1e-9, "stud at {l}");
        }
        // No two vertical members overlap.
        let mut v: Vec<f64> = m
            .iter()
            .filter(|x| x.transform.axis_x == UP && x.kind != MemberKind::CrippleStud)
            .map(left_edge)
            .collect();
        v.sort_by(f64::total_cmp);
        assert!(v.windows(2).all(|p| p[1] - p[0] >= 1.5 - 1e-9));
    }

    #[test]
    fn window_has_sill_and_lower_cripples() {
        let w = wall();
        let win = Opening::default_window(2, w.id, 60.0);
        let m = frame_wall(&w, &[&win], 0.0, &FramingDefaults::default());
        let sills = of(&m, MemberKind::Sill);
        assert_eq!(sills.len(), 1);
        assert_eq!(sills[0].length, 36.0);
        assert!((sills[0].transform.origin[1] - 23.25).abs() < 1e-9);
        let below: Vec<_> = of(&m, MemberKind::CrippleStud)
            .into_iter()
            .filter(|c| c.transform.origin[1] < 24.0)
            .collect();
        assert_eq!(below.len(), 2);
        assert!(below.iter().all(|c| (c.length - 21.0).abs() < 1e-9));
        // Window head at 24 + 60 = 84 -> above-header cripples start at 89.5.
        assert!(of(&m, MemberKind::CrippleStud)
            .iter()
            .any(|c| (c.transform.origin[1] - 89.5).abs() < 1e-9));
    }

    #[test]
    fn wide_opening_gets_deeper_header() {
        let d = FramingDefaults::default();
        assert_eq!(d.header_depth_for(36.0), 5.5);
        assert_eq!(d.header_depth_for(60.0), 7.25);
        assert_eq!(d.header_depth_for(96.0), 11.25);
    }

    #[test]
    fn two_by_four_wall_keeps_two_by_four() {
        let mut w = wall();
        w.thickness = 4.5;
        let m = frame_wall(&w, &[], 0.0, &FramingDefaults::default());
        assert!(m.iter().all(|x| x.lumber.nominal_name() == "2x4"));
    }

    // ----- header table, cripples, corner and tee backing, blocking -----

    fn wide_window(w: &Wall, center: f64, width: f64) -> Opening {
        let mut o = Opening::default_window(2, w.id, center);
        o.width = width;
        o
    }

    #[test]
    fn a_six_foot_opening_gets_a_two_by_ten_header_from_the_table() {
        let d = FramingDefaults::default();
        // The table: 2x6 to 4', 2x8 to 5', 2x10 to 6', 2x12 beyond.
        let names: Vec<_> = [36.0, 48.0, 60.0, 72.0, 73.0, 120.0]
            .iter()
            .map(|w| d.header_lumber_for(*w).nominal_name())
            .collect();
        assert_eq!(names, ["2x6", "2x6", "2x8", "2x10", "2x12", "2x12"]);
        let w = wall();
        let win = wide_window(&w, 120.0, 72.0);
        let m = frame_wall(&w, &[&win], 0.0, &d);
        let headers = of(&m, MemberKind::Header);
        assert_eq!(headers.len(), 2);
        for h in &headers {
            assert_eq!(h.lumber.nominal_name(), "2x10");
            assert_eq!(h.length, 75.0, "the opening plus a trimmer each side");
            assert!((h.lumber.depth - 9.25).abs() < 1e-9);
        }
        // The header sits on top of the opening (sill 24" + 60" high).
        let y = headers[0].transform.origin[1];
        assert!((y - (84.0 + 9.25 / 2.0)).abs() < 1e-9);
        // A fixed header depth overrides the table.
        let fixed = FramingDefaults {
            header_depth: 5.5,
            ..FramingDefaults::default()
        };
        assert_eq!(fixed.header_depth_for(120.0), 5.5);
        // The table survives a round trip and an older file without it.
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<FramingDefaults>(&json).unwrap(), d);
        let old: FramingDefaults = serde_json::from_str("{\"stud_spacing\": 24.0}").unwrap();
        assert_eq!(old.stud_spacing, 24.0);
        assert_eq!(old.header_table, default_header_table());
    }

    use crate::defaults::default_header_table;

    #[test]
    fn a_wide_window_has_kings_trimmers_sill_and_the_right_cripples() {
        let d = FramingDefaults::default();
        let mut w = wall();
        w.end = Point::new(240.0, 0.0);
        let win = wide_window(&w, 120.0, 72.0);
        let m = frame_wall(&w, &[&win], 0.0, &d);
        assert_eq!(of(&m, MemberKind::KingStud).len(), 2);
        assert_eq!(of(&m, MemberKind::TrimmerStud).len(), 2);
        let sills = of(&m, MemberKind::Sill);
        assert_eq!(sills.len(), 1);
        assert_eq!(sills[0].length, 72.0);
        // Above the header: one over each trimmer plus the 16" grid between
        // them (96, 112, 128, 144). Below the sill: the grid under the opening.
        let cripples = of(&m, MemberKind::CrippleStud);
        let above: Vec<_> = cripples
            .iter()
            .filter(|c| c.transform.origin[1] > 84.0)
            .collect();
        let below: Vec<_> = cripples
            .iter()
            .filter(|c| c.transform.origin[1] < 24.0)
            .collect();
        assert_eq!(above.len(), 2 + 4);
        assert_eq!(below.len(), 4);
        assert_eq!(cripples.len(), 10);
        // Above cripples fill the gap between the header (9 1/4") and the top plates.
        let gap = 106.125 - (84.0 + 9.25);
        assert!(above.iter().all(|c| (c.length - gap).abs() < 1e-9));
    }

    fn neighbours(w: &Wall) -> Vec<Wall> {
        // A 4 1/2" partition butting into the middle of `w`.
        let mut partition = w.clone();
        partition.id = 20;
        partition.start = Point::new(120.0, 0.0);
        partition.end = Point::new(120.0, 100.0);
        partition.thickness = 4.5;
        vec![partition]
    }

    #[test]
    fn joints_find_corners_and_tees_but_not_straight_continuations() {
        let mut w = wall();
        w.end = Point::new(240.0, 0.0);
        w.start = Point::new(0.0, 0.0);
        let mut rise = w.clone();
        rise.id = 30;
        rise.start = Point::new(240.0, 0.0);
        rise.end = Point::new(240.0, 100.0);
        let mut straight = w.clone();
        straight.id = 31;
        straight.start = Point::new(-100.0, 0.0);
        straight.end = Point::new(0.0, 0.0);
        let mut all = neighbours(&w);
        all.extend([rise, straight]);
        let j = WallJoints::of(&w, &all);
        assert!(j.end_corner, "a wall rises from the far end");
        assert!(!j.start_corner, "the wall before it continues straight");
        assert_eq!(j.tees.len(), 1);
        assert!((j.tees[0].offset - 120.0).abs() < 1e-9);
        assert_eq!(j.tees[0].thickness, 4.5);
        assert_eq!(WallJoints::of(&w, &[]), WallJoints::default());
    }

    #[test]
    fn corner_and_tee_studs_back_the_joints() {
        let mut w = wall();
        w.end = Point::new(240.0, 0.0);
        let d = FramingDefaults::house();
        let joints = WallJoints {
            start_corner: false,
            end_corner: true,
            tees: vec![Tee {
                offset: 120.0,
                thickness: 4.5,
            }],
        };
        let m = frame_wall_joined(&w, &[], 0.0, &d, None, &joints);
        let corner = of(&m, MemberKind::CornerStud);
        assert_eq!(corner.len(), 1);
        // Beside the end stud (which sits flush at 238.5): 237 .. 238.5 - 1.5.
        assert!((left_edge(corner[0]) - 237.0).abs() < 1e-9);
        let tees = of(&m, MemberKind::TeeStud);
        assert_eq!(tees.len(), 2);
        let mut lefts: Vec<f64> = tees.iter().map(|t| left_edge(t)).collect();
        lefts.sort_by(f64::total_cmp);
        // Flush with the partition's faces: 120 +- 2.25.
        assert_eq!(lefts, [117.75 - 1.5, 122.25]);
        // Full-height studs, none overlapping another vertical.
        assert!(tees.iter().all(|t| (t.length - 104.625).abs() < 1e-9));
        let mut v: Vec<f64> = m
            .iter()
            .filter(|x| {
                matches!(
                    x.kind,
                    MemberKind::Stud | MemberKind::CornerStud | MemberKind::TeeStud
                )
            })
            .map(left_edge)
            .collect();
        v.sort_by(f64::total_cmp);
        assert!(v.windows(2).all(|p| p[1] - p[0] >= 1.5 - 1e-9), "{v:?}");
        // Without joints (or with the plain defaults) there is no backing.
        let plain = frame_wall_joined(&w, &[], 0.0, &FramingDefaults::default(), None, &joints);
        assert!(of(&plain, MemberKind::CornerStud).is_empty());
        let none = frame_wall_joined(&w, &[], 0.0, &d, None, &WallJoints::default());
        assert!(of(&none, MemberKind::TeeStud).is_empty());
    }

    #[test]
    fn blocking_rows_sit_at_48_inches_between_the_studs() {
        let mut w = wall();
        w.end = Point::new(240.0, 0.0);
        let d = FramingDefaults::house();
        let door = Opening::default_door(1, w.id, 120.0);
        let m = frame_wall_joined(&w, &[&door], 0.0, &d, None, &WallJoints::default());
        let blocks = of(&m, MemberKind::Blocking);
        assert!(!blocks.is_empty());
        let mut rows: Vec<f64> = blocks.iter().map(|b| b.transform.origin[1]).collect();
        rows.sort_by(f64::total_cmp);
        rows.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        // 48" and 96" above the bottom of the wall; the third (144") is over the top.
        assert_eq!(rows, [48.0, 96.0]);
        // Never across the door opening (102 .. 138, 99 .. 141 with the kings).
        for b in &blocks {
            let s = b.transform.origin[0];
            assert!(
                s + b.length <= 99.0 + 1e-9 || s >= 141.0 - 1e-9,
                "block at {s}"
            );
            assert!(b.length > 1.0 && b.length <= 16.0, "{}", b.length);
        }
        // Rows are flat 2x6 pieces; blocking can be turned off.
        assert!(blocks.iter().all(|b| b.lumber.nominal_name() == "2x6"));
        let off = FramingDefaults {
            wall_blocking: false,
            ..d.clone()
        };
        let m = frame_wall_joined(&w, &[&door], 0.0, &off, None, &WallJoints::default());
        assert!(of(&m, MemberKind::Blocking).is_empty());
        // A closer spacing adds rows.
        let tight = FramingDefaults {
            wall_blocking_spacing: 24.0,
            ..d
        };
        let m = frame_wall_joined(&w, &[&door], 0.0, &tight, None, &WallJoints::default());
        let mut rows: Vec<f64> = of(&m, MemberKind::Blocking)
            .iter()
            .map(|b| b.transform.origin[1])
            .collect();
        rows.sort_by(f64::total_cmp);
        rows.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(rows, [24.0, 48.0, 72.0, 96.0]);
    }

    #[test]
    fn double_top_plates_and_single_bottom_plate_by_default() {
        let m = frame_wall(&wall(), &[], 0.0, &FramingDefaults::default());
        assert_eq!(of(&m, MemberKind::TopPlate).len(), 2);
        assert_eq!(of(&m, MemberKind::BottomPlate).len(), 1);
        // Studs are cut to fit between: wall - 3 plates.
        assert!(of(&m, MemberKind::Stud)
            .iter()
            .all(|s| (s.length - 104.625).abs() < 1e-9));
    }
}
