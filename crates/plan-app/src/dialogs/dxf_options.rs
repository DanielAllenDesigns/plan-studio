//! File > Export > DXF options (L-44, L-45): the window that asks what the
//! DXF should contain before it is saved — the unit, the layer names (Chief's
//! or the AIA standard, with a map of your own), line weights, text as text or
//! as lines, which floors, and 2D or 3D. The writing itself is
//! `plan_core::export::dxf_options`; this builds each floor's drawing the way
//! the plain export does (`exchange::floor_dxf`) and hands them over.

use super::exchange::floor_dxf;
use crate::editor::EditorContext;
use eframe::egui;
use plan_core::export::dxf_options::{
    build, DxfOptions, DxfUnits, FloorDxf, LayerNaming, SolidDxf, TextMode,
};
use std::cell::RefCell;

/// Which floors go in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FloorChoice {
    #[default]
    ThisFloor,
    AllFloors,
    Picked,
}

/// The window's fields.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfExportDialog {
    pub opts: DxfOptions,
    pub floors: FloorChoice,
    /// One flag per floor of the plan, for [`FloorChoice::Picked`].
    pub picked: Vec<bool>,
    /// The layer map as lines of `plan layer = file layer`.
    pub map_text: String,
    pub message: String,
}

impl DxfExportDialog {
    pub fn new(cx: &EditorContext) -> Self {
        let mut d = LAST.with(|l| l.borrow().clone()).unwrap_or(Self {
            opts: DxfOptions::default(),
            floors: FloorChoice::ThisFloor,
            picked: Vec::new(),
            map_text: String::new(),
            message: String::new(),
        });
        d.picked.resize(cx.project.floors.len(), false);
        if !d.picked.iter().any(|p| *p) {
            if let Some(p) = d.picked.get_mut(cx.floor) {
                *p = true;
            }
        }
        d.message.clear();
        d
    }

    /// The floors the choice stands for.
    pub fn floor_indices(&self, cx: &EditorContext) -> Vec<usize> {
        let n = cx.project.floors.len();
        match self.floors {
            FloorChoice::ThisFloor => vec![cx.floor.min(n.saturating_sub(1))],
            FloorChoice::AllFloors => (0..n)
                .filter(|i| !cx.project.floors[*i].is_cad_detail())
                .collect(),
            FloorChoice::Picked => (0..n)
                .filter(|i| self.picked.get(*i).copied().unwrap_or(false))
                .collect(),
        }
    }

    /// The options with the layer map text read in.
    pub fn options(&self) -> DxfOptions {
        let mut o = self.opts.clone();
        o.layer_map = parse_map(&self.map_text);
        o
    }

    /// The DXF text the fields ask for.
    pub fn text(&self, cx: &mut EditorContext) -> Result<String, String> {
        let floors = self.floor_indices(cx);
        if floors.is_empty() {
            return Err("Pick at least one floor".into());
        }
        Ok(build_text(cx, &floors, &self.options()))
    }
}

/// Lines of `from = to` as a layer map; other lines are ignored.
pub fn parse_map(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (a, b) = l.split_once('=')?;
            let (a, b) = (a.trim(), b.trim());
            (!a.is_empty() && !b.is_empty()).then(|| (a.to_string(), b.to_string()))
        })
        .collect()
}

/// The DXF of `floors` of the plan with `opts`.
pub fn build_text(cx: &mut EditorContext, floors: &[usize], opts: &DxfOptions) -> String {
    let saved = cx.floor;
    let mut parts: Vec<FloorDxf> = Vec::new();
    for &i in floors {
        cx.floor = i;
        cx.mark_dirty();
        cx.refresh();
        parts.push(FloorDxf {
            name: cx.project.floors[i].name.clone(),
            elevation: cx.project.floors[i].elevation,
            text: floor_dxf(cx),
        });
    }
    cx.floor = saved;
    cx.mark_dirty();
    cx.refresh();
    let solids = if opts.three_d {
        model_solids(cx)
    } else {
        Vec::new()
    };
    build(&parts, &solids, &cx.project.layers, opts)
}

/// The 3D model as triangles in plan coordinates (the 3D view's scene is
/// Y up with Z = minus the plan's Y).
pub fn model_solids(cx: &EditorContext) -> Vec<SolidDxf> {
    let scene = crate::shell::layout_window::view_scene(&cx.project);
    scene
        .meshes
        .iter()
        .filter(|m| !m.indices.is_empty())
        .map(|m| {
            let at = |i: u32| {
                let p = m.vertices[i as usize].position;
                [f64::from(p[0]), -f64::from(p[2]), f64::from(p[1])]
            };
            SolidDxf {
                layer: m.material.name().to_string(),
                triangles: m
                    .indices
                    .chunks_exact(3)
                    .map(|t| [at(t[0]), at(t[1]), at(t[2])])
                    .collect(),
            }
        })
        .collect()
}

thread_local! {
    static DIALOG: RefCell<Option<DxfExportDialog>> = const { RefCell::new(None) };
    /// The fields as they were left, so the next export starts there.
    static LAST: RefCell<Option<DxfExportDialog>> = const { RefCell::new(None) };
}

/// File > Export > DXF: opens the options window.
pub fn open(cx: &EditorContext) {
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(DxfExportDialog::new(cx));
        }
    });
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn close() {
    DIALOG.with(|d| *d.borrow_mut() = None);
}

fn save(name: &str, text: &str) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter("dxf", &["dxf"])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, text.as_bytes()) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

impl DxfExportDialog {
    /// Draws the window; false once it is closed.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        let mut open = true;
        let mut export = false;
        let mut cancel = false;
        egui::Window::new("DXF Export Options")
            .id(egui::Id::new("dxf_export_options"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("dxf_options_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Units");
                        egui::ComboBox::from_id_salt("dxf_units")
                            .selected_text(self.opts.units.label())
                            .show_ui(ui, |ui| {
                                for u in DxfUnits::ALL {
                                    ui.selectable_value(&mut self.opts.units, u, u.label());
                                }
                            });
                        ui.end_row();
                        ui.label("Layer names");
                        ui.horizontal(|ui| {
                            ui.radio_value(
                                &mut self.opts.naming,
                                LayerNaming::Chief,
                                "Plan layer names",
                            );
                            ui.radio_value(&mut self.opts.naming, LayerNaming::Aia, "AIA standard");
                        });
                        ui.end_row();
                        ui.label("Line weights");
                        ui.checkbox(
                            &mut self.opts.line_weights,
                            "Write the weight of each layer",
                        );
                        ui.end_row();
                        ui.label("Text");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.opts.text, TextMode::Text, "As text");
                            ui.radio_value(&mut self.opts.text, TextMode::Lines, "As lines");
                        });
                        ui.end_row();
                        ui.label("Floors");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.floors, FloorChoice::ThisFloor, "This floor");
                            ui.radio_value(&mut self.floors, FloorChoice::AllFloors, "All floors");
                            ui.radio_value(&mut self.floors, FloorChoice::Picked, "Pick...");
                        });
                        ui.end_row();
                        ui.label("");
                        ui.checkbox(&mut self.opts.floor_layers, "Floor name on each layer");
                        ui.end_row();
                        ui.label("Drawing");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.opts.three_d, false, "2D plan");
                            ui.radio_value(
                                &mut self.opts.three_d,
                                true,
                                "3D (floors at their elevation + model)",
                            );
                        });
                        ui.end_row();
                    });
                if self.floors == FloorChoice::Picked {
                    ui.separator();
                    for (i, f) in cx.project.floors.iter().enumerate() {
                        if let Some(p) = self.picked.get_mut(i) {
                            ui.checkbox(p, &f.name);
                        }
                    }
                }
                ui.separator();
                ui.label("Layer map (plan layer = file layer, one per line)");
                ui.add(
                    egui::TextEdit::multiline(&mut self.map_text)
                        .desired_rows(3)
                        .desired_width(320.0),
                );
                ui.separator();
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
                ui.horizontal(|ui| {
                    export = ui.button("Export...").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if export {
            match self.text(cx) {
                Ok(text) => {
                    let name = format!("{} - {}.dxf", cx.project.name, cx.floor().name);
                    self.message = save(&name, &text);
                    cx.status = self.message.clone();
                    LAST.with(|l| *l.borrow_mut() = Some(self.clone()));
                    return false;
                }
                Err(e) => self.message = e,
            }
        }
        LAST.with(|l| *l.borrow_mut() = Some(self.clone()));
        open && !cancel
    }
}

/// Draws the window when it is open; the shell calls it once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) {
        if d.show(ctx, cx) {
            DIALOG.with(|slot| {
                if slot.borrow().is_none() {
                    *slot.borrow_mut() = Some(d);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::{Point, WallKind};

    fn house() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 96.0, WallKind::Exterior);
        }
        cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::cad::CadItem::Text {
                pos: Point::new(10.0, 10.0),
                text: "Den".into(),
                height: 6.0,
                angle: 0.0,
            },
        );
        cx.refresh();
        cx
    }

    #[test]
    fn the_layer_map_text_is_read_line_by_line() {
        let m = parse_map("Walls, Normal = A-WALL\nnonsense\n  = x\nDoors=A-DOOR\n");
        assert_eq!(
            m,
            vec![
                ("Walls, Normal".to_string(), "A-WALL".to_string()),
                ("Doors".to_string(), "A-DOOR".to_string())
            ]
        );
    }

    #[test]
    fn the_fields_decide_what_is_exported() {
        let mut cx = house();
        close();
        let mut d = DxfExportDialog::new(&cx);
        assert_eq!(d.floor_indices(&cx), vec![0]);
        d.opts.units = DxfUnits::Millimeters;
        d.opts.naming = LayerNaming::Aia;
        d.opts.text = TextMode::Lines;
        d.opts.line_weights = true;
        let text = d.text(&mut cx).unwrap();
        assert!(text.contains("A-WALL"), "AIA names");
        assert!(!text.lines().any(|l| l == "TEXT"), "text drawn as lines");
        assert!(text.lines().any(|l| l == "370"), "layer weights");
        let lines: Vec<&str> = text.lines().collect();
        let i = lines.iter().position(|l| *l == "$INSUNITS").unwrap();
        assert_eq!(lines[i + 2], "4", "millimeters");
        // The plan itself is untouched by the export.
        assert_eq!(cx.floor, 0);
        d.floors = FloorChoice::Picked;
        d.picked = vec![false];
        assert!(d.text(&mut cx).is_err());
    }

    #[test]
    fn two_floors_in_3d_carry_the_model_and_the_elevations() {
        let mut cx = house();
        let up = cx.project.floors[0].clone();
        let mut up = up;
        up.name = "2nd Floor".into();
        up.elevation = 108.0;
        cx.project.floors.push(up);
        cx.refresh();
        let mut d = DxfExportDialog::new(&cx);
        d.floors = FloorChoice::AllFloors;
        d.opts.three_d = true;
        let text = d.text(&mut cx).unwrap();
        assert!(
            text.contains("1st Floor - Walls, Normal")
                && text.contains("2nd Floor - Walls, Normal")
        );
        assert!(
            text.lines().filter(|l| *l == "3DFACE").count() > 0,
            "the model is faces"
        );
        assert!(text.contains("108.0000"));
        let flat = {
            d.opts.three_d = false;
            d.text(&mut cx).unwrap()
        };
        assert_eq!(flat.lines().filter(|l| *l == "3DFACE").count(), 0);
    }

    #[test]
    fn the_window_opens_draws_and_remembers() {
        let mut cx = house();
        close();
        assert!(!is_open());
        open(&cx);
        assert!(is_open());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show(ctx, &mut cx));
        }
        assert!(is_open());
        close();
    }
}
