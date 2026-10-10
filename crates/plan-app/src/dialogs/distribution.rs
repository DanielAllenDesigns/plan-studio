//! Distribution Region and Path Specification (reference manual
//! "Distributed Objects", pp. 1091 to 1098; parity rows CB-468..CB-474): the
//! options of a distribution record. Tabs: General (Show Objects, Show
//! Region or Path, Auto Spacing, spacing), Layout (region style, offsets
//! from the polyline, path count or distance), Angle and Scale (absolute,
//! relative or random angle; scaling range) and Path Positioning (side to
//! side). The options are `plan_core::distribution::DistOptions`.
//!
//! OK stores the options on the record, rebuilds the copies and is one undo
//! step.

use super::{row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::distribution::{
    auto_spacing, AngleMode, DistOptions, OffsetFrom, RegionStyle, SideMode,
};
use plan_core::images::DistKind;
use plan_core::Id;
use std::cell::RefCell;

const REGION_TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Layout",
        enabled: true,
    },
    Tab {
        name: "Angle and Scale",
        enabled: true,
    },
    Tab {
        name: "Path Positioning",
        enabled: false,
    },
];

const PATH_TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Layout",
        enabled: true,
    },
    Tab {
        name: "Angle and Scale",
        enabled: true,
    },
    Tab {
        name: "Path Positioning",
        enabled: true,
    },
];

pub struct DistributionDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    record: Id,
    kind: DistKind,
    item: String,
    item_size: [f64; 3],
    spacing: f64,
    options: DistOptions,
    copies: usize,
    fields: Fields,
}

impl DistributionDialog {
    /// The window for distribution record `id`, or `None` when it is gone.
    pub fn new(cx: &EditorContext, id: Id) -> Option<Self> {
        let rec = cx.floor().symbol(id)?;
        let d = rec.distribution.as_ref()?;
        let options = d
            .options
            .clone()
            .unwrap_or_else(|| DistOptions::from_legacy(d));
        let title = match d.kind {
            DistKind::Region => "Distribution Region Specification",
            DistKind::Path => "Distribution Path Specification",
        };
        Some(Self {
            frame: SpecDialog::new(title, ("distribution_spec", title)),
            form: Form {
                record: id,
                kind: d.kind,
                item: d.item.clone(),
                item_size: d.item_size,
                spacing: d.spacing,
                options,
                copies: cx.project.distribution_copies(cx.floor, id),
                fields: Fields::default(),
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    #[cfg(test)]
    pub fn options_mut(&mut self) -> &mut DistOptions {
        &mut self.form.options
    }

    #[cfg(test)]
    pub fn set_spacing(&mut self, spacing: f64) {
        self.form.spacing = spacing;
    }

    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        apply(cx, self.form.record, &self.form.options, self.form.spacing)
    }
}

/// Stores `options` and `spacing` on distribution record `id` and rebuilds
/// its copies: one undo step, nothing when neither changed.
pub fn apply(cx: &mut EditorContext, id: Id, options: &DistOptions, spacing: f64) -> bool {
    let fl = cx.floor;
    let Some(d) = cx
        .floor()
        .symbol(id)
        .and_then(|s| s.distribution.as_ref())
        .cloned()
    else {
        return false;
    };
    let legacy = DistOptions::from_legacy(&d);
    let unchanged_options = match &d.options {
        Some(o) => o == options,
        None => *options == legacy,
    };
    if unchanged_options && (d.spacing - spacing).abs() < 1e-9 {
        return false;
    }
    cx.begin_change("Distribution Specification");
    if let Some(rec) = cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == id)
    {
        if let Some(dist) = rec.distribution.as_mut() {
            dist.spacing = spacing;
            dist.options = Some(options.clone());
        }
    }
    cx.project.rebuild_distribution(fl, id);
    cx.mark_dirty();
    true
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        match self.kind {
            DistKind::Region => REGION_TABS,
            DistKind::Path => PATH_TABS,
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if !self.options.auto_spacing && self.spacing <= 0.0 {
            return Some("The spacing must be greater than zero".into());
        }
        if self.options.scaling
            && (self.options.scale_min <= 0.0 || self.options.scale_max < self.options.scale_min)
        {
            return Some("The scaling range must run from a smaller to a larger percent".into());
        }
        if self.kind == DistKind::Path && self.options.by_count && self.options.count < 1 {
            return Some("Distribute at least one object".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => self.general(ui),
            1 => self.layout(ui),
            2 => self.angle_scale(ui),
            _ => self.positioning(ui),
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = egui::Color32::from_rgb(0x2B, 0x2B, 0x2B);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}\n{} objects", self.item, self.copies),
            egui::FontId::proportional(13.0),
            ink,
        );
    }
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        let region = self.kind == DistKind::Region;
        let o = &mut self.options;
        section(ui, "Display");
        ui.checkbox(&mut o.show_objects, "Show Objects");
        ui.checkbox(
            &mut o.show_outline,
            if region { "Show Region" } else { "Show Path" },
        );
        ui.add_space(4.0);
        section(ui, "Spacing");
        ui.checkbox(&mut o.auto_spacing, "Auto Spacing")
            .on_hover_text("10 percent more than the larger side of the object");
        if o.auto_spacing {
            self.spacing = auto_spacing(self.item_size);
        }
        ui.add_enabled_ui(!o.auto_spacing, |ui| {
            self.fields.length_row(
                ui,
                if region {
                    "Spacing"
                } else {
                    "Minimum distance"
                },
                "spacing",
                &mut self.spacing,
            );
        });
        if region {
            self.fields
                .length_row(ui, "Random offset (up to)", "max_offset", &mut o.max_offset);
        }
    }

    fn layout(&mut self, ui: &mut Ui) {
        let region = self.kind == DistKind::Region;
        let o = &mut self.options;
        if region {
            section(ui, "Distribution Style");
            for s in RegionStyle::ALL {
                ui.radio_value(&mut o.region_style, s, s.name());
            }
            ui.add_space(4.0);
            section(ui, "Offset");
            self.fields
                .length_row(ui, "X offset", "off_x", &mut o.offset_x);
            self.fields
                .length_row(ui, "Y offset", "off_y", &mut o.offset_y);
            row(ui, "Measured from", |ui| {
                ui.radio_value(
                    &mut o.offset_from,
                    OffsetFrom::PolylineStart,
                    "Polyline start",
                );
                ui.radio_value(
                    &mut o.offset_from,
                    OffsetFrom::PolylineCenter,
                    "Polyline center",
                );
            });
        } else {
            section(ui, "Number of Objects");
            ui.radio_value(&mut o.by_count, false, "By distance");
            ui.radio_value(&mut o.by_count, true, "Evenly, a number of objects");
            ui.add_enabled_ui(o.by_count, |ui| {
                row(ui, "Objects", |ui| {
                    ui.add(egui::DragValue::new(&mut o.count).range(1..=500))
                });
            });
            ui.add_enabled_ui(!o.by_count, |ui| {
                self.fields
                    .length_row(ui, "Maximum distance", "max_distance", &mut o.max_distance);
            });
            ui.add_space(4.0);
            section(ui, "Start");
            ui.checkbox(&mut o.center_objects, "Center Objects");
            ui.add_enabled_ui(!o.center_objects, |ui| {
                self.fields
                    .length_row(ui, "Start offset", "start_offset", &mut o.start_offset);
            });
        }
    }

    fn angle_scale(&mut self, ui: &mut Ui) {
        let o = &mut self.options;
        section(ui, "Angle");
        for m in AngleMode::ALL {
            ui.radio_value(&mut o.angle_mode, m, m.name());
        }
        ui.add_enabled_ui(o.angle_mode != AngleMode::Random, |ui| {
            row(ui, "Angle", |ui| {
                self.fields.degrees(ui, "deg_angle", &mut o.angle)
            });
        });
        ui.add_space(4.0);
        section(ui, "Scaling");
        ui.checkbox(&mut o.scaling, "Scale objects at random");
        ui.add_enabled_ui(o.scaling, |ui| {
            row(ui, "Smallest (percent)", |ui| {
                ui.add(
                    egui::DragValue::new(&mut o.scale_min)
                        .range(1.0..=1000.0)
                        .suffix(" %"),
                )
            });
            row(ui, "Largest (percent)", |ui| {
                ui.add(
                    egui::DragValue::new(&mut o.scale_max)
                        .range(1.0..=1000.0)
                        .suffix(" %"),
                )
            });
        });
    }

    fn positioning(&mut self, ui: &mut Ui) {
        let o = &mut self.options;
        section(ui, "Side to Side");
        for m in SideMode::ALL {
            ui.radio_value(&mut o.side_mode, m, m.name());
        }
        self.fields
            .length_row(ui, "Closest to the path", "side_min", &mut o.side_min);
        self.fields
            .length_row(ui, "Farthest from the path", "side_max", &mut o.side_max);
    }
}

thread_local! {
    static DIALOG: RefCell<Option<DistributionDialog>> = const { RefCell::new(None) };
}

/// Opens the window for record `id`.
pub fn open(cx: &EditorContext, id: Id) {
    if let Some(d) = DistributionDialog::new(cx, id) {
        DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

#[cfg(test)]
pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

#[cfg(test)]
pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// Draws the window when open and applies an OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => DIALOG.with(|slot| *slot.borrow_mut() = Some(d)),
        Outcome::Ok => {
            d.apply(cx);
        }
        Outcome::Cancel => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::images::Distribution;

    fn cx_with(kind: DistKind) -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let pts = match kind {
            DistKind::Region => vec![
                Point::new(0.0, 0.0),
                Point::new(120.0, 0.0),
                Point::new(120.0, 120.0),
                Point::new(0.0, 120.0),
            ],
            DistKind::Path => vec![Point::new(0.0, 0.0), Point::new(240.0, 0.0)],
        };
        let mut d = Distribution::new(kind, false, pts, "Shrub", [12.0, 12.0, 24.0]);
        d.spacing = 30.0;
        let id = cx.project.add_distribution(0, d);
        (cx, id)
    }

    #[test]
    fn an_unchanged_dialog_stores_nothing() {
        let (mut cx, id) = cx_with(DistKind::Region);
        let d = DistributionDialog::new(&cx, id).unwrap();
        assert!(!d.apply(&mut cx));
        assert!(cx.undo_label().is_none());
    }

    #[test]
    fn region_options_change_the_copies_in_one_undo_step() {
        let (mut cx, id) = cx_with(DistKind::Region);
        let before = cx.project.distribution_copies(0, id);
        let mut d = DistributionDialog::new(&cx, id).unwrap();
        d.options_mut().region_style = RegionStyle::EvenlyScattered;
        d.options_mut().scaling = true;
        d.options_mut().scale_min = 50.0;
        d.options_mut().scale_max = 150.0;
        d.set_spacing(20.0);
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("Distribution Specification"));
        let after = cx.project.distribution_copies(0, id);
        assert!(
            after > before,
            "tighter spacing, more objects ({before} to {after})"
        );
        let rec = cx.floor().symbol(id).unwrap();
        assert!(rec.distribution.as_ref().unwrap().options.is_some());
        cx.undo();
        assert_eq!(cx.project.distribution_copies(0, id), before);
    }

    #[test]
    fn a_path_can_be_spread_by_count_and_validates() {
        let (mut cx, id) = cx_with(DistKind::Path);
        let mut d = DistributionDialog::new(&cx, id).unwrap();
        d.options_mut().by_count = true;
        d.options_mut().count = 7;
        d.options_mut().center_objects = true;
        assert!(d.form.error().is_none());
        assert!(d.apply(&mut cx));
        assert_eq!(cx.project.distribution_copies(0, id), 7);
        d.options_mut().scaling = true;
        d.options_mut().scale_min = 200.0;
        d.options_mut().scale_max = 100.0;
        assert!(d.form.error().is_some());
    }

    #[test]
    fn auto_spacing_follows_the_object_size() {
        let (mut cx, id) = cx_with(DistKind::Region);
        let mut d = DistributionDialog::new(&cx, id).unwrap();
        d.options_mut().auto_spacing = true;
        d.set_spacing(auto_spacing([12.0, 12.0, 24.0]));
        assert!(d.apply(&mut cx));
        let rec = cx.floor().symbol(id).unwrap();
        let s = rec.distribution.as_ref().unwrap().spacing;
        assert!((s - 13.2).abs() < 1e-9, "{s}");
    }
}
