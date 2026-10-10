//! Chief-parity scenario tests (QA pass).
//!
//! Each file drives the real tools through pointer and key events, the way
//! the application shell does (`PlanApp::dispatch_pointer`), and then checks
//! the model, the undo stack, the dialog requests, the 3D scene and the
//! documents. No window is opened: dialogs are exercised through an
//! `egui::Context` that runs headless frames.
//!
//! [`Sim`] is the shared harness. Findings that these scenarios turned up are
//! written to `docs/qa-findings.md`; a test that exposes one is marked
//! `#[ignore = "QA-nn"]` so the gate stays green.
#![cfg(test)]
#![allow(dead_code)]

mod r14_integration;
mod s01_house_shell;
mod s02_openings;
mod s03_interior;
mod s04_cabinets;
mod s05_stairs_floors;
mod s06_roof;
mod s07_dimensions_text_cad;
mod s08_electrical_terrain;
mod s09_scene_3d;
mod s10_documents;
mod s11_every_tool;
mod s12_hotkeys;
mod s13_opening_variants;
mod s14_wall_edit_and_snaps;
mod s15_edit_commands;
mod s16_walls_typed_and_edit;
mod s17_dimensions;
mod s18_doors_windows_3d;
mod s19_rooms_floors;
mod s20_roofs_3d;
mod s21_layout_print;
mod s22_files;
mod s23_cabinets_underlays_prefs;
mod s24_view3d_picking_textures;
mod s25_walls_r14;
mod s26_openings_r14;
mod s27_text_dims_r14;
mod s28_roofs_r14;
mod s29_rooms_r14;
mod s30_select_r14;
mod s31_cab_stairs_r14;
mod s32_electrical_r14;
mod s33_framing_r14;
mod s34_cameras_r14;
mod s35_tool_dialog_sweep;
mod s36_terrain_r14;
mod s37_materials_r14;
mod s38_details_r14;
mod s39_prefs_r14;
mod s40_roundtrips;
mod s41_code_minimums;
mod s42_opening_tabs;
mod s43_import3d_export_picture;
mod s44_decks_chimneys;
mod s45_painters_spell;
mod s46_nkba_calculators;
mod s47_defaults_shell;
mod s48_excel_roundtrip;
mod s49_walls_r15;
mod s50_cameras_r15;
mod s51_cad_r15;
mod s52_library_r15;
mod s53_material_packages;
mod s54_dimensions_r15;
mod s55_stairs_r15;
mod s56_callouts_r15;
mod s57_dxf_import;
mod s58_integration2_layout;
mod s58_materials_list_r15;
mod s59_terrain_r15;
mod s60_blocks_solids_r15;
mod s60_layout_pages;
mod s61_layout_boxes;
mod s62_print_watermark;
mod s63_integration2_shell;
mod s64_common_pages;
mod s65_survey_entry;
mod s66_construction_reference;
mod s67_line_fill_poche;
mod s68_layers;
mod s69_edit_behaviours;
mod s70_plan_agent;
mod s71_wall_types;
mod s73_wall_edit_tools;
mod s74_rooms;
mod s75_integration2_a;
mod s75_tray_ceilings;
mod s77_roof_eaves;
mod s78_roof_baselines;
mod s79_roof_trim;
mod s80_stairs_engine;
mod s81_openings_mulled_bay;
mod s82_cabinet_runs;
mod s83_cabinet_faces;
mod s84_layered_assemblies;
mod s87_dimension_segments;
mod s88_framing_members;
mod s88_schedules;
mod s89_framing_layout;
mod s90_moldings;
mod s91_camera_sections;
mod s91_electrical;
mod s92_floors_foundation;
mod s92_terrain_site;
mod s94_text_macros_rescheck;
mod s97_saved_defaults_views;
mod tutorials_a;
mod tutorials_support;

use crate::editor::{EditorContext, EditorRequest, ObjectRef};
use crate::tools::{KeyEvent, PointerEvent, ToolId, ToolResult};
use crate::PlanApp;
use eframe::egui;
use plan_core::geometry::Point;

/// The application without a window.
pub struct Sim {
    pub app: PlanApp,
    pub ctx: egui::Context,
    /// Every tool result, in order.
    pub results: Vec<ToolResult>,
    /// Every request the tools queued (before the shell handled it).
    pub requests: Vec<EditorRequest>,
}

impl Sim {
    pub fn new() -> Sim {
        let app = PlanApp::new(
            crate::theme::AppSettings::default(),
            crate::plan_defaults::embedded(),
            None,
        );
        Sim {
            app,
            ctx: egui::Context::default(),
            results: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn cx(&mut self) -> &mut EditorContext {
        &mut self.app.cx
    }

    pub fn floor_walls(&self) -> usize {
        self.app.cx.floor().walls.len()
    }

    /// Activates a tool the way a toolbar click does.
    pub fn tool(&mut self, id: ToolId) {
        self.app.apply(crate::toolbar::Action::SetTool(id));
    }

    /// What the shell does after every tool call.
    fn finish(&mut self, res: ToolResult) -> ToolResult {
        self.requests.extend(self.app.cx.requests.iter().copied());
        self.app.finish_tool_call(&self.ctx, &res);
        self.app.cx.refresh();
        self.results.push(res.clone());
        res
    }

    fn event(&self, x: f64, y: f64) -> PointerEvent {
        PointerEvent::at(&self.app.cx, Point::new(x, y))
    }

    pub fn move_to(&mut self, x: f64, y: f64) -> ToolResult {
        let ev = self.event(x, y);
        let res = self
            .app
            .tools
            .active_mut()
            .pointer_move(&mut self.app.cx, ev);
        self.finish(res)
    }

    pub fn down(&mut self, x: f64, y: f64) -> ToolResult {
        let ev = self.event(x, y).with_down(true);
        let res = self
            .app
            .tools
            .active_mut()
            .pointer_down(&mut self.app.cx, ev);
        self.finish(res)
    }

    pub fn up(&mut self, x: f64, y: f64) -> ToolResult {
        let ev = self.event(x, y);
        let res = self.app.tools.active_mut().pointer_up(&mut self.app.cx, ev);
        self.finish(res)
    }

    /// Hover, press and release at one point.
    pub fn click(&mut self, x: f64, y: f64) -> ToolResult {
        self.move_to(x, y);
        let d = self.down(x, y);
        let u = self.up(x, y);
        if d.commit.is_some() || (d.consumed && !u.consumed) {
            d
        } else {
            u
        }
    }

    /// Press at `a`, drag to `b`, release (one wall for the wall tools).
    pub fn drag(&mut self, a: (f64, f64), b: (f64, f64)) -> ToolResult {
        self.move_to(a.0, a.1);
        self.down(a.0, a.1);
        let ev = self.event(b.0, b.1).with_down(true);
        let mv = self
            .app
            .tools
            .active_mut()
            .pointer_move(&mut self.app.cx, ev);
        self.finish(mv);
        self.up(b.0, b.1)
    }

    /// The shell sends a double-click as `double_click`, then (when the tool
    /// did not take it) `pointer_down`.
    pub fn double_click(&mut self, x: f64, y: f64) -> ToolResult {
        self.move_to(x, y);
        let ev = self.event(x, y).with_down(true);
        let res = self
            .app
            .tools
            .active_mut()
            .double_click(&mut self.app.cx, ev);
        let res = self.finish(res);
        if res.consumed {
            return res;
        }
        self.down(x, y);
        self.up(x, y)
    }

    pub fn key(&mut self, k: KeyEvent) -> ToolResult {
        let res = self.app.tools.active_mut().key(&mut self.app.cx, k.clone());
        let res = self.finish(res);
        // The shell's defaults for an unconsumed key.
        if !res.consumed {
            if k.is(egui::Key::Escape) && self.app.tools.active_id() != ToolId::Select {
                self.app.set_tool(ToolId::Select);
            } else if k.is(egui::Key::Delete) || k.is(egui::Key::Backspace) {
                self.app.cx.delete_selection();
            }
        }
        res
    }

    pub fn esc(&mut self) -> ToolResult {
        self.key(KeyEvent::escape())
    }

    /// A menu or toolbar command.
    pub fn action(&mut self, a: crate::toolbar::Action) {
        self.app.apply(a);
        self.app.process_requests();
        self.app.cx.refresh();
    }

    pub fn undo(&mut self) -> Option<String> {
        self.app.cx.undo()
    }

    pub fn redo(&mut self) -> Option<String> {
        self.app.cx.redo()
    }

    /// Runs one headless frame of the dialogs, optionally with Enter (OK).
    pub fn dialog_frame(&mut self, enter: bool) {
        self.dialog_frame_key(enter.then_some(egui::Key::Enter));
    }

    /// One headless frame of the dialogs with `key` pressed (Enter = OK,
    /// Escape = Cancel).
    pub fn dialog_frame_key(&mut self, key: Option<egui::Key>) {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        if let Some(key) = key {
            input.events.push(egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let ctx = self.ctx.clone();
        let mut cam = self.app.camera;
        let _ = ctx.run(input, |ctx| {
            // The canvas: tools that own dialogs draw them from their overlay.
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (resp, painter) =
                        ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                    cam.rect = resp.rect;
                    self.app
                        .tools
                        .active()
                        .draw_overlay(&self.app.cx, &painter, &cam);
                });
            self.app.dialogs(ctx);
            crate::dialogs::build_tools::show_all(ctx, &mut self.app.cx, &mut cam);
            crate::dialogs::exchange::show_all(ctx, &mut self.app.cx);
        });
        self.app.camera = cam;
        // What the shell does at the top of its next frame.
        self.app.tools.frame(&mut self.app.cx, &ctx);
        self.app.process_requests();
        self.app.cx.refresh();
    }

    /// One headless frame of the plan canvas; returns every shape drawn, in
    /// order (nested shape lists flattened).
    pub fn plan_shapes(&mut self) -> Vec<egui::Shape> {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let ctx = self.ctx.clone();
        let mut cam = self.app.camera;
        self.app.cx.refresh();
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let (resp, painter) =
                        ui.allocate_painter(ui.available_size(), egui::Sense::hover());
                    cam.rect = resp.rect;
                    crate::editor::render::draw_plan(&self.app.cx, &painter, &cam);
                });
        });
        let mut leaves = Vec::new();
        for clipped in out.shapes {
            flatten(clipped.shape, &mut leaves);
        }
        leaves
    }

    /// Draws a dialog for two frames (so it lays out) and returns whether it
    /// is still open, then presses Enter and returns whether it closed.
    pub fn open_then_ok(&mut self) -> (bool, bool) {
        self.dialog_frame(false);
        self.dialog_frame(false);
        let still_open = self.app.has_dialog();
        self.dialog_frame(true);
        self.dialog_frame(false);
        (still_open, !self.app.has_dialog())
    }

    /// Closes the open dialog with OK (two layout frames, then Enter).
    pub fn ok(&mut self) {
        self.dialog_frame(false);
        self.dialog_frame(false);
        self.dialog_frame(true);
        self.dialog_frame(false);
    }

    /// Closes the open dialog with Cancel (Escape).
    pub fn cancel(&mut self) {
        self.dialog_frame(false);
        self.dialog_frame(false);
        self.dialog_frame_key(Some(egui::Key::Escape));
        self.dialog_frame(false);
    }

    /// Selects `o` and asks for its specification the way Select Objects does
    /// on Enter, then lets the shell open it.
    pub fn open_spec(&mut self, o: ObjectRef) -> bool {
        self.app.cx.selection.set(o);
        self.app.cx.requests.push(EditorRequest::OpenSpec(o));
        self.app.process_requests();
        self.app.has_dialog()
    }

    pub fn wall_ids(&self) -> Vec<plan_core::Id> {
        self.app.cx.floor().walls.iter().map(|w| w.id).collect()
    }
}

/// Draws the exterior shell as four click-drag walls with deliberately
/// imprecise ends (a few inches off), like a hand-drawn plan. Returns the
/// number of walls.
pub fn draw_shell(sim: &mut Sim, w: f64, h: f64) -> usize {
    use plan_core::WallKind;
    sim.tool(ToolId::Wall {
        kind: WallKind::Exterior,
    });
    sim.drag((0.0, 0.0), (w + 1.0, 1.0));
    sim.drag((w + 2.0, 2.0), (w - 1.0, h + 1.0));
    sim.drag((w + 1.0, h - 1.0), (2.0, h + 2.0));
    sim.drag((1.0, h + 1.0), (1.0, 2.0));
    sim.app.cx.refresh();
    sim.floor_walls()
}

/// Debug-text equality for model types without `PartialEq`.
pub fn same<T: std::fmt::Debug>(a: &T, b: &T) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

fn flatten(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(v) => v.into_iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// Every solid fill and stroke color a shape draws with.
pub fn shape_colors(shape: &egui::Shape) -> Vec<egui::Color32> {
    use egui::epaint::ColorMode;
    let mut out = Vec::new();
    let path_color = |c: &ColorMode, out: &mut Vec<egui::Color32>| {
        if let ColorMode::Solid(c) = c {
            out.push(*c);
        }
    };
    match shape {
        egui::Shape::LineSegment { stroke, .. } => out.push(stroke.color),
        egui::Shape::Path(p) => {
            out.push(p.fill);
            path_color(&p.stroke.color, &mut out);
        }
        egui::Shape::Rect(r) => {
            out.push(r.fill);
            out.push(r.stroke.color);
        }
        egui::Shape::Circle(c) => {
            out.push(c.fill);
            out.push(c.stroke.color);
        }
        _ => {}
    }
    out.retain(|c| c.a() > 0);
    out
}
