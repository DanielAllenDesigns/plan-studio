//! Text and annotation tools (TXT-1..TXT-15 in
//! `docs/parity/dimensions-text-cad.md`).
//!
//! Every annotation is a CAD item on the `Text` layer (TXT-11): text is a
//! `CadItem::Text`, leaders and callout shapes are polylines and circles
//! grouped with their text so they select together.
//!
//! * Text (TXT-1): click places the anchor (the text's bottom-left), typing
//!   fills it, Enter commits, a click elsewhere commits and starts the next
//!   text, Esc cancels. Clicking existing text edits it in place. The height
//!   is `defaults.text.height` in plan inches; when the Text layer's style
//!   holds a printed size, the text is stored at the style's character height
//!   and drawn (and picked) at the size on paper for the sheet scale (TXT-2).
//! * Rich Text (TXT-4): Enter adds a line, Tab commits; the size scale is
//!   stored in the text height, bold/italic/underline are kept per session
//!   ([`TextTool::style_of`]) because the model has no style fields.
//! * Leader Line (TXT-5): click the arrow tip and the bends, double-click or
//!   Enter ends. Text Line with Arrow (TXT-6) then asks for the text.
//! * Callout (TXT-7, TXT-49..51), Marker (TXT-8, TXT-52, TXT-53) and Note
//!   (TXT-9, TXT-54, TXT-55): a click opens the Callout, Marker or Note
//!   Specification (`dialogs::text::annot`) filled from the Saved Defaults;
//!   OK places the object (`plan_core::callout`: a record plus the grouped
//!   CAD objects it draws as). Placed ones have edit handles
//!   ([`annot_handles`]) and the Edit toolbar buttons of [`edit_actions`].
//!
//! * Rich Text keeps its runs: inline `<b> <i> <u> <size=1.5> <color=#RRGGBB>`
//!   markup typed in the text (and the B / I / U buttons) become
//!   `RichRun`s stored with the text (`CadAttrs::runs`); the text item keeps
//!   the plain words.
//! * Text macros: `%room.name%`, `%plan.date%`, `%floor%` and the user's own
//!   macros expand when text is placed or edited (Text Macro Management).
//! * Notes take their number from the order they were placed in, per note
//!   type (Note Type Management); the Note Schedule reads them back.
//!
//! The shell sends typed characters to a tool only while `cx.temp.editing`
//! is set, so the tool raises that flag while text is being typed
//! ([`set_typing`]).

use super::cad::{add_cad_items, arrowhead, set_typing, OptionStrip, StripButton};
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::text::annot::{self, AnnotKind};
use crate::dialogs::text::manage::{MacroDialog, NoteTypeDialog, StyleOp, TextStyleDialog};
use crate::dialogs::Outcome;
use crate::editor::selection::{cad_by_id, cad_distance, hit_test};
use crate::editor::snap::SnapResult;
use crate::editor::{render, Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Rect, Stroke};
use plan_core::callout::{CalloutShape, MarkerKind};
use plan_core::cad::{CadItem, TEXT_WIDTH_FACTOR};
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::text_box::{layout, TextBox};
use plan_core::text_styles::{
    expand_macros, runs_from_markup, runs_plain, runs_to_markup, MacroContext, RichRun,
};
use plan_core::Id;
use std::collections::HashMap;

/// Layer of text and annotations (TXT-11).
pub const TEXT_LAYER: &str = "Text";

/// The layer the Text tools draw on: the active layer chosen for them in
/// Tools > Layer Settings, else [`TEXT_LAYER`].
fn text_layer(cx: &EditorContext) -> String {
    cx.project.layers.tool_layer("text")
}
/// Prefix of note texts; see [`note_text`].
pub const NOTE_PREFIX: &str = "Note ";
/// The note type every plan starts with.
pub const GENERAL_NOTE: &str = "General Note";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextMode {
    Text,
    RichText,
    LeaderLine,
    ArrowLine,
    Callout,
    Marker,
    Note,
    /// Opens Note Type Management.
    NoteTypes,
    /// Opens Text Macro Management.
    Macros,
    /// Opens Text Style Management (rename and remove styles).
    TextStyles,
}

impl TextMode {
    pub const ALL: [TextMode; 10] = [
        TextMode::Text,
        TextMode::RichText,
        TextMode::LeaderLine,
        TextMode::ArrowLine,
        TextMode::Callout,
        TextMode::Marker,
        TextMode::Note,
        TextMode::NoteTypes,
        TextMode::Macros,
        TextMode::TextStyles,
    ];

    /// Chief's name from the Text Tools flyout.
    pub fn name(self) -> &'static str {
        match self {
            TextMode::Text => "Text",
            TextMode::RichText => "Rich Text",
            TextMode::LeaderLine => "Leader Line",
            TextMode::ArrowLine => "Text Line with Arrow",
            TextMode::Callout => "Callout",
            TextMode::Marker => "Marker",
            TextMode::Note => "Note",
            TextMode::NoteTypes => "Note Type Management",
            TextMode::Macros => "Text Macro Management",
            TextMode::TextStyles => "Text Style Management",
        }
    }

    /// Opens a dialog when picked instead of drawing.
    fn is_command(self) -> bool {
        matches!(
            self,
            TextMode::NoteTypes | TextMode::Macros | TextMode::TextStyles
        )
    }

    pub fn from_name(name: &str) -> Option<TextMode> {
        TextMode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }

    fn short(self) -> &'static str {
        match self {
            TextMode::ArrowLine => "Line + Text",
            TextMode::NoteTypes => "Note Types",
            TextMode::Macros => "Macros",
            TextMode::TextStyles => "Styles",
            m => m.name(),
        }
    }

    fn hint(self) -> &'static str {
        match self {
            TextMode::Text => "Text: click to place text, type, Enter to finish",
            TextMode::RichText => "Rich Text: click, type (Enter adds a line), Tab to finish",
            TextMode::LeaderLine => "Leader Line: click the arrow tip and the bends; Enter or double-click ends",
            TextMode::ArrowLine => "Text Line with Arrow: click the arrow tip and the bends; Enter ends, then type the text",
            TextMode::Callout => "Callout: click to place a callout; the Callout Specification opens",
            TextMode::Marker => "Marker: click to place a marker; the Marker Specification opens",
            TextMode::Note => "Note: click to place a note; the Note Specification opens",
            TextMode::NoteTypes => "Note Type Management: add note types and their label prefixes",
            TextMode::Macros => {
                "Text Macro Management: define %macros% that expand when text is placed"
            }
            TextMode::TextStyles => {
                "Text Style Management: rename or remove text styles; users follow the change"
            }
        }
    }
}

/// The style of a Rich Text box. Only the size scale reaches the model (as
/// the text height); the rest is kept for the session.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RichStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub size_scale: f64,
    /// The rest of the Rich Text Edit Bar (TXT-29): strikethrough,
    /// uppercase, a font family and colour from the strip's short lists,
    /// the box alignment and a bullet or number list. They reach the runs
    /// and the text box when the text is placed.
    pub strike: bool,
    pub upper: bool,
    pub font: Option<&'static str>,
    pub color: Option<[u8; 3]>,
    pub halign: plan_core::text_box::HAlign,
    pub list: crate::dialogs::text::editbar::ListKind,
}

impl Default for RichStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            size_scale: 1.0,
            strike: false,
            upper: false,
            font: None,
            color: None,
            halign: plan_core::text_box::HAlign::Left,
            list: crate::dialogs::text::editbar::ListKind::None,
        }
    }
}

/// The font families the Rich Text strip cycles through.
pub const STRIP_FONTS: [&str; 5] = ["Avenir", "Arial", "Helvetica", "Times New Roman", "Georgia"];
/// The colours the Rich Text strip cycles through.
pub const STRIP_COLORS: [[u8; 3]; 5] = [
    [0, 0, 0],
    [190, 30, 30],
    [20, 80, 170],
    [30, 120, 50],
    [150, 90, 20],
];

// ----- pure builders -----

/// Width of `text` in plan inches (longest line).
pub fn text_width(text: &str, height: f64) -> f64 {
    text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64 * height * TEXT_WIDTH_FACTOR
}

/// The text of note number `n`.
pub fn note_text(n: u32, body: &str) -> String {
    format!("{NOTE_PREFIX}{n}: {body}")
}

/// Seconds since the Unix epoch (0 when the clock is unavailable).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The note number of a note text (for a Note schedule).
pub fn note_number(text: &str) -> Option<u32> {
    let rest = text.strip_prefix(NOTE_PREFIX)?;
    let (n, _) = rest.split_once(':')?;
    n.trim().parse().ok()
}

/// A leader: an open polyline from the arrow tip `pts[0]` through the bends,
/// with a closed triangle for the arrowhead (TXT-5).
pub fn leader_items(pts: &[Point], arrow: f64) -> Vec<CadItem> {
    if pts.len() < 2 {
        return Vec::new();
    }
    vec![
        CadItem::Polyline {
            points: pts.to_vec(),
            closed: false,
        },
        CadItem::Polyline {
            points: arrowhead(pts[0], pts[1], arrow),
            closed: true,
        },
    ]
}

/// A numbered marker: a circle with the number centered in it (TXT-8).
pub fn marker_items(center: Point, number: u32, height: f64) -> Vec<CadItem> {
    let text = number.to_string();
    let w = text_width(&text, height);
    vec![
        CadItem::Circle {
            center,
            radius: height * 1.2,
        },
        CadItem::Text {
            pos: Point::new(center.x - w * 0.5, center.y - height * 0.5),
            text,
            height,
            angle: 0.0,
        },
    ]
}

// ----- the tool -----

pub struct TextTool {
    mode: TextMode,
    rich: RichStyle,
    /// Where the text being typed is anchored (bottom-left, or the center of
    /// a callout); `Some` while typing.
    anchor: Option<Point>,
    buf: String,
    /// An existing text being edited.
    editing: Option<Id>,
    /// Leader / callout points clicked so far.
    pts: Vec<Point>,
    /// A finished leader waiting for its text (Text Line with Arrow).
    leader: Vec<Point>,
    hover: Option<Point>,
    styles: HashMap<Id, RichStyle>,
    strip: OptionStrip,
    /// The note type new notes are made with (Note Type Management).
    note_type: String,
    /// The management dialog a command mode opened.
    dialog: Option<TextUi>,
    /// Open the mode's dialog on the next frame.
    pending: bool,
    /// A new text's press: where it landed on screen and in the plan. A drag
    /// from it defines the text box (TXT-1).
    press: Option<(egui::Pos2, Point)>,
    /// The opposite corner of the box being dragged.
    box_drag: Option<Point>,
    /// The box a finished drag made, and the plan y of its top edge: the box
    /// keeps its top where it was dragged as the text grows (TXT-13).
    text_box: Option<(TextBox, f64)>,
    /// The vertical guide of a left-edge alignment (TXT-10): from the text
    /// aligned to, to the anchor.
    align_guide: Option<(Point, Point)>,
}

/// The dialogs of the two management commands.
enum TextUi {
    NoteTypes(Box<NoteTypeDialog>),
    Macros(Box<MacroDialog>),
    Styles(Box<TextStyleDialog>),
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            mode: TextMode::Text,
            rich: RichStyle::default(),
            anchor: None,
            buf: String::new(),
            editing: None,
            pts: Vec::new(),
            leader: Vec::new(),
            hover: None,
            styles: HashMap::new(),
            strip: OptionStrip::default(),
            note_type: GENERAL_NOTE.to_string(),
            dialog: None,
            pending: false,
            press: None,
            box_drag: None,
            text_box: None,
            align_guide: None,
        }
    }
}

const BTN_NOTE_TYPE: u16 = 120;
const BTN_MARKER_KIND: u16 = 121;
const BTN_STRIKE: u16 = 115;
const BTN_UPPER: u16 = 116;
const BTN_FONT: u16 = 117;
const BTN_COLOR: u16 = 118;
const BTN_ALIGN: u16 = 119;
const BTN_LIST: u16 = 122;
const BTN_LINK: u16 = 123;
const BTN_SHAPE: u16 = 100;
const BTN_BOLD: u16 = 110;
const BTN_ITALIC: u16 = 111;
const BTN_UNDERLINE: u16 = 112;
const BTN_SMALLER: u16 = 113;
const BTN_LARGER: u16 = 114;

impl TextTool {
    pub fn mode(&self) -> TextMode {
        self.mode
    }

    /// Switches the variant (a flyout entry), dropping work in progress.
    pub fn set_mode(&mut self, mode: TextMode) {
        self.mode = mode;
        self.reset_state();
        self.pending = mode.is_command();
    }

    pub fn set_mode_by_name(&mut self, name: &str) -> bool {
        match TextMode::from_name(name) {
            Some(m) => {
                self.set_mode(m);
                true
            }
            None => false,
        }
    }

    pub fn rich_style(&self) -> RichStyle {
        self.rich
    }

    /// The Rich Text style a text was created with this session.
    pub fn style_of(&self, id: Id) -> Option<RichStyle> {
        self.styles.get(&id).copied()
    }

    /// The text typed so far, while typing.
    pub fn typed(&self) -> Option<&str> {
        self.anchor.map(|_| self.buf.as_str())
    }

    fn typing(&self) -> bool {
        self.anchor.is_some()
    }

    fn reset_state(&mut self) {
        self.anchor = None;
        self.buf.clear();
        self.editing = None;
        self.pts.clear();
        self.leader.clear();
        self.press = None;
        self.box_drag = None;
        self.text_box = None;
        self.align_guide = None;
    }

    fn cancel(&mut self, cx: &mut EditorContext) {
        self.reset_state();
        set_typing(cx, false);
        cx.readout = None;
    }

    /// Text height in plan inches: the defaults' height, scaled in Rich Text
    /// (TXT-2).
    ///
    /// A Text layer whose style holds a printed size places text at the
    /// style's own character height, which draws at that size on paper at any
    /// scale ([`Self::drawn_height`]).
    fn height(&self, cx: &EditorContext) -> f64 {
        // Text Defaults / Rich Text Defaults (Default Settings > Text).
        let own = if self.mode == TextMode::RichText {
            cx.project.annot_defaults.rich.height
        } else {
            cx.project.annot_defaults.text.height
        };
        let h = if own > 0.0 { own } else { cx.defaults.text.height };
        let h = if h > 0.0 { h } else { 6.0 };
        let h = cx
            .project
            .text_styles
            .placed_height(cx.layers(), &text_layer(cx), None, h);
        if self.mode == TextMode::RichText {
            h * self.rich.size_scale
        } else {
            h
        }
    }

    /// The plan height a text of the stored `height` is drawn at on the
    /// sheet: the size on paper for a printed-size style (see
    /// [`plan_core::text_styles::TextStyles::drawn_height`]).
    fn drawn_height(cx: &EditorContext, height: f64) -> f64 {
        cx.project.text_styles.drawn_height(
            cx.layers(),
            &text_layer(cx),
            None,
            height,
            cx.sheet.scale.inches_per_foot(),
        )
    }

    fn drawn_height_of(&self, cx: &EditorContext, height: f64) -> f64 {
        Self::drawn_height(cx, height)
    }

    fn arrow_size(&self, cx: &EditorContext) -> f64 {
        (self.height(cx) * 1.2).max(3.0)
    }

    /// `text` with its `%macros%` expanded for a text placed at `at`.
    pub fn expand(cx: &EditorContext, text: &str, at: Point) -> String {
        if !text.contains('%') {
            return text.to_string();
        }
        let room = cx
            .rooms
            .iter()
            .find(|r| point_in_polygon(at, &r.inner_polygon) || point_in_polygon(at, &r.polygon));
        let floor = cx.floor();
        let ctx = MacroContext {
            room_name: room.map(|r| cx.room_name(r)).unwrap_or_default(),
            room_number: String::new(),
            room_area: room
                .map(|r| format!("{} sq ft", r.interior_area_sq_ft().round()))
                .unwrap_or_default(),
            plan_name: cx.project.name.clone(),
            plan_date: plan_core::text_styles::date_string(unix_now()),
            floor_name: floor.name.clone(),
            floor_number: cx.floor + 1,
            floor_count: cx.project.floors.len(),
            ceiling_height: cx.fmt_dim(floor.ceiling_height),
        };
        expand_macros(text, &ctx, &cx.project.text_macros())
    }

    /// The runs of Rich Text typed with inline markup and the B / I / U
    /// buttons, and their plain text.
    fn rich_runs(&self, text: &str) -> (Vec<RichRun>, String) {
        let mut runs = runs_from_markup(text);
        for r in &mut runs {
            r.bold |= self.rich.bold;
            r.italic |= self.rich.italic;
            r.underline |= self.rich.underline;
            r.strike |= self.rich.strike;
            r.upper |= self.rich.upper;
            if r.font.is_none() {
                r.font = self.rich.font.map(str::to_string);
            }
            if r.color.is_none() {
                r.color = self.rich.color.filter(|c| *c != [0, 0, 0]);
            }
        }
        let plain = runs_plain(&runs);
        (runs, plain)
    }

    /// Where the text being typed sits: its anchor, or for a text box the
    /// lower left corner that keeps the dragged top edge (TXT-13).
    fn text_pos(&self, anchor: Point, text: &str, runs: &[RichRun], height: f64) -> Point {
        match self.text_box {
            Some((tb, top)) => Point::new(anchor.x, top - layout(text, runs, height, &tb).height),
            None => anchor,
        }
    }

    fn begin_typing(&mut self, cx: &mut EditorContext, anchor: Point) {
        self.anchor = Some(anchor);
        self.buf.clear();
        set_typing(cx, true);
        cx.status = "Type the text; Enter finishes, Esc cancels".into();
    }

    /// The items the current state would create, for the preview and commit.
    fn pending_items(&self, cx: &EditorContext, extra: Option<Point>) -> Vec<CadItem> {
        let height = self.height(cx);
        match self.mode {
            TextMode::Text | TextMode::RichText => {
                let Some(a) = self.anchor else {
                    return Vec::new();
                };
                let text = if self.mode == TextMode::RichText {
                    runs_plain(&runs_from_markup(&self.buf))
                } else {
                    self.buf.clone()
                };
                let runs = if self.mode == TextMode::RichText {
                    runs_from_markup(&self.buf)
                } else {
                    Vec::new()
                };
                vec![CadItem::Text {
                    pos: self.text_pos(a, &text, &runs, self.drawn_height_of(cx, height)),
                    text,
                    height,
                    angle: 0.0,
                }]
            }
            TextMode::LeaderLine | TextMode::ArrowLine => {
                let mut pts = if self.anchor.is_some() {
                    self.leader.clone()
                } else {
                    self.pts.clone()
                };
                if self.anchor.is_none() {
                    pts.extend(extra);
                }
                let mut items = leader_items(&pts, self.arrow_size(cx));
                if let (Some(a), true) = (self.anchor, !self.buf.is_empty()) {
                    items.push(CadItem::Text {
                        pos: a,
                        text: self.buf.clone(),
                        height,
                        angle: 0.0,
                    });
                }
                items
            }
            TextMode::Callout | TextMode::Marker | TextMode::Note => extra
                .map(|c| annot_ghost(cx, mode_kind(self.mode), c))
                .unwrap_or_default(),
            TextMode::NoteTypes | TextMode::Macros | TextMode::TextStyles => Vec::new(),
        }
    }

    /// Finishes the text being typed (Enter, Tab, a click elsewhere).
    fn commit_typing(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(anchor) = self.anchor else {
            return ToolResult::ignored();
        };
        let height = self.height(cx);
        let mut raw = self.buf.trim_end().to_string();
        let editing = self.editing;
        let mode = self.mode;
        // The strip's bullets and numbering (Paragraph Options).
        if mode == TextMode::RichText && editing.is_none() && self.rich.list != Default::default() {
            let n = raw.chars().count();
            raw = crate::dialogs::text::editbar::set_list(&raw, (0, n), self.rich.list).0;
        }
        // Rich Text keeps its runs; every text expands its %macros%.
        let rich_edit =
            editing.is_some_and(|id| cx.floor().cad_attrs(id).is_some_and(|a| !a.runs.is_empty()));
        let (runs, plain) = if mode == TextMode::RichText || rich_edit {
            self.rich_runs(&raw)
        } else {
            (Vec::new(), raw.clone())
        };
        let text = Self::expand(cx, &plain, anchor);
        let runs: Vec<RichRun> = if runs.iter().any(|r| r.text.contains('%')) {
            // Expanding changed the words; keep the formats by run.
            runs.into_iter()
                .map(|mut r| {
                    r.text = Self::expand(cx, &r.text, anchor);
                    r
                })
                .collect()
        } else {
            runs
        };
        let leader = std::mem::take(&mut self.leader);
        let boxed = self.text_box.take();
        self.press = None;
        self.box_drag = None;
        self.align_guide = None;
        self.anchor = None;
        self.buf.clear();
        self.editing = None;
        self.pts.clear();
        set_typing(cx, false);
        cx.status.clear();

        if let Some(id) = editing {
            return self.finish_edit(cx, id, text, runs);
        }
        let (items, label) = match mode {
            TextMode::Text | TextMode::RichText => {
                if text.trim().is_empty() {
                    return ToolResult::consumed();
                }
                // A dragged text box keeps its top edge where it was dragged.
                let pos = match boxed {
                    Some((tb, top)) => Point::new(
                        anchor.x,
                        top - layout(&text, &runs, Self::drawn_height(cx, height), &tb).height,
                    ),
                    None => anchor,
                };
                (
                    vec![CadItem::Text {
                        pos,
                        text,
                        height,
                        angle: 0.0,
                    }],
                    "Place Text",
                )
            }
            TextMode::ArrowLine => {
                let mut items = leader_items(&leader, self.arrow_size(cx));
                if !text.trim().is_empty() {
                    items.push(CadItem::Text {
                        pos: anchor,
                        text,
                        height,
                        angle: 0.0,
                    });
                }
                (items, "Text Line with Arrow")
            }
            _ => return ToolResult::consumed(),
        };
        let rich = mode == TextMode::RichText;
        let layer = text_layer(cx);
        match add_cad_items(cx, &layer, items, label) {
            Some(ids) => {
                if rich {
                    self.styles.insert(ids[0], self.rich);
                    Self::store_runs(cx, ids[0], runs);
                }
                if let (Some((tb, _)), TextMode::Text | TextMode::RichText) = (boxed, mode) {
                    let fl = cx.floor;
                    cx.project.edit_cad_attrs(fl, ids[0], |a| a.text_box = tb);
                }
                if matches!(mode, TextMode::Text | TextMode::RichText) {
                    let spec = if rich {
                        cx.project.annot_defaults.rich.clone()
                    } else {
                        cx.project.annot_defaults.text.clone()
                    };
                    if spec.style.is_some() || (boxed.is_none() && !spec.text_box.is_plain()) {
                        let fl = cx.floor;
                        let own_box = boxed.is_none();
                        cx.project.edit_cad_attrs(fl, ids[0], |a| {
                            if a.text_style.is_none() {
                                a.text_style = spec.style.clone();
                            }
                            if own_box {
                                a.text_box = spec.text_box;
                            }
                        });
                    }
                }
                if rich && self.rich.halign != plan_core::text_box::HAlign::Left {
                    let (fl, h) = (cx.floor, self.rich.halign);
                    cx.project.edit_cad_attrs(fl, ids[0], |a| a.text_box.halign = h);
                }
                ToolResult::committed(label)
            }
            None => ToolResult::consumed(),
        }
    }

    /// The runs worth keeping: none when no run carries a format (a single
    /// plain run is just the text).
    fn normalized_runs(runs: Vec<RichRun>) -> Vec<RichRun> {
        let formatted = runs.iter().any(|r| {
            r.bold
                || r.italic
                || r.underline
                || r.strike
                || r.upper
                || (r.scale - 1.0).abs() > 1e-9
                || r.color.is_some()
                || r.font.is_some()
                || r.link.is_some()
        });
        if formatted {
            runs
        } else {
            Vec::new()
        }
    }

    /// Keeps the runs with a rich text object when they carry any format.
    fn store_runs(cx: &mut EditorContext, id: Id, runs: Vec<RichRun>) {
        let fl = cx.floor;
        let runs = Self::normalized_runs(runs);
        cx.project.edit_cad_attrs(fl, id, |a| a.runs = runs);
    }

    /// Writes the edited text back; empty text deletes the object.
    fn finish_edit(
        &mut self,
        cx: &mut EditorContext,
        id: Id,
        text: String,
        runs: Vec<RichRun>,
    ) -> ToolResult {
        if !cx.check_unlocked(ObjectRef::Cad(id)) {
            return ToolResult::consumed();
        }
        let old_runs = cx.floor().cad_attrs(id).map(|a| a.runs).unwrap_or_default();
        let runs = Self::normalized_runs(runs);
        let same = matches!(cad_by_id(cx.floor(), id).map(|c| &c.item),
            Some(CadItem::Text { text: t, .. }) if *t == text)
            && runs == old_runs;
        if same {
            return ToolResult::consumed();
        }
        cx.begin_change("Edit Text");
        let fl = cx.floor;
        // A box grows and shrinks from its top edge (TXT-13).
        let tb = cx
            .floor()
            .cad_attrs(id)
            .map(|a| a.text_box)
            .filter(TextBox::is_boxed);
        if text.trim().is_empty() {
            Self::remove_text(cx, id);
            cx.selection.clear();
        } else if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
            if let CadItem::Text {
                text: t,
                pos,
                height,
                angle,
            } = &mut c.item
            {
                if let (Some(tb), true) = (tb, angle.abs() < 1e-9) {
                    let old = layout(t, &old_runs, *height, &tb).height;
                    let new = layout(&text, &runs, *height, &tb).height;
                    pos.y += old - new;
                }
                *t = text;
            }
            if runs != old_runs {
                Self::store_runs(cx, id, runs);
            }
        }
        cx.mark_dirty();
        ToolResult::committed("Edit Text")
    }

    fn remove_text(cx: &mut EditorContext, id: Id) {
        let fl = cx.floor;
        cx.project.remove_cad(fl, id);
        cx.project.prune_cad_data(fl);
    }

    /// Ends a leader: Leader Line commits; Text Line with Arrow asks for the
    /// text.
    fn finish_leader(&mut self, cx: &mut EditorContext) -> ToolResult {
        if self.pts.len() < 2 {
            self.cancel(cx);
            return ToolResult::consumed();
        }
        let pts = std::mem::take(&mut self.pts);
        if self.mode == TextMode::ArrowLine {
            let last = *pts.last().expect("points");
            self.leader = pts;
            // The text sits just beyond the last point.
            self.begin_typing(cx, last.add(Point::new(3.0, 3.0)));
            return ToolResult::consumed();
        }
        let items = leader_items(&pts, self.arrow_size(cx));
        let layer = text_layer(cx);
        match add_cad_items(cx, &layer, items, "Leader Line") {
            Some(_) => {
                cx.readout = None;
                ToolResult::committed("Leader Line")
            }
            None => ToolResult::consumed(),
        }
    }

    /// The text object under `p`, for editing it in place.
    ///
    /// Text in a printed-size style is picked by the box it is drawn in at
    /// the sheet's scale, not by its stored height.
    fn text_under(cx: &EditorContext, p: Point) -> Option<Id> {
        let tol = cx.pick_tol() * 0.5;
        let attrs = cx.floor().cad_attr_map();
        cx.floor().cad.iter().rev().find_map(|c| {
            if !matches!(c.item, CadItem::Text { .. }) || !cx.layers().is_visible(&c.layer) {
                return None;
            }
            let drawn = render::printed_text_object(cx, c, attrs.get(&c.id));
            let item = drawn.as_ref().map_or(&c.item, |o| &o.item);
            // A text with a box is picked by its box and frame.
            let dist = match attrs
                .get(&c.id)
                .and_then(|a| plan_core::text_box::placed(item, a))
            {
                Some(pb) => pb.distance(p),
                None => cad_distance(item, p),
            };
            (dist <= tol).then_some(c.id)
        })
    }

    fn strip_items(&self, cx: &EditorContext) -> Vec<StripButton> {
        let mut v: Vec<StripButton> = TextMode::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| StripButton::new(m.short(), i as u16, *m == self.mode))
            .collect();
        let d = &cx.project.annot_defaults;
        if self.mode == TextMode::Callout {
            v.push(StripButton::new(
                format!("Shape: {}", d.callout.shape.label()),
                BTN_SHAPE,
                true,
            ));
        }
        if self.mode == TextMode::Marker {
            v.push(StripButton::new(
                format!("Type: {}", d.marker.kind.label()),
                BTN_MARKER_KIND,
                true,
            ));
        }
        if self.mode == TextMode::Note {
            v.push(StripButton::new(
                format!("Type: {}", self.note_type),
                BTN_NOTE_TYPE,
                true,
            ));
        }
        if self.mode == TextMode::RichText {
            v.push(StripButton::new("B", BTN_BOLD, self.rich.bold));
            v.push(StripButton::new("I", BTN_ITALIC, self.rich.italic));
            v.push(StripButton::new("U", BTN_UNDERLINE, self.rich.underline));
            v.push(StripButton::new("S", BTN_STRIKE, self.rich.strike));
            v.push(StripButton::new("AA", BTN_UPPER, self.rich.upper));
            v.push(StripButton::new(
                format!("Font: {}", self.rich.font.unwrap_or("Style's")),
                BTN_FONT,
                self.rich.font.is_some(),
            ));
            v.push(StripButton::new("Color", BTN_COLOR, self.rich.color.is_some()));
            v.push(StripButton::new(
                format!("Align: {}", self.rich.halign.label()),
                BTN_ALIGN,
                true,
            ));
            v.push(StripButton::new(
                format!("List: {}", self.rich.list.label()),
                BTN_LIST,
                self.rich.list != Default::default(),
            ));
            v.push(StripButton::new("Link", BTN_LINK, false));
            v.push(StripButton::new("A\u{2212}", BTN_SMALLER, false));
            v.push(StripButton::new(
                format!("{:.0}%", self.rich.size_scale * 100.0),
                BTN_LARGER + 1,
                true,
            ));
            v.push(StripButton::new("A+", BTN_LARGER, false));
        }
        v
    }

    /// Cycles the active note type.
    fn next_note_type(&mut self, cx: &EditorContext) {
        let types = cx.project.note_types();
        let names: Vec<&str> = types.types.iter().map(|t| t.name.as_str()).collect();
        let at = names.iter().position(|n| *n == self.note_type);
        let next = at.map_or(0, |i| (i + 1) % names.len().max(1));
        if let Some(n) = names.get(next) {
            self.note_type = (*n).to_string();
        }
    }

    /// Shows the open management dialog and applies its result.
    fn frame_dialogs(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        let Some(mut ui) = self.dialog.take() else {
            return;
        };
        let out = match &mut ui {
            TextUi::NoteTypes(d) => d.show(ctx),
            TextUi::Macros(d) => d.show(ctx),
            TextUi::Styles(d) => d.show(ctx),
        };
        self.dialog_outcome(cx, ui, out);
    }

    /// What closing (or not closing) a management dialog does.
    fn dialog_outcome(&mut self, cx: &mut EditorContext, ui: TextUi, out: Outcome) {
        match out {
            Outcome::Open => {
                self.dialog = Some(ui);
                return;
            }
            Outcome::Cancel => {}
            Outcome::Ok => match ui {
                TextUi::NoteTypes(d) => {
                    if *d.draft() != cx.project.note_types() {
                        cx.begin_change("Change Note Types");
                        cx.project.set_note_types(d.draft());
                        cx.mark_dirty();
                        if cx.project.note_types().get(&self.note_type).is_none() {
                            self.note_type = GENERAL_NOTE.to_string();
                        }
                    }
                }
                TextUi::Macros(d) => {
                    if *d.draft() != cx.project.text_macros() {
                        cx.begin_change("Change Text Macros");
                        cx.project.set_text_macros(d.draft());
                        cx.mark_dirty();
                    }
                }
                TextUi::Styles(d) => {
                    if !d.ops().is_empty() {
                        cx.begin_change("Change Text Styles");
                        if StyleOp::apply_all(d.ops(), &mut cx.project) > 0 {
                            cx.mark_dirty();
                        } else {
                            cx.cancel_change();
                        }
                    }
                }
            },
        }
        cx.requests.push(EditorRequest::SetTool(ToolId::Select));
    }

    /// Opens the dialog of a management mode.
    fn open_dialog(&mut self, cx: &mut EditorContext) {
        self.dialog = match self.mode {
            TextMode::NoteTypes => {
                let styles = cx
                    .project
                    .text_styles
                    .names()
                    .into_iter()
                    .map(str::to_string)
                    .collect();
                Some(TextUi::NoteTypes(Box::new(NoteTypeDialog::new(
                    cx.project.note_types(),
                    styles,
                ))))
            }
            TextMode::Macros => Some(TextUi::Macros(Box::new(MacroDialog::new(
                cx.project.text_macros(),
            )))),
            TextMode::TextStyles => Some(TextUi::Styles(Box::new(TextStyleDialog::new(
                cx.project
                    .text_styles
                    .names()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            )))),
            _ => None,
        };
    }

    fn strip_click(&mut self, cx: &mut EditorContext, id: u16) -> ToolResult {
        if let Some(m) = TextMode::ALL.get(id as usize).copied() {
            self.cancel(cx);
            self.set_mode(m);
            cx.status = m.hint().into();
            return ToolResult::consumed();
        }
        match id {
            BTN_SHAPE => {
                // Cycles the shape new callouts start with (Saved Defaults).
                let d = &mut cx.project.annot_defaults.callout;
                let all = CalloutShape::TEN;
                let at = all.iter().position(|s| *s == d.shape).map_or(0, |i| i + 1);
                d.shape = all[at % all.len()];
            }
            BTN_MARKER_KIND => {
                let d = &mut cx.project.annot_defaults.marker;
                let all = MarkerKind::ALL;
                let at = all.iter().position(|k| *k == d.kind).map_or(0, |i| i + 1);
                d.kind = all[at % all.len()];
            }
            BTN_NOTE_TYPE => self.next_note_type(cx),
            BTN_BOLD => self.rich.bold = !self.rich.bold,
            BTN_ITALIC => self.rich.italic = !self.rich.italic,
            BTN_UNDERLINE => self.rich.underline = !self.rich.underline,
            BTN_STRIKE => self.rich.strike = !self.rich.strike,
            BTN_UPPER => self.rich.upper = !self.rich.upper,
            BTN_FONT => {
                let at = STRIP_FONTS.iter().position(|f| Some(*f) == self.rich.font);
                self.rich.font = match at {
                    None => Some(STRIP_FONTS[0]),
                    Some(i) if i + 1 < STRIP_FONTS.len() => Some(STRIP_FONTS[i + 1]),
                    Some(_) => None,
                };
            }
            BTN_COLOR => {
                let at = STRIP_COLORS.iter().position(|c| Some(*c) == self.rich.color);
                self.rich.color = Some(STRIP_COLORS[at.map_or(1, |i| (i + 1) % STRIP_COLORS.len())]);
            }
            BTN_ALIGN => {
                use plan_core::text_box::HAlign;
                self.rich.halign = match self.rich.halign {
                    HAlign::Left => HAlign::Center,
                    HAlign::Center => HAlign::Right,
                    HAlign::Right => HAlign::Left,
                };
            }
            BTN_LIST => {
                use crate::dialogs::text::editbar::ListKind;
                let all = ListKind::ALL;
                let at = all.iter().position(|k| *k == self.rich.list).unwrap_or(0);
                self.rich.list = all[(at + 1) % all.len()];
            }
            BTN_LINK => {
                // Inserts a hyperlink to fill in at the end of the typed text.
                if self.typing() {
                    self.buf.push_str(
                        "<link=https://example.com><u><color=#0000EE>link</color></u></link>",
                    );
                }
            }
            BTN_SMALLER => self.rich.size_scale = (self.rich.size_scale - 0.25).max(0.5),
            BTN_LARGER => self.rich.size_scale = (self.rich.size_scale + 0.25).min(4.0),
            _ => {}
        }
        ToolResult::consumed()
    }
}

impl Tool for TextTool {
    fn id(&self) -> ToolId {
        ToolId::Text
    }

    fn name(&self) -> &'static str {
        self.mode.name()
    }

    fn hint(&self) -> String {
        self.mode.hint().into()
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Text
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::TextVariant(m) = id {
            self.set_mode(m);
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, ctx: &egui::Context) {
        if self.pending {
            self.pending = false;
            self.open_dialog(cx);
            if self.dialog.is_none() {
                cx.requests.push(EditorRequest::SetTool(ToolId::Select));
            }
        }
        self.frame_dialogs(cx, ctx);
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.reset_state();
        cx.status = self.mode.hint().into();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        // Text typed but not finished is kept (TXT-15: leaving the tool
        // commits what was typed).
        if self.typing() && self.editing.is_none() {
            let _ = self.commit_typing(cx);
        }
        self.cancel(cx);
        self.hover = None;
        cx.last_snap = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let mut s = cx.snap_at(p.world, self.pts.last().copied(), p.modifiers.alt, &[]);
        // Dragging from a new text's anchor sizes its box (TXT-1).
        if let Some((scr, _)) = self.press {
            if p.down
                && self.typing()
                && self.buf.is_empty()
                && self.editing.is_none()
                && (p.screen - scr).length() >= BOX_DRAG_PX
            {
                self.box_drag = Some(s.point);
            }
        }
        // The anchor of the next text lines up with the left edge of nearby
        // text (TXT-10).
        self.align_guide = None;
        if !self.typing()
            && !p.modifiers.alt
            && matches!(self.mode, TextMode::Text | TextMode::RichText)
        {
            if let Some((at, other)) = align_left_edge(cx, &s) {
                s.point = at;
                self.align_guide = Some((other, at));
            }
        }
        self.hover = Some(s.point);
        cx.last_snap = Some(s);
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if let Some(id) = self.strip.hit(p.screen) {
            return self.strip_click(cx, id);
        }
        if self.mode.is_command() || self.dialog.is_some() {
            return ToolResult::consumed();
        }
        // A click away from the text being typed finishes it (TXT-1); in the
        // text tools it also starts the next text.
        if self.typing() {
            let res = self.commit_typing(cx);
            if !matches!(self.mode, TextMode::Text | TextMode::RichText) {
                return res;
            }
            let started = self.start_text(cx, &p);
            return ToolResult {
                commit: res.commit,
                ..started
            };
        }
        match self.mode {
            TextMode::Text | TextMode::RichText => self.start_text(cx, &p),
            TextMode::LeaderLine | TextMode::ArrowLine => {
                let s = cx.snap_at(p.world, self.pts.last().copied(), p.modifiers.alt, &[]);
                if self.pts.last().is_none_or(|l| l.dist(s.point) >= 0.5) {
                    self.pts.push(s.point);
                }
                ToolResult::consumed()
            }
            TextMode::Callout | TextMode::Marker | TextMode::Note => {
                // The specification opens; OK places the object.
                let layer = text_layer(cx);
                if cx.layers().is_locked(&layer) {
                    cx.status = format!("The layer \"{layer}\" is locked");
                    return ToolResult::consumed();
                }
                let at = if p.modifiers.alt { p.world } else { p.snapped };
                annot::post_new(annot::new_at(cx, mode_kind(self.mode), at));
                ToolResult::consumed()
            }
            TextMode::NoteTypes | TextMode::Macros | TextMode::TextStyles => ToolResult::consumed(),
        }
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let press = self.press.take();
        let Some(drag_end) = self.box_drag.take() else {
            return ToolResult::ignored();
        };
        let Some((_, a)) = press else {
            return ToolResult::ignored();
        };
        if !self.typing() || self.editing.is_some() {
            return ToolResult::ignored();
        }
        let b = if p.modifiers.alt {
            p.world
        } else {
            cx.snap_at(p.world, None, false, &[]).point
        };
        let b = if b.dist(drag_end) < 1e-9 { drag_end } else { b };
        let h = Self::drawn_height(cx, self.height(cx));
        let pitch = h * plan_core::text_box::LINE_SPACING;
        let (lo_x, lo_y) = (a.x.min(b.x), a.y.min(b.y));
        let (w, dy) = ((a.x - b.x).abs(), (a.y - b.y).abs());
        if w < h * TEXT_WIDTH_FACTOR * 3.0 {
            // Too narrow for a box: an ordinary click.
            return ToolResult::consumed();
        }
        let tb = TextBox {
            width: w,
            height: if dy > pitch { dy } else { 0.0 },
            ..TextBox::default()
        };
        self.anchor = Some(Point::new(lo_x, lo_y));
        self.text_box = Some((tb, lo_y + dy.max(pitch)));
        cx.status = format!(
            "Text box {} wide: type the text; Enter finishes, Esc cancels",
            cx.fmt_dim(w)
        );
        ToolResult::consumed()
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        if matches!(self.mode, TextMode::LeaderLine | TextMode::ArrowLine) && !self.pts.is_empty() {
            return self.finish_leader(cx);
        }
        if !self.typing() && self.pts.is_empty() {
            let tol = cx.pick_tol();
            let hit = hit_test(cx.floor(), cx.layers(), p.world, tol)
                .into_iter()
                .find(|o| matches!(o, ObjectRef::Cad(_)));
            if let Some(o) = hit {
                cx.selection.set(o);
                cx.requests.push(EditorRequest::OpenSpec(o));
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if self.typing() {
            if k.is(egui::Key::Escape) {
                self.cancel(cx);
                cx.status.clear();
                return ToolResult::consumed();
            }
            if k.is(egui::Key::Enter) {
                if self.mode == TextMode::RichText && self.editing.is_none() {
                    self.buf.push('\n');
                    return ToolResult::consumed();
                }
                return self.commit_typing(cx);
            }
            if k.is(egui::Key::Tab) && self.mode == TextMode::RichText {
                return self.commit_typing(cx);
            }
            if k.is(egui::Key::Backspace) {
                self.buf.pop();
            } else if let Some(s) = &k.text {
                self.buf.extend(s.chars().filter(|c| !c.is_control()));
            }
            return ToolResult::consumed();
        }
        if k.is(egui::Key::Escape) {
            if !self.pts.is_empty() {
                self.cancel(cx);
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if k.is(egui::Key::Enter)
            && matches!(self.mode, TextMode::LeaderLine | TextMode::ArrowLine)
            && !self.pts.is_empty()
        {
            return self.finish_leader(cx);
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        self.strip.draw(painter, cam, pal, &self.strip_items(cx));
        let ghost = Stroke::new(1.0_f32, pal.ghost_stroke);
        let hover = self.hover;
        let items = self.pending_items(cx, hover);
        // The left-edge alignment guide (TXT-10).
        if let Some((from, to)) = self.align_guide {
            painter.line_segment(
                [cam.world_to_screen(from), cam.world_to_screen(to)],
                Stroke::new(1.0_f32, pal.selection),
            );
        }
        // The rectangle being dragged for a new text box (TXT-1).
        if let (Some((_, a)), Some(b)) = (self.press, self.box_drag) {
            let r = Rect::from_two_pos(cam.world_to_screen(a), cam.world_to_screen(b));
            painter.rect_stroke(r, 0.0, ghost, egui::StrokeKind::Middle);
        }
        for it in &items {
            // The ghost is drawn at the size the text will print.
            let shown = match it {
                CadItem::Text {
                    pos,
                    text,
                    height,
                    angle,
                } => CadItem::Text {
                    pos: *pos,
                    text: text.clone(),
                    height: Self::drawn_height(cx, *height),
                    angle: *angle,
                },
                other => other.clone(),
            };
            let it = &shown;
            if let (CadItem::Text { .. }, Some((tb, _))) = (it, self.text_box) {
                // A text box: wrapped lines inside the dragged width.
                let obj = plan_core::CadObject {
                    id: 0,
                    layer: TEXT_LAYER.to_string(),
                    item: it.clone(),
                };
                let mut attrs = plan_core::cad::CadAttrs::new(0);
                attrs.text_box = tb;
                if self.mode == TextMode::RichText {
                    attrs.runs = runs_from_markup(&self.buf);
                }
                if let Some(pb) = plan_core::text_box::placed(it, &attrs) {
                    render::draw_text_box(cx, painter, cam, &obj, &attrs);
                    let pts: Vec<egui::Pos2> = pb
                        .layout
                        .corners(pb.pos, pb.angle, 0.0)
                        .iter()
                        .map(|q| cam.world_to_screen(*q))
                        .collect();
                    painter.add(egui::Shape::closed_line(pts, ghost));
                }
                continue;
            }
            render::draw_cad(painter, cam, it, ghost, pal);
            if let CadItem::Text { text, .. } = it {
                if self.typing() {
                    let (lo, hi) = it.bounds();
                    let r = Rect::from_two_pos(cam.world_to_screen(lo), cam.world_to_screen(hi))
                        .expand(3.0);
                    painter.rect_stroke(r, 0.0, ghost, egui::StrokeKind::Outside);
                    // The caret.
                    let end = cam.world_to_screen(Point::new(hi.x, lo.y));
                    painter.line_segment(
                        [
                            end,
                            end + egui::vec2(0.0, -((hi.y - lo.y) * cam.px_per_in) as f32),
                        ],
                        Stroke::new(1.5_f32, pal.selection),
                    );
                    let _ = text;
                }
            }
        }
        for q in &self.pts {
            painter.circle_filled(cam.world_to_screen(*q), 3.0, pal.ghost_stroke);
        }
        if let Some(s) = cx.last_snap {
            render::draw_snap_marker(painter, cam, &s, pal.ghost_stroke);
        }
    }

    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<crate::editor::EditAction> {
        cx.common_edit_actions()
    }
}

impl TextTool {
    /// A click that places or edits text.
    fn start_text(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        if let Some(id) = Self::text_under(cx, p.world) {
            if let Some(CadItem::Text { pos, text, .. }) =
                cad_by_id(cx.floor(), id).map(|c| c.item.clone())
            {
                if !cx.check_unlocked(ObjectRef::Cad(id)) {
                    return ToolResult::consumed();
                }
                self.editing = Some(id);
                self.begin_typing(cx, pos);
                // A rich text comes back as markup so its formats can be edited.
                self.buf = match cx.floor().cad_attrs(id) {
                    Some(a) if !a.runs.is_empty() => runs_to_markup(&a.runs),
                    _ => text,
                };
                cx.selection.set(ObjectRef::Cad(id));
                return ToolResult::consumed();
            }
        }
        cx.selection.clear();
        let at = if p.modifiers.alt {
            p.snapped
        } else {
            align_left_edge(cx, &p.snap).map_or(p.snapped, |(a, _)| a)
        };
        self.begin_typing(cx, at);
        self.press = Some((p.screen, at));
        self.text_box = None;
        ToolResult::consumed()
    }
}

/// Pixels the pointer travels from a new text's anchor before the drag sizes
/// a text box.
const BOX_DRAG_PX: f32 = 5.0;
/// How close (screen pixels) an anchor must be to the left edge of another
/// text to line up with it (TXT-10).
const ALIGN_PX: f64 = 6.0;

/// TXT-10: when `snap` is a plain grid or free point, the point moved onto the
/// left edge of the nearest text whose left edge is within [`ALIGN_PX`]
/// screen pixels, with that text's anchor (the guide runs between the two).
/// Object snaps win, as they do for every other point.
pub fn align_left_edge(cx: &EditorContext, snap: &SnapResult) -> Option<(Point, Point)> {
    use crate::editor::snap::SnapKind;
    if !matches!(snap.kind, SnapKind::Grid | SnapKind::Free | SnapKind::Angle) {
        return None;
    }
    let tol = ALIGN_PX / cx.px_per_in.max(1e-6);
    let at = snap.point;
    cx.floor()
        .cad
        .iter()
        .filter(|c| cx.layers().is_visible(&c.layer))
        .filter_map(|c| match &c.item {
            CadItem::Text { pos, angle, .. } if angle.abs() < 1e-9 => Some(*pos),
            _ => None,
        })
        .filter(|pos| (pos.x - at.x).abs() <= tol && pos.dist(at) > 1e-9)
        .min_by(|a, b| {
            let da = (a.x - at.x).abs() * 1e3 + (a.y - at.y).abs() * 1e-3;
            let db = (b.x - at.x).abs() * 1e3 + (b.y - at.y).abs() * 1e-3;
            da.total_cmp(&db)
        })
        .map(|other| (Point::new(other.x, at.y), other))
}

// ----- callouts, markers and notes (TXT-49..56) -----

/// The kind of annotation a Text tool mode places.
fn mode_kind(m: TextMode) -> AnnotKind {
    match m {
        TextMode::Marker => AnnotKind::Marker,
        TextMode::Note => AnnotKind::Note,
        _ => AnnotKind::Callout,
    }
}

/// The items a callout, marker or note placed at `at` would draw (the ghost
/// that follows the pointer).
fn annot_ghost(cx: &EditorContext, kind: AnnotKind, at: Point) -> Vec<CadItem> {
    use plan_core::callout::{callout_items, marker_items, note_items, Callout, Marker, Note, Vars};
    let d = &cx.project.annot_defaults;
    let v = Vars {
        number: Some(1),
        ..Vars::default()
    };
    let g = match kind {
        AnnotKind::Callout => callout_items(
            &Callout {
                center: at,
                ..d.callout.clone()
            },
            &v,
        ),
        AnnotKind::Marker => marker_items(
            &Marker {
                center: at,
                ..d.marker.clone()
            },
            &v,
        ),
        AnnotKind::Note => note_items(
            &Note {
                center: at,
                ..d.note.clone()
            },
            &v,
            false,
        ),
    };
    g.items.into_iter().map(|(i, _)| i).collect()
}

/// The callout, marker or note whose objects are exactly the selection (a
/// click on one selects its whole group): its record and first CAD object.
pub fn selected_annot(cx: &EditorContext) -> Option<(plan_core::callout::AnnotRef, Id)> {
    let f = cx.floor();
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .map(|o| match o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let r = f.annot_of(*ids.first()?)?;
    let items = f.annot_items(r);
    (ids.len() == items.len() && items.iter().all(|i| ids.contains(i))).then(|| (r, items[0]))
}

/// The edit handles of the selected callout, marker or note (Concentric
/// Resize, Rotate, Extend, Add Text Line with Arrow, Add Callout Arrow and
/// the arrows' own Rotate handles); empty for anything else.
pub fn annot_handles(cx: &EditorContext, scale: f64) -> Vec<crate::editor::handles::Handle> {
    use crate::editor::handles::{Handle, HandleKind};
    use eframe::egui::CursorIcon;
    use plan_core::callout::{callout_handles, handle, marker_handles, note_handles, AnnotRef, Vars};
    let Some((r, head)) = selected_annot(cx) else {
        return Vec::new();
    };
    let f = cx.floor();
    let up = 24.0 / scale.max(1e-6);
    let hs = match r {
        AnnotRef::Callout(i) => {
            let c = &f.annots.callouts[i];
            let v = Vars {
                link: c.link.as_ref().map(|l| cx.project.resolve_view_link(l)),
                ..Vars::default()
            };
            callout_handles(c, &v, up)
        }
        AnnotRef::Marker(i) => marker_handles(&f.annots.markers[i], up),
        AnnotRef::Note(i) => note_handles(&f.annots.notes[i], &Vars::default(), up),
    };
    hs.into_iter()
        .map(|h| Handle {
            kind: HandleKind::Annot(h.id),
            pos: h.pos,
            cursor: match h.id {
                handle::RESIZE => CursorIcon::ResizeNeSw,
                handle::EXTEND => CursorIcon::Crosshair,
                _ => CursorIcon::Grab,
            },
            target: ObjectRef::Cad(head),
        })
        .collect()
}

/// The annotation handle under `at`, for the Select tool: the first CAD
/// object of the annotation and the handle.
pub fn annot_handle_at(
    cx: &EditorContext,
    at: Point,
    tol: f64,
) -> Option<(Id, crate::editor::handles::HandleKind)> {
    let hs = annot_handles(cx, cx.px_per_in);
    crate::editor::handles::hit_handle(&hs, at, tol).map(|h| (h.target.id(), h.kind))
}

// ----- Edit toolbar buttons of callouts, notes and text (TXT-42, TXT-50, TXT-54) -----

const CMD_LINK: &str = "annot.link_view";
const CMD_UNLINK: &str = "annot.unlink_view";
const CMD_FIND: &str = "annot.find_in_layout";
const CMD_IGNORE_LINKS: &str = "annot.ignore_invalid_links";
const CMD_NOTE_SCHEDULE: &str = "annot.note_schedule";
const CMD_IGNORE_NOTE: &str = "annot.ignore_note";
const CMD_IGNORE_NOTES: &str = "annot.ignore_all_notes";
const CMD_TEXT_TO_NOTE: &str = "annot.text_to_note";
const CMD_FOLLOW_LINK: &str = "annot.follow_hyperlink";

fn custom_action(id: &'static str, label: &'static str) -> crate::editor::EditAction {
    use crate::editor::actions::{EditAction, EditActionKind};
    EditAction {
        kind: EditActionKind::Custom {
            id,
            label,
            icon: "",
        },
        label,
        icon: None,
        enabled: true,
    }
}

/// The notes (indices into `floor.annots.notes`) some selected object
/// belongs to.
fn selected_notes(cx: &EditorContext) -> Vec<usize> {
    let f = cx.floor();
    let mut v: Vec<usize> = Vec::new();
    for o in &cx.selection.items {
        if let ObjectRef::Cad(id) | ObjectRef::Text(id) = o {
            if let Some(plan_core::callout::AnnotRef::Note(i)) = f.annot_of(*id) {
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
    }
    v
}

/// The text objects among the selection that are not part of a callout,
/// marker or note.
fn selected_plain_texts(cx: &EditorContext) -> Vec<Id> {
    let f = cx.floor();
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) | ObjectRef::Text(id) => Some(*id),
            _ => None,
        })
        .filter(|id| {
            f.annot_of(*id).is_none()
                && cad_by_id(f, *id).is_some_and(|c| matches!(c.item, CadItem::Text { .. }))
        })
        .collect()
}

/// The hyperlink of the first selected text that has one.
fn selected_hyperlink(cx: &EditorContext) -> Option<String> {
    let f = cx.floor();
    selected_plain_texts(cx).into_iter().find_map(|id| {
        f.cad_attrs(id)
            .and_then(|a| a.runs.iter().find_map(|r| r.link.clone()))
    })
}

/// The buttons the Edit toolbar adds for a selected callout (Link View,
/// Unlink View, Find in Layout, Ignore Invalid Links), note (Create Note
/// Schedule from Note, Ignore Note With No Schedule) or text (Convert Text
/// to Note, Follow Hyperlink).
pub fn edit_actions(cx: &EditorContext) -> Vec<crate::editor::EditAction> {
    let mut v = Vec::new();
    if let Some((plan_core::callout::AnnotRef::Callout(i), _)) = selected_annot(cx) {
        let c = &cx.floor().annots.callouts[i];
        v.push(custom_action(CMD_LINK, "Link View"));
        if let Some(l) = &c.link {
            v.push(custom_action(CMD_UNLINK, "Unlink View"));
            let info = cx.project.resolve_view_link(l);
            if info.valid {
                v.push(custom_action(CMD_FIND, "Find in Layout"));
            } else {
                v.push(custom_action(CMD_IGNORE_LINKS, "Ignore Invalid Links"));
            }
        }
    }
    let notes = selected_notes(cx);
    if !notes.is_empty() {
        v.push(custom_action(
            CMD_NOTE_SCHEDULE,
            "Create Note Schedule from Note(s)",
        ));
        let lone = notes.iter().any(|i| {
            let n = &cx.floor().annots.notes[*i];
            !n.ignore_no_schedule && !cx.project.note_schedule_exists(&n.note_type)
        });
        if lone {
            v.push(custom_action(CMD_IGNORE_NOTE, "Ignore Note With No Schedule"));
            v.push(custom_action(
                CMD_IGNORE_NOTES,
                "Ignore All Notes With No Schedule",
            ));
        }
    }
    if !selected_plain_texts(cx).is_empty() {
        v.push(custom_action(CMD_TEXT_TO_NOTE, "Convert Text to Note"));
    }
    if selected_hyperlink(cx).is_some() {
        v.push(custom_action(CMD_FOLLOW_LINK, "Follow Hyperlink"));
    }
    v
}

/// Runs one of the [`edit_actions`]; false for any other command.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let fl = cx.floor;
    match id {
        CMD_LINK => {
            if let Some((_, head)) = selected_annot(cx) {
                cx.requests
                    .push(EditorRequest::OpenSpec(ObjectRef::Cad(head)));
            }
        }
        CMD_UNLINK => {
            if let Some((plan_core::callout::AnnotRef::Callout(i), head)) = selected_annot(cx) {
                if cx.check_unlocked(ObjectRef::Cad(head)) && cx.floor().annots.callouts[i].link.is_some() {
                    cx.begin_change("Unlink View");
                    cx.project.floors[fl].annots.callouts[i].link = None;
                    cx.project.sync_annotations();
                    cx.mark_dirty();
                }
            }
        }
        CMD_FIND => {
            if let Some((plan_core::callout::AnnotRef::Callout(i), _)) = selected_annot(cx) {
                let link = cx.floor().annots.callouts[i].link.clone();
                let info = link.map(|l| cx.project.resolve_view_link(&l));
                cx.status = match info {
                    Some(i) if i.valid && !i.page_label.is_empty() => {
                        format!("{} is on layout page {} ({})", i.view_name, i.page_label, i.file_name)
                    }
                    Some(i) if i.valid => format!("{} has not been sent to layout", i.view_name),
                    _ => "The link is broken".into(),
                };
            }
        }
        CMD_IGNORE_LINKS => {
            cx.begin_change("Ignore Invalid Links");
            for c in &mut cx.project.floors[fl].annots.callouts {
                c.ignore_link = true;
            }
            cx.project.sync_annotations();
            cx.mark_dirty();
        }
        CMD_NOTE_SCHEDULE => {
            let notes = selected_notes(cx);
            if notes.is_empty() {
                return true;
            }
            let mut types: Vec<String> = notes
                .iter()
                .map(|i| cx.floor().annots.notes[*i].note_type.clone())
                .collect();
            types.dedup();
            let first = cx.floor().annots.notes[notes[0]].center;
            cx.begin_change("Create Note Schedule from Note");
            let at = Point::new(first.x + 60.0, first.y);
            if let Some(sid) = cx.project.create_note_schedule(fl, &types, at) {
                cx.project.sync_annotations();
                cx.selection.set(ObjectRef::Schedule(sid));
            }
            cx.mark_dirty();
        }
        CMD_IGNORE_NOTE | CMD_IGNORE_NOTES => {
            let only = if id == CMD_IGNORE_NOTE {
                selected_notes(cx)
            } else {
                Vec::new()
            };
            cx.begin_change(if only.is_empty() {
                "Ignore All Notes With No Schedule"
            } else {
                "Ignore Note With No Schedule"
            });
            let n = cx.project.ignore_notes_without_schedule(fl, &only);
            if n == 0 {
                cx.cancel_change();
            } else {
                cx.project.sync_annotations();
                cx.mark_dirty();
            }
        }
        CMD_TEXT_TO_NOTE => {
            let texts = selected_plain_texts(cx);
            if texts.is_empty() {
                return true;
            }
            let base = cx.project.annot_defaults.note.clone();
            let ty = base.note_type.clone();
            cx.begin_change("Convert Text to Note");
            let mut made: Vec<ObjectRef> = Vec::new();
            for t in texts {
                if let Some(head) = cx.project.convert_text_to_note(fl, t, &ty, &base) {
                    made.extend(crate::editor::selection::expand_groups(cx, &[ObjectRef::Cad(head)]));
                }
            }
            cx.selection.items = made;
            cx.mark_dirty();
        }
        CMD_FOLLOW_LINK => {
            if let Some(url) = selected_hyperlink(cx) {
                cx.status = match follow_hyperlink(&url) {
                    Ok(()) => format!("Opened {url}"),
                    Err(e) => format!("Cannot open {url}: {e}"),
                };
            }
        }
        _ => return false,
    }
    true
}

/// Opens a hyperlink of a text in the default browser (a web address) or
/// application (a file). Does nothing under test.
pub fn follow_hyperlink(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("the link is empty".into());
    }
    if cfg!(test) {
        return Ok(());
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Dragging an annotation's handle under Select: applies the pointer
/// position `world` to the record whose first CAD object is `head` and
/// regenerates its objects. Angles snap to 15 degrees unless `free`.
fn drag_annot_handle(cx: &mut EditorContext, head: Id, handle_id: u8, world: Point, free: bool) -> bool {
    use plan_core::callout::{
        drag_callout_handle, drag_marker_handle, drag_note_handle, AnnotRef, Vars,
    };
    let fl = cx.floor;
    let step = (!free).then_some(15.0);
    let Some(r) = cx.floor().annot_of(head) else {
        return false;
    };
    let layer = match r {
        AnnotRef::Callout(i) => cx.floor().annots.callouts[i].layer.clone(),
        AnnotRef::Marker(i) => cx.floor().annots.markers[i].layer.clone(),
        AnnotRef::Note(i) => cx.floor().annots.notes[i].layer.clone(),
    };
    if cx.layers().is_locked(&layer) {
        return false;
    }
    let ok = match r {
        AnnotRef::Callout(i) => {
            let mut c = cx.project.floors[fl].annots.callouts[i].clone();
            let v = Vars {
                link: c.link.as_ref().map(|l| cx.project.resolve_view_link(l)),
                ..Vars::default()
            };
            let ok = drag_callout_handle(&mut c, &v, handle_id, world, step);
            cx.project.floors[fl].annots.callouts[i] = c;
            ok
        }
        AnnotRef::Marker(i) => {
            let mut m = cx.project.floors[fl].annots.markers[i].clone();
            let ok = drag_marker_handle(&mut m, handle_id, world, step);
            cx.project.floors[fl].annots.markers[i] = m;
            ok
        }
        AnnotRef::Note(i) => {
            let mut n = cx.project.floors[fl].annots.notes[i].clone();
            let ok = drag_note_handle(&mut n, &Vars::default(), handle_id, world, step);
            cx.project.floors[fl].annots.notes[i] = n;
            ok
        }
    };
    if ok {
        cx.project.sync_annotations();
        // The group keeps selecting as one; its objects may have changed.
        if let Some(r) = cx.floor().annot_of(head) {
            let members: Vec<ObjectRef> = cx
                .floor()
                .annot_items(r)
                .iter()
                .map(|i| ObjectRef::Cad(*i))
                .collect();
            cx.selection.items = members;
        }
    }
    ok
}

/// Dragging a text's box handle under Select (S-25, TXT-3, TXT-13). The
/// Select tool calls this for the `ResizeEnd` (wrap width), `Reshape(0)`
/// (minimum height) and `Reshape(1)` (both) handles of a text and applies the
/// pointer position `world` to the box; returns false for anything else.
/// Width and height are measured in the text's own frame, so a turned text
/// resizes along its own edges. Alt turns the grid off.
pub fn drag_box_handle(
    cx: &mut EditorContext,
    id: Id,
    kind: crate::editor::handles::HandleKind,
    world: Point,
    free: bool,
) -> bool {
    use crate::editor::handles::HandleKind;
    if let HandleKind::Annot(n) = kind {
        return drag_annot_handle(cx, id, n, world, free);
    }
    let (width, height) = match kind {
        HandleKind::ResizeEnd => (true, false),
        HandleKind::Reshape(0) => (false, true),
        HandleKind::Reshape(1) => (true, true),
        _ => return false,
    };
    let fl = cx.floor;
    let unit = cx.snap_unit();
    let Some((pos, angle, h)) = cx.project.floors[fl]
        .cad
        .iter()
        .find_map(|c| match &c.item {
            CadItem::Text {
                pos, angle, height, ..
            } if c.id == id => Some((*pos, *angle, *height)),
            _ => None,
        })
    else {
        return false;
    };
    let local = plan_core::text_box::to_local(pos, angle, world);
    let round = |v: f64| {
        if free || unit <= 0.0 {
            v
        } else {
            (v / unit).round() * unit
        }
    };
    let min_w = h * TEXT_WIDTH_FACTOR * 2.0;
    let new_w = round(local.x).max(min_w);
    let new_h = round(local.y).max(0.0);
    cx.project.edit_cad_attrs(fl, id, |a| {
        if width {
            a.text_box.width = new_w;
        }
        if height {
            a.text_box.height = new_h;
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::text_styles::NoteTypes;

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn tool(mode: TextMode) -> TextTool {
        let mut t = TextTool::default();
        t.set_mode(mode);
        t
    }

    fn click(t: &mut TextTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        let r = t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
        r
    }

    fn type_text(t: &mut TextTool, cx: &mut EditorContext, s: &str) {
        t.key(cx, KeyEvent::text(s));
    }

    fn enter(t: &mut TextTool, cx: &mut EditorContext) -> ToolResult {
        t.key(cx, KeyEvent::key(egui::Key::Enter))
    }

    #[test]
    fn text_goes_on_the_active_layer_of_the_text_tools() {
        use plan_core::Layer;
        let mut cx = new_cx();
        cx.project
            .layers
            .add(Layer::new("Text, Notes", [0, 0, 0], 18));
        assert!(cx.project.layers.set_tool_layer("text", "Text, Notes"));
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 10.0, 10.0);
        type_text(&mut t, &mut cx, "Note");
        enter(&mut t, &mut cx);
        assert_eq!(cx.floor().cad[0].layer, "Text, Notes");
        // A marker goes on that layer too.
        let mut m = tool(TextMode::Marker);
        click(&mut m, &mut cx, 50.0, 50.0);
        place(&mut m, &mut cx);
        assert!(cx
            .floor()
            .cad
            .iter()
            .skip(1)
            .all(|c| c.layer == "Text, Notes"));
        // Back on its own layer, the next text goes there.
        assert!(cx.project.layers.set_tool_layer("text", TEXT_LAYER));
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 80.0, 80.0);
        type_text(&mut t, &mut cx, "Other");
        enter(&mut t, &mut cx);
        assert_eq!(cx.floor().cad.last().unwrap().layer, TEXT_LAYER);
    }

    #[test]
    fn click_type_enter_places_text_at_the_default_height() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 10.0, 10.0);
        assert!(
            cx.temp.editing.is_some(),
            "typing asks the shell for characters"
        );
        assert!(
            cx.floor().cad.is_empty(),
            "nothing is stored until it is finished"
        );
        type_text(&mut t, &mut cx, "Hello");
        assert_eq!(t.typed(), Some("Hello"));
        let r = enter(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Place Text"));
        let c = &cx.floor().cad[0];
        assert_eq!(c.layer, TEXT_LAYER);
        assert_eq!(
            c.item,
            CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Hello".into(),
                height: cx.defaults.text.height,
                angle: 0.0
            }
        );
        assert!(cx.temp.editing.is_none());
        assert_eq!(cx.selection.single(), Some(ObjectRef::Cad(c.id)));
        // Undo removes it.
        assert_eq!(cx.undo().as_deref(), Some("Place Text"));
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn printed_size_text_is_placed_at_the_style_height_and_picked_by_its_drawn_box() {
        use plan_docs::Scale;
        let mut cx = new_cx();
        // The Text layer's style (the default one) holds 1/8" on paper and a
        // 9" character height.
        let i = cx
            .project
            .text_styles
            .styles
            .iter()
            .position(|s| s.name == "Default Text Style")
            .unwrap();
        cx.project.text_styles.styles[i].height_in = 9.0;
        cx.project.text_styles.styles[i].set_printed_in(0.125);
        cx.project.text_styles.styles[i].use_printed_size(true);
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 10.0, 10.0);
        type_text(&mut t, &mut cx, "KITCHEN");
        enter(&mut t, &mut cx);
        // Stored at the style's height, not the tool's 6".
        let id = cx.floor().cad[0].id;
        assert!(matches!(
            cx.floor().cad[0].item,
            CadItem::Text { height, .. } if (height - 9.0).abs() < 1e-9
        ));
        let tol = cx.pick_tol() * 0.5;
        assert!(tol < 5.0, "{tol}");
        // 1/4": drawn 6" high, 25.2" wide; the stored 9" box (37.8" wide)
        // reaches farther than the drawn one.
        cx.sheet.scale = Scale::QuarterInch;
        assert_eq!(TextTool::text_under(&cx, Point::new(30.0, 12.0)), Some(id));
        assert_eq!(TextTool::text_under(&cx, Point::new(46.0, 12.0)), None);
        assert_eq!(TextTool::text_under(&cx, Point::new(30.0, 20.0)), None);
        // 1/8": drawn 12" high, 50.4" wide: past the stored box now.
        cx.sheet.scale = Scale::EighthInch;
        assert_eq!(TextTool::text_under(&cx, Point::new(55.0, 15.0)), Some(id));
        assert_eq!(TextTool::text_under(&cx, Point::new(30.0, 21.0)), Some(id));
        assert_eq!(TextTool::text_under(&cx, Point::new(70.0, 15.0)), None);
        // The Select tool's picking agrees at both scales.
        let hit = |cx: &EditorContext, x: f64, y: f64| {
            crate::editor::selection::hit_test_cx(cx, Point::new(x, y), cx.pick_tol() * 0.5)
                .contains(&ObjectRef::Cad(id))
        };
        assert!(hit(&cx, 55.0, 15.0));
        assert!(!hit(&cx, 70.0, 15.0));
        cx.sheet.scale = Scale::QuarterInch;
        assert!(hit(&cx, 30.0, 12.0));
        assert!(!hit(&cx, 46.0, 12.0));
        // A character-height style picks by the stored box at any scale.
        cx.project.text_styles.styles[i].use_printed_size(false);
        cx.sheet.scale = Scale::EighthInch;
        assert_eq!(TextTool::text_under(&cx, Point::new(46.0, 12.0)), Some(id));
        assert_eq!(TextTool::text_under(&cx, Point::new(55.0, 15.0)), None);
    }

    #[test]
    fn backspace_escape_and_empty_text() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "Abc");
        t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        assert_eq!(t.typed(), Some("Ab"));
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        assert!(cx.floor().cad.is_empty());
        assert!(cx.temp.editing.is_none());
        // Enter on nothing creates nothing.
        click(&mut t, &mut cx, 0.0, 0.0);
        enter(&mut t, &mut cx);
        assert!(cx.floor().cad.is_empty());
        assert!(
            !t.key(&mut cx, KeyEvent::escape()).consumed,
            "idle Esc leaves the tool"
        );
    }

    #[test]
    fn clicking_elsewhere_commits_and_starts_the_next_text() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "One");
        let r = click(&mut t, &mut cx, 100.0, 50.0);
        assert_eq!(r.commit.as_deref(), Some("Place Text"));
        assert_eq!(cx.floor().cad.len(), 1);
        assert_eq!(t.typed(), Some(""), "the second text is waiting for typing");
        type_text(&mut t, &mut cx, "Two");
        enter(&mut t, &mut cx);
        assert_eq!(cx.floor().cad.len(), 2);
    }

    #[test]
    fn clicking_existing_text_edits_it_in_place() {
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            TEXT_LAYER,
            CadItem::Text {
                pos: Point::new(20.0, 20.0),
                text: "Old".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 24.0, 23.0);
        assert_eq!(t.typed(), Some("Old"));
        type_text(&mut t, &mut cx, "er");
        let r = enter(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Edit Text"));
        let CadItem::Text { text, pos, .. } = &cad_by_id(cx.floor(), id).unwrap().item else {
            panic!("text")
        };
        assert_eq!(text, "Older");
        assert_eq!(*pos, Point::new(20.0, 20.0));
        assert_eq!(cx.floor().cad.len(), 1);
        // Clearing the text deletes the object.
        click(&mut t, &mut cx, 24.0, 23.0);
        for _ in 0..8 {
            t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        }
        enter(&mut t, &mut cx);
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn rich_text_takes_lines_and_a_size_scale() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::RichText);
        t.rich.size_scale = 2.0;
        t.rich.bold = true;
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "Line 1");
        assert!(enter(&mut t, &mut cx).commit.is_none(), "Enter adds a line");
        type_text(&mut t, &mut cx, "Line 2");
        let r = t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        assert!(r.commit.is_some());
        let c = &cx.floor().cad[0];
        let CadItem::Text { text, height, .. } = &c.item else {
            panic!("text")
        };
        assert_eq!(text, "Line 1\nLine 2");
        assert_eq!(*height, cx.defaults.text.height * 2.0);
        assert!(t.style_of(c.id).unwrap().bold);
    }

    #[test]
    fn leader_line_is_a_polyline_with_an_arrowhead() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::LeaderLine);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        click(&mut t, &mut cx, 60.0, 40.0);
        let p = PointerEvent::at(&cx, Point::new(60.0, 40.0));
        let r = t.double_click(&mut cx, p);
        assert_eq!(r.commit.as_deref(), Some("Leader Line"));
        assert_eq!(cx.floor().cad.len(), 2);
        let CadItem::Polyline { points, closed } = &cx.floor().cad[0].item else {
            panic!("polyline")
        };
        assert_eq!(points.len(), 3);
        assert!(!*closed);
        let CadItem::Polyline {
            points: head,
            closed,
        } = &cx.floor().cad[1].item
        else {
            panic!("arrowhead")
        };
        assert!(*closed);
        assert_eq!(head[0], Point::ZERO, "the arrow tip is the first point");
        assert_eq!(cx.floor().groups.len(), 1);
        assert!(cx.floor().cad.iter().all(|c| c.layer == TEXT_LAYER));
        assert_eq!(cx.undo().as_deref(), Some("Leader Line"));
        assert!(cx.floor().cad.is_empty());
    }

    #[test]
    fn text_line_with_arrow_attaches_text_to_the_last_point() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::ArrowLine);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        enter(&mut t, &mut cx);
        assert!(cx.floor().cad.is_empty());
        assert!(t.typing());
        type_text(&mut t, &mut cx, "Verify");
        enter(&mut t, &mut cx);
        assert_eq!(cx.floor().cad.len(), 3);
        let CadItem::Text { pos, text, .. } = &cx.floor().cad[2].item else {
            panic!("text")
        };
        assert_eq!(text, "Verify");
        assert!(pos.x > 60.0 && pos.y > 0.0);
        assert_eq!(cx.floor().groups.len(), 1);
    }

    #[test]
    fn locked_text_layer_refuses() {
        let mut cx = new_cx();
        cx.project.layers.set_locked(TEXT_LAYER, true);
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "No");
        enter(&mut t, &mut cx);
        assert!(cx.floor().cad.is_empty());
        assert!(cx.status.contains("locked"));
    }

    #[test]
    fn leaving_the_tool_keeps_typed_text() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Text);
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "Kept");
        t.deactivate(&mut cx);
        assert_eq!(cx.floor().cad.len(), 1);
        assert!(cx.temp.editing.is_none());
    }

    #[test]
    fn double_click_asks_for_the_text_dialog() {
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            TEXT_LAYER,
            CadItem::Text {
                pos: Point::ZERO,
                text: "Hi".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let mut t = tool(TextMode::Text);
        let p = PointerEvent::at(&cx, Point::new(3.0, 3.0));
        assert!(t.double_click(&mut cx, p).consumed);
        assert_eq!(
            cx.requests,
            vec![EditorRequest::OpenSpec(ObjectRef::Cad(id))]
        );
    }

    #[test]
    fn variants_by_name_and_request() {
        let mut t = TextTool::default();
        assert!(t.set_mode_by_name("leader line"));
        assert_eq!(t.mode(), TextMode::LeaderLine);
        t.set_variant(ToolId::TextVariant(TextMode::Callout));
        assert_eq!(t.mode(), TextMode::Callout);
        for m in TextMode::ALL {
            assert_eq!(TextMode::from_name(m.name()), Some(m));
        }
    }

    fn text_objects(cx: &EditorContext) -> Vec<(Id, String)> {
        cx.floor()
            .cad
            .iter()
            .filter(|c| c.layer == TEXT_LAYER)
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some((c.id, text.clone())),
                _ => None,
            })
            .collect()
    }

    fn type_and(
        t: &mut TextTool,
        cx: &mut EditorContext,
        at: (f64, f64),
        s: &str,
        finish: egui::Key,
    ) {
        click(t, cx, at.0, at.1);
        type_text(t, cx, s);
        t.key(cx, KeyEvent::key(finish));
    }

    /// OK in the specification the last click opened.
    fn place(_t: &mut TextTool, cx: &mut EditorContext) -> Option<Id> {
        annot::take_posted().and_then(|d| d.apply(cx))
    }

    /// Edits the label of the dialog the last click posted, putting it back.
    fn edit_label(_t: &mut TextTool, f: impl FnOnce(&mut String)) {
        if let Some(mut d) = annot::take_posted() {
            d.edit_label(f);
            annot::post_new(d);
        }
    }

    /// The posted dialog's title and tabs (it stays posted).
    fn posted(_t: &TextTool) -> (String, Vec<&'static str>) {
        let d = annot::take_posted().expect("a dialog was posted");
        let r = (d.title(), d.tab_names());
        annot::post_new(d);
        r
    }

    #[test]
    fn a_click_opens_the_callout_specification_and_ok_places_a_grouped_callout() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Callout);
        click(&mut t, &mut cx, 100.0, 50.0);
        assert!(cx.floor().cad.is_empty(), "nothing is placed before OK");
        let (title, tabs) = posted(&t);
        assert_eq!(title, "Callout Specification");
        assert_eq!(
            tabs,
            vec![
                "Callout",
                "Attributes",
                "Line Style",
                "Section Arrow",
                "Main Text Style",
                "Link"
            ]
        );
        let steps = cx.action_history().0.len();
        edit_label(&mut t, |l| *l = "A1".into());
        let id = place(&mut t, &mut cx).expect("placed");
        assert_eq!(cx.action_history().0.len(), steps + 1, "one undo step");
        assert_eq!(cx.undo_label(), Some("Place Callout"));
        // A record, its circle and its label, selected as one group.
        assert_eq!(cx.floor().annots.callouts.len(), 1);
        assert_eq!(cx.floor().annots.callouts[0].center, Point::new(100.0, 50.0));
        assert!(cx.floor().cad.iter().any(|c| matches!(&c.item,
            CadItem::Text { text, .. } if text == "A1")));
        assert!(cx.selection.items.contains(&ObjectRef::Cad(id)));
        assert_eq!(cx.selection.len(), cx.floor().cad.len());
        // The tool stays active for the next one; undo takes it all away.
        click(&mut t, &mut cx, 200.0, 50.0);
        assert!(annot::take_posted().is_some());
        cx.undo();
        assert!(cx.floor().cad.is_empty() && cx.floor().annots.is_empty());
        cx.redo();
        assert_eq!(cx.floor().annots.callouts.len(), 1);
    }

    #[test]
    fn markers_and_notes_open_their_own_specifications_and_notes_number_in_order() {
        let mut cx = new_cx();
        let mut m = tool(TextMode::Marker);
        click(&mut m, &mut cx, 0.0, 0.0);
        let (title, tabs) = posted(&m);
        assert_eq!(title, "Marker Specification");
        assert_eq!(tabs, vec!["Marker", "Line Style", "Text Style"]);
        place(&mut m, &mut cx);
        assert_eq!(cx.floor().annots.markers.len(), 1);
        assert_eq!(cx.undo_label(), Some("Place Marker"));

        let mut n = tool(TextMode::Note);
        for (x, body) in [(0.0, "Verify"), (60.0, "Match existing")] {
            click(&mut n, &mut cx, x, 100.0);
            let (title, tabs) = posted(&n);
            assert_eq!(title, "Note Specification");
            assert_eq!(
                tabs,
                vec!["Note", "Line Style", "Text Style", "Object Information", "Schedule"]
            );
            edit_label(&mut n, |t| *t = body.into());
            place(&mut n, &mut cx);
        }
        let rows = cx.project.note_rows();
        assert_eq!(
            rows.iter().map(|r| (r.mark.as_str(), r.text.as_str())).collect::<Vec<_>>(),
            vec![("Note 1", "Verify"), ("Note 2", "Match existing")]
        );
        assert_eq!(cx.undo_label(), Some("Place Note"));
    }

    #[test]
    fn the_strip_cycles_the_default_shape_and_marker_type() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Callout);
        let first = cx.project.annot_defaults.callout.shape;
        t.strip_click(&mut cx, BTN_SHAPE);
        assert_ne!(cx.project.annot_defaults.callout.shape, first);
        assert!(t
            .strip_items(&cx)
            .iter()
            .any(|b| b.label.starts_with("Shape: Oval")));
        // The next callout starts with that shape.
        click(&mut t, &mut cx, 0.0, 0.0);
        place(&mut t, &mut cx);
        assert_eq!(cx.floor().annots.callouts[0].shape, CalloutShape::Oval);
        let mut m = tool(TextMode::Marker);
        m.strip_click(&mut cx, BTN_MARKER_KIND);
        assert_eq!(cx.project.annot_defaults.marker.kind, MarkerKind::TestBoring);
        let mut n = tool(TextMode::Note);
        n.strip_click(&mut cx, BTN_NOTE_TYPE);
        assert_eq!(n.note_type, "Construction Note");
        assert!(n
            .strip_items(&cx)
            .iter()
            .any(|b| b.label == "Type: Construction Note"));
    }

    #[test]
    fn the_selected_callout_has_edit_handles_and_dragging_them_edits_the_record() {
        use crate::editor::handles::HandleKind;
        let mut cx = new_cx();
        let mut t = tool(TextMode::Callout);
        click(&mut t, &mut cx, 100.0, 100.0);
        place(&mut t, &mut cx);
        let hs = annot_handles(&cx, cx.px_per_in);
        let ids: Vec<HandleKind> = hs.iter().map(|h| h.kind).collect();
        assert!(ids.contains(&HandleKind::Annot(plan_core::callout::handle::RESIZE)));
        assert!(ids.contains(&HandleKind::Annot(plan_core::callout::handle::ROTATE)));
        assert!(ids.contains(&HandleKind::Annot(plan_core::callout::handle::ADD_LINE)));
        assert!(ids.contains(&HandleKind::Annot(plan_core::callout::handle::ADD_ARROW)));
        let head = cx.floor().annots.callouts[0].items[0];
        let at = annot_handle_at(&cx, hs[0].pos, 1.0).expect("a handle");
        assert_eq!(at.0, head);
        // Concentric resize.
        assert!(drag_box_handle(
            &mut cx,
            head,
            HandleKind::Annot(plan_core::callout::handle::RESIZE),
            Point::new(130.0, 100.0),
            true
        ));
        let c = &cx.floor().annots.callouts[0];
        assert!(!c.auto_size && (c.size - 30.0).abs() < 1e-6);
        // Add Text Line with Arrow drags out an arrow attached to the callout.
        assert!(drag_box_handle(
            &mut cx,
            head,
            HandleKind::Annot(plan_core::callout::handle::ADD_LINE),
            Point::new(100.0, 10.0),
            true
        ));
        assert_eq!(cx.floor().annots.callouts[0].leaders.len(), 1);
        // Add Callout Arrow drags a hat out; dragging to the center removes it.
        assert!(drag_box_handle(
            &mut cx,
            head,
            HandleKind::Annot(plan_core::callout::handle::ADD_ARROW),
            Point::new(180.0, 100.0),
            true
        ));
        assert_eq!(cx.floor().annots.callouts[0].arrows.angles.len(), 1);
        assert!(drag_box_handle(
            &mut cx,
            head,
            HandleKind::Annot(plan_core::callout::handle::ADD_ARROW),
            Point::new(101.0, 100.0),
            true
        ));
        assert!(cx.floor().annots.callouts[0].arrows.angles.is_empty());
        // Another kind of object has no annotation handles.
        cx.selection.clear();
        assert!(annot_handles(&cx, 1.0).is_empty());
    }

    #[test]
    fn rich_text_runs_keep_their_formats_and_round_trip_through_editing() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::RichText);
        let markup = "<b>Bold</b> and <i>italic</i> <size=1.5>big</size>";
        type_and(&mut t, &mut cx, (60.0, 60.0), markup, egui::Key::Tab);
        let texts = text_objects(&cx);
        assert_eq!(texts.len(), 1);
        let (id, plain) = &texts[0];
        assert_eq!(
            plain, "Bold and italic big",
            "the item keeps the plain words"
        );
        let attrs = cx.floor().cad_attrs(*id).expect("runs are stored");
        assert_eq!(
            attrs.runs,
            vec![
                RichRun::bold("Bold"),
                RichRun::plain(" and "),
                RichRun::italic("italic"),
                RichRun::plain(" "),
                RichRun::sized("big", 1.5),
            ]
        );
        assert_eq!(runs_to_markup(&attrs.runs), markup);

        // Clicking the text again edits it as markup; Enter with no change
        // adds no undo step.
        let steps = cx.undo_label().map(str::to_string);
        click(&mut t, &mut cx, 61.0, 61.0);
        assert_eq!(t.typed(), Some(markup));
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert_eq!(cx.undo_label().map(str::to_string), steps);
        // Edit a word: runs follow.
        click(&mut t, &mut cx, 61.0, 61.0);
        for _ in 0..10 {
            t.key(&mut cx, KeyEvent::key(egui::Key::Backspace));
        }
        type_text(&mut t, &mut cx, "huge</size>");
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        let attrs = cx.floor().cad_attrs(*id).unwrap();
        assert!(attrs
            .runs
            .last()
            .is_some_and(|r| r.text == "huge" && r.scale == 1.5));
        assert_eq!(text_objects(&cx)[0].1, "Bold and italic huge");
        assert_eq!(cx.undo().as_deref(), Some("Edit Text"));
        assert_eq!(cx.floor().cad_attrs(*id).unwrap().runs.len(), 5);
    }

    #[test]
    fn the_bold_italic_underline_buttons_format_the_whole_rich_text() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::RichText);
        t.strip_click(&mut cx, BTN_BOLD);
        t.strip_click(&mut cx, BTN_UNDERLINE);
        assert!(t.rich_style().bold && t.rich_style().underline);
        type_and(&mut t, &mut cx, (0.0, 0.0), "Plain words", egui::Key::Tab);
        let (id, text) = text_objects(&cx).remove(0);
        assert_eq!(text, "Plain words");
        let runs = cx.floor().cad_attrs(id).unwrap().runs;
        assert_eq!(runs.len(), 1);
        assert!(runs[0].bold && runs[0].underline && !runs[0].italic);
        // Unformatted rich text stores no runs at all.
        t.strip_click(&mut cx, BTN_BOLD);
        t.strip_click(&mut cx, BTN_UNDERLINE);
        type_and(
            &mut t,
            &mut cx,
            (0.0, 120.0),
            "Nothing special",
            egui::Key::Tab,
        );
        let (id2, _) = text_objects(&cx).remove(1);
        assert!(cx.floor().cad_attrs(id2).is_none());
    }

    #[test]
    fn macros_expand_when_text_is_placed() {
        let mut cx = new_cx();
        cx.project.name = "Smith Residence".into();
        let mut m = plan_core::text_styles::TextMacros::default();
        m.add("firm", "Daniel Allen Designs");
        cx.project.set_text_macros(&m);
        let mut t = tool(TextMode::Text);
        type_and(
            &mut t,
            &mut cx,
            (0.0, 0.0),
            "%plan.name% - %floor% - %firm% - %nope% 50%",
            egui::Key::Enter,
        );
        assert_eq!(
            text_objects(&cx)[0].1,
            "Smith Residence - 1st Floor - Daniel Allen Designs - %nope% 50%"
        );
        let mut d = tool(TextMode::Text);
        type_and(
            &mut d,
            &mut cx,
            (0.0, 120.0),
            "Printed %plan.date%",
            egui::Key::Enter,
        );
        let (_, text) = text_objects(&cx).remove(1);
        let date = text.strip_prefix("Printed ").unwrap();
        assert_eq!(date.len(), 10);
        assert_eq!(date.matches('-').count(), 2);
        // Notes and callouts expand too, when their dialog is OK'd.
        let mut n = tool(TextMode::Callout);
        click(&mut n, &mut cx, 0.0, 240.0);
        edit_label(&mut n, |l| *l = "By %firm%".into());
        place(&mut n, &mut cx);
        assert!(text_objects(&cx)
            .iter()
            .any(|(_, t)| t == "By Daniel Allen Designs"));
    }

    #[test]
    fn management_commands_open_dialogs_and_store_their_results() {
        let ctx = egui::Context::default();
        let mut cx = new_cx();
        let mut t = tool(TextMode::Macros);
        let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(&mut cx, ctx));
        let Some(TextUi::Macros(mut d)) = t.dialog.take() else {
            panic!("the macro dialog is open")
        };
        let mut next = plan_core::text_styles::TextMacros::default();
        next.add("firm", "DAD");
        let mut dialog = MacroDialog::new(next.clone());
        std::mem::swap(&mut *d, &mut dialog);
        t.dialog_outcome(&mut cx, TextUi::Macros(d), Outcome::Ok);
        assert_eq!(cx.project.text_macros(), next);
        assert_eq!(cx.requests, vec![EditorRequest::SetTool(ToolId::Select)]);
        assert_eq!(cx.undo().as_deref(), Some("Change Text Macros"));
        assert!(cx.project.text_macros().macros.is_empty());

        // Cancel changes nothing; Note Types stores the edited list.
        cx.requests.clear();
        let mut t = tool(TextMode::NoteTypes);
        let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(&mut cx, ctx));
        let Some(TextUi::NoteTypes(d)) = t.dialog.take() else {
            panic!("the note type dialog is open")
        };
        let before = cx.project.note_types();
        assert_eq!(*d.draft(), before);
        let mut edited = NoteTypeDialog::new(before.clone(), vec![]);
        edited.add_new_named("Plumbing Note", "P");
        t.dialog_outcome(
            &mut cx,
            TextUi::NoteTypes(Box::new(edited)),
            Outcome::Cancel,
        );
        assert_eq!(cx.project.note_types(), before, "cancel stores nothing");
        let mut types = NoteTypes::default();
        types.add("Plumbing Note", "P");
        let styles = vec!["Default Text Style".to_string()];
        let dialog = NoteTypeDialog::new(types.clone(), styles);
        t.dialog_outcome(&mut cx, TextUi::NoteTypes(Box::new(dialog)), Outcome::Ok);
        assert_eq!(cx.project.note_types(), types);
        // An open dialog stays open.
        let mut t = tool(TextMode::Macros);
        let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(&mut cx, ctx));
        assert!(t.dialog.is_some());
        let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(&mut cx, ctx));
        assert!(t.dialog.is_some());
    }
    fn drag(t: &mut TextTool, cx: &mut EditorContext, a: (f64, f64), b: (f64, f64)) {
        let pa = PointerEvent::at(cx, Point::new(a.0, a.1));
        let pb = PointerEvent::at(cx, Point::new(b.0, b.1));
        t.pointer_move(cx, pa);
        t.pointer_down(cx, pa.with_down(true));
        t.pointer_move(cx, pb.with_down(true));
        t.pointer_up(cx, pb);
    }

    fn the_text(cx: &EditorContext) -> (Id, Point, f64, plan_core::text_box::TextBox) {
        let c = cx.floor().cad.last().expect("a text");
        let CadItem::Text { pos, height, .. } = &c.item else {
            panic!("not a text")
        };
        let tb = cx
            .floor()
            .cad_attrs(c.id)
            .map(|a| a.text_box)
            .unwrap_or_default();
        (c.id, *pos, *height, tb)
    }

    #[test]
    fn dragging_defines_a_text_box_with_a_wrap_width() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let mut t = tool(TextMode::Text);
        drag(&mut t, &mut cx, (0.0, 100.0), (60.0, 40.0));
        assert!(t.typing(), "the box waits for the text");
        type_text(&mut t, &mut cx, "aa bb cc dd ee ff gg hh");
        let r = enter(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Place Text"));
        let (id, pos, h, tb) = the_text(&cx);
        assert_eq!(tb.width, 60.0);
        assert_eq!(tb.height, 60.0);
        // Two lines fit in the dragged height: the box is where it was dragged.
        assert_eq!(pos, Point::new(0.0, 40.0));
        let lay = layout("aa bb cc dd ee ff gg hh", &[], h, &tb);
        assert_eq!(lay.lines.len(), 2, "the text wraps at the box width");
        // One undo step takes text and box away.
        assert_eq!(cx.undo().as_deref(), Some("Place Text"));
        assert!(cx.floor().cad.is_empty());
        assert!(cx.floor().cad_attrs(id).is_none());
    }

    #[test]
    fn a_box_keeps_its_top_edge_as_the_text_grows() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let mut t = tool(TextMode::Text);
        drag(&mut t, &mut cx, (0.0, 100.0), (60.0, 90.0));
        type_text(&mut t, &mut cx, &"word ".repeat(30));
        enter(&mut t, &mut cx);
        let (id, pos, h, tb) = the_text(&cx);
        let CadItem::Text { text, .. } = &cad_by_id(cx.floor(), id).unwrap().item else {
            panic!()
        };
        let lay = layout(text, &[], h, &tb);
        assert!(lay.lines.len() > 3);
        assert!(
            (pos.y + lay.height - 100.0).abs() < 1e-6,
            "the top stays at 100"
        );
        // Editing in place shortens the box from the top, too.
        click(&mut t, &mut cx, 5.0, 100.0 - 2.0);
        assert_eq!(t.editing, Some(id), "a click in the box edits the text");
        t.buf = "short".into();
        enter(&mut t, &mut cx);
        let (_, pos2, _, tb2) = the_text(&cx);
        let lay2 = layout("short", &[], h, &tb2);
        assert!((pos2.y + lay2.height - 100.0).abs() < 1e-6);
        assert_eq!(tb2.width, 60.0, "the width is kept");
    }

    #[test]
    fn a_short_drag_is_an_ordinary_click_and_a_narrow_one_is_too() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let mut t = tool(TextMode::Text);
        drag(&mut t, &mut cx, (0.0, 0.0), (4.0, 0.0));
        type_text(&mut t, &mut cx, "plain");
        enter(&mut t, &mut cx);
        let (id, pos, _, tb) = the_text(&cx);
        assert!(tb.is_plain(), "{tb:?}");
        assert!(cx.floor().cad_attrs(id).is_none());
        assert_eq!(pos, Point::new(0.0, 0.0));
    }

    #[test]
    fn rich_text_can_be_dragged_into_a_box_too() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        let mut t = tool(TextMode::RichText);
        drag(&mut t, &mut cx, (0.0, 0.0), (80.0, 30.0));
        type_text(&mut t, &mut cx, "<b>bold</b> and plain words");
        t.key(&mut cx, KeyEvent::key(egui::Key::Tab));
        let (id, _, _, tb) = the_text(&cx);
        assert_eq!(tb.width, 80.0);
        assert!(cx
            .floor()
            .cad_attrs(id)
            .unwrap()
            .runs
            .iter()
            .any(|r| r.bold));
    }

    #[test]
    fn a_new_text_lines_up_with_the_left_edge_of_nearby_text() {
        let mut cx = new_cx();
        cx.px_per_in = 2.0;
        cx.project.add_cad(
            0,
            TEXT_LAYER,
            CadItem::Text {
                pos: Point::new(100.5, 200.0),
                text: "Notes".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        let mut t = tool(TextMode::Text);
        // One screen pixel off the other text's left edge (half an inch at
        // 2 px/in): the anchor takes the edge, and the guide is drawn.
        let p = PointerEvent::at(&cx, Point::new(101.0, 160.0));
        t.pointer_move(&mut cx, p);
        assert!(t.align_guide.is_some());
        t.pointer_down(&mut cx, p.with_down(true));
        type_text(&mut t, &mut cx, "Below");
        enter(&mut t, &mut cx);
        let (_, pos, _, _) = the_text(&cx);
        assert_eq!(pos.x, 100.5, "x lines up with the text above");
        // Far away: no alignment. Alt: none either.
        let far = PointerEvent::at(&cx, Point::new(140.0, 120.0));
        t.pointer_move(&mut cx, far);
        assert!(t.align_guide.is_none());
        let alt = PointerEvent::at(&cx, Point::new(101.0, 100.0)).with_modifiers(egui::Modifiers {
            alt: true,
            ..egui::Modifiers::NONE
        });
        t.pointer_move(&mut cx, alt);
        assert!(t.align_guide.is_none());
    }

    #[test]
    fn dragging_the_box_handles_sizes_the_box_in_the_texts_own_frame() {
        use crate::editor::handles::HandleKind;
        let mut cx = new_cx();
        let id = cx.project.add_cad(
            0,
            TEXT_LAYER,
            CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "one two three four five six".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        // Width: the pointer's x in the text's frame.
        assert!(drag_box_handle(
            &mut cx,
            id,
            HandleKind::ResizeEnd,
            Point::new(70.0, 30.0),
            true
        ));
        let a = cx.floor().cad_attrs(id).unwrap();
        assert_eq!(a.text_box.width, 60.0);
        assert_eq!(a.text_box.height, 0.0, "the height is untouched");
        // Height, then both.
        assert!(drag_box_handle(
            &mut cx,
            id,
            HandleKind::Reshape(0),
            Point::new(30.0, 50.0),
            true
        ));
        assert_eq!(cx.floor().cad_attrs(id).unwrap().text_box.height, 40.0);
        assert!(drag_box_handle(
            &mut cx,
            id,
            HandleKind::Reshape(1),
            Point::new(90.0, 80.0),
            true
        ));
        let tb = cx.floor().cad_attrs(id).unwrap().text_box;
        assert_eq!((tb.width, tb.height), (80.0, 70.0));
        // Not smaller than a couple of characters.
        assert!(drag_box_handle(
            &mut cx,
            id,
            HandleKind::ResizeEnd,
            Point::new(5.0, 30.0),
            true
        ));
        assert!(cx.floor().cad_attrs(id).unwrap().text_box.width >= 6.0 * 0.6 * 2.0 - 1e-9);
        // Other handles and other objects are not ours.
        assert!(!drag_box_handle(
            &mut cx,
            id,
            HandleKind::Rotate,
            Point::ZERO,
            true
        ));
        let line = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        assert!(!drag_box_handle(
            &mut cx,
            line,
            HandleKind::ResizeEnd,
            Point::ZERO,
            true
        ));
        // A turned text measures along its own edges.
        let turned = cx.project.add_cad(
            0,
            TEXT_LAYER,
            CadItem::Text {
                pos: Point::new(0.0, 0.0),
                text: "turned".into(),
                height: 6.0,
                angle: std::f64::consts::FRAC_PI_2,
            },
        );
        assert!(drag_box_handle(
            &mut cx,
            turned,
            HandleKind::ResizeEnd,
            Point::new(-5.0, 50.0),
            true
        ));
        let w = cx.floor().cad_attrs(turned).unwrap().text_box.width;
        assert!((w - 50.0).abs() < 1e-9, "{w}");
    }

    #[test]
    fn text_style_management_renames_and_removes_in_one_undo_step() {
        let mut cx = new_cx();
        cx.project.layers.layers[0].text_style = "Room Label Style".into();
        let mut t = tool(TextMode::TextStyles);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| t.frame(&mut cx, ctx));
        let Some(TextUi::Styles(mut d)) = t.dialog.take() else {
            panic!("the text style dialog is open")
        };
        assert!(d.rename("Room Label Style", "Labels"));
        assert!(d.remove("Schedule Style"));
        t.dialog_outcome(&mut cx, TextUi::Styles(d), Outcome::Ok);
        assert!(cx.project.text_styles.get("Labels").is_some());
        assert!(cx.project.text_styles.get("Schedule Style").is_none());
        assert_eq!(cx.project.layers.layers[0].text_style, "Labels");
        assert_eq!(cx.undo().as_deref(), Some("Change Text Styles"));
        assert!(cx.project.text_styles.get("Room Label Style").is_some());
        assert_eq!(cx.project.layers.layers[0].text_style, "Room Label Style");
        // Cancel stores nothing.
        let mut d = TextStyleDialog::new(vec!["Default Text Style".into(), "Labels".into()]);
        d.remove("Labels");
        t.dialog_outcome(&mut cx, TextUi::Styles(Box::new(d)), Outcome::Cancel);
        assert!(cx.project.text_styles.get("Room Label Style").is_some());
    }
}
