//! plan-layout: Chief's Layout, headless.
//!
//! A [`Layout`] is a set of sheets ([`LayoutPage`]); each page holds
//! [`LayoutBox`]es that show a plan view, elevation, section, schedule, CAD
//! detail, image or text at a drawing scale, plus paper-space CAD and a
//! title block ([`TitleBlockTemplate`]) whose fields use text macros such as
//! `%sheet.number%`.
//!
//! * [`send_to_layout`] places a new box for a source at a scale, packed into
//!   the first free area of the page; [`send_to_layout_auto`] picks the scale.
//! * [`default_construction_set`] builds Daniel's sheet set on 18x24.
//! * [`render_pdf`] prints every page through `plan-docs`' PDF writer: PDF
//!   clip rectangles per box, layer colours, weights and dashes, Chief's page
//!   background and Layout Edge border, bold and rotated text, embedded images
//!   and material hatches in elevations.
//!
//! Units: plan and drawing space are inches of the building; layout space
//! (box rectangles, page CAD) is inches of paper with the origin at the
//! sheet's bottom-left. A box at scale `s` draws a plan inch as
//! `72 * s.inches_per_foot() / 12` PDF points.

mod canvas;
mod clip;
mod extent;
mod hatch;
mod model;
mod render;
mod send;
mod titleblock;

pub use hatch::{pattern_for, wall_face_hatch, HatchStroke, MAX_HATCH_STROKES};
pub use model::{
    BoxSource, Layout, LayoutBox, LayoutPage, ScaleExt, ScheduleKind, LABEL_GAP_IN,
    LAYOUT_EDGE_WEIGHT,
};
pub use render::{render_box_lines, render_pdf, LayoutRenderContext};
pub use send::{
    default_construction_set, fit_largest_scale, send_to_layout, send_to_layout_auto,
    AUTO_SCALE_CEILING,
};
pub use titleblock::{
    long_date, MacroContext, TitleBlockStyle, TitleBlockTemplate, DANIEL_REVISION_ROWS,
};

pub use extent::source_size_in;

#[cfg(test)]
mod tests;
