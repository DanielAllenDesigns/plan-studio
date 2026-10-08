//! Quantity takeoff for a set of framing members.

use crate::member::{Member, MemberKind};
use serde::{Deserialize, Serialize};

/// A lumber list: counts per cut, board feet and linear feet per size.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Takeoff {
    /// `("2x6 x 92 5/8\" stud", count)` in order of first appearance.
    pub lines: Vec<(String, u32)>,
    /// Total board feet from nominal sizes: `T × W × length / 144`.
    pub board_feet: f64,
    /// Total linear feet per nominal size (e.g. `"2x6"`), thinnest/narrowest first.
    pub linear_feet_by_size: Vec<(String, f64)>,
}

/// Count `members` by cut, and total their board feet and linear feet.
pub fn takeoff(members: &[Member]) -> Takeoff {
    let mut lines: Vec<((MemberKind, &str), u32)> = Vec::new();
    let mut sizes: Vec<((u32, u32), f64)> = Vec::new();
    let mut board_feet = 0.0;
    for m in members {
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
    Takeoff {
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
