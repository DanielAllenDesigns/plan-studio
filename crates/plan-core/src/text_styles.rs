//! Chief-style text styles: a named font, size and colour that layers and
//! annotations refer to by name (`Layer::text_style`).

use crate::layers::LayerSet;
use serde::{Deserialize, Serialize};

/// The style used when a name is empty or unknown.
pub const DEFAULT_TEXT_STYLE_NAME: &str = "Default Text Style";

/// Plan inches of text height that prints `printed_in` paper inches tall at
/// `inches_per_foot` paper scale (1/8" at 1/4" scale: 6").
pub fn plan_height_for_printed(printed_in: f64, inches_per_foot: f64) -> f64 {
    printed_in * 12.0 / inches_per_foot
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub name: String,
    pub font: String,
    /// Character height in plan inches (6" is 1/8" on paper at 1/4" scale).
    pub height_in: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: [u8; 3],
    /// `true`: the plan height `height_in` is used as is, so printed size
    /// follows the drawing scale (Chief "Use character height"). `false`:
    /// `printed_pt` is the size on paper whatever the scale.
    pub size_by_scale: bool,
    /// Printed size in points (9 pt = 1/8"), where known.
    pub printed_pt: Option<f64>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle::plan_sized(DEFAULT_TEXT_STYLE_NAME, 6.0, false)
    }
}

impl TextStyle {
    /// An Arial style given by plan height; `printed_pt` is what that height
    /// prints at 1/4" scale.
    pub fn plan_sized(name: impl Into<String>, height_in: f64, bold: bool) -> Self {
        Self {
            name: name.into(),
            font: "Arial".into(),
            height_in,
            bold,
            italic: false,
            underline: false,
            color: [0, 0, 0],
            size_by_scale: true,
            printed_pt: Some(height_in * 0.25 / 12.0 * 72.0),
        }
    }

    /// The same style in another font (`Avenir`).
    pub fn with_font(mut self, font: impl Into<String>) -> Self {
        self.font = font.into();
        self
    }

    /// The same style with italic set or cleared.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Plan character height when drawn at `inches_per_foot` paper scale
    /// (0.25 for 1/4" scale).
    pub fn height_for_scale(&self, inches_per_foot: f64) -> f64 {
        match self.printed_pt {
            Some(pt) if !self.size_by_scale && inches_per_foot > 0.0 => {
                plan_height_for_printed(pt / 72.0, inches_per_foot)
            }
            _ => self.height_in,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyles {
    pub styles: Vec<TextStyle>,
}

impl Default for TextStyles {
    fn default() -> Self {
        Self::chief_defaults()
    }
}

impl TextStyles {
    /// Chief-like starting styles. Only "Default Text Style" (Arial, 6" plan,
    /// 1/8" printed) comes from Daniel's capture; the others are sized to
    /// match it.
    pub fn chief_defaults() -> Self {
        let s = TextStyle::plan_sized;
        Self {
            styles: vec![
                s(DEFAULT_TEXT_STYLE_NAME, 6.0, false),
                s("1/4\" Text Style", 6.0, false),
                s("Room Label Style", 6.0, true),
                s("Schedule Style", 4.5, false),
                s("Default Label Style", 4.5, false),
                s("Dimension Text Style", 4.5, false),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&TextStyle> {
        self.styles.iter().find(|s| s.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.styles.iter().map(|s| s.name.as_str()).collect()
    }

    /// Adds a style if its name is new and non-empty.
    pub fn add(&mut self, style: TextStyle) -> bool {
        if style.name.is_empty() || self.get(&style.name).is_some() {
            return false;
        }
        self.styles.push(style);
        true
    }

    /// Removes a style by name; "Default Text Style" cannot be removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if name == DEFAULT_TEXT_STYLE_NAME {
            return false;
        }
        let before = self.styles.len();
        self.styles.retain(|s| s.name != name);
        self.styles.len() != before
    }

    /// The style a `Layer::text_style` name refers to; an empty or unknown
    /// name gives "Default Text Style" (or the first style if that is gone).
    /// `None` only when there are no styles at all.
    pub fn resolve(&self, name: &str) -> Option<&TextStyle> {
        self.get(name)
            .or_else(|| self.get(DEFAULT_TEXT_STYLE_NAME))
            .or_else(|| self.styles.first())
    }

    /// The text style of a layer, by layer name. Unknown layers use the
    /// default style.
    pub fn resolve_for_layer(&self, layers: &LayerSet, layer: &str) -> Option<&TextStyle> {
        self.resolve(layers.get(layer).map_or("", |l| l.text_style.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::Layer;

    #[test]
    fn chief_default_list() {
        let t = TextStyles::default();
        assert_eq!(
            t.names(),
            vec![
                "Default Text Style",
                "1/4\" Text Style",
                "Room Label Style",
                "Schedule Style",
                "Default Label Style",
                "Dimension Text Style"
            ]
        );
        let d = t.get("Default Text Style").unwrap();
        assert_eq!((d.font.as_str(), d.height_in), ("Arial", 6.0));
        // 1/8" printed = 9 pt.
        assert!((d.printed_pt.unwrap() - 9.0).abs() < 1e-9);
        assert!(t.get("Room Label Style").unwrap().bold);
    }

    #[test]
    fn lookup_and_layer_resolution() {
        let t = TextStyles::default();
        assert_eq!(t.resolve("Schedule Style").unwrap().name, "Schedule Style");
        assert_eq!(t.resolve("").unwrap().name, DEFAULT_TEXT_STYLE_NAME);
        assert_eq!(t.resolve("Nope").unwrap().name, DEFAULT_TEXT_STYLE_NAME);
        let mut layers = LayerSet::default_floor_plan();
        let mut l = Layer::new("Labels", [0, 0, 0], 18);
        l.text_style = "Room Label Style".into();
        layers.add(l);
        assert_eq!(
            t.resolve_for_layer(&layers, "Labels").unwrap().name,
            "Room Label Style"
        );
        assert_eq!(
            t.resolve_for_layer(&layers, "Text").unwrap().name,
            DEFAULT_TEXT_STYLE_NAME
        );
        assert_eq!(
            t.resolve_for_layer(&layers, "Unknown").unwrap().name,
            DEFAULT_TEXT_STYLE_NAME
        );
        let empty = TextStyles { styles: vec![] };
        assert!(empty.resolve("x").is_none());
    }

    #[test]
    fn add_remove_and_sizes() {
        let mut t = TextStyles::default();
        assert!(!t.add(TextStyle::plan_sized("Schedule Style", 1.0, false)));
        assert!(!t.add(TextStyle::plan_sized("", 1.0, false)));
        assert!(t.add(TextStyle::plan_sized("1/8\" Text Style", 12.0, false)));
        assert!(!t.remove(DEFAULT_TEXT_STYLE_NAME));
        assert!(t.remove("1/8\" Text Style"));
        assert!(!t.remove("1/8\" Text Style"));
        // Character-height styles ignore the scale; printed-size styles follow it.
        let mut s = TextStyle::default();
        assert_eq!(s.height_for_scale(0.5), 6.0);
        s.size_by_scale = false;
        assert!((s.height_for_scale(0.25) - 6.0).abs() < 1e-9);
        assert!((s.height_for_scale(0.125) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn json_round_trip_and_sparse_style() {
        let t = TextStyles::default();
        let back: TextStyles = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        let s: TextStyle = serde_json::from_str(r#"{"name":"X","height_in":3.0}"#).unwrap();
        assert_eq!(s.font, "Arial");
        assert_eq!(s.height_in, 3.0);
        let empty: TextStyles = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, TextStyles::default());
    }
}
