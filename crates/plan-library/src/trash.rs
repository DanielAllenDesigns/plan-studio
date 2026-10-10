//! The User Catalog's Trash: Delete in the Library Browser moves an item here
//! instead of erasing it; Restore puts it back in its folder and Empty Trash
//! erases what is in it for good.
//!
//! The data is plain, like [`crate::manage`]: the app keeps a [`Trash`] in
//! `user-library-trash.json` next to the user library. A trashed item keeps
//! its 3D model file until the trash is emptied (see [`Trash::model_paths`]).

use crate::catalog::CatalogItem;
use crate::manage::{unique_id, USER_ROOT};
use serde::{Deserialize, Serialize};

/// Name of the Trash node at the top of the browser tree.
pub const TRASH_ROOT: &str = "Trash";

/// One trashed item and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrashEntry {
    /// The item as it was when deleted (its `category` is its old folder).
    pub item: CatalogItem,
    /// Seconds since the Unix epoch when it was deleted.
    pub deleted: u64,
}

/// The trashed items, oldest first.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Trash {
    #[serde(default)]
    pub entries: Vec<TrashEntry>,
}

impl Trash {
    /// Parses the saved trash; anything unreadable gives an empty one.
    pub fn from_json(text: &str) -> Trash {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The JSON the app saves.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Number of trashed items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The trashed items.
    pub fn items(&self) -> impl Iterator<Item = &CatalogItem> {
        self.entries.iter().map(|e| &e.item)
    }

    /// Whether `id` is in the trash.
    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.item.id == id)
    }

    /// Moves item `id` out of `items` into the trash; false for an unknown
    /// id. `now` is the time stamp (seconds since the epoch).
    pub fn discard(&mut self, items: &mut Vec<CatalogItem>, id: &str, now: u64) -> bool {
        let Some(pos) = items.iter().position(|i| i.id == id) else {
            return false;
        };
        let item = items.remove(pos);
        self.entries.push(TrashEntry { item, deleted: now });
        true
    }

    /// Puts item `id` back into `items`, in the folder it came from. When
    /// another item took its id meanwhile the restored copy gets a fresh one.
    /// Returns the id the item now has.
    pub fn restore(&mut self, items: &mut Vec<CatalogItem>, id: &str) -> Option<String> {
        let pos = self.entries.iter().position(|e| e.item.id == id)?;
        let mut item = self.entries.remove(pos).item;
        if items.iter().any(|i| i.id == item.id) {
            item.id = unique_id(items, "user.restored");
        }
        if item.category.first().map(String::as_str) != Some(USER_ROOT) {
            item.category = vec![USER_ROOT.to_string()];
        }
        let new_id = item.id.clone();
        items.push(item);
        Some(new_id)
    }

    /// Erases item `id` for good; returns it (so its model file can go).
    pub fn purge(&mut self, id: &str) -> Option<CatalogItem> {
        let pos = self.entries.iter().position(|e| e.item.id == id)?;
        Some(self.entries.remove(pos).item)
    }

    /// Erases everything; returns the items that were in the trash.
    pub fn empty(&mut self) -> Vec<CatalogItem> {
        self.entries.drain(..).map(|e| e.item).collect()
    }

    /// The model files trashed items still need.
    pub fn model_paths(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter_map(|e| e.item.model3d.as_deref())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Placement;
    use crate::symbol::Symbol2d;

    fn item(id: &str, folder: &[&str]) -> CatalogItem {
        CatalogItem::new(id, id, Placement::FreeStanding, Symbol2d::default()).with_category(folder)
    }

    #[test]
    fn discard_and_restore_keep_the_folder() {
        let mut items = vec![
            item("user.1", &["User", "Kitchen"]),
            item("user.2", &["User"]),
        ];
        let mut t = Trash::default();
        assert!(t.discard(&mut items, "user.1", 100));
        assert!(!t.discard(&mut items, "nope", 100));
        assert_eq!((items.len(), t.len()), (1, 1));
        assert!(t.contains("user.1"));
        let id = t.restore(&mut items, "user.1").unwrap();
        assert_eq!(id, "user.1");
        assert!(t.is_empty());
        assert_eq!(items.last().unwrap().category, ["User", "Kitchen"]);
    }

    #[test]
    fn a_taken_id_gets_a_fresh_one_and_odd_folders_go_to_the_root() {
        let mut items = vec![item("user.1", &["User"])];
        let mut t = Trash::default();
        t.entries.push(TrashEntry {
            item: item("user.1", &["Elsewhere"]),
            deleted: 5,
        });
        let id = t.restore(&mut items, "user.1").unwrap();
        assert_ne!(id, "user.1");
        let back = items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(back.category, ["User"]);
    }

    #[test]
    fn empty_and_purge_erase_and_report_the_models() {
        let mut items = vec![item("user.1", &["User"]), item("user.2", &["User"])];
        items[0].model3d = Some("user-models/a.psm".into());
        let mut t = Trash::default();
        t.discard(&mut items, "user.1", 1);
        t.discard(&mut items, "user.2", 2);
        assert_eq!(t.model_paths(), ["user-models/a.psm"]);
        assert_eq!(t.purge("user.2").unwrap().id, "user.2");
        assert!(t.purge("user.2").is_none());
        let gone = t.empty();
        assert_eq!(gone.len(), 1);
        assert!(t.is_empty());
    }

    #[test]
    fn json_round_trips_and_bad_text_is_empty() {
        let mut items = vec![item("user.1", &["User"])];
        let mut t = Trash::default();
        t.discard(&mut items, "user.1", 42);
        let back = Trash::from_json(&t.to_json());
        assert_eq!(back, t);
        assert!(Trash::from_json("{ nope").is_empty());
    }
}
