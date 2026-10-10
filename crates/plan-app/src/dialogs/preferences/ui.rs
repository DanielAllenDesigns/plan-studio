//! The Preferences pages that read and write [`PagePrefs`]: what each page
//! shows, and the Reset Options commands (which tests run without a window).

use super::pages::{
    self, EndCap, FolderKind, IconSize, PagePrefs, PreviewQuality, PreviewSize, ResizeAbout,
    RotateAbout,
};
use super::Preferences;
use crate::editor::EditorContext;
use crate::theme::{AppSettings, CanvasTheme, DockWidths};
use crate::toolbar::{config, Action};
use crate::tools::select::MarqueeMode;
use eframe::egui;
use plan_core::defaults::EditBehavior;
use plan_core::units::{format_length, parse_length, LengthFormat, LengthUnit};
use std::path::Path;

const GOOD: egui::Color32 = egui::Color32::from_rgb(0x4C, 0xAF, 0x50);
const BAD: egui::Color32 = egui::Color32::from_rgb(0xE5, 0x73, 0x73);

/// Runs `f` on the live page preferences and keeps the result when it changed.
fn edit_pages(f: impl FnOnce(&mut PagePrefs)) {
    let mut p = pages::current();
    let before = p.clone();
    f(&mut p);
    if p != before {
        pages::set(p);
    }
}

/// Runs `f` on the plan's editing defaults; a change is marked on the plan
/// and kept as the saved editing defaults.
fn edit_editing(cx: &mut EditorContext, f: impl FnOnce(&mut plan_core::defaults::EditingDefaults)) {
    let before = cx.defaults.editing.clone();
    f(&mut cx.defaults.editing);
    if cx.defaults.editing != before {
        cx.mark_dirty();
        pages::remember_editing(&cx.defaults.editing);
    }
}

/// A "use my own color" row: a switch, and the color when it is on.
fn color_row(ui: &mut egui::Ui, label: &str, slot: &mut Option<[u8; 3]>, theme: egui::Color32) {
    ui.horizontal(|ui| {
        let mut custom = slot.is_some();
        if ui.checkbox(&mut custom, label).changed() {
            *slot = custom.then(|| [theme.r(), theme.g(), theme.b()]);
        }
        if let Some(c) = slot {
            ui.color_edit_button_srgb(c);
        } else {
            ui.weak("theme color");
        }
    });
}

// ----- Appearance -----

/// Appearance: the canvas colors over the theme, the low-glare switches and
/// the icon size.
pub(super) fn appearance_extra(ui: &mut egui::Ui, p: &mut Preferences, settings: &mut AppSettings) {
    ui.add_space(8.0);
    ui.strong("Canvas colors");
    let palette = settings.theme.palette();
    let theme_sel = palette.selection;
    let mut custom_sel = p.selection_color;
    color_row(ui, "Selection", &mut custom_sel, theme_sel);
    p.selection_color = custom_sel;
    edit_pages(|pg| {
        let a = &mut pg.appearance;
        color_row(ui, "Background", &mut a.background, palette.background);
        color_row(ui, "Grid", &mut a.grid, palette.grid_major);
        color_row(ui, "Text", &mut a.text, palette.text);
        color_row(
            ui,
            "Temporary dimensions",
            &mut a.temp_dim,
            palette.dimension_text,
        );
        if ui.button("Use the theme's colors").clicked() {
            a.background = None;
            a.grid = None;
            a.text = None;
            a.temp_dim = None;
        }
    });
    ui.add_space(8.0);
    ui.strong("Low glare");
    let mut canvas = settings.theme == CanvasTheme::LowGlare;
    if ui
        .checkbox(&mut canvas, "Low-glare canvas (soft paper tones)")
        .changed()
    {
        settings.theme = if canvas {
            CanvasTheme::LowGlare
        } else {
            CanvasTheme::Paper
        };
    }
    let mut dim = settings.brightness <= LOW_GLARE_BRIGHTNESS + 0.001;
    if ui
        .checkbox(&mut dim, "Low-glare interface (dimmer toolbars and panels)")
        .changed()
    {
        settings.brightness = if dim { LOW_GLARE_BRIGHTNESS } else { 1.0 };
    }
    ui.add_space(8.0);
    edit_pages(|pg| {
        ui.horizontal(|ui| {
            ui.label("Icon size");
            egui::ComboBox::from_id_salt("prefs_icon_size")
                .selected_text(pg.appearance.icon_size.label())
                .show_ui(ui, |ui| {
                    for s in IconSize::ALL {
                        ui.selectable_value(&mut pg.appearance.icon_size, s, s.label());
                    }
                });
        });
    });
}

/// The UI brightness the low-glare interface switch sets.
pub const LOW_GLARE_BRIGHTNESS: f32 = 0.8;

// ----- Text -----

pub(super) fn text_links(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    ui.add_space(8.0);
    ui.strong("Default text");
    ui.weak("The font, size and style of new text come from the plan's text styles.");
    if ui.button("Default Font Settings...").clicked() {
        actions.push(Action::DefaultSettings);
    }
}

// ----- Library Browser -----

pub(super) fn library_browser(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.strong("Browser");
    edit_pages(|pg| {
        let b = &mut pg.library_browser;
        ui.horizontal(|ui| {
            ui.label("Preview size");
            egui::ComboBox::from_id_salt("prefs_preview_size")
                .selected_text(b.preview_size.label())
                .show_ui(ui, |ui| {
                    for s in PreviewSize::ALL {
                        ui.selectable_value(&mut b.preview_size, s, s.label());
                    }
                });
        });
        ui.add_space(4.0);
        ui.strong("Search looks at");
        ui.checkbox(&mut b.search_names, "Names");
        ui.checkbox(&mut b.search_descriptions, "Descriptions");
        ui.checkbox(&mut b.search_keywords, "Keywords");
        ui.checkbox(&mut b.search_catalog_names, "Catalog names");
        ui.checkbox(&mut b.whole_words, "Whole words only");
        ui.checkbox(&mut b.match_all_words, "Every word typed must match");
    });
}

// ----- Render -----

pub(super) fn render(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.strong("3D preview");
    edit_pages(|pg| {
        let r = &mut pg.render;
        ui.horizontal(|ui| {
            ui.label("Preview quality");
            egui::ComboBox::from_id_salt("prefs_preview_quality")
                .selected_text(r.preview_quality.label())
                .show_ui(ui, |ui| {
                    for q in PreviewQuality::ALL {
                        ui.selectable_value(&mut r.preview_quality, q, q.label());
                    }
                });
        });
        ui.checkbox(&mut r.shadows, "Shadows in new 3D views");
        ui.checkbox(
            &mut r.ambient_occlusion,
            "Ambient occlusion in new 3D views",
        );
    });
    ui.weak(
        "A 3D view opened from now on starts with these; the Shading menu changes the view itself.",
    );
    ui.add_space(8.0);
    ui.strong("Material packages (Lightbeans)");
    edit_pages(|pg| {
        let r = &mut pg.render;
        ui.checkbox(
            &mut r.pbr_maps,
            "PBR maps: draw normal, roughness, metallic, occlusion and opacity maps",
        );
        ui.horizontal(|ui| {
            ui.label("Max texture size");
            egui::ComboBox::from_id_salt("prefs_pbr_max_texture")
                .selected_text(format!("{} px", r.max_texture_side))
                .show_ui(ui, |ui| {
                    for side in [1024_u32, 2048, 4096, 8192] {
                        ui.selectable_value(&mut r.max_texture_side, side, format!("{side} px"));
                    }
                });
        });
        ui.checkbox(
            &mut r.watch_downloads,
            "Watch Downloads folder for new material packages",
        );
    });
    ui.weak(
        "Larger maps are shrunk for the 3D view; the ray tracer keeps up to 4096 px. Packages are never downloaded by Plan Studio.",
    );
}

// ----- Materials List -----

pub(super) fn materials(ui: &mut egui::Ui) {
    edit_pages(|pg| {
        let m = &mut pg.materials;
        ui.checkbox(&mut m.apply_waste, "Add each category's waste factor");
        ui.checkbox(
            &mut m.round_up,
            "Round the quantity to buy up to whole units",
        );
        ui.checkbox(&mut m.show_prices, "Show prices and totals");
        ui.checkbox(
            &mut m.all_floors,
            "List every floor, not only the active one",
        );
    });
    ui.add_space(6.0);
    match crate::tools::materials::user_library_path() {
        Some(p) => ui.weak(format!("Material library: {}", p.display())),
        None => ui.weak("Material library: none"),
    };
    ui.weak("Waste factors and prices are edited in the Materials List window.");
}

// ----- Folders -----

fn browse_folder(title: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .pick_folder()
        .map(|d| d.to_string_lossy().into_owned())
}

pub(super) fn folders(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    ui.label("Chief's data folders. Plan Studio reads these at run time; nothing is copied.");
    ui.add_space(4.0);
    let home = crate::paths::home_dir();
    let library = super::chief_folder();
    let mut pg = pages::current();
    let before = pg.folders.clone();
    let mut library_new: Option<String> = None;
    egui::Grid::new("prefs_folders")
        .num_columns(4)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            for kind in FolderKind::ALL {
                let st =
                    pages::folder_status(&pg.folders, library.as_deref(), kind, home.as_deref());
                ui.label(kind.label()).on_hover_text(kind.hint());
                let mut text = if kind == FolderKind::Library {
                    library
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    pg.folders.get(kind).unwrap_or_default().to_string()
                };
                let hint = st
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "next to each plan".into());
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .hint_text(hint)
                        .desired_width(300.0),
                );
                let mut changed = edit.lost_focus() && edit.changed();
                ui.horizontal(|ui| {
                    if ui.button("Browse...").clicked() {
                        if let Some(d) = browse_folder(kind.label()) {
                            text = d;
                            changed = true;
                        }
                    }
                    if ui
                        .add_enabled(st.custom, egui::Button::new("Default"))
                        .clicked()
                    {
                        text.clear();
                        changed = true;
                    }
                });
                if st.exists {
                    ui.colored_label(
                        GOOD,
                        if st.path.is_some() {
                            "\u{2713} found"
                        } else {
                            "\u{2713}"
                        },
                    );
                } else {
                    ui.colored_label(BAD, "\u{2717} not found");
                }
                ui.end_row();
                if changed {
                    if kind == FolderKind::Library {
                        library_new = Some(text);
                    } else {
                        pg.folders.set(kind, &text);
                    }
                }
            }
        });
    if let Some(text) = library_new {
        let _ = super::set_chief_folder(&text);
    }
    if pg.folders != before {
        edit_pages(|p| p.folders = pg.folders.clone());
    }
    ui.add_space(8.0);
    ui.strong("Templates and files");
    let t = crate::templates::load_settings();
    let show = |ui: &mut egui::Ui, label: &str, p: Option<&Path>| {
        ui.horizontal(|ui| {
            ui.label(label);
            match p {
                Some(p) => ui.weak(p.display().to_string()),
                None => ui.weak("none"),
            };
        });
    };
    show(ui, "Default plan template", t.plan.as_deref());
    show(ui, "Default layout template", t.layout.as_deref());
    show(
        ui,
        "Plan Studio files",
        crate::paths::user_file("").as_deref(),
    );
    show(
        ui,
        "Material library",
        crate::tools::materials::user_library_path().as_deref(),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Templates...").clicked() {
            crate::dialogs::exchange::open_templates_page();
        }
        if ui.button("Default Settings...").clicked() {
            actions.push(Action::DefaultSettings);
        }
    });
}

// ----- Edit -----

pub(super) fn edit(ui: &mut egui::Ui, cx: &mut EditorContext, actions: &mut Vec<Action>) {
    edit_pages(|pg| {
        ui.horizontal(|ui| {
            ui.label("Rotate about");
            egui::ComboBox::from_id_salt("prefs_rotate_about")
                .selected_text(pg.edit.rotate_about.label())
                .show_ui(ui, |ui| {
                    for a in RotateAbout::ALL {
                        ui.selectable_value(&mut pg.edit.rotate_about, a, a.label());
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Resize about");
            egui::ComboBox::from_id_salt("prefs_resize_about")
                .selected_text(pg.edit.resize_about.label())
                .show_ui(ui, |ui| {
                    for a in ResizeAbout::ALL {
                        ui.selectable_value(&mut pg.edit.resize_about, a, a.label());
                    }
                });
        });
        ui.add_space(4.0);
        ui.strong("Marquee selection");
        let mut m = pg.edit.marquee_mode();
        for mode in MarqueeMode::ALL {
            ui.radio_value(&mut m, mode, mode.label());
        }
        pg.edit.set_marquee_mode(m);
    });
    ui.add_space(8.0);
    ui.strong("Snaps");
    edit_editing(cx, |e| {
        ui.checkbox(&mut e.object_snaps, "Object snaps");
        ui.checkbox(&mut e.grid_snaps, "Grid snaps");
        ui.checkbox(&mut e.angle_snaps, "Angle snaps");
        ui.checkbox(&mut e.bumping, "Bumping");
    });
    ui.weak("Each snap kind and the sensitivity are on the Snap Properties page.");
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Default Settings...").clicked() {
            actions.push(Action::DefaultSettings);
        }
        if ui.button("Customize Hotkeys...").clicked() {
            actions.push(Action::OpenHotkeyDialog);
        }
    });
}

// ----- Behaviors -----

pub(super) fn behaviors(ui: &mut egui::Ui, cx: &mut EditorContext) {
    edit_editing(cx, |e| {
        let b = &mut e.behavior;
        ui.horizontal(|ui| {
            ui.label("Edit Type");
            egui::ComboBox::from_id_salt("prefs_edit_type")
                .selected_text(b.mode.label())
                .show_ui(ui, |ui| {
                    for m in EditBehavior::ALL {
                        ui.selectable_value(&mut b.mode, m, m.label());
                    }
                });
        });
        ui.checkbox(&mut b.resize_proportional, "Resize keeps proportions");
        ui.checkbox(
            &mut b.alternate_lock_axis,
            "Alternate locks the move to one axis",
        );
        ui.horizontal(|ui| {
            ui.label("Concentric");
            ui.add(
                egui::DragValue::new(&mut b.concentric_distance)
                    .speed(0.25)
                    .suffix("\""),
            );
            ui.add(
                egui::DragValue::new(&mut b.concentric_copies)
                    .range(1..=50)
                    .suffix(" copies"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Fillet radius");
            ui.add(
                egui::DragValue::new(&mut b.fillet_radius)
                    .speed(0.25)
                    .suffix("\""),
            );
            ui.label("Chamfer");
            ui.add(
                egui::DragValue::new(&mut b.chamfer_distance)
                    .speed(0.25)
                    .suffix("\""),
            );
        });
        ui.strong("Replicate");
        ui.horizontal(|ui| {
            ui.label("Copies");
            ui.add(egui::DragValue::new(&mut b.replicate_copies).range(1..=200));
        });
        ui.checkbox(
            &mut b.replicate_dialog,
            "Open the Replicate dialog after the drag",
        );
    });
    ui.add_space(8.0);
    ui.strong("Camera steps");
    edit_pages(|pg| {
        let c = &mut pg.behaviors;
        ui.horizontal(|ui| {
            ui.label("Move");
            ui.add(
                egui::DragValue::new(&mut c.camera_move_in)
                    .range(1.0..=240.0)
                    .speed(1.0)
                    .suffix("\""),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Turn / orbit");
            ui.add(
                egui::DragValue::new(&mut c.camera_turn_deg)
                    .range(1.0..=90.0)
                    .speed(0.5)
                    .suffix("\u{b0}"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Tilt");
            ui.add(
                egui::DragValue::new(&mut c.camera_tilt_deg)
                    .range(1.0..=45.0)
                    .speed(0.5)
                    .suffix("\u{b0}"),
            );
        });
    });
}

// ----- Snap Properties -----

/// Allowed drawing angles as the page types them: `0, 45, 90`.
pub fn angles_text(angles: &[f64]) -> String {
    angles
        .iter()
        .map(|a| format!("{a}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parses [`angles_text`]'s format; unreadable parts are dropped, the rest is
/// kept in 0..360 without repeats.
pub fn parse_angles(text: &str) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for part in text.split([',', ';', ' ']) {
        if let Ok(v) = part.trim().trim_end_matches('\u{b0}').parse::<f64>() {
            let v = v.rem_euclid(360.0);
            if v.is_finite() && !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

pub(super) fn snaps(ui: &mut egui::Ui, cx: &mut EditorContext) {
    edit_editing(cx, |e| {
        ui.checkbox(&mut e.object_snaps, "Object snaps");
        ui.add_enabled_ui(e.object_snaps, |ui| {
            ui.indent("prefs_object_snaps", |ui| {
                ui.checkbox(&mut e.snap_endpoint, "Endpoint");
                ui.checkbox(&mut e.snap_midpoint, "Midpoint");
                ui.checkbox(&mut e.snap_intersection, "Intersection");
                ui.checkbox(&mut e.snap_perpendicular, "Perpendicular");
                ui.checkbox(&mut e.snap_on_object, "On object");
                ui.checkbox(&mut e.snap_center, "Center");
                ui.checkbox(&mut e.snap_quadrant, "Quadrant");
                ui.checkbox(&mut e.snap_tangent, "Tangent");
                ui.checkbox(&mut e.snap_extension, "Extension");
                ui.checkbox(&mut e.snap_markers, "CAD points and markers");
            });
        });
        ui.checkbox(&mut e.grid_snaps, "Grid snaps");
        ui.checkbox(&mut e.angle_snaps, "Angle snaps");
        ui.horizontal(|ui| {
            ui.label("Angle increment");
            egui::ComboBox::from_id_salt("prefs_angle_snap")
                .selected_text(format!("{}\u{b0}", e.angle_snap_deg))
                .show_ui(ui, |ui| {
                    for a in [0.0, 15.0, 30.0, 45.0, 90.0] {
                        ui.selectable_value(&mut e.angle_snap_deg, a, format!("{a}\u{b0}"));
                    }
                });
        });
        let id = egui::Id::new("prefs_snap_angles_text");
        let mut text = ui
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| angles_text(&e.snap_angles));
        ui.horizontal(|ui| {
            ui.label("Allowed angles");
            let r = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text("every multiple of the increment")
                    .desired_width(180.0),
            );
            if r.changed() {
                e.snap_angles = parse_angles(&text);
            }
            if r.lost_focus() {
                text = angles_text(&e.snap_angles);
            }
        });
        ui.data_mut(|d| d.insert_temp(id, text));
        ui.horizontal(|ui| {
            ui.label("Sensitivity");
            ui.add(
                egui::DragValue::new(&mut e.snap_distance_px)
                    .range(1.0..=40.0)
                    .suffix(" px"),
            );
        });
        ui.checkbox(&mut e.bumping, "Bumping");
        ui.add_enabled_ui(e.bumping, |ui| {
            ui.horizontal(|ui| {
                ui.label("Bumping distance");
                ui.add(
                    egui::DragValue::new(&mut e.bumping_distance)
                        .range(0.0..=60.0)
                        .speed(0.25)
                        .suffix("\""),
                );
            });
        });
    });
    ui.weak("These are kept with My Template and for every plan you open.");
}

// ----- Architectural -----

pub(super) fn architectural(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.strong("Auto rebuild");
    edit_pages(|pg| {
        let a = &mut pg.architectural;
        ui.checkbox(&mut a.auto_rebuild_roofs, "Auto Rebuild Roofs");
        ui.checkbox(&mut a.auto_rebuild_walls, "Auto Rebuild Walls");
        ui.checkbox(&mut a.auto_rebuild_foundations, "Auto Rebuild Foundations");
        ui.checkbox(&mut a.auto_rebuild_attic_walls, "Auto Rebuild Attic Walls");
        ui.checkbox(
            &mut a.delete_unused_roof_planes,
            "Delete Unused Roof Planes",
        );
        ui.add_space(6.0);
        ui.strong("Code minimums");
        ui.checkbox(
            &mut a.seed_code_defaults,
            "Seed defaults from code minimums",
        );
        ui.checkbox(
            &mut a.check_while_drawing,
            "Plan Check: check while drawing",
        );
    });
    ui.weak("An automatic rebuild runs when the walls it depends on change; Build > Roof > Rebuild does it by hand.");
}

// ----- CAD -----

pub(super) fn cad(ui: &mut egui::Ui) {
    edit_pages(|pg| {
        let c = &mut pg.cad;
        ui.checkbox(
            &mut c.show_arc_centers,
            "Show the center of a selected arc or circle",
        );
        ui.horizontal(|ui| {
            ui.label("Line end caps");
            egui::ComboBox::from_id_salt("prefs_end_caps")
                .selected_text(c.end_caps.label())
                .show_ui(ui, |ui| {
                    for e in EndCap::ALL {
                        ui.selectable_value(&mut c.end_caps, e, e.label());
                    }
                });
        });
        ui.add_space(6.0);
        ui.strong("Line weights");
        egui::Grid::new("prefs_line_weights")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (name, w) in c.line_weights.iter_mut() {
                    ui.label(name.as_str());
                    ui.add(egui::DragValue::new(w).range(0..=200));
                    ui.end_row();
                }
            });
        if ui.button("Reset line weights").clicked() {
            c.line_weights = pages::CadPrefs::default().line_weights;
        }
    });
}

// ----- General Plan Defaults -----

pub(super) fn plan_defaults(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    ui.label("The defaults every new object takes (walls, doors, windows, rooms, text, dimensions) belong to the plan and are kept with My Template.");
    ui.add_space(6.0);
    if ui.button("General Plan Defaults...").clicked() {
        actions.push(Action::DefaultSettings);
    }
}

// ----- Unit Conversions -----

/// The converter's answers for `input`, one `(unit label, text)` per unit;
/// empty when the input is not a length.
pub fn convert(input: &str, p: &pages::UnitPrefs) -> Vec<(&'static str, String)> {
    let Some(inches) = parse_length(input, p.input_unit) else {
        return Vec::new();
    };
    let f = |unit: LengthUnit| LengthFormat {
        unit,
        fraction_denominator: p.fraction_denominator,
        decimals: p.decimals,
        ..LengthFormat::default()
    };
    [
        ("Feet and inches", LengthUnit::FeetInches),
        ("Inches", LengthUnit::Inches),
        ("Decimal feet", LengthUnit::DecimalFeet),
        ("Millimeters", LengthUnit::Millimeters),
        ("Centimeters", LengthUnit::Centimeters),
        ("Meters", LengthUnit::Meters),
    ]
    .into_iter()
    .map(|(label, unit)| (label, format_length(inches, &f(unit))))
    .collect()
}

fn unit_label(u: LengthUnit) -> &'static str {
    match u {
        LengthUnit::FeetInches => "Feet and inches",
        LengthUnit::Inches => "Inches",
        LengthUnit::DecimalFeet => "Decimal feet",
        LengthUnit::Millimeters => "Millimeters",
        LengthUnit::Centimeters => "Centimeters",
        LengthUnit::Meters => "Meters",
    }
}

pub(super) fn units(ui: &mut egui::Ui) {
    let mut input = super::live(|l| l.converter.clone());
    let mut pg = pages::current();
    let before = pg.units.clone();
    ui.horizontal(|ui| {
        ui.label("Length");
        ui.add(
            egui::TextEdit::singleline(&mut input)
                .hint_text("12'-6 1/2\"  or  3810 mm")
                .desired_width(200.0),
        );
        egui::ComboBox::from_id_salt("prefs_conv_unit")
            .selected_text(unit_label(pg.units.input_unit))
            .show_ui(ui, |ui| {
                for u in [
                    LengthUnit::FeetInches,
                    LengthUnit::Inches,
                    LengthUnit::DecimalFeet,
                    LengthUnit::Millimeters,
                    LengthUnit::Centimeters,
                    LengthUnit::Meters,
                ] {
                    ui.selectable_value(&mut pg.units.input_unit, u, unit_label(u));
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Fractions to");
        egui::ComboBox::from_id_salt("prefs_conv_frac")
            .selected_text(format!("1/{}", pg.units.fraction_denominator))
            .show_ui(ui, |ui| {
                for d in [2, 4, 8, 16, 32, 64] {
                    ui.selectable_value(&mut pg.units.fraction_denominator, d, format!("1/{d}"));
                }
            });
        ui.label("Decimals");
        ui.add(egui::DragValue::new(&mut pg.units.decimals).range(0..=6));
    });
    ui.add_space(6.0);
    let rows = convert(&input, &pg.units);
    if rows.is_empty() {
        ui.weak("Type a length to see it in every unit; a unit written after the number (mm, cm, m, ft, in) is used as typed.");
    } else {
        egui::Grid::new("prefs_conversions")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for (label, text) in rows {
                    ui.label(label);
                    ui.monospace(text);
                    ui.end_row();
                }
            });
    }
    super::live(|l| l.converter = input);
    if pg.units != before {
        edit_pages(|p| p.units = pg.units.clone());
    }
}

// ----- Reset Options -----

/// A Reset Options command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reset {
    Toolbars,
    DialogSizes,
    DontAskAgain,
    SideWindows,
}

impl Reset {
    pub const ALL: [Reset; 4] = [
        Reset::Toolbars,
        Reset::DialogSizes,
        Reset::DontAskAgain,
        Reset::SideWindows,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Reset::Toolbars => "Reset Toolbars",
            Reset::DialogSizes => "Reset Dialog Sizes",
            Reset::DontAskAgain => "Reset \"Don't Ask Again\" Messages",
            Reset::SideWindows => "Reset Side Windows",
        }
    }

    pub fn what(self) -> &'static str {
        match self {
            Reset::Toolbars => {
                "Every toolbar row of every view type goes back to Daniel's Chief set."
            }
            Reset::DialogSizes => "Dialogs open at their normal size and place again.",
            Reset::DontAskAgain => "Messages you turned off ask again.",
            Reset::SideWindows => {
                "The Library, Project and Layer panels return to their normal widths."
            }
        }
    }
}

/// Runs one reset and says what it did.
pub fn run_reset(ctx: &egui::Context, settings: &mut AppSettings, which: Reset) -> String {
    match which {
        Reset::Toolbars => match config::reset_all_live() {
            Ok(()) => "Toolbars are back to Daniel's Chief set".into(),
            Err(e) => format!("Toolbars are reset but could not be saved: {e}"),
        },
        Reset::DialogSizes => {
            ctx.memory_mut(|m| m.reset_areas());
            "Dialog sizes and places are reset".into()
        }
        Reset::DontAskAgain => {
            let n = pages::reset_dont_ask();
            format!("{n} message(s) will ask again")
        }
        Reset::SideWindows => {
            settings.dock_widths = DockWidths::default();
            "Side windows are back to their normal widths".into()
        }
    }
}

pub(super) fn reset_options(ui: &mut egui::Ui, settings: &mut AppSettings) {
    ui.label("Each button asks for a second click.");
    ui.add_space(4.0);
    let pending = super::live(|l| l.reset_pending);
    for r in Reset::ALL {
        ui.horizontal(|ui| {
            let armed = pending == Some(r);
            let text = if armed {
                format!("Click again: {}", r.label())
            } else {
                r.label().to_string()
            };
            if ui.add(egui::Button::new(text).selected(armed)).clicked() {
                if armed {
                    let note = run_reset(ui.ctx(), settings, r);
                    super::live(|l| {
                        l.reset_pending = None;
                        l.reset_note = note;
                    });
                } else {
                    super::live(|l| l.reset_pending = Some(r));
                }
            }
            ui.weak(r.what());
        });
    }
    let note = super::live(|l| l.reset_note.clone());
    if !note.is_empty() {
        ui.add_space(6.0);
        ui.label(note);
    }
}
