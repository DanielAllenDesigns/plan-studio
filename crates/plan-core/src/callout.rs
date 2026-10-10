//! Callouts, markers and notes (TXT-29..56, `docs/parity/dimensions-text-cad.md`;
//! reference manual pp. 548 to 563).
//!
//! # Model
//!
//! A callout, marker or note is a *record* (`Floor::annots`) that holds the
//! specification (shape, size, label, cross section line, arrows, link, ...)
//! and a list of plain CAD objects (`Floor::cad`) generated from it, grouped
//! so they select, move, copy and delete together and draw everywhere a CAD
//! object draws (plan, layout boxes, PDF, DXF). The record is the truth: the
//! sync pass ([`Project::sync_annotations`], run by the editor's refresh)
//! keeps the generated objects equal to what the record describes, and takes
//! a move or rotation of the group back into the record first
//! ([`pose_shift`]), so dragging a callout never loses its specification.
//!
//! # Generated, derived text
//!
//! Labels may hold the annotation macros `%linked_view_name%`,
//! `%linked_view_layout_page_label%`, `%referenced_view_callout_label%`,
//! `%layout_page_label%`, `%automatic_label%`, `%simple_schedule_number%` and
//! `%height%` ([`expand_macros`]). They are expanded on every sync from the
//! linked view ([`Project::resolve_view_link`]) and the notes' numbering
//! ([`Project::note_numbering`]), so a callout linked to a camera shows the
//! camera's callout label and the layout sheet it was sent to, and the notes
//! renumber in draw order when one is deleted.

use crate::cad::{ArrowStyle, CadAttrs, CadItem, FillAttr, TEXT_WIDTH_FACTOR};
use crate::geometry::Point;
use crate::groups::ObjectRef;
use crate::layers::LineStyle;
use crate::model::{Floor, Id, Project};
use crate::text_box::{HAlign, TextBox};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

/// The layer annotations go on unless a tool says otherwise.
pub const ANNOT_LAYER: &str = "Text";
/// Default text height of a callout, marker or note label, inches.
pub const DEFAULT_ANNOT_TEXT: f64 = 6.0;
/// Name of the note type new notes start with.
pub const DEFAULT_NOTE_TYPE: &str = "General Note";
/// Name of the Saved Default every plan starts with.
pub const DEFAULT_SAVED_NAME: &str = "Default";

// ===== value types =====

/// The outline of a callout or note (the Shape radio buttons of the Callout
/// panel). `None` produces the label alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, Serialize, Deserialize)]
pub enum CalloutShape {
    None,
    #[default]
    Circle,
    Oval,
    Square,
    Rectangle,
    RoundedRectangle,
    Hexagon,
    Octagon,
    Triangle,
    HalfCircle,
    Diamond,
}

impl CalloutShape {
    /// The ten shapes (without `None`).
    pub const TEN: [CalloutShape; 10] = [
        CalloutShape::Circle,
        CalloutShape::Oval,
        CalloutShape::Square,
        CalloutShape::Rectangle,
        CalloutShape::RoundedRectangle,
        CalloutShape::Hexagon,
        CalloutShape::Octagon,
        CalloutShape::Triangle,
        CalloutShape::HalfCircle,
        CalloutShape::Diamond,
    ];

    /// None and the ten shapes, in the order of the radio buttons.
    pub const ALL: [CalloutShape; 11] = [
        CalloutShape::None,
        CalloutShape::Circle,
        CalloutShape::Oval,
        CalloutShape::Square,
        CalloutShape::Rectangle,
        CalloutShape::RoundedRectangle,
        CalloutShape::Hexagon,
        CalloutShape::Octagon,
        CalloutShape::Triangle,
        CalloutShape::HalfCircle,
        CalloutShape::Diamond,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CalloutShape::None => "None",
            CalloutShape::Circle => "Circle",
            CalloutShape::Oval => "Oval",
            CalloutShape::Square => "Square",
            CalloutShape::Rectangle => "Rectangle",
            CalloutShape::RoundedRectangle => "Rounded Rectangle",
            CalloutShape::Hexagon => "Hexagon",
            CalloutShape::Octagon => "Octagon",
            CalloutShape::Triangle => "Triangle",
            CalloutShape::HalfCircle => "Half Circle",
            CalloutShape::Diamond => "Diamond",
        }
    }

    pub fn from_label(s: &str) -> Option<CalloutShape> {
        CalloutShape::ALL
            .into_iter()
            .find(|k| k.label().eq_ignore_ascii_case(s.trim()))
    }

    /// The outline in the shape's own frame (centered on the origin, the
    /// reading direction along +X) for the half extents `a` (along X) and `b`
    /// (along Y). Empty for `None`.
    pub fn outline(self, a: f64, b: f64) -> Vec<Point> {
        let ellipse = |n: usize, a: f64, b: f64, phase: f64| -> Vec<Point> {
            (0..n)
                .map(|i| {
                    let t = phase + TAU * i as f64 / n as f64;
                    Point::new(a * t.cos(), b * t.sin())
                })
                .collect()
        };
        match self {
            CalloutShape::None => Vec::new(),
            CalloutShape::Circle | CalloutShape::Oval => ellipse(64, a, b, 0.0),
            CalloutShape::Square | CalloutShape::Rectangle => vec![
                Point::new(-a, -b),
                Point::new(a, -b),
                Point::new(a, b),
                Point::new(-a, b),
            ],
            CalloutShape::RoundedRectangle => {
                let r = (a.min(b) * 0.4).max(0.1);
                let mut pts = Vec::new();
                for (cx, cy, from) in [
                    (a - r, b - r, 0.0),
                    (-(a - r), b - r, FRAC_PI_2),
                    (-(a - r), -(b - r), PI),
                    (a - r, -(b - r), PI + FRAC_PI_2),
                ] {
                    for k in 0..=6 {
                        let t = from + FRAC_PI_2 * f64::from(k) / 6.0;
                        pts.push(Point::new(cx + r * t.cos(), cy + r * t.sin()));
                    }
                }
                pts
            }
            CalloutShape::Hexagon => ellipse(6, a, b, 0.0),
            CalloutShape::Octagon => ellipse(8, a, b, PI / 8.0),
            CalloutShape::Triangle => ellipse(3, a, b, FRAC_PI_2),
            CalloutShape::HalfCircle => {
                // The flat side down; the box is centered on the middle of
                // the half's bounding box.
                let mut pts: Vec<Point> = (0..=24)
                    .map(|i| {
                        let t = PI * f64::from(i) / 24.0;
                        Point::new(a * t.cos(), b * 2.0 * t.sin() - b)
                    })
                    .collect();
                pts.push(Point::new(-a, -b));
                pts
            }
            CalloutShape::Diamond => vec![
                Point::new(a, 0.0),
                Point::new(0.0, b),
                Point::new(-a, 0.0),
                Point::new(0.0, -b),
            ],
        }
    }

    /// Half extents `(a, b)` that hold a label `tw` wide and `th` tall with
    /// `pad` around it, at least `min`.
    pub fn auto_half_extents(self, tw: f64, th: f64, pad: f64, min: f64) -> (f64, f64) {
        let hx = tw * 0.5 + pad;
        let hy = th * 0.5 + pad;
        let r = hx.hypot(hy);
        let (a, b) = match self {
            CalloutShape::None => (hx, hy),
            CalloutShape::Circle => (r, r),
            CalloutShape::Oval => (hx * 1.42, hy * 1.42),
            CalloutShape::Square => (hx.max(hy), hx.max(hy)),
            CalloutShape::Rectangle | CalloutShape::RoundedRectangle => (hx, hy),
            CalloutShape::Hexagon => (r * 1.1, r * 1.1),
            CalloutShape::Octagon => (r * 1.08, r * 1.08),
            CalloutShape::Triangle => (r * 1.75, r * 1.75),
            CalloutShape::HalfCircle => (r * 1.2, r * 0.85),
            CalloutShape::Diamond => (hx * 2.0, hy * 2.0),
        };
        (a.max(min), b.max(min))
    }

    /// Half extents of the fixed `size` (the radius or half width).
    pub fn sized_half_extents(self, size: f64) -> (f64, f64) {
        match self {
            CalloutShape::Oval => (size * 1.3, size),
            CalloutShape::Rectangle | CalloutShape::RoundedRectangle => (size * 1.3, size * 0.85),
            CalloutShape::HalfCircle => (size, size * 0.6),
            _ => (size, size),
        }
    }
}

/// Where text sits along a marker's or a callout's line.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum LineAlign {
    /// Next to the callout or marker shape.
    #[default]
    Toward,
    Centered,
    /// At the far end of the line.
    Away,
    /// Text Below Line only: as the text above.
    Match,
}

impl LineAlign {
    pub fn label(self, noun: &str) -> String {
        match self {
            LineAlign::Toward => format!("Toward {noun}"),
            LineAlign::Centered => "Centered".to_string(),
            LineAlign::Away => format!("Away From {noun}"),
            LineAlign::Match => "Match Text Above".to_string(),
        }
    }
}

/// Size of a callout arrow ("hat").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ArrowSize {
    #[default]
    Small,
    Large,
}

/// The Line Style panel: `None` follows the layer.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LineLook {
    pub style: Option<LineStyle>,
    /// Hundredths of a millimetre.
    pub weight: Option<u32>,
    pub color: Option<[u8; 3]>,
}

impl LineLook {
    fn attrs(&self) -> CadAttrs {
        CadAttrs {
            color: self.color,
            weight: self.weight,
            dash: self.style.filter(|s| *s != LineStyle::Solid),
            ..CadAttrs::default()
        }
    }
}

/// Text above or below a cross section or marker line.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LineText {
    pub text: String,
    /// A text style name; `None` matches the callout's.
    pub style: Option<String>,
    pub align: LineAlign,
}

/// The cross section line of a callout (Attributes panel) with its
/// Section Arrow.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionLine {
    pub on: bool,
    pub double: bool,
    pub look: LineLook,
    pub min_length: f64,
    /// Degrees, relative to the callout's shape angle.
    pub rel_angle: f64,
    pub auto_adjust_text: bool,
    pub above: LineText,
    pub below: LineText,
    /// Section Arrow panel: Include Arrow.
    pub arrow: bool,
    pub arrow_style: ArrowStyle,
    /// Length of the arrow's shaft, inches.
    pub arrow_length: f64,
}

impl Default for SectionLine {
    fn default() -> Self {
        Self {
            on: false,
            double: false,
            look: LineLook::default(),
            min_length: 24.0,
            rel_angle: 0.0,
            auto_adjust_text: true,
            above: LineText::default(),
            below: LineText {
                align: LineAlign::Match,
                ..LineText::default()
            },
            arrow: false,
            arrow_style: ArrowStyle::Filled,
            arrow_length: 10.0,
        }
    }
}

/// The arrows ("hats") around a callout's perimeter.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CalloutArrows {
    /// One angle per arrow, degrees relative to the shape angle.
    pub angles: Vec<f64>,
    pub size: ArrowSize,
    pub filled: bool,
    /// `None` follows the layer.
    pub color: Option<[u8; 3]>,
    /// Percent, 0 opaque.
    pub transparency: u8,
}

impl Default for CalloutArrows {
    fn default() -> Self {
        Self {
            angles: Vec::new(),
            size: ArrowSize::Small,
            filled: true,
            color: None,
            transparency: 0,
        }
    }
}

impl CalloutArrows {
    /// Sets the number of arrows: new ones start 90 degrees round from the
    /// last, removed ones come off the end.
    pub fn set_count(&mut self, n: usize) {
        while self.angles.len() > n {
            self.angles.pop();
        }
        while self.angles.len() < n {
            let next = self.angles.last().map_or(90.0, |a| (a + 90.0) % 360.0);
            self.angles.push(next);
        }
    }
}

/// What a callout is linked to (the Link panel).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ViewLink {
    pub kind: ViewKind,
    /// A camera's id; the number of a layout page; 0 for a CAD detail.
    pub id: Id,
    /// A CAD detail's name; a copy of the name the link was made with
    /// otherwise.
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ViewKind {
    /// A camera view: a cross section, elevation or other saved camera.
    Camera,
    CadDetail,
    LayoutPage,
}

impl ViewKind {
    pub fn label(self) -> &'static str {
        match self {
            ViewKind::Camera => "Camera View",
            ViewKind::CadDetail => "CAD Detail",
            ViewKind::LayoutPage => "Layout Page",
        }
    }
}

/// What the Link panel reports about the linked view.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct LinkInfo {
    /// The view (or page) still exists.
    pub valid: bool,
    pub view_name: String,
    pub view_type: String,
    /// The callout label of the view (a camera's callout number, a detail's
    /// name).
    pub callout_label: String,
    /// Label of the layout page the view was sent to ("A-3"); empty when it
    /// was not sent.
    pub page_label: String,
    pub page_number: Option<u32>,
    /// The layout file's name.
    pub file_name: String,
}

// ===== records =====

/// A callout: a shape holding a label, with an optional cross section line,
/// arrows, leaders and a link to a view (Callout Specification).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Callout {
    /// The CAD objects generated from this record, in generation order.
    pub items: Vec<Id>,
    /// The item whose pose [`pose_shift`] watches and the pose it had
    /// when last written: `[x, y, angle]`.
    pub pose_idx: usize,
    pub pose: Option<[f64; 3]>,
    pub layer: String,
    pub center: Point,
    /// Callout Label (inside the shape).
    pub label: String,
    /// Text Below Line (a second row inside the shape).
    pub text_below: String,
    /// Fill Text Below Line with the linked view's layout page label.
    pub auto_below: bool,
    pub shape: CalloutShape,
    pub filled: bool,
    /// `None` follows the layer.
    pub fill_color: Option<[u8; 3]>,
    /// Percent, 0 opaque.
    pub transparency: u8,
    pub auto_size: bool,
    /// Radius (half width) of a fixed-size shape, inches.
    pub size: f64,
    /// Degrees.
    pub shape_angle: f64,
    /// Degrees; `None` is Automatic (the shape angle).
    pub text_angle: Option<f64>,
    pub auto_adjust_text: bool,
    /// Text height of the label, plan inches.
    pub height: f64,
    /// A text style name; `None` is the layer's.
    pub text_style: Option<String>,
    pub line: LineLook,
    pub section: SectionLine,
    pub arrows: CalloutArrows,
    pub link: Option<ViewLink>,
    /// Text Lines with Arrow attached to the callout: `[0]` is the arrow
    /// tip, the rest are bends toward the callout. The tail attaches to the
    /// outline on its own.
    pub leaders: Vec<Vec<Point>>,
    /// The Caution symbol of a broken link is hidden (Ignore Invalid Links).
    pub ignore_link: bool,
}

impl Default for Callout {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            pose_idx: 0,
            pose: None,
            layer: ANNOT_LAYER.to_string(),
            center: Point::ZERO,
            label: "1".to_string(),
            text_below: String::new(),
            auto_below: false,
            shape: CalloutShape::Circle,
            filled: false,
            fill_color: None,
            transparency: 0,
            auto_size: true,
            size: 9.0,
            shape_angle: 0.0,
            text_angle: None,
            auto_adjust_text: true,
            height: DEFAULT_ANNOT_TEXT,
            text_style: None,
            line: LineLook::default(),
            section: SectionLine::default(),
            arrows: CalloutArrows::default(),
            link: None,
            leaders: Vec::new(),
            ignore_link: false,
        }
    }
}

/// The kinds of marker the Marker tool places.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MarkerKind {
    #[default]
    LevelLine,
    TestBoring,
    Point,
    /// The marker on the extension line of an elevation (Story Pole)
    /// dimension.
    Elevation,
}

impl MarkerKind {
    pub const ALL: [MarkerKind; 4] = [
        MarkerKind::LevelLine,
        MarkerKind::TestBoring,
        MarkerKind::Point,
        MarkerKind::Elevation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MarkerKind::LevelLine => "Level Line",
            MarkerKind::TestBoring => "Test Boring",
            MarkerKind::Point => "Point",
            MarkerKind::Elevation => "Elevation",
        }
    }

    /// Has a line (and text below it) rather than only a gap to the label.
    pub fn has_line(self) -> bool {
        matches!(self, MarkerKind::LevelLine | MarkerKind::Elevation)
    }
}

/// A marker (Marker Specification): Level Line, Test Boring, Point or an
/// elevation marker.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Marker {
    pub items: Vec<Id>,
    pub pose_idx: usize,
    pub pose: Option<[f64; 3]>,
    pub layer: String,
    pub center: Point,
    pub kind: MarkerKind,
    /// Minimum length of the line (Level Line) or the gap between marker and
    /// text, inches.
    pub min_length: f64,
    /// Degrees.
    pub angle: f64,
    pub auto_adjust_text: bool,
    pub label: LineText,
    /// Level Line only.
    pub below: LineText,
    /// Distance from the marker's center to the edge of its shape.
    pub radius: f64,
    /// Height relative to 0, shown by the `%height%` macro.
    pub height_z: f64,
    pub text_height: f64,
    pub text_style: Option<String>,
    pub line: LineLook,
}

impl Default for Marker {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            pose_idx: 0,
            pose: None,
            layer: ANNOT_LAYER.to_string(),
            center: Point::ZERO,
            kind: MarkerKind::LevelLine,
            min_length: 36.0,
            angle: 0.0,
            auto_adjust_text: true,
            label: LineText {
                text: "%height%".to_string(),
                ..LineText::default()
            },
            below: LineText {
                align: LineAlign::Match,
                ..LineText::default()
            },
            radius: 4.0,
            height_z: 0.0,
            text_height: DEFAULT_ANNOT_TEXT,
            text_style: None,
            line: LineLook::default(),
        }
    }
}

/// A note: a callout tied to a line of a Note Schedule (Note
/// Specification). The note's text lives in the schedule; the drawing shows
/// the label (the schedule number by default).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub items: Vec<Id>,
    pub pose_idx: usize,
    pub pose: Option<[f64; 3]>,
    pub layer: String,
    pub center: Point,
    /// Schedule text: what the Note Schedule lists.
    pub text: String,
    pub note_type: String,
    /// Text Above Line.
    pub label: String,
    pub text_below: String,
    pub generate_shape: bool,
    pub shape: CalloutShape,
    pub generate_fill: bool,
    pub filled: bool,
    pub fill_color: Option<[u8; 3]>,
    pub transparency: u8,
    pub generate_size: bool,
    pub auto_size: bool,
    pub size: f64,
    pub generate_angles: bool,
    pub shape_angle: f64,
    pub text_angle: Option<f64>,
    pub auto_adjust_text: bool,
    /// Z position, inches (the X and Y are the center).
    pub z: f64,
    pub auto_height: bool,
    pub height: f64,
    pub text_style: Option<String>,
    pub line: LineLook,
    /// Object Information panel: name and value pairs a schedule can use.
    pub info: Vec<(String, String)>,
    /// Schedule panel: listed in the schedules of its category.
    pub include_in_schedule: bool,
    pub category: Option<String>,
    /// Ignore Note With No Schedule: no Caution symbol.
    pub ignore_no_schedule: bool,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            pose_idx: 0,
            pose: None,
            layer: ANNOT_LAYER.to_string(),
            center: Point::ZERO,
            text: String::new(),
            note_type: DEFAULT_NOTE_TYPE.to_string(),
            label: "%simple_schedule_number%".to_string(),
            text_below: String::new(),
            generate_shape: false,
            shape: CalloutShape::Circle,
            generate_fill: false,
            filled: false,
            fill_color: None,
            transparency: 0,
            generate_size: false,
            auto_size: true,
            size: 9.0,
            generate_angles: false,
            shape_angle: 0.0,
            text_angle: None,
            auto_adjust_text: true,
            z: 0.0,
            auto_height: true,
            height: DEFAULT_ANNOT_TEXT,
            text_style: None,
            line: LineLook::default(),
            info: Vec::new(),
            include_in_schedule: true,
            category: None,
            ignore_no_schedule: false,
        }
    }
}

/// The three kinds of annotation record of a floor.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Annots {
    pub callouts: Vec<Callout>,
    pub markers: Vec<Marker>,
    pub notes: Vec<Note>,
}

impl Annots {
    pub fn is_empty(&self) -> bool {
        self.callouts.is_empty() && self.markers.is_empty() && self.notes.is_empty()
    }
}

/// Which record a CAD object belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnnotRef {
    Callout(usize),
    Marker(usize),
    Note(usize),
}

/// The Saved Defaults of the annotations (Default Settings > Text, Callouts
/// and Markers): what a new callout, marker or note starts as.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AnnotDefaults {
    /// Name of the Saved Default in the titles of the defaults dialogs.
    pub saved_name: String,
    pub callout: Callout,
    pub marker: Marker,
    pub note: Note,
    /// Text Defaults and Rich Text Defaults.
    pub text: TextSpec,
    pub rich: TextSpec,
}

impl Default for AnnotDefaults {
    fn default() -> Self {
        Self {
            saved_name: DEFAULT_SAVED_NAME.to_string(),
            callout: Callout::default(),
            marker: Marker::default(),
            note: Note::default(),
            text: TextSpec::default(),
            rich: TextSpec::default(),
        }
    }
}

/// What a new Text or Rich Text object starts with (Text Defaults, Rich Text
/// Defaults): its text style, height and box (alignment, wrap width, border,
/// background).
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSpec {
    /// A saved text style; `None` is the layer's.
    pub style: Option<String>,
    /// Plan inches; 0 follows the plan's Text height default.
    pub height: f64,
    pub text_box: TextBox,
}

impl Floor {
    /// The annotation record owning CAD object `id`.
    pub fn annot_of(&self, id: Id) -> Option<AnnotRef> {
        let a = &self.annots;
        if let Some(i) = a.callouts.iter().position(|c| c.items.contains(&id)) {
            return Some(AnnotRef::Callout(i));
        }
        if let Some(i) = a.markers.iter().position(|c| c.items.contains(&id)) {
            return Some(AnnotRef::Marker(i));
        }
        a.notes
            .iter()
            .position(|c| c.items.contains(&id))
            .map(AnnotRef::Note)
    }

    /// The CAD objects of the record `r`.
    pub fn annot_items(&self, r: AnnotRef) -> &[Id] {
        match r {
            AnnotRef::Callout(i) => &self.annots.callouts[i].items,
            AnnotRef::Marker(i) => &self.annots.markers[i].items,
            AnnotRef::Note(i) => &self.annots.notes[i].items,
        }
    }
}

// ===== macros =====

/// What the annotation macros expand to.
#[derive(Clone, Debug, Default)]
pub struct Vars {
    pub link: Option<LinkInfo>,
    /// The schedule number of a note (`%simple_schedule_number%`).
    pub number: Option<u32>,
    /// The note's schedule text (`%note_text%`).
    pub note_text: String,
    /// The marker's height (`%height%`).
    pub height: String,
}

/// `text` with the annotation macros replaced.
pub fn expand_macros(text: &str, v: &Vars) -> String {
    if !text.contains('%') {
        return text.to_string();
    }
    let l = v.link.as_ref();
    let valid = l.filter(|l| l.valid);
    let pairs: [(&str, String); 8] = [
        (
            "%linked_view_name%",
            valid.map_or(String::new(), |l| l.view_name.clone()),
        ),
        (
            "%linked_view_layout_page_label%",
            valid.map_or(String::new(), |l| l.page_label.clone()),
        ),
        (
            "%referenced_view_callout_label%",
            valid.map_or(String::new(), |l| l.callout_label.clone()),
        ),
        (
            "%layout_page_label%",
            valid.map_or(String::new(), |l| l.page_label.clone()),
        ),
        (
            "%automatic_label%",
            valid.map_or(String::new(), |l| l.callout_label.clone()),
        ),
        (
            "%simple_schedule_number%",
            v.number.map_or(String::new(), |n| n.to_string()),
        ),
        ("%height%", v.height.clone()),
        ("%note_text%", v.note_text.clone()),
    ];
    let mut out = text.to_string();
    for (k, val) in pairs {
        out = out.replace(k, &val);
    }
    out
}

// ===== geometry =====

fn rot(p: Point, a: f64) -> Point {
    let (s, c) = a.sin_cos();
    Point::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn dirv(a: f64) -> Point {
    Point::new(a.cos(), a.sin())
}

fn text_w(s: &str, h: f64) -> f64 {
    s.chars().count() as f64 * h * TEXT_WIDTH_FACTOR
}

/// `a` as an angle in (-PI, PI].
fn norm(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if r > PI {
        r - TAU
    } else {
        r
    }
}

/// `angle` turned half a circle when it would read upside down (the text
/// reads left to right or bottom to top). Returns the angle and whether it
/// turned.
pub fn readable_angle(angle: f64) -> (f64, bool) {
    let a = norm(angle);
    if a > FRAC_PI_2 + 1e-9 || a <= -FRAC_PI_2 + 1e-9 {
        (norm(a + PI), true)
    } else {
        (a, false)
    }
}

/// The generated objects of one record.
#[derive(Clone, Debug, Default)]
pub struct Gen {
    pub items: Vec<(CadItem, CadAttrs)>,
}

impl Gen {
    fn push(&mut self, item: CadItem, attrs: CadAttrs) {
        self.items.push((item, attrs));
    }

    /// A text centered on `center`.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        center: Point,
        s: &str,
        h: f64,
        angle: f64,
        style: &Option<String>,
        color: Option<[u8; 3]>,
    ) {
        if s.is_empty() {
            return;
        }
        let w = text_w(s, h);
        let pos = center.sub(rot(Point::new(w * 0.5, h * 0.5), angle));
        let mut attrs = CadAttrs {
            text_style: style.clone(),
            color,
            ..CadAttrs::default()
        };
        // Centered: the text draws through the box path, which turns it.
        attrs.text_box = TextBox {
            halign: HAlign::Center,
            ..TextBox::default()
        };
        self.push(
            CadItem::Text {
                pos,
                text: s.to_string(),
                height: h,
                angle,
            },
            attrs,
        );
    }

    fn line(&mut self, a: Point, b: Point, attrs: CadAttrs) {
        self.push(CadItem::Line { a, b }, attrs);
    }
}

/// The pose `(x, y, angle)` an item gives: its first point and the
/// direction of its first segment, or a text's anchor and angle. `None` for
/// items that have no orientation (a circle).
pub fn item_pose(item: &CadItem) -> Option<[f64; 3]> {
    match item {
        CadItem::Polyline { points, .. } => {
            let a = *points.first()?;
            let b = points.iter().find(|p| p.dist(a) > 1e-6)?;
            Some([a.x, a.y, b.sub(a).angle()])
        }
        CadItem::Line { a, b } if a.dist(*b) > 1e-9 => Some([a.x, a.y, b.sub(*a).angle()]),
        CadItem::Text { pos, angle, .. } => Some([pos.x, pos.y, *angle]),
        _ => None,
    }
}

/// The index and pose of the first generated item that has a pose.
pub fn gen_pose(g: &Gen) -> (usize, Option<[f64; 3]>) {
    for (i, (it, _)) in g.items.iter().enumerate() {
        if let Some(p) = item_pose(it) {
            return (i, Some(p));
        }
    }
    (0, None)
}

/// The rigid transform that took the generated pose `was` to the pose `now`
/// of the same item: `(rotation, translation applied after rotating about
/// the origin)`. `None` when they agree.
pub fn pose_change(was: [f64; 3], now: [f64; 3]) -> Option<(f64, Point)> {
    let d_ang = norm(now[2] - was[2]);
    let moved = Point::new(now[0], now[1]).dist(Point::new(was[0], was[1]));
    if d_ang.abs() < 1e-7 && moved < 1e-6 {
        return None;
    }
    // T(x) = R(d)(x - was) + now.
    Some((d_ang, Point::new(now[0], now[1])))
}

/// Applies the change `(d, now)` found by [`pose_change`] to `p`; `was` is
/// the earlier pose.
pub fn move_point(p: Point, was: [f64; 3], change: (f64, Point)) -> Point {
    let (d, now) = change;
    rot(p.sub(Point::new(was[0], was[1])), d).add(now)
}

/// The point where the ray from `c` along `d` leaves `poly`.
fn ray_exit(poly: &[Point], c: Point, d: Point) -> Option<Point> {
    let mut best: Option<f64> = None;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let e = q.sub(p);
        let den = d.cross(e);
        if den.abs() < 1e-12 {
            continue;
        }
        let w = p.sub(c);
        let t = w.cross(e) / den;
        let u = w.cross(d) / den;
        if t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
            best = Some(best.map_or(t, |b: f64| b.max(t)));
        }
    }
    best.map(|t| c.add(d.scale(t)))
}

/// A shape's outline in the plan: the points and the half extents.
struct Placed {
    pts: Vec<Point>,
    a: f64,
    b: f64,
}

impl Placed {
    fn new(shape: CalloutShape, c: Point, a: f64, b: f64, ang: f64) -> Placed {
        let pts = shape
            .outline(a, b)
            .into_iter()
            .map(|p| c.add(rot(p, ang)))
            .collect();
        Placed { pts, a, b }
    }

    /// Where a ray from the center `c` along `d` leaves the outline (the
    /// rim of a bounding circle when the shape is `None`).
    fn exit(&self, c: Point, d: Point) -> Point {
        ray_exit(&self.pts, c, d).unwrap_or_else(|| c.add(d.scale(self.a.max(self.b))))
    }
}

fn fill_attr(filled: bool, color: Option<[u8; 3]>, transparency: u8) -> Option<FillAttr> {
    filled.then(|| FillAttr {
        color: color.unwrap_or([255, 255, 255]),
        opacity: (255.0 * (100.0 - f64::from(transparency.min(100))) / 100.0).round() as u8,
        ..FillAttr::default()
    })
}

/// Where a text sits along a line that starts at `start`, runs `len` along
/// `dir`, above or below it. Returns the text's center and angle.
#[allow(clippy::too_many_arguments)]
fn place_on_line(
    start: Point,
    dir: Point,
    len: f64,
    text_len: f64,
    align: LineAlign,
    above: bool,
    h: f64,
    auto_adjust: bool,
) -> (Point, f64) {
    let ang = dir.angle();
    let (tang, flipped) = if auto_adjust {
        readable_angle(ang)
    } else {
        (ang, false)
    };
    let gap = h * 0.35;
    let along = match align {
        LineAlign::Toward | LineAlign::Match => text_len * 0.5 + h * 0.3,
        LineAlign::Centered => len * 0.5,
        LineAlign::Away => len - text_len * 0.5 - h * 0.3,
    };
    let n = dir.perp();
    let up = if flipped { n.scale(-1.0) } else { n };
    let side = if above { 1.0 } else { -1.0 };
    let c = start
        .add(dir.scale(along))
        .add(up.scale(side * (gap + h * 0.5)));
    (c, tang)
}

/// Everything a callout shows, from its record.
pub fn callout_items(c: &Callout, v: &Vars) -> Gen {
    let mut g = Gen::default();
    let h = c.height.max(0.5);
    let label = expand_macros(&c.label, v);
    let mut below = expand_macros(&c.text_below, v);
    if c.auto_below && below.is_empty() {
        if let Some(l) = v.link.as_ref().filter(|l| l.valid) {
            below.clone_from(&l.page_label);
        }
    }
    let ang = c.shape_angle.to_radians();
    let (a, b) = shape_extents(c.shape, c.auto_size, c.size, &label, &below, h);
    let mut text_ang = c.text_angle.map_or(ang, f64::to_radians);
    if c.auto_adjust_text {
        text_ang = readable_angle(text_ang).0;
    }
    let placed = Placed::new(c.shape, c.center, a, b, ang);
    let fill = fill_attr(c.filled, c.fill_color, c.transparency);
    let line_attrs = |extra_fill: Option<FillAttr>| {
        let mut at = c.line.attrs();
        at.fill = extra_fill;
        at
    };
    push_shape(
        &mut g,
        c.shape,
        c.center,
        &placed,
        &line_attrs(fill.clone()),
    );
    push_label(
        &mut g,
        c.center,
        &label,
        &below,
        h,
        text_ang,
        (a, b),
        ang,
        &c.text_style,
        &c.line,
    );

    // The cross section line (and a second shape for a double callout).
    if c.section.on {
        let s = &c.section;
        let d_ang = ang + s.rel_angle.to_radians();
        let d = dirv(d_ang);
        let start = placed.exit(c.center, d);
        let ab = expand_macros(&s.above.text, v);
        let bl = expand_macros(&s.below.text, v);
        let ha = h;
        let len = s
            .min_length
            .max(text_w(&ab, ha) + h)
            .max(text_w(&bl, ha) + h);
        let end = start.add(d.scale(len));
        let lattrs = s.look.attrs();
        g.line(start, end, lattrs);
        let below_align = if s.below.align == LineAlign::Match {
            s.above.align
        } else {
            s.below.align
        };
        for (text, style, align, up) in [
            (&ab, &s.above.style, s.above.align, true),
            (&bl, &s.below.style, below_align, false),
        ] {
            if text.is_empty() {
                continue;
            }
            let (ctr, tang) = place_on_line(
                start,
                d,
                len,
                text_w(text, ha),
                align,
                up,
                ha,
                s.auto_adjust_text,
            );
            let st = style.clone().or_else(|| c.text_style.clone());
            g.text(ctr, text, ha, tang, &st, None);
        }
        if s.arrow && !s.double {
            let n = d.perp();
            let tip = end.add(n.scale(s.arrow_length.max(1.0)));
            let mut at = s.look.attrs();
            at.arrow_end = s.arrow_style;
            at.arrow_size = (h * 1.2).max(3.0);
            g.line(end, tip, at);
        }
        if s.double {
            let reach = start.dist(c.center);
            let c2 = end.add(d.scale(reach));
            let placed2 = Placed::new(c.shape, c2, a, b, ang);
            push_shape(&mut g, c.shape, c2, &placed2, &line_attrs(fill));
            push_label(
                &mut g,
                c2,
                &label,
                &below,
                h,
                text_ang,
                (a, b),
                ang,
                &c.text_style,
                &c.line,
            );
        }
    }

    // The arrows ("hats").
    let ar = &c.arrows;
    for phi in &ar.angles {
        let d = dirv(ang + phi.to_radians());
        let p = placed.exit(c.center, d);
        let s = match ar.size {
            ArrowSize::Small => h * 0.9,
            ArrowSize::Large => h * 1.5,
        };
        let n = d.perp();
        let tri = vec![
            p.add(d.scale(s)),
            p.add(n.scale(s * 0.55)),
            p.sub(n.scale(s * 0.55)),
        ];
        let mut at = CadAttrs {
            color: ar.color,
            ..CadAttrs::default()
        };
        at.fill = fill_attr(
            ar.filled,
            Some(ar.color.unwrap_or([0, 0, 0])),
            ar.transparency,
        );
        g.push(
            CadItem::Polyline {
                points: tri,
                closed: true,
            },
            at,
        );
    }

    // Text lines with arrow attached to the callout.
    for pts in &c.leaders {
        if pts.is_empty() {
            continue;
        }
        let toward = *pts.last().expect("leader point");
        let dir = toward.sub(c.center);
        let dir = if dir.length() < 1e-9 {
            Point::new(1.0, 0.0)
        } else {
            dir.normalized()
        };
        let tail = placed.exit(c.center, dir);
        let mut line: Vec<Point> = pts.clone();
        line.push(tail);
        let mut at = c.line.attrs();
        at.arrow_start = ArrowStyle::Filled;
        at.arrow_size = (h * 1.2).max(3.0);
        g.push(
            CadItem::Polyline {
                points: line,
                closed: false,
            },
            at,
        );
    }
    g
}

fn shape_extents(
    shape: CalloutShape,
    auto: bool,
    size: f64,
    label: &str,
    below: &str,
    h: f64,
) -> (f64, f64) {
    if auto {
        let tw = text_w(label, h).max(text_w(below, h));
        let th = if below.is_empty() { h } else { h * 2.3 };
        shape.auto_half_extents(tw, th, h * 0.7, h)
    } else {
        shape.sized_half_extents(size.max(0.5))
    }
}

fn push_shape(g: &mut Gen, shape: CalloutShape, c: Point, placed: &Placed, attrs: &CadAttrs) {
    match shape {
        CalloutShape::None => {}
        CalloutShape::Circle if (placed.a - placed.b).abs() < 1e-9 => g.push(
            CadItem::Circle {
                center: c,
                radius: placed.a,
            },
            attrs.clone(),
        ),
        _ => g.push(
            CadItem::Polyline {
                points: placed.pts.clone(),
                closed: true,
            },
            attrs.clone(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_label(
    g: &mut Gen,
    c: Point,
    label: &str,
    below: &str,
    h: f64,
    text_ang: f64,
    half: (f64, f64),
    shape_ang: f64,
    style: &Option<String>,
    line: &LineLook,
) {
    let up = rot(Point::new(0.0, 1.0), text_ang);
    if below.is_empty() {
        g.text(c, label, h, text_ang, style, None);
        return;
    }
    // A line centered in the shape separates the two rows.
    let along = dirv(shape_ang).scale(half.0 * 0.85);
    g.line(c.sub(along), c.add(along), line.attrs());
    g.text(c.add(up.scale(h * 0.75)), label, h, text_ang, style, None);
    g.text(c.sub(up.scale(h * 0.75)), below, h, text_ang, style, None);
}

/// The Caution symbol: a triangle with an exclamation mark, drawn near `at`.
fn caution_items(g: &mut Gen, at: Point, h: f64) {
    let s = h * 0.9;
    g.push(
        CadItem::Polyline {
            points: vec![
                Point::new(at.x, at.y + s),
                Point::new(at.x - s * 0.95, at.y - s * 0.7),
                Point::new(at.x + s * 0.95, at.y - s * 0.7),
            ],
            closed: true,
        },
        CadAttrs {
            color: Some([200, 140, 0]),
            ..CadAttrs::default()
        },
    );
    g.text(
        Point::new(at.x, at.y - s * 0.1),
        "!",
        h * 0.8,
        0.0,
        &None,
        Some([200, 140, 0]),
    );
}

/// A callout with its Caution symbol when `caution` (a broken link).
pub fn callout_with_caution(c: &Callout, v: &Vars, caution: bool) -> Gen {
    let mut g = callout_items(c, v);
    if caution {
        let label = expand_macros(&c.label, v);
        let below = expand_macros(&c.text_below, v);
        let (a, b) = shape_extents(c.shape, c.auto_size, c.size, &label, &below, c.height);
        caution_items(
            &mut g,
            c.center.add(Point::new(a + c.height, b + c.height)),
            c.height,
        );
    }
    g
}

impl Note {
    /// The callout this note draws as, with the schedule `number`.
    pub fn as_callout(&self) -> Callout {
        Callout {
            items: Vec::new(),
            layer: self.layer.clone(),
            center: self.center,
            label: self.label.clone(),
            text_below: self.text_below.clone(),
            // "Generate ... from Schedule": the schedule here has no shape,
            // fill, size or angle settings of its own, so the Callout
            // defaults stand in for them.
            shape: if self.generate_shape {
                CalloutShape::Circle
            } else {
                self.shape
            },
            filled: self.filled && !self.generate_fill,
            fill_color: self.fill_color,
            transparency: self.transparency,
            auto_size: self.auto_size || self.generate_size,
            size: self.size,
            shape_angle: if self.generate_angles {
                0.0
            } else {
                self.shape_angle
            },
            text_angle: if self.generate_angles {
                None
            } else {
                self.text_angle
            },
            auto_adjust_text: self.auto_adjust_text,
            height: self.height,
            text_style: self.text_style.clone(),
            line: self.line,
            ..Callout::default()
        }
    }
}

/// Everything a note shows: its shape and label, and the Caution symbol
/// when no Note Schedule lists it.
pub fn note_items(n: &Note, v: &Vars, caution: bool) -> Gen {
    let mut v = v.clone();
    v.note_text.clone_from(&n.text);
    callout_with_caution(&n.as_callout(), &v, caution && !n.ignore_no_schedule)
}

/// Everything a marker shows.
pub fn marker_items(m: &Marker, v: &Vars) -> Gen {
    let mut g = Gen::default();
    let h = m.text_height.max(0.5);
    let r = m.radius.max(0.5);
    let c = m.center;
    let ang = m.angle.to_radians();
    let d = dirv(ang);
    let mut v = v.clone();
    v.height = crate::units::fmt_ft_in(m.height_z);
    let label = expand_macros(&m.label.text, &v);
    let below = expand_macros(&m.below.text, &v);
    let look = m.line.attrs();
    let black = Some(FillAttr {
        color: [0, 0, 0],
        opacity: 255,
        ..FillAttr::default()
    });
    match m.kind {
        MarkerKind::LevelLine => {
            g.push(
                CadItem::Circle {
                    center: c,
                    radius: r,
                },
                look.clone(),
            );
            // Two opposite quadrants filled, as on a survey target.
            for q in [0.0, PI] {
                let mut pts = vec![c];
                for k in 0..=8 {
                    let t = ang + q + FRAC_PI_2 * f64::from(k) / 8.0;
                    pts.push(c.add(dirv(t).scale(r)));
                }
                g.push(
                    CadItem::Polyline {
                        points: pts,
                        closed: true,
                    },
                    CadAttrs {
                        fill: black.clone(),
                        ..look.clone()
                    },
                );
            }
        }
        MarkerKind::Elevation => {
            // A filled triangle pointing at the level.
            let n = d.perp();
            let tri = vec![
                c,
                c.add(d.scale(r * 1.6)).add(n.scale(r)),
                c.add(d.scale(r * 1.6)).sub(n.scale(r)),
            ];
            g.push(
                CadItem::Polyline {
                    points: tri,
                    closed: true,
                },
                CadAttrs {
                    fill: black.clone(),
                    ..look.clone()
                },
            );
        }
        MarkerKind::TestBoring => {
            g.push(
                CadItem::Circle {
                    center: c,
                    radius: r,
                },
                look.clone(),
            );
            for t in [ang, ang + FRAC_PI_2] {
                g.line(
                    c.sub(dirv(t).scale(r * 1.5)),
                    c.add(dirv(t).scale(r * 1.5)),
                    look.clone(),
                );
            }
        }
        MarkerKind::Point => g.push(
            CadItem::Circle {
                center: c,
                radius: r,
            },
            CadAttrs {
                fill: black.clone(),
                ..look.clone()
            },
        ),
    }
    let edge = match m.kind {
        MarkerKind::Elevation => r * 1.6,
        _ => r,
    };
    let start = c.add(d.scale(edge));
    if m.kind.has_line() {
        let len = m
            .min_length
            .max(text_w(&label, h) + h)
            .max(text_w(&below, h) + h);
        let end = start.add(d.scale(len));
        g.line(start, end, look.clone());
        let below_align = if m.below.align == LineAlign::Match {
            m.label.align
        } else {
            m.below.align
        };
        for (text, style, align, up) in [
            (&label, &m.label.style, m.label.align, true),
            (&below, &m.below.style, below_align, false),
        ] {
            if text.is_empty() {
                continue;
            }
            let (ctr, tang) = place_on_line(
                start,
                d,
                len,
                text_w(text, h),
                align,
                up,
                h,
                m.auto_adjust_text,
            );
            let st = style.clone().or_else(|| m.text_style.clone());
            g.text(ctr, text, h, tang, &st, None);
        }
    } else if !label.is_empty() {
        // The minimum length is the gap between the marker and its text.
        let gap = m.min_length.max(h * 0.5);
        let tang = if m.auto_adjust_text {
            readable_angle(ang).0
        } else {
            ang
        };
        let w = text_w(&label, h);
        let ctr = start.add(d.scale(gap + w * 0.5));
        g.text(ctr, &label, h, tang, &m.text_style, None);
    }
    g
}

// ===== sync =====

/// The layout pages of the plan's layout files, read from their JSON (plan-core
/// does not depend on plan-layout).
#[derive(Clone, Debug, PartialEq)]
pub struct PageInfo {
    pub file: String,
    pub number: u32,
    pub title: String,
    /// `(kind, key)` of each box's source: `("Camera", id)`,
    /// `("CadDetail", name)`, `("Schedule", kind name)`.
    pub sources: Vec<(String, String)>,
    /// The page's label as Page Information gives it, its `#` numbered
    /// (empty: the plain sheet number).
    pub label: String,
}

impl PageInfo {
    pub fn label(&self) -> String {
        if self.label.is_empty() {
            format!("A-{}", self.number)
        } else {
            self.label.clone()
        }
    }
}

/// The labels of a layout file's pages in page order: a `#` takes the next
/// number among the pages with the identical label (template pages count
/// apart), a fixed label stays, an empty one is `A-{number}`. The same rule
/// as plan-layout's `resolve_labels`.
fn resolved_labels(pages: &[serde_json::Value]) -> Vec<String> {
    let mut counts: Vec<((String, bool), u32)> = Vec::new();
    pages
        .iter()
        .map(|p| {
            let label = p.get("label").and_then(|l| l.as_str()).unwrap_or("").trim();
            let template = p.get("template_page").and_then(|t| t.as_bool()) == Some(true);
            let number = p.get("number").and_then(|n| n.as_u64()).unwrap_or(0);
            if label.is_empty() {
                return format!("A-{number}");
            }
            if !label.contains('#') {
                return label.to_string();
            }
            let key = (label.to_string(), template);
            let n = match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, c)) => {
                    *c += 1;
                    *c
                }
                None => {
                    counts.push((key, 1));
                    1
                }
            };
            label.replace('#', &n.to_string())
        })
        .collect()
}

fn pages_of(file: &serde_json::Value) -> Vec<PageInfo> {
    let name = file
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    let Some(pages) = file.get("pages").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    let labels = resolved_labels(pages);
    pages
        .iter()
        .zip(labels)
        .map(|(p, label)| {
            let mut sources = Vec::new();
            for b in p
                .get("boxes")
                .and_then(|b| b.as_array())
                .into_iter()
                .flatten()
            {
                let Some(src) = b.get("source").and_then(|s| s.as_object()) else {
                    continue;
                };
                for (k, v) in src {
                    let key = match k.as_str() {
                        "Camera" | "Perspective" => v
                            .get("camera_id")
                            .and_then(|i| i.as_u64())
                            .map(|i| i.to_string()),
                        "CadDetail" => v.get("name").and_then(|n| n.as_str()).map(str::to_string),
                        "Schedule" => v.get("kind").and_then(|n| n.as_str()).map(str::to_string),
                        "PlacedSchedule" => Some(String::new()),
                        _ => None,
                    };
                    if let Some(key) = key {
                        let kind = if k == "Perspective" { "Camera" } else { k };
                        sources.push((kind.to_string(), key));
                    }
                }
            }
            PageInfo {
                file: name.clone(),
                number: p.get("number").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                title: p
                    .get("title")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string(),
                sources,
                label,
            }
        })
        .collect()
}

impl Project {
    /// The pages of the plan's layout files, in file then page order.
    pub fn layout_pages(&self) -> Vec<PageInfo> {
        let mut out = Vec::new();
        if let Some(l) = &self.layout {
            out.extend(pages_of(l));
        }
        for f in &self.layout_files {
            out.extend(pages_of(f));
        }
        out
    }

    /// The first layout page that holds a box of `kind` and `key`.
    fn page_with(&self, kind: &str, key: &str) -> Option<PageInfo> {
        self.layout_pages()
            .into_iter()
            .find(|p| p.sources.iter().any(|(k, v)| k == kind && v == key))
    }

    /// What the Link panel reports about `link`: the view, its callout label
    /// and the layout page it was sent to.
    pub fn resolve_view_link(&self, link: &ViewLink) -> LinkInfo {
        let file_name = self
            .layout
            .as_ref()
            .and_then(|l| l.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        match link.kind {
            ViewKind::Camera => {
                let Some(cam) = self.camera(link.id) else {
                    return LinkInfo::default();
                };
                let page = self.page_with("Camera", &link.id.to_string());
                LinkInfo {
                    valid: true,
                    view_name: cam.name.clone(),
                    view_type: ViewKind::Camera.label().to_string(),
                    callout_label: self
                        .callout_number(cam.id)
                        .map_or_else(|| cam.name.clone(), |n| n.to_string()),
                    page_label: page.as_ref().map_or(String::new(), PageInfo::label),
                    page_number: page.as_ref().map(|p| p.number),
                    file_name: page.map_or(file_name, |p| p.file),
                }
            }
            ViewKind::CadDetail => {
                let exists = self
                    .floors
                    .iter()
                    .any(|f| f.detail.is_some() && f.name == link.name);
                if !exists {
                    return LinkInfo::default();
                }
                let page = self.page_with("CadDetail", &link.name);
                LinkInfo {
                    valid: true,
                    view_name: link.name.clone(),
                    view_type: ViewKind::CadDetail.label().to_string(),
                    callout_label: link.name.clone(),
                    page_label: page.as_ref().map_or(String::new(), PageInfo::label),
                    page_number: page.as_ref().map(|p| p.number),
                    file_name: page.map_or(file_name, |p| p.file),
                }
            }
            ViewKind::LayoutPage => {
                let n = link.id as u32;
                let Some(p) = self.layout_pages().into_iter().find(|p| p.number == n) else {
                    return LinkInfo::default();
                };
                LinkInfo {
                    valid: true,
                    view_name: p.title.clone(),
                    view_type: ViewKind::LayoutPage.label().to_string(),
                    callout_label: p.title.clone(),
                    page_label: p.label(),
                    page_number: Some(p.number),
                    file_name: p.file,
                }
            }
        }
    }

    /// The schedule number of every note, `[floor][index]`: notes of one
    /// type are numbered 1, 2, 3 in draw order (floor by floor, in the order
    /// they were placed).
    pub fn note_numbering(&self) -> Vec<Vec<u32>> {
        let mut counts: HashMap<String, u32> = HashMap::new();
        self.floors
            .iter()
            .map(|f| {
                f.annots
                    .notes
                    .iter()
                    .map(|n| {
                        let c = counts.entry(n.note_type.clone()).or_insert(0);
                        *c += 1;
                        *c
                    })
                    .collect()
            })
            .collect()
    }

    /// Does a Note Schedule (a placed one, or a layout box) list notes of
    /// `note_type`?
    pub fn note_schedule_exists(&self, note_type: &str) -> bool {
        use crate::schedules::{ScheduleKind, ScheduleLayer};
        let placed = self.floors.iter().any(|f| {
            ScheduleLayer::load(f).schedules.iter().any(|s| {
                s.kind == ScheduleKind::Note
                    && (s.filter.trim().is_empty()
                        || note_type
                            .to_lowercase()
                            .contains(&s.filter.trim().to_lowercase()))
            })
        });
        placed
    }

    fn vars_for_link(&self, link: Option<&ViewLink>) -> Vars {
        Vars {
            link: link.map(|l| self.resolve_view_link(l)),
            ..Vars::default()
        }
    }

    /// Brings the CAD objects of every callout, marker and note up to date
    /// with their records (see the module docs). Returns true when anything
    /// changed.
    pub fn sync_annotations(&mut self) -> bool {
        let mut changed = false;
        let numbers = self.note_numbering();
        for fi in 0..self.floors.len() {
            if self.floors[fi].annots.is_empty() {
                continue;
            }
            changed |= self.drop_dead_records(fi);
            for i in 0..self.floors[fi].annots.callouts.len() {
                let mut rec = self.floors[fi].annots.callouts[i].clone();
                if let Some(t) = pose_shift(self, fi, &rec.items, rec.pose_idx, rec.pose) {
                    rec.center = t.pt(rec.center);
                    for l in &mut rec.leaders {
                        for p in l.iter_mut() {
                            *p = t.pt(*p);
                        }
                    }
                    rec.shape_angle += t.deg();
                    if let Some(a) = rec.text_angle.as_mut() {
                        *a += t.deg();
                    }
                }
                let v = self.vars_for_link(rec.link.as_ref());
                let broken = rec.link.is_some()
                    && !rec.ignore_link
                    && !v.link.as_ref().is_some_and(|l| l.valid);
                let g = callout_with_caution(&rec, &v, broken);
                let layer = rec.layer.clone();
                let wrote = self.write_gen(
                    fi,
                    &mut rec.items,
                    &mut rec.pose_idx,
                    &mut rec.pose,
                    &layer,
                    g,
                );
                let differs = rec != self.floors[fi].annots.callouts[i];
                if differs {
                    self.floors[fi].annots.callouts[i] = rec;
                }
                changed |= wrote || differs;
            }
            for i in 0..self.floors[fi].annots.markers.len() {
                let mut rec = self.floors[fi].annots.markers[i].clone();
                if let Some(t) = pose_shift(self, fi, &rec.items, rec.pose_idx, rec.pose) {
                    rec.center = t.pt(rec.center);
                    rec.angle += t.deg();
                }
                let g = marker_items(&rec, &Vars::default());
                let layer = rec.layer.clone();
                let wrote = self.write_gen(
                    fi,
                    &mut rec.items,
                    &mut rec.pose_idx,
                    &mut rec.pose,
                    &layer,
                    g,
                );
                let differs = rec != self.floors[fi].annots.markers[i];
                if differs {
                    self.floors[fi].annots.markers[i] = rec;
                }
                changed |= wrote || differs;
            }
            for i in 0..self.floors[fi].annots.notes.len() {
                let mut rec = self.floors[fi].annots.notes[i].clone();
                if let Some(t) = pose_shift(self, fi, &rec.items, rec.pose_idx, rec.pose) {
                    rec.center = t.pt(rec.center);
                    rec.shape_angle += t.deg();
                    if let Some(a) = rec.text_angle.as_mut() {
                        *a += t.deg();
                    }
                }
                let v = Vars {
                    number: numbers.get(fi).and_then(|n| n.get(i)).copied(),
                    ..Vars::default()
                };
                let caution = !self.note_schedule_exists(&rec.note_type);
                let g = note_items(&rec, &v, caution);
                let layer = rec.layer.clone();
                let wrote = self.write_gen(
                    fi,
                    &mut rec.items,
                    &mut rec.pose_idx,
                    &mut rec.pose,
                    &layer,
                    g,
                );
                let differs = rec != self.floors[fi].annots.notes[i];
                if differs {
                    self.floors[fi].annots.notes[i] = rec;
                }
                changed |= wrote || differs;
            }
        }
        // Text and Rich Text with macros follow the plan too (`crate::macros`).
        changed |= self.sync_macro_texts();
        changed
    }

    /// Drops the records whose objects were deleted (or partly deleted: the
    /// rest stay as plain CAD).
    fn drop_dead_records(&mut self, fi: usize) -> bool {
        let f = &mut self.floors[fi];
        let live: std::collections::HashSet<Id> = f.cad.iter().map(|c| c.id).collect();
        let ok = |items: &Vec<Id>| !items.is_empty() && items.iter().all(|i| live.contains(i));
        let n = f.annots.callouts.len() + f.annots.markers.len() + f.annots.notes.len();
        f.annots.callouts.retain(|c| ok(&c.items));
        f.annots.markers.retain(|c| ok(&c.items));
        f.annots.notes.retain(|c| ok(&c.items));
        n != f.annots.callouts.len() + f.annots.markers.len() + f.annots.notes.len()
    }

    /// Writes `g` over the CAD objects `items` of floor `fi` (reusing their
    /// ids in order, adding or removing at the end), updates the attributes
    /// and the group, and stores the pose. Returns true when the floor
    /// changed.
    pub fn write_gen(
        &mut self,
        fi: usize,
        items: &mut Vec<Id>,
        pose_idx: &mut usize,
        pose: &mut Option<[f64; 3]>,
        layer: &str,
        g: Gen,
    ) -> bool {
        let mut changed = false;
        let (idx, p) = gen_pose(&g);
        *pose_idx = idx;
        *pose = p;
        let want = g.items.len();
        while items.len() < want {
            let id = self.alloc_id();
            self.floors[fi].cad.push(crate::cad::CadObject {
                id,
                layer: layer.to_string(),
                item: CadItem::Line {
                    a: Point::ZERO,
                    b: Point::ZERO,
                },
            });
            items.push(id);
            changed = true;
        }
        if items.len() > want {
            let gone: Vec<Id> = items.split_off(want);
            self.floors[fi].cad.retain(|c| !gone.contains(&c.id));
            self.floors[fi]
                .cad_attrs
                .retain(|a| !gone.contains(&a.target));
            changed = true;
        }
        for (id, (item, mut attrs)) in items.clone().into_iter().zip(g.items) {
            attrs.target = id;
            let f = &mut self.floors[fi];
            if let Some(o) = f.cad.iter_mut().find(|o| o.id == id) {
                if o.item != item || o.layer != layer {
                    o.item = item;
                    o.layer = layer.to_string();
                    changed = true;
                }
            }
            let cur = f.cad_attrs(id);
            let differs = match &cur {
                Some(c) => *c != attrs,
                None => !attrs.is_default(),
            };
            if differs {
                self.set_cad_attrs(fi, attrs);
                changed = true;
            }
        }
        // They select as one.
        if items.len() >= 2 {
            let f = &self.floors[fi];
            let whole = f.group_of(ObjectRef::Cad(items[0])).is_some_and(|gr| {
                items
                    .iter()
                    .all(|i| gr.members.contains(&ObjectRef::Cad(*i)))
            });
            if !whole {
                let members: Vec<ObjectRef> = items.iter().map(|i| ObjectRef::Cad(*i)).collect();
                self.make_group(fi, &members);
                changed = true;
            }
        }
        changed
    }
}

/// A move or rotation of a record's generated objects since they were
/// written, found by [`pose_shift`]: apply it to every point the record
/// keeps with [`PoseShift::pt`] and add [`PoseShift::deg`] to its angles.
#[derive(Clone, Copy, Debug)]
pub struct PoseShift {
    was: [f64; 3],
    change: (f64, Point),
}

impl PoseShift {
    pub fn pt(&self, p: Point) -> Point {
        move_point(p, self.was, self.change)
    }

    /// The turn, degrees.
    pub fn deg(&self) -> f64 {
        self.change.0.to_degrees()
    }
}

/// Has the watched item of a record been moved or turned (by the Select
/// tool) since the record wrote it?
pub fn pose_shift(
    p: &Project,
    fi: usize,
    items: &[Id],
    pose_idx: usize,
    pose: Option<[f64; 3]>,
) -> Option<PoseShift> {
    let was = pose?;
    let id = *items.get(pose_idx)?;
    let now = p.floors[fi]
        .cad
        .iter()
        .find(|c| c.id == id)
        .and_then(|c| item_pose(&c.item))?;
    Some(PoseShift {
        was,
        change: pose_change(was, now)?,
    })
}

// ===== edit handles =====

/// Ids of the edit handles of an annotation (the `u8` of an [`AnnotHandle`]).
pub mod handle {
    /// The small circle on the perimeter: resizes about the center.
    pub const RESIZE: u8 = 0;
    /// The large triangle outside the shape: turns it.
    pub const ROTATE: u8 = 1;
    /// The square at the end of the cross section line (or of a marker's
    /// line).
    pub const EXTEND: u8 = 2;
    /// The diamond below a callout: drags out a Text Line with Arrow.
    pub const ADD_LINE: u8 = 3;
    /// The diamond just outside a callout: drags out an arrow ("hat"), or
    /// removes the last one when dragged to the center.
    pub const ADD_ARROW: u8 = 4;
    /// The small triangle at the end of arrow `k` is `ARROW_BASE + k`.
    pub const ARROW_BASE: u8 = 10;
}

/// One edit handle: which, and where in the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotHandle {
    pub id: u8,
    pub pos: Point,
}

fn snap_deg(deg: f64, step: Option<f64>) -> f64 {
    match step {
        Some(s) if s > 0.0 => (deg / s).round() * s,
        _ => (deg * 2.0).round() / 2.0,
    }
}

/// The edit handles of a callout; `up` is the distance the Rotate handle
/// stands off the outline (plan inches, a fixed number of pixels).
pub fn callout_handles(c: &Callout, v: &Vars, up: f64) -> Vec<AnnotHandle> {
    let h = c.height.max(0.5);
    let label = expand_macros(&c.label, v);
    let below = expand_macros(&c.text_below, v);
    let ang = c.shape_angle.to_radians();
    let (a, b) = shape_extents(c.shape, c.auto_size, c.size, &label, &below, h);
    let placed = Placed::new(c.shape, c.center, a, b, ang);
    let mut out = Vec::new();
    let mut rotate_dir = dirv(ang + FRAC_PI_2);
    let mut rotate_from = placed.exit(c.center, rotate_dir);
    if c.section.on {
        let d = dirv(ang + c.section.rel_angle.to_radians());
        let start = placed.exit(c.center, d);
        let ab = expand_macros(&c.section.above.text, v);
        let bl = expand_macros(&c.section.below.text, v);
        let len = c
            .section
            .min_length
            .max(text_w(&ab, h) + h)
            .max(text_w(&bl, h) + h);
        let end = start.add(d.scale(len));
        out.push(AnnotHandle {
            id: handle::RESIZE,
            pos: start,
        });
        out.push(AnnotHandle {
            id: handle::EXTEND,
            pos: end,
        });
        rotate_dir = d;
        rotate_from = end;
    } else {
        out.push(AnnotHandle {
            id: handle::RESIZE,
            pos: placed.exit(c.center, dirv(ang)),
        });
    }
    out.push(AnnotHandle {
        id: handle::ROTATE,
        pos: rotate_from.add(rotate_dir.scale(up)),
    });
    let below_dir = dirv(ang - FRAC_PI_2);
    out.push(AnnotHandle {
        id: handle::ADD_LINE,
        pos: placed
            .exit(c.center, below_dir)
            .add(below_dir.scale(up * 0.7)),
    });
    let hat = dirv(ang + FRAC_PI_4);
    out.push(AnnotHandle {
        id: handle::ADD_ARROW,
        pos: placed.exit(c.center, hat).add(hat.scale(up * 0.45)),
    });
    for (k, phi) in c.arrows.angles.iter().enumerate() {
        let d = dirv(ang + phi.to_radians());
        let size = match c.arrows.size {
            ArrowSize::Small => h * 0.9,
            ArrowSize::Large => h * 1.5,
        };
        out.push(AnnotHandle {
            id: handle::ARROW_BASE + k as u8,
            pos: placed.exit(c.center, d).add(d.scale(size + up * 0.4)),
        });
    }
    out
}

/// Drags handle `id` of `c` to `world` (the callout as it was when the drag
/// began). `step` snaps angles to that many degrees. Returns false for a
/// handle the callout does not have.
pub fn drag_callout_handle(
    c: &mut Callout,
    v: &Vars,
    id: u8,
    world: Point,
    step: Option<f64>,
) -> bool {
    let to = world.sub(c.center);
    let dist = to.length();
    let ang_deg = to.angle().to_degrees();
    match id {
        handle::RESIZE => {
            if dist < 0.5 {
                return true;
            }
            c.auto_size = false;
            c.size = (dist / c.shape.sized_half_extents(1.0).0).max(1.0);
        }
        handle::ROTATE => {
            // The handle stands at 90 degrees from the shape's X axis (or
            // at the end of the cross section line).
            let base = if c.section.on {
                c.section.rel_angle
            } else {
                90.0
            };
            c.shape_angle = snap_deg(ang_deg - base, step).rem_euclid(360.0);
            if c.shape_angle > 180.0 {
                c.shape_angle -= 360.0;
            }
        }
        handle::EXTEND => {
            if dist < 0.5 {
                return true;
            }
            c.section.on = true;
            c.section.rel_angle =
                norm((snap_deg(ang_deg, step) - c.shape_angle).to_radians()).to_degrees();
            // The line starts on the outline: its length is what is left.
            let mut probe = c.clone();
            probe.section.min_length = 0.0;
            let start = callout_handles(&probe, v, 0.0)
                .iter()
                .find(|h| h.id == handle::RESIZE)
                .map_or(c.center, |h| h.pos);
            c.section.min_length = (world.dist(start)).max(6.0);
        }
        handle::ADD_LINE => c.leaders.push(vec![world]),
        handle::ADD_ARROW => {
            let reach = {
                let label = expand_macros(&c.label, v);
                let below = expand_macros(&c.text_below, v);
                let (a, b) = shape_extents(c.shape, c.auto_size, c.size, &label, &below, c.height);
                a.max(b)
            };
            if dist < reach {
                c.arrows.angles.pop();
            } else {
                c.arrows
                    .angles
                    .push(norm((ang_deg - c.shape_angle).to_radians()).to_degrees());
            }
        }
        k if k >= handle::ARROW_BASE => {
            let i = usize::from(k - handle::ARROW_BASE);
            let Some(slot) = c.arrows.angles.get_mut(i) else {
                return false;
            };
            *slot = norm((snap_deg(ang_deg, step) - c.shape_angle).to_radians()).to_degrees();
        }
        _ => return false,
    }
    true
}

/// The edit handles of a marker: Concentric Resize, Extend and Rotate.
pub fn marker_handles(m: &Marker, up: f64) -> Vec<AnnotHandle> {
    let ang = m.angle.to_radians();
    let d = dirv(ang);
    let edge = if m.kind == MarkerKind::Elevation {
        m.radius * 1.6
    } else {
        m.radius
    };
    let start = m.center.add(d.scale(edge));
    let h = m.text_height.max(0.5);
    let label_w = text_w(&m.label.text, h).max(text_w(&m.below.text, h)) + h;
    let end = if m.kind.has_line() {
        start.add(d.scale(m.min_length.max(label_w)))
    } else {
        start.add(d.scale(m.min_length.max(h * 0.5)))
    };
    vec![
        AnnotHandle {
            id: handle::RESIZE,
            pos: m.center.add(d.perp().scale(m.radius)),
        },
        AnnotHandle {
            id: handle::EXTEND,
            pos: end,
        },
        AnnotHandle {
            id: handle::ROTATE,
            pos: end.add(d.scale(up)),
        },
    ]
}

/// Drags handle `id` of marker `m` to `world`.
pub fn drag_marker_handle(m: &mut Marker, id: u8, world: Point, step: Option<f64>) -> bool {
    let to = world.sub(m.center);
    let dist = to.length();
    match id {
        handle::RESIZE => m.radius = dist.max(1.0),
        handle::ROTATE => m.angle = snap_deg(to.angle().to_degrees(), step),
        handle::EXTEND => {
            m.angle = snap_deg(to.angle().to_degrees(), step);
            m.min_length = (dist - m.radius).max(6.0);
        }
        _ => return false,
    }
    true
}

/// The edit handles of a note: Concentric Resize and Rotate.
pub fn note_handles(n: &Note, v: &Vars, up: f64) -> Vec<AnnotHandle> {
    let mut v = v.clone();
    v.note_text.clone_from(&n.text);
    callout_handles(&n.as_callout(), &v, up)
        .into_iter()
        .filter(|h| matches!(h.id, handle::RESIZE | handle::ROTATE))
        .collect()
}

/// Drags handle `id` of note `n` to `world`.
pub fn drag_note_handle(n: &mut Note, v: &Vars, id: u8, world: Point, step: Option<f64>) -> bool {
    if !matches!(id, handle::RESIZE | handle::ROTATE) {
        return false;
    }
    let mut c = n.as_callout();
    if !drag_callout_handle(&mut c, v, id, world, step) {
        return false;
    }
    n.auto_size = c.auto_size;
    n.size = c.size;
    n.shape_angle = c.shape_angle;
    if id == handle::RESIZE {
        n.generate_size = false;
    } else {
        n.generate_angles = false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraKind, CameraObject};
    use crate::transform::{xform_cad_item, Xform};

    fn project() -> Project {
        Project::new("t")
    }

    fn item_of(p: &Project, id: Id) -> CadItem {
        p.floors[0]
            .cad
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .item
            .clone()
    }

    fn texts(p: &Project, items: &[Id]) -> Vec<String> {
        items
            .iter()
            .filter_map(|i| match item_of(p, *i) {
                CadItem::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn there_are_ten_shapes_and_none_and_each_has_an_outline() {
        assert_eq!(CalloutShape::TEN.len(), 10);
        assert_eq!(CalloutShape::ALL.len(), 11);
        assert!(CalloutShape::None.outline(5.0, 5.0).is_empty());
        for s in CalloutShape::TEN {
            let pts = s.outline(6.0, 4.0);
            assert!(pts.len() >= 3, "{}", s.label());
            // Every outline holds the origin (the label sits there).
            assert!(
                crate::geometry::point_in_polygon(Point::ZERO, &pts),
                "{}",
                s.label()
            );
            assert_eq!(CalloutShape::from_label(s.label()), Some(s));
        }
    }

    #[test]
    fn each_shape_makes_a_callout_whose_outline_holds_the_label() {
        for s in CalloutShape::TEN {
            let c = Callout {
                shape: s,
                label: "A-101".into(),
                ..Callout::default()
            };
            let g = callout_items(&c, &Vars::default());
            // Outline, then the label.
            assert_eq!(g.items.len(), 2, "{}", s.label());
            let (lo, hi) = g.items[0].0.bounds();
            let (tlo, thi) = g.items[1].0.bounds();
            assert!(
                lo.x <= tlo.x + 1e-6 && hi.x >= thi.x - 1e-6,
                "{}",
                s.label()
            );
        }
        let none = callout_items(
            &Callout {
                shape: CalloutShape::None,
                ..Callout::default()
            },
            &Vars::default(),
        );
        assert_eq!(none.items.len(), 1);
    }

    #[test]
    fn a_cross_section_line_has_its_length_angle_and_texts() {
        let mut c = Callout::default();
        c.section.on = true;
        c.section.min_length = 30.0;
        c.section.rel_angle = 90.0;
        c.section.above.text = "DETAIL".into();
        c.section.below.text = "SEE A-3".into();
        c.section.arrow = true;
        let g = callout_items(&c, &Vars::default());
        let lines: Vec<(Point, Point)> = g
            .items
            .iter()
            .filter_map(|(i, _)| match i {
                CadItem::Line { a, b } => Some((*a, *b)),
                _ => None,
            })
            .collect();
        // The section line and the section arrow.
        assert_eq!(lines.len(), 2);
        let (a, b) = lines[0];
        assert!(a.dist(b) >= 30.0 - 1e-6);
        assert!((b.sub(a).angle() - FRAC_PI_2).abs() < 1e-9);
        // The line starts on the circle's rim.
        let r = match &g.items[0].0 {
            CadItem::Circle { radius, .. } => *radius,
            other => panic!("{other:?}"),
        };
        assert!((a.dist(c.center) - r).abs() < 0.5);
        let n_text = g
            .items
            .iter()
            .filter(|(i, _)| matches!(i, CadItem::Text { .. }))
            .count();
        assert_eq!(n_text, 3, "label, text above and text below");
        let arrow = &g
            .items
            .iter()
            .find(|(i, _)| matches!(i, CadItem::Line { a: s, .. } if s.dist(b) < 1e-9))
            .unwrap()
            .1;
        assert_eq!(arrow.arrow_end, ArrowStyle::Filled);
    }

    #[test]
    fn the_section_line_follows_the_shape_angle_and_text_stays_readable() {
        let mut c = Callout {
            shape_angle: 180.0,
            ..Callout::default()
        };
        c.section.on = true;
        c.section.above.text = "X".into();
        let g = callout_items(&c, &Vars::default());
        let CadItem::Line { a, b } = g
            .items
            .iter()
            .find(|(i, _)| matches!(i, CadItem::Line { .. }))
            .unwrap()
            .0
            .clone()
        else {
            unreachable!()
        };
        assert!((norm(b.sub(a).angle() - PI)).abs() < 1e-9);
        // The label above the line reads left to right even though the
        // line points left.
        for (i, _) in &g.items {
            if let CadItem::Text { angle, .. } = i {
                assert!(angle.abs() < 1e-9, "{angle}");
            }
        }
        assert_eq!(readable_angle(-FRAC_PI_2).0, FRAC_PI_2);
        assert!(!readable_angle(0.5).1);
        assert!(readable_angle(3.0).1);
    }

    #[test]
    fn a_double_callout_has_two_shapes_and_the_same_label_in_both() {
        let mut c = Callout::default();
        c.section.on = true;
        c.section.double = true;
        c.label = "7".into();
        let g = callout_items(&c, &Vars::default());
        let circles = g
            .items
            .iter()
            .filter(|(i, _)| matches!(i, CadItem::Circle { .. }))
            .count();
        let labels = g
            .items
            .iter()
            .filter(|(i, _)| matches!(i, CadItem::Text { text, .. } if text == "7"))
            .count();
        assert_eq!((circles, labels), (2, 2));
    }

    #[test]
    fn arrows_are_triangles_on_the_rim_and_leaders_attach_to_the_outline() {
        let mut c = Callout::default();
        c.arrows.set_count(3);
        assert_eq!(c.arrows.angles, vec![90.0, 180.0, 270.0]);
        c.arrows.set_count(1);
        assert_eq!(c.arrows.angles.len(), 1);
        c.leaders = vec![vec![Point::new(100.0, 0.0)]];
        let g = callout_items(&c, &Vars::default());
        let tri = g.items.iter().filter(|(i, _)| matches!(i, CadItem::Polyline { points, closed: true } if points.len() == 3)).count();
        assert_eq!(tri, 1);
        let (leader, at) = g
            .items
            .iter()
            .find(|(i, a)| {
                matches!(i, CadItem::Polyline { closed: false, .. })
                    && a.arrow_start == ArrowStyle::Filled
            })
            .unwrap();
        let CadItem::Polyline { points, .. } = leader else {
            unreachable!()
        };
        assert_eq!(points[0], Point::new(100.0, 0.0));
        let r = match &g.items[0].0 {
            CadItem::Circle { radius, .. } => *radius,
            _ => unreachable!(),
        };
        // The tail is on the rim toward the tip.
        assert!((points[1].dist(c.center) - r).abs() < 0.2);
        assert!(at.arrow_size > 0.0);
    }

    #[test]
    fn markers_of_each_kind() {
        let mut m = Marker::default();
        m.label.text = "T.O. PLATE".into();
        m.below.text = "%height%".into();
        m.height_z = 96.0;
        let level = marker_items(&m, &Vars::default());
        // Circle, two filled quadrants, the line, two texts.
        assert_eq!(level.items.len(), 6);
        let t: Vec<String> = level
            .items
            .iter()
            .filter_map(|(i, _)| match i {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(t, vec!["T.O. PLATE".to_string(), "8'-0\"".to_string()]);
        m.kind = MarkerKind::TestBoring;
        m.below.text.clear();
        let boring = marker_items(&m, &Vars::default());
        // Circle, crosshair, label.
        assert_eq!(boring.items.len(), 4);
        m.kind = MarkerKind::Point;
        m.label.text.clear();
        assert_eq!(marker_items(&m, &Vars::default()).items.len(), 1);
        m.kind = MarkerKind::Elevation;
        m.label.text = "%height%".into();
        let e = marker_items(&m, &Vars::default());
        assert!(e
            .items
            .iter()
            .any(|(i, _)| matches!(i, CadItem::Text { text, .. } if text == "8'-0\"")));
    }

    #[test]
    fn adding_a_callout_draws_grouped_cad_and_a_second_sync_changes_nothing() {
        let mut p = project();
        let id = p.add_callout(
            0,
            Callout {
                label: "A".into(),
                ..Callout::default()
            },
        );
        {
            let f = &p.floors[0];
            assert_eq!(f.annots.callouts.len(), 1);
            assert_eq!(f.cad.len(), 2);
            assert_eq!(f.annot_of(id), Some(AnnotRef::Callout(0)));
            assert_eq!(f.group_members_of(ObjectRef::Cad(id)).len(), 2);
        }
        assert!(!p.sync_annotations());
        assert!(!p.sync_annotations());
        let f = &p.floors[0];
        // The text is drawn through the box path (so it turns).
        let txt = f
            .cad
            .iter()
            .find(|c| matches!(c.item, CadItem::Text { .. }))
            .unwrap();
        assert_eq!(f.cad_attrs(txt.id).unwrap().text_box.halign, HAlign::Center);
    }

    #[test]
    fn moving_or_turning_the_group_is_taken_back_into_the_record() {
        let mut p = project();
        let id = p.add_callout(
            0,
            Callout {
                shape: CalloutShape::Rectangle,
                center: Point::new(100.0, 50.0),
                label: "B".into(),
                ..Callout::default()
            },
        );
        let items = p.floors[0].annots.callouts[0].items.clone();
        let _ = id;
        // Move by (20, 10) and turn 90 degrees about (0, 0).
        let x =
            Xform::translate(Point::new(20.0, 10.0)).then(Xform::rotate(Point::ZERO, FRAC_PI_2));
        for c in &mut p.floors[0].cad {
            xform_cad_item(&mut c.item, &x);
        }
        let want = x.apply(Point::new(100.0, 50.0));
        assert!(p.sync_annotations());
        {
            let c = &p.floors[0].annots.callouts[0];
            assert!(c.center.dist(want) < 1e-6, "{:?} vs {want:?}", c.center);
            assert!((norm(c.shape_angle.to_radians() - FRAC_PI_2)).abs() < 1e-6);
        }
        // The objects now match what the record generates, and it is stable.
        assert!(!p.sync_annotations());
        assert_eq!(p.floors[0].annots.callouts[0].items, items);
    }

    #[test]
    fn a_linked_callout_shows_the_camera_label_and_the_sheet_it_was_sent_to() {
        let mut p = project();
        let cam = p.add_camera(CameraObject::new(
            CameraKind::CrossSection { back_clip: None },
            Point::new(0.0, 0.0),
            90.0,
            "Section A",
            0,
        ));
        let mut c = Callout {
            auto_below: true,
            label: "%referenced_view_callout_label%".into(),
            link: Some(ViewLink {
                kind: ViewKind::Camera,
                id: cam,
                name: "Section A".into(),
            }),
            ..Callout::default()
        };
        c.section.on = true;
        c.section.above.text = "%linked_view_name%".into();
        let id = p.add_callout(0, c);
        let items = p.floors[0].annots.callouts[0].items.clone();
        // Not on a sheet yet: the camera's callout number, no page.
        assert_eq!(texts(&p, &items), vec!["1", "Section A"]);
        // Sent to layout page 3.
        p.layout = Some(serde_json::json!({
            "name": "Plans",
            "pages": [
                {"number": 1, "title": "Plan", "boxes": []},
                {"number": 3, "title": "Sections", "boxes": [{"source": {"Camera": {"camera_id": cam}}}]}
            ]
        }));
        assert!(p.sync_annotations());
        let items = p.floors[0].annots.callouts[0].items.clone();
        assert_eq!(texts(&p, &items), vec!["1", "A-3", "Section A"]);
        let _ = id;
        let info = p.resolve_view_link(&ViewLink {
            kind: ViewKind::Camera,
            id: cam,
            name: String::new(),
        });
        assert_eq!(
            (info.page_label.as_str(), info.page_number),
            ("A-3", Some(3))
        );
        assert_eq!(info.file_name, "Plans");
        // Moved to another page: the callout follows.
        p.layout.as_mut().unwrap()["pages"][1]["number"] = 5.into();
        p.sync_annotations();
        let items = p.floors[0].annots.callouts[0].items.clone();
        assert!(texts(&p, &items).contains(&"A-5".to_string()));
        // Deleting the camera breaks the link: a Caution symbol appears
        // until Ignore Invalid Links.
        p.remove_camera(cam);
        p.sync_annotations();
        let n = p.floors[0].annots.callouts[0].items.len();
        assert!(texts(&p, &p.floors[0].annots.callouts[0].items.clone()).contains(&"!".to_string()));
        p.floors[0].annots.callouts[0].ignore_link = true;
        p.sync_annotations();
        assert!(p.floors[0].annots.callouts[0].items.len() < n);
    }

    #[test]
    fn a_detail_and_a_layout_page_can_be_linked_too() {
        let mut p = project();
        let mut d = crate::model::Floor::new("A", 0.0);
        d.detail = Some(crate::details::CadDetailInfo::default());
        p.floors.push(d);
        p.layout = Some(serde_json::json!({
            "name": "L", "pages": [{"number": 2, "title": "Details", "boxes": [{"source": {"CadDetail": {"name": "A", "items": []}}}]}]
        }));
        let i = p.resolve_view_link(&ViewLink {
            kind: ViewKind::CadDetail,
            id: 0,
            name: "A".into(),
        });
        assert!(i.valid);
        assert_eq!(
            (
                i.view_name.as_str(),
                i.page_label.as_str(),
                i.view_type.as_str()
            ),
            ("A", "A-2", "CAD Detail")
        );
        let l = p.resolve_view_link(&ViewLink {
            kind: ViewKind::LayoutPage,
            id: 2,
            name: String::new(),
        });
        assert!(l.valid && l.view_name == "Details" && l.page_label == "A-2");
        assert!(
            !p.resolve_view_link(&ViewLink {
                kind: ViewKind::LayoutPage,
                id: 9,
                name: String::new()
            })
            .valid
        );
        assert!(
            !p.resolve_view_link(&ViewLink {
                kind: ViewKind::CadDetail,
                id: 0,
                name: "Z".into()
            })
            .valid
        );
    }

    #[test]
    fn notes_number_per_type_in_draw_order_and_renumber_when_one_goes() {
        let mut p = project();
        let note = |t: &str, ty: &str| Note {
            text: t.into(),
            note_type: ty.into(),
            ..Note::default()
        };
        p.add_note(0, note("first", "General Note"));
        p.add_note(0, note("framing", "Framing Note"));
        p.add_note(0, note("second", "General Note"));
        p.add_note(0, note("third", "General Note"));
        let rows = p.note_rows();
        let list: Vec<(String, u32, String)> = rows
            .iter()
            .map(|r| (r.note_type.clone(), r.number, r.text.clone()))
            .collect();
        assert_eq!(
            list,
            vec![
                ("General Note".to_string(), 1, "first".to_string()),
                ("General Note".to_string(), 2, "second".to_string()),
                ("General Note".to_string(), 3, "third".to_string()),
                ("Framing Note".to_string(), 1, "framing".to_string()),
            ]
        );
        assert_eq!(rows[3].mark, "F 1");
        // The shape shows the number.
        assert!(texts(&p, &p.floors[0].annots.notes[3].items.clone()).contains(&"3".to_string()));
        // Deleting the first renumbers the rest.
        p.remove_annot(0, AnnotRef::Note(0));
        p.sync_annotations();
        let nums: Vec<u32> = p
            .note_rows()
            .iter()
            .filter(|r| r.note_type == "General Note")
            .map(|r| r.number)
            .collect();
        assert_eq!(nums, vec![1, 2]);
        let shown: Vec<Vec<String>> = p.floors[0]
            .annots
            .notes
            .iter()
            .filter(|n| n.note_type == "General Note")
            .map(|n| texts(&p, &n.items))
            .collect();
        assert_eq!(shown[0], vec!["1".to_string(), "!".to_string()]);
        assert_eq!(shown[1], vec!["2".to_string(), "!".to_string()]);
    }

    #[test]
    fn a_note_shows_a_caution_symbol_until_a_note_schedule_exists() {
        let mut p = project();
        let id = p.add_note(
            0,
            Note {
                text: "x".into(),
                ..Note::default()
            },
        );
        let n_items = |p: &Project| p.floors[0].annots.notes[0].items.len();
        // Shape, label, caution triangle and its mark.
        assert_eq!(n_items(&p), 4);
        let sid = p
            .create_note_schedule(0, &["General Note".to_string()], Point::new(500.0, 0.0))
            .unwrap();
        let _ = sid;
        p.sync_annotations();
        assert_eq!(n_items(&p), 2);
        assert_eq!(p.floors[0].annot_of(id), Some(AnnotRef::Note(0)));
        // Ignore Note With No Schedule hides it too.
        let mut q = project();
        q.add_note(0, Note::default());
        assert_eq!(q.ignore_notes_without_schedule(0, &[]), 1);
        q.sync_annotations();
        assert_eq!(q.floors[0].annots.notes[0].items.len(), 2);
    }

    #[test]
    fn convert_text_to_note_keeps_the_text_as_the_schedule_text() {
        let mut p = project();
        let t = p.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: Point::new(10.0, 20.0),
                text: "Verify".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let id = p
            .convert_text_to_note(0, t, "Electrical Note", &Note::default())
            .unwrap();
        assert!(p.floors[0].cad.iter().all(|c| c.id != t));
        let n = &p.floors[0].annots.notes[0];
        assert_eq!(
            (n.text.as_str(), n.note_type.as_str(), n.center),
            ("Verify", "Electrical Note", Point::new(10.0, 20.0))
        );
        assert_eq!(n.items[0], id);
        // A note is not converted again.
        assert!(p
            .convert_text_to_note(0, id, "General Note", &Note::default())
            .is_none());
    }

    #[test]
    fn removing_a_record_removes_its_objects_and_a_deleted_object_drops_the_record() {
        let mut p = project();
        let id = p.add_callout(0, Callout::default());
        p.remove_annot(0, AnnotRef::Callout(0));
        assert!(p.floors[0].cad.is_empty() && p.floors[0].annots.is_empty());
        let id2 = p.add_callout(0, Callout::default());
        // Deleting every object of the group (what the Delete key does) ends
        // the record on the next sync.
        let items = p.floors[0].annots.callouts[0].items.clone();
        for i in items {
            p.remove_cad(0, i);
        }
        assert!(p.sync_annotations());
        assert!(p.floors[0].annots.callouts.is_empty());
        let _ = (id, id2);
    }

    #[test]
    fn records_round_trip_through_json_and_an_empty_floor_writes_nothing() {
        let mut p = project();
        let mut c = Callout::default();
        c.section.on = true;
        c.link = Some(ViewLink {
            kind: ViewKind::Camera,
            id: 4,
            name: "x".into(),
        });
        p.add_callout(0, c);
        p.add_marker(0, Marker::default());
        p.add_note(0, Note::default());
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(back.floors[0].annots, p.floors[0].annots);
        let empty = serde_json::to_string(&Project::new("e")).unwrap();
        assert!(!empty.contains("\"annots\""));
        // Old files without the slot load.
        let v: Project = serde_json::from_str(&empty).unwrap();
        assert!(v.floors[0].annots.is_empty());
        assert_eq!(v.annot_defaults.saved_name, DEFAULT_SAVED_NAME);
    }

    #[test]
    fn filled_shapes_carry_a_fill_with_opacity_from_the_transparency() {
        let c = Callout {
            filled: true,
            fill_color: Some([10, 20, 30]),
            transparency: 50,
            ..Callout::default()
        };
        let g = callout_items(&c, &Vars::default());
        let f = g.items[0].1.fill.clone().unwrap();
        assert_eq!(f.color, [10, 20, 30]);
        assert!((i32::from(f.opacity) - 128).abs() <= 1);
    }

    #[test]
    fn macros_expand_from_the_link_and_number() {
        let v = Vars {
            link: Some(LinkInfo {
                valid: true,
                view_name: "V".into(),
                callout_label: "2".into(),
                page_label: "A-4".into(),
                ..LinkInfo::default()
            }),
            number: Some(9),
            ..Vars::default()
        };
        assert_eq!(expand_macros("%linked_view_name% %linked_view_layout_page_label% %referenced_view_callout_label% %layout_page_label% %automatic_label% %simple_schedule_number%", &v), "V A-4 2 A-4 2 9");
        let broken = Vars {
            link: Some(LinkInfo::default()),
            ..Vars::default()
        };
        assert_eq!(expand_macros("[%linked_view_name%]", &broken), "[]");
        assert_eq!(expand_macros("100%", &v), "100%");
    }
}
