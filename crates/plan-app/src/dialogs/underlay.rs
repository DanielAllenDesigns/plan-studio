//! The Underlays window (Tools > Underlays..., File > Import > Underlay
//! Image...): lists the active floor's underlays and edits the one that is
//! active: name, opacity, rotation, show and lock, the two-point calibration
//! (click two points on the picture with the Underlay tool, then type the
//! real distance here), the picture's real width, and Delete.
//!
//! The state (which underlay is active, the calibration clicks) lives in a
//! thread-local the Underlay tool reads (`tools::underlay`).

use crate::editor::{EditorContext, EditorRequest};
use crate::tools::underlay as tool;
use crate::tools::ToolId;
use eframe::egui::{self, Align2};
use plan_core::geometry::Point;
use plan_core::underlay::Underlay;
use plan_core::units::parse_ft_in;
use plan_core::Id;
use std::cell::RefCell;

/// A two-point calibration in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    pub id: Id,
    pub a: Option<Point>,
    pub b: Option<Point>,
    /// Rotate to Align instead of Point to Point Resize: the second click
    /// turns the picture at once, there is no distance to type.
    pub align: bool,
}

#[derive(Default)]
struct State {
    open: bool,
    active: Option<Id>,
    calibration: Option<Calibration>,
    /// The scanned page of a PDF the next import takes (1-based).
    pdf_page: usize,
}

#[derive(Default)]
struct Texts {
    distance: String,
    width: String,
    name_for: Option<Id>,
    name: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static TEXTS: RefCell<Texts> = RefCell::new(Texts::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens the window.
pub fn open() {
    state(|s| s.open = true);
}

pub fn is_open() -> bool {
    state(|s| s.open)
}

/// The underlay the window edits and the tool drags.
pub fn active() -> Option<Id> {
    state(|s| s.active)
}

pub fn set_active(id: Option<Id>) {
    state(|s| s.active = id);
}

/// The calibration in progress, if any.
pub fn calibration() -> Option<Calibration> {
    state(|s| s.calibration)
}

/// Starts a two-point calibration of `id`: the next two clicks of the
/// Underlay tool are the points.
pub fn begin_calibration(id: Id) {
    begin_trace(id, false);
}

/// Starts Rotate to Align of `id`: the next two clicks of the Underlay tool
/// are the ends of a line that should be level.
pub fn begin_alignment(id: Id) {
    begin_trace(id, true);
}

fn begin_trace(id: Id, align: bool) {
    state(|s| {
        s.active = Some(id);
        s.calibration = Some(Calibration {
            id,
            a: None,
            b: None,
            align,
        });
    });
    TEXTS.with(|t| t.borrow_mut().distance.clear());
}

/// A click of the Underlay tool in Calibrate mode: the first click is A,
/// the second B; further clicks start over from A.
pub fn push_calibration_point(p: Point) {
    state(|s| {
        if let Some(c) = &mut s.calibration {
            match (c.a, c.b) {
                (None, _) => c.a = Some(p),
                (Some(_), None) => c.b = Some(p),
                (Some(_), Some(_)) => {
                    c.a = Some(p);
                    c.b = None;
                }
            }
        }
    });
}

pub fn cancel_calibration() {
    state(|s| s.calibration = None);
}

/// The scanned page of a PDF the next import takes.
pub fn pdf_page() -> usize {
    state(|s| s.pdf_page.max(1))
}

/// Asks for a picture file and places it as an underlay.
pub fn pick_and_import(cx: &mut EditorContext) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(
            "Picture (PNG, JPEG) or scanned PDF",
            &["png", "jpg", "jpeg", "pdf"],
        )
        .pick_file()
    else {
        return;
    };
    let is_pdf = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    let result = if is_pdf {
        tool::import_pdf_page(cx, &path, pdf_page())
    } else {
        tool::import_image(cx, &path)
    };
    cx.status = match result {
        Ok(id) => {
            open();
            format!(
                "Placed {}; calibrate it with two points to set the scale",
                cx.floor()
                    .underlay(id)
                    .map_or("the picture", |u| u.name.as_str())
            )
        }
        Err(e) => e,
    };
}

/// Runs `f` on the underlay `id` as an undo step named `label` (consecutive
/// calls with one label, a slider drag, share the step).
fn edit(cx: &mut EditorContext, id: Id, label: &str, merged: bool, f: impl FnOnce(&mut Underlay)) {
    if merged {
        cx.begin_change_merged(label);
    } else {
        cx.begin_change(label);
    }
    let fl = cx.floor;
    if let Some(u) = cx.project.floors[fl].underlay_mut(id) {
        f(u);
    }
    cx.mark_dirty();
}

/// Draws the window when it is open.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    // The picture and CAD detail windows are drawn from here too: this is
    // the one per-frame hook of the images and details owner.
    super::images::show_windows(ctx, cx);
    super::details::show_windows(ctx, cx);
    if !is_open() {
        return;
    }
    let mut open = true;
    egui::Window::new("Underlays")
        .id(egui::Id::new("underlays_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::RIGHT_TOP)
        .default_pos(ctx.screen_rect().right_top() + egui::vec2(-16.0, 90.0))
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            content(ui, cx);
        });
    if !open {
        state(|s| s.open = false);
        cancel_calibration();
    }
}

fn content(ui: &mut egui::Ui, cx: &mut EditorContext) {
    ui.horizontal(|ui| {
        if ui.button("Import Picture\u{2026}").clicked() {
            pick_and_import(cx);
        }
        let mut page = pdf_page();
        ui.label("PDF page");
        if ui
            .add(egui::DragValue::new(&mut page).range(1..=999))
            .on_hover_text("Which scanned page of a PDF to take")
            .changed()
        {
            state(|s| s.pdf_page = page);
        }
    });
    ui.weak("PNG, JPEG, or a scanned PDF (a PDF drawn with vector lines cannot be used)");
    ui.separator();
    let list: Vec<(Id, String, bool, bool)> = cx
        .floor()
        .underlays
        .iter()
        .rev()
        .map(|u| (u.id, u.name.clone(), u.visible, u.locked))
        .collect();
    if list.is_empty() {
        ui.weak("No underlays on this floor.");
        return;
    }
    let active = active().filter(|a| list.iter().any(|(id, ..)| id == a));
    for (id, name, visible, locked) in &list {
        ui.horizontal(|ui| {
            let mut v = *visible;
            if ui.checkbox(&mut v, "").on_hover_text("Show").changed() {
                edit(cx, *id, "Show Underlay", false, |u| u.visible = v);
            }
            let mut l = *locked;
            if ui.checkbox(&mut l, "Lock").changed() {
                edit(cx, *id, "Lock Underlay", false, |u| u.locked = l);
            }
            if ui.selectable_label(active == Some(*id), name).clicked() {
                set_active(Some(*id));
            }
        });
    }
    let Some(id) = active else {
        ui.separator();
        ui.weak("Pick an underlay to edit it.");
        return;
    };
    let Some(u) = cx.floor().underlay(id).cloned() else {
        return;
    };
    ui.separator();
    selected(ui, cx, &u);
}

fn selected(ui: &mut egui::Ui, cx: &mut EditorContext, u: &Underlay) {
    let id = u.id;
    // Name.
    TEXTS.with(|t| {
        let mut t = t.borrow_mut();
        if t.name_for != Some(id) {
            t.name_for = Some(id);
            t.name = u.name.clone();
        }
        ui.horizontal(|ui| {
            ui.label("Name");
            let r = ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(200.0));
            if r.lost_focus() && t.name != u.name && !t.name.trim().is_empty() {
                let n = t.name.trim().to_string();
                edit(cx, id, "Rename Underlay", false, |u| u.name = n);
            }
        });
    });
    // Opacity and rotation.
    let mut pct = f64::from(u.opacity) * 100.0;
    ui.horizontal(|ui| {
        ui.label("Opacity");
        if ui
            .add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix("%"))
            .changed()
        {
            edit(cx, id, "Underlay Opacity", true, |u| {
                u.set_opacity((pct / 100.0) as f32)
            });
        }
    });
    let mut deg = u.rotation.to_degrees();
    ui.horizontal(|ui| {
        ui.label("Rotation");
        if ui
            .add(egui::DragValue::new(&mut deg).speed(0.1).suffix("\u{b0}"))
            .changed()
        {
            // Rotate about the picture's middle.
            edit(cx, id, "Rotate Underlay", true, |u| {
                let mid = u.pixel_to_plan(
                    f64::from(u.pixel_width) * 0.5,
                    f64::from(u.pixel_height) * 0.5,
                );
                u.rotation = deg.to_radians();
                let after = u.pixel_to_plan(
                    f64::from(u.pixel_width) * 0.5,
                    f64::from(u.pixel_height) * 0.5,
                );
                u.translate(mid.sub(after));
            });
        }
    });
    let (w, h) = u.size();
    ui.label(format!(
        "Picture {} \u{d7} {} px; in the plan {} \u{d7} {}{}",
        u.pixel_width,
        u.pixel_height,
        cx.fmt_dim(w),
        cx.fmt_dim(h),
        if u.calibrated {
            ""
        } else {
            " (not calibrated)"
        }
    ));
    ui.horizontal(|ui| {
        if ui
            .button("Point to Point Resize\u{2026}")
            .on_hover_text("Click two points on the picture, then type their real distance")
            .clicked()
        {
            begin_calibration(id);
            cx.requests.push(EditorRequest::SetTool(ToolId::Underlay));
        }
        if ui
            .button("Rotate to Align\u{2026}")
            .on_hover_text("Click the ends of a line that should be level (or plumb)")
            .clicked()
        {
            begin_alignment(id);
            cx.requests.push(EditorRequest::SetTool(ToolId::Underlay));
        }
        if ui
            .button("Move")
            .on_hover_text("Drag the picture with the mouse")
            .clicked()
        {
            cancel_calibration();
            cx.requests.push(EditorRequest::SetTool(ToolId::Underlay));
        }
        if ui.button("Delete").clicked() {
            cx.begin_change("Delete Underlay");
            let fl = cx.floor;
            cx.project.remove_underlay(fl, id);
            cx.mark_dirty();
            set_active(None);
            cancel_calibration();
        }
    });
    // Real width.
    ui.horizontal(|ui| {
        ui.label("Real width");
        let go = TEXTS.with(|t| {
            let mut t = t.borrow_mut();
            let r = ui.add(
                egui::TextEdit::singleline(&mut t.width)
                    .hint_text("e.g. 40'-6\"")
                    .desired_width(110.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.button("Set").clicked() || enter
        });
        if go {
            let text = TEXTS.with(|t| t.borrow().width.clone());
            match parse_ft_in(&text) {
                Some(width) if tool::set_width(cx, id, width) => {
                    cx.status = format!("Underlay width set to {}", cx.fmt_dim(width));
                }
                _ => cx.status = "Type a width such as 40'-6\"".into(),
            }
        }
    });
    calibration_pane(ui, cx, id);
}

fn calibration_pane(ui: &mut egui::Ui, cx: &mut EditorContext, id: Id) {
    let Some(c) = calibration().filter(|c| c.id == id) else {
        return;
    };
    ui.separator();
    ui.strong(if c.align {
        "Rotate to Align"
    } else {
        "Point to Point Resize"
    });
    let mark = |p: Option<Point>| if p.is_some() { "set" } else { "click it" };
    ui.label(format!("Point A: {}   Point B: {}", mark(c.a), mark(c.b)));
    let (Some(a), Some(b)) = (c.a, c.b) else {
        ui.weak("Pick the Underlay tool's two points on the picture.");
        if ui.button("Cancel").clicked() {
            cancel_calibration();
        }
        return;
    };
    ui.label(format!("Drawn distance now: {}", cx.fmt_dim(a.dist(b))));
    let mut apply = false;
    ui.horizontal(|ui| {
        ui.label("Real distance");
        TEXTS.with(|t| {
            let mut t = t.borrow_mut();
            let r = ui.add(
                egui::TextEdit::singleline(&mut t.distance)
                    .hint_text("e.g. 24'-0\"")
                    .desired_width(110.0),
            );
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                apply = true;
            }
        });
        if ui.button("Apply").clicked() {
            apply = true;
        }
        if ui.button("Cancel").clicked() {
            cancel_calibration();
        }
    });
    if apply {
        let text = TEXTS.with(|t| t.borrow().distance.clone());
        match parse_ft_in(&text) {
            Some(real) if tool::trace::resize_underlay(cx, id, a, b, real) => cancel_calibration(),
            Some(_) => {}
            None => cx.status = "Type the real distance, such as 24'-0\"".into(),
        }
    }
}
