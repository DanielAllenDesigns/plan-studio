//! Cabinet Specification (docs/chief-x18-dialogs.md; CB-7, CB-10..CB-13).
//!
//! Tabs: General (size/position, countertop with sink and cooktop cutouts,
//! backsplash, toe kick, plus the corner, blind, appliance-opening and
//! custom-top settings of those kinds), Box Construction, Front/Sides/Back
//! (the face-item tree and a front elevation whose dividers drag),
//! Door/Drawer (built-in slab, shaker and raised styles and the library's
//! "Cabinet Doors", handles, hinges), Accessories (front pilasters, feet,
//! finished end panels), Opening Indicators, Moldings (crown and light
//! rail), Layer, Fill Style (plan hatch), Materials (per part), Label,
//! Components (the parts with counts, sizes and materials), Object
//! Information and Schedule. The preview shows the plan symbol and a
//! front elevation of the resolved face items. The face-tree commands (Add,
//! Delete, Move, Split Vertical/Horizontal, Merge With Next, Equalize) are
//! plain functions on
//! `plan_cabinets::FaceLayout` addressed by a path of indices, so they test
//! without a GUI.

use super::cabinet_face::{item_spec_ui, FaceSpecContext};
use super::{
    dis_check, dis_combo, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog,
    SpecPages, Tab, PV_GLASS, PV_INK, PV_WALL,
};
use crate::editor::placed::{cabinet_label, cabinet_layer};
use crate::tools::library::door_styles::{self, LibraryStyle};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use plan_cabinets::{
    plan_symbol, AutoOnOff, Backsplash, BlindSide, Cabinet, CabinetKind, CabinetStyle, CornerSpec,
    CornerStyle, CornerTreatment, Countertop, CutoutKind, Divider, DoorProfile, DoorStyle,
    DrawerStyle, EdgeProfile, FaceCell, FaceItem, FaceLayout, FaceSide, FillPattern, FootStyle,
    HandleStyle, HingeStyle, ItemKind, Manufacturer, MaterialChoice, Molding, Overlay,
    PilasterStyle, PlanFill, SideKind, ToeKick,
};
use plan_core::geometry::Point;

/// Appliances an opening can hold.
const APPLIANCES: &[&str] = &["Dishwasher", "Range", "Refrigerator", "Microwave"];

const TABS: &[Tab] = &[
    on("General"),
    on("Box Construction"),
    on("Front/Sides/Back"),
    on("Door/Drawer"),
    on("Accessories"),
    on("Opening Indicators"),
    on("Moldings"),
    on("Layer"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
    on("Manufacturer"),
];

// ----- the face-item tree -----

/// Indices from `FaceLayout::items` down: the first picks a top-level item,
/// each further one a cell of the `HorizontalLayout` before it.
pub type Path = Vec<usize>;

/// The kinds the New Cabinet Face Item and Item Type lists offer: the 16
/// leaf types of Chief's list (a layout is made by splitting).
pub fn leaf_kinds() -> impl Iterator<Item = ItemKind> {
    ItemKind::ALL.into_iter().filter(|k| k.is_leaf())
}

/// The tree line for an item.
pub fn describe(item: &FaceItem) -> String {
    match item.base() {
        FaceItem::Appliance { name, .. } => format!("Appliance - {name}"),
        other => other.kind().label().to_string(),
    }
}

pub fn item_at<'a>(layout: &'a FaceLayout, path: &[usize]) -> Option<&'a FaceItem> {
    let (first, rest) = path.split_first()?;
    let mut item = layout.items.get(*first)?;
    for &i in rest {
        match item {
            FaceItem::HorizontalLayout { cells, .. } => item = &cells.get(i)?.item,
            FaceItem::VerticalLayout { items, .. } => item = items.get(i)?,
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
            FaceItem::VerticalLayout { items, .. } => item = items.get_mut(i)?,
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
        FaceItem::VerticalLayout { items, .. } => Some(Cont::Items(items)),
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
                FaceItem::VerticalLayout { items, .. } if items.len() == 1 => {
                    items.remove(0).with_height(height)
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

/// Merge With Next: joins the selected item and the one after it into one
/// (the selected item stays, the next one goes). In a vertical stack the
/// merged item is as tall as both together (auto when either is auto); in a
/// horizontal layout it is as wide as both (auto when either is auto). A
/// layout left with one cell becomes that cell. Returns the path to select
/// afterwards, or `None` when the item is the last of its list.
pub fn merge_with_next(layout: &mut FaceLayout, path: &[usize]) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    match container(layout, parent)? {
        Cont::Items(v) => {
            let next = v.get(idx + 1)?.height();
            let own = v.get(*idx)?.height();
            let merged = if own > 0.0 && next > 0.0 {
                own + next
            } else {
                0.0
            };
            let item = v.get_mut(*idx)?;
            *item = item.with_height(merged);
        }
        Cont::Cells(v) => {
            let next = v.get(idx + 1)?.width;
            let own = v.get(*idx)?.width;
            v.get_mut(*idx)?.width = match (own, next) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            };
        }
    }
    let mut next_path = parent.to_vec();
    next_path.push(idx + 1);
    delete_item(layout, &next_path)?;
    if item_at(layout, path).is_some() {
        Some(path.to_vec())
    } else {
        // The layout collapsed into its remaining cell.
        Some(parent.to_vec())
    }
}

/// Split Vertical: stacks a copy of the item under it (the two share its
/// height). Only items of a vertical stack split this way.
pub fn split_vertical(layout: &mut FaceLayout, path: &[usize]) -> Option<Path> {
    let (idx, parent) = path.split_last()?;
    if !parent.is_empty() {
        return None;
    }
    let item = layout.items.get(*idx)?.clone();
    if matches!(item.base(), FaceItem::Separation { .. }) {
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
        if matches!(cell.item.base(), FaceItem::Separation { .. }) {
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
        n if matches!(n.base(), FaceItem::Separation { .. }) => None,
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
                    if !matches!(it.base(), FaceItem::Separation { .. }) {
                        it.set_height(0.0);
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

/// True for the layouts (the containers of other items).
fn is_layout(item: &FaceItem) -> bool {
    matches!(
        item.base(),
        FaceItem::HorizontalLayout { .. } | FaceItem::VerticalLayout { .. }
    )
}

/// Changes the selected leaf to `ty`, keeping its height, its settings
/// (the Custom wrapper) and an appliance's name.
pub fn set_item_type(layout: &mut FaceLayout, path: &[usize], ty: ItemKind) -> bool {
    if !ty.is_leaf() {
        return false;
    }
    match item_at_mut(layout, path) {
        Some(it) if !is_layout(it) => {
            let name = match it.base() {
                FaceItem::Appliance { name, .. } => Some(name.clone()),
                _ => None,
            };
            let props = it.props().cloned();
            let mut next = ty.make(it.height());
            if let (FaceItem::Appliance { name: n, .. }, Some(old)) = (&mut next, name) {
                *n = old;
            }
            if let Some(p) = props {
                next = next.with_props(p);
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
    new_type: ItemKind,
    /// The selected manual shelf of the Cabinet Shelf Specification.
    shelf_sel: usize,
    /// The face the Front/Sides/Back tab edits.
    side: FaceSide,
    /// The library's "Cabinet Doors" styles the Door/Drawer tab offers.
    styles: Vec<LibraryStyle>,
    /// What the last Chief scan said.
    style_note: String,
    /// Why the last Cabinet Style change was refused.
    convert_note: String,
}

impl CabinetDialog {
    pub fn new(cabinet: Cabinet) -> Self {
        Self {
            frame: SpecDialog::new("Cabinet Specification", "cabinet"),
            form: CabinetForm {
                draft: cabinet,
                fields: Fields::default(),
                sel: Vec::new(),
                new_type: ItemKind::Drawer,
                shelf_sel: 0,
                side: FaceSide::Front,
                styles: door_styles::available(),
                style_note: String::new(),
                convert_note: String::new(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Cabinet {
        &self.form.draft
    }

    /// Tests edit the draft the way the pages do.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Cabinet {
        &mut self.form.draft
    }
}

fn handle_name(h: HandleStyle) -> &'static str {
    h.name()
}

fn handle_combo(ui: &mut Ui, salt: &str, value: &mut HandleStyle) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(handle_name(*value))
        .show_ui(ui, |ui| {
            for h in HandleStyle::ALL {
                ui.selectable_value(value, h, handle_name(h));
            }
        });
}

impl CabinetForm {
    fn face_width(&self) -> f64 {
        self.draft.face_width()
    }

    fn general(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Cabinet Style");
        let ordinary = d.appliance.is_none()
            && matches!(
                d.kind,
                CabinetKind::Base
                    | CabinetKind::Wall
                    | CabinetKind::FullHeight
                    | CabinetKind::CornerBase
                    | CabinetKind::CornerWall
            );
        if ordinary {
            // Converting between a standard cabinet and a corner, end,
            // radius end, peninsula, angled or bow front (CB-494, CB-495).
            let cur = d.style();
            let mut pick = cur;
            row(ui, "Type", |ui| {
                egui::ComboBox::from_id_salt("cab_style")
                    .selected_text(cur.name())
                    .show_ui(ui, |ui| {
                        for st in CabinetStyle::ALL {
                            ui.selectable_value(&mut pick, st, st.name());
                        }
                    });
            });
            if pick != cur {
                match d.convert(pick) {
                    Ok(()) => self.convert_note.clear(),
                    Err(e) => self.convert_note = e,
                }
            }
            if let Some(sp) = d.special {
                let mut amount = sp.resolved(d.width, d.depth);
                if f.length_row(ui, sp.shape.amount_label(), "special_amount", &mut amount) {
                    match d.set_special_amount(amount) {
                        Ok(()) => self.convert_note.clear(),
                        Err(e) => self.convert_note = e,
                    }
                }
            }
            if d.kind.is_corner() {
                f.length_row(
                    ui,
                    "Bow Depth (0 = straight)",
                    "corner_bow",
                    &mut d.corner_bow,
                );
            }
            if !self.convert_note.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(190, 40, 40), &self.convert_note);
            }
        } else {
            row(ui, "Type", |ui| {
                dis_combo(
                    ui,
                    "cab_type",
                    d.preset
                        .map_or_else(|| d.kind.name(), plan_cabinets::CabinetPreset::name),
                )
            });
        }
        dis_check(ui, "Treat As Filler", d.kind.is_filler());
        section(ui, "Size/Position");
        if d.kind.is_custom() {
            // A free-form top is sized by its outline, not by typed numbers.
            row(ui, "Extent (width x depth)", |ui| {
                ui.label(format!("{} x {}", fmt_short(d.width), fmt_short(d.depth)));
            });
        } else {
            f.length_row(ui, "Width", "width", &mut d.width);
            f.length_row(ui, "Height (including countertop)", "height", &mut d.height);
            f.length_row(ui, "Depth", "depth", &mut d.depth);
        }
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

        if let Some(spec) = d.corner.as_mut() {
            Self::corner_section(ui, f, spec);
        }
        if let Some(b) = d.blind.as_mut() {
            section(ui, "Blind Corner");
            row(ui, "Hidden end", |ui| {
                ui.radio_value(&mut b.side, BlindSide::Left, "Left");
                ui.radio_value(&mut b.side, BlindSide::Right, "Right");
            });
            f.length_row(ui, "Blind Width", "blind_w", &mut b.blind_width);
        }
        if d.kind == CabinetKind::Base {
            section(ui, "Appliance Opening");
            let mut bay = d.appliance.is_some();
            if ui
                .checkbox(&mut bay, "Open bay for an appliance (dishwasher, range)")
                .changed()
            {
                d.set_appliance(bay.then_some("Dishwasher"));
            }
        }
        if d.appliance.is_some() {
            Self::appliance_section(ui, d);
        }
        if let Some(c) = d.custom.as_mut() {
            section(ui, "Custom Top");
            if !d.joined.is_empty() {
                ui.weak(format!(
                    "Joined from {} cabinets: its outline and thickness follow them and are rebuilt when they change; the edge and corner treatment are kept.",
                    d.joined.len()
                ));
            }
            if f.length_row(ui, "Thickness", "cust_thick", &mut c.thickness)
                && d.kind == CabinetKind::CustomCountertop
            {
                // The slab is its thickness high; keep its top where it was.
                let top = d.elevation + d.height;
                d.height = c.thickness;
                d.elevation = top - c.thickness;
            }
            if d.kind == CabinetKind::CustomCountertop {
                row(ui, "Edge Profile", |ui| {
                    egui::ComboBox::from_id_salt("cust_edge")
                        .selected_text(c.edge.name())
                        .show_ui(ui, |ui| {
                            for e in EdgeProfile::ALL {
                                ui.selectable_value(&mut c.edge, e, e.name());
                            }
                        });
                });
                if !matches!(c.edge, EdgeProfile::Square | EdgeProfile::Waterfall) {
                    f.length_row(ui, "Edge Size", "cust_edge_size", &mut c.edge_size);
                }
                row(ui, "Corner Treatment", |ui| {
                    for k in CornerTreatment::ALL {
                        ui.radio_value(&mut c.corner, k, k.name());
                    }
                });
                if c.corner != CornerTreatment::None {
                    f.length_row(ui, "Corner Size", "cust_corner", &mut c.corner_size);
                }
            } else {
                // A backsplash is `height` high; the strip thickness is the custom one.
                f.length_row(ui, "Height", "cust_bs_height", &mut d.height);
            }
        }

        if d.kind == CabinetKind::CustomCountertop {
            // The polyline, selected line, waterfall and molding panels.
            if let Some(msg) = super::custom_countertop::custom_top_ui(ui, f, d) {
                self.convert_note = msg.to_string();
            }
            if !self.convert_note.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(190, 40, 40), &self.convert_note);
            }
        }
        if d.kind != CabinetKind::CustomBacksplash {
            Self::countertop_section(ui, f, d);
        }
        if !d.kind.is_custom() {
            section(ui, "Backsplash");
            let mut has = d.backsplash.is_some();
            if ui.checkbox(&mut has, "Backsplash").changed() {
                d.backsplash = has.then_some(Backsplash::new(4.0, 0.5));
            }
            if let Some(b) = d.backsplash.as_mut() {
                ui.checkbox(
                    &mut b.full_height,
                    "Full Height (to the wall cabinet above)",
                );
                if b.full_height {
                    row(ui, "Height", |ui| ui.label(fmt_short(b.height)));
                } else {
                    f.length_row(ui, "Height", "bs_height", &mut b.height);
                }
                f.length_row(ui, "Thickness", "bs_thick", &mut b.thickness);
                ui.checkbox(
                    &mut b.side,
                    "Side Backsplash (against a wall or taller cabinet)",
                );
                ui.checkbox(&mut b.always_present, "Always Present");
            }

            section(ui, "Toe Kick");
            let mut has = d.toe_kick.is_some();
            if ui.checkbox(&mut has, "Toe Kick").changed() {
                d.toe_kick = has.then(ToeKick::default);
            }
            if let Some(t) = d.toe_kick.as_mut() {
                f.length_row(ui, "Height", "tk_height", &mut t.height);
                f.length_row(ui, "Depth", "tk_depth", &mut t.depth);
                let o = &mut d.toe_options;
                ui.checkbox(&mut o.flat_sides, "Flat Sides");
                ui.checkbox(&mut o.flat_back, "Flat Back");
                ui.checkbox(&mut o.closed_toe, "Closed Toe");
                if o.closed_toe {
                    ui.checkbox(&mut o.closed_toe_always, "Always Closed (between cabinets)");
                }
            }
        }
    }

    fn corner_section(ui: &mut Ui, f: &mut Fields, spec: &mut CornerSpec) {
        section(ui, "Corner Cabinet");
        row(ui, "Front", |ui| {
            ui.radio_value(&mut spec.style, CornerStyle::Diagonal, "Diagonal");
            ui.radio_value(&mut spec.style, CornerStyle::PieCut, "Pie-Cut");
        });
        ui.add_enabled_ui(spec.style == CornerStyle::PieCut, |ui| {
            ui.checkbox(&mut spec.lazy_susan, "Lazy Susan shelves");
        });
        f.length_row(ui, "Arm Depth", "corner_arm", &mut spec.arm_depth);
    }

    fn appliance_section(ui: &mut Ui, d: &mut Cabinet) {
        let mut name = d.appliance.clone().unwrap_or_default();
        row(ui, "Appliance", |ui| {
            egui::ComboBox::from_id_salt("cab_appliance")
                .selected_text(name.clone())
                .show_ui(ui, |ui| {
                    for n in APPLIANCES {
                        ui.selectable_value(&mut name, (*n).to_string(), *n);
                    }
                });
        });
        if Some(&name) != d.appliance.as_ref() {
            d.set_appliance(Some(&name));
        }
        ui.weak("The bay stays open; the appliance fills it in plan and 3D.");
    }

    /// Countertop settings and the sink and cooktop cutouts.
    fn countertop_section(ui: &mut Ui, f: &mut Fields, d: &mut Cabinet) {
        if d.kind != CabinetKind::CustomCountertop {
            section(ui, "Countertop");
            let mut has = d.countertop.is_some();
            if ui.checkbox(&mut has, "Countertop").changed() {
                d.countertop = has.then(Countertop::default);
                if !has {
                    d.cutouts.clear();
                }
            }
            if let Some(t) = d.countertop.as_mut() {
                f.length_row(ui, "Thickness", "ct_thick", &mut t.thickness);
                f.length_row(ui, "Overhang Front", "ct_front", &mut t.overhang_front);
                f.length_row(ui, "Overhang Back", "ct_back", &mut t.overhang_back);
                f.length_row(ui, "Overhang Sides", "ct_sides", &mut t.overhang_sides);
                row(ui, "Corner Treatment", |ui| {
                    for c in CornerTreatment::ALL {
                        ui.radio_value(&mut t.corner, c, c.name());
                    }
                });
                if t.corner != CornerTreatment::None {
                    f.length_row(ui, "Corner Size", "ct_corner", &mut t.corner_size);
                }
                row(ui, "Edge Profile", |ui| {
                    egui::ComboBox::from_id_salt("ct_edge")
                        .selected_text(t.edge.name())
                        .show_ui(ui, |ui| {
                            for e in EdgeProfile::ALL {
                                ui.selectable_value(&mut t.edge, e, e.name());
                            }
                        });
                });
                if !matches!(t.edge, EdgeProfile::Square | EdgeProfile::Waterfall) {
                    f.length_row(ui, "Edge Size", "ct_edge_size", &mut t.edge_size);
                }
                if t.edge == EdgeProfile::Waterfall {
                    ui.weak("The slab runs down to the floor at both ends of the cabinet.");
                }
            }
        }
        if d.top_local().is_none() {
            return;
        }
        section(ui, "Sink and Cooktop Cutouts");
        ui.horizontal(|ui| {
            if ui.button("Add Sink").clicked() {
                d.add_cutout(CutoutKind::Sink);
            }
            if ui.button("Add Cooktop").clicked() {
                d.add_cutout(CutoutKind::Cooktop);
            }
        });
        let mut remove = None;
        for (i, cut) in d.cutouts.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}  ({:.0} sq in)", cut.name, cut.area()));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.cutouts.remove(i);
        }
        if d.cutouts.is_empty() {
            ui.weak("No cutouts.");
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
        if d.framed {
            // Extended stiles reach past the ends and act as fillers.
            f.length_row(
                ui,
                "Extend Left Stile",
                "stile_ext_l",
                &mut d.stile_ext_left,
            );
            f.length_row(
                ui,
                "Extend Right Stile",
                "stile_ext_r",
                &mut d.stile_ext_right,
            );
        }
        section(ui, "Top/Bottom/Sides");
        let bc = &mut d.box_construction;
        row(ui, "Top", |ui| {
            for (m, name) in [
                (AutoOnOff::Auto, "Auto"),
                (AutoOnOff::On, "Has Top"),
                (AutoOnOff::Off, "No Top"),
            ] {
                ui.radio_value(&mut bc.top, m, name);
            }
        });
        row(ui, "Bottom", |ui| {
            for (m, name) in [
                (AutoOnOff::Auto, "Auto"),
                (AutoOnOff::On, "Has Bottom"),
                (AutoOnOff::Off, "No Bottom"),
            ] {
                ui.radio_value(&mut bc.bottom, m, name);
            }
        });
        f.length_row(ui, "Side Thickness", "bc_side", &mut bc.side_thickness);
        f.length_row(ui, "Back Thickness", "bc_back", &mut bc.back_thickness);
        section(ui, "Box Corners");
        row(ui, "Corner Treatment", |ui| {
            for c in CornerTreatment::ALL {
                ui.radio_value(&mut bc.corner, c, c.name());
            }
        });
        if bc.corner != CornerTreatment::None {
            f.length_row(ui, "Corner Clip / Radius", "bc_corner", &mut bc.corner_size);
            ui.checkbox(&mut bc.auto_corners, "Automatic Placement");
            if !bc.auto_corners {
                ui.horizontal(|ui| {
                    for (i, name) in ["Back Left", "Back Right", "Front Left", "Front Right"]
                        .into_iter()
                        .enumerate()
                    {
                        ui.checkbox(&mut bc.corners[i], name);
                    }
                });
            }
        }
        section(ui, "General Options");
        ui.checkbox(&mut d.cut_room_moldings, "Cut Room Moldings");
        ui.checkbox(&mut d.suppress_fillers, "Suppress Automatic Fillers");
        f.length_row(
            ui,
            "Auto Door Threshold",
            "auto_door",
            &mut d.auto_door_threshold,
        );
        // Absolute ... From Roof, To Top / To Bottom: the shared widget
        // (stored with the cabinet's shared panels, applied in 3D).
        super::elevation_ref::row(ui, "Elevation Reference");
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

    /// The Front/Sides/Back tab: pick the face, its Side Type, and (for the
    /// front and Custom Face sides) edit its face items.
    fn front(&mut self, ui: &mut Ui) {
        section(ui, "Cabinet Side");
        let mut side = self.side;
        row(ui, "Side", |ui| {
            egui::ComboBox::from_id_salt("cab_side")
                .selected_text(side.name())
                .show_ui(ui, |ui| {
                    for s in FaceSide::ALL {
                        ui.selectable_value(&mut side, s, s.name());
                    }
                });
        });
        if side != self.side {
            self.side = side;
            self.sel.clear();
        }
        let rectangular = !self.draft.kind.is_corner() && !self.draft.kind.is_custom();
        if side == FaceSide::Front {
            row(ui, "Side Type", |ui| {
                dis_combo(ui, "cab_side_type", "Custom Face")
            });
            self.side_properties(ui);
            self.face_editor(ui, side);
            return;
        }
        let mut kind = self.draft.side_kind(side);
        let before = kind;
        ui.add_enabled_ui(rectangular, |ui| {
            row(ui, "Side Type", |ui| {
                egui::ComboBox::from_id_salt("cab_side_type")
                    .selected_text(kind.name())
                    .show_ui(ui, |ui| {
                        for k in SideKind::ALL {
                            ui.selectable_value(&mut kind, k, k.name());
                        }
                    });
            });
        });
        if !rectangular {
            ui.weak("Only rectangular cabinets have editable sides.");
            return;
        }
        if kind != before {
            let layout = self
                .draft
                .side_face(side)
                .map(|s| s.layout.clone())
                .unwrap_or_else(FaceLayout::single_door);
            self.draft.set_side_face(side, kind, layout);
            self.sel.clear();
        }
        match kind {
            SideKind::CustomFace => {
                // The editor below works on `draft.face`: lend it the side's
                // layout for the length of the call.
                let layout = self
                    .draft
                    .side_face(side)
                    .map(|s| s.layout.clone())
                    .unwrap_or_else(FaceLayout::single_door);
                let front = std::mem::replace(&mut self.draft.face, layout);
                self.face_editor(ui, side);
                let edited = std::mem::replace(&mut self.draft.face, front);
                self.draft.set_side_face(side, SideKind::CustomFace, edited);
            }
            SideKind::Plain => {
                ui.weak("The plain 3/4\" carcass panel.");
            }
            SideKind::Finished => {
                ui.weak("A finished slab panel in the door material.");
            }
            SideKind::Open => {
                ui.weak("No panel: the box is open on this side.");
            }
        }
    }

    /// Side Properties and Show Open options of the front (reference manual
    /// p. 677): the left and right stile widths and reveals follow the
    /// cabinet until a value is specified.
    fn side_properties(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Side Properties");
        let stile = d.face.frame_width;
        let reveal = match d.overlay {
            Overlay::Full { reveal } => reveal,
            Overlay::Traditional { overlap } => overlap,
            Overlay::Inset { clearance } => clearance,
        };
        opt_length_row(ui, f, "Left Stile", "st_left", &mut d.stiles.left, stile);
        opt_length_row(ui, f, "Right Stile", "st_right", &mut d.stiles.right, stile);
        opt_length_row(
            ui,
            f,
            "Left Reveal",
            "rv_left",
            &mut d.stiles.left_reveal,
            reveal,
        );
        opt_length_row(
            ui,
            f,
            "Right Reveal",
            "rv_right",
            &mut d.stiles.right_reveal,
            reveal,
        );
        section(ui, "Options");
        ui.horizontal_wrapped(|ui| {
            ui.label("Show Open:");
            ui.checkbox(&mut d.show_open.doors, "Doors");
            ui.checkbox(&mut d.show_open.drawers, "Drawers");
            ui.checkbox(&mut d.show_open.rollouts, "Rollouts");
        });
        f.length_row(
            ui,
            "Auto Door Threshold",
            "auto_door_front",
            &mut d.auto_door_threshold,
        );
    }

    /// The face canvas and the face-item tree of `draft.face`, which is the
    /// front's layout or (while a side is edited) that side's.
    fn face_editor(&mut self, ui: &mut Ui, side: FaceSide) {
        section(ui, "Elevation (drag a divider to resize)");
        let mut sel = std::mem::take(&mut self.sel);
        let (fh, fw) = (self.draft.face_height(), self.draft.side_width(side));
        face_canvas(ui, &mut self.draft, fh, fw, &mut sel);
        section(ui, "Face Items");
        let layout = &mut self.draft.face;
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
                    for t in leaf_kinds() {
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
            if ui
                .add_enabled(has, egui::Button::new("Merge With Next"))
                .on_hover_text("Joins the selected item and the one after it")
                .clicked()
            {
                if let Some(p) = merge_with_next(layout, &sel) {
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
                self.draft.face = if side != FaceSide::Front {
                    FaceLayout::single_door()
                } else {
                    match self.draft.kind {
                        CabinetKind::Wall => FaceLayout::wall_default(face_h),
                        CabinetKind::FullHeight => FaceLayout::full_height_default(face_h),
                        _ => FaceLayout::base_default(face_h),
                    }
                };
                sel.clear();
            }
            if side == FaceSide::Front && ui.button("Sink Base Face").clicked() {
                self.draft.face = FaceLayout::sink_base();
                sel.clear();
            }
        });
        self.selected_properties(ui, &mut sel);
        self.sel = sel;
    }

    fn selected_properties(&mut self, ui: &mut Ui, sel: &mut Path) {
        section(ui, "Selected Item Properties");
        let Some(item) = item_at(&self.draft.face, sel).cloned() else {
            ui.weak("Select a face item above.");
            return;
        };
        let in_cell = sel.len() > 1
            && sel.split_last().is_some_and(|(_, p)| {
                matches!(
                    item_at(&self.draft.face, p),
                    Some(FaceItem::HorizontalLayout { .. })
                )
            });
        let cur = item.kind();
        if cur.is_leaf() {
            let mut ty = cur;
            row(ui, "Item Type", |ui| {
                egui::ComboBox::from_id_salt("cab_item_type")
                    .selected_text(ty.label())
                    .show_ui(ui, |ui| {
                        for t in leaf_kinds() {
                            ui.selectable_value(&mut ty, t, t.label());
                        }
                    });
            });
            if ty != cur {
                set_item_type(&mut self.draft.face, sel, ty);
            }
        } else {
            row(ui, "Item Type", |ui| ui.label(cur.label()));
        }
        let mut h = item.height();
        if !in_cell
            && self
                .fields
                .length_row(ui, "Item Height (0 = auto)", "item_h", &mut h)
            && h >= 0.0
        {
            if let Some(it) = item_at_mut(&mut self.draft.face, sel) {
                it.set_height(h);
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
        if let Some(FaceItem::Appliance { name, .. }) =
            item_at_mut(&mut self.draft.face, sel).map(FaceItem::base_mut)
        {
            row(ui, "Appliance", |ui| {
                ui.text_edit_singleline(name);
            });
        }
        if cur.is_leaf() {
            let opening = self.opening_height_of(sel);
            let cx = FaceSpecContext {
                door: &self.draft.door_style,
                drawer: &self.draft.drawer_style,
                styles: &self.styles,
                opening_height: opening,
                allow_rollout: self.draft.kind != CabinetKind::Wall,
                key: {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    (self.side as u8, sel.as_slice()).hash(&mut h);
                    h.finish()
                },
            };
            let mut shelf_sel = self.shelf_sel;
            if let Some(it) = item_at_mut(&mut self.draft.face, sel) {
                item_spec_ui(ui, &mut self.fields, it, &cx, &mut shelf_sel);
            }
            self.shelf_sel = shelf_sel;
        }
    }

    /// Height of the opening the selected item fills, inches.
    fn opening_height_of(&self, sel: &Path) -> f64 {
        let (fh, fw) = (self.draft.face_height(), self.face_width());
        let Ok(leaves) = self.draft.face.resolve(fh, fw) else {
            return fh;
        };
        let paths = leaf_paths(&self.draft.face);
        paths
            .iter()
            .zip(&leaves)
            .find(|(p, _)| *p == sel)
            .map_or(fh, |(_, r)| r.rect.3)
    }

    fn door_drawer(&mut self, ui: &mut Ui) {
        let mut rescan = false;
        let f = &mut self.fields;
        let d = &mut self.draft;
        section(ui, "Door Panel");
        let styles = &self.styles;
        row(ui, "Main Style", |ui| {
            let mut pick: Option<StylePick> = None;
            egui::ComboBox::from_id_salt("door_style")
                .selected_text(d.door_style.name.clone())
                .show_ui(ui, |ui| {
                    for (name, _) in DoorStyle::BUILTIN {
                        let here = d.door_style.library.is_empty() && d.door_style.name == name;
                        if ui.selectable_label(here, name).clicked() {
                            pick = Some(StylePick::Builtin(name));
                        }
                    }
                    let lib = door_styles::doors(styles);
                    if !lib.is_empty() {
                        ui.separator();
                        ui.weak("Library: Cabinet Doors");
                        for s in lib {
                            let here = d.door_style.library == s.id;
                            if ui
                                .selectable_label(here, format!("{} ({})", s.name, s.source))
                                .clicked()
                            {
                                pick = Some(StylePick::Library(s.id.clone()));
                            }
                        }
                    }
                });
            if let Some(pick) = pick {
                pick_door_style(&mut d.door_style, pick, styles);
            }
        });
        if crate::tools::library::chief::enabled() {
            row(ui, "Chief styles", |ui| {
                let label = if door_styles::chief_scanned() {
                    "Scan Chief Library Again"
                } else {
                    "Load Chief Library Styles"
                };
                if ui.button(label).clicked() {
                    rescan = true;
                }
                ui.weak(self.style_note.as_str());
            });
        } else {
            ui.weak("Library styles come from the \"Cabinet Doors\" category; turn on the Chief catalogs in Preferences to list Chief's.");
        }
        profile_row(ui, "door_profile", &mut d.door_style.profile);
        f.length_row(ui, "Thickness", "door_thick", &mut d.door_style.thickness);
        if d.door_style.profile != DoorProfile::Slab {
            f.length_row(
                ui,
                "Stile and Rail Width",
                "door_frame",
                &mut d.door_style.frame_width,
            );
        }
        ui.checkbox(&mut d.door_style.glass, "Glass Doors");
        section(ui, "Door Handle");
        row(ui, "Main Style", |ui| {
            handle_combo(ui, "door_handle", &mut d.door_style.handle);
        });
        row(ui, "Vertical Position", |ui| {
            ui.radio_value(&mut d.door_style.handle_centered, true, "Centered");
            ui.radio_value(
                &mut d.door_style.handle_centered,
                false,
                "Distance From Top",
            );
        });
        if !d.door_style.handle_centered {
            f.length_row(
                ui,
                "Distance From Top",
                "door_h_top",
                &mut d.door_style.handle_from_top,
            );
        }
        f.length_row(
            ui,
            "Distance From Edge",
            "door_h_edge",
            &mut d.door_style.handle_from_edge,
        );
        if matches!(
            d.door_style.handle,
            HandleStyle::Pull | HandleStyle::Edge | HandleStyle::Cup
        ) {
            f.length_row(
                ui,
                "Pull Length",
                "door_h_len",
                &mut d.door_style.handle_length,
            );
        }
        ui.weak(
            "Base doors measure from the top, wall doors from the bottom; a pull \
             is measured to its near end. Tall doors hang their handle at 38\".",
        );
        section(ui, "Door Hinges");
        row(ui, "Main Style", |ui| {
            ui.radio_value(&mut d.door_style.hinge, HingeStyle::Hidden, "Hidden");
            ui.radio_value(&mut d.door_style.hinge, HingeStyle::Exposed, "Exposed");
        });
        f.length_row(
            ui,
            "Up/Down From Edge",
            "door_hinge",
            &mut d.door_style.hinge_from_edge,
        );
        section(ui, "Drawer Panel");
        row(ui, "Main Style", |ui| {
            let mut pick: Option<StylePick> = None;
            egui::ComboBox::from_id_salt("drawer_style")
                .selected_text(d.drawer_style.name.clone())
                .show_ui(ui, |ui| {
                    for (name, _) in DrawerStyle::BUILTIN {
                        let here = d.drawer_style.library.is_empty() && d.drawer_style.name == name;
                        if ui.selectable_label(here, name).clicked() {
                            pick = Some(StylePick::Builtin(name));
                        }
                    }
                    if !styles.is_empty() {
                        ui.separator();
                        ui.weak("Library: Cabinet Doors and Drawers");
                        for s in styles.iter() {
                            let here = d.drawer_style.library == s.id;
                            if ui
                                .selectable_label(here, format!("{} ({})", s.name, s.source))
                                .clicked()
                            {
                                pick = Some(StylePick::Library(s.id.clone()));
                            }
                        }
                    }
                });
            if let Some(pick) = pick {
                pick_drawer_style(&mut d.drawer_style, pick, styles);
            }
        });
        profile_row(ui, "drawer_profile", &mut d.drawer_style.profile);
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
        row(ui, "Horizontal Position", |ui| {
            ui.label("One Handle Centered");
        });
        row(ui, "Vertical Position", |ui| {
            ui.radio_value(&mut d.drawer_style.handle_centered, true, "Centered");
            ui.radio_value(&mut d.drawer_style.handle_centered, false, "Near the top");
        });
        if !d.drawer_style.handle_centered {
            f.length_row(
                ui,
                "Distance From Top",
                "drawer_h_top",
                &mut d.drawer_style.handle_from_top,
            );
        }
        if matches!(
            d.drawer_style.handle,
            HandleStyle::Pull | HandleStyle::Edge | HandleStyle::Cup
        ) {
            f.length_row(
                ui,
                "Pull Length",
                "drawer_h_len",
                &mut d.drawer_style.handle_length,
            );
        }
        ui.weak("Cup and edge pulls always sit at the top edge of the drawer front.");
        if rescan {
            let n = door_styles::scan_chief();
            self.style_note = format!("{n} Chief door and drawer styles found");
            self.styles = door_styles::available();
        }
    }

    fn indicators(&mut self, ui: &mut Ui) {
        section(ui, "Opening Indicators");
        ui.checkbox(
            &mut self.draft.indicators,
            "Show door swings and open drawers in plan",
        );
        ui.weak(
            "Doors draw a quarter-circle swing from their hinge; drawers draw \
             pulled out in front of the cabinet.",
        );
        ui.checkbox(
            &mut self.draft.indicators_3d,
            "Show open doors and drawers in 3D (shelves and drawer boxes inside)",
        );
        ui.weak(
            "Doors stand open at 90 degrees with their shelves showing; drawers \
             stand part way out with their boxes.",
        );
    }

    fn accessories(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let d = &mut self.draft;
        let acc = &mut d.accessories;
        section(ui, "Front Pilasters");
        row(ui, "Front Pilaster", |ui| {
            egui::ComboBox::from_id_salt("acc_pilaster")
                .selected_text(acc.pilaster.name())
                .show_ui(ui, |ui| {
                    for p in PilasterStyle::ALL {
                        ui.selectable_value(&mut acc.pilaster, p, p.name());
                    }
                });
        });
        if acc.pilaster != PilasterStyle::None {
            row(ui, "Place On", |ui| {
                ui.checkbox(&mut acc.pilaster_left, "Left");
                ui.checkbox(&mut acc.pilaster_right, "Right");
            });
            f.length_row(ui, "Width", "acc_pilaster_w", &mut acc.pilaster_width);
        }
        section(ui, "Feet");
        let has_kick = d.toe_kick.is_some_and(|t| t.height > 0.0);
        ui.add_enabled_ui(has_kick, |ui| {
            row(ui, "Foot Style", |ui| {
                egui::ComboBox::from_id_salt("acc_feet")
                    .selected_text(acc.feet.name())
                    .show_ui(ui, |ui| {
                        for p in FootStyle::ALL {
                            ui.selectable_value(&mut acc.feet, p, p.name());
                        }
                    });
            });
            if acc.feet != FootStyle::None {
                f.length_row(ui, "Foot Size", "acc_foot_size", &mut acc.foot_size);
            }
        });
        if !has_kick {
            ui.weak("Feet stand in place of a toe kick: give the cabinet one on the General tab.");
        } else {
            ui.weak("Four feet replace the toe kick board.");
        }
        section(ui, "Side Panels");
        for (side, label) in [
            (FaceSide::Left, "Finished panel on the left end"),
            (FaceSide::Right, "Finished panel on the right end"),
        ] {
            let mut finished = d.side_kind(side) == SideKind::Finished;
            if ui.checkbox(&mut finished, label).changed() {
                let layout = d
                    .side_face(side)
                    .map_or_else(FaceLayout::empty, |s| s.layout.clone());
                let kind = if finished {
                    SideKind::Finished
                } else {
                    SideKind::Plain
                };
                d.set_side_face(side, kind, layout);
            }
        }
        ui.weak("The same as Side Type on the Front/Sides/Back tab.");
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style (Box)");
        fill_editor(ui, &mut self.fields, "cab_fill", &mut self.draft.fill);
        if self.draft.countertop.is_some() || self.draft.kind.is_custom() {
            let mut own = self.draft.counter_fill.is_some();
            section(ui, "Fill Style (Countertop)");
            if ui
                .checkbox(&mut own, "Separate fill for the countertop")
                .changed()
            {
                self.draft.counter_fill = own.then_some(self.draft.fill);
            }
            if let Some(cf) = self.draft.counter_fill.as_mut() {
                fill_editor(ui, &mut self.fields, "cab_counter_fill", cf);
            }
        }
        ui.weak("Drawn inside the cabinet's outline in the plan view only.");
    }

    /// The Manufacturer panel (catalog cabinets): contact information.
    fn manufacturer(&mut self, ui: &mut Ui) {
        section(ui, "Manufacturer");
        let mut has = self.draft.manufacturer.is_some();
        if ui.checkbox(&mut has, "Catalog cabinet").changed() {
            self.draft.manufacturer = has.then(Manufacturer::default);
        }
        if let Some(m) = self.draft.manufacturer.as_mut() {
            for (label, text) in [
                ("Name", &mut m.name),
                ("Contact", &mut m.contact),
                ("Phone", &mut m.phone),
                ("Email", &mut m.email),
                ("Web Site", &mut m.website),
                ("Catalog", &mut m.catalog),
            ] {
                row(ui, label, |ui| {
                    ui.text_edit_singleline(text);
                });
            }
        } else {
            ui.weak("Turn this on to record where a cabinet is ordered from.");
        }
    }

    /// The parts the cabinet is made of, with counts, sizes and materials.
    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let rows = plan_cabinets::components(&self.draft);
        if rows.is_empty() {
            ui.weak("This kind of cabinet has no parts to list.");
            return;
        }
        egui::Grid::new("cab_components")
            .num_columns(4)
            .spacing([14.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for h in ["Component", "Count", "Size (W x H x T)", "Material"] {
                    ui.strong(h);
                }
                ui.end_row();
                for c in &rows {
                    ui.label(&c.name);
                    ui.label(c.count.to_string());
                    ui.label(&c.size);
                    ui.label(&c.material);
                    ui.end_row();
                }
            });
        ui.weak("Counts and sizes follow the box, Front/Sides/Back, Door/Drawer and Accessories tabs; materials are chosen on the Materials tab.");
    }

    fn object_information(&mut self, ui: &mut Ui) {
        let d = &self.draft;
        section(ui, "Cabinet");
        let size = format!(
            "{} x {} x {}",
            fmt_short(d.width),
            fmt_short(d.depth),
            fmt_short(d.height)
        );
        let source = d.preset.map_or_else(
            || "Plan Studio cabinet".to_string(),
            |p| p.name().to_string(),
        );
        let rows = [
            ("Object", d.kind.name().to_string()),
            ("Label", d.display_label()),
            ("Size (W x D x H)", size),
            ("Elevation", fmt_short(d.elevation)),
            ("Layer", cabinet_layer(d.kind).to_string()),
            ("Made from", source),
            (
                "Door style",
                if d.door_style.library.is_empty() {
                    d.door_style.name.clone()
                } else {
                    format!("{} ({})", d.door_style.name, d.door_style.library)
                },
            ),
        ];
        egui::Grid::new("cab_info").striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        section(ui, "Product");
        let info = &mut self.draft.info;
        row(ui, "Manufacturer", |ui| {
            ui.add(egui::TextEdit::singleline(&mut info.manufacturer).desired_width(220.0));
        });
        row(ui, "Model Number", |ui| {
            ui.add(egui::TextEdit::singleline(&mut info.model).desired_width(220.0));
        });
        row(ui, "Description", |ui| {
            ui.add(egui::TextEdit::singleline(&mut info.description).desired_width(220.0));
        });
        ui.label("Notes");
        ui.add(
            egui::TextEdit::multiline(&mut info.notes)
                .desired_rows(3)
                .desired_width(320.0),
        );
    }

    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Cabinet Schedule");
        ui.checkbox(
            &mut self.draft.in_schedule,
            "List this cabinet in the Cabinet Schedule",
        );
        let d = &self.draft;
        section(ui, "Schedule Row");
        egui::Grid::new("cab_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for h in [
                    "Label",
                    "Type",
                    "Width",
                    "Depth",
                    "Height",
                    "Door Style",
                    "Finish",
                    "Hardware",
                ] {
                    ui.strong(h);
                }
                ui.end_row();
                ui.label(d.display_label());
                ui.label(
                    d.preset
                        .map_or_else(|| d.kind.name().to_string(), |p| p.name().to_string()),
                );
                ui.label(fmt_short(d.width));
                ui.label(fmt_short(d.depth));
                ui.label(fmt_short(d.height));
                ui.label(&d.door_style.name);
                ui.label(d.finish_name());
                ui.label(d.hardware_name());
                ui.end_row();
            });
        if !d.in_schedule {
            ui.weak("Left out of the schedule (and its callouts).");
        }
    }

    fn moldings(&mut self, ui: &mut Ui) {
        const PROJ: [&str; 4] = ["mold_p0", "mold_p1", "mold_p2", "mold_p3"];
        const HIGH: [&str; 4] = ["mold_h0", "mold_h1", "mold_h2", "mold_h3"];
        section(ui, "Profiles");
        let f = &mut self.fields;
        let list = &mut self.draft.moldings;
        if list.is_empty() {
            ui.weak("(no moldings)");
        }
        let mut remove = None;
        for (i, m) in list.iter_mut().enumerate().take(PROJ.len()) {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("{}  {}", i + 1, m.name()));
                if ui.small_button("Delete").clicked() {
                    remove = Some(i);
                }
            });
            f.length_row(ui, "Projection", PROJ[i], &mut m.projection);
            f.length_row(ui, "Height", HIGH[i], &mut m.height);
        }
        if let Some(i) = remove {
            list.remove(i);
        }
        ui.horizontal(|ui| {
            let room = list.len() < PROJ.len();
            if ui
                .add_enabled(room, egui::Button::new("Add Crown"))
                .clicked()
            {
                list.push(Molding::crown());
            }
            if ui
                .add_enabled(room, egui::Button::new("Add Light Rail"))
                .clicked()
            {
                list.push(Molding::light_rail());
            }
        });
        ui.weak("Crown sits on top of the cabinet, light rail under it; both run along the front and return at the ends.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            let kind_layer = cabinet_layer(self.draft.kind);
            let mut own = self.draft.layer.is_some();
            if ui.checkbox(&mut !own, "Default").changed() {
                own = !own;
                self.draft.layer = own.then(|| kind_layer.to_string());
            }
            ui.add_enabled_ui(own, |ui| {
                let mut shown = self
                    .draft
                    .layer
                    .clone()
                    .unwrap_or_else(|| kind_layer.to_string());
                if super::select_layer::layer_field(
                    ui,
                    "cab_layer",
                    &mut shown,
                    ["Cabinets, Base", "Cabinets, Wall"],
                ) {
                    self.draft.layer = Some(shown);
                }
            });
        });
        ui.weak("Default puts the cabinet on the layer of its type.");
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        let m = &mut self.draft.materials;
        for (part, slot, salt) in [
            ("Box", &mut m.carcass, "mat_box"),
            ("Door Fronts", &mut m.door, "mat_door"),
            ("Drawer Fronts", &mut m.drawer, "mat_drawer"),
            ("Countertop", &mut m.countertop, "mat_top"),
            ("Backsplash", &mut m.backsplash, "mat_splash"),
            ("Toe Kick", &mut m.toe_kick, "mat_toe"),
            ("Molding", &mut m.molding, "mat_mold"),
        ] {
            row(ui, part, |ui| {
                egui::ComboBox::from_id_salt(salt)
                    .selected_text(slot.name())
                    .show_ui(ui, |ui| {
                        for c in MaterialChoice::ALL {
                            ui.selectable_value(slot, c, c.name());
                        }
                    });
            });
        }
        ui.weak("Default keeps the part's usual stand-in material.");
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
            row(ui, "Shown in plan", |ui| {
                ui.label(self.draft.display_label())
            });
            ui.weak(
                "Macros: <L> automatic label, <T> type letters, <W> <D> <H> sizes, \
                 <WxD> <WxH> <WxDxH> joined sizes, <N> cabinet name, <S> door style, \
                 <F> finish, <HW> hardware, <A> appliance.",
            );
        }
        row(ui, "Label position", |ui| {
            let moved = self.draft.label_offset != Point::ZERO;
            ui.label(if moved {
                "Moved from the centre"
            } else {
                "Centred"
            });
            if ui.add_enabled(moved, egui::Button::new("Reset")).clicked() {
                self.draft.label_offset = Point::ZERO;
            }
        });
        ui.weak("Drag the square handle next to the label in the plan to move it.");
    }
}

/// What the Main Style list picked.
#[derive(Debug, Clone, PartialEq)]
enum StylePick {
    Builtin(&'static str),
    Library(String),
}

/// Applies a Main Style pick to the door panel: a built-in style by name, or
/// a library style by id (its look and id are copied in).
fn pick_door_style(door: &mut DoorStyle, pick: StylePick, styles: &[LibraryStyle]) {
    match pick {
        StylePick::Builtin(name) => {
            door.apply_builtin(name);
        }
        StylePick::Library(id) => {
            if let Some(s) = styles.iter().find(|s| s.id == id) {
                door_styles::apply_door(s, door);
            }
        }
    }
}

/// The same for the drawer panel.
fn pick_drawer_style(drawer: &mut DrawerStyle, pick: StylePick, styles: &[LibraryStyle]) {
    match pick {
        StylePick::Builtin(name) => {
            drawer.apply_builtin(name);
        }
        StylePick::Library(id) => {
            if let Some(s) = styles.iter().find(|s| s.id == id) {
                door_styles::apply_drawer(s, drawer);
            }
        }
    }
}

/// The pattern, color, opacity and spacing of one plan fill.
fn fill_editor(ui: &mut Ui, f: &mut Fields, salt: &str, fill: &mut PlanFill) {
    row(ui, "Pattern", |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(fill.pattern.name())
            .show_ui(ui, |ui| {
                for p in FillPattern::ALL {
                    ui.selectable_value(&mut fill.pattern, p, p.name());
                }
            });
    });
    if fill.pattern != FillPattern::None {
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut fill.color);
        });
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut fill.alpha, 0.1..=1.0));
        });
        if matches!(fill.pattern, FillPattern::Hatch | FillPattern::CrossHatch) {
            f.length_row(ui, "Line Spacing", "cab_fill_gap", &mut fill.spacing);
            fill.spacing = fill.spacing.max(0.5);
        }
    }
}

/// A length that follows a default until it is specified: a check, then the
/// field. Unchecking returns it to following the default.
fn opt_length_row(
    ui: &mut Ui,
    f: &mut Fields,
    label: &str,
    key: &'static str,
    value: &mut Option<f64>,
    default: f64,
) {
    row(ui, label, |ui| {
        let mut own = value.is_some();
        if ui.checkbox(&mut own, "Specify").changed() {
            *value = own.then_some(default);
        }
        if let Some(v) = value.as_mut() {
            f.length(ui, key, v);
        } else {
            ui.weak(fmt_short(default));
        }
    });
}

/// A Slab / Shaker / Raised Panel choice.
fn profile_row(ui: &mut Ui, salt: &str, value: &mut DoorProfile) {
    row(ui, "Panel Profile", |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(value.name())
            .show_ui(ui, |ui| {
                for p in DoorProfile::ALL {
                    ui.selectable_value(value, p, p.name());
                }
            });
    });
}

// ----- the front elevation canvas -----

/// Paths of the leaf items in the order `FaceLayout::resolve` lists them.
pub fn leaf_paths(layout: &FaceLayout) -> Vec<Path> {
    fn walk(item: &FaceItem, path: &mut Path, out: &mut Vec<Path>) {
        match item {
            FaceItem::HorizontalLayout { cells, .. } => {
                for (j, c) in cells.iter().enumerate() {
                    path.push(j);
                    walk(&c.item, path, out);
                    path.pop();
                }
            }
            FaceItem::VerticalLayout { items, .. } => {
                for (j, c) in items.iter().enumerate() {
                    path.push(j);
                    walk(c, path, out);
                    path.pop();
                }
            }
            _ => out.push(path.clone()),
        }
    }
    let mut out = Vec::new();
    for (i, item) in layout.items.iter().enumerate() {
        walk(item, &mut vec![i], &mut out);
    }
    out
}

/// The leaf item at face point `(x, y)` (inches from the left and up from
/// the bottom of a `face_w` by `face_h` face).
pub fn path_at(layout: &FaceLayout, face_h: f64, face_w: f64, x: f64, y: f64) -> Option<Path> {
    let leaves = layout.resolve(face_h, face_w).ok()?;
    leaf_paths(layout)
        .into_iter()
        .zip(leaves)
        .find(|(_, r)| {
            let (rx, ry, rw, rh) = r.rect;
            x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
        })
        .map(|(p, _)| p)
}

/// Snap for dragged dividers, inches.
const DRAG_STEP: f64 = 0.125;

/// The front elevation of the face: items fill their rectangles, a click
/// selects one and dragging a divider between two items resizes them.
fn face_canvas(ui: &mut Ui, cab: &mut Cabinet, fh: f64, fw: f64, sel: &mut Path) {
    if fh <= 0.0 || fw <= 0.0 {
        ui.weak("This cabinet has no face.");
        return;
    }
    let avail = f64::from(ui.available_width().clamp(120.0, 420.0));
    let scale = ((avail - 8.0) / fw).min(220.0 / fh).max(0.5);
    let size = Vec2::new((fw * scale) as f32 + 8.0, (fh * scale) as f32 + 8.0);
    let (resp, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
    let origin = resp.rect.min + Vec2::splat(4.0);
    let at = |x: f64, y_top: f64| {
        Pos2::new(
            origin.x + (x * scale) as f32,
            origin.y + (y_top * scale) as f32,
        )
    };
    let ink = Stroke::new(1.0_f32, PV_INK);
    let selected = Stroke::new(2.5_f32, Color32::from_rgb(40, 110, 220));
    match cab.face.resolve(fh, fw) {
        Ok(leaves) => {
            for (path, r) in leaf_paths(&cab.face).iter().zip(&leaves) {
                let (x, y, w, h) = r.rect;
                let rect = Rect::from_two_pos(at(x, fh - (y + h)), at(x + w, fh - y));
                match &r.item {
                    FaceItem::Separation { .. } => {
                        painter.rect_filled(rect, 0.0, PV_WALL);
                    }
                    FaceItem::Opening { .. } => {
                        painter.rect_filled(rect, 0.0, Color32::from_gray(200));
                    }
                    FaceItem::Appliance { .. } => {
                        painter.rect_filled(rect, 0.0, PV_GLASS);
                    }
                    _ => {}
                }
                painter.rect_stroke(
                    rect,
                    0.0,
                    if path == sel { selected } else { ink },
                    StrokeKind::Inside,
                );
                if rect.height() > 12.0 && rect.width() > 36.0 {
                    pv_text(
                        &painter,
                        rect.center(),
                        Align2::CENTER_CENTER,
                        r.item.kind().label(),
                        9.0,
                    );
                }
            }
        }
        Err(e) => {
            pv_text(&painter, resp.rect.center(), Align2::CENTER_CENTER, e, 9.0);
            return;
        }
    }
    let dividers = cab.face.dividers(fh, fw).unwrap_or_default();
    let hit = |pos: Pos2| {
        dividers
            .iter()
            .find(|d| match d.divider {
                Divider::Horizontal { .. } => {
                    let y = at(0.0, d.pos).y;
                    (pos.y - y).abs() <= 4.0
                        && pos.x >= at(d.span.0, 0.0).x
                        && pos.x <= at(d.span.1, 0.0).x
                }
                Divider::Vertical { .. } => {
                    let x = at(d.pos, 0.0).x;
                    (pos.x - x).abs() <= 4.0
                        && pos.y >= at(0.0, d.span.0).y
                        && pos.y <= at(0.0, d.span.1).y
                }
            })
            .map(|d| d.divider)
    };
    let state_id = ui.id().with("cab_divider_drag");
    let (mut active, mut acc): (Option<Divider>, f64) = ui
        .memory(|m| m.data.get_temp(state_id))
        .unwrap_or((None, 0.0));
    if resp.drag_started() {
        active = ui.input(|i| i.pointer.press_origin()).and_then(hit);
        acc = 0.0;
    }
    if resp.dragged() {
        if let Some(div) = active {
            let px = match div {
                Divider::Horizontal { .. } => resp.drag_delta().y,
                Divider::Vertical { .. } => resp.drag_delta().x,
            };
            acc += f64::from(px) / scale;
            let step = (acc / DRAG_STEP).trunc() * DRAG_STEP;
            if step != 0.0 && cab.face.drag_divider(fh, fw, div, step).is_ok() {
                acc -= step;
            }
        }
    }
    if resp.drag_stopped() {
        active = None;
        acc = 0.0;
    }
    ui.memory_mut(|m| m.data.insert_temp(state_id, (active, acc)));
    if let Some(pos) = resp.hover_pos() {
        match active.or_else(|| hit(pos)) {
            Some(Divider::Horizontal { .. }) => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical)
            }
            Some(Divider::Vertical { .. }) => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal)
            }
            None => {}
        }
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let x = f64::from(pos.x - origin.x) / scale;
            let y = fh - f64::from(pos.y - origin.y) / scale;
            if let Some(p) = path_at(&cab.face, fh, fw, x, y) {
                *sel = p;
            }
        }
    }
}

/// One line of the face-item tree and its children.
fn tree_ui(ui: &mut Ui, item: &FaceItem, path: &mut Path, sel: &mut Path) {
    let text = format!("{}  {}", number(path), describe(item));
    if ui.selectable_label(sel == path, text).clicked() {
        *sel = path.clone();
    }
    let kids: Vec<&FaceItem> = match item {
        FaceItem::HorizontalLayout { cells, .. } => cells.iter().map(|c| &c.item).collect(),
        FaceItem::VerticalLayout { items, .. } => items.iter().collect(),
        _ => return,
    };
    ui.indent(("cab_tree", number(path)), |ui| {
        for (i, c) in kids.into_iter().enumerate() {
            path.push(i);
            tree_ui(ui, c, path, sel);
            path.pop();
        }
    });
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
        for sf in &d.sides {
            if sf.kind == SideKind::CustomFace {
                if let Err(e) = sf.layout.resolve(d.face_height(), d.side_width(sf.side)) {
                    return Some(format!("{} face items: {e}", sf.side.name()));
                }
            }
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
            Some("Accessories") => self.accessories(ui),
            Some("Opening Indicators") => self.indicators(ui),
            Some("Moldings") => self.moldings(ui),
            Some("Layer") => self.layer(ui),
            Some("Materials") => self.materials(ui),
            Some("Label") => self.label(ui),
            Some("Manufacturer") => self.manufacturer(ui),
            Some("Fill Style") => self.fill_style(ui),
            Some("Components") => self.components(ui),
            Some("Object Information") => self.object_information(ui),
            Some("Schedule") => self.schedule(ui),
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

pub(super) fn draw_preview(p: &Painter, area: Rect, cab: &Cabinet) {
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
            plan_cabinets::Stroke::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let sweep = (end - start).clamp(0.0, std::f64::consts::TAU);
                let pts: Vec<Pos2> = (0..=24)
                    .map(|i| {
                        let a = start + sweep * f64::from(i) / 24.0;
                        fit.pt(Point::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ))
                    })
                    .collect();
                p.add(egui::Shape::line(pts, ink));
            }
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
                    FaceItem::Panel { .. } => {
                        p.rect_stroke(r.shrink(0.5), 0.0, ink, StrokeKind::Inside);
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

// ----- Default Settings > Cabinets -----

use plan_core::defaults::{BoxDefaults, CabinetDefaults};

const DEFAULTS_TABS: &[Tab] = &[
    on("Base"),
    on("Wall"),
    on("Full Height"),
    on("Soffit"),
    on("Shelf"),
    on("Partition"),
    on("Countertop"),
    on("Backsplash"),
    on("Library Types"),
    on("Fillers and Corners"),
];

/// Edit > Default Settings > Cabinets: the sizes, styles and hardware every
/// new cabinet starts with, one page per kind (Base, Wall, Full Height,
/// Soffit, Shelf, Partition), the Countertop and Backsplash pages, the
/// library types (Vanity, Pantry, Tall Oven, Refrigerator) and the fillers
/// and corner cabinets. OK stores `draft()` in `PlanDefaults::cabinets`.
pub struct CabinetDefaultsDialog {
    frame: SpecDialog,
    form: DefaultsForm,
}

struct DefaultsForm {
    draft: CabinetDefaults,
    fields: Fields,
}

impl CabinetDefaultsDialog {
    pub fn new(defaults: &CabinetDefaults) -> Self {
        Self {
            frame: SpecDialog::new("Cabinet Defaults", "cabinet_defaults"),
            form: DefaultsForm {
                draft: defaults.clone(),
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &CabinetDefaults {
        &self.form.draft
    }

    /// Opens on the tab at index `tab` (Default Settings > Cabinets leaves).
    pub fn start_on(&mut self, tab: usize) {
        self.frame.start_on(tab.min(DEFAULTS_TABS.len() - 1));
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut CabinetDefaults {
        &mut self.form.draft
    }
}

/// Width, depth, height and elevation rows of a box-like kind. `keys` are
/// the four field keys.
fn box_rows(ui: &mut Ui, f: &mut Fields, d: &mut BoxDefaults, keys: [&'static str; 4], elev: &str) {
    f.length_row(ui, "Width", keys[0], &mut d.width);
    f.length_row(ui, "Depth", keys[1], &mut d.depth);
    f.length_row(ui, "Height", keys[2], &mut d.height);
    if !elev.is_empty() {
        f.length_row(ui, elev, keys[3], &mut d.elevation);
    }
}

/// A combo box over names, storing the chosen name.
fn name_combo(ui: &mut Ui, salt: &str, value: &mut String, names: &[&str]) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for n in names {
                if ui.selectable_label(value == n, *n).clicked() {
                    *value = (*n).to_string();
                }
            }
        });
}

impl DefaultsForm {
    fn base(&mut self, ui: &mut Ui) {
        let (f, b) = (&mut self.fields, &mut self.draft.base);
        section(ui, "Base Cabinet");
        f.length_row(ui, "Width", "dbase_w", &mut b.width);
        f.length_row(ui, "Depth", "dbase_d", &mut b.depth);
        f.length_row(ui, "Height (with countertop)", "dbase_h", &mut b.height);
        section(ui, "Countertop");
        f.length_row(ui, "Thickness", "dbase_ct", &mut b.countertop_thickness);
        f.length_row(ui, "Overhang Front", "dbase_ov", &mut b.countertop_overhang);
        section(ui, "Toe Kick");
        f.length_row(ui, "Height", "dbase_tkh", &mut b.toe_kick_height);
        f.length_row(ui, "Depth", "dbase_tkd", &mut b.toe_kick_depth);
        section(ui, "Door and Drawer Hardware");
        let door_names: Vec<&str> = DoorStyle::BUILTIN.iter().map(|(n, _)| *n).collect();
        let drawer_names: Vec<&str> = DrawerStyle::BUILTIN.iter().map(|(n, _)| *n).collect();
        row(ui, "Door Style", |ui| {
            name_combo(ui, "dbase_door", &mut b.door_style, &door_names)
        });
        row(ui, "Drawer Style", |ui| {
            name_combo(ui, "dbase_drawer", &mut b.drawer_style, &drawer_names)
        });
        let handles: Vec<&str> = HandleStyle::ALL.iter().map(|h| h.name()).collect();
        row(ui, "Handle", |ui| {
            name_combo(ui, "dbase_handle", &mut b.handle, &handles)
        });
    }

    fn wall(&mut self, ui: &mut Ui) {
        let (f, w) = (&mut self.fields, &mut self.draft.wall);
        section(ui, "Wall Cabinet");
        f.length_row(ui, "Width", "dwall_w", &mut w.width);
        f.length_row(ui, "Depth", "dwall_d", &mut w.depth);
        f.length_row(ui, "Height", "dwall_h", &mut w.height);
        f.length_row(ui, "Bottom From Floor", "dwall_e", &mut w.elevation);
    }

    fn full_height(&mut self, ui: &mut Ui) {
        let (f, c) = (&mut self.fields, &mut self.draft.full_height);
        section(ui, "Full Height Cabinet");
        f.length_row(ui, "Width", "dfull_w", &mut c.width);
        f.length_row(ui, "Depth", "dfull_d", &mut c.depth);
        f.length_row(ui, "Height", "dfull_h", &mut c.height);
    }

    fn countertop(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let (base, c) = (&mut self.draft.base, &mut self.draft.countertop);
        section(ui, "Countertop");
        f.length_row(ui, "Thickness", "dct_thick", &mut base.countertop_thickness);
        f.length_row(
            ui,
            "Overhang Front",
            "dct_front",
            &mut base.countertop_overhang,
        );
        f.length_row(ui, "Overhang Back", "dct_back", &mut c.overhang_back);
        f.length_row(ui, "Overhang Sides", "dct_sides", &mut c.overhang_sides);
        section(ui, "Edge");
        let edges: Vec<&str> = EdgeProfile::ALL.iter().map(|e| e.name()).collect();
        row(ui, "Edge Profile", |ui| {
            name_combo(ui, "dct_edge", &mut c.edge, &edges)
        });
        if !matches!(c.edge.as_str(), "Square" | "Waterfall") {
            f.length_row(ui, "Edge Size", "dct_edge_size", &mut c.edge_size);
        }
        section(ui, "Corners");
        row(ui, "Corner Treatment", |ui| {
            for k in CornerTreatment::ALL {
                ui.radio_value(&mut c.corner, k.name().to_string(), k.name());
            }
        });
        if c.corner != "None" {
            f.length_row(ui, "Corner Size", "dct_corner", &mut c.corner_size);
        }
    }

    fn backsplash(&mut self, ui: &mut Ui) {
        let (f, b) = (&mut self.fields, &mut self.draft.backsplash);
        section(ui, "Backsplash");
        ui.checkbox(&mut b.enabled, "New base cabinets get a backsplash");
        ui.checkbox(
            &mut b.full_height,
            "Full Height (to the wall cabinet above)",
        );
        if !b.full_height {
            f.length_row(ui, "Height", "dbs_h", &mut b.height);
        }
        f.length_row(ui, "Thickness", "dbs_t", &mut b.thickness);
    }

    fn library_types(&mut self, ui: &mut Ui) {
        let (f, d) = (&mut self.fields, &mut self.draft);
        section(ui, "Vanity Cabinet");
        box_rows(ui, f, &mut d.vanity, ["dv_w", "dv_d", "dv_h", ""], "");
        section(ui, "Pantry Cabinet");
        box_rows(ui, f, &mut d.pantry, ["dp_w", "dp_d", "dp_h", ""], "");
        section(ui, "Tall Oven Cabinet");
        box_rows(ui, f, &mut d.tall_oven, ["dt_w", "dt_d", "dt_h", ""], "");
        section(ui, "Refrigerator Cabinet");
        box_rows(ui, f, &mut d.refrigerator, ["dr_w", "dr_d", "dr_h", ""], "");
    }

    fn fillers_and_corners(&mut self, ui: &mut Ui) {
        let (f, d) = (&mut self.fields, &mut self.draft);
        section(ui, "Fillers");
        f.length_row(ui, "Starting Width", "dfill_w", &mut d.filler_width);
        section(ui, "Corner Cabinets");
        f.length_row(ui, "Corner Base Leg", "dcorner_b", &mut d.corner_base_leg);
        f.length_row(ui, "Corner Wall Leg", "dcorner_w", &mut d.corner_wall_leg);
        section(ui, "Blind Cabinets");
        f.length_row(ui, "Blind Base Width", "dblind_w", &mut d.blind_base_width);
        f.length_row(ui, "Hidden Width", "dblind_h", &mut d.blind_hidden_width);
    }
}

impl SpecPages for DefaultsForm {
    fn tabs(&self) -> &'static [Tab] {
        DEFAULTS_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = &self.draft;
        let sized = [
            (d.base.width, d.base.depth, d.base.height),
            (d.wall.width, d.wall.depth, d.wall.height),
            (
                d.full_height.width,
                d.full_height.depth,
                d.full_height.height,
            ),
            (d.soffit.width, d.soffit.depth, d.soffit.height),
            (d.shelf.width, d.shelf.depth, d.shelf.height),
            (d.partition.width, d.partition.depth, d.partition.height),
            (d.vanity.width, d.vanity.depth, d.vanity.height),
            (d.pantry.width, d.pantry.depth, d.pantry.height),
            (d.tall_oven.width, d.tall_oven.depth, d.tall_oven.height),
            (
                d.refrigerator.width,
                d.refrigerator.depth,
                d.refrigerator.height,
            ),
        ];
        if sized
            .iter()
            .any(|(w, dp, h)| *w < 1.0 || *dp <= 0.0 || *h <= 0.0)
        {
            return Some("Width, depth and height must be positive".into());
        }
        if d.base.countertop_thickness >= d.base.height {
            return Some("The countertop must be thinner than the base cabinet".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match DEFAULTS_TABS[tab].name {
            "Base" => self.base(ui),
            "Wall" => self.wall(ui),
            "Full Height" => self.full_height(ui),
            "Soffit" => {
                section(ui, "Soffit");
                box_rows(
                    ui,
                    &mut self.fields,
                    &mut self.draft.soffit,
                    ["dso_w", "dso_d", "dso_h", "dso_e"],
                    "Bottom From Floor",
                );
            }
            "Shelf" => {
                section(ui, "Shelf");
                box_rows(
                    ui,
                    &mut self.fields,
                    &mut self.draft.shelf,
                    ["dsh_w", "dsh_d", "dsh_h", "dsh_e"],
                    "Height From Floor",
                );
            }
            "Partition" => {
                section(ui, "Partition");
                box_rows(
                    ui,
                    &mut self.fields,
                    &mut self.draft.partition,
                    ["dpt_w", "dpt_d", "dpt_h", "dpt_e"],
                    "Bottom From Floor",
                );
            }
            "Countertop" => self.countertop(ui),
            "Backsplash" => self.backsplash(ui),
            "Library Types" => self.library_types(ui),
            "Fillers and Corners" => self.fillers_and_corners(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let d = &self.draft;
        let lines = [
            format!(
                "Base {} x {} x {}",
                fmt_short(d.base.width),
                fmt_short(d.base.depth),
                fmt_short(d.base.height)
            ),
            format!(
                "Wall {} x {} x {} at {}",
                fmt_short(d.wall.width),
                fmt_short(d.wall.depth),
                fmt_short(d.wall.height),
                fmt_short(d.wall.elevation)
            ),
            format!(
                "Full Height {} x {} x {}",
                fmt_short(d.full_height.width),
                fmt_short(d.full_height.depth),
                fmt_short(d.full_height.height)
            ),
            format!("Top edge: {}", d.countertop.edge),
            format!(
                "Backsplash: {}",
                if !d.backsplash.enabled {
                    "none".to_string()
                } else if d.backsplash.full_height {
                    "full height".to_string()
                } else {
                    fmt_short(d.backsplash.height)
                }
            ),
        ];
        for (i, text) in lines.iter().enumerate() {
            pv_text(
                painter,
                Pos2::new(rect.center().x, rect.min.y + 14.0 + 16.0 * i as f32),
                Align2::CENTER_CENTER,
                text,
                11.0,
            );
        }
    }
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
        assert_eq!(describe(item_at(&l, &[3]).unwrap()), "Door - Auto Right");
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
        assert!(set_item_type(&mut l, &[2], ItemKind::Appliance));
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
        set_item_type(&mut l, &[2], ItemKind::Appliance);
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
        assert_eq!(TABS.iter().filter(|t| t.enabled).count(), 15);

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

    #[test]
    fn leaf_paths_follow_the_resolved_order_and_clicks_find_items() {
        let mut l = base();
        split_horizontal(&mut l, &[3]).unwrap();
        let paths = leaf_paths(&l);
        assert_eq!(paths.len(), l.resolve(34.5, 24.0).unwrap().len());
        assert_eq!(paths[3], vec![3, 0]);
        assert_eq!(paths[4], vec![3, 1]);
        // The face is 24" wide and 34.5" tall; y runs up from the bottom.
        assert_eq!(path_at(&l, 34.5, 24.0, 6.0, 5.0), Some(vec![3, 0]));
        assert_eq!(path_at(&l, 34.5, 24.0, 18.0, 5.0), Some(vec![3, 1]));
        // The drawer is the 6" band under the top rail (y 27..33).
        assert_eq!(path_at(&l, 34.5, 24.0, 12.0, 30.0), Some(vec![1]));
        assert_eq!(path_at(&l, 34.5, 24.0, 99.0, 5.0), None);
    }

    #[test]
    fn split_then_drag_dividers_round_trips_the_face() {
        // Split the door in two columns, then resize both ways.
        let (fh, fw) = (34.5, 24.0);
        let mut l = base();
        split_horizontal(&mut l, &[3]).unwrap();
        // Add a second drawer below the first by splitting vertically.
        assert_eq!(split_vertical(&mut l, &[1]), Some(vec![1]));
        let before = l.resolve(fh, fw).unwrap();
        let handles = l.dividers(fh, fw).unwrap();
        let vertical: Vec<_> = handles
            .iter()
            .filter(|h| matches!(h.divider, Divider::Vertical { .. }))
            .collect();
        assert_eq!(vertical.len(), 1);
        l.drag_divider(fh, fw, vertical[0].divider, 4.0).unwrap();
        let horizontal: Vec<_> = l
            .dividers(fh, fw)
            .unwrap()
            .into_iter()
            .filter(|h| matches!(h.divider, Divider::Horizontal { .. }))
            .collect();
        assert!(!horizontal.is_empty());
        l.drag_divider(fh, fw, horizontal[0].divider, 1.0).unwrap();
        let resized = l.resolve(fh, fw).unwrap();
        // Total height and width are conserved.
        let col_h: f64 = resized
            .iter()
            .filter(|f| f.rect.0 == 0.0)
            .map(|f| f.rect.3)
            .sum();
        assert!((col_h - fh).abs() < 1e-9);
        let doors: Vec<_> = resized
            .iter()
            .filter(|f| matches!(f.item, FaceItem::DoorAuto { .. }))
            .collect();
        assert!((doors[0].rect.2 - 16.0).abs() < 1e-9 && (doors[1].rect.2 - 8.0).abs() < 1e-9);
        // Undo both drags: back to the equal split.
        l.drag_divider(fh, fw, horizontal[0].divider, -1.0).unwrap();
        l.drag_divider(fh, fw, vertical[0].divider, -4.0).unwrap();
        let back = l.resolve(fh, fw).unwrap();
        assert_eq!(before.len(), back.len());
        for (a, b) in before.iter().zip(&back) {
            for k in 0..4 {
                let (x, y) = (
                    [a.rect.0, a.rect.1, a.rect.2, a.rect.3][k],
                    [b.rect.0, b.rect.1, b.rect.2, b.rect.3][k],
                );
                assert!((x - y).abs() < 1e-9, "{:?} vs {:?}", a.rect, b.rect);
            }
        }
        // Add an opening, retype it as a panel, and keep resolving.
        let p = add_item(&mut l, &[1], FaceItem::Opening { height: 0.0 }).unwrap();
        assert!(set_item_type(&mut l, &p, ItemKind::DoorPanel));
        assert_eq!(describe(item_at(&l, &p).unwrap()), "Door Panel");
        assert!(l.resolve(fh, fw).is_ok());
    }

    fn nested() -> FaceLayout {
        FaceLayout {
            items: vec![
                FaceItem::Separation { height: 1.5 },
                FaceItem::VerticalLayout {
                    height: 0.0,
                    items: vec![
                        FaceItem::FalseDrawer { height: 6.0 },
                        FaceItem::HorizontalLayout {
                            height: 0.0,
                            cells: vec![
                                FaceCell {
                                    item: FaceItem::DoorLeft { height: 0.0 },
                                    width: None,
                                },
                                FaceCell {
                                    item: FaceItem::Rollout { height: 0.0 },
                                    width: None,
                                },
                            ],
                        },
                    ],
                },
                FaceItem::Separation { height: 1.5 },
            ],
            frame_width: 1.5,
        }
    }

    #[test]
    fn paths_reach_into_vertical_layouts_and_leaf_paths_follow_the_solver() {
        let l = nested();
        assert_eq!(item_at(&l, &[1, 0]).unwrap().kind(), ItemKind::FalseDrawer);
        assert_eq!(item_at(&l, &[1, 1, 1]).unwrap().kind(), ItemKind::Rollout);
        assert!(item_at(&l, &[0, 0]).is_none());
        let paths = leaf_paths(&l);
        assert_eq!(
            paths,
            vec![vec![0], vec![1, 0], vec![1, 1, 0], vec![1, 1, 1], vec![2]]
        );
        assert_eq!(paths.len(), l.resolve(34.5, 24.0).unwrap().len());
        // A click on the lower right of the face lands on the rollout.
        assert_eq!(path_at(&l, 34.5, 24.0, 20.0, 5.0), Some(vec![1, 1, 1]));
    }

    #[test]
    fn deleting_inside_a_vertical_layout_collapses_it() {
        let mut l = nested();
        let sel = delete_item(&mut l, &[1, 1]).unwrap();
        assert_eq!(sel, vec![1]);
        assert_eq!(l.items[1], FaceItem::FalseDrawer { height: 0.0 });
        let sel = add_item(&mut l, &[0], FaceItem::Opening { height: 0.0 }).unwrap();
        assert_eq!(sel, vec![1]);
    }

    #[test]
    fn retyping_keeps_the_items_own_settings_and_refuses_layouts() {
        let mut l = nested();
        {
            let it = item_at_mut(&mut l, &[1, 0]).unwrap();
            it.props_mut().unwrap().percent_open = Some(40.0);
        }
        assert!(set_item_type(&mut l, &[1, 0], ItemKind::Drawer));
        let it = item_at(&l, &[1, 0]).unwrap();
        assert_eq!(it.kind(), ItemKind::Drawer);
        assert_eq!(it.props().unwrap().percent_open, Some(40.0));
        assert_eq!(it.height(), 6.0);
        assert!(!set_item_type(&mut l, &[1], ItemKind::Drawer), "a layout");
        assert!(!set_item_type(&mut l, &[1, 0], ItemKind::VerticalLayout));
        assert_eq!(describe(item_at(&l, &[1, 0]).unwrap()), "Drawer");
    }

    #[test]
    fn the_cabinet_style_list_converts_and_explains_a_refusal() {
        let mut dlg = CabinetDialog::new(Cabinet::base(20.0));
        // A 20 in wide, 24 in deep cabinet cannot be a corner cabinet.
        let err = dlg.draft_mut().convert(CabinetStyle::Corner).unwrap_err();
        assert!(err.contains("greater than its depth"));
        dlg.draft_mut().width = 36.0;
        dlg.draft_mut()
            .convert(CabinetStyle::Special(plan_cabinets::SpecialShape::BowFront))
            .unwrap();
        // The General page shows the amount field for the shape.
        let drawn = cabinet_page_texts(&mut dlg, 0);
        assert!(drawn.iter().any(|t| t == "Bow Depth"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "Type"));
    }

    #[test]
    fn side_properties_and_the_manufacturer_page_draw() {
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        let front = TABS
            .iter()
            .position(|t| t.name == "Front/Sides/Back")
            .unwrap();
        let drawn = cabinet_page_texts(&mut dlg, front);
        assert!(drawn.iter().any(|t| t == "Left Stile"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "Auto Door Threshold"));
        let maker = TABS.iter().position(|t| t.name == "Manufacturer").unwrap();
        dlg.draft_mut().manufacturer = Some(plan_cabinets::Manufacturer::default());
        let drawn = cabinet_page_texts(&mut dlg, maker);
        assert!(drawn.iter().any(|t| t == "Web Site"), "{drawn:?}");
    }

    /// Every text a page draws.
    fn cabinet_page_texts(dlg: &mut CabinetDialog, tab: usize) -> Vec<String> {
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| texts(x, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
        });
        let mut all = Vec::new();
        for c in &out.shapes {
            texts(&c.shape, &mut all);
        }
        all
    }

    #[test]
    fn the_sides_and_back_tab_edits_the_other_faces() {
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        let tab = TABS
            .iter()
            .position(|t| t.name == "Front/Sides/Back")
            .unwrap();
        // The side list is live and starts on the front.
        let drawn = cabinet_page_texts(&mut dlg, tab);
        assert!(drawn.iter().any(|t| t == "Front"), "{drawn:?}");
        // Switch to the back, make it a custom face: one door by default.
        dlg.form.side = FaceSide::Back;
        dlg.form.draft.set_side_face(
            FaceSide::Back,
            SideKind::CustomFace,
            FaceLayout::single_door(),
        );
        let front_items = dlg.form.draft.face.items.len();
        let drawn = cabinet_page_texts(&mut dlg, tab);
        assert!(drawn.iter().any(|t| t == "Custom Face"), "{drawn:?}");
        // The front layout was handed back untouched.
        assert_eq!(dlg.form.draft.face.items.len(), front_items);
        let back = dlg.form.draft.side_face(FaceSide::Back).unwrap();
        assert_eq!(back.layout.items.len(), 1);
        // Edit the back's layout the way the buttons do (through `draft.face`).
        let mut layout = back.layout.clone();
        assert!(add_item(&mut layout, &[0], FaceItem::Drawer { height: 6.0 }).is_some());
        dlg.form
            .draft
            .set_side_face(FaceSide::Back, SideKind::CustomFace, layout);
        assert_eq!(
            dlg.form
                .draft
                .side_face(FaceSide::Back)
                .unwrap()
                .layout
                .items
                .len(),
            2
        );
        assert!(dlg.form.error().is_none());
        // A layout that cannot resolve is reported against its face.
        dlg.form.draft.set_side_face(
            FaceSide::Left,
            SideKind::CustomFace,
            FaceLayout {
                items: vec![FaceItem::Drawer { height: 99.0 }],
                frame_width: 1.5,
            },
        );
        assert!(dlg.form.error().unwrap().starts_with("Left face items"));
    }

    #[test]
    fn countertop_corner_treatment_and_edge_are_live_on_the_general_tab() {
        let mut dlg = CabinetDialog::new(Cabinet::base(36.0));
        let drawn = cabinet_page_texts(&mut dlg, 0);
        for want in ["Corner Treatment", "Clipped", "Rounded", "Edge Profile"] {
            assert!(drawn.iter().any(|t| t == want), "{want} in {drawn:?}");
        }
        let t = dlg.form.draft.countertop.as_mut().unwrap();
        t.corner = CornerTreatment::Rounded;
        t.edge = EdgeProfile::Ogee;
        let drawn = cabinet_page_texts(&mut dlg, 0);
        assert!(drawn.iter().any(|t| t == "Corner Size"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "Edge Size"), "{drawn:?}");
    }

    #[test]
    fn every_kind_opens_the_dialog_on_every_tab_and_previews() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for kind in CabinetKind::ALL {
                    let mut cab = Cabinet::new(kind, 36.0);
                    cab.indicators = true;
                    cab.moldings = vec![Molding::crown(), Molding::light_rail()];
                    if kind.is_base_like() {
                        cab.add_cutout(CutoutKind::Sink);
                    }
                    let mut dlg = CabinetDialog::new(cab);
                    dlg.form.sel = vec![0];
                    for tab in 0..TABS.len() {
                        dlg.form.page(ui, tab);
                    }
                    let (_, painter) =
                        ui.allocate_painter(Vec2::new(220.0, 400.0), egui::Sense::hover());
                    dlg.form.preview(&painter, painter.clip_rect());
                }
            });
        });
        // Corner, blind and appliance cabinets validate with their own face width.
        let blind = Cabinet::blind_base(48.0, 15.0, plan_cabinets::BlindSide::Left);
        assert_eq!(CabinetDialog::new(blind).form.face_width(), 33.0);
        assert!(CabinetDialog::new(Cabinet::corner_base(36.0))
            .form
            .error()
            .is_none());
        assert!(CabinetDialog::new(Cabinet::dishwasher_opening())
            .form
            .error()
            .is_none());
        let filler = Cabinet::filler(CabinetKind::BaseFiller, 3.0);
        assert!(CabinetDialog::new(filler).form.error().is_none());
    }

    #[test]
    fn style_materials_and_moldings_edit_the_draft() {
        let mut d = Cabinet::base(24.0);
        // Built-in styles set the panel profile.
        assert!(d.door_style.apply_builtin("Shaker Door"));
        assert!(d.drawer_style.apply_builtin("Raised Panel Drawer"));
        assert_eq!(d.door_style.profile, DoorProfile::Shaker);
        assert_eq!(d.drawer_style.profile, DoorProfile::Raised);
        d.materials.countertop = MaterialChoice::Stone;
        d.moldings.push(Molding::crown());
        let json = serde_json::to_string(&d).unwrap();
        let back: Cabinet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        // The dialog's apply path keeps all of it.
        let dlg = CabinetDialog::new(d.clone());
        assert_eq!(dlg.draft(), &d);
    }

    fn defaults_page_texts(dlg: &mut CabinetDefaultsDialog, tab: usize) -> Vec<String> {
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| texts(x, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
        });
        let mut all = Vec::new();
        for c in &out.shapes {
            texts(&c.shape, &mut all);
        }
        all
    }

    #[test]
    fn the_cabinet_defaults_dialog_has_a_page_for_every_kind() {
        let defaults = plan_core::PlanDefaults::chief_x18_daniel().cabinets;
        let mut dlg = CabinetDefaultsDialog::new(&defaults);
        let names: Vec<_> = DEFAULTS_TABS.iter().map(|t| t.name).collect();
        for want in [
            "Base",
            "Wall",
            "Full Height",
            "Soffit",
            "Shelf",
            "Partition",
            "Countertop",
            "Backsplash",
        ] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        let wants = [
            ("Base", "Door Style"),
            ("Wall", "Bottom From Floor"),
            ("Full Height", "Full Height Cabinet"),
            ("Soffit", "Bottom From Floor"),
            ("Shelf", "Height From Floor"),
            ("Partition", "Partition"),
            ("Countertop", "Edge Profile"),
            ("Backsplash", "Thickness"),
            ("Library Types", "Refrigerator Cabinet"),
            ("Fillers and Corners", "Hidden Width"),
        ];
        for (tab, want) in wants {
            let i = names.iter().position(|n| *n == tab).unwrap();
            let texts = defaults_page_texts(&mut dlg, i);
            assert!(texts.iter().any(|t| t.contains(want)), "{tab}: {texts:?}");
        }
        assert!(dlg.form.error().is_none());
        // Edits land in the draft, bad sizes are refused.
        dlg.draft_mut().shelf.elevation = 60.0;
        dlg.draft_mut().backsplash.enabled = true;
        assert_eq!(dlg.draft().shelf.elevation, 60.0);
        dlg.draft_mut().vanity.width = 0.0;
        assert!(dlg.form.error().is_some());
        dlg.draft_mut().vanity.width = 30.0;
        dlg.draft_mut().base.countertop_thickness = 40.0;
        assert!(dlg.form.error().is_some());
        // The preview draws.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, p) = ui.allocate_painter(Vec2::new(200.0, 120.0), egui::Sense::hover());
                dlg.form.preview(&p, p.clip_rect());
            });
        });
    }

    #[test]
    fn the_specification_offers_full_height_backsplash_hardware_and_3d_indicators() {
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        let tab = |name: &str| TABS.iter().position(|t| t.name == name).unwrap();
        dlg.form.draft.backsplash = Some(Backsplash::new(4.0, 0.5));
        let general = cabinet_page_texts(&mut dlg, tab("General"));
        assert!(
            general.iter().any(|t| t.contains("Full Height")),
            "{general:?}"
        );
        dlg.form.draft.countertop.as_mut().unwrap().edge = EdgeProfile::Waterfall;
        let general = cabinet_page_texts(&mut dlg, tab("General"));
        assert!(general.iter().any(|t| t.contains("runs down to the floor")));
        let ind = cabinet_page_texts(&mut dlg, tab("Opening Indicators"));
        assert!(ind.iter().any(|t| t.contains("3D")), "{ind:?}");
        dlg.form.draft.door_style.handle = HandleStyle::Pull;
        dlg.form.draft.drawer_style.handle_centered = false;
        let dd = cabinet_page_texts(&mut dlg, tab("Door/Drawer"));
        assert!(dd.iter().any(|t| t.contains("Pull Length")), "{dd:?}");
        assert!(dd.iter().any(|t| t.contains("Distance From Top")));
        let label = cabinet_page_texts(&mut dlg, tab("Label"));
        assert!(label.iter().any(|t| t.contains("Label position")));
        assert_eq!(HandleStyle::ALL.len(), 5);
        assert_eq!(handle_name(HandleStyle::Cup), "Cup Pull");
    }

    #[test]
    fn every_tab_of_the_specification_is_live_and_the_new_ones_show_their_content() {
        assert!(TABS.iter().all(|t| t.enabled), "no tab is greyed out");
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        let tab = |name: &str| TABS.iter().position(|t| t.name == name).unwrap();
        let has = |texts: &[String], want: &str| texts.iter().any(|t| t.contains(want));
        // Components: the box and its parts, with counts and materials.
        let comps = cabinet_page_texts(&mut dlg, tab("Components"));
        for want in ["Component", "Box side", "Countertop", "Toe kick"] {
            assert!(has(&comps, want), "{want} in {comps:?}");
        }
        // Object Information: the facts, then the product fields.
        let info = cabinet_page_texts(&mut dlg, tab("Object Information"));
        for want in ["Base Cabinet", "Manufacturer", "Model Number", "Notes"] {
            assert!(has(&info, want), "{want} in {info:?}");
        }
        // Schedule: the checkbox and the row.
        let sched = cabinet_page_texts(&mut dlg, tab("Schedule"));
        for want in ["List this cabinet in the Cabinet Schedule", "Door Style"] {
            assert!(has(&sched, want), "{want} in {sched:?}");
        }
        dlg.form.draft.in_schedule = false;
        let sched = cabinet_page_texts(&mut dlg, tab("Schedule"));
        assert!(has(&sched, "Left out of the schedule"), "{sched:?}");
        // Fill Style: a pattern first, then colour and spacing once chosen.
        let fill = cabinet_page_texts(&mut dlg, tab("Fill Style"));
        assert!(has(&fill, "Pattern") && !has(&fill, "Opacity"), "{fill:?}");
        dlg.form.draft.fill.pattern = FillPattern::Hatch;
        let fill = cabinet_page_texts(&mut dlg, tab("Fill Style"));
        for want in ["Color", "Opacity", "Line Spacing"] {
            assert!(has(&fill, want), "{want} in {fill:?}");
        }
        // Accessories: pilasters, feet (a base has a toe kick) and end panels.
        let acc = cabinet_page_texts(&mut dlg, tab("Accessories"));
        for want in [
            "Front Pilaster",
            "Foot Style",
            "Finished panel on the left end",
        ] {
            assert!(has(&acc, want), "{want} in {acc:?}");
        }
        dlg.form.draft.accessories.pilaster = PilasterStyle::Fluted;
        let acc = cabinet_page_texts(&mut dlg, tab("Accessories"));
        assert!(has(&acc, "Place On") && has(&acc, "Width"), "{acc:?}");
        // A wall cabinet has no toe kick: feet say so.
        let mut wall = CabinetDialog::new(Cabinet::new(CabinetKind::Wall, 30.0));
        let acc = cabinet_page_texts(&mut wall, tab("Accessories"));
        assert!(has(&acc, "give the cabinet one"), "{acc:?}");
    }

    #[test]
    fn components_follow_the_accessories_and_the_face() {
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        let comps = |dlg: &CabinetDialog| plan_cabinets::components(&dlg.form.draft);
        assert!(comps(&dlg).iter().all(|c| !c.name.contains("Foot")));
        dlg.form.draft.accessories.feet = FootStyle::Block;
        assert!(comps(&dlg)
            .iter()
            .any(|c| c.name == "Block Foot" && c.count == 4));
        // Finished end panels replace the plain sides in the list.
        dlg.form
            .draft
            .set_side_face(FaceSide::Left, SideKind::Finished, FaceLayout::empty());
        let c = comps(&dlg);
        assert_eq!(c.iter().find(|c| c.name == "Box side").unwrap().count, 1);
        assert_eq!(
            c.iter()
                .find(|c| c.name == "Finished end panel")
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn a_library_style_pick_copies_its_look_and_a_built_in_pick_clears_it() {
        let styles = vec![
            LibraryStyle {
                id: "chief.abc.7".into(),
                name: "Glass Shaker Door".into(),
                source: "Core".into(),
                profile: DoorProfile::Shaker,
                glass: true,
                drawer_only: false,
            },
            LibraryStyle {
                id: "user.drawer.1".into(),
                name: "Cup Front".into(),
                source: "User Library".into(),
                profile: DoorProfile::Raised,
                glass: false,
                drawer_only: true,
            },
        ];
        let mut cab = Cabinet::base(24.0);
        pick_door_style(
            &mut cab.door_style,
            StylePick::Library("chief.abc.7".into()),
            &styles,
        );
        assert_eq!(cab.door_style.name, "Glass Shaker Door");
        assert_eq!(cab.door_style.library, "chief.abc.7");
        assert_eq!(cab.door_style.profile, DoorProfile::Shaker);
        assert!(cab.door_style.glass);
        // The 3D door builds from the profile and the glass flag.
        assert!(!plan_cabinets::meshes(&cab).is_empty());
        pick_door_style(
            &mut cab.door_style,
            StylePick::Builtin("Slab Door"),
            &styles,
        );
        assert_eq!(cab.door_style.library, "");
        assert_eq!(cab.door_style.profile, DoorProfile::Slab);
        // A drawer-only style is still offered for drawer fronts.
        pick_drawer_style(
            &mut cab.drawer_style,
            StylePick::Library("user.drawer.1".into()),
            &styles,
        );
        assert_eq!(cab.drawer_style.library, "user.drawer.1");
        assert_eq!(cab.drawer_style.profile, DoorProfile::Raised);
        // An unknown id changes nothing.
        let before = cab.drawer_style.clone();
        pick_drawer_style(
            &mut cab.drawer_style,
            StylePick::Library("nope".into()),
            &styles,
        );
        assert_eq!(cab.drawer_style, before);
        // The dialog lists what the library has: door styles for the door.
        let doors = door_styles::doors(&styles);
        assert_eq!(doors.len(), 1);
        // And the Door/Drawer tab draws with styles loaded.
        let mut dlg = CabinetDialog::new(Cabinet::base(24.0));
        dlg.form.styles = styles;
        let tab = TABS.iter().position(|t| t.name == "Door/Drawer").unwrap();
        let texts = cabinet_page_texts(&mut dlg, tab);
        assert!(texts.iter().any(|t| t.contains("Main Style")), "{texts:?}");
    }

    #[test]
    fn merging_joins_an_item_with_the_next_one() {
        // Two fixed-height doors: the merged door is as tall as both.
        let mut l = FaceLayout {
            items: vec![
                FaceItem::DoorAuto { height: 10.0 },
                FaceItem::Separation { height: 1.5 },
                FaceItem::Drawer { height: 6.0 },
            ],
            frame_width: 1.5,
        };
        let p = merge_with_next(&mut l, &[0]).unwrap();
        assert_eq!(p, vec![0]);
        assert_eq!(l.items.len(), 2);
        assert_eq!(l.items[0], FaceItem::DoorAuto { height: 11.5 });
        // With an auto item in the pair the merge is auto too.
        let mut l = FaceLayout {
            items: vec![
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Drawer { height: 6.0 },
            ],
            frame_width: 1.5,
        };
        merge_with_next(&mut l, &[0]).unwrap();
        assert_eq!(l.items, vec![FaceItem::DoorAuto { height: 0.0 }]);
        // The last item has nothing to merge with.
        assert!(merge_with_next(&mut l, &[0]).is_none());
        // Cells of a horizontal layout: widths add; one cell left collapses.
        let mut l = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: Some(10.0),
                    },
                    FaceCell {
                        item: FaceItem::DoorRight { height: 0.0 },
                        width: Some(8.0),
                    },
                ],
            }],
            frame_width: 1.5,
        };
        let p = merge_with_next(&mut l, &[0, 0]).unwrap();
        assert_eq!(p, vec![0], "the layout became its remaining cell");
        assert_eq!(l.items, vec![FaceItem::DoorLeft { height: 0.0 }]);
        // Splitting then merging puts the face back.
        let mut l = base();
        let before = l.clone();
        let at = l
            .items
            .iter()
            .position(|i| !matches!(i, FaceItem::Separation { .. }))
            .unwrap();
        let sel = split_vertical(&mut l, &[at]).unwrap();
        assert_eq!(sel.len(), 1);
        assert_eq!(l.items.len(), before.items.len() + 1);
        let merged = merge_with_next(&mut l, &[at]).unwrap();
        assert_eq!(merged, vec![at]);
        assert_eq!(l.items.len(), before.items.len());
    }
}
