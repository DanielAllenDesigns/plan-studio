//! Electrical Service Specification (double-click a device; CB-62, CB-63).
//!
//! General: height above the floor, label, circuit. Options: the switches
//! that control the device. Layer and Label hold the controls the model has
//! no fields for yet, drawn disabled. The dialog edits a [`DeviceDraft`];
//! [`DeviceDraft::apply`] copies it onto the device.

use super::{
    dis_check, dis_combo, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_INK,
};
use crate::editor::{site_view, Camera};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Ui};
use plan_core::{Id, Point};
use plan_electrical::{Device, DeviceKind, ElectricalLayer};

const TABS: &[Tab] = &[on("General"), on("Options"), on("Layer"), on("Label")];

/// What the dialog edits of a device.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceDraft {
    pub id: Id,
    pub kind: DeviceKind,
    /// Height above the finished floor, inches.
    pub height: f64,
    pub label: String,
    pub circuit: Option<u32>,
    /// Switches that control the device.
    pub switched_by: Vec<Id>,
}

impl DeviceDraft {
    pub fn from_device(d: &Device) -> Self {
        Self {
            id: d.id,
            kind: d.kind,
            height: d.height,
            label: d.label.clone(),
            circuit: d.circuit,
            switched_by: d.switched_by.clone(),
        }
    }

    /// Copies the edited values onto `d`.
    pub fn apply(&self, d: &mut Device) {
        d.height = self.height;
        d.label = self.label.clone();
        d.circuit = self.circuit;
        d.switched_by = self.switched_by.clone();
    }
}

pub struct ElectricalDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: DeviceDraft,
    /// The switches of the floor that can control the device.
    switches: Vec<(Id, String)>,
    fields: Fields,
    circuit_text: String,
}

impl ElectricalDialog {
    pub fn for_device(d: &Device, layer: &ElectricalLayer) -> Self {
        let switches = layer
            .devices
            .iter()
            .filter(|s| s.kind.is_switch() && s.id != d.id)
            .map(|s| {
                let name = if s.label.is_empty() {
                    s.kind.name().to_string()
                } else {
                    format!("{} ({})", s.kind.name(), s.label)
                };
                (s.id, format!("{name} #{}", s.id))
            })
            .collect();
        Self {
            frame: SpecDialog::new("Electrical Service Specification", "electrical_service"),
            form: Form {
                draft: DeviceDraft::from_device(d),
                switches,
                fields: Fields::default(),
                circuit_text: d.circuit.map(|c| c.to_string()).unwrap_or_default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &DeviceDraft {
        &self.form.draft
    }

    #[cfg(test)]
    /// Test hook: the draft to edit before applying.
    pub fn draft_mut(&mut self) -> &mut DeviceDraft {
        &mut self.form.draft
    }
}

impl Form {
    fn circuit_valid(&self) -> bool {
        let t = self.circuit_text.trim();
        t.is_empty() || t.parse::<u32>().is_ok()
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Type", |ui| ui.label(self.draft.kind.name()));
        self.fields
            .length_row(ui, "Height", "height", &mut self.draft.height);
        row(ui, "Label", |ui| {
            ui.text_edit_singleline(&mut self.draft.label)
        });
        row(ui, "Circuit", |ui| {
            let edit = egui::TextEdit::singleline(&mut self.circuit_text).desired_width(60.0);
            if ui.add(edit).changed() {
                self.draft.circuit = self.circuit_text.trim().parse().ok();
            }
        });
    }

    fn options(&mut self, ui: &mut Ui) {
        section(ui, "Switched By");
        if self.draft.kind.is_switch() {
            ui.weak("A switch is not switched by another switch.");
            return;
        }
        if self.switches.is_empty() {
            ui.weak("There are no switches on this floor.");
        }
        for (id, name) in &self.switches {
            let mut on = self.draft.switched_by.contains(id);
            if ui.checkbox(&mut on, name).changed() {
                if on {
                    self.draft.switched_by.push(*id);
                } else {
                    self.draft.switched_by.retain(|s| s != id);
                }
            }
        }
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| dis_combo(ui, "elec_layer", "Electrical"));
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        dis_check(ui, "Show label in plan", !self.draft.label.is_empty());
        row(ui, "Text height", |ui| {
            ui.add_enabled(false, egui::Label::new("3\""))
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if !self.circuit_valid() {
            return Some("Circuit must be a number".into());
        }
        if self.draft.height < 0.0 {
            return Some("Height cannot be negative".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => self.general(ui),
            "Options" => self.options(ui),
            "Layer" => self.layer(ui),
            "Label" => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            self.draft.kind.name(),
            11.0,
        );
        let area = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + 16.0), rect.max);
        // Fit a 32" square, or the strip of a rope light.
        let (span, mid) = match self.draft.kind {
            DeviceKind::RopeLight { length } if length > 32.0 => {
                (length, Point::new(0.0, length * 0.5))
            }
            DeviceKind::RopeLight { length } => (32.0, Point::new(0.0, length * 0.5)),
            _ => (32.0, Point::ZERO),
        };
        let cam = Camera {
            center: mid,
            px_per_in: f64::from(area.width().min(area.height())) * 0.9 / span,
            rect: area,
        };
        site_view::draw_symbol(p, &cam, &self.draft.kind.symbol(), PV_INK, 1.5);
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 6.0),
            Align2::CENTER_CENTER,
            format!("Height {}", super::fmt_short(self.draft.height)),
            11.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_electrical::place_free;

    #[test]
    fn draft_round_trips_onto_the_device() {
        let mut layer = ElectricalLayer::default();
        let sw = layer.add(place_free(DeviceKind::Switch, Point::ZERO));
        let light = layer.add(place_free(DeviceKind::CeilingLight, Point::new(60.0, 0.0)));
        let d = layer.device(light).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        assert_eq!(dlg.form.switches.len(), 1);
        {
            let draft = dlg.draft_mut();
            draft.height = 90.0;
            draft.label = "Hall".into();
            draft.circuit = Some(4);
            draft.switched_by = vec![sw];
        }
        let mut edited = d.clone();
        dlg.draft().apply(&mut edited);
        assert_eq!(edited.height, 90.0);
        assert_eq!(edited.label, "Hall");
        assert_eq!(edited.circuit, Some(4));
        assert_eq!(edited.switched_by, vec![sw]);
        assert_eq!(edited.position, d.position);
    }

    #[test]
    fn dialog_draws_headlessly() {
        let mut layer = ElectricalLayer::default();
        let id = layer.add(place_free(
            DeviceKind::RopeLight { length: 96.0 },
            Point::ZERO,
        ));
        let d = layer.device(id).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(dlg.show(ctx), Outcome::Open);
            });
        }
    }
}
