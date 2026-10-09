//! The Components and Object Information panels that Chief puts on the
//! specification dialog of nearly every object (manual pp. 1389-1392).
//!
//! Object Information holds the Code, Comment, Description, Manufacturer and
//! Supplier an object gives the Materials List (and schedules), with a price
//! and an Insert Macro button on each text field. Components lists the line
//! items the object makes in the Materials List, with Add Line Item, Remove
//! Line Item, Restore and Revert, and for the selected line its Count,
//! Extra, Price, % Markup, Labor, Equipment and Total Cost.
//!
//! The data lives in the plan (`plan_core::materials_data`). A host arms a
//! session when it opens the dialog ([`for_object`]), draws the dialog inside
//! [`with_current`] (the shared dialog frame then adds the two tabs), and
//! writes the session when OK is pressed ([`after_apply`]), so the panels and
//! the dialog are one undo step.

use super::{row, section};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Ui};
use plan_core::materials_data::{ExtraComponent, ObjectInfo, CATEGORIES};
use plan_docs::materials::list::{self, BaseComponent, Calc};
use std::cell::RefCell;
use std::rc::Rc;

/// The key under which the materials data of `o` is filed (the Property
/// Manager's keys, and `foundation:<id>` for foundation objects).
pub fn object_key(cx: &EditorContext, o: ObjectRef) -> Option<String> {
    match o {
        ObjectRef::Foundation(id) => Some(format!("foundation:{id}")),
        _ => super::property_manager::object_key(cx, o).map(|(k, _)| k.0),
    }
}

/// One object's panels being edited.
#[derive(Debug)]
pub struct InfoSession {
    pub key: String,
    pub floor: usize,
    original: ObjectInfo,
    pub draft: ObjectInfo,
    base: Vec<BaseComponent>,
    /// The line shown in the data table: an index into `base`, then the added
    /// lines after it.
    selected: usize,
    /// The plan's user text macros, for the Insert Macro menus.
    macros: Vec<String>,
}

/// A session shared between the host that applies it and the frame that
/// draws it.
pub type SharedInfo = Rc<RefCell<InfoSession>>;

impl InfoSession {
    pub fn new(cx: &EditorContext, floor: usize, key: String) -> Self {
        let master = crate::shell::layout_window::load_master_list();
        let c = Calc {
            project: &cx.project,
            master: &master,
            active_rooms: Some((cx.floor, cx.rooms.as_slice())),
        };
        let base = list::object_components(&c, floor, &key);
        let original = cx.project.materials.info(&key).cloned().unwrap_or_default();
        Self {
            key,
            floor,
            draft: original.clone(),
            original,
            base,
            selected: 0,
            macros: user_macro_names(&cx.project),
        }
    }

    /// A session for `o` of the active floor; `None` for the objects that
    /// make no Materials List lines and take no information (dimensions, CAD).
    pub fn for_object(cx: &EditorContext, o: ObjectRef) -> Option<SharedInfo> {
        let key = object_key(cx, o)?;
        Some(Rc::new(RefCell::new(Self::new(cx, cx.floor, key))))
    }

    /// Has anything been changed?
    pub fn changed(&self) -> bool {
        let mut a = self.draft.clone();
        a.prune();
        let mut b = self.original.clone();
        b.prune();
        a != b
    }

    /// The line items shown: the object's own, then the ones added.
    pub fn rows(&self) -> Vec<Row> {
        let mut v: Vec<Row> = Vec::new();
        for b in &self.base {
            let e = self.draft.component(&b.key);
            let removed = e.is_some_and(|e| e.removed);
            let g = |own: Option<f64>, f: Option<f64>| own.or(f);
            let price = g(e.and_then(|e| e.price), self.draft.price).or(b.price);
            let markup = g(e.and_then(|e| e.markup), self.draft.markup).unwrap_or(b.markup);
            let labor = g(e.and_then(|e| e.labor), self.draft.labor).unwrap_or(b.labor);
            let equipment =
                g(e.and_then(|e| e.equipment), self.draft.equipment).unwrap_or(b.equipment);
            let extra = g(e.and_then(|e| e.extra), self.draft.extra).unwrap_or(0.0);
            let count = e.and_then(|e| e.count).unwrap_or(b.count);
            v.push(Row {
                name: b.item.clone(),
                category: b.category.clone(),
                unit: b.unit.clone(),
                count,
                extra,
                price,
                markup,
                labor,
                equipment,
                removed,
                added: None,
                changed: e.is_some_and(|e| !e.is_empty()),
            });
        }
        for (i, a) in self.draft.added.iter().enumerate() {
            v.push(Row {
                name: a.description.clone(),
                category: a.category.clone(),
                unit: a.unit.clone(),
                count: a.count,
                extra: 0.0,
                price: a.price,
                markup: a.markup,
                labor: a.labor,
                equipment: a.equipment,
                removed: false,
                added: Some(i),
                changed: true,
            });
        }
        v
    }

    /// Remove Line Item on row `i` (an added line is deleted, a component of
    /// the object is marked removed).
    pub fn remove_row(&mut self, i: usize) {
        if i < self.base.len() {
            let key = self.base[i].key.clone();
            self.draft.component_mut(&key).removed = true;
        } else if i - self.base.len() < self.draft.added.len() {
            self.draft.added.remove(i - self.base.len());
        }
        self.selected = self.selected.min(self.rows().len().saturating_sub(1));
    }

    /// Restore: brings back the removed components.
    pub fn restore(&mut self) {
        for c in &mut self.draft.components {
            c.removed = false;
        }
        self.draft.prune();
    }

    /// Revert: drops every change to the components.
    pub fn revert(&mut self) {
        self.draft.components.clear();
        self.draft.added.clear();
        self.draft.price = None;
        self.draft.markup = None;
        self.draft.labor = None;
        self.draft.equipment = None;
        self.draft.extra = None;
    }

    /// Add Line Item.
    pub fn add_row(&mut self) {
        self.draft.added.push(ExtraComponent {
            description: "New line item".into(),
            ..ExtraComponent::default()
        });
        self.selected = self.rows().len() - 1;
    }

    /// Sets one number of row `i`: `None` restores the automatic value.
    pub fn set_number(&mut self, i: usize, field: Field, value: Option<f64>) {
        if i < self.base.len() {
            let key = self.base[i].key.clone();
            let c = self.draft.component_mut(&key);
            match field {
                Field::Count => c.count = value,
                Field::Extra => c.extra = value,
                Field::Price => c.price = value,
                Field::Markup => c.markup = value,
                Field::Labor => c.labor = value,
                Field::Equipment => c.equipment = value,
            }
            self.draft.prune();
        } else if let Some(a) = self.draft.added.get_mut(i - self.base.len()) {
            match field {
                Field::Count => a.count = value.unwrap_or(1.0),
                Field::Extra => {}
                Field::Price => a.price = value,
                Field::Markup => a.markup = value.unwrap_or(0.0),
                Field::Labor => a.labor = value.unwrap_or(0.0),
                Field::Equipment => a.equipment = value.unwrap_or(0.0),
            }
        }
    }

    /// Writes the draft into the plan. Returns whether anything changed.
    pub fn apply(&self, project: &mut plan_core::Project) -> bool {
        let mut d = self.draft.clone();
        d.prune();
        let before = project.materials.objects.get(&self.key).cloned();
        if d.is_empty() {
            project.materials.objects.remove(&self.key);
        } else {
            project
                .materials
                .objects
                .insert(self.key.clone(), d.clone());
        }
        project.materials.objects.get(&self.key).cloned() != before
    }
}

/// A number of a line item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Count,
    Extra,
    Price,
    Markup,
    Labor,
    Equipment,
}

/// One line of the Components table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    pub category: String,
    pub unit: String,
    pub count: f64,
    pub extra: f64,
    pub price: Option<f64>,
    pub markup: f64,
    pub labor: f64,
    pub equipment: f64,
    pub removed: bool,
    /// The index into the added lines, for a line the user added.
    pub added: Option<usize>,
    pub changed: bool,
}

impl Row {
    /// The Total Cost of the line (the manual's formula).
    pub fn total(&self) -> Option<f64> {
        list::total_cost(
            self.count,
            self.extra,
            self.price,
            self.markup,
            self.labor,
            self.equipment,
        )
    }
}

// ----------------------------------------------------------------- hosting --

thread_local! {
    static CURRENT: RefCell<Option<SharedInfo>> = const { RefCell::new(None) };
}

/// The session of the dialog being drawn, if its host armed one.
pub fn current() -> Option<SharedInfo> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Runs `f` (a dialog's `show`) with `session` as the Components and Object
/// Information tabs of every `SpecDialog` drawn inside it.
pub fn with_current<R>(session: Option<&SharedInfo>, f: impl FnOnce() -> R) -> R {
    let prev = CURRENT.with(|c| c.replace(session.cloned()));
    let r = f();
    CURRENT.with(|c| *c.borrow_mut() = prev);
    r
}

/// Makes `session` the current one (see [`with_current`]) and returns the one
/// it replaces, for the host to put back after drawing the dialog.
pub fn set_current(session: Option<SharedInfo>) -> Option<SharedInfo> {
    CURRENT.with(|c| c.replace(session))
}

/// After a dialog's own apply: writes the panels. When the dialog (or the
/// Properties tab) made an undo step, the change joins it; otherwise it is a
/// step of its own, "Object Information".
pub fn after_apply(cx: &mut EditorContext, session: Option<&SharedInfo>, depth_before: usize) {
    let Some(s) = session else { return };
    if !s.borrow().changed() {
        return;
    }
    let own_step = cx.undo_depth() == depth_before;
    if own_step {
        cx.begin_change("Object Information");
    }
    let changed = s.borrow().apply(&mut cx.project);
    if own_step && !changed {
        cx.cancel_change();
    }
    cx.mark_dirty();
}

// ------------------------------------------------------------------- pages --

/// The tab names the frame adds, given the names the dialog already has
/// (a Wall Specification has its own Components tab for the wall's layers,
/// a Cabinet Specification its own Object Information).
pub fn tab_names(existing: &[&str]) -> (String, String) {
    let pick = |name: &str| {
        if existing.contains(&name) {
            format!("{name} (Materials List)")
        } else {
            name.to_string()
        }
    };
    (pick("Components"), pick("Object Information"))
}

fn text_with_macro(ui: &mut Ui, label: &str, value: &mut String, project_macros: &[String]) {
    row(ui, label, |ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(260.0));
        ui.menu_button("Insert Macro", |ui| {
            for (name, help) in plan_core::text_styles::BUILT_IN_MACROS {
                if name.starts_with("room.") {
                    continue;
                }
                if ui.button(*name).on_hover_text(*help).clicked() {
                    value.push_str(&format!("%{name}%"));
                    ui.close_menu();
                }
            }
            for name in project_macros {
                if ui.button(name).clicked() {
                    value.push_str(&format!("%{name}%"));
                    ui.close_menu();
                }
            }
        });
    });
}

fn parse_opt(text: &str) -> Option<Option<f64>> {
    let t = text.trim().trim_start_matches('$').replace(',', "");
    if t.is_empty() {
        Some(None)
    } else {
        t.parse::<f64>().ok().map(Some)
    }
}

/// The Object Information panel.
pub fn info_page(ui: &mut Ui, s: &mut InfoSession) {
    section(ui, "Object Information");
    let user_macros = s.macros.clone();
    let d = &mut s.draft;
    text_with_macro(ui, "Code", &mut d.code, &user_macros);
    text_with_macro(ui, "Comment", &mut d.comment, &user_macros);
    text_with_macro(ui, "Description", &mut d.description, &user_macros);
    text_with_macro(ui, "Manufacturer", &mut d.manufacturer, &user_macros);
    text_with_macro(ui, "Supplier", &mut d.supplier, &user_macros);
    row(ui, "Sub Category", |ui| {
        ui.add(egui::TextEdit::singleline(&mut d.sub_category).desired_width(200.0));
    });
    row(ui, "Accounting Code", |ui| {
        ui.add(egui::TextEdit::singleline(&mut d.accounting_code).desired_width(200.0));
    });
    row(ui, "Price", |ui| {
        let mut text = d.price.map_or(String::new(), |p| format!("{p:.2}"));
        if ui
            .add(egui::TextEdit::singleline(&mut text).desired_width(100.0))
            .changed()
        {
            if let Some(v) = parse_opt(&text) {
                d.price = v;
            }
        }
        ui.weak("each line of this object, unless its Components row has its own");
    });
    ui.add_space(6.0);
    ui.weak("A Code, Supplier, Manufacturer, Price or Comment typed here is used in place of the one in the Master List.");
}

/// The Components panel.
pub fn components_page(ui: &mut Ui, s: &mut InfoSession) {
    section(ui, "Components");
    let rows = s.rows();
    ui.horizontal(|ui| {
        if ui.button("Add Line Item").clicked() {
            s.add_row();
        }
        if ui
            .add_enabled(!rows.is_empty(), egui::Button::new("Remove Line Item"))
            .clicked()
        {
            let i = s.selected;
            s.remove_row(i);
        }
        if ui
            .add_enabled(
                s.draft.components.iter().any(|c| c.removed),
                egui::Button::new("Restore"),
            )
            .clicked()
        {
            s.restore();
        }
        if ui.button("Revert").clicked() {
            s.revert();
        }
    });
    let rows = s.rows();
    if rows.is_empty() {
        ui.add_space(6.0);
        ui.weak("This object makes no Materials List lines.");
        return;
    }
    egui::Grid::new("om_components")
        .striped(true)
        .num_columns(3)
        .show(ui, |ui| {
            ui.strong("Component");
            ui.strong("Count");
            ui.strong("Total Cost");
            ui.end_row();
            for (i, r) in rows.iter().enumerate() {
                let text = if r.removed {
                    egui::RichText::new(format!("{} (removed)", r.name)).strikethrough()
                } else {
                    egui::RichText::new(r.name.clone())
                };
                if ui.selectable_label(s.selected == i, text).clicked() {
                    s.selected = i;
                }
                ui.label(format!(
                    "{}{}",
                    fmt_count(r.count),
                    if r.unit == "ea" {
                        String::new()
                    } else {
                        format!(" {}", r.unit)
                    }
                ));
                ui.label(plan_docs::fmt_money(r.total()));
                ui.end_row();
            }
        });
    let i = s.selected.min(rows.len() - 1);
    let r = rows[i].clone();
    ui.add_space(6.0);
    section(ui, "Materials List Data");
    if let Some(a) = r.added {
        let mut cat = s.draft.added[a].category.clone();
        row(ui, "Description", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut s.draft.added[a].description).desired_width(240.0),
            );
        });
        row(ui, "ID (Category)", |ui| {
            egui::ComboBox::from_id_salt("om_cat")
                .selected_text(cat.clone())
                .show_ui(ui, |ui| {
                    for c in CATEGORIES {
                        ui.selectable_value(&mut cat, c.to_string(), c);
                    }
                });
        });
        s.draft.added[a].category = cat;
    }
    for (field, label, value) in [
        (Field::Count, "Count", Some(r.count)),
        (Field::Extra, "Extra", Some(r.extra).filter(|v| *v != 0.0)),
        (Field::Price, "Price", r.price),
        (
            Field::Markup,
            "% Markup",
            Some(r.markup).filter(|v| *v != 0.0),
        ),
        (Field::Labor, "Labor", Some(r.labor).filter(|v| *v != 0.0)),
        (
            Field::Equipment,
            "Equipment",
            Some(r.equipment).filter(|v| *v != 0.0),
        ),
    ] {
        row(ui, label, |ui| {
            let mut text = value.map_or(String::new(), |v| format!("{v}"));
            if ui
                .add(egui::TextEdit::singleline(&mut text).desired_width(100.0))
                .changed()
            {
                if let Some(v) = parse_opt(&text) {
                    s.set_number(i, field, v);
                }
            }
        });
    }
    row(ui, "Total Cost", |ui| {
        ui.label(plan_docs::fmt_money(s.rows()[i].total()));
    });
    ui.add_space(4.0);
    ui.weak("(Count + Extra) x Price x (1 + % Markup / 100) + (Count + Extra) x Labor + (Count + Extra) x Equipment. Clear a field to go back to the automatic value.");
}

fn fmt_count(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

/// The macro names a project defines, for the Insert Macro menus.
pub fn user_macro_names(project: &plan_core::Project) -> Vec<String> {
    project
        .text_macros
        .macros
        .iter()
        .map(|m| m.name.clone())
        .collect()
}
