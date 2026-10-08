//! The user library: items the user made themselves, such as a picture saved
//! with Create Image Library.
//!
//! A picture item places an image object: it is a free-standing item whose
//! plan symbol is a framed box with a diagonal cross, and whose tags carry
//! the picture file as `image:<path>` ([`image_path`]).

use crate::{Catalog, CatalogItem, Placement, Stroke, Symbol2d};
use plan_core::geometry::Point;

/// Prefix of the tag that names a picture file.
pub const IMAGE_TAG_PREFIX: &str = "image:";
/// Name of the catalog the user's items are saved as.
pub const USER_CATALOG_NAME: &str = "User Library";
/// Category path of saved pictures.
pub const IMAGE_CATEGORY: [&str; 2] = ["User", "Images"];

/// A library item that places the picture at `path`, `width` x `depth`
/// inches (height 0: a flat picture).
pub fn image_item(
    id: impl Into<String>,
    name: impl Into<String>,
    path: &str,
    width: f64,
    depth: f64,
) -> CatalogItem {
    let (hw, hd) = (width * 0.5, depth * 0.5);
    let corners = [
        Point::new(-hw, -hd),
        Point::new(hw, -hd),
        Point::new(hw, hd),
        Point::new(-hw, hd),
    ];
    let symbol = Symbol2d::new(vec![
        Stroke::Polyline {
            points: corners.to_vec(),
            closed: true,
        },
        Stroke::Polyline {
            points: vec![corners[0], corners[2]],
            closed: false,
        },
        Stroke::Polyline {
            points: vec![corners[1], corners[3]],
            closed: false,
        },
    ]);
    let tag = format!("{IMAGE_TAG_PREFIX}{path}");
    CatalogItem::new(id, name, Placement::FreeStanding, symbol)
        .with_category(&IMAGE_CATEGORY)
        .with_size(width, depth, 0.0)
        .with_tags(&["image", "picture", &tag])
}

/// The picture file an item places, if it is a picture item.
pub fn image_path(item: &CatalogItem) -> Option<&str> {
    item.tags
        .iter()
        .find_map(|t| t.strip_prefix(IMAGE_TAG_PREFIX))
}

/// A catalog of the user's `items` (replacing items with the same id).
pub fn user_catalog(items: &[CatalogItem]) -> Catalog {
    let mut c = Catalog::new(USER_CATALOG_NAME);
    for it in items {
        c.items.retain(|x| x.id != it.id);
        c.items.push(it.clone());
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_items_carry_their_file_and_round_trip() {
        let it = image_item("user.image.a1", "Rug", "/pics/rug.png", 96.0, 60.0);
        assert_eq!(image_path(&it), Some("/pics/rug.png"));
        assert_eq!((it.width, it.depth, it.height), (96.0, 60.0, 0.0));
        assert_eq!(it.category, vec!["User", "Images"]);
        let cat = user_catalog(&[it.clone(), it.clone()]);
        assert_eq!(cat.items.len(), 1);
        let back = Catalog::from_json(&cat.to_json().unwrap()).unwrap();
        assert_eq!(back, cat);
        assert!(image_path(&crate::core_catalog().items[0]).is_none());
    }
}
