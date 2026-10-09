//! Layout Box Specification for the boxes that show a view of the plan
//! (manual pp. 1409 to 1413): Linked View (plan views, camera views and
//! overviews), Box Scale, Layer Set, Line Style, Fill Style and Label, plus
//! the box's position. It answers with the same [`BoxSpec`] as the plain
//! specification, so `shell::layout_window::apply_spec` applies either.
//!
//! The Layout Line Specification (Edit Layout Lines) is in this file too.

use super::layout::{frame, BoxSpec, PageList};
use super::{row, section, Outcome};
use eframe::egui::{self, Ui};
use plan_core::callout::{CalloutShape, ViewKind, ViewLink};
use plan_core::fill_styles::{ColorSource, FillStyle, PatternType, SystemPattern};
use plan_core::{Id, LineStyle, Point};
use plan_docs::Scale;
use plan_layout::{
    parse_scale_text, rescale, BoxSource, CameraLink, LabelPos, LayoutBox, LineSpec, LineType,
    NewScale, ScaleMode, UpdateKind,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    General,
    LinkedView,
    BoxScale,
    LayerSet,
    LineStyle,
    FillStyle,
    Label,
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::LinkedView => "Linked View",
            Tab::BoxScale => "Box Scale",
            Tab::LayerSet => "Layer Set",
            Tab::LineStyle => "Line Style",
            Tab::FillStyle => "Fill Style",
            Tab::Label => "Label",
        }
    }
}

/// How the Box Scale panel scales the contents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScaleChoice {
    NoScale,
    Named,
    Typed,
}

/// What the dialog is told about the plan: choices for its lists and what a
/// link reports.
#[derive(Clone, Debug, Default)]
pub struct BoxSpecChoices {
    pub pages: PageList,
    pub floors: Vec<String>,
    pub layer_sets: Vec<String>,
    /// Saved Plan Views.
    pub plan_views: Vec<String>,
    /// Default Sets (Current Default Set of an unsaved plan view).
    pub default_sets: Vec<String>,
    /// The plan file the box is linked to.
    pub file_name: String,
    /// What a label's link points at: the link and what it reports (view
    /// name and the page it is on).
    pub links: Vec<(ViewLink, String)>,
}

/// Layout Box Specification of a view box.
pub struct LayoutBoxDialog {
    original: LayoutBox,
    draft: LayoutBox,
    page: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: String,
    choices: BoxSpecChoices,
    tab: Tab,
    scale_choice: ScaleChoice,
    named: Scale,
    typed: String,
    fill_kind: FillKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FillKind {
    None,
    Solid,
    Lines,
    CrossHatch,
    Dots,
}

impl FillKind {
    const ALL: [FillKind; 5] = [
        FillKind::None,
        FillKind::Solid,
        FillKind::Lines,
        FillKind::CrossHatch,
        FillKind::Dots,
    ];

    fn label(self) -> &'static str {
        match self {
            FillKind::None => "No fill",
            FillKind::Solid => "Solid",
            FillKind::Lines => "Lines",
            FillKind::CrossHatch => "Cross hatch",
            FillKind::Dots => "Dots",
        }
    }

    fn of(f: Option<&FillStyle>) -> FillKind {
        match f.map(|f| &f.pattern) {
            Some(PatternType::System(SystemPattern::Solid)) => FillKind::Solid,
            Some(PatternType::System(SystemPattern::Lines)) => FillKind::Lines,
            Some(PatternType::System(SystemPattern::CrossHatch)) => FillKind::CrossHatch,
            Some(PatternType::System(SystemPattern::Dots)) => FillKind::Dots,
            _ => FillKind::None,
        }
    }

    fn pattern(self) -> Option<SystemPattern> {
        match self {
            FillKind::None => None,
            FillKind::Solid => Some(SystemPattern::Solid),
            FillKind::Lines => Some(SystemPattern::Lines),
            FillKind::CrossHatch => Some(SystemPattern::CrossHatch),
            FillKind::Dots => Some(SystemPattern::Dots),
        }
    }
}

impl LayoutBoxDialog {
    /// Does this dialog handle the box (a view of the plan)?
    pub fn handles(b: &LayoutBox) -> bool {
        matches!(
            b.source,
            BoxSource::PlanView { .. }
                | BoxSource::Elevation { .. }
                | BoxSource::Section { .. }
                | BoxSource::Camera { .. }
                | BoxSource::CadDetail { .. }
        )
    }

    pub fn new(b: &LayoutBox, page: u32, choices: BoxSpecChoices) -> Self {
        let (a, c) = b.rect_in;
        let scale_choice = match b.view.scale_mode {
            ScaleMode::NoScale(_) => ScaleChoice::NoScale,
            ScaleMode::Custom(_) => ScaleChoice::Typed,
            ScaleMode::Named => ScaleChoice::Named,
        };
        let typed = match b.view.scale_mode {
            ScaleMode::Custom(_) => b.scale_note(),
            _ => String::new(),
        };
        Self {
            original: b.clone(),
            draft: b.clone(),
            page,
            x: a.x.min(c.x),
            y: a.y.min(c.y),
            w: (c.x - a.x).abs(),
            h: (c.y - a.y).abs(),
            label: b.label.clone().unwrap_or_default(),
            choices,
            tab: Tab::LinkedView,
            scale_choice,
            named: b.scale,
            typed,
            fill_kind: FillKind::of(b.view.fill.as_ref()),
        }
    }

    /// Opens on another panel (tests and the Layout Box Layers tool).
    pub fn on_tab(mut self, name: &str) -> Self {
        for t in [
            Tab::General,
            Tab::LinkedView,
            Tab::BoxScale,
            Tab::LayerSet,
            Tab::LineStyle,
            Tab::FillStyle,
            Tab::Label,
        ] {
            if t.name() == name {
                self.tab = t;
            }
        }
        self
    }

    fn typed_ipf(&self) -> Option<f64> {
        parse_scale_text(&self.typed)
    }

    /// The scale change the Box Scale panel asks for, if it differs from the
    /// box's now.
    fn requested_scale(&self) -> Option<NewScale> {
        let o = &self.original;
        match self.scale_choice {
            ScaleChoice::NoScale => (!o.is_no_scale()).then_some(NewScale::NoScale),
            ScaleChoice::Named => (o.view.scale_mode != ScaleMode::Named || o.scale != self.named)
                .then_some(NewScale::Named(self.named)),
            ScaleChoice::Typed => self
                .typed_ipf()
                .filter(|v| o.is_no_scale() || (o.effective_ipf() - v).abs() > 1e-9)
                .map(NewScale::PerFoot),
        }
    }

    /// The box with the dialog's fields written back.
    pub fn result(&self) -> BoxSpec {
        let mut b = self.draft.clone();
        if let Some(new) = self.requested_scale() {
            let mut o = self.original.clone();
            o.view.resize_with_scale = self.draft.view.resize_with_scale;
            rescale(&mut o, new);
            b.scale = o.scale;
            b.view.scale_mode = o.view.scale_mode;
            b.view.pan_in = o.view.pan_in;
            b.rect_in = o.rect_in;
        }
        let (a, c) = self.original.rect_in;
        let typed_rect = (self.x, self.y, self.w, self.h)
            != (
                a.x.min(c.x),
                a.y.min(c.y),
                (c.x - a.x).abs(),
                (c.y - a.y).abs(),
            );
        if typed_rect {
            b.rect_in = (
                Point::new(self.x, self.y),
                Point::new(self.x + self.w, self.y + self.h),
            );
        }
        b.label = (!self.label.trim().is_empty()).then(|| self.label.clone());
        b.view.fill = match self.fill_kind.pattern() {
            None => None,
            Some(p) => {
                let mut f = self.draft.view.fill.clone().unwrap_or_default();
                f.pattern = PatternType::System(p);
                if matches!(f.color, ColorSource::Layer) {
                    f.color = ColorSource::Single([0, 0, 0]);
                }
                Some(f)
            }
        };
        BoxSpec {
            layout_box: b,
            page: self.page,
        }
    }

    fn error(&self) -> Option<&'static str> {
        if self.w < 0.05 || self.h < 0.05 {
            Some("The box must be at least 0.05\" square")
        } else if self.scale_choice == ScaleChoice::Typed && self.typed_ipf().is_none() {
            Some("Type a scale such as 1:240, 1/8\" = 1' or 1 in = 80 ft")
        } else {
            None
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = self.error();
        frame(ctx, "Layout Box Specification", 470.0, error, |ui| {
            ui.horizontal_wrapped(|ui| {
                for t in [
                    Tab::General,
                    Tab::LinkedView,
                    Tab::BoxScale,
                    Tab::LayerSet,
                    Tab::LineStyle,
                    Tab::FillStyle,
                    Tab::Label,
                ] {
                    if t == Tab::LayerSet && !self.has_layer_set() {
                        continue;
                    }
                    ui.selectable_value(&mut self.tab, t, t.name());
                }
            });
            ui.separator();
            match self.tab {
                Tab::General => self.general(ui),
                Tab::LinkedView => self.linked_view(ui),
                Tab::BoxScale => self.box_scale(ui),
                Tab::LayerSet => self.layer_set(ui),
                Tab::LineStyle => self.line_style(ui),
                Tab::FillStyle => self.fill_style(ui),
                Tab::Label => self.label_panel(ui),
            }
        })
    }

    /// The Layer Set panel is for a linked view that is not saved.
    fn has_layer_set(&self) -> bool {
        matches!(self.draft.source, BoxSource::PlanView { .. })
            && self.draft.view.saved_view.is_none()
    }

    fn general(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        row(ui, "Update kind", |ui| ui.label(b.update_kind().label()));
        section(ui, "Position and size");
        row(ui, "Left / Bottom", |ui| {
            inches(ui, &mut self.x);
            inches(ui, &mut self.y);
        });
        row(ui, "Width / Height", |ui| {
            inches(ui, &mut self.w);
            inches(ui, &mut self.h);
        });
        row(ui, "Rotation", |ui| {
            let mut turns = b.quarter_turns();
            egui::ComboBox::from_id_salt("lbx_rotation")
                .selected_text(format!("{}\u{b0}", u32::from(turns) * 90))
                .show_ui(ui, |ui| {
                    for t in 0..4u8 {
                        ui.selectable_value(&mut turns, t, format!("{}\u{b0}", u32::from(t) * 90));
                    }
                });
            b.rotation_deg = f64::from(turns) * 90.0;
        });
        ui.checkbox(&mut b.clip, "Clip content to the box");
        if b.view.pan_in != (0.0, 0.0) && ui.button("Recenter contents").clicked() {
            b.view.pan_in = (0.0, 0.0);
        }
    }

    fn linked_view(&mut self, ui: &mut Ui) {
        let kind_name = match &self.draft.source {
            BoxSource::PlanView { .. } if self.draft.view.saved_view.is_some() => "Saved plan view",
            BoxSource::PlanView { .. } => "Floor level",
            BoxSource::Elevation { .. } | BoxSource::Section { .. } => "Cross section/elevation",
            BoxSource::Camera { .. } => "Camera view",
            BoxSource::CadDetail { .. } => "CAD Detail",
            _ => "",
        };
        section(ui, "Linked view");
        row(ui, "File name", |ui| {
            ui.label(self.choices.file_name.as_str())
        });
        row(ui, "View name", |ui| {
            let name = match (&self.draft.source, &self.draft.view.saved_view) {
                (BoxSource::PlanView { .. }, Some(n)) => n.clone(),
                (BoxSource::PlanView { floor, .. }, None) => {
                    self.choices.floors.get(*floor).cloned().unwrap_or_default()
                }
                (BoxSource::CadDetail { name, .. }, _) => name.clone(),
                (BoxSource::Elevation { dir }, _) => format!("{dir:?} elevation"),
                (BoxSource::Section { .. }, _) => "Cross section".to_string(),
                (BoxSource::Camera { camera_id }, _) => format!("Camera {camera_id}"),
                _ => String::new(),
            };
            ui.label(name)
        });
        row(ui, "View type", |ui| ui.label(kind_name));
        if self.draft.view.ignore_missing {
            ui.label("Invalid links are ignored for this box.");
        }
        ui.checkbox(
            &mut self.draft.view.ignore_missing,
            "Ignore Invalid Links (no caution symbol)",
        );
        match self.draft.source.clone() {
            BoxSource::PlanView { floor, .. } => self.plan_options(ui, floor),
            BoxSource::Elevation { .. } | BoxSource::Section { .. } | BoxSource::Camera { .. } => {
                self.camera_options(ui)
            }
            _ => {}
        }
    }

    fn plan_options(&mut self, ui: &mut Ui, floor: usize) {
        let b = &mut self.draft;
        section(ui, "Options");
        row(ui, "Saved Plan View", |ui| {
            let shown = b.view.saved_view.clone().unwrap_or_else(|| "None".into());
            egui::ComboBox::from_id_salt("lbx_saved_view")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut b.view.saved_view, None, "None");
                    for n in &self.choices.plan_views {
                        ui.selectable_value(&mut b.view.saved_view, Some(n.clone()), n);
                    }
                });
        });
        if b.view.saved_view.is_some() {
            row(ui, "Current floor", |ui| {
                ui.label(self.choices.floors.get(floor).cloned().unwrap_or_default())
            });
            ui.weak("The box follows the saved view's floor and layer set.");
        } else {
            row(ui, "Current floor", |ui| {
                let mut f = floor;
                egui::ComboBox::from_id_salt("lbx_floor")
                    .selected_text(self.choices.floors.get(floor).cloned().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (i, n) in self.choices.floors.iter().enumerate() {
                            ui.selectable_value(&mut f, i, n);
                        }
                    });
                if let BoxSource::PlanView { floor: fl, .. } = &mut b.source {
                    *fl = f;
                }
            });
            row(ui, "Current default set", |ui| {
                egui::ComboBox::from_id_salt("lbx_default_set")
                    .selected_text(if b.view.default_set.is_empty() {
                        "Current".to_string()
                    } else {
                        b.view.default_set.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut b.view.default_set, String::new(), "Current");
                        for n in &self.choices.default_sets {
                            ui.selectable_value(&mut b.view.default_set, n.clone(), n);
                        }
                    });
            });
            ui.checkbox(&mut b.view.show_color, "Show Color");
        }
        ui.checkbox(&mut b.view.poche, "Poch\u{e9}");
        ui.checkbox(&mut b.view.fill_window, "Entire plan (Fill Window extent)");
        if b.view.extent.is_some() && ui.button("Show the entire view").clicked() {
            b.view.extent = None;
        }
    }

    fn camera_options(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        section(ui, "Camera view options");
        ui.radio_value(
            &mut b.view.camera,
            CameraLink::Always,
            CameraLink::Always.label(),
        );
        ui.radio_value(
            &mut b.view.camera,
            CameraLink::OnDemand,
            CameraLink::OnDemand.label(),
        );
        ui.radio_value(
            &mut b.view.camera,
            CameraLink::PlotLines,
            CameraLink::PlotLines.label(),
        );
        let plot = b.view.camera == CameraLink::PlotLines;
        ui.add_enabled_ui(plot, |ui| {
            ui.checkbox(&mut b.view.plot.color_fill, "Color Fill");
            section(ui, "Edge line defaults");
            ui.checkbox(&mut b.view.plot.use_edge_defaults, "Use Edge Line Defaults");
            ui.add_enabled_ui(b.view.plot.use_edge_defaults, |ui| {
                row(ui, "Line weight", |ui| {
                    weight_field(ui, &mut b.view.plot.edge_weight_pt);
                });
                row(ui, "Line color", |ui| {
                    ui.color_edit_button_srgb(&mut b.view.plot.edge_color);
                });
            });
            section(ui, "Pattern line defaults");
            ui.checkbox(
                &mut b.view.plot.use_pattern_defaults,
                "Use Pattern Line Defaults",
            );
            ui.add_enabled_ui(b.view.plot.use_pattern_defaults, |ui| {
                row(ui, "Line weight", |ui| {
                    weight_field(ui, &mut b.view.plot.pattern_weight_pt);
                });
                row(ui, "Line color", |ui| {
                    ui.color_edit_button_srgb(&mut b.view.plot.pattern_color);
                });
            });
        });
        if let Some(art) = &b.view.art {
            let (e, p) = art.counts();
            ui.weak(format!("{e} edge lines and {p} pattern lines are kept."));
        }
        match b.update_kind() {
            UpdateKind::Dynamic => {
                ui.weak("Updates any time the model changes.");
            }
            UpdateKind::SemiDynamic => {
                ui.weak(
                    "Updates when asked and when its page prints. It can lose some quality as you zoom and pan on the page; update the view to restore it.",
                );
            }
            _ => {}
        }
    }

    fn box_scale(&mut self, ui: &mut Ui) {
        if !matches!(
            self.draft.source,
            BoxSource::PlanView { .. }
                | BoxSource::Elevation { .. }
                | BoxSource::Section { .. }
                | BoxSource::Camera { .. }
                | BoxSource::CadDetail { .. }
        ) {
            ui.label("This box has no scale.");
            return;
        }
        section(ui, "Box scale");
        ui.radio_value(&mut self.scale_choice, ScaleChoice::NoScale, "No Scale");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.scale_choice, ScaleChoice::Named, "Scale");
            egui::ComboBox::from_id_salt("lbx_scale")
                .selected_text(self.named.label())
                .show_ui(ui, |ui| {
                    for s in Scale::choices() {
                        if ui.selectable_value(&mut self.named, s, s.label()).clicked() {
                            self.scale_choice = ScaleChoice::Named;
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.scale_choice, ScaleChoice::Typed, "Other");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut self.typed)
                        .hint_text("1:240 or 1/8\" = 1'")
                        .desired_width(150.0),
                )
                .changed()
            {
                self.scale_choice = ScaleChoice::Typed;
            }
        });
        ui.checkbox(
            &mut self.draft.view.layout_line_scaling,
            "Use Layout Line Scaling",
        );
        ui.checkbox(
            &mut self.draft.view.resize_with_scale,
            "Scale Layout Box Contents Only (the box resizes with the scale)",
        );
        ui.weak(format!(
            "Now: {} ({})",
            self.original.scale_note(),
            if self.original.is_no_scale() {
                "no scale"
            } else {
                "scaled"
            }
        ));
    }

    fn layer_set(&mut self, ui: &mut Ui) {
        section(ui, "Active layer set");
        if let BoxSource::PlanView { layer_set, .. } = &mut self.draft.source {
            row(ui, "Layer set", |ui| {
                egui::ComboBox::from_id_salt("lbx_layer_set")
                    .selected_text(layer_set.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(layer_set, "All".to_string(), "All");
                        for n in &self.choices.layer_sets {
                            ui.selectable_value(layer_set, n.clone(), n);
                        }
                    });
            });
            ui.weak("\"All\" ignores layer visibility. Changes to a layer set affect every view that uses it.");
        }
    }

    fn line_style(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        section(ui, "Border");
        ui.checkbox(&mut b.border, "Draw border");
        let mut own_color = b.view.border.color.is_some();
        let mut own_weight = b.view.border.weight_pt.is_some();
        let mut own_dash = b.view.border.dash.is_some();
        row(ui, "Color", |ui| {
            ui.checkbox(&mut own_color, "Own color");
            let mut c = b.view.border.color.unwrap_or([0, 0, 0]);
            ui.add_enabled_ui(own_color, |ui| ui.color_edit_button_srgb(&mut c));
            b.view.border.color = own_color.then_some(c);
        });
        row(ui, "Line weight", |ui| {
            ui.checkbox(&mut own_weight, "Own weight");
            let mut w = b.view.border.weight_pt.unwrap_or(0.75);
            ui.add_enabled_ui(own_weight, |ui| weight_field(ui, &mut w));
            b.view.border.weight_pt = own_weight.then_some(w);
        });
        row(ui, "Line style", |ui| {
            ui.checkbox(&mut own_dash, "Own style");
            let mut d = b.view.border.dash.unwrap_or_default();
            ui.add_enabled_ui(own_dash, |ui| style_combo(ui, "lbx_border_dash", &mut d));
            b.view.border.dash = own_dash.then_some(d);
        });
        ui.weak("Otherwise the Layout Box Borders layer sets the border.");
        section(ui, "Contents");
        row(ui, "Line weight scaling", |ui| {
            ui.add(
                egui::DragValue::new(&mut b.line_weight_scale)
                    .speed(0.01)
                    .range(0.1..=5.0)
                    .max_decimals(2)
                    .suffix(" x"),
            );
        });
        ui.checkbox(&mut b.hatch_materials, "Material hatches (elevations)");
        section(ui, "Page");
        row(ui, "Page", |ui| {
            let shown = self
                .choices
                .pages
                .iter()
                .find(|p| p.0 == self.page)
                .map_or_else(|| format!("A-{}", self.page), |p| p.1.clone());
            egui::ComboBox::from_id_salt("lbx_page")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for p in &self.choices.pages {
                        ui.selectable_value(&mut self.page, p.0, p.1.clone());
                    }
                });
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill style");
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("lbx_fill_kind")
                .selected_text(self.fill_kind.label())
                .show_ui(ui, |ui| {
                    for k in FillKind::ALL {
                        ui.selectable_value(&mut self.fill_kind, k, k.label());
                    }
                });
        });
        if self.fill_kind != FillKind::None {
            let f = self.draft.view.fill.get_or_insert_with(FillStyle::default);
            let mut rgb = match f.color {
                ColorSource::Single(c) => c,
                _ => [0, 0, 0],
            };
            row(ui, "Color", |ui| ui.color_edit_button_srgb(&mut rgb));
            f.color = ColorSource::Single(rgb);
            row(ui, "Transparency", |ui| {
                ui.add(egui::Slider::new(&mut f.transparency, 0.0..=1.0));
            });
            if self.fill_kind != FillKind::Solid {
                row(ui, "Spacing", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut f.width)
                            .speed(0.05)
                            .range(0.02..=100.0)
                            .suffix("\""),
                    );
                    f.height = f.width;
                });
                row(ui, "Angle", |ui| {
                    ui.add(egui::DragValue::new(&mut f.angle_deg).suffix("\u{b0}"));
                });
            }
        }
        ui.weak("A fill shows only while the Layout Box Borders layer is on.");
    }

    fn label_panel(&mut self, ui: &mut Ui) {
        let b = &mut self.draft;
        section(ui, "Label");
        row(ui, "Text", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.label).desired_width(260.0));
        });
        ui.weak("Macros: %scale% %view_name% %view_type% %layout_page_label% %linked_view_name% %linked_view_layout_page_label%");
        ui.checkbox(&mut b.view.label.show_scale, "Show the scale note");
        row(ui, "Position", |ui| {
            egui::ComboBox::from_id_salt("lbx_label_pos")
                .selected_text(b.view.label.position.label())
                .show_ui(ui, |ui| {
                    for p in LabelPos::ALL {
                        ui.selectable_value(&mut b.view.label.position, p, p.label());
                    }
                });
        });
        row(ui, "Shape", |ui| {
            egui::ComboBox::from_id_salt("lbx_label_shape")
                .selected_text(b.view.label.shape.label())
                .show_ui(ui, |ui| {
                    for s in CalloutShape::ALL {
                        ui.selectable_value(&mut b.view.label.shape, s, s.label());
                    }
                });
        });
        if b.view.label.shape != CalloutShape::None {
            row(ui, "Callout text", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut b.view.label.callout_text).desired_width(220.0),
                );
            });
        }
        section(ui, "Link");
        let current = b.view.label.link.clone();
        let shown = current.as_ref().map_or("None".to_string(), |l| {
            format!("{}: {}", l.kind.label(), link_name(l, &self.choices))
        });
        row(ui, "Linked to", |ui| {
            egui::ComboBox::from_id_salt("lbx_label_link")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut b.view.label.link, None, "None");
                    for (l, text) in &self.choices.links {
                        ui.selectable_value(
                            &mut b.view.label.link,
                            Some(l.clone()),
                            format!("{}: {text}", l.kind.label()),
                        );
                    }
                });
        });
        if let Some(l) = &b.view.label.link {
            if let Some((_, report)) = self.choices.links.iter().find(|(c, _)| c == l) {
                ui.weak(report.as_str());
            }
        }
    }
}

fn link_name(l: &ViewLink, c: &BoxSpecChoices) -> String {
    c.links
        .iter()
        .find(|(k, _)| k == l)
        .map_or_else(|| l.name.clone(), |(_, t)| t.clone())
}

fn inches(ui: &mut Ui, v: &mut f64) {
    ui.add(
        egui::DragValue::new(v)
            .speed(0.05)
            .max_decimals(3)
            .suffix("\""),
    );
}

fn weight_field(ui: &mut Ui, v: &mut f64) {
    ui.add(
        egui::DragValue::new(v)
            .speed(0.01)
            .range(0.01..=12.0)
            .max_decimals(2)
            .suffix(" pt"),
    );
}

fn style_combo(ui: &mut Ui, salt: &str, v: &mut LineStyle) {
    let name = |s: LineStyle| match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-dot",
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(name(*v))
        .show_ui(ui, |ui| {
            for s in [
                LineStyle::Solid,
                LineStyle::Dashed,
                LineStyle::Dotted,
                LineStyle::DashDot,
            ] {
                ui.selectable_value(v, s, name(s));
            }
        });
}

/// Page, id and name of the label links a plan offers: every camera, CAD
/// detail and layout page, with what each reports.
pub fn link_choices(
    cameras: &[(Id, String)],
    details: &[String],
    pages: &[(u32, String)],
) -> Vec<(ViewLink, String)> {
    let mut out = Vec::new();
    for (id, name) in cameras {
        out.push((
            ViewLink {
                kind: ViewKind::Camera,
                id: *id,
                name: name.clone(),
            },
            name.clone(),
        ));
    }
    for name in details {
        out.push((
            ViewLink {
                kind: ViewKind::CadDetail,
                id: 0,
                name: name.clone(),
            },
            name.clone(),
        ));
    }
    for (n, title) in pages {
        out.push((
            ViewLink {
                kind: ViewKind::LayoutPage,
                id: Id::from(*n),
                name: title.clone(),
            },
            title.clone(),
        ));
    }
    out
}

// ------------------------------------------------ Layout Line Specification --

/// The Layout Line Specification: Line Type, Line Weight, Line Style and
/// Line Color of the selected plot lines, each with Use Default (manual
/// p. 1408).
pub struct LayoutLineDialog {
    count: usize,
    /// `None` is "No Change" (several lines of different kinds).
    kind: Option<LineType>,
    kind_changed: bool,
    weight: f64,
    weight_default: bool,
    weight_touched: bool,
    style: LineStyle,
    style_default: bool,
    style_touched: bool,
    color: [u8; 3],
    color_default: bool,
    color_touched: bool,
}

impl LayoutLineDialog {
    /// `lines` are the selected lines with the pen they draw with now.
    pub fn new(lines: &[(plan_layout::PlotLine, plan_layout::PlotPen)]) -> Self {
        let first = lines.first();
        let same_kind = lines.windows(2).all(|w| w[0].0.kind == w[1].0.kind);
        Self {
            count: lines.len(),
            kind: if same_kind {
                first.map(|l| l.0.kind)
            } else {
                None
            },
            kind_changed: false,
            weight: first.map_or(0.35, |l| l.1.weight_pt),
            weight_default: first.is_some_and(|l| l.0.weight_pt.is_none()),
            weight_touched: false,
            style: first.map_or(LineStyle::Solid, |l| l.1.style),
            style_default: first.is_some_and(|l| l.0.style.is_none()),
            style_touched: false,
            color: first.map_or([0, 0, 0], |l| l.1.color),
            color_default: first.is_some_and(|l| l.0.color.is_none()),
            color_touched: false,
        }
    }

    /// What the dialog changes.
    pub fn spec(&self) -> LineSpec {
        LineSpec {
            kind: self.kind.filter(|_| self.kind_changed),
            weight_pt: self
                .weight_touched
                .then(|| (!self.weight_default).then_some(self.weight)),
            style: self
                .style_touched
                .then(|| (!self.style_default).then_some(self.style)),
            color: self
                .color_touched
                .then(|| (!self.color_default).then_some(self.color)),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let n = self.count;
        frame(ctx, "Layout Line Specification", 380.0, None, |ui| {
            ui.label(format!("{n} line(s) selected"));
            section(ui, "Line type");
            ui.horizontal(|ui| {
                for k in [LineType::Edge, LineType::Pattern] {
                    if ui.radio(self.kind == Some(k), k.label()).clicked() {
                        self.kind = Some(k);
                        self.kind_changed = true;
                    }
                }
            });
            if self.kind.is_none() {
                ui.weak("No change: the lines are of different types.");
            }
            section(ui, "Line weight");
            row(ui, "Weight", |ui| {
                if ui
                    .add_enabled(
                        !self.weight_default,
                        egui::DragValue::new(&mut self.weight)
                            .speed(0.01)
                            .range(0.01..=12.0)
                            .max_decimals(2)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    self.weight_touched = true;
                }
                if ui
                    .checkbox(&mut self.weight_default, "Use Default Weight")
                    .changed()
                {
                    self.weight_touched = true;
                }
            });
            section(ui, "Line style");
            row(ui, "Style", |ui| {
                ui.add_enabled_ui(!self.style_default, |ui| {
                    let before = self.style;
                    style_combo(ui, "lld_style", &mut self.style);
                    if before != self.style {
                        self.style_touched = true;
                    }
                });
                if ui
                    .checkbox(&mut self.style_default, "Use Default Style")
                    .changed()
                {
                    self.style_touched = true;
                }
            });
            section(ui, "Line color");
            row(ui, "Color", |ui| {
                ui.add_enabled_ui(!self.color_default, |ui| {
                    if ui.color_edit_button_srgb(&mut self.color).changed() {
                        self.color_touched = true;
                    }
                });
                if ui
                    .checkbox(&mut self.color_default, "Use Default Color")
                    .changed()
                {
                    self.color_touched = true;
                }
            });
        })
    }
}

// ------------------------------------------------------------ Change Scale --

/// Rescale Layout View: the Change Scale dialog (manual p. 1407).
pub struct ChangeScaleDialog {
    original: LayoutBox,
    choice: ScaleChoice,
    named: Scale,
    typed: String,
    line_scaling: bool,
}

impl ChangeScaleDialog {
    pub fn new(b: &LayoutBox) -> Self {
        Self {
            original: b.clone(),
            choice: if b.is_no_scale() {
                ScaleChoice::NoScale
            } else if b.view.scale_mode == ScaleMode::Named {
                ScaleChoice::Named
            } else {
                ScaleChoice::Typed
            },
            named: b.scale,
            typed: match b.view.scale_mode {
                ScaleMode::Custom(_) => b.scale_note(),
                _ => String::new(),
            },
            line_scaling: b.view.layout_line_scaling,
        }
    }

    fn typed_ipf(&self) -> Option<f64> {
        parse_scale_text(&self.typed)
    }

    /// The scale asked for and Use Layout Line Scaling.
    pub fn result(&self) -> Option<(NewScale, bool)> {
        let new = match self.choice {
            ScaleChoice::NoScale => NewScale::NoScale,
            ScaleChoice::Named => NewScale::Named(self.named),
            ScaleChoice::Typed => NewScale::PerFoot(self.typed_ipf()?),
        };
        Some((new, self.line_scaling))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let error = (self.choice == ScaleChoice::Typed && self.typed_ipf().is_none())
            .then_some("Type a scale such as 1:240, 1/8\" = 1' or 1 in = 80 ft");
        frame(ctx, "Change Scale", 360.0, error, |ui| {
            ui.radio_value(&mut self.choice, ScaleChoice::NoScale, "No Scale");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.choice, ScaleChoice::Named, "Scale");
                egui::ComboBox::from_id_salt("chs_scale")
                    .selected_text(self.named.label())
                    .show_ui(ui, |ui| {
                        for s in Scale::choices() {
                            if ui.selectable_value(&mut self.named, s, s.label()).clicked() {
                                self.choice = ScaleChoice::Named;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.choice, ScaleChoice::Typed, "Other");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.typed)
                            .hint_text("1:240 or 1/8\" = 1'")
                            .desired_width(150.0),
                    )
                    .changed()
                {
                    self.choice = ScaleChoice::Typed;
                }
            });
            ui.checkbox(&mut self.line_scaling, "Use Layout Line Scaling");
            ui.weak(format!("Now: {}", self.original.scale_note()));
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_layout::{BoxView, PlotLine, PlotPen, Tier};

    fn plan_box() -> LayoutBox {
        LayoutBox::new(
            4,
            (Point::new(1.0, 1.0), Point::new(7.0, 5.0)),
            BoxSource::PlanView {
                floor: 0,
                layer_set: "Floor Plan".into(),
            },
            Scale::EighthInch,
        )
    }

    fn choices() -> BoxSpecChoices {
        BoxSpecChoices {
            pages: vec![(1, "A-1  Plans".into()), (2, "A-2  Sections".into())],
            floors: vec!["1st Floor".into()],
            layer_sets: vec!["Floor Plan".into()],
            plan_views: vec!["Main Plan".into()],
            file_name: "House (Layout)".into(),
            links: link_choices(
                &[(9, "Section A".into())],
                &["Stair Detail".into()],
                &[(2, "A-2  Sections".into())],
            ),
            ..BoxSpecChoices::default()
        }
    }

    #[test]
    fn only_boxes_that_show_a_view_use_this_dialog() {
        assert!(LayoutBoxDialog::handles(&plan_box()));
        let mut t = plan_box();
        t.source = BoxSource::text("x", 12.0);
        assert!(!LayoutBoxDialog::handles(&t));
        t.source = BoxSource::Perspective { camera_id: 1 };
        assert!(!LayoutBoxDialog::handles(&t));
    }

    #[test]
    fn nothing_changed_is_nothing_written() {
        let b = plan_box();
        let d = LayoutBoxDialog::new(&b, 1, choices());
        assert_eq!(d.result().layout_box, b);
        assert_eq!(d.result().page, 1);
    }

    #[test]
    fn a_new_scale_resizes_the_box_unless_only_the_contents_are_scaled_off() {
        let b = plan_box();
        let mut d = LayoutBoxDialog::new(&b, 1, choices());
        d.scale_choice = ScaleChoice::Named;
        d.named = Scale::QuarterInch;
        let r = d.result().layout_box;
        assert_eq!(r.scale, Scale::QuarterInch);
        assert!((r.size_in().0 / b.size_in().0 - 2.0).abs() < 1e-9);
        // Scale Layout Box Contents Only unchecked: the box keeps its size.
        d.draft.view.resize_with_scale = false;
        let r = d.result().layout_box;
        assert_eq!(r.scale, Scale::QuarterInch);
        assert_eq!(r.size_in(), b.size_in());
        // A typed scale on no list, then No Scale.
        d.scale_choice = ScaleChoice::Typed;
        d.typed = "1:37".into();
        assert_eq!(d.result().layout_box.scale_note(), "1:37");
        d.typed = "nonsense".into();
        assert!(d.error().is_some());
        d.scale_choice = ScaleChoice::NoScale;
        assert_eq!(d.error(), None);
        let r = d.result().layout_box;
        assert!(r.is_no_scale());
        assert!(
            (r.effective_ipf() - 0.125).abs() < 1e-12,
            "same size on the sheet"
        );
        // Typed position and size win over the rescale.
        d.x = 2.0;
        assert_eq!(d.result().layout_box.rect_in.0.x, 2.0);
    }

    #[test]
    fn linked_view_label_and_fill_panels_write_the_box_view() {
        let b = plan_box();
        let mut d = LayoutBoxDialog::new(&b, 1, choices());
        d.draft.view.saved_view = Some("Main Plan".into());
        d.draft.view.poche = true;
        d.draft.view.plot.color_fill = true;
        d.label = "%view_name% at %scale%".into();
        d.draft.view.label.shape = plan_core::callout::CalloutShape::Circle;
        d.draft.view.label.link = Some(d.choices.links[2].0.clone());
        d.fill_kind = FillKind::Lines;
        d.draft.view.border.weight_pt = Some(2.0);
        let r = d.result().layout_box;
        assert_eq!(r.view.saved_view.as_deref(), Some("Main Plan"));
        assert!(r.view.poche && r.view.plot.color_fill);
        assert_eq!(r.label.as_deref(), Some("%view_name% at %scale%"));
        assert_eq!(
            r.view.label.link.as_ref().unwrap().kind,
            ViewKind::LayoutPage
        );
        assert_eq!(r.view.border.weight_pt, Some(2.0));
        let fill = r.view.fill.unwrap();
        assert_eq!(fill.pattern, PatternType::System(SystemPattern::Lines));
        // No fill again removes it.
        d.fill_kind = FillKind::None;
        assert_eq!(d.result().layout_box.view.fill, None);
        let _ = BoxView::default();
    }

    #[test]
    fn the_layer_set_panel_is_for_unsaved_plan_views_only() {
        let mut d = LayoutBoxDialog::new(&plan_box(), 1, choices());
        assert!(d.has_layer_set());
        d.draft.view.saved_view = Some("Main Plan".into());
        assert!(!d.has_layer_set());
    }

    #[test]
    fn the_dialogs_draw_on_every_panel() {
        let ctx = egui::Context::default();
        let mut cam = plan_box();
        cam.source = BoxSource::Elevation {
            dir: plan_elevation::ViewDir::Front,
        };
        cam.view.camera = CameraLink::PlotLines;
        for b in [plan_box(), cam] {
            let mut d = LayoutBoxDialog::new(&b, 1, choices());
            for tab in [
                "General",
                "Linked View",
                "Box Scale",
                "Layer Set",
                "Line Style",
                "Fill Style",
                "Label",
            ] {
                d.tab = Tab::General;
                let mut d2 = LayoutBoxDialog::new(&b, 1, choices()).on_tab(tab);
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    d2.show(ctx);
                    d.show(ctx);
                });
            }
        }
        let mut c = ChangeScaleDialog::new(&plan_box());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            c.show(ctx);
        });
        assert_eq!(c.result(), Some((NewScale::Named(Scale::EighthInch), true)));
    }

    #[test]
    fn the_line_specification_changes_only_what_was_touched() {
        let line = |kind, weight: Option<f64>| PlotLine {
            id: 1,
            a: Point::ZERO,
            b: Point::new(1.0, 0.0),
            kind,
            tier: Tier::Medium,
            base_color: None,
            weight_pt: weight,
            style: None,
            color: None,
            added: false,
        };
        let pen = PlotPen {
            weight_pt: 0.35,
            color: [0, 0, 0],
            style: LineStyle::Solid,
        };
        let mut d = LayoutLineDialog::new(&[(line(LineType::Edge, None), pen)]);
        assert_eq!(d.spec(), LineSpec::default(), "No Change");
        d.weight_default = false;
        d.weight = 1.2;
        d.weight_touched = true;
        d.kind_changed = true;
        d.kind = Some(LineType::Pattern);
        let s = d.spec();
        assert_eq!(s.weight_pt, Some(Some(1.2)));
        assert_eq!(s.kind, Some(LineType::Pattern));
        assert_eq!(s.color, None);
        // "Use Default Weight" gives the view's default back.
        d.weight_default = true;
        assert_eq!(d.spec().weight_pt, Some(None));
        // Lines of different types leave the type at No Change.
        let d = LayoutLineDialog::new(&[
            (line(LineType::Edge, None), pen),
            (line(LineType::Pattern, Some(0.5)), pen),
        ]);
        assert_eq!(d.kind, None);
        assert_eq!(d.spec().kind, None);
    }
}
