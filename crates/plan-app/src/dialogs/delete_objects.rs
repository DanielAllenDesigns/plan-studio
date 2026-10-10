//! Edit > Delete Objects (S-88, Shift+Space): a list of object types with
//! checkboxes; OK deletes every object of the checked types on this floor,
//! or on every floor, as one undo step. Objects on hidden or locked layers
//! stay.

use crate::editor::selection::{all_selectable, ObjectRef};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::cad::CadItem;
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

/// Deletes every object of `categories` on the active floor (or all floors)
/// as one undo step named "Delete Objects". Returns how many went.
pub fn delete_by_category(
    cx: &mut EditorContext,
    categories: &[Category],
    all_floors: bool,
) -> usize {
    if categories.is_empty() {
        return 0;
    }
    let home = cx.floor;
    let floors: Vec<usize> = if all_floors {
        (0..cx.project.floors.len()).collect()
    } else {
        vec![home]
    };
    let mut total = 0;
    cx.as_one_step("Delete Objects", |cx| {
        for fl in floors {
            cx.floor = fl;
            let victims: Vec<ObjectRef> = all_selectable(cx)
                .into_iter()
                .filter(|o| Category::of(cx, *o).is_some_and(|c| categories.contains(&c)))
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

/// The dialog's state.
#[derive(Clone, Debug, Default)]
pub struct DeleteObjectsDialog {
    pub checked: Vec<Category>,
    pub all_floors: bool,
}

impl DeleteObjectsDialog {
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
                ui.separator();
                ui.checkbox(&mut self.all_floors, "On all floors");
                ui.horizontal(|ui| {
                    ok = ui
                        .add_enabled(!self.checked.is_empty(), egui::Button::new("Delete"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if ok {
            let n = delete_by_category(cx, &self.checked, self.all_floors);
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
