//! Shared editor services: the [`EditorContext`] every tool works on, plus
//! selection, snapping, handles, temporary dimensions, undo and rendering.
//! See `docs/architecture-tools.md`.

// This is the shared API for tool builders; not every item has a caller yet.
#![allow(dead_code)]

pub mod actions;
pub mod camera;
pub mod connect;
pub mod dispatch;
pub mod foundation_view;
pub mod framing_view;
pub mod handles;
pub mod history;
pub mod ops;
pub mod placed;
pub mod render;
pub mod restyle;
pub mod roof_view;
pub mod rooms_edit;
pub mod selection;
pub mod sheet;
pub mod site_view;
pub mod snap;
pub mod stairs_view;
pub mod tempdim;

pub use actions::{Clipboard, EditAction, EditActionKind};
pub use camera::Camera;
pub use selection::{ObjectRef, Selection};
pub use snap::{SnapResult, SnapSettings};
pub use tempdim::TempDims;

use crate::dialogs::{OpeningExtras, OpeningTarget, WallExtras};
use crate::plan_defaults;
use crate::theme::{AppSettings, Palette};
use crate::toolbar::ViewFlag;
use eframe::egui::Vec2;
use history::ChangeHistory;
use plan_core::geometry::{point_in_polygon, Point};
use plan_core::{
    detect_rooms, wall_layer_outlines, wall_outlines, Floor, Id, LayerSet, PlanDefaults, Project,
    Room, WallKind, WallLayerOutline, WallOutline, WallTypeDef,
};
use snap::SnapQuery;
use std::collections::{HashMap, HashSet};

/// Pick distance in screen pixels (also the snap distance).
pub const PICK_RADIUS_PX: f64 = 10.0;

/// Settings the model has no fields for yet, kept for the session only.
#[derive(Default)]
pub struct SessionExtras {
    pub walls: HashMap<Id, WallExtras>,
    pub openings: HashMap<Id, OpeningExtras>,
}

/// Things a tool asks the application shell to do (it owns dialogs and the
/// camera).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EditorRequest {
    /// Open the specification dialog of an object.
    OpenSpec(ObjectRef),
    /// Pan the view by screen pixels.
    PanPixels(Vec2),
    /// Switch to a tool (an Edit toolbar command such as Join Roof Planes).
    SetTool(crate::tools::ToolId),
}

pub struct EditorContext {
    pub project: Project,
    pub floor: usize,
    pub selection: Selection,
    pub snap: SnapSettings,
    /// Wall types, wall/door/window defaults, grid, dimension format, ...
    pub defaults: PlanDefaults,
    pub view_flags: HashSet<ViewFlag>,
    /// The active layout's sheet size and scale (Drawing Sheet, Print Preview).
    pub sheet: sheet::SheetSetup,
    /// The status-bar message (persists until replaced).
    pub status: String,
    /// A live value for the status bar while a tool works ("Length: ...").
    pub readout: Option<String>,
    pub extras: SessionExtras,
    pub clipboard: Option<Clipboard>,
    pub temp: TempDims,
    /// The object under the pointer (Select Objects).
    pub hover: Option<ObjectRef>,
    pub cursor_world: Option<Point>,
    /// The snap of the pointer position, for the marker and status bar.
    pub last_snap: Option<SnapResult>,
    /// Pixels per inch of the view; the shell keeps it current.
    pub px_per_in: f64,
    /// The canvas colors; the shell keeps them current.
    pub palette: Palette,
    pub requests: Vec<EditorRequest>,
    /// Detected rooms (valid after [`refresh`](Self::refresh)).
    pub rooms: Vec<Room>,
    /// Mitered wall outlines (valid after [`refresh`](Self::refresh)).
    pub outlines: Vec<WallOutline>,
    /// One polygon per wall layer (valid after [`refresh`](Self::refresh)).
    pub layer_outlines: Vec<WallLayerOutline>,
    /// The active floor's built framing (valid after [`refresh`](Self::refresh)).
    pub framing: Vec<plan_framing::Member>,
    /// The layers as the active plan view shows them, when that differs from
    /// `project.layers` (a non-default layer set or plan view is active).
    view_layers: Option<LayerSet>,
    history: ChangeHistory,
    dirty: bool,
}

impl EditorContext {
    pub fn new(defaults: PlanDefaults) -> Self {
        let project = Project::from_defaults("Untitled", &defaults);
        Self::with_project(project, defaults)
    }

    pub fn with_project(project: Project, defaults: PlanDefaults) -> Self {
        Self {
            project,
            floor: 0,
            selection: Selection::default(),
            snap: SnapSettings::default(),
            defaults,
            // Color is on until the F8 toggle turns it off.
            view_flags: HashSet::from([
                ViewFlag::Color,
                ViewFlag::TemporaryDimensions,
                ViewFlag::ReferenceGrid,
            ]),
            sheet: sheet::SheetSetup::default(),
            status: String::new(),
            readout: None,
            extras: SessionExtras::default(),
            clipboard: None,
            temp: TempDims::default(),
            hover: None,
            cursor_world: None,
            last_snap: None,
            px_per_in: 2.0,
            palette: AppSettings::default().theme.palette(),
            requests: Vec::new(),
            rooms: Vec::new(),
            outlines: Vec::new(),
            layer_outlines: Vec::new(),
            framing: Vec::new(),
            view_layers: None,
            history: ChangeHistory::new(),
            dirty: true,
        }
    }

    // ----- model access -----

    pub fn floor(&self) -> &Floor {
        &self.project.floors[self.floor]
    }

    pub fn floor_mut(&mut self) -> &mut Floor {
        &mut self.project.floors[self.floor]
    }

    /// The layers as the active plan view shows them (display, lock, color).
    pub fn layers(&self) -> &LayerSet {
        self.view_layers.as_ref().unwrap_or(&self.project.layers)
    }

    /// Replaces the project (New / Open): clears history, selection and
    /// per-project session state.
    pub fn set_project(&mut self, project: Project) {
        self.project = project;
        site_view::migrate_legacy_storage(&mut self.project);
        self.floor = 0;
        self.history.clear();
        self.reset_view_state();
    }

    /// Forgets selection and in-progress state (floor change, new project).
    pub fn reset_view_state(&mut self) {
        self.selection.clear();
        self.temp.clear();
        self.hover = None;
        self.readout = None;
        self.dirty = true;
    }

    // ----- undo -----

    /// Call before mutating the project: snapshots it as the undo step
    /// `label` (an imperative name such as "Move Wall").
    pub fn begin_change(&mut self, label: &str) {
        self.history.begin(&self.project, label);
        self.dirty = true;
    }

    /// Like [`begin_change`](Self::begin_change) but consecutive calls with
    /// the same label share one step (a slider drag). Ended by
    /// [`end_merge`](Self::end_merge).
    pub fn begin_change_merged(&mut self, label: &str) {
        self.history.begin_merged(&self.project, label);
        self.dirty = true;
    }

    pub fn end_merge(&mut self) {
        self.history.end_merge();
    }

    /// The change begun last turned out to be a no-op; drop its undo step.
    pub fn cancel_change(&mut self) {
        self.history.cancel();
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// Steps back; returns the undone step's label.
    pub fn undo(&mut self) -> Option<String> {
        let label = self.history.undo(&mut self.project);
        if label.is_some() {
            self.after_restore();
        }
        label
    }

    pub fn redo(&mut self) -> Option<String> {
        let label = self.history.redo(&mut self.project);
        if label.is_some() {
            self.after_restore();
        }
        label
    }

    fn after_restore(&mut self) {
        self.floor = self.floor.min(self.project.floors.len() - 1);
        self.temp.clear();
        self.hover = None;
        self.dirty = true;
        self.refresh();
    }

    // ----- derived data -----

    /// Rooms and outlines are recomputed lazily after `mark_dirty`.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Brings the derived data up to date. The shell calls it once a frame;
    /// tests call it before reading `rooms`.
    pub fn refresh(&mut self) {
        self.floor = self.floor.min(self.project.floors.len() - 1);
        use plan_core::layer_sets::{DEFAULT_LAYER_SET_NAME, DEFAULT_PLAN_VIEW_NAME};
        let custom_view = self.project.active_plan_view != DEFAULT_PLAN_VIEW_NAME
            || self.project.layer_sets.active != DEFAULT_LAYER_SET_NAME;
        self.view_layers = custom_view.then(|| self.project.view_layers());

        if self.dirty {
            let walls = &self.project.floors[self.floor].walls;
            self.rooms = detect_rooms(walls, 0.5);
            self.outlines = wall_outlines(walls, ops::JOIN_TOL);
            let types = if self.project.wall_types.is_empty() {
                &self.defaults.wall_types
            } else {
                &self.project.wall_types
            };
            self.layer_outlines = wall_layer_outlines(walls, types, ops::JOIN_TOL);
            self.framing = framing_view::load(&self.project.floors[self.floor]);
            self.dirty = false;
        }
        self.selection.retain_existing(&self.project, self.floor);
        let f = &self.project.floors[self.floor];

        if self.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            self.temp.compute(f, &self.selection);
        } else {
            self.temp.clear();
        }
    }

    /// The rooms, refreshed first.
    pub fn rooms_now(&mut self) -> &[Room] {
        self.refresh();
        &self.rooms
    }

    /// The wall type definitions in force: the plan's, else the defaults'.
    pub fn wall_types(&self) -> &[WallTypeDef] {
        if self.project.wall_types.is_empty() {
            &self.defaults.wall_types
        } else {
            &self.project.wall_types
        }
    }

    /// The room's name from the model, else its generated label.
    pub fn room_name(&self, room: &Room) -> String {
        self.floor()
            .room_names
            .iter()
            .find(|n| point_in_polygon(n.anchor, &room.polygon))
            .map_or_else(|| room.label.clone(), |n| n.name.clone())
    }

    // ----- geometry helpers -----

    /// Pick distance in inches at the current zoom.
    pub fn pick_tol(&self) -> f64 {
        PICK_RADIUS_PX / self.px_per_in.max(1e-6)
    }

    pub fn snap_tol(&self) -> f64 {
        self.snap.tolerance_px / self.px_per_in.max(1e-6)
    }

    /// Runs the snap engine. `origin` is the pending start point (angle and
    /// perpendicular snaps); `alt` suspends the angle snap; walls in
    /// `exclude` are ignored.
    pub fn snap_at(
        &self,
        raw: Point,
        origin: Option<Point>,
        alt: bool,
        exclude: &[Id],
    ) -> SnapResult {
        let q = SnapQuery {
            floor: self.floor(),
            layers: self.layers(),
            tol: self.snap_tol(),
            grid_step: self.defaults.grid.snap,
            angle_deg: self.defaults.grid.angle_snap_deg,
            origin,
            suspend_angle: alt,
            exclude,
        };
        snap::snap(raw, &q, &self.snap)
    }

    /// Dimension text in the active dimension defaults' format.
    pub fn fmt_dim(&self, inches: f64) -> String {
        self.dim_format().fmt_len(inches)
    }

    /// The dimension text format in force: the defaults' fraction and unit
    /// indicators, and the active set's own units (inches, metric, ...) when
    /// it picked others than feet and inches.
    pub fn dim_format(&self) -> plan_core::DimFormat {
        let base = self.defaults.dim_format();
        match self
            .defaults
            .active_dimension()
            .and_then(|s| s.format.length)
        {
            Some(l) if l.unit != plan_core::units::LengthUnit::FeetInches => plan_core::DimFormat {
                length: Some(l),
                ..base
            },
            _ => base,
        }
    }

    pub fn wall_thickness(&self, kind: WallKind) -> f64 {
        match kind {
            WallKind::Exterior => self.defaults.exterior_thickness(),
            WallKind::Interior => self.defaults.interior_thickness(),
        }
    }

    pub fn wall_height(&self, kind: WallKind) -> f64 {
        self.defaults.walls_for(kind).height
    }

    /// The grid snap unit, 1" unless the defaults say otherwise.
    pub fn snap_unit(&self) -> f64 {
        if self.defaults.grid.snap > 0.0 {
            self.defaults.grid.snap
        } else {
            1.0
        }
    }

    pub fn select_only(&mut self, o: ObjectRef) {
        self.selection.set(o);
    }

    /// The extras a default door or window dialog starts from: the ones kept
    /// this session, else the values in the plan defaults.
    pub fn default_opening_extras(&self, target: OpeningTarget) -> OpeningExtras {
        if let Some(e) = self.extras.openings.get(&target.key()) {
            return e.clone();
        }
        match target {
            OpeningTarget::DefaultWindow => {
                OpeningExtras::from_window_defaults(&self.defaults.window)
            }
            OpeningTarget::DefaultExteriorDoor => {
                OpeningExtras::from_door_defaults(&self.defaults.exterior_door, true)
            }
            _ => OpeningExtras::from_door_defaults(&self.defaults.interior_door, false),
        }
    }

    /// The template an opening of `kind` is placed from on a wall of
    /// `wall_kind`, with the defaults target that owns its dialog extras.
    pub fn opening_template(
        &self,
        kind: plan_core::OpeningKind,
        wall_kind: WallKind,
    ) -> (OpeningTarget, plan_core::Opening) {
        if kind == plan_core::OpeningKind::Window {
            (
                OpeningTarget::DefaultWindow,
                plan_defaults::window_template(&self.defaults),
            )
        } else if wall_kind == WallKind::Exterior {
            (
                OpeningTarget::DefaultExteriorDoor,
                plan_defaults::door_template(&self.defaults, true),
            )
        } else {
            (
                OpeningTarget::DefaultDoor,
                plan_defaults::door_template(&self.defaults, false),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_change_undo_redo_with_labels() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert!(!cx.can_undo());
        cx.begin_change("Draw Wall");
        cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(100.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        assert_eq!(cx.undo_label(), Some("Draw Wall"));
        assert_eq!(cx.undo().as_deref(), Some("Draw Wall"));
        assert!(cx.floor().walls.is_empty());
        assert_eq!(cx.redo_label(), Some("Draw Wall"));
        cx.redo();
        assert_eq!(cx.floor().walls.len(), 1);
    }

    #[test]
    fn an_active_plan_view_decides_which_layers_show() {
        use plan_core::SavedPlanView;
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.refresh();
        assert!(cx.layers().is_visible("Doors"));
        let mut set = cx.project.layer_sets.get("Default Set").unwrap().clone();
        set.name = "No Doors".into();
        set.ensure_state("Doors").display = false;
        assert!(cx.project.layer_sets.add_set(set));
        cx.project
            .plan_views
            .push(SavedPlanView::new("Doorless", "No Doors"));
        assert!(cx.project.activate_plan_view("Doorless"));
        cx.refresh();
        assert!(!cx.layers().is_visible("Doors"));
        // The base layers are untouched.
        assert!(cx.project.layers.is_visible("Doors"));
        assert!(cx.project.activate_plan_view("Floor Plan View"));
        cx.refresh();
        assert!(cx.layers().is_visible("Doors"));
    }
}
