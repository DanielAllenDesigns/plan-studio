//! Exchange property data with Excel (the workflow of ArchiCAD's "Exchange
//! Property Data with Excel", on Plan Studio's schedules).
//!
//! # Export
//!
//! [`export_workbook`] writes one worksheet per schedule. Each sheet has a
//! hidden first column, `PlanStudio ID`, holding the stable id of the object
//! on that row ([`schedule_kinds::prop_key`]: `door:12`, `device:0:4`,
//! `room:0:120,84`), a bold header row, the schedule's columns, and then the
//! custom properties of the kind that the schedule does not show. Cells the
//! user may change are unlocked and tinted; computed cells (sizes, areas,
//! the mark of a wall) are shaded and locked by the sheet protection XML
//! (no password). List and Yes/No properties get drop-down validation. A
//! hidden `_meta` sheet records the plan path, the export time and, for each
//! sheet, the schedule kind and the column to field map, so the import reads
//! columns by field and not by their (renamable) headings. It also keeps the
//! text of every exported cell (the baseline), so the import can tell what
//! the user changed in Excel from what merely moved in the plan since the
//! export (a renumbered mark, a recomputed area).
//!
//! # Import
//!
//! [`plan_import`] reads sheets (from [`crate::xlsx_read::read_xlsx`] or
//! [`crate::xlsx_read::read_csv`]), matches rows to objects by id, and builds
//! a list of [`Change`]s (object, field, old, new) and [`Note`]s for what it
//! could not use: an object that was deleted since the export, a computed
//! column that was edited (ignored), a row with no id, a value the field's
//! type refuses. [`apply_changes`] then applies the accepted changes to the
//! project; the caller makes that one undo step.
//!
//! Without `_meta` (a CSV, or a workbook rebuilt by hand) the rows are
//! matched by id and the columns by heading. A row with no id is matched by
//! its mark and kind when the sheet's kind is known.

use crate::schedule_kinds::{self, effective_columns, prop_key, prop_kind_of, ActiveRooms, Entry};
use crate::xlsx::{to_xlsx_edit, ColumnData, EditColumn, EditSheet};
use crate::xlsx_read::ReadSheet;
use plan_core::props::{fmt_number, PropDef, PropKey, PropKind, PropType, COLUMN_PREFIX};
use plan_core::schedules::{Schedule, ScheduleKind, ScheduleLayer};
use plan_core::units::{fmt_ft_in, parse_ft_in};
use plan_core::{detect_rooms, Id, Point, Project};
use std::collections::HashMap;

/// Heading of the hidden id column.
pub const ID_HEADER: &str = "PlanStudio ID";
/// Name of the hidden bookkeeping sheet.
pub const META_SHEET: &str = "_meta";
/// Field id of the id column in the `_meta` column map.
pub const ID_FIELD: &str = "_id";
/// Version number written into `_meta`.
const FORMAT_VERSION: &str = "1";
/// A length shown to 1/16" is the same length within 1/32".
const LENGTH_TOLERANCE: f64 = 1.0 / 32.0 + 1e-9;
/// How far a room's centre may have moved and still be the same room, inches.
const ROOM_MATCH: f64 = 6.0;

// ===================================================================
// What can be edited
// ===================================================================

/// How an editable built-in field is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    Text,
    /// A decimal number in `min..=max`.
    Number { min: f64, max: f64 },
    /// A whole number, or blank for none.
    Count,
    /// A length (`3'-6"`); blank clears the override when `blank_ok`.
    Length { blank_ok: bool },
}

/// The built-in fields of a schedule of `kind` that import writes back.
/// Everything else (sizes, areas, counts, wall numbers) is computed from the
/// geometry and is locked in the exported sheet.
pub fn builtin_field(kind: ScheduleKind, field: &str) -> Option<FieldType> {
    use ScheduleKind as K;
    let text = Some(FieldType::Text);
    match (kind, field) {
        (K::Door | K::Window, "mark" | "manufacturer" | "model" | "supplier" | "comment") => text,
        (K::Door | K::Window, "description" | "object_id") => text,
        (K::Door | K::Window, "u_factor") => Some(FieldType::Number { min: 0.0, max: 10.0 }),
        (K::Door | K::Window, "shgc") => Some(FieldType::Number { min: 0.0, max: 1.0 }),
        (K::Room | K::RoomFinish, "name" | "floor_finish" | "ceiling_finish") => text,
        (K::Room | K::RoomFinish, "ceiling_height") => Some(FieldType::Length { blank_ok: true }),
        (K::Cabinet, "label") => text,
        (K::Electrical, "label") => text,
        (K::Electrical, "circuit") => Some(FieldType::Count),
        (K::Electrical, "height") => Some(FieldType::Length { blank_ok: false }),
        (K::Fixture | K::Furniture | K::Plant, "name") => text,
        _ => None,
    }
}

/// Reads `text` as a value of `ty`: `Err` says why it is refused.
fn check_builtin(ty: FieldType, text: &str) -> Result<(), String> {
    let t = text.trim();
    match ty {
        FieldType::Text => Ok(()),
        FieldType::Number { min, max } => match t.parse::<f64>() {
            Ok(v) if v.is_finite() && v >= min && v <= max => Ok(()),
            Ok(_) => Err(format!("\"{t}\" is outside {}..{}", fmt_number(min), fmt_number(max))),
            Err(_) => Err(format!("\"{t}\" is not a number")),
        },
        FieldType::Count => {
            if t.is_empty() || t.parse::<u32>().is_ok() {
                Ok(())
            } else {
                Err(format!("\"{t}\" is not a whole number"))
            }
        }
        FieldType::Length { blank_ok } => {
            if t.is_empty() {
                return if blank_ok {
                    Ok(())
                } else {
                    Err("needs a length".into())
                };
            }
            match parse_ft_in(t) {
                Some(v) if v.is_finite() && v >= 0.0 => Ok(()),
                _ => Err(format!("\"{t}\" is not a length (try 3'-6\")")),
            }
        }
    }
}

/// Do two texts of a built-in field say the same thing?
fn same_builtin(ty: FieldType, old: &str, new: &str) -> bool {
    let (o, n) = (old.trim(), new.trim());
    if o == n {
        return true;
    }
    match ty {
        FieldType::Text => false,
        FieldType::Number { .. } | FieldType::Count => {
            matches!((o.parse::<f64>(), n.parse::<f64>()), (Ok(a), Ok(b)) if (a - b).abs() < 1e-9)
        }
        FieldType::Length { .. } => {
            matches!((parse_ft_in(o), parse_ft_in(n)), (Some(a), Some(b)) if (a - b).abs() < LENGTH_TOLERANCE)
        }
    }
}

/// Do two texts of a custom property say the same thing?
fn same_prop(def: &PropDef, old: &str, new: &str) -> bool {
    use plan_core::props::PropValue;
    match (def.parse(old), def.parse(new)) {
        (Ok(Some(PropValue::Number(x))), Ok(Some(PropValue::Number(y)))) => {
            let tol = if def.ty == PropType::Length {
                LENGTH_TOLERANCE
            } else {
                1e-9
            };
            (x - y).abs() < tol
        }
        (Ok(a), Ok(b)) => a == b,
        _ => old.trim() == new.trim(),
    }
}

// ===================================================================
// Export
// ===================================================================

/// A schedule to export and the floor it is placed on.
#[derive(Debug, Clone, Copy)]
pub struct ExportSchedule<'a> {
    pub def: &'a Schedule,
    pub home_floor: usize,
}

/// Facts the `_meta` sheet records.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Where the plan file is, as the user sees it ("" for an unsaved plan).
    pub plan_path: String,
    /// When the export was made, as text (the caller formats the clock).
    pub exported_at: String,
    /// Also export the custom properties of the kind that the schedule does
    /// not show, after its own columns.
    pub all_props: bool,
}

/// One exported column: its field id, heading and editability.
struct Col {
    field: String,
    title: String,
    editable: bool,
    data: ColumnData,
    choices: Vec<String>,
}

fn column_of(project: &Project, kind: ScheduleKind, field: &str, title: String) -> Col {
    let mut col = Col {
        field: field.to_string(),
        title,
        editable: false,
        data: ColumnData::Text,
        choices: Vec::new(),
    };
    if field.starts_with(COLUMN_PREFIX) {
        let def = prop_kind_of(kind)
            .zip(field.strip_prefix(COLUMN_PREFIX))
            .and_then(|(pk, name)| project.props.def(pk, name));
        if let Some(d) = def {
            col.editable = true;
            col.data = if d.ty == PropType::Number {
                ColumnData::Number
            } else {
                ColumnData::Text
            };
            col.choices = match d.ty {
                PropType::List => d.options.clone(),
                PropType::Bool => vec!["Yes".into(), "No".into()],
                _ => Vec::new(),
            };
        }
    } else if let Some(ty) = builtin_field(kind, field) {
        col.editable = true;
        if matches!(ty, FieldType::Number { .. } | FieldType::Count) {
            col.data = ColumnData::Number;
        }
    }
    col
}

/// The columns a schedule exports: the ones it shows, then (with
/// `all_props`) the other custom properties of the kind.
fn export_columns(project: &Project, s: &ExportSchedule, all_props: bool) -> Vec<Col> {
    let def = s.def;
    let shown = effective_columns(project, def);
    let mut cols: Vec<Col> = shown
        .iter()
        .map(|c| {
            let title = if c.title.trim().is_empty() {
                def.kind
                    .fields()
                    .iter()
                    .find(|f| f.id == c.field)
                    .map(|f| f.title.to_string())
                    .or_else(|| c.field.strip_prefix(COLUMN_PREFIX).map(str::to_string))
                    .unwrap_or_default()
            } else {
                c.title.clone()
            };
            column_of(project, def.kind, &c.field, title)
        })
        .collect();
    if all_props {
        if let Some(pk) = prop_kind_of(def.kind) {
            for d in project.props.defs_for(pk) {
                let id = d.column_id();
                if !cols.iter().any(|c| c.field == id) {
                    cols.push(column_of(project, def.kind, &id, d.name.clone()));
                }
            }
        }
    }
    cols
}

/// The key of a row's object, as the `PlanStudio ID` cell.
fn row_id(e: &Entry) -> String {
    prop_key(e.kind, e.floor, e.id, e.position)
        .map(|k| k.0)
        .unwrap_or_default()
}

/// The worksheet of one schedule and the column map it records.
fn export_sheet(
    project: &Project,
    s: &ExportSchedule,
    active: ActiveRooms,
    all_props: bool,
) -> (EditSheet, Vec<Col>, Vec<Entry>) {
    let cols = export_columns(project, s, all_props);
    let entries = schedule_kinds::rows(project, s.def, s.home_floor, active);
    let mut columns = vec![EditColumn {
        header: ID_HEADER.into(),
        hidden: true,
        ..EditColumn::default()
    }];
    columns.extend(cols.iter().map(|c| EditColumn {
        header: c.title.clone(),
        editable: c.editable,
        data: c.data,
        choices: c.choices.clone(),
        ..EditColumn::default()
    }));
    let rows = entries
        .iter()
        .map(|e| {
            let mut r = vec![row_id(e)];
            r.extend(cols.iter().map(|c| e.cell(&c.field).to_string()));
            r
        })
        .collect();
    let sheet = EditSheet {
        title: s.def.display_title(),
        hidden: false,
        protect: true,
        columns,
        rows,
    };
    (sheet, cols, entries)
}

/// The sheet names the workbook will have, in order (Excel's rules, made
/// unique), so `_meta` can name them.
fn final_names(titles: &[String]) -> Vec<String> {
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    crate::xlsx::unique_sheet_names(&refs)
}

/// The workbook for editing: one sheet per schedule plus a hidden `_meta`.
pub fn export_workbook(
    project: &Project,
    schedules: &[ExportSchedule],
    active: ActiveRooms,
    opts: &ExportOptions,
) -> Vec<u8> {
    let built: Vec<(EditSheet, Vec<Col>, Vec<Entry>)> = schedules
        .iter()
        .map(|s| export_sheet(project, s, active, opts.all_props))
        .collect();
    let mut titles: Vec<String> = built.iter().map(|(s, _, _)| s.title.clone()).collect();
    titles.push(META_SHEET.to_string());
    let names = final_names(&titles);
    let mut meta: Vec<Vec<String>> = vec![
        row(&["format", "Plan Studio property exchange", FORMAT_VERSION]),
        row(&["project", &project.name]),
        row(&["plan", &opts.plan_path]),
        row(&["exported", &opts.exported_at]),
    ];
    for (i, s) in schedules.iter().enumerate() {
        meta.push(row(&[
            "sheet",
            &names[i],
            s.def.kind.name(),
            &s.home_floor.to_string(),
            &s.def.id.to_string(),
            "baseline",
        ]));
        let (_, cols, entries) = &built[i];
        meta.push(row(&[
            "column",
            &names[i],
            "1",
            ID_HEADER,
            ID_FIELD,
            "no",
        ]));
        for (j, c) in cols.iter().enumerate() {
            meta.push(row(&[
                "column",
                &names[i],
                &(j + 2).to_string(),
                &c.title,
                &c.field,
                if c.editable { "yes" } else { "no" },
            ]));
        }
        // The text of every exported cell, by object and field.
        for e in entries {
            let id = row_id(e);
            if id.is_empty() {
                continue;
            }
            for c in cols {
                let v = e.cell(&c.field);
                if !v.is_empty() {
                    meta.push(row(&["base", &names[i], &id, &c.field, v]));
                }
            }
        }
    }
    let mut sheets: Vec<EditSheet> = built.into_iter().map(|(s, _, _)| s).collect();
    sheets.push(EditSheet {
        title: META_SHEET.into(),
        hidden: true,
        protect: false,
        columns: ["tag", "a", "b", "c", "d", "e"]
            .iter()
            .map(|h| EditColumn {
                header: (*h).into(),
                ..EditColumn::default()
            })
            .collect(),
        rows: meta,
    });
    to_xlsx_edit(&sheets)
}

fn row(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|c| (*c).to_string()).collect()
}

/// One schedule as CSV for editing: the `PlanStudio ID` column first (not
/// hidden: CSV has no hidden columns), then the same columns as the workbook.
pub fn export_csv(
    project: &Project,
    s: &ExportSchedule,
    active: ActiveRooms,
    all_props: bool,
) -> String {
    let (sheet, _, _) = export_sheet(project, s, active, all_props);
    let header: Vec<String> = sheet.columns.iter().map(|c| c.header.clone()).collect();
    let table = crate::Schedule {
        title: sheet.title,
        columns: header,
        rows: sheet.rows,
    };
    table.to_csv()
}

// ===================================================================
// Import: reading the sheets
// ===================================================================

/// How a row's object is found again.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub kind: ScheduleKind,
    pub floor: usize,
    pub id: Id,
    pub position: Point,
    pub key: PropKey,
}

/// One edit the import proposes.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub sheet: String,
    /// The spreadsheet row, counting the header as row 1.
    pub row: usize,
    /// The object, in words ("Door D01").
    pub object: String,
    /// The built-in field id, or `prop:Name`.
    pub field: String,
    pub field_title: String,
    pub old: String,
    pub new: String,
    pub target: Target,
    /// The custom property edited, when it is one.
    pub prop: Option<(PropKind, String)>,
    /// Why the value cannot be applied (the review dialog shows it and the
    /// change is never applied).
    pub problem: Option<String>,
    /// Set when the plan's value also changed since the export: Excel's
    /// value would overwrite a newer one.
    pub conflict: Option<String>,
}

/// Why a note was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// The object on the row is gone from the plan.
    ObjectDeleted,
    /// A computed column was edited; the edit is ignored.
    Computed,
    /// The row has no object id (or one that is not an id).
    NoId,
    /// A column or sheet the import does not know.
    Unknown,
}

/// Something the import left out, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub kind: NoteKind,
    pub sheet: String,
    pub row: usize,
    pub text: String,
}

/// Everything the review dialog shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportPlan {
    pub changes: Vec<Change>,
    pub notes: Vec<Note>,
    pub sheets_read: usize,
    pub rows_read: usize,
    /// The plan path recorded in `_meta`, when there is one.
    pub plan_path: String,
    /// The export time recorded in `_meta`.
    pub exported_at: String,
}

impl ImportPlan {
    /// Changes that can be applied (no problem).
    pub fn applicable(&self) -> usize {
        self.changes.iter().filter(|c| c.problem.is_none()).count()
    }
}

/// One sheet's `_meta` record.
struct MetaSheet {
    kind: Option<ScheduleKind>,
    home_floor: usize,
    schedule_id: Id,
    /// `(zero-based column, field id)`.
    columns: Vec<(usize, String)>,
    /// The export recorded the text of every cell.
    has_base: bool,
}

/// Everything `_meta` says.
#[derive(Default)]
struct Meta {
    sheets: HashMap<String, MetaSheet>,
    /// `(sheet, object id, field)` to the exported text.
    base: HashMap<(String, String, String), String>,
    plan: String,
    exported: String,
}

fn kind_by_name(name: &str) -> Option<ScheduleKind> {
    ScheduleKind::ALL
        .into_iter()
        .find(|k| k.name().eq_ignore_ascii_case(name.trim()))
}

fn parse_meta(sheets: &[ReadSheet]) -> Meta {
    let mut out = Meta::default();
    let Some(meta) = sheets
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(META_SHEET))
    else {
        return out;
    };
    fn cell(r: &[String], i: usize) -> &str {
        r.get(i).map_or("", |s| s.trim())
    }
    for r in &meta.rows {
        match cell(r, 0) {
            "plan" => out.plan = cell(r, 1).to_string(),
            "exported" => out.exported = cell(r, 1).to_string(),
            "sheet" => {
                out.sheets.insert(
                    cell(r, 1).to_lowercase(),
                    MetaSheet {
                        kind: kind_by_name(cell(r, 2)),
                        home_floor: cell(r, 3).parse().unwrap_or(0),
                        schedule_id: cell(r, 4).parse().unwrap_or(0),
                        columns: Vec::new(),
                        has_base: cell(r, 5) == "baseline",
                    },
                );
            }
            "column" => {
                if let (Some(m), Ok(n)) = (
                    out.sheets.get_mut(&cell(r, 1).to_lowercase()),
                    cell(r, 2).parse::<usize>(),
                ) {
                    if n >= 1 {
                        m.columns.push((n - 1, cell(r, 4).to_string()));
                    }
                }
            }
            "base" => {
                // Not trimmed: a baseline is compared as the cell was written.
                out.base.insert(
                    (
                        cell(r, 1).to_lowercase(),
                        cell(r, 2).to_string(),
                        cell(r, 3).to_string(),
                    ),
                    r.get(4).cloned().unwrap_or_default(),
                );
            }
            _ => {}
        }
    }
    out
}

/// The kinds of schedule an object key could have come from.
fn kinds_of_key(key: &str) -> &'static [ScheduleKind] {
    use ScheduleKind as K;
    match key.split(':').next().unwrap_or("") {
        "door" => &[K::Door],
        "window" => &[K::Window],
        "wall" => &[K::Wall],
        "cabinet" => &[K::Cabinet],
        "device" => &[K::Electrical],
        "symbol" => &[K::Fixture, K::Furniture, K::Plant],
        "stair" => &[K::Stair],
        "room" => &[K::Room],
        "framing" => &[K::Framing],
        _ => &[],
    }
}

/// The field a column heading names for a schedule of `kind`: the field's
/// title or id, else a custom property's name.
fn field_for_header(project: &Project, kind: ScheduleKind, header: &str) -> Option<String> {
    let h = header.trim();
    if h.is_empty() {
        return None;
    }
    kind.fields()
        .iter()
        .find(|f| f.title.eq_ignore_ascii_case(h) || f.id.eq_ignore_ascii_case(h))
        .map(|f| f.id.to_string())
        .or_else(|| {
            let name = h.strip_prefix(COLUMN_PREFIX).unwrap_or(h);
            project
                .props
                .def(prop_kind_of(kind)?, name)
                .map(PropDef::column_id)
        })
}

/// What the plan holds for one sheet's schedule: its entries (marked the
/// way the schedule marks them) and an index by object key.
struct Index {
    entries: Vec<Entry>,
    by_key: HashMap<String, usize>,
}

impl Index {
    fn build(project: &Project, def: &Schedule, active: ActiveRooms) -> Self {
        let mut entries = schedule_kinds::entries(project, def.kind, active);
        schedule_kinds::number(&mut entries, def.numbering, &def.label_prefix);
        let mut by_key = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            if let Some(k) = prop_key(e.kind, e.floor, e.id, e.position) {
                by_key.entry(k.0).or_insert(i);
            }
        }
        Index { entries, by_key }
    }

    /// The entry for `key`; a room whose centre moved a little still counts.
    fn find(&self, key: &str) -> Option<&Entry> {
        if let Some(i) = self.by_key.get(key) {
            return self.entries.get(*i);
        }
        let rest = key.strip_prefix("room:")?;
        let (floor, xy) = rest.split_once(':')?;
        let floor: usize = floor.parse().ok()?;
        let (x, y) = xy.split_once(',')?;
        let p = Point::new(x.parse().ok()?, y.parse().ok()?);
        self.entries
            .iter()
            .filter(|e| e.floor == floor && matches!(e.kind, ScheduleKind::Room | ScheduleKind::RoomFinish))
            .map(|e| (e.position.dist(p), e))
            .filter(|(d, _)| *d <= ROOM_MATCH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, e)| e)
    }

    /// The entry with this mark (the fallback for a row with no id).
    fn find_by_mark(&self, mark: &str) -> Option<&Entry> {
        let m = mark.trim();
        if m.is_empty() {
            return None;
        }
        let mut hits = self.entries.iter().filter(|e| e.cell("mark") == m);
        let first = hits.next()?;
        hits.next().is_none().then_some(first)
    }
}

fn describe(e: &Entry, project: &Project) -> String {
    let what = e.kind.name();
    let label = match e.kind {
        ScheduleKind::Room | ScheduleKind::RoomFinish => e.name.clone(),
        ScheduleKind::Door | ScheduleKind::Window | ScheduleKind::Wall | ScheduleKind::Stair => {
            e.cell("mark").to_string()
        }
        _ => e.name.clone(),
    };
    let floor = if project.floors.len() > 1 {
        project
            .floors
            .get(e.floor)
            .map(|f| format!(" ({})", f.name))
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!("{what} {label}{floor}").replace("  ", " ")
}

/// Reads `sheets` into a list of proposed changes and notes.
///
/// `active` lets the Room schedule use the editor's own room detection for
/// the active floor, as the schedules do.
pub fn plan_import(
    project: &Project,
    sheets: &[ReadSheet],
    active: ActiveRooms,
) -> Result<ImportPlan, String> {
    let meta = parse_meta(sheets);
    let mut plan = ImportPlan {
        plan_path: meta.plan.clone(),
        exported_at: meta.exported.clone(),
        ..ImportPlan::default()
    };
    let data: Vec<&ReadSheet> = sheets
        .iter()
        .filter(|s| !s.name.eq_ignore_ascii_case(META_SHEET))
        .collect();
    if data.is_empty() {
        return Err("The workbook has no sheets to read.".into());
    }
    for sheet in data {
        plan.sheets_read += 1;
        read_sheet(project, sheet, &meta, active, &mut plan);
    }
    if plan.rows_read == 0 && plan.changes.is_empty() {
        return Err(plan.notes.first().map_or_else(
            || "Nothing to import: the workbook has no rows.".to_string(),
            |n| format!("{}. Export a schedule with Export for Editing first.", n.text),
        ));
    }
    Ok(plan)
}

fn read_sheet(
    project: &Project,
    sheet: &ReadSheet,
    all_meta: &Meta,
    active: ActiveRooms,
    plan: &mut ImportPlan,
) {
    let meta = all_meta.sheets.get(&sheet.name.to_lowercase());
    let sheet_key = sheet.name.to_lowercase();
    let Some(header) = sheet.rows.first() else {
        return;
    };
    // The id column: by the column map, else by its heading.
    let id_col = meta
        .and_then(|m| m.columns.iter().find(|(_, f)| f == ID_FIELD).map(|(i, _)| *i))
        .or_else(|| {
            header
                .iter()
                .position(|h| h.trim().eq_ignore_ascii_case(ID_HEADER))
        });
    let Some(id_col) = id_col else {
        plan.notes.push(Note {
            kind: NoteKind::Unknown,
            sheet: sheet.name.clone(),
            row: 1,
            text: format!("Sheet \"{}\" has no PlanStudio ID column and was skipped", sheet.name),
        });
        return;
    };
    // The schedule that was exported: for the marks and the kind.
    let def: Option<Schedule> = meta.and_then(|m| {
        project
            .floors
            .get(m.home_floor)
            .and_then(|f| ScheduleLayer::load(f).find(m.schedule_id).cloned())
    });
    let sheet_kind = meta.and_then(|m| m.kind).or_else(|| def.as_ref().map(|d| d.kind));
    let mut indexes: HashMap<ScheduleKind, Index> = HashMap::new();
    // The sheet's own schedule definition numbers the marks.
    let ensure_index = |indexes: &mut HashMap<ScheduleKind, Index>, kind: ScheduleKind| {
        indexes.entry(kind).or_insert_with(|| {
            let d = def
                .clone()
                .filter(|d| d.kind == kind)
                .unwrap_or_else(|| Schedule::new(kind, Point::ZERO));
            Index::build(project, &d, active)
        });
    };

    for (ri, cells) in sheet.rows.iter().enumerate().skip(1) {
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        plan.rows_read += 1;
        let at = ri + 1;
        let id = cells.get(id_col).map_or("", |c| c.trim()).to_string();
        // Find the object, and the kind this row is a schedule of.
        let kinds: Vec<ScheduleKind> = match (sheet_kind, id.is_empty()) {
            (Some(k), _) => vec![k],
            (None, false) => kinds_of_key(&id).to_vec(),
            (None, true) => Vec::new(),
        };
        if kinds.is_empty() {
            plan.notes.push(Note {
                kind: NoteKind::NoId,
                sheet: sheet.name.clone(),
                row: at,
                text: format!("Row {at} has no object id; skipped"),
            });
            continue;
        }
        let mut found: Option<(ScheduleKind, Entry)> = None;
        for k in &kinds {
            ensure_index(&mut indexes, *k);
            let idx = &indexes[k];
            let hit = if id.is_empty() {
                // No id: match by mark and kind when the mark column is known.
                let mark_col = meta
                    .and_then(|m| m.columns.iter().find(|(_, f)| f == "mark").map(|(i, _)| *i))
                    .or_else(|| header.iter().position(|h| field_for_header(project, *k, h).as_deref() == Some("mark")));
                mark_col
                    .and_then(|c| cells.get(c))
                    .and_then(|m| idx.find_by_mark(m))
            } else {
                idx.find(&id)
            };
            if let Some(e) = hit {
                found = Some((*k, e.clone()));
                break;
            }
        }
        let Some((kind, entry)) = found else {
            let text = if id.is_empty() {
                format!("Row {at}: no object has that mark; skipped")
            } else {
                format!("Row {at}: the object {id} is no longer in the plan; skipped")
            };
            plan.notes.push(Note {
                kind: if id.is_empty() { NoteKind::NoId } else { NoteKind::ObjectDeleted },
                sheet: sheet.name.clone(),
                row: at,
                text,
            });
            continue;
        };
        let Some(key) = prop_key(entry.kind, entry.floor, entry.id, entry.position) else {
            plan.notes.push(Note {
                kind: NoteKind::NoId,
                sheet: sheet.name.clone(),
                row: at,
                text: format!("Row {at}: {} has no single object to edit; skipped", describe(&entry, project)),
            });
            continue;
        };
        let target = Target {
            kind,
            floor: entry.floor,
            id: entry.id,
            position: entry.position,
            key,
        };
        let object = describe(&entry, project);
        // The columns of this row: from the map, else by heading.
        let columns: Vec<(usize, String)> = match meta {
            Some(m) => m.columns.clone(),
            None => header
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != id_col)
                .filter_map(|(i, h)| field_for_header(project, kind, h).map(|f| (i, f)))
                .collect(),
        };
        if meta.is_none() && ri == 1 {
            for (i, h) in header.iter().enumerate() {
                if i != id_col
                    && !h.trim().is_empty()
                    && !columns.iter().any(|(c, _)| *c == i)
                {
                    plan.notes.push(Note {
                        kind: NoteKind::Unknown,
                        sheet: sheet.name.clone(),
                        row: 1,
                        text: format!("Column \"{}\" is not a field of {} schedules; ignored", h.trim(), kind.name()),
                    });
                }
            }
        }
        let has_base = meta.is_some_and(|m| m.has_base) && !id.is_empty();
        for (col, field) in columns {
            if field == ID_FIELD {
                continue;
            }
            let Some(new) = cells.get(col) else { continue };
            let title = header.get(col).map_or(field.clone(), |h| h.trim().to_string());
            let old = entry.cell(&field).to_string();
            // What the cell said when it was exported; absent means blank.
            let base: Option<&str> = has_base.then(|| {
                all_meta
                    .base
                    .get(&(sheet_key.clone(), id.clone(), field.clone()))
                    .map_or("", String::as_str)
            });
            // Compares two texts of this field the way its type does.
            let (same, check, prop): FieldCheck =
                if let Some(name) = field.strip_prefix(COLUMN_PREFIX) {
                    let Some(pk) = prop_kind_of(kind) else { continue };
                    let Some(def) = project.props.def(pk, name) else {
                        if ri == 1 {
                            plan.notes.push(Note {
                                kind: NoteKind::Unknown,
                                sheet: sheet.name.clone(),
                                row: 1,
                                text: format!(
                                    "The property \"{name}\" no longer exists; its column is ignored"
                                ),
                            });
                        }
                        continue;
                    };
                    let d = def.clone();
                    (
                        Box::new(move |a, b| same_prop(&d, a, b)),
                        def.parse(new).err(),
                        Some((pk, def.name.clone())),
                    )
                } else if let Some(ty) = builtin_field(kind, &field) {
                    (
                        Box::new(move |a, b| same_builtin(ty, a, b)),
                        check_builtin(ty, new).err(),
                        None,
                    )
                } else {
                    // A computed column: an edit is ignored, with a note.
                    let edited = match base {
                        Some(b) => !loose_same(b, new),
                        None => !loose_same(&old, new),
                    };
                    if edited {
                        plan.notes.push(Note {
                            kind: NoteKind::Computed,
                            sheet: sheet.name.clone(),
                            row: at,
                            text: format!(
                                "Row {at}, {title}: \"{}\" is computed from the plan; the edit was ignored",
                                new.trim()
                            ),
                        });
                    }
                    continue;
                };
            // Only a cell the user changed in Excel is an edit (the plan may
            // have moved on since the export), and only if the plan does not
            // already say the same.
            if let Some(b) = base {
                if same(b, new) {
                    continue;
                }
            }
            if same(&old, new) {
                continue;
            }
            let conflict = base
                .filter(|b| !same(b, &old))
                .map(|b| format!("also changed in Plan Studio since the export (was \"{b}\", now \"{old}\")"));
            plan.changes.push(Change {
                sheet: sheet.name.clone(),
                row: at,
                object: object.clone(),
                field: field.clone(),
                field_title: title,
                old,
                new: new.trim().to_string(),
                target: target.clone(),
                prop,
                problem: check,
                conflict,
            });
        }
    }
}

/// How one field is compared, why its new text is refused, and the custom
/// property it is (if it is one).
type FieldCheck = (
    Box<dyn Fn(&str, &str) -> bool>,
    Option<String>,
    Option<(PropKind, String)>,
);

/// Two cells that read the same: equal text, or equal numbers.
fn loose_same(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    a == b || matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if (x - y).abs() < 1e-9)
}

// ===================================================================
// Import: applying
// ===================================================================

/// The outcome of [`apply_changes`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ApplyReport {
    pub applied: usize,
    /// Changes that could not be applied, with the reason.
    pub failed: Vec<String>,
}

fn floor_of(project: &mut Project, floor: usize) -> Result<&mut plan_core::Floor, String> {
    project
        .floors
        .get_mut(floor)
        .ok_or_else(|| "the floor is gone".to_string())
}

fn set_opening(project: &mut Project, t: &Target, field: &str, value: &str) -> Result<(), String> {
    let f = floor_of(project, t.floor)?;
    let o = f
        .openings
        .iter_mut()
        .find(|o| o.id == t.id)
        .ok_or("the door or window is gone")?;
    let v = value.trim();
    match field {
        "mark" => o.schedule_number = (!v.is_empty()).then(|| v.to_string()),
        "manufacturer" => o.extras.spec.schedule.manufacturer = v.to_string(),
        "model" => o.extras.spec.schedule.model = v.to_string(),
        "supplier" => o.extras.spec.schedule.supplier = v.to_string(),
        "comment" => o.extras.spec.schedule.comment = v.to_string(),
        "description" => o.extras.spec.info.description = v.to_string(),
        "object_id" => o.extras.spec.info.id = v.to_string(),
        "u_factor" => o.extras.spec.energy.u_factor = v.parse().map_err(|_| "not a number")?,
        "shgc" => o.extras.spec.energy.shgc = v.parse().map_err(|_| "not a number")?,
        _ => return Err(format!("{field} cannot be changed")),
    }
    Ok(())
}

fn set_room(
    project: &mut Project,
    t: &Target,
    field: &str,
    value: &str,
    active: ActiveRooms,
) -> Result<(), String> {
    let v = value.trim();
    let f = floor_of(project, t.floor)?;
    let detected;
    let rooms: &[plan_core::Room] = match active {
        Some((af, r)) if af == t.floor => r,
        _ => {
            detected = detect_rooms(&f.walls, 1.0);
            &detected
        }
    };
    let room = rooms
        .iter()
        .map(|r| (r.centroid.dist(t.position), r))
        .filter(|(d, _)| *d <= ROOM_MATCH)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, r)| r.clone())
        .ok_or("the room is gone")?;
    let at = f.room_names.iter().position(|n| room.contains(n.anchor));
    let idx = match at {
        Some(i) => i,
        None => {
            // An unnamed room: a name entry anchored at its centre starts it.
            let label = room.label.clone();
            f.room_names
                .push(plan_core::RoomName::new(room.centroid, label, ""));
            f.room_names.len() - 1
        }
    };
    let n = &mut f.room_names[idx];
    match field {
        "name" => n.name = v.to_string(),
        "floor_finish" => n.floor_finish = (!v.is_empty()).then(|| v.to_string()),
        "ceiling_finish" => n.ceiling_finish = (!v.is_empty()).then(|| v.to_string()),
        "ceiling_height" => {
            n.ceiling_height = if v.is_empty() {
                None
            } else {
                Some(parse_ft_in(v).ok_or("not a length")?)
            }
        }
        _ => return Err(format!("{field} cannot be changed")),
    }
    Ok(())
}

fn json_by_id(items: &mut [serde_json::Value], id: Id) -> Option<&mut serde_json::Value> {
    items
        .iter_mut()
        .find(|v| v.get("id").and_then(|i| i.as_u64()) == Some(id))
}

fn set_cabinet(project: &mut Project, t: &Target, field: &str, value: &str) -> Result<(), String> {
    let f = floor_of(project, t.floor)?;
    let c = json_by_id(&mut f.cabinets, t.id).ok_or("the cabinet is gone")?;
    match field {
        "label" => {
            c["label"] = serde_json::Value::String(value.trim().to_string());
            Ok(())
        }
        _ => Err(format!("{field} cannot be changed")),
    }
}

fn set_device(project: &mut Project, t: &Target, field: &str, value: &str) -> Result<(), String> {
    let f = floor_of(project, t.floor)?;
    let layer = f.electrical.as_mut().ok_or("the device is gone")?;
    let devices = layer
        .get_mut("devices")
        .and_then(|d| d.as_array_mut())
        .ok_or("the device is gone")?;
    let d = json_by_id(devices, t.id).ok_or("the device is gone")?;
    let v = value.trim();
    match field {
        "label" => d["label"] = serde_json::Value::String(v.to_string()),
        "circuit" => {
            d["circuit"] = if v.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(v.parse::<u32>().map_err(|_| "not a whole number")?)
            }
        }
        "height" => d["height"] = serde_json::json!(parse_ft_in(v).ok_or("not a length")?),
        _ => return Err(format!("{field} cannot be changed")),
    }
    Ok(())
}

fn set_symbol(project: &mut Project, t: &Target, field: &str, value: &str) -> Result<(), String> {
    let f = floor_of(project, t.floor)?;
    let s = f
        .symbols
        .iter_mut()
        .find(|s| s.id == t.id)
        .ok_or("the object is gone")?;
    match field {
        "name" => {
            s.label = value.trim().to_string();
            Ok(())
        }
        _ => Err(format!("{field} cannot be changed")),
    }
}

/// Is the object a key names still in the plan?
fn alive(project: &Project, t: &Target, active: ActiveRooms) -> bool {
    let Some(f) = project.floors.get(t.floor) else {
        return false;
    };
    use ScheduleKind as K;
    match t.kind {
        K::Door | K::Window => f.openings.iter().any(|o| o.id == t.id),
        K::Wall => f.wall(t.id).is_some(),
        K::Cabinet => f
            .cabinets
            .iter()
            .any(|v| v.get("id").and_then(|i| i.as_u64()) == Some(t.id)),
        K::Fixture | K::Furniture | K::Plant => f.symbol(t.id).is_some(),
        K::Electrical => f
            .electrical
            .as_ref()
            .and_then(|l| l.get("devices"))
            .and_then(|d| d.as_array())
            .is_some_and(|ds| {
                ds.iter()
                    .any(|v| v.get("id").and_then(|i| i.as_u64()) == Some(t.id))
            }),
        K::Stair => f
            .stairs
            .iter()
            .any(|v| v.get("id").and_then(|i| i.as_u64()) == Some(t.id)),
        K::Room | K::RoomFinish => {
            let detected;
            let rooms: &[plan_core::Room] = match active {
                Some((af, r)) if af == t.floor => r,
                _ => {
                    detected = detect_rooms(&f.walls, 1.0);
                    &detected
                }
            };
            rooms.iter().any(|r| r.centroid.dist(t.position) <= ROOM_MATCH)
        }
        K::Framing => t.id != 0,
        K::Note | K::General => false,
    }
}

/// Applies `changes` to `project`. The caller opens one undo step first. A
/// change with a `problem`, or whose object has gone, is reported in
/// `failed` and left out.
pub fn apply_changes(project: &mut Project, changes: &[&Change], active: ActiveRooms) -> ApplyReport {
    let mut report = ApplyReport::default();
    for c in changes {
        if let Some(p) = &c.problem {
            report
                .failed
                .push(format!("{} {}: {p}", c.object, c.field_title));
            continue;
        }
        if !alive(project, &c.target, active) {
            report
                .failed
                .push(format!("{}: the object is no longer in the plan", c.object));
            continue;
        }
        let result: Result<(), String> = match &c.prop {
            Some((pk, name)) => match project.props.def(*pk, name).cloned() {
                Some(def) => project
                    .props
                    .set_text(&c.target.key, &def, &c.new)
                    .map(|_| ()),
                None => Err(format!("the property \"{name}\" no longer exists")),
            },
            None => match c.target.kind {
                ScheduleKind::Door | ScheduleKind::Window => {
                    set_opening(project, &c.target, &c.field, &c.new)
                }
                ScheduleKind::Room | ScheduleKind::RoomFinish => {
                    set_room(project, &c.target, &c.field, &c.new, active)
                }
                ScheduleKind::Cabinet => set_cabinet(project, &c.target, &c.field, &c.new),
                ScheduleKind::Electrical => set_device(project, &c.target, &c.field, &c.new),
                ScheduleKind::Fixture | ScheduleKind::Furniture | ScheduleKind::Plant => {
                    set_symbol(project, &c.target, &c.field, &c.new)
                }
                k => Err(format!("{} schedules have no editable fields", k.name())),
            },
        };
        match result {
            Ok(()) => report.applied += 1,
            Err(e) => report
                .failed
                .push(format!("{} {}: {e}", c.object, c.field_title)),
        }
    }
    report
}

/// The height of an electrical device as the schedule prints it (kept here
/// so tests and the review dialog format lengths the same way).
pub fn length_text(inches: f64) -> String {
    fmt_ft_in(inches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{house, rect_walls};
    use crate::xlsx_read::{read_csv, read_xlsx};
    use plan_cabinets::Cabinet;
    use plan_core::props::{PropKey, PropKind, PropType};
    use plan_core::{OpeningKind, PlacedSymbol, WallKind};
    use plan_electrical::ElectricalLayer;

    fn fire_rating() -> PropDef {
        let mut d = PropDef::new(PropKind::Door, "Fire Rating", PropType::List);
        d.options = vec!["None".into(), "20 min".into(), "45 min".into()];
        d.default = "None".into();
        d.show_in_schedule = true;
        d
    }

    /// A house with doors and windows, a cabinet, a device, a fixture and a
    /// property on doors.
    fn rich() -> Project {
        let mut p = house();
        let second_door = p.floors[0].walls[2].id;
        p.add_opening(0, second_door, 100.0, OpeningKind::Door).unwrap();
        let mut cab = Cabinet::base(24.0);
        cab.id = p.alloc_id();
        cab.position = Point::new(20.0, 20.0);
        p.floors[0].set_cabinets(&[cab]).unwrap();
        let mut layer = ElectricalLayer::default();
        let mut d = plan_electrical::Device {
            id: 1,
            kind: plan_electrical::DeviceKind::all()[0],
            position: Point::new(50.0, 3.0),
            angle: 0.0,
            height: 16.0,
            wall_id: None,
            circuit: Some(3),
            label: "A".into(),
            switched_by: Vec::new(),
            finish: String::new(),
            hide_label: false,
        };
        d.id = 1;
        layer.add(d);
        p.floors[0].electrical = Some(serde_json::to_value(&layer).unwrap());
        let mut s = PlacedSymbol::new("core.plumbing.toilet_elongated", Point::new(60.0, 100.0), 20.0, 30.0, 30.0);
        s.label = String::new();
        p.add_symbol(0, s);
        p.props.add_def(fire_rating()).unwrap();
        p
    }

    fn schedule(kind: ScheduleKind) -> Schedule {
        let mut s = Schedule::new(kind, Point::ZERO);
        s.id = 1;
        s.floor_scope = plan_core::schedules::FloorScope::All;
        // Show every field so the whole row is exchanged.
        for c in &mut s.columns {
            c.visible = true;
        }
        s
    }

    fn opts() -> ExportOptions {
        ExportOptions {
            plan_path: "/plans/house.plan".into(),
            exported_at: "2026-10-08 09:00".into(),
            all_props: true,
        }
    }

    fn export_read(p: &Project, kinds: &[ScheduleKind]) -> Vec<ReadSheet> {
        let defs: Vec<Schedule> = kinds.iter().map(|k| schedule(*k)).collect();
        let list: Vec<ExportSchedule> = defs
            .iter()
            .map(|d| ExportSchedule { def: d, home_floor: 0 })
            .collect();
        read_xlsx(&export_workbook(p, &list, None, &opts())).unwrap()
    }

    fn col(sheet: &ReadSheet, header: &str) -> usize {
        sheet.rows[0]
            .iter()
            .position(|h| h == header)
            .unwrap_or_else(|| panic!("no column {header} in {:?}", sheet.rows[0]))
    }

    fn set(sheets: &mut [ReadSheet], sheet: &str, row: usize, header: &str, text: &str) {
        let s = sheets.iter_mut().find(|s| s.name == sheet).unwrap();
        let c = col(s, header);
        s.rows[row][c] = text.to_string();
    }

    fn apply_all(p: &mut Project, plan: &ImportPlan) -> ApplyReport {
        let refs: Vec<&Change> = plan.changes.iter().collect();
        apply_changes(p, &refs, None)
    }

    #[test]
    fn the_workbook_has_a_hidden_id_column_meta_and_protection() {
        let p = rich();
        let list = [ExportSchedule {
            def: &schedule(ScheduleKind::Door),
            home_floor: 0,
        }];
        let zip = export_workbook(&p, &list, None, &opts());
        let parts = crate::xlsx_read::read_zip_entries(&zip).unwrap();
        let sheet1 = String::from_utf8(parts["xl/worksheets/sheet1.xml"].clone()).unwrap();
        assert!(sheet1.contains("hidden=\"1\""), "{sheet1}");
        assert!(sheet1.contains("<sheetProtection sheet=\"1\""), "{sheet1}");
        assert!(sheet1.contains("<dataValidation type=\"list\""), "{sheet1}");
        assert!(sheet1.contains("&quot;None,20 min,45 min&quot;") || sheet1.contains("\"None,20 min,45 min\""), "{sheet1}");
        // Editable cells are unlocked (style 3), computed ones locked (style 4).
        assert!(sheet1.contains("s=\"3\"") && sheet1.contains("s=\"4\""));
        let wb = String::from_utf8(parts["xl/workbook.xml"].clone()).unwrap();
        assert!(wb.contains("name=\"_meta\"") && wb.contains("state=\"hidden\""), "{wb}");
        let styles = String::from_utf8(parts["xl/styles.xml"].clone()).unwrap();
        assert!(styles.contains("<protection locked=\"0\"/>"));

        let sheets = read_xlsx(&zip).unwrap();
        assert_eq!(sheets[0].name, "Door Schedule");
        assert_eq!(sheets[0].rows[0][0], ID_HEADER);
        // Two doors, each with its stable id; the Fire Rating column is last.
        let id_cells: Vec<&str> = sheets[0].rows[1..].iter().map(|r| r[0].as_str()).collect();
        assert_eq!(id_cells.len(), 2);
        assert!(id_cells.iter().all(|c| c.starts_with("door:")), "{id_cells:?}");
        let fr = col(&sheets[0], "Fire Rating");
        assert_eq!(sheets[0].rows[1][fr], "None", "the default shows");
        let meta = sheets.iter().find(|s| s.name == META_SHEET).unwrap();
        assert!(meta.hidden);
        let text = format!("{:?}", meta.rows);
        for want in ["/plans/house.plan", "2026-10-08 09:00", "Door", "prop:Fire Rating", "manufacturer"] {
            assert!(text.contains(want), "{want} in {text}");
        }
    }

    #[test]
    fn an_untouched_export_imports_to_no_changes_for_every_kind() {
        let p = rich();
        let kinds = [
            ScheduleKind::Door,
            ScheduleKind::Window,
            ScheduleKind::Wall,
            ScheduleKind::Cabinet,
            ScheduleKind::Electrical,
            ScheduleKind::Fixture,
        ];
        let sheets = export_read(&p, &kinds);
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert!(plan.changes.is_empty(), "{:#?}", plan.changes);
        assert!(plan.notes.is_empty(), "{:#?}", plan.notes);
        assert!(plan.rows_read >= 6);
        assert_eq!(plan.plan_path, "/plans/house.plan");
    }

    #[test]
    fn rooms_and_room_finish_round_trip_unchanged_too() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        p.set_room_name(0, Point::new(50.0, 50.0), "Den", "Den", &rooms);
        let sheets = export_read(&p, &[ScheduleKind::Room, ScheduleKind::RoomFinish]);
        assert_eq!(sheets[0].rows.len(), 2);
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert!(plan.changes.is_empty(), "{:#?}", plan.changes);
        assert!(plan.notes.is_empty(), "{:#?}", plan.notes);
    }

    #[test]
    fn edits_in_excel_become_changes_and_apply() {
        let mut p = rich();
        let mut sheets = export_read(&p, &[ScheduleKind::Door, ScheduleKind::Cabinet, ScheduleKind::Electrical, ScheduleKind::Fixture]);
        set(&mut sheets, "Door Schedule", 1, "Manufacturer", "Therma-Tru");
        set(&mut sheets, "Door Schedule", 1, "Mark", "D-101");
        set(&mut sheets, "Door Schedule", 2, "Fire Rating", "45 min");
        set(&mut sheets, "Door Schedule", 2, "SHGC", "0.45");
        set(&mut sheets, "Cabinet Schedule", 1, "Label", "Sink Base");
        set(&mut sheets, "Electrical Schedule", 1, "Label", "Range outlet");
        set(&mut sheets, "Electrical Schedule", 1, "Circuit", "7");
        set(&mut sheets, "Electrical Schedule", 1, "Mount Height", "3'-0\"");
        set(&mut sheets, "Fixture Schedule", 1, "Name", "Master WC");
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 9, "{:#?}", plan.changes);
        assert!(plan.notes.is_empty(), "{:#?}", plan.notes);
        let first = &plan.changes[0];
        assert_eq!(first.object, "Door D01");
        assert_eq!((first.field.as_str(), first.old.as_str(), first.new.as_str()), ("mark", "D01", "D-101"));
        let report = apply_all(&mut p, &plan);
        assert_eq!(report.applied, 9, "{:?}", report.failed);

        let doors: Vec<_> = p.floors[0]
            .openings
            .iter()
            .filter(|o| o.kind == OpeningKind::Door)
            .collect();
        let d1 = doors.iter().find(|o| o.schedule_number.as_deref() == Some("D-101")).unwrap();
        assert_eq!(d1.extras.spec.schedule.manufacturer, "Therma-Tru");
        let d2 = doors.iter().find(|o| o.id != d1.id).unwrap();
        assert!((d2.extras.spec.energy.shgc - 0.45).abs() < 1e-9);
        let def = p.props.def(PropKind::Door, "Fire Rating").unwrap().clone();
        assert_eq!(p.props.text(&PropKey::door(d2.id), &def), "45 min");
        assert_eq!(p.props.text(&PropKey::door(d1.id), &def), "None");
        assert_eq!(p.floors[0].cabinets[0]["label"], "Sink Base");
        let dev = &p.floors[0].electrical.as_ref().unwrap()["devices"][0];
        assert_eq!((dev["label"].as_str(), dev["circuit"].as_u64(), dev["height"].as_f64()), (Some("Range outlet"), Some(7), Some(36.0)));
        assert_eq!(p.floors[0].symbols[0].label, "Master WC");

        // The edits show in the schedules, and exporting again changes nothing.
        let again = export_read(&p, &[ScheduleKind::Door, ScheduleKind::Cabinet, ScheduleKind::Electrical, ScheduleKind::Fixture]);
        assert!(plan_import(&p, &again, None).unwrap().changes.is_empty());
        let door = &again[0];
        assert!(door.rows[1..].iter().any(|r| r[col(door, "Mark")] == "D-101"));
    }

    #[test]
    fn room_edits_name_finish_and_ceiling_apply() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let mut sheets = export_read(&p, &[ScheduleKind::Room]);
        set(&mut sheets, "Room Schedule", 1, "Name", "Pantry");
        set(&mut sheets, "Room Schedule", 1, "Floor Finish", "Oak");
        set(&mut sheets, "Room Schedule", 1, "Ceiling height", "10'-0\"");
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 3, "{:#?}", plan.changes);
        let report = apply_all(&mut p, &plan);
        assert_eq!(report.applied, 3, "{:?}", report.failed);
        let n = &p.floors[0].room_names[0];
        assert_eq!(n.name, "Pantry");
        assert_eq!(n.floor_finish.as_deref(), Some("Oak"));
        assert_eq!(n.ceiling_height, Some(120.0));
        // Clearing the ceiling cell puts the floor's height back.
        let mut sheets = export_read(&p, &[ScheduleKind::Room]);
        set(&mut sheets, "Room Schedule", 1, "Ceiling height", "");
        let plan = plan_import(&p, &sheets, None).unwrap();
        apply_all(&mut p, &plan);
        assert_eq!(p.floors[0].room_names[0].ceiling_height, None);
    }

    #[test]
    fn values_are_checked_against_their_type() {
        let mut p = rich();
        p.props
            .add_def(PropDef::new(PropKind::Door, "Weight", PropType::Number))
            .unwrap();
        p.props
            .add_def(PropDef::new(PropKind::Door, "Reveal", PropType::Length))
            .unwrap();
        p.props
            .add_def(PropDef::new(PropKind::Door, "Keyed", PropType::Bool))
            .unwrap();
        let mut sheets = export_read(&p, &[ScheduleKind::Door]);
        set(&mut sheets, "Door Schedule", 1, "SHGC", "1.7");
        set(&mut sheets, "Door Schedule", 1, "U-Factor", "warm");
        set(&mut sheets, "Door Schedule", 1, "Fire Rating", "90 min");
        set(&mut sheets, "Door Schedule", 1, "Weight", "heavy");
        set(&mut sheets, "Door Schedule", 1, "Reveal", "wide");
        set(&mut sheets, "Door Schedule", 1, "Keyed", "maybe");
        set(&mut sheets, "Door Schedule", 2, "Weight", "45.5");
        set(&mut sheets, "Door Schedule", 2, "Reveal", "0'-1 1/2\"");
        set(&mut sheets, "Door Schedule", 2, "Keyed", "TRUE");
        let plan = plan_import(&p, &sheets, None).unwrap();
        let bad: Vec<&Change> = plan.changes.iter().filter(|c| c.problem.is_some()).collect();
        assert_eq!(bad.len(), 6, "{bad:#?}");
        assert!(bad.iter().any(|c| c.field == "shgc" && c.problem.as_ref().unwrap().contains("outside")));
        assert!(bad.iter().any(|c| c.field == "prop:Fire Rating" && c.problem.as_ref().unwrap().contains("not one of")));
        assert_eq!(plan.applicable(), 3);
        let before = p.clone();
        let report = apply_all(&mut p, &plan);
        assert_eq!(report.applied, 3);
        assert_eq!(report.failed.len(), 6);
        // The invalid cells changed nothing on door 1; door 2 took its three.
        let d1 = before.floors[0].openings.iter().find(|o| o.kind == OpeningKind::Door).unwrap().id;
        assert_eq!(p.props.values.get(PropKey::door(d1).as_str()), None);
        let weight = p.props.def(PropKind::Door, "Weight").unwrap().clone();
        let reveal = p.props.def(PropKind::Door, "Reveal").unwrap().clone();
        let keyed = p.props.def(PropKind::Door, "Keyed").unwrap().clone();
        let d2 = p.floors[0].openings.iter().filter(|o| o.kind == OpeningKind::Door).map(|o| o.id).find(|i| *i != d1).unwrap();
        let k2 = PropKey::door(d2);
        assert_eq!(p.props.text(&k2, &weight), "45.5");
        assert_eq!(p.props.text(&k2, &reveal), "0'-1 1/2\"");
        assert_eq!(p.props.text(&k2, &keyed), "Yes");
    }

    #[test]
    fn a_deleted_object_is_a_conflict_note_and_the_rest_still_applies() {
        let mut p = rich();
        let mut sheets = export_read(&p, &[ScheduleKind::Door]);
        set(&mut sheets, "Door Schedule", 1, "Manufacturer", "Gone Co");
        set(&mut sheets, "Door Schedule", 2, "Manufacturer", "Kept Co");
        let gone = sheets[0].rows[1][0].clone();
        let gone_id: Id = gone.strip_prefix("door:").unwrap().parse().unwrap();
        p.remove_opening(0, gone_id);
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].new, "Kept Co");
        let note = plan.notes.iter().find(|n| n.kind == NoteKind::ObjectDeleted).unwrap();
        assert!(note.text.contains(&gone), "{note:?}");
        assert_eq!(note.row, 2);
        // An object deleted between the review and the apply is reported too.
        let mut q = p.clone();
        let live = plan.changes[0].target.id;
        q.remove_opening(0, live);
        let report = apply_all(&mut q, &plan);
        assert_eq!(report.applied, 0);
        assert!(report.failed[0].contains("no longer"));
    }

    #[test]
    fn plan_changes_since_the_export_are_not_edits_and_double_edits_are_flagged() {
        let mut p = rich();
        let mut sheets = export_read(&p, &[ScheduleKind::Door]);
        let id1: Id = sheets[0].rows[1][0].strip_prefix("door:").unwrap().parse().unwrap();
        let set_mfr = |p: &mut Project, v: &str| {
            let o = p.floors[0].openings.iter_mut().find(|o| o.id == id1).unwrap();
            o.extras.spec.schedule.manufacturer = v.to_string();
        };
        // The plan moves on (a manufacturer typed, a door renumbered by hand);
        // the workbook is untouched, so there is nothing to import.
        set_mfr(&mut p, "Plan Co");
        let other = p.floors[0].openings.iter_mut().find(|o| o.id != id1 && o.kind == OpeningKind::Door).unwrap();
        other.schedule_number = Some("X9".into());
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert!(plan.changes.is_empty(), "{:#?}", plan.changes);
        assert!(plan.notes.is_empty(), "{:#?}", plan.notes);
        // Excel edits the same cell the plan changed: Excel wins, flagged.
        set(&mut sheets, "Door Schedule", 1, "Manufacturer", "Excel Co");
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 1);
        let c = &plan.changes[0];
        assert_eq!((c.old.as_str(), c.new.as_str()), ("Plan Co", "Excel Co"));
        assert!(c.conflict.as_ref().unwrap().contains("Plan Co"), "{c:?}");
        // An Excel edit the plan already agrees with is not a change.
        set(&mut sheets, "Door Schedule", 1, "Manufacturer", "Plan Co");
        assert!(plan_import(&p, &sheets, None).unwrap().changes.is_empty());
        // An unflagged edit has no conflict.
        set(&mut sheets, "Door Schedule", 2, "Model", "M-1");
        assert!(plan_import(&p, &sheets, None).unwrap().changes[0].conflict.is_none());
    }

    #[test]
    fn a_computed_column_edit_is_ignored_with_a_note() {
        let p = rich();
        let mut sheets = export_read(&p, &[ScheduleKind::Door, ScheduleKind::Wall]);
        set(&mut sheets, "Door Schedule", 1, "Width", "9'-0\"");
        set(&mut sheets, "Wall Schedule", 1, "Length", "99'-0\"");
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert!(plan.changes.is_empty());
        let computed: Vec<&Note> = plan.notes.iter().filter(|n| n.kind == NoteKind::Computed).collect();
        assert_eq!(computed.len(), 2, "{:#?}", plan.notes);
        assert!(computed[0].text.contains("Width") && computed[0].text.contains("computed"));
    }

    #[test]
    fn rows_without_ids_and_unknown_sheets_are_noted() {
        let p = rich();
        let mut sheets = export_read(&p, &[ScheduleKind::Door]);
        // A row typed in by hand, with no id and no mark that exists.
        sheets[0].rows.push(vec![String::new(), "D99".into()]);
        // Clear the id of a real row: it falls back to its mark.
        sheets[0].rows[1][0].clear();
        set(&mut sheets, "Door Schedule", 1, "Manufacturer", "By Mark");
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 1, "{:#?}", plan.changes);
        assert_eq!(plan.changes[0].new, "By Mark");
        assert_eq!(plan.changes[0].object, "Door D01");
        assert_eq!(plan.notes.len(), 1);
        assert_eq!(plan.notes[0].kind, NoteKind::NoId);
        assert!(plan_import(&p, &[], None).is_err());
        let blank = vec![ReadSheet { name: "S".into(), hidden: false, rows: vec![vec!["Mark".into()], vec!["x".into()]] }];
        assert!(plan_import(&p, &blank, None).unwrap_err().contains("no PlanStudio ID column"));
    }

    #[test]
    fn csv_round_trips_without_meta_by_heading() {
        let mut p = rich();
        let def = schedule(ScheduleKind::Door);
        let csv = export_csv(&p, &ExportSchedule { def: &def, home_floor: 0 }, None, true);
        assert!(csv.starts_with("PlanStudio ID,Mark,"), "{csv}");
        let mut rows = read_csv(&csv);
        let mfr = rows[0].iter().position(|h| h == "Manufacturer").unwrap();
        let fr = rows[0].iter().position(|h| h == "Fire Rating").unwrap();
        rows[1][mfr] = "CSV Doors".into();
        rows[2][fr] = "20 min".into();
        rows[0].push("Mystery".into());
        for r in rows.iter_mut().skip(1) {
            r.push("?".into());
        }
        let sheets = vec![ReadSheet { name: "CSV".into(), hidden: false, rows }];
        let plan = plan_import(&p, &sheets, None).unwrap();
        assert_eq!(plan.changes.len(), 2, "{:#?}", plan.changes);
        assert!(plan.notes.iter().any(|n| n.kind == NoteKind::Unknown && n.text.contains("Mystery")));
        let report = apply_all(&mut p, &plan);
        assert_eq!(report.applied, 2);
        assert!(p.floors[0].openings.iter().any(|o| o.extras.spec.schedule.manufacturer == "CSV Doors"));
    }

    #[test]
    fn custom_properties_are_schedule_columns() {
        let mut p = rich();
        let def = p.props.def(PropKind::Door, "Fire Rating").unwrap().clone();
        let door = p.floors[0].openings.iter().find(|o| o.kind == OpeningKind::Door).unwrap().id;
        p.props.set_text(&PropKey::door(door), &def, "45 min").unwrap();
        // A schedule with no such column still shows it (show in schedule).
        let mut s = Schedule::new(ScheduleKind::Door, Point::ZERO);
        s.id = 1;
        let t = schedule_kinds::table(&p, &s, 0, None);
        assert_eq!(t.columns.last().unwrap(), "Fire Rating");
        assert!(t.rows.iter().any(|r| r.last().unwrap() == "45 min"));
        assert!(t.rows.iter().any(|r| r.last().unwrap() == "None"));
        // Not flagged: hidden until the schedule has the column.
        p.props.defs[0].show_in_schedule = false;
        assert!(!schedule_kinds::table(&p, &s, 0, None).columns.contains(&"Fire Rating".to_string()));
        s.columns.push(plan_core::schedules::ColumnSpec::new("prop:Fire Rating", "Rating", true));
        let t = schedule_kinds::table(&p, &s, 0, None);
        assert_eq!(t.columns.last().unwrap(), "Rating");
        // Sorting and filtering see it; a column of a deleted property goes.
        s.sort.field = "prop:Fire Rating".into();
        s.sort.descending = true;
        assert_eq!(schedule_kinds::table(&p, &s, 0, None).rows[0].last().unwrap(), "None");
        p.props.remove_def(PropKind::Door, "Fire Rating");
        assert!(!schedule_kinds::table(&p, &s, 0, None).columns.contains(&"Rating".to_string()));
        // The column survives a reload of the layer.
        let mut layer = ScheduleLayer::default();
        layer.add(s);
        layer.store(&mut p.floors[0]);
        assert!(ScheduleLayer::load(&p.floors[0]).schedules[0].columns.iter().any(|c| c.field == "prop:Fire Rating"));
    }

    #[test]
    fn duplicate_headings_and_odd_text_survive() {
        let mut p = rich();
        let mut cab = p.floors[0].cabinets[0].clone();
        cab["label"] = serde_json::json!("R&D <\"base\"> 007");
        p.floors[0].cabinets[0] = cab;
        let sheets = export_read(&p, &[ScheduleKind::Cabinet]);
        let label = col(&sheets[0], "Label");
        assert_eq!(sheets[0].rows[1][label], "R&D <\"base\"> 007");
        assert!(plan_import(&p, &sheets, None).unwrap().changes.is_empty());
    }

    #[test]
    fn same_text_comparisons_follow_the_field_type() {
        assert!(same_builtin(FieldType::Number { min: 0.0, max: 1.0 }, "0.30", "0.3"));
        assert!(!same_builtin(FieldType::Number { min: 0.0, max: 1.0 }, "0.30", "0.31"));
        assert!(same_builtin(FieldType::Length { blank_ok: false }, "9'-1 1/8\"", "109.125"));
        assert!(!same_builtin(FieldType::Length { blank_ok: false }, "9'-1 1/8\"", "9'-2\""));
        assert!(same_builtin(FieldType::Count, "3", "3.0"));
        assert!(!same_builtin(FieldType::Text, "a", "A"));
        assert!(check_builtin(FieldType::Count, "-1").is_err());
        assert!(check_builtin(FieldType::Length { blank_ok: false }, "").is_err());
        assert!(check_builtin(FieldType::Length { blank_ok: true }, "").is_ok());
        assert_eq!(length_text(36.0), "3'-0\"");
    }
}
