//! CAD Line / Polyline Specification (CAD-13, CAD-16; tabs General, Line
//! Style, Fill Style and Layer). One dialog serves every CAD primitive the
//! model has: lines, polylines (rectangles, polygons, ellipses and splines
//! are polylines), circles and arcs. Text has its own dialog
//! (`dialogs/text.rs`).
//!
//! General edits the geometry: a line by its end points, length and angle; a
//! circle by center and radius; an arc by center, radius and angles; a
//! polyline lists its vertices and can be closed or opened. Line Style, Fill
//! Style and Arrow edit the object's own look (`plan_core::cad::CadAttrs`):
//! color, weight and dash style instead of the layer's, a solid or pattern
//! fill (a pattern is drawn as hatch lines grouped with the shape) and the
//! arrow ends of an open shape. The layer is editable. Everything applies as
//! one undo step.
//!
//! [`open_for`] builds the dialog for an `ObjectRef::Cad` that is not text;
//! call [`CadDialog::show`] each frame and [`CadDialog::apply`] on
//! `Outcome::Ok`.

#![allow(dead_code)]

pub mod blocks;
pub mod locks;

use self::locks::{ArcEdit, ArcLock, ArcShape, LineEdit, LineLock};
use super::{
    fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_FAINT, PV_INK,
};
use crate::editor::selection::cad_by_id;
use crate::editor::{EditorContext, ObjectRef};
use crate::tools::cad::{apply_hatch, plan_hatch, HATCHES};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use plan_core::cad::{ArrowStyle, CadAttrs, CadItem, FillAttr};
use plan_core::geometry::{polygon_area, Point};
use plan_core::{CadObject, Id, LineStyle};
use std::f64::consts::TAU;

const OPEN_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Arrow"), on("Layer")];
const CLOSED_TABS: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Fill Style"),
    on("Layer"),
];

/// What the object's layer says about how it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerLook {
    pub name: String,
    pub color: [u8; 3],
    /// Hundredths of a millimetre.
    pub weight: u32,
    pub style: LineStyle,
}

pub struct CadDialog {
    frame: SpecDialog,
    form: CadForm,
}

struct CadForm {
    orig: CadObject,
    draft: CadObject,
    orig_attrs: CadAttrs,
    attrs: CadAttrs,
    layers: Vec<LayerLook>,
    fields: Fields,
    /// What a line / an arc keeps fixed while a value changes (CAD-116).
    line_lock: LineLock,
    arc_lock: ArcLock,
}

impl CadDialog {
    pub fn new(obj: CadObject, layers: Vec<LayerLook>) -> Self {
        let title = match obj.item {
            CadItem::Line { .. } => "CAD Line Specification",
            CadItem::Polyline { .. } => "CAD Polyline Specification",
            CadItem::Circle { .. } => "CAD Circle Specification",
            CadItem::Arc { .. } => "CAD Arc Specification",
            CadItem::Text { .. } => "CAD Specification",
        };
        Self {
            frame: SpecDialog::new(title, "cad"),
            form: CadForm {
                orig_attrs: CadAttrs::new(obj.id),
                attrs: CadAttrs::new(obj.id),
                orig: obj.clone(),
                draft: obj,
                layers,
                fields: Fields::default(),
                line_lock: LineLock::default(),
                arc_lock: ArcLock::default(),
            },
        }
    }

    /// The object's own look (color, weight, fill, arrows) to start from.
    pub fn with_attrs(mut self, attrs: CadAttrs) -> Self {
        self.form.orig_attrs = attrs.clone();
        self.form.attrs = attrs;
        self
    }

    pub fn attrs(&self) -> &CadAttrs {
        &self.form.attrs
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn id(&self) -> Id {
        self.form.orig.id
    }

    pub fn draft(&self) -> &CadObject {
        &self.form.draft
    }

    /// Test access: the draft the form edits, so a scenario can change a
    /// value and press OK without typing into the egui widgets.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut CadObject {
        &mut self.form.draft
    }

    /// Stores the edited object and its look (one undo step). Returns false
    /// when nothing changed, the object is gone, its layer is locked or the
    /// fill cannot be drawn.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let new = &self.form.draft;
        let attrs_changed = self.form.attrs != self.form.orig_attrs;
        if *new == self.form.orig && !attrs_changed {
            return false;
        }
        if !cx.check_unlocked(ObjectRef::Cad(new.id)) {
            return false;
        }
        if cx.layers().is_locked(&new.layer) {
            cx.status = format!("The layer \"{}\" is locked", new.layer);
            return false;
        }
        let fl = cx.floor;
        if cad_by_id(cx.floor(), new.id).is_none() {
            return false;
        }
        // Work out a changed fill first so a failure leaves the plan alone.
        let old_fill = self.form.orig_attrs.fill.as_ref();
        let new_fill = self.form.attrs.fill.as_ref();
        let refill = match new_fill {
            Some(f)
                if old_fill.is_none_or(|o| o.pattern != f.pattern || o.spacing != f.spacing) =>
            {
                let choice = HATCHES
                    .iter()
                    .position(|(n, _)| *n == f.pattern)
                    .unwrap_or(0);
                match plan_hatch(cx, new.id, choice, f.spacing) {
                    Ok(job) => Some(job),
                    Err(e) => {
                        cx.status = e;
                        return false;
                    }
                }
            }
            _ => None,
        };
        cx.begin_change("Change CAD Object");
        if let Some(slot) = cx.project.floors[fl]
            .cad
            .iter_mut()
            .find(|c| c.id == new.id)
        {
            *slot = new.clone();
        }
        if attrs_changed {
            let mut a = self.form.attrs.clone();
            a.target = new.id;
            cx.project.set_cad_attrs(fl, a);
        }
        if let Some(job) = refill {
            apply_hatch(cx, job);
        } else if new_fill.is_none() {
            // No Fill: the hatch lines of an earlier fill go.
            for l in old_fill.map(|f| f.lines.clone()).unwrap_or_default() {
                cx.project.remove_cad(fl, l);
            }
            let me = plan_core::ObjectRef::Cad(new.id);
            let f = &mut cx.project.floors[fl];
            for g in &mut f.groups {
                g.members.retain(|m| *m == me || matches!(m, plan_core::ObjectRef::Cad(id) if f.cad.iter().any(|c| c.id == *id)));
            }
            f.groups.retain(|g| g.members.len() >= 2);
        }
        cx.mark_dirty();
        true
    }
}

/// The CAD specification for `o`, if it is a non-text CAD object of the
/// current floor.
pub fn open_for(cx: &EditorContext, o: ObjectRef) -> Option<CadDialog> {
    let (ObjectRef::Cad(id) | ObjectRef::Text(id)) = o else {
        return None;
    };
    let obj = cad_by_id(cx.floor(), id)?;
    if matches!(obj.item, CadItem::Text { .. }) {
        return None;
    }
    let layers = cx
        .layers()
        .layers
        .iter()
        .map(|l| LayerLook {
            name: l.name.clone(),
            color: l.color,
            weight: l.line_weight,
            style: l.line_style,
        })
        .collect();
    Some(
        CadDialog::new(obj.clone(), layers).with_attrs(
            cx.floor()
                .cad_attrs(id)
                .unwrap_or_else(|| CadAttrs::new(id)),
        ),
    )
}

fn style_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-Dot",
    }
}

fn polar(len: f64, deg: f64) -> Point {
    Point::new(deg.to_radians().cos() * len, deg.to_radians().sin() * len)
}

fn perimeter(points: &[Point], closed: bool) -> f64 {
    let mut p: f64 = points.windows(2).map(|w| w[0].dist(w[1])).sum();
    if closed && points.len() > 2 {
        p += points[points.len() - 1].dist(points[0]);
    }
    p
}

impl CadForm {
    fn is_closed_shape(&self) -> bool {
        matches!(
            self.draft.item,
            CadItem::Polyline { closed: true, .. } | CadItem::Circle { .. }
        )
    }

    fn tab_name(&self, tab: usize) -> &'static str {
        self.tabs()[tab].name
    }

    fn general(&mut self, ui: &mut Ui) {
        let fields = &mut self.fields;
        let (line_lock, arc_lock) = (&mut self.line_lock, &mut self.arc_lock);
        match &mut self.draft.item {
            CadItem::Line { a, b } => {
                section(ui, "Line");
                let lock = *line_lock;
                row(ui, "Lock", |ui| {
                    for l in LineLock::ALL {
                        ui.radio_value(line_lock, l, l.label());
                    }
                });
                let mut edit = None;
                let (mut sp, mut ep) = (*a, *b);
                ui.add_enabled_ui(lock.start_free(), |ui| {
                    let x = fields.length_row(ui, "Start X", "start_x", &mut sp.x);
                    let y = fields.length_row(ui, "Start Y", "start_y", &mut sp.y);
                    if x || y {
                        edit = Some(LineEdit::Start(sp));
                    }
                });
                ui.add_enabled_ui(lock.end_free(), |ui| {
                    let x = fields.length_row(ui, "End X", "end_x", &mut ep.x);
                    let y = fields.length_row(ui, "End Y", "end_y", &mut ep.y);
                    if x || y {
                        edit = Some(LineEdit::End(ep));
                    }
                });
                section(ui, "Length and Angle");
                let mut len = a.dist(*b);
                let mut deg = if len > 1e-9 {
                    b.sub(*a).angle().to_degrees().rem_euclid(360.0)
                } else {
                    0.0
                };
                ui.add_enabled_ui(lock.length_angle_free(), |ui| {
                    if fields.length_row(ui, "Length", "len", &mut len) && len > 0.0 {
                        edit = Some(LineEdit::Length(len));
                    }
                    if fields.degrees_row(ui, "Angle", "deg_angle", &mut deg) {
                        edit = Some(LineEdit::Angle(deg));
                    }
                });
                if let Some(e) = edit {
                    (*a, *b) = locks::line_edit(*a, *b, lock, e);
                }
            }
            CadItem::Circle { center, radius } => {
                section(ui, "Circle");
                fields.length_row(ui, "Center X", "cx", &mut center.x);
                fields.length_row(ui, "Center Y", "cy", &mut center.y);
                fields.length_row(ui, "Radius", "radius", radius);
                let mut dia = *radius * 2.0;
                if fields.length_row(ui, "Diameter", "diameter", &mut dia) && dia > 0.0 {
                    *radius = dia * 0.5;
                }
            }
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                section(ui, "Arc");
                let lock = *arc_lock;
                row(ui, "Lock", |ui| {
                    for l in ArcLock::ALL {
                        ui.radio_value(arc_lock, l, l.label());
                    }
                });
                let shape = ArcShape {
                    center: *center,
                    radius: *radius,
                    a0: *start_angle,
                    a1: *end_angle,
                };
                let mut edit = None;
                let mut c = *center;
                ui.add_enabled_ui(lock.center_free(), |ui| {
                    let x = fields.length_row(ui, "Center X", "cx", &mut c.x);
                    let y = fields.length_row(ui, "Center Y", "cy", &mut c.y);
                    if x || y {
                        edit = Some(ArcEdit::Center(c));
                    }
                });
                let mut r = *radius;
                ui.add_enabled_ui(lock.radius_free(), |ui| {
                    if fields.length_row(ui, "Radius", "radius", &mut r) && r > 0.0 {
                        edit = Some(ArcEdit::Radius(r));
                    }
                });
                let mut s = start_angle.to_degrees();
                let mut e = end_angle.to_degrees();
                ui.add_enabled_ui(lock.angles_free(), |ui| {
                    if fields.degrees_row(ui, "Start Angle", "deg_start", &mut s) {
                        edit = Some(ArcEdit::StartAngle(s.to_radians()));
                    }
                    if fields.degrees_row(ui, "End Angle", "deg_end", &mut e) {
                        edit = Some(ArcEdit::EndAngle(e.to_radians()));
                    }
                });
                if let Some(ed) = edit {
                    let after = locks::arc_edit(shape, lock, ed);
                    *center = after.center;
                    *radius = after.radius;
                    *start_angle = after.a0;
                    *end_angle = after.a1;
                }
                let after = ArcShape {
                    center: *center,
                    radius: *radius,
                    a0: *start_angle,
                    a1: *end_angle,
                };
                let (chord_len, chord_deg) = locks::chord(&after);
                let (d0, d1) = locks::directions(&after);
                row(ui, "Chord Length", |ui| {
                    ui.label(fmt_short(chord_len));
                });
                row(ui, "Chord Angle", |ui| {
                    ui.label(format!("{:.1}\u{b0}", chord_deg.rem_euclid(360.0)));
                });
                row(ui, "Start / End Direction", |ui| {
                    ui.label(format!("{d0:.1}\u{b0} / {d1:.1}\u{b0}"));
                });
                let sweep = (*end_angle - *start_angle).rem_euclid(TAU);
                row(ui, "Sweep", |ui| {
                    ui.label(format!("{:.1}\u{b0}", sweep.to_degrees()));
                });
                row(ui, "Arc Length", |ui| {
                    ui.label(fmt_short(*radius * sweep));
                });
            }
            CadItem::Polyline { points, closed } => {
                section(ui, "Polyline");
                ui.checkbox(closed, "Closed");
                row(ui, "Vertices", |ui| {
                    ui.label(points.len().to_string());
                });
                row(ui, "Perimeter", |ui| {
                    ui.label(fmt_short(perimeter(points, *closed)));
                });
                if *closed {
                    row(ui, "Area", |ui| {
                        ui.label(format!("{:.1} sq ft", polygon_area(points).abs() / 144.0));
                    });
                }
                section(ui, "Vertex List");
                egui::ScrollArea::vertical()
                    .id_salt("cad_vertices")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (i, p) in points.iter().enumerate() {
                            ui.label(format!(
                                "{}:  {}, {}",
                                i + 1,
                                fmt_short(p.x),
                                fmt_short(p.y)
                            ));
                        }
                    });
            }
            CadItem::Text { .. } => {
                ui.label("Text objects use the Text Specification.");
            }
        }
        self.label_boxes(ui);
    }

    /// Show Length, Show Angle (Radius on an arc), All Angles and Reverse
    /// Angle (CAD-118): live labels on the edges, in the Number Style.
    fn label_boxes(&mut self, ui: &mut Ui) {
        let (arc, poly, drawn) = match self.draft.item {
            CadItem::Line { .. } => (false, false, true),
            CadItem::Polyline { .. } => (false, true, true),
            CadItem::Arc { .. } => (true, false, true),
            _ => (false, false, false),
        };
        if !drawn {
            return;
        }
        section(ui, "Labels");
        let l = &mut self.attrs.labels;
        ui.checkbox(&mut l.show_length, "Show Length");
        if arc {
            ui.checkbox(&mut l.show_radius, "Show Radius");
        } else {
            ui.checkbox(&mut l.show_angle, "Show Angle");
            if poly {
                ui.checkbox(&mut l.all_angles, "All Angles");
            }
            ui.checkbox(&mut l.reverse_angle, "Reverse Angle");
        }
    }

    fn look(&self) -> Option<&LayerLook> {
        self.layers.iter().find(|l| l.name == self.draft.layer)
    }

    fn line_style(&mut self, ui: &mut Ui) {
        let look = self.look().cloned();
        let layer_weight = look.as_ref().map_or(25, |l| l.weight);
        let layer_color = look.as_ref().map_or([0, 0, 0], |l| l.color);
        let layer_style = look.as_ref().map_or(LineStyle::Solid, |l| l.style);
        let attrs = &mut self.attrs;
        section(ui, "Line Weight");
        let mut own = attrs.weight.is_some();
        if ui.checkbox(&mut own, "Use a weight of its own").changed() {
            attrs.weight = own.then_some(layer_weight);
        }
        match &mut attrs.weight {
            Some(w) => {
                let mut mm = f64::from(*w) / 100.0;
                row(ui, "Line Weight", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut mm)
                            .range(0.05..=5.0)
                            .speed(0.01)
                            .suffix(" mm"),
                    )
                });
                *w = (mm * 100.0).round() as u32;
            }
            None => {
                row(ui, "Line Weight", |ui| {
                    ui.label(format!("{:.2} mm (layer)", f64::from(layer_weight) / 100.0))
                });
            }
        }
        section(ui, "Color");
        let mut own = attrs.color.is_some();
        if ui.checkbox(&mut own, "Use a color of its own").changed() {
            attrs.color = own.then_some(layer_color);
        }
        match &mut attrs.color {
            Some(c) => {
                row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
            }
            None => {
                row(ui, "Color", |ui| {
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(36.0, 14.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        r,
                        2.0,
                        Color32::from_rgb(layer_color[0], layer_color[1], layer_color[2]),
                    );
                    ui.painter().rect_stroke(
                        r,
                        2.0,
                        Stroke::new(1.0_f32, PV_INK),
                        egui::StrokeKind::Inside,
                    );
                    ui.label("layer");
                });
            }
        }
        section(ui, "Line Style");
        let mut pick = attrs.dash;
        ui.radio_value(
            &mut pick,
            None,
            format!("By layer ({})", style_name(layer_style)),
        );
        for s in [
            LineStyle::Solid,
            LineStyle::Dashed,
            LineStyle::Dotted,
            LineStyle::DashDot,
        ] {
            ui.radio_value(&mut pick, Some(s), style_name(s));
        }
        attrs.dash = pick;
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        let fields = &mut self.fields;
        let attrs = &mut self.attrs;
        section(ui, "Fill Style");
        #[derive(PartialEq, Clone, Copy)]
        enum Kind {
            None,
            Solid,
            Pattern,
        }
        let kind = match &attrs.fill {
            None => Kind::None,
            Some(f) if f.pattern.is_empty() => Kind::Solid,
            Some(_) => Kind::Pattern,
        };
        let mut pick = kind;
        ui.radio_value(&mut pick, Kind::None, "No Fill");
        ui.radio_value(&mut pick, Kind::Solid, "Solid");
        ui.radio_value(&mut pick, Kind::Pattern, "Pattern");
        if pick != kind {
            attrs.fill = match pick {
                Kind::None => None,
                Kind::Solid => Some(FillAttr {
                    pattern: String::new(),
                    ..attrs.fill.take().unwrap_or_default()
                }),
                Kind::Pattern => Some(FillAttr {
                    pattern: HATCHES[1].0.to_string(),
                    ..attrs.fill.take().unwrap_or_default()
                }),
            };
        }
        let Some(f) = &mut attrs.fill else {
            return;
        };
        section(ui, "Fill");
        row(ui, "Color", |ui| ui.color_edit_button_srgb(&mut f.color));
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut f.opacity, 0..=255).show_value(false))
        });
        ui.weak("A solid fill is drawn by the plan renderer; lower the opacity to see through it.");
        if !f.pattern.is_empty() {
            row(ui, "Pattern", |ui| {
                egui::ComboBox::from_id_salt("cad_fill_pattern")
                    .selected_text(f.pattern.clone())
                    .show_ui(ui, |ui| {
                        for (name, _) in HATCHES.iter().skip(1) {
                            ui.selectable_value(&mut f.pattern, (*name).to_string(), *name);
                        }
                    });
            });
            let spaced = HATCHES.iter().any(|(n, uses)| *n == f.pattern && *uses);
            if spaced {
                fields.length_row(ui, "Spacing", "fill_spacing", &mut f.spacing);
            }
            ui.weak("The pattern is drawn as lines grouped with the shape when you press OK.");
        }
    }

    fn arrow(&mut self, ui: &mut Ui) {
        let fields = &mut self.fields;
        let attrs = &mut self.attrs;
        section(ui, "Arrows");
        for (label, key, sel) in [
            ("Start", "arrow_start", &mut attrs.arrow_start),
            ("End", "arrow_end", &mut attrs.arrow_end),
        ] {
            row(ui, label, |ui| {
                egui::ComboBox::from_id_salt(key)
                    .selected_text(sel.name())
                    .show_ui(ui, |ui| {
                        for a in ArrowStyle::ALL {
                            ui.selectable_value(sel, a, a.name());
                        }
                    });
            });
        }
        if attrs.arrow_start != ArrowStyle::None || attrs.arrow_end != ArrowStyle::None {
            if attrs.arrow_size <= 0.0 {
                attrs.arrow_size = 6.0;
            }
            fields.length_row(ui, "Arrow Size", "arrow_size", &mut attrs.arrow_size);
        }
        ui.weak("Arrows made with Line With Arrow are separate shapes; these settings add arrowheads to the line itself.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("cad_layer")
                .selected_text(self.draft.layer.clone())
                .show_ui(ui, |ui| {
                    for l in &self.layers {
                        ui.selectable_value(&mut self.draft.layer, l.name.clone(), l.name.as_str());
                    }
                });
        });
    }
}

impl SpecPages for CadForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.is_closed_shape() {
            CLOSED_TABS
        } else {
            OPEN_TABS
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft.item {
            CadItem::Line { a, b } if a.dist(*b) < 1e-6 => Some("The line has no length".into()),
            CadItem::Circle { radius, .. } | CadItem::Arc { radius, .. } if *radius <= 0.0 => {
                Some("The radius must be greater than zero".into())
            }
            CadItem::Polyline { points, closed } if points.len() < if *closed { 3 } else { 2 } => {
                Some("The polyline needs more vertices".into())
            }
            _ => None,
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match self.tab_name(tab) {
            "General" => self.general(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Arrow" => self.arrow(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let (lo, hi) = self.draft.item.bounds();
        let w = (hi.x - lo.x).max(1.0);
        let h = (hi.y - lo.y).max(1.0);
        let s = ((rect.width() as f64 - 16.0) / w)
            .min((rect.height() as f64 - 40.0) / h)
            .clamp(0.001, 50.0);
        let c = Point::lerp(lo, hi, 0.5);
        let to = |q: Point| {
            Pos2::new(
                rect.center().x + ((q.x - c.x) * s) as f32,
                rect.center().y - 6.0 - ((q.y - c.y) * s) as f32,
            )
        };
        let ink_color = self
            .attrs
            .color
            .map_or(PV_INK, |c| Color32::from_rgb(c[0], c[1], c[2]));
        let ink = Stroke::new(1.6_f32, ink_color);
        match &self.draft.item {
            CadItem::Line { a, b } => {
                p.line_segment([to(*a), to(*b)], ink);
                p.circle_filled(to(*a), 3.0, PV_ACCENT);
                p.circle_filled(to(*b), 3.0, PV_ACCENT);
            }
            CadItem::Circle { center, radius } => {
                p.circle_stroke(to(*center), (*radius * s) as f32, ink);
            }
            CadItem::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let sweep = (end_angle - start_angle).rem_euclid(TAU);
                let pts: Vec<Pos2> = (0..=32)
                    .map(|i| {
                        let a = start_angle + sweep * f64::from(i) / 32.0;
                        to(Point::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ))
                    })
                    .collect();
                p.add(Shape::line(pts, ink));
            }
            CadItem::Polyline { points, closed } => {
                let pts: Vec<Pos2> = points.iter().map(|q| to(*q)).collect();
                p.add(if *closed {
                    Shape::closed_line(pts, ink)
                } else {
                    Shape::line(pts, ink)
                });
            }
            CadItem::Text { .. } => {}
        }
        p.hline(
            rect.x_range(),
            rect.max.y - 22.0,
            Stroke::new(0.8_f32, PV_FAINT),
        );
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 10.0),
            Align2::CENTER_CENTER,
            self.draft.layer.as_str(),
            11.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn cx_with(item: CadItem) -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_cad(0, "CAD, Default", item);
        (cx, id)
    }

    fn line() -> CadItem {
        CadItem::Line {
            a: Point::ZERO,
            b: Point::new(100.0, 0.0),
        }
    }

    #[test]
    fn opens_for_cad_but_not_text_or_other_objects() {
        let (mut cx, id) = cx_with(line());
        assert!(open_for(&cx, ObjectRef::Cad(id)).is_some());
        let t = cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::ZERO,
                text: "x".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        assert!(open_for(&cx, ObjectRef::Cad(t)).is_none());
        assert!(open_for(&cx, ObjectRef::Wall(id)).is_none());
    }

    #[test]
    fn line_end_and_layer_edits_are_one_undo_step() {
        let (mut cx, id) = cx_with(line());
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(!d.apply(&mut cx));
        if let CadItem::Line { b, .. } = &mut d.form.draft.item {
            *b = Point::new(0.0, 60.0);
        }
        d.form.draft.layer = "Text".into();
        assert!(d.apply(&mut cx));
        let c = cad_by_id(cx.floor(), id).unwrap();
        assert_eq!(c.layer, "Text");
        assert_eq!(
            c.item,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(0.0, 60.0)
            }
        );
        assert_eq!(cx.undo().as_deref(), Some("Change CAD Object"));
        assert_eq!(cad_by_id(cx.floor(), id).unwrap().layer, "CAD, Default");
    }

    #[test]
    fn closed_shapes_get_a_fill_tab() {
        let (cx, id) = cx_with(CadItem::Polyline {
            points: vec![Point::ZERO, Point::new(10.0, 0.0), Point::new(10.0, 10.0)],
            closed: true,
        });
        let d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.form.tabs().iter().any(|t| t.name == "Fill Style"));
        let (cx, id) = cx_with(line());
        let d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(!d.form.tabs().iter().any(|t| t.name == "Fill Style"));
    }

    #[test]
    fn invalid_geometry_blocks_ok() {
        let (cx, id) = cx_with(CadItem::Circle {
            center: Point::ZERO,
            radius: 5.0,
        });
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(d.form.error().is_none());
        if let CadItem::Circle { radius, .. } = &mut d.form.draft.item {
            *radius = 0.0;
        }
        assert!(d.form.error().is_some());
    }

    #[test]
    fn dialog_draws_every_kind_and_tab_without_panicking() {
        let items = [
            line(),
            CadItem::Circle {
                center: Point::new(5.0, 5.0),
                radius: 5.0,
            },
            CadItem::Arc {
                center: Point::ZERO,
                radius: 10.0,
                start_angle: 0.0,
                end_angle: 1.5,
            },
            CadItem::Polyline {
                points: vec![Point::ZERO, Point::new(10.0, 0.0), Point::new(10.0, 10.0)],
                closed: true,
            },
        ];
        let ctx = egui::Context::default();
        for item in items {
            let (cx, id) = cx_with(item);
            let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
            for tab in 0..d.form.tabs().len() {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                    d.show(ctx);
                });
            }
        }
    }

    #[test]
    fn own_line_look_is_stored_with_the_object_in_one_undo_step() {
        let (mut cx, id) = cx_with(line());
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert!(!d.apply(&mut cx), "nothing changed yet");
        d.form.attrs.color = Some([200, 0, 0]);
        d.form.attrs.weight = Some(70);
        d.form.attrs.dash = Some(LineStyle::Dashed);
        d.form.attrs.arrow_end = ArrowStyle::Filled;
        d.form.attrs.arrow_size = 8.0;
        assert!(d.apply(&mut cx));
        let a = cx.floor().cad_attrs(id).unwrap();
        assert_eq!(a.color, Some([200, 0, 0]));
        assert_eq!((a.weight, a.dash), (Some(70), Some(LineStyle::Dashed)));
        assert_eq!(a.arrow_end, ArrowStyle::Filled);
        // Reopened, the dialog shows the stored look and a further change keeps it.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        assert_eq!(d.attrs().weight, Some(70));
        assert!(!d.apply(&mut cx));
        d.form.attrs.weight = None;
        assert!(d.apply(&mut cx));
        assert_eq!(cx.floor().cad_attrs(id).unwrap().weight, None);
        assert_eq!(cx.undo().as_deref(), Some("Change CAD Object"));
        assert_eq!(cx.floor().cad_attrs(id).unwrap().weight, Some(70));
        assert_eq!(cx.undo().as_deref(), Some("Change CAD Object"));
        assert!(cx.floor().cad_attrs(id).is_none());
    }

    #[test]
    fn fill_style_draws_a_hatch_and_no_fill_takes_it_away() {
        let (mut cx, id) = cx_with(CadItem::Polyline {
            points: vec![
                Point::ZERO,
                Point::new(120.0, 0.0),
                Point::new(120.0, 120.0),
                Point::new(0.0, 120.0),
            ],
            closed: true,
        });
        let count = |cx: &EditorContext| {
            cx.floor()
                .cad
                .iter()
                .filter(|c| c.layer == "CAD, Default")
                .count()
        };
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill = Some(FillAttr {
            pattern: "Diagonal Lines".into(),
            spacing: 12.0,
            ..FillAttr::default()
        });
        assert!(d.apply(&mut cx));
        let lines = count(&cx);
        assert!(lines > 8, "{lines}");
        assert_eq!(
            cx.floor().cad_attrs(id).unwrap().fill.unwrap().lines.len(),
            lines - 1
        );
        // A different pattern replaces the lines.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill.as_mut().unwrap().pattern = "Cross Hatch".into();
        assert!(d.apply(&mut cx));
        let cross = count(&cx);
        assert!(cross > lines && cross < lines * 3, "{cross} vs {lines}");
        // A solid fill keeps the outline alone.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill.as_mut().unwrap().pattern = String::new();
        assert!(d.apply(&mut cx));
        assert_eq!(count(&cx), 1);
        // And no fill at all drops the record of it.
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill = Some(FillAttr {
            pattern: "Brick".into(),
            ..FillAttr::default()
        });
        assert!(d.apply(&mut cx));
        assert!(count(&cx) > 1);
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill = None;
        assert!(d.apply(&mut cx));
        assert_eq!(count(&cx), 1);
        assert!(cx.floor().groups.is_empty());
        assert!(cx.floor().cad_attrs(id).is_none());
        // Undo brings the brick back.
        assert_eq!(cx.undo().as_deref(), Some("Change CAD Object"));
        assert!(count(&cx) > 1);
    }

    #[test]
    fn the_arrow_and_style_pages_draw() {
        let (cx, id) = cx_with(line());
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        let ctx = egui::Context::default();
        for name in ["Line Style", "Arrow"] {
            let tab = d.form.tabs().iter().position(|t| t.name == name).unwrap();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
            });
        }
        d.form.attrs.fill = Some(FillAttr::default());
        let (cx, id) = cx_with(CadItem::Circle {
            center: Point::ZERO,
            radius: 5.0,
        });
        let mut d = open_for(&cx, ObjectRef::Cad(id)).unwrap();
        d.form.attrs.fill = Some(FillAttr {
            pattern: "Brick".into(),
            ..FillAttr::default()
        });
        let tab = d
            .form
            .tabs()
            .iter()
            .position(|t| t.name == "Fill Style")
            .unwrap();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
        });
    }
}
