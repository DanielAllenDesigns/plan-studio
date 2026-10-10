//! Select Layer: the layer chooser shared by Send to Layer, the Current CAD
//! Layer and the Active Layers by Tool table (manual p. 215). A filter box
//! over a table of the layers (name, where used, display, lock, colour) and,
//! where the caller offers it, the "Use Default Layer" check box that sends
//! objects to their own system default layer instead of the chosen one.

use crate::editor::selection::layer_of;
use crate::editor::EditorContext;
use crate::editor::ObjectRef;
use eframe::egui;
use plan_core::layers::{is_system_layer, LayerUse};

/// The state of one Select Layer panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectLayer {
    pub chosen: Option<String>,
    pub filter: String,
    /// "Use Default Layer": objects go to their system default layer and the
    /// table is not used.
    pub use_default: bool,
}

impl SelectLayer {
    pub fn new(chosen: Option<String>) -> Self {
        Self {
            chosen,
            ..Self::default()
        }
    }

    /// The layer to use; `None` while Use Default Layer is on or nothing is
    /// chosen.
    pub fn layer(&self) -> Option<&str> {
        if self.use_default {
            None
        } else {
            self.chosen.as_deref()
        }
    }

    /// Whether OK has something to do.
    pub fn ready(&self) -> bool {
        self.use_default || self.chosen.is_some()
    }

    /// The layers the filter shows, in plan order.
    pub fn names(&self, cx: &EditorContext) -> Vec<String> {
        let needle = self.filter.trim().to_lowercase();
        cx.project
            .layers
            .layers
            .iter()
            .filter(|l| needle.is_empty() || l.name.to_lowercase().contains(&needle))
            .map(|l| l.name.clone())
            .collect()
    }

    /// Draws the filter, the table and (when `offer_default`) the Use
    /// Default Layer check box.
    pub fn panel(&mut self, ui: &mut egui::Ui, cx: &EditorContext, offer_default: bool) {
        if offer_default {
            ui.checkbox(&mut self.use_default, "Use Default Layer")
                .on_hover_text("Put each object on its own system default layer");
        }
        ui.add_enabled_ui(!self.use_default, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter"));
            let counts = cx.project.layer_object_counts();
            let effective = cx.layers();
            egui::ScrollArea::vertical()
                .id_salt("select_layer_table")
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("select_layer_grid")
                        .num_columns(5)
                        .striped(true)
                        .spacing([8.0, 3.0])
                        .show(ui, |ui| {
                            for h in ["Name", "Used", "Disp", "Lock", "Color"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            for name in self.names(cx) {
                                let Some(l) = effective.get(&name) else {
                                    continue;
                                };
                                let on = self.chosen.as_deref() == Some(name.as_str());
                                if ui.selectable_label(on, &name).clicked() {
                                    self.chosen = Some(name.clone());
                                }
                                let info = LayerUse {
                                    objects: counts.get(&name).copied().unwrap_or(0),
                                    defaults: cx.project.layer_defaults(&name),
                                    system: is_system_layer(&name),
                                };
                                let r = ui.label(if info.in_use() {
                                    "\u{25CF}"
                                } else if info.system {
                                    "\u{25CB}"
                                } else {
                                    ""
                                });
                                if !info.tooltip().is_empty() {
                                    r.on_hover_text(info.tooltip());
                                }
                                ui.label(if l.display { "\u{2713}" } else { "" });
                                ui.label(if l.locked { "\u{1F512}" } else { "" });
                                let [r, g, b] = l.color;
                                ui.colored_label(egui::Color32::from_rgb(r, g, b), "\u{25A0}");
                                ui.end_row();
                            }
                        });
                });
        });
    }
}

/// A drop-down of the plan's layers; returns the layer picked this frame.
pub fn layer_combo(
    ui: &mut egui::Ui,
    cx: &EditorContext,
    salt: impl std::hash::Hash,
    current: &str,
    width: f32,
) -> Option<String> {
    let mut pick = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current.to_string())
        .width(width)
        .show_ui(ui, |ui| {
            for l in &cx.project.layers.layers {
                if ui.selectable_label(l.name == current, &l.name).clicked() {
                    pick = Some(l.name.clone());
                }
            }
        });
    pick
}

/// The system default layer of an object (Use Default Layer): the layer its
/// tool draws on when nothing else is chosen. `None` for kinds that live on
/// the layer of their kind.
pub fn default_layer_of(cx: &EditorContext, o: ObjectRef) -> Option<String> {
    let layers = &cx.project.layers;
    match o {
        ObjectRef::Wall(id) => {
            let w = cx.floor().wall(id)?;
            let key = if w.kind == plan_core::model::WallKind::Exterior {
                "walls_exterior"
            } else {
                "walls_interior"
            };
            Some(layers.tool_layer(key))
        }
        ObjectRef::Cad(_) => Some(layers.tool_layer("cad")),
        ObjectRef::Text(_) => Some(layers.tool_layer("text")),
        _ => None,
    }
}

/// Send to Layer with the chosen layer, or with Use Default Layer each
/// object to its own default; one undo step. Returns how many moved.
pub fn send_selection(cx: &mut EditorContext, choice: &SelectLayer) -> usize {
    if let Some(layer) = choice.layer() {
        return cx.send_selection_to_layer(layer);
    }
    if !choice.use_default {
        return 0;
    }
    let items = cx.selection.items.clone();
    send_items_to_default(cx, &items, "Send to Layer")
}

/// Puts each of `items` on its own system default layer (the Layer Painter's
/// and Send to Layer's "Use Default Layer"); one undo step named `label`.
/// Returns how many moved.
pub fn send_items_to_default(cx: &mut EditorContext, items: &[ObjectRef], label: &str) -> usize {
    let targets: Vec<(ObjectRef, String)> = items
        .iter()
        .filter_map(|o| default_layer_of(cx, *o).map(|l| (*o, l)))
        .filter(|(_, l)| cx.project.layers.get(l).is_some())
        .collect();
    if targets.is_empty() {
        cx.status = "Those objects stay on the layer of their kind".into();
        return 0;
    }
    let locked =
        |o: &ObjectRef| layer_of(cx.floor(), *o).is_some_and(|l| cx.project.layers.is_locked(&l));
    if items.iter().any(locked) {
        cx.status = "Those objects are on a locked layer".into();
        return 0;
    }
    cx.begin_change(label);
    let fl = cx.floor;
    let mut n = 0;
    for (o, layer) in &targets {
        let f = &mut cx.project.floors[fl];
        match *o {
            ObjectRef::Wall(id) => {
                if let Some(w) = f.wall_mut(id) {
                    w.layer = layer.clone();
                    n += 1;
                }
            }
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                if let Some(c) = f.cad.iter_mut().find(|c| c.id == id) {
                    c.layer = layer.clone();
                    n += 1;
                }
            }
            _ => {}
        }
    }
    cx.mark_dirty();
    cx.status = format!(
        "Moved {n} object{} to their default layers",
        if n == 1 { "" } else { "s" }
    );
    n
}
