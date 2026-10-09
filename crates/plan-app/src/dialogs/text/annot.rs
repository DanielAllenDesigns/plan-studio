//! The Callout, Marker and Note Specification dialogs (TXT-50..55; reference
//! manual pp. 552 to 563) and what they share: the mode a dialog runs in,
//! the lists it picks from, the Insert menu, the Line Style and Text Style
//! pages and the preview.
//!
//! One dialog serves four jobs ([`Mode`]): the specification of a placed
//! object (`Edit`), the dialog that opens when the tool is clicked and places
//! the object on OK (`New`), and the defaults dialogs of Default Settings
//! (`Defaults`, titled with the Saved Default's name). [`AnnotDialog`] wraps
//! the three kinds for the dialog host and the tools.

use super::callout::CalloutDialog;
use super::marker::MarkerDialog;
use super::note::NoteDialog;
use crate::dialogs::{row, Fields, Outcome};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::callout::{
    AnnotRef, Callout, Gen, LineLook, LinkInfo, Marker, Note, ViewKind, ViewLink,
    ANNOT_LAYER,
};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::layers::LineStyle;
use plan_core::Id;

/// What a dialog is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// The specification of the placed object whose first CAD object is `id`.
    Edit(Id),
    /// Opened by a click of the tool; OK places the object.
    New,
    /// A defaults dialog of Default Settings.
    Defaults,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnnotKind {
    Callout,
    Marker,
    Note,
}

impl AnnotKind {
    /// The name in the dialog titles.
    pub fn name(self) -> &'static str {
        match self {
            AnnotKind::Callout => "Callout",
            AnnotKind::Marker => "Marker",
            AnnotKind::Note => "Note",
        }
    }
}

/// A view or page a callout can be linked to (the Link View dialog's list).
#[derive(Clone, PartialEq, Debug)]
pub struct LinkChoice {
    pub link: ViewLink,
    pub label: String,
    pub info: LinkInfo,
}

/// What the dialogs pick from, read from the plan when one opens.
#[derive(Clone, Default)]
pub struct Env {
    pub layers: Vec<String>,
    pub styles: Vec<String>,
    pub note_types: Vec<String>,
    pub links: Vec<LinkChoice>,
    pub saved_name: String,
    pub floor: usize,
}

impl Env {
    pub fn of(cx: &EditorContext) -> Env {
        let p = &cx.project;
        let mut links = Vec::new();
        let mut add = |link: ViewLink, label: String| {
            let info = p.resolve_view_link(&link);
            links.push(LinkChoice { link, label, info });
        };
        for c in &p.cameras {
            add(
                ViewLink {
                    kind: ViewKind::Camera,
                    id: c.id,
                    name: c.name.clone(),
                },
                format!("Camera Views / {}", c.name),
            );
        }
        for f in p.floors.iter().filter(|f| f.detail.is_some()) {
            add(
                ViewLink {
                    kind: ViewKind::CadDetail,
                    id: 0,
                    name: f.name.clone(),
                },
                format!("CAD Details / {}", f.name),
            );
        }
        for pg in p.layout_pages() {
            add(
                ViewLink {
                    kind: ViewKind::LayoutPage,
                    id: u64::from(pg.number),
                    name: pg.title.clone(),
                },
                format!("Pages / {} {}", pg.label(), pg.title),
            );
        }
        Env {
            layers: cx.layers().layers.iter().map(|l| l.name.clone()).collect(),
            styles: p
                .text_styles
                .names()
                .into_iter()
                .map(str::to_string)
                .collect(),
            note_types: p.note_types().types.iter().map(|t| t.name.clone()).collect(),
            links,
            saved_name: p.annot_defaults.saved_name.clone(),
            floor: cx.floor,
        }
    }
}

/// The dialog title for `kind` in `mode`.
pub fn title(kind: AnnotKind, mode: Mode, saved: &str) -> String {
    match mode {
        Mode::Defaults => format!("{} Defaults - {saved}", kind.name()),
        _ => format!("{} Specification", kind.name()),
    }
}

// ----- the wrapper the host and the tools use -----

pub enum AnnotDialog {
    Callout(Box<CalloutDialog>),
    Marker(Box<MarkerDialog>),
    Note(Box<NoteDialog>),
}

impl AnnotDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        match self {
            AnnotDialog::Callout(d) => d.show(ctx),
            AnnotDialog::Marker(d) => d.show(ctx),
            AnnotDialog::Note(d) => d.show(ctx),
        }
    }

    pub fn kind(&self) -> AnnotKind {
        match self {
            AnnotDialog::Callout(_) => AnnotKind::Callout,
            AnnotDialog::Marker(_) => AnnotKind::Marker,
            AnnotDialog::Note(_) => AnnotKind::Note,
        }
    }

    pub fn mode(&self) -> Mode {
        match self {
            AnnotDialog::Callout(d) => d.mode(),
            AnnotDialog::Marker(d) => d.mode(),
            AnnotDialog::Note(d) => d.mode(),
        }
    }

    pub fn title(&self) -> String {
        match self {
            AnnotDialog::Callout(d) => d.title(),
            AnnotDialog::Marker(d) => d.title(),
            AnnotDialog::Note(d) => d.title(),
        }
    }

    /// The names of the tabs, in order.
    pub fn tab_names(&self) -> Vec<&'static str> {
        match self {
            AnnotDialog::Callout(d) => d.tab_names(),
            AnnotDialog::Marker(d) => d.tab_names(),
            AnnotDialog::Note(d) => d.tab_names(),
        }
    }

    /// Applies the dialog as one undo step: places the object (`New`),
    /// changes it (`Edit`) or stores the defaults (`Defaults`). Returns the
    /// first CAD object of the object placed or changed.
    pub fn apply(&self, cx: &mut EditorContext) -> Option<Id> {
        match self {
            AnnotDialog::Callout(d) => d.apply(cx),
            AnnotDialog::Marker(d) => d.apply(cx),
            AnnotDialog::Note(d) => d.apply(cx),
        }
    }

    /// Test access: the draft's label text, changed by `f`.
    #[cfg(test)]
    pub fn edit_label(&mut self, f: impl FnOnce(&mut String)) {
        match self {
            AnnotDialog::Callout(d) => f(&mut d.draft_mut().label),
            AnnotDialog::Marker(d) => f(&mut d.draft_mut().label.text),
            AnnotDialog::Note(d) => f(&mut d.draft_mut().text),
        }
    }

    /// Selects the object an OK placed or changed: the whole group.
    pub fn select(cx: &mut EditorContext, head: Id) {
        let members = crate::editor::selection::expand_groups(cx, &[ObjectRef::Cad(head)]);
        cx.selection.items = members;
    }
}

thread_local! {
    static POSTED: std::cell::RefCell<Option<AnnotDialog>> = const { std::cell::RefCell::new(None) };
}

/// A tool's click hands its New dialog to the dialog host, which opens it on
/// the next frame (like the Properties tab of every other specification).
pub fn post_new(d: AnnotDialog) {
    POSTED.with(|p| *p.borrow_mut() = Some(d));
}

/// The dialog a tool posted, if any (the host takes it).
pub fn take_posted() -> Option<AnnotDialog> {
    POSTED.with(|p| p.borrow_mut().take())
}

/// The dialog for the callout, marker or note `o` belongs to.
pub fn open_for(cx: &EditorContext, o: ObjectRef) -> Option<AnnotDialog> {
    let (ObjectRef::Cad(id) | ObjectRef::Text(id)) = o else {
        return None;
    };
    let f = cx.floor();
    let env = Env::of(cx);
    Some(match f.annot_of(id)? {
        AnnotRef::Callout(i) => {
            let c = f.annots.callouts[i].clone();
            AnnotDialog::Callout(Box::new(CalloutDialog::new(
                c.clone(),
                Mode::Edit(c.items[0]),
                env,
            )))
        }
        AnnotRef::Marker(i) => {
            let m = f.annots.markers[i].clone();
            AnnotDialog::Marker(Box::new(MarkerDialog::new(
                m.clone(),
                Mode::Edit(m.items[0]),
                env,
            )))
        }
        AnnotRef::Note(i) => {
            let n = f.annots.notes[i].clone();
            AnnotDialog::Note(Box::new(NoteDialog::new(
                n.clone(),
                Mode::Edit(n.items[0]),
                env,
            )))
        }
    })
}

/// The layer annotations are placed on.
fn layer_for(cx: &EditorContext) -> String {
    let l = cx.project.layers.tool_layer("text");
    if l.is_empty() {
        ANNOT_LAYER.to_string()
    } else {
        l
    }
}

/// The dialog a click of the tool opens: the Saved Defaults at `at`.
pub fn new_at(cx: &EditorContext, kind: AnnotKind, at: Point) -> AnnotDialog {
    let d = &cx.project.annot_defaults;
    let env = Env::of(cx);
    let layer = layer_for(cx);
    match kind {
        AnnotKind::Callout => AnnotDialog::Callout(Box::new(CalloutDialog::new(
            Callout {
                center: at,
                layer,
                items: Vec::new(),
                pose: None,
                leaders: Vec::new(),
                ..d.callout.clone()
            },
            Mode::New,
            env,
        ))),
        AnnotKind::Marker => AnnotDialog::Marker(Box::new(MarkerDialog::new(
            Marker {
                center: at,
                layer,
                items: Vec::new(),
                pose: None,
                ..d.marker.clone()
            },
            Mode::New,
            env,
        ))),
        AnnotKind::Note => AnnotDialog::Note(Box::new(NoteDialog::new(
            Note {
                center: at,
                layer,
                items: Vec::new(),
                pose: None,
                ..d.note.clone()
            },
            Mode::New,
            env,
        ))),
    }
}

/// The defaults dialog of `kind` (Default Settings > Text, Callouts and
/// Markers), titled with the Saved Default's name.
pub fn defaults_dialog(cx: &EditorContext, kind: AnnotKind) -> AnnotDialog {
    let d = &cx.project.annot_defaults;
    let env = Env::of(cx);
    match kind {
        AnnotKind::Callout => AnnotDialog::Callout(Box::new(CalloutDialog::new(
            d.callout.clone(),
            Mode::Defaults,
            env,
        ))),
        AnnotKind::Marker => AnnotDialog::Marker(Box::new(MarkerDialog::new(
            d.marker.clone(),
            Mode::Defaults,
            env,
        ))),
        AnnotKind::Note => AnnotDialog::Note(Box::new(NoteDialog::new(
            d.note.clone(),
            Mode::Defaults,
            env,
        ))),
    }
}

// ----- widgets the three forms share -----

const SPECIAL: &[(&str, &str)] = &[
    ("Degree", "\u{b0}"),
    ("Plus/Minus", "\u{b1}"),
    ("Diameter", "\u{d8}"),
    ("Feet", "'"),
    ("Inches", "\""),
    ("One Half", "\u{bd}"),
    ("Number Sign", "#"),
];

const OBJECT_MACROS: &[&str] = &[
    "%linked_view_name%",
    "%linked_view_layout_page_label%",
    "%referenced_view_callout_label%",
    "%layout_page_label%",
    "%automatic_label%",
];

/// What the Insert menu offers beyond the special characters and the global
/// macros.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Macros {
    Callout,
    Marker,
    Note,
}

/// The Insert button of a text field: special characters and text macros
/// (Global, Object Specific). Appends the choice to `text`.
pub fn insert_menu(ui: &mut Ui, id: &str, text: &mut String, set: Macros) -> bool {
    let mut changed = false;
    ui.menu_button("Insert", |ui| {
        ui.menu_button("Special Characters", |ui| {
            for (name, ch) in SPECIAL {
                if ui.button(format!("{name}  {ch}")).clicked() {
                    text.push_str(ch);
                    changed = true;
                    ui.close_menu();
                }
            }
        });
        ui.menu_button("Global", |ui| {
            for (name, _) in plan_core::text_styles::BUILT_IN_MACROS {
                if ui.button(format!("%{name}%")).clicked() {
                    text.push_str(&format!("%{name}%"));
                    changed = true;
                    ui.close_menu();
                }
            }
        });
        ui.menu_button("Object Specific", |ui| {
            let list: &[&str] = match set {
                Macros::Callout => OBJECT_MACROS,
                Macros::Marker => &["%height%"],
                Macros::Note => &["%simple_schedule_number%", "%note_text%"],
            };
            for m in list {
                if ui.button(*m).clicked() {
                    text.push_str(m);
                    changed = true;
                    ui.close_menu();
                }
            }
        });
    })
    .response
    .on_hover_text(format!("Insert a special character or text macro ({id})"));
    changed
}

/// A "By Layer" checkbox and a colour button: `None` follows the layer.
pub fn color_row(ui: &mut Ui, label: &str, color: &mut Option<[u8; 3]>) {
    row(ui, label, |ui| {
        let mut by_layer = color.is_none();
        if ui.checkbox(&mut by_layer, "By Layer").changed() {
            *color = if by_layer { None } else { Some([0, 0, 0]) };
        }
        if let Some(c) = color {
            ui.color_edit_button_srgb(c);
        }
    });
}

/// The transparency slider (percent).
pub fn transparency_row(ui: &mut Ui, value: &mut u8) {
    row(ui, "Transparency", |ui| {
        let mut v = i32::from(*value);
        if ui
            .add(egui::Slider::new(&mut v, 0..=100).suffix(" %"))
            .changed()
        {
            *value = v.clamp(0, 100) as u8;
        }
    });
}

const LINE_STYLES: [(&str, LineStyle); 4] = [
    ("Solid", LineStyle::Solid),
    ("Dashed", LineStyle::Dashed),
    ("Dotted", LineStyle::Dotted),
    ("Dash Dot", LineStyle::DashDot),
];

/// Style and weight with "By Layer" boxes.
pub fn line_look_rows(ui: &mut Ui, look: &mut LineLook) {
    row(ui, "Style", |ui| {
        let mut by_layer = look.style.is_none();
        if ui.checkbox(&mut by_layer, "By Layer").changed() {
            look.style = if by_layer { None } else { Some(LineStyle::Solid) };
        }
        if let Some(s) = &mut look.style {
            egui::ComboBox::from_id_salt(("annot_line_style", ui.id()))
                .selected_text(LINE_STYLES.iter().find(|(_, v)| v == s).map_or("Solid", |(n, _)| n))
                .show_ui(ui, |ui| {
                    for (n, v) in LINE_STYLES {
                        ui.selectable_value(s, v, n);
                    }
                });
        }
    });
    row(ui, "Weight", |ui| {
        let mut by_layer = look.weight.is_none();
        if ui.checkbox(&mut by_layer, "By Layer").changed() {
            look.weight = if by_layer { None } else { Some(25) };
        }
        if let Some(w) = &mut look.weight {
            let mut v = *w;
            if ui
                .add(egui::DragValue::new(&mut v).range(1..=200).suffix(" /100 mm"))
                .changed()
            {
                *w = v;
            }
        }
    });
}

/// The Line Style panel: the outline's style, weight and colour, and the
/// layer.
pub fn line_style_page(ui: &mut Ui, look: &mut LineLook, layer: &mut String, layers: &[String]) {
    crate::dialogs::section(ui, "Line Style");
    line_look_rows(ui, look);
    color_row(ui, "Color", &mut look.color);
    crate::dialogs::section(ui, "Layer");
    row(ui, "Layer", |ui| {
        egui::ComboBox::from_id_salt(("annot_layer", ui.id()))
            .selected_text(layer.clone())
            .show_ui(ui, |ui| {
                for n in layers {
                    ui.selectable_value(layer, n.clone(), n.as_str());
                }
            });
    });
}

/// The Text Style panel: a saved text style (or the layer's) and the height.
pub fn text_style_page(
    ui: &mut Ui,
    style: &mut Option<String>,
    height: &mut f64,
    styles: &[String],
    fields: &mut Fields,
) {
    crate::dialogs::section(ui, "Text Style");
    row(ui, "Text Style", |ui| {
        let shown = style
            .clone()
            .unwrap_or_else(|| "Use Layer Text Style".to_string());
        egui::ComboBox::from_id_salt(("annot_text_style", ui.id()))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(style, None, "Use Layer Text Style");
                for n in styles {
                    ui.selectable_value(style, Some(n.clone()), n.as_str());
                }
            });
    });
    fields.length_row(ui, "Character Height", "annot_text_height", height);
    ui.weak("Plan inches; a Printed Size style prints at its own size on paper.");
}

// ----- the preview -----

/// Draws the generated objects of an annotation, scaled to fit `rect`.
pub fn draw_preview(p: &Painter, rect: Rect, g: &Gen) {
    let ink = Color32::from_rgb(0x2B, 0x2B, 0x2B);
    p.rect_stroke(
        rect.shrink(2.0),
        2.0,
        Stroke::new(0.8_f32, crate::dialogs::PV_FAINT),
        egui::StrokeKind::Inside,
    );
    if g.items.is_empty() {
        return;
    }
    let mut lo = Point::new(f64::MAX, f64::MAX);
    let mut hi = Point::new(f64::MIN, f64::MIN);
    for (it, _) in &g.items {
        let (a, b) = it.bounds();
        lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
        hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
    }
    let area = rect.shrink(12.0);
    let (w, h) = ((hi.x - lo.x).max(1e-3), (hi.y - lo.y).max(1e-3));
    let k = (f64::from(area.width()) / w).min(f64::from(area.height()) / h);
    let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    let sc = |q: Point| {
        Pos2::new(
            area.center().x + ((q.x - mid.x) * k) as f32,
            area.center().y - ((q.y - mid.y) * k) as f32,
        )
    };
    for (it, at) in &g.items {
        let stroke = Stroke::new(1.2_f32, ink);
        let fill = at
            .fill
            .as_ref()
            .map(|f| Color32::from_rgba_unmultiplied(f.color[0], f.color[1], f.color[2], f.opacity));
        match it {
            CadItem::Line { a, b } => {
                p.line_segment([sc(*a), sc(*b)], stroke);
                if at.arrow_end != plan_core::cad::ArrowStyle::None {
                    p.circle_filled(sc(*b), 2.5, ink);
                }
            }
            CadItem::Circle { center, radius } => {
                let c = sc(*center);
                let r = (*radius * k) as f32;
                if let Some(f) = fill {
                    p.circle_filled(c, r, f);
                }
                p.circle_stroke(c, r, stroke);
            }
            CadItem::Arc { center, radius, .. } => {
                p.circle_stroke(sc(*center), (*radius * k) as f32, stroke);
            }
            CadItem::Polyline { points, closed } => {
                let pts: Vec<Pos2> = points.iter().map(|q| sc(*q)).collect();
                if *closed {
                    if let Some(f) = fill {
                        p.add(egui::Shape::convex_polygon(pts.clone(), f, Stroke::NONE));
                    }
                    p.add(egui::Shape::closed_line(pts, stroke));
                } else {
                    p.add(egui::Shape::line(pts.clone(), stroke));
                    if at.arrow_start != plan_core::cad::ArrowStyle::None {
                        if let Some(first) = pts.first() {
                            p.circle_filled(*first, 2.5, ink);
                        }
                    }
                }
            }
            CadItem::Text {
                pos, text, height, ..
            } => {
                let size = ((*height * k) as f32).clamp(7.0, 18.0);
                p.text(
                    sc(*pos),
                    Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional(size),
                    ink,
                );
            }
        }
    }
}

// ----- applying -----

/// Writes the edited record over the placed one, keeping what the dialog
/// does not edit, and regenerates its objects. Shared by the three kinds.
pub fn lock_check(cx: &mut EditorContext, id: Id) -> bool {
    cx.check_unlocked(ObjectRef::Cad(id))
}

/// The Link panel's information lines.
pub fn link_lines(info: Option<&LinkInfo>) -> [(&'static str, String); 4] {
    match info.filter(|i| i.valid) {
        Some(i) => [
            ("File Name", i.file_name.clone()),
            ("View Name", i.view_name.clone()),
            ("View Type", i.view_type.clone()),
            (
                "Page",
                if i.page_label.is_empty() {
                    "(not sent to layout)".to_string()
                } else {
                    i.page_label.clone()
                },
            ),
        ],
        None => [
            ("File Name", String::new()),
            ("View Name", String::new()),
            ("View Type", String::new()),
            ("Page", String::new()),
        ],
    }
}
