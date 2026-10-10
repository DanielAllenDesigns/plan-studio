//! Custom patterns (CAD-71, CAD-79..CAD-81; manual pp. 222, 227-229).
//!
//! * **Create Pattern** (the edit button on group-selected CAD objects, or
//!   CAD > Patterns > Create New Pattern): the Pattern Specification dialog
//!   names the pattern and sets its Repeat Box (width, height) and the
//!   horizontal and vertical shifts, with a preview of the tiling. OK saves it
//!   in the plan; it is then a Library type in every Fill Style panel.
//! * **Pattern window** (CAD > Patterns > Edit Pattern, or the New Pattern
//!   window): the tile is drawn in the editable view with the Repeat Box
//!   around it and an uneditable preview beside it. Pattern Tile Groups (next,
//!   previous, add, delete), the Repeat Box dimensions and Infinite Pattern
//!   Lines are edited here; Save updates the pattern, Add to Library puts a
//!   copy in the User Catalog.
//! * **File > Import > Import Patterns** (.pat): the patterns go to the User
//!   Catalog.

use super::{row, section, Outcome};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{
    self, Align, Align2, Color32, Key, Layout, Modifiers, Pos2, RichText, Shape, Stroke, Vec2,
};
use plan_core::fill_styles::{FillStyle, PatternType};
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::patterns::{CustomPattern, InfiniteLine};
use std::cell::RefCell;

/// CAD > Patterns > Create New Pattern (from the selected CAD objects, else
/// an empty Pattern window).
pub const CREATE: &str = "pattern.create";
/// CAD > Patterns > Edit Pattern (the pattern of the selected object's fill).
pub const EDIT: &str = "pattern.edit";
/// File > Import > Import Patterns.
pub const IMPORT: &str = "pattern.import";
/// CAD > Patterns > Add Pattern to Library.
pub const ADD_TO_LIBRARY: &str = "pattern.addlib";
pub const NEXT_GROUP: &str = "pattern.next";
pub const PREVIOUS_GROUP: &str = "pattern.prev";
pub const ADD_GROUP: &str = "pattern.addgroup";
pub const DELETE_GROUP: &str = "pattern.delgroup";
pub const INFINITE_LINE: &str = "pattern.infinite";

thread_local! {
    static HOST: RefCell<Option<PatternDialog>> = const { RefCell::new(None) };
    static NEXT_PICK: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
}

/// Where the dialog is in the two-step flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Pattern Specification: name and repeat box of a pattern just made.
    Specification,
    /// The Pattern window.
    Window,
}

/// The Pattern Specification dialog and the Pattern window.
pub struct PatternDialog {
    pub mode: Mode,
    /// The pattern being edited.
    pub pattern: CustomPattern,
    /// The name of the saved pattern this edits (`None`: a new one).
    pub editing: Option<String>,
    /// The active Pattern Tile Group.
    pub group: usize,
    /// The selected tile line of the active group.
    pub selected: Option<usize>,
    pub show_preview: bool,
    /// The drag that is drawing a tile line.
    draw_from: Option<Point>,
    pub status: String,
}

#[allow(dead_code)]
impl PatternDialog {
    pub fn new(mode: Mode, pattern: CustomPattern, editing: Option<String>) -> Self {
        Self {
            mode,
            pattern,
            editing,
            group: 0,
            selected: None,
            show_preview: true,
            draw_from: None,
            status: String::new(),
        }
    }

    pub fn error(&self, book: &plan_core::fill_styles::StyleBook) -> Option<String> {
        let name = self.pattern.name.trim();
        if name.is_empty() {
            return Some("A pattern needs a name".into());
        }
        if book
            .patterns
            .iter()
            .any(|p| p.name == name && Some(&p.name) != self.editing.as_ref())
        {
            return Some(format!("There is already a pattern named {name}"));
        }
        None
    }

    /// Draws a tile line (snapped to 1/8").
    pub fn add_segment(&mut self, a: Point, b: Point) -> bool {
        let snap = |p: Point| Point::new((p.x * 8.0).round() / 8.0, (p.y * 8.0).round() / 8.0);
        let (a, b) = (snap(a), snap(b));
        if a.dist(b) < 1e-9 {
            return false;
        }
        let Some(g) = self.pattern.groups.get_mut(self.group) else {
            return false;
        };
        g.tile.push((a, b));
        self.selected = Some(g.tile.len() - 1);
        true
    }

    /// Deletes the selected tile line.
    pub fn delete_selected(&mut self) -> bool {
        let (Some(i), Some(g)) = (self.selected, self.pattern.groups.get_mut(self.group)) else {
            return false;
        };
        if i < g.tile.len() {
            g.tile.remove(i);
            self.selected = None;
            return true;
        }
        false
    }

    pub fn next_group(&mut self) {
        self.group = self.pattern.next_group(self.group);
        self.selected = None;
    }

    pub fn previous_group(&mut self) {
        self.group = self.pattern.previous_group(self.group);
        self.selected = None;
    }

    pub fn add_group(&mut self) {
        self.group = self.pattern.add_group();
        self.selected = None;
    }

    pub fn delete_group(&mut self) -> bool {
        let ok = self.pattern.delete_group(self.group);
        if ok {
            self.group = self.group.min(self.pattern.groups.len() - 1);
            self.selected = None;
        }
        ok
    }

    /// Infinite Pattern Line through the lower left of the active group's
    /// Repeat Box.
    pub fn add_infinite_line(&mut self, angle_deg: f64, spacing: f64) -> Result<(), String> {
        self.pattern
            .add_infinite_line(
                self.group,
                InfiniteLine {
                    angle_deg,
                    spacing,
                    ..InfiniteLine::default()
                },
            )
            .map(|_| ())
    }

    fn pick(&self, p: Point, tol: f64) -> Option<usize> {
        let g = self.pattern.groups.get(self.group)?;
        g.tile
            .iter()
            .enumerate()
            .map(|(i, (a, b))| (i, dist_to_segment(p, *a, *b)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn show(&mut self, ctx: &egui::Context, book: &plan_core::fill_styles::StyleBook) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error(book);
        let title = match self.mode {
            Mode::Specification => "Pattern Specification",
            Mode::Window => "Pattern",
        };
        egui::Window::new(title)
            .id(egui::Id::new("pattern_editor_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(560.0);
                if self.mode == Mode::Specification {
                    row(ui, "Fill Name", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.pattern.name).desired_width(220.0),
                        );
                    });
                } else {
                    row(ui, "Name", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.pattern.name).desired_width(220.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Previous Group").clicked() {
                            self.previous_group();
                        }
                        ui.label(format!(
                            "Tile group {} of {}",
                            self.group + 1,
                            self.pattern.groups.len()
                        ));
                        if ui.button("Next Group").clicked() {
                            self.next_group();
                        }
                        if ui.button("Add Group").clicked() {
                            self.add_group();
                        }
                        if ui
                            .add_enabled(
                                self.pattern.groups.len() > 1,
                                egui::Button::new("Delete Group"),
                            )
                            .clicked()
                        {
                            self.delete_group();
                        }
                    });
                }
                self.repeat_box_ui(ui);
                ui.horizontal_top(|ui| {
                    if self.mode == Mode::Window {
                        ui.vertical(|ui| {
                            section(ui, "Pattern Tile");
                            self.tile_canvas(ui);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        self.selected.is_some(),
                                        egui::Button::new("Delete Line"),
                                    )
                                    .clicked()
                                {
                                    self.delete_selected();
                                }
                                ui.checkbox(&mut self.show_preview, "Display Pattern Preview");
                            });
                            ui.weak("Drag in the tile to draw a line; click a line to select it.");
                        });
                    }
                    ui.vertical(|ui| {
                        section(ui, "Preview");
                        self.preview_canvas(ui);
                    });
                });
                if self.mode == Mode::Window {
                    self.infinite_ui(ui);
                }
                if !self.status.is_empty() {
                    ui.weak(&self.status);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if let Some(e) = &error {
                        ui.colored_label(Color32::from_rgb(0xFF, 0x7B, 0x7B), e);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                error.is_none(),
                                egui::Button::new(RichText::new("   OK   ").strong()),
                            )
                            .clicked()
                        {
                            outcome = Outcome::Ok;
                        }
                        if ui.button("Cancel").clicked() {
                            outcome = Outcome::Cancel;
                        }
                    });
                });
            });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn repeat_box_ui(&mut self, ui: &mut egui::Ui) {
        let Some(g) = self.pattern.groups.get_mut(self.group) else {
            return;
        };
        section(ui, "Pattern Dimensions");
        let d = |ui: &mut egui::Ui, v: &mut f64, lo: f64| {
            ui.add(
                egui::DragValue::new(v)
                    .speed(0.125)
                    .range(lo..=2000.0)
                    .suffix("\""),
            );
        };
        ui.horizontal(|ui| {
            ui.label("Width");
            d(ui, &mut g.width, 0.125);
            ui.label("Height");
            d(ui, &mut g.height, 0.125);
        });
        section(ui, "Pattern Offset");
        ui.horizontal(|ui| {
            ui.label("Horizontal Shift");
            d(ui, &mut g.h_shift, -2000.0);
            ui.label("Vertical Shift");
            d(ui, &mut g.v_shift, -2000.0);
        });
    }

    fn infinite_ui(&mut self, ui: &mut egui::Ui) {
        section(ui, "Infinite Pattern Lines");
        let n = self
            .pattern
            .groups
            .get(self.group)
            .map_or(0, |g| g.infinite.len());
        let mut remove = None;
        if let Some(g) = self.pattern.groups.get_mut(self.group) {
            for (i, l) in g.infinite.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Line {}", i + 1));
                    ui.label("Angle");
                    ui.add(
                        egui::DragValue::new(&mut l.angle_deg)
                            .speed(1.0)
                            .suffix("\u{b0}"),
                    );
                    ui.label("Distance Between Lines");
                    ui.add(
                        egui::DragValue::new(&mut l.spacing)
                            .speed(0.125)
                            .range(0.01..=2000.0),
                    );
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut l.x).speed(0.125));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut l.y).speed(0.125));
                    if ui.button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                g.infinite.remove(i);
            }
        }
        if ui.button("Add Infinite Pattern Line").clicked() && n < 32 {
            let _ = self.add_infinite_line(0.0, 6.0);
        }
    }

    /// The frame of the active group's tile: screen rect and the plan-inch
    /// to pixel factor.
    fn tile_frame(&self, rect: egui::Rect) -> (impl Fn(Point) -> Pos2 + Copy, f64) {
        let (w, h) = self
            .pattern
            .groups
            .get(self.group)
            .map_or((12.0, 12.0), |g| (g.width.max(0.25), g.height.max(0.25)));
        let span = w.max(h) * 1.5;
        let k = f64::from(rect.width().min(rect.height())) / span;
        let (ox, oy) = (
            f64::from(rect.left()) + (f64::from(rect.width()) - w * k) / 2.0,
            f64::from(rect.bottom()) - (f64::from(rect.height()) - h * k) / 2.0,
        );
        (
            move |p: Point| Pos2::new((ox + p.x * k) as f32, (oy - p.y * k) as f32),
            k,
        )
    }

    fn tile_canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(300.0, 300.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0xEC, 0xEA, 0xE3));
        let (to_screen, k) = self.tile_frame(rect);
        let from_screen = |p: Pos2| -> Point {
            // Invert to_screen via two probe points.
            let o = to_screen(Point::ZERO);
            Point::new(f64::from(p.x - o.x) / k, f64::from(o.y - p.y) / k)
        };
        if let Some(g) = self.pattern.groups.get(self.group) {
            let ring: Vec<Pos2> = self
                .pattern
                .repeat_box(self.group)
                .iter()
                .map(|p| to_screen(*p))
                .collect();
            painter.add(Shape::closed_line(
                ring,
                Stroke::new(1.0_f32, Color32::from_rgb(0x2B, 0x6C, 0xD0)),
            ));
            for (i, (a, b)) in g.tile.iter().enumerate() {
                let sel = self.selected == Some(i);
                painter.line_segment(
                    [to_screen(*a), to_screen(*b)],
                    Stroke::new(
                        if sel { 2.5_f32 } else { 1.5_f32 },
                        if sel {
                            Color32::from_rgb(0xD0, 0x2B, 0x2B)
                        } else {
                            Color32::from_rgb(0x2B, 0x2B, 0x2B)
                        },
                    ),
                );
            }
        }
        if resp.drag_started() {
            self.draw_from = resp.interact_pointer_pos().map(from_screen);
        }
        if resp.drag_stopped() {
            if let (Some(a), Some(p)) = (self.draw_from.take(), resp.interact_pointer_pos()) {
                self.add_segment(a, from_screen(p));
            }
        }
        if let (Some(a), Some(p)) = (self.draw_from, resp.hover_pos()) {
            if resp.dragged() {
                painter.line_segment(
                    [to_screen(a), p],
                    Stroke::new(1.0_f32, Color32::from_rgb(0xD0, 0x6A, 0x1C)),
                );
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.selected = self.pick(from_screen(p), 6.0 / k);
            }
        }
    }

    fn preview_canvas(&self, ui: &mut egui::Ui) {
        let size = Vec2::new(220.0, 220.0);
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(0xEC, 0xEA, 0xE3));
        if !self.show_preview {
            return;
        }
        let Some(g) = self.pattern.groups.first() else {
            return;
        };
        // Three repeat boxes across the widest group.
        let width = self
            .pattern
            .groups
            .iter()
            .map(|g| g.width.max(g.height))
            .fold(g.width, f64::max)
            * 3.0;
        let k = f64::from(rect.width()) / width.max(1.0);
        let h = f64::from(rect.height()) / k;
        let outer = vec![
            Point::new(0.0, 0.0),
            Point::new(width, 0.0),
            Point::new(width, h),
            Point::new(0.0, h),
        ];
        let name = self.pattern.name.clone();
        let style = FillStyle {
            pattern: PatternType::Library(name),
            color: plan_core::fill_styles::ColorSource::Single([0x2B, 0x2B, 0x2B]),
            ..FillStyle::default()
        };
        let to_screen = |p: Point| {
            Pos2::new(
                rect.left() + (p.x * k) as f32,
                rect.bottom() - (p.y * k) as f32,
            )
        };
        super::fill_style::paint_fill(
            &painter,
            &to_screen,
            k as f32,
            &outer,
            &[],
            &style,
            std::slice::from_ref(&self.pattern),
            [0x2B, 0x2B, 0x2B],
            [0xEC, 0xEA, 0xE3],
            1.0,
        );
    }
}

/// Applies an accepted draft as one undo step.
pub fn apply(cx: &mut EditorContext, d: &PatternDialog) {
    let name = d.pattern.name.trim().to_string();
    let mut p = d.pattern.clone();
    p.name = name.clone();
    match &d.editing {
        Some(old) if cx.project.styles.patterns.iter().any(|q| &q.name == old) => {
            cx.begin_change("Edit Pattern");
            cx.project.styles.update_pattern(old, p.clone());
            // Renaming repoints the fills that use it.
            if *old != name {
                if let Some(q) = cx
                    .project
                    .styles
                    .patterns
                    .iter_mut()
                    .find(|q| q.name == *old)
                {
                    q.name = name.clone();
                }
                for (_, s) in &mut cx.project.styles.fill_assign {
                    if s.pattern == PatternType::Library(old.clone()) {
                        s.pattern = PatternType::Library(name.clone());
                    }
                }
            }
        }
        _ => {
            cx.begin_change("Create Pattern");
            let saved = cx.project.styles.add_pattern(p);
            cx.status = format!("Pattern {saved} created");
        }
    }
    cx.mark_dirty();
}

/// The CAD items of the selected objects.
fn selected_items(cx: &EditorContext) -> Vec<plan_core::cad::CadItem> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => cx
                .floor()
                .cad
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.item.clone()),
            _ => None,
        })
        .collect()
}

/// Create Pattern from the selected CAD objects: opens the Pattern
/// Specification. Without drawn objects it opens an empty Pattern window.
pub fn create(cx: &mut EditorContext) {
    let items = selected_items(cx);
    let name = unique_name(cx, "New Pattern");
    let d = match plan_core::patterns::CustomPattern::from_cad(&name, &items) {
        Some(p) => PatternDialog::new(Mode::Specification, p, None),
        None => PatternDialog::new(Mode::Window, CustomPattern::new(&name), None),
    };
    HOST.with(|h| *h.borrow_mut() = Some(d));
}

fn unique_name(cx: &EditorContext, base: &str) -> String {
    let all = cx.project.styles.all_patterns();
    if !all.iter().any(|p| p.name == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !all.iter().any(|p| &p.name == n))
        .unwrap_or_else(|| base.to_string())
}

/// Edit Pattern: the Pattern window on the pattern the selected object's
/// fill uses (or the first pattern of the file).
pub fn edit(cx: &mut EditorContext) -> bool {
    let from_selection = cx
        .selection
        .items
        .iter()
        .find_map(|o| super::fill_style::read_fill(cx, *o))
        .and_then(|f| match f.pattern {
            PatternType::Library(n) => Some(n),
            _ => None,
        });
    let name =
        from_selection.or_else(|| cx.project.styles.patterns.first().map(|p| p.name.clone()));
    let Some(name) = name else {
        cx.status = "There is no custom pattern to edit".into();
        return false;
    };
    let Some(p) = cx.project.styles.pattern(&name).cloned() else {
        cx.status = format!("No pattern named {name}");
        return false;
    };
    // A User Catalog pattern is copied into the plan when edited.
    let editing = cx
        .project
        .styles
        .patterns
        .iter()
        .any(|q| q.name == name)
        .then(|| name.clone());
    HOST.with(|h| *h.borrow_mut() = Some(PatternDialog::new(Mode::Window, p, editing)));
    true
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut PatternDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    if d.error(&cx.project.styles).is_some() {
        return false;
    }
    apply(cx, &d);
    true
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx, &cx.project.styles) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => apply(cx, &d),
    }
}

/// File > Import > Import Patterns: reads `text` (a .pat file) into the User
/// Catalog; one undo step. Returns `(imported, skipped)`.
pub fn import_text(cx: &mut EditorContext, text: &str, unit: f64) -> (usize, usize) {
    cx.begin_change("Import Patterns");
    let imp = cx.project.styles.import_pat(text, unit);
    cx.mark_dirty();
    cx.status = format!(
        "Imported {} pattern(s) into the User Catalog",
        imp.patterns.len()
    );
    (imp.patterns.len(), imp.skipped.len())
}

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
        .add_filter("Patterns", &["pat"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Runs a pattern command by id; false when it is not ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        CREATE => create(cx),
        EDIT => {
            edit(cx);
        }
        IMPORT => match pick_file() {
            Some(path) => match std::fs::read_to_string(&path) {
                // AutoCAD pattern files have no unit: plan inches.
                Ok(text) => {
                    import_text(cx, &text, 1.0);
                }
                Err(e) => cx.status = format!("Could not read {path}: {e}"),
            },
            None => cx.status = "Import Patterns cancelled".into(),
        },
        ADD_TO_LIBRARY => {
            let pat = HOST
                .with(|h| h.borrow().as_ref().map(|d| d.pattern.clone()))
                .or_else(|| cx.project.styles.patterns.first().cloned());
            match pat {
                Some(p) => {
                    cx.begin_change("Add Pattern to Library");
                    let name = p.name.clone();
                    let s = &mut cx.project.styles;
                    match s.user_patterns.iter_mut().find(|q| q.name == name) {
                        Some(q) => *q = p,
                        None => s.user_patterns.push(p),
                    }
                    cx.mark_dirty();
                    cx.status = format!("Pattern {name} added to the User Catalog");
                }
                None => cx.status = "There is no pattern to add".into(),
            }
        }
        NEXT_GROUP | PREVIOUS_GROUP | ADD_GROUP | DELETE_GROUP | INFINITE_LINE => {
            HOST.with(|h| {
                if let Some(d) = h.borrow_mut().as_mut() {
                    match id {
                        NEXT_GROUP => d.next_group(),
                        PREVIOUS_GROUP => d.previous_group(),
                        ADD_GROUP => d.add_group(),
                        DELETE_GROUP => {
                            d.delete_group();
                        }
                        _ => {
                            let _ = d.add_infinite_line(0.0, 6.0);
                        }
                    }
                } else {
                    cx.status = "Open a Pattern window first".into();
                }
            });
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
    use plan_core::fill_styles::{fill_geometry, FillTarget};

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn draw_plus(cx: &mut EditorContext) {
        let a = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(100.0, 103.0),
                b: Point::new(106.0, 103.0),
            },
        );
        let b = cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line {
                a: Point::new(103.0, 100.0),
                b: Point::new(103.0, 106.0),
            },
        );
        cx.selection.items = vec![ObjectRef::Cad(a), ObjectRef::Cad(b)];
    }

    #[test]
    fn create_pattern_names_the_pattern_and_adds_it_as_a_library_type() {
        let mut cx = cx();
        draw_plus(&mut cx);
        run_command(&mut cx, CREATE);
        assert!(with_dialog(|d| d.mode == Mode::Specification).unwrap());
        with_dialog(|d| {
            d.pattern.name = "Plus".into();
            assert_eq!(
                (d.pattern.groups[0].width, d.pattern.groups[0].height),
                (6.0, 6.0)
            );
            d.pattern.groups[0].width = 8.0;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert_eq!(cx.undo_label(), Some("Create Pattern"));
        assert_eq!(cx.project.styles.patterns[0].name, "Plus");
        // A slab filled with it tiles the repeat box.
        let slab_outline = vec![
            Point::ZERO,
            Point::new(64.0, 0.0),
            Point::new(64.0, 48.0),
            Point::new(0.0, 48.0),
        ];
        let pats = cx.project.styles.all_patterns();
        let g = fill_geometry(&FillStyle::library("Plus"), &slab_outline, &[], &pats);
        assert_eq!(g.lines.len(), 8 * 8 * 2);
        cx.undo();
        assert!(cx.project.styles.patterns.is_empty());
        // A duplicate name blocks OK.
        draw_plus(&mut cx);
        run_command(&mut cx, CREATE);
        with_dialog(|d| d.pattern.name = "Plus".into()).unwrap();
        assert!(accept_dialog(&mut cx));
        run_command(&mut cx, CREATE);
        with_dialog(|d| d.pattern.name = "Plus".into()).unwrap();
        assert!(!accept_dialog(&mut cx));
        HOST.with(|h| *h.borrow_mut() = None);
    }

    #[test]
    fn the_pattern_window_draws_lines_manages_groups_and_saves_over_the_pattern() {
        let mut cx = cx();
        // No CAD selected: an empty Pattern window.
        run_command(&mut cx, CREATE);
        assert!(with_dialog(|d| d.mode == Mode::Window).unwrap());
        with_dialog(|d| {
            d.pattern.name = "Hash".into();
            assert!(d.add_segment(Point::new(0.0, 0.01), Point::new(12.0, 0.01)));
            assert_eq!(
                d.pattern.groups[0].tile[0].0,
                Point::ZERO,
                "snapped to 1/8\""
            );
            assert!(!d.add_segment(Point::new(1.0, 1.0), Point::new(1.0, 1.0)));
            d.add_segment(Point::new(0.0, 0.0), Point::new(0.0, 12.0));
            assert!(d.delete_selected());
            assert_eq!(d.pattern.groups[0].tile.len(), 1);
            d.add_group();
            assert_eq!(d.group, 1);
            d.pattern.groups[1].width = 6.0;
            assert!(d.add_infinite_line(45.0, 12.0).is_ok());
            assert!(d.add_infinite_line(0.0, 0.0).is_err());
            d.previous_group();
            assert_eq!(d.group, 0);
            d.next_group();
            assert!(d.delete_group());
            assert_eq!(d.pattern.groups.len(), 1);
            assert!(!d.delete_group(), "the last group stays");
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        // Edit Pattern opens the saved pattern; OK updates it in one step.
        assert!(edit(&mut cx));
        with_dialog(|d| {
            assert_eq!(d.editing.as_deref(), Some("Hash"));
            d.pattern.groups[0].width = 24.0;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert_eq!(cx.undo_label(), Some("Edit Pattern"));
        assert_eq!(cx.project.styles.patterns[0].groups[0].width, 24.0);
        assert_eq!(cx.project.styles.patterns.len(), 1);
        // Add Pattern to Library copies it to the User Catalog.
        run_command(&mut cx, ADD_TO_LIBRARY);
        assert_eq!(cx.project.styles.user_patterns[0].name, "Hash");
    }

    #[test]
    fn renaming_a_pattern_repoints_the_fills_that_use_it() {
        let mut cx = cx();
        cx.project.styles.add_pattern(CustomPattern::new("Old"));
        cx.project
            .styles
            .apply_fill(FillTarget::Slab(9), Some(FillStyle::library("Old")));
        assert!(edit(&mut cx));
        with_dialog(|d| d.pattern.name = "New".into()).unwrap();
        assert!(accept_dialog(&mut cx));
        assert_eq!(cx.project.styles.patterns[0].name, "New");
        assert_eq!(
            cx.project
                .styles
                .fill_for(&FillTarget::Slab(9))
                .unwrap()
                .pattern,
            PatternType::Library("New".into())
        );
    }

    #[test]
    fn import_patterns_fills_the_user_catalog() {
        let mut cx = cx();
        let dir = std::env::temp_dir().join(format!("ps_pat_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hatch.pat");
        std::fs::write(&file, "*ANSI31, iron\n45, 0,0, 0,3.175\n").unwrap();
        pick_next(Some(file.to_str().unwrap()));
        assert!(run_command(&mut cx, IMPORT));
        assert_eq!(cx.project.styles.user_patterns[0].name, "ANSI31");
        assert_eq!(cx.undo_label(), Some("Import Patterns"));
        // It is a Library type of the Fill Style panel at once.
        let all = cx.project.styles.all_patterns();
        assert_eq!(all[0].name, "ANSI31");
        let _ = std::fs::remove_dir_all(&dir);
        pick_next(None);
        run_command(&mut cx, IMPORT);
        assert!(cx.status.contains("cancelled"));
    }

    #[test]
    fn the_dialog_draws_in_both_modes() {
        let mut cx = cx();
        run_command(&mut cx, CREATE);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| host_frame(&mut cx, ctx));
        }
        assert!(dialog_open());
        HOST.with(|h| *h.borrow_mut() = None);
    }
}
