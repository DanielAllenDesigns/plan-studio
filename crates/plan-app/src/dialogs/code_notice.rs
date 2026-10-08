//! The code notice: a small amber line under a dialog field that is past a
//! code minimum, with the citation and a "Set to code" button.
//!
//! Dialogs warn, they never block: the notice only reports, and the button
//! writes the limit into the field (an edit of the dialog's draft, so the
//! dialog's OK is still the one undo step). The limits come from
//! [`crate::editor::code::active`], the minimums of the open plan's Plan Check
//! settings.
//!
//! * [`code_notice`] is the common case: a length field with a maximum or a
//!   minimum. It checks `*value` and, on the button, sets it to the limit.
//! * [`code_notice_for`] checks a value the caller computes (a net clear
//!   area, a solved riser) and tells the caller when the button was clicked.
//! * [`check_limit`] and [`notice_text`] are the same decision without a
//!   `Ui`, for tests and for the Plan Check "Fix" action.

use eframe::egui::{self, Color32, RichText, Ui};

/// Which side of the limit is legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    /// The value may not exceed the limit (a riser height).
    Max,
    /// The value may not be under the limit (a tread depth).
    Min,
}

impl LimitKind {
    /// `max` or `min`, as the notice words it.
    pub fn word(self) -> &'static str {
        match self {
            LimitKind::Max => "max",
            LimitKind::Min => "min",
        }
    }
}

/// Tolerance when comparing a value with its limit.
const EPS: f64 = 1e-6;

/// True when `value` is on the wrong side of `limit`.
pub fn check_limit(value: f64, limit: f64, kind: LimitKind) -> bool {
    match kind {
        LimitKind::Max => value > limit + EPS,
        LimitKind::Min => value < limit - EPS,
    }
}

/// The notice line, e.g. `R311.7.5.1 max riser 7 3/4"`.
pub fn notice_text(label: &str, limit_text: &str, kind: LimitKind) -> String {
    format!("{label}: {} {limit_text}", kind.word())
}

/// The notice color: readable on the dark and on the light dialog panel.
pub fn notice_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xFF, 0xC8, 0x57)
    } else {
        Color32::from_rgb(0x8A, 0x55, 0x00)
    }
}

/// Draws the notice line with its button when `violated`; true when "Set to
/// code" was clicked this frame. `limit_text` is the limit as it should read.
pub fn code_notice_text(ui: &mut Ui, text: &str, violated: bool) -> bool {
    if !violated {
        return false;
    }
    notice_line(ui, text, true)
}

/// A notice with no field to set (a room that is too small): the amber line
/// without a button, drawn only when `violated`.
pub fn code_note(ui: &mut Ui, text: &str, violated: bool) {
    if violated {
        notice_line(ui, text, false);
    }
}

/// The amber line itself, with a "Set to code" button or without; true when
/// the button was clicked.
fn notice_line(ui: &mut Ui, text: &str, button: bool) -> bool {
    let color = notice_color(ui);
    let mut clicked = false;
    ui.horizontal_wrapped(|ui| {
        // Line up under the control column of a labelled row.
        let k = (ui.style().text_styles[&egui::TextStyle::Body].size / 15.0).max(1.0);
        ui.add_space(super::LABEL_WIDTH * k);
        ui.label(
            RichText::new(format!("\u{26A0} {text}"))
                .color(color)
                .small(),
        );
        if button {
            clicked = ui
                .small_button("Set to code")
                .on_hover_text("Write the code limit into the field above")
                .clicked();
        }
    });
    clicked
}

/// What a span check is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    /// Floor joists (R502.3.1).
    Floor,
    /// Ceiling joists (read against the same table).
    Ceiling,
    /// Rafters (R802.4).
    Rafter,
}

/// The span check of a Build Framing size and spacing: spacing wider than the
/// span tables cover, or wider than the 16" they are written for, with the
/// table's span for the size. An amber line without a button (the spacing is
/// edited in the row above). True when a notice was shown.
pub fn span_check(ui: &mut Ui, kind: SpanKind, depth: f64, spacing: f64) -> bool {
    let m = crate::editor::code::active();
    let (rule, table) = match kind {
        SpanKind::Floor => ("IRC R502.3.1", m.joist_span(depth)),
        SpanKind::Ceiling => ("IRC R502.3.1 (ceiling joists)", m.joist_span(depth)),
        SpanKind::Rafter => ("IRC R802.4", m.rafter_span(depth)),
    };
    let span = plan_core::units::fmt_ft_in(table);
    let text = if check_limit(spacing, m.framing_spacing_max, LimitKind::Max) {
        format!(
            "{rule}: spacing max {} (the span tables stop there)",
            super::fmt_short(m.framing_spacing_max)
        )
    } else if kind != SpanKind::Ceiling
        && check_limit(spacing, m.framing_table_spacing, LimitKind::Max)
    {
        format!(
            "{rule}: the table span for this size, {span}, is for {} o.c.; wider spacing spans less",
            super::fmt_short(m.framing_table_spacing)
        )
    } else {
        return false;
    };
    notice_line(ui, &text, false);
    true
}

/// The egress notices of a Door or Window Specification: for a window in a
/// sleeping room, the net clear opening and the sill (IRC R310.2); for the
/// exterior door that is the required exit, its width and height (R311.2).
/// "Set to code" edits the draft's size or sill. True when one was clicked.
pub fn opening_notices(ui: &mut Ui, op: &mut plan_core::Opening) -> bool {
    use plan_core::OpeningKind;
    let m = crate::editor::code::active();
    match op.kind {
        OpeningKind::Window if crate::editor::code::window_in_sleeping_room(op.id) => {
            egress_window_notices(ui, op, &m)
        }
        OpeningKind::Door if crate::editor::code::door_is_required_egress(op.id) => {
            let mut hit = code_notice(
                ui,
                "IRC R311.2 egress door width (leaf)",
                &mut op.width,
                m.egress_door_width(),
                LimitKind::Min,
            );
            hit |= code_notice(
                ui,
                "IRC R311.2 egress door height",
                &mut op.height,
                m.egress_door_height,
                LimitKind::Min,
            );
            hit
        }
        _ => false,
    }
}

/// The egress notices for a window in a bedroom: an operable window with a
/// net clear opening of the minimum area, width and height, and a sill no
/// higher than the maximum.
fn egress_window_notices(
    ui: &mut Ui,
    op: &mut plan_core::Opening,
    m: &plan_check::CodeMinimums,
) -> bool {
    use plan_check::CodeMinimums;
    let mut hit = false;
    let Some((nw, nh)) = CodeMinimums::window_net_clear(op) else {
        let text = "IRC R310.2 egress: a fixed window cannot be the escape opening; choose an operable style";
        notice_line(ui, text, false);
        return false;
    };
    // A sliding window opens half its width.
    let factor = if nw < op.width - 1e-9 { 0.5 } else { 1.0 };
    let area = nw * nh / 144.0;
    let grade = crate::editor::code::active_grade_floor();
    let min_area = if grade {
        m.egress_min_area_grade
    } else {
        m.egress_min_area
    };
    if code_notice_for(
        ui,
        &format!("IRC R310.2.1 net clear opening ({area:.1} sq ft now)"),
        area,
        min_area,
        &format!("{min_area:.1} sq ft"),
        LimitKind::Min,
    ) {
        op.height = op
            .height
            .max((min_area * 144.0 / (op.width * factor)).ceil());
        hit = true;
    }
    if code_notice(
        ui,
        "IRC R310.2.1 net clear width",
        &mut op.width,
        m.egress_min_width / factor,
        LimitKind::Min,
    ) {
        hit = true;
    }
    if code_notice(
        ui,
        "IRC R310.2.1 net clear height",
        &mut op.height,
        m.egress_min_height,
        LimitKind::Min,
    ) {
        hit = true;
    }
    if code_notice(
        ui,
        "IRC R310.2.2 sill height",
        &mut op.sill_height,
        m.egress_max_sill,
        LimitKind::Max,
    ) {
        hit = true;
    }
    hit
}

/// A length field's notice: when `*value` is past `limit` (inches), shows
/// `label` with the limit and a "Set to code" button that writes the limit
/// into `*value`. True when the button changed the value.
pub fn code_notice(ui: &mut Ui, label: &str, value: &mut f64, limit: f64, kind: LimitKind) -> bool {
    let text = notice_text(label, &super::fmt_short(limit), kind);
    if code_notice_text(ui, &text, check_limit(*value, limit, kind)) {
        *value = limit;
        return true;
    }
    false
}

/// Like [`code_notice`] for a number the caller computes (`measured`) and
/// shows as `limit_text` (an area in sq ft, say). The caller applies the
/// fix when this returns true.
pub fn code_notice_for(
    ui: &mut Ui,
    label: &str,
    measured: f64,
    limit: f64,
    limit_text: &str,
    kind: LimitKind,
) -> bool {
    let text = notice_text(label, limit_text, kind);
    code_notice_text(ui, &text, check_limit(measured, limit, kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_compare_on_the_right_side() {
        assert!(check_limit(8.0, 7.75, LimitKind::Max));
        assert!(!check_limit(7.75, 7.75, LimitKind::Max));
        assert!(!check_limit(7.0, 7.75, LimitKind::Max));
        assert!(check_limit(9.0, 10.0, LimitKind::Min));
        assert!(!check_limit(10.0, 10.0, LimitKind::Min));
        assert!(!check_limit(10.0 - 1e-9, 10.0, LimitKind::Min));
    }

    #[test]
    fn the_text_names_the_citation_and_the_limit() {
        let t = notice_text(
            "R311.7.5.1 riser",
            &crate::dialogs::fmt_short(7.75),
            LimitKind::Max,
        );
        assert_eq!(t, "R311.7.5.1 riser: max 7 3/4\"");
        let t = notice_text("R311.7.5.2 tread", "10\"", LimitKind::Min);
        assert_eq!(t, "R311.7.5.2 tread: min 10\"");
    }

    /// Runs one headless frame; the first returned value is the button click,
    /// the second the texts the frame painted.
    fn frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        value: &mut f64,
    ) -> (bool, Vec<String>, Option<egui::Rect>) {
        let mut clicked = false;
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 300.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    clicked = code_notice(ui, "R311.7.5.1 riser", value, 7.75, LimitKind::Max);
                });
            },
        );
        let mut texts = Vec::new();
        let mut button = None;
        for cs in &out.shapes {
            collect(&cs.shape, &mut texts, &mut button);
        }
        (clicked, texts, button)
    }

    fn collect(shape: &egui::Shape, out: &mut Vec<String>, button: &mut Option<egui::Rect>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out, button)),
            egui::Shape::Text(t) => {
                let s = t.galley.text().to_string();
                if s == "Set to code" {
                    *button = Some(egui::Rect::from_min_size(t.pos, t.galley.size()));
                }
                out.push(s);
            }
            _ => {}
        }
    }

    #[test]
    fn no_notice_while_the_value_is_legal() {
        let ctx = egui::Context::default();
        let mut v = 7.5;
        let (clicked, texts, _) = frame(&ctx, Vec::new(), &mut v);
        assert!(!clicked);
        assert!(texts.is_empty(), "{texts:?}");
    }

    #[test]
    fn the_button_writes_the_limit_into_the_field() {
        let ctx = egui::Context::default();
        let mut v = 8.0;
        let (_, texts, button) = frame(&ctx, Vec::new(), &mut v);
        assert!(
            texts
                .iter()
                .any(|t| t.contains("R311.7.5.1 riser: max 7 3/4\"")),
            "{texts:?}"
        );
        let at = button.expect("the button is drawn").center();
        frame(&ctx, vec![egui::Event::PointerMoved(at)], &mut v);
        let press = |down| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![press(true)], &mut v);
        let (clicked, texts, _) = frame(&ctx, vec![press(false)], &mut v);
        assert!(clicked);
        assert_eq!(v, 7.75);
        let (_, texts2, _) = frame(&ctx, Vec::new(), &mut v);
        assert!(texts2.is_empty(), "{texts} {texts2:?}", texts = texts.len());
    }
}
