//! Room Type Defaults (R-99; manual pp. 444 to 446): the settings of one Room
//! Type, in the panels of the Room Specification that the type hands to its
//! rooms: General (name, function, living and conditioned inclusion), the
//! floor and ceiling Structure and Finish, the Deck framing, Moldings, Layer
//! and Drawing Group, Fill Style and Label. With several types selected the
//! Multiple Room Type Defaults dialog edits what they share: function,
//! living and conditioned inclusion, moldings, layer and fill.
//!
//! The Room Types list (`default_lists.rs`) opens it from Edit. The dialog
//! works on a copy of the type; OK hands the copy back.

use super::assembly_def::{floor_inherited, floor_legacy, PlatformEditor, RowContext};
use super::room::{deck_panel, moldings_panel};
use super::{
    on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, ERROR_RED, PV_FAINT, PV_INK,
    PV_WALL,
};
use crate::editor::rooms_edit::{FillPattern, FillStyle};
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::assemblies::{AssemblyLibrary, PlatformAssemblies};
use plan_core::defaults::RoomTypeDef;
use plan_core::extras::{AreaKind, MoldingKind, MoldingRef, RoomLabelOptions};
use plan_core::floors::FloorSettings;
use plan_core::rooms::{function_class, function_defaults, molding_def, ROOM_FUNCTIONS};

const SINGLE_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Deck"),
    on("Moldings"),
    on("Layer"),
    on("Fill Style"),
    on("Label"),
];

/// With several types selected only what they can share is offered.
const MULTI_TABS: &[Tab] = &[on("General"), on("Moldings"), on("Layer"), on("Fill Style")];

/// What the dialog needs from the plan besides the type.
#[derive(Clone, Default)]
pub struct TypeContext {
    pub library: AssemblyLibrary,
    pub floor: FloorSettings,
    pub layers: Vec<String>,
    /// The other types' names (a name is required and cannot repeat).
    pub other_names: Vec<String>,
}

/// The settings a Multiple Room Type Defaults dialog changes; `None` is
/// "No Change".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MultiEdit {
    pub function: Option<String>,
    pub living: Option<bool>,
    pub conditioned: Option<bool>,
    pub layer: Option<String>,
    pub drawing_group: Option<String>,
    pub fill: Option<FillStyle>,
    /// Base, chair rail and crown profile names.
    pub moldings: Option<[String; 3]>,
}

impl MultiEdit {
    /// Applies the changes to a type.
    pub fn apply(&self, t: &mut RoomTypeDef) {
        if let Some(f) = &self.function {
            t.function = f.clone();
        }
        if let Some(b) = self.living {
            t.include_in_living_area = b;
        }
        if let Some(b) = self.conditioned {
            t.conditioned = b;
        }
        if let Some(l) = &self.layer {
            t.spec.layer = l.clone();
        }
        if let Some(g) = &self.drawing_group {
            t.spec.drawing_group = g.clone();
        }
        if let Some(f) = &self.fill {
            t.spec.fill = f.to_room();
        }
        if let Some(m) = &self.moldings {
            t.spec.moldings = moldings_of(m);
        }
    }
}

fn moldings_of(names: &[String; 3]) -> Vec<MoldingRef> {
    [
        (MoldingKind::Base, &names[0]),
        (MoldingKind::Chair, &names[1]),
        (MoldingKind::Crown, &names[2]),
    ]
    .into_iter()
    .filter(|(_, n)| !n.trim().is_empty())
    .map(|(kind, n)| MoldingRef {
        kind,
        profile: n.clone(),
        height: molding_def(n).map_or(3.0, |d| d.height()),
    })
    .collect()
}

fn molding_names(t: &RoomTypeDef) -> [String; 3] {
    let find = |kind| {
        t.spec
            .moldings
            .iter()
            .find(|m| m.kind == kind)
            .map(|m| m.profile.clone())
            .unwrap_or_default()
    };
    [
        find(MoldingKind::Base),
        find(MoldingKind::Chair),
        find(MoldingKind::Crown),
    ]
}

/// Room Type Defaults for one type, or Multiple Room Type Defaults.
pub struct RoomTypeDialog {
    frame: SpecDialog,
    form: TypeForm,
}

struct TypeForm {
    /// The type being edited (a placeholder when several are).
    def: RoomTypeDef,
    ctx: TypeContext,
    fields: Fields,
    editor: PlatformEditor,
    molds: [String; 3],
    fill: FillStyle,
    label: RoomLabelOptions,
    /// How many types are edited (1 is the ordinary dialog).
    count: usize,
    multi: MultiEdit,
    shown: std::cell::Cell<usize>,
}

impl RoomTypeDialog {
    /// Room Type Defaults of `def`.
    pub fn new(def: RoomTypeDef, ctx: TypeContext) -> Self {
        let form = TypeForm {
            molds: molding_names(&def),
            fill: FillStyle::from_room(def.spec.fill.as_ref()),
            label: def.spec.label.clone().unwrap_or_default(),
            def,
            ctx,
            fields: Fields::default(),
            editor: PlatformEditor::default(),
            count: 1,
            multi: MultiEdit::default(),
            shown: std::cell::Cell::new(0),
        };
        Self {
            frame: SpecDialog::new("Room Type Defaults", "room_type_defaults"),
            form,
        }
    }

    /// Multiple Room Type Defaults for `count` types.
    pub fn multiple(count: usize, ctx: TypeContext) -> Self {
        let mut d = Self::new(RoomTypeDef::new("", "Standard", true, true), ctx);
        d.form.count = count.max(2);
        d.frame = SpecDialog::new("Multiple Room Type Defaults", "room_type_defaults_multi");
        d
    }

    pub fn is_multiple(&self) -> bool {
        self.form.count > 1
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let f = &mut self.form;
        f.editor.child(ctx, &mut f.def.spec.assemblies);
        self.frame.show(ctx, f)
    }

    /// The edited type, with the panels' choices written into it.
    pub fn result(&self) -> RoomTypeDef {
        let f = &self.form;
        let mut t = f.def.clone();
        t.name = t.name.trim().to_string();
        t.spec.moldings = moldings_of(&f.molds);
        t.spec.fill = f.fill.to_room();
        t.spec.label = (f.label != RoomLabelOptions::default()).then(|| f.label.clone());
        t
    }

    /// The changes of a Multiple Room Type Defaults dialog.
    pub fn changes(&self) -> MultiEdit {
        self.form.multi.clone()
    }

    /// Definitions saved from the layers window, for the plan's library.
    pub fn take_saved(&mut self) -> Vec<plan_core::assemblies::NamedAssembly> {
        self.form.editor.take_saved()
    }

    // The model API the tests drive.

    #[allow(dead_code)]
    pub fn def_mut(&mut self) -> &mut RoomTypeDef {
        &mut self.form.def
    }

    #[allow(dead_code)]
    pub fn multi_mut(&mut self) -> &mut MultiEdit {
        &mut self.form.multi
    }

    #[allow(dead_code)]
    pub fn set_function(&mut self, function: &str) {
        self.form.set_function(function);
    }

    #[allow(dead_code)]
    pub fn has_error(&self) -> bool {
        self.form.error().is_some()
    }
}

impl TypeForm {
    /// Choosing a function gives the type its living and conditioned
    /// defaults (manual p. 446); both stay editable.
    fn set_function(&mut self, function: &str) {
        self.def.function = function.to_string();
        let d = function_defaults(function, &self.def.name);
        self.def.include_in_living_area = d.living_area;
        self.def.conditioned = d.conditioned;
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.def.name).desired_width(200.0));
        });
        row(ui, "Function", |ui| {
            let mut picked: Option<&'static str> = None;
            egui::ComboBox::from_id_salt("type_function")
                .width(200.0)
                .selected_text(self.def.function.clone())
                .show_ui(ui, |ui| {
                    for (name, class) in ROOM_FUNCTIONS {
                        let text = format!("{name} ({})", class.name());
                        if ui
                            .selectable_label(self.def.function == name, text)
                            .clicked()
                        {
                            picked = Some(name);
                        }
                    }
                });
            if let Some(f) = picked {
                self.set_function(f);
            }
        });
        let class = function_class(&self.def.function, &self.def.name);
        let d = function_defaults(&self.def.function, &self.def.name);
        ui.weak(format!(
            "{} function. By default {}, {}, {}, {}.",
            class.name(),
            if d.living_area {
                "in the Living Area"
            } else {
                "out of the Living Area"
            },
            if d.conditioned {
                "conditioned"
            } else {
                "unconditioned"
            },
            if d.has_ceiling {
                "with a ceiling"
            } else {
                "no ceiling"
            },
            if d.roof_over {
                "with a roof"
            } else {
                "no roof"
            },
        ));
        section(ui, "Living Area");
        ui.checkbox(
            &mut self.def.include_in_living_area,
            "Include in Living Area",
        );
        section(ui, "Conditioned Room");
        ui.checkbox(&mut self.def.conditioned, "Conditioned");
        section(ui, "Floor Finish");
        row(ui, "Default Floor Finish", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.def.default_floor_finish).desired_width(160.0),
            );
        });
    }

    fn structure(&mut self, ui: &mut Ui) {
        section(ui, "Floor and Ceiling Structure and Finish");
        ui.weak("Only the floor and ceiling settings are available here. A room given this type takes these definitions; Use Default follows the floor level.");
        let cx = RowContext {
            allow_default: true,
            inherited: floor_inherited(&self.ctx.library, &self.ctx.floor),
            legacy: floor_legacy(&self.ctx.floor),
            library: &self.ctx.library,
        };
        let mut slots: PlatformAssemblies = self.def.spec.assemblies.clone();
        self.editor.rows(ui, &mut slots, &cx);
        self.def.spec.assemblies = slots;
    }

    fn deck(&mut self, ui: &mut Ui) {
        let on = deck_panel(
            ui,
            &mut self.fields,
            &mut self.def.spec.deck,
            0,
            "Room Type",
        );
        let _ = on;
    }

    fn moldings(&mut self, ui: &mut Ui) {
        let [b, c, k] = &mut self.molds;
        moldings_panel(ui, b, c, k);
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            let shown = if self.def.spec.layer.is_empty() {
                plan_core::rooms::ROOM_LAYER.to_string()
            } else {
                self.def.spec.layer.clone()
            };
            egui::ComboBox::from_id_salt("type_layer")
                .width(200.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.def.spec.layer,
                        String::new(),
                        plan_core::rooms::ROOM_LAYER,
                    );
                    for l in &self.ctx.layers {
                        if l != plan_core::rooms::ROOM_LAYER {
                            ui.selectable_value(&mut self.def.spec.layer, l.clone(), l);
                        }
                    }
                });
        });
        row(ui, "Drawing Group", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.def.spec.drawing_group)
                    .desired_width(200.0)
                    .hint_text("Rooms"),
            );
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        fill_panel(ui, &mut self.fill);
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Display in All Views");
        let l = &mut self.label;
        ui.checkbox(&mut l.show_name, "Room Name");
        ui.checkbox(&mut l.show_dimensions, "Interior Dimensions");
        ui.checkbox(&mut l.show_area, "Area");
        ui.add_enabled_ui(l.show_area, |ui| {
            ui.radio_value(&mut l.area_kind, AreaKind::Interior, "Interior Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Standard, "Standard Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Centerline, "Centerline Area");
        });
        section(ui, "Label Text");
        ui.add(
            egui::TextEdit::multiline(&mut l.template)
                .desired_rows(2)
                .desired_width(260.0)
                .hint_text("%dimensions%\\n%internal_area%"),
        );
        let macros: Vec<String> = crate::editor::rooms_edit::LABEL_MACROS
            .iter()
            .map(|(m, what)| format!("{m} {what}"))
            .collect();
        ui.weak(format!(
            "Macros: {}. Empty uses the choices above.",
            macros.join(", ")
        ));
    }

    // ----- Multiple Room Type Defaults -----

    /// A tri-state choice: No Change, or one of two values.
    fn tri(ui: &mut Ui, label: &str, value: &mut Option<bool>, on: &str, off: &str) {
        row(ui, label, |ui| {
            ui.radio_value(value, None, "No Change");
            ui.radio_value(value, Some(true), on);
            ui.radio_value(value, Some(false), off);
        });
    }

    fn multi_general(&mut self, ui: &mut Ui) {
        section(ui, "Selected Room Types");
        ui.weak(format!(
            "{} room types are selected. Only the settings they can share are available.",
            self.form_count()
        ));
        row(ui, "Function", |ui| {
            let shown = self
                .multi
                .function
                .clone()
                .unwrap_or_else(|| "No Change".into());
            egui::ComboBox::from_id_salt("multi_function")
                .width(200.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.multi.function, None, "No Change");
                    for (name, class) in ROOM_FUNCTIONS {
                        ui.selectable_value(
                            &mut self.multi.function,
                            Some(name.to_string()),
                            format!("{name} ({})", class.name()),
                        );
                    }
                });
        });
        Self::tri(
            ui,
            "Living Area",
            &mut self.multi.living,
            "Included",
            "Excluded",
        );
        Self::tri(
            ui,
            "Conditioned",
            &mut self.multi.conditioned,
            "Conditioned",
            "Unconditioned",
        );
    }

    fn form_count(&self) -> usize {
        self.count
    }

    fn multi_moldings(&mut self, ui: &mut Ui) {
        let mut change = self.multi.moldings.is_some();
        if ui.checkbox(&mut change, "Change the moldings").changed() {
            self.multi.moldings = change.then(Default::default);
        }
        if let Some([b, c, k]) = self.multi.moldings.as_mut() {
            moldings_panel(ui, b, c, k);
        }
    }

    fn multi_layer(&mut self, ui: &mut Ui) {
        let mut change = self.multi.layer.is_some();
        if ui.checkbox(&mut change, "Change the layer").changed() {
            self.multi.layer = change.then(String::new);
            self.multi.drawing_group = change.then(String::new);
        }
        if let (Some(layer), Some(group)) =
            (self.multi.layer.as_mut(), self.multi.drawing_group.as_mut())
        {
            row(ui, "Layer", |ui| {
                egui::ComboBox::from_id_salt("multi_layer")
                    .width(200.0)
                    .selected_text(if layer.is_empty() {
                        plan_core::rooms::ROOM_LAYER
                    } else {
                        layer.as_str()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(layer, String::new(), plan_core::rooms::ROOM_LAYER);
                        for l in &self.ctx.layers {
                            ui.selectable_value(layer, l.clone(), l);
                        }
                    });
            });
            row(ui, "Drawing Group", |ui| {
                ui.add(egui::TextEdit::singleline(group).desired_width(200.0));
            });
        }
    }

    fn multi_fill(&mut self, ui: &mut Ui) {
        let mut change = self.multi.fill.is_some();
        if ui.checkbox(&mut change, "Change the fill style").changed() {
            self.multi.fill = change.then(FillStyle::default);
        }
        if let Some(f) = self.multi.fill.as_mut() {
            fill_panel(ui, f);
        }
    }
}

fn fill_panel(ui: &mut Ui, fill: &mut FillStyle) {
    section(ui, "Fill Style");
    row(ui, "Pattern", |ui| {
        egui::ComboBox::from_id_salt("type_fill")
            .selected_text(fill.pattern.name())
            .show_ui(ui, |ui| {
                for p in FillPattern::ALL {
                    ui.selectable_value(&mut fill.pattern, p, p.name());
                }
            });
    });
    row(ui, "Color", |ui| {
        ui.color_edit_button_srgb(&mut fill.color);
    });
    row(ui, "Opacity", |ui| {
        ui.add(egui::Slider::new(&mut fill.alpha, 0.1..=1.0));
    });
    ui.weak("Drawn in the plan view only.");
}

impl SpecPages for TypeForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.count > 1 {
            MULTI_TABS
        } else {
            SINGLE_TABS
        }
    }

    fn error(&self) -> Option<String> {
        if self.count > 1 {
            return None;
        }
        let name = self.def.name.trim();
        if name.is_empty() {
            return Some("A name is required".into());
        }
        if self.ctx.other_names.iter().any(|n| n == name) {
            return Some("That name is already used".into());
        }
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        self.shown.set(tab);
        let name = self.tabs()[tab].name;
        if self.count > 1 {
            match name {
                "General" => self.multi_general(ui),
                "Moldings" => self.multi_moldings(ui),
                "Layer" => self.multi_layer(ui),
                "Fill Style" => self.multi_fill(ui),
                _ => {}
            }
            return;
        }
        match name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Deck" => self.deck(ui),
            "Moldings" => self.moldings(ui),
            "Layer" => self.layer(ui),
            "Fill Style" => self.fill_style(ui),
            "Label" => self.label(ui),
            _ => {}
        }
        if let Some(e) = self.error() {
            ui.colored_label(ERROR_RED, e);
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        // A swatch of the type: its fill and its name.
        let r = rect.shrink(10.0);
        let [cr, cg, cb] = self.fill.color;
        let fill = if self.fill.pattern == FillPattern::None {
            PV_WALL
        } else {
            egui::Color32::from_rgb(cr, cg, cb)
        };
        p.rect_filled(r, 2.0, fill);
        p.rect_stroke(
            r,
            2.0,
            egui::Stroke::new(1.5_f32, PV_INK),
            egui::StrokeKind::Inside,
        );
        let text = if self.count > 1 {
            format!("{} room types", self.count)
        } else {
            self.def.name.clone()
        };
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(13.0),
            PV_INK,
        );
        p.text(
            egui::Pos2::new(r.center().x, r.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            if self.count > 1 {
                String::new()
            } else {
                self.def.function.clone()
            },
            egui::FontId::proportional(10.0),
            PV_FAINT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TypeContext {
        TypeContext {
            floor: FloorSettings::default(),
            layers: vec!["Rooms".into(), "CAD, Default".into()],
            other_names: vec!["Kitchen".into()],
            ..TypeContext::default()
        }
    }

    fn draw(d: &mut RoomTypeDialog) {
        let c = egui::Context::default();
        let n = d.form.tabs().len();
        for tab in 0..n {
            let f = &mut d.form;
            let _ = c.run(egui::RawInput::default(), |c| {
                egui::CentralPanel::default().show(c, |ui| SpecPages::page(f, ui, tab));
            });
        }
    }

    #[test]
    fn a_function_gives_the_type_its_living_and_conditioned_defaults() {
        let mut d = RoomTypeDialog::new(RoomTypeDef::new("Sunroom", "Standard", true, true), ctx());
        d.set_function("Porch");
        assert_eq!(d.def_mut().function, "Porch");
        assert!(!d.def_mut().include_in_living_area && !d.def_mut().conditioned);
        d.set_function("Open Below");
        assert!(!d.def_mut().include_in_living_area && d.def_mut().conditioned);
        d.set_function("Standard");
        assert!(d.def_mut().include_in_living_area && d.def_mut().conditioned);
        // The user can still override.
        d.def_mut().include_in_living_area = false;
        assert!(!d.result().include_in_living_area);
    }

    #[test]
    fn the_name_is_required_and_unique() {
        let mut d = RoomTypeDialog::new(RoomTypeDef::new("Sunroom", "Standard", true, true), ctx());
        assert!(!d.has_error());
        d.def_mut().name = "  ".into();
        assert!(d.has_error());
        d.def_mut().name = "Kitchen".into();
        assert!(d.has_error());
        d.def_mut().name = "Sun Porch".into();
        assert!(!d.has_error());
    }

    #[test]
    fn every_panel_draws_and_its_settings_come_back_in_the_type() {
        let mut d = RoomTypeDialog::new(RoomTypeDef::new("Sunroom", "Standard", true, true), ctx());
        draw(&mut d);
        d.form.molds = [
            "Base - Colonial 5 1/4".into(),
            String::new(),
            "Crown - Cove 3 5/8".into(),
        ];
        d.form.fill.pattern = FillPattern::Solid;
        d.form.fill.color = [10, 200, 30];
        d.form.label.show_dimensions = true;
        d.def_mut().spec.layer = "CAD, Default".into();
        d.def_mut().spec.drawing_group = "Sun".into();
        d.def_mut().spec.deck = Some(plan_core::deck::DeckSpec::default());
        draw(&mut d);
        let t = d.result();
        assert_eq!(t.spec.moldings.len(), 2);
        assert!(t.spec.moldings.iter().any(|m| m.kind == MoldingKind::Crown));
        assert_eq!(t.spec.fill.as_ref().map(|f| f.color), Some([10, 200, 30]));
        assert!(t.spec.label.as_ref().is_some_and(|l| l.show_dimensions));
        assert_eq!(t.spec.layer, "CAD, Default");
        assert!(t.spec.deck.is_some());
    }

    #[test]
    fn multiple_types_change_only_what_is_set() {
        let mut a = RoomTypeDef::new("A", "Standard", true, true);
        let mut b = RoomTypeDef::new("B", "Garage", false, false);
        let mut d = RoomTypeDialog::multiple(2, ctx());
        assert!(d.is_multiple());
        assert_eq!(d.form.tabs().len(), 4, "only the shared panels");
        draw(&mut d);
        d.multi_mut().living = Some(false);
        d.multi_mut().layer = Some("CAD, Default".into());
        d.multi_mut().moldings =
            Some(["Base - Colonial 5 1/4".into(), String::new(), String::new()]);
        draw(&mut d);
        let ch = d.changes();
        ch.apply(&mut a);
        ch.apply(&mut b);
        assert!(!a.include_in_living_area && !b.include_in_living_area);
        assert!(a.conditioned && !b.conditioned, "No Change keeps each");
        assert_eq!(a.function, "Standard");
        assert_eq!(b.function, "Garage");
        assert_eq!(a.spec.layer, "CAD, Default");
        assert_eq!(b.spec.moldings.len(), 1);
    }
}
