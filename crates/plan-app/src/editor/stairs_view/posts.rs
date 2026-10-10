//! Newels and balusters from the library (CB-180, CB-181).
//!
//! A stair names a library item for its newels and one for its balusters
//! ([`StairParams::newel_item`], [`StairParams::baluster_item`]). When the item
//! has a 3D model, the 3D view leaves the stair's own post out and puts the
//! model at every place the stair would stand one: fitted to the post's size
//! and height, centred on the post. An item without a model (or an id that no
//! longer exists) falls back to the built-in post of the Newels/Balusters tab.
//!
//! The items offered are those of the library folders "Newels" and
//! "Balusters" (user-library models are filed there on import).

use super::*;
use crate::tools::library::{find_item, library_catalog, user};
use plan_3d::import::{fit_meshes_to_box, mesh_from_triangles, transform_mesh};
use plan_3d::Mesh;
use plan_library::CatalogItem;
use plan_stairs::{stair_posts, tagged_meshes_skipping, PostPlacement, PostSkip, StairPart};

/// Which post a library item is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PostKind {
    Newel,
    Baluster,
}

impl PostKind {
    /// The library folder the items live in.
    pub fn folder(self) -> &'static str {
        match self {
            PostKind::Newel => "Newels",
            PostKind::Baluster => "Balusters",
        }
    }
}

fn in_folder(item: &CatalogItem, kind: PostKind) -> bool {
    item.category
        .iter()
        .any(|c| c.eq_ignore_ascii_case(kind.folder()))
}

/// The library items for a newel or baluster: `(id, name)`, built-in catalogs
/// first, then the User Catalog.
pub fn library_posts(kind: PostKind) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = library_catalog()
        .all_items()
        .filter(|i| in_folder(i, kind))
        .map(|i| (i.id.clone(), i.name.clone()))
        .collect();
    out.extend(
        user::items()
            .iter()
            .filter(|i| in_folder(i, kind))
            .map(|i| (i.id.clone(), i.name.clone())),
    );
    out
}

/// The meshes of library item `id` fitted to a unit-high post of cross
/// section `size`: x centred, standing on y = 0, z from 0 to `size`. `None`
/// when the item is missing or has no model.
fn prototype(id: &str, size: f64) -> Option<Vec<Mesh>> {
    if id.is_empty() {
        return None;
    }
    let item = find_item(id)?;
    let model = user::model_of(&item)?;
    let model = if item.model_rotation != 0.0 {
        model.rotated_y(item.model_rotation)
    } else {
        (*model).clone()
    };
    let meshes: Vec<Mesh> = model
        .parts
        .iter()
        .map(|p| {
            mesh_from_triangles(
                &p.positions,
                None,
                &p.indices,
                plan_calib::mesh3d::material_for(p.color),
                None,
            )
        })
        .filter(|m| !m.indices.is_empty())
        .collect();
    if meshes.is_empty() {
        return None;
    }
    Some(fit_meshes_to_box(&meshes, size as f32, size as f32, 1.0))
}

/// The copies of `proto` standing at `places`, each as high as its place.
fn stand(proto: &[Mesh], places: &[PostPlacement], stair: u64) -> Vec<Mesh> {
    let mut out = Vec::new();
    for pl in places {
        let h = (pl.top - pl.foot).max(0.1) as f32;
        let origin = [
            pl.at.x as f32,
            pl.foot as f32,
            (-pl.at.y - pl.size * 0.5) as f32,
        ];
        for m in proto {
            let mut t = transform_mesh(m, origin, 0.0, [1.0, h, 1.0], false);
            t.object_id = Some(stair);
            out.push(t);
        }
    }
    out
}

/// The 3D parts of a stair with its library newels and balusters in place of
/// the built-in ones, at the floor elevation `floor_elevation`.
pub fn part_meshes(o: &StairObj, floor_elevation: f64) -> Vec<(StairPart, Mesh)> {
    let mut stair = o.stair.clone();
    stair.floor_elevation = floor_elevation;
    let p = &stair.params;
    let newel = prototype(&p.newel_item, p.railing.newel.size);
    let baluster = match p.railing.style {
        plan_stairs::RailStyle::Balusters { size, .. } => prototype(&p.baluster_item, size),
        _ => None,
    };
    let skip = PostSkip {
        newels: newel.is_some(),
        balusters: baluster.is_some(),
    };
    let mut out = tagged_meshes_skipping(&stair, skip);
    if newel.is_some() || baluster.is_some() {
        let posts = stair_posts(&stair);
        if let Some(proto) = &newel {
            out.extend(
                stand(proto, &posts.newels, stair.id)
                    .into_iter()
                    .map(|m| (StairPart::Handrail, m)),
            );
        }
        if let Some(proto) = &baluster {
            out.extend(
                stand(proto, &posts.balusters, stair.id)
                    .into_iter()
                    .map(|m| (StairPart::Handrail, m)),
            );
        }
    }
    out
}
