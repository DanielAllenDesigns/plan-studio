//! The smaller windows of the Materials List: Materials List Management,
//! Save Materials List, the prompt on closing, the Export Materials List
//! dialog, Details, and the Materials List Polyline Specification and its
//! defaults.

use super::State;
use crate::editor::{EditorContext, ObjectRef};
use eframe::egui;
use plan_core::materials_data::{IncludedObjects, MaterialsPolyline, PolylineSpec, CATEGORIES};
use plan_core::Id;
use plan_docs::materials::export::{self, ExportFormat, ExportOptions, ThirdParty};
use plan_docs::materials::list::{self, ListLine, UnitsMode};
use std::collections::BTreeSet;

/// A text prompt: Save Materials List, or the one on closing.
#[derive(Default, Clone)]
pub struct Prompt {
    pub name: String,
    /// What OK does.
    pub kind: PromptKind,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromptKind {
    #[default]
    SaveAs,
    /// The window is closing: save, then close.
    Close,
    /// Management: name for a copy of the selected list.
    Copy,
    /// Management: new name of the selected list.
    Rename,
}

pub struct ExportDialog {
    pub opts: ExportOptions,
}

pub struct DetailsDialog {
    pub lines: Vec<ListLine>,
}

/// Selection state of the category x floor grid.
#[derive(Default, Clone)]
pub struct GridSel {
    cats: BTreeSet<String>,
    floors: BTreeSet<usize>,
}

pub struct PolyDialog {
    pub cad_id: Option<Id>,
    pub floor: usize,
    pub name: String,
    pub spec: PolylineSpec,
    original: PolylineSpec,
    original_name: String,
    sel: GridSel,
    floors: Vec<String>,
    /// Perimeter, area (sq ft) and number of lines of the polyline.
    pub info: Option<(f64, f64, usize)>,
}

impl PolyDialog {
    pub fn changed(&self) -> bool {
        self.spec != self.original || self.name != self.original_name
    }
}

/// The windows of the Materials List other than the main one.
#[derive(Default)]
pub struct Extras {
    pub management: bool,
    pub mgmt_sel: Option<String>,
    pub prompt: Option<Prompt>,
    pub export: Option<ExportDialog>,
    pub details: Option<DetailsDialog>,
    pub polyline: Option<PolyDialog>,
    pub defaults: Option<PolyDialog>,
    /// The window of the Materials tools' by-surface list (a separate window).
    pub open_by_surface: bool,
}

impl Extras {
    pub fn open_management(&mut self) {
        self.management = true;
    }

    pub fn ask_save_as(&mut self, name: &str) {
        self.prompt = Some(Prompt {
            name: name.to_string(),
            kind: PromptKind::SaveAs,
        });
    }

    /// The window is being closed: ask to save when the list is new or changed.
    pub fn ask_close(&mut self, needs_save: bool, name: &str) {
        if needs_save {
            self.prompt = Some(Prompt {
                name: name.to_string(),
                kind: PromptKind::Close,
            });
        }
    }

    pub fn close_prompt_up(&self) -> bool {
        self.prompt
            .as_ref()
            .is_some_and(|p| p.kind == PromptKind::Close)
    }

    pub fn open_export(&mut self) {
        self.export = Some(ExportDialog {
            opts: ExportOptions::default(),
        });
    }

    pub fn open_details(&mut self, lines: Vec<ListLine>) {
        self.details = Some(DetailsDialog { lines });
    }

    pub fn open_polyline_defaults(&mut self, project: &plan_core::Project) {
        let spec = project
            .materials
            .polyline_defaults
            .clone()
            .unwrap_or_else(|| PolylineSpec::everything(project.floors.len()));
        self.defaults = Some(PolyDialog {
            cad_id: None,
            floor: 0,
            name: "Materials List Polyline Defaults".into(),
            original: spec.clone(),
            original_name: "Materials List Polyline Defaults".into(),
            spec,
            sel: GridSel::default(),
            floors: project.floors.iter().map(|f| f.name.clone()).collect(),
            info: None,
        });
    }

    /// Is any of the smaller windows up?
    pub fn any_up(&self) -> bool {
        self.management
            || self.prompt.is_some()
            || self.export.is_some()
            || self.details.is_some()
            || self.polyline.is_some()
            || self.defaults.is_some()
            || self.open_by_surface
    }
}

/// Opens the Materials List Polyline Specification of the CAD object
/// `cad_id` (Open Object, double-click). False when it is not a polyline.
pub fn open_polyline_spec(cx: &EditorContext, cad_id: Id) -> bool {
    let Some(pl) = cx.project.materials.polyline(cad_id) else {
        return false;
    };
    let info = list::polyline_points(&cx.project, cad_id).map(|(_, pts)| {
        let n = pts.len();
        let perimeter: f64 = (0..n).map(|i| pts[i].dist(pts[(i + 1) % n])).sum();
        let area = plan_core::geometry::polygon_area(&pts).abs() / 144.0;
        (perimeter, area, n)
    });
    let d = PolyDialog {
        cad_id: Some(cad_id),
        floor: pl.floor,
        name: pl.name.clone(),
        original: pl.spec.clone(),
        original_name: pl.name.clone(),
        spec: pl.spec.clone(),
        sel: GridSel::default(),
        floors: cx.project.floors.iter().map(|f| f.name.clone()).collect(),
        info,
    };
    super::with_state(|st| st.extras.polyline = Some(d));
    super::open_window();
    true
}

/// The bytes the Export Materials List dialog would write.
pub fn export_bytes(cx: &EditorContext, st: &State, opts: &ExportOptions) -> Vec<u8> {
    let lines = super::compute(cx, st).lines;
    export::export(&lines, &st.spec, opts, &st.spec.name)
}

fn export_to_file(cx: &EditorContext, st: &mut State, opts: &ExportOptions) {
    let ext = if opts.third_party == ThirdParty::BuilderTrend {
        "csv"
    } else {
        opts.format.extension()
    };
    let bytes = export_bytes(cx, st, opts);
    if cfg!(test) {
        st.status = format!("Exported {} bytes", bytes.len());
        return;
    }
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{}.{ext}", st.spec.name.replace(' ', "_")))
        .add_filter(ext, &[ext])
        .save_file()
    else {
        st.status = "Export cancelled".into();
        return;
    };
    st.status = match std::fs::write(&path, bytes) {
        Ok(()) => {
            if opts.open_in_editor {
                let _ = open_with_default(&path);
            }
            format!("Saved {}", path.display())
        }
        Err(e) => format!("Could not save: {e}"),
    };
}

fn open_with_default(path: &std::path::Path) -> std::io::Result<()> {
    let prog = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(prog)
        .arg(path)
        .spawn()
        .map(|_| ())
}

pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    management(ctx, cx, st);
    prompt(ctx, cx, st);
    export_dialog(ctx, cx, st);
    details(ctx, st);
    polyline_dialog(ctx, cx, st, false);
    polyline_dialog(ctx, cx, st, true);
    if st.extras.open_by_surface {
        st.extras.open_by_surface = crate::dialogs::materials::show(ctx, cx);
    }
}

// ------------------------------------------------------------- management --

fn management(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    if !st.extras.management {
        return;
    }
    let mut open = true;
    let mut edit: Option<String> = None;
    let mut delete: Option<String> = None;
    let mut ask: Option<(PromptKind, String)> = None;
    egui::Window::new("Materials List Management")
        .id(egui::Id::new("materials_list_mgmt"))
        .open(&mut open)
        .default_size(egui::vec2(380.0, 320.0))
        .show(ctx, |ui| {
            let lists: Vec<(String, bool)> = cx
                .project
                .materials
                .lists
                .iter()
                .map(|l| {
                    (
                        l.spec.name.clone(),
                        l.spec.kind == plan_core::materials_data::ListKind::Report,
                    )
                })
                .collect();
            if lists.is_empty() {
                ui.weak("No saved Materials Lists or Reports.");
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for (name, report) in &lists {
                        let label =
                            format!("{name}  ({})", if *report { "Report" } else { "Live List" });
                        let r = ui.selectable_label(
                            st.extras.mgmt_sel.as_deref() == Some(name.as_str()),
                            label,
                        );
                        if r.clicked() {
                            st.extras.mgmt_sel = Some(name.clone());
                        }
                        if r.double_clicked() {
                            edit = Some(name.clone());
                        }
                    }
                });
            let sel = st
                .extras
                .mgmt_sel
                .clone()
                .filter(|n| lists.iter().any(|(x, _)| x == n));
            ui.separator();
            ui.horizontal(|ui| {
                let on = sel.is_some();
                if ui.add_enabled(on, egui::Button::new("Edit")).clicked() {
                    edit = sel.clone();
                }
                if ui.add_enabled(on, egui::Button::new("Copy")).clicked() {
                    ask = sel.clone().map(|n| (PromptKind::Copy, format!("{n} copy")));
                }
                if ui.add_enabled(on, egui::Button::new("Rename")).clicked() {
                    ask = sel.clone().map(|n| (PromptKind::Rename, n));
                }
                if ui.add_enabled(on, egui::Button::new("Delete")).clicked() {
                    delete = sel.clone();
                }
            });
        });
    if let Some(name) = edit {
        if super::open_saved(st, &cx.project, &name) {
            st.extras.management = true;
        }
    }
    if let Some(name) = delete {
        if super::delete_saved(cx, &name) {
            st.status = format!("Deleted {name}");
            st.extras.mgmt_sel = None;
            if st.saved_as.as_deref() == Some(name.as_str()) {
                st.saved_as = None;
            }
        }
    }
    if let Some((kind, name)) = ask {
        st.extras.prompt = Some(Prompt { name, kind });
    }
    st.extras.management = open;
}

// ----------------------------------------------------------------- prompts --

fn prompt(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    let Some(mut p) = st.extras.prompt.take() else {
        return;
    };
    let (title, ok_label) = match p.kind {
        PromptKind::SaveAs => ("Save Materials List", "Save"),
        PromptKind::Close => ("Save Materials List?", "Yes"),
        PromptKind::Copy => ("Save Materials List", "Save"),
        PromptKind::Rename => ("Rename Materials List", "Rename"),
    };
    let mut ok = false;
    let mut no = false;
    let mut cancel = false;
    egui::Window::new(title)
        .id(egui::Id::new("materials_list_prompt"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if p.kind == PromptKind::Close {
                ui.label("Save this Materials List with the plan?");
            }
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut p.name);
            });
            ui.horizontal(|ui| {
                if ui.button(ok_label).clicked() {
                    ok = true;
                }
                if p.kind == PromptKind::Close && ui.button("No").clicked() {
                    no = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ok {
        match p.kind {
            PromptKind::SaveAs => {
                super::save_list_as(cx, st, &p.name);
            }
            PromptKind::Close => {
                super::save_list(cx, st, Some(&p.name));
                st.open = false;
            }
            PromptKind::Copy => {
                let from = st.extras.mgmt_sel.clone().unwrap_or_default();
                if let Some(n) = super::copy_saved(cx, &from, &p.name) {
                    st.status = format!("Copied to {n}");
                }
            }
            PromptKind::Rename => {
                let from = st.extras.mgmt_sel.clone().unwrap_or_default();
                if super::rename_saved(cx, &from, &p.name) {
                    if st.saved_as.as_deref() == Some(from.as_str()) {
                        st.saved_as = Some(p.name.trim().to_string());
                    }
                    st.extras.mgmt_sel = Some(p.name.trim().to_string());
                } else {
                    st.status = "That name is empty or already used".into();
                }
            }
        }
    } else if no {
        st.open = false;
        st.dirty = false;
    } else if !cancel {
        st.extras.prompt = Some(p);
    }
}

// ------------------------------------------------------------------ export --

fn export_dialog(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    let Some(mut d) = st.extras.export.take() else {
        return;
    };
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new("Export Materials List")
        .id(egui::Id::new("materials_list_export"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.strong("File Type");
            let bt = d.opts.third_party == ThirdParty::BuilderTrend;
            ui.add_enabled_ui(!bt, |ui| {
                for f in ExportFormat::ALL {
                    ui.radio_value(&mut d.opts.format, f, f.title());
                }
            });
            ui.strong("Options");
            ui.checkbox(&mut d.opts.headers, "Include Column Headers");
            ui.checkbox(&mut d.opts.hidden_columns, "Include Hidden Columns");
            ui.checkbox(&mut d.opts.colors, "Export with Colors (XML and HTML)");
            ui.checkbox(
                &mut d.opts.open_in_editor,
                "Open in Default Spreadsheet Editor",
            );
            ui.strong("Units");
            ui.radio_value(
                &mut d.opts.units,
                UnitsMode::WithAmounts,
                "Include Units with Amounts",
            );
            ui.radio_value(
                &mut d.opts.units,
                UnitsMode::NewColumn,
                "Include Units in a New Column",
            );
            ui.radio_value(&mut d.opts.units, UnitsMode::None, "Do Not Include Units");
            ui.strong("Third Party Formats");
            ui.radio_value(
                &mut d.opts.third_party,
                ThirdParty::NoFormatting,
                "No Formatting",
            );
            ui.radio_value(
                &mut d.opts.third_party,
                ThirdParty::BuilderTrend,
                "BuilderTREND (CSV only)",
            );
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ok {
        let opts = d.opts;
        export_to_file(cx, st, &opts);
    } else if !cancel {
        st.extras.export = Some(d);
    }
}

// ----------------------------------------------------------------- details --

fn details(ctx: &egui::Context, st: &mut State) {
    let Some(d) = st.extras.details.take() else {
        return;
    };
    let mut open = true;
    let mut find: Option<(usize, String)> = None;
    egui::Window::new("Materials List Details")
        .id(egui::Id::new("materials_list_details"))
        .open(&mut open)
        .default_size(egui::vec2(420.0, 420.0))
        .show(ctx, |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("ml_details").striped(true).show(ui, |ui| {
                    let tables: Vec<Vec<(String, String)>> =
                        d.lines.iter().map(list::details).collect();
                    if let Some(first) = tables.first() {
                        for (r, (k, _)) in first.iter().enumerate() {
                            ui.strong(k);
                            for t in &tables {
                                ui.label(t.get(r).map_or("", |x| x.1.as_str()));
                            }
                            ui.end_row();
                        }
                    }
                });
            });
            ui.separator();
            let floors: BTreeSet<usize> = d
                .lines
                .iter()
                .flat_map(|l| l.sources.iter().map(|s| s.floor))
                .collect();
            if ui
                .add_enabled(floors.len() == 1, egui::Button::new("Find"))
                .on_hover_text("Show the object in a plan view")
                .clicked()
            {
                find = d
                    .lines
                    .iter()
                    .find_map(|l| list::find_targets(l).into_iter().next());
            }
            if floors.len() > 1 {
                ui.weak("The objects are on several floors; find them one line at a time.");
            }
        });
    if let Some(f) = find {
        st.find = Some(f);
    }
    if open {
        st.extras.details = Some(d);
    }
}

// ------------------------------------------------------ polyline dialogs --

/// The category x floor grid with Chief's toggle buttons.
fn grid(ui: &mut egui::Ui, d: &mut PolyDialog) {
    let floors = d.floors.len();
    let shown: Vec<usize> = if d.spec.all_floors {
        (0..floors).collect()
    } else {
        vec![d.floor.min(floors.saturating_sub(1))]
    };
    ui.checkbox(&mut d.spec.all_floors, "Include All Floors");
    egui::Grid::new("ml_poly_grid")
        .striped(true)
        .show(ui, |ui| {
            ui.label("");
            for &f in &shown {
                let on = d.sel.floors.contains(&f);
                if ui.selectable_label(on, &d.floors[f]).clicked() && !d.sel.floors.remove(&f) {
                    d.sel.floors.insert(f);
                }
            }
            ui.end_row();
            for c in CATEGORIES {
                let on = d.sel.cats.contains(c);
                if ui.selectable_label(on, c).clicked() && !d.sel.cats.remove(c) {
                    d.sel.cats.insert(c.to_string());
                }
                for &f in &shown {
                    let mut v = d.spec.includes(c, f);
                    if ui.checkbox(&mut v, "").changed() {
                        d.spec.toggle(c, f);
                    }
                }
                ui.end_row();
            }
        });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Toggle Selected").clicked() {
            for c in d.sel.cats.clone() {
                for &f in &shown {
                    if d.sel.floors.is_empty() || d.sel.floors.contains(&f) {
                        d.spec.toggle(&c, f);
                    }
                }
            }
        }
        if ui.button("Toggle Category(s)").clicked() {
            for c in d.sel.cats.clone() {
                d.spec.toggle_category(&c, floors);
            }
        }
        if ui.button("Toggle Floor(s)").clicked() {
            for f in d.sel.floors.clone() {
                d.spec.toggle_floor(f);
            }
        }
        if ui.button("Toggle All").clicked() {
            d.spec.toggle_all(floors);
        }
        if ui.button("Revert All Changes").clicked() {
            d.spec = d.original.clone();
        }
    });
    ui.strong("Included Objects");
    for o in IncludedObjects::ALL {
        ui.radio_value(&mut d.spec.objects, o, o.title());
    }
}

fn polyline_dialog(ctx: &egui::Context, cx: &mut EditorContext, st: &mut State, defaults: bool) {
    let slot = if defaults {
        st.extras.defaults.take()
    } else {
        st.extras.polyline.take()
    };
    let Some(mut d) = slot else { return };
    let title = if defaults {
        "Materials List Polyline Defaults"
    } else {
        "Materials List Polyline Specification"
    };
    let (mut ok, mut cancel, mut calc) = (false, false, false);
    egui::Window::new(title)
        .id(egui::Id::new(("materials_list_poly", defaults)))
        .collapsible(false)
        .default_size(egui::vec2(560.0, 520.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(430.0).show(ui, |ui| {
                if !defaults {
                    ui.horizontal(|ui| {
                        ui.label("Label");
                        ui.text_edit_singleline(&mut d.name);
                    });
                    if let Some((perimeter, area, n)) = d.info {
                        ui.label(format!(
                            "Perimeter {}   Area {:.1} sq ft   Lines {n}",
                            plan_core::units::fmt_ft_in(perimeter),
                            area
                        ));
                    }
                    ui.separator();
                }
                ui.strong("Included Floors / Categories");
                grid(ui, &mut d);
                if !defaults {
                    ui.add_space(6.0);
                    ui.weak("Edit the shape with the CAD polyline handles; its line and fill style are the polyline's own (CAD Specification).");
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if !defaults && ui.button("Calculate Materials List").clicked() {
                    calc = true;
                }
            });
        });
    if ok || calc {
        if defaults {
            cx.begin_change("Materials List Polyline Defaults");
            cx.project.materials.polyline_defaults = Some(d.spec.clone());
            cx.mark_dirty();
        } else if let Some(id) = d.cad_id {
            if d.changed() {
                cx.begin_change("Materials List Polyline Specification");
                if let Some(p) = cx.project.materials.polyline_mut(id) {
                    p.spec = d.spec.clone();
                    p.name = d.name.trim().to_string();
                }
                cx.mark_dirty();
            }
            if calc {
                super::calculate_polyline(cx, st, id);
                st.open = true;
            }
        }
    } else if !cancel {
        if defaults {
            st.extras.defaults = Some(d);
        } else {
            st.extras.polyline = Some(d);
        }
    }
}

/// A polyline record for a new Materials List Polyline on `floor`, with the
/// plan's Materials List Polyline Defaults.
pub fn new_polyline(project: &plan_core::Project, floor: usize, cad_id: Id) -> MaterialsPolyline {
    let spec = project
        .materials
        .polyline_defaults
        .clone()
        .unwrap_or_else(|| PolylineSpec::everything(project.floors.len()));
    let n = project.materials.polylines.len() + 1;
    MaterialsPolyline {
        cad_id,
        floor,
        name: format!("Materials List Polyline {n}"),
        spec,
        holes: Vec::new(),
    }
}

/// Is the Cad object `o` a Materials List Polyline?
pub fn is_polyline(cx: &EditorContext, o: ObjectRef) -> Option<Id> {
    match o {
        ObjectRef::Cad(id) if cx.project.materials.polyline(id).is_some() => Some(id),
        _ => None,
    }
}
