//! The pages of the Import Drawing window.

use super::*;
use eframe::egui::{Align2, Color32, ComboBox, Grid, RichText, ScrollArea, Sense, Ui, Vec2};

enum Nav {
    Back,
    Next,
    Cancel,
}

/// The message window (a DWG file, a file that would not read).
pub(super) fn show_notice(ctx: &egui::Context) {
    let Some(msg) = notice_text() else { return };
    let mut open = true;
    let mut ok = false;
    egui::Window::new("Import Drawing")
        .id(egui::Id::new("import_drawing_notice"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(&msg);
            ui.add_space(6.0);
            ok = ui.button("OK").clicked();
        });
    if ok || !open {
        set_notice(None);
    }
}

fn notice_text() -> Option<String> {
    super::NOTICE.with(|n| n.borrow().clone())
}

fn title(a: &Assistant) -> &'static str {
    match a.page {
        Page::Files => "Import Drawing",
        _ => "Import Drawing Assistant",
    }
}

/// Draws the window; false once it is closed.
pub(super) fn show(a: &mut Assistant, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
    let mut open = true;
    let mut nav: Option<Nav> = None;
    let plan = plan_layers(cx);
    egui::Window::new(title(a))
        .id(egui::Id::new("import_drawing_assistant"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(560.0);
            match a.page {
                Page::Files => files_page(a, ui),
                Page::SelectFile => select_file_page(a, ui),
                Page::SelectLayers => select_layers_page(a, ui, cx),
                Page::LayerMapping => layer_mapping_page(a, ui, &plan),
                Page::AdvancedMapping => advanced_mapping_page(a, ui, &plan),
                Page::DuplicateBlocks => duplicate_page(a, ui, cx),
                Page::AdvancedDuplicates => advanced_duplicate_page(a, ui),
                Page::DrawingUnit => unit_page(a, ui, cx),
                Page::Complete => complete_page(a, ui, cx),
            }
            if !a.message.is_empty() {
                ui.colored_label(Color32::from_rgb(200, 60, 40), &a.message);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(a.page != Page::Files, egui::Button::new("< Back"))
                    .clicked()
                {
                    nav = Some(Nav::Back);
                }
                let label = match a.page {
                    Page::Files => "OK",
                    Page::Complete => "Import",
                    _ => "Next >",
                };
                if ui.button(label).clicked() {
                    nav = Some(Nav::Next);
                }
                if ui.button("Cancel").clicked() {
                    nav = Some(Nav::Cancel);
                }
            });
        });
    if !open {
        return false;
    }
    match nav {
        Some(Nav::Back) => {
            go_back(a);
            true
        }
        Some(Nav::Next) => go_next(a, cx),
        Some(Nav::Cancel) => false,
        None => true,
    }
}

fn go_back(a: &mut Assistant) {
    if let Some(p) = a.trail.pop() {
        a.page = p;
    }
    a.message.clear();
}

fn goto(a: &mut Assistant, p: Page) {
    a.trail.push(a.page);
    a.page = p;
    a.message.clear();
}

/// The Next button; false when the window is done.
pub(super) fn go_next(a: &mut Assistant, cx: &mut EditorContext) -> bool {
    match a.page {
        Page::Files => {
            if a.show_assistant {
                goto(a, Page::SelectFile);
                true
            } else {
                let r = a.import_all(cx);
                cx.status = status_line(&r);
                false
            }
        }
        Page::SelectFile => {
            // Paper space adds objects to the layers: read the tables again.
            if a.paper_space != a.loaded_paper {
                a.load_file(cx);
            }
            goto(a, Page::SelectLayers);
            true
        }
        Page::SelectLayers => {
            if !a.rows.iter().any(|r| r.include) {
                a.message = "Check at least one layer to import".into();
                return true;
            }
            goto(a, Page::LayerMapping);
            true
        }
        Page::LayerMapping => {
            if a.mode == MappingMode::Advanced {
                goto(a, Page::AdvancedMapping);
            } else {
                after_mapping(a, cx);
            }
            true
        }
        Page::AdvancedMapping => {
            after_mapping(a, cx);
            true
        }
        Page::DuplicateBlocks => {
            if a.dup_each {
                goto(a, Page::AdvancedDuplicates);
            } else {
                goto(a, Page::DrawingUnit);
            }
            true
        }
        Page::AdvancedDuplicates => {
            goto(a, Page::DrawingUnit);
            true
        }
        Page::DrawingUnit => {
            a.refresh_preview(cx);
            goto(a, Page::Complete);
            true
        }
        Page::Complete => {
            let r = a.import_all(cx);
            cx.status = status_line(&r);
            if a.show_each && a.cur + 1 < a.files.len() {
                a.cur += 1;
                a.load_file(cx);
                a.trail.clear();
                a.page = Page::SelectFile;
                a.message = format!("Imported. Next file: {}", a.file_name());
                return true;
            }
            false
        }
    }
}

fn after_mapping(a: &mut Assistant, cx: &EditorContext) {
    let dups = a.duplicate_blocks(cx);
    if dups.is_empty() {
        goto(a, Page::DrawingUnit);
        return;
    }
    a.dup_names = dups
        .into_iter()
        .map(|n| {
            let c = a
                .dup_names
                .iter()
                .find(|(m, _)| *m == n)
                .map_or(BlockConflict::AutoName, |(_, c)| *c);
            (n, c)
        })
        .collect();
    goto(a, Page::DuplicateBlocks);
}

// ----- pages -----

fn files_page(a: &mut Assistant, ui: &mut Ui) {
    ui.strong("Files Selected for Import");
    ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
        for f in &a.files {
            ui.label(format!(
                "{}  ({} objects, {} layers)",
                f.name,
                f.drawing.entities.len(),
                f.drawing.layers.len()
            ));
        }
    });
    ui.separator();
    ui.checkbox(&mut a.show_assistant, "Show Import Assistant");
    ui.add_enabled_ui(a.show_assistant && a.files.len() > 1, |ui| {
        ui.checkbox(&mut a.show_each, "Show For Each File");
    });
    ui.checkbox(&mut a.create_blocks, "Create CAD Blocks")
        .on_hover_text("Each imported drawing becomes one CAD block");
    ui.add_enabled_ui(a.files.len() > 1, |ui| {
        ui.checkbox(&mut a.auto_position, "Auto Position Blocks")
            .on_hover_text("Several drawings sit side by side near the origin");
    });
}

fn select_file_page(a: &mut Assistant, ui: &mut Ui) {
    let d = a.drawing();
    ui.strong(a.file_name());
    ui.label(format!(
        "{} objects, {} layers, {} blocks. Format: {}{}. Units in the file: {}.",
        d.entities.len(),
        d.layers.len(),
        d.blocks.values().filter(|b| !b.anonymous).count(),
        match d.format {
            plan_import::dxf::DxfFormat::Ascii => "ASCII DXF",
            plan_import::dxf::DxfFormat::Binary => "binary DXF",
        },
        if d.version.is_empty() {
            String::new()
        } else {
            format!(" ({})", plan_import::dxf::release_name(&d.version))
        },
        match d.units {
            DxfUnits::Unitless => "not stated".to_string(),
            u => u.label().to_string(),
        }
    ));
    if !d.xrefs.is_empty() {
        ui.colored_label(
            Color32::from_rgb(200, 120, 20),
            format!(
                "External references are not imported; their files are missing from this import: {}",
                d.xrefs.join(", ")
            ),
        );
    }
    ui.separator();
    ui.label("Convert lines with shared end points to:");
    ui.checkbox(&mut a.join_lines, "Polylines");
    ui.checkbox(&mut a.boxes, "Boxes");
    ui.weak("Polylines already in the file are not affected.");
    ui.separator();
    ui.checkbox(&mut a.import_hatch, "Import Hatch entities");
    ui.checkbox(&mut a.paper_space, "Include the first page of paper space")
        .on_hover_text("Imported as one CAD block; later pages are not recognized");
}

fn select_layers_page(a: &mut Assistant, ui: &mut Ui, cx: &EditorContext) {
    ui.strong("Select Layers");
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            a.rows.iter_mut().for_each(|r| r.include = true);
        }
        if ui.button("Clear All").clicked() {
            a.rows.iter_mut().for_each(|r| r.include = false);
        }
    });
    ScrollArea::vertical()
        .max_height(260.0)
        .id_salt("import_layers_scroll")
        .show(ui, |ui| {
            Grid::new("import_layers_grid")
                .num_columns(8)
                .spacing([10.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "",
                        "Layer",
                        "Color",
                        "Status",
                        "Line type",
                        "Weight",
                        "Objects",
                        "To walls",
                    ] {
                        ui.weak(h);
                    }
                    ui.end_row();
                    for r in a.rows.iter_mut() {
                        ui.checkbox(&mut r.include, "");
                        ui.label(&r.name);
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, swatch(r.color));
                        ui.label(if r.frozen {
                            "Frozen"
                        } else if r.on {
                            "Visible"
                        } else {
                            "Off"
                        });
                        ui.label(&r.linetype);
                        ui.label(if r.weight > 0 {
                            format!("{:.2} mm", f64::from(r.weight) / 100.0)
                        } else {
                            "Default".into()
                        });
                        ui.label(r.objects.to_string());
                        ui.add_enabled(r.include, egui::Checkbox::without_text(&mut r.walls));
                        ui.end_row();
                    }
                });
        });
    if a.rows.iter().any(|r| r.walls) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Walls from the checked layers' lines: wall type");
            let types: Vec<String> = cx.wall_types().iter().map(|t| t.name.clone()).collect();
            ComboBox::from_id_salt("import_wall_type")
                .selected_text(if a.wall_type.is_empty() {
                    "Default for its kind".to_string()
                } else {
                    a.wall_type.clone()
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut a.wall_type, String::new(), "Default for its kind");
                    for t in &types {
                        ui.selectable_value(&mut a.wall_type, t.clone(), t);
                    }
                });
        });
        ui.weak("Pairs of parallel lines become walls (CAD to Walls); the lines stay as CAD.");
    }
}

fn layer_mapping_page(a: &mut Assistant, ui: &mut Ui, plan: &[String]) {
    ui.strong("Layer Mapping");
    ui.radio_value(
        &mut a.mode,
        MappingMode::Single,
        "A single layer in Plan Studio",
    );
    ui.add_enabled_ui(a.mode == MappingMode::Single, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ComboBox::from_id_salt("import_single_layer")
                .selected_text(a.single_layer.clone())
                .show_ui(ui, |ui| {
                    for p in plan {
                        ui.selectable_value(&mut a.single_layer, p.clone(), p);
                    }
                });
            ui.label("or a new one:");
            ui.add(egui::TextEdit::singleline(&mut a.single_layer).desired_width(140.0));
        });
        ui.weak("Original layer attributes are lost; each object keeps its color, line style and weight.");
    });
    ui.radio_value(
        &mut a.mode,
        MappingMode::SameName,
        "Layers of the same names",
    );
    ui.add_enabled_ui(a.mode == MappingMode::SameName, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.checkbox(
                &mut a.layer_attrs,
                "Import the attributes of each layer (color, line style, weight)",
            );
        });
    });
    ui.radio_value(&mut a.mode, MappingMode::Advanced, "Advanced layer mapping");
}

fn advanced_mapping_page(a: &mut Assistant, ui: &mut Ui, plan: &[String]) {
    ui.strong("Advanced Layer Mapping");
    ScrollArea::vertical()
        .max_height(300.0)
        .id_salt("import_adv_scroll")
        .show(ui, |ui| {
            Grid::new("import_adv_grid")
                .num_columns(3)
                .spacing([12.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.weak("DXF layer");
                    ui.weak("Plan Studio layer");
                    ui.weak("");
                    ui.end_row();
                    let included: Vec<bool> = a
                        .advanced
                        .iter()
                        .map(|(n, _)| a.rows.iter().any(|r| r.name == *n && r.include))
                        .collect();
                    for (i, (name, choice)) in a.advanced.iter_mut().enumerate() {
                        if !included[i] {
                            continue;
                        }
                        ui.label(name.as_str());
                        let label = match choice {
                            LayerChoice::Same => format!("{name} (same name)"),
                            LayerChoice::Plan(n) => n.clone(),
                            LayerChoice::Named(_) => "New layer named...".into(),
                        };
                        ui.horizontal(|ui| {
                            ComboBox::from_id_salt(("import_adv_to", i))
                                .selected_text(label)
                                .width(200.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        choice,
                                        LayerChoice::Same,
                                        format!("{name} (same name)"),
                                    );
                                    for p in plan {
                                        ui.selectable_value(
                                            choice,
                                            LayerChoice::Plan(p.clone()),
                                            p,
                                        );
                                    }
                                    if ui
                                        .selectable_label(
                                            matches!(choice, LayerChoice::Named(_)),
                                            "New layer named...",
                                        )
                                        .clicked()
                                        && !matches!(choice, LayerChoice::Named(_))
                                    {
                                        *choice = LayerChoice::Named(name.clone());
                                    }
                                });
                            if let LayerChoice::Named(n) = choice {
                                ui.add(egui::TextEdit::singleline(n).desired_width(120.0));
                            }
                        });
                        let is_new = match choice {
                            LayerChoice::Plan(_) => false,
                            LayerChoice::Same => !plan.iter().any(|p| p.eq_ignore_ascii_case(name)),
                            LayerChoice::Named(n) => !plan.iter().any(|p| p == n),
                        };
                        ui.label(if is_new { "New" } else { "" });
                        ui.end_row();
                    }
                });
        });
    ui.checkbox(&mut a.layer_attrs, "Import the attributes of new layers");
}

fn conflict_label(c: BlockConflict) -> &'static str {
    match c {
        BlockConflict::AutoName => "Auto Name",
        BlockConflict::Replace => "Replace",
        BlockConflict::UseExisting => "Use Existing",
    }
}

fn duplicate_page(a: &mut Assistant, ui: &mut Ui, _cx: &EditorContext) {
    ui.strong("Duplicate CAD Blocks");
    ui.label(format!(
        "These blocks are already on this floor: {}",
        a.dup_names
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for (v, text) in [
        (
            BlockConflict::AutoName,
            "Give each duplicate a unique name (name_Copy_1)",
        ),
        (
            BlockConflict::Replace,
            "Replace the blocks on the floor with the ones being imported",
        ),
        (
            BlockConflict::UseExisting,
            "Keep the blocks on the floor and discard the imported ones",
        ),
    ] {
        if ui.radio_value(&mut a.dup_default, v, text).clicked() {
            a.dup_each = false;
        }
    }
    if ui
        .radio(a.dup_each, "Manage each duplicate individually")
        .clicked()
    {
        a.dup_each = true;
    }
}

fn advanced_duplicate_page(a: &mut Assistant, ui: &mut Ui) {
    ui.strong("Advanced Duplicate CAD Blocks");
    Grid::new("import_dup_grid")
        .num_columns(2)
        .spacing([14.0, 5.0])
        .show(ui, |ui| {
            for (i, (name, c)) in a.dup_names.iter_mut().enumerate() {
                ui.label(name.as_str());
                ComboBox::from_id_salt(("import_dup", i))
                    .selected_text(conflict_label(*c))
                    .show_ui(ui, |ui| {
                        for v in [
                            BlockConflict::AutoName,
                            BlockConflict::Replace,
                            BlockConflict::UseExisting,
                        ] {
                            ui.selectable_value(c, v, conflict_label(v));
                        }
                    });
                ui.end_row();
            }
        });
}

fn unit_page(a: &mut Assistant, ui: &mut Ui, cx: &EditorContext) {
    ui.strong("Drawing Unit");
    let label = a.units_label();
    Grid::new("import_unit_grid")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Unit");
            let file_units = default_units(a.drawing());
            ComboBox::from_id_salt("import_units")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut a.units,
                        None,
                        format!("As the file says ({})", file_units.label()),
                    );
                    for u in DxfUnits::CHOICES {
                        ui.selectable_value(&mut a.units, Some(u), u.label());
                    }
                });
            ui.end_row();
            ui.label("Scale");
            ui.add(
                egui::DragValue::new(&mut a.scale)
                    .speed(0.01)
                    .range(0.001..=1000.0)
                    .max_decimals(6),
            )
            .on_hover_text("A drawing made at another scale than 1:1: multiply its size");
            ui.end_row();
            ui.label("Rotation");
            ui.add(
                egui::DragValue::new(&mut a.rotation_deg)
                    .speed(0.5)
                    .range(-360.0..=360.0)
                    .suffix("\u{b0}"),
            );
            ui.end_row();
            ui.label("Place it on");
            let names: Vec<String> = cx.project.floors.iter().map(|f| f.name.clone()).collect();
            a.target_floor = a.target_floor.min(names.len().saturating_sub(1));
            ComboBox::from_id_salt("import_target_floor")
                .selected_text(names.get(a.target_floor).cloned().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for (i, n) in names.iter().enumerate() {
                        ui.selectable_value(&mut a.target_floor, i, n);
                    }
                });
            ui.end_row();
        });
    ui.separator();
    ui.label("Dimensions");
    ui.radio_value(
        &mut a.dims,
        DimensionMode::Objects,
        "Import as dimensions where possible",
    );
    ui.radio_value(&mut a.dims, DimensionMode::Blocks, "Import as CAD blocks");
    ui.separator();
    ui.checkbox(&mut a.to_origin, "Move drawing to the origin");
    ui.horizontal(|ui| {
        ui.label("Place it at   x");
        ui.add(egui::TextEdit::singleline(&mut a.insert_x).desired_width(70.0));
        ui.label("y");
        ui.add(egui::TextEdit::singleline(&mut a.insert_y).desired_width(70.0));
    });
    if !a.to_origin {
        ui.weak("Unchecked: the drawing goes where it was drawn.");
    }
}

fn complete_page(a: &mut Assistant, ui: &mut Ui, cx: &mut EditorContext) {
    if a.preview.is_none() {
        a.refresh_preview(cx);
    }
    ui.strong("Import Complete");
    let floor_name = cx
        .project
        .floors
        .get(a.target_floor)
        .map_or_else(String::new, |f| f.name.clone());
    ui.label(format!("{} goes onto {}.", a.file_name(), floor_name));
    if let Some(p) = a.preview.clone() {
        let s = p.summary;
        ui.label(format!(
            "{} objects: {} lines, {} polylines, {} arcs and circles, {} texts, {} hatches; {} dimensions; {} CAD blocks; {} layers.",
            s.objects, s.lines, s.polylines, s.arcs_circles, s.texts, s.hatches, s.dimensions, s.blocks, s.layers
        ));
        if let Some((w, h)) = p.size {
            ui.label(format!(
                "Size in the plan: {} x {}",
                cx.fmt_dim(w),
                cx.fmt_dim(h)
            ));
        }
        if p.walls > 0 {
            ui.label(format!(
                "{} walls will be made from the lines of the layers marked To walls.",
                p.walls
            ));
        }
        for n in &p.notes {
            ui.add(egui::Label::new(RichText::new(n).weak()).wrap());
        }
        if s.objects + s.dimensions == 0 {
            ui.colored_label(
                Color32::from_rgb(200, 60, 40),
                "Nothing would be imported: check the layers.",
            );
        }
    }
    ui.weak("The drawing's components are selected afterward, so you can move it.");
    if ui.button("Refresh").clicked() {
        a.refresh_preview(cx);
    }
}
