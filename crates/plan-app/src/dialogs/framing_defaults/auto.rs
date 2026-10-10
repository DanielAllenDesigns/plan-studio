//! The Automatic Framing Defaults dialog (manual pp. 889-900): the panels
//! Foundation, the floor levels (1st, 2nd ...), Deck, Deck Support, Wall,
//! Openings, Fireplaces, Roof and Trusses.
//!
//! It edits a [`FramingSettings`] (the wall, floor and roof framing
//! defaults and the Build Framing options) and the [`FramingCatalog`] (the
//! constructions of each Role and the fields neither holds). OK stores both
//! as one undo step; the Build Framing dialog keeps only its Automatic
//! Framing Defaults button, the Automatically Rebuild choices and Build
//! Framing Once (DECISIONS 112), and opens this dialog.

use super::{enum_combo, inches, name_combo, request_members};
use crate::dialogs::{on, row, section, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::framing_view::FramingSettings;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Ui};
use plan_framing::catalog::{FramingCatalog, Role, RoofSizeRow};
use plan_framing::{
    BearingMode, BlockingStyle, Connection, JoistDirection, Lumber, OverframeLayer, Splice,
    WallConnection, TWO_BY_EIGHT, TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_TEN, TWO_BY_TWELVE,
};

const SIZES: [Lumber; 5] = [TWO_BY_FOUR, TWO_BY_SIX, TWO_BY_EIGHT, TWO_BY_TEN, TWO_BY_TWELVE];

const TABS_1: &[Tab] = &[
    on("Foundation"),
    on("1st"),
    on("Deck"),
    on("Deck Support"),
    on("Wall"),
    on("Openings"),
    on("Fireplaces"),
    on("Roof"),
    on("Trusses"),
];
const TABS_2: &[Tab] = &[
    on("Foundation"),
    on("1st"),
    on("2nd"),
    on("Deck"),
    on("Deck Support"),
    on("Wall"),
    on("Openings"),
    on("Fireplaces"),
    on("Roof"),
    on("Trusses"),
];
const TABS_3: &[Tab] = &[
    on("Foundation"),
    on("1st"),
    on("2nd"),
    on("3rd"),
    on("Deck"),
    on("Deck Support"),
    on("Wall"),
    on("Openings"),
    on("Fireplaces"),
    on("Roof"),
    on("Trusses"),
];
const TABS_4: &[Tab] = &[
    on("Foundation"),
    on("1st"),
    on("2nd"),
    on("3rd"),
    on("4th"),
    on("Deck"),
    on("Deck Support"),
    on("Wall"),
    on("Openings"),
    on("Fireplaces"),
    on("Roof"),
    on("Trusses"),
];

/// A panel of the dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    Foundation,
    /// The floor level panel of floor `n` (1-based).
    Floor(usize),
    Deck,
    DeckSupport,
    Wall,
    Openings,
    Fireplaces,
    Roof,
    Trusses,
}

/// The Automatic Framing Defaults dialog.
pub struct AutoDialog {
    frame: SpecDialog,
    form: AutoForm,
}

struct AutoForm {
    settings: FramingSettings,
    catalog: FramingCatalog,
    floors: usize,
}

impl AutoDialog {
    pub fn new(settings: &FramingSettings, mut catalog: FramingCatalog) -> Self {
        let floors = settings.floor_names.len().clamp(1, 4);
        let mut settings = settings.clone();
        settings.build_on_ok = None;
        // The Blocking check boxes start from what the plan already builds.
        let a = &mut catalog.auto;
        if settings.walls.wall_blocking && !a.block_exterior && !a.block_interior {
            a.block_exterior = true;
            a.block_interior = true;
        }
        let n = settings.floor_names.len().max(1);
        a.ceiling_structure_default.resize(n, true);
        a.floor_structure_default.resize(n, true);
        Self {
            frame: SpecDialog::new("Automatic Framing Defaults", "framing_automatic"),
            form: AutoForm {
                settings,
                catalog,
                floors,
            },
        }
    }

    /// The panels of the dialog, in tab order.
    pub fn panels(&self) -> Vec<Panel> {
        self.form.panels()
    }

    /// Opens on the panel at index `tab`.
    pub fn start_on(&mut self, tab: usize) {
        let last = self.form.panels().len() - 1;
        self.frame.start_on(tab.min(last));
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    /// The settings with the Blocking boxes folded back into the walls.
    pub fn settings(&self) -> &FramingSettings {
        &self.form.settings
    }

    pub fn settings_mut(&mut self) -> &mut FramingSettings {
        &mut self.form.settings
    }

    pub fn catalog(&self) -> &FramingCatalog {
        &self.form.catalog
    }

    pub fn catalog_mut(&mut self) -> &mut FramingCatalog {
        &mut self.form.catalog
    }

    /// Folds the dialog's own fields back into the settings; what OK stores.
    pub fn finish(&mut self) {
        self.form.finish();
    }

    /// Draws the panel `panel` into `ui` (tests draw every panel).
    #[cfg(test)]
    pub fn draw_panel(&mut self, ui: &mut Ui, panel: Panel) {
        self.form.draw(ui, panel);
    }
}

impl AutoForm {
    fn panels(&self) -> Vec<Panel> {
        let mut v = vec![Panel::Foundation];
        v.extend((1..=self.floors).map(Panel::Floor));
        v.extend([
            Panel::Deck,
            Panel::DeckSupport,
            Panel::Wall,
            Panel::Openings,
            Panel::Fireplaces,
            Panel::Roof,
            Panel::Trusses,
        ]);
        v
    }

    fn finish(&mut self) {
        let a = &self.catalog.auto;
        self.settings.walls.wall_blocking = a.block_exterior || a.block_interior;
    }

    fn ref_on(&self, fi: usize) -> bool {
        self.settings
            .build
            .floor_reference
            .get(fi)
            .copied()
            .unwrap_or(true)
    }

    fn set_ref(&mut self, fi: usize, on: bool) {
        let v = &mut self.settings.build.floor_reference;
        if v.len() <= fi {
            v.resize(fi + 1, true);
        }
        v[fi] = on;
    }

    // ----- helpers -----

    fn construction(&mut self, ui: &mut Ui, label: &str, salt: &str, role: Role) {
        let names: Vec<String> = self.catalog.defs.iter().map(|d| d.name.clone()).collect();
        let mut cur = self
            .catalog
            .def_for(role)
            .map(|d| d.name.clone())
            .unwrap_or_default();
        let before = cur.clone();
        row(ui, label, |ui| {
            name_combo(ui, salt, &mut cur, &names);
            if ui.small_button("Define...").clicked() {
                request_members();
            }
        });
        if cur != before {
            self.catalog.set_construction(role, &cur);
        }
    }

    // ----- floor levels -----

    /// The Subfloor section of the platform of floor `no` (the floor panel of
    /// the floor below). `fi` indexes the per-floor switches.
    fn subfloor(&mut self, ui: &mut Ui, no: usize, fi: usize) {
        section(ui, &format!("Subfloor of Floor {no}"));
        let mut r = self.ref_on(fi);
        if ui
            .checkbox(&mut r, "Use Framing Reference")
            .on_hover_text("Start the joist layout at the Framing Reference Marker")
            .changed()
        {
            self.set_ref(fi, r);
        }
        let a = &mut self.catalog.auto;
        if a.floor_structure_default.len() <= fi {
            a.floor_structure_default.resize(fi + 1, true);
        }
        ui.horizontal(|ui| {
            ui.label("Floor Structure");
            ui.checkbox(&mut a.floor_structure_default[fi], "Default")
                .on_hover_text("Use the Floor Structure Definition of the Floor/Ceiling Platform Defaults");
        });
        let w = &mut self.settings.walls;
        row(ui, "On Center Spacing", |ui| {
            inches(ui, &mut w.joist_spacing, 4.0, 96.0)
        });
        row(ui, "Joist size", |ui| size_combo(ui, "af_joist", &mut w.joist_size));
        row(ui, "Joists run", |ui| {
            enum_combo(
                ui,
                "af_dir",
                &mut w.joist_direction,
                &[JoistDirection::Auto, JoistDirection::AlongX, JoistDirection::AlongY],
                direction_name,
            )
        });
        row(ui, "Bearing", |ui| {
            enum_combo(ui, "af_bear", &mut w.bearing, &BearingMode::ALL, BearingMode::name)
        });
        ui.checkbox(&mut w.blocking, "Mid-span blocking");
        ui.checkbox(&mut w.rim_joist, "Rim Joist");
        let on = self.settings.walls.rim_joist;
        ui.add_enabled_ui(on, |ui| {
            row(ui, "Rim plies", |ui| {
                ui.radio_value(&mut self.settings.walls.rim_plies, 1, "Single");
                ui.radio_value(&mut self.settings.walls.rim_plies, 2, "Double");
            });
            let d = &mut self.settings.build.detail;
            row(ui, "Rim Joist Width", |ui| inches(ui, &mut d.rim_width, 0.5, 12.0));
            let a = &mut self.catalog.auto;
            ui.horizontal(|ui| {
                ui.checkbox(&mut a.max_rim_on, "Max Rim Joist Length");
                ui.add_enabled_ui(a.max_rim_on, |ui| {
                    if a.max_rim_on && d.max_rim_length <= 0.0 {
                        d.max_rim_length = 192.0;
                    }
                    inches(ui, &mut d.max_rim_length, 12.0, 960.0)
                });
                if !a.max_rim_on {
                    d.max_rim_length = 0.0;
                }
            });
            row(ui, "Rim Joist Connection Style", |ui| {
                enum_combo(ui, "af_rimconn", &mut d.rim_connection, &Connection::ALL, Connection::name)
            });
        });
        self.construction(ui, "Joist construction", "af_jcon", Role::FloorJoist);
        self.construction(ui, "Rim Joist construction", "af_rcon", Role::RimJoist);
        let d = &mut self.settings.build.detail;
        row(ui, "Bear Joists on Beams and Walls", |ui| {
            enum_combo(ui, "af_splice", &mut d.splice, &Splice::ALL, Splice::name)
        });
        row(ui, "Blocking", |ui| {
            enum_combo(ui, "af_blk", &mut d.blocking_style, &BlockingStyle::ALL, BlockingStyle::name)
        });
        ui.weak("The lap is 8\" and centred over the support. The spacing, size and styles apply to every floor.");
    }

    /// The Ceiling section above floor `no`.
    fn ceiling(&mut self, ui: &mut Ui, no: usize, fi: usize) {
        section(ui, &format!("Ceiling Above Floor {no}"));
        let a = &mut self.catalog.auto;
        if a.ceiling_structure_default.len() <= fi {
            a.ceiling_structure_default.resize(fi + 1, true);
        }
        ui.horizontal(|ui| {
            ui.label("Ceiling Structure");
            ui.checkbox(&mut a.ceiling_structure_default[fi], "Default")
                .on_hover_text("Use the Ceiling Structure Definition of the Floor/Ceiling Platform Defaults");
        });
        let w = &mut self.settings.walls;
        row(ui, "On Center Spacing", |ui| {
            inches(ui, &mut w.ceiling_joist_spacing, 4.0, 96.0)
        });
        row(ui, "Ceiling joist size", |ui| {
            size_combo(ui, "af_cjoist", &mut w.ceiling_joist_size)
        });
        row(ui, "Joists run", |ui| {
            enum_combo(
                ui,
                "af_cdir",
                &mut w.ceiling_direction,
                &[JoistDirection::Auto, JoistDirection::AlongX, JoistDirection::AlongY],
                direction_name,
            )
        });
        self.construction(ui, "Joist construction", "af_ccon", Role::CeilingJoist);
        let d = &mut self.settings.build.detail;
        row(ui, "Bear Joists on Beams and Walls", |ui| {
            enum_combo(ui, "af_csplice", &mut d.ceiling_splice, &Splice::ALL, Splice::name)
        });
        row(ui, "Blocking", |ui| {
            enum_combo(
                ui,
                "af_cblk",
                &mut d.ceiling_blocking_style,
                &BlockingStyle::ALL,
                BlockingStyle::name,
            )
        });
        ui.weak("Ceiling platforms exist only above rooms without living space over them; roof trusses' bottom chords take the place of ceiling joists.");
    }

    fn foundation(&mut self, ui: &mut Ui) {
        ui.weak("Floor framing of Floor 1 is set here, on the panel of the floor below it.");
        self.subfloor(ui, 1, 0);
    }

    fn floor_level(&mut self, ui: &mut Ui, no: usize) {
        self.ceiling(ui, no, no - 1);
        if no < self.floors {
            ui.add_space(6.0);
            self.subfloor(ui, no + 1, no);
        }
    }

    // ----- deck -----

    fn deck(&mut self, ui: &mut Ui, support: bool) {
        section(ui, if support { "Deck Support" } else { "Deck" });
        ui.label(if support {
            "The Deck Support panel is the Deck Support panel of the Room Specification: the posts, beams and footings under a deck."
        } else {
            "The Deck panel is the Deck panel of the Room Specification: the joists, planking, railing and ledger of a deck."
        });
        ui.weak("Deck rooms have their own automatic framing, set in the Deck Room Defaults (Default Settings > Rooms) and in each deck's Room Specification, not in these framing defaults.");
    }

    // ----- walls -----

    fn wall(&mut self, ui: &mut Ui) {
        section(ui, "Studs and Girts");
        self.construction(ui, "Construction", "af_stud", Role::Stud);
        let w = &mut self.settings.walls;
        row(ui, "Width", |ui| size_combo(ui, "af_studsize", &mut w.stud_size));
        row(ui, "Spacing", |ui| inches(ui, &mut w.stud_spacing, 4.0, 96.0));
        let a = &mut self.catalog.auto;
        row(ui, "Max Girt Length", |ui| inches(ui, &mut a.max_girt_length, 12.0, 960.0));
        ui.checkbox(&mut a.allow_balloon, "Allow Automatic Balloon Framing");
        section(ui, "Wall Connections");
        let d = &mut self.settings.build.detail;
        row(ui, "Corners", |ui| {
            enum_combo(ui, "af_corner", &mut d.corner_style, &WallConnection::ALL, WallConnection::name)
        });
        row(ui, "Intersections", |ui| {
            let tees = [WallConnection::Standard, WallConnection::Reduced, WallConnection::Laddered];
            enum_combo(ui, "af_tee", &mut d.tee_style, &tees, WallConnection::name)
        });
        section(ui, "Plates");
        self.construction(ui, "Top Plate Construction", "af_tpc", Role::Plate);
        let w = &mut self.settings.walls;
        let a = &mut self.catalog.auto;
        row(ui, "Top Plate Width", |ui| inches(ui, &mut a.top_plate_width, 0.5, 12.0));
        row(ui, "Top Plate Count", |ui| {
            ui.add(egui::DragValue::new(&mut w.top_plates).range(1..=3));
        });
        let d = &mut self.settings.build.detail;
        row(ui, "Top Plate Connection Style", |ui| {
            enum_combo(ui, "af_tpconn", &mut d.top_plate_connection, &Connection::ALL, Connection::name)
        });
        row(ui, "Bottom Plate Count", |ui| {
            ui.add(egui::DragValue::new(&mut w.bottom_plates).range(1..=2));
        });
        row(ui, "Bottom Plate Thickness", |ui| {
            inches(ui, &mut a.bottom_plate_thickness, 0.5, 6.0)
        });
        row(ui, "Max Plate Length", |ui| inches(ui, &mut a.max_plate_length, 12.0, 960.0));
        section(ui, "Blocking");
        ui.label("Include Automatic Blocking");
        ui.horizontal(|ui| {
            ui.checkbox(&mut a.block_exterior, "Exterior");
            ui.checkbox(&mut a.block_interior, "Interior");
        });
        ui.add_enabled_ui(a.block_exterior || a.block_interior, |ui| {
            row(ui, "Blocking spacing", |ui| {
                inches(ui, &mut w.wall_blocking_spacing, 12.0, 240.0)
            });
        });
        ui.checkbox(&mut d.stagger_blocking, "Stagger Blocking");
        ui.weak("Wall blocking is generated half-way up each wall's total height.");
        section(ui, "Corner and tee backing");
        let w = &mut self.settings.walls;
        count(ui, "Corner studs", &mut w.corner_studs, 3);
        count(ui, "Tee backing studs per side", &mut w.tee_studs, 3);
        section(ui, "Mitre Ends of Angled Walls");
        let d = &mut self.settings.build.detail;
        ui.checkbox(&mut d.mitre_plate_ends, "Mitre Plate Ends");
        ui.checkbox(&mut d.rotate_end_studs, "Rotate End Studs");
        ui.checkbox(&mut d.frame_through_horizontal, "Horizontal Frame Through");
        section(ui, "Wall Detail Views");
        ui.checkbox(&mut d.details_from_exterior, "Build Wall Framing Details from Exterior");
    }

    // ----- openings -----

    fn openings(&mut self, ui: &mut Ui) {
        section(ui, "Header Sizes");
        ui.weak("Set so that the wider the opening, the deeper the header.");
        let w = &mut self.settings.walls;
        count(ui, "Plies", &mut w.header_plies, 4);
        let mut remove = None;
        let rows = w.header_table.len();
        for (i, r) in w.header_table.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if i + 1 == rows {
                    ui.label("Wider than the rows above");
                } else {
                    ui.label("Openings up to");
                    ui.add(
                        egui::DragValue::new(&mut r.up_to)
                            .speed(1.0)
                            .range(1.0..=480.0)
                            .suffix("\""),
                    );
                }
                size_combo(ui, &format!("af_hdr_{i}"), &mut r.lumber);
                if rows > 1 && ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            w.header_table.remove(i);
        }
        if ui.button("Add a row").clicked() {
            let last = w.header_table.last().copied();
            if let Some(last) = last {
                let up_to = w
                    .header_table
                    .iter()
                    .rev()
                    .nth(1)
                    .map_or(48.0, |r| r.up_to + 12.0);
                let n = w.header_table.len();
                w.header_table
                    .insert(n - 1, plan_framing::HeaderRow { up_to, lumber: last.lumber });
            }
        }
        let d = &mut self.settings.build.detail;
        row(ui, "Maximum Depth", |ui| {
            inches(ui, &mut d.header_max_depth, 0.0, 120.0)
        });
        ui.weak("An opening whose top is closer to the top plate than this gets a solid header and no cripples; 0 turns the rule off.");
        let w = &mut self.settings.walls;
        count(ui, "King studs per side", &mut w.king_studs, 3);
        count(ui, "Trimmers per side", &mut w.trimmers, 3);
        row(ui, "Cripple spacing", |ui| inches(ui, &mut w.cripple_spacing, 4.0, 96.0));
        self.construction(ui, "Header construction", "af_hcon", Role::Header);
        section(ui, "Bay/Box/Bow Trimmers");
        let a = &mut self.catalog.auto;
        count(ui, "Maximum Number", &mut a.bay_max_trimmers, 4);
        row(ui, "Component Thickness", |ui| inches(ui, &mut a.bay_component_thickness, 0.5, 6.0));
        section(ui, "Materials List");
        let d = &mut self.settings.build.detail;
        ui.checkbox(&mut d.list_cut_header_lengths, "List Cut Header Lengths in Mixed Reporting")
            .on_hover_text("Shows precut headers in the Materials List when Mixed Reporting is the method; otherwise one total footage of headers");
    }

    // ----- fireplaces -----

    fn fireplaces(&mut self, ui: &mut Ui) {
        let types: Vec<String> = self.catalog.types.iter().map(|t| t.name.clone()).collect();
        let a = &mut self.catalog.auto;
        section(ui, "Headers");
        row(ui, "Type", |ui| name_combo(ui, "af_fp_type", &mut a.fireplace_header_type, &types));
        row(ui, "Thickness", |ui| inches(ui, &mut a.fireplace_header_thickness, 0.5, 12.0));
        count(ui, "Count", &mut a.fireplace_header_count, 6);
        section(ui, "Trimmers");
        row(ui, "Double Trimmer At", |ui| inches(ui, &mut a.double_trimmer_at, 6.0, 240.0));
        row(ui, "Triple Trimmer At", |ui| inches(ui, &mut a.triple_trimmer_at, 6.0, 240.0));
        if a.triple_trimmer_at < a.double_trimmer_at {
            a.triple_trimmer_at = a.double_trimmer_at;
        }
        section(ui, "Sills");
        row(ui, "Thickness", |ui| inches(ui, &mut a.fireplace_sill_thickness, 0.5, 12.0));
        ui.checkbox(&mut a.fireplace_double_sill, "Double Sills");
        ui.weak("These are the rough openings of legacy fireplaces placed in walls; a sill is made only when the fireplace is raised off the floor.");
    }

    // ----- roof -----

    fn roof_row(
        ui: &mut Ui,
        label: &str,
        row_state: &mut RoofSizeRow,
        lumber: Option<&mut Lumber>,
    ) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut row_state.on, "");
            ui.label(format!("{label:<16}"));
            match lumber {
                Some(l) => {
                    ui.add(egui::DragValue::new(&mut l.thickness).speed(0.125).range(0.25..=12.0).suffix("\""));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut l.depth).speed(0.125).range(0.5..=24.0).suffix("\""));
                }
                None => {
                    ui.add(egui::DragValue::new(&mut row_state.width).speed(0.125).range(0.25..=12.0).suffix("\""));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut row_state.depth).speed(0.125).range(0.5..=24.0).suffix("\""));
                }
            }
        });
    }

    fn roof(&mut self, ui: &mut Ui) {
        ui.weak("Changes here do not affect existing roof planes; rebuild the roof to apply them.");
        section(ui, "Roof");
        let b = &mut self.settings.build;
        ui.checkbox(&mut b.roof_reference, "Use Framing Reference");
        let a = &mut self.catalog.auto;
        ui.checkbox(&mut a.angled_dormer_hole, "Angled Dormer Hole");
        let r = &mut self.settings.roof;
        ui.checkbox(&mut r.trim_to_soffits, "Trim Framing To Soffits");
        row(ui, "Spacing", |ui| inches(ui, &mut r.spacing, 4.0, 96.0));
        row(ui, "Maximum Lookout Spacing", |ui| inches(ui, &mut r.lookout_spacing, 4.0, 96.0));
        ui.horizontal(|ui| {
            ui.checkbox(&mut a.max_subfascia_on, "Maximum Subfascia Length");
            ui.add_enabled_ui(a.max_subfascia_on, |ui| {
                inches(ui, &mut a.max_subfascia_length, 12.0, 960.0)
            });
        });
        row(ui, "Blocking Style", |ui| {
            enum_combo(ui, "af_rblk", &mut a.roof_blocking_style, &BlockingStyle::ALL, BlockingStyle::name)
        });
        ui.checkbox(&mut a.roof_blocking_vertical, "Vertical");
        section(ui, "Roof Lookouts");
        ui.checkbox(&mut r.lookouts, "Lookouts under gable overhangs");
        row(ui, "Lookout Spacing", |ui| inches(ui, &mut r.lookout_spacing, 4.0, 96.0));
        let mut match_spacing = r.lookout_offset <= 0.0;
        if ui.checkbox(&mut match_spacing, "Match Spacing").changed() {
            r.lookout_offset = if match_spacing { 0.0 } else { r.lookout_spacing };
        }
        ui.add_enabled_ui(!match_spacing, |ui| {
            row(ui, "Offset from Subfascia", |ui| inches(ui, &mut r.lookout_offset, 0.0, 96.0))
        });
        section(ui, "Roof Layers");
        ui.weak("The Surface, Structure and Ceiling Finish definitions are edited in the Roof Defaults and the Roof Plane Specification.");
        ui.checkbox(&mut a.use_room_ceiling_finish, "Use Room Ceiling Finish");
        let mut soffits = r.soffit_thickness > 0.0;
        if ui.checkbox(&mut soffits, "Soffits").changed() {
            r.soffit_thickness = if soffits { 0.5 } else { 0.0 };
        }
        ui.add_enabled_ui(soffits, |ui| {
            row(ui, "Soffit thickness", |ui| inches(ui, &mut r.soffit_thickness, 0.25, 6.0))
        });
        ui.checkbox(&mut a.flat_under_eave_subfascia, "Flat Under Eave Subfascia");
        section(ui, "Roof Size");
        ui.weak("Width x depth of each roof framing member; clear a box to leave the member out.");
        Self::roof_row(ui, "Rafters", &mut a.rafters, Some(&mut r.rafter));
        Self::roof_row(ui, "Ridge", &mut a.ridge, Some(&mut r.ridge));
        Self::roof_row(ui, "Lookouts", &mut a.lookouts, Some(&mut r.lookout));
        Self::roof_row(ui, "Shoe Plate", &mut a.shoe_plate, Some(&mut r.shoe_plate));
        Self::roof_row(ui, "Gable Subfascia", &mut a.gable_subfascia, None);
        Self::roof_row(ui, "Eave Subfascia", &mut a.eave_subfascia, None);
        Self::roof_row(ui, "Gable Fascia", &mut a.gable_fascia, None);
        Self::roof_row(ui, "Eave Fascia", &mut a.eave_fascia, Some(&mut r.fascia));
        Self::roof_row(ui, "Blocking", &mut a.roof_blocking, None);
        self.construction(ui, "Rafter construction", "af_rafcon", Role::Rafter);
        self.construction(ui, "Ridge construction", "af_ridcon", Role::Ridge);
        self.construction(ui, "Fascia construction", "af_fascon", Role::Fascia);
        self.construction(ui, "Blocking construction", "af_rblcon", Role::RoofBlocking);
        let r = &mut self.settings.roof;
        section(ui, "Hip Girder Truss");
        count(ui, "Count", &mut r.hip_girder_count, 6);
        let mut auto = r.hip_girder_distance <= 0.0;
        if ui.checkbox(&mut auto, "Automatic (about 4')").changed() {
            r.hip_girder_distance = if auto { 0.0 } else { 48.0 };
        }
        ui.add_enabled_ui(!auto, |ui| {
            row(ui, "Distance from Wall Main Layer", |ui| {
                inches(ui, &mut r.hip_girder_distance, 1.0, 480.0)
            })
        });
        section(ui, "Roof Overframing");
        ui.checkbox(&mut r.overframing, "Roof Overframing");
        ui.add_enabled_ui(r.overframing, |ui| {
            row(ui, "Overframe Layer", |ui| {
                enum_combo(ui, "af_over", &mut r.overframe_layer, &OverframeLayer::ALL, OverframeLayer::name)
            })
        });
    }

    // ----- trusses -----

    fn trusses(&mut self, ui: &mut Ui) {
        ui.weak("Changes here do not affect trusses already in the plan. Chief does not engineer trusses; have yours designed by an engineer or truss company.");
        section(ui, "Roof Trusses");
        let a = &mut self.catalog.auto;
        ui.checkbox(&mut a.include_peak_trusses, "Include Peak Trusses");
        ui.checkbox(&mut a.end_truss_blocking, "Include Horizontal Blocking in End Trusses");
        ui.add_enabled_ui(a.end_truss_blocking, |ui| {
            row(ui, "Vertical Spacing", |ui| inches(ui, &mut a.end_truss_vertical_spacing, 4.0, 96.0));
            ui.checkbox(&mut a.end_truss_rollout_auto, "Automatic");
            ui.add_enabled_ui(!a.end_truss_rollout_auto, |ui| {
                row(ui, "Rollout Offset", |ui| inches(ui, &mut a.end_truss_rollout_offset, 0.0, 96.0))
            });
        });
        section(ui, "Trusses over a Truss Base");
        let t = &mut self.settings.build.trusses;
        row(ui, "Type", |ui| {
            enum_combo(ui, "af_trtype", &mut t.kind, &super::super::truss::TYPES, |k| k.name())
        });
        row(ui, "Pitch", |ui| {
            ui.add(egui::DragValue::new(&mut t.pitch).speed(0.25).range(0.0..=24.0).suffix(" in 12"));
        });
        row(ui, "Heel height", |ui| inches(ui, &mut t.heel_height, 0.0, 48.0));
        row(ui, "Overhang", |ui| inches(ui, &mut t.overhang, 0.0, 96.0));
        row(ui, "Spacing", |ui| inches(ui, &mut t.spacing, 4.0, 96.0));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for label in [
                "Edit Roof Truss Defaults",
                "Edit Roof Girder Truss Defaults",
                "Edit Floor Truss Defaults",
            ] {
                ui.add_enabled(false, egui::Button::new(label))
                    .on_disabled_hover_text("The truss defaults dialogs are not built yet; a truss specification holds the same fields");
            }
        });
    }

    fn draw(&mut self, ui: &mut Ui, panel: Panel) {
        match panel {
            Panel::Foundation => self.foundation(ui),
            Panel::Floor(n) => self.floor_level(ui, n),
            Panel::Deck => self.deck(ui, false),
            Panel::DeckSupport => self.deck(ui, true),
            Panel::Wall => self.wall(ui),
            Panel::Openings => self.openings(ui),
            Panel::Fireplaces => self.fireplaces(ui),
            Panel::Roof => self.roof(ui),
            Panel::Trusses => self.trusses(ui),
        }
    }
}

impl SpecPages for AutoForm {
    fn tabs(&self) -> &'static [Tab] {
        match self.floors {
            1 => TABS_1,
            2 => TABS_2,
            3 => TABS_3,
            _ => TABS_4,
        }
    }

    fn error(&self) -> Option<String> {
        let w = &self.settings.walls;
        if w.header_table.is_empty() {
            return Some("The header table needs a row".into());
        }
        if self.settings.roof.hip_girder_count == 0 {
            return Some("A hip girder needs at least one truss".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let panels = self.panels();
        let panel = panels[tab.min(panels.len() - 1)];
        self.draw(ui, panel);
        // OK reads the settings; keep the folded fields current.
        self.finish();
    }

    fn preview(&self, painter: &egui::Painter, rect: Rect) {
        sketch_members(painter, rect);
    }
}

// ----- small pieces -----

fn size_combo(ui: &mut Ui, salt: &str, value: &mut Lumber) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.nominal_name())
        .show_ui(ui, |ui| {
            for l in SIZES {
                if ui.selectable_label(*value == l, l.nominal_name()).clicked() {
                    *value = l;
                }
            }
        });
}

fn count(ui: &mut Ui, label: &str, value: &mut u32, max: u32) {
    row(ui, label, |ui| {
        ui.add(egui::DragValue::new(value).range(0..=max));
    });
}

fn direction_name(d: JoistDirection) -> &'static str {
    match d {
        JoistDirection::Auto => "Shortest span",
        JoistDirection::AlongX => "Along X",
        JoistDirection::AlongY => "Along Y",
    }
}

/// A small sketch of framing: joists and a rim under a wall, drawn like a
/// drawing sheet so it reads at any brightness.
pub(super) fn sketch_members(painter: &egui::Painter, rect: Rect) {
    let ink = Color32::from_rgb(0x2B, 0x2B, 0x2B);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(0xEC, 0xEA, 0xE3));
    let r = rect.shrink(18.0);
    let stroke = Stroke::new(1.5, ink);
    painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
    let n = 8;
    for i in 1..n {
        let x = r.min.x + r.width() * i as f32 / n as f32;
        painter.line_segment([Pos2::new(x, r.min.y), Pos2::new(x, r.max.y)], stroke);
    }
    painter.line_segment([Pos2::new(r.min.x, r.min.y + 6.0), Pos2::new(r.max.x, r.min.y + 6.0)], stroke);
    painter.line_segment([Pos2::new(r.min.x, r.max.y - 6.0), Pos2::new(r.max.x, r.max.y - 6.0)], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(floors: usize) -> FramingSettings {
        FramingSettings {
            floor_names: (1..=floors).map(|n| format!("Floor {n}")).collect(),
            ..FramingSettings::default()
        }
    }

    #[test]
    fn the_panels_follow_the_floor_count() {
        let d = AutoDialog::new(&settings(2), FramingCatalog::default());
        assert_eq!(
            d.panels(),
            [
                Panel::Foundation,
                Panel::Floor(1),
                Panel::Floor(2),
                Panel::Deck,
                Panel::DeckSupport,
                Panel::Wall,
                Panel::Openings,
                Panel::Fireplaces,
                Panel::Roof,
                Panel::Trusses,
            ]
        );
        assert_eq!(d.form.tabs().len(), d.panels().len());
        for n in 1..=4 {
            let d = AutoDialog::new(&settings(n), FramingCatalog::default());
            assert_eq!(d.form.tabs().len(), d.panels().len(), "{n} floors");
        }
    }

    #[test]
    fn the_blocking_boxes_start_from_the_wall_blocking_and_fold_back() {
        let mut s = settings(1);
        s.walls.wall_blocking = true;
        let mut d = AutoDialog::new(&s, FramingCatalog::default());
        assert!(d.catalog().auto.block_exterior && d.catalog().auto.block_interior);
        d.catalog_mut().auto.block_exterior = false;
        d.catalog_mut().auto.block_interior = false;
        d.finish();
        assert!(!d.settings().walls.wall_blocking);
        d.catalog_mut().auto.block_interior = true;
        d.finish();
        assert!(d.settings().walls.wall_blocking);
    }

    #[test]
    fn every_panel_draws_without_panicking() {
        let ctx = egui::Context::default();
        for floors in [1, 3] {
            let mut d = AutoDialog::new(&settings(floors), FramingCatalog::default());
            for panel in d.panels() {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| d.draw_panel(ui, panel));
                });
            }
        }
    }
}
