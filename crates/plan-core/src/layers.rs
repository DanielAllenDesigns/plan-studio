//! Chief-style layers: named groups of objects with display/lock state, a
//! colour and a plotted line weight.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub display: bool,
    pub locked: bool,
    /// RGB display colour.
    pub color: [u8; 3],
    /// Plotted line weight in hundredths of a millimetre (DXF convention).
    pub line_weight: u32,
}

impl Layer {
    pub fn new(name: impl Into<String>, color: [u8; 3], line_weight: u32) -> Self {
        Self {
            name: name.into(),
            display: true,
            locked: false,
            color,
            line_weight,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSet {
    pub name: String,
    pub layers: Vec<Layer>,
}

impl LayerSet {
    /// The default floor-plan layer set.
    pub fn default_floor_plan() -> LayerSet {
        let l = Layer::new;
        LayerSet {
            name: "Floor Plan".into(),
            layers: vec![
                l("Walls, Normal", [0, 0, 0], 50),
                l("Walls, Invisible", [150, 150, 150], 13),
                l("Doors", [0, 70, 200], 25),
                l("Windows", [0, 130, 210], 25),
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

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Unknown layers count as visible so stray objects never vanish.
    pub fn is_visible(&self, name: &str) -> bool {
        self.get(name).is_none_or(|l| l.display)
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

impl Default for LayerSet {
    fn default() -> Self {
        Self::default_floor_plan()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_set_lookups() {
        let mut set = LayerSet::default_floor_plan();
        assert!(set.layers.len() >= 16);
        for n in [
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
    }
}
