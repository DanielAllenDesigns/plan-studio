//! Electrical Service Specification (double-click a device; CB-62, CB-63).
//!
//! * General: the kind (any kind of the same family, e.g. a duplex outlet
//!   becomes a GFCI), its voltage and flags (110V / 220V, GFCI, WP,
//!   Dedicated), the height above the floor (to its Center, Bottom or Top),
//!   its Width and Height with Retain Aspect Ratio (E-33), label and circuit;
//!   with the plan's defaults loaded ([`ElectricalDialog::with_defaults`])
//!   also "Use as default height" and the four Default Heights with the Use
//!   Default Heights switch.
//! * Options (E-32): the mounting (wall, floor, ceiling or cabinet side), the
//!   recess (Distance from Wall, negative sets the device into the wall; Cuts
//!   Floor, Ceiling and Wall; Cut Depth; Insert Depth) and, for a switch,
//!   Automatically Change Switch Type When Wiring.
//! * Switches: for a switch, the lights and outlets it controls (check one to
//!   connect it, uncheck to remove the connection arc); for anything else,
//!   the switches that control it.
//! * Materials: the plate or fixture finish (drawn on the 3D plate).
//! * Label: show the label in the plan.
//! * Layer: the layer, drawn disabled.
//!
//! The dialog edits a [`DeviceDraft`]; [`DeviceDraft::apply`] copies it onto
//! the device and [`DeviceDraft::apply_to_layer`] also adds and removes the
//! connections of a switch.

use super::{
    dis_combo, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_INK,
};
use crate::editor::{site_view, Camera};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Ui};
use plan_core::{Id, Point, Wall};
use plan_electrical::{
    connect_in, disconnect, Device, DeviceKind, DeviceOptions, ElectricalDefaults, ElectricalLayer,
    HeightTo, Mount, FINISHES,
};

const TABS: &[Tab] = &[
    on("General"),
    on("Options"),
    on("Switches"),
    on("Materials"),
    on("Label"),
    on("Layer"),
];

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
    /// For a switch: the lights and outlets it is connected to.
    pub controls: Vec<Id>,
    /// Plate or fixture finish; empty is the default white.
    pub finish: String,
    /// Hide the label in the plan.
    pub hide_label: bool,
    /// Mounting, recess, size and switch options (stored beside the device).
    pub options: DeviceOptions,
    /// The plan's default heights as edited in the dialog; `None` when the
    /// dialog was opened without them (nothing to store then).
    pub defaults: Option<ElectricalDefaults>,
}

impl DeviceDraft {
    /// The draft of `d`; its connections come from `layer`.
    pub fn from_device(d: &Device, layer: &ElectricalLayer) -> Self {
        let controls = layer
            .connections
            .iter()
            .filter(|c| c.from == d.id)
            .filter(|c| layer.device(c.to).is_some_and(|t| !t.kind.is_switch()))
            .map(|c| c.to)
            .collect();
        Self {
            id: d.id,
            kind: d.kind,
            height: d.height,
            label: d.label.clone(),
            circuit: d.circuit,
            switched_by: d.switched_by.clone(),
            controls,
            finish: d.finish.clone(),
            hide_label: d.hide_label,
            options: layer.options_of(d.id),
            defaults: None,
        }
    }

    /// Stores the edited default heights in `project` (nothing changes when
    /// the dialog had none). Returns whether they differ from what was there.
    pub fn store_defaults(&self, project: &mut plan_core::Project) -> bool {
        let Some(defaults) = &self.defaults else {
            return false;
        };
        if *defaults == ElectricalDefaults::load(project) {
            return false;
        }
        defaults.store(project);
        true
    }

    /// Copies the edited values onto `d`.
    pub fn apply(&self, d: &mut Device) {
        d.kind = self.kind;
        d.height = self.height;
        d.label = self.label.clone();
        d.circuit = self.circuit;
        d.switched_by = self.switched_by.clone();
        d.finish = self.finish.clone();
        d.hide_label = self.hide_label;
    }

    /// [`apply`](Self::apply) onto the device in `layer`, then adds the
    /// connection arcs the Switches tab checked and removes the ones it
    /// unchecked (`walls` steer the new arcs away from the nearest wall).
    pub fn apply_to_layer(&self, layer: &mut ElectricalLayer, walls: &[Wall]) {
        let Some(d) = layer.device_mut(self.id) else {
            return;
        };
        let was_switch = d.kind.is_switch();
        self.apply(d);
        layer.set_options(self.id, self.options.clone());
        if !was_switch || !self.kind.is_switch() {
            return;
        }
        let current: Vec<Id> = layer
            .connections
            .iter()
            .filter(|c| c.from == self.id)
            .filter(|c| layer.device(c.to).is_some_and(|t| !t.kind.is_switch()))
            .map(|c| c.to)
            .collect();
        for gone in current.iter().filter(|t| !self.controls.contains(t)) {
            disconnect(layer, self.id, *gone);
        }
        for add in self.controls.iter().filter(|t| !current.contains(t)) {
            connect_in(layer, self.id, *add, walls);
        }
    }
}

pub struct ElectricalDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: DeviceDraft,
    /// The kinds the device can become.
    family: Vec<DeviceKind>,
    /// The switches of the floor that can control the device.
    switches: Vec<(Id, String)>,
    /// The lights and outlets of the floor a switch can be connected to.
    loads: Vec<(Id, String)>,
    fields: Fields,
    circuit_text: String,
    /// "Use as default height" is ticked.
    make_default: bool,
}

fn device_name(d: &Device) -> String {
    let name = if d.label.is_empty() {
        d.kind.name().to_string()
    } else {
        format!("{} ({})", d.kind.name(), d.label)
    };
    format!("{name} #{}", d.id)
}

impl ElectricalDialog {
    pub fn for_device(d: &Device, layer: &ElectricalLayer) -> Self {
        let switches = layer
            .devices
            .iter()
            .filter(|s| s.kind.is_switch() && s.id != d.id)
            .map(|s| (s.id, device_name(s)))
            .collect();
        let loads = layer
            .devices
            .iter()
            .filter(|l| {
                l.id != d.id
                    && !l.kind.is_switch()
                    && (l.kind.is_light()
                        || l.kind.is_outlet()
                        || matches!(l.kind, DeviceKind::CeilingFan))
            })
            .map(|l| (l.id, device_name(l)))
            .collect();
        Self {
            frame: SpecDialog::new("Electrical Service Specification", "electrical_service"),
            form: Form {
                draft: DeviceDraft::from_device(d, layer),
                family: d.kind.family(),
                switches,
                loads,
                fields: Fields::default(),
                circuit_text: d.circuit.map(|c| c.to_string()).unwrap_or_default(),
                make_default: false,
            },
        }
    }

    /// Opens the dialog with the plan's electrical defaults, so the General
    /// tab offers "Use as default height" and the four Default Heights.
    pub fn with_defaults(mut self, defaults: &ElectricalDefaults) -> Self {
        self.form.draft.defaults = Some(defaults.clone());
        self
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
    /// Folds the "Use as default height" tick into the draft's defaults (the
    /// Default Heights fields edit them in place).
    fn sync_defaults(&mut self) {
        let Some(defaults) = self.draft.defaults.as_mut() else {
            return;
        };
        if self.make_default {
            defaults.set_height(self.draft.kind, self.draft.height);
        }
    }

    fn circuit_valid(&self) -> bool {
        let t = self.circuit_text.trim();
        t.is_empty() || t.parse::<u32>().is_ok()
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Type", |ui| {
            let before = self.draft.kind;
            egui::ComboBox::from_id_salt("elec_kind")
                .selected_text(self.draft.kind.name())
                .show_ui(ui, |ui| {
                    for k in &self.family {
                        ui.selectable_value(&mut self.draft.kind, *k, k.name());
                    }
                });
            if self.draft.kind != before
                && (self.draft.height - before.default_height()).abs() < 1e-9
            {
                // A height nobody changed follows the new kind's default.
                self.draft.height = self.draft.kind.default_height();
            }
        });
        row(ui, "Voltage", |ui| {
            ui.label(match self.draft.kind.voltage() {
                Some(v) => format!("{v}V"),
                None => "-".to_string(),
            })
        });
        let flags: Vec<&str> = self
            .draft
            .kind
            .flags()
            .into_iter()
            .filter(|f| !f.ends_with('V'))
            .collect();
        row(ui, "Flags", |ui| {
            ui.label(if flags.is_empty() {
                "-".to_string()
            } else {
                flags.join(", ")
            })
        });
        // The height above the floor, measured to the center, bottom or top.
        row(ui, "Height to", |ui| {
            egui::ComboBox::from_id_salt("elec_height_to")
                .selected_text(self.draft.options.height_to.name())
                .show_ui(ui, |ui| {
                    for h in HeightTo::ALL {
                        ui.selectable_value(&mut self.draft.options.height_to, h, h.name());
                    }
                });
        });
        let kind = self.draft.kind;
        let to = self.draft.options.height_to;
        let mut shown = self
            .draft
            .options
            .height_measured(kind, self.draft.height, to);
        if self.fields.length_row(ui, "Height", "height", &mut shown) {
            self.draft.height = self.draft.options.height_to_center(kind, shown, to);
        }
        super::elevation_ref::row(ui, "Height Reference");
        section(ui, "Size");
        let (mut w, mut h) = self.draft.options.size(kind);
        if self.fields.length_row(ui, "Width", "width", &mut w) {
            self.draft.options.set_width(kind, w);
        }
        if self
            .fields
            .length_row(ui, "Size Height", "size_height", &mut h)
        {
            self.draft.options.set_size_height(kind, h);
        }
        ui.checkbox(&mut self.draft.options.retain_aspect, "Retain Aspect Ratio");
        if self.draft.defaults.is_some() {
            if let Some(g) = kind.height_group() {
                let group = match g {
                    plan_electrical::HeightGroup::Outlet => "Outlet",
                    plan_electrical::HeightGroup::Switch => "Switch",
                };
                let name = format!("Use as default {group} height");
                ui.checkbox(&mut self.make_default, name);
            }
            if let Some(d) = self.draft.defaults.as_mut() {
                ui.collapsing("Default Heights", |ui| {
                    ui.checkbox(&mut d.use_default_heights, "Use Default Heights");
                    self.fields
                        .length_row(ui, "Outlet", "def_outlet", &mut d.outlet_height);
                    self.fields
                        .length_row(ui, "Switch", "def_switch", &mut d.switch_height);
                    self.fields.length_row(
                        ui,
                        "Above Base Cabinet",
                        "def_above_base",
                        &mut d.above_base_cabinet,
                    );
                    self.fields.length_row(
                        ui,
                        "On Cabinet Side",
                        "def_cab_side",
                        &mut d.on_cabinet_side,
                    );
                });
            }
        }
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

    fn options_tab(&mut self, ui: &mut Ui) {
        section(ui, "Mounting");
        let o = &mut self.draft.options;
        row(ui, "Mounting", |ui| {
            egui::ComboBox::from_id_salt("elec_mount")
                .selected_text(o.mount.name())
                .show_ui(ui, |ui| {
                    for m in Mount::ALL {
                        ui.selectable_value(&mut o.mount, m, m.name());
                    }
                });
        });
        section(ui, "Recess");
        self.fields.length_row(
            ui,
            "Distance from Wall",
            "recess_distance",
            &mut o.recess.distance_from_wall,
        );
        ui.weak("A negative distance sets the device into the wall.");
        ui.checkbox(&mut o.recess.cuts_floor, "Cuts Floor");
        ui.checkbox(&mut o.recess.cuts_ceiling, "Cuts Ceiling");
        ui.checkbox(&mut o.recess.cuts_wall, "Cuts Wall");
        self.fields
            .length_row(ui, "Cut Depth", "recess_cut", &mut o.recess.cut_depth);
        self.fields.length_row(
            ui,
            "Insert Depth",
            "recess_insert",
            &mut o.recess.insert_depth,
        );
        if self.draft.kind.is_switch() {
            section(ui, "Wiring");
            ui.checkbox(
                &mut self.draft.options.auto_switch_type,
                "Automatically Change Switch Type When Wiring",
            );
        }
    }

    fn switches_tab(&mut self, ui: &mut Ui) {
        if self.draft.kind.is_switch() {
            section(ui, "Connected Lights and Outlets");
            if self.loads.is_empty() {
                ui.weak("There are no lights or outlets on this floor.");
            }
            for (id, name) in &self.loads {
                let mut on = self.draft.controls.contains(id);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        self.draft.controls.push(*id);
                    } else {
                        self.draft.controls.retain(|c| c != id);
                    }
                }
            }
            ui.add_space(4.0);
            ui.weak("A checked light is joined to this switch by a dashed arc in the plan.");
            return;
        }
        section(ui, "Switched By");
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

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Finish");
        row(ui, "Plate / fixture", |ui| {
            let shown = if self.draft.finish.is_empty() {
                FINISHES[0]
            } else {
                self.draft.finish.as_str()
            };
            egui::ComboBox::from_id_salt("elec_finish")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for f in FINISHES {
                        let current = self.draft.finish == f
                            || (self.draft.finish.is_empty() && f == FINISHES[0]);
                        if ui.selectable_label(current, f).clicked() {
                            self.draft.finish = if f == FINISHES[0] {
                                String::new()
                            } else {
                                f.to_string()
                            };
                        }
                    }
                });
        });
        ui.add_space(4.0);
        ui.weak("The finish colors the plate or fixture in the 3D view.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| dis_combo(ui, "elec_layer", "Electrical"));
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        let mut show = !self.draft.hide_label;
        if ui.checkbox(&mut show, "Show label in plan").changed() {
            self.draft.hide_label = !show;
        }
        row(ui, "Label", |ui| {
            ui.text_edit_singleline(&mut self.draft.label)
        });
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
            "Options" => self.options_tab(ui),
            "Switches" => self.switches_tab(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
        self.sync_defaults();
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
            rotation: 0.0,
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
    fn the_switches_tab_connects_and_disconnects_lights() {
        let mut layer = ElectricalLayer::default();
        let sw = layer.add(place_free(DeviceKind::Switch, Point::ZERO));
        let l1 = layer.add(place_free(DeviceKind::CeilingLight, Point::new(60.0, 0.0)));
        let l2 = layer.add(place_free(DeviceKind::RecessedCan, Point::new(60.0, 60.0)));
        let d = layer.device(sw).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        assert_eq!(dlg.form.loads.len(), 2);
        assert!(dlg.draft().controls.is_empty());
        dlg.draft_mut().controls = vec![l1, l2];
        dlg.draft_mut().kind = DeviceKind::SwitchDimmer;
        dlg.draft_mut().finish = "Ivory".into();
        let draft = dlg.draft().clone();
        draft.apply_to_layer(&mut layer, &[]);
        assert_eq!(layer.connections.len(), 2);
        assert_eq!(layer.device(sw).unwrap().kind, DeviceKind::SwitchDimmer);
        assert_eq!(layer.device(sw).unwrap().finish, "Ivory");
        assert_eq!(layer.device(l1).unwrap().switched_by, vec![sw]);
        // Reopen: both are checked; uncheck one.
        let d = layer.device(sw).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        assert_eq!(dlg.draft().controls.len(), 2);
        dlg.draft_mut().controls.retain(|c| *c != l2);
        let draft = dlg.draft().clone();
        draft.apply_to_layer(&mut layer, &[]);
        assert_eq!(layer.connections.len(), 1);
        assert!(layer.device(l2).unwrap().switched_by.is_empty());
        assert_eq!(layer.device(l1).unwrap().switched_by, vec![sw]);
    }

    #[test]
    fn the_kind_can_change_within_its_family() {
        let mut layer = ElectricalLayer::default();
        let o = layer.add(place_free(DeviceKind::Outlet110, Point::ZERO));
        let d = layer.device(o).unwrap().clone();
        let dlg = ElectricalDialog::for_device(&d, &layer);
        assert!(dlg.form.family.contains(&DeviceKind::Gfci));
        assert!(!dlg.form.family.contains(&DeviceKind::Switch));
        let mut draft = dlg.draft().clone();
        draft.kind = DeviceKind::Gfci;
        draft.hide_label = true;
        draft.apply_to_layer(&mut layer, &[]);
        let d = layer.device(o).unwrap();
        assert_eq!(d.kind, DeviceKind::Gfci);
        assert!(d.hide_label);
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

    #[test]
    fn the_four_default_heights_follow_the_ticked_group_and_the_edited_fields() {
        let mut layer = ElectricalLayer::default();
        let o = layer.add(place_free(DeviceKind::Outlet110, Point::ZERO));
        let d = layer.device(o).unwrap().clone();
        let plain = ElectricalDialog::for_device(&d, &layer);
        assert!(plain.draft().defaults.is_none(), "no defaults, no section");
        let mut dlg =
            ElectricalDialog::for_device(&d, &layer).with_defaults(&ElectricalDefaults::default());
        // Tick "Use as default Outlet height" with a new height.
        dlg.form.make_default = true;
        dlg.draft_mut().height = 18.0;
        // The Default Heights fields edit the draft's defaults in place.
        {
            let defaults = dlg.draft_mut().defaults.as_mut().unwrap();
            defaults.switch_height = 40.0;
            defaults.above_base_cabinet = 6.0;
            defaults.on_cabinet_side = 30.0;
            defaults.use_default_heights = true;
        }
        dlg.form.sync_defaults();
        let defaults = dlg.draft().defaults.clone().unwrap();
        assert_eq!(defaults.height(DeviceKind::Outlet110), 18.0);
        assert_eq!(defaults.height(DeviceKind::Gfci), 18.0, "one Outlet height");
        assert_eq!(defaults.height(DeviceKind::Switch), 40.0);
        assert_eq!(defaults.counter_height(), 42.0);
        assert_eq!(defaults.on_cabinet_side, 30.0);
        let mut project = plan_core::Project::new("x");
        assert!(dlg.draft().store_defaults(&mut project));
        assert!(
            !dlg.draft().store_defaults(&mut project),
            "unchanged the second time"
        );
        assert_eq!(ElectricalDefaults::load(&project), defaults);
        // Unticking leaves the typed field as the draft's own value.
        dlg.form.make_default = false;
        dlg.draft_mut()
            .defaults
            .as_mut()
            .unwrap()
            .set_height(DeviceKind::Outlet110, 12.0);
        dlg.form.sync_defaults();
        assert_eq!(
            dlg.draft()
                .defaults
                .as_ref()
                .unwrap()
                .height(DeviceKind::Outlet110),
            12.0
        );
    }

    #[test]
    fn options_recess_size_and_switch_type_are_stored_beside_the_device() {
        let mut layer = ElectricalLayer::default();
        let o = layer.add(place_free(DeviceKind::Outlet110, Point::ZERO));
        let sw = layer.add(place_free(DeviceKind::Switch, Point::new(20.0, 0.0)));
        let d = layer.device(o).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        assert!(dlg.draft().options.is_default());
        {
            let opts = &mut dlg.draft_mut().options;
            opts.mount = Mount::CabinetSide;
            opts.recess.distance_from_wall = -1.5;
            opts.recess.cuts_wall = true;
            opts.recess.cut_depth = 2.0;
            opts.recess.insert_depth = 1.5;
            // Retain Aspect Ratio: the height follows the width.
            opts.set_width(DeviceKind::Outlet110, 5.5);
        }
        let draft = dlg.draft().clone();
        draft.apply_to_layer(&mut layer, &[]);
        let stored = layer.options_of(o);
        assert_eq!(stored.mount, Mount::CabinetSide);
        assert!(stored.recess.is_recessed());
        assert_eq!(stored.size(DeviceKind::Outlet110), (5.5, 9.0));
        // A device with default options stores nothing.
        assert!(!layer.options.contains_key(&sw));
        // Reopen: the dialog shows what was stored; the options survive a save.
        let again = ElectricalDialog::for_device(layer.device(o).unwrap(), &layer);
        assert_eq!(again.draft().options, stored);
        let json = serde_json::to_string(&layer).unwrap();
        let back: ElectricalLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(back.options_of(o), stored);
        // Switch wiring type can be switched off per device.
        let d = layer.device(sw).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        dlg.draft_mut().options.auto_switch_type = false;
        dlg.draft().clone().apply_to_layer(&mut layer, &[]);
        assert!(!layer.options_of(sw).auto_switch_type);
    }

    #[test]
    fn the_height_can_be_read_to_the_bottom_or_the_top() {
        let mut opts = DeviceOptions::default();
        let k = DeviceKind::Outlet110;
        assert_eq!(opts.height_measured(k, 12.0, HeightTo::Center), 12.0);
        assert_eq!(opts.height_measured(k, 12.0, HeightTo::Bottom), 9.75);
        assert_eq!(opts.height_measured(k, 12.0, HeightTo::Top), 14.25);
        assert_eq!(opts.height_to_center(k, 9.75, HeightTo::Bottom), 12.0);
        opts.set_size_height(k, 9.0);
        assert_eq!(opts.width, Some(5.5), "the width follows the height");
    }

    #[test]
    fn voltage_and_flags_follow_the_type() {
        let mut layer = ElectricalLayer::default();
        let o = layer.add(place_free(DeviceKind::Outlet110, Point::ZERO));
        let d = layer.device(o).unwrap().clone();
        let mut dlg = ElectricalDialog::for_device(&d, &layer);
        assert!(dlg.form.family.contains(&DeviceKind::OutletWp));
        dlg.draft_mut().kind = DeviceKind::OutletWp;
        assert_eq!(dlg.draft().kind.voltage(), Some(110));
        assert_eq!(dlg.draft().kind.flags(), ["110V", "GFCI", "WP"]);
        let mut draft = dlg.draft().clone();
        draft.apply_to_layer(&mut layer, &[]);
        assert_eq!(layer.device(o).unwrap().kind, DeviceKind::OutletWp);
        draft.kind = DeviceKind::Outlet220;
        assert_eq!(draft.kind.flags(), ["220V", "Dedicated"]);
    }
}
