//! Gable Line Specification (manual p. 864; RF-136) and the Roof Trim dialog
//! (manual pp. 831 to 834, 861 to 879; RF-15, RF-26).
//!
//! The Roof Trim dialog sets which trim parts Build Roof generates (rafter
//! tails, ridge caps, gutters, frieze, shadow boards, subfascia, lookouts),
//! their profiles and sizes and the soffit style; OK stores the options with
//! the roof and regenerates the trim as one undo step
//! (`tools::roof_trim::apply_options`).
//!
//! The Gable Line panel sets the pitch and overhang of the gable's two
//! planes; the Line, Line Style and Arrow panels describe how the line draws
//! in plan. The dialog edits a draft of a `tools::gable_line::GableLineRecord`
//! and writes it back as one undo step. Like the Roof Baseline Specification
//! it is hosted here: [`host_frame`] shows it every frame (`ToolSet::frame`
//! calls it), so [`open_spec`] works whichever tool is active.

// The entry points are called from the roof, menu and edit-toolbar owners'
// files once the hooks in docs/integration-queue.md are in; until then
// only the scenario tests use them.
#![allow(dead_code)]

use super::roof_baseline::pitch_drag;
use super::{on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use super::{PV_ACCENT, PV_FAINT, PV_INK};
use crate::editor::EditorContext;
use crate::tools::gable_line::{self as gl, GableLineRecord};
use crate::tools::roof_trim as trim;
use eframe::egui::{self, Painter, Pos2, Rect, Stroke, Ui};
use plan_3d::{RafterTailRecipe, RoofTrimOptions, SoffitStyle, TrimSpec};
use plan_core::layers::LineStyle;
use plan_core::Id;
use std::cell::RefCell;

const TABS: &[Tab] = &[on("Gable Line"), on("Line"), on("Line Style"), on("Arrow")];

/// Largest overhang the dialog accepts, inches.
const MAX_OVERHANG: f64 = 120.0;

/// The dialog for one Gable/Roof Line.
pub struct GableLineDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    rec: GableLineRecord,
    fields: Fields,
}

impl GableLineDialog {
    fn new(rec: GableLineRecord) -> Self {
        Self {
            frame: SpecDialog::new("Gable Line Specification", "gable_line"),
            form: Form {
                rec,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn id(&self) -> Id {
        self.form.rec.id
    }

    /// The draft the dialog edits.
    pub fn record(&self) -> &GableLineRecord {
        &self.form.rec
    }

    pub fn record_mut(&mut self) -> &mut GableLineRecord {
        &mut self.form.rec
    }
}

fn dash_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash Dot",
    }
}

impl Form {
    fn gable_line(&mut self, ui: &mut Ui) {
        section(ui, "Gable Line");
        row(ui, "Pitch", |ui| pitch_drag(ui, &mut self.rec.pitch));
        self.fields
            .length_row(ui, "Overhang", "gl_overhang", &mut self.rec.overhang);
        ui.weak("Both planes of the gable use this pitch and overhang.");
        ui.weak("The gable is built at the next Build Roof and kept until the line is deleted.");
    }

    fn line(&self, ui: &mut Ui) {
        section(ui, "Line");
        row(ui, "Length", |ui| {
            ui.label(super::fmt_short(self.rec.line().length()))
        });
        row(ui, "Angle", |ui| {
            ui.label(format!(
                "{:.2}\u{b0}",
                self.rec.b.sub(self.rec.a).angle().to_degrees()
            ))
        });
        row(ui, "Gable Width", |ui| {
            ui.label(super::fmt_short(
                self.rec.line().length() + 2.0 * self.rec.overhang,
            ))
        });
        if !self.rec.openings.is_empty() {
            row(ui, "Over Openings", |ui| {
                ui.label(format!("{}", self.rec.openings.len()))
            });
        }
    }

    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let mut dash = self.rec.dash.unwrap_or(LineStyle::Dashed);
        row(ui, "Line Style", |ui| {
            egui::ComboBox::from_id_salt("gl_dash")
                .selected_text(dash_name(dash))
                .show_ui(ui, |ui| {
                    for s in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        ui.selectable_value(&mut dash, s, dash_name(s));
                    }
                });
        });
        self.rec.dash = (dash != LineStyle::Dashed).then_some(dash);
        let mut mm = f64::from(self.rec.weight.unwrap_or(25)) / 100.0;
        row(ui, "Line Weight", |ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut mm)
                        .range(0.05..=5.0)
                        .speed(0.01)
                        .suffix(" mm"),
                )
                .changed()
            {
                self.rec.weight = Some((mm * 100.0).round() as u32);
            }
        });
        let mut c = self.rec.color.unwrap_or([150, 90, 40]);
        row(ui, "Color", |ui| {
            if ui.color_edit_button_srgb(&mut c).changed() {
                self.rec.color = Some(c);
            }
        });
    }

    fn arrow(&mut self, ui: &mut Ui) {
        section(ui, "Arrow");
        ui.checkbox(&mut self.rec.arrow_start, "Arrowhead at the start");
        ui.checkbox(&mut self.rec.arrow_end, "Arrowhead at the end");
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter a valid length".into());
        }
        if self.rec.pitch <= 0.0 {
            return Some("The pitch must be above zero".into());
        }
        if self.rec.overhang < 0.0 || self.rec.overhang > MAX_OVERHANG {
            return Some("The overhang must be between 0 and 10 feet".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.gable_line(ui),
            1 => self.line(ui),
            2 => self.line_style(ui),
            _ => self.arrow(ui),
        }
    }

    /// The gable seen from the side: two planes of the pitch meeting at the
    /// ridge over the line, the overhang past each end.
    fn preview(&self, p: &Painter, area: Rect) {
        let inner = area.shrink(20.0);
        let len = self.rec.line().length().max(1.0);
        let total = len + 2.0 * self.rec.overhang;
        let rise = len * 0.5 * self.rec.pitch / 12.0;
        let k = (f64::from(inner.width()) / total).min(f64::from(inner.height()) / rise.max(1.0));
        let base = inner.bottom();
        let x = |d: f64| inner.center().x + (d * k) as f32;
        let (l, r, top) = (
            Pos2::new(x(-total * 0.5), base),
            Pos2::new(x(total * 0.5), base),
            Pos2::new(x(0.0), base - (rise * k) as f32),
        );
        p.add(egui::Shape::line(
            vec![l, top, r],
            Stroke::new(2.0_f32, PV_INK),
        ));
        p.line_segment(
            [
                Pos2::new(x(-len * 0.5), base),
                Pos2::new(x(len * 0.5), base),
            ],
            Stroke::new(3.0_f32, PV_ACCENT),
        );
        p.line_segment([l, r], Stroke::new(1.0_f32, PV_FAINT));
    }
}

thread_local! {
    static HOST: RefCell<Option<GableLineDialog>> = const { RefCell::new(None) };
}

/// Opens the Gable Line Specification of line `id` of the active floor.
/// False when `id` is not a Gable/Roof Line.
pub fn open_spec(cx: &EditorContext, id: Id) -> bool {
    let Some(rec) = gl::line(cx.floor(), id) else {
        return false;
    };
    HOST.with(|h| *h.borrow_mut() = Some(GableLineDialog::new(rec)));
    true
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open Gable Line Specification.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut GableLineDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open Gable Line Specification and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> bool {
    let Some(d) = HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    gl::set_spec(cx, &d.form.rec)
}

/// Shows the open dialogs once a frame and applies their OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    if let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
            Outcome::Cancel => {}
            Outcome::Ok => {
                gl::set_spec(cx, &d.form.rec);
            }
        }
    }
    if let Some(mut d) = TRIM_HOST.with(|h| h.borrow_mut().take()) {
        match d.show(ctx) {
            Outcome::Open => TRIM_HOST.with(|h| *h.borrow_mut() = Some(d)),
            Outcome::Cancel => {}
            Outcome::Ok => {
                trim::apply_options(cx, &d.form.opts);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Roof Trim
// ---------------------------------------------------------------------------

const TRIM_TABS: &[Tab] = &[
    on("Rafter Tails"),
    on("Ridge Caps"),
    on("Gutters"),
    on("Frieze"),
    on("Shadow Boards"),
    on("Subfascia"),
    on("Lookouts"),
    on("Soffits"),
];

/// The Roof Trim dialog.
pub struct RoofTrimDialog {
    frame: SpecDialog,
    form: TrimForm,
}

struct TrimForm {
    opts: RoofTrimOptions,
    /// Names of the profiles to pick from: the built-in ones and the plan's.
    profiles: Vec<String>,
    fields: Fields,
}

impl RoofTrimDialog {
    fn new(opts: RoofTrimOptions, profiles: Vec<String>) -> Self {
        Self {
            frame: SpecDialog::new("Roof Trim", "roof_trim"),
            form: TrimForm {
                opts,
                profiles,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    /// The draft the dialog edits.
    pub fn options(&self) -> &RoofTrimOptions {
        &self.form.opts
    }

    pub fn options_mut(&mut self) -> &mut RoofTrimOptions {
        &mut self.form.opts
    }
}

/// A profile pick box: "(plain board)" or a library profile.
fn profile_row(ui: &mut Ui, id: &str, names: &[String], value: &mut String) {
    row(ui, "Profile", |ui| {
        let shown = if value.is_empty() {
            "(plain board)".to_string()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .selected_text(shown)
            .width(210.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(value, String::new(), "(plain board)");
                for n in names {
                    ui.selectable_value(value, n.clone(), n);
                }
            });
    });
}

impl TrimForm {
    /// The common part of a trim panel: switch, profile, size.
    fn spec(
        ui: &mut Ui,
        fields: &mut Fields,
        names: &[String],
        s: &mut TrimSpec,
        key: (&'static str, &'static str, &'static str),
        what: &str,
    ) {
        ui.checkbox(&mut s.enabled, format!("Generate {what} at Build Roof"));
        ui.add_enabled_ui(s.enabled, |ui| {
            profile_row(ui, key.0, names, &mut s.profile);
            fields.length_row(ui, "Width (projection)", key.1, &mut s.width);
            fields.length_row(ui, "Height", key.2, &mut s.height);
        });
    }

    fn rafter_tails(&mut self, ui: &mut Ui) {
        section(ui, "Rafter Tails");
        let t = &mut self.opts.rafter_tails;
        ui.checkbox(&mut t.enabled, "Generate rafter tails at Build Roof");
        ui.add_enabled_ui(t.enabled, |ui| {
            row(ui, "Recipe", |ui| {
                egui::ComboBox::from_id_salt("rt_recipe")
                    .selected_text(match t.recipe {
                        RafterTailRecipe::Exposed => "Exposed",
                        RafterTailRecipe::Hidden => "Hidden",
                        RafterTailRecipe::PartiallyExposed => "Partially Exposed",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut t.recipe, RafterTailRecipe::Exposed, "Exposed");
                        ui.selectable_value(&mut t.recipe, RafterTailRecipe::Hidden, "Hidden");
                        ui.selectable_value(
                            &mut t.recipe,
                            RafterTailRecipe::PartiallyExposed,
                            "Partially Exposed",
                        );
                    });
            });
            profile_row(ui, "rt_profile", &self.profiles, &mut t.profile);
            ui.checkbox(&mut t.stretch_to_fit, "Stretch to Fit Rafter");
            ui.add_enabled_ui(!t.stretch_to_fit, |ui| {
                self.fields.length_row(ui, "Width", "rt_w", &mut t.width);
                self.fields.length_row(ui, "Height", "rt_h", &mut t.height);
            });
            self.fields.length_row(
                ui,
                "Extend Past Subfascia",
                "rt_ext",
                &mut t.extend_past_subfascia,
            );
            ui.add_enabled_ui(t.recipe == RafterTailRecipe::PartiallyExposed, |ui| {
                self.fields
                    .length_row(ui, "Exposed Length", "rt_exp", &mut t.exposed_length);
            });
        });
    }

    fn ridge_caps(&mut self, ui: &mut Ui) {
        section(ui, "Ridge Caps");
        let c = &mut self.opts.ridge_caps;
        Self::spec(
            ui,
            &mut self.fields,
            &self.profiles,
            &mut c.spec,
            ("rc_profile", "rc_w", "rc_h"),
            "ridge caps",
        );
        ui.add_enabled_ui(c.spec.enabled, |ui| {
            ui.checkbox(&mut c.bend_to_pitch, "Bend to Roof Pitch");
        });
        ui.weak("Width is the whole cap across both planes. The Ridge Cap setting of a single edge (Automatic, On, Off) is in the Roof Plane Specification.");
    }

    fn soffits(&mut self, ui: &mut Ui) {
        section(ui, "Soffits");
        row(ui, "Eave", |ui| {
            ui.radio_value(&mut self.opts.soffit, SoffitStyle::Boxed, "Boxed");
            ui.radio_value(&mut self.opts.soffit, SoffitStyle::Flush, "Flush");
        });
        ui.add_enabled_ui(self.opts.soffit == SoffitStyle::Flush, |ui| {
            ui.checkbox(&mut self.opts.higher_eaves_boxed, "Higher Eaves Boxed");
        });
        ui.checkbox(
            &mut self.opts.trim_framing_to_soffits,
            "Trim Framing To Soffits",
        );
        ui.weak("Boxed eaves have a horizontal soffit under the fascia; flush eaves follow the rafters.");
    }
}

impl SpecPages for TrimForm {
    fn tabs(&self) -> &'static [Tab] {
        TRIM_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter a valid length".into());
        }
        let o = &self.opts;
        let sizes = [
            &o.gutters,
            &o.frieze,
            &o.shadow_boards,
            &o.subfascia,
            &o.lookouts,
            &o.ridge_caps.spec,
        ];
        if sizes
            .iter()
            .any(|s| s.enabled && (s.width <= 0.0 || s.height <= 0.0))
        {
            return Some("Trim width and height must be above zero".into());
        }
        if o.lookouts.enabled && o.lookout_spacing < 4.0 {
            return Some("Lookouts need a spacing of at least 4 inches".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.rafter_tails(ui),
            1 => self.ridge_caps(ui),
            2 => {
                section(ui, "Gutters");
                let o = &mut self.opts;
                Self::spec(
                    ui,
                    &mut self.fields,
                    &self.profiles,
                    &mut o.gutters,
                    ("g_p", "g_w", "g_h"),
                    "gutters",
                );
                ui.weak("Gutters hang on eaves that do not slope.");
            }
            3 => {
                section(ui, "Frieze");
                let o = &mut self.opts;
                Self::spec(
                    ui,
                    &mut self.fields,
                    &self.profiles,
                    &mut o.frieze,
                    ("f_p", "f_w", "f_h"),
                    "frieze moldings",
                );
                ui.weak("Under the eaves and under the gable overhangs, against the wall.");
            }
            4 => {
                section(ui, "Shadow Boards");
                let o = &mut self.opts;
                Self::spec(
                    ui,
                    &mut self.fields,
                    &self.profiles,
                    &mut o.shadow_boards,
                    ("s_p", "s_w", "s_h"),
                    "shadow boards",
                );
                ui.weak("On the face of the fascia, eaves and rakes.");
            }
            5 => {
                section(ui, "Subfascia");
                let o = &mut self.opts;
                Self::spec(
                    ui,
                    &mut self.fields,
                    &self.profiles,
                    &mut o.subfascia,
                    ("sf_p", "sf_w", "sf_h"),
                    "subfascia",
                );
            }
            6 => {
                section(ui, "Lookouts");
                let o = &mut self.opts;
                Self::spec(
                    ui,
                    &mut self.fields,
                    &self.profiles,
                    &mut o.lookouts,
                    ("l_p", "l_w", "l_h"),
                    "lookouts",
                );
                ui.add_enabled_ui(o.lookouts.enabled, |ui| {
                    self.fields
                        .length_row(ui, "Spacing", "l_sp", &mut o.lookout_spacing);
                });
            }
            _ => self.soffits(ui),
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // The eave in section: wall, rafter, fascia, the parts that are on.
        let c = area.center();
        let wall = Pos2::new(c.x - 40.0, c.y + 40.0);
        let tip = Pos2::new(c.x + 50.0, c.y + 28.0);
        let ridge = Pos2::new(c.x - 40.0, c.y - 30.0);
        p.line_segment(
            [wall, Pos2::new(wall.x, wall.y + 40.0)],
            Stroke::new(2.0_f32, PV_INK),
        );
        p.line_segment([ridge, tip], Stroke::new(2.0_f32, PV_INK));
        p.line_segment(
            [tip, Pos2::new(tip.x, tip.y + 12.0)],
            Stroke::new(3.0_f32, PV_ACCENT),
        );
        let on = |b: bool| if b { PV_ACCENT } else { PV_FAINT };
        let o = &self.opts;
        p.circle_filled(
            Pos2::new(ridge.x, ridge.y - 3.0),
            3.0,
            on(o.ridge_caps.spec.enabled),
        );
        p.circle_filled(
            Pos2::new(tip.x + 4.0, tip.y + 16.0),
            3.0,
            on(o.gutters.enabled),
        );
        p.circle_filled(
            Pos2::new(wall.x + 4.0, wall.y + 4.0),
            3.0,
            on(o.frieze.enabled),
        );
        p.circle_filled(
            Pos2::new(tip.x - 8.0, tip.y + 8.0),
            3.0,
            on(o.rafter_tails.enabled),
        );
    }
}

thread_local! {
    static TRIM_HOST: RefCell<Option<RoofTrimDialog>> = const { RefCell::new(None) };
}

/// Opens the Roof Trim dialog for the active floor.
pub fn open_trim(cx: &EditorContext) {
    let opts = trim::options(cx.floor());
    let own = plan_core::details::DetailsLayer::load(cx.floor()).profiles;
    let mut names: Vec<String> = plan_core::moldings::builtin_profiles()
        .into_iter()
        .chain(own)
        .map(|p| p.name)
        .collect();
    names.dedup();
    TRIM_HOST.with(|h| *h.borrow_mut() = Some(RoofTrimDialog::new(opts, names)));
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn trim_dialog_open() -> bool {
    TRIM_HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open Roof Trim dialog.
#[cfg(test)]
pub fn with_trim_dialog<R>(f: impl FnOnce(&mut RoofTrimDialog) -> R) -> Option<R> {
    TRIM_HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open Roof Trim dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_trim(cx: &mut EditorContext) -> bool {
    let Some(d) = TRIM_HOST.with(|h| h.borrow_mut().take()) else {
        return false;
    };
    trim::apply_options(cx, &d.form.opts);
    true
}
