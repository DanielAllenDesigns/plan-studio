//! Door and window placement (DW-1..DW-15 in `docs/parity/doors-windows.md`).
//!
//! The opening is centered under the pointer's projection onto the wall and
//! snaps to the grid snap unit (1"); new openings come from the defaults
//! templates (exterior door on exterior walls, interior door otherwise,
//! window), sized for their flavor by the variant defaults (Doorway, Sliding,
//! Pocket, Bifold, Barn, Garage, Double and Shower doors; Casement, Fixed,
//! Sliding, Awning, Hopper, Bay, Bow and Box windows, Pass-Through and Wall
//! Niche). A ghost with temporary dimensions to both wall ends (and the width
//! and neighbouring openings) follows the pointer. The tool stays active after
//! a placement.
//!
//! Door swing and hinge (DW-8, DW-76) are derived from the pointer, not copied
//! from the template: the door swings toward the side of the wall the pointer
//! is on (`swing_flipped` false for the wall's left side, true for the right;
//! a pointer exactly on the centerline keeps the left), and the hinge goes to
//! the jamb nearer the closer wall end (`hinge_at_end` when the center is in
//! the far half of the wall). The plain window is unaffected; casements follow
//! the pointer like doors.

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::place_from_template;
use crate::editor::opening_view::draw_opening;
use crate::editor::tempdim::{self, opening_temp_dims, TempDims};
use crate::editor::{render, Camera, EditAction, EditorContext, ObjectRef};
use crate::toolbar::ViewFlag;
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::openings::door_defaults_for_pointer;
use plan_core::{Id, OpeningKind, OpeningStyle};

/// Which flavor of door or window a tool places.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OpeningVariant {
    pub kind: OpeningKind,
    pub style: OpeningStyle,
}

impl OpeningVariant {
    pub const fn door(style: OpeningStyle) -> Self {
        Self {
            kind: OpeningKind::Door,
            style,
        }
    }

    pub const fn window(style: OpeningStyle) -> Self {
        Self {
            kind: OpeningKind::Window,
            style,
        }
    }

    /// The tool id a flyout entry for this flavor uses: the plain Hinged Door
    /// and Window keep `Door` and `Window`.
    pub fn tool_id(self) -> ToolId {
        match (self.kind, self.style) {
            (OpeningKind::Door, OpeningStyle::Hinged) => ToolId::Door,
            (OpeningKind::Window, OpeningStyle::Window) => ToolId::Window,
            _ => ToolId::OpeningVariant(self),
        }
    }

    /// Chief's name of the tool.
    pub fn name(self) -> &'static str {
        self.style.name(self.kind)
    }
}

pub struct OpeningTool {
    kind: OpeningKind,
    style: OpeningStyle,
    hover: Option<(Id, f64)>,
    /// World position of the last pointer move (drives the ghost's swing).
    hover_pointer: Point,
}

impl Default for OpeningTool {
    fn default() -> Self {
        Self {
            kind: OpeningKind::Door,
            style: OpeningStyle::Hinged,
            hover: None,
            hover_pointer: Point::new(0.0, 0.0),
        }
    }
}

/// The wall under `p` and the snapped opening center along it.
fn target(cx: &EditorContext, p: Point, alt: bool) -> Option<(Id, f64)> {
    let tol = cx.pick_tol();
    let wall = cx
        .floor()
        .walls
        .iter()
        .filter(|w| cx.layers().is_visible(&w.layer))
        .map(|w| (w, w.closest_point(p).0.dist(p)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)?;
    // Along the wall is arc length on a curved wall (DW-88).
    let mut offset = wall.locate(p).0;
    let unit = cx.snap_unit();
    if !alt {
        offset = (offset / unit).round() * unit;
    }
    Some((wall.id, offset))
}

impl OpeningTool {
    /// The opening a click at `center` on `wall` would place: the defaults
    /// template of the wall, sized for the flavor, its swing and hinge taken
    /// from the pointer where the flavor has them.
    fn opening_for(
        &self,
        cx: &EditorContext,
        wall: &plan_core::Wall,
        pointer: Point,
        center: f64,
    ) -> (crate::dialogs::OpeningTarget, plan_core::Opening) {
        let (target_key, template) = cx.opening_template(self.kind, wall.kind);
        let mut o = cx.defaults.opening_variants.apply(&template, self.style);
        if self.swings() {
            (o.swing_flipped, o.hinge_at_end) = door_defaults_for_pointer(wall, pointer, center);
        }
        o.wall_id = wall.id;
        (target_key, o)
    }

    /// Does the flavor have a swing side and a hinge to take from the pointer?
    fn swings(&self) -> bool {
        match self.kind {
            OpeningKind::Door => true,
            OpeningKind::Window => self.style == OpeningStyle::Casement,
        }
    }

    fn variant(&self) -> OpeningVariant {
        OpeningVariant {
            kind: self.kind,
            style: self.style,
        }
    }
}

impl Tool for OpeningTool {
    fn id(&self) -> ToolId {
        match self.kind {
            OpeningKind::Door => ToolId::Door,
            OpeningKind::Window => ToolId::Window,
        }
    }

    fn name(&self) -> &'static str {
        self.variant().name()
    }

    fn hint(&self) -> String {
        let what = match self.kind {
            OpeningKind::Door => "a door",
            OpeningKind::Window => "a window",
        };
        let kind = match self.kind {
            OpeningKind::Door => "Door",
            OpeningKind::Window => "Window",
        };
        if self.style == OpeningStyle::default_for(self.kind) {
            format!("{kind}: click on a wall to place {what}")
        } else {
            format!("{}: click on a wall to place {what}", self.name())
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        match id {
            ToolId::Door => {
                self.kind = OpeningKind::Door;
                self.style = OpeningStyle::Hinged;
            }
            ToolId::Window => {
                self.kind = OpeningKind::Window;
                self.style = OpeningStyle::Window;
            }
            ToolId::OpeningVariant(v) => {
                self.kind = v.kind;
                self.style = v.style;
            }
            _ => {}
        }
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.hover = None;
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = target(cx, p.world, p.modifiers.alt);
        self.hover_pointer = p.world;
        ToolResult {
            repaint: true,
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let Some((wid, center)) = target(cx, p.world, p.modifiers.alt) else {
            return ToolResult::consumed();
        };
        let Some(wall) = cx.floor().wall(wid).cloned() else {
            return ToolResult::consumed();
        };
        let (target_key, template) = self.opening_for(cx, &wall, p.world, center);
        let extras = cx.default_opening_extras(target_key);
        let label = match self.kind {
            OpeningKind::Door => "Place Door",
            OpeningKind::Window => "Place Window",
        };
        // A window clicked onto a door goes over it as a transom (DW-52).
        let door_here = (self.kind == OpeningKind::Window)
            .then(|| {
                cx.floor()
                    .openings_on(wid)
                    .find(|d| {
                        d.kind == OpeningKind::Door
                            && center >= d.start_offset()
                            && center <= d.end_offset()
                    })
                    .map(|d| d.id)
            })
            .flatten();
        if let Some(door) = door_here {
            cx.begin_change("Add Transom");
            let fl = cx.floor;
            return match cx.project.add_transom(fl, door, template.height) {
                Ok(id) => {
                    cx.selection.set(ObjectRef::Opening(id));
                    cx.status = "Added a transom over the door".into();
                    cx.mark_dirty();
                    ToolResult::committed("Add Transom")
                }
                Err(e) => {
                    cx.cancel_change();
                    cx.status = e;
                    ToolResult::consumed()
                }
            };
        }
        cx.begin_change(label);
        let fl = cx.floor;
        match place_from_template(&mut cx.project, fl, wid, center, &template) {
            Some(id) => {
                // The tab values the Default Settings dialog set this
                // session go onto the new opening.
                if let Some(spec) = extras.default_spec().cloned() {
                    if let Some(o) = cx.project.floors[fl]
                        .openings
                        .iter_mut()
                        .find(|o| o.id == id)
                    {
                        o.extras.spec = spec;
                    }
                }
                cx.extras.openings.insert(id, extras);
                // The new opening is selected (DW-77) so its Edit toolbar
                // shows; the tool stays active for the next one.
                cx.selection.set(ObjectRef::Opening(id));
                cx.status.clear();
                cx.mark_dirty();
                ToolResult::committed(label)
            }
            None => {
                cx.cancel_change();
                cx.status = "Opening does not fit there (wall too short or overlap)".into();
                ToolResult::consumed()
            }
        }
    }

    /// Esc first lets go of the opening just placed, leaving the tool ready
    /// for the next one; a second Esc ends the tool.
    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(egui::Key::Escape)
            && cx
                .selection
                .items
                .iter()
                .any(|o| matches!(o, ObjectRef::Opening(_)))
        {
            cx.selection.clear();
            return ToolResult {
                repaint: true,
                ..ToolResult::consumed()
            };
        }
        ToolResult::ignored()
    }

    /// The Edit toolbar of the opening just placed: Center on Wall Segment,
    /// Flip Hinge, Mull... as under Select.
    fn edit_toolbar(&self, cx: &EditorContext) -> Vec<EditAction> {
        if !cx
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Opening(_)))
        {
            return Vec::new();
        }
        let mut v = cx.common_edit_actions();
        v.extend(cx.extra_edit_actions());
        v
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let Some((wid, center)) = self.hover else {
            return;
        };
        let Some(wall) = cx.floor().wall(wid) else {
            return;
        };
        let pal = &cx.palette;
        render::draw_wall_outline(
            painter,
            cam,
            cx,
            wall,
            egui::Stroke::new(3.0_f32, pal.hover),
        );
        let (_, mut ghost) = self.opening_for(cx, wall, self.hover_pointer, center);
        let half = ghost.width * 0.5;
        let len = wall.path_length();
        if len < ghost.width + 4.0 {
            return;
        }
        ghost.center_offset = center.clamp(half + 2.0, len - half - 2.0);
        let exterior = plan_core::exterior_sign(wall, &cx.rooms);
        draw_opening(painter, cam, wall, &ghost, pal, exterior, true);
        if cx.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            let dims = TempDims {
                dims: opening_temp_dims(
                    cx.floor(),
                    wall,
                    &ghost,
                    ObjectRef::Wall(wid),
                    &tempdim::TempLocate::of(cx),
                ),
                editing: None,
            };
            tempdim::draw(&dims, painter, cam, pal, &cx.dim_format());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;

    fn setup() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            7.625,
            109.125,
            WallKind::Exterior,
        );
        (cx, w)
    }

    fn click(t: &mut OpeningTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true))
    }

    #[test]
    fn doors_and_windows_come_from_the_templates() {
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.4, 0.0);
        let o = cx.floor().openings_on(w).next().unwrap();
        // Exterior wall: exterior door defaults, centered and snapped to 1".
        assert_eq!((o.width, o.height), (36.0, 96.0));
        assert_eq!(o.center_offset, 120.0);

        t.set_variant(ToolId::Window);
        let r = click(&mut t, &mut cx, 40.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Place Window"));
        let win = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap();
        assert_eq!((win.width, win.height, win.sill_height), (32.0, 72.0, 24.0));
        assert!(cx.extras.openings.contains_key(&win.id));
    }

    #[test]
    fn door_swing_follows_the_pointer_side_and_hinge_the_nearer_end() {
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        // Wall runs +x, so its left (normal) side is +y or -y by `perp`;
        // read it from the wall rather than assume.
        let n = cx.floor().wall(w).unwrap().normal();
        click(&mut t, &mut cx, 40.0, 3.0 * n.y);
        click(&mut t, &mut cx, 200.0, -3.0 * n.y);
        let mut ds: Vec<_> = cx.floor().openings_on(w).cloned().collect();
        ds.sort_by(|a, b| a.center_offset.total_cmp(&b.center_offset));
        // Pointer on the left side near the start: unflipped, hinge at start.
        assert!(!ds[0].swing_flipped && !ds[0].hinge_at_end);
        // Right side near the end: flipped, hinge at the end jamb.
        assert!(ds[1].swing_flipped && ds[1].hinge_at_end);

        // Windows are unaffected.
        t.set_variant(ToolId::Window);
        click(&mut t, &mut cx, 120.0, -3.0 * n.y);
        let win = cx
            .floor()
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap();
        assert!(!win.swing_flipped && !win.hinge_at_end);
    }

    #[test]
    fn interior_walls_get_the_interior_door() {
        let (mut cx, _) = setup();
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 60.0),
            Point::new(240.0, 60.0),
            4.5,
            109.125,
            WallKind::Interior,
        );
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 100.0, 60.0);
        let o = cx.floor().openings_on(w).next().unwrap();
        assert_eq!((o.width, o.height), (30.0, 96.0));
    }

    #[test]
    fn refused_placement_leaves_no_undo_step() {
        let (mut cx, _) = setup();
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.0, 0.0);
        let n = cx.floor().openings.len();
        click(&mut t, &mut cx, 130.0, 0.0);
        assert_eq!(cx.floor().openings.len(), n);
        assert!(cx.status.contains("does not fit"));
        assert_eq!(cx.undo().as_deref(), Some("Place Door"));
        assert!(cx.floor().openings.is_empty());
        assert!(!cx.can_undo());
    }

    #[test]
    fn the_new_opening_is_selected_with_its_edit_toolbar_and_esc_lets_go() {
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        assert!(t.edit_toolbar(&cx).is_empty());
        click(&mut t, &mut cx, 120.0, 0.0);
        let id = cx.floor().openings_on(w).next().unwrap().id;
        // The opening, not the wall, is selected (DW-77).
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(id)));
        // Its Edit toolbar shows while the tool stays active.
        let labels: Vec<&str> = t.edit_toolbar(&cx).iter().map(|a| a.label).collect();
        assert!(labels.contains(&"Center on Wall Segment"), "{labels:?}");
        assert!(labels.contains(&"Flip Hinge"), "{labels:?}");
        assert_eq!(t.id(), ToolId::Door);
        // A second click places another one and selects that.
        click(&mut t, &mut cx, 40.0, 0.0);
        let other = cx.floor().openings_on(w).find(|o| o.id != id).unwrap().id;
        assert_eq!(cx.selection.single(), Some(ObjectRef::Opening(other)));
        // Esc lets go of the selection and the tool is still the tool; the
        // next Esc is the app's, which returns to Select.
        let r = t.key(&mut cx, KeyEvent::escape());
        assert!(r.consumed);
        assert!(cx.selection.is_empty());
        assert!(t.edit_toolbar(&cx).is_empty());
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed);
        // One undo step removes the opening again and clears the selection.
        click(&mut t, &mut cx, 200.0, 0.0);
        assert_eq!(cx.undo().as_deref(), Some("Place Door"));
    }

    #[test]
    fn a_placed_opening_carries_the_tab_values_of_the_default_dialog() {
        use plan_core::openings::{ArchType, ShutterStyle};
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        t.set_variant(ToolId::Window);
        // The Default Settings dialog kept shutters and an arch this session.
        let key = crate::dialogs::OpeningTarget::DefaultWindow;
        let mut ex = cx.default_opening_extras(key);
        let mut dialog = crate::dialogs::OpeningDialog::for_default(
            key,
            plan_core::Opening::default_window(0, 0, 0.0),
            ex.clone(),
        );
        dialog.draft_mut().extras.spec.shutters.style = ShutterStyle::Louver;
        dialog.draft_mut().extras.spec.arch.kind = ArchType::RoundTop;
        dialog.sync_stored_for_test();
        ex = dialog.extras().clone();
        cx.extras.openings.insert(key.key(), ex);
        click(&mut t, &mut cx, 120.0, 0.0);
        let o = cx.floor().openings_on(w).next().unwrap();
        assert_eq!(o.extras.spec.shutters.style, ShutterStyle::Louver);
        assert_eq!(o.extras.spec.arch.kind, ArchType::RoundTop);
        // Without a dialog edit, the variant defaults supply the spec.
        let (mut cx2, w2) = setup();
        cx2.defaults.opening_variants.window_spec.sill.enabled = true;
        click(&mut t, &mut cx2, 120.0, 0.0);
        assert!(
            cx2.floor()
                .openings_on(w2)
                .next()
                .unwrap()
                .extras
                .spec
                .sill
                .enabled
        );
    }

    #[test]
    fn placing_the_first_opening_gives_an_old_plan_its_label_layers() {
        let (mut cx, _) = setup();
        cx.project
            .layers
            .layers
            .retain(|l| !l.name.ends_with(", Labels"));
        assert!(cx.project.layers.get("Doors, Labels").is_none());
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.0, 0.0);
        assert!(cx.project.layers.get("Doors, Labels").is_some());
        assert!(cx.project.layers.get("Windows, Labels").is_some());
        // And it is part of the same undo step.
        cx.undo();
        assert!(cx.project.layers.get("Doors, Labels").is_none());
    }

    #[test]
    fn shutters_show_in_the_elevation_of_the_outside() {
        use plan_core::openings::ShutterStyle;
        use plan_elevation::{elevation_from_project, Options, ViewDir};
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        t.set_variant(ToolId::Window);
        click(&mut t, &mut cx, 120.0, 0.0);
        let before = elevation_from_project(&cx.project, ViewDir::Back, &Options::default());
        cx.project.floors[0]
            .openings
            .iter_mut()
            .find(|o| o.wall_id == w)
            .unwrap()
            .extras
            .spec
            .shutters
            .style = ShutterStyle::Louver;
        let after = elevation_from_project(&cx.project, ViewDir::Back, &Options::default());
        // Louvered shutters stand each side of the window: more lines, and
        // the drawing is no narrower than the window's frame.
        assert!(
            after.lines.len() > before.lines.len() + 20,
            "{} vs {}",
            after.lines.len(),
            before.lines.len()
        );
        let louver_x = after
            .lines
            .iter()
            .map(|l| l.a.x.min(l.b.x))
            .fold(f64::MAX, f64::min);
        let wall_x = before
            .lines
            .iter()
            .map(|l| l.a.x.min(l.b.x))
            .fold(f64::MAX, f64::min);
        assert!(louver_x >= wall_x - 1e-6);
    }

    #[test]
    fn a_door_on_a_curved_wall_is_placed_at_the_arc_length_under_the_pointer() {
        let (mut cx, w) = setup();
        cx.project.floors[0].walls[0].curve = Some(plan_core::walls::WallCurve { bulge: 60.0 });
        let wall = cx.floor().wall(w).unwrap().clone();
        // A point on the arc 200" along it: far from the chord.
        let at = wall.point_along(200.0);
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, at.x, at.y);
        let (first, center) = {
            let o = cx.floor().openings_on(w).next().unwrap();
            (o.id, o.center_offset)
        };
        assert!((center - 200.0).abs() <= 0.5, "{center}");
        // The swing follows the pointer's side of the arc.
        let n = wall.normal_along(200.0);
        let out = wall.point_along(120.0) + n * -2.0;
        click(&mut t, &mut cx, out.x, out.y);
        let second = cx.floor().openings_on(w).find(|x| x.id != first).unwrap();
        assert!(second.swing_flipped);
        // Nothing is placed away from the arc (the chord's middle is 60" off).
        let before = cx.floor().openings.len();
        let mid = wall.point_at(wall.length() * 0.5);
        click(&mut t, &mut cx, mid.x, mid.y);
        assert_eq!(cx.floor().openings.len(), before);
    }

    #[test]
    fn a_window_clicked_onto_a_door_becomes_its_transom() {
        let (mut cx, w) = setup();
        let mut t = OpeningTool::default();
        click(&mut t, &mut cx, 120.0, 0.0);
        let door = cx.floor().openings_on(w).next().unwrap().id;
        t.set_variant(ToolId::Window);
        let r = click(&mut t, &mut cx, 125.0, 0.0);
        assert_eq!(r.commit.as_deref(), Some("Add Transom"));
        let all = &cx.floor().openings;
        assert_eq!(all.len(), 2);
        let (d, tr) = (
            all.iter().find(|o| o.id == door).unwrap(),
            all.iter().find(|o| o.id != door).unwrap(),
        );
        assert!(plan_core::openings::stands_over(tr, d));
        assert_eq!(tr.mull_group, d.mull_group);
        // The wall is 109 1/8" high and the door 96": the transom takes the room left.
        assert!(
            (tr.sill_height + tr.height - 109.125).abs() < 1e-9,
            "{tr:?}"
        );
        // A second click does not pile on another one.
        click(&mut t, &mut cx, 125.0, 0.0);
        assert_eq!(cx.floor().openings.len(), 2);
        assert!(cx.status.contains("over it already"), "{}", cx.status);
    }
}
