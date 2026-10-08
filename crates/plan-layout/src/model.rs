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

// `plan-elevation` types have no serde impls, so mirror them. (`plan-docs`
// `Scale` and `SheetSize` derive serde themselves.)
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

impl ScaleExt for Scale {
    fn from_label(label: &str) -> Option<Scale> {
        // The inherent `Scale::from_label` lives in plan-docs and wins here.
        Scale::from_label(label)
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
    /// The 2D drawing of a camera object (an elevation, section or wall
    /// elevation camera). The drawing comes from the hook the application sets
    /// on [`crate::LayoutRenderContext::camera_drawing`]; without it the box is
    /// an empty frame.
    Camera { camera_id: Id },
    /// A table of the project's doors, windows, rooms or walls.
    Schedule { kind: ScheduleKind },
    /// Loose CAD in detail space (inches of the detail, drawn at the box scale).
    CadDetail { name: String, items: Vec<CadObject> },
    /// A raster image by file name. The layout does not read files, so the box
    /// prints a placeholder frame with the name; use [`BoxSource::ImageData`]
    /// to embed pixels.
    Image { path: String },
    /// An embedded raster image: `rgba` holds `width * height * 4` bytes,
    /// rows top to bottom, flattened onto white when printed. It is drawn as
    /// an image XObject, scaled to fit the box and centred.
    ImageData {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// Plain text, drawn in paper points.
    Text { text: String, height_pt: f64 },
}

fn one() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

/// Chief's "Layout Edge" line weight in Daniel's preferences, 1/100 mm.
pub const LAYOUT_EDGE_WEIGHT: u32 = 18;

fn edge_weight() -> u32 {
    LAYOUT_EDGE_WEIGHT
}

/// A rectangular viewport on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub id: Id,
    /// `(lower-left, upper-right)` in paper inches, origin at the sheet's bottom-left.
    pub rect_in: (Point, Point),
    pub source: BoxSource,
    pub scale: Scale,
    /// Caption drawn under the box, with the scale note.
    pub label: Option<String>,
    /// Draw the box frame.
    pub border: bool,
    /// Multiplier on every pen weight inside the box.
    #[serde(default = "one")]
    pub line_weight_scale: f64,
    /// Cut content off at the box frame (a PDF clip rectangle).
    pub clip: bool,
    /// Elevation and section boxes: fill wall faces with their material's
    /// pattern (see [`crate::wall_face_hatch`]).
    #[serde(default = "yes")]
    pub hatch_materials: bool,
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
            hatch_materials: true,
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
    /// Chief's "Page Template" page: it is not printed itself, its boxes and
    /// CAD repeat on every other page (under that page's own content).
    #[serde(default)]
    pub template_page: bool,
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
    pub sheet: SheetSize,
    pub pages: Vec<LayoutPage>,
    pub title_block: TitleBlockTemplate,
    /// Border inset from the sheet edge, paper inches.
    pub margins_in: f64,
    /// Print the sheet index on the first page.
    #[serde(default)]
    pub sheet_index: bool,
    /// Fill every page with Chief's layout background (249, 248, 244).
    #[serde(default = "yes")]
    pub page_background: bool,
    /// Line weight of the page border ("Layout Edge"), 1/100 mm.
    #[serde(default = "edge_weight")]
    pub edge_line_weight: u32,
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
            page_background: true,
            edge_line_weight: LAYOUT_EDGE_WEIGHT,
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
            template_page: false,
        });
        self.pages.last_mut().expect("just pushed")
    }

    /// Pages that are printed: every page that is not a template page.
    pub fn content_pages(&self) -> Vec<&LayoutPage> {
        self.pages.iter().filter(|p| !p.template_page).collect()
    }

    /// The template pages, whose contents repeat on every printed page.
    pub fn template_pages(&self) -> Vec<&LayoutPage> {
        self.pages.iter().filter(|p| p.template_page).collect()
    }

    /// Sheet sizes this layout can be switched to: all of
    /// [`SheetSize::ALL`] (landscape), the current size first.
    pub fn sheet_sizes_available(&self) -> Vec<SheetSize> {
        std::iter::once(self.sheet)
            .chain(SheetSize::ALL.into_iter().filter(|s| *s != self.sheet))
            .collect()
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
        // plan-docs now also knows the 1" = 10' / 1" = 20' and metric scales.
        assert_eq!(Scale::from_label("1\" = 10'"), Some(Scale::OneInchEq10Ft));
        assert_eq!(Scale::from_label("1\" = 7'"), None);
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
