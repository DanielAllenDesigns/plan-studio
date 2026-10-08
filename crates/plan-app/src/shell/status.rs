//! The status bar: Chief's strip along the bottom of the window.
//!
//! Left to right: the cursor's X / Y, a pending hotkey prefix, the tool's live
//! readout and snap, the tool hint and the latest status message. At the right
//! (read right to left): "Saved n min ago", the Undo label, the zoom, the
//! layer set, the floor and the Appearance menu (UI scale, larger text, reduce
//! motion). Clicking the message opens a log of the last 50 messages.

use crate::editor::EditorContext;
use crate::theme::{self, AppSettings};
use eframe::egui::{self, Align, Layout, RichText, Sense};
use std::cell::RefCell;
use std::collections::VecDeque;

/// How many status messages the log keeps.
pub const LOG_CAP: usize = 50;

thread_local! {
    static LOG: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
}

/// Adds `message` to the log (empty messages and immediate repeats are
/// ignored). `PlanApp` calls this for every change of `EditorContext::status`.
pub fn record(message: &str) {
    let message = message.trim();
    if message.is_empty() {
        return;
    }
    LOG.with(|l| {
        let mut l = l.borrow_mut();
        if l.back().map(String::as_str) == Some(message) {
            return;
        }
        l.push_back(message.to_string());
        while l.len() > LOG_CAP {
            l.pop_front();
        }
    });
}

/// The logged messages, newest first.
pub fn recent() -> Vec<String> {
    LOG.with(|l| l.borrow().iter().rev().cloned().collect())
}

/// Empties the log.
pub fn clear_log() {
    LOG.with(|l| l.borrow_mut().clear());
}

/// Zoom as a percentage of life size at 96 points per inch.
pub fn zoom_percent(px_per_in: f64) -> f64 {
    px_per_in / 96.0 * 100.0
}

/// "Zoom 2%" (one decimal below 10 percent).
pub fn zoom_label(px_per_in: f64) -> String {
    let p = zoom_percent(px_per_in);
    if p < 10.0 {
        format!("Zoom {p:.1}%")
    } else {
        format!("Zoom {p:.0}%")
    }
}

/// Everything the status bar shows, gathered by the app each frame.
#[derive(Default, Clone, Debug)]
pub struct StatusFields {
    /// Formatted cursor X and Y; `None` while the pointer is off the canvas.
    pub cursor: Option<(String, String)>,
    /// The elevation the cursor sits at (the active floor's finished-floor
    /// level), shown after X and Y (S-98).
    pub z: Option<String>,
    /// "1 Straight Wall selected, Z 0\" to 9'-0\"" (S-98).
    pub selection: Option<String>,
    /// What the pointer is over: "Hinged Door 3068" (S-6, DW-65).
    pub hover: Option<String>,
    /// "Edit Behavior: Resize" while a behavior other than Default is on (S-66).
    pub behavior: Option<String>,
    /// "Code: IRC 2021" while the Plan Check settings are open and
    /// "Live check: 2 errors" while the live check has findings.
    pub code: Option<String>,
    /// A hotkey sequence in progress ("D, ...").
    pub pending_prefix: Option<String>,
    /// The tool's live readout (a length, an angle).
    pub readout: Option<String>,
    /// The object snap in effect.
    pub snap: Option<String>,
    /// The tool's one-line hint.
    pub hint: String,
    /// The latest status message.
    pub message: String,
    pub floor: String,
    pub layer_set: String,
    pub zoom: String,
    /// The step Undo would revert.
    pub undo: Option<String>,
    /// "Saved 2 min ago".
    pub saved: String,
}

impl StatusFields {
    /// The texts of the bar in reading order, as drawn.
    #[cfg(test)]
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(match &self.cursor {
            Some((x, y)) => format!("X: {x}  Y: {y}"),
            None => "X: --  Y: --".to_string(),
        });
        out.extend(self.z.as_ref().map(|z| format!("Z: {z}")));
        out.extend(self.pending_prefix.clone());
        out.extend(self.readout.clone());
        out.extend(self.snap.as_ref().map(|s| format!("Snap: {s}")));
        out.extend(self.behavior.clone());
        out.extend(self.code.clone());
        out.extend(self.selection.clone());
        out.extend(self.hover.clone());
        if !self.hint.is_empty() && self.selection.is_none() && self.hover.is_none() {
            out.push(self.hint.clone());
        }
        if !self.message.is_empty() {
            out.push(self.message.clone());
        }
        out.push(format!("Floor: {}", self.floor));
        out.push(format!("Layers: {}", self.layer_set));
        out.push(self.zoom.clone());
        if let Some(u) = &self.undo {
            out.push(format!("Undo: {u}"));
        }
        if !self.saved.is_empty() {
            out.push(self.saved.clone());
        }
        out
    }
}

/// The facts about the selection and the pointer that depend on the editor
/// (everything else comes from the shell): Z, the selection text, the hover
/// text and the Edit Behavior indicator. `select_active` is true while Select
/// Objects is the tool: the hover text and the indicator belong to it.
pub fn context_fields(cx: &EditorContext, select_active: bool) -> StatusFields {
    use crate::tools::select::describe;
    let hovering = select_active && cx.cursor_world.is_some();
    StatusFields {
        z: cx
            .cursor_world
            .is_some()
            .then(|| cx.fmt_dim(cx.floor().elevation)),
        selection: describe::selection_summary(cx),
        hover: if hovering {
            describe::hover_text(cx)
        } else {
            None
        },
        behavior: crate::editor::behaviors::indicator(cx),
        code: crate::editor::code::status_text(cx),
        ..StatusFields::default()
    }
}

/// The tooltip that hangs at the pointer after a short rest over an object
/// (S-6, DW-65); `None` while no object is under it.
pub fn hover_tooltip(cx: &EditorContext) -> Option<String> {
    crate::tools::select::describe::hover_text(cx)
}

fn log_id() -> egui::Id {
    egui::Id::new("status_message_log")
}

fn menu_id() -> egui::Id {
    egui::Id::new("status_appearance_menu")
}

/// Draws the bar's contents into `ui` (the caller supplies the panel).
pub fn show(ui: &mut egui::Ui, f: &StatusFields, settings: &mut AppSettings) {
    ui.horizontal(|ui| {
        let right = |ui: &mut egui::Ui, settings: &mut AppSettings| {
            // Right to left: the Appearance menu, then the facts.
            let aa = ui
                .add(egui::Button::new("Aa").small())
                .on_hover_text("Appearance: UI scale, larger text, reduce motion");
            if aa.clicked() {
                ui.memory_mut(|m| m.toggle_popup(menu_id()));
            }
            egui::popup_above_or_below_widget(
                ui,
                menu_id(),
                &aa,
                egui::AboveOrBelow::Above,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(260.0);
                    theme::appearance_controls(ui, settings);
                },
            );
            if !f.saved.is_empty() {
                ui.separator();
                ui.weak(&f.saved);
            }
            if let Some(u) = &f.undo {
                ui.separator();
                ui.label(format!("Undo: {u}"));
            }
            ui.separator();
            ui.label(&f.zoom);
            ui.separator();
            ui.label(format!("Layers: {}", f.layer_set));
            ui.separator();
            ui.label(format!("Floor: {}", f.floor));
        };
        // The right group is laid out first (right to left) in the space that
        // remains after the left group would take its share.
        let mut left_ui_width = ui.available_width();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            right(ui, settings);
            left_ui_width = ui.available_width();
        });
        // Left group: drawn over the same row, clipped to what is left of it.
        let row = ui.min_rect();
        let mut lui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    row.left_top(),
                    egui::vec2(left_ui_width.max(40.0), row.height().max(20.0)),
                ))
                .layout(Layout::left_to_right(Align::Center)),
        );
        lui.set_clip_rect(lui.max_rect());
        left(&mut lui, f);
    });
}

fn left(ui: &mut egui::Ui, f: &StatusFields) {
    match &f.cursor {
        Some((x, y)) => ui.monospace(format!("X: {x}  Y: {y}")),
        None => ui.monospace("X: --  Y: --"),
    };
    if let Some(z) = &f.z {
        ui.monospace(format!("Z: {z}"));
    }
    if let Some(p) = &f.pending_prefix {
        ui.separator();
        ui.strong(p);
    }
    if let Some(r) = &f.readout {
        ui.separator();
        ui.label(r);
    }
    if let Some(s) = &f.snap {
        ui.separator();
        ui.label(format!("Snap: {s}"));
    }
    if let Some(b) = &f.behavior {
        ui.separator();
        ui.strong(b);
    }
    if let Some(c) = &f.code {
        ui.separator();
        ui.label(c);
    }
    if let Some(sel) = &f.selection {
        ui.separator();
        ui.label(sel);
    }
    if let Some(h) = &f.hover {
        ui.separator();
        ui.label(egui::RichText::new(h).italics());
    }
    if !f.hint.is_empty() && f.selection.is_none() && f.hover.is_none() {
        ui.separator();
        ui.label(&f.hint);
    }
    if !f.message.is_empty() {
        ui.separator();
        let msg = ui
            .add(
                egui::Label::new(RichText::new(&f.message).weak())
                    .sense(Sense::click())
                    .truncate(),
            )
            .on_hover_text("Click for the last 50 messages")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if msg.clicked() {
            ui.memory_mut(|m| m.toggle_popup(log_id()));
        }
        egui::popup_above_or_below_widget(
            ui,
            log_id(),
            &msg,
            egui::AboveOrBelow::Above,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            log_body,
        );
    } else {
        // Keep the log reachable when nothing is showing.
        let log = ui.add(egui::Label::new(RichText::new("Messages").weak()).sense(Sense::click()));
        if log.clicked() {
            ui.memory_mut(|m| m.toggle_popup(log_id()));
        }
        egui::popup_above_or_below_widget(
            ui,
            log_id(),
            &log,
            egui::AboveOrBelow::Above,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            log_body,
        );
    }
}

/// The popup: the last [`LOG_CAP`] messages, newest first.
fn log_body(ui: &mut egui::Ui) {
    ui.set_min_width(360.0);
    ui.horizontal(|ui| {
        ui.strong("Messages");
        if ui.button("Clear").clicked() {
            clear_log();
        }
    });
    ui.separator();
    let msgs = recent();
    if msgs.is_empty() {
        ui.weak("No messages yet.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("status_log_scroll")
        .max_height(280.0)
        .show(ui, |ui| {
            for m in msgs {
                ui.label(m);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> StatusFields {
        StatusFields {
            cursor: Some(("12' 3\"".into(), "4' 0\"".into())),
            pending_prefix: Some("D, ...".into()),
            readout: Some("Length 10' 0\"".into()),
            snap: Some("Endpoint".into()),
            hint: "Click the first wall point".into(),
            message: "Wall added".into(),
            floor: "First Floor".into(),
            layer_set: "Plan View".into(),
            zoom: zoom_label(2.0),
            undo: Some("Move Wall".into()),
            saved: "Saved 2 min ago".into(),
            ..StatusFields::default()
        }
    }

    /// All text painted by one frame of the bar.
    fn painted(ctx: &egui::Context, f: &StatusFields, events: Vec<egui::Event>) -> String {
        let mut settings = AppSettings::default();
        let raw = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1700.0, 500.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| show(ui, f, &mut settings));
        });
        let mut text = String::new();
        for s in &out.shapes {
            collect(&s.shape, &mut text);
        }
        text
    }

    fn collect(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }

    #[test]
    fn zoom_reads_as_percent_of_life_size() {
        assert_eq!(zoom_label(96.0), "Zoom 100%");
        assert_eq!(zoom_label(2.0), "Zoom 2.1%");
        assert_eq!(zoom_label(24.0), "Zoom 25%");
    }

    #[test]
    fn the_log_keeps_the_last_50_newest_first_and_skips_repeats() {
        clear_log();
        record("");
        for i in 0..60 {
            record(&format!("message {i}"));
            record(&format!("message {i}"));
        }
        let r = recent();
        assert_eq!(r.len(), LOG_CAP);
        assert_eq!(r[0], "message 59");
        assert_eq!(r[LOG_CAP - 1], "message 10");
        clear_log();
        assert!(recent().is_empty());
    }

    #[test]
    fn texts_list_every_field_in_order() {
        let t = fields().texts();
        assert_eq!(t[0], "X: 12' 3\"  Y: 4' 0\"");
        assert_eq!(t[1], "D, ...");
        assert!(t.contains(&"Snap: Endpoint".to_string()));
        assert!(t.contains(&"Click the first wall point".to_string()));
        assert!(t.contains(&"Floor: First Floor".to_string()));
        assert!(t.contains(&"Layers: Plan View".to_string()));
        assert!(t.contains(&"Zoom 2.1%".to_string()));
        assert!(t.contains(&"Undo: Move Wall".to_string()));
        assert_eq!(t.last().unwrap(), "Saved 2 min ago");
        let off = StatusFields::default().texts();
        assert_eq!(off[0], "X: --  Y: --");
    }

    #[test]
    fn selection_hover_z_and_behavior_have_a_place_in_the_bar() {
        let mut f = fields();
        f.z = Some("0\"".into());
        f.selection = Some("1 Straight Wall selected, Z 0\" to 9'-0\"".into());
        f.hover = Some("Hinged Door 3068".into());
        f.behavior = Some("Edit Behavior: Resize".into());
        let t = f.texts();
        assert!(t.contains(&"Z: 0\"".to_string()));
        assert!(t.contains(&"Edit Behavior: Resize".to_string()));
        assert!(t.contains(&"1 Straight Wall selected, Z 0\" to 9'-0\"".to_string()));
        assert!(t.contains(&"Hinged Door 3068".to_string()));
        // The tool's hint steps aside for them.
        assert!(!t.contains(&"Click the first wall point".to_string()));
        let ctx = egui::Context::default();
        crate::theme::apply_settings(&ctx, &AppSettings::default());
        let _ = painted(&ctx, &f, vec![]);
        let text = painted(&ctx, &f, vec![]);
        for want in ["Z: 0\"", "Edit Behavior: Resize", "Hinged Door 3068"] {
            assert!(text.contains(want), "missing {want:?}:\n{text}");
        }
    }

    #[test]
    fn context_fields_read_the_editor() {
        use crate::editor::{EditorContext, ObjectRef};
        use plan_core::geometry::Point;
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            plan_core::WallKind::Interior,
        );
        cx.selection.set(ObjectRef::Wall(id));
        cx.hover = Some(ObjectRef::Wall(id));
        cx.cursor_world = Some(Point::new(10.0, 1.0));
        let f = context_fields(&cx, true);
        assert!(f.z.is_some());
        assert!(f.selection.unwrap().starts_with("1 Straight Wall selected"));
        assert!(f.hover.unwrap().starts_with("Interior Wall"));
        assert_eq!(f.behavior, None);
        // Another tool shows no hover text.
        assert!(context_fields(&cx, false).hover.is_none());
        assert!(hover_tooltip(&cx).is_some());
    }

    #[test]
    fn a_frame_paints_every_field() {
        let ctx = egui::Context::default();
        crate::theme::apply_settings(&ctx, &AppSettings::default());
        let f = fields();
        let _ = painted(&ctx, &f, vec![]);
        let text = painted(&ctx, &f, vec![]);
        for want in [
            "X: 12' 3\"  Y: 4' 0\"",
            "D, ...",
            "Length 10' 0\"",
            "Snap: Endpoint",
            "Click the first wall point",
            "Wall added",
            "Floor: First Floor",
            "Layers: Plan View",
            "Zoom 2.1%",
            "Undo: Move Wall",
            "Saved 2 min ago",
            "Aa",
        ] {
            assert!(
                text.contains(want),
                "status bar is missing {want:?}:\n{text}"
            );
        }
    }

    #[test]
    fn the_message_log_popup_lists_recent_messages() {
        clear_log();
        record("first message");
        record("second message");
        let ctx = egui::Context::default();
        crate::theme::apply_settings(&ctx, &AppSettings::default());
        let f = fields();
        let _ = painted(&ctx, &f, vec![]);
        assert!(!painted(&ctx, &f, vec![]).contains("second message"));
        ctx.memory_mut(|m| m.open_popup(log_id()));
        let _ = painted(&ctx, &f, vec![]);
        let text = painted(&ctx, &f, vec![]);
        assert!(text.contains("first message"), "{text}");
        assert!(text.contains("second message"), "{text}");
        assert!(text.contains("Clear"));
        clear_log();
    }
}
