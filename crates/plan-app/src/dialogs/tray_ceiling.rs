//! Tray Ceiling Specification (R-112; manual pp. 458-460): General,
//! Polyline, Moldings, Rope Lights, Line Style, Fill Style, Materials, Label
//! and Components. The same dialog opens from Make Tray Ceiling in Room and
//! Make Nested Tray Ceiling with the Width box (the Width setting is only
//! there for those two); OK then makes the tray as one undo step.
//!
//! The dialog edits a draft of the tray's [`TrayCeiling`] record and of the
//! line and fill attributes of its polyline. It is hosted here:
//! [`host_frame`] shows it every frame (`ToolSet::frame` calls it), so it
//! opens from the Edit toolbar or [`open_edit`] whichever tool is active.

use super::{on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab};
use super::{PV_ACCENT, PV_BG, PV_FAINT, PV_INK, PV_WALL};
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::cad::FillAttr;
use plan_core::extras::StructureLayer;
use plan_core::geometry::polygon_area;
use plan_core::layers::LineStyle;
use plan_core::rooms::MOLDING_LIBRARY;
use plan_core::tray::{self, RopeLight, TrayCeiling, TrayComponent, TrayMolding};
use plan_core::{Id, MoldingKind};
use std::cell::RefCell;

const TABS: &[Tab] = &[
    on("General"),
    on("Polyline"),
    on("Moldings"),
    on("Rope Lights"),
    on("Line Style"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
];

/// What the dialog does on OK.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Edit tray `id` of floor `floor`.
    Edit { floor: usize, id: Id },
    /// Make Tray Ceiling in Room: room `room` (an index into `cx.rooms`).
    MakeInRoom { floor: usize, room: usize },
    /// Make Nested Tray Ceiling inside tray `parent`.
    MakeNested { floor: usize, parent: Id },
}

/// Line and fill attributes of the tray's polyline (the Line Style and Fill
/// Style panels).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Look {
    pub dash: Option<LineStyle>,
    pub color: Option<[u8; 3]>,
    pub weight: Option<u32>,
    pub fill: Option<FillAttr>,
}

pub struct TrayDialog {
    frame: SpecDialog,
    form: Form,
    target: Target,
}

struct Form {
    draft: TrayCeiling,
    look: Look,
    fields: Fields,
    /// Perimeter and area of the hole (square inches), for the Polyline panel.
    perimeter: f64,
    area: f64,
    make: bool,
    layer: String,
}

// The accessors are the dialog's model API (the tests drive it).
#[allow(dead_code)]
impl TrayDialog {
    fn new(
        target: Target,
        draft: TrayCeiling,
        look: Look,
        outline: &[plan_core::Point],
        layer: String,
    ) -> Self {
        let make = !matches!(target, Target::Edit { .. });
        let title = "Tray Ceiling Specification";
        let n = outline.len();
        let perimeter = (0..n).map(|i| outline[i].dist(outline[(i + 1) % n])).sum();
        Self {
            frame: SpecDialog::new(title, "tray_ceiling"),
            form: Form {
                draft,
                look,
                fields: Fields::default(),
                perimeter,
                area: polygon_area(outline).abs(),
                make,
                layer,
            },
            target,
        }
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn draft(&self) -> &TrayCeiling {
        &self.form.draft
    }

    pub fn draft_mut(&mut self) -> &mut TrayCeiling {
        &mut self.form.draft
    }

    pub fn look_mut(&mut self) -> &mut Look {
        &mut self.form.look
    }

    /// Does the Width setting show (the Make dialogs)?
    pub fn shows_width(&self) -> bool {
        self.form.make
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }
}

fn look_of(cx: &EditorContext, floor: usize, id: Id) -> Look {
    let a = cx.project.floors[floor].cad_attrs(id).unwrap_or_default();
    Look {
        dash: a.dash,
        color: a.color,
        weight: a.weight,
        fill: a.fill,
    }
}

/// The dialog for tray `id` of the active floor.
pub fn dialog_for_edit(cx: &EditorContext, id: Id) -> Option<TrayDialog> {
    let rec = cx.floor().tray(id)?.clone();
    let outline = cx.floor().tray_outline(id)?;
    let layer = cx
        .floor()
        .cad
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.layer.clone())
        .unwrap_or_else(|| tray::LAYER.to_string());
    Some(TrayDialog::new(
        Target::Edit {
            floor: cx.floor,
            id,
        },
        rec,
        look_of(cx, cx.floor, id),
        &outline,
        layer,
    ))
}

/// The room's interior outline (the polyline a Make Tray Ceiling follows).
fn room_outline(cx: &EditorContext, room: usize) -> Option<Vec<plan_core::Point>> {
    let r = cx.rooms.get(room)?;
    Some(if r.inner_polygon.len() >= 3 {
        r.inner_polygon.clone()
    } else {
        r.polygon.clone()
    })
}

/// The Make Tray Ceiling in Room dialog for room `room` of the active floor.
pub fn dialog_for_room(cx: &EditorContext, room: usize) -> Option<TrayDialog> {
    let outline = room_outline(cx, room)?;
    let spec = default_spec(cx);
    let hole = tray::inset_outline(&outline, spec.width).unwrap_or_else(|| outline.clone());
    Some(TrayDialog::new(
        Target::MakeInRoom {
            floor: cx.floor,
            room,
        },
        spec,
        Look::default(),
        &hole,
        tray::LAYER.to_string(),
    ))
}

/// The Make Nested Tray Ceiling dialog for tray `parent`.
pub fn dialog_for_nested(cx: &EditorContext, parent: Id) -> Option<TrayDialog> {
    let outline = cx.floor().tray_outline(parent)?;
    let spec = TrayCeiling {
        width: 12.0,
        depth: 4.0,
        ..default_spec(cx)
    };
    let hole = tray::inset_outline(&outline, spec.width).unwrap_or_else(|| outline.clone());
    Some(TrayDialog::new(
        Target::MakeNested {
            floor: cx.floor,
            parent,
        },
        spec,
        Look::default(),
        &hole,
        tray::LAYER.to_string(),
    ))
}

/// Command id: Edit > Default Settings > Roofs > Tray Ceiling.
pub const DEFAULTS: &str = "defaults.tray_ceiling";

/// The floor number that marks the dialog as the Tray Ceiling Defaults (no
/// tray is made or edited; OK stores the plan defaults).
const DEFAULTS_FLOOR: usize = usize::MAX;

/// The tray a Make Tray Ceiling starts from: Default Settings > Roofs > Tray
/// Ceiling, else the built-in tray.
pub fn default_spec(cx: &EditorContext) -> TrayCeiling {
    cx.defaults.tray_default()
}

/// The Tray Ceiling Defaults dialog: the Tray Ceiling Specification on the
/// defaults a new tray starts from (no Width box, no polyline).
pub fn dialog_for_defaults(cx: &EditorContext) -> TrayDialog {
    let spec = default_spec(cx);
    let mut d = TrayDialog::new(
        Target::Edit {
            floor: DEFAULTS_FLOOR,
            id: 0,
        },
        spec,
        Look::default(),
        &[],
        tray::LAYER.to_string(),
    );
    d.frame = SpecDialog::new("Tray Ceiling Defaults", "tray_ceiling_defaults");
    d
}

/// Opens the Tray Ceiling Defaults dialog.
pub fn open_defaults(cx: &EditorContext) -> bool {
    host(Some(dialog_for_defaults(cx)))
}

/// Runs the commands of this module; false for any other id.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    match id {
        DEFAULTS => {
            open_defaults(cx);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------

fn style_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash Dot",
    }
}

fn crown_names() -> Vec<&'static str> {
    MOLDING_LIBRARY
        .iter()
        .filter(|m| m.kind == MoldingKind::Crown)
        .map(|m| m.name)
        .collect()
}

impl Form {
    fn sloped(&self) -> bool {
        !self.draft.is_vertical()
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "Tray Ceiling");
        if self.make {
            self.fields
                .length_row(ui, "Width", "tray_width", &mut self.draft.width);
        }
        self.fields
            .length_row(ui, "Depth", "tray_depth", &mut self.draft.depth);
        ui.checkbox(&mut self.draft.recess, "Recess into Ceiling");
        let mut vertical = self.draft.is_vertical();
        row(ui, "Pitch", |ui| {
            if ui.checkbox(&mut vertical, "Vertical").changed() {
                self.draft.pitch = if vertical { None } else { Some(12.0) };
            }
            if !vertical {
                let mut v = self.draft.pitch_display().unwrap_or(12.0);
                let (lo, hi) = if self.draft.pitch_in_degrees {
                    (1.0, 89.0)
                } else {
                    (0.1, 1000.0)
                };
                if ui
                    .add(egui::DragValue::new(&mut v).range(lo..=hi).speed(0.1))
                    .changed()
                {
                    self.draft.set_pitch_display(v);
                }
                ui.label(if self.draft.pitch_in_degrees {
                    "degrees"
                } else {
                    "in 12"
                });
            }
        });
        ui.add_enabled(
            !vertical,
            egui::Checkbox::new(&mut self.draft.pitch_in_degrees, "Pitch in Degrees"),
        );
        section(ui, "Ceiling Layers");
        row(ui, "Structure", |ui| {
            ui.label(format!(
                "{} thick",
                super::fmt_short(self.draft.structure_thickness())
            ));
        });
        let mut remove = None;
        for (i, l) in self.draft.structure.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Layer {}", i + 1));
                ui.add(egui::TextEdit::singleline(&mut l.material).desired_width(120.0));
                ui.add(
                    egui::DragValue::new(&mut l.thickness)
                        .range(0.0..=24.0)
                        .speed(0.05)
                        .suffix(" in"),
                );
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.draft.structure.remove(i);
        }
        if ui.button("Add Layer").clicked() {
            self.draft
                .structure
                .push(StructureLayer::new("Drywall", 0.5));
        }
        ui.checkbox(
            &mut self.draft.use_room_material_sides,
            "Use Room Ceiling Material for Sides",
        );
        ui.checkbox(
            &mut self.draft.use_room_material_top,
            "Use Room Ceiling Material for Top",
        );
        section(ui, "Framing");
        ui.checkbox(&mut self.draft.retain_framing, "Retain Framing");
        self.fields.length_row(
            ui,
            "Rafter Spacing",
            "tray_spacing",
            &mut self.draft.rafter_spacing,
        );
    }

    fn polyline(&mut self, ui: &mut Ui) {
        section(ui, "Polyline");
        row(ui, "Perimeter", |ui| {
            ui.label(super::fmt_short(self.perimeter));
        });
        row(ui, "Area", |ui| {
            ui.label(format!("{:.1} sq ft", self.area / 144.0));
        });
        ui.weak("The perimeter and area are of the inside of the tray (the hole).");
        ui.weak(format!("Layer: {}", self.layer));
    }

    fn moldings(&mut self, ui: &mut Ui) {
        let sloped = self.sloped();
        if sloped {
            ui.weak("Moldings cannot be specified while the sides are sloped.");
        }
        ui.add_enabled_ui(!sloped, |ui| {
            section(ui, "Moldings");
            let names = crown_names();
            let mut remove = None;
            for (i, m) in self.draft.moldings.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("tray_molding", i))
                        .selected_text(m.profile.clone())
                        .show_ui(ui, |ui| {
                            for n in &names {
                                ui.selectable_value(&mut m.profile, (*n).to_string(), *n);
                            }
                        });
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut m.height)
                            .range(0.25..=24.0)
                            .speed(0.05),
                    );
                    ui.label("H Offset");
                    ui.add(
                        egui::DragValue::new(&mut m.h_offset)
                            .range(-24.0..=24.0)
                            .speed(0.05),
                    );
                    ui.label("V Offset");
                    ui.add(
                        egui::DragValue::new(&mut m.v_offset)
                            .range(-24.0..=24.0)
                            .speed(0.05),
                    );
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.draft.moldings.remove(i);
            }
            if ui.button("Add Molding").clicked() {
                self.draft.moldings.push(TrayMolding::default());
            }
        });
    }

    fn rope_lights(&mut self, ui: &mut Ui) {
        let sloped = self.sloped();
        if sloped {
            ui.weak("Rope Lights cannot be specified while the sides are sloped.");
        }
        ui.add_enabled_ui(!sloped, |ui| {
            section(ui, "Rope Lights");
            let mut remove = None;
            for (i, r) in self.draft.rope_lights.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(120.0));
                    ui.label("Horizontal Offset");
                    ui.add(
                        egui::DragValue::new(&mut r.h_offset)
                            .range(-24.0..=24.0)
                            .speed(0.05),
                    );
                    ui.label("Vertical Offset");
                    ui.add(
                        egui::DragValue::new(&mut r.v_offset)
                            .range(0.0..=48.0)
                            .speed(0.05),
                    );
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.draft.rope_lights.remove(i);
            }
            if ui.button("Add Rope Light").clicked() {
                self.draft.rope_lights.push(RopeLight::default());
            }
            ui.weak("Only the offsets are set here; the Rope Light Specification holds the rest.");
        });
    }

    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let mut dash = self.look.dash.unwrap_or(LineStyle::Dashed);
        row(ui, "Line Style", |ui| {
            egui::ComboBox::from_id_salt("tray_dash")
                .selected_text(style_name(dash))
                .show_ui(ui, |ui| {
                    for s in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        ui.selectable_value(&mut dash, s, style_name(s));
                    }
                });
        });
        self.look.dash = (dash != LineStyle::Dashed).then_some(dash);
        let mut mm = f64::from(self.look.weight.unwrap_or(25)) / 100.0;
        row(ui, "Line Weight", |ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut mm)
                        .range(0.05..=5.0)
                        .speed(0.01)
                        .suffix(" mm"),
                )
                .changed()
            {
                self.look.weight = Some((mm * 100.0).round() as u32);
            }
        });
        let mut c = self.look.color.unwrap_or([150, 60, 150]);
        row(ui, "Color", |ui| {
            if ui.color_edit_button_srgb(&mut c).changed() {
                self.look.color = Some(c);
            }
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        let mut on_fill = self.look.fill.is_some();
        if ui
            .checkbox(&mut on_fill, "Fill the tray in plan view")
            .changed()
        {
            self.look.fill = on_fill.then(FillAttr::default);
        }
        if let Some(f) = self.look.fill.as_mut() {
            row(ui, "Color", |ui| {
                ui.color_edit_button_srgb(&mut f.color);
            });
            row(ui, "Pattern", |ui| {
                ui.add(egui::TextEdit::singleline(&mut f.pattern).desired_width(140.0));
            });
            ui.weak("Leave the pattern empty for a solid fill.");
            let mut op = f64::from(f.opacity) / 255.0 * 100.0;
            row(ui, "Opacity", |ui| {
                if ui
                    .add(egui::DragValue::new(&mut op).range(0.0..=100.0).suffix("%"))
                    .changed()
                {
                    f.opacity = (op / 100.0 * 255.0).round() as u8;
                }
            });
        }
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        ui.add_enabled_ui(!self.draft.use_room_material_sides, |ui| {
            row(ui, "Sides", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft.materials.sides)
                        .desired_width(180.0),
                );
            });
        });
        ui.add_enabled_ui(!self.draft.use_room_material_top, |ui| {
            row(ui, "Top", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft.materials.top).desired_width(180.0),
                );
            });
        });
        row(ui, "Outer Ceiling", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.materials.ring).desired_width(180.0));
        });
        ui.weak("An empty material follows the room's ceiling finish.");
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        row(ui, "Label", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.draft.label).desired_width(200.0));
        });
        ui.weak(
            "Labels show in plan views when the \"Roofs, Labels\" layer is on. \
             The automatic label is blank.",
        );
    }

    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Components");
        let mut remove = None;
        for (i, c) in self.draft.components.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(160.0));
                ui.add(
                    egui::DragValue::new(&mut c.quantity)
                        .range(0.0..=10000.0)
                        .speed(0.1),
                );
                ui.add(egui::TextEdit::singleline(&mut c.unit).desired_width(40.0));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.draft.components.remove(i);
        }
        if ui.button("Add Component").clicked() {
            self.draft.components.push(TrayComponent::default());
        }
        ui.weak("Components are listed in the Materials List.");
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("A number is not valid".into());
        }
        let d = &self.draft;
        if d.depth <= 0.0 {
            return Some("The Depth must be larger than zero".into());
        }
        if self.make && d.width <= 0.0 {
            return Some("The Width must be larger than zero".into());
        }
        if d.rafter_spacing < 1.0 {
            return Some("The Rafter Spacing must be at least 1 in".into());
        }
        if d.run() > d.width + 1e-6 && self.make {
            return Some("The sloped sides are wider than the Width".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => self.general(ui),
            "Polyline" => self.polyline(ui),
            "Moldings" => self.moldings(ui),
            "Rope Lights" => self.rope_lights(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Components" => self.components(ui),
            _ => {}
        }
    }

    /// A section through the step: the dropped (or raised) ceiling, the hole
    /// and the side, vertical or sloped.
    fn preview(&self, p: &Painter, rect: Rect) {
        p.rect_filled(rect, 2.0, PV_BG);
        pv_text(
            p,
            rect.min + egui::vec2(0.0, 8.0),
            Align2::LEFT_CENTER,
            "Section",
            11.0,
        );
        let d = &self.draft;
        let area = Rect::from_min_max(
            rect.min + egui::vec2(8.0, 28.0),
            rect.max - egui::vec2(8.0, 16.0),
        );
        let w = area.width();
        let scale = (area.height() * 0.5 / d.depth.max(1.0) as f32).min(4.0);
        let step = (d.depth as f32 * scale).max(4.0);
        let base_y = area.min.y + area.height() * 0.45;
        // Outer ceiling on the left, the hole in the middle (the hole is the
        // polyline), the outer ceiling again on the right.
        let (outer_y, inner_y) = if d.recess {
            (base_y, base_y - step)
        } else {
            (base_y + step, base_y)
        };
        let ring = w * 0.28;
        let run = if d.is_vertical() {
            0.0
        } else {
            ((d.run() as f32 * scale).min(ring)).max(2.0)
        };
        let x0 = area.min.x;
        let x1 = x0 + ring;
        let x2 = area.max.x - ring;
        let x3 = area.max.x;
        let pts = [
            Pos2::new(x0, outer_y),
            Pos2::new(x1 - run, outer_y),
            Pos2::new(x1, inner_y),
            Pos2::new(x2, inner_y),
            Pos2::new(x2 + run, outer_y),
            Pos2::new(x3, outer_y),
        ];
        // Walls on both sides.
        p.rect_filled(
            Rect::from_min_max(Pos2::new(x0 - 4.0, area.min.y), Pos2::new(x0, area.max.y)),
            0.0,
            PV_WALL,
        );
        p.rect_filled(
            Rect::from_min_max(Pos2::new(x3, area.min.y), Pos2::new(x3 + 4.0, area.max.y)),
            0.0,
            PV_WALL,
        );
        p.add(Shape::line(pts.to_vec(), Stroke::new(2.0_f32, PV_INK)));
        // The platform above the room's ceiling.
        p.line_segment(
            [
                Pos2::new(x0, base_y.min(inner_y) - 10.0),
                Pos2::new(x3, base_y.min(inner_y) - 10.0),
            ],
            Stroke::new(1.0_f32, PV_FAINT),
        );
        pv_text(
            p,
            Pos2::new((x1 + x2) / 2.0, inner_y + 12.0),
            Align2::CENTER_CENTER,
            if d.recess {
                "raised ceiling"
            } else {
                "room ceiling"
            },
            10.0,
        );
        pv_text(
            p,
            Pos2::new(x0 + ring / 2.0, outer_y + 12.0),
            Align2::CENTER_CENTER,
            if d.recess { "ceiling" } else { "dropped" },
            10.0,
        );
        pv_text(
            p,
            Pos2::new((x1 + x2) / 2.0, rect.max.y - 8.0),
            Align2::CENTER_CENTER,
            format!("Depth {}", super::fmt_short(d.depth)),
            11.0,
        );
        let _ = PV_ACCENT;
    }
}

// ---------------------------------------------------------------------------
// Applying and hosting
// ---------------------------------------------------------------------------

/// Writes an accepted dialog as one undo step. Returns the tray's id.
pub fn apply(
    cx: &mut EditorContext,
    target: Target,
    draft: &TrayCeiling,
    look: &Look,
) -> Option<Id> {
    if matches!(
        target,
        Target::Edit {
            floor: DEFAULTS_FLOOR,
            ..
        }
    ) {
        let mut spec = draft.clone();
        spec.id = 0;
        cx.defaults.tray_ceiling = (spec != TrayCeiling::default()).then_some(spec);
        cx.status = "Saved the tray ceiling defaults".into();
        return None;
    }
    let label = match target {
        Target::Edit { .. } => "Tray Ceiling Specification",
        Target::MakeInRoom { .. } => "Make Tray Ceiling in Room",
        Target::MakeNested { .. } => "Make Nested Tray Ceiling",
    };
    let floor = match target {
        Target::Edit { floor, .. }
        | Target::MakeInRoom { floor, .. }
        | Target::MakeNested { floor, .. } => floor,
    };
    if floor >= cx.project.floors.len() {
        return None;
    }
    cx.begin_change(label);
    let id = match target {
        Target::Edit { id, .. } => {
            if !cx.project.floors[floor].is_tray(id) {
                cx.cancel_change();
                return None;
            }
            let mut rec = draft.clone();
            rec.id = id;
            cx.project.floors[floor].trays.set(rec);
            Some(id)
        }
        Target::MakeInRoom { room, .. } => room_outline(cx, room)
            .and_then(|o| cx.project.make_tray_in_room(floor, &o, draft.clone())),
        Target::MakeNested { parent, .. } => {
            cx.project.make_nested_tray(floor, parent, draft.clone())
        }
    };
    let Some(id) = id else {
        cx.cancel_change();
        cx.status = "The tray ceiling does not fit: use a smaller Width".into();
        return None;
    };
    cx.project.edit_cad_attrs(floor, id, |a| {
        a.dash = look.dash;
        a.color = look.color;
        a.weight = look.weight;
        a.fill = look.fill.clone();
    });
    cx.mark_dirty();
    cx.refresh();
    cx.selection.set(ObjectRef::Cad(id));
    cx.status = format!("{label}: done");
    Some(id)
}

thread_local! {
    static HOST: RefCell<Option<TrayDialog>> = const { RefCell::new(None) };
}

fn host(d: Option<TrayDialog>) -> bool {
    let opened = d.is_some();
    if opened {
        HOST.with(|h| *h.borrow_mut() = d);
    }
    opened
}

/// Opens the Tray Ceiling Specification of tray `id` of the active floor.
/// False when `id` is not a tray ceiling.
pub fn open_edit(cx: &EditorContext, id: Id) -> bool {
    host(dialog_for_edit(cx, id))
}

/// Opens the Make Tray Ceiling in Room dialog for room `room`.
pub fn open_make_in_room(cx: &EditorContext, room: usize) -> bool {
    host(dialog_for_room(cx, room))
}

/// Opens the Make Nested Tray Ceiling dialog inside tray `parent`.
pub fn open_make_nested(cx: &EditorContext, parent: Id) -> bool {
    host(dialog_for_nested(cx, parent))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dialog_open() -> bool {
    HOST.with(|h| h.borrow().is_some())
}

/// Test access to the open dialog.
#[cfg(test)]
pub fn with_dialog<R>(f: impl FnOnce(&mut TrayDialog) -> R) -> Option<R> {
    HOST.with(|h| h.borrow_mut().as_mut().map(f))
}

/// Closes the open dialog and applies it as OK would.
#[cfg(test)]
pub fn accept_dialog(cx: &mut EditorContext) -> Option<Id> {
    let d = HOST.with(|h| h.borrow_mut().take())?;
    apply(cx, d.target, &d.form.draft, &d.form.look)
}

/// Shows the open dialog once a frame and applies its OK.
pub fn host_frame(cx: &mut EditorContext, ctx: &egui::Context) {
    let Some(mut d) = HOST.with(|h| h.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => HOST.with(|h| *h.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            apply(cx, d.target, &d.form.draft, &d.form.look);
        }
    }
}
