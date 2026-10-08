//! Reference Display (R-65, R-66, LAY-10): which floor the plan shows as a
//! dimmed reference under the active floor (the floor below, the floor above
//! or any floor), which layer set decides what of it is drawn, and the color
//! it is drawn in. Opened from Tools > Floor/Reference Display and the
//! Reference Display Options toolbar button; the Reference Display toggle
//! turns the display on and off.
//!
//! The choices are kept for the session; the floor is also written to the
//! active saved plan view (`reference_floor`, relative to the viewed floor)
//! so a view carries it.

use super::{row, section, Outcome};
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::{Project, Wall};
use std::cell::RefCell;

/// Which floor is the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceFloor {
    /// The floor under the active one.
    Below,
    /// The floor over the active one.
    Above,
    /// A fixed floor, by index.
    Floor(usize),
}

/// The Reference Display choices.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceSettings {
    pub floor: ReferenceFloor,
    /// Name of the layer set that decides which reference walls are drawn;
    /// `None` uses the active view's layers.
    pub layer_set: Option<String>,
    /// Color of the reference linework.
    pub color: [u8; 3],
}

impl Default for ReferenceSettings {
    fn default() -> Self {
        Self {
            floor: ReferenceFloor::Below,
            layer_set: None,
            color: [128, 128, 128],
        }
    }
}

thread_local! {
    static SETTINGS: RefCell<Option<ReferenceSettings>> = const { RefCell::new(None) };
}

/// The choices in force: the session's, else the floor below in gray, with
/// the active plan view's `reference_floor` (relative to `current`) deciding
/// above or below when it has one.
pub fn settings(project: &Project) -> ReferenceSettings {
    if let Some(s) = SETTINGS.with(|s| s.borrow().clone()) {
        return s;
    }
    let rel = project
        .current_plan_view()
        .and_then(|v| v.reference_floor)
        .unwrap_or(-1);
    ReferenceSettings {
        floor: if rel > 0 {
            ReferenceFloor::Above
        } else {
            ReferenceFloor::Below
        },
        ..ReferenceSettings::default()
    }
}

/// Keeps `s` as the choices in force.
pub fn set_settings(s: ReferenceSettings) {
    SETTINGS.with(|c| *c.borrow_mut() = Some(s));
}

/// Forgets the session's choices (back to the floor below in gray).
#[cfg_attr(not(test), allow(dead_code))]
pub fn reset_settings() {
    SETTINGS.with(|c| *c.borrow_mut() = None);
}

/// The floor index shown as the reference while `current` is active, if any:
/// out of range, or the active floor itself, shows nothing.
pub fn target_floor(s: &ReferenceSettings, current: usize, count: usize) -> Option<usize> {
    let idx = match s.floor {
        ReferenceFloor::Below => current.checked_sub(1)?,
        ReferenceFloor::Above => current + 1,
        ReferenceFloor::Floor(i) => i,
    };
    (idx < count && idx != current).then_some(idx)
}

/// The walls of the reference floor that draw and snap (R-65, LAY-10):
/// nothing when Reference Display is off or there is no such floor; else the
/// walls on layers that show (in the chosen layer set, or the active view's)
/// and whose layer has its "Ref" box on.
pub fn reference_walls(cx: &EditorContext) -> Vec<&Wall> {
    if !cx.view_flags.contains(&ViewFlag::ReferenceDisplay) {
        return Vec::new();
    }
    let settings = settings(&cx.project);
    let Some(idx) = target_floor(&settings, cx.floor, cx.project.floors.len()) else {
        return Vec::new();
    };
    let set_layers = settings
        .layer_set
        .as_deref()
        .map(|n| cx.project.layer_sets.effective_for(n, &cx.project.layers));
    let layers = set_layers.as_ref().unwrap_or_else(|| cx.layers());
    cx.project.floors[idx]
        .walls
        .iter()
        .filter(|w| {
            !w.flags.invisible
                && layers.is_visible(&w.layer)
                && layers.shows_in_reference(&w.layer)
        })
        .collect()
}

/// The relative offset to store in a plan view for `s` while `current` is
/// active.
pub fn relative_offset(s: &ReferenceSettings, current: usize) -> i32 {
    match s.floor {
        ReferenceFloor::Below => -1,
        ReferenceFloor::Above => 1,
        ReferenceFloor::Floor(i) => i as i32 - current as i32,
    }
}

/// Reference Display dialog: a draft of [`ReferenceSettings`] plus the floors
/// and layer sets to choose from.
pub struct ReferenceDisplayDialog {
    settings: ReferenceSettings,
    /// Turn the display on when OK is pressed.
    show: bool,
    floors: Vec<String>,
    current: usize,
    layer_sets: Vec<String>,
}

// The setters are the dialog's model API (the tests drive it); the UI edits
// the same fields directly.
#[allow(dead_code)]
impl ReferenceDisplayDialog {
    pub fn new(
        settings: ReferenceSettings,
        show: bool,
        floors: Vec<String>,
        current: usize,
        layer_sets: Vec<String>,
    ) -> Self {
        Self {
            settings,
            show,
            floors,
            current,
            layer_sets,
        }
    }

    pub fn settings(&self) -> &ReferenceSettings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut ReferenceSettings {
        &mut self.settings
    }

    pub fn show_display(&self) -> bool {
        self.show
    }

    pub fn set_show_display(&mut self, on: bool) {
        self.show = on;
    }

    pub fn current(&self) -> usize {
        self.current
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Reference Display")
            .id(egui::Id::new("reference_display_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.checkbox(&mut self.show, "Show the reference floor");
                section(ui, "Reference Floor");
                ui.radio_value(
                    &mut self.settings.floor,
                    ReferenceFloor::Below,
                    "Floor below the active floor",
                );
                ui.radio_value(
                    &mut self.settings.floor,
                    ReferenceFloor::Above,
                    "Floor above the active floor",
                );
                let is_fixed = matches!(self.settings.floor, ReferenceFloor::Floor(_));
                ui.horizontal(|ui| {
                    if ui.radio(is_fixed, "This floor").clicked() && !is_fixed {
                        let first = usize::from(self.current == 0);
                        self.settings.floor = ReferenceFloor::Floor(first);
                    }
                    ui.add_enabled_ui(is_fixed, |ui| {
                        let mut idx = match self.settings.floor {
                            ReferenceFloor::Floor(i) => i,
                            _ => 0,
                        };
                        let shown = self.floors.get(idx).cloned().unwrap_or_default();
                        egui::ComboBox::from_id_salt("ref_floor")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (i, name) in self.floors.iter().enumerate() {
                                    if i != self.current {
                                        ui.selectable_value(&mut idx, i, name);
                                    }
                                }
                            });
                        if is_fixed {
                            self.settings.floor = ReferenceFloor::Floor(idx);
                        }
                    });
                });
                section(ui, "Appearance");
                row(ui, "Layer Set", |ui| {
                    let shown = self
                        .settings
                        .layer_set
                        .clone()
                        .unwrap_or_else(|| "Active view's layers".to_string());
                    egui::ComboBox::from_id_salt("ref_layer_set")
                        .width(200.0)
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.layer_set,
                                None,
                                "Active view's layers",
                            );
                            for n in &self.layer_sets {
                                ui.selectable_value(
                                    &mut self.settings.layer_set,
                                    Some(n.clone()),
                                    n,
                                );
                            }
                        });
                });
                row(ui, "Color", |ui| {
                    ui.color_edit_button_srgb(&mut self.settings.color);
                });
                ui.weak("Reference objects are drawn dimmed and cannot be picked; their wall ends and intersections snap. The Ref column of Layer Display Options chooses the layers.");
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("   OK   ").strong()).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_floor_follows_the_choice() {
        let mut s = ReferenceSettings::default();
        assert_eq!(
            target_floor(&s, 0, 3),
            None,
            "nothing under the lowest floor"
        );
        assert_eq!(target_floor(&s, 2, 3), Some(1));
        s.floor = ReferenceFloor::Above;
        assert_eq!(target_floor(&s, 0, 3), Some(1));
        assert_eq!(target_floor(&s, 2, 3), None, "nothing over the top floor");
        s.floor = ReferenceFloor::Floor(2);
        assert_eq!(target_floor(&s, 0, 3), Some(2));
        assert_eq!(
            target_floor(&s, 2, 3),
            None,
            "a floor is not its own reference"
        );
        assert_eq!(relative_offset(&s, 0), 2);
    }
}
