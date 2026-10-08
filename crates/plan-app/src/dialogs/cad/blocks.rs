//! CAD Block Management and the CAD Block Specification (Edit CAD Block).
//!
//! A CAD block is a group of CAD objects with a name, an insertion point (the
//! point that lands on the cursor when the block is inserted) and an optional
//! arrow backoff point (CAD-31, CAD-32). The management dialog lists the
//! blocks of the plan; the specification edits one.
//!
//! The CAD tool shows both from `Tool::frame` and applies what they return.

#![allow(dead_code)]

use crate::dialogs::{
    on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK,
};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::cad::{CadBlockInfo, CadItem};
use plan_core::geometry::Point;
use plan_core::Id;
use std::f64::consts::TAU;

const BLOCK_TABS: &[Tab] = &[on("General"), on("Insertion Point"), on("Arrow Backoff")];

/// One line of the management list.
#[derive(Clone, Debug)]
pub struct BlockRow {
    pub info: CadBlockInfo,
    /// How many CAD objects the block holds.
    pub objects: usize,
}

/// What the management dialog asks the tool to do.
#[derive(Clone, Debug, PartialEq)]
pub enum ManagerAction {
    None,
    Close,
    Rename(Id, String),
    Edit(Id),
    Insert(Id),
    Explode(Id),
    Delete(Id),
}

/// The CAD Block Management window.
#[derive(Default)]
pub struct BlockManager {
    selected: Option<Id>,
    /// The name being typed for the selected block.
    name: String,
}

impl BlockManager {
    pub fn show(&mut self, ctx: &egui::Context, rows: &[BlockRow]) -> ManagerAction {
        let mut action = ManagerAction::None;
        let mut open = true;
        if let Some(sel) = self.selected {
            if !rows.iter().any(|r| r.info.group == sel) {
                self.selected = None;
            }
        }
        egui::Window::new("CAD Block Management")
            .id(egui::Id::new("cad_block_manager"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([480.0, 340.0])
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                if rows.is_empty() {
                    ui.label(
                        "There are no CAD blocks in this plan. Select two or more CAD objects \
                         and use Make CAD Block.",
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("cad_block_list")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for r in rows {
                                let label = format!(
                                    "{}   ({} object{})",
                                    r.info.name,
                                    r.objects,
                                    if r.objects == 1 { "" } else { "s" }
                                );
                                let on = self.selected == Some(r.info.group);
                                if ui.selectable_label(on, label).clicked() {
                                    self.selected = Some(r.info.group);
                                    self.name = r.info.name.clone();
                                }
                            }
                        });
                }
                ui.separator();
                let Some(sel) = self.selected else {
                    ui.weak("Pick a block to rename, edit, insert, explode or delete it.");
                    return;
                };
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.name);
                    let renamed = !self.name.trim().is_empty()
                        && rows
                            .iter()
                            .find(|r| r.info.group == sel)
                            .is_some_and(|r| r.info.name != self.name.trim());
                    if ui
                        .add_enabled(renamed, egui::Button::new("Rename"))
                        .clicked()
                    {
                        action = ManagerAction::Rename(sel, self.name.trim().to_string());
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Edit\u{2026}").clicked() {
                        action = ManagerAction::Edit(sel);
                    }
                    if ui.button("Insert").clicked() {
                        action = ManagerAction::Insert(sel);
                    }
                    if ui.button("Explode").clicked() {
                        action = ManagerAction::Explode(sel);
                    }
                    if ui.button("Delete").clicked() {
                        action = ManagerAction::Delete(sel);
                    }
                });
            });
        if !open || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            return ManagerAction::Close;
        }
        action
    }
}

/// CAD Block Specification: name, insertion point and arrow backoff point.
pub struct BlockEditDialog {
    frame: SpecDialog,
    form: BlockForm,
}

struct BlockForm {
    orig: CadBlockInfo,
    draft: CadBlockInfo,
    bounds: (Point, Point),
    items: Vec<CadItem>,
    fields: Fields,
}

impl BlockEditDialog {
    pub fn new(info: CadBlockInfo, bounds: (Point, Point), items: Vec<CadItem>) -> Self {
        Self {
            frame: SpecDialog::new("CAD Block Specification", "cad_block"),
            form: BlockForm {
                orig: info.clone(),
                draft: info,
                bounds,
                items,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &CadBlockInfo {
        &self.form.draft
    }

    pub fn id(&self) -> Id {
        self.form.orig.group
    }

    /// Edits the draft directly (tests).
    pub fn draft_mut(&mut self) -> &mut CadBlockInfo {
        &mut self.form.draft
    }
}

impl BlockForm {
    fn center(&self) -> Point {
        Point::lerp(self.bounds.0, self.bounds.1, 0.5)
    }

    /// A checkbox with X and Y fields for an optional point.
    fn point_page(
        ui: &mut Ui,
        fields: &mut Fields,
        point: &mut Option<Point>,
        default: Point,
        title: &str,
        key: (&'static str, &'static str),
    ) {
        section(ui, title);
        let mut set = point.is_some();
        if ui.checkbox(&mut set, "Use this point").changed() {
            *point = set.then_some(default);
        }
        if let Some(p) = point {
            fields.length_row(ui, "X", key.0, &mut p.x);
            fields.length_row(ui, "Y", key.1, &mut p.y);
            if ui.button("Reset to the middle of the block").clicked() {
                *p = default;
            }
        }
    }
}

impl SpecPages for BlockForm {
    fn tabs(&self) -> &'static [Tab] {
        BLOCK_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.draft.name.trim().is_empty() {
            return Some("Enter a name for the block".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        let center = self.center();
        match BLOCK_TABS[tab].name {
            "General" => {
                section(ui, "CAD Block");
                row(ui, "Name", |ui| {
                    ui.text_edit_singleline(&mut self.draft.name);
                });
                row(ui, "Objects", |ui| {
                    ui.label(self.items.len().to_string());
                });
                let size = self.bounds.1.sub(self.bounds.0);
                row(ui, "Size", |ui| {
                    ui.label(format!(
                        "{} x {}",
                        crate::dialogs::fmt_short(size.x),
                        crate::dialogs::fmt_short(size.y)
                    ));
                });
            }
            "Insertion Point" => Self::point_page(
                ui,
                &mut self.fields,
                &mut self.draft.insertion,
                center,
                "Insertion Point",
                ("ins_x", "ins_y"),
            ),
            _ => Self::point_page(
                ui,
                &mut self.fields,
                &mut self.draft.backoff,
                center,
                "Arrow Backoff Point",
                ("back_x", "back_y"),
            ),
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let (lo, hi) = self.bounds;
        let w = (hi.x - lo.x).max(1.0);
        let h = (hi.y - lo.y).max(1.0);
        let k = (f64::from(rect.width()) / w).min(f64::from(rect.height()) / h) * 0.85;
        let mid = Point::lerp(lo, hi, 0.5);
        let to = |q: Point| {
            Pos2::new(
                rect.center().x + ((q.x - mid.x) * k) as f32,
                rect.center().y - ((q.y - mid.y) * k) as f32,
            )
        };
        let ink = Stroke::new(1.2_f32, PV_INK);
        for item in &self.items {
            match item {
                CadItem::Line { a, b } => {
                    p.line_segment([to(*a), to(*b)], ink);
                }
                CadItem::Polyline { points, closed } => {
                    let mut v: Vec<Pos2> = points.iter().map(|q| to(*q)).collect();
                    if *closed {
                        if let Some(f) = v.first().copied() {
                            v.push(f);
                        }
                    }
                    p.add(egui::Shape::line(v, ink));
                }
                CadItem::Circle { center, radius } => {
                    p.circle_stroke(to(*center), (*radius * k) as f32, ink);
                }
                CadItem::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    let sweep = (end_angle - start_angle).rem_euclid(TAU);
                    let v: Vec<Pos2> = (0..=24)
                        .map(|i| {
                            let a = start_angle + sweep * f64::from(i) / 24.0;
                            to(Point::new(
                                center.x + radius * a.cos(),
                                center.y + radius * a.sin(),
                            ))
                        })
                        .collect();
                    p.add(egui::Shape::line(v, ink));
                }
                CadItem::Text { pos, .. } => {
                    p.circle_filled(to(*pos), 1.5, PV_FAINT);
                }
            }
        }
        let mark = Stroke::new(1.6_f32, PV_ACCENT);
        if let Some(i) = self.draft.insertion {
            let c = to(i);
            p.circle_stroke(c, 6.0, mark);
            p.line_segment([c - egui::vec2(9.0, 0.0), c + egui::vec2(9.0, 0.0)], mark);
            p.line_segment([c - egui::vec2(0.0, 9.0), c + egui::vec2(0.0, 9.0)], mark);
        }
        if let Some(b) = self.draft.backoff {
            let c = to(b);
            p.line_segment([c - egui::vec2(6.0, 6.0), c + egui::vec2(6.0, 6.0)], mark);
            p.line_segment([c - egui::vec2(6.0, -6.0), c + egui::vec2(6.0, -6.0)], mark);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> CadBlockInfo {
        CadBlockInfo {
            group: 7,
            name: "Chair".into(),
            insertion: Some(Point::new(5.0, 5.0)),
            backoff: None,
        }
    }

    fn items() -> Vec<CadItem> {
        vec![
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
            CadItem::Circle {
                center: Point::new(5.0, 5.0),
                radius: 3.0,
            },
        ]
    }

    #[test]
    fn edit_dialog_draws_every_tab_and_validates_the_name() {
        let mut d = BlockEditDialog::new(info(), (Point::ZERO, Point::new(10.0, 10.0)), items());
        let ctx = egui::Context::default();
        for tab in 0..BLOCK_TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, tab));
                d.show(ctx);
            });
        }
        assert!(d.form.error().is_none());
        d.draft_mut().name = "  ".into();
        assert!(d.form.error().is_some());
        d.draft_mut().name = "Armchair".into();
        d.draft_mut().backoff = Some(Point::new(1.0, 1.0));
        assert_eq!(d.draft().name, "Armchair");
        assert_eq!(d.id(), 7);
    }

    #[test]
    fn manager_shows_empty_and_listed_blocks() {
        let ctx = egui::Context::default();
        let mut m = BlockManager::default();
        let mut last = ManagerAction::None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            last = m.show(ctx, &[]);
        });
        assert_eq!(last, ManagerAction::None);
        let rows = vec![BlockRow {
            info: info(),
            objects: 2,
        }];
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            last = m.show(ctx, &rows);
        });
        assert_eq!(last, ManagerAction::None);
    }
}
