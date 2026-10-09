//! The layout data model: pages, boxes and their sources.

use crate::annot::{PageLeader, RevisionCloud};
use crate::layers::{LayoutLayers, LAYER_CAD, LAYER_TEXT};
use crate::textfit::TextFit;
use crate::pages::{PageNumbers, PageRevision};
use crate::titleblock::{TitleBlockStyle, TitleBlockTemplate};
use plan_core::{CadItem, CadObject, Id, Point};
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

/// Horizontal alignment of a text box's lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
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
    /// A schedule placed in the plan (`floor`, `id` of its
    /// `plan_core::schedules::Schedule`): the same table, with its columns,
    /// sort, filter, grouping and totals, kept up to date.
    PlacedSchedule { floor: usize, id: Id },
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
        /// Stored as base64 text in the layout's JSON (a list of numbers
        /// would take four times the room); files with a list still load.
        #[serde(with = "pixels_text")]
        rgba: Vec<u8>,
    },
    /// A text box: lines of text in paper points, aligned within the box,
    /// optionally bold. Double-click a text box in the layout view to edit it.
    Text {
        text: String,
        height_pt: f64,
        #[serde(default)]
        align: TextAlign,
        #[serde(default)]
        bold: bool,
    },
    /// A perspective (full camera) view of camera `camera_id`, ray traced at
    /// low sample counts by the hook the application sets on
    /// [`crate::LayoutRenderContext::perspective_image`] and embedded as an
    /// image. Without the hook the box is a placeholder frame.
    Perspective { camera_id: Id },
    /// The Materials List as a table: one `category` (`None` = all) of one
    /// `floor` (`None` = every floor), priced from the render context's master
    /// list. It follows the plan like a placed schedule.
    Materials {
        floor: Option<usize>,
        category: Option<String>,
    },
    /// The sheet index as a table (sheet number and title of every printed
    /// page), kept up to date as pages are added, renamed or reordered.
    SheetIndex,
    /// The Layout Page Table: label, title and description of every page
    /// that is listed (Include in Layout Table) and has data. On a page
    /// template it shows on every page that uses the template.
    PageTable,
    /// The Layout Revision Table: the revisions of the page it is drawn on.
    /// Placed on a page template it lists the revisions of each page that
    /// uses the template.
    RevisionTable,
}

/// `ImageData` pixels in JSON: base64 text, or (older files) a list of bytes.
mod pixels_text {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub(super) fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = (u32::from(chunk[0]) << 16)
                | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
                | u32::from(*chunk.get(2).unwrap_or(&0));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    pub(super) fn decode(text: &str) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(text.len() / 4 * 3);
        let (mut acc, mut bits) = (0u32, 0);
        for c in text.bytes() {
            if c == b'=' {
                break;
            }
            let v = ALPHABET.iter().position(|a| *a == c)? as u32;
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((acc >> bits) & 0xFF) as u8);
            }
        }
        Some(out)
    }

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        encode(bytes).serialize(s)
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Text(String),
        List(Vec<u8>),
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        match Stored::deserialize(d)? {
            Stored::List(v) => Ok(v),
            Stored::Text(t) => {
                decode(&t).ok_or_else(|| serde::de::Error::custom("bad base64 in image pixels"))
            }
        }
    }
}

impl BoxSource {
    /// A left-aligned, regular-weight text box source.
    pub fn text(text: impl Into<String>, height_pt: f64) -> Self {
        BoxSource::Text {
            text: text.into(),
            height_pt,
            align: TextAlign::Left,
            bold: false,
        }
    }
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
    /// Turns the box's content, counter-clockwise, in degrees. Only
    /// multiples of 90 are used (see [`LayoutBox::quarter_turns`]); the box
    /// itself, its frame and its caption stay where they are.
    #[serde(default)]
    pub rotation_deg: f64,
    /// Perspective boxes: the resolution the view is rendered at, dots per
    /// paper inch (`0` = [`DEFAULT_PERSPECTIVE_DPI`]).
    #[serde(default)]
    pub dpi: u32,
    /// Perspective boxes: ray-trace samples per pixel (`0` =
    /// [`DEFAULT_PERSPECTIVE_SAMPLES`]).
    #[serde(default)]
    pub samples: u32,
    /// Text boxes: wrap at the box width, shrink to fit, or leave as typed.
    #[serde(default)]
    pub text_fit: TextFit,
}

/// Resolution of a perspective box that has no DPI of its own: a 6" x 4.5"
/// box is rendered at 480 x 360.
pub const DEFAULT_PERSPECTIVE_DPI: u32 = 80;
/// Samples per pixel of a perspective box that has none of its own.
pub const DEFAULT_PERSPECTIVE_SAMPLES: u32 = 8;
/// Most pixels along one side of a perspective render.
pub const MAX_PERSPECTIVE_SIDE_PX: u32 = 4096;
/// Most pixels in one perspective render (the ray tracer's budget).
pub const MAX_PERSPECTIVE_PIXELS: u64 = 8_000_000;

/// The pixel size of a perspective render for a `w_in` x `h_in` box at `dpi`
/// dots per inch (`0` = the default), scaled down to the render budget. At
/// least 8 x 8.
pub fn perspective_pixels(w_in: f64, h_in: f64, dpi: u32) -> (u32, u32) {
    let dpi = if dpi == 0 {
        DEFAULT_PERSPECTIVE_DPI
    } else {
        dpi
    };
    let (mut w, mut h) = (w_in * f64::from(dpi), h_in * f64::from(dpi));
    let side = f64::from(MAX_PERSPECTIVE_SIDE_PX);
    let k = (side / w.max(h).max(1.0))
        .min((MAX_PERSPECTIVE_PIXELS as f64 / (w * h).max(1.0)).sqrt())
        .min(1.0);
    w *= k;
    h *= k;
    ((w.round() as u32).max(8), (h.round() as u32).max(8))
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
            rotation_deg: 0.0,
            dpi: 0,
            samples: 0,
            text_fit: TextFit::Wrap,
        }
    }

    /// Resolution of a perspective box, dots per inch.
    pub fn effective_dpi(&self) -> u32 {
        if self.dpi == 0 {
            DEFAULT_PERSPECTIVE_DPI
        } else {
            self.dpi
        }
    }

    /// Samples per pixel of a perspective box.
    pub fn effective_samples(&self) -> u32 {
        if self.samples == 0 {
            DEFAULT_PERSPECTIVE_SAMPLES
        } else {
            self.samples
        }
    }

    /// The content rotation as 0..=3 quarter turns counter-clockwise
    /// (other angles round to the nearest quarter).
    pub fn quarter_turns(&self) -> u8 {
        (((self.rotation_deg / 90.0).round() as i64).rem_euclid(4)) as u8
    }

    /// Width and height of the box, paper inches.
    pub fn size_in(&self) -> (f64, f64) {
        let (a, b) = self.rect_in;
        ((b.x - a.x).abs(), (b.y - a.y).abs())
    }

    /// Normalised `[x_min, y_min, x_max, y_max]` in paper inches.
    pub fn bounds_in(&self) -> [f64; 4] {
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
    /// Leaders (text with an arrow) in paper inches; they share the id space
    /// of [`cad`](Self::cad).
    #[serde(default)]
    pub leaders: Vec<PageLeader>,
    /// Revision clouds in paper inches; they share the id space of
    /// [`cad`](Self::cad).
    #[serde(default)]
    pub clouds: Vec<RevisionCloud>,
    /// Page Specification: this page's own sheet, `(width, height)` in paper
    /// inches as it prints, instead of the layout's. `None` follows the
    /// layout.
    #[serde(default)]
    pub size_override_in: Option<(f64, f64)>,
    /// Page Specification: the page prints without the border and title
    /// block (a full-page picture or a cover).
    #[serde(default)]
    pub no_title_block: bool,
    /// Page Information: the label. Text with a `#` is a numbering pattern
    /// (`A-#`, `A0.#`): `#` becomes the next free number among the pages
    /// with the same pattern (see [`crate::resolve_labels`]). Empty keeps the
    /// plain sheet number `A-{number}`. Duplicate labels are legal.
    #[serde(default)]
    pub label: String,
    /// Page Information: the description (layout tables and macros).
    #[serde(default)]
    pub description: String,
    /// Page Information: free comments (macros).
    #[serde(default)]
    pub comments: String,
    /// Page Information: list this page in a Layout Page Table (cleared for
    /// page templates).
    #[serde(default = "yes")]
    pub in_layout_table: bool,
    /// The page template assigned to this page: the `number` of a template
    /// page. `None` is the default template (the first template page, like
    /// Chief's page zero). Template pages have none.
    #[serde(default)]
    pub template: Option<u32>,
    /// Revisions of this page (Page Information > Page Revisions).
    #[serde(default)]
    pub revisions: Vec<PageRevision>,
    /// The numbers of this page frozen while the layout is printed with a
    /// range (see [`Layout::bake_numbering`]); never stored.
    #[serde(skip)]
    pub baked: Option<PageNumbers>,
}

impl LayoutPage {
    /// An id not used by any annotation of the page (CAD, leader or cloud).
    pub fn next_cad_id(&self) -> Id {
        self.cad
            .iter()
            .map(|o| o.id)
            .chain(self.leaders.iter().map(|l| l.id))
            .chain(self.clouds.iter().map(|c| c.id))
            .max()
            .map_or(1, |m| m + 1)
    }

    /// Adds layout CAD (paper inches) to the page and returns its id. Text goes
    /// on the `Text` layout layer, everything else on `Layout CAD`.
    pub fn add_cad(&mut self, item: CadItem) -> Id {
        let id = self.next_cad_id();
        let layer = if matches!(item, CadItem::Text { .. }) {
            LAYER_TEXT
        } else {
            LAYER_CAD
        };
        self.cad.push(CadObject {
            id,
            layer: layer.to_string(),
            item,
        });
        id
    }

    /// A line on the page.
    pub fn add_line(&mut self, a: Point, b: Point) -> Id {
        self.add_cad(CadItem::Line { a, b })
    }

    /// A rectangle on the page from two opposite corners.
    pub fn add_rect(&mut self, a: Point, b: Point) -> Id {
        self.add_cad(CadItem::Polyline {
            points: vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
            closed: true,
        })
    }

    /// A polyline on the page.
    pub fn add_polyline(&mut self, points: Vec<Point>, closed: bool) -> Id {
        self.add_cad(CadItem::Polyline { points, closed })
    }

    /// Text on the page, bottom-left at `pos`, `height_in` paper inches tall.
    pub fn add_text(&mut self, pos: Point, text: impl Into<String>, height_in: f64) -> Id {
        self.add_cad(CadItem::Text {
            pos,
            text: text.into(),
            height: height_in,
            angle: 0.0,
        })
    }

    /// Sheet number text without the layout around it: the frozen label of a
    /// printed range, a fixed label as typed, else `A-2` (a `#` pattern needs
    /// the other pages: use [`Layout::sheet_number_of`]).
    pub fn sheet_number(&self) -> String {
        if let Some(b) = &self.baked {
            return b.label.clone();
        }
        let l = self.label.trim();
        if l.is_empty() {
            format!("A-{}", self.number)
        } else {
            l.replace('#', &self.number.to_string())
        }
    }

    /// Does the page have anything on it? A page without data does not print.
    pub fn has_data(&self) -> bool {
        !(self.boxes.is_empty()
            && self.cad.is_empty()
            && self.leaders.is_empty()
            && self.clouds.is_empty())
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
    /// Portrait orientation: the sheet is as tall as `sheet` is wide.
    #[serde(default)]
    pub portrait: bool,
    /// Layer Display Options of the layout: which layout layers show and
    /// their line weights.
    #[serde(default)]
    pub layers: LayoutLayers,
    /// Customize Sheet Sizes: the sizes this layout adds to the standard
    /// list.
    #[serde(default)]
    pub custom_sizes: Vec<CustomSheetSize>,
    /// Customize Sheet Sizes: standard sizes left out of the size lists.
    #[serde(default)]
    pub hidden_sizes: Vec<SheetSize>,
    /// The layout's own sheet when it is one of the custom sizes (it wins
    /// over [`sheet`](Self::sheet), which then only names the nearest
    /// standard size for the plan's Drawing Sheet).
    #[serde(default)]
    pub custom_sheet: Option<CustomSheetSize>,
    /// General Layout Defaults: drags snap to the grid.
    #[serde(default = "yes")]
    pub snap_grid: bool,
    /// General Layout Defaults: the Grid Snap Unit, paper inches. It is also
    /// the step of an arrow-key nudge.
    #[serde(default = "default_snap_unit")]
    pub snap_unit_in: f64,
}

/// The Grid Snap Unit a layout starts with: 1/16 in.
pub const DEFAULT_SNAP_UNIT_IN: f64 = 1.0 / 16.0;
/// Smallest and largest Grid Snap Unit General Layout Defaults accepts.
pub const MIN_SNAP_UNIT_IN: f64 = 1.0 / 64.0;
pub const MAX_SNAP_UNIT_IN: f64 = 12.0;

fn default_snap_unit() -> f64 {
    DEFAULT_SNAP_UNIT_IN
}

/// Smallest and largest sheet side Customize Sheet Sizes accepts, inches.
pub const MIN_SHEET_SIDE_IN: f64 = 2.0;
pub const MAX_SHEET_SIDE_IN: f64 = 200.0;

/// A sheet size added in Customize Sheet Sizes: a name and the sheet's two
/// sides in inches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomSheetSize {
    pub name: String,
    pub width_in: f64,
    pub height_in: f64,
}

impl CustomSheetSize {
    pub fn new(name: impl Into<String>, width_in: f64, height_in: f64) -> Self {
        Self {
            name: name.into(),
            width_in,
            height_in,
        }
    }

    /// `(width, height)` landscape, the long side wide (like
    /// [`SheetSize::inches`]).
    pub fn inches(&self) -> (f64, f64) {
        (
            self.width_in.max(self.height_in),
            self.width_in.min(self.height_in),
        )
    }

    /// Name for the lists, sides short by long: `Poster (30 x 40)`.
    pub fn label(&self) -> String {
        let (long, short) = self.inches();
        format!(
            "{} ({} x {})",
            self.name.trim(),
            side_text(short),
            side_text(long)
        )
    }

    /// Why the size cannot be kept, if it cannot.
    pub fn problem(&self) -> Option<&'static str> {
        let ok = |v: f64| v.is_finite() && (MIN_SHEET_SIDE_IN..=MAX_SHEET_SIDE_IN).contains(&v);
        if self.name.trim().is_empty() {
            Some("Give the size a name")
        } else if !ok(self.width_in) || !ok(self.height_in) {
            Some("Each side must be between 2 and 200 inches")
        } else {
            None
        }
    }
}

fn side_text(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}").trim_end_matches('0').to_string()
    }
}

/// A sheet size the lists offer: a standard one or one of the layout's own.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetChoice {
    Standard(SheetSize),
    Custom(CustomSheetSize),
}

impl SheetChoice {
    pub fn label(&self) -> String {
        match self {
            SheetChoice::Standard(s) => s.label().to_string(),
            SheetChoice::Custom(c) => c.label(),
        }
    }

    /// Landscape `(width, height)`, inches.
    pub fn inches(&self) -> (f64, f64) {
        match self {
            SheetChoice::Standard(s) => s.inches(),
            SheetChoice::Custom(c) => c.inches(),
        }
    }
}

/// The standard sizes Daniel works in, ARCH C (18 x 24) first: the preset of
/// Customize Sheet Sizes.
pub const DANIEL_SIZES: [SheetSize; 6] = [
    SheetSize::ArchC,
    SheetSize::ArchD,
    SheetSize::ArchB,
    SheetSize::ArchA,
    SheetSize::ArchE,
    SheetSize::ArchE1,
];

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
            portrait: false,
            layers: LayoutLayers::default(),
            custom_sizes: Vec::new(),
            hidden_sizes: Vec::new(),
            custom_sheet: None,
            snap_grid: true,
            snap_unit_in: DEFAULT_SNAP_UNIT_IN,
        }
    }

    /// Width and height of the sheet in paper inches, turned upright when
    /// the layout is portrait.
    pub fn sheet_inches(&self) -> (f64, f64) {
        let (w, h) = match &self.custom_sheet {
            Some(c) => c.inches(),
            None => self.sheet.inches(),
        };
        if self.portrait {
            (w.min(h), w.max(h))
        } else {
            (w, h)
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
            leaders: Vec::new(),
            clouds: Vec::new(),
            size_override_in: None,
            no_title_block: false,
            label: String::new(),
            description: String::new(),
            comments: String::new(),
            in_layout_table: true,
            template: None,
            revisions: Vec::new(),
            baked: None,
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

    /// Width of the right title strip on a sheet `sheet_w_in` wide.
    pub(crate) fn right_strip_in_for(&self, sheet_w_in: f64) -> f64 {
        RIGHT_STRIP_IN.min(sheet_w_in * 0.22)
    }

    /// The area boxes are packed into (inside the border, beside the title
    /// block): `(lower-left, upper-right)` in paper inches.
    pub fn drawing_area(&self) -> (Point, Point) {
        self.drawing_area_for(self.sheet_inches())
    }

    /// [`drawing_area`](Self::drawing_area) of a sheet of `size` inches.
    pub fn drawing_area_for(&self, (w, h): (f64, f64)) -> (Point, Point) {
        let m = self.margins_in;
        let (mut right, mut bottom) = (w - m, m);
        match self.title_block.style {
            TitleBlockStyle::RightStrip => right -= self.right_strip_in_for(w),
            TitleBlockStyle::BottomStrip => bottom += BOTTOM_STRIP_IN,
            TitleBlockStyle::Custom(_) => {}
        }
        (Point::new(m, bottom), Point::new(right, h - m))
    }

    /// The sheet `page` prints on, `(width, height)` in paper inches: its
    /// own size from Page Specification, else the layout's.
    pub fn page_sheet_inches(&self, page: &LayoutPage) -> (f64, f64) {
        page.size_override_in.unwrap_or_else(|| self.sheet_inches())
    }

    /// The drawing area of `page` (its own sheet size counts).
    pub fn page_drawing_area(&self, page: &LayoutPage) -> (Point, Point) {
        self.drawing_area_for(self.page_sheet_inches(page))
    }

    /// Customize Sheet Sizes: the sizes the lists offer, standard ones that
    /// are not hidden (the layout's own sheet always stays), then the custom
    /// ones.
    pub fn size_choices(&self) -> Vec<SheetChoice> {
        let mut out: Vec<SheetChoice> = SheetSize::ALL
            .into_iter()
            .filter(|s| {
                !self.hidden_sizes.contains(s) || (self.custom_sheet.is_none() && *s == self.sheet)
            })
            .map(SheetChoice::Standard)
            .collect();
        out.extend(self.custom_sizes.iter().cloned().map(SheetChoice::Custom));
        out
    }

    /// The layout's sheet as a list choice.
    pub fn sheet_choice(&self) -> SheetChoice {
        match &self.custom_sheet {
            Some(c) => SheetChoice::Custom(c.clone()),
            None => SheetChoice::Standard(self.sheet),
        }
    }

    /// Makes `choice` the layout's sheet. A custom size is kept as the
    /// layout's own and the standard size is set to the smallest one that
    /// holds it (what the plan's Drawing Sheet shows).
    pub fn set_sheet_choice(&mut self, choice: &SheetChoice) {
        match choice {
            SheetChoice::Standard(s) => {
                self.sheet = *s;
                self.custom_sheet = None;
            }
            SheetChoice::Custom(c) => {
                let (w, h) = c.inches();
                if let Some(near) = SheetSize::ALL
                    .into_iter()
                    .filter(|s| {
                        let (sw, sh) = s.inches();
                        sw + 1e-6 >= w && sh + 1e-6 >= h
                    })
                    .min_by(|a, b| {
                        let area = |s: &SheetSize| s.inches().0 * s.inches().1;
                        area(a).total_cmp(&area(b))
                    })
                {
                    self.sheet = near;
                }
                self.custom_sheet = Some(c.clone());
            }
        }
    }

    /// Gives page `index` the sheet of `choice` (turned upright when
    /// `portrait`), or the layout's own when `choice` is `None`.
    pub fn set_page_sheet(&mut self, index: usize, choice: Option<&SheetChoice>, portrait: bool) {
        let size = choice.map(|c| {
            let (w, h) = c.inches();
            if portrait {
                (h, w)
            } else {
                (w, h)
            }
        });
        if let Some(p) = self.pages.get_mut(index) {
            p.size_override_in = size;
        }
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
    fn image_pixels_are_stored_as_base64_and_older_lists_still_load() {
        let rgba: Vec<u8> = (0..=255u8).cycle().take(4 * 7 * 5 + 1).collect();
        let src = BoxSource::ImageData {
            width: 7,
            height: 5,
            rgba: rgba.clone(),
        };
        let json = serde_json::to_value(&src).unwrap();
        assert!(json["ImageData"]["rgba"].is_string(), "{json}");
        let back: BoxSource = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, src);
        // A file written before the text form held a list of numbers.
        let mut old = json;
        old["ImageData"]["rgba"] = serde_json::json!(rgba);
        let back: BoxSource = serde_json::from_value(old).unwrap();
        assert_eq!(back, src);
        // Padding lengths 0..3 round trip.
        for n in 0..8usize {
            let v: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37)).collect();
            assert_eq!(
                pixels_text::decode(&pixels_text::encode(&v)).unwrap(),
                v,
                "{n}"
            );
        }
        assert_eq!(pixels_text::encode(b"Man"), "TWFu");
        assert_eq!(pixels_text::encode(b"Ma"), "TWE=");
    }

    #[test]
    fn drawing_area_leaves_room_for_the_strip() {
        let l = Layout::new("t", SheetSize::ArchC);
        let (lo, hi) = l.drawing_area();
        assert_eq!((lo.x, lo.y), (0.5, 0.5));
        assert_eq!((hi.x, hi.y), (21.0, 17.5));
    }
}
