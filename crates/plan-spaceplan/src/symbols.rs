//! 2D plan symbols for room boxes (what the assistant draws on the canvas).

use crate::boxes::RoomBox;
use plan_core::Point;
use serde::{Deserialize, Serialize};

/// Text height of box labels, inches (paper size is up to the renderer).
const LABEL_HEIGHT: f64 = 6.0;

/// A drawing primitive for the space-planning canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stroke {
    /// Filled polygon, counter-clockwise, with an RGB fill.
    Polygon { points: Vec<Point>, fill: [u8; 3] },
    /// Centered text anchored at `at`; `height` is the character height.
    Text {
        at: Point,
        text: String,
        height: f64,
    },
}

/// Symbols for the boxes: per box a filled rectangle, its name and an
/// "NNN sq ft" label centered below the name.
pub fn plan_symbols(boxes: &[RoomBox]) -> Vec<Stroke> {
    let mut out = Vec::with_capacity(boxes.len() * 3);
    for b in boxes {
        let (lo, hi) = b.rect;
        out.push(Stroke::Polygon {
            points: vec![lo, Point::new(hi.x, lo.y), hi, Point::new(lo.x, hi.y)],
            fill: b.color,
        });
        let c = b.center();
        out.push(Stroke::Text {
            at: Point::new(c.x, c.y + LABEL_HEIGHT),
            text: b.name.clone(),
            height: LABEL_HEIGHT,
        });
        out.push(Stroke::Text {
            at: Point::new(c.x, c.y - LABEL_HEIGHT),
            text: format!("{:.0} sq ft", b.area_sq_ft()),
            height: LABEL_HEIGHT,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_polygon_and_two_labels_per_box() {
        let b = RoomBox::new(1, "Kitchen", "Kitchen", Point::ZERO, (144.0, 192.0), 0);
        let s = plan_symbols(std::slice::from_ref(&b));
        assert_eq!(s.len(), 3);
        match &s[0] {
            Stroke::Polygon { points, fill } => {
                assert_eq!(points.len(), 4);
                assert_eq!(*fill, b.color);
            }
            other => panic!("expected polygon, got {other:?}"),
        }
        assert!(matches!(&s[1], Stroke::Text { text, .. } if text == "Kitchen"));
        assert!(matches!(&s[2], Stroke::Text { text, .. } if text == "192 sq ft"));
        let json = serde_json::to_string(&s).unwrap();
        let back: Vec<Stroke> = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
