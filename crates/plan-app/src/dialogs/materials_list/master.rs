//! The Master List view (manual pp. 1387-1389): the prices, suppliers,
//! manufacturers, codes, markups, labor and equipment of past take-offs, with
//! a Category drop-down, a Columns choice, a Find field, the Use, Quantity and
//! Default columns, Delete, and the waste factors and stock lengths that go
//! with it. Edits are kept until Save writes `~/.plan-studio/master-list.json`.

use super::State;
use crate::editor::EditorContext;
use crate::shell::layout_window as lw;
use eframe::egui::{self, Color32};
use plan_core::materials_data::{ColumnState, MlColumn, CATEGORIES};
use plan_docs::master_list::{default_master_columns, MasterItem};
use plan_docs::MasterList;

/// What the Master List view remembers besides the list.
#[derive(Default)]
pub struct MasterUi {
    pub category: Option<String>,
    pub selected: Option<usize>,
    pub find: String,
    pub found: Option<(usize, usize)>,
    pub stock_text: String,
    pub show_columns: bool,
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

fn columns_of(m: &MasterList) -> Vec<ColumnState> {
    if m.columns.is_empty() {
        default_master_columns()
    } else {
        let mut cols = m.columns.clone();
        for c in default_master_columns() {
            if !cols.iter().any(|x| x.col == c.col) {
                cols.push(ColumnState {
                    visible: false,
                    ..c
                });
            }
        }
        cols
    }
}

/// Edits the cell of master entry `i` in column `col` from `text`. False when
/// the text does not fit the column.
pub fn set_cell(m: &mut MasterList, i: usize, col: MlColumn, text: &str) -> bool {
    let Some(it) = m.items.get_mut(i) else {
        return false;
    };
    let num = |t: &str| -> Option<f64> {
        let t = t.trim().trim_start_matches('$').replace(',', "");
        if t.is_empty() {
            Some(0.0)
        } else {
            t.parse::<f64>().ok()
        }
    };
    match col {
        MlColumn::Supplier => it.supplier = text.to_string(),
        MlColumn::Manufacturer => it.manufacturer = text.to_string(),
        MlColumn::Code => it.code = text.to_string(),
        MlColumn::Size => it.size = text.to_string(),
        MlColumn::Description => it.description = text.to_string(),
        MlColumn::Label => it.label = text.to_string(),
        MlColumn::Comment => it.comment = text.to_string(),
        MlColumn::AccountingCode => it.accounting_code = text.to_string(),
        MlColumn::Price => match num(text) {
            Some(v) => it.unit_price = v,
            None => return false,
        },
        MlColumn::Markup => match num(text) {
            Some(v) => it.markup = v,
            None => return false,
        },
        MlColumn::Labor => match num(text) {
            Some(v) => it.labor = v,
            None => return false,
        },
        MlColumn::Equipment => match num(text) {
            Some(v) => it.equipment = v,
            None => return false,
        },
        MlColumn::Quantity => match num(text) {
            Some(v) => it.min_quantity = v,
            None => return false,
        },
        _ => return false,
    }
    true
}

/// A new, empty entry in `category`.
pub fn add_item(m: &mut MasterList, category: &str) -> usize {
    let mut it = MasterItem::blank();
    it.category = category.to_string();
    it.description = "New item".into();
    m.items.push(it);
    m.items.len() - 1
}

pub fn master_view(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State) {
    let _ = cx;
    let Some(mut m) = st.master.take() else {
        st.master = Some(lw::load_master_list());
        return;
    };
    let ui_state = &mut st.master_ui;
    if ui_state.stock_text.is_empty() {
        ui_state.stock_text = m
            .stock_lengths_ft
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
    }
    let mut changed = false;
    let cats = {
        let mut c = m.categories();
        for k in CATEGORIES {
            if !c.iter().any(|x| x == k) {
                c.push(k.to_string());
            }
        }
        c
    };
    ui.horizontal(|ui| {
        ui.label("Category");
        egui::ComboBox::from_id_salt("ml_master_cat")
            .selected_text(
                ui_state
                    .category
                    .clone()
                    .unwrap_or_else(|| "All categories".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ui_state.category, None, "All categories");
                for c in &cats {
                    ui.selectable_value(&mut ui_state.category, Some(c.clone()), c);
                }
            });
        if ui.button("Columns\u{2026}").clicked() {
            ui_state.show_columns = !ui_state.show_columns;
        }
        if ui.button("Add Item").clicked() {
            let cat = ui_state
                .category
                .clone()
                .unwrap_or_else(|| "Framing".into());
            ui_state.selected = Some(add_item(&mut m, &cat));
            changed = true;
        }
        if ui
            .add_enabled(ui_state.selected.is_some(), egui::Button::new("Delete"))
            .clicked()
        {
            if let Some(i) = ui_state.selected.take() {
                m.remove(i);
                changed = true;
            }
        }
        if ui
            .add_enabled(
                ui_state.selected.is_some(),
                egui::Button::new("Make Default"),
            )
            .clicked()
        {
            if let Some(i) = ui_state.selected {
                m.set_default(i);
                changed = true;
            }
        }
    });
    let mut cols = columns_of(&m);
    if ui_state.show_columns {
        ui.group(|ui| {
            ui.label("Columns (the Master List Specification)");
            let mut swap: Option<(usize, i32)> = None;
            for (i, c) in cols.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    let mut v = c.visible;
                    if ui.checkbox(&mut v, c.col.title()).changed() {
                        cols[i].visible = v;
                        changed = true;
                    }
                    if ui.small_button("\u{25B2}").clicked() {
                        swap = Some((i, -1));
                    }
                    if ui.small_button("\u{25BC}").clicked() {
                        swap = Some((i, 1));
                    }
                });
            }
            if let Some((i, d)) = swap {
                let j = i as i32 + d;
                if j >= 0 && (j as usize) < cols.len() {
                    cols.swap(i, j as usize);
                    changed = true;
                }
            }
        });
        if changed {
            m.columns = cols.clone();
        }
    }
    let shown_cols: Vec<MlColumn> = cols.iter().filter(|c| c.visible).map(|c| c.col).collect();
    // Find.
    ui.horizontal(|ui| {
        ui.label("Find");
        ui.add(egui::TextEdit::singleline(&mut ui_state.find).desired_width(160.0));
        if ui.button("Find Next").clicked() {
            ui_state.found = m.find_next(&shown_cols, ui_state.found, &ui_state.find.clone());
            if let Some((r, _)) = ui_state.found {
                ui_state.selected = Some(r);
            }
        }
    });
    ui.separator();
    // The table.
    let visible: Vec<usize> = (0..m.items.len())
        .filter(|i| {
            ui_state
                .category
                .as_ref()
                .is_none_or(|c| m.items[*i].category_name() == *c)
        })
        .collect();
    egui::ScrollArea::both().max_height(300.0).show(ui, |ui| {
        egui::Grid::new("ml_master_grid")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("#");
                for c in &shown_cols {
                    ui.strong(c.title());
                }
                ui.end_row();
                for &i in &visible {
                    let sel = ui_state.selected == Some(i);
                    if ui.selectable_label(sel, format!("{}", i + 1)).clicked() {
                        ui_state.selected = Some(i);
                    }
                    for (ci, c) in shown_cols.iter().enumerate() {
                        match c {
                            MlColumn::Use => {
                                let mut v = m.items[i].use_item;
                                if ui.checkbox(&mut v, "").changed() {
                                    m.items[i].use_item = v;
                                    changed = true;
                                }
                            }
                            MlColumn::Default => {
                                let mut v = m.items[i].is_default;
                                if ui.checkbox(&mut v, "").changed() {
                                    if v {
                                        m.set_default(i);
                                    } else {
                                        m.items[i].is_default = false;
                                    }
                                    changed = true;
                                }
                            }
                            MlColumn::Id => {
                                ui.label(m.items[i].category_name());
                            }
                            _ => {
                                let mut text = m.cell(i, *c);
                                let width = f32::from(match c {
                                    MlColumn::Description => 220_u16,
                                    MlColumn::Size | MlColumn::Comment => 120,
                                    _ => 80,
                                });
                                let hit = ui_state.found == Some((i, ci));
                                let mut edit =
                                    egui::TextEdit::singleline(&mut text).desired_width(width);
                                if hit {
                                    edit = edit.text_color(Color32::from_rgb(0xD0, 0x6A, 0x1C));
                                }
                                if ui.add(edit).changed() && set_cell(&mut m, i, *c, &text) {
                                    changed = true;
                                }
                            }
                        }
                    }
                    ui.end_row();
                }
            });
    });
    if m.items.is_empty() {
        ui.weak("Nothing in the Master List yet. Type a price in a Materials List and choose Update To Master List.");
    }
    ui.separator();
    // Waste and stock lengths.
    ui.collapsing("Waste and stock lengths", |ui| {
        egui::Grid::new("ml_master_waste").show(ui, |ui| {
            for c in CATEGORIES {
                ui.label(c);
                let mut w = m.waste_for(c);
                if ui
                    .add(egui::DragValue::new(&mut w).range(0.0..=100.0).suffix(" %"))
                    .changed()
                {
                    m.waste.insert(c.to_string(), w);
                    changed = true;
                }
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lumber stock lengths (ft)");
            if ui
                .add(egui::TextEdit::singleline(&mut ui_state.stock_text).desired_width(140.0))
                .changed()
            {
                let v = parse_stock(&ui_state.stock_text);
                if !v.is_empty() {
                    m.stock_lengths_ft = v;
                    changed = true;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                st.master_dirty || changed,
                egui::Button::new("Save Master List"),
            )
            .clicked()
        {
            match lw::save_master_list(&m) {
                Ok(()) => {
                    st.status = "Saved the Master List".into();
                    st.master_dirty = false;
                    changed = false;
                }
                Err(e) => st.status = e,
            }
        }
        if st.master_dirty || changed {
            ui.weak("Unsaved changes");
        }
    });
    if changed {
        st.master_dirty = true;
    }
    st.master = Some(m);
}
