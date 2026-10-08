//! Chief-style layer sets and saved plan views.
//!
//! A *layer set* is a named table of per-layer overrides (display, lock,
//! colour, line weight, line style, text style) laid over the base layer
//! definitions of a project ([`LayerSet`]). A *saved plan view* is a named
//! view that carries its own layer set, floor, reference display and camera,
//! like Chief's "Floor Plan View Dimensioned".

use crate::geometry::Point;
use crate::layers::{LayerSet, LineStyle};
use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Name of the layer set a new project starts with.
pub const DEFAULT_LAYER_SET_NAME: &str = "Default Set";
/// Name of the saved plan view a new project starts with.
pub const DEFAULT_PLAN_VIEW_NAME: &str = "Floor Plan View";

fn default_true() -> bool {
    true
}

/// How one layer looks inside one layer set. `None` fields inherit the base
/// layer definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerState {
    pub layer: String,
    #[serde(default = "default_true")]
    pub display: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Hundredths of a millimetre.
    #[serde(default)]
    pub line_weight: Option<u32>,
    #[serde(default)]
    pub line_style: Option<LineStyle>,
    #[serde(default)]
    pub text_style: Option<String>,
}

impl LayerState {
    /// A state that only sets display and lock; everything else inherits.
    pub fn new(layer: impl Into<String>, display: bool, locked: bool) -> Self {
        Self {
            layer: layer.into(),
            display,
            locked,
            color: None,
            line_weight: None,
            line_style: None,
            text_style: None,
        }
    }
}

/// One change to a layer's look inside a layer set: a cell of the Layer
/// Display Options table.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerEdit {
    Display(bool),
    Locked(bool),
    /// The Ref box: the layer's objects show on a reference floor.
    Reference(bool),
    Color([u8; 3]),
    /// Hundredths of a millimetre.
    LineWeight(u32),
    LineStyle(LineStyle),
    /// Text style name; empty = the default style.
    TextStyle(String),
}

/// A named layer set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSetDef {
    pub name: String,
    #[serde(default)]
    pub states: Vec<LayerState>,
    /// Layers whose Ref box this set sets (the rest inherit the layer).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reference: BTreeMap<String, bool>,
}

impl LayerSetDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            states: Vec::new(),
            reference: BTreeMap::new(),
        }
    }

    /// Applies one table edit to `layer` in this set.
    pub fn apply(&mut self, layer: &str, edit: &LayerEdit) {
        if let LayerEdit::Reference(on) = edit {
            self.reference.insert(layer.to_string(), *on);
            return;
        }
        let st = self.ensure_state(layer);
        match edit {
            LayerEdit::Display(on) => st.display = *on,
            LayerEdit::Locked(on) => st.locked = *on,
            LayerEdit::Color(c) => st.color = Some(*c),
            LayerEdit::LineWeight(w) => st.line_weight = Some(*w),
            LayerEdit::LineStyle(ls) => st.line_style = Some(*ls),
            LayerEdit::TextStyle(t) => st.text_style = Some(t.clone()),
            LayerEdit::Reference(_) => {}
        }
    }

    /// Puts `layer` back to the base layer's look: shown and locked as the
    /// base says, no colour, weight, style or reference override.
    pub fn reset_layer(&mut self, layer: &str, base: &LayerSet) {
        self.reference.remove(layer);
        let (display, locked) = base
            .get(layer)
            .map_or((true, false), |l| (l.display, l.locked));
        let st = self.ensure_state(layer);
        *st = LayerState::new(layer, display, locked);
    }

    pub fn state(&self, layer: &str) -> Option<&LayerState> {
        self.states.iter().find(|s| s.layer == layer)
    }

    pub fn state_mut(&mut self, layer: &str) -> Option<&mut LayerState> {
        self.states.iter_mut().find(|s| s.layer == layer)
    }

    /// The state of `layer`, added (visible, unlocked) when missing.
    pub fn ensure_state(&mut self, layer: &str) -> &mut LayerState {
        if let Some(i) = self.states.iter().position(|s| s.layer == layer) {
            return &mut self.states[i];
        }
        self.states.push(LayerState::new(layer, true, false));
        self.states.last_mut().expect("just pushed")
    }
}

/// All layer sets of a project (or of the plan defaults) and which is active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerSets {
    pub sets: Vec<LayerSetDef>,
    /// Name of the active set.
    pub active: String,
}

impl Default for LayerSets {
    /// One set, "Default Set", matching the default floor-plan layers.
    fn default() -> Self {
        Self::from_layers(&LayerSet::default_floor_plan())
    }
}

impl LayerSets {
    /// One "Default Set" whose states mirror `layers` (display and lock only;
    /// colour, weight and styles inherit).
    pub fn from_layers(layers: &LayerSet) -> Self {
        Self {
            sets: vec![LayerSetDef {
                name: DEFAULT_LAYER_SET_NAME.into(),
                states: layers
                    .layers
                    .iter()
                    .map(|l| LayerState::new(l.name.clone(), l.display, l.locked))
                    .collect(),
                reference: BTreeMap::new(),
            }],
            active: DEFAULT_LAYER_SET_NAME.into(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&LayerSetDef> {
        self.sets.iter().find(|s| s.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut LayerSetDef> {
        self.sets.iter_mut().find(|s| s.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.sets.iter().map(|s| s.name.as_str()).collect()
    }

    pub fn active_set(&self) -> Option<&LayerSetDef> {
        self.get(&self.active)
    }

    /// Makes `name` the active set; `false` if it does not exist.
    pub fn set_active(&mut self, name: &str) -> bool {
        if self.get(name).is_none() {
            return false;
        }
        self.active = name.to_string();
        true
    }

    /// Adds a set if its name is new and non-empty.
    pub fn add_set(&mut self, set: LayerSetDef) -> bool {
        if set.name.is_empty() || self.get(&set.name).is_some() {
            return false;
        }
        self.sets.push(set);
        true
    }

    /// Shows or hides `layer` in `set` (adding the state if needed). `false`
    /// if the set does not exist.
    pub fn set_display(&mut self, set: &str, layer: &str, display: bool) -> bool {
        match self.get_mut(set) {
            Some(s) => {
                s.ensure_state(layer).display = display;
                true
            }
            None => false,
        }
    }

    /// Locks or unlocks `layer` in `set`. `false` if the set does not exist.
    pub fn set_locked(&mut self, set: &str, layer: &str, locked: bool) -> bool {
        match self.get_mut(set) {
            Some(s) => {
                s.ensure_state(layer).locked = locked;
                true
            }
            None => false,
        }
    }

    /// The base layers with the states of set `set_name` laid over them.
    /// Layers without a state keep the base values; states naming unknown
    /// layers are ignored; an unknown set changes nothing.
    pub fn effective_for(&self, set_name: &str, layers: &LayerSet) -> LayerSet {
        let mut out = layers.clone();
        let Some(set) = self.get(set_name) else {
            return out;
        };
        for st in &set.states {
            let Some(l) = out.get_mut(&st.layer) else {
                continue;
            };
            l.display = st.display;
            l.locked = st.locked;
            if let Some(c) = st.color {
                l.color = c;
            }
            if let Some(w) = st.line_weight {
                l.line_weight = w;
            }
            if let Some(ls) = st.line_style {
                l.line_style = ls;
            }
            if let Some(t) = &st.text_style {
                l.text_style = t.clone();
            }
        }
        for (name, on) in &set.reference {
            if let Some(l) = out.get_mut(name) {
                l.reference = *on;
            }
        }
        out
    }

    /// Does set `name` change how any layer looks from the base layers? With
    /// `flags` off, display and lock do not count (the plan reads them from
    /// the base layers while the plain "Default Set" is shown).
    pub fn set_differs_from(&self, name: &str, base: &LayerSet, flags: bool) -> bool {
        let Some(set) = self.get(name) else {
            return false;
        };
        let reference = set
            .reference
            .iter()
            .any(|(n, on)| base.get(n).is_some_and(|l| l.reference != *on));
        reference
            || set.states.iter().any(|st| {
                let Some(l) = base.get(&st.layer) else {
                    return false;
                };
                (flags && (l.display != st.display || l.locked != st.locked))
                    || st.color.is_some_and(|c| c != l.color)
                    || st.line_weight.is_some_and(|w| w != l.line_weight)
                    || st.line_style.is_some_and(|s| s != l.line_style)
                    || st.text_style.as_ref().is_some_and(|t| *t != l.text_style)
            })
    }

    /// Applies `edit` to each of `layers`: in the set named `set_name`, or in
    /// every set when `all_sets` is on ("Modify All Layer Sets"). Returns how
    /// many (set, layer) cells were written.
    pub fn edit_layers(
        &mut self,
        set_name: &str,
        layers: &[String],
        edit: &LayerEdit,
        all_sets: bool,
    ) -> usize {
        let mut n = 0;
        for set in &mut self.sets {
            if !all_sets && set.name != set_name {
                continue;
            }
            for l in layers {
                set.apply(l, edit);
                n += 1;
            }
        }
        n
    }

    /// Resets `layers` of the set named `set` (every set when `all_sets`) to
    /// the base layers' look. Returns how many cells were reset.
    pub fn reset_layers(
        &mut self,
        layers: &[String],
        base: &LayerSet,
        set: &str,
        all_sets: bool,
    ) -> usize {
        let mut n = 0;
        for def in &mut self.sets {
            if !all_sets && def.name != set {
                continue;
            }
            for l in layers {
                def.reset_layer(l, base);
                n += 1;
            }
        }
        n
    }

    /// Copies how `layers` look in set `from` into each set of `to` ("Copy
    /// to other sets"). Returns how many cells were written; a missing `from`
    /// writes none.
    pub fn copy_layers_to(&mut self, from: &str, layers: &[String], to: &[String]) -> usize {
        let Some(src) = self.get(from).cloned() else {
            return 0;
        };
        let mut n = 0;
        for dst in &mut self.sets {
            if dst.name == from || !to.contains(&dst.name) {
                continue;
            }
            for l in layers {
                let st = src
                    .state(l)
                    .cloned()
                    .unwrap_or_else(|| LayerState::new(l.clone(), true, false));
                match dst.state_mut(l) {
                    Some(d) => *d = st,
                    None => dst.states.push(st),
                }
                match src.reference.get(l) {
                    Some(r) => {
                        dst.reference.insert(l.clone(), *r);
                    }
                    None => {
                        dst.reference.remove(l);
                    }
                }
                n += 1;
            }
        }
        n
    }

    /// A name for a copy of a set: `name` itself when free, else
    /// "name (2)", "name (3)", ...
    pub fn free_name(&self, name: &str) -> String {
        let base = if name.is_empty() { "Layer Set" } else { name };
        if self.get(base).is_none() {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} ({n})"))
            .find(|c| self.get(c).is_none())
            .expect("an unused name exists")
    }

    /// Brings the sets named `names` of another plan into this one (Layer Set
    /// Management > Import). Layers the other plan has and this one lacks are
    /// added to `base` first. A set whose name is taken arrives as "name (2)".
    /// Returns the names the sets were given here.
    pub fn import_sets(
        &mut self,
        other: &LayerSets,
        other_base: &LayerSet,
        names: &[String],
        base: &mut LayerSet,
    ) -> Vec<String> {
        let mut added = Vec::new();
        for name in names {
            let Some(src) = other.get(name) else {
                continue;
            };
            for st in &src.states {
                if base.get(&st.layer).is_none() {
                    if let Some(l) = other_base.get(&st.layer) {
                        base.add(l.clone());
                    }
                }
            }
            let mut def = src.clone();
            def.name = self.free_name(name);
            added.push(def.name.clone());
            self.sets.push(def);
        }
        added
    }

    /// [`effective_for`](Self::effective_for) the active set.
    pub fn effective(&self, layers: &LayerSet) -> LayerSet {
        self.effective_for(&self.active, layers)
    }

    /// Copies set `from` under `new_name`. `false` if `from` is missing or
    /// `new_name` is empty or taken.
    pub fn copy_set(&mut self, from: &str, new_name: &str) -> bool {
        if new_name.is_empty() || self.get(new_name).is_some() {
            return false;
        }
        let Some(src) = self.get(from) else {
            return false;
        };
        let mut copy = src.clone();
        copy.name = new_name.to_string();
        self.sets.push(copy);
        true
    }

    /// Renames a set (and the active pointer). `false` if `old` is missing or
    /// `new_name` is empty or taken.
    pub fn rename(&mut self, old: &str, new_name: &str) -> bool {
        if new_name.is_empty() || self.get(new_name).is_some() {
            return false;
        }
        let Some(s) = self.get_mut(old) else {
            return false;
        };
        s.name = new_name.to_string();
        if self.active == old {
            self.active = new_name.to_string();
        }
        true
    }

    /// Deletes a set. The last set cannot be deleted; deleting the active set
    /// activates the first remaining one. `false` if nothing was deleted.
    pub fn delete(&mut self, name: &str) -> bool {
        if self.sets.len() <= 1 {
            return false;
        }
        let Some(i) = self.sets.iter().position(|s| s.name == name) else {
            return false;
        };
        self.sets.remove(i);
        if self.active == name {
            self.active = self.sets[0].name.clone();
        }
        true
    }
}

/// A saved plan view: a layer set plus what the view looks at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPlanView {
    pub name: String,
    /// Name of an entry of [`LayerSets::sets`].
    pub layer_set: String,
    /// Floor index the view shows; `None` = whichever floor is current.
    #[serde(default)]
    pub floor: Option<usize>,
    #[serde(default)]
    pub reference_display: bool,
    /// Floor shown as reference, relative to the viewed floor (-1 = the one
    /// below).
    #[serde(default)]
    pub reference_floor: Option<i32>,
    /// Saved view centre and zoom.
    #[serde(default)]
    pub camera: Option<(Point, f64)>,
    /// Name of the dimension default set the view uses (Plan View
    /// Specification); empty = leave the active one.
    #[serde(default)]
    pub dimension_defaults: String,
    /// Name of the text style new text takes in this view; empty = leave it.
    #[serde(default)]
    pub text_style: String,
}

impl SavedPlanView {
    pub fn new(name: impl Into<String>, layer_set: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layer_set: layer_set.into(),
            floor: None,
            reference_display: false,
            reference_floor: None,
            camera: None,
            dimension_defaults: String::new(),
            text_style: String::new(),
        }
    }

    /// Stores what the view looks at right now: the floor, whether the
    /// reference floor shows, and the camera centre and zoom (Save Plan View).
    pub fn capture(&mut self, floor: usize, reference_display: bool, camera: Option<(Point, f64)>) {
        self.floor = Some(floor);
        self.reference_display = reference_display;
        if camera.is_some() {
            self.camera = camera;
        }
    }

    /// The views a new project starts with.
    pub fn defaults() -> Vec<SavedPlanView> {
        vec![SavedPlanView::new(
            DEFAULT_PLAN_VIEW_NAME,
            DEFAULT_LAYER_SET_NAME,
        )]
    }
}

/// The saved plan views of Daniel's working template and the layer set each
/// one shows (`docs/daniel-template-inventory.md`).
pub const TEMPLATE_PLAN_VIEWS: &[(&str, &str)] = &[
    ("Presentation Plan View", "Presentation Layer Set"),
    ("Working Plan View", "Working Layer Set"),
    ("Electrical Plan View", "Electrical Layer Set"),
    (
        "Floor Plan View Dimensioned",
        "Floor Plan Dimensioned Layer Set",
    ),
    ("Floor Plan View Shell", "Floor Plan Shell Layer Set"),
    ("Foundation Plan View", "Foundation Layer Set"),
    (
        "Foundation Plan View Dimensioned",
        "Foundation Plan Dimensioned Layer Set",
    ),
    ("Framing, Ceiling Plan View", "Framing, Ceiling Layer Set"),
    ("Framing, Floor Plan View", "Framing, Floor Layer Set"),
    ("Framing, Porch Plan View", "Framing Porch Layer Set"),
    ("Framing, Porch Roof Plan View", "Framing Porch Layer Set"),
    ("Framing, Roof Plan View", "Framing, Roof Layer Set"),
    ("HVAC Plan View", "HVAC Layer Set"),
    ("Kitchen and Bath Plan View", "Kitchen and Bath Layer Set"),
    ("Plot Plan View", "Plot Plan Layer Set"),
    ("Roof Plan View", "Roof Plan Layer Set"),
    ("Square Footage View", "Square Footage Layer Set"),
    ("Structural Steel Plan View", "Steel Framing Layer Set"),
    ("WINDWOS/ DOORS PLAN VIEW", "Window Schedule Layer Set"),
    ("DWG Export Plan View", "DWG Export Layer Set"),
];

pub(crate) fn default_active_plan_view() -> String {
    DEFAULT_PLAN_VIEW_NAME.into()
}

impl Project {
    pub fn plan_view(&self, name: &str) -> Option<&SavedPlanView> {
        self.plan_views.iter().find(|v| v.name == name)
    }

    /// The active saved plan view, if it exists.
    pub fn current_plan_view(&self) -> Option<&SavedPlanView> {
        self.plan_view(&self.active_plan_view)
    }

    /// Makes `name` the active plan view and, when its layer set exists,
    /// the active layer set. `false` if the view does not exist.
    pub fn activate_plan_view(&mut self, name: &str) -> bool {
        let Some(v) = self.plan_view(name) else {
            return false;
        };
        let set = v.layer_set.clone();
        self.active_plan_view = name.to_string();
        self.layer_sets.set_active(&set);
        true
    }

    /// The layers as the active plan view shows them: the view's layer set
    /// (or the active set when there is no such view) over the base layers.
    pub fn view_layers(&self) -> LayerSet {
        match self.current_plan_view() {
            Some(v) if self.layer_sets.get(&v.layer_set).is_some() => {
                self.layer_sets.effective_for(&v.layer_set, &self.layers)
            }
            _ => self.layer_sets.effective(&self.layers),
        }
    }

    /// The layer set the plan shows and the Layer Display table edits: the
    /// active plan view's set while it exists, else the active set.
    pub fn shown_layer_set(&self) -> &str {
        match self.current_plan_view() {
            Some(v) if self.layer_sets.get(&v.layer_set).is_some() => &v.layer_set,
            _ => &self.layer_sets.active,
        }
    }

    /// Shows layer set `name`: makes it the active set and the set of the
    /// active plan view. `false` if there is no such set.
    pub fn show_layer_set(&mut self, name: &str) -> bool {
        if !self.layer_sets.set_active(name) {
            return false;
        }
        let active = self.active_plan_view.clone();
        if let Some(v) = self.plan_views.iter_mut().find(|v| v.name == active) {
            v.layer_set = name.to_string();
        }
        true
    }

    /// Edits `layers` in the shown layer set (every set when `all_sets`,
    /// "Modify All Layer Sets"). While the plain "Default Set" is among the
    /// edited sets, display and lock also follow into the base layers, which
    /// the plan reads for them (and the camera layer reads always). Returns
    /// how many cells were written.
    pub fn edit_layers(&mut self, layers: &[String], edit: &LayerEdit, all_sets: bool) -> usize {
        let shown = self.shown_layer_set().to_string();
        let n = self.layer_sets.edit_layers(&shown, layers, edit, all_sets);
        let base_follows = (all_sets || shown == DEFAULT_LAYER_SET_NAME)
            && self.layer_sets.get(DEFAULT_LAYER_SET_NAME).is_some();
        if base_follows {
            for name in layers {
                if let Some(l) = self.layers.get_mut(name) {
                    match edit {
                        LayerEdit::Display(on) => l.display = *on,
                        LayerEdit::Locked(on) => l.locked = *on,
                        _ => {}
                    }
                }
            }
        }
        n
    }

    /// Resets `layers` to the base layers' look in the shown set (every set
    /// when `all_sets`).
    pub fn reset_layers(&mut self, layers: &[String], all_sets: bool) -> usize {
        let shown = self.shown_layer_set().to_string();
        self.layer_sets
            .reset_layers(layers, &self.layers, &shown, all_sets)
    }

    /// Does the shown layer set or plan view change how the layers look from
    /// the base layers (so the plan reads [`view_layers`](Self::view_layers))?
    /// The plain "Default Set" in the starting view reads the base layers for
    /// display and lock and only needs the overlay when it sets a colour,
    /// weight, style or Ref box.
    pub fn layer_view_differs(&self) -> bool {
        let shown = self.shown_layer_set();
        if self.active_plan_view != DEFAULT_PLAN_VIEW_NAME
            || self.layer_sets.active != DEFAULT_LAYER_SET_NAME
            || shown != DEFAULT_LAYER_SET_NAME
        {
            return true;
        }
        self.layer_sets.set_differs_from(shown, &self.layers, false)
    }

    /// Adds a plan view if its name is new and non-empty. `false` otherwise.
    pub fn add_plan_view(&mut self, view: SavedPlanView) -> bool {
        if view.name.is_empty() || self.plan_view(&view.name).is_some() {
            return false;
        }
        self.plan_views.push(view);
        true
    }

    /// Renames a plan view (and the active pointer). `false` if `old` is
    /// missing or `new_name` is empty or taken.
    pub fn rename_plan_view(&mut self, old: &str, new_name: &str) -> bool {
        if new_name.is_empty() || self.plan_view(new_name).is_some() {
            return false;
        }
        let Some(v) = self.plan_views.iter_mut().find(|v| v.name == old) else {
            return false;
        };
        v.name = new_name.to_string();
        if self.active_plan_view == old {
            self.active_plan_view = new_name.to_string();
        }
        true
    }

    /// Deletes a plan view. The last one stays; deleting the active one
    /// activates the first remaining view. `false` if nothing was deleted.
    pub fn delete_plan_view(&mut self, name: &str) -> bool {
        if self.plan_views.len() <= 1 {
            return false;
        }
        let Some(i) = self.plan_views.iter().position(|v| v.name == name) else {
            return false;
        };
        self.plan_views.remove(i);
        if self.active_plan_view == name {
            let first = self.plan_views[0].name.clone();
            self.activate_plan_view(&first);
        }
        true
    }

    /// Copies a plan view under a free name ("name (2)", ...). Returns the new
    /// name.
    pub fn duplicate_plan_view(&mut self, name: &str) -> Option<String> {
        let src = self.plan_view(name)?.clone();
        let mut n = 2;
        let new_name = loop {
            let candidate = format!("{name} ({n})");
            if self.plan_view(&candidate).is_none() {
                break candidate;
            }
            n += 1;
        };
        let mut copy = src;
        copy.name = new_name.clone();
        self.plan_views.push(copy);
        Some(new_name)
    }

    /// Moves the plan view at index `from` so it sits at index `to` (drag to
    /// reorder). `false` if either index is out of range or they are equal.
    pub fn move_plan_view(&mut self, from: usize, to: usize) -> bool {
        let n = self.plan_views.len();
        if from >= n || to >= n || from == to {
            return false;
        }
        let v = self.plan_views.remove(from);
        self.plan_views.insert(to, v);
        true
    }

    /// Adds Daniel's template plan views ([`TEMPLATE_PLAN_VIEWS`]) that the
    /// plan lacks, each on its own layer set: the template's when the plan has
    /// it, else a copy of the active set under that name. Returns how many
    /// views were added.
    pub fn seed_template_plan_views(&mut self) -> usize {
        let mut added = 0;
        for (view, set) in TEMPLATE_PLAN_VIEWS {
            if self.plan_view(view).is_some() {
                continue;
            }
            if self.layer_sets.get(set).is_none() {
                let active = self.layer_sets.active.clone();
                if !self.layer_sets.copy_set(&active, set) {
                    continue;
                }
            }
            self.plan_views.push(SavedPlanView::new(*view, *set));
            added += 1;
        }
        added
    }

    /// Does the plan hold only the starting plan view (so the template views
    /// are "absent")?
    pub fn has_only_starting_plan_view(&self) -> bool {
        self.plan_views.len() == 1 && self.plan_views[0].name == DEFAULT_PLAN_VIEW_NAME
    }

    /// Points every place that names text style `old` at `new_name` (`None`
    /// clears it, so the thing falls back to its layer's or the default
    /// style): the plan's layers, the layer sets' overrides, the saved plan
    /// views, CAD text attributes, dimensions and placed schedules. Returns
    /// how many references changed.
    fn repoint_text_style(&mut self, old: &str, new_name: Option<&str>) -> usize {
        let mut n = 0;
        let as_string = |s: Option<&str>| s.unwrap_or("").to_string();
        for l in &mut self.layers.layers {
            if l.text_style == old {
                l.text_style = as_string(new_name);
                n += 1;
            }
        }
        for set in &mut self.layer_sets.sets {
            for st in &mut set.states {
                if st.text_style.as_deref() == Some(old) {
                    st.text_style = new_name.map(str::to_string);
                    n += 1;
                }
            }
        }
        for v in &mut self.plan_views {
            if v.text_style == old {
                v.text_style = as_string(new_name);
                n += 1;
            }
        }
        for f in &mut self.floors {
            for a in &mut f.cad_attrs {
                if a.text_style.as_deref() == Some(old) {
                    a.text_style = new_name.map(str::to_string);
                    n += 1;
                }
            }
            for d in &mut f.dimensions {
                if d.text_style.as_deref() == Some(old) {
                    d.text_style = new_name.map(str::to_string);
                    n += 1;
                }
            }
            let mut layer = crate::schedules::ScheduleLayer::load(f);
            let mut changed = false;
            for sc in &mut layer.schedules {
                if sc.text_style == old {
                    sc.text_style = as_string(new_name);
                    changed = true;
                    n += 1;
                }
            }
            if changed {
                layer.store(f);
            }
        }
        n
    }

    /// Renames text style `old` to `new_name` and everything that used it
    /// follows (see `repoint_text_style`), so a rename never leaves a layer
    /// quietly set in the default style. `false` when `old` does not exist,
    /// is the Default Text Style, or `new_name` is empty or taken.
    pub fn rename_text_style(&mut self, old: &str, new_name: &str) -> bool {
        let new_name = new_name.trim();
        if new_name.is_empty()
            || new_name == old
            || old == crate::text_styles::DEFAULT_TEXT_STYLE_NAME
            || self.text_styles.get(new_name).is_some()
        {
            return false;
        }
        let Some(style) = self.text_styles.styles.iter_mut().find(|s| s.name == old) else {
            return false;
        };
        style.name = new_name.to_string();
        self.repoint_text_style(old, Some(new_name));
        true
    }

    /// Removes text style `name`; the layers, overrides, views, texts,
    /// dimensions and schedules that used it go back to inheriting. `false`
    /// when it does not exist or is the Default Text Style.
    pub fn remove_text_style(&mut self, name: &str) -> bool {
        if !self.text_styles.remove(name) {
            return false;
        }
        self.repoint_text_style(name, None);
        true
    }

    /// Renames a layer set and repoints the plan views that used it.
    pub fn rename_layer_set(&mut self, old: &str, new_name: &str) -> bool {
        if !self.layer_sets.rename(old, new_name) {
            return false;
        }
        for v in self.plan_views.iter_mut().filter(|v| v.layer_set == old) {
            v.layer_set = new_name.to_string();
        }
        true
    }

    /// Deletes a layer set; plan views that used it fall back to the new
    /// active set.
    pub fn delete_layer_set(&mut self, name: &str) -> bool {
        if !self.layer_sets.delete(name) {
            return false;
        }
        let active = self.layer_sets.active.clone();
        for v in self.plan_views.iter_mut().filter(|v| v.layer_set == name) {
            v.layer_set = active.clone();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::Layer;

    fn sets_with_two() -> LayerSets {
        let mut s = LayerSets::default();
        assert!(s.copy_set(DEFAULT_LAYER_SET_NAME, "Electrical Layer Set"));
        s
    }

    #[test]
    fn default_has_one_set_matching_base_layers() {
        let base = LayerSet::default_floor_plan();
        let s = LayerSets::default();
        assert_eq!(s.sets.len(), 1);
        assert_eq!(s.active, "Default Set");
        assert_eq!(s.sets[0].states.len(), base.layers.len());
        assert_eq!(s.effective(&base), base);
    }

    #[test]
    fn effective_applies_display_lock_and_overrides() {
        let base = LayerSet::default_floor_plan();
        let mut s = sets_with_two();
        assert!(s.set_display("Electrical Layer Set", "Doors", false));
        assert!(s.set_locked("Electrical Layer Set", "Text", true));
        {
            let st = s
                .get_mut("Electrical Layer Set")
                .unwrap()
                .ensure_state("Walls, Normal");
            st.color = Some([1, 2, 3]);
            st.line_weight = Some(70);
            st.line_style = Some(LineStyle::Dashed);
            st.text_style = Some("Room Label Style".into());
        }
        // Not active yet: base is unchanged.
        assert!(s.effective(&base).is_visible("Doors"));
        assert!(s.set_active("Electrical Layer Set"));
        let eff = s.effective(&base);
        assert!(!eff.is_visible("Doors"));
        assert!(eff.is_locked("Text"));
        let w = eff.get("Walls, Normal").unwrap();
        assert_eq!(w.color, [1, 2, 3]);
        assert_eq!(w.line_weight, 70);
        assert_eq!(w.line_style, LineStyle::Dashed);
        assert_eq!(w.text_style, "Room Label Style");
        // The base layers are never mutated.
        assert!(base.is_visible("Doors"));
        // Layers without a state keep base values; unknown layers ignored.
        s.get_mut("Electrical Layer Set")
            .unwrap()
            .states
            .retain(|st| st.layer != "Windows");
        s.set_display("Electrical Layer Set", "No Such Layer", false);
        let eff = s.effective(&base);
        assert_eq!(eff.get("Windows"), base.get("Windows"));
        assert!(eff.get("No Such Layer").is_none());
        // Unknown set -> base unchanged; set_display on unknown set fails.
        assert_eq!(s.effective_for("nope", &base), base);
        assert!(!s.set_display("nope", "Doors", false));
        assert!(!s.set_locked("nope", "Doors", true));
        assert!(!s.set_active("nope"));
    }

    #[test]
    fn copy_rename_delete() {
        let mut s = LayerSets::default();
        assert!(!s.copy_set("nope", "X"));
        assert!(!s.copy_set(DEFAULT_LAYER_SET_NAME, DEFAULT_LAYER_SET_NAME));
        assert!(!s.copy_set(DEFAULT_LAYER_SET_NAME, ""));
        assert!(s.copy_set(DEFAULT_LAYER_SET_NAME, "A"));
        assert_eq!(s.get("A").unwrap().states, s.sets[0].states);
        // A copy is independent.
        s.set_display("A", "Doors", false);
        assert!(s.sets[0].state("Doors").unwrap().display);
        // Rename.
        assert!(!s.rename("A", DEFAULT_LAYER_SET_NAME));
        assert!(!s.rename("zzz", "B"));
        assert!(s.rename("A", "B"));
        assert!(s.get("A").is_none() && s.get("B").is_some());
        // Renaming the active set moves the pointer.
        assert!(s.rename(DEFAULT_LAYER_SET_NAME, "Main"));
        assert_eq!(s.active, "Main");
        // Delete.
        assert!(!s.delete("zzz"));
        assert!(s.delete("Main"));
        assert_eq!(s.active, "B");
        assert!(!s.delete("B"), "the last set stays");
        assert_eq!(s.sets.len(), 1);
        assert!(s.add_set(LayerSetDef::new("C")));
        assert!(!s.add_set(LayerSetDef::new("C")));
        assert!(!s.add_set(LayerSetDef::new("")));
        assert_eq!(s.names(), vec!["B", "C"]);
    }

    #[test]
    fn default_project_has_one_set_and_one_view() {
        let p = Project::new("P");
        assert_eq!(p.layer_sets.sets.len(), 1);
        assert_eq!(p.plan_views.len(), 1);
        assert_eq!(p.active_plan_view, DEFAULT_PLAN_VIEW_NAME);
        let v = p.current_plan_view().unwrap();
        assert_eq!(v.layer_set, p.layer_sets.active);
        assert_eq!(p.view_layers(), p.layers);
    }

    #[test]
    fn project_views_follow_their_layer_sets() {
        let mut p = Project::new("P");
        assert!(p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Dimmed"));
        p.layer_sets.set_display("Dimmed", "Doors", false);
        p.plan_views.push(SavedPlanView::new("Doors Off", "Dimmed"));
        assert!(!p.activate_plan_view("nope"));
        assert!(p.activate_plan_view("Doors Off"));
        assert_eq!(p.layer_sets.active, "Dimmed");
        assert!(!p.view_layers().is_visible("Doors"));
        assert!(p.activate_plan_view(DEFAULT_PLAN_VIEW_NAME));
        assert!(p.view_layers().is_visible("Doors"));
        // Rename repoints views; delete falls back to the active set.
        assert!(p.rename_layer_set("Dimmed", "Quiet"));
        assert_eq!(p.plan_view("Doors Off").unwrap().layer_set, "Quiet");
        assert!(p.delete_layer_set("Quiet"));
        assert_eq!(
            p.plan_view("Doors Off").unwrap().layer_set,
            DEFAULT_LAYER_SET_NAME
        );
        assert!(!p.delete_layer_set(DEFAULT_LAYER_SET_NAME));
    }

    #[test]
    fn json_round_trip_and_old_files() {
        let mut p = Project::new("P");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Second");
        let mut v = SavedPlanView::new("Cam", "Second");
        v.floor = Some(0);
        v.reference_display = true;
        v.reference_floor = Some(-1);
        v.camera = Some((Point { x: 10.0, y: 20.0 }, 1.5));
        p.plan_views.push(v.clone());
        p.active_plan_view = "Cam".into();
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.layer_sets, p.layer_sets);
        assert_eq!(back.plan_views, p.plan_views);
        assert_eq!(back.active_plan_view, "Cam");
        assert_eq!(back.plan_view("Cam"), Some(&v));

        // An old file without any of the new fields still loads.
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = val.as_object_mut().unwrap();
        for k in [
            "layer_sets",
            "plan_views",
            "active_plan_view",
            "text_styles",
        ] {
            assert!(obj.remove(k).is_some(), "{k} missing from output");
        }
        let old: Project = serde_json::from_value(val).unwrap();
        assert_eq!(old.layer_sets.sets.len(), 1);
        assert_eq!(old.plan_views.len(), 1);
        assert_eq!(old.active_plan_view, DEFAULT_PLAN_VIEW_NAME);
        assert!(!old.text_styles.styles.is_empty());

        // Sparse layer state JSON.
        let st: LayerState = serde_json::from_str(r#"{"layer":"A"}"#).unwrap();
        assert!(st.display && !st.locked && st.color.is_none());
    }

    #[test]
    fn from_layers_mirrors_flags() {
        let mut base = LayerSet::default_floor_plan();
        base.set_display("Doors", false);
        base.set_locked("Text", true);
        base.add(Layer::new("Extra", [1, 1, 1], 5));
        let s = LayerSets::from_layers(&base);
        let d = s.active_set().unwrap();
        assert!(!d.state("Doors").unwrap().display);
        assert!(d.state("Text").unwrap().locked);
        assert!(d.state("Extra").is_some());
    }

    fn names(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn table_edits_stay_in_their_own_set() {
        let mut p = Project::new("P");
        assert!(p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Second"));
        let walls = names(&["Walls, Normal"]);
        // Edit the active (Default) set.
        assert_eq!(
            p.edit_layers(&walls, &LayerEdit::Color([9, 8, 7]), false),
            1
        );
        p.edit_layers(&walls, &LayerEdit::LineWeight(70), false);
        p.edit_layers(&walls, &LayerEdit::LineStyle(LineStyle::Dotted), false);
        p.edit_layers(
            &walls,
            &LayerEdit::TextStyle("Room Label Style".into()),
            false,
        );
        p.edit_layers(&walls, &LayerEdit::Reference(false), false);
        let a = p
            .layer_sets
            .effective_for(DEFAULT_LAYER_SET_NAME, &p.layers);
        let w = a.get("Walls, Normal").unwrap();
        assert_eq!(w.color, [9, 8, 7]);
        assert_eq!(w.line_weight, 70);
        assert_eq!(w.line_style, LineStyle::Dotted);
        assert_eq!(w.text_style, "Room Label Style");
        assert!(!w.reference);
        // The other set and the base layers are untouched.
        let b = p.layer_sets.effective_for("Second", &p.layers);
        assert_eq!(b.get("Walls, Normal"), p.layers.get("Walls, Normal"));
        assert_eq!(p.layers.get("Walls, Normal").unwrap().color, [0, 0, 0]);
        // Edit the other set once it is active: the first set keeps its look.
        assert!(p.show_layer_set("Second"));
        assert_eq!(p.current_plan_view().unwrap().layer_set, "Second");
        p.edit_layers(&walls, &LayerEdit::Color([1, 1, 1]), false);
        let a = p
            .layer_sets
            .effective_for(DEFAULT_LAYER_SET_NAME, &p.layers);
        assert_eq!(a.get("Walls, Normal").unwrap().color, [9, 8, 7]);
        let b = p.layer_sets.effective_for("Second", &p.layers);
        assert_eq!(b.get("Walls, Normal").unwrap().color, [1, 1, 1]);
        assert!(p
            .layer_sets
            .set_differs_from(p.shown_layer_set(), &p.layers, true));
    }

    #[test]
    fn display_and_lock_edits_reach_the_base_layers_too() {
        let mut p = Project::new("P");
        p.edit_layers(&names(&["Doors"]), &LayerEdit::Display(false), false);
        p.edit_layers(&names(&["Text"]), &LayerEdit::Locked(true), false);
        assert!(!p.layers.is_visible("Doors") && p.layers.is_locked("Text"));
        let eff = p.layer_sets.effective(&p.layers);
        assert!(!eff.is_visible("Doors") && eff.is_locked("Text"));
    }

    #[test]
    fn modify_all_layer_sets_changes_every_set() {
        let mut p = Project::new("P");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "B");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "C");
        let two = names(&["Doors", "Windows"]);
        assert_eq!(p.edit_layers(&two, &LayerEdit::Display(false), true), 6);
        for set in ["Default Set", "B", "C"] {
            let eff = p.layer_sets.effective_for(set, &p.layers);
            assert!(
                !eff.is_visible("Doors") && !eff.is_visible("Windows"),
                "{set}"
            );
            assert!(eff.is_visible("Rooms"), "{set}");
        }
        // Without the toggle only the active set changes.
        assert_eq!(
            p.edit_layers(&names(&["Rooms"]), &LayerEdit::Locked(true), false),
            1
        );
        assert!(p
            .layer_sets
            .effective_for("Default Set", &p.layers)
            .is_locked("Rooms"));
        assert!(!p
            .layer_sets
            .effective_for("B", &p.layers)
            .is_locked("Rooms"));
    }

    #[test]
    fn reset_and_copy_to_other_sets() {
        let mut p = Project::new("P");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "B");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "C");
        let l = names(&["Rooms"]);
        p.edit_layers(&l, &LayerEdit::Color([5, 5, 5]), false);
        p.edit_layers(&l, &LayerEdit::Reference(false), false);
        // Copy the look to B only.
        assert_eq!(
            p.layer_sets
                .copy_layers_to("Default Set", &l, &names(&["B", "Default Set"])),
            1
        );
        let b = p.layer_sets.effective_for("B", &p.layers);
        assert_eq!(b.get("Rooms").unwrap().color, [5, 5, 5]);
        assert!(!b.get("Rooms").unwrap().reference);
        let c = p.layer_sets.effective_for("C", &p.layers);
        assert_eq!(c.get("Rooms"), p.layers.get("Rooms"));
        // Reset in the active set only.
        assert_eq!(p.reset_layers(&l, false), 1);
        let a = p.layer_sets.effective_for("Default Set", &p.layers);
        assert_eq!(a.get("Rooms"), p.layers.get("Rooms"));
        assert_eq!(
            p.layer_sets
                .effective_for("B", &p.layers)
                .get("Rooms")
                .unwrap()
                .color,
            [5, 5, 5]
        );
        // Reset everywhere.
        p.reset_layers(&l, true);
        assert_eq!(
            p.layer_sets.effective_for("B", &p.layers).get("Rooms"),
            p.layers.get("Rooms")
        );
        assert!(!p
            .layer_sets
            .set_differs_from(p.shown_layer_set(), &p.layers, true));
        assert_eq!(p.layer_sets.copy_layers_to("nope", &l, &names(&["B"])), 0);
    }

    #[test]
    fn import_sets_adds_their_layers_and_renames_clashes() {
        let mut other = Project::new("O");
        other
            .layers
            .add(Layer::new("Imported Layer", [4, 4, 4], 20));
        other.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Theirs");
        other
            .layer_sets
            .get_mut("Theirs")
            .unwrap()
            .ensure_state("Imported Layer")
            .display = false;
        let mut p = Project::new("P");
        p.layer_sets.copy_set(DEFAULT_LAYER_SET_NAME, "Theirs");
        let added = p.layer_sets.import_sets(
            &other.layer_sets,
            &other.layers,
            &names(&["Theirs", "Nope"]),
            &mut p.layers,
        );
        assert_eq!(added, vec!["Theirs (2)"]);
        assert!(p.layers.get("Imported Layer").is_some());
        let eff = p.layer_sets.effective_for("Theirs (2)", &p.layers);
        assert!(!eff.is_visible("Imported Layer"));
        assert_eq!(p.layer_sets.free_name("Fresh"), "Fresh");
        assert_eq!(p.layer_sets.free_name(""), "Layer Set");
    }

    #[test]
    fn plan_view_list_edits() {
        let mut p = Project::new("P");
        assert!(p.add_plan_view(SavedPlanView::new("A", DEFAULT_LAYER_SET_NAME)));
        assert!(p.add_plan_view(SavedPlanView::new("B", DEFAULT_LAYER_SET_NAME)));
        assert!(!p.add_plan_view(SavedPlanView::new("A", DEFAULT_LAYER_SET_NAME)));
        assert!(!p.add_plan_view(SavedPlanView::new("", DEFAULT_LAYER_SET_NAME)));
        let order = |p: &Project| {
            p.plan_views
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(order(&p), vec!["Floor Plan View", "A", "B"]);
        assert!(p.move_plan_view(2, 0));
        assert_eq!(order(&p), vec!["B", "Floor Plan View", "A"]);
        assert!(!p.move_plan_view(1, 1) && !p.move_plan_view(0, 9));
        assert!(p.rename_plan_view("A", "A2"));
        assert!(!p.rename_plan_view("A2", "B") && !p.rename_plan_view("zzz", "Q"));
        p.activate_plan_view("A2");
        assert!(p.rename_plan_view("A2", "A3"));
        assert_eq!(p.active_plan_view, "A3");
        assert_eq!(p.duplicate_plan_view("A3").as_deref(), Some("A3 (2)"));
        assert_eq!(p.duplicate_plan_view("A3").as_deref(), Some("A3 (3)"));
        assert!(p.duplicate_plan_view("zzz").is_none());
        // Deleting the active view activates the first remaining one.
        assert!(p.delete_plan_view("A3"));
        assert_eq!(p.active_plan_view, "B");
        assert!(!p.delete_plan_view("zzz"));
        let mut one = Project::new("O");
        assert!(
            !one.delete_plan_view(DEFAULT_PLAN_VIEW_NAME),
            "the last view stays"
        );
    }

    #[test]
    fn template_plan_views_are_seeded_once_with_their_own_layer_sets() {
        let mut p = Project::new("P");
        assert!(p.has_only_starting_plan_view());
        assert_eq!(p.seed_template_plan_views(), 20);
        assert_eq!(p.plan_views.len(), 21);
        assert!(!p.has_only_starting_plan_view());
        let v = p.plan_view("Electrical Plan View").unwrap();
        assert_eq!(v.layer_set, "Electrical Layer Set");
        assert!(p.layer_sets.get("Electrical Layer Set").is_some());
        // Two views may share a set; nothing is added twice.
        assert_eq!(
            p.plan_view("Framing, Porch Roof Plan View")
                .unwrap()
                .layer_set,
            "Framing Porch Layer Set"
        );
        assert_eq!(p.seed_template_plan_views(), 0);
        // An existing template set is used, not copied over.
        let mut q = Project::new("Q");
        q.layer_sets
            .copy_set(DEFAULT_LAYER_SET_NAME, "Plot Plan Layer Set");
        q.layer_sets
            .set_display("Plot Plan Layer Set", "Doors", false);
        q.seed_template_plan_views();
        q.activate_plan_view("Plot Plan View");
        assert!(!q.view_layers().is_visible("Doors"));
    }

    #[test]
    fn plan_view_spec_fields_round_trip() {
        let mut v = SavedPlanView::new("Spec", DEFAULT_LAYER_SET_NAME);
        v.dimension_defaults = "1/4\" Scale Dimension Defaults".into();
        v.text_style = "1/4\" Text Style".into();
        v.capture(1, true, Some((Point { x: 5.0, y: 6.0 }, 2.5)));
        let json = serde_json::to_string(&v).unwrap();
        let back: SavedPlanView = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
        assert_eq!(back.floor, Some(1));
        assert!(back.reference_display);
        // A capture without a camera keeps the stored one.
        let mut w = back.clone();
        w.capture(0, false, None);
        assert_eq!(w.camera, Some((Point { x: 5.0, y: 6.0 }, 2.5)));
        // An old view without the new fields loads.
        let old: SavedPlanView = serde_json::from_str(r#"{"name":"O","layer_set":"S"}"#).unwrap();
        assert!(old.dimension_defaults.is_empty() && old.text_style.is_empty());
        // Layer set JSON without the reference map loads; empty map is not written.
        let def: LayerSetDef = serde_json::from_str(r#"{"name":"S"}"#).unwrap();
        assert!(def.reference.is_empty());
        assert!(!serde_json::to_string(&def).unwrap().contains("reference"));
    }

    #[test]
    fn renaming_a_text_style_repoints_everything_that_named_it() {
        use crate::text_styles::DEFAULT_TEXT_STYLE_NAME;
        let mut p = Project::new("t");
        let old = "1/4\" Text Style";
        p.layers.layers[0].text_style = old.into();
        p.layer_sets.sets[0].states[0].text_style = Some(old.into());
        p.plan_views[0].text_style = old.into();
        let id = p.add_cad(
            0,
            "Text",
            crate::CadItem::Text {
                pos: crate::Point::ZERO,
                text: "x".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        p.floors[0].cad_attrs.push(crate::cad::CadAttrs {
            target: id,
            text_style: Some(old.into()),
            ..crate::cad::CadAttrs::default()
        });
        let mut sched = crate::schedules::Schedule::new(
            crate::schedules::ScheduleKind::Door,
            crate::Point::ZERO,
        );
        sched.text_style = old.into();
        let mut layer = crate::schedules::ScheduleLayer::load(&p.floors[0]);
        layer.schedules.push(sched);
        layer.store(&mut p.floors[0]);

        assert!(p.rename_text_style(old, "Presentation Text"));
        assert!(
            p.text_styles.get(old).is_none() && p.text_styles.get("Presentation Text").is_some()
        );
        assert_eq!(p.layers.layers[0].text_style, "Presentation Text");
        assert_eq!(
            p.layer_sets.sets[0].states[0].text_style.as_deref(),
            Some("Presentation Text")
        );
        assert_eq!(p.plan_views[0].text_style, "Presentation Text");
        assert_eq!(
            p.floors[0].cad_attrs[0].text_style.as_deref(),
            Some("Presentation Text")
        );
        let layer = crate::schedules::ScheduleLayer::load(&p.floors[0]);
        assert_eq!(layer.schedules[0].text_style, "Presentation Text");
        // The default style and taken or empty names are refused.
        assert!(!p.rename_text_style(DEFAULT_TEXT_STYLE_NAME, "X"));
        assert!(!p.rename_text_style("Presentation Text", "Room Label Style"));
        assert!(!p.rename_text_style("Presentation Text", "  "));
        assert!(!p.rename_text_style("No Such Style", "Y"));
        // Removing one sends the things that used it back to inheriting.
        assert!(p.remove_text_style("Presentation Text"));
        assert_eq!(p.layers.layers[0].text_style, "");
        assert_eq!(p.layer_sets.sets[0].states[0].text_style, None);
        assert_eq!(p.plan_views[0].text_style, "");
        assert_eq!(p.floors[0].cad_attrs[0].text_style, None);
        let layer = crate::schedules::ScheduleLayer::load(&p.floors[0]);
        assert_eq!(layer.schedules[0].text_style, "");
        assert!(!p.remove_text_style(DEFAULT_TEXT_STYLE_NAME));
    }

    /// The 20 template plan views and the layer set each is paired with name
    /// things that exist in Daniel's template: the views are the template's
    /// saved plan views and every set is one of its 47 layer sets
    /// (`docs/daniel-template-inventory.md`). The pairing itself is by name:
    /// the saved view's link to its set is inside the binary plan and is not
    /// decoded.
    #[test]
    fn the_template_plan_view_pairings_name_things_in_the_template() {
        let doc = include_str!("../../../docs/daniel-template-inventory.md");
        let views = doc
            .split("### Saved plan views (20)")
            .nth(1)
            .and_then(|t| t.split("###").next())
            .expect("the inventory lists the saved plan views");
        let sets_line = doc
            .lines()
            .find(|l| l.contains("`Presentation Layer Set`") && l.contains("`Working Layer Set`"))
            .expect("the inventory lists the layer sets");
        assert_eq!(TEMPLATE_PLAN_VIEWS.len(), 20);
        for (view, set) in TEMPLATE_PLAN_VIEWS {
            assert!(views.contains(&format!("`{view}`")), "plan view {view}");
            assert!(sets_line.contains(&format!("`{set}`")), "layer set {set}");
        }
        // Each view is listed once; only the two porch views share a set.
        let mut names: Vec<&str> = TEMPLATE_PLAN_VIEWS.iter().map(|v| v.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 20);
        let mut sets: Vec<&str> = TEMPLATE_PLAN_VIEWS.iter().map(|v| v.1).collect();
        sets.sort_unstable();
        sets.dedup();
        assert_eq!(sets.len(), 19);
        // All 20 views of the template are covered.
        assert_eq!(views.matches('`').count(), 40);
    }
}
