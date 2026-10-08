//! Wall framing: plates, studs, and the king/trimmer/header/sill/cripple
//! assembly around each opening.

use crate::defaults::FramingDefaults;
use crate::lumber::{Lumber, TWO_BY_THICKNESS};
use crate::member::{add, scale, Member, MemberKind, Transform3, Vec3};
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
    let mut lefts: Vec<f64> = (0..)
        .map(|k| f64::from(k) * step)
        .take_while(|&g| g <= end_left - t + EPS)
        .filter(|&g| !in_zone(g))
        .collect();
    if end_left > EPS && !in_zone(end_left) {
        lefts.push(end_left);
    }
    for left in lefts {
        out.push(f.vertical(MemberKind::Stud, lumber, left, y_bot, stud_len));
    }

    for o in openings {
        frame_opening(&f, o, lumber, d, (y_bot, y_top), (len, stud_len), &mut out);
    }
    out
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
}
