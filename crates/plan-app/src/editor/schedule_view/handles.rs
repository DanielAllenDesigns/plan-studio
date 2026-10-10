//! The edit handles of a selected schedule (manual p. 711): Move at the
//! middle, Resize on the two vertical edges, Rotate below, Resize Column in
//! the title row, Move Row in the first column, Move Column and Sort by
//! Column in the heading row, and the Wrap diamond on the bottom (right,
//! when swapped) edge. Positions are plan points; the changes they make are
//! pure functions of the schedule as it was when the drag began.

use super::layout::{Frame, Layout};
use plan_core::geometry::Point;
use plan_core::schedules::{ColumnSpec, Schedule, WrapBy};

/// Pixels a handle can be missed by.
pub const HANDLE_PX: f32 = 8.0;

/// What a handle looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleShape {
    Circle,
    Square,
    Triangle,
    Diamond,
    /// The Sort by Column triangle; `true` points down (descending).
    Sort(bool),
}

/// What dragging a handle does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Move,
    /// The side Resize handles keep the other side where it is.
    ResizeLeft,
    ResizeRight,
    Rotate,
    /// Resize Column: the column (index into the table's columns) to the
    /// left of the grid line.
    ResizeColumn(usize),
    /// Move Row: the object (row of the data table).
    MoveRow(usize),
    /// Move Column: the column (index into the table's columns).
    MoveColumn(usize),
    /// Sort by Column (a click).
    Sort(usize),
    Wrap,
}

/// A handle in the plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handle {
    pub kind: HandleKind,
    pub at: Point,
    pub shape: HandleShape,
}

/// The handles of `s` laid out as `l`.
pub fn handles(s: &Schedule, l: &Layout) -> Vec<Handle> {
    let f = l.frame(s.position);
    let at = |x: f64, y: f64| f.to_plan(x, y);
    let mut v = Vec::new();
    let (w, h) = (l.width, l.height);
    v.push(Handle {
        kind: HandleKind::ResizeLeft,
        at: at(0.0, h / 2.0),
        shape: HandleShape::Circle,
    });
    v.push(Handle {
        kind: HandleKind::ResizeRight,
        at: at(w, h / 2.0),
        shape: HandleShape::Circle,
    });
    v.push(Handle {
        kind: HandleKind::Rotate,
        at: at(w / 2.0, h + 3.0 * l.metrics.body),
        shape: HandleShape::Triangle,
    });
    v.push(Handle {
        kind: HandleKind::Wrap,
        at: if l.swapped {
            at(w, h * 0.75)
        } else {
            at(w * 0.75, h)
        },
        shape: HandleShape::Diamond,
    });
    if !l.swapped {
        for piece in &l.pieces {
            let top = piece.y;
            // Resize Column handles sit on the grid lines of the title row
            // (the heading row when there is no title).
            let band_y = if piece.title && piece.title_h > 0.0 {
                top + piece.title_h / 2.0
            } else {
                top + piece.title_h + piece.row_h.first().copied().unwrap_or(0.0) / 2.0
            };
            let mut x = piece.x;
            for (c, cw) in piece.col_w.iter().enumerate() {
                if c + 1 < piece.col_w.len() {
                    v.push(Handle {
                        kind: HandleKind::ResizeColumn(c),
                        at: at(x + cw, band_y),
                        shape: HandleShape::Circle,
                    });
                }
                if piece.head_rows > 0 {
                    let y = top + piece.title_h + piece.row_h[0] / 2.0;
                    v.push(Handle {
                        kind: HandleKind::MoveColumn(c),
                        at: at(x + cw / 2.0, y),
                        shape: HandleShape::Circle,
                    });
                    v.push(Handle {
                        kind: HandleKind::Sort(c),
                        at: at(x + cw * 0.78, y),
                        shape: HandleShape::Sort(
                            s.sort.descending
                                && l.fields.get(c).is_some_and(|f| *f == s.sort.field),
                        ),
                    });
                }
                x += cw;
            }
            // Move Row handles, centred in the first column.
            let mut y = top + piece.title_h;
            for (r, rh) in piece.row_h.iter().enumerate() {
                if r >= piece.head_rows && !piece.col_w.is_empty() {
                    let obj = piece.first + r - piece.head_rows;
                    // Totals and padding rows have no object behind them.
                    if l.previews.get(obj).is_some_and(Option::is_some)
                        || l.table.rows.get(obj).is_some()
                    {
                        v.push(Handle {
                            kind: HandleKind::MoveRow(obj),
                            at: at(piece.x + piece.col_w[0] / 2.0, y + rh / 2.0),
                            shape: HandleShape::Circle,
                        });
                    }
                }
                y += rh;
            }
        }
    }
    v.push(Handle {
        kind: HandleKind::Move,
        at: at(w / 2.0, h / 2.0),
        shape: HandleShape::Square,
    });
    v
}

/// The handle under `p` (within `tol` plan inches); handles drawn later win,
/// so the Move handle in the middle comes last in the list and loses to the
/// small ones around it.
pub fn hit(hs: &[Handle], p: Point, tol: f64) -> Option<Handle> {
    let mut best: Option<(f64, Handle)> = None;
    for h in hs {
        let d = h.at.dist(p);
        if d <= tol && best.is_none_or(|(b, _)| d < b - 1e-9) {
            best = Some((d, *h));
        }
    }
    best.map(|(_, h)| h)
}

/// The width an auto-sized or sized column has now.
fn width_of(l: &Layout, col: usize) -> f64 {
    l.pieces
        .first()
        .and_then(|p| p.col_w.get(col).copied())
        .unwrap_or(0.0)
}

/// The column of `s` that draws the table's column `i` (a column that is
/// shown but not yet in the list, a custom property flagged "show in
/// schedule", is added to it).
pub fn column_of<'a>(s: &'a mut Schedule, l: &Layout, i: usize) -> Option<&'a mut ColumnSpec> {
    let field = l.fields.get(i)?.clone();
    if !s.columns.iter().any(|c| c.field == field) {
        let title = l.table.columns.get(i).cloned().unwrap_or_default();
        s.columns.push(ColumnSpec::new(&field, &title, true));
    }
    s.columns.iter_mut().find(|c| c.field == field)
}

/// Where the top-left corner has to go so that the point at local `new` of
/// the resized schedule (`size`) lies where local `old` of the old one lay.
pub fn reanchor(
    old: &Frame,
    old_local: (f64, f64),
    size: (f64, f64),
    angle_deg: f64,
    new_local: (f64, f64),
) -> Point {
    let p = old.to_plan(old_local.0, old_local.1);
    let a = angle_deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    let (vx, vy) = (new_local.0 - size.0 / 2.0, size.1 / 2.0 - new_local.1);
    let (rx, ry) = (vx * c - vy * s, vx * s + vy * c);
    Point::new(p.x - size.0 / 2.0 - rx, p.y + size.1 / 2.0 - ry)
}

/// The length of the schedule along the wrap direction if it were not
/// wrapped: the height of one tall table (the width when swapped).
pub fn natural_len(l: &Layout) -> f64 {
    if l.swapped {
        let first = l.pieces.first();
        let head: f64 = first.map_or(0.0, |p| p.col_w.iter().take(p.head_cols).sum());
        head + l
            .pieces
            .iter()
            .map(|p| p.col_w.iter().skip(p.head_cols).sum::<f64>())
            .sum::<f64>()
    } else {
        let first = l.pieces.first();
        let over = first.map_or(0.0, |p| {
            p.title_h + p.row_h.iter().take(p.head_rows).sum::<f64>()
        });
        over + l
            .pieces
            .iter()
            .map(|p| p.row_h.iter().skip(p.head_rows).sum::<f64>())
            .sum::<f64>()
    }
}

/// What a drag of a handle changes in the schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct Dragged {
    pub def: Schedule,
    /// The point of the schedule that must stay put once it has been laid
    /// out again: `(old local point, plan point)` to be re-anchored at the
    /// same local point (`new_local`), `None` when the corner is free.
    pub keep: Option<Keep>,
}

/// See [`Dragged::keep`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keep {
    pub old_local: (f64, f64),
    /// Which local point of the new layout is the same one: `(x, y)` where
    /// `x` is `Some(0.0)` for the left edge or `None` for the right edge.
    pub right_edge: bool,
}

/// The schedule after dragging `kind` from plan point `start` to `now`,
/// starting from `orig` (laid out as `l`). Handles that act on release
/// (Move Row, Sort) change nothing here.
pub fn drag(kind: HandleKind, orig: &Schedule, l: &Layout, start: Point, now: Point) -> Dragged {
    let f = l.frame(orig.position);
    let (sx, _) = f.to_local(start);
    let (nx, ny) = f.to_local(now);
    let dx = nx - sx;
    let mut d = orig.clone();
    let mut keep = None;
    match kind {
        HandleKind::Move => {
            d.position = Point::new(
                orig.position.x + (now.x - start.x),
                orig.position.y + (now.y - start.y),
            );
        }
        HandleKind::ResizeRight | HandleKind::ResizeLeft => {
            let last = l.table.columns.len().saturating_sub(1);
            let own = width_of(l, last);
            let grow = if kind == HandleKind::ResizeRight {
                dx
            } else {
                -dx
            };
            if let Some(c) = column_of(&mut d, l, last) {
                c.width = (own + grow).max(2.0);
            }
            keep = Some(Keep {
                old_local: if kind == HandleKind::ResizeRight {
                    (0.0, 0.0)
                } else {
                    (l.width, 0.0)
                },
                right_edge: kind == HandleKind::ResizeLeft,
            });
        }
        HandleKind::ResizeColumn(i) => {
            let own = width_of(l, i);
            if let Some(c) = column_of(&mut d, l, i) {
                c.width = (own + dx).max(2.0);
            }
            keep = Some(Keep {
                old_local: (0.0, 0.0),
                right_edge: false,
            });
        }
        HandleKind::Rotate => {
            let centre = f.centre;
            let v = (now.x - centre.x, now.y - centre.y);
            if v.0.abs() + v.1.abs() > 1e-6 {
                // The handle sits straight below the middle (-90 degrees).
                let mut a = v.1.atan2(v.0).to_degrees() + 90.0;
                let snapped = (a / 15.0).round() * 15.0;
                a = if (a - snapped).abs() < 2.0 {
                    snapped
                } else {
                    (a * 10.0).round() / 10.0
                };
                d.angle = a.rem_euclid(360.0);
                if d.angle > 180.0 {
                    d.angle -= 360.0;
                }
            }
        }
        HandleKind::MoveColumn(_) | HandleKind::MoveRow(_) | HandleKind::Sort(_) => {}
        HandleKind::Wrap => {
            let natural = natural_len(l);
            let pos = if l.swapped { nx } else { ny };
            // Dragged outward to the size the table would be unwrapped:
            // back to a single table.
            if pos >= natural - 1.0 {
                d.wrap.enabled = false;
            } else {
                d.wrap.enabled = true;
                d.wrap.by = WrapBy::MaxSize(pos.max(1.0));
            }
            keep = Some(Keep {
                old_local: (0.0, 0.0),
                right_edge: false,
            });
        }
    }
    Dragged { def: d, keep }
}

/// Moves the corner of `def` so that `keep` holds, given the new layout `l`
/// of `def` and the frame `old` it had before the change.
pub fn apply_keep(def: &mut Schedule, l: &Layout, old: &Frame, keep: Keep) {
    let new_local = if keep.right_edge {
        (l.width, 0.0)
    } else {
        (0.0, 0.0)
    };
    def.position = reanchor(
        old,
        keep.old_local,
        (l.width, l.height),
        def.angle,
        new_local,
    );
}

/// The index among the table's columns whose horizontal extent holds local
/// `x` (the Move Column target).
pub fn column_at(l: &Layout, x: f64) -> Option<usize> {
    let p = l.pieces.first()?;
    let mut edge = p.x;
    for (c, w) in p.col_w.iter().enumerate() {
        edge += w;
        if x < edge || c + 1 == p.col_w.len() {
            return Some(c);
        }
    }
    None
}

/// The row (object index) whose vertical extent holds local `y`.
pub fn row_at(l: &Layout, y: f64) -> Option<usize> {
    for piece in &l.pieces {
        let mut top = piece.y + piece.title_h;
        for (r, rh) in piece.row_h.iter().enumerate() {
            if r >= piece.head_rows && y >= top && y < top + rh {
                return Some(piece.first + r - piece.head_rows);
            }
            top += rh;
        }
    }
    // Past the last row: the last object.
    l.pieces
        .last()
        .filter(|p| p.count > 0)
        .map(|p| p.first + p.count - 1)
}

/// Moves the column showing field `from` so it stands where the column
/// showing field `to` is (Move Column handles).
pub fn move_column_to(s: &mut Schedule, from: &str, to: &str) -> bool {
    let Some(a) = s.columns.iter().position(|c| c.field == from) else {
        return false;
    };
    let Some(b) = s.columns.iter().position(|c| c.field == to) else {
        return false;
    };
    if a == b {
        return false;
    }
    let c = s.columns.remove(a);
    s.columns.insert(b, c);
    true
}

/// Sort by Column: the first click sorts ascending, the next reverses it.
pub fn toggle_sort(s: &mut Schedule, field: &str) {
    if s.sort.field == field {
        s.sort.descending = !s.sort.descending;
    } else {
        s.sort.field = field.to_string();
        s.sort.descending = false;
    }
}
