//! Framing Member Specification (double-click a member with a framing tool).
//!
//! * General: the member type, lumber size, material, plies, length (plan
//!   length, or height for a post), rise, bottom elevation, rotation (roll
//!   about the member's own axis); for a post with footing also the footing
//!   size and thickness; and the Options: Bearing Beam, Treated, Show Cross,
//!   Show Multi-Ply Lines and the Rotate choice (Flat to Inside or Outside).
//! * End Profile (joists, beams, rafters, General Framing): a shape and size
//!   for each end.
//! * Line Style: line style and weight follow the member's layer, so they
//!   are shown disabled.
//! * Fill Style: the fill in plan view and in a Wall Detail.
//! * Label: Automatic or a Specified label with Insert Macro (Nominal Size,
//!   Width, Depth, Length ...).
//! * Layer: the layer the member is drawn on.
//! * A truss has the panels of `dialogs/truss.rs` instead of General.
//!
//! A Joist Direction Line and a Roof Truss Direction Line open the same window
//! with their own Specification (`DirectionForm`).
//!
//! The dialog edits a cloned [`FramingMember`]; the tool stores the draft on OK
//! (`FramingMemberDialog::apply`).
//!
//! [`FramingDefaultsDialog`] is two views of one window. Opened as a command
//! (Build > Framing > Build Framing, after [`request_build`]) it is Chief's
//! Build Framing dialog: the Automatic Framing Defaults button, the
//! Automatically Rebuild Framing choices and Build Framing Once with a floor
//! choice; OK saves them and builds (`framing_view::set_settings` runs the
//! build when the draft asks). The Automatic Framing Defaults button switches
//! the window to the defaults panels (Floor Levels, Wall, Openings, Roof,
//! Trusses, Posts), which are also what Default Settings > Framing opens.

use super::{
    dis_combo, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::framing_view::FramingSettings;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::geometry::Point;
use plan_framing::span::{allowable_span, SpanUse};
use plan_framing::{
    BearingMode, BlockingStyle, Connection, EndShape, FlatTo, FloorPick, FramingMaterial,
    FramingMember, Group, JoistDirection, LumberSize, ManualMemberKind, OverframeLayer, Splice,
    Truss, TrussSpec, TrussType, WallConnection,
};

/// Member types that take a decorative End Profile.
fn takes_end_profile(kind: ManualMemberKind) -> bool {
    use ManualMemberKind as K;
    matches!(
        kind,
        K::Joist | K::FloorCeilingBeam | K::RoofBeam | K::Rafter | K::GeneralFraming
    )
}

const TABS_PLAIN: &[Tab] = &[
    on("General"),
    on("End Profile"),
    on("Line Style"),
    on("Fill Style"),
    on("Label"),
    on("Layer"),
];
const TABS_NO_PROFILE: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Fill Style"),
    on("Label"),
    on("Layer"),
];

/// The member types the dialog can switch between (layout lines have no
/// lumber and are not members).
pub const KINDS: [ManualMemberKind; 14] = [
    ManualMemberKind::GeneralFraming,
    ManualMemberKind::Post,
    ManualMemberKind::PostWithFooting,
    ManualMemberKind::Blocking,
    ManualMemberKind::Joist,
    ManualMemberKind::JoistBlocking,
    ManualMemberKind::FloorCeilingBeam,
    ManualMemberKind::FloorCeilingTruss,
    ManualMemberKind::Rafter,
    ManualMemberKind::RoofBeam,
    ManualMemberKind::RoofBlocking,
    ManualMemberKind::RoofPurlin,
    ManualMemberKind::RoofTruss,
    ManualMemberKind::GirderTruss,
];

pub const TRUSS_TYPES: [TrussType; 8] = super::truss::TYPES;

const MATERIALS: [FramingMaterial; 5] = [
    FramingMaterial::Lumber,
    FramingMaterial::Steel,
    FramingMaterial::Glulam,
    FramingMaterial::Lvl,
    FramingMaterial::Psl,
];

const MAX_PLIES: u32 = 6;
const MIN_LENGTH: f64 = 1.0;
const MAX_PITCH: f64 = 24.0;

/// Chief's name of a member type.
pub fn kind_label(kind: ManualMemberKind) -> &'static str {
    use ManualMemberKind as K;
    match kind {
        K::GeneralFraming => "General Framing",
        K::Post => "Post",
        K::PostWithFooting => "Post with Footing",
        K::Blocking => "Blocking",
        K::Joist => "Joist",
        K::JoistBlocking => "Joist Blocking",
        K::FloorCeilingBeam => "Floor/Ceiling Beam",
        K::FloorCeilingTruss => "Floor/Ceiling Truss",
        K::BearingLine => "Bearing Line",
        K::Rafter => "Rafter",
        K::RoofBeam => "Roof Beam",
        K::RoofBlocking => "Roof Blocking",
        K::RoofPurlin => "Roof Purlin",
        K::RoofTruss => "Roof Truss",
        K::GirderTruss => "Girder Truss",
        K::TrussBase => "Truss Base",
    }
}

/// The sizes offered in the lumber combo.
pub fn lumber_presets() -> Vec<LumberSize> {
    vec![
        LumberSize::TWO_BY_FOUR,
        LumberSize::TWO_BY_SIX,
        LumberSize::TWO_BY_EIGHT,
        LumberSize::TWO_BY_TEN,
        LumberSize::TWO_BY_TWELVE,
        LumberSize::FOUR_BY_FOUR,
        LumberSize::dim(4, 6),
        LumberSize::FOUR_BY_TEN,
        LumberSize::dim(4, 12),
        LumberSize::SIX_BY_SIX,
        LumberSize::dim(6, 8),
        LumberSize::Glulam {
            width: 5.125,
            depth: 12.0,
        },
        LumberSize::Lvl {
            width: 1.75,
            depth: 11.875,
        },
    ]
}

/// What the Framing Member Specification window is editing.
enum Subject {
    /// A member placed by hand or laid out by Build Framing.
    Member,
    /// A Joist Direction Line.
    JoistDirection(plan_core::Id),
    /// A Roof Truss Direction Line.
    TrussDirection(plan_core::Id),
}

pub struct FramingMemberDialog {
    frame: SpecDialog,
    form: Form,
    subject: Subject,
    direction: Option<DirectionForm>,
}

struct Form {
    draft: FramingMember,
    fields: Fields,
    /// Plan length (post: height) as edited.
    length: f64,
    layers: Vec<String>,
    /// The truss panel's Options that are not in the spec.
    switches: super::truss::Switches,
    /// Plan View Fill Style or Wall Detail Fill Style being edited.
    fill_width: f64,
}

impl FramingMemberDialog {
    pub fn new(member: &FramingMember, layer_names: Vec<String>) -> Self {
        let mut layers = layer_names;
        if !layers.contains(&member.layer_name) {
            layers.insert(0, member.layer_name.clone());
        }
        let form = Form {
            length: if member.kind.is_vertical() {
                member.height
            } else {
                member.plan_length()
            },
            draft: member.clone(),
            fields: Fields::default(),
            layers,
            switches: super::truss::Switches::default(),
            fill_width: 12.0,
        };
        let title = match member.kind {
            ManualMemberKind::RoofTruss => "Roof Truss Specification",
            ManualMemberKind::GirderTruss => "Girder Truss Specification",
            ManualMemberKind::FloorCeilingTruss => "Floor/Ceiling Truss Specification",
            _ => "Framing Member Specification",
        };
        Self {
            frame: SpecDialog::new(title, ("framing_member", member.id)),
            form,
            subject: Subject::Member,
            direction: None,
        }
    }

    /// The dialog of a truss that Build Framing laid out: its
    /// "Automatically Generated Truss" box is on, and stays on unless the
    /// user unchecks it.
    pub fn for_built(mut self) -> Self {
        self.form.switches = super::truss::Switches {
            auto: true,
            was_auto: true,
        };
        self
    }

    /// The Specification of a Joist Direction Line or a Roof Truss Direction
    /// Line (`None` for any other record).
    pub fn for_direction(rec: &crate::editor::framing_view::Record) -> Option<Self> {
        use crate::editor::framing_view::Record;
        let (subject, form, title) = match rec {
            Record::JoistDirection { id, dir } => (
                Subject::JoistDirection(*id),
                DirectionForm::joist(dir.clone()),
                "Joist Direction Specification",
            ),
            Record::TrussDirection { id, dir } => (
                Subject::TrussDirection(*id),
                DirectionForm::truss(dir.clone()),
                "Roof Truss Direction Specification",
            ),
            _ => return None,
        };
        let key = match subject {
            Subject::JoistDirection(i) | Subject::TrussDirection(i) => i,
            Subject::Member => 0,
        };
        let placeholder = FramingMember::new(
            key,
            ManualMemberKind::GeneralFraming,
            Point::ZERO,
            Point::new(1.0, 0.0),
        );
        Some(Self {
            frame: SpecDialog::new(title, ("framing_direction", key)),
            form: Form {
                draft: placeholder,
                fields: Fields::default(),
                length: 1.0,
                layers: Vec::new(),
                switches: super::truss::Switches::default(),
                fill_width: 12.0,
            },
            subject,
            direction: Some(form),
        })
    }

    #[cfg(test)]
    pub fn id(&self) -> plan_core::Id {
        self.form.draft.id
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        match self.direction.as_mut() {
            Some(d) => self.frame.show(ctx, d),
            None => self.frame.show(ctx, &mut self.form),
        }
    }

    pub fn draft(&self) -> &FramingMember {
        &self.form.draft
    }

    /// Stores the edit on the plan (OK): a member's specification, or a
    /// direction line's. Returns whether anything changed.
    pub fn apply(&self, cx: &mut crate::editor::EditorContext) -> bool {
        use crate::editor::framing_view as fv;
        match (&self.subject, &self.direction) {
            (Subject::JoistDirection(id), Some(DirectionForm::Joist(dir))) => {
                fv::apply_record_edit(
                    cx,
                    "Joist Direction Specification",
                    fv::Record::JoistDirection {
                        id: *id,
                        dir: dir.clone(),
                    },
                )
            }
            (Subject::TrussDirection(id), Some(DirectionForm::Truss(dir))) => {
                fv::apply_record_edit(
                    cx,
                    "Roof Truss Direction Specification",
                    fv::Record::TrussDirection {
                        id: *id,
                        dir: dir.clone(),
                    },
                )
            }
            _ => {
                // A truss that was laid out stays laid out while its
                // Automatically Generated Truss box is checked.
                let keep = self.form.switches.was_auto && self.form.switches.auto;
                fv::apply_edit_keep(cx, self.form.draft.clone(), keep)
            }
        }
    }

    /// Edit hooks (what the controls call), public so tests can drive them.
    #[cfg(test)]
    pub fn set_kind(&mut self, kind: ManualMemberKind) {
        self.form.set_kind(kind);
    }

    #[cfg(test)]
    pub fn set_lumber(&mut self, lumber: LumberSize) {
        self.form.set_lumber(lumber);
    }

    #[cfg(test)]
    pub fn set_plies(&mut self, plies: u32) {
        self.form.set_plies(plies);
    }

    #[cfg(test)]
    pub fn set_length(&mut self, length: f64) {
        self.form.set_length(length);
    }

    #[cfg(test)]
    pub fn set_layer(&mut self, layer: &str) {
        self.form.draft.layer_name = layer.to_string();
    }

    #[cfg(test)]
    pub fn set_truss_type(&mut self, kind: TrussType) {
        if let Some(t) = &mut self.form.draft.truss {
            t.kind = kind;
        }
    }

    #[cfg(test)]
    pub fn set_pitch(&mut self, pitch: f64) {
        if let Some(t) = &mut self.form.draft.truss {
            t.pitch = pitch.clamp(0.0, MAX_PITCH);
        }
    }

    /// The truss spec being edited (tests change it like the controls do).
    #[cfg(test)]
    pub fn truss_mut(&mut self) -> Option<&mut TrussSpec> {
        self.form.draft.truss.as_mut()
    }

    /// The draft member (tests change the options like the controls do).
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut FramingMember {
        &mut self.form.draft
    }

    /// The Direction Line being edited.
    #[cfg(test)]
    pub fn direction_mut(&mut self) -> Option<&mut DirectionForm> {
        self.direction.as_mut()
    }

    /// The Automatically Generated Truss box.
    #[cfg(test)]
    pub fn set_auto(&mut self, on: bool) {
        self.form.switches.auto = on;
    }

    /// Chords and webs the truss gets for the current inputs (0 for other
    /// members).
    #[cfg(test)]
    pub fn truss_member_count(&self) -> usize {
        self.form.truss_member_count()
    }
}

/// The Specification of a direction line.
#[derive(Clone, Debug, PartialEq)]
pub enum DirectionForm {
    Joist(plan_framing::JoistDirectionLine),
    Truss(plan_framing::RoofTrussDirection),
}

const DIR_TABS: &[Tab] = &[on("General"), on("Line Style"), on("Arrow")];
const TRUSS_DIR_TABS: &[Tab] = &[on("Roof Trusses"), on("Line Style"), on("Arrow")];

impl DirectionForm {
    pub fn joist(d: plan_framing::JoistDirectionLine) -> Self {
        DirectionForm::Joist(d)
    }

    pub fn truss(d: plan_framing::RoofTrussDirection) -> Self {
        DirectionForm::Truss(d)
    }
}

/// Framing Member Default names the Construction list offers until the
/// management dialog lists the plan's own.
const CONSTRUCTIONS: [&str; 5] = [
    "Floor Joist",
    "Ceiling Joist",
    "I-Joist",
    "Floor Truss",
    "Engineered Joist",
];

impl SpecPages for DirectionForm {
    fn tabs(&self) -> &'static [Tab] {
        match self {
            DirectionForm::Joist(_) => DIR_TABS,
            DirectionForm::Truss(_) => TRUSS_DIR_TABS,
        }
    }

    fn error(&self) -> Option<String> {
        let spacing = match self {
            DirectionForm::Joist(d) => d.spacing,
            DirectionForm::Truss(d) => d.spacing,
        };
        (spacing < 0.0).then(|| "The spacing cannot be negative".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match (self.tabs()[tab].name, &mut *self) {
            ("General", DirectionForm::Joist(d)) => {
                section(ui, "Joists");
                row(ui, "Construction", |ui| {
                    egui::ComboBox::from_id_salt("dir_construction")
                        .selected_text(if d.construction.is_empty() {
                            "Platform default"
                        } else {
                            d.construction.as_str()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut d.construction,
                                String::new(),
                                "Platform default",
                            );
                            for c in CONSTRUCTIONS {
                                ui.selectable_value(&mut d.construction, c.to_string(), c);
                            }
                        });
                    ui.add_enabled(false, egui::Button::new("Define\u{2026}"))
                        .on_hover_text("The Framing Member Defaults Management dialog");
                });
                row(ui, "Depth (0 = platform's)", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.depth)
                            .range(0.0..=24.0)
                            .suffix(" in"),
                    )
                });
                row(ui, "Width (0 = platform's)", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.width)
                            .range(0.0..=12.0)
                            .suffix(" in"),
                    )
                });
                row(ui, "Spacing (0 = platform's)", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.spacing)
                            .range(0.0..=96.0)
                            .suffix(" in"),
                    )
                });
                ui.add_space(6.0);
                ui.weak("These override the Room Specification's joists for the platform the line is drawn in. They apply the next time the framing is built.");
            }
            ("Roof Trusses", DirectionForm::Truss(d)) => {
                section(ui, "Roof Trusses");
                row(ui, "Truss Spacing", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut d.spacing)
                            .range(0.0..=120.0)
                            .suffix(" in"),
                    )
                });
                for (label, v) in [
                    ("Top Chord depth (0 = default)", &mut d.top_chord_depth),
                    (
                        "Bottom Chord depth (0 = top chord's)",
                        &mut d.bottom_chord_depth,
                    ),
                    ("Webbing depth (0 = top chord's)", &mut d.web_depth),
                    ("Maximum Horizontal Span (0 = none)", &mut d.max_span),
                ] {
                    row(ui, label, |ui| {
                        ui.add(egui::DragValue::new(v).range(0.0..=480.0).suffix(" in"))
                    });
                }
                ui.checkbox(&mut d.require_kingpost, "Require Kingpost");
            }
            ("Line Style", _) => {
                section(ui, "Line Style");
                row(ui, "Line style", |ui| dis_combo(ui, "dir_ls", "By layer"));
                row(ui, "Line weight", |ui| dis_combo(ui, "dir_lw", "By layer"));
            }
            ("Arrow", _) => {
                section(ui, "Arrow");
                row(ui, "Arrowheads", |ui| {
                    dis_combo(ui, "dir_arrow", "Both ends")
                });
                ui.weak("Joist and Roof Truss Direction Lines do not use Attach or Auto Position.");
            }
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let (name, spacing) = match self {
            DirectionForm::Joist(d) => ("Joist Direction", d.spacing),
            DirectionForm::Truss(d) => ("Roof Truss Direction", d.spacing),
        };
        pv_text(
            painter,
            rect.center(),
            Align2::CENTER_CENTER,
            if spacing > 0.0 {
                format!("{name}\n{}\" o.c.", fmt_short(spacing))
            } else {
                name.to_string()
            },
            12.0,
        );
    }
}

impl Form {
    fn is_truss(&self) -> bool {
        self.draft.kind.is_truss()
    }

    fn set_kind(&mut self, kind: ManualMemberKind) {
        let old_default = crate::editor::framing_view::layer_for(self.draft.kind);
        // The plain "Framing" layer is every member's fallback.
        let was_default_layer =
            self.draft.layer_name == old_default || self.draft.layer_name == "Framing";
        self.draft.kind = kind;
        if was_default_layer {
            self.draft.layer_name = crate::editor::framing_view::layer_for(kind).to_string();
        }
        if kind.is_truss() {
            if self.draft.truss.is_none() {
                let mut s = TrussSpec::new(TrussType::Fink, self.draft.plan_length(), 6.0);
                s.plies = self.draft.plies.max(1);
                self.draft.truss = Some(s);
            }
            self.sync_truss();
        } else {
            self.draft.truss = None;
            self.draft.width = self.draft.lumber.width() * f64::from(self.draft.plies.max(1));
        }
        if kind.is_vertical() {
            // A post stands on its start point.
            self.draft.end = self.draft.start;
            if self.draft.height <= 0.0 {
                self.draft.height = self.length.max(MIN_LENGTH);
            }
            self.length = self.draft.height;
        }
        if kind == ManualMemberKind::PostWithFooting && self.draft.footing_spec.size <= 0.0 {
            self.draft.footing_spec = plan_framing::FootingSpec::default();
        }
    }

    fn set_lumber(&mut self, lumber: LumberSize) {
        let label_was_default = self.draft.label == default_label(&self.draft);
        let (rise, rotation) = (self.draft.rise, self.draft.rotation);
        self.draft = self.draft.clone().with_lumber(lumber);
        self.draft.rise = rise;
        self.draft.rotation = rotation;
        if !label_was_default {
            // `with_lumber` rewrote the label; keep the edited text.
            return;
        }
        self.draft.label = default_label(&self.draft);
    }

    fn set_plies(&mut self, plies: u32) {
        let plies = plies.clamp(1, MAX_PLIES);
        self.draft.plies = plies;
        self.sync_truss();
        if !self.is_truss() {
            self.draft.width = self.draft.lumber.width() * f64::from(plies);
        }
    }

    /// Keeps a truss's spec in step with the member (plies, span, thickness).
    fn sync_truss(&mut self) {
        let (plies, span) = (self.draft.plies.max(1), self.draft.plan_length());
        if let Some(t) = &mut self.draft.truss {
            t.plies = plies;
            t.span = span;
            self.draft.width = t.thickness();
        }
    }

    fn set_length(&mut self, length: f64) {
        let length = length.max(0.0);
        self.length = length;
        if self.draft.kind.is_vertical() {
            self.draft.height = length;
            return;
        }
        let dir = if self.draft.plan_length() > 1e-9 {
            (self.draft.end - self.draft.start).normalized()
        } else {
            Point::new(1.0, 0.0)
        };
        self.draft.end = self.draft.start + dir * length;
        self.sync_truss();
    }

    fn truss_member_count(&self) -> usize {
        self.draft.truss.as_ref().map_or(0, |t| {
            let mut s = t.clone();
            s.span = self.draft.plan_length();
            Truss::generate(&s).members.len()
        })
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Type", |ui| {
            let mut kind = self.draft.kind;
            egui::ComboBox::from_id_salt("framing_kind")
                .selected_text(kind_label(kind))
                .show_ui(ui, |ui| {
                    for k in KINDS {
                        ui.selectable_value(&mut kind, k, kind_label(k));
                    }
                });
            if kind != self.draft.kind {
                self.set_kind(kind);
            }
        });
        row(ui, "Lumber Size", |ui| {
            let mut sel = self.draft.lumber;
            let mut options = lumber_presets();
            if !options.contains(&sel) {
                options.insert(0, sel);
            }
            egui::ComboBox::from_id_salt("framing_lumber")
                .selected_text(sel.name())
                .show_ui(ui, |ui| {
                    for o in options {
                        ui.selectable_value(&mut sel, o, o.name());
                    }
                });
            if sel != self.draft.lumber {
                self.set_lumber(sel);
            }
        });
        row(ui, "Material", |ui| {
            egui::ComboBox::from_id_salt("framing_material")
                .selected_text(self.draft.material.name())
                .show_ui(ui, |ui| {
                    for m in MATERIALS {
                        ui.selectable_value(&mut self.draft.material, m, m.name());
                    }
                });
        });
        row(ui, "Plies", |ui| {
            let mut plies = self.draft.plies.max(1);
            if ui
                .add(egui::DragValue::new(&mut plies).range(1..=MAX_PLIES))
                .changed()
            {
                self.set_plies(plies);
            }
        });

        section(ui, "Dimensions");
        let label = if self.draft.kind.is_vertical() {
            "Height"
        } else if self.is_truss() {
            "Span"
        } else {
            "Length (plan)"
        };
        let mut length = self.length;
        if self.fields.length_row(ui, label, "length", &mut length) {
            self.set_length(length);
        }
        if !self.draft.kind.is_vertical() && !self.is_truss() {
            self.fields
                .length_row(ui, "Rise (end - start)", "rise", &mut self.draft.rise);
            row(ui, "Cut length", |ui| {
                ui.label(fmt_short(self.draft.length()))
            });
        }
        self.fields.length_row(
            ui,
            "Bottom elevation",
            "elevation",
            &mut self.draft.elevation_bottom,
        );
        if !self.is_truss() {
            self.fields
                .degrees_row(ui, "Rotation", "deg_rotation", &mut self.draft.rotation);
        }
        self.options(ui);
        if self.draft.kind == ManualMemberKind::PostWithFooting {
            section(ui, "Footing");
            self.fields.length_row(
                ui,
                "Footing size",
                "footing",
                &mut self.draft.footing_spec.size,
            );
            self.fields.length_row(
                ui,
                "Footing thickness",
                "footing_t",
                &mut self.draft.footing_spec.thickness,
            );
        }
    }

    /// The Options of the General panel (manual p. 929).
    fn options(&mut self, ui: &mut Ui) {
        use ManualMemberKind as K;
        section(ui, "Options");
        let kind = self.draft.kind;
        ui.add_enabled_ui(kind == K::FloorCeilingBeam, |ui| {
            ui.checkbox(&mut self.draft.bearing_beam, "Bearing Beam").on_hover_text(
                "Joists run across the beam and lap or butt over it; a beam standing 1\" or more above the joists holds them by its sides",
            );
        });
        ui.checkbox(&mut self.draft.treated, "Treated");
        ui.add_enabled_ui(kind.is_vertical(), |ui| {
            ui.checkbox(&mut self.draft.show_cross, "Show Cross")
                .on_hover_text("Draws the post as a cross box in plan view");
        });
        ui.add_enabled_ui(
            matches!(
                kind,
                K::Post | K::PostWithFooting | K::FloorCeilingBeam | K::RoofBeam
            ) && self.draft.plies > 1,
            |ui| {
                ui.checkbox(&mut self.draft.show_ply_lines, "Show Multi-Ply Lines");
            },
        );
        ui.add_enabled_ui(matches!(kind, K::GeneralFraming | K::Blocking), |ui| {
            row(ui, "Rotate", |ui| {
                egui::ComboBox::from_id_salt("framing_flat")
                    .selected_text(self.draft.flat.name())
                    .show_ui(ui, |ui| {
                        for f in FlatTo::ALL {
                            ui.selectable_value(&mut self.draft.flat, f, f.name());
                        }
                    });
            });
        });
    }

    /// The End Profile panel: a shape and size for each end.
    fn profile(&mut self, ui: &mut Ui) {
        section(ui, "End Profile");
        for (i, name) in ["Start", "End"].into_iter().enumerate() {
            row(ui, name, |ui| {
                let p = &mut self.draft.end_profile[i];
                egui::ComboBox::from_id_salt(("framing_end_shape", i))
                    .selected_text(p.shape.name())
                    .show_ui(ui, |ui| {
                        for s in EndShape::ALL {
                            ui.selectable_value(&mut p.shape, s, s.name());
                        }
                    });
                ui.add_enabled(
                    p.shape != EndShape::Square,
                    egui::DragValue::new(&mut p.size)
                        .range(0.0..=24.0)
                        .speed(0.1)
                        .suffix(" in"),
                );
            });
        }
        ui.add_space(6.0);
        ui.weak("A decorative profile cut into the end of a joist, beam, rafter or General Framing member; it shows in the specification and the cut list.");
    }

    /// The Fill Style panel: the fill in plan view and in a Wall Detail.
    fn fill(&mut self, ui: &mut Ui) {
        use plan_core::fill_styles::FillStyle;
        let layer_rgb = [180, 140, 60];
        for (salt, title, wall_detail) in [
            ("fill_plan", "Plan View Fill Style", false),
            ("fill_detail", "Wall Detail Fill Style", true),
        ] {
            section(ui, title);
            let slot = if wall_detail {
                &mut self.draft.detail_fill
            } else {
                &mut self.draft.fill
            };
            let mut custom = slot.is_some();
            if ui
                .checkbox(&mut custom, "Custom fill for this member")
                .changed()
            {
                *slot = custom.then(FillStyle::default);
            }
            if let Some(style) = slot.as_mut() {
                super::fill_style::panel(
                    ui,
                    salt,
                    style,
                    &[],
                    layer_rgb,
                    &mut self.fill_width,
                    false,
                );
            }
        }
    }

    /// The Label panel: Automatic Label or a Specified one with macros.
    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        let mut specify = !self.draft.custom_label.is_empty();
        ui.horizontal(|ui| {
            if ui.radio(!specify, "Automatic Label").clicked() {
                self.draft.custom_label.clear();
                specify = false;
            }
            if ui.radio(specify, "Specify Label").clicked() && !specify {
                self.draft.custom_label = "%nominal_size%".into();
                specify = true;
            }
        });
        if specify {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft.custom_label).desired_width(240.0),
                );
                ui.menu_button("Insert Macro", |ui| {
                    for (name, help) in crate::editor::framing_view::LABEL_MACROS {
                        if ui.button(*name).on_hover_text(*help).clicked() {
                            self.draft.custom_label.push_str(&format!("%{name}%"));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.weak(format!(
                "The label reads: {}",
                crate::editor::framing_view::label_text(&self.draft)
            ));
        } else {
            ui.weak("A framing member has no automatic label on the plan; specify one to show it on the \"Framing, Labels\" layer.");
        }
    }

    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        row(ui, "Line style", |ui| {
            dis_combo(ui, "framing_ls", "By layer")
        });
        row(ui, "Line weight", |ui| {
            dis_combo(ui, "framing_lw", "By layer")
        });
        ui.add_space(6.0);
        ui.weak("Line style and weight follow the member's layer.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            super::select_layer::layer_field(
                ui,
                "framing_layer",
                &mut self.draft.layer_name,
                self.layers.iter().map(String::as_str),
            );
        });
    }
}

fn default_label(m: &FramingMember) -> String {
    format!("{} {}", m.lumber.name(), m.kind.name())
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        if self.is_truss() {
            super::truss::ROOF_TABS
        } else if takes_end_profile(self.draft.kind) {
            TABS_PLAIN
        } else {
            TABS_NO_PROFILE
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.length < MIN_LENGTH {
            return Some("The length must be at least 1\"".into());
        }
        if self.draft.layer_name.trim().is_empty() {
            return Some("Pick a layer".into());
        }
        if self.draft.kind == ManualMemberKind::PostWithFooting
            && (self.draft.footing_spec.size <= 0.0 || self.draft.footing_spec.thickness <= 0.0)
        {
            return Some("The footing needs a size and a thickness".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match self.tabs()[tab].name {
            "General" if self.is_truss() => {
                let Form {
                    draft,
                    switches,
                    fields,
                    ..
                } = self;
                super::truss::general(draft, switches, fields, ui);
                // The span is the member's length; the plies make its width.
                let len = draft.plan_length();
                if let Some(t) = draft.truss.as_mut() {
                    t.span = len;
                    t.plies = t.plies.max(1);
                    draft.plies = t.plies;
                    draft.width = t.thickness();
                }
            }
            "General" => self.general(ui),
            "End Profile" => self.profile(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill(ui),
            "Label" => self.label(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let m = &self.draft;
        if let (true, Some(spec)) = (m.kind.is_truss(), &m.truss) {
            let mut s = spec.clone();
            s.span = m.plan_length().max(MIN_LENGTH * 13.0);
            let truss = Truss::generate(&s);
            let env = &truss.envelope;
            let (w, h) = ((env.max_x - env.min_x).max(1.0), env.peak_height.max(1.0));
            let scale = f64::from(rect.width() / 1.05) / w;
            let scale = scale.min(f64::from(rect.height() * 0.6) / h);
            let to = |p: Point| {
                Pos2::new(
                    rect.center().x + ((p.x - (env.min_x + env.max_x) / 2.0) * scale) as f32,
                    rect.center().y + (h * scale / 2.0) as f32 - (p.y * scale) as f32,
                )
            };
            for tm in &truss.members {
                painter.line_segment([to(tm.a), to(tm.b)], Stroke::new(1.2_f32, PV_INK));
            }
            pv_text(
                painter,
                rect.center_bottom(),
                Align2::CENTER_BOTTOM,
                format!("{} truss, {}:12", s.kind.name(), s.pitch),
                11.0,
            );
            return;
        }
        // The section to scale, with its size under it.
        let (w, d) = (m.width, m.depth);
        let scale = f64::from(rect.width().min(rect.height()) * 0.6) / w.max(d).max(1.0);
        let (pw, ph) = ((w * scale) as f32, (d * scale) as f32);
        let r = Rect::from_center_size(rect.center(), egui::vec2(pw, ph));
        painter.rect_filled(r, 1.0, PV_WALL);
        painter.rect_stroke(
            r,
            1.0,
            Stroke::new(1.2_f32, PV_INK),
            egui::StrokeKind::Inside,
        );
        pv_text(
            painter,
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            format!("{} {}", m.lumber.name(), kind_label(m.kind)),
            11.0,
        );
        painter.text(
            rect.center_top(),
            Align2::CENTER_TOP,
            fmt_short(self.length),
            egui::FontId::proportional(11.0),
            PV_FAINT,
        );
    }
}

// ===================================================================
// Framing Defaults
// ===================================================================

/// The panels of the Automatic Framing Defaults (manual p. 889): the floor
/// levels, the walls and their openings, the roof, trusses and posts.
const DEFAULTS_TABS: &[Tab] = &[
    on("Floor Levels"),
    on("Wall"),
    on("Openings"),
    on("Roof"),
    on("Trusses"),
    on("Posts"),
];

/// The Build Framing command (manual p. 914).
const BUILD_TABS: &[Tab] = &[on("Build Framing")];

thread_local! {
    /// Set by [`request_build`]; the next [`FramingDefaultsDialog`] opens as
    /// the Build Framing dialog.
    static BUILD_REQUEST: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Makes the next Framing dialog the Build Framing dialog (Build > Framing >
/// Build Framing...). `all_floors` starts with "Build every floor" checked.
pub fn request_build(all_floors: bool) {
    BUILD_REQUEST.with(|c| c.set(Some(all_floors)));
}

/// The lumber sizes the Framing Defaults page offers for a member kind.
const SIZES: [plan_framing::Lumber; 5] = [
    plan_framing::TWO_BY_FOUR,
    plan_framing::TWO_BY_SIX,
    plan_framing::TWO_BY_EIGHT,
    plan_framing::TWO_BY_TEN,
    plan_framing::TWO_BY_TWELVE,
];

/// Chief's Framing Defaults page (Default Settings > Framing): the size and
/// spacing of each kind of member Build Framing makes, the header table, the
/// corner / tee backing, wall blocking, floor and roof framing. It edits a
/// [`FramingSettings`]; the caller stores it with
/// `framing_view::set_settings` on OK.
pub struct FramingDefaultsDialog {
    frame: SpecDialog,
    form: DefaultsForm,
}

struct DefaultsForm {
    draft: FramingSettings,
    fields: Fields,
    /// The Build Framing command rather than the Framing Defaults page.
    build_mode: bool,
    /// The Build Framing command is showing its Automatic Framing Defaults.
    sub: bool,
    /// The draft as it was when the defaults opened, for Cancel.
    backup: Option<FramingSettings>,
    /// The window goes back to its first tab on the next frame.
    reset_tab: bool,
}

impl FramingDefaultsDialog {
    /// The Framing Defaults page, or the Build Framing dialog when
    /// [`request_build`] was called since the last one opened.
    pub fn new(settings: &FramingSettings) -> Self {
        let request = BUILD_REQUEST.with(std::cell::Cell::take);
        let mut draft = settings.clone();
        draft.build_on_ok = request;
        Self {
            frame: if request.is_some() {
                SpecDialog::new("Build Framing", "framing_build")
            } else {
                SpecDialog::new("Framing Defaults", "framing_defaults")
            },
            form: DefaultsForm {
                draft,
                fields: Fields::default(),
                build_mode: request.is_some(),
                sub: false,
                backup: None,
                reset_tab: false,
            },
        }
    }

    /// Whether this is the Build Framing dialog (OK builds).
    #[cfg(test)]
    pub fn is_build(&self) -> bool {
        self.form.build_mode
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let out = self.frame.show(ctx, &mut self.form);
        if self.form.build_mode && self.form.sub {
            // OK and Cancel of the Automatic Framing Defaults return to the
            // Build Framing command (Cancel puts the defaults back).
            match out {
                Outcome::Ok => {
                    self.form.sub = false;
                    self.form.backup = None;
                    self.frame.start_on(0);
                    return Outcome::Open;
                }
                Outcome::Cancel => {
                    if let Some(b) = self.form.backup.take() {
                        self.form.draft = b;
                    }
                    self.form.sub = false;
                    self.frame.start_on(0);
                    return Outcome::Open;
                }
                Outcome::Open => {}
            }
        }
        if self.form.reset_tab {
            self.form.reset_tab = false;
            self.frame.start_on(0);
        }
        out
    }

    pub fn draft(&self) -> &FramingSettings {
        &self.form.draft
    }

    /// Opens on the tab at index `tab` (Default Settings > Framing leaves);
    /// the Build Framing dialog keeps its first tab.
    pub fn start_on(&mut self, tab: usize) {
        if !self.form.build_mode {
            self.frame.start_on(tab.min(DEFAULTS_TABS.len() - 1));
        }
    }

    /// Presses the Automatic Framing Defaults button of the Build Framing
    /// command (tests drive it like the button).
    #[cfg(test)]
    pub fn open_automatic_defaults(&mut self) {
        self.form.enter_defaults();
    }

    /// Whether the window is showing the Automatic Framing Defaults.
    #[cfg(test)]
    pub fn in_defaults(&self) -> bool {
        !self.form.build_mode || self.form.sub
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut FramingSettings {
        &mut self.form.draft
    }
}

/// A combo box over [`SIZES`] (a size that is not among them stays listed).
fn size_combo(ui: &mut Ui, salt: &str, value: &mut plan_framing::Lumber) {
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

fn count_row(ui: &mut Ui, label: &str, value: &mut u32, max: u32) {
    row(ui, label, |ui| {
        ui.add(egui::DragValue::new(value).range(0..=max));
    });
}

impl DefaultsForm {
    fn walls(&mut self, ui: &mut Ui) {
        let d = &mut self.draft.walls;
        section(ui, "Studs and plates");
        row(ui, "Stud size", |ui| {
            size_combo(ui, "fd_stud", &mut d.stud_size)
        });
        self.fields
            .length_row(ui, "Stud spacing", "fd_stud_spacing", &mut d.stud_spacing);
        ui.weak("A 2x4 stud becomes 2x6 in walls 6\" and thicker.");
        count_row(ui, "Top plates", &mut d.top_plates, 3);
        count_row(ui, "Bottom plates", &mut d.bottom_plates, 2);
        section(ui, "Openings");
        count_row(ui, "King studs per side", &mut d.king_studs, 3);
        count_row(ui, "Trimmers per side", &mut d.trimmers, 3);
        self.fields.length_row(
            ui,
            "Cripple spacing",
            "fd_cripple_spacing",
            &mut d.cripple_spacing,
        );
        section(ui, "Corners, tees and blocking");
        count_row(ui, "Corner studs", &mut d.corner_studs, 3);
        count_row(ui, "Tee backing studs per side", &mut d.tee_studs, 3);
        ui.checkbox(&mut d.wall_blocking, "Blocking between studs");
        ui.add_enabled_ui(d.wall_blocking, |ui| {
            self.fields.length_row(
                ui,
                "Blocking spacing",
                "fd_block_spacing",
                &mut d.wall_blocking_spacing,
            );
        });
        let o = &mut self.draft.build.detail;
        ui.checkbox(&mut o.stagger_blocking, "Stagger Blocking")
            .on_hover_text("Blocking alternates on either side of a horizontal centre line");
        section(ui, "Wall connections");
        row(ui, "Wall corners", |ui| {
            connection_combo(ui, "fd_corner", &mut o.corner_style, true)
        });
        row(ui, "Wall intersections", |ui| {
            connection_combo(ui, "fd_tee", &mut o.tee_style, false)
        });
        ui.weak("Standard: three studs. Reduced Stud: two. Laddered: two with ladder blocking. U Shaped (corners only): three in a U.");
        section(ui, "Plates");
        row(ui, "Top plate connection", |ui| {
            style_combo(ui, "fd_plate_conn", &mut o.top_plate_connection)
        });
        section(ui, "Mitre ends of angled walls");
        ui.checkbox(&mut o.mitre_plate_ends, "Mitre Plate Ends");
        ui.checkbox(&mut o.rotate_end_studs, "Rotate End Studs");
        ui.checkbox(&mut o.frame_through_horizontal, "Horizontal Frame Through");
        section(ui, "Wall Detail views");
        ui.checkbox(
            &mut o.details_from_exterior,
            "Build Wall Framing Details from Exterior",
        );
        ui.checkbox(
            &mut o.show_cross,
            "Draw studs and posts as cross boxes in plan",
        );
        ui.checkbox(
            &mut o.bearing_wall_headers,
            "Bearing walls: double plates and headers",
        );
    }

    fn headers(&mut self, ui: &mut Ui) {
        let d = &mut self.draft.walls;
        section(ui, "Header size by opening width");
        count_row(ui, "Plies", &mut d.header_plies, 4);
        let mut remove: Option<usize> = None;
        let rows = d.header_table.len();
        for (i, r) in d.header_table.iter_mut().enumerate() {
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
                size_combo(ui, &format!("fd_header_{i}"), &mut r.lumber);
                if rows > 1 && ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.header_table.remove(i);
            // The last row always covers everything wider.
            if let Some(last) = d.header_table.last_mut() {
                last.up_to = 1.0e9;
            }
        }
        if ui.button("Add a row").clicked() {
            let before = d.header_table.len().saturating_sub(1);
            let up_to = d
                .header_table
                .get(before.saturating_sub(1))
                .map_or(36.0, |r| r.up_to + 12.0);
            let lumber = d
                .header_table
                .last()
                .map_or(plan_framing::TWO_BY_SIX, |r| r.lumber);
            d.header_table
                .insert(before, plan_framing::HeaderRow { up_to, lumber });
        }
        // Rows stay in order of width.
        let last = d.header_table.len().saturating_sub(1);
        d.header_table[..last].sort_by(|a, b| a.up_to.total_cmp(&b.up_to));
        section(ui, "Or a fixed header");
        self.fields.length_row(
            ui,
            "Header depth (0 = table)",
            "fd_header_depth",
            &mut d.header_depth,
        );
        let o = &mut self.draft.build.detail;
        section(ui, "Maximum depth");
        self.fields.length_row(
            ui,
            "Maximum Depth (0 = off)",
            "fd_header_max",
            &mut o.header_max_depth,
        );
        ui.weak("An opening whose top is this close to the top plate gets one solid header filling the space and no cripples.");
        section(ui, "Materials List");
        ui.checkbox(&mut o.list_cut_header_lengths, "List Cut Header Lengths");
    }

    fn floor(&mut self, ui: &mut Ui) {
        let d = &mut self.draft.walls;
        section(ui, "Floor joists");
        row(ui, "Joist size", |ui| {
            size_combo(ui, "fd_joist", &mut d.joist_size)
        });
        self.fields.length_row(
            ui,
            "Joist spacing",
            "fd_joist_spacing",
            &mut d.joist_spacing,
        );
        ui.checkbox(&mut d.rim_joist, "Rim joists");
        ui.checkbox(&mut d.blocking, "Mid-span blocking");
        row(ui, "Joists run", |ui| {
            direction_combo(ui, "fd_joist_dir", &mut d.joist_direction)
        });
        row(ui, "Joists bear on", |ui| {
            egui::ComboBox::from_id_salt("fd_bearing")
                .selected_text(d.bearing.name())
                .show_ui(ui, |ui| {
                    for m in BearingMode::ALL {
                        ui.selectable_value(&mut d.bearing, m, m.name());
                    }
                });
        });
        ui.weak("Bearing Lines drawn with the Bearing Line tool always carry joists, and so do walls marked Bearing Wall and beams marked Bearing Beam when only the exterior walls bear. Rooms with no floor (Open Below, decks) get no floor framing.");
        Self::span_note(ui, SpanUse::Floor, d.joist_size, d.joist_spacing);
        section(ui, "Ceiling joists");
        row(ui, "Ceiling joist size", |ui| {
            size_combo(ui, "fd_ceiling_joist", &mut d.ceiling_joist_size)
        });
        self.fields.length_row(
            ui,
            "Ceiling joist spacing",
            "fd_ceiling_spacing",
            &mut d.ceiling_joist_spacing,
        );
        Self::span_note(
            ui,
            SpanUse::Ceiling,
            d.ceiling_joist_size,
            d.ceiling_joist_spacing,
        );
        section(ui, "Holes (stairwells)");
        count_row(ui, "Header and trimmer plies", &mut d.hole_plies, 4);
        ui.weak("Trimmers run beside the hole, headers across its ends.");
        let o = &mut self.draft.build.detail;
        section(ui, "Joists over bearing walls and beams");
        row(ui, "Floor joists", |ui| {
            splice_combo(ui, "fd_splice", &mut o.splice)
        });
        row(ui, "Ceiling joists", |ui| {
            splice_combo(ui, "fd_csplice", &mut o.ceiling_splice)
        });
        ui.weak("Lap: side by side, 8\" of lap centred on the support. Butt: end to end over the support.");
        section(ui, "Blocking");
        row(ui, "Floor blocking", |ui| {
            blocking_combo(ui, "fd_fblock", &mut o.blocking_style)
        });
        row(ui, "Ceiling blocking", |ui| {
            blocking_combo(ui, "fd_cblock", &mut o.ceiling_blocking_style)
        });
        section(ui, "Rim joists");
        self.fields
            .length_row(ui, "Rim Joist Width", "fd_rim_width", &mut o.rim_width);
        row(ui, "Rim Joist Connection", |ui| {
            style_combo(ui, "fd_rim_conn", &mut o.rim_connection)
        });
        self.fields.length_row(
            ui,
            "Max Rim Joist Length (0 = full)",
            "fd_rim_max",
            &mut o.max_rim_length,
        );
        section(ui, "Use Framing Reference");
        let names = self.draft.floor_names.clone();
        let build = &mut self.draft.build;
        if names.is_empty() {
            ui.weak("The plan has no floors.");
        }
        for (i, name) in names.iter().enumerate() {
            let mut on_ = build.uses_reference(i);
            if ui
                .checkbox(
                    &mut on_,
                    format!("{name}: start the joist layout at the Framing Reference Marker"),
                )
                .changed()
            {
                build.set_reference(i, on_);
            }
        }
    }

    fn roof(&mut self, ui: &mut Ui) {
        use plan_framing::OverhangCut;
        let r = &mut self.draft.roof;
        section(ui, "Rafters");
        row(ui, "Rafter size", |ui| {
            size_combo(ui, "fd_rafter", &mut r.rafter)
        });
        self.fields
            .length_row(ui, "Rafter spacing", "fd_rafter_spacing", &mut r.spacing);
        Self::span_note(ui, SpanUse::Rafter, r.rafter, r.spacing);
        row(ui, "Ridge", |ui| size_combo(ui, "fd_ridge", &mut r.ridge));
        row(ui, "Hip and valley", |ui| {
            size_combo(ui, "fd_hip", &mut r.hip_valley);
        });
        row(ui, "Fascia", |ui| {
            size_combo(ui, "fd_fascia", &mut r.fascia)
        });
        section(ui, "Cuts");
        row(ui, "Tail cut", |ui| {
            egui::ComboBox::from_id_salt("fd_tail")
                .selected_text(match r.overhang_cut {
                    OverhangCut::Plumb => "Plumb",
                    OverhangCut::Level => "Level",
                    OverhangCut::Square => "Square",
                })
                .show_ui(ui, |ui| {
                    for (c, name) in [
                        (OverhangCut::Plumb, "Plumb"),
                        (OverhangCut::Level, "Level"),
                        (OverhangCut::Square, "Square"),
                    ] {
                        ui.selectable_value(&mut r.overhang_cut, c, name);
                    }
                });
        });
        ui.weak("A plane's own Eave cut wins over this one.");
        self.fields
            .length_row(ui, "Birdsmouth seat", "fd_seat", &mut r.birdsmouth_seat);
        section(ui, "Other roof framing");
        ui.checkbox(&mut r.collar_ties, "Collar ties");
        ui.checkbox(&mut r.ceiling_joists, "Ceiling joists");
        ui.checkbox(&mut r.trusses, "Trusses instead of rafters");
        self.fields.length_row(
            ui,
            "Truss spacing",
            "fd_truss_spacing",
            &mut r.truss_spacing,
        );
        self.fields.length_row(
            ui,
            "Trusses over a span of (0 = never)",
            "fd_truss_span",
            &mut r.use_trusses_over_span,
        );
        section(ui, "Layout");
        ui.checkbox(
            &mut self.draft.build.roof_reference,
            "Use Framing Reference",
        )
        .on_hover_text(
            "Rafters and trusses start at the Framing Reference Marker (useful for gable roofs)",
        );
        ui.checkbox(&mut r.trim_to_soffits, "Trim Framing To Soffits");
        section(ui, "Roof Lookouts");
        ui.checkbox(&mut r.lookouts, "Lookouts under gable overhangs");
        ui.add_enabled_ui(r.lookouts, |ui| {
            self.fields
                .length_row(ui, "Lookout Spacing", "fd_lookout", &mut r.lookout_spacing);
            self.fields.length_row(
                ui,
                "Offset from Subfascia (0 = match spacing)",
                "fd_lookout_off",
                &mut r.lookout_offset,
            );
            self.fields
                .length_row(ui, "Gable overhang", "fd_rake", &mut r.rake_overhang);
        });
        section(ui, "Hip Girder Truss");
        count_row(ui, "Count", &mut r.hip_girder_count, 4);
        self.fields.length_row(
            ui,
            "Distance from Wall Main Layer (0 = automatic)",
            "fd_girder",
            &mut r.hip_girder_distance,
        );
        section(ui, "Roof Overframing");
        ui.checkbox(&mut r.overframing, "Roof Overframing (shoe plates)");
        ui.add_enabled_ui(r.overframing, |ui| {
            row(ui, "Overframe Layer", |ui| {
                egui::ComboBox::from_id_salt("fd_overframe")
                    .selected_text(r.overframe_layer.name())
                    .show_ui(ui, |ui| {
                        for l in OverframeLayer::ALL {
                            ui.selectable_value(&mut r.overframe_layer, l, l.name());
                        }
                    });
            });
        });
    }
}

fn splice_combo(ui: &mut Ui, salt: &str, v: &mut Splice) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(v.name())
        .show_ui(ui, |ui| {
            for s in Splice::ALL {
                ui.selectable_value(v, s, s.name());
            }
        });
}

fn blocking_combo(ui: &mut Ui, salt: &str, v: &mut BlockingStyle) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(v.name())
        .show_ui(ui, |ui| {
            for s in BlockingStyle::ALL {
                ui.selectable_value(v, s, s.name());
            }
        });
}

fn style_combo(ui: &mut Ui, salt: &str, v: &mut Connection) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(v.name())
        .show_ui(ui, |ui| {
            for s in Connection::ALL {
                ui.selectable_value(v, s, s.name());
            }
        });
}

fn connection_combo(ui: &mut Ui, salt: &str, v: &mut WallConnection, corner: bool) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(v.name())
        .show_ui(ui, |ui| {
            for s in WallConnection::ALL {
                // U Shaped is only for corners.
                if s == WallConnection::UShaped && !corner {
                    continue;
                }
                ui.selectable_value(v, s, s.name());
            }
        });
}

/// A combo over the joist directions.
fn direction_combo(ui: &mut Ui, salt: &str, value: &mut JoistDirection) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(direction_name(*value))
        .show_ui(ui, |ui| {
            for d in [
                JoistDirection::Auto,
                JoistDirection::AlongX,
                JoistDirection::AlongY,
            ] {
                ui.selectable_value(value, d, direction_name(d));
            }
        });
}

fn direction_name(d: JoistDirection) -> &'static str {
    match d {
        JoistDirection::Auto => "Across the shorter side",
        JoistDirection::AlongX => "Parallel to X (left to right)",
        JoistDirection::AlongY => "Parallel to Y (up and down)",
    }
}

/// `15' 7"` for a span in inches.
fn feet_inches(inches: f64) -> String {
    let ft = (inches / 12.0).floor();
    format!("{ft}' {}\"", (inches - ft * 12.0).round())
}

impl DefaultsForm {
    /// The Automatic Framing Defaults button of the Build Framing command.
    fn enter_defaults(&mut self) {
        self.backup = Some(self.draft.clone());
        self.sub = true;
        self.reset_tab = true;
    }

    fn span_note(ui: &mut Ui, use_: SpanUse, lumber: plan_framing::Lumber, spacing: f64) {
        let kind = match use_ {
            SpanUse::Floor => super::code_notice::SpanKind::Floor,
            SpanUse::Ceiling => super::code_notice::SpanKind::Ceiling,
            SpanUse::Rafter => super::code_notice::SpanKind::Rafter,
        };
        super::code_notice::span_check(ui, kind, lumber.depth, spacing);
        ui.weak(format!(
            "A {} at {}\" o.c. carries about {} as a {} (planning value, not a code check).",
            lumber.nominal_name(),
            fmt_short(spacing),
            feet_inches(allowable_span(use_, lumber, spacing)),
            use_.name()
        ));
    }

    fn posts(&mut self, ui: &mut Ui) {
        section(ui, "New posts");
        let p = &mut self.draft.build.posts;
        row(ui, "Post size", |ui| {
            egui::ComboBox::from_id_salt("bp_size")
                .selected_text(p.size.name())
                .show_ui(ui, |ui| {
                    for l in lumber_presets() {
                        ui.selectable_value(&mut p.size, l, l.name());
                    }
                });
        });
        row(ui, "Material", |ui| {
            egui::ComboBox::from_id_salt("bp_material")
                .selected_text(p.material.name())
                .show_ui(ui, |ui| {
                    for m in MATERIALS {
                        ui.selectable_value(&mut p.material, m, m.name());
                    }
                });
        });
        ui.checkbox(&mut p.footing, "Footing under every new post");
        ui.weak("The Post and Post with Footing tools start with these.");
    }

    fn trusses(&mut self, ui: &mut Ui) {
        section(ui, "Trusses over a Truss Base");
        let t = &mut self.draft.build.trusses;
        row(ui, "Truss type", |ui| {
            egui::ComboBox::from_id_salt("bt_type")
                .selected_text(t.kind.name())
                .show_ui(ui, |ui| {
                    for k in TRUSS_TYPES {
                        ui.selectable_value(&mut t.kind, k, k.name());
                    }
                });
        });
        row(ui, "Top chord pitch (rise per 12)", |ui| {
            ui.add(
                egui::DragValue::new(&mut t.pitch)
                    .range(0.0..=MAX_PITCH)
                    .speed(0.1),
            );
        });
        self.fields
            .length_row(ui, "Heel height", "bt_heel", &mut t.heel_height);
        self.fields
            .length_row(ui, "Overhang", "bt_overhang", &mut t.overhang);
        self.fields
            .length_row(ui, "Spacing", "bt_spacing", &mut t.spacing);
        ui.weak(
            "Used where a Truss Base is drawn and no Roof Truss Direction gives its own spacing.",
        );
        section(ui, "Roof planes");
        let r = &mut self.draft.roof;
        ui.checkbox(&mut r.trusses, "Trusses instead of rafters");
        self.fields.length_row(
            ui,
            "Roof truss spacing",
            "bt_roof_spacing",
            &mut r.truss_spacing,
        );
        self.fields.length_row(
            ui,
            "Trusses over a span of (0 = never)",
            "bt_span",
            &mut r.use_trusses_over_span,
        );
    }

    /// A group's switch in the Automatically Rebuild Framing list.
    fn auto_row(&mut self, ui: &mut Ui, g: Group, label: &str, hint: &str) {
        let opts = &mut self.draft.build;
        let mut auto = opts.auto_rebuild.get(g);
        ui.checkbox(&mut auto, label).on_hover_text(hint);
        opts.auto_rebuild.set(g, auto);
    }

    /// The floor choice of a Build Framing Once line.
    fn floor_pick(ui: &mut Ui, salt: &str, pick: &mut FloorPick, names: &[String]) {
        let label = |p: FloorPick| match p {
            FloorPick::Same => "Current Floor".to_string(),
            FloorPick::All => "All Floors".to_string(),
            FloorPick::Floor(i) => names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Floor {i}")),
        };
        egui::ComboBox::from_id_salt(salt)
            .selected_text(label(*pick))
            .show_ui(ui, |ui| {
                for p in [FloorPick::Same, FloorPick::All] {
                    ui.selectable_value(pick, p, label(p));
                }
                for (i, _) in names.iter().enumerate() {
                    ui.selectable_value(pick, FloorPick::Floor(i), label(FloorPick::Floor(i)));
                }
            });
    }

    /// The Build Framing command (manual pp. 914, 915): the Automatic Framing
    /// Defaults button, the Automatically Rebuild Framing choices and the
    /// Build Framing Once choices with their floor selectors.
    fn build_page(&mut self, ui: &mut Ui) {
        if ui.button("Automatic Framing Defaults\u{2026}").clicked() {
            self.enter_defaults();
        }
        section(ui, "Automatically Rebuild Framing");
        self.auto_row(ui, Group::Floor, "Floor", "Rebuilds the floor framing when the walls, rooms or layout lines it is made from change.");
        self.auto_row(
            ui,
            Group::Ceiling,
            "Ceiling",
            "Rebuilds the ceiling framing when its rooms or walls change.",
        );
        self.auto_row(
            ui,
            Group::Wall,
            "Wall",
            "Rebuilds the wall framing when the walls or their openings change.",
        );
        self.auto_row(
            ui,
            Group::Roof,
            "Roof",
            "Rebuilds the roof framing when the roof planes change.",
        );
        section(ui, "Build Framing Once");
        let names = self.draft.floor_names.clone();
        for (g, label, salt) in [
            (Group::Floor, "Floor", "bo_floor"),
            (Group::Ceiling, "Ceiling", "bo_ceiling"),
        ] {
            ui.horizontal(|ui| {
                let mut on_ = self.draft.build.build.get(g);
                ui.checkbox(&mut on_, label);
                self.draft.build.build.set(g, on_);
                ui.add_enabled_ui(on_, |ui| {
                    let pick = if g == Group::Floor {
                        &mut self.draft.build.floor_pick
                    } else {
                        &mut self.draft.build.ceiling_pick
                    };
                    Self::floor_pick(ui, salt, pick, &names);
                });
            });
        }
        for (g, label) in [(Group::Wall, "Wall"), (Group::Roof, "Roof")] {
            let mut on_ = self.draft.build.build.get(g);
            ui.checkbox(&mut on_, label);
            self.draft.build.build.set(g, on_);
        }
        let mut all = self.draft.build_on_ok == Some(true);
        if ui
            .checkbox(&mut all, "Build the walls and roof of every floor")
            .changed()
        {
            self.draft.build_on_ok = Some(all);
        }
        ui.weak("OK builds the checked parts once, as one undo step. A part that is not checked keeps the framing it has.");
        section(ui, "Retain existing framing");
        for g in Group::ALL {
            let mut keep = self.draft.build.retain.get(g);
            ui.checkbox(
                &mut keep,
                format!("Retain all existing {} framing", g.name().to_lowercase()),
            )
            .on_hover_text("A build leaves this part of the plan as it is, so edits to its members survive. Retain Framing on a wall, room, roof plane or tray ceiling protects just that object.");
            self.draft.build.retain.set(g, keep);
        }
        let (nw, np) = (
            self.draft.build.retain_walls.len(),
            self.draft.build.retain_planes.len(),
        );
        row(ui, "Retain Wall Framing", |ui| {
            ui.label(format!("{nw} wall(s)"));
            if ui
                .add_enabled(nw > 0, egui::Button::new("Clear"))
                .on_hover_text(
                    "Walls marked Retain Wall Framing keep their framing through a build.",
                )
                .clicked()
            {
                self.draft.build.retain_walls.clear();
            }
        });
        row(ui, "Retain Framing on roof planes", |ui| {
            ui.label(format!("{np} plane(s)"));
            if ui.add_enabled(np > 0, egui::Button::new("Clear")).clicked() {
                self.draft.build.retain_planes.clear();
            }
        });
        section(ui, "After a build");
        ui.checkbox(
            &mut self.draft.build.show_layers,
            "Turn the framing layers on",
        );
        ui.weak("Framing layers start off in a new plan, like Chief's. Leave this checked to see the result.");
    }
}

impl SpecPages for DefaultsForm {
    fn tabs(&self) -> &'static [Tab] {
        if self.build_mode && !self.sub {
            BUILD_TABS
        } else {
            DEFAULTS_TABS
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        let w = &self.draft.walls;
        if w.stud_spacing < 4.0
            || w.joist_spacing < 4.0
            || w.ceiling_joist_spacing < 4.0
            || self.draft.roof.spacing < 4.0
            || self.draft.build.trusses.spacing < 6.0
        {
            return Some("Spacings must be at least 4\" (trusses 6\")".into());
        }
        if w.header_table.is_empty() {
            return Some("The header table needs a row".into());
        }
        if w.wall_blocking && w.wall_blocking_spacing < 12.0 {
            return Some("Blocking rows must be at least 12\" apart".into());
        }
        let o = &self.draft.build.detail;
        if o.rim_width < 0.5 {
            return Some("The rim joist width must be at least 1/2\"".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let name = self.tabs()[tab].name;
        match name {
            "Build Framing" => self.build_page(ui),
            "Floor Levels" => self.floor(ui),
            "Wall" => self.walls(ui),
            "Openings" => self.headers(ui),
            "Roof" => self.roof(ui),
            "Trusses" => self.trusses(ui),
            "Posts" => self.posts(ui),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let w = &self.draft.walls;
        let lines = [
            format!(
                "Studs {} @ {}\" o.c.",
                w.stud_size.nominal_name(),
                fmt_short(w.stud_spacing)
            ),
            format!(
                "Header over 3': {}",
                w.header_lumber_for(36.0).nominal_name()
            ),
            format!(
                "Header over 6': {}",
                w.header_lumber_for(72.0).nominal_name()
            ),
            format!("Joists {}", w.joist_size.nominal_name()),
            format!(
                "Rafters {} @ {}\"",
                self.draft.roof.rafter.nominal_name(),
                fmt_short(self.draft.roof.spacing)
            ),
        ];
        for (i, text) in lines.iter().enumerate() {
            pv_text(
                painter,
                Pos2::new(rect.center().x, rect.min.y + 14.0 + 16.0 * i as f32),
                Align2::CENTER_CENTER,
                text,
                11.0,
            );
        }
    }
}

// ===================================================================
// The Framing Group question
// ===================================================================

thread_local! {
    /// A Build Framing for Selected Object(s) waiting for the answer to
    /// "create a new Framing Group?": the room and whether it came from the
    /// Parent Object(s) button.
    static GROUP_PROMPT: std::cell::Cell<Option<(usize, bool)>> = const { std::cell::Cell::new(None) };
}

/// Opens the Framing Group question for room `room` (manual p. 917): the room
/// stands beside rooms of its Framing Group.
pub fn ask_framing_group(room: usize, parent: bool) {
    GROUP_PROMPT.with(|c| c.set(Some((room, parent))));
}

/// Whether the Framing Group question is open.
pub fn framing_group_asked() -> bool {
    GROUP_PROMPT.with(|c| c.get().is_some())
}

/// Answers the question: Yes creates a new Framing Group for the room and
/// builds its platform; No builds across the whole platform with the groups
/// as they are. Returns the outcome of the build.
pub fn answer_framing_group(
    cx: &mut crate::editor::EditorContext,
    yes: bool,
) -> Option<crate::editor::framing_view::Outcome> {
    let (_, parent) = GROUP_PROMPT.with(std::cell::Cell::take)?;
    Some(if parent {
        crate::editor::framing_view::build_parents(cx, Some(yes))
    } else {
        crate::editor::framing_view::build_selected(cx, Some(yes))
    })
}

/// Cancels the question.
pub fn cancel_framing_group() {
    GROUP_PROMPT.with(std::cell::Cell::take);
}

/// Draws the Framing Group question when it is open. Called every frame by
/// the shell with the other dialogs.
pub fn host_frame(cx: &mut crate::editor::EditorContext, ctx: &egui::Context) {
    super::truss_detail::host_frame(cx, ctx);
    if !framing_group_asked() {
        return;
    }
    let mut answer: Option<Option<bool>> = None;
    egui::Window::new("Framing Group")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(
                "This room is next to rooms that are in the same Framing Group.\n\
                 Create a new Framing Group for it and frame it on its own?",
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    answer = Some(Some(true));
                }
                if ui.button("No").clicked() {
                    answer = Some(Some(false));
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(None);
                }
            });
        });
    match answer {
        Some(Some(yes)) => {
            answer_framing_group(cx, yes);
        }
        Some(None) => cancel_framing_group(),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    fn joist() -> FramingMember {
        FramingMember::new(
            7,
            ManualMemberKind::Joist,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
        )
    }

    fn layers() -> Vec<String> {
        vec!["Framing".into(), "Framing, Floor Joists".into()]
    }

    #[test]
    fn size_plies_length_and_kind_edits_update_the_draft() {
        let mut d = FramingMemberDialog::new(&joist(), layers());
        assert_eq!(d.id(), 7);
        d.set_lumber(LumberSize::TWO_BY_TWELVE);
        assert_eq!(d.draft().lumber, LumberSize::TWO_BY_TWELVE);
        assert!((d.draft().depth - 11.25).abs() < 1e-9);
        assert_eq!(d.draft().label, "2x12 joist");
        d.set_plies(2);
        assert!((d.draft().width - 3.0).abs() < 1e-9);
        d.set_length(96.0);
        assert!((d.draft().plan_length() - 96.0).abs() < 1e-9);
        assert!(d.draft().end.y.abs() < 1e-9, "stays on its line");
        d.set_kind(ManualMemberKind::RoofTruss);
        assert!(d.draft().truss.is_some());
        assert_eq!(d.draft().layer_name, "Framing, Trusses");
        assert_eq!(d.draft().truss.as_ref().unwrap().span, 96.0);
        d.set_plies(3);
        assert_eq!(d.draft().truss.as_ref().unwrap().plies, 3);
        d.set_layer("Framing");
        assert_eq!(d.draft().layer_name, "Framing");
        assert!((d.draft().width - 3.0 * 1.5).abs() < 1e-9);
    }

    #[test]
    fn truss_edits_change_the_preview_count_and_posts_use_height() {
        let truss = FramingMember::new(
            1,
            ManualMemberKind::RoofTruss,
            Point::ZERO,
            Point::new(288.0, 0.0),
        );
        let mut d = FramingMemberDialog::new(&truss, layers());
        let fink = d.truss_member_count();
        assert_eq!(fink, 9, "a 24' 6:12 Fink has 9 members");
        d.set_truss_type(TrussType::KingPost);
        assert!(d.truss_member_count() < fink);
        d.set_pitch(40.0);
        assert_eq!(d.draft().truss.as_ref().unwrap().pitch, MAX_PITCH);

        let post = FramingMember::new(2, ManualMemberKind::Post, Point::ZERO, Point::ZERO);
        let mut p = FramingMemberDialog::new(&post, layers());
        p.set_length(108.0);
        assert_eq!(p.draft().height, 108.0);
        assert_eq!(p.draft().end, p.draft().start);
        assert_eq!(p.truss_member_count(), 0);
    }

    #[test]
    fn the_dialog_draws_and_tabs_follow_the_kind() {
        let ctx = egui::Context::default();
        for m in [
            joist(),
            FramingMember::new(
                3,
                ManualMemberKind::GirderTruss,
                Point::ZERO,
                Point::new(240.0, 0.0),
            ),
            FramingMember::new(
                5,
                ManualMemberKind::FloorCeilingTruss,
                Point::ZERO,
                Point::new(240.0, 0.0),
            ),
            FramingMember::new(
                4,
                ManualMemberKind::PostWithFooting,
                Point::ZERO,
                Point::ZERO,
            ),
        ] {
            let mut d = FramingMemberDialog::new(&m, layers());
            // Joists take an End Profile; trusses their own General; posts neither.
            let want = if m.kind.is_truss() {
                5
            } else if takes_end_profile(m.kind) {
                6
            } else {
                5
            };
            assert_eq!(d.form.tabs().len(), want, "{:?}", m.kind);
            assert!(d.form.tabs().iter().any(|t| t.name == "Fill Style"));
            assert!(d.form.tabs().iter().any(|t| t.name == "Label"));
            for tab in 0..d.form.tabs().len() {
                let mut ui_form = FramingMemberDialog::new(&m, layers());
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| ui_form.form.page(ui, tab));
                });
            }
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    assert_eq!(d.show(ctx), Outcome::Open);
                });
            }
            assert!(d.form.error().is_none());
        }
    }

    #[test]
    fn the_new_member_options_are_in_the_draft() {
        let mut beam = FramingMember::new(
            8,
            ManualMemberKind::FloorCeilingBeam,
            Point::ZERO,
            Point::new(144.0, 0.0),
        );
        assert!(!beam.bearing_beam);
        beam.bearing_beam = true;
        beam.end_profile[1].shape = EndShape::Round;
        beam.end_profile[1].size = 3.0;
        let mut g = FramingMember::new(
            9,
            ManualMemberKind::GeneralFraming,
            Point::ZERO,
            Point::new(48.0, 0.0),
        );
        g.flat = FlatTo::Inside;
        g.custom_label = "%nominal_size% blk".into();
        assert_eq!(
            crate::editor::framing_view::label_text(&g),
            "2x4 blk",
            "the macro is the nominal size"
        );
        let ctx = egui::Context::default();
        for m in [beam, g] {
            let mut d = FramingMemberDialog::new(&m, layers());
            // Every panel draws, with the options on.
            for tab in 0..d.form.tabs().len() {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                });
            }
            assert_eq!(d.draft().flat, m.flat);
            assert_eq!(d.draft().bearing_beam, m.bearing_beam);
        }
    }

    #[test]
    fn a_truss_dialog_edits_the_spec_and_old_trusses_open() {
        let mut t = FramingMember::new(
            3,
            ManualMemberKind::RoofTruss,
            Point::ZERO,
            Point::new(288.0, 0.0),
        );
        let mut d = FramingMemberDialog::new(&t, layers());
        {
            let s = d.truss_mut().unwrap();
            s.locked = true;
            s.end_truss = true;
            s.max_span_bottom = 60.0;
        }
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, 0));
        });
        // The panel keeps the span the member's length and does not unlock.
        let s = d.draft().truss.as_ref().unwrap();
        assert!(s.locked && s.end_truss && (s.span - 288.0).abs() < 1e-9);
        assert!(!s.force_rebuild, "a locked truss cannot be forced");
        t.truss = Some(TrussSpec::new(TrussType::DoubleFink, 288.0, 8.0));
        assert!(FramingMemberDialog::new(&t, layers()).truss_member_count() > 15);
    }

    #[test]
    fn direction_lines_have_a_specification() {
        use crate::editor::framing_view::Record;
        let rec = Record::JoistDirection {
            id: 11,
            dir: plan_framing::JoistDirectionLine::new(
                (Point::new(0.0, 0.0), Point::new(0.0, 50.0)),
                16.0,
            ),
        };
        let mut d = FramingMemberDialog::for_direction(&rec).expect("a dialog");
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
        if let Some(DirectionForm::Joist(j)) = d.direction_mut() {
            j.depth = 11.25;
            j.spacing = 12.0;
        }
        let tr = Record::TrussDirection {
            id: 12,
            dir: plan_framing::RoofTrussDirection::new(
                (Point::new(0.0, 0.0), Point::new(0.0, 50.0)),
                24.0,
            ),
        };
        assert!(FramingMemberDialog::for_direction(&tr).is_some());
        let marker = Record::Marker {
            id: 13,
            marker: plan_framing::ReferenceMarker {
                point: Point::ZERO,
                angle: 0.0,
            },
        };
        assert!(FramingMemberDialog::for_direction(&marker).is_none());
    }

    fn form(draft: FramingSettings, build_mode: bool) -> DefaultsForm {
        DefaultsForm {
            draft,
            fields: Fields::default(),
            build_mode,
            sub: false,
            backup: None,
            reset_tab: false,
        }
    }

    #[test]
    fn the_framing_defaults_dialog_edits_a_copy_and_draws_every_page() {
        let stored = FramingSettings::default();
        let mut d = FramingDefaultsDialog::new(&stored);
        assert_eq!(d.draft(), &stored);
        d.draft_mut().walls.stud_spacing = 24.0;
        d.draft_mut().walls.header_table[2].lumber = plan_framing::TWO_BY_TWELVE;
        d.draft_mut().roof.overhang_cut = plan_framing::OverhangCut::Level;
        assert_eq!(
            stored.walls.stud_spacing, 16.0,
            "the stored settings are untouched"
        );
        assert_eq!(
            d.draft().walls.header_lumber_for(72.0).nominal_name(),
            "2x12"
        );
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(d.show(ctx), Outcome::Open);
            });
        }
        // The Automatic Framing Defaults panels, Chief's names.
        let names: Vec<&str> = DEFAULTS_TABS.iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "Floor Levels",
                "Wall",
                "Openings",
                "Roof",
                "Trusses",
                "Posts"
            ]
        );
        // Each page draws without panicking.
        for tab in 0..DEFAULTS_TABS.len() {
            let mut f = form(d.draft().clone(), false);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| f.page(ui, tab));
            });
            assert!(f.error().is_none());
        }
        // Bad values are refused.
        let mut bad = form(d.draft().clone(), false);
        bad.draft.walls.stud_spacing = 1.0;
        assert!(bad.error().is_some());
        bad.draft.walls.stud_spacing = 16.0;
        bad.draft.walls.header_table.clear();
        assert!(bad.error().is_some());
        let mut thin = form(d.draft().clone(), false);
        thin.draft.build.detail.rim_width = 0.1;
        assert!(thin.error().is_some());
    }

    #[test]
    fn the_build_framing_command_has_the_defaults_button_and_the_once_choices() {
        request_build(true);
        let mut stored = FramingSettings::default();
        stored.floor_names = vec!["1st Floor".into(), "2nd Floor".into()];
        let mut d = FramingDefaultsDialog::new(&stored);
        assert!(d.is_build());
        assert!(!d.in_defaults());
        assert_eq!(d.draft().build_on_ok, Some(true), "Build All Framing");
        let names: Vec<&str> = BUILD_TABS.iter().map(|t| t.name).collect();
        assert_eq!(names, ["Build Framing"]);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(d.show(ctx), Outcome::Open);
        });
        // The command page draws, with the floor selectors and both lists.
        let mut f = form(d.draft().clone(), true);
        f.draft.build.floor_pick = FloorPick::Floor(1);
        f.draft.build.ceiling_pick = FloorPick::All;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f.page(ui, 0));
        });
        assert!(f.error().is_none());
        // The Automatic Framing Defaults button leads to the panels, and
        // Cancel there puts the defaults back.
        d.open_automatic_defaults();
        assert!(d.in_defaults());
        d.draft_mut().walls.stud_spacing = 24.0;
        assert_eq!(d.form.tabs().len(), DEFAULTS_TABS.len());
        // The request is used up: the next window is the plain defaults page.
        let plain = FramingDefaultsDialog::new(&stored);
        assert!(!plain.is_build());
        assert_eq!(plain.draft().build_on_ok, None);
        // Bad truss spacing is refused.
        let mut bad = form(stored.clone(), true);
        bad.draft.build.trusses.spacing = 2.0;
        assert!(bad.error().is_some());
    }

    #[test]
    fn the_framing_group_question_is_answered_once() {
        use crate::editor::EditorContext;
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        assert!(!framing_group_asked());
        ask_framing_group(0, false);
        assert!(framing_group_asked());
        cancel_framing_group();
        assert!(!framing_group_asked());
        assert!(answer_framing_group(&mut cx, true).is_none());
        // The window draws while the question is open.
        ask_framing_group(0, true);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| host_frame(&mut cx, ctx));
        assert!(framing_group_asked());
        cancel_framing_group();
    }

    #[test]
    fn ok_in_the_build_dialog_saves_the_options_and_builds() {
        use crate::editor::{framing_view, EditorContext};
        use plan_core::WallKind;
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        cx.refresh();
        request_build(false);
        let mut d = FramingDefaultsDialog::new(&framing_view::settings(&cx.project));
        d.draft_mut().build.build.ceiling = true;
        d.draft_mut().build.auto_rebuild.wall = true;
        d.draft_mut().walls.ceiling_joist_spacing = 24.0;
        framing_view::set_settings(&mut cx, d.draft().clone());
        let st = framing_view::settings(&cx.project);
        assert!(st.build.build.ceiling && st.build.auto_rebuild.wall);
        assert_eq!(st.walls.ceiling_joist_spacing, 24.0);
        assert!(cx
            .framing
            .iter()
            .any(|m| m.kind == plan_framing::MemberKind::CeilingJoist));
        assert_eq!(cx.undo_label(), Some("Build Framing"));
    }

    #[test]
    fn spans_print_in_feet_and_inches() {
        assert_eq!(feet_inches(185.0), "15' 5\"");
        assert_eq!(feet_inches(96.0), "8' 0\"");
    }
}
