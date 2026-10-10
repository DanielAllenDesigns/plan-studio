//! Routes the shared edit commands (Delete, Copy, Paste in Place, Reverse
//! Swing and the module-specific Edit toolbar commands) to the module that
//! owns each object kind: `stairs_view`, `placed`, `roof_view`, `site_view`,
//! `foundation_view` and the camera tool.

use super::actions::{EditAction, EditActionKind};
use super::selection::ObjectRef;
use super::{
    details_view, foundation_view, framing_view, placed, roof_view, schedule_view, site_view,
    stairs_view, EditorContext, EditorRequest,
};
use crate::shell::view3d_panel::{Outbox, ViewRequest};
use crate::tools::roof::RoofMode;
use crate::tools::ToolId;
use plan_core::Id;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Custom command ids (the `id` of [`EditActionKind::Custom`]).
pub mod cmd {
    pub const STAIR_WELL: &str = "stair.auto_stairwell";
    pub const STAIR_FLARE: &str = "stair.flare_curve";
    pub const STAIR_BREAK: &str = "stair.breakline";
    pub const STAIR_RAILING: &str = "stair.railing";
    pub const ROOF_JOIN: &str = "roof.join";
    pub const ROOF_REBUILD: &str = "roof.rebuild";
    pub const ROOF_DELETE_ALL: &str = "roof.delete_all";
    pub const ROOF_DELETE_CEILINGS: &str = "roof.delete_ceilings";
    pub const ROOF_EXPLODE_DORMER: &str = "roof.explode_dormer";
    pub const SYMBOL_REPLACE: &str = "symbol.replace";
    pub const DEVICE_FLIP: &str = "device.flip";
    pub const DEVICE_ROTATE: &str = "device.rotate";
}

fn stair_id(c: stairs_view::StairCommand) -> &'static str {
    c.id()
}

fn stair_command(id: &str) -> Option<stairs_view::StairCommand> {
    stairs_view::StairCommand::ALL
        .into_iter()
        .find(|c| stair_id(*c) == id)
}

fn is_core(o: ObjectRef) -> bool {
    matches!(
        o,
        ObjectRef::Wall(_)
            | ObjectRef::Opening(_)
            | ObjectRef::Dimension(_)
            | ObjectRef::Cad(_)
            | ObjectRef::Text(_)
    )
}

impl EditorContext {
    fn selected_ids(&self, pick: impl Fn(ObjectRef) -> Option<Id>) -> Vec<Id> {
        self.selection
            .items
            .iter()
            .filter_map(|o| pick(*o))
            .collect()
    }

    /// Does the selection hold anything the walls/openings/dimensions/CAD
    /// code in `ops` handles?
    pub(super) fn selection_has_core(&self) -> bool {
        self.selection.items.iter().any(|o| is_core(*o))
    }

    /// Deletes the selected stairs, cabinets, symbols, devices, roof records,
    /// foundation objects, details, schedules, terrain elements and cameras (each kind is one undo step),
    /// and the wall material regions and hatches of deleted walls (inside the
    /// walls' own undo step). Returns how many went.
    pub(super) fn delete_extra(&mut self) -> usize {
        let mut n = 0;
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Wall(_)))
        {
            n += details_view::drop_orphans(self);
        }
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Stair(_)))
        {
            n += stairs_view::delete_selected(self);
        }
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Cabinet(_) | ObjectRef::Symbol(_)))
        {
            n += placed::delete_placed(self);
            // A deleted fireplace takes its specification with it.
            super::fireplace_view::drop_orphans(self);
        }
        let devices = self.selected_ids(|o| match o {
            ObjectRef::Device(i) => Some(i),
            _ => None,
        });
        if !devices.is_empty() {
            site_view::edit_electrical(self, "Delete", |layer, _| {
                for id in &devices {
                    layer.remove(*id);
                }
            });
            self.selection
                .items
                .retain(|o| !matches!(o, ObjectRef::Device(_)));
            n += devices.len();
        }
        let planes = self.selected_ids(|o| match o {
            ObjectRef::RoofPlane(i) => Some(i),
            _ => None,
        });
        if !planes.is_empty() {
            self.begin_change("Delete Roof Plane");
            let fl = self.floor;
            n += roof_view::delete_records(&mut self.project, fl, &planes);
            self.selection
                .items
                .retain(|o| !matches!(o, ObjectRef::RoofPlane(_)));
            self.mark_dirty();
        }
        let foundation = self.selected_ids(|o| match o {
            ObjectRef::Foundation(i) => Some(i),
            _ => None,
        });
        if !foundation.is_empty() {
            n += foundation_view::delete_ids(self, &foundation);
            self.selection
                .items
                .retain(|o| !matches!(o, ObjectRef::Foundation(_)));
        }
        let framing = self.selected_ids(|o| match o {
            ObjectRef::Framing(i) => Some(i),
            _ => None,
        });
        if !framing.is_empty() {
            // delete_records drops them from the selection itself.
            n += framing_view::delete_records(self, &framing);
        }
        let details = self.selected_ids(|o| match o {
            ObjectRef::Detail(i) => Some(i),
            _ => None,
        });
        if !details.is_empty() {
            // delete_ids drops them from the selection itself.
            n += details_view::delete_ids(self, &details);
        }
        let compounds = self.selected_ids(|o| match o {
            ObjectRef::Solid(i) => Some(i),
            _ => None,
        });
        if !compounds.is_empty() {
            self.begin_change("Delete 3D Solid");
            // delete_ids drops them from the selection itself.
            n += super::solids_view::delete_ids(self, &compounds);
        }
        let schedules = self.selected_ids(|o| match o {
            ObjectRef::Schedule(i) => Some(i),
            _ => None,
        });
        if !schedules.is_empty() {
            // delete_ids drops them from the selection itself.
            n += schedule_view::delete_ids(self, &schedules);
        }
        let terrain: Vec<site_view::TerrainHit> = self
            .selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::TerrainObject(h) => Some(*h),
                _ => None,
            })
            .collect();
        if !terrain.is_empty() {
            let mut removed = 0;
            site_view::edit_terrain(self, "Delete Terrain Element", |rec| {
                removed = site_view::remove_terrain_elements(&mut rec.terrain, &terrain);
            });
            if removed == 0 {
                self.cancel_change();
            }
            // The remaining indices are stale: nothing of the terrain stays selected.
            self.selection
                .items
                .retain(|o| !matches!(o, ObjectRef::TerrainObject(_)));
            n += removed;
        }
        let cams = self.selected_ids(|o| match o {
            ObjectRef::Camera(i) => Some(i),
            _ => None,
        });
        if !cams.is_empty() {
            self.begin_change("Delete Camera");
            for id in &cams {
                if self.project.remove_camera(*id) {
                    n += 1;
                    Outbox::global().post(ViewRequest::CameraDeleted(*id));
                }
            }
            self.selection
                .items
                .retain(|o| !matches!(o, ObjectRef::Camera(_)));
            self.mark_dirty();
        }
        n
    }

    /// Does the Paste in Place button have anything to paste?
    pub(super) fn has_clipboard(&self) -> bool {
        self.clipboard.as_ref().is_some_and(|c| !c.is_empty())
    }

    /// The module-specific Edit toolbar buttons of the selection.
    pub fn extra_edit_actions(&self) -> Vec<EditAction> {
        let mut v = Vec::new();
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Cabinet(_)))
        {
            let mut a = EditAction::new(EditActionKind::ReverseSwing);
            a.label = "Reverse Door Swing";
            v.push(a);
            // Open/Close Cabinet Doors/Drawers, Generate Custom Countertop,
            // Make Cabinet Molding Polyline (CB-486, CB-490).
            v.extend(super::cabinet_edit::edit_actions(self));
        }
        // Calculate Materials in Room, Turn Ceiling On/Off, the room
        // polylines, Create Schedule from Room, elevation views and Auto
        // Room Dimensions (R-107, R-110).
        v.extend(super::rooms_edit::edit_actions(self));
        // Reverse Layers, Break Wall, Change Line/Arc, Make Arc Tangent.
        v.extend(super::wall_edit::edit_actions(self));
        // Center on Wall Segment, Mull, Unmull.
        v.extend(super::opening_edit::edit_actions(self));
        // Renumber Schedule, Open Row Object(s), Find Schedule(s), Move Up /
        // Down in Schedule, Create Schedule from Room.
        v.extend(super::schedule_view::edit_actions(self));
        // Hip / Full Gable / High Shed / Knee / Dutch Gable Wall (RF-4).
        v.extend(roof_view::wall_edit_actions(self));
        // Fireplace Specification, deck framing and steps at level changes.
        v.extend(super::fireplace_view::edit_actions(self));
        // Build Framing for Selected / Parent Object(s), Wall and Truss Details,
        // Move to Framing Ref, joins and breaks.
        v.extend(super::framing_view::edit_actions(self));
        // Polyline Boolean, Trim/Extend to Boundary, Insert Point, Multiple Copy.
        v.extend(crate::tools::cad_ops::edit_actions(self));
        // Architectural blocks, 3D solid Booleans, material layers, distribution options.
        v.extend(crate::tools::arch_block::edit_actions(self));
        // Object Layer Properties (primary and secondary layers).
        v.extend(crate::dialogs::object_layers::edit_actions(self));
        // Callout links, Note Schedules, Convert Text to Note, hyperlinks.
        v.extend(crate::tools::text::edit_actions(self));
        // Selected 3D solids become a User Catalog symbol.
        if crate::tools::library::convert::can_convert(self) {
            let label = "Convert to Symbol";
            v.push(EditAction {
                kind: EditActionKind::Custom {
                    id: crate::tools::library::convert::CONVERT_TO_SYMBOL,
                    label,
                    icon: "",
                },
                label,
                icon: None,
                enabled: true,
            });
        }
        let Some(one) = self.selection.single() else {
            return v;
        };
        let custom = |id, label, icon, enabled| EditAction {
            kind: EditActionKind::Custom { id, label, icon },
            label,
            icon: (!icon.is_empty()).then_some(icon),
            enabled,
        };
        match one {
            // The CAD edit tools (fillet, chamfer, offset, ...) under Select.
            ObjectRef::Cad(_) => {
                v.extend(crate::tools::cad::edit_actions(self));
                // Specification, Set as Default and the conversions (CAD-67).
                v.extend(crate::dialogs::construction_line::edit_actions(self));
            }
            ObjectRef::Dimension(_) => v.extend(crate::tools::dimension::edit_actions(self)),
            ObjectRef::Stair(_) => {
                for (c, on) in stairs_view::edit_commands(self) {
                    v.push(custom(stair_id(c), c.label(), c.icon().unwrap_or(""), on));
                }
            }
            ObjectRef::RoofPlane(id) => {
                if roof_view::load(self.floor()).dormer(id).is_some() {
                    v.push(custom(
                        cmd::ROOF_EXPLODE_DORMER,
                        "Explode Dormer",
                        "dormer",
                        true,
                    ));
                } else {
                    v.push(custom(
                        cmd::ROOF_JOIN,
                        "Join Roof Planes",
                        "roof_plane",
                        true,
                    ));
                    v.push(custom(
                        cmd::ROOF_REBUILD,
                        "Rebuild Roofs",
                        "roof_build",
                        true,
                    ));
                    for (c, label) in roof_view::PLANE_COMMANDS {
                        v.push(custom(c, label, "roof_plane", true));
                    }
                }
            }
            ObjectRef::Symbol(_) | ObjectRef::Cabinet(_) => {
                v.push(custom(
                    cmd::SYMBOL_REPLACE,
                    "Replace From Library",
                    "",
                    true,
                ));
            }
            ObjectRef::Device(id) => {
                let free = site_view::electrical_layer(self.floor, self.floor())
                    .device(id)
                    .is_some_and(|d| d.wall_id.is_none());
                v.push(custom(
                    cmd::SYMBOL_REPLACE,
                    "Replace From Library",
                    "",
                    true,
                ));
                v.push(custom(cmd::DEVICE_FLIP, "Flip Side", "", true));
                v.push(custom(cmd::DEVICE_ROTATE, "Rotate", "", free));
                // Set as Default and the outlet type switches (E-29).
                let kind = site_view::electrical_layer(self.floor, self.floor())
                    .device(id)
                    .map(|d| d.kind);
                let ganged = site_view::electrical_layer(self.floor, self.floor())
                    .gang_members(id)
                    .len()
                    > 1;
                if ganged {
                    v.push(custom(
                        crate::tools::electrical::cmd::EXPLODE_GANG,
                        "Explode Ganged Electrical Block",
                        "",
                        true,
                    ));
                }
                match kind {
                    Some(plan_electrical::DeviceKind::Outlet110) => v.push(custom(
                        crate::tools::electrical::cmd::TO_GFCI,
                        "Change to GFCI Outlet",
                        "",
                        true,
                    )),
                    Some(plan_electrical::DeviceKind::Gfci) => v.push(custom(
                        crate::tools::electrical::cmd::TO_110,
                        "Change to 110V Outlet",
                        "",
                        true,
                    )),
                    _ => {}
                }
                v.push(custom(
                    crate::tools::electrical::cmd::SET_DEFAULT,
                    "Set as Default",
                    "",
                    true,
                ));
            }
            _ => {}
        }
        v
    }

    /// Runs a [`EditActionKind::Custom`] command on the selection.
    pub fn run_custom(&mut self, id: &str) {
        // One command is one undo step, whatever families it touches.
        self.undo_group(|cx| cx.run_custom_ungrouped(id));
    }

    fn run_custom_ungrouped(&mut self, id: &str) {
        // Clipboard, selection, transform and the other `edit.*` commands.
        if id.starts_with("edit.") {
            self.run_edit_command(id);
            return;
        }
        if super::camera_edit::run_command(self, id)
            || super::stairs_view::staircase::run_command(self, id)
        {
            return;
        }
        // Tools > Materials List and the Calculate Materials buttons.
        if crate::dialogs::materials_list::run_command(self, id) {
            return;
        }
        // Construction lines and the Reference Display's commands.
        if crate::dialogs::construction_line::run_command(self, id)
            || crate::dialogs::reference_display::run_command(self, id)
            || crate::dialogs::wall_types::run_command(self, id)
        {
            return;
        }
        if let Some(c) = stair_command(id) {
            stairs_view::run_command(self, c);
            return;
        }
        if super::wall_edit::run_command(self, id) {
            return;
        }
        // Framing Member Defaults, Framing Types, Automatic and Manual Framing
        // Defaults, Structural Member Reporting (brief 29).
        if crate::dialogs::framing_defaults::run_command(self, id) {
            return;
        }
        if super::framing_view::run_command(self, id) {
            return;
        }
        if super::rooms_edit::run_command(self, id) {
            return;
        }
        // Polyline Union/Subtract/Intersect, Trim/Extend to Boundary, Insert
        // Point, Multiple Copy, Drawing Group, outer-face Plan Footprint.
        if crate::tools::cad_ops::run_command(self, id) {
            return;
        }
        // Architectural blocks, 3D solid Booleans, material layers, distribution options, soffits.
        if crate::tools::arch_block::run_command(self, id) {
            return;
        }
        if crate::dialogs::object_layers::run_command(self, id) {
            return;
        }
        // Match Properties and Object Painter Modes.
        if crate::tools::painters::run_command(self, id)
            || crate::dialogs::spell_check::run_command(self, id)
        {
            return;
        }
        // Fireplace Specification, Build Deck Framing, Add Steps (CB-86, CB-87, R-86).
        if super::fireplace_view::run_command(self, id) {
            return;
        }
        if super::cabinet_edit::run_command(self, id) {
            return;
        }
        if crate::tools::text::run_command(self, id) {
            return;
        }
        if super::opening_edit::run_command(self, id) {
            return;
        }
        if super::schedule_view::run_command(self, id) {
            return;
        }
        if roof_view::run_wall_command(self, id) {
            return;
        }
        if roof_view::run_plane_command(self, id) {
            return;
        }
        match id {
            cmd::ROOF_JOIN => self
                .requests
                .push(EditorRequest::SetTool(ToolId::RoofVariant(RoofMode::Join))),
            cmd::ROOF_REBUILD => self.rebuild_roofs(),
            cmd::ROOF_DELETE_ALL => self.delete_all_roof_planes(),
            cmd::ROOF_DELETE_CEILINGS => self.delete_all_ceiling_planes(),
            cmd::ROOF_EXPLODE_DORMER => self.explode_selected_dormer(),
            cmd::SYMBOL_REPLACE => self.replace_symbol_from_library(),
            _ if crate::editor::placed::run_command(self, id) => {}
            _ if crate::tools::cabinet::run_preset_command(self, id) => {}
            // Set as Default, Reset Curvature, Change to GFCI / 110V (Electrical Tools).
            _ if crate::tools::electrical::run_command(self, id) => {}
            cmd::DEVICE_FLIP => self.edit_selected_device("Flip Side", |d, wall| {
                site_view::flip_side(d, wall);
            }),
            cmd::DEVICE_ROTATE => self.edit_selected_device("Rotate Device", |d, _| {
                if d.wall_id.is_none() {
                    d.angle = (d.angle + FRAC_PI_2).rem_euclid(TAU);
                }
            }),
            _ => {}
        }
    }

    fn delete_all_roof_planes(&mut self) {
        self.begin_change("Delete Roof Planes");
        let mut n = 0;
        for fi in 0..self.project.floors.len() {
            n += roof_view::delete_all(&mut self.project, fi);
        }
        if n == 0 {
            self.cancel_change();
            self.status = "There are no roof planes to delete".into();
            return;
        }
        self.selection
            .items
            .retain(|o| !matches!(o, ObjectRef::RoofPlane(_)));
        self.mark_dirty();
        self.status = format!("Deleted {n} roof plane{}", if n == 1 { "" } else { "s" });
    }

    fn delete_all_ceiling_planes(&mut self) {
        self.begin_change("Delete Ceiling Planes");
        let mut n = 0;
        for fi in 0..self.project.floors.len() {
            n += roof_view::delete_ceilings(&mut self.project, fi);
        }
        if n == 0 {
            self.cancel_change();
            self.status = "There are no ceiling planes to delete".into();
            return;
        }
        self.selection.retain_existing(&self.project, self.floor);
        self.mark_dirty();
        self.status = format!("Deleted {n} ceiling plane{}", if n == 1 { "" } else { "s" });
    }

    fn explode_selected_dormer(&mut self) {
        let Some(ObjectRef::RoofPlane(id)) = self.selection.single() else {
            return;
        };
        self.begin_change("Explode Dormer");
        let fl = self.floor;
        match roof_view::explode_dormer_record(&mut self.project, fl, id, &self.defaults) {
            Ok(done) => {
                self.selection.clear();
                self.mark_dirty();
                self.status = format!(
                    "Exploded the dormer into {} roof planes and {} walls",
                    done.planes, done.walls
                );
            }
            Err(e) => {
                self.cancel_change();
                self.status = format!("Explode Dormer: {e}");
            }
        }
    }

    fn rebuild_roofs(&mut self) {
        let settings = self
            .project
            .floors
            .iter()
            .find_map(|f| roof_view::load(f).settings);
        let Some(s) = settings else {
            self.status = "No roof has been built yet: use Build Roof".into();
            return;
        };
        let fi = roof_view::build_floor(&self.project, s.ignore_top_floor, self.floor);
        self.begin_change("Rebuild Roofs");
        match roof_view::rebuild(&mut self.project, fi, s, false) {
            Ok(rep) => {
                self.mark_dirty();
                self.status = format!("Rebuilt the roof: {} planes", rep.planes);
            }
            Err(e) => {
                self.cancel_change();
                self.status = format!("Rebuild Roofs: {e}");
            }
        }
    }

    /// Replace From Library (CB-57): every selected symbol takes the catalog
    /// item picked in the Library Browser, in one undo step (cabinets and
    /// devices one at a time; see `tools::library::convert`).
    fn replace_symbol_from_library(&mut self) {
        crate::tools::library::convert::replace_selected(self);
    }

    fn edit_selected_device(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut plan_electrical::Device, Option<&plan_core::Wall>),
    ) {
        let Some(ObjectRef::Device(id)) = self.selection.single() else {
            return;
        };
        site_view::edit_electrical(self, label, |layer, floor| {
            if let Some(d) = layer.device_mut(id) {
                let wall = d.wall_id.and_then(|w| floor.wall(w)).cloned();
                edit(d, wall.as_ref());
            }
        });
    }

    /// Translates the selected stairs, cabinets, symbols, devices, roof
    /// records, foundation objects, details, terrain elements and cameras by
    /// `d` (group drags and nudges).
    pub fn translate_extra(&mut self, items: &[ObjectRef], d: plan_core::geometry::Point) {
        let fl = self.floor;
        for o in items {
            match *o {
                ObjectRef::Stair(id) => {
                    stairs_view::update(&mut self.project, fl, id, |s| {
                        s.stair.origin = s.stair.origin + d;
                    });
                }
                ObjectRef::Cabinet(id) => {
                    if let Some(mut c) = placed::cabinet_by_id(self.floor(), id) {
                        c.position = c.position + d;
                        placed::replace_cabinet(&mut self.project, fl, &c);
                    }
                }
                ObjectRef::Symbol(id) => {
                    if let Some(s) = self.project.floors[fl]
                        .symbols
                        .iter_mut()
                        .find(|s| s.id == id)
                    {
                        s.position = s.position + d;
                    }
                }
                ObjectRef::Device(id) => {
                    let mut layer = site_view::load_electrical(self.floor());
                    if let Some(dev) = layer.device_mut(id) {
                        dev.position = dev.position + d;
                        site_view::save_electrical(&mut self.project, fl, &layer);
                    }
                }
                ObjectRef::RoofPlane(id) => {
                    let mut set = roof_view::load(&self.project.floors[fl]);
                    if roof_view::translate_record(&mut set, id, d) {
                        roof_view::store(&mut self.project, fl, &mut set);
                    }
                }
                ObjectRef::Camera(id) => {
                    self.project
                        .update_camera(id, |c| c.position = c.position + d);
                }
                _ => {}
            }
        }
        let foundation: Vec<Id> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Foundation(id) => Some(*id),
                _ => None,
            })
            .collect();
        foundation_view::translate_ids(self, &foundation, d);
        let framing: Vec<Id> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Framing(id) => Some(*id),
                _ => None,
            })
            .collect();
        if !framing.is_empty() {
            framing_view::translate_in(&mut self.project.floors[fl], &framing, d);
        }
        let details: Vec<Id> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Detail(id) => Some(*id),
                _ => None,
            })
            .collect();
        details_view::translate_ids(self, &details, d);
        let compounds: Vec<Id> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Solid(id) => Some(*id),
                _ => None,
            })
            .collect();
        super::solids_view::translate_ids(self, &compounds, d);
        let schedules: Vec<Id> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Schedule(id) => Some(*id),
                _ => None,
            })
            .collect();
        schedule_view::translate_ids(self, &schedules, d);
        let terrain: Vec<site_view::TerrainHit> = items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::TerrainObject(h) => Some(*h),
                _ => None,
            })
            .collect();
        if !terrain.is_empty() {
            if let Some(mut rec) = site_view::load_terrain(&self.project) {
                if site_view::move_terrain_elements(&mut rec.terrain, &terrain, d) > 0 {
                    site_view::save_terrain(&mut self.project, &rec);
                }
            }
        }
        self.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::{CameraKind, CameraObject};

    #[test]
    fn copy_paste_carries_cad_attrs_and_blocks_and_delete_drops_them() {
        use plan_core::cad::{CadItem, DEFAULT_CAD_LAYER};
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let ids: Vec<Id> = (0..3)
            .map(|i| {
                cx.project.add_cad(
                    0,
                    DEFAULT_CAD_LAYER,
                    CadItem::Line {
                        a: Point::new(f64::from(i) * 10.0, 0.0),
                        b: Point::new(f64::from(i) * 10.0, 10.0),
                    },
                )
            })
            .collect();
        cx.project
            .edit_cad_attrs(0, ids[0], |a| a.weight = Some(70));
        let g = cx.project.make_cad_block(0, &ids, Some("Bench")).unwrap();
        cx.selection.items = ids.iter().map(|i| ObjectRef::Cad(*i)).collect();
        cx.copy_selection();
        cx.paste_in_place();

        let pasted: Vec<Id> = cx
            .selection
            .items
            .iter()
            .filter_map(|o| match o {
                ObjectRef::Cad(id) => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(pasted.len(), 3);
        assert!(pasted.iter().all(|id| !ids.contains(id)));
        assert_eq!(cx.floor().cad_attrs(pasted[0]).unwrap().weight, Some(70));
        assert!(cx.floor().cad_attrs(pasted[1]).is_none());
        let blocks = cx.floor().cad_blocks();
        assert_eq!(blocks.len(), 2, "the copy is a block of its own");
        let copy = blocks.iter().find(|b| b.group != g).unwrap();
        assert_eq!(copy.name, "Bench");
        assert_eq!(cx.floor().group_members_cad(copy.group), pasted);

        // Deleting the copy takes its attributes and its block with it.
        cx.delete_selection();
        assert!(cx.floor().cad_attrs(pasted[0]).is_none());
        assert_eq!(cx.floor().cad_attrs.len(), 1);
        assert_eq!(cx.floor().cad_blocks().len(), 1);
        assert_eq!(cx.floor().cad_blocks[0].group, g);
        // Undo restores them with the objects.
        cx.undo();
        assert_eq!(cx.floor().cad_attrs.len(), 2);
        assert_eq!(cx.floor().cad_blocks().len(), 2);
    }

    #[test]
    fn deleting_a_camera_and_a_roof_plane_through_the_selection() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let id = cx.project.add_camera(CameraObject::new(
            CameraKind::FullCamera,
            Point::new(10.0, 10.0),
            90.0,
            "Camera",
            0,
        ));

        cx.selection.set(ObjectRef::Camera(id));
        cx.refresh();
        assert_eq!(cx.selection.len(), 1);
        cx.delete_selection();
        assert!(cx.project.camera(id).is_none());
        assert!(cx.selection.is_empty());
    }
}
