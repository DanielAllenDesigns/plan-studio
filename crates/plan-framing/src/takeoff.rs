//! Quantity takeoff for a set of framing members.

use crate::member::{Member, MemberKind};
use serde::{Deserialize, Serialize};

/// One line of the framing schedule: pieces of one member type, size and cut
/// length.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutLine {
    /// Member type, e.g. `"stud"`, `"header"`, `"rafter"`.
    pub member: String,
    /// Nominal size, e.g. `"2x6"`.
    pub size: String,
    /// Cut length, inches (the long point for a mitered or cut piece).
    pub cut: f64,
    pub count: u32,
}

impl CutLine {
    /// Linear feet of all the pieces.
    pub fn linear_feet(&self) -> f64 {
        self.cut * f64::from(self.count) / 12.0
    }

    /// Board feet of all the pieces, from the nominal size (`0` when the size
    /// is not a `TxW` dimension).
    pub fn board_feet(&self) -> f64 {
        let (t, w) = self.size.split_once('x').unwrap_or(("", ""));
        match (t.parse::<f64>(), w.parse::<f64>()) {
            (Ok(t), Ok(w)) => t * w * self.cut * f64::from(self.count) / 144.0,
            _ => 0.0,
        }
    }
}

/// Adds `count` pieces to `cuts`, merging lines of the same member, size and
/// cut length (to 1/16").
pub(crate) fn add_cut(cuts: &mut Vec<CutLine>, member: &str, size: &str, cut: f64, count: u32) {
    let key = |c: f64| (c * 16.0).round() as i64;
    match cuts
        .iter_mut()
        .find(|l| l.member == member && l.size == size && key(l.cut) == key(cut))
    {
        Some(l) => l.count += count,
        None => cuts.push(CutLine {
            member: member.to_string(),
            size: size.to_string(),
            cut,
            count,
        }),
    }
}

/// Order the schedule by member type (first appearance), then size and the
/// longest cut first.
pub(crate) fn sort_cuts(cuts: &mut [CutLine]) {
    let mut first: Vec<String> = Vec::new();
    for l in cuts.iter() {
        if !first.contains(&l.member) {
            first.push(l.member.clone());
        }
    }
    let size = |s: &str| {
        let mut it = s.split('x').map(|n| n.parse::<u32>().unwrap_or(u32::MAX));
        (it.next().unwrap_or(u32::MAX), it.next().unwrap_or(0))
    };
    cuts.sort_by(|a, b| {
        let ra = first.iter().position(|m| *m == a.member);
        let rb = first.iter().position(|m| *m == b.member);
        ra.cmp(&rb)
            .then(size(&a.size).cmp(&size(&b.size)))
            .then(b.cut.total_cmp(&a.cut))
    });
}

/// A lumber list: counts per cut, board feet and linear feet per size.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Takeoff {
    /// `("2x6 x 92 5/8\" stud", count)` in order of first appearance.
    pub lines: Vec<(String, u32)>,
    /// Total board feet from nominal sizes: `T × W × length / 144`.
    pub board_feet: f64,
    /// Total linear feet per nominal size (e.g. `"2x6"`), thinnest/narrowest first.
    pub linear_feet_by_size: Vec<(String, f64)>,
    /// The framing schedule: pieces by member type, size and cut length.
    #[serde(default)]
    pub cuts: Vec<CutLine>,
}

/// Count `members` by cut, and total their board feet and linear feet.
pub fn takeoff(members: &[Member]) -> Takeoff {
    let mut lines: Vec<((MemberKind, &str), u32)> = Vec::new();
    let mut sizes: Vec<((u32, u32), f64)> = Vec::new();
    let mut board_feet = 0.0;
    let mut cuts: Vec<CutLine> = Vec::new();
    for m in members {
        add_cut(
            &mut cuts,
            m.kind.name(),
            &m.lumber.nominal_name(),
            m.length,
            1,
        );
        let key = (m.kind, m.label.as_str());
        match lines.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => lines.push((key, 1)),
        }
        let size = (m.lumber.nominal_thickness(), m.lumber.nominal_depth());
        match sizes.iter_mut().find(|(k, _)| *k == size) {
            Some((_, inches)) => *inches += m.length,
            None => sizes.push((size, m.length)),
        }
        board_feet += f64::from(size.0 * size.1) * m.length / 144.0;
    }
    sizes.sort_by_key(|&(size, _)| size);
    sort_cuts(&mut cuts);
    Takeoff {
        cuts,
        lines: lines
            .into_iter()
            .map(|((kind, label), n)| (format!("{label}\" {}", kind.name()), n))
            .collect(),
        board_feet,
        linear_feet_by_size: sizes
            .into_iter()
            .map(|((t, w), inches)| (format!("{t}x{w}"), inches / 12.0))
            .collect(),
    }
}

/// [`takeoff`] with the Framing Types applied: the member names of the cut
/// schedule and the count lines say the type for the types that ask for it
/// (Include Name in Labels), as in the Materials List description of joists,
/// rafters, headers and beams (manual p. 887). Totals do not change.
pub fn typed_takeoff(members: &[Member], catalog: &crate::catalog::FramingCatalog) -> Takeoff {
    let mut lines: Vec<((MemberKind, String), u32)> = Vec::new();
    let mut sizes: Vec<((u32, u32), f64)> = Vec::new();
    let mut board_feet = 0.0;
    let mut cuts: Vec<CutLine> = Vec::new();
    for m in members {
        let ty = catalog.type_of_member(m);
        let name = if ty.include_name_in_labels {
            format!("{} ({})", m.kind.name(), ty.name)
        } else {
            m.kind.name().to_string()
        };
        add_cut(&mut cuts, &name, &m.lumber.nominal_name(), m.length, 1);
        let key = (m.kind, m.label.clone());
        match lines.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => lines.push((key, 1)),
        }
        let size = (m.lumber.nominal_thickness(), m.lumber.nominal_depth());
        match sizes.iter_mut().find(|(k, _)| *k == size) {
            Some((_, inches)) => *inches += m.length,
            None => sizes.push((size, m.length)),
        }
        board_feet += f64::from(size.0 * size.1) * m.length / 144.0;
    }
    sizes.sort_by_key(|&(size, _)| size);
    sort_cuts(&mut cuts);
    Takeoff {
        cuts,
        lines: lines
            .into_iter()
            .map(|((kind, label), n)| (format!("{label}\" {}", kind.name()), n))
            .collect(),
        board_feet,
        linear_feet_by_size: sizes
            .into_iter()
            .map(|((t, w), inches)| (format!("{t}x{w}"), inches / 12.0))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frame_wall, FramingDefaults};
    use plan_core::{Point, Wall, WallKind};

    #[test]
    fn counts_and_board_feet() {
        let wall = Wall {
            id: 1,
            start: Point::new(0.0, 0.0),
            end: Point::new(120.0, 0.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let m = frame_wall(&wall, &[], 0.0, &FramingDefaults::default());
        let t = takeoff(&m);
        let count = |name: &str| t.lines.iter().find(|(l, _)| l == name).map(|(_, n)| *n);
        assert_eq!(count("2x6 x 104 5/8\" stud"), Some(9));
        assert_eq!(count("2x6 x 120\" top plate"), Some(2));
        assert_eq!(count("2x6 x 120\" bottom plate"), Some(1));
        assert_eq!(t.lines.len(), 3);
        let inches = 9.0 * 104.625 + 3.0 * 120.0;
        assert_eq!(t.linear_feet_by_size, [("2x6".to_string(), inches / 12.0)]);
        assert!((t.board_feet - 12.0 * inches / 144.0).abs() < 1e-9);
    }

    #[test]
    fn a_typed_takeoff_names_the_type_in_the_cut_schedule_and_keeps_the_totals() {
        use crate::catalog::{FramingCatalog, Role};
        use crate::{Transform3, TWO_BY_TEN};
        let tf = Transform3 {
            origin: [0.0; 3],
            axis_x: [1.0, 0.0, 0.0],
            axis_y: [0.0, 1.0, 0.0],
        };
        let joists: Vec<Member> = (0..4)
            .map(|_| Member::new(MemberKind::Joist, TWO_BY_TEN, 120.0, tf, None))
            .collect();
        let mut c = FramingCatalog::default();
        let plain = typed_takeoff(&joists, &c);
        assert_eq!(plain.cuts[0].member, "joist");
        assert_eq!(plain, takeoff(&joists));
        c.set_construction(Role::FloorJoist, "Joists - I Joists");
        let typed = typed_takeoff(&joists, &c);
        assert_eq!(typed.cuts[0].member, "joist (I-Joist)");
        assert_eq!(typed.cuts[0].count, 4);
        assert_eq!(typed.linear_feet_by_size, plain.linear_feet_by_size);
        assert_eq!(typed.board_feet, plain.board_feet);
    }

    #[test]
    fn eight_foot_two_by_four_is_five_and_a_third_board_feet() {
        use crate::{Member, MemberKind, Transform3, TWO_BY_FOUR};
        let tf = Transform3 {
            origin: [0.0; 3],
            axis_x: [1.0, 0.0, 0.0],
            axis_y: [0.0, 1.0, 0.0],
        };
        let m = Member::new(MemberKind::Stud, TWO_BY_FOUR, 96.0, tf, None);
        assert!((takeoff(&[m]).board_feet - 16.0 / 3.0).abs() < 1e-9);
    }
}
