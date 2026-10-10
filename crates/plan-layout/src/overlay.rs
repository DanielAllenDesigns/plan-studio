//! Plan overlay: objects the layout crate cannot build itself (cabinets,
//! stairs, placed symbols) drawn in plan boxes.
//!
//! `plan-layout` has no dependency on `plan-cabinets`, `plan-stairs` or the
//! symbol library, so the application hands it the finished strokes through
//! [`crate::LayoutRenderContext::plan_overlay`]: for a floor, a list of
//! [`PlanOverlayItem`]s in plan inches. Each belongs to a layer (hidden with
//! it, drawn in its colour and weight) and is a line, a filled polygon or a
//! text. A dashed item is drawn dashed (the treads seen through a stairwell,
//! merged countertops).

use plan_core::Point;

/// What one overlay item draws.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayShape {
    /// A line through `points` (a closed ring when `closed`).
    Polyline { points: Vec<Point>, closed: bool },
    /// A filled polygon: `rgb` colour at `alpha` (0..1) over the white sheet.
    Fill {
        points: Vec<Point>,
        rgb: [u8; 3],
        alpha: f32,
    },
    /// A text at `at` (its baseline start), `height` plan inches tall,
    /// turned `angle` radians.
    Text {
        at: Point,
        text: String,
        height: f64,
        angle: f64,
    },
}

/// One stroke, fill or text a plan box draws on top of the building.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanOverlayItem {
    /// The layer it is drawn on; a hidden layer hides it.
    pub layer: String,
    pub shape: OverlayShape,
    /// Draw the line dashed.
    pub dashed: bool,
    /// Multiplier on the layer's pen width (`1.0` is the layer's weight).
    pub weight: f64,
}

impl PlanOverlayItem {
    /// A solid line at the layer's weight.
    pub fn line(layer: impl Into<String>, points: Vec<Point>, closed: bool) -> Self {
        Self {
            layer: layer.into(),
            shape: OverlayShape::Polyline { points, closed },
            dashed: false,
            weight: 1.0,
        }
    }

    /// The same item drawn dashed.
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    /// The same item at `weight` times the layer's pen.
    pub fn weighted(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// The overlay of floor `n`: what the application draws on top of the walls.
pub type PlanOverlayFn<'a> = Box<dyn Fn(usize) -> Vec<PlanOverlayItem> + 'a>;
