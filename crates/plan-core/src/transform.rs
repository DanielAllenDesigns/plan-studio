//! Pure plan transforms over objects (S-47, S-48, S-52, S-101..S-106, DW-23):
//! move, rotate, resize and reflect as one affine [`Xform`], the alignment
//! and distribution arithmetic of the Edit toolbar, and `Make Parallel`.
//!
//! An [`Xform`] is a 2D similarity (rotation or reflection, uniform scale and
//! a shift), so a chain of steps collapses into one value and a replicate
//! with N copies is `xform.pow(k)` for copy k. The functions here know the
//! model's own kinds ([`ObjectRef`]: walls with their openings, dimensions,
//! CAD, symbols, cameras); the editor handles the kinds stored as opaque
//! records with the same [`Xform`].

use crate::cad::CadItem;
use crate::camera::CameraObject;
use crate::dimension::Dimension;
use crate::geometry::Point;
use crate::groups::ObjectRef;
use crate::model::{Id, Project, Wall};
use crate::symbols::PlacedSymbol;
use crate::walls::Side;
use std::collections::HashSet;
use std::f64::consts::{FRAC_PI_2, TAU};

/// A 2D similarity transform: `x' = a x + b y + e`, `y' = c x + d y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Xform {
    pub const IDENTITY: Xform = Xform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Shift by `d`.
    pub fn translate(d: Point) -> Xform {
        Xform {
            e: d.x,
            f: d.y,
            ..Xform::IDENTITY
        }
    }

    /// Counter-clockwise rotation by `angle` radians about `center`.
    pub fn rotate(center: Point, angle: f64) -> Xform {
        let (s, c) = angle.sin_cos();
        Xform {
            a: c,
            b: -s,
            c: s,
            d: c,
            e: center.x - (c * center.x - s * center.y),
            f: center.y - (s * center.x + c * center.y),
        }
    }

    /// Uniform scale by `factor` about `center`.
    pub fn scale(center: Point, factor: f64) -> Xform {
        Xform {
            a: factor,
            b: 0.0,
            c: 0.0,
            d: factor,
            e: center.x * (1.0 - factor),
            f: center.y * (1.0 - factor),
        }
    }

    /// Mirror about the line through `a` and `b`. A zero-length axis is the
    /// identity.
    pub fn reflect(a: Point, b: Point) -> Xform {
        let dir = b.sub(a);
        if dir.length() < 1e-9 {
            return Xform::IDENTITY;
        }
        let phi = dir.angle();
        let (s, c) = (2.0 * phi).sin_cos();
        // Reflection across the line through the origin at angle phi, then
        // shifted so that `a` stays put.
        Xform {
            a: c,
            b: s,
            c: s,
            d: -c,
            e: a.x - (c * a.x + s * a.y),
            f: a.y - (s * a.x - c * a.y),
        }
    }

    /// `self` first, then `next`.
    pub fn then(self, next: Xform) -> Xform {
        Xform {
            a: next.a * self.a + next.b * self.c,
            b: next.a * self.b + next.b * self.d,
            c: next.c * self.a + next.d * self.c,
            d: next.c * self.b + next.d * self.d,
            e: next.a * self.e + next.b * self.f + next.e,
            f: next.c * self.e + next.d * self.f + next.f,
        }
    }

    /// `self` applied `k` times (`k = 0` is the identity).
    pub fn pow(self, k: u32) -> Xform {
        let mut out = Xform::IDENTITY;
        for _ in 0..k {
            out = out.then(self);
        }
        out
    }

    pub fn apply(&self, p: Point) -> Point {
        Point::new(
            self.a * p.x + self.b * p.y + self.e,
            self.c * p.x + self.d * p.y + self.f,
        )
    }

    /// The vector `v` carried through the linear part (no shift).
    pub fn apply_vector(&self, v: Point) -> Point {
        Point::new(self.a * v.x + self.b * v.y, self.c * v.x + self.d * v.y)
    }

    /// Does the transform mirror (swap handedness)?
    pub fn is_mirror(&self) -> bool {
        self.a * self.d - self.b * self.c < 0.0
    }

    /// The uniform scale factor.
    pub fn scale_factor(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// The direction angle (radians) of a line that pointed along `angle`.
    pub fn map_angle(&self, angle: f64) -> f64 {
        self.apply_vector(Point::new(angle.cos(), angle.sin()))
            .angle()
    }

    /// The angle the x axis turns to (the rotation of a non-mirror
    /// transform; for a mirror the angle of the mirrored x axis).
    pub fn turn(&self) -> f64 {
        self.map_angle(0.0)
    }

    /// Is the linear part the identity (a pure shift)?
    pub fn is_translation(&self) -> bool {
        (self.a - 1.0).abs() < 1e-9
            && self.b.abs() < 1e-9
            && self.c.abs() < 1e-9
            && (self.d - 1.0).abs() < 1e-9
    }

    /// The shift of a pure translation (the image of the origin).
    pub fn shift(&self) -> Point {
        Point::new(self.e, self.f)
    }

    pub fn is_identity(&self) -> bool {
        let id = Xform::IDENTITY;
        [
            self.a - id.a,
            self.b - id.b,
            self.c - id.c,
            self.d - id.d,
            self.e - id.e,
            self.f - id.f,
        ]
        .iter()
        .all(|v| v.abs() < 1e-9)
    }
}

// ----- per kind -----

/// Transforms a wall's centerline (and curve and exterior side). Thickness
/// and height stay: a resize scales plan positions, not wall sections.
pub fn xform_wall(w: &mut Wall, x: &Xform) {
    w.start = x.apply(w.start);
    w.end = x.apply(w.end);
    if let Some(c) = w.curve.as_mut() {
        c.bulge *= x.scale_factor();
        if x.is_mirror() {
            c.bulge = -c.bulge;
        }
    }
    if x.is_mirror() {
        w.exterior_side = match w.exterior_side {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        };
    }
}

/// Transforms a CAD item. Text keeps reading left to right (a mirror moves
/// it but does not flip the letters).
pub fn xform_cad_item(item: &mut CadItem, x: &Xform) {
    let k = x.scale_factor();
    match item {
        CadItem::Line { a, b } => {
            *a = x.apply(*a);
            *b = x.apply(*b);
        }
        CadItem::Circle { center, radius } => {
            *center = x.apply(*center);
            *radius *= k;
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            *center = x.apply(*center);
            *radius *= k;
            let (s, e) = (x.map_angle(*start_angle), x.map_angle(*end_angle));
            // A mirrored counter-clockwise arc runs clockwise: swap its ends.
            (*start_angle, *end_angle) = if x.is_mirror() { (e, s) } else { (s, e) };
            while *end_angle < *start_angle {
                *end_angle += TAU;
            }
        }
        CadItem::Polyline { points, .. } => {
            for p in points.iter_mut() {
                *p = x.apply(*p);
            }
        }
        CadItem::Text {
            pos, height, angle, ..
        } => {
            *pos = x.apply(*pos);
            *height *= k;
            if !x.is_mirror() {
                *angle = x.map_angle(*angle);
            }
        }
    }
}

/// Transforms a dimension's points. A mirror puts the dimension line on the
/// mirrored side. The anchors are cleared; the caller re-attaches them when
/// the walls moved with it.
pub fn xform_dimension(d: &mut Dimension, x: &Xform) {
    d.start = x.apply(d.start);
    d.end = x.apply(d.end);
    d.offset *= x.scale_factor();
    if x.is_mirror() {
        d.offset = -d.offset;
    }
    // A curved dimension carries its arc with it.
    if let Some(c) = d.look.seg.curve.as_mut() {
        let mirror = x.is_mirror();
        c.center = x.apply(c.center);
        c.radius *= x.scale_factor();
        c.start = x.map_angle(c.start);
        if mirror {
            c.sweep = -c.sweep;
            c.lateral = -c.lateral;
        }
        c.lateral *= x.scale_factor();
    }
    if let Some(m) = d.look.seg.label_move.as_mut() {
        *m = m.scale(x.scale_factor());
    }
}

/// Transforms a placed symbol: position, facing and size; a mirror also
/// flips it left to right.
pub fn xform_symbol(s: &mut PlacedSymbol, x: &Xform) {
    let k = x.scale_factor();
    s.position = x.apply(s.position);
    s.angle = x.map_angle(s.angle);
    s.width *= k;
    s.depth *= k;
    if x.is_mirror() {
        s.flip = !s.flip;
    }
}

/// Transforms a camera: eye position, view direction and walkthrough path.
pub fn xform_camera(c: &mut CameraObject, x: &Xform) {
    c.position = x.apply(c.position);
    c.direction_deg = x.map_angle(c.direction_deg.to_radians()).to_degrees();
    for p in c.path.iter_mut() {
        *p = x.apply(*p);
    }
    if let Some(sec) = c.section.as_mut() {
        sec.a = x.apply(sec.a);
        sec.b = x.apply(sec.b);
    }
}

/// Shifts a camera by `d`: eye, walkthrough path and the cut line of a
/// section all move together.
pub fn translate_camera(c: &mut CameraObject, d: Point) {
    c.position = c.position + d;
    for p in c.path.iter_mut() {
        *p = *p + d;
    }
    if let Some(sec) = c.section.as_mut() {
        sec.a = sec.a + d;
        sec.b = sec.b + d;
    }
}

impl Project {
    /// Applies `x` to the listed objects of `floor`; returns how many were
    /// transformed. Walls take their openings along (a mirror flips the
    /// swing so doors still mirror their appearance, DW-23, S-106);
    /// dimensions tied to a wall outside the set are released. An opening
    /// alone is not transformed: it follows its wall.
    pub fn transform_objects(&mut self, floor: usize, refs: &[ObjectRef], x: &Xform) -> usize {
        let walls: HashSet<Id> = refs
            .iter()
            .filter_map(|r| match r {
                ObjectRef::Wall(id) => Some(*id),
                _ => None,
            })
            .collect();
        let k = x.scale_factor();
        let mirror = x.is_mirror();
        let mut n = 0;
        let f = &mut self.floors[floor];
        for w in f.walls.iter_mut().filter(|w| walls.contains(&w.id)) {
            xform_wall(w, x);
            n += 1;
        }
        for o in f.openings.iter_mut().filter(|o| walls.contains(&o.wall_id)) {
            o.center_offset *= k;
            if mirror {
                o.swing_flipped = !o.swing_flipped;
            }
        }
        for r in refs {
            match *r {
                ObjectRef::Dimension(id) => {
                    if let Some(d) = f.dimensions.iter_mut().find(|d| d.id == id) {
                        xform_dimension(d, x);
                        for a in d.anchors.iter_mut() {
                            if a.as_ref().is_some_and(|an| !walls.contains(&an.wall)) {
                                *a = None;
                            }
                        }
                        // A curved dimension stays tied to walls that
                        // moved with it only.
                        if let Some(c) = d.look.seg.curve.as_mut() {
                            for w in c.walls.iter_mut() {
                                if w.is_some_and(|id| !walls.contains(&id)) {
                                    *w = None;
                                }
                            }
                        }
                        n += 1;
                    }
                }
                ObjectRef::Cad(id) => {
                    if let Some(c) = f.cad.iter_mut().find(|c| c.id == id) {
                        xform_cad_item(&mut c.item, x);
                        n += 1;
                    }
                }
                ObjectRef::Symbol(id) => {
                    if let Some(s) = f.symbols.iter_mut().find(|s| s.id == id) {
                        xform_symbol(s, x);
                        n += 1;
                    }
                }
                _ => {}
            }
        }
        for r in refs {
            if let ObjectRef::Camera(id) = *r {
                if self.update_camera(id, |c| xform_camera(c, x)) {
                    n += 1;
                }
            }
        }
        n
    }
}

// ----- bounds -----

/// Lower-left and upper-right of `points`, or `None` when empty.
pub fn bounds_of(points: impl IntoIterator<Item = Point>) -> Option<(Point, Point)> {
    let mut it = points.into_iter();
    let first = it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some((lo, hi))
}

/// The box around two boxes.
pub fn union_box(a: (Point, Point), b: (Point, Point)) -> (Point, Point) {
    (
        Point::new(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
        Point::new(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
    )
}

/// The box around a model object, for the kinds `plan-core` owns.
pub fn object_bounds(project: &Project, floor: usize, r: ObjectRef) -> Option<(Point, Point)> {
    let f = project.floors.get(floor)?;
    match r {
        ObjectRef::Wall(id) => f.wall(id).and_then(|w| bounds_of(w.footprint())),
        ObjectRef::Opening(id) => {
            let o = f.openings.iter().find(|o| o.id == id)?;
            let w = f.wall(o.wall_id)?;
            let n = w.normal().scale(w.thickness * 0.5);
            let (a, b) = (w.point_at(o.start_offset()), w.point_at(o.end_offset()));
            bounds_of([a.add(n), b.add(n), b.sub(n), a.sub(n)])
        }
        ObjectRef::Dimension(id) => {
            let d = f.dimensions.iter().find(|d| d.id == id)?;
            let (p, q) = d.line_points();
            bounds_of([d.start, d.end, p, q])
        }
        ObjectRef::Cad(id) => f.cad.iter().find(|c| c.id == id).map(|c| c.bounds()),
        ObjectRef::Symbol(id) => f.symbol(id).and_then(|s| bounds_of(s.footprint())),
        ObjectRef::Camera(id) => project.camera(id).map(|c| (c.position, c.position)),
        _ => None,
    }
}

// ----- align and distribute -----

/// Where Align puts the selected boxes (Y is up, so Top is the largest y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Left,
    Right,
    /// Centers on one vertical line.
    Center,
    Top,
    Bottom,
    /// Centers on one horizontal line.
    Middle,
}

impl AlignMode {
    pub const ALL: [AlignMode; 6] = [
        AlignMode::Left,
        AlignMode::Center,
        AlignMode::Right,
        AlignMode::Top,
        AlignMode::Middle,
        AlignMode::Bottom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AlignMode::Left => "Align Left",
            AlignMode::Right => "Align Right",
            AlignMode::Center => "Align Center",
            AlignMode::Top => "Align Top",
            AlignMode::Bottom => "Align Bottom",
            AlignMode::Middle => "Align Middle",
        }
    }

    /// The command id the editor routes (`edit.align.left`, ...).
    pub fn id(self) -> &'static str {
        match self {
            AlignMode::Left => "edit.align.left",
            AlignMode::Right => "edit.align.right",
            AlignMode::Center => "edit.align.center",
            AlignMode::Top => "edit.align.top",
            AlignMode::Bottom => "edit.align.bottom",
            AlignMode::Middle => "edit.align.middle",
        }
    }
}

/// The shift each box needs to line up (`boxes` are lower-left, upper-right
/// pairs). One box or none: no shift.
pub fn align_offsets(boxes: &[(Point, Point)], mode: AlignMode) -> Vec<Point> {
    let Some(all) = boxes.iter().copied().reduce(union_box) else {
        return Vec::new();
    };
    let mid = Point::new((all.0.x + all.1.x) * 0.5, (all.0.y + all.1.y) * 0.5);
    boxes
        .iter()
        .map(|(lo, hi)| match mode {
            AlignMode::Left => Point::new(all.0.x - lo.x, 0.0),
            AlignMode::Right => Point::new(all.1.x - hi.x, 0.0),
            AlignMode::Center => Point::new(mid.x - (lo.x + hi.x) * 0.5, 0.0),
            AlignMode::Top => Point::new(0.0, all.1.y - hi.y),
            AlignMode::Bottom => Point::new(0.0, all.0.y - lo.y),
            AlignMode::Middle => Point::new(0.0, mid.y - (lo.y + hi.y) * 0.5),
        })
        .collect()
}

/// Which way Distribute spaces the boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// The shift each box needs so the gaps between neighbours are equal. The
/// outer two boxes stay; with `gap` the spacing is that value instead and
/// only the first box stays. Needs three boxes (two with a `gap`).
pub fn distribute_offsets(boxes: &[(Point, Point)], axis: Axis, gap: Option<f64>) -> Vec<Point> {
    let n = boxes.len();
    let none = vec![Point::ZERO; n];
    if n < 2 || (n < 3 && gap.is_none()) {
        return none;
    }
    type Edge = fn(&(Point, Point)) -> f64;
    let (lo_of, hi_of): (Edge, Edge) = match axis {
        Axis::Horizontal => (|b| b.0.x, |b| b.1.x),
        Axis::Vertical => (|b| b.0.y, |b| b.1.y),
    };
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| {
        (lo_of(&boxes[i]) + hi_of(&boxes[i])).total_cmp(&(lo_of(&boxes[j]) + hi_of(&boxes[j])))
    });
    let sizes: f64 = boxes.iter().map(|b| hi_of(b) - lo_of(b)).sum();
    let first = lo_of(&boxes[order[0]]);
    let last = hi_of(&boxes[order[n - 1]]);
    let gap = gap.unwrap_or_else(|| (last - first - sizes) / (n - 1) as f64);
    let mut out = none;
    let mut at = first;
    for &i in &order {
        let shift = at - lo_of(&boxes[i]);
        out[i] = match axis {
            Axis::Horizontal => Point::new(shift, 0.0),
            Axis::Vertical => Point::new(0.0, shift),
        };
        at += hi_of(&boxes[i]) - lo_of(&boxes[i]) + gap;
    }
    out
}

// ----- parallel and perpendicular -----

/// Make Parallel / Perpendicular (S-41): the new end of a segment that keeps
/// its `start` and length but runs along the reference direction `along`
/// (rotated a quarter turn when `perpendicular`), on whichever of the two
/// senses is nearer its present direction.
pub fn parallel_end(start: Point, end: Point, along: Point, perpendicular: bool) -> Point {
    let len = start.dist(end);
    let mut dir = along.normalized();
    if perpendicular {
        dir = dir.perp();
    }
    if dir.length() < 1e-9 || len < 1e-9 {
        return end;
    }
    if dir.dot(end.sub(start)) < 0.0 {
        dir = dir.scale(-1.0);
    }
    start.add(dir.scale(len))
}

/// A quarter turn, for callers that build perpendicular axes.
pub const QUARTER_TURN: f64 = FRAC_PI_2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OpeningKind, WallKind};

    fn close(a: Point, b: Point) -> bool {
        a.dist(b) < 1e-6
    }

    #[test]
    fn rotate_about_a_point_and_scale_about_a_point() {
        let r = Xform::rotate(Point::new(10.0, 10.0), FRAC_PI_2);
        assert!(close(
            r.apply(Point::new(20.0, 10.0)),
            Point::new(10.0, 20.0)
        ));
        assert!(close(
            r.apply(Point::new(10.0, 10.0)),
            Point::new(10.0, 10.0)
        ));
        let s = Xform::scale(Point::new(10.0, 0.0), 2.0);
        assert!(close(
            s.apply(Point::new(20.0, 5.0)),
            Point::new(30.0, 10.0)
        ));
        assert!((s.scale_factor() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn reflecting_about_a_line_mirrors_points_and_angles() {
        // The vertical line x = 100.
        let m = Xform::reflect(Point::new(100.0, 0.0), Point::new(100.0, 50.0));
        assert!(m.is_mirror());
        assert!(close(
            m.apply(Point::new(70.0, 5.0)),
            Point::new(130.0, 5.0)
        ));
        assert!(close(
            m.apply(Point::new(100.0, 9.0)),
            Point::new(100.0, 9.0)
        ));
        // A line along +x now points along -x.
        assert!((m.map_angle(0.0).abs() - std::f64::consts::PI).abs() < 1e-9);
        // A 45 degree axis swaps x and y.
        let d = Xform::reflect(Point::ZERO, Point::new(1.0, 1.0));
        assert!(close(d.apply(Point::new(3.0, 1.0)), Point::new(1.0, 3.0)));
        assert!(m.then(m).is_identity());
    }

    #[test]
    fn pow_repeats_a_step() {
        let step = Xform::translate(Point::new(10.0, 5.0));
        assert!(close(
            step.pow(3).apply(Point::ZERO),
            Point::new(30.0, 15.0)
        ));
        assert!(step.pow(0).is_identity());
        let turn = Xform::rotate(Point::ZERO, FRAC_PI_2);
        assert!(close(
            turn.pow(2).apply(Point::new(1.0, 0.0)),
            Point::new(-1.0, 0.0)
        ));
    }

    fn plan() -> (Project, Id, Id) {
        let mut p = Project::new("t");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(200.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let o = p.add_opening(0, w, 60.0, OpeningKind::Door).unwrap();
        (p, w, o)
    }

    #[test]
    fn mirroring_a_wall_keeps_its_door_offset_and_flips_the_swing() {
        let (mut p, w, o) = plan();
        let before = p.floors[0].openings[0].swing_flipped;
        let x = Xform::reflect(Point::new(0.0, 50.0), Point::new(100.0, 50.0));
        let n = p.transform_objects(0, &[ObjectRef::Wall(w)], &x);
        assert_eq!(n, 1);
        let wall = p.floors[0].wall(w).unwrap();
        assert!(close(wall.start, Point::new(0.0, 100.0)));
        assert!(close(wall.end, Point::new(200.0, 100.0)));
        assert_eq!(wall.exterior_side, Side::Right);
        let door = p.floors[0].openings.iter().find(|d| d.id == o).unwrap();
        assert!((door.center_offset - 60.0).abs() < 1e-9);
        assert_eq!(door.swing_flipped, !before);
        // Twice is the original.
        p.transform_objects(0, &[ObjectRef::Wall(w)], &x);
        let wall = p.floors[0].wall(w).unwrap();
        assert!(close(wall.start, Point::new(0.0, 0.0)));
        assert_eq!(wall.exterior_side, Side::Left);
    }

    #[test]
    fn rotating_and_scaling_cad_items() {
        let mut item = CadItem::Arc {
            center: Point::new(10.0, 0.0),
            radius: 5.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        };
        xform_cad_item(&mut item, &Xform::rotate(Point::ZERO, FRAC_PI_2));
        let CadItem::Arc {
            center,
            start_angle,
            end_angle,
            ..
        } = item.clone()
        else {
            unreachable!()
        };
        assert!(close(center, Point::new(0.0, 10.0)));
        assert!((start_angle - FRAC_PI_2).abs() < 1e-9);
        assert!((end_angle - std::f64::consts::PI).abs() < 1e-9);
        xform_cad_item(&mut item, &Xform::scale(Point::ZERO, 2.0));
        let CadItem::Arc { radius, .. } = item else {
            unreachable!()
        };
        assert!((radius - 10.0).abs() < 1e-9);
    }

    #[test]
    fn mirrored_symbols_flip_and_dimensions_swap_sides() {
        let mut s = PlacedSymbol::new("x", Point::new(20.0, 0.0), 10.0, 5.0, 8.0);
        s.angle = 0.0;
        xform_symbol(&mut s, &Xform::reflect(Point::ZERO, Point::new(0.0, 1.0)));
        assert!(close(s.position, Point::new(-20.0, 0.0)));
        assert!(s.flip);
        let mut d = Dimension::new(
            1,
            crate::dimension::DimensionKind::Manual,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            24.0,
        );
        xform_dimension(&mut d, &Xform::reflect(Point::ZERO, Point::new(1.0, 0.0)));
        assert!((d.offset + 24.0).abs() < 1e-9);
    }

    #[test]
    fn align_left_and_center() {
        let boxes = [
            (Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
            (Point::new(30.0, 5.0), Point::new(50.0, 15.0)),
        ];
        let off = align_offsets(&boxes, AlignMode::Left);
        assert_eq!(off, vec![Point::ZERO, Point::new(-30.0, 0.0)]);
        let off = align_offsets(&boxes, AlignMode::Right);
        assert_eq!(off, vec![Point::new(40.0, 0.0), Point::ZERO]);
        let off = align_offsets(&boxes, AlignMode::Middle);
        // Overall y extent 0..15, middle 7.5.
        assert!((off[0].y - 2.5).abs() < 1e-9 && (off[1].y + 2.5).abs() < 1e-9);
        let off = align_offsets(&boxes, AlignMode::Top);
        assert_eq!(off, vec![Point::new(0.0, 5.0), Point::ZERO]);
    }

    #[test]
    fn distribute_makes_equal_gaps() {
        let b = |x0: f64, x1: f64| (Point::new(x0, 0.0), Point::new(x1, 10.0));
        let boxes = [b(0.0, 10.0), b(12.0, 22.0), b(90.0, 100.0)];
        let off = distribute_offsets(&boxes, Axis::Horizontal, None);
        // Span 100, widths 30, gaps 35 each: the middle box moves to 45..55.
        assert!((off[1].x - 33.0).abs() < 1e-9);
        assert_eq!(off[0], Point::ZERO);
        assert_eq!(off[2], Point::ZERO);
        // A fixed gap keeps the first box and chains the rest.
        let off = distribute_offsets(&boxes, Axis::Horizontal, Some(5.0));
        assert!((off[1].x - 3.0).abs() < 1e-9);
        // The last box goes from 90..100 to 30..40 (10 + 5 + 10 + 5).
        assert!((off[2].x + 60.0).abs() < 1e-9);
        // Two boxes without a gap: nothing to do.
        let off = distribute_offsets(&boxes[..2], Axis::Horizontal, None);
        assert!(off.iter().all(|p| *p == Point::ZERO));
    }

    #[test]
    fn parallel_and_perpendicular_ends() {
        let s = Point::new(0.0, 0.0);
        let e = Point::new(100.0, 10.0);
        let par = parallel_end(s, e, Point::new(0.0, 1.0), false);
        assert!(close(par, Point::new(0.0, e.length())));
        let perp = parallel_end(s, e, Point::new(1.0, 0.0), true);
        assert!(close(perp, Point::new(0.0, e.length())));
        // The sense nearest the present direction wins.
        let back = parallel_end(s, Point::new(-50.0, 1.0), Point::new(1.0, 0.0), false);
        assert!(back.x < 0.0);
    }
}
