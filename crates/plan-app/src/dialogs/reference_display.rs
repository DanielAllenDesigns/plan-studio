//! Reference Display and the Change Floor/Reference dialog (R-65, R-66,
//! LAY-10, LAY-39..LAY-45; manual pp. 89-91): which floors, of this plan or
//! of other plan files, the plan shows dimmed under (or over) the active
//! floor, with which layer set, how, and where.
//!
//! The dialog lists the floors (the Current Floor) and the table of
//! reference rows in draw order, with the Current line among them. A row
//! names a plan (this one, or a file read only), the floor (Automatic, a
//! fixed floor, Match Current), the layer set and whether fill patterns
//! show (Details); another plan also takes X/Y/Z offsets and an Angle,
//! which Edit Reference Document Offset sets with the mouse. The table is
//! kept in the plan ([`plan_core::construction::ReferenceTable`]); while it
//! is empty one row stands for the session's choices ([`ReferenceSettings`]:
//! the floor below, gray), as before.
//!
//! * [`CHANGE`] opens the dialog (Tools > Floor/Reference Display, the
//!   Reference Display Options button); [`host_frame`] shows it every
//!   frame and an OK is one undo step ([`apply_dialog`]).
//! * [`SWAP`] is Swap Floor/Reference: go to the reference floor and make
//!   the old floor the reference (not with several rows).
//! * [`EDIT_OFFSET`] is Edit Reference Document Offset (the
//!   `ReferenceOffset` tool).

use super::{row, section, Fields, Outcome};
use crate::editor::{EditorContext, EditorRequest};
use crate::toolbar::ViewFlag;
use crate::tools::ToolId;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::construction::{ReferenceRow, ReferenceSource, ReferenceTable, RowFloor, TableLine};
use plan_core::{Project, Wall};
use std::cell::RefCell;

/// Custom command: open the Change Floor/Reference dialog.
pub const CHANGE: &str = "reference.change";
/// Custom command: Swap Floor/Reference (Go to the Reference Floor).
pub const SWAP: &str = "reference.swap";
/// Custom command: Edit Reference Document Offset.
pub const EDIT_OFFSET: &str = "reference.edit_offset";

/// The name of the layer set a new reference row starts with when the plan
/// has it.
pub const REFERENCE_LAYER_SET: &str = "Reference Display Layer Set";

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
    static HOST: RefCell<Option<ReferenceDisplayDialog>> = const { RefCell::new(None) };
}

/// The session's choices (or the active plan view's floor) while the plan's
/// table is empty.
fn session_settings(project: &Project) -> ReferenceSettings {
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

/// The settings a table row stands for.
fn settings_of_row(row: &ReferenceRow) -> ReferenceSettings {
    ReferenceSettings {
        floor: match row.floor {
            RowFloor::Above => ReferenceFloor::Above,
            RowFloor::Fixed(i) => ReferenceFloor::Floor(i),
            RowFloor::Below | RowFloor::Automatic | RowFloor::MatchCurrent => ReferenceFloor::Below,
        },
        layer_set: row.layer_set.clone(),
        color: row.color,
    }
}

/// The row a set of session choices stands for.
fn row_of_settings(s: &ReferenceSettings) -> ReferenceRow {
    ReferenceRow {
        floor: match s.floor {
            ReferenceFloor::Below => RowFloor::Below,
            ReferenceFloor::Above => RowFloor::Above,
            ReferenceFloor::Floor(i) => RowFloor::Fixed(i),
        },
        layer_set: s.layer_set.clone(),
        color: s.color,
        ..ReferenceRow::default()
    }
}

/// The row that stands for the session's choices while the plan's table is
/// empty.
pub fn default_row(project: &Project) -> ReferenceRow {
    row_of_settings(&session_settings(project))
}

/// The choices in force: the plan's table (its first row of this plan), else
/// the session's, else the floor below in gray, with the active plan view's
/// `reference_floor` (relative to `current`) deciding above or below when it
/// has one.
pub fn settings(project: &Project) -> ReferenceSettings {
    project
        .reference_table
        .rows
        .iter()
        .find(|r| r.source.is_this_plan())
        .map(settings_of_row)
        .unwrap_or_else(|| session_settings(project))
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

/// The walls of the reference floor of this plan that draw and snap (R-65,
/// LAY-10): nothing when Reference Display is off or there is no such floor;
/// else the walls on layers that show (in the chosen layer set, or the active
/// view's) and whose layer has its "Ref" box on. This is the first reference
/// row of this plan; [`crate::editor::ref_overlay::reference_layers`] has
/// every row, other plan files included.
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
            !w.flags.invisible && layers.is_visible(&w.layer) && layers.shows_in_reference(&w.layer)
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

// ---------------------------------------------------------------------------
// The dialog
// ---------------------------------------------------------------------------

/// What to do once the dialog's OK has been applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    Swap,
    EditOffset,
}

/// Change Floor/Reference dialog: the Current Floor, Show Reference
/// Floor(s) and the table of reference rows.
pub struct ReferenceDisplayDialog {
    /// The first reference row of this plan, in the session-settings form
    /// (what the dialog's older callers read).
    settings: ReferenceSettings,
    /// Turn the display on when OK is pressed.
    show: bool,
    floors: Vec<String>,
    current: usize,
    layer_sets: Vec<String>,
    table: ReferenceTable,
    /// The selected line of [`ReferenceTable::lines`].
    selected: usize,
    /// The floor OK makes current.
    new_current: usize,
    /// The layer set a new row starts with.
    default_layer_set: Option<String>,
    follow: Option<Follow>,
    fields: Fields,
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
        let mut table = ReferenceTable::default();
        table.rows.push(row_of_settings(&settings));
        Self::with_table(table, settings, show, floors, current, layer_sets)
    }

    /// The dialog on the plan's table (the default row while it is empty).
    pub fn for_plan(cx: &EditorContext) -> Self {
        let layer_sets = cx
            .project
            .layer_sets
            .names()
            .into_iter()
            .map(String::from)
            .collect();
        Self::with_table(
            crate::editor::ref_overlay::table_for_dialog(&cx.project),
            settings(&cx.project),
            cx.view_flags.contains(&ViewFlag::ReferenceDisplay),
            cx.project.floors.iter().map(|f| f.name.clone()).collect(),
            cx.floor,
            layer_sets,
        )
    }

    fn with_table(
        table: ReferenceTable,
        settings: ReferenceSettings,
        show: bool,
        floors: Vec<String>,
        current: usize,
        layer_sets: Vec<String>,
    ) -> Self {
        let default_layer_set = layer_sets
            .iter()
            .find(|n| n.as_str() == REFERENCE_LAYER_SET)
            .cloned();
        // Select the first reference row.
        let selected = table
            .lines()
            .iter()
            .position(|l| matches!(l, TableLine::Row(_)))
            .unwrap_or(0);
        Self {
            settings,
            show,
            floors,
            current,
            layer_sets,
            table,
            selected,
            new_current: current,
            default_layer_set,
            follow: None,
            fields: Fields::default(),
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

    pub fn table(&self) -> &ReferenceTable {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut ReferenceTable {
        &mut self.table
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, line: usize) {
        self.selected = line.min(self.table.lines().len() - 1);
    }

    /// The floor OK makes current.
    pub fn new_current(&self) -> usize {
        self.new_current
    }

    pub fn set_current_floor(&mut self, i: usize) {
        if i < self.floors.len() {
            self.new_current = i;
        }
    }

    pub fn follow(&self) -> Option<Follow> {
        self.follow
    }

    /// A new row: the Automatic floor and the Reference Display Layer Set.
    fn new_row(&self) -> ReferenceRow {
        ReferenceRow {
            layer_set: self.default_layer_set.clone(),
            ..ReferenceRow::default()
        }
    }

    pub fn insert_above(&mut self) {
        let row = self.new_row();
        self.selected = self.table.insert_above(self.selected, row);
    }

    pub fn insert_below(&mut self) {
        let row = self.new_row();
        self.selected = self.table.insert_below(self.selected, row);
    }

    pub fn move_up(&mut self) {
        if let Some(i) = self.table.move_up(self.selected) {
            self.selected = i;
        }
    }

    pub fn move_down(&mut self) {
        if let Some(i) = self.table.move_down(self.selected) {
            self.selected = i;
        }
    }

    pub fn can_delete(&self) -> bool {
        self.table.can_delete(self.selected)
    }

    pub fn delete(&mut self) {
        if self.table.delete(self.selected) {
            self.selected = self.selected.min(self.table.lines().len() - 1);
        }
    }

    /// The reference row at table line `line`, if that line is a row.
    pub fn row_at(&self, line: usize) -> Option<&ReferenceRow> {
        let at = self.table.current_at.min(self.table.rows.len());
        match line.cmp(&at) {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => self.table.rows.get(line),
            std::cmp::Ordering::Greater => self.table.rows.get(line - 1),
        }
    }

    /// Mutable access to the row at table line `line`.
    pub fn row_at_mut(&mut self, line: usize) -> Option<&mut ReferenceRow> {
        let at = self.table.current_at.min(self.table.rows.len());
        match line.cmp(&at) {
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Less => self.table.rows.get_mut(line),
            std::cmp::Ordering::Greater => self.table.rows.get_mut(line - 1),
        }
    }

    /// Points the row at table line `line` at another plan file (Choose
    /// Existing Plan): it shows the same floor level as the current plan.
    pub fn set_row_file(&mut self, line: usize, path: &str) {
        if let Some(r) = self.row_at_mut(line) {
            r.source = ReferenceSource::File(path.to_string());
            r.floor = RowFloor::MatchCurrent;
            r.layer_set = None;
        }
    }

    /// Keeps the older settings form in step with the table.
    fn sync_settings(&mut self) {
        if let Some(r) = self.table.rows.iter().find(|r| r.source.is_this_plan()) {
            self.settings = settings_of_row(r);
        }
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Change Floor/Reference")
            .id(egui::Id::new("reference_display_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(620.0);
                self.current_floor_list(ui);
                section(ui, "Reference Display");
                ui.checkbox(&mut self.show, "Show Reference Floor(s)");
                self.table_ui(ui);
                self.selected_row_ui(ui);
                ui.checkbox(&mut self.table.xor, "XOR drawing")
                    .on_hover_text(
                    "Lines of the reference laid over lines of the current floor change color; \
                     identical lines are not drawn",
                );
                ui.weak(
                    "Reference objects are drawn dimmed and cannot be picked; their wall ends and \
                     crossings snap.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let one = self.table.rows.len() <= 1;
                    if ui
                        .add_enabled(one, egui::Button::new("Go to Reference Floor"))
                        .on_disabled_hover_text("Not available with several Reference Floors")
                        .clicked()
                    {
                        self.follow = Some(Follow::Swap);
                        outcome = Outcome::Ok;
                    }
                    let files = !self.table.file_rows().is_empty();
                    if ui
                        .add_enabled(files, egui::Button::new("Edit Reference Document Offset"))
                        .on_disabled_hover_text("Needs a row that refers to another plan file")
                        .clicked()
                    {
                        self.follow = Some(Follow::EditOffset);
                        outcome = Outcome::Ok;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("   OK   ").strong()).clicked() {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                    });
                });
            });
        self.sync_settings();
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

    fn current_floor_list(&mut self, ui: &mut egui::Ui) {
        section(ui, "Current Floor");
        egui::ScrollArea::vertical()
            .id_salt("ref_current_floor")
            .max_height(80.0)
            .show(ui, |ui| {
                for (i, name) in self.floors.clone().iter().enumerate().rev() {
                    if ui.selectable_label(self.new_current == i, name).clicked() {
                        self.new_current = i;
                    }
                }
            });
    }

    fn table_ui(&mut self, ui: &mut egui::Ui) {
        let lines = self.table.lines();
        ui.horizontal_top(|ui| {
            egui::Grid::new("ref_table")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for h in ["Current", "Plan", "Floor", "Layer Set", "Details"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, line) in lines.iter().enumerate() {
                        let picked = self.selected == i;
                        match line {
                            TableLine::Current => {
                                if ui.selectable_label(picked, "\u{25CF}").clicked() {
                                    self.selected = i;
                                }
                                ui.label("Current Plan");
                                ui.label(
                                    self.floors
                                        .get(self.new_current)
                                        .cloned()
                                        .unwrap_or_default(),
                                );
                                ui.label("");
                                ui.label("");
                            }
                            TableLine::Row(_) => {
                                if ui.selectable_label(picked, "").clicked() {
                                    self.selected = i;
                                }
                                self.row_cells(ui, i);
                            }
                        }
                        ui.end_row();
                    }
                });
            ui.vertical(|ui| {
                if ui.button("Insert Above").clicked() {
                    self.insert_above();
                }
                if ui.button("Insert Below").clicked() {
                    self.insert_below();
                }
                if ui
                    .add_enabled(self.selected > 0, egui::Button::new("Move Up"))
                    .clicked()
                {
                    self.move_up();
                }
                if ui
                    .add_enabled(
                        self.selected + 1 < self.table.lines().len(),
                        egui::Button::new("Move Down"),
                    )
                    .clicked()
                {
                    self.move_down();
                }
                if ui
                    .add_enabled(self.can_delete(), egui::Button::new("Delete"))
                    .clicked()
                {
                    self.delete();
                }
            });
        });
    }

    /// The Plan, Floor, Layer Set and Details cells of the row at line `i`.
    fn row_cells(&mut self, ui: &mut egui::Ui, i: usize) {
        let Some(mut r) = self.row_at(i).cloned() else {
            return;
        };
        let before = r.clone();
        // Plan.
        let mut choose = false;
        egui::ComboBox::from_id_salt(("ref_plan", i))
            .width(130.0)
            .selected_text(r.source.label())
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(r.source.is_this_plan(), "Current Plan")
                    .clicked()
                {
                    r.source = ReferenceSource::ThisPlan;
                    r.floor = RowFloor::Automatic;
                    r.layer_set = self.default_layer_set.clone();
                }
                if let ReferenceSource::File(_) = &r.source {
                    let _ = ui.selectable_label(true, r.source.label());
                }
                if ui
                    .selectable_label(false, "Choose Existing Plan\u{2026}")
                    .clicked()
                {
                    choose = true;
                }
            });
        // Floor.
        let other = !r.source.is_this_plan();
        let floor_names = self.floor_names_of(&r.source);
        egui::ComboBox::from_id_salt(("ref_floor", i))
            .width(110.0)
            .selected_text(floor_label(r.floor, &floor_names))
            .show_ui(ui, |ui| {
                let mut choices = vec![RowFloor::Automatic];
                if other {
                    choices.push(RowFloor::MatchCurrent);
                } else {
                    choices.push(RowFloor::Below);
                    choices.push(RowFloor::Above);
                }
                choices.extend((0..floor_names.len()).map(RowFloor::Fixed));
                for c in choices {
                    ui.selectable_value(&mut r.floor, c, floor_label(c, &floor_names));
                }
            });
        // Layer set.
        let sets = self.layer_sets_of(&r.source);
        egui::ComboBox::from_id_salt(("ref_set", i))
            .width(150.0)
            .selected_text(
                r.layer_set
                    .clone()
                    .unwrap_or_else(|| "Active view's layers".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut r.layer_set, None, "Active view's layers");
                for n in sets {
                    ui.selectable_value(&mut r.layer_set, Some(n.clone()), n);
                }
            });
        ui.checkbox(&mut r.details, "");
        if choose {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Plan Studio", &["psplan"])
                .pick_file()
            {
                r.source = ReferenceSource::File(path.to_string_lossy().into_owned());
                r.floor = RowFloor::MatchCurrent;
                r.layer_set = None;
            }
        }
        if r != before {
            if let Some(slot) = self.row_at_mut(i) {
                *slot = r;
            }
        }
    }

    /// The floor names of the plan a source refers to (this plan's, or the
    /// other plan's when it can be read).
    fn floor_names_of(&self, source: &ReferenceSource) -> Vec<String> {
        match source {
            ReferenceSource::ThisPlan => self.floors.clone(),
            ReferenceSource::File(p) => crate::editor::ref_overlay::other_plan(p)
                .map(|plan| plan.floors.iter().map(|f| f.name.clone()).collect())
                .unwrap_or_default(),
        }
    }

    /// The layer set names of the plan a source refers to.
    fn layer_sets_of(&self, source: &ReferenceSource) -> Vec<String> {
        match source {
            ReferenceSource::ThisPlan => self.layer_sets.clone(),
            ReferenceSource::File(p) => crate::editor::ref_overlay::other_plan(p)
                .map(|plan| {
                    plan.layer_sets
                        .names()
                        .into_iter()
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Color, and X/Y/Z offsets and Angle for another plan, of the
    /// selected row.
    fn selected_row_ui(&mut self, ui: &mut egui::Ui) {
        let sel = self.selected;
        let Some(mut r) = self.row_at(sel).cloned() else {
            ui.weak("The Current line's attributes cannot be edited here.");
            return;
        };
        let before = r.clone();
        section(ui, "Selected Row");
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut r.color);
        });
        if !r.source.is_this_plan() {
            self.fields
                .length_row(ui, "X Offset", "ref_x", &mut r.offset[0]);
            self.fields
                .length_row(ui, "Y Offset", "ref_y", &mut r.offset[1]);
            self.fields
                .length_row(ui, "Z Offset", "ref_z", &mut r.offset[2]);
            self.fields
                .degrees_row(ui, "Angle", "deg_ref_angle", &mut r.angle_deg);
        }
        if r != before {
            if let Some(slot) = self.row_at_mut(sel) {
                *slot = r;
            }
        }
    }
}

fn floor_label(f: RowFloor, names: &[String]) -> String {
    match f {
        RowFloor::Fixed(i) => names
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("Floor {}", i + 1)),
        other => other.label(),
    }
}

// ---------------------------------------------------------------------------
// Applying, swapping, hosting
// ---------------------------------------------------------------------------

/// The relative floor offset a plan view stores for `row` while `current` is
/// active.
fn view_offset(row: &ReferenceRow, current: usize) -> i32 {
    match row.floor {
        RowFloor::Above => 1,
        RowFloor::Fixed(i) => i as i32 - current as i32,
        RowFloor::Automatic if current == 0 => 1,
        _ => -1,
    }
}

/// OK of the Change Floor/Reference dialog: the table, the Current Floor and
/// Show Reference Floor(s) in one undo step.
pub fn apply_dialog(cx: &mut EditorContext, d: &ReferenceDisplayDialog) {
    cx.begin_change("Reference Display");
    cx.project.reference_table = d.table.clone();
    // The table is the truth now; the session's choices stand aside.
    reset_settings();
    let first_this = d
        .table
        .rows
        .iter()
        .find(|r| r.source.is_this_plan())
        .map(|r| view_offset(r, d.new_current));
    let active = cx.project.active_plan_view.clone();
    if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == active) {
        if let Some(off) = first_this {
            v.reference_floor = Some(off);
        }
        v.reference_display = d.show_display();
    }
    if d.show_display() {
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
    } else {
        cx.view_flags.remove(&ViewFlag::ReferenceDisplay);
    }
    if d.new_current != cx.floor && d.new_current < cx.project.floors.len() {
        cx.floor = d.new_current;
        cx.reset_view_state();
    }
    cx.mark_dirty();
    cx.status = "Updated the reference display".into();
}

/// Swap Floor/Reference (manual p. 91): goes to the reference floor, which
/// becomes the Current Floor, and the old Current Floor becomes the
/// reference; swapping again returns. Not with several Reference Floors and
/// not when the reference is another plan file. Returns the floor now current.
pub fn swap_floor_reference(cx: &mut EditorContext) -> Result<usize, String> {
    let rows = crate::editor::ref_overlay::effective_rows(&cx.project);
    if rows.len() > 1 {
        return Err("Swap Floor/Reference is not available with several Reference Floors".into());
    }
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| "There is no reference floor".to_string())?;
    if !row.source.is_this_plan() {
        return Err("The reference is another plan file; there is no floor to go to".into());
    }
    let target = row
        .floor
        .resolve(cx.floor, cx.project.floors.len(), false)
        .ok_or_else(|| "There is no reference floor to go to".to_string())?;
    let old = cx.floor;
    if cx.project.reference_table.rows.is_empty() {
        let mut s = session_settings(&cx.project);
        s.floor = ReferenceFloor::Floor(old);
        set_settings(s);
    } else {
        cx.project.reference_table.rows[0].floor = RowFloor::Fixed(old);
    }
    cx.floor = target;
    cx.reset_view_state();
    cx.mark_dirty();
    cx.status = format!("Went to {}", cx.project.floors[target].name);
    Ok(target)
}

/// Opens the Change Floor/Reference dialog.
pub fn open_dialog(cx: &EditorContext) {
    let d = ReferenceDisplayDialog::for_plan(cx);
    HOST.with(|h| *h.borrow_mut() = Some(d));
}

/// Is the dialog open?
#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open dialog.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut ReferenceDisplayDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the dialog and applies it as OK would. Returns false when it was
/// not open.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    finish(cx, d);
    true
}

fn finish(cx: &mut EditorContext, d: ReferenceDisplayDialog) {
    apply_dialog(cx, &d);
    match d.follow {
        Some(Follow::Swap) => {
            if let Err(e) = swap_floor_reference(cx) {
                cx.status = e;
            }
        }
        Some(Follow::EditOffset) => run_edit_offset(cx),
        None => {}
    }
}

/// Shows the open dialog once a frame and applies its OK (called from
/// [`crate::tools::ToolSet::frame`]).
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => finish(cx, d),
    }
}

fn run_edit_offset(cx: &mut EditorContext) {
    if cx.project.reference_table.file_rows().is_empty() {
        cx.status = "No other plan file is in the Reference Display".into();
        return;
    }
    cx.requests
        .push(EditorRequest::SetTool(ToolId::ReferenceOffset));
}

/// The commands of this module (Custom action ids); true when `id` was one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CHANGE => open_dialog(cx),
        SWAP => {
            if let Err(e) = swap_floor_reference(cx) {
                cx.status = e;
            }
        }
        EDIT_OFFSET => run_edit_offset(cx),
        _ => return false,
    }
    true
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

    #[test]
    fn a_table_row_and_the_session_choices_stand_for_each_other() {
        let s = ReferenceSettings {
            floor: ReferenceFloor::Floor(2),
            layer_set: Some("Ref".into()),
            color: [1, 2, 3],
        };
        let row = row_of_settings(&s);
        assert_eq!(row.floor, RowFloor::Fixed(2));
        assert_eq!(settings_of_row(&row), s);
        assert_eq!(
            settings_of_row(&ReferenceRow::default()).floor,
            ReferenceFloor::Below
        );
        assert_eq!(view_offset(&ReferenceRow::default(), 0), 1);
        assert_eq!(view_offset(&ReferenceRow::default(), 2), -1);
    }

    #[test]
    fn rows_are_managed_through_the_dialog_with_the_current_line_among_them() {
        let mut d = ReferenceDisplayDialog::new(
            ReferenceSettings::default(),
            true,
            vec!["1st".into(), "2nd".into()],
            1,
            vec![REFERENCE_LAYER_SET.into(), "Other".into()],
        );
        // [Current, row]; the row is selected.
        assert_eq!(d.table().lines().len(), 2);
        assert_eq!(d.selected(), 1);
        assert!(!d.can_delete(), "the only reference row");
        d.insert_below();
        assert_eq!(d.selected(), 2);
        assert_eq!(
            d.row_at(2).unwrap().layer_set.as_deref(),
            Some(REFERENCE_LAYER_SET),
            "new rows start in the Reference Display Layer Set"
        );
        assert_eq!(d.row_at(2).unwrap().floor, RowFloor::Automatic);
        d.set_row_file(2, "/x/old.psplan");
        assert_eq!(d.row_at(2).unwrap().floor, RowFloor::MatchCurrent);
        d.move_up();
        assert_eq!(d.selected(), 1);
        assert!(d.row_at(0).is_none(), "line 0 is the Current line");
        assert!(d.can_delete());
        d.delete();
        assert_eq!(d.table().rows.len(), 1);
        d.set_current_floor(0);
        assert_eq!(d.new_current(), 0);
        d.set_current_floor(9);
        assert_eq!(d.new_current(), 0, "no such floor");
        d.select(0);
        assert!(!d.can_delete(), "the Current line stays");
    }
}
