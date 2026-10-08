//! Decks, fireplaces and split-level floors through the public API (CB-86,
//! CB-87, R-86): the Deck Specification and the Fireplace record survive a
//! save and load, the planking and the 3-2-10 chimney rule give the numbers
//! the dialogs show, and the level steps of a split level are found.

use plan_core::deck::{plank_layout, DeckPlanking, DeckSpec};
use plan_core::fireplace::{chimney_top, ChimneyTop, FireplaceKind};
use plan_core::split_level::{level_steps, risers_for};
use plan_core::{detect_rooms, Point, Project, RoomName, Wall, WallKind};

fn rect(w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ]
}

#[test]
fn a_deck_specification_and_a_fireplace_round_trip_through_a_saved_plan() {
    let mut p = Project::new("save");
    let mut name = RoomName::new(Point::new(10.0, 10.0), "Deck", "Deck");
    name.deck = Some(DeckSpec {
        planking: DeckPlanking {
            border: true,
            angle: 45.0,
            ..DeckPlanking::default()
        },
        ..DeckSpec::default()
    });
    p.floors[0].room_names.push(name);
    let id = p.add_fireplace(0, FireplaceKind::Prefab, Point::new(5.0, 5.0), 90.0);
    let json = serde_json::to_string(&p).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    let spec = back.floors[0].room_names[0].deck.as_ref().unwrap();
    assert!(spec.planking.border && spec.planking.angle == 45.0);
    let sym = back.floors[0].symbol(id).unwrap();
    assert_eq!(sym.angle, 90.0);
    assert_eq!(back.floors[0].fireplace_of(sym).kind, FireplaceKind::Prefab);
    // A plan from before decks and fireplaces loads without the new fields.
    let plain = serde_json::to_string(&Project::new("old")).unwrap();
    assert!(!plain.contains("fireplaces"));
    let old: Project = serde_json::from_str(&plain).unwrap();
    assert!(old.floors[0].fireplaces.is_empty());
}

#[test]
fn the_planking_numbers_follow_the_specification() {
    let outline = rect(144.0, 96.0);
    let spec = DeckPlanking::default();
    let layout = plank_layout(&outline, &spec);
    // 96 in of width at 5 3/4 in a board: 17 boards.
    assert_eq!(layout.field().count(), 17);
    let wide = plank_layout(
        &outline,
        &DeckPlanking {
            board_width: 7.25,
            ..spec.clone()
        },
    );
    assert!(wide.field().count() < layout.field().count());
    // Board feet: the covered area times 1 1/2 in over 144.
    let bf = layout.board_feet(1.5);
    assert!(bf > 100.0 && bf < 144.0, "{bf}");
}

#[test]
fn the_chimney_rule_reads_the_roof_it_is_given() {
    let mut p = Project::new("rule");
    let id = p.add_fireplace(0, FireplaceKind::Masonry, Point::new(100.0, 100.0), 0.0);
    let sym = p.floors[0].symbol(id).unwrap();
    let fp = p.floors[0].fireplace_of(sym);
    let flat = |_: Point| Some(120.0);
    assert!((chimney_top(&fp, sym, 0.0, 109.125, &flat) - 156.0).abs() < 1e-9);
    let mut fixed = fp.clone();
    fixed.chimney.top = ChimneyTop::Height(200.0);
    assert_eq!(chimney_top(&fixed, sym, 0.0, 109.125, &flat), 200.0);
}

#[test]
fn a_split_level_has_one_step_of_the_right_height_and_risers() {
    let mut p = Project::new("split");
    let f = &mut p.floors[0];
    let c = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        f.walls.push(Wall::new(
            Point::new(a.0, a.1),
            Point::new(b.0, b.1),
            4.5,
            109.0,
            WallKind::Exterior,
        ));
    }
    f.walls.push(Wall::new(
        Point::new(120.0, 0.0),
        Point::new(120.0, 120.0),
        4.5,
        109.0,
        WallKind::Interior,
    ));
    for (i, w) in f.walls.iter_mut().enumerate() {
        w.id = i as u64 + 1;
    }
    f.room_names
        .push(RoomName::new(Point::new(60.0, 60.0), "Low", "Living"));
    let mut high = RoomName::new(Point::new(180.0, 60.0), "High", "Living");
    high.floor_height_offset = 18.0;
    f.room_names.push(high);
    let rooms = detect_rooms(&f.walls, 0.5);
    let steps = level_steps(f, &rooms);
    assert_eq!(steps.len(), 1);
    assert!((steps[0].rise() - 18.0).abs() < 1e-9);
    assert_eq!(steps[0].risers(), risers_for(18.0));
    assert_eq!(risers_for(18.0), 3);
}
