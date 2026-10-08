//! Chief-style text styles: a named font, size and colour that layers and
//! annotations refer to by name (`Layer::text_style`).

use crate::layers::LayerSet;
use serde::{Deserialize, Serialize};

/// The style used when a name is empty or unknown.
pub const DEFAULT_TEXT_STYLE_NAME: &str = "Default Text Style";

/// Plan inches of text height that prints `printed_in` paper inches tall at
/// `inches_per_foot` paper scale (1/8" at 1/4" scale: 6").
pub fn plan_height_for_printed(printed_in: f64, inches_per_foot: f64) -> f64 {
    printed_in * 12.0 / inches_per_foot
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub name: String,
    pub font: String,
    /// The face inside the family the template names ("Book", "Heavy");
    /// empty: the bold and italic flags choose the face.
    pub font_style: String,
    /// Character height in plan inches (6" is 1/8" on paper at 1/4" scale).
    pub height_in: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: [u8; 3],
    /// `true`: the plan height `height_in` is used as is, so printed size
    /// follows the drawing scale (Chief "Use character height"). `false`:
    /// `printed_pt` is the size on paper whatever the scale.
    pub size_by_scale: bool,
    /// Printed size in points (9 pt = 1/8"), where known.
    pub printed_pt: Option<f64>,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle::plan_sized(DEFAULT_TEXT_STYLE_NAME, 6.0, false)
    }
}

impl TextStyle {
    /// An Arial style given by plan height; `printed_pt` is what that height
    /// prints at 1/4" scale.
    pub fn plan_sized(name: impl Into<String>, height_in: f64, bold: bool) -> Self {
        Self {
            name: name.into(),
            font: "Arial".into(),
            font_style: String::new(),
            height_in,
            bold,
            italic: false,
            underline: false,
            color: [0, 0, 0],
            size_by_scale: true,
            printed_pt: Some(height_in * 0.25 / 12.0 * 72.0),
        }
    }

    /// The same style in another font (`Avenir`).
    pub fn with_font(mut self, font: impl Into<String>) -> Self {
        self.font = font.into();
        self
    }

    /// The same style naming a face inside its family (`Heavy`).
    pub fn with_font_style(mut self, style: impl Into<String>) -> Self {
        self.font_style = style.into();
        self
    }

    /// The family the font names: `Avenir` for `Avenir` and for `Avenir Book`.
    pub fn font_family(&self) -> &str {
        split_font_name(&self.font).0
    }

    /// The face style asked for by name: `font_style`, else the style words
    /// that end the font name (`Book` of `Avenir Book`); empty when neither.
    pub fn font_face_style(&self) -> &str {
        if self.font_style.trim().is_empty() {
            split_font_name(&self.font).1
        } else {
            self.font_style.trim()
        }
    }

    /// The same style with italic set or cleared.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Plan character height when drawn at `inches_per_foot` paper scale
    /// (0.25 for 1/4" scale).
    pub fn height_for_scale(&self, inches_per_foot: f64) -> f64 {
        match self.printed_pt {
            Some(pt) if !self.size_by_scale && inches_per_foot > 0.0 => {
                plan_height_for_printed(pt / 72.0, inches_per_foot)
            }
            _ => self.height_in,
        }
    }

    /// The size on paper, inches: `printed_pt` where set, else what
    /// `height_in` prints at 1/4" scale.
    pub fn printed_in(&self) -> f64 {
        self.printed_pt
            .unwrap_or(self.height_in * 0.25 / 12.0 * 72.0)
            / 72.0
    }

    /// Sets the printed size, paper inches (Chief "Use Printed Size").
    pub fn set_printed_in(&mut self, printed_in: f64) {
        self.printed_pt = Some(printed_in * 72.0);
    }

    /// Chooses between "Printed Size" (`true`: the size on paper holds at
    /// any scale) and "Character Height" (`false`: the plan height holds).
    /// The printed size starts from what the plan height prints at 1/4".
    pub fn use_printed_size(&mut self, on: bool) {
        if on && self.printed_pt.is_none() {
            self.printed_pt = Some(self.printed_in() * 72.0);
        }
        self.size_by_scale = !on;
    }

    pub fn is_printed_size(&self) -> bool {
        !self.size_by_scale
    }

    /// Plan character height at `inches_per_foot` paper scale, using the
    /// printed size when `force_printed` is set even for a character-height
    /// style (a dimension set that holds its sizes on paper).
    pub fn plan_height_at(&self, inches_per_foot: f64, force_printed: bool) -> f64 {
        if (force_printed || !self.size_by_scale) && inches_per_foot > 0.0 {
            plan_height_for_printed(self.printed_in(), inches_per_foot)
        } else {
            self.height_in
        }
    }

    /// The plan height of a text object of `item_height` set in this style
    /// at `inches_per_foot`: a printed-size style keeps its size on paper
    /// whatever the scale, keeping the object's relation to the style (an
    /// object twice the style's height stays twice it); a character-height
    /// style leaves the object's height alone.
    pub fn text_height(&self, item_height: f64, inches_per_foot: f64) -> f64 {
        if self.size_by_scale || inches_per_foot <= 0.0 || self.height_in <= 0.0 {
            item_height
        } else {
            item_height * plan_height_for_printed(self.printed_in(), inches_per_foot)
                / self.height_in
        }
    }
}

/// Words that end a font name as a face style (`Avenir Book`). `Black` is
/// left out: `Arial Black` is a family.
const FACE_STYLE_WORDS: [&str; 9] = [
    "Book", "Heavy", "Roman", "Regular", "Light", "Medium", "Bold", "Italic", "Oblique",
];

/// Splits a font name into its family and a trailing face style:
/// `Avenir Book` is `("Avenir", "Book")`, `Arial` is `("Arial", "")`.
pub fn split_font_name(name: &str) -> (&str, &str) {
    let name = name.trim();
    if let Some((family, last)) = name.rsplit_once(' ') {
        if !family.trim().is_empty()
            && FACE_STYLE_WORDS
                .iter()
                .any(|w| w.eq_ignore_ascii_case(last.trim()))
        {
            return (family.trim(), last.trim());
        }
    }
    (name, "")
}

/// A font name reduced to letters and digits in lower case, to compare
/// names (`Helvetica Neue` and `helveticaneue`).
pub fn normalize_font_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Do two font names name the same family (`Avenir` and `Avenir Book`)?
pub fn same_font_family(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize_font_name(a), normalize_font_name(b));
    na == nb
        || normalize_font_name(split_font_name(a).0) == normalize_font_name(split_font_name(b).0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyles {
    pub styles: Vec<TextStyle>,
}

impl Default for TextStyles {
    fn default() -> Self {
        Self::chief_defaults()
    }
}

impl TextStyles {
    /// Chief-like starting styles. Only "Default Text Style" (Arial, 6" plan,
    /// 1/8" printed) comes from Daniel's capture; the others are sized to
    /// match it.
    pub fn chief_defaults() -> Self {
        let s = TextStyle::plan_sized;
        Self {
            styles: vec![
                s(DEFAULT_TEXT_STYLE_NAME, 6.0, false),
                s("1/4\" Text Style", 6.0, false),
                s("Room Label Style", 6.0, true),
                s("Schedule Style", 4.5, false),
                s("Default Label Style", 4.5, false),
                s("Dimension Text Style", 4.5, false),
            ],
        }
    }

    pub fn get(&self, name: &str) -> Option<&TextStyle> {
        self.styles.iter().find(|s| s.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.styles.iter().map(|s| s.name.as_str()).collect()
    }

    /// The font families the styles use, sorted, each once (first spelling).
    pub fn fonts_used(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.styles {
            let fam = s.font_family();
            if !fam.is_empty() && !out.iter().any(|o| same_font_family(o, fam)) {
                out.push(fam.to_string());
            }
        }
        out.sort_by_key(|f| normalize_font_name(f));
        out
    }

    /// Replace Fonts (Chief TXT-12): every style set in the family `from`
    /// is set in `to` instead (its face style is dropped, the bold and
    /// italic flags pick the face). Returns the number of styles changed.
    pub fn replace_font(&mut self, from: &str, to: &str) -> usize {
        let to = to.trim();
        if to.is_empty() || from.trim().is_empty() || same_font_family(from, to) {
            return 0;
        }
        let mut n = 0;
        for s in &mut self.styles {
            if same_font_family(&s.font, from) {
                s.font = to.to_string();
                s.font_style.clear();
                n += 1;
            }
        }
        n
    }

    /// Adds a style if its name is new and non-empty.
    pub fn add(&mut self, style: TextStyle) -> bool {
        if style.name.is_empty() || self.get(&style.name).is_some() {
            return false;
        }
        self.styles.push(style);
        true
    }

    /// Removes a style by name; "Default Text Style" cannot be removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if name == DEFAULT_TEXT_STYLE_NAME {
            return false;
        }
        let before = self.styles.len();
        self.styles.retain(|s| s.name != name);
        self.styles.len() != before
    }

    /// The style a `Layer::text_style` name refers to; an empty or unknown
    /// name gives "Default Text Style" (or the first style if that is gone).
    /// `None` only when there are no styles at all.
    pub fn resolve(&self, name: &str) -> Option<&TextStyle> {
        self.get(name)
            .or_else(|| self.get(DEFAULT_TEXT_STYLE_NAME))
            .or_else(|| self.styles.first())
    }

    /// The text style of a layer, by layer name. Unknown layers use the
    /// default style.
    pub fn resolve_for_layer(&self, layers: &LayerSet, layer: &str) -> Option<&TextStyle> {
        self.resolve(layers.get(layer).map_or("", |l| l.text_style.as_str()))
    }

    /// The style a text object on `layer` is set in: its own style name when
    /// it has one, else the layer's.
    pub fn style_of_text(
        &self,
        layers: &LayerSet,
        layer: &str,
        own: Option<&str>,
    ) -> Option<&TextStyle> {
        match own {
            Some(name) if !name.is_empty() => self.resolve(name),
            _ => self.resolve_for_layer(layers, layer),
        }
    }

    /// The plan character height a text object of stored `item_height` is
    /// drawn (and picked, and exported) at for `inches_per_foot` of paper
    /// scale: a printed-size style holds its size on paper, a
    /// character-height style (or no scale) leaves `item_height` alone.
    pub fn drawn_height(
        &self,
        layers: &LayerSet,
        layer: &str,
        own: Option<&str>,
        item_height: f64,
        inches_per_foot: f64,
    ) -> f64 {
        self.style_of_text(layers, layer, own)
            .map_or(item_height, |s| s.text_height(item_height, inches_per_foot))
    }

    /// The height to store in a new text placed on `layer`: a printed-size
    /// style's own character height (so the text draws exactly at the printed
    /// size at any scale), else `default_height`, the Text tool's height.
    pub fn placed_height(
        &self,
        layers: &LayerSet,
        layer: &str,
        own: Option<&str>,
        default_height: f64,
    ) -> f64 {
        match self.style_of_text(layers, layer, own) {
            Some(s) if !s.size_by_scale && s.height_in > 0.0 => s.height_in,
            _ => default_height,
        }
    }
}

// ===== rich text runs =====

/// A stretch of text in one format (Rich Text: bold, italic, underline, a
/// size scale and an optional colour).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RichRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Multiplies the text height; 1.0 is the base size.
    pub scale: f64,
    pub color: Option<[u8; 3]>,
}

impl Default for RichRun {
    fn default() -> Self {
        Self {
            text: String::new(),
            bold: false,
            italic: false,
            underline: false,
            scale: 1.0,
            color: None,
        }
    }
}

impl RichRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub fn bold(text: impl Into<String>) -> Self {
        Self {
            bold: true,
            ..Self::plain(text)
        }
    }

    pub fn italic(text: impl Into<String>) -> Self {
        Self {
            italic: true,
            ..Self::plain(text)
        }
    }

    pub fn underlined(text: impl Into<String>) -> Self {
        Self {
            underline: true,
            ..Self::plain(text)
        }
    }

    pub fn sized(text: impl Into<String>, scale: f64) -> Self {
        Self {
            scale,
            ..Self::plain(text)
        }
    }

    fn same_format(&self, o: &RichRun) -> bool {
        self.bold == o.bold
            && self.italic == o.italic
            && self.underline == o.underline
            && (self.scale - o.scale).abs() < 1e-9
            && self.color == o.color
    }
}

/// The runs' text with the formatting dropped.
pub fn runs_plain(runs: &[RichRun]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

/// Drops empty runs and joins neighbours of the same format.
pub fn merge_runs(runs: Vec<RichRun>) -> Vec<RichRun> {
    let mut out: Vec<RichRun> = Vec::new();
    for r in runs.into_iter().filter(|r| !r.text.is_empty()) {
        match out.last_mut() {
            Some(last) if last.same_format(&r) => last.text.push_str(&r.text),
            _ => out.push(r),
        }
    }
    out
}

fn escape_markup(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The runs as inline markup: `<b>`, `<i>`, `<u>`, `<size=1.5>` and
/// `<color=#RRGGBB>` around the text, with `&amp; &lt; &gt;` escapes.
pub fn runs_to_markup(runs: &[RichRun]) -> String {
    let mut out = String::new();
    for r in runs {
        let mut close: Vec<&str> = Vec::new();
        if let Some([cr, cg, cb]) = r.color {
            out.push_str(&format!("<color=#{cr:02X}{cg:02X}{cb:02X}>"));
            close.push("</color>");
        }
        if (r.scale - 1.0).abs() > 1e-9 {
            out.push_str(&format!("<size={}>", r.scale));
            close.push("</size>");
        }
        for (on, open, shut) in [
            (r.bold, "<b>", "</b>"),
            (r.italic, "<i>", "</i>"),
            (r.underline, "<u>", "</u>"),
        ] {
            if on {
                out.push_str(open);
                close.push(shut);
            }
        }
        out.push_str(&escape_markup(&r.text));
        for c in close.iter().rev() {
            out.push_str(c);
        }
    }
    out
}

/// Parses [`runs_to_markup`] output (and hand-written markup). Unknown tags
/// are kept as text; unmatched closing tags are ignored.
pub fn runs_from_markup(s: &str) -> Vec<RichRun> {
    #[derive(Clone)]
    struct Fmt {
        bold: bool,
        italic: bool,
        underline: bool,
        scale: f64,
        color: Option<[u8; 3]>,
    }
    let mut cur = Fmt {
        bold: false,
        italic: false,
        underline: false,
        scale: 1.0,
        color: None,
    };
    let mut stack: Vec<(String, Fmt)> = Vec::new();
    let mut runs: Vec<RichRun> = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, cur: &Fmt, runs: &mut Vec<RichRun>| {
        if !text.is_empty() {
            runs.push(RichRun {
                text: std::mem::take(text),
                bold: cur.bold,
                italic: cur.italic,
                underline: cur.underline,
                scale: cur.scale,
                color: cur.color,
            });
        }
    };
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '&' {
            let rest: String = chars[i..].iter().take(5).collect();
            for (ent, ch) in [("&amp;", '&'), ("&lt;", '<'), ("&gt;", '>')] {
                if rest.starts_with(ent) {
                    text.push(ch);
                    i += ent.len();
                    break;
                }
            }
            if !rest.starts_with("&amp;") && !rest.starts_with("&lt;") && !rest.starts_with("&gt;")
            {
                text.push('&');
                i += 1;
            }
            continue;
        }
        if c == '<' {
            if let Some(end) = chars[i..].iter().position(|ch| *ch == '>') {
                let tag: String = chars[i + 1..i + end].iter().collect();
                let (name, arg) = match tag.split_once('=') {
                    Some((n, a)) => (n.to_string(), Some(a.to_string())),
                    None => (tag.clone(), None),
                };
                let opened = match (name.as_str(), arg.as_deref()) {
                    ("b", None) | ("i", None) | ("u", None) => true,
                    ("size", Some(a)) => a.parse::<f64>().is_ok(),
                    ("color", Some(a)) => parse_hex_color(a).is_some(),
                    _ => false,
                };
                if opened {
                    flush(&mut text, &cur, &mut runs);
                    stack.push((name.clone(), cur.clone()));
                    match name.as_str() {
                        "b" => cur.bold = true,
                        "i" => cur.italic = true,
                        "u" => cur.underline = true,
                        "size" => {
                            cur.scale = arg.as_deref().and_then(|a| a.parse().ok()).unwrap_or(1.0)
                        }
                        _ => cur.color = arg.as_deref().and_then(parse_hex_color),
                    }
                    i += end + 1;
                    continue;
                }
                if let Some(close) = name.strip_prefix('/') {
                    if let Some(at) = stack.iter().rposition(|(n, _)| n == close) {
                        flush(&mut text, &cur, &mut runs);
                        cur = stack[at].1.clone();
                        stack.truncate(at);
                        i += end + 1;
                        continue;
                    }
                    if matches!(close, "b" | "i" | "u" | "size" | "color") {
                        i += end + 1;
                        continue;
                    }
                }
            }
        }
        text.push(c);
        i += 1;
    }
    flush(&mut text, &cur, &mut runs);
    merge_runs(runs)
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

// ===== text macros =====

/// The values the built-in text macros expand to; the caller fills in what it
/// knows (the room under the text, the plan, the floor).
#[derive(Debug, Clone, Default)]
pub struct MacroContext {
    pub room_name: String,
    pub room_number: String,
    pub room_area: String,
    pub plan_name: String,
    /// `YYYY-MM-DD`, see [`date_string`].
    pub plan_date: String,
    pub floor_name: String,
    /// 1-based.
    pub floor_number: usize,
    pub floor_count: usize,
    pub ceiling_height: String,
}

/// The built-in macros: `(name, what it gives)`. Written `%name%` in text.
pub const BUILT_IN_MACROS: &[(&str, &str)] = &[
    ("room.name", "Name of the room under the text"),
    ("room.number", "Number of the room under the text"),
    ("room.area", "Floor area of the room under the text"),
    ("plan.name", "Plan (project) name"),
    ("plan.date", "Today's date, YYYY-MM-DD"),
    ("floor", "Name of the floor"),
    ("floor.number", "Number of the floor, 1 is the lowest"),
    ("floor.count", "Number of floors in the plan"),
    ("floor.height", "Ceiling height of the floor"),
];

/// A user-defined macro: `%name%` expands to `text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextMacro {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMacros {
    pub macros: Vec<TextMacro>,
}

impl TextMacros {
    pub fn get(&self, name: &str) -> Option<&TextMacro> {
        self.macros.iter().find(|m| m.name == name)
    }

    /// Adds a macro; the name must be new, non-empty, made of letters,
    /// digits, `.`, `_` or `-`, and not a built-in.
    pub fn add(&mut self, name: &str, text: &str) -> bool {
        let ok = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
            && !BUILT_IN_MACROS.iter().any(|(n, _)| *n == name)
            && self.get(name).is_none();
        if ok {
            self.macros.push(TextMacro {
                name: name.to_string(),
                text: text.to_string(),
            });
        }
        ok
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.macros.len();
        self.macros.retain(|m| m.name != name);
        self.macros.len() != before
    }
}

fn builtin_value(name: &str, ctx: &MacroContext) -> Option<String> {
    Some(match name {
        "room.name" => ctx.room_name.clone(),
        "room.number" => ctx.room_number.clone(),
        "room.area" => ctx.room_area.clone(),
        "plan.name" => ctx.plan_name.clone(),
        "plan.date" => ctx.plan_date.clone(),
        "floor" => ctx.floor_name.clone(),
        "floor.number" => ctx.floor_number.to_string(),
        "floor.count" => ctx.floor_count.to_string(),
        "floor.height" => ctx.ceiling_height.clone(),
        _ => return None,
    })
}

/// Does the text contain a `%macro%` that [`expand_macros`] would replace?
pub fn has_macros(text: &str, user: &TextMacros) -> bool {
    BUILT_IN_MACROS
        .iter()
        .any(|(n, _)| text.contains(&format!("%{n}%")))
        || user
            .macros
            .iter()
            .any(|m| text.contains(&format!("%{}%", m.name)))
}

/// Replaces `%name%` with the built-in or user macro `name`. Unknown names
/// and stray `%` signs stay as typed. User macros may use other macros
/// (nested up to four deep).
pub fn expand_macros(text: &str, ctx: &MacroContext, user: &TextMacros) -> String {
    expand_depth(text, ctx, user, 0)
}

fn expand_depth(text: &str, ctx: &MacroContext, user: &TextMacros, depth: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        let is_name = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
        let value = if !is_name {
            None
        } else if let Some(v) = builtin_value(name, ctx) {
            Some(v)
        } else {
            user.get(name).map(|m| {
                if depth < 4 {
                    expand_depth(&m.text, ctx, user, depth + 1)
                } else {
                    m.text.clone()
                }
            })
        };
        match value {
            Some(v) => {
                out.push_str(&v);
                rest = &after[end + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `YYYY-MM-DD` (UTC) for a Unix time in seconds.
pub fn date_string(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// ===== note types =====

/// A kind of note (Note Type Management): its label prefix and text style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteType {
    pub name: String,
    /// Starts the note text: `"{prefix} {n}: {body}"`.
    pub prefix: String,
    /// Text style name; empty is the default style.
    pub style: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteTypes {
    pub types: Vec<NoteType>,
}

impl Default for NoteTypes {
    fn default() -> Self {
        let t = |name: &str, prefix: &str| NoteType {
            name: name.into(),
            prefix: prefix.into(),
            style: String::new(),
        };
        Self {
            types: vec![
                t("General Note", "Note"),
                t("Construction Note", "C"),
                t("Framing Note", "F"),
                t("Electrical Note", "E"),
            ],
        }
    }
}

impl NoteTypes {
    pub fn get(&self, name: &str) -> Option<&NoteType> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Adds a type with a new name and a non-empty prefix of letters and digits.
    pub fn add(&mut self, name: &str, prefix: &str) -> bool {
        let ok = !name.trim().is_empty()
            && self.get(name).is_none()
            && !prefix.is_empty()
            && prefix.chars().all(char::is_alphanumeric);
        if ok {
            self.types.push(NoteType {
                name: name.trim().to_string(),
                prefix: prefix.to_string(),
                style: String::new(),
            });
        }
        ok
    }

    /// Removes a type; the first one ("General Note") stays.
    pub fn remove(&mut self, name: &str) -> bool {
        if self.types.first().is_some_and(|t| t.name == name) {
            return false;
        }
        let before = self.types.len();
        self.types.retain(|t| t.name != name);
        self.types.len() != before
    }

    /// The text of note `n` of `type_name` (unknown names use the first type).
    pub fn format(&self, type_name: &str, n: u32, body: &str) -> String {
        let prefix = self
            .get(type_name)
            .or(self.types.first())
            .map_or("Note", |t| t.prefix.as_str());
        format!("{prefix} {n}: {body}")
    }

    /// Reads a note text back: its type name and number.
    pub fn parse(&self, text: &str) -> Option<(&str, u32)> {
        let mut best: Option<(&NoteType, u32)> = None;
        for t in &self.types {
            let Some(rest) = text.strip_prefix(&format!("{} ", t.prefix)) else {
                continue;
            };
            let Some((n, _)) = rest.split_once(':') else {
                continue;
            };
            let Ok(n) = n.trim().parse::<u32>() else {
                continue;
            };
            if best.is_none_or(|(b, _)| t.prefix.len() > b.prefix.len()) {
                best = Some((t, n));
            }
        }
        best.map(|(t, n)| (t.name.as_str(), n))
    }

    /// The next free number of `type_name` among `texts`.
    pub fn next_number<'a>(
        &self,
        type_name: &str,
        texts: impl IntoIterator<Item = &'a str>,
    ) -> u32 {
        texts
            .into_iter()
            .filter_map(|t| self.parse(t))
            .filter(|(name, _)| *name == type_name)
            .map(|(_, n)| n)
            .max()
            .unwrap_or(0)
            + 1
    }
}

impl crate::model::Project {
    /// The plan's user text macros.
    pub fn text_macros(&self) -> TextMacros {
        self.text_macros.clone()
    }

    pub fn set_text_macros(&mut self, m: &TextMacros) {
        self.text_macros = m.clone();
    }

    /// The plan's note types (the defaults until edited).
    pub fn note_types(&self) -> NoteTypes {
        self.note_types.clone()
    }

    pub fn set_note_types(&mut self, n: &NoteTypes) {
        self.note_types = n.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::Layer;

    #[test]
    fn chief_default_list() {
        let t = TextStyles::default();
        assert_eq!(
            t.names(),
            vec![
                "Default Text Style",
                "1/4\" Text Style",
                "Room Label Style",
                "Schedule Style",
                "Default Label Style",
                "Dimension Text Style"
            ]
        );
        let d = t.get("Default Text Style").unwrap();
        assert_eq!((d.font.as_str(), d.height_in), ("Arial", 6.0));
        // 1/8" printed = 9 pt.
        assert!((d.printed_pt.unwrap() - 9.0).abs() < 1e-9);
        assert!(t.get("Room Label Style").unwrap().bold);
    }

    #[test]
    fn lookup_and_layer_resolution() {
        let t = TextStyles::default();
        assert_eq!(t.resolve("Schedule Style").unwrap().name, "Schedule Style");
        assert_eq!(t.resolve("").unwrap().name, DEFAULT_TEXT_STYLE_NAME);
        assert_eq!(t.resolve("Nope").unwrap().name, DEFAULT_TEXT_STYLE_NAME);
        let mut layers = LayerSet::default_floor_plan();
        let mut l = Layer::new("Labels", [0, 0, 0], 18);
        l.text_style = "Room Label Style".into();
        layers.add(l);
        assert_eq!(
            t.resolve_for_layer(&layers, "Labels").unwrap().name,
            "Room Label Style"
        );
        assert_eq!(
            t.resolve_for_layer(&layers, "Text").unwrap().name,
            DEFAULT_TEXT_STYLE_NAME
        );
        assert_eq!(
            t.resolve_for_layer(&layers, "Unknown").unwrap().name,
            DEFAULT_TEXT_STYLE_NAME
        );
        let empty = TextStyles { styles: vec![] };
        assert!(empty.resolve("x").is_none());
    }

    #[test]
    fn add_remove_and_sizes() {
        let mut t = TextStyles::default();
        assert!(!t.add(TextStyle::plan_sized("Schedule Style", 1.0, false)));
        assert!(!t.add(TextStyle::plan_sized("", 1.0, false)));
        assert!(t.add(TextStyle::plan_sized("1/8\" Text Style", 12.0, false)));
        assert!(!t.remove(DEFAULT_TEXT_STYLE_NAME));
        assert!(t.remove("1/8\" Text Style"));
        assert!(!t.remove("1/8\" Text Style"));
        // Character-height styles ignore the scale; printed-size styles follow it.
        let mut s = TextStyle::default();
        assert_eq!(s.height_for_scale(0.5), 6.0);
        s.size_by_scale = false;
        assert!((s.height_for_scale(0.25) - 6.0).abs() < 1e-9);
        assert!((s.height_for_scale(0.125) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn printed_size_holds_on_paper_at_any_scale() {
        let mut s = TextStyle::default();
        // Character height: the object's plan height stays.
        assert_eq!(s.text_height(9.0, 0.125), 9.0);
        assert!((s.printed_in() - 0.125).abs() < 1e-9);
        s.use_printed_size(true);
        assert!(s.is_printed_size());
        // 1/8" on paper: 6" at 1/4" scale, 12" at 1/8", 3" at 1/2".
        assert!((s.text_height(6.0, 0.25) - 6.0).abs() < 1e-9);
        assert!((s.text_height(6.0, 0.125) - 12.0).abs() < 1e-9);
        assert!((s.text_height(6.0, 0.5) - 3.0).abs() < 1e-9);
        // An object 1.5 times the style stays 1.5 times it.
        assert!((s.text_height(9.0, 0.125) - 18.0).abs() < 1e-9);
        s.set_printed_in(0.25);
        assert!((s.plan_height_at(0.25, false) - 12.0).abs() < 1e-9);
        // A character-height style can still be forced to print size.
        let mut c = TextStyle::plan_sized("c", 4.5, false);
        assert_eq!(c.plan_height_at(0.125, false), 4.5);
        assert!((c.plan_height_at(0.125, true) - 9.0).abs() < 1e-9);
        c.use_printed_size(false);
        assert!(!c.is_printed_size());
    }

    #[test]
    fn drawn_and_placed_heights_follow_the_layer_style() {
        use crate::layers::LayerSet;
        let layers = LayerSet::default();
        let mut t = TextStyles::default();
        // Character height: placed at the tool's height, drawn as stored.
        assert_eq!(t.placed_height(&layers, "Text", None, 7.0), 7.0);
        assert_eq!(t.drawn_height(&layers, "Text", None, 7.0, 0.125), 7.0);
        let i = t
            .styles
            .iter()
            .position(|s| s.name == DEFAULT_TEXT_STYLE_NAME)
            .unwrap();
        t.styles[i].use_printed_size(true);
        // Printed size: placed at the style's own height, which draws at
        // 1/8" on paper at any scale.
        let h = t.placed_height(&layers, "Text", None, 7.0);
        assert_eq!(h, 6.0);
        for (ipf, plan) in [(0.25, 6.0), (0.125, 12.0), (0.5, 3.0)] {
            let d = t.drawn_height(&layers, "Text", None, h, ipf);
            assert!((d - plan).abs() < 1e-9, "{ipf}: {d}");
        }
        // An object's own style name wins over the layer's.
        assert_eq!(
            t.drawn_height(&layers, "Text", Some("Schedule Style"), 4.5, 0.125),
            4.5
        );
    }

    #[test]
    fn json_round_trip_and_sparse_style() {
        let t = TextStyles::default();
        let back: TextStyles = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        let s: TextStyle = serde_json::from_str(r#"{"name":"X","height_in":3.0}"#).unwrap();
        assert_eq!(s.font, "Arial");
        assert_eq!(s.height_in, 3.0);
        let empty: TextStyles = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, TextStyles::default());
    }

    #[test]
    fn rich_runs_round_trip_through_markup() {
        let runs = vec![
            RichRun::plain("Hello "),
            RichRun::bold("bold & "),
            RichRun {
                text: "both".into(),
                bold: true,
                italic: true,
                underline: true,
                scale: 1.5,
                color: Some([255, 0, 16]),
            },
            RichRun::sized(" <big> ", 2.0),
            RichRun::italic("end"),
        ];
        let markup = runs_to_markup(&runs);
        assert!(markup.contains("<b>bold &amp; </b>"));
        assert!(markup.contains("<size=1.5>"));
        assert_eq!(runs_from_markup(&markup), runs);
        assert_eq!(runs_plain(&runs), "Hello bold & both <big> end");
        // Neighbours of one format merge; empty runs vanish.
        let merged = merge_runs(vec![
            RichRun::bold("a"),
            RichRun::bold("b"),
            RichRun::plain(""),
            RichRun::plain("c"),
        ]);
        assert_eq!(merged, vec![RichRun::bold("ab"), RichRun::plain("c")]);
        // Hand-written, nested and sloppy markup.
        let r = runs_from_markup("<b>x<i>y</i></b></u>z");
        assert_eq!(
            r,
            vec![
                RichRun::bold("x"),
                RichRun {
                    bold: true,
                    italic: true,
                    ..RichRun::plain("y")
                },
                RichRun::plain("z")
            ]
        );
        let json = serde_json::to_string(&runs).unwrap();
        assert_eq!(serde_json::from_str::<Vec<RichRun>>(&json).unwrap(), runs);
    }

    #[test]
    fn macros_expand_built_in_and_user_names() {
        let ctx = MacroContext {
            room_name: "Kitchen".into(),
            plan_date: date_string(1_700_000_000),
            floor_name: "1st Floor".into(),
            floor_number: 1,
            floor_count: 2,
            ..MacroContext::default()
        };
        let mut user = TextMacros::default();
        assert!(user.add("firm", "Daniel Allen Designs"));
        assert!(user.add("stamp", "%firm% - %plan.date%"));
        assert!(!user.add("firm", "again") && !user.add("room.name", "x") && !user.add("a b", "x"));
        assert_eq!(
            expand_macros(
                "%room.name% on %floor% (%floor.number%/%floor.count%)",
                &ctx,
                &user
            ),
            "Kitchen on 1st Floor (1/2)"
        );
        assert_eq!(
            expand_macros("%stamp%", &ctx, &user),
            "Daniel Allen Designs - 2023-11-14"
        );
        // Unknown names and stray percent signs stay.
        assert_eq!(
            expand_macros("50% of %nope% and 20%", &ctx, &user),
            "50% of %nope% and 20%"
        );
        assert!(has_macros("%plan.date%", &user) && !has_macros("plain", &user));
        assert_eq!(date_string(0), "1970-01-01");
        assert_eq!(date_string(951_782_400), "2000-02-29");
        // A macro that names itself terminates.
        let mut loopy = TextMacros::default();
        loopy.add("a", "%a%");
        assert_eq!(expand_macros("%a%", &ctx, &loopy), "%a%");
    }

    #[test]
    fn note_types_number_per_type_and_parse_back() {
        let mut t = NoteTypes::default();
        assert_eq!(t.format("General Note", 3, "Verify"), "Note 3: Verify");
        assert_eq!(t.format("Electrical Note", 1, "GFCI"), "E 1: GFCI");
        assert_eq!(t.format("Missing", 2, "x"), "Note 2: x");
        assert_eq!(
            t.parse("C 12: Seal joints"),
            Some(("Construction Note", 12))
        );
        assert_eq!(t.parse("Nothing here"), None);
        let texts = ["Note 1: a", "Note 2: b", "E 7: c"];
        assert_eq!(t.next_number("General Note", texts), 3);
        assert_eq!(t.next_number("Electrical Note", texts), 8);
        assert_eq!(t.next_number("Framing Note", texts), 1);
        assert!(t.add("Plumbing Note", "P") && !t.add("Plumbing Note", "Q") && !t.add("x", "a b"));
        assert!(t.remove("Plumbing Note") && !t.remove("General Note"));
    }

    #[test]
    fn font_names_split_into_family_and_face_style() {
        assert_eq!(split_font_name("Avenir Book"), ("Avenir", "Book"));
        assert_eq!(split_font_name("avenir heavy"), ("avenir", "heavy"));
        assert_eq!(split_font_name("Arial"), ("Arial", ""));
        assert_eq!(split_font_name("Arial Black"), ("Arial Black", ""));
        assert_eq!(split_font_name("Book"), ("Book", ""));
        let s = TextStyle::plan_sized("x", 6.0, false).with_font("Avenir Book");
        assert_eq!((s.font_family(), s.font_face_style()), ("Avenir", "Book"));
        let s = TextStyle::plan_sized("x", 6.0, true)
            .with_font("Avenir")
            .with_font_style("Heavy");
        assert_eq!((s.font_family(), s.font_face_style()), ("Avenir", "Heavy"));
        assert!(same_font_family("Avenir", "AVENIR book"));
        assert!(!same_font_family("Arial", "Arial Narrow"));
    }

    #[test]
    fn replace_font_changes_every_style_of_the_family() {
        let mut t = TextStyles::chief_defaults();
        t.styles[1].font = "Avenir".into();
        t.styles[1].font_style = "Heavy".into();
        t.styles[2].font = "Avenir Book".into();
        assert_eq!(t.fonts_used(), vec!["Arial", "Avenir"]);
        assert_eq!(t.replace_font("Arial", "Helvetica Neue"), 4);
        assert_eq!(t.replace_font("avenir", "Georgia"), 2);
        assert!(t
            .styles
            .iter()
            .all(|s| s.font != "Arial" && s.font != "Avenir"));
        assert!(t.styles[1].font_style.is_empty());
        assert_eq!(t.fonts_used(), vec!["Georgia", "Helvetica Neue"]);
        // Nothing to do for the same family or an empty target.
        assert_eq!(t.replace_font("Georgia", "georgia"), 0);
        assert_eq!(t.replace_font("Georgia", " "), 0);
    }

    #[test]
    fn a_style_without_a_font_style_field_loads() {
        let s: TextStyle = serde_json::from_str(r#"{"name":"A","font":"Avenir"}"#).unwrap();
        assert!(s.font_style.is_empty());
        let back: TextStyle = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn macros_and_note_types_persist_in_the_project() {
        let mut p = crate::model::Project::new("Test");
        assert!(p.text_macros().macros.is_empty());
        assert_eq!(p.note_types(), NoteTypes::default());
        let mut m = TextMacros::default();
        m.add("firm", "DAD");
        p.set_text_macros(&m);
        let mut n = NoteTypes::default();
        n.add("Plumbing Note", "P");
        p.set_note_types(&n);
        let back: crate::model::Project =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back.text_macros(), m);
        assert_eq!(back.note_types(), n);
        p.set_text_macros(&TextMacros::default());
        assert!(p.text_macros().macros.is_empty());
        assert!(p.text_macros.macros.is_empty());
    }
}
