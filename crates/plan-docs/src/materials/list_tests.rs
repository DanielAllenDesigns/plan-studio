//! Tests of the Materials List engine, the 21 columns, scopes, report
//! options, per-object information, the master list and the exports.

use super::engine::{self, Filter, Options};
use super::export::{self, ExportFormat, ExportOptions, ThirdParty};
use super::list::{self, Calc, ListLine, Row, UnitsMode};
use super::*;
use crate::test_support::{house, rect_walls};
use plan_core::cad::{CadItem, CadObject};
use plan_core::materials_data::{
    FramingStyle, GroupBy, IncludedObjects, ListScope, ListSpec, MaterialsPolyline, MlColumn,
    ObjectAddr, PolylineSpec, SortKey, SupplierFilter,
};
use plan_core::{OpeningKind, Point, Project, WallKind};

fn calc<'a>(p: &'a Project, m: &'a MasterList) -> Calc<'a> {
    Calc {
        project: p,
        master: m,
        active_rooms: None,
    }
}

fn find<'a>(lines: &'a [ListLine], item: &str) -> &'a ListLine {
    lines
        .iter()
        .find(|l| l.line.item == item)
        .unwrap_or_else(|| {
            panic!(
                "no row {item}: {:?}",
                lines.iter().map(|l| &l.line.item).collect::<Vec<_>>()
            )
        })
}

fn plain() -> MasterList {
    MasterList::without_waste()
}

fn all(p: &Project) -> Vec<ListLine> {
    list::calculate(
        &calc(p, &plain()),
        &ListSpec::new("L", ListScope::AllFloors),
    )
    .lines
}

const STUD: &str = "2x6 Stud @ 16\" o.c.";

#[test]
fn the_default_list_equals_the_plain_report() {
    let p = house();
    let old = materials_report(&p, MaterialsScope::AllFloors, None, &plain());
    let new = all(&p);
    assert_eq!(old.len(), new.len());
    for (a, b) in old.iter().zip(new.iter()) {
        assert_eq!(
            (&a.id, &a.item, a.quantity),
            (&b.line.id, &b.line.item, b.line.quantity)
        );
    }
}

#[test]
fn rows_remember_the_objects_behind_them() {
    let p = house();
    let lines = all(&p);
    let studs = find(&lines, STUD);
    assert_eq!(studs.sources.len(), 4, "one source per wall");
    assert!(studs.sources.iter().all(|s| s.key.starts_with("wall:")));
    let sum: f64 = studs.sources.iter().map(|s| s.qty).sum();
    assert!((sum - studs.line.net).abs() < 1e-9);
    let win = lines.iter().find(|l| l.line.category == "Windows").unwrap();
    assert!(win.sources[0].key.starts_with("window:"));
    assert_eq!(list::find_targets(win).len(), win.sources.len());
}

#[test]
fn selection_counts_only_the_selected_objects() {
    let p = house();
    let wall = p.floors[0].walls[0].id;
    let door = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Door)
        .unwrap()
        .id;
    let scope = list::selection_scope(vec![
        (0, format!("wall:{wall}")),
        (0, format!("door:{door}")),
    ]);
    let lines = list::calculate(&calc(&p, &plain()), &ListSpec::new("S", scope)).lines;
    let w = &p.floors[0].walls[0];
    let openings = p.floors[0].openings_on(w.id).count() as f64;
    let want = (w.length() / 16.0).ceil() + 1.0 + 4.0 * openings;
    assert_eq!(find(&lines, STUD).line.quantity, want);
    assert!(lines.iter().any(|l| l.line.category == "Doors"));
    // The windows were not selected, and no other wall counted.
    assert!(!lines.iter().any(|l| l.line.category == "Windows"));
    assert!(!lines.iter().any(|l| l.line.category == "Roofing"));
}

fn polyline_project(mode: IncludedObjects, x1: f64) -> (Project, plan_core::Id) {
    let mut p = house();
    let id = p.alloc_id();
    p.floors[0].cad.push(CadObject {
        id,
        layer: "CAD, Default".into(),
        item: CadItem::Polyline {
            points: vec![
                Point::new(-20.0, -20.0),
                Point::new(x1, -20.0),
                Point::new(x1, 400.0),
                Point::new(-20.0, 400.0),
            ],
            closed: true,
        },
    });
    let mut spec = PolylineSpec::everything(1);
    spec.objects = mode;
    p.materials.polylines.push(MaterialsPolyline {
        cad_id: id,
        floor: 0,
        name: "Area".into(),
        spec,
        holes: Vec::new(),
    });
    (p, id)
}

fn area_lines(p: &Project, id: plan_core::Id) -> Vec<ListLine> {
    list::calculate(
        &calc(p, &plain()),
        &ListSpec::new("A", ListScope::Polyline(id)),
    )
    .lines
}

#[test]
fn a_polyline_counts_objects_by_center_contained_or_intersected() {
    // The house is 480 x 360. The polyline covers x -20..250: the left wall
    // and the halves of the top and bottom walls.
    let (p, id) = polyline_project(IncludedObjects::ByCenter, 250.0);
    let studs = |lines: &[ListLine]| {
        lines
            .iter()
            .find(|l| l.line.item == STUD)
            .map_or(0, |l| l.sources.len())
    };
    // Centres: left wall (0,180) inside; bottom (240,0) and top (240,360)
    // inside; right wall (480,180) outside.
    assert_eq!(studs(&area_lines(&p, id)), 3);
    // Wholly contained: the long walls stick out past x = 250.
    let (p2, id2) = polyline_project(IncludedObjects::Contained, 250.0);
    assert_eq!(studs(&area_lines(&p2, id2)), 1);
    // Intersected: the right wall (x 477..483) is clear, the others touch.
    let (p3, id3) = polyline_project(IncludedObjects::Intersected, 250.0);
    assert_eq!(studs(&area_lines(&p3, id3)), 3);
    // Reach the right wall and it joins.
    let (p4, id4) = polyline_project(IncludedObjects::Intersected, 479.0);
    assert_eq!(studs(&area_lines(&p4, id4)), 4);
}

#[test]
fn a_hole_keeps_objects_out_and_a_deleted_polyline_says_so() {
    let (mut p, id) = polyline_project(IncludedObjects::ByCenter, 600.0);
    let all_studs = area_lines(&p, id)
        .iter()
        .find(|l| l.line.item == STUD)
        .unwrap()
        .sources
        .len();
    assert_eq!(all_studs, 4);
    // Punch a hole over the left wall's centre.
    p.materials.polyline_mut(id).unwrap().holes = vec![vec![
        Point::new(-10.0, 150.0),
        Point::new(10.0, 150.0),
        Point::new(10.0, 210.0),
        Point::new(-10.0, 210.0),
    ]];
    let n = area_lines(&p, id)
        .iter()
        .find(|l| l.line.item == STUD)
        .unwrap()
        .sources
        .len();
    assert_eq!(n, 3);
    p.floors[0].cad.clear();
    let out = list::calculate(
        &calc(&p, &plain()),
        &ListSpec::new("A", ListScope::Polyline(id)),
    );
    assert!(out.lines.is_empty());
    assert!(out.note.unwrap().contains("Polyline"));
}

#[test]
fn the_polyline_grid_turns_a_category_off_on_a_floor() {
    let (mut p, id) = polyline_project(IncludedObjects::ByCenter, 600.0);
    assert!(area_lines(&p, id)
        .iter()
        .any(|l| l.line.category == "Windows"));
    p.materials
        .polyline_mut(id)
        .unwrap()
        .spec
        .toggle("Windows", 0);
    let lines = area_lines(&p, id);
    assert!(!lines.iter().any(|l| l.line.category == "Windows"));
    assert!(lines.iter().any(|l| l.line.category == "Doors"));
}

#[test]
fn a_room_list_has_its_contents_finishes_and_facing_walls() {
    let p = house();
    let rooms = plan_core::detect_rooms(&p.floors[0].walls, 1.0);
    let c = rooms[0].centroid;
    let spec = ListSpec::new(
        "R",
        ListScope::Room {
            floor: 0,
            x: c.x,
            y: c.y,
        },
    );
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    // Its floor and ceiling, the wall surface facing in, the door and windows.
    assert!(lines.iter().any(|l| l.line.item.starts_with("Flooring")));
    assert!(lines
        .iter()
        .any(|l| l.line.item.starts_with("Ceiling drywall")));
    let drywall = find(&lines, "Wall drywall 1/2\" 4x8 sheet");
    // Inner perimeter x 109 1/8" less the openings, over 32 sq ft sheets.
    assert!(
        drywall.line.quantity >= 36.0 && drywall.line.quantity <= 40.0,
        "{}",
        drywall.line.quantity
    );
    assert!(lines.iter().any(|l| l.line.category == "Doors"));
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.line.category == "Windows")
            .map(|l| l.line.quantity)
            .sum::<f64>(),
        2.0
    );
    // No framing: the walls around the room are not counted.
    assert!(!lines.iter().any(|l| l.line.category == "Framing"));
    assert!(!lines.iter().any(|l| l.line.category == "Siding"));
}

#[test]
fn a_selected_room_counts_its_finishes_but_not_doors_or_framing() {
    let p = house();
    let rooms = plan_core::detect_rooms(&p.floors[0].walls, 1.0);
    let key = engine::room_key(0, &rooms[0]);
    let lines = list::calculate(
        &calc(&p, &plain()),
        &ListSpec::new("S", list::selection_scope(vec![(0, key)])),
    )
    .lines;
    assert!(lines.iter().any(|l| l.line.item.starts_with("Flooring")));
    assert!(lines
        .iter()
        .any(|l| l.line.item == "Wall drywall 1/2\" 4x8 sheet"));
    assert!(!lines.iter().any(|l| l.line.category == "Doors"));
    assert!(!lines.iter().any(|l| l.line.category == "Framing"));
}

#[test]
fn the_total_cost_formula_uses_extra_markup_labor_and_equipment() {
    // (Count + Extra) * Price * (1 + Markup/100) + (Count + Extra) * Labor + (Count + Extra) * Equipment
    let t = list::total_cost(10.0, 2.0, Some(5.0), 20.0, 1.5, 0.5).unwrap();
    assert!((t - (12.0 * 5.0 * 1.2 + 12.0 * 1.5 + 12.0 * 0.5)).abs() < 1e-9);
    // Labor alone prices a row; with nothing set the row is unpriced.
    assert_eq!(list::total_cost(3.0, 0.0, None, 0.0, 2.0, 0.0), Some(6.0));
    assert_eq!(list::total_cost(3.0, 0.0, None, 0.0, 0.0, 0.0), None);
    // A list carries it through the cells.
    let mut p = one_wall();
    let key = "Framing|2x4 stud";
    let mut info = plan_core::materials_data::ObjectInfo::default();
    {
        let c = info.component_mut(key);
        c.price = Some(3.0);
        c.markup = Some(10.0);
        c.labor = Some(1.0);
        c.equipment = Some(0.25);
        c.extra = Some(1.0);
    }
    let wall = p.floors[0].walls[0].id;
    p.materials.objects.insert(format!("wall:{wall}"), info);
    let lines = all(&p);
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    // 9 studs + 1 extra.
    let want = 10.0 * 3.0 * 1.1 + 10.0 * 1.0 + 10.0 * 0.25;
    assert!((s.line.price.unwrap() - want).abs() < 1e-9);
    assert_eq!(list::cell(s, MlColumn::Extra), "1");
    assert_eq!(list::cell(s, MlColumn::Markup), "10");
    assert_eq!(
        list::cell(s, MlColumn::TotalCost),
        super::fmt_money(Some(want))
    );
    assert_eq!(list::cell(s, MlColumn::Price), "$3.00");
    assert_eq!(list::cell(s, MlColumn::Floor), "1st Floor");
}

fn one_wall() -> Project {
    let mut p = Project::new("w");
    p.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(120.0, 0.0),
        4.5,
        96.0,
        WallKind::Interior,
    );
    p
}

#[test]
fn object_information_splits_rows_and_shows_in_the_columns() {
    let p0 = house();
    let (a, b) = (p0.floors[0].walls[0].id, p0.floors[0].walls[1].id);
    let mut p = p0;
    {
        let i = p.materials.info_mut(&format!("wall:{a}"));
        i.supplier = "Acme Lumber".into();
        i.code = "SKU-1".into();
        i.manufacturer = "Weyerhaeuser".into();
        i.comment = "Delivery Monday".into();
        i.sub_category = "Exterior".into();
    }
    let lines = all(&p);
    let studs: Vec<&ListLine> = lines.iter().filter(|l| l.line.item == STUD).collect();
    // The first wall's studs have a row of their own.
    assert_eq!(studs.len(), 2);
    let acme = studs.iter().find(|l| l.supplier == "Acme Lumber").unwrap();
    assert_eq!(acme.sources.len(), 1);
    assert_eq!(acme.code, "SKU-1");
    assert_eq!(acme.manufacturer, "Weyerhaeuser");
    assert_eq!(acme.comment, "Delivery Monday");
    assert_eq!(acme.sub_category, "Exterior");
    let rest = studs.iter().find(|l| l.supplier.is_empty()).unwrap();
    assert_eq!(rest.sources.len(), 3);
    assert!(!rest
        .sources
        .iter()
        .any(|s| s.key.ends_with(&format!(":{a}"))));
    let _ = b;
    // Restrict to Supplier.
    let mut spec = ListSpec::new("L", ListScope::AllFloors);
    spec.supplier = SupplierFilter::Only("Acme Lumber".into());
    let only = list::calculate(&calc(&p, &plain()), &spec).lines;
    assert!(only.iter().all(|l| l.supplier == "Acme Lumber"));
    assert!(!only.is_empty());
    spec.supplier = SupplierFilter::NoSupplier;
    let none = list::calculate(&calc(&p, &plain()), &spec).lines;
    assert!(none.iter().all(|l| l.supplier.is_empty()));
    assert!(none.len() < lines.len() + 1);
}

#[test]
fn a_component_can_be_removed_or_given_another_count() {
    let mut p = one_wall();
    let wall = p.floors[0].walls[0].id;
    p.materials
        .info_mut(&format!("wall:{wall}"))
        .component_mut("Framing|2x4 plate")
        .removed = true;
    p.materials
        .info_mut(&format!("wall:{wall}"))
        .component_mut("Framing|2x4 stud")
        .count = Some(20.0);
    let lines = all(&p);
    assert!(!lines.iter().any(|l| l.line.item.starts_with("2x4 Plate")));
    assert_eq!(find(&lines, "2x4 Stud @ 16\" o.c.").line.quantity, 20.0);
}

#[test]
fn added_components_join_the_list_of_their_object() {
    let mut p = one_wall();
    let wall = p.floors[0].walls[0].id;
    p.materials.info_mut(&format!("wall:{wall}")).added.push(
        plan_core::materials_data::ExtraComponent {
            category: "Framing".into(),
            description: "Hurricane tie".into(),
            count: 12.0,
            price: Some(0.8),
            ..Default::default()
        },
    );
    let lines = all(&p);
    let tie = find(&lines, "Hurricane tie");
    assert_eq!(tie.line.quantity, 12.0);
    assert!((tie.line.price.unwrap() - 9.6).abs() < 1e-9);
    // A list that does not hold the wall does not get its accessory.
    let spec = ListSpec::new("L", list::selection_scope(vec![(0, "wall:999".into())]));
    assert!(!list::calculate(&calc(&p, &plain()), &spec)
        .lines
        .iter()
        .any(|l| l.line.item == "Hurricane tie"));
}

#[test]
fn the_master_list_fills_blanks_honors_use_and_the_object_wins() {
    let mut p = one_wall();
    let wall = p.floors[0].walls[0].id;
    let mut m = plain();
    let mut it = MasterItem::blank();
    it.category = "Framing".into();
    it.description = "2x4 Stud @ 16\" o.c.".into();
    it.size = "2x4".into();
    it.unit_price = 4.0;
    it.supplier = "Master Supply".into();
    it.manufacturer = "Maker".into();
    it.markup = 10.0;
    m.items.push(it);
    let lines = list::calculate(&calc(&p, &m), &ListSpec::new("L", ListScope::AllFloors)).lines;
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    assert_eq!(s.line.unit_price, Some(4.0));
    assert_eq!(s.supplier, "Master Supply");
    assert!((s.line.price.unwrap() - 9.0 * 4.0 * 1.1).abs() < 1e-9);
    // The object's own supplier is kept over the master list's.
    p.materials.info_mut(&format!("wall:{wall}")).supplier = "On Site".into();
    let lines = list::calculate(&calc(&p, &m), &ListSpec::new("L", ListScope::AllFloors)).lines;
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    assert_eq!(s.supplier, "On Site");
    assert_eq!(s.manufacturer, "Maker");
    // An item that is not used leaves the list.
    m.items[0].use_item = false;
    let lines = list::calculate(&calc(&p, &m), &ListSpec::new("L", ListScope::AllFloors)).lines;
    assert!(!lines.iter().any(|l| l.line.item == "2x4 Stud @ 16\" o.c."));
}

#[test]
fn older_key_prices_still_apply() {
    let p = one_wall();
    let mut m = plain();
    m.set_price("Framing|2x4 stud", "ea", 3.5);
    let lines = list::calculate(&calc(&p, &m), &ListSpec::new("L", ListScope::AllFloors)).lines;
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    assert_eq!(s.line.price, Some(31.5));
}

#[test]
fn waste_rounds_the_count_up_in_a_list_like_in_the_report() {
    let p = one_wall();
    let lines = list::calculate(
        &calc(&p, &MasterList::default()),
        &ListSpec::new("L", ListScope::AllFloors),
    )
    .lines;
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    assert_eq!((s.line.net, s.line.quantity), (9.0, 10.0));
    assert!(list::count_tooltip(s).contains("10% waste"));
}

#[test]
fn editing_a_cell_writes_the_objects_and_the_next_list_shows_it() {
    let mut p = house();
    let lines = all(&p);
    let win = lines
        .iter()
        .find(|l| l.line.category == "Windows")
        .unwrap()
        .clone();
    assert!(list::edit_cell(&mut p, &win, MlColumn::Supplier, "Pella"));
    assert!(list::edit_cell(&mut p, &win, MlColumn::Price, "$412.50"));
    assert!(list::edit_cell(&mut p, &win, MlColumn::Code, "W-3040"));
    assert!(!list::edit_cell(&mut p, &win, MlColumn::Price, "twelve"));
    assert!(!list::edit_cell(&mut p, &win, MlColumn::Count, "3"));
    let lines = all(&p);
    let w = lines.iter().find(|l| l.line.category == "Windows").unwrap();
    assert_eq!(w.supplier, "Pella");
    assert_eq!(w.code, "W-3040");
    assert_eq!(w.line.unit_price, Some(412.5));
    assert_eq!(w.line.price, Some(412.5 * w.line.quantity));
    // Clearing the cell restores the automatic value and prunes the entry.
    assert!(list::edit_cell(&mut p, w, MlColumn::Supplier, ""));
    assert!(list::edit_cell(&mut p, w, MlColumn::Price, ""));
    assert!(list::edit_cell(&mut p, w, MlColumn::Code, ""));
    assert!(p.materials.objects.is_empty());
    // A row's category can be changed from its ID cell.
    let lines = all(&p);
    let door = lines
        .iter()
        .find(|l| l.line.category == "Doors")
        .unwrap()
        .clone();
    assert!(!list::edit_cell(&mut p, &door, MlColumn::Id, "Plumbing"));
    assert!(list::edit_cell(&mut p, &door, MlColumn::Id, "Fixtures"));
    let moved = all(&p);
    assert!(!moved.iter().any(|l| l.line.category == "Doors"));
    assert!(moved
        .iter()
        .any(|l| l.line.category == "Fixtures" && l.line.item.starts_with("Door")));
}

#[test]
fn a_row_without_objects_keeps_what_is_typed_under_its_line_key() {
    let mut p = house();
    let lines = all(&p);
    let soil_like = lines.iter().find(|l| l.line.item == "Roof area");
    assert!(soil_like.is_none(), "no roof in the house");
    let mut line = lines[0].clone();
    line.sources.clear();
    assert!(list::edit_cell(&mut p, &line, MlColumn::Comment, "Check"));
    assert!(p.materials.objects.keys().any(|k| k.starts_with("line:")));
}

#[test]
fn rows_expand_into_their_objects() {
    let p = house();
    let lines = all(&p);
    let i = lines.iter().position(|l| l.line.item == STUD).unwrap();
    let kids = list::expand(&p, &lines, i);
    assert_eq!(kids.len(), 4);
    assert!(kids.iter().all(|k| k.depth == 1 && k.parent == Some(i)));
    let sum: f64 = kids.iter().map(|k| k.line.quantity).sum();
    assert!((sum - lines[i].line.net).abs() < 1e-6);
    // A single-object row has nothing to expand.
    let j = lines
        .iter()
        .position(|l| l.line.category == "Doors")
        .unwrap();
    assert!(list::expand(&p, &lines, j).is_empty());
    let d = list::details(&lines[i]);
    assert!(d
        .iter()
        .any(|(k, v)| k == "Source Object" && v.starts_with("Wall ")));
    assert!(d.iter().any(|(k, _)| k == "% Markup"));
    assert_eq!(list::describe_key("device:0:4"), "Electrical device 4");
    assert_eq!(list::describe_key("door:7"), "Door 7");
}

#[test]
fn report_groups_sort_and_subtotals() {
    let p = house();
    let mut m = plain();
    m.set_price("Framing|2x6 stud", "ea", 2.0);
    m.set_price("Siding|Siding", "sq ft", 1.0);
    let mut spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &m), &spec).lines;
    let rows = list::arrange(&lines, &spec, false);
    // Group headings in Chief's category order, a subtotal under each, a grand total.
    let groups: Vec<&String> = rows
        .iter()
        .filter_map(|r| if let Row::Group(g) = r { Some(g) } else { None })
        .collect();
    assert_eq!(groups[0], "Framing");
    assert!(
        groups.iter().position(|g| *g == "Siding") > groups.iter().position(|g| *g == "Framing")
    );
    let subs = rows
        .iter()
        .filter(|r| matches!(r, Row::Subtotal(..)))
        .count();
    assert_eq!(subs, groups.len());
    let Some(Row::GrandTotal(Some(t))) = rows.last() else {
        panic!("no grand total: {:?}", rows.last())
    };
    assert!((t - list::total_of(&lines).unwrap()).abs() < 1e-9);
    // No grouping, sorted by description descending, no subtotals.
    spec.report.group_by = GroupBy::None;
    spec.report.sort = SortKey::Description;
    spec.report.descending = true;
    let rows = list::arrange(&lines, &spec, false);
    let items: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            if let Row::Line(i) = r {
                Some(lines[*i].line.item.to_lowercase())
            } else {
                None
            }
        })
        .collect();
    let mut sorted = items.clone();
    sorted.sort();
    sorted.reverse();
    assert_eq!(items, sorted);
    assert!(!rows
        .iter()
        .any(|r| matches!(r, Row::Group(_) | Row::Subtotal(..))));
    // Totals can be turned off.
    spec.report.grand_total = false;
    assert!(!list::arrange(&lines, &spec, false)
        .iter()
        .any(|r| matches!(r, Row::GrandTotal(_))));
}

#[test]
fn hidden_categories_leave_the_view_but_stay_in_exports() {
    let p = house();
    let mut spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    spec.show_all_categories(false);
    spec.set_category_shown("Doors", true);
    let view = list::arrange(&lines, &spec, false);
    assert!(view
        .iter()
        .filter_map(|r| if let Row::Line(i) = r {
            Some(&lines[*i])
        } else {
            None
        })
        .all(|l| l.line.category == "Doors"));
    let exported = list::arrange(&lines, &spec, true);
    assert!(exported
        .iter()
        .filter_map(|r| if let Row::Line(i) = r {
            Some(&lines[*i])
        } else {
            None
        })
        .any(|l| l.line.category == "Framing"));
}

#[test]
fn columns_show_hide_and_reorder_in_the_table() {
    let p = house();
    let mut spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    let t = export::table(&lines, &spec, &ExportOptions::default(), false);
    assert_eq!(
        t.header,
        vec!["ID", "Size", "Description", "Count", "Price", "Total Cost"]
    );
    spec.set_visible(MlColumn::Supplier, true);
    spec.set_visible(MlColumn::Size, false);
    // Description moves past the hidden columns to the front.
    while spec.columns[0].col != MlColumn::Description {
        assert!(spec.move_column(MlColumn::Description, -1));
    }
    let t = export::table(&lines, &spec, &ExportOptions::default(), false);
    assert_eq!(t.header[0], "Description");
    assert!(t.header.contains(&"Supplier".to_string()));
    assert!(!t.header.contains(&"Size".to_string()));
    // Hidden columns on request: all 18 columns of a Materials List.
    let all_cols = ExportOptions {
        hidden_columns: true,
        ..ExportOptions::default()
    };
    assert_eq!(
        export::table(&lines, &spec, &all_cols, false).header.len(),
        18
    );
}

#[test]
fn exports_write_tab_comma_xml_html_and_workbook() {
    let mut p = house();
    p.materials.info_mut("window:1").comment = "R&D <test>".into();
    let mut m = plain();
    m.set_price("Framing|2x6 stud", "ea", 2.0);
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &m), &spec).lines;
    let text =
        |o: ExportOptions| String::from_utf8(export::export(&lines, &spec, &o, "T")).unwrap();
    let base = ExportOptions::default();
    let csv = text(base);
    assert!(csv.starts_with("ID,Size,Description,Count,Price,Total Cost\n"));
    assert!(csv.trim_end().lines().last().unwrap().starts_with("Total,"));
    let no_head = text(ExportOptions {
        headers: false,
        ..base
    });
    assert!(!no_head.starts_with("ID,"));
    let txt = text(ExportOptions {
        format: ExportFormat::Txt,
        ..base
    });
    assert!(txt
        .lines()
        .next()
        .unwrap()
        .contains("ID\tSize\tDescription"));
    assert!(txt.lines().all(|l| l.matches('\t').count() == 5));
    let xml = text(ExportOptions {
        format: ExportFormat::Xml,
        ..base
    });
    assert!(xml.contains("<Worksheet ss:Name=\"Materials List\">"));
    assert!(xml.contains("ss:Type=\"Number\""));
    assert!(xml.contains("urn:schemas-microsoft-com:office:spreadsheet"));
    let html = text(ExportOptions {
        format: ExportFormat::Html,
        headers: false,
        ..base
    });
    // Column headers are always in HTML.
    assert!(html.contains("<th>Description</th>"));
    assert!(html.contains("<tr class=\"group\">"));
    assert!(html.contains("<title>T</title>"));
    let xlsx = export::export(
        &lines,
        &spec,
        &ExportOptions {
            format: ExportFormat::Xlsx,
            ..base
        },
        "T",
    );
    assert_eq!(&xlsx[..2], b"PK");
}

#[test]
fn html_escapes_and_colors_follow_the_choice() {
    let mut p = house();
    let win = all(&p)
        .into_iter()
        .find(|l| l.line.category == "Windows")
        .unwrap();
    list::edit_cell(&mut p, &win, MlColumn::Comment, "R&D <b>");
    let mut spec = ListSpec::new("L", ListScope::AllFloors);
    spec.set_visible(MlColumn::Comment, true);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    let o = ExportOptions {
        format: ExportFormat::Html,
        ..ExportOptions::default()
    };
    let h = String::from_utf8(export::export(&lines, &spec, &o, "T")).unwrap();
    assert!(h.contains("R&amp;D &lt;b&gt;"), "{h}");
    assert!(!h.contains("<b>"));
    // White and pale gray without colors; the list's colors with them.
    assert!(h.contains("#FFFFFF") && h.contains("#F2F2F2"));
    spec.appearance.custom_colors = true;
    spec.appearance.background = [0xFF, 0xEE, 0xDD];
    let c = ExportOptions { colors: true, ..o };
    let h = String::from_utf8(export::export(&lines, &spec, &c, "T")).unwrap();
    assert!(h.contains("#FFEEDD"));
}

#[test]
fn units_go_with_the_amount_in_a_column_or_nowhere() {
    let p = house();
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    let plates = lines
        .iter()
        .find(|l| l.line.item.starts_with("2x6 Plate"))
        .unwrap();
    assert!(list::cell_units(plates, MlColumn::Count, UnitsMode::WithAmounts).ends_with(" lf"));
    assert!(!list::cell_units(plates, MlColumn::Count, UnitsMode::None).contains("lf"));
    let o = ExportOptions {
        units: UnitsMode::NewColumn,
        ..ExportOptions::default()
    };
    let t = export::table(&lines, &spec, &o, false);
    let at = t.header.iter().position(|h| h == "Count").unwrap();
    assert_eq!(t.header[at + 1], "Unit");
    let row = t
        .rows
        .iter()
        .find(|r| r.cells[2].starts_with("2x6 Plate"))
        .unwrap();
    assert_eq!(row.cells[at + 1], "lf");
    assert!(!row.cells[at].contains("lf"));
}

#[test]
fn builder_trend_csv_has_cost_codes_and_unit_costs() {
    let mut p = house();
    let door = all(&p)
        .into_iter()
        .find(|l| l.line.category == "Doors")
        .unwrap();
    list::edit_cell(&mut p, &door, MlColumn::AccountingCode, "0310");
    list::edit_cell(&mut p, &door, MlColumn::Price, "250");
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    let o = ExportOptions {
        third_party: ThirdParty::BuilderTrend,
        ..ExportOptions::default()
    };
    let text = String::from_utf8(export::export(&lines, &spec, &o, "T")).unwrap();
    assert!(text.starts_with("Cost Code,Title,Description,Quantity,Unit Cost,Unit,Markup %\n"));
    assert!(text.contains("0310,"));
    assert!(text.contains(",250.00,"));
}

#[test]
fn print_writes_a_paged_pdf_table() {
    let mut p = Project::new("big");
    rect_walls(&mut p, 480.0, 360.0, 6.5, WallKind::Exterior);
    for i in 0..60 {
        let w = p.floors[0].walls[0].id;
        let _ = p.add_opening(0, w, 20.0 + f64::from(i) * 6.0, OpeningKind::Window);
    }
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &MasterList::default()), &spec).lines;
    let pdf = export::to_pdf(&lines, &spec, "Materials List", "Big plan");
    assert!(pdf.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("Page 1 of"));
    // Wide column sets print on landscape pages.
    let mut wide = spec.clone();
    for c in MlColumn::ALL {
        if c.in_materials_list() {
            wide.set_visible(c, true);
        }
    }
    let wide_pdf = export::to_pdf(&lines, &wide, "Materials List", "Big plan");
    assert!(String::from_utf8_lossy(&wide_pdf).contains("792"));
}

#[test]
fn reports_freeze_edit_and_update_from_the_master_list() {
    let p = house();
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let lines = list::calculate(&calc(&p, &plain()), &spec).lines;
    let mut rows = list::freeze(&lines);
    assert_eq!(rows.len(), lines.len());
    let i = rows.iter().position(|r| r.category == "Doors").unwrap();
    assert!(list::edit_report_cell(
        &mut rows[i],
        MlColumn::Price,
        "$100"
    ));
    assert!(list::edit_report_cell(
        &mut rows[i],
        MlColumn::Markup,
        "10%"
    ));
    assert!(list::edit_report_cell(
        &mut rows[i],
        MlColumn::Supplier,
        "Doors Inc"
    ));
    assert!(!list::edit_report_cell(&mut rows[i], MlColumn::Price, "x"));
    assert!(!list::edit_report_cell(
        &mut rows[i],
        MlColumn::TotalCost,
        "1"
    ));
    let back = list::thaw(&rows);
    assert_eq!(back[i].line.price, Some(100.0 * 1.1 * rows[i].count));
    assert_eq!(back[i].supplier, "Doors Inc");
    // The report no longer follows the plan.
    let mut p2 = p.clone();
    p2.floors[0].openings.clear();
    assert_eq!(list::freeze(&all(&house())).len(), rows.len());
    // Update from Master List fills what the report left blank.
    let mut m = plain();
    let mut it = MasterItem::blank();
    it.category = "Windows".into();
    it.size = rows
        .iter()
        .find(|r| r.category == "Windows")
        .unwrap()
        .size
        .clone();
    it.description = rows
        .iter()
        .find(|r| r.category == "Windows")
        .unwrap()
        .description
        .clone();
    it.unit_price = 300.0;
    it.code = "W-1".into();
    m.items.push(it);
    let n = list::report_update_from_master(&mut rows, &m);
    assert_eq!(n, 1);
    let w = rows.iter().find(|r| r.category == "Windows").unwrap();
    assert_eq!((w.price, w.code.as_str()), (Some(300.0), "W-1"));
}

#[test]
fn update_to_master_list_saves_the_edited_rows() {
    let mut p = house();
    let door = all(&p)
        .into_iter()
        .find(|l| l.line.category == "Doors")
        .unwrap();
    list::edit_cell(&mut p, &door, MlColumn::Price, "180");
    list::edit_cell(&mut p, &door, MlColumn::Supplier, "Doors Inc");
    let lines = all(&p);
    let i = lines
        .iter()
        .position(|l| l.line.category == "Doors")
        .unwrap();
    let mut m = plain();
    assert_eq!(list::update_to_master(&mut m, &lines, &[i]), 1);
    assert_eq!(m.items.len(), 1);
    assert_eq!(m.items[0].unit_price, 180.0);
    // Again: the same entry is updated, not added.
    list::update_to_master(&mut m, &lines, &[i]);
    assert_eq!(m.items.len(), 1);
    // A fresh plan with the same door now picks the price up.
    let fresh = house();
    let lines = list::calculate(&calc(&fresh, &m), &ListSpec::new("L", ListScope::AllFloors)).lines;
    let d = lines.iter().find(|l| l.line.category == "Doors").unwrap();
    assert_eq!(d.line.unit_price, Some(180.0));
    assert_eq!(d.supplier, "Doors Inc");
}

fn framed() -> Project {
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
    p
}

#[test]
fn framing_is_a_buy_list_a_cut_list_or_linear_feet() {
    let p = framed();
    let run = |style: FramingStyle| {
        let mut spec = ListSpec::new("L", ListScope::AllFloors);
        spec.framing = style;
        list::calculate(&calc(&p, &plain()), &spec).lines
    };
    let buy = run(FramingStyle::BuyList);
    assert!(buy.iter().any(|l| l.line.item.contains("' lumber")));
    let cut = run(FramingStyle::CutList);
    assert!(cut.iter().any(|l| l.line.item.contains(" cut to ")));
    assert!(!cut.iter().any(|l| l.line.item.contains("' lumber")));
    let lf = run(FramingStyle::LinearFeet);
    let row = lf
        .iter()
        .find(|l| l.line.unit == "lf" && l.line.item.contains("linear feet"))
        .unwrap();
    assert!(row.line.quantity > 20.0);
    // Board feet stay in all three.
    for l in [&buy, &cut, &lf] {
        assert!(l.iter().any(|x| x.line.unit == "bf"));
    }
    // The member pieces add up the same in a cut list as in a buy list.
    let pieces = |ls: &[ListLine], needle: &str| -> f64 {
        ls.iter()
            .filter(|l| l.line.item.contains(needle) && l.line.unit == "ea")
            .map(|l| l.line.quantity)
            .sum()
    };
    assert!(pieces(&cut, "cut to") > 0.0);
}

#[test]
fn a_framing_selection_follows_the_wall() {
    let p = framed();
    let wall = p.floors[0].walls[0].id;
    let sel = list::calculate(
        &calc(&p, &plain()),
        &ListSpec::new(
            "S",
            list::selection_scope(vec![(0, format!("wall:{wall}"))]),
        ),
    )
    .lines;
    assert!(sel.iter().any(|l| l.line.unit == "bf"));
    let other = list::calculate(
        &calc(&p, &plain()),
        &ListSpec::new("S", list::selection_scope(vec![(0, "wall:424242".into())])),
    )
    .lines;
    assert!(other.is_empty());
}

#[test]
fn page_totals_and_ids_run_by_category() {
    let p = house();
    let lines = all(&p);
    let framing: Vec<&ListLine> = lines
        .iter()
        .filter(|l| l.line.category == "Framing")
        .collect();
    assert_eq!(framing[0].line.id, "FRM-001");
    assert_eq!(framing[1].line.id, "FRM-002");
    assert!(list::total_of(&lines).is_none(), "nothing is priced");
}

#[test]
fn the_engine_with_a_selection_filter_ignores_unlisted_objects() {
    let p = house();
    let opts = Options {
        filter: Filter::Selection([(0usize, "wall:99999".to_string())].into_iter().collect()),
        ..Options::default()
    };
    let raws = engine::take_off_raw(&p, &[0], None, &plain(), &opts);
    assert!(raws.is_empty());
}

#[test]
fn a_saved_scope_with_a_missing_object_still_calculates() {
    let p = house();
    let scope = ListScope::Selection(vec![
        ObjectAddr {
            floor: 0,
            key: "wall:77777".into(),
        },
        ObjectAddr {
            floor: 3,
            key: "wall:1".into(),
        },
    ]);
    let out = list::calculate(&calc(&p, &plain()), &ListSpec::new("S", scope));
    assert!(out.lines.is_empty());
}

#[test]
fn site_lines_follow_all_floors_and_selection_of_the_terrain() {
    let p = house();
    let lines = all(&p);
    // The house has no terrain: no soil or plant rows, and no panic.
    assert!(!lines.iter().any(|l| l.line.category == "Landscaping"));
    let sel = list::calculate(
        &calc(&p, &plain()),
        &ListSpec::new("S", list::selection_scope(vec![(0, "terrain".into())])),
    );
    assert!(sel.lines.is_empty());
}

#[test]
fn an_objects_components_come_from_the_take_off_of_that_object_alone() {
    let p = one_wall();
    let wall = p.floors[0].walls[0].id;
    let mut m = plain();
    let mut it = MasterItem::blank();
    it.category = "Framing".into();
    it.size = "2x4".into();
    it.description = "2x4 Stud @ 16\" o.c.".into();
    it.unit_price = 3.0;
    it.markup = 10.0;
    m.items.push(it);
    let comps = list::object_components(&calc(&p, &m), 0, &format!("wall:{wall}"));
    let stud = comps
        .iter()
        .find(|c| c.item == "2x4 Stud @ 16\" o.c.")
        .expect("studs");
    assert_eq!(stud.count, 9.0);
    assert_eq!(stud.price, Some(3.0));
    assert_eq!(stud.markup, 10.0);
    assert_eq!(stud.key, "Framing|2x4 stud");
    assert!(comps.iter().any(|c| c.item.starts_with("2x4 Plate")));
    // An object that is not there has no components.
    assert!(list::object_components(&calc(&p, &m), 0, "wall:424242").is_empty());
}

#[test]
fn text_macros_in_typed_cells_expand_in_the_list() {
    let mut p = one_wall();
    p.name = "Maple House".into();
    let wall = p.floors[0].walls[0].id;
    p.materials.info_mut(&format!("wall:{wall}")).comment = "For %plan.name% on %floor%".into();
    let lines = all(&p);
    let s = find(&lines, "2x4 Stud @ 16\" o.c.");
    assert_eq!(s.comment, "For Maple House on 1st Floor");
    // Unknown macros stay as typed.
    assert_eq!(
        list::expand_text(&p, 0, "100% sure %nope%"),
        "100% sure %nope%"
    );
}

#[test]
fn every_column_of_a_materials_list_has_its_cell() {
    let mut p = house();
    let door = p.floors[0]
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Door)
        .unwrap()
        .id;
    {
        let i = p.materials.info_mut(&format!("door:{door}"));
        i.sub_category = "Entry".into();
        i.label = "D1".into();
        i.supplier = "Supplier".into();
        i.manufacturer = "Maker".into();
        i.code = "C1".into();
        i.comment = "note".into();
        i.accounting_code = "0310".into();
        i.price = Some(100.0);
        i.extra = Some(1.0);
        i.markup = Some(10.0);
        i.labor = Some(5.0);
        i.equipment = Some(2.0);
    }
    let lines = all(&p);
    let d = lines
        .iter()
        .find(|l| l.line.category == "Doors")
        .expect("a door row");
    let want = [
        (MlColumn::Id, "DR-001"),
        (MlColumn::Use, ""),
        (MlColumn::SubCategory, "Entry"),
        (MlColumn::Floor, "1st Floor"),
        (MlColumn::Label, "D1"),
        (MlColumn::Supplier, "Supplier"),
        (MlColumn::Manufacturer, "Maker"),
        (MlColumn::Code, "C1"),
        (MlColumn::Size, "3'-0\" x 6'-8\""),
        (MlColumn::Quantity, ""),
        (MlColumn::Count, "1"),
        (MlColumn::Extra, "1"),
        (MlColumn::Price, "$100.00"),
        (MlColumn::Markup, "10"),
        (MlColumn::Labor, "$5.00"),
        (MlColumn::Equipment, "$2.00"),
        // (1 + 1) x 100 x 1.1 + 2 x 5 + 2 x 2
        (MlColumn::TotalCost, "$234.00"),
        (MlColumn::Default, ""),
        (MlColumn::Comment, "note"),
        (MlColumn::AccountingCode, "0310"),
    ];
    for (col, text) in want {
        assert_eq!(list::cell(d, col), text, "{}", col.title());
    }
    assert!(list::cell(d, MlColumn::Description).starts_with("Door"));
    // All 21 columns are covered between the table and the line above.
    assert_eq!(want.len() + 1, MlColumn::ALL.len());
}

// ---- Round 17 brief 04: reported framing and trim ----

fn catalogued_frame() -> Project {
    let mut p = framed();
    plan_framing::catalog::store(
        &mut p.floors[0].framing,
        &plan_framing::catalog::FramingCatalog::default(),
    );
    p
}

#[test]
fn a_catalogued_plan_lists_the_structural_member_report_by_category() {
    let p = catalogued_frame();
    let (auto, manual) = framing_members(&p.floors[0]);
    let mut cat = plan_framing::catalog::of_project(&p);
    // The active default decides the rows: Buy List, then Cut List.
    for cut in [false, true] {
        if cut {
            let d = cat
                .reporting
                .active_default()
                .converted("Cuts", plan_framing::reporting::ReportMethod::CutList);
            cat.reporting.defaults.push(d);
            assert!(cat.reporting.set_active("Cuts"));
            let mut q = p.clone();
            plan_framing::catalog::store(&mut q.floors[0].framing, &cat);
            check_report(&q, &auto, &manual, &cat);
        } else {
            check_report(&p, &auto, &manual, &cat);
        }
    }
}

fn check_report(
    p: &Project,
    auto: &[plan_framing::Member],
    manual: &[plan_framing::FramingMember],
    cat: &plan_framing::catalog::FramingCatalog,
) {
    let want = plan_framing::reporting::report_members(auto, manual, cat, false);
    assert!(!want.lines.is_empty());
    let got = all(p);
    for l in &want.lines {
        let row = got
            .iter()
            .find(|g| g.line.category == l.category.name() && g.line.item == l.description)
            .unwrap_or_else(|| panic!("no row for {}", l.description));
        assert!((row.line.net - l.qty).abs() < 1e-6, "{}", l.description);
        assert_eq!(row.line.unit, l.unit);
    }
    // The old per-style names are gone.
    assert!(!got.iter().any(|g| g.line.item.ends_with("linear feet")));
}

fn trim_house() -> Project {
    use plan_core::details::DetailsLayer;
    use plan_core::moldings::{builtin_profiles, MoldingTable, MoldingType};
    let mut p = Project::new("t");
    rect_walls(&mut p, 240.0, 180.0, 6.5, WallKind::Exterior);
    p.floors[0].room_names.push(plan_core::model::RoomName::new(
        Point::new(120.0, 90.0),
        "Kitchen",
        "Kitchen",
    ));
    let crown = builtin_profiles()
        .into_iter()
        .find(|x| x.kind == MoldingType::Crown)
        .unwrap();
    let mut table = MoldingTable::default();
    table.add_new(crown);
    let mut layer = DetailsLayer::default();
    layer.room_moldings_mut(Point::new(120.0, 90.0)).table = table;
    let floor = p.floors[0].clone();
    let rooms = plan_core::detect_rooms(&floor.walls, 0.5);
    let mut next = 100;
    layer.auto_corner_boards(&floor, &rooms, &mut || {
        next += 1;
        next
    });
    layer.store(&mut p.floors[0]);
    p
}

fn trim_total(lines: &[ListLine], category: &str) -> f64 {
    lines
        .iter()
        .filter(|l| l.line.category == category && l.line.unit == "lf")
        .map(|l| l.line.net)
        .sum()
}

#[test]
fn crown_and_corner_boards_list_as_interior_and_exterior_trim() {
    let p = trim_house();
    let lines = all(&p);
    let rooms = plan_core::detect_rooms(&p.floors[0].walls, 0.5);
    let ring = &rooms[0].inner_polygon;
    let perimeter: f64 = (0..ring.len())
        .map(|i| ring[i].dist(ring[(i + 1) % ring.len()]))
        .sum();
    let crown = trim_total(&lines, "Interior Trim");
    assert!((crown - (perimeter / 12.0).ceil()).abs() <= 1.0, "{crown}");
    let boards = find_prefix(&lines, "Exterior Trim", "Corner Board");
    let h = p.floors[0].walls[0].height;
    assert_eq!(boards.line.net, (4.0 * 2.0 * h / 12.0 - 1e-9).ceil());
    // Rows are numbered with the trim prefixes and sort after the finishes.
    assert!(boards.line.id.starts_with("ETR-"), "{}", boards.line.id);
    let spec = ListSpec::new("L", ListScope::AllFloors);
    let csv = String::from_utf8(export::export(
        &lines,
        &spec,
        &ExportOptions::default(),
        "T",
    ))
    .unwrap();
    assert!(csv.contains("ITR-001") && csv.contains("ETR-001"), "{csv}");
    let flat = to_csv(&take_off(&p, MaterialsScope::AllFloors, None, &plain()));
    assert!(flat.contains("Interior Trim") && flat.contains("Exterior Trim"));
}

fn find_prefix<'a>(lines: &'a [ListLine], category: &str, item: &str) -> &'a ListLine {
    lines
        .iter()
        .find(|l| l.line.category == category && l.line.item.starts_with(item))
        .unwrap_or_else(|| panic!("no {category} row {item}"))
}

#[test]
fn selection_and_room_scopes_count_the_trim_of_what_they_pick() {
    let p = trim_house();
    let floors = [0usize];
    let run = |filter: Filter| {
        engine::take_off_raw(
            &p,
            &floors,
            None,
            &plain(),
            &Options {
                filter,
                ..Options::default()
            },
        )
    };
    let trim_rows = |raws: &[engine::Raw]| {
        raws.iter()
            .filter(|r| r.line.category.ends_with("Trim"))
            .map(|r| r.line.category.clone())
            .collect::<Vec<_>>()
    };
    // One corner board selected: its two boards, no crown.
    let one = run(Filter::Selection(
        [(0, "corner_board:101".to_string())].into_iter().collect(),
    ));
    assert_eq!(trim_rows(&one), vec!["Exterior Trim".to_string()]);
    let h = p.floors[0].walls[0].height;
    assert_eq!(one[0].line.net, (2.0 * h / 12.0 - 1e-9).ceil());
    // The room selected: its crown, no corner boards.
    let rooms = plan_core::detect_rooms(&p.floors[0].walls, 1.0);
    let key = engine::room_key(0, &rooms[0]);
    let sel = run(Filter::Selection([(0, key.clone())].into_iter().collect()));
    assert!(trim_rows(&sel).iter().all(|c| c == "Interior Trim"));
    assert!(!trim_rows(&sel).is_empty());
    // In Room counts the room's crown and leaves the outside boards out.
    let room = run(Filter::Room {
        floor: 0,
        key,
        polygon: rooms[0].polygon.clone(),
    });
    assert!(trim_rows(&room).iter().all(|c| c == "Interior Trim"));
    assert!(!trim_rows(&room).is_empty());
}
