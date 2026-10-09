//! The Elevation Reference widget (reference manual "Elevation References",
//! p. 21; parity rows R-89, R-90) and the glue that applies a reference.
//!
//! A height field of a cabinet, device, symbol, opening, soffit or slab keeps
//! its number; the widget beside it says what that number is measured from
//! (Absolute, From Floor, From Finished Floor, From Terrain, From Ceiling,
//! From Roof) and whether it reaches the object's top or its bottom. The
//! choice is stored with the object's shared panels
//! (`PropTable::pages[key].elevation`), so an object that never chooses
//! anything stores nothing and the default, From Floor to Bottom, is what
//! every height field has always meant.
//!
//! A dialog opts in with one line next to its height field:
//! `elevation_ref::row(ui, "Elevation Reference")`. The row binds to the
//! Components/Object Information session the host armed for the dialog
//! ([`super::object_info::current`]); it draws nothing when there is none.
//!
//! [`Shifts`] is the other half: the scene and plan builders ask it how far to
//! move an object whose reference is not the default.

use super::object_info;
use crate::editor::roof_view;
use crate::editor::site_view;
use eframe::egui::{self, Ui};
use plan_core::elevation_ref::{
    local_bottom, ElevationBase, ElevationEdge, ElevationRef, Resolver, Surfaces,
};
use plan_core::geometry::point_in_polygon;
use plan_core::{Point, Project};
use std::cell::RefCell;
use std::collections::HashMap;

/// The terrain and roof surfaces of a plan, for the resolver.
pub struct AppSurfaces<'a> {
    project: &'a Project,
}

impl<'a> AppSurfaces<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self { project }
    }
}

impl Surfaces for AppSurfaces<'_> {
    fn terrain_at(&self, _floor: usize, at: Point) -> Option<f64> {
        site_view::terrain_elevation_at(self.project, at)
    }

    /// The lowest roof plane over `at` on the floor or above it (the underside
    /// a ceiling-hung or roof-hung object measures from).
    fn roof_at(&self, floor: usize, at: Point) -> Option<f64> {
        let mut best: Option<f64> = None;
        for f in self.project.floors.iter().skip(floor) {
            let set = roof_view::load(f);
            for p in &set.planes {
                let outline: Vec<Point> = p
                    .polygon3d
                    .iter()
                    .map(|v| Point::new(v[0], -v[2]))
                    .collect();
                if outline.len() < 3 || !point_in_polygon(at, &outline) {
                    continue;
                }
                let plane = plan_roof::RoofPlane {
                    polygon3d: p.polygon3d.clone(),
                    pitch_in_12: p.pitch,
                    baseline: p.baseline,
                    source_edge: 0,
                };
                if let Some(h) = plane.height_at(at) {
                    best = Some(best.map_or(h, |b| b.min(h)));
                }
            }
        }
        best
    }
}

/// How far each object with a non-default reference moves from where its
/// typed height puts it. Cheap to build: nothing is computed until an object
/// that chose a reference asks.
pub struct Shifts<'a> {
    project: &'a Project,
    surfaces: &'a dyn Surfaces,
    resolvers: RefCell<HashMap<usize, Resolver<'a>>>,
}

impl<'a> Shifts<'a> {
    pub fn new(project: &'a Project, surfaces: &'a dyn Surfaces) -> Self {
        Self {
            project,
            surfaces,
            resolvers: RefCell::new(HashMap::new()),
        }
    }

    /// The reference stored for the object under `key`.
    pub fn reference(&self, key: &str) -> Option<ElevationRef> {
        self.project.props.elevation_of(key)
    }

    /// Inches to add to the level-local elevation `stored` (the typed height
    /// of the object under `key`, tall `height`, standing at `at` on level
    /// `floor`) so that it sits where its reference puts it. Zero for an
    /// object with the default reference.
    pub fn shift(&self, floor: usize, key: &str, at: Point, stored: f64, height: f64) -> f64 {
        let Some(r) = self.reference(key) else {
            return 0.0;
        };
        let mut map = self.resolvers.borrow_mut();
        let resolver = map
            .entry(floor)
            .or_insert_with(|| Resolver::new(self.project, floor, self.surfaces));
        local_bottom(resolver, r, at, stored, height) - stored
    }
}

/// The level-local bottom of one object (see [`Shifts::shift`]), for callers
/// that ask once.
#[cfg(test)]
pub fn resolved_bottom(
    project: &Project,
    floor: usize,
    key: &str,
    at: Point,
    stored: f64,
    height: f64,
) -> f64 {
    let surfaces = AppSurfaces::new(project);
    let shifts = Shifts::new(project, &surfaces);
    stored + shifts.shift(floor, key, at, stored, height)
}

/// A copy of the plan in which every opening, symbol, cabinet and device that
/// measures its height from something other than the floor stands where its
/// reference puts it (its sill, elevation or height rewritten to the
/// level-local bottom), so the scene builders need no knowledge of
/// references. `None` when no object chose a reference: nothing is copied and
/// old plans build exactly as before.
pub fn effective_project(project: &Project) -> Option<Project> {
    use super::common_pages::{describe, Kind};
    if !project.props.has_elevation_refs() {
        return None;
    }
    let surfaces = AppSurfaces::new(project);
    let shifts = Shifts::new(project, &surfaces);
    let mut out = project.clone();
    let mut changed = false;
    for key in project.props.pages.keys() {
        if shifts.reference(key).is_none() {
            continue;
        }
        let Some(kind) = Kind::of_key(key) else {
            continue;
        };
        let Some(d) = describe(project, key) else {
            continue;
        };
        let id: Option<u64> = key.rsplit(':').next().and_then(|n| n.parse().ok());
        let Some(id) = id else { continue };
        let stored = d.facts.elevation.unwrap_or(0.0);
        let height = match kind {
            Kind::Device => 0.0,
            _ => d.facts.height.unwrap_or(0.0),
        };
        let shift = shifts.shift(d.floor, key, d.at, stored, height);
        if shift.abs() < 1e-9 {
            continue;
        }
        let floor = &mut out.floors[d.floor];
        match kind {
            Kind::Door | Kind::Window => {
                if let Some(o) = floor.openings.iter_mut().find(|o| o.id == id) {
                    o.sill_height += shift;
                }
            }
            Kind::Symbol => {
                if let Some(s) = floor.symbols.iter_mut().find(|s| s.id == id) {
                    s.elevation += shift;
                }
            }
            Kind::Cabinet => {
                let mut cabs = crate::editor::placed::load_cabinets(floor);
                if let Some(c) = cabs.iter_mut().find(|c| c.id == id) {
                    c.elevation += shift;
                }
                let _ = floor.set_cabinets(&cabs);
            }
            Kind::Device => {
                let mut layer = site_view::load_electrical(floor);
                if let Some(dv) = layer.device_mut(id) {
                    dv.height += shift;
                }
                site_view::save_electrical(&mut out, d.floor, &layer);
            }
            _ => continue,
        }
        changed = true;
    }
    changed.then_some(out)
}

/// The widget: the base in a drop-down and the To Top / To Bottom choice.
/// Returns whether `r` changed.
pub fn widget(ui: &mut Ui, id_salt: &str, r: &mut ElevationRef) -> bool {
    let before = *r;
    egui::ComboBox::from_id_salt(("elevation_ref", id_salt))
        .selected_text(r.base.name())
        .width(170.0)
        .show_ui(ui, |ui| {
            for b in ElevationBase::ALL {
                ui.selectable_value(&mut r.base, b, b.name());
            }
        })
        .response
        .on_hover_text("What the height next to this box is measured from");
    for e in ElevationEdge::ALL {
        ui.radio_value(&mut r.edge, e, e.name())
            .on_hover_text(match e {
                ElevationEdge::ToBottom => "The height reaches the bottom of the object",
                ElevationEdge::ToTop => "The height reaches the top of the object",
            });
    }
    *r != before
}

/// The one-line opt-in: a labelled widget bound to the dialog's armed
/// session. Draws nothing when the host armed none (an object kind that takes
/// no shared panels).
pub fn row(ui: &mut Ui, label: &str) {
    let Some(session) = object_info::current() else {
        return;
    };
    let mut s = session.borrow_mut();
    if !s.takes_elevation() {
        return;
    }
    let mut r = s.elevation();
    super::row(ui, label, |ui| {
        if widget(ui, &s.key, &mut r) {
            s.set_elevation(r);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::model::WallKind;

    fn house() -> Project {
        let mut p = Project::new("elev");
        let pts = [(0.0, 0.0), (240.0, 0.0), (240.0, 144.0), (0.0, 144.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p.floors[0].ceiling_height = 96.0;
        p
    }

    fn set(p: &mut Project, key: &str, base: ElevationBase, edge: ElevationEdge) {
        let mut pages = p.props.pages_of(key).cloned().unwrap_or_default();
        pages.elevation = Some(ElevationRef::new(base, edge));
        assert!(p.props.set_pages(key, pages));
    }

    #[test]
    fn an_object_with_no_reference_does_not_move() {
        let p = house();
        let at = Point::new(60.0, 60.0);
        assert_eq!(resolved_bottom(&p, 0, "cabinet:1", at, 54.0, 30.0), 54.0);
        let surf = AppSurfaces::new(&p);
        assert!(!p.props.has_elevation_refs());
        let _ = surf;
    }

    #[test]
    fn a_cabinet_measured_from_the_ceiling_follows_the_ceiling_height() {
        let mut p = house();
        let at = Point::new(60.0, 60.0);
        // 30 in tall, its top 6 in below the ceiling: value -6 to the top.
        set(
            &mut p,
            "cabinet:1",
            ElevationBase::FromCeiling,
            ElevationEdge::ToTop,
        );
        assert_eq!(resolved_bottom(&p, 0, "cabinet:1", at, -6.0, 30.0), 60.0);
        p.floors[0].ceiling_height = 108.0;
        assert_eq!(resolved_bottom(&p, 0, "cabinet:1", at, -6.0, 30.0), 72.0);
    }

    #[test]
    fn absolute_ignores_the_level_and_terrain_falls_back_to_the_floor() {
        let mut p = house();
        p.floors[0].elevation = 12.0;
        let at = Point::new(60.0, 60.0);
        set(
            &mut p,
            "symbol:2",
            ElevationBase::Absolute,
            ElevationEdge::ToBottom,
        );
        // The level is 12 in up, so 30 in absolute is 18 in above it.
        assert_eq!(resolved_bottom(&p, 0, "symbol:2", at, 30.0, 10.0), 18.0);
        set(
            &mut p,
            "symbol:3",
            ElevationBase::FromTerrain,
            ElevationEdge::ToBottom,
        );
        // No terrain: From Floor.
        assert_eq!(resolved_bottom(&p, 0, "symbol:3", at, 5.0, 10.0), 5.0);
    }
}
