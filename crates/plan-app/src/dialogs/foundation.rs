//! Specification dialogs of the Slab tools and the platform hole tools:
//! Slab Specification (General, Fill Style, Line Style, Materials, Layer), Slab Hole,
//! Pad and Pier Specification (General, Materials, Layer) and Platform Hole
//! Specification. Opened by a double-click on the object; the dialog edits a
//! [`Draft`] clone that the tool stores on OK as one undo step.
//!
//! Also the Foundation Defaults and Build Foundation dialog body
//! ([`FoundationForm`], R-61, R-62, R-129..R-134; manual pp. 738-741): the
//! Foundation panel (type, Auto Rebuild, Hang Platform, S markers, slab,
//! stem walls, piers, Garage Options) and the Options panel (rebar, Foam
//! Seal, Termite Flashing), and the Slab and Pier/Pad specifications with
//! their Elevation Reference, Hole in Slab and Round Pier or Square Pad
//! choices (R-141, R-142).

use super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, WallTypeDialog,
    PV_ACCENT, PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::rooms_edit::{FoundationSpec, FoundationType};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::defaults::WallTypeDef;
use plan_core::elevation_ref::{ElevationBase, Resolver};
use plan_core::floors::{FoundationRooms, BASEMENT_LIVING_CLEAR_HEIGHT, BASEMENT_MIN_CLEAR_HEIGHT};
use plan_core::foundation::{
    bounds, Footing, FoundationLayer, FoundationRef, Pad, Pier, PierShape, PlatformHole,
    PlatformKind, Rebar, Slab, SlabHole,
};
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::LineStyle;
use plan_core::ResizeAbout;

const SLAB_TABS: &[Tab] = &[
    on("General"),
    on("Fill Style"),
    on("Line Style"),
    on("Materials"),
    on("Layer"),
];
// TODO parity: Chief's Label and Schedule tabs need label and schedule slots
// on Slab, SlabHole, Pad, Pier and PlatformHole (plan-core foundation.rs); a
// slab hole and a platform hole have no material of their own either.
const HOLE_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
const PAD_TABS: &[Tab] = &[on("General"), on("Materials"), on("Layer")];
const PLATFORM_TABS: &[Tab] = &[on("General")];

/// Materials the 3D concrete mesh understands.
pub const MATERIALS: [&str; 3] = ["Concrete", "Stone", "Brick"];
/// Plan fill patterns of a slab.
pub const PATTERNS: [&str; 5] = ["None", "Solid", "Hatch", "Cross Hatch", "Grid"];
const LINE_STYLES: [(LineStyle, &str); 4] = [
    (LineStyle::Solid, "Solid"),
    (LineStyle::Dashed, "Dashed"),
    (LineStyle::Dotted, "Dotted"),
    (LineStyle::DashDot, "Dash Dot"),
];

/// The object being edited.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    Slab(Slab),
    Hole(SlabHole),
    Pad(Pad),
    Pier(Pier),
    Platform(PlatformHole),
}

impl Draft {
    /// The draft of the object `r` on `layer`.
    pub fn of(layer: &FoundationLayer, r: FoundationRef) -> Option<Draft> {
        match r {
            FoundationRef::Slab(i) => layer.slab(i).cloned().map(Draft::Slab),
            FoundationRef::SlabHole(i) => layer.hole(i).cloned().map(Draft::Hole),
            FoundationRef::Pad(i) => layer.pad(i).cloned().map(Draft::Pad),
            FoundationRef::Pier(i) => layer.pier(i).cloned().map(Draft::Pier),
            FoundationRef::PlatformHole(i) => layer.platform_hole(i).cloned().map(Draft::Platform),
        }
    }

    /// The id of the object being edited.
    pub fn id(&self) -> u64 {
        match self {
            Draft::Slab(d) => d.id,
            Draft::Hole(d) => d.id,
            Draft::Pad(d) => d.id,
            Draft::Pier(d) => d.id,
            Draft::Platform(d) => d.id,
        }
    }

    /// Writes the draft back over the object with the same id; returns
    /// whether that object still exists. A slab turned into a hole (Hole in
    /// Slab), or a pier into a pad, is moved to the list of its new kind.
    pub fn apply(&self, layer: &mut FoundationLayer) -> bool {
        fn has<T>(v: &[T], id: u64, get: impl Fn(&T) -> u64) -> bool {
            v.iter().any(|x| get(x) == id)
        }
        fn take<T>(v: &mut Vec<T>, id: u64, get: impl Fn(&T) -> u64) {
            v.retain(|x| get(x) != id);
        }
        fn put<T: Clone>(v: &mut Vec<T>, id: u64, get: impl Fn(&T) -> u64, new: &T) {
            match v.iter_mut().find(|x| get(x) == id) {
                Some(x) => *x = new.clone(),
                None => v.push(new.clone()),
            }
        }
        let id = self.id();
        let existed = has(&layer.slabs, id, |s| s.id)
            || has(&layer.holes, id, |s| s.id)
            || has(&layer.pads, id, |s| s.id)
            || has(&layer.piers, id, |s| s.id)
            || has(&layer.platform_holes, id, |s| s.id);
        if !existed {
            return false;
        }
        match self {
            Draft::Slab(d) => {
                take(&mut layer.holes, id, |s| s.id);
                put(&mut layer.slabs, id, |s| s.id, d);
            }
            Draft::Hole(d) => {
                take(&mut layer.slabs, id, |s| s.id);
                put(&mut layer.holes, id, |s| s.id, d);
            }
            Draft::Pad(d) => {
                take(&mut layer.piers, id, |s| s.id);
                put(&mut layer.pads, id, |s| s.id, d);
            }
            Draft::Pier(d) => {
                take(&mut layer.pads, id, |s| s.id);
                put(&mut layer.piers, id, |s| s.id, d);
            }
            Draft::Platform(d) => put(&mut layer.platform_holes, id, |s| s.id, d),
        }
        true
    }

    /// The plan outline of a slab, hole or platform hole.
    pub fn outline(&self) -> Option<Vec<Point>> {
        match self {
            Draft::Slab(d) => Some(d.outline.clone()),
            Draft::Hole(d) => Some(d.outline.clone()),
            Draft::Platform(d) => Some(d.outline.clone()),
            Draft::Pad(_) | Draft::Pier(_) => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Draft::Slab(_) => "Slab Specification",
            Draft::Hole(_) => "Slab Hole Specification",
            Draft::Pad(_) => "Square Pad Specification",
            Draft::Pier(_) => "Round Pier Specification",
            Draft::Platform(_) => "Platform Hole Specification",
        }
    }

    fn tabs(&self) -> &'static [Tab] {
        match self {
            Draft::Slab(_) => SLAB_TABS,
            Draft::Hole(_) => HOLE_TABS,
            Draft::Pad(_) | Draft::Pier(_) => PAD_TABS,
            Draft::Platform(_) => PLATFORM_TABS,
        }
    }
}

pub struct FoundationDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: Draft,
    /// Slab holes inside the slab (for the volume line of the General page).
    inner_holes: Vec<SlabHole>,
    layers: Vec<String>,
    fields: Fields,
    /// What the Elevation Reference choices measure from.
    datums: Datums,
    /// The outlines of the other slabs, for Hole in Slab.
    other_slabs: Vec<Vec<Point>>,
    /// The object as it was opened, to turn back to when Hole in Slab or
    /// the pier type is switched twice.
    opened: Draft,
}

impl FoundationDialog {
    /// The dialog for object `r`, or `None` when it no longer exists.
    /// `layers` are the plan's layer names for the Layer page.
    pub fn new(layer: &FoundationLayer, r: FoundationRef, layers: Vec<String>) -> Option<Self> {
        let draft = Draft::of(layer, r)?;
        let inner_holes = match &draft {
            Draft::Slab(s) => layer.holes_in(s).into_iter().cloned().collect(),
            _ => Vec::new(),
        };
        let other_slabs = layer
            .slabs
            .iter()
            .filter(|s| s.id != draft.id())
            .map(|s| s.outline.clone())
            .collect();
        Some(Self {
            frame: SpecDialog::new(draft.title(), ("foundation_spec", draft.title())),
            form: Form {
                opened: draft.clone(),
                draft,
                inner_holes,
                layers,
                fields: Fields::default(),
                datums: Datums::floor_only(0.0),
                other_slabs,
            },
        })
    }

    /// The dialog measuring Top and Bottom from the references of `datums`.
    pub fn with_datums(mut self, datums: Datums) -> Self {
        self.form.datums = datums;
        self
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Draft {
        &self.form.draft
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Draft {
        &mut self.form.draft
    }

    #[cfg(test)]
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.form.draft.tabs().iter().map(|t| t.name).collect()
    }
}

/// The absolute elevations the Elevation Reference choices measure from at
/// the object being edited, in [`ElevationBase::ALL`] order, and the
/// elevation of the floor the object sits on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Datums {
    pub floor_elevation: f64,
    pub by_base: [f64; 6],
}

impl Datums {
    /// Datums when the plan is not at hand: the floor stands for every base
    /// but Absolute.
    pub fn floor_only(floor_elevation: f64) -> Self {
        let mut by_base = [floor_elevation; 6];
        by_base[0] = 0.0;
        Self {
            floor_elevation,
            by_base,
        }
    }

    fn of(&self, base: ElevationBase) -> f64 {
        ElevationBase::ALL
            .iter()
            .position(|b| *b == base)
            .map_or(self.floor_elevation, |i| self.by_base[i])
    }
}

/// The datums at plan point `at` of the active floor.
pub fn datums_at(cx: &EditorContext, at: Point) -> Datums {
    let surfaces = super::elevation_ref::AppSurfaces::new(&cx.project);
    let resolver = Resolver::new(&cx.project, cx.floor, &surfaces);
    let mut by_base = [0.0; 6];
    for (i, b) in ElevationBase::ALL.iter().enumerate() {
        by_base[i] = resolver.datum(at, *b);
    }
    Datums {
        floor_elevation: cx.floor().elevation,
        by_base,
    }
}

/// The datums for the object `r` of `layer` on the active floor.
pub fn datums_for(cx: &EditorContext, layer: &FoundationLayer, r: FoundationRef) -> Datums {
    let at = match r {
        FoundationRef::Pad(i) => layer.pad(i).map(|p| p.center),
        FoundationRef::Pier(i) => layer.pier(i).map(|p| p.center),
        _ => super::foundation::Draft::of(layer, r)
            .and_then(|d| d.outline().map(|o| bounds_center(&o))),
    };
    datums_at(cx, at.unwrap_or(Point::ZERO))
}

fn bounds_center(outline: &[Point]) -> Point {
    let (lo, hi) = bounds(outline);
    Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5)
}

/// Which panel of the Foundation Defaults and Build Foundation dialogs shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FoundationPanel {
    #[default]
    Foundation,
    Options,
}

/// What the Edit buttons of the Foundation panel open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WallEdit {
    SlabFooting,
    FoundationWall,
    GarageCurb,
}

/// The body of the Foundation Defaults and Build Foundation dialogs (manual
/// pp. 738-741): a Foundation panel and an Options panel over one
/// [`FoundationSpec`]. The OK and Cancel buttons are the caller's.
pub struct FoundationForm {
    pub spec: FoundationSpec,
    pub panel: FoundationPanel,
    fields: Fields,
    /// The plan's wall types and the name of the default foundation wall's:
    /// the Edit buttons open the Wall Type Definitions on it.
    types: Vec<WallTypeDef>,
    type_name: String,
    define: Option<(WallEdit, WallTypeDialog)>,
    edited: Vec<WallTypeDef>,
}

impl FoundationForm {
    pub fn new(spec: FoundationSpec) -> Self {
        Self {
            spec,
            panel: FoundationPanel::Foundation,
            fields: Fields::default(),
            types: Vec::new(),
            type_name: String::new(),
            define: None,
            edited: Vec::new(),
        }
    }

    /// The form with the plan's wall types, so the Edit buttons work.
    pub fn with_wall_types(mut self, types: Vec<WallTypeDef>, foundation_type: &str) -> Self {
        self.types = types;
        self.type_name = foundation_type.to_string();
        self
    }

    /// The wall types edited through the Edit buttons (for the plan's
    /// defaults).
    pub fn take_edited_types(&mut self) -> Vec<WallTypeDef> {
        std::mem::take(&mut self.edited)
    }

    pub fn any_invalid(&self) -> bool {
        self.fields.any_invalid()
    }

    /// Why the dialog cannot be accepted, if so.
    pub fn error(&self) -> Option<&'static str> {
        if self.fields.any_invalid() {
            Some("Fix the highlighted field")
        } else {
            self.spec.error()
        }
    }

    /// Drives the Wall Type Definitions window an Edit button opened.
    pub fn child(&mut self, ctx: &egui::Context) {
        let Some((_, d)) = self.define.as_mut() else {
            return;
        };
        match d.show(ctx) {
            Outcome::Open => {}
            Outcome::Ok => {
                if let Some((_, mut d)) = self.define.take() {
                    for t in d.changed_types() {
                        match self.types.iter_mut().find(|x| x.name == t.name) {
                            Some(slot) => *slot = t.clone(),
                            None => self.types.push(t.clone()),
                        }
                        match self.edited.iter_mut().find(|x| x.name == t.name) {
                            Some(slot) => *slot = t,
                            None => self.edited.push(t),
                        }
                    }
                }
            }
            Outcome::Cancel => self.define = None,
        }
    }

    /// Whether an Edit window is open (it owns Enter and Escape).
    pub fn editing(&self) -> bool {
        self.define.is_some()
    }

    #[cfg(test)]
    pub fn open_edit(&mut self) -> bool {
        start_edit(
            &self.types,
            &self.type_name,
            &mut self.define,
            WallEdit::FoundationWall,
        );
        self.define.is_some()
    }

    /// Draws the panel buttons and the chosen panel.
    pub fn show(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.panel, FoundationPanel::Foundation, "Foundation");
            ui.selectable_value(&mut self.panel, FoundationPanel::Options, "Options");
        });
        ui.separator();
        match self.panel {
            FoundationPanel::Foundation => self.foundation_panel(ui),
            FoundationPanel::Options => self.options_panel(ui),
        }
    }

    fn foundation_panel(&mut self, ui: &mut Ui) {
        let Self {
            spec,
            fields,
            types,
            type_name,
            define,
            ..
        } = self;
        let can_edit = !types.is_empty();
        ui.checkbox(
            &mut spec.settings.auto_rebuild,
            "Automatically Rebuild Foundation",
        )
        .on_hover_text(
            "Rebuild the foundation whenever Floor 1 changes in a way that affects it. \
             Walls and rooms on Floor 0 cannot then be edited by hand.",
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Foundation Type").strong());
        for t in FoundationType::ALL {
            ui.radio_value(&mut spec.kind, t, t.name());
        }
        let walls = spec.kind == FoundationType::WallsWithFootings;
        let mono = spec.kind == FoundationType::MonolithicSlab;
        let piers = spec.kind == FoundationType::Piers;
        ui.add_enabled(
            walls,
            egui::Checkbox::new(
                &mut spec.settings.hang_platform,
                "Hang 1st Floor Platform Inside Foundation Walls",
            ),
        )
        .on_hover_text(
            "Stem walls build up to the top of Floor 1's platform. Unchecked, they stop \
             under it and the platform bears on top.",
        );
        ui.add_enabled(
            !piers,
            egui::Checkbox::new(
                &mut spec.settings.s_markers,
                "Show \"S\" Markers on Step Foundation",
            ),
        );

        section(ui, "Slab");
        let shown = if type_name.is_empty() {
            "Foundation-8".to_string()
        } else {
            type_name.clone()
        };
        if mono {
            let mut edit = false;
            row(ui, "Default Slab Footing Wall Type", |ui| {
                ui.label(&shown);
                edit = ui
                    .add_enabled(can_edit, egui::Button::new("Edit Default Slab Footing"))
                    .clicked();
            });
            if edit {
                start_edit(types, type_name, define, WallEdit::SlabFooting);
            }
        }
        fields.length_row(ui, "Slab Thickness", "slab_t", &mut spec.slab_thickness);
        ui.add_enabled(
            walls,
            egui::Checkbox::new(
                &mut spec.settings.slab_at_stem_top,
                "Slab at top of Stem Wall",
            ),
        );
        if mono {
            fields.length_row(
                ui,
                "Stem Wall Height",
                "slab_stem",
                &mut spec.slab_stem_height,
            );
            fields.length_row(
                ui,
                "Chamfer Width",
                "chamfer_w",
                &mut spec.settings.chamfer_width,
            );
            fields.length_row(
                ui,
                "Chamfer Height",
                "chamfer_h",
                &mut spec.settings.chamfer_height,
            );
        }

        section(ui, "Stem Walls");
        let mut edit_wall = None;
        row(ui, "Default Foundation Wall Type", |ui| {
            ui.label(&shown);
            if ui
                .add_enabled(can_edit, egui::Button::new("Edit Default Foundation Wall"))
                .clicked()
            {
                edit_wall = Some(WallEdit::FoundationWall);
            }
            if mono
                && ui
                    .add_enabled(can_edit, egui::Button::new("Edit Garage Curb"))
                    .clicked()
            {
                edit_wall = Some(WallEdit::GarageCurb);
            }
        });
        if let Some(w) = edit_wall {
            start_edit(types, type_name, define, w);
        }
        if walls {
            fields.length_row(ui, "Stem Wall Height", "stem_h", &mut spec.stem_height);
        }
        if piers {
            fields.length_row(ui, "Grade Beam Height", "beam_h", &mut spec.beam_height);
        }
        fields.length_row(ui, "Minimum Height", "stem_min", &mut spec.min_stem_height);
        let min = spec.min_stem_height.max(0.0);
        let slab = spec.slab_thickness;
        let height = if walls {
            spec.stem_height.max(min)
        } else {
            min
        };
        let basement = height - slab;
        row(ui, "Basement Ceiling Height", |ui| {
            ui.label(fmt_ft_in(basement.max(0.0)));
        });
        if walls {
            ui.weak(basement_note(height, slab));
            row(ui, "Footing", |ui| {
                ui.checkbox(&mut spec.footing, "Footing under the walls")
            });
            if spec.footing {
                fields.length_row(ui, "Footing Width", "footing_w", &mut spec.footing_width);
                let (min_w, min_t) =
                    crate::editor::code::footing_limits(spec.stem_height.max(spec.min_stem_height));
                super::code_notice::code_notice(
                    ui,
                    "IRC R403.1.1 footing width (Table R403.1(1))",
                    &mut spec.footing_width,
                    min_w,
                    super::code_notice::LimitKind::Min,
                );
                fields.length_row(ui, "Footing Depth", "footing_d", &mut spec.footing_depth);
                super::code_notice::code_notice(
                    ui,
                    "IRC R403.1.4 / R403.1.1 footing thickness to the frost depth",
                    &mut spec.footing_depth,
                    min_t,
                    super::code_notice::LimitKind::Min,
                );
            }
            ui.checkbox(
                &mut spec.settings.vertical_step_footings,
                "Stepped stem walls have vertical footings",
            );
            row(ui, "Room", |ui| {
                ui.label("made inside the walls:");
            });
            for (value, label) in [
                (
                    FoundationRooms::Auto,
                    "Automatic (crawl space, basement from 4' clear, finished from 6')",
                ),
                (FoundationRooms::Basement, "Basement"),
                (FoundationRooms::CrawlSpace, "Crawl Space"),
                (FoundationRooms::None, "No room"),
            ] {
                ui.radio_value(&mut spec.rooms, value, label);
            }
        }

        if piers {
            section(ui, "Piers");
            fields.length_row(ui, "Width", "pier_w", &mut spec.settings.pier_width);
            fields.length_row(ui, "Depth", "pier_h", &mut spec.pier_height);
            fields.length_row(ui, "Maximum Separation", "pier_s", &mut spec.pier_spacing);
            row(ui, "Shape", |ui| {
                for s in PierShape::ALL {
                    ui.radio_value(&mut spec.settings.pier_shape, s, s.name());
                }
            });
            ui.weak("Piers at every corner and along the walls, a grade beam on top.");
        }

        section(ui, "Garage Options");
        ui.checkbox(&mut spec.garage_floor, "Build Garage Floor")
            .on_hover_text(
                "Garage and Slab rooms get a lowered slab of their own with stem walls \
                 (or curbs on a monolithic slab).",
            );
        ui.add_enabled_ui(spec.garage_floor, |ui| {
            fields.length_row(
                ui,
                "Garage Floor to Stem Wall Top",
                "garage_stem",
                &mut spec.settings.garage_floor_to_stem_top,
            );
            fields.length_row(
                ui,
                "Lower Garage Floor",
                "garage_lower",
                &mut spec.settings.lower_garage_floor,
            );
            fields.length_row(
                ui,
                "Minimum Garage Height",
                "garage_min",
                &mut spec.settings.min_garage_height,
            );
        });
    }

    fn options_panel(&mut self, ui: &mut Ui) {
        let kind = self.spec.kind;
        let spec = &mut self.spec;
        section(ui, "Rebar");
        egui::Grid::new("foundation_rebar")
            .num_columns(5)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("");
                for h in ["Bars per Course", "Course Spacing", "Rebar Size", "Overlap"] {
                    ui.label(egui::RichText::new(h).weak());
                }
                ui.end_row();
                let r = &mut spec.settings.rebar;
                let rows: [(&str, &mut Rebar, bool, bool); 5] = [
                    (
                        "Footing",
                        &mut r.footing,
                        kind == FoundationType::WallsWithFootings,
                        false,
                    ),
                    ("Wall Horizontal", &mut r.wall_horizontal, true, true),
                    ("Wall Vertical", &mut r.wall_vertical, true, true),
                    ("Pier", &mut r.pier, kind == FoundationType::Piers, false),
                    ("Slab", &mut r.slab, true, true),
                ];
                for (label, rebar, enabled, spaced) in rows {
                    ui.label(label);
                    ui.add_enabled(enabled, egui::DragValue::new(&mut rebar.bars).range(0..=20));
                    ui.add_enabled(
                        enabled && spaced,
                        egui::DragValue::new(&mut rebar.spacing)
                            .range(1.0..=240.0)
                            .suffix(" in"),
                    );
                    ui.add_enabled(
                        enabled,
                        egui::DragValue::new(&mut rebar.size)
                            .range(1..=18)
                            .suffix("/8"),
                    );
                    ui.add_enabled(
                        enabled,
                        egui::DragValue::new(&mut rebar.overlap)
                            .range(0.0..=100.0)
                            .suffix(" dia"),
                    );
                    ui.end_row();
                }
            });
        ui.checkbox(
            &mut spec.settings.rebar.use_mesh,
            "Use Mesh (instead of rebar in the slab)",
        );
        section(ui, "Foundation Options");
        ui.checkbox(&mut spec.settings.foam_seal, "Foam Seal");
        ui.checkbox(&mut spec.settings.termite_flashing, "Termite Flashing");
        ui.weak("Rebar, mesh, foam seal and termite flashing go to the Materials List only.");
    }
}

/// What the basement the stem wall height makes would be.
fn basement_note(height: f64, slab: f64) -> String {
    let clear = height - slab;
    if clear >= BASEMENT_MIN_CLEAR_HEIGHT {
        format!(
            "Clear height {}: a finished basement with a {} slab.",
            fmt_ft_in(clear),
            fmt_ft_in(slab)
        )
    } else if height >= BASEMENT_LIVING_CLEAR_HEIGHT {
        format!(
            "Clear height {}: a basement without finishes that counts as living area.",
            fmt_ft_in(height)
        )
    } else {
        format!("Clear height {}: a crawl space.", fmt_ft_in(height))
    }
}

/// Opens the Wall Type Definitions on the default foundation wall type.
fn start_edit(
    types: &[WallTypeDef],
    type_name: &str,
    define: &mut Option<(WallEdit, WallTypeDialog)>,
    what: WallEdit,
) {
    if types.is_empty() || define.is_some() {
        return;
    }
    let current = (!type_name.is_empty()).then_some(type_name);
    *define = Some((
        what,
        WallTypeDialog::new(types.to_vec(), current, ResizeAbout::default()),
    ));
}

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for m in MATERIALS {
                ui.selectable_value(value, m.to_string(), m);
            }
        });
}

fn footing_rows(fields: &mut Fields, ui: &mut Ui, footing: &mut Option<Footing>, side: bool) {
    let mut on = footing.is_some();
    row(ui, "Footing", |ui| ui.checkbox(&mut on, "Add footing"));
    if on != footing.is_some() {
        *footing = on.then(Footing::default);
    }
    if let Some(f) = footing {
        let width = if side {
            "Footing size"
        } else {
            "Footing width"
        };
        fields.length_row(ui, width, "footing_width", &mut f.width);
        fields.length_row(ui, "Footing depth", "footing_depth", &mut f.depth);
        super::code_notice::code_notice(
            ui,
            "IRC R403.1.1 footing thickness",
            &mut f.depth,
            crate::editor::code::active().footing_min_thickness,
            super::code_notice::LimitKind::Min,
        );
    }
}

fn materials_page(ui: &mut Ui, salt: &str, material: &mut String) {
    section(ui, "Materials");
    row(ui, "Material", |ui| material_combo(ui, salt, material));
}

fn layer_page(ui: &mut Ui, layers: &[String], layer: &mut String) {
    section(ui, "Layer");
    row(ui, "Layer", |ui| {
        super::select_layer::layer_field(
            ui,
            "foundation_layer",
            layer,
            layers.iter().map(String::as_str),
        );
    });
}

fn line_style_page(ui: &mut Ui, style: &mut LineStyle) {
    section(ui, "Line Style");
    row(ui, "Line style", |ui| {
        let current = LINE_STYLES
            .iter()
            .find(|(s, _)| s == style)
            .map_or("Solid", |(_, n)| *n);
        egui::ComboBox::from_id_salt("foundation_line_style")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (s, name) in LINE_STYLES {
                    ui.selectable_value(style, s, name);
                }
            });
    });
}

/// The Size and Position rows of the Pier/Pad Specification: Top Height and
/// Bottom Height measured from zero, and Width. `width`, `depth` (the pier's
/// height or the pad's thickness) and `top` (absolute); returns the new
/// `(width, depth, top)` when a row was edited.
fn size_and_position(
    ui: &mut Ui,
    fields: &mut Fields,
    width: f64,
    depth: f64,
    top: f64,
) -> Option<(f64, f64, f64)> {
    let mut out = None;
    let mut t = top;
    let mut b = top - depth;
    let mut w = width;
    fields.length_row(ui, "Top Height", "pier_top", &mut t);
    fields.length_row(ui, "Bottom Height", "pier_bottom", &mut b);
    fields.length_row(ui, "Width", "pier_width", &mut w);
    if (t - top).abs() > 1e-9 {
        // The top moves the whole object.
        out = Some((width, depth, t));
    } else if (b - (top - depth)).abs() > 1e-9 {
        out = Some((width, (top - b).max(0.0), top));
    } else if (w - width).abs() > 1e-9 {
        out = Some((w, depth, top));
    }
    out
}

/// Height rows measured from an Elevation Reference: `top` and `bottom` are
/// the object's absolute elevations; the user types them relative to the
/// reference datum. Returns the new absolute `(top, bottom)` when one was
/// edited.
fn reference_rows(
    ui: &mut Ui,
    fields: &mut Fields,
    datum: f64,
    top: f64,
    bottom: f64,
    keys: (&'static str, &'static str),
) -> Option<(f64, f64)> {
    let mut t = top - datum;
    let mut b = bottom - datum;
    let (ct, cb) = (t, b);
    fields.length_row(ui, "Top", keys.0, &mut t);
    fields.length_row(ui, "Bottom", keys.1, &mut b);
    if (t - ct).abs() > 1e-9 {
        // Moving the top moves the whole object.
        let d = t - ct;
        return Some((top + d, bottom + d));
    }
    if (b - cb).abs() > 1e-9 {
        // Moving the bottom changes the thickness.
        return Some((top, bottom + (b - cb)));
    }
    None
}

fn reference_combo(ui: &mut Ui, salt: &str, base: &mut ElevationBase) {
    row(ui, "Elevation Reference", |ui| {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(base.name())
            .show_ui(ui, |ui| {
                for b in ElevationBase::ALL {
                    ui.selectable_value(base, b, b.name());
                }
            });
    });
}

impl Form {
    /// Hole in Slab: turns the slab into a slab hole and back.
    fn set_hole(&mut self, hole: bool) {
        match (&self.draft, hole) {
            (Draft::Slab(s), true) => {
                self.draft = Draft::Hole(SlabHole {
                    id: s.id,
                    outline: s.outline.clone(),
                    with_footing: s.footing.is_some(),
                    layer: s.layer.clone(),
                    line_style: s.line_style,
                });
            }
            (Draft::Hole(h), false) => {
                self.draft = match &self.opened {
                    Draft::Slab(orig) => Draft::Slab(Slab {
                        outline: h.outline.clone(),
                        layer: h.layer.clone(),
                        line_style: h.line_style,
                        footing: h.with_footing.then(|| orig.footing.unwrap_or_default()),
                        ..orig.clone()
                    }),
                    _ => Draft::Slab(Slab {
                        id: h.id,
                        outline: h.outline.clone(),
                        footing: h.with_footing.then(Footing::default),
                        layer: h.layer.clone(),
                        line_style: h.line_style,
                        ..Slab::default()
                    }),
                };
            }
            _ => {}
        }
    }

    /// Round Pier or Square Pad: turns one into the other, keeping the
    /// width, the top and the depth.
    fn set_pier_shape(&mut self, round: bool) {
        match (&self.draft, round) {
            (Draft::Pad(p), true) => {
                self.draft = Draft::Pier(Pier {
                    id: p.id,
                    center: p.center,
                    diameter: p.size,
                    height: p.thickness,
                    elevation: p.elevation,
                    footing: None,
                    material: p.material.clone(),
                    layer: p.layer.clone(),
                });
            }
            (Draft::Pier(p), false) => {
                self.draft = Draft::Pad(Pad {
                    id: p.id,
                    center: p.center,
                    size: p.diameter,
                    thickness: p.height,
                    elevation: p.elevation,
                    material: p.material.clone(),
                    layer: p.layer.clone(),
                });
            }
            _ => {}
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let mut hole = None;
        let mut round = None;
        let floor_y = self.datums.floor_elevation;
        let datums = self.datums;
        let fields = &mut self.fields;
        match &mut self.draft {
            Draft::Slab(s) => {
                let mut is_hole = false;
                row(ui, "Hole in Slab", |ui| {
                    ui.checkbox(&mut is_hole, "Make this slab a hole in the slab it is in")
                });
                if is_hole {
                    hole = Some(true);
                }
                fields.length_row(ui, "Thickness", "thickness", &mut s.thickness);
                reference_combo(ui, "slab_reference", &mut s.elevation_base);
                let datum = datums.of(s.elevation_base);
                let top = floor_y + s.top_elevation;
                if let Some((t, b)) = reference_rows(
                    ui,
                    fields,
                    datum,
                    top,
                    top - s.thickness,
                    ("slab_top", "slab_bottom"),
                ) {
                    s.top_elevation = t - floor_y;
                    s.thickness = (t - b).max(0.0);
                }
                let mut has = s.footing.is_some();
                row(ui, "Footing", |ui| ui.checkbox(&mut has, "Has Footing"));
                if has != s.footing.is_some() {
                    s.footing = has.then(Footing::default);
                }
                if let Some(f) = &mut s.footing {
                    fields.length_row(ui, "Footing Height", "footing_depth", &mut f.depth);
                    fields.length_row(ui, "Footing Width", "footing_width", &mut f.width);
                    fields.length_row(
                        ui,
                        "Footing Offset",
                        "footing_offset",
                        &mut s.footing_offset,
                    );
                    super::code_notice::code_notice(
                        ui,
                        "IRC R403.1.1 footing thickness",
                        &mut f.depth,
                        crate::editor::code::active().footing_min_thickness,
                        super::code_notice::LimitKind::Min,
                    );
                }
                row(ui, "Material", |ui| {
                    material_combo(ui, "slab_material", &mut s.material)
                });
                ui.add_space(6.0);
                let extra: Vec<&SlabHole> = self.inner_holes.iter().collect();
                ui.weak(format!(
                    "Area {:.1} sq ft, perimeter {}, concrete {:.2} cu yd",
                    s.net_area(&extra) / 144.0,
                    fmt_ft_in(s.perimeter()),
                    s.concrete_cu_yd(&extra),
                ));
            }
            Draft::Hole(h) => {
                let mut is_hole = true;
                row(ui, "Hole in Slab", |ui| {
                    ui.checkbox(&mut is_hole, "A hole in the slab it is in")
                });
                if !is_hole {
                    hole = Some(false);
                }
                row(ui, "Footing", |ui| {
                    ui.checkbox(&mut h.with_footing, "Footing around the hole")
                });
                ui.add_space(6.0);
                ui.weak(format!("Area {:.1} sq ft", h.area() / 144.0));
                if !self
                    .other_slabs
                    .iter()
                    .any(|o| plan_core::foundation::hole_inside(&h.outline, o))
                {
                    ui.colored_label(
                        super::ERROR_RED,
                        "A slab hole must be contained within a larger slab.",
                    );
                }
            }
            Draft::Pad(p) => {
                let mut r = false;
                row(ui, "Type", |ui| {
                    ui.radio_value(&mut r, true, "Round Pier");
                    ui.radio_value(&mut r, false, "Square Pad");
                });
                if r {
                    round = Some(true);
                }
                if let Some((w, thickness, top)) =
                    size_and_position(ui, fields, p.size, p.thickness, p.elevation + floor_y)
                {
                    p.size = w;
                    p.thickness = thickness;
                    p.elevation = top - floor_y;
                }
                row(ui, "Material", |ui| {
                    material_combo(ui, "pad_material", &mut p.material)
                });
                ui.add_space(6.0);
                ui.weak(format!("Concrete {:.3} cu yd", p.concrete_cu_yd()));
            }
            Draft::Pier(p) => {
                let mut r = true;
                row(ui, "Type", |ui| {
                    ui.radio_value(&mut r, true, "Round Pier");
                    ui.radio_value(&mut r, false, "Square Pad");
                });
                if !r {
                    round = Some(false);
                }
                if let Some((w, h, top)) =
                    size_and_position(ui, fields, p.diameter, p.height, p.elevation + floor_y)
                {
                    p.diameter = w;
                    p.height = h;
                    p.elevation = top - floor_y;
                }
                footing_rows(fields, ui, &mut p.footing, true);
                row(ui, "Material", |ui| {
                    material_combo(ui, "pier_material", &mut p.material)
                });
                ui.add_space(6.0);
                ui.weak(format!("Concrete {:.3} cu yd", p.concrete_cu_yd()));
            }
            Draft::Platform(h) => {
                row(ui, "Hole in", |ui| {
                    ui.radio_value(&mut h.kind, PlatformKind::Floor, "Floor platform");
                    ui.radio_value(&mut h.kind, PlatformKind::Ceiling, "Ceiling platform");
                });
                ui.add_space(6.0);
                ui.weak(format!(
                    "Area {:.1} sq ft, drawn dashed on \"{}\"",
                    h.area() / 144.0,
                    h.layer()
                ));
            }
        }
        if let Some(h) = hole {
            self.set_hole(h);
        }
        if let Some(r) = round {
            self.set_pier_shape(r);
        }
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        let Draft::Slab(s) = &mut self.draft else {
            return;
        };
        row(ui, "Pattern", |ui| {
            egui::ComboBox::from_id_salt("slab_pattern")
                .selected_text(s.fill_pattern.clone())
                .show_ui(ui, |ui| {
                    for p in PATTERNS {
                        ui.selectable_value(&mut s.fill_pattern, p.to_string(), p);
                    }
                });
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut s.fill_color)
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        self.draft.tabs()
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft {
            Draft::Slab(s) => {
                if s.thickness <= 0.0 {
                    return Some("The thickness must be greater than zero".into());
                }
                if let Some(f) = s.footing {
                    if f.width <= 0.0 || f.depth <= 0.0 {
                        return Some("The footing needs a width and a depth".into());
                    }
                }
                if s.layer.trim().is_empty() {
                    return Some("Pick a layer".into());
                }
            }
            Draft::Pad(p) => {
                if p.size <= 0.0 || p.thickness <= 0.0 {
                    return Some("The pad needs a size and a thickness".into());
                }
            }
            Draft::Pier(p) => {
                if p.diameter <= 0.0 || p.height <= 0.0 {
                    return Some("The pier needs a diameter and a height".into());
                }
                if let Some(f) = p.footing {
                    if f.width < p.diameter || f.depth <= 0.0 {
                        return Some("The footing must be wider than the pier".into());
                    }
                }
            }
            Draft::Hole(h) => {
                if !self
                    .other_slabs
                    .iter()
                    .any(|o| plan_core::foundation::hole_inside(&h.outline, o))
                {
                    return Some("A slab hole must be contained within a larger slab".into());
                }
            }
            Draft::Platform(_) => {}
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let name = self.draft.tabs()[tab].name;
        match name {
            "General" => self.general(ui),
            "Fill Style" => self.fill_style(ui),
            "Line Style" => match &mut self.draft {
                Draft::Slab(s) => line_style_page(ui, &mut s.line_style),
                Draft::Hole(h) => line_style_page(ui, &mut h.line_style),
                _ => {}
            },
            "Materials" => match &mut self.draft {
                Draft::Slab(s) => materials_page(ui, "slab_material_tab", &mut s.material),
                Draft::Pad(p) => materials_page(ui, "pad_material_tab", &mut p.material),
                Draft::Pier(p) => materials_page(ui, "pier_material_tab", &mut p.material),
                _ => {}
            },
            "Layer" => {
                let layers = self.layers.clone();
                match &mut self.draft {
                    Draft::Slab(s) => layer_page(ui, &layers, &mut s.layer),
                    Draft::Hole(h) => layer_page(ui, &layers, &mut h.layer),
                    Draft::Pad(p) => layer_page(ui, &layers, &mut p.layer),
                    Draft::Pier(p) => layer_page(ui, &layers, &mut p.layer),
                    Draft::Platform(_) => {}
                }
            }
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            self.draft.title().trim_end_matches(" Specification"),
            11.0,
        );
        let area = Rect::from_min_max(
            Pos2::new(rect.min.x + 10.0, rect.min.y + 26.0),
            Pos2::new(rect.max.x - 10.0, rect.max.y - 10.0),
        );
        let ink = Stroke::new(1.5_f32, PV_INK);
        match &self.draft {
            Draft::Slab(s) => outline_preview(p, area, &s.outline, PV_WALL, ink),
            Draft::Hole(h) => outline_preview(p, area, &h.outline, Color32::TRANSPARENT, ink),
            Draft::Platform(h) => {
                outline_preview(
                    p,
                    area,
                    &h.outline,
                    Color32::TRANSPARENT,
                    Stroke::new(1.5_f32, PV_ACCENT),
                );
            }
            Draft::Pad(_) => {
                let r = Rect::from_center_size(area.center(), egui::vec2(60.0, 60.0));
                p.rect_filled(r, 0.0, PV_WALL);
                p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
                p.line_segment(
                    [r.left_top(), r.right_bottom()],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
                p.line_segment(
                    [r.right_top(), r.left_bottom()],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
            Draft::Pier(pier) => {
                let c = area.center();
                if pier.footing.is_some() {
                    let r = Rect::from_center_size(c, egui::vec2(70.0, 70.0));
                    p.rect_stroke(
                        r,
                        0.0,
                        Stroke::new(1.0_f32, PV_FAINT),
                        egui::StrokeKind::Inside,
                    );
                }
                p.circle(c, 24.0, PV_WALL, ink);
                p.line_segment(
                    [c - egui::vec2(24.0, 0.0), c + egui::vec2(24.0, 0.0)],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
                p.line_segment(
                    [c - egui::vec2(0.0, 24.0), c + egui::vec2(0.0, 24.0)],
                    Stroke::new(1.0_f32, PV_FAINT),
                );
            }
        }
        if let Draft::Slab(s) = &self.draft {
            pv_text(
                p,
                Pos2::new(rect.center().x, rect.max.y - 4.0),
                Align2::CENTER_BOTTOM,
                format!("{} thick", fmt_ft_in(s.thickness)),
                10.0,
            );
        }
    }
}

/// Draws `outline` scaled to fit `area`.
fn outline_preview(p: &Painter, area: Rect, outline: &[Point], fill: Color32, stroke: Stroke) {
    if outline.len() < 3 {
        return;
    }
    let (lo, hi) = bounds(outline);
    let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
    let k = (f64::from(area.width()) / w).min(f64::from(area.height()) / h) as f32;
    let origin = area.center() - egui::vec2(w as f32 * k * 0.5, -(h as f32) * k * 0.5);
    let to_screen = |q: &Point| {
        Pos2::new(
            origin.x + (q.x - lo.x) as f32 * k,
            origin.y - (q.y - lo.y) as f32 * k,
        )
    };
    let pts: Vec<Pos2> = outline.iter().map(to_screen).collect();
    p.add(egui::Shape::convex_polygon(pts.clone(), fill, stroke));
    if fill == Color32::TRANSPARENT {
        let mut closed = pts;
        closed.push(closed[0]);
        p.add(egui::Shape::line(closed, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::foundation::rect_outline;

    fn layer() -> FoundationLayer {
        FoundationLayer {
            slabs: vec![Slab::new(
                1,
                rect_outline(Point::ZERO, Point::new(240.0, 120.0)),
            )],
            holes: vec![SlabHole::new(
                2,
                rect_outline(Point::new(10.0, 10.0), Point::new(30.0, 30.0)),
                false,
            )],
            pads: vec![Pad::new(3, Point::new(300.0, 0.0))],
            piers: vec![Pier::new(4, Point::new(340.0, 0.0))],
            platform_holes: vec![PlatformHole::new(
                5,
                rect_outline(Point::new(50.0, 50.0), Point::new(80.0, 80.0)),
                PlatformKind::Floor,
            )],
        }
    }

    fn names() -> Vec<String> {
        vec!["Slabs".into(), "Piers/Pads".into()]
    }

    #[test]
    fn each_kind_has_its_own_pages() {
        let l = layer();
        let tabs = |r| FoundationDialog::new(&l, r, names()).unwrap().tab_names();
        assert_eq!(
            tabs(FoundationRef::Slab(1)),
            ["General", "Fill Style", "Line Style", "Materials", "Layer"]
        );
        assert_eq!(
            tabs(FoundationRef::SlabHole(2)),
            ["General", "Line Style", "Layer"]
        );
        assert_eq!(
            tabs(FoundationRef::Pad(3)),
            ["General", "Materials", "Layer"]
        );
        assert_eq!(
            tabs(FoundationRef::Pier(4)),
            ["General", "Materials", "Layer"]
        );
        assert_eq!(tabs(FoundationRef::PlatformHole(5)), ["General"]);
        assert!(FoundationDialog::new(&l, FoundationRef::Pad(99), names()).is_none());
    }

    #[test]
    fn applying_a_draft_replaces_only_that_object() {
        let mut l = layer();
        let mut d = FoundationDialog::new(&l, FoundationRef::Slab(1), names()).unwrap();
        if let Draft::Slab(s) = d.draft_mut() {
            s.thickness = 6.0;
            s.footing = Some(Footing {
                width: 20.0,
                depth: 10.0,
            });
            s.fill_pattern = "Grid".into();
        }
        assert!(d.draft().apply(&mut l));
        let s = l.slab(1).unwrap();
        assert_eq!(s.thickness, 6.0);
        assert_eq!(s.footing.unwrap().width, 20.0);
        assert_eq!(l.pad(3).unwrap().size, 24.0);
        // The draft of a deleted object does not apply.
        l.remove(FoundationRef::Slab(1));
        assert!(!d.draft().apply(&mut l));
    }

    #[test]
    fn the_form_rejects_nonsense() {
        let l = layer();
        let mut d = FoundationDialog::new(&l, FoundationRef::Pier(4), names()).unwrap();
        assert!(d.form.error().is_none());
        if let Draft::Pier(p) = d.draft_mut() {
            p.footing = Some(Footing {
                width: 6.0,
                depth: 8.0,
            });
        }
        assert!(d.form.error().unwrap().contains("wider"));
        if let Draft::Pier(p) = d.draft_mut() {
            p.footing = None;
            p.height = 0.0;
        }
        assert!(d.form.error().is_some());

        let mut s = FoundationDialog::new(&l, FoundationRef::Slab(1), names()).unwrap();
        if let Draft::Slab(slab) = s.draft_mut() {
            slab.thickness = 0.0;
        }
        assert!(s.form.error().is_some());
    }

    fn open(layer: &FoundationLayer, r: FoundationRef) -> FoundationDialog {
        FoundationDialog::new(layer, r, names()).unwrap()
    }

    fn draw(d: &mut FoundationDialog, tab: usize) {
        let ctx = egui::Context::default();
        let form = &mut d.form;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                SpecPages::page(form, ui, tab);
            });
        });
    }

    #[test]
    fn hole_in_slab_turns_a_slab_into_a_hole_and_back() {
        let mut l = layer();
        // A slab inside the big one.
        l.slabs.push(Slab::new(
            6,
            rect_outline(Point::new(100.0, 20.0), Point::new(140.0, 60.0)),
        ));
        let mut d = open(&l, FoundationRef::Slab(6));
        d.form.set_hole(true);
        assert!(matches!(d.draft(), Draft::Hole(h) if h.id == 6));
        assert!(d.form.error().is_none(), "it lies inside the bigger slab");
        let mut applied = l.clone();
        assert!(d.draft().apply(&mut applied));
        assert!(applied.slab(6).is_none() && applied.hole(6).is_some());
        // Switched back it is the slab it was.
        d.form.set_hole(false);
        assert!(matches!(d.draft(), Draft::Slab(s) if s.id == 6 && s.thickness == 4.0));
        let mut again = applied.clone();
        assert!(d.draft().apply(&mut again));
        assert!(again.slab(6).is_some() && again.hole(6).is_none());
        // A hole outside every other slab is refused.
        let mut far = open(&l, FoundationRef::SlabHole(2));
        assert!(far.form.error().is_none());
        if let Draft::Hole(h) = far.draft_mut() {
            h.outline = rect_outline(Point::new(900.0, 900.0), Point::new(940.0, 940.0));
        }
        assert!(far
            .form
            .error()
            .unwrap()
            .contains("contained within a larger slab"));
        for tab in 0..2 {
            draw(&mut far, tab);
        }
    }

    #[test]
    fn a_pier_and_a_pad_turn_into_each_other_keeping_size_top_and_depth() {
        let mut l = layer();
        l.pads[0].size = 18.0;
        l.pads[0].thickness = 10.0;
        l.pads[0].elevation = -6.0;
        let mut d = open(&l, FoundationRef::Pad(3));
        d.form.set_pier_shape(true);
        let Draft::Pier(p) = d.draft() else {
            panic!("a round pier");
        };
        assert_eq!(
            (p.id, p.diameter, p.height, p.elevation),
            (3, 18.0, 10.0, -6.0)
        );
        let mut applied = l.clone();
        assert!(d.draft().apply(&mut applied));
        assert!(applied.pad(3).is_none() && applied.pier(3).is_some());
        d.form.set_pier_shape(false);
        assert!(matches!(d.draft(), Draft::Pad(p) if p.size == 18.0 && p.thickness == 10.0));
        draw(&mut d, 0);
    }

    #[test]
    fn the_slab_top_and_bottom_follow_the_elevation_reference() {
        let l = layer();
        let mut d = open(&l, FoundationRef::Slab(1)).with_datums(Datums::floor_only(-48.0));
        // The floor stands at -48; the slab top at 0 above it, 4 thick.
        let Draft::Slab(s) = d.draft() else {
            panic!("a slab");
        };
        let top_abs = -48.0 + s.top_elevation;
        assert_eq!(top_abs, -48.0);
        // Absolute puts the datum at Z = 0: typing a Top of -40 moves the slab
        // 8 in up and keeps its thickness.
        let datums = Datums::floor_only(-48.0);
        assert_eq!(datums.of(ElevationBase::Absolute), 0.0);
        assert_eq!(datums.of(ElevationBase::FromFloor), -48.0);
        // Typing a Bottom changes the thickness and keeps the top.
        let (top, bottom) = (top_abs, top_abs - 4.0);
        assert_eq!((top, bottom), (-48.0, -52.0));
        draw(&mut d, 0);
        if let Draft::Slab(s) = d.draft_mut() {
            s.elevation_base = ElevationBase::Absolute;
            s.footing = Some(Footing::default());
            s.footing_offset = 3.0;
        }
        draw(&mut d, 0);
        let mut applied = l;
        assert!(d.draft().apply(&mut applied));
        let s = applied.slab(1).unwrap();
        assert_eq!(
            (s.elevation_base, s.footing_offset),
            (ElevationBase::Absolute, 3.0)
        );
    }

    #[test]
    fn size_and_position_edits_move_the_top_or_change_the_depth() {
        let ctx = egui::Context::default();
        let mut fields = Fields::default();
        let mut got = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                got = size_and_position(ui, &mut fields, 12.0, 36.0, -6.0);
            });
        });
        assert!(got.is_none(), "nothing typed, nothing changed");
    }
}
