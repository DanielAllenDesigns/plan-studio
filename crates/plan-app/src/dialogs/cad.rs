//! CAD Line / Polyline Specification (CAD-13, CAD-16; tabs General, Line
//! Style, Fill Style and Layer). One dialog serves every CAD primitive the
//! model has: lines, polylines (rectangles, polygons, ellipses and splines
//! are polylines), circles and arcs. Text has its own dialog
//! (`dialogs/text.rs`).
//!
//! General edits the geometry: a line by its end points, length and angle; a
//! circle by center and radius; an arc by center, radius and angles; a
//! polyline lists its vertices and can be closed or opened. The model keeps
//! no per-object line weight, color, dash style or fill, so the Line Style and
//! Fill Style pages show the values the object's layer gives and the fill
//! options disabled. The layer is editable.
//!
//! [`open_for`] builds the dialog for an `ObjectRef::Cad` that is not text;
//! call [`CadDialog::show`] each frame and [`CadDialog::apply`] on
//! `Outcome::Ok`.

#![allow(dead_code)]

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, on, pv_text, row, section, Fields, Outcome,
    SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK,
};
use crate::editor::selection::cad_by_id;
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use plan_core::cad::CadItem;
use plan_core::geometry::{polygon_area, Point};
use plan_core::{CadObject, Id, LineStyle};
use std::f64::consts::TAU;

const OPEN_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
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
    layers: Vec<LayerLook>,
    fields: Fields,
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
                orig: obj.clone(),
                draft: obj,
                layers,
                fields: Fields::default(),
            },
        }
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

    /// Stores the edited object (one undo step). Returns false when nothing
    /// changed, the object is gone or its layer is locked.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let new = &self.form.draft;
        if *new == self.form.orig {
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
        cx.begin_change("Change CAD Object");
        if let Some(slot) = cx.project.floors[fl]
            .cad
            .iter_mut()
            .find(|c| c.id == new.id)
        {
            *slot = new.clone();
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
    Some(CadDialog::new(obj.clone(), layers))
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
        match &mut self.draft.item {
            CadItem::Line { a, b } => {
                section(ui, "Line");
                fields.length_row(ui, "Start X", "start_x", &mut a.x);
                fields.length_row(ui, "Start Y", "start_y", &mut a.y);
                fields.length_row(ui, "End X", "end_x", &mut b.x);
                fields.length_row(ui, "End Y", "end_y", &mut b.y);
                section(ui, "Length and Angle");
                let mut len = a.dist(*b);
                let mut deg = if len > 1e-9 {
                    b.sub(*a).angle().to_degrees().rem_euclid(360.0)
                } else {
                    0.0
                };
                if fields.length_row(ui, "Length", "len", &mut len) && len > 0.0 {
                    *b = a.add(polar(len, deg));
                }
                if fields.degrees_row(ui, "Angle", "deg_angle", &mut deg) {
                    *b = a.add(polar(len, deg));
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
                fields.length_row(ui, "Center X", "cx", &mut center.x);
                fields.length_row(ui, "Center Y", "cy", &mut center.y);
                fields.length_row(ui, "Radius", "radius", radius);
                let mut s = start_angle.to_degrees();
                let mut e = end_angle.to_degrees();
                if fields.degrees_row(ui, "Start Angle", "deg_start", &mut s) {
                    *start_angle = s.to_radians();
                }
                if fields.degrees_row(ui, "End Angle", "deg_end", &mut e) {
                    *end_angle = e.to_radians();
                }
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
    }

    fn look(&self) -> Option<&LayerLook> {
        self.layers.iter().find(|l| l.name == self.draft.layer)
    }

    fn line_style(&mut self, ui: &mut Ui) {
        let look = self.look().cloned();
        section(ui, "Line (by layer)");
        row(ui, "Line Weight", |ui| match &look {
            Some(l) => {
                ui.label(format!("{:.2} mm", f64::from(l.weight) / 100.0));
            }
            None => dis_combo(ui, "cad_weight", "Default"),
        });
        row(ui, "Color", |ui| match &look {
            Some(l) => {
                let (r, _) = ui.allocate_exact_size(Vec2::new(36.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    r,
                    2.0,
                    Color32::from_rgb(l.color[0], l.color[1], l.color[2]),
                );
                ui.painter().rect_stroke(
                    r,
                    2.0,
                    Stroke::new(1.0_f32, PV_INK),
                    egui::StrokeKind::Inside,
                );
            }
            None => dis_combo(ui, "cad_color", "Black"),
        });
        section(ui, "Line Style");
        let style = look.as_ref().map_or(LineStyle::Solid, |l| l.style);
        dis_radio(ui, "Solid", style == LineStyle::Solid);
        dis_radio(ui, "Dashed", style == LineStyle::Dashed);
        dis_radio(ui, "Dotted", style == LineStyle::Dotted);
        dis_radio(ui, "Dash-Dot", style == LineStyle::DashDot);
        ui.weak("Weight, color and style come from the layer until objects can carry their own.");
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        dis_radio(ui, "No Fill", true);
        dis_radio(ui, "Solid", false);
        dis_radio(ui, "Pattern", false);
        dis_check(ui, "Fill Is Transparent", false);
        ui.weak("Fills are not stored in the model yet.");
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
        let ink = Stroke::new(1.6_f32, PV_INK);
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
}
