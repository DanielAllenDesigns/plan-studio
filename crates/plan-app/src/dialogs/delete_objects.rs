//! Edit > Delete Objects (S-88, Shift+Space): a list of object types with
//! checkboxes; OK deletes every object of the checked types on this floor,
//! or on every floor, as one undo step. Objects on hidden or locked layers
//! stay.

use crate::editor::selection::{all_selectable, ObjectRef};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::cad::CadItem;
use plan_core::Point;
use std::cell::RefCell;

/// One row of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Walls,
    Doors,
    Windows,
    Cabinets,
    Fixtures,
    Stairs,
    Dimensions,
    CadLines,
    Text,
    Roofs,
    Electrical,
    Foundation,
    Framing,
    Details,
    Schedules,
    Cameras,
}

impl Category {
    pub const ALL: [Category; 16] = [
        Category::Walls,
        Category::Doors,
        Category::Windows,
        Category::Cabinets,
        Category::Fixtures,
        Category::Stairs,
        Category::Dimensions,
        Category::CadLines,
        Category::Text,
        Category::Roofs,
        Category::Electrical,
        Category::Foundation,
        Category::Framing,
        Category::Details,
        Category::Schedules,
        Category::Cameras,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Walls => "Walls",
            Category::Doors => "Doors",
            Category::Windows => "Windows",
            Category::Cabinets => "Cabinets",
            Category::Fixtures => "Fixtures and Symbols",
            Category::Stairs => "Stairs",
            Category::Dimensions => "Dimensions",
            Category::CadLines => "CAD Lines and Shapes",
            Category::Text => "Text",
            Category::Roofs => "Roof Planes",
            Category::Electrical => "Electrical Devices",
            Category::Foundation => "Foundation Objects",
            Category::Framing => "Framing",
            Category::Details => "Details",
            Category::Schedules => "Schedules",
            Category::Cameras => "Cameras",
        }
    }

    /// The category of an object of the active floor of `cx`.
    pub fn of(cx: &EditorContext, o: ObjectRef) -> Option<Category> {
        Some(match o {
            ObjectRef::Wall(_) => Category::Walls,
            ObjectRef::Opening(id) => {
                let op = cx.floor().openings.iter().find(|x| x.id == id)?;
                match op.kind {
                    plan_core::OpeningKind::Door => Category::Doors,
                    plan_core::OpeningKind::Window => Category::Windows,
                }
            }
            ObjectRef::Cabinet(_) => Category::Cabinets,
            ObjectRef::Symbol(_) => Category::Fixtures,
            ObjectRef::Stair(_) => Category::Stairs,
            ObjectRef::Dimension(_) => Category::Dimensions,
            ObjectRef::Cad(id) | ObjectRef::Text(id) => {
                let c = cx.floor().cad.iter().find(|c| c.id == id)?;
                if matches!(c.item, CadItem::Text { .. }) {
                    Category::Text
                } else {
                    Category::CadLines
                }
            }
            ObjectRef::RoofPlane(_) => Category::Roofs,
            ObjectRef::Device(_) => Category::Electrical,
            ObjectRef::Foundation(_) => Category::Foundation,
            ObjectRef::Framing(_) => Category::Framing,
            ObjectRef::Detail(_) | ObjectRef::Solid(_) => Category::Details,
            ObjectRef::Block(_) => Category::Fixtures,
            ObjectRef::Schedule(_) => Category::Schedules,
            ObjectRef::Camera(_) => Category::Cameras,
            ObjectRef::Room(_) | ObjectRef::Terrain | ObjectRef::TerrainObject(_) => return None,
        })
    }
}

/// How many objects of each category the active floor holds (displayed,
/// unlocked layers only), for the counts beside the checkboxes.
pub fn counts(cx: &EditorContext) -> Vec<(Category, usize)> {
    let all = all_selectable(cx);
    Category::ALL
        .iter()
        .map(|c| {
            (
                *c,
                all.iter()
                    .filter(|o| Category::of(cx, **o) == Some(*c))
                    .count(),
            )
        })
        .collect()
}

/// Where Delete Objects reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeleteScope {
    /// The active floor.
    #[default]
    Floor,
    /// Objects inside one room of the active floor (index into the detected
    /// rooms). Walls and openings are on the room's edge, shared with the
    /// next room, so this scope leaves them alone.
    Room(usize),
    /// Objects inside any room of the active floor.
    AllRooms,
    /// Every floor of the plan.
    Plan,
}

/// Deletes every object of `categories` on the active floor (or all floors)
/// as one undo step named "Delete Objects". Returns how many went.
pub fn delete_by_category(
    cx: &mut EditorContext,
    categories: &[Category],
    all_floors: bool,
) -> usize {
    let scope = if all_floors {
        DeleteScope::Plan
    } else {
        DeleteScope::Floor
    };
    delete_by_scope(cx, categories, scope)
}

/// Deletes every object of `categories` within `scope` as one undo step named
/// "Delete Objects". Returns how many went.
pub fn delete_by_scope(
    cx: &mut EditorContext,
    categories: &[Category],
    scope: DeleteScope,
) -> usize {
    if categories.is_empty() {
        return 0;
    }
    let home = cx.floor;
    let floors: Vec<usize> = if scope == DeleteScope::Plan {
        (0..cx.project.floors.len()).collect()
    } else {
        vec![home]
    };
    let room: Option<Vec<Vec<Point>>> = match scope {
        DeleteScope::Room(i) => match cx.rooms.get(i) {
            Some(r) => Some(vec![r.polygon.clone()]),
            None => return 0,
        },
        DeleteScope::AllRooms => Some(cx.rooms.iter().map(|r| r.polygon.clone()).collect()),
        _ => None,
    };
    let mut total = 0;
    cx.as_one_step("Delete Objects", |cx| {
        for fl in floors {
            cx.floor = fl;
            let victims: Vec<ObjectRef> = all_selectable(cx)
                .into_iter()
                .filter(|o| Category::of(cx, *o).is_some_and(|c| categories.contains(&c)))
                .filter(|o| match &room {
                    None => true,
                    Some(polys) => polys.iter().any(|poly| in_room(cx, *o, poly)),
                })
                .collect();
            if victims.is_empty() {
                continue;
            }
            total += victims.len();
            cx.selection.items = victims;
            cx.delete_selection();
        }
        cx.floor = home;
        cx.selection.clear();
    });
    cx.mark_dirty();
    total
}

/// Does the middle of the object lie inside the room? Walls and openings are
/// never counted (see [`DeleteScope::Room`]).
fn in_room(cx: &EditorContext, o: ObjectRef, poly: &[Point]) -> bool {
    if matches!(o, ObjectRef::Wall(_) | ObjectRef::Opening(_)) {
        return false;
    }
    let pts = crate::editor::transform::object_points(cx, o);
    if pts.is_empty() {
        return false;
    }
    let n = pts.len() as f64;
    let mid = Point::new(
        pts.iter().map(|p| p.x).sum::<f64>() / n,
        pts.iter().map(|p| p.y).sum::<f64>() / n,
    );
    plan_core::geometry::point_in_polygon(mid, poly)
}

/// The room of the active floor that holds the selection's middle (the first
/// room when nothing is selected, or `None` without rooms).
pub fn default_room(cx: &EditorContext) -> Option<usize> {
    if cx.rooms.is_empty() {
        return None;
    }
    let pts: Vec<Point> = cx
        .selection
        .items
        .iter()
        .flat_map(|o| crate::editor::transform::object_points(cx, *o))
        .collect();
    if pts.is_empty() {
        return Some(0);
    }
    let n = pts.len() as f64;
    let mid = Point::new(
        pts.iter().map(|p| p.x).sum::<f64>() / n,
        pts.iter().map(|p| p.y).sum::<f64>() / n,
    );
    Some(
        cx.rooms
            .iter()
            .position(|r| plan_core::geometry::point_in_polygon(mid, &r.polygon))
            .unwrap_or(0),
    )
}

/// The dialog's state.
#[derive(Clone, Debug, Default)]
pub struct DeleteObjectsDialog {
    pub checked: Vec<Category>,
    pub all_floors: bool,
    /// Limits the deletion to one room of this floor (Room scope).
    pub room: Option<usize>,
    /// Limits it to objects inside any room of this floor.
    pub all_rooms: bool,
}

impl DeleteObjectsDialog {
    /// The scope the controls stand at.
    pub fn scope(&self) -> DeleteScope {
        match (self.all_floors, self.room) {
            (true, _) => DeleteScope::Plan,
            (false, Some(r)) => DeleteScope::Room(r),
            (false, None) if self.all_rooms => DeleteScope::AllRooms,
            (false, None) => DeleteScope::Floor,
        }
    }

    pub fn set_scope(&mut self, s: DeleteScope) {
        self.all_rooms = s == DeleteScope::AllRooms;
        (self.all_floors, self.room) = match s {
            DeleteScope::Floor | DeleteScope::AllRooms => (false, None),
            DeleteScope::Room(r) => (false, Some(r)),
            DeleteScope::Plan => (true, None),
        };
    }

    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut ok = false;
        let mut cancel = false;
        let counts = counts(cx);
        egui::Window::new("Delete Objects")
            .id(egui::Id::new("delete_objects"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Delete every object of the checked types:");
                egui::Grid::new("delete_objects_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (i, (cat, n)) in counts.iter().enumerate() {
                            let mut on = self.checked.contains(cat);
                            if ui
                                .checkbox(&mut on, format!("{} ({n})", cat.label()))
                                .changed()
                            {
                                if on {
                                    self.checked.push(*cat);
                                } else {
                                    self.checked.retain(|c| c != cat);
                                }
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("Select All").clicked() {
                        self.checked = counts.iter().map(|(c, _)| *c).collect();
                    }
                    if ui.button("Clear All").clicked() {
                        self.checked.clear();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Scope");
                    let mut scope = self.scope();
                    ui.radio_value(&mut scope, DeleteScope::Floor, "This floor");
                    if let Some(r) = default_room(cx) {
                        let r = self.room.unwrap_or(r);
                        ui.radio_value(&mut scope, DeleteScope::Room(r), "One room");
                        if let DeleteScope::Room(_) = scope {
                            let mut i = r;
                            egui::ComboBox::from_id_salt("delete_objects_room")
                                .selected_text(
                                    cx.rooms.get(i).map_or(String::new(), |x| x.label.clone()),
                                )
                                .show_ui(ui, |ui| {
                                    for (k, room) in cx.rooms.iter().enumerate() {
                                        ui.selectable_value(&mut i, k, room.label.clone());
                                    }
                                });
                            scope = DeleteScope::Room(i);
                        }
                    }
                    ui.radio_value(&mut scope, DeleteScope::AllRooms, "All rooms");
                    ui.radio_value(&mut scope, DeleteScope::Plan, "All floors");
                    self.set_scope(scope);
                });
                ui.horizontal(|ui| {
                    ok = ui
                        .add_enabled(!self.checked.is_empty(), egui::Button::new("Delete"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if ok {
            let n = delete_by_scope(cx, &self.checked, self.scope());
            cx.status = format!("Deleted {n} object{}", if n == 1 { "" } else { "s" });
            return false;
        }
        open && !cancel
    }
}

thread_local! {
    static DIALOG: RefCell<Option<DeleteObjectsDialog>> = const { RefCell::new(None) };
}

/// Opens the dialog.
pub fn open() {
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(DeleteObjectsDialog::default());
        }
    });
}

pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            DIALOG.with(|slot| *slot.borrow_mut() = Some(d));
        }
    }
}
