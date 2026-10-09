//! Door / Drawer / Side Panel Face Item Specification (reference manual
//! pp. 679-686): the settings of one face item. An item that has none of
//! them set follows the cabinet's Door/Drawer tab; the first change wraps it
//! in `FaceItem::Custom` (see `plan_cabinets::ItemProps`), and the wrapper
//! goes away again when everything is back to the default.
//!
//! Sections: Main Style (Slab, Framed or a Library style), Hardware with the
//! Hardware Size/Orientation controls, the Cabinet Shelf Specification for
//! doors and openings, Show Open, Panel Overlaps, Door Back Inserts, the
//! Drawer Box/Pullout insert, Reverse Appliance and Lock from Auto Resize.
//!
//! Everything here is plain functions on `FaceItem` so tests drive them
//! without a GUI; `item_spec_ui` is the egui page the Cabinet Specification
//! embeds below the face-item tree.

use super::cabinet_shelf::{shelf_spec_ui, ShelfContext};
use super::{row, section, Fields};
use crate::tools::library::door_styles::{self, LibraryStyle};
use eframe::egui::{self, Ui};
use plan_cabinets::{
    DoorProfile, DoorStyle, DrawerStyle, FaceItem, HandleStyle, HardwareSize, InsertOrder,
};

/// Which Face Item Specification dialog an item opens (the manual names
/// three: Door, Drawer and Side Panel).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecKind {
    Door,
    Drawer,
    SidePanel,
    /// Openings, rollouts, appliances, cutting boards: shelves, hardware and
    /// Show Open only.
    Other,
}

impl SpecKind {
    pub fn of(item: &FaceItem) -> SpecKind {
        if item.is_door() {
            SpecKind::Door
        } else if item.is_drawer() {
            SpecKind::Drawer
        } else if matches!(item.base(), FaceItem::Panel { .. } | FaceItem::Blank { .. }) {
            SpecKind::SidePanel
        } else {
            SpecKind::Other
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            SpecKind::Door => "Door Face Item Specification",
            SpecKind::Drawer => "Drawer Face Item Specification",
            SpecKind::SidePanel => "Side Panel Face Item Specification",
            SpecKind::Other => "Face Item Specification",
        }
    }
}

/// Where the main style of an item currently comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainStyle {
    /// Follows the cabinet's Door/Drawer panel.
    Cabinet,
    Slab,
    Framed,
    Library,
}

impl MainStyle {
    pub fn name(self) -> &'static str {
        match self {
            MainStyle::Cabinet => "Same as Cabinet",
            MainStyle::Slab => "Slab",
            MainStyle::Framed => "Framed",
            MainStyle::Library => "Library",
        }
    }
}

/// The main style of the item, read from its own door or drawer style.
pub fn main_style(item: &FaceItem) -> MainStyle {
    let Some(p) = item.props() else {
        return MainStyle::Cabinet;
    };
    let (profile, lib) = match (&p.door, &p.drawer) {
        (Some(d), _) => (d.profile, !d.library.is_empty()),
        (None, Some(d)) => (d.profile, !d.library.is_empty()),
        _ => return MainStyle::Cabinet,
    };
    if lib {
        MainStyle::Library
    } else if profile == DoorProfile::Slab {
        MainStyle::Slab
    } else {
        MainStyle::Framed
    }
}

/// Sets the item's main style. `door` and `drawer` are the cabinet's own
/// styles (the starting point); Library keeps the item's current look.
pub fn set_main_style(item: &mut FaceItem, to: MainStyle, door: &DoorStyle, drawer: &DrawerStyle) {
    let kind = SpecKind::of(item);
    let Some(p) = item.props_mut() else {
        return;
    };
    match to {
        MainStyle::Cabinet => {
            p.door = None;
            p.drawer = None;
        }
        _ => {
            if kind == SpecKind::Drawer {
                let mut d = p.drawer.clone().unwrap_or_else(|| drawer.clone());
                match to {
                    MainStyle::Slab => {
                        d.profile = DoorProfile::Slab;
                        d.library.clear();
                    }
                    MainStyle::Framed => {
                        if d.profile == DoorProfile::Slab {
                            d.profile = DoorProfile::Shaker;
                        }
                        d.library.clear();
                    }
                    _ => {}
                }
                p.drawer = Some(d);
            } else {
                let mut d = p.door.clone().unwrap_or_else(|| door.clone());
                match to {
                    MainStyle::Slab => {
                        d.profile = DoorProfile::Slab;
                        d.library.clear();
                    }
                    MainStyle::Framed => {
                        if d.profile == DoorProfile::Slab {
                            d.profile = DoorProfile::Shaker;
                        }
                        d.library.clear();
                    }
                    _ => {}
                }
                p.door = Some(d);
            }
        }
    }
    item.tidy();
}

/// Applies a Library style (category "Cabinet Doors") to the item.
pub fn apply_library_style(
    item: &mut FaceItem,
    style: &LibraryStyle,
    door: &DoorStyle,
    drawer: &DrawerStyle,
) {
    let kind = SpecKind::of(item);
    let Some(p) = item.props_mut() else {
        return;
    };
    if kind == SpecKind::Drawer {
        let mut d = p.drawer.clone().unwrap_or_else(|| drawer.clone());
        door_styles::apply_drawer(style, &mut d);
        p.drawer = Some(d);
    } else {
        let mut d = p.door.clone().unwrap_or_else(|| door.clone());
        door_styles::apply_door(style, &mut d);
        p.door = Some(d);
    }
}

/// Everything the page needs from the cabinet around the item.
pub struct FaceSpecContext<'a> {
    pub door: &'a DoorStyle,
    pub drawer: &'a DrawerStyle,
    pub styles: &'a [LibraryStyle],
    /// Height of the opening the item fills, inches (shelf placement).
    pub opening_height: f64,
    /// Wall cabinets have no roll-outs.
    pub allow_rollout: bool,
    /// Identifies the selected item (per-item UI state such as the Library
    /// mode of Main Style is remembered under it).
    pub key: u64,
}

fn handle_combo(ui: &mut Ui, salt: &str, value: &mut HandleStyle) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.name())
        .show_ui(ui, |ui| {
            for h in HandleStyle::ALL {
                ui.selectable_value(value, h, h.name());
            }
        });
}

/// Draws the Face Item Specification page for `item`. `shelf_sel` is the
/// selected manual shelf, kept by the caller. Returns true when the item
/// changed.
pub fn item_spec_ui(
    ui: &mut Ui,
    fields: &mut Fields,
    item: &mut FaceItem,
    cx: &FaceSpecContext<'_>,
    shelf_sel: &mut usize,
) -> bool {
    let before = item.clone();
    let kind = SpecKind::of(item);
    section(ui, kind.title());
    if matches!(item.base(), FaceItem::Separation { .. }) {
        ui.weak("A separation has no settings of its own.");
        return false;
    }

    // Main style: Same as Cabinet / Slab / Framed / Library.
    if matches!(
        kind,
        SpecKind::Door | SpecKind::Drawer | SpecKind::SidePanel
    ) {
        // Picking Library only chooses the mode; the item turns into a
        // library style once one is picked from the list below, so the mode
        // is remembered per item for the length of the dialog.
        let lib_id = ui.id().with(("fi_lib_mode", cx.key));
        let mut lib_mode: bool = ui.memory(|m| m.data.get_temp(lib_id)).unwrap_or(false);
        let cur = if lib_mode {
            MainStyle::Library
        } else {
            main_style(item)
        };
        let mut pick = cur;
        row(ui, "Main Style", |ui| {
            egui::ComboBox::from_id_salt("fi_main_style")
                .selected_text(cur.name())
                .show_ui(ui, |ui| {
                    for m in [
                        MainStyle::Cabinet,
                        MainStyle::Slab,
                        MainStyle::Framed,
                        MainStyle::Library,
                    ] {
                        ui.selectable_value(&mut pick, m, m.name());
                    }
                });
        });
        if pick != cur {
            lib_mode = pick == MainStyle::Library;
            ui.memory_mut(|m| m.data.insert_temp(lib_id, lib_mode));
            if !lib_mode {
                set_main_style(item, pick, cx.door, cx.drawer);
            }
        }
        if lib_mode || main_style(item) == MainStyle::Library {
            // The library lists drawer fronts with the doors.
            let styles = door_styles::doors(cx.styles);
            let mut chosen: Option<LibraryStyle> = None;
            row(ui, "Library Style", |ui| {
                egui::ComboBox::from_id_salt("fi_lib_style")
                    .selected_text(
                        item.props()
                            .and_then(|p| {
                                p.door
                                    .as_ref()
                                    .map(|d| d.name.clone())
                                    .or_else(|| p.drawer.as_ref().map(|d| d.name.clone()))
                            })
                            .unwrap_or_else(|| "(pick one)".to_string()),
                    )
                    .show_ui(ui, |ui| {
                        if styles.is_empty() {
                            ui.weak("No Cabinet Doors in the library");
                        }
                        for s in styles {
                            if ui.selectable_label(false, &s.name).clicked() {
                                chosen = Some((*s).clone());
                            }
                        }
                    });
            });
            if let Some(s) = chosen {
                apply_library_style(item, &s, cx.door, cx.drawer);
            }
        }
        // Panel profile detail for an item with a style of its own.
        if let Some(p) = item.props_mut() {
            if let Some(d) = p.door.as_mut() {
                if d.profile != DoorProfile::Slab {
                    fields.length_row(ui, "Stile and Rail Width", "fi_frame", &mut d.frame_width);
                }
                ui.checkbox(&mut d.glass, "Glass Door");
            }
            if let Some(d) = p.drawer.as_mut() {
                if d.profile != DoorProfile::Slab {
                    ui.weak("Framed drawer front");
                }
            }
        }
        item.tidy();
    }

    // Hardware.
    if matches!(kind, SpecKind::Door | SpecKind::Drawer) {
        section(ui, "Hardware");
        let cabinet_handle = if kind == SpecKind::Drawer {
            cx.drawer.handle
        } else {
            cx.door.handle
        };
        if let Some(p) = item.props_mut() {
            let mut handle = if kind == SpecKind::Drawer {
                p.drawer.as_ref().map_or(cabinet_handle, |d| d.handle)
            } else {
                p.door.as_ref().map_or(cabinet_handle, |d| d.handle)
            };
            let was = handle;
            row(ui, "Handle", |ui| {
                handle_combo(ui, "fi_handle", &mut handle)
            });
            if handle != was {
                if kind == SpecKind::Drawer {
                    p.drawer.get_or_insert_with(|| cx.drawer.clone()).handle = handle;
                } else {
                    p.door.get_or_insert_with(|| cx.door.clone()).handle = handle;
                }
            }
            let mut sized = p.hardware.is_some();
            if ui
                .checkbox(&mut sized, "Hardware Size/Orientation")
                .changed()
            {
                p.hardware = sized.then(HardwareSize::default);
            }
            if let Some(h) = p.hardware.as_mut() {
                let mut w = h.width;
                if fields.length_row(ui, "Width", "fi_hw_w", &mut w) && w > 0.0 {
                    h.set(0, w);
                }
                let mut ht = h.height;
                if fields.length_row(ui, "Height", "fi_hw_h", &mut ht) && ht > 0.0 {
                    h.set(1, ht);
                }
                let mut dp = h.depth;
                if fields.length_row(ui, "Depth", "fi_hw_d", &mut dp) && dp > 0.0 {
                    h.set(2, dp);
                }
                ui.checkbox(&mut h.retain_aspect, "Retain Aspect");
                ui.horizontal(|ui| {
                    ui.label(format!("Angle {:.0}\u{B0}", h.angle));
                    if ui.button("Rotate 90\u{B0}").clicked() {
                        h.rotate(90.0);
                    }
                    if ui.button("Rotate -90\u{B0}").clicked() {
                        h.rotate(-90.0);
                    }
                });
            }
        }
        item.tidy();
    }

    // Shelves (doors, openings, rollouts).
    if item.has_shelves() {
        if let Some(p) = item.props_mut() {
            shelf_spec_ui(
                ui,
                fields,
                &mut p.shelves,
                ShelfContext {
                    opening_height: cx.opening_height,
                    allow_rollout: cx.allow_rollout,
                },
                shelf_sel,
            );
        }
        item.tidy();
    }

    // Show Open (doors, drawers, rollouts).
    if item.is_door() || item.opens_as_drawer() || matches!(item.base(), FaceItem::Rollout { .. }) {
        section(ui, "Show Open");
        let is_door = item.is_door();
        if let Some(p) = item.props_mut() {
            if is_door {
                optional_number(ui, "Swing Angle", &mut p.swing_angle, 90.0, 0.0..=180.0);
            } else {
                optional_number(ui, "Percent Open", &mut p.percent_open, 50.0, 0.0..=100.0);
            }
        }
        item.tidy();
    }

    // Panel overlaps (frameless traditional overlay).
    if matches!(
        kind,
        SpecKind::Door | SpecKind::Drawer | SpecKind::SidePanel
    ) {
        section(ui, "Panel Overlaps");
        if let Some(p) = item.props_mut() {
            let mut own = p.overlap.is_some();
            if ui
                .checkbox(&mut own, "Specify overlaps for this item")
                .changed()
            {
                p.overlap = own.then_some([0.5; 4]);
            }
            if let Some(o) = p.overlap.as_mut() {
                for (i, name) in ["Left", "Right", "Top", "Bottom"].into_iter().enumerate() {
                    row(ui, name, |ui| {
                        ui.add(
                            egui::DragValue::new(&mut o[i])
                                .speed(0.0625)
                                .range(-1.0..=3.0)
                                .suffix("\""),
                        );
                    });
                }
            }
        }
        item.tidy();
    }

    // Door back inserts and the drawer box.
    if kind == SpecKind::Door {
        section(ui, "Door Back Inserts");
        if let Some(p) = item.props_mut() {
            let mut remove = None;
            for (i, ins) in p.inserts.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", i + 1));
                    ui.text_edit_singleline(ins);
                    if ui.button("Delete").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                p.inserts.remove(i);
            }
            if ui.button("Add Insert").clicked() {
                p.inserts.push(String::new());
            }
            row(ui, "Insert Order", |ui| {
                for o in [InsertOrder::TopToBottom, InsertOrder::BottomToTop] {
                    ui.radio_value(&mut p.insert_order, o, o.name());
                }
            });
        }
        item.tidy();
    }
    if item.opens_as_drawer() {
        section(ui, "Drawer Box/Pullout");
        if let Some(p) = item.props_mut() {
            row(ui, "Library Object", |ui| {
                ui.text_edit_singleline(&mut p.drawer_box);
            });
        }
        item.tidy();
    }
    if matches!(item.base(), FaceItem::Appliance { .. }) {
        section(ui, "Appliance");
        if let Some(p) = item.props_mut() {
            row(ui, "Library Object", |ui| {
                ui.text_edit_singleline(&mut p.library);
            });
            ui.checkbox(&mut p.reverse, "Reverse Appliance");
        }
        item.tidy();
    }

    // Lock from Auto Resize (every item).
    let mut locked = item.props().is_some_and(|p| p.locked);
    if ui.checkbox(&mut locked, "Lock from Auto Resize").changed() {
        if let Some(p) = item.props_mut() {
            p.locked = locked;
        }
        item.tidy();
    }
    *item != before
}

/// A "Specify" check followed by a drag value: `None` follows the cabinet.
fn optional_number(
    ui: &mut Ui,
    label: &str,
    value: &mut Option<f64>,
    start: f64,
    range: std::ops::RangeInclusive<f64>,
) {
    row(ui, label, |ui| {
        let mut own = value.is_some();
        if ui.checkbox(&mut own, "Specify").changed() {
            *value = own.then_some(start);
        }
        if let Some(v) = value.as_mut() {
            ui.add(egui::DragValue::new(v).range(range));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_kind_follows_the_item() {
        assert_eq!(
            SpecKind::of(&FaceItem::DoorAuto { height: 0.0 }),
            SpecKind::Door
        );
        assert_eq!(
            SpecKind::of(&FaceItem::FalseDrawer { height: 6.0 }),
            SpecKind::Drawer
        );
        assert_eq!(
            SpecKind::of(&FaceItem::Panel { height: 0.0 }),
            SpecKind::SidePanel
        );
        assert_eq!(
            SpecKind::of(&FaceItem::Rollout { height: 0.0 }),
            SpecKind::Other
        );
    }

    #[test]
    fn main_style_round_trips_and_unwraps() {
        let door = DoorStyle::default();
        let drawer = DrawerStyle::default();
        let mut it = FaceItem::DoorLeft { height: 0.0 };
        assert_eq!(main_style(&it), MainStyle::Cabinet);
        set_main_style(&mut it, MainStyle::Framed, &door, &drawer);
        assert_eq!(main_style(&it), MainStyle::Framed);
        assert!(matches!(it, FaceItem::Custom { .. }));
        set_main_style(&mut it, MainStyle::Slab, &door, &drawer);
        assert_eq!(main_style(&it), MainStyle::Slab);
        set_main_style(&mut it, MainStyle::Cabinet, &door, &drawer);
        assert_eq!(
            it,
            FaceItem::DoorLeft { height: 0.0 },
            "default settings unwrap"
        );
    }

    #[test]
    fn a_drawer_gets_a_drawer_style_not_a_door_style() {
        let door = DoorStyle::default();
        let drawer = DrawerStyle::default();
        let mut it = FaceItem::Drawer { height: 6.0 };
        set_main_style(&mut it, MainStyle::Framed, &door, &drawer);
        let p = it.props().unwrap();
        assert!(p.door.is_none());
        assert_eq!(p.drawer.as_ref().unwrap().profile, DoorProfile::Shaker);
        assert_eq!(it.height(), 6.0);
    }

    #[test]
    fn hardware_resize_retains_the_aspect_ratio() {
        let mut it = FaceItem::Drawer { height: 6.0 };
        let hw = it
            .props_mut()
            .unwrap()
            .hardware
            .get_or_insert_with(HardwareSize::default);
        hw.set(0, 8.0);
        assert!((hw.width - 8.0).abs() < 1e-9);
        assert!((hw.height - 1.5).abs() < 1e-9, "aspect kept: {hw:?}");
        hw.rotate(90.0);
        hw.rotate(-180.0);
        assert!((hw.angle - 270.0).abs() < 1e-9);
        // A drawer with hardware settings is wrapped and keeps its type.
        assert_eq!(it.kind(), plan_cabinets::ItemKind::Drawer);
        assert!(it.props().unwrap().hardware.is_some());
    }

    #[test]
    fn the_page_draws_for_every_leaf_kind() {
        let ctx = egui::Context::default();
        let door = DoorStyle::default();
        let drawer = DrawerStyle::default();
        let styles: Vec<LibraryStyle> = Vec::new();
        let cx = FaceSpecContext {
            door: &door,
            drawer: &drawer,
            styles: &styles,
            opening_height: 24.0,
            allow_rollout: true,
            key: 0,
        };
        let mut fields = Fields::default();
        let mut sel = 0;
        for k in plan_cabinets::ItemKind::ALL
            .into_iter()
            .filter(|k| k.is_leaf())
        {
            let mut item = k.make(0.0);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    item_spec_ui(ui, &mut fields, &mut item, &cx, &mut sel);
                });
            });
            assert_eq!(item.kind(), k, "drawing the page must not change the type");
        }
    }
}
