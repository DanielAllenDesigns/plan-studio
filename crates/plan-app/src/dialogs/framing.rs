//! Framing Member Specification (double-click a member with a framing tool).
//!
//! * General: the member type, lumber size, material, plies, length (plan
//!   length, or height for a post), rise, bottom elevation, rotation (roll
//!   about the member's own axis) and the cut-list label; for a post with
//!   footing also the footing size and thickness.
//! * Truss (truss types only): truss type, pitch, heel height, overhang and
//!   the number of chords and webs the truss gets; the span is the length.
//! * Line Style: line style and weight follow the member's layer, so they
//!   are shown disabled.
//! * Layer: the layer the member is drawn on.
//!
//! The dialog edits a cloned [`FramingMember`]; the tool stores the draft on OK
//! (`framing_view::apply_edit`).

use super::{
    dis_combo, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_FAINT, PV_INK, PV_WALL,
};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::geometry::Point;
use plan_framing::{
    FramingMaterial, FramingMember, LumberSize, ManualMemberKind, Truss, TrussSpec, TrussType,
};

const TABS_PLAIN: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
const TABS_TRUSS: &[Tab] = &[on("General"), on("Truss"), on("Line Style"), on("Layer")];

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

pub const TRUSS_TYPES: [TrussType; 6] = [
    TrussType::Fink,
    TrussType::Howe,
    TrussType::KingPost,
    TrussType::Scissor,
    TrussType::Attic,
    TrussType::Mono,
];

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

pub struct FramingMemberDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: FramingMember,
    fields: Fields,
    /// Plan length (post: height) as edited.
    length: f64,
    layers: Vec<String>,
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
        };
        Self {
            frame: SpecDialog::new(
                "Framing Member Specification",
                ("framing_member", member.id),
            ),
            form,
        }
    }

    #[cfg(test)]
    pub fn id(&self) -> plan_core::Id {
        self.form.draft.id
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &FramingMember {
        &self.form.draft
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

    /// Chords and webs the truss gets for the current inputs (0 for other
    /// members).
    #[cfg(test)]
    pub fn truss_member_count(&self) -> usize {
        self.form.truss_member_count()
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
        row(ui, "Label", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.label).desired_width(200.0))
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

    fn truss(&mut self, ui: &mut Ui) {
        section(ui, "Truss");
        let span = self.draft.plan_length();
        let count = self.truss_member_count();
        let Some(spec) = self.draft.truss.as_mut() else {
            ui.weak("This member type is not a truss.");
            return;
        };
        row(ui, "Truss Type", |ui| {
            egui::ComboBox::from_id_salt("framing_truss_type")
                .selected_text(spec.kind.name())
                .show_ui(ui, |ui| {
                    for t in TRUSS_TYPES {
                        ui.selectable_value(&mut spec.kind, t, t.name());
                    }
                });
        });
        row(ui, "Span", |ui| ui.label(fmt_short(span)));
        row(ui, "Pitch (rise per 12)", |ui| {
            ui.add(
                egui::DragValue::new(&mut spec.pitch)
                    .range(0.0..=MAX_PITCH)
                    .speed(0.1)
                    .suffix(":12"),
            )
        });
        self.fields
            .length_row(ui, "Heel height", "heel", &mut spec.heel_height);
        self.fields
            .length_row(ui, "Overhang", "overhang", &mut spec.overhang);
        row(ui, "Plies", |ui| ui.label(spec.plies.to_string()));
        ui.add_space(6.0);
        ui.weak(format!("This truss gets {count} chords and webs."));
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
            egui::ComboBox::from_id_salt("framing_layer")
                .selected_text(self.draft.layer_name.clone())
                .show_ui(ui, |ui| {
                    for l in &self.layers {
                        ui.selectable_value(&mut self.draft.layer_name, l.clone(), l);
                    }
                });
        });
    }
}

fn default_label(m: &FramingMember) -> String {
    format!("{} {}", m.lumber.name(), m.kind.name())
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        if self.is_truss() {
            TABS_TRUSS
        } else {
            TABS_PLAIN
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
            "General" => self.general(ui),
            "Truss" => self.truss(ui),
            "Line Style" => self.line_style(ui),
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

#[cfg(test)]
mod tests {
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
                4,
                ManualMemberKind::PostWithFooting,
                Point::ZERO,
                Point::ZERO,
            ),
        ] {
            let mut d = FramingMemberDialog::new(&m, layers());
            assert_eq!(d.form.tabs().len(), if m.kind.is_truss() { 4 } else { 3 });
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    assert_eq!(d.show(ctx), Outcome::Open);
                });
            }
            assert!(d.form.error().is_none());
        }
    }
}
