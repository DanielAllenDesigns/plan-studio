//! Temporary dimensions (S-56..S-64): shown for the selected wall or opening,
//! never saved. Clicking a value turns it into an edit field; Enter applies it
//! by moving the selected object (the referenced object stays fixed, S-60).

use super::ops;
use super::selection::{ObjectRef, Selection};
use super::{Camera, EditorContext};
use crate::theme::Palette;
use eframe::egui::{self, FontId, Rect, Shape, Stroke, Vec2};
use plan_core::geometry::Point;
use plan_core::units::parse_ft_in;
use plan_core::{DimFormat, Floor, Opening, Wall, WallEnd};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TempDimKind {
    /// The wall's own length (Lock: start stays, the end moves).
    WallLength,
    /// Face-to-face gap to the nearest parallel wall on one side.
    WallGap,
    /// Opening: left jamb to the wall start.
    OpeningToStart,
    /// Opening: right jamb to the wall end.
    OpeningToEnd,
}

#[derive(Clone, Copy, Debug)]
pub struct TempDim {
    pub kind: TempDimKind,
    /// The measured segment.
    pub a: Point,
    pub b: Point,
    pub value: f64,
    /// Unit direction in which moving the selected object *increases* the value.
    pub axis: Point,
    pub target: ObjectRef,
    /// Where the dimension line is drawn relative to the measured segment:
    /// inches plus screen pixels along the segment's left normal.
    pub offset_in: f64,
    pub offset_px: f32,
}

impl TempDim {
    /// The drawn dimension line at screen scale `scale` (pixels per inch).
    pub fn line(&self, scale: f64) -> (Point, Point) {
        let n = self.b.sub(self.a).normalized().perp();
        let off = n.scale(self.offset_in + self.offset_px as f64 / scale.max(1e-6));
        (self.a.add(off), self.b.add(off))
    }

    pub fn label_pos(&self, scale: f64) -> Point {
        let (p, q) = self.line(scale);
        Point::lerp(p, q, 0.5)
    }
}

/// The value being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct EditField {
    pub index: usize,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct TempDims {
    pub dims: Vec<TempDim>,
    pub editing: Option<EditField>,
}

impl TempDims {
    /// Rebuilds the dimensions for the current selection (a single wall or
    /// opening). Kept as is while a value is being typed.
    pub fn compute(&mut self, floor: &Floor, selection: &Selection) {
        if self.editing.is_some() {
            return;
        }
        self.dims.clear();
        match selection.single() {
            Some(ObjectRef::Wall(id)) => wall_dims(floor, id, &mut self.dims),
            Some(ObjectRef::Opening(id)) => opening_dims(floor, id, &mut self.dims),
            _ => {}
        }
    }

    pub fn clear(&mut self) {
        self.dims.clear();
        self.editing = None;
    }

    /// The value label near `p`, if any.
    pub fn hit_label(&self, p: Point, scale: f64) -> Option<usize> {
        let r = 22.0 / scale.max(1e-6);
        self.dims
            .iter()
            .position(|d| d.label_pos(scale).dist(p) <= r)
    }

    pub fn begin_edit(&mut self, index: usize) -> bool {
        match self.dims.get(index) {
            Some(d) => {
                self.editing = Some(EditField {
                    index,
                    text: format!("{:.3}", d.value)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                });
                true
            }
            None => false,
        }
    }

    /// Replaces the field with typed text (the first keystroke clears the
    /// old value).
    pub fn type_text(&mut self, s: &str) {
        if let Some(e) = &mut self.editing {
            e.text.push_str(s);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(e) = &mut self.editing {
            e.text.pop();
        }
    }

    /// Tab: the next dimension of the same object.
    pub fn next_field(&mut self) {
        if let Some(e) = &mut self.editing {
            if !self.dims.is_empty() {
                let next = (e.index + 1) % self.dims.len();
                let value = self.dims[next].value;
                *e = EditField {
                    index: next,
                    text: format!("{value:.3}")
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                };
            }
        }
    }

    pub fn cancel(&mut self) {
        self.editing = None;
    }
}

fn wall_dims(floor: &Floor, id: u64, out: &mut Vec<TempDim>) {
    let Some(s) = floor.wall(id) else { return };
    let (u, n, len) = (s.direction(), s.normal(), s.length());
    if len < 1e-6 {
        return;
    }
    // Nearest parallel wall on each side: (gap, side, overlap midpoint).
    let mut best: [Option<(f64, f64)>; 2] = [None, None];
    for o in &floor.walls {
        if o.id == id || o.length() < 1e-6 || u.cross(o.direction()).abs() > 0.01 {
            continue;
        }
        let off_a = o.start.sub(s.start).dot(n);
        let off_b = o.end.sub(s.start).dot(n);
        if (off_a - off_b).abs() > 0.05 {
            continue;
        }
        let (s0, s1) = (o.start.sub(s.start).dot(u), o.end.sub(s.start).dot(u));
        let lo = s0.min(s1).max(0.0);
        let hi = s0.max(s1).min(len);
        if hi - lo <= 1.0 {
            continue;
        }
        let gap = off_a.abs() - (s.thickness + o.thickness) * 0.5;
        if gap <= 0.01 {
            continue;
        }
        let slot = &mut best[usize::from(off_a < 0.0)];
        if slot.is_none_or(|(g, _)| gap < g) {
            *slot = Some((gap, (lo + hi) * 0.5));
        }
    }
    for (i, side) in [1.0_f64, -1.0].into_iter().enumerate() {
        if let Some((gap, mid)) = best[i] {
            let a = s.start + u * mid + n * (side * s.thickness * 0.5);
            out.push(TempDim {
                kind: TempDimKind::WallGap,
                a,
                b: a + n * (side * gap),
                value: gap,
                axis: n * -side,
                target: ObjectRef::Wall(id),
                offset_in: 0.0,
                offset_px: 0.0,
            });
        }
    }
    out.push(TempDim {
        kind: TempDimKind::WallLength,
        a: s.start,
        b: s.end,
        value: len,
        axis: u,
        target: ObjectRef::Wall(id),
        offset_in: -s.thickness * 0.5,
        offset_px: -14.0,
    });
}

fn opening_dims(floor: &Floor, id: u64, out: &mut Vec<TempDim>) {
    let Some(o) = floor.openings.iter().find(|o| o.id == id) else {
        return;
    };
    let Some(w) = floor.wall(o.wall_id) else {
        return;
    };
    out.extend(jamb_dims(w, o, ObjectRef::Opening(id)));
}

/// The two jamb-to-wall-end dimensions of an opening (also used for the
/// placement ghost).
pub fn jamb_dims(w: &Wall, o: &Opening, target: ObjectRef) -> [TempDim; 2] {
    let len = w.length();
    [
        TempDim {
            kind: TempDimKind::OpeningToStart,
            a: w.point_at(0.0),
            b: w.point_at(o.start_offset()),
            value: o.start_offset(),
            axis: w.direction(),
            target,
            offset_in: w.thickness * 0.5,
            offset_px: 14.0,
        },
        TempDim {
            kind: TempDimKind::OpeningToEnd,
            a: w.point_at(o.end_offset()),
            b: w.point_at(len),
            value: len - o.end_offset(),
            axis: w.direction() * -1.0,
            target,
            offset_in: w.thickness * 0.5,
            offset_px: 14.0,
        },
    ]
}

/// Applies `value` (inches) to `dim`: moves the selected object so the
/// dimension reads `value`. One undo step. Returns the undo label.
pub fn apply(cx: &mut EditorContext, dim: &TempDim, value: f64) -> Result<&'static str, String> {
    if value < 0.0 {
        return Err("A dimension cannot be negative".into());
    }
    let fl = cx.floor;
    let change = value - dim.value;
    match (dim.kind, dim.target) {
        (TempDimKind::WallLength, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            if value < 1.0 {
                return Err("A wall must be at least 1\" long".into());
            }
            cx.begin_change("Change Wall Length");
            let to = w.start + w.direction() * value;
            ops::move_wall_end_joined(&mut cx.project, fl, id, WallEnd::End, to);
            cx.mark_dirty();
            Ok("Change Wall Length")
        }
        (TempDimKind::WallGap, ObjectRef::Wall(id)) => {
            let Some(w) = cx.floor().wall(id).cloned() else {
                return Err("Wall not found".into());
            };
            cx.begin_change("Move Wall");
            let s = change * dim.axis.dot(w.normal());
            ops::move_wall_perpendicular(&mut cx.project, fl, id, s);
            cx.mark_dirty();
            Ok("Move Wall")
        }
        (TempDimKind::OpeningToStart | TempDimKind::OpeningToEnd, ObjectRef::Opening(id)) => {
            let Some(o) = cx.floor().openings.iter().find(|o| o.id == id).cloned() else {
                return Err("Opening not found".into());
            };
            let sign = if dim.kind == TempDimKind::OpeningToStart {
                1.0
            } else {
                -1.0
            };
            let center = o.center_offset + change * sign;
            cx.begin_change("Move Opening");
            if ops::place_opening_at(&mut cx.project, fl, id, o.wall_id, center) {
                cx.mark_dirty();
                Ok("Move Opening")
            } else {
                cx.cancel_change();
                Err("The opening does not fit there".into())
            }
        }
        _ => Err("This dimension cannot be edited".into()),
    }
}

/// Commits the field being typed. Returns the undo label on success.
pub fn commit_edit(cx: &mut EditorContext) -> Result<&'static str, String> {
    let Some(field) = cx.temp.editing.clone() else {
        return Err("Nothing is being edited".into());
    };
    let Some(dim) = cx.temp.dims.get(field.index).copied() else {
        cx.temp.cancel();
        return Err("That dimension is gone".into());
    };
    let Some(value) = parse_ft_in(&field.text) else {
        return Err(format!("\"{}\" is not a length", field.text));
    };
    cx.temp.editing = None;
    apply(cx, &dim, value)
}

/// Draws the dimensions (and the edit field) over the plan.
pub fn draw(
    dims: &TempDims,
    painter: &egui::Painter,
    cam: &Camera,
    pal: &Palette,
    fmt: &DimFormat,
) {
    let scale = cam.px_per_in;
    let line = Stroke::new(1.0_f32, pal.dimension_text);
    let ext = Stroke::new(0.7_f32, pal.dimension_text.gamma_multiply(0.7));
    for (i, d) in dims.dims.iter().enumerate() {
        let (p, q) = d.line(scale);
        let (sa, sb) = (cam.world_to_screen(p), cam.world_to_screen(q));
        painter.line_segment([cam.world_to_screen(d.a), sa], ext);
        painter.line_segment([cam.world_to_screen(d.b), sb], ext);
        painter.line_segment([sa, sb], line);
        let dir = (sb - sa).normalized();
        let tick = Vec2::new(-dir.y, dir.x) * 4.0;
        for s in [sa, sb] {
            painter.line_segment([s - tick, s + tick], line);
        }
        let editing = dims.editing.as_ref().filter(|e| e.index == i);
        let text = match editing {
            Some(e) => format!("{}|", e.text),
            None => fmt.fmt_len(d.value),
        };
        let mid = cam.world_to_screen(d.label_pos(scale));
        let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), pal.dimension_text);
        let rect = Rect::from_center_size(mid, galley.size() + Vec2::new(8.0, 4.0));
        let fill = if editing.is_some() {
            pal.background
        } else {
            pal.background.gamma_multiply(0.85)
        };
        painter.add(Shape::rect_filled(rect, 2.0, fill));
        if editing.is_some() {
            painter.rect_stroke(rect, 2.0, line, egui::StrokeKind::Inside);
        }
        painter.galley(
            rect.center() - galley.size() * 0.5,
            galley,
            pal.dimension_text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::{OpeningKind, Project, WallKind};

    fn plan() -> (Project, u64, u64) {
        let mut p = Project::new("t");
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let mid = p.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(240.0, 100.0),
            4.0,
            100.0,
            WallKind::Interior,
        );
        p.add_wall(
            0,
            Point::new(0.0, 200.0),
            Point::new(240.0, 200.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        (p, a, mid)
    }

    #[test]
    fn wall_shows_gaps_to_both_sides_and_its_length() {
        let (p, _, mid) = plan();
        let mut sel = Selection::default();
        sel.set(ObjectRef::Wall(mid));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel);
        let kinds: Vec<_> = t.dims.iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TempDimKind::WallGap,
                TempDimKind::WallGap,
                TempDimKind::WallLength
            ]
        );
        // 100" centerline spacing minus half of each thickness.
        assert!((t.dims[0].value - 95.0).abs() < 1e-9);
        assert!((t.dims[1].value - 95.0).abs() < 1e-9);
        assert_eq!(t.dims[2].value, 240.0);
    }

    #[test]
    fn opening_shows_distances_to_both_wall_ends() {
        let (mut p, a, _) = plan();
        let o = p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        let mut sel = Selection::default();
        sel.set(ObjectRef::Opening(o));
        let mut t = TempDims::default();
        t.compute(&p.floors[0], &sel);
        assert_eq!(t.dims.len(), 2);
        assert_eq!(t.dims[0].value, 82.0);
        assert_eq!(t.dims[1].value, 240.0 - 118.0);
    }

    #[test]
    fn typing_a_gap_moves_the_wall_and_undoes() {
        let (p, _, mid) = plan();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Wall(mid));
        cx.refresh();
        assert!(cx.temp.begin_edit(0));
        cx.temp.editing.as_mut().unwrap().text = "5'".into();
        // The first gap is on the +normal side (the wall runs +x, so +y).
        let label = commit_edit(&mut cx).unwrap();
        assert_eq!(label, "Move Wall");
        let w = cx.floor().wall(mid).unwrap();
        // The gap to the wall above shrinks from 95" to 60": the selected wall
        // moves 35" toward the fixed reference wall.
        assert!((w.start.y - 135.0).abs() < 1e-9, "{}", w.start.y);
        cx.undo();
        assert_eq!(cx.floor().wall(mid).unwrap().start.y, 100.0);
    }

    #[test]
    fn typing_an_opening_distance_slides_it() {
        let (mut p, a, _) = plan();
        let o = p.add_opening(0, a, 100.0, OpeningKind::Door).unwrap();
        let mut cx = EditorContext::with_project(p, plan_core::PlanDefaults::chief_x18_daniel());
        cx.selection.set(ObjectRef::Opening(o));
        cx.refresh();
        cx.temp.begin_edit(0);
        cx.temp.editing.as_mut().unwrap().text = "60".into();
        commit_edit(&mut cx).unwrap();
        let op = cx.floor().openings.iter().find(|x| x.id == o).unwrap();
        assert_eq!(op.start_offset(), 60.0);
    }
}
