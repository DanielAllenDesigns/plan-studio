//! Floor Defaults (R-56, R-58, R-117, R-118): the ceiling height, platform and
//! finish thicknesses, the Moldings and Fill Style panels, default room type
//! and materials of a floor. Opened from the
//! Floor Defaults toolbar button and Build > Floor > Floor Defaults for the
//! active floor, and from Edit > Default Settings > Floors and Rooms > Floor
//! Defaults for the defaults every new floor starts with.

use super::assembly_def::{
    floor_inherited, floor_legacy, AssemblyDefDialog, PlatformEditor, RowContext,
};
use super::molding::{MoldingPanel, PanelOptions};
use super::{row, section, Fields, Outcome, ERROR_RED};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::assemblies::{Assembly, AssemblyKind, AssemblyLibrary, AssemblySlot, NamedAssembly};
use plan_core::extras::RoomFill;
use plan_core::floors::FloorSettings;
use plan_core::moldings::{builtin_profiles, MoldingTable};
use plan_core::units::fmt_ft_in;

/// The panels of the dialog (manual p. 762): Structure, Moldings, Fill Style
/// and Materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FloorPanel {
    #[default]
    Structure,
    Moldings,
    FillStyle,
    Materials,
}

impl FloorPanel {
    pub const ALL: [FloorPanel; 4] = [
        FloorPanel::Structure,
        FloorPanel::Moldings,
        FloorPanel::FillStyle,
        FloorPanel::Materials,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FloorPanel::Structure => "Structure",
            FloorPanel::Moldings => "Moldings",
            FloorPanel::FillStyle => "Fill Style",
            FloorPanel::Materials => "Materials",
        }
    }
}

/// The fill patterns the Fill Style panel offers (the room's own list).
const FILL_PATTERNS: [&str; 5] = ["None", "Solid", "Hatch", "Cross Hatch", "Grid"];

/// What the dialog edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FloorDefaultsTarget {
    /// The active floor (its name is shown).
    ThisFloor(String),
    /// The defaults of floors built from now on.
    PlanDefaults,
}

pub struct FloorDefaultsDialog {
    target: FloorDefaultsTarget,
    ceiling_height: f64,
    settings: FloorSettings,
    /// Also make the values the plan defaults (this-floor target only).
    as_plan_default: bool,
    /// Names of the plan's room types, for the default room type list.
    types: Vec<String>,
    fields: Fields,
    /// The plan's assembly library: the plan-wide platform definitions a
    /// floor that follows the default reads, and the saved definitions.
    library: AssemblyLibrary,
    /// The four platform rows (Round 16) and the layers window they open.
    editor: PlatformEditor,
    panel: FloorPanel,
    /// The Moldings panel's table of this floor's defaults (this-floor target
    /// only; `None` when the dialog was opened without one).
    moldings: Option<MoldingTable>,
    molding_panel: MoldingPanel,
    moldings_original: Option<MoldingTable>,
}

// The setters are the dialog's model API (the tests drive it); the UI edits
// the same fields directly.
#[allow(dead_code)]
impl FloorDefaultsDialog {
    pub fn new(
        target: FloorDefaultsTarget,
        ceiling_height: f64,
        settings: FloorSettings,
        types: Vec<String>,
    ) -> Self {
        Self {
            target,
            ceiling_height,
            settings,
            as_plan_default: false,
            types,
            fields: Fields::default(),
            library: AssemblyLibrary::default(),
            editor: PlatformEditor::default(),
            panel: FloorPanel::Structure,
            moldings: None,
            molding_panel: MoldingPanel::new(PanelOptions::room(), builtin_profiles()),
            moldings_original: None,
        }
    }

    /// The dialog with the floor's default moldings, so its Moldings panel
    /// edits them.
    pub fn with_moldings(mut self, table: MoldingTable) -> Self {
        self.moldings_original = Some(table.clone());
        self.moldings = Some(table);
        self
    }

    /// The moldings table when the Moldings panel changed it.
    pub fn changed_moldings(&self) -> Option<&MoldingTable> {
        match (&self.moldings, &self.moldings_original) {
            (Some(m), Some(o)) if m != o => Some(m),
            (Some(m), None) => Some(m),
            _ => None,
        }
    }

    /// The Moldings panel's table, to edit it without a frame.
    pub fn moldings_mut(&mut self) -> Option<&mut MoldingTable> {
        self.moldings.as_mut()
    }

    /// The panel showing (for the tests).
    pub fn panel(&self) -> FloorPanel {
        self.panel
    }

    pub fn set_panel(&mut self, panel: FloorPanel) {
        self.panel = panel;
    }

    /// The dialog on the plan's assembly library, so Use Default can show
    /// the plan-wide definitions and the layers window can offer the saved
    /// ones.
    pub fn with_library(mut self, library: AssemblyLibrary) -> Self {
        self.library = library;
        self.settings.sync_thicknesses(&self.library);
        self
    }

    pub fn library(&self) -> &AssemblyLibrary {
        &self.library
    }

    /// Definitions saved from the layers window, for the plan's library.
    pub fn take_saved(&mut self) -> Vec<NamedAssembly> {
        self.editor.take_saved()
    }

    /// Gives the floor level its own definition of `kind` (what the layers
    /// window's OK does).
    pub fn set_platform(&mut self, kind: AssemblyKind, assembly: Assembly) {
        self.settings
            .platform
            .set(kind, AssemblySlot::Own(assembly));
        self.settings.sync_thicknesses(&self.library);
    }

    /// Use Default on or off for `kind`: on follows the plan-wide
    /// definition; off keeps a copy of what it gave as the level's own.
    pub fn set_follow_default(&mut self, kind: AssemblyKind, follow: bool) {
        if follow {
            self.settings.platform.set(kind, AssemblySlot::Default);
        } else if let Some(a) = self.library_effective(kind) {
            self.settings.platform.set(kind, AssemblySlot::Own(a));
        }
        self.settings.sync_thicknesses(&self.library);
    }

    /// The definition of `kind` this floor level has now.
    pub fn library_effective(&self, kind: AssemblyKind) -> Option<Assembly> {
        let cx = self.row_context();
        Some(cx.effective(kind, self.settings.platform.slot(kind)))
    }

    fn row_context(&self) -> RowContext<'_> {
        RowContext {
            allow_default: true,
            inherited: floor_inherited(&self.library, &self.settings),
            legacy: floor_legacy(&self.settings),
            library: &self.library,
        }
    }

    /// The layers window, if one is open (for the tests).
    pub fn editing(&self) -> bool {
        self.editor.is_editing()
    }

    /// Opens the layers window on `kind` (the Edit button).
    pub fn open_editor(&mut self, kind: AssemblyKind) {
        let cx = RowContext {
            allow_default: true,
            inherited: floor_inherited(&self.library, &self.settings),
            legacy: floor_legacy(&self.settings),
            library: &self.library,
        };
        self.editor.open(kind, &self.settings.platform, &cx);
    }

    /// Takes the open layers window's definition as if OK was pressed.
    pub fn accept_editor(&mut self, window: AssemblyDefDialog) {
        self.editor.accept(window, &mut self.settings.platform);
        self.settings.sync_thicknesses(&self.library);
    }

    pub fn target(&self) -> &FloorDefaultsTarget {
        &self.target
    }

    pub fn ceiling_height(&self) -> f64 {
        self.ceiling_height
    }

    pub fn settings(&self) -> &FloorSettings {
        &self.settings
    }

    pub fn as_plan_default(&self) -> bool {
        self.as_plan_default
    }

    pub fn set_ceiling_height(&mut self, v: f64) {
        self.ceiling_height = v;
    }

    pub fn settings_mut(&mut self) -> &mut FloorSettings {
        &mut self.settings
    }

    fn error(&self) -> Option<&'static str> {
        if self.fields.any_invalid() {
            Some("Fix the highlighted field")
        } else if self.ceiling_height <= 0.0 {
            Some("Ceiling height must be greater than zero")
        } else if self.settings.floor_structure_thickness < 0.0
            || self.settings.ceiling_structure_thickness < 0.0
        {
            Some("Structure thickness cannot be negative")
        } else {
            None
        }
    }

    /// The Moldings panel: the base, chair rail and crown moldings new rooms
    /// on this floor start with.
    fn moldings_panel(&mut self, ui: &mut egui::Ui) {
        match self.moldings.as_mut() {
            Some(table) => {
                let _ = self.molding_panel.show(ui, table);
            }
            None => {
                section(ui, "Moldings");
                ui.weak(
                    "The moldings are set for each floor: open Floor Defaults on a floor to \
                     edit its moldings.",
                );
            }
        }
    }

    /// The Fill Style panel: the plan fill new rooms on this floor start
    /// with (a room with a fill of its own keeps it).
    fn fill_style_panel(&mut self, ui: &mut egui::Ui) {
        section(ui, "Fill Style");
        let mut fill = self.settings.room_fill.clone().unwrap_or(RoomFill {
            color: [0xC8, 0xB4, 0x8C],
            pattern: "None".into(),
            alpha: 1.0,
        });
        row(ui, "Pattern", |ui| {
            egui::ComboBox::from_id_salt("fd_fill_pattern")
                .selected_text(fill.pattern.clone())
                .show_ui(ui, |ui| {
                    for p in FILL_PATTERNS {
                        ui.selectable_value(&mut fill.pattern, p.to_string(), p);
                    }
                });
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut fill.color);
        });
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut fill.alpha, 0.1..=1.0));
        });
        self.settings.room_fill = (fill.pattern != "None").then_some(fill);
        ui.weak("Rooms without a fill style of their own on this floor draw with it.");
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        let title = match &self.target {
            FloorDefaultsTarget::ThisFloor(_) => "Floor Defaults",
            FloorDefaultsTarget::PlanDefaults => "Floor Defaults (new floors)",
        };
        egui::Window::new(title)
            .id(egui::Id::new("floor_defaults_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                if let FloorDefaultsTarget::ThisFloor(name) = &self.target {
                    ui.label(RichText::new(name.as_str()).strong());
                }
                ui.horizontal(|ui| {
                    for p in FloorPanel::ALL {
                        ui.selectable_value(&mut self.panel, p, p.name());
                    }
                });
                ui.separator();
                if self.panel == FloorPanel::Structure {
                    section(ui, "Heights");
                    self.fields.length_row(
                        ui,
                        "Ceiling Height",
                        "fd_ceiling",
                        &mut self.ceiling_height,
                    );
                    row(ui, "Floor Height", |ui| {
                        ui.label(fmt_ft_in(self.settings.floor_height(self.ceiling_height)));
                    });
                    ui.weak(
                    "Floors above move with the platforms; walls at the old ceiling height follow.",
                );

                    section(ui, "Floor and Ceiling Platforms");
                    let cx = RowContext {
                        allow_default: true,
                        inherited: floor_inherited(&self.library, &self.settings),
                        legacy: floor_legacy(&self.settings),
                        library: &self.library,
                    };
                    let before = self.settings.platform.clone();
                    let mut slots = before.clone();
                    self.editor.rows(ui, &mut slots, &cx);
                    if slots != before {
                        self.settings.platform = slots;
                        self.settings.sync_thicknesses(&self.library);
                    }
                    // A platform nobody has laid out in layers keeps the single
                    // thickness an older plan stored.
                    for (kind, label, key) in [
                        (
                            AssemblyKind::FloorStructure,
                            "Floor Structure Thickness",
                            "fd_floor_struct",
                        ),
                        (
                            AssemblyKind::CeilingStructure,
                            "Ceiling Structure Thickness",
                            "fd_ceil_struct",
                        ),
                        (
                            AssemblyKind::FloorFinish,
                            "Floor Finish Thickness",
                            "fd_floor_fin",
                        ),
                        (
                            AssemblyKind::CeilingFinish,
                            "Ceiling Finish Thickness",
                            "fd_ceil_fin",
                        ),
                    ] {
                        if self.settings.platform.slot(kind).is_legacy() {
                            let field = match kind {
                                AssemblyKind::FloorStructure => {
                                    &mut self.settings.floor_structure_thickness
                                }
                                AssemblyKind::CeilingStructure => {
                                    &mut self.settings.ceiling_structure_thickness
                                }
                                AssemblyKind::FloorFinish => {
                                    &mut self.settings.floor_finish_thickness
                                }
                                _ => &mut self.settings.ceiling_finish_thickness,
                            };
                            self.fields.length_row(ui, label, key, field);
                        }
                    }
                }
                match self.panel {
                    FloorPanel::Moldings => self.moldings_panel(ui),
                    FloorPanel::FillStyle => self.fill_style_panel(ui),
                    _ => {}
                }
                if self.panel == FloorPanel::Materials {
                    section(ui, "New Rooms");
                    row(ui, "Default Room Type", |ui| {
                        let shown = if self.settings.default_room_type.is_empty() {
                            "(first in the list)".to_string()
                        } else {
                            self.settings.default_room_type.clone()
                        };
                        egui::ComboBox::from_id_salt("fd_room_type")
                            .width(200.0)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.default_room_type,
                                    String::new(),
                                    "(first in the list)",
                                );
                                for t in &self.types {
                                    ui.selectable_value(
                                        &mut self.settings.default_room_type,
                                        t.clone(),
                                        t,
                                    );
                                }
                            });
                    });
                    row(ui, "Floor Material", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.floor_material)
                                .desired_width(200.0),
                        );
                    });
                    row(ui, "Ceiling Material", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.ceiling_material)
                                .desired_width(200.0),
                        );
                    });
                    row(ui, "Wall Material", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.wall_material)
                                .desired_width(200.0),
                        );
                    });
                }
                if matches!(self.target, FloorDefaultsTarget::ThisFloor(_)) {
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.as_plan_default,
                        "Use these for floors built from now on",
                    );
                }
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(RichText::new("   OK   ").strong());
                    if ui.add_enabled(error.is_none(), ok).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if self.editor.child(ctx, &mut self.settings.platform) {
            self.settings.sync_thicknesses(&self.library);
        }
        if !open {
            outcome = Outcome::Cancel;
        }
        // The layers window owns Enter and Escape while it is open.
        if self.editor.is_editing() {
            return Outcome::Open;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_height_is_ceiling_plus_both_platforms() {
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::PlanDefaults,
            96.0,
            FloorSettings::default(),
            vec!["Bedroom".into()],
        );
        assert_eq!(d.settings().floor_height(d.ceiling_height()), 96.0 + 10.25);
        d.settings_mut().ceiling_structure_thickness = 1.0;
        assert_eq!(d.settings().floor_height(d.ceiling_height()), 96.0 + 11.25);
        assert!(d.error().is_none());
        d.set_ceiling_height(0.0);
        assert!(d.error().is_some());
    }

    fn twelve_inch_floor() -> Assembly {
        use plan_core::assemblies::{AssemblyLayer, LayerRole};
        Assembly::new(vec![
            AssemblyLayer::new("3/4 OSB", LayerRole::Sheathing, 0.75),
            AssemblyLayer::new("2x12", LayerRole::Framing, 11.25),
        ])
    }

    #[test]
    fn a_layered_floor_structure_sets_the_floor_height() {
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::PlanDefaults,
            96.0,
            FloorSettings::default(),
            vec![],
        );
        d.set_platform(AssemblyKind::FloorStructure, twelve_inch_floor());
        assert_eq!(d.settings().floor_structure_thickness, 12.0);
        assert_eq!(d.settings().floor_height(96.0), 96.0 + 12.0);
        // A tile finish is the finish thickness.
        d.set_platform(
            AssemblyKind::FloorFinish,
            Assembly::from_thickness("Tile", plan_core::assemblies::LayerRole::Finish, 0.875),
        );
        assert_eq!(d.settings().floor_finish_thickness, 0.875);
    }

    #[test]
    fn use_default_follows_the_plan_wide_definition_and_off_keeps_a_copy() {
        let mut lib = AssemblyLibrary::default();
        lib.plan_wide.set(
            AssemblyKind::FloorStructure,
            AssemblySlot::Own(twelve_inch_floor()),
        );
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::ThisFloor("1st Floor".into()),
            96.0,
            FloorSettings::default(),
            vec![],
        )
        .with_library(lib);
        // Nothing follows yet: the floor keeps its own 10 1/4 in.
        assert_eq!(d.settings().floor_structure_thickness, 10.25);
        d.set_follow_default(AssemblyKind::FloorStructure, true);
        assert_eq!(d.settings().floor_structure_thickness, 12.0);
        assert_eq!(d.settings().platform.floor_structure, AssemblySlot::Default);
        d.set_follow_default(AssemblyKind::FloorStructure, false);
        assert_eq!(
            d.settings().platform.floor_structure,
            AssemblySlot::Own(twelve_inch_floor())
        );
        assert_eq!(d.settings().floor_structure_thickness, 12.0);
    }

    #[test]
    fn the_dialog_draws_its_rows_and_the_layers_window() {
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::ThisFloor("1st Floor".into()),
            96.0,
            FloorSettings::default(),
            vec!["Bedroom".into()],
        );
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), Outcome::Open);
        });
        for kind in AssemblyKind::PLATFORM {
            d.open_editor(kind);
            assert!(d.editing());
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
    }

    #[test]
    fn the_moldings_and_fill_style_panels_edit_the_floor_defaults() {
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::ThisFloor("1st Floor".into()),
            96.0,
            FloorSettings::default(),
            vec![],
        )
        .with_moldings(MoldingTable::default());
        assert_eq!(d.panel(), FloorPanel::Structure);
        assert!(d.changed_moldings().is_none(), "nothing edited yet");
        let ctx = egui::Context::default();
        for p in FloorPanel::ALL {
            d.set_panel(p);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
        // The Fill Style panel keeps a room fill only when it has a pattern.
        assert!(d.settings().room_fill.is_none());
        d.settings_mut().room_fill = Some(RoomFill {
            color: [10, 20, 30],
            pattern: "Hatch".into(),
            alpha: 0.5,
        });
        d.set_panel(FloorPanel::FillStyle);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = d.show(ctx);
        });
        assert_eq!(d.settings().room_fill.as_ref().unwrap().pattern, "Hatch");
        // The Moldings panel of a plan default has no table to edit.
        let mut plan = FloorDefaultsDialog::new(
            FloorDefaultsTarget::PlanDefaults,
            96.0,
            FloorSettings::default(),
            vec![],
        );
        plan.set_panel(FloorPanel::Moldings);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(plan.show(ctx), Outcome::Open);
        });
        assert!(plan.changed_moldings().is_none());
    }
}
