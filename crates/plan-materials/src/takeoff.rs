//! Materials List by surface: how much of each library material the plan's
//! surfaces use, with the Materials List tab's columns (name, manufacturer,
//! supplier, price, unit) and the quantity and cost they give.
//!
//! The app collects the surfaces of a region (a room, a floor or the whole
//! plan) as [`SurfaceArea`]s; this module adds them up per material.

use std::collections::BTreeMap;

use crate::library::MaterialLibrary;
use crate::material::PriceUnit;

/// Column headings of the by-surface Materials List.
pub const SURFACE_COLUMNS: [&str; 10] = [
    "Material",
    "Category",
    "Manufacturer",
    "Supplier",
    "Surfaces",
    "Area (sq ft)",
    "Price",
    "Unit",
    "Quantity",
    "Cost",
];

/// The part of the plan a take-off covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// The surfaces of one room.
    Room(usize),
    /// The surfaces of the active floor.
    Floor,
    /// Every floor.
    Plan,
}

/// One surface and the material it shows.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceArea {
    /// A library material name (or a blend name, see [`crate::blend_name`]).
    pub material: String,
    /// Net area, square inches.
    pub area_sq_in: f64,
    /// What it is ("Exterior wall", "Floor", "Roof"...), for the Surfaces column.
    pub what: String,
}

/// The line of one material.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialQuantity {
    pub name: String,
    pub category: String,
    pub manufacturer: String,
    pub supplier: String,
    /// The kinds of surface that use it, e.g. "Floor, Ceiling".
    pub surfaces: String,
    pub area_sq_ft: f64,
    pub price: f64,
    pub unit: PriceUnit,
    /// How many `unit`s the area comes to (an each-priced material counts
    /// its surfaces; a linear-foot one cannot be derived from an area and
    /// shows 0).
    pub quantity: f64,
    pub cost: f64,
    /// False when the library does not know the material (it keeps its area).
    pub known: bool,
}

impl MaterialQuantity {
    /// The cells in [`SURFACE_COLUMNS`] order.
    pub fn cells(&self) -> [String; 10] {
        [
            self.name.clone(),
            self.category.clone(),
            self.manufacturer.clone(),
            self.supplier.clone(),
            self.surfaces.clone(),
            format!("{:.1}", self.area_sq_ft),
            if self.price > 0.0 {
                format!("{:.2}", self.price)
            } else {
                String::new()
            },
            self.unit.label().to_string(),
            format!("{:.2}", self.quantity),
            if self.cost > 0.0 {
                format!("{:.2}", self.cost)
            } else {
                String::new()
            },
        ]
    }
}

/// Adds `surfaces` up per material, largest area first. Surfaces of a
/// material the library lacks are kept under their own name.
pub fn summarize(lib: &MaterialLibrary, surfaces: &[SurfaceArea]) -> Vec<MaterialQuantity> {
    struct Acc {
        sq_in: f64,
        count: u32,
        what: Vec<String>,
    }
    let mut by: BTreeMap<&str, Acc> = BTreeMap::new();
    for s in surfaces
        .iter()
        .filter(|s| s.area_sq_in.is_finite() && s.area_sq_in > 0.0)
    {
        let a = by.entry(s.material.as_str()).or_insert(Acc {
            sq_in: 0.0,
            count: 0,
            what: Vec::new(),
        });
        a.sq_in += s.area_sq_in;
        a.count += 1;
        if !a.what.contains(&s.what) {
            a.what.push(s.what.clone());
        }
    }
    let mut out: Vec<MaterialQuantity> = by
        .into_iter()
        .map(|(name, a)| {
            let def = lib.resolve(name);
            let area_sq_ft = a.sq_in / 144.0;
            let (price, unit) = def
                .as_ref()
                .map_or((0.0, PriceUnit::SqFt), |d| d.quoted_price());
            let quantity = match unit.sq_ft() {
                Some(per) => area_sq_ft / per,
                None if unit == PriceUnit::Each => f64::from(a.count),
                None => 0.0,
            };
            MaterialQuantity {
                name: name.to_string(),
                category: def
                    .as_ref()
                    .map_or(String::new(), |d| d.category.join(" > ")),
                manufacturer: def
                    .as_ref()
                    .map_or(String::new(), |d| d.manufacturer.clone()),
                supplier: def.as_ref().map_or(String::new(), |d| d.supplier.clone()),
                surfaces: a.what.join(", "),
                area_sq_ft,
                price,
                unit,
                quantity,
                cost: price * quantity,
                known: def.is_some(),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.area_sq_ft
            .total_cmp(&a.area_sq_ft)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// The list as CSV (header row first).
pub fn to_csv(lines: &[MaterialQuantity]) -> String {
    let quote = |s: &str| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = SURFACE_COLUMNS.join(",");
    out.push('\n');
    for l in lines {
        let cells: Vec<String> = l.cells().iter().map(|c| quote(c)).collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core_library, MaterialDef};

    fn area(m: &str, sq_ft: f64, what: &str) -> SurfaceArea {
        SurfaceArea {
            material: m.into(),
            area_sq_in: sq_ft * 144.0,
            what: what.into(),
        }
    }

    #[test]
    fn areas_add_up_per_material_and_price_by_unit() {
        let mut lib = core_library();
        let mut tile = MaterialDef::new("Slate Tile", &["Flooring"], [60, 60, 70]);
        tile.manufacturer = "Acme".into();
        tile.supplier = "Tile Depot".into();
        tile.price = 54.0;
        tile.unit = PriceUnit::SqYd;
        lib.add(tile);
        let mut each = MaterialDef::new("Mosaic Medallion", &["Flooring"], [1, 2, 3]);
        each.price = 80.0;
        each.unit = PriceUnit::Each;
        lib.add(each);
        let lines = summarize(
            &lib,
            &[
                area("Slate Tile", 90.0, "Floor"),
                area("Slate Tile", 90.0, "Floor"),
                area("Slate Tile", 18.0, "Wall"),
                area("Mosaic Medallion", 4.0, "Floor"),
                area("Mosaic Medallion", 4.0, "Floor"),
                area("Mystery", 5.0, "Wall"),
                area("Slate Tile", 0.0, "Floor"),
            ],
        );
        assert_eq!(lines.len(), 3);
        let slate = &lines[0];
        assert_eq!(slate.name, "Slate Tile");
        assert!((slate.area_sq_ft - 198.0).abs() < 1e-9);
        assert_eq!(slate.surfaces, "Floor, Wall");
        assert_eq!(
            (slate.manufacturer.as_str(), slate.supplier.as_str()),
            ("Acme", "Tile Depot")
        );
        assert!((slate.quantity - 22.0).abs() < 1e-9, "198 sq ft = 22 sq yd");
        assert!((slate.cost - 22.0 * 54.0).abs() < 1e-6);
        let medallion = lines.iter().find(|l| l.name == "Mosaic Medallion").unwrap();
        assert_eq!(medallion.quantity, 2.0);
        assert_eq!(medallion.cost, 160.0);
        let mystery = lines.iter().find(|l| l.name == "Mystery").unwrap();
        assert!(!mystery.known && mystery.cost == 0.0);
        assert!((mystery.area_sq_ft - 5.0).abs() < 1e-9);
    }

    #[test]
    fn the_older_square_foot_cost_still_prices_a_line() {
        let mut lib = MaterialLibrary::default();
        lib.add(MaterialDef::new("Old", &["Custom"], [0; 3]).with_cost(3.5, "X"));
        let l = summarize(&lib, &[area("Old", 100.0, "Wall")]);
        assert_eq!((l[0].price, l[0].unit), (3.5, PriceUnit::SqFt));
        assert!((l[0].cost - 350.0).abs() < 1e-9);
    }

    #[test]
    fn csv_has_the_columns_and_quotes_commas() {
        let mut lib = MaterialLibrary::default();
        let mut m = MaterialDef::new("Tile, Gray", &["Flooring"], [0; 3]);
        m.price = 2.0;
        lib.add(m);
        let csv = to_csv(&summarize(&lib, &[area("Tile, Gray", 10.0, "Floor")]));
        let mut rows = csv.lines();
        assert_eq!(rows.next().unwrap(), SURFACE_COLUMNS.join(","));
        assert!(rows.next().unwrap().starts_with("\"Tile, Gray\",Flooring,"));
    }
}
