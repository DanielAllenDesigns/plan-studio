//! The panels most specification dialogs share besides Components and Object
//! Information (reference manual: Label Panel p. 730, Schedule Panel p. 735,
//! Manufacturer Panel p. 655), and the labels they put on the plan.
//!
//! The data is [`plan_core::object_pages::ObjectPages`], kept per object in
//! `Project.props.pages` under the object's Property Manager key. A dialog
//! needs no code to get the panels: the shared frame (`SpecDialog::frame`)
//! asks the armed [`super::object_info::InfoSession`] which panels the object
//! takes ([`Kind`]) and adds a tab for each. A dialog that already has a tab
//! of the same name (an opening's own Label and Schedule tabs, a symbol's
//! Schedule) keeps it, and the frame appends the part of the shared panel the
//! tab lacks under it ([`Placement::Appended`]).

use super::{row, section};
use crate::editor::camera::Camera;
use crate::editor::EditorContext;
use eframe::egui::{self, Color32, FontId, Ui, Vec2};
use plan_core::object_pages::{
    opening_automatic_label, LabelFacts, LabelLayer, LabelPage, ManufacturerPage, SchedulePage,
    OBJECT_MACROS,
};
use plan_core::openings::SizeFormat;
use plan_core::text_styles::MacroContext;
use plan_core::{Point, Project};

/// The kind of object a Property Manager key names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wall,
    Door,
    Window,
    Cabinet,
    Symbol,
    Stair,
    Roof,
    Framing,
    Device,
    Room,
    Foundation,
}

impl Kind {
    pub fn of_key(key: &str) -> Option<Kind> {
        Some(match key.split(':').next()? {
            "wall" => Kind::Wall,
            "door" => Kind::Door,
            "window" => Kind::Window,
            "cabinet" => Kind::Cabinet,
            "symbol" => Kind::Symbol,
            "stair" => Kind::Stair,
            "roof" => Kind::Roof,
            "framing" => Kind::Framing,
            "device" => Kind::Device,
            "room" => Kind::Room,
            "foundation" => Kind::Foundation,
            _ => return None,
        })
    }

    /// Does the object take the Label panel?
    pub fn takes_label(self) -> bool {
        self != Kind::Foundation
    }

    /// Does the object take the Schedule panel?
    pub fn takes_schedule(self) -> bool {
        self != Kind::Foundation
    }

    /// Does the object take the Manufacturer panel (the catalog items)?
    pub fn takes_manufacturer(self) -> bool {
        matches!(
            self,
            Kind::Door | Kind::Window | Kind::Cabinet | Kind::Symbol | Kind::Device
        )
    }

    /// Does the object have a height field the Elevation Reference widget
    /// can go beside?
    pub fn takes_elevation(self) -> bool {
        matches!(
            self,
            Kind::Door
                | Kind::Window
                | Kind::Cabinet
                | Kind::Symbol
                | Kind::Device
                | Kind::Foundation
        )
    }

    /// Does the object already draw a label of its own on the plan (the
    /// opening's, the cabinet's, a room's name, a symbol's or device's text)?
    /// The shared label then draws only when the user typed a custom one.
    pub fn draws_own_label(self) -> bool {
        matches!(
            self,
            Kind::Door
                | Kind::Window
                | Kind::Cabinet
                | Kind::Symbol
                | Kind::Device
                | Kind::Room
                | Kind::Roof
        )
    }

    /// The label layer of this kind (Label Layer > Use System Layer).
    pub fn system_layer(self) -> &'static str {
        match self {
            Kind::Wall => "Walls, Labels",
            Kind::Door => plan_core::layers::DOOR_LABEL_LAYER,
            Kind::Window => plan_core::layers::WINDOW_LABEL_LAYER,
            Kind::Cabinet => plan_core::layers::CABINET_LABEL_LAYER,
            Kind::Symbol => "Fixtures, Labels",
            Kind::Stair => "Stairs, Labels",
            Kind::Roof => "Roof, Labels",
            Kind::Framing => "Framing, Labels",
            Kind::Device => "Electrical, Labels",
            Kind::Room => "Room Labels",
            Kind::Foundation => "Foundation, Labels",
        }
    }
}

/// How a shared panel sits in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A tab of its own.
    Tab,
    /// Under the dialog's own tab of the same name: only the parts that tab
    /// does not already have.
    Appended,
}

// ----------------------------------------------------------------- panels --

/// The Label panel of one object.
pub fn label_panel(
    ui: &mut Ui,
    page: &mut LabelPage,
    facts: &LabelFacts,
    kind: Kind,
    placement: Placement,
    user_macros: &[String],
) {
    let full = placement == Placement::Tab;
    if full {
        section(ui, "Display Options");
        ui.checkbox(&mut page.suppress, "Suppress Label in All Views");
        ui.add_enabled_ui(!page.suppress, |ui| {
            ui.checkbox(&mut page.display_in_plan, "Display in Plan View");
        });
        section(ui, "Label");
        ui.horizontal(|ui| {
            ui.radio_value(&mut page.specify, false, "Automatic Label");
            ui.radio_value(&mut page.specify, true, "Specify Label");
        });
        if page.specify {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut page.text)
                        .desired_rows(2)
                        .desired_width(260.0),
                );
                ui.menu_button("Insert Macro", |ui| {
                    for (name, help) in OBJECT_MACROS {
                        if ui.button(*name).on_hover_text(*help).clicked() {
                            page.text.push_str(&format!("%{name}%"));
                            ui.close_menu();
                        }
                    }
                    for name in user_macros {
                        if ui.button(name).clicked() {
                            page.text.push_str(&format!("%{name}%"));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.checkbox(&mut page.default_formatting, "Use Default Formatting")
                .on_hover_text("Lengths as feet and inches; clear for plain inches");
        } else {
            if matches!(kind, Kind::Door | Kind::Window) {
                row(ui, "Size Format", |ui| {
                    ui.radio_value(
                        &mut page.size_format,
                        SizeFormat::HeightWidth,
                        "Height/Width",
                    );
                    ui.radio_value(
                        &mut page.size_format,
                        SizeFormat::WidthHeight,
                        "Width/Height",
                    );
                    ui.radio_value(&mut page.size_format, SizeFormat::WidthOnly, "Width Only");
                });
                ui.checkbox(&mut page.include_schedule_number, "Include Schedule Number");
                if kind == Kind::Window {
                    ui.checkbox(&mut page.include_type, "Include Type");
                }
            } else {
                ui.checkbox(&mut page.include_schedule_number, "Include Schedule Number");
            }
            let shown = automatic_text(page, facts, kind);
            ui.weak(format!(
                "The label reads: {}",
                if shown.is_empty() {
                    "(nothing)"
                } else {
                    &shown
                }
            ));
        }
    } else {
        section(ui, "Label Placement (all objects)");
    }
    row(ui, "X Offset", |ui| {
        ui.add(egui::DragValue::new(&mut page.offset_x).suffix(" in"))
            .on_hover_text("Along the front face of the object");
    });
    row(ui, "Y Offset", |ui| {
        ui.add(egui::DragValue::new(&mut page.offset_y).suffix(" in"))
            .on_hover_text("Away from the front face of the object");
    });
    row(ui, "Angle", |ui| {
        ui.add(
            egui::DragValue::new(&mut page.angle)
                .suffix(" deg")
                .range(-360.0..=360.0),
        );
        ui.checkbox(&mut page.absolute_angle, "Absolute Angle");
    });
    section(ui, "Label Layer");
    for l in LabelLayer::ALL {
        ui.radio_value(&mut page.layer, l, l.name());
    }
    if page.layer == LabelLayer::Custom {
        row(ui, "Layer Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.custom_layer).desired_width(180.0));
        });
    }
}

/// What the Automatic Label of the panel's current settings reads.
pub fn automatic_text(page: &LabelPage, facts: &LabelFacts, kind: Kind) -> String {
    let base = if matches!(kind, Kind::Door | Kind::Window) {
        // The size comes from the facts; the format and type from the panel.
        let (w, h) = (facts.width.unwrap_or(0.0), facts.height.unwrap_or(0.0));
        let ok = if kind == Kind::Door {
            plan_core::OpeningKind::Door
        } else {
            plan_core::OpeningKind::Window
        };
        let style = facts.style.unwrap_or_default();
        opening_automatic_label(
            ok,
            style,
            w,
            h,
            page.size_format,
            page.include_type,
            page.include_schedule_number
                .then_some(facts.schedule_number.as_str()),
        )
    } else if page.include_schedule_number && !facts.schedule_number.is_empty() {
        format!("{} {}", facts.schedule_number, facts.automatic)
    } else {
        facts.automatic.clone()
    };
    base.trim().to_string()
}

/// The Schedule panel of one object.
pub fn schedule_panel(ui: &mut Ui, page: &mut SchedulePage, placement: Placement) {
    section(ui, "Schedule");
    if placement == Placement::Tab {
        ui.checkbox(&mut page.include, "Include in Schedule")
            .on_hover_text("A cleared box leaves the object out of every schedule");
    }
    ui.add_enabled_ui(page.include, |ui| {
        ui.checkbox(&mut page.show_callout, "Show Schedule Callout")
            .on_hover_text("Draws the schedule's mark next to the object on the plan");
        row(ui, "Callout Location Rotation", |ui| {
            ui.add(
                egui::DragValue::new(&mut page.callout_rotation)
                    .suffix(" deg")
                    .range(-360.0..=360.0),
            );
        });
        if placement == Placement::Tab {
            let mut text = page.categories.join(", ");
            row(ui, "Include in Schedule As", |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text("Auto Schedule Category")
                            .desired_width(240.0),
                    )
                    .changed()
                {
                    page.categories = text
                        .split(',')
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                        .collect();
                }
            });
            ui.weak(
                "Blank follows the object's own schedule. Separate custom categories with commas.",
            );
        }
    });
}

/// The Manufacturer panel of one object (manual p. 655).
pub fn manufacturer_panel(ui: &mut Ui, page: &mut ManufacturerPage) {
    section(ui, "Manufacturer");
    for (label, text) in [
        ("Name", &mut page.name),
        ("Contact", &mut page.contact),
        ("Phone", &mut page.phone),
        ("Email", &mut page.email),
        ("Website", &mut page.website),
        ("Catalog", &mut page.catalog),
    ] {
        row(ui, label, |ui| {
            ui.add(egui::TextEdit::singleline(text).desired_width(260.0));
        });
    }
    row(ui, "Address", |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut page.address)
                .desired_rows(3)
                .desired_width(260.0),
        );
    });
}

// ------------------------------------------------------------------ facts --

/// What the plan knows about one object, for its label.
#[derive(Debug, Clone)]
pub struct Described {
    pub floor: usize,
    /// Where the object is (its centre), plan inches.
    pub at: Point,
    /// The direction of the object's front face, radians.
    pub front: f64,
    pub object_layer: String,
    pub facts: LabelFacts,
}

/// Finds the object under `key` and says what its label can report. `None`
/// for a key the plan no longer has, or one with nothing to label.
pub fn describe(project: &Project, key: &str) -> Option<Described> {
    let kind = Kind::of_key(key)?;
    let mut parts = key.split(':').skip(1);
    let first = parts.next()?;
    match kind {
        Kind::Wall => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let Some(w) = f.wall(id) else { continue };
                let d = Point::new(w.end.x - w.start.x, w.end.y - w.start.y);
                let name = w.wall_type.clone().unwrap_or_else(|| {
                    match w.kind {
                        plan_core::WallKind::Exterior => "Exterior Wall",
                        plan_core::WallKind::Interior => "Interior Wall",
                    }
                    .to_string()
                });
                return Some(Described {
                    floor: fi,
                    at: Point::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0),
                    front: d.y.atan2(d.x),
                    object_layer: w.layer.clone(),
                    facts: LabelFacts {
                        automatic: name.clone(),
                        type_name: name.clone(),
                        name,
                        length: Some(d.length()),
                        depth: Some(w.thickness),
                        height: Some(w.height),
                        ..LabelFacts::default()
                    },
                });
            }
            None
        }
        Kind::Door | Kind::Window => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let Some(o) = f.openings.iter().find(|o| o.id == id) else {
                    continue;
                };
                let w = f.wall(o.wall_id)?;
                let d = Point::new(w.end.x - w.start.x, w.end.y - w.start.y);
                let len = d.length().max(1e-9);
                let t = o.center_offset / len;
                let mark = o.schedule_number.clone().unwrap_or_default();
                let facts = LabelFacts {
                    automatic: opening_automatic_label(
                        o.kind,
                        o.style,
                        o.width,
                        o.height,
                        SizeFormat::WidthHeight,
                        false,
                        None,
                    ),
                    type_name: o.type_name().to_string(),
                    name: o.type_name().to_string(),
                    width: Some(o.width),
                    height: Some(o.height),
                    elevation: Some(o.sill_height),
                    schedule_number: mark,
                    style: Some(o.style),
                    ..LabelFacts::default()
                };
                return Some(Described {
                    floor: fi,
                    at: Point::new(w.start.x + d.x * t, w.start.y + d.y * t),
                    front: d.y.atan2(d.x),
                    object_layer: o.layer_name().to_string(),
                    facts,
                });
            }
            None
        }
        Kind::Symbol => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let Some(s) = f.symbol(id) else { continue };
                let name = if s.label.trim().is_empty() {
                    s.catalog_id.clone()
                } else {
                    s.label.clone()
                };
                return Some(Described {
                    floor: fi,
                    at: s.position,
                    front: s.angle,
                    object_layer: s.layer.clone(),
                    facts: LabelFacts {
                        automatic: name.clone(),
                        type_name: s.catalog_id.clone(),
                        name,
                        width: Some(s.width),
                        depth: Some(s.depth),
                        height: Some(s.height),
                        elevation: Some(s.elevation),
                        ..LabelFacts::default()
                    },
                });
            }
            None
        }
        Kind::Cabinet => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let Some(c) = crate::editor::placed::cabinet_by_id(f, id) else {
                    continue;
                };
                let auto = plan_cabinets::auto_label(&c);
                // The cabinet's local origin is its back-left corner.
                let (s, co) = c.angle.sin_cos();
                let (hw, hd) = (c.width / 2.0, c.depth / 2.0);
                let at = Point::new(
                    c.position.x + hw * co - hd * s,
                    c.position.y + hw * s + hd * co,
                );
                return Some(Described {
                    floor: fi,
                    at,
                    front: c.angle,
                    object_layer: plan_core::layers::CABINET_LABEL_LAYER.to_string(),
                    facts: LabelFacts {
                        automatic: auto.clone(),
                        type_name: format!("{:?}", c.kind),
                        name: auto,
                        width: Some(c.width),
                        depth: Some(c.depth),
                        height: Some(c.height),
                        elevation: Some(c.elevation),
                        ..LabelFacts::default()
                    },
                });
            }
            None
        }
        Kind::Stair => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let Some(s) = crate::editor::stairs_view::find(f, id) else {
                    continue;
                };
                let outline = plan_stairs::footprint(&s.stair);
                if outline.is_empty() {
                    return None;
                }
                return Some(Described {
                    floor: fi,
                    at: plan_core::details::centroid(&outline),
                    front: 0.0,
                    object_layer: String::new(),
                    facts: LabelFacts {
                        automatic: "Stair".into(),
                        type_name: "Stair".into(),
                        name: "Stair".into(),
                        ..LabelFacts::default()
                    },
                });
            }
            None
        }
        Kind::Roof => {
            let id: u64 = first.parse().ok()?;
            for (fi, f) in project.floors.iter().enumerate() {
                let set = crate::editor::roof_view::load(f);
                let Some(p) = set.plane(id) else { continue };
                let outline: Vec<Point> = p
                    .polygon3d
                    .iter()
                    .map(|v| Point::new(v[0], -v[2]))
                    .collect();
                if outline.len() < 3 {
                    return None;
                }
                let name = if p.label.trim().is_empty() {
                    "Roof Plane".to_string()
                } else {
                    p.label.clone()
                };
                return Some(Described {
                    floor: fi,
                    at: plan_core::details::centroid(&outline),
                    front: 0.0,
                    object_layer: p.layer.clone(),
                    facts: LabelFacts {
                        automatic: name.clone(),
                        type_name: "Roof Plane".into(),
                        name,
                        ..LabelFacts::default()
                    },
                });
            }
            None
        }
        Kind::Device => {
            let fi: usize = first.parse().ok()?;
            let id: u64 = parts.next()?.parse().ok()?;
            let f = project.floors.get(fi)?;
            let layer = crate::editor::site_view::load_electrical(f);
            let d = layer.device(id)?;
            let name = if d.label.trim().is_empty() {
                d.kind.name().to_string()
            } else {
                d.label.clone()
            };
            Some(Described {
                floor: fi,
                at: d.position,
                front: d.angle,
                object_layer: String::new(),
                facts: LabelFacts {
                    automatic: name.clone(),
                    type_name: d.kind.name().to_string(),
                    name,
                    elevation: Some(d.height),
                    ..LabelFacts::default()
                },
            })
        }
        Kind::Room => {
            let fi: usize = first.parse().ok()?;
            let xy = parts.next()?;
            let (x, y) = xy.split_once(',')?;
            let at = Point::new(x.parse().ok()?, y.parse().ok()?);
            let f = project.floors.get(fi)?;
            let rooms = plan_core::detect_rooms(&f.walls, 0.5);
            let room = rooms
                .iter()
                .find(|r| (r.centroid.x - at.x).abs() < 1.0 && (r.centroid.y - at.y).abs() < 1.0)?;
            let name = room
                .name_entry(&f.room_names)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            Some(Described {
                floor: fi,
                at: room.centroid,
                front: 0.0,
                object_layer: String::new(),
                facts: LabelFacts {
                    automatic: name.clone(),
                    type_name: "Room".into(),
                    name,
                    ..LabelFacts::default()
                },
            })
        }
        Kind::Framing | Kind::Foundation => None,
    }
}

/// Adds what Object Information says about the object to the facts.
pub fn with_info(project: &Project, key: &str, facts: &mut LabelFacts) {
    if let Some(i) = project.materials.info(key) {
        facts.code = i.code.clone();
        facts.comment = i.comment.clone();
        facts.description = i.description.clone();
        facts.manufacturer = i.manufacturer.clone();
        facts.supplier = i.supplier.clone();
    }
}

// ----------------------------------------------------------- plan labels --

/// One label to put on the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanLabel {
    pub key: String,
    pub text: String,
    pub at: Point,
    /// Text rotation, radians counter-clockwise in the plan.
    pub angle: f64,
    pub layer: String,
}

/// The labels the shared Label panels put on floor `floor`.
pub fn plan_labels(project: &Project, floor: usize, ctx: &MacroContext) -> Vec<PlanLabel> {
    let mut out = Vec::new();
    for (key, pages) in &project.props.pages {
        let Some(page) = &pages.label else { continue };
        let Some(kind) = Kind::of_key(key) else {
            continue;
        };
        // Objects that label themselves are only added to when the user typed
        // a custom label.
        if kind.draws_own_label() && !page.specify {
            continue;
        }
        let Some(mut d) = describe(project, key) else {
            continue;
        };
        if d.floor != floor {
            continue;
        }
        with_info(project, key, &mut d.facts);
        if matches!(kind, Kind::Door | Kind::Window) {
            d.facts.automatic = automatic_text(page, &d.facts, kind);
        }
        let Some(text) = page.text(&d.facts, ctx, &project.text_macros) else {
            continue;
        };
        let (s, c) = d.front.sin_cos();
        let at = Point::new(
            d.at.x + page.offset_x * c - page.offset_y * s,
            d.at.y + page.offset_x * s + page.offset_y * c,
        );
        let rad = page.angle.to_radians();
        let angle = if page.absolute_angle {
            rad
        } else {
            d.front + rad
        };
        out.push(PlanLabel {
            key: key.clone(),
            text,
            at,
            angle,
            layer: page.layer_name(kind.system_layer(), &d.object_layer),
        });
    }
    out
}

/// The macro values for labels on floor `floor`.
pub fn macro_context(project: &Project, floor: usize) -> MacroContext {
    let f = project.floors.get(floor);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    MacroContext {
        plan_name: project.name.clone(),
        plan_date: plan_core::text_styles::date_string(secs),
        floor_name: f.map(|f| f.name.clone()).unwrap_or_default(),
        floor_number: floor + 1,
        floor_count: project.floors.len(),
        ceiling_height: f
            .map(|f| plan_core::units::fmt_ft_in(f.ceiling_height))
            .unwrap_or_default(),
        ..MacroContext::default()
    }
}

/// Draws the shared labels of the active floor.
pub fn draw_labels(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    if cx.project.props.pages.is_empty() {
        return;
    }
    let ctx = macro_context(&cx.project, cx.floor);
    for l in plan_labels(&cx.project, cx.floor, &ctx) {
        if !cx.layers().is_visible(&l.layer) {
            continue;
        }
        let galley = painter.layout_no_wrap(
            l.text.clone(),
            FontId::proportional(crate::editor::rooms_edit::LABEL_FONT_PX as f32),
            cx.palette.room_label,
        );
        let centre = cam.world_to_screen(l.at);
        // The plan's y axis points down the screen; a plan angle turns the
        // text the other way round.
        let a = -l.angle as f32;
        let half = galley.size() * 0.5;
        let (s, c) = a.sin_cos();
        let origin = centre - Vec2::new(half.x * c - half.y * s, half.x * s + half.y * c);
        painter
            .add(egui::epaint::TextShape::new(origin, galley, Color32::PLACEHOLDER).with_angle(a));
    }
}

/// The names of the panels an object of `kind` takes besides Components and
/// Object Information, in tab order.
#[cfg(test)]
pub fn panel_names(kind: Kind) -> Vec<&'static str> {
    let mut v = Vec::new();
    if kind.takes_label() {
        v.push("Label");
    }
    if kind.takes_schedule() {
        v.push("Schedule");
    }
    if kind.takes_manufacturer() {
        v.push("Manufacturer");
    }
    v
}

/// Convenience for tests: the pages with one label panel set.
#[cfg(test)]
pub fn pages_with_label(page: LabelPage) -> plan_core::object_pages::ObjectPages {
    plan_core::object_pages::ObjectPages {
        label: Some(page),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::model::WallKind;

    fn wall_plan() -> (Project, u64) {
        let mut p = Project::new("labels");
        let id = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (p, id)
    }

    #[test]
    fn kinds_come_from_the_property_manager_keys() {
        assert_eq!(Kind::of_key("wall:3"), Some(Kind::Wall));
        assert_eq!(Kind::of_key("device:0:4"), Some(Kind::Device));
        assert_eq!(Kind::of_key("foundation:9"), Some(Kind::Foundation));
        assert_eq!(Kind::of_key("nope:1"), None);
        assert!(Kind::Cabinet.takes_manufacturer());
        assert!(!Kind::Wall.takes_manufacturer());
        assert!(Kind::Cabinet.takes_elevation());
        assert!(!Kind::Foundation.takes_label());
        assert_eq!(
            panel_names(Kind::Window),
            ["Label", "Schedule", "Manufacturer"]
        );
        assert_eq!(panel_names(Kind::Stair), ["Label", "Schedule"]);
    }

    #[test]
    fn a_wall_with_a_custom_label_puts_it_on_the_plan_with_its_macros() {
        let (mut p, id) = wall_plan();
        let key = format!("wall:{id}");
        // Nothing stored: nothing drawn.
        let ctx = macro_context(&p, 0);
        assert!(plan_labels(&p, 0, &ctx).is_empty());
        let page = LabelPage {
            specify: true,
            text: "%name% %length% (%code%)".into(),
            offset_y: 10.0,
            ..LabelPage::default()
        };
        assert!(p.props.set_pages(&key, pages_with_label(page)));
        let info = plan_core::materials_data::ObjectInfo {
            code: "W-1".into(),
            ..Default::default()
        };
        p.materials.objects.insert(key.clone(), info);
        let labels = plan_labels(&p, 0, &ctx);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, "Exterior Wall 10'-0\" (W-1)");
        // Ten inches off the wall, along its normal; the middle of the wall.
        assert_eq!(labels[0].at, Point::new(60.0, 10.0));
        assert_eq!(labels[0].layer, "Walls, Labels");
        // Suppressing it takes it off the plan.
        let mut page = p.props.pages_of(&key).unwrap().label_or_default();
        page.suppress = true;
        assert!(p.props.set_pages(&key, pages_with_label(page)));
        assert!(plan_labels(&p, 0, &ctx).is_empty());
    }

    #[test]
    fn a_stored_automatic_label_draws_the_automatic_text_and_layer_choice() {
        let (mut p, id) = wall_plan();
        let key = format!("wall:{id}");
        let page = LabelPage {
            layer: LabelLayer::Object,
            angle: 90.0,
            absolute_angle: true,
            offset_x: 5.0,
            ..LabelPage::default()
        };
        p.props.set_pages(&key, pages_with_label(page));
        let ctx = macro_context(&p, 0);
        let labels = plan_labels(&p, 0, &ctx);
        assert_eq!(labels[0].text, "Exterior Wall");
        assert_eq!(labels[0].layer, "Walls, Normal");
        assert!((labels[0].angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(labels[0].at, Point::new(65.0, 0.0));
    }

    #[test]
    fn a_window_label_follows_the_size_format_and_type() {
        let page = |fmt, ty| LabelPage {
            size_format: fmt,
            include_type: ty,
            ..LabelPage::default()
        };
        let facts = LabelFacts {
            width: Some(36.0),
            height: Some(48.0),
            style: Some(plan_core::OpeningStyle::Window),
            ..LabelFacts::default()
        };
        let t = |p: LabelPage| automatic_text(&p, &facts, Kind::Window);
        assert_eq!(t(page(SizeFormat::WidthHeight, true)), "3040 DH");
        assert_eq!(t(page(SizeFormat::HeightWidth, false)), "4030");
        assert_eq!(t(page(SizeFormat::WidthOnly, false)), "30");
    }
}
