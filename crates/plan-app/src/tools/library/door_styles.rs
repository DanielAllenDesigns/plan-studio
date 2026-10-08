//! Door and drawer front styles from the Library (CB-12).
//!
//! Chief's Cabinet Specification picks the Main Style of the Door Panel and
//! the Drawer Panel from library objects in the category "Cabinet Doors".
//! Here the styles come from every library Plan Studio can read:
//!
//! * the built-in, user and (once scanned) Chief catalogs, by the category
//!   path: an object whose category path has a "Cabinet Doors" step is a door
//!   style, one with a "Cabinet Drawers" step a drawer style (a door style
//!   also serves as a drawer front);
//! * the Chief catalogs are read in place and only for the names and
//!   categories of those objects; nothing of Chief's content is copied.
//!   Scanning opens the installed catalogs, so it runs on request
//!   ([`scan_chief`]) and the result is kept for the session.
//!
//! A picked style copies its look into the cabinet's [`DoorStyle`] (the name,
//! the profile, the glass and the library id) so the plan keeps it when the
//! library is not installed; the 3D view builds the front from the profile.
//! The profile is read from the object's name and keywords ("shaker",
//! "raised", "glass", ...).

use plan_cabinets::{DoorProfile, DoorStyle, DrawerStyle};
use plan_library::CatalogItem;
use std::cell::RefCell;

/// The category step that marks a door style.
pub const DOOR_CATEGORY: &str = "Cabinet Doors";
/// The category step that marks a drawer front style.
pub const DRAWER_CATEGORY: &str = "Cabinet Drawers";

/// One style on offer in the Door/Drawer tab.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryStyle {
    /// The library object id (`core...`, `user....`, `chief.<uuid>.<n>`).
    pub id: String,
    pub name: String,
    /// Where it comes from: the catalog's name or "Library".
    pub source: String,
    pub profile: DoorProfile,
    pub glass: bool,
    /// Offered for drawer fronts only.
    pub drawer_only: bool,
}

/// The panel profile and glass a style's name and keywords describe.
pub fn look_of(name: &str, keywords: &[String]) -> (DoorProfile, bool) {
    let text = std::iter::once(name)
        .chain(keywords.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let has = |w: &str| text.contains(w);
    let profile = if has("raised") || has("cathedral") || has("arch") {
        DoorProfile::Raised
    } else if has("shaker") || has("recessed") || has("mission") || has("frame") {
        DoorProfile::Shaker
    } else {
        DoorProfile::Slab
    };
    let glass = has("glass") || has("mullion") || has("lite") || has("light door");
    (profile, glass)
}

fn category_has(path: &[String], step: &str) -> bool {
    path.iter().any(|s| s.eq_ignore_ascii_case(step))
}

/// The styles among `items`: objects under "Cabinet Doors" or "Cabinet
/// Drawers", sorted by source then name.
pub fn from_items<'a>(
    items: impl IntoIterator<Item = &'a CatalogItem>,
    source: &str,
) -> Vec<LibraryStyle> {
    let mut out: Vec<LibraryStyle> = items
        .into_iter()
        .filter_map(|i| {
            let door = category_has(&i.category, DOOR_CATEGORY);
            let drawer = category_has(&i.category, DRAWER_CATEGORY);
            if !door && !drawer {
                return None;
            }
            let (profile, glass) = look_of(&i.name, &i.tags);
            Some(LibraryStyle {
                id: i.id.clone(),
                name: i.name.clone(),
                source: source.to_string(),
                profile,
                glass,
                drawer_only: drawer && !door,
            })
        })
        .collect();
    out.sort_by_key(|s| s.name.to_lowercase());
    out
}

thread_local! {
    static CHIEF: RefCell<Option<Vec<LibraryStyle>>> = const { RefCell::new(None) };
}

/// Reads the names of the "Cabinet Doors" and "Cabinet Drawers" objects of
/// the installed Chief catalogs (core, bonus and user catalogs; manufacturer
/// catalogs are left out) and keeps them. Returns how many styles were found;
/// nothing is read when the Chief catalogs are switched off.
pub fn scan_chief() -> usize {
    let mut found = Vec::new();
    if let Some(lib) = super::chief::library() {
        for (idx, entry) in lib.catalogs().iter().enumerate() {
            use plan_calib::CatalogKind;
            if entry.path.is_none()
                || matches!(entry.kind, CatalogKind::Deleted | CatalogKind::Manufacturer)
            {
                continue;
            }
            let Ok(cat) = lib.open(idx) else { continue };
            let Ok(objects) = cat.objects() else { continue };
            for o in objects {
                let door = category_has(&o.category_path, DOOR_CATEGORY);
                let drawer = category_has(&o.category_path, DRAWER_CATEGORY);
                if !door && !drawer {
                    continue;
                }
                let (profile, glass) = look_of(&o.name, &o.keywords);
                found.push(LibraryStyle {
                    id: format!(
                        "{}{}.{}",
                        super::chief::ID_PREFIX,
                        cat.id(),
                        o.library_object_id
                    ),
                    name: o.name.clone(),
                    source: entry.name.clone(),
                    profile,
                    glass,
                    drawer_only: drawer && !door,
                });
            }
        }
    }
    found.sort_by_key(|s| s.name.to_lowercase());
    let n = found.len();
    CHIEF.with(|c| *c.borrow_mut() = Some(found));
    n
}

/// Forgets the scanned Chief styles (the install folder changed).
pub fn forget_chief() {
    CHIEF.with(|c| *c.borrow_mut() = None);
}

/// Whether the Chief catalogs were scanned in this session.
pub fn chief_scanned() -> bool {
    CHIEF.with(|c| c.borrow().is_some())
}

/// Every library style on offer: the built-in and user catalogs, then the
/// scanned Chief ones.
pub fn available() -> Vec<LibraryStyle> {
    let mut out = from_items(super::library_catalog().all_items(), "Library");
    out.extend(from_items(super::user::items().iter(), "User Library"));
    CHIEF.with(|c| {
        if let Some(chief) = c.borrow().as_ref() {
            out.extend(chief.iter().cloned());
        }
    });
    out
}

/// The door styles among `styles` (drawer-only ones left out).
pub fn doors(styles: &[LibraryStyle]) -> Vec<&LibraryStyle> {
    styles.iter().filter(|s| !s.drawer_only).collect()
}

/// Makes `door` the library style `s`: its name, profile, glass and id.
pub fn apply_door(s: &LibraryStyle, door: &mut DoorStyle) {
    door.name = s.name.clone();
    door.library = s.id.clone();
    door.profile = s.profile;
    door.glass = s.glass;
}

/// Makes `drawer` the library style `s`: its name, profile and id.
pub fn apply_drawer(s: &LibraryStyle, drawer: &mut DrawerStyle) {
    drawer.name = s.name.clone();
    drawer.library = s.id.clone();
    drawer.profile = s.profile;
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::{Placement, Symbol2d};

    fn item(id: &str, name: &str, category: &[&str], tags: &[&str]) -> CatalogItem {
        let mut i = CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default())
            .with_category(category);
        i.tags = tags.iter().map(|t| (*t).to_string()).collect();
        i
    }

    #[test]
    fn the_name_and_keywords_give_the_panel_look() {
        assert_eq!(look_of("Shaker Door", &[]), (DoorProfile::Shaker, false));
        assert_eq!(
            look_of("Cathedral Raised Panel", &[]),
            (DoorProfile::Raised, false)
        );
        assert_eq!(
            look_of("Lincoln Flat Panel", &["glass".into()]),
            (DoorProfile::Slab, true)
        );
        assert_eq!(
            look_of("Mullion Door", &["shaker".into()]),
            (DoorProfile::Shaker, true)
        );
    }

    #[test]
    fn only_the_cabinet_door_and_drawer_categories_are_styles() {
        let items = vec![
            item("a", "Shaker Door", &["Architectural", "Cabinet Doors"], &[]),
            item("b", "Slab Door", &["Cabinet Doors", "Flat"], &[]),
            item("c", "Plain Drawer", &["Cabinet Drawers"], &[]),
            item("d", "Toilet", &["Architectural", "Plumbing"], &[]),
        ];
        let s = from_items(&items, "Test");
        let names: Vec<&str> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Plain Drawer", "Shaker Door", "Slab Door"]);
        assert!(s[0].drawer_only);
        assert_eq!(doors(&s).len(), 2);
        assert_eq!(s[1].profile, DoorProfile::Shaker);
    }

    #[test]
    fn picking_a_library_style_copies_its_look_and_id() {
        let items = vec![item(
            "chief.x.9",
            "Glass Shaker Door",
            &["Cabinet Doors"],
            &[],
        )];
        let s = &from_items(&items, "Core")[0];
        let mut door = DoorStyle::default();
        apply_door(s, &mut door);
        assert_eq!(door.name, "Glass Shaker Door");
        assert_eq!(door.library, "chief.x.9");
        assert_eq!(door.profile, DoorProfile::Shaker);
        assert!(door.glass);
        let mut drawer = DrawerStyle::default();
        apply_drawer(s, &mut drawer);
        assert_eq!(drawer.library, "chief.x.9");
        assert_eq!(drawer.profile, DoorProfile::Shaker);
    }

    #[test]
    fn scanning_without_chief_finds_nothing_and_is_remembered() {
        forget_chief();
        assert!(!chief_scanned());
        assert_eq!(scan_chief(), 0);
        assert!(chief_scanned());
        forget_chief();
        // Whatever the built-in catalogs offer, available() does not panic.
        let _ = available();
    }
}
