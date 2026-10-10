//! Object Layer Properties, primary and secondary layers, the Layer Hider and
//! Find Objects on Layer(s) (LAY-62, LAY-64, LAY-72, LAY-73; manual pp. 206,
//! 216, 217).
//!
//! * Every object has one **primary layer**, which decides whether it shows,
//!   and some have **secondary layers** that only change how it looks (a
//!   cabinet's labels and module lines, a door's label, a wall's layer lines).
//!   [`layers_of_selection`] lists both for the selected objects; a secondary
//!   layer of an object whose primary layer is off shows nothing.
//! * The **Object Layer Properties** edit button opens a window with the same
//!   table as Layer Display Options, listing the layers the selected objects
//!   touch (or all layers with Show All Layers) and the properties of the
//!   selected rows. Edits go to the shown layer set.
//! * The **Layer Hider** tool turns off the primary layer of a clicked object
//!   in the shown layer set ([`hide_primary_layer`]).
//! * **Find Objects on Layer(s)** (the layer table's context menu) selects the
//!   objects on the chosen layers, asking in the **Select Location** window
//!   which floor to look at when they are on several.

use crate::dialogs::layer_display::{layer_panel, select_all_on_layers, LayerPanelState};
use crate::editor::actions::{EditAction, EditActionKind};
use crate::editor::selection::layer_of;
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Vec2};
use plan_core::layer_sets::LayerEdit;
use plan_core::layers::{
    BRICK_LEDGE_LAYER, CABINET_LABEL_LAYER, ELECTRICAL_CONNECTION_LAYER, FOOTINGS_LAYER,
    WALL_LAYERS_LAYER, WALL_MAIN_ONLY_LAYER, WALL_NO_LOCATE_LAYER, WALL_THROUGH_LINES_LAYER,
};
use std::cell::RefCell;

/// The Edit toolbar button (and command) that opens the window.
pub const OPEN: &str = "objlayers.open";

/// Is `id` a command of this module?
pub fn is_command(id: &str) -> bool {
    id == OPEN
}

// ----- primary and secondary layers -----

/// The layers a selection touches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectLayers {
    /// Primary layers, in the order the objects were selected.
    pub primary: Vec<String>,
    /// Secondary layers the plan has, without the primary ones.
    pub secondary: Vec<String>,
}

impl ObjectLayers {
    /// Every layer, primary first.
    pub fn all(&self) -> Vec<String> {
        let mut v = self.primary.clone();
        v.extend(self.secondary.iter().cloned());
        v
    }
}

/// The secondary layer names an object kind could use; the caller keeps the
/// ones the plan has.
fn secondary_candidates(cx: &EditorContext, o: ObjectRef) -> Vec<&'static str> {
    let floor = cx.floor();
    match o {
        ObjectRef::Wall(id) => {
            let mut v = vec![
                WALL_LAYERS_LAYER,
                WALL_MAIN_ONLY_LAYER,
                WALL_THROUGH_LINES_LAYER,
                BRICK_LEDGE_LAYER,
            ];
            if let Some(w) = floor.wall(id) {
                if w.is_foundation() {
                    v.push(FOOTINGS_LAYER);
                }
                if w.flags.no_locate {
                    v.push(WALL_NO_LOCATE_LAYER);
                }
            }
            v
        }
        ObjectRef::Opening(id) => floor
            .openings
            .iter()
            .find(|x| x.id == id)
            .map(|x| vec![plan_core::LayerSet::label_layer_of(x.kind)])
            .unwrap_or_default(),
        // The countertop, door and drawer fronts, front indicators, module
        // lines and labels of a cabinet (the first three exist only in plans
        // that made them).
        ObjectRef::Cabinet(_) => vec![
            "Cabinets, Countertops",
            "Cabinets, Doors & Drawers",
            "Cabinets, Front Indicators",
            "Cabinets, Module Lines",
            CABINET_LABEL_LAYER,
        ],
        ObjectRef::Device(_) => vec![ELECTRICAL_CONNECTION_LAYER],
        ObjectRef::Room(_) => vec!["Rooms", "Room Labels"],
        _ => Vec::new(),
    }
}

/// The primary and secondary layers of one object.
pub fn layers_of_object(cx: &EditorContext, o: ObjectRef) -> ObjectLayers {
    let mut out = ObjectLayers::default();
    if let Some(p) = layer_of(cx.floor(), o) {
        out.primary.push(p);
    }
    for name in secondary_candidates(cx, o) {
        if cx.project.layers.get(name).is_some() && !out.primary.iter().any(|p| p == name) {
            out.secondary.push(name.to_string());
        }
    }
    out
}

/// The primary and secondary layers of the selected objects: a layer that is
/// primary for any object is not listed as secondary.
pub fn layers_of_selection(cx: &EditorContext) -> ObjectLayers {
    let mut out = ObjectLayers::default();
    for o in &cx.selection.items {
        for p in layers_of_object(cx, *o).primary {
            if !out.primary.contains(&p) {
                out.primary.push(p);
            }
        }
    }
    for o in &cx.selection.items {
        for s in layers_of_object(cx, *o).secondary {
            if !out.primary.contains(&s) && !out.secondary.contains(&s) {
                out.secondary.push(s);
            }
        }
    }
    out
}

// ----- Layer Hider -----

/// Layer Hider: turns off the primary layer of `o` in the shown layer set (one
/// undo step) and returns its name. Nothing happens for an object without a
/// layer or on a layer that is already off.
pub fn hide_primary_layer(cx: &mut EditorContext, o: ObjectRef) -> Option<String> {
    let Some(layer) = layer_of(cx.floor(), o) else {
        cx.status = format!("A {} is not on a layer", o.type_name());
        return None;
    };
    if cx.project.layers.get(&layer).is_none() {
        cx.status = format!("There is no layer named {layer}");
        return None;
    }
    if !cx.layers().is_visible(&layer) {
        cx.status = format!("{layer} is already off");
        return None;
    }
    cx.begin_change("Layer Hider");
    cx.project.edit_layers(
        std::slice::from_ref(&layer),
        &LayerEdit::Display(false),
        false,
    );
    cx.mark_dirty();
    cx.refresh_layer_view();
    cx.selection.items.retain(|i| *i != o);
    cx.hover = None;
    cx.status = format!(
        "Layer Hider: {layer} is off in {}",
        cx.project.shown_layer_set()
    );
    Some(layer)
}

// ----- Object Layer Properties window -----

struct Window {
    panel: LayerPanelState,
    show_all: bool,
}

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

/// Is the Object Layer Properties window open?
pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Opens the window for the selection.
pub fn open(cx: &mut EditorContext) {
    if cx.selection.is_empty() {
        cx.status = "Select objects first".into();
        return;
    }
    let layers = layers_of_selection(cx);
    WINDOW.with(|w| {
        let mut w = w.borrow_mut();
        if w.is_none() {
            let mut panel = LayerPanelState {
                compact: true,
                ..LayerPanelState::default()
            };
            if let Some(first) = layers.primary.first() {
                panel.select_only(first);
            }
            *w = Some(Window {
                panel,
                show_all: false,
            });
        }
    });
}

/// Closes the window.
pub fn close() {
    WINDOW.with(|w| *w.borrow_mut() = None);
}

/// The Edit toolbar button for the selection: every object but the active
/// camera symbol and layout boxes has layers.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let has_layers = cx
        .selection
        .items
        .iter()
        .any(|o| !matches!(o, ObjectRef::Camera(_)));
    if !has_layers {
        return Vec::new();
    }
    let label = "Object Layer Properties";
    vec![EditAction {
        kind: EditActionKind::Custom {
            id: OPEN,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }]
}

/// Runs the module's commands.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    if id != OPEN {
        return false;
    }
    open(cx);
    true
}

/// Draws the window (called every frame by `layer_sets::show_all`).
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    show_window(ctx, cx);
}

fn show_window(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut w) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut close = false;
    let layers = layers_of_selection(cx);
    // The table lists the object's layers, or all of them.
    w.panel.only = if w.show_all { None } else { Some(layers.all()) };
    w.panel.heading = Some(format!(
        "Properties of Object Layers \u{2013} {}",
        cx.project.shown_layer_set()
    ));
    egui::Window::new("Object Layer Properties")
        .id(egui::Id::new("object_layer_properties"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(560.0, 460.0))
        .anchor(Align2::RIGHT_CENTER, Vec2::new(-12.0, 0.0))
        .show(ctx, |ui| {
            if cx.selection.is_empty() {
                ui.weak("Select an object to see its layers.");
            } else {
                ui.label(format!(
                    "Primary layer: {}",
                    if layers.primary.is_empty() {
                        "none".to_string()
                    } else {
                        layers.primary.join(", ")
                    }
                ));
                ui.label(format!(
                    "Secondary layers: {}",
                    if layers.secondary.is_empty() {
                        "none".to_string()
                    } else {
                        layers.secondary.join(", ")
                    }
                ))
                .on_hover_text(
                    "Secondary layers change how the object looks; they show nothing while the primary layer is off",
                );
            }
            ui.checkbox(&mut w.show_all, "Show All Layers");
            layer_panel(ui, cx, &mut w.panel, 0.5);
            ui.separator();
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if open && !close {
        WINDOW.with(|slot| *slot.borrow_mut() = Some(w));
    }
}

// ----- Find Objects on Layer(s), Select Location -----

/// Find Objects on Layer(s): with objects on one floor it goes there and
/// selects them; with objects on several it opens the Select Location
/// window (`select_location`) to choose the floor. Returns how many floors
/// have objects.
pub fn find_objects_on_layers(cx: &mut EditorContext, layers: &[String]) -> usize {
    use crate::dialogs::select_location::{ask_locations, Location, Target};
    let found = cx.project.find_objects_on_layers(layers);
    match found.len() {
        0 => {
            cx.status = "There are no objects on those layers".into();
        }
        1 => {
            go_to(cx, found[0].floor, layers);
        }
        _ => {
            let items = found
                .iter()
                .map(|f| Location {
                    title: cx
                        .project
                        .floors
                        .get(f.floor)
                        .map_or("?".to_string(), |x| x.name.clone()),
                    what: "Floor",
                    count: f.objects,
                    target: Target::LayerObjects {
                        floor: f.floor,
                        layers: layers.to_vec(),
                    },
                })
                .collect();
            ask_locations("Select Location", items);
        }
    }
    found.len()
}

/// Goes to `floor` and selects the objects on `layers` there. Returns how
/// many were selected (objects on layers that are off or locked cannot be).
pub fn go_to(cx: &mut EditorContext, floor: usize, layers: &[String]) -> usize {
    if floor >= cx.project.floors.len() {
        return 0;
    }
    if cx.floor != floor {
        cx.floor = floor;
        cx.reset_view_state();
        cx.refresh();
    }
    let n = select_all_on_layers(cx, layers);
    cx.status = if n > 0 {
        format!(
            "Found {n} object{} on {} layer{} on {}",
            if n == 1 { "" } else { "s" },
            layers.len(),
            if layers.len() == 1 { "" } else { "s" },
            cx.floor().name
        )
    } else {
        "The objects are on layers that are off or locked: turn the layers on to select them".into()
    };
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::cad::CadItem;
    use plan_core::geometry::Point;

    fn cx() -> EditorContext {
        crate::editor::plan_tabs::plain_cx()
    }

    fn circle() -> CadItem {
        CadItem::Circle {
            center: Point::ZERO,
            radius: 5.0,
        }
    }

    #[test]
    fn a_cabinet_lists_its_primary_and_secondary_layers() {
        let mut cx = cx();
        let c = crate::editor::placed::add_cabinet(
            &mut cx.project,
            0,
            plan_cabinets::Cabinet::base(24.0),
        )
        .unwrap();
        let l = layers_of_object(&cx, ObjectRef::Cabinet(c));
        assert_eq!(l.primary, vec!["Cabinets, Base".to_string()]);
        assert!(l.secondary.contains(&CABINET_LABEL_LAYER.to_string()));
        assert!(!l.secondary.contains(&"Cabinets, Base".to_string()));
    }

    #[test]
    fn a_wall_lists_the_wall_system_layers_the_plan_has() {
        let mut cx = cx();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Exterior,
        );
        let none = layers_of_object(&cx, ObjectRef::Wall(w));
        assert!(none.secondary.is_empty(), "the plan has none of them yet");
        cx.project.ensure_wall_system_layers();
        let l = layers_of_object(&cx, ObjectRef::Wall(w));
        assert_eq!(l.primary.len(), 1);
        assert!(l.secondary.contains(&WALL_LAYERS_LAYER.to_string()));
        assert!(l.secondary.contains(&WALL_MAIN_ONLY_LAYER.to_string()));
    }

    #[test]
    fn the_selection_lists_each_layer_once_primary_first() {
        let mut cx = cx();
        let a = cx.project.add_cad(0, "CAD, Default", circle());
        let b = cx.project.add_cad(0, "CAD, Default", circle());
        cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
        let l = layers_of_selection(&cx);
        assert_eq!(l.primary, vec!["CAD, Default".to_string()]);
        assert!(l.secondary.is_empty());
        assert_eq!(l.all(), vec!["CAD, Default".to_string()]);
    }

    #[test]
    fn the_layer_hider_turns_off_the_primary_layer_in_the_shown_set_only() {
        let mut cx = cx();
        let id = cx.project.add_cad(0, "CAD, Default", circle());
        let mut other = plan_core::layer_sets::LayerSetDef::new("Other");
        other.ensure_state("CAD, Default");
        cx.project.layer_sets.add_set(other);
        cx.refresh_layer_view();
        let shown = cx.project.shown_layer_set().to_string();
        assert_eq!(
            hide_primary_layer(&mut cx, ObjectRef::Cad(id)).as_deref(),
            Some("CAD, Default")
        );
        assert_eq!(cx.undo_label(), Some("Layer Hider"));
        assert!(!cx.layers().is_visible("CAD, Default"));
        let other_view = cx
            .project
            .layer_sets
            .effective_for("Other", &cx.project.layers);
        assert!(other_view.is_visible("CAD, Default"), "{shown} only");
        // Already off: nothing more to do, no second undo step.
        assert!(hide_primary_layer(&mut cx, ObjectRef::Cad(id)).is_none());
        cx.undo();
        assert!(cx.layers().is_visible("CAD, Default"));
    }

    #[test]
    fn find_objects_goes_to_the_one_floor_or_asks_for_a_location() {
        let mut cx = cx();
        cx.project
            .floors
            .push(plan_core::Floor::new("Upper", 108.0));
        cx.project.new_layer("Notes").unwrap();
        cx.project.add_cad(1, "Notes", circle());
        let layers = vec!["Notes".to_string()];
        assert_eq!(find_objects_on_layers(&mut cx, &layers), 1);
        assert_eq!(cx.floor, 1, "it went to the floor that has them");
        assert_eq!(cx.selection.len(), 1);
        cx.project.add_cad(0, "Notes", circle());
        assert_eq!(find_objects_on_layers(&mut cx, &layers), 2);
        assert!(crate::dialogs::select_location::is_open());
        assert_eq!(go_to(&mut cx, 0, &layers), 1);
        assert_eq!(cx.floor, 0);
        assert_eq!(find_objects_on_layers(&mut cx, &["Rooms".to_string()]), 0);
    }

    #[test]
    fn the_window_opens_for_a_selection_and_draws() {
        let mut cx = cx();
        open(&mut cx);
        assert!(!is_open(), "needs a selection");
        let id = cx.project.add_cad(0, "CAD, Default", circle());
        cx.selection.items = vec![ObjectRef::Cad(id)];
        assert_eq!(edit_actions(&cx).len(), 1);
        assert!(run_command(&mut cx, OPEN));
        assert!(is_open());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open());
        close();
        assert!(!is_open());
    }
}
