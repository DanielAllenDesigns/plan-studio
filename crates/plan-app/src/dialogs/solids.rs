//! 3D Solid Options and the compound 3D Solid Specification (reference
//! manual "3D Solid Tools", pp. 1063 to 1081; parity rows CB-439..CB-458).
//!
//! * **3D Solid Options** supplements the Details tool's own specification
//!   of a box, cylinder, sphere, cone, pyramid or polyline solid with what
//!   the record lacks: rotation about the X and Y axes, the elevation
//!   reference, Retain Aspect Ratio, the label, 3D Surface Quality and the
//!   pyramid's sides, definition, size and truncation (`SolidExt`).
//! * **3D Solid Specification** of a compound solid (the result of Union,
//!   Subtract and Intersect): name, material, layer, elevation, label and
//!   surface quality.
//!
//! Both edit a copy; OK is one undo step.

use super::{row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use crate::editor::{details_view, EditorContext, ObjectRef};
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::arch_block::ElevationRef;
use plan_core::details::{DetailsLayer, Solid3d, SolidKind};
use plan_core::solids::{CompoundSolid, PyramidDef, PyramidSpec, SolidExt};
use plan_core::{Id, ObjectRef as CoreRef};
use std::cell::RefCell;

const OPTION_TABS: &[Tab] = &[
    Tab {
        name: "Size/Position",
        enabled: true,
    },
    Tab {
        name: "Pyramid",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
    Tab {
        name: "Surface Quality",
        enabled: true,
    },
];

const COMPOUND_TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
    Tab {
        name: "Surface Quality",
        enabled: true,
    },
];

/// What the window edits.
#[derive(Clone, Debug, PartialEq)]
pub enum Draft {
    /// A primitive: the record, its extra fields and the working pyramid.
    Primitive {
        solid: Solid3d,
        ext: SolidExt,
        pyramid: Option<PyramidSpec>,
    },
    /// A compound solid and the height of its bottom above the floor.
    Compound(Box<CompoundSolid>, f64),
}

pub struct SolidDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: Draft,
    /// The pyramid spec the dialog opened with (derived from the outline
    /// when none is stored): an unchanged one is not stored.
    initial_pyramid: Option<PyramidSpec>,
    layers: Vec<String>,
    materials: Vec<String>,
    fields: Fields,
}

impl SolidDialog {
    /// The window for solid `r` (a [`CoreRef::Detail`] primitive or a
    /// [`CoreRef::Solid`]), or `None` when it is gone.
    pub fn new(cx: &EditorContext, r: CoreRef) -> Option<Self> {
        let f = cx.floor();
        let (draft, initial_pyramid, title, key) = match r {
            CoreRef::Detail(id) => {
                let layer = DetailsLayer::load(f);
                let solid = layer.solid(id)?.clone();
                let ext = f.solid_layer.ext_or_default(id);
                let pyramid = match &solid.kind {
                    SolidKind::Pyramid { outline, h } => Some(
                        ext.pyramid
                            .unwrap_or_else(|| PyramidSpec::of_outline(outline, *h)),
                    ),
                    _ => None,
                };
                (
                    Draft::Primitive {
                        solid,
                        ext,
                        pyramid,
                    },
                    pyramid,
                    "3D Solid Options",
                    "solid_options",
                )
            }
            CoreRef::Solid(id) => {
                let c = f.solid_layer.compound(id)?.clone();
                let bottom = c.bottom();
                (
                    Draft::Compound(Box::new(c), bottom),
                    None,
                    "3D Solid Specification",
                    "solid_compound",
                )
            }
            _ => return None,
        };
        let mut layers: Vec<String> = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        let own = match &draft {
            Draft::Primitive { solid, .. } => solid.layer.clone(),
            Draft::Compound(c, _) => c.layer.clone(),
        };
        if !layers.contains(&own) {
            layers.push(own);
        }
        Some(Self {
            frame: SpecDialog::new(title, key),
            form: Form {
                draft,
                initial_pyramid,
                layers,
                materials: details_view::material_names(),
                fields: Fields::default(),
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &Draft {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut Draft {
        &mut self.form.draft
    }

    /// OK: one undo step. Returns whether anything changed.
    pub fn apply(&self, cx: &mut EditorContext) -> bool {
        apply(cx, &self.form.draft, self.form.initial_pyramid)
    }
}

/// Stores `draft` (see [`SolidDialog::apply`]).
pub fn apply(cx: &mut EditorContext, draft: &Draft, initial_pyramid: Option<PyramidSpec>) -> bool {
    let fl = cx.floor;
    match draft {
        Draft::Primitive {
            solid,
            ext,
            pyramid,
        } => {
            let layer = DetailsLayer::load(cx.floor());
            let Some(old) = layer.solid(solid.id).cloned() else {
                return false;
            };
            let old_ext = cx.floor().solid_layer.ext_or_default(solid.id);
            let mut solid = solid.clone();
            let mut ext = ext.clone();
            // A pyramid the user reshaped rewrites the outline; an untouched
            // one keeps the record as it was.
            if *pyramid != initial_pyramid {
                if let (Some(spec), SolidKind::Pyramid { outline, h }) = (pyramid, &mut solid.kind)
                {
                    *outline = spec.outline();
                    let _ = h;
                    ext.pyramid = Some(*spec);
                }
            } else if old_ext.pyramid.is_none() {
                ext.pyramid = None;
            }
            if solid == old && ext == old_ext {
                return false;
            }
            details_view::edit(cx, "3D Solid Options", |l| {
                if let Some(s) = l.solids.iter_mut().find(|s| s.id == solid.id) {
                    *s = solid.clone();
                }
            });
            cx.project.floors[fl].solid_layer.set_ext(ext);
            cx.mark_dirty();
            true
        }
        Draft::Compound(c, bottom) => {
            let Some(old) = cx.floor().solid_layer.compound(c.id).cloned() else {
                return false;
            };
            let mut new = (**c).clone();
            if (new.bottom() - bottom).abs() > 1e-9 {
                new.set_bottom(*bottom);
            }
            if new == old {
                return false;
            }
            cx.begin_change("3D Solid Specification");
            if let Some(slot) = cx.project.floors[fl].solid_layer.compound_mut(new.id) {
                *slot = new;
            }
            crate::editor::solids_view::ensure_layers(&mut cx.project.layers);
            cx.mark_dirty();
            true
        }
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        match self.draft {
            Draft::Primitive { .. } => OPTION_TABS,
            Draft::Compound(..) => COMPOUND_TABS,
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft {
            Draft::Primitive { pyramid, ext, .. } => {
                if let Some(p) = pyramid {
                    if !(3..=64).contains(&p.sides) {
                        return Some("A pyramid has 3 to 64 sides".into());
                    }
                    if p.size <= 0.0 {
                        return Some("The pyramid needs a size".into());
                    }
                    if p.truncated && p.truncated_height <= 0.0 {
                        return Some("The truncated top needs a height".into());
                    }
                }
                if !ext.quality.automatic && ext.quality.max_deflection <= 0.0 {
                    return Some("Maximum Deflection must be greater than zero".into());
                }
                None
            }
            Draft::Compound(c, _) => {
                if c.name.trim().is_empty() {
                    Some("The solid needs a name".into())
                } else if !c.quality.automatic && c.quality.max_deflection <= 0.0 {
                    Some("Maximum Deflection must be greater than zero".into())
                } else {
                    None
                }
            }
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match &mut self.draft {
            Draft::Primitive {
                solid,
                ext,
                pyramid,
            } => match tab {
                0 => primitive_position(ui, &mut self.fields, solid, ext),
                1 => pyramid_page(ui, &mut self.fields, solid, pyramid),
                2 => label_page(ui, &mut ext.label),
                _ => quality_page(ui, &mut self.fields, &mut ext.quality),
            },
            Draft::Compound(c, bottom) => match tab {
                0 => compound_general(
                    ui,
                    &mut self.fields,
                    c,
                    bottom,
                    &self.layers,
                    &self.materials,
                ),
                1 => label_page(ui, &mut c.label),
                _ => quality_page(ui, &mut self.fields, &mut c.quality),
            },
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let ink = egui::Color32::from_rgb(0x2B, 0x2B, 0x2B);
        let text = match &self.draft {
            Draft::Primitive { solid, .. } => match solid.kind {
                SolidKind::Box { .. } => "Box",
                SolidKind::Cylinder { .. } => "Cylinder",
                SolidKind::Sphere { .. } => "Sphere",
                SolidKind::Cone { .. } => "Cone",
                SolidKind::PolylineSolid { .. } => "Polyline Solid",
                SolidKind::Pyramid { .. } => "Pyramid",
                SolidKind::Face { .. } => "Face",
            }
            .to_string(),
            Draft::Compound(c, _) => format!("{}\n{:.0} cu in", c.name, c.volume()),
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(13.0),
            ink,
        );
    }
}

fn primitive_position(ui: &mut Ui, fields: &mut Fields, solid: &Solid3d, ext: &mut SolidExt) {
    section(ui, "Rotation");
    ui.label(format!(
        "About the vertical axis: {:.1}\u{B0} (3D Solid Specification)",
        solid.rotation
    ));
    row(ui, "Rotation about X", |ui| {
        fields.degrees(ui, "deg_rot_x", &mut ext.rot_x)
    });
    row(ui, "Rotation about Y", |ui| {
        fields.degrees(ui, "deg_rot_y", &mut ext.rot_y)
    });
    ui.add_space(4.0);
    section(ui, "Elevation");
    row(ui, "Elevations measured", |ui| {
        egui::ComboBox::from_id_salt("solid_elev_ref")
            .selected_text(ext.elevation_ref.name())
            .show_ui(ui, |ui| {
                for r in ElevationRef::ALL {
                    ui.selectable_value(&mut ext.elevation_ref, r, r.name());
                }
            });
    });
    ui.checkbox(&mut ext.retain_aspect, "Retain Aspect Ratio");
}

fn pyramid_page(
    ui: &mut Ui,
    fields: &mut Fields,
    solid: &Solid3d,
    pyramid: &mut Option<PyramidSpec>,
) {
    section(ui, "Pyramid");
    let (Some(p), SolidKind::Pyramid { .. }) = (pyramid.as_mut(), &solid.kind) else {
        ui.weak("This solid is not a pyramid.");
        return;
    };
    row(ui, "Number of sides", |ui| {
        ui.add(egui::DragValue::new(&mut p.sides).range(3..=64))
    });
    row(ui, "Defined by", |ui| {
        egui::ComboBox::from_id_salt("pyramid_def")
            .selected_text(p.def.name())
            .show_ui(ui, |ui| {
                for d in PyramidDef::ALL {
                    ui.selectable_value(&mut p.def, d, d.name());
                }
            });
    });
    fields.length_row(ui, "Size", "py_size", &mut p.size);
    ui.checkbox(&mut p.truncated, "Truncate the top");
    ui.add_enabled_ui(p.truncated, |ui| {
        fields.length_row(
            ui,
            "Height of the flat top",
            "py_trunc",
            &mut p.truncated_height,
        );
    });
}

fn label_page(ui: &mut Ui, label: &mut String) {
    section(ui, "Label");
    row(ui, "Label text", |ui| ui.text_edit_singleline(label));
    ui.weak("Blank is the automatic label (none).");
}

fn quality_page(ui: &mut Ui, fields: &mut Fields, q: &mut plan_core::solids::SurfaceQuality) {
    section(ui, "3D Surface Quality");
    ui.radio_value(&mut q.automatic, true, "Automatic");
    ui.radio_value(&mut q.automatic, false, "Maximum Deflection");
    ui.add_enabled_ui(!q.automatic, |ui| {
        fields.length_row(
            ui,
            "Maximum Deflection",
            "deflection",
            &mut q.max_deflection,
        );
    });
}

fn compound_general(
    ui: &mut Ui,
    fields: &mut Fields,
    c: &mut CompoundSolid,
    bottom: &mut f64,
    layers: &[String],
    materials: &[String],
) {
    section(ui, "3D Solid");
    row(ui, "Name", |ui| ui.text_edit_singleline(&mut c.name));
    row(ui, "Material", |ui| {
        egui::ComboBox::from_id_salt("compound_material")
            .selected_text(c.material.clone())
            .show_ui(ui, |ui| {
                for m in materials {
                    ui.selectable_value(&mut c.material, m.clone(), m);
                }
            });
    });
    row(ui, "Layer", |ui| {
        egui::ComboBox::from_id_salt("compound_layer")
            .selected_text(c.layer.clone())
            .show_ui(ui, |ui| {
                for n in layers {
                    ui.selectable_value(&mut c.layer, n.clone(), n);
                }
            });
    });
    row(ui, "Elevations measured", |ui| {
        egui::ComboBox::from_id_salt("compound_elev_ref")
            .selected_text(c.elevation_ref.name())
            .show_ui(ui, |ui| {
                for r in ElevationRef::ALL {
                    ui.selectable_value(&mut c.elevation_ref, r, r.name());
                }
            });
    });
    fields.length_row(ui, "Bottom elevation", "bottom", bottom);
    ui.add_space(4.0);
    section(ui, "Size");
    if let Some((lo, hi)) = c.bounds3() {
        ui.label(format!(
            "{:.1} x {:.1} x {:.1} in, volume {:.0} cubic in",
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2],
            c.volume()
        ));
    }
}

thread_local! {
    static DIALOG: RefCell<Option<SolidDialog>> = const { RefCell::new(None) };
}

/// Opens the window for solid `r`.
pub fn open(cx: &EditorContext, r: CoreRef) {
    if let Some(d) = SolidDialog::new(cx, r) {
        DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

/// Opens the specification of compound solid `id` (the spec route).
pub fn open_compound(cx: &EditorContext, id: Id) {
    open(cx, CoreRef::Solid(id));
}

pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

/// Test access to the open window's draft.
#[cfg(test)]
pub fn with_open<R>(f: impl FnOnce(&mut SolidDialog) -> R) -> Option<R> {
    DIALOG.with(|d| d.borrow_mut().as_mut().map(f))
}

/// Draws the window when open and applies an OK.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => DIALOG.with(|slot| *slot.borrow_mut() = Some(d)),
        Outcome::Ok => {
            if d.apply(cx) {
                if let Draft::Primitive { solid, .. } = d.draft() {
                    cx.selection.items = vec![ObjectRef::Detail(solid.id)];
                }
            }
        }
        Outcome::Cancel => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::solids::{boolean_floor, BoolOp};

    fn cx_with_pyramid() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = details_view::add_solid(
            &mut cx,
            SolidKind::Pyramid {
                outline: PyramidSpec::default().outline(),
                h: 36.0,
            },
            Point::new(10.0, 10.0),
        );
        (cx, id)
    }

    #[test]
    fn an_untouched_pyramid_stores_nothing() {
        let (mut cx, id) = cx_with_pyramid();
        let d = SolidDialog::new(&cx, CoreRef::Detail(id)).unwrap();
        assert!(!d.apply(&mut cx), "unchanged: no undo step");
        assert!(cx.floor().solid_layer.ext.is_empty());
    }

    #[test]
    fn reshaping_a_pyramid_rewrites_its_outline_and_truncates() {
        let (mut cx, id) = cx_with_pyramid();
        let mut d = SolidDialog::new(&cx, CoreRef::Detail(id)).unwrap();
        if let Draft::Primitive { pyramid, ext, .. } = d.draft_mut() {
            let p = pyramid.as_mut().unwrap();
            p.sides = 6;
            p.size = 20.0;
            p.truncated = true;
            p.truncated_height = 12.0;
            ext.rot_x = 15.0;
            ext.label = "Cap".into();
        }
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("3D Solid Options"));
        let layer = DetailsLayer::load(cx.floor());
        let SolidKind::Pyramid { outline, .. } = &layer.solid(id).unwrap().kind else {
            panic!("still a pyramid");
        };
        assert_eq!(outline.len(), 6);
        let ext = cx.floor().solid_layer.ext_of(id).unwrap();
        assert!(ext.pyramid.unwrap().truncated);
        assert_eq!(ext.rot_x, 15.0);
        assert_eq!(ext.label, "Cap");
        cx.undo();
        assert!(cx.floor().solid_layer.ext.is_empty());
        let layer = DetailsLayer::load(cx.floor());
        let SolidKind::Pyramid { outline, .. } = &layer.solid(id).unwrap().kind else {
            panic!("still a pyramid");
        };
        assert_eq!(outline.len(), 4);
    }

    #[test]
    fn a_bad_pyramid_blocks_ok() {
        let (cx, id) = cx_with_pyramid();
        let mut d = SolidDialog::new(&cx, CoreRef::Detail(id)).unwrap();
        assert!(d.form.error().is_none());
        if let Draft::Primitive { pyramid, .. } = d.draft_mut() {
            pyramid.as_mut().unwrap().sides = 2;
        }
        assert!(d.form.error().is_some());
    }

    #[test]
    fn the_compound_spec_edits_material_name_and_elevation() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let a = details_view::add_solid(
            &mut cx,
            SolidKind::Box {
                w: 24.0,
                d: 24.0,
                h: 24.0,
            },
            Point::ZERO,
        );
        let b = details_view::add_solid(
            &mut cx,
            SolidKind::Box {
                w: 12.0,
                d: 12.0,
                h: 40.0,
            },
            Point::ZERO,
        );
        let fl = cx.floor;
        let id = cx.project.alloc_id();
        boolean_floor(
            &mut cx.project.floors[fl],
            BoolOp::Union,
            &[CoreRef::Detail(a), CoreRef::Detail(b)],
            id,
        )
        .unwrap();
        let mut d = SolidDialog::new(&cx, CoreRef::Solid(id)).unwrap();
        if let Draft::Compound(c, bottom) = d.draft_mut() {
            c.name = "Mantel".into();
            c.material = "Oak".into();
            *bottom = 30.0;
        }
        assert!(d.apply(&mut cx));
        assert_eq!(cx.undo_label(), Some("3D Solid Specification"));
        let c = cx.floor().solid_layer.compound(id).unwrap();
        assert_eq!(c.name, "Mantel");
        assert!((c.bottom() - 30.0).abs() < 1e-9);
        cx.undo();
        assert_eq!(cx.floor().solid_layer.compound(id).unwrap().bottom(), 0.0);
    }
}
