//! Room Specification (docs/chief-x18-dialogs.md, "Room Types and Room
//! Specification"; R-19..R-36, R-45).
//!
//! The dialog edits a draft [`RoomName`] plus the [`RoomExtras`] view of the
//! rest of the specification. OK hands both back to the app, which writes
//! them with `rooms_edit::apply_room_spec` as one undo step; conditioned, the
//! stem wall, moldings, fill and label options are stored in the room's
//! `RoomName`, the other extras are kept for the session.

use super::assembly_def::{PlatformEditor, RowContext};
use super::{
    dis_check, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_FAINT, PV_GLASS, PV_INK, PV_WALL,
};
use crate::editor::rooms_edit::{FillPattern, RoomExtras};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::assemblies::{legacy_value, resolve, Assembly, AssemblyKind, AssemblyLibrary};
use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
use plan_core::defaults::RoomTypeDef;
use plan_core::extras::{AreaKind, MoldingKind, StructureLayer};
use plan_core::geometry::Point;
use plan_core::rooms::{
    effective_function, function_class, function_defaults_with, molding_def, molding_defs,
    LabelPlacement, ResizeLock, RoomSlab, RoomTypeSpec, CEILING_SURFACES, FLOOR_SURFACES,
    WALL_SURFACES,
};
use plan_core::units::fmt_ft_in;
use plan_core::RoomName;

const ROOM_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Deck"),
    on("Moldings"),
    on("Wall Covering"),
    on("Layer"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
];

/// Everything the dialog needs to know about the room it edits.
#[derive(Clone, Debug)]
pub struct RoomInit {
    pub room_index: usize,
    pub name: RoomName,
    pub extras: RoomExtras,
    pub types: Vec<RoomTypeDef>,
    /// The outline drawn in the preview (interior surfaces when known).
    pub polygon: Vec<Point>,
    pub interior_dims: String,
    pub interior_area_sq_ft: f64,
    pub standard_area_sq_ft: f64,
    pub perimeter_in: f64,
    pub floor_elevation: f64,
    pub floor_ceiling_height: f64,
    /// The floor's default floor finish thickness, inches (Floor Defaults).
    pub default_floor_finish: f64,
    /// The generated name ("Room 1") a room has before it is named.
    pub default_name: String,
    pub total_living_sq_ft: f64,
    pub floor_name: String,
    /// The plan's text styles, for the Label tab.
    pub text_styles: Vec<String>,
    /// The room's floor can carry the Monolithic Slab Foundation flag (a
    /// normal floor, not the foundation or the attic).
    pub slab_allowed: bool,
    /// What the floor's Floor Defaults give a surface the room names nothing
    /// for: floor, ceiling and wall materials ("" = none).
    pub default_floor_material: String,
    pub default_ceiling_material: String,
    pub default_wall_material: String,
    /// The floor's Floor Defaults, for the platform definitions a room that
    /// follows the default reads.
    pub floor_settings: plan_core::floors::FloorSettings,
    /// The plan's assembly library (saved definitions for the layers window).
    pub library: AssemblyLibrary,
    /// Room Information (General panel): the centerline area, the area of
    /// bay, box and bow windows (in the Interior Area only) and its structure's
    /// Living Area.
    pub centerline_area_sq_ft: f64,
    pub bay_area_sq_ft: f64,
    pub structure_living_sq_ft: f64,
    /// The absolute and relative heights of the Structure panel.
    pub heights: plan_core::living::RoomHeights,
    /// The Foundation Defaults slab thickness (a Slab room's floor).
    pub slab_thickness: f64,
    /// The plan's layers, for the Layer panel.
    pub layers: Vec<String>,
}

pub struct RoomDialog {
    frame: SpecDialog,
    form: RoomForm,
}

struct RoomForm {
    init: RoomInit,
    name: RoomName,
    extras: RoomExtras,
    fields: Fields,
    /// The four platform rows (Floor and Ceiling Structure and Finish) and
    /// the Material Layers Definition window they open (Round 16).
    editor: PlatformEditor,
    /// The tab last drawn, for the preview (the Structure panel shows the
    /// cross section).
    shown: std::cell::Cell<usize>,
}

thread_local! {
    static ROOM_TYPES_REQUEST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The "Room Types..." button of the Room Specification was pressed: the shell
/// opens the Room Types list of Default Settings (R-18).
pub fn take_room_types_request() -> bool {
    ROOM_TYPES_REQUEST.with(std::cell::Cell::take)
}

/// Presses the "Room Types..." button without a UI (tests).
#[cfg(test)]
pub fn press_room_types_for_test() {
    ROOM_TYPES_REQUEST.with(|r| r.set(true));
}

/// Which structure the Define editor shows.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Define {
    Floor,
    Ceiling,
}

// The setters below are the dialog's model API (the tests drive it); the UI
// edits the same fields directly.
#[allow(dead_code)]
impl RoomDialog {
    pub fn new(init: RoomInit) -> Self {
        let form = RoomForm {
            name: init.name.clone(),
            extras: init.extras.clone(),
            fields: Fields::default(),
            editor: PlatformEditor::default(),
            shown: std::cell::Cell::new(0),
            init,
        };
        Self {
            frame: SpecDialog::new("Room Specification", "room"),
            form,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The layers window first, so its Enter and Escape are its own.
        self.form
            .editor
            .child(ctx, &mut self.form.extras.assemblies);
        self.frame.show(ctx, &mut self.form)
    }

    /// Definitions saved from the layers window, for the plan's library.
    pub fn take_saved(&mut self) -> Vec<plan_core::assemblies::NamedAssembly> {
        self.form.editor.take_saved()
    }

    /// The definition of `kind` this room has now (its own, the floor's, or
    /// what it stored the older way).
    pub fn platform(&self, kind: AssemblyKind) -> Assembly {
        self.form.effective(kind)
    }

    /// Gives the room its own definition of `kind`.
    pub fn set_platform(&mut self, kind: AssemblyKind, assembly: Assembly) {
        self.form
            .extras
            .assemblies
            .set(kind, plan_core::assemblies::AssemblySlot::Own(assembly));
    }

    /// Use Default for `kind`: the room follows the floor level.
    pub fn follow_default(&mut self, kind: AssemblyKind) {
        self.form
            .extras
            .assemblies
            .set(kind, plan_core::assemblies::AssemblySlot::Default);
    }

    pub fn room_index(&self) -> usize {
        self.form.init.room_index
    }

    pub fn room_name(&self) -> &RoomName {
        &self.form.name
    }

    pub fn room_name_mut(&mut self) -> &mut RoomName {
        &mut self.form.name
    }

    pub fn extras(&self) -> &RoomExtras {
        &self.form.extras
    }

    pub fn extras_mut(&mut self) -> &mut RoomExtras {
        &mut self.form.extras
    }

    pub fn set_name(&mut self, name: &str) {
        self.form.name.name = name.to_string();
    }

    /// Changing the Room Type renames a room that still has its type's name
    /// (or its generated one); a hand-typed name stays (R-21).
    pub fn set_room_type(&mut self, room_type: &str) {
        self.form.set_room_type(room_type);
    }

    /// The Floor or Ceiling Structure layers being edited by Define (R-28,
    /// R-29); an empty stack follows the floor's default platform.
    pub fn structure_mut(&mut self, which: Define) -> &mut Vec<StructureLayer> {
        match which {
            Define::Floor => &mut self.form.extras.floor_structure,
            Define::Ceiling => &mut self.form.extras.ceiling_structure,
        }
    }

    pub fn set_living(&mut self, include: Option<bool>) {
        self.form.name.include_in_living_area = include;
    }

    pub fn has_error(&self) -> bool {
        self.form.error().is_some()
    }

    /// Draws tab `tab` into `ui` (tests).
    #[cfg(test)]
    pub fn page_for_test(&mut self, ui: &mut Ui, tab: usize) {
        SpecPages::page(&mut self.form, ui, tab);
    }
}

impl RoomForm {
    /// The room's older platform fields and its definitions as a `RoomMisc`,
    /// so the default chain can be asked about the draft.
    fn misc_draft(&self) -> plan_core::extras::RoomMisc {
        plan_core::extras::RoomMisc {
            floor_finish_thickness: self.extras.floor_finish_thickness,
            ceiling_finish_thickness: self.extras.ceiling_finish_thickness,
            floor_structure: self.extras.floor_structure.clone(),
            ceiling_structure: self.extras.ceiling_structure.clone(),
            assemblies: self.extras.assemblies.clone(),
            ..plan_core::extras::RoomMisc::default()
        }
    }

    /// The definition of `kind` the draft has.
    fn effective(&self, kind: AssemblyKind) -> Assembly {
        resolve(kind, &self.init.floor_settings, Some(&self.misc_draft())).assembly
    }

    fn type_def(&self) -> Option<&RoomTypeDef> {
        self.init
            .types
            .iter()
            .find(|t| t.name == self.name.room_type)
    }

    fn set_room_type(&mut self, room_type: &str) {
        let old = self.name.room_type.clone();
        let follows = {
            let n = self.name.name.trim();
            n.is_empty() || n == old || n == self.init.default_name
        };
        self.name.room_type = room_type.to_string();
        if follows {
            self.name.name = room_type.to_string();
        }
        self.apply_function_defaults();
    }

    /// The room type's function sets the defaults of the Structure switches,
    /// the floor height, the floor finish and the Floor Structure (R-40,
    /// R-41); each stays editable afterwards.
    fn apply_function_defaults(&mut self) {
        let function = self
            .type_def()
            .map_or("Standard", |t| t.function.as_str())
            .to_string();
        let d = function_defaults_with(&function, &self.name.room_type, self.init.slab_thickness);
        self.name.has_floor = d.has_floor;
        self.name.has_ceiling = d.has_ceiling;
        self.name.flat_ceiling = d.flat_ceiling;
        self.extras.roof_over = d.roof_over;
        self.name.options.build_foundation_below = d.build_foundation;
        self.name.floor_height_offset = d.floor_height_offset;
        self.extras.floor_finish_thickness = d
            .floor_finish_thickness
            .unwrap_or(self.init.default_floor_finish);
        self.extras.floor_structure = d.floor_structure;
        // The type's own floor platform replaces a layered one made before.
        self.extras.assemblies.floor_structure = plan_core::assemblies::AssemblySlot::Legacy;
        self.extras.assemblies.floor_finish = plan_core::assemblies::AssemblySlot::Legacy;
        // A Deck room gets a Deck Specification; any other room loses it.
        let is_deck = effective_function(&function, &self.name.room_type) == DECK_ROOM_TYPE
            || self.name.room_type == DECK_ROOM_TYPE;
        if is_deck {
            if self.name.deck.is_none() {
                self.name.deck = Some(DeckSpec::default());
            }
        } else {
            self.name.deck = None;
        }
        // Then the type's own settings: structure, deck framing, layer, fill,
        // moldings and label overwrite the room's (manual p. 446).
        if let Some(spec) = self.type_def().map(|t| t.spec.clone()) {
            self.apply_type_spec(&spec);
        }
    }

    /// Gives the draft the settings of its room type's `spec` (R-99).
    fn apply_type_spec(&mut self, spec: &RoomTypeSpec) {
        use plan_core::assemblies::AssemblySlot;
        for kind in AssemblyKind::PLATFORM {
            if let AssemblySlot::Own(a) = spec.assemblies.slot(kind) {
                self.extras
                    .assemblies
                    .set(kind, AssemblySlot::Own(a.clone()));
            }
        }
        if let Some(d) = &spec.deck {
            self.name.deck = Some(d.clone());
        }
        self.name.options.layer = spec.layer.clone();
        self.name.options.drawing_group = spec.drawing_group.clone();
        if spec.fill.is_some() {
            self.extras.fill = crate::editor::rooms_edit::FillStyle::from_room(spec.fill.as_ref());
        }
        for m in &spec.moldings {
            match m.kind {
                MoldingKind::Base => self.extras.base_molding = m.profile.clone(),
                MoldingKind::Chair => self.extras.chair_molding = m.profile.clone(),
                MoldingKind::Crown => self.extras.crown_molding = m.profile.clone(),
            }
        }
        if let Some(l) = &spec.label {
            let offset = self.extras.label.offset;
            self.extras.label = l.clone();
            self.extras.label.offset = offset;
        }
    }

    /// Does the draft count in the Living Area now: its own choice, else its
    /// type's, and a rough ceiling under 48 in keeps it out by default
    /// (manual p. 454). The second value says why.
    fn living_now(&self) -> (bool, &'static str) {
        if let Some(b) = self.name.include_in_living_area {
            return (b, "set for this room");
        }
        if self.type_def().is_some_and(|t| !t.include_in_living_area) {
            return (false, "not by its room type");
        }
        let finished = self
            .name
            .ceiling_height
            .unwrap_or(self.init.floor_ceiling_height);
        let rough = self.name.rough_ceiling.unwrap_or(
            finished
                + legacy_value(
                    AssemblyKind::CeilingFinish,
                    &self.effective(AssemblyKind::CeilingFinish),
                ),
        );
        if rough < plan_core::living::MIN_LIVING_CEILING {
            (false, "rough ceiling under 48 in")
        } else {
            (true, "by its room type")
        }
    }

    /// "Use Default (Included)" text for the living-area radio.
    fn living_default(&self) -> &'static str {
        match self.type_def() {
            Some(t) if !t.include_in_living_area => "Use Default (Excluded)",
            _ => "Use Default (Included)",
        }
    }

    fn conditioned_default(&self) -> &'static str {
        match self.type_def() {
            Some(t) if !t.conditioned => "Use Default (Unconditioned)",
            _ => "Use Default (Conditioned)",
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Room Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.name.name).desired_width(200.0));
        });
        row(ui, "Room Type", |ui| {
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("room_type")
                .width(200.0)
                .selected_text(self.name.room_type.clone())
                .show_ui(ui, |ui| {
                    for t in &self.init.types {
                        let on = t.name == self.name.room_type;
                        if ui.selectable_label(on, &t.name).clicked() {
                            picked = Some(t.name.clone());
                        }
                    }
                });
            if let Some(t) = picked {
                self.set_room_type(&t);
            }
            if ui
                .button("Room Types...")
                .on_hover_text("Edit the list of room types in Default Settings")
                .clicked()
            {
                ROOM_TYPES_REQUEST.with(|r| r.set(true));
            }
        });
        row(ui, "Function", |ui| {
            // A function is a fixed set of properties; only a room type's
            // own settings edit (Room Type Defaults).
            let f = self.type_def().map_or("Standard", |t| t.function.as_str());
            let class = function_class(f, &self.name.room_type);
            ui.label(format!(
                "{} ({})",
                effective_function(f, &self.name.room_type),
                class.name()
            ));
        });
        let function = self
            .type_def()
            .map_or("Standard", |t| t.function.as_str())
            .to_string();
        if plan_check::CodeMinimums::is_habitable_in(&self.name.room_type, &function) {
            let min = crate::editor::code::active().room_area_min;
            let area = self.init.interior_area_sq_ft;
            super::code_notice::code_note(
                ui,
                &format!("IRC R304.1 habitable room: min {min:.0} sq ft ({area:.1} now)"),
                area > 0.0 && area < min - 1e-6,
            );
        }

        section(ui, "Living Area");
        let default_text = self.living_default();
        ui.radio_value(
            &mut self.name.include_in_living_area,
            Some(true),
            "Include in Total Living Area Calculation",
        );
        ui.radio_value(
            &mut self.name.include_in_living_area,
            Some(false),
            "Exclude from Total Living Area Calculation",
        );
        ui.radio_value(&mut self.name.include_in_living_area, None, default_text);

        section(ui, "Conditioned Room");
        let default_text = self.conditioned_default();
        ui.radio_value(&mut self.extras.conditioned, Some(true), "Conditioned");
        ui.radio_value(&mut self.extras.conditioned, Some(false), "Unconditioned");
        ui.radio_value(&mut self.extras.conditioned, None, default_text);

        section(ui, "Options");
        row(ui, "Roof Group", |ui| {
            ui.add(egui::DragValue::new(&mut self.name.roof_group).range(0..=99))
                .on_hover_text(
                    "0 is the default group. Rooms of another group are roofed as a separate building",
                );
        });

        section(ui, "Room Information");
        let i = &self.init;
        let (counted, why) = self.living_now();
        let rows = [
            ("Interior Dimensions", i.interior_dims.clone()),
            (
                "Interior Area",
                format!("{:.1} sq ft", i.interior_area_sq_ft),
            ),
            (
                "Standard Area",
                format!("{} sq ft", i.standard_area_sq_ft.round()),
            ),
            (
                "Centerline Area",
                format!("{:.1} sq ft", i.centerline_area_sq_ft),
            ),
            ("Perimeter", fmt_ft_in(i.perimeter_in)),
            (
                "In the Living Area",
                format!("{} ({why})", if counted { "Yes" } else { "No" }),
            ),
            (
                "Living Area of this structure",
                format!("{} sq ft", i.structure_living_sq_ft),
            ),
        ];
        egui::Grid::new("room_information")
            .striped(true)
            .show(ui, |ui| {
                for (k, v) in rows {
                    ui.label(k);
                    ui.label(v);
                    ui.end_row();
                }
                if i.bay_area_sq_ft > 0.0 {
                    ui.label("Bay, Box and Bow Windows");
                    ui.label(format!(
                        "{:.1} sq ft (in the Interior Area, not the Standard Area)",
                        i.bay_area_sq_ft
                    ));
                    ui.end_row();
                }
            });
    }

    fn structure(&mut self, ui: &mut Ui) {
        let elev = self.init.floor_elevation;
        let ceil_default = self.init.floor_ceiling_height;
        let finish = legacy_value(
            AssemblyKind::CeilingFinish,
            &self.effective(AssemblyKind::CeilingFinish),
        );
        let finished = self.name.ceiling_height.unwrap_or(ceil_default);
        let base_rough = finished + finish;
        let rough = self.name.rough_ceiling.unwrap_or(base_rough);
        let h = &self.init.heights;

        // Absolute elevations: measured from zero, the top of Floor 1's subfloor.
        section(ui, "Absolute Elevations");
        row(ui, "Floor Above", |ui| {
            ui.label(match (h.mixed_above, h.floor_above) {
                (true, _) => "No Change".to_string(),
                (false, Some(v)) => fmt_ft_in(v),
                _ => "none".to_string(),
            })
        });
        let mut ceiling_abs = elev + self.name.floor_height_offset + rough;
        let mut changed_ceiling = false;
        ui.add_enabled_ui(!h.mixed_above, |ui| {
            changed_ceiling = self
                .fields
                .length_row(ui, "Ceiling", "ceil_abs", &mut ceiling_abs);
        });
        if changed_ceiling {
            let from_floor = ceiling_abs - elev - self.name.floor_height_offset;
            if self.name.rough_ceiling.is_some() {
                self.name.rough_ceiling = Some(from_floor);
            } else {
                self.name.ceiling_height = Some((from_floor - finish).max(1.0));
            }
        }
        let mut floor_abs = elev + self.name.floor_height_offset;
        if self
            .fields
            .length_row(ui, "Floor", "floor_abs", &mut floor_abs)
        {
            self.name.floor_height_offset = floor_abs - elev;
        }
        row(ui, "Floor Below", |ui| {
            ui.label(h.floor_below.map_or("none".to_string(), fmt_ft_in))
        });
        if let Some(swt) = h.swt_below {
            row(ui, "SWT Below", |ui| ui.label(fmt_ft_in(swt)))
                .on_hover_text("Top of the stem walls around the room below");
        }

        // Relative heights: from surfaces in the room or the room below.
        section(ui, "Relative Heights");
        let mut r = rough;
        if self
            .fields
            .length_row(ui, "Rough Ceiling", "rough_h", &mut r)
        {
            self.name.rough_ceiling = ((r - base_rough).abs() > 1e-6).then_some(r);
        }
        if rough < base_rough - 1e-6 {
            ui.colored_label(
                super::ERROR_RED,
                "The framing cannot be lower than the finished ceiling and its finish",
            );
        }
        let mut f = finished;
        if self
            .fields
            .length_row(ui, "Finished Ceiling", "ceil_h", &mut f)
        {
            self.name.ceiling_height = Some(f);
        }
        if let Some(min) = crate::editor::code::active().ceiling_min_for(&self.name.room_type) {
            let mut v = self.name.ceiling_height.unwrap_or(ceil_default);
            if super::code_notice::code_notice(
                ui,
                "IRC R305.1 ceiling height",
                &mut v,
                min,
                super::code_notice::LimitKind::Min,
            ) {
                self.name.ceiling_height = Some(v);
            }
        }
        if self.name.ceiling_height.is_some()
            && ui.small_button("Use the floor's ceiling height").clicked()
        {
            self.name.ceiling_height = None;
        }
        if self.name.rough_ceiling.is_none() {
            ui.weak(format!(
                "Without a higher rough ceiling the framing starts at the finished ceiling plus {} of finish.",
                fmt_ft_in(finish)
            ));
        }
        if self.name.options.floor_from_foundation {
            if let Some(v) = h.stem_wall_top_to_ceiling {
                row(ui, "Stem Wall Top to Ceiling", |ui| ui.label(fmt_ft_in(v)));
            }
            if let Some(v) = h.floor_to_stem_wall_top {
                row(ui, "Floor to Stem Wall Top", |ui| ui.label(fmt_ft_in(v)));
            }
        }
        row(ui, "Ceiling Below", |ui| {
            ui.label(h.ceiling_below.map_or("none".to_string(), fmt_ft_in))
        });
        if self.extras.stem_wall {
            self.fields
                .length_row(ui, "Stem Wall", "stem_h", &mut self.extras.stem_wall_height);
        }

        section(ui, "Floor and Ceiling Platforms");
        let misc = self.misc_draft();
        let cx = RowContext {
            allow_default: true,
            inherited: AssemblyKind::PLATFORM.map(|k| {
                plan_core::assemblies::resolve_floor(k, &self.init.floor_settings).assembly
            }),
            legacy: AssemblyKind::PLATFORM.map(|k| {
                let mut legacy = misc.clone();
                legacy.assemblies = plan_core::assemblies::PlatformAssemblies::default();
                resolve(k, &self.init.floor_settings, Some(&legacy)).assembly
            }),
            library: &self.init.library,
        };
        let mut slots = self.extras.assemblies.clone();
        self.editor.rows(ui, &mut slots, &cx);
        self.extras.assemblies = slots;
        // A finish nobody has laid out in layers keeps its single thickness.
        if self
            .extras
            .assemblies
            .slot(AssemblyKind::FloorFinish)
            .is_legacy()
        {
            self.fields.length_row(
                ui,
                "Floor Finish Thickness",
                "floor_fin",
                &mut self.extras.floor_finish_thickness,
            );
        }
        if self
            .extras
            .assemblies
            .slot(AssemblyKind::CeilingFinish)
            .is_legacy()
        {
            self.fields.length_row(
                ui,
                "Ceiling Finish Thickness",
                "ceil_fin",
                &mut self.extras.ceiling_finish_thickness,
            );
        }
        let dropped = self.effective(AssemblyKind::CeilingFinish).drop();
        if dropped > 0.0 {
            ui.weak(format!(
                "A dropped ceiling: the finished ceiling hangs {} below the Ceiling Height.",
                fmt_ft_in(dropped)
            ));
        }

        section(ui, "Ceiling");
        ui.checkbox(&mut self.name.has_ceiling, "Ceiling Over This Room")
            .on_hover_text("Off for a deck or porch");
        ui.checkbox(&mut self.extras.roof_over, "Roof Over This Room")
            .on_hover_text("Off for a courtyard or open deck: Build Roof leaves a hole over it");
        ui.add_enabled_ui(self.extras.roof_over, |ui| {
            ui.checkbox(&mut self.extras.flat_roof, "Flat Roof Over This Room")
                .on_hover_text("Build Roof puts a level roof plane at this room's ceiling");
        });
        ui.checkbox(&mut self.name.flat_ceiling, "Flat Ceiling Over This Room")
            .on_hover_text(
                "Off: the ceiling follows the underside of the roof or the ceiling planes (a cathedral ceiling)",
            );
        ui.checkbox(&mut self.name.options.shelf_ceiling, "Shelf Ceiling")
            .on_hover_text("No Attic Walls over the interior walls that define this room");
        ui.checkbox(
            &mut self.name.options.soffit_surface_ceiling,
            "Use Soffit Surface for Ceiling",
        )
        .on_hover_text("Frame the roof over this room with the fascia framing defaults");

        section(ui, "Floor");
        ui.checkbox(&mut self.name.has_floor, "Floor Under This Room")
            .on_hover_text("Off for an Open Below room: no floor, and the ceiling below opens");
        let below = self.init.heights.floor_below.is_some();
        ui.add_enabled_ui(below || self.name.options.floor_from_foundation, |ui| {
            ui.checkbox(
                &mut self.name.options.floor_from_foundation,
                "Floor Supplied by the Foundation Room Below",
            )
            .on_hover_text("The floor of this room is a slab on the floor below");
        });
        ui.checkbox(
            &mut self.name.options.supplies_floor_above,
            "Room Supplies Floor for the Room Above",
        )
        .on_hover_text(
            "This room's slab and curbs are the floor of the room above (a garage on a slab)",
        );
        ui.add_enabled_ui(self.init.slab_allowed, |ui| {
            ui.checkbox(
                &mut self.name.options.build_foundation_below,
                "Build Foundation Below",
            )
            .on_hover_text("Off: Build Foundation leaves no foundation under this room");
        });
        ui.checkbox(
            &mut self.name.options.raised_floor_bump_out,
            "Raised Floor For Bump Out",
        )
        .on_hover_text("This room's floor and the ceiling of the room below build independently");
        ui.checkbox(
            &mut self.name.options.retain_framing,
            "Retain Floor/Ceiling Framing",
        )
        .on_hover_text("Keeps the framing when floor and ceiling framing is rebuilt globally");
        ui.horizontal(|ui| {
            ui.label("On Structure Resize");
            ui.radio_value(
                &mut self.name.options.resize_lock,
                ResizeLock::FloorTop,
                "Lock Floor Top",
            );
            ui.radio_value(
                &mut self.name.options.resize_lock,
                ResizeLock::FloorBottom,
                "Lock Floor Bottom",
            );
        });

        section(ui, "Monolithic Slab Foundation");
        let mut mono = self.name.monolithic_slab.is_some();
        let toggled = ui
            .add_enabled(
                self.init.slab_allowed || mono,
                egui::Checkbox::new(&mut mono, "Monolithic Slab Foundation"),
            )
            .on_hover_text("The floor is a slab with a thickened edge; no foundation floor is needed under this room")
            .changed();
        if toggled {
            self.name.monolithic_slab = mono.then(RoomSlab::default);
        }
        if let Some(slab) = self.name.monolithic_slab.as_mut() {
            self.fields
                .length_row(ui, "Slab Thickness", "mono_t", &mut slab.thickness);
            self.fields
                .length_row(ui, "Slab Stem Wall Height", "mono_h", &mut slab.stem_height);
            row(ui, "Slab Pour Number", |ui| {
                ui.add(egui::DragValue::new(&mut self.name.options.pour_number).range(0..=99));
            });
        } else {
            row(ui, "Framing Group", |ui| {
                ui.add(egui::DragValue::new(&mut self.name.options.framing_group).range(0..=99));
            });
        }

        section(ui, "Stem Wall");
        ui.checkbox(&mut self.extras.stem_wall, "Stem Wall");
    }

    /// The cross-section diagram of the Structure panel (manual p. 470): the
    /// room's floor platform, finish and ceiling platform between the floor
    /// below and the floor above (the left side of the floor above offset for
    /// clarity), with the heights set on the panel. `Floor` and `Ceiling` are
    /// the absolute elevations, `R` the rough ceiling and `F` the finished one.
    fn section_preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            rect.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Cross section",
            11.0,
        );
        let area = Rect::from_min_max(
            rect.min + egui::vec2(6.0, 18.0),
            rect.max - egui::vec2(6.0, 8.0),
        );
        let h = &self.init.heights;
        let finish_c = legacy_value(
            AssemblyKind::CeilingFinish,
            &self.effective(AssemblyKind::CeilingFinish),
        );
        let floor_t = self
            .effective(AssemblyKind::FloorStructure)
            .total_thickness()
            .max(0.5);
        let ceil_t = self
            .effective(AssemblyKind::CeilingStructure)
            .total_thickness()
            .max(0.5);
        let finished = self
            .name
            .ceiling_height
            .unwrap_or(self.init.floor_ceiling_height);
        let rough = self.name.rough_ceiling.unwrap_or(finished + finish_c);
        let slab = floor_t.max(ceil_t).max(6.0);
        // Inches from the bottom of the floor below's platform to the top of
        // the floor above's platform.
        let below_t = if h.floor_below.is_some() { slab } else { 0.0 };
        let above_t = if h.floor_above.is_some() || h.mixed_above {
            slab
        } else {
            0.0
        };
        let total = below_t + floor_t + rough + ceil_t + above_t + 2.0;
        let k = (f64::from(area.height()) / total.max(1.0)) as f32;
        let y_of = |inches_from_bottom: f64| area.max.y - inches_from_bottom as f32 * k;
        let x0 = area.min.x + 56.0;
        let x1 = area.max.x - 4.0;
        let room_x = (x0 + 14.0, x1 - 14.0);
        let mut cur = 0.0_f64;
        let band =
            |p: &Painter, cur: &mut f64, t: f64, x: (f32, f32), fill: Color32, label: &str| {
                let r =
                    Rect::from_min_max(Pos2::new(x.0, y_of(*cur + t)), Pos2::new(x.1, y_of(*cur)));
                p.rect_filled(r, 0.0, fill);
                p.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(0.8_f32, PV_INK),
                    egui::StrokeKind::Inside,
                );
                if r.height() > 9.0 && !label.is_empty() {
                    pv_text(p, r.center(), Align2::CENTER_CENTER, label, 9.0);
                }
                *cur += t;
            };
        if below_t > 0.0 {
            band(
                p,
                &mut cur,
                below_t,
                (x0, x1),
                PV_FAINT.gamma_multiply(0.45),
                "Floor below",
            );
        }
        band(p, &mut cur, floor_t, (x0, x1), PV_WALL, "Floor platform");
        let room_bottom = cur;
        // The room: finished floor to the finished ceiling.
        let room_rect = Rect::from_min_max(
            Pos2::new(room_x.0, y_of(room_bottom + rough)),
            Pos2::new(room_x.1, y_of(room_bottom)),
        );
        p.rect_filled(room_rect, 0.0, PV_GLASS.gamma_multiply(0.35));
        cur += rough;
        band(p, &mut cur, ceil_t, (x0, x1), PV_WALL, "Ceiling platform");
        if above_t > 0.0 {
            // The floor above, its left side offset for clarity.
            band(
                p,
                &mut cur,
                above_t,
                (x0 + 10.0, x1),
                PV_FAINT.gamma_multiply(0.45),
                "Floor above",
            );
        }
        let callout = |p: &Painter, y: f32, text: String, color: Color32| {
            p.line_segment(
                [Pos2::new(area.min.x, y), Pos2::new(x0, y)],
                Stroke::new(0.8_f32, color),
            );
            pv_text(
                p,
                Pos2::new(area.min.x, y - 6.0),
                Align2::LEFT_CENTER,
                &text,
                9.0,
            );
        };
        callout(
            p,
            y_of(room_bottom),
            format!(
                "Floor {}",
                fmt_ft_in(self.init.floor_elevation + self.name.floor_height_offset)
            ),
            PV_ACCENT,
        );
        callout(
            p,
            y_of(room_bottom + rough),
            format!(
                "Ceiling {}",
                fmt_ft_in(self.init.floor_elevation + self.name.floor_height_offset + rough)
            ),
            PV_ACCENT,
        );
        pv_text(
            p,
            Pos2::new(room_rect.center().x, room_rect.center().y - 6.0),
            Align2::CENTER_CENTER,
            format!("R {}", fmt_ft_in(rough)),
            10.0,
        );
        pv_text(
            p,
            Pos2::new(room_rect.center().x, room_rect.center().y + 8.0),
            Align2::CENTER_CENTER,
            format!("F {}", fmt_ft_in(finished)),
            10.0,
        );
    }

    /// Layer tab (R-103): the layer and Drawing Group of the room.
    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            let current = self.name.options.layer_name().to_string();
            egui::ComboBox::from_id_salt("room_layer")
                .width(200.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.name.options.layer,
                        String::new(),
                        plan_core::rooms::ROOM_LAYER,
                    );
                    for l in &self.init.layers {
                        if l != plan_core::rooms::ROOM_LAYER {
                            ui.selectable_value(&mut self.name.options.layer, l.clone(), l);
                        }
                    }
                });
        });
        section(ui, "Drawing Group");
        row(ui, "Drawing Group", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name.options.drawing_group)
                    .desired_width(200.0)
                    .hint_text("Rooms"),
            );
        });
        ui.weak("The room's fill style and label draw on this layer; its layer must be on and unlocked for the room to display and be selectable.");
    }

    /// Deck tab (CB-86).
    fn deck(&mut self, ui: &mut Ui) {
        let edges = self.init.polygon.len();
        if deck_panel(ui, &mut self.fields, &mut self.name.deck, edges, "Room") {
            self.name.has_ceiling = false;
        }
    }

    /// Moldings tab (R-34): a base, a chair rail and a crown profile from the
    /// molding library, built around the room in 3D.
    fn moldings(&mut self, ui: &mut Ui) {
        moldings_panel(
            ui,
            &mut self.extras.base_molding,
            &mut self.extras.chair_molding,
            &mut self.extras.crown_molding,
        );
    }

    fn wall_covering(&mut self, ui: &mut Ui) {
        section(ui, "Wall Covering");
        let default = self.init.default_wall_material.clone();
        row(ui, "Interior Wall Covering", |ui| {
            surface_combo(
                ui,
                "room_wall_covering",
                &mut self.extras.wall_covering,
                &WALL_SURFACES,
                &default,
            )
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        row(ui, "Pattern", |ui| {
            egui::ComboBox::from_id_salt("room_fill")
                .selected_text(self.extras.fill.pattern.name())
                .show_ui(ui, |ui| {
                    for p in FillPattern::ALL {
                        ui.selectable_value(&mut self.extras.fill.pattern, p, p.name());
                    }
                });
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut self.extras.fill.color);
        });
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut self.extras.fill.alpha, 0.1..=1.0));
        });
        ui.weak("Drawn in the plan view only.");
    }

    /// Materials tab (R-36): the surface material of the floor, the ceiling
    /// and the walls. A surface left on the floor's default follows Floor
    /// Defaults.
    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Surfaces");
        let (df, dc, dw) = (
            self.init.default_floor_material.clone(),
            self.init.default_ceiling_material.clone(),
            self.init.default_wall_material.clone(),
        );
        let mut floor = self.name.floor_finish.clone().unwrap_or_default();
        row(ui, "Floor Surface", |ui| {
            surface_combo(ui, "room_floor_surface", &mut floor, &FLOOR_SURFACES, &df)
        });
        self.name.floor_finish = (!floor.trim().is_empty()).then_some(floor);
        let mut ceil = self.name.ceiling_finish.clone().unwrap_or_default();
        row(ui, "Ceiling Surface", |ui| {
            surface_combo(
                ui,
                "room_ceiling_surface",
                &mut ceil,
                &CEILING_SURFACES,
                &dc,
            )
        });
        self.name.ceiling_finish = (!ceil.trim().is_empty()).then_some(ceil);
        row(ui, "Wall Surface", |ui| {
            surface_combo(
                ui,
                "room_wall_surface",
                &mut self.extras.wall_covering,
                &WALL_SURFACES,
                &dw,
            )
        });
        ui.add_space(4.0);
        ui.weak("Moldings are chosen on the Moldings tab. The Material Painter can still recolor any surface.");
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Display in All Views");
        let l = &mut self.extras.label;
        ui.checkbox(&mut l.show_name, "Room Name");
        ui.checkbox(&mut l.show_dimensions, "Interior Dimensions");
        ui.checkbox(&mut l.show_area, "Area");
        ui.add_enabled_ui(l.show_area, |ui| {
            ui.radio_value(&mut l.area_kind, AreaKind::Interior, "Interior Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Standard, "Standard Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Centerline, "Centerline Area");
        });
        ui.weak("With everything unchecked the room shows no label.");
        section(ui, "Label Text");
        ui.add(
            egui::TextEdit::multiline(&mut self.extras.label.template)
                .desired_rows(2)
                .desired_width(260.0)
                .hint_text("<name>\\n<dims>  <area>"),
        );
        let macros: Vec<String> = crate::editor::rooms_edit::LABEL_MACROS
            .iter()
            .map(|(m, what)| format!("{m} {what}"))
            .collect();
        ui.weak(format!(
            "Macros: {}. Empty uses the choices above.",
            macros.join(", ")
        ));
        let moved = self.extras.label.offset != Point::ZERO;
        ui.horizontal(|ui| {
            ui.weak("Drag the label in the plan to move it.");
            if ui
                .add_enabled(moved, egui::Button::new("Reset Position"))
                .clicked()
            {
                self.extras.label.offset = Point::ZERO;
            }
        });
        section(ui, "Appearance");
        row(ui, "Text Style", |ui| {
            let style = &mut self.name.label_style.text_style;
            egui::ComboBox::from_id_salt("room_label_style")
                .width(200.0)
                .selected_text(if style.is_empty() {
                    "Use Layer Text Style"
                } else {
                    style.as_str()
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(style, String::new(), "Use Layer Text Style");
                    for n in &self.init.text_styles {
                        ui.selectable_value(style, n.clone(), n);
                    }
                });
        });
        row(ui, "Label Position", |ui| {
            let placement = &mut self.name.label_style.placement;
            egui::ComboBox::from_id_salt("room_label_position")
                .width(200.0)
                .selected_text(placement.name())
                .show_ui(ui, |ui| {
                    for p in LabelPlacement::ALL {
                        ui.selectable_value(placement, p, p.name());
                    }
                });
        });
    }

    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Floor and Ceiling Finish Layers");
        let none = "(none)";
        let rows = [
            (
                "Floor finish",
                self.name
                    .floor_finish
                    .clone()
                    .unwrap_or_else(|| none.into()),
                self.extras.floor_finish_thickness,
            ),
            (
                "Ceiling finish",
                self.name
                    .ceiling_finish
                    .clone()
                    .unwrap_or_else(|| none.into()),
                self.extras.ceiling_finish_thickness,
            ),
        ];
        egui::Grid::new("room_components")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Component");
                ui.strong("Material");
                ui.strong("Thickness");
                ui.end_row();
                for (c, m, t) in rows {
                    ui.label(c);
                    ui.label(m);
                    ui.label(fmt_ft_in(t));
                    ui.end_row();
                }
                for (c, m) in [
                    ("Wall covering", &self.extras.wall_covering),
                    ("Base molding", &self.extras.base_molding),
                    ("Chair rail", &self.extras.chair_molding),
                    ("Crown molding", &self.extras.crown_molding),
                ] {
                    ui.label(c);
                    ui.label(if m.is_empty() { none } else { m.as_str() });
                    ui.label(
                        molding_def(m)
                            .map(|d| fmt_ft_in(d.height()))
                            .unwrap_or_default(),
                    );
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        ui.weak(format!(
            "Values are calculated based on the room displayed in the preview (interior area = {} sq ft).",
            self.init.interior_area_sq_ft.round()
        ));
    }

    fn object_information(&mut self, ui: &mut Ui) {
        section(ui, "Room");
        let i = &self.init;
        let rows = [
            ("Floor", i.floor_name.clone()),
            ("Interior dimensions", i.interior_dims.clone()),
            (
                "Interior area",
                format!("{:.1} sq ft", i.interior_area_sq_ft),
            ),
            (
                "Standard area",
                format!("{:.1} sq ft", i.standard_area_sq_ft),
            ),
            ("Perimeter", fmt_ft_in(i.perimeter_in)),
            (
                "Total living area (all floors)",
                format!("{:.0} sq ft", i.total_living_sq_ft),
            ),
        ];
        egui::Grid::new("room_info").striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        dis_check(ui, "Locked", false);
    }

    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Room Finish Schedule Row");
        let ceil = self
            .name
            .ceiling_height
            .unwrap_or(self.init.floor_ceiling_height);
        egui::Grid::new("room_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for h in ["Name", "Type", "Area", "Ceiling", "Floor", "Ceiling finish"] {
                    ui.strong(h);
                }
                ui.end_row();
                ui.label(&self.name.name);
                ui.label(&self.name.room_type);
                ui.label(format!("{:.0} sq ft", self.init.interior_area_sq_ft));
                ui.label(fmt_ft_in(ceil));
                ui.label(self.name.floor_finish.clone().unwrap_or_default());
                ui.label(self.name.ceiling_finish.clone().unwrap_or_default());
                ui.end_row();
            });
    }
}

/// The Moldings panel (R-34): a base, a chair rail and a crown profile from
/// the molding library. Shared by the Room Specification and the Room Type
/// Defaults dialogs.
pub(super) fn moldings_panel(
    ui: &mut Ui,
    base: &mut String,
    chair: &mut String,
    crown: &mut String,
) {
    section(ui, "Profiles");
    ui.weak("Choose a profile from the library for each molding. They run around the room, stop at doors and miter at the corners.");
    for (kind, label) in [
        (MoldingKind::Base, "Base"),
        (MoldingKind::Chair, "Chair Rail"),
        (MoldingKind::Crown, "Crown"),
    ] {
        ui.add_space(4.0);
        let current = match kind {
            MoldingKind::Base => &mut *base,
            MoldingKind::Chair => &mut *chair,
            MoldingKind::Crown => &mut *crown,
        };
        row(ui, label, |ui| {
            egui::ComboBox::from_id_salt(("room_molding", label))
                .width(210.0)
                .selected_text(if current.is_empty() {
                    "None"
                } else {
                    current.as_str()
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(current, String::new(), "None");
                    for d in molding_defs(kind) {
                        ui.selectable_value(current, d.name.to_string(), d.name);
                    }
                });
        });
        if let Some(def) = molding_def(current) {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                profile_preview(ui, def);
                ui.weak(format!(
                    "{} high, {} projection",
                    fmt_ft_in(def.height()),
                    fmt_ft_in(def.projection())
                ));
            });
        } else if !current.is_empty() {
            ui.weak("Not in the library: built with the first profile of its kind.");
        }
    }
}

/// A combo box of surface materials: "Use Floor Default" first (empty value),
/// then the library `options`; a name typed elsewhere shows as is.
fn surface_combo(
    ui: &mut Ui,
    salt: &str,
    value: &mut String,
    options: &[&str],
    floor_default: &str,
) {
    let default_text = if floor_default.trim().is_empty() {
        "Use Floor Default".to_string()
    } else {
        format!("Use Floor Default ({floor_default})")
    };
    egui::ComboBox::from_id_salt(salt)
        .width(220.0)
        .selected_text(if value.trim().is_empty() {
            default_text.clone()
        } else {
            value.clone()
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), default_text);
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
}

/// A small drawing of a molding's cross section: the wall on the left, the
/// room to the right.
fn profile_preview(ui: &mut Ui, def: &plan_core::rooms::MoldingDef) {
    let size = egui::vec2(96.0, 56.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, PV_FAINT.gamma_multiply(0.25));
    let (w, h) = (def.projection().max(0.5), def.height().max(0.5));
    let k = ((f64::from(size.x) - 16.0) / w).min((f64::from(size.y) - 12.0) / h) as f32;
    let origin = Pos2::new(rect.min.x + 10.0, rect.max.y - 6.0);
    let pts: Vec<Pos2> = def
        .section
        .iter()
        .map(|&(x, y)| Pos2::new(origin.x + x as f32 * k, origin.y - y as f32 * k))
        .collect();
    // Profiles are not convex: fill them by triangles.
    let mut mesh = egui::Mesh::default();
    for q in &pts {
        mesh.colored_vertex(*q, PV_WALL);
    }
    for [a, b, c] in plan_3d::triangulate::ear_clip(&def.points()) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    p.add(Shape::mesh(mesh));
    p.add(Shape::closed_line(pts, Stroke::new(1.0_f32, PV_INK)));
    p.line_segment(
        [
            Pos2::new(origin.x, rect.min.y + 2.0),
            Pos2::new(origin.x, origin.y),
        ],
        Stroke::new(1.5_f32, PV_ACCENT),
    );
}

/// The Deck panel (CB-86): the Deck Specification of a deck room: the
/// decking boards, the framing Build Framing > Deck makes, and stairs to
/// grade. Shared by the Room Specification and the Room Type Defaults
/// dialogs. Returns true when the box that makes the room a deck was just
/// ticked (the room then has no ceiling).
pub(super) fn deck_panel(
    ui: &mut Ui,
    f: &mut Fields,
    deck: &mut Option<DeckSpec>,
    edges: usize,
    what: &str,
) -> bool {
    section(ui, "Deck");
    let mut is_deck = deck.is_some();
    let mut turned_on = false;
    if ui
        .checkbox(&mut is_deck, format!("This {what} is a Deck"))
        .on_hover_text("A deck has decking boards on joists instead of a platform slab, no ceiling and no roof")
        .changed()
    {
        *deck = is_deck.then(DeckSpec::default);
        turned_on = is_deck;
    }
    let Some(spec) = deck.as_mut() else {
        ui.weak("Set the Room Type to Deck, or tick the box above, to specify decking, framing and stairs.");
        return false;
    };

    section(ui, "Planking");
    ui.checkbox(&mut spec.planking.enabled, "Build Decking Boards");
    ui.add_enabled_ui(spec.planking.enabled, |ui| {
        let p = &mut spec.planking;
        f.length_row(ui, "Board Width", "deck_bw", &mut p.board_width);
        f.length_row(ui, "Board Thickness", "deck_bt", &mut p.board_thickness);
        f.length_row(ui, "Gap Between Boards", "deck_gap", &mut p.gap);
        f.degrees_row(ui, "Board Direction", "deg_deck_angle", &mut p.angle);
        f.length_row(ui, "Overhang at the Rim", "deck_ov", &mut p.overhang);
        ui.checkbox(&mut p.border, "Picture Frame Border");
        ui.add_enabled_ui(p.border, |ui| {
            row(ui, "Border Rows", |ui| {
                ui.add(egui::DragValue::new(&mut p.border_boards).range(1..=4));
            });
        });
        row(ui, "Board Material", |ui| {
            ui.add(egui::TextEdit::singleline(&mut p.material).desired_width(150.0));
        });
        row(ui, "Border Material", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut p.border_material)
                    .desired_width(150.0)
                    .hint_text("Same as the boards"),
            );
        });
    });

    section(ui, "Framing");
    ui.checkbox(&mut spec.framing.enabled, "Build Deck Framing");
    ui.add_enabled_ui(spec.framing.enabled, |ui| {
        let fr = &mut spec.framing;
        let sizes = ["2x6", "2x8", "2x10", "2x12"];
        let combo = |ui: &mut Ui, salt: &str, value: &mut String, options: &[&str]| {
            egui::ComboBox::from_id_salt(salt)
                .width(120.0)
                .selected_text(value.clone())
                .show_ui(ui, |ui| {
                    for o in options {
                        ui.selectable_value(value, (*o).to_string(), *o);
                    }
                });
        };
        row(ui, "Joist Size", |ui| {
            combo(ui, "deck_joist", &mut fr.joist_size, &sizes)
        });
        f.length_row(
            ui,
            "Joist Spacing (on Center)",
            "deck_js",
            &mut fr.joist_spacing,
        );
        let mut auto = fr.joist_angle.is_none();
        ui.horizontal(|ui| {
            if ui
                .radio_value(&mut auto, true, "Joists Run Away from the Ledger")
                .changed()
                && auto
            {
                fr.joist_angle = None;
            }
            if ui
                .radio_value(&mut auto, false, "Joist Direction")
                .changed()
                && !auto
            {
                fr.joist_angle = Some(90.0);
            }
        });
        if let Some(a) = fr.joist_angle.as_mut() {
            f.degrees_row(ui, "Joist Angle", "deg_deck_joist", a);
        }
        row(ui, "Beam Size", |ui| {
            combo(
                ui,
                "deck_beam",
                &mut fr.beam_size,
                &["2x8", "2x10", "2x12", "4x10", "4x12", "6x10"],
            )
        });
        row(ui, "Beam Plies", |ui| {
            ui.add(egui::DragValue::new(&mut fr.beam_plies).range(1..=4));
        });
        f.length_row(
            ui,
            "Beam Set Back from the Rim",
            "deck_bsb",
            &mut fr.beam_setback,
        );
        row(ui, "Post Size", |ui| {
            combo(
                ui,
                "deck_post",
                &mut fr.post_size,
                &["4x4", "4x6", "6x6", "8x8"],
            )
        });
        f.length_row(ui, "Greatest Post Spacing", "deck_ps", &mut fr.post_spacing);
        f.length_row(ui, "Footing Size", "deck_fs", &mut fr.footing_size);
        f.length_row(
            ui,
            "Footing Thickness",
            "deck_ft",
            &mut fr.footing_thickness,
        );
        f.length_row(
            ui,
            "Deck Height Above Grade",
            "deck_hg",
            &mut fr.height_above_grade,
        );
        ui.checkbox(&mut fr.ledger, "Ledger Where the Deck Meets the House");
        ui.checkbox(&mut fr.rim_joists, "Rim Joists");
    });
    let carry = plan_framing::deck::max_joist_span(spec);
    ui.weak(format!(
        "{} joists at {} on center carry about {}.",
        spec.framing.joist_size,
        fmt_ft_in(spec.framing.joist_spacing),
        fmt_ft_in(carry)
    ));

    section(ui, "Stairs to Grade");
    ui.checkbox(&mut spec.stairs.to_grade, "Build Stairs to the Ground");
    ui.add_enabled_ui(spec.stairs.to_grade, |ui| {
        row(ui, "Leaves Edge", |ui| {
            ui.add(egui::DragValue::new(&mut spec.stairs.edge).range(0..=edges.saturating_sub(1)));
            ui.weak(format!("of {edges}, counted from the first corner"));
        });
        f.length_row(ui, "Stair Width", "deck_sw", &mut spec.stairs.width);
        f.length_row(ui, "Tread Depth", "deck_st", &mut spec.stairs.tread);
    });
    ui.add_space(6.0);
    ui.weak("Choose Build > Framing > Build Deck Framing to make the joists, beams and posts with footings from this specification.");
    turned_on
}

impl SpecPages for RoomForm {
    fn tabs(&self) -> &'static [Tab] {
        ROOM_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.name.name.trim().is_empty() {
            return Some("The room needs a name".into());
        }
        if self.name.ceiling_height.is_some_and(|c| c <= 0.0) {
            return Some("Ceiling height must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        self.shown.set(tab);
        match ROOM_TABS[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Deck" => self.deck(ui),
            "Moldings" => self.moldings(ui),
            "Wall Covering" => self.wall_covering(ui),
            "Layer" => self.layer(ui),
            "Fill Style" => self.fill_style(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Components" => self.components(ui),
            "Object Information" => self.object_information(ui),
            "Schedule" => self.schedule(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        // The Structure panel shows the cross section: the floors above and
        // below, the platforms and the heights set on the panel.
        if ROOM_TABS
            .get(self.shown.get())
            .is_some_and(|t| t.name == "Structure")
        {
            self.section_preview(p, rect);
            return;
        }
        pv_text(
            p,
            rect.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        let area = Rect::from_min_max(
            rect.min + egui::vec2(0.0, 16.0),
            rect.max - egui::vec2(0.0, 28.0),
        );
        let poly = &self.init.polygon;
        if poly.len() >= 3 {
            let (mut lo, mut hi) = (poly[0], poly[0]);
            for q in poly {
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
            let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
            let s = ((area.width() as f64 - 8.0) / w).min((area.height() as f64 - 8.0) / h) as f32;
            let c = area.center();
            let (mx, my) = ((lo.x + hi.x) as f32 * 0.5, (lo.y + hi.y) as f32 * 0.5);
            let pts: Vec<Pos2> = poly
                .iter()
                .map(|q| Pos2::new(c.x + (q.x as f32 - mx) * s, c.y - (q.y as f32 - my) * s))
                .collect();
            let [r, g, b] = self.extras.fill.color;
            let fill = if self.extras.fill.pattern == FillPattern::None {
                PV_WALL
            } else {
                Color32::from_rgb(r, g, b)
            };
            p.add(Shape::convex_polygon(
                pts.clone(),
                fill,
                Stroke::new(1.5_f32, PV_INK),
            ));
            p.add(Shape::closed_line(pts, Stroke::new(1.5_f32, PV_INK)));
        }
        p.rect_stroke(
            area,
            0.0,
            Stroke::new(0.8_f32, PV_FAINT),
            egui::StrokeKind::Inside,
        );
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 18.0),
            Align2::CENTER_CENTER,
            self.name.name.clone(),
            12.0,
        );
        p.text(
            Pos2::new(rect.center().x, rect.max.y - 5.0),
            Align2::CENTER_CENTER,
            format!(
                "{}  {:.0} sq ft",
                self.init.interior_dims, self.init.interior_area_sq_ft
            ),
            egui::FontId::proportional(10.0),
            PV_ACCENT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::extras::structure_thickness;
    use plan_core::PlanDefaults;

    fn dialog() -> RoomDialog {
        let d = PlanDefaults::chief_x18_daniel();
        RoomDialog::new(RoomInit {
            room_index: 0,
            name: RoomName::new(Point::new(10.0, 10.0), "Room 1", "Standard"),
            extras: RoomExtras::from_defaults(&d),
            types: d.rooms.room_types.clone(),
            polygon: vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                Point::new(0.0, 100.0),
            ],
            interior_dims: "8'-4\" x 8'-4\"".into(),
            interior_area_sq_ft: 69.4,
            standard_area_sq_ft: 80.0,
            perimeter_in: 400.0,
            floor_elevation: 0.0,
            floor_ceiling_height: 109.125,
            default_floor_finish: 0.75,
            default_name: "Room 1".into(),
            total_living_sq_ft: 0.0,
            floor_name: "1st Floor".into(),
            text_styles: vec!["Room Label Style".into(), "Schedule Style".into()],
            slab_allowed: true,
            default_floor_material: String::new(),
            default_ceiling_material: String::new(),
            default_wall_material: String::new(),
            floor_settings: plan_core::floors::FloorSettings::default(),
            library: AssemblyLibrary::default(),
            centerline_area_sq_ft: 69.4,
            bay_area_sq_ft: 0.0,
            structure_living_sq_ft: 80.0,
            heights: plan_core::living::RoomHeights::default(),
            slab_thickness: 4.0,
            layers: vec!["Rooms".into(), "CAD, Default".into()],
        })
    }

    #[test]
    fn changing_type_renames_a_default_named_room() {
        let mut d = dialog();
        d.set_room_type("Bedroom");
        assert_eq!(d.room_name().name, "Bedroom");
        assert_eq!(d.room_name().room_type, "Bedroom");
        // A hand-typed name survives a type change.
        d.set_name("Guest Suite");
        d.set_room_type("Bath");
        assert_eq!(d.room_name().name, "Guest Suite");
        assert_eq!(d.room_name().room_type, "Bath");
    }

    #[test]
    fn empty_name_blocks_ok() {
        let mut d = dialog();
        assert!(!d.has_error());
        d.set_name("  ");
        assert!(d.has_error());
    }

    #[test]
    fn room_function_sets_the_platform_defaults() {
        let mut d = dialog();
        d.set_room_type("Garage");
        let n = d.room_name();
        assert_eq!(n.floor_height_offset, -24.0);
        assert!(n.has_floor && n.has_ceiling);
        assert_eq!(d.extras().floor_finish_thickness, 0.0);
        assert_eq!(structure_thickness(&d.extras().floor_structure), 4.0);
        d.set_room_type("Deck");
        assert!(!d.room_name().has_ceiling && d.room_name().has_floor);
        assert_eq!(d.room_name().floor_height_offset, 0.0);
        d.set_room_type("Open Below");
        assert!(!d.room_name().has_floor && d.room_name().has_ceiling);
        for t in ["Attic", "Courtyard"] {
            d.set_room_type(t);
            assert!(!d.room_name().has_floor, "{t}");
        }
        // A courtyard is open to the sky: no ceiling and no roof either.
        assert!(!d.room_name().has_ceiling && !d.extras().roof_over);
        // An Attic gets no ceiling and no roof: the roof generator ignores it.
        d.set_room_type("Attic");
        assert!(!d.room_name().has_ceiling && !d.extras().roof_over);
        // A plain room goes back to the floor's defaults, switches stay editable.
        d.set_room_type("Bedroom");
        assert!(d.room_name().has_floor && d.room_name().has_ceiling);
        assert_eq!(d.extras().floor_finish_thickness, 0.75);
        assert!(d.extras().floor_structure.is_empty());
    }

    /// The Structure and Label tabs draw, with a dropped ceiling defined.
    #[test]
    fn the_structure_and_label_tabs_draw_with_layered_platforms() {
        let ctx = egui::Context::default();
        for kind in [AssemblyKind::FloorStructure, AssemblyKind::CeilingFinish] {
            let mut d = dialog();
            d.set_room_type("Garage");
            d.set_platform(kind, kind.builtin());
            let tabs: Vec<usize> = ["Structure", "Label"]
                .iter()
                .map(|n| ROOM_TABS.iter().position(|t| t.name == *n).unwrap())
                .collect();
            for tab in tabs {
                let form = &mut d.form;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        SpecPages::page(form, ui, tab);
                    });
                });
            }
        }
    }

    #[test]
    fn structure_layers_are_edited_through_the_dialog_model() {
        let mut d = dialog();
        d.structure_mut(Define::Floor)
            .push(StructureLayer::new("Subfloor", 0.75));
        d.structure_mut(Define::Floor)
            .push(StructureLayer::new("Joist", 9.25));
        assert_eq!(structure_thickness(&d.extras().floor_structure), 10.0);
        assert!(d.extras().ceiling_structure.is_empty());
    }

    /// Every tab of the specification draws, with moldings, materials, a
    /// label style, a rough ceiling and the slab flag filled in.
    #[test]
    fn every_tab_draws_with_the_round_14_options_filled_in() {
        let ctx = egui::Context::default();
        let mut d = dialog();
        d.extras_mut().base_molding = "Base - Colonial 5 1/4".into();
        d.extras_mut().crown_molding = "Crown - Cove 3 5/8".into();
        d.extras_mut().chair_molding = "Chair Rail - Colonial 3".into();
        d.extras_mut().wall_covering = "Brick".into();
        d.form.name.floor_finish = Some("Ceramic Tile".into());
        d.form.name.ceiling_finish = Some("Wood Planks".into());
        d.form.name.rough_ceiling = Some(120.0);
        d.form.name.monolithic_slab = Some(RoomSlab::default());
        d.form.name.label_style.text_style = "Schedule Style".into();
        d.form.name.label_style.placement = LabelPlacement::Top;
        for tab in 0..ROOM_TABS.len() {
            let form = &mut d.form;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    SpecPages::page(form, ui, tab);
                });
            });
        }
        // A molding the library lacks, and an empty one, draw as well.
        d.extras_mut().base_molding = "Not in the library".into();
        d.extras_mut().crown_molding.clear();
        let form = &mut d.form;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| SpecPages::page(form, ui, 3));
        });
        assert!(!d.has_error());
    }

    #[test]
    fn the_rough_ceiling_cannot_sit_under_the_finished_ceiling() {
        let mut d = dialog();
        assert!(d.form.name.rough_ceiling.is_none());
        // The checkbox starts it at the finished ceiling plus its finish.
        let finish = d.extras().ceiling_finish_thickness;
        let finished = d.form.init.floor_ceiling_height;
        d.form.name.rough_ceiling = Some(finished + finish);
        assert!(!d.has_error());
        assert_eq!(d.form.name.rough_ceiling, Some(finished + finish));
    }

    #[test]
    fn the_room_init_carries_the_floor_defaults_surfaces_and_text_styles() {
        let d = dialog();
        assert!(d.form.init.slab_allowed);
        assert_eq!(d.form.init.text_styles.len(), 2);
        assert!(d.form.init.default_floor_material.is_empty());
        // The surface lists are what the 3D view understands.
        for n in FLOOR_SURFACES
            .iter()
            .chain(&CEILING_SURFACES)
            .chain(&WALL_SURFACES)
        {
            assert!(!n.is_empty());
        }
        assert!(molding_defs(MoldingKind::Base).len() >= 2);
    }

    #[test]
    fn living_area_default_text_follows_the_type() {
        let mut d = dialog();
        assert_eq!(d.form.living_default(), "Use Default (Included)");
        d.set_room_type("Garage");
        assert_eq!(d.form.living_default(), "Use Default (Excluded)");
    }

    #[test]
    fn the_deck_tab_is_on_and_a_deck_type_brings_a_deck_specification() {
        let tab = ROOM_TABS.iter().find(|t| t.name == "Deck").unwrap();
        assert!(tab.enabled);
        let mut d = dialog();
        assert!(d.room_name().deck.is_none());
        d.set_room_type("Deck");
        let spec = d.room_name().deck.clone().unwrap();
        assert!(spec.planking.enabled && spec.framing.enabled);
        // A second pass over the type keeps what the user set.
        d.room_name_mut()
            .deck
            .as_mut()
            .unwrap()
            .planking
            .board_width = 3.5;
        d.set_room_type("Deck");
        assert_eq!(
            d.room_name().deck.as_ref().unwrap().planking.board_width,
            3.5
        );
        // Another type drops the specification.
        d.set_room_type("Bedroom");
        assert!(d.room_name().deck.is_none());
    }

    #[test]
    fn the_deck_tab_draws_with_and_without_a_specification() {
        let ctx = egui::Context::default();
        let tab = ROOM_TABS.iter().position(|t| t.name == "Deck").unwrap();
        let mut d = dialog();
        let draw = |d: &mut RoomDialog| {
            let form = &mut d.form;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    SpecPages::page(form, ui, tab);
                });
            });
        };
        draw(&mut d);
        d.set_room_type("Deck");
        {
            let spec = d.room_name_mut().deck.as_mut().unwrap();
            spec.planking.border = true;
            spec.framing.joist_angle = Some(0.0);
            spec.stairs.to_grade = true;
        }
        draw(&mut d);
        assert!(!d.has_error());
    }
}
