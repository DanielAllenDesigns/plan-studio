//! The Material Specification dialog (Chief's Material Definition): one
//! library material in four tabs.
//!
//! * **Pattern**: the CAD pattern, its scale and angle, used for the hatch in
//!   plan and elevation views.
//! * **Texture**: a picture from Chief's texture folders (read at run time)
//!   or any file, or the generated bitmap; tile size, offset, angle and a
//!   blend colour.
//! * **Properties**: the material class (General, Plastic, Metal, Glass,
//!   Mirror, Emissive, Transparent), colour, roughness, metalness,
//!   transparency, emissive strength and bump. The 3D view and the ray tracer
//!   read these.
//! * **Materials List**: manufacturer, supplier, price and unit, which the
//!   Materials List window multiplies by the areas of the plan.
//!
//! The dialog edits a draft. With *Update 3D view while editing* on, the
//! draft shows live on the painted surfaces that use the material (the
//! Interactive Material Editor); OK saves it to My Materials (a core
//! material keeps its name, so the user's copy takes its place) and Cancel
//! leaves the library alone.

use super::{pattern_by_name, pattern_name, scene_material, PATTERNS};
use eframe::egui::{self, Align2, Color32};
use plan_materials::{
    clip_strokes_to_polygon, filter_texture_files, list_texture_files, textures, transform_rgba,
    MapKind, MaterialClass, MaterialDef, PriceUnit, TextureFile, BLEND_PREFIX,
};

/// What the dialog wants done after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still editing.
    Keep,
    /// OK: save to My Materials.
    Save,
    /// Remove the user's copy of the material (back to the core one).
    Revert,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Pattern,
    Texture,
    Properties,
    List,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Pattern, Tab::Texture, Tab::Properties, Tab::List];

    fn label(self) -> &'static str {
        match self {
            Tab::Pattern => "Pattern",
            Tab::Texture => "Texture",
            Tab::Properties => "Properties",
            Tab::List => "Materials List",
        }
    }
}

/// Most picture files listed from Chief's folders.
const MAX_FILES: usize = 6000;
/// Rows of the picture list drawn at once.
const MAX_ROWS: usize = 300;

/// The draft of one material.
pub struct MaterialSpec {
    def: MaterialDef,
    /// The CAD pattern's name (`PATTERNS`).
    pattern: &'static str,
    /// The category path as typed ("Siding > Custom").
    category: String,
    tab: Tab,
    /// The library name this draft started from.
    original: Option<String>,
    /// Does the 3D view show the draft while it is edited?
    live: bool,
    /// Why the last save failed.
    pub note: String,
    files: Option<Vec<TextureFile>>,
    file_query: String,
    preview: Option<(u64, egui::TextureHandle)>,
}

impl MaterialSpec {
    /// A new, nameless material.
    pub fn new() -> Self {
        let mut def = MaterialDef::new("", &["Custom"], [200, 200, 200]);
        def.category = vec!["Custom".to_string()];
        Self::draft(def, None)
    }

    /// A draft of `d` (editing it keeps its name).
    pub fn from_def(d: &MaterialDef) -> Self {
        Self::draft(d.clone(), Some(d.name.clone()))
    }

    fn draft(def: MaterialDef, original: Option<String>) -> Self {
        Self {
            pattern: pattern_name(&def.pattern),
            category: def.category.join(" > "),
            def,
            tab: Tab::Pattern,
            original,
            live: true,
            note: String::new(),
            files: None,
            file_query: String::new(),
            preview: None,
        }
    }

    /// The name typed in the dialog.
    pub fn name(&self) -> &str {
        &self.def.name
    }

    /// The library name the draft started from ("" for a new material).
    pub fn original_name(&self) -> &str {
        self.original.as_deref().unwrap_or("")
    }

    /// Direct access for tests and the scenarios.
    pub fn draft_def_mut(&mut self) -> &mut MaterialDef {
        &mut self.def
    }

    pub fn set_pattern(&mut self, name: &'static str) {
        self.pattern = name;
        self.def.pattern = pattern_by_name(name);
    }

    pub fn set_category(&mut self, text: &str) {
        self.category = text.to_string();
    }

    /// The material the draft makes, or why it cannot be saved.
    pub fn def(&self) -> Result<MaterialDef, String> {
        let name = self.def.name.trim();
        if name.is_empty() {
            return Err("A material needs a name".to_string());
        }
        if name.starts_with(BLEND_PREFIX) {
            return Err(format!(
                "Names starting with \"{}\" belong to Blend Colors: pick another name",
                BLEND_PREFIX.trim_end()
            ));
        }
        let mut d = self.def.clone();
        d.name = name.to_string();
        let cats: Vec<String> = self
            .category
            .split('>')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        d.category = if cats.is_empty() {
            vec!["Custom".to_string()]
        } else {
            cats
        };
        d.pattern = pattern_by_name(self.pattern);
        d.texture_path = d
            .texture_path
            .take()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());
        d.roughness = d.roughness.clamp(0.0, 1.0);
        d.metallic = d.metallic.clamp(0.0, 1.0);
        d.transparency = d.transparency.clamp(0.0, 1.0);
        d.emissive = d.emissive.clamp(0.0, 1.0);
        d.bump = d.bump.clamp(0.0, 1.0);
        d.blend_amount = d.blend_amount.clamp(0.0, 1.0);
        d.pattern_scale = if d.pattern_scale.is_finite() && d.pattern_scale > 0.0 {
            d.pattern_scale.clamp(0.05, 20.0)
        } else {
            1.0
        };
        d.texture_scale_in = (
            d.texture_scale_in.0.clamp(0.5, 960.0),
            d.texture_scale_in.1.clamp(0.5, 960.0),
        );
        d.price = d.price.max(0.0);
        Ok(d)
    }

    /// The draft for the live 3D view; `None` when updating live is off or
    /// the draft cannot be saved yet.
    pub fn live_def(&self) -> Option<MaterialDef> {
        if self.live {
            self.def().ok()
        } else {
            None
        }
    }

    /// Draws the dialog.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut open = true;
        let mut outcome = Outcome::Keep;
        egui::Window::new("Material Specification")
            .id(egui::Id::new("materials_spec"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                egui::Grid::new("spec_head")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut self.def.name).desired_width(300.0));
                        ui.end_row();
                        ui.label("Category");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.category)
                                .hint_text("Siding > Custom")
                                .desired_width(300.0),
                        );
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    for t in Tab::ALL {
                        ui.selectable_value(&mut self.tab, t, t.label());
                    }
                });
                ui.separator();
                match self.tab {
                    Tab::Pattern => self.pattern_tab(ui),
                    Tab::Texture => self.texture_tab(ui, ctx),
                    Tab::Properties => self.properties_tab(ui),
                    Tab::List => self.list_tab(ui),
                }
                ui.separator();
                ui.checkbox(&mut self.live, "Update 3D view while editing");
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        outcome = Outcome::Save;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    let can_revert = self
                        .original
                        .as_deref()
                        .is_some_and(|n| super::user_library().find(n).is_some());
                    if ui
                        .add_enabled(can_revert, egui::Button::new("Revert to Library"))
                        .on_hover_text("Delete my copy of this material")
                        .clicked()
                    {
                        outcome = Outcome::Revert;
                    }
                });
                match self.def() {
                    Ok(d) => {
                        ui.weak(format!(
                            "Shows in 3D as {} \u{b7} saved to My Materials",
                            scene_material(&d).name()
                        ));
                    }
                    Err(e) => {
                        ui.weak(e);
                    }
                }
                if !self.note.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, self.note.clone());
                }
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    // ----- Pattern -----

    fn pattern_tab(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("spec_pattern")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("CAD pattern");
                let before = self.pattern;
                egui::ComboBox::from_id_salt("spec_pattern_combo")
                    .selected_text(self.pattern)
                    .show_ui(ui, |ui| {
                        for p in PATTERNS {
                            ui.selectable_value(&mut self.pattern, p, p);
                        }
                    });
                if self.pattern != before {
                    self.def.pattern = pattern_by_name(self.pattern);
                }
                ui.end_row();
                ui.label("Scale");
                ui.add(
                    egui::DragValue::new(&mut self.def.pattern_scale)
                        .speed(0.02)
                        .range(0.05..=20.0),
                );
                ui.end_row();
                ui.label("Angle");
                ui.add(
                    egui::DragValue::new(&mut self.def.pattern_angle)
                        .speed(0.5)
                        .range(-360.0..=360.0)
                        .suffix("\u{b0}"),
                );
                ui.end_row();
            });
        ui.add_space(6.0);
        ui.label("Plan and elevation hatch at 1/4\" = 1'");
        let (rect, _) = ui.allocate_exact_size(egui::vec2(300.0, 140.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(250, 248, 242));
        for (a, b) in hatch_preview(&self.def, self.pattern, rect.size()) {
            painter.line_segment(
                [
                    rect.min + egui::vec2(a.0, a.1),
                    rect.min + egui::vec2(b.0, b.1),
                ],
                egui::Stroke::new(1.0_f32, Color32::from_gray(60)),
            );
        }
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0_f32, Color32::from_gray(150)),
            egui::StrokeKind::Inside,
        );
    }

    // ----- Texture -----

    fn texture_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut path = self.def.texture_path.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Picture file");
            ui.add(
                egui::TextEdit::singleline(&mut path)
                    .hint_text("none: the generated texture")
                    .desired_width(260.0),
            );
            if ui.button("Browse...").clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    path = p.to_string_lossy().into_owned();
                }
            }
            if ui.button("Generated").clicked() {
                path.clear();
            }
        });
        self.def.texture_path = (!path.trim().is_empty()).then_some(path);
        // Chief's texture folders, read when first needed.
        let files = self.files.get_or_insert_with(|| {
            list_texture_files(&textures::default_texture_dirs(), MAX_FILES)
        });
        ui.horizontal(|ui| {
            ui.label("Chief textures");
            ui.add(
                egui::TextEdit::singleline(&mut self.file_query)
                    .hint_text("search")
                    .desired_width(160.0),
            );
            ui.weak(format!("{} pictures", files.len()));
        });
        let shown = filter_texture_files(files, &self.file_query);
        let mut picked: Option<(String, Option<f32>)> = None;
        egui::ScrollArea::vertical()
            .id_salt("spec_texture_files")
            .max_height(110.0)
            .show(ui, |ui| {
                if shown.is_empty() {
                    ui.weak(if files.is_empty() {
                        "Chief's texture folders were not found: use Browse"
                    } else {
                        "No picture matches"
                    });
                }
                for f in shown.iter().take(MAX_ROWS) {
                    let chosen =
                        self.def.texture_path.as_deref() == Some(f.path.to_string_lossy().as_ref());
                    let label = if f.folder.is_empty() {
                        f.name.clone()
                    } else {
                        format!("{}  \u{b7}  {}", f.name, f.folder)
                    };
                    if ui.selectable_label(chosen, label).clicked() {
                        picked = Some((f.path.to_string_lossy().into_owned(), f.tile_in));
                    }
                }
            });
        if let Some((p, tile)) = picked {
            self.def.texture_path = Some(p);
            if let Some(t) = tile {
                self.def.texture_scale_in = (f64::from(t), f64::from(t));
            }
        }
        self.package_maps(ui);
        ui.separator();
        egui::Grid::new("spec_texture")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Tile size");
                ui.horizontal(|ui| {
                    drag(ui, &mut self.def.texture_scale_in.0, 0.5, 960.0, " in", 0.5);
                    ui.label("\u{d7}");
                    drag(ui, &mut self.def.texture_scale_in.1, 0.5, 960.0, " in", 0.5);
                });
                ui.end_row();
                ui.label("Offset");
                ui.horizontal(|ui| {
                    drag(
                        ui,
                        &mut self.def.texture_offset_in.0,
                        -960.0,
                        960.0,
                        " in",
                        0.25,
                    );
                    ui.label(",");
                    drag(
                        ui,
                        &mut self.def.texture_offset_in.1,
                        -960.0,
                        960.0,
                        " in",
                        0.25,
                    );
                });
                ui.end_row();
                ui.label("Angle");
                drag(
                    ui,
                    &mut self.def.texture_angle_deg,
                    -360.0,
                    360.0,
                    "\u{b0}",
                    0.5,
                );
                ui.end_row();
                ui.label("Blend color");
                ui.horizontal(|ui| {
                    let mut on = self.def.blend_color.is_some();
                    if ui.checkbox(&mut on, "").changed() {
                        self.def.blend_color =
                            on.then(|| self.def.blend_color.unwrap_or([255, 255, 255]));
                        if on && self.def.blend_amount <= 0.0 {
                            self.def.blend_amount = 0.25;
                        }
                    }
                    if let Some(c) = &mut self.def.blend_color {
                        ui.color_edit_button_srgb(c);
                        ui.add(
                            egui::Slider::new(&mut self.def.blend_amount, 0.0..=1.0)
                                .show_value(true),
                        );
                    }
                });
                ui.end_row();
            });
        self.texture_preview(ui, ctx);
    }

    /// The extra maps of a material package (Lightbeans): one switch each.
    /// Nothing is drawn for a material without any.
    fn package_maps(&mut self, ui: &mut egui::Ui) {
        if !self.def.has_pbr_maps() {
            return;
        }
        ui.separator();
        ui.strong("Package maps");
        for kind in MapKind::ALL {
            if kind == MapKind::Albedo {
                continue;
            }
            let Some(path) = self.def.map_path(kind).map(str::to_string) else {
                continue;
            };
            let mut on = self.def.map_enabled(kind);
            let file = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(path.clone());
            let note = if kind == MapKind::Height && self.def.normal_map.is_some() {
                "  (the normal map is used)"
            } else {
                ""
            };
            if ui
                .checkbox(&mut on, format!("{}  \u{b7}  {file}{note}", kind.label()))
                .on_hover_text(path)
                .changed()
            {
                self.def.set_map_enabled(kind, on);
            }
        }
        if let Some(src) = &self.def.package_source {
            ui.weak(format!(
                "From {src}{}",
                self.def
                    .package_imported
                    .as_ref()
                    .map(|d| format!(", imported {d}"))
                    .unwrap_or_default()
            ));
        }
        ui.weak("The bump strength on the Properties tab scales the normals.");
    }

    fn texture_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let d = self.def.clone();
        let key = preview_key(&d);
        if self.preview.as_ref().is_none_or(|(k, _)| *k != key) {
            let (w, rgba) = preview_pixels(&d, &textures::TextureStore::shared());
            let img = egui::ColorImage::from_rgba_unmultiplied([w, w], &rgba);
            let tex = ctx.load_texture("spec_texture_preview", img, egui::TextureOptions::LINEAR);
            self.preview = Some((key, tex));
        }
        if let Some((_, tex)) = &self.preview {
            ui.horizontal(|ui| {
                ui.image(egui::load::SizedTexture::new(
                    tex.id(),
                    egui::vec2(96.0, 96.0),
                ));
                ui.weak("2 \u{d7} 2 repeats");
            });
        }
    }

    // ----- Properties -----

    fn properties_tab(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("spec_properties")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Material class");
                let mut class = self.def.class;
                egui::ComboBox::from_id_salt("spec_class")
                    .selected_text(class.name())
                    .show_ui(ui, |ui| {
                        for c in MaterialClass::ALL {
                            ui.selectable_value(&mut class, c, c.name());
                        }
                    });
                if class != self.def.class {
                    self.def.set_class(class);
                }
                ui.end_row();
                ui.label("Color");
                ui.color_edit_button_srgb(&mut self.def.color);
                ui.end_row();
                ui.label("Roughness");
                ui.add(egui::Slider::new(&mut self.def.roughness, 0.0..=1.0));
                ui.end_row();
                ui.label("Metalness");
                ui.add(egui::Slider::new(&mut self.def.metallic, 0.0..=1.0));
                ui.end_row();
                ui.label("Transparency");
                ui.add(egui::Slider::new(&mut self.def.transparency, 0.0..=1.0));
                ui.end_row();
                ui.label("Emissive");
                ui.add(egui::Slider::new(&mut self.def.emissive, 0.0..=1.0));
                ui.end_row();
                ui.label("Bump");
                ui.add(egui::Slider::new(&mut self.def.bump, 0.0..=1.0));
                ui.end_row();
            });
        let s = self.def.surface();
        ui.weak(format!(
            "Rendered with roughness {:.2}, metalness {:.2}, transparency {:.2}, emissive {:.2}",
            s.roughness, s.metallic, s.transparency, s.emissive
        ));
        ui.weak(class_note(self.def.class));
    }

    // ----- Materials List -----

    fn list_tab(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("spec_list")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Manufacturer");
                ui.add(egui::TextEdit::singleline(&mut self.def.manufacturer).desired_width(260.0));
                ui.end_row();
                ui.label("Supplier");
                ui.add(egui::TextEdit::singleline(&mut self.def.supplier).desired_width(260.0));
                ui.end_row();
                ui.label("Price");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.def.price)
                            .speed(0.05)
                            .range(0.0..=1.0e6)
                            .prefix("$"),
                    );
                    ui.label("per");
                    egui::ComboBox::from_id_salt("spec_unit")
                        .selected_text(self.def.unit.label())
                        .show_ui(ui, |ui| {
                            for u in PriceUnit::ALL {
                                ui.selectable_value(&mut self.def.unit, u, u.label());
                            }
                        });
                });
                ui.end_row();
                ui.label("Accounting code");
                ui.add(
                    egui::TextEdit::singleline(&mut self.def.accounting_code).desired_width(160.0),
                );
                ui.end_row();
            });
        let per = self.def.price_per_sq_ft();
        ui.weak(if per > 0.0 {
            format!("${per:.2} per square foot of surface")
        } else {
            "Not priced by area: the Materials List counts it by the piece".to_string()
        });
    }
}

impl Default for MaterialSpec {
    fn default() -> Self {
        Self::new()
    }
}

fn drag(ui: &mut egui::Ui, v: &mut f64, lo: f64, hi: f64, suffix: &str, speed: f64) {
    ui.add(
        egui::DragValue::new(v)
            .speed(speed)
            .range(lo..=hi)
            .suffix(suffix),
    );
}

/// What a class does, for the Properties tab.
pub fn class_note(c: MaterialClass) -> &'static str {
    match c {
        MaterialClass::General => "General: the sliders are used as they are.",
        MaterialClass::Plastic => "Plastic: glossy and not metallic.",
        MaterialClass::Metal => "Metal: metalness stays at 80% or more.",
        MaterialClass::Glass => "Glass: clear (50% or more) and smooth.",
        MaterialClass::Mirror => "Mirror: fully metallic, opaque and smooth.",
        MaterialClass::Emissive => "Emissive: glows at 30% or more.",
        MaterialClass::Transparent => "Transparent: see-through by 20% or more.",
    }
}

/// The hatch strokes of the Pattern tab's preview, in pixels inside a box of
/// `size`: the material's pattern over a 150 x 70 inch piece drawn at 2
/// pixels per inch (clipped to it).
pub fn hatch_preview(
    def: &MaterialDef,
    pattern: &str,
    size: egui::Vec2,
) -> Vec<((f32, f32), (f32, f32))> {
    use plan_core::Point;
    let mut d = def.clone();
    d.pattern = pattern_by_name(pattern);
    let (w, h) = (f64::from(size.x) / 2.0, f64::from(size.y) / 2.0);
    let rect = (Point::new(0.0, 0.0), Point::new(w, h));
    let poly = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    let strokes = clip_strokes_to_polygon(&d.hatch_strokes(rect, 0.25), &poly);
    strokes
        .into_iter()
        .map(|(a, b)| {
            // Plan y is up; the screen's is down.
            (
                (a.x as f32 * 2.0, size.y - a.y as f32 * 2.0),
                (b.x as f32 * 2.0, size.y - b.y as f32 * 2.0),
            )
        })
        .collect()
}

fn preview_key(d: &MaterialDef) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    d.name.hash(&mut h);
    d.texture_path.hash(&mut h);
    for v in [
        d.texture_scale_in.0,
        d.texture_scale_in.1,
        d.texture_offset_in.0,
        d.texture_offset_in.1,
        d.texture_angle_deg,
    ] {
        v.to_bits().hash(&mut h);
    }
    d.blend_color.hash(&mut h);
    d.blend_amount.to_bits().hash(&mut h);
    d.color.hash(&mut h);
    format!("{:?}", d.texture).hash(&mut h);
    h.finish()
}

/// A 96 x 96 picture of two repeats across and two down of the material's
/// texture with its offset, angle and blend colour applied (RGBA8).
pub fn preview_pixels(def: &MaterialDef, store: &textures::TextureStore) -> (usize, Vec<u8>) {
    const SIDE: usize = 96;
    let tex = store.definition(def);
    let tile =
        if def.texture_path.is_none() && matches!(def.texture, plan_materials::Texture::Solid) {
            // A flat material has no bitmap: its colour (tinted by the blend).
            let mut px = vec![0u8; 4 * 4];
            for p in px.chunks_mut(4) {
                p[..3].copy_from_slice(&def.color);
                p[3] = 255;
            }
            let flat = MaterialDef {
                texture_offset_in: (0.0, 0.0),
                texture_angle_deg: 0.0,
                ..def.clone()
            };
            (2u32, 2u32, transform_rgba(&px, 2, 2, [12.0, 12.0], &flat))
        } else {
            (
                tex.image.width,
                tex.image.height,
                transform_rgba(
                    &tex.image.rgba,
                    tex.image.width,
                    tex.image.height,
                    tex.scale_in,
                    def,
                ),
            )
        };
    let (tw, th, px) = tile;
    let mut out = vec![0u8; SIDE * SIDE * 4];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let u = (x * 2 * tw as usize / SIDE) % tw as usize;
            let v = (y * 2 * th as usize / SIDE) % th as usize;
            let s = (v * tw as usize + u) * 4;
            out[(y * SIDE + x) * 4..(y * SIDE + x) * 4 + 4].copy_from_slice(&px[s..s + 4]);
        }
    }
    (SIDE, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_needs_a_name_and_refuses_blend_names() {
        let mut sp = MaterialSpec::new();
        assert!(sp.def().is_err());
        sp.draft_def_mut().name = "  Barn Red ".into();
        let d = sp.def().unwrap();
        assert_eq!(d.name, "Barn Red");
        assert_eq!(d.category, ["Custom"]);
        sp.draft_def_mut().name = "Blend: A | B | 10%".into();
        assert!(sp.def().unwrap_err().contains("Blend Colors"));
    }

    #[test]
    fn the_draft_keeps_every_tab_and_clamps_wild_values() {
        let mut sp = MaterialSpec::new();
        sp.set_category("Masonry > Custom");
        sp.set_pattern("Brick");
        {
            let d = sp.draft_def_mut();
            d.name = "Painted Brick".into();
            d.set_class(MaterialClass::Plastic);
            d.pattern_scale = 99.0;
            d.pattern_angle = 15.0;
            d.texture_path = Some("  /tmp/b.png ".into());
            d.texture_scale_in = (0.0, 5000.0);
            d.texture_offset_in = (2.0, 3.0);
            d.texture_angle_deg = 90.0;
            d.blend_color = Some([1, 2, 3]);
            d.blend_amount = 7.0;
            d.manufacturer = "Acme".into();
            d.supplier = "Depot".into();
            d.price = -4.0;
            d.unit = PriceUnit::SqYd;
            d.metallic = 9.0;
        }
        let d = sp.def().unwrap();
        assert_eq!(d.category, ["Masonry", "Custom"]);
        assert_eq!(d.pattern, pattern_by_name("Brick"));
        assert_eq!(d.class, MaterialClass::Plastic);
        assert_eq!((d.pattern_scale, d.pattern_angle), (20.0, 15.0));
        assert_eq!(d.texture_path.as_deref(), Some("/tmp/b.png"));
        assert_eq!(d.texture_scale_in, (0.5, 960.0));
        assert_eq!(
            (d.texture_offset_in, d.texture_angle_deg),
            ((2.0, 3.0), 90.0)
        );
        assert_eq!((d.blend_color, d.blend_amount), (Some([1, 2, 3]), 1.0));
        assert_eq!(
            (d.manufacturer.as_str(), d.supplier.as_str()),
            ("Acme", "Depot")
        );
        assert_eq!((d.price, d.unit, d.metallic), (0.0, PriceUnit::SqYd, 1.0));
        // Editing a library material starts from it and remembers its name.
        let again = MaterialSpec::from_def(&d);
        assert_eq!(again.original_name(), "Painted Brick");
        assert_eq!(again.def().unwrap(), d);
    }

    #[test]
    fn live_updates_can_be_switched_off() {
        let mut sp = MaterialSpec::from_def(&MaterialDef::new("M", &["Custom"], [1, 2, 3]));
        assert!(sp.live_def().is_some());
        sp.live = false;
        assert!(sp.live_def().is_none());
        sp.live = true;
        sp.draft_def_mut().name.clear();
        assert!(sp.live_def().is_none(), "a nameless draft is not shown");
    }

    #[test]
    fn the_pattern_preview_follows_scale_and_angle() {
        let mut d = MaterialDef::new("B", &["Masonry"], [150, 70, 50]);
        let size = egui::vec2(300.0, 140.0);
        assert!(hatch_preview(&d, "None", size).is_empty());
        let plain = hatch_preview(&d, "Brick", size);
        assert!(plain.len() > 20);
        d.pattern_scale = 2.0;
        assert!(hatch_preview(&d, "Brick", size).len() < plain.len());
        d.pattern_scale = 1.0;
        d.pattern_angle = 45.0;
        let turned = hatch_preview(&d, "Brick", size);
        assert!(!turned.is_empty() && turned != plain);
        // Every stroke stays inside the box.
        for (a, b) in turned {
            for p in [a, b] {
                assert!(
                    p.0 >= -0.01 && p.0 <= 300.01 && p.1 >= -0.01 && p.1 <= 140.01,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn the_texture_preview_shows_the_blend_and_a_flat_colour() {
        let store = textures::TextureStore::with_dirs(Vec::new());
        let flat = MaterialDef::new("Flat", &["Custom"], [200, 100, 50]);
        let (w, px) = preview_pixels(&flat, &store);
        assert_eq!(px.len(), w * w * 4);
        assert_eq!(px[..4], [200, 100, 50, 255]);
        let mut tinted = flat.clone();
        tinted.blend_color = Some([0, 0, 0]);
        tinted.blend_amount = 0.5;
        assert_eq!(preview_pixels(&tinted, &store).1[..4], [100, 50, 25, 255]);
        // A generated texture has more than one colour.
        let lib = plan_materials::core_library();
        let (w, px) = preview_pixels(lib.find("Oak Flooring").unwrap(), &store);
        let first = &px[..4];
        assert!(px.chunks(4).any(|p| p != first), "{w}");
    }

    #[test]
    fn the_dialog_draws_headlessly_on_every_tab() {
        let ctx = egui::Context::default();
        let mut sp = MaterialSpec::from_def(&MaterialDef::new("M", &["Custom"], [1, 2, 3]));
        for t in Tab::ALL {
            sp.tab = t;
            let mut out = Outcome::Cancel;
            let _ = ctx.run(egui::RawInput::default(), |ctx| out = sp.show(ctx));
            assert_eq!(out, Outcome::Keep, "{t:?}");
        }
        assert!(PATTERNS
            .iter()
            .all(|p| pattern_name(&pattern_by_name(p)) == *p));
    }
}
