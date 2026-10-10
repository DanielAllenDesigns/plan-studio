//! Chief-style layers: named groups of objects with display/lock state, a
//! colour and a plotted line weight.

use serde::{Deserialize, Serialize};

/// Line style of a layer (LAY ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub display: bool,
    pub locked: bool,
    /// RGB display colour.
    pub color: [u8; 3],
    /// Plotted line weight in hundredths of a millimetre (DXF convention).
    pub line_weight: u32,
    /// Name of the text style objects on this layer use; empty = default.
    #[serde(default)]
    pub text_style: String,
    /// Line style of the layer.
    #[serde(default)]
    pub line_style: LineStyle,
    /// The layer's objects show on a reference floor (Layer Display Options
    /// "Ref" column, LAY-10).
    #[serde(default = "reference_default")]
    pub reference: bool,
}

fn reference_default() -> bool {
    true
}

impl Layer {
    pub fn new(name: impl Into<String>, color: [u8; 3], line_weight: u32) -> Self {
        Self {
            name: name.into(),
            display: true,
            locked: false,
            color,
            line_weight,
            text_style: String::new(),
            line_style: LineStyle::Solid,
            reference: true,
        }
    }
}

/// System layer of door labels (DW-63).
pub const DOOR_LABEL_LAYER: &str = "Doors, Labels";
/// The layer cabinet labels are drawn on.
pub const CABINET_LABEL_LAYER: &str = "Cabinets, Labels";
/// The layer the dashed arcs between switches and the lights or outlets they
/// control are drawn on (Chief keeps the connections apart from the devices
/// on "Electrical").
pub const ELECTRICAL_CONNECTION_LAYER: &str = "Electrical Connection";
/// System layer of window labels (DW-63).
pub const WINDOW_LABEL_LAYER: &str = "Windows, Labels";

/// The wall system layers of Layer Display Options (LAY-76, manual p. 389):
/// the display switches for a wall's layer lines, for showing only the main
/// layer, for the lines that say which wall builds through at a corner, for
/// footings, brick ledge lines, walls that are not located by dimensions and
/// the walls of the attic.
pub const WALL_LAYERS_LAYER: &str = "Walls, Layers";
pub const WALL_MAIN_ONLY_LAYER: &str = "Walls, Main Layer Only";
pub const WALL_THROUGH_LINES_LAYER: &str = "Walls, Through Wall Lines";
pub const FOOTINGS_LAYER: &str = "Footings";
pub const BRICK_LEDGE_LAYER: &str = "Brick Ledge Lines";
pub const WALL_NO_LOCATE_LAYER: &str = "Walls, No Locate";
pub const WALL_ATTIC_LAYER: &str = "Walls, Attic";

/// One wall system layer: name, colour, line weight and whether a new plan
/// shows it. Only the layer lines are on at first (that is how plans have
/// always drawn walls); the rest are switches the user turns on.
#[derive(Debug, Clone, Copy)]
pub struct WallSystemLayer {
    pub name: &'static str,
    pub color: [u8; 3],
    pub weight: u32,
    pub shown: bool,
}

pub const WALL_SYSTEM_LAYERS: [WallSystemLayer; 7] = [
    WallSystemLayer {
        name: WALL_LAYERS_LAYER,
        color: [0, 0, 0],
        weight: 13,
        shown: true,
    },
    WallSystemLayer {
        name: WALL_MAIN_ONLY_LAYER,
        color: [0, 0, 0],
        weight: 25,
        shown: false,
    },
    WallSystemLayer {
        name: WALL_THROUGH_LINES_LAYER,
        color: [200, 0, 200],
        weight: 13,
        shown: false,
    },
    WallSystemLayer {
        name: FOOTINGS_LAYER,
        color: [100, 100, 100],
        weight: 25,
        shown: false,
    },
    WallSystemLayer {
        name: BRICK_LEDGE_LAYER,
        color: [150, 75, 0],
        weight: 13,
        shown: false,
    },
    WallSystemLayer {
        name: WALL_NO_LOCATE_LAYER,
        color: [150, 150, 150],
        weight: 13,
        shown: false,
    },
    WallSystemLayer {
        name: WALL_ATTIC_LAYER,
        color: [0, 100, 100],
        weight: 25,
        shown: false,
    },
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSet {
    pub name: String,
    pub layers: Vec<Layer>,
    /// The layer each tool draws on when it is not the tool's own default
    /// (Default Settings > Layers, "Current CAD Layer"). Only differences
    /// are stored.
    #[serde(default)]
    pub tool_layers: Vec<ToolLayer>,
}

/// One tool's chosen layer (see [`LayerSet::tool_layer`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLayer {
    pub tool: String,
    pub layer: String,
}

/// A tool that has an active layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolLayerInfo {
    /// Stable key stored in the plan.
    pub key: &'static str,
    /// Label in the Active Layer table.
    pub label: &'static str,
    /// The layer the tool draws on when nothing else is chosen.
    pub default_layer: &'static str,
}

/// Key of the Current CAD Layer entry (LAY-6, CAD-1).
pub const CAD_TOOL: &str = "cad";

/// Every tool with an active layer, in the order the table lists them. The
/// CAD entry is the toolbar's "Current CAD Layer".
pub const TOOL_LAYERS: &[ToolLayerInfo] = &[
    ToolLayerInfo {
        key: CAD_TOOL,
        label: "CAD (Current CAD Layer)",
        default_layer: "CAD, Default",
    },
    ToolLayerInfo {
        key: "text",
        label: "Text",
        default_layer: "Text",
    },
    ToolLayerInfo {
        key: "dimensions",
        label: "Manual Dimensions",
        default_layer: "Dimensions, Manual",
    },
    ToolLayerInfo {
        key: "dimensions_auto",
        label: "Automatic Dimensions",
        default_layer: "Dimensions, Automatic",
    },
    ToolLayerInfo {
        key: "walls_exterior",
        label: "Exterior Walls",
        default_layer: "Walls, Normal",
    },
    ToolLayerInfo {
        key: "walls_interior",
        label: "Interior Walls",
        default_layer: "Walls, Normal",
    },
    ToolLayerInfo {
        key: "doors",
        label: "Doors",
        default_layer: "Doors",
    },
    ToolLayerInfo {
        key: "windows",
        label: "Windows",
        default_layer: "Windows",
    },
    ToolLayerInfo {
        key: "cabinets_base",
        label: "Base Cabinets",
        default_layer: "Cabinets, Base",
    },
    ToolLayerInfo {
        key: "cabinets_wall",
        label: "Wall Cabinets",
        default_layer: "Cabinets, Wall",
    },
    ToolLayerInfo {
        key: "electrical",
        label: "Electrical",
        default_layer: "Electrical",
    },
    ToolLayerInfo {
        key: "stairs",
        label: "Stairs",
        default_layer: "Stairs",
    },
    ToolLayerInfo {
        key: "roof",
        label: "Roof Planes",
        default_layer: "Roof Planes",
    },
    ToolLayerInfo {
        key: "framing",
        label: "Framing",
        default_layer: "Framing",
    },
];

impl LayerSet {
    /// The default floor-plan layer set.
    pub fn default_floor_plan() -> LayerSet {
        let l = Layer::new;
        LayerSet {
            name: "Floor Plan".into(),
            tool_layers: Vec::new(),
            layers: vec![
                l("Walls, Normal", [0, 0, 0], 50),
                l("Walls, Invisible", [150, 150, 150], 13),
                l("Doors", [0, 70, 200], 25),
                l(DOOR_LABEL_LAYER, [0, 70, 200], 18),
                l("Windows", [0, 130, 210], 25),
                l(WINDOW_LABEL_LAYER, [0, 130, 210], 18),
                l("Rooms", [120, 120, 120], 13),
                l("Room Labels", [60, 60, 60], 18),
                l("Dimensions, Manual", [180, 0, 0], 18),
                l("Dimensions, Automatic", [0, 128, 0], 18),
                l("Text", [0, 0, 0], 18),
                l("CAD, Default", [0, 0, 0], 25),
                l("Cabinets, Base", [139, 90, 43], 25),
                l("Cabinets, Wall", [170, 125, 75], 18),
                l("Electrical", [200, 120, 0], 18),
                l("Stairs", [90, 90, 90], 25),
                l("Roof Planes", [128, 0, 128], 25),
                l("Framing", [180, 140, 60], 18),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// The layer `tool` (a [`TOOL_LAYERS`] key) draws on: the chosen layer
    /// while it still exists, else the tool's own default. Unknown tools get
    /// an empty name.
    pub fn tool_layer(&self, tool: &str) -> String {
        let Some(info) = TOOL_LAYERS.iter().find(|t| t.key == tool) else {
            return String::new();
        };
        self.tool_layers
            .iter()
            .find(|t| t.tool == tool && self.get(&t.layer).is_some())
            .map_or_else(|| info.default_layer.to_string(), |t| t.layer.clone())
    }

    /// Chooses the layer `tool` draws on. Choosing the tool's own default
    /// clears the choice. `false` when the tool or the layer is unknown.
    pub fn set_tool_layer(&mut self, tool: &str, layer: &str) -> bool {
        let Some(info) = TOOL_LAYERS.iter().find(|t| t.key == tool) else {
            return false;
        };
        if self.get(layer).is_none() {
            return false;
        }
        self.tool_layers.retain(|t| t.tool != tool);
        if layer != info.default_layer {
            self.tool_layers.push(ToolLayer {
                tool: tool.to_string(),
                layer: layer.to_string(),
            });
        }
        true
    }

    /// Puts every tool back on its default layer.
    pub fn reset_tool_layers(&mut self) {
        self.tool_layers.clear();
    }

    /// The Current CAD Layer (CAD-1, LAY-6).
    pub fn current_cad_layer(&self) -> String {
        self.tool_layer(CAD_TOOL)
    }

    /// Adds the "Doors, Labels" and "Windows, Labels" layers beside their
    /// objects' layers when a plan from before them lacks them (DW-63).
    /// Returns whether a layer was added.
    pub fn ensure_opening_label_layers(&mut self) -> bool {
        let mut added = false;
        for (host, label, weight) in [
            ("Doors", DOOR_LABEL_LAYER, 18),
            ("Windows", WINDOW_LABEL_LAYER, 18),
        ] {
            if self.get(label).is_some() {
                continue;
            }
            let at = self.layers.iter().position(|l| l.name == host);
            let color = at.map_or([0, 0, 0], |i| self.layers[i].color);
            let layer = Layer::new(label, color, weight);
            match at {
                Some(i) => self.layers.insert(i + 1, layer),
                None => self.layers.push(layer),
            }
            added = true;
        }
        added
    }

    /// Adds the "Cabinets, Labels" layer after "Cabinets, Wall" (or the end)
    /// when the plan lacks it; the labels of cabinets are drawn on it.
    /// Returns whether a layer was added.
    pub fn ensure_cabinet_label_layer(&mut self) -> bool {
        if self.get(CABINET_LABEL_LAYER).is_some() {
            return false;
        }
        let at = self
            .layers
            .iter()
            .position(|l| l.name == "Cabinets, Wall")
            .or_else(|| self.layers.iter().position(|l| l.name == "Cabinets, Base"));
        let color = at.map_or([0, 0, 0], |i| self.layers[i].color);
        let layer = Layer::new(CABINET_LABEL_LAYER, color, 18);
        match at {
            Some(i) => self.layers.insert(i + 1, layer),
            None => self.layers.push(layer),
        }
        true
    }

    /// Adds the "Electrical Connection" layer after "Electrical" (or at the
    /// end) when the plan lacks it; the connection arcs are drawn on it.
    /// Returns whether a layer was added.
    pub fn ensure_electrical_connection_layer(&mut self) -> bool {
        if self.get(ELECTRICAL_CONNECTION_LAYER).is_some() {
            return false;
        }
        let at = self.layers.iter().position(|l| l.name == "Electrical");
        let color = at.map_or([200, 120, 0], |i| self.layers[i].color);
        let layer = Layer::new(ELECTRICAL_CONNECTION_LAYER, color, 13);
        match at {
            Some(i) => self.layers.insert(i + 1, layer),
            None => self.layers.push(layer),
        }
        true
    }

    /// Adds the wall system layers (LAY-76) after "Walls, Invisible" (or at
    /// the end) when the plan lacks them. Returns the names added.
    pub fn ensure_wall_system_layers(&mut self) -> Vec<&'static str> {
        let mut added = Vec::new();
        let mut at = self
            .layers
            .iter()
            .position(|l| l.name == "Walls, Invisible")
            .map(|i| i + 1);
        for spec in WALL_SYSTEM_LAYERS {
            if self.get(spec.name).is_some() {
                continue;
            }
            let mut layer = Layer::new(spec.name, spec.color, spec.weight);
            layer.display = spec.shown;
            match at {
                Some(i) => {
                    self.layers.insert(i, layer);
                    at = Some(i + 1);
                }
                None => self.layers.push(layer),
            }
            added.push(spec.name);
        }
        added
    }

    /// Do the walls draw the lines between their layers? A plan without the
    /// "Walls, Layers" layer always did.
    pub fn wall_layer_lines(&self) -> bool {
        self.is_visible(WALL_LAYERS_LAYER)
    }

    /// Is "Walls, Main Layer Only" on, so a wall draws just its main layer?
    /// Off while the layer does not exist.
    pub fn main_layer_only(&self) -> bool {
        self.get(WALL_MAIN_ONLY_LAYER).is_some_and(|l| l.display)
    }

    /// The layer an opening's label is drawn on.
    pub fn label_layer_of(kind: crate::model::OpeningKind) -> &'static str {
        match kind {
            crate::model::OpeningKind::Door => DOOR_LABEL_LAYER,
            crate::model::OpeningKind::Window => WINDOW_LABEL_LAYER,
        }
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Unknown layers count as visible so stray objects never vanish.
    pub fn is_visible(&self, name: &str) -> bool {
        self.get(name).is_none_or(|l| l.display)
    }

    /// Does the layer draw on a reference floor (its "Ref" box)? Unknown
    /// layers do.
    pub fn shows_in_reference(&self, name: &str) -> bool {
        self.get(name).is_none_or(|l| l.reference)
    }

    /// Unknown layers count as unlocked.
    pub fn is_locked(&self, name: &str) -> bool {
        self.get(name).is_some_and(|l| l.locked)
    }

    /// Returns `false` if the layer does not exist.
    pub fn set_display(&mut self, name: &str, display: bool) -> bool {
        match self.get_mut(name) {
            Some(l) => {
                l.display = display;
                true
            }
            None => false,
        }
    }

    /// Returns `false` if the layer does not exist.
    pub fn set_locked(&mut self, name: &str, locked: bool) -> bool {
        match self.get_mut(name) {
            Some(l) => {
                l.locked = locked;
                true
            }
            None => false,
        }
    }

    /// Add a layer if the name is new; returns whether it was added.
    pub fn add(&mut self, layer: Layer) -> bool {
        if self.get(&layer.name).is_some() {
            return false;
        }
        self.layers.push(layer);
        true
    }
}

impl crate::layer_sets::LayerSets {
    /// Gives every layer set a state for the opening label layers that
    /// follows its state for the door or window layer: a set that hides or
    /// locks the doors does the same to their labels (DW-63).
    pub fn follow_opening_label_layers(&mut self) {
        for set in &mut self.sets {
            for (host, label) in [("Doors", DOOR_LABEL_LAYER), ("Windows", WINDOW_LABEL_LAYER)] {
                if set.state(label).is_some() {
                    continue;
                }
                if let Some(h) = set.state(host).cloned() {
                    set.states.push(crate::layer_sets::LayerState::new(
                        label, h.display, h.locked,
                    ));
                }
            }
        }
    }
}

impl crate::model::Project {
    /// Adds the wall system layers (LAY-76) to the plan and to every layer
    /// set, shown or hidden as a new plan has them. Returns the names added.
    pub fn ensure_wall_system_layers(&mut self) -> Vec<&'static str> {
        let added = self.layers.ensure_wall_system_layers();
        for name in &added {
            let shown = WALL_SYSTEM_LAYERS
                .iter()
                .find(|s| s.name == *name)
                .is_some_and(|s| s.shown);
            for set in &mut self.layer_sets.sets {
                set.ensure_state(name).display = shown;
            }
        }
        added
    }
}

impl crate::model::Project {
    /// Makes sure the opening label layers exist in the plan and in each of
    /// its layer sets (a plan saved before they existed gets them when its
    /// first label is edited or an opening is placed).
    pub fn ensure_opening_label_layers(&mut self) {
        self.layers.ensure_opening_label_layers();
        self.layer_sets.follow_opening_label_layers();
    }
}

// ----- layer management (LAY-63, LAY-67) -----

/// Where a layer is in use (the Used column and its tool tip, LAY-63).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayerUse {
    /// Objects of the plan on the layer.
    pub objects: usize,
    /// Defaults that name the layer ("Current CAD Layer", "Manual
    /// Dimensions", ...), set by the user and not the tool's own default.
    pub defaults: Vec<String>,
    /// The layer is one the program ships (and tools draw on by default).
    pub system: bool,
}

/// A floor that has objects on some layers (the Select Location list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerLocation {
    pub floor: usize,
    pub objects: usize,
}

impl LayerUse {
    /// Objects or a defaults page use the layer: it cannot be deleted.
    pub fn in_use(&self) -> bool {
        self.objects > 0 || !self.defaults.is_empty()
    }

    /// The tool tip of the Used column; empty when the layer is unused and
    /// not a system layer.
    pub fn tooltip(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.objects > 0 {
            parts.push(format!(
                "{} object{} on this layer",
                self.objects,
                if self.objects == 1 { "" } else { "s" }
            ));
        }
        if !self.defaults.is_empty() {
            parts.push(format!("Set in defaults: {}", self.defaults.join(", ")));
        }
        if self.system {
            parts.push("System default layer".to_string());
        }
        parts.join("; ")
    }
}

/// A layer name Chief ships (and this program draws on by default). System
/// layers cannot be merged away or deleted (manual p. 210).
pub fn is_system_layer(name: &str) -> bool {
    use std::sync::OnceLock;
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        let mut v: Vec<String> = LayerSet::default_floor_plan()
            .layers
            .into_iter()
            .map(|l| l.name)
            .collect();
        for extra in [
            DOOR_LABEL_LAYER,
            WINDOW_LABEL_LAYER,
            CABINET_LABEL_LAYER,
            ELECTRICAL_CONNECTION_LAYER,
        ] {
            v.push(extra.to_string());
        }
        for t in TOOL_LAYERS {
            v.push(t.default_layer.to_string());
        }
        for w in WALL_SYSTEM_LAYERS {
            v.push(w.name.to_string());
        }
        v
    });
    names.iter().any(|n| n == name)
}

/// One attribute over several selected layers: the shared value, or the
/// "No Change" the Layer Display Options show when they differ.
#[derive(Debug, Clone, PartialEq)]
pub enum Mixed<T> {
    Same(T),
    NoChange,
}

impl<T: PartialEq + Clone> Mixed<T> {
    fn of(values: impl IntoIterator<Item = T>) -> Option<Self> {
        let mut it = values.into_iter();
        let first = it.next()?;
        Some(if it.all(|v| v == first) {
            Mixed::Same(first)
        } else {
            Mixed::NoChange
        })
    }
}

impl<T> Mixed<T> {
    pub fn is_mixed(&self) -> bool {
        matches!(self, Mixed::NoChange)
    }

    /// The shared value, if there is one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Mixed::Same(v) => Some(v),
            Mixed::NoChange => None,
        }
    }
}

/// The properties of a selection of layers (manual p. 210: "No Change" shows
/// for an attribute the layers do not share).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerCommon {
    pub display: Mixed<bool>,
    pub locked: Mixed<bool>,
    pub reference: Mixed<bool>,
    pub color: Mixed<[u8; 3]>,
    pub line_weight: Mixed<u32>,
    pub line_style: Mixed<LineStyle>,
    pub text_style: Mixed<String>,
}

/// The shared properties of `layers`; `None` for an empty selection.
pub fn common_props(layers: &[&Layer]) -> Option<LayerCommon> {
    Some(LayerCommon {
        display: Mixed::of(layers.iter().map(|l| l.display))?,
        locked: Mixed::of(layers.iter().map(|l| l.locked))?,
        reference: Mixed::of(layers.iter().map(|l| l.reference))?,
        color: Mixed::of(layers.iter().map(|l| l.color))?,
        line_weight: Mixed::of(layers.iter().map(|l| l.line_weight))?,
        line_style: Mixed::of(layers.iter().map(|l| l.line_style))?,
        text_style: Mixed::of(layers.iter().map(|l| l.text_style.clone()))?,
    })
}

impl crate::model::Project {
    /// How many objects of the whole plan are on each layer (every floor;
    /// a layer an object does not name falls back to its kind's own).
    pub fn layer_object_counts(&self) -> std::collections::BTreeMap<String, usize> {
        let mut used: std::collections::BTreeMap<String, usize> = Default::default();
        for f in &self.floors {
            Self::add_floor_layer_counts(f, &mut used);
        }
        used
    }

    /// How many objects of floor `floor` are on each layer (Find Objects on
    /// Layer, LAY-62).
    pub fn layer_object_counts_on_floor(
        &self,
        floor: usize,
    ) -> std::collections::BTreeMap<String, usize> {
        let mut used: std::collections::BTreeMap<String, usize> = Default::default();
        if let Some(f) = self.floors.get(floor) {
            Self::add_floor_layer_counts(f, &mut used);
        }
        used
    }

    fn add_floor_layer_counts(
        f: &crate::model::Floor,
        used: &mut std::collections::BTreeMap<String, usize>,
    ) {
        use crate::dimension::DimensionKind;
        let mut add = |name: &str| *used.entry(name.to_string()).or_insert(0) += 1;
        for w in &f.walls {
            add(&w.layer);
        }
        for o in &f.openings {
            add(o.layer_name());
        }
        for d in &f.dimensions {
            add(d.layer_or(match d.kind {
                DimensionKind::AutoExterior => "Dimensions, Automatic",
                _ => "Dimensions, Manual",
            }));
        }
        for c in &f.cad {
            add(&c.layer);
        }
        for s in &f.symbols {
            add(&s.layer);
        }
        for b in &f.blocks.blocks {
            add(&b.layer);
        }
        for r in &f.room_names {
            add(r.options.layer_name());
        }
    }

    /// Where the objects of `layers` are (Find Objects on Layer(s), LAY-62):
    /// one entry per floor that has any, with how many.
    pub fn find_objects_on_layers(&self, layers: &[String]) -> Vec<LayerLocation> {
        (0..self.floors.len())
            .filter_map(|floor| {
                let counts = self.layer_object_counts_on_floor(floor);
                let objects: usize = layers
                    .iter()
                    .map(|l| counts.get(l).copied().unwrap_or(0))
                    .sum();
                (objects > 0).then_some(LayerLocation { floor, objects })
            })
            .collect()
    }

    /// The defaults that choose `name` instead of their own layer.
    pub fn layer_defaults(&self, name: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in &self.layers.tool_layers {
            if t.layer == name {
                let label = TOOL_LAYERS
                    .iter()
                    .find(|i| i.key == t.tool)
                    .map_or(t.tool.as_str(), |i| i.label);
                out.push(label.to_string());
            }
        }
        for (i, label) in ["Manual Dimensions", "Automatic Dimensions"]
            .iter()
            .enumerate()
        {
            if self.dimension_layers[i] == name && !out.iter().any(|o| o == label) {
                out.push((*label).to_string());
            }
        }
        out
    }

    /// Where `name` is in use.
    pub fn layer_use(&self, name: &str) -> LayerUse {
        LayerUse {
            objects: self.layer_object_counts().get(name).copied().unwrap_or(0),
            defaults: self.layer_defaults(name),
            system: is_system_layer(name),
        }
    }

    fn free_layer_name(&self, base: &str) -> String {
        let taken = |n: &str| {
            self.layers
                .layers
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case(n))
        };
        if !taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| format!("{base} {i}"))
            .find(|n| !taken(n))
            .expect("unbounded range")
    }

    /// A name for a new layer that is not taken ("New Layer 19").
    pub fn free_layer_name_for_new(&self) -> String {
        let mut n = self.layers.layers.len() + 1;
        loop {
            let candidate = format!("New Layer {n}");
            if self.layers.get(&candidate).is_none() {
                return candidate;
            }
            n += 1;
        }
    }

    /// New (LAY-67): a layer with a unique name, added to every layer set;
    /// shown in the active set and hidden in the others. Returns its name.
    pub fn new_layer(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Type a name for the layer".into());
        }
        if self
            .layers
            .layers
            .iter()
            .any(|l| l.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("A layer named {name} already exists"));
        }
        self.layers.layers.push(Layer::new(name, [0, 0, 0], 18));
        let active = self.layer_sets.active.clone();
        for set in &mut self.layer_sets.sets {
            let shown = set.name == active;
            set.ensure_state(name).display = shown;
        }
        Ok(name.to_string())
    }

    /// Copy (LAY-67): a copy of `name` directly below it, with its look in
    /// every set. Returns the new name.
    pub fn copy_layer(&mut self, name: &str) -> Result<String, String> {
        let at = self
            .layers
            .layers
            .iter()
            .position(|l| l.name == name)
            .ok_or_else(|| format!("There is no layer named {name}"))?;
        let new_name = self.free_layer_name(&format!("{name} Copy"));
        let mut copy = self.layers.layers[at].clone();
        copy.name = new_name.clone();
        self.layers.layers.insert(at + 1, copy);
        for set in &mut self.layer_sets.sets {
            if let Some(mut st) = set.state(name).cloned() {
                st.layer = new_name.clone();
                set.states.push(st);
            }
            if let Some(r) = set.reference.get(name).copied() {
                set.reference.insert(new_name.clone(), r);
            }
        }
        // The layer's fill style and library line style come with the copy.
        use crate::fill_styles::FillTarget;
        use crate::line_styles::LineTarget;
        if let Some(f) = self
            .styles
            .fill_for(&FillTarget::Layer(name.to_string()))
            .cloned()
        {
            self.styles
                .apply_fill(FillTarget::Layer(new_name.clone()), Some(f));
        }
        let line = self
            .styles
            .line_assign
            .iter()
            .find(|(t, _)| *t == LineTarget::Layer(name.to_string()))
            .map(|(_, s)| s.clone());
        if let Some(l) = line {
            self.styles
                .line_assign
                .push((LineTarget::Layer(new_name.clone()), l));
        }
        Ok(new_name)
    }

    /// Rewrites every layer name the plan stores (objects, the Layer panel
    /// choices of dimensions and rooms, the Active Layer defaults) from one
    /// of `from` to `to`. Returns how many objects moved.
    fn relabel_layers(&mut self, from: &[String], to: &str) -> usize {
        let mut moved = 0;
        let mut hit = |name: &mut String| {
            if from.iter().any(|f| f == name) {
                *name = to.to_string();
                moved += 1;
            }
        };
        for f in &mut self.floors {
            for w in &mut f.walls {
                hit(&mut w.layer);
            }
            for o in &mut f.openings {
                if let Some(l) = o.extras.spec.layer.as_mut() {
                    hit(l);
                }
            }
            for d in &mut f.dimensions {
                if let Some(l) = d.look.layer.as_mut() {
                    hit(l);
                }
            }
            for c in &mut f.cad {
                hit(&mut c.layer);
            }
            for s in &mut f.symbols {
                hit(&mut s.layer);
            }
            for b in &mut f.blocks.blocks {
                hit(&mut b.layer);
            }
            for r in &mut f.room_names {
                hit(&mut r.options.layer);
            }
        }
        // Defaults follow the merge without counting as moved objects.
        for t in &mut self.layers.tool_layers {
            if from.contains(&t.layer) {
                t.layer = to.to_string();
            }
        }
        for l in &mut self.dimension_layers {
            if from.contains(l) {
                *l = to.to_string();
            }
        }
        moved
    }

    fn drop_layer(&mut self, name: &str) {
        self.layers.layers.retain(|l| l.name != name);
        self.styles
            .fill_assign
            .retain(|(t, _)| *t != crate::fill_styles::FillTarget::Layer(name.to_string()));
        self.styles
            .line_assign
            .retain(|(t, _)| *t != crate::line_styles::LineTarget::Layer(name.to_string()));
        for set in &mut self.layer_sets.sets {
            set.states.retain(|s| s.layer != name);
            set.reference.remove(name);
        }
    }

    /// Merge (LAY-67): the layers of `others` are folded into `keep`; their
    /// objects and the defaults that chose them move to it. System layers
    /// cannot be merged away. Returns how many objects moved.
    pub fn merge_layers(&mut self, keep: &str, others: &[String]) -> Result<usize, String> {
        if self.layers.get(keep).is_none() {
            return Err(format!("There is no layer named {keep}"));
        }
        let mut from: Vec<String> = Vec::new();
        for o in others {
            if o == keep || from.contains(o) {
                continue;
            }
            if self.layers.get(o).is_none() {
                return Err(format!("There is no layer named {o}"));
            }
            if is_system_layer(o) {
                return Err(format!("{o} is a system layer and cannot be merged away"));
            }
            from.push(o.clone());
        }
        if from.is_empty() {
            return Err("Select two or more layers to merge".into());
        }
        let moved = self.relabel_layers(&from, keep);
        for name in &from {
            self.drop_layer(name);
        }
        Ok(moved)
    }

    /// Delete (LAY-67): a layer that is not a system layer and is not in use.
    pub fn delete_layer(&mut self, name: &str) -> Result<(), String> {
        if self.layers.get(name).is_none() {
            return Err(format!("There is no layer named {name}"));
        }
        if is_system_layer(name) {
            return Err(format!("{name} is a system layer and cannot be deleted"));
        }
        if self.layer_use(name).in_use() {
            return Err(format!("{name} is in use and cannot be deleted"));
        }
        self.drop_layer(name);
        Ok(())
    }

    /// Delete Unused Layers (LAY-67): every non-system layer nothing uses.
    /// Returns the deleted names.
    pub fn delete_unused_layers(&mut self) -> Vec<String> {
        let counts = self.layer_object_counts();
        let doomed: Vec<String> = self
            .layers
            .layers
            .iter()
            .filter(|l| {
                !is_system_layer(&l.name)
                    && counts.get(&l.name).copied().unwrap_or(0) == 0
                    && self.layer_defaults(&l.name).is_empty()
            })
            .map(|l| l.name.clone())
            .collect();
        for name in &doomed {
            self.drop_layer(name);
        }
        doomed
    }

    /// Reset Layer Names (LAY-67): brings back any system layer the plan
    /// lacks, visible in every set. Layers cannot be renamed yet, so the
    /// names themselves never drift. Returns how many were restored.
    pub fn reset_layer_names(&mut self) -> usize {
        let mut missing: Vec<Layer> = LayerSet::default_floor_plan()
            .layers
            .into_iter()
            .filter(|l| self.layers.get(&l.name).is_none())
            .collect();
        let before = self.layers.layers.len();
        self.layers.ensure_opening_label_layers();
        self.layers.ensure_cabinet_label_layer();
        self.layers.ensure_electrical_connection_layer();
        let mut restored = self.layers.layers.len() - before;
        restored += self.ensure_wall_system_layers().len();
        for l in missing.drain(..) {
            if self.layers.add(l) {
                restored += 1;
            }
        }
        if restored > 0 {
            let names: Vec<String> = self.layers.layers.iter().map(|l| l.name.clone()).collect();
            for set in &mut self.layer_sets.sets {
                for n in &names {
                    if set.state(n).is_none() {
                        set.ensure_state(n);
                    }
                }
            }
        }
        restored
    }
}

impl Default for LayerSet {
    fn default() -> Self {
        Self::default_floor_plan()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_layers_are_added_to_old_plans_and_follow_their_door_layer() {
        let mut old = LayerSet::default_floor_plan();
        old.layers.retain(|l| !l.name.ends_with(", Labels"));
        assert!(old.get(DOOR_LABEL_LAYER).is_none());
        assert!(old.ensure_opening_label_layers());
        assert!(!old.ensure_opening_label_layers());
        let i = old.layers.iter().position(|l| l.name == "Doors").unwrap();
        assert_eq!(old.layers[i + 1].name, DOOR_LABEL_LAYER);
        assert_eq!(
            old.get(WINDOW_LABEL_LAYER).unwrap().color,
            old.get("Windows").unwrap().color
        );

        let mut p = crate::model::Project::new("x");
        p.layers.layers.retain(|l| !l.name.ends_with(", Labels"));
        p.layer_sets
            .sets
            .iter_mut()
            .for_each(|s| s.states.retain(|st| !st.layer.ends_with(", Labels")));
        p.layer_sets.set_display("Default Set", "Doors", false);
        p.ensure_opening_label_layers();
        assert!(p.layers.get(DOOR_LABEL_LAYER).is_some());
        let eff = p.layer_sets.effective(&p.layers);
        assert!(
            !eff.is_visible(DOOR_LABEL_LAYER),
            "hidden doors hide their labels"
        );
        assert!(eff.is_visible(WINDOW_LABEL_LAYER));
    }

    #[test]
    fn default_set_lookups() {
        let mut set = LayerSet::default_floor_plan();
        assert!(set.layers.len() >= 18);
        for n in [
            "Doors, Labels",
            "Windows, Labels",
            "Walls, Normal",
            "Walls, Invisible",
            "Doors",
            "Windows",
            "Rooms",
            "Room Labels",
            "Dimensions, Manual",
            "Dimensions, Automatic",
            "Text",
            "CAD, Default",
            "Cabinets, Base",
            "Cabinets, Wall",
            "Electrical",
            "Stairs",
            "Roof Planes",
            "Framing",
        ] {
            assert!(set.get(n).is_some(), "missing layer {n}");
        }
        assert_eq!(set.get("Walls, Normal").unwrap().color, [0, 0, 0]);
        assert!(set.is_visible("Doors"));
        assert!(set.set_display("Doors", false));
        assert!(!set.is_visible("Doors"));
        assert!(!set.set_display("Nope", false));
        assert!(set.is_visible("Nope"));
        assert!(set.set_locked("Text", true) && set.is_locked("Text"));
        assert!(!set.add(Layer::new("Text", [0, 0, 0], 1)));
        assert!(set.add(Layer::new("Extra", [1, 2, 3], 1)));
        // Old layer JSON without the newer fields still loads.
        let old: Layer = serde_json::from_str(
            r#"{"name":"A","display":true,"locked":false,"color":[1,2,3],"line_weight":18}"#,
        )
        .unwrap();
        assert_eq!(old.line_style, LineStyle::Solid);
        assert!(old.text_style.is_empty());
    }

    #[test]
    fn tools_draw_on_their_default_layer_until_one_is_chosen() {
        let mut set = LayerSet::default_floor_plan();
        assert_eq!(set.current_cad_layer(), "CAD, Default");
        assert_eq!(set.tool_layer("text"), "Text");
        assert_eq!(set.tool_layer("nope"), "");
        set.add(Layer::new("Notes", [1, 2, 3], 18));
        assert!(set.set_tool_layer(CAD_TOOL, "Notes"));
        assert_eq!(set.current_cad_layer(), "Notes");
        assert!(!set.set_tool_layer(CAD_TOOL, "No Such Layer"));
        assert!(!set.set_tool_layer("nope", "Notes"));
        assert_eq!(set.tool_layers.len(), 1);
        // Choosing the default again clears the choice.
        assert!(set.set_tool_layer(CAD_TOOL, "CAD, Default"));
        assert!(set.tool_layers.is_empty());
        // A chosen layer that is gone falls back to the default.
        set.set_tool_layer(CAD_TOOL, "Notes");
        set.layers.retain(|l| l.name != "Notes");
        assert_eq!(set.current_cad_layer(), "CAD, Default");
        // Old JSON without the field loads.
        let old: LayerSet = serde_json::from_str(r#"{"name":"x","layers":[]}"#).unwrap();
        assert!(old.tool_layers.is_empty());
        set.set_tool_layer(CAD_TOOL, "Text");
        set.reset_tool_layers();
        assert_eq!(set.current_cad_layer(), "CAD, Default");
    }
}

#[cfg(test)]
mod management_tests {
    use super::*;
    use crate::cad::CadItem;
    use crate::geometry::Point;
    use crate::model::{Project, WallKind};

    fn circle() -> CadItem {
        CadItem::Circle {
            center: Point::ZERO,
            radius: 5.0,
        }
    }

    #[test]
    fn a_new_layer_is_unique_and_hidden_in_every_set_but_the_active_one() {
        let mut p = Project::new("l");
        let mut other = crate::layer_sets::LayerSetDef::new("Other");
        other.ensure_state("Text");
        p.layer_sets.add_set(other);
        assert_eq!(p.new_layer("  Notes ").unwrap(), "Notes");
        assert!(p.new_layer("notes").is_err(), "names are unique");
        assert!(p.new_layer("  ").is_err());
        let active = p.layer_sets.active.clone();
        assert!(
            p.layer_sets
                .get(&active)
                .unwrap()
                .state("Notes")
                .unwrap()
                .display
        );
        assert!(
            !p.layer_sets
                .get("Other")
                .unwrap()
                .state("Notes")
                .unwrap()
                .display
        );
        // Through the sets' own view of the layers it is hidden in Other.
        assert!(!p
            .layer_sets
            .effective_for("Other", &p.layers)
            .is_visible("Notes"));
        assert!(p
            .layer_sets
            .effective_for(&active, &p.layers)
            .is_visible("Notes"));
    }

    #[test]
    fn copy_sits_below_the_original_and_keeps_its_look() {
        let mut p = Project::new("l");
        p.new_layer("Notes").unwrap();
        p.layers.get_mut("Notes").unwrap().color = [9, 8, 7];
        p.new_layer("Zed").unwrap();
        let copy = p.copy_layer("Notes").unwrap();
        assert_eq!(copy, "Notes Copy");
        let names: Vec<&str> = p.layers.layers.iter().map(|l| l.name.as_str()).collect();
        let at = names.iter().position(|n| *n == "Notes").unwrap();
        assert_eq!(names[at + 1], "Notes Copy");
        assert_eq!(p.layers.get("Notes Copy").unwrap().color, [9, 8, 7]);
        assert_eq!(p.copy_layer("Notes").unwrap(), "Notes Copy 2");
        assert!(p.copy_layer("Missing").is_err());
    }

    #[test]
    fn merge_moves_objects_and_defaults_into_the_first_layer() {
        let mut p = Project::new("l");
        p.new_layer("Keep").unwrap();
        p.new_layer("Gone").unwrap();
        p.new_layer("Gone Too").unwrap();
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p.floors[0].wall_mut(w).unwrap().layer = "Gone".into();
        let c = p.add_cad(0, "Gone Too", circle());
        p.layers.set_tool_layer(CAD_TOOL, "Gone Too");
        p.dimension_layers[0] = "Gone".into();
        assert!(p.layer_use("Gone").in_use());
        let moved = p
            .merge_layers("Keep", &["Gone".into(), "Gone Too".into(), "Keep".into()])
            .unwrap();
        assert_eq!(moved, 2, "the wall and the CAD item; defaults do not count");
        assert_eq!(p.floors[0].wall(w).unwrap().layer, "Keep");
        assert_eq!(
            p.floors[0].cad.iter().find(|x| x.id == c).unwrap().layer,
            "Keep"
        );
        assert_eq!(p.layers.current_cad_layer(), "Keep");
        assert_eq!(p.dimension_layers[0], "Keep");
        assert!(p.layers.get("Gone").is_none() && p.layers.get("Gone Too").is_none());
        for set in &p.layer_sets.sets {
            assert!(set.state("Gone").is_none());
        }
        assert_eq!(p.layer_use("Keep").objects, 2);
    }

    #[test]
    fn merge_refuses_system_layers_and_a_lone_layer() {
        let mut p = Project::new("l");
        p.new_layer("Keep").unwrap();
        assert!(p.merge_layers("Keep", &["Doors".into()]).is_err());
        assert!(p.merge_layers("Keep", &["Keep".into()]).is_err());
        assert!(p.merge_layers("Keep", &["Nope".into()]).is_err());
        assert!(p.layers.get("Doors").is_some());
        // A custom layer may merge into a system layer.
        p.new_layer("Extra").unwrap();
        p.add_cad(0, "Extra", circle());
        assert_eq!(
            p.merge_layers("Walls, Normal", &["Extra".into()]).unwrap(),
            1
        );
    }

    #[test]
    fn delete_refuses_system_or_used_layers() {
        let mut p = Project::new("l");
        p.new_layer("Used").unwrap();
        p.new_layer("Free").unwrap();
        p.new_layer("Chosen").unwrap();
        p.add_cad(0, "Used", circle());
        p.layers.set_tool_layer("text", "Chosen");
        assert!(p
            .delete_layer("Walls, Normal")
            .unwrap_err()
            .contains("system"));
        assert!(p.delete_layer("Used").unwrap_err().contains("in use"));
        assert!(p.delete_layer("Chosen").unwrap_err().contains("in use"));
        assert!(p.delete_layer("Missing").is_err());
        assert!(p.delete_layer("Free").is_ok());
        assert!(p.layers.get("Free").is_none());
        let gone = p.delete_unused_layers();
        assert!(gone.is_empty(), "Used and Chosen stay: {gone:?}");
        p.new_layer("Idle").unwrap();
        assert_eq!(p.delete_unused_layers(), vec!["Idle".to_string()]);
        assert!(p.layers.get("Used").is_some());
    }

    #[test]
    fn the_used_column_says_where_a_layer_is_used() {
        let mut p = Project::new("l");
        p.new_layer("Used").unwrap();
        p.add_cad(0, "Used", circle());
        p.layers.set_tool_layer(CAD_TOOL, "Used");
        let u = p.layer_use("Used");
        assert_eq!(u.objects, 1);
        assert_eq!(u.defaults, vec!["CAD (Current CAD Layer)".to_string()]);
        assert!(u.tooltip().contains("1 object on this layer"));
        let sys = p.layer_use("Doors");
        assert!(sys.system && !sys.in_use());
        assert_eq!(sys.tooltip(), "System default layer");
        assert_eq!(p.layer_use("Nope").tooltip(), "");
    }

    #[test]
    fn mixed_selections_show_no_change() {
        let mut a = Layer::new("A", [1, 2, 3], 18);
        let mut b = Layer::new("B", [1, 2, 3], 25);
        a.locked = true;
        b.locked = true;
        let c = common_props(&[&a, &b]).unwrap();
        assert_eq!(c.color, Mixed::Same([1, 2, 3]));
        assert_eq!(c.locked, Mixed::Same(true));
        assert_eq!(c.line_weight, Mixed::NoChange);
        assert!(c.line_weight.is_mixed() && c.line_weight.value().is_none());
        assert_eq!(c.color.value(), Some(&[1, 2, 3]));
        assert!(common_props(&[]).is_none());
        b.text_style = "Fancy".into();
        assert!(common_props(&[&a, &b]).unwrap().text_style.is_mixed());
        assert!(!common_props(&[&a]).unwrap().text_style.is_mixed());
    }

    #[test]
    fn reset_layer_names_restores_missing_system_layers_only() {
        let mut p = Project::new("l");
        p.new_layer("Mine").unwrap();
        // A fresh plan may still lack the label layers that are made on use.
        p.reset_layer_names();
        assert_eq!(p.reset_layer_names(), 0);
        p.layers
            .layers
            .retain(|l| l.name != "Rooms" && l.name != "Doors, Labels");
        assert_eq!(p.reset_layer_names(), 2);
        assert!(p.layers.get("Rooms").is_some() && p.layers.get("Mine").is_some());
        assert!(is_system_layer("Doors, Labels") && !is_system_layer("Mine"));
    }

    #[test]
    fn wall_system_layers_are_added_once_to_the_plan_and_every_set() {
        let mut p = Project::new("l");
        let mut other = crate::layer_sets::LayerSetDef::new("Other");
        other.ensure_state("Text");
        p.layer_sets.add_set(other);
        let before = p.layers.layers.len();
        let added = p.ensure_wall_system_layers();
        assert_eq!(added.len(), WALL_SYSTEM_LAYERS.len());
        assert_eq!(p.layers.layers.len(), before + added.len());
        assert!(p.ensure_wall_system_layers().is_empty(), "only once");
        // They sit beside the wall layers and are system layers.
        let at = |n: &str| p.layers.layers.iter().position(|l| l.name == n).unwrap();
        assert_eq!(at(WALL_LAYERS_LAYER), at("Walls, Invisible") + 1);
        assert!(is_system_layer(BRICK_LEDGE_LAYER) && is_system_layer(FOOTINGS_LAYER));
        assert!(p.delete_layer(WALL_ATTIC_LAYER).is_err());
        // Layer lines are on and main-layer-only is off, in every set.
        for set in ["Other", p.layer_sets.active.clone().as_str()] {
            let eff = p.layer_sets.effective_for(set, &p.layers);
            assert!(eff.wall_layer_lines(), "{set}");
            assert!(!eff.main_layer_only(), "{set}");
            assert!(!eff.is_visible(WALL_THROUGH_LINES_LAYER), "{set}");
        }
        // A plan without the layers draws walls as it always did.
        let old = LayerSet::default_floor_plan();
        assert!(old.wall_layer_lines() && !old.main_layer_only());
    }

    #[test]
    fn find_objects_on_layers_lists_the_floors_that_hold_them() {
        let mut p = Project::new("l");
        p.floors.push(crate::model::Floor::new("Upper", 108.0));
        let f1 = p.floors.len() - 1;
        p.new_layer("Notes").unwrap();
        p.add_cad(0, "Notes", circle());
        p.add_cad(0, "Notes", circle());
        p.add_cad(f1, "Notes", circle());
        p.add_cad(f1, "Text", circle());
        let found = p.find_objects_on_layers(&["Notes".to_string()]);
        assert_eq!(
            found,
            vec![
                LayerLocation {
                    floor: 0,
                    objects: 2
                },
                LayerLocation {
                    floor: f1,
                    objects: 1
                }
            ]
        );
        assert!(p.find_objects_on_layers(&["Rooms".to_string()]).is_empty());
        let both = p.find_objects_on_layers(&["Notes".to_string(), "Text".to_string()]);
        assert_eq!(both[1].objects, 2);
    }

    #[test]
    fn a_layer_fill_style_follows_copy_and_goes_with_delete() {
        use crate::fill_styles::{FillStyle, FillTarget};
        let mut p = Project::new("l");
        p.new_layer("Hatched").unwrap();
        p.styles.apply_fill(
            FillTarget::Layer("Hatched".into()),
            Some(FillStyle::hatch(45.0, 6.0, [0, 0, 0])),
        );
        let copy = p.copy_layer("Hatched").unwrap();
        assert!(p
            .styles
            .fill_for(&FillTarget::Layer(copy.clone()))
            .is_some());
        p.delete_layer(&copy).unwrap();
        assert!(p.styles.fill_for(&FillTarget::Layer(copy)).is_none());
        assert!(p
            .styles
            .fill_for(&FillTarget::Layer("Hatched".into()))
            .is_some());
        p.delete_layer("Hatched").unwrap();
        assert!(p.styles.fill_assign.is_empty());
    }
}
