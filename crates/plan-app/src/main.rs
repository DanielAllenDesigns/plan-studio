//! Plan Studio: a 2D floor-plan editor on top of `plan-core`.
//!
//! World units are inches with Y up; the camera flips Y when mapping to screen.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::units::fmt_ft_in;
use plan_core::{
    detect_rooms, Floor, Id, OpeningKind, Project, Room, Wall, WallKind, DEFAULT_CEILING_HEIGHT,
    DEFAULT_EXTERIOR_THICKNESS, DEFAULT_INTERIOR_THICKNESS,
};
use std::path::PathBuf;

const PICK_RADIUS_PX: f64 = 10.0;
const BACKGROUND: Color32 = Color32::from_rgb(250, 250, 247);
const SELECT_COLOR: Color32 = Color32::from_rgb(255, 140, 0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Wall,
    Door,
    Window,
}

/// Maps between world inches (Y up) and screen pixels (Y down).
#[derive(Clone, Copy)]
struct Camera {
    /// World point shown at the viewport center.
    center: Point,
    px_per_in: f64,
}

impl Camera {
    fn world_to_screen(&self, rect: Rect, p: Point) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.px_per_in) as f32,
            c.y - ((p.y - self.center.y) * self.px_per_in) as f32,
        )
    }

    fn screen_to_world(&self, rect: Rect, s: Pos2) -> Point {
        let c = rect.center();
        Point::new(
            self.center.x + (s.x - c.x) as f64 / self.px_per_in,
            self.center.y - (s.y - c.y) as f64 / self.px_per_in,
        )
    }

    /// Zoom by `factor` keeping the world point under screen position `at` fixed.
    fn zoom_about(&mut self, rect: Rect, at: Pos2, factor: f64) {
        let anchor = self.screen_to_world(rect, at);
        self.px_per_in = (self.px_per_in * factor).clamp(0.05, 50.0);
        let c = rect.center();
        self.center = Point::new(
            anchor.x - (at.x - c.x) as f64 / self.px_per_in,
            anchor.y + (at.y - c.y) as f64 / self.px_per_in,
        );
    }

    fn pan_by_pixels(&mut self, d: Vec2) {
        self.center.x -= d.x as f64 / self.px_per_in;
        self.center.y += d.y as f64 / self.px_per_in;
    }
}

struct PlanApp {
    project: Project,
    floor: usize,
    path: Option<PathBuf>,
    camera: Camera,
    tool: Tool,
    wall_kind: WallKind,
    exterior_thickness: f64,
    interior_thickness: f64,
    wall_height: f64,
    grid_in: f64,
    snap_in: f64,
    selected: Option<Id>,
    pending_start: Option<Point>,
    rooms: Vec<Room>,
    rooms_dirty: bool,
    cursor_world: Option<Point>,
    snapped: Option<Point>,
    hover_wall: Option<Id>,
    message: String,
}

impl PlanApp {
    fn new() -> Self {
        Self {
            project: Project::new("Untitled"),
            floor: 0,
            path: None,
            camera: Camera {
                center: Point::new(240.0, 150.0),
                px_per_in: 2.0,
            },
            tool: Tool::Select,
            wall_kind: WallKind::Exterior,
            exterior_thickness: DEFAULT_EXTERIOR_THICKNESS,
            interior_thickness: DEFAULT_INTERIOR_THICKNESS,
            wall_height: DEFAULT_CEILING_HEIGHT,
            grid_in: 12.0,
            snap_in: 1.0,
            selected: None,
            pending_start: None,
            rooms: Vec::new(),
            rooms_dirty: true,
            cursor_world: None,
            snapped: None,
            hover_wall: None,
            message: String::new(),
        }
    }

    fn floor(&self) -> &Floor {
        &self.project.floors[self.floor]
    }

    fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.tool = tool;
            self.pending_start = None;
            self.hover_wall = None;
        }
    }

    fn reset_view_state(&mut self) {
        self.selected = None;
        self.pending_start = None;
        self.hover_wall = None;
        self.rooms_dirty = true;
    }

    // ----- file operations -----

    fn new_project(&mut self) {
        self.project = Project::new("Untitled");
        self.path = None;
        self.reset_view_state();
        self.message = "New project".into();
    }

    fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &["psplan"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| Project::from_json(&s).map_err(|e| e.to_string()))
        {
            Ok(p) if !p.floors.is_empty() => {
                self.project = p;
                self.floor = 0;
                self.message = format!("Opened {}", path.display());
                self.path = Some(path);
                self.reset_view_state();
            }
            Ok(_) => self.message = "File contains no floors".into(),
            Err(e) => self.message = format!("Open failed: {e}"),
        }
    }

    fn save_project(&mut self) {
        match self.path.clone() {
            Some(p) => self.write_to(p),
            None => self.save_project_as(),
        }
    }

    fn save_project_as(&mut self) {
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &["psplan"])
            .set_file_name("plan.psplan")
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("psplan");
        }
        self.write_to(path);
    }

    fn write_to(&mut self, path: PathBuf) {
        let result = self
            .project
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.message = format!("Saved {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.message = format!("Save failed: {e}"),
        }
    }

    // ----- model edits -----

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.project.remove_wall(self.floor, id);
            self.rooms_dirty = true;
        }
    }

    fn current_thickness(&self) -> f64 {
        match self.wall_kind {
            WallKind::Exterior => self.exterior_thickness,
            WallKind::Interior => self.interior_thickness,
        }
    }

    /// Nearest wall whose centerline is within the pick radius of `p`.
    fn nearest_wall(&self, p: Point) -> Option<Id> {
        let max = PICK_RADIUS_PX / self.camera.px_per_in;
        self.floor()
            .walls
            .iter()
            .map(|w| (w.id, dist_to_segment(p, w.start, w.end)))
            .filter(|(_, d)| *d <= max)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Wall-tool snapping: existing endpoints first, then the grid, then 15 degree angles.
    fn snap_wall_point(&self, raw: Point, alt: bool) -> Point {
        let max = PICK_RADIUS_PX / self.camera.px_per_in;
        let endpoint = self
            .floor()
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .map(|e| (e, e.dist(raw)))
            .filter(|(_, d)| *d <= max)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(e, _)| e);
        if let Some(e) = endpoint {
            return e;
        }
        let grid = snap_to_grid(raw, self.snap_in);
        match self.pending_start {
            Some(start) if !alt => angle_snap(start, raw, self.snap_in).unwrap_or(grid),
            _ => grid,
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (k1, k2, k3, k4, esc, del) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Num1),
                i.key_pressed(egui::Key::Num2),
                i.key_pressed(egui::Key::Num3),
                i.key_pressed(egui::Key::Num4),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            )
        });
        if k1 {
            self.set_tool(Tool::Select);
        }
        if k2 {
            self.set_tool(Tool::Wall);
        }
        if k3 {
            self.set_tool(Tool::Door);
        }
        if k4 {
            self.set_tool(Tool::Window);
        }
        if esc {
            self.pending_start = None;
        }
        if del {
            self.delete_selected();
        }
    }

    // ----- UI panels -----

    fn menu_and_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.new_project();
                        ui.close_menu();
                    }
                    if ui.button("Open…").clicked() {
                        ui.close_menu();
                        self.open_project();
                    }
                    if ui.button("Save").clicked() {
                        ui.close_menu();
                        self.save_project();
                    }
                    if ui.button("Save As…").clicked() {
                        ui.close_menu();
                        self.save_project_as();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned());
                ui.label(egui::RichText::new(name.unwrap_or_else(|| "(unsaved)".into())).weak());
            });
            ui.horizontal(|ui| {
                let before = self.tool;
                let mut tool = before;
                ui.selectable_value(&mut tool, Tool::Select, "Select (1)");
                ui.selectable_value(&mut tool, Tool::Wall, "Wall (2)");
                ui.selectable_value(&mut tool, Tool::Door, "Door (3)");
                ui.selectable_value(&mut tool, Tool::Window, "Window (4)");
                self.set_tool(tool);
                ui.separator();
                ui.label("Wall kind:");
                ui.selectable_value(&mut self.wall_kind, WallKind::Exterior, "Exterior");
                ui.selectable_value(&mut self.wall_kind, WallKind::Interior, "Interior");
            });
        });
    }

    fn properties_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("properties")
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                ui.label("Default walls");
                inch_drag(
                    ui,
                    "Exterior thickness",
                    &mut self.exterior_thickness,
                    1.0..=24.0,
                );
                inch_drag(
                    ui,
                    "Interior thickness",
                    &mut self.interior_thickness,
                    1.0..=24.0,
                );
                inch_drag(ui, "Wall height", &mut self.wall_height, 24.0..=240.0);
                ui.separator();
                ui.label("Grid");
                inch_drag(ui, "Grid spacing", &mut self.grid_in, 1.0..=240.0);
                inch_drag(ui, "Snap spacing", &mut self.snap_in, 0.25..=48.0);
                ui.separator();
                ui.label("Rooms");
                if self.rooms.is_empty() {
                    ui.weak("None detected (close a loop of walls)");
                }
                for room in &self.rooms {
                    ui.label(format!(
                        "{}  -  {} sq ft",
                        room.label,
                        room.area_sq_ft().round()
                    ));
                }
                ui.separator();
                self.selected_wall_section(ui);
            });
    }

    fn selected_wall_section(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected else {
            ui.weak("No wall selected");
            return;
        };
        let Some(wall) = self.floor().wall(id) else {
            self.selected = None;
            return;
        };
        let (len, kind, mut thickness) = (wall.length(), wall.kind, wall.thickness);
        let openings: Vec<(Id, OpeningKind, f64, f64)> = self
            .floor()
            .openings_on(id)
            .map(|o| (o.id, o.kind, o.center_offset, o.width))
            .collect();

        ui.label("Selected wall");
        ui.label(format!("Length: {}", fmt_ft_in(len)));
        ui.label(format!(
            "Kind: {}",
            if kind == WallKind::Exterior {
                "Exterior"
            } else {
                "Interior"
            }
        ));
        if inch_drag(ui, "Thickness", &mut thickness, 1.0..=24.0) {
            let floor = self.floor;
            if let Some(w) = self.project.floors[floor].wall_mut(id) {
                w.thickness = thickness;
            }
            self.rooms_dirty = true;
        }
        if ui.button("Delete wall").clicked() {
            self.delete_selected();
            return;
        }
        ui.label("Openings");
        if openings.is_empty() {
            ui.weak("None");
        }
        let mut remove = None;
        for (oid, okind, center, width) in openings {
            ui.horizontal(|ui| {
                let name = if okind == OpeningKind::Door {
                    "Door"
                } else {
                    "Window"
                };
                ui.label(format!(
                    "{name} {} @ {}",
                    fmt_ft_in(width),
                    fmt_ft_in(center)
                ));
                if ui.small_button("✕").clicked() {
                    remove = Some(oid);
                }
            });
        }
        if let Some(oid) = remove {
            self.project.remove_opening(self.floor, oid);
        }
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                match self.cursor_world {
                    Some(p) => {
                        ui.monospace(format!("X: {}  Y: {}", fmt_ft_in(p.x), fmt_ft_in(p.y)))
                    }
                    None => ui.monospace("X: --  Y: --"),
                };
                if let (Some(start), Some(end)) = (self.pending_start, self.snapped) {
                    ui.separator();
                    ui.label(format!("Length: {}", fmt_ft_in(start.dist(end))));
                }
                ui.separator();
                ui.label(match self.tool {
                    Tool::Select => "Select: click a wall to select it; Delete removes it",
                    Tool::Wall => {
                        "Wall: click to place points; Alt disables angle snap; Esc/right-click ends"
                    }
                    Tool::Door => "Door: click on a wall to place a door",
                    Tool::Window => "Window: click on a wall to place a window",
                });
                if !self.message.is_empty() {
                    ui.separator();
                    ui.weak(&self.message);
                }
            });
        });
    }

    // ----- canvas -----

    fn canvas(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;

        self.handle_camera_input(ui, &resp, rect);

        let alt = ctx.input(|i| i.modifiers.alt);
        let new_cursor = resp
            .hover_pos()
            .map(|p| self.camera.screen_to_world(rect, p));
        if new_cursor.map(|p| (p.x, p.y)) != self.cursor_world.map(|p| (p.x, p.y)) {
            ctx.request_repaint();
        }
        self.cursor_world = new_cursor;
        self.snapped = None;
        self.hover_wall = None;
        if let Some(raw) = new_cursor {
            match self.tool {
                Tool::Wall => self.snapped = Some(self.snap_wall_point(raw, alt)),
                Tool::Door | Tool::Window => self.hover_wall = self.nearest_wall(raw),
                Tool::Select => {}
            }
        }

        if resp.clicked_by(egui::PointerButton::Primary) {
            if let Some(raw) = new_cursor {
                self.handle_click(raw);
            }
        }
        if resp.clicked_by(egui::PointerButton::Secondary) {
            self.pending_start = None;
        }

        self.draw(&painter, rect);
    }

    fn handle_camera_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect) {
        if resp.dragged_by(egui::PointerButton::Middle)
            || resp.dragged_by(egui::PointerButton::Secondary)
        {
            self.camera.pan_by_pixels(resp.drag_delta());
        }
        if let Some(pos) = resp.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.005).exp() * zoom as f64;
            if (factor - 1.0).abs() > 1e-9 {
                self.camera.zoom_about(rect, pos, factor);
            }
        }
    }

    fn handle_click(&mut self, raw: Point) {
        match self.tool {
            Tool::Select => self.selected = self.nearest_wall(raw),
            Tool::Wall => {
                let Some(p) = self.snapped else { return };
                match self.pending_start {
                    None => self.pending_start = Some(p),
                    Some(start) if start.dist(p) >= 1.0 => {
                        let id = self.project.add_wall(
                            self.floor,
                            start,
                            p,
                            self.current_thickness(),
                            self.wall_height,
                            self.wall_kind,
                        );
                        self.selected = Some(id);
                        self.pending_start = Some(p);
                        self.rooms_dirty = true;
                    }
                    Some(_) => {}
                }
            }
            Tool::Door | Tool::Window => {
                let Some(wid) = self.nearest_wall(raw) else {
                    return;
                };
                let Some(wall) = self.floor().wall(wid) else {
                    return;
                };
                let (t, _) = project_on_segment(raw, wall.start, wall.end);
                let offset = t * wall.length();
                let kind = if self.tool == Tool::Door {
                    OpeningKind::Door
                } else {
                    OpeningKind::Window
                };
                match self.project.add_opening(self.floor, wid, offset, kind) {
                    Some(_) => {
                        self.selected = Some(wid);
                        self.message.clear();
                    }
                    None => {
                        self.message =
                            "Opening does not fit there (wall too short or overlap)".into()
                    }
                }
            }
        }
    }

    fn draw(&self, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, BACKGROUND);
        self.draw_grid(painter, rect);
        self.draw_rooms(painter, rect);
        let floor = self.floor();
        for wall in &floor.walls {
            self.draw_wall(painter, rect, wall);
        }
        for wall in &floor.walls {
            for o in floor.openings_on(wall.id) {
                self.draw_opening(painter, rect, wall, o);
            }
        }
        if let Some(w) = self.selected.and_then(|id| floor.wall(id)) {
            self.draw_wall_outline(painter, rect, w, Stroke::new(3.0_f32, SELECT_COLOR));
        }
        if let Some(w) = self.hover_wall.and_then(|id| floor.wall(id)) {
            self.draw_wall_outline(
                painter,
                rect,
                w,
                Stroke::new(3.0_f32, Color32::from_rgb(40, 140, 255)),
            );
        }
        self.draw_rubber_band(painter, rect);
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let cam = self.camera;
        let mut spacing = self.grid_in.max(0.25);
        while spacing * cam.px_per_in < 8.0 {
            spacing *= 5.0;
        }
        let tl = cam.screen_to_world(rect, rect.left_top());
        let br = cam.screen_to_world(rect, rect.right_bottom());
        let minor = Stroke::new(1.0_f32, Color32::from_gray(232));
        let major = Stroke::new(1.0_f32, Color32::from_gray(215));
        let x0 = (tl.x / spacing).floor() as i64;
        let x1 = (br.x / spacing).ceil() as i64;
        for k in x0..=x1 {
            let x = cam
                .world_to_screen(rect, Point::new(k as f64 * spacing, 0.0))
                .x;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                stroke,
            );
        }
        let y0 = (br.y / spacing).floor() as i64;
        let y1 = (tl.y / spacing).ceil() as i64;
        for k in y0..=y1 {
            let y = cam
                .world_to_screen(rect, Point::new(0.0, k as f64 * spacing))
                .y;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                stroke,
            );
        }
        let o = cam.world_to_screen(rect, Point::ZERO);
        let red = Stroke::new(1.5_f32, Color32::from_rgb(200, 60, 60));
        painter.line_segment([o - Vec2::new(6.0, 0.0), o + Vec2::new(6.0, 0.0)], red);
        painter.line_segment([o - Vec2::new(0.0, 6.0), o + Vec2::new(0.0, 6.0)], red);
    }

    fn draw_rooms(&self, painter: &egui::Painter, rect: Rect) {
        let outline = Stroke::new(1.5_f32, Color32::from_rgb(90, 130, 190));
        for room in &self.rooms {
            let pts: Vec<Pos2> = room
                .polygon
                .iter()
                .map(|p| self.camera.world_to_screen(rect, *p))
                .collect();
            painter.add(Shape::closed_line(pts, outline));
            painter.text(
                self.camera.world_to_screen(rect, room.centroid),
                Align2::CENTER_CENTER,
                format!("{}\n{} sq ft", room.label, room.area_sq_ft().round()),
                FontId::proportional(13.0),
                Color32::from_rgb(60, 80, 120),
            );
        }
    }

    fn quad(&self, rect: Rect, pts: [Point; 4]) -> Vec<Pos2> {
        pts.iter()
            .map(|p| self.camera.world_to_screen(rect, *p))
            .collect()
    }

    fn draw_wall(&self, painter: &egui::Painter, rect: Rect, wall: &Wall) {
        let fill = match wall.kind {
            WallKind::Exterior => Color32::from_gray(190),
            WallKind::Interior => Color32::from_gray(215),
        };
        let pts = self.quad(rect, wall.footprint());
        painter.add(Shape::convex_polygon(
            pts,
            fill,
            Stroke::new(1.0_f32, Color32::BLACK),
        ));
    }

    fn draw_wall_outline(&self, painter: &egui::Painter, rect: Rect, wall: &Wall, stroke: Stroke) {
        painter.add(Shape::closed_line(
            self.quad(rect, wall.footprint()),
            stroke,
        ));
    }

    fn draw_opening(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        wall: &Wall,
        o: &plan_core::Opening,
    ) {
        let cam = self.camera;
        let sc = |p: Point| cam.world_to_screen(rect, p);
        let line = Stroke::new(1.0_f32, Color32::BLACK);
        let d = wall.direction();
        let n = wall.normal();
        let half = wall.thickness * 0.5;
        let over = half + 1.0 + 1.0 / cam.px_per_in;
        let pa = wall.point_at(o.start_offset());
        let pb = wall.point_at(o.end_offset());

        // White quad hides the wall fill and its stroke across the opening.
        let white = [
            pa.add(n.scale(over)),
            pb.add(n.scale(over)),
            pb.sub(n.scale(over)),
            pa.sub(n.scale(over)),
        ];
        painter.add(Shape::convex_polygon(
            self.quad(rect, white),
            Color32::WHITE,
            Stroke::NONE,
        ));
        for j in [pa, pb] {
            painter.line_segment([sc(j.add(n.scale(half))), sc(j.sub(n.scale(half)))], line);
        }

        match o.kind {
            OpeningKind::Door => {
                let (hinge, toward_other, side) = if o.swing_flipped {
                    (pb, d.scale(-1.0), n.scale(-1.0))
                } else {
                    (pa, d, n)
                };
                painter.line_segment([sc(hinge), sc(hinge.add(side.scale(o.width)))], line);
                let arc: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let a = i as f64 / 16.0 * std::f64::consts::FRAC_PI_2;
                        let v = toward_other.scale(a.cos()).add(side.scale(a.sin()));
                        sc(hinge.add(v.scale(o.width)))
                    })
                    .collect();
                painter.add(Shape::line(
                    arc,
                    Stroke::new(1.0_f32, Color32::from_gray(90)),
                ));
            }
            OpeningKind::Window => {
                for off in [half, 0.0, -half] {
                    let s = n.scale(off);
                    painter.line_segment(
                        [sc(pa.add(s)), sc(pb.add(s))],
                        Stroke::new(0.8_f32, Color32::BLACK),
                    );
                }
            }
        }
    }

    fn draw_rubber_band(&self, painter: &egui::Painter, rect: Rect) {
        if self.tool != Tool::Wall {
            return;
        }
        let Some(snapped) = self.snapped else { return };
        if let Some(start) = self.pending_start {
            let len = start.dist(snapped);
            if len > 0.01 {
                let ghost = Wall {
                    id: 0,
                    start,
                    end: snapped,
                    thickness: self.current_thickness(),
                    height: self.wall_height,
                    kind: self.wall_kind,
                };
                painter.add(Shape::convex_polygon(
                    self.quad(rect, ghost.footprint()),
                    Color32::from_rgba_unmultiplied(80, 140, 255, 90),
                    Stroke::new(1.0_f32, Color32::from_rgb(40, 100, 220)),
                ));
                let mid = Point::lerp(start, snapped, 0.5)
                    .add(ghost.normal().scale(ghost.thickness * 0.5));
                painter.text(
                    self.camera.world_to_screen(rect, mid)
                        + Vec2::new(0.0, -8.0) * ghost.normal().y.signum() as f32,
                    Align2::CENTER_CENTER,
                    fmt_ft_in(len),
                    FontId::proportional(13.0),
                    Color32::from_rgb(20, 60, 160),
                );
            }
        }
        painter.circle_stroke(
            self.camera.world_to_screen(rect, snapped),
            5.0,
            Stroke::new(1.5_f32, Color32::from_rgb(40, 100, 220)),
        );
    }
}

impl eframe::App for PlanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.rooms_dirty {
            self.rooms = detect_rooms(&self.floor().walls, 0.5);
            self.rooms_dirty = false;
        }
        self.handle_keys(ctx);
        self.menu_and_toolbar(ctx);
        self.status_bar(ctx);
        self.properties_panel(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.canvas(ctx, ui));
        if self.rooms_dirty {
            ctx.request_repaint();
        }
    }
}

/// DragValue row in inches; returns true if the value changed.
fn inch_drag(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .speed(0.25)
                .range(range)
                .suffix("\""),
        )
        .changed()
    })
    .inner
}

fn snap_to_grid(p: Point, step: f64) -> Point {
    if step <= 0.0 {
        return p;
    }
    Point::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// Snap `p` to a 15 degree angle from `start`, then re-snap the length to `step`.
fn angle_snap(start: Point, p: Point, step: f64) -> Option<Point> {
    let v = p.sub(start);
    let len = v.length();
    if len < 1e-6 {
        return None;
    }
    let inc = 15f64.to_radians();
    let a = (v.angle() / inc).round() * inc;
    let len = if step > 0.0 {
        (len / step).round() * step
    } else {
        len
    };
    Some(start.add(Point::new(a.cos(), a.sin()).scale(len)))
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Plan Studio"),
        ..Default::default()
    };
    eframe::run_native(
        "Plan Studio",
        options,
        Box::new(|_cc| Ok(Box::new(PlanApp::new()))),
    )
}
