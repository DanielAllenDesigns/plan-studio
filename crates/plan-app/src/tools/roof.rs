//! Roof tools (`docs/parity/roofs.md`).
//!
//! One tool object with several modes; since the toolbar entries all map to
//! `ToolId::Roof`, the mode is picked in a small palette the tool draws in the
//! corner of the canvas (and by `set_mode` in tests):
//!
//! * **Roof Plane** (RF-35): press-drag the eave baseline, then click on the
//!   side the plane rises to; a rectangle at the default pitch is made;
//! * **Edit Planes** (RF-36..RF-38): click selects, drag moves, drag a vertex
//!   reshapes (the plane stays planar), double-click / Enter opens the Roof
//!   Plane Specification, Delete removes the plane;
//! * **Build Roof** (RF-1..RF-8): a click opens the Build Roof dialog; OK
//!   builds the automatic planes from the exterior walls' roof directives
//!   (RF-18, RF-19, RF-20, RF-22) with Auto Rebuild (RF-6);
//! * **Gable/Roof Line** (RF-18, RF-20, RF-44): click a wall to flip it
//!   between Hip and Full Gable;
//! * **Roof Hole** (RF-42), **Skylight** (RF-43);
//! * **Delete Roof Planes** (RF-40) and **Rebuild Roofs** as palette buttons.
//!
//! Dialogs are owned and drawn by the tool (from `draw_overlay`, through the
//! painter's egui context); what OK/buttons ask for is queued and applied on
//! the tool's next event, since drawing has no mutable access to the plan.
//!
//! Deferred: Join Roof Planes (RF-41), Auto Dormer (RF-48), Dutch gable,
//! knee wall and other directives beyond Hip/Gable/Shed (RF-21..RF-25), roof
//! returns (RF-27), ceiling planes (RF-45, RF-46), framing (RF-52..).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::roof::{BuildRoofDialog, RoofPlaneDialog};
use crate::dialogs::Outcome;
use crate::editor::roof_view::{
    self, apply_edits, auto_rebuild, build_floor, delete_all, load, manual_plane_geometry, rebuild,
    rect_polygon, store, toggle_gable, RoofPlaneRecord, RoofSettings, SKYLIGHT_SIZE,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext};
use eframe::egui::{self, Align2, FontId, Pos2, Stroke, Vec2};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{Id, WallKind};
use std::cell::{Cell, RefCell};

/// Planes need a baseline at least this long, inches.
const MIN_BASELINE: f64 = 12.0;
/// Holes are at least this big on each side, inches.
const MIN_HOLE: f64 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RoofMode {
    #[default]
    Plane,
    Edit,
    Build,
    GableLine,
    Hole,
    Skylight,
    Join,
    Dormer,
}

impl RoofMode {
    const ALL: [RoofMode; 8] = [
        RoofMode::Plane,
        RoofMode::Edit,
        RoofMode::Build,
        RoofMode::GableLine,
        RoofMode::Hole,
        RoofMode::Skylight,
        RoofMode::Join,
        RoofMode::Dormer,
    ];

    /// Chief's names.
    fn label(self) -> &'static str {
        match self {
            RoofMode::Plane => "Roof Plane",
            RoofMode::Edit => "Edit Roof Planes",
            RoofMode::Build => "Build Roof",
            RoofMode::GableLine => "Gable/Roof Line",
            RoofMode::Hole => "Roof Hole",
            RoofMode::Skylight => "Skylight",
            RoofMode::Join => "Join Roof Planes",
            RoofMode::Dormer => "Auto Dormer",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            RoofMode::Plane => {
                "Roof Plane: press and drag the baseline, then click toward the ridge; Esc cancels"
            }
            RoofMode::Edit => {
                "Roof planes: click to select, drag to move, drag a corner to reshape, Enter opens"
            }
            RoofMode::Build => "Build Roof: click to open the Build Roof dialog",
            RoofMode::GableLine => {
                "Gable/Roof Line: click an exterior wall to make it gable or hip"
            }
            RoofMode::Hole => "Roof Hole: press and drag a rectangle inside a roof plane",
            RoofMode::Skylight => "Skylight: click inside a roof plane",
            RoofMode::Join => "Join Roof Planes: click the first plane, then the second",
            RoofMode::Dormer => "Auto Dormer: click a roof plane (not implemented yet)",
        }
    }
}

/// What the palette and dialogs ask for.
enum Cmd {
    OpenBuild,
    ApplyBuild(RoofSettings),
    ApplyPlane(Id, Box<RoofPlaneRecord>),
    DeleteAll,
    Rebuild,
    OpenPlane(Id),
    DeletePlane(Id),
}

enum Gesture {
    None,
    /// Dragging the baseline of a new plane.
    Baseline {
        from: Point,
        to: Point,
    },
    /// Baseline set; waiting for the click toward the ridge.
    Draft {
        a: Point,
        b: Point,
    },
    /// Dragging a hole rectangle.
    Rect {
        from: Point,
        to: Point,
    },
    Move {
        id: Id,
        last: Point,
        began: bool,
    },
    Vertex {
        id: Id,
        idx: usize,
        began: bool,
    },
}

#[derive(Default)]
pub struct RoofTool {
    mode: Cell<RoofMode>,
    cmds: RefCell<Vec<Cmd>>,
    build_dialog: RefCell<Option<BuildRoofDialog>>,
    plane_dialog: RefCell<Option<(Id, RoofPlaneDialog)>>,
    /// The palette changed mode: forget the gesture at the next event.
    reset: Cell<bool>,
    selected: Option<Id>,
    gesture: GestureSlot,
    hover: Option<Point>,
    join_first: Option<Id>,
}

/// `Gesture` with a `Default` of `None`.
struct GestureSlot(Gesture);

impl Default for GestureSlot {
    fn default() -> Self {
        Self(Gesture::None)
    }
}

impl RoofTool {
    pub fn mode(&self) -> RoofMode {
        self.mode.get()
    }

    pub fn set_mode(&mut self, mode: RoofMode) {
        self.mode.set(mode);
        self.gesture.0 = Gesture::None;
        self.join_first = None;
    }

    /// The plane selected in Edit Roof Planes mode.
    pub fn selected(&self) -> Option<Id> {
        self.selected
    }

    fn dialogs_open(&self) -> bool {
        self.build_dialog.borrow().is_some() || self.plane_dialog.borrow().is_some()
    }

    // ----- commands -----

    /// Start of every event: palette/dialog requests, Auto Rebuild, stale
    /// selection. Returns the undo label of a change they made.
    fn begin_event(&mut self, cx: &mut EditorContext) -> Option<String> {
        if self.reset.take() {
            self.gesture.0 = Gesture::None;
            self.join_first = None;
        }
        let mut label = None;
        let cmds: Vec<Cmd> = self.cmds.borrow_mut().drain(..).collect();
        for c in cmds {
            let l = match c {
                Cmd::OpenBuild => {
                    self.open_build(cx);
                    None
                }
                Cmd::ApplyBuild(s) => self.build(cx, s),
                Cmd::ApplyPlane(id, rec) => self.apply_plane(cx, id, &rec),
                Cmd::DeleteAll => self.delete_all_planes(cx),
                Cmd::Rebuild => self.rebuild_stored(cx),
                Cmd::OpenPlane(id) => {
                    self.open_plane(cx, id);
                    None
                }
                Cmd::DeletePlane(id) => self.delete_plane(cx, id),
            };
            label = l.or(label);
        }
        if auto_rebuild(cx) {
            label = label.or(Some("Rebuild Roof".into()));
        }
        if self
            .selected
            .is_some_and(|id| !roof_view::exists(cx.floor(), id))
        {
            self.selected = None;
        }
        label
    }

    fn with_pre(pre: Option<String>, mut res: ToolResult) -> ToolResult {
        if res.commit.is_none() {
            if let Some(l) = pre {
                res.commit = Some(l);
                res.repaint = true;
            }
        }
        res
    }

    /// The settings the dialog starts from: the stored ones, else the defaults.
    fn current_settings(cx: &EditorContext) -> (RoofSettings, Option<usize>) {
        for (i, f) in cx.project.floors.iter().enumerate() {
            if let Some(s) = load(f).settings {
                return (s, Some(i));
            }
        }
        (RoofSettings::from_defaults(&cx.defaults), None)
    }

    fn open_build(&mut self, cx: &EditorContext) {
        if self.build_dialog.borrow().is_some() {
            return;
        }
        let (s, _) = Self::current_settings(cx);
        let fi = build_floor(&cx.project, s.ignore_top_floor, cx.floor);
        let note = format!(
            "The roof is built over the exterior walls of {}.",
            cx.project.floors[fi].name
        );
        *self.build_dialog.borrow_mut() = Some(BuildRoofDialog::new(s, note));
    }

    /// Build Roof OK (RF-1): automatic planes from the exterior walls.
    pub(crate) fn build(&mut self, cx: &mut EditorContext, s: RoofSettings) -> Option<String> {
        let fi = build_floor(&cx.project, s.ignore_top_floor, cx.floor);
        cx.begin_change("Build Roof");
        // A roof built over another floor before moves here: drop that one.
        for j in 0..cx.project.floors.len() {
            if j != fi {
                let mut old = load(&cx.project.floors[j]);
                if old.settings.is_some() {
                    old.planes.retain(|p| !p.auto);
                    old.settings = None;
                    store(&mut cx.project, j, &mut old);
                }
            }
        }
        match rebuild(&mut cx.project, fi, s, false) {
            Ok(rep) => {
                cx.mark_dirty();
                cx.status = format!(
                    "Built {} roof plane{} over {}{}",
                    rep.planes,
                    if rep.planes == 1 { "" } else { "s" },
                    cx.project.floors[fi].name,
                    if rep.approximate {
                        " (approximated: the footprint is too complex for an exact roof)"
                    } else {
                        ""
                    }
                );
                Some("Build Roof".into())
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = format!("Build Roof: {e}");
                None
            }
        }
    }

    fn rebuild_stored(&mut self, cx: &mut EditorContext) -> Option<String> {
        let (s, at) = Self::current_settings(cx);
        if at.is_none() {
            cx.status = "No roof has been built yet: use Build Roof".into();
            return None;
        }
        self.build(cx, s)
    }

    fn delete_all_planes(&mut self, cx: &mut EditorContext) -> Option<String> {
        cx.begin_change("Delete Roof Planes");
        let mut n = 0;
        for fi in 0..cx.project.floors.len() {
            n += delete_all(&mut cx.project, fi);
        }
        if n == 0 {
            cx.cancel_change();
            cx.status = "There are no roof planes to delete".into();
            return None;
        }
        self.selected = None;
        cx.mark_dirty();
        cx.status = format!("Deleted {n} roof plane{}", if n == 1 { "" } else { "s" });
        Some("Delete Roof Planes".into())
    }

    fn delete_plane(&mut self, cx: &mut EditorContext, id: Id) -> Option<String> {
        let fi = cx.floor;
        let mut set = load(&cx.project.floors[fi]);
        set.plane(id)?;
        cx.begin_change("Delete Roof Plane");

        set.planes.retain(|p| p.id != id);
        store(&mut cx.project, fi, &mut set);
        self.selected = None;
        cx.mark_dirty();
        Some("Delete Roof Plane".into())
    }

    fn open_plane(&mut self, cx: &EditorContext, id: Id) {
        let Some(rec) = load(cx.floor()).plane(id).cloned() else {
            return;
        };
        let layers = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        *self.plane_dialog.borrow_mut() = Some((id, RoofPlaneDialog::new(rec, layers)));
    }

    fn apply_plane(
        &mut self,
        cx: &mut EditorContext,
        id: Id,
        new: &RoofPlaneRecord,
    ) -> Option<String> {
        let fi = cx.floor;
        let mut set = load(&cx.project.floors[fi]);
        set.plane(id)?;
        cx.begin_change("Roof Plane Specification");
        if let Some(old) = set.plane_mut(id) {
            apply_edits(old, new);
        }
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        Some("Roof Plane Specification".into())
    }

    // ----- gestures -----

    /// Roof Plane (RF-35): the rectangle on baseline `a -> b` toward `toward`.
    fn create_plane(&mut self, cx: &mut EditorContext, a: Point, b: Point, toward: Point) -> bool {
        let fi = cx.floor;
        let mut set = load(&cx.project.floors[fi]);
        let settings = set
            .settings
            .clone()
            .unwrap_or_else(|| RoofSettings::from_defaults(&cx.defaults));
        let top = cx
            .floor()
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior)
            .map(|w| w.height)
            .fold(f64::NEG_INFINITY, f64::max);
        let top = if top.is_finite() {
            top
        } else {
            cx.wall_height(WallKind::Exterior)
        };
        let elev = cx.floor().elevation + top + settings.raise_off_plate;
        let Some((baseline, poly)) = manual_plane_geometry(a, b, toward, elev, settings.pitch)
        else {
            cx.status = "Click away from the baseline, on the side the roof rises to".into();
            return false;
        };
        cx.begin_change("Draw Roof Plane");
        let id = cx.project.alloc_id();
        let mut rec = RoofPlaneRecord::new(id, poly, settings.pitch, baseline);
        rec.material = settings.material;
        set.planes.push(rec);
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        self.selected = Some(id);
        cx.status = format!(
            "Roof plane at {} pitch",
            roof_view::pitch_label(settings.pitch)
        );
        true
    }

    /// Gable/Roof Line: flips the exterior wall under `at` between Hip and
    /// Full Gable and rebuilds the automatic roof.
    fn gable_line(&mut self, cx: &mut EditorContext, at: Point) -> bool {
        let tol = cx.pick_tol();
        let hit = cx
            .floor()
            .walls
            .iter()
            .filter(|w| w.kind == WallKind::Exterior)
            .map(|w| {
                (
                    dist_to_segment(at, w.start, w.end) - w.thickness * 0.5,
                    w.id,
                )
            })
            .filter(|(d, _)| *d <= tol)
            .min_by(|x, y| x.0.total_cmp(&y.0));
        let Some((_, id)) = hit else {
            cx.status = "Click an exterior wall".into();
            return false;
        };
        cx.begin_change("Gable/Roof Line");
        let fi = cx.floor;
        let Some(kind) = toggle_gable(&mut cx.project, fi, id) else {
            cx.cancel_change();
            return false;
        };
        cx.mark_dirty();
        let rebuilt = auto_rebuild(cx);
        cx.status = format!(
            "Wall is now a {}{}",
            if kind == RoofWallKind::FullGable {
                "Full Gable Wall"
            } else {
                "Hip Wall"
            },
            if rebuilt {
                ""
            } else {
                " (use Build Roof to rebuild the roof)"
            }
        );
        true
    }

    /// Roof Hole (RF-42): the rectangle becomes the hole of the plane under
    /// its center.
    fn make_hole(&mut self, cx: &mut EditorContext, a: Point, b: Point) -> bool {
        if (a.x - b.x).abs() < MIN_HOLE || (a.y - b.y).abs() < MIN_HOLE {
            return false;
        }
        let fi = cx.floor;
        let mut set = load(&cx.project.floors[fi]);
        let Some(id) = set.plane_at(Point::lerp(a, b, 0.5)) else {
            cx.status = "Draw the hole inside a roof plane".into();
            return false;
        };
        cx.begin_change("Roof Hole");
        if let Some(r) = set.plane_mut(id) {
            r.hole = Some(rect_polygon(a, b));
        }
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        cx.status = "Roof hole made".into();
        true
    }

    /// Skylight (RF-43): 24" x 48" at `at`, on the plane under it.
    fn place_skylight(&mut self, cx: &mut EditorContext, at: Point) -> bool {
        let fi = cx.floor;
        let mut set = load(&cx.project.floors[fi]);
        let Some(id) = set.plane_at(at) else {
            cx.status = "Click inside a roof plane".into();
            return false;
        };
        cx.begin_change("Place Skylight");
        if let Some(r) = set.plane_mut(id) {
            r.skylights.push((at, SKYLIGHT_SIZE.0, SKYLIGHT_SIZE.1));
        }
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        cx.status = "Skylight placed".into();
        true
    }

    /// Edit mode press: a vertex handle of the selected plane, else a plane.
    fn press_edit(&mut self, cx: &mut EditorContext, p: &PointerEvent) {
        let set = load(cx.floor());
        if let Some(rec) = self.selected.and_then(|id| set.plane(id)) {
            let tol = cx.pick_tol();
            let hit = rec
                .plan_polygon()
                .iter()
                .position(|v| v.dist(p.world) <= tol);
            if let Some(idx) = hit {
                self.gesture.0 = Gesture::Vertex {
                    id: rec.id,
                    idx,
                    began: false,
                };
                return;
            }
        }
        match set.plane_at(p.world) {
            Some(id) => {
                self.selected = Some(id);
                self.gesture.0 = Gesture::Move {
                    id,
                    last: p.snapped,
                    began: false,
                };
            }
            None => self.selected = None,
        }
    }

    /// Applies a drag step of Move / Vertex; the first step starts the undo step.
    fn drag_step(&mut self, cx: &mut EditorContext, to: Point) {
        let fi = cx.floor;
        let (id, began, delta, idx) = match &self.gesture.0 {
            Gesture::Move { id, last, began } => (*id, *began, Some(to.sub(*last)), 0),
            Gesture::Vertex { id, idx, began } => (*id, *began, None, *idx),
            _ => return,
        };
        if let Some(d) = delta {
            if d.length() < 1e-9 {
                return;
            }
        }
        if !began {
            cx.begin_change(if delta.is_some() {
                "Move Roof Plane"
            } else {
                "Reshape Roof Plane"
            });
        }
        let mut set = load(&cx.project.floors[fi]);
        if let Some(r) = set.plane_mut(id) {
            match delta {
                Some(d) => r.translate(d),
                None => r.move_vertex(idx, to),
            }
        }
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        match &mut self.gesture.0 {
            Gesture::Move { last, began, .. } => {
                *last = to;
                *began = true;
            }
            Gesture::Vertex { began, .. } => *began = true,
            _ => {}
        }
    }
}

impl Tool for RoofTool {
    fn id(&self) -> ToolId {
        ToolId::Roof
    }

    fn name(&self) -> &'static str {
        "Roof"
    }

    fn hint(&self) -> String {
        self.mode.get().hint().to_string()
    }

    fn cursor(&self) -> egui::CursorIcon {
        match self.mode.get() {
            RoofMode::Edit => egui::CursorIcon::Default,
            _ => egui::CursorIcon::Crosshair,
        }
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::RoofVariant(m) = id {
            self.set_mode(m);
            if m == RoofMode::Build {
                self.cmds.borrow_mut().push(Cmd::OpenBuild);
            }
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        // Palette and dialog requests apply right away, not on the next event.
        self.begin_event(cx);
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.gesture.0 = Gesture::None;
        self.join_first = None;
        self.selected = None;
        self.begin_event(cx);
        cx.status = self.mode.get().hint().to_string();
    }

    fn deactivate(&mut self, cx: &mut EditorContext) {
        self.gesture.0 = Gesture::None;
        self.join_first = None;
        self.hover = None;
        *self.build_dialog.borrow_mut() = None;
        *self.plane_dialog.borrow_mut() = None;
        cx.readout = None;
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        if self.dialogs_open() {
            return Self::with_pre(pre, ToolResult::consumed());
        }
        let mut res = ToolResult::consumed();
        match self.mode.get() {
            RoofMode::Plane => {
                match std::mem::replace(&mut self.gesture.0, Gesture::None) {
                    Gesture::Draft { a, b } => {
                        if self.create_plane(cx, a, b, p.snapped) {
                            res = ToolResult::committed("Draw Roof Plane");
                        } else {
                            // Stay on the draft so the user can click again.
                            self.gesture.0 = Gesture::Draft { a, b };
                        }
                    }
                    _ => {
                        self.gesture.0 = Gesture::Baseline {
                            from: p.snapped,
                            to: p.snapped,
                        };
                    }
                }
            }
            RoofMode::Edit => self.press_edit(cx, &p),
            RoofMode::Build => self.open_build(cx),
            RoofMode::GableLine => {
                if self.gable_line(cx, p.world) {
                    res = ToolResult::committed("Gable/Roof Line");
                }
            }
            RoofMode::Hole => {
                self.gesture.0 = Gesture::Rect {
                    from: p.snapped,
                    to: p.snapped,
                };
            }
            RoofMode::Skylight => {
                if self.place_skylight(cx, p.world) {
                    res = ToolResult::committed("Place Skylight");
                }
            }
            RoofMode::Join => {
                let set = load(cx.floor());
                match (set.plane_at(p.world), self.join_first) {
                    (None, _) => cx.status = "Click a roof plane".into(),
                    (Some(a), None) => {
                        self.join_first = Some(a);
                        cx.status = "Join Roof Planes: now click the second plane".into();
                    }
                    (Some(b), Some(a)) => {
                        self.join_first = None;
                        cx.status =
                            format!("Join Roof Planes (planes {a} and {b}): not implemented yet");
                    }
                }
            }
            RoofMode::Dormer => cx.status = "Auto Dormer: not implemented yet".into(),
        }
        Self::with_pre(pre, res)
    }

    fn pointer_move(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        self.hover = Some(p.snapped);
        cx.last_snap = Some(p.snap);
        match &mut self.gesture.0 {
            Gesture::Baseline { from, to } => {
                *to = p.snapped;
                cx.readout = Some(format!("Length: {}", cx.fmt_dim(from.dist(*to))));
            }
            Gesture::Rect { to, .. } => *to = p.snapped,
            Gesture::Draft { a, b } => {
                let d = p.snapped.sub(*a).dot(b.sub(*a).normalized().perp()).abs();
                cx.readout = Some(format!("Run: {}", cx.fmt_dim(d)));
            }
            Gesture::Move { .. } | Gesture::Vertex { .. } if p.down => {
                self.drag_step(cx, p.snapped);
            }
            _ => {}
        }
        Self::with_pre(
            pre,
            ToolResult {
                repaint: true,
                ..ToolResult::default()
            },
        )
    }

    fn pointer_up(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        let mut res = ToolResult::consumed();
        match std::mem::replace(&mut self.gesture.0, Gesture::None) {
            Gesture::Baseline { from, .. } => {
                let to = p.snapped;
                if from.dist(to) >= MIN_BASELINE {
                    self.gesture.0 = Gesture::Draft { a: from, b: to };
                    cx.status = "Click on the side the roof plane rises to".into();
                } else {
                    cx.readout = None;
                }
            }
            Gesture::Rect { from, .. } => {
                if self.make_hole(cx, from, p.snapped) {
                    res = ToolResult::committed("Roof Hole");
                }
            }
            Gesture::Move { id, began, .. } => {
                if began {
                    res = ToolResult::committed("Move Roof Plane");
                } else {
                    self.selected = Some(id);
                }
            }
            Gesture::Vertex { began, .. } => {
                if began {
                    res = ToolResult::committed("Reshape Roof Plane");
                }
            }
            other @ Gesture::Draft { .. } => self.gesture.0 = other,
            Gesture::None => return Self::with_pre(pre, ToolResult::ignored()),
        }
        Self::with_pre(pre, res)
    }

    fn double_click(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        if self.mode.get() == RoofMode::Edit {
            if let Some(id) = load(cx.floor()).plane_at(p.world) {
                self.selected = Some(id);
                self.open_plane(cx, id);
                return Self::with_pre(pre, ToolResult::consumed());
            }
        }
        Self::with_pre(pre, ToolResult::ignored())
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        if self.dialogs_open() {
            // The dialogs take Esc / Enter themselves.
            return Self::with_pre(pre, ToolResult::consumed());
        }
        let busy = !matches!(self.gesture.0, Gesture::None) || self.join_first.is_some();
        if k.is(egui::Key::Escape) {
            if busy || self.selected.is_some() {
                self.gesture.0 = Gesture::None;
                self.join_first = None;
                self.selected = None;
                cx.readout = None;
                return Self::with_pre(pre, ToolResult::consumed());
            }
        } else if k.is(egui::Key::Delete) || k.is(egui::Key::Backspace) {
            if let Some(id) = self.selected {
                if let Some(l) = self.delete_plane(cx, id) {
                    return Self::with_pre(pre, ToolResult::committed(&l));
                }
            }
        } else if k.is(egui::Key::Enter) {
            if let Some(id) = self.selected {
                self.open_plane(cx, id);
                return Self::with_pre(pre, ToolResult::consumed());
            }
        }
        Self::with_pre(pre, ToolResult::ignored())
    }

    fn edit_toolbar(&self, _cx: &EditorContext) -> Vec<EditAction> {
        // The shell applies these to `cx.selection`, which cannot hold roof
        // planes yet (`ObjectRef::exists` does not know them); the palette
        // has working Open/Delete buttons meanwhile.
        if self.selected.is_none() {
            return Vec::new();
        }
        vec![
            EditAction::new(EditActionKind::OpenObject),
            EditAction::new(EditActionKind::Delete),
        ]
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        let pal = &cx.palette;
        let accent = pal.selection;
        let ghost = Stroke::new(1.5_f32, pal.ghost_stroke);
        let scr = |p: Point| cam.world_to_screen(p);
        match &self.gesture.0 {
            Gesture::Baseline { from, to } => {
                painter.line_segment([scr(*from), scr(*to)], Stroke::new(2.5_f32, accent));
                painter.text(
                    scr(Point::lerp(*from, *to, 0.5)) + Vec2::new(0.0, -10.0),
                    Align2::CENTER_CENTER,
                    cx.fmt_dim(from.dist(*to)),
                    FontId::proportional(13.0),
                    pal.dimension_text,
                );
            }
            Gesture::Draft { a, b } => {
                painter.line_segment([scr(*a), scr(*b)], Stroke::new(2.5_f32, accent));
                if let Some(h) = self.hover {
                    let (pitch, elev) = (8.0, 0.0);
                    if let Some((_, poly)) = manual_plane_geometry(*a, *b, h, elev, pitch) {
                        let pts: Vec<Pos2> =
                            poly.iter().map(|v| scr(Point::new(v[0], -v[2]))).collect();
                        painter.add(egui::Shape::convex_polygon(pts, pal.ghost_fill, ghost));
                    }
                }
            }
            Gesture::Rect { from, to } => {
                let pts: Vec<Pos2> = rect_polygon(*from, *to).iter().map(|q| scr(*q)).collect();
                painter.add(egui::Shape::closed_line(pts, ghost));
            }
            _ => {}
        }
        if let Some(id) = self.selected {
            if let Some(rec) = load(cx.floor()).plane(id) {
                let poly = rec.plan_polygon();
                let pts: Vec<Pos2> = poly.iter().map(|q| scr(*q)).collect();
                painter.add(egui::Shape::closed_line(
                    pts.clone(),
                    Stroke::new(3.0_f32, accent),
                ));
                for p in &pts {
                    painter.rect_filled(
                        egui::Rect::from_center_size(*p, Vec2::splat(7.0)),
                        0.0,
                        accent,
                    );
                }
                painter.circle_stroke(scr(rec.centroid()), 5.0, Stroke::new(1.5_f32, accent));
            }
        }
        self.palette(painter, cam);
        self.dialogs(painter.ctx());
    }
}

impl RoofTool {
    /// The mode palette in the top-left corner of the canvas.
    fn palette(&self, painter: &egui::Painter, cam: &Camera) {
        let ctx = painter.ctx();
        egui::Area::new(egui::Id::new("roof_palette"))
            .fixed_pos(cam.rect.left_top() + Vec2::new(10.0, 10.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong("Roof");
                    for m in RoofMode::ALL {
                        let on = self.mode.get() == m;
                        if ui.selectable_label(on, m.label()).clicked() && !on {
                            self.mode.set(m);
                            self.reset.set(true);
                            if m == RoofMode::Build {
                                self.cmds.borrow_mut().push(Cmd::OpenBuild);
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Rebuild Roofs").clicked() {
                        self.cmds.borrow_mut().push(Cmd::Rebuild);
                    }
                    if ui.button("Delete Roof Planes").clicked() {
                        self.cmds.borrow_mut().push(Cmd::DeleteAll);
                    }
                    if let Some(id) = self.selected {
                        ui.separator();
                        if ui.button("Open Selected Plane").clicked() {
                            self.cmds.borrow_mut().push(Cmd::OpenPlane(id));
                        }
                        if ui.button("Delete Selected Plane").clicked() {
                            self.cmds.borrow_mut().push(Cmd::DeletePlane(id));
                        }
                    }
                });
            });
    }

    /// Draws the open dialogs and queues what OK asked for.
    fn dialogs(&self, ctx: &egui::Context) {
        let mut build_done = None;
        if let Some(d) = self.build_dialog.borrow_mut().as_mut() {
            match d.show(ctx) {
                Outcome::Ok => build_done = Some(Some(d.settings().clone())),
                Outcome::Cancel => build_done = Some(None),
                Outcome::Open => {}
            }
        }
        if let Some(done) = build_done {
            *self.build_dialog.borrow_mut() = None;
            if let Some(s) = done {
                self.cmds.borrow_mut().push(Cmd::ApplyBuild(s));
            }
            ctx.request_repaint();
        }
        let mut plane_done = None;
        if let Some((id, d)) = self.plane_dialog.borrow_mut().as_mut() {
            match d.show(ctx) {
                Outcome::Ok => plane_done = Some(Some((*id, d.draft().clone()))),
                Outcome::Cancel => plane_done = Some(None),
                Outcome::Open => {}
            }
        }
        if let Some(done) = plane_done {
            *self.plane_dialog.borrow_mut() = None;
            if let Some((id, rec)) = done {
                self.cmds
                    .borrow_mut()
                    .push(Cmd::ApplyPlane(id, Box::new(rec)));
            }
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{Floor, Project};

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    /// 40' x 24' of exterior walls.
    fn house(cx: &mut EditorContext) {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        cx.mark_dirty();
        cx.refresh();
    }

    fn click(t: &mut RoofTool, cx: &mut EditorContext, x: f64, y: f64) {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true));
        t.pointer_up(cx, p);
    }

    fn drag(t: &mut RoofTool, cx: &mut EditorContext, from: (f64, f64), to: (f64, f64)) {
        let a = PointerEvent::at(cx, Point::new(from.0, from.1));
        let b = PointerEvent::at(cx, Point::new(to.0, to.1));
        t.pointer_move(cx, a);
        t.pointer_down(cx, a.with_down(true));
        t.pointer_move(cx, b.with_down(true));
        t.pointer_up(cx, b);
    }

    fn planes(cx: &EditorContext) -> Vec<RoofPlaneRecord> {
        load(cx.floor()).planes
    }

    fn build_default(t: &mut RoofTool, cx: &mut EditorContext) {
        let s = RoofSettings::from_defaults(&cx.defaults);
        assert!(t.build(cx, s).is_some());
    }

    #[test]
    fn build_roof_over_a_rectangle_makes_four_auto_planes() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        let p = planes(&cx);
        assert_eq!(p.len(), 4);
        assert!(p.iter().all(|r| r.auto));
        assert_eq!(cx.undo_label(), Some("Build Roof"));
        assert!(cx.status.starts_with("Built 4 roof planes"));
    }

    #[test]
    fn build_dialog_ok_is_applied_on_the_next_event() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        let s = RoofSettings::from_defaults(&cx.defaults);
        t.cmds.borrow_mut().push(Cmd::ApplyBuild(s));
        let p = PointerEvent::at(&cx, Point::new(10.0, 10.0));
        let res = t.pointer_move(&mut cx, p);
        assert_eq!(res.commit.as_deref(), Some("Build Roof"));
        assert_eq!(planes(&cx).len(), 4);
    }

    #[test]
    fn build_on_an_empty_plan_reports_and_changes_nothing() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        let s = RoofSettings::from_defaults(&cx.defaults);
        assert!(t.build(&mut cx, s).is_none());
        assert!(cx.status.contains("do not enclose"));
        assert!(!cx.can_undo());
        assert!(planes(&cx).is_empty());
    }

    #[test]
    fn gable_line_turns_a_short_wall_into_a_gable_and_rebuilds() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        t.set_mode(RoofMode::GableLine);
        // The right (short, 24') wall.
        click(&mut t, &mut cx, 480.0, 144.0);
        assert_eq!(planes(&cx).len(), 3);
        assert!(cx
            .floor()
            .walls
            .iter()
            .any(|w| w.roof.kind == RoofWallKind::FullGable));
        // Clicking it again turns it back to a hip.
        click(&mut t, &mut cx, 480.0, 144.0);
        assert_eq!(planes(&cx).len(), 4);
        // Undo goes back one toggle, roof planes included.
        cx.undo();
        assert_eq!(planes(&cx).len(), 3);
    }

    #[test]
    fn gable_line_without_a_roof_only_marks_the_wall() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        t.set_mode(RoofMode::GableLine);
        click(&mut t, &mut cx, 480.0, 144.0);
        assert!(planes(&cx).is_empty());
        assert!(cx.status.contains("Build Roof"));
    }

    #[test]
    fn auto_rebuild_follows_wall_changes_and_keeps_manual_planes() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        // A manual plane far away.
        t.set_mode(RoofMode::Plane);
        drag(&mut t, &mut cx, (1000.0, 0.0), (1120.0, 0.0));
        click(&mut t, &mut cx, 1060.0, 60.0);
        assert_eq!(planes(&cx).len(), 5);
        // Make the house longer: the auto roof follows, the manual one stays.
        let before: Vec<f64> = planes(&cx)
            .iter()
            .filter(|r| r.auto)
            .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
            .collect();
        let ids: Vec<Id> = cx.floor().walls.iter().map(|w| w.id).collect();
        for id in ids {
            let fl = cx.floor;
            if let Some(w) = cx.project.floors[fl].wall_mut(id) {
                if w.start.x > 100.0 {
                    w.start.x += 120.0;
                }
                if w.end.x > 100.0 {
                    w.end.x += 120.0;
                }
            }
        }
        assert!(auto_rebuild(&mut cx));
        let after = planes(&cx);
        assert_eq!(after.len(), 5);
        assert_eq!(after.iter().filter(|r| !r.auto).count(), 1);
        let max_before = before.iter().cloned().fold(f64::MIN, f64::max);
        let max_after = after
            .iter()
            .filter(|r| r.auto)
            .flat_map(|r| r.polygon3d.iter().map(|v| v[0]))
            .fold(f64::MIN, f64::max);
        assert!((max_after - max_before - 120.0).abs() < 1e-6);
        // Unchanged walls: nothing to do.
        assert!(!auto_rebuild(&mut cx));
        // Auto Rebuild off: walls change, roof does not.
        let mut s = load(cx.floor()).settings.unwrap();
        s.auto_rebuild = false;
        assert!(t.build(&mut cx, s).is_some());
        let id = cx.floor().walls[0].id;
        let fl = cx.floor;
        cx.project.floors[fl].wall_mut(id).unwrap().height += 10.0;
        assert!(!auto_rebuild(&mut cx));
    }

    #[test]
    fn manual_plane_from_a_drag_and_a_click() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        t.set_mode(RoofMode::Plane);
        drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
        assert!(planes(&cx).is_empty());
        click(&mut t, &mut cx, 60.0, 60.0);
        let p = planes(&cx);
        assert_eq!(p.len(), 1);
        let r = &p[0];
        assert!(!r.auto);
        assert_eq!(r.pitch, 8.0);
        assert_eq!(r.baseline, (Point::new(0.0, 0.0), Point::new(120.0, 0.0)));
        // Rises 60" * 8/12 = 40" from the eave.
        let eave = r.polygon3d[0][1];
        assert!((r.polygon3d[2][1] - eave - 40.0).abs() < 1e-9);
        assert_eq!(cx.undo_label(), Some("Draw Roof Plane"));
        assert_eq!(t.selected(), Some(r.id));
        // Undo removes it.
        cx.undo();
        assert!(planes(&cx).is_empty());
    }

    #[test]
    fn a_short_baseline_drag_makes_nothing() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (6.0, 0.0));
        click(&mut t, &mut cx, 60.0, 60.0);
        assert!(planes(&cx).is_empty());
    }

    fn with_plane(cx: &mut EditorContext, t: &mut RoofTool) {
        let before = planes(cx).len();
        t.set_mode(RoofMode::Plane);
        drag(t, cx, (0.0, 0.0), (240.0, 0.0));
        click(t, cx, 120.0, 120.0);
        assert_eq!(planes(cx).len(), before + 1);
    }

    #[test]
    fn hole_and_skylight_are_stored_on_the_plane() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Hole);
        drag(&mut t, &mut cx, (60.0, 36.0), (96.0, 72.0));
        let r = &planes(&cx)[0];
        let hole = r.hole.as_ref().unwrap();
        assert_eq!(hole.len(), 4);
        assert_eq!(hole[0], Point::new(60.0, 36.0));
        assert_eq!(cx.undo_label(), Some("Roof Hole"));
        t.set_mode(RoofMode::Skylight);
        click(&mut t, &mut cx, 180.0, 60.0);
        let r = &planes(&cx)[0];
        assert_eq!(r.skylights, vec![(Point::new(180.0, 60.0), 24.0, 48.0)]);
        // Outside every plane: nothing is added and no undo step is made.
        click(&mut t, &mut cx, 900.0, 900.0);
        assert_eq!(planes(&cx)[0].skylights.len(), 1);
        assert_eq!(cx.undo_label(), Some("Place Skylight"));
        // A hole outside the plane is refused.
        t.set_mode(RoofMode::Hole);
        drag(&mut t, &mut cx, (600.0, 600.0), (660.0, 660.0));
        assert_eq!(
            planes(&cx)[0].hole.as_ref().unwrap()[0],
            Point::new(60.0, 36.0)
        );
    }

    #[test]
    fn edit_mode_selects_moves_reshapes_and_deletes() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Edit);
        click(&mut t, &mut cx, 120.0, 60.0);
        let id = planes(&cx)[0].id;
        assert_eq!(t.selected(), Some(id));
        // Move by (24, 12).
        drag(&mut t, &mut cx, (120.0, 60.0), (144.0, 72.0));
        let r = planes(&cx).remove(0);
        assert_eq!(r.baseline.0, Point::new(24.0, 12.0));
        assert!(!r.auto);
        assert_eq!(cx.undo_label(), Some("Move Roof Plane"));
        // Reshape: drag the ridge corner (index 2).
        let corner = r.plan_polygon()[2];
        drag(
            &mut t,
            &mut cx,
            (corner.x, corner.y),
            (corner.x + 48.0, corner.y + 24.0),
        );
        let r2 = planes(&cx).remove(0);
        assert_eq!(
            r2.plan_polygon()[2],
            Point::new(corner.x + 48.0, corner.y + 24.0)
        );
        assert_eq!(cx.undo_label(), Some("Reshape Roof Plane"));
        // Still planar at 8:12 from the baseline.
        let up = r2.up_slope();
        let d = r2.plan_polygon()[2].sub(r2.baseline.0).dot(up);
        assert!((r2.polygon3d[2][1] - r2.baseline_height() - d * 8.0 / 12.0).abs() < 1e-9);
        // Delete.
        let res = t.key(&mut cx, KeyEvent::key(egui::Key::Delete));
        assert_eq!(res.commit.as_deref(), Some("Delete Roof Plane"));
        assert!(planes(&cx).is_empty());
        cx.undo();
        assert_eq!(planes(&cx).len(), 1);
    }

    #[test]
    fn delete_roof_planes_clears_everything_in_one_undo_step() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        t.cmds.borrow_mut().push(Cmd::DeleteAll);
        let p = PointerEvent::at(&cx, Point::new(1.0, 1.0));
        t.pointer_move(&mut cx, p);
        assert!(planes(&cx).is_empty());
        assert!(load(cx.floor()).settings.is_none());
        // Auto rebuild no longer revives it.
        assert!(!auto_rebuild(&mut cx));
        cx.undo();
        assert_eq!(planes(&cx).len(), 4);
        // Nothing to delete: no undo step.
        let mut cx2 = new_cx();
        let mut t2 = RoofTool::default();
        assert!(t2.delete_all_planes(&mut cx2).is_none());
        assert!(!cx2.can_undo());
    }

    #[test]
    fn plane_dialog_edits_are_applied_and_make_the_plane_manual() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        let old = planes(&cx).remove(0);
        let mut edited = old.clone();
        edited.set_pitch(12.0);
        edited.label = "Main".into();
        t.cmds
            .borrow_mut()
            .push(Cmd::ApplyPlane(old.id, Box::new(edited)));
        let p = PointerEvent::at(&cx, Point::new(1.0, 1.0));
        t.pointer_move(&mut cx, p);
        let now = planes(&cx).into_iter().find(|r| r.id == old.id).unwrap();
        assert_eq!(now.pitch, 12.0);
        assert_eq!(now.label, "Main");
        assert!(!now.auto);
        assert!(now
            .polygon3d
            .iter()
            .any(|v| v[1] > old.polygon3d.iter().map(|o| o[1]).fold(0.0, f64::max)));
        // The edited plane survives an automatic rebuild untouched.
        let s = load(cx.floor()).settings.unwrap();
        let fl = cx.floor;
        rebuild(&mut cx.project, fl, s, false).unwrap();
        assert_eq!(planes(&cx).len(), 5);
    }

    #[test]
    fn roof_set_survives_save_and_load() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        with_plane(&mut cx, &mut t);
        let json = serde_json::to_string(&cx.project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(load(&back.floors[0]), load(cx.floor()));
        assert_eq!(load(&back.floors[0]).planes.len(), 5);
    }

    #[test]
    fn escape_cancels_a_draft_then_leaves_the_tool() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        drag(&mut t, &mut cx, (0.0, 0.0), (120.0, 0.0));
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        click(&mut t, &mut cx, 60.0, 60.0);
        assert!(planes(&cx).is_empty());
        assert!(!t.key(&mut cx, KeyEvent::escape()).consumed);
    }

    #[test]
    fn join_and_dormer_are_placeholders() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Join);
        click(&mut t, &mut cx, 120.0, 60.0);
        click(&mut t, &mut cx, 120.0, 90.0);
        assert!(cx.status.contains("not implemented"));
        t.set_mode(RoofMode::Dormer);
        click(&mut t, &mut cx, 120.0, 60.0);
        assert!(cx.status.contains("Auto Dormer"));
        assert_eq!(planes(&cx).len(), 1);
    }

    #[test]
    fn roof_follows_the_floor_it_is_built_over() {
        let mut cx = new_cx();
        house(&mut cx);
        cx.project.floors.push(Floor::new("2nd Floor", 109.0));
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 144.0),
            Point::new(0.0, 144.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(1, pts[i], pts[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior);
        }
        let mut t = RoofTool::default();
        let mut s = RoofSettings::from_defaults(&cx.defaults);
        assert!(t.build(&mut cx, s.clone()).is_some());
        assert_eq!(load(&cx.project.floors[1]).planes.len(), 4);
        assert!(load(&cx.project.floors[0]).planes.is_empty());
        // Eave: 2nd floor elevation + wall height.
        let eave = load(&cx.project.floors[1]).planes[0].baseline_height();
        assert!((eave - 218.0).abs() < 1e-6);
        // Ignore Top Floor moves it down.
        s.ignore_top_floor = true;
        assert!(t.build(&mut cx, s).is_some());
        assert!(load(&cx.project.floors[1]).planes.is_empty());
        assert_eq!(load(&cx.project.floors[0]).planes.len(), 4);
    }
}
