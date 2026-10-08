//! Staircase Specification and Landing Specification (CB-32 in
//! `docs/parity/cabinets-stairs-framing-terrain-library.md`, the Staircase
//! Specification of `docs/chief-x18-dialogs.md`).
//!
//! Tabs of a stair: General (width, tread depth, riser height, number of
//! risers and treads, bottom and top height, the lock settings, the shape and
//! the solved result), Style (open or closed risers, nosing and tread
//! thickness, stringers), Newels/Balusters, Rails (what stands on each side
//! and the rail sizes), Line Style, Fill Style, Materials and Label. A landing
//! has General (width, depth, height, thickness), Line Style, Fill Style,
//! Materials and Label. The preview is the plan symbol above and a
//! side-elevation stick figure below.
//!
//! The dialog edits a cloned [`StairObj`]; the shell stores an OK with
//! `editor::stairs_view::apply_edit` (one undo step).

// The shell opens this dialog for `EditorRequest::OpenSpec(ObjectRef::Stair)`;
// until it does, nothing calls these items.
#![allow(dead_code)]

use super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED, PV_ACCENT,
    PV_FAINT, PV_INK,
};
use crate::editor::stairs_view::{self as view, first_flight_treads, StairObj};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::geometry::Point;
use plan_core::Id;
use plan_stairs::{solve, RailStyle, SideKind, StairShape, StringerStyle, Turn};

const STAIR_TABS: &[Tab] = &[
    on("General"),
    on("Style"),
    on("Newels/Balusters"),
    on("Rails"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
];

const LANDING_TABS: &[Tab] = &[
    on("General"),
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
    Curved,
    Ramp,
}

const SHAPES: [(ShapeSel, &str); 6] = [
    (ShapeSel::Straight, "Straight"),
    (ShapeSel::LShaped, "L-Shaped"),
    (ShapeSel::UShaped, "U-Shaped"),
    (ShapeSel::Winder, "L-Shaped with winders"),
    (ShapeSel::Curved, "Curved"),
    (ShapeSel::Ramp, "Ramp"),
];

fn shape_sel(s: &StairShape) -> ShapeSel {
    match s {
        StairShape::Straight | StairShape::Landing { .. } => ShapeSel::Straight,
        StairShape::LShaped { .. } => ShapeSel::LShaped,
        StairShape::UShaped { .. } => ShapeSel::UShaped,
        StairShape::Winder { .. } => ShapeSel::Winder,
        StairShape::Curved { .. } => ShapeSel::Curved,
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
        ShapeSel::Curved => StairShape::Curved {
            inner_radius: view::DEFAULT_CURVE_RADIUS - draft.stair.params.width / 2.0,
        },
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
            "Staircase Specification"
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
    fn landing_general(&mut self, ui: &mut Ui) {
        section(ui, "Landing");
        self.fields
            .length_row(ui, "Width", "width", &mut self.draft.stair.params.width);
        if self.draft.is_polygon_landing() {
            ui.weak("Drawn as a polygon: the outline follows the corners you clicked.");
        } else {
            let mut depth = self.draft.landing_depth().unwrap_or(view::DEFAULT_LANDING);
            if self
                .fields
                .length_row(ui, "Depth", "landing_depth", &mut depth)
            {
                self.draft.set_landing_depth(depth);
            }
        }
        self.fields.length_row(
            ui,
            "Height",
            "landing_height",
            &mut self.draft.stair.params.total_rise,
        );
        self.fields.length_row(
            ui,
            "Thickness",
            "landing_thickness",
            &mut self.draft.stair.params.slab_thickness,
        );
        ui.weak("A stair section that arrives on the landing sets its height; one that starts on it begins there.");
    }

    fn general(&mut self, ui: &mut Ui) {
        if self.draft.is_landing() {
            self.landing_general(ui);
            return;
        }
        section(ui, "General");
        self.fields
            .length_row(ui, "Width", "width", &mut self.draft.stair.params.width);
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
            let sol = solve(&self.draft.stair.params);
            let (mut risers, mut treads) = (sol.risers, sol.treads);
            row(ui, "Number of Risers", |ui| {
                if ui
                    .add(egui::DragValue::new(&mut risers).range(2..=99))
                    .changed()
                {
                    view::set_risers(&mut self.draft, risers);
                }
            });
            row(ui, "Number of Treads", |ui| {
                if ui
                    .add(egui::DragValue::new(&mut treads).range(1..=98))
                    .changed()
                {
                    view::set_treads(&mut self.draft, treads);
                }
            });
        }
        section(ui, "Heights");
        let (mut bottom, mut top) = (self.draft.bottom_height(), self.draft.top_height());
        if self
            .fields
            .length_row(ui, "Bottom Height", "bottom_height", &mut bottom)
        {
            view::set_bottom_height(&mut self.draft, bottom);
        }
        if self
            .fields
            .length_row(ui, "Top Height", "top_height", &mut top)
        {
            view::set_top_height(&mut self.draft, top);
        }
        if self.draft.x.story_rise > 0.0 {
            row(ui, "Floor to Floor", |ui| {
                ui.label(super::fmt_short(self.draft.x.story_rise));
                if ui.button("Fit stair to floor-to-floor").clicked() {
                    view::fit_to_story(&mut self.draft);
                }
            });
        }
        if !ramp {
            section(ui, "Lock Settings");
            row(ui, "Lock", |ui| {
                ui.checkbox(&mut self.draft.x.lock_tread, "Tread depth");
                ui.checkbox(&mut self.draft.x.lock_riser, "Riser height");
                ui.checkbox(&mut self.draft.x.lock_count, "Number of treads");
            });
            ui.weak("Locked values stay put when the heights or the number of risers change.");
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
            StairShape::Curved { inner_radius } => {
                let mut r = *inner_radius;
                if self
                    .fields
                    .length_row(ui, "Inside Radius", "inner_radius", &mut r)
                {
                    *inner_radius = r.max(0.0);
                }
            }
            StairShape::Straight | StairShape::Landing { .. } => {}
        }
        // The winders option: the turn of an L-shaped stair is a landing or a fan of treads.
        match self.draft.stair.params.shape {
            StairShape::LShaped { .. } => {
                let mut winders = false;
                if ui
                    .checkbox(&mut winders, "Use winders in the turn instead of a landing")
                    .changed()
                    && winders
                {
                    set_shape(&mut self.draft, ShapeSel::Winder);
                }
            }
            StairShape::Winder { .. } => {
                let mut winders = true;
                if ui
                    .checkbox(&mut winders, "Use winders in the turn instead of a landing")
                    .changed()
                    && !winders
                {
                    set_shape(&mut self.draft, ShapeSel::LShaped);
                }
            }
            _ => {}
        }
        if matches!(
            self.draft.stair.params.shape,
            StairShape::LShaped { .. }
                | StairShape::UShaped { .. }
                | StairShape::Winder { .. }
                | StairShape::Curved { .. }
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
            ro(
                ui,
                "Actual Riser Height",
                super::fmt_short(sol.riser_height),
            );
        } else if sol.landings > 0 {
            ro(ui, "Ramp Landings", sol.landings.to_string());
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
        row(ui, "Stringer Style", |ui| {
            egui::ComboBox::from_id_salt("stair_stringer")
                .selected_text(stringer_name(p.stringer))
                .show_ui(ui, |ui| {
                    for s in [
                        StringerStyle::Closed,
                        StringerStyle::Open,
                        StringerStyle::None,
                    ] {
                        ui.selectable_value(&mut p.stringer, s, stringer_name(s));
                    }
                });
        });
        self.fields
            .length_row(ui, "Stringer Depth", "stringer", &mut p.stringer_depth);
        self.fields
            .length_row(ui, "Slab Thickness", "slab_t", &mut p.slab_thickness);
        section(ui, "Handrail");
        ui.checkbox(&mut p.handrail, "Handrail on both sides");
    }

    fn newels_balusters(&mut self, ui: &mut Ui) {
        let r = &mut self.draft.stair.params.railing;
        section(ui, "Newels");
        self.fields
            .length_row(ui, "Newel Size", "newel_size", &mut r.newel.size);
        self.fields
            .length_row(ui, "Newel Height", "newel_height", &mut r.newel.height);
        self.fields.length_row(
            ui,
            "Maximum Spacing",
            "newel_spacing",
            &mut r.newel.max_spacing,
        );
        ui.checkbox(&mut r.newel.cap, "Newel cap");
        section(ui, "Balusters");
        let name = baluster_name(&r.style);
        row(ui, "Infill", |ui| {
            egui::ComboBox::from_id_salt("stair_infill")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (label, make) in BALUSTER_STYLES {
                        let on = baluster_name(&r.style) == label;
                        if ui.selectable_label(on, label).clicked() && !on {
                            r.style = make();
                        }
                    }
                });
        });
        match &mut r.style {
            RailStyle::Balusters { spacing, size } => {
                self.fields
                    .length_row(ui, "Clear Spacing", "baluster_spacing", spacing);
                self.fields
                    .length_row(ui, "Baluster Size", "baluster_size", size);
            }
            RailStyle::Cable { rows } => {
                row(ui, "Cable Rows", |ui| {
                    ui.add(egui::DragValue::new(rows).range(1..=12));
                });
            }
            _ => {}
        }
        ui.weak("Balusters stand on the treads, enough per tread to keep every opening within the clear spacing (4\" by code).");
    }

    fn rails(&mut self, ui: &mut Ui) {
        section(ui, "Sides");
        for (label, left) in [("Left Side", true), ("Right Side", false)] {
            row(ui, label, |ui| {
                let p = &mut self.draft.stair.params;
                let side = if left {
                    &mut p.left_side
                } else {
                    &mut p.right_side
                };
                egui::ComboBox::from_id_salt(if left { "stair_left" } else { "stair_right" })
                    .selected_text(side.name())
                    .show_ui(ui, |ui| {
                        for k in SideKind::ALL {
                            ui.selectable_value(side, k, k.name());
                        }
                    });
            });
        }
        let r = &mut self.draft.stair.params.railing;
        section(ui, "Rails");
        self.fields
            .length_row(ui, "Guard Height", "guard", &mut r.height);
        self.fields
            .length_row(ui, "Top Rail Width", "top_rail_w", &mut r.top_rail.0);
        self.fields
            .length_row(ui, "Top Rail Height", "top_rail_h", &mut r.top_rail.1);
        self.fields.length_row(
            ui,
            "Bottom Rail Width",
            "bottom_rail_w",
            &mut r.bottom_rail.0,
        );
        self.fields.length_row(
            ui,
            "Bottom Rail Height",
            "bottom_rail_h",
            &mut r.bottom_rail.1,
        );
        ui.weak("Railing: newels, balusters and a rail that follows the pitch. Half Wall: a cap rail on a solid panel. Wall: a full-height wall.");
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

fn stringer_name(s: StringerStyle) -> &'static str {
    match s {
        StringerStyle::Closed => "Closed (full board)",
        StringerStyle::Open => "Open (notched)",
        StringerStyle::None => "None",
    }
}

type MakeStyle = fn() -> RailStyle;

const BALUSTER_STYLES: [(&str, MakeStyle); 5] = [
    ("Balusters", || RailStyle::Balusters {
        spacing: plan_stairs::MAX_BALUSTER_CLEAR,
        size: 1.5,
    }),
    ("Panels", || RailStyle::Panels),
    ("Solid", || RailStyle::Solid),
    ("Cables", || RailStyle::Cable { rows: 4 }),
    ("Glass", || RailStyle::Glass),
];

fn baluster_name(s: &RailStyle) -> &'static str {
    match s {
        RailStyle::Balusters { .. } => "Balusters",
        RailStyle::Panels => "Panels",
        RailStyle::Solid => "Solid",
        RailStyle::Cable { .. } => "Cables",
        RailStyle::Glass => "Glass",
    }
}

impl SpecPages for StairForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.draft.is_landing() {
            LANDING_TABS
        } else {
            STAIR_TABS
        }
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
        let tabs = self.tabs();
        match tabs[tab.min(tabs.len() - 1)].name {
            "General" => self.general(ui),
            "Style" => self.style(ui),
            "Newels/Balusters" => self.newels_balusters(ui),
            "Rails" => self.rails(ui),
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

    #[test]
    fn the_tabs_follow_the_staircase_and_landing_specifications() {
        let (_, o) = cx_with_stair();
        let d = StairDialog::new(o.clone());
        let names: Vec<_> = d.form.tabs().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "General",
                "Style",
                "Newels/Balusters",
                "Rails",
                "Line Style",
                "Fill Style",
                "Materials",
                "Label"
            ]
        );
        let mut landing = o;
        landing.set_landing_depth(48.0);
        let d = StairDialog::new(landing);
        let names: Vec<_> = d.form.tabs().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            ["General", "Line Style", "Fill Style", "Materials", "Label"]
        );
    }

    #[test]
    fn every_page_and_the_preview_draw_for_each_kind_of_object() {
        let (cx, o) = cx_with_stair();
        let mut objs = vec![o.clone()];
        let mut curved = o.clone();
        set_shape(&mut curved, ShapeSel::Curved);
        objs.push(curved);
        for sel in [
            ShapeSel::LShaped,
            ShapeSel::UShaped,
            ShapeSel::Winder,
            ShapeSel::Ramp,
        ] {
            let mut x = o.clone();
            set_shape(&mut x, sel);
            objs.push(x);
        }
        let mut landing = o.clone();
        landing.set_landing_depth(48.0);
        objs.push(landing);
        let mut polygon = view::build_polygon_landing(
            &cx.project,
            0,
            &[
                Point::new(0.0, 0.0),
                Point::new(60.0, 0.0),
                Point::new(60.0, 40.0),
            ],
        );
        polygon.stair.params.total_rise = 30.0;
        objs.push(polygon);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            for obj in &objs {
                let mut dlg = StairDialog::new(obj.clone());
                let _ = dlg.show(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    for tab in 0..dlg.form.tabs().len() {
                        dlg.form.page(ui, tab);
                    }
                    let (_, painter) =
                        ui.allocate_painter(egui::Vec2::new(220.0, 400.0), egui::Sense::hover());
                    dlg.form.preview(&painter, painter.clip_rect());
                });
            }
        });
    }

    #[test]
    fn the_dialog_applies_everything_with_one_undo_step() {
        let (mut cx, o) = cx_with_stair();
        let before = view::find(cx.floor(), o.id()).unwrap();
        let mut d = StairDialog::new(o.clone());
        {
            let draft = d.draft_mut();
            // Number of risers, a railing on the left, a half-wall on the right,
            // notched stringers, the balusters and the locks.
            view::set_risers(draft, 17);
            draft.stair.params.left_side = SideKind::Railing;
            draft.stair.params.right_side = SideKind::HalfWall;
            draft.stair.params.stringer = StringerStyle::Open;
            draft.stair.params.open_risers = true;
            draft.stair.params.railing.style = RailStyle::Balusters {
                spacing: 3.0,
                size: 1.25,
            };
            draft.stair.params.railing.newel.size = 4.0;
            draft.x.lock_tread = true;
            draft.x.lock_count = true;
        }
        assert!(d.problem().is_none());
        assert!(view::apply_edit(&mut cx, d.draft()));
        let after = view::find(cx.floor(), o.id()).unwrap();
        assert_eq!(after.solution().risers, 17);
        assert_eq!(after.stair.params.left_side, SideKind::Railing);
        assert_eq!(after.stair.params.right_side, SideKind::HalfWall);
        assert_eq!(after.stair.params.stringer, StringerStyle::Open);
        assert!(after.stair.params.open_risers);
        assert_eq!(after.stair.params.railing.newel.size, 4.0);
        assert!(after.x.lock_tread && after.x.lock_count);
        // One undo step takes it all back.
        assert_eq!(cx.undo().as_deref(), Some("Stair Specification"));
        assert_eq!(view::find(cx.floor(), o.id()).unwrap(), before);
        assert_ne!(cx.undo_label(), Some("Stair Specification"));
        // The sides show up in the 3D meshes after the edit.
        cx.redo();
        let parts = plan_stairs::tagged_meshes(&view::find(cx.floor(), o.id()).unwrap().stair);
        assert!(parts
            .iter()
            .any(|(p, _)| *p == plan_stairs::StairPart::Handrail));
    }

    #[test]
    fn the_heights_and_locks_of_the_general_tab_follow_the_rules() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        let top = d.draft().top_height();
        let draft = d.draft_mut();
        // Top height 100": 13 risers; with the number of treads locked 90" keeps 13.
        view::set_top_height(draft, 100.0);
        assert_eq!(draft.solution().risers, 13);
        draft.x.lock_count = true;
        view::set_top_height(draft, 90.0);
        assert_eq!(draft.solution().risers, 13);
        assert!(draft.solution().riser_height < 7.0);
        // The floor-to-floor button restores the drawn height.
        assert!(view::fit_to_story(draft));
        assert!((draft.top_height() - top).abs() < 1e-9);
    }

    #[test]
    fn a_landing_dialog_edits_depth_height_and_thickness() {
        let (mut cx, o) = cx_with_stair();
        let mut landing = view::build(
            &cx.project,
            0,
            StairKind::Landing,
            Turn::Left,
            Point::new(200.0, 0.0),
            Some(Point::new(260.0, 40.0)),
        );
        cx.begin_change("Landing");
        let id = view::add(&mut cx.project, 0, landing.clone());
        landing = view::find(cx.floor(), id).unwrap();
        let mut d = StairDialog::new(landing);
        d.draft_mut().set_landing_depth(72.0);
        d.draft_mut().stair.params.total_rise = 54.0;
        d.draft_mut().stair.params.slab_thickness = 5.5;
        assert!(view::apply_edit(&mut cx, d.draft()));
        let back = view::find(cx.floor(), id).unwrap();
        assert_eq!(back.landing_depth(), Some(72.0));
        assert_eq!(back.landing_height(), 54.0);
        assert_eq!(back.stair.params.slab_thickness, 5.5);
        let (lo, hi) = plan_stairs::tagged_meshes(&back.stair)[0]
            .1
            .bounds()
            .unwrap();
        assert!((f64::from(hi[1]) - 54.0).abs() < 1e-6 && (f64::from(lo[1]) - 48.5).abs() < 1e-6);
        let _ = o;
    }

    #[test]
    fn the_winders_option_swaps_the_landing_for_pie_treads() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        set_shape(d.draft_mut(), ShapeSel::LShaped);
        assert!(matches!(
            d.draft().stair.params.shape,
            StairShape::LShaped { .. }
        ));
        set_shape(d.draft_mut(), ShapeSel::Winder);
        let sol = d.draft().solution();
        assert!(matches!(
            d.draft().stair.params.shape,
            StairShape::Winder { winders: 3 }
        ));
        assert_eq!(sol.landings, 0, "winders replace the flat landing");
        assert_eq!(sol.risers, 16);
        set_shape(d.draft_mut(), ShapeSel::Curved);
        assert!(d.draft().is_curved());
        assert!(
            d.draft().solution().code_ok,
            "{:?}",
            d.draft().solution().warnings
        );
    }
}
