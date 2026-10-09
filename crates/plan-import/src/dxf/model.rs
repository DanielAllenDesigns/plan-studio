//! What the DXF reader produces: layers, tables, blocks and entities in the
//! drawing's own units (angles in degrees unless a field says radians).

use plan_core::Point;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Drawing units from `$INSUNITS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DxfUnits {
    /// No units declared (`0`, absent, or a unit this crate does not know).
    #[default]
    Unitless,
    Inches,
    Feet,
    Millimeters,
    Centimeters,
    Meters,
    Yards,
    Miles,
    Kilometers,
    Decimeters,
}

impl DxfUnits {
    /// Map an `$INSUNITS` code; unknown codes are treated as unitless.
    pub fn from_code(code: i32) -> DxfUnits {
        match code {
            1 => DxfUnits::Inches,
            2 | 21 => DxfUnits::Feet,
            3 => DxfUnits::Miles,
            4 => DxfUnits::Millimeters,
            5 => DxfUnits::Centimeters,
            6 => DxfUnits::Meters,
            7 => DxfUnits::Kilometers,
            10 => DxfUnits::Yards,
            14 => DxfUnits::Decimeters,
            _ => DxfUnits::Unitless,
        }
    }

    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            DxfUnits::Unitless => "Unitless",
            DxfUnits::Inches => "Inches",
            DxfUnits::Feet => "Feet",
            DxfUnits::Millimeters => "Millimeters",
            DxfUnits::Centimeters => "Centimeters",
            DxfUnits::Meters => "Meters",
            DxfUnits::Yards => "Yards",
            DxfUnits::Miles => "Miles",
            DxfUnits::Kilometers => "Kilometers",
            DxfUnits::Decimeters => "Decimeters",
        }
    }

    /// The units the import window offers.
    pub const CHOICES: [DxfUnits; 9] = [
        DxfUnits::Inches,
        DxfUnits::Feet,
        DxfUnits::Millimeters,
        DxfUnits::Centimeters,
        DxfUnits::Meters,
        DxfUnits::Decimeters,
        DxfUnits::Yards,
        DxfUnits::Miles,
        DxfUnits::Kilometers,
    ];
}

/// How an entity or layer is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DxfColor {
    #[default]
    ByLayer,
    ByBlock,
    /// AutoCAD Color Index 1..=255.
    Aci(u8),
    Rgb([u8; 3]),
}

impl DxfColor {
    /// The colour as RGB; `None` for BYLAYER and BYBLOCK.
    pub fn rgb(self) -> Option<[u8; 3]> {
        match self {
            DxfColor::Aci(i) => crate::dxf::aci::aci_rgb(i32::from(i)),
            DxfColor::Rgb(c) => Some(c),
            DxfColor::ByLayer | DxfColor::ByBlock => None,
        }
    }
}

/// Lineweight code value of "by layer" (`-1`), "by block" (`-2`) and the
/// default weight (`-3`), as group 370 stores them.
pub const WEIGHT_BY_LAYER: i32 = -1;
pub const WEIGHT_BY_BLOCK: i32 = -2;
pub const WEIGHT_DEFAULT: i32 = -3;

/// A layer from the `LAYER` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfLayer {
    pub name: String,
    pub color: DxfColor,
    /// `false` when the layer is off (negative colour) or frozen.
    pub visible: bool,
    pub frozen: bool,
    pub locked: bool,
    /// Line type name (`CONTINUOUS` when the file names none).
    pub linetype: String,
    /// Group 370: hundredths of a millimetre, or one of the negative codes.
    pub weight: i32,
    /// `false` for a non-plotting layer (group 290 = 0).
    pub plot: bool,
}

impl DxfLayer {
    /// The layer's AutoCAD index for display (7 when it has none).
    pub fn aci(&self) -> i32 {
        match self.color {
            DxfColor::Aci(i) => i32::from(i),
            _ => 7,
        }
    }

    /// The layer's colour as RGB (black for white/index 7).
    pub fn rgb(&self) -> [u8; 3] {
        self.color.rgb().unwrap_or([0, 0, 0])
    }

    /// Plotted weight in hundredths of a millimetre (18 for the default).
    pub fn weight_hundredths(&self) -> u32 {
        if self.weight >= 0 {
            self.weight as u32
        } else {
            18
        }
    }
}

/// A line type from the `LTYPE` table.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DxfLinetype {
    pub name: String,
    pub description: String,
    /// Dash elements: positive draws, negative skips, zero is a dot.
    pub pattern: Vec<f64>,
}

/// A text style from the `STYLE` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfTextStyle {
    pub name: String,
    pub font: String,
    pub width_factor: f64,
    pub oblique_deg: f64,
    /// Fixed height; zero lets each text choose its own.
    pub height: f64,
}

/// The dimension style values the importer uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfDimStyle {
    pub name: String,
    /// `$DIMSCALE`: multiplies the sizes below.
    pub scale: f64,
    /// Arrow size (`$DIMASZ`).
    pub arrow: f64,
    /// Extension line offset from the origin point (`$DIMEXO`).
    pub ext_offset: f64,
    /// Extension line extension past the dimension line (`$DIMEXE`).
    pub ext_extend: f64,
    /// Text height (`$DIMTXT`).
    pub text_height: f64,
    /// Gap around the text (`$DIMGAP`).
    pub gap: f64,
    /// Linear measurement scale factor (`$DIMLFAC`).
    pub linear_factor: f64,
}

impl Default for DxfDimStyle {
    fn default() -> Self {
        Self {
            name: "STANDARD".into(),
            scale: 1.0,
            arrow: 0.18,
            ext_offset: 0.0625,
            ext_extend: 0.18,
            text_height: 0.18,
            gap: 0.09,
            linear_factor: 1.0,
        }
    }
}

impl DxfDimStyle {
    /// A size of this style as drawn (style size times `scale`).
    pub fn sized(&self, v: f64) -> f64 {
        v * if self.scale > 0.0 { self.scale } else { 1.0 }
    }
}

/// The properties every entity carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfProps {
    pub layer: String,
    pub color: DxfColor,
    /// Group 370 (see [`WEIGHT_BY_LAYER`]).
    pub weight: i32,
    /// Line type name; empty is BYLAYER.
    pub linetype: String,
    /// Drawn on a paper space page.
    pub paper: bool,
}

impl Default for DxfProps {
    fn default() -> Self {
        Self {
            layer: "0".into(),
            color: DxfColor::ByLayer,
            weight: WEIGHT_BY_LAYER,
            linetype: String::new(),
            paper: false,
        }
    }
}

/// Horizontal justification of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HJust {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical justification of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VJust {
    #[default]
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// A stretch of MTEXT in one format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Multiplies the text height.
    pub scale: f64,
    pub color: Option<[u8; 3]>,
    /// The font family of the first `\f` code; it applies to the whole text.
    pub font: Option<String>,
}

impl Default for DxfRun {
    fn default() -> Self {
        Self {
            text: String::new(),
            bold: false,
            italic: false,
            underline: false,
            scale: 1.0,
            color: None,
            font: None,
        }
    }
}

impl DxfRun {
    /// Is this run plain text of the base size?
    pub fn is_plain(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.underline
            && (self.scale - 1.0).abs() < 1e-9
            && self.color.is_none()
    }
}

/// TEXT, MTEXT, ATTRIB and ATTDEF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfText {
    /// The justification point (the insertion point of left-aligned text).
    pub pos: Point,
    pub h: HJust,
    pub v: VJust,
    /// Plain text with `\n` between lines.
    pub text: String,
    /// Formatted runs of an MTEXT; empty when the text is plain.
    pub runs: Vec<DxfRun>,
    pub height: f64,
    pub angle_deg: f64,
    pub width_factor: f64,
    pub oblique_deg: f64,
    pub style: String,
    /// MTEXT reference rectangle width (zero: no wrapping).
    pub wrap_width: f64,
    pub mtext: bool,
    /// The attribute tag of an ATTRIB or ATTDEF.
    pub tag: String,
    pub attdef: bool,
    /// An invisible attribute.
    pub invisible: bool,
}

/// One DIMENSION entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfDimension {
    /// Type without the flag bits: 0 linear/rotated, 1 aligned, 2 angular,
    /// 3 diameter, 4 radius, 5 angular 3-point, 6 ordinate.
    pub dtype: i32,
    /// The anonymous block that draws it (`*D1`), if any.
    pub block: String,
    /// Group 10: a point on the dimension line (the vertex of 3-point
    /// angular and the far end of a diameter).
    pub def_pt: Point,
    /// Group 11: the middle of the text.
    pub text_pt: Point,
    pub p13: Point,
    pub p14: Point,
    pub p15: Point,
    pub p16: Point,
    /// Group 50: direction of the dimension line of a rotated dimension.
    pub angle_deg: f64,
    /// Group 1: the text ("" or "<>" is the measurement).
    pub text: String,
    pub measurement: Option<f64>,
    pub style: String,
}

/// A MULTILEADER.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfMLeader {
    /// Each leader line from the arrow tip to the landing.
    pub lines: Vec<Vec<Point>>,
    pub text: String,
    pub text_pos: Option<Point>,
    pub height: f64,
    pub angle_deg: f64,
    pub arrow: bool,
}

/// One hatch boundary loop (arcs and splines already sampled).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HatchLoop {
    pub points: Vec<Point>,
    pub external: bool,
}

/// A HATCH.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfHatch {
    pub loops: Vec<HatchLoop>,
    pub pattern: String,
    pub solid: bool,
    pub angle_deg: f64,
    pub scale: f64,
    /// Distance between the lines of the pattern (drawing units, scale
    /// applied), when the pattern definition gives it.
    pub spacing: Option<f64>,
}

/// An INSERT (or MINSERT array) with its attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfInsert {
    pub block: String,
    pub pos: Point,
    pub scale: (f64, f64),
    pub rotation_deg: f64,
    /// ATTRIB texts, in the coordinates of the INSERT.
    pub attribs: Vec<DxfEntity>,
    pub columns: u32,
    pub rows: u32,
    pub col_spacing: f64,
    pub row_spacing: f64,
}

/// The geometry of one entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DxfKind {
    Line {
        a: Point,
        b: Point,
    },
    /// LWPOLYLINE or POLYLINE. `bulges[i]` belongs to the segment from
    /// vertex `i` to the next one; same length as `points`.
    Polyline {
        points: Vec<Point>,
        closed: bool,
        bulges: Vec<f64>,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    /// Counter-clockwise arc from `start_deg` to `end_deg`.
    Arc {
        center: Point,
        radius: f64,
        start_deg: f64,
        end_deg: f64,
    },
    /// `center + u cos t + v sin t` for `t` from `t0` to `t1` (radians).
    Ellipse {
        center: Point,
        u: Point,
        v: Point,
        t0: f64,
        t1: f64,
    },
    Spline {
        degree: usize,
        closed: bool,
        knots: Vec<f64>,
        weights: Vec<f64>,
        control: Vec<Point>,
        fit: Vec<Point>,
    },
    Hatch(Box<DxfHatch>),
    /// SOLID/TRACE (`filled`) or 3DFACE: the corners in outline order.
    Face {
        points: Vec<Point>,
        filled: bool,
    },
    /// A POINT.
    Marker {
        pos: Point,
    },
    Text(Box<DxfText>),
    Dimension(Box<DxfDimension>),
    /// LEADER: the arrow is at the first point.
    Leader {
        points: Vec<Point>,
        arrow: bool,
    },
    MLeader(Box<DxfMLeader>),
    Insert(Box<DxfInsert>),
}

/// One drawing entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfEntity {
    pub kind: DxfKind,
    pub props: DxfProps,
}

impl DxfEntity {
    pub fn new(kind: DxfKind, layer: &str) -> Self {
        Self {
            kind,
            props: DxfProps {
                layer: layer.to_string(),
                ..DxfProps::default()
            },
        }
    }

    pub fn layer(&self) -> &str {
        &self.props.layer
    }
}

/// A block definition from the `BLOCKS` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfBlock {
    pub name: String,
    /// Block base point; INSERT positions refer to it.
    pub base: Point,
    pub entities: Vec<DxfEntity>,
    /// An anonymous block (dimensions, groups: `*D1`, `*U2`).
    pub anonymous: bool,
    /// Path of an external reference; empty for ordinary blocks.
    pub xref: String,
}

/// File format a drawing was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DxfFormat {
    #[default]
    Ascii,
    Binary,
}

/// A parsed DXF file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DxfDrawing {
    pub format: DxfFormat,
    /// `$ACADVER` (`AC1009` ... `AC1032`), empty when absent.
    pub version: String,
    pub layers: Vec<DxfLayer>,
    pub linetypes: Vec<DxfLinetype>,
    pub text_styles: Vec<DxfTextStyle>,
    pub dim_styles: Vec<DxfDimStyle>,
    /// Model-space entities, with INSERTs still unexpanded.
    pub entities: Vec<DxfEntity>,
    /// Entities of the first paper space page (not imported unless asked).
    pub paper_entities: Vec<DxfEntity>,
    pub units: DxfUnits,
    /// `$MEASUREMENT` is metric (1).
    pub metric: bool,
    /// `$DIMSCALE` of the header (zero when absent).
    pub dim_scale: f64,
    /// The header's current dimension variables (`$DIMSCALE`, `$DIMTXT`, ...).
    pub header_dim: DxfDimStyle,
    /// `$LTSCALE`.
    pub ltscale: f64,
    /// `($EXTMIN, $EXTMAX)` when the header carries sane values.
    pub extents: Option<(Point, Point)>,
    /// Block definitions keyed by upper-cased name.
    pub blocks: BTreeMap<String, DxfBlock>,
    /// External reference paths (blocks that point at another file); they are
    /// not imported.
    pub xrefs: Vec<String>,
    /// Entity types that were not imported, with how many of each.
    pub skipped: Vec<(String, usize)>,
}

impl DxfDrawing {
    /// Look up a block by name (DXF block names are case-insensitive).
    pub fn block(&self, name: &str) -> Option<&DxfBlock> {
        self.blocks.get(&name.to_uppercase())
    }

    /// A layer of the table by name.
    pub fn layer(&self, name: &str) -> Option<&DxfLayer> {
        self.layers.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    pub fn linetype(&self, name: &str) -> Option<&DxfLinetype> {
        self.linetypes.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    pub fn text_style(&self, name: &str) -> Option<&DxfTextStyle> {
        self.text_styles.iter().find(|l| l.name.eq_ignore_ascii_case(name))
    }

    /// The dimension style `name`, else the header's current values.
    pub fn dim_style(&self, name: &str) -> DxfDimStyle {
        self.dim_styles
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .cloned()
            .unwrap_or_else(|| self.header_dim.clone())
    }

    /// Every entity of the model space and all block definitions, for
    /// counting by layer.
    pub fn all_entities(&self) -> impl Iterator<Item = &DxfEntity> {
        self.entities
            .iter()
            .chain(self.blocks.values().flat_map(|b| b.entities.iter()))
    }
}
