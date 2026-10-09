//! Specification dialogs of the Slab tools and the platform hole tools:
//! Slab Specification (General, Fill Style, Line Style, Layer), Slab Hole,
//! Pad and Pier Specification (General, Layer) and Platform Hole
//! Specification. Opened by a double-click on the object; the dialog edits a
//! [`Draft`] clone that the tool stores on OK as one undo step.
//!
//! Also the options of the Build Foundation dialog ([`build_options`], R-61,
//! R-62): Walls with Footings, Monolithic Slab and Grade Beams on Piers, each
//! with its own sizes, and the basement or crawl space room to make.

use super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT,
    PV_INK, PV_WALL,
};
use crate::editor::rooms_edit::{FoundationSpec, FoundationType};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::floors::{FoundationRooms, BASEMENT_MIN_CLEAR_HEIGHT};
use plan_core::foundation::{
    bounds, Footing, FoundationLayer, FoundationRef, Pad, Pier, PlatformHole, PlatformKind, Slab,
    SlabHole,
};
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::LineStyle;

const SLAB_TABS: &[Tab] = &[
    on("General"),
    on("Fill Style"),
    on("Line Style"),
    on("Layer"),
];
const HOLE_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
const PAD_TABS: &[Tab] = &[on("General"), on("Layer")];
const PLATFORM_TABS: &[Tab] = &[on("General")];

/// Materials the 3D concrete mesh understands.
pub const MATERIALS: [&str; 3] = ["Concrete", "Stone", "Brick"];
/// Plan fill patterns of a slab.
pub const PATTERNS: [&str; 5] = ["None", "Solid", "Hatch", "Cross Hatch", "Grid"];
const LINE_STYLES: [(LineStyle, &str); 4] = [
    (LineStyle::Solid, "Solid"),
    (LineStyle::Dashed, "Dashed"),
    (LineStyle::Dotted, "Dotted"),
    (LineStyle::DashDot, "Dash Dot"),
];

/// The object being edited.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    Slab(Slab),
    Hole(SlabHole),
    Pad(Pad),
    Pier(Pier),
    Platform(PlatformHole),
}

impl Draft {
    /// The draft of the object `r` on `layer`.
    pub fn of(layer: &FoundationLayer, r: FoundationRef) -> Option<Draft> {
        match r {
            FoundationRef::Slab(i) => layer.slab(i).cloned().map(Draft::Slab),
            FoundationRef::SlabHole(i) => layer.hole(i).cloned().map(Draft::Hole),
            FoundationRef::Pad(i) => layer.pad(i).cloned().map(Draft::Pad),
            FoundationRef::Pier(i) => layer.pier(i).cloned().map(Draft::Pier),
            FoundationRef::PlatformHole(i) => layer.platform_hole(i).cloned().map(Draft::Platform),
        }
    }

    /// Writes the draft back over the object with the same id; returns
    /// whether that object still exists.
    pub fn apply(&self, layer: &mut FoundationLayer) -> bool {
        fn put<T: Clone>(v: &mut [T], id: u64, get: impl Fn(&T) -> u64, new: &T) -> bool {
            v.iter_mut()
                .find(|x| get(x) == id)
                .map(|x| *x = new.clone())
                .is_some()
        }
        match self {
            Draft::Slab(d) => put(&mut layer.slabs, d.id, |s| s.id, d),
            Draft::Hole(d) => put(&mut layer.holes, d.id, |s| s.id, d),
            Draft::Pad(d) => put(&mut layer.pads, d.id, |s| s.id, d),
            Draft::Pier(d) => put(&mut layer.piers, d.id, |s| s.id, d),
            Draft::Platform(d) => put(&mut layer.platform_holes, d.id, |s| s.id, d),
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Draft::Slab(_) => "Slab Specification",
            Draft::Hole(_) => "Slab Hole Specification",
            Draft::Pad(_) => "Square Pad Specification",
            Draft::Pier(_) => "Round Pier Specification",
            Draft::Platform(_) => "Platform Hole Specification",
        }
    }

    fn tabs(&self) -> &'static [Tab] {
        match self {
            Draft::Slab(_) => SLAB_TABS,
            Draft::Hole(_) => HOLE_TABS,
            Draft::Pad(_) | Draft::Pier(_) => PAD_TABS,
            Draft::Platform(_) => PLATFORM_TABS,
        }
    }
}

pub struct FoundationDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: Draft,
    /// Slab holes inside the slab (for the volume line of the General page).
    inner_holes: Vec<SlabHole>,
    layers: Vec<String>,
    fields: Fields,
}

impl FoundationDialog {
    /// The dialog for object `r`, or `None` when it no longer exists.
    /// `layers` are the plan's layer names for the Layer page.
    pub fn new(layer: &FoundationLayer, r: FoundationRef, layers: Vec<String>) -> Option<Self> {
        let draft = Draft::of(layer, r)?;
        let inner_holes = match &draft {
            Draft::Slab(s) => layer.holes_in(s).into_iter().cloned().collect(),
            _ => Vec::new(),
        };
        Some(Self {
            frame: SpecDialog::new(draft.title(), ("foundation_spec", draft.title())),
            form: Form {
                draft,
                inner_holes,
                layers,
                fields: Fields::default(),
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Draft {
        &self.form.draft
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Draft {
        &mut self.form.draft
    }

    #[cfg(test)]
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.form.draft.tabs().iter().map(|t| t.name).collect()
    }
}

/// The Foundation Type list and the options of the chosen type (the body of
/// the Build Foundation dialog; the OK and Cancel buttons are the caller's).
pub fn build_options(ui: &mut Ui, spec: &mut FoundationSpec, fields: &mut Fields) {
    ui.label(egui::RichText::new("Foundation Type").strong());
    for t in FoundationType::ALL {
        ui.radio_value(&mut spec.kind, t, t.name());
    }
    match spec.kind {
        FoundationType::WallsWithFootings => {
            section(ui, "Walls with Footings");
            fields.length_row(ui, "Stem Wall Height", "stem_h", &mut spec.stem_height);
            fields.length_row(
                ui,
                "Minimum Stem Wall",
                "stem_min",
                &mut spec.min_stem_height,
            );
            row(ui, "Footing", |ui| {
                ui.checkbox(&mut spec.footing, "Footing under the walls")
            });
            if spec.footing {
                fields.length_row(ui, "Footing Width", "footing_w", &mut spec.footing_width);
                let (min_w, min_t) =
                    crate::editor::code::footing_limits(spec.stem_height.max(spec.min_stem_height));
                super::code_notice::code_notice(
                    ui,
                    "IRC R403.1.1 footing width (Table R403.1(1))",
                    &mut spec.footing_width,
                    min_w,
                    super::code_notice::LimitKind::Min,
                );
                fields.length_row(ui, "Footing Depth", "footing_d", &mut spec.footing_depth);
                super::code_notice::code_notice(
                    ui,
                    "IRC R403.1.4 / R403.1.1 footing thickness to the frost depth",
                    &mut spec.footing_depth,
                    min_t,
                    super::code_notice::LimitKind::Min,
                );
            }
            section(ui, "Room");
            for (value, label) in [
                (
                    FoundationRooms::Auto,
                    "Automatic (basement from 6' clear, else crawl space)",
                ),
                (FoundationRooms::Basement, "Basement"),
                (FoundationRooms::CrawlSpace, "Crawl Space"),
                (FoundationRooms::None, "No room"),
            ] {
                ui.radio_value(&mut spec.rooms, value, label);
            }
            let clear = spec.stem_height.max(spec.min_stem_height) - rooms_platform_hint();
            ui.weak(format!(
                "Clear height under the first floor about {} ({}).",
                fmt_ft_in(clear.max(0.0)),
                if clear >= BASEMENT_MIN_CLEAR_HEIGHT {
                    "a basement"
                } else {
                    "a crawl space"
                }
            ));
        }
        FoundationType::MonolithicSlab => {
            section(ui, "Monolithic Slab");
            fields.length_row(ui, "Slab Thickness", "slab_t", &mut spec.slab_thickness);
            fields.length_row(
                ui,
                "Stem Wall Height",
                "slab_stem",
                &mut spec.slab_stem_height,
            );
            ui.weak("A thickened edge under the exterior walls and a slab inside it.");
        }
        FoundationType::Piers => {
            section(ui, "Grade Beams on Piers");
            fields.length_row(ui, "Grade Beam Height", "beam_h", &mut spec.beam_height);
            fields.length_row(ui, "Pier Height", "pier_h", &mut spec.pier_height);
            fields.length_row(ui, "Pier Spacing", "pier_s", &mut spec.pier_spacing);
            ui.weak("Piers at every corner and along the walls, a grade beam on top.");
        }
    }
}

/// The first floor's platform, which the clear height of a basement loses
/// (the dialog only has the plan defaults at hand).
fn rooms_platform_hint() -> f64 {
    plan_core::floors::FLOOR_PLATFORM_THICKNESS
}

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for m in MATERIALS {
                ui.selectable_value(value, m.to_string(), m);
            }
        });
}

fn footing_rows(fields: &mut Fields, ui: &mut Ui, footing: &mut Option<Footing>, side: bool) {
    let mut on = footing.is_some();
    row(ui, "Footing", |ui| ui.checkbox(&mut on, "Add footing"));
    if on != footing.is_some() {
        *footing = on.then(Footing::default);
    }
    if let Some(f) = footing {
        let width = if side {
            "Footing size"
        } else {
            "Footing width"
        };
        fields.length_row(ui, width, "footing_width", &mut f.width);
        fields.length_row(ui, "Footing depth", "footing_depth", &mut f.depth);
        super::code_notice::code_notice(
            ui,
            "IRC R403.1.1 footing thickness",
            &mut f.depth,
            crate::editor::code::active().footing_min_thickness,
            super::code_notice::LimitKind::Min,
        );
    }
}

fn layer_page(ui: &mut Ui, layers: &[String], layer: &mut String) {
    section(ui, "Layer");
    row(ui, "Layer", |ui| {
        egui::ComboBox::from_id_salt("foundation_layer")
            .selected_text(layer.clone())
            .show_ui(ui, |ui| {
                for name in layers {
                    ui.selectable_value(layer, name.clone(), name);
                }
            });
    });
}

fn line_style_page(ui: &mut Ui, style: &mut LineStyle) {
    section(ui, "Line Style");
    row(ui, "Line style", |ui| {
        let current = LINE_STYLES
            .iter()
            .find(|(s, _)| s == style)
            .map_or("Solid", |(_, n)| *n);
        egui::ComboBox::from_id_salt("foundation_line_style")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (s, name) in LINE_STYLES {
                    ui.selectable_value(style, s, name);
                }
            });
    });
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let fields = &mut self.fields;
        match &mut self.draft {
            Draft::Slab(s) => {
                fields.length_row(ui, "Thickness", "thickness", &mut s.thickness);
                fields.length_row(ui, "Top height", "top", &mut s.top_elevation);
                footing_rows(fields, ui, &mut s.footing, false);
                row(ui, "Material", |ui| {
                    material_combo(ui, "slab_material", &mut s.material)
                });
                ui.add_space(6.0);
                let extra: Vec<&SlabHole> = self.inner_holes.iter().collect();
                ui.weak(format!(
                    "Area {:.1} sq ft, perimeter {}, concrete {:.2} cu yd",
                    s.net_area(&extra) / 144.0,
                    fmt_ft_in(s.perimeter()),
                    s.concrete_cu_yd(&extra),
                ));
            }
            Draft::Hole(h) => {
                row(ui, "Footing", |ui| {
                    ui.checkbox(&mut h.with_footing, "Footing around the hole")
                });
                ui.add_space(6.0);
                ui.weak(format!("Area {:.1} sq ft", h.area() / 144.0));
            }
            Draft::Pad(p) => {
                fields.length_row(ui, "Size", "size", &mut p.size);
                fields.length_row(ui, "Thickness", "thickness", &mut p.thickness);
                fields.length_row(ui, "Top height", "top", &mut p.elevation);
                row(ui, "Material", |ui| {
                    material_combo(ui, "pad_material", &mut p.material)
                });
                ui.add_space(6.0);
                ui.weak(format!("Concrete {:.3} cu yd", p.concrete_cu_yd()));
            }
            Draft::Pier(p) => {
                fields.length_row(ui, "Diameter", "diameter", &mut p.diameter);
                fields.length_row(ui, "Height", "height", &mut p.height);
                fields.length_row(ui, "Top height", "top", &mut p.elevation);
                footing_rows(fields, ui, &mut p.footing, true);
                row(ui, "Material", |ui| {
                    material_combo(ui, "pier_material", &mut p.material)
                });
                ui.add_space(6.0);
                ui.weak(format!("Concrete {:.3} cu yd", p.concrete_cu_yd()));
            }
            Draft::Platform(h) => {
                row(ui, "Hole in", |ui| {
                    ui.radio_value(&mut h.kind, PlatformKind::Floor, "Floor platform");
                    ui.radio_value(&mut h.kind, PlatformKind::Ceiling, "Ceiling platform");
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "Area {:.1} sq ft, drawn dashed on \"{}\"",
                    h.area() / 144.0,
                    h.layer()
                ));
            }
        }
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        let Draft::Slab(s) = &mut self.draft else {
            return;
        };
        row(ui, "Pattern", |ui| {
            egui::ComboBox::from_id_salt("slab_pattern")
                .selected_text(s.fill_pattern.clone())
                .show_ui(ui, |ui| {
                    for p in PATTERNS {
                        ui.selectable_value(&mut s.fill_pattern, p.to_string(), p);
                    }
                });
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut s.fill_color)
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        self.draft.tabs()
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft {
            Draft::Slab(s) => {
                if s.thickness <= 0.0 {
                    return Some("The thickness must be greater than zero".into());
                }
                if let Some(f) = s.footing {
                    if f.width <= 0.0 || f.depth <= 0.0 {
                        return Some("The footing needs a width and a depth".into());
                    }
                }
                if s.layer.trim().is_empty() {
                    return Some("Pick a layer".into());
                }
            }
            Draft::Pad(p) => {
                if p.size <= 0.0 || p.thickness <= 0.0 {
                    return Some("The pad needs a size and a thickness".into());
                }
            }
            Draft::Pier(p) => {
                if p.diameter <= 0.0 || p.height <= 0.0 {
                    return Some("The pier needs a diameter and a height".into());
                }
                if let Some(f) = p.footing {
                    if f.width < p.diameter || f.depth <= 0.0 {
                        return Some("The footing must be wider than the pier".into());
                    }
                }
            }
            Draft::Hole(_) | Draft::Platform(_) => {}
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let name = self.draft.tabs()[tab].name;
        match name {
            "General" => self.general(ui),
            "Fill Style" => self.fill_style(ui),
            "Line Style" => match &mut self.draft {
                Draft::Slab(s) => line_style_page(ui, &mut s.line_style),
                Draft::Hole(h) => line_style_page(ui, &mut h.line_style),
                _ => {}
            },
            "Layer" => {
                let layers = self.layers.clone();
                match &mut self.draft {
                    Draft::Slab(s) => layer_page(ui, &layers, &mut s.layer),
                    Draft::Hole(h) => layer_page(ui, &layers, &mut h.layer),
                    Draft::Pad(p) => layer_page(ui, &layers, &mut p.layer),
                    Draft::Pier(p) => layer_page(ui, &layers, &mut p.layer),
                    Draft::Platform(_) => {}
                }
            }
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            self.draft.title().trim_end_matches(" Specification"),
            11.0,
        );
        let area = Rect::from_min_max(
            Pos2::new(rect.min.x + 10.0, rect.min.y + 26.0),
            Pos2::new(rect.max.x - 10.0, rect.max.y - 10.0),
        );
        let ink = Stroke::new(1.5_f32, PV_INK);
        match &self.draft {
            Draft::Slab(s) => outline_preview(p, area, &s.outline, PV_WALL, ink),
            Draft::Hole(h) => outline_preview(p, area, &h.outline, Color32::TRANSPARENT, ink),
            Draft::Platform(h) => {
                outline_preview(
                    p,
                    area,
                    &h.outline,
                    Color32::TRANSPARENT,
                    Stroke::new(1.5_f32, PV_ACCENT),
                );
            }
            Draft::Pad(_) => {
                let r = Rect::from_center_size(area.center(), egui::vec2(60.0, 60.0));
                p.rect_filled(r, 0.0, PV_WALL);
                p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
                p.line_segment(
                    [r.left_top(), r.right_bottom()],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
                p.line_segment(
                    [r.right_top(), r.left_bottom()],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
            Draft::Pier(pier) => {
                let c = area.center();
                if pier.footing.is_some() {
                    let r = Rect::from_center_size(c, egui::vec2(70.0, 70.0));
                    p.rect_stroke(
                        r,
                        0.0,
                        Stroke::new(1.0_f32, PV_FAINT),
                        egui::StrokeKind::Inside,
                    );
                }
                p.circle(c, 24.0, PV_WALL, ink);
                p.line_segment(
                    [c - egui::vec2(24.0, 0.0), c + egui::vec2(24.0, 0.0)],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
                p.line_segment(
                    [c - egui::vec2(0.0, 24.0), c + egui::vec2(0.0, 24.0)],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
        }
        if let Draft::Slab(s) = &self.draft {
            pv_text(
                p,
                Pos2::new(rect.center().x, rect.max.y - 4.0),
                Align2::CENTER_BOTTOM,
                format!("{} thick", fmt_ft_in(s.thickness)),
                10.0,
            );
        }
    }
}

/// Draws `outline` scaled to fit `area`.
fn outline_preview(p: &Painter, area: Rect, outline: &[Point], fill: Color32, stroke: Stroke) {
    if outline.len() < 3 {
        return;
    }
    let (lo, hi) = bounds(outline);
    let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
    let k = (f64::from(area.width()) / w).min(f64::from(area.height()) / h) as f32;
    let origin = area.center() - egui::vec2(w as f32 * k * 0.5, -(h as f32) * k * 0.5);
    let to_screen = |q: &Point| {
        Pos2::new(
            origin.x + (q.x - lo.x) as f32 * k,
            origin.y - (q.y - lo.y) as f32 * k,
        )
    };
    let pts: Vec<Pos2> = outline.iter().map(to_screen).collect();
    p.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
    if fill == Color32::TRANSPARENT {
        let mut closed = pts;
        closed.push(closed[0]);
        p.add(egui::Shape::line(closed, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::foundation::rect_outline;

    fn layer() -> FoundationLayer {
        FoundationLayer {
            slabs: vec![Slab::new(
                1,
                rect_outline(Point::ZERO, Point::new(240.0, 120.0)),
            )],
            holes: vec![SlabHole::new(
                2,
                rect_outline(Point::new(10.0, 10.0), Point::new(30.0, 30.0)),
                false,
            )],
            pads: vec![Pad::new(3, Point::new(300.0, 0.0))],
            piers: vec![Pier::new(4, Point::new(340.0, 0.0))],
            platform_holes: vec![PlatformHole::new(
                5,
                rect_outline(Point::new(50.0, 50.0), Point::new(80.0, 80.0)),
                PlatformKind::Floor,
            )],
        }
    }

    fn names() -> Vec<String> {
        vec!["Slabs".into(), "Piers/Pads".into()]
    }

    #[test]
    fn each_kind_has_its_own_pages() {
        let l = layer();
        let tabs = |r| FoundationDialog::new(&l, r, names()).unwrap().tab_names();
        assert_eq!(
            tabs(FoundationRef::Slab(1)),
            ["General", "Fill Style", "Line Style", "Layer"]
        );
        assert_eq!(
            tabs(FoundationRef::SlabHole(2)),
            ["General", "Line Style", "Layer"]
        );
        assert_eq!(tabs(FoundationRef::Pad(3)), ["General", "Layer"]);
        assert_eq!(tabs(FoundationRef::Pier(4)), ["General", "Layer"]);
        assert_eq!(tabs(FoundationRef::PlatformHole(5)), ["General"]);
        assert!(FoundationDialog::new(&l, FoundationRef::Pad(99), names()).is_none());
    }

    #[test]
    fn applying_a_draft_replaces_only_that_object() {
        let mut l = layer();
        let mut d = FoundationDialog::new(&l, FoundationRef::Slab(1), names()).unwrap();
        if let Draft::Slab(s) = d.draft_mut() {
            s.thickness = 6.0;
            s.footing = Some(Footing {
                width: 20.0,
                depth: 10.0,
            });
            s.fill_pattern = "Grid".into();
        }
        assert!(d.draft().apply(&mut l));
        let s = l.slab(1).unwrap();
        assert_eq!(s.thickness, 6.0);
        assert_eq!(s.footing.unwrap().width, 20.0);
        assert_eq!(l.pad(3).unwrap().size, 24.0);
        // The draft of a deleted object does not apply.
        l.remove(FoundationRef::Slab(1));
        assert!(!d.draft().apply(&mut l));
    }

    #[test]
    fn the_form_rejects_nonsense() {
        let l = layer();
        let mut d = FoundationDialog::new(&l, FoundationRef::Pier(4), names()).unwrap();
        assert!(d.form.error().is_none());
        if let Draft::Pier(p) = d.draft_mut() {
            p.footing = Some(Footing {
                width: 6.0,
                depth: 8.0,
            });
        }
        assert!(d.form.error().unwrap().contains("wider"));
        if let Draft::Pier(p) = d.draft_mut() {
            p.footing = None;
            p.height = 0.0;
        }
        assert!(d.form.error().is_some());

        let mut s = FoundationDialog::new(&l, FoundationRef::Slab(1), names()).unwrap();
        if let Draft::Slab(slab) = s.draft_mut() {
            slab.thickness = 0.0;
        }
        assert!(s.form.error().is_some());
    }
}
