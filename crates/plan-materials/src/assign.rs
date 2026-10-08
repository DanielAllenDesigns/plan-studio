//! Default material assignments per object component (Chief's Components tab).

use serde::{Deserialize, Serialize};

/// Maps one named component of an object to a material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialAssignment {
    /// Component name as shown on the Components tab, e.g. `"Door Panel"`.
    pub component: String,
    /// Name of a [`crate::MaterialDef`] in the library.
    pub material: String,
}

/// Default component-to-material table for an object kind.
///
/// Recognised kinds (case-insensitive): `Wall`, `Door`, `Window`, `Room`,
/// `Cabinet`, `Roof`. Unknown kinds give an empty list. Every material name
/// exists in [`crate::core_library`].
pub fn default_assignments_for(object_kind: &str) -> Vec<MaterialAssignment> {
    let table: &[(&str, &str)] = match object_kind.trim().to_ascii_lowercase().as_str() {
        "wall" => &[
            ("Exterior Wall Surface", "Sand Finish – Eggshell"),
            ("Interior Wall Surface", "Drywall"),
            ("Sill Plate", "Fir Framing"),
        ],
        "door" => &[
            ("Door Panel", "Lincoln Door"),
            ("Casing", "Painted White Trim"),
            ("Jamb", "Painted White Trim"),
            ("Hardware", "Brushed Nickel"),
        ],
        "window" => &[
            ("Frame", "Painted White Trim"),
            ("Sash", "Painted White Trim"),
            ("Glass", "Clear Glass"),
            ("Casing", "Painted White Trim"),
            ("Sill", "Painted White Trim"),
        ],
        "room" => &[
            ("Floor Finish", "Oak Flooring"),
            ("Ceiling Finish", "Color – White"),
            ("Base Molding", "Painted White Trim"),
            ("Crown", "Painted White Trim"),
        ],
        "cabinet" => &[
            ("Box", "Plywood"),
            ("Door/Drawer", "Painted White Trim"),
            ("Countertop", "Quartz – White"),
            ("Hardware", "Brushed Nickel"),
        ],
        "roof" => &[
            ("Roof Surface", "Architectural Shingles – Weathered Wood"),
            ("Fascia", "Painted White Trim"),
            ("Soffit", "Painted White Trim"),
            ("Gutter", "Color – White"),
        ],
        _ => &[],
    };
    table
        .iter()
        .map(|(c, m)| MaterialAssignment {
            component: (*c).to_string(),
            material: (*m).to_string(),
        })
        .collect()
}
