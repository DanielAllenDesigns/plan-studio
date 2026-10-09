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

mod annot;
mod arrange;
mod boxops;
mod boxview;
mod cadattrs;
mod canvas;
mod clip;
mod extent;
mod hatch;
mod layers;
mod model;
mod overlay;
mod pages;
mod print;
mod render;
mod send;
mod template;
mod textfit;
mod titleblock;

pub use annot::{
    arc_from_points, cloud_outline, AnnotationKind, PageLeader, RevisionCloud, CLOUD_BUMP_IN,
    LEADER_TEXT_IN, MIN_CLOUD_IN,
};
pub use arrange::{align_boxes, copy_boxes, distribute_boxes, AlignEdge, Spread, COPY_OFFSET_IN};
pub use hatch::{pattern_for, wall_face_hatch, HatchStroke, MAX_HATCH_STROKES};
pub use layers::{
    LayoutLayer, LayoutLayers, LAYER_BOX_BORDERS, LAYER_CAD, LAYER_REVISION_CLOUDS, LAYER_TEXT,
    LAYER_TITLE_BLOCK, MAX_WEIGHT_PT, MIN_WEIGHT_PT,
};
pub use model::{
    perspective_pixels, BoxSource, CustomSheetSize, Layout, LayoutBox, LayoutPage, ScaleExt,
    ScheduleKind, SheetChoice, TextAlign, DANIEL_SIZES, DEFAULT_PERSPECTIVE_DPI,
    DEFAULT_PERSPECTIVE_SAMPLES, LABEL_GAP_IN, LAYOUT_EDGE_WEIGHT, MAX_PERSPECTIVE_PIXELS,
    MAX_PERSPECTIVE_SIDE_PX, MAX_SHEET_SIDE_IN, MAX_SNAP_UNIT_IN, MIN_SHEET_SIDE_IN,
    MIN_SNAP_UNIT_IN, DEFAULT_SNAP_UNIT_IN,
};
pub use pages::{
    resolve_labels, revision_rows, PageInfo, PageNumbers, PageRevision, PAGE_TABLE_COLUMNS,
    REVISION_TABLE_COLUMNS,
};
pub use print::{
    adopt_layout_sheet_sizes, check_plot_label, drawing_sheet_window, extent_center,
    global_custom_papers, global_sheet_sizes, layout_print_preview, paper_for_check_plot,
    plan_extent, plan_print_scale, plan_view_image, plan_view_print_preview, print_info,
    print_layout_pdf, print_model_pdf, print_plan_view_pdf, print_scale_factor, rasterize_lines,
    scale_to_fit, set_global_sheet_sizes, tile_grid, use_sheet_sizes, window_of,
    with_perspective_quality, PaperSize, PenSetup, PreviewItem, PreviewMark, PreviewMarkKind,
    PreviewPage, PrintColor, PrintOptions, PrintScale, SheetSizeFile, TileGrid,
    CHECK_PLOT_FRACTIONS, DEFAULT_FIT_PERCENT,
};
pub use render::{
    macros_for, perspective_request, render_box_artwork, render_box_artwork_in, render_box_lines,
    render_pdf, BoxArtwork, BoxImage, BoxText, CameraDrawingFn, LayoutRenderContext, PerspectiveFn,
    PerspectiveImage, PerspectiveRenderFn, PerspectiveRequest, PictureFn, SceneBuilderFn,
};
pub use send::{
    add_materials_page, append_construction_set, append_construction_set_in,
    default_construction_set, default_construction_set_with, fit_largest_scale, plan_label,
    send_camera_to_layout, send_to_layout, send_to_layout_auto, send_to_layout_sized,
    AUTO_SCALE_CEILING,
};
pub use template::{template_file_stem, LayoutTemplate, TEMPLATE_EXTENSION, TEMPLATE_VERSION};
pub use textfit::{fit_text_box, wrap_lines, FittedText, TextFit, MIN_SHRINK_PT};
pub use titleblock::{
    long_date, MacroContext, PageMacros, TitleBlockStyle, TitleBlockTemplate, DANIEL_REVISION_ROWS,
};

pub use extent::{box_table, source_size_in};
pub use overlay::{OverlayShape, PlanOverlayFn, PlanOverlayItem};

#[cfg(test)]
mod drawing_group_tests;
#[cfg(test)]
mod feature_tests;
#[cfg(test)]
mod font_tests;
#[cfg(test)]
mod overlay_tests;
#[cfg(test)]
mod page_tools_tests;
#[cfg(test)]
mod round14_tests;
#[cfg(test)]
mod tests;
