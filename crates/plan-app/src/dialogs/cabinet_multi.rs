//! The Cabinet Specification over several cabinets (Open Object with a
//! selection of cabinets, CB-631; tutorial pp. 249 and 268). Fields whose
//! values differ between the cabinets show "No Change" (a blank length, an
//! indeterminate check box, no combo choice); only the fields the user edits
//! are written to every cabinet, in one undo step (the shell wraps the
//! dialog's OK in one group).
//!
//! The dialog covers the settings Chief's multi dialog is used for: size and
//! elevation, countertop, backsplash, toe kick, door and drawer styles,
//! construction, label and show-open options. Everything else (face items,
//! accessories, materials) keeps its own per-cabinet value.

use super::cabinet::draw_preview;
use super::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::placed;
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_cabinets::{Backsplash, Cabinet, Countertop, DoorStyle, DrawerStyle, ToeKick};
use plan_core::Id;
use std::collections::{HashMap, HashSet};

const TABS: &[Tab] = &[on("General"), on("Box Construction"), on("Door/Drawer")];

/// One value of a field, as read from a cabinet.
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Len(f64),
    Bool(bool),
    Text(String),
    /// The cabinet does not have this setting (no countertop for a
    /// countertop field).
    NA,
}

impl Val {
    fn same(&self, other: &Val) -> bool {
        match (self, other) {
            (Val::Len(a), Val::Len(b)) => (a - b).abs() < 1e-6,
            (a, b) => a == b,
        }
    }
}

type Get = fn(&Cabinet) -> Val;
type Set = fn(&mut Cabinet, &Val);

struct Field {
    key: &'static str,
    label: &'static str,
    get: Get,
    set: Set,
}

fn len_of(v: &Val) -> f64 {
    if let Val::Len(x) = v {
        *x
    } else {
        0.0
    }
}

fn bool_of(v: &Val) -> bool {
    matches!(v, Val::Bool(true))
}

macro_rules! len_field {
    ($key:literal, $label:literal, |$c:ident| $get:expr, |$m:ident, $x:ident| $set:expr) => {
        Field {
            key: $key,
            label: $label,
            get: |$c| $get,
            set: |$m, v| {
                let $x = len_of(v);
                $set
            },
        }
    };
}

macro_rules! bool_field {
    ($key:literal, $label:literal, |$c:ident| $get:expr, |$m:ident, $x:ident| $set:expr) => {
        Field {
            key: $key,
            label: $label,
            get: |$c| Val::Bool($get),
            set: |$m, v| {
                let $x = bool_of(v);
                $set
            },
        }
    };
}

/// Every field the dialog edits.
fn fields() -> Vec<Field> {
    vec![
        len_field!("width", "Width", |c| Val::Len(c.width), |m, x| m.width =
            x.max(1.0)),
        len_field!("depth", "Depth", |c| Val::Len(c.depth), |m, x| m.depth =
            x.max(1.0)),
        len_field!("height", "Height", |c| Val::Len(c.height), |m, x| m
            .height =
            x.max(1.0)),
        len_field!(
            "elev",
            "Finished Floor to Bottom",
            |c| Val::Len(c.elevation),
            |m, x| m.elevation = x
        ),
        bool_field!(
            "has_top",
            "Countertop",
            |c| c.countertop.is_some(),
            |m, x| {
                if x && m.countertop.is_none() {
                    m.countertop = Some(Countertop::default());
                } else if !x {
                    m.countertop = None;
                    m.cutouts.clear();
                }
            }
        ),
        len_field!(
            "ct_thick",
            "Countertop Thickness",
            |c| c
                .countertop
                .as_ref()
                .map_or(Val::NA, |t| Val::Len(t.thickness)),
            |m, x| if let Some(t) = m.countertop.as_mut() {
                t.thickness = x.max(0.05)
            }
        ),
        len_field!(
            "ct_front",
            "Overhang Front",
            |c| c
                .countertop
                .as_ref()
                .map_or(Val::NA, |t| Val::Len(t.overhang_front)),
            |m, x| if let Some(t) = m.countertop.as_mut() {
                t.overhang_front = x
            }
        ),
        len_field!(
            "ct_sides",
            "Overhang Sides",
            |c| c
                .countertop
                .as_ref()
                .map_or(Val::NA, |t| Val::Len(t.overhang_sides)),
            |m, x| if let Some(t) = m.countertop.as_mut() {
                t.overhang_sides = x
            }
        ),
        len_field!(
            "ct_back",
            "Overhang Back",
            |c| c
                .countertop
                .as_ref()
                .map_or(Val::NA, |t| Val::Len(t.overhang_back)),
            |m, x| if let Some(t) = m.countertop.as_mut() {
                t.overhang_back = x
            }
        ),
        bool_field!(
            "has_bs",
            "Backsplash",
            |c| c.backsplash.is_some(),
            |m, x| {
                if x && m.backsplash.is_none() {
                    m.backsplash = Some(Backsplash::new(4.0, 0.5));
                } else if !x {
                    m.backsplash = None;
                }
            }
        ),
        len_field!(
            "bs_height",
            "Backsplash Height",
            |c| c
                .backsplash
                .as_ref()
                .map_or(Val::NA, |b| Val::Len(b.height)),
            |m, x| if let Some(b) = m.backsplash.as_mut() {
                b.height = x.max(0.1)
            }
        ),
        len_field!(
            "bs_thick",
            "Backsplash Thickness",
            |c| c
                .backsplash
                .as_ref()
                .map_or(Val::NA, |b| Val::Len(b.thickness)),
            |m, x| if let Some(b) = m.backsplash.as_mut() {
                b.thickness = x.max(0.05)
            }
        ),
        bool_field!("has_kick", "Toe Kick", |c| c.toe_kick.is_some(), |m, x| {
            if x && m.toe_kick.is_none() {
                m.toe_kick = Some(ToeKick::default());
            } else if !x {
                m.toe_kick = None;
            }
        }),
        len_field!(
            "tk_height",
            "Toe Kick Height",
            |c| c.toe_kick.as_ref().map_or(Val::NA, |t| Val::Len(t.height)),
            |m, x| if let Some(t) = m.toe_kick.as_mut() {
                t.height = x.max(0.0)
            }
        ),
        len_field!(
            "tk_depth",
            "Toe Kick Depth",
            |c| c.toe_kick.as_ref().map_or(Val::NA, |t| Val::Len(t.depth)),
            |m, x| if let Some(t) = m.toe_kick.as_mut() {
                t.depth = x.max(0.0)
            }
        ),
        bool_field!("framed", "Framed", |c| c.framed, |m, x| m.framed = x),
        bool_field!(
            "cut_room",
            "Cut Room Moldings",
            |c| c.cut_room_moldings,
            |m, x| m.cut_room_moldings = x
        ),
        bool_field!(
            "no_fillers",
            "Suppress Automatic Fillers",
            |c| c.suppress_fillers,
            |m, x| m.suppress_fillers = x
        ),
        bool_field!(
            "no_label",
            "Suppress Label",
            |c| c.suppress_label,
            |m, x| m.suppress_label = x
        ),
        bool_field!(
            "open_doors",
            "Show Doors Open",
            |c| c.show_open.doors,
            |m, x| m.show_open.doors = x
        ),
        bool_field!(
            "open_drawers",
            "Show Drawers Open",
            |c| c.show_open.drawers,
            |m, x| m.show_open.drawers = x
        ),
        bool_field!(
            "open_rollouts",
            "Show Rollouts Open",
            |c| c.show_open.rollouts,
            |m, x| m.show_open.rollouts = x
        ),
        Field {
            key: "door_style",
            label: "Door Style",
            get: |c| Val::Text(c.door_style.name.clone()),
            set: |m, v| {
                if let Val::Text(n) = v {
                    m.door_style.apply_builtin(n);
                }
            },
        },
        Field {
            key: "drawer_style",
            label: "Drawer Style",
            get: |c| Val::Text(c.drawer_style.name.clone()),
            set: |m, v| {
                if let Val::Text(n) = v {
                    m.drawer_style.apply_builtin(n);
                }
            },
        },
    ]
}

/// What the dialog keeps for one multi-cabinet session.
struct MultiForm {
    ids: Vec<Id>,
    /// The first cabinet; fields edit this and `touched` says which count.
    draft: Cabinet,
    table: Vec<Field>,
    mixed: HashSet<&'static str>,
    touched: HashSet<&'static str>,
    fields: Fields,
    /// Text typed into a mixed length field.
    bufs: HashMap<&'static str, String>,
}

pub struct CabinetMultiDialog {
    frame: SpecDialog,
    form: MultiForm,
}

impl CabinetMultiDialog {
    /// A dialog over `cabinets` (two or more).
    pub fn new(cabinets: Vec<Cabinet>) -> Self {
        let first = cabinets.first().cloned().expect("at least one cabinet");
        let table = fields();
        let mut mixed = HashSet::new();
        for f in &table {
            let v0 = (f.get)(&first);
            if cabinets.iter().skip(1).any(|c| !(f.get)(c).same(&v0)) {
                mixed.insert(f.key);
            }
        }
        Self {
            frame: SpecDialog::new("Cabinet Specification (Multiple Cabinets)", "cabinet_multi"),
            form: MultiForm {
                ids: cabinets.iter().map(|c| c.id).collect(),
                draft: first,
                table,
                mixed,
                touched: HashSet::new(),
                fields: Fields::default(),
                bufs: HashMap::new(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    #[cfg(test)]
    pub fn ids(&self) -> &[Id] {
        &self.form.ids
    }

    /// Does `key` show No Change (several cabinets disagree and it has not
    /// been edited)?
    #[cfg(test)]
    pub fn is_mixed(&self, key: &str) -> bool {
        self.form.is_mixed(key)
    }

    /// Keys of the fields edited so far.
    #[cfg(test)]
    pub fn touched(&self) -> Vec<&'static str> {
        let mut v: Vec<_> = self.form.touched.iter().copied().collect();
        v.sort_unstable();
        v
    }

    /// Edits field `key` of the draft the way its page does (tests).
    #[cfg(test)]
    pub fn edit(&mut self, key: &'static str, val: Val2) {
        let draft = self.form.draft.clone();
        let mut next = draft;
        if let Some(f) = self.form.table.iter().find(|f| f.key == key) {
            let v = match val {
                Val2::Len(x) => Val::Len(x),
                Val2::Bool(b) => Val::Bool(b),
                Val2::Text(t) => Val::Text(t),
            };
            (f.set)(&mut next, &v);
            self.form.draft = next;
            self.form.touched.insert(key);
        }
    }

    /// Writes the edited fields to every cabinet; the caller opens the undo
    /// group. Returns how many cabinets changed.
    pub fn apply(&self, cx: &mut EditorContext) -> usize {
        let f = &self.form;
        if f.touched.is_empty() {
            return 0;
        }
        let mut changed = 0;
        for id in &f.ids {
            let Some(before) = placed::cabinet_by_id(cx.floor(), *id) else {
                continue;
            };
            let mut next = before.clone();
            for fld in f.table.iter().filter(|fld| f.touched.contains(fld.key)) {
                let v = (fld.get)(&f.draft);
                if v != Val::NA {
                    (fld.set)(&mut next, &v);
                }
            }
            if next != before && placed::apply_cabinet(cx, &next) {
                changed += 1;
            }
        }
        changed
    }
}

/// Values the tests hand to [`CabinetMultiDialog::edit`].
#[cfg(test)]
pub enum Val2 {
    Len(f64),
    Bool(bool),
    Text(String),
}

impl MultiForm {
    fn is_mixed(&self, key: &str) -> bool {
        self.mixed.contains(key) && !self.touched.contains(key)
    }

    fn field(&self, key: &str) -> Option<&Field> {
        self.table.iter().find(|f| f.key == key)
    }

    fn set_draft(&mut self, key: &'static str, v: &Val) {
        if let Some(i) = self.table.iter().position(|f| f.key == key) {
            let set = self.table[i].set;
            set(&mut self.draft, v);
            self.touched.insert(key);
        }
    }

    /// A length row; mixed fields are blank ("No Change") until typed in.
    fn len(&mut self, ui: &mut Ui, key: &'static str) {
        let Some(f) = self.field(key) else {
            return;
        };
        let label = f.label;
        let cur = (f.get)(&self.draft);
        let Val::Len(mut x) = cur else {
            // The first cabinet has no such setting: nothing to edit.
            return;
        };
        if !self.is_mixed(key) {
            if self.fields.length_row(ui, label, key, &mut x) {
                self.set_draft(key, &Val::Len(x));
            }
            return;
        }
        let mut typed = None;
        row(ui, label, |ui| {
            let buf = self.bufs.entry(key).or_default();
            let resp = ui.add(
                egui::TextEdit::singleline(buf)
                    .desired_width(100.0)
                    .hint_text("No Change"),
            );
            if resp.changed() {
                typed = plan_core::units::parse_ft_in(buf);
            }
        });
        if let Some(v) = typed {
            self.set_draft(key, &Val::Len(v));
        }
    }

    /// A check box; the mixed state shows as indeterminate.
    fn chk(&mut self, ui: &mut Ui, key: &'static str) {
        let Some(f) = self.field(key) else {
            return;
        };
        let label = f.label;
        let mut v = bool_of(&(f.get)(&self.draft));
        let resp = ui.add(egui::Checkbox::new(&mut v, label).indeterminate(self.is_mixed(key)));
        if resp.changed() {
            self.set_draft(key, &Val::Bool(v));
        }
    }

    /// A built-in style combo; mixed shows "No Change".
    fn style(&mut self, ui: &mut Ui, key: &'static str, names: Vec<&'static str>) {
        let Some(f) = self.field(key) else {
            return;
        };
        let label = f.label;
        let Val::Text(cur) = (f.get)(&self.draft) else {
            return;
        };
        let mut pick = None;
        let shown = if self.is_mixed(key) {
            "No Change".to_string()
        } else {
            cur.clone()
        };
        row(ui, label, |ui| {
            egui::ComboBox::from_id_salt(("cab_multi", key))
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for n in names {
                        if ui
                            .selectable_label(n == cur && !self.mixed.contains(key), n)
                            .clicked()
                        {
                            pick = Some(n.to_string());
                        }
                    }
                });
        });
        if let Some(n) = pick {
            self.set_draft(key, &Val::Text(n));
        }
    }
}

impl SpecPages for MultiForm {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter valid lengths".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        ui.weak(format!(
            "{} cabinets. Fields marked No Change differ; only the fields you edit are applied.",
            self.ids.len()
        ));
        match TABS.get(tab).map(|t| t.name) {
            Some("General") => {
                section(ui, "Size/Position");
                for k in ["width", "depth", "height", "elev"] {
                    self.len(ui, k);
                }
                section(ui, "Countertop");
                self.chk(ui, "has_top");
                for k in ["ct_thick", "ct_front", "ct_sides", "ct_back"] {
                    self.len(ui, k);
                }
                section(ui, "Backsplash");
                self.chk(ui, "has_bs");
                for k in ["bs_height", "bs_thick"] {
                    self.len(ui, k);
                }
                section(ui, "Toe Kick");
                self.chk(ui, "has_kick");
                for k in ["tk_height", "tk_depth"] {
                    self.len(ui, k);
                }
            }
            Some("Box Construction") => {
                section(ui, "Box Construction");
                for k in ["framed", "cut_room", "no_fillers", "no_label"] {
                    self.chk(ui, k);
                }
                section(ui, "Show Open");
                for k in ["open_doors", "open_drawers", "open_rollouts"] {
                    self.chk(ui, k);
                }
            }
            Some("Door/Drawer") => {
                section(ui, "Styles");
                self.style(
                    ui,
                    "door_style",
                    DoorStyle::BUILTIN.iter().map(|(n, _)| *n).collect(),
                );
                self.style(
                    ui,
                    "drawer_style",
                    DrawerStyle::BUILTIN.iter().map(|(n, _)| *n).collect(),
                );
            }
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        draw_preview(painter, rect, &self.draft);
    }
}
