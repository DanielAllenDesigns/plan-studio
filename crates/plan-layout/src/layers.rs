//! The layout's own layer set (Layer Display Options in the layout view).
//!
//! Chief keeps one small layer set for the paper: box borders, layout CAD,
//! text, the title block and revision clouds. Each layer can be shown or
//! hidden and has a line weight (and a colour), which the printed page and
//! the layout window both follow.

use plan_core::cad::DEFAULT_CAD_LAYER;
use plan_core::{CadItem, CadObject};
use serde::{Deserialize, Serialize};

/// The frames drawn around layout boxes.
pub const LAYER_BOX_BORDERS: &str = "Layout Box Borders";
/// Lines, arcs, circles and polylines drawn on the page.
pub const LAYER_CAD: &str = "Layout CAD";
/// Page text, text boxes and the text of leaders.
pub const LAYER_TEXT: &str = "Text";
/// The border, strips, fields and revision table of the title block.
pub const LAYER_TITLE_BLOCK: &str = "Title Block";
/// Revision clouds and their tags.
pub const LAYER_REVISION_CLOUDS: &str = "Revision Clouds";

/// Pen weight of the title block's own lines at weight 1.0 (points); the
/// Title Block layer's weight scales the drawn pens by `weight / this`.
pub(crate) const TITLE_BLOCK_BASE_PT: f64 = 0.75;

/// One layer of the layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutLayer {
    pub name: String,
    /// Shown on the page and in print.
    pub display: bool,
    /// Pen width in points.
    pub line_weight_pt: f64,
    /// Pen colour (RGB).
    pub color: [u8; 3],
}

impl LayoutLayer {
    fn new(name: &str, line_weight_pt: f64) -> Self {
        Self {
            name: name.to_string(),
            display: true,
            line_weight_pt,
            color: [0, 0, 0],
        }
    }
}

/// Smallest and largest pen width a layer may be given, points.
pub const MIN_WEIGHT_PT: f64 = 0.05;
pub const MAX_WEIGHT_PT: f64 = 6.0;

/// The five layout layers, in the order the Layer Display Options lists them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutLayers {
    pub layers: Vec<LayoutLayer>,
}

impl Default for LayoutLayers {
    fn default() -> Self {
        Self {
            layers: vec![
                LayoutLayer::new(LAYER_BOX_BORDERS, 0.75),
                LayoutLayer::new(LAYER_CAD, 0.5),
                LayoutLayer::new(LAYER_TEXT, 0.5),
                LayoutLayer::new(LAYER_TITLE_BLOCK, TITLE_BLOCK_BASE_PT),
                LayoutLayer::new(LAYER_REVISION_CLOUDS, 1.0),
            ],
        }
    }
}

impl LayoutLayers {
    pub fn get(&self, name: &str) -> Option<&LayoutLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut LayoutLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Is layer `name` shown? Unknown layers are.
    pub fn is_visible(&self, name: &str) -> bool {
        self.get(name).is_none_or(|l| l.display)
    }

    /// Pen width of layer `name`, points (0.5 for an unknown layer).
    pub fn weight_pt(&self, name: &str) -> f64 {
        self.get(name).map_or(0.5, |l| l.line_weight_pt)
    }

    /// Pen colour of layer `name` (black for an unknown layer).
    pub fn color(&self, name: &str) -> [u8; 3] {
        self.get(name).map_or([0, 0, 0], |l| l.color)
    }

    /// Shows or hides a layer; false when there is no such layer.
    pub fn set_visible(&mut self, name: &str, on: bool) -> bool {
        self.get_mut(name).map(|l| l.display = on).is_some()
    }

    /// Sets a layer's pen width (clamped to the supported range).
    pub fn set_weight(&mut self, name: &str, pt: f64) -> bool {
        self.get_mut(name)
            .map(|l| l.line_weight_pt = pt.clamp(MIN_WEIGHT_PT, MAX_WEIGHT_PT))
            .is_some()
    }

    /// The layout layer a page CAD object is on: its own when it names one of
    /// ours, else text on `Text` and everything else on `Layout CAD` (objects
    /// drawn before the layer set existed sit on `CAD, Default`).
    pub fn layer_of(o: &CadObject) -> &'static str {
        match o.layer.as_str() {
            LAYER_BOX_BORDERS => LAYER_BOX_BORDERS,
            LAYER_CAD => LAYER_CAD,
            LAYER_TEXT => LAYER_TEXT,
            LAYER_TITLE_BLOCK => LAYER_TITLE_BLOCK,
            LAYER_REVISION_CLOUDS => LAYER_REVISION_CLOUDS,
            _ => {
                if matches!(o.item, CadItem::Text { .. }) {
                    LAYER_TEXT
                } else {
                    LAYER_CAD
                }
            }
        }
    }

    /// The layer new page CAD of this kind goes on.
    pub fn new_layer_for(item: &CadItem) -> &'static str {
        if matches!(item, CadItem::Text { .. }) {
            LAYER_TEXT
        } else {
            LAYER_CAD
        }
    }

    /// Does `name` belong to the old default CAD layer? (kept for callers
    /// that still file things on `CAD, Default`)
    pub fn is_legacy(name: &str) -> bool {
        name == DEFAULT_CAD_LAYER
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;

    fn obj(layer: &str, item: CadItem) -> CadObject {
        CadObject {
            id: 1,
            layer: layer.into(),
            item,
        }
    }

    #[test]
    fn the_default_set_has_the_five_layers_all_shown() {
        let l = LayoutLayers::default();
        let names: Vec<&str> = l.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Layout Box Borders",
                "Layout CAD",
                "Text",
                "Title Block",
                "Revision Clouds"
            ]
        );
        assert!(l.layers.iter().all(|l| l.display));
        assert_eq!(l.weight_pt(LAYER_BOX_BORDERS), 0.75);
        assert_eq!(l.weight_pt(LAYER_CAD), 0.5);
    }

    #[test]
    fn show_hide_and_weights_apply_and_clamp() {
        let mut l = LayoutLayers::default();
        assert!(l.set_visible(LAYER_TEXT, false));
        assert!(!l.is_visible(LAYER_TEXT));
        assert!(l.is_visible("something else"));
        assert!(l.set_weight(LAYER_CAD, 99.0));
        assert_eq!(l.weight_pt(LAYER_CAD), MAX_WEIGHT_PT);
        assert!(l.set_weight(LAYER_CAD, 0.0));
        assert_eq!(l.weight_pt(LAYER_CAD), MIN_WEIGHT_PT);
        assert!(!l.set_visible("nope", true));
    }

    #[test]
    fn legacy_cad_sorts_text_and_drawings_onto_the_layout_layers() {
        let line = CadItem::Line {
            a: Point::ZERO,
            b: Point::new(1.0, 0.0),
        };
        let text = CadItem::Text {
            pos: Point::ZERO,
            text: "x".into(),
            height: 0.1,
            angle: 0.0,
        };
        assert_eq!(
            LayoutLayers::layer_of(&obj(DEFAULT_CAD_LAYER, line.clone())),
            LAYER_CAD
        );
        assert_eq!(
            LayoutLayers::layer_of(&obj(DEFAULT_CAD_LAYER, text)),
            LAYER_TEXT
        );
        assert_eq!(
            LayoutLayers::layer_of(&obj(LAYER_REVISION_CLOUDS, line)),
            LAYER_REVISION_CLOUDS
        );
    }
}
