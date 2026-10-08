//! The face layout tree: Chief's "Front/Sides/Back" face items.

use serde::{Deserialize, Serialize};

/// Chief's default separation (rail) height, inches.
const SEPARATION: f64 = 1.5;
/// Chief's default drawer front height, inches.
const DRAWER: f64 = 6.0;
/// Slack when comparing summed lengths.
const EPS: f64 = 1e-9;

/// One item in a face layout. A `height` of `0.0` means "auto": the item
/// receives an equal share of whatever height the fixed items leave over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FaceItem {
    /// A horizontal rail between items (a face-frame rail when framed).
    Separation { height: f64 },
    /// A drawer front.
    Drawer { height: f64 },
    /// A door whose swing side is chosen automatically by its position.
    DoorAuto { height: f64 },
    /// A door hinged on the left.
    DoorLeft { height: f64 },
    /// A door hinged on the right.
    DoorRight { height: f64 },
    /// A pair of doors meeting in the middle.
    DoubleDoor { height: f64 },
    /// An open (front-less) bay.
    Opening { height: f64 },
    /// An appliance or fixture front (for example a dishwasher or a sink tip-out).
    Appliance { height: f64, name: String },
    /// Items placed side by side. Its own `height` follows the vertical rules;
    /// each cell's item fills that height.
    HorizontalLayout { height: f64, cells: Vec<FaceCell> },
}

/// A cell of a [`FaceItem::HorizontalLayout`]: an item and its optional width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceCell {
    pub item: FaceItem,
    /// Fixed width in inches; `None` shares the remaining width equally.
    pub width: Option<f64>,
}

impl FaceItem {
    /// The item's declared height (`0.0` = auto).
    pub fn height(&self) -> f64 {
        match self {
            FaceItem::Separation { height }
            | FaceItem::Drawer { height }
            | FaceItem::DoorAuto { height }
            | FaceItem::DoorLeft { height }
            | FaceItem::DoorRight { height }
            | FaceItem::DoubleDoor { height }
            | FaceItem::Opening { height }
            | FaceItem::Appliance { height, .. }
            | FaceItem::HorizontalLayout { height, .. } => *height,
        }
    }

    /// A copy of this item with its height replaced.
    pub fn with_height(&self, h: f64) -> FaceItem {
        let mut item = self.clone();
        match &mut item {
            FaceItem::Separation { height }
            | FaceItem::Drawer { height }
            | FaceItem::DoorAuto { height }
            | FaceItem::DoorLeft { height }
            | FaceItem::DoorRight { height }
            | FaceItem::DoubleDoor { height }
            | FaceItem::Opening { height }
            | FaceItem::Appliance { height, .. }
            | FaceItem::HorizontalLayout { height, .. } => *height = h,
        }
        item
    }
}

/// A leaf face item placed in face coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFace {
    /// `(x, y, w, h)`: `x` from the face's left edge, `y` up from the face's
    /// bottom edge. The item's `height` equals `h`.
    pub rect: (f64, f64, f64, f64),
    pub item: FaceItem,
}

/// The vertical layout of one cabinet side. Items run top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceLayout {
    pub items: Vec<FaceItem>,
    /// Stile width for framed cabinets (Chief's default is 1.5").
    pub frame_width: f64,
}

impl Default for FaceLayout {
    fn default() -> Self {
        Self::empty()
    }
}

impl FaceLayout {
    /// A layout with no items.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            frame_width: SEPARATION,
        }
    }

    /// Chief's default base face: separation, 6" drawer, separation, auto
    /// door, separation. `height` is the face height; the drawer shrinks when
    /// the face is too short for the fixed items to fit.
    pub fn base_default(height: f64) -> Self {
        let drawer = DRAWER.min(((height - 3.0 * SEPARATION) / 3.0).max(0.0));
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::Drawer { height: drawer },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// A wall cabinet face: one auto door, or two stacked doors for tall
    /// (42" and up) cabinets.
    pub fn wall_default(height: f64) -> Self {
        let mut items = vec![
            FaceItem::Separation { height: SEPARATION },
            FaceItem::DoorAuto { height: 0.0 },
            FaceItem::Separation { height: SEPARATION },
        ];
        if height >= 42.0 {
            items.push(FaceItem::DoorAuto { height: 0.0 });
            items.push(FaceItem::Separation { height: SEPARATION });
        }
        Self {
            items,
            frame_width: SEPARATION,
        }
    }

    /// A pantry/oven-tower face: an auto-height upper door over a lower door of
    /// at most 36" (45% of the face on short faces).
    pub fn full_height_default(height: f64) -> Self {
        let lower = 36.0_f64.min(height * 0.45).max(0.0);
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoorAuto { height: lower },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// `n` equal auto-height drawers separated by rails (empty for `n == 0`).
    pub fn drawer_bank(n: usize) -> Self {
        let mut items = Vec::new();
        if n > 0 {
            items.push(FaceItem::Separation { height: SEPARATION });
            for _ in 0..n {
                items.push(FaceItem::Drawer { height: 0.0 });
                items.push(FaceItem::Separation { height: SEPARATION });
            }
        }
        Self {
            items,
            frame_width: SEPARATION,
        }
    }

    /// A sink base: a 6" tip-out front named "Sink" over a double door.
    pub fn sink_base() -> Self {
        Self {
            items: vec![
                FaceItem::Separation { height: SEPARATION },
                FaceItem::Appliance {
                    height: DRAWER,
                    name: "Sink".to_string(),
                },
                FaceItem::Separation { height: SEPARATION },
                FaceItem::DoubleDoor { height: 0.0 },
                FaceItem::Separation { height: SEPARATION },
            ],
            frame_width: SEPARATION,
        }
    }

    /// True when any item (at any depth) is an [`FaceItem::Appliance`] named `name`.
    pub fn has_appliance(&self, name: &str) -> bool {
        fn walk(item: &FaceItem, name: &str) -> bool {
            match item {
                FaceItem::Appliance { name: n, .. } => n == name,
                FaceItem::HorizontalLayout { cells, .. } => {
                    cells.iter().any(|c| walk(&c.item, name))
                }
                _ => false,
            }
        }
        self.items.iter().any(|i| walk(i, name))
    }

    /// Place every leaf item in face coordinates (see [`ResolvedFace::rect`]).
    ///
    /// Items with height `0` split the height left over by the fixed items
    /// equally; `HorizontalLayout` cells with no width do the same with the
    /// width. Horizontal layouts are flattened to their cells. Fixed items are
    /// stacked from the top, so a layout without auto items that falls short
    /// of `total_height` leaves a gap at the bottom.
    ///
    /// # Errors
    /// Returns a message when fixed heights (or widths) exceed what is
    /// available, or when a size is negative or not finite.
    pub fn resolve(
        &self,
        total_height: f64,
        total_width: f64,
    ) -> Result<Vec<ResolvedFace>, String> {
        let heights = distribute(
            self.items.iter().map(FaceItem::height).collect(),
            total_height,
            "height",
        )?;
        let mut out = Vec::new();
        let mut top = total_height;
        for (item, h) in self.items.iter().zip(heights) {
            let y = top - h;
            place(item, 0.0, y, total_width, h, &mut out)?;
            top = y;
        }
        Ok(out)
    }
}

/// Split `total` among `sizes` (`<= 0` = auto) and return each final size.
fn distribute(sizes: Vec<f64>, total: f64, what: &str) -> Result<Vec<f64>, String> {
    if sizes.iter().any(|s| !s.is_finite() || *s < 0.0) {
        return Err(format!("{what} must be finite and not negative"));
    }
    let fixed: f64 = sizes.iter().sum();
    if fixed > total + EPS {
        return Err(format!(
            "fixed item {what}s total {fixed:.3}\" but only {total:.3}\" is available"
        ));
    }
    let autos = sizes.iter().filter(|s| **s <= 0.0).count();
    let share = if autos > 0 {
        (total - fixed) / autos as f64
    } else {
        0.0
    };
    Ok(sizes
        .into_iter()
        .map(|s| if s <= 0.0 { share } else { s })
        .collect())
}

/// Emit `item` into the rect `(x, y, w, h)`, recursing through horizontal layouts.
fn place(
    item: &FaceItem,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    out: &mut Vec<ResolvedFace>,
) -> Result<(), String> {
    match item {
        FaceItem::HorizontalLayout { cells, .. } => {
            let widths = distribute(
                cells.iter().map(|c| c.width.unwrap_or(0.0)).collect(),
                w,
                "width",
            )?;
            let mut cx = x;
            for (cell, cw) in cells.iter().zip(widths) {
                place(&cell.item, cx, y, cw, h, out)?;
                cx += cw;
            }
        }
        leaf => out.push(ResolvedFace {
            rect: (x, y, w, h),
            item: leaf.with_height(h),
        }),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_default_fills_box_height_exactly() {
        let r = FaceLayout::base_default(34.5).resolve(34.5, 21.0).unwrap();
        assert_eq!(r.len(), 5);
        let sum: f64 = r.iter().map(|f| f.rect.3).sum();
        assert!((sum - 34.5).abs() < 1e-9, "sum {sum}");
        // Door gets the rest: 34.5 - 1.5 - 6 - 1.5 - 1.5 = 24.
        assert!((r[3].rect.3 - 24.0).abs() < 1e-9);
        // Top item sits at the top of the face, bottom item at y = 0.
        assert!((r[0].rect.1 + r[0].rect.3 - 34.5).abs() < 1e-9);
        assert!(r[4].rect.1.abs() < 1e-9);
    }

    #[test]
    fn resolve_errors_when_fixed_items_exceed_height() {
        let layout = FaceLayout {
            items: vec![
                FaceItem::Drawer { height: 20.0 },
                FaceItem::Drawer { height: 20.0 },
            ],
            frame_width: 1.5,
        };
        assert!(layout.resolve(34.5, 24.0).is_err());
    }

    #[test]
    fn horizontal_layout_splits_widths() {
        let layout = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::Drawer { height: 0.0 },
                        width: Some(10.0),
                    },
                    FaceCell {
                        item: FaceItem::DoorAuto { height: 0.0 },
                        width: None,
                    },
                    FaceCell {
                        item: FaceItem::DoorAuto { height: 0.0 },
                        width: None,
                    },
                ],
            }],
            frame_width: 1.5,
        };
        let r = layout.resolve(30.0, 40.0).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].rect, (0.0, 0.0, 10.0, 30.0));
        assert_eq!(r[1].rect, (10.0, 0.0, 15.0, 30.0));
        assert_eq!(r[2].rect, (25.0, 0.0, 15.0, 30.0));
        let too_wide = FaceLayout {
            items: vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![FaceCell {
                    item: FaceItem::Drawer { height: 0.0 },
                    width: Some(50.0),
                }],
            }],
            frame_width: 1.5,
        };
        assert!(too_wide.resolve(30.0, 40.0).is_err());
    }

    #[test]
    fn presets_resolve_and_sink_is_detected() {
        for (l, h) in [
            (FaceLayout::wall_default(30.0), 30.0),
            (FaceLayout::wall_default(42.0), 42.0),
            (FaceLayout::full_height_default(80.0), 80.0),
            (FaceLayout::drawer_bank(3), 30.0),
            (FaceLayout::sink_base(), 30.0),
        ] {
            let r = l.resolve(h, 24.0).unwrap();
            assert!(!r.is_empty());
        }
        assert!(FaceLayout::sink_base().has_appliance("Sink"));
        assert!(!FaceLayout::base_default(34.5).has_appliance("Sink"));
        assert!(FaceLayout::drawer_bank(0).items.is_empty());
    }
}
