//! Stair Specification (CB-32 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`).
//!
//! Tabs: General (width, tread depth, riser height with lock options, the
//! solved risers / treads / rise / run, headroom, shape), Style (open risers,
//! nosing, stringer depth, handrail), Railing (placeholder), Line Style, Fill
//! Style, Materials and Label. The preview is the plan symbol above and a
//! side-elevation stick figure of the risers and treads below.
//!
//! The dialog edits a cloned [`StairObj`]; the shell stores an OK with
//! `editor::stairs_view::apply_edit`.

// The shell opens this dialog for `EditorRequest::OpenSpec(ObjectRef::Stair)`;
// until it does, nothing calls these items.
#![allow(dead_code)]

use super::{
    dis_combo, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED,
    PV_ACCENT, PV_FAINT, PV_INK,
};
use crate::editor::stairs_view::{self as view, first_flight_treads, StairObj};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::geometry::Point;
use plan_core::Id;
use plan_stairs::{solve, StairShape, Turn};

const STAIR_TABS: &[Tab] = &[
    on("General"),
    on("Style"),
    on("Railing"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ShapeSel {
    Straight,
    LShaped,
    UShaped,
    Winder,
    Ramp,
}

const SHAPES: [(ShapeSel, &str); 5] = [
    (ShapeSel::Straight, "Straight"),
    (ShapeSel::LShaped, "L-Shaped"),
    (ShapeSel::UShaped, "U-Shaped"),
    (ShapeSel::Winder, "Curved (winders)"),
    (ShapeSel::Ramp, "Ramp"),
];

fn shape_sel(s: &StairShape) -> ShapeSel {
    match s {
        StairShape::Straight => ShapeSel::Straight,
        StairShape::LShaped { .. } => ShapeSel::LShaped,
        StairShape::UShaped { .. } => ShapeSel::UShaped,
        StairShape::Winder { .. } => ShapeSel::Winder,
        StairShape::Ramp { .. } => ShapeSel::Ramp,
    }
}

/// Switches the draft to another shape with sensible defaults.
fn set_shape(draft: &mut StairObj, sel: ShapeSel) {
    let risers = solve(&draft.stair.params).risers;
    let half = risers.saturating_sub(2) / 2;
    draft.stair.params.shape = match sel {
        ShapeSel::Straight => StairShape::Straight,
        ShapeSel::LShaped => StairShape::LShaped {
            treads_before_landing: half,
        },
        ShapeSel::UShaped => StairShape::UShaped {
            treads_before_landing: half,
        },
        ShapeSel::Winder => StairShape::Winder { winders: 3 },
        ShapeSel::Ramp => StairShape::Ramp { slope_1_in: 12.0 },
    };
}

struct StairForm {
    draft: StairObj,
    fields: Fields,
}

pub struct StairDialog {
    frame: SpecDialog,
    form: StairForm,
}

impl StairDialog {
    pub fn new(obj: StairObj) -> Self {
        let title = if obj.is_landing() {
            "Landing Specification"
        } else if obj.is_ramp() {
            "Ramp Specification"
        } else {
            "Stair Specification"
        };
        Self {
            frame: SpecDialog::new(title, "stairs"),
            form: StairForm {
                draft: obj,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    /// The edited stair; store it with `stairs_view::apply_edit` on OK.
    pub fn draft(&self) -> &StairObj {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut StairObj {
        &mut self.form.draft
    }

    pub fn id(&self) -> Id {
        self.form.draft.id()
    }

    /// Why OK is refused, if it is.
    pub fn problem(&self) -> Option<String> {
        self.form.error()
    }
}

impl StairForm {
    fn general(&mut self, ui: &mut Ui) {
        let landing = self.draft.is_landing();
        section(ui, "General");
        self.fields
            .length_row(ui, "Width", "width", &mut self.draft.stair.params.width);
        if landing {
            let mut depth = self.draft.x.landing_depth.unwrap_or(view::DEFAULT_LANDING);
            if self
                .fields
                .length_row(ui, "Depth", "landing_depth", &mut depth)
            {
                self.draft.x.landing_depth = Some(depth);
            }
            return;
        }
        let ramp = self.draft.is_ramp();
        if !ramp {
            self.fields.length_row(
                ui,
                "Tread Depth",
                "tread",
                &mut self.draft.stair.params.tread_depth,
            );
            self.fields.length_row(
                ui,
                "Riser Height",
                "riser",
                &mut self.draft.stair.params.riser_height_target,
            );
            row(ui, "Lock", |ui| {
                ui.checkbox(&mut self.draft.x.lock_tread, "Tread depth");
                ui.checkbox(&mut self.draft.x.lock_riser, "Riser height");
            });
        }
        self.fields.length_row(
            ui,
            "Headroom",
            "headroom",
            &mut self.draft.stair.params.headroom_min,
        );

        section(ui, "Shape");
        row(ui, "Stair Shape", |ui| {
            let cur = shape_sel(&self.draft.stair.params.shape);
            let name = SHAPES.iter().find(|s| s.0 == cur).map_or("", |s| s.1);
            egui::ComboBox::from_id_salt("stair_shape")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (sel, label) in SHAPES {
                        if ui.selectable_label(cur == sel, label).clicked() && cur != sel {
                            set_shape(&mut self.draft, sel);
                        }
                    }
                });
        });
        let risers = solve(&self.draft.stair.params).risers;
        let regular = risers.saturating_sub(2);
        match &mut self.draft.stair.params.shape {
            StairShape::LShaped {
                treads_before_landing,
            }
            | StairShape::UShaped {
                treads_before_landing,
            } => {
                row(ui, "Treads Before Landing", |ui| {
                    ui.add(egui::DragValue::new(treads_before_landing).range(0..=regular));
                });
            }
            StairShape::Winder { winders } => {
                row(ui, "Winder Treads", |ui| {
                    ui.add(egui::DragValue::new(winders).range(1..=6));
                });
            }
            StairShape::Ramp { slope_1_in } => {
                row(ui, "Slope (1 in)", |ui| {
                    ui.add(
                        egui::DragValue::new(slope_1_in)
                            .range(1.0..=40.0)
                            .speed(0.1),
                    );
                });
            }
            StairShape::Straight => {}
        }
        if matches!(
            self.draft.stair.params.shape,
            StairShape::LShaped { .. } | StairShape::UShaped { .. } | StairShape::Winder { .. }
        ) {
            row(ui, "Turn", |ui| {
                let t = &mut self.draft.stair.params.turn;
                ui.radio_value(t, Turn::Left, "Left");
                ui.radio_value(t, Turn::Right, "Right");
            });
        }
        if matches!(
            self.draft.stair.params.shape,
            StairShape::LShaped { .. } | StairShape::UShaped { .. }
        ) {
            self.fields.length_row(
                ui,
                "Landing Depth",
                "landing",
                &mut self.draft.stair.params.landing_depth,
            );
        }

        section(ui, "Solved from the floor-to-floor rise");
        let sol = solve(&self.draft.stair.params);
        let ro = |ui: &mut Ui, label: &str, text: String| {
            row(ui, label, |ui| ui.label(text));
        };
        ro(
            ui,
            "Total Rise",
            super::fmt_short(self.draft.stair.params.total_rise),
        );
        if !ramp {
            ro(ui, "Number of Risers", sol.risers.to_string());
            ro(ui, "Number of Treads", sol.treads.to_string());
            ro(
                ui,
                "Actual Riser Height",
                super::fmt_short(sol.riser_height),
            );
        }
        ro(ui, "Total Run", super::fmt_short(sol.total_run));
        if sol.code_ok && sol.warnings.is_empty() {
            ui.weak("Meets the IRC limits.");
        }
        for w in &sol.warnings {
            ui.colored_label(ERROR_RED, w);
        }
    }

    fn style(&mut self, ui: &mut Ui) {
        if self.draft.is_landing() {
            ui.weak("A landing has no steps.");
            return;
        }
        let p = &mut self.draft.stair.params;
        section(ui, "Treads and Risers");
        ui.checkbox(&mut p.open_risers, "Open risers");
        self.fields
            .length_row(ui, "Nosing", "nosing", &mut p.nosing);
        self.fields
            .length_row(ui, "Tread Thickness", "tread_t", &mut p.tread_thickness);
        self.fields
            .length_row(ui, "Riser Thickness", "riser_t", &mut p.riser_thickness);
        section(ui, "Stringers");
        self.fields
            .length_row(ui, "Stringer Depth", "stringer", &mut p.stringer_depth);
        section(ui, "Handrail");
        ui.checkbox(&mut p.handrail, "Handrail on both sides");
    }

    fn railing(&mut self, ui: &mut Ui) {
        section(ui, "Railing");
        ui.checkbox(&mut self.draft.x.railing_left, "Left railing");
        ui.checkbox(&mut self.draft.x.railing_right, "Right railing");
        section(ui, "Rail Style");
        row(ui, "Rail Style", |ui| {
            dis_combo(ui, "stair_rail_style", "(none yet)")
        });
        row(ui, "Newel Style", |ui| {
            dis_combo(ui, "stair_newel_style", "(none yet)")
        });
        row(ui, "Baluster Style", |ui| {
            dis_combo(ui, "stair_baluster_style", "(none yet)")
        });
        ui.weak("Railing objects arrive with the Railing tool; the sides marked here are kept on the stair.");
    }

    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Plan Lines");
        row(ui, "Line Weight", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.x.line_weight)
                    .range(0.25..=4.0)
                    .speed(0.05),
            );
        });
        ui.checkbox(&mut self.draft.x.dashed, "Dashed lines");
        if !self.draft.is_landing() {
            ui.checkbox(&mut self.draft.x.break_line, "Break line");
            ui.checkbox(&mut self.draft.x.show_risers, "Show number of risers");
        }
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Plan Fill");
        ui.checkbox(&mut self.draft.x.fill, "Fill the stair in plan");
        row(ui, "Fill Tone", |ui| {
            let mut g = f64::from(self.draft.x.fill_gray);
            if ui
                .add(egui::Slider::new(&mut g, 0.0..=255.0).integer())
                .changed()
            {
                self.draft.x.fill_gray = g as u8;
            }
        });
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        for (component, material) in &mut self.draft.x.materials {
            let label = component.clone();
            row(ui, &label, |ui| {
                ui.add(egui::TextEdit::singleline(material).desired_width(160.0));
            });
        }
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        ui.checkbox(&mut self.draft.x.show_label, "Show label in plan");
        row(ui, "Label Text", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.x.label).desired_width(180.0));
        });
    }
}

impl SpecPages for StairForm {
    fn tabs(&self) -> &'static [Tab] {
        STAIR_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        let p = &self.draft.stair.params;
        if p.width <= 0.0 {
            return Some("Width must be greater than zero".into());
        }
        if self.draft.is_landing() {
            return None;
        }
        if !self.draft.is_ramp() && (p.tread_depth <= 0.0 || p.riser_height_target <= 0.0) {
            return Some("Tread depth and riser height must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match STAIR_TABS[tab].name {
            "General" => self.general(ui),
            "Style" => self.style(ui),
            "Railing" => self.railing(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let top = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y - 4.0));
        let bottom = Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y + 4.0), rect.max);
        pv_text(
            p,
            top.min + egui::vec2(0.0, 4.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        plan_preview(
            p,
            Rect::from_min_max(top.min + egui::vec2(0.0, 12.0), top.max),
            &self.draft,
        );
        pv_text(
            p,
            bottom.min + egui::vec2(0.0, 4.0),
            Align2::LEFT_CENTER,
            "Side elevation",
            11.0,
        );
        elevation_preview(
            p,
            Rect::from_min_max(bottom.min + egui::vec2(0.0, 12.0), bottom.max),
            &self.draft,
        );
    }
}

/// The plan symbol fitted into `area`.
fn plan_preview(p: &Painter, area: Rect, o: &StairObj) {
    let fp = o.footprint();
    let (mut lo, mut hi) = (
        Point::new(f64::MAX, f64::MAX),
        Point::new(f64::MIN, f64::MIN),
    );
    for q in &fp {
        lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
    }
    if fp.is_empty() {
        return;
    }
    let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
    let area = area.shrink(6.0);
    let s = (f64::from(area.width()) / w).min(f64::from(area.height()) / h);
    let cam = Camera {
        center: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
        px_per_in: s.max(0.01),
        rect: area,
    };
    view::draw_strokes(p, &cam, &view::symbol_strokes(o), PV_INK, 1.0, false);
}

/// Risers and treads in section, from the solved layout.
fn elevation_preview(p: &Painter, area: Rect, o: &StairObj) {
    let pts = view::elevation_points(o);
    let (mut max_x, mut max_y) = (1.0f64, 1.0f64);
    for q in &pts {
        max_x = max_x.max(q.0);
        max_y = max_y.max(q.1);
    }
    let area = area.shrink2(egui::vec2(6.0, 14.0));
    let s = (f64::from(area.width()) / max_x).min(f64::from(area.height()) / max_y) as f32;
    let at = |q: &(f64, f64)| Pos2::new(area.min.x + q.0 as f32 * s, area.max.y - q.1 as f32 * s);
    let screen: Vec<Pos2> = pts.iter().map(at).collect();
    p.add(Shape::line(screen.clone(), Stroke::new(1.5_f32, PV_INK)));
    let floor = Stroke::new(0.8_f32, PV_FAINT);
    p.hline(
        area.min.x - 4.0..=area.min.x + (max_x as f32) * s + 4.0,
        area.max.y,
        floor,
    );
    if let Some(last) = screen.last() {
        p.hline(
            (last.x - 4.0)..=(last.x + 12.0),
            last.y,
            Stroke::new(1.0_f32, PV_ACCENT),
        );
    }
    let sol = o.solution();
    let text = if o.is_ramp() {
        format!(
            "1:{:.1} ramp, run {}",
            sol.total_run / o.stair.params.total_rise.max(1.0),
            super::fmt_short(sol.total_run)
        )
    } else {
        format!(
            "{} risers @ {}, {} treads @ {} (first flight {} treads)",
            sol.risers,
            super::fmt_short(sol.riser_height),
            sol.treads,
            super::fmt_short(sol.tread_depth),
            first_flight_treads(&o.stair.params, sol.risers)
        )
    };
    pv_text(
        p,
        Pos2::new(area.min.x, area.max.y + 8.0),
        Align2::LEFT_CENTER,
        text,
        10.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::stairs_view::StairKind;
    use crate::editor::{EditorContext, ObjectRef};
    use crate::plan_defaults;

    fn cx_with_stair() -> (EditorContext, StairObj) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let obj = view::build(
            &cx.project,
            0,
            StairKind::Draw,
            Turn::Left,
            Point::new(0.0, 0.0),
            Some(Point::new(150.0, 0.0)),
        );
        cx.begin_change("Draw Stairs");
        let id = view::add(&mut cx.project, 0, obj);
        cx.selection.set(ObjectRef::Stair(id));
        let o = view::find(cx.floor(), id).unwrap();
        (cx, o)
    }

    #[test]
    fn a_width_edit_persists_and_undoes() {
        let (mut cx, o) = cx_with_stair();
        let mut d = StairDialog::new(o.clone());
        assert_eq!(d.id(), o.id());
        assert!(d.problem().is_none());
        d.draft_mut().stair.params.width = 48.0;
        assert!(view::apply_edit(&mut cx, d.draft()));
        let back = view::find(cx.floor(), o.id()).unwrap();
        assert_eq!(back.stair.params.width, 48.0);
        assert_eq!(cx.undo().as_deref(), Some("Stair Specification"));
        assert_eq!(
            view::find(cx.floor(), o.id()).unwrap().stair.params.width,
            36.0
        );
    }

    #[test]
    fn shape_switch_keeps_the_solved_rise() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        set_shape(d.draft_mut(), ShapeSel::LShaped);
        assert_eq!(
            d.draft().stair.params.shape,
            StairShape::LShaped {
                treads_before_landing: 7
            }
        );
        assert_eq!(d.draft().solution().risers, 16);
        set_shape(d.draft_mut(), ShapeSel::Ramp);
        assert!(d.draft().is_ramp());
    }

    #[test]
    fn invalid_values_block_ok() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        d.draft_mut().stair.params.width = 0.0;
        assert!(d.problem().is_some());
    }
}
