//! The layout data model: pages, boxes and their sources.

use crate::titleblock::{TitleBlockStyle, TitleBlockTemplate};
use plan_core::{CadObject, Id, Point};
use plan_docs::{Scale, SheetSize};
use plan_elevation::{SectionCut, ViewDir};
use serde::{Deserialize, Serialize};

/// Vertical space reserved below a box for its label, paper inches.
pub const LABEL_GAP_IN: f64 = 0.45;

/// Width of the right title strip, paper inches (capped on small sheets).
pub(crate) const RIGHT_STRIP_IN: f64 = 2.5;
/// Height of the bottom title strip, paper inches.
pub(crate) const BOTTOM_STRIP_IN: f64 = 1.25;

// `plan-docs` and `plan-elevation` types have no serde impls, so mirror them.
#[derive(Serialize, Deserialize)]
#[serde(remote = "SheetSize")]
enum SheetSizeDef {
    ArchD,
    ArchC,
    Letter,
    Tabloid,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Scale")]
enum ScaleDef {
    HalfInch,
    QuarterInch,
    ThreeSixteenths,
    EighthInch,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "SectionCut")]
struct SectionCutDef {
    plane_normal: ViewDir,
    offset: f64,
}

/// Extra constructors for [`plan_docs::Scale`] (which lives in another crate).
pub trait ScaleExt: Sized {
    /// Parse a scale note such as `1/4" = 1'`, `1/4"=1'-0"` or `3/16 in = 1 ft`.
    /// Returns `None` for text that is not one of the supported scales.
    fn from_label(label: &str) -> Option<Self>;
}

fn parse_inches(s: &str) -> Option<f64> {
    let s = s
        .trim()
        .trim_end_matches(['"', '\u{201d}'])
        .trim_end_matches("inches")
        .trim_end_matches("inch")
        .trim_end_matches("in")
        .trim();
    match s.split_once('/') {
        Some((n, d)) => {
            let (n, d) = (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?);
            (d != 0.0).then_some(n / d)
        }
        None => s.parse().ok(),
    }
}

impl ScaleExt for Scale {
    fn from_label(label: &str) -> Option<Scale> {
        let (lhs, rhs) = label.split_once('=')?;
        let rhs = rhs.trim();
        let one_foot = rhs.starts_with("1'")
            || rhs.starts_with("1 '")
            || rhs.starts_with("1ft")
            || rhs.starts_with("1 ft")
            || rhs.starts_with("1\u{2019}");
        if !one_foot {
            return None;
        }
        let v = parse_inches(lhs)?;
        Scale::DESCENDING
            .into_iter()
            .find(|s| (s.inches_per_foot() - v).abs() < 1e-9)
    }
}

/// Which schedule a [`BoxSource::Schedule`] box shows (all floors, one table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleKind {
    Door,
    Window,
    Room,
    Wall,
}

/// What a layout box shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BoxSource {
    /// A floor plan. `layer_set` names the layer set whose visibility applies:
    /// the project's own set (matched by name, or any unknown name) or `"All"`
    /// to ignore layer visibility.
    PlanView { floor: usize, layer_set: String },
    /// An exterior elevation seen from `dir`.
    Elevation { dir: ViewDir },
    /// A cross section.
    Section {
        #[serde(with = "SectionCutDef")]
        cut: SectionCut,
    },
    /// A table of the project's doors, windows, rooms or walls.
    Schedule { kind: ScheduleKind },
    /// Loose CAD in detail space (inches of the detail, drawn at the box scale).
    CadDetail { name: String, items: Vec<CadObject> },
    /// A raster image. The PDF writer has no image support yet, so the box
    /// prints a placeholder frame with the file name.
    Image { path: String },
    /// Plain text, drawn in paper points.
    Text { text: String, height_pt: f64 },
}

fn one() -> f64 {
    1.0
}

/// A rectangular viewport on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub id: Id,
    /// `(lower-left, upper-right)` in paper inches, origin at the sheet's bottom-left.
    pub rect_in: (Point, Point),
    pub source: BoxSource,
    #[serde(with = "ScaleDef")]
    pub scale: Scale,
    /// Caption drawn under the box, with the scale note.
    pub label: Option<String>,
    /// Draw the box frame.
    pub border: bool,
    /// Multiplier on every pen weight inside the box.
    #[serde(default = "one")]
    pub line_weight_scale: f64,
    /// Cut content off at the box frame.
    pub clip: bool,
}

impl LayoutBox {
    /// A box with a frame, clipping on and weights unscaled.
    pub fn new(id: Id, rect_in: (Point, Point), source: BoxSource, scale: Scale) -> Self {
        Self {
            id,
            rect_in,
            source,
            scale,
            label: None,
            border: true,
            line_weight_scale: 1.0,
            clip: true,
        }
    }

    /// Width and height of the box, paper inches.
    pub fn size_in(&self) -> (f64, f64) {
        let (a, b) = self.rect_in;
        ((b.x - a.x).abs(), (b.y - a.y).abs())
    }

    /// Normalised `[x_min, y_min, x_max, y_max]` in paper inches.
    pub(crate) fn bounds_in(&self) -> [f64; 4] {
        let (a, b) = self.rect_in;
        [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
    }
}

/// One sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutPage {
    /// Sheet number; shown as `A-{number}` (the cover is 0).
    pub number: u32,
    pub title: String,
    pub boxes: Vec<LayoutBox>,
    /// Page annotations in paper inches (title text, notes, leaders).
    pub cad: Vec<CadObject>,
}

impl LayoutPage {
    /// Sheet number text, e.g. `A-2`.
    pub fn sheet_number(&self) -> String {
        format!("A-{}", self.number)
    }
}

/// A multi-page layout file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    #[serde(with = "SheetSizeDef")]
    pub sheet: SheetSize,
    pub pages: Vec<LayoutPage>,
    pub title_block: TitleBlockTemplate,
    /// Border inset from the sheet edge, paper inches.
    pub margins_in: f64,
    /// Print the sheet index on the first page.
    #[serde(default)]
    pub sheet_index: bool,
}

impl Layout {
    /// An empty layout with the presentation title block and 0.5" margins.
    pub fn new(name: impl Into<String>, sheet: SheetSize) -> Self {
        Self {
            name: name.into(),
            sheet,
            pages: Vec::new(),
            title_block: TitleBlockTemplate::presentation_18x24(),
            margins_in: 0.5,
            sheet_index: false,
        }
    }

    /// The page with sheet number `number`.
    pub fn page(&self, number: u32) -> Option<&LayoutPage> {
        self.pages.iter().find(|p| p.number == number)
    }

    /// Mutable access to the page with sheet number `number`.
    pub fn page_mut(&mut self, number: u32) -> Option<&mut LayoutPage> {
        self.pages.iter_mut().find(|p| p.number == number)
    }

    /// Append a page and return it.
    pub fn add_page(&mut self, number: u32, title: impl Into<String>) -> &mut LayoutPage {
        self.pages.push(LayoutPage {
            number,
            title: title.into(),
            boxes: Vec::new(),
            cad: Vec::new(),
        });
        self.pages.last_mut().expect("just pushed")
    }

    /// An id not used by any box in the layout.
    pub(crate) fn next_box_id(&self) -> Id {
        self.pages
            .iter()
            .flat_map(|p| p.boxes.iter().map(|b| b.id))
            .max()
            .map_or(1, |m| m + 1)
    }

    /// Width of the right title strip for this sheet, paper inches.
    pub(crate) fn right_strip_in(&self) -> f64 {
        RIGHT_STRIP_IN.min(self.sheet.inches().0 * 0.22)
    }

    /// The area boxes are packed into (inside the border, beside the title
    /// block): `(lower-left, upper-right)` in paper inches.
    pub fn drawing_area(&self) -> (Point, Point) {
        let (w, h) = self.sheet.inches();
        let m = self.margins_in;
        let (mut right, mut bottom) = (w - m, m);
        match self.title_block.style {
            TitleBlockStyle::RightStrip => right -= self.right_strip_in(),
            TitleBlockStyle::BottomStrip => bottom += BOTTOM_STRIP_IN,
            TitleBlockStyle::Custom(_) => {}
        }
        (Point::new(m, bottom), Point::new(right, h - m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_labels_parse() {
        assert_eq!(Scale::from_label("1/4\" = 1'"), Some(Scale::QuarterInch));
        assert_eq!(Scale::from_label("1/4\"=1'-0\""), Some(Scale::QuarterInch));
        assert_eq!(
            Scale::from_label("3/16 in = 1 ft"),
            Some(Scale::ThreeSixteenths)
        );
        assert_eq!(Scale::from_label("0.5\" = 1'"), Some(Scale::HalfInch));
        assert_eq!(Scale::from_label("1\" = 10'"), None);
        assert_eq!(Scale::from_label("1/4"), None);
        for s in Scale::DESCENDING {
            assert_eq!(Scale::from_label(s.label()), Some(s));
        }
        assert!((Scale::QuarterInch.points_per_inch() - 1.5).abs() < 1e-12);
    }

    #[test]
    fn drawing_area_leaves_room_for_the_strip() {
        let l = Layout::new("t", SheetSize::ArchC);
        let (lo, hi) = l.drawing_area();
        assert_eq!((lo.x, lo.y), (0.5, 0.5));
        assert_eq!((hi.x, hi.y), (21.0, 17.5));
    }
}
