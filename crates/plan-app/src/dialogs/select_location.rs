//! Select Location (manual p. 93): the dialog Find Schedule(s) from Object
//! opens when an object is listed in more than one schedule. It lists the
//! places an object can be found, each with its count; OK goes to the chosen
//! one (its floor, the schedule selected).
//!
//! The list is general: a [`Location`] is a floor view, a schedule or a
//! default, whatever the caller found. The dialog keeps its request in a
//! thread local, like the other plan windows, so any command can raise it and
//! [`show`] draws it once a frame.

use crate::editor::schedule_view::{self, Listing};
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::Id;
use std::cell::RefCell;

/// Where a location leads.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A schedule (selects it on its floor); the row holds the object.
    Schedule {
        floor: usize,
        id: Id,
        row: Option<usize>,
    },
    /// A floor view of the plan.
    Floor(usize),
    /// The objects on some layers on a floor (Find Objects on Layer(s)):
    /// goes to the floor and selects them.
    LayerObjects { floor: usize, layers: Vec<String> },
}

/// One place in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub title: String,
    /// What the place is (`Schedule`, `View`, `Default`).
    pub what: &'static str,
    /// How many entries it holds (rows of a schedule).
    pub count: usize,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq)]
struct Request {
    title: String,
    items: Vec<Location>,
    chosen: usize,
}

thread_local! {
    static REQUEST: RefCell<Option<Request>> = const { RefCell::new(None) };
}

/// The locations of the schedules in `list`, with the floor name in the title.
pub fn locations(cx: &EditorContext, list: &[Listing]) -> Vec<Location> {
    list.iter()
        .map(|l| Location {
            title: format!(
                "{} ({})",
                l.title,
                cx.project
                    .floors
                    .get(l.floor)
                    .map_or("?", |f| f.name.as_str())
            ),
            what: "Schedule",
            count: l.rows,
            target: Target::Schedule {
                floor: l.floor,
                id: l.id,
                row: l.row,
            },
        })
        .collect()
}

/// Opens the dialog on the schedules in `list`.
pub fn ask(list: Vec<Listing>) {
    let items = list
        .into_iter()
        .map(|l| Location {
            title: l.title.clone(),
            what: "Schedule",
            count: l.rows,
            target: Target::Schedule {
                floor: l.floor,
                id: l.id,
                row: l.row,
            },
        })
        .collect();
    ask_locations("Select Location", items);
}

/// Opens the dialog on any list of locations.
pub fn ask_locations(title: &str, items: Vec<Location>) {
    REQUEST.with(|r| {
        *r.borrow_mut() = Some(Request {
            title: title.to_string(),
            items,
            chosen: 0,
        })
    });
}

/// Is the dialog up?
pub fn is_open() -> bool {
    REQUEST.with(|r| r.borrow().is_some())
}

/// Goes to `target`.
pub fn go(cx: &mut EditorContext, target: &Target) {
    match target {
        Target::Schedule { floor, id, row } => schedule_view::show_schedule(cx, *floor, *id, *row),
        Target::Floor(f) => {
            if *f < cx.project.floors.len() && *f != cx.floor {
                cx.floor = *f;
                cx.reset_view_state();
                cx.refresh();
            }
        }
        Target::LayerObjects { floor, layers } => {
            crate::dialogs::object_layers::go_to(cx, *floor, layers);
        }
    }
}

/// Draws the dialog when it is up and carries out the choice.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut req) = REQUEST.with(|r| r.borrow_mut().take()) else {
        return;
    };
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new(req.title.clone())
        .id(egui::Id::new("select_location"))
        .collapsible(false)
        .resizable(true)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label("Go to:");
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    egui::Grid::new("select_location_grid")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("");
                            ui.strong("Location");
                            ui.strong("Count");
                            ui.end_row();
                            for (i, it) in req.items.iter().enumerate() {
                                if ui.radio(req.chosen == i, "").clicked() {
                                    req.chosen = i;
                                }
                                let r = ui.selectable_label(
                                    req.chosen == i,
                                    format!("{}: {}", it.what, it.title),
                                );
                                if r.clicked() {
                                    req.chosen = i;
                                }
                                if r.double_clicked() {
                                    req.chosen = i;
                                    accept = true;
                                }
                                ui.label(it.count.to_string());
                                ui.end_row();
                            }
                        });
                });
            ui.separator();
            ui.horizontal(|ui| {
                accept |= ui.button("OK").clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
    if accept {
        if let Some(it) = req.items.get(req.chosen).cloned() {
            go(cx, &it.target);
        }
    } else if !cancel {
        REQUEST.with(|r| *r.borrow_mut() = Some(req));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    #[test]
    fn the_dialog_lists_locations_and_goes_to_the_chosen_one() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let a = schedule_view::add(
            &mut cx,
            plan_core::schedules::ScheduleKind::Door,
            plan_core::geometry::Point::ZERO,
        );
        let b = schedule_view::add(
            &mut cx,
            plan_core::schedules::ScheduleKind::Window,
            plan_core::geometry::Point::new(0.0, -200.0),
        );
        let list = vec![
            Listing {
                floor: 0,
                id: a,
                title: "Door Schedule".into(),
                kind: plan_core::schedules::ScheduleKind::Door,
                row: Some(0),
                rows: 3,
            },
            Listing {
                floor: 0,
                id: b,
                title: "Window Schedule".into(),
                kind: plan_core::schedules::ScheduleKind::Window,
                row: None,
                rows: 2,
            },
        ];
        let locs = locations(&cx, &list);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0].count, 3);
        assert!(locs[1].title.contains("1st Floor"));
        ask(list);
        assert!(is_open());
        let ctx = egui::Context::default();
        // Frames pass with the dialog up; nothing is chosen yet.
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open());
        go(&mut cx, &locs[1].target);
        assert_eq!(schedule_view::selected(&cx), Some(b));
        REQUEST.with(|r| *r.borrow_mut() = None);
        assert!(!is_open());
    }
}
