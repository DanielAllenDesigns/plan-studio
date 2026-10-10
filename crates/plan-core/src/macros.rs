//! Text macros (Round 16, brief 27; TXT-20, TXT-59 to TXT-65; reference
//! manual pp. 568 to 570 and 1446 to 1457).
//!
//! A macro is a `%name%` in a text. This module is the one place that knows
//! every macro the plan offers and evaluates them:
//!
//! * **Global** macros: the file (`%plan.name%`, `%floor%`...), the room the
//!   text sits in (`%room.name%`, `%room.nvp_name%`...), the date and time
//!   (`%date.short%`, `%time.short%`...), special characters (`%char.degree%`)
//!   and the **Project Information owners** (`%client.name%`,
//!   `%project.number%`, `%builder.license%`...). The owners are the three
//!   system owners Project, Designer and Client, which read the plan's
//!   [`ProjectInfo`], and the custom owners and extra name-value pairs kept
//!   in [`MacroOwners`].
//! * **User defined** macros (`Project::text_macros`): plain text that may use
//!   other macros. Ruby macros are out of scope (DECISIONS TM1).
//! * **Referenced object** macros (`%comment%`, `%description%`,
//!   `%automatic_description%`, `%automatic_label%`, `%nominal_size%`,
//!   `%width%`...): for a text with an arrow, they report the object the arrow
//!   points at ([`reference_facts`]).
//!
//! # Live texts
//!
//! A Text or Rich Text that holds macros keeps what was typed in
//! [`MacroTexts`] (`Project::macro_texts`, by the text's id) and its CAD item
//! holds what the macros give. [`Project::sync_macro_texts`] (run by the
//! editor's refresh, through `sync_annotations`) evaluates the macros again, so
//! the text follows the room name, the Project Information or the pointed-at
//! door. A text someone else changed (spell check, a painter) is no longer
//! live: the entry is dropped and the edit stands.

use crate::cad::{CadItem, TEXT_WIDTH_FACTOR};
use crate::geometry::{point_in_polygon, Point};
use crate::model::{Floor, Id, Project};
use crate::object_pages::{fmt_label_length, opening_automatic_label};
use crate::openings::{size_text, SizeFormat, SizeStyle};
use crate::rooms::{detect_rooms, Room};
use crate::schedules::ProjectInfo;
use crate::text_styles::{merge_runs, runs_plain, RichRun, TextMacro, TextMacros};
use crate::units::fmt_ft_in;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How deep a user macro may use other macros.
const MAX_DEPTH: usize = 6;

/// Seconds since 1970-01-01 (UTC); zero when the clock is unreadable.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ===================================================================
// Names
// ===================================================================

/// Is `name` shaped like a macro name (letters, digits, `.`, `_`, `-`)?
pub fn is_macro_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// `Drawn By` as it appears in a macro name: `drawn_by`.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if matches!(c, ' ' | '_' | '-' | '.') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

/// The macro (without percent signs) of name-value pair `name` of `owner`:
/// `client.phone`, `designer.drawn_by`.
pub fn owner_macro_name(owner: &str, name: &str) -> String {
    format!("{}.{}", slug(owner), slug(name))
}

/// The `%name%` tokens of `text`: `(start, end, name)` in bytes, `end`
/// after the closing percent sign. Names are macro-shaped; a lone `%` and
/// things like `50% of` are skipped.
pub fn tokens(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = text[i..].find('%') {
        let start = i + rel;
        let after = &text[start + 1..];
        let Some(end_rel) = after.find('%') else {
            break;
        };
        let name = &after[..end_rel];
        if is_macro_name(name) {
            let end = start + 1 + end_rel + 1;
            out.push((start, end, name.to_string()));
            i = end;
        } else {
            i = start + 1;
        }
    }
    out
}

// ===================================================================
// Project Information owners
// ===================================================================

pub const OWNER_PROJECT: &str = "Project";
pub const OWNER_DESIGNER: &str = "Designer";
pub const OWNER_CLIENT: &str = "Client";
/// The owners that cannot be renamed or deleted.
pub const SYSTEM_OWNERS: [&str; 3] = [OWNER_PROJECT, OWNER_DESIGNER, OWNER_CLIENT];

/// The names every system owner has (they cannot be edited or deleted).
pub fn system_names(owner: &str) -> &'static [&'static str] {
    match owner {
        OWNER_PROJECT => &["Number", "Address", "Lot", "Date", "Revision"],
        OWNER_DESIGNER => &["Name", "Company", "Drawn By", "Checked By"],
        OWNER_CLIENT => &["Name", "Address", "Phone", "Email"],
        _ => &[],
    }
}

/// The value a system name reads from the plan's [`ProjectInfo`]; `None` for
/// a name the information has no field for (`Lot`), which is stored in
/// [`MacroOwners`].
fn info_value(info: &ProjectInfo, owner: &str, name: &str) -> Option<String> {
    Some(match (owner, name) {
        (OWNER_PROJECT, "Number") => info.project_number.clone(),
        (OWNER_PROJECT, "Address") => info.project_address.clone(),
        (OWNER_PROJECT, "Date") => info.date.clone(),
        (OWNER_PROJECT, "Revision") => info.revision.clone(),
        (OWNER_DESIGNER, "Name") => info.designer.clone(),
        (OWNER_DESIGNER, "Company") => info.company.clone(),
        (OWNER_DESIGNER, "Drawn By") => info.drawn_by.clone(),
        (OWNER_DESIGNER, "Checked By") => info.checked_by.clone(),
        (OWNER_CLIENT, "Name") => info.client_name.clone(),
        (OWNER_CLIENT, "Address") => info.client_address.join("\n"),
        (OWNER_CLIENT, "Phone") => info.client_phone.clone(),
        (OWNER_CLIENT, "Email") => info.client_email.clone(),
        _ => return None,
    })
}

fn set_info_value(info: &mut ProjectInfo, owner: &str, name: &str, value: &str) -> bool {
    let v = value.to_string();
    match (owner, name) {
        (OWNER_PROJECT, "Number") => info.project_number = v,
        (OWNER_PROJECT, "Address") => info.project_address = v,
        (OWNER_PROJECT, "Date") => info.date = v,
        (OWNER_PROJECT, "Revision") => info.revision = v,
        (OWNER_DESIGNER, "Name") => info.designer = v,
        (OWNER_DESIGNER, "Company") => info.company = v,
        (OWNER_DESIGNER, "Drawn By") => info.drawn_by = v,
        (OWNER_DESIGNER, "Checked By") => info.checked_by = v,
        (OWNER_CLIENT, "Name") => info.client_name = v,
        (OWNER_CLIENT, "Address") => {
            info.client_address = value
                .lines()
                .map(|l| l.trim_end().to_string())
                .filter(|l| !l.trim().is_empty())
                .collect()
        }
        (OWNER_CLIENT, "Phone") => info.client_phone = v,
        (OWNER_CLIENT, "Email") => info.client_email = v,
        _ => return false,
    }
    true
}

/// One name-value pair of an owner.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Nvp {
    pub name: String,
    pub value: String,
}

/// An owner kept in the plan: a custom owner with its pairs, or a system
/// owner with the extra pairs added to it (and the value of `Lot`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Owner {
    pub name: String,
    pub pairs: Vec<Nvp>,
}

/// What the Project Information dialog shows for one name-value pair.
#[derive(Debug, Clone, PartialEq)]
pub struct PairView {
    pub name: String,
    pub value: String,
    /// A system default name: cannot be renamed or deleted.
    pub system: bool,
}

/// What the dialog shows for one owner.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerView {
    pub name: String,
    /// One of Project, Designer, Client.
    pub system: bool,
    pub pairs: Vec<PairView>,
}

/// The custom owners and extra pairs of Tools > Project Information.
///
/// They are kept inside [`ProjectInfo::custom`] under a reserved key
/// ([`OWNERS_KEY`]) so the plan needs no field of its own and every path that
/// stores the Project Information stores them ([`MacroOwners::from_info`],
/// [`MacroOwners::store`]). The custom fields of the Project owner are the
/// `custom` pairs of the information itself (`%custom.<name>%` in layouts).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MacroOwners {
    pub owners: Vec<Owner>,
}

/// The key of [`ProjectInfo::custom`] that holds the [`MacroOwners`] as JSON.
pub const OWNERS_KEY: &str = "\u{1}owners";

fn name_ok(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Type a name".into());
    }
    if slug(n).is_empty() {
        return Err("Use letters or digits in the name".into());
    }
    Ok(n.to_string())
}

impl MacroOwners {
    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    fn owner(&self, name: &str) -> Option<&Owner> {
        self.owners.iter().find(|o| o.name == name)
    }

    fn owner_mut(&mut self, name: &str) -> Option<&mut Owner> {
        self.owners.iter_mut().find(|o| o.name == name)
    }

    fn stored(&self, owner: &str, name: &str) -> Option<&str> {
        self.owner(owner)?
            .pairs
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value.as_str())
    }

    /// Does an owner called `name` exist (system or custom, any case)?
    pub fn has_owner(&self, name: &str) -> bool {
        let n = name.trim().to_lowercase();
        SYSTEM_OWNERS.iter().any(|s| s.to_lowercase() == n)
            || self.owners.iter().any(|o| o.name.to_lowercase() == n)
    }

    /// Reads the custom owners kept in `info`.
    pub fn from_info(info: &ProjectInfo) -> MacroOwners {
        info.custom
            .iter()
            .find(|(k, _)| k == OWNERS_KEY)
            .and_then(|(_, v)| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// Keeps the owners in `info` (nothing is kept when there are none).
    pub fn store(&self, info: &mut ProjectInfo) {
        info.custom.retain(|(k, _)| k != OWNERS_KEY);
        if !self.is_empty() {
            if let Ok(json) = serde_json::to_string(self) {
                info.custom.push((OWNERS_KEY.to_string(), json));
            }
        }
    }

    /// The custom fields of the Project owner: `info.custom` without the
    /// reserved key.
    fn project_custom(info: &ProjectInfo) -> impl Iterator<Item = &(String, String)> {
        info.custom
            .iter()
            .filter(|(k, _)| k != OWNERS_KEY && !k.trim().is_empty())
    }

    /// The owners as the dialog lists them: Project, Designer, Client, then
    /// the custom owners in the order they were added.
    pub fn views(&self, info: &ProjectInfo) -> Vec<OwnerView> {
        let mut out = Vec::new();
        for s in SYSTEM_OWNERS {
            let mut pairs: Vec<PairView> = system_names(s)
                .iter()
                .map(|n| PairView {
                    name: (*n).to_string(),
                    value: info_value(info, s, n)
                        .or_else(|| self.stored(s, n).map(str::to_string))
                        .unwrap_or_default(),
                    system: true,
                })
                .collect();
            if s == OWNER_PROJECT {
                for (k, v) in Self::project_custom(info) {
                    pairs.push(PairView {
                        name: k.clone(),
                        value: v.clone(),
                        system: false,
                    });
                }
            }
            if let Some(o) = self.owner(s) {
                for p in &o.pairs {
                    if !system_names(s).contains(&p.name.as_str()) {
                        pairs.push(PairView {
                            name: p.name.clone(),
                            value: p.value.clone(),
                            system: false,
                        });
                    }
                }
            }
            out.push(OwnerView {
                name: s.to_string(),
                system: true,
                pairs,
            });
        }
        for o in self
            .owners
            .iter()
            .filter(|o| !SYSTEM_OWNERS.contains(&o.name.as_str()))
        {
            out.push(OwnerView {
                name: o.name.clone(),
                system: false,
                pairs: o
                    .pairs
                    .iter()
                    .map(|p| PairView {
                        name: p.name.clone(),
                        value: p.value.clone(),
                        system: false,
                    })
                    .collect(),
            });
        }
        out
    }

    /// The value of pair `name` of `owner`.
    pub fn value(&self, info: &ProjectInfo, owner: &str, name: &str) -> String {
        self.views(info)
            .into_iter()
            .find(|v| v.name == owner)
            .and_then(|v| v.pairs.into_iter().find(|p| p.name == name))
            .map(|p| p.value)
            .unwrap_or_default()
    }

    /// Sets the value of an existing pair. False for an unknown owner or
    /// name.
    pub fn set_value(
        &mut self,
        info: &mut ProjectInfo,
        owner: &str,
        name: &str,
        value: &str,
    ) -> bool {
        if SYSTEM_OWNERS.contains(&owner) && set_info_value(info, owner, name, value) {
            return true;
        }
        if owner == OWNER_PROJECT {
            if let Some((_, v)) = info
                .custom
                .iter_mut()
                .find(|(k, _)| k == name && k != OWNERS_KEY)
            {
                *v = value.to_string();
                return true;
            }
        }
        let system_stored = SYSTEM_OWNERS.contains(&owner) && system_names(owner).contains(&name);
        if system_stored {
            // A system name the information has no field for (Lot).
            let o = self.owner_entry(owner);
            match o.pairs.iter_mut().find(|p| p.name == name) {
                Some(p) => p.value = value.to_string(),
                None => o.pairs.push(Nvp {
                    name: name.to_string(),
                    value: value.to_string(),
                }),
            }
            return true;
        }
        match self
            .owner_mut(owner)
            .and_then(|o| o.pairs.iter_mut().find(|p| p.name == name))
        {
            Some(p) => {
                p.value = value.to_string();
                true
            }
            None => false,
        }
    }

    fn owner_entry(&mut self, name: &str) -> &mut Owner {
        if self.owner(name).is_none() {
            self.owners.push(Owner {
                name: name.to_string(),
                pairs: Vec::new(),
            });
        }
        self.owner_mut(name).expect("just added")
    }

    /// Add: a new custom owner with no pairs.
    pub fn add_owner(&mut self, name: &str) -> Result<String, String> {
        let n = name_ok(name)?;
        if self.has_owner(&n) {
            return Err(format!("There is already an owner called {n}"));
        }
        self.owners.push(Owner {
            name: n.clone(),
            pairs: Vec::new(),
        });
        Ok(n)
    }

    /// Duplicate: a new custom owner (`<name> 2`) with the same pairs as
    /// `from`, a system owner's included.
    pub fn duplicate_owner(&mut self, info: &ProjectInfo, from: &str) -> Result<String, String> {
        let view = self
            .views(info)
            .into_iter()
            .find(|v| v.name == from)
            .ok_or_else(|| format!("No owner called {from}"))?;
        let mut n = 2;
        let name = loop {
            let c = format!("{from} {n}");
            if !self.has_owner(&c) {
                break c;
            }
            n += 1;
        };
        self.owners.push(Owner {
            name: name.clone(),
            pairs: view
                .pairs
                .into_iter()
                .map(|p| Nvp {
                    name: p.name,
                    value: p.value,
                })
                .collect(),
        });
        Ok(name)
    }

    /// Rename: a custom owner only.
    pub fn rename_owner(&mut self, from: &str, to: &str) -> Result<(), String> {
        if SYSTEM_OWNERS.contains(&from) {
            return Err(format!("{from} cannot be renamed"));
        }
        let to = name_ok(to)?;
        if to != from && self.has_owner(&to) {
            return Err(format!("There is already an owner called {to}"));
        }
        match self.owner_mut(from) {
            Some(o) => {
                o.name = to;
                Ok(())
            }
            None => Err(format!("No owner called {from}")),
        }
    }

    /// Delete: a custom owner only.
    pub fn delete_owner(&mut self, name: &str) -> bool {
        if SYSTEM_OWNERS.contains(&name) {
            return false;
        }
        let before = self.owners.len();
        self.owners.retain(|o| o.name != name);
        self.owners.len() != before
    }

    fn field_exists(
        &self,
        info: &ProjectInfo,
        owner: &str,
        name: &str,
        except: Option<&str>,
    ) -> bool {
        let n = name.to_lowercase();
        let mut names: Vec<String> = system_names(owner)
            .iter()
            .map(|s| s.to_lowercase())
            .collect();
        if owner == OWNER_PROJECT {
            names.extend(Self::project_custom(info).map(|(k, _)| k.to_lowercase()));
        }
        if let Some(o) = self.owner(owner) {
            names.extend(
                o.pairs
                    .iter()
                    .filter(|p| Some(p.name.as_str()) != except)
                    .map(|p| p.name.to_lowercase()),
            );
        }
        if let (Some(e), true) = (except, owner == OWNER_PROJECT) {
            // The field being renamed does not clash with itself.
            if let Some(i) = names.iter().position(|x| *x == e.to_lowercase()) {
                names.remove(i);
            }
        }
        names.contains(&n)
    }

    /// Add Field: a custom name-value pair on any owner.
    pub fn add_field(
        &mut self,
        info: &mut ProjectInfo,
        owner: &str,
        name: &str,
    ) -> Result<String, String> {
        let n = name_ok(name)?;
        if n == OWNERS_KEY || self.field_exists(info, owner, &n, None) {
            return Err(format!("{owner} already has a field called {n}"));
        }
        if !SYSTEM_OWNERS.contains(&owner) && self.owner(owner).is_none() {
            return Err(format!("No owner called {owner}"));
        }
        if owner == OWNER_PROJECT {
            info.custom.push((n.clone(), String::new()));
        } else {
            self.owner_entry(owner).pairs.push(Nvp {
                name: n.clone(),
                value: String::new(),
            });
        }
        Ok(n)
    }

    /// Rename a custom field.
    pub fn rename_field(
        &mut self,
        info: &mut ProjectInfo,
        owner: &str,
        from: &str,
        to: &str,
    ) -> Result<(), String> {
        if system_names(owner).contains(&from) {
            return Err(format!("{from} cannot be renamed"));
        }
        let to = name_ok(to)?;
        if self.field_exists(info, owner, &to, Some(from)) {
            return Err(format!("{owner} already has a field called {to}"));
        }
        if owner == OWNER_PROJECT {
            if let Some((k, _)) = info
                .custom
                .iter_mut()
                .find(|(k, _)| k == from && k != OWNERS_KEY)
            {
                *k = to;
                return Ok(());
            }
        }
        match self
            .owner_mut(owner)
            .and_then(|o| o.pairs.iter_mut().find(|p| p.name == from))
        {
            Some(p) => {
                p.name = to;
                Ok(())
            }
            None => Err(format!("No field called {from}")),
        }
    }

    /// Delete Field: a custom pair only.
    pub fn delete_field(&mut self, info: &mut ProjectInfo, owner: &str, name: &str) -> bool {
        if system_names(owner).contains(&name) || name == OWNERS_KEY {
            return false;
        }
        if owner == OWNER_PROJECT {
            let before = info.custom.len();
            info.custom.retain(|(k, _)| k != name);
            if info.custom.len() != before {
                return true;
            }
        }
        match self.owner_mut(owner) {
            Some(o) => {
                let before = o.pairs.len();
                o.pairs.retain(|p| p.name != name);
                o.pairs.len() != before
            }
            None => false,
        }
    }

    /// Clear Values: every value of `owner` goes blank, system names too.
    pub fn clear_values(&mut self, info: &mut ProjectInfo, owner: &str) {
        for n in system_names(owner) {
            set_info_value(info, owner, n, "");
        }
        if owner == OWNER_PROJECT {
            for (k, v) in &mut info.custom {
                if k != OWNERS_KEY {
                    v.clear();
                }
            }
        }
        if let Some(o) = self.owner_mut(owner) {
            for p in &mut o.pairs {
                p.value.clear();
            }
        }
    }

    /// Every pair as `(macro name, value)`: `client.name`, `project.lot`,
    /// `builder.license`. Addresses are on one line.
    pub fn pairs(&self, info: &ProjectInfo) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for v in self.views(info) {
            for p in &v.pairs {
                let value = if v.name == OWNER_CLIENT && p.name == "Address" {
                    info.client_address_line()
                } else {
                    p.value.clone()
                };
                out.push((owner_macro_name(&v.name, &p.name), value));
            }
        }
        out
    }
}

// ===================================================================
// The macros and their menu
// ===================================================================

/// Special characters: `(name, character, help)`, written `%char.<name>%`.
pub const SPECIAL_CHARS: &[(&str, &str, &str)] = &[
    ("degree", "\u{00B0}", "Degree sign"),
    ("plusminus", "\u{00B1}", "Plus or minus"),
    ("diameter", "\u{2300}", "Diameter sign"),
    ("feet", "'", "Feet mark"),
    ("inches", "\"", "Inches mark"),
    ("bullet", "\u{2022}", "Bullet"),
    ("copyright", "\u{00A9}", "Copyright sign"),
    ("registered", "\u{00AE}", "Registered sign"),
    ("trademark", "\u{2122}", "Trademark sign"),
    ("half", "\u{00BD}", "One half"),
    ("quarter", "\u{00BC}", "One quarter"),
    ("threequarters", "\u{00BE}", "Three quarters"),
    ("newline", "\n", "A line break"),
    ("percent", "%", "A percent sign"),
];

/// Macros of the file and the room: `(name, help)`.
pub const FILE_MACROS: &[(&str, &str)] = &[
    ("plan.name", "Plan (project) name"),
    ("plan.date", "Today's date, YYYY-MM-DD"),
    ("floor", "Name of the floor"),
    ("floor.number", "Number of the floor, 1 is the lowest"),
    ("floor.count", "Number of floors in the plan"),
    ("floor.height", "Ceiling height of the floor"),
];

pub const ROOM_MACROS: &[(&str, &str)] = &[
    ("room.name", "Name of the room the text is in"),
    ("room.nvp_name", "The room's name, for a custom room label"),
    ("room.number", "Number of the room"),
    ("room.type", "Room type"),
    ("room.area", "Floor area of the room"),
    ("room.perimeter", "Distance around the room"),
    ("room.ceiling_height", "Ceiling height of the room"),
];

pub const TIME_MACROS: &[(&str, &str)] = &[
    ("date.short", "Short date, 10/9/2026"),
    ("date.long", "Long date, October 9, 2026"),
    ("date.iso", "Date as 2026-10-09"),
    ("date.year", "Year"),
    ("date.month", "Month name"),
    ("date.day", "Day of the month"),
    ("date.weekday", "Day of the week"),
    ("time.short", "Time, 3:45 PM"),
    ("time.long", "Time with seconds, 15:45:09"),
];

/// Macros that report the object a text's arrow points at.
pub const REFERENCE_MACROS: &[(&str, &str)] = &[
    ("comment", "Comment from Object Information"),
    ("description", "Description from Object Information"),
    (
        "automatic_description",
        "The program's description of the object",
    ),
    ("automatic_label", "The object's Automatic Label"),
    ("nominal_size", "Nominal size, 3068 for 3'-0\" x 6'-8\""),
    ("object_type", "Door, Window, Wall, Cabinet..."),
    ("type", "The object's type"),
    ("name", "The object's name"),
    ("width", "Width"),
    ("height", "Height"),
    ("depth", "Depth"),
    ("length", "Length"),
    ("elevation", "Elevation above the floor"),
    ("schedule_number", "The Schedule Number"),
    ("code", "Code from Object Information"),
    ("manufacturer", "Manufacturer from Object Information"),
    ("supplier", "Supplier from Object Information"),
];

/// The macros of callouts, markers and notes (expanded when the annotation
/// is drawn, see `callout::expand_macros`).
pub const ANNOTATION_MACROS: &[(&str, &str)] = &[
    ("linked_view_name", "Name of the linked view"),
    (
        "linked_view_layout_page_label",
        "Layout page of the linked view",
    ),
    (
        "referenced_view_callout_label",
        "Callout label of the linked view",
    ),
    ("layout_page_label", "Layout page label"),
    ("automatic_label", "Callout label of the linked view"),
    ("simple_schedule_number", "Schedule number of a note"),
    ("height", "Height of a marker"),
    ("note_text", "Schedule text of a note"),
];

/// Every global macro name (no owners, no user macros).
pub fn global_names() -> Vec<String> {
    let mut v: Vec<String> = FILE_MACROS
        .iter()
        .chain(ROOM_MACROS)
        .chain(TIME_MACROS)
        .map(|(n, _)| (*n).to_string())
        .collect();
    v.extend(SPECIAL_CHARS.iter().map(|(n, _, _)| format!("char.{n}")));
    v
}

/// Can a user macro not take this name (it is a built-in, a reference or an
/// annotation macro, or a system owner's macro)?
pub fn is_reserved(name: &str) -> bool {
    global_names().iter().any(|n| n == name)
        || REFERENCE_MACROS.iter().any(|(n, _)| *n == name)
        || ANNOTATION_MACROS.iter().any(|(n, _)| *n == name)
        || SYSTEM_OWNERS
            .iter()
            .flat_map(|o| system_names(o).iter().map(move |n| owner_macro_name(o, n)))
            .any(|n| n == name)
        || matches!(
            name,
            "client"
                | "address"
                | "designer"
                | "date"
                | "revision"
                | "company"
                | "drawn.by"
                | "checked.by"
        )
}

/// One entry of the Insert Macro menu.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MenuItem {
    pub label: String,
    /// The macro it inserts, with percent signs; `None` for a submenu.
    pub insert: Option<String>,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    fn leaf(label: &str, name: &str) -> Self {
        Self {
            label: label.to_string(),
            insert: Some(format!("%{name}%")),
            children: Vec::new(),
        }
    }

    fn sub(label: &str, children: Vec<MenuItem>) -> Self {
        Self {
            label: label.to_string(),
            insert: None,
            children,
        }
    }

    /// Every macro this entry and its submenus insert.
    pub fn inserts(&self) -> Vec<String> {
        let mut out: Vec<String> = self.insert.iter().cloned().collect();
        for c in &self.children {
            out.extend(c.inserts());
        }
        out
    }
}

fn pretty(name: &str) -> String {
    let last = name.rsplit('.').next().unwrap_or(name);
    let mut out = String::new();
    for (i, w) in last.split('_').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut cs = w.chars();
        if let Some(c) = cs.next() {
            out.extend(c.to_uppercase());
            out.push_str(cs.as_str());
        }
    }
    out
}

/// The Insert Macro menu: Global (Project owners, File, Room, Time Date,
/// Special Characters), User Defined, Referenced Object and Callout.
pub fn insert_menu(project: &Project) -> Vec<MenuItem> {
    let owners: Vec<MenuItem> = MacroOwners::from_info(&project.info)
        .views(&project.info)
        .into_iter()
        .map(|o| {
            MenuItem::sub(
                &o.name,
                o.pairs
                    .iter()
                    .map(|p| MenuItem::leaf(&p.name, &owner_macro_name(&o.name, &p.name)))
                    .collect(),
            )
        })
        .collect();
    let list = |items: &[(&str, &str)]| -> Vec<MenuItem> {
        items
            .iter()
            .map(|(n, _)| MenuItem::leaf(&pretty(n), n))
            .collect()
    };
    let mut time: Vec<MenuItem> = TIME_MACROS
        .iter()
        .map(|(n, _)| {
            let label = match *n {
                "date.short" => "Short Date",
                "date.long" => "Long Date",
                "date.iso" => "ISO Date",
                "time.short" => "Short Time",
                "time.long" => "Long Time",
                _ => "",
            };
            let label = if label.is_empty() {
                pretty(n)
            } else {
                label.to_string()
            };
            MenuItem::leaf(&label, n)
        })
        .collect();
    time.sort_by_key(|m| {
        TIME_MACROS
            .iter()
            .position(|(n, _)| m.insert.as_deref() == Some(&format!("%{n}%")))
    });
    let chars: Vec<MenuItem> = SPECIAL_CHARS
        .iter()
        .map(|(n, c, _)| {
            let shown = if *c == "\n" { "line break" } else { c };
            MenuItem::leaf(&format!("{} ({shown})", pretty(n)), &format!("char.{n}"))
        })
        .collect();
    let user: Vec<MenuItem> = project
        .text_macros
        .macros
        .iter()
        .map(|m| MenuItem::leaf(&m.name, &m.name))
        .collect();
    vec![
        MenuItem::sub(
            "Global",
            vec![
                MenuItem::sub("Project Information", owners),
                MenuItem::sub("File", list(FILE_MACROS)),
                MenuItem::sub("Room", list(ROOM_MACROS)),
                MenuItem::sub("Time Date", time),
                MenuItem::sub("Special Characters", chars),
            ],
        ),
        MenuItem::sub("User Defined", user),
        MenuItem::sub("Referenced Object", list(REFERENCE_MACROS)),
        MenuItem::sub("Callout, Marker and Note", list(ANNOTATION_MACROS)),
    ]
}

/// What a macro reports, for the Text Macro Management list and tooltips.
pub fn help_for(project: &Project, name: &str) -> Option<String> {
    FILE_MACROS
        .iter()
        .chain(ROOM_MACROS)
        .chain(TIME_MACROS)
        .chain(REFERENCE_MACROS)
        .chain(ANNOTATION_MACROS)
        .find(|(n, _)| *n == name)
        .map(|(_, h)| (*h).to_string())
        .or_else(|| {
            SPECIAL_CHARS
                .iter()
                .find(|(n, _, _)| format!("char.{n}") == name)
                .map(|(_, _, h)| (*h).to_string())
        })
        .or_else(|| {
            project
                .text_macros
                .get(name)
                .map(|m| format!("User defined: {}", m.text))
        })
        .or_else(|| {
            MacroOwners::from_info(&project.info)
                .pairs(&project.info)
                .into_iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| format!("Project Information: {v}"))
        })
}

// ===================================================================
// Evaluation
// ===================================================================

/// A macro that could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    /// No macro has this name.
    Unknown,
    /// The macro uses itself, directly or through other macros.
    Loop,
    /// User macros use each other too deeply.
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub name: String,
    pub kind: IssueKind,
}

impl Issue {
    /// The sentence Show Evaluation Error gives.
    pub fn message(&self) -> String {
        match self.kind {
            IssueKind::Unknown => format!("There is no macro called %{}%", self.name),
            IssueKind::Loop => format!("%{}% uses itself", self.name),
            IssueKind::TooDeep => format!("%{}% is used too many levels deep", self.name),
        }
    }
}

/// A text after evaluation and what went wrong.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Expanded {
    pub text: String,
    pub issues: Vec<Issue>,
}

/// The values of every macro for one text.
#[derive(Debug, Clone, Default)]
pub struct Env {
    values: HashMap<String, String>,
    user: TextMacros,
}

/// `(year, month, day)` of a day count since 1970-01-01.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
];

/// The date and time macros for `unix` seconds (UTC): `(name, value)`.
pub fn time_values(unix: i64) -> Vec<(&'static str, String)> {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    let (h, mi, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    vec![
        ("date.short", format!("{m}/{d}/{y}")),
        (
            "date.long",
            format!("{} {d}, {y}", MONTHS[(m - 1) as usize]),
        ),
        ("date.iso", format!("{y:04}-{m:02}-{d:02}")),
        ("date.year", y.to_string()),
        ("date.month", MONTHS[(m - 1) as usize].to_string()),
        ("date.day", d.to_string()),
        (
            "date.weekday",
            WEEKDAYS[days.rem_euclid(7) as usize].to_string(),
        ),
        (
            "time.short",
            format!("{h12}:{mi:02} {}", if h < 12 { "AM" } else { "PM" }),
        ),
        ("time.long", format!("{h:02}:{mi:02}:{s:02}")),
    ]
}

impl Env {
    /// The global macros of `project` for a text on `floor` whose centre is
    /// `at` (the room it sits in answers the room macros); `rooms` are the
    /// detected rooms of that floor; `now` is the clock.
    pub fn new(
        project: &Project,
        floor: usize,
        at: Option<Point>,
        rooms: &[Room],
        now: i64,
    ) -> Env {
        let mut values: HashMap<String, String> = HashMap::new();
        let f = project.floors.get(floor);
        let put = |values: &mut HashMap<String, String>, n: &str, v: String| {
            values.insert(n.to_string(), v);
        };
        // Project Information: the title-block names first (`%client%`...),
        // then every owner's pairs.
        for (k, v) in project.info.macro_pairs() {
            let k = k.trim_matches('%').to_string();
            values.insert(k, v);
        }
        for (k, v) in MacroOwners::from_info(&project.info).pairs(&project.info) {
            values.insert(k, v);
        }
        put(&mut values, "plan.name", project.name.clone());
        put(
            &mut values,
            "plan.date",
            crate::text_styles::date_string(now),
        );
        put(
            &mut values,
            "floor",
            f.map(|f| f.name.clone()).unwrap_or_default(),
        );
        put(&mut values, "floor.number", (floor + 1).to_string());
        put(&mut values, "floor.count", project.floors.len().to_string());
        put(
            &mut values,
            "floor.height",
            f.map(|f| fmt_ft_in(f.ceiling_height)).unwrap_or_default(),
        );
        for (n, v) in time_values(now) {
            put(&mut values, n, v);
        }
        for (n, c, _) in SPECIAL_CHARS {
            put(&mut values, &format!("char.{n}"), (*c).to_string());
        }
        // The room the text is in.
        let room = at.and_then(|p| {
            rooms
                .iter()
                .find(|r| point_in_polygon(p, &r.inner_polygon) || point_in_polygon(p, &r.polygon))
        });
        let entry = room.and_then(|r| f.and_then(|f| r.name_entry(&f.room_names)));
        let name = match (room, entry) {
            (_, Some(e)) if !e.name.is_empty() => e.name.clone(),
            (Some(r), _) => r.label.clone(),
            _ => String::new(),
        };
        put(&mut values, "room.name", name.clone());
        put(&mut values, "room.nvp_name", name);
        put(&mut values, "room.number", String::new());
        put(
            &mut values,
            "room.type",
            entry.map(|e| e.room_type.clone()).unwrap_or_default(),
        );
        put(
            &mut values,
            "room.area",
            room.map(|r| format!("{} sq ft", r.interior_area_sq_ft().round()))
                .unwrap_or_default(),
        );
        put(
            &mut values,
            "room.perimeter",
            room.map(|r| {
                let n = r.inner_polygon.len();
                let per: f64 = (0..n)
                    .map(|i| r.inner_polygon[i].dist(r.inner_polygon[(i + 1) % n]))
                    .sum();
                fmt_ft_in(per)
            })
            .unwrap_or_default(),
        );
        put(
            &mut values,
            "room.ceiling_height",
            room.map(|_| {
                entry
                    .and_then(|e| e.ceiling_height)
                    .or(f.map(|f| f.ceiling_height))
                    .map(fmt_ft_in)
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        );
        Env {
            values,
            user: project.text_macros.clone(),
        }
    }

    /// An environment with no plan behind it (tests, free-standing text).
    pub fn bare(now: i64) -> Env {
        let mut env = Env::default();
        for (n, v) in time_values(now) {
            env.values.insert(n.to_string(), v);
        }
        for (n, c, _) in SPECIAL_CHARS {
            env.values.insert(format!("char.{n}"), (*c).to_string());
        }
        env
    }

    /// Adds or replaces macros (the referenced object's, owners...).
    pub fn with_values(mut self, extra: &[(String, String)]) -> Env {
        for (k, v) in extra {
            self.values.insert(k.clone(), v.clone());
        }
        self
    }

    pub fn with_user(mut self, user: TextMacros) -> Env {
        self.user = user;
        self
    }

    /// Is `name` a macro this environment knows (a value or a user macro)?
    pub fn knows(&self, name: &str) -> bool {
        self.values.contains_key(name)
            || self.user.get(name).is_some()
            || REFERENCE_MACROS.iter().any(|(n, _)| *n == name)
    }

    /// `text` with the macros replaced. Unknown names and stray percent
    /// signs stay as typed.
    pub fn expand(&self, text: &str) -> String {
        self.evaluate(text).text
    }

    /// Like [`Env::expand`], and says which macros could not be evaluated.
    pub fn evaluate(&self, text: &str) -> Expanded {
        let mut issues = Vec::new();
        let mut stack: Vec<String> = Vec::new();
        let text = self.eval(text, &mut stack, &mut issues);
        Expanded { text, issues }
    }

    fn eval(&self, text: &str, stack: &mut Vec<String>, issues: &mut Vec<Issue>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for (start, end, name) in tokens(text) {
            out.push_str(&text[at..start]);
            at = end;
            if let Some(v) = self.values.get(&name) {
                out.push_str(v);
            } else if let Some(m) = self.user.get(&name) {
                if stack.contains(&name) {
                    issues.push(Issue {
                        name: name.clone(),
                        kind: IssueKind::Loop,
                    });
                    out.push_str(&text[start..end]);
                } else if stack.len() >= MAX_DEPTH {
                    issues.push(Issue {
                        name: name.clone(),
                        kind: IssueKind::TooDeep,
                    });
                    out.push_str(&text[start..end]);
                } else {
                    stack.push(name.clone());
                    let inner = self.eval(&m.text, stack, issues);
                    stack.pop();
                    out.push_str(&inner);
                }
            } else {
                if !REFERENCE_MACROS.iter().any(|(n, _)| *n == name)
                    && !ANNOTATION_MACROS.iter().any(|(n, _)| *n == name)
                {
                    issues.push(Issue {
                        name: name.clone(),
                        kind: IssueKind::Unknown,
                    });
                }
                out.push_str(&text[start..end]);
            }
        }
        out.push_str(&text[at..]);
        out
    }

    /// Does `text` hold at least one macro this environment knows?
    pub fn has_macros(&self, text: &str) -> bool {
        tokens(text).iter().any(|(_, _, n)| self.knows(n))
    }
}

/// The error Show Evaluation Error gives for user macro `name`, if its text
/// does not evaluate cleanly (an unknown macro, a loop).
pub fn macro_error(project: &Project, name: &str) -> Option<String> {
    let m = project.text_macros.get(name)?;
    let env = Env::new(project, 0, None, &[], unix_now());
    let mut stack = vec![name.to_string()];
    let mut issues = Vec::new();
    env.eval(&m.text, &mut stack, &mut issues);
    issues.first().map(Issue::message)
}

// ===================================================================
// The object a text points at
// ===================================================================

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let d = b.sub(a);
    let len2 = d.x * d.x + d.y * d.y;
    if len2 < 1e-12 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0);
    p.dist(Point::new(a.x + d.x * t, a.y + d.y * t))
}

/// How near an arrow tip must be to an object to point at it, inches.
pub const REFERENCE_REACH: f64 = 6.0;

/// The arrow tip of the leader that ends at text `id` on `floor`: the far
/// end of an open polyline whose other end touches the text.
pub fn arrow_tip_of(floor: &Floor, pos: Point, height: f64, width: f64) -> Option<Point> {
    let reach = height * 2.0 + 4.0;
    let near = |p: Point| {
        p.x >= pos.x - reach
            && p.x <= pos.x + width + reach
            && p.y >= pos.y - reach
            && p.y <= pos.y + height + reach
    };
    let mut best: Option<(f64, Point)> = None;
    for o in &floor.cad {
        let CadItem::Polyline {
            points,
            closed: false,
        } = &o.item
        else {
            continue;
        };
        if points.len() < 2 {
            continue;
        }
        let (first, last) = (points[0], points[points.len() - 1]);
        // The text hangs on the last point of a leader; the tip is the first.
        for (end, tip) in [(last, first), (first, last)] {
            if near(end) && !near(tip) {
                let d = end.dist(Point::new(pos.x, pos.y));
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, tip));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// The facts of the door, window, wall or room under `tip` on `floor`, as
/// macro values (`comment`, `width`, ...). Empty when nothing is there; the
/// cabinets, fixtures and other kinds that live outside plan-core are
/// supplied by the app ([`MacroText::facts`]).
pub fn reference_facts(
    project: &Project,
    floor: usize,
    tip: Point,
    rooms: &[Room],
) -> Vec<(String, String)> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    // Openings first: they sit in walls, so they would otherwise lose.
    let mut best: Option<(f64, &crate::model::Opening)> = None;
    for o in &f.openings {
        let Some(w) = f.wall(o.wall_id) else { continue };
        let dir = w.end.sub(w.start);
        let len = dir.length();
        if len < 1e-9 {
            continue;
        }
        let u = Point::new(dir.x / len, dir.y / len);
        let a = Point::new(
            w.start.x + u.x * o.start_offset(),
            w.start.y + u.y * o.start_offset(),
        );
        let b = Point::new(
            w.start.x + u.x * o.end_offset(),
            w.start.y + u.y * o.end_offset(),
        );
        let d = seg_dist(tip, a, b);
        if d <= w.thickness * 0.5 + REFERENCE_REACH && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, o));
        }
    }
    if let Some((_, o)) = best {
        return opening_facts(project, f, o);
    }
    let mut best_wall: Option<(f64, &crate::model::Wall)> = None;
    for w in &f.walls {
        let d = seg_dist(tip, w.start, w.end);
        if d <= w.thickness * 0.5 + REFERENCE_REACH && best_wall.is_none_or(|(bd, _)| d < bd) {
            best_wall = Some((d, w));
        }
    }
    if let Some((_, w)) = best_wall {
        return wall_facts(project, w);
    }
    if let Some(r) = rooms
        .iter()
        .find(|r| point_in_polygon(tip, &r.inner_polygon) || point_in_polygon(tip, &r.polygon))
    {
        return room_facts(project, f, r);
    }
    Vec::new()
}

fn len_text(v: f64) -> String {
    fmt_label_length(v, true)
}

fn info_facts(project: &Project, key: &str, out: &mut Vec<(String, String)>) {
    let i = project.materials.info(key);
    for (n, v) in [
        ("code", i.map(|i| i.code.clone())),
        ("comment", i.map(|i| i.comment.clone())),
        ("description", i.map(|i| i.description.clone())),
        ("manufacturer", i.map(|i| i.manufacturer.clone())),
        ("supplier", i.map(|i| i.supplier.clone())),
    ] {
        out.push((n.to_string(), v.unwrap_or_default()));
    }
}

fn opening_facts(project: &Project, f: &Floor, o: &crate::model::Opening) -> Vec<(String, String)> {
    let door = o.kind == crate::model::OpeningKind::Door;
    let key = format!("{}:{}", if door { "door" } else { "window" }, o.id);
    let sched = o.schedule_number.clone().unwrap_or_default();
    let type_name = o.type_name().to_string();
    let size = size_text(
        o.width,
        o.height,
        SizeFormat::WidthHeight,
        SizeStyle::Architectural,
    );
    let mut v = vec![
        (
            "object_type".to_string(),
            if door { "Door" } else { "Window" }.to_string(),
        ),
        ("type".into(), type_name.clone()),
        ("name".into(), type_name.clone()),
        (
            "automatic_label".into(),
            opening_automatic_label(
                o.kind,
                o.style,
                o.width,
                o.height,
                SizeFormat::WidthHeight,
                false,
                None,
            ),
        ),
        (
            "nominal_size".into(),
            size_text(
                o.width,
                o.height,
                SizeFormat::WidthHeight,
                SizeStyle::Shorthand,
            ),
        ),
        (
            "automatic_description".into(),
            format!("{type_name}, {size}"),
        ),
        ("width".into(), len_text(o.width)),
        ("height".into(), len_text(o.height)),
        ("elevation".into(), len_text(o.sill_height)),
        ("schedule_number".into(), sched),
    ];
    if let Some(w) = f.wall(o.wall_id) {
        v.push(("depth".into(), len_text(w.thickness)));
    }
    info_facts(project, &key, &mut v);
    v
}

fn wall_facts(project: &Project, w: &crate::model::Wall) -> Vec<(String, String)> {
    let name = w.wall_type.clone().unwrap_or_else(|| {
        match w.kind {
            crate::model::WallKind::Exterior => "Exterior Wall",
            crate::model::WallKind::Interior => "Interior Wall",
        }
        .to_string()
    });
    let length = w.end.sub(w.start).length();
    let mut v = vec![
        ("object_type".to_string(), "Wall".to_string()),
        ("type".into(), name.clone()),
        ("name".into(), name.clone()),
        (
            "automatic_label".into(),
            w.extras.label_text.clone().unwrap_or_else(|| name.clone()),
        ),
        (
            "automatic_description".into(),
            format!("{name}, {} long", len_text(length)),
        ),
        ("length".into(), len_text(length)),
        ("depth".into(), len_text(w.thickness)),
        ("height".into(), len_text(w.height)),
    ];
    info_facts(project, &format!("wall:{}", w.id), &mut v);
    v
}

fn room_facts(project: &Project, f: &Floor, r: &Room) -> Vec<(String, String)> {
    let entry = r.name_entry(&f.room_names);
    let name = entry
        .map(|e| e.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| r.label.clone());
    let mut v = vec![
        ("object_type".to_string(), "Room".to_string()),
        (
            "type".into(),
            entry.map(|e| e.room_type.clone()).unwrap_or_default(),
        ),
        ("name".into(), name.clone()),
        ("automatic_label".into(), name.clone()),
        (
            "automatic_description".into(),
            format!("{name}, {} sq ft", r.interior_area_sq_ft().round()),
        ),
    ];
    info_facts(project, "room", &mut v);
    v
}

/// Reference facts for an object whose label facts the app knows (cabinets,
/// fixtures, devices...): the same names, from a `LabelFacts`.
pub fn facts_from_label(
    object_type: &str,
    facts: &crate::object_pages::LabelFacts,
) -> Vec<(String, String)> {
    let len = |v: Option<f64>| v.map(len_text).unwrap_or_default();
    vec![
        ("object_type".to_string(), object_type.to_string()),
        ("type".into(), facts.type_name.clone()),
        ("name".into(), facts.name.clone()),
        ("automatic_label".into(), facts.automatic.clone()),
        (
            "automatic_description".into(),
            if facts.type_name.is_empty() {
                facts.automatic.clone()
            } else {
                format!("{}, {}", facts.type_name, facts.automatic)
            },
        ),
        ("nominal_size".into(), facts.automatic.clone()),
        ("width".into(), len(facts.width)),
        ("height".into(), len(facts.height)),
        ("depth".into(), len(facts.depth)),
        ("length".into(), len(facts.length)),
        ("elevation".into(), len(facts.elevation)),
        ("schedule_number".into(), facts.schedule_number.clone()),
        ("code".into(), facts.code.clone()),
        ("comment".into(), facts.comment.clone()),
        ("description".into(), facts.description.clone()),
        ("manufacturer".into(), facts.manufacturer.clone()),
        ("supplier".into(), facts.supplier.clone()),
    ]
}

// ===================================================================
// Live texts
// ===================================================================

/// A Text or Rich Text whose macros are evaluated again as the plan changes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MacroText {
    /// The CAD text object.
    pub id: Id,
    /// What was typed, macros included; one plain run for simple Text.
    pub runs: Vec<RichRun>,
    /// Rich Text: the evaluated runs go to the text's `CadAttrs::runs`.
    pub rich: bool,
    /// The text the object had after the last evaluation; a different text
    /// means someone edited the object, which ends the link.
    pub last: String,
    /// Facts of the pointed-at object supplied by the app when it is of a
    /// kind plan-core cannot look up (cabinets, fixtures...).
    pub facts: Vec<(String, String)>,
}

impl MacroText {
    /// The text as typed, macros included.
    pub fn source(&self) -> String {
        runs_plain(&self.runs)
    }
}

/// The plan's live texts (`Project::macro_texts`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MacroTexts {
    pub texts: Vec<MacroText>,
}

impl MacroTexts {
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    pub fn get(&self, id: Id) -> Option<&MacroText> {
        self.texts.iter().find(|t| t.id == id)
    }
}

fn text_item(f: &Floor, id: Id) -> Option<(Point, f64, String)> {
    f.cad
        .iter()
        .find(|o| o.id == id)
        .and_then(|o| match &o.item {
            CadItem::Text {
                pos, height, text, ..
            } => Some((*pos, *height, text.clone())),
            _ => None,
        })
}

impl Project {
    /// The source of the live text `id`, if it is one.
    pub fn live_text(&self, id: Id) -> Option<&MacroText> {
        self.macro_texts.get(id)
    }

    fn eval_runs(env: &Env, runs: &[RichRun]) -> Vec<RichRun> {
        runs.iter()
            .map(|r| RichRun {
                text: env.expand(&r.text),
                ..r.clone()
            })
            .collect()
    }

    /// Makes text `id` of `floor` live with the source `runs` (or turns it
    /// into plain text when the source holds no macro). The CAD item and,
    /// for Rich Text, its runs are brought to what the macros give. Returns
    /// whether the text is live now.
    pub fn set_live_text(
        &mut self,
        floor: usize,
        id: Id,
        runs: Vec<RichRun>,
        rich: bool,
        facts: Vec<(String, String)>,
    ) -> bool {
        let rooms = self
            .floors
            .get(floor)
            .map(|f| detect_rooms(&f.walls, 0.5))
            .unwrap_or_default();
        self.macro_texts.texts.retain(|t| t.id != id);
        let Some((pos, height, _)) = self.floors.get(floor).and_then(|f| text_item(f, id)) else {
            return false;
        };
        let source = runs_plain(&runs);
        let env0 = Env::new(
            self,
            floor,
            Some(centre_of(pos, height, &source)),
            &rooms,
            unix_now(),
        );
        if !env0.has_macros(&source) {
            return false;
        }
        let mut entry = MacroText {
            id,
            runs,
            rich,
            last: String::new(),
            facts,
        };
        self.apply_live(floor, &mut entry, &rooms, unix_now());
        self.macro_texts.texts.push(entry);
        true
    }

    /// Writes the evaluated text of `e` to its CAD object; returns whether
    /// anything changed.
    fn apply_live(&mut self, floor: usize, e: &mut MacroText, rooms: &[Room], now: i64) -> bool {
        let Some((pos, height, current)) = self.floors.get(floor).and_then(|f| text_item(f, e.id))
        else {
            return false;
        };
        let source = runs_plain(&e.runs);
        let at = centre_of(pos, height, &current);
        let f = &self.floors[floor];
        let width = current.chars().count() as f64 * height * TEXT_WIDTH_FACTOR;
        let mut env = Env::new(self, floor, Some(at), rooms, now);
        let facts = match arrow_tip_of(f, pos, height, width) {
            Some(tip) => {
                let core = reference_facts(self, floor, tip, rooms);
                if core.is_empty() {
                    e.facts.clone()
                } else {
                    core
                }
            }
            None => e.facts.clone(),
        };
        env = env.with_values(&facts);
        let _ = source;
        let runs = Self::eval_runs(&env, &e.runs);
        let plain = runs_plain(&runs);
        let mut changed = false;
        if plain != current {
            if let Some(o) = self.floors[floor].cad.iter_mut().find(|o| o.id == e.id) {
                if let CadItem::Text { text, .. } = &mut o.item {
                    *text = plain.clone();
                    changed = true;
                }
            }
        }
        if e.rich {
            let merged = merge_runs(runs);
            let now_runs = self.floors[floor]
                .cad_attrs(e.id)
                .map(|a| a.runs)
                .unwrap_or_default();
            if merged != now_runs {
                self.edit_cad_attrs(floor, e.id, |a| a.runs = merged);
                changed = true;
            }
        }
        e.last = plain;
        changed
    }

    /// Evaluates every live text again. A text whose CAD object is gone or
    /// was edited by someone else is no longer live. Returns whether any
    /// object changed.
    pub fn sync_macro_texts(&mut self) -> bool {
        if self.macro_texts.texts.is_empty() {
            return false;
        }
        let now = unix_now();
        let entries = std::mem::take(&mut self.macro_texts.texts);
        let mut kept = Vec::with_capacity(entries.len());
        let mut changed = false;
        let mut rooms: HashMap<usize, Vec<Room>> = HashMap::new();
        for mut e in entries {
            let Some(fi) = self
                .floors
                .iter()
                .position(|f| text_item(f, e.id).is_some())
            else {
                continue;
            };
            let current = text_item(&self.floors[fi], e.id)
                .map(|(_, _, t)| t)
                .unwrap_or_default();
            if !e.last.is_empty() && current != e.last {
                continue;
            }
            let r = rooms
                .entry(fi)
                .or_insert_with(|| detect_rooms(&self.floors[fi].walls, 0.5))
                .clone();
            changed |= self.apply_live(fi, &mut e, &r, now);
            kept.push(e);
        }
        self.macro_texts.texts = kept;
        changed
    }
}

fn centre_of(pos: Point, height: f64, text: &str) -> Point {
    let w = text.chars().count() as f64 * height * TEXT_WIDTH_FACTOR;
    Point::new(pos.x + w * 0.5, pos.y + height * 0.5)
}

// ===================================================================
// Import and export of user macros
// ===================================================================

/// What to do with an imported macro whose name the plan already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Keep both: the imported one gets this name.
    Rename(String),
    /// Leave the plan's macro as it is.
    Discard,
    /// Replace the plan's macro with the imported one.
    Replace,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportReport {
    pub added: Vec<String>,
    pub renamed: Vec<(String, String)>,
    pub replaced: Vec<String>,
    pub discarded: Vec<String>,
    /// Names the plan cannot take (built-in names, bad characters).
    pub rejected: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Pack {
    format: String,
    macros: Vec<TextMacro>,
}

const PACK_FORMAT: &str = "plan-studio-text-macros-1";

/// The macros named `names` (all when empty) as a file that another plan,
/// layout or library can import.
pub fn export_macros(macros: &TextMacros, names: &[String]) -> String {
    let pack = Pack {
        format: PACK_FORMAT.to_string(),
        macros: macros
            .macros
            .iter()
            .filter(|m| names.is_empty() || names.contains(&m.name))
            .cloned()
            .collect(),
    };
    serde_json::to_string_pretty(&pack).unwrap_or_default()
}

/// Reads an exported file.
pub fn parse_macro_pack(json: &str) -> Result<Vec<TextMacro>, String> {
    let pack: Pack = serde_json::from_str(json).map_err(|e| format!("Not a macro file: {e}"))?;
    if pack.format != PACK_FORMAT {
        return Err("Not a Plan Studio text macro file".into());
    }
    Ok(pack.macros)
}

/// The imported macros whose names the plan already uses.
pub fn conflicts(existing: &TextMacros, incoming: &[TextMacro]) -> Vec<String> {
    incoming
        .iter()
        .filter(|m| existing.get(&m.name).is_some())
        .map(|m| m.name.clone())
        .collect()
}

/// A name free in `existing`: `name_2`, `name_3`...
pub fn free_name(existing: &TextMacros, name: &str) -> String {
    let mut n = 2;
    loop {
        let c = format!("{name}_{n}");
        if existing.get(&c).is_none() && !is_reserved(&c) {
            return c;
        }
        n += 1;
    }
}

/// Imports `incoming` into `existing`; `resolve` answers for each name that
/// is already there.
pub fn import_macros(
    existing: &mut TextMacros,
    incoming: Vec<TextMacro>,
    resolve: &mut dyn FnMut(&TextMacro) -> Resolution,
) -> ImportReport {
    let mut report = ImportReport::default();
    for m in incoming {
        if !is_macro_name(&m.name) || is_reserved(&m.name) {
            report.rejected.push(m.name);
            continue;
        }
        if existing.get(&m.name).is_none() {
            report.added.push(m.name.clone());
            existing.macros.push(m);
            continue;
        }
        match resolve(&m) {
            Resolution::Discard => report.discarded.push(m.name),
            Resolution::Replace => {
                if let Some(slot) = existing.macros.iter_mut().find(|x| x.name == m.name) {
                    slot.text = m.text;
                }
                report.replaced.push(m.name);
            }
            Resolution::Rename(to) => {
                let to = if existing.get(&to).is_some() || !is_macro_name(&to) || is_reserved(&to) {
                    free_name(existing, &m.name)
                } else {
                    to
                };
                report.renamed.push((m.name.clone(), to.clone()));
                existing.macros.push(TextMacro {
                    name: to,
                    text: m.text,
                });
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_styles::RichRun;
    use crate::{OpeningKind, WallKind};

    /// 2026-10-09 15:45:09 UTC.
    const NOW: i64 = 1_791_560_709;

    fn house() -> Project {
        let mut p = Project::new("Smith Residence");
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 180.0), (0.0, 180.0)];
        for i in 0..4 {
            let a = pts[i];
            let b = pts[(i + 1) % 4];
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p
    }

    fn text(p: &mut Project, at: Point, s: &str) -> Id {
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Text {
                pos: at,
                text: s.into(),
                height: 6.0,
                angle: 0.0,
            },
        )
    }

    #[test]
    fn names_are_slugs_of_owner_and_field() {
        assert_eq!(slug("Drawn By"), "drawn_by");
        assert_eq!(owner_macro_name("Client", "Phone"), "client.phone");
        assert_eq!(
            owner_macro_name("Builder", "License No."),
            "builder.license_no"
        );
        assert!(is_macro_name("date.short") && !is_macro_name("a b") && !is_macro_name(""));
        let t = tokens("50% of %plan.name% and %x% 20%");
        assert_eq!(
            t.iter().map(|(_, _, n)| n.as_str()).collect::<Vec<_>>(),
            ["plan.name", "x"]
        );
    }

    #[test]
    fn global_macros_give_the_file_the_date_and_special_characters() {
        let p = house();
        let env = Env::new(&p, 0, None, &[], NOW);
        assert_eq!(
            env.expand("%plan.name% %floor% %floor.number%/%floor.count% on %date.short%, %date.long% (%date.iso%) %time.short%"),
            "Smith Residence 1st Floor 1/1 on 10/9/2026, October 9, 2026 (2026-10-09) 3:45 PM"
        );
        assert_eq!(env.expand("%date.weekday%"), "Friday");
        assert_eq!(
            env.expand("45%char.degree% %char.plusminus%1/8%char.inches%"),
            "45\u{b0} \u{b1}1/8\""
        );
        assert_eq!(env.expand("%nope% 50%"), "%nope% 50%");
    }

    #[test]
    fn project_information_owners_are_macros() {
        let mut p = house();
        p.info.client_name = "Jane Smith".into();
        p.info.client_address = vec!["12 Elm St".into(), "Atlanta GA".into()];
        p.info.project_number = "26-041".into();
        let mut owners = MacroOwners::from_info(&p.info);
        let mut info = p.info.clone();
        owners.set_value(&mut info, "Project", "Lot", "Lot 7");
        owners.add_owner("Builder").unwrap();
        owners
            .add_field(&mut info, "Builder", "License No")
            .unwrap();
        owners.set_value(&mut info, "Builder", "License No", "GA-5521");
        // The Project owner's own extra fields are the information's custom
        // fields, and reach layouts as %custom.<name>%.
        owners.add_field(&mut info, "Project", "Zoning").unwrap();
        owners.set_value(&mut info, "Project", "Zoning", "R-2");
        owners.store(&mut info);
        p.info = info;
        assert!(p
            .info
            .custom
            .iter()
            .any(|(k, v)| k == "Zoning" && v == "R-2"));
        assert_eq!(MacroOwners::from_info(&p.info), owners);
        let env = Env::new(&p, 0, None, &[], NOW);
        assert_eq!(
            env.expand("%client.name%, %client.address% / %project.number% %project.lot% %project.zoning% / %builder.license_no%"),
            "Jane Smith, 12 Elm St, Atlanta GA / 26-041 Lot 7 R-2 / GA-5521"
        );
        // The title-block names still work in plan text.
        assert_eq!(env.expand("%client% %project.number%"), "Jane Smith 26-041");
    }

    #[test]
    fn owners_follow_the_dialog_rules() {
        let mut info = ProjectInfo::default();
        let mut o = MacroOwners::default();
        let names: Vec<String> = o.views(&info).iter().map(|v| v.name.clone()).collect();
        assert_eq!(names, ["Project", "Designer", "Client"]);
        assert!(o.add_owner("project").is_err());
        assert!(o.add_owner("  ").is_err());
        assert_eq!(o.add_owner("Builder").unwrap(), "Builder");
        assert!(o.add_owner("builder").is_err());
        assert!(o.add_field(&mut info, "Builder", "Phone").is_ok());
        assert!(o.add_field(&mut info, "Client", "Phone").is_err());
        assert!(o.add_field(&mut info, "Client", "Fax").is_ok());
        assert!(!o.delete_field(&mut info, "Client", "Phone"));
        assert!(o.delete_field(&mut info, "Client", "Fax"));
        // The Project owner's fields live in the information; renaming one
        // may not take another's name.
        assert!(o.add_field(&mut info, "Project", "Zoning").is_ok());
        assert!(o.add_field(&mut info, "Project", "zoning").is_err());
        assert!(o.add_field(&mut info, "Project", "Permit").is_ok());
        assert!(o
            .rename_field(&mut info, "Project", "Zoning", "Permit")
            .is_err());
        assert!(o
            .rename_field(&mut info, "Project", "Zoning", "Zone")
            .is_ok());
        assert!(o
            .rename_field(&mut info, "Project", "Number", "Num")
            .is_err());
        assert_eq!(o.value(&info, "Project", "Zone"), "");
        assert!(o.delete_field(&mut info, "Project", "Zone"));
        assert!(o.delete_field(&mut info, "Project", "Permit"));
        assert!(info.custom.is_empty());
        assert!(o.rename_owner("Client", "X").is_err());
        assert!(o.rename_owner("Builder", "GC").is_ok());
        o.set_value(&mut info, "GC", "Phone", "555-0100");
        let dup = o.duplicate_owner(&info, "GC").unwrap();
        assert_eq!(dup, "GC 2");
        assert_eq!(o.value(&info, "GC 2", "Phone"), "555-0100");
        o.set_value(&mut info, "Client", "Name", "Jane");
        o.clear_values(&mut info, "Client");
        assert_eq!(info.client_name, "");
        o.clear_values(&mut info, "GC");
        assert_eq!(o.value(&info, "GC", "Phone"), "");
        assert!(!o.delete_owner("Client") && o.delete_owner("GC 2"));
        assert!(o.views(&info).iter().all(|v| v.name != "GC 2"));
    }

    #[test]
    fn user_macros_nest_and_report_loops() {
        let mut p = house();
        p.text_macros.add("firm", "Daniel Allen Designs");
        p.text_macros.add("stamp", "%firm% for %plan.name%");
        p.text_macros.add("a", "%b%");
        p.text_macros.add("b", "%a%");
        p.text_macros.add("bad", "%missing%");
        let env = Env::new(&p, 0, None, &[], NOW);
        assert_eq!(
            env.expand("%stamp%"),
            "Daniel Allen Designs for Smith Residence"
        );
        let e = env.evaluate("%a%");
        assert!(e.issues.iter().any(|i| i.kind == IssueKind::Loop));
        assert_eq!(e.text, "%a%");
        assert!(macro_error(&p, "bad").unwrap().contains("%missing%"));
        assert!(macro_error(&p, "a").unwrap().contains("uses itself"));
        assert!(macro_error(&p, "stamp").is_none());
        // Built-in names cannot be taken.
        assert!(!p.text_macros.add("date.short", "x"));
        assert!(!p.text_macros.add("client.name", "x"));
    }

    #[test]
    fn the_room_macros_answer_for_the_room_the_text_is_in() {
        let mut p = house();
        p.floors[0].room_names.push(crate::model::RoomName {
            anchor: Point::new(120.0, 90.0),
            name: "Great Room".into(),
            room_type: "Living Room".into(),
            ..Default::default()
        });
        let rooms = detect_rooms(&p.floors[0].walls, 0.5);
        assert!(!rooms.is_empty());
        let env = Env::new(&p, 0, Some(Point::new(100.0, 80.0)), &rooms, NOW);
        assert_eq!(
            env.expand("%room.name%|%room.nvp_name%|%room.type%"),
            "Great Room|Great Room|Living Room"
        );
        assert!(env.expand("%room.area%").ends_with("sq ft"));
        let out = Env::new(&p, 0, Some(Point::new(900.0, 900.0)), &rooms, NOW);
        assert_eq!(out.expand("[%room.name%]"), "[]");
    }

    #[test]
    fn a_live_text_follows_the_room_name_and_stops_when_edited_elsewhere() {
        let mut p = house();
        p.floors[0].room_names.push(crate::model::RoomName {
            anchor: Point::new(120.0, 90.0),
            name: "Den".into(),
            room_type: "Living Room".into(),
            ..Default::default()
        });
        let id = text(&mut p, Point::new(80.0, 80.0), "Den");
        let live = p.set_live_text(
            0,
            id,
            vec![RichRun::plain("%room.name% - %floor%")],
            false,
            Vec::new(),
        );
        assert!(live);
        let shown = |p: &Project| match &p.floors[0].cad.iter().find(|o| o.id == id).unwrap().item {
            CadItem::Text { text, .. } => text.clone(),
            _ => unreachable!(),
        };
        assert_eq!(shown(&p), "Den - 1st Floor");
        assert_eq!(p.live_text(id).unwrap().source(), "%room.name% - %floor%");
        // Renaming the room changes the text.
        p.floors[0].room_names[0].name = "Study".into();
        assert!(p.sync_macro_texts());
        assert_eq!(shown(&p), "Study - 1st Floor");
        // Nothing to do the second time.
        assert!(!p.sync_macro_texts());
        // Someone else edits the text: the link ends and the edit stands.
        if let Some(o) = p.floors[0].cad.iter_mut().find(|o| o.id == id) {
            if let CadItem::Text { text, .. } = &mut o.item {
                *text = "Library".into();
            }
        }
        p.floors[0].room_names[0].name = "Other".into();
        p.sync_macro_texts();
        assert_eq!(shown(&p), "Library");
        assert!(p.live_text(id).is_none());
        // Text without a macro is not live.
        let plain = text(&mut p, Point::new(10.0, 10.0), "Hall");
        assert!(!p.set_live_text(0, plain, vec![RichRun::plain("Hall")], false, Vec::new()));
    }

    #[test]
    fn rich_live_text_keeps_its_formats() {
        let mut p = house();
        let id = text(&mut p, Point::new(10.0, 10.0), "x");
        let runs = vec![RichRun::bold("Plan "), RichRun::plain("%plan.name%")];
        assert!(p.set_live_text(0, id, runs, true, Vec::new()));
        let got = p.floors[0].cad_attrs(id).unwrap().runs;
        assert_eq!(got.len(), 2);
        assert!(got[0].bold && got[0].text == "Plan ");
        assert_eq!(got[1].text, "Smith Residence");
        p.name = "Jones".into();
        p.sync_macro_texts();
        assert_eq!(p.floors[0].cad_attrs(id).unwrap().runs[1].text, "Jones");
    }

    #[test]
    fn a_text_with_an_arrow_reads_the_door_it_points_at() {
        let mut p = house();
        let wall = p.floors[0].walls[0].id;
        let door = p.add_opening(0, wall, 120.0, OpeningKind::Door).unwrap();
        p.materials.info_mut(&format!("door:{door}")).comment = "Solid core".into();
        p.materials.info_mut(&format!("door:{door}")).description = "Entry door".into();
        // A leader from the text at (120, -60) to the door at (120, 0).
        p.add_cad(
            0,
            "CAD, Default",
            CadItem::Polyline {
                points: vec![Point::new(120.0, 0.0), Point::new(120.0, -40.0)],
                closed: false,
            },
        );
        let id = text(&mut p, Point::new(118.0, -46.0), "t");
        assert!(p.set_live_text(
            0,
            id,
            vec![RichRun::plain(
                "%object_type%: %automatic_label% (%nominal_size%) %description% / %comment%"
            )],
            false,
            Vec::new()
        ));
        let shown = match &p.floors[0].cad.iter().find(|o| o.id == id).unwrap().item {
            CadItem::Text { text, .. } => text.clone(),
            _ => unreachable!(),
        };
        assert!(shown.starts_with("Door: "), "{shown}");
        assert!(shown.contains("Entry door / Solid core"), "{shown}");
        assert!(shown.contains("(3068)"), "{shown}");
        // A text with no arrow leaves the reference macros as typed.
        let free = text(&mut p, Point::new(400.0, 400.0), "t");
        p.set_live_text(
            0,
            free,
            vec![RichRun::plain("%comment% %plan.name%")],
            false,
            Vec::new(),
        );
        let shown = match &p.floors[0].cad.iter().find(|o| o.id == free).unwrap().item {
            CadItem::Text { text, .. } => text.clone(),
            _ => unreachable!(),
        };
        assert_eq!(shown, "%comment% Smith Residence");
    }

    #[test]
    fn the_insert_menu_has_every_category() {
        let mut p = house();
        p.text_macros.add("firm", "DAD");
        let m = insert_menu(&p);
        let top: Vec<&str> = m.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            top,
            [
                "Global",
                "User Defined",
                "Referenced Object",
                "Callout, Marker and Note"
            ]
        );
        let all: Vec<String> = m.iter().flat_map(MenuItem::inserts).collect();
        for want in [
            "%date.short%",
            "%client.name%",
            "%room.name%",
            "%firm%",
            "%comment%",
            "%automatic_description%",
            "%char.degree%",
            "%floor%",
        ] {
            assert!(all.iter().any(|a| a == want), "{want} missing");
        }
        // Every inserted macro is either known to a bare environment, an owner
        // pair, the plan's, or a reference/annotation macro.
        let env = Env::new(&p, 0, None, &[], NOW);
        for a in &all {
            let n = a.trim_matches('%');
            assert!(
                env.knows(n) || ANNOTATION_MACROS.iter().any(|(x, _)| *x == n),
                "{n} unknown"
            );
        }
    }

    #[test]
    fn macros_export_and_import_with_the_conflict_choices() {
        let mut a = TextMacros::default();
        a.add("firm", "DAD");
        a.add("stamp", "Drawn by %firm%");
        let json = export_macros(&a, &[]);
        let one = export_macros(&a, &["firm".to_string()]);
        assert_eq!(parse_macro_pack(&one).unwrap().len(), 1);
        assert!(parse_macro_pack("{}").is_err());
        let mut b = TextMacros::default();
        b.add("firm", "Other Firm");
        let incoming = parse_macro_pack(&json).unwrap();
        assert_eq!(conflicts(&b, &incoming), ["firm"]);
        let mut seen = 0;
        let report = import_macros(&mut b, incoming.clone(), &mut |_| {
            seen += 1;
            Resolution::Rename("firm_dad".into())
        });
        assert_eq!(seen, 1);
        assert_eq!(report.added, ["stamp"]);
        assert_eq!(
            report.renamed,
            [("firm".to_string(), "firm_dad".to_string())]
        );
        assert_eq!(b.get("firm").unwrap().text, "Other Firm");
        assert_eq!(b.get("firm_dad").unwrap().text, "DAD");
        let mut c = TextMacros::default();
        c.add("firm", "Old");
        let r = import_macros(&mut c, incoming.clone(), &mut |_| Resolution::Replace);
        assert_eq!(r.replaced, ["firm"]);
        assert_eq!(c.get("firm").unwrap().text, "DAD");
        let mut d = TextMacros::default();
        d.add("firm", "Old");
        let r = import_macros(&mut d, incoming, &mut |_| Resolution::Discard);
        assert_eq!(r.discarded, ["firm"]);
        assert_eq!(d.get("firm").unwrap().text, "Old");
        // A reserved name is rejected.
        let bad = vec![TextMacro {
            name: "date.short".into(),
            text: "x".into(),
        }];
        let mut e = TextMacros::default();
        assert_eq!(
            import_macros(&mut e, bad, &mut |_| Resolution::Discard).rejected,
            ["date.short"]
        );
    }
}
