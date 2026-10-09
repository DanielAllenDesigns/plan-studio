//! The Moldings / Profiles / Rails panel (Reference Manual pp. 970 to 974),
//! shared by every dialog that has a Moldings panel: the Molding
//! Specification today, the room, Floor Defaults, cabinet, tray ceiling, wall
//! cap and soffit dialogs through the hook in the integration queue.
//!
//! The panel edits one [`MoldingTable`]. It has the Use Floor Default / No
//! Change choice (when the owner asks for it), the table of profiles with
//! Width, Height, Repeat Distance and the offsets, the buttons (Add New,
//! Make Copy, Edit, Replace, Default, Delete, Make Stack, Explode Stack,
//! Move Up, Move Down) and the Selected Profile Options (Vertical Position,
//! Type, Profile Rotation, Reflect Horizontal / Vertical, Retain Aspect
//! Ratio, Auto Offset, Full Wall Width, Split Profile, On Selected Edge,
//! Texture Up Direction, Count Components) with a preview of the profile.
//!
//! The buttons are [`PanelAction`]s that [`MoldingPanel::apply`] runs on the
//! table, so a test (or another dialog) can press them without a frame.

use crate::dialogs::{
    row, section, Fields, ERROR_RED, PV_ACCENT, PV_BG, PV_FAINT, PV_INK, PV_WALL,
};
use eframe::egui::{self, Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use plan_core::geometry::Point;
use plan_core::moldings::{
    EdgeMode, MoldingEntry, MoldingTable, MoldingType, ProfileDef, TableSource,
};

/// What the owner of the panel allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelOptions {
    /// Use Floor Default / No Change / own moldings (rooms and floors).
    pub source: bool,
    /// On Selected Edge: Automatic / On / Off.
    pub edge: bool,
    /// Full Wall Width and Split Profile (rails and wall caps).
    pub rails: bool,
    /// Repeat Distance of 3D moldings.
    pub repeat: bool,
    /// Auto Offset and Vertical Position of the stack.
    pub vertical: bool,
}

impl Default for PanelOptions {
    fn default() -> Self {
        PanelOptions {
            source: false,
            edge: true,
            rails: false,
            repeat: true,
            vertical: true,
        }
    }
}

impl PanelOptions {
    /// The panel of a room or the Floor Defaults.
    pub fn room() -> Self {
        PanelOptions {
            source: true,
            ..PanelOptions::default()
        }
    }

    /// The panel of a wall cap or a rail (for the dialogs that embed the
    /// panel later; see docs/integration-queue.md).
    #[allow(dead_code)]
    pub fn rails() -> Self {
        PanelOptions {
            rails: true,
            repeat: false,
            edge: false,
            ..PanelOptions::default()
        }
    }
}

/// A button of the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelAction {
    /// Add New with the library profile of this name.
    AddNew(ProfileDef),
    MakeCopy,
    /// Edit: the owner opens the profile (Edit Molding Profile).
    #[allow(dead_code)]
    Edit,
    /// Replace the selected row's profile.
    Replace(ProfileDef),
    Default,
    Delete,
    MakeStack,
    ExplodeStack,
    MoveUp,
    MoveDown,
}

/// What a frame of the panel did that the owner must know.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PanelEvent {
    /// The table changed.
    pub changed: bool,
    /// The Edit button was pressed: the name of the profile to open.
    pub edit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picking {
    Add,
    Replace,
}

/// State of one panel.
pub struct MoldingPanel {
    selected: usize,
    marked: Vec<usize>,
    picking: Option<Picking>,
    filter: String,
    fields: Fields,
    pub options: PanelOptions,
    /// The profiles Add New and Replace offer (built-in and the plan's own).
    pub catalog: Vec<ProfileDef>,
    /// Message under the buttons.
    pub note: String,
}

impl MoldingPanel {
    pub fn new(options: PanelOptions, catalog: Vec<ProfileDef>) -> Self {
        Self {
            selected: 0,
            marked: Vec::new(),
            picking: None,
            filter: String::new(),
            fields: Fields::default(),
            options,
            catalog,
            note: String::new(),
        }
    }

    /// The row the Selected Profile Options show.
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[allow(dead_code)]
    pub fn select(&mut self, i: usize) {
        self.selected = i;
    }

    /// Mark rows for Make Stack (the selected row counts as marked).
    pub fn mark(&mut self, i: usize, on: bool) {
        self.marked.retain(|m| *m != i);
        if on {
            self.marked.push(i);
        }
    }

    pub fn any_invalid(&self) -> bool {
        self.fields.any_invalid()
    }

    /// Runs a button on `table`; returns whether the table changed.
    pub fn apply(&mut self, table: &mut MoldingTable, action: PanelAction) -> bool {
        self.note.clear();
        let sel = self.selected.min(table.len().saturating_sub(1));
        let changed = match action {
            PanelAction::AddNew(p) => {
                if !p.is_valid() {
                    self.note = "That profile has no usable section".into();
                    return false;
                }
                self.selected = table.add_new(p);
                true
            }
            PanelAction::MakeCopy => match table.make_copy(sel) {
                Some(i) => {
                    self.selected = i;
                    true
                }
                None => false,
            },
            PanelAction::Edit => false,
            PanelAction::Replace(p) => {
                if !p.is_valid() {
                    self.note = "That profile has no usable section".into();
                    return false;
                }
                table.replace(sel, p)
            }
            PanelAction::Default => table.set_default(sel),
            PanelAction::Delete => {
                let done = table.delete(sel);
                if done {
                    self.marked.clear();
                    self.selected = sel.saturating_sub(usize::from(sel >= table.len()));
                }
                done
            }
            PanelAction::MakeStack => {
                let mut picks = self.marked.clone();
                if !picks.contains(&sel) {
                    picks.push(sel);
                }
                match table.make_stack(&picks) {
                    Some(_) => {
                        self.marked.clear();
                        // The stack was gathered at its first row.
                        self.selected = picks.iter().copied().min().unwrap_or(sel);
                        true
                    }
                    None => {
                        self.note = "Mark two or more profiles to stack".into();
                        false
                    }
                }
            }
            PanelAction::ExplodeStack => table.explode_stack(sel),
            PanelAction::MoveUp => {
                let done = table.move_up(sel);
                if done {
                    self.selected = sel - 1;
                }
                done
            }
            PanelAction::MoveDown => {
                let done = table.move_down(sel);
                if done {
                    self.selected = sel + 1;
                }
                done
            }
        };
        self.selected = self.selected.min(table.len().saturating_sub(1));
        changed
    }

    /// Draws the panel; returns what the owner must know.
    pub fn show(&mut self, ui: &mut Ui, table: &mut MoldingTable) -> PanelEvent {
        let mut ev = PanelEvent::default();
        self.selected = self.selected.min(table.len().saturating_sub(1));
        if self.options.source {
            section(ui, "Moldings");
            row(ui, "Use", |ui| {
                for s in [
                    TableSource::UseFloorDefault,
                    TableSource::Own,
                    TableSource::NoChange,
                ] {
                    let label = match s {
                        TableSource::Own => "These moldings",
                        other => other.name(),
                    };
                    if ui.radio_value(&mut table.source, s, label).changed() {
                        ev.changed = true;
                    }
                }
            });
            if table.source != TableSource::Own {
                ui.weak(match table.source {
                    TableSource::UseFloorDefault => {
                        "The moldings of the Floor Defaults apply; the table below is kept."
                    }
                    _ => "The moldings stay as they are on each selected object.",
                });
                if table.source == TableSource::NoChange {
                    return ev;
                }
            }
        }
        section(ui, "Profiles");
        self.table_grid(ui, table, &mut ev);
        ui.add_space(4.0);
        self.buttons(ui, table, &mut ev);
        if !self.note.is_empty() {
            ui.colored_label(ERROR_RED, self.note.clone());
        }
        if self.picking.is_some() {
            self.picker(ui, table, &mut ev);
        }
        if let Some(entry) = table.rows.get_mut(self.selected) {
            section(ui, "Selected Profile Options");
            let mut changed = false;
            self.options_for(ui, entry, &mut changed);
            ev.changed |= changed;
            ui.add_space(4.0);
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width().min(260.0), 110.0),
                Sense::hover(),
            );
            if ui.is_rect_visible(rect) {
                preview(&ui.painter_at(rect), rect, table, self.selected);
            }
        } else {
            ui.weak("No profiles yet. Add New picks one from the library.");
        }
        ev
    }

    fn table_grid(&mut self, ui: &mut Ui, table: &mut MoldingTable, ev: &mut PanelEvent) {
        let mut picked = None;
        let mut marks: Vec<(usize, bool)> = Vec::new();
        egui::Grid::new("molding_panel_table")
            .striped(true)
            .num_columns(8)
            .show(ui, |ui| {
                ui.strong("");
                ui.strong("Profile");
                ui.strong("Width");
                ui.strong("Height");
                if self.options.repeat {
                    ui.strong("Repeat");
                }
                ui.strong("H Offset");
                ui.strong("V Offset");
                ui.strong("Stack");
                ui.end_row();
                for (i, r) in table.rows.iter().enumerate() {
                    let mut mark = self.marked.contains(&i);
                    if ui
                        .checkbox(&mut mark, "")
                        .on_hover_text("Mark for Make Stack")
                        .changed()
                    {
                        marks.push((i, mark));
                    }
                    if ui
                        .selectable_label(i == self.selected, &r.profile.name)
                        .clicked()
                    {
                        picked = Some(i);
                    }
                    ui.label(plan_core::units::fmt_ft_in(r.width));
                    ui.label(plan_core::units::fmt_ft_in(r.height));
                    if self.options.repeat {
                        ui.label(if r.repeat_distance > 0.0 {
                            plan_core::units::fmt_ft_in(r.repeat_distance)
                        } else {
                            "-".into()
                        });
                    }
                    ui.label(plan_core::units::fmt_ft_in(r.h_offset));
                    ui.label(plan_core::units::fmt_ft_in(r.v_offset));
                    ui.label(if r.stack == 0 {
                        "-".to_string()
                    } else {
                        r.stack.to_string()
                    });
                    ui.end_row();
                }
            });
        for (i, on) in marks {
            self.mark(i, on);
        }
        if let Some(i) = picked {
            self.selected = i;
        }
        let _ = ev;
    }

    fn buttons(&mut self, ui: &mut Ui, table: &mut MoldingTable, ev: &mut PanelEvent) {
        let has = !table.is_empty();
        let stacked = table.rows.get(self.selected).is_some_and(|r| r.stack != 0);
        let mut act: Option<PanelAction> = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add New\u{2026}").clicked() {
                self.picking = Some(Picking::Add);
            }
            if ui
                .add_enabled(has, egui::Button::new("Make Copy"))
                .clicked()
            {
                act = Some(PanelAction::MakeCopy);
            }
            if ui.add_enabled(has, egui::Button::new("Edit")).clicked() {
                if let Some(r) = table.rows.get(self.selected) {
                    ev.edit = Some(r.profile.name.clone());
                }
            }
            if ui
                .add_enabled(has, egui::Button::new("Replace\u{2026}"))
                .clicked()
            {
                self.picking = Some(Picking::Replace);
            }
            if ui.add_enabled(has, egui::Button::new("Default")).clicked() {
                act = Some(PanelAction::Default);
            }
            if ui.add_enabled(has, egui::Button::new("Delete")).clicked() {
                act = Some(PanelAction::Delete);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(table.len() > 1, egui::Button::new("Make Stack"))
                .clicked()
            {
                act = Some(PanelAction::MakeStack);
            }
            if ui
                .add_enabled(stacked, egui::Button::new("Explode Stack"))
                .clicked()
            {
                act = Some(PanelAction::ExplodeStack);
            }
            if ui
                .add_enabled(self.selected > 0, egui::Button::new("Move Up"))
                .clicked()
            {
                act = Some(PanelAction::MoveUp);
            }
            if ui
                .add_enabled(
                    self.selected + 1 < table.len(),
                    egui::Button::new("Move Down"),
                )
                .clicked()
            {
                act = Some(PanelAction::MoveDown);
            }
        });
        if let Some(a) = act {
            ev.changed |= self.apply(table, a);
        }
    }

    /// Select Library Object: the profiles to pick from.
    fn picker(&mut self, ui: &mut Ui, table: &mut MoldingTable, ev: &mut PanelEvent) {
        let mode = self.picking;
        let mut chosen: Option<ProfileDef> = None;
        let mut close = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(match mode {
                    Some(Picking::Replace) => "Replace with",
                    _ => "Select Library Object",
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .hint_text("search")
                        .desired_width(120.0),
                );
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
            let f = self.filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("molding_picker")
                .show(ui, |ui| {
                    for p in self
                        .catalog
                        .iter()
                        .filter(|p| f.is_empty() || p.name.to_lowercase().contains(&f))
                    {
                        let label = format!(
                            "{}  ({}, {})",
                            p.name,
                            p.kind.name(),
                            plan_core::units::fmt_ft_in(p.height())
                        );
                        if ui.selectable_label(false, label).clicked() {
                            chosen = Some(p.clone());
                        }
                    }
                });
        });
        if let Some(p) = chosen {
            let action = match mode {
                Some(Picking::Replace) => PanelAction::Replace(p),
                _ => PanelAction::AddNew(p),
            };
            ev.changed |= self.apply(table, action);
            close = true;
        }
        if close {
            self.picking = None;
        }
    }

    fn options_for(&mut self, ui: &mut Ui, e: &mut MoldingEntry, changed: &mut bool) {
        let f = &mut self.fields;
        let mut w = e.width;
        if f.length_row(ui, "Width", "mp_width", &mut w) {
            e.set_width(w);
            *changed = true;
        }
        let mut h = e.height;
        if f.length_row(ui, "Height", "mp_height", &mut h) {
            e.set_height(h);
            *changed = true;
        }
        row(ui, "Aspect", |ui| {
            *changed |= ui
                .checkbox(&mut e.retain_aspect, "Retain Aspect Ratio")
                .changed();
        });
        if self.options.repeat {
            *changed |= f.length_row(ui, "Repeat Distance", "mp_repeat", &mut e.repeat_distance);
            if e.repeat_distance > 0.0 {
                *changed |= f.length_row(ui, "Symbol Length", "mp_element", &mut e.element_length);
            }
        }
        *changed |= f.length_row(ui, "Horizontal Offset", "mp_hoff", &mut e.h_offset);
        *changed |= f.length_row(ui, "Vertical Offset", "mp_voff", &mut e.v_offset);
        if self.options.vertical {
            row(ui, "Auto Offset", |ui| {
                *changed |= ui
                    .checkbox(&mut e.auto_offset, "Stacked profiles sit on the one before")
                    .changed();
            });
            let mut own = e.vertical_position.is_some();
            row(ui, "Vertical Position", |ui| {
                if ui.checkbox(&mut own, "Set").changed() {
                    e.vertical_position = own.then_some(0.0);
                    *changed = true;
                }
            });
            if let Some(v) = &mut e.vertical_position {
                *changed |= f.length_row(ui, "Bottom From Floor", "mp_vpos", v);
            }
        }
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("molding_panel_type")
                .selected_text(e.kind.name())
                .show_ui(ui, |ui| {
                    for t in MoldingType::ALL {
                        *changed |= ui.selectable_value(&mut e.kind, t, t.name()).changed();
                    }
                });
        });
        *changed |= f.degrees_row(ui, "Profile Rotation", "deg_mp_rotation", &mut e.rotation);
        row(ui, "Reflect", |ui| {
            *changed |= ui.checkbox(&mut e.reflect_h, "Horizontal").changed();
            *changed |= ui.checkbox(&mut e.reflect_v, "Vertical").changed();
        });
        if self.options.rails {
            row(ui, "Rails", |ui| {
                *changed |= ui
                    .checkbox(&mut e.full_wall_width, "Full Wall Width")
                    .changed();
                *changed |= ui.checkbox(&mut e.split_profile, "Split Profile").changed();
            });
        }
        if self.options.edge {
            row(ui, "On Selected Edge", |ui| {
                for m in EdgeMode::ALL {
                    *changed |= ui.radio_value(&mut e.edge, m, m.name()).changed();
                }
            });
        }
        row(ui, "Texture", |ui| {
            *changed |= ui
                .checkbox(&mut e.texture_up, "Texture Up Direction")
                .changed();
            *changed |= ui
                .checkbox(&mut e.count_components, "Count Components")
                .changed();
        });
        if e.profile.is_stack() {
            ui.weak("Stacked profile: one material for each part");
            for (k, part) in e.profile.parts.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Part {}", k + 1));
                    *changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut part.material)
                                .hint_text("material of the molding")
                                .desired_width(160.0),
                        )
                        .changed();
                });
            }
        }
    }
}

/// Draws the rows of `table` as they stack up: the selected profile in
/// color, the others faint, the wall as a gray strip at the left.
pub fn preview(p: &egui::Painter, rect: Rect, table: &MoldingTable, selected: usize) {
    p.rect_filled(rect, 0.0, PV_BG);
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, PV_FAINT),
        StrokeKind::Inside,
    );
    let resolved = table.resolve_from(0.0);
    let mut lo = Point::new(f64::MAX, f64::MAX);
    let mut hi = Point::new(f64::MIN, f64::MIN);
    for r in &resolved {
        for part in &r.parts {
            for q in &part.section {
                let q = *q + Point::new(0.0, r.bottom);
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
        }
    }
    if lo.x > hi.x {
        return;
    }
    lo = Point::new(lo.x.min(0.0), lo.y);
    let (w, h) = ((hi.x - lo.x).max(0.5), (hi.y - lo.y).max(0.5));
    let area = rect.shrink(10.0);
    let k = (f64::from(area.width()) / w).min(f64::from(area.height()) / h) as f32;
    let to = |q: Point| {
        Pos2::new(
            area.min.x + (q.x - lo.x) as f32 * k,
            area.max.y - (q.y - lo.y) as f32 * k,
        )
    };
    // The wall: the back of the molding is the vertical line at x = 0.
    let wall = Rect::from_min_max(
        Pos2::new(area.min.x - 8.0, rect.min.y + 2.0),
        Pos2::new(to(Point::new(0.0, 0.0)).x, rect.max.y - 2.0),
    );
    p.rect_filled(wall, 0.0, PV_WALL);
    for r in &resolved {
        let on = r.index == selected;
        for part in &r.parts {
            let pts: Vec<Pos2> = part
                .section
                .iter()
                .map(|q| to(*q + Point::new(0.0, r.bottom)))
                .collect();
            if pts.len() < 3 {
                continue;
            }
            let fill = if on {
                PV_ACCENT.gamma_multiply(0.45)
            } else {
                PV_WALL
            };
            p.add(egui::Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
            let mut ring = pts;
            ring.push(ring[0]);
            p.add(egui::Shape::line(
                ring,
                Stroke::new(
                    if on { 1.8_f32 } else { 1.0 },
                    if on { PV_INK } else { PV_FAINT },
                ),
            ));
        }
    }
    p.text(
        Pos2::new(rect.max.x - 4.0, rect.min.y + 4.0),
        Align2::RIGHT_TOP,
        "wall at left",
        egui::FontId::proportional(9.0),
        PV_FAINT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::moldings::{builtin_profiles, square_profile};

    fn frame(panel: &mut MoldingPanel, table: &mut MoldingTable) -> PanelEvent {
        let ctx = egui::Context::default();
        let mut ev = PanelEvent::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ev = panel.show(ui, table);
            });
        });
        ev
    }

    fn panel() -> MoldingPanel {
        MoldingPanel::new(PanelOptions::room(), builtin_profiles())
    }

    #[test]
    fn the_buttons_edit_the_table() {
        let mut p = panel();
        let mut t = MoldingTable::default();
        let crown = builtin_profiles()
            .into_iter()
            .find(|x| x.kind == MoldingType::Crown)
            .unwrap();
        assert!(p.apply(&mut t, PanelAction::AddNew(square_profile())));
        assert!(p.apply(&mut t, PanelAction::AddNew(crown.clone())));
        assert_eq!(t.len(), 2);
        assert_eq!(p.selected(), 1);
        assert!(p.apply(&mut t, PanelAction::MakeCopy));
        assert_eq!(t.len(), 3);
        assert_eq!(p.selected(), 2);
        assert!(p.apply(&mut t, PanelAction::MoveUp));
        assert_eq!(p.selected(), 1);
        assert!(p.apply(&mut t, PanelAction::MoveDown));
        // Mark the first row and stack with the selected one.
        p.mark(0, true);
        assert!(p.apply(&mut t, PanelAction::MakeStack));
        assert!(t.rows[0].stack != 0 && t.rows[0].stack == t.rows[1].stack);
        assert!(p.apply(&mut t, PanelAction::ExplodeStack));
        assert_eq!(t.rows[0].stack, 0);
        // Make Stack with nothing marked says so.
        p.mark(0, false);
        assert!(!p.apply(&mut t, PanelAction::MakeStack));
        assert!(!p.note.is_empty());
        // Replace and Default.
        t.rows[0].h_offset = 1.0;
        p.select(0);
        assert!(p.apply(&mut t, PanelAction::Replace(crown)));
        assert_eq!(t.rows[0].h_offset, 1.0);
        assert!(p.apply(&mut t, PanelAction::Default));
        assert_eq!(t.rows[0].h_offset, 0.0);
        // Delete leaves a sane selection.
        p.select(2);
        assert!(p.apply(&mut t, PanelAction::Delete));
        assert_eq!(t.len(), 2);
        assert!(p.selected() < 2);
        // A profile with no section is refused.
        assert!(!p.apply(&mut t, PanelAction::AddNew(ProfileDef::default())));
    }

    #[test]
    fn a_frame_draws_every_state_without_panicking() {
        let mut p = panel();
        let mut t = MoldingTable::default();
        // Empty table, then rows, then a stack, then No Change.
        frame(&mut p, &mut t);
        p.apply(&mut t, PanelAction::AddNew(square_profile()));
        p.apply(&mut t, PanelAction::AddNew(builtin_profiles()[0].clone()));
        frame(&mut p, &mut t);
        p.mark(0, true);
        p.apply(&mut t, PanelAction::MakeStack);
        t.rows[0].repeat_distance = 12.0;
        let ev = frame(&mut p, &mut t);
        assert!(!ev.changed);
        t.source = TableSource::NoChange;
        frame(&mut p, &mut t);
        t.source = TableSource::UseFloorDefault;
        frame(&mut p, &mut t);
        let mut rails = MoldingPanel::new(PanelOptions::rails(), builtin_profiles());
        t.source = TableSource::Own;
        frame(&mut rails, &mut t);
        assert!(!p.any_invalid());
    }
}
