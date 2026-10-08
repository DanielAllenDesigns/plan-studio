//! Symbol Specification (CB-60): General (size, elevation, position, angle),
//! Options (flip), Layer and Label for a placed library symbol.

// The shell opens this dialog; until it is wired the items are unused.
#![allow(dead_code)]

use super::{
    dis_combo, off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_INK,
    PV_WALL,
};
use crate::editor::placed::{placed_symbol_strokes, stroke_polylines, symbol_placement};
use crate::tools::library::library_catalog;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use plan_core::geometry::Point;
use plan_core::PlacedSymbol;
use plan_library::Placement;

const TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    off("3D"),
    on("Layer"),
    on("Label"),
    off("Components"),
    off("Object Information"),
];

pub struct SymbolDialog {
    frame: SpecDialog,
    form: SymbolForm,
}

struct SymbolForm {
    draft: PlacedSymbol,
    name: String,
    category: String,
    placement: Placement,
    /// Library size, for "Reset to Library Size".
    library_size: Option<(f64, f64, f64)>,
    layers: Vec<String>,
    fields: Fields,
}

fn placement_name(p: Placement) -> &'static str {
    match p {
        Placement::WallMounted => "Wall mounted",
        Placement::FreeStanding => "Free standing",
        Placement::Ceiling => "Ceiling",
        Placement::Countertop => "Countertop",
    }
}

impl SymbolDialog {
    /// A dialog for `symbol`; `layers` are the plan's layer names.
    pub fn new(symbol: PlacedSymbol, layers: Vec<String>) -> Self {
        let item = library_catalog().get(&symbol.catalog_id);
        let form = SymbolForm {
            name: item.map_or_else(|| symbol.catalog_id.clone(), |i| i.name.clone()),
            category: item.map_or_else(String::new, |i| i.category.join(" > ")),
            placement: symbol_placement(&symbol),
            library_size: item.map(|i| (i.width, i.depth, i.height)),
            layers,
            fields: Fields::default(),
            draft: symbol,
        };
        Self {
            frame: SpecDialog::new("Symbol Specification", "symbol"),
            form,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &PlacedSymbol {
        &self.form.draft
    }
}

impl SymbolForm {
    fn general(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Symbol");
        row(ui, "Name", |ui| ui.label(&self.name));
        row(ui, "Category", |ui| ui.label(&self.category));
        row(ui, "Placement", |ui| {
            ui.label(placement_name(self.placement))
        });
        section(ui, "Size");
        f.length_row(ui, "Width", "width", &mut d.width);
        f.length_row(ui, "Depth", "depth", &mut d.depth);
        f.length_row(ui, "Height", "height", &mut d.height);
        if let Some((w, dp, h)) = self.library_size {
            if ui.button("Reset to Library Size").clicked() {
                d.width = w;
                d.depth = dp;
                d.height = h;
            }
        }
        section(ui, "Position");
        f.length_row(ui, "Elevation (from floor)", "elev", &mut d.elevation);
        f.length_row(ui, "Position X (back center)", "pos_x", &mut d.position.x);
        f.length_row(ui, "Position Y (back center)", "pos_y", &mut d.position.y);
        f.degrees_row(ui, "Angle", "deg_angle", &mut d.angle);
    }

    fn options(&mut self, ui: &mut Ui) {
        section(ui, "Options");
        ui.checkbox(&mut self.draft.flip, "Flip (mirror left to right)");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        let d = &mut self.draft;
        row(ui, "Layer", |ui| {
            if self.layers.is_empty() {
                dis_combo(ui, "sym_layer", &d.layer);
                return;
            }
            egui::ComboBox::from_id_salt("sym_layer")
                .selected_text(d.layer.clone())
                .show_ui(ui, |ui| {
                    for l in &self.layers {
                        ui.selectable_value(&mut d.layer, l.clone(), l);
                    }
                });
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        row(ui, "Label", |ui| {
            ui.text_edit_singleline(&mut self.draft.label);
        });
    }
}

impl SpecPages for SymbolForm {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = &self.draft;
        (d.width < 1.0 || d.depth < 1.0 || d.height <= 0.0)
            .then(|| "Width, depth and height must be positive".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS.get(tab).map(|t| t.name) {
            Some("General") => self.general(ui),
            Some("Options") => self.options(ui),
            Some("Layer") => self.layer(ui),
            Some("Label") => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let mut s = self.draft.clone();
        s.position = Point::ZERO;
        s.angle = 0.0;
        let foot = s.footprint();
        let mut pts: Vec<Point> = foot.to_vec();
        let lines = placed_symbol_strokes(&s).map(|sym| stroke_polylines(&sym));
        if let Some(lines) = &lines {
            pts.extend(lines.iter().flat_map(|(v, _)| v.iter().copied()));
        }
        let (mut lo, mut hi) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for q in &pts {
            lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        let area = rect.shrink2(Vec2::new(4.0, 16.0));
        let (w, h) = ((hi.x - lo.x).max(1.0) as f32, (hi.y - lo.y).max(1.0) as f32);
        let scale = (area.width() / w).min(area.height() / h);
        let size = Vec2::new(w * scale, h * scale);
        let tl = area.center() - size * 0.5;
        let map = |q: Point| {
            Pos2::new(
                tl.x + (q.x - lo.x) as f32 * scale,
                tl.y + size.y - (q.y - lo.y) as f32 * scale,
            )
        };
        let ink = Stroke::new(1.0_f32, PV_INK);
        let outline: Vec<Pos2> = foot.iter().map(|q| map(*q)).collect();
        painter.add(egui::Shape::closed_line(
            outline,
            Stroke::new(0.8_f32, PV_WALL),
        ));
        painter.rect_stroke(
            Rect::from_min_size(tl, size),
            0.0,
            Stroke::new(0.5_f32, PV_WALL),
            StrokeKind::Outside,
        );
        match lines {
            Some(lines) => {
                for (v, closed) in lines {
                    let mut sp: Vec<Pos2> = v.iter().map(|q| map(*q)).collect();
                    if closed {
                        if let Some(first) = sp.first().copied() {
                            sp.push(first);
                        }
                    }
                    painter.add(egui::Shape::line(sp, ink));
                }
            }
            None => pv_text(
                painter,
                rect.center(),
                Align2::CENTER_CENTER,
                "No symbol drawing",
                10.0,
            ),
        }
        pv_text(
            painter,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            &self.name,
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_edits_a_draft_and_validates() {
        let item = library_catalog()
            .all_items()
            .find(|i| i.placement == Placement::FreeStanding)
            .unwrap();
        let s = PlacedSymbol::new(
            item.id.clone(),
            Point::new(10.0, 20.0),
            item.width,
            item.depth,
            item.height,
        );
        let mut dlg = SymbolDialog::new(s, vec!["CAD, Default".into(), "Electrical".into()]);
        assert_eq!(dlg.form.name, item.name);
        assert!(dlg.form.error().is_none());
        dlg.form.draft.flip = true;
        dlg.form.draft.width = 0.0;
        assert!(dlg.form.error().is_some());
        dlg.form.draft.width = 30.0;
        assert!(dlg.draft().flip);

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = dlg.show(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                for tab in 0..TABS.len() {
                    dlg.form.page(ui, tab);
                }
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(220.0, 300.0), egui::Sense::hover());
                dlg.form.preview(&painter, painter.clip_rect());
            });
        });
        // An unknown catalog id still opens.
        let unknown = SymbolDialog::new(
            PlacedSymbol::new("nope", Point::ZERO, 10.0, 10.0, 10.0),
            Vec::new(),
        );
        assert_eq!(unknown.form.name, "nope");
    }
}
