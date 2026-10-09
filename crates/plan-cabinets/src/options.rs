//! Cabinet Specification options that sit beside the main model: Box
//! Construction, the manufacturer, which front items show open, the ends of
//! a cabinet that are exposed or mated, and the settings of a custom
//! countertop (reference manual pp. 665, 669 to 673, 688 to 690).

use serde::{Deserialize, Serialize};

use crate::dress::PilasterStyle;
use crate::top::CornerTreatment;

/// A part that is on, off or decided by the program (Top, Bottom of the box;
/// Left and Right front pilasters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutoOnOff {
    #[default]
    Auto,
    On,
    Off,
}

impl AutoOnOff {
    pub const ALL: [AutoOnOff; 3] = [AutoOnOff::Auto, AutoOnOff::On, AutoOnOff::Off];

    pub fn name(self) -> &'static str {
        match self {
            AutoOnOff::Auto => "Auto",
            AutoOnOff::On => "On",
            AutoOnOff::Off => "Off",
        }
    }

    /// Resolves to a yes or no given what Auto means here.
    pub fn resolve(self, auto: bool) -> bool {
        match self {
            AutoOnOff::Auto => auto,
            AutoOnOff::On => true,
            AutoOnOff::Off => false,
        }
    }
}

/// The Box Construction panel (p. 672): top, bottom, side and back thickness
/// and the corner treatment of the box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxConstruction {
    /// Does the box have a top? Auto: wall and full height cabinets do,
    /// base cabinets and fillers do not.
    pub top: AutoOnOff,
    /// Does it have a bottom? Auto: a bottom unless the lowest face item is
    /// something other than a separation or a blank area (an appliance
    /// garage has none).
    pub bottom: AutoOnOff,
    pub side_thickness: f64,
    pub back_thickness: f64,
    /// Clipped or rounded corners of the box (Standard and Bow Front).
    pub corner: CornerTreatment,
    /// Corner Clip distance or Radius, inches.
    pub corner_size: f64,
    /// Automatic Placement: the treatment goes on every corner that is not
    /// against a wall or a cabinet of the same type.
    pub auto_corners: bool,
    /// Back Left, Back Right, Front Left, Front Right when placement is not
    /// automatic.
    pub corners: [bool; 4],
    /// Corner pilaster (Clipped corners only).
    pub corner_pilaster: PilasterStyle,
    pub corner_pilaster_width: f64,
    pub corner_pilaster_to_bottom: bool,
}

impl Default for BoxConstruction {
    fn default() -> Self {
        Self {
            top: AutoOnOff::Auto,
            bottom: AutoOnOff::Auto,
            side_thickness: 0.75,
            back_thickness: 0.75,
            corner: CornerTreatment::None,
            corner_size: 2.0,
            auto_corners: true,
            corners: [false; 4],
            corner_pilaster: PilasterStyle::None,
            corner_pilaster_width: 2.5,
            corner_pilaster_to_bottom: false,
        }
    }
}

impl BoxConstruction {
    pub fn is_default(&self) -> bool {
        *self == BoxConstruction::default()
    }
}

/// Which front items are shown open (Front/Sides/Back, Options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ShowOpen {
    pub doors: bool,
    pub drawers: bool,
    pub rollouts: bool,
}

impl ShowOpen {
    pub fn any(self) -> bool {
        self.doors || self.drawers || self.rollouts
    }

    pub fn all() -> Self {
        Self {
            doors: true,
            drawers: true,
            rollouts: true,
        }
    }
}

/// The Manufacturer panel: contact information of a catalog cabinet.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Manufacturer {
    pub name: String,
    pub contact: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub catalog: String,
}

/// The ends of a cabinet that touch a wall, an appliance or another cabinet
/// (mated); the others are exposed. Derived by `exposures` and stored on the
/// cabinet by the editor's sync pass; a cabinet that has none (`None`)
/// stands free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Ends {
    pub left: bool,
    pub right: bool,
    pub back: bool,
    /// The back is against a wall (not just another cabinet).
    pub back_wall: bool,
}

/// How a countertop edge gets its molding (Molding on Selected Edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeMolding {
    /// None against a wall, the profile otherwise.
    #[default]
    Automatic,
    NoMolding,
    HasMolding,
}

impl EdgeMolding {
    pub const ALL: [EdgeMolding; 3] = [
        EdgeMolding::Automatic,
        EdgeMolding::NoMolding,
        EdgeMolding::HasMolding,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EdgeMolding::Automatic => "Automatic",
            EdgeMolding::NoMolding => "No Molding",
            EdgeMolding::HasMolding => "Has Molding",
        }
    }
}

/// One edge of a custom countertop.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TopEdge {
    pub molding: EdgeMolding,
    /// A vertical slab down from this edge.
    pub waterfall: bool,
}

/// The Custom Countertop Specification beyond the outline (pp. 688 to 690).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TopSpec {
    /// One entry per edge of the outline (edge `i` runs from point `i` to
    /// point `i + 1`); missing entries are default.
    pub edges: Vec<TopEdge>,
    /// The index of the selected edge.
    pub selected: usize,
    /// Hole in Countertop: this top is a hole in the one that contains it
    /// (set by the conversion, which removes the top itself).
    pub hole: bool,
    pub thickness_from_cabinet: bool,
    pub height_from_cabinet: bool,
    /// Mitre All Waterfall Edges.
    pub mitre_waterfall: bool,
    /// The waterfall builds down to the floor.
    pub waterfall_auto_height: bool,
    /// Height of the waterfall below the bottom of the horizontal top.
    pub waterfall_height: f64,
    /// Draw two edge lines (the width of the molding) in plan. On by
    /// default: custom tops with an edge profile have always drawn it.
    pub display_molding_edges: bool,
}

impl Default for TopSpec {
    fn default() -> Self {
        Self {
            edges: Vec::new(),
            selected: 0,
            hole: false,
            thickness_from_cabinet: false,
            height_from_cabinet: false,
            mitre_waterfall: true,
            waterfall_auto_height: true,
            waterfall_height: 34.5,
            display_molding_edges: true,
        }
    }
}

impl TopSpec {
    pub fn is_default(&self) -> bool {
        *self == TopSpec::default()
    }

    /// The edge `i`, default when none is stored.
    pub fn edge(&self, i: usize) -> TopEdge {
        self.edges.get(i).copied().unwrap_or_default()
    }

    /// Mutable edge `i`, stored first when needed.
    pub fn edge_mut(&mut self, i: usize) -> &mut TopEdge {
        if self.edges.len() <= i {
            self.edges.resize(i + 1, TopEdge::default());
        }
        &mut self.edges[i]
    }

    /// Does any edge have a waterfall?
    pub fn has_waterfall(&self) -> bool {
        self.edges.iter().any(|e| e.waterfall)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_on_off_resolves() {
        assert!(AutoOnOff::Auto.resolve(true));
        assert!(!AutoOnOff::Auto.resolve(false));
        assert!(AutoOnOff::On.resolve(false));
        assert!(!AutoOnOff::Off.resolve(true));
    }

    #[test]
    fn top_spec_stores_edges_on_demand() {
        let mut t = TopSpec::default();
        assert!(t.is_default());
        assert_eq!(t.edge(5), TopEdge::default());
        t.edge_mut(2).waterfall = true;
        assert_eq!(t.edges.len(), 3);
        assert!(t.has_waterfall());
        assert!(!t.is_default());
    }

    #[test]
    fn old_files_load_with_defaults() {
        let b: BoxConstruction = serde_json::from_str("{}").unwrap();
        assert!(b.is_default());
        let t: TopSpec = serde_json::from_str("{}").unwrap();
        assert!(t.is_default());
    }
}
