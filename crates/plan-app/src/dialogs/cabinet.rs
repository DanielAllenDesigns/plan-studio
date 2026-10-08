//! Cabinet Specification (docs/chief-x18-dialogs.md; CB-7, CB-10..CB-13).
//!
//! Tabs: General (size/position, countertop, backsplash, toe kick), Box
//! Construction, Front/Sides/Back (the face-item tree), Door/Drawer,
//! Accessories, Moldings (list only), Layer, Materials (list), Label. The
//! preview shows the plan symbol and a front elevation of the resolved face
//! items. The face-tree commands (Add, Delete, Move, Split Vertical/
//! Horizontal, Equalize) are plain functions on `plan_cabinets::FaceLayout`
//! addressed by a path of indices, so they test without a GUI.

// The shell opens this dialog; until it is wired the items are unused.
#![allow(dead_code)]

use super::{
    dis_check, dis_combo, dis_radio, fmt_short, off, on, pv_text, row, section, Fields, Outcome,
    SpecDialog, SpecPages, Tab, PV_GLASS, PV_INK, PV_WALL,
};
use crate::editor::placed::{cabinet_label, cabinet_layer};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use plan_cabinets::{
    plan_symbol, Backsplash, Cabinet, CabinetKind, Countertop, FaceCell, FaceItem, FaceLayout,
    HandleStyle, Overlay, ToeKick,
};
use plan_core::geometry::Point;

const TABS: &[Tab] = &[
    on("General"),
    on("Box Construction"),
    on("Front/Sides/Back"),
    on("Door/Drawer"),
    on("Accessories"),
    off("Opening Indicators"),
    on("Moldings"),
    on("Layer"),
    off("Fill Style"),
    on("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
];

// ----- the face-item tree -----

/// Indices from `FaceLayout::items` down: the first picks a top-level item,
/// each further one a cell of the `HorizontalLayout` before it.
pub type Path = Vec<usize>;

/// The kinds the Item Type list offers (a layout is made by splitting).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemType {
    Separation,
    Drawer,
    DoorAuto,
    DoorLeft,
    DoorRight,
    DoubleDoor,
    Opening,
    Appliance,
}

impl ItemType {
    pub const ALL: [ItemType; 8] = [
        ItemType::Separation,
        ItemType::Drawer,
        ItemType::DoorAuto,
        ItemType::DoorLeft,
        ItemType::DoorRight,
        ItemType::DoubleDoor,
        ItemType::Opening,
        ItemType::Appliance,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ItemType::Separation => "Separation",
            ItemType::Drawer => "Drawer",
            ItemType::DoorAuto => "Door - Auto",
            ItemType::DoorLeft => "Door - Left",
            ItemType::DoorRight => "Door - Right",
            ItemType::DoubleDoor => "Double Door",
            ItemType::Opening => "Opening",
            ItemType::Appliance => "Appliance",
        }
    }

    pub fn of(item: &FaceItem) -> Option<ItemType> {
        Some(match item {
            FaceItem::Separation { .. } => ItemType::Separation,
            FaceItem::Drawer { .. } => ItemType::Drawer,
            FaceItem::DoorAuto { .. } => ItemType::DoorAuto,
            FaceItem::DoorLeft { .. } => ItemType::DoorLeft,
            FaceItem::DoorRight { .. } => ItemType::DoorRight,
            FaceItem::DoubleDoor { .. } => ItemType::DoubleDoor,
            FaceItem::Opening { .. } => ItemType::Opening,
            FaceItem::Appliance { .. } => ItemType::Appliance,
            FaceItem::HorizontalLayout { .. } => return None,
        })
    }

    /// A new item of this type with `height` (0 = auto).
    pub fn make(self, height: f64) -> FaceItem {
        match self {
            ItemType::Separation => FaceItem::Separation { height },
            ItemType::Drawer => FaceItem::Drawer { height },
            ItemType::DoorAuto => FaceItem::DoorAuto { height },
            ItemType::DoorLeft => FaceItem::DoorLeft { height },
            ItemType::DoorRight => FaceItem::DoorRight { height },
            ItemType::DoubleDoor => FaceItem::DoubleDoor { height },
            ItemType::Opening => FaceItem::Opening { height },
            ItemType::Appliance => FaceItem::Appliance {
                height,
                name: "Appliance".to_string(),
            },
        }
    }
}

/// The tree line for an item.
pub fn describe(item: &FaceItem) -> String {
    match item {
        FaceItem::Appliance { name, .. } => format!("Appliance - {name}"),
        FaceItem::HorizontalLayout { .. } => "Layout - Horizontal".to_string(),
        other => ItemType::of(other)
            .map_or("Item", ItemType::label)
            .to_string(),
    }
}

pub fn item_at<'a>(layout: &'a FaceLayout, path: &[usize]) -> Option<&'a FaceItem> {
    let (first, rest) = path.split_first()?;
    let mut item = layout.items.get(*first)?;
    for &i in rest {
        match item {
            FaceItem::HorizontalLayout { cells, .. } => item = &cells.get(i)?.item,
            _ => return None,
        }
    }
    Some(item)
}

pub fn item_at_mut<'a>(layout: &'a mut FaceLayout, path: &[usize]) -> Option<&'a mut FaceItem> {
    let (first, rest) = path.split_first()?;
    let mut item = layout.items.get_mut(*first)?;
    for &i in rest {
        match item {
            FaceItem::HorizontalLayout { cells, .. } => item = &mut cells.get_mut(i)?.item,
            _ => return None,
        }
    }
    Some(item)
}

/// The cell at `path` when its container is a horizontal layout.
fn cell_at_mut<'a>(layout: &'a mut FaceLayout, path: &[usize]) -> Option<&'a mut FaceCell> {
    let (last, parent) = path.split_last()?;
    match item_at_mut(layout, parent)? {
        FaceItem::HorizontalLayout { cells, .. } => cells.get_mut(*last),
        _ => None,
    }
}

/// The list holding the item at `parent`'s children.
enum Cont<'a> {
    Items(&'a mut Vec<FaceItem>),
    Cells(&'a mut Vec<FaceCell>),
}

fn container<'a>(layout: &'a mut FaceLayout, parent: &[usize]) -> Option<Cont<'a>> {
    if parent.is_empty() {
        return Some(Cont::Items(&mut layout.items));
    }
    match item_at_mut(layout, parent)? {
        FaceItem::HorizontalLayout { cells, .. } => Some(Cont::Cells(cells)),
        _ => None,
    }
}

/// Add New: inserts `item` after the selected one (at the end for an empty
/// path). Returns the new item's path.
pub fn add_item(layout: &mut FaceLayout, path: &[usize], item: FaceItem) -> Option<Path> {
    let Some((idx, parent)) = path.split_last() else {
        layout.items.push(item);
        return Some(vec![layout.items.len() - 1]);
    };
    let at = idx + 1;
    match container(layout, parent)? {
        Cont::Items(v) => v.insert(at.min(v.len()), item),
        Cont::Cells(v) => v.insert(at.min(v.len()), FaceCell { item, width: None }),
    }
    let mut p = parent.to_vec();
    p.push(at);
    Some(p)
}

/// Delete: removes the item; a horizontal layout left with one cell becomes
/// that cell's item, with none an opening. Returns the path to select next.
pub fn delete_item(layout: &mut FaceLayout, path: &[usize]) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    let left = match container(layout, parent)? {
        Cont::Items(v) => {
            if *idx >= v.len() {
                return None;
            }
            v.remove(*idx);
            v.len()
        }
        Cont::Cells(v) => {
            if *idx >= v.len() {
                return None;
            }
            v.remove(*idx);
            v.len()
        }
    };
    if !parent.is_empty() && left <= 1 {
        if let Some(node) = item_at_mut(layout, parent) {
            let height = node.height();
            let replacement = match node {
                FaceItem::HorizontalLayout { cells, .. } if cells.len() == 1 => {
                    cells.remove(0).item.with_height(height)
                }
                _ => FaceItem::Opening { height },
            };
            *node = replacement;
        }
        return Some(parent.to_vec());
    }
    if left == 0 {
        return Some(Vec::new());
    }
    let mut p = parent.to_vec();
    p.push((*idx).min(left - 1));
    Some(p)
}

/// Split Vertical: stacks a copy of the item under it (the two share its
/// height). Only items of a vertical stack split this way.
pub fn split_vertical(layout: &mut FaceLayout, path: &[usize]) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    if !parent.is_empty() {
        return None;
    }
    let item = layout.items.get(*idx)?.clone();
    if matches!(item, FaceItem::Separation { .. }) {
        return None;
    }
    let half = item.height() / 2.0;
    let upper = if item.height() > 0.0 {
        item.with_height(half)
    } else {
        item.clone()
    };
    layout.items[*idx] = upper.clone();
    layout.items.insert(idx + 1, upper);
    Some(path.to_vec())
}

/// Split Horizontal: replaces the item by two side-by-side cells (or, inside a
/// horizontal layout, adds a neighbor cell). The first cell stays selected.
pub fn split_horizontal(layout: &mut FaceLayout, path: &[usize]) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    if !parent.is_empty() {
        let cell = cell_at_mut(layout, path)?;
        if matches!(cell.item, FaceItem::Separation { .. }) {
            return None;
        }
        let half = cell.width.map(|w| w / 2.0);
        cell.width = half;
        let twin = FaceCell {
            item: cell.item.clone(),
            width: half,
        };
        if let FaceItem::HorizontalLayout { cells, .. } = item_at_mut(layout, parent)? {
            cells.insert(idx + 1, twin);
        }
        return Some(path.to_vec());
    }
    let node = layout.items.get_mut(*idx)?;
    match node {
        FaceItem::Separation { .. } => None,
        FaceItem::HorizontalLayout { cells, .. } => {
            cells.push(FaceCell {
                item: FaceItem::Opening { height: 0.0 },
                width: None,
            });
            Some(vec![*idx, cells.len() - 1])
        }
        leaf => {
            let height = leaf.height();
            let half = leaf.with_height(0.0);
            *leaf = FaceItem::HorizontalLayout {
                height,
                cells: vec![
                    FaceCell {
                        item: half.clone(),
                        width: None,
                    },
                    FaceCell {
                        item: half,
                        width: None,
                    },
                ],
            };
            Some(vec![*idx, 0])
        }
    }
}

/// Equalize: gives the selected item's siblings equal shares (separations
/// keep their height); a selected horizontal layout equalizes its cells.
/// Returns whether anything changed.
pub fn equalize(layout: &mut FaceLayout, path: &[usize]) -> bool {
    let before = layout.clone();
    if let Some(FaceItem::HorizontalLayout { cells, .. }) = item_at_mut(layout, path) {
        for c in cells.iter_mut() {
            c.width = None;
        }
    } else if let Some((_, parent)) = path.split_last() {
        match container(layout, parent) {
            Some(Cont::Items(v)) => {
                for it in v.iter_mut() {
                    if !matches!(it, FaceItem::Separation { .. }) {
                        *it = it.with_height(0.0);
                    }
                }
            }
            Some(Cont::Cells(v)) => {
                for c in v.iter_mut() {
                    c.width = None;
                }
            }
            None => {}
        }
    }
    *layout != before
}

/// Move Up / Move Down within the container. Returns the new path.
pub fn move_item(layout: &mut FaceLayout, path: &[usize], up: bool) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    let other = if up { idx.checked_sub(1)? } else { idx + 1 };
    match container(layout, parent)? {
        Cont::Items(v) => {
            if other >= v.len() {
                return None;
            }
            v.swap(*idx, other);
        }
        Cont::Cells(v) => {
            if other >= v.len() {
                return None;
            }
            v.swap(*idx, other);
        }
    }
    let mut p = parent.to_vec();
    p.push(other);
    Some(p)
}

/// Changes the selected leaf to `ty`, keeping its height.
pub fn set_item_type(layout: &mut FaceLayout, path: &[usize], ty: ItemType) -> bool {
    match item_at_mut(layout, path) {
        Some(it) if !matches!(it, FaceItem::HorizontalLayout { .. }) => {
            let name = match it {
                FaceItem::Appliance { name, .. } => Some(name.clone()),
                _ => None,
            };
            let mut next = ty.make(it.height());
            if let (FaceItem::Appliance { name: n, .. }, Some(old)) = (&mut next, name) {
                *n = old;
            }
            *it = next;
            true
        }
        _ => false,
    }
}

/// `1`, `2.1`, ... as the Face Items tree numbers them.
fn number(path: &[usize]) -> String {
    path.iter()
        .map(|i| (i + 1).to_string())
        .collect::<Vec<_>>()
        .join(".")
}

// ----- the dialog -----

pub struct CabinetDialog {
    frame: SpecDialog,
    form: CabinetForm,
}

struct CabinetForm {
    draft: Cabinet,
    fields: Fields,
    sel: Path,
    new_type: ItemType,
}

impl CabinetDialog {
    pub fn new(cabinet: Cabinet) -> Self {
        Self {
            frame: SpecDialog::new("Cabinet Specification", "cabinet"),
            form: CabinetForm {
                draft: cabinet,
                fields: Fields::default(),
                sel: Vec::new(),
                new_type: ItemType::Drawer,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Cabinet {
        &self.form.draft
    }
}

fn handle_name(h: HandleStyle) -> &'static str {
    match h {
        HandleStyle::None => "None",
        HandleStyle::Knob => "Knob",
        HandleStyle::Pull => "Pull",
    }
}

fn handle_combo(ui: &mut Ui, salt: &str, value: &mut HandleStyle) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(handle_name(*value))
        .show_ui(ui, |ui| {
            for h in [HandleStyle::None, HandleStyle::Knob, HandleStyle::Pull] {
                ui.selectable_value(value, h, handle_name(h));
            }
        });
}

fn kind_name(k: CabinetKind) -> &'static str {
    match k {
        CabinetKind::Base => "Base",
        CabinetKind::Wall => "Wall",
        CabinetKind::FullHeight => "Full Height",
        CabinetKind::Soffit => "Soffit",
        CabinetKind::Shelf => "Shelf",
        CabinetKind::Partition => "Partition",
    }
}

impl CabinetForm {
    fn face_width(&self) -> f64 {
        self.draft.width
    }

    fn general(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Cabinet Style");
        row(ui, "Type", |ui| {
            dis_combo(ui, "cab_type", kind_name(d.kind))
        });
        dis_check(ui, "Treat As Filler", false);
        section(ui, "Size/Position");
        f.length_row(ui, "Width", "width", &mut d.width);
        f.length_row(ui, "Height (including countertop)", "height", &mut d.height);
        f.length_row(ui, "Depth", "depth", &mut d.depth);
        f.length_row(ui, "Finished Floor to Bottom", "elev", &mut d.elevation);
        row(ui, "Finished Floor to Top", |ui| {
            ui.label(fmt_short(d.elevation + d.height));
        });
        f.length_row(ui, "Position X (back left)", "pos_x", &mut d.position.x);
        f.length_row(ui, "Position Y (back left)", "pos_y", &mut d.position.y);
        let mut deg = d.angle.to_degrees();
        if f.degrees_row(ui, "Angle", "deg_angle", &mut deg) {
            d.angle = deg.to_radians();
        }

        section(ui, "Countertop");
        let mut has = d.countertop.is_some();
        if ui.checkbox(&mut has, "Countertop").changed() {
            d.countertop = has.then(Countertop::default);
        }
        if let Some(t) = d.countertop.as_mut() {
            f.length_row(ui, "Thickness", "ct_thick", &mut t.thickness);
            f.length_row(ui, "Overhang Front", "ct_front", &mut t.overhang_front);
            f.length_row(ui, "Overhang Back", "ct_back", &mut t.overhang_back);
            f.length_row(ui, "Overhang Sides", "ct_sides", &mut t.overhang_sides);
            row(ui, "Corner Treatment", |ui| {
                dis_radio(ui, "None", true);
                dis_radio(ui, "Clipped", false);
                dis_radio(ui, "Rounded", false);
            });
        }

        section(ui, "Backsplash");
        let mut has = d.backsplash.is_some();
        if ui.checkbox(&mut has, "Backsplash").changed() {
            d.backsplash = has.then_some(Backsplash {
                height: 4.0,
                thickness: 0.5,
            });
        }
        if let Some(b) = d.backsplash.as_mut() {
            f.length_row(ui, "Height", "bs_height", &mut b.height);
            f.length_row(ui, "Thickness", "bs_thick", &mut b.thickness);
        }

        section(ui, "Toe Kick");
        let mut has = d.toe_kick.is_some();
        if ui.checkbox(&mut has, "Toe Kick").changed() {
            d.toe_kick = has.then(ToeKick::default);
        }
        if let Some(t) = d.toe_kick.as_mut() {
            f.length_row(ui, "Height", "tk_height", &mut t.height);
            f.length_row(ui, "Depth", "tk_depth", &mut t.depth);
        }
    }

    fn box_construction(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Box Construction");
        ui.horizontal(|ui| {
            ui.radio_value(&mut d.framed, true, "Framed");
            ui.radio_value(&mut d.framed, false, "Frameless");
        });
        if d.framed {
            f.length_row(ui, "Separation", "frame_width", &mut d.face.frame_width);
        }
        section(ui, "Top/Bottom/Sides");
        row(ui, "Top", |ui| {
            dis_radio(ui, "Auto", true);
            dis_radio(ui, "Has Top", false);
            dis_radio(ui, "No Top", false);
        });
        row(ui, "Bottom", |ui| {
            dis_radio(ui, "Auto", true);
            dis_radio(ui, "Has Bottom", false);
            dis_radio(ui, "No Bottom", false);
        });
        row(ui, "Side / Back Thickness", |ui| {
            ui.label(fmt_short(0.75));
        });
        section(ui, "Door/Drawer Overlay");
        let (mut trad, mut full, mut inset) = (false, false, false);
        let mut value = match d.overlay {
            Overlay::Traditional { overlap } => {
                trad = true;
                overlap
            }
            Overlay::Full { reveal } => {
                full = true;
                reveal
            }
            Overlay::Inset { clearance } => {
                inset = true;
                clearance
            }
        };
        let mut next = None;
        ui.horizontal(|ui| {
            if ui.radio(trad, "Traditional Overlay").clicked() {
                next = Some(Overlay::Traditional { overlap: 0.375 });
            }
            if ui.radio(full, "Full Overlay").clicked() {
                next = Some(Overlay::Full { reveal: 0.0625 });
            }
            if ui.radio(inset, "Inset").clicked() {
                next = Some(Overlay::Inset { clearance: 0.0625 });
            }
        });
        if let Some(n) = next {
            d.overlay = n;
            value = match n {
                Overlay::Traditional { overlap } => overlap,
                Overlay::Full { reveal } => reveal,
                Overlay::Inset { clearance } => clearance,
            };
        }
        let label = match d.overlay {
            Overlay::Traditional { .. } => "Overlap",
            Overlay::Full { .. } => "Reveal",
            Overlay::Inset { .. } => "Clearance",
        };
        if f.length_row(ui, label, "overlay", &mut value) {
            d.overlay = match d.overlay {
                Overlay::Traditional { .. } => Overlay::Traditional { overlap: value },
                Overlay::Full { .. } => Overlay::Full { reveal: value },
                Overlay::Inset { .. } => Overlay::Inset { clearance: value },
            };
        }
    }

    fn front(&mut self, ui: &mut Ui) {
        section(ui, "Cabinet Side");
        row(ui, "Side", |ui| dis_combo(ui, "cab_side", "Front"));
        row(ui, "Side Type", |ui| {
            dis_combo(ui, "cab_side_type", "Custom Face")
        });
        section(ui, "Face Items");
        let layout = &mut self.draft.face;
        let mut sel = std::mem::take(&mut self.sel);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if layout.items.is_empty() {
                ui.weak("(no face items)");
            }
            for (i, item) in layout.items.iter().enumerate() {
                tree_ui(ui, item, &mut vec![i], &mut sel);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let add_ty = self.new_type;
            egui::ComboBox::from_id_salt("cab_new_type")
                .selected_text(add_ty.label())
                .show_ui(ui, |ui| {
                    for t in ItemType::ALL {
                        ui.selectable_value(&mut self.new_type, t, t.label());
                    }
                });
            if ui.button("Add New").clicked() {
                if let Some(p) = add_item(layout, &sel, add_ty.make(0.0)) {
                    sel = p;
                }
            }
            let has = !sel.is_empty() && item_at(layout, &sel).is_some();
            if ui.add_enabled(has, egui::Button::new("Delete")).clicked() {
                if let Some(p) = delete_item(layout, &sel) {
                    sel = p;
                }
            }
            if ui.add_enabled(has, egui::Button::new("Move Up")).clicked() {
                if let Some(p) = move_item(layout, &sel, true) {
                    sel = p;
                }
            }
            if ui
                .add_enabled(has, egui::Button::new("Move Down"))
                .clicked()
            {
                if let Some(p) = move_item(layout, &sel, false) {
                    sel = p;
                }
            }
            if ui
                .add_enabled(has, egui::Button::new("Split Vertical"))
                .clicked()
            {
                if let Some(p) = split_vertical(layout, &sel) {
                    sel = p;
                }
            }
            if ui
                .add_enabled(has, egui::Button::new("Split Horizontal"))
                .clicked()
            {
                if let Some(p) = split_horizontal(layout, &sel) {
                    sel = p;
                }
            }
            if ui.add_enabled(has, egui::Button::new("Equalize")).clicked() {
                equalize(layout, &sel);
            }
        });
        ui.horizontal(|ui| {
            let face_h = self.draft.face_height();
            if ui.button("Reset to Default Face").clicked() {
                self.draft.face = match self.draft.kind {
                    CabinetKind::Wall => FaceLayout::wall_default(face_h),
                    CabinetKind::FullHeight => FaceLayout::full_height_default(face_h),
                    _ => FaceLayout::base_default(face_h),
                };
                sel.clear();
            }
            if ui.button("Sink Base Face").clicked() {
                self.draft.face = FaceLayout::sink_base();
                sel.clear();
            }
        });
        self.selected_properties(ui, &mut sel);
        self.sel = sel;
    }

    fn selected_properties(&mut self, ui: &mut Ui, sel: &mut Path) {
        section(ui, "Selected Item Properties");
        if item_at(&self.draft.face, sel).is_none() {
            ui.weak("Select a face item above.");
            return;
        }
        let in_cell = sel.len() > 1;
        let Some(item) = item_at(&self.draft.face, sel).cloned() else {
            return;
        };
        match ItemType::of(&item) {
            Some(cur) => {
                let mut ty = cur;
                row(ui, "Item Type", |ui| {
                    egui::ComboBox::from_id_salt("cab_item_type")
                        .selected_text(ty.label())
                        .show_ui(ui, |ui| {
                            for t in ItemType::ALL {
                                ui.selectable_value(&mut ty, t, t.label());
                            }
                        });
                });
                if ty != cur {
                    set_item_type(&mut self.draft.face, sel, ty);
                }
            }
            None => {
                row(ui, "Item Type", |ui| ui.label("Layout - Horizontal"));
            }
        }
        let mut h = item.height();
        if !in_cell
            && self
                .fields
                .length_row(ui, "Item Height (0 = auto)", "item_h", &mut h)
            && h >= 0.0
        {
            if let Some(it) = item_at_mut(&mut self.draft.face, sel) {
                *it = it.with_height(h);
            }
        }
        if in_cell {
            if let Some(cell) = cell_at_mut(&mut self.draft.face, sel) {
                let mut auto = cell.width.is_none();
                if ui.checkbox(&mut auto, "Auto width").changed() {
                    cell.width = if auto { None } else { Some(12.0) };
                }
                if let Some(w) = cell.width.as_mut() {
                    self.fields.length_row(ui, "Item Width", "item_w", w);
                }
            }
        }
        if let Some(FaceItem::Appliance { name, .. }) = item_at_mut(&mut self.draft.face, sel) {
            row(ui, "Appliance", |ui| {
                ui.text_edit_singleline(name);
            });
        }
        dis_check(ui, "Lock from Auto-Resize", false);
    }

    fn door_drawer(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Door Panel");
        row(ui, "Main Style", |ui| {
            ui.text_edit_singleline(&mut d.door_style.name);
        });
        f.length_row(ui, "Thickness", "door_thick", &mut d.door_style.thickness);
        ui.checkbox(&mut d.door_style.glass, "Glass Doors");
        section(ui, "Door Handle");
        row(ui, "Main Style", |ui| {
            handle_combo(ui, "door_handle", &mut d.door_style.handle);
        });
        f.length_row(
            ui,
            "Distance From Top",
            "door_h_top",
            &mut d.door_style.handle_from_top,
        );
        f.length_row(
            ui,
            "Distance From Edge",
            "door_h_edge",
            &mut d.door_style.handle_from_edge,
        );
        section(ui, "Drawer Panel");
        row(ui, "Main Style", |ui| {
            ui.text_edit_singleline(&mut d.drawer_style.name);
        });
        f.length_row(
            ui,
            "Thickness",
            "drawer_thick",
            &mut d.drawer_style.thickness,
        );
        section(ui, "Drawer Handle");
        row(ui, "Main Style", |ui| {
            handle_combo(ui, "drawer_handle", &mut d.drawer_style.handle);
        });
    }

    fn accessories(ui: &mut Ui) {
        section(ui, "Front Pilasters");
        row(ui, "Front Pilaster", |ui| {
            dis_combo(ui, "acc_pilaster", "None")
        });
        section(ui, "Feet");
        row(ui, "Foot Style", |ui| dis_combo(ui, "acc_feet", "None"));
        section(ui, "Side Panels");
        row(ui, "Main Panel Style", |ui| {
            dis_combo(ui, "acc_panel", "Slab Panels")
        });
        dis_check(ui, "Full Size Panel", true);
        ui.weak("Accessories are not stored in the model yet.");
    }

    fn moldings(ui: &mut Ui) {
        section(ui, "Profiles");
        ui.weak("(no moldings)");
        ui.horizontal(|ui| {
            for b in ["Add New...", "Make Copy", "Edit...", "Delete"] {
                ui.add_enabled(false, egui::Button::new(b));
            }
        });
        ui.weak("Moldings are listed here once the model stores them.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            dis_combo(ui, "cab_layer", cabinet_layer(self.draft.kind));
        });
        ui.weak("Cabinets sit on the layer of their type.");
    }

    fn materials(ui: &mut Ui) {
        section(ui, "Materials");
        for part in [
            "Box",
            "Door Fronts",
            "Drawer Fronts",
            "Countertop",
            "Backsplash",
            "Toe Kick",
        ] {
            row(ui, part, |ui| dis_combo(ui, part, "Default"));
        }
        ui.weak("Materials are applied by style until the model stores them.");
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        let auto = plan_cabinets::auto_label(&self.draft);
        row(ui, "Automatic label", |ui| ui.label(auto));
        let mut specify = !self.draft.label.is_empty();
        if ui.checkbox(&mut specify, "Specify label").changed() {
            self.draft.label = if specify {
                plan_cabinets::auto_label(&self.draft)
            } else {
                String::new()
            };
        }
        if specify {
            row(ui, "Label", |ui| {
                ui.text_edit_singleline(&mut self.draft.label);
            });
        }
    }
}

/// One line of the face-item tree and its children.
fn tree_ui(ui: &mut Ui, item: &FaceItem, path: &mut Path, sel: &mut Path) {
    let text = format!("{}  {}", number(path), describe(item));
    if ui.selectable_label(sel == path, text).clicked() {
        *sel = path.clone();
    }
    if let FaceItem::HorizontalLayout { cells, .. } = item {
        ui.indent(("cab_tree", number(path)), |ui| {
            for (i, c) in cells.iter().enumerate() {
                path.push(i);
                tree_ui(ui, &c.item, path, sel);
                path.pop();
            }
        });
    }
}

impl SpecPages for CabinetForm {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = &self.draft;
        if d.width < 1.0 || d.depth <= 0.0 || d.height <= 0.0 {
            return Some("Width, depth and height must be positive".into());
        }
        d.face
            .resolve(d.face_height(), self.face_width())
            .err()
            .map(|e| format!("Face items: {e}"))
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS.get(tab).map(|t| t.name) {
            Some("General") => self.general(ui),
            Some("Box Construction") => self.box_construction(ui),
            Some("Front/Sides/Back") => self.front(ui),
            Some("Door/Drawer") => self.door_drawer(ui),
            Some("Accessories") => Self::accessories(ui),
            Some("Moldings") => Self::moldings(ui),
            Some("Layer") => self.layer(ui),
            Some("Materials") => Self::materials(ui),
            Some("Label") => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        draw_preview(painter, rect, &self.draft);
    }
}

// ----- preview -----

/// Maps plan/elevation inches into `area`, y up, keeping the aspect.
struct Fit {
    origin: Pos2,
    scale: f32,
    min: Point,
}

impl Fit {
    fn new(area: Rect, min: Point, max: Point) -> Fit {
        let (w, h) = (
            (max.x - min.x).max(1.0) as f32,
            (max.y - min.y).max(1.0) as f32,
        );
        let scale = (area.width() / w).min(area.height() / h);
        let size = Vec2::new(w * scale, h * scale);
        let tl = area.center() - size * 0.5;
        Fit {
            origin: Pos2::new(tl.x, tl.y + size.y),
            scale,
            min,
        }
    }

    fn pt(&self, p: Point) -> Pos2 {
        Pos2::new(
            self.origin.x + (p.x - self.min.x) as f32 * self.scale,
            self.origin.y - (p.y - self.min.y) as f32 * self.scale,
        )
    }

    fn rect(&self, x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::from_two_pos(self.pt(Point::new(x, y)), self.pt(Point::new(x + w, y + h)))
    }
}

fn draw_preview(p: &Painter, area: Rect, cab: &Cabinet) {
    let split = area.min.y + area.height() * 0.4;
    let plan = Rect::from_min_max(area.min, Pos2::new(area.max.x, split - 4.0));
    let front = Rect::from_min_max(Pos2::new(area.min.x, split + 4.0), area.max);
    draw_plan_preview(p, plan, cab);
    draw_front_preview(p, front, cab);
}

fn draw_plan_preview(p: &Painter, area: Rect, cab: &Cabinet) {
    let mut c = cab.clone();
    c.position = Point::ZERO;
    c.angle = 0.0;
    let strokes = plan_symbol(&c);
    let ink = Stroke::new(1.0_f32, PV_INK);
    let mut pts: Vec<Point> = Vec::new();
    for s in &strokes {
        match s {
            plan_cabinets::Stroke::Line(a, b) => pts.extend([*a, *b]),
            plan_cabinets::Stroke::Polyline(v, _) => pts.extend(v.iter().copied()),
            _ => {}
        }
    }
    let area = area.shrink2(Vec2::new(4.0, 10.0));
    let (mut lo, mut hi) = (
        Point::new(f64::MAX, f64::MAX),
        Point::new(f64::MIN, f64::MIN),
    );
    for q in &pts {
        lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
    }
    if pts.is_empty() {
        return;
    }
    let fit = Fit::new(area, lo, hi);
    for s in &strokes {
        match s {
            plan_cabinets::Stroke::Line(a, b) => {
                p.line_segment([fit.pt(*a), fit.pt(*b)], ink);
            }
            plan_cabinets::Stroke::Polyline(v, closed) => {
                let mut sp: Vec<Pos2> = v.iter().map(|q| fit.pt(*q)).collect();
                if *closed {
                    if let Some(first) = sp.first().copied() {
                        sp.push(first);
                    }
                }
                p.add(egui::Shape::line(sp, ink));
            }
            plan_cabinets::Stroke::Text { at, text, .. } => {
                pv_text(p, fit.pt(*at), Align2::CENTER_CENTER, text, 10.0);
            }
            plan_cabinets::Stroke::Arc { .. } => {}
        }
    }
    pv_text(
        p,
        Pos2::new(area.center().x, area.min.y - 6.0),
        Align2::CENTER_CENTER,
        format!("Plan  {} x {}", fmt_short(cab.width), fmt_short(cab.depth)),
        10.0,
    );
}

fn knob(p: &Painter, at: Pos2) {
    p.circle_filled(at, 1.8, PV_INK);
}

fn draw_front_preview(p: &Painter, area: Rect, cab: &Cabinet) {
    let area = area.shrink2(Vec2::new(4.0, 12.0));
    let (w, h) = (cab.width.max(1.0), cab.height.max(1.0));
    let fit = Fit::new(area, Point::ZERO, Point::new(w, h));
    let ink = Stroke::new(1.0_f32, PV_INK);
    let toe = cab.toe_kick.map_or(0.0, |t| t.height);
    let top = cab.countertop.map_or(0.0, |t| t.thickness);
    let body = fit.rect(0.0, toe, w, h - toe - top);
    p.rect_stroke(body, 0.0, ink, StrokeKind::Inside);
    if toe > 0.0 {
        let r = fit.rect(2.0, 0.0, w - 4.0, toe);
        p.rect_filled(r, 0.0, PV_WALL);
        p.rect_stroke(r, 0.0, ink, StrokeKind::Inside);
    }
    if top > 0.0 {
        let r = fit.rect(0.0, h - top, w, top);
        p.rect_filled(r, 0.0, PV_WALL);
        p.rect_stroke(r, 0.0, ink, StrokeKind::Inside);
    }
    match cab.face.resolve(cab.face_height(), w) {
        Ok(items) => {
            for it in items {
                let (x, y, iw, ih) = it.rect;
                let r = fit.rect(x, toe + y, iw, ih);
                match &it.item {
                    FaceItem::Separation { .. } => {
                        p.rect_filled(r, 0.0, PV_WALL);
                    }
                    FaceItem::Opening { .. } => {
                        p.rect_filled(r, 0.0, Color32::from_gray(200));
                        p.rect_stroke(r, 0.0, ink, StrokeKind::Inside);
                    }
                    FaceItem::Appliance { name, .. } => {
                        p.rect_filled(r, 0.0, PV_GLASS);
                        p.rect_stroke(r, 0.0, ink, StrokeKind::Inside);
                        pv_text(p, r.center(), Align2::CENTER_CENTER, name, 9.0);
                    }
                    FaceItem::Drawer { .. } => {
                        p.rect_stroke(r.shrink(0.5), 0.0, ink, StrokeKind::Inside);
                        knob(p, r.center());
                    }
                    FaceItem::DoubleDoor { .. } => {
                        p.rect_stroke(r.shrink(0.5), 0.0, ink, StrokeKind::Inside);
                        p.line_segment([r.center_top(), r.center_bottom()], ink);
                        knob(p, Pos2::new(r.center().x - 3.0, r.center().y));
                        knob(p, Pos2::new(r.center().x + 3.0, r.center().y));
                    }
                    other => {
                        p.rect_stroke(r.shrink(0.5), 0.0, ink, StrokeKind::Inside);
                        let right_hinged = matches!(other, FaceItem::DoorRight { .. });
                        let x = if right_hinged {
                            r.min.x + 4.0
                        } else {
                            r.max.x - 4.0
                        };
                        knob(p, Pos2::new(x, r.center().y));
                    }
                }
            }
        }
        Err(e) => {
            pv_text(p, body.center(), Align2::CENTER_CENTER, e, 9.0);
        }
    }
    pv_text(
        p,
        Pos2::new(area.center().x, area.min.y - 8.0),
        Align2::CENTER_CENTER,
        format!("Front  {}  {}", fmt_short(cab.height), cabinet_label(cab)),
        10.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> FaceLayout {
        FaceLayout::base_default(34.5)
    }

    #[test]
    fn add_inserts_after_the_selection_and_at_the_end() {
        let mut l = base();
        let p = add_item(&mut l, &[1], FaceItem::Drawer { height: 0.0 }).unwrap();
        assert_eq!(p, vec![2]);
        assert_eq!(l.items.len(), 6);
        assert_eq!(l.items[2], FaceItem::Drawer { height: 0.0 });
        let p = add_item(&mut l, &[], FaceItem::Opening { height: 0.0 }).unwrap();
        assert_eq!(p, vec![6]);
        assert!(add_item(&mut l, &[99], FaceItem::Opening { height: 0.0 }).is_some());
    }

    #[test]
    fn delete_removes_and_reselects() {
        let mut l = base();
        assert_eq!(describe(item_at(&l, &[3]).unwrap()), "Door - Auto");
        let sel = delete_item(&mut l, &[3]).unwrap();
        assert_eq!(l.items.len(), 4);
        assert_eq!(sel, vec![3]);
        let sel = delete_item(&mut l, &[3]).unwrap();
        assert_eq!(sel, vec![2]);
        assert!(delete_item(&mut l, &[9]).is_none());
    }

    #[test]
    fn split_vertical_stacks_a_copy_and_halves_the_height() {
        let mut l = FaceLayout {
            items: vec![FaceItem::Drawer { height: 12.0 }],
            frame_width: 1.5,
        };
        assert_eq!(split_vertical(&mut l, &[0]), Some(vec![0]));
        assert_eq!(l.items.len(), 2);
        assert_eq!(l.items[0].height(), 6.0);
        assert_eq!(l.items[1].height(), 6.0);
        assert!(l.resolve(12.0, 24.0).is_ok());
        // Separations do not split; neither do cells.
        let mut sep = base();
        assert!(split_vertical(&mut sep, &[0]).is_none());
    }

    #[test]
    fn split_horizontal_makes_side_by_side_cells() {
        let mut l = base();
        // Split the auto door (index 3).
        let sel = split_horizontal(&mut l, &[3]).unwrap();
        assert_eq!(sel, vec![3, 0]);
        let FaceItem::HorizontalLayout { cells, height } = &l.items[3] else {
            panic!("expected a layout");
        };
        assert_eq!((cells.len(), *height), (2, 0.0));
        let r = l.resolve(34.5, 24.0).unwrap();
        // Separation, drawer, separation, two doors, separation.
        assert_eq!(r.len(), 6);
        let doors: Vec<_> = r
            .iter()
            .filter(|f| matches!(f.item, FaceItem::DoorAuto { .. }))
            .collect();
        assert_eq!(doors.len(), 2);
        assert!((doors[0].rect.2 - 12.0).abs() < 1e-9 && (doors[1].rect.0 - 12.0).abs() < 1e-9);
        // Splitting a cell adds a neighbor with half the fixed width.
        if let Some(c) = cell_at_mut(&mut l, &[3, 0]) {
            c.width = Some(10.0);
        }
        split_horizontal(&mut l, &[3, 0]).unwrap();
        let FaceItem::HorizontalLayout { cells, .. } = &l.items[3] else {
            panic!();
        };
        assert_eq!(cells.len(), 3);
        assert_eq!((cells[0].width, cells[1].width), (Some(5.0), Some(5.0)));
        // Splitting a layout appends a cell; separations refuse.
        let sel = split_horizontal(&mut l, &[3]).unwrap();
        assert_eq!(sel, vec![3, 3]);
        assert!(split_horizontal(&mut l, &[0]).is_none());
    }

    #[test]
    fn deleting_cells_collapses_the_layout() {
        let mut l = base();
        split_horizontal(&mut l, &[3]).unwrap();
        let sel = delete_item(&mut l, &[3, 1]).unwrap();
        assert_eq!(sel, vec![3]);
        assert_eq!(l.items[3], FaceItem::DoorAuto { height: 0.0 });
    }

    #[test]
    fn equalize_resets_sibling_heights_but_keeps_separations() {
        let mut l = FaceLayout {
            items: vec![
                FaceItem::Separation { height: 1.5 },
                FaceItem::Drawer { height: 6.0 },
                FaceItem::Drawer { height: 10.0 },
                FaceItem::Separation { height: 1.5 },
            ],
            frame_width: 1.5,
        };
        assert!(equalize(&mut l, &[1]));
        assert_eq!(l.items[1].height(), 0.0);
        assert_eq!(l.items[2].height(), 0.0);
        assert_eq!(l.items[0].height(), 1.5);
        let r = l.resolve(30.0, 24.0).unwrap();
        assert!((r[1].rect.3 - 13.5).abs() < 1e-9 && (r[2].rect.3 - 13.5).abs() < 1e-9);
        assert!(!equalize(&mut l, &[1]), "already equal");
        // A layout equalizes its cells.
        let mut l = base();
        split_horizontal(&mut l, &[3]).unwrap();
        if let Some(c) = cell_at_mut(&mut l, &[3, 0]) {
            c.width = Some(5.0);
        }
        assert!(equalize(&mut l, &[3]));
        let r = l.resolve(34.5, 24.0).unwrap();
        let doors: Vec<_> = r
            .iter()
            .filter(|f| matches!(f.item, FaceItem::DoorAuto { .. }))
            .collect();
        assert!((doors[0].rect.2 - doors[1].rect.2).abs() < 1e-9);
    }

    #[test]
    fn move_and_retype() {
        let mut l = base();
        assert_eq!(move_item(&mut l, &[1], false), Some(vec![2]));
        assert_eq!(describe(&l.items[2]), "Drawer");
        assert_eq!(move_item(&mut l, &[0], true), None);
        assert!(set_item_type(&mut l, &[2], ItemType::Appliance));
        assert_eq!(
            l.items[2],
            FaceItem::Appliance {
                height: 6.0,
                name: "Appliance".into()
            }
        );
        // Retyping an appliance keeps its name.
        if let FaceItem::Appliance { name, .. } = &mut l.items[2] {
            *name = "Dishwasher".into();
        }
        set_item_type(&mut l, &[2], ItemType::Appliance);
        assert_eq!(describe(&l.items[2]), "Appliance - Dishwasher");
        assert!(l.resolve(34.5, 24.0).is_ok());
    }

    #[test]
    fn the_dialog_validates_and_previews() {
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        assert!(dlg.form.error().is_none());
        dlg.form.draft.height = 8.0; // too short for the fixed face items
        assert!(dlg.form.error().unwrap().starts_with("Face items"));
        dlg.form.draft.height = 36.0;
        dlg.form.draft.width = 0.0;
        assert!(dlg.form.error().is_some());
        dlg.form.draft.width = 30.0;
        assert_eq!(dlg.draft().width, 30.0);
        assert_eq!(TABS.iter().filter(|t| t.enabled).count(), 9);

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let mut dlg = CabinetDialog::new(Cabinet::sink_base(36.0));
            dlg.form.sel = vec![1];
            let _ = dlg.show(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                for tab in 0..TABS.len() {
                    dlg.form.page(ui, tab);
                }
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(220.0, 400.0), egui::Sense::hover());
                dlg.form.preview(&painter, painter.clip_rect());
                let mut wall = Cabinet::wall(30.0);
                wall.face = FaceLayout::wall_default(30.0);
                draw_preview(&painter, painter.clip_rect(), &wall);
            });
        });
    }
}
