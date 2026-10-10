//! Material Layers Definition (Round 16, brief 13; manual pp. 769-770 and
//! 1088-1091): the layer table behind a floor or ceiling platform, a finish
//! or a roof plane, and the Floor/Ceiling Platform Defaults page.
//!
//! [`AssemblyDefDialog`] edits one [`Assembly`]: a table of layers (material,
//! fill for Auto Detail, role, thickness) with Insert / Delete / Move
//! buttons, the framing options of a Framing layer (joists or trusses, lumber,
//! I-joist or hat channel, width, spacing), the energy values, the total
//! thickness and a cross-section preview. It is a window the owner draws
//! while it is open: Floor Defaults, Room Specification, the roof plane
//! Structure panel and the plan-wide page below open it from their Edit
//! buttons and take the result on OK.
//!
//! [`PlatformEditor`] draws the four rows (Floor Structure, Floor Finish,
//! Ceiling Structure, Ceiling Finish) with their total depth, an Edit button
//! and a Use Default box, and owns the open definition window.
//! [`PlatformDefaultsPage`] is Edit > Default Settings > Floor/Ceiling
//! Platform: the plan-wide level of the chain (see `plan_core::assemblies`);
//! OK is one undo step.

use super::{fmt_short, row, section, Outcome, ERROR_RED};
use crate::editor::EditorContext;
use eframe::egui::{
    self, Align, Align2, Color32, Key, Layout, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind,
    Ui, Vec2,
};
use plan_core::assemblies::{
    resolve_floor, Assembly, AssemblyKind, AssemblyLayer, AssemblyLibrary, AssemblySlot,
    FramingConstruction, FramingMethod, FramingSpec, LayerRole, NamedAssembly, PlatformAssemblies,
};
use plan_core::floors::FloorSettings;
use plan_core::units::parse_ft_in;
use std::cell::RefCell;

/// Materials the picker offers for any definition.
const COMMON_MATERIALS: &[&str] = &[
    "Plywood Subfloor",
    "OSB Subfloor",
    "Floor Joist",
    "Ceiling Joist",
    "Rafter",
    "Hat Channel",
    "Drywall",
    "Backerboard",
    "Thinset Mortar",
    "Ceramic Tile",
    "Hardwood",
    "Carpet",
    "Vinyl",
    "Concrete",
    "Insulation",
    "Roof Sheathing",
    "Asphalt Shingles",
    "Plenum",
];

/// Fills Auto Detail can draw a layer with.
const FILLS: &[&str] = &[
    "",
    "Wood",
    "Plywood",
    "Framing",
    "Insulation",
    "Gypsum",
    "Concrete",
    "Tile",
    "Air",
];

/// One open Material Layers Definition window.
pub struct AssemblyDefDialog {
    kind: AssemblyKind,
    assembly: Assembly,
    selected: usize,
    /// Definitions saved for this kind, offered by the Library list.
    saved: Vec<NamedAssembly>,
    save_name: String,
    /// Definitions the user saved while the window was open; the owner puts
    /// them in the plan's library on OK.
    pending_saves: Vec<NamedAssembly>,
}

// The mutators are the dialog's model API (the tests drive it); the UI edits
// the same fields directly.
#[allow(dead_code)]
impl AssemblyDefDialog {
    pub fn new(kind: AssemblyKind, assembly: Assembly, library: &AssemblyLibrary) -> Self {
        Self {
            kind,
            assembly,
            selected: 0,
            saved: library
                .named
                .iter()
                .filter(|n| n.kind == kind)
                .cloned()
                .collect(),
            save_name: String::new(),
            pending_saves: Vec::new(),
        }
    }

    pub fn kind(&self) -> AssemblyKind {
        self.kind
    }

    pub fn assembly(&self) -> &Assembly {
        &self.assembly
    }

    pub fn into_assembly(self) -> Assembly {
        self.assembly
    }

    pub fn pending_saves(&self) -> &[NamedAssembly] {
        &self.pending_saves
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, i: usize) {
        if i < self.assembly.layers.len() {
            self.selected = i;
        }
    }

    /// Insert Layer: a new Standard layer above the selected one (at the
    /// end for an empty table); it becomes the selection.
    pub fn insert_layer(&mut self) -> usize {
        let at = if self.assembly.is_empty() {
            0
        } else {
            self.selected.min(self.assembly.layers.len())
        };
        let layer = AssemblyLayer::new("New Layer", LayerRole::Standard, 0.5);
        self.selected = self.assembly.insert(at, layer);
        self.selected
    }

    /// Delete Layer: the selected one.
    pub fn delete_layer(&mut self) -> bool {
        let ok = self.assembly.remove(self.selected);
        self.selected = self
            .selected
            .min(self.assembly.layers.len().saturating_sub(1));
        ok
    }

    pub fn move_up(&mut self) -> bool {
        match self.assembly.move_up(self.selected) {
            Some(i) => {
                self.selected = i;
                true
            }
            None => false,
        }
    }

    pub fn move_down(&mut self) -> bool {
        match self.assembly.move_down(self.selected) {
            Some(i) => {
                self.selected = i;
                true
            }
            None => false,
        }
    }

    pub fn layer_mut(&mut self, i: usize) -> Option<&mut AssemblyLayer> {
        self.assembly.layers.get_mut(i)
    }

    pub fn set_role(&mut self, i: usize, role: LayerRole) {
        if let Some(l) = self.layer_mut(i) {
            l.set_role(role);
        }
    }

    pub fn total_thickness(&self) -> f64 {
        self.assembly.total_thickness()
    }

    /// Back to the built-in definition of the kind.
    pub fn reset(&mut self) {
        self.assembly = self.kind.builtin();
        self.selected = 0;
    }

    /// Takes the saved definition `name` of this kind.
    pub fn load_saved(&mut self, name: &str) -> bool {
        let found = self
            .pending_saves
            .iter()
            .chain(self.saved.iter())
            .find(|n| n.name == name)
            .map(|n| n.assembly.clone());
        match found {
            Some(a) => {
                self.assembly = a;
                self.selected = 0;
                true
            }
            None => false,
        }
    }

    /// Saves the table under `name` for reuse in this plan.
    pub fn save_as(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.assembly.is_empty() {
            return false;
        }
        self.pending_saves.retain(|n| n.name != name);
        self.pending_saves.push(NamedAssembly {
            name: name.to_string(),
            kind: self.kind,
            assembly: self.assembly.clone(),
        });
        true
    }

    /// Why OK is off, if it is.
    pub fn error(&self) -> Option<String> {
        self.assembly.error()
    }

    /// The layers of the cross-section preview as `(top, bottom)` fractions
    /// of the total thickness, in table order.
    pub fn preview_spans(&self) -> Vec<(f32, f32)> {
        let total = self.assembly.total_thickness().max(1e-9);
        let mut y = 0.0;
        self.assembly
            .layers
            .iter()
            .map(|l| {
                let t = l.thickness.max(0.0);
                let span = ((y / total) as f32, ((y + t) / total) as f32);
                y += t;
                span
            })
            .collect()
    }

    /// Draws the window; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        let title = self.kind.dialog_title();
        egui::Window::new(title)
            .id(egui::Id::new(("assembly_def", self.kind.label())))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([640.0, 520.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(440.0);
                        self.table(ui);
                        ui.add_space(4.0);
                        self.buttons(ui);
                        self.options(ui);
                    });
                    ui.add_space(8.0);
                    ui.vertical(|ui| self.preview(ui));
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Total Thickness {}",
                        fmt_short(self.total_thickness())
                    ));
                    ui.separator();
                    ui.label(format!("R-Value {:.1}", self.assembly.r_value()));
                });
                self.library(ui);
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new("   OK   ");
                    if ui.add_enabled(error.is_none(), ok).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = &error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        // The window owns Enter and Escape whatever the owner under it does.
        let esc = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let enter = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        if esc {
            outcome = Outcome::Cancel;
        } else if enter && error.is_none() {
            outcome = Outcome::Ok;
        }
        outcome
    }

    fn table(&mut self, ui: &mut Ui) {
        let mut select = None;
        egui::ScrollArea::vertical()
            .id_salt(("assembly_def_rows", self.kind.label()))
            .max_height(220.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new(("assembly_def_grid", self.kind.label()))
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        for h in ["Layer", "Material", "Fill", "Role", "Thickness"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for (i, l) in self.assembly.layers.iter_mut().enumerate() {
                            if ui
                                .selectable_label(self.selected == i, format!("{}", i + 1))
                                .clicked()
                            {
                                select = Some(i);
                            }
                            ui.horizontal(|ui| {
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut l.material)
                                        .desired_width(140.0),
                                );
                                if r.gained_focus() {
                                    select = Some(i);
                                }
                                suggest(ui, ("mat", i), COMMON_MATERIALS, &mut l.material);
                            });
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(&mut l.fill).desired_width(70.0));
                                suggest(ui, ("fill", i), FILLS, &mut l.fill);
                            });
                            let mut role = l.role;
                            egui::ComboBox::from_id_salt(("assembly_role", i))
                                .width(96.0)
                                .selected_text(role.label())
                                .show_ui(ui, |ui| {
                                    for r in LayerRole::ALL {
                                        ui.selectable_value(&mut role, r, r.label());
                                    }
                                });
                            if role != l.role {
                                l.set_role(role);
                                select = Some(i);
                            }
                            let r = ui.add(length_drag(&mut l.thickness));
                            if r.gained_focus() {
                                select = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                if self.assembly.is_empty() {
                    ui.weak("No layers. Use Insert Layer to add one.");
                }
            });
        if let Some(i) = select {
            self.selected = i;
        }
    }

    fn buttons(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Insert Layer").clicked() {
                self.insert_layer();
            }
            let has = self.selected < self.assembly.layers.len();
            if ui
                .add_enabled(has, egui::Button::new("Delete Layer"))
                .clicked()
            {
                self.delete_layer();
            }
            if ui
                .add_enabled(self.selected > 0 && has, egui::Button::new("Move Up"))
                .clicked()
            {
                self.move_up();
            }
            if ui
                .add_enabled(
                    has && self.selected + 1 < self.assembly.layers.len(),
                    egui::Button::new("Move Down"),
                )
                .clicked()
            {
                self.move_down();
            }
            if ui.button("Built-in").clicked() {
                self.reset();
            }
        });
    }

    /// The options of the selected layer: framing (Framing role) and energy.
    fn options(&mut self, ui: &mut Ui) {
        let Some(l) = self.assembly.layers.get_mut(self.selected) else {
            return;
        };
        section(ui, &format!("Layer {} Options", self.selected + 1));
        if l.role == LayerRole::Framing {
            let f = l.framing.get_or_insert_with(FramingSpec::default);
            row(ui, "Framing Method", |ui| {
                egui::ComboBox::from_id_salt("assembly_method")
                    .selected_text(f.method.label())
                    .show_ui(ui, |ui| {
                        for m in FramingMethod::ALL {
                            ui.selectable_value(&mut f.method, m, m.label());
                        }
                    });
            });
            row(ui, "Construction", |ui| {
                egui::ComboBox::from_id_salt("assembly_construction")
                    .selected_text(f.construction.label())
                    .show_ui(ui, |ui| {
                        for c in FramingConstruction::ALL {
                            ui.selectable_value(&mut f.construction, c, c.label());
                        }
                    });
            });
            row(ui, "Member Width", |ui| ui.add(length_drag(&mut f.width)));
            row(ui, "Spacing On Center", |ui| {
                ui.add(length_drag(&mut f.spacing))
            });
            ui.weak("The layer's thickness is the member depth.");
        } else {
            ui.weak("Set the Role to Framing for joist, truss and spacing options.");
        }
        row(ui, "R-Value, Cavity", |ui| {
            ui.add(
                egui::DragValue::new(&mut l.r_cavity)
                    .speed(0.1)
                    .range(0.0..=100.0),
            )
        });
        row(ui, "R-Value, Continuous", |ui| {
            ui.add(
                egui::DragValue::new(&mut l.r_continuous)
                    .speed(0.1)
                    .range(0.0..=100.0),
            )
        });
    }

    fn library(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Library");
            let mut pick: Option<String> = None;
            egui::ComboBox::from_id_salt(("assembly_library", self.kind.label()))
                .width(160.0)
                .selected_text("Choose a saved definition")
                .show_ui(ui, |ui| {
                    for n in self.pending_saves.iter().chain(self.saved.iter()) {
                        if ui.selectable_label(false, &n.name).clicked() {
                            pick = Some(n.name.clone());
                        }
                    }
                });
            if let Some(name) = pick {
                self.load_saved(&name);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.save_name)
                    .hint_text("Name")
                    .desired_width(110.0),
            );
            let name = self.save_name.clone();
            if ui
                .add_enabled(!name.trim().is_empty(), egui::Button::new("Save"))
                .clicked()
                && self.save_as(&name)
            {
                self.save_name.clear();
            }
        });
    }

    /// The cross-section: the layers stacked top first, scaled by thickness.
    fn preview(&self, ui: &mut Ui) {
        ui.strong("Cross Section");
        let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0, 260.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, super::PV_BG);
        let inner = rect.shrink(10.0);
        let n = self.assembly.layers.len();
        if n == 0 {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No layers",
                egui::FontId::proportional(12.0),
                super::PV_FAINT,
            );
            return;
        }
        // Thin layers keep a readable height; the rest share what is left.
        const MIN_H: f32 = 9.0;
        let spans = self.preview_spans();
        let mut heights: Vec<f32> = spans
            .iter()
            .map(|(a, b)| (b - a) * inner.height())
            .collect();
        let short: f32 = heights.iter().filter(|h| **h < MIN_H).count() as f32 * MIN_H;
        let long: f32 = heights.iter().filter(|h| **h >= MIN_H).sum();
        let scale = if long > 0.0 {
            ((inner.height() - short) / long).clamp(0.05, 1.0)
        } else {
            1.0
        };
        for h in &mut heights {
            *h = if *h < MIN_H { MIN_H } else { *h * scale };
        }
        let mut y = inner.top();
        for (i, l) in self.assembly.layers.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(inner.left(), y),
                Vec2::new(inner.width(), heights[i]),
            );
            let selected = i == self.selected;
            if l.role == LayerRole::AirGap {
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.0_f32, super::PV_FAINT),
                    StrokeKind::Inside,
                );
                painter.line_segment(
                    [r.left_top(), r.right_bottom()],
                    Stroke::new(0.5_f32, super::PV_FAINT),
                );
                painter.line_segment(
                    [r.left_bottom(), r.right_top()],
                    Stroke::new(0.5_f32, super::PV_FAINT),
                );
            } else {
                painter.rect_filled(r, 0.0, layer_color(l));
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(
                        if selected { 2.0_f32 } else { 1.0_f32 },
                        if selected {
                            super::PV_ACCENT
                        } else {
                            super::PV_INK
                        },
                    ),
                    StrokeKind::Inside,
                );
            }
            if heights[i] >= 11.0 {
                painter.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    &l.material,
                    egui::FontId::proportional(10.0),
                    super::PV_INK,
                );
            }
            y += heights[i];
        }
    }
}

/// A feet-and-inches drag value.
fn length_drag(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .speed(0.0625)
        .range(0.0..=480.0)
        .custom_formatter(|n, _| fmt_short(n))
        .custom_parser(parse_ft_in)
}

/// A small menu of suggestions that fills `target`.
fn suggest(ui: &mut Ui, id: (&str, usize), names: &[&str], target: &mut String) {
    ui.push_id(id, |ui| {
        ui.menu_button("\u{25BE}", |ui| {
            for n in names {
                let label = if n.is_empty() { "(material's own)" } else { n };
                if ui.button(label).clicked() {
                    *target = (*n).to_string();
                    ui.close_menu();
                }
            }
        });
    });
}

/// The preview colour of a layer: by role, with a few material keywords.
fn layer_color(l: &AssemblyLayer) -> Color32 {
    let n = l.material.to_ascii_lowercase();
    if n.contains("tile") || n.contains("ceramic") {
        return Color32::from_rgb(0xB4, 0xC4, 0xCC);
    }
    if n.contains("backer") || n.contains("concrete") || n.contains("mortar") {
        return Color32::from_rgb(0xA8, 0xA6, 0xA0);
    }
    if n.contains("insul") {
        return Color32::from_rgb(0xF0, 0xD8, 0xE0);
    }
    match l.role {
        LayerRole::Framing => Color32::from_rgb(0xD9, 0xB6, 0x7F),
        LayerRole::Sheathing => Color32::from_rgb(0xE8, 0xD2, 0xA0),
        LayerRole::Finish => Color32::from_rgb(0xF4, 0xF1, 0xEA),
        LayerRole::Cladding => Color32::from_rgb(0xC9, 0xB8, 0xA0),
        LayerRole::Standard => Color32::from_rgb(0xD6, 0xD3, 0xCB),
        LayerRole::AirGap => Color32::TRANSPARENT,
    }
}

// ------------------------------------------------------------------ rows --

/// What the four platform rows need to know besides the slots they edit.
pub struct RowContext<'a> {
    /// Offer the Use Default box (the plan-wide level has nothing above it).
    pub allow_default: bool,
    /// What Use Default gives, by position in [`AssemblyKind::PLATFORM`].
    pub inherited: [Assembly; 4],
    /// What a slot that is still Legacy means, same order.
    pub legacy: [Assembly; 4],
    pub library: &'a AssemblyLibrary,
}

impl RowContext<'_> {
    fn index(kind: AssemblyKind) -> usize {
        AssemblyKind::PLATFORM
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(0)
    }

    /// The definition a slot stands for.
    pub fn effective(&self, kind: AssemblyKind, slot: &AssemblySlot) -> Assembly {
        let i = Self::index(kind);
        match slot {
            AssemblySlot::Own(a) => a.clone(),
            AssemblySlot::Default if self.allow_default => self.inherited[i].clone(),
            _ => self.legacy[i].clone(),
        }
    }
}

/// The four platform rows and the window they open.
#[derive(Default)]
pub struct PlatformEditor {
    child: Option<AssemblyDefDialog>,
    /// Library saves made in definition windows that were accepted.
    saved: Vec<NamedAssembly>,
}

impl PlatformEditor {
    /// The definitions saved from the windows that were accepted, for the
    /// owner to put in the plan's library.
    pub fn take_saved(&mut self) -> Vec<NamedAssembly> {
        std::mem::take(&mut self.saved)
    }

    pub fn is_editing(&self) -> bool {
        self.child.is_some()
    }

    /// Opens the definition window for `kind` on the slot's definition.
    pub fn open(&mut self, kind: AssemblyKind, slots: &PlatformAssemblies, cx: &RowContext<'_>) {
        let a = cx.effective(kind, slots.slot(kind));
        self.child = Some(AssemblyDefDialog::new(kind, a, cx.library));
    }

    /// Draws one row per platform definition. Edit opens the window, the Use
    /// Default box hands the slot to the level above (or takes a copy of it
    /// back as the level's own).
    pub fn rows(&mut self, ui: &mut Ui, slots: &mut PlatformAssemblies, cx: &RowContext<'_>) {
        let mut open = None;
        for kind in AssemblyKind::PLATFORM {
            let slot = slots.slot(kind).clone();
            let a = cx.effective(kind, &slot);
            row(ui, kind.label(), |ui| {
                let shown = if a.is_empty() {
                    "none".to_string()
                } else {
                    fmt_short(a.total_thickness())
                };
                ui.label(format!(
                    "{} layer{}, {}",
                    a.layers.len(),
                    if a.layers.len() == 1 { "" } else { "s" },
                    shown
                ));
                if ui.button("Edit\u{2026}").clicked() {
                    open = Some(kind);
                }
                if cx.allow_default {
                    let mut follows = matches!(slot, AssemblySlot::Default);
                    if ui.checkbox(&mut follows, "Use Default").changed() {
                        slots.set(
                            kind,
                            if follows {
                                AssemblySlot::Default
                            } else {
                                AssemblySlot::Own(a.clone())
                            },
                        );
                    }
                } else if slot.own().is_some() && ui.button("Clear").clicked() {
                    slots.set(kind, AssemblySlot::Legacy);
                }
            });
        }
        if let Some(kind) = open {
            self.open(kind, slots, cx);
        }
    }

    /// Draws the open definition window; OK stores the result as the level's
    /// own definition. Returns whether a slot changed.
    pub fn child(&mut self, ctx: &egui::Context, slots: &mut PlatformAssemblies) -> bool {
        let Some(mut d) = self.child.take() else {
            return false;
        };
        match d.show(ctx) {
            Outcome::Open => {
                self.child = Some(d);
                false
            }
            Outcome::Cancel => false,
            Outcome::Ok => {
                self.accept(d, slots);
                true
            }
        }
    }

    /// Takes an accepted window's result (the tests call it directly).
    pub fn accept(&mut self, d: AssemblyDefDialog, slots: &mut PlatformAssemblies) {
        self.saved.extend(d.pending_saves.iter().cloned());
        slots.set(d.kind, AssemblySlot::Own(d.into_assembly()));
    }
}

/// What Use Default gives a floor level: the plan-wide definition when there
/// is one, else the floor's own single thickness read as a layer.
pub fn floor_inherited(lib: &AssemblyLibrary, floor: &FloorSettings) -> [Assembly; 4] {
    let mut as_default = floor.clone();
    as_default.platform = PlatformAssemblies::default();
    AssemblyKind::PLATFORM.map(|k| match lib.plan_wide.slot(k) {
        AssemblySlot::Own(a) => a.clone(),
        _ => resolve_floor(k, &as_default).assembly,
    })
}

/// What a floor level's slots mean when they are still Legacy.
pub fn floor_legacy(floor: &FloorSettings) -> [Assembly; 4] {
    let mut legacy = floor.clone();
    legacy.platform = PlatformAssemblies::default();
    AssemblyKind::PLATFORM.map(|k| resolve_floor(k, &legacy).assembly)
}

// ------------------------------------------------------------ the page --

/// Default Settings > Floors and Rooms > Floor/Ceiling Platform: the plan-wide
/// Floor Structure, Floor Finish, Ceiling Structure and Ceiling Finish. The
/// total depth of each is reported for reference; Edit opens the layers.
pub struct PlatformDefaultsPage {
    slots: PlatformAssemblies,
    original: PlatformAssemblies,
    library: AssemblyLibrary,
    legacy: [Assembly; 4],
    editor: PlatformEditor,
}

// The accessors are the page's model API (the tests drive it).
#[allow(dead_code)]
impl PlatformDefaultsPage {
    pub fn new(cx: &EditorContext) -> Self {
        let lib = cx.project.assemblies.clone();
        Self {
            slots: lib.plan_wide.clone(),
            original: lib.plan_wide.clone(),
            library: lib,
            // Not set: the definitions shown are the first floor's thicknesses.
            legacy: floor_legacy(&cx.project.floors[0].settings),
            editor: PlatformEditor::default(),
        }
    }

    pub fn slots(&self) -> &PlatformAssemblies {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut PlatformAssemblies {
        &mut self.slots
    }

    /// The total depth of each definition for reference, in order.
    pub fn depths(&self) -> [f64; 4] {
        let cx = self.row_context();
        AssemblyKind::PLATFORM.map(|k| cx.effective(k, self.slots.slot(k)).total_thickness())
    }

    fn row_context(&self) -> RowContext<'_> {
        RowContext {
            allow_default: false,
            inherited: self.legacy.clone(),
            legacy: self.legacy.clone(),
            library: &self.library,
        }
    }

    /// Stores the page in the plan; one undo step. Returns whether the plan
    /// changed.
    pub fn apply(&mut self, cx: &mut EditorContext) -> bool {
        let saved = self.editor.take_saved();
        let changed = AssemblyKind::PLATFORM
            .iter()
            .any(|k| self.slots.slot(*k) != self.original.slot(*k));
        if !changed && saved.is_empty() {
            return false;
        }
        cx.begin_change("Floor/Ceiling Platform Defaults");
        for kind in AssemblyKind::PLATFORM {
            if self.slots.slot(kind) != self.original.slot(kind) {
                let a = self.slots.slot(kind).own().cloned();
                cx.project.set_plan_wide_assembly(kind, a);
            }
        }
        for n in saved {
            cx.project
                .assemblies
                .save_named(n.kind, &n.name, n.assembly);
        }
        cx.project.sync_platform_mirrors();
        cx.mark_dirty();
        cx.refresh();
        true
    }

    fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let editing = self.editor.is_editing();
        let ctx_rows = RowContext {
            allow_default: false,
            inherited: self.legacy.clone(),
            legacy: self.legacy.clone(),
            library: &self.library,
        };
        let mut slots = self.slots.clone();
        let mut editor = std::mem::take(&mut self.editor);
        egui::Window::new("Default Settings: Floor/Ceiling Platform")
            .id(egui::Id::new("default_page_platforms"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.add_enabled_ui(!editing, |ui| {
                    ui.weak("The plan-wide definitions of the floor and ceiling platforms. A floor or room set to Use Default follows them; the total depth of each is shown.");
                    section(ui, "Platforms");
                    editor.rows(ui, &mut slots, &ctx_rows);
                    ui.add_space(4.0);
                    let floor = fmt_short(
                        ctx_rows
                            .effective(AssemblyKind::FloorStructure, slots.slot(AssemblyKind::FloorStructure))
                            .total_thickness()
                            + ctx_rows
                                .effective(
                                    AssemblyKind::CeilingStructure,
                                    slots.slot(AssemblyKind::CeilingStructure),
                                )
                                .total_thickness(),
                    );
                    ui.weak(format!("Floor and ceiling structure together add {floor} to the floor-to-floor height."));
                    ui.add_space(6.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("   OK   ").clicked() {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                        if ui.button("Reset Page").clicked() {
                            slots = PlatformAssemblies::default();
                        }
                    });
                });
            });
        editor.child(ctx, &mut slots);
        self.editor = editor;
        self.slots = slots;
        if !open {
            outcome = Outcome::Cancel;
        }
        if !self.editor.is_editing() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                outcome = Outcome::Cancel;
            } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                outcome = Outcome::Ok;
            }
        }
        outcome
    }
}

thread_local! {
    static WANTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PAGE: RefCell<Option<PlatformDefaultsPage>> = const { RefCell::new(None) };
}

/// Asks for the page; [`show_page`] opens it on its next frame.
pub fn request_page() {
    WANTED.with(|c| c.set(true));
}

/// Is the page asked for or showing?
pub fn page_is_open() -> bool {
    WANTED.with(std::cell::Cell::get) || PAGE.with(|p| p.borrow().is_some())
}

/// Runs `f` on the open page (for the tests).
#[cfg(test)]
pub fn with_page<R>(f: impl FnOnce(&mut PlatformDefaultsPage) -> R) -> Option<R> {
    PAGE.with(|p| p.borrow_mut().as_mut().map(f))
}

/// Draws the page (when open) and applies OK.
pub fn show_page(ctx: &egui::Context, cx: &mut EditorContext) {
    if WANTED.with(|c| c.replace(false)) {
        PAGE.with(|p| *p.borrow_mut() = Some(PlatformDefaultsPage::new(cx)));
    }
    let Some(mut page) = PAGE.with(|p| p.borrow_mut().take()) else {
        return;
    };
    match page.show(ctx) {
        Outcome::Open => PAGE.with(|p| *p.borrow_mut() = Some(page)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            cx.status = if page.apply(cx) {
                "Saved the Floor/Ceiling Platform defaults".into()
            } else {
                "The Floor/Ceiling Platform defaults did not change".into()
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::assemblies::AssemblyKind as K;

    fn dialog(kind: K, a: Assembly) -> AssemblyDefDialog {
        AssemblyDefDialog::new(kind, a, &AssemblyLibrary::default())
    }

    #[test]
    fn insert_delete_and_move_edit_the_table() {
        let mut d = dialog(K::FloorStructure, K::FloorStructure.builtin());
        assert_eq!(d.assembly().layers.len(), 2);
        assert_eq!(d.insert_layer(), 0, "above the selected layer");
        assert_eq!(d.assembly().layers.len(), 3);
        assert_eq!(d.assembly().layers[0].material, "New Layer");
        assert!(d.move_down());
        assert_eq!(d.selected(), 1);
        assert!(d.delete_layer());
        assert_eq!(d.assembly().layers.len(), 2);
        assert!(!d.move_up() || d.selected() == 0);
        d.select(1);
        assert!(!d.move_down(), "already last");
        d.reset();
        assert_eq!(d.total_thickness(), 10.25);
    }

    #[test]
    fn an_empty_table_takes_an_inserted_layer() {
        let mut d = dialog(K::CeilingFinish, Assembly::default());
        assert!(!d.delete_layer());
        assert_eq!(d.insert_layer(), 0);
        assert_eq!(d.assembly().layers.len(), 1);
        assert!(d.preview_spans().len() == 1);
    }

    #[test]
    fn the_role_decides_the_framing_options() {
        let mut d = dialog(K::FloorStructure, K::FloorStructure.builtin());
        let framing = d.assembly().layers[1].framing.unwrap();
        assert_eq!(framing.construction, FramingConstruction::Lumber);
        d.set_role(1, LayerRole::Standard);
        assert!(d.assembly().layers[1].framing.is_none());
        d.set_role(1, LayerRole::Framing);
        d.layer_mut(1).unwrap().framing = Some(FramingSpec {
            method: FramingMethod::Trusses,
            construction: FramingConstruction::IJoist,
            width: 2.5,
            spacing: 24.0,
        });
        let (spec, depth) = d.assembly().framing().unwrap();
        assert_eq!(
            (spec.method, spec.spacing, depth),
            (FramingMethod::Trusses, 24.0, 9.25)
        );
    }

    #[test]
    fn the_preview_stacks_the_layers_by_thickness() {
        let d = dialog(
            K::FloorStructure,
            Assembly::new(vec![
                AssemblyLayer::new("OSB", LayerRole::Sheathing, 1.0),
                AssemblyLayer::new("Joist", LayerRole::Framing, 3.0),
            ]),
        );
        let s = d.preview_spans();
        assert_eq!(s[0], (0.0, 0.25));
        assert_eq!(s[1], (0.25, 1.0));
    }

    #[test]
    fn a_negative_thickness_blocks_ok() {
        let mut d = dialog(K::FloorFinish, K::FloorFinish.builtin());
        assert!(d.error().is_none());
        d.layer_mut(0).unwrap().thickness = -1.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn the_library_keeps_saved_definitions() {
        let mut lib = AssemblyLibrary::default();
        lib.save_named(K::CeilingFinish, "Hat channel", K::CeilingFinish.builtin());
        lib.save_named(K::FloorFinish, "Tile", K::FloorFinish.builtin());
        let mut d = AssemblyDefDialog::new(K::CeilingFinish, Assembly::default(), &lib);
        assert!(
            !d.load_saved("Tile"),
            "another kind's definition is not offered"
        );
        assert!(d.load_saved("Hat channel"));
        assert_eq!(d.total_thickness(), 0.625);
        assert!(d.save_as("Mine"));
        assert!(!d.save_as("  "));
        assert!(d.load_saved("Mine"), "pending saves are offered too");
        assert_eq!(d.pending_saves().len(), 1);
    }

    #[test]
    fn the_window_draws_with_every_role_in_it() {
        let mut layers = Vec::new();
        for (i, r) in LayerRole::ALL.iter().enumerate() {
            layers.push(AssemblyLayer::new(format!("Layer {i}"), *r, 0.5 + i as f64));
        }
        let mut d = dialog(K::CeilingFinish, Assembly::new(layers));
        let ctx = egui::Context::default();
        for sel in 0..6 {
            d.select(sel);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
    }

    #[test]
    fn rows_use_the_level_above_when_a_slot_follows_it() {
        let lib = AssemblyLibrary::default();
        let floor = FloorSettings::default();
        let cx = RowContext {
            allow_default: true,
            inherited: floor_inherited(&lib, &floor),
            legacy: floor_legacy(&floor),
            library: &lib,
        };
        assert_eq!(
            cx.effective(K::FloorStructure, &AssemblySlot::Default)
                .total_thickness(),
            10.25
        );
        let own = Assembly::from_thickness("Slab", LayerRole::Standard, 4.0);
        assert_eq!(
            cx.effective(K::FloorStructure, &AssemblySlot::Own(own))
                .total_thickness(),
            4.0
        );
        let mut ed = PlatformEditor::default();
        let mut slots = PlatformAssemblies::default();
        ed.open(K::FloorFinish, &slots, &cx);
        assert!(ed.is_editing());
        let mut d = AssemblyDefDialog::new(K::FloorFinish, Assembly::default(), &lib);
        d.insert_layer();
        ed.accept(d, &mut slots);
        assert!(slots.slot(K::FloorFinish).own().is_some());
    }
}
