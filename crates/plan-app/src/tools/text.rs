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
//! * Callout (TXT-7): click the target, click the callout position, type;
//!   a circle or hexagon is drawn around the text.
//! * Marker (TXT-8): a numbered circle. Note (TXT-9): text "Note n: ..." with
//!   the next free note number, which a Note schedule can read back with
//!   [`note_number`].
//!
//! * Rich Text keeps its runs: inline `<b> <i> <u> <size=1.5> <color=#RRGGBB>`
//!   markup typed in the text (and the B / I / U buttons) become
//!   `RichRun`s stored with the text (`CadAttrs::runs`); the text item keeps
//!   the plain words.
//! * Text macros: `%room.name%`, `%plan.date%`, `%floor%` and the user's own
//!   macros expand when text is placed or edited (Text Macro Management).
//! * Notes take their label from the active note type (Note Type
//!   Management): `Note 3:`, `E 1:`, ...
//!
//! The shell sends typed characters to a tool only while `cx.temp.editing`
//! is set, so the tool raises that flag while text is being typed
//! ([`set_typing`]).

use super::cad::{add_cad_items, arrowhead, regular_polygon, set_typing, OptionStrip, StripButton};
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::text::manage::{MacroDialog, NoteTypeDialog};
use crate::dialogs::Outcome;
use crate::editor::selection::{cad_by_id, cad_distance, hit_test};
use crate::editor::{render, Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Rect, Stroke};
use plan_core::cad::{CadItem, TEXT_WIDTH_FACTOR};
use plan_core::geometry::{point_in_polygon, Point};
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
}

impl TextMode {
    pub const ALL: [TextMode; 9] = [
        TextMode::Text,
        TextMode::RichText,
        TextMode::LeaderLine,
        TextMode::ArrowLine,
        TextMode::Callout,
        TextMode::Marker,
        TextMode::Note,
        TextMode::NoteTypes,
        TextMode::Macros,
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
        }
    }

    /// Opens a dialog when picked instead of drawing.
    fn is_command(self) -> bool {
        matches!(self, TextMode::NoteTypes | TextMode::Macros)
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
            m => m.name(),
        }
    }

    fn hint(self) -> &'static str {
        match self {
            TextMode::Text => "Text: click to place text, type, Enter to finish",
            TextMode::RichText => "Rich Text: click, type (Enter adds a line), Tab to finish",
            TextMode::LeaderLine => "Leader Line: click the arrow tip and the bends; Enter or double-click ends",
            TextMode::ArrowLine => "Text Line with Arrow: click the arrow tip and the bends; Enter ends, then type the text",
            TextMode::Callout => "Callout: click the target, click the callout position, type the text",
            TextMode::Marker => "Marker: click to place the next numbered marker",
            TextMode::Note => "Note: click to place a numbered note, type, Enter to finish",
            TextMode::NoteTypes => "Note Type Management: add note types and their label prefixes",
            TextMode::Macros => {
                "Text Macro Management: define %macros% that expand when text is placed"
            }
        }
    }
}

/// The outline drawn around callout text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalloutShape {
    Circle,
    Hexagon,
    Square,
}

/// The style of a Rich Text box. Only the size scale reaches the model (as
/// the text height); the rest is kept for the session.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RichStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub size_scale: f64,
}

impl Default for RichStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            size_scale: 1.0,
        }
    }
}

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

/// `items` with every text set to the stored `height` (the shapes around it
/// were sized for the text as drawn).
fn keep_text_height(mut items: Vec<CadItem>, height: f64) -> Vec<CadItem> {
    for it in &mut items {
        if let CadItem::Text { height: h, .. } = it {
            *h = height;
        }
    }
    items
}

/// A callout around `text` centered on `center` with a leader to `target`
/// (TXT-7): shape first, then the leader and arrowhead, then the text.
pub fn callout_items(
    target: Point,
    center: Point,
    text: &str,
    height: f64,
    shape: CalloutShape,
) -> Vec<CadItem> {
    let w = text_width(text, height);
    let lines = text.lines().count().max(1) as f64;
    let h = height * lines;
    let r = (w.max(h) * 0.5 + height * 0.7).max(height);
    let mut items = Vec::new();
    // Half the box of a square callout.
    let (hx, hy) = (
        (w * 0.5 + height * 0.7).max(height),
        (h * 0.5 + height * 0.7).max(height),
    );
    let reach = match shape {
        CalloutShape::Square => hx.max(hy),
        _ => r,
    };
    match shape {
        CalloutShape::Circle => items.push(CadItem::Circle { center, radius: r }),
        CalloutShape::Hexagon => items.push(CadItem::Polyline {
            points: regular_polygon(center, center.add(Point::new(r * 1.1, 0.0)), 6),
            closed: true,
        }),
        CalloutShape::Square => items.push(CadItem::Polyline {
            points: vec![
                Point::new(center.x - hx, center.y - hy),
                Point::new(center.x + hx, center.y - hy),
                Point::new(center.x + hx, center.y + hy),
                Point::new(center.x - hx, center.y + hy),
            ],
            closed: true,
        }),
    }
    if target.dist(center) > reach * 1.2 {
        let dir = target.sub(center).normalized();
        // Where the leader meets the outline.
        let edge = match shape {
            CalloutShape::Square => {
                let k = (dir.x.abs() / hx).max(dir.y.abs() / hy);
                center.add(dir.scale(1.0 / k.max(1e-9)))
            }
            _ => center.add(dir.scale(r)),
        };
        items.extend(leader_items(&[target, edge], (height * 1.2).max(3.0)));
    }
    items.push(CadItem::Text {
        pos: Point::new(center.x - w * 0.5, center.y - h * 0.5),
        text: text.to_string(),
        height,
        angle: 0.0,
    });
    items
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
    shape: CalloutShape,
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
}

/// The dialogs of the two management commands.
enum TextUi {
    NoteTypes(Box<NoteTypeDialog>),
    Macros(Box<MacroDialog>),
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            mode: TextMode::Text,
            shape: CalloutShape::Circle,
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
        }
    }
}

const BTN_NOTE_TYPE: u16 = 120;
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

    pub fn set_callout_shape(&mut self, s: CalloutShape) {
        self.shape = s;
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
        let h = cx.defaults.text.height;
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

    fn arrow_size(&self, cx: &EditorContext) -> f64 {
        (self.height(cx) * 1.2).max(3.0)
    }

    /// The next free number of the active note type (TXT-9).
    fn next_note(&self, cx: &EditorContext) -> u32 {
        let types = cx.project.note_types();
        let texts: Vec<&str> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        types.next_number(&self.note_type, texts)
    }

    /// The text of the next note of the active type.
    fn note_label(&self, cx: &EditorContext, body: &str) -> String {
        cx.project
            .note_types()
            .format(&self.note_type, self.next_note(cx), body)
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
        }
        let plain = runs_plain(&runs);
        (runs, plain)
    }

    /// The next marker number: one more than the largest number text on the
    /// Text layer that sits inside a marker circle.
    fn next_marker(cx: &EditorContext) -> u32 {
        let layer = text_layer(cx);
        cx.floor()
            .cad
            .iter()
            .filter(|c| c.layer == layer)
            .filter_map(|c| match &c.item {
                CadItem::Text { text, pos, .. } => {
                    let n: u32 = text.parse().ok()?;
                    let inside = cx.floor().cad.iter().any(|o| {
                        matches!(o.item, CadItem::Circle { center, radius }
                            if center.dist(*pos) <= radius * 1.2)
                    });
                    inside.then_some(n)
                }
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
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
            TextMode::Text | TextMode::RichText | TextMode::Note => {
                let Some(a) = self.anchor else {
                    return Vec::new();
                };
                let text = if self.mode == TextMode::Note && self.editing.is_none() {
                    self.note_label(cx, &self.buf)
                } else if self.mode == TextMode::RichText {
                    runs_plain(&runs_from_markup(&self.buf))
                } else {
                    self.buf.clone()
                };
                vec![CadItem::Text {
                    pos: a,
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
            TextMode::Callout => {
                // The shape is sized for the text as drawn; the text keeps
                // its stored (style) height.
                let dh = Self::drawn_height(cx, height);
                let items = match (self.pts.first(), self.anchor) {
                    (Some(t), Some(c)) => callout_items(*t, c, &self.buf, dh, self.shape),
                    (Some(t), None) => extra
                        .map(|c| callout_items(*t, c, "  ", dh, self.shape))
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                keep_text_height(items, height)
            }
            TextMode::Marker => extra
                .map(|c| {
                    let dh = Self::drawn_height(cx, height);
                    keep_text_height(marker_items(c, Self::next_marker(cx), dh), height)
                })
                .unwrap_or_default(),
            TextMode::NoteTypes | TextMode::Macros => Vec::new(),
        }
    }

    /// Finishes the text being typed (Enter, Tab, a click elsewhere).
    fn commit_typing(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some(anchor) = self.anchor else {
            return ToolResult::ignored();
        };
        let height = self.height(cx);
        let raw = self.buf.trim_end().to_string();
        let editing = self.editing;
        let mode = self.mode;
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
        let target = self.pts.first().copied();
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
                (
                    vec![CadItem::Text {
                        pos: anchor,
                        text,
                        height,
                        angle: 0.0,
                    }],
                    "Place Text",
                )
            }
            TextMode::Note => {
                if text.trim().is_empty() {
                    return ToolResult::consumed();
                }
                (
                    vec![CadItem::Text {
                        pos: anchor,
                        text: self.note_label(cx, &text),
                        height,
                        angle: 0.0,
                    }],
                    "Place Note",
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
            TextMode::Callout => {
                let Some(t) = target else {
                    return ToolResult::consumed();
                };
                if text.trim().is_empty() {
                    return ToolResult::consumed();
                }
                let dh = Self::drawn_height(cx, height);
                (
                    keep_text_height(callout_items(t, anchor, &text, dh, self.shape), height),
                    "Place Callout",
                )
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
                ToolResult::committed(label)
            }
            None => ToolResult::consumed(),
        }
    }

    /// The runs worth keeping: none when no run carries a format (a single
    /// plain run is just the text).
    fn normalized_runs(runs: Vec<RichRun>) -> Vec<RichRun> {
        let formatted = runs.iter().any(|r| {
            r.bold || r.italic || r.underline || (r.scale - 1.0).abs() > 1e-9 || r.color.is_some()
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
        if text.trim().is_empty() {
            Self::remove_text(cx, id);
            cx.selection.clear();
        } else if let Some(c) = cx.project.floors[fl].cad.iter_mut().find(|c| c.id == id) {
            if let CadItem::Text { text: t, .. } = &mut c.item {
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
            (cad_distance(item, p) <= tol).then_some(c.id)
        })
    }

    fn strip_items(&self) -> Vec<StripButton> {
        let mut v: Vec<StripButton> = TextMode::ALL
            .iter()
            .enumerate()
            .map(|(i, m)| StripButton::new(m.short(), i as u16, *m == self.mode))
            .collect();
        if self.mode == TextMode::Callout {
            v.push(StripButton::new(
                "Circle",
                BTN_SHAPE,
                self.shape == CalloutShape::Circle,
            ));
            v.push(StripButton::new(
                "Hexagon",
                BTN_SHAPE + 1,
                self.shape == CalloutShape::Hexagon,
            ));
            v.push(StripButton::new(
                "Square",
                BTN_SHAPE + 2,
                self.shape == CalloutShape::Square,
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
            BTN_SHAPE => self.shape = CalloutShape::Circle,
            i if i == BTN_SHAPE + 1 => self.shape = CalloutShape::Hexagon,
            i if i == BTN_SHAPE + 2 => self.shape = CalloutShape::Square,
            BTN_NOTE_TYPE => self.next_note_type(cx),
            BTN_BOLD => self.rich.bold = !self.rich.bold,
            BTN_ITALIC => self.rich.italic = !self.rich.italic,
            BTN_UNDERLINE => self.rich.underline = !self.rich.underline,
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
        let s = cx.snap_at(p.world, self.pts.last().copied(), p.modifiers.alt, &[]);
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
            if !matches!(
                self.mode,
                TextMode::Text | TextMode::RichText | TextMode::Note
            ) {
                return res;
            }
            let started = self.start_text(cx, &p);
            return ToolResult {
                commit: res.commit,
                ..started
            };
        }
        match self.mode {
            TextMode::Text | TextMode::RichText | TextMode::Note => self.start_text(cx, &p),
            TextMode::LeaderLine | TextMode::ArrowLine => {
                let s = cx.snap_at(p.world, self.pts.last().copied(), p.modifiers.alt, &[]);
                if self.pts.last().is_none_or(|l| l.dist(s.point) >= 0.5) {
                    self.pts.push(s.point);
                }
                ToolResult::consumed()
            }
            TextMode::Callout => {
                let s = cx.snap_at(p.world, self.pts.last().copied(), p.modifiers.alt, &[]);
                if self.pts.is_empty() {
                    self.pts.push(s.point);
                } else {
                    self.begin_typing(cx, s.point);
                }
                ToolResult::consumed()
            }
            TextMode::NoteTypes | TextMode::Macros => ToolResult::consumed(),
            TextMode::Marker => {
                let height = self.height(cx);
                let n = Self::next_marker(cx);
                let at = p.snapped;
                let dh = Self::drawn_height(cx, height);
                let items = keep_text_height(marker_items(at, n, dh), height);
                let layer = text_layer(cx);
                match add_cad_items(cx, &layer, items, "Place Marker") {
                    Some(_) => ToolResult::committed("Place Marker"),
                    None => ToolResult::consumed(),
                }
            }
        }
    }

    fn pointer_up(&mut self, _cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        ToolResult::ignored()
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
        self.strip.draw(painter, cam, pal, &self.strip_items());
        let ghost = Stroke::new(1.0_f32, pal.ghost_stroke);
        let hover = self.hover;
        let items = self.pending_items(cx, hover);
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
        self.begin_typing(cx, p.snapped);
        ToolResult::consumed()
    }
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
        // Marker numbers count the markers on that layer.
        let mut m = tool(TextMode::Marker);
        click(&mut m, &mut cx, 50.0, 50.0);
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
    fn callout_draws_a_shape_a_leader_and_centered_text() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Callout);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        type_text(&mut t, &mut cx, "A1");
        let r = enter(&mut t, &mut cx);
        assert_eq!(r.commit.as_deref(), Some("Place Callout"));
        let items: Vec<&CadItem> = cx.floor().cad.iter().map(|c| &c.item).collect();
        assert!(
            matches!(items[0], CadItem::Circle { center, .. } if *center == Point::new(120.0, 0.0))
        );
        assert!(items
            .iter()
            .any(|i| matches!(i, CadItem::Text { text, .. } if text == "A1")));
        assert_eq!(items.len(), 4, "circle, leader, arrowhead, text");

        let mut t = tool(TextMode::Callout);
        t.set_callout_shape(CalloutShape::Hexagon);
        click(&mut t, &mut cx, 0.0, 100.0);
        click(&mut t, &mut cx, 120.0, 100.0);
        type_text(&mut t, &mut cx, "B");
        enter(&mut t, &mut cx);
        assert!(cx.floor().cad.iter().any(
            |c| matches!(&c.item, CadItem::Polyline { points, closed: true } if points.len() == 6)
        ));
    }

    #[test]
    fn markers_count_up() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Marker);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 60.0, 0.0);
        let numbers: Vec<String> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(numbers, vec!["1", "2"]);
    }

    #[test]
    fn notes_are_numbered_and_readable_by_a_schedule() {
        let mut cx = new_cx();
        let mut t = tool(TextMode::Note);
        for (x, body) in [(0.0, "Verify"), (60.0, "Match existing")] {
            click(&mut t, &mut cx, x, 0.0);
            type_text(&mut t, &mut cx, body);
            enter(&mut t, &mut cx);
        }
        let texts: Vec<String> = cx
            .floor()
            .cad
            .iter()
            .filter_map(|c| match &c.item {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Note 1: Verify", "Note 2: Match existing"]);
        assert_eq!(note_number(&texts[1]), Some(2));
        assert_eq!(note_number("Just text"), None);
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
        // Notes and callouts expand too.
        let mut n = tool(TextMode::Note);
        type_and(&mut n, &mut cx, (0.0, 240.0), "By %firm%", egui::Key::Enter);
        assert!(text_objects(&cx)
            .iter()
            .any(|(_, t)| t == "Note 1: By Daniel Allen Designs"));
    }

    #[test]
    fn notes_take_the_label_and_number_of_their_type() {
        let mut cx = new_cx();
        let mut types = cx.project.note_types();
        types.add("Plumbing Note", "P");
        cx.project.set_note_types(&types);
        let mut t = tool(TextMode::Note);
        type_and(&mut t, &mut cx, (0.0, 0.0), "General one", egui::Key::Enter);
        t.note_type = "Plumbing Note".into();
        type_and(&mut t, &mut cx, (0.0, 60.0), "Vent", egui::Key::Enter);
        type_and(&mut t, &mut cx, (0.0, 120.0), "Trap", egui::Key::Enter);
        t.note_type = GENERAL_NOTE.into();
        type_and(
            &mut t,
            &mut cx,
            (0.0, 180.0),
            "General two",
            egui::Key::Enter,
        );
        let texts: Vec<String> = text_objects(&cx).into_iter().map(|(_, t)| t).collect();
        assert_eq!(
            texts,
            vec![
                "Note 1: General one",
                "P 1: Vent",
                "P 2: Trap",
                "Note 2: General two"
            ]
        );
        // The strip cycles through the types.
        t.strip_click(&mut cx, BTN_NOTE_TYPE);
        assert_eq!(t.note_type, "Construction Note");
        assert!(t
            .strip_items()
            .iter()
            .any(|b| b.label == "Type: Construction Note"));
    }

    #[test]
    fn square_callouts_have_a_box_a_leader_and_centered_text() {
        let items = callout_items(
            Point::new(300.0, 0.0),
            Point::new(0.0, 0.0),
            "Verify",
            6.0,
            CalloutShape::Square,
        );
        // Box, leader line, arrowhead, text.
        assert_eq!(items.len(), 4);
        let CadItem::Polyline { points, closed } = &items[0] else {
            panic!("a box")
        };
        assert!(*closed && points.len() == 4);
        let CadItem::Polyline { points: leader, .. } = &items[1] else {
            panic!("the leader")
        };
        assert_eq!(leader[0], Point::new(300.0, 0.0));
        let hx = points[1].x;
        assert!(
            (leader[1].x - hx).abs() < 1e-6 && leader[1].y.abs() < 1e-6,
            "{leader:?}"
        );
        assert!(matches!(&items[3], CadItem::Text { text, .. } if text == "Verify"));
        // A target inside the box needs no leader.
        let inside = callout_items(
            Point::new(1.0, 0.0),
            Point::ZERO,
            "x",
            6.0,
            CalloutShape::Square,
        );
        assert_eq!(inside.len(), 2);
        // The strip offers the shape.
        let mut t = tool(TextMode::Callout);
        let mut cx = new_cx();
        t.strip_click(&mut cx, BTN_SHAPE + 2);
        assert!(t.strip_items().iter().any(|b| b.label == "Square" && b.on));
        click(&mut t, &mut cx, 300.0, 0.0);
        click(&mut t, &mut cx, 0.0, 0.0);
        type_text(&mut t, &mut cx, "Hi");
        t.key(&mut cx, KeyEvent::key(egui::Key::Enter));
        assert!(cx.floor().cad.iter().any(
            |c| matches!(&c.item, CadItem::Polyline { points, closed: true } if points.len() == 4)
        ));
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
}
