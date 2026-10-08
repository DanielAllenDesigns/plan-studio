//! plan-layout: Chief's Layout, headless.
//!
//! A [`Layout`] is a set of sheets ([`LayoutPage`]); each page holds
//! [`LayoutBox`]es that show a plan view, elevation, section, schedule, CAD
//! detail, image or text at a drawing scale, plus paper-space CAD and a
//! title block ([`TitleBlockTemplate`]) whose fields use text macros such as
//! `%sheet.number%`.
//!
//! * [`send_to_layout`] places a new box for a source at a scale, packed into
//!   the first free area of the page.
//! * [`default_construction_set`] builds Chief's usual sheet set on 18x24.
//! * [`render_pdf`] prints every page through `plan-docs`' PDF writer.
//!
//! Units: plan and drawing space are inches of the building; layout space
//! (box rectangles, page CAD) is inches of paper with the origin at the
//! sheet's bottom-left. A box at scale `s` draws a plan inch as
//! `72 * s.inches_per_foot() / 12` PDF points.

mod clip;
mod extent;
mod model;
mod render;
mod send;
mod titleblock;

pub use model::{BoxSource, Layout, LayoutBox, LayoutPage, ScaleExt, ScheduleKind, LABEL_GAP_IN};
pub use render::{render_box_lines, render_pdf, LayoutRenderContext};
pub use send::{default_construction_set, send_to_layout};
pub use titleblock::{MacroContext, TitleBlockStyle, TitleBlockTemplate};

pub use extent::source_size_in;

#[cfg(test)]
mod tests;
