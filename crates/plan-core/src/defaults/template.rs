//! Plan templates (manual pp. 110 to 114): what a template file holds, Save
//! as Template's purge of unwanted data, and starting a new plan from one.
//!
//! A template carries what Chief's does: the Default Settings, the Default
//! Sets and Multiple Saved Defaults, the layers and layer sets, the Saved Plan
//! Views, the wall type definitions, the text macros, the note types, the
//! CAD Details and anything drawn in the plan. Save as Template purges the
//! categories of data the user ticks ([`PurgeCategory`]) and saves a copy.

use super::PlanDefaults;
use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A category of data Save as Template can delete from the copy it saves
/// (the checklist of the Save as Plan Template dialog, manual p. 112).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PurgeCategory {
    Walls,
    DoorsAndWindows,
    Rooms,
    Cabinets,
    Stairs,
    RoofsAndCeilings,
    Electrical,
    Framing,
    Foundation,
    Details,
    Symbols,
    Dimensions,
    CadAndText,
    Schedules,
    Underlays,
    Cameras,
    Terrain,
    ExtraFloors,
    CadDetails,
}

impl PurgeCategory {
    pub const ALL: [PurgeCategory; 19] = [
        PurgeCategory::Walls,
        PurgeCategory::DoorsAndWindows,
        PurgeCategory::Rooms,
        PurgeCategory::Cabinets,
        PurgeCategory::Stairs,
        PurgeCategory::RoofsAndCeilings,
        PurgeCategory::Electrical,
        PurgeCategory::Framing,
        PurgeCategory::Foundation,
        PurgeCategory::Details,
        PurgeCategory::Symbols,
        PurgeCategory::Dimensions,
        PurgeCategory::CadAndText,
        PurgeCategory::Schedules,
        PurgeCategory::Underlays,
        PurgeCategory::Cameras,
        PurgeCategory::Terrain,
        PurgeCategory::ExtraFloors,
        PurgeCategory::CadDetails,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PurgeCategory::Walls => "Walls",
            PurgeCategory::DoorsAndWindows => "Doors and Windows",
            PurgeCategory::Rooms => "Rooms and Room Labels",
            PurgeCategory::Cabinets => "Cabinets",
            PurgeCategory::Stairs => "Stairs",
            PurgeCategory::RoofsAndCeilings => "Roofs and Ceilings",
            PurgeCategory::Electrical => "Electrical",
            PurgeCategory::Framing => "Framing",
            PurgeCategory::Foundation => "Slabs, Pads and Piers",
            PurgeCategory::Details => "Moldings, Trim, Decks and 3D Solids",
            PurgeCategory::Symbols => "Library Objects and Fireplaces",
            PurgeCategory::Dimensions => "Dimensions",
            PurgeCategory::CadAndText => "CAD, Text, Callouts and Markers",
            PurgeCategory::Schedules => "Schedules",
            PurgeCategory::Underlays => "Underlays and Images",
            PurgeCategory::Cameras => "Cameras",
            PurgeCategory::Terrain => "Terrain",
            PurgeCategory::ExtraFloors => "Floors above the first",
            PurgeCategory::CadDetails => "CAD Details",
        }
    }
}

impl Project {
    /// Deletes the data of the chosen categories from every floor (CAD Detail
    /// floors are left alone unless [`PurgeCategory::CadDetails`] is chosen,
    /// because a template keeps its detail drawings). Returns how many things
    /// were removed. Layers, layer sets, plan views, wall types, text styles,
    /// macros, note types and the saved defaults are never purged.
    pub fn purge(&mut self, cats: &BTreeSet<PurgeCategory>) -> usize {
        use PurgeCategory as P;
        let has = |c: P| cats.contains(&c);
        let mut n = 0;
        if has(P::CadDetails) {
            let before = self.floors.len();
            self.floors.retain(|f| f.detail.is_none());
            n += before - self.floors.len();
            if self.floors.is_empty() {
                self.floors
                    .push(crate::model::Floor::new("First Floor", 0.0));
            }
        }
        if has(P::ExtraFloors) {
            // Keep the first floor that is not a CAD detail, and the details.
            let mut seen_plan_floor = false;
            let before = self.floors.len();
            self.floors.retain(|f| {
                if f.detail.is_some() {
                    return true;
                }
                let keep = !seen_plan_floor;
                seen_plan_floor = true;
                keep
            });
            n += before - self.floors.len();
        }
        for f in self.floors.iter_mut().filter(|f| f.detail.is_none()) {
            if has(P::Walls) {
                n += f.walls.len();
                f.walls.clear();
                // Openings stand in walls.
                n += f.openings.len();
                f.openings.clear();
            }
            if has(P::DoorsAndWindows) {
                n += f.openings.len();
                f.openings.clear();
            }
            if has(P::Rooms) {
                n += f.room_names.len() + f.exterior_rooms.len();
                f.room_names.clear();
                f.exterior_rooms.clear();
            }
            if has(P::Cabinets) {
                n += f.cabinets.len();
                f.cabinets.clear();
            }
            if has(P::Stairs) {
                n += f.stairs.len();
                f.stairs.clear();
            }
            if has(P::RoofsAndCeilings) {
                n += f.roofs.len() + f.trays.trays.len();
                f.roofs.clear();
                f.trays = Default::default();
            }
            if has(P::Electrical) && f.electrical.take().is_some() {
                n += 1;
            }
            if has(P::Framing) {
                n += f.framing.len();
                f.framing.clear();
            }
            if has(P::Foundation) && f.foundation.take().is_some() {
                n += 1;
            }
            if has(P::Details) {
                if f.details.take().is_some() {
                    n += 1;
                }
                f.region_layers.clear();
                f.solid_layer = Default::default();
            }
            if has(P::Symbols) {
                n += f.symbols.len() + f.fireplaces.len();
                f.symbols.clear();
                f.fireplaces.clear();
            }
            if has(P::Dimensions) {
                n += f.dimensions.len();
                f.dimensions.clear();
            }
            if has(P::CadAndText) {
                n += f.cad.len();
                f.cad.clear();
                f.cad_attrs.clear();
                f.cad_blocks.clear();
                f.construction = Default::default();
                f.annots = Default::default();
                f.blocks = Default::default();
                f.groups.clear();
                f.drawing_groups.clear();
            }
            if has(P::Schedules) && f.schedules.take().is_some() {
                n += 1;
            }
            if has(P::Underlays) {
                n += f.underlays.len();
                f.underlays.clear();
            }
        }
        if has(P::Cameras) {
            n += self.cameras.len();
            self.cameras.clear();
        }
        if has(P::Terrain) && self.terrain.take().is_some() {
            n += 1;
        }
        n
    }
}

/// The version of the template file layout.
pub const TEMPLATE_VERSION: u32 = 1;

/// A plan template file: the plan (purged) with its defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    pub version: u32,
    pub name: String,
    /// U.S. units (feet and inches) or metric: a plan only starts from a
    /// template of its own units.
    pub imperial: bool,
    pub project: Project,
    pub defaults: PlanDefaults,
}

impl PlanTemplate {
    /// A template of `project` and `defaults` named `name`, after purging
    /// the chosen categories from the copy. The project is renamed so a plan
    /// made from the template starts untitled.
    pub fn from_plan(
        name: &str,
        project: &Project,
        defaults: &PlanDefaults,
        purge: &BTreeSet<PurgeCategory>,
    ) -> PlanTemplate {
        let mut p = project.clone();
        p.purge(purge);
        p.name = "Untitled".into();
        // What is recorded about objects (Use Default states, which saved
        // default made which object) means nothing once they are gone.
        if !purge.is_empty() {
            p.saved_defaults.follow = Default::default();
            p.saved_defaults.uses.clear();
        }
        let mut d = defaults.clone();
        // The saved defaults travel with the plan's own lists.
        p.saved_commit_all(&mut d);
        PlanTemplate {
            version: TEMPLATE_VERSION,
            name: name.to_string(),
            imperial: defaults.units.imperial,
            project: p,
            defaults: d,
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<PlanTemplate> {
        serde_json::from_str(s)
    }

    /// The plan and defaults a new plan made from the template starts as.
    pub fn instantiate(&self) -> (Project, PlanDefaults) {
        let mut p = self.project.clone();
        p.name = "Untitled".into();
        (p, self.defaults.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::WallKind;

    fn plan() -> (Project, PlanDefaults) {
        let d = PlanDefaults::chief_x18_daniel();
        let mut p = Project::from_defaults("My Job", &d);
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(0.0, 120.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let o = crate::model::Opening::default_door(0, w, 40.0);
        p.floors[0].openings.push(o);
        p.floors[0].cabinets.push(serde_json::json!({"id": 1}));
        p.floors[0].electrical = Some(serde_json::json!({}));
        p.insert_floor_above(0).unwrap();
        p.floors[1].stairs.push(serde_json::json!({"id": 2}));
        p.terrain = Some(serde_json::json!({"x": 1}));
        (p, d)
    }

    fn cats(c: &[PurgeCategory]) -> BTreeSet<PurgeCategory> {
        c.iter().copied().collect()
    }

    #[test]
    fn purge_deletes_only_the_chosen_categories() {
        let (mut p, _) = plan();
        let sets = p.layer_sets.sets.len();
        let n = p.purge(&cats(&[PurgeCategory::Cabinets, PurgeCategory::Terrain]));
        assert_eq!(n, 2);
        assert!(p.floors[0].cabinets.is_empty() && p.terrain.is_none());
        assert_eq!(p.floors[0].walls.len(), 2, "walls stay");
        assert_eq!(p.floors[0].openings.len(), 1);
        assert!(p.floors[0].electrical.is_some());
        // Walls take their openings with them.
        p.purge(&cats(&[PurgeCategory::Walls]));
        assert!(p.floors[0].walls.is_empty() && p.floors[0].openings.is_empty());
        // Data that is not an object is never purged.
        assert_eq!(p.layer_sets.sets.len(), sets);
        assert!(!p.wall_types.is_empty() && !p.plan_views.is_empty());
        // Purging everything leaves a plan that still works.
        let n = p.purge(&PurgeCategory::ALL.iter().copied().collect());
        assert!(n > 0);
        assert_eq!(p.floors.len(), 1, "extra floors go");
        assert!(p.floors[0].stairs.is_empty());
        assert_eq!(
            p.purge(&PurgeCategory::ALL.iter().copied().collect()),
            0,
            "nothing twice"
        );
    }

    #[test]
    fn a_template_keeps_cad_detail_floors_unless_asked() {
        let (mut p, _) = plan();
        let mut detail = crate::model::Floor::new("Detail 1", 0.0);
        detail.detail = Some(Default::default());
        detail.dimensions.push(crate::dimension::Dimension {
            id: 9,
            kind: crate::dimension::DimensionKind::Manual,
            start: Point::ZERO,
            end: Point::new(10.0, 0.0),
            offset: 0.0,
            text_override: None,
            anchors: [None, None],
            hide_ext: [false, false],
            auto_group: Default::default(),
            text_style: None,
            look: Default::default(),
        });
        p.floors.push(detail);
        let all: BTreeSet<_> = PurgeCategory::ALL
            .iter()
            .copied()
            .filter(|c| *c != PurgeCategory::CadDetails)
            .collect();
        p.purge(&all);
        assert!(p.floors.iter().any(|f| f.detail.is_some()));
        assert_eq!(
            p.floors
                .iter()
                .find(|f| f.detail.is_some())
                .unwrap()
                .dimensions
                .len(),
            1
        );
        p.purge(&cats(&[PurgeCategory::CadDetails]));
        assert!(p.floors.iter().all(|f| f.detail.is_none()));
    }

    #[test]
    fn a_template_round_trips_and_starts_an_untitled_plan_with_the_defaults() {
        let (mut p, mut d) = plan();
        p.saved_copy(
            &mut d,
            crate::defaults::saved::SavedKind::RichText,
            "Default",
            "Plot",
        )
        .unwrap();
        d.units.imperial = true;
        let t = PlanTemplate::from_plan("Mine", &p, &d, &cats(&[PurgeCategory::Walls]));
        assert_eq!(t.project.name, "Untitled");
        assert!(t.project.floors[0].walls.is_empty());
        assert_eq!(p.floors[0].walls.len(), 2, "the open plan is untouched");
        let back = PlanTemplate::from_json(&t.to_json().unwrap()).unwrap();
        let (np, nd) = back.instantiate();
        assert_eq!(np.name, "Untitled");
        assert_eq!(nd, t.defaults);
        assert!(np
            .saved_defaults
            .list(crate::defaults::saved::SavedKind::RichText)
            .unwrap()
            .items
            .iter()
            .any(|s| s.name == "Plot"));
        assert!(back.imperial);
    }
}
