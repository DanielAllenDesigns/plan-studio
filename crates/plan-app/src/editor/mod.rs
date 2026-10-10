//! Shared editor services: the [`EditorContext`] every tool works on, plus
//! selection, snapping, handles, temporary dimensions, undo and rendering.
//! See `docs/architecture-tools.md`.

// This is the shared API for tool builders; not every item has a caller yet.
#![allow(dead_code)]

pub mod actions;
pub mod behaviors;
pub mod cabinet_edit;
pub mod camera;
pub mod clipboard;
pub mod code;
pub mod connect;
pub mod details_view;
pub mod dispatch;
pub mod edit_commands;
#[cfg(test)]
mod edit_tests;
pub mod fireplace_view;
pub mod foundation_view;
pub mod framing_view;
pub mod handles;
pub mod history;
pub mod opening_edit;
pub mod opening_view;
pub mod ops;
pub mod placed;
pub mod plan_overlay;
pub mod plan_tabs;
pub mod ref_overlay;
pub mod render;
pub mod restyle;
pub mod roof_view;
pub mod rooms_edit;
pub mod schedule_view;
pub mod selection;
pub mod sheet;
pub mod site_view;
pub mod snap;
pub mod solids_view;
pub mod stairs_view;
pub mod tempdim;
pub mod transform;
pub mod typed_input;
pub mod wall_edit;
pub mod camera_edit;

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
use plan_core::geometry::Point;
use plan_core::{
    detect_rooms, wall_layer_outlines, wall_outlines, Floor, Id, LayerSet, PlanDefaults, Project,
    Room, WallKind, WallLayerOutline, WallOutline, WallTypeDef,
};
use snap::SnapQuery;
use std::collections::{HashMap, HashSet};

/// Hands every [`EditorContext`] its own `uid`.
static NEXT_CONTEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The inputs the rooms and wall outlines were last computed from.
struct DerivedFrom {
    floor: usize,
    walls: Vec<plan_core::Wall>,
    types: Vec<WallTypeDef>,
    /// The `Floor.framing` values `framing` was parsed from (`None`: nothing
    /// parsed yet).
    framing_values: Option<Vec<serde_json::Value>>,
}

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
    /// Length and angle typed while a tool draws or drags (W-15, W-16).
    pub typed_input: typed_input::TypedInput,
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
    /// Names this context in draw caches (see [`cache_key`](Self::cache_key)).
    uid: u64,
    /// Counts the change signals (`begin_change`, `mark_dirty`, undo, ...).
    rev: u64,
    /// What the derived data was last computed from (see `refresh`).
    derived_from: Option<DerivedFrom>,
}

impl EditorContext {
    pub fn new(defaults: PlanDefaults) -> Self {
        let project = Project::from_defaults("Untitled", &defaults);
        let mut cx = Self::with_project(project, defaults);
        // The starting plan carries Daniel's template plan views.
        cx.seed_template_plan_views();
        cx
    }

    pub fn with_project(project: Project, defaults: PlanDefaults) -> Self {
        Self {
            project,
            floor: 0,
            selection: Selection::default(),
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
            typed_input: typed_input::TypedInput::default(),
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
            uid: NEXT_CONTEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            rev: 0,
            derived_from: None,
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
        self.touch();
    }

    // ----- undo -----

    /// Call before mutating the project: snapshots it as the undo step
    /// `label` (an imperative name such as "Move Wall").
    pub fn begin_change(&mut self, label: &str) {
        self.sync_dimension_layers();
        self.history.begin(&self.project, label);
        self.touch();
    }

    /// Copies the Dimension Defaults' Layer panel into the plan, where
    /// `Project::add_dimension` reads it (DIM-57).
    pub fn sync_dimension_layers(&mut self) {
        let st = &self.defaults.dimensions.setup;
        if self.project.dimension_layers[0] != st.layer_manual
            || self.project.dimension_layers[1] != st.layer_automatic
        {
            self.project.dimension_layers = [st.layer_manual.clone(), st.layer_automatic.clone()];
        }
    }

    /// Like [`begin_change`](Self::begin_change) but consecutive calls with
    /// the same label share one step (a slider drag). Ended by
    /// [`end_merge`](Self::end_merge).
    pub fn begin_change_merged(&mut self, label: &str) {
        self.sync_dimension_layers();
        self.history.begin_merged(&self.project, label);
        self.touch();
    }

    pub fn end_merge(&mut self) {
        self.history.end_merge();
    }

    /// Runs `f`, a command that may change several kinds of object (each
    /// family opening its own [`begin_change`](Self::begin_change)), as ONE
    /// undo step named after the first change; a command that changed nothing
    /// leaves no step at all (QA-24, QA-26). Groups nest.
    pub fn undo_group<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.begin_undo_group();
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.end_undo_group();
        match run {
            Ok(r) => r,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    /// Opens an undo group by hand; pair with
    /// [`end_undo_group`](Self::end_undo_group) (use `undo_group` when the
    /// commands run in a closure).
    pub fn begin_undo_group(&mut self) {
        self.history.begin_group();
    }

    /// Closes the group opened by [`begin_undo_group`](Self::begin_undo_group).
    pub fn end_undo_group(&mut self) {
        if self.history.end_group(&self.project) {
            self.touch();
        }
    }

    /// Forgets every undo and redo step (the plan stays). For test sweeps that
    /// must not run into the history's cap of steps.
    #[cfg(test)]
    pub fn forget_history(&mut self) {
        self.history.clear();
    }

    /// Records `before` (the project as it was before a change made outside
    /// the context, such as an edit in the layout window) as the undo step
    /// `label` of the one shared history.
    pub fn record_undo_step(&mut self, before: &Project, label: &str) {
        self.history.begin(before, label);
        self.touch();
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
        let keep = self.active_floor_key();
        let label = self.history.undo(&mut self.project);
        if label.is_some() {
            self.after_restore(keep);
        }
        label
    }

    pub fn redo(&mut self) -> Option<String> {
        let keep = self.active_floor_key();
        let label = self.history.redo(&mut self.project);
        if label.is_some() {
            self.after_restore(keep);
        }
        label
    }

    /// Name and elevation of the active floor: floors have no ids, and
    /// undoing the addition of a floor below the active one shifts its index.
    fn active_floor_key(&self) -> Option<(String, f64)> {
        self.project
            .floors
            .get(self.floor)
            .map(|f| (f.name.clone(), f.elevation))
    }

    fn after_restore(&mut self, keep: Option<(String, f64)>) {
        if let Some((name, elevation)) = keep {
            let same = |f: &Floor| f.name == name && (f.elevation - elevation).abs() < 1e-6;
            let still_there = self.project.floors.get(self.floor).is_some_and(same);
            if !still_there {
                let hits: Vec<usize> = (0..self.project.floors.len())
                    .filter(|&i| same(&self.project.floors[i]))
                    .collect();
                if let [only] = hits[..] {
                    self.floor = only;
                }
            }
        }
        self.floor = self.floor.min(self.project.floors.len() - 1);
        self.temp.clear();
        self.hover = None;
        self.touch();
        self.refresh();
    }

    // ----- derived data -----

    /// A change signal: derived data is stale and draw caches must not trust
    /// what they hold.
    fn touch(&mut self) {
        self.dirty = true;
        self.rev = self.rev.wrapping_add(1);
    }

    /// Names this context and its state of change for caches of drawn data
    /// (schedule tables, labels): the key differs after every change signal
    /// (`begin_change`, `mark_dirty`, undo, redo, a new project) and between
    /// two contexts.
    pub fn cache_key(&self) -> (u64, u64) {
        (self.uid, self.rev)
    }

    /// Recomputes the layers the plan reads (`layers()`) from the shown layer
    /// set and plan view. A set or view that changes how layers look (display,
    /// lock, colour, weight, styles, Ref) is read through `view_layers`; the
    /// plain starting view reads the base layers. `refresh` does this each
    /// frame; a layer edit calls it to take effect at once. It leaves the
    /// dirty flag alone.
    pub fn refresh_layer_view(&mut self) {
        let custom_view = self.project.layer_view_differs();
        self.view_layers = custom_view.then(|| self.project.view_layers());
    }

    /// Rooms and outlines are recomputed lazily after `mark_dirty`.
    pub fn mark_dirty(&mut self) {
        self.touch();
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Brings the derived data up to date. The shell calls it once a frame;
    /// tests call it before reading `rooms`.
    pub fn refresh(&mut self) {
        self.floor = self.floor.min(self.project.floors.len() - 1);
        crate::tools::cad::survey::on_refresh(self);
        self.refresh_layer_view();

        if self.dirty {
            self.rev = self.rev.wrapping_add(1);
            // Dimensions tied to walls follow them: tied ends, curved
            // dimensions of walls and Grid Rounding of the strings.
            let dim_fmt = self.dim_format();
            for f in &mut self.project.floors {
                f.refresh_dimensions(&dim_fmt);
            }
            // Callouts, markers and notes: moved groups, linked views and the
            // notes' numbers are brought up to date (`plan_core::callout`).
            self.project.sync_annotations();
            let walls = &self.project.floors[self.floor].walls;
            let types = if self.project.wall_types.is_empty() {
                &self.defaults.wall_types
            } else {
                &self.project.wall_types
            };
            // Rooms and outlines depend on the walls and wall types alone, so
            // an edit of anything else (a CAD line, a dimension, a schedule)
            // keeps them.
            let same = self.derived_from.as_ref().is_some_and(|d| {
                d.floor == self.floor
                    && d.types.as_slice() == types.as_slice()
                    && plan_core::walls_equal(&d.walls, walls)
            });
            if !same {
                self.rooms = detect_rooms(walls, 0.5);
                self.outlines = wall_outlines(walls, ops::JOIN_TOL);
                self.layer_outlines = wall_layer_outlines(walls, types, ops::JOIN_TOL);
                // The framing values carry over: whether the parsed members
                // still match is a question about the values alone.
                let framing_values = self.derived_from.take().and_then(|d| d.framing_values);
                self.derived_from = Some(DerivedFrom {
                    floor: self.floor,
                    walls: walls.clone(),
                    types: types.to_vec(),
                    framing_values,
                });
            }
            // Parsing the framing of a built house takes milliseconds; do it
            // only when the stored values changed.
            let values = &self.project.floors[self.floor].framing;
            let same_framing = self
                .derived_from
                .as_ref()
                .and_then(|d| d.framing_values.as_deref())
                .is_some_and(|v| v == values.as_slice());
            if !same_framing {
                self.framing = framing_view::load(&self.project.floors[self.floor]);
                if let Some(d) = self.derived_from.as_mut() {
                    d.framing_values = Some(values.clone());
                }
            }
            // Auto Refresh: the automatic strings follow the model (DIM-52).
            if crate::tools::dimension::auto_refresh(self) {
                let dim_fmt = self.dim_format();
                self.project.floors[self.floor].refresh_dimensions(&dim_fmt);
            }
            self.dirty = false;
        }
        self.selection.retain_existing(&self.project, self.floor);
        let f = &self.project.floors[self.floor];

        if self.view_flags.contains(&ViewFlag::TemporaryDimensions) {
            let loc = tempdim::TempLocate::of(self);
            self.temp.compute(f, &self.selection, &loc);
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
        room.name_entry(&self.floor().room_names)
            .map_or_else(|| room.label.clone(), |n| n.name.clone())
    }

    // ----- geometry helpers -----

    /// Pick distance in inches at the current zoom.
    pub fn pick_tol(&self) -> f64 {
        PICK_RADIUS_PX / self.px_per_in.max(1e-6)
    }

    pub fn snap_tol(&self) -> f64 {
        SnapSettings::from_editing(&self.defaults.editing).tolerance_px / self.px_per_in.max(1e-6)
    }

    /// Runs the snap engine. `origin` is the pending start point (angle and
    /// perpendicular snaps); `overrides` (Ctrl/Cmd held, DT2) suspends every
    /// snap (S-74); walls in `exclude` are ignored.
    pub fn snap_at(
        &self,
        raw: Point,
        origin: Option<Point>,
        overrides: bool,
        exclude: &[Id],
    ) -> SnapResult {
        // Shift restricts the angle snaps to 90 or 45 degrees; extra allowed
        // angles join the increment's own (manual pp. 122, 193).
        let allowed = snap::allowed_angle_set(
            &self.defaults.editing,
            self.defaults.grid.angle_snap_deg,
            snap::held().shift,
        );
        let plain = self.defaults.editing.snap_angles.is_empty()
            && self.defaults.editing.additional_angles.is_empty()
            && !snap::held().shift;
        let q = SnapQuery {
            floor: self.floor(),
            layers: self.layers(),
            tol: self.snap_tol(),
            grid_step: self.defaults.grid.snap,
            angle_deg: self.defaults.grid.angle_snap_deg,
            angles: if plain { &[] } else { &allowed },
            origin,
            suspend_angle: overrides,
            suspend_all: overrides,
            exclude,
        };
        // The reference floor's wall ends and crossings snap too (R-65).
        let reference = snap::reference_segments(self);
        snap::snap_with_reference(
            raw,
            &q,
            &SnapSettings::from_editing(&self.defaults.editing),
            &reference,
        )
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
            OpeningTarget::DefaultType(k) if k.kind == plan_core::OpeningKind::Window => {
                OpeningExtras::from_window_defaults(&self.defaults.window)
            }
            OpeningTarget::DefaultExteriorDoor => {
                OpeningExtras::from_door_defaults(&self.defaults.exterior_door, true)
            }
            OpeningTarget::DefaultType(k) if k.exterior => {
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
    fn undoing_a_floor_below_keeps_the_active_floor() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.begin_change("Build Foundation");
        let idx = cx
            .project
            .build_foundation(plan_core::floors::FoundationKind::StemWall { height: 36.0 });
        assert_eq!(idx, 0);
        cx.floor = 1;
        let name = cx.floor().name.clone();
        cx.undo();
        // The foundation floor is gone and the view stays on the same floor.
        assert_eq!(cx.floor, 0);
        assert_eq!(cx.floor().name, name);
        cx.redo();
        assert_eq!(cx.floor, 1);
        assert_eq!(cx.floor().name, name);
    }

    #[test]
    fn a_dimension_tied_to_a_wall_follows_it_and_undo_restores_both() {
        use plan_core::{Dimension, DimensionKind};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let dim = cx.project.add_dimension(
            0,
            Dimension::new(
                0,
                DimensionKind::Manual,
                Point::ZERO,
                Point::new(120.0, 0.0),
                24.0,
            ),
        );
        assert_eq!(cx.project.floors[0].attach_dimension(dim), 2);
        cx.begin_change("Stretch Wall");
        cx.project.floors[0].wall_mut(id).unwrap().end = Point::new(180.0, 0.0);
        cx.mark_dirty();
        cx.refresh();
        let d = cx.project.floors[0].dimensions[0].clone();
        assert_eq!(d.end, Point::new(180.0, 0.0));
        cx.undo();
        let d = &cx.project.floors[0].dimensions[0];
        assert_eq!(d.end, Point::new(120.0, 0.0));
        assert_eq!(cx.floor().walls[0].end, Point::new(120.0, 0.0));
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
