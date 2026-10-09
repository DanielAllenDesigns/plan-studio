//! Multiple Saved Defaults and Default Sets (manual pp. 104 to 110).
//!
//! Manual Dimensions, Revision Clouds, Rich Text, Text, Callouts, Markers,
//! Notes and Arrows (and, outside the annotation tools, Room Functions,
//! Structural Member Reporting and Framing Types) each keep any number of
//! *saved defaults*; one of them is active. The active one is what the tools
//! read; it lives where the tools already look for it:
//!
//! * Text, Rich Text, Callouts, Markers and Notes: [`Project::annot_defaults`];
//! * Manual Dimensions: [`PlanDefaults::dimension_sets`] and the active set
//!   name (the sets are older than this module and stay where the dimension
//!   tools find them);
//! * Revision Clouds, Arrows, Room Functions, Structural Member Reporting and
//!   Framing Types: the page values of [`PlanDefaults::pages`] under the
//!   kind's prefix ([`SavedKind::prefix`]).
//!
//! [`SavedDefaults`] (a slot of the plan) holds the *list* of saved defaults of
//! every kind and the Default Sets. A saved default that is not active holds
//! its values; the active one holds the values from the last time it was
//! committed, and [`Project::saved_commit`] copies the live values into it
//! before the list is read, edited or switched. Switching is not an undo step
//! (it changes no object); the dialogs that edit the lists work on a draft and
//! apply it on OK.
//!
//! A *Default Set* is a named bundle: one saved default per annotation kind
//! plus a layer set and the current CAD layer. A saved plan view names the set
//! (or the individual picks) it shows with.

use super::{PageValue, PlanDefaults};
use crate::callout::{Callout, Marker, Note, TextSpec, DEFAULT_SAVED_NAME};
use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The key of the Current CAD Layer in [`PlanDefaults::pages`] (General CAD
/// Defaults).
pub const CURRENT_CAD_LAYER_KEY: &str = "cad.general.current_layer";
/// The built-in Current CAD Layer.
pub const DEFAULT_CAD_LAYER_NAME: &str = "CAD, Default";

/// A tool (or list) that has multiple saved defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SavedKind {
    ManualDimensions,
    Text,
    RichText,
    RevisionClouds,
    Callouts,
    Markers,
    Notes,
    Arrows,
    RoomFunctions,
    StructuralMemberReporting,
    FramingTypes,
}

impl SavedKind {
    /// Every kind, in the order of the Default Settings list.
    pub const ALL: [SavedKind; 11] = [
        SavedKind::ManualDimensions,
        SavedKind::Text,
        SavedKind::RichText,
        SavedKind::RevisionClouds,
        SavedKind::Callouts,
        SavedKind::Markers,
        SavedKind::Notes,
        SavedKind::Arrows,
        SavedKind::RoomFunctions,
        SavedKind::StructuralMemberReporting,
        SavedKind::FramingTypes,
    ];

    /// The kinds a Default Set holds (the annotation tools).
    pub const ANNOTATION: [SavedKind; 8] = [
        SavedKind::ManualDimensions,
        SavedKind::Text,
        SavedKind::RichText,
        SavedKind::RevisionClouds,
        SavedKind::Callouts,
        SavedKind::Markers,
        SavedKind::Notes,
        SavedKind::Arrows,
    ];

    /// A stable id: the key in files and in a Default Set.
    pub fn id(self) -> &'static str {
        match self {
            SavedKind::ManualDimensions => "dimensions",
            SavedKind::Text => "text",
            SavedKind::RichText => "rich_text",
            SavedKind::RevisionClouds => "revision_clouds",
            SavedKind::Callouts => "callouts",
            SavedKind::Markers => "markers",
            SavedKind::Notes => "notes",
            SavedKind::Arrows => "arrows",
            SavedKind::RoomFunctions => "room_functions",
            SavedKind::StructuralMemberReporting => "structural_reporting",
            SavedKind::FramingTypes => "framing_types",
        }
    }

    /// The kind with id `id`.
    pub fn from_id(id: &str) -> Option<SavedKind> {
        SavedKind::ALL.into_iter().find(|k| k.id() == id)
    }

    /// The name Chief shows for the kind.
    pub fn label(self) -> &'static str {
        match self {
            SavedKind::ManualDimensions => "Manual Dimensions",
            SavedKind::Text => "Text",
            SavedKind::RichText => "Rich Text",
            SavedKind::RevisionClouds => "Revision Clouds",
            SavedKind::Callouts => "Callouts",
            SavedKind::Markers => "Markers",
            SavedKind::Notes => "Notes",
            SavedKind::Arrows => "Arrows",
            SavedKind::RoomFunctions => "Room Functions",
            SavedKind::StructuralMemberReporting => "Structural Member Reporting",
            SavedKind::FramingTypes => "Framing Types",
        }
    }

    /// Is this kind part of a Default Set?
    pub fn in_default_set(self) -> bool {
        Self::ANNOTATION.contains(&self)
    }

    /// The keys of [`PlanDefaults::pages`] the kind's values live under, for
    /// the kinds that keep their active values there.
    pub fn prefix(self) -> Option<&'static str> {
        match self {
            SavedKind::RevisionClouds => Some("revision_clouds."),
            SavedKind::Arrows => Some("text.arrows."),
            SavedKind::RoomFunctions => Some("saved.room_functions."),
            SavedKind::StructuralMemberReporting => Some("saved.structural_reporting."),
            SavedKind::FramingTypes => Some("saved.framing_types."),
            _ => None,
        }
    }

    /// The generic Default Settings page that edits the kind's values, for the
    /// kinds that keep them as page values. Manual Dimensions, Text, Rich
    /// Text, Callouts, Markers and Notes have defaults dialogs of their own.
    pub fn page_id(self) -> Option<&'static str> {
        match self {
            SavedKind::RevisionClouds => Some("revision_clouds"),
            SavedKind::Arrows => Some("text.arrows"),
            SavedKind::RoomFunctions => Some("saved.room_functions"),
            SavedKind::StructuralMemberReporting => Some("saved.structural_reporting"),
            SavedKind::FramingTypes => Some("saved.framing_types"),
            _ => None,
        }
    }

    /// Can the last saved default be deleted? (Structural Member Reporting
    /// keeps at least one, manual p. 906; so does every other kind here, a
    /// tool needs an active default.)
    pub fn keeps_one(self) -> bool {
        true
    }
}

/// The values of one saved default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SavedValue {
    Text(TextSpec),
    Rich(TextSpec),
    Callout(Callout),
    Marker(Marker),
    Note(Note),
    /// Page values by key (Revision Clouds, Arrows, Room Functions,
    /// Structural Member Reporting, Framing Types).
    Values(BTreeMap<String, PageValue>),
}

/// One named saved default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedDefault {
    pub name: String,
    pub value: SavedValue,
}

/// The saved defaults of one kind and which one is active.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct KindList {
    pub active: String,
    pub items: Vec<SavedDefault>,
}

impl KindList {
    fn index(&self, name: &str) -> Option<usize> {
        self.items.iter().position(|s| s.name == name)
    }
}

/// A named bundle of saved defaults, a layer set and the current CAD layer.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DefaultSet {
    pub name: String,
    /// Saved default name by [`SavedKind::id`]; a kind that is missing keeps
    /// whatever is active.
    #[serde(default)]
    pub members: BTreeMap<String, String>,
    /// Layer set; empty keeps the shown one.
    #[serde(default)]
    pub layer_set: String,
    /// Current CAD layer; empty keeps it.
    #[serde(default)]
    pub cad_layer: String,
}

impl DefaultSet {
    /// The saved default this set uses for `kind`.
    pub fn member(&self, kind: SavedKind) -> Option<&str> {
        self.members.get(kind.id()).map(String::as_str)
    }
}

/// The lists of saved defaults and the Default Sets of a plan (or of the
/// template new plans start from).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SavedDefaults {
    /// By [`SavedKind::id`]. Manual Dimensions are not here: their sets live
    /// in [`PlanDefaults::dimension_sets`].
    #[serde(default)]
    pub lists: BTreeMap<String, KindList>,
    #[serde(default)]
    pub sets: Vec<DefaultSet>,
    /// The Default Set last activated; empty while "Active Defaults" are in
    /// use (see [`Project::using_default_set`]).
    #[serde(default)]
    pub active_set: String,
    /// What each object kind's Use Default state is, per object.
    #[serde(default, skip_serializing_if = "super::dynamic::FollowTable::is_empty")]
    pub follow: super::dynamic::FollowTable,
    /// Which saved default each object was made with, by kind id then object
    /// id: the in-use rule of Delete.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uses: BTreeMap<String, BTreeMap<u64, String>>,
}

impl SavedDefaults {
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
            && self.sets.is_empty()
            && self.active_set.is_empty()
            && self.follow.is_empty()
            && self.uses.is_empty()
    }

    /// The Default Set `name`.
    pub fn set(&self, name: &str) -> Option<&DefaultSet> {
        self.sets.iter().find(|s| s.name == name)
    }

    /// The saved defaults of `kind` (empty before the kind is first used).
    pub fn list(&self, kind: SavedKind) -> Option<&KindList> {
        self.lists.get(kind.id())
    }

    /// Records that object `id` was made with saved default `name` of `kind`.
    pub fn note_use(&mut self, kind: SavedKind, id: u64, name: &str) {
        self.uses
            .entry(kind.id().to_string())
            .or_default()
            .insert(id, name.to_string());
    }

    /// How many recorded objects use `name`.
    pub fn use_count(&self, kind: SavedKind, name: &str) -> usize {
        self.uses
            .get(kind.id())
            .map_or(0, |m| m.values().filter(|n| *n == name).count())
    }

    /// Forgets the recorded uses of objects that no longer exist.
    pub fn prune_uses(&mut self, live: &dyn Fn(u64) -> bool) {
        for m in self.uses.values_mut() {
            m.retain(|id, _| live(*id));
        }
        self.uses.retain(|_, m| !m.is_empty());
        self.follow.retain_objects(live);
    }
}

/// Why a name is refused.
pub fn check_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        Err("Type a name".into())
    } else {
        Ok(n.to_string())
    }
}

// ----- the live values -----

fn page_slice(d: &PlanDefaults, prefix: &str) -> BTreeMap<String, PageValue> {
    d.pages
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

impl Project {
    /// The values a kind's tools read now. `None` for Manual Dimensions (their
    /// sets are in the plan defaults).
    fn saved_live(&self, d: &PlanDefaults, kind: SavedKind) -> Option<SavedValue> {
        Some(match kind {
            SavedKind::Text => SavedValue::Text(self.annot_defaults.text.clone()),
            SavedKind::RichText => SavedValue::Rich(self.annot_defaults.rich.clone()),
            SavedKind::Callouts => SavedValue::Callout(self.annot_defaults.callout.clone()),
            SavedKind::Markers => SavedValue::Marker(self.annot_defaults.marker.clone()),
            SavedKind::Notes => SavedValue::Note(self.annot_defaults.note.clone()),
            SavedKind::ManualDimensions => return None,
            other => SavedValue::Values(page_slice(d, other.prefix()?)),
        })
    }

    /// Makes `value` what a kind's tools read.
    fn saved_set_live(&mut self, d: &mut PlanDefaults, kind: SavedKind, value: &SavedValue) {
        match (kind, value) {
            (SavedKind::Text, SavedValue::Text(v)) => self.annot_defaults.text = v.clone(),
            (SavedKind::RichText, SavedValue::Rich(v)) => self.annot_defaults.rich = v.clone(),
            (SavedKind::Callouts, SavedValue::Callout(v)) => {
                self.annot_defaults.callout = v.clone()
            }
            (SavedKind::Markers, SavedValue::Marker(v)) => self.annot_defaults.marker = v.clone(),
            (SavedKind::Notes, SavedValue::Note(v)) => self.annot_defaults.note = v.clone(),
            (_, SavedValue::Values(map)) => {
                if let Some(prefix) = kind.prefix() {
                    d.pages.retain(|k, _| !k.starts_with(prefix));
                    for (k, v) in map {
                        d.pages.insert(k.clone(), v.clone());
                    }
                }
            }
            _ => {}
        }
    }

    /// A project made from a template whose saved defaults are stored (a
    /// template saved with its lists): the tools start with the active saved
    /// default of each annotation kind.
    pub fn saved_adopt_active(&mut self) {
        for kind in [
            SavedKind::Text,
            SavedKind::RichText,
            SavedKind::Callouts,
            SavedKind::Markers,
            SavedKind::Notes,
        ] {
            let Some(list) = self.saved_defaults.list(kind) else {
                continue;
            };
            let Some(item) = list.items.iter().find(|s| s.name == list.active) else {
                continue;
            };
            let (name, value) = (item.name.clone(), item.value.clone());
            match (kind, value) {
                (SavedKind::Text, SavedValue::Text(v)) => self.annot_defaults.text = v,
                (SavedKind::RichText, SavedValue::Rich(v)) => self.annot_defaults.rich = v,
                (SavedKind::Callouts, SavedValue::Callout(v)) => self.annot_defaults.callout = v,
                (SavedKind::Markers, SavedValue::Marker(v)) => self.annot_defaults.marker = v,
                (SavedKind::Notes, SavedValue::Note(v)) => self.annot_defaults.note = v,
                _ => continue,
            }
            if kind != SavedKind::Notes {
                self.annot_defaults.saved_name = name;
            }
        }
    }

    /// The name of the one saved default a kind has before its list exists:
    /// the annotation defaults dialogs' "Saved Default" name, else "Default".
    fn virtual_name(&self, kind: SavedKind) -> String {
        const NAMED: [SavedKind; 4] = [
            SavedKind::Text,
            SavedKind::RichText,
            SavedKind::Callouts,
            SavedKind::Markers,
        ];
        // The one "Saved Default" name of the annotation dialogs belongs to
        // the first list that is made; once any of the four has a list, the
        // others start as "Default".
        let named_already = NAMED
            .iter()
            .any(|k| self.saved_defaults.lists.contains_key(k.id()));
        if NAMED.contains(&kind) && !named_already && !self.annot_defaults.saved_name.is_empty() {
            self.annot_defaults.saved_name.clone()
        } else {
            DEFAULT_SAVED_NAME.to_string()
        }
    }

    /// Makes sure the list of `kind` exists (a first saved default named after
    /// the active one, holding the values the tools read now).
    fn saved_ensure(&mut self, d: &PlanDefaults, kind: SavedKind) {
        if kind == SavedKind::ManualDimensions {
            return;
        }
        if self.saved_defaults.lists.contains_key(kind.id()) {
            return;
        }
        let name = self.virtual_name(kind);
        let Some(value) = self.saved_live(d, kind) else {
            return;
        };
        self.saved_defaults.lists.insert(
            kind.id().to_string(),
            KindList {
                active: name.clone(),
                items: vec![SavedDefault { name, value }],
            },
        );
    }

    /// Copies the values the tools read into the active saved default of
    /// `kind` (when the kind has a list: reading the lists never makes one).
    /// Call before changing the list.
    pub fn saved_commit(&mut self, d: &mut PlanDefaults, kind: SavedKind) {
        if kind == SavedKind::ManualDimensions || !self.saved_defaults.lists.contains_key(kind.id())
        {
            // The Dimension Defaults pages already keep the active set in step.
            return;
        }
        let Some(live) = self.saved_live(d, kind) else {
            return;
        };
        if let Some(list) = self.saved_defaults.lists.get_mut(kind.id()) {
            if let Some(i) = list.index(&list.active.clone()) {
                list.items[i].value = live;
            }
        }
    }

    /// Commits every kind (see [`Project::saved_commit`]).
    pub fn saved_commit_all(&mut self, d: &mut PlanDefaults) {
        for k in SavedKind::ALL {
            self.saved_commit(d, k);
        }
    }

    /// The names of the saved defaults of `kind`, in list order.
    pub fn saved_names(&self, d: &PlanDefaults, kind: SavedKind) -> Vec<String> {
        if kind == SavedKind::ManualDimensions {
            return d.dimension_sets.iter().map(|s| s.name.clone()).collect();
        }
        match self.saved_defaults.list(kind) {
            Some(l) => l.items.iter().map(|s| s.name.clone()).collect(),
            None => vec![self.virtual_name(kind)],
        }
    }

    /// The name of the active saved default of `kind`.
    pub fn saved_active(&self, d: &PlanDefaults, kind: SavedKind) -> String {
        if kind == SavedKind::ManualDimensions {
            return d.active_dimension_set.clone();
        }
        match self.saved_defaults.list(kind) {
            Some(l) => l.active.clone(),
            None => self.virtual_name(kind),
        }
    }

    /// Makes saved default `name` the one the tools of `kind` use. The values
    /// of the one being left are kept in it first. `false` when there is no
    /// such saved default.
    pub fn saved_activate(&mut self, d: &mut PlanDefaults, kind: SavedKind, name: &str) -> bool {
        if kind == SavedKind::ManualDimensions {
            return d.set_active_dimension_set(name);
        }
        self.saved_ensure(d, kind);
        self.saved_commit(d, kind);
        let Some(list) = self.saved_defaults.lists.get(kind.id()) else {
            return false;
        };
        let Some(i) = list.index(name) else {
            return false;
        };
        let value = list.items[i].value.clone();
        self.saved_defaults
            .lists
            .get_mut(kind.id())
            .expect("list exists")
            .active = name.to_string();
        self.saved_set_live(d, kind, &value);
        if matches!(
            kind,
            SavedKind::Text | SavedKind::RichText | SavedKind::Callouts | SavedKind::Markers
        ) {
            // The titles of the annotation defaults dialogs name the saved
            // default in use.
            self.annot_defaults.saved_name = name.to_string();
        }
        true
    }

    /// Why saved default `name` of `kind` cannot be deleted, if it cannot:
    /// it is in a Default Set, a saved plan view's picks, used by an object,
    /// or the last one of its kind.
    pub fn saved_blocked(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
    ) -> Option<String> {
        let count = self.saved_names(d, kind).len();
        if count <= 1 && kind.keeps_one() {
            return Some(format!(
                "\"{name}\" is the only {} default and cannot be deleted",
                kind.label()
            ));
        }
        if let Some(set) = self
            .saved_defaults
            .sets
            .iter()
            .find(|s| s.member(kind) == Some(name))
        {
            return Some(format!(
                "\"{name}\" is assigned to the Default Set \"{}\"",
                set.name
            ));
        }
        if let Some(v) = self
            .plan_views
            .iter()
            .find(|v| v.spec.selected.get(kind.id()).map(String::as_str) == Some(name))
        {
            return Some(format!(
                "\"{name}\" is used by the plan view \"{}\"",
                v.name
            ));
        }
        let uses = self.saved_defaults.use_count(kind, name);
        if uses > 0 {
            return Some(format!(
                "\"{name}\" is used by {uses} object(s) in the plan"
            ));
        }
        None
    }

    /// Adds saved default `new_name` as a copy of `from` (the Copy button; the
    /// caller opens its defaults dialog next). Names are case-sensitive and
    /// unique.
    pub fn saved_copy(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        from: &str,
        new_name: &str,
    ) -> Result<(), String> {
        let new_name = check_name(new_name)?;
        let names = self.saved_names(d, kind);
        if names.contains(&new_name) {
            return Err(format!("\"{new_name}\" exists already"));
        }
        if !names.iter().any(|n| n == from) {
            return Err(format!("There is no saved default \"{from}\""));
        }
        if kind == SavedKind::ManualDimensions {
            let set = d
                .dimension_set(from)
                .ok_or_else(|| format!("There is no saved default \"{from}\""))?
                .cloned_as(new_name);
            d.dimension_sets.push(set);
            return Ok(());
        }
        self.saved_ensure(d, kind);
        self.saved_commit(d, kind);
        let list = self
            .saved_defaults
            .lists
            .get_mut(kind.id())
            .ok_or("No saved defaults")?;
        let i = list.index(from).ok_or("No such saved default")?;
        let mut copy = list.items[i].clone();
        copy.name = new_name;
        list.items.push(copy);
        Ok(())
    }

    /// Renames a saved default; Default Sets, plan views and recorded uses
    /// follow.
    pub fn saved_rename(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        old: &str,
        new_name: &str,
    ) -> Result<(), String> {
        let new_name = check_name(new_name)?;
        if new_name == old {
            return Ok(());
        }
        let names = self.saved_names(d, kind);
        if names.contains(&new_name) {
            return Err(format!("\"{new_name}\" exists already"));
        }
        if !names.iter().any(|n| n == old) {
            return Err(format!("There is no saved default \"{old}\""));
        }
        if kind == SavedKind::ManualDimensions {
            let was_active = d.active_dimension_set == old;
            if let Some(s) = d.dimension_sets.iter_mut().find(|s| s.name == old) {
                let renamed = s.cloned_as(new_name.clone());
                *s = renamed;
            }
            if was_active {
                d.active_dimension_set = new_name.clone();
            }
            for v in &mut self.plan_views {
                if v.dimension_defaults == old {
                    v.dimension_defaults = new_name.clone();
                }
            }
        } else if {
            self.saved_ensure(d, kind);
            self.saved_commit(d, kind);
            true
        } && self.saved_defaults.lists.contains_key(kind.id())
        {
            let list = self
                .saved_defaults
                .lists
                .get_mut(kind.id())
                .expect("list exists");
            if let Some(i) = list.index(old) {
                list.items[i].name = new_name.clone();
            }
            if list.active == old {
                list.active = new_name.clone();
            }
        }
        for set in &mut self.saved_defaults.sets {
            if let Some(m) = set.members.get_mut(kind.id()) {
                if m == old {
                    *m = new_name.clone();
                }
            }
        }
        for v in &mut self.plan_views {
            if let Some(m) = v.spec.selected.get_mut(kind.id()) {
                if m == old {
                    *m = new_name.clone();
                }
            }
        }
        if let Some(m) = self.saved_defaults.uses.get_mut(kind.id()) {
            for n in m.values_mut() {
                if n == old {
                    *n = new_name.clone();
                }
            }
        }
        Ok(())
    }

    /// Deletes a saved default unless [`Project::saved_blocked`] says why not.
    /// Deleting the active one activates the first that remains.
    pub fn saved_delete(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
    ) -> Result<(), String> {
        if let Some(why) = self.saved_blocked(d, kind, name) {
            return Err(why);
        }
        let active = self.saved_active(d, kind);
        if kind == SavedKind::ManualDimensions {
            d.dimension_sets.retain(|s| s.name != name);
        } else {
            self.saved_ensure(d, kind);
            self.saved_commit(d, kind);
            if let Some(list) = self.saved_defaults.lists.get_mut(kind.id()) {
                list.items.retain(|s| s.name != name);
            }
        }
        if active == name {
            let first = self.saved_names(d, kind).into_iter().next();
            if let Some(f) = first {
                self.saved_activate(d, kind, &f);
            }
        }
        Ok(())
    }

    /// Adds or replaces the saved default `name` of `kind` with `value` (not
    /// active). Used when an object is pasted into this file and brings its
    /// saved default along, and by Import Settings. `replace` decides what an
    /// existing name does. Returns the name it was stored under.
    pub fn saved_store(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
        value: SavedValue,
        replace: bool,
    ) -> String {
        if kind == SavedKind::ManualDimensions {
            return name.to_string();
        }
        self.saved_ensure(d, kind);
        self.saved_commit(d, kind);
        let Some(list) = self.saved_defaults.lists.get_mut(kind.id()) else {
            return name.to_string();
        };
        match list.index(name) {
            Some(i) if replace => {
                list.items[i].value = value;
                let active = list.active == name;
                if active {
                    let v = list.items[i].value.clone();
                    self.saved_set_live(d, kind, &v);
                }
                name.to_string()
            }
            Some(_) => {
                let mut n = 2;
                let new_name = loop {
                    let c = format!("{name} {n}");
                    if list.index(&c).is_none() {
                        break c;
                    }
                    n += 1;
                };
                list.items.push(SavedDefault {
                    name: new_name.clone(),
                    value,
                });
                new_name
            }
            None => {
                list.items.push(SavedDefault {
                    name: name.to_string(),
                    value,
                });
                name.to_string()
            }
        }
    }

    /// The stored values of saved default `name` (the live ones for the
    /// active default).
    pub fn saved_value(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
    ) -> Option<SavedValue> {
        self.saved_ensure(d, kind);
        self.saved_commit(d, kind);
        let list = self.saved_defaults.list(kind)?;
        list.items
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.value.clone())
    }

    /// Replaces the values of saved default `name` (a defaults dialog's OK on
    /// a saved default that is not active); the active one writes through to
    /// the tools.
    pub fn saved_set_value(
        &mut self,
        d: &mut PlanDefaults,
        kind: SavedKind,
        name: &str,
        value: SavedValue,
    ) -> bool {
        self.saved_ensure(d, kind);
        self.saved_commit(d, kind);
        let Some(list) = self.saved_defaults.lists.get_mut(kind.id()) else {
            return false;
        };
        let Some(i) = list.index(name) else {
            return false;
        };
        list.items[i].value = value.clone();
        if list.active == name {
            self.saved_set_live(d, kind, &value);
        }
        true
    }

    // ----- Default Sets -----

    /// The kinds of a set with the saved default each one uses now.
    pub fn current_picks(&mut self, d: &mut PlanDefaults) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        for k in SavedKind::ANNOTATION {
            let a = self.saved_active(d, k);
            if !a.is_empty() {
                m.insert(k.id().to_string(), a);
            }
        }
        m
    }

    /// The Current CAD Layer.
    pub fn current_cad_layer(d: &PlanDefaults) -> String {
        d.page_text(CURRENT_CAD_LAYER_KEY, DEFAULT_CAD_LAYER_NAME)
    }

    /// The Default Set whose picks, layer set and CAD layer are all in force
    /// now, preferring the one activated last; `None` is "Using Active
    /// Defaults" (manual p. 108).
    pub fn using_default_set(&mut self, d: &mut PlanDefaults) -> Option<String> {
        let picks = self.current_picks(d);
        let layer_set = self.shown_layer_set().to_string();
        let cad = Self::current_cad_layer(d);
        let matches = |s: &DefaultSet| {
            s.members.iter().all(|(k, v)| picks.get(k) == Some(v))
                && (s.layer_set.is_empty() || s.layer_set == layer_set)
                && (s.cad_layer.is_empty() || s.cad_layer == cad)
        };
        let last = self.saved_defaults.active_set.clone();
        if let Some(s) = self.saved_defaults.set(&last) {
            if matches(s) {
                return Some(last);
            }
        }
        self.saved_defaults
            .sets
            .iter()
            .find(|s| matches(s))
            .map(|s| s.name.clone())
    }

    /// Creates Default Set `name` from the saved defaults, layer set and CAD
    /// layer in force now (Save New Default Set).
    pub fn default_set_save_new(&mut self, d: &mut PlanDefaults, name: &str) -> Result<(), String> {
        let name = check_name(name)?;
        if self.saved_defaults.set(&name).is_some() {
            return Err(format!("A Default Set named \"{name}\" exists already"));
        }
        let members = self.current_picks(d);
        let set = DefaultSet {
            name: name.clone(),
            members,
            layer_set: self.shown_layer_set().to_string(),
            cad_layer: Self::current_cad_layer(d),
        };
        self.saved_defaults.sets.push(set);
        self.saved_defaults.active_set = name;
        Ok(())
    }

    /// Activates Default Set `name`: every saved default it names, its layer
    /// set (in the shown plan view) and its CAD layer. A member that no longer
    /// exists is skipped. `false` when there is no such set.
    pub fn default_set_activate(&mut self, d: &mut PlanDefaults, name: &str) -> bool {
        let Some(set) = self.saved_defaults.set(name).cloned() else {
            return false;
        };
        for k in SavedKind::ANNOTATION {
            self.saved_commit(d, k);
        }
        for (kid, saved) in &set.members {
            if let Some(kind) = SavedKind::from_id(kid) {
                self.saved_activate(d, kind, saved);
            }
        }
        if !set.layer_set.is_empty() {
            self.show_layer_set(&set.layer_set);
        }
        if !set.cad_layer.is_empty() {
            d.pages.insert(
                CURRENT_CAD_LAYER_KEY.to_string(),
                PageValue::Text(set.cad_layer.clone()),
            );
        }
        self.saved_defaults.active_set = name.to_string();
        true
    }

    /// Copies Default Set `from` as `new_name` (the New button of the Default
    /// Sets dialog).
    pub fn default_set_copy(&mut self, from: &str, new_name: &str) -> Result<(), String> {
        let new_name = check_name(new_name)?;
        if self.saved_defaults.set(&new_name).is_some() {
            return Err(format!("A Default Set named \"{new_name}\" exists already"));
        }
        let mut copy = self
            .saved_defaults
            .set(from)
            .cloned()
            .ok_or_else(|| format!("There is no Default Set \"{from}\""))?;
        copy.name = new_name;
        self.saved_defaults.sets.push(copy);
        Ok(())
    }

    /// Renames a Default Set (names are unique, case-insensitively); plan
    /// views follow.
    pub fn default_set_rename(&mut self, old: &str, new_name: &str) -> Result<(), String> {
        let new_name = check_name(new_name)?;
        if new_name == old {
            return Ok(());
        }
        if self
            .saved_defaults
            .sets
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&new_name) && s.name != old)
        {
            return Err(format!("A Default Set named \"{new_name}\" exists already"));
        }
        let s = self
            .saved_defaults
            .sets
            .iter_mut()
            .find(|s| s.name == old)
            .ok_or_else(|| format!("There is no Default Set \"{old}\""))?;
        s.name = new_name.clone();
        if self.saved_defaults.active_set == old {
            self.saved_defaults.active_set = new_name.clone();
        }
        for v in &mut self.plan_views {
            if v.spec.default_set == old {
                v.spec.default_set = new_name.clone();
            }
        }
        Ok(())
    }

    /// Deletes a Default Set. Plan views that named it keep their picks.
    pub fn default_set_delete(&mut self, name: &str) -> Result<(), String> {
        if self.saved_defaults.set(name).is_none() {
            return Err(format!("There is no Default Set \"{name}\""));
        }
        self.saved_defaults.sets.retain(|s| s.name != name);
        if self.saved_defaults.active_set == name {
            self.saved_defaults.active_set.clear();
        }
        for v in &mut self.plan_views {
            if v.spec.default_set == name {
                v.spec.default_set.clear();
            }
        }
        Ok(())
    }

    // ----- views -----

    /// Stores the defaults in force into saved plan view `name` (Save Active
    /// View): the Default Set while one is in use, else the individual picks,
    /// plus the CAD layer.
    pub fn view_capture_defaults(&mut self, d: &mut PlanDefaults, name: &str) {
        let picks = self.current_picks(d);
        let set = self.using_default_set(d).unwrap_or_default();
        let cad = Self::current_cad_layer(d);
        if let Some(v) = self.plan_views.iter_mut().find(|v| v.name == name) {
            v.spec.default_set = set;
            v.spec.selected = picks;
            v.spec.cad_layer = cad;
        }
    }

    /// Makes the defaults saved with plan view `name` the active ones (opening
    /// a saved view): its Default Set, then the individual picks, then the
    /// CAD layer. A view saved before defaults were stored with it changes
    /// nothing.
    pub fn view_apply_defaults(&mut self, d: &mut PlanDefaults, name: &str) {
        let Some(v) = self.plan_view(name) else {
            return;
        };
        let (set, picks, cad) = (
            v.spec.default_set.clone(),
            v.spec.selected.clone(),
            v.spec.cad_layer.clone(),
        );
        if !set.is_empty() {
            self.default_set_activate(d, &set);
        }
        for k in SavedKind::ANNOTATION {
            self.saved_commit(d, k);
        }
        for (kid, saved) in &picks {
            if let Some(kind) = SavedKind::from_id(kid) {
                self.saved_activate(d, kind, saved);
            }
        }
        if !cad.is_empty() {
            d.pages
                .insert(CURRENT_CAD_LAYER_KEY.to_string(), PageValue::Text(cad));
        }
        if set.is_empty() && !picks.is_empty() {
            self.saved_defaults.active_set.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (Project, PlanDefaults) {
        let d = PlanDefaults::chief_x18_daniel();
        (Project::from_defaults("T", &d), d)
    }

    #[test]
    fn a_kind_starts_with_one_saved_default_named_after_the_active_one() {
        let (mut p, mut d) = fresh();
        assert_eq!(p.saved_names(&mut d, SavedKind::Callouts), vec!["Default"]);
        assert_eq!(p.saved_active(&mut d, SavedKind::Callouts), "Default");
        assert_eq!(
            p.saved_names(&mut d, SavedKind::ManualDimensions).len(),
            d.dimension_sets.len()
        );
    }

    #[test]
    fn copy_edit_switch_restores_the_values_each_default_held() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::Callouts;
        p.saved_copy(&mut d, kind, "Default", "Large").unwrap();
        // Edit the live callout while "Default" is active.
        p.annot_defaults.callout.pose_idx = 1;
        assert!(p.saved_activate(&mut d, kind, "Large"));
        assert_ne!(
            p.annot_defaults.callout.pose_idx, 1,
            "Large still holds the copy"
        );
        p.annot_defaults.callout.pose_idx = 2;
        assert!(p.saved_activate(&mut d, kind, "Default"));
        assert_eq!(p.annot_defaults.callout.pose_idx, 1);
        assert!(p.saved_activate(&mut d, kind, "Large"));
        assert_eq!(p.annot_defaults.callout.pose_idx, 2);
        assert!(!p.saved_activate(&mut d, kind, "Missing"));
    }

    #[test]
    fn page_backed_kinds_swap_their_page_values() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::Arrows;
        d.pages
            .insert("text.arrows.size".into(), PageValue::Num(6.0));
        p.saved_copy(&mut d, kind, "Default", "Big").unwrap();
        p.saved_activate(&mut d, kind, "Big");
        d.pages
            .insert("text.arrows.size".into(), PageValue::Num(12.0));
        p.saved_activate(&mut d, kind, "Default");
        assert_eq!(d.page_num("text.arrows.size", 0.0), 6.0);
        p.saved_activate(&mut d, kind, "Big");
        assert_eq!(d.page_num("text.arrows.size", 0.0), 12.0);
    }

    #[test]
    fn rename_follows_into_sets_views_and_the_active_pointer() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::Markers;
        p.saved_copy(&mut d, kind, "Default", "Small").unwrap();
        p.default_set_save_new(&mut d, "Set A").unwrap();
        assert_eq!(
            p.saved_defaults.set("Set A").unwrap().member(kind),
            Some("Default")
        );
        p.saved_rename(&mut d, kind, "Default", "Plain").unwrap();
        assert_eq!(p.saved_active(&mut d, kind), "Plain");
        assert_eq!(
            p.saved_defaults.set("Set A").unwrap().member(kind),
            Some("Plain")
        );
        assert!(p.saved_rename(&mut d, kind, "Plain", "Small").is_err());
        assert!(p.saved_rename(&mut d, kind, "Plain", "  ").is_err());
        assert!(p.saved_rename(&mut d, kind, "Nope", "X").is_err());
    }

    #[test]
    fn delete_is_blocked_in_a_set_in_use_or_when_last() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::Text;
        assert!(
            p.saved_delete(&mut d, kind, "Default").is_err(),
            "the last one stays"
        );
        p.saved_copy(&mut d, kind, "Default", "Other").unwrap();
        p.default_set_save_new(&mut d, "Set A").unwrap();
        // "Default" is in Set A.
        let why = p.saved_delete(&mut d, kind, "Default").unwrap_err();
        assert!(why.contains("Set A"), "{why}");
        // "Other" is free until an object uses it.
        p.saved_defaults.note_use(kind, 7, "Other");
        assert!(p
            .saved_delete(&mut d, kind, "Other")
            .unwrap_err()
            .contains("used by 1"));
        p.saved_defaults.prune_uses(&|_| false);
        p.saved_delete(&mut d, kind, "Other").unwrap();
        assert_eq!(p.saved_names(&mut d, kind), vec!["Default"]);
        // Deleting a set frees its members.
        p.saved_copy(&mut d, kind, "Default", "Third").unwrap();
        p.default_set_delete("Set A").unwrap();
        p.saved_activate(&mut d, kind, "Third");
        p.saved_delete(&mut d, kind, "Default").unwrap();
        assert_eq!(p.saved_active(&mut d, kind), "Third");
    }

    #[test]
    fn a_view_names_the_picks_that_block_a_delete() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::Arrows;
        p.saved_copy(&mut d, kind, "Default", "B").unwrap();
        p.plan_views[0]
            .spec
            .selected
            .insert(kind.id().into(), "B".into());
        assert!(p
            .saved_delete(&mut d, kind, "B")
            .unwrap_err()
            .contains("plan view"));
    }

    #[test]
    fn dimension_saved_defaults_use_the_plan_default_sets() {
        let (mut p, mut d) = fresh();
        let kind = SavedKind::ManualDimensions;
        let n = d.dimension_sets.len();
        p.saved_copy(&mut d, kind, "1/4\" Scale", "Mine").unwrap();
        assert_eq!(d.dimension_sets.len(), n + 1);
        assert!(p.saved_activate(&mut d, kind, "1/8\" Scale"));
        assert_eq!(d.active_dimension_set, "1/8\" Scale");
        p.saved_rename(&mut d, kind, "Mine", "Mine 2").unwrap();
        assert!(d.dimension_set("Mine 2").is_some());
        p.default_set_save_new(&mut d, "S").unwrap();
        assert!(p
            .saved_delete(&mut d, kind, "1/8\" Scale")
            .unwrap_err()
            .contains("Default Set"));
        p.saved_delete(&mut d, kind, "Mine 2").unwrap();
        assert_eq!(d.dimension_sets.len(), n);
    }

    #[test]
    fn default_sets_switch_every_member_the_layer_set_and_the_cad_layer() {
        let (mut p, mut d) = fresh();
        // Two sets that differ in dimensions, text and layer set.
        p.saved_copy(&mut d, SavedKind::Text, "Default", "Big")
            .unwrap();
        p.layer_sets.copy_set("Default Set", "Dimmed");
        p.default_set_save_new(&mut d, "Quarter").unwrap();
        d.set_active_dimension_set("1/8\" Scale");
        p.saved_activate(&mut d, SavedKind::Text, "Big");
        p.annot_defaults.text.height = 9.0;
        p.show_layer_set("Dimmed");
        d.pages.insert(
            CURRENT_CAD_LAYER_KEY.into(),
            PageValue::Text("CAD, Detail".into()),
        );
        assert_eq!(p.using_default_set(&mut d), None, "no set matches yet");
        p.default_set_save_new(&mut d, "Eighth").unwrap();
        assert_eq!(p.using_default_set(&mut d).as_deref(), Some("Eighth"));
        // Back to the first.
        assert!(p.default_set_activate(&mut d, "Quarter"));
        assert_eq!(d.active_dimension_set, "1/4\" Scale");
        assert_eq!(p.saved_active(&mut d, SavedKind::Text), "Default");
        assert_eq!(p.shown_layer_set(), "Default Set");
        assert_eq!(Project::current_cad_layer(&d), DEFAULT_CAD_LAYER_NAME);
        // And on to the second: the edit made while Big was active is kept.
        assert!(p.default_set_activate(&mut d, "Eighth"));
        assert_eq!(d.active_dimension_set, "1/8\" Scale");
        assert_eq!(p.annot_defaults.text.height, 9.0);
        assert_eq!(p.shown_layer_set(), "Dimmed");
        assert_eq!(Project::current_cad_layer(&d), "CAD, Detail");
        // Changing one member leaves "Using Active Defaults".
        p.saved_activate(&mut d, SavedKind::Text, "Default");
        assert_eq!(p.using_default_set(&mut d), None);
        assert!(!p.default_set_activate(&mut d, "Nope"));
    }

    #[test]
    fn default_set_names_are_unique_and_rename_and_delete_follow_into_views() {
        let (mut p, mut d) = fresh();
        p.default_set_save_new(&mut d, "A").unwrap();
        assert!(p.default_set_save_new(&mut d, "A").is_err());
        assert!(p.default_set_save_new(&mut d, " ").is_err());
        p.default_set_copy("A", "B").unwrap();
        assert!(p.default_set_copy("A", "B").is_err());
        assert!(p.default_set_rename("B", "a").is_err(), "case-insensitive");
        p.plan_views[0].spec.default_set = "B".into();
        p.default_set_rename("B", "C").unwrap();
        assert_eq!(p.plan_views[0].spec.default_set, "C");
        p.default_set_delete("C").unwrap();
        assert_eq!(p.plan_views[0].spec.default_set, "");
        assert!(p.default_set_delete("C").is_err());
    }

    #[test]
    fn a_saved_view_restores_its_defaults_on_opening() {
        let (mut p, mut d) = fresh();
        p.saved_copy(&mut d, SavedKind::RichText, "Default", "Plot")
            .unwrap();
        p.saved_activate(&mut d, SavedKind::RichText, "Plot");
        d.set_active_dimension_set("Plot Plan");
        let name = p.active_plan_view.clone();
        p.view_capture_defaults(&mut d, &name);
        // Switch everything away, then open the view.
        p.saved_activate(&mut d, SavedKind::RichText, "Default");
        d.set_active_dimension_set("1/4\" Scale");
        p.view_apply_defaults(&mut d, &name);
        assert_eq!(p.saved_active(&mut d, SavedKind::RichText), "Plot");
        assert_eq!(d.active_dimension_set, "Plot Plan");
        // A view saved with a Default Set activates the set.
        p.default_set_save_new(&mut d, "Plot Set").unwrap();
        p.view_capture_defaults(&mut d, &name);
        assert_eq!(p.plan_views[0].spec.default_set, "Plot Set");
        p.saved_activate(&mut d, SavedKind::RichText, "Default");
        p.view_apply_defaults(&mut d, &name);
        assert_eq!(p.saved_active(&mut d, SavedKind::RichText), "Plot");
        assert_eq!(p.saved_defaults.active_set, "Plot Set");
    }

    #[test]
    fn storing_a_pasted_default_recreates_it_once() {
        let (mut p, mut d) = fresh();
        let mut m = BTreeMap::new();
        m.insert("revision_clouds.radius".to_string(), PageValue::Num(3.0));
        let kind = SavedKind::RevisionClouds;
        let a = p.saved_store(&mut d, kind, "Pasted", SavedValue::Values(m.clone()), false);
        assert_eq!(a, "Pasted");
        // The same name again is renamed, not duplicated over the first.
        let b = p.saved_store(&mut d, kind, "Pasted", SavedValue::Values(m.clone()), false);
        assert_eq!(b, "Pasted 2");
        // Replace keeps one.
        let c = p.saved_store(&mut d, kind, "Pasted", SavedValue::Values(m), true);
        assert_eq!(c, "Pasted");
        assert_eq!(p.saved_names(&mut d, kind).len(), 3);
    }

    #[test]
    fn the_store_round_trips_through_the_plan_file() {
        let (mut p, mut d) = fresh();
        p.saved_copy(&mut d, SavedKind::Markers, "Default", "M2")
            .unwrap();
        p.default_set_save_new(&mut d, "S").unwrap();
        p.saved_defaults.note_use(SavedKind::Markers, 3, "M2");
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.saved_defaults, p.saved_defaults);
        // An old file has none.
        let old = Project::new("old");
        assert!(old.saved_defaults.is_empty());
    }
}
