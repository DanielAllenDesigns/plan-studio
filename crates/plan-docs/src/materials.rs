//! Materials / quantity take-off.
//!
//! Quantities are derived directly from the plan model and are deliberately
//! simple (no waste factor, no cut-list optimisation):
//!
//! * **Studs** at 16" o.c. per wall: `ceil(length / 16) + 1`, plus 2 kings and
//!   2 trimmers for every opening hosted in the wall. Exterior walls are
//!   2x6, interior walls 2x4.
//! * **Plates**: bottom plate plus doubled top plate, so wall length x 3, in
//!   linear feet (rounded up per wall kind).
//! * **Drywall** (4x8 sheets = 32 sq ft): net wall area (openings deducted),
//!   both sides of interior walls and the inside face of exterior walls.
//! * **Sheathing** sheets and **siding** square feet: net exterior wall area.
//! * **Flooring** square feet and **ceiling drywall** sheets, one line per room.
//! * **Doors and windows** counted by size.

use crate::schedule::{push_csv_row, room_name};
use plan_core::units::fmt_ft_in;
use plan_core::{OpeningKind, Project, Room, WallKind};
use std::collections::BTreeMap;

/// Square feet in a 4x8 sheet.
const SHEET_SQ_FT: f64 = 32.0;
/// Stud spacing, inches on centre.
const STUD_SPACING: f64 = 16.0;

/// One row of the take-off.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialLine {
    pub category: String,
    pub item: String,
    pub quantity: f64,
    pub unit: String,
}

fn line(category: &str, item: impl Into<String>, quantity: f64, unit: &str) -> MaterialLine {
    MaterialLine {
        category: category.to_string(),
        item: item.into(),
        quantity,
        unit: unit.to_string(),
    }
}

/// Build the take-off for one floor. `rooms` supplies the flooring and
/// ceiling lines. An out-of-range `floor` yields an empty list.
pub fn materials_list(project: &Project, floor: usize, rooms: &[Room]) -> Vec<MaterialLine> {
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };

    let mut studs = [0.0_f64; 2]; // [exterior 2x6, interior 2x4]
    let mut plate_lf = [0.0_f64; 2];
    let mut drywall_sq_ft = 0.0;
    let mut exterior_sq_ft = 0.0;
    let mut doors: BTreeMap<(i64, i64), f64> = BTreeMap::new();
    let mut windows: BTreeMap<(i64, i64), f64> = BTreeMap::new();

    for w in &f.walls {
        let len = w.length();
        let openings: Vec<_> = f.openings_on(w.id).collect();
        let k = usize::from(w.kind == WallKind::Interior);
        studs[k] += (len / STUD_SPACING).ceil() + 1.0 + 4.0 * openings.len() as f64;
        plate_lf[k] += len / 12.0 * 3.0;

        let opening_sq_ft: f64 = openings.iter().map(|o| o.width * o.height / 144.0).sum();
        let net = (len * w.height / 144.0 - opening_sq_ft).max(0.0);
        match w.kind {
            WallKind::Exterior => {
                exterior_sq_ft += net;
                drywall_sq_ft += net;
            }
            WallKind::Interior => drywall_sq_ft += 2.0 * net,
        }
        for o in openings {
            // Key sizes at 1/16" so equal sizes group and order stably.
            let key = (
                (o.width * 16.0).round() as i64,
                (o.height * 16.0).round() as i64,
            );
            let map = match o.kind {
                OpeningKind::Door => &mut doors,
                OpeningKind::Window => &mut windows,
            };
            *map.entry(key).or_insert(0.0) += 1.0;
        }
    }

    let mut out = Vec::new();
    let names = ["2x6 Stud @ 16\" o.c.", "2x4 Stud @ 16\" o.c."];
    let plates = [
        "2x6 Plate (1 bottom + 2 top)",
        "2x4 Plate (1 bottom + 2 top)",
    ];
    for k in 0..2 {
        if studs[k] > 0.0 {
            out.push(line("Framing", names[k], studs[k], "ea"));
        }
    }
    for k in 0..2 {
        if plate_lf[k] > 0.0 {
            out.push(line("Framing", plates[k], plate_lf[k].ceil(), "lf"));
        }
    }
    if drywall_sq_ft > 0.0 {
        out.push(line(
            "Drywall",
            "Wall drywall 1/2\" 4x8 sheet",
            (drywall_sq_ft / SHEET_SQ_FT).ceil(),
            "sheet",
        ));
    }
    if exterior_sq_ft > 0.0 {
        out.push(line(
            "Exterior",
            "Wall sheathing 7/16\" OSB 4x8 sheet",
            (exterior_sq_ft / SHEET_SQ_FT).ceil(),
            "sheet",
        ));
        out.push(line("Exterior", "Siding", exterior_sq_ft.ceil(), "sq ft"));
    }
    for r in rooms {
        let name = room_name(f, r);
        let area = r.area_sq_ft();
        out.push(line(
            "Flooring",
            format!("Flooring - {name}"),
            area.ceil(),
            "sq ft",
        ));
        out.push(line(
            "Drywall",
            format!("Ceiling drywall 1/2\" 4x8 sheet - {name}"),
            (area / SHEET_SQ_FT).ceil(),
            "sheet",
        ));
    }
    for ((w, h), n) in doors {
        let (w, h) = (f64::from(w as i32) / 16.0, f64::from(h as i32) / 16.0);
        out.push(line(
            "Doors",
            format!("Door {} x {}", fmt_ft_in(w), fmt_ft_in(h)),
            n,
            "ea",
        ));
    }
    for ((w, h), n) in windows {
        let (w, h) = (f64::from(w as i32) / 16.0, f64::from(h as i32) / 16.0);
        out.push(line(
            "Windows",
            format!("Window {} x {}", fmt_ft_in(w), fmt_ft_in(h)),
            n,
            "ea",
        ));
    }
    out
}

fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{q:.1}")
    }
}

/// CSV with header `Category,Item,Quantity,Unit`.
pub fn to_csv(lines: &[MaterialLine]) -> String {
    let mut out = String::new();
    push_csv_row(
        &mut out,
        &["Category", "Item", "Quantity", "Unit"].map(String::from),
    );
    for l in lines {
        push_csv_row(
            &mut out,
            &[
                l.category.clone(),
                l.item.clone(),
                fmt_qty(l.quantity),
                l.unit.clone(),
            ],
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::rect_walls;
    use plan_core::{detect_rooms, Point};

    fn qty(lines: &[MaterialLine], item: &str) -> f64 {
        lines
            .iter()
            .find(|l| l.item == item)
            .unwrap_or_else(|| panic!("missing {item}"))
            .quantity
    }

    fn one_wall(open: bool) -> Project {
        let mut p = Project::new("w");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        if open {
            p.add_opening(0, w, 60.0, OpeningKind::Door).unwrap();
        }
        p
    }

    #[test]
    fn studs_ten_foot_wall() {
        let l = materials_list(&one_wall(false), 0, &[]);
        // ceil(120 / 16) + 1 = 9
        assert_eq!(qty(&l, "2x4 Stud @ 16\" o.c."), 9.0);
        // 10 lf x 3
        assert_eq!(qty(&l, "2x4 Plate (1 bottom + 2 top)"), 30.0);
    }

    #[test]
    fn opening_adds_kings_and_trimmers() {
        let l = materials_list(&one_wall(true), 0, &[]);
        assert_eq!(qty(&l, "2x4 Stud @ 16\" o.c."), 13.0);
        assert_eq!(qty(&l, "Door 3'-0\" x 6'-8\""), 1.0);
        // Interior: both sides of (120*96 - 36*80)/144 = 60 sq ft -> 120 / 32 -> 4 sheets.
        assert_eq!(qty(&l, "Wall drywall 1/2\" 4x8 sheet"), 4.0);
    }

    #[test]
    fn rooms_and_exterior() {
        let mut p = Project::new("h");
        rect_walls(&mut p, 240.0, 120.0, 6.5, WallKind::Exterior);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let l = materials_list(&p, 0, &rooms);
        assert!(l
            .iter()
            .any(|x| x.item.starts_with("Flooring - ") && x.quantity == 200.0));
        assert!(l
            .iter()
            .any(|x| x.item.starts_with("Ceiling drywall") && x.quantity == 7.0));
        // 720" of wall x 109.125" = 545.6 sq ft.
        assert_eq!(qty(&l, "Siding"), 546.0);
        assert_eq!(qty(&l, "Wall sheathing 7/16\" OSB 4x8 sheet"), 18.0);
    }

    #[test]
    fn csv_output() {
        let l = materials_list(&one_wall(true), 0, &[]);
        let csv = to_csv(&l);
        assert!(csv.starts_with("Category,Item,Quantity,Unit\n"));
        assert!(csv.contains("Framing,\"2x4 Stud @ 16\"\" o.c.\",13,ea\n"));
    }
}
