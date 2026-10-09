//! Door, window, room and wall schedules.
//!
//! Each generator returns a [`Schedule`]: a title, column headings and rows of
//! pre-formatted strings, ready for [`Schedule::to_csv`] or
//! [`Schedule::to_markdown`]. Lengths use [`fmt_ft_in`]; areas use one decimal.
//!
//! Doors, windows and walls are numbered in reading order across the plan
//! (by x, then y, of the object's centre), so the numbers are stable for a
//! given drawing and independent of the order objects were drawn in.

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

    /// Swap Rows/Columns: one column per object, headed by the object's
    /// first cell (its number), with the attribute names down the first
    /// column. The headings of the original columns become the first cell of
    /// each row.
    pub fn transposed(&self) -> Schedule {
        let Some(first) = self.columns.first() else {
            return self.clone();
        };
        let mut columns = vec![first.clone()];
        columns.extend(
            self.rows
                .iter()
                .map(|r| r.first().cloned().unwrap_or_default()),
        );
        let rows = (1..self.columns.len())
            .map(|i| {
                let mut row = vec![self.columns[i].clone()];
                row.extend(
                    self.rows
                        .iter()
                        .map(|r| r.get(i).cloned().unwrap_or_default()),
                );
                row
            })
            .collect();
        Schedule {
            title: self.title.clone(),
            columns,
            rows,
        }
    }

    /// Wrapping: the rows cut into consecutive tables of `counts` rows each
    /// (see [`wrap_counts`]). The last `tail` rows (the Totals row) stay with
    /// the last table. The tables share the title and the columns.
    pub fn wrapped(&self, counts: &[usize], tail: usize) -> Vec<Schedule> {
        let body = self.rows.len().saturating_sub(tail);
        let mut cuts: Vec<(usize, usize)> = Vec::new();
        let mut at = 0;
        for n in counts {
            if at >= body {
                break;
            }
            let end = (at + (*n).max(1)).min(body);
            cuts.push((at, end));
            at = end;
        }
        if at < body {
            cuts.push((at, body));
        }
        if cuts.is_empty() {
            cuts.push((0, body));
        }
        let last = cuts.len() - 1;
        cuts.into_iter()
            .enumerate()
            .map(|(i, (from, to))| {
                let mut rows = self.rows[from..to].to_vec();
                if i == last {
                    rows.extend(self.rows[body..].iter().cloned());
                }
                Schedule {
                    title: self.title.clone(),
                    columns: self.columns.clone(),
                    rows,
                }
            })
            .collect()
    }

    /// Schedule to Text: the table as tab-delimited text, the title line
    /// first when `title` is set and the column headings next when `headings`
    /// is, one line per row.
    pub fn to_tsv(&self, title: bool, headings: bool) -> String {
        let mut out = String::new();
        if title {
            out.push_str(&self.title);
            out.push('\n');
        }
        let line = |cells: &[String]| {
            let cells: Vec<String> = cells
                .iter()
                .map(|c| c.replace(['\t', '\n', '\r'], " "))
                .collect();
            format!("{}\n", cells.join("\t"))
        };
        if headings {
            out.push_str(&line(&self.columns));
        }
        for r in &self.rows {
            out.push_str(&line(r));
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

/// Where a wrapped schedule breaks: how many items (rows, or columns when
/// swapped) go into each table. `sizes` is the length of each item along the
/// wrap direction (a row's height, a column's width), `first` the length the
/// first table spends before its items (the title and the heading row) and
/// `rest` what each later table spends (zero when the title and headings are
/// shown only once). Every table holds at least one item.
///
/// `by` is Entries per Table (a fixed count) or Max Table Size (a length).
pub fn wrap_counts(
    sizes: &[f64],
    first: f64,
    rest: f64,
    by: plan_core::schedules::WrapBy,
) -> Vec<usize> {
    use plan_core::schedules::WrapBy;
    if sizes.is_empty() {
        return vec![0];
    }
    let mut counts = Vec::new();
    let mut i = 0;
    while i < sizes.len() {
        let overhead = if counts.is_empty() { first } else { rest };
        let n = match by {
            WrapBy::Entries(k) => k.max(1).min(sizes.len() - i),
            WrapBy::MaxSize(max) => {
                let mut used = overhead;
                let mut n = 0;
                while i + n < sizes.len() && (n == 0 || used + sizes[i + n] <= max + 1e-9) {
                    used += sizes[i + n];
                    n += 1;
                }
                n
            }
        };
        counts.push(n);
        i += n;
    }
    counts
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
pub(crate) fn reading_key(p: Point) -> (i64, i64) {
    ((p.x * 10.0).round() as i64, (p.y * 10.0).round() as i64)
}

pub(crate) fn floor_of(project: &Project, floor: usize) -> Option<&Floor> {
    project.floors.get(floor)
}

/// Wall ids in reading order of their midpoints, paired with their number
/// (`WL01`, `WL02`, ...).
pub(crate) fn wall_numbers(f: &Floor) -> Vec<(&Wall, String)> {
    // "Include in Schedule" (Wall Specification, Schedule tab): a cleared box
    // leaves the wall out; the invisible walls generated between platforms
    // are not drawn walls and are never listed.
    let mut walls: Vec<&Wall> = f
        .walls
        .iter()
        .filter(|w| w.spec.schedule.include && !w.flags.auto_generated)
        .collect();
    walls.sort_by_key(|w| (reading_key(w.start.add(w.end).scale(0.5)), w.id));
    walls
        .into_iter()
        .enumerate()
        .map(|(i, w)| (w, format!("WL{:02}", i + 1)))
        .collect()
}

/// Openings of one kind with their host wall, in reading order of their
/// world-space centre.
pub(crate) fn ordered_openings(f: &Floor, kind: OpeningKind) -> Vec<(&Opening, &Wall)> {
    let mut v: Vec<(&Opening, &Wall, Point)> = f
        .openings
        .iter()
        // "Include in Schedule" (L-29): a cleared box leaves it out.
        .filter(|o| o.kind == kind && o.extras.spec.schedule.include)
        .filter_map(|o| {
            let w = f.wall(o.wall_id)?;
            Some((o, w, w.point_at(o.center_offset)))
        })
        .collect();
    v.sort_by_key(|(o, _, c)| (reading_key(*c), o.id));
    v.into_iter().map(|(o, w, _)| (o, w)).collect()
}

pub(crate) fn wall_number_of(numbers: &[(&Wall, String)], id: Id) -> String {
    numbers
        .iter()
        .find(|(w, _)| w.id == id)
        .map(|(_, n)| n.clone())
        .unwrap_or_default()
}

pub(crate) fn kind_label(k: WallKind) -> &'static str {
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
pub fn room_name(f: &Floor, room: &Room) -> String {
    room.name_entry(&f.room_names)
        .map(|n| n.name.clone())
        .unwrap_or_else(|| room.label.clone())
}

/// Perimeter of a closed polygon, inches.
pub(crate) fn perimeter(poly: &[Point]) -> f64 {
    (0..poly.len())
        .map(|i| poly[i].dist(poly[(i + 1) % poly.len()]))
        .sum()
}

/// The area a schedule reports for `room`, square feet: the Interior Area
/// (to the inside wall surfaces) that the plan label shows (R-2, R-49), or
/// the centerline area for a room without an interior outline.
pub fn room_area_sq_ft(room: &Room) -> f64 {
    if room.inner_polygon.is_empty() {
        room.area_sq_ft()
    } else {
        room.interior_area_sq_ft()
    }
}

/// The ceiling height of `room`: its own override (R-24) when set, else the
/// floor's.
pub fn room_ceiling_height(f: &Floor, room: &Room) -> f64 {
    room.name_entry(&f.room_names)
        .and_then(|n| n.ceiling_height)
        .unwrap_or(f.ceiling_height)
}

/// Room schedule. Columns: Number, Name, Area sq ft, Perimeter ft,
/// Ceiling height. Rooms keep the order given. The area is the Interior Area
/// (same as the plan label, QA-03), the perimeter is measured on the
/// wall-centerline polygon, and the ceiling height is the room's override
/// when it has one.
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
            format!("{:.1}", room_area_sq_ft(r)),
            format!("{:.1}", perimeter(&r.polygon) / 12.0),
            fmt_ft_in(room_ceiling_height(f, r)),
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
        // Interior Area (to the inside wall surfaces), like the plan label:
        // (240 - 4.5) x (120 - 4.5) / 144 = 188.9; perimeter stays centerline.
        assert_eq!(
            s.rows[0][2],
            format!("{:.1}", rooms[0].interior_area_sq_ft())
        );
        assert_eq!(s.rows[0][2], "188.9");
        assert_eq!(s.rows[0][3], "60.0");
        assert_eq!(s.rows[0][4], "9'-1 1/8\"");
    }

    #[test]
    fn room_schedule_uses_the_ceiling_override() {
        let mut p = Project::new("R");
        rect_walls(&mut p, 240.0, 120.0, 4.5, WallKind::Interior);
        let rooms = detect_rooms(&p.floors[0].walls, 1.0);
        p.set_room_name(0, Point::new(50.0, 50.0), "Den", "Den", &rooms);
        p.floors[0].room_names[0].ceiling_height = Some(120.0);
        assert_eq!(room_schedule(&p, 0, &rooms).rows[0][4], "10'-0\"");
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
