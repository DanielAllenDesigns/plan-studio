//! Roof Baseline Specification (manual pp. 852 to 853; RF-127), the Join
//! Curved Roof Plane dialog (p. 857; RF-61) and the Curved Roof section of
//! the Roof Plane Specification (p. 847).
//!
//! The Roof Baseline Specification edits a draft of a roof baseline
//! polyline's [`RoofBaseline`] (the Baseline Height and one directive per
//! edge) and of the line and fill attributes of its CAD polyline. Its panels
//! are Roof Baseline, Polyline, Selected Line, Line Style and Fill Style.
//! Like the Tray Ceiling Specification it is hosted here: [`host_frame`] shows
//! it every frame (`ToolSet::frame` calls it), so it opens from the Edit
//! toolbar or [`open_spec`] whichever tool is active.

use super::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use super::{PV_ACCENT, PV_FAINT, PV_INK};
use crate::editor::roof_view::RoofPlaneRecord;
use crate::editor::EditorContext;
use crate::tools::roof_baseline as rb;
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::cad::FillAttr;
use plan_core::geometry::Point;
use plan_core::layers::LineStyle;
use plan_core::Id;
use plan_roof::{directive_text, BaselineEdge, BaselineOption, CurvedSpec, JoinLock, RoofBaseline};
use std::cell::RefCell;

const TABS: &[Tab] = &[
    on("Roof Baseline"),
    on("Polyline"),
    on("Selected Line"),
    on("Line Style"),
    on("Fill Style"),
];

/// Pitch limits in rise per 12 (the Build Roof dialog's).
const MIN_PITCH: f64 = 0.5;
const MAX_PITCH: f64 = 24.0;

/// A pitch box: rise per 12, or degrees (-89 to 89, here 1 to 89) with Pitch
/// in Degrees on. True when the value changed.
pub fn pitch_drag(ui: &mut Ui, pitch: &mut f64) -> bool {
    if plan_roof::pitch_display_degrees() {
        let mut d = plan_roof::pitch_to_degrees(*pitch);
        let changed = ui
            .add(
                egui::DragValue::new(&mut d)
                    .range(1.0..=89.0)
                    .speed(0.1)
                    .max_decimals(2)
                    .suffix("\u{b0}"),
            )
            .changed();
        if changed {
            *pitch = plan_roof::degrees_to_pitch(d);
        }
        changed
    } else {
        ui.add(
            egui::DragValue::new(pitch)
                .range(MIN_PITCH..=MAX_PITCH)
                .speed(0.1)
                .max_decimals(2)
                .suffix(" : 12"),
        )
        .changed()
    }
}

/// Line and fill attributes of the polyline.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Look {
    pub dash: Option<LineStyle>,
    pub color: Option<[u8; 3]>,
    pub weight: Option<u32>,
    pub fill: Option<FillAttr>,
}

/// The dialog for one roof baseline polyline.
pub struct RoofBaselineDialog {
    frame: SpecDialog,
    form: Form,
    floor: usize,
    id: Id,
}

struct Form {
    spec: RoofBaseline,
    look: Look,
    points: Vec<Point>,
    selected: usize,
    fields: Fields,
    /// The pitch the Default button loads.
    default_pitch: f64,
    default_overhang: f64,
}

#[allow(dead_code)]
impl RoofBaselineDialog {
    fn new(
        floor: usize,
        b: &rb::Baseline,
        look: Look,
        edge: usize,
        default: &BaselineEdge,
    ) -> Self {
        Self {
            frame: SpecDialog::new("Roof Baseline Specification", "roof_baseline"),
            form: Form {
                spec: b.spec.clone(),
                look,
                points: b.points.clone(),
                selected: edge.min(b.edge_count().saturating_sub(1)),
                fields: Fields::default(),
                default_pitch: default.pitch,
                default_overhang: default.overhang,
            },
            floor,
            id: b.id,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn id(&self) -> Id {
        self.id
    }

    pub fn spec(&self) -> &RoofBaseline {
        &self.form.spec
    }

    pub fn spec_mut(&mut self) -> &mut RoofBaseline {
        &mut self.form.spec
    }

    pub fn look_mut(&mut self) -> &mut Look {
        &mut self.form.look
    }

    /// The edge the Roof Baseline panel edits.
    pub fn selected_edge(&self) -> usize {
        self.form.selected
    }

    pub fn select_edge(&mut self, edge: usize) {
        self.form.selected = edge.min(self.form.spec.edges.len().saturating_sub(1));
    }
}

fn dash_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash Dot",
    }
}

impl Form {
    fn edge(&mut self) -> &mut BaselineEdge {
        let i = self.selected.min(self.spec.edges.len().saturating_sub(1));
        &mut self.spec.edges[i]
    }

    fn roof_baseline(&mut self, ui: &mut Ui) {
        section(ui, "Selected Edge");
        let n = self.spec.edges.len();
        let shown = self.selected;
        let label = |i: usize, e: &BaselineEdge| format!("Edge {} ({})", i + 1, directive_text(e));
        row(ui, "Edge", |ui| {
            egui::ComboBox::from_id_salt("rb_edge")
                .selected_text(label(shown, &self.spec.edges[shown.min(n - 1)]))
                .show_ui(ui, |ui| {
                    for i in 0..n {
                        let text = label(i, &self.spec.edges[i]);
                        ui.selectable_value(&mut self.selected, i, text);
                    }
                });
        });
        self.fields
            .length_row(ui, "Baseline Height", "rb_height", &mut self.spec.height);

        section(ui, "Roof Options");
        let default_pitch = self.default_pitch;
        let default_overhang = self.default_overhang;
        let sel = self.selected.min(n - 1);
        {
            let e = &mut self.spec.edges[sel];
            for o in BaselineOption::ALL {
                ui.radio_value(&mut e.option, o, o.label());
            }
            let mut extend = e.extend_down.is_some();
            ui.horizontal(|ui| {
                if ui.checkbox(&mut extend, "Extend Slope Downward").changed() {
                    e.extend_down = extend.then_some(12.0);
                }
            });
            ui.checkbox(&mut e.against_wall, "Against Wall")
                .on_hover_text("The roof plane rising from this baseline butts an exterior wall");
        }
        if self.spec.edges[sel].extend_down.is_some() {
            let mut d = self.spec.edges[sel].extend_down.unwrap_or(12.0);
            if self.fields.length_row(ui, "Extend by", "rb_extend", &mut d) {
                self.spec.edges[sel].extend_down = Some(d.max(0.0));
            }
        }

        section(ui, "Pitch Options");
        row(ui, "Pitch", |ui| {
            let e = &mut self.spec.edges[sel];
            pitch_drag(ui, &mut e.pitch);
            if ui
                .small_button("Default")
                .on_hover_text("Load the current default pitch")
                .clicked()
            {
                e.pitch = default_pitch;
            }
        });
        let mut over = self.spec.edges[sel].overhang;
        if self
            .fields
            .length_row(ui, "Overhang", "rb_overhang", &mut over)
        {
            self.spec.edges[sel].overhang = over.max(0.0);
        }
        if ui.small_button("Default Overhang").clicked() {
            self.spec.edges[sel].overhang = default_overhang;
        }
        let e = &mut self.spec.edges[sel];
        if e.option != BaselineOption::FullGable || e.upper_pitch.is_some() {
            let mut upper = e.upper_pitch.is_some();
            let label = if e.option == BaselineOption::FullGable {
                "Half hip pitch"
            } else {
                "Upper Pitch"
            };
            ui.horizontal(|ui| {
                if ui.checkbox(&mut upper, label).changed() {
                    e.upper_pitch = upper.then_some(e.pitch);
                }
                if let Some(p) = e.upper_pitch.as_mut() {
                    pitch_drag(ui, p);
                }
            });
        }
        ui.add_space(6.0);
        if ui.button("Copy these options to every edge").clicked() {
            let e = self.spec.edges[sel].clone();
            for x in &mut self.spec.edges {
                *x = e.clone();
            }
        }
    }

    fn polyline(&self, ui: &mut Ui) {
        let (perimeter, area) = plan_roof::perimeter_and_area(&self.points);
        section(ui, "Polyline");
        row(ui, "Perimeter", |ui| ui.label(super::fmt_short(perimeter)));
        row(ui, "Area", |ui| {
            ui.label(format!("{:.1} sq ft", area / 144.0))
        });
        row(ui, "Number of Lines", |ui| {
            ui.label(format!("{}", self.points.len()))
        });
        ui.weak("A roof baseline polyline always closes and has straight sides.");
    }

    fn selected_line(&self, ui: &mut Ui) {
        let n = self.points.len();
        let i = self.selected.min(n - 1);
        let (p, q) = (self.points[i], self.points[(i + 1) % n]);
        section(ui, "Selected Line");
        row(ui, "Edge", |ui| ui.label(format!("{} of {n}", i + 1)));
        row(ui, "Length", |ui| ui.label(super::fmt_short(p.dist(q))));
        row(ui, "Angle", |ui| {
            ui.label(format!("{:.2}\u{b0}", q.sub(p).angle().to_degrees()))
        });
    }

    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let mut dash = self.look.dash.unwrap_or(LineStyle::Solid);
        row(ui, "Line Style", |ui| {
            egui::ComboBox::from_id_salt("rb_dash")
                .selected_text(dash_name(dash))
                .show_ui(ui, |ui| {
                    for s in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        ui.selectable_value(&mut dash, s, dash_name(s));
                    }
                });
        });
        self.look.dash = (dash != LineStyle::Solid).then_some(dash);
        let mut mm = f64::from(self.look.weight.unwrap_or(25)) / 100.0;
        row(ui, "Line Weight", |ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut mm)
                        .range(0.05..=5.0)
                        .speed(0.01)
                        .suffix(" mm"),
                )
                .changed()
            {
                self.look.weight = Some((mm * 100.0).round() as u32);
            }
        });
        let mut c = self.look.color.unwrap_or([150, 90, 40]);
        row(ui, "Color", |ui| {
            if ui.color_edit_button_srgb(&mut c).changed() {
                self.look.color = Some(c);
            }
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        let mut on_fill = self.look.fill.is_some();
        if ui
            .checkbox(&mut on_fill, "Fill the polyline in plan view")
            .changed()
        {
            self.look.fill = on_fill.then(FillAttr::default);
        }
        if let Some(f) = self.look.fill.as_mut() {
            row(ui, "Color", |ui| {
                ui.color_edit_button_srgb(&mut f.color);
            });
            row(ui, "Pattern", |ui| {
                ui.add(egui::TextEdit::singleline(&mut f.pattern).desired_width(140.0));
            });
            let mut op = f64::from(f.opacity) / 255.0 * 100.0;
            row(ui, "Opacity", |ui| {
                if ui
                    .add(egui::DragValue::new(&mut op).range(0.0..=100.0).suffix("%"))
                    .changed()
                {
                    f.opacity = (op / 100.0 * 255.0).round() as u8;
                }
            });
        }
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter a valid length".into());
        }
        if let Some(i) = self.spec.edges.iter().position(|e| e.pitch <= 0.0) {
            return Some(format!("Edge {} needs a pitch above zero", i + 1));
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.roof_baseline(ui),
            1 => self.polyline(ui),
            2 => self.selected_line(ui),
            3 => self.line_style(ui),
            _ => self.fill_style(ui),
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        let n = self.points.len();
        if n < 3 {
            return;
        }
        let (mut lo, mut hi) = (self.points[0], self.points[0]);
        for q in &self.points {
            lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let inner = area.shrink(18.0);
        let k = (f64::from(inner.width()) / w).min(f64::from(inner.height()) / h);
        let to = |q: Point| {
            Pos2::new(
                inner.center().x + ((q.x - (lo.x + hi.x) * 0.5) * k) as f32,
                inner.center().y - ((q.y - (lo.y + hi.y) * 0.5) * k) as f32,
            )
        };
        let mut ring: Vec<Pos2> = self.points.iter().map(|q| to(*q)).collect();
        ring.push(ring[0]);
        p.add(Shape::line(ring, Stroke::new(1.5_f32, PV_INK)));
        for i in 0..n {
            let (a, b) = (to(self.points[i]), to(self.points[(i + 1) % n]));
            let on = i == self.selected;
            if on {
                p.line_segment([a, b], Stroke::new(3.0_f32, PV_ACCENT));
            }
            let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
            let toward = Pos2::new(inner.center().x - mid.x, inner.center().y - mid.y);
            let len = toward.to_vec2().length().max(1.0);
            let at = Pos2::new(mid.x + toward.x / len * 12.0, mid.y + toward.y / len * 12.0);
            p.text(
                at,
                Align2::CENTER_CENTER,
                directive_text(&self.spec.edges[i]),
                FontId::proportional(10.0),
                if on { PV_ACCENT } else { PV_FAINT },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Opening, hosting and applying
// ---------------------------------------------------------------------------

fn look_of(cx: &EditorContext, floor: usize, id: Id) -> Look {
    let a = cx.project.floors[floor].cad_attrs(id).unwrap_or_default();
    Look {
        dash: a.dash,
        color: a.color,
        weight: a.weight,
        fill: a.fill,
    }
}

/// The dialog for baseline polyline `id` of the active floor, starting on
/// edge `edge`.
pub fn dialog_for(cx: &EditorContext, id: Id, edge: Option<usize>) -> Option<RoofBaselineDialog> {
    let b = rb::baseline(cx.floor(), id)?;
    Some(RoofBaselineDialog::new(
        cx.floor,
        &b,
        look_of(cx, cx.floor, id),
        edge.unwrap_or(0),
        &rb::default_edge(cx),
    ))
}

/// Writes the dialog's draft: the specification and the line and fill
/// attributes, as one undo step.
pub fn apply(
    cx: &mut EditorContext,
    floor: usize,
    id: Id,
    spec: &RoofBaseline,
    look: &Look,
) -> bool {
    if cx
        .project
        .floors
        .get(floor)
        .and_then(|f| rb::baseline(f, id))
        .is_none()
    {
        return false;
    }
    cx.begin_change("Roof Baseline Specification");
    let ok = rb::set_spec(&mut cx.project, floor, id, spec.clone());
    if !ok {
        cx.cancel_change();
        return false;
    }
    cx.project.edit_cad_attrs(floor, id, |a| {
        a.dash = look.dash;
        a.color = look.color;
        a.weight = look.weight;
        a.fill = look.fill.clone();
    });
    cx.mark_dirty();
    cx.refresh();
    true
}

/// The Join Curved Roof Plane dialog: join edge `edge` of plane `a` to plane
/// `b`, keeping the radius or the angle at the ridge.
pub struct JoinCurvedDialog {
    frame: SpecDialog,
    page: JoinPage,
    floor: usize,
    a: Id,
    edge: usize,
    b: Id,
}

struct JoinPage {
    lock: JoinLock,
}

const JOIN_TABS: &[Tab] = &[on("Join Curved Roof Plane")];

impl SpecPages for JoinPage {
    fn tabs(&self) -> &'static [Tab] {
        JOIN_TABS
    }

    fn error(&self) -> Option<String> {
        None
    }

    fn page(&mut self, ui: &mut Ui, _tab: usize) {
        section(ui, "Join Curved Roof Plane");
        ui.label("The curved plane needs a different curve to meet the other plane.");
        ui.radio_value(
            &mut self.lock,
            JoinLock::Radius,
            "Lock the Radius to Roof Surface (the angle at the ridge changes)",
        );
        ui.radio_value(
            &mut self.lock,
            JoinLock::AngleAtRidge,
            "Lock the Angle at Ridge (the curvature changes)",
        );
    }

    fn preview(&self, p: &Painter, area: Rect) {
        let c = area.center();
        let r = area.width().min(area.height()) * 0.3;
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let t = std::f32::consts::PI * (0.15 + 0.7 * i as f32 / 16.0);
                Pos2::new(c.x - r * 1.1 * t.cos(), c.y + r * 0.6 - r * 1.1 * t.sin())
            })
            .collect();
        p.add(Shape::line(pts, Stroke::new(2.0_f32, PV_INK)));
        p.line_segment(
            [
                Pos2::new(c.x - r * 1.3, c.y + r * 0.6),
                Pos2::new(c.x + r * 1.3, c.y + r * 0.6),
            ],
            Stroke::new(1.0_f32, PV_FAINT),
        );
    }
}

impl JoinCurvedDialog {
    pub fn lock(&self) -> JoinLock {
        self.page.lock
    }

    pub fn set_lock(&mut self, lock: JoinLock) {
        self.page.lock = lock;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.page)
    }
}

thread_local! {
    static HOST: RefCell<Option<RoofBaselineDialog>> = const { RefCell::new(None) };
    static JOIN: RefCell<Option<JoinCurvedDialog>> = const { RefCell::new(None) };
}

/// Opens the Roof Baseline Specification of polyline `id` of the active
/// floor, on edge `edge` when given. False when `id` is not a baseline.
pub fn open_spec(cx: &EditorContext, id: Id, edge: Option<usize>) -> bool {
    let d = dialog_for(cx, id, edge);
    let opened = d.is_some();
    if opened {
        HOST.with(|h| *h.borrow_mut() = d);
    }
    opened
}

/// Opens the Join Curved Roof Plane dialog for a join of edge `edge` of plane
/// `a` to plane `b` on floor `floor`.
pub fn open_join_curved(_cx: &EditorContext, floor: usize, a: Id, edge: usize, b: Id) {
    JOIN.with(|j| {
        *j.borrow_mut() = Some(JoinCurvedDialog {
            frame: SpecDialog::new("Join Curved Roof Plane", "join_curved"),
            page: JoinPage {
                lock: JoinLock::Radius,
            },
            floor,
            a,
            edge,
            b,
        });
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn join_dialog_open() -> bool {
    JOIN.with(|j| j.borrow().is_some())
}

/// Test access to the open Roof Baseline Specification.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut RoofBaselineDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Test access to the open Join Curved Roof Plane dialog.
#[cfg(test)]
pub fn with_join_dialog<R>(f: impl FnOnce(&mut JoinCurvedDialog) -> R) -> Option<R> {
    JOIN.with(|j| j.borrow_mut().as_mut().map(f))
}

/// Closes the open Roof Baseline Specification and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    apply(cx, d.floor, d.id, &d.form.spec, &d.form.look)
}

/// Closes the open Join Curved Roof Plane dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_join(cx: &mut EditorContext) -> bool {
    let Some(d) = JOIN.with(|j| j.borrow_mut().take()) else {
        return false;
    };
    rb::join_curved(cx, d.floor, d.a, d.edge, d.b, d.page.lock)
}

/// Shows the open dialogs once a frame and applies their OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    if let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
            Outcome::Cancel => {}
            Outcome::Ok => {
                apply(cx, d.floor, d.id, &d.form.spec, &d.form.look);
            }
        }
    }
    if let Some(mut d) = JOIN.with(|j| j.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Open => JOIN.with(|j| *j.borrow_mut() = Some(d)),
            Outcome::Cancel => {}
            Outcome::Ok => {
                rb::join_curved(cx, d.floor, d.a, d.edge, d.b, d.page.lock);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Curved Roof section of the Roof Plane Specification
// ---------------------------------------------------------------------------

/// The Curved Roof check box and its fields for `plane` (General panel): the
/// Angle at Eave, Angle at Ridge and Radius to Roof Surface, which move
/// together, and the Facet Angle. Changing one angle keeps the pitch.
pub fn curved_section(ui: &mut Ui, plane: &mut RoofPlaneRecord, fields: &mut Fields) {
    let mut curved = plane.curved.is_some();
    if ui.checkbox(&mut curved, "Curved Roof").changed() {
        plane.curved = curved.then(|| CurvedSpec::straight(plane.pitch));
    }
    let Some(mut c) = plane.curved else {
        return;
    };
    let run = plan_roof::plane_run(&plane.to_roof_plane(0));
    let pitch = plane.pitch;
    let angle = |ui: &mut Ui, label: &str, v: &mut f64| {
        row(ui, label, |ui| {
            ui.add(
                egui::DragValue::new(v)
                    .range(-89.0..=89.0)
                    .speed(0.1)
                    .max_decimals(2)
                    .suffix("\u{b0}"),
            )
            .changed()
        })
    };
    let mut eave = c.angle_at_eave;
    if angle(ui, "Angle at Eave", &mut eave) {
        c = c.with_eave_angle(pitch, eave);
    }
    let mut ridge = c.angle_at_ridge;
    if angle(ui, "Angle at Ridge", &mut ridge) {
        c = c.with_ridge_angle(pitch, ridge);
    }
    let mut radius = c.radius(run);
    if fields.length_row(ui, "Radius to Roof Surface", "curve_radius", &mut radius) {
        if radius <= 0.0 {
            c = CurvedSpec::straight(pitch);
        } else if let Some(next) = c.with_radius(pitch, run, radius) {
            c = next;
        }
    }
    ui.checkbox(&mut c.auto_facet, "Automatic Facet Angle");
    ui.add_enabled_ui(!c.auto_facet, |ui| {
        row(ui, "Facet Angle", |ui| {
            ui.add(
                egui::DragValue::new(&mut c.facet_angle)
                    .range(1.0..=45.0)
                    .speed(0.1)
                    .suffix("\u{b0}"),
            );
        });
    });
    ui.weak(format!(
        "{} facet{} of {:.2}\u{b0} in 3D views",
        c.facet_count(),
        if c.facet_count() == 1 { "" } else { "s" },
        c.facet_angle_used()
    ));
    plane.curved = Some(c);
}

#[allow(dead_code)]
const _PRIM: Option<&Point> = None;
