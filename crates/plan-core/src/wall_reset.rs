//! Edit > Reset to Defaults (manual pp. 118-119, DECISIONS DS4 follow-up):
//! puts critical structural values inside the plan back to their defaults, for
//! the current floor or for all floors. The dialog lives in
//! `plan-app/src/dialogs/reset_plan_values.rs`; this is the model work.

use crate::model::{Project, Wall};
use crate::walls::WallRoofDirective;

/// Which settings to reset (the check boxes of the dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResetOptions {
    /// Room floor-height offsets and ceiling-height overrides.
    pub floor_and_ceiling_heights: bool,
    /// Every room back to Roof Group 0.
    pub roof_groups: bool,
    /// The Roof panel of every Wall Specification.
    pub wall_roof_directives: bool,
    /// Delete the Roof Gable Lines.
    pub gable_lines: bool,
    /// Wall tops and bottoms edited in 3D, sections and elevations: height
    /// back to the floor's, bottom back to the floor, steps and rakes gone.
    pub wall_heights: bool,
    /// The Auto Connect locks and dismissed notification icons.
    pub wall_auto_connections: bool,
    /// Suppressed dimension values and replacement text, entire plan.
    pub overridden_dimension_text: bool,
}

impl ResetOptions {
    /// Every box checked.
    pub fn all() -> Self {
        Self {
            floor_and_ceiling_heights: true,
            roof_groups: true,
            wall_roof_directives: true,
            gable_lines: true,
            wall_heights: true,
            wall_auto_connections: true,
            overridden_dimension_text: true,
        }
    }

    /// Whether anything is checked.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }
}

/// How many objects each reset changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResetReport {
    pub room_heights: usize,
    pub roof_groups: usize,
    pub wall_roof_directives: usize,
    pub gable_lines: usize,
    pub wall_heights: usize,
    pub wall_auto_connections: usize,
    pub dimension_texts: usize,
}

impl ResetReport {
    pub fn total(&self) -> usize {
        self.room_heights
            + self.roof_groups
            + self.wall_roof_directives
            + self.gable_lines
            + self.wall_heights
            + self.wall_auto_connections
            + self.dimension_texts
    }
}

/// A wall that takes the floor's ceiling height: not a railing, half wall,
/// pony wall, foundation wall, room divider or generated platform wall.
fn takes_floor_height(w: &Wall) -> bool {
    w.class.is_standard()
        && !w.flags.railing
        && !w.flags.half_wall
        && !w.flags.foundation
        && !w.flags.room_divider
        && !w.flags.auto_generated
        && w.flags.pony.is_none()
}

impl Project {
    /// Resets the checked settings on floor `current`, or on every floor when
    /// `all_floors` (the dimension text is always the entire plan, as the
    /// dialog says). Returns what changed; the caller reconnects the walls
    /// when `wall_auto_connections` changed any.
    pub fn reset_plan_values(
        &mut self,
        current: usize,
        all_floors: bool,
        o: &ResetOptions,
    ) -> ResetReport {
        let mut r = ResetReport::default();
        let range: Vec<usize> = if all_floors {
            (0..self.floors.len()).collect()
        } else if current < self.floors.len() {
            vec![current]
        } else {
            Vec::new()
        };
        for fi in range {
            let f = &mut self.floors[fi];
            if o.floor_and_ceiling_heights {
                for n in &mut f.room_names {
                    if n.floor_height_offset != 0.0 || n.ceiling_height.is_some() {
                        n.floor_height_offset = 0.0;
                        n.ceiling_height = None;
                        r.room_heights += 1;
                    }
                }
            }
            if o.roof_groups {
                for n in &mut f.room_names {
                    if n.roof_group != 0 {
                        n.roof_group = 0;
                        r.roof_groups += 1;
                    }
                }
            }
            if o.gable_lines {
                let before = f.roofs.len();
                f.roofs.retain(|v| {
                    v.get("kind").and_then(|k| k.as_str()) != Some("gable_line")
                });
                r.gable_lines += before - f.roofs.len();
            }
            let ceiling = f.ceiling_height;
            for w in &mut f.walls {
                if o.wall_roof_directives && w.roof != WallRoofDirective::default() {
                    w.roof = WallRoofDirective::default();
                    r.wall_roof_directives += 1;
                }
                if o.wall_heights && takes_floor_height(w) {
                    let profile = !w.spec.profile.is_plain();
                    if (w.height - ceiling).abs() > 1e-9 || w.bottom_offset != 0.0 || profile {
                        w.height = ceiling;
                        w.bottom_offset = 0.0;
                        w.spec.profile = Default::default();
                        r.wall_heights += 1;
                    }
                }
                if o.wall_auto_connections {
                    let fl = &mut w.flags;
                    if fl.lock_start || fl.lock_end || fl.ignore_off_angle || fl.ignore_unconnected
                    {
                        fl.lock_start = false;
                        fl.lock_end = false;
                        fl.ignore_off_angle = false;
                        fl.ignore_unconnected = false;
                        r.wall_auto_connections += 1;
                    }
                }
            }
        }
        if o.overridden_dimension_text {
            for f in &mut self.floors {
                for d in &mut f.dimensions {
                    if d.text_override.is_some() {
                        d.text_override = None;
                        r.dimension_texts += 1;
                    }
                }
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::{Id, RoomName, WallKind};

    fn plan() -> (Project, Id, Id) {
        let mut p = Project::new("t");
        let ceiling = p.floors[0].ceiling_height;
        let a = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            6.0,
            ceiling + 24.0,
            WallKind::Exterior,
        );
        let b = p.add_wall(
            0,
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            6.0,
            ceiling,
            WallKind::Exterior,
        );
        (p, a, b)
    }

    #[test]
    fn each_box_resets_only_its_own_values() {
        let (mut p, a, b) = plan();
        let ceiling = p.floors[0].ceiling_height;
        p.floors[0].wall_mut(a).unwrap().flags.lock_end = true;
        p.floors[0].wall_mut(b).unwrap().roof.overhang = Some(30.0);
        p.floors[0].room_names.push(RoomName {
            ceiling_height: Some(144.0),
            roof_group: 2,
            ..RoomName::default()
        });
        p.floors[0].roofs.push(serde_json::json!({"kind": "gable_line", "id": 9}));
        p.floors[0].roofs.push(serde_json::json!({"kind": "other"}));

        let only_heights = ResetOptions {
            wall_heights: true,
            ..ResetOptions::default()
        };
        let r = p.reset_plan_values(0, false, &only_heights);
        assert_eq!(r.wall_heights, 1);
        assert_eq!(p.floors[0].wall(a).unwrap().height, ceiling);
        assert!(p.floors[0].wall(a).unwrap().flags.lock_end, "locks stay");
        assert_eq!(p.floors[0].roofs.len(), 2);

        let r = p.reset_plan_values(0, false, &ResetOptions::all());
        assert_eq!(r.wall_auto_connections, 1);
        assert_eq!(r.wall_roof_directives, 1);
        assert_eq!(r.gable_lines, 1);
        assert_eq!((r.room_heights, r.roof_groups), (1, 1));
        assert_eq!(p.floors[0].roofs.len(), 1, "other roof objects stay");
        assert!(!p.floors[0].wall(a).unwrap().flags.lock_end);
        assert!(p.floors[0].wall(b).unwrap().roof.overhang.is_none());
        // Nothing left to do the second time.
        assert_eq!(p.reset_plan_values(0, false, &ResetOptions::all()).total(), 0);
    }

    #[test]
    fn the_scope_picks_the_floors_but_dimension_text_is_the_whole_plan() {
        let (mut p, a, _) = plan();
        p.insert_floor_above(0).unwrap();
        let id = p.alloc_id();
        let up = p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(80.0, 0.0),
            6.0,
            200.0,
            WallKind::Interior,
        );
        p.floors[1].wall_mut(up).unwrap().flags.lock_start = true;
        p.floors[0].wall_mut(a).unwrap().flags.lock_start = true;
        let mut d = crate::dimension::Dimension::new(
            id,
            crate::dimension::DimensionKind::Manual,
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            12.0,
        );
        d.text_override = Some("EQ".into());
        p.floors[1].dimensions.push(d);
        let o = ResetOptions {
            wall_auto_connections: true,
            overridden_dimension_text: true,
            ..ResetOptions::default()
        };
        let r = p.reset_plan_values(0, false, &o);
        assert_eq!(r.wall_auto_connections, 1, "only the current floor");
        assert!(p.floors[1].wall(up).unwrap().flags.lock_start);
        assert_eq!(r.dimension_texts, 1, "the entire plan");
        let r = p.reset_plan_values(0, true, &o);
        assert_eq!(r.wall_auto_connections, 1);
        assert!(!p.floors[1].wall(up).unwrap().flags.lock_start);
    }

    #[test]
    fn railings_and_half_walls_keep_their_own_height() {
        let (mut p, a, _) = plan();
        let w = p.floors[0].wall_mut(a).unwrap();
        w.flags.half_wall = true;
        w.height = 42.0;
        let o = ResetOptions {
            wall_heights: true,
            ..ResetOptions::default()
        };
        assert_eq!(p.reset_plan_values(0, false, &o).wall_heights, 0);
        assert_eq!(p.floors[0].wall(a).unwrap().height, 42.0);
    }
}
