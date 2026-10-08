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
use super::code_notice::{code_notice, LimitKind};
use crate::editor::code;
use crate::editor::stairs_view::{self as view, first_flight_treads, StairObj};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::geometry::Point;
use plan_core::Id;
use plan_stairs::{
    solve, Bullnose, RailStyle, RailingParams, SideKind, StairParams, StairShape, StringerStyle,
    Turn,
};

const STAIR_TABS: &[Tab] = &[
    on("General"),
    on("Style"),
    on("Newels/Balusters"),
    on("Rails"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Components"),
    on("Schedule"),
    on("Label"),
];

const LANDING_TABS: &[Tab] = &[
    on("General"),
    on("Rails"),
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
        code_notice(ui, "IRC R311.7.6 landing width", &mut self.draft.stair.params.width, code::active().stair_width_min, LimitKind::Min);
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
        code_notice(ui, "IRC R311.7.1 stair width", &mut self.draft.stair.params.width, code::active().stair_width_min, LimitKind::Min);
        let ramp = self.draft.is_ramp();
        if !ramp {
            self.fields.length_row(
                ui,
                "Tread Depth",
                "tread",
                &mut self.draft.stair.params.tread_depth,
            );
            code_notice(ui, "IRC R311.7.5.2 tread depth", &mut self.draft.stair.params.tread_depth, code::active().stair_tread_min, LimitKind::Min);
            self.fields.length_row(
                ui,
                "Riser Height",
                "riser",
                &mut self.draft.stair.params.riser_height_target,
            );
            code_notice(ui, "IRC R311.7.5.1 riser height", &mut self.draft.stair.params.riser_height_target, code::active().stair_riser_max, LimitKind::Max);
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
        code_notice(ui, "IRC R311.7.2 headroom", &mut self.draft.stair.params.headroom_min, code::active().stair_headroom_min, LimitKind::Min);

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
        if matches!(
            p.shape,
            StairShape::Straight
                | StairShape::LShaped { .. }
                | StairShape::UShaped { .. }
                | StairShape::Winder { .. }
        ) {
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
        }
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
        let r = scoped_railing(&mut self.draft.stair.params, scope);
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
                code_notice(ui, "IRC R312.1.3 baluster opening (sphere)", spacing, code::active().guard_sphere, LimitKind::Max);
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
        self.scope_row(ui);
        let scope = self.scope;
        let r = scoped_railing(&mut self.draft.stair.params, scope);
        section(ui, "Rails");
        self.fields
            .length_row(ui, "Guard Height", "guard", &mut r.height);
        code_notice(ui, "IRC R311.7.8.1 handrail height", &mut r.height, code::active().stair_guard_height, LimitKind::Min);
        code_notice(ui, "IRC R311.7.8.1 handrail height", &mut r.height, code::active().handrail_max, LimitKind::Max);
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
        ui.weak("Railing: a guard with newels, balusters and a rail that follows the pitch. Handrail: a rail on the wall only, no guard. Half Wall: a cap rail on a solid panel. Wall: a full-height wall.");
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
                "Components",
                "Schedule",
                "Label"
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
                "Line Style",
                "Fill Style",
                "Materials",
                "Label"
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
