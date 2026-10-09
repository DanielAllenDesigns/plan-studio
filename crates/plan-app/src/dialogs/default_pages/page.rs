//! The generic Default Settings page: a list of sections and fields (a
//! [`PageSpec`]), a draft of their values and the window that edits them.
//!
//! A field is either *bound* to a typed slot of `PlanDefaults` (a getter and
//! a setter) or *stored*: its value lives in `PlanDefaults::pages` under
//! `"<page>.<field>"` and only a value that differs from the field's
//! built-in value is written, so a template file holds just the changes.
//! OK writes the draft; Reset Page puts back the built-in values of the page.

use crate::dialogs::{row, section, Outcome};
use eframe::egui::{self, Align, Align2, Color32, Key, Layout, Modifiers};
use plan_core::defaults::{PageValue, PlanDefaults};
use plan_core::units::{fmt_ft_in, parse_ft_in};
use std::collections::{BTreeMap, HashMap};

/// Error text on the dialog's dark panel.
const ERROR_RED: Color32 = Color32::from_rgb(0xFF, 0x7B, 0x7B);

/// A getter and setter over a typed slot of the plan defaults.
#[derive(Clone, Copy)]
pub struct Bind {
    pub get: fn(&PlanDefaults) -> PageValue,
    pub set: fn(&mut PlanDefaults, &PageValue),
}

/// Where a list-valued field takes its choices from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListSource {
    WallTypes,
    TextStyles,
    DimensionSets,
    RoomTypes,
}

/// What a field edits and how it is drawn.
#[derive(Clone)]
pub enum Kind {
    Flag,
    /// Inches, shown as feet and inches.
    Length,
    /// Degrees.
    Degrees,
    /// A decimal number with a unit label.
    Number(&'static str),
    /// A whole number in a range with a unit label.
    Int(i64, i64, &'static str),
    /// One of a fixed list of names.
    Choice(Vec<&'static str>),
    /// One of a list taken from the plan defaults.
    List(ListSource),
    Text,
    /// `#RRGGBB`.
    Color,
}

/// One setting of a page.
#[derive(Clone)]
pub struct Field {
    /// `"<page>.<field>"`; the key under `PlanDefaults::pages` for a stored
    /// field.
    pub key: String,
    pub label: String,
    pub kind: Kind,
    /// The built-in value.
    pub default: PageValue,
    pub bind: Option<Bind>,
}

impl Field {
    fn new(key: &str, label: &str, kind: Kind, default: PageValue) -> Self {
        Field {
            key: key.to_string(),
            label: label.to_string(),
            kind,
            default,
            bind: None,
        }
    }

    /// A checkbox.
    pub fn flag(key: &str, label: &str, default: bool) -> Self {
        Self::new(key, label, Kind::Flag, PageValue::Bool(default))
    }

    /// A length in inches.
    pub fn len(key: &str, label: &str, default: f64) -> Self {
        Self::new(key, label, Kind::Length, PageValue::Num(default))
    }

    /// An angle in degrees.
    pub fn deg(key: &str, label: &str, default: f64) -> Self {
        Self::new(key, label, Kind::Degrees, PageValue::Num(default))
    }

    /// A decimal number followed by `unit`.
    pub fn num(key: &str, label: &str, default: f64, unit: &'static str) -> Self {
        Self::new(key, label, Kind::Number(unit), PageValue::Num(default))
    }

    /// A whole number in `min..=max` followed by `unit`.
    pub fn int(
        key: &str,
        label: &str,
        default: i64,
        range: (i64, i64),
        unit: &'static str,
    ) -> Self {
        Self::new(
            key,
            label,
            Kind::Int(range.0, range.1, unit),
            PageValue::Int(default),
        )
    }

    /// One of `names`; `default` is one of them.
    pub fn pick(key: &str, label: &str, names: &[&'static str], default: &str) -> Self {
        Self::new(
            key,
            label,
            Kind::Choice(names.to_vec()),
            PageValue::Text(default.to_string()),
        )
    }

    /// One of a list the plan defaults hold.
    pub fn list(key: &str, label: &str, source: ListSource, default: &str) -> Self {
        Self::new(
            key,
            label,
            Kind::List(source),
            PageValue::Text(default.to_string()),
        )
    }

    /// Free text.
    pub fn text(key: &str, label: &str, default: &str) -> Self {
        Self::new(key, label, Kind::Text, PageValue::Text(default.to_string()))
    }

    /// A colour as `#RRGGBB`.
    pub fn color(key: &str, label: &str, default: &str) -> Self {
        Self::new(
            key,
            label,
            Kind::Color,
            PageValue::Text(default.to_string()),
        )
    }

    /// Edits a typed slot of the plan defaults instead of the stored value.
    pub fn bound(mut self, bind: Bind) -> Self {
        self.bind = Some(bind);
        self
    }

    /// The field's value in `d`.
    pub fn read(&self, d: &PlanDefaults) -> PageValue {
        match &self.bind {
            Some(b) => (b.get)(d),
            None => d
                .page_value(&self.key)
                .cloned()
                .unwrap_or_else(|| self.default.clone()),
        }
    }

    /// Writes `v` into `d`.
    pub fn write(&self, d: &mut PlanDefaults, v: &PageValue) {
        match &self.bind {
            Some(b) => (b.set)(d, v),
            None => d.set_page_value(&self.key, v.clone(), &self.default),
        }
    }

    /// Does the field hold a value of the kind it needs?
    fn valid(&self, v: &PageValue) -> bool {
        match &self.kind {
            Kind::Color => {
                let t = v.text();
                t.len() == 7 && t.starts_with('#') && t[1..].chars().all(|c| c.is_ascii_hexdigit())
            }
            Kind::Length | Kind::Degrees | Kind::Number(_) => v.num().is_finite(),
            _ => true,
        }
    }
}

/// A titled group of fields.
#[derive(Clone)]
pub struct Section {
    pub title: String,
    pub fields: Vec<Field>,
}

/// A page: its id (the prefix of its stored keys), window title and fields.
#[derive(Clone)]
pub struct PageSpec {
    pub id: String,
    pub title: String,
    pub sections: Vec<Section>,
    /// A line under the title ("Saved with the defaults").
    pub note: String,
}

impl PageSpec {
    pub fn new(id: &str, title: &str) -> Self {
        PageSpec {
            id: id.to_string(),
            title: title.to_string(),
            sections: Vec::new(),
            note: String::new(),
        }
    }

    /// Adds a section.
    pub fn section(mut self, title: &str, fields: Vec<Field>) -> Self {
        self.sections.push(Section {
            title: title.to_string(),
            fields,
        });
        self
    }

    /// Sets the note line.
    pub fn note(mut self, note: &str) -> Self {
        self.note = note.to_string();
        self
    }

    /// Every field, in page order.
    pub fn fields(&self) -> impl Iterator<Item = &Field> {
        self.sections.iter().flat_map(|s| s.fields.iter())
    }

    /// The field stored under `key`.
    pub fn field(&self, key: &str) -> Option<&Field> {
        self.fields().find(|f| f.key == key)
    }

    /// Does any field write to `PlanDefaults::pages` (rather than only to
    /// typed slots)?
    pub fn has_stored(&self) -> bool {
        self.fields().any(|f| f.bind.is_none())
    }
}

/// The choices of the list-valued fields, read when the page opens.
#[derive(Default, Clone)]
pub struct Lists {
    pub wall_types: Vec<String>,
    pub text_styles: Vec<String>,
    pub dimension_sets: Vec<String>,
    pub room_types: Vec<String>,
}

impl Lists {
    pub fn of(d: &PlanDefaults) -> Self {
        Lists {
            wall_types: d.wall_types.iter().map(|t| t.name.clone()).collect(),
            text_styles: d
                .text_styles
                .names()
                .iter()
                .map(|n| n.to_string())
                .collect(),
            dimension_sets: d.dimension_sets.iter().map(|s| s.name.clone()).collect(),
            room_types: d.rooms.room_types.iter().map(|t| t.name.clone()).collect(),
        }
    }

    fn get(&self, s: ListSource) -> &[String] {
        match s {
            ListSource::WallTypes => &self.wall_types,
            ListSource::TextStyles => &self.text_styles,
            ListSource::DimensionSets => &self.dimension_sets,
            ListSource::RoomTypes => &self.room_types,
        }
    }
}

/// An open page: the spec, the values being edited and the text of the
/// fields being typed into.
pub struct GenericPage {
    spec: PageSpec,
    draft: BTreeMap<String, PageValue>,
    bufs: HashMap<String, String>,
    lists: Lists,
}

impl GenericPage {
    pub fn new(spec: PageSpec, d: &PlanDefaults) -> Self {
        let draft = spec.fields().map(|f| (f.key.clone(), f.read(d))).collect();
        GenericPage {
            spec,
            draft,
            bufs: HashMap::new(),
            lists: Lists::of(d),
        }
    }

    pub fn spec(&self) -> &PageSpec {
        &self.spec
    }

    /// The draft value of field `key`.
    pub fn get(&self, key: &str) -> Option<&PageValue> {
        self.draft.get(key)
    }

    /// Sets the draft value of field `key`.
    pub fn set(&mut self, key: &str, v: PageValue) {
        if self.draft.contains_key(key) {
            self.bufs.remove(key);
            self.draft.insert(key.to_string(), v);
        }
    }

    /// Puts the built-in values of the page back into the draft.
    pub fn reset(&mut self) {
        let builtin = PlanDefaults::default();
        self.bufs.clear();
        for f in self.spec.fields() {
            let v = match &f.bind {
                Some(_) => f.read(&builtin),
                None => f.default.clone(),
            };
            self.draft.insert(f.key.clone(), v);
        }
    }

    /// Why OK is off, if it is.
    pub fn error(&self) -> Option<String> {
        for f in self.spec.fields() {
            if let Some(buf) = self.bufs.get(&f.key) {
                if parse_text(&f.kind, buf).is_none() {
                    return Some(format!("{}: enter a valid value", f.label));
                }
            }
            if let Some(v) = self.draft.get(&f.key) {
                if !f.valid(v) {
                    return Some(format!("{}: enter a valid value", f.label));
                }
            }
        }
        None
    }

    /// Writes the draft into `d`.
    pub fn apply(&self, d: &mut PlanDefaults) {
        for f in self.spec.fields() {
            if let Some(v) = self.draft.get(&f.key) {
                f.write(d, v);
            }
        }
    }

    /// Draws the window; the outcome is `Ok` or `Cancel` once answered.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let mut reset = false;
        let error = self.error();
        let spec = self.spec.clone();
        egui::Window::new(format!("Default Settings: {}", spec.title))
            .id(egui::Id::new(("default_page", &spec.id)))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([480.0, 480.0])
            .min_size([360.0, 240.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let height = (ui.available_height() - 64.0).max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt(("default_page_scroll", &spec.id))
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !spec.note.is_empty() {
                            ui.weak(&spec.note);
                        }
                        if spec.has_stored() {
                            ui.weak("* Stored with the defaults; the tools do not read these yet.");
                        }
                        for s in &spec.sections {
                            if !s.title.is_empty() {
                                section(ui, &s.title);
                            }
                            for f in &s.fields {
                                self.field(ui, f);
                            }
                        }
                    });
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(error.is_none(), egui::Button::new("   OK   "))
                        .clicked()
                    {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if ui.button("Reset Page").clicked() {
                        reset = true;
                    }
                    if let Some(e) = &error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if reset {
            self.reset();
        }
        if !open {
            outcome = Outcome::Cancel;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }

    fn field(&mut self, ui: &mut egui::Ui, f: &Field) {
        let Some(mut value) = self.draft.get(&f.key).cloned() else {
            return;
        };
        let before = value.clone();
        // A field without a typed slot is kept with the defaults only.
        let label = if f.bind.is_none() {
            format!("{} *", f.label)
        } else {
            f.label.clone()
        };
        match &f.kind {
            Kind::Flag => {
                let mut b = value.flag();
                if ui.checkbox(&mut b, &label).changed() {
                    value = PageValue::Bool(b);
                }
            }
            Kind::Int(lo, hi, unit) => {
                let mut n = value.int().clamp(*lo, *hi);
                row(ui, &label, |ui| {
                    ui.add(egui::DragValue::new(&mut n).range(*lo..=*hi));
                    if !unit.is_empty() {
                        ui.label(*unit);
                    }
                });
                value = PageValue::Int(n);
            }
            Kind::Choice(names) => {
                let mut cur = value.text();
                row(ui, &label, |ui| {
                    combo(
                        ui,
                        &f.key,
                        &mut cur,
                        names.iter().map(|n| n.to_string()).collect(),
                    );
                });
                value = PageValue::Text(cur);
            }
            Kind::List(src) => {
                let mut cur = value.text();
                let mut names = self.lists.get(*src).to_vec();
                if !cur.is_empty() && !names.contains(&cur) {
                    names.insert(0, cur.clone());
                }
                row(ui, &label, |ui| combo(ui, &f.key, &mut cur, names));
                value = PageValue::Text(cur);
            }
            Kind::Color => {
                let text = value.text();
                let mut rgb = parse_color(&text).unwrap_or([0, 0, 0]);
                row(ui, &label, |ui| {
                    ui.color_edit_button_srgb(&mut rgb);
                });
                if parse_color(&text) != Some(rgb) {
                    value = PageValue::Text(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
                }
            }
            Kind::Length | Kind::Degrees | Kind::Number(_) | Kind::Text => {
                let mut text = self
                    .bufs
                    .get(&f.key)
                    .cloned()
                    .unwrap_or_else(|| format_value(&f.kind, &value));
                let invalid = parse_text(&f.kind, &text).is_none();
                let unit = match &f.kind {
                    Kind::Number(u) => *u,
                    _ => "",
                };
                let resp = row(ui, &label, |ui| {
                    let mut edit = egui::TextEdit::singleline(&mut text).desired_width(
                        if matches!(f.kind, Kind::Text) {
                            200.0
                        } else {
                            100.0
                        },
                    );
                    if invalid {
                        edit = edit.text_color(ERROR_RED);
                    }
                    let r = ui.add(edit);
                    if !unit.is_empty() {
                        ui.label(unit);
                    }
                    r
                });
                if resp.changed() {
                    if let Some(v) = parse_text(&f.kind, &text) {
                        value = v;
                    }
                    self.bufs.insert(f.key.clone(), text);
                } else if !resp.has_focus()
                    && self
                        .bufs
                        .get(&f.key)
                        .is_some_and(|t| parse_text(&f.kind, t).is_some())
                {
                    self.bufs.remove(&f.key);
                }
            }
        }
        if !value.same(&before) || value != before {
            self.draft.insert(f.key.clone(), value);
        }
    }
}

fn combo(ui: &mut egui::Ui, key: &str, cur: &mut String, names: Vec<String>) {
    egui::ComboBox::from_id_salt(("default_page_combo", key))
        .selected_text(cur.clone())
        .show_ui(ui, |ui| {
            for n in names {
                if ui.selectable_label(*cur == n, &n).clicked() {
                    *cur = n;
                }
            }
        });
}

/// The text a value shows as in a text field.
pub fn format_value(kind: &Kind, v: &PageValue) -> String {
    match kind {
        Kind::Length => fmt_ft_in(v.num()),
        Kind::Degrees => format!("{:.1}\u{B0}", v.num()),
        Kind::Number(_) => {
            let s = format!("{:.4}", v.num());
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        _ => v.text(),
    }
}

/// The value a text field's text means, if it means one.
pub fn parse_text(kind: &Kind, s: &str) -> Option<PageValue> {
    match kind {
        Kind::Length => parse_ft_in(s).filter(|v| v.is_finite()).map(PageValue::Num),
        Kind::Degrees => s
            .trim()
            .trim_end_matches('\u{B0}')
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(PageValue::Num),
        Kind::Number(_) => s
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(PageValue::Num),
        _ => Some(PageValue::Text(s.to_string())),
    }
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

// ----- bindings -----

/// A binding over a number slot of `PlanDefaults`: `bn!(grid.spacing)`.
#[macro_export]
macro_rules! bind_num {
    ($($p:ident).+) => {
        $crate::dialogs::default_pages::page::Bind {
            get: |d| plan_core::defaults::PageValue::Num(d.$($p).+),
            set: |d, v| d.$($p).+ = v.num(),
        }
    };
}

/// A binding over a flag slot.
#[macro_export]
macro_rules! bind_flag {
    ($($p:ident).+) => {
        $crate::dialogs::default_pages::page::Bind {
            get: |d| plan_core::defaults::PageValue::Bool(d.$($p).+),
            set: |d, v| d.$($p).+ = v.flag(),
        }
    };
}

/// A binding over a text slot.
#[macro_export]
macro_rules! bind_text {
    ($($p:ident).+) => {
        $crate::dialogs::default_pages::page::Bind {
            get: |d| plan_core::defaults::PageValue::Text(d.$($p).+.clone()),
            set: |d, v| d.$($p).+ = v.text(),
        }
    };
}

/// A binding over a whole-number slot (`u32`).
#[macro_export]
macro_rules! bind_int {
    ($($p:ident).+) => {
        $crate::dialogs::default_pages::page::Bind {
            get: |d| plan_core::defaults::PageValue::Int(i64::from(d.$($p).+)),
            set: |d, v| d.$($p).+ = v.int().max(0) as u32,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> PageSpec {
        PageSpec::new("t", "Test").section(
            "S",
            vec![
                Field::len("t.len", "Length", 12.0),
                Field::flag("t.flag", "Flag", false),
                Field::pick("t.pick", "Pick", &["A", "B"], "A"),
                Field::len("t.grid", "Grid", 12.0).bound(crate::bind_num!(grid.spacing)),
            ],
        )
    }

    #[test]
    fn stored_fields_keep_only_changes_and_bound_ones_write_the_slot() {
        let mut d = PlanDefaults::default();
        let mut page = GenericPage::new(spec(), &d);
        page.set("t.len", PageValue::Num(30.0));
        page.set("t.pick", PageValue::Text("B".into()));
        page.set("t.grid", PageValue::Num(24.0));
        page.apply(&mut d);
        assert_eq!(d.page_num("t.len", 0.0), 30.0);
        assert_eq!(d.page_text("t.pick", ""), "B");
        assert_eq!(d.grid.spacing, 24.0);
        assert!(
            d.page_value("t.grid").is_none(),
            "a bound field is not stored"
        );
        assert!(
            d.page_value("t.flag").is_none(),
            "an unchanged field is not stored"
        );
        // Back to the built-in value: the entry disappears.
        let mut page = GenericPage::new(spec(), &d);
        page.set("t.len", PageValue::Num(12.0));
        page.apply(&mut d);
        assert!(d.page_value("t.len").is_none());
        // The stored values survive the template JSON.
        let json = serde_json::to_string(&d).unwrap();
        let back: PlanDefaults = serde_json::from_str(&json).unwrap();
        assert_eq!(back.page_text("t.pick", ""), "B");
        assert_eq!(back, d);
    }

    #[test]
    fn reset_puts_the_builtin_values_back_and_text_is_checked() {
        let mut d = PlanDefaults::default();
        d.grid.spacing = 60.0;
        let mut page = GenericPage::new(spec(), &d);
        assert_eq!(page.get("t.grid").unwrap().num(), 60.0);
        page.reset();
        assert_eq!(page.get("t.grid").unwrap().num(), 12.0);
        assert!(parse_text(&Kind::Length, "3'-6\"").is_some());
        assert!(parse_text(&Kind::Length, "abc").is_none());
        page.bufs.insert("t.len".into(), "abc".into());
        assert!(page.error().is_some());
    }
}
