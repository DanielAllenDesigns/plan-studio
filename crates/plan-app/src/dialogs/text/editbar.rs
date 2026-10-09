//! The Rich Text Edit Bar (TXT-29, TXT-31, TXT-33; manual pp. 527 to 530):
//! font family, text size and Print Size, colour, bold, italic, underline,
//! strikethrough, uppercase, alignment, bullets and numbering (Paragraph
//! Options), Insert Hyperlink, Insert Macro and Spell Check.
//!
//! The text of a Rich Text object is edited as markup
//! (`plan_core::text_styles::runs_to_markup`): every button here wraps the
//! selected stretch of the markup in the tags of its format (`<b>`, `<s>`,
//! `<upper>`, `<font=Avenir>`, `<size=1.5>`, `<color=#RRGGBB>`,
//! `<link=https://...>`), so the formats end up on the runs. The functions
//! that change the markup are plain and tested; [`show`] draws the bar.

use eframe::egui::{self, Ui};
use plan_core::text_box::HAlign;

/// What a Paragraph Options list is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ListKind {
    #[default]
    None,
    Bullet,
    Numbered,
    Lettered,
}

impl ListKind {
    pub const ALL: [ListKind; 4] = [
        ListKind::None,
        ListKind::Bullet,
        ListKind::Numbered,
        ListKind::Lettered,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ListKind::None => "None",
            ListKind::Bullet => "Bullets",
            ListKind::Numbered => "1. 2. 3.",
            ListKind::Lettered => "a. b. c.",
        }
    }

    /// The prefix of item `n` (0 based) of the list.
    pub fn prefix(self, n: usize) -> String {
        match self {
            ListKind::None => String::new(),
            ListKind::Bullet => "\u{2022} ".to_string(),
            ListKind::Numbered => format!("{}. ", n + 1),
            ListKind::Lettered => {
                let c = (b'a' + (n % 26) as u8) as char;
                format!("{c}. ")
            }
        }
    }
}

/// The colour a hyperlink is set in.
pub const LINK_COLOR: &str = "#0000EE";

fn byte_of(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i)
}

fn clamp_sel(s: &str, sel: (usize, usize)) -> (usize, usize) {
    let n = s.chars().count();
    let (a, b) = (sel.0.min(sel.1).min(n), sel.0.max(sel.1).min(n));
    (a, b)
}

/// `markup` with `open` before and `close` after the selected characters
/// (an empty selection gets both at the cursor, to type between). Returns
/// the new markup and the selection that keeps covering the same words.
pub fn wrap_selection(
    markup: &str,
    sel: (usize, usize),
    open: &str,
    close: &str,
) -> (String, (usize, usize)) {
    let (a, b) = clamp_sel(markup, sel);
    let (ba, bb) = (byte_of(markup, a), byte_of(markup, b));
    let mut out = String::with_capacity(markup.len() + open.len() + close.len());
    out.push_str(&markup[..ba]);
    out.push_str(open);
    out.push_str(&markup[ba..bb]);
    out.push_str(close);
    out.push_str(&markup[bb..]);
    let o = open.chars().count();
    (out, (a + o, b + o))
}

/// Toggles the format `tag` (`b`, `i`, `u`, `s`, `upper`) on the selection:
/// when the selection is already inside that tag, the tag comes off.
pub fn toggle_tag(markup: &str, sel: (usize, usize), tag: &str) -> (String, (usize, usize)) {
    let (a, b) = clamp_sel(markup, sel);
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let (ba, bb) = (byte_of(markup, a), byte_of(markup, b));
    if markup[..ba].ends_with(&open) && markup[bb..].starts_with(&close) {
        let mut out = String::with_capacity(markup.len());
        out.push_str(&markup[..ba - open.len()]);
        out.push_str(&markup[ba..bb]);
        out.push_str(&markup[bb + close.len()..]);
        let o = open.chars().count();
        return (out, (a - o, b - o));
    }
    wrap_selection(markup, (a, b), &open, &close)
}

/// Sets the font family of the selection.
pub fn set_font(markup: &str, sel: (usize, usize), family: &str) -> (String, (usize, usize)) {
    wrap_selection(markup, sel, &format!("<font={family}>"), "</font>")
}

/// Sets the size of the selection to `size` plan inches where the text is
/// `base` tall (a scale of the text height).
pub fn set_size(
    markup: &str,
    sel: (usize, usize),
    size: f64,
    base: f64,
) -> (String, (usize, usize)) {
    let scale = if base > 0.0 { size / base } else { 1.0 };
    let scale = (scale * 1000.0).round() / 1000.0;
    wrap_selection(markup, sel, &format!("<size={scale}>"), "</size>")
}

/// Sets the colour of the selection.
pub fn set_color(markup: &str, sel: (usize, usize), rgb: [u8; 3]) -> (String, (usize, usize)) {
    wrap_selection(
        markup,
        sel,
        &format!("<color=#{:02X}{:02X}{:02X}>", rgb[0], rgb[1], rgb[2]),
        "</color>",
    )
}

/// Insert Hyperlink: the selection becomes (or is replaced by) `text` linked
/// to `address`, underlined and blue. An empty address removes the link.
pub fn insert_hyperlink(
    markup: &str,
    sel: (usize, usize),
    text: &str,
    address: &str,
) -> (String, (usize, usize)) {
    let (a, b) = clamp_sel(markup, sel);
    let (ba, bb) = (byte_of(markup, a), byte_of(markup, b));
    let linked = if address.trim().is_empty() {
        text.to_string()
    } else {
        format!(
            "<link={}><u><color={LINK_COLOR}>{text}</color></u></link>",
            address.trim().replace('>', "%3E")
        )
    };
    let mut out = String::new();
    out.push_str(&markup[..ba]);
    out.push_str(&linked);
    out.push_str(&markup[bb..]);
    (out, (a, a + linked.chars().count()))
}

/// Paragraph Options: the lines the selection touches get `kind` as their
/// list (numbered or lettered lists count from the first of them). Any list
/// prefix the lines already have is replaced.
pub fn set_list(markup: &str, sel: (usize, usize), kind: ListKind) -> (String, (usize, usize)) {
    let (a, b) = clamp_sel(markup, sel);
    let mut lines: Vec<String> = markup.split('\n').map(str::to_string).collect();
    let mut start = 0usize;
    let mut first: Option<usize> = None;
    let mut last = 0usize;
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        // The line holds characters start..=start+n (its newline included).
        if a <= start + n && b >= start {
            first.get_or_insert(i);
            last = i;
        }
        start += n + 1;
    }
    let Some(first) = first else {
        return (markup.to_string(), (a, b));
    };
    let mut delta: isize = 0;
    for (k, line) in lines[first..=last].iter_mut().enumerate() {
        let stripped = strip_list_prefix(line);
        let new = format!("{}{}", kind.prefix(k), stripped);
        delta += new.chars().count() as isize - line.chars().count() as isize;
        *line = new;
    }
    let out = lines.join("\n");
    let end = (b as isize + delta).max(a as isize) as usize;
    (out, (a, end))
}

/// A line without the bullet, number or letter that starts it.
pub fn strip_list_prefix(line: &str) -> String {
    if let Some(rest) = line.strip_prefix("\u{2022} ") {
        return rest.to_string();
    }
    let b = line.as_bytes();
    let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if d > 0 && b.get(d) == Some(&b'.') && b.get(d + 1) == Some(&b' ') {
        return line[d + 2..].to_string();
    }
    if b.len() >= 3 && b[0].is_ascii_lowercase() && b[1] == b'.' && b[2] == b' ' {
        return line[3..].to_string();
    }
    line.to_string()
}

/// The list kind a line starts with.
pub fn list_kind_of(line: &str) -> ListKind {
    if line.starts_with("\u{2022} ") {
        ListKind::Bullet
    } else if strip_list_prefix(line) != line {
        if line.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            ListKind::Numbered
        } else {
            ListKind::Lettered
        }
    } else {
        ListKind::None
    }
}

/// The links of a markup: `(text, address)` of each hyperlink run.
pub fn links_of(markup: &str) -> Vec<(String, String)> {
    plan_core::text_styles::runs_from_markup(markup)
        .into_iter()
        .filter_map(|r| r.link.map(|l| (r.text, l)))
        .collect()
}

// ----- the bar -----

/// What the bar remembers between frames.
#[derive(Default)]
pub struct EditBarState {
    /// The characters selected in the text field (it keeps them while the
    /// bar has the pointer).
    pub sel: (usize, usize),
    pub link_open: bool,
    pub link_text: String,
    pub link_addr: String,
    pub size: f64,
    pub print_open: bool,
    pub printed: f64,
    pub paragraph_open: bool,
    pub list: ListKind,
    pub color: [u8; 3],
    pub font: String,
}

/// What the bar needs to know about the text it edits.
pub struct BarEnv<'a> {
    /// The text height of the object (plan inches).
    pub base: f64,
    /// Paper inches per foot of the sheet scale (for Print Size).
    pub ipf: f64,
    /// Font families to pick from.
    pub families: &'a [String],
}

/// One click on the bar.
enum Op {
    Font(String),
    Size(f64),
    Color([u8; 3]),
    Tag(&'static str),
    List(ListKind),
    Link(String, String),
}

/// Draws the Edit Bar and applies a click to `markup`. Returns true when
/// the markup (or the alignment) changed.
pub fn show(
    ui: &mut Ui,
    markup: &mut String,
    halign: &mut HAlign,
    st: &mut EditBarState,
    env: &BarEnv,
) -> bool {
    let mut changed = false;
    let mut op: Option<Op> = None;
    if st.size <= 0.0 {
        st.size = env.base;
    }
    ui.horizontal_wrapped(|ui| {
        // Font family.
        egui::ComboBox::from_id_salt("editbar_font")
            .width(120.0)
            .selected_text(if st.font.is_empty() {
                "Font".to_string()
            } else {
                st.font.clone()
            })
            .show_ui(ui, |ui| {
                for f in env.families {
                    if ui.selectable_label(st.font == *f, f.as_str()).clicked() {
                        st.font.clone_from(f);
                        op = Some(Op::Font(f.clone()));
                    }
                }
            });
        // Size in drawing units; Print Size.
        let mut size = st.size;
        if ui
            .add(
                egui::DragValue::new(&mut size)
                    .range(0.25..=400.0)
                    .speed(0.25)
                    .suffix("\""),
            )
            .on_hover_text("Text Size in drawing units; applies to the selection")
            .changed()
        {
            st.size = size;
            op = Some(Op::Size(size));
        }
        if ui
            .button("Print Size")
            .on_hover_text("Size from a printed size at the sheet scale")
            .clicked()
        {
            st.print_open = !st.print_open;
        }
        // Colour.
        if ui.color_edit_button_srgb(&mut st.color).changed() {
            op = Some(Op::Color(st.color));
        }
        if ui
            .button("Link")
            .on_hover_text("Insert Hyperlink")
            .clicked()
        {
            st.link_open = !st.link_open;
            if st.link_open {
                let (a, b) = clamp_sel(markup, st.sel);
                let (ba, bb) = (byte_of(markup, a), byte_of(markup, b));
                let selected: String = markup[ba..bb].to_string();
                // Existing link text and address populate the dialog.
                match links_of(&selected).into_iter().next() {
                    Some((t, addr)) => {
                        st.link_text = t;
                        st.link_addr = addr;
                    }
                    None => {
                        st.link_text = plan_core::text_styles::runs_plain(
                            &plan_core::text_styles::runs_from_markup(&selected),
                        );
                        st.link_addr.clear();
                    }
                }
            }
        }
        ui.separator();
        for (label, tag, tip) in [
            ("B", "b", "Bold (Cmd+B)"),
            ("I", "i", "Italic (Cmd+I)"),
            ("U", "u", "Underline (Cmd+U)"),
            ("S", "s", "Strikethrough"),
            ("AA", "upper", "Uppercase"),
        ] {
            if ui.button(label).on_hover_text(tip).clicked() {
                op = Some(Op::Tag(tag));
            }
        }
        ui.separator();
        for (label, a) in [
            ("Left", HAlign::Left),
            ("Center", HAlign::Center),
            ("Right", HAlign::Right),
        ] {
            if ui.selectable_label(*halign == a, label).clicked() {
                *halign = a;
                changed = true;
            }
        }
        if ui
            .button("Paragraph")
            .on_hover_text("Paragraph Options: bullets and numbering")
            .clicked()
        {
            st.paragraph_open = !st.paragraph_open;
        }
    });
    if st.print_open {
        ui.horizontal(|ui| {
            ui.label("Printed size (paper inches)");
            ui.add(
                egui::DragValue::new(&mut st.printed)
                    .range(0.0..=2.0)
                    .speed(0.01)
                    .fixed_decimals(3),
            );
            let plan = if env.ipf > 0.0 {
                st.printed * 12.0 / env.ipf
            } else {
                0.0
            };
            ui.weak(format!("= {plan:.2}\" in the plan at this scale"));
            if ui.button("Apply").clicked() && plan > 0.0 {
                st.size = plan;
                op = Some(Op::Size(plan));
                st.print_open = false;
            }
        });
    }
    if st.paragraph_open {
        ui.horizontal(|ui| {
            ui.label("Bullets and numbering");
            for k in ListKind::ALL {
                if ui.selectable_label(st.list == k, k.label()).clicked() {
                    st.list = k;
                    op = Some(Op::List(k));
                }
            }
        });
    }
    if st.link_open {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.add(egui::TextEdit::singleline(&mut st.link_text).desired_width(140.0));
            ui.label("Address");
            ui.add(egui::TextEdit::singleline(&mut st.link_addr).desired_width(220.0));
            if ui.button("OK").clicked() {
                op = Some(Op::Link(st.link_text.clone(), st.link_addr.clone()));
                st.link_open = false;
            }
            if ui.button("Cancel").clicked() {
                st.link_open = false;
            }
        });
    }
    if let Some(op) = op {
        let sel = st.sel;
        let (m, s) = match op {
            Op::Font(f) => set_font(markup, sel, &f),
            Op::Size(v) => set_size(markup, sel, v, env.base),
            Op::Color(c) => set_color(markup, sel, c),
            Op::Tag(t) => toggle_tag(markup, sel, t),
            Op::List(k) => set_list(markup, sel, k),
            Op::Link(t, a) => insert_hyperlink(markup, sel, &t, &a),
        };
        *markup = m;
        st.sel = s;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::text_styles::{runs_from_markup, RichRun};

    #[test]
    fn wrap_selects_the_same_words_and_an_empty_selection_types_between() {
        let (m, s) = wrap_selection("hello world", (6, 11), "<b>", "</b>");
        assert_eq!(m, "hello <b>world</b>");
        assert_eq!(s, (9, 14));
        let (m, s) = wrap_selection("ab", (1, 1), "<i>", "</i>");
        assert_eq!(m, "a<i></i>b");
        assert_eq!(s, (4, 4));
        // Multi-byte characters count as one.
        let (m, _) = wrap_selection("\u{b0}C x", (0, 2), "<u>", "</u>");
        assert_eq!(m, "<u>\u{b0}C</u> x");
    }

    #[test]
    fn toggling_a_tag_twice_restores_the_text() {
        let (m, s) = toggle_tag("one two", (4, 7), "b");
        assert_eq!(m, "one <b>two</b>");
        let (m2, s2) = toggle_tag(&m, s, "b");
        assert_eq!(m2, "one two");
        assert_eq!(s2, (4, 7));
        let runs = runs_from_markup(&toggle_tag("x y", (2, 3), "s").0);
        assert!(runs.iter().any(|r| r.strike && r.text == "y"));
        let runs = runs_from_markup(&toggle_tag("x y", (0, 1), "upper").0);
        assert!(runs.iter().any(|r| r.upper && r.text == "x"));
    }

    #[test]
    fn font_size_and_color_reach_the_runs() {
        let (m, _) = set_font("Plan notes", (0, 4), "Avenir");
        let (m, _) = set_size(&m, (0, 4), 9.0, 6.0);
        let (m, _) = set_color(&m, (0, 4), [200, 0, 0]);
        let runs = runs_from_markup(&m);
        let first = &runs[0];
        assert_eq!(first.text, "Plan");
        assert_eq!(first.font.as_deref(), Some("Avenir"));
        assert!((first.scale - 1.5).abs() < 1e-9);
        assert_eq!(first.color, Some([200, 0, 0]));
        assert_eq!(runs[1], RichRun::plain(" notes"));
    }

    #[test]
    fn a_hyperlink_is_a_blue_underlined_linked_run_and_can_be_removed() {
        let (m, s) = insert_hyperlink("see site now", (4, 8), "site", "https://example.com");
        let runs = runs_from_markup(&m);
        let link = runs.iter().find(|r| r.link.is_some()).expect("a link run");
        assert_eq!(link.text, "site");
        assert!(link.underline);
        assert_eq!(link.color, Some([0, 0, 0xEE]));
        assert_eq!(link.link.as_deref(), Some("https://example.com"));
        assert_eq!(
            links_of(&m),
            vec![("site".to_string(), "https://example.com".to_string())]
        );
        // An empty address removes the link, keeping the words.
        let selected: String = m.chars().skip(s.0).take(s.1 - s.0).collect();
        assert!(selected.contains("<link="));
        let (gone, _) = insert_hyperlink(&m, s, "site", "");
        assert_eq!(gone, "see site now");
        assert!(links_of(&gone).is_empty());
    }

    #[test]
    fn lists_prefix_the_lines_the_selection_touches() {
        let text = "one\ntwo\nthree";
        let (m, _) = set_list(text, (0, 7), ListKind::Numbered);
        assert_eq!(m, "1. one\n2. two\nthree");
        let (m, _) = set_list(&m, (0, 3), ListKind::Bullet);
        assert_eq!(m, "\u{2022} one\n2. two\nthree");
        let (m, _) = set_list(&m, (0, m.chars().count()), ListKind::Lettered);
        assert_eq!(m, "a. one\nb. two\nc. three");
        let (m, _) = set_list(&m, (0, m.chars().count()), ListKind::None);
        assert_eq!(m, text);
        assert_eq!(list_kind_of("1. x"), ListKind::Numbered);
        assert_eq!(list_kind_of("b. x"), ListKind::Lettered);
        assert_eq!(list_kind_of("\u{2022} x"), ListKind::Bullet);
        assert_eq!(list_kind_of("plain"), ListKind::None);
        // A cursor in the middle line numbers just that line.
        let (m, _) = set_list(text, (5, 5), ListKind::Numbered);
        assert_eq!(m, "one\n1. two\nthree");
    }

    #[test]
    fn the_bar_draws() {
        let mut markup = "Hello".to_string();
        let mut h = HAlign::Left;
        let mut st = EditBarState {
            link_open: true,
            print_open: true,
            paragraph_open: true,
            ..EditBarState::default()
        };
        let families = vec!["Avenir".to_string(), "Arial".to_string()];
        let env = BarEnv {
            base: 6.0,
            ipf: 0.25,
            families: &families,
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, &mut markup, &mut h, &mut st, &env);
            });
        });
        assert_eq!(markup, "Hello");
    }
}
