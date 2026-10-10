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

use super::code_notice::{code_notice, LimitKind};
use super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED, PV_ACCENT,
    PV_FAINT, PV_INK,
};

use crate::editor::code;
use crate::editor::stairs_view::{self as view, first_flight_treads, StairObj};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::geometry::Point;
use plan_core::Id;
use plan_stairs::{
    solve, ArrowStyle, BreakStyle, Bullnose, DisplayRule, EdgeRail, LockEnd, PostProfile,
    RadiusRef, RailStyle, RailingParams, SideKind, StairParams, StairShape, Starter, StringerStyle,
    TreadMode, Turn, ViewMode,
};

/// The Staircase Specification (and Ramp Specification) of Chief X18. The
/// frame adds Properties, the Materials List's Components and Object
/// Information after these (`object_info`), so a stair has all of Chief's.
const STAIR_TABS: &[Tab] = &[
    on("General"),
    on("Style"),
    on("Stringers"),
    on("Rails"),
    on("Newels/Balusters"),
    on("Rail Style"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Schedule"),
];

const LANDING_TABS: &[Tab] = &[
    on("General"),
    on("Rails"),
    on("Newels/Balusters"),
    on("Rail Style"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
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

/// Which side the Newels/Balusters and Rails tabs edit: both share one set
/// of settings until a side is given its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum RailScope {
    #[default]
    Both,
    Left,
    Right,
}

/// The railing settings `scope` edits. A side that has none of its own is
/// given a copy of the shared ones the first time it is edited.
fn scoped_railing(p: &mut StairParams, scope: RailScope) -> &mut RailingParams {
    let shared = p.railing;
    match scope {
        RailScope::Both => &mut p.railing,
        RailScope::Left => p.left_railing.get_or_insert(shared),
        RailScope::Right => p.right_railing.get_or_insert(shared),
    }
}

struct StairForm {
    draft: StairObj,
    fields: Fields,
    scope: RailScope,
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
                scope: RailScope::default(),
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
        code_notice(
            ui,
            "IRC R311.7.6 landing width",
            &mut self.draft.stair.params.width,
            code::active().stair_width_min,
            LimitKind::Min,
        );
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
        if self.fields.length_row(
            ui,
            "Height",
            "landing_height",
            &mut self.draft.stair.params.total_rise,
        ) {
            // A typed height holds, whatever the stairs do.
            self.draft.stair.params.landing_auto_height = false;
        }
        ui.checkbox(
            &mut self.draft.stair.params.landing_auto_height,
            "Auto Adjust Height",
        );
        if self.fields.length_row(
            ui,
            "Thickness",
            "landing_thickness",
            &mut self.draft.stair.params.slab_thickness,
        ) {
            self.draft.stair.params.landing_auto_thickness = false;
        }
        ui.checkbox(
            &mut self.draft.stair.params.landing_auto_thickness,
            "Auto Adjust Thickness",
        );
        ui.weak("A stair section that arrives on the landing sets its height; one that starts on it begins there.");
    }

    /// The Staircase Information read-outs and Make Best Fit.
    fn staircase_information(&mut self, ui: &mut Ui) {
        section(ui, "Staircase Information");
        let p = &self.draft.stair.params;
        let sol = self.draft.solution();
        let info = plan_stairs::info(p.total_rise, sol.risers, p.tread_depth, true);
        ui.label(&info.reach);
        ui.label(&info.best_fit);
        let sections = plan_stairs::spec_rows(&[&self.draft.stair])
            .iter()
            .map(|r| r.number.split('-').next().unwrap_or("").to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        ui.label(format!(
            "Sections: {sections}   Risers: {}   Rise Angle: {:.1} deg",
            sol.risers, info.rise_angle
        ));
        if ui
            .add_enabled(info.can_make_best_fit, egui::Button::new("Make Best Fit"))
            .clicked()
        {
            view::best_fit_draft(&mut self.draft);
        }
    }

    /// The table of sections and subsections (ten lines at most).
    fn specifications(&mut self, ui: &mut Ui) {
        section(ui, "Specifications");
        let rows = plan_stairs::spec_rows(&[&self.draft.stair]);
        egui::Grid::new("stair_specs").striped(true).show(ui, |ui| {
            for h in [
                "Section",
                "Length",
                "Width",
                "Tread Depth",
                "Treads",
                "Bottom",
                "Top",
                "Riser",
            ] {
                ui.strong(h);
            }
            ui.end_row();
            for r in &rows {
                ui.label(&r.number);
                ui.label(super::fmt_short(r.length));
                ui.label(super::fmt_short(r.width));
                ui.label(super::fmt_short(r.tread_depth));
                ui.label(r.treads.to_string());
                ui.label(super::fmt_short(r.bottom_height));
                ui.label(super::fmt_short(r.top_height));
                ui.label(super::fmt_short(r.riser_height));
                ui.end_row();
            }
        });
    }

    fn general(&mut self, ui: &mut Ui) {
        if self.draft.is_landing() {
            self.landing_general(ui);
            return;
        }
        section(ui, "General");
        self.fields
            .length_row(ui, "Width", "width", &mut self.draft.stair.params.width);
        code_notice(
            ui,
            "IRC R311.7.1 stair width",
            &mut self.draft.stair.params.width,
            code::active().stair_width_min,
            LimitKind::Min,
        );
        let ramp = self.draft.is_ramp();
        if !ramp {
            self.fields.length_row(
                ui,
                "Tread Depth",
                "tread",
                &mut self.draft.stair.params.tread_depth,
            );
            code_notice(
                ui,
                "IRC R311.7.5.2 tread depth",
                &mut self.draft.stair.params.tread_depth,
                code::active().stair_tread_min,
                LimitKind::Min,
            );
            self.fields.length_row(
                ui,
                "Riser Height",
                "riser",
                &mut self.draft.stair.params.riser_height_target,
            );
            code_notice(
                ui,
                "IRC R311.7.5.1 riser height",
                &mut self.draft.stair.params.riser_height_target,
                code::active().stair_riser_max,
                LimitKind::Max,
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
        self.fields.length_row(
            ui,
            "Headroom",
            "headroom",
            &mut self.draft.stair.params.headroom_min,
        );
        code_notice(
            ui,
            "IRC R311.7.2 headroom",
            &mut self.draft.stair.params.headroom_min,
            code::active().stair_headroom_min,
            LimitKind::Min,
        );
        if !ramp {
            self.staircase_information(ui);
            section(ui, "Advanced Options");
            let mut mode = view::tread_mode(&self.draft);
            row(ui, "Tread Depth", |ui| {
                for m in TreadMode::ALL {
                    ui.selectable_value(&mut mode, m, m.name());
                }
            });
            if mode != view::tread_mode(&self.draft) {
                if let Some((depth, count)) = mode.locks() {
                    self.draft.x.lock_tread = depth;
                    self.draft.x.lock_count = count;
                }
            }
            row(ui, "Lock", |ui| {
                ui.checkbox(&mut self.draft.x.lock_riser, "Riser height");
            });
            let mut len = view::section_length(&self.draft);
            if self
                .fields
                .length_row(ui, "Length", "section_length", &mut len)
            {
                view::set_length(&mut self.draft, len);
            }
            row(ui, "Lock End", |ui| {
                ui.selectable_value(&mut self.draft.x.lock_end, LockEnd::Top, "Lock Top");
                ui.selectable_value(&mut self.draft.x.lock_end, LockEnd::Bottom, "Lock Bottom");
            });
            ui.weak("Locked values stay put when the heights or the number of risers change; the Lock End says which end of the section stays when its length changes.");
            self.specifications(ui);
        }

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
                code_notice(
                    ui,
                    "IBC 1012.2 ramp slope (1 in)",
                    slope_1_in,
                    plan_stairs::RAMP_MIN_SLOPE,
                    LimitKind::Min,
                );
            }
            StairShape::Curved { inner_radius } => {
                let mut r = *inner_radius;
                let label = if self.draft.stair.params.spiral {
                    "Pole Radius"
                } else {
                    "Inside Radius"
                };
                if self.fields.length_row(ui, label, "inner_radius", &mut r) {
                    *inner_radius = r.max(0.0);
                }
                if self.draft.stair.params.spiral {
                    let outside = r + self.draft.stair.params.width;
                    row(ui, "Outside Radius", |ui| {
                        ui.label(super::fmt_short(outside))
                    });
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
        if matches!(self.draft.stair.params.shape, StairShape::UShaped { .. }) {
            let p = &mut self.draft.stair.params;
            self.fields
                .length_row(ui, "Gap Between Flights", "u_gap", &mut p.u_gap);
            p.u_gap = p.u_gap.max(0.0);
            ui.checkbox(
                &mut p.split_landing,
                "Split landing (two landings, one at the end of each flight)",
            );
        }
        if matches!(self.draft.stair.params.shape, StairShape::Curved { .. })
            && !self.draft.stair.params.spiral
        {
            let p = &mut self.draft.stair.params;
            combo(
                ui,
                "Radius Reference",
                "radius_ref",
                &mut p.radius_ref,
                &RadiusRef::ALL,
                RadiusRef::name,
            );
            let which = p.radius_ref;
            let mut r = p.curve_radius(which).unwrap_or(0.0);
            if self.fields.length_row(ui, "Radius", "curve_radius", &mut r) {
                p.set_curve_radius(which, r);
            }
        }
        if self.draft.is_ramp() {
            let p = &mut self.draft.stair.params;
            let mut curved = p.ramp_curve.is_some();
            if ui.checkbox(&mut curved, "Curved ramp").changed() {
                p.ramp_curve =
                    curved.then(|| (view::DEFAULT_CURVE_RADIUS - p.width / 2.0).max(0.0));
            }
            if let Some(r) = &mut p.ramp_curve {
                self.fields.length_row(ui, "Inside Radius", "ramp_inner", r);
                *r = r.max(0.0);
            }
        }
        if let StairShape::Curved { inner_radius } = &mut self.draft.stair.params.shape {
            // A spiral: wedge treads round a centre pole, judged by the
            // spiral-stair code (9 1/2" risers, 6 3/4" treads, 26" wide).
            let before = self.draft.stair.params.spiral;
            ui.checkbox(
                &mut self.draft.stair.params.spiral,
                "Spiral stair (wedge treads round a centre pole)",
            );
            if self.draft.stair.params.spiral && !before {
                *inner_radius = plan_stairs::SPIRAL_POLE_RADIUS;
            }
        }
        if matches!(
            self.draft.stair.params.shape,
            StairShape::LShaped { .. }
                | StairShape::UShaped { .. }
                | StairShape::Winder { .. }
                | StairShape::Curved { .. }
        ) || (self.draft.is_ramp() && self.draft.stair.params.ramp_curve.is_some())
        {
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

        if self.draft.is_ramp() {
            let p = &mut self.draft.stair.params;
            section(ui, "Options");
            ui.checkbox(
                &mut p.railing_openings,
                "Automatic railing openings (a doorway is cut in a railing the ramp meets)",
            );
            ui.checkbox(&mut p.ramp.open_underneath, "Open underneath");
            if !p.ramp.open_underneath {
                self.fields.length_row(
                    ui,
                    "Max Thickness",
                    "ramp_max_t",
                    &mut p.ramp.max_thickness,
                );
                p.ramp.max_thickness = p.ramp.max_thickness.max(p.slab_thickness);
                ui.weak("A closed ramp is filled down to the floor, no deeper than this.");
            }
            section(ui, "Tread Surface");
            ui.checkbox(&mut p.ramp.has_surface, "Has tread surface");
            if p.ramp.has_surface {
                self.fields.length_row(
                    ui,
                    "Tread Overhang",
                    "ramp_surf_oh",
                    &mut p.ramp.surface_overhang,
                );
                self.fields.length_row(
                    ui,
                    "Tread Thickness",
                    "ramp_surf_t",
                    &mut p.ramp.surface_thickness,
                );
                p.ramp.surface_overhang = p.ramp.surface_overhang.max(0.0);
                p.ramp.surface_thickness = p.ramp.surface_thickness.clamp(0.0, p.slab_thickness);
            }
        }

        ui.checkbox(
            &mut self.draft.stair.params.down,
            "Downward stair (the plan arrow starts at the top and reads DN)",
        );

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
        let straight = view::straight_family(&self.draft);
        let p = &mut self.draft.stair.params;
        section(ui, "Treads and Risers");
        ui.checkbox(&mut p.open_risers, "Open risers");
        self.fields
            .length_row(ui, "Nosing", "nosing", &mut p.nosing);
        self.fields
            .length_row(ui, "Tread Thickness", "tread_t", &mut p.tread_thickness);
        self.fields
            .length_row(ui, "Riser Thickness", "riser_t", &mut p.riser_thickness);
        if matches!(
            p.shape,
            StairShape::Straight
                | StairShape::LShaped { .. }
                | StairShape::UShaped { .. }
                | StairShape::Winder { .. }
        ) {
            section(ui, "Bottom Tread");
            self.fields
                .length_row(ui, "Flared Bottom Tread", "flare", &mut p.flare);
            ui.weak("The bottom tread reaches this far past the stair on each side, in a half-round end. 0 keeps it square.");
            p.flare = p.flare.max(0.0);
            row(ui, "Bullnose Bottom Tread", |ui| {
                egui::ComboBox::from_id_salt("stair_bullnose")
                    .selected_text(p.bullnose.name())
                    .show_ui(ui, |ui| {
                        for b in Bullnose::ALL {
                            ui.selectable_value(&mut p.bullnose, b, b.name());
                        }
                    });
            });
            ui.weak("A bullnose rounds the chosen end of the bottom tread into a half-round the depth of the tread; it wins over the flare on that end.");
            combo(
                ui,
                "Starter Treads",
                "stair_starter",
                &mut p.starter,
                &Starter::ALL,
                Starter::name,
            );
            ui.weak("Starter treads are rounded and reach past the open sides; the second is concentric with the first. The Starter Tread edit mode has a handle for them.");
        }
        if straight {
            section(ui, "Flare and Curve");
            let fl = &mut p.flare_shape;
            for (i, label) in [
                "Flare, Bottom Left",
                "Flare, Bottom Right",
                "Flare, Top Left",
                "Flare, Top Right",
            ]
            .into_iter()
            .enumerate()
            {
                if i >= 2 && p.shape != StairShape::Straight {
                    break;
                }
                self.fields
                    .length_row(ui, label, FLARE_KEYS[i], &mut fl.corners[i]);
                fl.corners[i] = fl.corners[i].clamp(0.0, view::MAX_FLARE);
            }
            row(ui, "Soften Flare", |ui| {
                ui.add(egui::Slider::new(&mut fl.soften, 0.0..=1.0));
            });
            row(ui, "Flare Starts At", |ui| {
                let mut pct = if fl.start > 1e-9 {
                    fl.start * 100.0
                } else {
                    100.0
                };
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(10.0..=100.0)
                            .suffix("% of the run")
                            .speed(0.5),
                    )
                    .changed()
                {
                    fl.start = if pct >= 99.0 { 0.0 } else { pct / 100.0 };
                }
            });
            self.fields.length_row(
                ui,
                "Curve Bottom Treads",
                "curve_bottom",
                &mut fl.curve_bottom,
            );
            self.fields
                .length_row(ui, "Curve All Treads", "curve_all", &mut fl.curve_all);
            fl.curve_bottom = fl.curve_bottom.clamp(0.0, p.tread_depth.max(0.0));
            fl.curve_all = fl.curve_all.clamp(0.0, p.tread_depth.max(0.0));
            ui.weak("Treads curve down the stair by at most one tread depth. The Flare/Curve Stairs edit mode has handles for all of these.");
        }
        if matches!(p.shape, StairShape::Winder { .. }) {
            section(ui, "Winders");
            self.fields.length_row(
                ui,
                "Max Tread Contraction",
                "winder_contraction",
                &mut p.winder_contraction,
            );
            ui.weak("The narrowest a winder tread may get at the inside corner; 0 lets the points of the fan meet. 2\" leaves room for a wall under the stair.");
            p.winder_contraction = p.winder_contraction.max(0.0);
        }
        section(ui, "Runner");
        self.fields
            .length_row(ui, "Runner Width", "runner_w", &mut p.runner.width);
        p.runner.width = p.runner.width.max(0.0);
        ui.checkbox(&mut p.runner.tucked, "Runner tucked under the nosing");
        section(ui, "Walkline");
        ui.checkbox(
            &mut p.walkline.on,
            "Use walkline (tread depth is measured along it)",
        );
        self.fields.length_row(
            ui,
            "Distance From Edge",
            "walk_dist",
            &mut p.walkline.distance,
        );
        p.walkline.distance = p.walkline.distance.max(0.0);
        ui.checkbox(&mut p.walkline.show, "Show walkline in plan");
        section(ui, "Top Landing");
        ui.checkbox(&mut p.top_landing.nosing, "Nosing at top landing");
        ui.checkbox(
            &mut p.top_landing.riser_surface,
            "Riser surface at top landing",
        );
        section(ui, "Options");
        ui.checkbox(
            &mut p.railing_openings,
            "Automatic railing openings (a doorway is cut in a railing the stair meets)",
        );
        ui.checkbox(
            &mut p.allow_wrap,
            "Allow wrap (sections wrap around a deck or landing corner and share attributes)",
        );
    }

    fn stringers(&mut self, ui: &mut Ui) {
        let p = &mut self.draft.stair.params;
        section(ui, "Stringer Style");
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
        ui.weak("Closed: a full board whose top edge follows the nosing line. Open: a notched (cut) stringer with the steps cut out of the board. None: the treads span between walls.");
        self.fields
            .length_row(ui, "Stringer Depth", "stringer", &mut p.stringer_depth);
        self.fields.length_row(
            ui,
            "Stringer Thickness",
            "stringer_t",
            &mut p.stringers.thickness,
        );
        p.stringers.thickness = p.stringers.thickness.max(0.25);
        row(ui, "Middle Stringers", |ui| {
            let mut n = u32::from(p.stringers.centre);
            if ui.add(egui::DragValue::new(&mut n).range(0..=3)).changed() {
                p.stringers.centre = n as u8;
            }
        });
        ui.checkbox(
            &mut p.stringers.no_sides,
            "Leave out the side stringers (only the middle ones remain)",
        );
        ui.weak("A steel stringer with concrete treads: one middle stringer, no side stringers, Open Underneath.");
        section(ui, "Underneath");
        ui.checkbox(&mut p.stringers.open_underneath, "Open underneath");
        ui.add_enabled_ui(!p.stringers.open_underneath, |ui| {
            self.fields
                .length_row(ui, "Side Inset", "side_inset", &mut p.stringers.side_inset);
        });
        p.stringers.side_inset = p.stringers.side_inset.max(0.0);
        ui.weak("Off closes the underside with a soffit and a skirt along both sides, set in by the side inset.");
        ui.checkbox(
            &mut p.stringers.extend_top,
            "Extend stringer top (it carries on up to the floor above)",
        );
        ui.checkbox(
            &mut p.stringers.large_base,
            "Large stringer base (a deeper board at the bottom, for concrete stairs)",
        );
        self.fields
            .length_row(ui, "Slab Thickness", "slab_t", &mut p.slab_thickness);
    }

    /// The "Applies to" row of the Newels/Balusters and Rails tabs: both
    /// sides, or the left or right one alone.
    fn scope_row(&mut self, ui: &mut Ui) {
        section(ui, "Applies To");
        let before = self.scope;
        row(ui, "Side", |ui| {
            ui.radio_value(&mut self.scope, RailScope::Both, "Both sides");
            ui.radio_value(&mut self.scope, RailScope::Left, "Left side");
            ui.radio_value(&mut self.scope, RailScope::Right, "Right side");
        });
        let p = &mut self.draft.stair.params;
        // Leaving a side whose settings still equal the shared ones drops its
        // override again.
        if before != self.scope {
            let shared = p.railing;
            for o in [&mut p.left_railing, &mut p.right_railing] {
                if *o == Some(shared) {
                    *o = None;
                }
            }
        }
        match self.scope {
            RailScope::Both => {
                if p.left_railing.is_some() || p.right_railing.is_some() {
                    ui.horizontal(|ui| {
                        ui.weak("A side has settings of its own.");
                        if ui.small_button("Use these for both sides").clicked() {
                            p.left_railing = None;
                            p.right_railing = None;
                        }
                    });
                }
            }
            side => {
                let own = match side {
                    RailScope::Left => p.left_railing.is_some(),
                    _ => p.right_railing.is_some(),
                };
                if own {
                    ui.horizontal(|ui| {
                        ui.weak("This side has settings of its own.");
                        if ui.small_button("Same as both sides").clicked() {
                            match side {
                                RailScope::Left => p.left_railing = None,
                                _ => p.right_railing = None,
                            }
                        }
                    });
                } else {
                    ui.weak("Editing this side gives it settings of its own.");
                }
            }
        }
    }

    fn newels_balusters(&mut self, ui: &mut Ui) {
        self.scope_row(ui);
        let scope = self.scope;
        // The library items are stair-wide; the rest follows the scope.
        {
            let params = &mut self.draft.stair.params;
            library_row(
                ui,
                "Newel Library Item",
                "newel_item",
                &mut params.newel_item,
                view::PostKind::Newel,
            );
            library_row(
                ui,
                "Baluster Library Item",
                "baluster_item",
                &mut params.baluster_item,
                view::PostKind::Baluster,
            );
        }
        let r = scoped_railing(&mut self.draft.stair.params, scope);
        section(ui, "Newels");
        combo(
            ui,
            "Newel Type",
            "newel_type",
            &mut r.newel.profile,
            &PostProfile::ALL,
            PostProfile::name,
        );
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
        ui.checkbox(
            &mut r.newel.post_to_beam,
            "Post to beam (newels reach down through the floor structure)",
        );
        if r.newel.post_to_beam {
            self.fields
                .length_row(ui, "Post Below Foot", "beam_drop", &mut r.newel.beam_drop);
            r.newel.beam_drop = r.newel.beam_drop.max(0.0);
        }
        section(ui, "Balusters");
        combo(
            ui,
            "Baluster Type",
            "baluster_type",
            &mut r.baluster_profile,
            &PostProfile::ALL,
            PostProfile::name,
        );
        match &mut r.style {
            RailStyle::Balusters { spacing, size } => {
                self.fields
                    .length_row(ui, "Clear Spacing", "baluster_spacing", spacing);
                code_notice(
                    ui,
                    "IRC R312.1.3 baluster opening (sphere)",
                    spacing,
                    code::active().guard_sphere,
                    LimitKind::Max,
                );
                self.fields
                    .length_row(ui, "Baluster Size", "baluster_size", size);
            }
            _ => {
                ui.weak("The Rail Style tab has the infill; balusters are in use when it is set to Balusters.");
            }
        }
        ui.weak("Balusters stand on the treads, enough per tread to keep every opening within the clear spacing (4\" by code). A library item with a 3D model stands in for the built-in post at the same places and sizes.");
        section(ui, "Plan Display");
        let plan = &mut self.draft.stair.params.plan;
        ui.checkbox(&mut plan.draw_newels, "Draw newels in plan");
        ui.checkbox(&mut plan.draw_balusters, "Draw balusters in plan");
        ui.checkbox(&mut plan.draw_rails, "Draw rails in plan");
    }

    /// The infill between the rails (Chief's Rail Style tab).
    fn rail_style(&mut self, ui: &mut Ui) {
        self.scope_row(ui);
        let scope = self.scope;
        let r = scoped_railing(&mut self.draft.stair.params, scope);
        section(ui, "Rail Style");
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
            RailStyle::Cable { rows } => {
                row(ui, "Cable Rows", |ui| {
                    ui.add(egui::DragValue::new(rows).range(1..=12));
                });
            }
            RailStyle::Balusters { .. } => {
                ui.weak("Spacing, size and type of the balusters are on the Newels/Balusters tab.");
            }
            RailStyle::Panels => {
                ui.weak("Framed panels between the newels.");
            }
            RailStyle::Solid => {
                ui.weak("A solid infill between the newels.");
            }
            RailStyle::Glass => {
                ui.weak("Glass panels between the newels, held by the rails.");
            }
        }
        section(ui, "Half Wall");
        let mut half = r.half_wall.is_some();
        if ui
            .checkbox(&mut half, "Infill stands on a half wall")
            .changed()
        {
            r.half_wall = half.then_some(plan_stairs::GUARD_HEIGHT * 0.5);
        }
        if let Some(h) = &mut r.half_wall {
            self.fields
                .length_row(ui, "Half Wall Height", "half_wall_h", h);
            *h = h.max(0.0);
        }
    }

    fn rails(&mut self, ui: &mut Ui) {
        if self.draft.is_landing() {
            self.landing_edges(ui);
        }
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
        self.scope_row(ui);
        let scope = self.scope;
        let r = scoped_railing(&mut self.draft.stair.params, scope);
        section(ui, "Rails");
        self.fields
            .length_row(ui, "Guard Height", "guard", &mut r.height);
        code_notice(
            ui,
            "IRC R311.7.8.1 handrail height",
            &mut r.height,
            code::active().stair_guard_height,
            LimitKind::Min,
        );
        code_notice(
            ui,
            "IRC R311.7.8.1 handrail height",
            &mut r.height,
            code::active().handrail_max,
            LimitKind::Max,
        );
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
        if !self.draft.is_landing() {
            let p = &mut self.draft.stair.params;
            section(ui, "Handrail");
            ui.checkbox(&mut p.handrail, "Handrail on both sides");
            let h = &mut p.handrail_options;
            self.fields
                .length_row(ui, "Extend Top End", "rail_ext_top", &mut h.extend_top);
            self.fields.length_row(
                ui,
                "Extend Bottom End",
                "rail_ext_bottom",
                &mut h.extend_bottom,
            );
            h.extend_top = h.extend_top.max(0.0);
            h.extend_bottom = h.extend_bottom.max(0.0);
            ui.checkbox(&mut h.return_top, "Return to wall at the top");
            ui.checkbox(&mut h.return_bottom, "Return to wall at the bottom");
        }
        ui.weak("Railing: a guard with newels, balusters and a rail that follows the pitch. Handrail: a rail on the wall only, no guard. Half Wall: a cap rail on a solid panel. Wall: a full-height wall.");
    }

    /// The Selected Edge panel of a landing: what stands on each edge.
    fn landing_edges(&mut self, ui: &mut Ui) {
        let n = self.draft.footprint().len();
        let rails = &mut self.draft.stair.params.edge_rails;
        if rails.len() < n {
            rails.resize(n, EdgeRail::Automatic);
        }
        section(ui, "Landing Edges");
        for (i, rail) in rails.iter_mut().enumerate().take(n) {
            let label = format!("Edge {}", i + 1);
            combo(
                ui,
                &label,
                &format!("edge_rail_{i}"),
                rail,
                &EdgeRail::ALL,
                EdgeRail::name,
            );
        }
        let mut all = None;
        row(ui, "Apply to All Edges", |ui| {
            for e in EdgeRail::ALL {
                if ui.button(e.name()).clicked() {
                    all = Some(e);
                }
            }
        });
        if let Some(e) = all {
            rails.iter_mut().for_each(|r| *r = e);
        }
        // Back to the shorter list when nothing is forced.
        if rails.iter().all(|e| *e == EdgeRail::Automatic) {
            rails.clear();
        }
        ui.weak("Automatic puts a railing on the open sides and none where a stair or landing meets the edge; No Railing and Has Railing override it for one edge.");
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
            if self.draft.x.break_line && !self.draft.is_ramp() {
                row(ui, "Break Line At", |ui| {
                    let mut pct = self.draft.x.break_at * 100.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(view::MIN_BREAK_AT * 100.0..=view::MAX_BREAK_AT * 100.0)
                                .suffix("% of the run")
                                .speed(0.5),
                        )
                        .changed()
                    {
                        self.draft.x.break_at = pct / 100.0;
                    }
                });
                ui.weak("Where the floor above cuts the stair; Chief draws it two thirds of the way up.");
            }
            ui.checkbox(&mut self.draft.x.show_risers, "Show number of risers");
            let has_well = self.draft.x.stairwell_hole.is_some();
            ui.add_enabled(
                has_well,
                egui::Checkbox::new(
                    &mut self.draft.x.stairwell_guard,
                    "Guard railing around the stairwell opening",
                ),
            )
            .on_disabled_hover_text("Make a stairwell first (Auto Stairwell)");

            let p = &mut self.draft.stair.params;
            section(ui, "Plan Display");
            combo(
                ui,
                "Display on Floor Above",
                "plan_floor_above",
                &mut p.plan.floor_above,
                &DisplayRule::ALL,
                DisplayRule::name,
            );
            ui.checkbox(
                &mut self.draft.x.apply_display_all,
                "Apply to All Connected Sections",
            );
            combo(
                ui,
                "Floor Above Display",
                "plan_above_view",
                &mut p.plan.above_view,
                &ViewMode::ALL,
                ViewMode::name,
            );
            combo(
                ui,
                "Current Floor Display",
                "plan_beyond_view",
                &mut p.plan.beyond_view,
                &ViewMode::ALL,
                ViewMode::name,
            );
            ui.weak("Floor Above Display is the part of the stair before the break line as the floor above sees it; Current Floor Display is the part beyond the break line on the stair's own floor.");
            ui.checkbox(&mut p.plan.number_treads, "Number the treads");
            if matches!(p.shape, StairShape::Curved { .. }) || p.ramp_curve.is_some() {
                ui.checkbox(
                    &mut p.plan.show_arc_centers,
                    "Show arc centers and ends (a cross at the centre, lines to the ends)",
                );
            }
            section(ui, "Break Line");
            combo(
                ui,
                "Break Style",
                "plan_break_style",
                &mut p.plan.break_style,
                &BreakStyle::ALL,
                BreakStyle::name,
            );
            self.fields.degrees_row(
                ui,
                "Break Angle",
                "deg_break_angle",
                &mut p.plan.break_angle,
            );
            self.fields
                .length_row(ui, "Break Size", "break_size", &mut p.plan.break_size);
            self.fields
                .length_row(ui, "Gap After Break", "break_gap", &mut p.plan.break_gap);
            p.plan.break_size = p.plan.break_size.max(0.0);
            p.plan.break_gap = p.plan.break_gap.max(0.0);
            section(ui, "Arrow");
            combo(
                ui,
                "Arrow Style",
                "plan_arrow",
                &mut p.plan.arrow,
                &ArrowStyle::ALL,
                ArrowStyle::name,
            );
            self.fields
                .length_row(ui, "Arrow Size", "arrow_size", &mut p.plan.arrow_size);
            p.plan.arrow_size = p.plan.arrow_size.max(0.0);
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

    /// What the stair is made of: the parts, how many of each and their
    /// sizes, with the material of the Materials tab.
    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let rows = view::components(&self.draft);
        egui::Grid::new("stair_components")
            .num_columns(4)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                for h in ["Component", "Count", "Size", "Material"] {
                    ui.strong(h);
                }
                ui.end_row();
                for c in &rows {
                    ui.label(&c.name);
                    ui.label(c.count.to_string());
                    ui.label(&c.size);
                    ui.label(&c.material);
                    ui.end_row();
                }
            });
        ui.weak("Counts and sizes come from the solved layout; change them on the General, Style and Rails tabs.");
    }

    /// The row the stair has in the Stair Schedule: the same columns, from
    /// the solved layout.
    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Stair Schedule Row");
        let o = &self.draft;
        let p = &o.stair.params;
        let sol = solve(p);
        let ramp = o.is_ramp();
        let kind = match p.shape {
            StairShape::Straight | StairShape::Landing { .. } => "Straight",
            StairShape::LShaped { .. } => "L-Shaped",
            StairShape::UShaped { .. } => "U-Shaped",
            StairShape::Winder { .. } => "Winder",
            StairShape::Ramp { .. } => "Ramp",
            StairShape::Curved { .. } if p.spiral => "Spiral",
            StairShape::Curved { .. } => "Curved",
        };
        let steps = |v: String| if ramp { String::new() } else { v };
        let cells = [
            ("Type", kind.to_string()),
            ("Treads", steps(sol.treads.to_string())),
            ("Risers", steps(sol.risers.to_string())),
            ("Riser height", steps(format!("{:.3}\"", sol.riser_height))),
            ("Tread depth", steps(format!("{:.2}\"", sol.tread_depth))),
            ("Total rise", super::fmt_short(p.total_rise)),
            ("Total run", super::fmt_short(sol.total_run)),
            ("Width", super::fmt_short(p.width)),
            ("Headroom", super::fmt_short(p.headroom_min)),
        ];
        egui::Grid::new("stair_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for (h, _) in &cells {
                    ui.strong(*h);
                }
                ui.end_row();
                for (_, v) in &cells {
                    ui.label(v);
                }
                ui.end_row();
            });
        ui.weak("Listed in the Stair Schedule (Tools > Schedules > Stair); landings are not.");
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        ui.checkbox(&mut self.draft.x.show_label, "Show label in plan");
        row(ui, "Label Text", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.x.label).desired_width(180.0));
        });
    }
}

const FLARE_KEYS: [&str; 4] = ["flare_bl", "flare_br", "flare_tl", "flare_tr"];

/// A labelled drop-down over `all`, named by `name`.
fn combo<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    salt: &str,
    value: &mut T,
    all: &[T],
    name: impl Fn(T) -> &'static str,
) {
    row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(name(*value))
            .show_ui(ui, |ui| {
                for &v in all {
                    ui.selectable_value(value, v, name(v));
                }
            });
    });
}

/// The drop-down of library newels or balusters: the built-in post, then the
/// items of the library folder.
fn library_row(ui: &mut Ui, label: &str, salt: &str, current: &mut String, kind: view::PostKind) {
    let items = view::library_posts(kind);
    let shown = if current.is_empty() {
        "Built-in".to_string()
    } else {
        items
            .iter()
            .find(|i| i.0 == *current)
            .map_or_else(|| current.clone(), |i| i.1.clone())
    };
    row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_empty(), "Built-in")
                    .clicked()
                {
                    current.clear();
                }
                for (id, name) in &items {
                    if ui.selectable_label(current == id, name).clicked() {
                        *current = id.clone();
                    }
                }
            });
    });
    if items.is_empty() {
        ui.weak(format!(
            "The library has no items in its {} folder yet: import a 3D model there (User Catalog) to use it here.",
            kind.folder()
        ));
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
            "Stringers" => self.stringers(ui),
            "Newels/Balusters" => self.newels_balusters(ui),
            "Rail Style" => self.rail_style(ui),
            "Rails" => self.rails(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Materials" => self.materials(ui),
            "Components" => self.components(ui),
            "Schedule" => self.schedule(ui),
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
        rotation: 0.0,
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
                "Stringers",
                "Rails",
                "Newels/Balusters",
                "Rail Style",
                "Line Style",
                "Fill Style",
                "Materials",
                "Label",
                "Components",
                "Schedule"
            ]
        );
        let mut landing = o;
        landing.set_landing_depth(48.0);
        let d = StairDialog::new(landing);
        let names: Vec<_> = d.form.tabs().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "General",
                "Rails",
                "Newels/Balusters",
                "Rail Style",
                "Line Style",
                "Fill Style",
                "Materials",
                "Label",
                "Components"
            ]
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
        let mut railed = landing.clone();
        railed.stair.params.left_side = SideKind::Railing;
        objs.push(railed);
        objs.push(landing);
        let mut flared = o.clone();
        flared.stair.params.flare = 6.0;
        objs.push(flared);
        let mut bull = o.clone();
        bull.stair.params.bullnose = Bullnose::Both;
        bull.stair.params.left_side = SideKind::Handrail;
        bull.stair.params.right_railing = Some(RailingParams::default());
        objs.push(bull);
        objs.push(view::build(
            &cx.project,
            0,
            StairKind::Spiral,
            Turn::Left,
            Point::new(100.0, 100.0),
            None,
        ));
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
        // Typing a height or thickness turns its Auto Adjust off (ST21-4).
        d.draft_mut().stair.params.landing_auto_height = false;
        d.draft_mut().stair.params.landing_auto_thickness = false;
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

    fn page_texts(d: &mut StairDialog, tab: &str) -> Vec<String> {
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|x| texts(x, out)),
                _ => {}
            }
        }
        let i = d.form.tabs().iter().position(|t| t.name == tab).unwrap();
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, i));
        });
        let mut all = Vec::new();
        for c in &out.shapes {
            texts(&c.shape, &mut all);
        }
        all
    }

    #[test]
    fn a_ramp_specification_general_tab_has_the_options_and_tread_surface_rows() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        set_shape(d.draft_mut(), ShapeSel::Ramp);
        let t = page_texts(&mut d, "General");
        for want in [
            "Options",
            "Open underneath",
            "Tread Surface",
            "Has tread surface",
        ] {
            assert!(t.iter().any(|x| x.contains(want)), "{want} in {t:?}");
        }
        assert!(!t.iter().any(|x| x.contains("Max Thickness")), "{t:?}");
        d.draft_mut().stair.params.ramp.open_underneath = false;
        d.draft_mut().stair.params.ramp.has_surface = true;
        let t = page_texts(&mut d, "General");
        for want in ["Max Thickness", "Tread Overhang", "Tread Thickness"] {
            assert!(t.iter().any(|x| x.contains(want)), "{want} in {t:?}");
        }
        // A stair has none of these rows.
        let (_, o) = cx_with_stair();
        let mut s = StairDialog::new(o);
        let t = page_texts(&mut s, "General");
        assert!(!t.iter().any(|x| x.contains("Tread Surface")), "{t:?}");
    }

    #[test]
    fn the_schedule_tab_shows_the_row_the_stair_has_in_the_stair_schedule() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        let t = page_texts(&mut d, "Schedule");
        for want in [
            "Type",
            "Treads",
            "Risers",
            "Riser height",
            "Tread depth",
            "Total rise",
            "Total run",
            "Width",
            "Headroom",
            "Straight",
        ] {
            assert!(t.iter().any(|x| x == want), "{want} in {t:?}");
        }
        let sol = solve(&d.draft().stair.params);
        assert!(t.iter().any(|x| *x == sol.risers.to_string()), "{t:?}");
        assert!(t.iter().any(|x| *x == sol.treads.to_string()), "{t:?}");
    }

    #[test]
    fn the_rails_tabs_edit_one_side_without_touching_the_other() {
        let (_, o) = cx_with_stair();
        let mut d = StairDialog::new(o);
        let shared = d.draft().stair.params.railing;
        // Both sides: the shared settings.
        scoped_railing(&mut d.draft_mut().stair.params, RailScope::Both)
            .newel
            .size = 4.0;
        assert_eq!(d.draft().stair.params.railing.newel.size, 4.0);
        assert!(d.draft().stair.params.right_railing.is_none());
        // The right side gets a copy of its own the first time it is edited.
        {
            let p = &mut d.draft_mut().stair.params;
            scoped_railing(p, RailScope::Right).newel.size = 5.0;
            scoped_railing(p, RailScope::Right).style = RailStyle::Glass;
        }
        let p = &d.draft().stair.params;
        assert_eq!(p.railing.newel.size, 4.0, "the shared set is untouched");
        assert_eq!(p.left_railing, None);
        let own = p.right_railing.unwrap();
        assert_eq!((own.newel.size, own.style), (5.0, RailStyle::Glass));
        assert_ne!(own, shared);
        assert_eq!(p.railing_for(plan_stairs::RailSide::Left), p.railing);
        // The tabs draw for each scope, with the side's own note.
        d.form.scope = RailScope::Right;
        let t = page_texts(&mut d, "Newels/Balusters");
        assert!(
            t.iter()
                .any(|x| x.contains("This side has settings of its own")),
            "{t:?}"
        );
        d.form.scope = RailScope::Left;
        let t = page_texts(&mut d, "Rails");
        assert!(
            t.iter().any(|x| x.contains("Editing this side gives it")),
            "{t:?}"
        );
        // Leaving a side whose settings equal the shared ones drops its copy.
        d.form.scope = RailScope::Both;
        let t = page_texts(&mut d, "Rails");
        assert!(t.iter().any(|x| x.contains("A side has settings")), "{t:?}");
        // The Style tab offers the bullnose.
        let t = page_texts(&mut d, "Style");
        assert!(t.iter().any(|x| x == "Bullnose Bottom Tread"), "{t:?}");
    }

    #[test]
    fn a_handrail_side_is_offered_and_changes_the_3d_and_the_components() {
        let (_, o) = cx_with_stair();
        assert!(SideKind::ALL.contains(&SideKind::Handrail));
        let mut d = StairDialog::new(o);
        d.draft_mut().stair.params.left_side = SideKind::Handrail;
        let comps = view::components(d.draft());
        let h = comps.iter().find(|c| c.name == "Handrails").unwrap();
        assert_eq!((h.count, h.size.as_str()), (1, "on the left"));
        d.draft_mut().stair.params.handrail = true;
        let comps = view::components(d.draft());
        assert_eq!(
            comps.iter().find(|c| c.name == "Handrails").unwrap().count,
            2
        );
        // A bullnose and a flare show as their own lines.
        d.draft_mut().stair.params.bullnose = Bullnose::Left;
        d.draft_mut().stair.params.flare = 4.0;
        let comps = view::components(d.draft());
        assert!(comps
            .iter()
            .any(|c| c.name == "Bullnose bottom tread" && c.size == "Left End"));
        assert!(comps
            .iter()
            .any(|c| c.name == "Flared bottom tread" && c.size.contains("the other end")));
    }
}
