//! Door, window, room and wall schedules.
//!
//! Each generator returns a [`Schedule`]: a title, column headings and rows of
//! pre-formatted strings, ready for [`Schedule::to_csv`] or
//! [`Schedule::to_markdown`]. Lengths use [`fmt_ft_in`]; areas use one decimal.
//!
//! Doors, windows and walls are numbered in reading order across the plan
//! (by x, then y, of the object's centre), so the numbers are stable for a
//! given drawing and independent of the order objects were drawn in.

use plan_core::geometry::point_in_polygon;
use plan_core::units::fmt_ft_in;
use plan_core::{Floor, Id, Opening, OpeningKind, Point, Project, Room, Wall, WallKind};

/// A table of strings with a title.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Schedule {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Schedule {
    fn new(title: impl Into<String>, columns: &[&str]) -> Self {
        Self {
            title: title.into(),
            columns: columns.iter().map(|c| (*c).to_string()).collect(),
            rows: Vec::new(),
        }
    }

    /// RFC 4180-style CSV: header row then data rows, `\n` line endings.
    /// Fields containing commas, quotes or newlines are quoted (the `"`
    /// inch mark in feet-inches values therefore always arrives quoted).
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        push_csv_row(&mut out, &self.columns);
        for r in &self.rows {
            push_csv_row(&mut out, r);
        }
        out
    }

    /// A Markdown section: `### title` followed by a pipe table.
    pub fn to_markdown(&self) -> String {
        let esc = |s: &str| s.replace('|', "\\|");
        let line = |cells: &[String]| {
            let cells: Vec<String> = cells.iter().map(|c| esc(c)).collect();
            format!("| {} |\n", cells.join(" | "))
        };
        let mut out = format!("### {}\n\n", self.title);
        out.push_str(&line(&self.columns));
        out.push_str(&format!("|{}\n", " --- |".repeat(self.columns.len())));
        for r in &self.rows {
            out.push_str(&line(r));
        }
        out
    }
}

/// Quote a CSV field when needed.
pub(crate) fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(crate) fn push_csv_row(out: &mut String, cells: &[String]) {
    let fields: Vec<String> = cells.iter().map(|c| csv_field(c)).collect();
    out.push_str(&fields.join(","));
    out.push('\n');
}

/// Reading-order key: x then y, rounded to 0.1" so near-equal values tie.
fn reading_key(p: Point) -> (i64, i64) {
    ((p.x * 10.0).round() as i64, (p.y * 10.0).round() as i64)
}

fn floor_of(project: &Project, floor: usize) -> Option<&Floor> {
    project.floors.get(floor)
}

/// Wall ids in reading order of their midpoints, paired with their number
/// (`WL01`, `WL02`, ...).
fn wall_numbers(f: &Floor) -> Vec<(&Wall, String)> {
    let mut walls: Vec<&Wall> = f.walls.iter().collect();
    walls.sort_by_key(|w| (reading_key(w.start.add(w.end).scale(0.5)), w.id));
    walls
        .into_iter()
        .enumerate()
        .map(|(i, w)| (w, format!("WL{:02}", i + 1)))
        .collect()
}

/// Openings of one kind with their host wall, in reading order of their
/// world-space centre.
fn ordered_openings(f: &Floor, kind: OpeningKind) -> Vec<(&Opening, &Wall)> {
    let mut v: Vec<(&Opening, &Wall, Point)> = f
        .openings
        .iter()
        .filter(|o| o.kind == kind)
        .filter_map(|o| {
            let w = f.wall(o.wall_id)?;
            Some((o, w, w.point_at(o.center_offset)))
        })
        .collect();
    v.sort_by_key(|(o, _, c)| (reading_key(*c), o.id));
    v.into_iter().map(|(o, w, _)| (o, w)).collect()
}

fn wall_number_of(numbers: &[(&Wall, String)], id: Id) -> String {
    numbers
        .iter()
        .find(|(w, _)| w.id == id)
        .map(|(_, n)| n.clone())
        .unwrap_or_default()
}

fn kind_label(k: WallKind) -> &'static str {
    match k {
        WallKind::Exterior => "Exterior",
        WallKind::Interior => "Interior",
    }
}

/// Door schedule. Columns: Number, Floor, Width, Height, Type, Wall, Swing.
///
/// Type is the host wall's kind (Exterior / Interior). Swing is `Standard`
/// (hinge at the wall-start side, swinging to the left of the wall) or
/// `Flipped`. `floor` out of range yields an empty schedule.
pub fn door_schedule(project: &Project, floor: usize) -> Schedule {
    let mut s = Schedule::new(
        "Door Schedule",
        &[
            "Number", "Floor", "Width", "Height", "Type", "Wall", "Swing",
        ],
    );
    let Some(f) = floor_of(project, floor) else {
        return s;
    };
    let numbers = wall_numbers(f);
    for (i, (o, w)) in ordered_openings(f, OpeningKind::Door)
        .into_iter()
        .enumerate()
    {
        s.rows.push(vec![
            format!("D{:02}", i + 1),
            f.name.clone(),
            fmt_ft_in(o.width),
            fmt_ft_in(o.height),
            kind_label(w.kind).to_string(),
            wall_number_of(&numbers, w.id),
            if o.swing_flipped {
                "Flipped"
            } else {
                "Standard"
            }
            .to_string(),
        ]);
    }
    s
}

/// Window schedule. Columns: Number, Width, Height, Sill, Head, Type, Wall.
///
/// Head is sill plus height. Type is the host wall's kind.
pub fn window_schedule(project: &Project, floor: usize) -> Schedule {
    let mut s = Schedule::new(
        "Window Schedule",
        &["Number", "Width", "Height", "Sill", "Head", "Type", "Wall"],
    );
    let Some(f) = floor_of(project, floor) else {
        return s;
    };
    let numbers = wall_numbers(f);
    for (i, (o, w)) in ordered_openings(f, OpeningKind::Window)
        .into_iter()
        .enumerate()
    {
        s.rows.push(vec![
            format!("W{:02}", i + 1),
            fmt_ft_in(o.width),
            fmt_ft_in(o.height),
            fmt_ft_in(o.sill_height),
            fmt_ft_in(o.sill_height + o.height),
            kind_label(w.kind).to_string(),
            wall_number_of(&numbers, w.id),
        ]);
    }
    s
}

/// The user-assigned name of `room` on `floor`, else the detected label.
pub(crate) fn room_name(f: &Floor, room: &Room) -> String {
    f.room_names
        .iter()
        .find(|n| point_in_polygon(n.anchor, &room.polygon))
        .map(|n| n.name.clone())
        .unwrap_or_else(|| room.label.clone())
}

/// Perimeter of a closed polygon, inches.
fn perimeter(poly: &[Point]) -> f64 {
    (0..poly.len())
        .map(|i| poly[i].dist(poly[(i + 1) % poly.len()]))
        .sum()
}

/// Room schedule. Columns: Number, Name, Area sq ft, Perimeter ft,
/// Ceiling height. Rooms keep the order given; areas and perimeters are
/// measured on the wall-centerline polygon.
pub fn room_schedule(project: &Project, floor: usize, rooms: &[Room]) -> Schedule {
    let mut s = Schedule::new(
        "Room Schedule",
        &[
            "Number",
            "Name",
            "Area sq ft",
            "Perimeter ft",
            "Ceiling height",
        ],
    );
    let Some(f) = floor_of(project, floor) else {
        return s;
    };
    for (i, r) in rooms.iter().enumerate() {
        s.rows.push(vec![
            format!("R{:02}", i + 1),
            room_name(f, r),
            format!("{:.1}", r.area_sq_ft()),
            format!("{:.1}", perimeter(&r.polygon) / 12.0),
            fmt_ft_in(f.ceiling_height),
        ]);
    }
    s
}

/// Wall schedule. Columns: Number, Type, Length, Thickness, Height,
/// Area sq ft, Openings. Area is gross (length x height, openings not
/// deducted); Openings counts doors and windows hosted in the wall.
pub fn wall_schedule(project: &Project, floor: usize) -> Schedule {
    let mut s = Schedule::new(
        "Wall Schedule",
        &[
            "Number",
            "Type",
            "Length",
            "Thickness",
            "Height",
            "Area sq ft",
            "Openings",
        ],
    );
    let Some(f) = floor_of(project, floor) else {
        return s;
    };
    for (w, num) in wall_numbers(f) {
        s.rows.push(vec![
            num,
            kind_label(w.kind).to_string(),
            fmt_ft_in(w.length()),
            fmt_ft_in(w.thickness),
            fmt_ft_in(w.height),
            format!("{:.1}", w.length() * w.height / 144.0),
            f.openings_on(w.id).count().to_string(),
        ]);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::rect_walls;
    use plan_core::detect_rooms;

    fn small_project() -> Project {
        let mut p = Project::new("Small");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        // Added right-to-left to prove numbering is positional.
        p.add_opening(0, w, 200.0, OpeningKind::Door).unwrap();
        p.add_opening(0, w, 60.0, OpeningKind::Door).unwrap();
        p.add_opening(0, w, 130.0, OpeningKind::Window).unwrap();
        p
    }

    #[test]
    fn doors_numbered_left_to_right_and_formatted() {
        let s = door_schedule(&small_project(), 0);
        assert_eq!(
            s.columns,
            ["Number", "Floor", "Width", "Height", "Type", "Wall", "Swing"]
        );
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0][0], "D01");
        assert_eq!(s.rows[1][0], "D02");
        assert_eq!(s.rows[0][1], "1st Floor");
        assert_eq!(s.rows[0][2], "3'-0\"");
        assert_eq!(s.rows[0][3], "6'-8\"");
        assert_eq!(s.rows[0][4], "Exterior");
        assert_eq!(s.rows[0][5], "WL01");
        assert_eq!(s.rows[0][6], "Standard");
    }

    #[test]
    fn window_schedule_has_sill_and_head() {
        let s = window_schedule(&small_project(), 0);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(
            s.rows[0],
            ["W01", "3'-0\"", "5'-0\"", "2'-0\"", "7'-0\"", "Exterior", "WL01"]
        );
    }

    #[test]
    fn csv_quotes_inch_marks_and_markdown_has_table() {
        let s = door_schedule(&small_project(), 0);
        let csv = s.to_csv();
        assert!(csv.starts_with("Number,Floor,Width,Height,Type,Wall,Swing\n"));
        assert!(csv.contains("D01,1st Floor,\"3'-0\"\"\",\"6'-8\"\"\",Exterior,WL01,Standard\n"));
        let md = s.to_markdown();
        assert!(md.starts_with("### Door Schedule\n\n| Number | Floor |"));
        assert!(md.contains("| --- | --- | --- | --- | --- | --- | --- |"));
        assert!(md.contains("| D02 |"));
    }

    #[test]
    fn room_schedule_area_200() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        let s = room_schedule(&p, 0, &rooms);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0][0], "R01");
        assert_eq!(s.rows[0][2], "200.0");
        assert_eq!(s.rows[0][3], "60.0");
        assert_eq!(s.rows[0][4], "9'-1 1/8\"");
    }

    #[test]
    fn room_schedule_uses_assigned_name() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        p.set_room_name(0, Point::new(50.0, 50.0), "Kitchen", "Kitchen", &rooms);
        assert_eq!(room_schedule(&p, 0, &rooms).rows[0][1], "Kitchen");
    }

    #[test]
    fn wall_schedule_counts_openings() {
        let s = wall_schedule(&small_project(), 0);
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.rows[0][1], "Exterior");
        assert_eq!(s.rows[0][2], "20'-0\"");
        assert_eq!(s.rows[0][3], "0'-6 1/2\"");
        assert_eq!(s.rows[0][5], "181.9");
        assert_eq!(s.rows[0][6], "3");
    }

    #[test]
    fn bad_floor_index_is_empty() {
        assert!(door_schedule(&small_project(), 7).rows.is_empty());
    }
}
