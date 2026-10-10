//! Send to Layout with all of Chief's choices (manual pp. 1398 to 1401): the
//! source view, the layout file and page, the send position, Send Options
//! (Entire Plan/View, Current Screen, Current Screen As Image, Link Saved Plan
//! View), Camera View Options (Live View with Update on Demand or Always
//! Update, Plot Lines with Color Fill and the line defaults), Scaling (the
//! lists including the site scales, a typed ratio, Fit to Sheet), Use Layout
//! Line Scaling, the "too big for the sheet" warning and Send All Remaining
//! Views.
//!
//! The dialog edits a draft; `shell::layout_window` turns the answers into a
//! [`plan_layout::SendRequest`].

use super::layout::{
    frame, LayoutTarget, PageChoice, PageList, Placement, SendSource, SendSpec, SnapshotSpec,
};
use super::{row, section, Outcome};
use eframe::egui::{self, RichText};
use plan_docs::Scale;
use plan_layout::{parse_scale_text, CameraLink, SendExtent, SendOptions, SendScale};
use std::hash::{Hash, Hasher};

/// How the Scaling panel picks the scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleChoice {
    /// The largest scale that fits the page (up to 1/4").
    Largest,
    /// A scale of the lists.
    Named,
    /// A ratio or scale typed in.
    Typed,
    /// Fit to Sheet (No Scale).
    FitToSheet,
}

/// What Send to Layout does beyond the view, the page and the scale of the
/// base [`SendSpec`].
#[derive(Clone, Debug, PartialEq)]
pub struct SendDetails {
    pub scale: SendScale,
    pub options: SendOptions,
    /// Snap to Active CAD Point.
    pub snap_to_point: bool,
    /// Show Layout Page: go to the page when the view is sent.
    pub show_page: bool,
    /// Send All Remaining Views to Layout With These Settings.
    pub all_remaining: bool,
    /// Current Screen As Image: embed a picture of the view instead of
    /// linking to it.
    pub as_image: bool,
}

/// The dialog's answers.
#[derive(Clone, Debug, PartialEq)]
pub struct SendAnswers {
    pub spec: SendSpec,
    pub details: SendDetails,
    pub target: LayoutTarget,
    pub snapshot: Option<SnapshotSpec>,
}

/// The extent radio buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExtentChoice {
    Entire,
    Screen,
    AsImage,
}

/// Send to Layout (L-2, L-155, L-156, L-229).
pub struct SendToLayoutDialog {
    spec: SendSpec,
    pages: PageList,
    floors: Vec<String>,
    layer_sets: Vec<String>,
    layouts: Vec<String>,
    target: LayoutTarget,
    snapshot_offered: bool,
    snapshot: Option<SnapshotSpec>,
    /// The Saved Plan View in use (`None`: an unsaved plan view).
    saved_view: Option<String>,
    link_saved_view: bool,
    /// The part of the plan on screen, if the application can tell.
    screen: Option<[f64; 4]>,
    extent: ExtentChoice,
    camera: CameraLink,
    color_fill: bool,
    use_edge_defaults: bool,
    use_pattern_defaults: bool,
    scale_choice: ScaleChoice,
    named: Scale,
    typed: String,
    layout_line_scaling: bool,
    snap_to_point: bool,
    show_page: bool,
    all_remaining: bool,
    /// Views waiting behind this one (selected together).
    remaining: usize,
    /// The warning for the last answers, and the answers it is for.
    warning: Option<String>,
    warn_key: Option<u64>,
}

impl SendToLayoutDialog {
    pub fn new(
        source: SendSource,
        pages: PageList,
        current_page: Option<u32>,
        floors: Vec<String>,
        layer_sets: Vec<String>,
    ) -> Self {
        let page = current_page
            .filter(|n| pages.iter().any(|p| p.0 == *n))
            .or_else(|| pages.first().map(|p| p.0))
            .map_or(PageChoice::New, PageChoice::Existing);
        let is_perspective = matches!(source, SendSource::Perspective { .. });
        Self {
            spec: SendSpec {
                source,
                page,
                scale: None,
                placement: Placement::FirstFree,
            },
            pages,
            floors,
            layer_sets,
            layouts: Vec::new(),
            target: LayoutTarget::Current,
            snapshot_offered: false,
            snapshot: None,
            saved_view: None,
            link_saved_view: false,
            screen: None,
            extent: if is_perspective {
                ExtentChoice::AsImage
            } else {
                ExtentChoice::Entire
            },
            camera: CameraLink::Always,
            color_fill: false,
            use_edge_defaults: false,
            use_pattern_defaults: false,
            scale_choice: ScaleChoice::Largest,
            named: Scale::QuarterInch,
            typed: String::new(),
            layout_line_scaling: true,
            snap_to_point: false,
            show_page: true,
            all_remaining: false,
            remaining: 0,
            warning: None,
            warn_key: None,
        }
    }

    /// Offers the plan's layout files (the open one first).
    pub fn with_layouts(mut self, names: Vec<String>) -> Self {
        self.layouts = names;
        self
    }

    /// Offers the 3D view as a picture; `chosen` starts with it picked.
    pub fn with_snapshot(mut self, chosen: bool) -> Self {
        self.snapshot_offered = true;
        self.snapshot = chosen.then(SnapshotSpec::default);
        self
    }

    /// The Saved Plan View the plan is showing: Link Saved Plan View starts
    /// checked and names it.
    pub fn with_saved_view(mut self, name: Option<String>) -> Self {
        self.link_saved_view = name.is_some();
        self.saved_view = name;
        self
    }

    /// The part of the plan view on screen (plan inches), for Current Screen.
    pub fn with_screen(mut self, extent: Option<[f64; 4]>) -> Self {
        self.screen = extent;
        self
    }

    /// The Drawing Scale of the active view's Drawing Sheet Setup, the
    /// dialog's starting scale when the plan has set one (manual p. 1426).
    pub fn with_default_scale(mut self, scale: Option<Scale>) -> Self {
        if let Some(s) = scale {
            self.scale_choice = ScaleChoice::Named;
            self.named = s;
        }
        self
    }

    /// `n` more selected views wait behind this one.
    pub fn with_remaining(mut self, n: usize) -> Self {
        self.remaining = n;
        self
    }

    /// Starts from the settings used last in this session.
    pub fn with_defaults(mut self, last: &SendAnswers) -> Self {
        self.set_details(&last.details);
        self
    }

    fn set_details(&mut self, d: &SendDetails) {
        self.camera = d.options.camera;
        self.color_fill = d.options.plot.color_fill;
        self.use_edge_defaults = d.options.plot.use_edge_defaults;
        self.use_pattern_defaults = d.options.plot.use_pattern_defaults;
        self.layout_line_scaling = d.options.layout_line_scaling;
        self.snap_to_point = d.snap_to_point;
        self.show_page = d.show_page;
        match d.scale {
            SendScale::Largest(_) => self.scale_choice = ScaleChoice::Largest,
            SendScale::Named(s) => {
                self.scale_choice = ScaleChoice::Named;
                self.named = s;
            }
            SendScale::PerFoot(_) => {}
            SendScale::FitToSheet => self.scale_choice = ScaleChoice::FitToSheet,
        }
    }

    pub fn spec(&self) -> &SendSpec {
        &self.spec
    }

    pub fn target(&self) -> &LayoutTarget {
        &self.target
    }

    pub fn snapshot(&self) -> Option<SnapshotSpec> {
        self.snapshot
    }

    /// The typed scale as paper inches per foot, if it reads as one.
    fn typed_ipf(&self) -> Option<f64> {
        parse_scale_text(&self.typed)
    }

    /// The details for the answers now.
    pub fn details(&self) -> SendDetails {
        let scale = match self.scale_choice {
            ScaleChoice::Largest => SendScale::Largest(plan_layout::AUTO_SCALE_CEILING),
            ScaleChoice::Named => SendScale::Named(self.named),
            ScaleChoice::Typed => self
                .typed_ipf()
                .map_or(SendScale::Named(self.named), SendScale::PerFoot),
            ScaleChoice::FitToSheet => SendScale::FitToSheet,
        };
        let mut options = SendOptions {
            extent: match self.extent {
                ExtentChoice::Entire | ExtentChoice::AsImage => SendExtent::EntireView,
                ExtentChoice::Screen => self
                    .screen
                    .map_or(SendExtent::EntireView, SendExtent::CurrentScreen),
            },
            link_saved_view: if self.link_saved_view {
                self.saved_view.clone()
            } else {
                None
            },
            camera: self.camera,
            layout_line_scaling: self.layout_line_scaling,
            fit_to_sheet: self.scale_choice == ScaleChoice::FitToSheet,
            snap_to_point: self.snap_to_point,
            ..SendOptions::default()
        };
        options.plot.color_fill = self.color_fill;
        options.plot.use_edge_defaults = self.use_edge_defaults;
        options.plot.use_pattern_defaults = self.use_pattern_defaults;
        if self.extent == ExtentChoice::AsImage {
            // A picture is a snapshot: no live link.
            options.camera = CameraLink::Always;
        }
        SendDetails {
            scale,
            options,
            snap_to_point: self.snap_to_point,
            show_page: self.show_page,
            all_remaining: self.all_remaining && self.remaining > 0,
            as_image: self.as_image(),
        }
    }

    /// Current Screen As Image: send a picture instead of a linked view.
    pub fn as_image(&self) -> bool {
        self.extent == ExtentChoice::AsImage
            && !matches!(self.spec.source, SendSource::Perspective { .. })
    }

    /// Everything the dialog answers.
    pub fn answers(&self) -> SendAnswers {
        let mut spec = self.spec.clone();
        spec.scale = match self.details().scale {
            SendScale::Named(s) => Some(s),
            _ => None,
        };
        SendAnswers {
            spec,
            details: self.details(),
            target: self.target.clone(),
            snapshot: self.snapshot,
        }
    }

    fn error(&self) -> Option<&'static str> {
        if self.scale_choice == ScaleChoice::Typed && self.typed_ipf().is_none() {
            return Some("Type a scale such as 1:240, 1/8\" = 1' or 1 in = 80 ft");
        }
        match &self.target {
            LayoutTarget::New(n) if n.trim().is_empty() => Some("Name the new layout file"),
            LayoutTarget::New(n)
                if self
                    .layouts
                    .iter()
                    .any(|l| l.trim().eq_ignore_ascii_case(n.trim())) =>
            {
                Some("A layout file with that name exists")
            }
            _ => None,
        }
    }

    /// `warn` says whether the answers make a view too big for the sheet; it
    /// is asked again only when the answers change.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        warn: &mut dyn FnMut(&SendAnswers) -> Option<String>,
    ) -> Outcome {
        let answers = self.answers();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{answers:?}").hash(&mut h);
        let key = h.finish();
        if self.warn_key != Some(key) {
            self.warning = if self.error().is_none() && !self.as_image() {
                warn(&answers)
            } else {
                None
            };
            self.warn_key = Some(key);
        }
        let error = self.error();
        let warning = self.warning.clone();
        frame(ctx, "Send to Layout", 460.0, error, |ui| {
            if let Some(w) = &warning {
                ui.colored_label(egui::Color32::from_rgb(0xFF, 0xB0, 0x40), w);
            }
            self.layout_section(ui);
            self.source_section(ui);
            self.position_section(ui);
            self.options_section(ui);
            self.scaling_section(ui);
            if self.remaining > 0 {
                ui.add_space(6.0);
                ui.checkbox(
                    &mut self.all_remaining,
                    format!(
                        "Send all remaining views to layout with these settings ({})",
                        self.remaining
                    ),
                );
            }
        })
    }

    fn layout_section(&mut self, ui: &mut egui::Ui) {
        if self.layouts.is_empty() {
            return;
        }
        section(ui, "Choose layout");
        row(ui, "Send to", |ui| {
            let shown = match &self.target {
                LayoutTarget::Current => self
                    .layouts
                    .first()
                    .map_or("The open layout".to_string(), |n| format!("{n} (open)")),
                LayoutTarget::Existing(n) => n.clone(),
                LayoutTarget::New(_) => "New layout file".to_string(),
            };
            egui::ComboBox::from_id_salt("stl_layout_file")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (i, n) in self.layouts.iter().enumerate() {
                        let (value, label) = if i == 0 {
                            (LayoutTarget::Current, format!("{n} (open)"))
                        } else {
                            (LayoutTarget::Existing(n.clone()), n.clone())
                        };
                        ui.selectable_value(&mut self.target, value, label);
                    }
                    if ui
                        .selectable_label(
                            matches!(self.target, LayoutTarget::New(_)),
                            "New layout file...",
                        )
                        .clicked()
                        && !matches!(self.target, LayoutTarget::New(_))
                    {
                        self.target = LayoutTarget::New(String::new());
                    }
                });
        });
        if let LayoutTarget::New(name) = &mut self.target {
            row(ui, "Name", |ui| {
                ui.add(egui::TextEdit::singleline(name).desired_width(220.0));
            });
        }
    }

    fn source_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "Source view");
        match &mut self.spec.source {
            SendSource::Plan { floor, layer_set } => {
                row(ui, "View type", |ui| {
                    ui.label(if self.saved_view.is_some() {
                        "Saved plan view"
                    } else {
                        "Floor level"
                    })
                });
                row(ui, "Floor plan", |ui| {
                    let shown = self.floors.get(*floor).cloned().unwrap_or_default();
                    egui::ComboBox::from_id_salt("stl_floor")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (i, n) in self.floors.iter().enumerate() {
                                ui.selectable_value(floor, i, n);
                            }
                        });
                });
                row(ui, "Layer set", |ui| {
                    egui::ComboBox::from_id_salt("stl_layer_set")
                        .selected_text(layer_set.clone())
                        .show_ui(ui, |ui| {
                            for n in &self.layer_sets {
                                ui.selectable_value(layer_set, n.clone(), n);
                            }
                        });
                });
            }
            SendSource::Camera { name, .. } => {
                row(ui, "View type", |ui| ui.label("Cross section/elevation"));
                row(ui, "View name", |ui| ui.label(name.as_str()));
            }
            SendSource::Perspective { name, .. } => {
                row(ui, "View type", |ui| ui.label("Camera view"));
                row(ui, "View name", |ui| ui.label(name.as_str()));
                ui.weak("Rendered views are sent as an image; Update Views renders them again.");
            }
        }
        if self.snapshot_offered {
            let mut picture = self.snapshot.is_some();
            if ui
                .checkbox(&mut picture, "Send a picture of the 3D view as it is now")
                .on_hover_text(
                    "A frozen picture of the view on screen (its camera and everything it shows), embedded in the PDF",
                )
                .changed()
            {
                self.snapshot = picture.then(SnapshotSpec::default);
            }
            if let Some(sn) = &mut self.snapshot {
                row(ui, "Width", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut sn.width_in)
                            .range(1.0..=60.0)
                            .speed(0.1)
                            .suffix("\""),
                    );
                });
                row(ui, "Resolution", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut sn.dpi)
                            .range(20..=600)
                            .suffix(" dpi"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut sn.samples)
                            .range(1..=512)
                            .suffix(" samples"),
                    );
                });
            }
        }
    }

    fn position_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "Send position");
        row(ui, "Send to layout page #", |ui| {
            let shown = match self.spec.page {
                PageChoice::Existing(n) => self
                    .pages
                    .iter()
                    .find(|p| p.0 == n)
                    .map_or_else(|| format!("A-{n}"), |p| p.1.clone()),
                PageChoice::New => "New page".to_string(),
            };
            egui::ComboBox::from_id_salt("stl_page")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for p in &self.pages {
                        ui.selectable_value(
                            &mut self.spec.page,
                            PageChoice::Existing(p.0),
                            p.1.clone(),
                        );
                    }
                    ui.selectable_value(&mut self.spec.page, PageChoice::New, "New page");
                });
        });
        row(ui, "Position", |ui| {
            ui.radio_value(
                &mut self.spec.placement,
                Placement::FirstFree,
                "First free area",
            );
            ui.radio_value(&mut self.spec.placement, Placement::Centered, "Centered");
            ui.radio_value(&mut self.spec.placement, Placement::Click, "Click on page");
        });
        ui.checkbox(&mut self.snap_to_point, "Snap to Active CAD Point");
        ui.checkbox(&mut self.show_page, "Show Layout Page");
    }

    fn options_section(&mut self, ui: &mut egui::Ui) {
        let perspective = matches!(self.spec.source, SendSource::Perspective { .. });
        let is_plan = matches!(self.spec.source, SendSource::Plan { .. });
        section(ui, "Send options");
        ui.add_enabled_ui(!perspective, |ui| {
            ui.radio_value(&mut self.extent, ExtentChoice::Entire, "Entire Plan/View");
            ui.add_enabled_ui(true, |ui| {
                ui.radio_value(&mut self.extent, ExtentChoice::Screen, "Current Screen")
                    .on_hover_text(if self.screen.is_some() {
                        "Only what is on screen"
                    } else {
                        "The plan view is not showing: the whole view is sent"
                    });
            });
        });
        ui.radio_value(
            &mut self.extent,
            ExtentChoice::AsImage,
            "Current Screen As Image",
        )
        .on_hover_text("An embedded picture: it never updates, only gets replaced");
        if is_plan {
            let name = self
                .saved_view
                .as_ref()
                .map_or("none".to_string(), |n| n.clone());
            ui.add_enabled(
                self.saved_view.is_some() && self.extent != ExtentChoice::AsImage,
                egui::Checkbox::new(
                    &mut self.link_saved_view,
                    format!("Link Saved Plan View ({name})"),
                ),
            );
        }
        let camera_view = matches!(self.spec.source, SendSource::Camera { .. });
        if camera_view && self.extent != ExtentChoice::AsImage {
            section(ui, "Camera view options");
            ui.radio_value(
                &mut self.camera,
                CameraLink::Always,
                "Live View: Always Update",
            )
            .on_hover_text("Updates any time the model changes; may slow the program");
            ui.radio_value(
                &mut self.camera,
                CameraLink::OnDemand,
                "Live View: Update on Demand",
            )
            .on_hover_text("Updates when asked and when its page prints");
            ui.radio_value(&mut self.camera, CameraLink::PlotLines, "Plot Lines")
                .on_hover_text("Edge and pattern lines you can edit; only you update them");
            let plot = self.camera == CameraLink::PlotLines;
            ui.add_enabled_ui(plot, |ui| {
                ui.checkbox(&mut self.color_fill, "Color Fill");
                ui.checkbox(&mut self.use_edge_defaults, "Use Edge Line Defaults");
                ui.checkbox(&mut self.use_pattern_defaults, "Use Pattern Line Defaults");
            });
        }
    }

    fn scaling_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "Scaling");
        let perspective = matches!(self.spec.source, SendSource::Perspective { .. });
        ui.add_enabled_ui(!perspective, |ui| {
            ui.radio_value(
                &mut self.scale_choice,
                ScaleChoice::FitToSheet,
                "Fit to Sheet (No Scale)",
            );
            ui.radio_value(
                &mut self.scale_choice,
                ScaleChoice::Largest,
                "Largest scale that fits",
            );
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.scale_choice, ScaleChoice::Named, "Scale");
                egui::ComboBox::from_id_salt("stl_scale")
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
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.typed)
                        .hint_text("1:240 or 1/8\" = 1'")
                        .desired_width(150.0),
                );
                if edit.changed() {
                    self.scale_choice = ScaleChoice::Typed;
                }
                if self.scale_choice == ScaleChoice::Typed {
                    if let Some(ipf) = self.typed_ipf() {
                        ui.weak(format!("1:{:.0}", 12.0 / ipf));
                    }
                }
            });
        });
        ui.checkbox(
            &mut self.layout_line_scaling,
            RichText::new("Use Layout Line Scaling"),
        )
        .on_hover_text("Lines look like lines of the same weight drawn on the layout");
    }
}

/// A short description of the answers for the status line.
pub fn describe(a: &SendAnswers) -> String {
    let scale = match a.details.scale {
        SendScale::Largest(_) => "largest scale that fits".to_string(),
        SendScale::Named(s) => s.label().to_string(),
        SendScale::PerFoot(v) => plan_layout::custom_label(v),
        SendScale::FitToSheet => "no scale".to_string(),
    };
    format!("Sent to layout at {scale}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages() -> PageList {
        vec![(1, "A-1  Plans".into()), (2, "A-2  Elevations".into())]
    }

    fn plan_dialog() -> SendToLayoutDialog {
        SendToLayoutDialog::new(
            SendSource::Plan {
                floor: 0,
                layer_set: "Floor Plan".into(),
            },
            pages(),
            Some(2),
            vec!["1st Floor".into()],
            vec!["Floor Plan".into()],
        )
    }

    fn camera_dialog() -> SendToLayoutDialog {
        SendToLayoutDialog::new(
            SendSource::Camera {
                id: 7,
                name: "Front Elevation".into(),
            },
            pages(),
            None,
            vec![],
            vec![],
        )
    }

    fn headless(d: &mut SendToLayoutDialog, warn: &mut dyn FnMut(&SendAnswers) -> Option<String>) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            d.show(ctx, warn);
        });
    }

    #[test]
    fn the_defaults_send_the_whole_view_live_at_the_largest_scale_that_fits() {
        let d = plan_dialog();
        let a = d.answers();
        assert_eq!(a.spec.page, PageChoice::Existing(2), "the current page");
        assert_eq!(a.spec.scale, None);
        assert_eq!(
            a.details.scale,
            SendScale::Largest(plan_layout::AUTO_SCALE_CEILING)
        );
        assert_eq!(a.details.options.extent, SendExtent::EntireView);
        assert_eq!(a.details.options.camera, CameraLink::Always);
        assert!(a.details.options.layout_line_scaling);
        assert!(a.details.show_page && !a.details.snap_to_point && !a.details.as_image);
        assert_eq!(a.details.options.link_saved_view, None);
    }

    #[test]
    fn site_scales_a_typed_ratio_and_fit_to_sheet_are_choices() {
        let mut d = plan_dialog();
        d.scale_choice = ScaleChoice::Named;
        d.named = Scale::OneInchEq100Ft;
        assert_eq!(d.details().scale, SendScale::Named(Scale::OneInchEq100Ft));
        assert_eq!(d.answers().spec.scale, Some(Scale::OneInchEq100Ft));
        assert!(Scale::choices().contains(&Scale::OneInchEq30Ft));
        assert!(Scale::choices().contains(&Scale::OneInchEq100Ft));
        d.scale_choice = ScaleChoice::Typed;
        d.typed = "1:600".into();
        assert_eq!(d.details().scale, SendScale::PerFoot(0.02));
        assert_eq!(d.error(), None);
        d.typed = "banana".into();
        assert!(
            d.error().is_some(),
            "a scale that reads as nothing is refused"
        );
        d.scale_choice = ScaleChoice::FitToSheet;
        assert_eq!(d.error(), None);
        assert_eq!(d.details().scale, SendScale::FitToSheet);
        assert!(d.details().options.fit_to_sheet);
    }

    #[test]
    fn send_options_and_camera_view_options_reach_the_answers() {
        let mut d = plan_dialog().with_saved_view(Some("Main Plan".into()));
        assert!(
            d.link_saved_view,
            "starts checked while a saved view is in use"
        );
        assert_eq!(
            d.details().options.link_saved_view.as_deref(),
            Some("Main Plan")
        );
        d.link_saved_view = false;
        assert_eq!(d.details().options.link_saved_view, None);
        // Current Screen needs to know the screen.
        d.extent = ExtentChoice::Screen;
        assert_eq!(d.details().options.extent, SendExtent::EntireView);
        let mut d = plan_dialog().with_screen(Some([0.0, 0.0, 100.0, 50.0]));
        d.extent = ExtentChoice::Screen;
        assert_eq!(
            d.details().options.extent,
            SendExtent::CurrentScreen([0.0, 0.0, 100.0, 50.0])
        );
        // An elevation as Plot Lines with Color Fill and the line defaults.
        let mut c = camera_dialog();
        c.camera = CameraLink::PlotLines;
        c.color_fill = true;
        c.use_edge_defaults = true;
        c.use_pattern_defaults = true;
        let o = c.details().options;
        assert_eq!(o.camera, CameraLink::PlotLines);
        assert!(o.plot.color_fill && o.plot.use_edge_defaults && o.plot.use_pattern_defaults);
        // As an image it is a picture: static, no live link.
        c.extent = ExtentChoice::AsImage;
        assert!(c.details().as_image);
        assert_eq!(c.details().options.camera, CameraLink::Always);
        // A rendered (perspective) view is an image already, not an embedded
        // drawing.
        let p = SendToLayoutDialog::new(
            SendSource::Perspective {
                id: 3,
                name: "View".into(),
            },
            pages(),
            None,
            vec![],
            vec![],
        );
        assert!(!p.details().as_image);
    }

    #[test]
    fn send_all_remaining_needs_waiting_views_and_the_last_answers_carry_over() {
        let mut d = plan_dialog();
        d.all_remaining = true;
        assert!(!d.details().all_remaining, "nothing is waiting");
        let mut d = plan_dialog().with_remaining(3);
        d.all_remaining = true;
        d.scale_choice = ScaleChoice::Named;
        d.named = Scale::OneInchEq50Ft;
        d.show_page = false;
        d.camera = CameraLink::OnDemand;
        let last = d.answers();
        assert!(last.details.all_remaining && !last.details.show_page);
        let next = camera_dialog().with_defaults(&last);
        assert_eq!(next.details().scale, SendScale::Named(Scale::OneInchEq50Ft));
        assert_eq!(next.camera, CameraLink::OnDemand);
        assert!(!next.show_page);
    }

    #[test]
    fn the_too_big_warning_is_asked_for_once_per_set_of_answers() {
        let mut d = plan_dialog();
        let mut asked = 0;
        let mut warn = |_: &SendAnswers| {
            asked += 1;
            Some("too big".to_string())
        };
        headless(&mut d, &mut warn);
        headless(&mut d, &mut warn);
        assert_eq!(asked, 1, "the answers did not change");
        assert_eq!(d.warning.as_deref(), Some("too big"));
        d.scale_choice = ScaleChoice::Named;
        let mut warn = |_: &SendAnswers| None;
        headless(&mut d, &mut warn);
        assert_eq!(d.warning, None);
        // A picture has no scale to warn about.
        d.extent = ExtentChoice::AsImage;
        let mut warn = |_: &SendAnswers| Some("never".to_string());
        headless(&mut d, &mut warn);
        assert_eq!(d.warning, None);
    }

    #[test]
    fn a_new_layout_file_needs_a_name_nobody_has() {
        let mut d = plan_dialog().with_layouts(vec!["House Layout".into()]);
        d.target = LayoutTarget::New(String::new());
        assert!(d.error().is_some());
        d.target = LayoutTarget::New("house layout".into());
        assert!(d.error().is_some(), "names compare without case");
        d.target = LayoutTarget::New("Presentation".into());
        assert_eq!(d.error(), None);
        assert_eq!(d.answers().target, LayoutTarget::New("Presentation".into()));
    }
}
