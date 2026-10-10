//! The plant list of the Plant tools: the built-in Plants catalog of
//! `plan-library`, read once. A plant run stores the catalog id, the canopy
//! width and the height; the plan draws the canopy and the 3D view a tree, a
//! cone or a shrub from them. The Plant Chooser of the Plant Specification
//! lists them by category ([`plant_categories`], [`plants_in`]).

use plan_terrain::{is_conifer, Landscape, PlantForm};
use std::sync::OnceLock;

/// One plant of the library.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantChoice {
    pub id: String,
    pub name: String,
    /// Category path under the Plants root, e.g. `Trees > Evergreen`.
    pub category: String,
    /// Mature canopy width, inches.
    pub width: f64,
    /// Mature height, inches.
    pub height: f64,
    /// Search words of the catalog entry.
    pub tags: Vec<String>,
    /// A conifer: built as a cone in 3D.
    pub conifer: bool,
}

impl PlantChoice {
    /// Does the plant match the words typed in the chooser's search field
    /// (every word must appear in the name, category or a tag)?
    pub fn matches(&self, search: &str) -> bool {
        let hay = format!("{} {} {}", self.name, self.category, self.tags.join(" ")).to_lowercase();
        search
            .split_whitespace()
            .all(|w| hay.contains(&w.to_lowercase()))
    }
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
            .map(|i| {
                let category = i
                    .category
                    .iter()
                    .skip(1)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" > ");
                PlantChoice {
                    conifer: is_conifer(&i.id) || is_conifer(&i.name),
                    id: i.id,
                    name: i.name,
                    category,
                    width: i.width,
                    height: i.height,
                    tags: i.tags,
                }
            })
            .collect()
    })
}

/// The categories of the Plants catalog, in catalog order, without repeats.
pub fn plant_categories() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in plant_choices() {
        if !p.category.is_empty() && !out.contains(&p.category) {
            out.push(p.category.clone());
        }
    }
    out
}

/// The plants of `category` (all when empty) that match the `search` words.
pub fn plants_in(category: &str, search: &str) -> Vec<&'static PlantChoice> {
    plant_choices()
        .iter()
        .filter(|p| category.is_empty() || p.category == category)
        .filter(|p| p.matches(search))
        .collect()
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
            category: String::new(),
            width: 36.0,
            height: 36.0,
            tags: Vec::new(),
            conifer: false,
        })
}

/// Sets a run's plant: its id, canopy width, height, a spacing of one canopy
/// and its 3D form (a cone for a conifer, else the automatic one).
pub fn apply_plant(obj: &mut Landscape, plant: &PlantChoice) {
    obj.plant = plant.id.clone();
    obj.size = plant.width.max(6.0);
    obj.height = plant.height.max(6.0);
    obj.spacing = obj.size;
    // The catalog size is the mature size: Grow All Plants scales from it.
    obj.mature_height = obj.height;
    obj.mature_width = obj.size;
    obj.maturity_months = plan_terrain::default_age_at_maturity(obj.height);
    obj.form = if plant.conifer {
        PlantForm::Cone
    } else {
        PlantForm::Auto
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chooser_lists_the_catalog_by_category_and_search() {
        let cats = plant_categories();
        assert!(cats.contains(&"Trees > Evergreen".to_string()), "{cats:?}");
        assert!(cats.iter().any(|c| c.starts_with("Shrubs")));
        let all = plants_in("", "");
        assert_eq!(all.len(), plant_choices().len());
        let evergreen = plants_in("Trees > Evergreen", "");
        assert!(!evergreen.is_empty() && evergreen.len() < all.len());
        assert!(evergreen.iter().all(|p| p.category == "Trees > Evergreen"));
        // Search words all have to match, by name, category or tag.
        let oaks = plants_in("", "oak");
        assert!(oaks.iter().any(|p| p.id == "core.plants.oak_20ft"));
        assert!(plants_in("Trees > Evergreen", "oak").is_empty());
        assert!(plants_in("", "shade tree")
            .iter()
            .all(|p| p.matches("tree")));
        assert!(plants_in("", "zzz nothing").is_empty());
    }

    #[test]
    fn applying_a_conifer_makes_it_a_cone_and_the_others_automatic() {
        let mut run = Landscape::new(
            plan_terrain::LandscapeKind::Plants,
            plan_terrain::ShapeKind::Polyline,
            vec![],
        );
        let pine = plant_choices()
            .iter()
            .find(|p| p.id == "core.plants.pine_20ft")
            .unwrap();
        assert!(pine.conifer);
        apply_plant(&mut run, pine);
        assert_eq!(run.form, PlantForm::Cone);
        assert_eq!(run.plant, pine.id);
        assert_eq!(run.size, pine.width);
        apply_plant(&mut run, &default_plant());
        assert_eq!(run.form, PlantForm::Auto);
        assert_eq!(run.plant_form(), PlantForm::Round);
    }
}
