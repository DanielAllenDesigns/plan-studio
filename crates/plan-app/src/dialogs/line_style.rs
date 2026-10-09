//! Line styles as a library (CAD-82..CAD-86; manual pp. 230-233).
//!
//! * **CAD > Lines > Line Style Management**: the styles saved in the plan
//!   with their sample, name and Used icon (red plus: assigned to objects,
//!   wrench: named in a defaults dialog, S: system); Edit, New, Copy, Move
//!   Up/Down, Purge, Delete, Merge, Add to Library and Auto Purge. OK is one
//!   undo step.
//! * **Line Style Specification**: the name, a preview with Highlight
//!   Selection and Mark Repeating Segments, Dash/Dot/Text buttons that insert
//!   before or after the selected component, the component order list and the
//!   length, spacing, text and height of the selected component.
//! * **File > Import > Import Line Styles** (.lin): the styles go to the
//!   User Catalog.
//! * [`picker`]: the Line Style panel's drop-down with the Library button,
//!   for every dialog that has a line style.
//! * [`paint_pieces`]: draws what the shared stroker
//!   (`plan_core::line_styles::stroke_path`) produced; plan, layout and the
//!   previews all go through it.
//! * [`apply_to_selection`] and [`poll_drawing_style`]: put a library style on
//!   the selected CAD objects, or on every line drawn while one is picked
//!   in the Library Browser.

use super::{row, section, Outcome};
use crate::editor::EditorContext;
use eframe::egui::{
    self, Align, Align2, Color32, Key, Layout, Modifiers, Pos2, RichText, Shape, Stroke, Vec2,
};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::line_styles::{
    catalog, enum_name, stroke_path, usage, ComponentKind, LineComponent, LineStyleDef,
    LineStyleLibrary, LineTarget, Piece, UseMark,
};
use plan_core::{Id, LineStyle};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

/// CAD > Lines > Line Style Management.
pub const MANAGEMENT: &str = "linestyle.manage";
/// File > Import > Import Line Styles.
pub const IMPORT: &str = "linestyle.import";
/// The Line Style panel for the selected CAD objects.
pub const ASSIGN: &str = "linestyle.assign";

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    /// The next file the import dialog returns (tests).
    static NEXT_PICK: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
    /// The line style picked in the Library Browser: new lines use it.
    static DRAWING: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The highest CAD id seen by [`poll_drawing_style`].
    static SEEN: RefCell<Id> = const { RefCell::new(0) };
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

/// Draws the pieces of a stroked line. `to_screen` maps path points to
/// screen points; `px_per_unit` is screen pixels per path unit (it sizes dots
/// and text); `stroke` is the line's colour and width.
pub fn paint_pieces(
    painter: &egui::Painter,
    pieces: &[Piece],
    stroke: Stroke,
    px_per_unit: f32,
    to_screen: &dyn Fn(Point) -> Pos2,
) {
    for piece in pieces {
        match piece {
            Piece::Run(pts) => {
                let v: Vec<Pos2> = pts.iter().map(|p| to_screen(*p)).collect();
                if v.len() >= 2 {
                    painter.add(Shape::line(v, stroke));
                }
            }
            Piece::Dot(p) => {
                painter.add(Shape::circle_filled(
                    to_screen(*p),
                    (stroke.width * 0.9).max(1.0),
                    stroke.color,
                ));
            }
            Piece::Text {
                at,
                angle,
                text,
                height,
                ..
            } => {
                let size = (*height as f32 * px_per_unit).clamp(3.0, 200.0);
                let galley = painter.layout_no_wrap(
                    text.clone(),
                    egui::FontId::proportional(size),
                    stroke.color,
                );
                let c = to_screen(*at);
                // The plan is Y up, the screen Y down: the word turns the
                // other way on the screen.
                let a = -(*angle as f32);
                let half = galley.size() / 2.0;
                let (s, co) = a.sin_cos();
                let top_left = c - Vec2::new(half.x * co - half.y * s, half.x * s + half.y * co);
                painter.add(
                    egui::epaint::TextShape::new(top_left, galley, stroke.color).with_angle(a),
                );
            }
        }
    }
}

/// Strokes and paints a path in the plan view; `scale` is plan inches per
/// paper inch.
pub fn paint_path(
    painter: &egui::Painter,
    cam: &crate::editor::Camera,
    def: &LineStyleDef,
    pts: &[Point],
    closed: bool,
    scale: f64,
    stroke: Stroke,
) {
    let pieces = stroke_path(def, pts, closed, scale);
    paint_pieces(painter, &pieces, stroke, cam.px_per_in as f32, &|p| {
        cam.world_to_screen(p)
    });
}

/// The path (in plan inches) of a CAD item that a line style follows, and
/// whether it is closed.
pub fn item_path(item: &CadItem) -> Option<(Vec<Point>, bool)> {
    plan_core::line_styles::cad_item_path(item)
}

const SAMPLE_PPI: f64 = 96.0;

/// A sample of `def` in a `size` box (the Style column and the pickers).
pub fn sample(ui: &mut egui::Ui, def: &LineStyleDef, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let color = ui.visuals().text_color();
    let y = rect.center().y as f64;
    let pts = [
        Point::new(f64::from(rect.left()) + 3.0, y),
        Point::new(f64::from(rect.right()) - 3.0, y),
    ];
    let painter = ui.painter_at(rect);
    let pieces = stroke_path(def, &pts, false, SAMPLE_PPI);
    paint_pieces(
        &painter,
        &pieces,
        Stroke::new(1.5_f32, color),
        SAMPLE_PPI as f32,
        &|p| Pos2::new(p.x as f32, p.y as f32),
    );
}

// ---------------------------------------------------------------------------
// The Line Style panel picker
// ---------------------------------------------------------------------------

/// The Line Style panel's style drop-down and Library button. `current` is
/// the style name in use ("" is the layer's); returns the style chosen this
/// frame (a catalog choice is returned whole so the caller can save it in
/// the plan with `StyleBook::set_line`).
pub fn picker(
    ui: &mut egui::Ui,
    salt: &str,
    book: &plan_core::fill_styles::StyleBook,
    current: &str,
) -> Option<LineStyleDef> {
    let mut chosen = None;
    let shown = if current.is_empty() {
        "Use Layer"
    } else {
        current
    };
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(("line_style_pick", salt))
            .selected_text(shown)
            .width(150.0)
            .show_ui(ui, |ui| {
                for s in &book.line_styles.styles {
                    ui.horizontal(|ui| {
                        sample(ui, s, Vec2::new(70.0, 14.0));
                        if ui.selectable_label(s.name == current, &s.name).clicked() {
                            chosen = Some(s.clone());
                        }
                    });
                }
            });
        let popup_id = ui.make_persistent_id(("line_style_lib", salt));
        let btn = ui
            .button("Library\u{2026}")
            .on_hover_text("Choose a line style from the library");
        if btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(popup_id));
        }
        egui::popup_below_widget(
            ui,
            popup_id,
            &btn,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(240.0);
                for s in library_styles(book) {
                    ui.horizontal(|ui| {
                        sample(ui, &s, Vec2::new(80.0, 14.0));
                        if ui.selectable_label(false, &s.name).clicked() {
                            chosen = Some(s.clone());
                            ui.memory_mut(|m| m.close_popup());
                        }
                    });
                }
            },
        );
    });
    chosen
}

/// The Library Browser's line styles: the built-in catalog then the User
/// Catalog.
pub fn library_styles(book: &plan_core::fill_styles::StyleBook) -> Vec<LineStyleDef> {
    let mut v = catalog();
    for u in &book.user_lines {
        match v.iter_mut().find(|s| s.name == u.name) {
            Some(s) => *s = u.clone(),
            None => v.push(u.clone()),
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Line Style Specification
// ---------------------------------------------------------------------------

/// The Line Style Specification dialog's draft.
#[derive(Debug, Clone)]
pub struct SpecDraft {
    /// The row of the file's list being edited; `None` for a new style.
    pub index: Option<usize>,
    pub def: LineStyleDef,
    /// The selected component.
    pub selected: usize,
    pub insert_after: bool,
    pub highlight: bool,
    pub mark_repeats: bool,
}

impl SpecDraft {
    pub fn new(index: Option<usize>, def: LineStyleDef) -> Self {
        Self {
            index,
            def,
            selected: 0,
            insert_after: true,
            highlight: true,
            mark_repeats: false,
        }
    }

    /// Dash, Dot or Text button: the new component goes before or after the
    /// selected one.
    pub fn add(&mut self, kind: ComponentKind) {
        let c = match kind {
            ComponentKind::Dash => LineComponent::dash(0.125, 0.0625),
            ComponentKind::Dot => LineComponent::dot(0.0625),
            ComponentKind::Text => LineComponent::text("TEXT", 0.09, 0.0625),
        };
        let n = self.def.components.len();
        let at = if n == 0 {
            0
        } else if self.insert_after {
            (self.selected + 1).min(n)
        } else {
            self.selected.min(n)
        };
        self.def.components.insert(at, c);
        self.selected = at;
    }

    /// The Up arrow: one place earlier in the list and to the left.
    pub fn move_up(&mut self) {
        if self.selected > 0 && self.selected < self.def.components.len() {
            self.def.components.swap(self.selected, self.selected - 1);
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.def.components.len() {
            self.def.components.swap(self.selected, self.selected + 1);
            self.selected += 1;
        }
    }

    pub fn remove(&mut self) {
        if self.selected < self.def.components.len() {
            self.def.components.remove(self.selected);
            self.selected = self
                .selected
                .min(self.def.components.len().saturating_sub(1));
        }
    }

    /// Why OK is refused.
    pub fn error(&self, lib: &LineStyleLibrary) -> Option<String> {
        if self.def.name.trim().is_empty() {
            return Some("A line style needs a name".into());
        }
        if lib
            .styles
            .iter()
            .enumerate()
            .any(|(i, s)| Some(i) != self.index && s.name == self.def.name)
        {
            return Some(format!(
                "There is already a line style named {}",
                self.def.name
            ));
        }
        if !self.def.components.is_empty() && !self.def.strokable() {
            return Some("The components need some length".into());
        }
        None
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(430.0, 54.0);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0xEC, 0xEA, 0xE3));
        let ink = Color32::from_rgb(0x2B, 0x2B, 0x2B);
        let y = f64::from(rect.center().y);
        let x0 = f64::from(rect.left()) + 8.0;
        let pts = [
            Point::new(x0, y),
            Point::new(f64::from(rect.right()) - 8.0, y),
        ];
        let pieces = stroke_path(&self.def, &pts, false, SAMPLE_PPI);
        paint_pieces(
            &painter,
            &pieces,
            Stroke::new(2.0_f32, ink),
            SAMPLE_PPI as f32,
            &|p| Pos2::new(p.x as f32, p.y as f32),
        );
        let offs = self.def.offsets();
        if self.highlight {
            if let (Some(o), Some(c)) = (
                offs.get(self.selected),
                self.def.components.get(self.selected),
            ) {
                let w = (c.advance() * SAMPLE_PPI).max(4.0);
                let r = egui::Rect::from_min_size(
                    Pos2::new((x0 + o * SAMPLE_PPI) as f32 - 2.0, rect.center().y - 14.0),
                    Vec2::new(w as f32 + 4.0, 28.0),
                );
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.5_f32, Color32::from_rgb(0xD0, 0x2B, 0x2B)),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if self.mark_repeats && self.def.period() > 1e-6 {
            let mut x = x0 + self.def.period() * SAMPLE_PPI;
            while x < f64::from(rect.right()) - 8.0 {
                painter.extend(Shape::dashed_line(
                    &[
                        Pos2::new(x as f32, rect.top() + 4.0),
                        Pos2::new(x as f32, rect.bottom() - 4.0),
                    ],
                    Stroke::new(1.0_f32, Color32::from_rgb(0x2B, 0x6C, 0xD0)),
                    3.0,
                    3.0,
                ));
                x += self.def.period() * SAMPLE_PPI;
            }
        }
        // A click on a component in the preview selects it.
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let at = (f64::from(pos.x) - x0) / SAMPLE_PPI;
                let period = self.def.period();
                if period > 1e-6 {
                    let at = at.rem_euclid(period);
                    let offs = self.def.offsets();
                    if let Some(i) = (0..offs.len()).rev().find(|i| offs[*i] <= at + 1e-9) {
                        self.selected = i;
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, lib: &LineStyleLibrary) {
        let error = self.error(lib);
        row(ui, "Line Style Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.def.name).desired_width(220.0));
        });
        section(ui, "Line Style Preview");
        self.preview(ui);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.highlight, "Highlight Selection");
            ui.checkbox(&mut self.mark_repeats, "Mark Repeating Segments");
        });
        section(ui, "Line Style Components");
        ui.horizontal(|ui| {
            if ui.button("Dash").clicked() {
                self.add(ComponentKind::Dash);
            }
            if ui.button("Dot").clicked() {
                self.add(ComponentKind::Dot);
            }
            if ui.button("Text").clicked() {
                self.add(ComponentKind::Text);
            }
            ui.radio_value(&mut self.insert_after, false, "Insert Before");
            ui.radio_value(&mut self.insert_after, true, "Insert After");
        });
        section(ui, "Component Order");
        ui.horizontal_top(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("line_style_components")
                .max_height(110.0)
                .min_scrolled_width(200.0)
                .show(ui, |ui| {
                    let labels = self.def.component_labels();
                    for (i, l) in labels.iter().enumerate() {
                        if ui.selectable_label(self.selected == i, l).clicked() {
                            self.selected = i;
                        }
                    }
                });
            ui.vertical(|ui| {
                if ui.button("\u{25B2}").on_hover_text("Move up").clicked() {
                    self.move_up();
                }
                if ui.button("\u{25BC}").on_hover_text("Move down").clicked() {
                    self.move_down();
                }
                if ui.button("Remove").clicked() {
                    self.remove();
                }
            });
        });
        if let Some(c) = self.def.components.get_mut(self.selected) {
            ui.add_space(4.0);
            match c.kind {
                ComponentKind::Dash => {
                    row(ui, "Length", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut c.length)
                                .speed(0.005)
                                .range(0.0..=20.0)
                                .suffix("\""),
                        );
                    });
                }
                ComponentKind::Dot => {}
                ComponentKind::Text => {
                    row(ui, "Text", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut c.text).desired_width(140.0));
                    });
                    row(ui, "Height", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut c.height)
                                .speed(0.005)
                                .range(0.01..=2.0)
                                .suffix("\""),
                        );
                    });
                    row(ui, "Font", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut c.font)
                                .hint_text("Layer text style")
                                .desired_width(140.0),
                        );
                    });
                }
            }
            row(ui, "Spacing After", |ui| {
                ui.add(
                    egui::DragValue::new(&mut c.spacing)
                        .speed(0.005)
                        .range(0.0..=20.0)
                        .suffix("\""),
                );
            });
        }
        ui.weak(
            "Sizes are paper inches and scale with the drawing; text uses the layer's text style.",
        );
        if let Some(e) = error {
            ui.colored_label(Color32::from_rgb(0xFF, 0x7B, 0x7B), e);
        }
    }
}

// ---------------------------------------------------------------------------
// Line Style Management
// ---------------------------------------------------------------------------

/// The Line Style Management dialog on a draft of the plan's list.
pub struct ManageDialog {
    pub lib: LineStyleLibrary,
    assign: Vec<(LineTarget, String)>,
    user_lines: Vec<LineStyleDef>,
    /// The selected rows.
    pub selected: Vec<usize>,
    enum_uses: Vec<LineStyle>,
    cad_ids: HashSet<Id>,
    spec: Option<SpecDraft>,
    pub status: String,
}

#[allow(dead_code)]
impl ManageDialog {
    pub fn new(cx: &EditorContext) -> Self {
        let p = &cx.project;
        let mut enum_uses: Vec<LineStyle> = p.layers.layers.iter().map(|l| l.line_style).collect();
        let mut cad_ids = HashSet::new();
        for f in &p.floors {
            cad_ids.extend(f.cad.iter().map(|o| o.id));
            enum_uses.extend(f.cad_attrs.iter().filter_map(|a| a.dash));
        }
        Self {
            lib: p.styles.line_styles.clone(),
            assign: p.styles.line_assign.clone(),
            user_lines: p.styles.user_lines.clone(),
            selected: Vec::new(),
            enum_uses,
            cad_ids,
            spec: None,
            status: String::new(),
        }
    }

    /// The Used column, by style name.
    pub fn usage(&self) -> BTreeMap<String, UseMark> {
        usage(
            &self.assign,
            &self.enum_uses,
            &|id| self.cad_ids.contains(&id),
            &self.lib,
        )
    }

    fn in_use(&self, name: &str) -> bool {
        self.usage().get(name).is_some_and(UseMark::in_use)
    }

    pub fn select(&mut self, rows: &[usize]) {
        self.selected = rows
            .iter()
            .copied()
            .filter(|r| *r < self.lib.styles.len())
            .collect();
    }

    /// New: opens the specification on a fresh style.
    pub fn new_style(&mut self) {
        let mut def = LineStyleDef::default();
        def.name = self.lib.unique_name(&def.name);
        self.spec = Some(SpecDraft::new(None, def));
    }

    /// Edit: opens the specification on the first selected style.
    pub fn edit(&mut self) {
        if let Some(&i) = self.selected.first() {
            if let Some(def) = self.lib.styles.get(i) {
                self.spec = Some(SpecDraft::new(Some(i), def.clone()));
            }
        }
    }

    pub fn spec_mut(&mut self) -> Option<&mut SpecDraft> {
        self.spec.as_mut()
    }

    /// OK in the specification.
    pub fn accept_spec(&mut self) -> Result<(), String> {
        let Some(s) = self.spec.take() else {
            return Ok(());
        };
        if let Some(e) = s.error(&self.lib) {
            self.spec = Some(s);
            return Err(e);
        }
        match s.index {
            None => {
                let i = self.lib.add(s.def);
                self.selected = vec![i];
            }
            Some(i) => {
                let old = self.lib.styles[i].name.clone();
                let new = s.def.name.clone();
                if let Err(e) = self.lib.edit(i, s.def.clone()) {
                    self.spec = Some(s);
                    return Err(e);
                }
                for (_, n) in &mut self.assign {
                    if *n == old {
                        n.clone_from(&new);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn cancel_spec(&mut self) {
        self.spec = None;
    }

    pub fn copy(&mut self) {
        let mut out = Vec::new();
        for &i in self.selected.iter().rev() {
            if let Some(n) = self.lib.copy(i) {
                out.push(n);
            }
        }
        out.sort_unstable();
        if !out.is_empty() {
            self.selected = out;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.lib.move_up(&self.selected);
    }

    pub fn move_down(&mut self) {
        self.selected = self.lib.move_down(&self.selected);
    }

    /// Purge: deletes every unused style.
    pub fn purge(&mut self) -> usize {
        let used = self.usage();
        let gone = self
            .lib
            .purge(&|n| used.get(n).is_some_and(UseMark::in_use));
        self.selected.clear();
        self.status = format!("Purged {} unused line style(s)", gone.len());
        gone.len()
    }

    /// Delete: removes the selected styles that nothing uses.
    pub fn delete(&mut self) -> usize {
        let used = self.usage();
        let rows = self.selected.clone();
        let gone = self
            .lib
            .delete(&rows, &|n| used.get(n).is_some_and(UseMark::in_use));
        self.selected.clear();
        gone.len()
    }

    pub fn can_delete(&self) -> bool {
        let used = self.usage();
        !self.selected.is_empty()
            && self.selected.iter().all(|i| {
                self.lib
                    .styles
                    .get(*i)
                    .is_some_and(|s| !s.system && !used.get(&s.name).is_some_and(UseMark::in_use))
            })
    }

    /// Would Merge be accepted for the selection?
    pub fn can_merge_into_system(&self) -> bool {
        let mut l = self.lib.clone();
        l.merge(&self.selected).is_ok()
    }

    /// Merge: keeps the topmost selected style; the others' instances now use
    /// it.
    pub fn merge(&mut self) -> Result<(), String> {
        let (kept, dropped) = self.lib.merge(&self.selected)?;
        for (_, n) in &mut self.assign {
            if dropped.contains(n) {
                n.clone_from(&kept);
            }
        }
        self.selected = self.lib.index_of(&kept).into_iter().collect();
        Ok(())
    }

    /// Add to Library: the selected styles go to the User Catalog.
    pub fn add_to_library(&mut self) -> usize {
        let mut n = 0;
        for &i in &self.selected {
            if let Some(s) = self.lib.styles.get(i) {
                let s = LineStyleDef {
                    system: false,
                    ..s.clone()
                };
                match self.user_lines.iter_mut().find(|u| u.name == s.name) {
                    Some(u) => *u = s,
                    None => self.user_lines.push(s),
                }
                n += 1;
            }
        }
        self.status = format!("Added {n} line style(s) to the User Catalog");
        n
    }

    fn click_row(&mut self, i: usize, mods: Modifiers) {
        if mods.shift {
            let a = self.selected.first().copied().unwrap_or(i);
            let (lo, hi) = (a.min(i), a.max(i));
            self.selected = (lo..=hi).collect();
        } else if mods.command {
            if let Some(p) = self.selected.iter().position(|r| *r == i) {
                self.selected.remove(p);
            } else {
                self.selected.push(i);
                self.selected.sort_unstable();
            }
        } else {
            self.selected = vec![i];
        }
    }

    /// Draws the dialog; Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let usage = self.usage();
        egui::Window::new("Line Style Management")
            .id(egui::Id::new("line_style_management"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(560.0);
                section(ui, "Line Styles");
                ui.horizontal_top(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("line_style_table")
                        .max_height(260.0)
                        .min_scrolled_width(380.0)
                        .show(ui, |ui| {
                            egui::Grid::new("line_style_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("Style");
                                    ui.strong("Name");
                                    ui.strong("Used");
                                    ui.end_row();
                                    for i in 0..self.lib.styles.len() {
                                        let s = self.lib.styles[i].clone();
                                        sample(ui, &s, Vec2::new(110.0, 16.0));
                                        let sel = self.selected.contains(&i);
                                        let r = ui.selectable_label(sel, &s.name);
                                        if r.clicked() {
                                            let m = ui.input(|inp| inp.modifiers);
                                            self.click_row(i, m);
                                        }
                                        if r.double_clicked() {
                                            self.selected = vec![i];
                                            self.edit();
                                        }
                                        let mark = usage.get(&s.name).cloned().unwrap_or_default();
                                        used_icons(ui, &mark);
                                        ui.end_row();
                                    }
                                });
                        });
                    ui.vertical(|ui| {
                        let one = self.selected.len() == 1;
                        if ui.add_enabled(one, egui::Button::new("Edit")).clicked() {
                            self.edit();
                        }
                        if ui.button("New").clicked() {
                            self.new_style();
                        }
                        if ui
                            .add_enabled(!self.selected.is_empty(), egui::Button::new("Copy"))
                            .clicked()
                        {
                            self.copy();
                        }
                        if ui
                            .add_enabled(!self.selected.is_empty(), egui::Button::new("Move Up"))
                            .clicked()
                        {
                            self.move_up();
                        }
                        if ui
                            .add_enabled(!self.selected.is_empty(), egui::Button::new("Move Down"))
                            .clicked()
                        {
                            self.move_down();
                        }
                        if ui
                            .button("Purge")
                            .on_hover_text("Delete every unused line style")
                            .clicked()
                        {
                            self.purge();
                        }
                        if ui
                            .add_enabled(self.can_delete(), egui::Button::new("Delete"))
                            .clicked()
                        {
                            self.delete();
                        }
                        if ui
                            .add_enabled(self.selected.len() >= 2, egui::Button::new("Merge"))
                            .clicked()
                        {
                            if let Err(e) = self.merge() {
                                self.status = e;
                            }
                        }
                        if ui
                            .add_enabled(
                                !self.selected.is_empty(),
                                egui::Button::new("Add to Library"),
                            )
                            .clicked()
                        {
                            self.add_to_library();
                        }
                    });
                });
                ui.checkbox(&mut self.lib.auto_purge, "Auto Purge Line Styles");
                if !self.status.is_empty() {
                    ui.weak(&self.status);
                }
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("   OK   ").strong()).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                });
            });
        if self.spec.is_some() {
            self.show_spec(ctx);
        } else if !open {
            outcome = Outcome::Cancel;
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn show_spec(&mut self, ctx: &egui::Context) {
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        let mut done: Option<bool> = None;
        let error = spec.error(&self.lib);
        egui::Window::new("Line Style Specification")
            .id(egui::Id::new("line_style_specification"))
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(450.0);
                spec.ui(ui, &self.lib);
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            error.is_none(),
                            egui::Button::new(RichText::new("   OK   ").strong()),
                        )
                        .clicked()
                    {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            done = Some(false);
        }
        self.spec = Some(spec);
        match done {
            Some(true) => {
                if let Err(e) = self.accept_spec() {
                    self.status = e;
                }
            }
            Some(false) => self.cancel_spec(),
            None => {}
        }
    }
}

/// The Used column's icons: a red plus, a wrench, a grey S.
fn used_icons(ui: &mut egui::Ui, m: &UseMark) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let tip = m.tip();
        if m.objects > 0 {
            ui.label(
                RichText::new("+")
                    .strong()
                    .color(Color32::from_rgb(0xD0, 0x2B, 0x2B)),
            )
            .on_hover_text(&tip);
        }
        if !m.defaults.is_empty() {
            ui.label("\u{1F527}").on_hover_text(&tip);
        }
        if m.system {
            ui.label(RichText::new("S").color(Color32::GRAY))
                .on_hover_text(&tip);
        }
    });
}

struct Host {
    manage: ManageDialog,
}

/// Applies an accepted draft as one undo step.
pub fn apply(cx: &mut EditorContext, d: &ManageDialog) {
    cx.begin_change("Line Style Management");
    let s = &mut cx.project.styles;
    s.line_styles = d.lib.clone();
    s.line_assign = d.assign.clone();
    s.user_lines = d.user_lines.clone();
    if s.line_styles.auto_purge {
        s.purge_line_styles();
    }
    cx.mark_dirty();
    cx.status = "Updated the line styles".into();
}

/// Opens Line Style Management on the plan's list.
pub fn open(cx: &EditorContext) {
    let h = Host {
        manage: ManageDialog::new(cx),
    };
    HOST.with(|c| *c.borrow_mut() = Some(h));
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open dialog.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut ManageDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(|h| f(&mut h.manage)))
}

/// Closes the open dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(h) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    apply(cx, &h.manage);
    true
}

/// Shows the open dialog once a frame and applies its OK. Also follows the
/// line style picked in the Library Browser (see [`poll_drawing_style`]).
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    poll_drawing_style(cx);
    let Some(mut h) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match h.manage.show(ctx) {
        Outcome::Open => HOST.with(|c| *c.borrow_mut() = Some(h)),
        Outcome::Cancel => {}
        Outcome::Ok => apply(cx, &h.manage),
    }
}

// ---------------------------------------------------------------------------
// Using a style
// ---------------------------------------------------------------------------

/// Library Browser: a line style picked there is used by the lines drawn
/// next, until another line style or drawing tool is selected. `None` ends it.
pub fn set_drawing_style(cx: &EditorContext, name: Option<&str>) {
    DRAWING.with(|d| *d.borrow_mut() = name.map(str::to_string));
    SEEN.with(|s| *s.borrow_mut() = max_cad_id(cx));
}

pub fn drawing_style() -> Option<String> {
    DRAWING.with(|d| d.borrow().clone())
}

fn max_cad_id(cx: &EditorContext) -> Id {
    cx.project
        .floors
        .iter()
        .flat_map(|f| f.cad.iter().map(|o| o.id))
        .max()
        .unwrap_or(0)
}

/// Gives every CAD line, polyline, arc or circle drawn since the last call
/// the style picked in the library (the assignment rides on the drawing's
/// own undo step). Returns how many objects took it.
pub fn poll_drawing_style(cx: &mut EditorContext) -> usize {
    let Some(name) = drawing_style() else {
        return 0;
    };
    let seen = SEEN.with(|s| *s.borrow());
    let new: Vec<Id> = cx
        .project
        .floors
        .iter()
        .flat_map(|f| f.cad.iter())
        .filter(|o| o.id > seen && item_path(&o.item).is_some())
        .map(|o| o.id)
        .collect();
    SEEN.with(|s| *s.borrow_mut() = max_cad_id(cx));
    let Some(def) = cx.project.styles.line_style(&name) else {
        return 0;
    };
    for id in &new {
        cx.project.styles.set_line(LineTarget::Cad(*id), Some(&def));
    }
    new.len()
}

/// Puts `def` on the selected CAD objects as one undo step (the Line Style
/// panel of their specification). `None` clears the library style. Returns
/// how many objects changed.
pub fn apply_to_selection(cx: &mut EditorContext, def: Option<&LineStyleDef>) -> usize {
    let ids: Vec<Id> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            crate::editor::ObjectRef::Cad(id) => Some(*id),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        cx.status = "Select a CAD line, polyline, arc or circle".into();
        return 0;
    }
    cx.begin_change("Line Style");
    for id in &ids {
        cx.project.styles.set_line(LineTarget::Cad(*id), def);
    }
    cx.mark_dirty();
    cx.status = match def {
        Some(d) => format!("{} applied to {} object(s)", d.name, ids.len()),
        None => "Library line style removed".to_string(),
    };
    ids.len()
}

/// Gives layer `layer` the library style `def` (Layer Display Options' Line
/// Style column). One undo step.
pub fn apply_to_layer(cx: &mut EditorContext, layer: &str, def: Option<&LineStyleDef>) {
    cx.begin_change("Layer Line Style");
    cx.project
        .styles
        .set_line(LineTarget::Layer(layer.to_string()), def);
    cx.mark_dirty();
}

/// File > Import > Import Line Styles: reads `text` (a .lin file) into the
/// User Catalog; one undo step. Returns `(imported, skipped)`.
pub fn import_text(cx: &mut EditorContext, text: &str, unit: f64) -> (usize, usize) {
    cx.begin_change("Import Line Styles");
    let imp = cx.project.styles.import_lin(text, unit);
    cx.mark_dirty();
    cx.status = format!(
        "Imported {} line style(s) into the User Catalog{}",
        imp.styles.len(),
        if imp.skipped.is_empty() {
            String::new()
        } else {
            format!("; skipped {}", imp.skipped.join(", "))
        }
    );
    (imp.styles.len(), imp.skipped.len())
}

/// Test hook: the next import picks `path` (or cancels with `None`).
#[cfg(test)]
pub fn pick_next(path: Option<&str>) {
    NEXT_PICK.with(|p| *p.borrow_mut() = Some(path.map(str::to_string)));
}

fn pick_file() -> Option<String> {
    if let Some(v) = NEXT_PICK.with(|p| p.borrow_mut().take()) {
        return v;
    }
    if cfg!(test) {
        return None;
    }
    rfd::FileDialog::new()
        .add_filter("Line styles", &["lin", "dat"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Runs a line style menu command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        MANAGEMENT => open(cx),
        IMPORT => match pick_file() {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(text) => {
                    // AutoCAD line files have no unit: paper inches.
                    import_text(cx, &text, 1.0);
                }
                Err(e) => cx.status = format!("Could not read {path}: {e}"),
            },
            None => cx.status = "Import Line Styles cancelled".into(),
        },
        ASSIGN => {
            // Applies the style picked in the library, else Dashed.
            let name = drawing_style().unwrap_or_else(|| enum_name(LineStyle::Dashed).to_string());
            if let Some(def) = cx.project.styles.line_style(&name) {
                apply_to_selection(cx, Some(&def));
            }
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::cad::CadItem;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn add_line(cx: &mut EditorContext) -> Id {
        cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(240.0, 0.0),
            },
        )
    }

    #[test]
    fn the_specification_adds_components_before_and_after_and_reorders_them() {
        let mut s = SpecDraft::new(None, LineStyleDef::new("X", Vec::new()));
        s.add(ComponentKind::Dash);
        s.add(ComponentKind::Dot);
        s.insert_after = false;
        s.selected = 1;
        s.add(ComponentKind::Text);
        let kinds: Vec<ComponentKind> = s.def.components.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![ComponentKind::Dash, ComponentKind::Text, ComponentKind::Dot]
        );
        assert_eq!(s.selected, 1);
        s.move_down();
        assert_eq!(s.def.components[2].kind, ComponentKind::Text);
        s.move_up();
        s.remove();
        assert_eq!(s.def.components.len(), 2);
        assert!(s.error(&LineStyleLibrary::default()).is_none());
        s.def.name = "Dashed".into();
        assert!(
            s.error(&LineStyleLibrary::default()).is_some(),
            "name taken"
        );
        s.def.name = " ".into();
        assert!(s.error(&LineStyleLibrary::default()).is_some());
    }

    #[test]
    fn management_creates_edits_copies_merges_and_purges_in_one_undo_step() {
        let mut cx = cx();
        let line = add_line(&mut cx);
        // A catalog style on a line puts it in the file.
        let gas = cx.project.styles.line_style("Gas Line").unwrap();
        cx.project
            .styles
            .set_line(LineTarget::Cad(line), Some(&gas));
        open(&cx);
        assert!(dialog_open());
        with_dialog(|d| {
            d.new_style();
            d.spec_mut().unwrap().def.name = "Hidden Dash".into();
            d.spec_mut().unwrap().def.components = vec![LineComponent::dash(0.1, 0.05)];
            d.accept_spec().unwrap();
            let sel = d.selected.clone();
            assert_eq!(d.lib.styles[sel[0]].name, "Hidden Dash");
            // Copy numbers the name.
            d.copy();
            assert_eq!(d.lib.styles[d.selected[0]].name, "Hidden Dash 2");
            // The new styles are unused: Delete takes the copy, Purge the rest.
            assert!(d.can_delete());
            assert_eq!(d.delete(), 1);
            assert!(d.lib.get("Hidden Dash 2").is_none());
            assert_eq!(d.purge(), 1);
            assert!(d.lib.get("Hidden Dash").is_none());
            // The used one stays and shows the red plus; system ones the S.
            let u = d.usage();
            assert_eq!(u["Gas Line"].objects, 1);
            assert!(u["Solid"].system);
            let g = d.lib.index_of("Gas Line").unwrap();
            d.select(&[g]);
            assert!(!d.can_delete(), "in use");
            assert_eq!(d.delete(), 0);
            // Add to Library puts it in the User Catalog.
            d.select(&[g]);
            assert_eq!(d.add_to_library(), 1);
            d.lib.auto_purge = true;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert!(cx.project.styles.line_styles.auto_purge);
        assert!(cx
            .project
            .styles
            .user_lines
            .iter()
            .any(|s| s.name == "Gas Line"));
        assert_eq!(cx.undo_label(), Some("Line Style Management"));
        cx.undo();
        assert!(!cx.project.styles.line_styles.auto_purge);
    }

    #[test]
    fn merge_repoints_the_dropped_styles_and_protects_system_ones() {
        let mut cx = cx();
        let a = add_line(&mut cx);
        let b = add_line(&mut cx);
        let mine = LineStyleDef::new("Mine", vec![LineComponent::dash(0.3, 0.1)]);
        let other = LineStyleDef::new("Other", vec![LineComponent::dash(0.2, 0.1)]);
        cx.project.styles.set_line(LineTarget::Cad(a), Some(&mine));
        cx.project.styles.set_line(LineTarget::Cad(b), Some(&other));
        open(&cx);
        with_dialog(|d| {
            let (m, o, s) = (
                d.lib.index_of("Mine").unwrap(),
                d.lib.index_of("Other").unwrap(),
                d.lib.index_of("Dashed").unwrap(),
            );
            let solid = d.lib.index_of("Solid").unwrap();
            d.select(&[solid, s]);
            assert!(
                d.merge().is_err(),
                "a system style below the top is refused"
            );
            // A system style on top of a plain one is allowed to absorb it.
            d.select(&[s, m]);
            assert!(d.can_merge_into_system());
            d.select(&[m, o]);
            d.merge().unwrap();
            assert!(d.lib.get("Other").is_none());
        })
        .unwrap();
        accept_dialog(&mut cx);
        assert_eq!(
            cx.project.styles.assigned_line(&LineTarget::Cad(b)),
            Some("Mine")
        );
    }

    #[test]
    fn a_line_style_goes_on_the_selection_and_on_new_lines_in_one_undo_step() {
        let mut cx = cx();
        let line = add_line(&mut cx);
        cx.selection.items = vec![crate::editor::ObjectRef::Cad(line)];
        let ex = cx.project.styles.line_style("Existing").unwrap();
        assert_eq!(apply_to_selection(&mut cx, Some(&ex)), 1);
        assert_eq!(cx.undo_label(), Some("Line Style"));
        assert_eq!(
            cx.project.styles.assigned_line(&LineTarget::Cad(line)),
            Some("Existing")
        );
        let def = cx
            .project
            .assigned_line_style(line, "CAD, Default")
            .unwrap();
        assert_eq!(def.components[1].text, "EX.");
        cx.undo();
        assert!(cx
            .project
            .styles
            .assigned_line(&LineTarget::Cad(line))
            .is_none());
        // Picking a style in the library: lines drawn next take it.
        set_drawing_style(&cx, Some("Property Line"));
        let drawn = add_line(&mut cx);
        assert_eq!(poll_drawing_style(&mut cx), 1);
        assert_eq!(
            cx.project.styles.assigned_line(&LineTarget::Cad(drawn)),
            Some("Property Line")
        );
        assert_eq!(poll_drawing_style(&mut cx), 0, "once");
        set_drawing_style(&cx, None);
        let plain = add_line(&mut cx);
        assert_eq!(poll_drawing_style(&mut cx), 0);
        assert!(cx
            .project
            .styles
            .assigned_line(&LineTarget::Cad(plain))
            .is_none());
    }

    #[test]
    fn import_line_styles_fills_the_user_catalog() {
        let mut cx = cx();
        let dir = std::env::temp_dir().join(format!("ps_lin_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("custom.lin");
        std::fs::write(
            &file,
            "*CENTER2,Center\nA,1.25,-.25,.25,-.25\n*BAD,x\nA,oops\n",
        )
        .unwrap();
        pick_next(Some(file.to_str().unwrap()));
        assert!(run_command(&mut cx, IMPORT));
        assert_eq!(cx.project.styles.user_lines.len(), 1);
        assert_eq!(cx.project.styles.user_lines[0].name, "CENTER2");
        assert_eq!(cx.undo_label(), Some("Import Line Styles"));
        // The Library button lists catalog and imported styles.
        let names: Vec<String> = library_styles(&cx.project.styles)
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert!(names.contains(&"CENTER2".to_string()) && names.contains(&"Gas Line".to_string()));
        pick_next(None);
        assert!(run_command(&mut cx, IMPORT));
        assert!(cx.status.contains("cancelled"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_dialogs_and_the_picker_draw() {
        let mut cx = cx();
        open(&cx);
        let ctx = egui::Context::default();
        for k in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                if k == 1 {
                    with_dialog(|d| d.new_style());
                }
                host_frame(&mut cx, ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = picker(ui, "t", &cx.project.styles, "Dashed");
                });
            });
        }
        assert!(dialog_open());
        HOST.with(|h| *h.borrow_mut() = None);
    }
}
