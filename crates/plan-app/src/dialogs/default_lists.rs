//! The list dialogs behind Edit > Default Settings:
//!
//! * **Saved Dimension Defaults**: the plan defaults' `dimension_sets` with
//!   Edit / Copy / Rename / Delete and a "Currently Active" combo; Edit opens
//!   the Dimension Defaults dialog (Primary Format, Setup Automatic,
//!   Extensions, Arrow, Text Style).
//! * **Room Types**: name, function, living area, conditioned, with
//!   Add / Edit / Rename / Delete.
//! * **Text Styles**: name, font, height, bold / italic / underline and color,
//!   for the plan and for the defaults new plans start from.
//!
//! Each dialog works on a draft; OK applies it to the context (the plan's
//! part as one undo step), Cancel or Escape drops it. The draft types carry
//! the editing rules (unique names, protected entries) so they are tested
//! without a window.

use super::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED, PV_INK};
use crate::editor::EditorContext;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::defaults::{DimensionDefaultSet, RoomTypeDef};
use plan_core::text_styles::{TextStyles, DEFAULT_TEXT_STYLE_NAME};
use plan_core::units::{LengthFormat, LengthUnit};
use plan_core::{AutoString, ObjectLocate, OpeningLocate, PlanDefaults, WallLocate};

/// The Default Settings list dialog that is open.
pub enum DefaultsList {
    Dimensions(Box<DimensionSetsDialog>),
    RoomTypes(Box<RoomTypesDialog>),
    TextStyles(Box<TextStylesDialog>),
}

impl DefaultsList {
    pub fn dimensions(cx: &EditorContext) -> Self {
        DefaultsList::Dimensions(Box::new(DimensionSetsDialog::new(&cx.defaults)))
    }

    pub fn room_types(cx: &EditorContext) -> Self {
        DefaultsList::RoomTypes(Box::new(RoomTypesDialog::new(&cx.defaults)))
    }

    pub fn text_styles(cx: &EditorContext) -> Self {
        DefaultsList::TextStyles(Box::new(TextStylesDialog::new(cx)))
    }

    /// Draws the dialog; false once it is closed (OK applied or cancelled).
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let outcome = match self {
            DefaultsList::Dimensions(d) => d.show(ctx),
            DefaultsList::RoomTypes(d) => d.show(ctx),
            DefaultsList::TextStyles(d) => d.show(ctx),
        };
        match outcome {
            Outcome::Open => true,
            Outcome::Cancel => false,
            Outcome::Ok => {
                match self {
                    DefaultsList::Dimensions(d) => {
                        d.sets.apply(&mut cx.defaults);
                        cx.mark_dirty();
                        cx.status = "Saved the dimension defaults".into();
                    }
                    DefaultsList::RoomTypes(d) => {
                        d.types.apply(cx);
                        cx.status = "Saved the room types".into();
                    }
                    DefaultsList::TextStyles(d) => {
                        d.apply(cx);
                        cx.status = "Saved the text styles".into();
                    }
                }
                false
            }
        }
    }
}

/// A unique name based on `base` among `taken` (`New`, `New 2`, ...).
fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

/// Window chrome shared by the three lists: Enter/Esc handling and the
/// OK / Cancel row. `body` draws the content.
fn list_window(
    ctx: &egui::Context,
    title: &str,
    key: &str,
    enabled: bool,
    error: Option<&str>,
    size: [f32; 2],
    body: impl FnOnce(&mut Ui),
) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("default_list", key)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(size)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                let h = (ui.available_height() - 44.0).max(60.0);
                ui.allocate_ui(egui::vec2(ui.available_width(), h), |ui| body(ui));
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(egui::RichText::new("   OK   ").strong());
                    if ui.add_enabled(error.is_none(), ok).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        });
    if !open {
        outcome = Outcome::Cancel;
    }
    if enabled && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    outcome
}

/// A one-line "name" prompt used for Rename.
fn name_prompt(
    ctx: &egui::Context,
    title: &str,
    text: &mut String,
    error: Option<&str>,
) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(title)
        .id(egui::Id::new(("name_prompt", title)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(text).desired_width(260.0));
            r.request_focus();
            ui.add_space(6.0);
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
                if let Some(e) = error {
                    ui.colored_label(ERROR_RED, e);
                }
            });
        });
    if !open {
        outcome = Outcome::Cancel;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        outcome = Outcome::Cancel;
    } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        outcome = Outcome::Ok;
    }
    outcome
}

// ----- Saved Dimension Defaults -----

/// The draft of the dimension default sets and which one is active.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionSets {
    pub sets: Vec<DimensionDefaultSet>,
    pub active: String,
}

impl DimensionSets {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            sets: d.dimension_sets.clone(),
            active: d.active_dimension_set.clone(),
        }
    }

    fn taken(&self, name: &str) -> bool {
        self.sets.iter().any(|s| s.name == name)
    }

    /// Copy: a new set after `idx` named `<name> Copy`; returns its index.
    pub fn copy(&mut self, idx: usize) -> Option<usize> {
        let src = self.sets.get(idx)?;
        let name = unique_name(&format!("{} Copy", src.name), |n| self.taken(n));
        let mut set = src.clone();
        set.name = name.clone();
        set.auto.set_name = format!("{name} Dimension Defaults");
        self.sets.insert(idx + 1, set);
        Some(idx + 1)
    }

    /// Why `new` cannot name set `idx`, if it cannot.
    pub fn name_error(&self, idx: usize, new: &str) -> Option<&'static str> {
        let new = new.trim();
        if new.is_empty() {
            Some("A name is required")
        } else if self
            .sets
            .iter()
            .enumerate()
            .any(|(i, s)| i != idx && s.name == new)
        {
            Some("That name is already used")
        } else {
            None
        }
    }

    pub fn rename(&mut self, idx: usize, new: &str) -> Result<(), String> {
        if let Some(e) = self.name_error(idx, new) {
            return Err(e.into());
        }
        let new = new.trim().to_string();
        let set = self.sets.get_mut(idx).ok_or("No such set")?;
        if self.active == set.name {
            self.active = new.clone();
        }
        set.auto.set_name = format!("{new} Dimension Defaults");
        set.name = new;
        Ok(())
    }

    /// The active set cannot be deleted, and the last set stays.
    pub fn delete(&mut self, idx: usize) -> Result<(), String> {
        let name = self.sets.get(idx).ok_or("No such set")?.name.clone();
        if name == self.active {
            return Err("Make another set active before deleting this one".into());
        }
        if self.sets.len() <= 1 {
            return Err("The last set cannot be deleted".into());
        }
        self.sets.remove(idx);
        Ok(())
    }

    /// Writes the sets to the defaults and loads the active one into
    /// `defaults.dimensions` (the first set when the active one is gone).
    pub fn apply(&self, d: &mut PlanDefaults) {
        d.dimension_sets = self.sets.clone();
        let active = if self.sets.iter().any(|s| s.name == self.active) {
            self.active.clone()
        } else {
            self.sets
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_default()
        };
        if !d.set_active_dimension_set(&active) {
            d.active_dimension_set = active;
        }
    }
}

enum Prompt {
    Rename { idx: usize, text: String },
}

pub struct DimensionSetsDialog {
    pub sets: DimensionSets,
    selected: usize,
    edit: Option<(usize, SpecDialog, DimensionSetForm)>,
    prompt: Option<Prompt>,
    message: Option<String>,
}

impl DimensionSetsDialog {
    pub fn new(d: &PlanDefaults) -> Self {
        let sets = DimensionSets::from_defaults(d);
        let selected = sets
            .sets
            .iter()
            .position(|s| s.name == sets.active)
            .unwrap_or(0);
        Self {
            sets,
            selected,
            edit: None,
            prompt: None,
            message: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // Sub-dialogs first: they take Esc and Enter.
        if let Some((idx, mut frame, mut form)) = self.edit.take() {
            match frame.show(ctx, &mut form) {
                Outcome::Open => self.edit = Some((idx, frame, form)),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if let Some(slot) = self.sets.sets.get_mut(idx) {
                        *slot = form.finished();
                    }
                }
            }
        }
        if let Some(Prompt::Rename { idx, mut text }) = self.prompt.take() {
            let err = self.sets.name_error(idx, &text);
            match name_prompt(ctx, "Rename Dimension Defaults", &mut text, err) {
                Outcome::Open => self.prompt = Some(Prompt::Rename { idx, text }),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if let Err(e) = self.sets.rename(idx, &text) {
                        self.message = Some(e);
                    }
                }
            }
        }
        let enabled = self.edit.is_none() && self.prompt.is_none();
        let (sets, selected, message) = (&mut self.sets, &mut self.selected, &mut self.message);
        let mut want_edit = None;
        let mut want_rename = None;
        let outcome = list_window(
            ctx,
            "Saved Dimension Defaults",
            "dimension_sets",
            enabled,
            None,
            [380.0, 460.0],
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Currently Active");
                    egui::ComboBox::from_id_salt("dim_sets_active")
                        .selected_text(sets.active.clone())
                        .show_ui(ui, |ui| {
                            let names: Vec<String> =
                                sets.sets.iter().map(|s| s.name.clone()).collect();
                            for n in names {
                                ui.selectable_value(&mut sets.active, n.clone(), n);
                            }
                        });
                });
                ui.separator();
                let list_h = (ui.available_height() - 36.0).max(60.0);
                egui::ScrollArea::vertical()
                    .id_salt("dim_sets_list")
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, s) in sets.sets.iter().enumerate() {
                            let label = if s.name == sets.active {
                                format!("{}  (active)", s.name)
                            } else {
                                s.name.clone()
                            };
                            let r = ui.selectable_label(*selected == i, label);
                            if r.clicked() {
                                *selected = i;
                            }
                            if r.double_clicked() {
                                want_edit = Some(i);
                            }
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let has = *selected < sets.sets.len();
                    if ui
                        .add_enabled(has, egui::Button::new("Edit\u{2026}"))
                        .clicked()
                    {
                        want_edit = Some(*selected);
                    }
                    if ui.add_enabled(has, egui::Button::new("Copy")).clicked() {
                        if let Some(n) = sets.copy(*selected) {
                            *selected = n;
                            *message = None;
                        }
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Rename\u{2026}"))
                        .clicked()
                    {
                        want_rename = Some(*selected);
                    }
                    if ui.add_enabled(has, egui::Button::new("Delete")).clicked() {
                        match sets.delete(*selected) {
                            Ok(()) => {
                                *selected = (*selected).min(sets.sets.len().saturating_sub(1));
                                *message = None;
                            }
                            Err(e) => *message = Some(e),
                        }
                    }
                });
                if let Some(m) = message.as_deref() {
                    ui.colored_label(ERROR_RED, m);
                }
            },
        );
        if let Some(i) = want_edit {
            if let Some(set) = self.sets.sets.get(i) {
                self.edit = Some((
                    i,
                    SpecDialog::new(
                        format!("Dimension Defaults \u{2013} {}", set.name),
                        "dimension_defaults",
                    ),
                    DimensionSetForm::new(set.clone()),
                ));
            }
        }
        if let Some(i) = want_rename {
            if let Some(set) = self.sets.sets.get(i) {
                self.prompt = Some(Prompt::Rename {
                    idx: i,
                    text: set.name.clone(),
                });
            }
        }
        outcome
    }
}

// ----- the Dimension Defaults editor -----

const DIM_TABS: &[Tab] = &[
    on("Primary Format"),
    on("Setup Automatic"),
    on("Extensions"),
    on("Arrow"),
    on("Text Style"),
    on("Locate Objects"),
];

const UNITS: [(LengthUnit, &str); 6] = [
    (LengthUnit::FeetInches, "Feet and Inches"),
    (LengthUnit::Inches, "Inches"),
    (LengthUnit::DecimalFeet, "Decimal Feet"),
    (LengthUnit::Millimeters, "Millimeters"),
    (LengthUnit::Centimeters, "Centimeters"),
    (LengthUnit::Meters, "Meters"),
];

const FRACTIONS: [u32; 6] = [2, 4, 8, 16, 32, 64];
const FRACTION_STYLES: [&str; 3] = ["Diagonal", "Horizontal", "Stacked"];
const LEADER_STYLES: [&str; 3] = ["Square Corner", "Round Corner", "Diagonal"];

fn unit_label(u: LengthUnit) -> &'static str {
    UNITS.iter().find(|(x, _)| *x == u).map_or("", |(_, l)| l)
}

/// The Dimension Defaults dialog's draft.
pub struct DimensionSetForm {
    draft: DimensionDefaultSet,
    fields: Fields,
    /// Locate Objects group shown: 0 manual and automatic, 1 temporary,
    /// 2 elevation.
    locate_group: u8,
}

impl DimensionSetForm {
    pub fn new(mut set: DimensionDefaultSet) -> Self {
        set.format.length.get_or_insert_with(LengthFormat::default);
        Self {
            draft: set,
            fields: Fields::default(),
            locate_group: 0,
        }
    }

    /// The edited set with its text format kept in step with the automatic
    /// settings (fraction, unit indicators, trailing zeroes).
    pub fn finished(&self) -> DimensionDefaultSet {
        let mut set = self.draft.clone();
        sync_format(&mut set);
        set
    }
}

/// Copies the shared fields from the automatic settings into the format.
pub fn sync_format(set: &mut DimensionDefaultSet) {
    let a = &set.auto;
    set.format.smallest_fraction = a.smallest_fraction.max(1);
    set.format.unit_indicators = a.unit_indicators;
    let l = set.format.length.get_or_insert_with(LengthFormat::default);
    l.fraction_denominator = a.smallest_fraction.max(1);
    l.unit_indicators = a.unit_indicators;
    l.trailing_zeroes = a.trailing_zeroes;
}

fn fraction_label(n: u32) -> String {
    format!("1/{n}\"")
}

impl SpecPages for DimensionSetForm {
    fn tabs(&self) -> &'static [Tab] {
        DIM_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Fix the highlighted field".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let set = &mut self.draft;
        match tab {
            0 => {
                section(ui, "Primary Format");
                let l = set.format.length.get_or_insert_with(LengthFormat::default);
                row(ui, "Units", |ui| {
                    egui::ComboBox::from_id_salt("dim_units")
                        .selected_text(unit_label(l.unit))
                        .show_ui(ui, |ui| {
                            for (u, name) in UNITS {
                                ui.selectable_value(&mut l.unit, u, name);
                            }
                        });
                });
                row(ui, "Smallest Fraction", |ui| {
                    egui::ComboBox::from_id_salt("dim_fraction")
                        .selected_text(fraction_label(set.auto.smallest_fraction))
                        .show_ui(ui, |ui| {
                            for n in FRACTIONS {
                                ui.selectable_value(
                                    &mut set.auto.smallest_fraction,
                                    n,
                                    fraction_label(n),
                                );
                            }
                        });
                });
                row(ui, "Fraction Style", |ui| {
                    string_combo(
                        ui,
                        "dim_fraction_style",
                        &mut set.auto.fraction_style,
                        &FRACTION_STYLES,
                    );
                });
                row(ui, "Decimal Places", |ui| {
                    ui.add(egui::DragValue::new(&mut l.decimals).range(0..=6));
                });
                ui.checkbox(&mut set.auto.unit_indicators, "Unit Indicators");
                ui.checkbox(&mut set.auto.trailing_zeroes, "Trailing Zeroes");
            }
            1 => {
                section(ui, "Setup Automatic");
                self.fields.length_row(
                    ui,
                    "Exterior Offset",
                    "auto_offset",
                    &mut set.auto.auto_exterior_offset,
                );
                self.fields.length_row(
                    ui,
                    "Line Separation",
                    "auto_separation",
                    &mut set.auto.auto_line_separation,
                );
                // The strings of Auto Exterior Dimensions, nearest the wall first.
                section(ui, "Exterior Strings (nearest the wall first)");
                let current = set.auto.exterior_strings();
                let mut slots: [Option<AutoString>; 3] = [None; 3];
                for (i, st) in current.iter().take(3).enumerate() {
                    slots[i] = Some(*st);
                }
                for (i, slot) in slots.iter_mut().enumerate() {
                    row(ui, &format!("String {}", i + 1), |ui| {
                        egui::ComboBox::from_id_salt(format!("auto_string_{i}"))
                            .selected_text(slot.map_or("None", AutoString::label))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(slot, None, "None");
                                for st in [
                                    AutoString::Openings,
                                    AutoString::WallToWall,
                                    AutoString::Overall,
                                ] {
                                    ui.selectable_value(slot, Some(st), st.label());
                                }
                            });
                    });
                }
                let picked: Vec<AutoString> = slots.iter().flatten().copied().collect();
                set.auto.auto_strings = if picked.is_empty() {
                    vec![AutoString::Overall]
                } else {
                    picked
                };
            }
            2 => {
                section(ui, "Extension Lines");
                self.fields.length_row(
                    ui,
                    "Gap from Object",
                    "ext_gap",
                    &mut set.auto.extension_gap,
                );
                self.fields.length_row(
                    ui,
                    "Extend Past Dimension Line",
                    "ext_past",
                    &mut set.auto.extension_past,
                );
            }
            3 => {
                section(ui, "Arrow");
                self.fields
                    .length_row(ui, "Arrow Size", "arrow_size", &mut set.auto.arrow_size);
                row(ui, "Leader Style", |ui| {
                    string_combo(ui, "dim_leader", &mut set.auto.leader_style, &LEADER_STYLES);
                });
            }
            5 => {
                // Three groups: the manual and automatic dimensions' (the
                // set's own), the temporary dimensions' and the elevation
                // dimensions' (DIM-40).
                ui.horizontal(|ui| {
                    for (i, name) in ["Manual and Automatic", "Temporary", "Elevation"]
                        .into_iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.locate_group, i as u8, name);
                    }
                });
                if self.locate_group > 0 {
                    let mut g = if self.locate_group == 1 {
                        set.auto.temp_group()
                    } else {
                        set.auto.elevation_group()
                    };
                    section(ui, "Walls");
                    for m in WallLocate::ALL {
                        ui.radio_value(&mut g.walls, m, m.label());
                    }
                    section(ui, "Openings");
                    for m in OpeningLocate::ALL {
                        ui.radio_value(&mut g.openings, m, m.label());
                    }
                    section(ui, "Cabinets");
                    for m in ObjectLocate::ALL {
                        ui.radio_value(&mut g.cabinets, m, m.label());
                    }
                    section(ui, "Fixtures");
                    for m in ObjectLocate::ALL {
                        ui.radio_value(&mut g.fixtures, m, m.label());
                    }
                    if self.locate_group == 1 {
                        if g != set.auto.temp_group() {
                            set.auto.temp_locate = Some(g);
                        }
                    } else if g != set.auto.elevation_group() {
                        set.auto.elevation_locate = Some(g);
                    }
                    return;
                }
                section(ui, "Walls");
                for m in WallLocate::ALL {
                    ui.radio_value(&mut set.auto.locate_walls, m, m.label());
                }
                ui.checkbox(
                    &mut set.auto.interior_locates_interior_surfaces,
                    "Interior dimensions locate interior surfaces",
                );
                section(ui, "Openings");
                let mut mode = set.auto.opening_locate();
                for m in OpeningLocate::ALL {
                    ui.radio_value(&mut mode, m, m.label());
                }
                if Some(mode) != set.auto.locate_openings {
                    set.auto.set_opening_locate(mode);
                }
                section(ui, "Cabinets");
                for m in ObjectLocate::ALL {
                    ui.radio_value(&mut set.auto.locate_cabinets, m, m.label());
                }
                section(ui, "Fixtures");
                for m in ObjectLocate::ALL {
                    ui.radio_value(&mut set.auto.locate_fixtures, m, m.label());
                }
            }
            _ => {
                section(ui, "Text Style");
                row(ui, "Text Style", |ui| {
                    let mut name = set.auto.text_style.clone();
                    let hint = "Dimension Text Style";
                    ui.add(
                        egui::TextEdit::singleline(&mut name)
                            .hint_text(hint)
                            .desired_width(180.0),
                    );
                    set.auto.text_style = name;
                });
                ui.checkbox(
                    &mut set.auto.printed_size,
                    "Printed Size (text and arrows keep their size on paper at any scale)",
                );
                ui.checkbox(&mut set.auto.text_above_line, "Text Above Dimension Line");
                row(ui, "Fraction Text Size", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut set.auto.fraction_text_size_pct)
                            .range(25..=100)
                            .suffix(" %"),
                    );
                });
            }
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let mut set = self.draft.clone();
        sync_format(&mut set);
        let y = rect.center().y;
        let (a, b) = (rect.left() + 10.0, rect.right() - 10.0);
        let ink = Stroke::new(1.2_f32, PV_INK);
        painter.line_segment([Pos2::new(a, y), Pos2::new(b, y)], ink);
        for x in [a, b] {
            painter.line_segment([Pos2::new(x, y - 8.0), Pos2::new(x, y + 8.0)], ink);
        }
        let text_y = if set.auto.text_above_line {
            y - 12.0
        } else {
            y + 12.0
        };
        painter.text(
            Pos2::new(rect.center().x, text_y),
            Align2::CENTER_CENTER,
            set.format.fmt_len(150.5),
            egui::FontId::proportional(13.0),
            PV_INK,
        );
        painter.text(
            Pos2::new(rect.center().x, rect.top() + 12.0),
            Align2::CENTER_CENTER,
            &set.name,
            egui::FontId::proportional(11.0),
            PV_INK,
        );
    }
}

/// A combo over `options` that also lists the current value when it is not
/// one of them (a name read from a Chief template).
fn string_combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[&str]) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            if !options.contains(&value.as_str()) && !value.is_empty() {
                let cur = value.clone();
                ui.selectable_value(value, cur.clone(), cur);
            }
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
}

// ----- Room Types -----

/// Chief's room functions.
pub const FUNCTIONS: [&str; 7] = [
    "Standard",
    "Living",
    "Utility",
    "Deck",
    "Garage",
    "Porch",
    "Open Below",
];

/// The draft of the room types: each entry remembers the name it had when
/// the dialog opened, so renames can follow into the plan's rooms.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomTypeList {
    pub types: Vec<(Option<String>, RoomTypeDef)>,
}

impl RoomTypeList {
    pub fn from_defaults(d: &PlanDefaults) -> Self {
        Self {
            types: d
                .rooms
                .room_types
                .iter()
                .map(|t| (Some(t.name.clone()), t.clone()))
                .collect(),
        }
    }

    pub fn name_error(&self, idx: usize, new: &str) -> Option<&'static str> {
        let new = new.trim();
        if new.is_empty() {
            Some("A name is required")
        } else if self
            .types
            .iter()
            .enumerate()
            .any(|(i, (_, t))| i != idx && t.name == new)
        {
            Some("That name is already used")
        } else {
            None
        }
    }

    /// Adds a Standard room type with a fresh name; returns its index.
    pub fn add(&mut self) -> usize {
        let name = unique_name("New Room Type", |n| {
            self.types.iter().any(|(_, t)| t.name == n)
        });
        self.types.push((
            None,
            RoomTypeDef {
                name,
                function: "Standard".into(),
                include_in_living_area: true,
                conditioned: true,
                default_floor_finish: String::new(),
            },
        ));
        self.types.len() - 1
    }

    pub fn rename(&mut self, idx: usize, new: &str) -> Result<(), String> {
        if let Some(e) = self.name_error(idx, new) {
            return Err(e.into());
        }
        let t = self.types.get_mut(idx).ok_or("No such room type")?;
        t.1.name = new.trim().to_string();
        Ok(())
    }

    /// "Unspecified" is what rooms without a type use and stays.
    pub fn delete(&mut self, idx: usize) -> Result<(), String> {
        let name = self
            .types
            .get(idx)
            .ok_or("No such room type")?
            .1
            .name
            .clone();
        if name == "Unspecified" {
            return Err("\"Unspecified\" cannot be deleted".into());
        }
        self.types.remove(idx);
        Ok(())
    }

    /// `(old, new)` for every type renamed since the dialog opened.
    pub fn renames(&self) -> Vec<(String, String)> {
        self.types
            .iter()
            .filter_map(|(orig, t)| match orig {
                Some(o) if *o != t.name => Some((o.clone(), t.name.clone())),
                _ => None,
            })
            .collect()
    }

    /// Writes the types to the defaults, and the renames into the plan's
    /// named rooms as one undo step.
    pub fn apply(&self, cx: &mut EditorContext) {
        let renames = self.renames();
        if !renames.is_empty() {
            let uses = |p: &plan_core::Project| {
                p.floors
                    .iter()
                    .flat_map(|f| &f.room_names)
                    .any(|r| renames.iter().any(|(o, _)| *o == r.room_type))
            };
            if uses(&cx.project) {
                cx.begin_change("Rename Room Types");
                for f in &mut cx.project.floors {
                    for r in &mut f.room_names {
                        if let Some((_, new)) = renames.iter().find(|(o, _)| *o == r.room_type) {
                            r.room_type = new.clone();
                        }
                    }
                }
                cx.mark_dirty();
            }
        }
        cx.defaults.rooms.room_types = self.types.iter().map(|(_, t)| t.clone()).collect();
    }
}

pub struct RoomTypesDialog {
    pub types: RoomTypeList,
    selected: usize,
    /// The Edit form: a copy of the type being edited.
    edit: Option<(usize, RoomTypeDef)>,
    prompt: Option<(usize, String)>,
    message: Option<String>,
}

impl RoomTypesDialog {
    pub fn new(d: &PlanDefaults) -> Self {
        Self {
            types: RoomTypeList::from_defaults(d),
            selected: 0,
            edit: None,
            prompt: None,
            message: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if let Some((idx, mut t)) = self.edit.take() {
            match edit_room_type(ctx, &mut t) {
                Outcome::Open => self.edit = Some((idx, t)),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if let Some(slot) = self.types.types.get_mut(idx) {
                        slot.1.function = t.function;
                        slot.1.include_in_living_area = t.include_in_living_area;
                        slot.1.conditioned = t.conditioned;
                        slot.1.default_floor_finish = t.default_floor_finish;
                    }
                }
            }
        }
        if let Some((idx, mut text)) = self.prompt.take() {
            let err = self.types.name_error(idx, &text);
            match name_prompt(ctx, "Rename Room Type", &mut text, err) {
                Outcome::Open => self.prompt = Some((idx, text)),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if let Err(e) = self.types.rename(idx, &text) {
                        self.message = Some(e);
                    }
                }
            }
        }
        let enabled = self.edit.is_none() && self.prompt.is_none();
        let (types, selected, message) = (&mut self.types, &mut self.selected, &mut self.message);
        let mut want_edit = None;
        let mut want_rename = None;
        let outcome = list_window(
            ctx,
            "Room Types",
            "room_types",
            enabled,
            None,
            [520.0, 480.0],
            |ui| {
                let list_h = (ui.available_height() - 40.0).max(60.0);
                egui::ScrollArea::vertical()
                    .id_salt("room_types_list")
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("room_types_grid")
                            .num_columns(4)
                            .striped(true)
                            .spacing([14.0, 3.0])
                            .show(ui, |ui| {
                                for h in ["Name", "Function", "Living Area", "Conditioned"] {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for (i, (_, t)) in types.types.iter().enumerate() {
                                    let r = ui.selectable_label(*selected == i, &t.name);
                                    if r.clicked() {
                                        *selected = i;
                                    }
                                    if r.double_clicked() {
                                        want_edit = Some(i);
                                    }
                                    ui.label(&t.function);
                                    ui.label(yes_no(t.include_in_living_area));
                                    ui.label(yes_no(t.conditioned));
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    let has = *selected < types.types.len();
                    if ui.button("Add").clicked() {
                        *selected = types.add();
                        want_edit = Some(*selected);
                        *message = None;
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Edit\u{2026}"))
                        .clicked()
                    {
                        want_edit = Some(*selected);
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Rename\u{2026}"))
                        .clicked()
                    {
                        want_rename = Some(*selected);
                    }
                    if ui.add_enabled(has, egui::Button::new("Delete")).clicked() {
                        match types.delete(*selected) {
                            Ok(()) => {
                                *selected = (*selected).min(types.types.len().saturating_sub(1));
                                *message = None;
                            }
                            Err(e) => *message = Some(e),
                        }
                    }
                });
                if let Some(m) = message.as_deref() {
                    ui.colored_label(ERROR_RED, m);
                }
            },
        );
        if let Some(i) = want_edit {
            if let Some((_, t)) = self.types.types.get(i) {
                self.edit = Some((i, t.clone()));
            }
        }
        if let Some(i) = want_rename {
            if let Some((_, t)) = self.types.types.get(i) {
                self.prompt = Some((i, t.name.clone()));
            }
        }
        outcome
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "Yes"
    } else {
        "No"
    }
}

fn edit_room_type(ctx: &egui::Context, t: &mut RoomTypeDef) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    egui::Window::new(format!("Room Type \u{2013} {}", t.name))
        .id(egui::Id::new("room_type_edit"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            row(ui, "Function", |ui| {
                string_combo(ui, "room_type_function", &mut t.function, &FUNCTIONS);
            });
            ui.checkbox(&mut t.include_in_living_area, "Include in Living Area");
            ui.checkbox(&mut t.conditioned, "Conditioned");
            row(ui, "Default Floor Finish", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut t.default_floor_finish).desired_width(160.0),
                );
            });
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("   OK   ").clicked() {
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

// ----- Text Styles -----

/// Where a text style list lives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StyleScope {
    /// The open plan (`Project::text_styles`), one undo step.
    Plan,
    /// What new plans start from (`PlanDefaults::text_styles`).
    Defaults,
}

impl StyleScope {
    fn label(self) -> &'static str {
        match self {
            StyleScope::Plan => "This plan",
            StyleScope::Defaults => "New-plan defaults",
        }
    }
}

/// Editing rules for a list of text styles.
pub trait StyleListExt {
    fn name_error(&self, idx: usize, new: &str) -> Option<&'static str>;
    fn add_style(&mut self, from: Option<usize>) -> usize;
    fn delete_style(&mut self, idx: usize) -> Result<(), String>;
}

impl StyleListExt for TextStyles {
    fn name_error(&self, idx: usize, new: &str) -> Option<&'static str> {
        let new = new.trim();
        if new.is_empty() {
            Some("A name is required")
        } else if self
            .styles
            .iter()
            .enumerate()
            .any(|(i, s)| i != idx && s.name == new)
        {
            Some("That name is already used")
        } else {
            None
        }
    }

    /// A new style: a copy of `from`, or the default look, with a fresh name.
    fn add_style(&mut self, from: Option<usize>) -> usize {
        let mut style = from
            .and_then(|i| self.styles.get(i).cloned())
            .unwrap_or_default();
        style.name = unique_name(
            &if from.is_some() {
                format!("{} Copy", style.name)
            } else {
                "New Text Style".to_string()
            },
            |n| self.get(n).is_some(),
        );
        self.styles.push(style);
        self.styles.len() - 1
    }

    fn delete_style(&mut self, idx: usize) -> Result<(), String> {
        let name = self.styles.get(idx).ok_or("No such style")?.name.clone();
        if name == DEFAULT_TEXT_STYLE_NAME {
            return Err("\"Default Text Style\" cannot be deleted".into());
        }
        self.styles.remove(idx);
        Ok(())
    }
}

pub struct TextStylesDialog {
    scope: StyleScope,
    plan: TextStyles,
    defaults: TextStyles,
    /// Names the plan's styles had when the dialog opened (to follow
    /// renames into layers).
    plan_orig: Vec<String>,
    selected: usize,
    message: Option<String>,
    /// Replace Fonts: the family to replace and the one to use instead.
    replace_from: String,
    replace_to: String,
    /// What the last Replace Fonts did.
    notice: Option<String>,
}

impl TextStylesDialog {
    pub fn new(cx: &EditorContext) -> Self {
        Self {
            scope: StyleScope::Plan,
            plan: cx.project.text_styles.clone(),
            defaults: cx.defaults.text_styles.clone(),
            plan_orig: cx
                .project
                .text_styles
                .styles
                .iter()
                .map(|s| s.name.clone())
                .collect(),
            selected: 0,
            message: None,
            replace_from: String::new(),
            replace_to: String::new(),
            notice: None,
        }
    }

    /// Writes both lists. The plan's list is one undo step; a style renamed
    /// in the plan is renamed on the layers that used it.
    pub fn apply(&self, cx: &mut EditorContext) {
        if self.plan != cx.project.text_styles {
            cx.begin_change("Text Styles");
            for (i, orig) in self.plan_orig.iter().enumerate() {
                // Styles keep their position unless some were deleted; match
                // by position only while the list shape is unchanged.
                if let Some(now) = self.plan.styles.get(i).filter(|s| s.name != *orig) {
                    for l in &mut cx.project.layers.layers {
                        if l.text_style == *orig {
                            l.text_style = now.name.clone();
                        }
                    }
                }
            }
            cx.project.text_styles = self.plan.clone();
            cx.mark_dirty();
        }
        cx.defaults.text_styles = self.defaults.clone();
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut scope = self.scope;
        let mut selected = self.selected;
        let mut message = self.message.take();
        let mut notice = self.notice.take();
        let (mut replace_from, mut replace_to) = (
            std::mem::take(&mut self.replace_from),
            std::mem::take(&mut self.replace_to),
        );
        let (plan, defaults) = (&mut self.plan, &mut self.defaults);
        let outcome = list_window(
            ctx,
            "Text Styles",
            "text_styles",
            true,
            None,
            [640.0, 460.0],
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Edit styles of");
                    for s in [StyleScope::Plan, StyleScope::Defaults] {
                        if ui.radio_value(&mut scope, s, s.label()).clicked() {
                            selected = 0;
                            message = None;
                        }
                    }
                });
                ui.separator();
                let list = match scope {
                    StyleScope::Plan => &mut *plan,
                    StyleScope::Defaults => &mut *defaults,
                };
                selected = selected.min(list.styles.len().saturating_sub(1));
                ui.horizontal_top(|ui| {
                    ui.allocate_ui(egui::vec2(190.0, ui.available_height() - 4.0), |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("text_styles_list")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (i, s) in list.styles.iter().enumerate() {
                                    if ui.selectable_label(selected == i, &s.name).clicked() {
                                        selected = i;
                                    }
                                }
                            });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        let sel = selected;
                        if let Some(err) = style_form(ui, list, sel) {
                            message = Some(err.to_string());
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("New").clicked() {
                                selected = list.add_style(None);
                                message = None;
                            }
                            if ui.button("Copy").clicked() {
                                selected = list.add_style(Some(sel));
                                message = None;
                            }
                            if ui.button("Delete").clicked() {
                                match list.delete_style(sel) {
                                    Ok(()) => {
                                        selected = sel.saturating_sub(1);
                                        message = None;
                                    }
                                    Err(e) => message = Some(e),
                                }
                            }
                        });
                        if let Some(m) = &message {
                            ui.colored_label(ERROR_RED, m);
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        replace_fonts_section(
                            ui,
                            list,
                            &mut replace_from,
                            &mut replace_to,
                            &mut notice,
                        );
                    });
                });
            },
        );
        self.scope = scope;
        self.selected = selected;
        self.message = message;
        self.notice = notice;
        self.replace_from = replace_from;
        self.replace_to = replace_to;
        outcome
    }
}

/// Replace Fonts (Chief TXT-12): every style set in one family is set in
/// another. `from` offers the families the list's styles use now.
fn replace_fonts_section(
    ui: &mut Ui,
    list: &mut TextStyles,
    from: &mut String,
    to: &mut String,
    notice: &mut Option<String>,
) {
    ui.label(egui::RichText::new("Replace Fonts").strong());
    let used = list.fonts_used();
    if !used.iter().any(|u| u == from) {
        *from = used.first().cloned().unwrap_or_default();
    }
    ui.horizontal(|ui| {
        ui.label("Replace");
        let names: Vec<&str> = used.iter().map(String::as_str).collect();
        string_combo(ui, "replace_fonts_from", from, &names);
        ui.label("with");
    });
    crate::fonts::font_picker(ui, "replace_fonts_to", to, "", false, false);
    let ready = !from.is_empty()
        && !to.trim().is_empty()
        && !plan_core::text_styles::same_font_family(from, to);
    if ui
        .add_enabled(ready, egui::Button::new("Replace in all styles"))
        .clicked()
    {
        let n = list.replace_font(from, to);
        *notice = Some(match n {
            0 => "No style uses that font.".to_string(),
            1 => format!("1 style now uses {to}."),
            n => format!("{n} styles now use {to}."),
        });
    }
    if let Some(n) = notice {
        ui.weak(n.as_str());
    }
}

/// The form for style `idx` of `list`; returns an error to show.
fn style_form(ui: &mut Ui, list: &mut TextStyles, idx: usize) -> Option<&'static str> {
    let Some(style) = list.styles.get(idx).cloned() else {
        ui.weak("No style selected");
        return None;
    };
    let mut s = style.clone();
    let mut error = None;
    section(ui, "Text Style");
    row(ui, "Name", |ui| {
        let locked = style.name == DEFAULT_TEXT_STYLE_NAME;
        let mut name = s.name.clone();
        let r = ui.add_enabled(
            !locked,
            egui::TextEdit::singleline(&mut name).desired_width(180.0),
        );
        if r.changed() {
            match list.name_error(idx, &name) {
                None => s.name = name.trim().to_string(),
                Some(e) => error = Some(e),
            }
        }
    });
    row(ui, "Font", |ui| {
        // Installed families with a preview in the face the style maps to.
        ui.vertical(|ui| {
            if crate::fonts::font_picker(
                ui,
                "text_style_font",
                &mut s.font,
                &style.font_style,
                s.bold,
                s.italic,
            ) {
                s.font_style.clear();
            }
        });
    });
    row(ui, "Height", |ui| {
        let r = ui.add(
            egui::DragValue::new(&mut s.height_in)
                .speed(0.1)
                .range(0.25..=96.0)
                .suffix("\""),
        );
        // A character-height style prints this tall at 1/4" scale.
        if r.changed() && !s.is_printed_size() {
            s.printed_pt = Some(s.height_in * 0.25 / 12.0 * 72.0);
        }
    });
    row(ui, "Size by", |ui| {
        let mut printed = s.is_printed_size();
        ui.radio_value(&mut printed, false, "Character Height");
        ui.radio_value(&mut printed, true, "Printed Size");
        if printed != s.is_printed_size() {
            s.use_printed_size(printed);
        }
    });
    if s.is_printed_size() {
        row(ui, "Printed Size", |ui| {
            let mut inches = s.printed_in();
            if ui
                .add(
                    egui::DragValue::new(&mut inches)
                        .speed(0.005)
                        .range(0.02..=2.0)
                        .suffix("\" on paper"),
                )
                .changed()
            {
                s.set_printed_in(inches);
            }
        });
    }
    row(ui, "Style", |ui| {
        ui.checkbox(&mut s.bold, "Bold");
        ui.checkbox(&mut s.italic, "Italic");
        ui.checkbox(&mut s.underline, "Underline");
    });
    // Bold and italic pick the real face, so a named face style (Heavy)
    // steps aside when either is toggled.
    if s.bold != style.bold || s.italic != style.italic {
        s.font = plan_core::text_styles::split_font_name(&s.font)
            .0
            .to_string();
        s.font_style.clear();
    }
    row(ui, "Color", |ui| {
        ui.color_edit_button_srgb(&mut s.color);
    });
    if s != style {
        list.styles[idx] = s;
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn defaults() -> PlanDefaults {
        plan_defaults::embedded()
    }

    #[test]
    fn dimension_sets_copy_rename_delete_and_activate() {
        let mut d = defaults();
        let mut s = DimensionSets::from_defaults(&d);
        let n = s.sets.len();
        let active = s.active.clone();
        let i = s.sets.iter().position(|x| x.name == active).unwrap();

        let c = s.copy(i).unwrap();
        assert_eq!(s.sets.len(), n + 1);
        assert_eq!(s.sets[c].name, format!("{active} Copy"));
        assert_eq!(
            s.sets[c].auto.set_name,
            format!("{active} Copy Dimension Defaults")
        );
        // A second copy gets a number.
        let c2 = s.copy(i).unwrap();
        assert_eq!(s.sets[c2].name, format!("{active} Copy 2"));

        // Rename: unique and non-empty; the active name follows.
        assert!(s.rename(c, "").is_err());
        assert!(s.rename(c, &active).is_err());
        s.rename(i, "Mine").unwrap();
        assert_eq!(s.active, "Mine");

        // The active set cannot be deleted; others can.
        assert!(s.delete(i).is_err());
        s.delete(c2).unwrap();
        assert_eq!(s.sets.len(), n + 1);

        // Applying activates the chosen set and loads its settings.
        let copy_name = format!("{active} Copy");
        let other = s.sets.iter().position(|x| x.name == copy_name).unwrap();
        s.sets[other].auto.arrow_size = 7.0;
        s.active = copy_name.clone();
        s.apply(&mut d);
        assert_eq!(d.active_dimension_set, copy_name);
        assert_eq!(d.dimensions.arrow_size, 7.0);
        assert!(d.dimension_set("Mine").is_some());
    }

    #[test]
    fn the_last_dimension_set_stays() {
        let mut s = DimensionSets::from_defaults(&defaults());
        s.sets.truncate(1);
        s.active = "elsewhere".into();
        assert!(s.delete(0).is_err());
    }

    #[test]
    fn editing_a_set_keeps_its_text_format_in_step() {
        let set = defaults().dimension_sets[2].clone();
        let mut form = DimensionSetForm::new(set);
        form.draft.auto.smallest_fraction = 16;
        form.draft.auto.unit_indicators = false;
        form.draft.auto.trailing_zeroes = true;
        form.draft.format.length.as_mut().unwrap().unit = LengthUnit::Inches;
        let done = form.finished();
        let l = done.format.length.unwrap();
        assert_eq!(l.fraction_denominator, 16);
        assert!(!l.unit_indicators && l.trailing_zeroes);
        assert_eq!(l.unit, LengthUnit::Inches);
        assert_eq!(done.format.smallest_fraction, 16);
        assert!(!done.format.unit_indicators);
        // The text format is what the preview shows.
        assert_eq!(done.format.fmt_len(150.5), "150 1/2");
    }

    #[test]
    fn room_types_add_rename_delete_and_follow_into_rooms() {
        let mut cx = EditorContext::new(defaults());
        let mut list = RoomTypeList::from_defaults(&cx.defaults);
        let n = list.types.len();
        let i = list.add();
        assert_eq!(list.types[i].1.name, "New Room Type");
        assert_eq!(list.add(), i + 1);
        assert_eq!(list.types[i + 1].1.name, "New Room Type 2");
        assert!(list.rename(i, "Kitchen").is_err(), "Kitchen exists");
        list.rename(i, "Wine Cellar").unwrap();

        let bath = list
            .types
            .iter()
            .position(|(_, t)| t.name == "Bath")
            .unwrap();
        list.rename(bath, "Bathroom").unwrap();
        assert_eq!(
            list.renames(),
            vec![("Bath".to_string(), "Bathroom".to_string())]
        );
        let unspecified = list
            .types
            .iter()
            .position(|(_, t)| t.name == "Unspecified")
            .unwrap();
        assert!(list.delete(unspecified).is_err());
        list.delete(i + 1).unwrap();
        assert_eq!(list.types.len(), n + 1);

        // A named room of the renamed type follows, as one undo step.
        cx.project.floors[0]
            .room_names
            .push(plan_core::model::RoomName::new(
                plan_core::Point::new(1.0, 1.0),
                "Hall Bath",
                "Bath",
            ));
        list.apply(&mut cx);
        assert_eq!(cx.project.floors[0].room_names[0].room_type, "Bathroom");
        assert!(cx.defaults.room_type("Wine Cellar").is_some());
        assert!(cx.defaults.room_type("Bath").is_none());
        cx.undo();
        assert_eq!(cx.project.floors[0].room_names[0].room_type, "Bath");
    }

    #[test]
    fn text_styles_add_copy_delete_and_apply_to_plan_and_defaults() {
        let mut cx = EditorContext::new(defaults());
        let mut dlg = TextStylesDialog::new(&cx);
        let n = dlg.plan.styles.len();
        let i = dlg.plan.add_style(None);
        assert_eq!(dlg.plan.styles[i].name, "New Text Style");
        let c = dlg.plan.add_style(Some(0));
        assert_eq!(dlg.plan.styles[c].name, "Default Text Style Copy");
        assert!(dlg.plan.name_error(c, "New Text Style").is_some());
        assert!(dlg.plan.delete_style(0).is_err(), "the default stays");
        dlg.plan.delete_style(c).unwrap();
        dlg.plan.styles[i].bold = true;
        dlg.plan.styles[i].color = [10, 20, 30];
        dlg.defaults.styles[0].font = "Georgia".into();
        dlg.apply(&mut cx);
        assert_eq!(cx.project.text_styles.styles.len(), n + 1);
        assert!(cx.project.text_styles.get("New Text Style").unwrap().bold);
        assert_eq!(cx.defaults.text_styles.styles[0].font, "Georgia");
        cx.undo();
        assert_eq!(cx.project.text_styles.styles.len(), n);
    }

    #[test]
    fn replace_fonts_updates_every_style_with_one_undo_step() {
        let mut cx = EditorContext::new(defaults());
        let before = cx.project.text_styles.clone();
        let n = before.styles.len();
        assert!(n >= 3);
        let mut dlg = TextStylesDialog::new(&cx);
        let used = dlg.plan.fonts_used();
        assert_eq!(used, vec!["Arial"], "the shipped styles are Arial");
        assert_eq!(dlg.plan.replace_font("Arial", "Helvetica Neue"), n);
        assert_eq!(dlg.plan.replace_font("Arial", "Georgia"), 0, "nothing left");
        dlg.apply(&mut cx);
        assert!(cx
            .project
            .text_styles
            .styles
            .iter()
            .all(|s| s.font == "Helvetica Neue"));
        // The new-plan defaults are separate and untouched.
        assert!(cx
            .defaults
            .text_styles
            .styles
            .iter()
            .all(|s| s.font != "Helvetica Neue"));
        // One undo step restores every style.
        assert_eq!(cx.undo().as_deref(), Some("Text Styles"));
        assert_eq!(cx.project.text_styles, before);
        // The defaults scope replaces in the defaults only.
        let mut dlg = TextStylesDialog::new(&cx);
        assert!(dlg.defaults.replace_font("arial", "Georgia") > 0);
        dlg.apply(&mut cx);
        assert!(cx
            .defaults
            .text_styles
            .styles
            .iter()
            .all(|s| s.font == "Georgia"));
        assert_eq!(cx.project.text_styles, before);
    }

    #[test]
    fn a_renamed_plan_style_follows_into_the_layers() {
        let mut cx = EditorContext::new(defaults());
        let first = cx.project.text_styles.styles[1].name.clone();
        cx.project.layers.layers[0].text_style = first.clone();
        let mut dlg = TextStylesDialog::new(&cx);
        dlg.plan.styles[1].name = "Renamed".into();
        dlg.apply(&mut cx);
        assert_eq!(cx.project.layers.layers[0].text_style, "Renamed");
    }

    #[test]
    fn every_list_dialog_draws_without_panicking() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(defaults());
        for mut dlg in [
            DefaultsList::dimensions(&cx),
            DefaultsList::room_types(&cx),
            DefaultsList::text_styles(&cx),
        ] {
            for _ in 0..3 {
                let mut open = true;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    open = dlg.show(ctx, &mut cx);
                });
                assert!(open);
            }
        }
        // The Dimension Defaults editor draws every tab.
        let mut form = DimensionSetForm::new(cx.defaults.dimension_sets[0].clone());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for tab in 0..DIM_TABS.len() {
                    form.page(ui, tab);
                }
                let p = ui.painter().clone();
                form.preview(&p, ui.max_rect());
            });
        });
    }
}
