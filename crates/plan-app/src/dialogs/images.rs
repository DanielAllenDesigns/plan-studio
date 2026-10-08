//! Image Specification (file, size, transparency colour, rotation) and
//! Distribution Path / Region Specification (object, spacing, offset,
//! scatter, random rotation and size) for the Image and Distributed Objects
//! tools (`docs/chief-x18-dialogs.md`).
//!
//! Both edit a draft [`PlacedSymbol`] (a picture, or a distribution record).
//! The Symbol Specification hosts them (`dialogs::symbol::SymbolDialog`) so
//! double-clicking a picture or a distribution opens the right form; OK goes
//! through `placed::apply_symbol`, which regenerates a distribution's copies
//! in the same undo step.

use super::{
    dis_combo, pv_text, row, section, Fields, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK, PV_WALL,
};
use crate::tools::images::{load_spec, pick_image_file, texture};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui, Vec2};
use plan_core::geometry::Point;
use plan_core::images::{
    DistKind, ImageFormat, RegionPattern, BILLBOARD_PLAN_DEPTH, DEFAULT_BILLBOARD_HEIGHT,
    MIN_SPACING,
};
use plan_core::PlacedSymbol;

const IMAGE_TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Image",
        enabled: true,
    },
    Tab {
        name: "Layer",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
];

const DIST_TABS: &[Tab] = &[
    Tab {
        name: "General",
        enabled: true,
    },
    Tab {
        name: "Random",
        enabled: true,
    },
    Tab {
        name: "Layer",
        enabled: true,
    },
    Tab {
        name: "Label",
        enabled: true,
    },
];

fn layer_row(ui: &mut Ui, salt: &str, layers: &[String], layer: &mut String) {
    section(ui, "Layer");
    row(ui, "Layer", |ui| {
        if layers.is_empty() {
            dis_combo(ui, salt, layer);
            return;
        }
        egui::ComboBox::from_id_salt(salt)
            .selected_text(layer.clone())
            .show_ui(ui, |ui| {
                for l in layers {
                    ui.selectable_value(layer, l.clone(), l);
                }
            });
    });
}

fn label_row(ui: &mut Ui, label: &mut String) {
    section(ui, "Label");
    row(ui, "Label", |ui| {
        ui.text_edit_singleline(label);
    });
}

// ----- pictures -----

pub struct ImageForm {
    pub draft: PlacedSymbol,
    layers: Vec<String>,
    fields: Fields,
    note: String,
}

impl ImageForm {
    pub fn new(draft: PlacedSymbol, layers: Vec<String>) -> Self {
        Self {
            draft,
            layers,
            fields: Fields::default(),
            note: String::new(),
        }
    }

    fn billboard(&self) -> bool {
        self.draft.image.as_ref().is_some_and(|i| i.billboard)
    }

    /// Resizes the picture to the file's proportions, keeping its width.
    pub fn match_proportions(&mut self) {
        let Some(spec) = &self.draft.image else {
            return;
        };
        let a = spec.aspect();
        if spec.billboard {
            self.draft.height = self.draft.width / a;
        } else {
            self.draft.depth = self.draft.width / a;
        }
    }

    /// Switches between a flat picture and a billboard.
    pub fn set_billboard(&mut self, on: bool) {
        if self.billboard() == on {
            return;
        }
        if let Some(spec) = &mut self.draft.image {
            spec.billboard = on;
        }
        if on {
            self.draft.depth = BILLBOARD_PLAN_DEPTH;
            self.draft.height = DEFAULT_BILLBOARD_HEIGHT;
        } else {
            self.draft.height = 0.0;
            self.draft.depth = self.draft.width;
        }
        self.match_proportions();
    }

    /// Replaces the picture file (keeping size, rotation and key colour).
    pub fn set_file(&mut self, path: &str) {
        match load_spec(path) {
            Ok(new) => {
                if let Some(spec) = &mut self.draft.image {
                    spec.path = new.path;
                    spec.format = new.format;
                    spec.px_w = new.px_w;
                    spec.px_h = new.px_h;
                    spec.color = new.color;
                    spec.note = new.note;
                }
                self.match_proportions();
                self.note = format!("Loaded {path}");
            }
            Err(e) => self.note = e,
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        let mut browse = false;
        let mut proportions = false;
        let billboard = self.billboard();
        let f = &mut self.fields;
        let d = &mut self.draft;
        if let Some(spec) = &mut d.image {
            section(ui, "Picture");
            row(ui, "File", |ui| {
                ui.add(egui::TextEdit::singleline(&mut spec.path).desired_width(150.0));
                if ui.button("Browse...").clicked() {
                    browse = true;
                }
            });
            row(ui, "Picture size", |ui| {
                ui.label(format!("{} x {} px", spec.px_w, spec.px_h))
            });
            if !spec.note.is_empty() {
                ui.weak(&spec.note);
            }
        }
        if !self.note.is_empty() {
            ui.weak(&self.note);
        }
        section(ui, "Size");
        f.length_row(ui, "Width", "width", &mut d.width);
        if billboard {
            f.length_row(ui, "Height", "height", &mut d.height);
        } else {
            f.length_row(ui, "Depth", "depth", &mut d.depth);
        }
        if ui.button("Match picture proportions").clicked() {
            proportions = true;
        }
        section(ui, "Position");
        f.length_row(ui, "Elevation (from floor)", "elev", &mut d.elevation);
        f.length_row(ui, "Position X (back center)", "pos_x", &mut d.position.x);
        f.length_row(ui, "Position Y (back center)", "pos_y", &mut d.position.y);
        f.degrees_row(ui, "Rotation", "deg_angle", &mut d.angle);
        ui.checkbox(&mut d.flip, "Flip (mirror left to right)");
        if browse {
            if let Some(path) = pick_image_file() {
                self.set_file(&path);
            }
        }
        if proportions {
            self.match_proportions();
        }
    }

    fn image(&mut self, ui: &mut Ui) {
        let mut billboard = self.billboard();
        let was = billboard;
        if let Some(spec) = &mut self.draft.image {
            section(ui, "Transparency");
            let mut on = spec.transparency.is_some();
            if ui
                .checkbox(&mut on, "Make one colour transparent")
                .changed()
            {
                spec.transparency = on.then(|| spec.transparency.unwrap_or([255, 255, 255]));
            }
            if let Some(key) = &mut spec.transparency {
                row(ui, "Transparent colour", |ui| {
                    ui.color_edit_button_srgb(key);
                });
                row(ui, "Tolerance", |ui| {
                    ui.add(egui::Slider::new(&mut spec.tolerance, 0..=128));
                });
            }
            if spec.format != ImageFormat::Png {
                ui.weak("Transparency applies to PNG pictures only");
            }
            section(ui, "Behavior");
        }
        ui.checkbox(&mut billboard, "Billboard (stands up and faces the camera)");
        if billboard != was {
            self.set_billboard(billboard);
        }
    }
}

impl SpecPages for ImageForm {
    fn tabs(&self) -> &'static [Tab] {
        IMAGE_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = &self.draft;
        let path_empty = d.image.as_ref().is_none_or(|i| i.path.trim().is_empty());
        if path_empty {
            return Some("Choose a picture file".into());
        }
        let tall = if self.billboard() { d.height } else { d.depth };
        (d.width < 1.0 || tall < 1.0).then(|| "Width and height must be positive".to_string())
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match IMAGE_TABS.get(tab).map(|t| t.name) {
            Some("General") => self.general(ui),
            Some("Image") => self.image(ui),
            Some("Layer") => layer_row(ui, "img_layer", &self.layers, &mut self.draft.layer),
            Some("Label") => label_row(ui, &mut self.draft.label),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let area = rect.shrink2(Vec2::new(8.0, 20.0));
        let Some(spec) = &self.draft.image else {
            return;
        };
        let a = spec.aspect() as f32;
        let (mut w, mut h) = (area.width(), area.width() / a);
        if h > area.height() {
            h = area.height();
            w = h * a;
        }
        let r = Rect::from_center_size(area.center(), Vec2::new(w, h));
        match texture(painter.ctx(), spec) {
            Some(id) => {
                painter.image(
                    id,
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => {
                let [cr, cg, cb] = spec.color;
                painter.rect_filled(r, 0.0, Color32::from_rgb(cr, cg, cb).gamma_multiply(0.5));
                painter.line_segment(
                    [r.left_top(), r.right_bottom()],
                    Stroke::new(0.6_f32, PV_FAINT),
                );
                painter.line_segment(
                    [r.right_top(), r.left_bottom()],
                    Stroke::new(0.6_f32, PV_FAINT),
                );
            }
        }
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0_f32, PV_INK),
            egui::StrokeKind::Outside,
        );
        let name = std::path::Path::new(&spec.path).file_name().map_or_else(
            || "Picture".to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        pv_text(
            painter,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            name,
            10.0,
        );
        if texture(painter.ctx(), spec).is_none() && !spec.note.is_empty() {
            pv_text(
                painter,
                Pos2::new(rect.center().x, rect.max.y - 8.0),
                Align2::CENTER_CENTER,
                "No preview",
                9.0,
            );
        }
    }
}

// ----- distributions -----

pub struct DistributionForm {
    pub draft: PlacedSymbol,
    layers: Vec<String>,
    fields: Fields,
    object: String,
    note: String,
}

impl DistributionForm {
    pub fn new(draft: PlacedSymbol, layers: Vec<String>) -> Self {
        let mut f = Self {
            draft,
            layers,
            fields: Fields::default(),
            object: String::new(),
            note: String::new(),
        };
        f.describe();
        f
    }

    fn describe(&mut self) {
        let id = self
            .draft
            .distribution
            .as_ref()
            .map(|d| d.item.clone())
            .unwrap_or_default();
        self.object = crate::tools::library::find_item(&id).map_or(id, |i| i.name.clone());
    }

    /// Use Active Library Item: the Library Browser's active item becomes
    /// the object that is distributed.
    pub fn use_active_item(&mut self) {
        let Some(id) = crate::tools::library::active_item() else {
            self.note = "Pick an item in the Library Browser first".into();
            return;
        };
        let Some(item) = crate::tools::library::find_item(&id) else {
            self.note = "Unknown library item".into();
            return;
        };
        if let Some(d) = &mut self.draft.distribution {
            d.item = item.id.clone();
            d.item_size = [item.width, item.depth, item.height];
            d.item_elevation = item.elevation;
            d.item_image = crate::tools::images::spec_from_item(&item);
        }
        self.describe();
        self.note = format!("Distributing {}", self.object);
    }

    fn general(&mut self, ui: &mut Ui) {
        let mut use_active = false;
        let f = &mut self.fields;
        let Some(d) = &mut self.draft.distribution else {
            return;
        };
        section(ui, "Object");
        row(ui, "Object", |ui| ui.label(&self.object));
        row(ui, "Library", |ui| {
            if ui.button("Use Active Library Item").clicked() {
                use_active = true;
            }
        });
        if !self.note.is_empty() {
            ui.weak(&self.note);
        }
        f.length_row(ui, "Object width", "item_w", &mut d.item_size[0]);
        f.length_row(ui, "Object depth", "item_d", &mut d.item_size[1]);
        f.length_row(ui, "Object height", "item_h", &mut d.item_size[2]);
        f.length_row(ui, "Object elevation", "item_e", &mut d.item_elevation);
        section(
            ui,
            if d.kind == DistKind::Path {
                "Path"
            } else {
                "Region"
            },
        );
        f.length_row(
            ui,
            if d.kind == DistKind::Path {
                "Spacing"
            } else {
                "Grid spacing"
            },
            "spacing",
            &mut d.spacing,
        );
        f.length_row(
            ui,
            if d.kind == DistKind::Path {
                "Offset (start of path)"
            } else {
                "Offset (inset from edge)"
            },
            "offset",
            &mut d.offset,
        );
        if d.kind == DistKind::Path {
            f.length_row(ui, "Side offset (left)", "side", &mut d.side_offset);
            ui.checkbox(&mut d.align_to_path, "Turn objects to follow the path");
        } else {
            row(ui, "Pattern", |ui| {
                ui.radio_value(&mut d.pattern, RegionPattern::Grid, "Grid");
                ui.radio_value(&mut d.pattern, RegionPattern::Random, "Random");
            });
        }
        let n = d.copies().len();
        ui.weak(format!("{n} object{}", if n == 1 { "" } else { "s" }));
        if use_active {
            self.use_active_item();
        }
    }

    fn random(&mut self, ui: &mut Ui) {
        let f = &mut self.fields;
        let Some(d) = &mut self.draft.distribution else {
            return;
        };
        section(ui, "Random");
        f.length_row(ui, "Scatter (radius)", "scatter", &mut d.scatter);
        row(ui, "Random rotation (+/- deg)", |ui| {
            ui.add(egui::DragValue::new(&mut d.random_rotation).range(0.0..=360.0));
        });
        row(ui, "Random size (+/- %)", |ui| {
            ui.add(egui::DragValue::new(&mut d.random_size).range(0.0..=90.0));
        });
        row(ui, "Random seed", |ui| {
            ui.add(egui::DragValue::new(&mut d.seed));
            if ui.button("New pattern").clicked() {
                d.seed = d.seed.wrapping_add(1);
            }
        });
    }
}

impl SpecPages for DistributionForm {
    fn tabs(&self) -> &'static [Tab] {
        DIST_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Enter valid lengths".into());
        }
        let d = self.draft.distribution.as_ref()?;
        if d.spacing < MIN_SPACING {
            return Some(format!("Spacing must be at least {MIN_SPACING}\""));
        }
        if d.offset < 0.0 || d.scatter < 0.0 {
            return Some("Offset and scatter cannot be negative".into());
        }
        if d.item_size.iter().take(2).any(|v| *v <= 0.0) {
            return Some("Object width and depth must be positive".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match DIST_TABS.get(tab).map(|t| t.name) {
            Some("General") => self.general(ui),
            Some("Random") => self.random(ui),
            Some("Layer") => {
                let layers = self.layers.clone();
                if let Some(d) = &mut self.draft.distribution {
                    layer_row(ui, "dist_layer", &layers, &mut d.layer);
                }
            }
            Some("Label") => label_row(ui, &mut self.draft.label),
            _ => {}
        }
    }

    fn preview(&self, painter: &Painter, rect: Rect) {
        let Some(d) = &self.draft.distribution else {
            return;
        };
        let line = d.polyline();
        let copies = d.copies();
        let mut pts: Vec<Point> = line.clone();
        pts.extend(copies.iter().map(|c| c.center));
        let Some(first) = pts.first().copied() else {
            return;
        };
        let (mut lo, mut hi) = (first, first);
        for p in &pts {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let area = rect.shrink2(Vec2::new(8.0, 20.0));
        let (w, h) = ((hi.x - lo.x).max(1.0) as f32, (hi.y - lo.y).max(1.0) as f32);
        let scale = (area.width() / w).min(area.height() / h);
        let size = Vec2::new(w * scale, h * scale);
        let tl = area.center() - size * 0.5;
        let map = |p: Point| {
            Pos2::new(
                tl.x + (p.x - lo.x) as f32 * scale,
                tl.y + size.y - (p.y - lo.y) as f32 * scale,
            )
        };
        let mut outline: Vec<Pos2> = line.iter().map(|p| map(*p)).collect();
        if d.kind == DistKind::Region {
            if let Some(f) = outline.first().copied() {
                outline.push(f);
            }
        }
        painter.add(egui::Shape::line(outline, Stroke::new(1.0_f32, PV_WALL)));
        let r = (d.item_size[0].max(d.item_size[1]) as f32 * scale * 0.5).clamp(1.5, 8.0);
        for c in &copies {
            painter.circle_filled(map(c.center), r, PV_ACCENT.gamma_multiply(0.8));
        }
        pv_text(
            painter,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            format!("{} x {}", copies.len(), self.object),
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{placed, EditorContext};
    use crate::plan_defaults;
    use crate::tools::images::tests::fixture_file;
    use crate::tools::images::{place_image, set_user_library_path};
    use crate::tools::library::{clear_active_item, library_catalog};
    use plan_core::images::{Distribution, ImageSpec};

    fn cx() -> EditorContext {
        set_user_library_path(Some(None));
        clear_active_item();
        EditorContext::new(plan_defaults::embedded())
    }

    fn smoke(form: &mut dyn SpecPages) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for tab in 0..form.tabs().len() {
                    form.page(ui, tab);
                }
                let (_, painter) =
                    ui.allocate_painter(Vec2::new(220.0, 300.0), egui::Sense::hover());
                form.preview(&painter, painter.clip_rect());
            });
        });
    }

    #[test]
    fn distribution_dialog_applies_in_one_undo_step() {
        let mut cx = cx();
        let item = library_catalog().all_items().next().unwrap();
        let mut d = Distribution::new(
            DistKind::Path,
            false,
            vec![Point::new(0.0, 0.0), Point::new(240.0, 0.0)],
            item.id.clone(),
            [item.width, item.depth, item.height],
        );
        d.spacing = 24.0;
        cx.begin_change("Polyline Distribution Path");
        let fl = cx.floor;
        let id = cx.project.add_distribution(fl, d);
        cx.mark_dirty();
        assert_eq!(cx.project.distribution_copies(fl, id), 11);

        let rec = cx.floor().symbol(id).unwrap().clone();
        let mut form = DistributionForm::new(rec, vec!["CAD, Default".into()]);
        assert!(form.error().is_none());
        smoke(&mut form);
        form.draft.distribution.as_mut().unwrap().spacing = 48.0;
        form.draft.distribution.as_mut().unwrap().scatter = 3.0;
        assert!(placed::apply_symbol(&mut cx, &form.draft));
        assert_eq!(cx.project.distribution_copies(fl, id), 6);
        // Exactly one undo step reverts the whole specification.
        assert_eq!(cx.undo().as_deref(), Some("Distribution Specification"));
        assert_eq!(cx.project.distribution_copies(fl, id), 11);
        assert_eq!(cx.floor().symbols.len(), 12);
        assert_eq!(cx.undo().as_deref(), Some("Polyline Distribution Path"));
        // Validation.
        form.draft.distribution.as_mut().unwrap().spacing = 0.2;
        assert!(form.error().is_some());
    }

    #[test]
    fn distribution_dialog_swaps_the_object_for_the_active_item() {
        let mut cx = cx();
        let items: Vec<_> = library_catalog().all_items().take(2).collect();
        let d = Distribution::new(
            DistKind::Region,
            false,
            vec![
                Point::new(0.0, 0.0),
                Point::new(96.0, 0.0),
                Point::new(96.0, 96.0),
                Point::new(0.0, 96.0),
            ],
            items[0].id.clone(),
            [items[0].width, items[0].depth, items[0].height],
        );
        let fl = cx.floor;
        let id = cx.project.add_distribution(fl, d);
        let mut form = DistributionForm::new(cx.floor().symbol(id).unwrap().clone(), Vec::new());
        form.use_active_item();
        assert!(form.note.contains("Library Browser"), "{}", form.note);
        crate::tools::library::set_active_item(&mut cx, &items[1].id);
        form.use_active_item();
        assert_eq!(form.draft.distribution.as_ref().unwrap().item, items[1].id);
        assert_eq!(form.object, items[1].name);
        smoke(&mut form);
    }

    #[test]
    fn image_dialog_edits_size_transparency_rotation_and_billboard() {
        let mut cx = cx();
        let path = fixture_file("dialog.png");
        let spec = crate::tools::images::load_spec(&path).unwrap();
        let id = place_image(&mut cx, &spec, Point::new(100.0, 100.0), false);
        let sym = cx.floor().symbol(id).unwrap().clone();
        let mut form = ImageForm::new(sym, vec!["CAD, Default".into()]);
        assert!(form.error().is_none());
        smoke(&mut form);
        form.draft.width = 60.0;
        form.draft.angle = 45.0;
        form.draft.image.as_mut().unwrap().transparency = Some([255, 255, 255]);
        form.match_proportions();
        assert!((form.draft.depth - 30.0).abs() < 1e-9);
        assert!(placed::apply_symbol(&mut cx, &form.draft));
        let s = cx.floor().symbol(id).unwrap();
        assert_eq!((s.width, s.depth, s.angle), (60.0, 30.0, 45.0));
        assert_eq!(
            s.image.as_ref().unwrap().transparency,
            Some([255, 255, 255])
        );
        assert_eq!(cx.undo().as_deref(), Some("Symbol Specification"));
        // Billboard toggle swaps depth and height.
        form.set_billboard(true);
        assert_eq!(form.draft.depth, BILLBOARD_PLAN_DEPTH);
        assert!((form.draft.height - 30.0).abs() < 1e-9);
        form.set_billboard(false);
        assert_eq!(form.draft.height, 0.0);
        assert!((form.draft.depth - 30.0).abs() < 1e-9);
        // A new file replaces the picture facts.
        let other = fixture_file("other.png");
        form.set_file(&other);
        assert_eq!(form.draft.image.as_ref().unwrap().path, other);
        form.set_file("/nope.png");
        assert!(form.note.contains("Cannot read"));
        // An empty path blocks OK.
        form.draft.image = Some(ImageSpec::new("", 0, 0));
        assert!(form.error().is_some());
    }

    #[test]
    fn the_symbol_dialog_hosts_pictures_and_distributions() {
        use crate::dialogs::symbol::SymbolDialog;
        let mut cx = cx();
        let path = fixture_file("host.png");
        let spec = crate::tools::images::load_spec(&path).unwrap();
        let id = place_image(&mut cx, &spec, Point::new(10.0, 10.0), true);
        let dlg = SymbolDialog::new(cx.floor().symbol(id).unwrap().clone(), Vec::new());
        assert_eq!(dlg.draft().id, id);
        assert!(dlg.draft().image.as_ref().unwrap().billboard);
        let item = library_catalog().all_items().next().unwrap();
        let d = Distribution::new(
            DistKind::Path,
            false,
            vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            item.id.clone(),
            [item.width, item.depth, item.height],
        );
        let fl = cx.floor;
        let rid = cx.project.add_distribution(fl, d);
        let dlg = SymbolDialog::new(cx.floor().symbol(rid).unwrap().clone(), Vec::new());
        assert!(dlg.draft().distribution.is_some());
    }
}
