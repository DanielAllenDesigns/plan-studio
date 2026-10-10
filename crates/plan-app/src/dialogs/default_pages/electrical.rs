//! Default Settings > Electrical (manual pp. 691-693; E-11, E-19..E-22).
//!
//! One dialog with the three defaults dialogs of Chief as tabs:
//!
//! * **Electrical Defaults**: the Default Library Objects (the symbol each
//!   Electrical Tool places, picked with the Library button; here one of the
//!   built-in types of the tool's family), a preview, and the four Default
//!   Heights (Outlet, Switch, Above Base Cabinet measured up from the counter
//!   top, On Cabinet Side measured up from the cabinet's bottom) with the Use
//!   Default Heights switch.
//! * **Electrical Connection Defaults**: the Spline panel (curvature ratio),
//!   the Line Style (with an arrow) and the Label.
//! * **Rope Light Defaults**: the fields of the Rope Light Specification.
//!
//! The values belong to the plan (`Project::electrical_defaults`, read by
//! `plan_electrical::ElectricalDefaults`), so OK is one undo step, "Electrical
//! Defaults"; a plan with every default keeps no record. Double-clicking an
//! Electrical Tools button opens the page on the tab of that tool
//! ([`request_open_for`]).

use crate::dialogs::rope_light::{general_fields, spec_error};
use crate::dialogs::{row, section, Fields, Outcome, PV_INK};
use crate::editor::{site_view, Camera, EditorContext};
use crate::tools::electrical::{DefaultsTab, ElecVariant};
use eframe::egui::{self, Align, Align2, Color32, Key, Layout, Modifiers, Rect, Sense, Stroke};
use plan_core::{LineStyle, Point};
use plan_electrical::{Arrow, DeviceKind, ElectricalDefaults, ToolSlot, TOOL_SLOTS};

const ERROR_RED: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x7B);

/// The row of the Default Library Objects list that stands for the rope light
/// (its settings are the third tab).
const ROPE_ROW: usize = usize::MAX;

pub struct ElectricalPage {
    tab: DefaultsTab,
    defaults: ElectricalDefaults,
    /// The selected row of the Default Library Objects: an index into
    /// [`TOOL_SLOTS`], or [`ROPE_ROW`].
    selected: usize,
    fields: Fields,
}

impl ElectricalPage {
    pub fn new(cx: &EditorContext) -> Self {
        Self::with_tab(cx, DefaultsTab::Electrical)
    }

    pub fn with_tab(cx: &EditorContext, tab: DefaultsTab) -> Self {
        Self {
            tab,
            defaults: ElectricalDefaults::load(&cx.project),
            selected: 0,
            fields: Fields::default(),
        }
    }

    pub fn tab(&self) -> DefaultsTab {
        self.tab
    }

    /// The defaults as edited so far.
    pub fn defaults(&self) -> &ElectricalDefaults {
        &self.defaults
    }

    /// Mutable access for the tests and the tool.
    pub fn defaults_mut(&mut self) -> &mut ElectricalDefaults {
        &mut self.defaults
    }

    /// The kind the tool in row `slot` places as edited so far.
    pub fn object(&self, slot: &ToolSlot) -> DeviceKind {
        self.defaults.object(slot.key, slot.builtin)
    }

    /// The Library button: chooses `kind` for the tool named `key`.
    pub fn choose_object(&mut self, key: &str, kind: DeviceKind) -> bool {
        match TOOL_SLOTS.iter().find(|s| s.key == key) {
            Some(slot) if slot.choices.contains(&kind) => {
                self.defaults.set_object(slot.key, slot.builtin, kind);
                true
            }
            _ => false,
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        let d = &self.defaults;
        for (name, h) in [
            ("Outlet", d.outlet_height),
            ("Switch", d.switch_height),
            ("Above Base Cabinet", d.above_base_cabinet),
            ("On Cabinet Side", d.on_cabinet_side),
        ] {
            if !(0.0..=1200.0).contains(&h) {
                return Some(format!("{name}: the height must be between 0 and 100'"));
            }
        }
        if !(0.0..=0.5).contains(&d.connection.curvature_ratio) {
            return Some("The curvature ratio must be between 0 and 0.5".into());
        }
        spec_error(&d.rope)
    }

    /// Stores the defaults in the plan; one undo step. Returns whether the
    /// plan changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let before = ElectricalDefaults::load(&cx.project);
        if before == self.defaults {
            return false;
        }
        cx.begin_change("Electrical Defaults");
        self.defaults.store(&mut cx.project);
        cx.mark_dirty();
        true
    }

    /// Reset Page: the tab's values go back to the built-in ones.
    fn reset(&mut self) {
        let built = ElectricalDefaults::default();
        match self.tab {
            DefaultsTab::Electrical => {
                self.defaults.use_default_heights = built.use_default_heights;
                self.defaults.outlet_height = built.outlet_height;
                self.defaults.switch_height = built.switch_height;
                self.defaults.above_base_cabinet = built.above_base_cabinet;
                self.defaults.on_cabinet_side = built.on_cabinet_side;
                self.defaults.objects.clear();
            }
            DefaultsTab::Connection => self.defaults.connection = built.connection,
            DefaultsTab::RopeLight => self.defaults.rope = built.rope,
        }
        self.fields = Fields::default();
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (t, name) in [
                (DefaultsTab::Electrical, "Electrical Defaults"),
                (DefaultsTab::Connection, "Electrical Connection"),
                (DefaultsTab::RopeLight, "Rope Light"),
            ] {
                if ui.selectable_label(self.tab == t, name).clicked() {
                    self.tab = t;
                }
            }
        });
        ui.separator();
    }

    /// Default Library Objects, Preview and Default Heights.
    fn electrical_tab(&mut self, ui: &mut egui::Ui) {
        section(ui, "Default Library Objects");
        egui::ScrollArea::vertical()
            .id_salt("elec_defaults_objects")
            .max_height(150.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, slot) in TOOL_SLOTS.iter().enumerate() {
                    let kind = self.object(slot);
                    let changed =
                        std::mem::discriminant(&kind) != std::mem::discriminant(&slot.builtin);
                    let text = if changed {
                        format!("{}: {} (changed)", slot.key, kind.name())
                    } else {
                        format!("{}: {}", slot.key, kind.name())
                    };
                    if ui.selectable_label(self.selected == i, text).clicked() {
                        self.selected = i;
                    }
                }
                if ui
                    .selectable_label(
                        self.selected == ROPE_ROW,
                        "Rope Light: see the Rope Light tab",
                    )
                    .clicked()
                {
                    self.selected = ROPE_ROW;
                }
            });
        if let Some(slot) = TOOL_SLOTS.get(self.selected) {
            let current = self.object(slot);
            ui.horizontal(|ui| {
                ui.label(format!("Object: {}", current.name()));
                ui.add_enabled_ui(slot.choices.len() > 1, |ui| {
                    egui::ComboBox::from_id_salt("elec_default_library")
                        .selected_text("Library")
                        .show_ui(ui, |ui| {
                            for c in slot.choices {
                                if ui.selectable_label(*c == current, c.name()).clicked() {
                                    self.defaults.set_object(slot.key, slot.builtin, *c);
                                }
                            }
                        });
                });
                ui.add_enabled(false, egui::Button::new("Edit"))
                    .on_disabled_hover_text(
                        "Edit the placed object instead: its Electrical Service Specification",
                    );
            });
            ui.weak(current.description());
            preview(ui, current);
        } else {
            ui.weak(
                "The rope light has no library object: its settings are on the Rope Light tab.",
            );
        }
        section(ui, "Default Heights");
        ui.checkbox(
            &mut self.defaults.use_default_heights,
            "Use Default Heights",
        );
        ui.add_enabled_ui(self.defaults.use_default_heights, |ui| {
            let d = &mut self.defaults;
            self.fields
                .length_row(ui, "Outlet", "elec_outlet", &mut d.outlet_height);
            self.fields
                .length_row(ui, "Switch", "elec_switch", &mut d.switch_height);
            self.fields.length_row(
                ui,
                "Above Base Cabinet",
                "elec_above_base",
                &mut d.above_base_cabinet,
            );
            self.fields.length_row(
                ui,
                "On Cabinet Side",
                "elec_cab_side",
                &mut d.on_cabinet_side,
            );
        });
        ui.weak(
            "Outlet also sets the phone, data and TV jacks; Switch the doorbells and \
             thermostats. Above Base Cabinet is measured up from the counter top, On \
             Cabinet Side up from the bottom of the cabinet.",
        );
    }

    /// The Spline, Line Style and Label panels of the connection defaults.
    fn connection_tab(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.defaults.connection;
        section(ui, "Spline");
        row(ui, "Curvature Ratio", |ui| {
            ui.add(
                egui::DragValue::new(&mut c.curvature_ratio)
                    .speed(0.01)
                    .range(0.0..=0.5)
                    .fixed_decimals(2),
            )
        });
        ui.weak("The first arc's sag over its length: 0 draws a straight line.");
        section(ui, "Line Style");
        row(ui, "Line Style", |ui| {
            egui::ComboBox::from_id_salt("elec_conn_style")
                .selected_text(style_name(c.line_style))
                .show_ui(ui, |ui| {
                    for s in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        ui.selectable_value(&mut c.line_style, s, style_name(s));
                    }
                });
        });
        row(ui, "Arrow", |ui| {
            egui::ComboBox::from_id_salt("elec_conn_arrow")
                .selected_text(c.arrow.name())
                .show_ui(ui, |ui| {
                    for a in Arrow::ALL {
                        ui.selectable_value(&mut c.arrow, a, a.name());
                    }
                });
        });
        section(ui, "Label");
        row(ui, "Label", |ui| ui.text_edit_singleline(&mut c.label));
    }

    fn rope_tab(&mut self, ui: &mut egui::Ui) {
        general_fields(ui, &mut self.fields, &mut self.defaults.rope);
        section(ui, "Strip Profile");
        let r = &mut self.defaults.rope;
        self.fields
            .length_row(ui, "Width", "elec_rope_w", &mut r.profile_width);
        self.fields
            .length_row(ui, "Height", "elec_rope_h", &mut r.profile_height);
    }

    fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let mut reset = false;
        let error = self.error();
        egui::Window::new("Default Settings: Electrical")
            .id(egui::Id::new("default_page_electrical"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([480.0, 560.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                self.tabs(ui);
                let height = (ui.available_height() - 64.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt("default_page_electrical_scroll")
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        DefaultsTab::Electrical => self.electrical_tab(ui),
                        DefaultsTab::Connection => self.connection_tab(ui),
                        DefaultsTab::RopeLight => self.rope_tab(ui),
                    });
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                        .clicked()
                    {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if ui.button("Reset Page").clicked() {
                        reset = true;
                    }
                    if let Some(e) = &error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if reset {
            self.reset();
        }
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

fn style_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-Dot",
    }
}

/// The Preview pane: the symbol of the selected object.
fn preview(ui: &mut egui::Ui, kind: DeviceKind) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 84.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0_f32, Color32::from_gray(120)),
        egui::StrokeKind::Inside,
    );
    let cam = Camera {
        center: Point::ZERO,
        px_per_in: f64::from(rect.height().min(rect.width())) * 0.8 / 24.0,
        rect: Rect::from_center_size(rect.center(), rect.size()),
        rotation: 0.0,
    };
    site_view::draw_symbol(&p, &cam, &kind.symbol(), PV_INK, 1.5);
}

thread_local! {
    static OPEN: std::cell::Cell<Option<DefaultsTab>> = const { std::cell::Cell::new(None) };
    static PAGE: std::cell::RefCell<Option<ElectricalPage>> = const { std::cell::RefCell::new(None) };
}

/// Asks for the page; [`show`] opens it on its next frame.
pub fn request_open() {
    request_open_tab(DefaultsTab::Electrical);
}

/// Asks for the page on one of its three tabs.
pub fn request_open_tab(tab: DefaultsTab) {
    OPEN.with(|c| c.set(Some(tab)));
}

/// The double-click of an Electrical Tools button: the defaults page on the
/// tab of that tool (Electrical Connection, Rope Light or the general one).
pub fn request_open_for(tool: ElecVariant) {
    request_open_tab(tool.defaults_tab());
}

/// Is the page asked for or showing?
pub fn is_open() -> bool {
    OPEN.with(|c| c.get().is_some()) || PAGE.with(|p| p.borrow().is_some())
}

/// Runs `f` on the open page (for the tests).
#[cfg(test)]
pub fn with_page<R>(f: impl FnOnce(&mut ElectricalPage) -> R) -> Option<R> {
    PAGE.with(|p| p.borrow_mut().as_mut().map(f))
}

/// Draws the page (when open) and applies OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(tab) = OPEN.with(|c| c.take()) {
        PAGE.with(|p| *p.borrow_mut() = Some(ElectricalPage::with_tab(cx, tab)));
    }
    let Some(mut page) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match page.show(ctx) {
        Outcome::Open => PAGE.with(|p| *p.borrow_mut() = Some(page)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            cx.status = if page.apply(cx) {
                "Saved the electrical defaults".into()
            } else {
                "The electrical defaults did not change".into()
            };
        }
    }
}
