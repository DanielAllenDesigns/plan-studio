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

use crate::schedule::{push_csv_row, room_name};
use crate::schedule_kinds::entries;
use plan_cabinets::{auto_label, Cabinet};
use plan_core::foundation::FoundationLayer;
use plan_core::schedules::ScheduleKind;
use plan_core::units::fmt_ft_in;
use plan_core::{detect_rooms, OpeningKind, Project, Room, WallKind};
use plan_electrical::ElectricalLayer;
use plan_framing::{FramingMember, MaterialList, Member};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Square feet in a 4x8 sheet.
const SHEET_SQ_FT: f64 = 32.0;
/// Stud spacing, inches on centre.
const STUD_SPACING: f64 = 16.0;
/// Bundles of shingles per roofing square.
const BUNDLES_PER_SQUARE: f64 = 3.0;

/// Chief's Materials List categories, in the order they are listed.
pub const CATEGORIES: [&str; 11] = [
    "Foundation",
    "Framing",
    "Roofing",
    "Siding",
    "Windows",
    "Doors",
    "Cabinets",
    "Electrical",
    "Fixtures",
    "Interior Finishes",
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

// --------------------------------------------------------------- master list --

/// One priced entry of the master list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterItem {
    /// `Category|item`, as in [`MaterialLine::key`].
    pub key: String,
    pub unit: String,
    pub unit_price: f64,
    #[serde(default)]
    pub supplier: String,
}

/// Prices, waste factors and stock lengths behind the Materials List: the
/// Master List, kept per user in `~/.plan-studio/master-list.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterList {
    /// Waste per category, percent.
    pub waste: BTreeMap<String, f64>,
    /// Lumber stock lengths, feet, shortest first.
    pub stock_lengths_ft: Vec<u32>,
    pub items: Vec<MasterItem>,
}

impl Default for MasterList {
    /// Waste of 10% on framing, roofing, siding and interior finishes, 5% on
    /// foundation concrete, none elsewhere; lumber in 8, 10, 12, 14 and 16
    /// foot lengths; no prices (an unpriced row shows no price).
    fn default() -> Self {
        let waste = [
            ("Foundation", 5.0),
            ("Framing", 10.0),
            ("Roofing", 10.0),
            ("Siding", 10.0),
            ("Interior Finishes", 10.0),
        ]
        .into_iter()
        .map(|(c, w)| (c.to_string(), w))
        .collect();
        Self {
            waste,
            stock_lengths_ft: vec![8, 10, 12, 14, 16],
            items: Vec::new(),
        }
    }
}

impl MasterList {
    /// A list with no waste at all (the plain take-off).
    pub fn without_waste() -> Self {
        Self {
            waste: BTreeMap::new(),
            ..Self::default()
        }
    }

    /// Waste for `category`, percent.
    pub fn waste_for(&self, category: &str) -> f64 {
        self.waste.get(category).copied().unwrap_or(0.0)
    }

    /// The entry for `key`.
    pub fn item(&self, key: &str) -> Option<&MasterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    /// The unit price of `key`, if it is priced (a price of 0 is "not set").
    pub fn price_of(&self, key: &str) -> Option<f64> {
        self.item(key).map(|i| i.unit_price).filter(|p| *p > 0.0)
    }

    /// Sets the price of `key` (adds the entry when it is new).
    pub fn set_price(&mut self, key: &str, unit: &str, unit_price: f64) {
        match self.items.iter_mut().find(|i| i.key == key) {
            Some(i) => i.unit_price = unit_price,
            None => self.items.push(MasterItem {
                key: key.to_string(),
                unit: unit.to_string(),
                unit_price,
                supplier: String::new(),
            }),
        }
    }

    /// The stock length that holds a piece of `inches`: the shortest stock
    /// length that is long enough, else the longest (the piece is then
    /// spliced; see [`stock_pieces`](Self::stock_pieces)).
    pub fn stock_length_ft(&self, inches: f64) -> u32 {
        let feet = inches / 12.0 - 1e-6;
        self.stock_lengths_ft
            .iter()
            .copied()
            .find(|l| f64::from(*l) >= feet)
            .or_else(|| self.stock_lengths_ft.last().copied())
            .unwrap_or(16)
    }

    /// `(stock length in feet, pieces)` that supply one cut of `inches`.
    pub fn stock_pieces(&self, inches: f64) -> (u32, u32) {
        let l = self.stock_length_ft(inches);
        let pieces = (inches / 12.0 / f64::from(l) - 1e-6).ceil().max(1.0) as u32;
        (l, pieces)
    }

    /// The list as JSON text.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// A list from JSON text; `None` when it does not parse.
    pub fn from_json(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// Reads the list at `path`; the default list when the file is missing
    /// or unreadable.
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| Self::from_json(&t))
            .unwrap_or_default()
    }

    /// Writes the list to `path`, making its folder first.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_json())
    }
}

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

/// Running totals over the floors of a scope.
#[derive(Default)]
struct Takeoff {
    studs: [f64; 2],
    plate_lf: [f64; 2],
    drywall_sq_ft: f64,
    exterior_sq_ft: f64,
    doors: BTreeMap<SizeKey, f64>,
    windows: BTreeMap<SizeKey, f64>,
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

/// Roof planes stored on a floor: `(true area sq in, eave length in, gutters)`.
fn roof_planes(f: &plan_core::Floor) -> Vec<(f64, f64, bool)> {
    f.roofs
        .iter()
        .filter(|v| v.get("kind").and_then(|k| k.as_str()) == Some("plane"))
        .filter_map(|v| {
            let poly: Vec<[f64; 3]> = serde_json::from_value(v.get("polygon3d")?.clone()).ok()?;
            if poly.len() < 3 {
                return None;
            }
            // Newell vector: half its length is the true area.
            let mut s = [0.0; 3];
            for i in 0..poly.len() {
                let (c, d) = (poly[i], poly[(i + 1) % poly.len()]);
                s[0] += (c[1] - d[1]) * (c[2] + d[2]);
                s[1] += (c[2] - d[2]) * (c[0] + d[0]);
                s[2] += (c[0] - d[0]) * (c[1] + d[1]);
            }
            let area = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() * 0.5;
            let eave = {
                let (a, b) = (poly[0], poly[1]);
                ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
            };
            let gutters = v.get("gutters").and_then(|g| g.as_bool()).unwrap_or(false);
            Some((area, eave, gutters))
        })
        .collect()
}

/// The net take-off of `scope`, before waste and prices. `active_rooms`
/// supplies the detected rooms of one floor (others are detected here).
fn take_off(
    project: &Project,
    scope: MaterialsScope,
    active_rooms: Option<(usize, &[Room])>,
    master: &MasterList,
) -> Vec<MaterialLine> {
    let range = floor_range(project, scope);
    let multi = range.len() > 1;
    let mut t = Takeoff::default();
    let mut out: Vec<MaterialLine> = Vec::new();
    let mut room_lines: Vec<MaterialLine> = Vec::new();
    let mut framing_auto: Vec<Member> = Vec::new();
    let mut framing_manual: Vec<FramingMember> = Vec::new();

    for fi in range.clone() {
        let f = &project.floors[fi];
        let (auto, manual) = framing_members(f);
        let framed = !auto.is_empty() || !manual.is_empty();
        framing_auto.extend(auto);
        framing_manual.extend(manual);

        for w in &f.walls {
            let len = w.length();
            let openings: Vec<_> = f.openings_on(w.id).collect();
            let k = usize::from(w.kind == WallKind::Interior);
            if !framed {
                t.studs[k] += (len / STUD_SPACING).ceil() + 1.0 + 4.0 * openings.len() as f64;
                t.plate_lf[k] += len / 12.0 * 3.0;
            }
            let opening_sq_ft: f64 = openings.iter().map(|o| o.width * o.height / 144.0).sum();
            let net = (len * w.height / 144.0 - opening_sq_ft).max(0.0);
            match w.kind {
                WallKind::Exterior => {
                    t.exterior_sq_ft += net;
                    t.drywall_sq_ft += net;
                }
                WallKind::Interior => t.drywall_sq_ft += 2.0 * net,
            }
            for o in openings {
                let key = size_key(o.width, o.height);
                let map = match o.kind {
                    OpeningKind::Door => &mut t.doors,
                    OpeningKind::Window => &mut t.windows,
                };
                *map.entry(key).or_insert(0.0) += 1.0;
            }
        }

        let detected;
        let rooms: &[Room] = match active_rooms {
            Some((a, r)) if a == fi => r,
            _ => {
                detected = detect_rooms(&f.walls, 1.0);
                &detected
            }
        };
        for r in rooms {
            let name = room_name(f, r);
            let name = if multi {
                format!("{} - {name}", f.name)
            } else {
                name
            };
            let area = r.area_sq_ft();
            room_lines.push(MaterialLine::new(
                "Interior Finishes",
                "Flooring",
                format!("Flooring - {name}"),
                "",
                area.ceil(),
                "sq ft",
            ));
            room_lines.push(MaterialLine::new(
                "Interior Finishes",
                "Ceiling drywall 1/2\" 4x8 sheet",
                format!("Ceiling drywall 1/2\" 4x8 sheet - {name}"),
                "4x8",
                (area / SHEET_SQ_FT).ceil(),
                "sheet",
            ));
        }
    }

    // ---- Foundation ----
    let (mut slab, mut pad, mut pier) = (0.0, 0.0, 0.0);
    for fi in range.clone() {
        let layer = FoundationLayer::load(&project.floors[fi]);
        let holes: Vec<_> = layer.holes.iter().collect();
        slab += layer
            .slabs
            .iter()
            .map(|s| s.concrete_cu_yd(&holes))
            .sum::<f64>();
        pad += layer.pads.iter().map(|p| p.concrete_cu_yd()).sum::<f64>();
        pier += layer.piers.iter().map(|p| p.concrete_cu_yd()).sum::<f64>();
    }
    for (key, item, v) in [
        ("Slab concrete", "Slab and footing concrete", slab),
        ("Pad concrete", "Pad concrete", pad),
        ("Pier concrete", "Pier concrete", pier),
    ] {
        if v > 0.0 {
            out.push(MaterialLine::new(
                "Foundation",
                key,
                item,
                "",
                (v * 100.0).ceil() / 100.0,
                "cu yd",
            ));
        }
    }

    // ---- Framing ----
    let names = ["2x6 Stud @ 16\" o.c.", "2x4 Stud @ 16\" o.c."];
    let stud_keys = ["2x6 stud", "2x4 stud"];
    let plates = [
        "2x6 Plate (1 bottom + 2 top)",
        "2x4 Plate (1 bottom + 2 top)",
    ];
    let plate_keys = ["2x6 plate", "2x4 plate"];
    for k in 0..2 {
        if t.studs[k] > 0.0 {
            out.push(MaterialLine::new(
                "Framing",
                stud_keys[k],
                names[k],
                if k == 0 { "2x6" } else { "2x4" },
                t.studs[k],
                "ea",
            ));
        }
    }
    for k in 0..2 {
        if t.plate_lf[k] > 0.0 {
            out.push(MaterialLine::new(
                "Framing",
                plate_keys[k],
                plates[k],
                if k == 0 { "2x6" } else { "2x4" },
                t.plate_lf[k].ceil(),
                "lf",
            ));
        }
    }
    if !framing_auto.is_empty() || !framing_manual.is_empty() {
        let list = MaterialList::from_members(&framing_auto, &framing_manual);
        // (size, stock feet) -> pieces
        let mut stock: BTreeMap<(String, u32), f64> = BTreeMap::new();
        for r in &list.rows {
            let (l, per_cut) = master.stock_pieces(r.length_in);
            *stock.entry((r.size.clone(), l)).or_default() += f64::from(r.qty * per_cut);
        }
        for ((size, l), n) in stock {
            out.push(MaterialLine::new(
                "Framing",
                &format!("{size} lumber"),
                format!("{size} x {l}' lumber"),
                &format!("{size} x {l}'"),
                n,
                "ea",
            ));
        }
        out.push(MaterialLine::new(
            "Framing",
            "board feet",
            "Framing lumber, board feet",
            "",
            list.total_board_feet().ceil(),
            "bf",
        ));
    }
    if t.exterior_sq_ft > 0.0 {
        out.push(MaterialLine::new(
            "Framing",
            "Wall sheathing 7/16\" OSB 4x8 sheet",
            "Wall sheathing 7/16\" OSB 4x8 sheet",
            "4x8",
            (t.exterior_sq_ft / SHEET_SQ_FT).ceil(),
            "sheet",
        ));
    }

    // ---- Roofing ----
    let (mut area_in2, mut eave_in, mut gutter_in) = (0.0, 0.0, 0.0);
    for fi in range.clone() {
        for (a, e, g) in roof_planes(&project.floors[fi]) {
            area_in2 += a;
            eave_in += e;
            if g {
                gutter_in += e;
            }
        }
    }
    if area_in2 > 0.0 {
        let sq_ft = area_in2 / 144.0;
        let squares = sq_ft / 100.0;
        out.push(MaterialLine::new(
            "Roofing",
            "Roof area",
            "Roof area",
            "",
            sq_ft.ceil(),
            "sq ft",
        ));
        out.push(MaterialLine::new(
            "Roofing",
            "Roofing squares",
            "Roofing (100 sq ft squares)",
            "",
            (squares * 100.0).ceil() / 100.0,
            "sq",
        ));
        out.push(MaterialLine::new(
            "Roofing",
            "Shingle bundles",
            "Shingles (3 bundles per square)",
            "",
            (squares * BUNDLES_PER_SQUARE).ceil(),
            "bundle",
        ));
        out.push(MaterialLine::new(
            "Roofing",
            "Underlayment",
            "Roof underlayment",
            "",
            (squares * 100.0).ceil() / 100.0,
            "sq",
        ));
        out.push(MaterialLine::new(
            "Roofing",
            "Drip edge",
            "Drip edge along eaves",
            "",
            (eave_in / 12.0).ceil(),
            "lf",
        ));
        if gutter_in > 0.0 {
            out.push(MaterialLine::new(
                "Roofing",
                "Gutters",
                "Gutters",
                "",
                (gutter_in / 12.0).ceil(),
                "lf",
            ));
        }
    }

    // ---- Siding ----
    if t.exterior_sq_ft > 0.0 {
        out.push(MaterialLine::new(
            "Siding",
            "Siding",
            "Siding",
            "",
            t.exterior_sq_ft.ceil(),
            "sq ft",
        ));
    }

    // ---- Windows, Doors ----
    for (k, n) in &t.windows {
        out.push(MaterialLine::new(
            "Windows",
            &format!("Window {}", size_text(*k)),
            format!("Window {}", size_text(*k)),
            &size_text(*k),
            *n,
            "ea",
        ));
    }
    for (k, n) in &t.doors {
        out.push(MaterialLine::new(
            "Doors",
            &format!("Door {}", size_text(*k)),
            format!("Door {}", size_text(*k)),
            &size_text(*k),
            *n,
            "ea",
        ));
    }

    // ---- Cabinets (by label), countertop ----
    let mut cabinets: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut counter_sq_ft = 0.0;
    for fi in range.clone() {
        for v in &project.floors[fi].cabinets {
            let Ok(c) = serde_json::from_value::<Cabinet>(v.clone()) else {
                continue;
            };
            let label = if c.label.trim().is_empty() {
                auto_label(&c)
            } else {
                c.label.clone()
            };
            let size = format!(
                "{} x {} x {}",
                fmt_ft_in(c.width),
                fmt_ft_in(c.depth),
                fmt_ft_in(c.height)
            );
            *cabinets.entry((label, size)).or_default() += 1.0;
            if c.countertop.is_some() {
                counter_sq_ft += c.width * c.depth / 144.0;
            }
        }
    }
    for ((label, size), n) in cabinets {
        out.push(MaterialLine::new(
            "Cabinets",
            &format!("Cabinet {label}"),
            format!("Cabinet {label}"),
            &size,
            n,
            "ea",
        ));
    }
    if counter_sq_ft > 0.0 {
        out.push(MaterialLine::new(
            "Cabinets",
            "Countertop",
            "Countertop",
            "",
            counter_sq_ft.ceil(),
            "sq ft",
        ));
    }

    // ---- Electrical ----
    let mut devices: BTreeMap<String, f64> = BTreeMap::new();
    for fi in range.clone() {
        let Some(v) = project.floors[fi].electrical.as_ref() else {
            continue;
        };
        if let Ok(layer) = serde_json::from_value::<ElectricalLayer>(v.clone()) {
            for d in &layer.devices {
                *devices.entry(d.kind.name().to_string()).or_default() += 1.0;
            }
        }
    }
    for (name, n) in devices {
        out.push(MaterialLine::new(
            "Electrical",
            &name,
            name.clone(),
            "",
            n,
            "ea",
        ));
    }

    // ---- Fixtures, Landscaping ----
    let listed = |kind: ScheduleKind| -> BTreeMap<(String, String), f64> {
        let mut m: BTreeMap<(String, String), f64> = BTreeMap::new();
        for e in entries(project, kind, None) {
            if range.contains(&e.floor)
                || (kind == ScheduleKind::Plant && scope == MaterialsScope::AllFloors)
            {
                *m.entry((e.name.clone(), e.size.clone())).or_default() += 1.0;
            }
        }
        m
    };
    for ((name, size), n) in listed(ScheduleKind::Fixture) {
        out.push(MaterialLine::new(
            "Fixtures",
            &name,
            name.clone(),
            &size,
            n,
            "ea",
        ));
    }
    for ((name, size), n) in listed(ScheduleKind::Plant) {
        out.push(MaterialLine::new(
            "Landscaping",
            &name,
            name.clone(),
            &size,
            n,
            "ea",
        ));
    }
    // The site's cut and fill (graded pads), in cubic yards.
    if range.contains(&0) || scope == MaterialsScope::AllFloors {
        let (cut, fill) = crate::terrain_report::soil_yards(project);
        for (item, qty) in [
            ("Soil cut (excavation)", cut),
            ("Soil fill (backfill)", fill),
        ] {
            if qty > 0.0 {
                out.push(MaterialLine::new(
                    "Landscaping",
                    item,
                    item,
                    "",
                    qty,
                    "cu yd",
                ));
            }
        }
    }

    // ---- Interior Finishes ----
    if t.drywall_sq_ft > 0.0 {
        out.push(MaterialLine::new(
            "Interior Finishes",
            "Wall drywall 1/2\" 4x8 sheet",
            "Wall drywall 1/2\" 4x8 sheet",
            "4x8",
            (t.drywall_sq_ft / SHEET_SQ_FT).ceil(),
            "sheet",
        ));
    }
    out.extend(room_lines);
    out
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
    use plan_core::{detect_rooms, Point};
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
}
