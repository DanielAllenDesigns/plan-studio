//! Schedule Specification: the dialog behind a double-click on a placed
//! schedule (manual pp. 719 to 728), and the Schedule Defaults dialogs, which
//! look the same. Panels: General (title, floors, rooms, categories),
//! Columns/Rows, Number Formatting, Attributes, Line Style, Fill Style, the
//! three text styles and Labels. The panels are in [`panels`].
//!
//! The dialog edits a clone of the [`Schedule`]; OK hands it back and the
//! host stores it as one undo step. The General page also has *Export CSV*
//! and *Open in Window*; the dialog cannot reach the plan, so it only raises
//! those as [`SpecActions`] for the host to carry out. What the dialog needs
//! to know about the plan (floor names, rooms, the category trees) it gets
//! once, as a [`SpecContext`].
//!
//! The Schedule Defaults (`Project::schedule_setup`), the Create Schedule
//! from Room type chooser and the other windows of the schedule commands are
//! drawn by [`show_extras`], which the plan windows call once a frame.

mod panels;

use super::{on, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK};
use crate::editor::{schedule_view, Camera, EditorContext, EditorRequest};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, StrokeKind, Ui};
use plan_core::props::PropDef;
use plan_core::schedules::{ColumnSpec, RoomRef, Schedule, ScheduleKind};
use plan_core::Id;
use plan_docs::schedule_kinds::CategoryGroup;
use std::cell::RefCell;
use std::collections::BTreeSet;

const TABS: &[Tab] = &[
    on("General"),
    on("Columns/Rows"),
    on("Number Formatting"),
    on("Attributes"),
    on("Line Style"),
    on("Fill Style"),
    on("Main Text Style"),
    on("Title Text Style"),
    on("Header Text Style"),
    on("Labels"),
];

/// What the user asked for besides editing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpecActions {
    pub export_csv: bool,
    /// Save the table as an Excel workbook.
    pub export_xlsx: bool,
    pub open_window: bool,
    /// Put the schedule on a layout page as a box.
    pub send_to_layout: bool,
    /// Save the schedule as a workbook made for editing in Excel
    /// (`plan_docs::props_exchange`).
    pub export_for_editing: bool,
    /// Read property data back from a workbook or CSV.
    pub import_props: bool,
}

/// Which dialog this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The specification of a placed schedule.
    Placed,
    /// Schedule Defaults of one kind: no position, no output buttons.
    Defaults,
}

/// What the dialog needs to know about the plan.
#[derive(Clone, Debug, Default)]
pub struct SpecContext {
    /// Floor names, by index.
    pub floors: Vec<String>,
    /// Every room of every floor, with its name.
    pub rooms: Vec<(RoomRef, String)>,
    /// The Categories to Include tree of each kind.
    pub trees: Vec<(ScheduleKind, Vec<CategoryGroup>)>,
    /// The plan's custom schedule categories.
    pub custom: Vec<String>,
}

impl SpecContext {
    /// Reads the plan once.
    pub fn from_cx(cx: &EditorContext) -> Self {
        let rooms = plan_docs::schedule_kinds::room_choices(
            &cx.project,
            Some((cx.floor, cx.rooms.as_slice())),
        )
        .into_iter()
        .map(|(f, name, p)| (RoomRef::at(f, p), name))
        .collect();
        Self {
            floors: cx.project.floors.iter().map(|f| f.name.clone()).collect(),
            rooms,
            trees: ScheduleKind::ALL
                .iter()
                .map(|k| {
                    (
                        *k,
                        plan_docs::schedule_kinds::category_tree(&cx.project, *k),
                    )
                })
                .collect(),
            custom: cx
                .project
                .schedule_setup
                .categories
                .iter()
                .map(|c| c.name.clone())
                .collect(),
        }
    }

    pub fn tree_of(&self, kind: ScheduleKind) -> Vec<CategoryGroup> {
        self.trees
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, t)| t.clone())
            .unwrap_or_default()
    }
}

pub struct ScheduleSpecDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    pub(self) mode: Mode,
    /// Floor the schedule is placed on.
    pub(self) floor: usize,
    pub(self) def: Schedule,
    /// The plan's custom property definitions, for the property columns.
    prop_defs: Vec<PropDef>,
    pub(self) text_styles: Vec<String>,
    pub(self) layers: Vec<String>,
    actions: SpecActions,
    pub(self) ctx: SpecContext,
    /// The table's size now, for the Position fields.
    pub(self) size: (f64, f64),
    // ----- what the panels remember -----
    pub(self) avail_sel: BTreeSet<String>,
    pub(self) incl_sel: BTreeSet<String>,
    pub(self) limit_to_categories: bool,
    pub(self) rename_to: Option<String>,
    pub(self) rename_text: String,
    pub(self) fmt_sel: Option<String>,
    pub(self) new_category: String,
    pub(self) category_error: String,
    /// Custom categories made in this dialog, which the plan gets on OK.
    pub(self) new_categories: Vec<String>,
}

impl ScheduleSpecDialog {
    /// `text_styles` and `layers` are the plan's names for the pickers.
    pub fn new(floor: usize, def: Schedule, text_styles: Vec<String>, layers: Vec<String>) -> Self {
        let mut def = def;
        def.reconcile_columns();
        Self {
            frame: SpecDialog::new("Schedule Specification", "schedule_spec"),
            form: Form {
                mode: Mode::Placed,
                floor,
                def,
                prop_defs: Vec::new(),
                text_styles,
                layers,
                actions: SpecActions::default(),
                ctx: SpecContext::default(),
                size: (0.0, 0.0),
                avail_sel: BTreeSet::new(),
                incl_sel: BTreeSet::new(),
                limit_to_categories: true,
                rename_to: None,
                rename_text: String::new(),
                fmt_sel: None,
                new_category: String::new(),
                category_error: String::new(),
                new_categories: Vec::new(),
            },
        }
    }

    /// The Schedule Defaults dialog of `def.kind`.
    pub fn for_defaults(def: Schedule, text_styles: Vec<String>, layers: Vec<String>) -> Self {
        let kind = def.kind;
        let mut d = Self::new(0, def, text_styles, layers);
        d.frame = SpecDialog::new(
            format!("{} Defaults", kind.title()),
            ("schedule_defaults", kind as u8),
        );
        d.form.mode = Mode::Defaults;
        d
    }

    /// What the plan has: floors, rooms, categories.
    pub fn with_context(mut self, ctx: SpecContext) -> Self {
        self.form.ctx = ctx;
        self
    }

    /// The table's size now (plan inches), for the Position fields.
    pub fn with_size(mut self, size: (f64, f64)) -> Self {
        self.form.size = size;
        self
    }

    /// The plan's custom property definitions (`Project.props.defs`): the
    /// ones of this kind of schedule are listed as columns, hidden unless
    /// flagged "show in schedule", and can be shown, moved and sorted by like
    /// any other.
    pub fn with_props(mut self, defs: Vec<PropDef>) -> Self {
        self.form.prop_defs = defs;
        self.form.sync_prop_columns();
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn floor(&self) -> usize {
        self.form.floor
    }

    pub fn id(&self) -> Id {
        self.form.def.id
    }

    pub fn draft(&self) -> &Schedule {
        &self.form.def
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Schedule {
        &mut self.form.def
    }

    #[cfg(test)]
    pub fn raise_open_window(&mut self) {
        self.form.actions.open_window = true;
    }

    /// The export / open-window requests since the last call.
    pub fn take_actions(&mut self) -> SpecActions {
        std::mem::take(&mut self.form.actions)
    }
}

fn combo<T: PartialEq + Copy>(ui: &mut Ui, salt: &str, value: &mut T, options: &[(T, &str)]) {
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or("", |(_, n)| *n);
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (v, name) in options {
                ui.selectable_value(value, *v, *name);
            }
        });
}

impl Form {
    /// Lists a column for every custom property of the schedule's kind that
    /// the columns lack, and drops the columns of properties that are gone.
    pub(self) fn sync_prop_columns(&mut self) {
        let kind = plan_docs::schedule_kinds::prop_kind_of(self.def.kind);
        let wanted: Vec<&PropDef> = self
            .prop_defs
            .iter()
            .filter(|d| Some(d.kind) == kind)
            .collect();
        self.def.columns.retain(|c| {
            !c.field.starts_with(plan_core::props::COLUMN_PREFIX)
                || wanted.iter().any(|d| d.column_id() == c.field)
        });
        for d in wanted {
            if !self.def.columns.iter().any(|c| c.field == d.column_id()) {
                self.def
                    .columns
                    .push(ColumnSpec::new(&d.column_id(), &d.name, d.show_in_schedule));
            }
        }
    }

    /// Shows every field of the schedule's kind, keeping order and headings.
    #[cfg(test)]
    fn show_all_columns(&mut self) {
        for c in &mut self.def.columns {
            c.visible = true;
        }
    }

    /// The kind's default columns, headings and order; the sort and the
    /// grouping go back too when their field is not shown any more.
    #[cfg(test)]
    fn reset_columns(&mut self) {
        self.def.columns = self.def.kind.default_columns();
        self.def.reconcile_columns();
        self.sync_prop_columns();
        let shown = |f: &str, d: &Schedule| d.columns.iter().any(|c| c.field == f);
        if !shown(&self.def.sort.field, &self.def) {
            self.def.sort.field.clear();
        }
        if !shown(&self.def.group_by, &self.def) {
            self.def.group_by.clear();
        }
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        (!self.def.columns.iter().any(|c| c.visible)).then(|| "Show at least one column".into())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.columns_rows(ui),
            2 => self.number_formatting(ui),
            3 => self.attributes(ui),
            4 => self.line_style(ui),
            5 => self.fill_style(ui),
            6 => self.text_style(ui, 0),
            7 => self.text_style(ui, 1),
            8 => self.text_style(ui, 2),
            _ => self.labels(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        // The table's skeleton: title bar, heading row and a few rows (or
        // the same turned over when rows and columns are swapped).
        let cols: Vec<&str> = self
            .def
            .visible_columns()
            .map(|c| c.title.as_str())
            .collect();
        if cols.is_empty() {
            return;
        }
        let title_h: f32 = if self.def.show_title { 22.0 } else { 0.0 };
        let row_h: f32 = 16.0;
        let rows = if self.def.swap { cols.len().min(6) } else { 5 };
        let body = Rect::from_min_size(
            rect.min,
            egui::vec2(
                rect.width(),
                (title_h + row_h * rows as f32).min(rect.height()),
            ),
        );
        let ink = PV_INK;
        if self.def.border {
            painter.rect_stroke(body, 0.0, Stroke::new(1.5_f32, ink), StrokeKind::Inside);
        }
        if self.def.show_title {
            painter.text(
                Pos2::new(body.center().x, body.min.y + title_h * 0.5),
                Align2::CENTER_CENTER,
                self.def.display_title(),
                egui::FontId::proportional(11.0),
                ink,
            );
            painter.hline(
                body.x_range(),
                body.min.y + title_h,
                Stroke::new(1.0_f32, ink),
            );
        }
        if self.def.swap {
            // Attribute names down the left, objects across.
            let lw = body.width() * 0.34;
            let n_obj = 3;
            let cw = (body.width() - lw) / n_obj as f32;
            for (r, name) in cols.iter().take(rows).enumerate() {
                let y = body.min.y + title_h + row_h * (r as f32 + 0.5);
                painter.text(
                    Pos2::new(body.min.x + 3.0, y),
                    Align2::LEFT_CENTER,
                    name,
                    egui::FontId::proportional(8.0),
                    PV_ACCENT,
                );
                for c in 0..n_obj {
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(body.min.x + lw + cw * c as f32 + 4.0, y - 2.0),
                            egui::vec2((cw - 10.0).max(4.0), 4.0),
                        ),
                        1.0,
                        PV_FAINT,
                    );
                }
            }
            if self.def.grid_lines {
                painter.vline(
                    body.min.x + lw,
                    (body.min.y + title_h)..=body.max.y,
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
            return;
        }
        let cw = body.width() / cols.len() as f32;
        for (i, c) in cols.iter().enumerate() {
            let x = body.min.x + cw * i as f32;
            if i > 0 && self.def.grid_lines {
                painter.vline(
                    x,
                    (body.min.y + title_h)..=body.max.y,
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
            if self.def.show_headings {
                painter.text(
                    Pos2::new(x + 3.0, body.min.y + title_h + row_h * 0.5),
                    Align2::LEFT_CENTER,
                    c,
                    egui::FontId::proportional(8.0),
                    PV_ACCENT,
                );
            }
            for r in 1..4 {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(x + 3.0, body.min.y + title_h + row_h * (r as f32 + 0.4)),
                        egui::vec2((cw - 8.0).max(4.0), 4.0),
                    ),
                    1.0,
                    PV_FAINT,
                );
            }
        }
        if self.def.show_headings {
            painter.hline(
                body.x_range(),
                body.min.y + title_h + row_h,
                Stroke::new(1.0_f32, ink),
            );
        }
    }
}

// ===================================================================
// Schedule Defaults, Create Schedule from Room and the other windows
// ===================================================================

#[derive(Default)]
struct Extras {
    /// The kinds whose Schedule Defaults dialog is asked for.
    defaults_request: Vec<ScheduleKind>,
    defaults: Option<ScheduleSpecDialog>,
    /// A room (index into the active floor's rooms) waiting for the type
    /// of the schedule made from it.
    room_request: Option<usize>,
    room_kind: Option<ScheduleKind>,
}

thread_local! {
    static EXTRAS: RefCell<Extras> = RefCell::new(Extras::default());
}

/// The schedule kind a Default Settings > Schedules leaf (`door`,
/// `room_finish`, ...) stands for.
pub fn kind_from_slug(slug: &str) -> Option<ScheduleKind> {
    Some(match slug {
        "door" => ScheduleKind::Door,
        "window" => ScheduleKind::Window,
        "room" => ScheduleKind::Room,
        "wall" => ScheduleKind::Wall,
        "cabinet" => ScheduleKind::Cabinet,
        "electrical" => ScheduleKind::Electrical,
        "framing" => ScheduleKind::Framing,
        "fixture" => ScheduleKind::Fixture,
        "furniture" => ScheduleKind::Furniture,
        "plant" => ScheduleKind::Plant,
        "stair" => ScheduleKind::Stair,
        "room_finish" => ScheduleKind::RoomFinish,
        "note" => ScheduleKind::Note,
        "general" => ScheduleKind::General,
        _ => return None,
    })
}

/// Asks for the Schedule Defaults dialog of `kind` (double-clicking the
/// Schedule Tools button, or Default Settings > Schedules > the kind).
pub fn request_defaults(kind: ScheduleKind) {
    EXTRAS.with(|e| e.borrow_mut().defaults_request.push(kind));
}

/// Is a Schedule Defaults dialog up?
pub fn defaults_open() -> bool {
    EXTRAS.with(|e| e.borrow().defaults.is_some())
}

/// Create Schedule from Room: asks which type of schedule to make from room
/// `room` (an index into the rooms of the active floor).
pub fn ask_schedule_type(room: usize) {
    EXTRAS.with(|e| {
        let mut e = e.borrow_mut();
        e.room_request = Some(room);
        e.room_kind = Some(ScheduleKind::Door);
    });
}

/// Is the schedule type chooser up?
pub fn type_chooser_open() -> bool {
    EXTRAS.with(|e| e.borrow().room_request.is_some())
}

fn names_for(cx: &EditorContext) -> (Vec<String>, Vec<String>) {
    let styles = cx
        .project
        .text_styles
        .names()
        .into_iter()
        .map(String::from)
        .collect();
    let layers = cx
        .project
        .layers
        .layers
        .iter()
        .map(|l| l.name.clone())
        .collect();
    (styles, layers)
}

/// Builds the Schedule Defaults dialog of `kind` from the plan's setup.
pub fn defaults_dialog(cx: &EditorContext, kind: ScheduleKind) -> ScheduleSpecDialog {
    let def = cx
        .project
        .schedule_setup
        .template(kind, plan_core::geometry::Point::ZERO);
    let (styles, layers) = names_for(cx);
    ScheduleSpecDialog::for_defaults(def, styles, layers)
        .with_context(SpecContext::from_cx(cx))
        .with_props(cx.project.props.defs.clone())
}

/// Draws the windows of the schedule commands: Schedule Defaults, the type
/// chooser of Create Schedule from Room, Select Location, Manage Custom
/// Schedule Categories and the Automatic Sorting prompt, and centres the view
/// where Find in Plan asked. Called once a frame by the plan windows.
pub fn show_extras(ctx: &egui::Context, cx: &mut EditorContext, cam: &mut Camera) {
    if let Some(p) = schedule_view::take_focus() {
        cam.center = p;
    }
    super::select_location::show(ctx, cx);
    super::schedule_categories::show(ctx, cx);
    show_sort_prompt(ctx, cx);

    let mut x = EXTRAS.with(|e| std::mem::take(&mut *e.borrow_mut()));
    // Schedule Defaults.
    if x.defaults.is_none() {
        if let Some(kind) = x.defaults_request.pop() {
            x.defaults = Some(defaults_dialog(cx, kind));
        }
    }
    x.defaults_request.clear();
    if let Some(mut d) = x.defaults.take() {
        match d.show(ctx) {
            Outcome::Open => x.defaults = Some(d),
            Outcome::Cancel => {}
            Outcome::Ok => {
                cx.begin_change("Schedule Defaults");
                let mut def = d.draft().clone();
                sync_new_categories(&mut cx.project, &mut def, &d.form.new_categories);
                cx.project.schedule_setup.set_default(def);
                cx.mark_dirty();
            }
        }
    }
    // The type of the schedule made from a room.
    if let Some(room) = x.room_request {
        let mut kind = x.room_kind.unwrap_or(ScheduleKind::Door);
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Create Schedule from Room")
            .id(egui::Id::new("schedule_from_room"))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    "Select a schedule type, then click OK and click in the plan to place it.",
                );
                for k in crate::tools::schedule::FLYOUT_KINDS {
                    if k == ScheduleKind::Room
                        || k == ScheduleKind::RoomFinish
                        || k == ScheduleKind::General
                    {
                        continue;
                    }
                    ui.radio_value(&mut kind, k, k.title());
                }
                ui.radio_value(
                    &mut kind,
                    ScheduleKind::RoomFinish,
                    ScheduleKind::RoomFinish.title(),
                );
                ui.separator();
                ui.horizontal(|ui| {
                    accept = ui.button("OK").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        x.room_kind = Some(kind);
        if accept {
            if let Some(r) = cx.rooms.get(room) {
                let name = r
                    .name_entry(&cx.floor().room_names)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                crate::tools::schedule::arm_from_room(RoomRef::at(cx.floor, r.centroid), name);
                cx.requests.push(EditorRequest::SetTool(
                    crate::tools::ToolId::ScheduleVariant(kind),
                ));
            }
            x.room_request = None;
        } else if cancel {
            x.room_request = None;
        }
    }
    EXTRAS.with(|e| {
        let mut slot = e.borrow_mut();
        // Requests made while this frame ran stay.
        let late = std::mem::take(&mut slot.defaults_request);
        *slot = x;
        slot.defaults_request.extend(late);
    });
}

/// Puts the custom categories a draft ticks into the plan's setup when the
/// plan does not know them yet (the dialog's New Custom Category button).
pub fn sync_new_categories(project: &mut plan_core::Project, def: &mut Schedule, made: &[String]) {
    for name in made {
        if project.schedule_setup.category(name).is_none() {
            let _ = project.schedule_setup.add_category(name);
        }
    }
    // A tick of a category the plan lost (deleted meanwhile) is dropped.
    def.categories.retain(|k, _| {
        k.strip_prefix("Custom/")
            .is_none_or(|n| project.schedule_setup.category(n).is_some())
    });
}

fn show_sort_prompt(ctx: &egui::Context, cx: &mut EditorContext) {
    if schedule_view::pending_sort_prompt().is_none() {
        return;
    }
    let mut answer: Option<bool> = None;
    egui::Window::new("Automatic Sorting")
        .id(egui::Id::new("schedule_sort_prompt"))
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label("This schedule is sorted automatically. Turn Automatic Sorting off to move a row by hand?");
            ui.horizontal(|ui| {
                if ui.button("Turn Off Sorting").clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
    if let Some(a) = answer {
        schedule_view::answer_sort_prompt(cx, a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn dialog() -> ScheduleSpecDialog {
        let mut def = Schedule::new(ScheduleKind::Door, Point::ZERO);
        def.id = 7;
        ScheduleSpecDialog::new(
            0,
            def,
            vec!["Schedule Style".into(), "Default Text Style".into()],
            vec!["Schedules".into(), "Text".into()],
        )
    }

    #[test]
    fn toggling_a_column_hides_it_in_the_draft() {
        let mut d = dialog();
        assert!(d.draft().visible_columns().any(|c| c.field == "swing"));
        let i = d
            .draft()
            .columns
            .iter()
            .position(|c| c.field == "swing")
            .unwrap();
        d.draft_mut().set_column_visible(i, false);
        assert!(!d.draft().visible_columns().any(|c| c.field == "swing"));
        assert_eq!(d.id(), 7);
        assert_eq!(d.floor(), 0);
    }

    #[test]
    fn show_all_and_reset_columns_rebuild_the_column_list() {
        let mut d = dialog();
        let fields = d.draft().kind.fields().len();
        let defaults = d.draft().visible_columns().count();
        assert!(defaults < fields, "some fields start hidden");
        d.form.show_all_columns();
        assert_eq!(d.draft().visible_columns().count(), fields);
        // Rename and reorder, sort by a field, then reset.
        d.draft_mut().columns[0].title = "Door No.".into();
        d.draft_mut().move_column(0, false);
        let field = d.draft().columns[1].field.clone();
        d.draft_mut().sort.field = field;
        d.form.reset_columns();
        assert_eq!(d.draft().visible_columns().count(), defaults);
        assert_eq!(d.draft().columns.len(), fields);
        assert!(d.draft().columns.iter().all(|c| c.title != "Door No."));
        assert_eq!(
            d.draft().columns[0].field,
            d.draft().kind.default_columns()[0].field
        );
        // A sort on a field that is still there stays.
        let first = d.draft().columns[0].field.clone();
        d.draft_mut().sort.field = first.clone();
        d.form.reset_columns();
        assert_eq!(d.draft().sort.field, first);
    }

    #[test]
    fn hiding_every_column_blocks_ok() {
        let mut d = dialog();
        for c in &mut d.draft_mut().columns {
            c.visible = false;
        }
        assert!(d.form.error().is_some());
        d.draft_mut().columns[0].visible = true;
        assert!(d.form.error().is_none());
    }

    #[test]
    fn every_page_and_the_preview_draw() {
        let mut d = dialog();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
        for swap in [false, true] {
            d.draft_mut().swap = swap;
            for tab in 0..TABS.len() {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        d.form.page(ui, tab);
                        let (_, painter) =
                            ui.allocate_painter(egui::vec2(220.0, 300.0), egui::Sense::hover());
                        d.form.preview(&painter, painter.clip_rect());
                    });
                });
            }
        }
        assert_eq!(d.take_actions(), SpecActions::default());
    }

    #[test]
    fn custom_property_columns_are_listed_for_the_kind_and_follow_it() {
        use plan_core::props::{PropKind, PropType};
        let mut flagged = PropDef::new(PropKind::Door, "Fire Rating", PropType::Text);
        flagged.show_in_schedule = true;
        let hidden = PropDef::new(PropKind::Door, "Notes", PropType::Text);
        let window = PropDef::new(PropKind::Window, "Glazing", PropType::Text);
        let mut d = dialog().with_props(vec![flagged, hidden, window]);
        let col = |d: &ScheduleSpecDialog, f: &str| {
            d.draft().columns.iter().find(|c| c.field == f).cloned()
        };
        assert!(col(&d, "prop:Fire Rating").unwrap().visible);
        assert!(!col(&d, "prop:Notes").unwrap().visible);
        assert!(col(&d, "prop:Glazing").is_none(), "a window property");
        assert!(d
            .draft()
            .visible_columns()
            .any(|c| c.field == "prop:Fire Rating"));
        // Changing the kind swaps the property columns for the new kind's.
        d.draft_mut().set_kind(ScheduleKind::Window);
        d.form.sync_prop_columns();
        assert!(col(&d, "prop:Fire Rating").is_none());
        assert!(col(&d, "prop:Glazing").is_some());
        // Reset Columns keeps the kind's property columns, hidden unless flagged.
        d.form.reset_columns();
        assert!(col(&d, "prop:Glazing").is_some());
        // The new buttons raise their requests once.
        d.form.actions.export_for_editing = true;
        d.form.actions.import_props = true;
        let a = d.take_actions();
        assert!(a.export_for_editing && a.import_props);
        assert_eq!(d.take_actions(), SpecActions::default());
    }

    #[test]
    fn actions_are_raised_once() {
        let mut d = dialog();
        d.form.actions.export_csv = true;
        d.form.actions.export_xlsx = true;
        let a = d.take_actions();
        assert!(a.export_csv && a.export_xlsx);
        assert!(!d.take_actions().export_csv);
    }

    #[test]
    fn the_column_lists_add_move_rename_and_remove() {
        let mut d = dialog();
        let f = &mut d.form;
        assert!(
            !f.def
                .columns
                .iter()
                .find(|c| c.field == "area")
                .unwrap()
                .visible
        );
        f.include_column("area");
        let last_visible = f.def.columns.iter().rposition(|c| c.visible).unwrap();
        assert_eq!(
            f.def.columns[last_visible].field, "area",
            "added at the end"
        );
        f.move_included("area", true);
        let vis: Vec<&str> = f.def.visible_columns().map(|c| c.field.as_str()).collect();
        let n = vis.len();
        assert_eq!(vis[n - 2], "area");
        f.exclude_column("area");
        assert!(
            !f.def
                .columns
                .iter()
                .find(|c| c.field == "area")
                .unwrap()
                .visible
        );
        // The last column cannot be removed.
        for c in f.def.columns.clone() {
            f.exclude_column(&c.field);
        }
        assert_eq!(f.def.visible_columns().count(), 1);
        // Reset restores the default headings.
        f.def.columns[0].title = "X".into();
        f.reset_titles();
        assert_eq!(f.def.columns[0].title, "Mark");
    }

    #[test]
    fn the_category_tree_toggles_and_new_categories_are_kept() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.0,
            plan_core::WallKind::Exterior,
        );
        cx.project
            .add_opening(0, w, 60.0, plan_core::OpeningKind::Door)
            .unwrap();
        cx.mark_dirty();
        cx.refresh();
        let mut def = Schedule::new(ScheduleKind::Door, Point::ZERO);
        def.id = 1;
        let mut d =
            ScheduleSpecDialog::new(0, def, vec![], vec![]).with_context(SpecContext::from_cx(&cx));
        let tree = d.form.tree();
        let door = tree.iter().find(|g| g.id == "Door").unwrap();
        assert!(door.items.iter().any(|n| n.id == "Door/Hinged Door"));
        d.form.materialize(&tree);
        d.form.def.set_category("Door/Hinged Door", false);
        let t = schedule_view::table_for(&cx, d.draft(), 0);
        assert_eq!(t.rows.len(), 0, "the hinged door is unticked");
        // A new custom category made here joins the plan on OK.
        d.form.new_categories.push("Glazing".into());
        let mut def = d.draft().clone();
        def.set_category("Custom/Glazing", true);
        sync_new_categories(&mut cx.project, &mut def, &d.form.new_categories);
        assert!(cx.project.schedule_setup.category("Glazing").is_some());
        assert!(def.categories.contains_key("Custom/Glazing"));
        // A tick of a category that is gone is dropped.
        def.set_category("Custom/Gone", true);
        sync_new_categories(&mut cx.project, &mut def, &[]);
        assert!(!def.categories.contains_key("Custom/Gone"));
    }

    #[test]
    fn schedule_defaults_start_new_schedules_of_that_kind() {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let d = defaults_dialog(&cx, ScheduleKind::Window);
        assert_eq!(d.draft().kind, ScheduleKind::Window);
        assert_eq!(d.form.mode, Mode::Defaults);
        let mut def = d.draft().clone();
        def.border = false;
        def.show_title = false;
        def.label_prefix = "WIN-".into();
        cx.begin_change("Schedule Defaults");
        cx.project.schedule_setup.set_default(def);
        cx.mark_dirty();
        let id = schedule_view::add(&mut cx, ScheduleKind::Window, Point::ZERO);
        let s = schedule_view::find(&cx, id).unwrap();
        assert!(!s.border && !s.show_title);
        assert_eq!(s.label_prefix, "WIN-");
        // Another kind is untouched.
        let id2 = schedule_view::add(&mut cx, ScheduleKind::Door, Point::new(0.0, -100.0));
        assert!(schedule_view::find(&cx, id2).unwrap().border);
        // The dialog draws in both modes.
        let ctx = egui::Context::default();
        let mut dd = defaults_dialog(&cx, ScheduleKind::Door);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = dd.show(ctx);
        });
    }
}
