//! plan-docs: construction-document outputs generated from a plan.
//!
//! Everything here is headless (it reads the plan through `plan-core` and the
//! object crates for cabinets, electrical, framing and the library):
//!
//! * [`schedule`]: door, window, room and wall schedules with CSV and
//!   Markdown export.
//! * [`schedule_kinds`]: rows, columns and callout labels for the schedules
//!   placed in the plan (`plan_core::schedules`): door, window, room, wall,
//!   cabinet, electrical, framing, fixture, furniture, plant and general.
//! * [`materials`]: a framing / finish quantity take-off and its CSV export.
//! * [`terrain_report`]: the terrain's cut/fill table (cubic yards per graded pad and in
//!   total) and the soil lines of the Materials List.
//! * [`pdf`]: a PDF 1.4 writer (RGB colour, dashes, clipping, rotated and
//!   bold text, Bezier curves, hatches, embedded RGB images, mixed page
//!   sizes) plus [`pdf::plan_sheet`], a scaled floor-plan sheet with a
//!   Chief-style title block, walls, openings, dimensions and CAD items.
//!
//! Units follow `plan-core`: lengths are inches. Floors are addressed by
//! index into `Project::floors`, as in `Project::add_wall`.

pub mod materials;
pub mod pdf;
pub mod schedule;
pub mod schedule_kinds;
pub mod terrain_report;
pub mod xlsx;

pub use materials::{
    fmt_money, materials_list, materials_report, price_keys, row_cells as materials_cells,
    to_csv as materials_to_csv, to_schedule as materials_to_schedule,
    total_price as materials_total, MasterItem, MasterList, MaterialLine, MaterialsScope,
    CATEGORIES as MATERIAL_CATEGORIES, COLUMNS as MATERIAL_COLUMNS,
};
pub use pdf::{
    plan_sheet, plan_sheet_with, LineCap, LineJoin, PdfColor, PdfColorMode, PdfDoc,
    PlanSheetOptions, PlanSheetResult, RoomAreaBasis, Scale, SheetSize, TitleBlock,
    CHIEF_SHEET_BACKGROUND,
};
pub use schedule::{
    door_schedule, room_name, room_schedule, wall_schedule, window_schedule, Schedule,
};
pub use xlsx::{materials_to_xlsx, sheet_name as xlsx_sheet_name};

#[cfg(test)]
pub(crate) mod test_support {
    use plan_core::{OpeningKind, Point, Project, WallKind};

    /// A closed rectangle of four walls starting at the origin. Returns the
    /// ids in order bottom, right, top, left.
    pub fn rect_walls(p: &mut Project, w: f64, h: f64, thickness: f64, kind: WallKind) -> [u64; 4] {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ];
        let mut ids = [0; 4];
        for i in 0..4 {
            ids[i] = p.add_wall(0, c[i], c[(i + 1) % 4], thickness, 109.125, kind);
        }
        ids
    }

    /// A 40' x 30' exterior box with one door and two windows.
    pub fn house() -> Project {
        let mut p = Project::new("House");
        let ids = rect_walls(&mut p, 480.0, 360.0, 6.5, WallKind::Exterior);
        p.add_opening(0, ids[0], 240.0, OpeningKind::Door).unwrap();
        p.add_opening(0, ids[0], 100.0, OpeningKind::Window)
            .unwrap();
        p.add_opening(0, ids[2], 240.0, OpeningKind::Window)
            .unwrap();
        p
    }
}
