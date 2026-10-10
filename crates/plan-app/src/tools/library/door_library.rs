//! Doors from the library for the Door Specification (DW-55, DW-126, DW-139).
//!
//! The Door Style list of the General panel has a Library entry and a button
//! that opens Select Library Object, listing only doors: the "Doors and
//! Doorways" branch of the Chief catalogs (its Interior Doors, Exterior
//! Doors, Garage and Entryways folders, as the install has them) and the
//! doors of the built-in and user libraries. Chief catalogs are read in place
//! and only for names and categories; nothing of their content is copied.
//! Only the catalog id and the name are stored with the door
//! ([`LibraryDoor`]); the 3D view reads the symbol when the catalog is
//! installed and keeps the built-in panel door when it is not.

use crate::editor::EditorContext;
use crate::tools::ToolResult;
use plan_core::geometry::Point;
use plan_core::openings::{DoorLeafStyle, LibraryDoor};
use plan_core::{Opening, OpeningKind, OpeningStyle, PlacedSymbol, Project};
use plan_library::{CatalogItem, LibType};
use std::cell::RefCell;

/// The Chief category whose folders hold the doors.
pub const CATEGORY: &str = "Doors and Doorways";

/// One door on offer in Select Library Object.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorEntry {
    pub id: String,
    pub name: String,
    /// The catalog's name, or "Library" / "User Library".
    pub source: String,
    /// The folder under "Doors and Doorways" ("Interior Doors", "Exterior
    /// Doors", "Garage", "Entryways"...), empty when there is none.
    pub folder: String,
    /// The door type the symbol's Options panel sets, read from its name and
    /// keywords (DOORLIB-4); `None` when the catalog says nothing, so the
    /// dialog's Door Type stands.
    pub door_type: Option<OpeningStyle>,
}

/// The door type a catalog door names in its name or keywords (Bi-Fold,
/// Pocket, Barn, Sliding, Overhead or Garage, Double Door, Hinged or Swing).
/// The catalog's own `type_code` is not decoded, so this is the only reading
/// of the symbol's Options-panel door type that is safe.
pub fn door_type_hint(name: &str, keywords: &[String]) -> Option<OpeningStyle> {
    let text = format!("{name} {}", keywords.join(" ")).to_lowercase();
    let has = |w: &str| text.contains(w);
    if has("bi-fold") || has("bifold") || has("bi fold") {
        Some(OpeningStyle::Bifold)
    } else if has("pocket") {
        Some(OpeningStyle::Pocket)
    } else if has("barn") {
        Some(OpeningStyle::Barn)
    } else if has("slider") || has("sliding") {
        Some(OpeningStyle::Sliding)
    } else if has("overhead") || has("garage") {
        Some(OpeningStyle::Garage)
    } else if has("double door") {
        Some(OpeningStyle::DoubleDoor)
    } else if has("hinged") || has("swing") {
        Some(OpeningStyle::Hinged)
    } else {
        None
    }
}

thread_local! {
    static LAST_FOLDER: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The folder Select Library Object was last used in (empty: all folders).
pub fn last_folder() -> String {
    LAST_FOLDER.with(|f| f.borrow().clone())
}

/// Remembers `folder` as the picker's last folder.
pub fn set_last_folder(folder: &str) {
    LAST_FOLDER.with(|f| *f.borrow_mut() = folder.to_string());
}

/// The entries in `folder` (empty: every folder) whose name contains `search`
/// (case-insensitive).
pub fn filtered<'a>(entries: &'a [DoorEntry], folder: &str, search: &str) -> Vec<&'a DoorEntry> {
    let s = search.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| folder.is_empty() || e.folder == folder)
        .filter(|e| s.is_empty() || e.name.to_lowercase().contains(&s))
        .collect()
}

fn has_step(path: &[String], step: &str) -> bool {
    path.iter().any(|s| s.eq_ignore_ascii_case(step))
}

/// The folder below "Doors and Doorways" in `path`.
pub fn folder_of(path: &[String]) -> String {
    path.iter()
        .position(|s| s.eq_ignore_ascii_case(CATEGORY))
        .and_then(|i| path.get(i + 1))
        .cloned()
        .unwrap_or_default()
}

/// Whether an object with this category path, name and keywords is a door
/// (a cabinet door or drawer front is not).
pub fn is_door_object(path: &[String], name: &str, keywords: &[String]) -> bool {
    if has_step(path, "Cabinet Doors") || has_step(path, "Cabinet Drawers") {
        return false;
    }
    if has_step(path, CATEGORY) {
        return true;
    }
    let text = format!("{} {} {}", path.join(" "), name, keywords.join(" "));
    plan_library::types::classify_text(&text) == Some(LibType::Doors)
}

/// The doors among `items`, sorted by folder then name.
pub fn from_items<'a>(
    items: impl IntoIterator<Item = &'a CatalogItem>,
    source: &str,
) -> Vec<DoorEntry> {
    let mut out: Vec<DoorEntry> = items
        .into_iter()
        .filter(|i| {
            plan_library::types::classify(i) == Some(LibType::Doors)
                && is_door_object(&i.category, &i.name, &i.tags)
        })
        .map(|i| DoorEntry {
            id: i.id.clone(),
            name: i.name.clone(),
            source: source.to_string(),
            folder: folder_of(&i.category),
            door_type: door_type_hint(&i.name, &i.tags),
        })
        .collect();
    out.sort_by_key(|e| (e.folder.to_lowercase(), e.name.to_lowercase()));
    out
}

thread_local! {
    static CHIEF: RefCell<Option<Vec<DoorEntry>>> = const { RefCell::new(None) };
}

/// Reads the doors of the installed Chief catalogs (core, bonus and user
/// catalogs; manufacturer catalogs are left out) and keeps them for the
/// session. Returns how many were found; nothing is read when the Chief
/// catalogs are switched off.
pub fn scan_chief() -> usize {
    let found = chief_doors();
    let n = found.len();
    CHIEF.with(|c| *c.borrow_mut() = Some(found));
    n
}

/// The doors of the installed Chief catalogs, read now.
pub fn chief_doors() -> Vec<DoorEntry> {
    use super::chief;
    use plan_calib::CatalogKind;
    let mut found = Vec::new();
    if let Some(lib) = chief::library() {
        for (idx, entry) in lib.catalogs().iter().enumerate() {
            if entry.path.is_none()
                || matches!(entry.kind, CatalogKind::Deleted | CatalogKind::Manufacturer)
            {
                continue;
            }
            let Ok(cat) = lib.open(idx) else { continue };
            let Ok(objects) = cat.objects() else { continue };
            for o in objects {
                if !is_door_object(&o.category_path, &o.name, &o.keywords) {
                    continue;
                }
                found.push(DoorEntry {
                    id: format!("{}{}.{}", chief::ID_PREFIX, cat.id(), o.library_object_id),
                    name: o.name.clone(),
                    source: entry.name.clone(),
                    folder: folder_of(&o.category_path),
                    door_type: door_type_hint(&o.name, &o.keywords),
                });
            }
        }
    }
    found.sort_by_key(|e| (e.folder.to_lowercase(), e.name.to_lowercase()));
    found
}

/// Keeps `entries` as the scanned Chief doors (a fixture catalog in tests).
pub fn install_scanned(entries: Vec<DoorEntry>) {
    CHIEF.with(|c| *c.borrow_mut() = Some(entries));
}

/// Forgets the scanned doors (the install folder changed).
pub fn forget_chief() {
    CHIEF.with(|c| *c.borrow_mut() = None);
}

/// Whether the Chief catalogs were scanned in this session.
pub fn chief_scanned() -> bool {
    CHIEF.with(|c| c.borrow().is_some())
}

/// Every door on offer: the built-in and user libraries, then the scanned
/// Chief ones.
pub fn available() -> Vec<DoorEntry> {
    let mut out = from_items(super::library_catalog().all_items(), "Library");
    out.extend(from_items(super::user::items().iter(), "User Library"));
    CHIEF.with(|c| {
        if let Some(chief) = c.borrow().as_ref() {
            out.extend(chief.iter().cloned());
        }
    });
    out
}

/// The folder names of `entries`, in order of first appearance.
pub fn folders(entries: &[DoorEntry]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in entries {
        if !e.folder.is_empty() && !out.contains(&e.folder) {
            out.push(e.folder.clone());
        }
    }
    out
}

/// Whether the library item `item` is a door that can stand in an opening.
pub fn is_door_item(item: &CatalogItem) -> bool {
    from_items(std::iter::once(item), "").len() == 1
}

/// Makes `o` the library door `entry`: the Library style, the stored item and
/// the name the Library Style row shows.
pub fn choose(entry: &DoorEntry, o: &mut Opening) {
    let spec = &mut o.extras.spec;
    spec.door_style = DoorLeafStyle::Library;
    spec.library_door = Some(LibraryDoor {
        id: entry.id.clone(),
        name: entry.name.clone(),
    });
    o.extras.style_name = Some(entry.name.clone());
    // The symbol's own door type wins over the dialog's (DOORLIB-4).
    if let Some(t) = entry.door_type {
        o.style = t;
    }
}

// ----- 3D -----

/// A symbol standing in `o`'s opening in `wall`: as wide as the opening, as
/// deep as the wall, from the sill up.
fn symbol_in(project: &Project, floor: usize, o: &Opening) -> Option<PlacedSymbol> {
    let lib = o.extras.spec.library_door.as_ref()?;
    let wall = project
        .floors
        .get(floor)?
        .walls
        .iter()
        .find(|w| w.id == o.wall_id)?;
    let c = wall.point_offset(o.center_offset, 0.0);
    let d = wall.direction();
    let mut s = PlacedSymbol::new(lib.id.clone(), c, o.width, wall.thickness, o.height);
    s.angle = d.y.atan2(d.x).to_degrees()
        + if o.extras.spec.library_reversed {
            180.0
        } else {
            0.0
        };
    s.elevation = o.sill_height;
    Some(s)
}

/// The scene's library doors: the project to build the scene from (openings
/// whose symbol is drawn here have the flag that stops the built-in leaf) and
/// the symbol meshes. `None` when no door uses a library item that can be
/// drawn, so the built-in panel door stays (also when the catalog is not
/// installed).
pub fn scene_doors(project: &Project) -> Option<(Project, Vec<plan_3d::Mesh>)> {
    use super::chief::{self, Chief3d};
    let mut work: Option<Project> = None;
    let mut meshes = Vec::new();
    for fi in 0..project.floors.len() {
        for oi in 0..project.floors[fi].openings.len() {
            let o = &project.floors[fi].openings[oi];
            if o.kind != OpeningKind::Door || !o.extras.spec.is_library_door() {
                continue;
            }
            let Some(sym) = symbol_in(project, fi, o) else {
                continue;
            };
            let elev = project.floors[fi].elevation;
            let drawn = if chief::is_chief_id(&sym.catalog_id) {
                match chief::placed_meshes(&sym, elev) {
                    Chief3d::Meshes(m) => Some(m),
                    _ => None,
                }
            } else {
                super::user::placed_meshes(&sym, elev)
            };
            if let Some(m) = drawn {
                meshes.extend(m);
                work.get_or_insert_with(|| project.clone()).floors[fi].openings[oi]
                    .extras
                    .spec
                    .library_drawn = true;
            }
        }
    }
    work.map(|p| (p, meshes))
}

// ----- placing from the Library Browser -----

/// A door picked in the Library Browser, clicked at `at`: on a door it
/// replaces that door's style; on a wall it places a new door of the default
/// type carrying the style. `None` when the click is on neither (or the door
/// would not fit), so the generic placement goes on.
pub fn place_door(cx: &mut EditorContext, item: &CatalogItem, at: Point) -> Option<ToolResult> {
    let entry = from_items(std::iter::once(item), "Library")
        .into_iter()
        .next()?;
    let fl = cx.floor;
    let floor = cx.project.floors.get(fl)?;
    // An existing door (or doorway) there takes the library door.
    if let Some(id) = crate::editor::selection::hit_opening(floor, at, 2.0) {
        let o = floor.openings.iter().find(|o| o.id == id)?;
        if o.kind == OpeningKind::Door {
            cx.begin_change("Place Library Door");
            let o = cx.project.floors[fl]
                .openings
                .iter_mut()
                .find(|o| o.id == id)?;
            choose(&entry, o);
            // A doorway becomes a door; the older style stays otherwise.
            if o.style == OpeningStyle::Doorway {
                o.style = OpeningStyle::Hinged;
            }
            cx.selection
                .set(crate::editor::selection::ObjectRef::Opening(id));
            cx.mark_dirty();
            cx.status = format!("Placed {}", entry.name);
            return Some(ToolResult::committed("Place Library Door"));
        }
    }
    // A wall: a new door of the default type.
    let wall = crate::tools::camera::wall_at(&cx.project, fl, at, 6.0)?.clone();
    let (along, _) = wall.locate(at);
    let (_, template) = cx.opening_template(OpeningKind::Door, wall.kind);
    let mut o = cx.defaults.opening_variants.place(
        &template,
        OpeningStyle::Hinged,
        wall.kind == plan_core::WallKind::Exterior,
    );
    o.wall_id = wall.id;
    o.center_offset = along;
    choose(&entry, &mut o);
    if wall.length() < o.width + 2.0 {
        return None;
    }
    let half = o.width * 0.5 + 1.0;
    o.center_offset = along.clamp(half, wall.length() - half);
    if cx.project.floors[fl]
        .openings_on(wall.id)
        .any(|x| plan_core::openings::placement::conflict(&o, x))
    {
        return None;
    }
    cx.begin_change("Place Library Door");
    o.id = cx.project.alloc_id();
    let id = o.id;
    cx.project.floors[fl].openings.push(o);
    cx.selection
        .set(crate::editor::selection::ObjectRef::Opening(id));
    cx.mark_dirty();
    cx.status = format!("Placed {}", entry.name);
    Some(ToolResult::committed("Place Library Door"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::{Placement, Symbol2d};

    pub(crate) fn item(id: &str, name: &str, category: &[&str]) -> CatalogItem {
        CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default())
            .with_category(category)
    }

    #[test]
    fn only_doors_are_listed_and_cabinet_doors_are_not() {
        let items = [
            item(
                "a",
                "Six Panel",
                &["Architectural", CATEGORY, "Interior Doors"],
            ),
            item(
                "b",
                "Flush Slab",
                &["Architectural", CATEGORY, "Exterior Doors"],
            ),
            item("c", "Shaker Front", &["Cabinets", "Cabinet Doors"]),
            item("d", "Round Table", &["Furniture", "Tables"]),
        ];
        let e = from_items(items.iter(), "Test");
        assert_eq!(e.len(), 2);
        assert_eq!(folders(&e), vec!["Exterior Doors", "Interior Doors"]);
        assert!(is_door_item(&items[0]) && !is_door_item(&items[2]) && !is_door_item(&items[3]));
    }

    #[test]
    fn choosing_sets_the_style_item_and_name() {
        let mut o = Opening::default_door(1, 1, 50.0);
        choose(
            &DoorEntry {
                id: "chief.u.3".into(),
                name: "Colonial".into(),
                source: "T".into(),
                folder: String::new(),
                door_type: None,
            },
            &mut o,
        );
        assert!(o.extras.spec.is_library_door());
        assert_eq!(o.extras.style_name.as_deref(), Some("Colonial"));
    }

    fn entry(name: &str, folder: &str, door_type: Option<OpeningStyle>) -> DoorEntry {
        DoorEntry {
            id: format!("chief.t.{name}"),
            name: name.into(),
            source: "T".into(),
            folder: folder.into(),
            door_type,
        }
    }

    #[test]
    fn the_door_type_comes_from_the_symbol_name_or_keywords_else_the_dialog_keeps_its_own() {
        let kw = |k: &[&str]| k.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            door_type_hint("Bifold 4 Panel", &[]),
            Some(OpeningStyle::Bifold)
        );
        assert_eq!(
            door_type_hint("Plain Slab", &kw(&["Pocket"])),
            Some(OpeningStyle::Pocket)
        );
        assert_eq!(door_type_hint("Barn Door", &[]), Some(OpeningStyle::Barn));
        assert_eq!(door_type_hint("Six Panel", &kw(&["interior"])), None);
        let mut o = Opening::default_door(1, 1, 50.0);
        o.style = OpeningStyle::Sliding;
        choose(&entry("Six Panel", "Panel", None), &mut o);
        assert_eq!(o.style, OpeningStyle::Sliding, "no hint: Door Type stands");
        choose(
            &entry("Pocket Slab", "Slab", Some(OpeningStyle::Pocket)),
            &mut o,
        );
        assert_eq!(o.style, OpeningStyle::Pocket);
        // The type is read when the catalog items are listed.
        let items = [item(
            "p",
            "Pocket Panel",
            &["Architectural", CATEGORY, "Panel"],
        )];
        assert_eq!(
            from_items(items.iter(), "T")[0].door_type,
            Some(OpeningStyle::Pocket)
        );
    }

    #[test]
    fn the_picker_remembers_its_last_folder_and_searches_by_name() {
        let all = vec![
            entry("Six Panel", "Panel", None),
            entry("Flush", "Slab", None),
            entry("Four Panel", "Panel", None),
        ];
        set_last_folder("");
        assert_eq!(last_folder(), "");
        set_last_folder("Panel");
        assert_eq!(last_folder(), "Panel");
        assert_eq!(filtered(&all, &last_folder(), "").len(), 2);
        assert_eq!(filtered(&all, "", "FLU").len(), 1);
        assert_eq!(filtered(&all, "Panel", "four")[0].name, "Four Panel");
        assert!(filtered(&all, "Slab", "panel").is_empty());
        set_last_folder("");
    }

    /// Needs Daniel's installed Chief catalogs. Run with:
    /// `cq.sh test -p plan-app --lib real_install_door_folders -- --ignored`
    #[test]
    #[ignore = "reads the real Chief X18 install"]
    fn real_install_door_folders() {
        super::super::chief::reset_for_tests();
        // The library handle answers only while the catalogs are switched on.
        super::super::chief::configure(&super::super::chief::ChiefSettings {
            enabled: true,
            folder: None,
        });
        super::super::chief::set_library(std::sync::Arc::new(super::super::chief::discover(None)));
        let doors = chief_doors();
        // Chief X18's core catalog files doors by style, not by interior or
        // exterior: Bi-Fold and Side Lites, Doorways and Openings, Entryways,
        // Garage Doors, Glass Panel, Louvered, Panel and Slab (DECISIONS
        // DOORLIB-3). Interior versus exterior is the Door Defaults' choice.
        let folders = folders(&doors);
        eprintln!("door folders: {folders:?} ({} doors)", doors.len());
        assert!(doors.len() >= 100, "only {} doors found", doors.len());
        for f in ["Panel", "Glass Panel", "Slab"] {
            assert!(
                folders.iter().any(|x| x == f),
                "no {f} folder in {folders:?}"
            );
        }
    }
}
