//! The plant list of the Plant tools: the built-in Plants catalog of
//! `plan-library`, read once. A plant run stores the catalog id, the canopy
//! width and the height; the plan draws the canopy and the 3D view a tree or
//! shrub from them.

use plan_terrain::Landscape;
use std::sync::OnceLock;

/// One plant of the library.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantChoice {
    pub id: String,
    pub name: String,
    /// Mature canopy width, inches.
    pub width: f64,
    /// Mature height, inches.
    pub height: f64,
}

/// Catalog id of the plant a new run starts with.
const DEFAULT_PLANT_ID: &str = "core.plants.boxwood_2ft";

/// Every plant of the built-in Plants catalog.
pub fn plant_choices() -> &'static [PlantChoice] {
    static PLANTS: OnceLock<Vec<PlantChoice>> = OnceLock::new();
    PLANTS.get_or_init(|| {
        plan_library::catalog_plants::catalog()
            .items
            .into_iter()
            .map(|i| PlantChoice {
                id: i.id,
                name: i.name,
                width: i.width,
                height: i.height,
            })
            .collect()
    })
}

/// The boxwood shrub, or the first plant of the catalog, or a plain 3' shrub
/// when the catalog is empty.
pub fn default_plant() -> PlantChoice {
    let all = plant_choices();
    all.iter()
        .find(|p| p.id == DEFAULT_PLANT_ID)
        .or(all.first())
        .cloned()
        .unwrap_or(PlantChoice {
            id: String::new(),
            name: "Shrub".into(),
            width: 36.0,
            height: 36.0,
        })
}

/// Sets a run's plant: its id, canopy width, height and a spacing of one canopy.
pub fn apply_plant(obj: &mut Landscape, plant: &PlantChoice) {
    obj.plant = plant.id.clone();
    obj.size = plant.width.max(6.0);
    obj.height = plant.height.max(6.0);
    obj.spacing = obj.size;
}
