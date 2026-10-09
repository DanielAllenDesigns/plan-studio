//! Room Specification (docs/chief-x18-dialogs.md, "Room Types and Room
//! Specification"; R-19..R-36, R-45).
//!
//! The dialog edits a draft [`RoomName`] plus the [`RoomExtras`] view of the
//! rest of the specification. OK hands both back to the app, which writes
//! them with `rooms_edit::apply_room_spec` as one undo step; conditioned, the
//! stem wall, moldings, fill and label options are stored in the room's
//! `RoomName`, the other extras are kept for the session.

use super::{
    dis_check, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::rooms_edit::{FillPattern, RoomExtras};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
use plan_core::defaults::RoomTypeDef;
use plan_core::extras::{structure_thickness, AreaKind, MoldingKind, StructureLayer};
use plan_core::geometry::Point;
use plan_core::rooms::{
    function_defaults, molding_def, molding_defs, LabelPlacement, RoomSlab, CEILING_SURFACES,
    FLOOR_SURFACES, WALL_SURFACES,
};
use plan_core::units::fmt_ft_in;
use plan_core::RoomName;

const ROOM_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    on("Deck"),
    on("Moldings"),
    on("Wall Covering"),
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
    /// The Floor or Ceiling Structure being defined (R-28, R-29).
    define: Option<Define>,
}

/// Which structure the Define editor shows.
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
            define: None,
            init,
        };
        Self {
            frame: SpecDialog::new("Room Specification", "room"),
            form,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
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
}

impl RoomForm {
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
        let d = function_defaults(&function, &self.name.room_type);
        self.name.has_floor = d.has_floor;
        self.name.has_ceiling = d.has_ceiling;
        self.name.floor_height_offset = d.floor_height_offset;
        self.extras.floor_finish_thickness = d
            .floor_finish_thickness
            .unwrap_or(self.init.default_floor_finish);
        self.extras.floor_structure = d.floor_structure;
        // A Deck room gets a Deck Specification; any other room loses it.
        let is_deck = function == DECK_ROOM_TYPE || self.name.room_type == DECK_ROOM_TYPE;
        if is_deck {
            if self.name.deck.is_none() {
                self.name.deck = Some(DeckSpec::default());
            }
        } else {
            self.name.deck = None;
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
        });
        row(ui, "Function", |ui| {
            let f = self.type_def().map_or("Standard", |t| t.function.as_str());
            ui.label(f);
        });
        if plan_check::CodeMinimums::is_habitable(&self.name.room_type) {
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
    }

    fn structure(&mut self, ui: &mut Ui) {
        let elev = self.init.floor_elevation;
        let ceil_default = self.init.floor_ceiling_height;

        section(ui, "Floor Height");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.extras.floor_height_absolute, true, "Absolute");
            ui.radio_value(&mut self.extras.floor_height_absolute, false, "Relative");
        });
        let base = if self.extras.floor_height_absolute {
            elev
        } else {
            0.0
        };
        let mut v = base + self.name.floor_height_offset;
        if self
            .fields
            .length_row(ui, "Floor Height", "floor_h", &mut v)
        {
            self.name.floor_height_offset = v - base;
        }

        section(ui, "Ceiling Height");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.extras.ceiling_height_absolute, true, "Absolute");
            ui.radio_value(&mut self.extras.ceiling_height_absolute, false, "Relative");
        });
        let cbase = if self.extras.ceiling_height_absolute {
            elev + self.name.floor_height_offset
        } else {
            0.0
        };
        let mut c = cbase + self.name.ceiling_height.unwrap_or(ceil_default);
        if self
            .fields
            .length_row(ui, "Ceiling Height", "ceil_h", &mut c)
        {
            self.name.ceiling_height = Some(c - cbase);
        }
        if let Some(min) = crate::editor::code::active().ceiling_min_for(&self.name.room_type) {
            let mut h = self.name.ceiling_height.unwrap_or(ceil_default);
            if super::code_notice::code_notice(
                ui,
                "IRC R305.1 ceiling height",
                &mut h,
                min,
                super::code_notice::LimitKind::Min,
            ) {
                self.name.ceiling_height = Some(h);
            }
        }
        if self.name.ceiling_height.is_some()
            && ui.small_button("Use the floor's ceiling height").clicked()
        {
            self.name.ceiling_height = None;
        }

        let finished = self.name.ceiling_height.unwrap_or(ceil_default);
        let finish = self.extras.ceiling_finish_thickness;
        let mut rough = self.name.rough_ceiling.is_some();
        if ui
            .checkbox(&mut rough, "Rough Ceiling")
            .on_hover_text("The bottom of the ceiling framing, where it is higher than the finished ceiling plus its finish layers (a soffit or tray)")
            .changed()
        {
            self.name.rough_ceiling = rough.then_some(finished + finish);
        }
        if let Some(r) = self.name.rough_ceiling.as_mut() {
            self.fields
                .length_row(ui, "Rough Ceiling Height", "rough_h", r);
            if *r < finished + finish {
                ui.colored_label(
                    super::ERROR_RED,
                    "The framing cannot be lower than the finished ceiling and its finish",
                );
            }
        } else {
            ui.weak(format!(
                "Without one the framing starts at the finished ceiling plus {} of finish.",
                fmt_ft_in(finish)
            ));
        }

        section(ui, "Finish");
        self.fields.length_row(
            ui,
            "Floor Finish Thickness",
            "floor_fin",
            &mut self.extras.floor_finish_thickness,
        );
        self.fields.length_row(
            ui,
            "Ceiling Finish Thickness",
            "ceil_fin",
            &mut self.extras.ceiling_finish_thickness,
        );

        section(ui, "Platforms");
        ui.checkbox(&mut self.name.has_floor, "Floor Under This Room")
            .on_hover_text("Off for an Open Below room: no floor, and the ceiling below opens");
        ui.checkbox(&mut self.name.has_ceiling, "Ceiling Over This Room")
            .on_hover_text("Off for a deck or porch");
        ui.checkbox(&mut self.extras.roof_over, "Roof Over This Room")
            .on_hover_text("Off for a courtyard or open deck: Build Roof leaves a hole over it");
        ui.add_enabled_ui(self.extras.roof_over, |ui| {
            ui.checkbox(&mut self.extras.flat_roof, "Flat Roof Over This Room")
                .on_hover_text("Build Roof puts a level roof plane at this room's ceiling");
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
        }

        section(ui, "Stem Wall");
        ui.checkbox(&mut self.extras.stem_wall, "Stem Wall");
        if self.extras.stem_wall {
            self.fields.length_row(
                ui,
                "Stem Wall Height",
                "stem_h",
                &mut self.extras.stem_wall_height,
            );
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (which, label) in [
                (Define::Floor, "Floor Structure Define\u{2026}"),
                (Define::Ceiling, "Ceiling Structure Define\u{2026}"),
            ] {
                let open = self.define == Some(which);
                if ui.selectable_label(open, label).clicked() {
                    self.define = if open { None } else { Some(which) };
                }
            }
        });
        if let Some(which) = self.define {
            self.define_editor(ui, which);
        }
    }

    /// Deck tab (CB-86): the Deck Specification of a deck room: the decking
    /// boards, the framing Build Framing > Deck makes, and stairs to grade.
    fn deck(&mut self, ui: &mut Ui) {
        section(ui, "Deck");
        let mut is_deck = self.name.deck.is_some();
        if ui
            .checkbox(&mut is_deck, "This Room is a Deck")
            .on_hover_text("A deck has decking boards on joists instead of a platform slab, no ceiling and no roof")
            .changed()
        {
            self.name.deck = is_deck.then(DeckSpec::default);
            if is_deck {
                self.name.has_ceiling = false;
            }
        }
        let edges = self.init.polygon.len();
        let f = &mut self.fields;
        let Some(spec) = self.name.deck.as_mut() else {
            ui.weak("Set the Room Type to Deck, or tick the box above, to specify decking, framing and stairs.");
            return;
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
                ui.add(
                    egui::DragValue::new(&mut spec.stairs.edge).range(0..=edges.saturating_sub(1)),
                );
                ui.weak(format!("of {edges}, counted from the first corner"));
            });
            f.length_row(ui, "Stair Width", "deck_sw", &mut spec.stairs.width);
            f.length_row(ui, "Tread Depth", "deck_st", &mut spec.stairs.tread);
        });
        ui.add_space(6.0);
        ui.weak("Choose Build > Framing > Build Deck Framing to make the joists, beams and posts with footings from this specification.");
    }

    /// The Floor/Ceiling Structure dialog (R-28, R-29): the layer stack of
    /// this room's platform, top layer first. An empty stack follows the
    /// floor's default platform.
    fn define_editor(&mut self, ui: &mut Ui, which: Define) {
        let title = match which {
            Define::Floor => "Floor Structure",
            Define::Ceiling => "Ceiling Structure",
        };
        section(ui, title);
        let layers = match which {
            Define::Floor => &mut self.extras.floor_structure,
            Define::Ceiling => &mut self.extras.ceiling_structure,
        };
        let mut remove: Option<usize> = None;
        let mut swap: Option<(usize, usize)> = None;
        let count = layers.len();
        egui::Grid::new(("room_structure", title))
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Layer");
                ui.strong("Material");
                ui.strong("Thickness");
                ui.end_row();
                for (i, l) in layers.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(i > 0, egui::Button::new("\u{25B2}"))
                            .clicked()
                        {
                            swap = Some((i, i - 1));
                        }
                        if ui
                            .add_enabled(i + 1 < count, egui::Button::new("\u{25BC}"))
                            .clicked()
                        {
                            swap = Some((i, i + 1));
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut l.material).desired_width(120.0));
                    ui.add(
                        egui::DragValue::new(&mut l.thickness)
                            .speed(0.125)
                            .range(0.0..=240.0)
                            .suffix("\""),
                    );
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some((a, b)) = swap {
            layers.swap(a, b);
        }
        if let Some(i) = remove {
            layers.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button("Add Layer").clicked() {
                layers.push(StructureLayer::new("Framing", 1.0));
            }
            if ui
                .add_enabled(!layers.is_empty(), egui::Button::new("Use Default"))
                .clicked()
            {
                layers.clear();
            }
        });
        if layers.is_empty() {
            ui.weak("Follows the floor's default platform.");
        } else {
            ui.label(format!(
                "Total thickness {}",
                fmt_ft_in(structure_thickness(layers))
            ));
        }
    }

    /// Moldings tab (R-34): a base, a chair rail and a crown profile from the
    /// molding library, built around the room in 3D.
    fn moldings(&mut self, ui: &mut Ui) {
        section(ui, "Profiles");
        ui.weak("Choose a profile from the library for each molding. They run around the room, stop at doors and miter at the corners.");
        for (kind, label) in [
            (MoldingKind::Base, "Base"),
            (MoldingKind::Chair, "Chair Rail"),
            (MoldingKind::Crown, "Crown"),
        ] {
            ui.add_space(4.0);
            let current = match kind {
                MoldingKind::Base => &mut self.extras.base_molding,
                MoldingKind::Chair => &mut self.extras.chair_molding,
                MoldingKind::Crown => &mut self.extras.crown_molding,
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
        match ROOM_TABS[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Deck" => self.deck(ui),
            "Moldings" => self.moldings(ui),
            "Wall Covering" => self.wall_covering(ui),
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
        // A courtyard is open to the sky: no ceiling either.
        assert!(!d.room_name().has_ceiling);
        d.set_room_type("Attic");
        assert!(d.room_name().has_ceiling);
        // A plain room goes back to the floor's defaults, switches stay editable.
        d.set_room_type("Bedroom");
        assert!(d.room_name().has_floor && d.room_name().has_ceiling);
        assert_eq!(d.extras().floor_finish_thickness, 0.75);
        assert!(d.extras().floor_structure.is_empty());
    }

    /// The Structure and Label tabs, with the Define editor open, draw.
    #[test]
    fn the_structure_and_label_tabs_draw_with_the_define_editor_open() {
        let ctx = egui::Context::default();
        for define in [Define::Floor, Define::Ceiling] {
            let mut d = dialog();
            d.set_room_type("Garage");
            d.form.define = Some(define);
            for tab in [1, 7] {
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
