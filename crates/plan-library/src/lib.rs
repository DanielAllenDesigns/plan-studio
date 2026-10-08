//! plan-library: the catalog system for Plan Studio.
//!
//! This crate is the equivalent of Chief Architect's Library Browser. It
//! provides:
//!
//! * a JSON **catalog format** ([`Catalog`], [`CatalogItem`], [`Symbol2d`]);
//! * an in-memory **index** ([`Library`]) with ranked text search and a
//!   nested category tree ([`CategoryNode`]);
//! * a built-in **starter catalog** ([`core_catalog`]) of parametric 2D plan
//!   symbols drawn from polylines, arcs and circles;
//! * four larger catalogs ([`all_core_catalogs`]): plants, bath & kitchen,
//!   lighting & electrical, and furniture & exterior.
//!
//! # Conventions
//!
//! * All lengths are **inches**, matching `plan-core`.
//! * A symbol is drawn in a local frame with X to the right and Y *into the
//!   room* (the same Y-up plan space as `plan_core::geometry::Point`).
//! * [`Placement::WallMounted`] items sit against a wall: the origin is the
//!   **back-center** of the item, so the back edge lies on `y = 0` and the
//!   symbol extends towards `+y`. All other placements are drawn about their
//!   **center**.
//! * 3D models are not implemented yet; [`CatalogItem::model3d`] reserves a
//!   field for a future glTF path.
//!
//! ```
//! use plan_library::{core_catalog, Library};
//!
//! let mut lib = Library::default();
//! lib.add(core_catalog());
//! let hits = lib.search("toilet");
//! assert_eq!(hits[0].id, "core.plumbing.toilet_elongated");
//! ```

mod catalog;
pub mod catalog_bath_kitchen;
pub mod catalog_furniture_exterior;
pub mod catalog_lighting_electrical;
pub mod catalog_plants;
mod library;
mod shapes;
mod starter;
mod symbol;
pub mod user;

pub use catalog::{Catalog, CatalogItem, Placement};
pub use library::{CategoryNode, Library};
pub use starter::core_catalog;
pub use symbol::{Bounds, Stroke, Symbol2d};

/// Every built-in catalog: the starter [`core_catalog`] followed by Plants,
/// Bath & Kitchen, Lighting & Electrical and Furniture & Exterior.
pub fn all_core_catalogs() -> Vec<Catalog> {
    vec![
        core_catalog(),
        catalog_plants::catalog(),
        catalog_bath_kitchen::catalog(),
        catalog_lighting_electrical::catalog(),
        catalog_furniture_exterior::catalog(),
    ]
}
