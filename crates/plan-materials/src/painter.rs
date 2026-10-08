//! Material Painter support: which 3D scene material stands for a library
//! material, and the Material Builder's validation.
//!
//! The 3D viewport shades each mesh with one of a fixed set of
//! [`plan_3d::Material`]s (it has no per-mesh colour), so painting an object
//! with a library material shows the scene material that is closest to it:
//! by what the material is (a brick, a shingle, a floor...) where the library
//! says, else by colour.

use plan_3d::Material;

use crate::library::MaterialLibrary;
use crate::material::{MaterialDef, ProceduralKind, Texture};
use crate::pattern::Pattern;

/// Scene materials a colour can match: the opaque ones that are not tints.
const COLOR_MATCHES: [Material; 17] = [
    Material::WallExterior,
    Material::WallInterior,
    Material::Floor,
    Material::Ceiling,
    Material::DoorPanel,
    Material::WindowFrame,
    Material::Roof,
    Material::Stucco,
    Material::Siding,
    Material::Brick,
    Material::Stone,
    Material::Concrete,
    Material::Trim,
    Material::Metal,
    Material::Framing,
    Material::Grass,
    Material::Asphalt,
];

/// The scene materials' base colors are written the way the library's are
/// (display values), so colors compare without a gamma step.
fn unit(c: u8) -> f32 {
    f32::from(c) / 255.0
}

/// The opaque scene material nearest to `rgb` (sRGB).
pub fn nearest_by_color(rgb: [u8; 3]) -> Material {
    let want = rgb.map(unit);
    COLOR_MATCHES
        .into_iter()
        .min_by(|a, b| {
            let d = |m: &Material| {
                let c = m.color();
                (0..3).map(|i| (c[i] - want[i]).powi(2)).sum::<f32>()
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(Material::WallExterior)
}

/// The scene material that stands for `def` in the 3D view.
pub fn scene_material(def: &MaterialDef) -> Material {
    if def.transparency >= 0.5 {
        return Material::Glass;
    }
    let kind = match &def.texture {
        Texture::Procedural(k) => Some(k),
        Texture::Solid => None,
    };
    let cat0 = def.category.first().map(String::as_str).unwrap_or("");
    let cat1 = def.category.get(1).map(String::as_str).unwrap_or("");
    let name = def.name.to_ascii_lowercase();
    match cat0 {
        "Roofing" => return Material::Roof,
        "Flooring" => return Material::Floor,
        "Glass" => return Material::Glass,
        "Metal" => return Material::Metal,
        "Doors" => return Material::DoorPanel,
        "Framing" => return Material::Framing,
        "Siding" => {
            return if matches!(kind, Some(ProceduralKind::Stucco { .. })) {
                Material::Stucco
            } else {
                Material::Siding
            }
        }
        "Masonry" => {
            return match kind {
                Some(ProceduralKind::Brick { .. }) => Material::Brick,
                Some(ProceduralKind::Stone) => Material::Stone,
                _ if cat1 == "Brick" => Material::Brick,
                _ if cat1 == "Stone" => Material::Stone,
                _ => Material::Concrete,
            }
        }
        "Countertop" => {
            return if cat1 == "Wood" {
                Material::Floor
            } else {
                Material::Stone
            }
        }
        "Site" => {
            return if name.contains("asphalt") {
                Material::Asphalt
            } else if name.contains("gravel") {
                Material::Gravel
            } else if matches!(kind, Some(ProceduralKind::Grass)) {
                Material::Grass
            } else {
                Material::Mulch
            }
        }
        "Paint" if cat1 == "Trim" => return Material::Trim,
        _ => {}
    }
    match kind {
        Some(ProceduralKind::Brick { .. }) => Material::Brick,
        Some(ProceduralKind::Stucco { .. }) => Material::Stucco,
        Some(ProceduralKind::Shingles) => Material::Roof,
        Some(ProceduralKind::LapSiding) => Material::Siding,
        Some(ProceduralKind::Stone) => Material::Stone,
        Some(ProceduralKind::Glass) => Material::Glass,
        Some(ProceduralKind::Metal) => Material::Metal,
        Some(ProceduralKind::Grass) => Material::Grass,
        Some(ProceduralKind::Tile { .. } | ProceduralKind::Carpet) => Material::Floor,
        _ => nearest_by_color(def.color),
    }
}

impl MaterialLibrary {
    /// This library with `user`'s materials added; a user material replaces
    /// the one of the same name.
    pub fn merged_with(&self, user: &MaterialLibrary) -> MaterialLibrary {
        let mut out = self.clone();
        for m in &user.materials {
            out.add(m.clone());
        }
        out
    }

    /// Removes the material called `name`; true when there was one.
    pub fn remove(&mut self, name: &str) -> bool {
        let n = self.materials.len();
        self.materials.retain(|m| m.name != name);
        self.materials.len() != n
    }
}

/// The Material Builder's result: a material of `name` in the category path
/// `category` ("A > B"), `color`, `roughness` (0 mirror .. 1 diffuse), a 2D
/// `pattern` and an optional `texture_path` (an image file). The texture
/// scale stays at the default 12 x 12 inch tile.
pub fn build_material(
    name: &str,
    category: &str,
    color: [u8; 3],
    roughness: f32,
    pattern: Pattern,
    texture_path: &str,
) -> Result<MaterialDef, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A material needs a name".to_string());
    }
    let cats: Vec<&str> = category
        .split('>')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let cats = if cats.is_empty() {
        vec!["Custom"]
    } else {
        cats
    };
    let mut def = MaterialDef::new(name, &cats, color)
        .with_surface(roughness.clamp(0.0, 1.0), 0.0)
        .with_pattern(pattern);
    let path = texture_path.trim();
    def.texture_path = (!path.is_empty()).then(|| path.to_string());
    Ok(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::core_library;

    #[test]
    fn library_materials_map_to_what_they_are() {
        let lib = core_library();
        let find = |n: &str| scene_material(lib.find(n).unwrap_or_else(|| panic!("{n}")));
        assert_eq!(find("Sand Finish – Eggshell"), Material::Stucco);
        assert_eq!(find("Quartz – White"), Material::Stone);
        assert_eq!(find("Painted White Trim"), Material::Trim);
        assert_eq!(find("Clear Glass"), Material::Glass);
        assert_eq!(find("Asphalt"), Material::Asphalt);
        assert_eq!(find("Gravel"), Material::Gravel);
        assert_eq!(find("Grass"), Material::Grass);
        for m in lib.materials.iter().filter(|m| m.category[0] == "Roofing") {
            assert_eq!(scene_material(m), Material::Roof, "{}", m.name);
        }
        for m in lib.materials.iter().filter(|m| m.category[0] == "Flooring") {
            assert_eq!(scene_material(m), Material::Floor, "{}", m.name);
        }
        // Every material maps to something drawable (never the selection tint).
        for m in &lib.materials {
            assert_ne!(scene_material(m), Material::Selection, "{}", m.name);
        }
    }

    #[test]
    fn plain_colors_pick_the_nearest_scene_color() {
        assert_eq!(nearest_by_color([158, 77, 56]), Material::Brick);
        assert!(matches!(
            nearest_by_color([250, 250, 247]),
            Material::Trim | Material::Ceiling
        ));
        assert_eq!(nearest_by_color([40, 40, 40]), Material::Asphalt);
        let red = MaterialDef::new("Barn Red", &["Custom"], [160, 75, 55]);
        assert_eq!(scene_material(&red), Material::Brick);
        let clear = MaterialDef::new("Pane", &["Custom"], [200, 220, 235]).with_transparency(0.8);
        assert_eq!(scene_material(&clear), Material::Glass);
    }

    #[test]
    fn merged_library_lets_the_user_win_and_remove_works() {
        let core = core_library();
        let mut user = MaterialLibrary::default();
        user.add(MaterialDef::new("Drywall", &["Custom"], [1, 2, 3]));
        user.add(MaterialDef::new("Mine", &["Custom"], [4, 5, 6]));
        let all = core.merged_with(&user);
        assert_eq!(all.find("Drywall").unwrap().color, [1, 2, 3]);
        assert!(all.find("Mine").is_some());
        assert_eq!(all.materials.len(), core.materials.len() + 1);
        let mut all = all;
        assert!(all.remove("Mine"));
        assert!(!all.remove("Mine"));
    }

    #[test]
    fn builder_validates_and_round_trips_through_json() {
        assert!(build_material("  ", "", [0; 3], 0.5, Pattern::None, "").is_err());
        let d = build_material(
            " Painted Brick ",
            "Masonry > Custom",
            [180, 90, 70],
            1.7,
            Pattern::brick(),
            " /tmp/brick.png ",
        )
        .unwrap();
        assert_eq!(d.name, "Painted Brick");
        assert_eq!(d.category, ["Masonry", "Custom"]);
        assert_eq!(d.roughness, 1.0);
        assert_eq!(d.texture_path.as_deref(), Some("/tmp/brick.png"));
        let mut lib = MaterialLibrary::default();
        lib.add(d.clone());
        let back = MaterialLibrary::from_json(&lib.to_json().unwrap()).unwrap();
        assert_eq!(back.find("Painted Brick"), Some(&d));
        // A file written before textures had paths still loads.
        let mut v = serde_json::to_value(&lib).unwrap();
        v["materials"][0]
            .as_object_mut()
            .unwrap()
            .remove("texture_path");
        let old: MaterialLibrary = serde_json::from_value(v).unwrap();
        assert_eq!(old.materials[0].texture_path, None);
    }
}
