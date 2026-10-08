//! Default Settings > Camera Tools: the starting values of each camera tool
//! (Full Camera, Floor Camera, Doll House, Glass House, Overview, Elevation,
//! Section, Backclipped, Wall Elevation). The Full Camera page edits the
//! shared eye height and angle of view (`view3d_panel::camera_defaults`, the
//! same values as 3D View Defaults); the others are stored with the defaults
//! (`camera.<tool>.<field>`).

use super::page::{Bind, Field as F, PageSpec};
use plan_core::defaults::PageValue;

/// The camera tool pages in tree order: `(slug, title)`.
pub const LEAVES: &[(&str, &str)] = &[
    ("full", "Full Camera"),
    ("floor", "Floor Camera"),
    ("doll_house", "Doll House"),
    ("glass_house", "Glass House"),
    ("overview", "Overview"),
    ("elevation", "Elevation"),
    ("section", "Section"),
    ("backclipped", "Backclipped"),
    ("wall_elevation", "Wall Elevation"),
];

const TECHNIQUES: &[&str] = &[
    "Standard",
    "Vector View",
    "Watercolor",
    "Technical Illustration",
    "Sketch",
    "Glass House",
];

fn eye_height() -> Bind {
    Bind {
        get: |_| PageValue::Num(crate::shell::view3d_panel::camera_defaults().eye_height),
        set: |_, v| {
            let mut c = crate::shell::view3d_panel::camera_defaults();
            c.eye_height = v.num().max(1.0);
            crate::shell::view3d_panel::set_camera_defaults(c);
        },
    }
}

fn angle_of_view() -> Bind {
    Bind {
        get: |_| PageValue::Num(crate::shell::view3d_panel::camera_defaults().fov_deg),
        set: |_, v| {
            let mut c = crate::shell::view3d_panel::camera_defaults();
            c.fov_deg = v.num().clamp(5.0, 170.0);
            crate::shell::view3d_panel::set_camera_defaults(c);
        },
    }
}

/// The page of camera tool `slug`.
pub fn page(slug: &str) -> Option<PageSpec> {
    let (_, title) = LEAVES.iter().find(|(s, _)| *s == slug)?;
    let id = format!("camera.{slug}");
    let p = id.as_str();
    let spec = PageSpec::new(p, title);
    let technique = F::pick(&format!("{p}.technique"), "Rendering Technique", TECHNIQUES, "Standard");
    Some(match slug {
        "full" => spec
            .note("Eye height and angle of view are shared with 3D View Defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.eye_height"), "Eye Height", 66.0).bound(eye_height()),
                    F::deg(&format!("{p}.fov"), "Angle of View", 60.0).bound(angle_of_view()),
                    F::deg(&format!("{p}.tilt"), "Tilt", 0.0),
                    technique,
                ],
            ),
        "floor" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.height"), "Height Above Floor", 66.0),
                    F::deg(&format!("{p}.fov"), "Angle of View", 60.0),
                    technique,
                ],
            ),
        "doll_house" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::deg(&format!("{p}.angle"), "Viewing Angle", 45.0),
                    F::deg(&format!("{p}.rotation"), "Rotation", 45.0),
                    F::flag(&format!("{p}.cutaway"), "Remove Roof and Ceilings", true),
                    technique,
                ],
            ),
        "glass_house" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::int(&format!("{p}.transparency"), "Wall Transparency", 70, (0, 100), "%"),
                    F::flag(&format!("{p}.show_framing"), "Show Framing", false),
                    technique,
                ],
            ),
        "overview" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::deg(&format!("{p}.angle"), "Viewing Angle", 35.0),
                    F::deg(&format!("{p}.fov"), "Angle of View", 45.0),
                    F::flag(&format!("{p}.show_terrain"), "Show Terrain", true),
                    technique,
                ],
            ),
        "elevation" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.depth"), "Camera Depth", 0.0),
                    F::flag(&format!("{p}.include_terrain"), "Include Terrain", true),
                    F::flag(&format!("{p}.orthographic"), "Orthographic", true),
                    technique,
                ],
            ),
        "section" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.depth"), "Section Depth", 120.0),
                    F::flag(&format!("{p}.cut_fill"), "Fill the Cut Surfaces", true),
                    F::flag(&format!("{p}.orthographic"), "Orthographic", true),
                    technique,
                ],
            ),
        "backclipped" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.clip_distance"), "Back Clip Distance", 120.0),
                    F::flag(&format!("{p}.orthographic"), "Orthographic", true),
                    technique,
                ],
            ),
        "wall_elevation" => spec
            .note("Saved with the plan defaults.")
            .section(
                "Camera",
                vec![
                    F::len(&format!("{p}.offset"), "Distance From the Wall", 48.0),
                    F::flag(&format!("{p}.show_cabinets"), "Show Cabinets and Fixtures", true),
                    F::flag(&format!("{p}.show_dimensions"), "Show Dimensions", false),
                    technique,
                ],
            ),
        _ => return None,
    })
}
