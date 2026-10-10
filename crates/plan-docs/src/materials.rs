//! Materials List: the quantity take-off of the plan by category, with waste
//! factors, stock lengths and prices from a [`MasterList`].
//!
//! Categories follow Chief's Materials List (RF-60, CB-41, L-33..L-37):
//! Foundation, Framing, Roofing, Siding, Windows, Doors, Cabinets,
//! Electrical, Fixtures, Interior Finishes and Landscaping. Each row has an ID,
//! size, description, count, unit, unit price and price.
//!
//! Quantities come straight from the plan model:
//!
//! * **Framing**: where a floor has framing members (Framing tools), their
//!   cut list rounded up to stock lengths (8, 10, 12, 14, 16 ft) and the board
//!   feet. Otherwise a wall formula: studs at 16" o.c. per wall
//!   (`ceil(length / 16) + 1`, plus 2 kings and 2 trimmers for every opening)
//!   and plates (bottom plate plus doubled top plate). Exterior walls are 2x6,
//!   interior walls 2x4. Wall sheathing sheets are framing too.
//! * **Wall components**: a wall whose type (Wall Specification > Components)
//!   has layers beyond the structural one counts each layer's face area:
//!   sheathing and drywall in 4x8 sheets, siding, stucco, brick or stone in
//!   Siding, insulation in Framing, other layers in Interior Finishes; an air
//!   space is not a purchase. Such walls skip the formula's siding, sheathing
//!   and drywall.
//! * **Roofing**: the stored roof planes' true sloped area in square feet and
//!   in squares (100 sq ft), shingle bundles (3 per square), drip edge along
//!   the eaves and gutters where a plane has them.
//! * **Siding**: net exterior wall area, openings deducted.
//! * **Windows / Doors**: counted by size. **Cabinets**: counted by label and
//!   size, countertop area. **Electrical**: devices by type. **Fixtures**:
//!   placed fixture symbols. **Landscaping**: the terrain's plants and the
//!   soil cut and filled by its graded pads (cubic yards, from
//!   [`crate::terrain_report`]).
//! * **Interior Finishes**: drywall sheets (4x8 = 32 sq ft) and flooring by room.
//! * **Foundation**: slab, pad and pier concrete in cubic yards.
//!
//! [`materials_list`] is the plain take-off of one floor (no waste, no
//! prices). [`materials_report`] adds the master list's waste per category
//! (counts round up to whole units) and unit prices.

use crate::schedule::push_csv_row;
use plan_core::units::fmt_ft_in;
use plan_core::{Project, Room};
use plan_framing::{FramingMember, Member};
use std::collections::BTreeMap;

/// Square feet in a 4x8 sheet.
const SHEET_SQ_FT: f64 = 32.0;
/// Stud spacing, inches on centre.
const STUD_SPACING: f64 = 16.0;
/// Bundles of shingles per roofing square.
const BUNDLES_PER_SQUARE: f64 = 3.0;

/// Chief's Materials List categories, in the order they are listed.
pub const CATEGORIES: [&str; 15] = [
    "Foundation",
    "Framing",
    "Subfloor",
    "Roofing",
    "Decks-Walks",
    "Siding",
    "Windows",
    "Doors",
    "Cabinets",
    "Electrical",
    "Fixtures",
    "Interior Finishes",
    "Interior Trim",
    "Exterior Trim",
    "Landscaping",
];

/// One row of the Materials List.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialLine {
    pub category: String,
    /// Description.
    pub item: String,
    /// The count to buy: the net quantity plus waste, rounded up to whole
    /// units. Equals [`net`](Self::net) in a plain take-off.
    pub quantity: f64,
    pub unit: String,
    /// Row ID within the list, e.g. `FRM-003`.
    pub id: String,
    /// Size or dimensions, e.g. `2x6 x 16'` or `3'-0" x 6'-8"`.
    pub size: String,
    /// The quantity before waste.
    pub net: f64,
    /// Waste factor applied, percent.
    pub waste_pct: f64,
    /// Unit price from the master list, if it has one.
    pub unit_price: Option<f64>,
    /// `quantity x unit_price`.
    pub price: Option<f64>,
    /// The master list entry this row is priced by, e.g. `Framing|2x6 stud`.
    pub key: String,
}

impl MaterialLine {
    fn new(
        category: &str,
        key: &str,
        item: impl Into<String>,
        size: &str,
        net: f64,
        unit: &str,
    ) -> Self {
        Self {
            category: category.to_string(),
            item: item.into(),
            quantity: net,
            unit: unit.to_string(),
            id: String::new(),
            size: size.to_string(),
            net,
            waste_pct: 0.0,
            unit_price: None,
            price: None,
            key: format!("{category}|{key}"),
        }
    }
}

/// Which floors a materials list covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialsScope {
    /// One floor (index into `Project::floors`).
    Floor(usize),
    /// Every floor, plus the site (landscaping).
    AllFloors,
}

pub use crate::master_list::{MasterItem, MasterList};

pub mod engine;
pub mod export;
pub mod list;
#[cfg(test)]
mod list_tests;

// ----------------------------------------------------------------- take-off --

type SizeKey = (i64, i64);

/// Key sizes at 1/16" so equal sizes group and order stably.
fn size_key(w: f64, h: f64) -> SizeKey {
    ((w * 16.0).round() as i64, (h * 16.0).round() as i64)
}

fn size_text(k: SizeKey) -> String {
    format!(
        "{} x {}",
        fmt_ft_in(k.0 as f64 / 16.0),
        fmt_ft_in(k.1 as f64 / 16.0)
    )
}

/// What one non-structural layer of a wall type becomes in the list.
#[derive(Debug, Clone, PartialEq)]
enum Component {
    /// Left out: air spaces and layers with no material.
    Skip,
    Sheathing,
    Drywall,
    /// The layer named "Siding": merged with the formula's siding row.
    Siding,
    /// Any other layer, listed by its own name in a category.
    Other(&'static str),
}

/// Reads a wall-type layer (Wall Specification > Components) as a material
/// row: sheathing and drywall count in 4x8 sheets, exterior finishes
/// (siding, stucco, brick, stone...) in Siding, insulation in Framing, and
/// anything else among the Interior Finishes.
fn component_of(name: &str, material: &str) -> Component {
    let n = name.to_lowercase();
    let m = material.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w) || m.contains(w));
    if n.contains("air space")
        || (m == "air" && n.contains("air"))
        || (n.is_empty() && m.is_empty())
    {
        Component::Skip
    } else if has(&["sheath"]) {
        Component::Sheathing
    } else if has(&["drywall", "gypsum", "plaster", "gwb"]) {
        Component::Drywall
    } else if n.trim() == "siding" {
        Component::Siding
    } else if has(&[
        "siding", "stucco", "brick", "stone", "veneer", "cladding", "shingle", "hardie", "cement",
    ]) {
        Component::Other("Siding")
    } else if has(&["insul"]) {
        Component::Other("Framing")
    } else {
        Component::Other("Interior Finishes")
    }
}

/// The wall type of `w` when it has layers beyond the structural one.
fn layered_type<'a>(
    project: &'a Project,
    w: &plan_core::Wall,
) -> Option<&'a plan_core::WallTypeDef> {
    let def = project.wall_type_def(w.wall_type.as_deref()?)?;
    def.layers.iter().any(|l| !l.is_main).then_some(def)
}

fn floor_range(project: &Project, scope: MaterialsScope) -> std::ops::Range<usize> {
    match scope {
        MaterialsScope::Floor(f) => f.min(project.floors.len())..(f + 1).min(project.floors.len()),
        MaterialsScope::AllFloors => 0..project.floors.len(),
    }
}

/// The framing members stored on a floor, as the schedule reads them.
fn framing_members(f: &plan_core::Floor) -> (Vec<Member>, Vec<FramingMember>) {
    let (mut auto, mut manual) = (Vec::new(), Vec::new());
    for v in &f.framing {
        let record = v.as_object().filter(|o| o.len() == 1).and_then(|o| {
            o.get("Manual")
                .or_else(|| o.get("Built"))
                .and_then(|m| serde_json::from_value::<FramingMember>(m.clone()).ok())
        });
        if let Some(m) = record {
            manual.push(m);
        } else if let Ok(m) = serde_json::from_value::<Member>(v.clone()) {
            auto.push(m);
        }
    }
    (auto, manual)
}

/// The net take-off of `scope`, before waste and prices. `active_rooms`
/// supplies the detected rooms of one floor (others are detected here).
fn take_off(
    project: &Project,
    scope: MaterialsScope,
    active_rooms: Option<(usize, &[Room])>,
    master: &MasterList,
) -> Vec<MaterialLine> {
    let floors: Vec<usize> = floor_range(project, scope).collect();
    let opts = engine::Options {
        site_everywhere: scope == MaterialsScope::AllFloors,
        ..engine::Options::default()
    };
    engine::take_off_raw(project, &floors, active_rooms, master, &opts)
        .into_iter()
        .map(|r| r.line)
        .collect()
}

/// The row of a wall-type layer measured in square feet: `Stucco (Sand
/// Finish)` is priced by the key `<category>|<layer name>`.
fn layer_line(category: &str, name: &str, material: &str, sq_ft: f64) -> MaterialLine {
    let item = if material.trim().is_empty() || material.eq_ignore_ascii_case(name) {
        name.to_string()
    } else {
        format!("{name} ({material})")
    };
    MaterialLine::new(category, name, item, "", sq_ft.ceil(), "sq ft")
}

/// Numbers the rows of each category (`FRM-001`) after sorting by category.
fn number_rows(lines: &mut [MaterialLine]) {
    let order = |c: &str| {
        CATEGORIES
            .iter()
            .position(|k| *k == c)
            .unwrap_or(CATEGORIES.len())
    };
    lines.sort_by_key(|l| order(&l.category));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for l in lines {
        let prefix = match l.category.as_str() {
            "Foundation" => "FND",
            "Framing" => "FRM",
            "Roofing" => "RF",
            "Siding" => "SD",
            "Windows" => "WIN",
            "Doors" => "DR",
            "Cabinets" => "CAB",
            "Electrical" => "EL",
            "Fixtures" => "FX",
            "Interior Finishes" => "INT",
            "Interior Trim" => "ITR",
            "Exterior Trim" => "ETR",
            "Subfloor" => "SUB",
            "Decks-Walks" => "DW",
            "Landscaping" => "LS",
            _ => "MISC",
        };
        let n = counts.entry(l.category.clone()).or_default();
        *n += 1;
        l.id = format!("{prefix}-{n:03}");
    }
}

/// Counts that are bought whole round up; cubic yards and squares keep
/// hundredths.
fn round_up(q: f64, unit: &str) -> f64 {
    match unit {
        "cu yd" | "sq" => (q * 100.0).ceil() / 100.0,
        _ => (q - 1e-9).ceil(),
    }
}

/// The take-off of one floor, in plain quantities: no waste and no prices.
/// An out-of-range `floor` yields an empty list. `rooms` supplies the flooring
/// and ceiling lines of that floor.
pub fn materials_list(project: &Project, floor: usize, rooms: &[Room]) -> Vec<MaterialLine> {
    if floor >= project.floors.len() {
        return Vec::new();
    }
    let mut lines = take_off(
        project,
        MaterialsScope::Floor(floor),
        Some((floor, rooms)),
        &MasterList::without_waste(),
    );
    number_rows(&mut lines);
    lines
}

/// The Materials List of `scope`: the take-off with `master`'s waste factors
/// (counts round up to whole units) and unit prices.
pub fn materials_report(
    project: &Project,
    scope: MaterialsScope,
    active_rooms: Option<(usize, &[Room])>,
    master: &MasterList,
) -> Vec<MaterialLine> {
    let mut lines = take_off(project, scope, active_rooms, master);
    for l in &mut lines {
        l.waste_pct = master.waste_for(&l.category);
        // Board feet and the roof area are information, not purchases.
        let info = l.unit == "bf" || l.key == "Roofing|Roof area";
        if !info {
            l.quantity = round_up(l.net * (1.0 + l.waste_pct / 100.0), &l.unit);
        }
        l.unit_price = master.price_of(&l.key);
        l.price = l.unit_price.map(|p| p * l.quantity);
    }
    number_rows(&mut lines);
    lines
}

/// The sum of the priced rows.
pub fn total_price(lines: &[MaterialLine]) -> f64 {
    lines.iter().filter_map(|l| l.price).sum()
}

/// The distinct master-list keys of `lines` with their units, in list order.
pub fn price_keys(lines: &[MaterialLine]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for l in lines {
        if !out.iter().any(|(k, _)| *k == l.key) {
            out.push((l.key.clone(), l.unit.clone()));
        }
    }
    out
}

fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{q:.2}")
    }
}

/// `$1,234.50` for a price; empty when unpriced.
pub fn fmt_money(v: Option<f64>) -> String {
    let Some(v) = v else {
        return String::new();
    };
    let cents = (v * 100.0).round() as i64;
    let (d, c) = (cents / 100, cents % 100);
    let digits = d.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("${grouped}.{c:02}")
}

/// The column titles of the Materials List.
pub const COLUMNS: [&str; 8] = [
    "Category",
    "ID",
    "Description",
    "Size",
    "Count",
    "Unit",
    "Unit Price",
    "Price",
];

/// The cells of one row, in [`COLUMNS`] order.
pub fn row_cells(l: &MaterialLine) -> Vec<String> {
    vec![
        l.category.clone(),
        l.id.clone(),
        l.item.clone(),
        l.size.clone(),
        fmt_qty(l.quantity),
        l.unit.clone(),
        fmt_money(l.unit_price),
        fmt_money(l.price),
    ]
}

/// CSV with the [`COLUMNS`] header and a final total when anything is priced.
pub fn to_csv(lines: &[MaterialLine]) -> String {
    let mut out = String::new();
    push_csv_row(&mut out, &COLUMNS.map(String::from));
    for l in lines {
        push_csv_row(&mut out, &row_cells(l));
    }
    if lines.iter().any(|l| l.price.is_some()) {
        let mut total = vec![String::new(); COLUMNS.len()];
        total[0] = "Total".into();
        total[7] = fmt_money(Some(total_price(lines)));
        push_csv_row(&mut out, &total);
    }
    out
}

/// The list as a schedule-style table (for a layout box or a PDF page):
/// a title, [`COLUMNS`] and one row per line, with a total row when priced.
pub fn to_schedule(lines: &[MaterialLine], title: &str) -> crate::schedule::Schedule {
    let mut rows: Vec<Vec<String>> = lines.iter().map(row_cells).collect();
    if lines.iter().any(|l| l.price.is_some()) {
        let mut total = vec![String::new(); COLUMNS.len()];
        total[0] = "Total".into();
        total[7] = fmt_money(Some(total_price(lines)));
        rows.push(total);
    }
    crate::schedule::Schedule {
        title: title.to_string(),
        columns: COLUMNS.map(String::from).to_vec(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::rect_walls;
    use plan_core::foundation::FoundationLayer;
    use plan_core::{detect_rooms, OpeningKind, Point, WallKind};
    use serde_json::json;

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

    /// A 40' x 30' house with a 6:12 gable roof along the 40' side.
    fn house_with_roof() -> Project {
        let mut p = Project::new("h");
        rect_walls(&mut p, 480.0, 360.0, 6.5, WallKind::Exterior);
        // Eave (0,0)-(480,0), rise 6 per 12 over a 180" run, ridge at y = 180.
        let rise = 90.0;
        let south = json!({"kind": "plane", "id": 1, "pitch": 6.0,
            "polygon3d": [[0.0,0.0,0.0],[480.0,0.0,0.0],[480.0,rise,-180.0],[0.0,rise,-180.0]],
            "baseline": [[0.0,0.0],[480.0,0.0]], "gutters": true});
        let north = json!({"kind": "plane", "id": 2, "pitch": 6.0,
            "polygon3d": [[480.0,0.0,-360.0],[0.0,0.0,-360.0],[0.0,rise,-180.0],[480.0,rise,-180.0]],
            "baseline": [[480.0,360.0],[0.0,360.0]], "gutters": false});
        p.floors[0].roofs = vec![south, north];
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
    fn rows_have_chiefs_categories_and_ids() {
        let l = materials_list(&house_with_roof(), 0, &[]);
        for x in &l {
            assert!(CATEGORIES.contains(&x.category.as_str()), "{}", x.category);
            assert!(x.id.contains('-'), "{x:?}");
        }
        let cats: Vec<&str> = l.iter().map(|x| x.category.as_str()).collect();
        let pos = |c: &str| CATEGORIES.iter().position(|k| *k == c).unwrap();
        assert!(cats.windows(2).all(|w| pos(w[0]) <= pos(w[1])), "{cats:?}");
        assert_eq!(
            l.iter().find(|x| x.category == "Roofing").unwrap().id,
            "RF-001"
        );
    }

    #[test]
    fn roofing_squares_for_the_forty_by_thirty_house() {
        let p = house_with_roof();
        let l = materials_list(&p, 0, &[]);
        // 480 x 360 in plan, 6:12: 172,800 in2 x sqrt(1.25) = 1341.6 sq ft.
        assert_eq!(qty(&l, "Roof area"), 1342.0);
        assert!((qty(&l, "Roofing (100 sq ft squares)") - 13.42).abs() < 1e-9);
        assert_eq!(qty(&l, "Shingles (3 bundles per square)"), 41.0);
        assert_eq!(qty(&l, "Drip edge along eaves"), 80.0);
        assert_eq!(qty(&l, "Gutters"), 40.0);
        // With the default 10% waste on roofing the purchase is 14.77 squares.
        let r = materials_report(&p, MaterialsScope::Floor(0), None, &MasterList::default());
        let sq = r
            .iter()
            .find(|x| x.item.starts_with("Roofing (100"))
            .unwrap();
        assert_eq!(sq.waste_pct, 10.0);
        assert!((sq.quantity - 14.77).abs() < 1e-9, "{}", sq.quantity);
    }

    #[test]
    fn siding_area_deducts_openings() {
        let p = crate::test_support::house();
        let l = materials_list(&p, 0, &[]);
        // 1680" of 109.125" wall = 1273.1 sq ft minus a door and two windows.
        let door = 36.0 * 80.0 / 144.0;
        let n = |k| {
            p.floors[0]
                .openings
                .iter()
                .filter(|o| o.kind == k)
                .map(|o| o.width * o.height / 144.0)
                .sum::<f64>()
        };
        let want = 1680.0 * 109.125 / 144.0 - n(OpeningKind::Door) - n(OpeningKind::Window);
        assert!(door > 0.0);
        assert_eq!(qty(&l, "Siding"), want.ceil());
    }

    #[test]
    fn waste_rounds_counts_up_and_prices_extend() {
        let p = one_wall(true);
        let mut m = MasterList::default();
        m.set_price("Framing|2x4 stud", "ea", 3.5);
        let r = materials_report(&p, MaterialsScope::Floor(0), None, &m);
        let studs = r.iter().find(|x| x.item.starts_with("2x4 Stud")).unwrap();
        // 13 studs + 10% = 14.3 -> 15.
        assert_eq!((studs.net, studs.quantity), (13.0, 15.0));
        assert_eq!(studs.unit_price, Some(3.5));
        assert_eq!(studs.price, Some(52.5));
        let plates = r.iter().find(|x| x.item.starts_with("2x4 Plate")).unwrap();
        assert_eq!(plates.price, None);
        assert_eq!(total_price(&r), 52.5);
        let csv = to_csv(&r);
        assert!(csv.starts_with("Category,ID,Description,Size,Count,Unit,Unit Price,Price\n"));
        assert!(csv.contains("$52.50"));
        assert!(csv.trim_end().ends_with("Total,,,,,,,$52.50"));
    }

    #[test]
    fn master_list_price_round_trips_through_json_and_a_file() {
        let mut m = MasterList::default();
        m.set_price("Roofing|Shingle bundles", "bundle", 38.25);
        m.set_price("Roofing|Shingle bundles", "bundle", 41.0);
        assert_eq!(m.items.len(), 1);
        let back = MasterList::from_json(&m.to_json()).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.price_of("Roofing|Shingle bundles"), Some(41.0));
        let dir = std::env::temp_dir().join(format!("ps-master-{}", std::process::id()));
        let path = dir.join("master-list.json");
        m.save(&path).unwrap();
        assert_eq!(MasterList::load(&path), m);
        assert_eq!(
            MasterList::load(&dir.join("missing.json")),
            MasterList::default()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stock_lengths_round_up_and_splice() {
        let m = MasterList::default();
        assert_eq!(m.stock_length_ft(92.625), 8);
        assert_eq!(m.stock_length_ft(97.0), 10);
        assert_eq!(m.stock_length_ft(144.0), 12);
        assert_eq!(m.stock_pieces(120.0), (10, 1));
        assert_eq!(m.stock_pieces(200.0), (16, 2));
    }

    #[test]
    fn framing_members_are_counted_in_stock_lengths_with_board_feet() {
        let mut p = Project::new("f");
        let w = p.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.5,
            109.125,
            WallKind::Exterior,
        );
        let wall = p.floors[0]
            .walls
            .iter()
            .find(|x| x.id == w)
            .unwrap()
            .clone();
        let members =
            plan_framing::frame_wall(&wall, &[], 0.0, &plan_framing::FramingDefaults::default());
        p.floors[0].framing = members
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();
        let r = materials_report(&p, MaterialsScope::Floor(0), None, &MasterList::default());
        // Framed members replace the wall formula.
        assert!(r.iter().all(|x| !x.item.starts_with("2x6 Stud @")));
        let studs = r.iter().find(|x| x.item == "2x6 x 10' lumber").unwrap();
        assert!(studs.net >= 10.0, "{studs:?}");
        let bf = r.iter().find(|x| x.unit == "bf").unwrap();
        assert!(bf.quantity > 50.0);
    }

    #[test]
    fn cabinets_electrical_and_foundation_have_lines() {
        let mut p = Project::new("c");
        let mut a = plan_cabinets::Cabinet::base(24.0);
        a.id = 1;
        let mut b = plan_cabinets::Cabinet::base(24.0);
        b.id = 2;
        p.floors[0].set_cabinets(&[a, b]).unwrap();
        let mut layer = FoundationLayer::default();
        layer.slabs.push(plan_core::foundation::Slab::new(
            1,
            vec![
                Point::new(0.0, 0.0),
                Point::new(360.0, 0.0),
                Point::new(360.0, 360.0),
                Point::new(0.0, 360.0),
            ],
        ));
        layer.store(&mut p.floors[0]);
        let l = materials_list(&p, 0, &[]);
        let cab = l.iter().find(|x| x.category == "Cabinets").unwrap();
        assert_eq!(cab.quantity, 2.0);
        assert!(cab.size.contains("x"));
        let slab = l.iter().find(|x| x.category == "Foundation").unwrap();
        assert!(slab.quantity > 3.0 && slab.unit == "cu yd");
    }

    #[test]
    fn all_floors_scope_adds_the_floors_together() {
        let mut p = one_wall(false);
        p.floors.push(plan_core::Floor::new("2nd Floor", 109.0));
        p.add_wall(
            1,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            4.5,
            96.0,
            WallKind::Interior,
        );
        let one = materials_report(
            &p,
            MaterialsScope::Floor(0),
            None,
            &MasterList::without_waste(),
        );
        let all = materials_report(
            &p,
            MaterialsScope::AllFloors,
            None,
            &MasterList::without_waste(),
        );
        assert_eq!(
            qty(&all, "2x4 Stud @ 16\" o.c."),
            2.0 * qty(&one, "2x4 Stud @ 16\" o.c.")
        );
    }

    #[test]
    fn schedule_form_has_a_total_row_when_priced() {
        let mut m = MasterList::without_waste();
        m.set_price("Framing|2x4 stud", "ea", 2.0);
        let r = materials_report(&one_wall(false), MaterialsScope::Floor(0), None, &m);
        let s = to_schedule(&r, "MATERIALS LIST");
        assert_eq!(s.columns.len(), 8);
        assert_eq!(s.rows.last().unwrap()[0], "Total");
        assert_eq!(s.rows.last().unwrap()[7], "$18.00");
    }

    /// A 40' x 30' box of walls of the named type from Daniel's defaults.
    fn typed_box(ty: &str, kind: WallKind) -> Project {
        let mut p = Project::new("typed");
        for t in plan_core::PlanDefaults::chief_x18_daniel().wall_types {
            p.register_wall_type(t);
        }
        let ids = rect_walls(&mut p, 480.0, 360.0, 6.5, kind);
        for id in ids {
            let w = p.floors[0].walls.iter_mut().find(|w| w.id == id).unwrap();
            w.wall_type = Some(ty.to_string());
        }
        p
    }

    #[test]
    fn a_wall_types_layers_become_quantities() {
        // 140' of 109 1/8" wall, no openings.
        let net: f64 = 1_680.0 * 109.125 / 144.0;
        let l = materials_list(&typed_box("Stucco-6", WallKind::Exterior), 0, &[]);
        let stucco = l
            .iter()
            .find(|l| l.category == "Siding" && l.item.starts_with("Stucco"))
            .expect("a Stucco row");
        assert_eq!(stucco.unit, "sq ft");
        assert_eq!(stucco.quantity, net.ceil());
        assert!(stucco.item.contains("Sand Finish"), "{}", stucco.item);
        // The formula's plain Siding row does not appear for a stucco wall.
        assert!(!l.iter().any(|l| l.item == "Siding"));
        // Sheathing and drywall come from the type's own layers, once.
        assert_eq!(
            qty(&l, "Wall sheathing 7/16\" OSB 4x8 sheet"),
            (net / 32.0).ceil()
        );
        assert_eq!(qty(&l, "Wall drywall 1/2\" 4x8 sheet"), (net / 32.0).ceil());
    }

    #[test]
    fn a_siding_wall_merges_with_the_siding_row_and_brick_gets_its_own() {
        let net: f64 = 1_680.0 * 109.125 / 144.0;
        let l = materials_list(&typed_box("Siding-6", WallKind::Exterior), 0, &[]);
        assert_eq!(qty(&l, "Siding"), net.ceil());
        let l = materials_list(&typed_box("Brick-6", WallKind::Exterior), 0, &[]);
        let brick = l.iter().find(|l| l.item == "Brick").expect("a Brick row");
        assert_eq!(brick.category, "Siding");
        assert_eq!(brick.quantity, net.ceil());
        // The air space is not a purchase.
        assert!(!l.iter().any(|l| l.item.contains("Air")), "{l:?}");
    }

    #[test]
    fn walls_without_a_type_keep_the_formula() {
        let mut p = typed_box("Siding-6", WallKind::Exterior);
        for w in &mut p.floors[0].walls {
            w.wall_type = None;
        }
        let net: f64 = 1_680.0 * 109.125 / 144.0;
        let l = materials_list(&p, 0, &[]);
        assert_eq!(qty(&l, "Siding"), net.ceil());
        assert_eq!(qty(&l, "Wall drywall 1/2\" 4x8 sheet"), (net / 32.0).ceil());
    }
}
