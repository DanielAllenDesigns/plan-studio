//! Pen weights per source object: Chief's Vector View draws each object's
//! lines at the pen weight of its layer (walls heavy, trim and doors lighter).
//!
//! The caller maps wall and opening ids to the plotted weight (hundredths of
//! a millimetre) of their layers; the drawing pipeline then restyles the lines
//! of those objects before merging.

use crate::drawing::{EdgeKind, LineWeight};
use plan_core::Id;
use std::collections::HashMap;

/// Pens at or above this many hundredths of a millimetre draw Heavy.
pub const HEAVY_PEN: u32 = 35;
/// Pens at or above this draw Medium; lighter ones draw Light.
pub const MEDIUM_PEN: u32 = 20;

/// The line weight class of a layer pen (hundredths of a millimetre).
pub fn pen_class(pen: u32) -> LineWeight {
    if pen >= HEAVY_PEN {
        LineWeight::Heavy
    } else if pen >= MEDIUM_PEN {
        LineWeight::Medium
    } else {
        LineWeight::Light
    }
}

/// Line weight class per source object (wall or opening id).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObjectWeights {
    classes: HashMap<Id, LineWeight>,
}

impl ObjectWeights {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the class of one object.
    pub fn set(&mut self, id: Id, class: LineWeight) {
        self.classes.insert(id, class);
    }

    /// Sets the class of one object from its layer's pen weight.
    pub fn set_pen(&mut self, id: Id, pen: u32) {
        self.set(id, pen_class(pen));
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }

    /// The class of object `id`, if it has one.
    pub fn class_of(&self, id: Id) -> Option<LineWeight> {
        self.classes.get(&id).copied()
    }

    /// The weight of a line of `kind` that the geometry drew at `weight` for
    /// an object of class `class`: outlines and creases take the object's
    /// class; seams between materials never go past Medium; cut lines, hatches
    /// and notes keep theirs.
    pub fn restyle(&self, kind: EdgeKind, weight: LineWeight, class: LineWeight) -> LineWeight {
        match kind {
            EdgeKind::Silhouette | EdgeKind::Crease => class,
            EdgeKind::Material => class.min(LineWeight::Medium),
            _ => weight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pens_map_to_classes() {
        assert_eq!(pen_class(50), LineWeight::Heavy);
        assert_eq!(pen_class(25), LineWeight::Medium);
        assert_eq!(pen_class(18), LineWeight::Light);
    }

    #[test]
    fn restyle_leaves_cuts_and_caps_seams() {
        let w = ObjectWeights::new();
        assert_eq!(
            w.restyle(EdgeKind::Cut, LineWeight::Heavy, LineWeight::Light),
            LineWeight::Heavy
        );
        assert_eq!(
            w.restyle(EdgeKind::Silhouette, LineWeight::Heavy, LineWeight::Light),
            LineWeight::Light
        );
        assert_eq!(
            w.restyle(EdgeKind::Material, LineWeight::Light, LineWeight::Heavy),
            LineWeight::Medium
        );
    }
}
