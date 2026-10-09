//! The Materials List data the plan keeps (manual pp. 1368-1392): the
//! information objects carry (Object Information and Components panels),
//! the saved Live Lists and static Reports (Project Browser, Materials List
//! Management) and the Materials List Polylines.
//!
//! These are plain data. The take-off, the columns' cell text, the exports and
//! the master list live in `plan-docs`; the window, the polyline tool and the
//! dialog panels live in `plan-app`.
//!
//! Objects are addressed by the same strings as the Property Manager
//! ([`crate::props::PropKey`]): `wall:12`, `door:5`, `cabinet:3`,
//! `device:0:4`, `room:0:120,84`, `foundation:7`, and so on.

use crate::geometry::Point;
use crate::model::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ columns --

/// The 21 columns of Chief's Materials List and Master List (p. 1377).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MlColumn {
    Id,
    Use,
    SubCategory,
    Floor,
    Label,
    Supplier,
    Manufacturer,
    Code,
    Size,
    Description,
    Quantity,
    Count,
    Extra,
    Price,
    Markup,
    Labor,
    Equipment,
    TotalCost,
    Default,
    Comment,
    AccountingCode,
}

impl MlColumn {
    /// Every column in Chief's order.
    pub const ALL: [MlColumn; 21] = [
        MlColumn::Id,
        MlColumn::Use,
        MlColumn::SubCategory,
        MlColumn::Floor,
        MlColumn::Label,
        MlColumn::Supplier,
        MlColumn::Manufacturer,
        MlColumn::Code,
        MlColumn::Size,
        MlColumn::Description,
        MlColumn::Quantity,
        MlColumn::Count,
        MlColumn::Extra,
        MlColumn::Price,
        MlColumn::Markup,
        MlColumn::Labor,
        MlColumn::Equipment,
        MlColumn::TotalCost,
        MlColumn::Default,
        MlColumn::Comment,
        MlColumn::AccountingCode,
    ];

    /// The heading Chief prints.
    pub fn title(self) -> &'static str {
        match self {
            MlColumn::Id => "ID",
            MlColumn::Use => "Use",
            MlColumn::SubCategory => "Sub Category",
            MlColumn::Floor => "Floor",
            MlColumn::Label => "Label",
            MlColumn::Supplier => "Supplier",
            MlColumn::Manufacturer => "Manufacturer",
            MlColumn::Code => "Code",
            MlColumn::Size => "Size",
            MlColumn::Description => "Description",
            MlColumn::Quantity => "Quantity",
            MlColumn::Count => "Count",
            MlColumn::Extra => "Extra",
            MlColumn::Price => "Price",
            MlColumn::Markup => "% Markup",
            MlColumn::Labor => "Labor",
            MlColumn::Equipment => "Equipment",
            MlColumn::TotalCost => "Total Cost",
            MlColumn::Default => "Default",
            MlColumn::Comment => "Comment",
            MlColumn::AccountingCode => "Accounting Code",
        }
    }

    /// Does a Materials List (as against the Master List only) have this
    /// column? Use, Quantity and Default belong to the Master List.
    pub fn in_materials_list(self) -> bool {
        !matches!(self, MlColumn::Use | MlColumn::Quantity | MlColumn::Default)
    }

    /// Does the Master List have this column? (Floor and Count/Extra/Total
    /// Cost are list-only.)
    pub fn in_master_list(self) -> bool {
        !matches!(
            self,
            MlColumn::Floor
                | MlColumn::SubCategory
                | MlColumn::Count
                | MlColumn::Extra
                | MlColumn::TotalCost
        )
    }

    /// Can the cell be typed into in a live list or a report?
    pub fn editable(self) -> bool {
        !matches!(
            self,
            MlColumn::Id
                | MlColumn::Floor
                | MlColumn::Use
                | MlColumn::Quantity
                | MlColumn::Default
                | MlColumn::TotalCost
        )
    }

    /// Right-aligned number columns.
    pub fn numeric(self) -> bool {
        matches!(
            self,
            MlColumn::Quantity
                | MlColumn::Count
                | MlColumn::Extra
                | MlColumn::Price
                | MlColumn::Markup
                | MlColumn::Labor
                | MlColumn::Equipment
                | MlColumn::TotalCost
        )
    }

    /// The width a new list gives the column, in points.
    pub fn default_width(self) -> f32 {
        match self {
            MlColumn::Id | MlColumn::Use | MlColumn::Default => 70.0,
            MlColumn::Description => 260.0,
            MlColumn::Size | MlColumn::Comment => 140.0,
            MlColumn::Supplier | MlColumn::Manufacturer | MlColumn::SubCategory => 110.0,
            MlColumn::AccountingCode | MlColumn::Label | MlColumn::Floor => 100.0,
            _ => 80.0,
        }
    }
}

/// One column of a list: shown or hidden, with its width. The order of the
/// vector is the order of the columns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColumnState {
    pub col: MlColumn,
    pub visible: bool,
    pub width: f32,
}

/// A new list's columns: all 21 in Chief's order, with the ones a take-off
/// needs first shown (ID, Size, Description, Count, Price, Total Cost).
pub fn default_columns() -> Vec<ColumnState> {
    MlColumn::ALL
        .iter()
        .map(|&col| ColumnState {
            col,
            visible: matches!(
                col,
                MlColumn::Id
                    | MlColumn::Size
                    | MlColumn::Description
                    | MlColumn::Count
                    | MlColumn::Price
                    | MlColumn::TotalCost
            ),
            width: col.default_width(),
        })
        .collect()
}

// -------------------------------------------------------------- list scopes --

/// An object a Selection list was made from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectAddr {
    pub floor: usize,
    /// The object's key (`wall:12`, `cabinet:3`, `room:0:120,84` ...).
    pub key: String,
}

/// What a Materials List calculates (the Tools > Materials List commands).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ListScope {
    /// Calculate Materials for All Floors.
    #[default]
    AllFloors,
    /// One floor (Restrict to Floor).
    Floor(usize),
    /// Calculate From Area: a Materials List Polyline (the CAD polyline's id).
    Polyline(Id),
    /// Calculate Materials in Room: the room of `floor` whose centre is near
    /// `(x, y)` inches.
    Room { floor: usize, x: f64, y: f64 },
    /// Calculate Materials From Selection.
    Selection(Vec<ObjectAddr>),
}

impl ListScope {
    /// Chief's name of the command that makes this scope.
    pub fn title(&self) -> &'static str {
        match self {
            ListScope::AllFloors => "All Floors",
            ListScope::Floor(_) => "Floor",
            ListScope::Polyline(_) => "Area",
            ListScope::Room { .. } => "Room",
            ListScope::Selection(_) => "Selection",
        }
    }
}

/// Live lists follow the model; a Report is frozen (p. 1369).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ListKind {
    #[default]
    Live,
    Report,
}

/// How the Structural Member Reporting control counts framing (p. 1370).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FramingStyle {
    /// The Buy List: stock lengths, spliced when longer than the longest.
    #[default]
    BuyList,
    /// The Cut List: one row per cut length, no rounding to stock.
    CutList,
    /// Linear feet per lumber size.
    LinearFeet,
}

impl FramingStyle {
    pub const ALL: [FramingStyle; 3] = [
        FramingStyle::BuyList,
        FramingStyle::CutList,
        FramingStyle::LinearFeet,
    ];

    pub fn title(self) -> &'static str {
        match self {
            FramingStyle::BuyList => "Buy List",
            FramingStyle::CutList => "Cut List",
            FramingStyle::LinearFeet => "Linear Feet",
        }
    }
}

/// Restrict to Supplier (p. 1374).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SupplierFilter {
    /// Show All Suppliers.
    #[default]
    All,
    /// Show Only No Supplier.
    NoSupplier,
    /// Only the items of this supplier.
    Only(String),
}

/// Which rows a Report groups under headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GroupBy {
    #[default]
    Category,
    Floor,
    Supplier,
    None,
}

impl GroupBy {
    pub const ALL: [GroupBy; 4] = [
        GroupBy::Category,
        GroupBy::Floor,
        GroupBy::Supplier,
        GroupBy::None,
    ];

    pub fn title(self) -> &'static str {
        match self {
            GroupBy::Category => "Category",
            GroupBy::Floor => "Floor",
            GroupBy::Supplier => "Supplier",
            GroupBy::None => "None",
        }
    }
}

/// What a list is ordered by inside its groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortKey {
    /// The take-off order (ID).
    #[default]
    Id,
    Description,
    Size,
    Count,
    TotalCost,
}

impl SortKey {
    pub const ALL: [SortKey; 5] = [
        SortKey::Id,
        SortKey::Description,
        SortKey::Size,
        SortKey::Count,
        SortKey::TotalCost,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SortKey::Id => "ID",
            SortKey::Description => "Description",
            SortKey::Size => "Size",
            SortKey::Count => "Count",
            SortKey::TotalCost => "Total Cost",
        }
    }
}

/// The Report tab: grouping, sorting and totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOptions {
    pub group_by: GroupBy,
    pub sort: SortKey,
    pub descending: bool,
    /// A subtotal under each group.
    pub subtotals: bool,
    /// A grand total at the foot.
    pub grand_total: bool,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            group_by: GroupBy::Category,
            sort: SortKey::Id,
            descending: false,
            subtotals: true,
            grand_total: true,
        }
    }
}

/// The Appearance panel (p. 1375), shown in the Text Style tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub horizontal_lines: bool,
    pub vertical_lines: bool,
    pub solid_lines: bool,
    pub custom_colors: bool,
    /// Background, text and grid colours when `custom_colors` is on.
    pub background: [u8; 3],
    pub text: [u8; 3],
    pub grid: [u8; 3],
    pub font: String,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            horizontal_lines: true,
            vertical_lines: true,
            solid_lines: true,
            custom_colors: false,
            background: [255, 255, 255],
            text: [0, 0, 0],
            grid: [192, 192, 192],
            font: "Arial".into(),
            font_size: 10.0,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
        }
    }
}

/// The Materials List Specification (p. 1372): General, Categories, Columns,
/// Report and Text Style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSpec {
    pub name: String,
    pub kind: ListKind,
    pub scope: ListScope,
    /// The categories the Categories panel left unchecked: suppressed from
    /// the view, but still part of the list and of exports (p. 1373).
    #[serde(default)]
    pub hidden_categories: Vec<String>,
    #[serde(default = "default_columns")]
    pub columns: Vec<ColumnState>,
    #[serde(default)]
    pub supplier: SupplierFilter,
    #[serde(default)]
    pub framing: FramingStyle,
    /// The layer on which a placed list (a Materials List Polyline label)
    /// draws; empty uses the default layer.
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub report: ReportOptions,
    #[serde(default)]
    pub appearance: Appearance,
}

impl Default for ListSpec {
    fn default() -> Self {
        ListSpec::new("Materials List", ListScope::AllFloors)
    }
}

impl ListSpec {
    pub fn new(name: impl Into<String>, scope: ListScope) -> Self {
        Self {
            name: name.into(),
            kind: ListKind::Live,
            scope,
            hidden_categories: Vec::new(),
            columns: default_columns(),
            supplier: SupplierFilter::All,
            framing: FramingStyle::BuyList,
            layer: String::new(),
            report: ReportOptions::default(),
            appearance: Appearance::default(),
        }
    }

    /// Is `category` shown?
    pub fn shows(&self, category: &str) -> bool {
        !self.hidden_categories.iter().any(|c| c == category)
    }

    /// Select All / Clear All of the Categories panel.
    pub fn show_all_categories(&mut self, show: bool) {
        self.hidden_categories = if show {
            Vec::new()
        } else {
            CATEGORIES.iter().map(|c| c.to_string()).collect()
        };
    }

    /// Checks or unchecks one category.
    pub fn set_category_shown(&mut self, category: &str, show: bool) {
        self.hidden_categories.retain(|c| c != category);
        if !show {
            self.hidden_categories.push(category.to_string());
        }
    }

    /// The visible columns, in order.
    pub fn visible_columns(&self) -> Vec<ColumnState> {
        self.columns.iter().copied().filter(|c| c.visible).collect()
    }

    /// Makes sure every column of Chief appears exactly once (a plan saved by
    /// an older build, or a hand-edited file, may lack some).
    pub fn normalize_columns(&mut self) {
        let mut seen: Vec<MlColumn> = Vec::new();
        self.columns.retain(|c| {
            if seen.contains(&c.col) {
                false
            } else {
                seen.push(c.col);
                true
            }
        });
        for col in MlColumn::ALL {
            if !seen.contains(&col) {
                self.columns.push(ColumnState {
                    col,
                    visible: false,
                    width: col.default_width(),
                });
            }
        }
    }

    /// Shows or hides a column.
    pub fn set_visible(&mut self, col: MlColumn, visible: bool) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.col == col) {
            c.visible = visible;
        }
    }

    /// Moves a column up (`-1`) or down (`1`) one place (Move Up / Move Down).
    pub fn move_column(&mut self, col: MlColumn, delta: i32) -> bool {
        let Some(i) = self.columns.iter().position(|c| c.col == col) else {
            return false;
        };
        let j = i as i32 + delta;
        if j < 0 || j as usize >= self.columns.len() {
            return false;
        }
        self.columns.swap(i, j as usize);
        true
    }

    /// Sets a column's width.
    pub fn set_width(&mut self, col: MlColumn, width: f32) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.col == col) {
            c.width = width.clamp(20.0, 800.0);
        }
    }
}

// --------------------------------------------------------- per-object data --

/// One component line of an object (a Components panel row) the user changed:
/// the line is found by the Materials List line key (`Framing|2x4 stud`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ComponentEdit {
    /// The line key of the component.
    pub key: String,
    /// Take the component out of the list (Remove Line Item).
    #[serde(default)]
    pub removed: bool,
    /// A different count for this object's share.
    #[serde(default)]
    pub count: Option<f64>,
    /// Price, markup, labor and equipment of this component.
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub markup: Option<f64>,
    #[serde(default)]
    pub labor: Option<f64>,
    #[serde(default)]
    pub equipment: Option<f64>,
    #[serde(default)]
    pub extra: Option<f64>,
}

impl ComponentEdit {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ..Self::default()
        }
    }

    /// Does the edit change nothing (so it can be dropped)?
    pub fn is_empty(&self) -> bool {
        !self.removed
            && self.count.is_none()
            && self.price.is_none()
            && self.markup.is_none()
            && self.labor.is_none()
            && self.equipment.is_none()
            && self.extra.is_none()
    }
}

/// A line the user added in a Components panel (Add Line Item).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtraComponent {
    pub category: String,
    pub description: String,
    pub size: String,
    pub unit: String,
    pub count: f64,
    pub price: Option<f64>,
    pub markup: f64,
    pub labor: f64,
    pub equipment: f64,
}

impl Default for ExtraComponent {
    fn default() -> Self {
        Self {
            category: "Fixtures".into(),
            description: String::new(),
            size: String::new(),
            unit: "ea".into(),
            count: 1.0,
            price: None,
            markup: 0.0,
            labor: 0.0,
            equipment: 0.0,
        }
    }
}

/// What an object tells the Materials List: the Object Information panel
/// (Code, Comment, Description, Manufacturer, Supplier) and the Components
/// panel (price, extra, markup, labor, equipment, per-component changes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ObjectInfo {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub supplier: String,
    #[serde(default)]
    pub sub_category: String,
    #[serde(default)]
    pub accounting_code: String,
    /// Size text typed over the automatic one (empty: automatic).
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub label: String,
    /// The category the user moved the object's lines to (empty: automatic).
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub extra: Option<f64>,
    #[serde(default)]
    pub markup: Option<f64>,
    #[serde(default)]
    pub labor: Option<f64>,
    #[serde(default)]
    pub equipment: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentEdit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<ExtraComponent>,
}

impl ObjectInfo {
    /// Does the object carry nothing (so its entry can be dropped)?
    pub fn is_empty(&self) -> bool {
        *self == ObjectInfo::default()
    }

    /// The edit for the component `key`.
    pub fn component(&self, key: &str) -> Option<&ComponentEdit> {
        self.components.iter().find(|c| c.key == key)
    }

    /// The edit for the component `key`, made when it is new.
    pub fn component_mut(&mut self, key: &str) -> &mut ComponentEdit {
        if let Some(i) = self.components.iter().position(|c| c.key == key) {
            return &mut self.components[i];
        }
        self.components.push(ComponentEdit::new(key));
        self.components.last_mut().expect("just pushed")
    }

    /// Drops the component edits that change nothing.
    pub fn prune(&mut self) {
        self.components.retain(|c| !c.is_empty());
    }
}

// --------------------------------------------------------- saved lists, etc --

/// One row of a static Report: the editable cells of a line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReportRow {
    pub category: String,
    pub id: String,
    pub sub_category: String,
    pub floor: String,
    pub label: String,
    pub supplier: String,
    pub manufacturer: String,
    pub code: String,
    pub size: String,
    pub description: String,
    pub count: f64,
    pub unit: String,
    pub extra: f64,
    pub price: Option<f64>,
    pub markup: f64,
    pub labor: f64,
    pub equipment: f64,
    pub comment: String,
    pub accounting_code: String,
    /// The master-list key the row was priced by.
    pub key: String,
}

/// A list saved with the plan: a Live List (re-calculated when opened) or a
/// Report (the frozen rows).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedList {
    pub spec: ListSpec,
    /// The rows of a Report; empty for a Live List.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<ReportRow>,
}

/// Which objects a Materials List Polyline takes in (p. 1383).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IncludedObjects {
    /// Any object whose bounding box the polyline touches or holds.
    Intersected,
    /// Only objects wholly inside.
    Contained,
    /// Objects whose centre is inside.
    #[default]
    ByCenter,
}

impl IncludedObjects {
    pub const ALL: [IncludedObjects; 3] = [
        IncludedObjects::Intersected,
        IncludedObjects::Contained,
        IncludedObjects::ByCenter,
    ];

    pub fn title(self) -> &'static str {
        match self {
            IncludedObjects::Intersected => "Include Intersected Objects",
            IncludedObjects::Contained => "Include Contained Objects",
            IncludedObjects::ByCenter => "Include Objects by Center",
        }
    }
}

/// The Materials List panel of a Materials List Polyline (also its defaults).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolylineSpec {
    /// The category x floor grid: the categories with the floors they are
    /// included on. A category that is missing is not included.
    pub included: BTreeMap<String, Vec<usize>>,
    /// Include All Floors (the grid has a column per floor); off shows only
    /// the polyline's own floor.
    pub all_floors: bool,
    pub objects: IncludedObjects,
}

impl PolylineSpec {
    /// Every category on every one of `floors` floors.
    pub fn everything(floors: usize) -> Self {
        let mut included = BTreeMap::new();
        for c in crate::materials_data::CATEGORIES {
            included.insert(c.to_string(), (0..floors).collect());
        }
        Self {
            included,
            all_floors: true,
            objects: IncludedObjects::ByCenter,
        }
    }

    /// Is `category` counted on `floor`?
    pub fn includes(&self, category: &str, floor: usize) -> bool {
        self.included
            .get(category)
            .is_some_and(|f| f.contains(&floor))
    }

    /// Toggles one field of the grid.
    pub fn toggle(&mut self, category: &str, floor: usize) {
        let row = self.included.entry(category.to_string()).or_default();
        if let Some(i) = row.iter().position(|f| *f == floor) {
            row.remove(i);
        } else {
            row.push(floor);
            row.sort_unstable();
        }
    }

    /// Toggle Category(s): every floor of the category on, or all off when
    /// the category is already on everywhere.
    pub fn toggle_category(&mut self, category: &str, floors: usize) {
        let row = self.included.entry(category.to_string()).or_default();
        if row.len() >= floors {
            row.clear();
        } else {
            *row = (0..floors).collect();
        }
    }

    /// Toggle Floor(s): every category on for the floor, or all off.
    pub fn toggle_floor(&mut self, floor: usize) {
        let all_on = CATEGORIES.iter().all(|c| self.includes(c, floor));
        for c in CATEGORIES {
            let row = self.included.entry(c.to_string()).or_default();
            row.retain(|f| *f != floor);
            if !all_on {
                row.push(floor);
                row.sort_unstable();
            }
        }
    }

    /// Toggle All: everything on, or everything off when all is on.
    pub fn toggle_all(&mut self, floors: usize) {
        let all_on = CATEGORIES
            .iter()
            .all(|c| (0..floors).all(|f| self.includes(c, f)));
        for c in CATEGORIES {
            self.included.insert(
                c.to_string(),
                if all_on {
                    Vec::new()
                } else {
                    (0..floors).collect()
                },
            );
        }
    }
}

/// A Materials List Polyline (p. 1382): the area is a closed CAD polyline of
/// the floor (so it moves, rotates and reshapes like one); this record holds
/// the Materials List panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialsPolyline {
    /// The closed CAD polyline that is the area.
    pub cad_id: Id,
    pub floor: usize,
    pub name: String,
    pub spec: PolylineSpec,
    /// Holes in the area (polyline holes, p. 1382).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Point>>,
}

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

/// Everything the plan keeps for the Materials List.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MaterialsData {
    /// Object Information and Components of single objects, by object key.
    /// A key starting `line:` holds what the user typed into a line of a list
    /// that stands for no single object (`line:Framing|2x4 stud`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub objects: BTreeMap<String, ObjectInfo>,
    /// Saved Live Lists and Reports (Project Browser, Materials List
    /// Management).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lists: Vec<SavedList>,
    /// The plan's Materials List Polylines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polylines: Vec<MaterialsPolyline>,
    /// Materials List Polyline Defaults (None: everything on all floors).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polyline_defaults: Option<PolylineSpec>,
}

impl MaterialsData {
    pub fn is_empty(&self) -> bool {
        *self == MaterialsData::default()
    }

    /// The information of object `key`.
    pub fn info(&self, key: &str) -> Option<&ObjectInfo> {
        self.objects.get(key)
    }

    /// The information of object `key`, made when it is new.
    pub fn info_mut(&mut self, key: &str) -> &mut ObjectInfo {
        self.objects.entry(key.to_string()).or_default()
    }

    /// Drops the entries that carry nothing.
    pub fn prune(&mut self) {
        for i in self.objects.values_mut() {
            i.prune();
        }
        self.objects.retain(|_, i| !i.is_empty());
    }

    /// The saved list called `name`.
    pub fn list(&self, name: &str) -> Option<&SavedList> {
        self.lists.iter().find(|l| l.spec.name == name)
    }

    /// A name for a new list that no saved list has: `name`, else
    /// `name 2`, `name 3` ...
    pub fn unique_name(&self, name: &str) -> String {
        let base = name.trim();
        let base = if base.is_empty() {
            "Materials List"
        } else {
            base
        };
        if self.list(base).is_none() {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| self.list(n).is_none())
            .expect("an unused name")
    }

    /// Saves (or replaces by name) a list. Returns the name it was saved as.
    pub fn save_list(&mut self, list: SavedList) -> String {
        let name = list.spec.name.clone();
        match self.lists.iter_mut().find(|l| l.spec.name == name) {
            Some(slot) => *slot = list,
            None => self.lists.push(list),
        }
        name
    }

    /// Save Active View As: a copy under a new name.
    pub fn copy_list(&mut self, name: &str, new_name: &str) -> Option<String> {
        let mut copy = self.list(name)?.clone();
        copy.spec.name = self.unique_name(new_name);
        let saved = copy.spec.name.clone();
        self.lists.push(copy);
        Some(saved)
    }

    /// Rename. False when `name` is missing or `new_name` is taken.
    pub fn rename_list(&mut self, name: &str, new_name: &str) -> bool {
        let new_name = new_name.trim();
        if new_name.is_empty() || (new_name != name && self.list(new_name).is_some()) {
            return false;
        }
        match self.lists.iter_mut().find(|l| l.spec.name == name) {
            Some(l) => {
                l.spec.name = new_name.to_string();
                true
            }
            None => false,
        }
    }

    /// Delete. False when there is no such list.
    pub fn delete_list(&mut self, name: &str) -> bool {
        let before = self.lists.len();
        self.lists.retain(|l| l.spec.name != name);
        self.lists.len() != before
    }

    /// The polyline whose area is the CAD object `cad_id`.
    pub fn polyline(&self, cad_id: Id) -> Option<&MaterialsPolyline> {
        self.polylines.iter().find(|p| p.cad_id == cad_id)
    }

    pub fn polyline_mut(&mut self, cad_id: Id) -> Option<&mut MaterialsPolyline> {
        self.polylines.iter_mut().find(|p| p.cad_id == cad_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_21_columns_with_chiefs_titles() {
        assert_eq!(MlColumn::ALL.len(), 21);
        let t: Vec<&str> = MlColumn::ALL.iter().map(|c| c.title()).collect();
        assert_eq!(t[0], "ID");
        assert!(t.contains(&"% Markup"));
        assert!(t.contains(&"Accounting Code"));
        assert_eq!(t[17], "Total Cost");
        // Use, Quantity, Default exist only in the Master List.
        assert!(!MlColumn::Use.in_materials_list());
        assert!(MlColumn::Use.in_master_list());
        assert!(!MlColumn::Floor.in_master_list());
    }

    #[test]
    fn column_order_move_and_normalize() {
        let mut s = ListSpec::new("L", ListScope::AllFloors);
        assert_eq!(s.visible_columns().len(), 6);
        assert!(s.move_column(MlColumn::Size, -1));
        assert_eq!(s.columns[1].col, MlColumn::Use);
        assert!(!s.move_column(MlColumn::Id, -1));
        s.columns.retain(|c| c.col != MlColumn::Comment);
        s.columns.push(s.columns[0]);
        s.normalize_columns();
        assert_eq!(s.columns.len(), 21);
        assert!(s.columns.iter().any(|c| c.col == MlColumn::Comment));
        s.set_width(MlColumn::Description, 5.0);
        assert_eq!(
            s.columns
                .iter()
                .find(|c| c.col == MlColumn::Description)
                .unwrap()
                .width,
            20.0
        );
    }

    #[test]
    fn saved_list_management() {
        let mut d = MaterialsData::default();
        let name = d.unique_name("Takeoff");
        d.save_list(SavedList {
            spec: ListSpec::new(&name, ListScope::AllFloors),
            rows: Vec::new(),
        });
        assert_eq!(d.unique_name("Takeoff"), "Takeoff 2");
        assert_eq!(
            d.copy_list("Takeoff", "Takeoff").as_deref(),
            Some("Takeoff 2")
        );
        assert!(!d.rename_list("Takeoff 2", "Takeoff"));
        assert!(d.rename_list("Takeoff 2", "Kitchen"));
        assert!(d.delete_list("Kitchen"));
        assert!(!d.delete_list("Kitchen"));
        assert_eq!(d.lists.len(), 1);
    }

    #[test]
    fn object_info_prunes_and_round_trips() {
        let mut d = MaterialsData::default();
        d.info_mut("wall:3").supplier = "Home Depot".into();
        d.info_mut("wall:3").component_mut("Framing|2x4 stud").price = Some(3.5);
        d.info_mut("door:9").component_mut("x");
        d.prune();
        assert!(d.info("door:9").is_none());
        let json = serde_json::to_string(&d).unwrap();
        let back: MaterialsData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        assert_eq!(
            back.info("wall:3")
                .unwrap()
                .component("Framing|2x4 stud")
                .unwrap()
                .price,
            Some(3.5)
        );
        // An empty slab serialises to nothing and loads from `{}`.
        let empty: MaterialsData = serde_json::from_str("{}").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn polyline_grid_toggles() {
        let mut p = PolylineSpec::everything(2);
        assert!(p.includes("Framing", 1));
        p.toggle("Framing", 1);
        assert!(!p.includes("Framing", 1));
        assert!(p.includes("Framing", 0));
        p.toggle_category("Roofing", 2);
        assert!(!p.includes("Roofing", 0));
        p.toggle_category("Roofing", 2);
        assert!(p.includes("Roofing", 1));
        // Framing is off on floor 1, so the floor is not all on: it goes on.
        p.toggle_floor(1);
        assert!(p.includes("Framing", 1));
        p.toggle_floor(1);
        assert!(!p.includes("Doors", 1));
        assert!(p.includes("Doors", 0));
        p.toggle_all(2);
        assert!(p.includes("Doors", 1));
        p.toggle_all(2);
        assert!(!p.includes("Doors", 0));
    }

    #[test]
    fn category_filter_defaults_to_all() {
        let mut s = ListSpec::new("L", ListScope::AllFloors);
        assert!(s.shows("Framing"));
        s.show_all_categories(false);
        s.set_category_shown("Doors", true);
        assert!(s.shows("Doors"));
        assert!(!s.shows("Framing"));
        s.show_all_categories(true);
        assert!(s.shows("Framing"));
    }
}
