//! Wall Specification (docs/chief-x18-dialogs.md, "Wall Specification").

use super::opening::OPENING_MARGIN;
use super::{
    dis_check, dis_combo, dis_radio, layer_stack, off, on, pv_text, row, section, session_check,
    wall_plan_sketch, Fields, Outcome, SpecDialog, SpecPages, Tab, WallTypeDialog, PV_FAINT,
    SESSION_NOTE,
};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::{Id, Opening, Wall, WallKind, WallTypeDef};

/// Extras-map keys for the two default-wall dialogs (real ids count up from 1).
const DEFAULT_EXTERIOR_KEY: Id = Id::MAX;
const DEFAULT_INTERIOR_KEY: Id = Id::MAX - 1;
const DEFAULT_FOUNDATION_KEY: Id = Id::MAX - 5;

/// What a wall dialog is bound to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WallTarget {
    Wall(Id),
    /// Edit > Default Settings > Walls > Exterior Wall.
    DefaultExterior,
    DefaultInterior,
    DefaultFoundation,
}

impl WallTarget {
    /// Key of this target in the app's per-session extras map.
    pub fn key(self) -> Id {
        match self {
            WallTarget::Wall(id) => id,
            WallTarget::DefaultExterior => DEFAULT_EXTERIOR_KEY,
            WallTarget::DefaultInterior => DEFAULT_INTERIOR_KEY,
            WallTarget::DefaultFoundation => DEFAULT_FOUNDATION_KEY,
        }
    }

    fn is_default(self) -> bool {
        !matches!(self, WallTarget::Wall(_))
    }
}

const WALL_LAYERS: [&str; 4] = [
    "Walls, Normal",
    "Walls, Invisible",
    "Walls, Labels",
    "Footings",
];

const WALL_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    off("Roof"),
    off("Foundation"),
    on("Wall Types"),
    off("Wall Cap"),
    off("Wall Covering"),
    off("Rail Style"),
    off("Newels/Balusters"),
    off("Rails"),
    on("Layer"),
    off("Materials"),
    on("Label"),
    off("Components"),
    off("Object Information"),
    off("Schedule"),
];

/// Wall settings the model has no fields for yet. The app keeps one per wall
/// id for the session only. (Invisible, No Room Definition and No Locate live
/// in `Wall.flags`.)
#[derive(Clone, Debug, PartialEq)]
pub struct WallExtras {
    /// Name of the wall type last picked in the Wall Types tab.
    wall_type: Option<String>,
    suppress_label: bool,
    display_in_plan: bool,
    specify_label: bool,
    label_text: String,
}

impl Default for WallExtras {
    fn default() -> Self {
        Self {
            wall_type: None,
            suppress_label: false,
            display_in_plan: true,
            specify_label: false,
            label_text: String::new(),
        }
    }
}

/// Which wall point stays put when the length is edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WallLock {
    Start,
    Center,
    End,
}

pub struct WallDialog {
    frame: SpecDialog,
    form: WallForm,
}

struct WallForm {
    target: WallTarget,
    draft: Wall,
    extras: WallExtras,
    /// The wall's openings as they were when the dialog opened.
    openings: Vec<Opening>,
    orig_start: Point,
    lock: WallLock,
    default_height: f64,
    default_top: bool,
    fields: Fields,
    /// The wall types offered in the Wall Types tab (`PlanDefaults::wall_types`).
    types: Vec<WallTypeDef>,
    /// The Wall Type Definitions dialog, while open.
    define: Option<WallTypeDialog>,
    /// Wall types edited or created in that dialog (to store with the plan
    /// and the defaults on OK).
    edited_types: Vec<WallTypeDef>,
}

impl WallDialog {
    /// `default_height` is the app's default wall height (for the Structure
    /// tab's "Default Wall Top Height" box).
    pub fn new(
        target: WallTarget,
        wall: Wall,
        openings: Vec<Opening>,
        extras: WallExtras,
        default_height: f64,
        types: Vec<WallTypeDef>,
    ) -> Self {
        let title = match target {
            WallTarget::Wall(_) => "Wall Specification",
            WallTarget::DefaultExterior => "Wall Specification (Exterior Wall Defaults)",
            WallTarget::DefaultInterior => "Wall Specification (Interior Wall Defaults)",
            WallTarget::DefaultFoundation => "Wall Specification (Foundation Wall Defaults)",
        };
        let default_top = (wall.height - default_height).abs() < 1e-6;
        Self {
            frame: SpecDialog::new(title, "wall"),
            form: WallForm {
                target,
                orig_start: wall.start,
                draft: wall,
                extras,
                openings,
                lock: WallLock::Start,
                default_height,
                default_top,
                fields: Fields::default(),
                types,
                define: None,
                edited_types: Vec::new(),
            },
        }
    }

    /// A dialog for one of the default wall settings; `type_name` is the
    /// wall type currently set in the defaults.
    pub fn for_default(
        target: WallTarget,
        thickness: f64,
        height: f64,
        mut extras: WallExtras,
        types: Vec<WallTypeDef>,
        type_name: &str,
    ) -> Self {
        extras.wall_type = Some(type_name.to_string());
        let kind = if target == WallTarget::DefaultInterior {
            WallKind::Interior
        } else {
            WallKind::Exterior
        };
        // A sample wall for the preview; length and angle are not editable.
        let wall = crate::editor::ops::make_wall(
            0,
            Point::ZERO,
            Point::new(144.0, 0.0),
            thickness,
            height,
            kind,
        );
        Self::new(target, wall, Vec::new(), extras, height, types)
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        // The definitions dialog is drawn first so it takes Esc and Enter.
        if let Some(mut d) = self.form.define.take() {
            match d.show(ctx) {
                Outcome::Open => self.form.define = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.form.apply_defined(&mut d),
            }
        }
        self.frame.show(ctx, &mut self.form)
    }

    /// Wall types edited or created through "Define...".
    pub fn edited_types(&self) -> &[WallTypeDef] {
        &self.form.edited_types
    }

    pub fn target(&self) -> WallTarget {
        self.form.target
    }

    pub fn draft(&self) -> &Wall {
        &self.form.draft
    }

    /// Tests edit the draft as the form's controls would.
    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut Wall {
        &mut self.form.draft
    }

    pub fn extras(&self) -> &WallExtras {
        &self.form.extras
    }

    /// The wall type picked in the Wall Types tab, if any.
    pub fn picked_type(&self) -> Option<&str> {
        self.form.extras.wall_type.as_deref()
    }

    /// The wall's openings with their offsets adjusted to the edited start
    /// point, so they stay where they were along the wall.
    pub fn adjusted_openings(&self) -> Vec<Opening> {
        self.form.adjusted_openings()
    }
}

impl WallForm {
    /// Takes the result of the Wall Type Definitions dialog.
    fn apply_defined(&mut self, d: &mut WallTypeDialog) {
        for t in d.changed_types() {
            match self.types.iter_mut().find(|x| x.name == t.name) {
                Some(slot) => *slot = t.clone(),
                None => self.types.push(t.clone()),
            }
            match self.edited_types.iter_mut().find(|x| x.name == t.name) {
                Some(slot) => *slot = t,
                None => self.edited_types.push(t),
            }
        }
        self.draft.resize_about = d.resize_about();
        if let Some(def) = d
            .selected_name()
            .and_then(|n| self.types.iter().find(|t| t.name == n))
        {
            self.draft.thickness = def.thickness();
            if !self.target.is_default() {
                self.draft.kind = def.kind;
            }
            self.draft.wall_type = Some(def.name.clone());
            self.extras.wall_type = Some(def.name.clone());
        }
    }

    fn adjusted_openings(&self) -> Vec<Opening> {
        let dir = self.draft.direction();
        let shift = self.orig_start.sub(self.draft.start).dot(dir);
        self.openings
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o.center_offset += shift;
                o
            })
            .collect()
    }

    /// Direction of the draft; +x for a degenerate wall.
    fn dir(&self) -> Point {
        let d = self.draft.direction();
        if d.length() < 0.5 {
            Point::new(1.0, 0.0)
        } else {
            d
        }
    }

    fn angle_deg(&self) -> f64 {
        let a = self.dir().angle().to_degrees().rem_euclid(360.0);
        let a = (a * 100.0).round() / 100.0;
        if a >= 360.0 {
            0.0
        } else {
            a
        }
    }

    /// Moves the end point (Start lock), start point (End lock) or both
    /// (Center lock) along the wall direction.
    fn set_length(&mut self, len: f64) {
        let len = len.max(1.0);
        let d = self.dir();
        let (s, e) = (self.draft.start, self.draft.end);
        match self.lock {
            WallLock::Start => self.draft.end = s.add(d.scale(len)),
            WallLock::End => self.draft.start = e.sub(d.scale(len)),
            WallLock::Center => {
                let mid = Point::lerp(s, e, 0.5);
                self.draft.start = mid.sub(d.scale(len * 0.5));
                self.draft.end = mid.add(d.scale(len * 0.5));
            }
        }
    }

    /// Rotates the wall about its start point, keeping the length.
    fn set_angle(&mut self, deg: f64) {
        let a = deg.to_radians();
        let len = self.draft.length().max(1.0);
        self.draft.end = self
            .draft
            .start
            .add(Point::new(a.cos(), a.sin()).scale(len));
    }

    /// The type matching the draft: the one picked last if it still fits,
    /// else the first with the same kind and thickness.
    fn current_type(&self) -> Option<usize> {
        let fits = |t: &WallTypeDef| (t.thickness() - self.draft.thickness).abs() < 1e-6;
        let picked = self.extras.wall_type.as_deref().and_then(|n| {
            self.types
                .iter()
                .position(|t| t.name == n)
                .filter(|i| fits(&self.types[*i]))
        });
        picked.or_else(|| {
            self.types
                .iter()
                .position(|t| fits(t) && t.kind == self.draft.kind)
        })
    }

    /// Layers to draw for the draft: the type's, or a generic stack.
    fn layers(&self) -> Vec<(&str, f64)> {
        if let Some(i) = self.current_type() {
            return self.types[i]
                .layers
                .iter()
                .map(|l| (l.name.as_str(), l.thickness))
                .collect();
        }
        let t = self.draft.thickness.max(0.1);
        if t >= 1.5 {
            vec![("Drywall", 0.5), ("Framing", t - 1.0), ("Drywall", 0.5)]
        } else {
            vec![("Drywall", t)]
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        let is_default = self.target.is_default();
        section(ui, "General");
        ui.horizontal_wrapped(|ui| {
            for l in [
                "Foundation Wall",
                "Railing",
                "Terrain Retaining Wall",
                "Attic Wall",
            ] {
                dis_check(ui, l, false);
            }
        });
        self.fields
            .length_row(ui, "Thickness", "thickness", &mut self.draft.thickness);
        ui.add_enabled_ui(!is_default, |ui| {
            let mut len = self.draft.length();
            if self
                .fields
                .length_row(ui, "Wall Length", "length", &mut len)
            {
                self.set_length(len);
            }
            let mut angle = self.angle_deg();
            if self
                .fields
                .degrees_row(ui, "Wall Angle", "deg_angle", &mut angle)
            {
                self.set_angle(angle);
            }
            let tip = "The point that stays fixed when the wall length changes";
            row(ui, "Lock", |ui| {
                ui.radio_value(&mut self.lock, WallLock::Start, "Start")
                    .on_hover_text(tip);
                ui.radio_value(&mut self.lock, WallLock::Center, "Center")
                    .on_hover_text(tip);
                ui.radio_value(&mut self.lock, WallLock::End, "End")
                    .on_hover_text(tip);
            });
        });
        if !is_default {
            ui.weak("The angle rotates about the start point. Connected walls are not moved.");
        }

        section(ui, "Options");
        if is_default {
            for l in ["Invisible", "No Room Definition", "No Locate"] {
                dis_check(ui, l, false);
            }
        } else {
            let f = &mut self.draft.flags;
            ui.checkbox(&mut f.invisible, "Invisible");
            ui.checkbox(&mut f.no_room_definition, "No Room Definition");
            ui.checkbox(&mut f.no_locate, "No Locate");
        }
        for l in [
            "Lock Center",
            "No Room Moldings Exterior",
            "No Room Moldings Interior",
            "Automatically Generated Wall",
            "Ignored by Hide Exterior Walls",
        ] {
            dis_check(ui, l, false);
        }

        ui.add_enabled_ui(false, |ui| {
            section(ui, "Curved Wall");
            row(ui, "Radius to", |ui| {
                dis_radio(ui, "Outer Surface", false);
                dis_radio(ui, "Main Layer Outside", true);
            });
            row(ui, "Lock", |ui| {
                dis_radio(ui, "Arc Center", false);
                dis_radio(ui, "Ends", true);
            });
            dis_check(ui, "Automatic Facet Angle", true);
        });
    }

    fn structure(&mut self, ui: &mut Ui) {
        section(ui, "Default Wall Heights");
        if ui
            .checkbox(&mut self.default_top, "Default Wall Top Height")
            .changed()
            && self.default_top
        {
            self.draft.height = self.default_height;
        }
        ui.add_enabled_ui(!self.default_top, |ui| {
            self.fields
                .length_row(ui, "Wall Height", "height", &mut self.draft.height);
        });
        dis_check(ui, "Default Wall Bottom Height", true);

        ui.add_enabled_ui(false, |ui| {
            section(ui, "Platform Intersections");
            dis_check(
                ui,
                "Invisible Walls and Railings: Generate Between Platforms",
                true,
            );
            row(ui, "Ceiling Platform", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Stop at Ceiling Above", false);
            });
            row(ui, "Floor Platform", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Stop at Floor Below", false);
            });
            section(ui, "Wall Intersections");
            dis_check(ui, "Through Wall At End", false);
            dis_check(ui, "Through Wall At Start", false);
            section(ui, "Rim Joist");
            row(ui, "Rim Joist", |ui| {
                dis_radio(ui, "Automatic", true);
                dis_radio(ui, "Double", false);
                dis_radio(ui, "Single", false);
            });
            section(ui, "Double Wall");
            row(ui, "Double Wall", |ui| {
                dis_radio(ui, "Furred Wall", false);
                dis_radio(ui, "Split Framing", false);
                dis_radio(ui, "Frame Through", true);
            });
            section(ui, "Stud Layout");
            dis_check(ui, "Use Framing Reference", true);
            dis_check(ui, "Reverse Stud Rollout Direction", false);
            section(ui, "Framing");
            dis_check(ui, "Retain Wall Framing", false);
            dis_check(ui, "Bearing Wall", false);
            dis_check(ui, "Stagger Multiple Framing Layers", true);
            dis_check(ui, "Create Wall/Footing Below", false);
            dis_check(ui, "Insert Floor Framing Below", true);
        });
    }

    fn wall_types(&mut self, ui: &mut Ui) {
        section(ui, "General");
        let current = self.current_type();
        let default_kind = self.target.is_default().then_some(self.draft.kind);
        row(ui, "Wall Type", |ui| {
            let text = match current {
                Some(i) => format!(
                    "{} ({})",
                    self.types[i].name,
                    super::fmt_short(self.types[i].thickness())
                ),
                None => format!("Custom ({})", super::fmt_short(self.draft.thickness)),
            };
            egui::ComboBox::from_id_salt("wall_type")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for (i, t) in self.types.iter().enumerate() {
                        // A default dialog keeps its own wall kind.
                        if default_kind.is_some_and(|k| k != t.kind) {
                            continue;
                        }
                        let label = format!("{} ({})", t.name, super::fmt_short(t.thickness()));
                        if ui.selectable_label(current == Some(i), label).clicked() {
                            self.draft.thickness = t.thickness();
                            self.draft.kind = t.kind;
                            self.draft.wall_type = Some(t.name.clone());
                            self.extras.wall_type = Some(t.name.clone());
                        }
                    }
                });
            if ui.button("Define\u{2026}").clicked() && self.define.is_none() {
                let current = self
                    .current_type()
                    .and_then(|i| self.types.get(i))
                    .map(|t| t.name.clone());
                self.define = Some(WallTypeDialog::new(
                    self.types.clone(),
                    current.as_deref(),
                    self.draft.resize_about,
                ));
            }
            ui.add_enabled(false, egui::Button::new("Library\u{2026}"));
        });
        if let Some(t) = current.map(|i| &self.types[i]) {
            ui.weak(format!(
                "Main layer starts {} in from the exterior face",
                super::fmt_short(t.main_layer_offset())
            ));
        }
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(420.0), 120.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, super::PV_BG);
        layer_stack(&painter, rect.shrink(8.0), &self.layers());

        ui.add_enabled_ui(false, |ui| {
            section(ui, "Pony Wall");
            dis_check(ui, "Pony Wall", false);
            row(ui, "Lower Wall Type", |ui| {
                dis_combo(ui, "pony_type", "stone-6")
            });
            row(ui, "Elevation of Lower Wall Top", |ui| {
                ui.label("2'-0\"");
            });
        });
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            dis_check(ui, "Default", true);
            egui::ComboBox::from_id_salt("wall_layer")
                .selected_text(self.draft.layer.clone())
                .show_ui(ui, |ui| {
                    for name in WALL_LAYERS {
                        ui.selectable_value(&mut self.draft.layer, name.to_string(), name);
                    }
                });
        });
        row(ui, "Drawing Group", |ui| {
            dis_combo(ui, "wall_group", "Default: 29 \u{2013} Wall")
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Display Options");
        let e = &mut self.extras;
        session_check(ui, &mut e.suppress_label, "Suppress Label in All Views");
        session_check(ui, &mut e.display_in_plan, "Display in Plan View");
        section(ui, "Label Content");
        ui.radio_value(&mut e.specify_label, false, "Automatic Label")
            .on_hover_text(SESSION_NOTE);
        ui.radio_value(&mut e.specify_label, true, "Specify Label")
            .on_hover_text(SESSION_NOTE);
        ui.add_enabled(
            e.specify_label,
            egui::TextEdit::singleline(&mut e.label_text).desired_width(260.0),
        );
        if !e.specify_label {
            let name = self
                .current_type()
                .map_or("Custom", |i| self.types[i].name.as_str());
            ui.weak(format!(
                "Automatic label: Wall \u{2013} {name} \u{2013} {}",
                fmt_ft_in(self.draft.length())
            ));
        }
        ui.add_enabled_ui(false, |ui| {
            section(ui, "Appearance");
            dis_check(ui, "Display Border", false);
            row(ui, "Text Style", |ui| {
                dis_combo(ui, "wall_text_style", "Use Layer Text Style")
            });
            row(ui, "Alignment", |ui| dis_combo(ui, "wall_align", "Left"));
            dis_check(ui, "Auto Adjust Text Direction", true);
            section(ui, "Label Layer");
            dis_radio(ui, "Use System Layer (Walls, Labels)", true);
            dis_radio(ui, "Use Object Layer (Walls, Normal)", false);
        });
    }
}

impl SpecPages for WallForm {
    fn tabs(&self) -> &'static [Tab] {
        WALL_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.draft.thickness <= 0.0 {
            return Some("Thickness must be greater than zero".into());
        }
        if self.draft.height <= 0.0 {
            return Some("Wall height must be greater than zero".into());
        }
        let len = self.draft.length();
        let too_short = self.adjusted_openings().iter().any(|o| {
            o.start_offset() < OPENING_MARGIN - 1e-6 || o.end_offset() > len - OPENING_MARGIN + 1e-6
        });
        too_short.then(|| "Wall is too short for its openings".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match WALL_TABS[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Wall Types" => self.wall_types(ui),
            "Layer" => self.layer(ui),
            "Label" => self.label(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let top = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y - 4.0));
        let bottom = Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y + 4.0), rect.max);
        pv_text(
            p,
            top.min + egui::vec2(0.0, 4.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        let plan = Rect::from_min_max(top.min + egui::vec2(0.0, 10.0), top.max);
        wall_plan_sketch(
            p,
            plan,
            self.draft.length(),
            self.draft.thickness,
            &self.adjusted_openings(),
            None,
            90.0,
        );
        p.hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(0.8_f32, PV_FAINT),
        );
        pv_text(
            p,
            bottom.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Section",
            11.0,
        );
        layer_stack(
            p,
            Rect::from_min_max(bottom.min + egui::vec2(0.0, 12.0), bottom.max),
            &self.layers(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(lock: WallLock) -> WallForm {
        let wall = crate::editor::ops::make_wall(
            1,
            Point::new(10.0, 10.0),
            Point::new(110.0, 10.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut opening = Opening::default_door(2, 1, 50.0);
        opening.width = 30.0;
        WallForm {
            target: WallTarget::Wall(1),
            orig_start: wall.start,
            draft: wall,
            extras: WallExtras::default(),
            openings: vec![opening],
            lock,
            default_height: 109.125,
            default_top: true,
            fields: Fields::default(),
            types: plan_core::PlanDefaults::chief_x18_daniel().wall_types,
            define: None,
            edited_types: Vec::new(),
        }
    }

    #[test]
    fn length_lock_start_moves_end() {
        let mut f = form(WallLock::Start);
        f.set_length(150.0);
        assert!((f.draft.start.x - 10.0).abs() < 1e-9);
        assert!((f.draft.end.x - 160.0).abs() < 1e-9);
        assert!((f.adjusted_openings()[0].center_offset - 50.0).abs() < 1e-9);
    }

    #[test]
    fn length_lock_end_keeps_openings_in_place() {
        let mut f = form(WallLock::End);
        f.set_length(80.0);
        assert!((f.draft.end.x - 110.0).abs() < 1e-9);
        assert!((f.draft.start.x - 30.0).abs() < 1e-9);
        // The door was 50" from the old start (x = 60) and is now 30" from x = 30.
        assert!((f.adjusted_openings()[0].center_offset - 30.0).abs() < 1e-9);
        assert!(f.error().is_none());
        f.set_length(40.0);
        assert!(f.error().is_some());
    }

    #[test]
    fn angle_rotates_about_start() {
        let mut f = form(WallLock::Start);
        f.set_angle(90.0);
        assert!((f.draft.start.x - 10.0).abs() < 1e-9);
        assert!((f.draft.end.x - 10.0).abs() < 1e-6);
        assert!((f.draft.end.y - 110.0).abs() < 1e-6);
        assert!((f.angle_deg() - 90.0).abs() < 1e-9);
    }
}
