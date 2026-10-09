//! The Reference Display's rows and the construction lines on the plan
//! canvas (manual pp. 83-91; CAD-62..CAD-66, LAY-9, LAY-39..LAY-45, S-194).
//!
//! * [`reference_layers`] resolves the Change Floor/Reference table
//!   ([`plan_core::construction::ReferenceTable`]) into drawable layers: the
//!   walls of the floor each row shows, in this plan or in another plan file
//!   (read-only, cached by file time), already mapped through the row's
//!   offset and angle.
//! * [`draw_reference`] draws them dimmed (edges only without Details; XOR
//!   suppresses lines identical to the current floor's and recolors the
//!   ones laid over it), [`snap_segments`] hands their centerlines and the
//!   construction lines to the snap engine, and [`aligned_wall_ids`] finds
//!   the walls drawn exactly over a reference wall (S-194).
//! * [`draw_construction`] draws the construction lines: infinite ones across
//!   the whole view with their callouts pinned to the view edge, finite ones
//!   with their callouts past their ends.

use super::{Camera, EditorContext};
use eframe::egui::{self, Color32, FontId, Pos2, Shape, Stroke};
use plan_core::construction::{
    self, CalloutEnds, CalloutShape, CalloutSpec, ReferenceRow, ReferenceSource, ReferenceTable,
    ResolvedLine, RowFloor, ViewType,
};
use plan_core::geometry::{point_in_polygon, segment_intersection, Point};
use plan_core::layers::{LayerSet, LineStyle};
use plan_core::{Id, Project};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

/// Walls this close to a reference wall (inches) count as drawn exactly over
/// it (S-194).
pub const ALIGN_TOLERANCE: f64 = 0.25;
/// Reference edges this close to an edge of the current floor are identical
/// for XOR drawing.
pub const XOR_TOLERANCE: f64 = 0.5;
/// The light blue of an aligned wall's edge lines (S-194).
pub const ALIGNED_BLUE: Color32 = Color32::from_rgb(110, 190, 255);

// ---------------------------------------------------------------------------
// Other plan files
// ---------------------------------------------------------------------------

type PlanCache = HashMap<String, (Option<SystemTime>, Option<Rc<Project>>)>;

thread_local! {
    static OTHER_PLANS: RefCell<PlanCache> = RefCell::new(HashMap::new());
}

/// The plan at `path`, read once and again only when the file changes. The
/// file is only read, never written. `None` when it cannot be read.
pub fn other_plan(path: &str) -> Option<Rc<Project>> {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    OTHER_PLANS.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((seen, plan)) = c.get(path) {
            if *seen == mtime {
                return plan.clone();
            }
        }
        let plan = plan_core::io::load_project(std::path::Path::new(path))
            .ok()
            .map(Rc::new);
        c.insert(path.to_string(), (mtime, plan.clone()));
        plan
    })
}

/// Forgets the plans read so far (a changed file is picked up anyway).
#[cfg_attr(not(test), allow(dead_code))]
pub fn forget_other_plans() {
    OTHER_PLANS.with(|c| c.borrow_mut().clear());
}

/// Puts `plan` in the cache as if it had been read from `path` (tests, and
/// plans not yet saved).
#[cfg_attr(not(test), allow(dead_code))]
pub fn preload_other_plan(path: &str, plan: Project) {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    OTHER_PLANS.with(|c| {
        c.borrow_mut()
            .insert(path.to_string(), (mtime, Some(Rc::new(plan))));
    });
}

// ---------------------------------------------------------------------------
// Rows to layers
// ---------------------------------------------------------------------------

/// One wall of a reference layer, in the current plan's coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct RefWall {
    pub footprint: Vec<Point>,
    pub start: Point,
    pub end: Point,
    pub thickness: f64,
}

/// A reference row resolved against the plan: what it draws now.
#[derive(Debug, Clone, PartialEq)]
pub struct RefLayer {
    /// Index into [`ReferenceTable::rows`] (0 for the default row).
    pub row_index: usize,
    pub row: ReferenceRow,
    /// The floor of the source plan the row shows.
    pub floor: usize,
    /// Drawn in front of the plan (above the Current line in the table).
    pub front: bool,
    pub walls: Vec<RefWall>,
}

impl RefLayer {
    /// The corner box of the layer's walls in plan coordinates.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut it = self.walls.iter().flat_map(|w| w.footprint.iter().copied());
        let first = it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        Some((lo, hi))
    }

    /// The footprint polygons.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn polygons(&self) -> Vec<Vec<Point>> {
        self.walls.iter().map(|w| w.footprint.clone()).collect()
    }
}

/// The reference rows in force: the project's table, or, while it is empty,
/// one row from the session's Reference Display choices (the floor below,
/// gray, unless the Reference Display dialog chose otherwise).
pub fn effective_rows(project: &Project) -> Vec<ReferenceRow> {
    if !project.reference_table.rows.is_empty() {
        return project.reference_table.rows.clone();
    }
    vec![crate::dialogs::reference_display::default_row(project)]
}

/// How many rows are drawn in front of the plan.
pub fn rows_in_front(project: &Project) -> usize {
    if project.reference_table.rows.is_empty() {
        0
    } else {
        project.reference_table.current_at
    }
}

/// The walls of the floor `row` shows in `source`, through the layer set of
/// the row. `active` is the active view's layers (used for this plan when the
/// row names no layer set).
fn row_walls(
    source: &Project,
    row: &ReferenceRow,
    floor: usize,
    active: &LayerSet,
    this_plan: bool,
) -> Vec<RefWall> {
    let set_layers = row
        .layer_set
        .as_deref()
        .map(|n| source.layer_sets.effective_for(n, &source.layers));
    let layers: &LayerSet = match (&set_layers, this_plan) {
        (Some(l), _) => l,
        (None, true) => active,
        (None, false) => &source.layers,
    };
    source.floors[floor]
        .walls
        .iter()
        .filter(|w| {
            !w.flags.invisible && layers.is_visible(&w.layer) && layers.shows_in_reference(&w.layer)
        })
        .map(|w| RefWall {
            footprint: w.footprint().iter().map(|p| row.to_world(*p)).collect(),
            start: row.to_world(w.start),
            end: row.to_world(w.end),
            thickness: w.thickness,
        })
        .collect()
}

/// The layers the Reference Display draws now: one per row that has a floor
/// to show. Nothing while Reference Display is off.
pub fn reference_layers(cx: &EditorContext) -> Vec<RefLayer> {
    if !cx
        .view_flags
        .contains(&crate::toolbar::ViewFlag::ReferenceDisplay)
    {
        return Vec::new();
    }
    let front_rows = rows_in_front(&cx.project);
    let mut out = Vec::new();
    for (i, row) in effective_rows(&cx.project).into_iter().enumerate() {
        let (floor, walls) = match &row.source {
            ReferenceSource::ThisPlan => {
                let Some(f) = row.floor.resolve(cx.floor, cx.project.floors.len(), false) else {
                    continue;
                };
                (f, row_walls(&cx.project, &row, f, cx.layers(), true))
            }
            ReferenceSource::File(path) => {
                let Some(plan) = other_plan(path) else {
                    continue;
                };
                let Some(f) = row.floor.resolve(cx.floor, plan.floors.len(), true) else {
                    continue;
                };
                (f, row_walls(&plan, &row, f, cx.layers(), false))
            }
        };
        out.push(RefLayer {
            row_index: i,
            floor,
            front: i < front_rows,
            row,
            walls,
        });
    }
    out
}

/// The centerlines the snap engine gets besides the walls of the active
/// floor: the walls of every reference layer, and the construction lines
/// that are infinite in plan (as very long segments, see
/// [`construction::infinite_segment`]) or drawn on another floor.
pub fn snap_segments(cx: &EditorContext) -> Vec<(Point, Point)> {
    let mut out: Vec<(Point, Point)> = reference_layers(cx)
        .iter()
        .flat_map(|l| l.walls.iter().map(|w| (w.start, w.end)))
        .collect();
    out.extend(construction_snap_segments(cx));
    out
}

/// The construction lines as snap segments (see [`snap_segments`]).
pub fn construction_snap_segments(cx: &EditorContext) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for l in visible_construction(cx) {
        if l.spec.infinite_plan {
            out.push(construction::infinite_segment(l.a, l.b));
        } else if l.floor != cx.floor {
            // The active floor's finite lines are CAD lines already.
            out.push((l.a, l.b));
        }
    }
    out
}

/// The construction lines shown on the active floor whose layer shows.
pub fn visible_construction(cx: &EditorContext) -> Vec<ResolvedLine> {
    cx.project
        .construction_lines_on(cx.floor)
        .into_iter()
        .filter(|l| cx.layers().is_visible(&l.layer))
        .collect()
}

// ---------------------------------------------------------------------------
// Alignment (S-194)
// ---------------------------------------------------------------------------

fn same_wall(a: (Point, Point, f64), b: (Point, Point, f64)) -> bool {
    let near = |p: Point, q: Point| p.dist(q) <= ALIGN_TOLERANCE;
    (b.2 - a.2).abs() <= ALIGN_TOLERANCE
        && ((near(a.0, b.0) && near(a.1, b.1)) || (near(a.0, b.1) && near(a.1, b.0)))
}

/// The walls of the active floor that lie exactly over a wall of the
/// Reference Display: same ends (either way round) and thickness within a
/// quarter inch (S-194).
pub fn aligned_wall_ids(cx: &EditorContext) -> Vec<Id> {
    let layers = reference_layers(cx);
    if layers.is_empty() {
        return Vec::new();
    }
    let refs: Vec<(Point, Point, f64)> = layers
        .iter()
        .flat_map(|l| l.walls.iter().map(|w| (w.start, w.end, w.thickness)))
        .collect();
    cx.floor()
        .walls
        .iter()
        .filter(|w| !w.flags.invisible && cx.layers().is_visible(&w.layer))
        .filter(|w| {
            refs.iter()
                .any(|r| same_wall((w.start, w.end, w.thickness), *r))
        })
        .map(|w| w.id)
        .collect()
}

/// The aligned walls' edge lines in light blue (S-194).
pub fn draw_alignment(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    for id in aligned_wall_ids(cx) {
        if let Some(w) = cx.floor().wall(id) {
            super::render::draw_wall_outline(
                painter,
                cam,
                cx,
                w,
                Stroke::new(2.5_f32, ALIGNED_BLUE),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// XOR drawing
// ---------------------------------------------------------------------------

/// What XOR drawing does with one reference edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeState {
    /// Nothing under it: drawn in the row's color.
    Plain,
    /// Identical to an edge of the current floor: not drawn.
    Identical,
    /// Laid over the current floor's walls: drawn in the changed color.
    Changed,
}

/// The XOR state of each edge of `edges` against the footprints of the
/// current floor's walls.
pub fn xor_edges(edges: &[(Point, Point)], current: &[Vec<Point>]) -> Vec<EdgeState> {
    let cur_edges: Vec<(Point, Point)> = current
        .iter()
        .flat_map(|poly| {
            (0..poly.len())
                .map(|i| (poly[i], poly[(i + 1) % poly.len()]))
                .collect::<Vec<_>>()
        })
        .collect();
    edges
        .iter()
        .map(|&(a, b)| {
            let identical = cur_edges.iter().any(|&(c, d)| {
                (a.dist(c) <= XOR_TOLERANCE && b.dist(d) <= XOR_TOLERANCE)
                    || (a.dist(d) <= XOR_TOLERANCE && b.dist(c) <= XOR_TOLERANCE)
            });
            if identical {
                return EdgeState::Identical;
            }
            let mid = Point::lerp(a, b, 0.5);
            let over = current.iter().any(|poly| point_in_polygon(mid, poly))
                || cur_edges
                    .iter()
                    .any(|&(c, d)| segment_intersection(a, b, c, d).is_some());
            if over {
                EdgeState::Changed
            } else {
                EdgeState::Plain
            }
        })
        .collect()
}

/// The color XOR drawing gives the lines laid over the current floor.
pub fn xor_color(c: [u8; 3]) -> [u8; 3] {
    [255 - c[0], 255 - c[1], 255 - c[2]]
}

// ---------------------------------------------------------------------------
// Drawing: reference rows
// ---------------------------------------------------------------------------

fn quad(cam: &Camera, pts: &[Point]) -> Vec<Pos2> {
    pts.iter().map(|p| cam.world_to_screen(*p)).collect()
}

/// Draws the reference layers dimmed. `front` picks the rows drawn in front
/// of the plan (above the Current line); the rest are drawn first, behind.
/// Lower rows of the table are drawn before the ones above them.
pub fn draw_reference(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, front: bool) {
    let layers = reference_layers(cx);
    if layers.is_empty() {
        return;
    }
    let xor = cx.project.reference_table.xor;
    let current: Vec<Vec<Point>> = if xor {
        cx.floor()
            .walls
            .iter()
            .filter(|w| !w.flags.invisible && cx.layers().is_visible(&w.layer))
            .map(|w| w.footprint().to_vec())
            .collect()
    } else {
        Vec::new()
    };
    for layer in layers.iter().rev().filter(|l| l.front == front) {
        let [r, g, b] = layer.row.color;
        let fill = Color32::from_rgba_unmultiplied(r, g, b, 70);
        let edge = Stroke::new(0.75_f32, Color32::from_rgba_unmultiplied(r, g, b, 170));
        for w in &layer.walls {
            if !xor {
                let pts = quad(cam, &w.footprint);
                if layer.row.details {
                    painter.add(Shape::convex_polygon(pts, fill, edge));
                } else {
                    painter.add(Shape::closed_line(pts, edge));
                }
                continue;
            }
            if layer.row.details {
                painter.add(Shape::convex_polygon(
                    quad(cam, &w.footprint),
                    fill,
                    Stroke::NONE,
                ));
            }
            let n = w.footprint.len();
            let edges: Vec<(Point, Point)> = (0..n)
                .map(|i| (w.footprint[i], w.footprint[(i + 1) % n]))
                .collect();
            let [xr, xg, xb] = xor_color(layer.row.color);
            let changed = Stroke::new(0.9_f32, Color32::from_rgba_unmultiplied(xr, xg, xb, 200));
            for ((a, b), state) in edges.iter().zip(xor_edges(&edges, &current)) {
                let seg = [cam.world_to_screen(*a), cam.world_to_screen(*b)];
                match state {
                    EdgeState::Identical => {}
                    EdgeState::Plain => {
                        painter.line_segment(seg, edge);
                    }
                    EdgeState::Changed => {
                        painter.line_segment(seg, changed);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing: construction lines
// ---------------------------------------------------------------------------

/// The outline of a callout of `shape` as offsets from its center, for a box
/// `w` by `h` (pixels) turned by `angle` radians.
pub fn callout_outline(shape: CalloutShape, w: f32, h: f32, angle: f32) -> Vec<Pos2> {
    use std::f32::consts::TAU;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let ngon = |n: usize, start: f32| -> Vec<Pos2> {
        (0..n)
            .map(|i| {
                let t = start + TAU * i as f32 / n as f32;
                Pos2::new(hw * t.cos(), hh * t.sin())
            })
            .collect()
    };
    let pts: Vec<Pos2> = match shape {
        CalloutShape::Circle | CalloutShape::Ellipse => ngon(40, 0.0),
        CalloutShape::Square | CalloutShape::Rectangle => vec![
            Pos2::new(-hw, -hh),
            Pos2::new(hw, -hh),
            Pos2::new(hw, hh),
            Pos2::new(-hw, hh),
        ],
        CalloutShape::Diamond => vec![
            Pos2::new(0.0, -hh),
            Pos2::new(hw, 0.0),
            Pos2::new(0.0, hh),
            Pos2::new(-hw, 0.0),
        ],
        CalloutShape::Hexagon => ngon(6, 0.0),
        CalloutShape::Octagon => ngon(8, TAU / 16.0),
        CalloutShape::Capsule => {
            // Two half circles joined by straight sides.
            let r = hh;
            let cx = (hw - r).max(0.0);
            let mut v = Vec::new();
            for i in 0..=12 {
                let t = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                v.push(Pos2::new(cx + r * t.cos(), r * t.sin()));
            }
            for i in 0..=12 {
                let t = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 12.0;
                v.push(Pos2::new(-cx + r * t.cos(), r * t.sin()));
            }
            v
        }
    };
    if !shape.has_angle() || angle == 0.0 {
        return pts;
    }
    let (s, c) = angle.sin_cos();
    pts.into_iter()
        .map(|p| Pos2::new(p.x * c - p.y * s, p.x * s + p.y * c))
        .collect()
}

fn color_of(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The stroke style of a line: dash and gap lengths in pixels, `None` solid.
fn dashes(style: LineStyle) -> Option<(f32, f32)> {
    match style {
        LineStyle::Solid => None,
        LineStyle::Dashed => Some((9.0, 5.0)),
        LineStyle::Dotted => Some((2.0, 4.0)),
        LineStyle::DashDot => Some((10.0, 6.0)),
    }
}

fn dashed_or_solid(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke, style: LineStyle) {
    match dashes(style) {
        None => {
            painter.line_segment([a, b], stroke);
        }
        Some((dash, gap)) => {
            painter.extend(Shape::dashed_line(&[a, b], stroke, dash, gap));
        }
    }
}

/// Draws one callout centered at `at`.
#[allow(clippy::too_many_arguments)]
fn draw_callout(
    painter: &egui::Painter,
    at: Pos2,
    spec: &CalloutSpec,
    texts: (&str, &str),
    layer_color: Color32,
    line_stroke: Stroke,
    text_color: Color32,
    px_per_in: f64,
) {
    let font = FontId::proportional(13.0);
    let top = painter.layout_no_wrap(texts.0.to_string(), font.clone(), text_color);
    let below = (!texts.1.is_empty())
        .then(|| painter.layout_no_wrap(texts.1.to_string(), font.clone(), text_color));
    let text_w = top.size().x.max(below.as_ref().map_or(0.0, |g| g.size().x));
    let text_h = top.size().y + below.as_ref().map_or(0.0, |g| g.size().y);
    let side = if spec.auto_size {
        (text_w.max(text_h) + 10.0).max(20.0)
    } else {
        ((spec.size * px_per_in) as f32).max(14.0)
    };
    let (w, h) = match spec.shape {
        CalloutShape::Ellipse | CalloutShape::Capsule | CalloutShape::Rectangle => {
            (side * 1.5, side * 0.8)
        }
        _ => (side, side),
    };
    let (w, h) = if spec.auto_size {
        (w.max(text_w + 10.0), h.max(text_h + 4.0))
    } else {
        (w, h)
    };
    let pts: Vec<Pos2> = callout_outline(spec.shape, w, h, spec.angle_deg.to_radians() as f32)
        .into_iter()
        .map(|p| at + p.to_vec2())
        .collect();
    let fill = if spec.filled {
        let base = if spec.fill_by_layer {
            layer_color
        } else {
            color_of(spec.fill_color)
        };
        let alpha = (255.0 * (1.0 - f32::from(spec.transparency.min(100)) / 100.0)) as u8;
        Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
    } else {
        Color32::TRANSPARENT
    };
    let outline = if spec.custom_outline {
        Stroke::new(
            if spec.outline_weight_by_layer {
                line_stroke.width
            } else {
                (spec.outline_weight as f32 / 25.0).clamp(0.5, 4.0)
            },
            if spec.outline_color_by_layer {
                line_stroke.color
            } else {
                color_of(spec.outline_color)
            },
        )
    } else {
        line_stroke
    };
    painter.add(Shape::convex_polygon(pts, fill, outline));
    match below {
        None => {
            painter.galley(at - top.size() / 2.0, top, text_color);
        }
        Some(b) => {
            let split = at.y;
            painter.line_segment(
                [
                    Pos2::new(at.x - w / 2.0, split),
                    Pos2::new(at.x + w / 2.0, split),
                ],
                outline,
            );
            painter.galley(
                Pos2::new(at.x - top.size().x / 2.0, split - top.size().y - 1.0),
                top,
                text_color,
            );
            painter.galley(
                Pos2::new(at.x - b.size().x / 2.0, split + 1.0),
                b,
                text_color,
            );
        }
    }
}

/// The screen points (start, end) a line is drawn between: the view edges
/// for an infinite line, its own ends otherwise; `None` off screen.
fn screen_extent(cam: &Camera, l: &ResolvedLine, infinite: bool) -> Option<(Pos2, Pos2)> {
    let a = cam.world_to_screen(l.a);
    let b = cam.world_to_screen(l.b);
    if !infinite {
        return Some((a, b));
    }
    let (s, e) = construction::clip_line_to_rect(
        Point::new(f64::from(a.x), f64::from(a.y)),
        Point::new(f64::from(b.x), f64::from(b.y)),
        Point::new(f64::from(cam.rect.min.x), f64::from(cam.rect.min.y)),
        Point::new(f64::from(cam.rect.max.x), f64::from(cam.rect.max.y)),
    )?;
    Some((
        Pos2::new(s.x as f32, s.y as f32),
        Pos2::new(e.x as f32, e.y as f32),
    ))
}

/// Draws the construction lines in the drawing groups behind the walls
/// (`behind`) or over them.
pub fn draw_construction(cx: &EditorContext, painter: &egui::Painter, cam: &Camera, behind: bool) {
    let lines = visible_construction(cx);
    if lines.is_empty() {
        return;
    }
    let table = &cx.project.drawing_group_defaults;
    let walls_group = table.group_of(plan_core::drawing_group::WALLS);
    let labels = cx.project.construction_order(cx.floor, ViewType::Plan);
    let weights = cx
        .view_flags
        .contains(&crate::toolbar::ViewFlag::LineWeights);
    let mut ordered: Vec<(i32, &ResolvedLine)> = lines
        .iter()
        .map(|l| {
            let g = if l.floor == cx.floor {
                cx.project.floors[l.floor].drawing_group(table, plan_core::ObjectRef::Cad(l.id))
            } else {
                construction::DEFAULT_GROUP
            };
            (g, l)
        })
        .filter(|(g, _)| (*g < walls_group) == behind)
        .collect();
    ordered.sort_by_key(|(g, _)| *g);
    let text_color = cx.palette.text;
    for (_, l) in ordered {
        let layer = cx.layers().get(&l.layer);
        let layer_color = layer.map_or(Color32::BLACK, |x| color_of(x.color));
        let color = l.spec.color.map_or(layer_color, color_of);
        let style = l
            .spec
            .line_style
            .or(layer.map(|x| x.line_style))
            .unwrap_or_default();
        let weight = l
            .spec
            .line_weight
            .or(layer.map(|x| x.line_weight))
            .unwrap_or(25);
        let width = if weights {
            crate::editor::restyle::weight_factor(weight)
        } else {
            1.0
        };
        let stroke = Stroke::new(width, color);
        let Some((s, e)) = screen_extent(cam, l, l.spec.infinite_plan) else {
            continue;
        };
        dashed_or_solid(painter, s, e, stroke, style);
        let ends = l.spec.callouts.plan;
        if ends == CalloutEnds::None {
            continue;
        }
        let auto = labels.get(&l.id).map(String::as_str);
        let (top, below) = l.spec.callouts.texts(auto);
        let dir = (e - s).normalized();
        // Infinite lines keep their callouts inside the view edge; finite
        // ones put them past the ends.
        let inward = if l.spec.infinite_plan { 1.0 } else { -1.0 };
        let margin = 16.0;
        let at_start = s + dir * (margin * inward);
        let at_end = e - dir * (margin * inward);
        let draw_at = |at: Pos2| {
            draw_callout(
                painter,
                at,
                &l.spec.callouts,
                (&top, &below),
                layer_color,
                stroke,
                text_color,
                cam.px_per_in,
            )
        };
        if ends.at_start() {
            draw_at(at_start);
        }
        if ends.at_end() {
            draw_at(at_end);
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
/// The ordering label shown for line `id` on the active floor (None when it
/// takes no part in automatic ordering or no rule set takes it).
pub fn order_label(cx: &EditorContext, id: Id) -> Option<String> {
    cx.project
        .construction_order(cx.floor, ViewType::Plan)
        .remove(&id)
}

/// The reference table with its default row filled in (what the Change
/// Floor/Reference dialog lists).
pub fn table_for_dialog(project: &Project) -> ReferenceTable {
    let mut t = project.reference_table.clone();
    if t.rows.is_empty() {
        t.rows = effective_rows(project);
        t.current_at = 0;
    }
    t
}

/// Does `row` refer to a floor of its plan at all right now?
#[cfg_attr(not(test), allow(dead_code))]
pub fn row_floor_exists(cx: &EditorContext, row: &ReferenceRow) -> bool {
    match &row.source {
        ReferenceSource::ThisPlan => row
            .floor
            .resolve(cx.floor, cx.project.floors.len(), false)
            .is_some(),
        ReferenceSource::File(p) => other_plan(p).is_some_and(|plan| {
            row.floor
                .resolve(cx.floor, plan.floors.len(), true)
                .is_some()
        }),
    }
}

/// The floor a fixed-floor row of this plan names, for the dialog's label.
#[cfg_attr(not(test), allow(dead_code))]
pub fn floor_choices(count: usize) -> Vec<RowFloor> {
    let mut v = vec![
        RowFloor::Automatic,
        RowFloor::Below,
        RowFloor::Above,
        RowFloor::MatchCurrent,
    ];
    v.extend((0..count).map(RowFloor::Fixed));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::WallKind;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn cx_with_two_floors() -> EditorContext {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        cx.project
            .add_wall(0, p(0.0, 0.0), p(120.0, 0.0), 6.0, 96.0, WallKind::Exterior);
        cx.project
            .floors
            .push(plan_core::Floor::new("2nd Floor", 108.0));
        cx.floor = 1;
        cx.view_flags
            .insert(crate::toolbar::ViewFlag::ReferenceDisplay);
        crate::dialogs::reference_display::reset_settings();
        cx
    }

    #[test]
    fn the_default_row_is_the_floor_below() {
        let cx = cx_with_two_floors();
        let layers = reference_layers(&cx);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].floor, 0);
        assert_eq!(layers[0].walls.len(), 1);
        assert!(!layers[0].front);
        let (lo, hi) = layers[0].bounds().unwrap();
        assert!(lo.x <= 0.0 && hi.x >= 120.0);
    }

    #[test]
    fn another_plan_file_is_shown_through_its_offset_and_angle() {
        let mut other = Project::new("existing");
        other.add_wall(0, p(0.0, 0.0), p(100.0, 0.0), 6.0, 96.0, WallKind::Exterior);
        let path = "/nonexistent/existing-house.psplan";
        preload_other_plan(path, other);
        let mut cx = cx_with_two_floors();
        cx.floor = 0;
        let mut row = ReferenceRow::for_file(path);
        row.offset = [50.0, 20.0, 0.0];
        row.angle_deg = 90.0;
        cx.project.reference_table.rows = vec![row];
        let layers = reference_layers(&cx);
        assert_eq!(layers.len(), 1, "match current: the same level");
        let w = &layers[0].walls[0];
        // (100, 0) turned 90 degrees is (0, 100), then moved by (50, 20).
        assert!(w.end.dist(p(50.0, 120.0)) < 1e-6, "{:?}", w.end);
        assert!(w.start.dist(p(50.0, 20.0)) < 1e-6);
        // The centerlines feed the snaps.
        let segs = snap_segments(&cx);
        assert!(segs
            .iter()
            .any(|(a, b)| a.dist(w.start) < 1e-6 && b.dist(w.end) < 1e-6));
        // A file that cannot be read shows nothing.
        cx.project.reference_table.rows = vec![ReferenceRow::for_file("/nonexistent/none.psplan")];
        assert!(reference_layers(&cx).is_empty());
    }

    #[test]
    fn rows_above_the_current_line_draw_in_front() {
        let mut cx = cx_with_two_floors();
        cx.project.reference_table.rows = vec![ReferenceRow::default(), ReferenceRow::default()];
        cx.project.reference_table.current_at = 1;
        let layers = reference_layers(&cx);
        assert_eq!(layers.len(), 2);
        assert!(layers[0].front && !layers[1].front);
    }

    #[test]
    fn reference_display_off_shows_no_rows() {
        let mut cx = cx_with_two_floors();
        cx.view_flags
            .remove(&crate::toolbar::ViewFlag::ReferenceDisplay);
        assert!(reference_layers(&cx).is_empty());
        assert!(snap_segments(&cx).is_empty());
    }

    #[test]
    fn a_wall_drawn_exactly_over_its_reference_counterpart_is_aligned() {
        let mut cx = cx_with_two_floors();
        let id = cx
            .project
            .add_wall(1, p(120.0, 0.0), p(0.0, 0.0), 6.0, 96.0, WallKind::Exterior);
        let off = cx.project.add_wall(
            1,
            p(0.0, 50.0),
            p(120.0, 51.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let aligned = aligned_wall_ids(&cx);
        assert!(aligned.contains(&id), "same ends, either way round");
        assert!(!aligned.contains(&off));
        // Moved a half inch off: no longer exactly aligned.
        cx.project.floors[1].wall_mut(id).unwrap().start = p(120.0, 0.5);
        assert!(!aligned_wall_ids(&cx).contains(&id));
        // With the display off nothing is aligned.
        cx.view_flags
            .remove(&crate::toolbar::ViewFlag::ReferenceDisplay);
        assert!(aligned_wall_ids(&cx).is_empty());
    }

    #[test]
    fn xor_drops_identical_edges_and_marks_the_ones_laid_over_walls() {
        let current = vec![vec![
            p(0.0, -3.0),
            p(120.0, -3.0),
            p(120.0, 3.0),
            p(0.0, 3.0),
        ]];
        let edges = [
            (p(0.0, -3.0), p(120.0, -3.0)),
            (p(60.0, -20.0), p(60.0, 20.0)),
            (p(0.0, 40.0), p(120.0, 40.0)),
        ];
        let st = xor_edges(&edges, &current);
        assert_eq!(st[0], EdgeState::Identical);
        assert_eq!(st[1], EdgeState::Changed);
        assert_eq!(st[2], EdgeState::Plain);
        assert_eq!(xor_color([128, 128, 128]), [127, 127, 127]);
    }

    #[test]
    fn construction_lines_are_snap_segments_when_infinite_or_on_other_floors() {
        let mut cx = cx_with_two_floors();
        let inf = cx
            .project
            .add_construction_line(1, p(0.0, 10.0), p(5.0, 10.0), None)
            .unwrap();
        let fin = cx
            .project
            .add_construction_line(1, p(0.0, 30.0), p(5.0, 30.0), None)
            .unwrap();
        cx.project.floors[1]
            .construction
            .get_mut(fin)
            .unwrap()
            .infinite_plan = false;
        let segs = construction_snap_segments(&cx);
        assert_eq!(segs.len(), 1, "the finite line is a CAD line already");
        assert!(construction::is_infinite_segment(segs[0].0, segs[0].1));
        // On the floor below, a line set to all floors comes as it is drawn.
        cx.project.floors[1]
            .construction
            .get_mut(inf)
            .unwrap()
            .all_floors = true;
        cx.project.floors[1]
            .construction
            .get_mut(fin)
            .unwrap()
            .all_floors = true;
        cx.floor = 0;
        let segs = construction_snap_segments(&cx);
        assert_eq!(segs.len(), 2);
        assert!(segs
            .iter()
            .any(|(a, b)| !construction::is_infinite_segment(*a, *b)));
        // A hidden layer takes its lines away.
        cx.project.layers.set_display(construction::LAYER, false);
        assert!(construction_snap_segments(&cx).is_empty());
    }

    #[test]
    fn callout_shapes_fit_their_box_and_turn() {
        for s in CalloutShape::ALL {
            let pts = callout_outline(s, 40.0, 40.0, 0.0);
            assert!(pts.len() >= 4, "{s:?}");
            assert!(
                pts.iter().all(|p| p.x.abs() <= 20.01 && p.y.abs() <= 20.01),
                "{s:?}"
            );
        }
        let sq = callout_outline(CalloutShape::Square, 20.0, 20.0, 0.0);
        let turned = callout_outline(
            CalloutShape::Square,
            20.0,
            20.0,
            std::f32::consts::FRAC_PI_4,
        );
        assert!((turned[0].x - sq[0].x).abs() > 1.0 || (turned[0].y - sq[0].y).abs() > 1.0);
        // The angle does not apply to a rectangle.
        let r0 = callout_outline(CalloutShape::Rectangle, 30.0, 16.0, 0.0);
        let r1 = callout_outline(CalloutShape::Rectangle, 30.0, 16.0, 1.0);
        assert_eq!(r0, r1);
    }

    #[test]
    fn floor_choices_list_the_automatic_ones_then_each_floor() {
        let c = floor_choices(3);
        assert_eq!(c[0], RowFloor::Automatic);
        assert_eq!(c.len(), 7);
        let cx = cx_with_two_floors();
        assert!(row_floor_exists(&cx, &ReferenceRow::default()));
    }
}
