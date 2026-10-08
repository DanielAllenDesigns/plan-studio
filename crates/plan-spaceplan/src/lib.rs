//! plan-spaceplan: Chief's Space Planning Assistant.
//!
//! The user answers a short [`Questionnaire`]; [`generate_boxes`] produces
//! colored, named *room boxes* already arranged into a plausible blob. The user
//! drags boxes around ([`bump`] snaps them to their neighbors), checks the
//! layout with [`validate`], and finally [`build_house`] turns the boxes into
//! walls, doors and windows in a `plan_core::Project`.
//!
//! All lengths are inches; box corners live on a 6" grid.

pub mod arrange;
pub mod boxes;
pub mod build;
pub mod questionnaire;
pub mod symbols;

pub use boxes::{bump, validate, Issue, Rect, RoomBox, GRID};
pub use build::{build_house, BuildOptions, BuildReport};
pub use questionnaire::{generate_boxes, Questionnaire};
pub use symbols::{plan_symbols, Stroke};
