//! Wall Detail: a 2D framing elevation for one wall.

use crate::lumber::format_inches;
use crate::member::{dot, Member, MemberKind};
use plan_core::{Point, Wall};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Text height of member labels, inches.
const LABEL_HEIGHT: f64 = 2.0;

/// A 2D drawing primitive in the elevation frame (X along the wall from its
/// start, Y up from the bottom of the wall).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    Line(Point, Point),
    Rect {
        min: Point,
        max: Point,
    },
    Text {
        pos: Point,
        text: String,
        height: f64,
    },
}

/// Draw the framing elevation of `wall` from its `members`.
///
/// Emits the wall's bottom and top lines, a rectangle per member (members of
/// other walls are ignored) and a label at each member's centre. Repeated
/// vertical members with the same kind and label (studs, cripples) are
/// labelled once to keep the drawing readable. The wall bottom is taken as
/// the lowest point of the wall's members.
pub fn wall_detail(wall: &Wall, members: &[Member]) -> Vec<Stroke> {
    let (d, start) = (wall.direction(), wall.start);
    let dir3 = [d.x, 0.0, -d.y];
    let start3 = [start.x, 0.0, -start.y];
    let mine: Vec<&Member> = members
        .iter()
        .filter(|m| m.wall_id == Some(wall.id))
        .collect();
    let base = mine
        .iter()
        .flat_map(|m| m.corners())
        .map(|c| c[1])
        .fold(f64::INFINITY, f64::min);
    let len = wall.length();
    let mut out = Vec::new();
    if base.is_finite() {
        out.push(Stroke::Line(Point::new(0.0, 0.0), Point::new(len, 0.0)));
        out.push(Stroke::Line(
            Point::new(0.0, wall.height),
            Point::new(len, wall.height),
        ));
    }
    let mut labelled = HashSet::new();
    for m in mine {
        let (min, max) = elevation_rect(m, start3, dir3, base);
        out.push(Stroke::Rect { min, max });
        let vertical = max.y - min.y > max.x - min.x;
        if !vertical || labelled.insert((m.kind, m.label.clone())) {
            out.push(Stroke::Text {
                pos: Point::lerp(min, max, 0.5),
                text: m.label.clone(),
                height: LABEL_HEIGHT,
            });
        }
    }
    out
}

/// The rectangle `(min, max)` of `m` in the wall's elevation frame.
fn elevation_rect(m: &Member, start3: [f64; 3], dir3: [f64; 3], base: f64) -> (Point, Point) {
    let pts: Vec<Point> = m
        .corners()
        .iter()
        .map(|c| {
            let rel = [c[0] - start3[0], 0.0, c[2] - start3[2]];
            Point::new(dot(rel, dir3), c[1] - base)
        })
        .collect();
    let min = pts
        .iter()
        .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
            Point::new(a.x.min(p.x), a.y.min(p.y))
        });
    let max = pts
        .iter()
        .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
            Point::new(a.x.max(p.x), a.y.max(p.y))
        });
    (min, max)
}

/// One member of a Wall Detail as the window shows and selects it: the index
/// of the member in the slice given to [`wall_detail_members`], what it is and
/// where it stands in the elevation frame of [`wall_detail`].
#[derive(Debug, Clone, PartialEq)]
pub struct DetailMember {
    pub index: usize,
    pub kind: MemberKind,
    pub label: String,
    pub min: Point,
    pub max: Point,
}

impl DetailMember {
    /// Whether the elevation point `p` is inside the member's rectangle,
    /// grown by `tol` inches.
    pub fn contains(&self, p: Point, tol: f64) -> bool {
        p.x >= self.min.x - tol
            && p.x <= self.max.x + tol
            && p.y >= self.min.y - tol
            && p.y <= self.max.y + tol
    }
}

/// The members of `wall` among `members`, in the elevation frame of
/// [`wall_detail`], each with its index in `members` so an edit can find it
/// again. Plies of a doubled header share a rectangle and are all listed.
pub fn wall_detail_members(wall: &Wall, members: &[Member]) -> Vec<DetailMember> {
    let (d, start) = (wall.direction(), wall.start);
    let dir3 = [d.x, 0.0, -d.y];
    let start3 = [start.x, 0.0, -start.y];
    let base = members
        .iter()
        .filter(|m| m.wall_id == Some(wall.id))
        .flat_map(|m| m.corners())
        .map(|c| c[1])
        .fold(f64::INFINITY, f64::min);
    if !base.is_finite() {
        return Vec::new();
    }
    members
        .iter()
        .enumerate()
        .filter(|(_, m)| m.wall_id == Some(wall.id))
        .map(|(index, m)| {
            let (min, max) = elevation_rect(m, start3, dir3, base);
            DetailMember {
                index,
                kind: m.kind,
                label: m.label.clone(),
                min,
                max,
            }
        })
        .collect()
}

/// A member cut by a section plane drawn as Chief does (manual p. 924): the
/// rectangle `(min, max)` with an X through it, or with a single diagonal for
/// blocking.
pub fn cut_symbol(min: Point, max: Point, blocking: bool) -> Vec<Stroke> {
    let mut out = vec![Stroke::Rect { min, max }];
    out.push(Stroke::Line(min, max));
    if !blocking {
        out.push(Stroke::Line(
            Point::new(min.x, max.y),
            Point::new(max.x, min.y),
        ));
    }
    out
}

/// The S and E marks of a selected framing member (Start and End Indicators in
/// Preferences): a letter just past each end, `size` inches tall.
pub fn start_end_marks(start: Point, end: Point, size: f64) -> [Stroke; 2] {
    let along = (end - start).normalized();
    let off = size * 1.2;
    [
        Stroke::Text {
            pos: start - along * off,
            text: "S".into(),
            height: size,
        },
        Stroke::Text {
            pos: end + along * off,
            text: "E".into(),
            height: size,
        },
    ]
}

/// What a [`DetailDim`] measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimKind {
    /// The wall's overall length.
    Overall,
    /// Centre to centre of two neighbouring common studs.
    StudSpacing,
    /// Width between the trimmers of an opening (the rough opening).
    RoughWidth,
    /// Height of the rough opening, sill (or floor) to header.
    RoughHeight,
    /// Height of the underside of a header above the bottom plate's base.
    HeaderHeight,
    /// Height of the top of a sill.
    SillHeight,
    /// The wall's height, bottom of the bottom plate to top of the top plate.
    WallHeight,
}

/// One dimension of a wall detail, in the elevation frame of [`wall_detail`]:
/// from `a` to `b`, drawn `offset` inches off the members (positive is above
/// or to the right).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailDim {
    pub kind: DimKind,
    pub a: Point,
    pub b: Point,
    pub offset: f64,
    pub text: String,
}

/// Distance between the framing lines of a detail dimension string.
fn inch_text(v: f64) -> String {
    format!("{}\"", format_inches(v))
}

const NEAR: f64 = 0.05;

/// The dimensions Chief's Wall Detail draws on the framing elevation of
/// `wall`: its length and height, the on-centre spacing of the common studs,
/// and for each opening the rough opening width and height, the header
/// height and, for a window, the sill height. Members of other walls are
/// ignored.
pub fn wall_detail_dims(wall: &Wall, members: &[Member]) -> Vec<DetailDim> {
    let (d, start) = (wall.direction(), wall.start);
    let dir3 = [d.x, 0.0, -d.y];
    let start3 = [start.x, 0.0, -start.y];
    let mine: Vec<&Member> = members
        .iter()
        .filter(|m| m.wall_id == Some(wall.id))
        .collect();
    let base = mine
        .iter()
        .flat_map(|m| m.corners())
        .map(|c| c[1])
        .fold(f64::INFINITY, f64::min);
    if !base.is_finite() {
        return Vec::new();
    }
    let rects: Vec<(MemberKind, Point, Point)> = mine
        .iter()
        .map(|m| {
            let (lo, hi) = elevation_rect(m, start3, dir3, base);
            (m.kind, lo, hi)
        })
        .collect();
    let top = rects.iter().map(|r| r.2.y).fold(0.0, f64::max);
    let len = wall.length();
    let mut out = vec![
        DetailDim {
            kind: DimKind::Overall,
            a: Point::new(0.0, 0.0),
            b: Point::new(len, 0.0),
            offset: -12.0,
            text: inch_text(len),
        },
        DetailDim {
            kind: DimKind::WallHeight,
            a: Point::new(0.0, 0.0),
            b: Point::new(0.0, top),
            offset: -12.0,
            text: inch_text(top),
        },
    ];
    // Common studs, centre to centre.
    let mut studs: Vec<f64> = rects
        .iter()
        .filter(|r| r.0 == MemberKind::Stud)
        .map(|r| (r.1.x + r.2.x) / 2.0)
        .collect();
    studs.sort_by(f64::total_cmp);
    studs.dedup_by(|a, b| (*a - *b).abs() < NEAR);
    for w in studs.windows(2) {
        out.push(DetailDim {
            kind: DimKind::StudSpacing,
            a: Point::new(w[0], top),
            b: Point::new(w[1], top),
            offset: 6.0,
            text: inch_text(w[1] - w[0]),
        });
    }
    // Openings: each header names its trimmers (those that carry it).
    let mut headers: Vec<&(MemberKind, Point, Point)> =
        rects.iter().filter(|r| r.0 == MemberKind::Header).collect();
    headers.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
    // The plies of one header sit side by side across the thickness, so they
    // share an elevation rectangle: keep one.
    headers.dedup_by(|a, b| (a.1.x - b.1.x).abs() < NEAR && (a.1.y - b.1.y).abs() < NEAR);
    for h in headers {
        let mid = (h.1.x + h.2.x) / 2.0;
        let carriers = rects.iter().filter(|r| {
            r.0 == MemberKind::TrimmerStud
                && (r.2.y - h.1.y).abs() < NEAR
                && r.2.x >= h.1.x - NEAR
                && r.1.x <= h.2.x + NEAR
        });
        let left = carriers
            .clone()
            .filter(|r| (r.1.x + r.2.x) / 2.0 < mid)
            .map(|r| r.2.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let right = carriers
            .filter(|r| (r.1.x + r.2.x) / 2.0 > mid)
            .map(|r| r.1.x)
            .fold(f64::INFINITY, f64::min);
        if !left.is_finite() || !right.is_finite() {
            continue;
        }
        // A sill closes the opening from below; a door runs down to the plate.
        let sill = rects
            .iter()
            .filter(|r| {
                r.0 == MemberKind::Sill
                    && r.1.x < right + NEAR
                    && r.2.x > left - NEAR
                    && r.2.y < h.1.y
            })
            .map(|r| r.2.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let floor = rects
            .iter()
            .filter(|r| r.0 == MemberKind::BottomPlate)
            .map(|r| r.2.y)
            .fold(0.0, f64::max);
        let bottom = if sill.is_finite() { sill } else { floor };
        out.push(DetailDim {
            kind: DimKind::RoughWidth,
            a: Point::new(left, h.1.y),
            b: Point::new(right, h.1.y),
            offset: -6.0,
            text: inch_text(right - left),
        });
        out.push(DetailDim {
            kind: DimKind::RoughHeight,
            a: Point::new(right, bottom),
            b: Point::new(right, h.1.y),
            offset: 6.0,
            text: inch_text(h.1.y - bottom),
        });
        out.push(DetailDim {
            kind: DimKind::HeaderHeight,
            a: Point::new(left, 0.0),
            b: Point::new(left, h.1.y),
            offset: -6.0,
            text: inch_text(h.1.y),
        });
        if sill.is_finite() {
            out.push(DetailDim {
                kind: DimKind::SillHeight,
                a: Point::new(left, 0.0),
                b: Point::new(left, sill),
                offset: -18.0,
                text: inch_text(sill),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{frame_wall, FramingDefaults, MemberKind};
    use plan_core::{Opening, WallKind};

    #[test]
    fn elevation_rects_match_members() {
        let wall = Wall {
            id: 3,
            start: Point::new(10.0, 20.0),
            end: Point::new(10.0, 140.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let door = Opening::default_door(1, wall.id, 60.0);
        let m = frame_wall(&wall, &[&door], 0.0, &FramingDefaults::default());
        let strokes = wall_detail(&wall, &m);
        let rects: Vec<_> = strokes
            .iter()
            .filter_map(|s| match s {
                Stroke::Rect { min, max } => Some((*min, *max)),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), m.len());
        // Everything lies inside the wall's elevation outline.
        for (lo, hi) in &rects {
            assert!(lo.x >= -1e-9 && hi.x <= 120.0 + 1e-9);
            assert!(lo.y >= -1e-9 && hi.y <= 109.125 + 1e-9);
        }
        // The first member is the bottom plate: full length, 1 1/2" tall.
        assert_eq!(m[0].kind, MemberKind::BottomPlate);
        assert!((rects[0].1.x - rects[0].0.x - 120.0).abs() < 1e-9);
        assert!((rects[0].1.y - 1.5).abs() < 1e-9);
        let texts = strokes
            .iter()
            .filter(|s| matches!(s, Stroke::Text { .. }))
            .count();
        assert!(texts > 0 && texts < m.len());
        // Members of other walls are ignored.
        let mut other = wall.clone();
        other.id = 99;
        assert!(wall_detail(&other, &m).is_empty());
    }

    #[test]
    fn the_detail_dimensions_give_the_rough_opening_header_and_stud_spacing() {
        let wall = Wall {
            id: 3,
            start: Point::new(0.0, 0.0),
            end: Point::new(240.0, 0.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let door = Opening::default_door(1, wall.id, 60.0);
        let m = frame_wall(&wall, &[&door], 0.0, &FramingDefaults::default());
        let dims = wall_detail_dims(&wall, &m);
        let of = |k: DimKind| dims.iter().filter(|d| d.kind == k).collect::<Vec<_>>();
        assert_eq!(of(DimKind::Overall)[0].text, "240\"");
        assert_eq!(of(DimKind::WallHeight)[0].text, "109 1/8\"");
        // Studs are on 16" centres between the openings' studs.
        let spacing = of(DimKind::StudSpacing);
        assert!(!spacing.is_empty());
        assert!(spacing.iter().any(|d| d.text == "16\""));
        // One opening: its rough width is the door width, its header sits at
        // the rough height, and a door has no sill.
        let width = of(DimKind::RoughWidth);
        assert_eq!(width.len(), 1);
        let ro = width[0].b.x - width[0].a.x;
        assert!(
            (ro - door.width).abs() < 1.0 + 1e-9,
            "rough width {ro} for a {} door",
            door.width
        );
        assert_eq!(of(DimKind::RoughHeight).len(), 1);
        assert_eq!(of(DimKind::HeaderHeight).len(), 1);
        assert!(of(DimKind::SillHeight).is_empty());
        // Other walls get nothing.
        let mut other = wall.clone();
        other.id = 9;
        assert!(wall_detail_dims(&other, &m).is_empty());
    }

    #[test]
    fn a_window_adds_a_sill_height_and_a_shorter_rough_opening() {
        let wall = Wall {
            id: 4,
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 200.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let win = Opening::default_window(1, wall.id, 100.0);
        let m = frame_wall(&wall, &[&win], 0.0, &FramingDefaults::default());
        let dims = wall_detail_dims(&wall, &m);
        let sill: Vec<_> = dims
            .iter()
            .filter(|d| d.kind == DimKind::SillHeight)
            .collect();
        assert_eq!(sill.len(), 1);
        let height = dims
            .iter()
            .find(|d| d.kind == DimKind::RoughHeight)
            .unwrap();
        let header = dims
            .iter()
            .find(|d| d.kind == DimKind::HeaderHeight)
            .unwrap();
        // Rough height = header underside minus the sill top.
        assert!((height.b.y - height.a.y - (header.b.y - sill[0].b.y)).abs() < 1e-9);
        assert!(height.b.y - height.a.y < header.b.y);
    }

    // ----- Round 16: members, cut symbols, S and E -----

    fn walled() -> (Wall, Vec<Member>) {
        let w = Wall {
            id: 7,
            start: Point::new(0.0, 0.0),
            end: Point::new(144.0, 0.0),
            thickness: 6.5,
            height: 109.125,
            kind: WallKind::Exterior,
            layer: "Walls, Normal".into(),
            ..Default::default()
        };
        let win = Opening::default_window(2, w.id, 72.0);
        let m = frame_wall(&w, &[&win], 0.0, &FramingDefaults::default());
        (w, m)
    }

    #[test]
    fn a_wall_detail_lists_the_members_of_a_wall_with_a_window() {
        let (w, mut members) = walled();
        // A member of another wall is not in this wall's detail.
        let mut other = members[0].clone();
        other.wall_id = Some(99);
        members.push(other);
        let list = wall_detail_members(&w, &members);
        assert_eq!(list.len(), members.len() - 1);
        let kinds = |k: MemberKind| list.iter().filter(|m| m.kind == k).count();
        assert_eq!(kinds(MemberKind::Sill), 1);
        assert_eq!(kinds(MemberKind::Header), 2);
        assert_eq!(kinds(MemberKind::TrimmerStud), 2);
        assert_eq!(kinds(MemberKind::KingStud), 2);
        assert_eq!(kinds(MemberKind::TopPlate), 2);
        assert!(kinds(MemberKind::CrippleStud) >= 4);
        // The sill stands at 24" over the bottom of the bottom plate, a window 36 wide.
        let sill = list.iter().find(|m| m.kind == MemberKind::Sill).unwrap();
        assert!((sill.min.y - 22.5).abs() < 1e-9 && (sill.max.x - sill.min.x - 36.0).abs() < 1e-9);
        assert!(sill.contains(Point::new(72.0, 23.0), 0.0));
        assert!(!sill.contains(Point::new(72.0, 60.0), 1.0));
        // Indexes point back into the slice that was given.
        assert!(list.iter().all(|m| members[m.index].kind == m.kind));
        assert!(list.iter().all(|m| members[m.index].wall_id == Some(7)));
        // The window's rough opening width is between the trimmers.
        let trimmers: Vec<_> = list
            .iter()
            .filter(|m| m.kind == MemberKind::TrimmerStud)
            .collect();
        let (a, b) = (trimmers[0], trimmers[1]);
        let gap = (a.min.x.max(b.min.x)) - (a.max.x.min(b.max.x));
        assert!((gap - 36.0).abs() < 1e-6, "{gap}");
    }

    #[test]
    fn a_cut_member_is_a_box_with_an_x_and_blocking_has_one_diagonal() {
        let (lo, hi) = (Point::new(0.0, 0.0), Point::new(1.5, 9.25));
        let x = cut_symbol(lo, hi, false);
        assert!(matches!(x[0], Stroke::Rect { .. }));
        assert_eq!(x.len(), 3);
        let block = cut_symbol(lo, hi, true);
        assert_eq!(block.len(), 2);
        let [s, e] = start_end_marks(Point::new(0.0, 0.0), Point::new(100.0, 0.0), 2.0);
        match (s, e) {
            (
                Stroke::Text {
                    pos: p, text: a, ..
                },
                Stroke::Text {
                    pos: q, text: b, ..
                },
            ) => {
                assert_eq!((a.as_str(), b.as_str()), ("S", "E"));
                assert!(p.x < 0.0 && q.x > 100.0);
            }
            _ => panic!("two labels"),
        }
    }
}
