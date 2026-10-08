//! Default Settings > Electrical (E-11): the height each kind of device is
//! placed at. The heights belong to the plan (`Project::electrical_defaults`,
//! read by `plan_electrical::ElectricalDefaults`), so OK is one undo step,
//! "Electrical Defaults"; a height equal to the built-in one removes its
//! override, and a plan with no override keeps no record.

use crate::dialogs::{row, section, Outcome};
use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Color32, Key, Layout, Modifiers};
use plan_core::units::{fmt_ft_in, parse_ft_in};
use plan_electrical::{DeviceKind, ElectricalDefaults, COUNTER_OUTLET_KEY};
use std::collections::HashMap;

const ERROR_RED: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x7B);

/// One row of the page.
struct Row {
    /// `DeviceKind::name` or [`COUNTER_OUTLET_KEY`].
    key: &'static str,
    kind: Option<DeviceKind>,
    height: f64,
    builtin: f64,
}

pub struct ElectricalPage {
    rows: Vec<Row>,
    bufs: HashMap<&'static str, String>,
}

impl ElectricalPage {
    pub fn new(cx: &EditorContext) -> Self {
        let stored = ElectricalDefaults::load(&cx.project);
        let mut rows: Vec<Row> = DeviceKind::all()
            .into_iter()
            .map(|k| Row {
                key: k.name(),
                kind: Some(k),
                height: stored.height(k),
                builtin: k.default_height(),
            })
            .collect();
        rows.push(Row {
            key: COUNTER_OUTLET_KEY,
            kind: None,
            height: stored.counter_height(),
            builtin: plan_electrical::ElectricalDefaults::default().counter_height(),
        });
        ElectricalPage {
            rows,
            bufs: HashMap::new(),
        }
    }

    /// The keys of the rows, in page order.
    #[cfg(test)]
    pub fn keys(&self) -> Vec<&'static str> {
        self.rows.iter().map(|r| r.key).collect()
    }

    /// The height of the row `key`.
    #[cfg(test)]
    pub fn height(&self, key: &str) -> Option<f64> {
        self.rows.iter().find(|r| r.key == key).map(|r| r.height)
    }

    /// Sets the height of row `key`.
    pub fn set_height(&mut self, key: &str, h: f64) {
        if let Some(r) = self.rows.iter_mut().find(|r| r.key == key) {
            r.height = h;
            self.bufs.remove(r.key);
        }
    }

    fn error(&self) -> Option<String> {
        self.bufs
            .iter()
            .find(|(_, t)| parse_ft_in(t).is_none())
            .map(|(k, _)| format!("{k}: enter a valid height"))
            .or_else(|| {
                self.rows
                    .iter()
                    .find(|r| !(0.0..=1200.0).contains(&r.height))
                    .map(|r| format!("{}: the height must be between 0 and 100'", r.key))
            })
    }

    /// Stores the heights in the plan; one undo step. Returns whether the
    /// plan changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        let mut d = ElectricalDefaults::default();
        for r in &self.rows {
            match r.kind {
                Some(k) => d.set_height(k, r.height),
                None => d.set_counter_height(r.height),
            }
        }
        let before = ElectricalDefaults::load(&cx.project);
        if before == d {
            return false;
        }
        cx.begin_change("Electrical Defaults");
        d.store(&mut cx.project);
        cx.mark_dirty();
        true
    }

    fn reset(&mut self) {
        self.bufs.clear();
        for r in &mut self.rows {
            r.height = r.builtin;
        }
    }

    fn group(&self, title: &str) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| group_of(r) == title)
            .map(|(i, _)| i)
            .collect()
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
            .default_size([440.0, 520.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let height = (ui.available_height() - 64.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt("default_page_electrical_scroll")
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.weak("Heights above the finished floor to the center of the device. They are kept with this plan.");
                        for title in ["Receptacles", "Switches", "Lights", "Other"] {
                            section(ui, title);
                            for i in self.group(title) {
                                self.height_row(ui, i);
                            }
                        }
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

    fn height_row(&mut self, ui: &mut egui::Ui, i: usize) {
        let key = self.rows[i].key;
        let mut text = self
            .bufs
            .get(key)
            .cloned()
            .unwrap_or_else(|| fmt_ft_in(self.rows[i].height));
        let invalid = parse_ft_in(&text).is_none();
        let resp = row(ui, key, |ui| {
            let mut edit = egui::TextEdit::singleline(&mut text).desired_width(100.0);
            if invalid {
                edit = edit.text_color(ERROR_RED);
            }
            ui.add(edit)
        });
        if resp.changed() {
            if let Some(v) = parse_ft_in(&text) {
                self.rows[i].height = v;
            }
            self.bufs.insert(key, text);
        } else if !resp.has_focus() && self.bufs.get(key).is_some_and(|t| parse_ft_in(t).is_some())
        {
            self.bufs.remove(key);
        }
    }
}

fn group_of(r: &Row) -> &'static str {
    match r.kind {
        None => "Receptacles",
        Some(
            DeviceKind::Outlet110
            | DeviceKind::Outlet110Quad
            | DeviceKind::Outlet220
            | DeviceKind::Gfci
            | DeviceKind::OutletFloor
            | DeviceKind::OutletWp
            | DeviceKind::OutletDedicated,
        ) => "Receptacles",
        Some(
            DeviceKind::Switch
            | DeviceKind::Switch3Way
            | DeviceKind::Switch4Way
            | DeviceKind::SwitchDimmer,
        ) => "Switches",
        Some(
            DeviceKind::CeilingLight
            | DeviceKind::RecessedCan
            | DeviceKind::PendantLight
            | DeviceKind::WallSconce
            | DeviceKind::CeilingFan
            | DeviceKind::RopeLight { .. },
        ) => "Lights",
        Some(_) => "Other",
    }
}

thread_local! {
    static OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PAGE: std::cell::RefCell<Option<ElectricalPage>> = const { std::cell::RefCell::new(None) };
}

/// Asks for the page; [`show`] opens it on its next frame.
pub fn request_open() {
    OPEN.with(|c| c.set(true));
}

/// Is the page asked for or showing?
pub fn is_open() -> bool {
    OPEN.with(std::cell::Cell::get) || PAGE.with(|p| p.borrow().is_some())
}

/// Runs `f` on the open page (for the tests).
#[cfg(test)]
pub fn with_page<R>(f: impl FnOnce(&mut ElectricalPage) -> R) -> Option<R> {
    PAGE.with(|p| p.borrow_mut().as_mut().map(f))
}

/// Draws the page (when open) and applies OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if OPEN.with(|c| c.replace(false)) {
        PAGE.with(|p| *p.borrow_mut() = Some(ElectricalPage::new(cx)));
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
