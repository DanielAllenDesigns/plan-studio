//! Annotation tools in a section or elevation view (R17-01, manual pp. 1165
//! to 1200). While a view's drawing is the active view, the Text, Rich Text,
//! Note, Leader Line, Dimension (manual, interior, point to point, auto
//! elevation) and CAD (line, polyline, box) tools write [`ViewAnnotation`]s
//! into `CameraView::annotations`, one undo step each, and Select Objects
//! moves, edits and deletes them through `AnnotKind::handles/set_handle`.
//!
//! The host stands in for the active tool (see `ToolSet::active_mut`) and
//! reads pointer positions as drawing coordinates: inches, X along the view,
//! Y up (the view panel maps the screen to the drawing before it calls).

use plan_core::camera::CameraObject;
use plan_core::camera_view::{AnnotKind, DrawSurface, ViewAnnotation};
use plan_core::{Id, Point, Project};

use super::cad::CadMode;
use super::dimension::DimMode;
use super::text::TextMode;
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::EditorContext;

/// Pick distance in drawing inches.
pub const HIT_TOL: f64 = 4.0;
/// A drawing vertex closer than this catches an Interior / Point to Point click.
const SNAP_TOL: f64 = 6.0;
/// Distance of the first auto string from the drawing, inches.
const FIRST_STRING: f64 = 14.0;
/// Space between auto strings, inches.
const STRING_GAP: f64 = 14.0;
/// Levels closer than this are one level, inches.
const LEVEL_TOL: f64 = 0.5;

/// What the host is in the middle of.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    /// Text, Note or Leader waiting for its words (Enter ends).
    Words {
        kind: AnnotKind,
        label: &'static str,
    },
    /// Leader line: the arrow tip is placed, the text position is next.
    Tip([f64; 2]),
    /// A dimension with `pts.len()` points placed (offset is the third click).
    Dim(Vec<[f64; 2]>),
    Line([f64; 2]),
    Poly(Vec<[f64; 2]>),
    Box([f64; 2]),
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    id: Id,
    handle: Option<usize>,
    last: [f64; 2],
    started: bool,
}

/// The annotation host: a [`Tool`] for the drawing view.
pub struct ViewAnnotHost {
    /// The camera whose drawing is the active view.
    pub view: Option<Id>,
    /// The tool the toolbar has picked (with its variant).
    pub picked: ToolId,
    /// The annotation Select Objects last picked.
    pub selected: Option<Id>,
    pub tol: f64,
    pending: Option<Pending>,
    drag: Option<Drag>,
    editing: Option<(Id, bool)>,
}

impl Default for ViewAnnotHost {
    fn default() -> Self {
        Self {
            view: None,
            picked: ToolId::Select,
            selected: None,
            tol: HIT_TOL,
            pending: None,
            drag: None,
            editing: None,
        }
    }
}

fn p2(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

fn result(label: Option<&str>) -> ToolResult {
    ToolResult {
        consumed: true,
        repaint: true,
        commit: label.map(str::to_string),
        ..ToolResult::default()
    }
}

/// Snap target: the nearest end point of the drawing's lines (not annotations).
fn snap_to_drawing(drawing: &plan_elevation::Drawing, at: [f64; 2]) -> [f64; 2] {
    use plan_elevation::EdgeKind;
    let me = Point::new(at[0], at[1]);
    drawing
        .lines
        .iter()
        .filter(|l| {
            !matches!(
                l.kind,
                EdgeKind::Annotation | EdgeKind::Hatch | EdgeKind::Hidden
            )
        })
        .flat_map(|l| [l.a, l.b])
        .map(|p| (p.dist(me), p))
        .filter(|(d, _)| *d <= SNAP_TOL)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(at, |(_, p)| p2(p))
}

fn drawing_of(project: &Project, c: &CameraObject) -> plan_elevation::Drawing {
    use crate::dialogs::camera::{elevation_options, render_elevation_with};
    let o = plan_elevation::Options {
        raster_px: 256,
        ..elevation_options(c)
    };
    render_elevation_with(project, c, &o)
}

impl ViewAnnotHost {
    /// Ends whatever is pending: words typed so far are committed.
    pub fn finish(&mut self, cx: &mut EditorContext) {
        if let Some(Pending::Words { kind, label }) = self.pending.take() {
            self.add(cx, kind, label);
        }
        self.pending = None;
        self.drag = None;
        self.editing = None;
    }

    fn camera<'a>(&self, cx: &'a EditorContext) -> Option<&'a CameraObject> {
        cx.project.camera(self.view?)
    }

    fn fallback_layer(kind: &AnnotKind) -> &'static str {
        match kind {
            AnnotKind::Dimension { .. } | AnnotKind::PointMarker { .. } => "Dimensions",
            AnnotKind::Note { .. } => "Notes",
            AnnotKind::Text { .. } | AnnotKind::Leader { .. } => "Text",
            _ => "CAD",
        }
    }

    /// Adds one annotation as one undo step; returns its id.
    fn add(&mut self, cx: &mut EditorContext, kind: AnnotKind, label: &str) -> Option<Id> {
        let cam = self.view?;
        cx.project.camera(cam)?;
        cx.begin_change(label);
        let is_dim = matches!(kind, AnnotKind::Dimension { .. });
        let mut new_id = 0;
        let layer_kind = Self::fallback_layer(&kind);
        cx.project.update_camera(cam, |c| {
            new_id = c.view.annotations.iter().map(|a| a.id).max().unwrap_or(0) + 1;
            let layer = c.view.selected.annotation_layer(layer_kind).to_string();
            c.view.annotations.push(ViewAnnotation {
                id: new_id,
                kind,
                surface: DrawSurface::drawing(),
                layer,
                weight: None,
            });
        });
        if is_dim {
            let lines = cx
                .project
                .camera(cam)
                .map(|c| crate::dialogs::camera::cross_section_lines_of(&cx.project, c))
                .unwrap_or_default();
            cx.project.update_camera(cam, |c| {
                crate::dialogs::camera::attach_cut_markers(c, &lines);
            });
        }
        self.selected = Some(new_id);
        Some(new_id)
    }

    fn hit(&self, cx: &EditorContext, p: [f64; 2]) -> Option<(Id, Option<usize>)> {
        let c = self.camera(cx)?;
        for a in c.view.annotations.iter().rev() {
            for (i, h) in a.kind.handles().iter().enumerate() {
                if (h[0] - p[0]).hypot(h[1] - p[1]) <= self.tol {
                    return Some((a.id, Some(i)));
                }
            }
        }
        c.view
            .annotations
            .iter()
            .rev()
            .find(|a| a.kind.distance(p) <= self.tol)
            .map(|a| (a.id, None))
    }

    fn snapped(&self, cx: &EditorContext, p: [f64; 2], snap: bool) -> [f64; 2] {
        if !snap {
            return p;
        }
        self.camera(cx)
            .map_or(p, |c| snap_to_drawing(&drawing_of(&cx.project, c), p))
    }

    fn press_tool(&mut self, cx: &mut EditorContext, p: [f64; 2]) -> ToolResult {
        // A click anywhere ends the words being typed.
        if matches!(self.pending, Some(Pending::Words { .. })) {
            self.finish(cx);
            return result(Some("Add Text"));
        }
        let size = plan_core::camera_view::annot::DIM_TEXT;
        match self.picked {
            ToolId::TextVariant(m) => match m {
                TextMode::Text | TextMode::RichText | TextMode::Note => {
                    let rich = m == TextMode::RichText;
                    let (kind, label) = if m == TextMode::Note {
                        (
                            AnnotKind::Note {
                                at: p,
                                text: String::new(),
                                note_type: "General".into(),
                                size,
                            },
                            "Add Note",
                        )
                    } else {
                        (
                            AnnotKind::Text {
                                at: p,
                                text: String::new(),
                                size,
                                rich,
                                faces_camera: false,
                            },
                            if rich { "Add Rich Text" } else { "Add Text" },
                        )
                    };
                    self.pending = Some(Pending::Words { kind, label });
                    result(None)
                }
                TextMode::LeaderLine | TextMode::ArrowLine | TextMode::Callout => {
                    match self.pending.take() {
                        Some(Pending::Tip(tip)) => {
                            let kind = AnnotKind::Leader {
                                tip,
                                at: p,
                                text: String::new(),
                                size,
                            };
                            self.pending = Some(Pending::Words {
                                kind,
                                label: "Add Leader Line",
                            });
                        }
                        _ => self.pending = Some(Pending::Tip(p)),
                    }
                    result(None)
                }
                _ => {
                    cx.status = "This text tool works in the plan only".into();
                    result(None)
                }
            },
            ToolId::DimensionVariant(m) => self.press_dimension(cx, m, p),
            ToolId::CadVariant(m) => self.press_cad(cx, m, p),
            _ => {
                cx.status = "This tool works in the plan only".into();
                result(None)
            }
        }
    }

    fn press_dimension(&mut self, cx: &mut EditorContext, m: DimMode, p: [f64; 2]) -> ToolResult {
        match m {
            DimMode::AutoElevation => {
                let n = self.view.map_or(0, |v| auto_elevation(cx, v));
                cx.status = format!("{n} elevation dimensions placed");
                result((n > 0).then_some("Auto Elevation Dimensions"))
            }
            DimMode::Manual | DimMode::Interior | DimMode::PointToPoint | DimMode::EndToEnd => {
                let p = self.snapped(cx, p, m != DimMode::Manual);
                let mut pts = match self.pending.take() {
                    Some(Pending::Dim(v)) => v,
                    _ => Vec::new(),
                };
                pts.push(p);
                if pts.len() < 3 {
                    self.pending = Some(Pending::Dim(pts));
                    return result(None);
                }
                // The third click places the dimension line and the text.
                let (a, b) = (pts[0], pts[1]);
                let mut kind = AnnotKind::Dimension {
                    a,
                    b,
                    offset: [0.0, 0.0],
                    a_cut: None,
                    b_cut: None,
                    text: None,
                };
                kind.set_handle(2, pts[2]);
                self.add(cx, kind, "Add Dimension");
                result(Some("Add Dimension"))
            }
            _ => {
                cx.status = "This dimension works in the plan only".into();
                result(None)
            }
        }
    }

    fn press_cad(&mut self, cx: &mut EditorContext, m: CadMode, p: [f64; 2]) -> ToolResult {
        match m {
            CadMode::Line | CadMode::InputLine | CadMode::LineArrow => match self.pending.take() {
                Some(Pending::Line(a)) => {
                    self.add(cx, AnnotKind::Line { a, b: p }, "Draw Line");
                    result(Some("Draw Line"))
                }
                _ => {
                    self.pending = Some(Pending::Line(p));
                    result(None)
                }
            },
            CadMode::Polyline | CadMode::Polygon => {
                let mut pts = match self.pending.take() {
                    Some(Pending::Poly(v)) => v,
                    _ => Vec::new(),
                };
                pts.push(p);
                self.pending = Some(Pending::Poly(pts));
                result(None)
            }
            CadMode::RectPolyline | CadMode::Box | CadMode::CrossBox => match self.pending.take() {
                Some(Pending::Box(a)) => {
                    self.add(cx, AnnotKind::Rect { a, b: p }, "Draw Box");
                    result(Some("Draw Box"))
                }
                _ => {
                    self.pending = Some(Pending::Box(p));
                    result(None)
                }
            },
            _ => {
                cx.status = "This CAD tool works in the plan only".into();
                result(None)
            }
        }
    }

    fn end_polyline(&mut self, cx: &mut EditorContext) -> ToolResult {
        if let Some(Pending::Poly(mut pts)) = self.pending.take() {
            pts.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
            if pts.len() >= 2 {
                let closed = matches!(self.picked, ToolId::CadVariant(CadMode::Polygon));
                self.add(cx, AnnotKind::Polyline { pts, closed }, "Draw Polyline");
                return result(Some("Draw Polyline"));
            }
        }
        result(None)
    }

    fn edit_annotation(&mut self, cx: &mut EditorContext, id: Id, f: impl FnOnce(&mut AnnotKind)) {
        let Some(cam) = self.view else { return };
        cx.project.update_camera(cam, |c| {
            if let Some(a) = c.view.annotations.iter_mut().find(|a| a.id == id) {
                f(&mut a.kind);
            }
        });
    }
}

impl Tool for ViewAnnotHost {
    fn id(&self) -> ToolId {
        ToolId::Select
    }

    fn name(&self) -> &'static str {
        "View Annotation"
    }

    fn hint(&self) -> String {
        "Annotate the section or elevation view".into()
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, ev: PointerEvent) -> ToolResult {
        if self.view.is_none() {
            return ToolResult::ignored();
        }
        let p = p2(ev.world);
        if self.picked.base() != ToolId::Select {
            return self.press_tool(cx, p);
        }
        self.editing = None;
        match self.hit(cx, p) {
            Some((id, handle)) => {
                self.selected = Some(id);
                self.drag = Some(Drag {
                    id,
                    handle,
                    last: p,
                    started: false,
                });
            }
            None => self.selected = None,
        }
        result(None)
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, ev: PointerEvent) -> ToolResult {
        let Some(mut d) = self.drag.filter(|_| ev.down) else {
            return ToolResult::ignored();
        };
        let p = p2(ev.world);
        if p == d.last {
            return result(None);
        }
        if !d.started {
            cx.begin_change(if d.handle.is_some() {
                "Move Handle"
            } else {
                "Move Annotation"
            });
            d.started = true;
        }
        let delta = [p[0] - d.last[0], p[1] - d.last[1]];
        match d.handle {
            Some(i) => self.edit_annotation(cx, d.id, |k| k.set_handle(i, p)),
            None => self.edit_annotation(cx, d.id, |k| k.translate(delta)),
        }
        d.last = p;
        self.drag = Some(d);
        result(None)
    }

    fn pointer_up(&mut self, _cx: &mut EditorContext, _ev: PointerEvent) -> ToolResult {
        match self.drag.take() {
            Some(d) if d.started => result(Some("Move Annotation")),
            _ => ToolResult::ignored(),
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, ev: PointerEvent) -> ToolResult {
        if self.view.is_none() {
            return ToolResult::ignored();
        }
        if self.picked.base() != ToolId::Select {
            return self.end_polyline(cx);
        }
        if let Some((id, _)) = self.hit(cx, p2(ev.world)) {
            self.selected = Some(id);
            self.editing = Some((id, false));
            self.drag = None;
            return result(None);
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        use eframe::egui::Key;
        if self.view.is_none() {
            return ToolResult::ignored();
        }
        // Words being placed.
        if let Some(Pending::Words { kind, label }) = self.pending.as_mut() {
            if k.is(Key::Escape) {
                self.pending = None;
                return result(None);
            }
            if k.is(Key::Enter) {
                let (kind, label) = (kind.clone(), *label);
                self.pending = None;
                self.add(cx, kind, label);
                return result(Some(label));
            }
            if let Some(t) = kind.text_mut() {
                if k.is(Key::Backspace) {
                    t.pop();
                } else if let Some(s) = &k.text {
                    t.push_str(s);
                }
            }
            return result(None);
        }
        // Words of a picked annotation being edited.
        if let Some((id, started)) = self.editing {
            if k.is(Key::Enter) || k.is(Key::Escape) {
                self.editing = None;
                return result(None);
            }
            let text = k.text.clone();
            let back = k.is(Key::Backspace);
            if back || text.is_some() {
                if !started {
                    cx.begin_change("Edit Text");
                    self.editing = Some((id, true));
                }
                self.edit_annotation(cx, id, |kind| {
                    if let Some(t) = kind.text_mut() {
                        if back {
                            t.pop();
                        } else if let Some(s) = &text {
                            t.push_str(s);
                        }
                    }
                });
                return result(Some("Edit Text"));
            }
        }
        if k.is(Key::Enter) {
            return self.end_polyline(cx);
        }
        if k.is(Key::Escape) {
            self.pending = None;
            return result(None);
        }
        if (k.is(Key::Delete) || k.is(Key::Backspace)) && self.selected.is_some() {
            let (Some(id), Some(cam)) = (self.selected.take(), self.view) else {
                return ToolResult::ignored();
            };
            cx.begin_change("Delete Annotation");
            cx.project.update_camera(cam, |c| {
                c.view.annotations.retain(|a| a.id != id);
                c.view.auto_elevation.retain(|x| *x != id);
            });
            return result(Some("Delete Annotation"));
        }
        ToolResult::ignored()
    }
}

// ----- Auto Elevation Dimensions (DIM-61, DIM-62) -----

/// The levels an elevation string measures, taken from the model and located
/// on the horizontal edges of `drawing`: finished floors, tops of walls and
/// the sills and heads of openings. A level the drawing has no edge at (it
/// is clipped away or hidden) is left out. Sorted, inches.
pub fn elevation_levels(project: &Project, drawing: &plan_elevation::Drawing) -> Vec<f64> {
    use plan_elevation::EdgeKind;
    let mut want = Vec::new();
    for f in &project.floors {
        want.push(f.elevation);
        let top = f
            .walls
            .iter()
            .filter(|w| !w.flags.invisible && !w.flags.railing)
            .map(|w| w.height)
            .reduce(f64::max);
        if let Some(h) = top {
            want.push(f.elevation + h);
        }
        for o in &f.openings {
            want.push(f.elevation + o.sill_height);
            want.push(f.elevation + o.sill_height + o.height);
        }
    }
    let edges: Vec<f64> = drawing
        .lines
        .iter()
        .filter(|l| {
            matches!(
                l.kind,
                EdgeKind::Cut | EdgeKind::Silhouette | EdgeKind::Crease | EdgeKind::Material
            ) && (l.a.y - l.b.y).abs() < 0.1
                && (l.a.x - l.b.x).abs() >= 2.0
        })
        .map(|l| l.a.y)
        .collect();
    let mut out: Vec<f64> = Vec::new();
    for w in want {
        let found = edges
            .iter()
            .copied()
            .filter(|y| (y - w).abs() <= 0.75)
            .min_by(|a, b| (a - w).abs().total_cmp(&(b - w).abs()));
        if let Some(y) = found {
            if !out.iter().any(|o| (o - y).abs() < LEVEL_TOL) {
                out.push(y);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// The dimensions of the strings as `(from, to, string)`, string 0 being the
/// one next to the drawing.
fn string_spans(
    levels: &[f64],
    overall: bool,
    outer: bool,
    floors: &[f64],
) -> Vec<(f64, f64, usize)> {
    let mut v: Vec<(f64, f64, usize)> = levels.windows(2).map(|w| (w[0], w[1], 0)).collect();
    let mut next = 1;
    if outer {
        let mut fl: Vec<f64> = floors
            .iter()
            .copied()
            .filter(|f| levels.iter().any(|l| (l - f).abs() < LEVEL_TOL))
            .collect();
        if let Some(top) = levels.last() {
            fl.push(*top);
        }
        fl.sort_by(f64::total_cmp);
        fl.dedup_by(|a, b| (*a - *b).abs() < LEVEL_TOL);
        if fl.len() >= 2 && fl.len() < levels.len() {
            v.extend(fl.windows(2).map(|w| (w[0], w[1], next)));
            next += 1;
        }
    }
    if overall && levels.len() >= 3 {
        v.push((levels[0], levels[levels.len() - 1], next));
    }
    v
}

/// The Auto Elevation Dimensions of a view as annotations (not yet placed).
pub fn auto_elevation_kinds(
    project: &Project,
    c: &CameraObject,
    overall: bool,
    outer: bool,
) -> Vec<AnnotKind> {
    let drawing = drawing_of(project, c);
    let levels = elevation_levels(project, &drawing);
    let floors: Vec<f64> = project.floors.iter().map(|f| f.elevation).collect();
    let x = drawing.bounds.0.x;
    string_spans(&levels, overall, outer, &floors)
        .into_iter()
        .map(|(from, to, string)| {
            let (a, b) = ([x, from], [x, to]);
            let want = FIRST_STRING + STRING_GAP * string as f64;
            let mut kind = AnnotKind::Dimension {
                a,
                b,
                offset: [0.0, want],
                a_cut: None,
                b_cut: None,
                text: None,
            };
            // The dimension line goes to the left of the drawing.
            if kind.handles()[2][0] > x {
                if let AnnotKind::Dimension { offset, .. } = &mut kind {
                    offset[1] = -want;
                }
            }
            kind
        })
        .collect()
}

fn same_dims(a: &[AnnotKind], b: &[&AnnotKind]) -> bool {
    a.len() == b.len()
        && a.iter().all(|x| {
            b.iter().any(|y| match (x, *y) {
                (
                    AnnotKind::Dimension {
                        a: a1,
                        b: b1,
                        offset: o1,
                        ..
                    },
                    AnnotKind::Dimension {
                        a: a2,
                        b: b2,
                        offset: o2,
                        ..
                    },
                ) => {
                    let near = |p: &[f64; 2], q: &[f64; 2]| {
                        (p[0] - q[0]).abs() + (p[1] - q[1]).abs() < 0.01
                    };
                    near(a1, a2) && near(b1, b2) && near(o1, o2)
                }
                _ => false,
            })
        })
}

/// Replaces the automatic elevation strings of camera `cam` with the ones the
/// model gives now, as the caller's undo step. Returns how many there are.
pub fn auto_elevation(cx: &mut EditorContext, cam: Id) -> usize {
    let setup = &cx.defaults.dimensions.setup;
    let (overall, outer) = (setup.elevation_overall, setup.elevation_outer);
    let Some(c) = cx.project.camera(cam) else {
        return 0;
    };
    let kinds = auto_elevation_kinds(&cx.project, c, overall, outer);
    cx.begin_change("Auto Elevation Dimensions");
    let n = kinds.len();
    cx.project.update_camera(cam, |c| {
        let old = std::mem::take(&mut c.view.auto_elevation);
        c.view.annotations.retain(|a| !old.contains(&a.id));
        for kind in kinds {
            let id = c.view.annotations.iter().map(|a| a.id).max().unwrap_or(0) + 1;
            let layer = c.view.selected.annotation_layer("Dimensions").to_string();
            c.view.annotations.push(ViewAnnotation {
                id,
                kind,
                surface: DrawSurface::drawing(),
                layer,
                weight: None,
            });
            c.view.auto_elevation.push(id);
        }
    });
    n
}

/// Elevation Auto Refresh (DIM-62): every view that has automatic strings
/// gets them replaced when the model moves a level. A run that would come
/// out the same is left alone. Returns whether anything changed.
pub fn auto_refresh_elevations(cx: &mut EditorContext) -> bool {
    let setup = &cx.defaults.dimensions.setup;
    if !setup.elevation_auto_refresh {
        return false;
    }
    let (overall, outer) = (setup.elevation_overall, setup.elevation_outer);
    let cams: Vec<Id> = cx
        .project
        .cameras
        .iter()
        .filter(|c| !c.view.auto_elevation.is_empty())
        .map(|c| c.id)
        .collect();
    let mut changed = false;
    for id in cams {
        let Some(c) = cx.project.camera(id) else {
            continue;
        };
        let kinds = auto_elevation_kinds(&cx.project, c, overall, outer);
        let have: Vec<&AnnotKind> = c
            .view
            .annotations
            .iter()
            .filter(|a| c.view.auto_elevation.contains(&a.id))
            .map(|a| &a.kind)
            .collect();
        if same_dims(&kinds, &have) {
            continue;
        }
        cx.project.update_camera(id, |c| {
            let old = std::mem::take(&mut c.view.auto_elevation);
            c.view.annotations.retain(|a| !old.contains(&a.id));
            for kind in kinds {
                let nid = c.view.annotations.iter().map(|a| a.id).max().unwrap_or(0) + 1;
                let layer = c.view.selected.annotation_layer("Dimensions").to_string();
                c.view.annotations.push(ViewAnnotation {
                    id: nid,
                    kind,
                    surface: DrawSurface::drawing(),
                    layer,
                    weight: None,
                });
                c.view.auto_elevation.push(nid);
            }
        });
        changed = true;
    }
    changed
}
