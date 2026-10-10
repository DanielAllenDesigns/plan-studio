//! Scenario 52: the Library Browser and library objects, round 15.
//!
//! The dock filters by name and by type, a click or a drop places an object
//! (one undo step), Open Object shows the Library Object Specification with
//! its tabs and saves a User Catalog item's defaults, Replace From Library and
//! Convert to Symbol are one undo step, Delete goes to the Trash and comes
//! back, Export Library writes plain JSON and Import Library reads it back, a
//! path-traced thumbnail is cached, and the Chief importer's symbol resolver
//! finds library items by GUID and name.

use super::Sim;
use crate::dialogs::library_object;
use crate::editor::{details_view, ObjectRef};
use crate::toolbar::Dock;
use crate::tools::library::convert;
use crate::tools::library::user::{self as store, tests_support};
use crate::tools::library::{active_item, clear_active_item, library_catalog, set_active_item};
use crate::tools::ToolId;
use eframe::egui;
use plan_core::details::{Solid3d, SolidKind};
use plan_core::geometry::Point;
use plan_library::{CatalogItem, ItemKind, LibType, Placement, Stroke, Symbol2d};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ps-s52-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A fresh isolated User Catalog and a headless app.
fn setup() -> Sim {
    tests_support::fresh(false);
    clear_active_item();
    library_object::close();
    Sim::new()
}

/// A square user symbol `w` x `d` named `name` in `User > Symbols`.
fn user_symbol(name: &str, w: f64, d: f64) -> CatalogItem {
    let (hw, hd) = (w / 2.0, d / 2.0);
    let mut i = CatalogItem::new(
        store::new_id(ItemKind::Symbol),
        name,
        Placement::FreeStanding,
        Symbol2d::new(vec![Stroke::Polyline {
            points: vec![
                Point::new(-hw, -hd),
                Point::new(hw, -hd),
                Point::new(hw, hd),
                Point::new(-hw, hd),
            ],
            closed: true,
        }]),
    )
    .with_category(&["User", "Symbols"])
    .with_size(w, d, 30.0);
    i.tags = vec!["stool".into(), "seat".into()];
    i
}

/// One headless frame of the Library dock; returns the requests it made.
fn dock_frame(sim: &mut Sim) -> Vec<crate::shell::docks::DockRequest> {
    let ctx = sim.ctx.clone();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::shell::docks::show(ui, Dock::Library, &mut sim.app.cx, &mut sim.app.docks);
        });
    });
    std::mem::take(&mut sim.app.docks.requests)
}

/// One headless frame of the Library Object Specification, with `key`.
fn object_frame(sim: &mut Sim, key: Option<egui::Key>) {
    let ctx = sim.ctx.clone();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    if let Some(key) = key {
        input.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    let _ = ctx.run(input, |ctx| library_object::show_all(ctx, &mut sim.app.cx));
}

fn first_of_type(t: LibType) -> String {
    library_catalog()
        .all_items()
        .find(|i| {
            plan_library::types::classify(i) == Some(t) && i.placement == Placement::FreeStanding
        })
        .or_else(|| {
            library_catalog()
                .all_items()
                .find(|i| plan_library::types::classify(i) == Some(t))
        })
        .unwrap_or_else(|| panic!("a built-in {t:?} item"))
        .id
        .clone()
}

#[test]
fn the_dock_filters_by_name_and_by_type() {
    let mut sim = setup();
    // Search by name: "toilet" finds the toilets.
    sim.app.docks.library.query = "toilet".into();
    dock_frame(&mut sim);
    let hits = sim.app.docks.library.results();
    assert!(!hits.is_empty());
    assert!(hits
        .iter()
        .any(|i| i.id == "core.plumbing.toilet_elongated"));
    // Type Fixtures keeps them; Furniture drops them.
    sim.app.docks.library.user.filter.types = vec![LibType::Fixtures];
    dock_frame(&mut sim);
    assert!(sim
        .app
        .docks
        .library
        .results()
        .iter()
        .any(|i| i.id == "core.plumbing.toilet_elongated"));
    sim.app.docks.library.user.filter.types = vec![LibType::Furniture];
    dock_frame(&mut sim);
    assert!(sim.app.docks.library.results().is_empty());
    // Every type lists something in the built-in library (materials and
    // images come from the User Catalog and the Materials switch).
    sim.app.docks.library.query.clear();
    for t in [
        LibType::Cabinets,
        LibType::Furniture,
        LibType::Plants,
        LibType::Fixtures,
        LibType::Electrical,
    ] {
        sim.app.docks.library.user.filter.types = vec![t];
        dock_frame(&mut sim);
        let got = sim.app.docks.library.results();
        assert!(!got.is_empty(), "{t:?}");
        assert!(got
            .iter()
            .all(|i| plan_library::types::classify(i) == Some(t)));
    }
    // The twelve type names parse back.
    assert_eq!(LibType::ALL.len(), 12);
}

#[test]
fn a_click_places_the_object_in_one_undo_step() {
    let mut sim = setup();
    let id = first_of_type(LibType::Furniture);
    let ev = crate::shell::library_browser::LibraryEvent::Activate(id.clone());
    sim.app.docks.library.activate(&id);
    let tool =
        crate::shell::library_browser::apply_event(ev, &mut sim.app.docks.library, &mut sim.app.cx);
    assert_eq!(
        tool,
        Some(ToolId::Library),
        "choosing an item asks for the Library tool"
    );
    sim.tool(ToolId::Library);
    assert_eq!(active_item().as_deref(), Some(id.as_str()));
    let before = sim.cx().floor().symbols.len();
    let r = sim.click(200.0, 150.0);
    assert_eq!(r.commit.as_deref(), Some("Place Symbol"));
    assert_eq!(sim.cx().floor().symbols.len(), before + 1);
    assert_eq!(sim.cx().floor().symbols.last().unwrap().catalog_id, id);
    assert_eq!(sim.undo().as_deref(), Some("Place Symbol"));
    assert_eq!(sim.cx().floor().symbols.len(), before);
}

#[test]
fn a_drop_on_the_plan_places_the_dragged_object() {
    let mut sim = setup();
    let id = first_of_type(LibType::Furniture);
    let mut tool = crate::tools::library::LibraryTool::default();
    let before = sim.cx().floor().symbols.len();
    assert!(tool.drop_item(sim.cx(), &id, Point::new(300.0, 200.0)));
    let placed = sim.cx().floor().symbols.last().unwrap().clone();
    assert_eq!(placed.catalog_id, id);
    // Free-standing: the drop point is the middle of the footprint.
    let fp = placed.footprint();
    let mid = Point::new(
        fp.iter().map(|p| p.x).sum::<f64>() / 4.0,
        fp.iter().map(|p| p.y).sum::<f64>() / 4.0,
    );
    assert!(mid.dist(Point::new(300.0, 200.0)) < 30.0, "{mid:?}");
    assert_eq!(sim.cx().floor().symbols.len(), before + 1);
    assert_eq!(sim.undo().as_deref(), Some("Place Symbol"));
    assert_eq!(sim.cx().floor().symbols.len(), before);
    // An unknown id places nothing.
    assert!(!tool.drop_item(sim.cx(), "nope.nothing", Point::new(1.0, 1.0)));
    assert_eq!(sim.cx().floor().symbols.len(), before);
}

#[test]
fn open_object_shows_the_tabs_and_saves_a_user_items_defaults() {
    let mut sim = setup();
    let item = store::add(user_symbol("Stool", 16.0, 16.0), None).unwrap();
    // The Library menu path: the browser's Open Object request.
    sim.app
        .docks
        .library
        .user
        .perform(crate::shell::library_browser::UserAction::OpenObject(
            item.id.clone(),
        ));
    dock_frame(&mut sim);
    assert!(library_object::is_open(), "Open Object opened the dialog");
    assert_eq!(library_object::open_id().as_deref(), Some(item.id.as_str()));
    assert_eq!(
        crate::dialogs::symbol::tab_names(),
        [
            "General",
            "Options",
            "3D",
            "Materials",
            "Layer",
            "Label",
            "Components",
            "Object Information",
            "Schedule"
        ]
    );
    for _ in 0..3 {
        object_frame(&mut sim, None);
    }
    assert!(library_object::is_open());
    // OK with no change keeps the item as it was.
    object_frame(&mut sim, Some(egui::Key::Enter));
    object_frame(&mut sim, None);
    assert!(!library_object::is_open(), "Enter is OK");
    let same = store::item(&item.id).unwrap();
    assert_eq!((same.width, same.depth), (16.0, 16.0));
    assert!(same.defaults.is_none());
    // Cancel (Escape) closes without saving.
    assert!(library_object::open_item(sim.cx(), &item.id));
    object_frame(&mut sim, None);
    object_frame(&mut sim, None);
    object_frame(&mut sim, Some(egui::Key::Escape));
    object_frame(&mut sim, None);
    assert!(!library_object::is_open());
}

#[test]
fn replace_from_library_swaps_the_selection_in_one_undo_step() {
    let mut sim = setup();
    let a = first_of_type(LibType::Furniture);
    let b = library_catalog()
        .all_items()
        .find(|i| i.id != a && plan_library::types::classify(i) == Some(LibType::Furniture))
        .unwrap()
        .id
        .clone();
    sim.tool(ToolId::Library);
    assert!(set_active_item(sim.cx(), &a));
    sim.click(100.0, 100.0);
    sim.click(250.0, 100.0);
    let ids: Vec<_> = sim
        .cx()
        .floor()
        .symbols
        .iter()
        .map(|s| (s.id, s.position, s.angle))
        .collect();
    assert_eq!(ids.len(), 2);
    sim.cx().selection.clear();
    for (id, _, _) in &ids {
        sim.cx().selection.add(ObjectRef::Symbol(*id));
    }
    // Pick another library item, then Replace From Library.
    assert!(set_active_item(sim.cx(), &b));
    convert::set_keep_size(true);
    let n = convert::replace_selected(sim.cx());
    assert_eq!(n, 2);
    for (id, pos, ang) in &ids {
        let s = sim.cx().floor().symbol(*id).unwrap().clone();
        assert_eq!(s.catalog_id, b);
        assert_eq!(
            (s.position, s.angle),
            (*pos, *ang),
            "position and angle stay"
        );
    }
    // One undo step puts both back.
    assert_eq!(sim.undo().as_deref(), Some("Replace From Library"));
    for (id, _, _) in &ids {
        assert_eq!(sim.cx().floor().symbol(*id).unwrap().catalog_id, a);
    }
    // With Keep size off the new item's own size is used.
    convert::set_keep_size(false);
    let item = crate::tools::library::find_item(&b).unwrap();
    assert_eq!(convert::replace_selected(sim.cx()), 2);
    let s = sim.cx().floor().symbol(ids[0].0).unwrap().clone();
    assert_eq!((s.width, s.depth), (item.width, item.depth));
    convert::set_keep_size(true);
    // The same item again changes nothing.
    assert_eq!(convert::replace_selected(sim.cx()), 0);
}

#[test]
fn convert_to_symbol_turns_solids_into_a_user_symbol_in_one_step() {
    let mut sim = setup();
    // Two boxes and a polyline solid.
    let ids: Vec<_> = (0..3).map(|_| sim.cx().project.alloc_id()).collect();
    details_view::edit(sim.cx(), "Solids", |l| {
        let mut a = Solid3d::new(
            ids[0],
            SolidKind::Box {
                w: 40.0,
                d: 20.0,
                h: 30.0,
            },
            Point::new(100.0, 100.0),
        );
        a.elevation = 4.0;
        let mut b = Solid3d::new(
            ids[1],
            SolidKind::Box {
                w: 10.0,
                d: 10.0,
                h: 50.0,
            },
            Point::new(125.0, 100.0),
        );
        b.elevation = 4.0;
        let mut c = Solid3d::new(
            ids[2],
            SolidKind::PolylineSolid {
                outline: vec![
                    Point::new(-10.0, -10.0),
                    Point::new(10.0, -10.0),
                    Point::new(0.0, 10.0),
                ],
                h: 12.0,
            },
            Point::new(100.0, 130.0),
        );
        c.elevation = 4.0;
        l.solids.extend([a, b, c]);
    });
    sim.cx().selection.clear();
    assert!(!convert::can_convert(sim.cx()));
    for id in &ids {
        sim.cx().selection.add(ObjectRef::Detail(*id));
    }
    assert!(convert::can_convert(sim.cx()));
    let before_undo = sim.cx().undo_label().map(str::to_owned);
    let c = convert::convert_to_symbol(sim.cx(), Some("Planter Group")).unwrap();
    assert_eq!(c.solids, 3);
    assert!(
        details_view::load(sim.cx()).solids.is_empty(),
        "the solids are gone"
    );
    // The new symbol is in the User Catalog with a 3D model and the bounding
    // box of the solids: x 80..130, y 90..140 (the triangle), z 4..54.
    let item = store::item(&c.item.id).expect("in the User Catalog");
    assert_eq!(item.name, "Planter Group");
    assert_eq!(item.kind, ItemKind::Model);
    assert!(store::model_of(&item).is_some());
    assert!((item.width - 50.0).abs() < 0.1, "{}", item.width);
    assert!((item.depth - 50.0).abs() < 0.1, "{}", item.depth);
    assert!((item.height - 50.0).abs() < 0.1, "{}", item.height);
    assert!(!item.symbol.is_empty());
    // A placed copy took the solids' place, selected.
    let sym = sim.cx().floor().symbol(c.symbol).unwrap().clone();
    assert_eq!(sym.catalog_id, item.id);
    assert_eq!(sym.elevation, 4.0);
    assert_eq!(
        sim.cx().selection.single(),
        Some(ObjectRef::Symbol(c.symbol))
    );
    // One undo step brings the solids back and removes the symbol.
    assert_eq!(sim.undo().as_deref(), Some("Convert to Symbol"));
    assert_eq!(details_view::load(sim.cx()).solids.len(), 3);
    assert!(sim.cx().floor().symbol(c.symbol).is_none());
    let _ = before_undo;
    // Nothing selected: a message, no change.
    sim.cx().selection.clear();
    assert!(convert::convert_to_symbol(sim.cx(), None).is_err());
}

#[test]
fn delete_goes_to_the_trash_and_restore_brings_it_back() {
    let mut sim = setup();
    store::create_folder(&["User".to_string(), "Kitchen".to_string()]).unwrap();
    let mut it = user_symbol("Island Stool", 14.0, 14.0);
    it.category = vec!["User".into(), "Kitchen".into()];
    let it = store::add(it, None).unwrap();
    let ui = &mut sim.app.docks.library.user;
    ui.perform(crate::shell::library_browser::UserAction::Delete(
        it.id.clone(),
    ));
    assert!(store::item(&it.id).is_some(), "asks first");
    assert!(store::delete(&it.id).unwrap());
    assert!(store::item(&it.id).is_none());
    assert_eq!(store::trash().len(), 1);
    // The panel draws the Trash node and list.
    sim.app.docks.library.user.view = crate::shell::library_browser::View::Trash;
    dock_frame(&mut sim);
    let msg =
        sim.app
            .docks
            .library
            .user
            .perform(crate::shell::library_browser::UserAction::Restore(
                it.id.clone(),
            ));
    assert!(msg.unwrap().contains("Restored"));
    let back = store::item(&it.id).unwrap();
    assert_eq!(back.category, ["User", "Kitchen"], "back in its folder");
    assert!(store::trash().is_empty());
    // Emptying erases for good.
    assert!(store::delete(&it.id).unwrap());
    assert_eq!(store::empty_trash().unwrap(), 1);
    assert!(store::restore(&it.id).is_err());
}

#[test]
fn export_writes_json_and_import_reads_it_back() {
    let mut sim = setup();
    let dir = temp_dir("io");
    let a = store::add(user_symbol("Chair A", 18.0, 18.0), None).unwrap();
    let b = store::add(user_symbol("Chair B", 20.0, 20.0), None).unwrap();
    store::toggle_favorite(&a.id);
    let out = dir.join("my-library.json");
    assert_eq!(store::export_library_to(&out).unwrap(), 2);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.trim_start().starts_with('{'));
    assert!(store::export_library_to(&dir.join("my.calib")).is_err());
    assert!(!dir.join("my.calib").exists(), "never a .calib");
    // Into an empty catalog.
    tests_support::fresh(false);
    assert!(store::items().is_empty());
    let status = store::import_status(&out);
    assert!(status.contains("Imported 2"), "{status}");
    assert!(store::item(&a.id).is_some() && store::item(&b.id).is_some());
    assert!(store::meta().is_favorite(&a.id));
    // A file that is no library says so; the plan stays untouched.
    let junk = dir.join("notes.txt");
    std::fs::write(&junk, "hello").unwrap();
    assert!(store::import_status(&junk).contains("neither"));
    let _ = sim.cx();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_importers_resolver_finds_library_items_by_guid_and_name() {
    let _sim = setup();
    let guid = "4C528223-D712-45E5-B7A3-27F75F3E88C1";
    let mut it = user_symbol("Quiet Toilet", 15.0, 28.0);
    it.tags.push(plan_library::resolve::guid_tag(guid));
    let it = store::add(it, None).unwrap();
    // By GUID (any letter case), ahead of the name.
    assert_eq!(
        crate::tools::library::resolve_symbol_query("Elongated Toilet", &[], Some(guid), &[]),
        Some(it.id.clone())
    );
    // By name among the built-in catalogs.
    let toilet = library_catalog()
        .get("core.plumbing.toilet_elongated")
        .unwrap();
    let found =
        crate::tools::library::resolve_symbol_query(&toilet.name.to_uppercase(), &[], None, &[]);
    assert!(found.is_some());
    assert_eq!(
        crate::tools::library::find_item(&found.unwrap())
            .unwrap()
            .name,
        toilet.name
    );
    assert_eq!(
        crate::tools::library::resolve_symbol_query("No Such Thing", &[], None, &[]),
        None
    );
    // The importer's hook (Chief catalogs off) falls back to the same lookup.
    let q = plan_chiefplan::import::SymbolQuery {
        name: "Quiet Toilet".into(),
        tags: Vec::new(),
        unique_id: None,
        candidates: Vec::new(),
    };
    assert_eq!(crate::chief_link::resolve_symbol(&q), Some(it.id.clone()));
}

#[test]
fn the_preview_pane_renders_a_cached_path_traced_thumbnail() {
    use crate::shell::library_panel::ThumbService;
    let mut sim = setup();
    let dir = temp_dir("thumbs");
    let id = first_of_type(LibType::Furniture);
    {
        let st = &mut sim.app.docks.library;
        st.user.thumbs =
            ThumbService::with_cache(Some(plan_library::thumbs::ThumbCache::new(&dir)));
        st.user.selected = Some(id.clone());
        st.query = "a".into();
    }
    // Frames until the background render lands in the cache.
    let end = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        dock_frame(&mut sim);
        if !sim
            .app
            .docks
            .library
            .user
            .thumbs
            .cache()
            .unwrap()
            .is_empty()
        {
            break;
        }
        assert!(std::time::Instant::now() < end, "thumbnail never rendered");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let n = std::fs::read_dir(&dir).unwrap().count();
    assert!(n >= 1);
    // A new session finds it without rendering.
    let mut again = ThumbService::with_cache(Some(plan_library::thumbs::ThumbCache::new(&dir)));
    let item = crate::tools::library::find_item(&id).unwrap();
    let (model, _) = crate::shell::library_browser::preview_model(&item);
    assert!(matches!(
        again.get(&sim.ctx, &item, &model),
        crate::shell::library_panel::Thumb::Ready(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
}
