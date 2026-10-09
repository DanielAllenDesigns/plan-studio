//! The Master List behind the Materials List (manual pp. 1387-1389): prices,
//! suppliers, manufacturers, codes, markups, labor and equipment for the items
//! of past take-offs, kept per user in `~/.plan-studio/master-list.json`.
//!
//! The file also carries the waste factor of each category and the lumber
//! stock lengths. Files written before the 21-column list have only a key,
//! unit, unit price and supplier per item; those still load, and still price a
//! row by its key (`Category|item`). Items made by Update to Master List also
//! carry the category, size, description, label, code and the other columns,
//! and are matched on those the way Chief matches them: same Category, Size,
//! Description and Label (or the same Code).
//!
//! The file belongs to the user: tests write only to a temporary folder.

use plan_core::materials_data::{ColumnState, MlColumn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// One entry of the master list: a row of Chief's Master List.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterItem {
    /// `Category|item`, as in `MaterialLine::key`. Empty for an item made by
    /// Update to Master List from an edited row.
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub unit_price: f64,
    #[serde(default)]
    pub supplier: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub size: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub accounting_code: String,
    /// Percent.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub markup: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub labor: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub equipment: f64,
    /// The Use column: an item that is not used is left out of the lists
    /// (several items bought as one unit).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub use_item: bool,
    /// The Quantity column: how many of the item a list needs before this
    /// entry applies (a quantity discount).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_quantity: f64,
    /// The Default column: wins over later entries for the same item.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_default: bool,
}

impl MasterItem {
    /// An entry with no information yet.
    pub fn blank() -> Self {
        Self {
            key: String::new(),
            unit: String::new(),
            unit_price: 0.0,
            supplier: String::new(),
            category: String::new(),
            size: String::new(),
            description: String::new(),
            label: String::new(),
            manufacturer: String::new(),
            code: String::new(),
            comment: String::new(),
            accounting_code: String::new(),
            markup: 0.0,
            labor: 0.0,
            equipment: 0.0,
            use_item: true,
            min_quantity: 0.0,
            is_default: false,
        }
    }
}

/// What a row tells the master list when it looks for its entry.
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemFacts<'a> {
    pub category: &'a str,
    pub size: &'a str,
    pub description: &'a str,
    pub label: &'a str,
    pub code: &'a str,
    /// The row's legacy price key (`Category|item`).
    pub key: &'a str,
    /// The row's count, for a Quantity threshold.
    pub quantity: f64,
}

fn default_waste() -> BTreeMap<String, f64> {
    [
        ("Foundation", 5.0),
        ("Framing", 10.0),
        ("Roofing", 10.0),
        ("Siding", 10.0),
        ("Interior Finishes", 10.0),
    ]
    .into_iter()
    .map(|(c, w)| (c.to_string(), w))
    .collect()
}

fn default_stock() -> Vec<u32> {
    vec![8, 10, 12, 14, 16]
}

/// Prices, waste factors and stock lengths behind the Materials List: the
/// Master List, kept per user in `~/.plan-studio/master-list.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterList {
    /// Waste per category, percent.
    #[serde(default = "default_waste")]
    pub waste: BTreeMap<String, f64>,
    /// Lumber stock lengths, feet, shortest first.
    #[serde(default = "default_stock")]
    pub stock_lengths_ft: Vec<u32>,
    #[serde(default)]
    pub items: Vec<MasterItem>,
    /// The columns the Master List window shows (its Columns panel); empty
    /// means [`default_master_columns`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnState>,
}

impl Default for MasterList {
    /// Waste of 10% on framing, roofing, siding and interior finishes, 5% on
    /// foundation concrete, none elsewhere; lumber in 8, 10, 12, 14 and 16
    /// foot lengths; no prices (an unpriced row shows no price).
    fn default() -> Self {
        Self {
            waste: default_waste(),
            stock_lengths_ft: default_stock(),
            items: Vec::new(),
            columns: Vec::new(),
        }
    }
}

/// The columns a Master List shows until the user changes them.
pub fn default_master_columns() -> Vec<ColumnState> {
    MlColumn::ALL
        .iter()
        .filter(|c| c.in_master_list())
        .map(|&col| ColumnState {
            col,
            visible: !matches!(col, MlColumn::Label | MlColumn::AccountingCode),
            width: col.default_width(),
        })
        .collect()
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
        self.items
            .iter()
            .find(|i| !i.key.is_empty() && i.key == key)
    }

    /// The unit price of `key`, if it is priced (a price of 0 is "not set").
    pub fn price_of(&self, key: &str) -> Option<f64> {
        self.item(key).map(|i| i.unit_price).filter(|p| *p > 0.0)
    }

    /// Sets the price of `key` (adds the entry when it is new).
    pub fn set_price(&mut self, key: &str, unit: &str, unit_price: f64) {
        match self
            .items
            .iter_mut()
            .find(|i| !i.key.is_empty() && i.key == key)
        {
            Some(i) => i.unit_price = unit_price,
            None => self.items.push(MasterItem {
                key: key.to_string(),
                unit: unit.to_string(),
                unit_price,
                ..MasterItem::blank()
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

    // ----- Chief's matching (Update from / to Master List) -----

    /// Does `item` stand for the row `f`? An entry made by Update to Master
    /// List matches on Category, Size, Description and Label, or on a Code;
    /// an older entry matches on its key.
    fn matches(item: &MasterItem, f: &ItemFacts) -> bool {
        let detailed = !item.description.is_empty()
            || !item.size.is_empty()
            || !item.code.is_empty()
            || !item.label.is_empty();
        if !detailed {
            return !item.key.is_empty() && item.key == f.key;
        }
        if !item.code.is_empty() && !f.code.is_empty() && item.code == f.code {
            return true;
        }
        (item.category.is_empty() || item.category == f.category)
            && item.size == f.size
            && item.description == f.description
            && item.label == f.label
    }

    /// The entry that applies to a row: among the entries that match and
    /// whose Quantity threshold the row reaches, the one marked Default, else
    /// the one entered last; the highest threshold first for discounts.
    pub fn find_for(&self, f: &ItemFacts) -> Option<&MasterItem> {
        let mut hits: Vec<&MasterItem> = self
            .items
            .iter()
            .filter(|i| Self::matches(i, f) && i.min_quantity <= f.quantity + 1e-9)
            .collect();
        if hits.is_empty() {
            return None;
        }
        if let Some(d) = hits.iter().rev().find(|i| i.is_default) {
            return Some(d);
        }
        // Several entries at different thresholds: the highest threshold the
        // row reaches wins; ties go to the entry entered last.
        let top = hits.iter().map(|i| i.min_quantity).fold(f64::MIN, f64::max);
        hits.retain(|i| (i.min_quantity - top).abs() < 1e-9);
        hits.last().copied()
    }

    /// Update to Master List: the entry with the same Category, Size,
    /// Description and Label (and Quantity threshold) takes the new values,
    /// else a new entry is added. Returns the index of the entry.
    pub fn update_to(&mut self, mut new: MasterItem) -> usize {
        let at = self.items.iter().position(|i| {
            i.category == new.category
                && i.size == new.size
                && i.description == new.description
                && i.label == new.label
                && (i.min_quantity - new.min_quantity).abs() < 1e-9
        });
        match at {
            Some(i) => {
                // Keep what the user set on the entry itself.
                new.use_item = self.items[i].use_item;
                new.is_default = self.items[i].is_default;
                if new.key.is_empty() {
                    new.key = self.items[i].key.clone();
                }
                self.items[i] = new;
                i
            }
            None => {
                self.items.push(new);
                self.items.len() - 1
            }
        }
    }

    /// Marks entry `index` as the Default of its item (clearing the others
    /// that stand for the same item).
    pub fn set_default(&mut self, index: usize) {
        let Some(target) = self.items.get(index).cloned() else {
            return;
        };
        for (i, it) in self.items.iter_mut().enumerate() {
            let same = it.category == target.category
                && it.size == target.size
                && it.description == target.description
                && it.label == target.label
                && it.key == target.key;
            if same {
                it.is_default = i == index;
            }
        }
    }

    /// Deletes entry `index`.
    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            true
        } else {
            false
        }
    }

    /// The categories the entries are in, in Chief's order, for the Category
    /// drop-down of the Master List.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = Vec::new();
        for i in &self.items {
            let c = i.category_name();
            if !c.is_empty() && !cats.contains(&c) {
                cats.push(c);
            }
        }
        let order = |c: &str| {
            plan_core::materials_data::CATEGORIES
                .iter()
                .position(|k| *k == c)
                .unwrap_or(usize::MAX)
        };
        cats.sort_by_key(|c| order(c));
        cats
    }

    /// The cell text of an entry in a Master List column.
    pub fn cell(&self, index: usize, col: MlColumn) -> String {
        let Some(i) = self.items.get(index) else {
            return String::new();
        };
        let money = |v: f64| {
            if v == 0.0 {
                String::new()
            } else {
                format!("{v:.2}")
            }
        };
        match col {
            MlColumn::Id => i.key.clone(),
            MlColumn::Use => if i.use_item { "yes" } else { "no" }.into(),
            MlColumn::Supplier => i.supplier.clone(),
            MlColumn::Manufacturer => i.manufacturer.clone(),
            MlColumn::Code => i.code.clone(),
            MlColumn::Size => i.size.clone(),
            MlColumn::Description => {
                if i.description.is_empty() {
                    i.key.split('|').nth(1).unwrap_or("").to_string()
                } else {
                    i.description.clone()
                }
            }
            MlColumn::Label => i.label.clone(),
            MlColumn::Quantity => {
                if i.min_quantity == 0.0 {
                    String::new()
                } else {
                    format!("{}", i.min_quantity)
                }
            }
            MlColumn::Price => money(i.unit_price),
            MlColumn::Markup => money(i.markup),
            MlColumn::Labor => money(i.labor),
            MlColumn::Equipment => money(i.equipment),
            MlColumn::Default => if i.is_default { "yes" } else { "" }.into(),
            MlColumn::Comment => i.comment.clone(),
            MlColumn::AccountingCode => i.accounting_code.clone(),
            MlColumn::SubCategory
            | MlColumn::Floor
            | MlColumn::Count
            | MlColumn::Extra
            | MlColumn::TotalCost => String::new(),
        }
    }

    /// Find (the Master List's Find field): the next cell after
    /// `(row, col_index)`, moving right then down, whose text contains
    /// `needle` (case-insensitive). `cols` are the columns shown.
    pub fn find_next(
        &self,
        cols: &[MlColumn],
        from: Option<(usize, usize)>,
        needle: &str,
    ) -> Option<(usize, usize)> {
        let needle = needle.trim().to_lowercase();
        if needle.is_empty() || cols.is_empty() {
            return None;
        }
        let start = from.map_or(0, |(r, c)| r * cols.len() + c + 1);
        (start..self.items.len() * cols.len())
            .map(|n| (n / cols.len(), n % cols.len()))
            .find(|(r, c)| self.cell(*r, cols[*c]).to_lowercase().contains(&needle))
    }
}

impl MasterItem {
    /// The category of the entry: its own, else the front of its key.
    pub fn category_name(&self) -> String {
        if !self.category.is_empty() {
            self.category.clone()
        } else {
            self.key.split('|').next().unwrap_or("").to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(cat: &'a str, size: &'a str, desc: &'a str, qty: f64) -> ItemFacts<'a> {
        ItemFacts {
            category: cat,
            size,
            description: desc,
            quantity: qty,
            ..ItemFacts::default()
        }
    }

    #[test]
    fn an_older_file_with_four_fields_per_item_still_loads_and_prices() {
        let text = r#"{"waste":{"Framing":12.0},"stock_lengths_ft":[8,12],
            "items":[{"key":"Framing|2x4 stud","unit":"ea","unit_price":3.5,"supplier":"Lumber Co"}]}"#;
        let m = MasterList::from_json(text).expect("loads");
        assert_eq!(m.price_of("Framing|2x4 stud"), Some(3.5));
        assert_eq!(m.items[0].supplier, "Lumber Co");
        assert!(m.items[0].use_item);
        assert_eq!(m.waste_for("Framing"), 12.0);
        // Saved again, the new columns stay out of the file until used.
        let json = m.to_json();
        assert!(!json.contains("manufacturer"), "{json}");
        assert!(!json.contains("use_item"), "{json}");
        assert_eq!(MasterList::from_json(&json).unwrap(), m);
        // A file with only prices still loads with the default waste.
        let bare = MasterList::from_json(r#"{"items":[]}"#).unwrap();
        assert_eq!(bare.waste_for("Framing"), 10.0);
        assert_eq!(bare.stock_lengths_ft, vec![8, 10, 12, 14, 16]);
    }

    #[test]
    fn update_to_matches_on_category_size_description_and_label() {
        let mut m = MasterList::default();
        let mut a = MasterItem::blank();
        a.category = "Doors".into();
        a.size = "3'-0\" x 6'-8\"".into();
        a.description = "Door".into();
        a.unit_price = 120.0;
        a.supplier = "A".into();
        let i = m.update_to(a.clone());
        let mut b = a.clone();
        b.unit_price = 135.0;
        let j = m.update_to(b);
        assert_eq!((i, j, m.items.len()), (0, 0, 1));
        assert_eq!(m.items[0].unit_price, 135.0);
        // A different size is a new entry.
        let mut c = a;
        c.size = "2'-8\" x 6'-8\"".into();
        assert_eq!(m.update_to(c), 1);
        // Lookup by the same facts.
        let hit = m
            .find_for(&facts("Doors", "3'-0\" x 6'-8\"", "Door", 1.0))
            .unwrap();
        assert_eq!(hit.unit_price, 135.0);
        assert!(m.find_for(&facts("Doors", "9'-0\"", "Door", 1.0)).is_none());
    }

    #[test]
    fn default_wins_then_the_last_entry_and_quantity_discounts_apply() {
        let mut m = MasterList::default();
        let mk = |price: f64, min: f64| {
            let mut i = MasterItem::blank();
            i.category = "Framing".into();
            i.description = "Stud".into();
            i.unit_price = price;
            i.min_quantity = min;
            i
        };
        // Same item entered twice (different suppliers): the last entry wins.
        m.items.push(mk(3.0, 0.0));
        m.items.push(mk(3.5, 0.0));
        let f = facts("Framing", "", "Stud", 10.0);
        assert_eq!(m.find_for(&f).unwrap().unit_price, 3.5);
        // Marking the first as Default makes it win.
        m.set_default(0);
        assert_eq!(m.find_for(&f).unwrap().unit_price, 3.0);
        // A discount entry applies only from its quantity.
        m.items.clear();
        m.items.push(mk(4.0, 0.0));
        m.items.push(mk(3.0, 100.0));
        assert_eq!(
            m.find_for(&facts("Framing", "", "Stud", 50.0))
                .unwrap()
                .unit_price,
            4.0
        );
        assert_eq!(
            m.find_for(&facts("Framing", "", "Stud", 100.0))
                .unwrap()
                .unit_price,
            3.0
        );
    }

    #[test]
    fn code_matches_across_descriptions_and_keys_match_older_entries() {
        let mut m = MasterList::default();
        m.set_price("Roofing|Shingle bundles", "bundle", 38.0);
        let f = ItemFacts {
            category: "Roofing",
            description: "Shingles (3 bundles per square)",
            key: "Roofing|Shingle bundles",
            quantity: 5.0,
            ..ItemFacts::default()
        };
        assert_eq!(m.find_for(&f).unwrap().unit_price, 38.0);
        let mut by_code = MasterItem::blank();
        by_code.code = "SKU-9".into();
        by_code.description = "Something else".into();
        by_code.unit_price = 9.0;
        m.items.push(by_code);
        let g = ItemFacts {
            code: "SKU-9",
            description: "Whatever the list calls it",
            ..ItemFacts::default()
        };
        assert_eq!(m.find_for(&g).unwrap().unit_price, 9.0);
    }

    #[test]
    fn find_walks_right_then_down() {
        let mut m = MasterList::default();
        for (d, s) in [("Stud", "Acme"), ("Plate", "Acme"), ("Joist", "Beta")] {
            let mut i = MasterItem::blank();
            i.description = d.into();
            i.supplier = s.into();
            m.items.push(i);
        }
        let cols = [MlColumn::Supplier, MlColumn::Description];
        assert_eq!(m.find_next(&cols, None, "acme"), Some((0, 0)));
        assert_eq!(m.find_next(&cols, Some((0, 0)), "acme"), Some((1, 0)));
        assert_eq!(m.find_next(&cols, Some((1, 0)), "joist"), Some((2, 1)));
        assert_eq!(m.find_next(&cols, Some((2, 1)), "joist"), None);
    }

    #[test]
    fn categories_come_in_chiefs_order() {
        let mut m = MasterList::default();
        m.set_price("Roofing|x", "ea", 1.0);
        m.set_price("Foundation|y", "ea", 1.0);
        assert_eq!(m.categories(), vec!["Foundation", "Roofing"]);
        assert!(m.remove(0));
        assert!(!m.remove(5));
    }

    #[test]
    fn delete_and_default_columns() {
        let cols = default_master_columns();
        assert!(cols.iter().any(|c| c.col == MlColumn::Use));
        assert!(!cols.iter().any(|c| c.col == MlColumn::Floor));
        assert!(
            !cols
                .iter()
                .find(|c| c.col == MlColumn::Label)
                .unwrap()
                .visible
        );
    }
}
