//! The panels of the Schedule Specification dialog (manual pp. 719 to 728):
//! General, Columns/Rows, Number Formatting, Attributes, Line Style, Fill
//! Style, the three text styles and Labels.

use super::{combo, Form, Mode};
use crate::dialogs::{row, section, ERROR_RED};
use eframe::egui::{self, Ui};
use plan_core::schedules::{
    Accuracy, CalloutLayer, CalloutShape, ColorFrom, ColumnSpec, FloorScope, FractionStyle,
    LabelFormat, NumFormat, NumKind, NumUnit, NumberStyle, Numbering, Reduce, RoomRef,
    ScheduleKind, TableAlign, TextAlign, Thousands, VAlign, WrapBy,
};
use plan_docs::schedule_kinds::{category_ticked, CategoryGroup};

fn kind_name(k: ScheduleKind) -> &'static str {
    if k == ScheduleKind::General {
        "General (custom)"
    } else {
        k.name()
    }
}

impl Form {
    // ----- General -----

    pub(super) fn general(&mut self, ui: &mut Ui) {
        section(ui, "Main Title");
        row(ui, "Main Title", |ui| {
            let hint = self.def.kind.title();
            ui.add(
                egui::TextEdit::singleline(&mut self.def.title)
                    .hint_text(hint)
                    .desired_width(240.0),
            );
            ui.checkbox(&mut self.def.show_title, "Display");
        });
        row(ui, "Schedule type", |ui| {
            let mut kind = self.def.kind;
            let options: Vec<(ScheduleKind, &str)> = ScheduleKind::ALL
                .iter()
                .map(|k| (*k, kind_name(*k)))
                .collect();
            combo(ui, "schedule_kind", &mut kind, &options);
            if kind != self.def.kind {
                self.def.set_kind(kind);
                self.sync_prop_columns();
            }
        });

        section(ui, "Include Options");
        let mut all = self.def.floor_scope == FloorScope::All;
        if ui
            .checkbox(&mut all, "Include Objects from All Floors")
            .changed()
        {
            self.def.floor_scope = if all {
                FloorScope::All
            } else {
                FloorScope::ThisFloor
            };
        }
        ui.add_enabled_ui(!all, |ui| {
            ui.label("Include Objects from Floor");
            let home = self.floor;
            let names = self.ctx.floors.clone();
            for (i, name) in names.iter().enumerate() {
                let listed = if self.def.floors.is_empty() {
                    i == home
                } else {
                    self.def.floors.contains(&i)
                };
                let mut on = listed;
                if ui.checkbox(&mut on, name).changed() {
                    if self.def.floors.is_empty() {
                        self.def.floors = vec![home];
                    }
                    if on {
                        if !self.def.floors.contains(&i) {
                            self.def.floors.push(i);
                        }
                    } else {
                        self.def.floors.retain(|f| *f != i);
                    }
                    self.def.floors.sort_unstable();
                    // The home floor again is the same as no choice.
                    if self.def.floors == [home] {
                        self.def.floors.clear();
                    }
                }
            }
        });
        ui.add_space(4.0);
        ui.label("Include Objects from Room");
        let rooms: Vec<(RoomRef, String)> = self
            .ctx
            .rooms
            .iter()
            .filter(|(r, _)| self.floor_listed(r.floor))
            .cloned()
            .collect();
        ui.horizontal(|ui| {
            if ui.button("Select All").clicked() {
                self.def.rooms = rooms.iter().map(|(r, _)| *r).collect();
            }
            if ui.button("Clear All").clicked() {
                self.def.rooms.clear();
            }
            ui.weak("none checked: every room");
        });
        egui::ScrollArea::vertical()
            .id_salt("schedule_rooms")
            .max_height(90.0)
            .show(ui, |ui| {
                if rooms.is_empty() {
                    ui.weak("The floors listed have no rooms");
                }
                for (r, name) in &rooms {
                    let at = self
                        .def
                        .rooms
                        .iter()
                        .position(|x| x.floor == r.floor && x.point().dist(r.point()) < 1.0);
                    let mut on = at.is_some();
                    let floor_name = self.ctx.floors.get(r.floor).cloned().unwrap_or_default();
                    let label = if name.is_empty() {
                        format!("Room ({floor_name})")
                    } else {
                        format!("{name} ({floor_name})")
                    };
                    if ui.checkbox(&mut on, label).changed() {
                        match (on, at) {
                            (true, None) => self.def.rooms.push(*r),
                            (false, Some(i)) => {
                                self.def.rooms.remove(i);
                            }
                            _ => {}
                        }
                    }
                }
            });
        // Rooms of floors that are no longer listed do not count.
        let listed: Vec<usize> = (0..self.ctx.floors.len())
            .filter(|f| self.floor_listed(*f))
            .collect();
        self.def.rooms.retain(|r| listed.contains(&r.floor));
        row(ui, "Row filter", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.def.filter)
                    .hint_text("only rows containing...")
                    .desired_width(200.0),
            )
        });

        section(ui, "Included Categories");
        self.categories_tree(ui);

        section(ui, "Output");
        self.output_buttons(ui);
    }

    fn output_buttons(&mut self, ui: &mut Ui) {
        if self.mode == Mode::Defaults {
            ui.weak("New schedules of this type start with these settings.");
            return;
        }
        ui.horizontal(|ui| {
            if ui.button("Export CSV\u{2026}").clicked() {
                self.actions.export_csv = true;
            }
            if ui.button("Export Excel\u{2026}").clicked() {
                self.actions.export_xlsx = true;
            }
            if ui.button("Open in Window").clicked() {
                self.actions.open_window = true;
            }
            if ui.button("Send to Layout").clicked() {
                self.actions.send_to_layout = true;
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Export for Editing (XLSX)\u{2026}")
                .on_hover_text(
                    "A workbook with a hidden PlanStudio ID column: edit names, marks and \
                     properties in Excel, then import it back",
                )
                .clicked()
            {
                self.actions.export_for_editing = true;
            }
            if ui
                .button("Import Property Data\u{2026}")
                .on_hover_text("Read an edited workbook or CSV back into the plan")
                .clicked()
            {
                self.actions.import_props = true;
            }
        });
    }

    pub(super) fn tree(&self) -> Vec<CategoryGroup> {
        let mut t = self.ctx.tree_of(self.def.kind);
        // Categories made in this dialog and not yet in the plan.
        let fresh: Vec<&String> = self
            .new_categories
            .iter()
            .filter(|n| {
                !t.iter()
                    .any(|g| g.items.iter().any(|i| i.title == **n && i.custom))
            })
            .collect();
        if !fresh.is_empty() {
            let id = "Custom".to_string();
            if !t.iter().any(|g| g.id == id) {
                t.push(CategoryGroup {
                    id: id.clone(),
                    title: "Custom Categories".into(),
                    items: Vec::new(),
                });
            }
            if let Some(g) = t.iter_mut().find(|g| g.id == id) {
                for n in fresh {
                    g.items.push(plan_docs::schedule_kinds::CategoryNode {
                        id: plan_core::schedules::custom_category_id(n),
                        title: n.clone(),
                        custom: true,
                    });
                }
            }
        }
        t
    }

    /// Writes every category of the tree into the schedule so that one
    /// change leaves the others as they showed.
    pub(super) fn materialize(&mut self, tree: &[CategoryGroup]) {
        if self.def.categories.is_empty() {
            for g in tree {
                for n in &g.items {
                    let on = category_ticked(&self.def, &n.id) && !n.custom;
                    self.def.set_category(&n.id, on);
                }
            }
        }
    }

    fn categories_tree(&mut self, ui: &mut Ui) {
        let tree = self.tree();
        if tree.is_empty() {
            ui.weak("This kind of schedule has no categories.");
        }
        for g in &tree {
            let ticked = |f: &Form, id: &str| category_ticked(&f.def, id);
            let n_on = g.items.iter().filter(|n| ticked(self, &n.id)).count();
            let state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                egui::Id::new(("schedule_cat_group", &g.id)),
                tree.len() == 1,
            );
            state
                .show_header(ui, |ui| {
                    let mut all = n_on == g.items.len() && !g.items.is_empty();
                    let label = format!("{}  ({}/{})", g.title, n_on, g.items.len());
                    if ui.checkbox(&mut all, label).changed() {
                        self.materialize(&tree);
                        for n in &g.items {
                            self.def.set_category(&n.id, all);
                        }
                    }
                })
                .body(|ui| {
                    for n in &g.items {
                        let mut on = ticked(self, &n.id);
                        if ui.checkbox(&mut on, &n.title).changed() {
                            self.materialize(&tree);
                            self.def.set_category(&n.id, on);
                        }
                    }
                });
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_category)
                    .hint_text("new category name")
                    .desired_width(150.0),
            );
            if ui.button("New Custom Category").clicked() {
                let name = self.new_category.trim().to_string();
                if name.is_empty() {
                    self.category_error = "Type a name for the category".into();
                } else if self
                    .ctx
                    .custom
                    .iter()
                    .chain(self.new_categories.iter())
                    .any(|c| *c == name)
                {
                    self.category_error = format!("There is already a category named \"{name}\"");
                } else {
                    let tree = self.tree();
                    self.materialize(&tree);
                    self.def
                        .set_category(&plan_core::schedules::custom_category_id(&name), true);
                    self.new_categories.push(name);
                    self.new_category.clear();
                    self.category_error.clear();
                }
            }
        });
        if !self.category_error.is_empty() {
            ui.colored_label(ERROR_RED, self.category_error.clone());
        }
        ui.checkbox(
            &mut self.def.new_types_included,
            "Include new wall, room and note types",
        );
    }

    // ----- Columns/Rows -----

    pub(super) fn columns_rows(&mut self, ui: &mut Ui) {
        section(ui, "Columns");
        ui.checkbox(&mut self.def.show_headings, "Display Column Headings");
        ui.checkbox(
            &mut self.limit_to_categories,
            "Limit List to Included Categories",
        );
        self.column_lists(ui);
        self.column_totals(ui);

        section(ui, "Rows");
        ui.checkbox(&mut self.def.group_similar, "Group Similar Objects");
        ui.horizontal(|ui| {
            let can = self.def.kind.has_totals_row();
            ui.add_enabled_ui(can || self.has_calc_total(), |ui| {
                ui.checkbox(&mut self.def.totals_row, "Display Totals Row");
            });
            ui.label("Label");
            ui.add(egui::TextEdit::singleline(&mut self.def.totals_label).desired_width(90.0));
        });
        row(ui, "Minimum Rows", |ui| {
            let mut n = self.def.min_rows as u32;
            ui.add(egui::DragValue::new(&mut n).range(0..=200));
            self.def.min_rows = n as usize;
        });
        ui.horizontal(|ui| {
            let mut sorted = !self.def.sort.field.is_empty();
            if ui.checkbox(&mut sorted, "Automatically Sort by").changed() {
                if sorted {
                    if let Some(c) = self.def.columns.iter().find(|c| c.visible) {
                        self.def.sort.field = c.field.clone();
                    }
                } else {
                    self.def.sort.field.clear();
                }
            }
            ui.add_enabled_ui(sorted, |ui| {
                let current = self
                    .def
                    .columns
                    .iter()
                    .find(|c| c.field == self.def.sort.field)
                    .map_or(String::new(), |c| c.title.clone());
                egui::ComboBox::from_id_salt("schedule_sort")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for c in self.def.columns.iter().filter(|c| c.visible) {
                            ui.selectable_value(
                                &mut self.def.sort.field,
                                c.field.clone(),
                                &c.title,
                            );
                        }
                    });
                ui.radio_value(&mut self.def.sort.descending, false, "Ascending");
                ui.radio_value(&mut self.def.sort.descending, true, "Descending");
            });
        });
        ui.checkbox(&mut self.def.swap, "Swap Rows/Columns");
        row(ui, "Group by column", |ui| {
            let current = self
                .def
                .columns
                .iter()
                .find(|c| c.field == self.def.group_by)
                .map_or("(none: one line per object)".to_string(), |c| {
                    c.title.clone()
                });
            egui::ComboBox::from_id_salt("schedule_group")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.def.group_by,
                        String::new(),
                        "(none: one line per object)",
                    );
                    for c in &self.def.columns {
                        ui.selectable_value(&mut self.def.group_by, c.field.clone(), &c.title);
                    }
                });
        });
        ui.checkbox(
            &mut self.def.totals,
            "Count line (number of objects, sums of areas)",
        );

        section(ui, "Wrapping");
        ui.checkbox(&mut self.def.wrap.enabled, "Wrapping");
        let swapped = self.def.swap;
        ui.add_enabled_ui(self.def.wrap.enabled, |ui| {
            let mut entries = matches!(self.def.wrap.by, WrapBy::Entries(_));
            ui.horizontal(|ui| {
                if ui
                    .radio_value(&mut entries, true, "Entries per Table")
                    .clicked()
                {
                    self.def.wrap.by = WrapBy::Entries(10);
                }
                if ui
                    .radio_value(&mut entries, false, "Max Table Size")
                    .clicked()
                {
                    self.def.wrap.by = WrapBy::MaxSize(240.0);
                }
            });
            match &mut self.def.wrap.by {
                WrapBy::Entries(n) => {
                    row(
                        ui,
                        if swapped {
                            "Columns per table"
                        } else {
                            "Rows per table"
                        },
                        |ui| {
                            let mut v = *n as u32;
                            ui.add(egui::DragValue::new(&mut v).range(1..=500));
                            *n = v as usize;
                        },
                    );
                }
                WrapBy::MaxSize(len) => {
                    row(
                        ui,
                        if swapped {
                            "Max width (in)"
                        } else {
                            "Max height (in)"
                        },
                        |ui| {
                            ui.add(egui::DragValue::new(len).range(6.0..=2400.0).speed(1.0));
                        },
                    );
                }
            }
            row(ui, "Wrapped Schedule Offset", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.def.wrap.offset)
                        .range(0.0..=240.0)
                        .speed(0.5),
                );
            });
            ui.checkbox(&mut self.def.wrap.justify, "Justify Wrapped Tables");
            row(ui, "Table Alignment", |ui| {
                let opts = [
                    (TableAlign::Start, TableAlign::Start.name(swapped)),
                    (TableAlign::Centered, "Centered"),
                    (TableAlign::End, TableAlign::End.name(swapped)),
                ];
                combo(ui, "schedule_wrap_align", &mut self.def.wrap.align, &opts);
            });
            ui.checkbox(
                &mut self.def.wrap.title_each,
                "Display Title on Wrapped Tables",
            );
            ui.checkbox(
                &mut self.def.wrap.headings_each,
                "Display Column Headings in Wrapped Tables",
            );
        });

        section(ui, "Object Preview Options");
        let p = &mut self.def.previews;
        ui.checkbox(&mut p.show_color, "Show Color");
        ui.add_enabled_ui(p.show_color, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut p.color_from, ColorFrom::Plan, "From Plan");
                ui.radio_value(&mut p.color_from, ColorFrom::Schedule, "From Schedule");
            });
        });
        ui.checkbox(&mut p.scale_images, "Scale Images");
        let kind = self.def.kind;
        ui.add_enabled_ui(
            matches!(
                kind,
                ScheduleKind::Electrical
                    | ScheduleKind::Fixture
                    | ScheduleKind::Furniture
                    | ScheduleKind::Note
                    | ScheduleKind::Plant
                    | ScheduleKind::Wall
                    | ScheduleKind::General
            ),
            |ui| {
                ui.checkbox(&mut p.plan_view_scale, "Use Plan View Scale");
            },
        );
        ui.add_enabled_ui(
            matches!(
                kind,
                ScheduleKind::Door
                    | ScheduleKind::Window
                    | ScheduleKind::Cabinet
                    | ScheduleKind::General
            ),
            |ui| {
                ui.checkbox(&mut p.opening_indicators, "Show Opening Indicators");
            },
        );
        ui.add_enabled_ui(
            matches!(
                kind,
                ScheduleKind::Door | ScheduleKind::Window | ScheduleKind::General
            ),
            |ui| {
                ui.checkbox(&mut p.casing, "Show Casing, Lintel, Sill");
                ui.checkbox(&mut p.treatments, "Show Treatments, Shutters");
            },
        );
    }

    fn has_calc_total(&self) -> bool {
        self.def.columns.iter().any(|c| c.visible && c.calc_total)
    }

    /// Available Columns and Columns to Include.
    fn column_lists(&mut self, ui: &mut Ui) {
        let kind = self.def.kind;
        // (field, title) in the order of the columns to include.
        let included: Vec<(String, String)> = self
            .def
            .columns
            .iter()
            .filter(|c| c.visible)
            .map(|c| (c.field.clone(), c.title.clone()))
            .collect();
        let mut available: Vec<(String, String)> = self
            .def
            .columns
            .iter()
            .filter(|c| !c.visible)
            .map(|c| (c.field.clone(), c.title.clone()))
            .collect();
        if self.limit_to_categories && !kind.has_previews() {
            available.retain(|(f, _)| !plan_core::schedules::is_preview_field(f));
        }
        available.sort_by_key(|(_, t)| t.to_lowercase());
        let mut add: Vec<String> = Vec::new();
        let mut remove: Vec<String> = Vec::new();
        ui.columns(2, |cols| {
            cols[0].strong("Available Columns");
            egui::ScrollArea::vertical()
                .id_salt("schedule_avail")
                .max_height(150.0)
                .show(&mut cols[0], |ui| {
                    for (f, t) in &available {
                        let sel = self.avail_sel.contains(f);
                        let r = ui.selectable_label(sel, t);
                        if r.double_clicked() {
                            add.push(f.clone());
                        } else if r.clicked() {
                            let multi = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                            if !multi {
                                self.avail_sel.clear();
                            }
                            if !self.avail_sel.remove(f) || !multi {
                                self.avail_sel.insert(f.clone());
                            }
                        }
                    }
                });
            cols[1].strong("Columns to Include");
            egui::ScrollArea::vertical()
                .id_salt("schedule_incl")
                .max_height(150.0)
                .show(&mut cols[1], |ui| {
                    for (f, t) in &included {
                        let sel = self.incl_sel.contains(f);
                        let r = ui.selectable_label(sel, t);
                        if r.double_clicked() {
                            self.rename_to = Some(f.clone());
                            self.rename_text = t.clone();
                        } else if r.clicked() {
                            let multi = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                            if !multi {
                                self.incl_sel.clear();
                            }
                            if !self.incl_sel.remove(f) || !multi {
                                self.incl_sel.insert(f.clone());
                            }
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.avail_sel.is_empty(),
                    egui::Button::new("Add \u{25B6}"),
                )
                .clicked()
            {
                add.extend(self.avail_sel.iter().cloned());
            }
            if ui
                .add_enabled(
                    !self.incl_sel.is_empty(),
                    egui::Button::new("\u{25C0} Remove"),
                )
                .clicked()
            {
                remove.extend(self.incl_sel.iter().cloned());
            }
            let one = self.incl_sel.len() == 1;
            if ui.add_enabled(one, egui::Button::new("Move Up")).clicked() {
                if let Some(f) = self.incl_sel.iter().next().cloned() {
                    self.move_included(&f, true);
                }
            }
            if ui
                .add_enabled(one, egui::Button::new("Move Down"))
                .clicked()
            {
                if let Some(f) = self.incl_sel.iter().next().cloned() {
                    self.move_included(&f, false);
                }
            }
            if ui.add_enabled(one, egui::Button::new("Rename")).clicked() {
                if let Some(f) = self.incl_sel.iter().next().cloned() {
                    self.rename_text = self
                        .def
                        .columns
                        .iter()
                        .find(|c| c.field == f)
                        .map_or(String::new(), |c| c.title.clone());
                    self.rename_to = Some(f);
                }
            }
            if ui.button("Reset").clicked() {
                self.reset_titles();
            }
        });
        if let Some(f) = self.rename_to.clone() {
            ui.horizontal(|ui| {
                ui.label("New name");
                let r =
                    ui.add(egui::TextEdit::singleline(&mut self.rename_text).desired_width(160.0));
                let done = ui.button("OK").clicked()
                    || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if done {
                    if let Some(c) = self.def.columns.iter_mut().find(|c| c.field == f) {
                        c.title = self.rename_text.trim().to_string();
                    }
                    self.rename_to = None;
                }
                if ui.button("Cancel").clicked() {
                    self.rename_to = None;
                }
            });
        }
        for f in add {
            self.include_column(&f);
        }
        for f in remove {
            self.exclude_column(&f);
        }
    }

    /// Calculate Total and Sum Similar Rows for the selected column.
    fn column_totals(&mut self, ui: &mut Ui) {
        let kind = self.def.kind;
        let Some(field) = self
            .incl_sel
            .iter()
            .next()
            .cloned()
            .filter(|_| self.incl_sel.len() == 1)
        else {
            ui.weak("Select one column to set its totals.");
            return;
        };
        let numeric = kind.num_kind(&field).is_some();
        if let Some(c) = self.def.columns.iter_mut().find(|c| c.field == field) {
            ui.add_enabled_ui(numeric, |ui| {
                ui.checkbox(&mut c.calc_total, "Calculate Total");
                ui.checkbox(&mut c.sum_similar, "Sum Similar Rows");
            });
            if !numeric {
                ui.weak("Only columns that report a quantity, length, area or volume have totals.");
            }
        }
    }

    // ----- Number Formatting -----

    pub(super) fn number_formatting(&mut self, ui: &mut Ui) {
        section(ui, "Fraction Format");
        row(ui, "Fraction Style", |ui| {
            let opts: Vec<(FractionStyle, &str)> =
                FractionStyle::ALL.iter().map(|s| (*s, s.name())).collect();
            combo(
                ui,
                "schedule_fraction_style",
                &mut self.def.fraction.style,
                &opts,
            );
        });
        ui.add_enabled_ui(self.def.fraction.style != FractionStyle::Horizontal, |ui| {
            row(ui, "Fraction Text Size (%)", |ui| {
                ui.add(egui::DragValue::new(&mut self.def.fraction.text_pct).range(20.0..=100.0));
            });
        });

        section(ui, "Format Column");
        let kind = self.def.kind;
        let numeric: Vec<(String, String, NumKind)> = self
            .def
            .columns
            .iter()
            .filter(|c| c.visible)
            .filter_map(|c| {
                kind.num_kind(&c.field)
                    .map(|n| (c.field.clone(), c.title.clone(), n))
            })
            .collect();
        if numeric.is_empty() {
            ui.weak("No column of this schedule holds a number.");
            return;
        }
        egui::Grid::new("schedule_numfmt_list")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Name");
                ui.strong("Preview");
                ui.end_row();
                for (f, t, nk) in &numeric {
                    let c = self.def.columns.iter().find(|c| c.field == *f);
                    let sample = sample_value(*nk);
                    let preview = match c.and_then(|c| c.format) {
                        Some(fmt) => plan_core::schedules::format_value(sample, *nk, &fmt),
                        None => "(plan text)".to_string(),
                    };
                    let sel = self.fmt_sel.as_deref() == Some(f.as_str());
                    if ui.selectable_label(sel, t).clicked() {
                        self.fmt_sel = Some(f.clone());
                    }
                    ui.label(preview);
                    ui.end_row();
                }
            });
        if ui.button("Reset").clicked() {
            for c in &mut self.def.columns {
                c.format = None;
            }
        }
        let Some(sel) = self
            .fmt_sel
            .clone()
            .filter(|s| numeric.iter().any(|(f, _, _)| f == s))
        else {
            ui.weak("Select a column to set its number format.");
            return;
        };
        let nk = kind.num_kind(&sel).unwrap_or(NumKind::Length);
        let Some(col) = self.def.columns.iter_mut().find(|c| c.field == sel) else {
            return;
        };
        let mut on = col.format.is_some();
        if ui.checkbox(&mut on, "Format this column").changed() {
            col.format = on.then(|| NumFormat::default_for(nk));
        }
        let Some(f) = col.format.as_mut() else {
            return;
        };
        section(ui, "Format");
        if nk == NumKind::Length {
            row(ui, "Units", |ui| {
                let opts: Vec<(NumUnit, &str)> =
                    NumUnit::ALL.iter().map(|u| (*u, u.name())).collect();
                combo(ui, "schedule_numfmt_units", &mut f.units, &opts);
            });
        }
        ui.checkbox(&mut f.unit_indicators, "Unit Indicators");
        ui.checkbox(&mut f.leading_zeros, "Leading Zeros");
        ui.checkbox(&mut f.trailing_zeros, "Trailing Zeros");
        row(ui, "Thousands Separator", |ui| {
            let opts: Vec<(Thousands, &str)> =
                Thousands::ALL.iter().map(|t| (*t, t.name())).collect();
            combo(ui, "schedule_numfmt_thousands", &mut f.thousands, &opts);
        });
        section(ui, "Accuracy");
        let mut decimal = matches!(f.accuracy, Accuracy::Decimal(_));
        ui.horizontal(|ui| {
            if ui
                .radio_value(&mut decimal, true, "Decimal Places")
                .clicked()
            {
                f.accuracy = Accuracy::Decimal(match f.accuracy {
                    Accuracy::Decimal(n) => n,
                    Accuracy::Fraction(_) => 2,
                });
            }
            if ui
                .radio_value(&mut decimal, false, "Smallest Fraction")
                .clicked()
            {
                f.accuracy = Accuracy::Fraction(match f.accuracy {
                    Accuracy::Fraction(n) => n,
                    Accuracy::Decimal(_) => 16,
                });
            }
        });
        match &mut f.accuracy {
            Accuracy::Decimal(n) => {
                row(ui, "Decimal Places (0 to 20)", |ui| {
                    ui.add(egui::DragValue::new(n).range(0..=20));
                });
            }
            Accuracy::Fraction(d) => {
                row(ui, "Largest denominator (1 to 128)", |ui| {
                    ui.add(egui::DragValue::new(d).range(1..=128));
                });
                ui.checkbox(&mut f.show_denominator, "Show Denominator");
                ui.checkbox(&mut f.reduce_fractions, "Reduce Fractions");
                ui.add_enabled_ui(f.reduce_fractions, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut f.reduce_mode,
                            Reduce::Gcd,
                            "Use Greatest Common Divisor",
                        );
                        ui.radio_value(&mut f.reduce_mode, Reduce::Closest, "Use Closest Fraction");
                    });
                });
            }
        }
    }

    // ----- Attributes -----

    pub(super) fn attributes(&mut self, ui: &mut Ui) {
        section(ui, "Box/Grid");
        ui.checkbox(&mut self.def.border, "Display Border");
        ui.checkbox(&mut self.def.grid_lines, "Display Grid Lines");
        section(ui, "Alignment");
        row(ui, "Horizontal", |ui| {
            let opts: Vec<(TextAlign, &str)> =
                TextAlign::ALL.iter().map(|a| (*a, a.name())).collect();
            combo(ui, "schedule_halign", &mut self.def.h_align, &opts);
        });
        row(ui, "Vertical", |ui| {
            let opts: Vec<(VAlign, &str)> = VAlign::ALL.iter().map(|a| (*a, a.name())).collect();
            combo(ui, "schedule_valign", &mut self.def.v_align, &opts);
        });
        if self.mode == Mode::Placed {
            section(ui, "Position");
            let (w, h) = self.size;
            let centre = (self.def.position.x + w / 2.0, self.def.position.y - h / 2.0);
            let mut c = centre;
            row(ui, "X Position", |ui| {
                ui.add(egui::DragValue::new(&mut c.0).speed(1.0));
            });
            row(ui, "Y Position", |ui| {
                ui.add(egui::DragValue::new(&mut c.1).speed(1.0));
            });
            if c != centre {
                self.def.position.x += c.0 - centre.0;
                self.def.position.y += c.1 - centre.1;
            }
            row(ui, "Angle", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.def.angle)
                        .range(-360.0..=360.0)
                        .speed(0.5),
                );
            });
        }
        section(ui, "Margins");
        let m = &mut self.def.margins;
        for (i, name) in ["Left", "Right", "Top", "Bottom"].iter().enumerate() {
            row(ui, name, |ui| {
                ui.add(egui::DragValue::new(&mut m[i]).range(0.0..=24.0).speed(0.1));
            });
        }
    }

    // ----- Line Style, Fill Style, text styles -----

    pub(super) fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let mut own = self.def.line_color.is_some();
        if ui.checkbox(&mut own, "Own line color").changed() {
            self.def.line_color = own.then_some([0, 0, 0]);
        }
        if let Some(c) = self.def.line_color.as_mut() {
            row(ui, "Color", |ui| {
                ui.color_edit_button_srgb(c);
            });
        } else {
            ui.weak("The lines follow the color of the main text.");
        }
        row(ui, "Line Weight", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.def.line_weight)
                    .range(0.2..=8.0)
                    .speed(0.1),
            );
        });
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("schedule_layer")
                .selected_text(self.def.layer.clone())
                .show_ui(ui, |ui| {
                    for name in &self.layers {
                        ui.selectable_value(&mut self.def.layer, name.clone(), name);
                    }
                });
        });
    }

    pub(super) fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        ui.checkbox(&mut self.def.fill, "Fill the table's background");
        ui.add_enabled_ui(self.def.fill, |ui| {
            let mut own = self.def.fill_color.is_some();
            if ui.checkbox(&mut own, "Own fill color").changed() {
                self.def.fill_color = own.then_some([255, 255, 255]);
            }
            if let Some(c) = self.def.fill_color.as_mut() {
                row(ui, "Color", |ui| {
                    ui.color_edit_button_srgb(c);
                });
            } else {
                ui.weak("The background is the paper color of the plan.");
            }
        });
    }

    pub(super) fn text_style(&mut self, ui: &mut Ui, which: usize) {
        let (title, hint) = match which {
            0 => ("Main Text Style", "Used in the body of the schedule."),
            1 => (
                "Title Text Style",
                "Used in the title. Blank follows the main style, a little larger.",
            ),
            _ => (
                "Header Text Style",
                "Used in the column headings. Blank follows the main style.",
            ),
        };
        section(ui, title);
        let current = match which {
            0 => &mut self.def.text_style,
            1 => &mut self.def.title_style,
            _ => &mut self.def.header_style,
        };
        row(ui, "Text style", |ui| {
            egui::ComboBox::from_id_salt(("schedule_text_style", which))
                .selected_text(if current.is_empty() {
                    "(follow main)"
                } else {
                    current.as_str()
                })
                .show_ui(ui, |ui| {
                    if which != 0 {
                        ui.selectable_value(current, String::new(), "(follow main)");
                    }
                    for name in &self.text_styles {
                        ui.selectable_value(current, name.clone(), name);
                    }
                });
        });
        ui.add_space(6.0);
        ui.weak(hint);
        if which == 0 {
            ui.weak("Callout labels use the \"Schedule Label\" style when the plan has one.");
        }
    }

    // ----- Labels -----

    pub(super) fn labels(&mut self, ui: &mut Ui) {
        let supported = self.def.kind.has_labels();
        section(ui, "Label Format");
        ui.add_enabled_ui(supported, |ui| {
            ui.checkbox(
                &mut self.def.show_labels,
                "Show schedule number labels in the plan",
            );
            for f in LabelFormat::ALL {
                ui.radio_value(&mut self.def.label.format, f, f.name());
            }
            row(ui, "Numbering", |ui| {
                combo(
                    ui,
                    "schedule_numbering",
                    &mut self.def.numbering,
                    &[
                        (
                            Numbering::ByFloor,
                            "By floor (each floor starts at the first number)",
                        ),
                        (Numbering::Whole, "Whole plan (keeps counting)"),
                    ],
                )
            });
        });
        section(ui, "Label Text");
        row(ui, "Schedule Number Prefix", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.def.label_prefix).desired_width(80.0));
            if ui.button("Default").clicked() {
                self.def.label_prefix = self.def.kind.default_prefix().to_string();
            }
        });
        row(ui, "Schedule Start Number", |ui| {
            ui.add(egui::DragValue::new(&mut self.def.label.start_number).range(0..=9999));
        });
        row(ui, "Schedule Numbers Format", |ui| {
            let opts: Vec<(NumberStyle, &str)> =
                NumberStyle::ALL.iter().map(|s| (*s, s.name())).collect();
            combo(
                ui,
                "schedule_number_style",
                &mut self.def.label.number_style,
                &opts,
            );
        });
        ui.checkbox(&mut self.def.label.leading_zeros, "Include Leading Zeroes");
        let first = self.def.mark_text(1);
        let second = self.def.mark_text(2);
        ui.weak(format!("Marks read {first}, {second}, ..."));
        if !supported {
            ui.weak(format!(
                "{} objects have no callout label in the plan; the prefix only sets the Mark column.",
                self.def.kind.name()
            ));
        }

        section(ui, "Callout Shape");
        let o = &mut self.def.label;
        row(ui, "Shape", |ui| {
            let current = o.shape.map_or("(the kind's own)", |s| s.name());
            egui::ComboBox::from_id_salt("schedule_callout_shape")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut o.shape, None, "(the kind's own)");
                    for s in CalloutShape::ALL {
                        ui.selectable_value(&mut o.shape, Some(s), s.name());
                    }
                });
        });
        section(ui, "Callout Fill Color");
        ui.checkbox(&mut o.filled, "Filled");
        ui.add_enabled_ui(o.filled, |ui| {
            ui.checkbox(&mut o.fill_by_layer, "By Layer");
            if !o.fill_by_layer {
                row(ui, "Color", |ui| {
                    ui.color_edit_button_srgb(&mut o.fill_color);
                });
            }
            row(ui, "Transparency (%)", |ui| {
                let mut t = u32::from(o.transparency);
                ui.add(egui::Slider::new(&mut t, 0..=100));
                o.transparency = t as u8;
            });
        });
        section(ui, "Callout Size/Orientation");
        ui.checkbox(&mut o.auto_size, "Automatic");
        ui.add_enabled_ui(!o.auto_size, |ui| {
            row(ui, "Size", |ui| {
                ui.add(
                    egui::DragValue::new(&mut o.size)
                        .range(1.0..=60.0)
                        .speed(0.25),
                );
            });
        });
        row(ui, "Shape Angle", |ui| {
            ui.add(egui::DragValue::new(&mut o.shape_angle).range(-360.0..=360.0));
        });
        ui.checkbox(&mut o.auto_text_angle, "Text Angle Automatic");
        ui.add_enabled_ui(!o.auto_text_angle, |ui| {
            row(ui, "Text Angle", |ui| {
                ui.add(egui::DragValue::new(&mut o.text_angle).range(-360.0..=360.0));
            });
        });
        ui.checkbox(&mut o.follow_label, "Follow Label");
        section(ui, "Callout Layer");
        let mut choice = match &o.layer {
            CalloutLayer::ObjectLabel => 0,
            CalloutLayer::Schedule => 1,
            CalloutLayer::Custom(_) => 2,
        };
        let mut changed = false;
        changed |= ui
            .radio_value(&mut choice, 0, "Use Object Label Layer")
            .clicked();
        changed |= ui
            .radio_value(
                &mut choice,
                1,
                format!("Use Object Layer ({})", self.def.layer),
            )
            .clicked();
        changed |= ui.radio_value(&mut choice, 2, "Use Custom Layer").clicked();
        if changed {
            o.layer = match choice {
                0 => CalloutLayer::ObjectLabel,
                1 => CalloutLayer::Schedule,
                _ => CalloutLayer::Custom(self.layers.first().cloned().unwrap_or_default()),
            };
        }
        if let CalloutLayer::Custom(name) = &mut o.layer {
            egui::ComboBox::from_id_salt("schedule_callout_layer")
                .selected_text(name.clone())
                .show_ui(ui, |ui| {
                    for l in &self.layers {
                        ui.selectable_value(name, l.clone(), l);
                    }
                });
        }
    }

    // ----- Column list helpers -----

    /// Does the schedule list objects of `floor` (before the rooms narrow it)?
    pub(super) fn floor_listed(&self, floor: usize) -> bool {
        self.def.lists_floor(floor, self.floor)
    }

    /// Moves the included column `field` one place among the included ones.
    pub(super) fn move_included(&mut self, field: &str, up: bool) {
        let vis: Vec<usize> = self
            .def
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.visible)
            .map(|(i, _)| i)
            .collect();
        let Some(pos) = vis.iter().position(|i| self.def.columns[*i].field == field) else {
            return;
        };
        let other = if up {
            pos.checked_sub(1)
        } else {
            (pos + 1 < vis.len()).then_some(pos + 1)
        };
        if let Some(o) = other {
            self.def.columns.swap(vis[pos], vis[o]);
        }
    }

    /// Adds a column to the Columns to Include (at the end of the included).
    pub(super) fn include_column(&mut self, field: &str) {
        let Some(i) = self.def.columns.iter().position(|c| c.field == field) else {
            return;
        };
        let mut c = self.def.columns.remove(i);
        c.visible = true;
        let at = self
            .def
            .columns
            .iter()
            .rposition(|c| c.visible)
            .map_or(0, |p| p + 1);
        self.def.columns.insert(at, c);
        self.avail_sel.remove(field);
    }

    pub(super) fn exclude_column(&mut self, field: &str) {
        // At least one column stays.
        if self.def.columns.iter().filter(|c| c.visible).count() <= 1 {
            return;
        }
        if let Some(c) = self.def.columns.iter_mut().find(|c| c.field == field) {
            c.visible = false;
        }
        self.incl_sel.remove(field);
    }

    /// The default heading of every column.
    pub(super) fn reset_titles(&mut self) {
        let kind = self.def.kind;
        for c in &mut self.def.columns {
            if let Some(f) = kind.fields().iter().find(|f| f.id == c.field) {
                c.title = f.title.to_string();
            }
        }
    }
}

/// A value to show what a format looks like.
fn sample_value(kind: NumKind) -> f64 {
    match kind {
        NumKind::Length => 36.3125,
        NumKind::Area => 1234.567,
        NumKind::Volume => 9876.5,
        NumKind::Feet => 42.25,
        NumKind::Count => 12.0,
        NumKind::BoardFeet => 18.667,
    }
}

#[allow(dead_code)]
fn _unused(_: &ColumnSpec) {}
