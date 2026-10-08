//! plan-docs: construction-document outputs generated from a plan.
//!
//! Everything here is headless and dependency-free (beyond `plan-core`):
//!
//! * [`schedule`]: door, window, room and wall schedules with CSV and
//!   Markdown export.
//! * [`materials`]: a framing / finish quantity take-off and its CSV export.
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

pub use materials::{materials_list, to_csv as materials_to_csv, MaterialLine};
pub use pdf::{
    plan_sheet, plan_sheet_with, LineCap, LineJoin, PdfColor, PdfDoc, PlanSheetOptions,
    PlanSheetResult, RoomAreaBasis, Scale, SheetSize, TitleBlock, CHIEF_SHEET_BACKGROUND,
};
pub use schedule::{
    door_schedule, room_name, room_schedule, wall_schedule, window_schedule, Schedule,
};

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
