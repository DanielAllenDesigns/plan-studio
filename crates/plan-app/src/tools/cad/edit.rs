//! The click-driven CAD edit tools, CAD blocks and hatching, as more modes of
//! the CAD tool (CAD-20 .. CAD-35 in `docs/parity/dimensions-text-cad.md`):
//!
//! * Fillet and Chamfer (two lines, or one polyline corner), Offset, Trim
//!   Line, Extend Line, Break Line, Reverse Direction, Make Parallel and Make
//!   Perpendicular work on the CAD objects clicked;
//! * Convert to Polyline / Spline and Convert Polyline to Lines work on the
//!   selection;
//! * Hatch fills a closed polyline or circle with a `plan_materials` pattern
//!   drawn as real lines (grouped with the outline), so it draws and prints
//!   without a renderer change;
//! * CAD blocks: Add Insertion Point, Add Arrow Backoff Point, Edit CAD Block,
//!   CAD Block Management and Insert CAD Block, on top of the blocks the
//!   model keeps (`plan_core::cad`);
//! * Delete Temporary Points, CAD Detail From View, and the dialogs the block
//!   commands open (shown from [`Tool::frame`]).

use super::*;
use crate::dialogs::cad::blocks::{BlockEditDialog, BlockManager, BlockRow, ManagerAction};
use crate::dialogs::Outcome as BlockOutcome;
use plan_core::cad::{
    break_polyline, break_segment, chamfer_lines_picked, chamfer_polyline_vertex, detail_items,
    extend_segment, fillet_lines_picked, fillet_polyline_vertex, lines_to_polylines, make_parallel,
    make_perpendicular, offset_polyline, offset_segment, polyline_to_lines, reverse_item,
    trim_segment, CadAttrs, CadBlockInfo, CadObject, FillAttr,
};
use plan_core::geometry::{dist_to_segment, point_in_polygon};
use plan_core::layers::Layer;
use plan_materials::{clip_strokes_to_polygon, pattern_strokes, Pattern};

/// The layer Place Point and Input Point drop their points on.
pub const TEMP_POINT_LAYER: &str = "CAD, Temporary Points";
/// Hatches with more lines than this are refused.
const MAX_HATCH_LINES: usize = 4000;
/// Hatch patterns of the option strip: `(name, uses the spacing)`.
pub const HATCHES: [(&str, bool); 11] = [
    ("Solid", false),
    ("Diagonal Lines", true),
    ("Cross Hatch", true),
    ("Horizontal Lines", true),
    ("Vertical Lines", true),
    ("Brick", false),
    ("Block", false),
    ("Tile", true),
    ("Insulation", false),
    ("Concrete", false),
    ("Earth", false),
];

/// The `plan_materials` pattern of hatch choice `i`; `None` for Solid.
pub fn hatch_pattern(i: usize, spacing: f64) -> Option<Pattern> {
    let sp = spacing.max(0.5);
    Some(match i {
        1 => Pattern::Lines {
            angle_deg: 45.0,
            spacing: sp,
        },
        2 => Pattern::CrossHatch {
            angle_deg: 45.0,
            spacing: sp,
        },
        3 => Pattern::Lines {
            angle_deg: 0.0,
            spacing: sp,
        },
        4 => Pattern::Lines {
            angle_deg: 90.0,
            spacing: sp,
        },
        5 => Pattern::brick(),
        6 => Pattern::block(),
        7 => Pattern::Tile {
            w: sp * 2.0,
            h: sp * 2.0,
        },
        8 => Pattern::Insulation,
        9 => Pattern::Concrete,
        10 => Pattern::Earth,
        _ => return None,
    })
}

/// The CAD edit tools as Edit toolbar commands: `(mode, command id)`. Run a
/// command with [`run_edit_command`]; the ids are `EditActionKind::Custom` ids.
pub const EDIT_COMMANDS: [(CadMode, &str); 15] = [
    (CadMode::Fillet, "cad.fillet"),
    (CadMode::Chamfer, "cad.chamfer"),
    (CadMode::Offset, "cad.offset"),
    (CadMode::Trim, "cad.trim"),
    (CadMode::Extend, "cad.extend"),
    (CadMode::BreakLine, "cad.break"),
    (CadMode::ChangeLineArc, "cad.change_line_arc"),
    (CadMode::DeleteBreak, "cad.delete_break"),
    (CadMode::MakeArcTangent, "cad.arc_tangent"),
    (CadMode::ReverseDirection, "cad.reverse"),
    (CadMode::MakeParallel, "cad.parallel"),
    (CadMode::MakePerpendicular, "cad.perpendicular"),
    (CadMode::ConvertToPolyline, "cad.to_polyline"),
    (CadMode::ConvertToSpline, "cad.to_spline"),
    (CadMode::PolylineToLines, "cad.polyline_to_lines"),
];

/// The Edit toolbar buttons of the CAD edit tools (Fillet, Chamfer, Offset,
/// Trim, Extend, Break, Reverse Direction, Make Parallel / Perpendicular and
/// the converts) when a drawn CAD object (not a text) is selected.
pub fn edit_actions(cx: &EditorContext) -> Vec<crate::editor::EditAction> {
    use crate::editor::{EditAction, EditActionKind};
    let floor = cx.floor();
    let drawn = cx.selection.items.iter().any(|o| match o {
        ObjectRef::Cad(id) => floor
            .cad
            .iter()
            .any(|c| c.id == *id && !matches!(c.item, plan_core::cad::CadItem::Text { .. })),
        _ => false,
    });
    if !drawn {
        return Vec::new();
    }
    EDIT_COMMANDS
        .iter()
        .map(|(mode, id)| {
            let label = mode.name();
            EditAction {
                kind: EditActionKind::Custom {
                    id,
                    label,
                    icon: "",
                },
                label,
                icon: None,
                enabled: true,
            }
        })
        .collect()
}

/// Runs an Edit toolbar command of the CAD edit tools: switches to that mode
/// of the CAD tool (the selection stays, the converts act on it at once).
/// False when `id` is not one of the CAD commands.
pub fn run_edit_command(cx: &mut EditorContext, id: &str) -> bool {
    match EDIT_COMMANDS.iter().find(|(_, c)| *c == id) {
        Some((mode, _)) => {
            cx.requests
                .push(EditorRequest::SetTool(ToolId::CadVariant(*mode)));
            true
        }
        None => false,
    }
}

/// How the Spline tool fits its curve (CAD-29).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplineKind {
    /// Through the points, Catmull-Rom.
    Fit,
    /// Through the points as joined Beziers with an adjustable tension.
    Bezier,
}

/// A click that picked an object and waits for the next click.
#[derive(Clone, Copy, Debug)]
pub struct EditPick {
    pub id: Id,
    pub at: Point,
}

/// The dialogs the block commands open.
pub enum BlockUi {
    Manager(BlockManager),
    Edit(Id, Box<BlockEditDialog>),
}

/// Settings and in-progress state of the edit modes.
pub struct EditState {
    pub radius: f64,
    pub chamfer: (f64, f64),
    /// Offset distance; zero offsets through the clicked point.
    pub offset: f64,
    pub pick: Option<EditPick>,
    /// The block the next insertion/backoff click belongs to.
    pub block: Option<Id>,
    /// The block Insert CAD Block places.
    pub insert: Option<Id>,
    pub hatch: usize,
    pub hatch_spacing: f64,
    pub spline: SplineKind,
    pub tension: f64,
    pub dialog: Option<BlockUi>,
    /// Run the mode's command on the next frame.
    pub pending: bool,
}

impl Default for EditState {
    fn default() -> Self {
        Self {
            radius: 12.0,
            chamfer: (6.0, 6.0),
            offset: 0.0,
            pick: None,
            block: None,
            insert: None,
            hatch: 1,
            hatch_spacing: 6.0,
            spline: SplineKind::Fit,
            tension: 0.5,
            dialog: None,
            pending: false,
        }
    }
}

// ----- pure helpers -----

/// The straight pieces of an item (arcs and circles sampled), for cutters
/// and boundaries.
pub fn item_segments(item: &CadItem) -> Vec<(Point, Point)> {
    let sample = |c: Point, r: f64, a0: f64, sweep: f64, n: usize| -> Vec<(Point, Point)> {
        let at = |k: usize| {
            let a = a0 + sweep * k as f64 / n as f64;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        };
        (0..n).map(|k| (at(k), at(k + 1))).collect()
    };
    match item {
        CadItem::Line { a, b } => vec![(*a, *b)],
        CadItem::Polyline { points, closed } => polyline_to_lines(points, *closed)
            .into_iter()
            .filter_map(|l| match l {
                CadItem::Line { a, b } => Some((a, b)),
                _ => None,
            })
            .collect(),
        CadItem::Circle { center, radius } => sample(*center, *radius, 0.0, TAU, ELLIPSE_SEGMENTS),
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => sample(
            *center,
            *radius,
            *start_angle,
            (end_angle - start_angle).rem_euclid(TAU),
            24,
        ),
        CadItem::Text { .. } => Vec::new(),
    }
}

/// The outline of a closed shape: a closed polyline or a circle.
pub fn closed_outline(item: &CadItem) -> Option<Vec<Point>> {
    match item {
        CadItem::Polyline {
            points,
            closed: true,
        } if points.len() >= 3 => Some(points.clone()),
        CadItem::Circle { center, radius } => Some(
            item_segments(&CadItem::Circle {
                center: *center,
                radius: *radius,
            })
            .iter()
            .map(|s| s.0)
            .collect(),
        ),
        _ => None,
    }
}

/// Index of the vertex of `points` within `tol` of `p` (the nearest).
fn nearest_vertex(points: &[Point], p: Point, tol: f64) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .map(|(i, q)| (i, q.dist(p)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The pieces of a polyline left after trimming the segment nearest `pick`
/// at the `cutters`; each piece is an open polyline. `None` when no cutter
/// crosses that segment.
pub fn trim_polyline(
    points: &[Point],
    closed: bool,
    pick: Point,
    cutters: &[(Point, Point)],
) -> Option<Vec<Vec<Point>>> {
    let n = points.len();
    if n < 2 {
        return None;
    }
    let segs = if closed { n } else { n - 1 };
    let k = (0..segs).min_by(|a, b| {
        let da = dist_to_segment(pick, points[*a], points[(*a + 1) % n]);
        let db = dist_to_segment(pick, points[*b], points[(*b + 1) % n]);
        da.total_cmp(&db)
    })?;
    let seg = (points[k], points[(k + 1) % n]);
    let pieces = trim_segment(seg, pick, cutters)?;
    let before = pieces.iter().find(|p| p.0.dist(seg.0) < 1e-9).map(|p| p.1);
    let after = pieces
        .iter()
        .find(|p| p.1.dist(seg.1) < 1e-9 && p.0.dist(seg.0) >= 1e-9)
        .map(|p| p.0);
    let mut out: Vec<Vec<Point>> = Vec::new();
    if closed {
        // The ring opens at the trimmed segment: points[k+1] .. points[k].
        let mut linear: Vec<Point> = points.to_vec();
        linear.rotate_left((k + 1) % n);
        let mut path = Vec::new();
        path.extend(after);
        path.extend(linear);
        path.extend(before);
        out.push(path);
    } else {
        let mut left: Vec<Point> = points[..=k].to_vec();
        left.extend(before);
        let mut right: Vec<Point> = after.into_iter().collect();
        right.extend_from_slice(&points[k + 1..]);
        out.push(left);
        out.push(right);
    }
    out.retain(|p| p.len() >= 2);
    Some(out)
}

/// An item offset toward `side` by `dist` (zero: through `side`).
pub fn offset_item(item: &CadItem, side: Point, dist: f64) -> Option<CadItem> {
    match item {
        CadItem::Line { a, b } => {
            let dir = b.sub(*a).normalized();
            let signed = side.sub(*a).dot(dir.perp());
            let d = if dist > 0.0 { dist } else { signed.abs() };
            (d > 1e-9).then(|| {
                let (a, b) = offset_segment(*a, *b, d * signed.signum());
                CadItem::Line { a, b }
            })
        }
        CadItem::Polyline { points, closed } => {
            let segs = item_segments(item);
            let (i, _) = segs
                .iter()
                .enumerate()
                .map(|(i, s)| (i, dist_to_segment(side, s.0, s.1)))
                .min_by(|a, b| a.1.total_cmp(&b.1))?;
            let s = segs[i];
            let signed = side.sub(s.0).dot(s.1.sub(s.0).normalized().perp());
            let d = if dist > 0.0 { dist } else { signed.abs() };
            if d <= 1e-9 {
                return None;
            }
            let pts = offset_polyline(points, *closed, d * signed.signum());
            (pts.len() >= 2).then_some(CadItem::Polyline {
                points: pts,
                closed: *closed,
            })
        }
        CadItem::Circle { center, radius } => {
            let to = side.dist(*center);
            let d = if dist > 0.0 {
                dist
            } else {
                (to - radius).abs()
            };
            let r = if to >= *radius {
                radius + d
            } else {
                radius - d
            };
            (r >= MIN_SIZE).then_some(CadItem::Circle {
                center: *center,
                radius: r,
            })
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let to = side.dist(*center);
            let d = if dist > 0.0 {
                dist
            } else {
                (to - radius).abs()
            };
            let r = if to >= *radius {
                radius + d
            } else {
                radius - d
            };
            (r >= MIN_SIZE).then_some(CadItem::Arc {
                center: *center,
                radius: r,
                start_angle: *start_angle,
                end_angle: *end_angle,
            })
        }
        CadItem::Text { .. } => None,
    }
}

/// The hatch lines of `pattern` clipped to `outline`; `None` when there would
/// be too many.
pub fn hatch_lines(outline: &[Point], pattern: &Pattern) -> Option<Vec<(Point, Point)>> {
    let lo = Point::new(
        outline.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
        outline.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
    );
    let hi = Point::new(
        outline
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max),
        outline
            .iter()
            .map(|p| p.y)
            .fold(f64::NEG_INFINITY, f64::max),
    );
    let strokes = pattern_strokes(pattern, (lo, hi), 0.25);
    let clipped = clip_strokes_to_polygon(&strokes, outline);
    (clipped.len() <= MAX_HATCH_LINES).then_some(clipped)
}

fn polygon_area_abs(p: &[Point]) -> f64 {
    polygon_area(p).abs()
}

// ----- the tool's edit modes -----

impl CadTool {
    /// The CAD object (not text, not a data record) under the pointer.
    fn cad_hit(&self, cx: &EditorContext, p: &PointerEvent) -> Option<CadObject> {
        let tol = cx.pick_tol();
        hit_test(cx.floor(), cx.layers(), p.world, tol)
            .into_iter()
            .find_map(|o| match o {
                ObjectRef::Cad(id) => cad_by_id(cx.floor(), id)
                    .filter(|c| !matches!(c.item, CadItem::Text { .. }))
                    .cloned(),
                _ => None,
            })
    }

    fn set_item(cx: &mut EditorContext, id: Id, item: CadItem) {
        let fl = cx.floor;
        if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
            c.item = item;
        }
    }

    /// Removes a CAD object and takes it out of its group; a group (or block)
    /// left with fewer than two members dissolves.
    pub(super) fn remove_clean(cx: &mut EditorContext, id: Id) {
        let fl = cx.floor;
        cx.project.remove_cad(fl, id);
        let me = plan_core::ObjectRef::Cad(id);
        let f = &mut cx.project.floors[fl];
        for g in &mut f.groups {
            g.members.retain(|m| *m != me);
        }
        f.groups.retain(|g| g.members.len() >= 2);
        cx.project.prune_cad_data(fl);
    }

    /// Segments of every visible CAD object but `skip`.
    fn others(cx: &EditorContext, skip: Id) -> Vec<(Point, Point)> {
        cx.floor()
            .cad
            .iter()
            .filter(|c| c.id != skip && cx.layers().is_visible(&c.layer))
            .flat_map(|c| item_segments(&c.item))
            .collect()
    }

    fn lock_ok(cx: &mut EditorContext, id: Id) -> bool {
        cx.check_unlocked(ObjectRef::Cad(id))
    }

    fn done(cx: &mut EditorContext, label: &str) -> ToolResult {
        cx.mark_dirty();
        cx.readout = None;
        ToolResult::committed(label)
    }

    /// A click in one of the pick modes.
    pub(super) fn pick_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        match self.mode {
            CadMode::Fillet | CadMode::Chamfer => self.corner_click(cx, &p),
            CadMode::Offset => self.offset_click(cx, &p),
            CadMode::Trim => self.trim_click(cx, &p),
            CadMode::Extend => self.extend_click(cx, &p),
            CadMode::BreakLine => self.break_click(cx, &p),
            CadMode::ChangeLineArc => self.change_arc_click(cx, &p),
            CadMode::DeleteBreak => self.delete_break_click(cx, &p),
            CadMode::DisconnectEdges => self.disconnect_click(cx, &p),
            CadMode::HideShowEdge => self.edge_visibility_click(cx, &p),
            CadMode::MakeArcTangent => self.arc_tangent_click(cx, &p),
            CadMode::ReverseDirection => self.reverse_click(cx, &p),
            CadMode::MakeParallel | CadMode::MakePerpendicular => self.turn_click(cx, &p),
            CadMode::Hatch => self.hatch_click(cx, &p),
            CadMode::AddInsertionPoint | CadMode::AddBackoffPoint => self.block_point_click(cx, &p),
            CadMode::InsertBlock => self.insert_click(cx, &p),
            _ => ToolResult::consumed(),
        }
    }

    fn corner_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let chamfer = self.mode == CadMode::Chamfer;
        let label = if chamfer { "Chamfer" } else { "Fillet" };
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = format!("{label}: click a line or a polyline corner");
            return ToolResult::consumed();
        };
        let (d0, d1) = self.edit.chamfer;
        match &obj.item {
            CadItem::Polyline { points, closed } => {
                self.edit.pick = None;
                let tol = cx.pick_tol() * 3.0;
                let Some(i) = nearest_vertex(points, p.world, tol) else {
                    cx.status = format!("{label}: click closer to a corner");
                    return ToolResult::consumed();
                };
                let new = if chamfer {
                    chamfer_polyline_vertex(points, *closed, i, d0, d1)
                } else {
                    fillet_polyline_vertex(points, *closed, i, self.edit.radius, 15.0)
                };
                let Some(np) = new else {
                    cx.status = format!("{label}: that corner does not fit those dimensions");
                    return ToolResult::consumed();
                };
                if !Self::lock_ok(cx, obj.id) {
                    return ToolResult::consumed();
                }
                cx.begin_change(label);
                Self::set_item(
                    cx,
                    obj.id,
                    CadItem::Polyline {
                        points: np,
                        closed: *closed,
                    },
                );
                cx.status.clear();
                Self::done(cx, label)
            }
            CadItem::Line { a, b } => {
                let Some(first) = self.edit.pick else {
                    self.edit.pick = Some(EditPick {
                        id: obj.id,
                        at: p.world,
                    });
                    cx.status = format!("{label}: click the second line");
                    return ToolResult::consumed();
                };
                if first.id == obj.id {
                    cx.status = format!("{label}: click a different line");
                    return ToolResult::consumed();
                }
                self.edit.pick = None;
                let Some(CadObject {
                    item: CadItem::Line { a: a0, b: b0 },
                    layer,
                    ..
                }) = cad_by_id(cx.floor(), first.id).cloned()
                else {
                    return ToolResult::consumed();
                };
                let joined = if chamfer {
                    chamfer_lines_picked((a0, b0), first.at, (*a, *b), p.world, d0, d1)
                } else {
                    fillet_lines_picked((a0, b0), first.at, (*a, *b), p.world, self.edit.radius)
                };
                let Some(j) = joined else {
                    cx.status = format!("{label}: those lines cannot be joined that way");
                    return ToolResult::consumed();
                };
                if !Self::lock_ok(cx, first.id) || !Self::lock_ok(cx, obj.id) {
                    return ToolResult::consumed();
                }
                cx.begin_change(label);
                Self::set_item(
                    cx,
                    first.id,
                    CadItem::Line {
                        a: j.first.0,
                        b: j.first.1,
                    },
                );
                Self::set_item(
                    cx,
                    obj.id,
                    CadItem::Line {
                        a: j.second.0,
                        b: j.second.1,
                    },
                );
                if let Some(joiner) = j.joiner {
                    let fl = cx.floor;
                    cx.project.add_cad(fl, layer, joiner);
                }
                cx.status.clear();
                Self::done(cx, label)
            }
            _ => {
                cx.status = format!("{label} works on lines and polyline corners");
                ToolResult::consumed()
            }
        }
    }

    fn offset_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(first) = self.edit.pick else {
            if let Some(obj) = self.cad_hit(cx, p) {
                self.edit.pick = Some(EditPick {
                    id: obj.id,
                    at: p.world,
                });
                cx.status = "Offset: click the side to offset to".into();
            } else {
                cx.status = "Offset: click a line, polyline, circle or arc".into();
            }
            return ToolResult::consumed();
        };
        self.edit.pick = None;
        let Some(obj) = cad_by_id(cx.floor(), first.id).cloned() else {
            return ToolResult::consumed();
        };
        let Some(new) = offset_item(&obj.item, p.world, self.edit.offset) else {
            cx.status = "Offset: nothing to offset to there".into();
            return ToolResult::consumed();
        };
        if cx.layers().is_locked(&obj.layer) {
            cx.status = format!("The layer \"{}\" is locked", obj.layer);
            return ToolResult::consumed();
        }
        cx.begin_change("Offset CAD Object");
        let fl = cx.floor;
        let id = cx.project.add_cad(fl, obj.layer, new);
        cx.selection.set(ObjectRef::Cad(id));
        cx.status.clear();
        Self::done(cx, "Offset CAD Object")
    }

    fn trim_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = "Trim Line: click the part of a line to remove".into();
            return ToolResult::consumed();
        };
        let cutters = Self::others(cx, obj.id);
        let pieces: Vec<CadItem> = match &obj.item {
            CadItem::Line { a, b } => match trim_segment((*a, *b), p.world, &cutters) {
                Some(v) => v.into_iter().map(|(a, b)| CadItem::Line { a, b }).collect(),
                None => {
                    cx.status = "Trim Line: no other object crosses there".into();
                    return ToolResult::consumed();
                }
            },
            CadItem::Polyline { points, closed } => {
                match trim_polyline(points, *closed, p.world, &cutters) {
                    Some(v) => v
                        .into_iter()
                        .map(|points| CadItem::Polyline {
                            points,
                            closed: false,
                        })
                        .collect(),
                    None => {
                        cx.status = "Trim Line: no other object crosses there".into();
                        return ToolResult::consumed();
                    }
                }
            }
            CadItem::Arc { .. } | CadItem::Circle { .. } => {
                match crate::tools::cad_ops::trim_for_tool(cx, &obj.item, p.world, &cutters) {
                    Some(v) => v,
                    None => return ToolResult::consumed(),
                }
            }
            _ => {
                cx.status = "Trim Line works on lines, polylines, arcs and circles".into();
                return ToolResult::consumed();
            }
        };
        if !Self::lock_ok(cx, obj.id) {
            return ToolResult::consumed();
        }
        cx.begin_change("Trim Line");
        let fl = cx.floor;
        let mut it = pieces.into_iter();
        match it.next() {
            Some(first) => Self::set_item(cx, obj.id, first),
            None => Self::remove_clean(cx, obj.id),
        }
        for rest in it {
            cx.project.add_cad(fl, obj.layer.clone(), rest);
        }
        cx.status.clear();
        Self::done(cx, "Trim Line")
    }

    fn extend_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = "Extend Line: click the end of a line".into();
            return ToolResult::consumed();
        };
        if !matches!(obj.item, CadItem::Line { .. }) {
            return crate::tools::cad_ops::extend_for_tool(cx, obj.id, p.world);
        }
        let CadItem::Line { a, b } = obj.item else {
            return ToolResult::consumed();
        };
        let bounds = Self::others(cx, obj.id);
        let Some((na, nb)) = extend_segment((a, b), p.world, &bounds) else {
            cx.status = "Extend Line: nothing in the way to extend to".into();
            return ToolResult::consumed();
        };
        if !Self::lock_ok(cx, obj.id) {
            return ToolResult::consumed();
        }
        cx.begin_change("Extend Line");
        Self::set_item(cx, obj.id, CadItem::Line { a: na, b: nb });
        cx.status.clear();
        Self::done(cx, "Extend Line")
    }

    fn break_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = "Break Line: click a line or polyline".into();
            return ToolResult::consumed();
        };
        let at = self.snap(cx, p);
        let pieces: Vec<CadItem> = match &obj.item {
            CadItem::Line { a, b } => break_segment(*a, *b, at)
                .into_iter()
                .filter(|(s, e)| s.dist(*e) > 1e-6)
                .map(|(a, b)| CadItem::Line { a, b })
                .collect(),
            CadItem::Polyline { points, closed } => break_polyline(points, *closed, at)
                .unwrap_or_default()
                .into_iter()
                .map(|points| CadItem::Polyline {
                    points,
                    closed: false,
                })
                .collect(),
            _ => Vec::new(),
        };
        if pieces.len() < 2 && !matches!(obj.item, CadItem::Polyline { closed: true, .. }) {
            cx.status = "Break Line: click inside the line, not at its end".into();
            return ToolResult::consumed();
        }
        if !Self::lock_ok(cx, obj.id) {
            return ToolResult::consumed();
        }
        cx.begin_change("Break Line");
        let fl = cx.floor;
        let mut it = pieces.into_iter();
        if let Some(first) = it.next() {
            Self::set_item(cx, obj.id, first);
        }
        for rest in it {
            cx.project.add_cad(fl, obj.layer.clone(), rest);
        }
        cx.status.clear();
        Self::done(cx, "Break Line")
    }

    fn reverse_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = "Reverse Direction: click a line or polyline".into();
            return ToolResult::consumed();
        };
        let selected: Vec<Id> = cx
            .selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect();
        let targets = if selected.contains(&obj.id) {
            selected
        } else {
            vec![obj.id]
        };
        let mut changes = Vec::new();
        for id in targets {
            if let Some(mut c) = cad_by_id(cx.floor(), id).cloned() {
                if reverse_item(&mut c.item) {
                    changes.push((id, c.item));
                }
            }
        }
        if changes.is_empty() {
            cx.status = "Only lines and polylines have a direction".into();
            return ToolResult::consumed();
        }
        if changes.iter().any(|(id, _)| !Self::lock_ok(cx, *id)) {
            return ToolResult::consumed();
        }
        cx.begin_change("Reverse Direction");
        for (id, item) in changes {
            Self::set_item(cx, id, item);
        }
        cx.status.clear();
        Self::done(cx, "Reverse Direction")
    }

    /// Make Parallel / Make Perpendicular: click the end of the line to turn,
    /// then the line to match.
    fn turn_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let perp = self.mode == CadMode::MakePerpendicular;
        let label = if perp {
            "Make Perpendicular"
        } else {
            "Make Parallel"
        };
        let Some(obj) = self.cad_hit(cx, p) else {
            cx.status = format!("{label}: click a line");
            return ToolResult::consumed();
        };
        let Some(first) = self.edit.pick else {
            if matches!(obj.item, CadItem::Line { .. }) {
                self.edit.pick = Some(EditPick {
                    id: obj.id,
                    at: p.world,
                });
                cx.status = format!("{label}: click the line to match");
            } else {
                cx.status = format!("{label} turns a line");
            }
            return ToolResult::consumed();
        };
        self.edit.pick = None;
        let Some(CadItem::Line { a, b }) = cad_by_id(cx.floor(), first.id).map(|c| c.item.clone())
        else {
            return ToolResult::consumed();
        };
        let reference = item_segments(&obj.item)
            .into_iter()
            .min_by(|x, y| {
                dist_to_segment(p.world, x.0, x.1).total_cmp(&dist_to_segment(p.world, y.0, y.1))
            })
            .filter(|_| obj.id != first.id);
        let Some(reference) = reference else {
            cx.status = format!("{label}: click a different line");
            return ToolResult::consumed();
        };
        // The end nearer the first click moves; the other stays.
        let moving_is_b = first.at.dist(b) <= first.at.dist(a);
        let (fixed, moving) = if moving_is_b { (a, b) } else { (b, a) };
        let (_, new_moving) = if perp {
            make_perpendicular((fixed, moving), reference)
        } else {
            make_parallel((fixed, moving), reference)
        };
        if !Self::lock_ok(cx, first.id) {
            return ToolResult::consumed();
        }
        cx.begin_change(label);
        let item = if moving_is_b {
            CadItem::Line {
                a: fixed,
                b: new_moving,
            }
        } else {
            CadItem::Line {
                a: new_moving,
                b: fixed,
            }
        };
        Self::set_item(cx, first.id, item);
        cx.status.clear();
        Self::done(cx, label)
    }

    /// Hatch: the smallest closed outline under the click gets the pattern.
    fn hatch_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let best = cx
            .floor()
            .cad
            .iter()
            .filter(|c| cx.layers().is_visible(&c.layer))
            .filter_map(|c| closed_outline(&c.item).map(|o| (c, o)))
            .filter(|(_, o)| point_in_polygon(p.world, o))
            .min_by(|a, b| polygon_area_abs(&a.1).total_cmp(&polygon_area_abs(&b.1)))
            .map(|(c, _)| c.id);
        let Some(id) = best else {
            cx.status = "Hatch: click inside a closed polyline or circle".into();
            return ToolResult::consumed();
        };
        if !Self::lock_ok(cx, id) {
            return ToolResult::consumed();
        }
        let job = match plan_hatch(cx, id, self.edit.hatch, self.edit.hatch_spacing) {
            Ok(j) => j,
            Err(e) => {
                cx.status = e;
                return ToolResult::consumed();
            }
        };
        cx.begin_change("Hatch");
        let name = job.name;
        apply_hatch(cx, job);
        cx.selection.set(ObjectRef::Cad(id));
        cx.status = format!("Hatched with {name}");
        Self::done(cx, "Hatch")
    }

    // ----- blocks -----

    /// The block under the click, or the block of the selection.
    fn block_of(&self, cx: &EditorContext, p: Option<&PointerEvent>) -> Option<Id> {
        let of = |id: Id| {
            let g = cx.floor().group_of(plan_core::ObjectRef::Cad(id))?;
            cx.floor().cad_block(g.id).map(|b| b.group)
        };
        if let Some(p) = p {
            let tol = cx.pick_tol();
            for o in hit_test(cx.floor(), cx.layers(), p.world, tol) {
                if let ObjectRef::Cad(id) = o {
                    if let Some(b) = of(id) {
                        return Some(b);
                    }
                }
            }
        }
        cx.selection.items.iter().find_map(|o| match o {
            ObjectRef::Cad(id) => of(*id),
            _ => None,
        })
    }

    fn block_point_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let insertion = self.mode == CadMode::AddInsertionPoint;
        let what = if insertion {
            "insertion point"
        } else {
            "arrow backoff point"
        };
        let Some(group) = self.edit.block.or_else(|| self.block_of(cx, Some(p))) else {
            cx.status = format!("Click a CAD block first, then its {what}");
            return ToolResult::consumed();
        };
        if self.edit.block.is_none() {
            self.edit.block = Some(group);
            if let Some(first) = cx.floor().group_members_cad(group).first() {
                cx.selection.set(ObjectRef::Cad(*first));
            }
            cx.status = format!("Click the {what}");
            return ToolResult::consumed();
        }
        let at = self.snap(cx, p);
        self.edit.block = None;
        let fl = cx.floor;
        cx.begin_change(self.mode.name());
        let ok = cx.project.edit_cad_block(fl, group, |b| {
            if insertion {
                b.insertion = Some(at);
            } else {
                b.backoff = Some(at);
            }
        });
        if !ok {
            cx.cancel_change();
            return ToolResult::consumed();
        }
        cx.status.clear();
        Self::done(cx, self.mode.name())
    }

    fn insert_click(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(group) = self.edit.insert else {
            cx.status = "Pick a block in CAD Block Management first".into();
            return ToolResult::consumed();
        };
        let at = self.snap(cx, p);
        let fl = cx.floor;
        if cx.layers().is_locked(CAD_LAYER) {
            cx.status = format!("The layer \"{CAD_LAYER}\" is locked");
            return ToolResult::consumed();
        }
        cx.begin_change("Insert CAD Block");
        match cx.project.insert_cad_block(fl, group, at) {
            Some((_, ids)) => {
                if let Some(first) = ids.first() {
                    cx.selection.set(ObjectRef::Cad(*first));
                }
                cx.status.clear();
                Self::done(cx, "Insert CAD Block")
            }
            None => {
                cx.cancel_change();
                cx.status = "That block no longer exists".into();
                self.edit.insert = None;
                ToolResult::consumed()
            }
        }
    }

    // ----- commands -----

    /// Runs a command mode (on the selection, or by opening a dialog).
    pub(super) fn run_command(&mut self, cx: &mut EditorContext, mode: CadMode) -> ToolResult {
        match mode {
            CadMode::MakeBlock => self.make_block(cx),
            CadMode::ExplodeBlock => self.explode_block(cx),
            CadMode::DeleteTempPoints => self.delete_temp_points(cx),
            CadMode::EditBlock => self.open_block_editor(cx),
            CadMode::BlockManagement => {
                self.edit.dialog = Some(BlockUi::Manager(BlockManager::default()));
                ToolResult::consumed()
            }
            CadMode::ConvertToPolyline => self.convert_to_polyline(cx),
            CadMode::ConvertToSpline => self.convert_to_spline(cx),
            CadMode::PolylineToLines => self.polyline_to_lines_cmd(cx),
            CadMode::DetailFromView => self.detail_from_view(cx),
            _ => ToolResult::ignored(),
        }
    }

    fn selected_cad_ids(cx: &EditorContext) -> Vec<Id> {
        cx.selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn delete_temp_points(&mut self, cx: &mut EditorContext) -> ToolResult {
        let ids: Vec<Id> = cx
            .floor()
            .cad
            .iter()
            .filter(|c| c.layer == TEMP_POINT_LAYER)
            .map(|c| c.id)
            .collect();
        if ids.is_empty() {
            cx.status = "There are no temporary points".into();
            return ToolResult::consumed();
        }
        if cx.layers().is_locked(TEMP_POINT_LAYER) {
            cx.status = format!("The layer \"{TEMP_POINT_LAYER}\" is locked");
            return ToolResult::consumed();
        }
        cx.begin_change("Delete Temporary Points");
        for id in &ids {
            Self::remove_clean(cx, *id);
        }
        cx.selection.retain_existing(&cx.project, cx.floor);
        cx.status = format!(
            "Deleted {} temporary point object{}",
            ids.len(),
            if ids.len() == 1 { "" } else { "s" }
        );
        Self::done(cx, "Delete Temporary Points")
    }

    fn open_block_editor(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(group) = self.block_of(cx, None) else {
            cx.status = "Select a CAD block to edit".into();
            return ToolResult::consumed();
        };
        let Some(info) = cx.floor().cad_block(group) else {
            return ToolResult::consumed();
        };
        let (lo, hi) = cx.floor().block_bounds(group);
        let items = cx.floor().group_items(group);
        self.edit.dialog = Some(BlockUi::Edit(
            group,
            Box::new(BlockEditDialog::new(info, (lo, hi), items)),
        ));
        ToolResult::consumed()
    }

    /// Joins the selected lines that touch into polylines.
    fn convert_to_polyline(&mut self, cx: &mut EditorContext) -> ToolResult {
        let ids = Self::selected_cad_ids(cx);
        let lines: Vec<(Id, String, Point, Point)> = ids
            .iter()
            .filter_map(|id| cad_by_id(cx.floor(), *id))
            .filter_map(|c| match c.item {
                CadItem::Line { a, b } => Some((c.id, c.layer.clone(), a, b)),
                _ => None,
            })
            .collect();
        if lines.is_empty() {
            cx.status = "Select the lines to join into a polyline".into();
            return ToolResult::consumed();
        }
        if lines.iter().any(|l| !Self::lock_ok(cx, l.0)) {
            return ToolResult::consumed();
        }
        let layer = lines[0].1.clone();
        let segs: Vec<(Point, Point)> = lines.iter().map(|l| (l.2, l.3)).collect();
        let chains = lines_to_polylines(&segs, 0.05);
        cx.begin_change("Convert to Polyline");
        for l in &lines {
            Self::remove_clean(cx, l.0);
        }
        let fl = cx.floor;
        let mut made = Vec::new();
        for (points, closed) in chains {
            made.push(
                cx.project
                    .add_cad(fl, layer.clone(), CadItem::Polyline { points, closed }),
            );
        }
        cx.selection.items = made.iter().map(|i| ObjectRef::Cad(*i)).collect();
        cx.status = format!(
            "Joined {} line{} into {} polyline{}",
            lines.len(),
            if lines.len() == 1 { "" } else { "s" },
            made.len(),
            if made.len() == 1 { "" } else { "s" }
        );
        Self::done(cx, "Convert to Polyline")
    }

    /// Smooths the selected polylines through their vertices.
    fn convert_to_spline(&mut self, cx: &mut EditorContext) -> ToolResult {
        let ids = Self::selected_cad_ids(cx);
        let mut changes = Vec::new();
        for id in ids {
            if let Some(CadItem::Polyline { points, closed }) =
                cad_by_id(cx.floor(), id).map(|c| c.item.clone())
            {
                if points.len() >= 3 {
                    changes.push((
                        id,
                        CadItem::Polyline {
                            points: catmull_rom(&points, closed, SPLINE_SEGMENTS_PER_SPAN),
                            closed,
                        },
                    ));
                }
            }
        }
        if changes.is_empty() {
            cx.status = "Select polylines with three or more points".into();
            return ToolResult::consumed();
        }
        if changes.iter().any(|c| !Self::lock_ok(cx, c.0)) {
            return ToolResult::consumed();
        }
        cx.begin_change("Convert to Spline");
        let n = changes.len();
        for (id, item) in changes {
            Self::set_item(cx, id, item);
        }
        cx.status = format!(
            "Converted {n} polyline{} to splines",
            if n == 1 { "" } else { "s" }
        );
        Self::done(cx, "Convert to Spline")
    }

    /// Splits the selected polylines into separate lines.
    fn polyline_to_lines_cmd(&mut self, cx: &mut EditorContext) -> ToolResult {
        let ids = Self::selected_cad_ids(cx);
        let polys: Vec<(Id, String, Vec<Point>, bool)> = ids
            .iter()
            .filter_map(|id| cad_by_id(cx.floor(), *id))
            .filter_map(|c| match &c.item {
                CadItem::Polyline { points, closed } if points.len() >= 2 => {
                    Some((c.id, c.layer.clone(), points.clone(), *closed))
                }
                _ => None,
            })
            .collect();
        if polys.is_empty() {
            cx.status = "Select the polylines to split into lines".into();
            return ToolResult::consumed();
        }
        if polys.iter().any(|p| !Self::lock_ok(cx, p.0)) {
            return ToolResult::consumed();
        }
        cx.begin_change("Convert Polyline to Lines");
        let fl = cx.floor;
        let mut made = Vec::new();
        for (id, layer, points, closed) in polys {
            Self::remove_clean(cx, id);
            for l in polyline_to_lines(&points, closed) {
                made.push(cx.project.add_cad(fl, layer.clone(), l));
            }
        }
        cx.selection.items = made.iter().map(|i| ObjectRef::Cad(*i)).collect();
        cx.status = format!("Made {} lines", made.len());
        Self::done(cx, "Convert Polyline to Lines")
    }

    /// Copies the lines of this view into a new "CAD Detail" floor above it.
    fn detail_from_view(&mut self, cx: &mut EditorContext) -> ToolResult {
        let fl = cx.floor;
        let items = detail_items(&cx.project.floors[fl], cx.layers(), CAD_LAYER);
        let dims = cx.project.floors[fl].dimensions.clone();
        if items.is_empty() && dims.is_empty() {
            cx.status = "There are no lines in this view to copy".into();
            return ToolResult::consumed();
        }
        cx.begin_change("CAD Detail From View");
        let Some(at) = cx.project.insert_floor_above(fl) else {
            cx.cancel_change();
            cx.status = "No floor can be added above this one".into();
            return ToolResult::consumed();
        };
        let taken: Vec<String> = cx.project.floors.iter().map(|f| f.name.clone()).collect();
        let name = (1..)
            .map(|n| {
                if n == 1 {
                    "CAD Detail".to_string()
                } else {
                    format!("CAD Detail {n}")
                }
            })
            .find(|n| !taken.contains(n))
            .unwrap_or_else(|| "CAD Detail".into());
        cx.project.floors[at].name = name.clone();
        let n = items.len() + dims.len();
        for (layer, item) in items {
            cx.project.add_cad(at, layer, item);
        }
        for d in dims {
            cx.project.add_dimension(at, d);
        }
        cx.floor = at;
        cx.selection.clear();
        cx.mark_dirty();
        cx.refresh();
        cx.status = format!("Copied {n} objects into \"{name}\"");
        ToolResult::committed("CAD Detail From View")
    }

    // ----- settings typed with Enter -----

    pub(super) fn start_typed_setting(&mut self, cx: &mut EditorContext) {
        let fields = match self.mode {
            CadMode::Fillet => vec![("Radius", fmt_len(self.edit.radius), false)],
            CadMode::Chamfer => vec![
                ("Distance 1", fmt_len(self.edit.chamfer.0), false),
                ("Distance 2", fmt_len(self.edit.chamfer.1), false),
            ],
            CadMode::Offset => vec![("Distance", fmt_len(self.edit.offset), false)],
            _ => return,
        };
        self.typed = Some(Typed::new(TypedKind::Setting, fields));
        set_typing(cx, true);
    }

    /// Enter in the typed setting fields.
    pub(super) fn commit_setting(&mut self, cx: &mut EditorContext, t: &Typed) -> ToolResult {
        let ok = match self.mode {
            CadMode::Fillet => t
                .value(0)
                .filter(|v| *v >= 0.0)
                .map(|v| self.edit.radius = v),
            CadMode::Chamfer => t
                .value(0)
                .zip(t.value(1))
                .filter(|(a, b)| *a > 0.0 && *b > 0.0)
                .map(|v| self.edit.chamfer = v),
            CadMode::Offset => t
                .value(0)
                .filter(|v| *v >= 0.0)
                .map(|v| self.edit.offset = v),
            _ => None,
        };
        if ok.is_none() {
            cx.status = "Enter a valid distance".into();
            return ToolResult::consumed();
        }
        self.typed = None;
        set_typing(cx, false);
        cx.status = self.mode.hint().into();
        ToolResult::consumed()
    }

    // ----- the option strip -----

    /// The buttons of the current mode's settings.
    pub(super) fn setting_buttons(&self) -> Vec<StripButton> {
        fn b(label: impl Into<String>, id: u16, on: bool) -> StripButton {
            StripButton::new(label, id, on)
        }
        match self.mode {
            CadMode::Fillet => vec![
                b("Radius \u{2212}", BTN_SET_MINUS, false),
                b(
                    format!("R {}", fmt_len(self.edit.radius)),
                    BTN_SET_LABEL,
                    true,
                ),
                b("Radius +", BTN_SET_PLUS, false),
            ],
            CadMode::Chamfer => vec![
                b("Dist \u{2212}", BTN_SET_MINUS, false),
                b(
                    format!(
                        "{} x {}",
                        fmt_len(self.edit.chamfer.0),
                        fmt_len(self.edit.chamfer.1)
                    ),
                    BTN_SET_LABEL,
                    true,
                ),
                b("Dist +", BTN_SET_PLUS, false),
            ],
            CadMode::Offset => vec![
                b("Dist \u{2212}", BTN_SET_MINUS, false),
                b(
                    if self.edit.offset > 0.0 {
                        fmt_len(self.edit.offset)
                    } else {
                        "Through click".to_string()
                    },
                    BTN_SET_LABEL,
                    true,
                ),
                b("Dist +", BTN_SET_PLUS, false),
            ],
            CadMode::Hatch => {
                let (name, spaced) = HATCHES[self.edit.hatch.min(HATCHES.len() - 1)];
                let mut v = vec![b(format!("Pattern: {name}"), BTN_HATCH_NEXT, true)];
                if spaced {
                    v.push(b("Spacing \u{2212}", BTN_SET_MINUS, false));
                    v.push(b(fmt_len(self.edit.hatch_spacing), BTN_SET_LABEL, true));
                    v.push(b("Spacing +", BTN_SET_PLUS, false));
                }
                v
            }
            CadMode::Spline => vec![
                b(
                    "Fit points",
                    BTN_SPLINE_FIT,
                    self.edit.spline == SplineKind::Fit,
                ),
                b(
                    "Bezier",
                    BTN_SPLINE_BEZIER,
                    self.edit.spline == SplineKind::Bezier,
                ),
                b("Tension \u{2212}", BTN_SET_MINUS, false),
                b(format!("{:.1}", self.edit.tension), BTN_SET_LABEL, true),
                b("Tension +", BTN_SET_PLUS, false),
            ],
            _ => Vec::new(),
        }
    }

    /// A click on one of the setting buttons. Returns whether it was one.
    pub(super) fn setting_click(&mut self, id: u16) -> bool {
        let step = |v: f64| {
            if v >= 24.0 {
                6.0
            } else if v >= 6.0 {
                1.0
            } else {
                0.5
            }
        };
        match id {
            BTN_SET_MINUS | BTN_SET_PLUS => {
                let up = id == BTN_SET_PLUS;
                let apply = |v: &mut f64, floor: f64| {
                    let s = step(*v);
                    *v = if up { *v + s } else { (*v - s).max(floor) };
                };
                match self.mode {
                    CadMode::Fillet => apply(&mut self.edit.radius, 0.0),
                    CadMode::Chamfer => {
                        apply(&mut self.edit.chamfer.0, 0.5);
                        self.edit.chamfer.1 = self.edit.chamfer.0;
                    }
                    CadMode::Offset => apply(&mut self.edit.offset, 0.0),
                    CadMode::Hatch => apply(&mut self.edit.hatch_spacing, 0.5),
                    CadMode::Spline => {
                        let t = self.edit.tension + if up { 0.1 } else { -0.1 };
                        self.edit.tension = t.clamp(0.1, 1.0);
                    }
                    _ => {}
                }
                true
            }
            BTN_SET_LABEL => true,
            BTN_HATCH_NEXT => {
                self.edit.hatch = (self.edit.hatch + 1) % HATCHES.len();
                true
            }
            BTN_SPLINE_FIT => {
                self.edit.spline = SplineKind::Fit;
                true
            }
            BTN_SPLINE_BEZIER => {
                self.edit.spline = SplineKind::Bezier;
                true
            }
            _ => false,
        }
    }

    // ----- dialogs (per frame) -----

    fn block_rows(cx: &EditorContext) -> Vec<BlockRow> {
        let f = cx.floor();
        f.cad_blocks()
            .into_iter()
            .map(|info| BlockRow {
                objects: f.group_members_cad(info.group).len(),
                info,
            })
            .collect()
    }

    /// Shows the open block dialog and applies what it asked for.
    pub(super) fn frame_dialogs(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        let Some(ui) = self.edit.dialog.take() else {
            return;
        };
        match ui {
            BlockUi::Manager(mut m) => {
                let rows = Self::block_rows(cx);
                let action = m.show(ctx, &rows);
                self.manager_action(cx, m, action);
            }
            BlockUi::Edit(g, mut d) => {
                let out = d.show(ctx);
                self.edit_outcome(cx, g, d, out);
            }
        }
    }

    /// Applies what the management dialog asked for.
    pub(super) fn manager_action(
        &mut self,
        cx: &mut EditorContext,
        m: BlockManager,
        action: ManagerAction,
    ) {
        let keep = |this: &mut Self, m: BlockManager| this.edit.dialog = Some(BlockUi::Manager(m));
        let fl = cx.floor;
        match action {
            ManagerAction::None => keep(self, m),
            ManagerAction::Close => self.finish_command(cx),
            ManagerAction::Rename(g, name) => {
                cx.begin_change("Rename CAD Block");
                if cx.project.edit_cad_block(fl, g, |b| b.name = name) {
                    cx.mark_dirty();
                } else {
                    cx.cancel_change();
                }
                keep(self, m);
            }
            ManagerAction::Edit(g) => {
                if let Some(info) = cx.floor().cad_block(g) {
                    let (lo, hi) = cx.floor().block_bounds(g);
                    let items = cx.floor().group_items(g);
                    self.edit.dialog = Some(BlockUi::Edit(
                        g,
                        Box::new(BlockEditDialog::new(info, (lo, hi), items)),
                    ));
                } else {
                    keep(self, m);
                }
            }
            ManagerAction::Insert(g) => {
                self.set_mode(CadMode::InsertBlock);
                self.edit.insert = Some(g);
                cx.status = CadMode::InsertBlock.hint().into();
            }
            ManagerAction::Explode(g) => {
                cx.begin_change("Explode CAD Block");
                match cx.project.explode_cad_block(fl, g) {
                    Some(ids) => {
                        cx.selection.items = ids.into_iter().map(ObjectRef::Cad).collect();
                        cx.mark_dirty();
                    }
                    None => cx.cancel_change(),
                }
                keep(self, m);
            }
            ManagerAction::Delete(g) => {
                cx.begin_change("Delete CAD Block");
                if cx.project.delete_cad_block(fl, g) > 0 {
                    cx.selection.retain_existing(&cx.project, cx.floor);
                    cx.mark_dirty();
                } else {
                    cx.cancel_change();
                }
                keep(self, m);
            }
        }
    }

    /// Applies the block specification when it closes.
    pub(super) fn edit_outcome(
        &mut self,
        cx: &mut EditorContext,
        g: Id,
        d: Box<BlockEditDialog>,
        out: BlockOutcome,
    ) {
        match out {
            BlockOutcome::Open => self.edit.dialog = Some(BlockUi::Edit(g, d)),
            BlockOutcome::Cancel => self.finish_command(cx),
            BlockOutcome::Ok => {
                let draft: CadBlockInfo = d.draft().clone();
                if cx.floor().cad_block(g).as_ref() != Some(&draft) {
                    cx.begin_change("Edit CAD Block");
                    let fl = cx.floor;
                    if cx.project.edit_cad_block(fl, g, |b| *b = draft) {
                        cx.mark_dirty();
                    } else {
                        cx.cancel_change();
                    }
                }
                self.finish_command(cx);
            }
        }
    }

    /// A command that opened a dialog is done: back to Select Objects.
    fn finish_command(&mut self, cx: &mut EditorContext) {
        self.edit.dialog = None;
        cx.requests.push(EditorRequest::SetTool(ToolId::Select));
    }

    // ----- overlay -----

    /// Highlights the first pick and the block points.
    pub(super) fn draw_edit_overlay(
        &self,
        cx: &EditorContext,
        painter: &egui::Painter,
        cam: &Camera,
    ) {
        let pal = &cx.palette;
        if let Some(first) = self.edit.pick {
            if let Some(c) = cad_by_id(cx.floor(), first.id) {
                render::draw_cad(
                    painter,
                    cam,
                    &c.item,
                    Stroke::new(2.5_f32, pal.selection),
                    pal,
                );
            }
        }
        let blocks_visible = matches!(
            self.mode,
            CadMode::MakeBlock
                | CadMode::ExplodeBlock
                | CadMode::AddInsertionPoint
                | CadMode::AddBackoffPoint
                | CadMode::EditBlock
                | CadMode::BlockManagement
                | CadMode::InsertBlock
        );
        if blocks_visible {
            let sel = Stroke::new(1.5_f32, pal.selection);
            for b in cx.floor().cad_blocks() {
                if let Some(p) = b.insertion {
                    let c = cam.world_to_screen(p);
                    painter.circle_stroke(c, 6.0, sel);
                    painter.line_segment([c - Vec2::new(9.0, 0.0), c + Vec2::new(9.0, 0.0)], sel);
                    painter.line_segment([c - Vec2::new(0.0, 9.0), c + Vec2::new(0.0, 9.0)], sel);
                }
                if let Some(p) = b.backoff {
                    let c = cam.world_to_screen(p);
                    let d = 6.0;
                    painter.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], sel);
                    painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], sel);
                }
            }
        }
        // The block being inserted follows the pointer as its outline.
        if let (CadMode::InsertBlock, Some(g), Some(h)) = (self.mode, self.edit.insert, self.hover)
        {
            if let Some(info) = cx.floor().cad_block(g) {
                let (lo, hi) = cx.floor().block_bounds(g);
                let from = info.insertion.unwrap_or_else(|| Point::lerp(lo, hi, 0.5));
                let d = h.sub(from);
                let ghost = Stroke::new(1.0_f32, pal.ghost_stroke);
                for item in cx.floor().group_items(g) {
                    render::draw_cad(
                        painter,
                        cam,
                        &plan_core::cad::translated(&item, d),
                        ghost,
                        pal,
                    );
                }
            }
        }
    }
}

/// A hatch worked out but not yet drawn (see [`plan_hatch`]).
pub struct HatchJob {
    id: Id,
    layer: String,
    pub name: &'static str,
    spacing: f64,
    lines: Vec<(Point, Point)>,
    solid: bool,
}

/// Works out the hatch of choice `choice` (an index into [`HATCHES`]) for the
/// closed shape `id`. `Err` says why it cannot be drawn.
pub fn plan_hatch(
    cx: &EditorContext,
    id: Id,
    choice: usize,
    spacing: f64,
) -> Result<HatchJob, String> {
    let obj = cad_by_id(cx.floor(), id).ok_or("That object no longer exists")?;
    let outline =
        closed_outline(&obj.item).ok_or("Only closed polylines and circles take a fill")?;
    let choice = choice.min(HATCHES.len() - 1);
    let pattern = hatch_pattern(choice, spacing);
    let lines = match &pattern {
        Some(pt) => {
            hatch_lines(&outline, pt).ok_or("Hatch: too many lines; increase the spacing")?
        }
        None => Vec::new(),
    };
    Ok(HatchJob {
        id,
        layer: obj.layer.clone(),
        name: HATCHES[choice].0,
        spacing,
        lines,
        solid: pattern.is_none(),
    })
}

/// Draws a planned hatch: removes the lines of the shape's earlier hatch,
/// adds the new ones grouped with the outline and records the fill. The caller
/// begins the undo step.
pub fn apply_hatch(cx: &mut EditorContext, job: HatchJob) {
    let fl = cx.floor;
    let id = job.id;
    let old: Vec<Id> = cx
        .floor()
        .cad_attrs(id)
        .and_then(|a| a.fill)
        .map(|f| f.lines)
        .unwrap_or_default();
    for o in old {
        CadTool::remove_clean(cx, o);
    }
    let mut members = vec![plan_core::ObjectRef::Cad(id)];
    let mut line_ids = Vec::new();
    for (a, b) in job.lines {
        let lid = cx
            .project
            .add_cad(fl, job.layer.clone(), CadItem::Line { a, b });
        line_ids.push(lid);
        members.push(plan_core::ObjectRef::Cad(lid));
    }
    if members.len() > 1 {
        cx.project.make_group(fl, &members);
    }
    let (name, spacing, solid) = (job.name, job.spacing, job.solid);
    cx.project.edit_cad_attrs(fl, id, |at| {
        let keep = at.fill.take().unwrap_or_default();
        at.fill = Some(FillAttr {
            pattern: if solid {
                String::new()
            } else {
                name.to_string()
            },
            spacing,
            lines: line_ids,
            ..keep
        });
    });
    cx.mark_dirty();
}

/// Feet and inches for the option strip and typed fields.
pub fn fmt_len(inches: f64) -> String {
    plan_core::units::fmt_ft_in_frac(inches, 16)
}

/// Makes sure a layer exists (hidden-by-default layers are not made here).
pub fn ensure_layer(project: &mut plan_core::Project, name: &str, color: [u8; 3], weight: u32) {
    if project.layers.get(name).is_none() {
        project.layers.add(Layer::new(name, color, weight));
    }
}

/// Attributes of a hatch boundary, for the specification dialog.
pub fn fill_of(cx: &EditorContext, id: Id) -> Option<CadAttrs> {
    cx.floor().cad_attrs(id)
}
