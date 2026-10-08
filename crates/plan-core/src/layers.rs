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
/// System layer of window labels (DW-63).
pub const WINDOW_LABEL_LAYER: &str = "Windows, Labels";

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
    /// Makes sure the opening label layers exist in the plan and in each of
    /// its layer sets (a plan saved before they existed gets them when its
    /// first label is edited or an opening is placed).
    pub fn ensure_opening_label_layers(&mut self) {
        self.layers.ensure_opening_label_layers();
        self.layer_sets.follow_opening_label_layers();
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
