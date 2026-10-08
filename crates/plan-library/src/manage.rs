//! User Catalog management: folders, favorites, recently used items and the
//! item operations (rename, duplicate, move, delete) on a list of items.
//!
//! A *folder* is a category path starting with [`USER_ROOT`]
//! (`["User", "Kitchen", "Mine"]`); an item lives in the folder named by its
//! `category`. Folders that hold no item are remembered in
//! [`UserMeta::folders`]. The data is plain: the app keeps the items in
//! `user-library.json` and a [`UserMeta`] next to it.

use crate::catalog::CatalogItem;
use serde::{Deserialize, Serialize};

/// First element of every user folder path.
pub const USER_ROOT: &str = "User";
/// Most items kept in the recently-used list.
pub const RECENT_MAX: usize = 24;

/// Folders, favorites and recents of the user catalog.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UserMeta {
    /// Folders created by hand, including empty ones.
    #[serde(default)]
    pub folders: Vec<Vec<String>>,
    /// Favorite item ids, in the order they were starred.
    #[serde(default)]
    pub favorites: Vec<String>,
    /// Recently used item ids, most recent first.
    #[serde(default)]
    pub recent: Vec<String>,
}

/// Checks a single folder or item name.
pub fn valid_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("The name is empty".into());
    }
    if n.chars().any(|c| matches!(c, '/' | '\\' | '>' | '\0')) {
        return Err("A name cannot contain / \\ or >".into());
    }
    Ok(n.to_string())
}

fn is_user_path(path: &[String]) -> bool {
    path.first().map(String::as_str) == Some(USER_ROOT) && path.len() >= 2
}

impl UserMeta {
    /// Parses the meta file; anything unreadable gives an empty one.
    pub fn from_json(text: &str) -> UserMeta {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// Serializes to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Is `id` starred?
    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|f| f == id)
    }

    /// Stars or un-stars `id`; returns the new state.
    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        match self.favorites.iter().position(|f| f == id) {
            Some(i) => {
                self.favorites.remove(i);
                false
            }
            None => {
                self.favorites.push(id.to_string());
                true
            }
        }
    }

    /// Records a use of `id`: it moves to the front of the recent list.
    pub fn add_recent(&mut self, id: &str) {
        self.recent.retain(|r| r != id);
        self.recent.insert(0, id.to_string());
        self.recent.truncate(RECENT_MAX);
    }

    /// Forgets a deleted item everywhere.
    pub fn forget(&mut self, id: &str) {
        self.favorites.retain(|f| f != id);
        self.recent.retain(|r| r != id);
    }

    /// Creates the folder `path` (`["User", ...]`). Existing folders are
    /// not an error.
    pub fn create_folder(&mut self, path: &[String]) -> Result<(), String> {
        if !is_user_path(path) {
            return Err("Folders live under the User catalog".into());
        }
        for name in &path[1..] {
            valid_name(name)?;
        }
        if !self.folders.iter().any(|f| f == path) {
            self.folders.push(path.to_vec());
        }
        Ok(())
    }

    /// Every folder path: the hand-made ones, the folders items sit in and
    /// all their ancestors, sorted, without duplicates. `["User"]` itself is
    /// not listed.
    pub fn all_folders(&self, items: &[CatalogItem]) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        let mut add = |path: &[String]| {
            for n in 2..=path.len() {
                if !out.iter().any(|p| p.as_slice() == &path[..n]) {
                    out.push(path[..n].to_vec());
                }
            }
        };
        for f in &self.folders {
            if is_user_path(f) {
                add(f);
            }
        }
        for it in items {
            if is_user_path(&it.category) {
                add(&it.category);
            }
        }
        out.sort_by_key(|p| p.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>());
        out
    }

    /// Renames the last element of folder `path` to `new_name`, moving the
    /// items inside (and sub-folders). Returns how many items moved.
    pub fn rename_folder(
        &mut self,
        items: &mut [CatalogItem],
        path: &[String],
        new_name: &str,
    ) -> Result<usize, String> {
        if !is_user_path(path) {
            return Err("Only user folders can be renamed".into());
        }
        let new_name = valid_name(new_name)?;
        let mut target = path.to_vec();
        *target.last_mut().expect("path has two elements") = new_name;
        if target == path {
            return Ok(0);
        }
        if self.all_folders(items).contains(&target) {
            return Err("A folder with that name exists".into());
        }
        Ok(self.relocate(items, path, &target))
    }

    /// Moves folder `path` (with everything inside) to be a child of
    /// `new_parent`.
    pub fn move_folder(
        &mut self,
        items: &mut [CatalogItem],
        path: &[String],
        new_parent: &[String],
    ) -> Result<usize, String> {
        if !is_user_path(path) {
            return Err("Only user folders can be moved".into());
        }
        if new_parent.first().map(String::as_str) != Some(USER_ROOT) {
            return Err("Folders can only move inside the User catalog".into());
        }
        if new_parent.len() >= path.len() && new_parent[..path.len()] == *path {
            return Err("A folder cannot move into itself".into());
        }
        let mut target = new_parent.to_vec();
        target.push(path.last().expect("path has two elements").clone());
        if target == path {
            return Ok(0);
        }
        if self.all_folders(items).contains(&target) {
            return Err("A folder with that name is already there".into());
        }
        Ok(self.relocate(items, path, &target))
    }

    fn relocate(&mut self, items: &mut [CatalogItem], from: &[String], to: &[String]) -> usize {
        let remap = |p: &mut Vec<String>| -> bool {
            if p.len() >= from.len() && p[..from.len()] == *from {
                let mut np = to.to_vec();
                np.extend_from_slice(&p[from.len()..]);
                *p = np;
                true
            } else {
                false
            }
        };
        let mut moved = 0;
        for it in items.iter_mut() {
            if remap(&mut it.category) {
                moved += 1;
            }
        }
        for f in &mut self.folders {
            remap(f);
        }
        self.folders.dedup();
        moved
    }

    /// Deletes folder `path` with every item and sub-folder inside; returns
    /// the ids removed.
    pub fn delete_folder(&mut self, items: &mut Vec<CatalogItem>, path: &[String]) -> Vec<String> {
        if !is_user_path(path) {
            return Vec::new();
        }
        let inside = |p: &[String]| p.len() >= path.len() && p[..path.len()] == *path;
        let mut removed = Vec::new();
        items.retain(|it| {
            if inside(&it.category) {
                removed.push(it.id.clone());
                false
            } else {
                true
            }
        });
        self.folders.retain(|f| !inside(f));
        for id in &removed {
            self.forget(id);
        }
        removed
    }
}

/// A fresh item id under `prefix`: `prefix.1`, `prefix.2`, ... not used by
/// `items`.
pub fn unique_id(items: &[CatalogItem], prefix: &str) -> String {
    let mut n = items.len() + 1;
    loop {
        let id = format!("{prefix}.{n}");
        if !items.iter().any(|i| i.id == id) {
            return id;
        }
        n += 1;
    }
}

/// Renames item `id`.
pub fn rename_item(items: &mut [CatalogItem], id: &str, name: &str) -> Result<(), String> {
    let name = valid_name(name)?;
    let it = items
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or("No such item")?;
    it.name = name;
    Ok(())
}

/// Appends a copy of item `id` named "<name> copy" in the same folder and
/// returns the copy's id. The copy shares the original's model file.
pub fn duplicate_item(items: &mut Vec<CatalogItem>, id: &str) -> Option<String> {
    let src = items.iter().find(|i| i.id == id)?.clone();
    let prefix = src.id.rsplit_once('.').map_or("user.copy", |(p, _)| p);
    let new_id = unique_id(items, prefix);
    let mut copy = src;
    copy.id = new_id.clone();
    copy.name = format!("{} copy", copy.name);
    items.push(copy);
    Some(new_id)
}

/// Moves item `id` into folder `folder`; false for an unknown item or a path
/// outside the user catalog.
pub fn move_item(items: &mut [CatalogItem], id: &str, folder: &[String]) -> bool {
    if !is_user_path(folder) {
        return false;
    }
    match items.iter_mut().find(|i| i.id == id) {
        Some(it) => {
            it.category = folder.to_vec();
            true
        }
        None => false,
    }
}

/// Removes item `id`; returns it.
pub fn delete_item(items: &mut Vec<CatalogItem>, id: &str) -> Option<CatalogItem> {
    let i = items.iter().position(|i| i.id == id)?;
    Some(items.remove(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Placement, Symbol2d};

    fn item(id: &str, name: &str, cat: &[&str]) -> CatalogItem {
        CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::default()).with_category(cat)
    }

    fn path(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn folders_list_their_ancestors_and_empty_folders() {
        let mut meta = UserMeta::default();
        meta.create_folder(&path(&["User", "Empty"])).unwrap();
        let items = vec![item("user.a.1", "A", &["User", "Kitchen", "Mine"])];
        let all = meta.all_folders(&items);
        assert_eq!(
            all,
            vec![
                path(&["User", "Empty"]),
                path(&["User", "Kitchen"]),
                path(&["User", "Kitchen", "Mine"]),
            ]
        );
        assert!(meta.create_folder(&path(&["Other", "X"])).is_err());
        assert!(meta.create_folder(&path(&["User", "a/b"])).is_err());
        assert!(meta.create_folder(&path(&["User"])).is_err());
    }

    #[test]
    fn rename_move_and_delete_folders_carry_their_items() {
        let mut meta = UserMeta::default();
        meta.create_folder(&path(&["User", "Kitchen"])).unwrap();
        let mut items = vec![
            item("user.a.1", "A", &["User", "Kitchen"]),
            item("user.a.2", "B", &["User", "Kitchen", "Sub"]),
            item("user.a.3", "C", &["User", "Other"]),
        ];
        let n = meta
            .rename_folder(&mut items, &path(&["User", "Kitchen"]), "Cooking")
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(items[1].category, path(&["User", "Cooking", "Sub"]));
        assert_eq!(meta.folders, vec![path(&["User", "Cooking"])]);
        assert!(meta
            .rename_folder(&mut items, &path(&["User", "Cooking"]), "Other")
            .is_err());
        meta.move_folder(&mut items, &path(&["User", "Cooking"]), &path(&["User", "Other"]))
            .unwrap();
        assert_eq!(items[0].category, path(&["User", "Other", "Cooking"]));
        assert!(meta
            .move_folder(&mut items, &path(&["User", "Other"]), &path(&["User", "Other", "Cooking"]))
            .is_err());
        meta.toggle_favorite("user.a.2");
        let gone = meta.delete_folder(&mut items, &path(&["User", "Other"]));
        assert_eq!(gone.len(), 3);
        assert!(items.is_empty());
        assert!(meta.favorites.is_empty() && meta.folders.is_empty());
    }

    #[test]
    fn item_operations() {
        let mut items = vec![item("user.m.1", "Chair", &["User", "Furniture"])];
        rename_item(&mut items, "user.m.1", " Armchair ").unwrap();
        assert_eq!(items[0].name, "Armchair");
        assert!(rename_item(&mut items, "user.m.1", "  ").is_err());
        assert!(rename_item(&mut items, "nope", "X").is_err());
        let copy = duplicate_item(&mut items, "user.m.1").unwrap();
        assert_ne!(copy, "user.m.1");
        assert!(copy.starts_with("user.m."));
        assert_eq!(items[1].name, "Armchair copy");
        assert_eq!(items[1].category, items[0].category);
        assert!(move_item(&mut items, &copy, &path(&["User", "Moved"])));
        assert!(!move_item(&mut items, &copy, &path(&["Core", "X"])));
        assert_eq!(items[1].category, path(&["User", "Moved"]));
        assert_eq!(delete_item(&mut items, &copy).unwrap().id, copy);
        assert!(delete_item(&mut items, &copy).is_none());
    }

    #[test]
    fn favorites_and_recents_persist_through_json() {
        let mut meta = UserMeta::default();
        assert!(meta.toggle_favorite("a"));
        assert!(meta.toggle_favorite("b"));
        assert!(!meta.toggle_favorite("a"));
        for i in 0..30 {
            meta.add_recent(&format!("r{i}"));
        }
        meta.add_recent("r5");
        assert_eq!(meta.recent.len(), RECENT_MAX);
        assert_eq!(meta.recent[0], "r5");
        assert_eq!(meta.recent.iter().filter(|r| *r == "r5").count(), 1);
        meta.create_folder(&path(&["User", "F"])).unwrap();
        let back = UserMeta::from_json(&meta.to_json());
        assert_eq!(back, meta);
        assert!(back.is_favorite("b") && !back.is_favorite("a"));
        assert_eq!(UserMeta::from_json("garbage"), UserMeta::default());
    }
}
