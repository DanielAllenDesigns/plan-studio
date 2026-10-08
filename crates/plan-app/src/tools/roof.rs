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
//! * **Gable/Roof Line** (RF-18, RF-20, RF-44): click near an eave: an
//!   automatic plane's edge becomes a gable end (a Build Roof edge override
//!   and a rebuild), a manual one goes through `plan_roof::apply_gable_line`;
//!   clicking an exterior wall still flips it between Hip and Full Gable;
//! * **Roof Hole** (RF-42), **Skylight** (RF-43): drag a rectangle inside a
//!   plane (a click places a default skylight);
//! * **Ceiling Plane** (RF-45): like Roof Plane, a vaulted ceiling plane on
//!   layer "Ceiling Planes";
//! * **Auto Dormer** (RF-48): click a plane, set the dimensions in the
//!   Dormer Specification; **Auto Floating Dormer** (RF-49) is the same
//!   without the hole in the roof under it; **Explode Dormer** (RF-51) turns
//!   a dormer into plain planes (its walls are not kept: a wall cannot start
//!   at the roof surface yet);
//! * **Join Roof Planes** (RF-41): click an edge of the first plane (or
//!   select a plane and start from the Edit toolbar), then click the second
//!   plane: the edge is extended or trimmed to the line where they meet;
//! * **Roof Return** (RF-27): click an eave corner (Shift: half return,
//!   Alt: boxed return);
//! * **Delete Roof Planes** (RF-40) and **Rebuild Roofs** as palette buttons.
//!
//! Dialogs are owned and drawn by the tool (from `draw_overlay`, through the
//! painter's egui context); what OK/buttons ask for is queued and applied on
//! the tool's next event, since drawing has no mutable access to the plan.
//!
//! Deferred: Dutch gable, knee wall and other directives beyond
//! Hip/Gable/Shed/Extend Slope Downward (RF-21..RF-25), framing (RF-52..).

use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::dialogs::roof::{BuildRoofDialog, DormerDialog, ReturnDialog, RoofPlaneDialog};
use crate::dialogs::Outcome;
use crate::editor::roof_view::{
    self, auto_rebuild, build_floor, delete_all, load, manual_plane_geometry, rebuild,
    rect_polygon, store, toggle_gable, RoofPlaneRecord, RoofSettings,
};
use crate::editor::{Camera, EditAction, EditActionKind, EditorContext};
use eframe::egui::{self, Align2, FontId, Pos2, Stroke, Vec2};
use plan_core::defaults::RoofWallKind;
use plan_core::geometry::{dist_to_segment, Point};
use plan_core::{Id, WallKind};
use plan_roof::{DormerSpec, ReturnKind, ReturnSpec};
use std::cell::{Cell, RefCell};

/// Planes need a baseline at least this long, inches.
const MIN_BASELINE: f64 = 12.0;
/// Holes are at least this big on each side, inches.
const MIN_HOLE: f64 = 6.0;
/// Length of the roof returns the Roof Return mode makes, inches.
const RETURN_LENGTH: f64 = roof_view::AUTO_RETURN_LENGTH;
/// Pitch of a new ceiling plane when no roof has been built, rise per 12.
const CEILING_PITCH: f64 = 4.0;

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
    FloatingDormer,
    Ceiling,
    Explode,
    Return,
}

impl RoofMode {
    const ALL: [RoofMode; 12] = [
        RoofMode::Plane,
        RoofMode::Edit,
        RoofMode::Build,
        RoofMode::Ceiling,
        RoofMode::GableLine,
        RoofMode::Hole,
        RoofMode::Skylight,
        RoofMode::Join,
        RoofMode::Dormer,
        RoofMode::FloatingDormer,
        RoofMode::Explode,
        RoofMode::Return,
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
            RoofMode::FloatingDormer => "Auto Floating Dormer",
            RoofMode::Ceiling => "Ceiling Plane",
            RoofMode::Explode => "Explode Dormer",
            RoofMode::Return => "Roof Return",
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
                "Gable/Roof Line: click an eave to make it a gable end (or an exterior wall to flip it)"
            }
            RoofMode::Hole => "Roof Hole: press and drag a rectangle inside a roof plane",
            RoofMode::Skylight => {
                "Skylight: press and drag a rectangle inside a roof plane, or click for 24 x 48"
            }
            RoofMode::Join => {
                "Join Roof Planes: click an edge of the first plane, then click the second plane"
            }
            RoofMode::Dormer => "Auto Dormer: click a roof plane, then set the dormer",
            RoofMode::FloatingDormer => {
                "Auto Floating Dormer: click a roof plane; the roof is not cut under the dormer"
            }
            RoofMode::Ceiling => {
                "Ceiling Plane: press and drag the baseline, then click toward the high side; Esc cancels"
            }
            RoofMode::Explode => "Explode Dormer: click a dormer",
            RoofMode::Return => {
                "Roof Return: click an eave corner (Shift: half return, Alt: boxed return)"
            }
        }
    }
}

/// The open Dormer Specification: the main plane, the dormer being edited if
/// any, whether a new dormer floats, and the dialog.
type DormerSlot = (Id, Option<Id>, bool, DormerDialog);

/// What the palette and dialogs ask for.
enum Cmd {
    OpenBuild,
    ApplyBuild(RoofSettings),
    ApplyPlane(Id, Box<RoofPlaneRecord>),
    DeleteAll,
    Rebuild,
    OpenPlane(Id),
    DeletePlane(Id),
    /// Dormer dialog OK: the plane, the dormer being edited if any, its size,
    /// and whether a new dormer is a floating one.
    ApplyDormer(Id, Option<Id>, DormerSpec, bool),
    /// Roof Return dialog OK: the type and length of the next returns.
    ApplyReturn(ReturnSpec),
    OpenReturn,
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
    /// Auto Dormer: the plane, the dormer being edited if any, whether a new
    /// dormer floats, the dialog.
    dormer_dialog: RefCell<Option<DormerSlot>>,
    /// The Roof Return settings dialog.
    return_dialog: RefCell<Option<ReturnDialog>>,
    /// What the Roof Return tool makes (set by that dialog); `None` is a
    /// full return of [`RETURN_LENGTH`].
    return_spec: Cell<Option<ReturnSpec>>,
    /// The palette changed mode: forget the gesture at the next event.
    reset: Cell<bool>,
    selected: Option<Id>,
    gesture: GestureSlot,
    hover: Option<Point>,
    /// Join Roof Planes: the first plane and its picked edge.
    join_first: Option<(Id, usize)>,
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
        self.build_dialog.borrow().is_some()
            || self.plane_dialog.borrow().is_some()
            || self.dormer_dialog.borrow().is_some()
            || self.return_dialog.borrow().is_some()
    }

    /// The type and length the Roof Return tool makes now.
    pub fn return_settings(&self) -> ReturnSpec {
        self.return_spec.get().unwrap_or(ReturnSpec {
            kind: ReturnKind::Full,
            length: RETURN_LENGTH,
        })
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
                Cmd::ApplyDormer(main, edit, spec, floating) => {
                    self.apply_dormer(cx, main, edit, spec, floating)
                }
                Cmd::ApplyReturn(spec) => {
                    self.return_spec.set(Some(spec));
                    cx.status = format!(
                        "Roof Return: {} return, {}",
                        match spec.kind {
                            ReturnKind::Full => "full",
                            ReturnKind::Half => "half",
                            ReturnKind::Boxed => "boxed",
                        },
                        cx.fmt_dim(spec.length)
                    );
                    None
                }
                Cmd::OpenReturn => {
                    if self.return_dialog.borrow().is_none() {
                        *self.return_dialog.borrow_mut() =
                            Some(ReturnDialog::new(self.return_settings()));
                    }
                    None
                }
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
        if !roof_view::exists(&cx.project.floors[fi], id) {
            return None;
        }
        cx.begin_change("Delete Roof Plane");
        roof_view::delete_records(&mut cx.project, fi, &[id]);
        self.selected = None;
        cx.mark_dirty();
        Some("Delete Roof Plane".into())
    }

    /// Opens the specification of plane or dormer `id`.
    fn open_plane(&mut self, cx: &EditorContext, id: Id) {
        let set = load(cx.floor());
        if let Some(rec) = set.plane(id).cloned() {
            let layers = cx
                .project
                .layers
                .layers
                .iter()
                .map(|l| l.name.clone())
                .collect();
            *self.plane_dialog.borrow_mut() = Some((id, RoofPlaneDialog::new(rec, layers)));
        } else if let Some(d) = set.dormer(id) {
            *self.dormer_dialog.borrow_mut() =
                Some((d.main, Some(id), d.floating, DormerDialog::new(d.spec)));
        }
    }

    fn apply_plane(
        &mut self,
        cx: &mut EditorContext,
        id: Id,
        new: &RoofPlaneRecord,
    ) -> Option<String> {
        let fi = cx.floor;
        load(&cx.project.floors[fi]).plane(id)?;
        cx.begin_change("Roof Plane Specification");
        roof_view::apply_plane_edit(&mut cx.project, fi, new);
        cx.mark_dirty();
        Some("Roof Plane Specification".into())
    }

    /// Auto Dormer / Dormer Specification OK (RF-48).
    fn apply_dormer(
        &mut self,
        cx: &mut EditorContext,
        main: Id,
        edit: Option<Id>,
        spec: DormerSpec,
        floating: bool,
    ) -> Option<String> {
        let fi = cx.floor;
        let label = match (edit.is_some(), floating) {
            (true, _) => "Dormer Specification",
            (false, true) => "Auto Floating Dormer",
            (false, false) => "Auto Dormer",
        };
        cx.begin_change(label);
        let done = if floating && edit.is_none() {
            roof_view::apply_floating_dormer(&mut cx.project, fi, main, edit, spec)
        } else {
            roof_view::apply_dormer(&mut cx.project, fi, main, edit, spec)
        };
        match done {
            Ok(id) => {
                self.selected = Some(id);
                cx.mark_dirty();
                cx.status = "Dormer made".into();
                Some(label.into())
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = format!("{label}: {e}");
                None
            }
        }
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

    /// Ceiling Plane (RF-45): the rectangle on baseline `a -> b` toward
    /// `toward`, at the floor's ceiling height.
    fn create_ceiling(
        &mut self,
        cx: &mut EditorContext,
        a: Point,
        b: Point,
        toward: Point,
    ) -> bool {
        let fi = cx.floor;
        let pitch = load(&cx.project.floors[fi])
            .settings
            .map_or(CEILING_PITCH, |s| s.pitch);
        let height = cx.floor().elevation + cx.floor().ceiling_height;
        cx.begin_change("Draw Ceiling Plane");
        match roof_view::add_ceiling(&mut cx.project, fi, (a, b, toward), height, pitch) {
            Ok(id) => {
                cx.mark_dirty();
                self.selected = Some(id);
                cx.status = format!("Ceiling plane at {} pitch", roof_view::pitch_label(pitch));
                true
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = e;
                false
            }
        }
    }

    /// Gable/Roof Line (RF-44): click near an eave. An automatic plane's
    /// edge becomes a gable end of the Build Roof settings (and the roof is
    /// rebuilt); manual planes go through `apply_gable_line`. Clicking an
    /// exterior wall instead flips it between Hip and Full Gable.
    fn gable_line(&mut self, cx: &mut EditorContext, at: Point) -> bool {
        let tol = cx.pick_tol();
        let set = load(cx.floor());
        if let Some(id) = roof_view::eave_near(&set, at, tol) {
            let auto = set.plane(id).is_some_and(|p| p.auto);
            let fi = cx.floor;
            cx.begin_change("Gable/Roof Line");
            let done = if auto {
                roof_view::set_edge_gable(&mut cx.project, fi, id)
                    .map(|()| "That edge is now a gable end".to_string())
            } else {
                roof_view::gable_line_manual(&mut cx.project, fi, id)
                    .map(|n| format!("Gable line made ({n} manual roof planes)"))
            };
            return match done {
                Ok(msg) => {
                    self.selected = None;
                    cx.mark_dirty();
                    cx.status = msg;
                    true
                }
                Err(e) => {
                    cx.cancel_change();
                    cx.status = format!("Gable/Roof Line: {e}");
                    false
                }
            };
        }
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
            cx.status = "Click an eave or an exterior wall".into();
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

    /// Roof Hole (RF-42) or Skylight (RF-43): the rectangle becomes a hole of
    /// the plane under its center. A skylight press without a drag places the
    /// default 24" x 48" one at `a`.
    fn make_hole(&mut self, cx: &mut EditorContext, a: Point, b: Point, skylight: bool) -> bool {
        let tiny = (a.x - b.x).abs() < MIN_HOLE || (a.y - b.y).abs() < MIN_HOLE;
        if tiny && !skylight {
            return false;
        }
        let fi = cx.floor;
        let label = if skylight {
            "Place Skylight"
        } else {
            "Roof Hole"
        };
        cx.begin_change(label);
        let done = if tiny {
            roof_view::place_skylight(&mut cx.project, fi, a)
        } else {
            roof_view::add_hole(&mut cx.project, fi, a, b, skylight)
        };
        match done {
            Ok(_) => {
                cx.mark_dirty();
                cx.status = if skylight {
                    "Skylight placed".into()
                } else {
                    "Roof hole made".into()
                };
                true
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = e;
                false
            }
        }
    }

    /// Explode Dormer (RF-51).
    fn explode(&mut self, cx: &mut EditorContext, at: Point) -> bool {
        let fi = cx.floor;
        let Some(id) = load(cx.floor()).dormer_at(at) else {
            cx.status = "Click a dormer".into();
            return false;
        };
        cx.begin_change("Explode Dormer");
        match roof_view::explode_dormer_record(&mut cx.project, fi, id, &cx.defaults) {
            Ok(done) => {
                self.selected = None;
                cx.mark_dirty();
                cx.status = format!(
                    "Exploded the dormer into {} roof planes and {} walls",
                    done.planes, done.walls
                );
                true
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = format!("Explode Dormer: {e}");
                false
            }
        }
    }

    /// Join Roof Planes (RF-41). The first click picks an edge of the first
    /// plane (of the plane selected on the Edit toolbar when it has one near
    /// the click); the second click on another plane joins the edge to it.
    /// Returns whether a change was made.
    fn join_click(&mut self, cx: &mut EditorContext, at: Point) -> bool {
        let set = load(cx.floor());
        let Some((a, edge)) = self.join_first else {
            let tol = cx.pick_tol() * 2.0;
            let preferred = match cx.selection.single() {
                Some(crate::editor::ObjectRef::RoofPlane(id)) if set.plane(id).is_some() => {
                    Some(id)
                }
                _ => None,
            };
            let found = preferred
                .and_then(|id| roof_view::edge_near(&set, at, tol, Some(id)))
                .or_else(|| roof_view::edge_near(&set, at, tol, None));
            match found {
                Some((id, e)) => {
                    self.join_first = Some((id, e));
                    self.selected = Some(id);
                    cx.status = "Join Roof Planes: now click the plane to join to".into();
                }
                None => cx.status = "Join Roof Planes: click an edge of the first plane".into(),
            }
            return false;
        };
        let Some(b) = set.plane_at(at).filter(|b| *b != a) else {
            cx.status = "Join Roof Planes: click a different roof plane".into();
            return false;
        };
        self.join_first = None;
        let fi = cx.floor;
        cx.begin_change("Join Roof Planes");
        match roof_view::join_planes_record(&mut cx.project, fi, a, edge, b) {
            Ok(()) => {
                cx.mark_dirty();
                cx.status = "Roof planes joined".into();
                true
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = format!("Join Roof Planes: {e}");
                false
            }
        }
    }

    /// Auto Roof Return (RF-27) at the eave corner near `at`.
    fn roof_return(&mut self, cx: &mut EditorContext, at: Point, kind: ReturnKind) -> bool {
        let fi = cx.floor;
        let tol = cx.pick_tol() * 2.0;
        let Some((id, at_start)) = roof_view::eave_corner_near(&load(cx.floor()), at, tol) else {
            cx.status = "Click an eave corner".into();
            return false;
        };
        cx.begin_change("Roof Return");
        let spec = ReturnSpec {
            kind,
            length: self.return_settings().length,
        };
        match roof_view::add_return(&mut cx.project, fi, id, at_start, spec) {
            Ok(new) => {
                self.selected = Some(new);
                cx.mark_dirty();
                cx.status = "Roof return made".into();
                true
            }
            Err(e) => {
                cx.cancel_change();
                cx.status = format!("Roof Return: {e}");
                false
            }
        }
    }

    /// Edit mode press: a vertex handle of the selected plane, else a record
    /// (dormer, plane or ceiling plane).
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
        match set.record_at(p.world) {
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
        match delta {
            Some(d) => {
                roof_view::translate_record(&mut set, id, d);
            }
            None => {
                if let Some(r) = set.plane_mut(id) {
                    r.move_vertex(idx, to);
                }
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
        self.mode.get().label()
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
        *self.dormer_dialog.borrow_mut() = None;
        *self.return_dialog.borrow_mut() = None;
        cx.readout = None;
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        let pre = self.begin_event(cx);
        if self.dialogs_open() {
            return Self::with_pre(pre, ToolResult::consumed());
        }
        let mut res = ToolResult::consumed();
        let mode = self.mode.get();
        match mode {
            RoofMode::Plane | RoofMode::Ceiling => {
                match std::mem::replace(&mut self.gesture.0, Gesture::None) {
                    Gesture::Draft { a, b } => {
                        let (made, label) = if mode == RoofMode::Ceiling {
                            (
                                self.create_ceiling(cx, a, b, p.snapped),
                                "Draw Ceiling Plane",
                            )
                        } else {
                            (self.create_plane(cx, a, b, p.snapped), "Draw Roof Plane")
                        };
                        if made {
                            res = ToolResult::committed(label);
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
            RoofMode::Hole | RoofMode::Skylight => {
                self.gesture.0 = Gesture::Rect {
                    from: p.snapped,
                    to: p.snapped,
                };
            }
            RoofMode::Join => {
                if self.join_click(cx, p.world) {
                    res = ToolResult::committed("Join Roof Planes");
                }
            }
            RoofMode::Dormer | RoofMode::FloatingDormer => {
                let floating = mode == RoofMode::FloatingDormer;
                let set = load(cx.floor());
                match set.plane_at(p.world) {
                    Some(id) => {
                        let spec = roof_view::dormer_spec_at(&set, id, p.world);
                        *self.dormer_dialog.borrow_mut() =
                            Some((id, None, floating, DormerDialog::new(spec)));
                    }
                    None => cx.status = format!("{}: click inside a roof plane", mode.label()),
                }
            }
            RoofMode::Explode => {
                if self.explode(cx, p.world) {
                    res = ToolResult::committed("Explode Dormer");
                }
            }
            RoofMode::Return => {
                let kind = if p.modifiers.shift {
                    ReturnKind::Half
                } else if p.modifiers.alt {
                    ReturnKind::Boxed
                } else {
                    self.return_settings().kind
                };
                if self.roof_return(cx, p.world, kind) {
                    res = ToolResult::committed("Roof Return");
                }
            }
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
                let skylight = self.mode.get() == RoofMode::Skylight;
                if self.make_hole(cx, from, p.snapped, skylight) {
                    res = ToolResult::committed(if skylight {
                        "Place Skylight"
                    } else {
                        "Roof Hole"
                    });
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
            if let Some(id) = load(cx.floor()).record_at(p.world) {
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
        if let Some((id, edge)) = self.join_first {
            if let Some(rec) = load(cx.floor()).plane(id) {
                let poly = rec.plan_polygon();
                let n = poly.len();
                if edge < n {
                    painter.line_segment(
                        [scr(poly[edge]), scr(poly[(edge + 1) % n])],
                        Stroke::new(4.5_f32, accent),
                    );
                }
            }
        }
        if let Some(id) = self.selected {
            let set = load(cx.floor());
            for (rid, _, polys) in set.pick_polys() {
                if rid != id {
                    continue;
                }
                for poly in polys {
                    let pts: Vec<Pos2> = poly.iter().map(|q| scr(*q)).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(3.0_f32, accent)));
                }
            }
            if let Some(rec) = set.plane(id) {
                for q in rec.plan_polygon() {
                    painter.rect_filled(
                        egui::Rect::from_center_size(scr(q), Vec2::splat(7.0)),
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
                    if self.mode.get() == RoofMode::Return
                        && ui.button("Roof Return Settings...").clicked()
                    {
                        self.cmds.borrow_mut().push(Cmd::OpenReturn);
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
        let mut dormer_done = None;
        if let Some((main, edit, floating, d)) = self.dormer_dialog.borrow_mut().as_mut() {
            match d.show(ctx) {
                Outcome::Ok => dormer_done = Some(Some((*main, *edit, d.spec(), *floating))),
                Outcome::Cancel => dormer_done = Some(None),
                Outcome::Open => {}
            }
        }
        if let Some(done) = dormer_done {
            *self.dormer_dialog.borrow_mut() = None;
            if let Some((main, edit, spec, floating)) = done {
                self.cmds
                    .borrow_mut()
                    .push(Cmd::ApplyDormer(main, edit, spec, floating));
            }
            ctx.request_repaint();
        }
        let mut return_done = None;
        if let Some(d) = self.return_dialog.borrow_mut().as_mut() {
            match d.show(ctx) {
                Outcome::Ok => return_done = Some(Some(d.spec())),
                Outcome::Cancel => return_done = Some(None),
                Outcome::Open => {}
            }
        }
        if let Some(done) = return_done {
            *self.return_dialog.borrow_mut() = None;
            if let Some(spec) = done {
                self.cmds.borrow_mut().push(Cmd::ApplyReturn(spec));
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
        assert_eq!(r.holes.len(), 1);
        assert!(!r.holes[0].is_skylight());
        assert_eq!(r.holes[0].outline.len(), 4);
        assert_eq!(r.holes[0].outline[0], Point::new(60.0, 36.0));
        assert_eq!(cx.undo_label(), Some("Roof Hole"));
        t.set_mode(RoofMode::Skylight);
        // A click places the default 24" x 48" skylight, long side up the slope.
        click(&mut t, &mut cx, 180.0, 60.0);
        let r = &planes(&cx)[0];
        assert_eq!(r.holes.len(), 2);
        assert!(r.holes[1].is_skylight());
        let (w, l) = r.holes[1].size();
        assert!(
            (w - 24.0).abs() < 1e-9 && (l - 48.0).abs() < 1e-9,
            "{w} x {l}"
        );
        // Outside every plane: nothing is added and no undo step is made.
        click(&mut t, &mut cx, 900.0, 900.0);
        assert_eq!(planes(&cx)[0].holes.len(), 2);
        assert_eq!(cx.undo_label(), Some("Place Skylight"));
        // A hole outside the plane is refused.
        t.set_mode(RoofMode::Hole);
        drag(&mut t, &mut cx, (600.0, 600.0), (660.0, 660.0));
        assert_eq!(planes(&cx)[0].holes.len(), 2);
        // So is one that sticks out over the plane's edge.
        drag(&mut t, &mut cx, (200.0, 100.0), (300.0, 140.0));
        assert_eq!(planes(&cx)[0].holes.len(), 2);
        assert!(cx.status.contains("inside"));
    }

    #[test]
    fn a_dragged_skylight_takes_the_rectangle() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Skylight);
        drag(&mut t, &mut cx, (60.0, 36.0), (100.0, 96.0));
        let r = &planes(&cx)[0];
        assert!(r.holes[0].is_skylight());
        assert_eq!(r.holes[0].size(), (40.0, 60.0));
        assert_eq!(cx.undo_label(), Some("Place Skylight"));
    }

    #[test]
    fn a_skylight_cuts_the_roof_mesh_and_adds_glass() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        let plain = roof_view::roof_meshes(&cx.project);
        assert!(plain.iter().all(|m| m.material == plan_3d::Material::Roof));
        t.set_mode(RoofMode::Skylight);
        drag(&mut t, &mut cx, (96.0, 36.0), (144.0, 84.0));
        let meshes = roof_view::roof_meshes(&cx.project);
        let probe = Point::new(120.0, 60.0);
        // No roof-slab triangle covers the center of the opening any more.
        let covers = |ms: &[plan_3d::Mesh], mat: plan_3d::Material, up_only: bool| {
            ms.iter().filter(|m| m.material == mat).any(|m| {
                m.indices.chunks(3).any(|tri| {
                    let v = |i: u32| m.vertices[i as usize];
                    let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
                    if up_only && a.normal[1] < 0.5 {
                        return false;
                    }
                    let plan = |q: plan_3d::Vertex| {
                        Point::new(f64::from(q.position[0]), -f64::from(q.position[2]))
                    };
                    plan_core::geometry::point_in_polygon(probe, &[plan(a), plan(b), plan(c)])
                })
            })
        };
        assert!(covers(&plain, plan_3d::Material::Roof, true));
        assert!(
            !covers(&meshes, plan_3d::Material::Roof, true),
            "the hole is cut"
        );
        // The glass pane sits over the opening.
        assert!(covers(&meshes, plan_3d::Material::Glass, true));
        assert!(meshes.iter().any(|m| m.material == plan_3d::Material::Trim));
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

    /// Two manual planes facing each other across a gap: the south one (y 0
    /// to 100) and the north one (y 360 down to 260), both 8:12 from the same
    /// eave height, so they meet in a ridge at y = 180.
    fn facing_planes(cx: &mut EditorContext) -> (Id, Id) {
        let fi = cx.floor;
        let mut set = load(cx.floor());
        let ids = [cx.project.alloc_id(), cx.project.alloc_id()];
        for (id, a, b, toward) in [
            (
                ids[0],
                Point::new(0.0, 0.0),
                Point::new(480.0, 0.0),
                Point::new(240.0, 100.0),
            ),
            (
                ids[1],
                Point::new(480.0, 360.0),
                Point::new(0.0, 360.0),
                Point::new(240.0, 260.0),
            ),
        ] {
            let (base, poly) = manual_plane_geometry(a, b, toward, 100.0, 8.0).unwrap();
            set.planes.push(RoofPlaneRecord::new(id, poly, 8.0, base));
        }
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        cx.refresh();
        (ids[0], ids[1])
    }

    #[test]
    fn join_roof_planes_extends_an_edge_to_the_other_plane() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        let (a, b) = facing_planes(&mut cx);
        t.set_mode(RoofMode::Join);
        // A click away from every edge only asks for one.
        click(&mut t, &mut cx, 240.0, 40.0);
        assert!(t.join_first.is_none());
        assert!(cx.status.contains("edge"), "{}", cx.status);
        // The first click picks the top edge (index 2) of the south plane.
        click(&mut t, &mut cx, 240.0, 100.0);
        assert_eq!(t.join_first, Some((a, 2)));
        // A click outside every plane keeps the pick.
        click(&mut t, &mut cx, 700.0, 700.0);
        assert_eq!(t.join_first, Some((a, 2)));
        // The second click on the other plane joins them, as one undo step.
        click(&mut t, &mut cx, 240.0, 300.0);
        assert!(t.join_first.is_none());
        assert_eq!(cx.undo_label(), Some("Join Roof Planes"), "{}", cx.status);
        let set = load(cx.floor());
        let top = set.plane(a).unwrap().plan_polygon();
        assert!((top[2].y - 180.0).abs() < 1e-6 && (top[3].y - 180.0).abs() < 1e-6);
        assert!(!set.plane(a).unwrap().auto);
        cx.undo();
        let top = load(cx.floor()).plane(a).unwrap().plan_polygon();
        assert!((top[2].y - 100.0).abs() < 1e-6);
        assert!(load(cx.floor()).plane(b).is_some());
    }

    #[test]
    fn join_starts_from_the_plane_selected_on_the_edit_toolbar() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        let (a, _) = facing_planes(&mut cx);
        // The Edit toolbar offers the command for a selected plane...
        cx.selection.set(crate::editor::ObjectRef::RoofPlane(a));
        let actions = cx.extra_edit_actions();
        assert!(actions.iter().any(|x| x.label == "Join Roof Planes"));
        // ...and the tool prefers that plane's edges.
        t.set_mode(RoofMode::Join);
        click(&mut t, &mut cx, 5.0, 50.0);
        assert_eq!(t.join_first, Some((a, 3)));
    }

    #[test]
    fn parallel_planes_refuse_to_join() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        let (a, b) = facing_planes(&mut cx);
        // Make the north plane parallel to the south one.
        let fi = cx.floor;
        let mut set = load(cx.floor());
        let base = set.plane(a).unwrap().clone();
        let rec = set.plane_mut(b).unwrap();
        rec.polygon3d = base
            .polygon3d
            .iter()
            .map(|v| [v[0], v[1], v[2] - 360.0])
            .collect();
        store(&mut cx.project, fi, &mut set);
        cx.mark_dirty();
        cx.refresh();
        t.set_mode(RoofMode::Join);
        click(&mut t, &mut cx, 240.0, 100.0);
        assert_eq!(t.join_first, Some((a, 2)));
        click(&mut t, &mut cx, 240.0, 400.0);
        assert!(cx.status.contains("Join Roof Planes"), "{}", cx.status);
        assert!(cx.status.contains("cannot be joined"), "{}", cx.status);
        assert_ne!(cx.undo_label(), Some("Join Roof Planes"));
    }

    #[test]
    fn auto_floating_dormer_does_not_cut_the_roof() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        // Floating mode opens the same dialog on a plane click.
        t.set_mode(RoofMode::FloatingDormer);
        click(&mut t, &mut cx, 120.0, 40.0);
        assert!(matches!(
            t.dormer_dialog.borrow().as_ref(),
            Some((_, None, true, _))
        ));
        let spec = t.dormer_dialog.borrow().as_ref().unwrap().3.spec();
        *t.dormer_dialog.borrow_mut() = None;
        make_dormer_as(&mut t, &mut cx, spec, true);
        assert_eq!(
            cx.undo_label(),
            Some("Auto Floating Dormer"),
            "{}",
            cx.status
        );
        let set = load(cx.floor());
        assert_eq!(set.dormers.len(), 1);
        assert!(set.dormers[0].floating);
        let main = set.planes[0].id;
        let tris = |cx: &EditorContext| -> usize {
            roof_view::roof_meshes(&cx.project)
                .iter()
                .filter(|m| m.object_id == Some(main))
                .map(plan_3d::Mesh::triangle_count)
                .sum()
        };
        let floating = tris(&cx);
        // The same dormer, not floating, cuts the plane: more triangles.
        cx.undo();
        make_dormer(&mut t, &mut cx, spec);
        let cut = tris(&cx);
        assert!(cut > floating, "{cut} vs {floating}");
        assert!(!load(cx.floor()).dormers[0].floating);
        // The flag survives a save and a specification edit.
        cx.undo();
        make_dormer_as(&mut t, &mut cx, spec, true);
        let json = serde_json::to_string(&cx.project).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert!(load(&back.floors[0]).dormers[0].floating);
        let id = load(cx.floor()).dormers[0].id;
        roof_view::apply_dormer(&mut cx.project, 0, main, Some(id), spec).unwrap();
        assert!(load(cx.floor()).dormers[0].floating);
        // Exploding it leaves the main plane uncut.
        roof_view::explode_dormer_record(&mut cx.project, 0, id, &cx.defaults).unwrap();
        assert!(load(cx.floor()).plane(main).unwrap().holes.is_empty());
    }

    fn roof_tris(cx: &EditorContext) -> usize {
        roof_view::roof_meshes(&cx.project)
            .iter()
            .map(plan_3d::Mesh::triangle_count)
            .sum()
    }

    /// A dormer made the way the dialog's OK does.
    fn make_dormer(t: &mut RoofTool, cx: &mut EditorContext, spec: DormerSpec) {
        make_dormer_as(t, cx, spec, false);
    }

    fn make_dormer_as(t: &mut RoofTool, cx: &mut EditorContext, spec: DormerSpec, floating: bool) {
        let main = load(cx.floor()).planes[0].id;
        t.cmds
            .borrow_mut()
            .push(Cmd::ApplyDormer(main, None, spec, floating));
        let p = PointerEvent::at(cx, Point::new(1.0, 1.0));
        t.pointer_move(cx, p);
    }

    #[test]
    fn auto_dormer_stores_a_record_with_two_planes_and_a_hole() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        let before = roof_tris(&cx);
        // The click opens the dialog on the plane under it.
        t.set_mode(RoofMode::Dormer);
        click(&mut t, &mut cx, 120.0, 40.0);
        assert!(t.dormer_dialog.borrow().is_some());
        let spec = {
            let g = t.dormer_dialog.borrow();
            g.as_ref().unwrap().3.spec()
        };
        assert!((spec.position_along_eave - 120.0).abs() < 1e-6);
        assert!((spec.setback_from_eave - 40.0).abs() < 1e-6);
        *t.dormer_dialog.borrow_mut() = None;

        make_dormer(&mut t, &mut cx, spec);
        assert_eq!(cx.undo_label(), Some("Auto Dormer"), "{}", cx.status);
        let set = load(cx.floor());
        assert_eq!(set.dormers.len(), 1);
        let geom = roof_view::dormer_geometry(&set, &set.dormers[0]).unwrap();
        assert_eq!(geom.roof_planes.len(), 2);
        assert!(geom.hole_in_main_roof.outline.len() >= 3);
        // The 3D view grows: dormer walls and roof planes are added.
        assert!(roof_tris(&cx) > before, "{} vs {before}", roof_tris(&cx));
        // The main plane is cut where the dormer stands.
        let hole = Point::lerp(
            geom.hole_in_main_roof.outline[0],
            geom.hole_in_main_roof.outline[2],
            0.5,
        );
        let uncut = roof_view::roof_meshes(&cx.project)
            .iter()
            .filter(|m| m.object_id == Some(set.planes[0].id))
            .any(|m| {
                m.indices.chunks(3).any(|tri| {
                    let v = |i: u32| m.vertices[i as usize];
                    let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
                    let plan = |q: plan_3d::Vertex| {
                        Point::new(f64::from(q.position[0]), -f64::from(q.position[2]))
                    };
                    a.normal[1] > 0.5
                        && plan_core::geometry::point_in_polygon(hole, &[plan(a), plan(b), plan(c)])
                })
            });
        assert!(!uncut, "the main roof has a hole under the dormer");
        // Selecting and picking: the dormer is a roof record.
        assert!(set
            .pick_polys()
            .iter()
            .any(|(id, _, _)| *id == set.dormers[0].id));
        assert!(roof_view::exists(cx.floor(), set.dormers[0].id));
        // Undo removes it again.
        cx.undo();
        assert!(load(cx.floor()).dormers.is_empty());
    }

    #[test]
    fn a_dormer_that_does_not_fit_is_refused() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        let spec = DormerSpec {
            width: 600.0,
            ..DormerSpec::default()
        };
        make_dormer(&mut t, &mut cx, spec);
        assert!(load(cx.floor()).dormers.is_empty());
        assert!(cx.status.contains("does not fit"), "{}", cx.status);
        assert!(!cx.can_undo() || cx.undo_label() != Some("Auto Dormer"));
    }

    #[test]
    fn explode_dormer_makes_plain_planes_and_keeps_the_hole() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        let spec = DormerSpec {
            position_along_eave: 120.0,
            setback_from_eave: 40.0,
            ..DormerSpec::default()
        };
        make_dormer(&mut t, &mut cx, spec);
        assert_eq!(load(cx.floor()).dormers.len(), 1);
        t.set_mode(RoofMode::Explode);
        // A click away from the dormer does nothing.
        click(&mut t, &mut cx, 10.0, 10.0);
        assert_eq!(load(cx.floor()).dormers.len(), 1);
        let geom = {
            let set = load(cx.floor());
            roof_view::dormer_geometry(&set, &set.dormers[0]).unwrap()
        };
        let at = geom.roof_planes[0].plan_polygon();
        let geom_walls = plan_roof::explode_dormer(&geom).walls;
        click(
            &mut t,
            &mut cx,
            polygon_center(&at).x,
            polygon_center(&at).y,
        );
        let set = load(cx.floor());
        assert!(set.dormers.is_empty());
        assert_eq!(
            set.planes.len(),
            3,
            "the main plane and the two dormer planes"
        );
        assert_eq!(set.planes[0].holes.len(), 1, "the footprint stays a hole");
        // The front and the two cheek walls are real walls that start at the
        // roof surface under them and take the default exterior type.
        let floor = cx.floor();
        assert_eq!(floor.walls.len(), 3);
        let default_type = cx.defaults.exterior_wall.wall_type.clone();
        for (w, dw) in floor.walls.iter().zip(&geom_walls) {
            assert!(
                (w.bottom_offset - (dw.base_elevation - floor.elevation)).abs() < 1e-9,
                "{} vs {}",
                w.bottom_offset,
                dw.base_elevation
            );
            assert!(w.bottom_offset > 0.0, "the roof is above the floor");
            assert_eq!(w.height, spec.wall_height);
            assert_eq!(w.kind, WallKind::Exterior);
            assert_eq!(w.wall_type.as_deref(), Some(default_type.as_str()));
            assert_eq!(w.thickness, cx.defaults.exterior_thickness());
        }
        // The front wall spans the dormer width (its outer face is on the
        // footprint), the cheeks run up the slope.
        assert!((floor.walls[0].length() - spec.width).abs() < 1e-6);
        assert!(floor.walls[1].length() > 1.0 && floor.walls[2].length() > 1.0);
        // They are also meshed in 3D, up at the roof.
        let scene = plan_3d::build_scene(&cx.project);
        let ys: Vec<f32> = scene
            .meshes
            .iter()
            .filter(|m| floor.walls.iter().any(|w| m.object_id == Some(w.id)))
            .flat_map(|m| m.vertices.iter().map(|v| v.position[1]))
            .collect();
        let low = floor.walls[0].bottom_offset as f32 + floor.elevation as f32;
        assert!(!ys.is_empty());
        assert!((ys.iter().copied().fold(f32::MAX, f32::min) - low).abs() < 1e-3);
        assert!(
            (ys.iter().copied().fold(f32::MIN, f32::max) - low - spec.wall_height as f32).abs()
                < 1e-3
        );
        assert_eq!(cx.undo_label(), Some("Explode Dormer"));
        cx.undo();
        assert_eq!(load(cx.floor()).dormers.len(), 1);
        assert!(cx.floor().walls.is_empty(), "undo takes the walls back");
    }

    fn polygon_center(p: &[Point]) -> Point {
        plan_core::geometry::polygon_centroid(p)
    }

    /// A closed ring of manual planes: the four planes of a hip roof with
    /// their automatic flag cleared.
    fn manual_hip(cx: &mut EditorContext, t: &mut RoofTool) {
        house(cx);
        build_default(t, cx);
        let fl = cx.floor;
        let mut set = load(&cx.project.floors[fl]);
        for p in &mut set.planes {
            p.auto = false;
        }
        set.settings = None;
        store(&mut cx.project, fl, &mut set);
    }

    #[test]
    fn gable_line_on_manual_planes_drops_the_end_plane() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        manual_hip(&mut cx, &mut t);
        assert_eq!(planes(&cx).len(), 4);
        // The eave of the right (short) end: x = 480 + 3 + 16, mid-height.
        let eave = planes(&cx)
            .into_iter()
            .find(|r| {
                (r.baseline.0.x - 499.0).abs() < 1e-6 && (r.baseline.1.x - 499.0).abs() < 1e-6
            })
            .unwrap();
        let mid = Point::lerp(eave.baseline.0, eave.baseline.1, 0.5);
        t.set_mode(RoofMode::GableLine);
        click(&mut t, &mut cx, mid.x, mid.y);
        assert_eq!(planes(&cx).len(), 3, "{}", cx.status);
        assert!(planes(&cx).iter().all(|r| !r.auto));
        assert_eq!(cx.undo_label(), Some("Gable/Roof Line"));
        cx.undo();
        assert_eq!(planes(&cx).len(), 4);
    }

    #[test]
    fn gable_line_on_an_automatic_eave_sets_the_edge_and_rebuilds() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        let eave = planes(&cx)
            .into_iter()
            .find(|r| {
                (r.baseline.0.x - 499.0).abs() < 1e-6 && (r.baseline.1.x - 499.0).abs() < 1e-6
            })
            .unwrap();
        let mid = Point::lerp(eave.baseline.0, eave.baseline.1, 0.5);
        t.set_mode(RoofMode::GableLine);
        click(&mut t, &mut cx, mid.x, mid.y);
        assert_eq!(planes(&cx).len(), 3, "{}", cx.status);
        let s = load(cx.floor()).settings.unwrap();
        assert_eq!(s.edge_specs.len(), 1);
        assert!(s.edge_specs[0].over.gable);
        // The override survives Auto Rebuild: the end stays a gable.
        let fl = cx.floor;
        let s2 = s.clone();
        rebuild(&mut cx.project, fl, s2, false).unwrap();
        assert_eq!(planes(&cx).len(), 3);
        // Clicking the wall flips it back to a hip and clears the override.
        click(&mut t, &mut cx, 480.0, 144.0);
        assert_eq!(planes(&cx).len(), 4);
        assert!(load(cx.floor()).settings.unwrap().edge_specs.is_empty());
    }

    #[test]
    fn edge_fields_of_the_plane_dialog_feed_build_roof() {
        let mut cx = new_cx();
        house(&mut cx);
        let mut t = RoofTool::default();
        build_default(&mut t, &mut cx);
        let old = planes(&cx).remove(0);
        assert!(old.source.is_some());
        let mut edited = old.clone();
        edited.edge.pitch = Some(12.0);
        edited.edge.overhang = Some(30.0);
        t.cmds
            .borrow_mut()
            .push(Cmd::ApplyPlane(old.id, Box::new(edited)));
        let p = PointerEvent::at(&cx, Point::new(1.0, 1.0));
        t.pointer_move(&mut cx, p);
        let set = load(cx.floor());
        assert_eq!(set.settings.as_ref().unwrap().edge_specs.len(), 1);
        let now = set
            .planes
            .iter()
            .find(|r| r.source == old.source)
            .expect("the plane of that edge is rebuilt");
        assert_eq!(now.pitch, 12.0);
        assert_eq!(now.overhang, 30.0);
        assert!(now.auto);
        assert_eq!(now.edge.pitch, Some(12.0));
        // The other edges keep the wall defaults.
        assert!(set
            .planes
            .iter()
            .filter(|r| r.source != old.source)
            .all(|r| r.pitch == 8.0));
    }

    #[test]
    fn ceiling_plane_is_a_record_on_its_own_layer_with_3d_meshes() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        t.set_mode(RoofMode::Ceiling);
        drag(&mut t, &mut cx, (0.0, 0.0), (240.0, 0.0));
        click(&mut t, &mut cx, 120.0, 100.0);
        let set = load(cx.floor());
        assert_eq!(set.ceilings.len(), 1);
        assert!(set.planes.is_empty());
        let c = &set.ceilings[0];
        assert_eq!(c.layer, roof_view::LAYER_CEILING);
        assert!(cx.project.layers.get(roof_view::LAYER_CEILING).is_some());
        assert_eq!(c.outline.len(), 4);
        assert_eq!(cx.undo_label(), Some("Draw Ceiling Plane"));
        assert!(roof_view::exists(cx.floor(), c.id));
        let meshes = roof_view::roof_meshes(&cx.project);
        assert!(meshes
            .iter()
            .any(|m| m.material == plan_3d::Material::WallInterior));
        assert!(meshes
            .iter()
            .any(|m| m.material == plan_3d::Material::Framing));
        // Delete Ceiling Planes removes it, in one undo step.
        cx.run_custom(crate::editor::dispatch::cmd::ROOF_DELETE_CEILINGS);
        assert!(load(cx.floor()).ceilings.is_empty());
        cx.undo();
        assert_eq!(load(cx.floor()).ceilings.len(), 1);
    }

    #[test]
    fn roof_return_adds_a_plane_at_an_eave_corner() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Return);
        click(&mut t, &mut cx, 240.0, 0.0);
        assert_eq!(planes(&cx).len(), 2, "{}", cx.status);
        assert_eq!(cx.undo_label(), Some("Roof Return"));
        // Away from every corner: nothing.
        click(&mut t, &mut cx, 120.0, 60.0);
        assert_eq!(planes(&cx).len(), 2);
    }

    #[test]
    fn roof_return_settings_set_the_length_and_type_of_the_next_returns() {
        let mut cx = new_cx();
        let mut t = RoofTool::default();
        with_plane(&mut cx, &mut t);
        t.set_mode(RoofMode::Return);
        assert_eq!(t.return_settings().length, RETURN_LENGTH);
        // The dialog's OK queues the new settings; the next event applies them.
        t.cmds.borrow_mut().push(Cmd::OpenReturn);
        click(&mut t, &mut cx, 240.0, 0.0);
        assert!(t.return_dialog.borrow().is_some(), "{}", cx.status);
        let before = planes(&cx).len();
        *t.return_dialog.borrow_mut() = None;
        t.cmds.borrow_mut().push(Cmd::ApplyReturn(ReturnSpec {
            kind: ReturnKind::Boxed,
            length: 60.0,
        }));
        click(&mut t, &mut cx, 240.0, 0.0);
        assert_eq!(t.return_settings().length, 60.0);
        assert_eq!(t.return_settings().kind, ReturnKind::Boxed);
        assert!(planes(&cx).len() > before);
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
