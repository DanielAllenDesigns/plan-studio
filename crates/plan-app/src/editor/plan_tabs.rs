//! Plan views as tabs above the canvas, and the commands around saved plan
//! views: showing one, Save Plan View and Reset Plan View.
//!
//! A saved plan view ([`plan_core::SavedPlanView`]) carries its own layer set,
//! floor, reference display and camera. Several views can be open as tabs
//! ([`PlanTabs`]); switching tabs stores what the view being left looks at
//! (floor, reference flag, zoom and pan) in that view and shows the other one,
//! so each tab comes back as it was left.
//!
//! The window shell owns the camera. It reports the current centre and zoom
//! with [`report_camera`] and applies the camera a switch asks for with
//! [`take_pending_camera`]; without those calls the floor, layer set and
//! reference display still switch, only the pan and zoom stay put.

use super::EditorContext;
use crate::toolbar::ViewFlag;
use plan_core::geometry::Point;
use plan_core::{Project, SavedPlanView};
use std::cell::RefCell;

/// A view's centre and zoom (pixels per plan inch).
pub type CameraState = (Point, f64);

/// The plan views open as tabs, in tab order.
#[derive(Default)]
pub struct PlanTabs {
    open: Vec<String>,
    /// The camera the shell last reported.
    camera: Option<CameraState>,
    /// The camera the shell must show next.
    pending: Option<CameraState>,
}

impl PlanTabs {
    /// Forgets tabs whose view is gone and makes sure the active view is a
    /// tab (a menu or an undo may have changed it).
    pub fn sync(&mut self, project: &Project) {
        self.open.retain(|n| project.plan_view(n).is_some());
        let active = &project.active_plan_view;
        if project.plan_view(active).is_some() && !self.open.contains(active) {
            self.open.push(active.clone());
        }
    }

    /// The open tabs (call [`sync`](Self::sync) first).
    pub fn open(&self) -> &[String] {
        &self.open
    }

    /// The shell reports where the camera is.
    pub fn report_camera(&mut self, center: Point, zoom: f64) {
        self.camera = Some((center, zoom));
    }

    /// The camera a tab switch wants the shell to show, once.
    pub fn take_pending_camera(&mut self) -> Option<CameraState> {
        self.pending.take()
    }

    /// The camera of the view being left: the reported centre, else the one
    /// stored in the view; the reported zoom, else the one of `cx`.
    fn outgoing_camera(&self, cx: &EditorContext) -> Option<CameraState> {
        let stored = cx.project.current_plan_view().and_then(|v| v.camera);
        let center = self.camera.map(|c| c.0).or(stored.map(|c| c.0))?;
        let zoom = self.camera.map_or(cx.px_per_in, |c| c.1);
        Some((center, if zoom > 0.0 { zoom } else { stored?.1 }))
    }

    /// Stores what the active view looks at into the active view itself.
    fn store_active(&self, cx: &mut EditorContext) {
        let camera = self.outgoing_camera(cx);
        let flag = cx.view_flags.contains(&ViewFlag::ReferenceDisplay);
        let floor = cx.floor;
        let active = cx.project.active_plan_view.clone();
        if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == active) {
            v.capture(floor, flag, camera);
        }
    }

    /// Opens `name` as a tab (if it is not one) and switches to it. `false`
    /// when there is no such view.
    pub fn open_view(&mut self, cx: &mut EditorContext, name: &str) -> bool {
        if cx.project.plan_view(name).is_none() {
            return false;
        }
        self.sync(&cx.project);
        if !self.open.iter().any(|n| n == name) {
            self.open.push(name.to_string());
        }
        self.switch_to(cx, name)
    }

    /// Switches to the tab `name`: the view being left keeps its floor,
    /// reference display, zoom and pan, the other one is shown. `false` when
    /// there is no such view.
    pub fn switch_to(&mut self, cx: &mut EditorContext, name: &str) -> bool {
        if cx.project.plan_view(name).is_none() {
            return false;
        }
        self.sync(&cx.project);
        if cx.project.active_plan_view == name {
            return true;
        }
        self.store_active(cx);
        let Some(view) = cx.show_plan_view(name) else {
            return false;
        };
        self.pending = view.camera;
        cx.status = format!("Plan view: {name}");
        true
    }

    /// Closes the tab `name`; closing the shown one shows its neighbour. The
    /// last tab stays. `false` when nothing was closed.
    pub fn close(&mut self, cx: &mut EditorContext, name: &str) -> bool {
        self.sync(&cx.project);
        let Some(i) = self.open.iter().position(|n| n == name) else {
            return false;
        };
        if self.open.len() <= 1 {
            return false;
        }
        if cx.project.active_plan_view == name {
            let next = if i > 0 { i - 1 } else { 1 };
            let to = self.open[next].clone();
            if !self.switch_to(cx, &to) {
                return false;
            }
        }
        self.open.retain(|n| n != name);
        true
    }

    /// Moves the tab at `from` to `to` (drag a tab).
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        let n = self.open.len();
        if from >= n || to >= n || from == to {
            return false;
        }
        let t = self.open.remove(from);
        self.open.insert(to, t);
        true
    }

    /// Save Plan View: stores the floor, reference display, zoom and pan the
    /// plan shows now in the active view (one undo step).
    pub fn save_active(&mut self, cx: &mut EditorContext) -> bool {
        let active = cx.project.active_plan_view.clone();
        if cx.project.plan_view(&active).is_none() {
            return false;
        }
        cx.begin_change("Save Plan View");
        self.store_active(cx);
        cx.mark_dirty();
        cx.status = format!("Saved plan view {active}");
        true
    }

    /// Reset Plan View: shows the active view as it was saved again (floor,
    /// reference display, defaults, zoom and pan).
    pub fn reset_active(&mut self, cx: &mut EditorContext) -> bool {
        let active = cx.project.active_plan_view.clone();
        let Some(view) = cx.show_plan_view(&active) else {
            return false;
        };
        self.pending = view.camera;
        cx.status = format!("Reset plan view {active}");
        true
    }
}

thread_local! {
    static TABS: RefCell<PlanTabs> = RefCell::new(PlanTabs::default());
}

/// Runs `f` on the session's open tabs.
pub fn with_tabs<R>(f: impl FnOnce(&mut PlanTabs) -> R) -> R {
    TABS.with(|t| f(&mut t.borrow_mut()))
}

/// The shell reports the camera each frame (see the module docs).
pub fn report_camera(center: Point, zoom: f64) {
    with_tabs(|t| t.report_camera(center, zoom));
}

/// The camera to show now, if a tab switch or Reset Plan View asked for one.
pub fn take_pending_camera() -> Option<CameraState> {
    with_tabs(PlanTabs::take_pending_camera)
}

impl EditorContext {
    /// Adds Daniel's template plan views (with a layer set each) when the plan
    /// holds only the starting view, so a new plan starts with the working
    /// views of the template. Not an undo step. Returns how many were added.
    pub fn seed_template_plan_views(&mut self) -> usize {
        if !self.project.has_only_starting_plan_view() {
            return 0;
        }
        let n = self.project.seed_template_plan_views();
        if n > 0 {
            self.mark_dirty();
        }
        n
    }

    /// Shows the saved plan view `name`: makes it and its layer set active,
    /// and brings the reference display flag, the floor and the view's
    /// dimension and text defaults along. Not an undo step (it is navigation).
    /// Returns the view so the caller can apply its camera.
    pub fn show_plan_view(&mut self, name: &str) -> Option<SavedPlanView> {
        let view = self.project.plan_view(name)?.clone();
        self.project.activate_plan_view(name);
        if view.reference_display {
            self.view_flags.insert(ViewFlag::ReferenceDisplay);
        } else {
            self.view_flags.remove(&ViewFlag::ReferenceDisplay);
        }
        if let Some(f) = view.floor.filter(|f| *f < self.project.floors.len()) {
            if f != self.floor {
                self.floor = f;
                self.reset_view_state();
            }
        }
        if !view.dimension_defaults.is_empty() {
            self.defaults
                .set_active_dimension_set(&view.dimension_defaults);
        }
        if let Some(st) = self.project.text_styles.get(&view.text_style).cloned() {
            self.defaults.text.font = st.font;
            self.defaults.text.height = st.height_in;
        }
        self.mark_dirty();
        Some(view)
    }
}

/// A context whose plan holds the starting plan view and layer set only (the
/// template plan views are not seeded), for tests of the view lists.
#[cfg(test)]
pub fn plain_cx() -> EditorContext {
    let defaults = crate::plan_defaults::embedded();
    EditorContext::with_project(Project::from_defaults("Untitled", &defaults), defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::layer_sets::DEFAULT_PLAN_VIEW_NAME;

    fn cx_with_views() -> EditorContext {
        let mut cx = plain_cx();
        cx.project.insert_floor_above(0).unwrap();
        assert!(cx.project.layer_sets.copy_set("Default Set", "Dimmed"));
        cx.project.layer_sets.set_display("Dimmed", "Doors", false);
        let mut a = SavedPlanView::new("Upstairs", "Dimmed");
        a.floor = Some(1);
        a.reference_display = true;
        a.camera = Some((Point::new(500.0, 300.0), 2.0));
        assert!(cx.project.add_plan_view(a));
        cx
    }

    #[test]
    fn showing_a_view_brings_its_layer_set_floor_and_reference_display() {
        let mut cx = cx_with_views();
        let view = cx.show_plan_view("Upstairs").unwrap();
        assert_eq!(cx.project.active_plan_view, "Upstairs");
        assert_eq!(cx.project.layer_sets.active, "Dimmed");
        assert_eq!(cx.floor, 1);
        assert!(cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
        assert_eq!(view.camera, Some((Point::new(500.0, 300.0), 2.0)));
        cx.refresh();
        assert!(!cx.layers().is_visible("Doors"));
        assert!(cx.show_plan_view("nope").is_none());
        cx.show_plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap();
        // The starting view has no floor of its own: the floor stays.
        assert_eq!(cx.floor, 1);
        assert!(!cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
        cx.refresh();
        assert!(cx.layers().is_visible("Doors"));
    }

    #[test]
    fn a_view_can_set_dimension_and_text_defaults() {
        let mut cx = cx_with_views();
        let set = cx.defaults.dimension_sets[1].name.clone();
        let style = cx.project.text_styles.styles[1].clone();
        let v = cx
            .project
            .plan_views
            .iter_mut()
            .find(|v| v.name == "Upstairs")
            .unwrap();
        v.dimension_defaults = set.clone();
        v.text_style = style.name.clone();
        cx.show_plan_view("Upstairs").unwrap();
        assert_eq!(cx.defaults.active_dimension_set, set);
        assert_eq!(cx.defaults.text.height, style.height_in);
    }

    #[test]
    fn tabs_open_close_and_switch_keeping_zoom_floor_and_pan() {
        let mut cx = cx_with_views();
        let mut tabs = PlanTabs::default();
        tabs.sync(&cx.project);
        assert_eq!(tabs.open(), [DEFAULT_PLAN_VIEW_NAME]);

        // Work in the starting view: floor 0, some pan and zoom.
        cx.floor = 0;
        cx.px_per_in = 1.5;
        tabs.report_camera(Point::new(40.0, 50.0), 1.5);

        // Open the second view as a tab: it comes up as saved.
        assert!(tabs.open_view(&mut cx, "Upstairs"));
        assert_eq!(tabs.open(), [DEFAULT_PLAN_VIEW_NAME, "Upstairs"]);
        assert_eq!(cx.project.active_plan_view, "Upstairs");
        assert_eq!(cx.floor, 1);
        assert_eq!(
            tabs.take_pending_camera(),
            Some((Point::new(500.0, 300.0), 2.0))
        );
        assert_eq!(tabs.take_pending_camera(), None, "the camera is asked once");
        // The view that was left kept what it showed.
        let first = cx.project.plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap();
        assert_eq!(first.floor, Some(0));
        assert_eq!(first.camera, Some((Point::new(40.0, 50.0), 1.5)));

        // Zoom and pan the second view, then go back.
        cx.px_per_in = 3.0;
        tabs.report_camera(Point::new(10.0, 20.0), 3.0);
        assert!(tabs.switch_to(&mut cx, DEFAULT_PLAN_VIEW_NAME));
        assert_eq!(cx.floor, 0);
        assert_eq!(
            tabs.take_pending_camera(),
            Some((Point::new(40.0, 50.0), 1.5))
        );
        assert_eq!(
            cx.project.plan_view("Upstairs").unwrap().camera,
            Some((Point::new(10.0, 20.0), 3.0))
        );
        assert!(cx.project.plan_view("Upstairs").unwrap().reference_display);

        // And forward again: floor and zoom return.
        assert!(tabs.switch_to(&mut cx, "Upstairs"));
        assert_eq!(cx.floor, 1);
        assert_eq!(
            tabs.take_pending_camera(),
            Some((Point::new(10.0, 20.0), 3.0))
        );

        // Closing the shown tab shows its neighbour; the last tab stays.
        assert!(tabs.close(&mut cx, "Upstairs"));
        assert_eq!(tabs.open(), [DEFAULT_PLAN_VIEW_NAME]);
        assert_eq!(cx.project.active_plan_view, DEFAULT_PLAN_VIEW_NAME);
        assert!(!tabs.close(&mut cx, DEFAULT_PLAN_VIEW_NAME));
        assert!(!tabs.close(&mut cx, "nope"));
        assert!(!tabs.open_view(&mut cx, "nope"));
        assert!(!tabs.switch_to(&mut cx, "nope"));
    }

    #[test]
    fn tabs_follow_deleted_views_and_menu_activation() {
        let mut cx = cx_with_views();
        let mut tabs = PlanTabs::default();
        tabs.open_view(&mut cx, "Upstairs");
        // The menu activates a view that is not a tab: it becomes one.
        assert!(cx
            .project
            .add_plan_view(SavedPlanView::new("Other", "Dimmed")));
        cx.project.activate_plan_view("Other");
        tabs.sync(&cx.project);
        assert!(tabs.open().iter().any(|n| n == "Other"));
        // A deleted view loses its tab.
        assert!(cx.project.delete_plan_view("Upstairs"));
        tabs.sync(&cx.project);
        assert!(!tabs.open().iter().any(|n| n == "Upstairs"));
        // Tabs reorder.
        let n = tabs.open().len();
        assert!(n >= 2);
        let last = tabs.open()[n - 1].clone();
        assert!(tabs.move_tab(n - 1, 0));
        assert_eq!(tabs.open()[0], last);
        assert!(!tabs.move_tab(0, 0) && !tabs.move_tab(9, 0));
    }

    #[test]
    fn save_and_reset_plan_view() {
        let mut cx = cx_with_views();
        let mut tabs = PlanTabs::default();
        tabs.open_view(&mut cx, "Upstairs");
        let _ = tabs.take_pending_camera();
        // Change the view, then save it.
        cx.floor = 0;
        cx.view_flags.remove(&ViewFlag::ReferenceDisplay);
        tabs.report_camera(Point::new(7.0, 8.0), 4.0);
        assert!(tabs.save_active(&mut cx));
        assert_eq!(cx.undo_label(), Some("Save Plan View"));
        let v = cx.project.plan_view("Upstairs").unwrap();
        assert_eq!(v.floor, Some(0));
        assert!(!v.reference_display);
        assert_eq!(v.camera, Some((Point::new(7.0, 8.0), 4.0)));
        // Move away, then reset: back to the saved state.
        cx.floor = 1;
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
        assert!(tabs.reset_active(&mut cx));
        assert_eq!(cx.floor, 0);
        assert!(!cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
        assert_eq!(
            tabs.take_pending_camera(),
            Some((Point::new(7.0, 8.0), 4.0))
        );
        // Saving is one undo step.
        cx.undo();
        assert_eq!(
            cx.project.plan_view("Upstairs").unwrap().camera,
            Some((Point::new(500.0, 300.0), 2.0))
        );
    }

    #[test]
    fn without_a_reported_camera_zoom_alone_is_not_stored() {
        let mut cx = cx_with_views();
        let mut tabs = PlanTabs::default();
        cx.px_per_in = 9.0;
        tabs.open_view(&mut cx, "Upstairs");
        // No centre known and none stored: the starting view keeps no camera.
        assert_eq!(
            cx.project.plan_view(DEFAULT_PLAN_VIEW_NAME).unwrap().camera,
            None
        );
        // The thread-local wrappers drive the same state.
        report_camera(Point::new(1.0, 2.0), 2.0);
        assert!(with_tabs(|t| t.switch_to(&mut cx, DEFAULT_PLAN_VIEW_NAME)));
        assert_eq!(
            cx.project.plan_view("Upstairs").unwrap().camera,
            Some((Point::new(1.0, 2.0), 2.0))
        );
        assert!(take_pending_camera().is_none());
    }

    #[test]
    fn a_new_plan_starts_with_the_template_plan_views_but_a_worked_plan_keeps_its_own() {
        let fresh = EditorContext::new(crate::plan_defaults::embedded());
        assert_eq!(fresh.project.plan_views.len(), 21);
        assert_eq!(fresh.project.plan_views[0].name, DEFAULT_PLAN_VIEW_NAME);
        for v in &fresh.project.plan_views {
            assert!(
                fresh.project.layer_sets.get(&v.layer_set).is_some(),
                "{} shows a layer set the plan lacks",
                v.name
            );
        }
        assert!(fresh.project.plan_view("Working Plan View").is_some());
        // A plan with views of its own is left alone.
        let mut cx = plain_cx();
        cx.project
            .add_plan_view(SavedPlanView::new("Mine", "Default Set"));
        assert_eq!(cx.seed_template_plan_views(), 0);
        assert_eq!(cx.project.plan_views.len(), 2);
        // A plain plan gets them once.
        let mut cx = plain_cx();
        assert_eq!(cx.seed_template_plan_views(), 20);
        assert_eq!(cx.seed_template_plan_views(), 0);
    }
}
