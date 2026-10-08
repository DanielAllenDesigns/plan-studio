//! Tools > Property Manager, the Properties tab of the specification
//! dialogs, and the Excel exchange commands.
//!
//! # Property Manager
//!
//! A window listing the plan's custom properties by kind of object (door,
//! window, cabinet, room, wall, fixture / symbol, electrical, stair, roof
//! plane, framing) with a form to add, change and delete one: name, type
//! (text, number, length, yes / no, list), default and "show in schedule".
//! Every add, change and delete is one undo step; the definitions live in
//! `Project.props` (`plan_core::props`).
//!
//! # Properties tab
//!
//! Once a kind of object has a property, its specification dialog gets a
//! last tab, Properties, with one field per property. The tab is added by the
//! shared dialog frame (`SpecDialog::frame` asks [`current`]), so no dialog
//! carries its own page. The host opens a [`PropSession`] for the object when
//! it opens the dialog and wraps the dialog's `show` in [`with_current`]; OK
//! applies the dialog and the properties as one undo step
//! ([`before_apply`] / [`after_apply`]).
//!
//! # Excel exchange
//!
//! Export for Editing writes the schedules as a workbook (see
//! `plan_docs::props_exchange`), Import Property Data reads one back, shows
//! the review dialog ([`super::import_review`]) and applies the checked
//! changes as one undo step. After an export the plan watches the file: every
//! two seconds, on the UI thread, it looks at the file's modification time,
//! and when the workbook is newer than the last import it offers "Workbook
//! changed - import?" over the status bar.

use super::import_review::ImportReview;
use super::{row, section, Outcome, ERROR_RED};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, RichText};
use plan_core::props::{PropDef, PropKey, PropKind, PropType};
use plan_core::schedules::{FloorScope, Schedule, ScheduleKind, ScheduleLayer};
use plan_core::{OpeningKind, Point};
use plan_docs::props_exchange::{self, ExportOptions, ExportSchedule};
use plan_docs::xlsx_read::{read_csv, read_xlsx, ReadSheet};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

/// Tools > Property Manager...
pub const OPEN: &str = "props.manager";
/// Tools > Export Property Data (XLSX)...: every placed schedule.
pub const EXPORT_ALL: &str = "props.export_all";
/// The context menu of a selected schedule: export just that one.
pub const EXPORT_SELECTED: &str = "props.export_selected";
/// Tools > Import Property Data (XLSX)...
pub const IMPORT: &str = "props.import";

/// How often the exported workbook's modification time is looked at.
const POLL: Duration = Duration::from_secs(2);

/// Is `id` one of this module's menu commands?
pub fn is_command(id: &str) -> bool {
    matches!(id, OPEN | EXPORT_ALL | EXPORT_SELECTED | IMPORT)
}

// ===================================================================
// Objects and their keys
// ===================================================================

/// The key and kind under which the object `o` of the active floor keeps its
/// custom properties; `None` for the objects that take none.
pub fn object_key(cx: &EditorContext, o: ObjectRef) -> Option<(PropKey, PropKind)> {
    let f = cx.floor();
    match o {
        ObjectRef::Wall(id) => f.wall(id).map(|_| (PropKey::wall(id), PropKind::Wall)),
        ObjectRef::Opening(id) => f.openings.iter().find(|x| x.id == id).map(|x| match x.kind {
            OpeningKind::Door => (PropKey::door(id), PropKind::Door),
            OpeningKind::Window => (PropKey::window(id), PropKind::Window),
        }),
        ObjectRef::Cabinet(id) => Some((PropKey::cabinet(id), PropKind::Cabinet)),
        ObjectRef::Symbol(id) => f.symbol(id).map(|_| (PropKey::symbol(id), PropKind::Symbol)),
        ObjectRef::Stair(id) => Some((PropKey::stair(id), PropKind::Stair)),
        ObjectRef::RoofPlane(id) => Some((PropKey::roof(id), PropKind::RoofPlane)),
        ObjectRef::Framing(id) => Some((PropKey::framing(id), PropKind::Framing)),
        ObjectRef::Device(id) => Some((PropKey::device(cx.floor, id), PropKind::Electrical)),
        ObjectRef::Room(i) => cx
            .rooms
            .get(i)
            .map(|r| (PropKey::room(cx.floor, r.centroid), PropKind::Room)),
        _ => None,
    }
}

// ===================================================================
// Property Manager: editing the definitions
// ===================================================================

/// Adds a definition as one undo step.
pub fn add_property(cx: &mut EditorContext, def: PropDef) -> Result<(), String> {
    cx.begin_change("Add Property");
    match cx.project.props.add_def(def) {
        Ok(()) => {
            cx.mark_dirty();
            Ok(())
        }
        Err(e) => {
            cx.cancel_change();
            Err(e)
        }
    }
}

/// Replaces a definition (rename, retype, new default) as one undo step.
/// Returns how many stored values no longer fit the new type and were
/// dropped.
pub fn update_property(
    cx: &mut EditorContext,
    kind: PropKind,
    old_name: &str,
    def: PropDef,
) -> Result<usize, String> {
    cx.begin_change("Change Property");
    match cx.project.props.replace_def(kind, old_name, def) {
        Ok(n) => {
            cx.mark_dirty();
            Ok(n)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e)
        }
    }
}

/// Deletes a definition and its values as one undo step.
pub fn delete_property(cx: &mut EditorContext, kind: PropKind, name: &str) -> bool {
    cx.begin_change("Delete Property");
    if cx.project.props.remove_def(kind, name) {
        cx.mark_dirty();
        true
    } else {
        cx.cancel_change();
        false
    }
}

/// The form of the Property Manager.
#[derive(Clone, Debug, PartialEq)]
struct Draft {
    kind: PropKind,
    name: String,
    ty: PropType,
    default: String,
    /// One choice per line.
    options: String,
    show: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            kind: PropKind::Door,
            name: String::new(),
            ty: PropType::Text,
            default: String::new(),
            options: String::new(),
            show: false,
        }
    }
}

impl Draft {
    fn of(d: &PropDef) -> Self {
        Self {
            kind: d.kind,
            name: d.name.clone(),
            ty: d.ty,
            default: d.default.clone(),
            options: d.options.join("\n"),
            show: d.show_in_schedule,
        }
    }

    fn to_def(&self) -> PropDef {
        PropDef {
            kind: self.kind,
            name: self.name.trim().to_string(),
            ty: self.ty,
            default: self.default.trim().to_string(),
            options: self
                .options
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            show_in_schedule: self.show,
        }
    }

    /// Why the draft is not a usable definition, if it is not.
    fn problem(&self) -> Option<String> {
        let d = self.to_def();
        if d.name.is_empty() {
            return Some("Give the property a name".into());
        }
        if d.ty == PropType::List && d.options.is_empty() {
            return Some("A list needs at least one choice (one per line)".into());
        }
        if !d.default.is_empty() {
            if let Err(e) = d.parse(&d.default) {
                return Some(format!("Default: {e}"));
            }
        }
        None
    }
}

/// The Property Manager window's state.
struct Manager {
    draft: Draft,
    /// The definition being changed (kind, name); `None` for a new one.
    editing: Option<(PropKind, String)>,
    message: Option<(bool, String)>,
}

impl Manager {
    fn new() -> Self {
        Self {
            draft: Draft::default(),
            editing: None,
            message: None,
        }
    }

    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        egui::Window::new("Property Manager")
            .id(egui::Id::new("property_manager"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([720.0, 440.0])
            .show(ctx, |ui| self.body(ui, cx));
        open
    }

    fn body(&mut self, ui: &mut egui::Ui, cx: &mut EditorContext) {
        ui.columns(2, |cols| {
            self.list(&mut cols[0], cx);
            self.form(&mut cols[1], cx);
        });
    }

    fn list(&mut self, ui: &mut egui::Ui, cx: &EditorContext) {
        ui.strong("Properties");
        ui.weak("Define your own properties for each kind of object. They show in the object's Properties tab, in schedules, and in the Excel exchange.");
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("property_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for kind in PropKind::ALL {
                    let defs: Vec<&PropDef> = cx.project.props.defs_for(kind).collect();
                    egui::CollapsingHeader::new(format!("{} ({})", kind.name(), defs.len()))
                        .id_salt(("prop_kind", kind))
                        .default_open(!defs.is_empty())
                        .show(ui, |ui| {
                            if defs.is_empty() {
                                ui.weak("none yet");
                            }
                            for d in defs {
                                let selected = self.editing.as_ref().is_some_and(|(k, n)| {
                                    *k == kind && n.eq_ignore_ascii_case(&d.name)
                                });
                                let label = format!(
                                    "{}  ({}){}",
                                    d.name,
                                    d.ty.name(),
                                    if d.show_in_schedule { "  \u{25A6}" } else { "" }
                                );
                                if ui.selectable_label(selected, label).clicked() {
                                    self.draft = Draft::of(d);
                                    self.editing = Some((kind, d.name.clone()));
                                    self.message = None;
                                }
                            }
                        });
                }
            });
    }

    fn form(&mut self, ui: &mut egui::Ui, cx: &mut EditorContext) {
        section(
            ui,
            if self.editing.is_some() {
                "Change Property"
            } else {
                "New Property"
            },
        );
        row(ui, "Object kind", |ui| {
            egui::ComboBox::from_id_salt("prop_kind_pick")
                .selected_text(self.draft.kind.name())
                .show_ui(ui, |ui| {
                    for k in PropKind::ALL {
                        ui.selectable_value(&mut self.draft.kind, k, k.name());
                    }
                });
        });
        row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(180.0));
        });
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("prop_type_pick")
                .selected_text(self.draft.ty.name())
                .show_ui(ui, |ui| {
                    for t in PropType::ALL {
                        ui.selectable_value(&mut self.draft.ty, t, t.name());
                    }
                });
        });
        if self.draft.ty == PropType::List {
            row(ui, "Choices", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.draft.options)
                        .desired_rows(4)
                        .desired_width(180.0)
                        .hint_text("one per line"),
                );
            });
        }
        row(ui, "Default", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.draft.default)
                    .desired_width(180.0)
                    .hint_text(match self.draft.ty {
                        PropType::Bool => "Yes or No",
                        PropType::Length => "3'-6\"",
                        _ => "",
                    }),
            );
        });
        ui.checkbox(&mut self.draft.show, "Show in schedules of this kind");
        ui.add_space(6.0);
        let problem = self.draft.problem();
        if let Some(p) = &problem {
            ui.colored_label(ERROR_RED, p);
        }
        ui.horizontal(|ui| {
            let ok = problem.is_none();
            match self.editing.clone() {
                Some((kind, old)) => {
                    if ui.add_enabled(ok, egui::Button::new("Update")).clicked() {
                        self.message = Some(
                            match update_property(cx, kind, &old, self.draft.to_def()) {
                                Ok(0) => {
                                    self.editing = Some((self.draft.kind, self.draft.name.trim().to_string()));
                                    (true, "Property updated".into())
                                }
                                Ok(n) => {
                                    self.editing = Some((self.draft.kind, self.draft.name.trim().to_string()));
                                    (true, format!("Property updated; {n} value{} no longer fit and were cleared", if n == 1 { "" } else { "s" }))
                                }
                                Err(e) => (false, e),
                            },
                        );
                    }
                    if ui.button("Delete").clicked() {
                        delete_property(cx, kind, &old);
                        self.editing = None;
                        self.draft = Draft::default();
                        self.message = Some((true, "Property deleted".into()));
                    }
                    if ui.button("New").clicked() {
                        self.editing = None;
                        self.draft = Draft::default();
                        self.message = None;
                    }
                }
                None => {
                    if ui.add_enabled(ok, egui::Button::new("Add Property")).clicked() {
                        self.message = Some(match add_property(cx, self.draft.to_def()) {
                            Ok(()) => {
                                self.editing = Some((self.draft.kind, self.draft.name.trim().to_string()));
                                (true, "Property added".into())
                            }
                            Err(e) => (false, e),
                        });
                    }
                }
            }
        });
        if let Some((good, m)) = &self.message {
            if *good {
                ui.weak(m);
            } else {
                ui.colored_label(ERROR_RED, m);
            }
        }
    }
}

// ===================================================================
// The Properties tab
// ===================================================================

/// The custom properties of one object, being edited in its dialog.
#[derive(Debug, Clone)]
pub struct PropSession {
    pub key: PropKey,
    pub kind: PropKind,
    defs: Vec<PropDef>,
    texts: Vec<String>,
    original: Vec<String>,
}

/// A session shared between the host that applies it and the dialog frame
/// that draws it.
pub type SharedSession = Rc<RefCell<PropSession>>;

impl PropSession {
    /// A session for the object under `key`, or `None` when its kind has no
    /// properties (then the dialog gets no Properties tab).
    pub fn new(project: &plan_core::Project, key: PropKey, kind: PropKind) -> Option<Self> {
        let defs: Vec<PropDef> = project.props.defs_for(kind).cloned().collect();
        if defs.is_empty() {
            return None;
        }
        let texts: Vec<String> = defs.iter().map(|d| project.props.text(&key, d)).collect();
        Some(Self {
            key,
            kind,
            original: texts.clone(),
            texts,
            defs,
        })
    }

    /// A shared session for `o`, when it takes properties and its kind has some.
    pub fn for_object(cx: &EditorContext, o: ObjectRef) -> Option<SharedSession> {
        let (key, kind) = object_key(cx, o)?;
        Self::new(&cx.project, key, kind).map(|s| Rc::new(RefCell::new(s)))
    }

    /// Has any field been changed?
    pub fn changed(&self) -> bool {
        self.texts != self.original
    }

    /// Edits field `i` (for tests and keyboard shortcuts).
    pub fn set_text(&mut self, name: &str, text: &str) -> bool {
        match self.defs.iter().position(|d| d.name.eq_ignore_ascii_case(name)) {
            Some(i) => {
                self.texts[i] = text.to_string();
                true
            }
            None => false,
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.defs
            .iter()
            .position(|d| d.name.eq_ignore_ascii_case(name))
            .map(|i| self.texts[i].as_str())
    }

    /// The first field whose text its type refuses (blocks OK).
    pub fn error(&self) -> Option<String> {
        self.defs
            .iter()
            .zip(&self.texts)
            .find_map(|(d, t)| d.parse(t).err().map(|e| format!("{}: {e}", d.name)))
    }

    /// Writes the changed fields into the project. Returns how many changed.
    pub fn apply(&self, project: &mut plan_core::Project) -> usize {
        let mut n = 0;
        for ((d, t), o) in self.defs.iter().zip(&self.texts).zip(&self.original) {
            if t == o {
                continue;
            }
            // The definition may have been changed since the dialog opened.
            let Some(def) = project.props.def(self.kind, &d.name).cloned() else {
                continue;
            };
            if matches!(project.props.set_text(&self.key, &def, t), Ok(true)) {
                n += 1;
            }
        }
        n
    }
}

thread_local! {
    static CURRENT: RefCell<Option<SharedSession>> = const { RefCell::new(None) };
}

/// The session of the dialog being drawn, if its host armed one.
pub fn current() -> Option<SharedSession> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Runs `f` (a dialog's `show`) with `session` as the Properties tab of
/// every `SpecDialog` drawn inside it.
pub fn with_current<R>(session: Option<&SharedSession>, f: impl FnOnce() -> R) -> R {
    let prev = CURRENT.with(|c| c.replace(session.cloned()));
    let r = f();
    CURRENT.with(|c| *c.borrow_mut() = prev);
    r
}

/// Draws the Properties tab.
pub fn page(ui: &mut egui::Ui, s: &mut PropSession) {
    section(ui, "Properties");
    let defs = s.defs.clone();
    for (i, d) in defs.iter().enumerate() {
        row(ui, &d.name, |ui| match d.ty {
            PropType::List => {
                let shown = if s.texts[i].is_empty() {
                    "(none)".to_string()
                } else {
                    s.texts[i].clone()
                };
                egui::ComboBox::from_id_salt(("prop_page", i))
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.texts[i], String::new(), "(none)");
                        for o in &d.options {
                            ui.selectable_value(&mut s.texts[i], o.clone(), o);
                        }
                    });
            }
            PropType::Bool => {
                egui::ComboBox::from_id_salt(("prop_page", i))
                    .selected_text(if s.texts[i].is_empty() { "(none)" } else { &s.texts[i] })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.texts[i], String::new(), "(none)");
                        ui.selectable_value(&mut s.texts[i], "Yes".to_string(), "Yes");
                        ui.selectable_value(&mut s.texts[i], "No".to_string(), "No");
                    });
            }
            _ => {
                let bad = d.parse(&s.texts[i]).is_err();
                let mut edit = egui::TextEdit::singleline(&mut s.texts[i]).desired_width(200.0);
                if bad {
                    edit = edit.text_color(ERROR_RED);
                }
                ui.add(edit);
                if d.ty == PropType::Length {
                    ui.weak("e.g. 3'-6\"");
                }
            }
        });
    }
    if let Some(e) = s.error() {
        ui.colored_label(ERROR_RED, e);
    }
    ui.add_space(6.0);
    ui.weak("Define more in Tools > Property Manager.");
}

/// Before a dialog's own apply: the undo depth to compare against.
pub fn before_apply(cx: &EditorContext) -> usize {
    cx.undo_depth()
}

/// After a dialog's own apply: writes the Properties tab. When the dialog
/// made an undo step the property edit joins it (the step restores the plan
/// from before both); otherwise it is a step of its own, "Object Properties".
pub fn after_apply(cx: &mut EditorContext, session: Option<&SharedSession>, depth_before: usize) {
    let Some(s) = session else { return };
    if !s.borrow().changed() || s.borrow().error().is_some() {
        return;
    }
    // The dialog made no step: the property edit is a step of its own.
    let own_step = cx.undo_depth() == depth_before;
    if own_step {
        cx.begin_change("Object Properties");
    }
    let n = s.borrow().apply(&mut cx.project);
    if own_step && n == 0 {
        cx.cancel_change();
    }
    cx.mark_dirty();
}

// ===================================================================
// Export and import
// ===================================================================

/// `YYYY-MM-DD HH:MM UTC` for the `_meta` sheet.
pub fn utc_stamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// The schedules an export covers: every schedule placed in the plan, else
/// the standard set (door, window, room, wall, cabinet, electrical, fixture,
/// stair), each over the whole plan.
pub fn export_set(cx: &EditorContext) -> Vec<(usize, Schedule)> {
    let mut out = Vec::new();
    for (fi, f) in cx.project.floors.iter().enumerate() {
        for s in ScheduleLayer::load(f).schedules {
            if s.kind != ScheduleKind::Note && s.kind != ScheduleKind::General {
                out.push((fi, s));
            }
        }
    }
    if out.is_empty() {
        for kind in [
            ScheduleKind::Door,
            ScheduleKind::Window,
            ScheduleKind::Room,
            ScheduleKind::Wall,
            ScheduleKind::Cabinet,
            ScheduleKind::Electrical,
            ScheduleKind::Fixture,
            ScheduleKind::Stair,
        ] {
            let mut s = Schedule::new(kind, Point::ZERO);
            s.floor_scope = FloorScope::All;
            out.push((cx.floor, s));
        }
    }
    out
}

/// The workbook for `set`, with `plan_path` recorded in `_meta`.
pub fn build_workbook(cx: &mut EditorContext, set: &[(usize, Schedule)], plan_path: &str) -> Vec<u8> {
    cx.refresh();
    let list: Vec<ExportSchedule> = set
        .iter()
        .map(|(f, s)| ExportSchedule {
            def: s,
            home_floor: *f,
        })
        .collect();
    let opts = ExportOptions {
        plan_path: plan_path.to_string(),
        exported_at: utc_stamp(SystemTime::now()),
        all_props: true,
    };
    props_exchange::export_workbook(&cx.project, &list, Some((cx.floor, cx.rooms.as_slice())), &opts)
}

/// Reads a workbook or CSV `bytes` into the review dialog.
pub fn begin_import(
    cx: &mut EditorContext,
    bytes: &[u8],
    file_name: &str,
    is_csv: bool,
) -> Result<(), String> {
    let sheets: Vec<ReadSheet> = if is_csv {
        vec![ReadSheet {
            name: file_name.to_string(),
            hidden: false,
            rows: read_csv(&String::from_utf8_lossy(bytes)),
        }]
    } else {
        read_xlsx(bytes)?
    };
    cx.refresh();
    let plan = props_exchange::plan_import(
        &cx.project,
        &sheets,
        Some((cx.floor, cx.rooms.as_slice())),
    )?;
    state(|s| s.review = Some(ImportReview::new(plan, file_name)));
    Ok(())
}

/// What the plan remembers about an exported workbook.
#[derive(Debug, Clone)]
struct Watch {
    path: PathBuf,
    /// The plan file the export was made from (the watch ends when another
    /// plan is open).
    plan: Option<PathBuf>,
    /// The workbook's modification time just after it was written, or at the
    /// last import from it.
    seen: Option<SystemTime>,
    /// A change the user dismissed.
    dismissed: Option<SystemTime>,
    last_poll: Instant,
}

impl Watch {
    /// Does a modification time `now` call for the "Workbook changed" offer?
    fn changed(&self, now: Option<SystemTime>) -> Option<SystemTime> {
        let now = now?;
        let newer = self.seen.is_none_or(|s| now > s);
        (newer && self.dismissed != Some(now)).then_some(now)
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[derive(Default)]
struct State {
    manager: Option<Manager>,
    review: Option<ImportReview>,
    /// The file the review is of, to mark it imported.
    review_path: Option<PathBuf>,
    watch: Option<Watch>,
    /// A newer modification time of the watched workbook.
    offer: Option<SystemTime>,
    /// Commands waiting for the next frame (they open file dialogs).
    pending: Vec<&'static str>,
    /// The schedule to export when the command is Export Selected.
    selected: Option<(usize, plan_core::Id)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Runs a menu command of this module.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        OPEN => state(|s| {
            s.manager.get_or_insert_with(Manager::new);
        }),
        EXPORT_ALL => state(|s| s.pending.push(EXPORT_ALL)),
        EXPORT_SELECTED => {
            let sel = cx.selection.items.iter().find_map(|o| match o {
                ObjectRef::Schedule(i) => Some((cx.floor, *i)),
                _ => None,
            });
            state(|s| {
                s.selected = sel;
                s.pending.push(if sel.is_some() { EXPORT_SELECTED } else { EXPORT_ALL });
            });
        }
        IMPORT => state(|s| s.pending.push(IMPORT)),
        _ => return false,
    }
    true
}

/// Export for Editing of one schedule (the Schedule Specification button).
pub fn request_export_one(floor: usize, def: Schedule) {
    state(|s| {
        s.selected = None;
        s.pending.push(EXPORT_SELECTED);
        EXPORT_ONE.with(|e| *e.borrow_mut() = Some((floor, def)));
    });
}

thread_local! {
    static EXPORT_ONE: RefCell<Option<(usize, Schedule)>> = const { RefCell::new(None) };
}

/// Import Property Data (the Schedule Specification button).
pub fn request_import() {
    state(|s| s.pending.push(IMPORT));
}

fn file_stem(cx: &EditorContext) -> String {
    let n: String = cx
        .project
        .name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    if n.trim_matches('_').is_empty() {
        "Plan".into()
    } else {
        n
    }
}

/// Asks where to save, writes the workbook (or, for a .csv name, the first
/// schedule as CSV) and starts watching the file.
fn export_to_dialog(cx: &mut EditorContext, set: Vec<(usize, Schedule)>, plan: Option<&Path>) {
    let default = if set.len() == 1 {
        format!("{}.xlsx", set[0].1.display_title().replace(' ', "_"))
    } else {
        format!("{} Properties.xlsx", file_stem(cx))
    };
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default)
        .add_filter("Excel workbook", &["xlsx"])
        .add_filter("CSV", &["csv"])
        .save_file()
    else {
        cx.status = "Export cancelled".into();
        return;
    };
    cx.status = export_to_path(cx, &set, &path, plan);
}

/// Writes the export to `path`; returns the status line.
pub fn export_to_path(
    cx: &mut EditorContext,
    set: &[(usize, Schedule)],
    path: &Path,
    plan: Option<&Path>,
) -> String {
    let plan_text = plan.map(|p| p.display().to_string()).unwrap_or_default();
    let is_csv = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    let bytes = if is_csv {
        cx.refresh();
        let (f, s) = &set[0];
        props_exchange::export_csv(
            &cx.project,
            &ExportSchedule {
                def: s,
                home_floor: *f,
            },
            Some((cx.floor, cx.rooms.as_slice())),
            true,
        )
        .into_bytes()
    } else {
        build_workbook(cx, set, &plan_text)
    };
    match std::fs::write(path, &bytes) {
        Ok(()) => {
            state(|s| {
                s.offer = None;
                s.watch = Some(Watch {
                    path: path.to_path_buf(),
                    plan: plan.map(Path::to_path_buf),
                    seen: mtime(path),
                    dismissed: None,
                    last_poll: Instant::now(),
                });
            });
            if is_csv && set.len() > 1 {
                format!("Saved {} (CSV holds the first schedule only)", path.display())
            } else {
                format!(
                    "Saved {}. Edit it in Excel, then Tools > Import Property Data",
                    path.display()
                )
            }
        }
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

/// Reads `path` into the review dialog.
pub fn import_path(cx: &mut EditorContext, path: &Path) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_csv = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    match std::fs::read(path) {
        Ok(bytes) => match begin_import(cx, &bytes, &name, is_csv) {
            Ok(()) => state(|s| s.review_path = Some(path.to_path_buf())),
            Err(e) => cx.status = format!("Could not import {name}: {e}"),
        },
        Err(e) => cx.status = format!("Could not read {}: {e}", path.display()),
    }
}

fn import_from_dialog(cx: &mut EditorContext) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Property data", &["xlsx", "csv"])
        .pick_file()
    else {
        cx.status = "Import cancelled".into();
        return;
    };
    import_path(cx, &path);
}

/// Draws the Property Manager, the import review and the "Workbook changed"
/// offer, and carries out the commands the menus queued. `plan` is the plan
/// file's path (a watch ends when another plan is open).
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext, plan: Option<&Path>) {
    for cmd in state(|s| std::mem::take(&mut s.pending)) {
        match cmd {
            EXPORT_ALL => {
                let set = export_set(cx);
                export_to_dialog(cx, set, plan);
            }
            EXPORT_SELECTED => {
                let one = EXPORT_ONE.with(|e| e.borrow_mut().take()).or_else(|| {
                    let (f, id) = state(|s| s.selected.take())?;
                    let def = ScheduleLayer::load(cx.project.floors.get(f)?)
                        .find(id)
                        .cloned()?;
                    Some((f, def))
                });
                match one {
                    Some(o) => export_to_dialog(cx, vec![o], plan),
                    None => cx.status = "That schedule is no longer in the plan".into(),
                }
            }
            IMPORT => import_from_dialog(cx),
            _ => {}
        }
    }
    if let Some(mut m) = state(|s| s.manager.take()) {
        if m.show(ctx, cx) {
            state(|s| s.manager = Some(m));
        }
    }
    if let Some(mut r) = state(|s| s.review.take()) {
        match r.show(ctx) {
            Outcome::Open => state(|s| s.review = Some(r)),
            Outcome::Cancel => state(|s| s.review_path = None),
            Outcome::Ok => {
                r.apply(cx);
                state(|s| {
                    if let (Some(w), Some(p)) = (s.watch.as_mut(), s.review_path.take()) {
                        if w.path == p {
                            w.seen = mtime(&p);
                            s.offer = None;
                        }
                    }
                });
            }
        }
    }
    watch_frame(ctx, cx, plan);
}

/// The two-second poll and the offer to import the changed workbook.
fn watch_frame(ctx: &egui::Context, cx: &mut EditorContext, plan: Option<&Path>) {
    let mut import: Option<PathBuf> = None;
    state(|s| {
        let Some(w) = s.watch.as_mut() else { return };
        if w.plan.as_deref() != plan {
            s.watch = None;
            s.offer = None;
            return;
        }
        if w.last_poll.elapsed() >= POLL {
            w.last_poll = Instant::now();
            s.offer = w.changed(mtime(&w.path));
        }
        ctx.request_repaint_after(POLL);
    });
    let Some(when) = state(|s| s.offer) else { return };
    let mut dismiss = false;
    egui::Area::new(egui::Id::new("workbook_changed"))
        .anchor(egui::Align2::LEFT_BOTTOM, [8.0, -34.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Workbook changed \u{2014} import?").strong());
                    if ui.button("Import\u{2026}").clicked() {
                        import = state(|s| s.watch.as_ref().map(|w| w.path.clone()));
                    }
                    if ui.button("Dismiss").clicked() {
                        dismiss = true;
                    }
                });
            });
        });
    if dismiss {
        state(|s| {
            if let Some(w) = s.watch.as_mut() {
                w.dismissed = Some(when);
            }
            s.offer = None;
        });
    }
    if let Some(p) = import {
        state(|s| s.offer = None);
        import_path(cx, &p);
    }
}

/// Test access: the open review dialog.
#[cfg(test)]
pub fn take_review() -> Option<ImportReview> {
    state(|s| s.review.take())
}

/// Test access: forget everything this module keeps.
#[cfg(test)]
pub fn reset() {
    state(|s| *s = State::default());
}

/// Test access: is the Property Manager open?
#[cfg(test)]
pub fn manager_open() -> bool {
    state(|s| s.manager.is_some())
}

/// Test access: arm a watch on `path` as if it had just been exported.
#[cfg(test)]
pub fn arm_watch(path: &Path, plan: Option<&Path>, seen: Option<SystemTime>) {
    state(|s| {
        s.watch = Some(Watch {
            path: path.to_path_buf(),
            plan: plan.map(Path::to_path_buf),
            seen,
            dismissed: None,
            // Long ago, so the next frame polls at once.
            last_poll: Instant::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or_else(Instant::now),
        })
    });
}

/// Test access: the modification time the status bar currently offers.
#[cfg(test)]
pub fn offered() -> Option<SystemTime> {
    state(|s| s.offer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn watch(seen: Option<SystemTime>) -> Watch {
        Watch {
            path: PathBuf::from("/tmp/x.xlsx"),
            plan: None,
            seen,
            dismissed: None,
            last_poll: Instant::now(),
        }
    }

    #[test]
    fn the_watch_offers_only_a_workbook_newer_than_the_last_import() {
        let mut w = watch(Some(at(100)));
        assert_eq!(w.changed(Some(at(100))), None, "unchanged since export");
        assert_eq!(w.changed(Some(at(90))), None, "older is not newer");
        assert_eq!(w.changed(None), None, "a missing file offers nothing");
        assert_eq!(w.changed(Some(at(101))), Some(at(101)));
        // Dismissed once: not offered again for that save, but for the next.
        w.dismissed = Some(at(101));
        assert_eq!(w.changed(Some(at(101))), None);
        assert_eq!(w.changed(Some(at(150))), Some(at(150)));
        // An import moves the mark forward.
        w.seen = Some(at(150));
        assert_eq!(w.changed(Some(at(150))), None);
        // No mark at all (the stat failed at export): any file counts.
        assert!(watch(None).changed(Some(at(5))).is_some());
    }

    #[test]
    fn stamps_are_utc_dates() {
        assert_eq!(utc_stamp(at(0)), "1970-01-01 00:00 UTC");
        assert_eq!(utc_stamp(at(1_791_450_000)), "2026-10-08 08:20 UTC");
        assert_eq!(utc_stamp(at(951_782_400 + 3600 * 5 + 60 * 7)), "2000-02-29 05:07 UTC");
    }

    #[test]
    fn definitions_add_change_and_delete_each_in_one_undo_step() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut d = PropDef::new(PropKind::Door, "Fire Rating", PropType::List);
        d.options = vec!["None".into(), "20 min".into()];
        add_property(&mut cx, d.clone()).unwrap();
        assert_eq!(cx.undo_label(), Some("Add Property"));
        assert!(add_property(&mut cx, d.clone()).unwrap_err().contains("already"));
        assert_eq!(cx.undo_label(), Some("Add Property"), "a refused add leaves no step");
        let depth = cx.undo_depth();
        let mut renamed = d.clone();
        renamed.name = "Rating".into();
        assert_eq!(update_property(&mut cx, PropKind::Door, "Fire Rating", renamed).unwrap(), 0);
        assert_eq!(cx.undo_depth(), depth + 1);
        assert!(cx.project.props.def(PropKind::Door, "Rating").is_some());
        assert!(delete_property(&mut cx, PropKind::Door, "Rating"));
        assert!(!delete_property(&mut cx, PropKind::Door, "Rating"));
        assert!(cx.project.props.defs.is_empty());
        cx.undo();
        assert_eq!(cx.project.props.defs.len(), 1);
        cx.undo();
        cx.undo();
        assert!(cx.project.props.defs.is_empty());
    }

    #[test]
    fn the_draft_is_checked_before_it_is_added() {
        let mut d = Draft::default();
        assert!(d.problem().unwrap().contains("name"));
        d.name = "Finish".into();
        d.ty = PropType::List;
        assert!(d.problem().unwrap().contains("choice"));
        d.options = "Matte\n\n Gloss ".into();
        assert!(d.problem().is_none());
        assert_eq!(d.to_def().options, ["Matte", "Gloss"]);
        d.default = "Satin".into();
        assert!(d.problem().unwrap().contains("Default"));
        d.default = "gloss".into();
        assert!(d.problem().is_none());
        assert_eq!(Draft::of(&d.to_def()).options, "Matte\nGloss");
    }

    #[test]
    fn manager_and_properties_page_draw() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.125,
            plan_core::WallKind::Exterior,
        );
        let door = cx.project.add_opening(0, w, 100.0, OpeningKind::Door).unwrap();
        for (name, ty) in [("Note", PropType::Text), ("Keyed", PropType::Bool), ("Gauge", PropType::Number)] {
            add_property(&mut cx, PropDef::new(PropKind::Door, name, ty)).unwrap();
        }
        let mut list = PropDef::new(PropKind::Door, "Rating", PropType::List);
        list.options = vec!["A".into(), "B".into()];
        add_property(&mut cx, list).unwrap();
        let session = PropSession::for_object(&cx, ObjectRef::Opening(door)).unwrap();
        let egui_ctx = egui::Context::default();
        let mut m = Manager::new();
        m.editing = Some((PropKind::Door, "Note".into()));
        let _ = egui_ctx.run(egui::RawInput::default(), |ctx| {
            assert!(m.show(ctx, &mut cx));
            egui::CentralPanel::default().show(ctx, |ui| page(ui, &mut session.borrow_mut()));
        });
        // A kind with no properties has no session, so its dialog has no tab.
        assert!(PropSession::for_object(&cx, ObjectRef::Wall(w)).is_none());
        reset();
        run_command(&mut cx, OPEN);
        assert!(manager_open());
        assert!(is_command(IMPORT) && !is_command("edit.copy"));
    }
}
