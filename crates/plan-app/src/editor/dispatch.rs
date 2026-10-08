//! Routes the shared edit commands (Delete, Copy, Paste in Place, Reverse
//! Swing and the module-specific Edit toolbar commands) to the module that
//! owns each object kind: `stairs_view`, `placed`, `roof_view`, `site_view`,
//! `foundation_view` and the camera tool.

use super::actions::{EditAction, EditActionKind};
use super::selection::ObjectRef;
use super::{
    foundation_view, placed, roof_view, site_view, stairs_view, EditorContext, EditorRequest,
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
    use stairs_view::StairCommand as C;
    match c {
        C::AutoStairwell => cmd::STAIR_WELL,
        C::FlareCurve => cmd::STAIR_FLARE,
        C::ToggleBreakLine => cmd::STAIR_BREAK,
        C::MakeRailing => cmd::STAIR_RAILING,
    }
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
    /// foundation objects and cameras (each kind is one undo step). Returns
    /// how many went.
    pub(super) fn delete_extra(&mut self) -> usize {
        let mut n = 0;
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

    /// Copies the selected cabinets and symbols. Returns how many.
    pub(super) fn copy_extra(&mut self) -> usize {
        if self
            .selection
            .items
            .iter()
            .any(|o| matches!(o, ObjectRef::Cabinet(_) | ObjectRef::Symbol(_)))
        {
            placed::copy_placed(self)
        } else {
            0
        }
    }

    /// Pastes the placed-object clipboard. Returns how many.
    pub(super) fn paste_extra(&mut self) -> usize {
        placed::paste_placed(self)
    }

    /// Does the Paste in Place button have anything to paste?
    pub(super) fn has_clipboard(&self) -> bool {
        self.clipboard.as_ref().is_some_and(|c| !c.is_empty()) || placed::clipboard_len() > 0
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
                }
            }
            ObjectRef::Symbol(_) => {
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
                v.push(custom(cmd::DEVICE_FLIP, "Flip Side", "", true));
                v.push(custom(cmd::DEVICE_ROTATE, "Rotate", "", free));
            }
            _ => {}
        }
        v
    }

    /// Runs a [`EditActionKind::Custom`] command on the selection.
    pub fn run_custom(&mut self, id: &str) {
        if let Some(c) = stair_command(id) {
            stairs_view::run_command(self, c);
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
        match roof_view::explode_dormer_record(&mut self.project, fl, id) {
            Ok(n) => {
                self.selection.clear();
                self.mark_dirty();
                self.status = format!("Exploded the dormer into {n} roof planes");
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

    /// Replace From Library (CB-57): the symbol takes the catalog item
    /// picked in the Library Browser.
    fn replace_symbol_from_library(&mut self) {
        let Some(ObjectRef::Symbol(id)) = self.selection.single() else {
            return;
        };
        let Some(item) = crate::tools::library::active_item() else {
            self.status =
                "Pick an item in the Library Browser, then choose Replace From Library".into();
            return;
        };
        let Some(mut sym) = self.floor().symbol(id).cloned() else {
            return;
        };
        if sym.catalog_id == item {
            self.status = "The symbol already is that library item".into();
            return;
        }
        sym.catalog_id = item;
        placed::apply_symbol(self, &sym);
        self.status = "Replaced the symbol from the library".into();
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
    /// records, foundation objects and cameras by `d` (group drags and
    /// nudges).
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
