//! The Materials List as Chief shows it (manual pp. 1368-1392): a list made
//! from a [`ListSpec`] (scope, categories, columns, report options), with the
//! 21 columns, per-object information and component changes, the master list
//! applied by Chief's rules, totals by the Total Cost formula, and the
//! operations on rows (expand, details, edit a cell, update to and from the
//! master list, freeze into a Report).

use super::engine::{self, Filter, Options, Raw, Rule, Source};
use super::{round_up, MasterList, MaterialLine, MaterialsScope};
use crate::master_list::{ItemFacts, MasterItem};
use plan_core::cad::CadItem;
use plan_core::geometry::Point;
use plan_core::materials_data::{
    GroupBy, ListScope, ListSpec, MlColumn, ObjectAddr, ObjectInfo, PolylineSpec, ReportRow,
    SortKey, SupplierFilter, CATEGORIES,
};
use plan_core::{detect_rooms, Project, Room};
use std::collections::BTreeSet;

// -------------------------------------------------------------------- lines --

/// One row of a list: the take-off line with the other columns of Chief's
/// Materials List.
#[derive(Debug, Clone)]
pub struct ListLine {
    /// Category, Description (`item`), Count (`quantity`), unit, ID, Size, net
    /// count, waste, price per unit (`unit_price`) and Total Cost (`price`).
    pub line: MaterialLine,
    pub sub_category: String,
    pub floor: String,
    pub label: String,
    pub supplier: String,
    pub manufacturer: String,
    pub code: String,
    pub comment: String,
    pub accounting_code: String,
    pub extra: f64,
    pub markup: f64,
    pub labor: f64,
    pub equipment: f64,
    /// The objects behind the row.
    pub sources: Vec<Source>,
    pub rule: Rule,
    /// Nesting of an expanded row: 1 for the objects under a line.
    pub depth: u8,
    /// Index of the line an expanded row belongs to.
    pub parent: Option<usize>,
}

impl ListLine {
    /// The Total Cost formula of the manual (p. 1380):
    /// `(Count + Extra) * Price * (1 + Markup/100) + (Count + Extra) * Labor +
    /// (Count + Extra) * Equipment`. `None` while nothing is priced.
    pub fn total_cost(&self) -> Option<f64> {
        total_cost(
            self.line.quantity,
            self.extra,
            self.line.unit_price,
            self.markup,
            self.labor,
            self.equipment,
        )
    }

    /// Sets `line.price` from the columns.
    pub fn refresh_total(&mut self) {
        self.line.price = self.total_cost();
    }

    /// The facts the master list matches a row on.
    pub fn facts(&self) -> ItemFacts<'_> {
        ItemFacts {
            category: &self.line.category,
            size: &self.line.size,
            description: &self.line.item,
            label: &self.label,
            code: &self.code,
            key: &self.line.key,
            quantity: self.line.quantity,
        }
    }
}

/// The Total Cost formula on plain numbers.
pub fn total_cost(
    count: f64,
    extra: f64,
    price: Option<f64>,
    markup: f64,
    labor: f64,
    equipment: f64,
) -> Option<f64> {
    if price.is_none() && labor == 0.0 && equipment == 0.0 {
        return None;
    }
    let n = count + extra;
    Some(n * price.unwrap_or(0.0) * (1.0 + markup / 100.0) + n * labor + n * equipment)
}

/// A list made from a spec.
#[derive(Debug, Clone, Default)]
pub struct Calculated {
    pub lines: Vec<ListLine>,
    /// Why the list is empty or short, when the scope could not be resolved
    /// (a deleted polyline or room).
    pub note: Option<String>,
}

impl Calculated {
    /// The sum of the Total Cost column.
    pub fn total(&self) -> Option<f64> {
        total_of(&self.lines)
    }
}

/// The sum of the Total Cost of `lines` (`None` when nothing is priced).
pub fn total_of(lines: &[ListLine]) -> Option<f64> {
    let mut any = false;
    let mut sum = 0.0;
    for l in lines {
        if let Some(t) = l.line.price {
            any = true;
            sum += t;
        }
    }
    any.then_some(sum)
}

/// What a list is calculated from.
pub struct Calc<'a> {
    pub project: &'a Project,
    pub master: &'a MasterList,
    /// The detected rooms of the floor being edited (others are detected
    /// here).
    pub active_rooms: Option<(usize, &'a [Room])>,
}

// -------------------------------------------------------------------- scope --

struct Resolved {
    floors: Vec<usize>,
    opts: Options,
    allow: Option<(PolylineSpec, Vec<usize>)>,
    note: Option<String>,
}

/// The CAD polyline that is the area of Materials List Polyline `cad_id`.
pub fn polyline_points(project: &Project, cad_id: plan_core::Id) -> Option<(usize, Vec<Point>)> {
    let pl = project.materials.polyline(cad_id)?;
    let f = project.floors.get(pl.floor)?;
    let c = f.cad.iter().find(|c| c.id == cad_id)?;
    match &c.item {
        CadItem::Polyline { points, .. } if points.len() >= 3 => Some((pl.floor, points.clone())),
        _ => None,
    }
}

/// The room of `floor` whose centre is nearest `(x, y)`.
pub fn find_room(
    project: &Project,
    active: Option<(usize, &[Room])>,
    floor: usize,
    x: f64,
    y: f64,
) -> Option<Room> {
    let detected;
    let rooms: &[Room] = match active {
        Some((a, r)) if a == floor => r,
        _ => {
            detected = detect_rooms(&project.floors.get(floor)?.walls, 1.0);
            &detected
        }
    };
    let p = Point::new(x, y);
    rooms
        .iter()
        .min_by(|a, b| {
            a.centroid
                .dist(p)
                .partial_cmp(&b.centroid.dist(p))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn resolve(c: &Calc, spec: &ListSpec) -> Resolved {
    let n = c.project.floors.len();
    let mut r = Resolved {
        floors: (0..n).collect(),
        opts: Options {
            framing: spec.framing,
            ..Options::default()
        },
        allow: None,
        note: None,
    };
    match &spec.scope {
        ListScope::AllFloors => r.opts.site_everywhere = true,
        ListScope::Floor(f) => r.floors = vec![(*f).min(n.saturating_sub(1))],
        ListScope::Polyline(id) => match (
            c.project.materials.polyline(*id),
            polyline_points(c.project, *id),
        ) {
            (Some(pl), Some((floor, points))) => {
                r.floors = if pl.spec.all_floors {
                    (0..n).collect()
                } else {
                    vec![floor]
                };
                r.opts.filter = Filter::Area {
                    polygon: points,
                    holes: pl.holes.clone(),
                    mode: pl.spec.objects,
                };
                r.allow = Some((pl.spec.clone(), r.floors.clone()));
            }
            _ => {
                r.floors.clear();
                r.note = Some("The Materials List Polyline is gone".into());
            }
        },
        ListScope::Room { floor, x, y } => {
            match find_room(c.project, c.active_rooms, *floor, *x, *y) {
                Some(room) => {
                    r.floors = vec![*floor];
                    r.opts.filter = Filter::Room {
                        floor: *floor,
                        key: engine::room_key(*floor, &room),
                        polygon: room.polygon.clone(),
                    };
                }
                None => {
                    r.floors.clear();
                    r.note = Some("There is no room there".into());
                }
            }
        }
        ListScope::Selection(addrs) => {
            let set: BTreeSet<(usize, String)> =
                addrs.iter().map(|a| (a.floor, a.key.clone())).collect();
            let mut floors: Vec<usize> = set.iter().map(|(f, _)| *f).collect();
            floors.sort_unstable();
            floors.dedup();
            if set.iter().any(|(_, k)| k == "terrain") && !floors.contains(&0) {
                floors.insert(0, 0);
            }
            r.floors = floors;
            r.opts.filter = Filter::Selection(set);
        }
    }
    r
}

// ----------------------------------------------------------- object details --

/// What the objects say about a line: the Object Information and the changes
/// to the component, merged over the sources of one group.
#[derive(Debug, Clone, PartialEq, Default)]
struct Meta {
    category: String,
    sub_category: String,
    label: String,
    supplier: String,
    manufacturer: String,
    code: String,
    comment: String,
    description: String,
    size: String,
    accounting_code: String,
    price: Option<f64>,
    extra: Option<f64>,
    markup: Option<f64>,
    labor: Option<f64>,
    equipment: Option<f64>,
}

impl Meta {
    fn of(obj: Option<&ObjectInfo>, line_info: Option<&ObjectInfo>, key: &str) -> Meta {
        let mut m = Meta::default();
        for info in [obj, line_info].into_iter().flatten() {
            let pick = |dst: &mut String, v: &str| {
                if !v.is_empty() {
                    *dst = v.to_string();
                }
            };
            pick(&mut m.category, &info.category);
            pick(&mut m.sub_category, &info.sub_category);
            pick(&mut m.label, &info.label);
            pick(&mut m.supplier, &info.supplier);
            pick(&mut m.manufacturer, &info.manufacturer);
            pick(&mut m.code, &info.code);
            pick(&mut m.comment, &info.comment);
            pick(&mut m.description, &info.description);
            pick(&mut m.size, &info.size);
            pick(&mut m.accounting_code, &info.accounting_code);
            for (dst, v) in [
                (&mut m.price, info.price),
                (&mut m.extra, info.extra),
                (&mut m.markup, info.markup),
                (&mut m.labor, info.labor),
                (&mut m.equipment, info.equipment),
            ] {
                if v.is_some() {
                    *dst = v;
                }
            }
            if let Some(c) = info.component(key) {
                for (dst, v) in [
                    (&mut m.price, c.price),
                    (&mut m.extra, c.extra),
                    (&mut m.markup, c.markup),
                    (&mut m.labor, c.labor),
                    (&mut m.equipment, c.equipment),
                ] {
                    if v.is_some() {
                        *dst = v;
                    }
                }
            }
        }
        m
    }
}

fn unit_scale(rule: Rule) -> f64 {
    if rule == Rule::Sheets {
        32.0
    } else {
        1.0
    }
}

fn floor_names(project: &Project, sources: &[Source]) -> String {
    let mut floors: Vec<usize> = sources.iter().map(|s| s.floor).collect();
    floors.sort_unstable();
    floors.dedup();
    floors
        .iter()
        .filter_map(|f| project.floors.get(*f))
        .map(|f| f.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Splits each take-off line by what its objects say, so objects that differ
/// (a supplier, a price, a removed component) get rows of their own, and
/// applies the count changes. Components the user added join the list.
fn apply_objects(
    project: &Project,
    raws: Vec<Raw>,
    scope_keys: &BTreeSet<(usize, String)>,
) -> Vec<(Raw, Meta)> {
    let data = &project.materials;
    let mut out: Vec<(Raw, Meta)> = Vec::new();
    let mut seen: BTreeSet<(usize, String)> = scope_keys.clone();
    for raw in raws {
        let line_info = data.info(&format!("line:{}", raw.line.key));
        if raw.sources.is_empty() {
            let meta = Meta::of(None, line_info, &raw.line.key);
            out.push((raw, meta));
            continue;
        }
        let scale = unit_scale(raw.rule);
        let mut groups: Vec<(Meta, Vec<Source>)> = Vec::new();
        for s in &raw.sources {
            seen.insert((s.floor, s.key.clone()));
            let obj = data.info(&s.key);
            let mut s = s.clone();
            if let Some(c) = obj.and_then(|o| o.component(&raw.line.key)) {
                if c.removed {
                    continue;
                }
                if let Some(n) = c.count {
                    s.qty = n * scale;
                }
            }
            let meta = Meta::of(obj, line_info, &raw.line.key);
            match groups.iter_mut().find(|(m, _)| *m == meta) {
                Some((_, v)) => v.push(s),
                None => groups.push((meta, vec![s])),
            }
        }
        for (meta, sources) in groups {
            let mut r = raw.clone();
            r.sources = sources;
            r.recompute();
            out.push((r, meta));
        }
    }
    // Added components (Components panel > Add Line Item).
    for (key, info) in &data.objects {
        if info.added.is_empty() {
            continue;
        }
        let Some((floor, _)) = seen.iter().find(|(_, k)| k == key) else {
            continue;
        };
        for a in &info.added {
            let mut line = MaterialLine::new(
                &a.category,
                &a.description,
                a.description.clone(),
                &a.size,
                a.count,
                &a.unit,
            );
            line.net = a.count;
            let raw = Raw {
                line,
                sources: vec![Source {
                    floor: *floor,
                    key: key.clone(),
                    qty: a.count,
                }],
                rule: Rule::Sum,
            };
            let meta = Meta {
                price: a.price,
                markup: Some(a.markup).filter(|v| *v != 0.0),
                labor: Some(a.labor).filter(|v| *v != 0.0),
                equipment: Some(a.equipment).filter(|v| *v != 0.0),
                ..Meta::of(Some(info), None, "")
            };
            out.push((raw, meta));
        }
    }
    out
}

fn category_order(c: &str) -> usize {
    CATEGORIES
        .iter()
        .position(|k| *k == c)
        .unwrap_or(CATEGORIES.len())
}

fn id_prefix(category: &str) -> &'static str {
    match category {
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
    }
}

/// Orders the rows by category and numbers each category (`FRM-001`).
pub fn number_lines(lines: &mut [ListLine]) {
    lines.sort_by_key(|l| category_order(&l.line.category));
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for l in lines.iter_mut() {
        let n = counts.entry(l.line.category.clone()).or_default();
        *n += 1;
        l.line.id = format!("{}-{:03}", id_prefix(&l.line.category), n);
    }
}

// ---------------------------------------------------------------- calculate --

/// Calculates the list `spec` describes: the scope's take-off, the objects'
/// information, the master list, waste, totals and ids.
pub fn calculate(c: &Calc, spec: &ListSpec) -> Calculated {
    let res = resolve(c, spec);
    if res.floors.is_empty() {
        return Calculated {
            lines: Vec::new(),
            note: res.note,
        };
    }
    let mut raws =
        engine::take_off_raw(c.project, &res.floors, c.active_rooms, c.master, &res.opts);
    // The category x floor grid of a Materials List Polyline.
    if let Some((grid, _)) = &res.allow {
        for r in &mut raws {
            let had = !r.sources.is_empty();
            let cat = r.line.category.clone();
            r.sources.retain(|s| grid.includes(&cat, s.floor));
            if had {
                r.recompute();
            }
        }
        raws.retain(|r| !r.sources.is_empty());
    }
    let scope_keys: BTreeSet<(usize, String)> = match &spec.scope {
        ListScope::Selection(a) => a.iter().map(|x| (x.floor, x.key.clone())).collect(),
        _ => BTreeSet::new(),
    };
    let work = apply_objects(c.project, raws, &scope_keys);
    let mut lines: Vec<ListLine> = Vec::new();
    for (raw, meta) in work {
        let Raw {
            mut line,
            sources,
            rule,
        } = raw;
        let first_floor = sources.first().map_or(0, |s| s.floor);
        let mut meta = meta;
        let x = |t: &str| expand_text(c.project, first_floor, t);
        meta.description = x(&meta.description);
        meta.size = x(&meta.size);
        meta.sub_category = x(&meta.sub_category);
        meta.label = x(&meta.label);
        meta.supplier = x(&meta.supplier);
        meta.manufacturer = x(&meta.manufacturer);
        meta.code = x(&meta.code);
        meta.comment = x(&meta.comment);
        meta.accounting_code = x(&meta.accounting_code);
        line.item = x(&line.item);
        line.size = x(&line.size);
        if !meta.category.is_empty() && CATEGORIES.contains(&meta.category.as_str()) {
            line.category = meta.category.clone();
        }
        if !meta.description.is_empty() {
            line.item = meta.description.clone();
        }
        if !meta.size.is_empty() {
            line.size = if meta.size == " " {
                String::new()
            } else {
                meta.size.clone()
            };
        }
        let set = Set {
            price: meta.price.is_some(),
            markup: meta.markup.is_some(),
            labor: meta.labor.is_some(),
            equipment: meta.equipment.is_some(),
        };
        let mut l = ListLine {
            floor: floor_names(c.project, &sources),
            sub_category: meta.sub_category,
            label: meta.label,
            supplier: meta.supplier,
            manufacturer: meta.manufacturer,
            code: meta.code,
            comment: meta.comment,
            accounting_code: meta.accounting_code,
            extra: meta.extra.unwrap_or(0.0),
            markup: meta.markup.unwrap_or(0.0),
            labor: meta.labor.unwrap_or(0.0),
            equipment: meta.equipment.unwrap_or(0.0),
            sources,
            rule,
            depth: 0,
            parent: None,
            line: {
                line.waste_pct = c.master.waste_for(&line.category);
                line
            },
        };
        l.line.unit_price = meta.price;
        // The master list fills what the objects left empty.
        let item = c.master.find_for(&l.facts()).cloned();
        if let Some(it) = item {
            if !it.use_item {
                continue;
            }
            fill_from_item(&mut l, &it, &set);
        }
        // Count: the net quantity plus waste, rounded up (board feet and the
        // roof area are information, not purchases).
        let info = l.line.unit == "bf" || l.line.key == "Roofing|Roof area";
        l.line.quantity = if info {
            l.line.net
        } else {
            round_up(l.line.net * (1.0 + l.line.waste_pct / 100.0), &l.line.unit)
        };
        l.refresh_total();
        lines.push(l);
    }
    // Restrict to Supplier.
    lines.retain(|l| match &spec.supplier {
        SupplierFilter::All => true,
        SupplierFilter::NoSupplier => l.supplier.is_empty(),
        SupplierFilter::Only(s) => l.supplier == *s,
    });
    number_lines(&mut lines);
    Calculated {
        lines,
        note: res.note,
    }
}

/// Which of the money columns the objects set (so the master list leaves them).
#[derive(Clone, Copy)]
struct Set {
    price: bool,
    markup: bool,
    labor: bool,
    equipment: bool,
}

/// Copies an item's information into the columns the objects left blank
/// (an entry in the Master List is not used when the object's own dialog has
/// one, p. 1377).
fn fill_from_item(l: &mut ListLine, it: &MasterItem, set: &Set) {
    let fill = |dst: &mut String, v: &str| {
        if dst.is_empty() && !v.is_empty() {
            *dst = v.to_string();
        }
    };
    fill(&mut l.supplier, &it.supplier);
    fill(&mut l.manufacturer, &it.manufacturer);
    fill(&mut l.code, &it.code);
    fill(&mut l.comment, &it.comment);
    fill(&mut l.accounting_code, &it.accounting_code);
    if !set.price && it.unit_price > 0.0 {
        l.line.unit_price = Some(it.unit_price);
    }
    if !set.markup {
        l.markup = it.markup;
    }
    if !set.labor {
        l.labor = it.labor;
    }
    if !set.equipment {
        l.equipment = it.equipment;
    }
}

/// The plain scope of a [`MaterialsScope`], as a spec (a bridge for callers
/// of the older API).
pub fn spec_of_scope(scope: MaterialsScope) -> ListSpec {
    ListSpec::new(
        "Materials List",
        match scope {
            MaterialsScope::AllFloors => ListScope::AllFloors,
            MaterialsScope::Floor(f) => ListScope::Floor(f),
        },
    )
}

// ------------------------------------------------------------------- cells --

fn fmt_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        format!("{q:.2}")
    }
}

fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        String::new()
    } else {
        fmt_qty(v)
    }
}

/// How units are written on export (p. 1386).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitsMode {
    /// In the Count cell, next to the amount.
    #[default]
    WithAmounts,
    /// In a column of their own after Count.
    NewColumn,
    /// Not at all.
    None,
}

/// The text of a cell of the window.
pub fn cell(l: &ListLine, col: MlColumn) -> String {
    cell_units(l, col, UnitsMode::WithAmounts)
}

/// The text of a cell with the units written as `units` says.
pub fn cell_units(l: &ListLine, col: MlColumn, units: UnitsMode) -> String {
    use super::fmt_money;
    match col {
        MlColumn::Id => l.line.id.clone(),
        MlColumn::Use | MlColumn::Quantity | MlColumn::Default => String::new(),
        MlColumn::SubCategory => l.sub_category.clone(),
        MlColumn::Floor => l.floor.clone(),
        MlColumn::Label => l.label.clone(),
        MlColumn::Supplier => l.supplier.clone(),
        MlColumn::Manufacturer => l.manufacturer.clone(),
        MlColumn::Code => l.code.clone(),
        MlColumn::Size => l.line.size.clone(),
        MlColumn::Description => l.line.item.clone(),
        MlColumn::Count => {
            let n = fmt_qty(l.line.quantity);
            if units == UnitsMode::WithAmounts && l.line.unit != "ea" && !l.line.unit.is_empty() {
                format!("{n} {}", l.line.unit)
            } else {
                n
            }
        }
        MlColumn::Extra => {
            let n = fmt_num(l.extra);
            if units == UnitsMode::WithAmounts
                && !n.is_empty()
                && l.line.unit != "ea"
                && !l.line.unit.is_empty()
            {
                format!("{n} {}", l.line.unit)
            } else {
                n
            }
        }
        MlColumn::Price => fmt_money(l.line.unit_price),
        MlColumn::Markup => fmt_num(l.markup),
        MlColumn::Labor => fmt_money(Some(l.labor).filter(|v| *v != 0.0)),
        MlColumn::Equipment => fmt_money(Some(l.equipment).filter(|v| *v != 0.0)),
        MlColumn::TotalCost => fmt_money(l.line.price),
        MlColumn::Comment => l.comment.clone(),
        MlColumn::AccountingCode => l.accounting_code.clone(),
    }
}

/// The unit shown for the Count cell's tool tip: the value with the precision
/// the rounding hid (p. 1380).
pub fn count_tooltip(l: &ListLine) -> String {
    format!(
        "{:.4} {} before waste ({:.0}% waste, rounded up to {})",
        l.line.net,
        l.line.unit,
        l.line.waste_pct,
        fmt_qty(l.line.quantity)
    )
}

// ----------------------------------------------------------------- arranging --

/// A row of the arranged list.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// A heading over a group (a category, a floor, a supplier).
    Group(String),
    /// A line, by index into the list.
    Line(usize),
    /// The subtotal under a group.
    Subtotal(String, Option<f64>),
    /// The total at the foot.
    GrandTotal(Option<f64>),
}

fn group_of(l: &ListLine, by: GroupBy) -> String {
    match by {
        GroupBy::Category => l.line.category.clone(),
        GroupBy::Floor => l.floor.clone(),
        GroupBy::Supplier => {
            if l.supplier.is_empty() {
                "No Supplier".into()
            } else {
                l.supplier.clone()
            }
        }
        GroupBy::None => String::new(),
    }
}

/// Orders the list as the Report tab says: grouped, sorted inside the groups,
/// with the subtotals and the grand total. The Categories panel's display
/// filter applies unless `for_export` (hidden categories stay in exports,
/// p. 1373).
pub fn arrange(lines: &[ListLine], spec: &ListSpec, for_export: bool) -> Vec<Row> {
    let shown: Vec<usize> = (0..lines.len())
        .filter(|i| for_export || spec.shows(&lines[*i].line.category))
        .collect();
    let by = spec.report.group_by;
    let mut keys: Vec<String> = shown.iter().map(|i| group_of(&lines[*i], by)).collect();
    keys.sort();
    keys.dedup();
    if by == GroupBy::Category {
        keys.sort_by_key(|k| category_order(k));
    }
    let mut rows = Vec::new();
    let mut grand = 0.0;
    let mut any = false;
    for g in &keys {
        let mut members: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|i| group_of(&lines[*i], by) == *g)
            .collect();
        sort_members(
            &mut members,
            lines,
            spec.report.sort,
            spec.report.descending,
        );
        if by != GroupBy::None {
            rows.push(Row::Group(g.clone()));
        }
        let mut sub = 0.0;
        let mut sub_any = false;
        for i in members {
            if let Some(t) = lines[i].line.price {
                sub += t;
                sub_any = true;
            }
            rows.push(Row::Line(i));
        }
        if sub_any {
            grand += sub;
            any = true;
        }
        if by != GroupBy::None && spec.report.subtotals {
            rows.push(Row::Subtotal(g.clone(), sub_any.then_some(sub)));
        }
    }
    if spec.report.grand_total && !shown.is_empty() {
        rows.push(Row::GrandTotal(any.then_some(grand)));
    }
    rows
}

fn sort_members(members: &mut [usize], lines: &[ListLine], key: SortKey, desc: bool) {
    members.sort_by(|a, b| {
        let (x, y) = (&lines[*a], &lines[*b]);
        let ord = match key {
            SortKey::Id => a.cmp(b),
            SortKey::Description => x.line.item.to_lowercase().cmp(&y.line.item.to_lowercase()),
            SortKey::Size => x.line.size.to_lowercase().cmp(&y.line.size.to_lowercase()),
            SortKey::Count => x
                .line
                .quantity
                .partial_cmp(&y.line.quantity)
                .unwrap_or(std::cmp::Ordering::Equal),
            SortKey::TotalCost => x
                .line
                .price
                .unwrap_or(0.0)
                .partial_cmp(&y.line.price.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
}

// ----------------------------------------------------------- expand, details --

/// Expand: the objects behind line `index`, one row each (a line with several
/// sources has an arrow in Chief, p. 1379). Each row carries the object's
/// share of the count.
pub fn expand(project: &Project, lines: &[ListLine], index: usize) -> Vec<ListLine> {
    let Some(parent) = lines.get(index) else {
        return Vec::new();
    };
    if parent.sources.len() < 2 {
        return Vec::new();
    }
    let scale = unit_scale(parent.rule);
    parent
        .sources
        .iter()
        .map(|s| {
            let mut child = parent.clone();
            let share = s.qty / scale;
            child.sources = vec![s.clone()];
            child.line.net = share;
            // Shares are shown as they are; the rounding is on the whole row.
            child.line.quantity = (share * 100.0).round() / 100.0;
            child.line.waste_pct = 0.0;
            child.floor = floor_names(project, std::slice::from_ref(s));
            child.depth = 1;
            child.parent = Some(index);
            child.refresh_total();
            child
        })
        .collect()
}

/// A readable name for an object key: `wall:12` is "Wall 12".
pub fn describe_key(key: &str) -> String {
    let mut parts = key.split(':');
    let kind = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    let name = match kind {
        "wall" => "Wall",
        "door" => "Door",
        "window" => "Window",
        "cabinet" => "Cabinet",
        "symbol" => "Symbol",
        "stair" => "Stairs",
        "roof" => "Roof plane",
        "framing" => "Framing member",
        "device" => "Electrical device",
        "room" => "Room",
        "foundation" => "Foundation object",
        "plant" => "Plant",
        "terrain" => "Terrain",
        "line" => return format!("List line {}", rest.join(":")),
        other => other,
    };
    match kind {
        // Device and room keys carry the floor first.
        "device" | "room" if rest.len() == 2 => format!("{name} {}", rest[1]),
        _ if rest.is_empty() => name.to_string(),
        _ => format!("{name} {}", rest.join(":")),
    }
}

/// The Details dialog: every column of the row, then its Source Objects.
pub fn details(l: &ListLine) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = MlColumn::ALL
        .iter()
        .filter(|c| c.in_materials_list())
        .map(|c| (c.title().to_string(), cell(l, *c)))
        .collect();
    v.push(("Category".into(), l.line.category.clone()));
    v.push(("Unit".into(), l.line.unit.clone()));
    for s in &l.sources {
        v.push(("Source Object".into(), describe_key(&s.key)));
    }
    v
}

/// The `(floor, key)` pairs behind a line (Find Object in Plan).
pub fn find_targets(l: &ListLine) -> Vec<(usize, String)> {
    l.sources
        .iter()
        .filter(|s| s.key != "terrain")
        .map(|s| (s.floor, s.key.clone()))
        .collect()
}

// ------------------------------------------------------------- editing cells --

fn parse_money(text: &str) -> Option<Option<f64>> {
    let t = text.trim().trim_start_matches('$').replace(',', "");
    if t.is_empty() {
        return Some(None);
    }
    t.trim_end_matches('%').parse::<f64>().ok().map(Some)
}

/// Types `text` into a cell of a live list: the information goes to the
/// objects behind the row (or to the line itself when it has none), so the
/// next calculation shows it. An empty string restores the automatic value.
/// Returns false when the column cannot be edited or the text is not valid.
pub fn edit_cell(project: &mut Project, l: &ListLine, col: MlColumn, text: &str) -> bool {
    if !col.editable() && col != MlColumn::Id {
        return false;
    }
    if col == MlColumn::Count {
        return false;
    }
    let keys: Vec<String> = if l.sources.is_empty() {
        vec![format!("line:{}", l.line.key)]
    } else {
        l.sources.iter().map(|s| s.key.clone()).collect()
    };
    // A number column: validate once.
    let number = match col {
        MlColumn::Price
        | MlColumn::Markup
        | MlColumn::Labor
        | MlColumn::Equipment
        | MlColumn::Extra => match parse_money(text) {
            Some(n) => Some(n),
            None => return false,
        },
        _ => None,
    };
    if col == MlColumn::Id && !text.trim().is_empty() && !CATEGORIES.contains(&text.trim()) {
        return false;
    }
    let comp_key = l.line.key.clone();
    for key in keys {
        let info = project.materials.info_mut(&key);
        match col {
            MlColumn::Id => info.category = text.trim().to_string(),
            MlColumn::SubCategory => info.sub_category = text.to_string(),
            MlColumn::Label => info.label = text.to_string(),
            MlColumn::Supplier => info.supplier = text.to_string(),
            MlColumn::Manufacturer => info.manufacturer = text.to_string(),
            MlColumn::Code => info.code = text.to_string(),
            MlColumn::Size => info.size = text.to_string(),
            MlColumn::Description => info.description = text.to_string(),
            MlColumn::Comment => info.comment = text.to_string(),
            MlColumn::AccountingCode => info.accounting_code = text.to_string(),
            MlColumn::Price => info.component_mut(&comp_key).price = number.flatten(),
            MlColumn::Markup => info.component_mut(&comp_key).markup = number.flatten(),
            MlColumn::Labor => info.component_mut(&comp_key).labor = number.flatten(),
            MlColumn::Equipment => info.component_mut(&comp_key).equipment = number.flatten(),
            MlColumn::Extra => info.component_mut(&comp_key).extra = number.flatten(),
            _ => {}
        }
    }
    project.materials.prune();
    true
}

// ------------------------------------------------------------ master list ops --

/// A master list entry made from a row (Update to Master List).
pub fn item_of_line(l: &ListLine) -> MasterItem {
    MasterItem {
        key: l.line.key.clone(),
        unit: l.line.unit.clone(),
        unit_price: l.line.unit_price.unwrap_or(0.0),
        supplier: l.supplier.clone(),
        category: l.line.category.clone(),
        size: l.line.size.clone(),
        description: l.line.item.clone(),
        label: l.label.clone(),
        manufacturer: l.manufacturer.clone(),
        code: l.code.clone(),
        comment: l.comment.clone(),
        accounting_code: l.accounting_code.clone(),
        markup: l.markup,
        labor: l.labor,
        equipment: l.equipment,
        ..MasterItem::blank()
    }
}

/// Update to Master List for the rows `rows`: saves what the rows carry.
/// Returns how many entries were made or changed.
pub fn update_to_master(master: &mut MasterList, lines: &[ListLine], rows: &[usize]) -> usize {
    let mut n = 0;
    for &i in rows {
        if let Some(l) = lines.get(i) {
            master.update_to(item_of_line(l));
            n += 1;
        }
    }
    n
}

// -------------------------------------------------------------------- reports --

/// Generate a Report: the rows of a list frozen, no longer linked to the
/// model (p. 1369).
pub fn freeze(lines: &[ListLine]) -> Vec<ReportRow> {
    lines
        .iter()
        .map(|l| ReportRow {
            category: l.line.category.clone(),
            id: l.line.id.clone(),
            sub_category: l.sub_category.clone(),
            floor: l.floor.clone(),
            label: l.label.clone(),
            supplier: l.supplier.clone(),
            manufacturer: l.manufacturer.clone(),
            code: l.code.clone(),
            size: l.line.size.clone(),
            description: l.line.item.clone(),
            count: l.line.quantity,
            unit: l.line.unit.clone(),
            extra: l.extra,
            price: l.line.unit_price,
            markup: l.markup,
            labor: l.labor,
            equipment: l.equipment,
            comment: l.comment.clone(),
            accounting_code: l.accounting_code.clone(),
            key: l.line.key.clone(),
        })
        .collect()
}

/// The rows of a Report as list lines (no sources; totals from the cells).
pub fn thaw(rows: &[ReportRow]) -> Vec<ListLine> {
    rows.iter()
        .map(|r| {
            let mut line = MaterialLine {
                category: r.category.clone(),
                item: r.description.clone(),
                quantity: r.count,
                unit: r.unit.clone(),
                id: r.id.clone(),
                size: r.size.clone(),
                net: r.count,
                waste_pct: 0.0,
                unit_price: r.price,
                price: None,
                key: r.key.clone(),
            };
            line.price = None;
            let mut l = ListLine {
                line,
                sub_category: r.sub_category.clone(),
                floor: r.floor.clone(),
                label: r.label.clone(),
                supplier: r.supplier.clone(),
                manufacturer: r.manufacturer.clone(),
                code: r.code.clone(),
                comment: r.comment.clone(),
                accounting_code: r.accounting_code.clone(),
                extra: r.extra,
                markup: r.markup,
                labor: r.labor,
                equipment: r.equipment,
                sources: Vec::new(),
                rule: Rule::Sum,
                depth: 0,
                parent: None,
            };
            l.refresh_total();
            l
        })
        .collect()
}

/// Types into a cell of a Report row (the changes stay in the Report, not in
/// the plan). Same columns and rules as [`edit_cell`].
pub fn edit_report_cell(row: &mut ReportRow, col: MlColumn, text: &str) -> bool {
    let number = match col {
        MlColumn::Price
        | MlColumn::Markup
        | MlColumn::Labor
        | MlColumn::Equipment
        | MlColumn::Extra
        | MlColumn::Count => match parse_money(text) {
            Some(n) => Some(n),
            None => return false,
        },
        _ => None,
    };
    match col {
        MlColumn::Id => {
            let t = text.trim();
            if !CATEGORIES.contains(&t) {
                return false;
            }
            row.category = t.to_string();
        }
        MlColumn::SubCategory => row.sub_category = text.to_string(),
        MlColumn::Label => row.label = text.to_string(),
        MlColumn::Supplier => row.supplier = text.to_string(),
        MlColumn::Manufacturer => row.manufacturer = text.to_string(),
        MlColumn::Code => row.code = text.to_string(),
        MlColumn::Size => row.size = text.to_string(),
        MlColumn::Description => row.description = text.to_string(),
        MlColumn::Comment => row.comment = text.to_string(),
        MlColumn::AccountingCode => row.accounting_code = text.to_string(),
        MlColumn::Price => row.price = number.flatten(),
        MlColumn::Markup => row.markup = number.flatten().unwrap_or(0.0),
        MlColumn::Labor => row.labor = number.flatten().unwrap_or(0.0),
        MlColumn::Equipment => row.equipment = number.flatten().unwrap_or(0.0),
        MlColumn::Extra => row.extra = number.flatten().unwrap_or(0.0),
        MlColumn::Count => row.count = number.flatten().unwrap_or(0.0),
        _ => return false,
    }
    true
}

/// Update from Master List on a Report: fills the cells a row left empty
/// from the entry that applies (Default first, else the last entered).
/// Returns how many rows found an entry.
pub fn report_update_from_master(rows: &mut [ReportRow], master: &MasterList) -> usize {
    let mut n = 0;
    for r in rows {
        let facts = ItemFacts {
            category: &r.category,
            size: &r.size,
            description: &r.description,
            label: &r.label,
            code: &r.code,
            key: &r.key,
            quantity: r.count,
        };
        let Some(it) = master.find_for(&facts).cloned() else {
            continue;
        };
        n += 1;
        let fill = |dst: &mut String, v: &str| {
            if dst.is_empty() {
                *dst = v.to_string();
            }
        };
        fill(&mut r.supplier, &it.supplier);
        fill(&mut r.manufacturer, &it.manufacturer);
        fill(&mut r.code, &it.code);
        fill(&mut r.comment, &it.comment);
        fill(&mut r.accounting_code, &it.accounting_code);
        if r.price.is_none() && it.unit_price > 0.0 {
            r.price = Some(it.unit_price);
        }
        if r.markup == 0.0 {
            r.markup = it.markup;
        }
        if r.labor == 0.0 {
            r.labor = it.labor;
        }
        if r.equipment == 0.0 {
            r.equipment = it.equipment;
        }
    }
    n
}

/// A selection of objects as a scope.
pub fn selection_scope(items: Vec<(usize, String)>) -> ListScope {
    ListScope::Selection(
        items
            .into_iter()
            .map(|(floor, key)| ObjectAddr { floor, key })
            .collect(),
    )
}

// ------------------------------------------------------------------ macros --

/// Expands the text macros (`%plan.name%`, `%floor%`, user macros) in a cell
/// text typed into the Object Information or a list. `%room.*%` macros need
/// a room under the text and stay empty here.
pub fn expand_text(project: &Project, floor: usize, text: &str) -> String {
    use plan_core::text_styles::{expand_macros, has_macros, MacroContext};
    if !text.contains('%') || !has_macros(text, &project.text_macros) {
        return text.to_string();
    }
    let f = project.floors.get(floor);
    let ctx = MacroContext {
        plan_name: project.name.clone(),
        plan_date: plan_core::text_styles::date_string(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
        ),
        floor_name: f.map_or(String::new(), |f| f.name.clone()),
        floor_number: floor + 1,
        floor_count: project.floors.len(),
        ..MacroContext::default()
    };
    expand_macros(text, &ctx, &project.text_macros)
}

// --------------------------------------------------------- object components --

/// One line item of an object as the Components panel shows it before the
/// object's own changes: what the take-off makes of the object alone.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseComponent {
    /// The line key (`Framing|2x4 stud`) the changes are filed under.
    pub key: String,
    pub category: String,
    pub item: String,
    pub size: String,
    pub unit: String,
    /// The object's share of the count, to hundredths.
    pub count: f64,
    /// The master list's price and costs for it.
    pub price: Option<f64>,
    pub markup: f64,
    pub labor: f64,
    pub equipment: f64,
}

/// The components of object `key` on `floor`: the lines the take-off makes of
/// it alone, with the master list's prices.
pub fn object_components(c: &Calc, floor: usize, key: &str) -> Vec<BaseComponent> {
    let set: BTreeSet<(usize, String)> = [(floor, key.to_string())].into_iter().collect();
    let opts = Options {
        filter: Filter::Selection(set),
        framing: plan_core::materials_data::FramingStyle::BuyList,
        site_everywhere: false,
    };
    engine::take_off_raw(c.project, &[floor], c.active_rooms, c.master, &opts)
        .into_iter()
        .map(|r| {
            let scale = unit_scale(r.rule);
            let share: f64 = r
                .sources
                .iter()
                .filter(|s| s.key == key)
                .map(|s| s.qty)
                .sum::<f64>()
                / scale;
            let facts = ItemFacts {
                category: &r.line.category,
                size: &r.line.size,
                description: &r.line.item,
                label: "",
                code: "",
                key: &r.line.key,
                quantity: share,
            };
            let item = c.master.find_for(&facts);
            BaseComponent {
                key: r.line.key.clone(),
                category: r.line.category.clone(),
                item: r.line.item.clone(),
                size: r.line.size.clone(),
                unit: r.line.unit.clone(),
                count: (share * 100.0).round() / 100.0,
                price: item.map(|i| i.unit_price).filter(|p| *p > 0.0),
                markup: item.map_or(0.0, |i| i.markup),
                labor: item.map_or(0.0, |i| i.labor),
                equipment: item.map_or(0.0, |i| i.equipment),
            }
        })
        .collect()
}
