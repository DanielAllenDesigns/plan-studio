//! Chief-style specification dialogs (docs/chief-x18-dialogs.md).
//!
//! Every Chief object dialog shares one frame: a vertical tab list on the
//! left, the active tab's panel in the middle, a preview on the right and a
//! Help / Cancel / OK row along the bottom. [`SpecDialog`] draws that frame as
//! an `egui::Window`; each object supplies its pages through [`SpecPages`].
//!
//! Dialogs edit a cloned draft. OK hands the draft back to the app, Cancel or
//! Escape drops it, Enter is OK. Controls the model cannot store yet are drawn
//! disabled (the full option set stays discoverable) or, where the spec asks
//! for it, kept per session by the app (see `WallExtras` / `OpeningExtras`).

mod defaults;
mod opening;
mod wall;
mod wall_types;

pub use defaults::{DefaultsDialog, DefaultsEntry, DefaultsOutcome};
pub use opening::{place_from_template, OpeningDialog, OpeningExtras, OpeningTarget};
pub use wall::{WallDialog, WallExtras, WallTarget};
pub use wall_types::WallTypeDialog;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Key, Layout, Modifiers, Painter, Pos2, Rect, RichText,
    Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};
use plan_core::units::{fmt_ft_in, parse_ft_in};
use plan_core::{Id, Opening, OpeningKind};
use std::collections::HashMap;
use std::hash::Hash;

/// Fixed width of the left tab list.
const TAB_LIST_WIDTH: f32 = 150.0;
/// Fixed width of the right preview panel.
const PREVIEW_WIDTH: f32 = 220.0;
const BOTTOM_HEIGHT: f32 = 40.0;
const LABEL_WIDTH: f32 = 175.0;
const SESSION_NOTE: &str = "Stored per session until the model grows these fields";

const ERROR_RED: Color32 = Color32::from_rgb(0xE0, 0x4B, 0x4B);

// The preview is drawn like a drawing sheet so it reads at any UI brightness.
const PV_BG: Color32 = Color32::from_rgb(0xEC, 0xEA, 0xE3);
const PV_INK: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const PV_FAINT: Color32 = Color32::from_rgb(0x9A, 0x98, 0x92);
const PV_WALL: Color32 = Color32::from_rgb(0xC9, 0xC6, 0xBC);
const PV_GLASS: Color32 = Color32::from_rgb(0xB9, 0xD6, 0xE8);
const PV_ACCENT: Color32 = Color32::from_rgb(0xD0, 0x6A, 0x1C);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Open,
    Ok,
    Cancel,
}

/// One entry of the vertical tab list. Disabled tabs stay visible.
#[derive(Clone, Copy)]
pub struct Tab {
    pub name: &'static str,
    pub enabled: bool,
}

const fn on(name: &'static str) -> Tab {
    Tab {
        name,
        enabled: true,
    }
}

const fn off(name: &'static str) -> Tab {
    Tab {
        name,
        enabled: false,
    }
}

/// What an object dialog provides to the shared frame.
pub trait SpecPages {
    fn tabs(&self) -> &'static [Tab];
    /// A reason OK is not allowed right now (shown in red next to the buttons).
    fn error(&self) -> Option<String>;
    /// Draws the panel for tab `tab` (an index into [`SpecPages::tabs`]).
    fn page(&mut self, ui: &mut Ui, tab: usize);
    /// Draws the preview inside `rect`.
    fn preview(&self, painter: &Painter, rect: Rect);
}

/// The shared dialog frame.
pub struct SpecDialog {
    title: String,
    id: egui::Id,
    active: usize,
    show_help: bool,
}

impl SpecDialog {
    /// `title` is the full window title, e.g. `"Wall Specification"`; `key`
    /// keeps the window position/size separate between dialog kinds.
    pub fn new(title: impl Into<String>, key: impl Hash) -> Self {
        Self {
            title: title.into(),
            id: egui::Id::new(("spec_dialog", key)),
            active: 0,
            show_help: false,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, pages: &mut dyn SpecPages) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = pages.error();
        let tabs = pages.tabs();
        if !tabs.get(self.active).is_some_and(|t| t.enabled) {
            self.active = 0;
        }
        egui::Window::new(self.title.clone())
            .id(self.id)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([820.0, 560.0])
            .min_size([640.0, 380.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                let avail = ui.available_rect_before_wrap();
                ui.allocate_rect(avail, Sense::hover());
                let body = Rect::from_min_max(
                    avail.min,
                    Pos2::new(avail.max.x, avail.max.y - BOTTOM_HEIGHT),
                );
                let bottom = Rect::from_min_max(Pos2::new(avail.min.x, body.max.y), avail.max);
                let tab_rect =
                    Rect::from_min_size(body.min, Vec2::new(TAB_LIST_WIDTH, body.height()));
                let prev_rect =
                    Rect::from_min_max(Pos2::new(body.max.x - PREVIEW_WIDTH, body.min.y), body.max);
                let mid_rect = Rect::from_min_max(
                    Pos2::new(tab_rect.max.x + 10.0, body.min.y),
                    Pos2::new(prev_rect.min.x - 10.0, body.max.y),
                );

                // Tab list.
                let mut tc = ui.new_child(UiBuilder::new().max_rect(tab_rect));
                tc.painter()
                    .rect_filled(tab_rect, 3.0, tc.visuals().extreme_bg_color);
                egui::ScrollArea::vertical()
                    .id_salt("spec_tab_list")
                    .auto_shrink([false, false])
                    .show(&mut tc, |ui| {
                        ui.add_space(4.0);
                        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                            for (i, tab) in tabs.iter().enumerate() {
                                let label = egui::SelectableLabel::new(self.active == i, tab.name);
                                if ui.add_enabled(tab.enabled, label).clicked() {
                                    self.active = i;
                                }
                            }
                        });
                    });

                // Active tab panel.
                let mut mc = ui.new_child(UiBuilder::new().max_rect(mid_rect));
                egui::ScrollArea::vertical()
                    .id_salt(("spec_page", self.active))
                    .auto_shrink([false, false])
                    .show(&mut mc, |ui| {
                        ui.set_width(ui.available_width());
                        pages.page(ui, self.active);
                    });

                // Preview.
                let painter = ui.painter_at(prev_rect);
                painter.rect_filled(prev_rect, 4.0, PV_BG);
                painter.rect_stroke(
                    prev_rect,
                    4.0,
                    Stroke::new(1.0_f32, Color32::from_gray(110)),
                    StrokeKind::Inside,
                );
                pages.preview(&painter, prev_rect.shrink(8.0));

                // Bottom row.
                ui.painter().hline(
                    avail.x_range(),
                    bottom.min.y + 2.0,
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                let mut bc = ui.new_child(
                    UiBuilder::new()
                        .max_rect(bottom.shrink2(Vec2::new(0.0, 5.0)))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                if bc.button("Help").clicked() {
                    self.show_help = !self.show_help;
                }
                if self.show_help {
                    bc.weak("Help pages are not written yet. Hover a control for a tooltip.");
                }
                bc.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(RichText::new("   OK   ").strong());
                    let clicked = ui.add_enabled(error.is_none(), ok).clicked();
                    if clicked {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = &error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        // Consumed so a dialog stacked underneath (Default Settings) does not
        // also react to the same key press.
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

// ----- field helpers -----

/// Section heading: a bold label followed by a horizontal rule, like Chief's
/// `General ──────`.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong());
        let width = ui.available_width().max(8.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
    });
    ui.add_space(2.0);
}

/// A labelled row: fixed-width label, then the control(s).
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(LABEL_WIDTH, 20.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        add(ui)
    })
    .inner
}

/// A checkbox that is shown but cannot be changed.
pub fn dis_check(ui: &mut Ui, label: &str, checked: bool) {
    let mut v = checked;
    ui.add_enabled(false, egui::Checkbox::new(&mut v, label));
}

/// A radio button that is shown but cannot be changed.
pub fn dis_radio(ui: &mut Ui, label: &str, selected: bool) {
    ui.add_enabled(false, egui::RadioButton::new(selected, label));
}

/// A combo box that is shown but cannot be changed.
pub fn dis_combo(ui: &mut Ui, salt: &str, text: &str) {
    ui.add_enabled_ui(false, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(text)
            .show_ui(ui, |_| {});
    });
}

/// Plain `ui.checkbox` with the "stored per session" tooltip.
pub fn session_check(ui: &mut Ui, value: &mut bool, label: &str) -> egui::Response {
    ui.checkbox(value, label).on_hover_text(SESSION_NOTE)
}

/// Inches in the short architectural form, without a leading `0'-`.
pub fn fmt_short(inches: f64) -> String {
    let s = fmt_ft_in(inches);
    match s.strip_prefix("0'-") {
        Some(rest) => rest.to_string(),
        None => s,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FieldKind {
    Length,
    Degrees,
}

impl FieldKind {
    fn format(self, v: f64) -> String {
        match self {
            FieldKind::Length => fmt_ft_in(v),
            FieldKind::Degrees => format!("{v:.1}\u{B0}"),
        }
    }

    fn parse(self, s: &str) -> Option<f64> {
        match self {
            FieldKind::Length => parse_ft_in(s),
            FieldKind::Degrees => s
                .trim()
                .trim_end_matches('\u{B0}')
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite()),
        }
    }
}

/// Raw text of the length / angle fields being edited. A field that is not in
/// `bufs` shows the formatted value; one that is shows what the user typed
/// (red when it does not parse). Valid text updates the value as it is typed,
/// so OK never has to chase half-finished edits; invalid text blocks OK.
#[derive(Default)]
pub struct Fields {
    bufs: HashMap<&'static str, String>,
}

impl Fields {
    pub fn any_invalid(&self) -> bool {
        self.bufs.iter().any(|(k, text)| {
            let kind = if k.starts_with("deg_") {
                FieldKind::Degrees
            } else {
                FieldKind::Length
            };
            kind.parse(text).is_none()
        })
    }

    /// Feet-inches text field. Returns true when `value` changed.
    pub fn length(&mut self, ui: &mut Ui, key: &'static str, value: &mut f64) -> bool {
        self.edit(ui, key, FieldKind::Length, value)
    }

    /// Degrees text field; `key` must start with `deg_`.
    pub fn degrees(&mut self, ui: &mut Ui, key: &'static str, value: &mut f64) -> bool {
        debug_assert!(key.starts_with("deg_"));
        self.edit(ui, key, FieldKind::Degrees, value)
    }

    pub fn length_row(
        &mut self,
        ui: &mut Ui,
        label: &str,
        key: &'static str,
        value: &mut f64,
    ) -> bool {
        row(ui, label, |ui| self.length(ui, key, value))
    }

    pub fn degrees_row(
        &mut self,
        ui: &mut Ui,
        label: &str,
        key: &'static str,
        value: &mut f64,
    ) -> bool {
        row(ui, label, |ui| self.degrees(ui, key, value))
    }

    fn edit(&mut self, ui: &mut Ui, key: &'static str, kind: FieldKind, value: &mut f64) -> bool {
        let mut text = self
            .bufs
            .get(key)
            .cloned()
            .unwrap_or_else(|| kind.format(*value));
        let invalid = kind.parse(&text).is_none();
        let mut edit = egui::TextEdit::singleline(&mut text).desired_width(100.0);
        if invalid {
            edit = edit.text_color(ERROR_RED);
        }
        let resp = ui.add(edit);
        let mut changed = false;
        if resp.changed() {
            if let Some(v) = kind.parse(&text) {
                changed = (v - *value).abs() > 1e-9;
                *value = v;
            }
            self.bufs.insert(key, text);
        } else if !resp.has_focus() && self.bufs.get(key).is_some_and(|t| kind.parse(t).is_some()) {
            self.bufs.remove(key);
        }
        changed
    }
}

// ----- preview drawing -----

fn pv_text(p: &Painter, pos: Pos2, anchor: Align2, text: impl ToString, size: f32) {
    p.text(
        pos,
        anchor,
        text.to_string(),
        FontId::proportional(size),
        PV_INK,
    );
}

fn layer_color(name: &str) -> Color32 {
    match name {
        "Siding" => Color32::from_rgb(0xD8, 0xC7, 0x9E),
        "Stucco" => Color32::from_rgb(0xDD, 0xCF, 0xB0),
        "Brick" => Color32::from_rgb(0xB5, 0x5D, 0x44),
        "Sheathing" => Color32::from_rgb(0xC8, 0xA9, 0x6E),
        "Framing" => Color32::from_rgb(0xE6, 0xCF, 0x93),
        "Drywall" => Color32::from_rgb(0xF4, 0xF2, 0xEC),
        "Concrete" => Color32::from_rgb(0xA5, 0xA5, 0xA2),
        "Stone" => Color32::from_rgb(0x9C, 0x96, 0x8C),
        "Air Space" => Color32::from_rgb(0xE4, 0xEE, 0xF2),
        _ => Color32::from_rgb(0xCC, 0xCC, 0xCC),
    }
}

/// A layer-stack strip with bands proportional to layer thickness, the total
/// above it and a legend below.
pub fn layer_stack(p: &Painter, area: Rect, layers: &[(&str, f64)]) {
    let total: f64 = layers.iter().map(|l| l.1).sum::<f64>().max(0.01);
    let strip_h = 36.0_f32.min((area.height() - 20.0).max(8.0));
    let strip = Rect::from_min_size(
        Pos2::new(area.min.x, area.min.y + 16.0),
        Vec2::new(area.width(), strip_h),
    );
    let mut x = strip.min.x;
    for (name, t) in layers {
        let w = (*t / total) as f32 * strip.width();
        let r = Rect::from_min_size(Pos2::new(x, strip.min.y), Vec2::new(w, strip_h));
        p.rect_filled(r, 0.0, layer_color(name));
        p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, PV_INK), StrokeKind::Inside);
        x += w;
    }
    pv_text(
        p,
        Pos2::new(strip.center().x, area.min.y + 7.0),
        Align2::CENTER_CENTER,
        fmt_short(total),
        11.0,
    );
    let mut y = strip.max.y + 10.0;
    for (name, t) in layers {
        if y + 12.0 > area.max.y {
            break;
        }
        let sw = Rect::from_min_size(Pos2::new(area.min.x, y - 4.0), Vec2::splat(8.0));
        p.rect_filled(sw, 0.0, layer_color(name));
        p.rect_stroke(sw, 0.0, Stroke::new(0.8_f32, PV_INK), StrokeKind::Inside);
        pv_text(
            p,
            Pos2::new(area.min.x + 14.0, y),
            Align2::LEFT_CENTER,
            format!("{name}  {}", fmt_short(*t)),
            11.0,
        );
        y += 14.0;
    }
}

/// Plan view of a wall laid out horizontally (start at the left, the left-hand
/// side of the wall up) with its openings. Doors show their leaf and swing arc
/// for `swing_deg`; `highlight` outlines one opening.
pub fn wall_plan_sketch(
    p: &Painter,
    area: Rect,
    wall_len: f64,
    thick: f64,
    openings: &[Opening],
    highlight: Option<Id>,
    swing_deg: f64,
) {
    let len = wall_len.max(1.0);
    let widest_door = openings
        .iter()
        .filter(|o| o.kind == OpeningKind::Door)
        .map(|o| o.width)
        .fold(thick, f64::max);
    let s_x = (area.width() as f64 - 12.0) / len;
    let s_y = (area.height() as f64 * 0.5 - 18.0).max(10.0) / widest_door.max(1.0);
    let s = s_x.min(s_y).clamp(0.02, 6.0) as f32;
    let th = (thick as f32 * s).max(5.0);
    let yc = area.center().y + 6.0;
    let x0 = area.center().x - len as f32 * s * 0.5;
    let xo = |off: f64| x0 + off as f32 * s;
    let ink = Stroke::new(1.0_f32, PV_INK);

    let wall = Rect::from_min_max(
        Pos2::new(xo(0.0), yc - th * 0.5),
        Pos2::new(xo(len), yc + th * 0.5),
    );
    p.rect_filled(wall, 0.0, PV_WALL);
    p.rect_stroke(wall, 0.0, ink, StrokeKind::Inside);
    for o in openings {
        let (a, b) = (xo(o.start_offset()), xo(o.end_offset()));
        let gap = Rect::from_min_max(
            Pos2::new(a, yc - th * 0.5 - 1.0),
            Pos2::new(b, yc + th * 0.5 + 1.0),
        );
        p.rect_filled(gap, 0.0, PV_BG);
        for x in [a, b] {
            p.line_segment(
                [Pos2::new(x, yc - th * 0.5), Pos2::new(x, yc + th * 0.5)],
                ink,
            );
        }
        match o.kind {
            OpeningKind::Door => {
                let (hinge_x, dir) = if o.swing_flipped { (b, -1.0) } else { (a, 1.0) };
                let r = o.width as f32 * s;
                let ang = swing_deg.clamp(0.0, 180.0).to_radians() as f32;
                let tip = Pos2::new(hinge_x + dir * r * ang.cos(), yc - th * 0.5 - r * ang.sin());
                let hinge = Pos2::new(hinge_x, yc - th * 0.5);
                p.line_segment([hinge, tip], ink);
                let arc: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let a = ang * i as f32 / 16.0;
                        Pos2::new(hinge.x + dir * r * a.cos(), hinge.y - r * a.sin())
                    })
                    .collect();
                p.add(Shape::line(arc, Stroke::new(0.8_f32, PV_FAINT)));
            }
            OpeningKind::Window => {
                for dy in [-th * 0.5, 0.0, th * 0.5] {
                    p.line_segment(
                        [Pos2::new(a, yc + dy), Pos2::new(b, yc + dy)],
                        Stroke::new(0.8_f32, PV_INK),
                    );
                }
            }
        }
        if highlight == Some(o.id) {
            p.rect_stroke(
                gap.expand(2.0),
                0.0,
                Stroke::new(1.5_f32, PV_ACCENT),
                StrokeKind::Outside,
            );
        }
    }
    pv_text(
        p,
        Pos2::new(
            area.center().x,
            (yc + th * 0.5 + 12.0).min(area.max.y - 6.0),
        ),
        Align2::CENTER_CENTER,
        fmt_ft_in(wall_len),
        11.0,
    );
}
