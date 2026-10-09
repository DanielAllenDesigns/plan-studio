//! Construction Line Specification and Defaults (CAD-66, CAD-67; manual pp.
//! 85-88) and the construction line commands of the Edit toolbar.
//!
//! Four panels: Construction Line (infinite in plan and elevation, display
//! on all floors, include in automatic ordering, Define Rules), Callouts
//! (display on either or both ends per view, label and text below the line
//! with Automatic and Insert, shape, fill, size and angle, custom outline),
//! Line Style and Text Style. The dialog edits a draft of the line's
//! [`ConstructionLine`] record; OK writes it as one undo step. The same
//! dialog is the Construction Line Defaults (CAD > Line > Construction Line
//! Defaults) and Set as Default fills it from a line.
//!
//! The dialog is hosted here: [`host_frame`] shows it every frame
//! (`ToolSet::frame` calls it), so it opens from the Edit toolbar button, a
//! menu command or [`open_spec`] whichever tool is active.

use super::text::annot::{color_row, insert_menu, transparency_row, Macros};
use super::{on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use super::{PV_ACCENT, PV_FAINT, PV_INK};
use crate::editor::{EditAction, EditActionKind, EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::construction::{
    CalloutEnds, CalloutShape, CalloutSpec, ConstructionLine, ViewType, LAYER,
};
use plan_core::layers::LineStyle;
use plan_core::Id;
use std::cell::RefCell;

/// Custom command: the Construction Line Specification of the selected line.
pub const SPEC: &str = "construction.spec";
/// Custom command: Construction Line Defaults.
pub const DEFAULTS: &str = "construction.defaults";
/// Custom command: Set as Default (the selected line becomes the default).
pub const SET_AS_DEFAULT: &str = "construction.set_default";
/// Custom command: Convert to Polyline (selected construction lines).
pub const TO_POLYLINE: &str = "construction.to_polyline";
/// Custom command: Convert Polyline to Construction Line.
pub const FROM_POLYLINE: &str = "construction.from_polyline";
/// Custom command: Construction Line Order Management.
pub const ORDER: &str = "construction.order";

const TABS: &[Tab] = &[
    on("Construction Line"),
    on("Callouts"),
    on("Line Style"),
    on("Text Style"),
];

/// What the dialog edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The line `id` of floor `floor`.
    Line { floor: usize, id: Id },
    /// The plan's Construction Line Defaults.
    Defaults,
}

pub struct ConstructionLineDialog {
    frame: SpecDialog,
    form: Form,
    target: Target,
    /// Define Rules was pressed: open Order Management after OK or Cancel.
    define_rules: bool,
}

struct Form {
    draft: ConstructionLine,
    text_styles: Vec<String>,
    /// Weight in millimetres while it is a custom one.
    fields: Fields,
    layer: String,
}

// The accessors are the dialog's model API (the tests drive it); the UI
// edits the same fields directly.
#[allow(dead_code)]
impl ConstructionLineDialog {
    pub fn new(
        target: Target,
        draft: ConstructionLine,
        layer: String,
        text_styles: Vec<String>,
    ) -> Self {
        let title = match target {
            Target::Line { .. } => "Construction Line Specification",
            Target::Defaults => "Construction Line Defaults",
        };
        Self {
            frame: SpecDialog::new(title, "construction_line"),
            form: Form {
                draft,
                text_styles,
                fields: Fields::default(),
                layer,
            },
            target,
            define_rules: false,
        }
    }

    /// The dialog of the construction line `id` on the active floor.
    pub fn for_line(cx: &EditorContext, id: Id) -> Option<Self> {
        let rec = cx.floor().construction_line(id)?.clone();
        let layer = cx
            .floor()
            .cad
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.layer.clone())
            .unwrap_or_else(|| LAYER.to_string());
        Some(Self::new(
            Target::Line {
                floor: cx.floor,
                id,
            },
            rec,
            layer,
            text_style_names(cx),
        ))
    }

    /// The dialog of the plan's Construction Line Defaults.
    pub fn for_defaults(cx: &EditorContext) -> Self {
        Self::new(
            Target::Defaults,
            cx.project.construction.defaults.clone(),
            LAYER.to_string(),
            text_style_names(cx),
        )
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn draft(&self) -> &ConstructionLine {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut ConstructionLine {
        &mut self.form.draft
    }

    pub fn wants_rules(&self) -> bool {
        self.define_rules
    }

    pub fn request_rules(&mut self) {
        self.define_rules = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }
}

fn text_style_names(cx: &EditorContext) -> Vec<String> {
    cx.project
        .text_styles
        .names()
        .into_iter()
        .map(String::from)
        .collect()
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("A number is not valid".into());
        }
        let c = &self.draft.callouts;
        if !c.auto_size && c.size <= 0.0 {
            return Some("The callout size must be larger than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "Construction Line" => self.line_panel(ui),
            "Callouts" => self.callouts_panel(ui),
            "Line Style" => self.line_style_panel(ui),
            "Text Style" => self.text_style_panel(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            rect.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        let area = Rect::from_min_max(
            rect.min + egui::vec2(0.0, 18.0),
            rect.max - egui::vec2(0.0, 8.0),
        );
        let c = area.center();
        let half = area.width() / 2.0 - 4.0;
        let (a, b) = (Pos2::new(c.x - half, c.y), Pos2::new(c.x + half, c.y));
        let stroke = Stroke::new(1.4_f32, PV_INK);
        let style = self.draft.line_style.unwrap_or(LineStyle::Dashed);
        match style {
            LineStyle::Solid => {
                p.line_segment([a, b], stroke);
            }
            _ => {
                p.extend(Shape::dashed_line(&[a, b], stroke, 8.0, 5.0));
            }
        }
        let call = &self.draft.callouts;
        let (top, below) = call.texts(self.draft.in_ordering.then_some("A"));
        let put = |at: Pos2| {
            let r = 13.0;
            let pts: Vec<Pos2> = crate::editor::ref_overlay::callout_outline(
                call.shape,
                r * 2.0,
                r * 2.0,
                call.angle_deg.to_radians() as f32,
            )
            .into_iter()
            .map(|q| at + q.to_vec2())
            .collect();
            let fill = if call.filled {
                Color32::from_rgb(0xF0, 0xE0, 0xC8)
            } else {
                Color32::TRANSPARENT
            };
            p.add(Shape::convex_polygon(
                pts,
                fill,
                Stroke::new(1.2_f32, PV_ACCENT),
            ));
            pv_text(p, at, Align2::CENTER_CENTER, top.clone(), 11.0);
            if !below.is_empty() {
                pv_text(
                    p,
                    at + egui::vec2(0.0, 7.0),
                    Align2::CENTER_CENTER,
                    below.clone(),
                    8.0,
                );
            }
        };
        if call.plan.at_start() {
            put(Pos2::new(a.x + 16.0, c.y));
        }
        if call.plan.at_end() {
            put(Pos2::new(b.x - 16.0, c.y));
        }
        let note = if self.draft.infinite_plan {
            "Infinite in plan"
        } else {
            "Finite in plan"
        };
        pv_text(
            p,
            Pos2::new(c.x, area.max.y - 6.0),
            Align2::CENTER_CENTER,
            note,
            10.0,
        );
        let _ = PV_FAINT;
    }
}

impl Form {
    fn line_panel(&mut self, ui: &mut Ui) {
        section(ui, "Infinite Line");
        ui.checkbox(
            &mut self.draft.infinite_plan,
            "Draw Infinite Line in Plan View",
        );
        ui.checkbox(
            &mut self.draft.infinite_elevation,
            "Draw Infinite Line in Elevation View",
        );
        ui.weak(
            "Infinite construction lines do not affect the extents of any view they display in.",
        );
        section(ui, "Options");
        ui.checkbox(
            &mut self.draft.all_floors,
            "Display on All Floors in Plan View",
        );
        ui.checkbox(&mut self.draft.in_ordering, "Include in Automatic Ordering");
        ui.horizontal(|ui| {
            row(ui, "", |ui| {
                if ui.button("Define Rules\u{2026}").clicked() {
                    // Handled by the dialog's owner (see `wants_rules`).
                    RULES_REQUEST.with(|r| r.set(true));
                }
            });
        });
        ui.weak(format!("Layer: {}", self.layer));
    }

    fn callouts_panel(&mut self, ui: &mut Ui) {
        let c = &mut self.draft.callouts;
        section(ui, "Display Callout on");
        for (label, view) in [
            ("Plan View", ViewType::Plan),
            ("Elevation View", ViewType::Elevation),
        ] {
            row(ui, label, |ui| {
                let slot = match view {
                    ViewType::Plan => &mut c.plan,
                    ViewType::Elevation => &mut c.elevation,
                };
                for e in CalloutEnds::ALL {
                    ui.radio_value(slot, e, e.label());
                }
            });
        }
        section(ui, "Label");
        text_row(
            ui,
            "Label",
            "construction_label",
            &mut c.label,
            &mut c.label_auto,
        );
        text_row(
            ui,
            "Text Below Line",
            "construction_below",
            &mut c.text_below,
            &mut c.below_auto,
        );
        section(ui, "Shape");
        ui.horizontal_wrapped(|ui| {
            for s in CalloutShape::ALL {
                ui.radio_value(&mut c.shape, s, s.label());
            }
        });
        section(ui, "Fill Color");
        ui.checkbox(&mut c.filled, "Filled");
        ui.add_enabled_ui(c.filled, |ui| {
            row(ui, "Color", |ui| {
                ui.checkbox(&mut c.fill_by_layer, "By Layer");
                if !c.fill_by_layer {
                    ui.color_edit_button_srgb(&mut c.fill_color);
                }
            });
            transparency_row(ui, &mut c.transparency);
        });
        section(ui, "Size/Orientation");
        ui.checkbox(&mut c.auto_size, "Automatic");
        if !c.auto_size {
            self.fields.length_row(ui, "Size", "size", &mut c.size);
        }
        ui.add_enabled_ui(c.shape.has_angle(), |ui| {
            self.fields
                .degrees_row(ui, "Angle", "deg_angle", &mut c.angle_deg);
        });
        section(ui, "Custom Callout Line Options");
        ui.checkbox(&mut c.custom_outline, "Custom Callout Line Options");
        ui.add_enabled_ui(c.custom_outline, |ui| {
            row(ui, "Color", |ui| {
                ui.checkbox(&mut c.outline_color_by_layer, "By Layer");
                if !c.outline_color_by_layer {
                    ui.color_edit_button_srgb(&mut c.outline_color);
                }
            });
            row(ui, "Line Style", |ui| {
                ui.checkbox(&mut c.outline_style_by_layer, "By Layer");
                if !c.outline_style_by_layer {
                    style_combo(ui, "co_style", &mut c.outline_style);
                }
            });
            row(ui, "Line Weight", |ui| {
                ui.checkbox(&mut c.outline_weight_by_layer, "By Layer");
                if !c.outline_weight_by_layer {
                    weight_drag(ui, &mut c.outline_weight);
                }
            });
        });
    }

    fn line_style_panel(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        color_row(ui, "Color", &mut self.draft.color);
        row(ui, "Line Style", |ui| {
            let mut by_layer = self.draft.line_style.is_none();
            if ui.checkbox(&mut by_layer, "By Layer").changed() {
                self.draft.line_style = if by_layer {
                    None
                } else {
                    Some(LineStyle::Solid)
                };
            }
            if let Some(s) = &mut self.draft.line_style {
                style_combo(ui, "cl_style", s);
            }
        });
        row(ui, "Line Weight", |ui| {
            let mut by_layer = self.draft.line_weight.is_none();
            if ui.checkbox(&mut by_layer, "By Layer").changed() {
                self.draft.line_weight = if by_layer { None } else { Some(25) };
            }
            if let Some(w) = &mut self.draft.line_weight {
                weight_drag(ui, w);
            }
        });
    }

    fn text_style_panel(&mut self, ui: &mut Ui) {
        section(ui, "Text Style");
        row(ui, "Text Style", |ui| {
            let shown = if self.draft.text_style.is_empty() {
                "By Layer".to_string()
            } else {
                self.draft.text_style.clone()
            };
            egui::ComboBox::from_id_salt("cl_text_style")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.draft.text_style, String::new(), "By Layer");
                    for n in &self.text_styles {
                        ui.selectable_value(&mut self.draft.text_style, n.clone(), n);
                    }
                });
        });
        ui.weak("The Uppercase style is not applied to callout text under Automatic Ordering.");
    }
}

thread_local! {
    static RULES_REQUEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static HOST: RefCell<Option<ConstructionLineDialog>> = const { RefCell::new(None) };
}

fn text_row(ui: &mut Ui, label: &str, id: &str, text: &mut String, auto: &mut bool) {
    row(ui, label, |ui| {
        ui.add_enabled(
            !*auto,
            egui::TextEdit::singleline(text).desired_width(150.0),
        );
        ui.add_enabled_ui(!*auto, |ui| {
            insert_menu(ui, id, text, Macros::Callout);
        });
        ui.checkbox(auto, "Automatic");
    });
}

fn style_combo(ui: &mut Ui, salt: &str, style: &mut LineStyle) {
    let name = |s: LineStyle| match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash Dot",
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(name(*style))
        .show_ui(ui, |ui| {
            for s in [
                LineStyle::Solid,
                LineStyle::Dashed,
                LineStyle::Dotted,
                LineStyle::DashDot,
            ] {
                ui.selectable_value(style, s, name(s));
            }
        });
}

/// A line weight in millimetres (stored in hundredths).
fn weight_drag(ui: &mut Ui, weight: &mut u32) {
    let mut mm = f64::from(*weight) / 100.0;
    if ui
        .add(
            egui::DragValue::new(&mut mm)
                .range(0.05..=5.0)
                .speed(0.01)
                .suffix(" mm"),
        )
        .changed()
    {
        *weight = (mm * 100.0).round() as u32;
    }
}

// ---------------------------------------------------------------------------
// Applying and hosting
// ---------------------------------------------------------------------------

/// Writes an accepted draft: the line's record (one undo step) or the
/// plan's defaults.
pub fn apply(cx: &mut EditorContext, target: Target, draft: &ConstructionLine) -> bool {
    match target {
        Target::Line { floor, id } => {
            let Some(f) = cx.project.floors.get(floor) else {
                return false;
            };
            if !f.is_construction_line(id) {
                return false;
            }
            cx.begin_change("Construction Line Specification");
            let mut rec = draft.clone();
            rec.id = id;
            cx.project.floors[floor].construction.set(rec);
            cx.mark_dirty();
            true
        }
        Target::Defaults => {
            cx.begin_change("Construction Line Defaults");
            let mut rec = draft.clone();
            rec.id = 0;
            cx.project.construction.defaults = rec;
            cx.mark_dirty();
            cx.status = "Updated the Construction Line Defaults".into();
            true
        }
    }
}

/// Opens the specification of the construction line `id` of the active
/// floor. False when `id` is not a construction line. (The Open Object
/// route for a selected line, see docs/integration-queue.md.)
pub fn open_spec(cx: &mut EditorContext, id: Id) -> bool {
    let Some(d) = ConstructionLineDialog::for_line(cx, id) else {
        return false;
    };
    HOST.with(|h| *h.borrow_mut() = Some(d));
    true
}

/// Opens the Construction Line Defaults.
pub fn open_defaults(cx: &EditorContext) {
    let d = ConstructionLineDialog::for_defaults(cx);
    HOST.with(|h| *h.borrow_mut() = Some(d));
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open dialog.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut ConstructionLineDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    apply(cx, d.target, &d.form.draft);
    true
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    let outcome = d.show(ctx);
    if RULES_REQUEST.with(|r| r.replace(false)) {
        d.define_rules = true;
    }
    match outcome {
        Outcome::Open => {
            if d.define_rules {
                d.define_rules = false;
                super::construction_order::open(cx);
            }
            HOST.with(|h| *h.borrow_mut() = Some(d));
        }
        Outcome::Cancel => {}
        Outcome::Ok => {
            apply(cx, d.target, &d.form.draft);
        }
    }
}

// ---------------------------------------------------------------------------
// Edit toolbar commands
// ---------------------------------------------------------------------------

/// The ids of the construction lines in the selection.
fn selected_lines(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) if cx.floor().is_construction_line(*id) => Some(*id),
            _ => None,
        })
        .collect()
}

/// The ids of the CAD polylines in the selection.
fn selected_polylines(cx: &EditorContext) -> Vec<Id> {
    cx.selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id)
                if cx.floor().cad.iter().any(|c| {
                    c.id == *id && matches!(c.item, plan_core::CadItem::Polyline { .. })
                }) =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect()
}

/// The Edit toolbar buttons of the selection: Open Object for a
/// construction line, Set as Default, Convert to Polyline; Convert to
/// Construction Line for a polyline (CAD-67).
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    let mut add = |id: &'static str, label: &'static str| {
        v.push(EditAction {
            kind: EditActionKind::Custom {
                id,
                label,
                icon: "",
            },
            label,
            icon: None,
            enabled: true,
        });
    };
    let lines = selected_lines(cx);
    if lines.len() == 1 && cx.selection.items.len() == 1 {
        add(SPEC, "Construction Line Specification");
        add(SET_AS_DEFAULT, "Set as Default");
    }
    if !lines.is_empty() {
        add(TO_POLYLINE, "Convert to Polyline");
    }
    if !selected_polylines(cx).is_empty() {
        add(FROM_POLYLINE, "Convert Polyline to Construction Line");
    }
    v
}

/// Runs a construction line command; true when `id` was one.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        SPEC => {
            let lines = selected_lines(cx);
            match lines.as_slice() {
                [one] => {
                    open_spec(cx, *one);
                }
                _ => cx.status = "Select one construction line".into(),
            }
        }
        DEFAULTS => open_defaults(cx),
        ORDER => super::construction_order::open(cx),
        SET_AS_DEFAULT => {
            let lines = selected_lines(cx);
            let Some(rec) = lines
                .first()
                .and_then(|i| cx.floor().construction_line(*i))
                .cloned()
            else {
                cx.status = "Select a construction line to set as the default".into();
                return true;
            };
            cx.begin_change("Set as Default");
            cx.project.construction.defaults = rec.for_line(0);
            cx.mark_dirty();
            cx.status = "New construction lines will look like this one".into();
        }
        TO_POLYLINE => {
            let lines = selected_lines(cx);
            if lines.is_empty() {
                return true;
            }
            cx.begin_change("Convert to Polyline");
            let fl = cx.floor;
            let n = lines
                .iter()
                .filter(|i| cx.project.construction_to_polyline(fl, **i))
                .count();
            cx.mark_dirty();
            cx.status = format!("Converted {n} construction line(s) to polylines");
        }
        FROM_POLYLINE => {
            let polys = selected_polylines(cx);
            if polys.is_empty() {
                return true;
            }
            cx.begin_change("Convert Polyline to Construction Line");
            let fl = cx.floor;
            let mut made = Vec::new();
            for id in polys {
                made.extend(cx.project.polyline_to_construction(fl, id));
            }
            cx.selection.items.clear();
            for id in &made {
                cx.selection.add(ObjectRef::Cad(*id));
            }
            cx.mark_dirty();
            cx.status = format!("Made {} construction line(s)", made.len());
        }
        _ => return false,
    }
    true
}

/// A copy of a callout spec with the given ends (tests and menus).
#[cfg_attr(not(test), allow(dead_code))]
pub fn with_ends(mut spec: CalloutSpec, plan: CalloutEnds) -> CalloutSpec {
    spec.plan = plan;
    spec
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn cx() -> EditorContext {
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let _ = &mut cx;
        cx
    }

    fn line(cx: &mut EditorContext) -> Id {
        let id = cx
            .project
            .add_construction_line(0, Point::new(0.0, 0.0), Point::new(100.0, 0.0), None)
            .unwrap();
        cx.selection.set(ObjectRef::Cad(id));
        id
    }

    #[test]
    fn the_specification_edits_the_record_in_one_undo_step() {
        let mut cx = cx();
        let id = line(&mut cx);
        assert!(open_spec(&mut cx, id));
        with_dialog(|d| {
            assert_eq!(d.target(), Target::Line { floor: 0, id });
            let c = d.draft_mut();
            c.all_floors = true;
            c.infinite_plan = false;
            c.callouts.plan = CalloutEnds::Both;
            c.callouts.label_auto = false;
            c.callouts.label = "N".into();
            c.callouts.shape = CalloutShape::Hexagon;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        let rec = cx.floor().construction_line(id).unwrap();
        assert!(rec.all_floors && !rec.infinite_plan);
        assert_eq!(rec.callouts.plan, CalloutEnds::Both);
        assert_eq!(rec.callouts.shape, CalloutShape::Hexagon);
        assert_eq!(rec.id, id);
        assert_eq!(cx.undo_label(), Some("Construction Line Specification"));
        cx.undo();
        assert!(!cx.floor().construction_line(id).unwrap().all_floors);
    }

    #[test]
    fn a_plain_cad_line_has_no_specification() {
        let mut cx = cx();
        let id = cx.project.alloc_id();
        cx.project.floors[0].cad.push(plan_core::CadObject {
            id,
            layer: plan_core::cad::DEFAULT_CAD_LAYER.into(),
            item: plan_core::CadItem::Line {
                a: Point::ZERO,
                b: Point::new(5.0, 0.0),
            },
        });
        assert!(!open_spec(&mut cx, id));
        assert!(!dialog_open());
    }

    #[test]
    fn set_as_default_and_the_defaults_dialog_feed_new_lines() {
        let mut cx = cx();
        let id = line(&mut cx);
        cx.project.floors[0]
            .construction
            .get_mut(id)
            .unwrap()
            .in_ordering = false;
        cx.project.floors[0]
            .construction
            .get_mut(id)
            .unwrap()
            .callouts
            .plan = CalloutEnds::End;
        assert!(run_command(&mut cx, SET_AS_DEFAULT));
        assert_eq!(cx.undo_label(), Some("Set as Default"));
        let next = cx
            .project
            .add_construction_line(0, Point::new(0.0, 9.0), Point::new(9.0, 9.0), None)
            .unwrap();
        let rec = cx.floor().construction_line(next).unwrap();
        assert_eq!(rec.id, next);
        assert!(!rec.in_ordering);
        assert_eq!(rec.callouts.plan, CalloutEnds::End);
        // Construction Line Defaults edit the same slot.
        assert!(run_command(&mut cx, DEFAULTS));
        with_dialog(|d| {
            assert_eq!(d.target(), Target::Defaults);
            d.draft_mut().infinite_plan = false;
        })
        .unwrap();
        assert!(accept_dialog(&mut cx));
        assert!(!cx.project.construction.defaults.infinite_plan);
        assert_eq!(cx.project.construction.defaults.id, 0);
    }

    #[test]
    fn convert_buttons_go_both_ways_in_one_undo_step() {
        let mut cx = cx();
        let id = line(&mut cx);
        let acts = edit_actions(&cx);
        let labels: Vec<_> = acts.iter().map(|a| a.label).collect();
        assert!(labels.contains(&"Construction Line Specification"));
        assert!(labels.contains(&"Set as Default"));
        assert!(labels.contains(&"Convert to Polyline"));
        run_command(&mut cx, TO_POLYLINE);
        assert!(!cx.floor().is_construction_line(id));
        assert_eq!(cx.undo_label(), Some("Convert to Polyline"));
        // The polyline offers the way back.
        let labels: Vec<_> = edit_actions(&cx).iter().map(|a| a.label).collect();
        assert_eq!(labels, vec!["Convert Polyline to Construction Line"]);
        run_command(&mut cx, FROM_POLYLINE);
        assert!(cx.floor().is_construction_line(id));
        assert_eq!(cx.selection.items.len(), 1);
        cx.undo();
        assert!(!cx.floor().is_construction_line(id), "back to the polyline");
        cx.undo();
        assert!(cx.floor().is_construction_line(id));
    }

    #[test]
    fn the_dialog_wants_the_rules_dialog_on_request() {
        let mut cx = cx();
        let id = line(&mut cx);
        open_spec(&mut cx, id);
        with_dialog(|d| {
            assert!(!d.wants_rules());
            d.request_rules();
            assert!(d.wants_rules());
        })
        .unwrap();
        let _ = with_ends(CalloutSpec::default(), CalloutEnds::Start);
    }
}
