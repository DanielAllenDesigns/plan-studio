//! User-defined properties (Tools > Property Manager).
//!
//! Chief has only the fixed Object Information fields; ArchiCAD lets a firm
//! add its own properties and exchange them with Excel. This is the model
//! behind that: a project-level list of [`PropDef`]s (name, type, default,
//! "show in schedule") per kind of object, and the values stored per object.
//!
//! # Storage
//!
//! Everything lives in one typed slot, `Project.props` ([`PropTable`]), so a
//! snapshot undo step restores definitions and values together. The values
//! are a `BTreeMap<String, PropValue>` per object, found by the object's
//! [`PropKey`] string (`door:12`, `cabinet:7`, ...). Keeping them beside the
//! objects rather than inside each object type means every kind of object,
//! including the ones the plan stores as opaque JSON (cabinets, stairs,
//! devices, roof planes, framing), takes custom properties the same way.
//!
//! An object with no stored value for a property shows the property's
//! default. Storing a value equal to the default removes the entry.
//!
//! # Keys
//!
//! Ids are unique across the project for walls, doors and windows, cabinets,
//! symbols, stairs, roof records and framing records, so their keys carry no
//! floor.
//! Electrical devices number from 1 on every floor, and rooms have no id at
//! all, so those keys carry the floor index; a room is found by the rounded
//! position of its centre.

use crate::geometry::Point;
use crate::model::Id;
use crate::units::{fmt_ft_in, parse_ft_in};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of object that take custom properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PropKind {
    Door,
    Window,
    Cabinet,
    Room,
    Wall,
    /// Fixtures, furniture and plants (the placed library symbols).
    Symbol,
    Electrical,
    Stair,
    RoofPlane,
    Framing,
}

impl PropKind {
    pub const ALL: [PropKind; 10] = [
        PropKind::Door,
        PropKind::Window,
        PropKind::Cabinet,
        PropKind::Room,
        PropKind::Wall,
        PropKind::Symbol,
        PropKind::Electrical,
        PropKind::Stair,
        PropKind::RoofPlane,
        PropKind::Framing,
    ];

    /// The name the Property Manager shows.
    pub fn name(self) -> &'static str {
        match self {
            PropKind::Door => "Door",
            PropKind::Window => "Window",
            PropKind::Cabinet => "Cabinet",
            PropKind::Room => "Room",
            PropKind::Wall => "Wall",
            PropKind::Symbol => "Fixture / Symbol",
            PropKind::Electrical => "Electrical",
            PropKind::Stair => "Stair",
            PropKind::RoofPlane => "Roof Plane",
            PropKind::Framing => "Framing",
        }
    }
}

/// What a property holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PropType {
    #[default]
    Text,
    Number,
    /// A length, stored in inches and shown as feet-inches.
    Length,
    Bool,
    /// One of the definition's `options`.
    List,
}

impl PropType {
    pub const ALL: [PropType; 5] = [
        PropType::Text,
        PropType::Number,
        PropType::Length,
        PropType::Bool,
        PropType::List,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PropType::Text => "Text",
            PropType::Number => "Number",
            PropType::Length => "Length",
            PropType::Bool => "Yes / No",
            PropType::List => "List",
        }
    }
}

/// A stored value. A length is a [`PropValue::Number`] in inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropValue {
    Text(String),
    Number(f64),
    Bool(bool),
}

/// One user-defined property of one kind of object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropDef {
    pub kind: PropKind,
    pub name: String,
    pub ty: PropType,
    /// The value an object shows until one is set, as typed text (parsed
    /// with the property's type); empty is "no value".
    pub default: String,
    /// The choices of a [`PropType::List`].
    pub options: Vec<String>,
    /// Shows as a column in every schedule of this kind of object, unless
    /// the schedule already has (or has hidden) the column.
    pub show_in_schedule: bool,
}

impl Default for PropDef {
    fn default() -> Self {
        Self {
            kind: PropKind::Door,
            name: String::new(),
            ty: PropType::Text,
            default: String::new(),
            options: Vec::new(),
            show_in_schedule: false,
        }
    }
}

/// The prefix of a schedule column id that names a custom property.
pub const COLUMN_PREFIX: &str = "prop:";

impl PropDef {
    pub fn new(kind: PropKind, name: &str, ty: PropType) -> Self {
        Self {
            kind,
            name: name.trim().to_string(),
            ty,
            ..Self::default()
        }
    }

    /// The schedule column id of the property (`prop:Fire Rating`).
    pub fn column_id(&self) -> String {
        format!("{COLUMN_PREFIX}{}", self.name)
    }

    /// Reads typed or imported `text` as a value of this property's type.
    /// `Ok(None)` is an empty cell (no value); `Err` says why the text is not
    /// a valid value.
    pub fn parse(&self, text: &str) -> Result<Option<PropValue>, String> {
        let t = text.trim();
        if t.is_empty() {
            return Ok(None);
        }
        match self.ty {
            PropType::Text => Ok(Some(PropValue::Text(text.trim().to_string()))),
            PropType::Number => t
                .replace(',', "")
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| Some(PropValue::Number(v)))
                .ok_or_else(|| format!("\"{t}\" is not a number")),
            PropType::Length => parse_ft_in(t)
                .filter(|v| v.is_finite())
                .map(|v| Some(PropValue::Number(v)))
                .ok_or_else(|| format!("\"{t}\" is not a length (try 3'-6\")")),
            PropType::Bool => match t.to_lowercase().as_str() {
                "yes" | "y" | "true" | "t" | "1" | "x" | "on" => Ok(Some(PropValue::Bool(true))),
                "no" | "n" | "false" | "f" | "0" | "off" => Ok(Some(PropValue::Bool(false))),
                _ => Err(format!("\"{t}\" is not Yes or No")),
            },
            PropType::List => self
                .options
                .iter()
                .find(|o| o.trim().eq_ignore_ascii_case(t))
                .map(|o| Some(PropValue::Text(o.trim().to_string())))
                .ok_or_else(|| format!("\"{t}\" is not one of: {}", self.options.join(", "))),
        }
    }

    /// The text a value shows as (a schedule cell, a spreadsheet cell, the
    /// Properties tab).
    pub fn format(&self, v: &PropValue) -> String {
        match (self.ty, v) {
            (_, PropValue::Text(s)) => s.clone(),
            (PropType::Length, PropValue::Number(n)) => fmt_ft_in(*n),
            (_, PropValue::Number(n)) => fmt_number(*n),
            (_, PropValue::Bool(b)) => if *b { "Yes" } else { "No" }.to_string(),
        }
    }

    /// The default value, when it has one (an unparsable default is none).
    pub fn default_value(&self) -> Option<PropValue> {
        self.parse(&self.default).ok().flatten()
    }
}

/// A number without a trailing `.0` (and at most 6 decimals).
pub fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let s = format!("{n:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The values of one object, by property name.
pub type PropMap = BTreeMap<String, PropValue>;

/// Who owns a set of values: `door:12`, `device:0:4`, `room:0:120,84`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropKey(pub String);

impl PropKey {
    pub fn wall(id: Id) -> Self {
        Self(format!("wall:{id}"))
    }

    pub fn door(id: Id) -> Self {
        Self(format!("door:{id}"))
    }

    pub fn window(id: Id) -> Self {
        Self(format!("window:{id}"))
    }

    /// The key of a door or a window by its opening kind.
    pub fn opening(id: Id, is_door: bool) -> Self {
        if is_door {
            Self::door(id)
        } else {
            Self::window(id)
        }
    }

    pub fn cabinet(id: Id) -> Self {
        Self(format!("cabinet:{id}"))
    }

    pub fn symbol(id: Id) -> Self {
        Self(format!("symbol:{id}"))
    }

    pub fn stair(id: Id) -> Self {
        Self(format!("stair:{id}"))
    }

    pub fn roof(id: Id) -> Self {
        Self(format!("roof:{id}"))
    }

    pub fn framing(id: Id) -> Self {
        Self(format!("framing:{id}"))
    }

    /// Electrical device ids restart on every floor.
    pub fn device(floor: usize, id: Id) -> Self {
        Self(format!("device:{floor}:{id}"))
    }

    /// A room, by floor and the centre of the room rounded to whole inches.
    pub fn room(floor: usize, centre: Point) -> Self {
        Self(format!(
            "room:{floor}:{},{}",
            centre.x.round() as i64,
            centre.y.round() as i64
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part of the key before the first `:`.
    pub fn prefix(&self) -> &str {
        self.0.split(':').next().unwrap_or("")
    }

    /// Does this key belong to an object of `kind`?
    pub fn fits(&self, kind: PropKind) -> bool {
        matches!(
            (self.prefix(), kind),
            ("wall", PropKind::Wall)
                | ("door", PropKind::Door)
                | ("window", PropKind::Window)
                | ("cabinet", PropKind::Cabinet)
                | ("symbol", PropKind::Symbol)
                | ("stair", PropKind::Stair)
                | ("roof", PropKind::RoofPlane)
                | ("framing", PropKind::Framing)
                | ("device", PropKind::Electrical)
                | ("room", PropKind::Room)
        )
    }
}

/// `Project.props`: the definitions and every object's values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropTable {
    pub defs: Vec<PropDef>,
    /// Values by [`PropKey`] string.
    pub values: BTreeMap<String, PropMap>,
}

impl PropTable {
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty() && self.values.is_empty()
    }

    /// The definitions of `kind`, in the order they were made.
    pub fn defs_for(&self, kind: PropKind) -> impl Iterator<Item = &PropDef> {
        self.defs.iter().filter(move |d| d.kind == kind)
    }

    pub fn def(&self, kind: PropKind, name: &str) -> Option<&PropDef> {
        self.defs
            .iter()
            .find(|d| d.kind == kind && d.name.eq_ignore_ascii_case(name.trim()))
    }

    /// Adds a definition. Names are unique per kind, ignoring case, and may
    /// not be empty. Returns why not.
    pub fn add_def(&mut self, def: PropDef) -> Result<(), String> {
        let mut def = def;
        def.name = def.name.trim().to_string();
        if def.name.is_empty() {
            return Err("A property needs a name".into());
        }
        if self.def(def.kind, &def.name).is_some() {
            return Err(format!(
                "{} already has a property named \"{}\"",
                def.kind.name(),
                def.name
            ));
        }
        if def.ty == PropType::List && def.options.iter().all(|o| o.trim().is_empty()) {
            return Err("A list property needs at least one choice".into());
        }
        def.options.retain(|o| !o.trim().is_empty());
        self.defs.push(def);
        Ok(())
    }

    /// Replaces the definition `kind`/`old_name` with `def`, carrying the
    /// stored values over to a new name. A value that no longer fits the new
    /// type is dropped. Returns why not, or how many values were dropped.
    pub fn replace_def(
        &mut self,
        kind: PropKind,
        old_name: &str,
        def: PropDef,
    ) -> Result<usize, String> {
        let mut def = def;
        def.kind = kind;
        def.name = def.name.trim().to_string();
        def.options.retain(|o| !o.trim().is_empty());
        if def.name.is_empty() {
            return Err("A property needs a name".into());
        }
        let Some(i) = self
            .defs
            .iter()
            .position(|d| d.kind == kind && d.name.eq_ignore_ascii_case(old_name))
        else {
            return Err(format!("There is no property \"{old_name}\""));
        };
        if self
            .defs
            .iter()
            .enumerate()
            .any(|(j, d)| j != i && d.kind == kind && d.name.eq_ignore_ascii_case(&def.name))
        {
            return Err(format!(
                "\"{}\" is already a {} property",
                def.name,
                kind.name()
            ));
        }
        let old = std::mem::replace(&mut self.defs[i], def.clone());
        let mut dropped = 0;
        for (key, map) in &mut self.values {
            if !PropKey(key.clone()).fits(kind) {
                continue;
            }
            let Some(v) = map.remove(&old.name) else {
                continue;
            };
            // Re-read the value through the new definition.
            let text = old.format(&v);
            match def.parse(&text) {
                Ok(Some(nv)) => {
                    map.insert(def.name.clone(), nv);
                }
                _ => dropped += 1,
            }
        }
        self.values.retain(|_, m| !m.is_empty());
        Ok(dropped)
    }

    /// Removes a definition and every value stored for it. Returns whether
    /// it existed.
    pub fn remove_def(&mut self, kind: PropKind, name: &str) -> bool {
        let Some(i) = self
            .defs
            .iter()
            .position(|d| d.kind == kind && d.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        let old = self.defs.remove(i);
        for (key, map) in &mut self.values {
            if PropKey(key.clone()).fits(kind) {
                map.remove(&old.name);
            }
        }
        self.values.retain(|_, m| !m.is_empty());
        true
    }

    /// The value of property `def` on the object `key`: the stored one, else
    /// the default.
    pub fn value(&self, key: &PropKey, def: &PropDef) -> Option<PropValue> {
        self.values
            .get(key.as_str())
            .and_then(|m| m.get(&def.name))
            .cloned()
            .or_else(|| def.default_value())
    }

    /// [`PropTable::value`] as the text a cell shows.
    pub fn text(&self, key: &PropKey, def: &PropDef) -> String {
        self.value(key, def)
            .map(|v| def.format(&v))
            .unwrap_or_default()
    }

    /// Stores `value` for property `def` on `key`. `None`, or a value equal
    /// to the default, removes the stored entry. Returns whether anything
    /// changed in what the object shows.
    pub fn set(&mut self, key: &PropKey, def: &PropDef, value: Option<PropValue>) -> bool {
        let before = self.value(key, def);
        let value = value.filter(|v| def.default_value().as_ref() != Some(v));
        match value {
            Some(v) => {
                self.values
                    .entry(key.0.clone())
                    .or_default()
                    .insert(def.name.clone(), v);
            }
            None => {
                if let Some(m) = self.values.get_mut(key.as_str()) {
                    m.remove(&def.name);
                    if m.is_empty() {
                        self.values.remove(key.as_str());
                    }
                }
            }
        }
        self.value(key, def) != before
    }

    /// Sets the property from typed text (an empty string clears it).
    pub fn set_text(&mut self, key: &PropKey, def: &PropDef, text: &str) -> Result<bool, String> {
        let v = def.parse(text)?;
        Ok(self.set(key, def, v))
    }

    /// The stored values of one object.
    pub fn map(&self, key: &PropKey) -> Option<&PropMap> {
        self.values.get(key.as_str())
    }

    /// Drops the values of objects `alive` no longer finds. Returns how many
    /// objects were dropped.
    pub fn purge(&mut self, alive: impl Fn(&PropKey) -> bool) -> usize {
        let before = self.values.len();
        self.values.retain(|k, _| alive(&PropKey(k.clone())));
        before - self.values.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    fn rating() -> PropDef {
        let mut d = PropDef::new(PropKind::Door, "Fire Rating", PropType::List);
        d.options = vec!["None".into(), "20 min".into(), "45 min".into()];
        d.default = "None".into();
        d
    }

    #[test]
    fn values_parse_and_format_by_type() {
        let n = PropDef::new(PropKind::Wall, "R-Value", PropType::Number);
        assert_eq!(n.parse("13.5"), Ok(Some(PropValue::Number(13.5))));
        assert_eq!(n.parse("1,200"), Ok(Some(PropValue::Number(1200.0))));
        assert!(n.parse("abc").is_err());
        assert_eq!(n.parse("  "), Ok(None));
        assert_eq!(n.format(&PropValue::Number(13.0)), "13");
        assert_eq!(n.format(&PropValue::Number(13.25)), "13.25");

        let l = PropDef::new(PropKind::Cabinet, "Reveal", PropType::Length);
        assert_eq!(l.parse("1'-6\""), Ok(Some(PropValue::Number(18.0))));
        assert!(l.parse("wide").is_err());
        assert_eq!(l.format(&PropValue::Number(18.0)), "1'-6\"");

        let b = PropDef::new(PropKind::Room, "Egress", PropType::Bool);
        assert_eq!(b.parse("yes"), Ok(Some(PropValue::Bool(true))));
        assert_eq!(b.parse("FALSE"), Ok(Some(PropValue::Bool(false))));
        assert!(b.parse("maybe").is_err());
        assert_eq!(b.format(&PropValue::Bool(true)), "Yes");

        let r = rating();
        assert_eq!(
            r.parse("45 MIN"),
            Ok(Some(PropValue::Text("45 min".into())))
        );
        assert!(r.parse("90 min").unwrap_err().contains("not one of"));
    }

    #[test]
    fn defaults_show_until_a_value_is_set_and_equal_values_are_not_stored() {
        let mut t = PropTable::default();
        t.add_def(rating()).unwrap();
        let def = t.def(PropKind::Door, "fire rating").unwrap().clone();
        let k = PropKey::door(12);
        assert_eq!(t.text(&k, &def), "None");
        assert!(t.set_text(&k, &def, "20 min").unwrap());
        assert_eq!(t.text(&k, &def), "20 min");
        assert_eq!(t.values.len(), 1);
        // Back to the default: the entry goes away.
        assert!(t.set_text(&k, &def, "None").unwrap());
        assert!(t.values.is_empty());
        // Setting what is shown is no change.
        assert!(!t.set_text(&k, &def, "none").unwrap());
        // Clearing an unset property changes nothing.
        assert!(!t.set_text(&k, &def, "").unwrap());
        assert!(t.set_text(&k, &def, "x").is_err());
    }

    #[test]
    fn definitions_are_unique_per_kind_and_validated() {
        let mut t = PropTable::default();
        t.add_def(rating()).unwrap();
        assert!(t.add_def(rating()).unwrap_err().contains("already"));
        let mut other = rating();
        other.kind = PropKind::Window;
        assert!(t.add_def(other).is_ok());
        assert!(t
            .add_def(PropDef::new(PropKind::Door, "  ", PropType::Text))
            .is_err());
        assert!(t
            .add_def(PropDef::new(PropKind::Door, "Pick", PropType::List))
            .unwrap_err()
            .contains("choice"));
        assert_eq!(t.defs_for(PropKind::Door).count(), 1);
    }

    #[test]
    fn renaming_and_retyping_carry_or_drop_values() {
        let mut t = PropTable::default();
        t.add_def(PropDef::new(PropKind::Door, "Note", PropType::Text))
            .unwrap();
        let def = t.def(PropKind::Door, "Note").unwrap().clone();
        t.set_text(&PropKey::door(1), &def, "12").unwrap();
        t.set_text(&PropKey::door(2), &def, "twelve").unwrap();
        // A window with the same property name keeps its own.
        t.add_def(PropDef::new(PropKind::Window, "Note", PropType::Text))
            .unwrap();
        let wdef = t.def(PropKind::Window, "Note").unwrap().clone();
        t.set_text(&PropKey::window(3), &wdef, "w").unwrap();
        let new = PropDef::new(PropKind::Door, "Count", PropType::Number);
        let dropped = t.replace_def(PropKind::Door, "Note", new).unwrap();
        assert_eq!(dropped, 1, "\"twelve\" is not a number");
        let d = t.def(PropKind::Door, "Count").unwrap().clone();
        assert_eq!(t.text(&PropKey::door(1), &d), "12");
        assert_eq!(t.text(&PropKey::door(2), &d), "");
        // The window's own "Note" is untouched.
        assert_eq!(t.text(&PropKey::window(3), &wdef), "w");
        assert!(t.remove_def(PropKind::Door, "count"));
        assert!(!t.remove_def(PropKind::Door, "count"));
        assert_eq!(t.text(&PropKey::window(3), &wdef), "w");
    }

    #[test]
    fn keys_name_their_objects() {
        assert_eq!(PropKey::door(5).as_str(), "door:5");
        assert_eq!(PropKey::device(1, 4).as_str(), "device:1:4");
        assert_eq!(
            PropKey::room(0, Point::new(120.4, 83.6)).as_str(),
            "room:0:120,84"
        );
        assert!(PropKey::window(5).fits(PropKind::Window));
        assert!(!PropKey::window(5).fits(PropKind::Door));
        assert!(!PropKey::wall(5).fits(PropKind::Door));
        assert_eq!(PropKey::opening(5, true), PropKey::door(5));
    }

    #[test]
    fn the_table_round_trips_with_the_project_and_old_files_load() {
        let mut p = Project::new("props");
        p.props.add_def(rating()).unwrap();
        let def = p.props.defs[0].clone();
        p.props.set_text(&PropKey::door(9), &def, "45 min").unwrap();
        let q = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(q.props, p.props);
        let old = r#"{"name":"old","floors":[{"name":"1st Floor","elevation":0.0,
            "ceiling_height":109.125,"walls":[],"openings":[]}],"next_id":1}"#;
        assert!(Project::from_json(old).unwrap().props.is_empty());
    }

    #[test]
    fn purge_drops_values_of_vanished_objects() {
        let mut t = PropTable::default();
        t.add_def(PropDef::new(PropKind::Wall, "Tag", PropType::Text))
            .unwrap();
        let def = t.defs[0].clone();
        t.set_text(&PropKey::wall(1), &def, "a").unwrap();
        t.set_text(&PropKey::wall(2), &def, "b").unwrap();
        assert_eq!(t.purge(|k| k.as_str() == "wall:1"), 1);
        assert_eq!(t.values.len(), 1);
    }
}
