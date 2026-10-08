//! Icon lookup: every toolbar icon is an SVG embedded at compile time.

use eframe::egui;

/// Registers the image loaders (SVG) once at startup.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

/// Returns the embedded SVG for an icon id, falling back to `select`.
pub fn icon(id: &str) -> egui::ImageSource<'static> {
    match id {
        "add_lights" => egui::include_image!("../assets/icons/add_lights.svg"),
        "adjust_material" => egui::include_image!("../assets/icons/adjust_material.svg"),
        "arc" => egui::include_image!("../assets/icons/arc.svg"),
        "arc_centers" => egui::include_image!("../assets/icons/arc_centers.svg"),
        "arrow_line" => egui::include_image!("../assets/icons/arrow_line.svg"),
        "auto_detail" => egui::include_image!("../assets/icons/auto_detail.svg"),
        "beam" => egui::include_image!("../assets/icons/beam.svg"),
        "box" => egui::include_image!("../assets/icons/box.svg"),
        "cabinet_base" => egui::include_image!("../assets/icons/cabinet_base.svg"),
        "cabinet_full" => egui::include_image!("../assets/icons/cabinet_full.svg"),
        "cabinet_wall" => egui::include_image!("../assets/icons/cabinet_wall.svg"),
        "cad_layer" => egui::include_image!("../assets/icons/cad_layer.svg"),
        "callout" => egui::include_image!("../assets/icons/callout.svg"),
        "camera_full" => egui::include_image!("../assets/icons/camera_full.svg"),
        "camera_orbit" => egui::include_image!("../assets/icons/camera_orbit.svg"),
        "circle" => egui::include_image!("../assets/icons/circle.svg"),
        "color" => egui::include_image!("../assets/icons/color.svg"),
        "config_default" => egui::include_image!("../assets/icons/config_default.svg"),
        "config_extended" => egui::include_image!("../assets/icons/config_extended.svg"),
        "config_space_planning" => {
            egui::include_image!("../assets/icons/config_space_planning.svg")
        }
        "connect_cad" => egui::include_image!("../assets/icons/connect_cad.svg"),
        "connect_electrical" => egui::include_image!("../assets/icons/connect_electrical.svg"),
        "corner_boards" => egui::include_image!("../assets/icons/corner_boards.svg"),
        "countertop" => egui::include_image!("../assets/icons/countertop.svg"),
        "cross_section" => egui::include_image!("../assets/icons/cross_section.svg"),
        "crosshairs" => egui::include_image!("../assets/icons/crosshairs.svg"),
        "cylinder" => egui::include_image!("../assets/icons/cylinder.svg"),
        "deck_edge" => egui::include_image!("../assets/icons/deck_edge.svg"),
        "deck_railing" => egui::include_image!("../assets/icons/deck_railing.svg"),
        "default_settings" => egui::include_image!("../assets/icons/default_settings.svg"),
        "delete_surface" => egui::include_image!("../assets/icons/delete_surface.svg"),
        "dim_angular" => egui::include_image!("../assets/icons/dim_angular.svg"),
        "dim_auto_exterior" => egui::include_image!("../assets/icons/dim_auto_exterior.svg"),
        "dim_auto_interior" => egui::include_image!("../assets/icons/dim_auto_interior.svg"),
        "dim_end_to_end" => egui::include_image!("../assets/icons/dim_end_to_end.svg"),
        "dim_interior" => egui::include_image!("../assets/icons/dim_interior.svg"),
        "dim_manual" => egui::include_image!("../assets/icons/dim_manual.svg"),
        "display_options" => egui::include_image!("../assets/icons/display_options.svg"),
        "door_barn" => egui::include_image!("../assets/icons/door_barn.svg"),
        "door_bifold" => egui::include_image!("../assets/icons/door_bifold.svg"),
        "door_garage" => egui::include_image!("../assets/icons/door_garage.svg"),
        "door_hinged" => egui::include_image!("../assets/icons/door_hinged.svg"),
        "door_pocket" => egui::include_image!("../assets/icons/door_pocket.svg"),
        "door_sliding" => egui::include_image!("../assets/icons/door_sliding.svg"),
        "doorway" => egui::include_image!("../assets/icons/doorway.svg"),
        "dormer" => egui::include_image!("../assets/icons/dormer.svg"),
        "drawing_sheet" => egui::include_image!("../assets/icons/drawing_sheet.svg"),
        "elevation_line" => egui::include_image!("../assets/icons/elevation_line.svg"),
        "ellipse" => egui::include_image!("../assets/icons/ellipse.svg"),
        "file_new" => egui::include_image!("../assets/icons/file_new.svg"),
        "file_open" => egui::include_image!("../assets/icons/file_open.svg"),
        "file_print" => egui::include_image!("../assets/icons/file_print.svg"),
        "file_save" => egui::include_image!("../assets/icons/file_save.svg"),
        "fill_building" => egui::include_image!("../assets/icons/fill_building.svg"),
        "fill_selected" => egui::include_image!("../assets/icons/fill_selected.svg"),
        "fill_window" => egui::include_image!("../assets/icons/fill_window.svg"),
        "floor_defaults" => egui::include_image!("../assets/icons/floor_defaults.svg"),
        "floor_delete" => egui::include_image!("../assets/icons/floor_delete.svg"),
        "floor_down" => egui::include_image!("../assets/icons/floor_down.svg"),
        "floor_insert" => egui::include_image!("../assets/icons/floor_insert.svg"),
        "floor_new" => egui::include_image!("../assets/icons/floor_new.svg"),
        "floor_up" => egui::include_image!("../assets/icons/floor_up.svg"),
        "foundation" => egui::include_image!("../assets/icons/foundation.svg"),
        "framing_general" => egui::include_image!("../assets/icons/framing_general.svg"),
        "gable_line" => egui::include_image!("../assets/icons/gable_line.svg"),
        "glass_railing" => egui::include_image!("../assets/icons/glass_railing.svg"),
        "help" => egui::include_image!("../assets/icons/help.svg"),
        "joist" => egui::include_image!("../assets/icons/joist.svg"),
        "landing" => egui::include_image!("../assets/icons/landing.svg"),
        "layer_display" => egui::include_image!("../assets/icons/layer_display.svg"),
        "leader_line" => egui::include_image!("../assets/icons/leader_line.svg"),
        "library_browser" => egui::include_image!("../assets/icons/library_browser.svg"),
        "light" => egui::include_image!("../assets/icons/light.svg"),
        "line" => egui::include_image!("../assets/icons/line.svg"),
        "line_weights" => egui::include_image!("../assets/icons/line_weights.svg"),
        "marker" => egui::include_image!("../assets/icons/marker.svg"),
        "material_editor" => egui::include_image!("../assets/icons/material_editor.svg"),
        "material_eyedropper" => egui::include_image!("../assets/icons/material_eyedropper.svg"),
        "material_painter" => egui::include_image!("../assets/icons/material_painter.svg"),
        "note" => egui::include_image!("../assets/icons/note.svg"),
        "object_eyedropper" => egui::include_image!("../assets/icons/object_eyedropper.svg"),
        "outlet_110" => egui::include_image!("../assets/icons/outlet_110.svg"),
        "outlet_220" => egui::include_image!("../assets/icons/outlet_220.svg"),
        "pan" => egui::include_image!("../assets/icons/pan.svg"),
        "partition" => egui::include_image!("../assets/icons/partition.svg"),
        "pass_through" => egui::include_image!("../assets/icons/pass_through.svg"),
        "paste_hold" => egui::include_image!("../assets/icons/paste_hold.svg"),
        "plan_database" => egui::include_image!("../assets/icons/plan_database.svg"),
        "point" => egui::include_image!("../assets/icons/point.svg"),
        "polygon" => egui::include_image!("../assets/icons/polygon.svg"),
        "polyline" => egui::include_image!("../assets/icons/polyline.svg"),
        "post" => egui::include_image!("../assets/icons/post.svg"),
        "preferences" => egui::include_image!("../assets/icons/preferences.svg"),
        "print_preview" => egui::include_image!("../assets/icons/print_preview.svg"),
        "project_browser" => egui::include_image!("../assets/icons/project_browser.svg"),
        "quoins" => egui::include_image!("../assets/icons/quoins.svg"),
        "rafter" => egui::include_image!("../assets/icons/rafter.svg"),
        "railing" => egui::include_image!("../assets/icons/railing.svg"),
        "railing_curved" => egui::include_image!("../assets/icons/railing_curved.svg"),
        "ramp" => egui::include_image!("../assets/icons/ramp.svg"),
        "rect_polyline" => egui::include_image!("../assets/icons/rect_polyline.svg"),
        "redo" => egui::include_image!("../assets/icons/redo.svg"),
        "reference_display" => egui::include_image!("../assets/icons/reference_display.svg"),
        "render_standard" => egui::include_image!("../assets/icons/render_standard.svg"),
        "revision_cloud" => egui::include_image!("../assets/icons/revision_cloud.svg"),
        "rich_text" => egui::include_image!("../assets/icons/rich_text.svg"),
        "road" => egui::include_image!("../assets/icons/road.svg"),
        "roof_build" => egui::include_image!("../assets/icons/roof_build.svg"),
        "roof_plane" => egui::include_image!("../assets/icons/roof_plane.svg"),
        "select" => egui::include_image!("../assets/icons/select.svg"),
        "send_to_layout" => egui::include_image!("../assets/icons/send_to_layout.svg"),
        "shelf" => egui::include_image!("../assets/icons/shelf.svg"),
        "skylight" => egui::include_image!("../assets/icons/skylight.svg"),
        "slab" => egui::include_image!("../assets/icons/slab.svg"),
        "soffit" => egui::include_image!("../assets/icons/soffit.svg"),
        "solid_3d" => egui::include_image!("../assets/icons/solid_3d.svg"),
        "spline" => egui::include_image!("../assets/icons/spline.svg"),
        "stairs" => egui::include_image!("../assets/icons/stairs.svg"),
        "stairs_curved" => egui::include_image!("../assets/icons/stairs_curved.svg"),
        "sun_angle" => egui::include_image!("../assets/icons/sun_angle.svg"),
        "switch" => egui::include_image!("../assets/icons/switch.svg"),
        "temp_dimensions" => egui::include_image!("../assets/icons/temp_dimensions.svg"),
        "terrain" => egui::include_image!("../assets/icons/terrain.svg"),
        "text" => egui::include_image!("../assets/icons/text.svg"),
        "truss" => egui::include_image!("../assets/icons/truss.svg"),
        "undo" => egui::include_image!("../assets/icons/undo.svg"),
        "view_3d" => egui::include_image!("../assets/icons/view_3d.svg"),
        "view_edit" => egui::include_image!("../assets/icons/view_edit.svg"),
        "view_plan" => egui::include_image!("../assets/icons/view_plan.svg"),
        "view_save" => egui::include_image!("../assets/icons/view_save.svg"),
        "view_save_as" => egui::include_image!("../assets/icons/view_save_as.svg"),
        "walkthrough" => egui::include_image!("../assets/icons/walkthrough.svg"),
        "wall_attic" => egui::include_image!("../assets/icons/wall_attic.svg"),
        "wall_break" => egui::include_image!("../assets/icons/wall_break.svg"),
        "wall_curved" => egui::include_image!("../assets/icons/wall_curved.svg"),
        "wall_curved_interior" => egui::include_image!("../assets/icons/wall_curved_interior.svg"),
        "wall_exterior" => egui::include_image!("../assets/icons/wall_exterior.svg"),
        "wall_fix" => egui::include_image!("../assets/icons/wall_fix.svg"),
        "wall_foundation" => egui::include_image!("../assets/icons/wall_foundation.svg"),
        "wall_half" => egui::include_image!("../assets/icons/wall_half.svg"),
        "wall_hatch" => egui::include_image!("../assets/icons/wall_hatch.svg"),
        "wall_interior" => egui::include_image!("../assets/icons/wall_interior.svg"),
        "wall_pony" => egui::include_image!("../assets/icons/wall_pony.svg"),
        "wall_room_divider" => egui::include_image!("../assets/icons/wall_room_divider.svg"),
        "window" => egui::include_image!("../assets/icons/window.svg"),
        "window_bay" => egui::include_image!("../assets/icons/window_bay.svg"),
        "window_bow" => egui::include_image!("../assets/icons/window_bow.svg"),
        "window_box" => egui::include_image!("../assets/icons/window_box.svg"),
        "zoom" => egui::include_image!("../assets/icons/zoom.svg"),
        "zoom_in" => egui::include_image!("../assets/icons/zoom_in.svg"),
        "zoom_out" => egui::include_image!("../assets/icons/zoom_out.svg"),
        "zoom_undo" => egui::include_image!("../assets/icons/zoom_undo.svg"),
        _ => egui::include_image!("../assets/icons/select.svg"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn icon_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons")
    }

    fn svgs() -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = std::fs::read_dir(icon_dir())
            .expect("assets/icons exists")
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "svg"))
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(e.path()).unwrap(),
                )
            })
            .collect();
        out.sort();
        out
    }

    /// Every `#RRGGBB` color written in the SVG, upper-cased.
    fn colors(svg: &str) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        let b = svg.as_bytes();
        for (i, _) in svg.match_indices('#') {
            if let Some(hex) = svg.get(i + 1..i + 7) {
                if hex.bytes().all(|c| c.is_ascii_hexdigit())
                    && !b.get(i + 7).is_some_and(u8::is_ascii_hexdigit)
                {
                    let v = |k: usize| u8::from_str_radix(&hex[k..k + 2], 16).unwrap();
                    out.push([v(0), v(2), v(4)]);
                }
            }
        }
        out
    }

    /// A colored glyph: some ink has a clear hue (channel spread above 40).
    fn has_color_ink(svg: &str) -> bool {
        colors(svg)
            .iter()
            .any(|c| c.iter().max().unwrap() - c.iter().min().unwrap() > 40)
    }

    /// The halo marker: a white stroke with a stroke-opacity, drawn under (or
    /// as an edge around) the glyph's own strokes.
    fn has_halo(svg: &str) -> bool {
        svg.split('<')
            .any(|el| el.contains("stroke=\"#FFFFFF\"") && el.contains("stroke-opacity"))
    }

    /// White and light-gray glyphs are their own halo on the dark chrome; any
    /// icon with colored ink must carry the white halo stroke.
    #[test]
    fn every_colored_icon_has_the_white_halo() {
        let offenders: Vec<String> = svgs()
            .into_iter()
            .filter(|(_, s)| has_color_ink(s) && !has_halo(s))
            .map(|(n, _)| n)
            .collect();
        assert!(
            offenders.is_empty(),
            "icons with colored ink and no white halo stroke: {offenders:?}"
        );
    }

    #[test]
    fn every_icon_file_is_valid_enough_and_square() {
        let all = svgs();
        assert!(all.len() >= 150, "{} icons", all.len());
        for (name, svg) in all {
            assert!(svg.starts_with("<svg "), "{name}");
            assert!(svg.contains("viewBox=\"0 0 24 24\""), "{name} is not 24x24");
            assert!(svg.trim_end().ends_with("</svg>"), "{name}");
            // Balanced group tags.
            assert_eq!(
                svg.matches("<g").count(),
                svg.matches("</g>").count(),
                "{name}"
            );
        }
    }

    /// Every file has a lookup entry and every entry a file.
    #[test]
    fn every_icon_file_is_registered_in_the_lookup() {
        let src = include_str!("icons.rs");
        for (name, _) in svgs() {
            let stem = name.trim_end_matches(".svg");
            assert!(
                src.contains(&format!("\"{stem}\" =>")),
                "{name} is not in icons::icon"
            );
        }
    }
}
