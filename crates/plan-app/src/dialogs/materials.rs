//! Tools > Materials List: the take-off by category with waste, stock
//! lengths and prices (RF-60, CB-41, L-33..L-37), and the Master List behind
//! it (`~/.plan-studio/master-list.json`).
//!
//! The window lists the active floor or every floor, narrowed to one
//! category, with Chief's columns (ID, size, description, count, unit, unit
//! price, price). Its Master List tab edits the waste per category, the stock
//! lengths and the unit prices of the rows; Save writes the file. From the
//! window the list goes to CSV, to a PDF sheet, or to the layout as a table
//! box. The construction set carries it as a page.

use crate::editor::EditorContext;
use crate::shell::layout_window as lw;
use eframe::egui::{self, Ui};
use plan_docs::{
    fmt_money, materials_report, materials_to_csv, materials_total, price_keys, MasterList,
    MaterialLine, MaterialsScope, MATERIAL_CATEGORIES, MATERIAL_COLUMNS,
};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    List,
    Master,
}

/// What the Materials List window remembers.
struct State {
    all_floors: bool,
    category: Option<String>,
    tab: Tab,
    master: Option<MasterList>,
    stock_text: String,
    status: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            all_floors: false,
            category: None,
            tab: Tab::List,
            master: None,
            stock_text: String::new(),
            status: String::new(),
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The lines of the active floor (or every floor), priced from the saved
/// master list.
pub fn lines(cx: &EditorContext, all_floors: bool, master: &MasterList) -> Vec<MaterialLine> {
    let scope = if all_floors {
        MaterialsScope::AllFloors
    } else {
        MaterialsScope::Floor(cx.floor)
    };
    materials_report(&cx.project, scope, Some((cx.floor, &cx.rooms)), master)
}

/// The active floor's materials list with the saved master list's waste and prices.
#[cfg_attr(not(test), allow(dead_code))]
pub fn lines_for_floor(cx: &EditorContext) -> Vec<MaterialLine> {
    lines(cx, false, &lw::load_master_list())
}

/// CSV of the active floor's list.
#[cfg_attr(not(test), allow(dead_code))]
pub fn csv_for_floor(cx: &EditorContext) -> String {
    materials_to_csv(&lines_for_floor(cx))
}

fn stock_text(list: &MasterList) -> String {
    list.stock_lengths_ft
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Stock lengths from `8, 10, 12`; empty and bad entries are dropped.
pub fn parse_stock(text: &str) -> Vec<u32> {
    let mut v: Vec<u32> = text
        .split([',', ' ', ';'])
        .filter_map(|t| t.trim().parse::<u32>().ok())
        .filter(|l| (1..=60).contains(l))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Asks for a file name and writes `bytes` there (the Excel workbook).
fn save_bytes(name: &str, ext: &str, bytes: &[u8]) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save: {e}"),
    }
}

fn save_text(name: &str, ext: &str, text: &str) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, text) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save: {e}"),
    }
}

/// Draws the window; false once it is closed.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) -> bool {
    let mut open = true;
    STATE.with(|st| {
        let mut st = st.borrow_mut();
        if st.master.is_none() {
            let m = lw::load_master_list();
            st.stock_text = stock_text(&m);
            st.master = Some(m);
        }
        egui::Window::new("Materials List")
            .id(egui::Id::new("materials_list"))
            .open(&mut open)
            .default_pos(ctx.screen_rect().center())
            .default_width(760.0)
            .show(ctx, |ui| contents(ui, cx, &mut st));
    });
    if !open {
        // Reload from the file next time so unsaved edits are dropped.
        STATE.with(|st| st.borrow_mut().master = None);
    }
    open
}

fn contents(ui: &mut Ui, cx: &mut EditorContext, st: &mut State) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut st.tab, Tab::List, "Materials List");
        ui.selectable_value(&mut st.tab, Tab::Master, "Master List");
        ui.separator();
        ui.radio_value(&mut st.all_floors, false, "Active floor");
        ui.radio_value(&mut st.all_floors, true, "All floors");
    });
    let master = st.master.clone().unwrap_or_default();
    let all = lines(cx, st.all_floors, &master);
    ui.separator();
    match st.tab {
        Tab::List => list_tab(ui, cx, st, &all),
        Tab::Master => master_tab(ui, st, &all),
    }
    if !st.status.is_empty() {
        ui.separator();
        ui.label(&st.status);
    }
}

fn list_tab(ui: &mut Ui, cx: &mut EditorContext, st: &mut State, all: &[MaterialLine]) {
    ui.horizontal(|ui| {
        ui.label("Category");
        egui::ComboBox::from_id_salt("materials_category")
            .selected_text(
                st.category
                    .clone()
                    .unwrap_or_else(|| "All categories".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut st.category, None, "All categories");
                for c in MATERIAL_CATEGORIES {
                    ui.selectable_value(&mut st.category, Some(c.to_string()), c);
                }
            });
    });
    let shown: Vec<MaterialLine> = all
        .iter()
        .filter(|l| st.category.as_ref().is_none_or(|c| &l.category == c))
        .cloned()
        .collect();
    egui::ScrollArea::both().max_height(380.0).show(ui, |ui| {
        egui::Grid::new("materials_grid")
            .striped(true)
            .num_columns(MATERIAL_COLUMNS.len())
            .show(ui, |ui| {
                for c in MATERIAL_COLUMNS {
                    ui.strong(c);
                }
                ui.end_row();
                for l in &shown {
                    for (i, cell) in plan_docs::materials_cells(l).into_iter().enumerate() {
                        if i >= 4 {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| ui.label(cell),
                            );
                        } else {
                            ui.label(cell);
                        }
                    }
                    ui.end_row();
                }
            });
    });
    ui.separator();
    let total = materials_total(&shown);
    ui.horizontal(|ui| {
        ui.label(format!(
            "{} lines for {}",
            shown.len(),
            if st.all_floors {
                "all floors".to_string()
            } else {
                cx.floor().name.clone()
            }
        ));
        if shown.iter().any(|l| l.price.is_some()) {
            ui.strong(format!("Total {}", fmt_money(Some(total))));
        } else {
            ui.weak("No prices yet: enter them on the Master List tab");
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Export CSV\u{2026}").clicked() {
            st.status = save_text("materials_list.csv", "csv", &materials_to_csv(&shown));
        }
        if ui.button("Export Excel\u{2026}").clicked() {
            st.status = save_bytes(
                "materials_list.xlsx",
                "xlsx",
                &plan_docs::materials_to_xlsx(&shown),
            );
        }
        if ui.button("Export PDF\u{2026}").clicked() {
            st.status = match lw::materials_pdf(&cx.project) {
                Some(bytes) => crate::dialogs::print::deliver(
                    &bytes,
                    crate::dialogs::print::Destination::Pdf,
                    1,
                    "materials_list",
                ),
                None => "The plan has no materials yet".into(),
            };
        }
        if ui
            .button("Send to Layout")
            .on_hover_text("Add this list to the layout as a table box")
            .clicked()
        {
            let floor = (!st.all_floors).then_some(cx.floor);
            st.status = lw::send_materials(cx, floor, st.category.clone());
        }
    });
}

fn master_tab(ui: &mut Ui, st: &mut State, all: &[MaterialLine]) {
    let Some(master) = st.master.as_mut() else {
        return;
    };
    ui.label(
        "The Master List holds the waste factor of each category, the lumber stock \
         lengths and the unit price of each item. Prices left at 0 are not priced.",
    );
    ui.add_space(4.0);
    egui::CollapsingHeader::new("Waste factors")
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("master_waste")
                .num_columns(2)
                .show(ui, |ui| {
                    for c in MATERIAL_CATEGORIES {
                        ui.label(c);
                        let mut w = master.waste_for(c);
                        if ui
                            .add(egui::DragValue::new(&mut w).range(0.0..=100.0).suffix(" %"))
                            .changed()
                        {
                            if w > 0.0 {
                                master.waste.insert(c.to_string(), w);
                            } else {
                                master.waste.remove(c);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    ui.horizontal(|ui| {
        ui.label("Stock lengths (ft)");
        if ui.text_edit_singleline(&mut st.stock_text).lost_focus() {
            let v = parse_stock(&st.stock_text);
            if !v.is_empty() {
                master.stock_lengths_ft = v;
            }
            st.stock_text = stock_text(master);
        }
    });
    egui::CollapsingHeader::new("Unit prices")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("master_prices")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Item");
                            ui.strong("Unit");
                            ui.strong("Unit price");
                            ui.strong("Supplier");
                            ui.end_row();
                            for (key, unit) in price_keys(all) {
                                ui.label(key.replace('|', ": "));
                                ui.label(&unit);
                                let mut price = master.item(&key).map_or(0.0, |i| i.unit_price);
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut price)
                                            .range(0.0..=1.0e7)
                                            .speed(0.05)
                                            .prefix("$")
                                            .max_decimals(2),
                                    )
                                    .changed()
                                {
                                    master.set_price(&key, &unit, price);
                                }
                                let mut supplier = master
                                    .item(&key)
                                    .map(|i| i.supplier.clone())
                                    .unwrap_or_default();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut supplier)
                                            .desired_width(120.0),
                                    )
                                    .changed()
                                {
                                    master.set_price(&key, &unit, price);
                                    if let Some(i) = master.items.iter_mut().find(|i| i.key == key)
                                    {
                                        i.supplier = supplier;
                                    }
                                }
                                ui.end_row();
                            }
                        });
                });
        });
    ui.horizontal(|ui| {
        if ui.button("Save Master List").clicked() {
            st.status = match lw::save_master_list(master) {
                Ok(()) => "Saved the Master List".into(),
                Err(e) => e,
            };
        }
        if ui.button("Reset to defaults").clicked() {
            *master = MasterList::default();
            st.stock_text = stock_text(master);
            st.status = "Defaults restored; Save to keep them".into();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{OpeningKind, Point, WallKind};

    fn cx_with_house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut ids = Vec::new();
        let c = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 360.0),
            Point::new(0.0, 360.0),
        ];
        for i in 0..4 {
            ids.push(cx.project.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.5,
                109.125,
                WallKind::Exterior,
            ));
        }
        cx.project
            .add_opening(0, ids[0], 240.0, OpeningKind::Door)
            .unwrap();
        cx
    }

    #[test]
    fn stock_lengths_parse_sort_and_dedupe() {
        assert_eq!(parse_stock("16, 8 10;12  8 x 99"), vec![8, 10, 12, 16]);
        assert!(parse_stock("none").is_empty());
    }

    #[test]
    fn lines_follow_the_master_list_and_scope() {
        lw::use_memory_master_list(MasterList::default());
        let mut cx = cx_with_house();
        cx.rooms = plan_core::detect_rooms(&cx.project.floors[0].walls, 1.0);
        let none = lines_for_floor(&cx);
        assert!(none.iter().all(|l| l.price.is_none()));
        let mut m = MasterList::default();
        m.set_price("Doors|Door 3'-0\" x 6'-8\"", "ea", 120.0);
        lw::save_master_list(&m).unwrap();
        let priced = lines_for_floor(&cx);
        let door = priced.iter().find(|l| l.category == "Doors").unwrap();
        assert_eq!(door.price, Some(120.0));
        assert!(csv_for_floor(&cx).contains("$120.00"));
        assert!(csv_for_floor(&cx).starts_with("Category,ID,Description"));
        assert_eq!(lines(&cx, true, &m).len(), priced.len());
    }

    #[test]
    fn the_window_draws_both_tabs_headless() {
        lw::use_memory_master_list(MasterList::default());
        let mut cx = cx_with_house();
        let ctx = egui::Context::default();
        for tab in [Tab::List, Tab::Master, Tab::List] {
            STATE.with(|s| s.borrow_mut().tab = tab);
            for _ in 0..2 {
                let mut open = true;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    open = show(ctx, &mut cx);
                });
                assert!(open);
            }
        }
        STATE.with(|s| *s.borrow_mut() = State::default());
    }
    #[test]
    fn roofing_squares_for_the_forty_by_thirty_house_and_send_to_layout() {
        lw::use_memory_master_list(MasterList::default());
        let mut cx = cx_with_house();
        // A 6:12 gable over the 40' x 30' plan: 480 x 360 in plan, two planes.
        let plane = |pts: [[f64; 3]; 4], base: [[f64; 2]; 2]| {
            serde_json::json!({"kind": "plane", "id": 1, "pitch": 6.0,
                "polygon3d": pts, "baseline": base})
        };
        cx.project.floors[0].roofs = vec![
            plane(
                [
                    [0.0, 0.0, 0.0],
                    [480.0, 0.0, 0.0],
                    [480.0, 90.0, -180.0],
                    [0.0, 90.0, -180.0],
                ],
                [[0.0, 0.0], [480.0, 0.0]],
            ),
            plane(
                [
                    [480.0, 0.0, -360.0],
                    [0.0, 0.0, -360.0],
                    [0.0, 90.0, -180.0],
                    [480.0, 90.0, -180.0],
                ],
                [[480.0, 360.0], [0.0, 360.0]],
            ),
        ];
        let net = lines(&cx, false, &MasterList::without_waste());
        let squares = net
            .iter()
            .find(|l| l.item.starts_with("Roofing (100"))
            .unwrap();
        assert!(
            (squares.quantity - 13.42).abs() < 1e-9,
            "{}",
            squares.quantity
        );
        let wasted = lines_for_floor(&cx);
        let squares = wasted
            .iter()
            .find(|l| l.item.starts_with("Roofing (100"))
            .unwrap();
        assert!(
            (squares.quantity - 14.77).abs() < 1e-9,
            "{}",
            squares.quantity
        );
        assert_eq!(squares.waste_pct, 10.0);
        // Send to Layout makes the layout and a Materials List box on page 1.
        let status = lw::send_materials(&mut cx, None, Some("Roofing".into()));
        assert_eq!(status, "Sent the Materials List to the layout");
        let layout = lw::load(&cx.project).unwrap();
        assert!(layout.pages.iter().flat_map(|p| &p.boxes).any(|b| matches!(
            &b.source,
            plan_layout::BoxSource::Materials { category: Some(c), floor: None } if c == "Roofing"
        )));
        // The PDF sheet of the list carries the roofing table.
        let pdf = lw::materials_pdf(&cx.project).unwrap();
        let t: String = pdf.iter().map(|&b| b as char).collect();
        assert!(t.contains("(MATERIALS LIST - ROOFING)"));
    }
}
