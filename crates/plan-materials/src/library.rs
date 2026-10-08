//! The material library and Chief-like core entries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::material::{MaterialDef, ProceduralKind};
use crate::pattern::Pattern;

/// An ordered collection of [`MaterialDef`]s with unique names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MaterialLibrary {
    pub materials: Vec<MaterialDef>,
}

impl MaterialLibrary {
    /// Exact-name lookup, falling back to a case-insensitive match.
    pub fn find(&self, name: &str) -> Option<&MaterialDef> {
        self.materials.iter().find(|m| m.name == name).or_else(|| {
            self.materials
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(name))
        })
    }

    /// Case-insensitive substring search over names and category paths.
    /// An empty query returns every material.
    pub fn search(&self, query: &str) -> Vec<&MaterialDef> {
        let q = query.trim().to_lowercase();
        self.materials
            .iter()
            .filter(|m| {
                q.is_empty()
                    || m.name.to_lowercase().contains(&q)
                    || m.category.iter().any(|c| c.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Materials grouped by their top-level category (`category[0]`, or
    /// `"Uncategorized"`), keys sorted alphabetically.
    pub fn by_category(&self) -> BTreeMap<String, Vec<&MaterialDef>> {
        let mut map: BTreeMap<String, Vec<&MaterialDef>> = BTreeMap::new();
        for m in &self.materials {
            let key = m
                .category
                .first()
                .cloned()
                .unwrap_or_else(|| "Uncategorized".to_string());
            map.entry(key).or_default().push(m);
        }
        map
    }

    /// Adds `def`, replacing any existing material with the same name.
    pub fn add(&mut self, def: MaterialDef) {
        match self.materials.iter_mut().find(|m| m.name == def.name) {
            Some(slot) => *slot = def,
            None => self.materials.push(def),
        }
    }

    /// Serialises to pretty JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a library from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

fn wood(grain: [u8; 3], ring: f64) -> ProceduralKind {
    ProceduralKind::Wood {
        grain_color: grain,
        ring_spacing: ring,
    }
}

/// Chief-like core library: paints, framing, sheathing, siding, masonry,
/// roofing, flooring, counters, glass, metals, finishes and site materials.
pub fn core_library() -> MaterialLibrary {
    use ProceduralKind as K;
    let m = MaterialDef::new;
    let lines = |angle_deg, spacing| Pattern::Lines { angle_deg, spacing };
    let materials = vec![
        // Paint and wall finishes.
        m(
            "Sand Finish – Eggshell",
            &["Paint", "Wall Finish"],
            [240, 234, 220],
        )
        .with_surface(0.7, 0.0)
        .with_texture(K::Stucco { grain: 0.25 }, (24.0, 24.0))
        .with_bump(0.15)
        .with_cost(0.45, "09-9100"),
        m(
            "Sand Finish – Flat",
            &["Paint", "Wall Finish"],
            [236, 232, 222],
        )
        .with_surface(0.95, 0.0)
        .with_texture(K::Stucco { grain: 0.3 }, (24.0, 24.0))
        .with_bump(0.2)
        .with_cost(0.45, "09-9100"),
        m("Drywall", &["Interior", "Wall Surface"], [232, 230, 224])
            .with_surface(0.9, 0.0)
            .with_cost(1.10, "09-2900"),
        m("Color – Bone", &["Paint", "Color"], [227, 218, 201])
            .with_surface(0.75, 0.0)
            .with_cost(0.45, "09-9100"),
        m("Color – White", &["Paint", "Color"], [248, 248, 245])
            .with_surface(0.75, 0.0)
            .with_cost(0.45, "09-9100"),
        // Framing and sheathing.
        m("Fir Framing", &["Framing", "Lumber"], [214, 178, 125])
            .with_pattern(lines(45.0, 3.0))
            .with_texture(wood([150, 105, 60], 0.6), (48.0, 48.0))
            .with_cost(1.35, "06-1100"),
        m("OSB-Hrz", &["Framing", "Sheathing"], [196, 160, 104])
            .with_pattern(Pattern::CrossHatch {
                angle_deg: 45.0,
                spacing: 6.0,
            })
            .with_texture(K::Concrete, (96.0, 48.0))
            .with_cost(0.85, "06-1600"),
        m("Housewrap", &["Framing", "Sheathing"], [236, 238, 240])
            .with_surface(0.6, 0.0)
            .with_cost(0.20, "07-2500"),
        m("Plywood", &["Framing", "Sheathing"], [208, 170, 112])
            .with_pattern(lines(45.0, 4.0))
            .with_texture(wood([160, 118, 70], 1.2), (48.0, 48.0))
            .with_cost(1.40, "06-1600"),
        // Siding and exterior finishes.
        m("Stucco (Sand)", &["Siding", "Stucco"], [215, 205, 185])
            .with_surface(0.95, 0.0)
            .with_pattern(Pattern::Concrete)
            .with_texture(K::Stucco { grain: 0.6 }, (36.0, 36.0))
            .with_bump(0.5)
            .with_cost(8.50, "09-2400"),
        m(
            "Lap Siding – White",
            &["Siding", "Lap Siding"],
            [244, 244, 240],
        )
        .with_pattern(Pattern::lap_siding())
        .with_texture(K::LapSiding, (24.0, 24.0))
        .with_bump(0.3)
        .with_cost(6.25, "07-4600"),
        m(
            "Board & Batten",
            &["Siding", "Board & Batten"],
            [226, 222, 212],
        )
        .with_pattern(Pattern::board_and_batten())
        .with_texture(wood([180, 170, 150], 1.5), (48.0, 48.0))
        .with_bump(0.35)
        .with_cost(7.00, "07-4600"),
        // Masonry.
        m("Brick – Red", &["Masonry", "Brick"], [160, 72, 52])
            .with_pattern(Pattern::brick())
            .with_texture(
                K::Brick {
                    mortar_color: [190, 186, 176],
                },
                (32.0, 9.0),
            )
            .with_bump(0.4)
            .with_cost(14.00, "04-2100"),
        m("Brick – Tan", &["Masonry", "Brick"], [196, 164, 124])
            .with_pattern(Pattern::brick())
            .with_texture(
                K::Brick {
                    mortar_color: [200, 196, 184],
                },
                (32.0, 9.0),
            )
            .with_bump(0.4)
            .with_cost(14.00, "04-2100"),
        m(
            "Stone Veneer – Fieldstone",
            &["Masonry", "Stone"],
            [150, 140, 126],
        )
        .with_pattern(Pattern::Concrete)
        .with_texture(K::Stone, (36.0, 36.0))
        .with_bump(0.7)
        .with_cost(18.00, "04-7200"),
        m("Concrete", &["Masonry", "Concrete"], [170, 170, 166])
            .with_surface(0.9, 0.0)
            .with_pattern(Pattern::Concrete)
            .with_texture(K::Concrete, (48.0, 48.0))
            .with_cost(6.00, "03-3000"),
        m("Concrete Block", &["Masonry", "Block"], [160, 160, 156])
            .with_surface(0.95, 0.0)
            .with_pattern(Pattern::block())
            .with_texture(
                K::Brick {
                    mortar_color: [135, 135, 130],
                },
                (64.0, 16.0),
            )
            .with_bump(0.3)
            .with_cost(9.00, "04-2200"),
        // Roofing.
        m(
            "Asphalt Shingles – Charcoal",
            &["Roofing", "Shingles"],
            [70, 70, 72],
        )
        .with_pattern(Pattern::shingle())
        .with_texture(K::Shingles, (36.0, 30.0))
        .with_bump(0.4)
        .with_cost(2.60, "07-3100"),
        m(
            "Architectural Shingles – Weathered Wood",
            &["Roofing", "Shingles"],
            [112, 98, 86],
        )
        .with_pattern(Pattern::shingle())
        .with_texture(K::Shingles, (36.0, 30.0))
        .with_bump(0.5)
        .with_cost(3.40, "07-3100"),
        m("Standing Seam Metal", &["Roofing", "Metal"], [96, 104, 112])
            .with_surface(0.35, 0.8)
            .with_pattern(Pattern::BoardAndBatten { spacing: 16.0 })
            .with_texture(K::Metal, (32.0, 32.0))
            .with_cost(9.50, "07-4100"),
        m("Clay Tile", &["Roofing", "Tile"], [184, 96, 64])
            .with_surface(0.6, 0.0)
            .with_pattern(Pattern::Shingle {
                exposure: 10.0,
                width: 13.0,
            })
            .with_texture(K::Shingles, (39.0, 30.0))
            .with_bump(0.6)
            .with_cost(12.00, "07-3200"),
        // Flooring.
        m("Oak Flooring", &["Flooring", "Wood"], [196, 150, 96])
            .with_surface(0.45, 0.0)
            .with_pattern(Pattern::Tile { w: 36.0, h: 3.25 })
            .with_texture(wood([140, 96, 52], 0.8), (36.0, 12.0))
            .with_cost(8.00, "09-6400"),
        m("Maple Flooring", &["Flooring", "Wood"], [226, 196, 150])
            .with_surface(0.45, 0.0)
            .with_pattern(Pattern::Tile { w: 36.0, h: 3.25 })
            .with_texture(wood([190, 150, 100], 1.0), (36.0, 12.0))
            .with_cost(7.50, "09-6400"),
        m("Walnut Flooring", &["Flooring", "Wood"], [110, 78, 54])
            .with_surface(0.45, 0.0)
            .with_pattern(Pattern::Tile { w: 36.0, h: 5.0 })
            .with_texture(wood([62, 40, 26], 0.9), (36.0, 12.0))
            .with_cost(11.00, "09-6400"),
        m("Carpet – Beige", &["Flooring", "Carpet"], [206, 190, 164])
            .with_surface(1.0, 0.0)
            .with_pattern(lines(45.0, 2.0))
            .with_texture(K::Carpet, (24.0, 24.0))
            .with_bump(0.2)
            .with_cost(3.50, "09-6800"),
        m("Ceramic Tile 12x12", &["Flooring", "Tile"], [214, 208, 196])
            .with_surface(0.3, 0.0)
            .with_pattern(Pattern::Tile { w: 12.0, h: 12.0 })
            .with_texture(
                K::Tile {
                    grout: [160, 156, 148],
                },
                (24.0, 24.0),
            )
            .with_cost(5.50, "09-3000"),
        m(
            "Porcelain Tile 24x24",
            &["Flooring", "Tile"],
            [226, 224, 220],
        )
        .with_surface(0.2, 0.0)
        .with_pattern(Pattern::Tile { w: 24.0, h: 24.0 })
        .with_texture(
            K::Tile {
                grout: [170, 168, 162],
            },
            (48.0, 48.0),
        )
        .with_cost(8.00, "09-3000"),
        // Countertops.
        m(
            "Granite – Black Pearl",
            &["Countertop", "Granite"],
            [36, 36, 38],
        )
        .with_surface(0.15, 0.0)
        .with_texture(K::Stone, (48.0, 48.0))
        .with_cost(55.00, "06-6100"),
        m("Quartz – White", &["Countertop", "Quartz"], [240, 240, 238])
            .with_surface(0.18, 0.0)
            .with_cost(60.00, "06-6100"),
        m("Butcher Block", &["Countertop", "Wood"], [188, 140, 84])
            .with_surface(0.5, 0.0)
            .with_texture(wood([140, 96, 52], 0.7), (36.0, 24.0))
            .with_cost(30.00, "06-6100"),
        m(
            "Batt Insulation",
            &["Thermal", "Insulation"],
            [238, 206, 120],
        )
        .with_pattern(Pattern::Insulation)
        .with_cost(0.70, "07-2100"),
        // Glass.
        m("Glass", &["Glass"], [200, 225, 235])
            .with_surface(0.05, 0.0)
            .with_transparency(0.7)
            .with_texture(K::Glass, (48.0, 48.0))
            .with_cost(15.00, "08-8000"),
        m("Clear Glass", &["Glass"], [214, 234, 240])
            .with_surface(0.02, 0.0)
            .with_transparency(0.85)
            .with_texture(K::Glass, (48.0, 48.0))
            .with_cost(18.00, "08-8000"),
        m("Frosted Glass", &["Glass"], [230, 238, 240])
            .with_surface(0.6, 0.0)
            .with_transparency(0.35)
            .with_texture(K::Glass, (48.0, 48.0))
            .with_cost(24.00, "08-8000"),
        // Metals.
        m("Chrome", &["Metal", "Plated"], [218, 220, 224])
            .with_surface(0.05, 1.0)
            .with_texture(K::Metal, (12.0, 12.0))
            .with_cost(0.0, "08-7100"),
        m("Brushed Nickel", &["Metal", "Plated"], [176, 176, 172])
            .with_surface(0.35, 0.95)
            .with_texture(K::Metal, (12.0, 12.0))
            .with_cost(0.0, "08-7100"),
        m("Oil-Rubbed Bronze", &["Metal", "Plated"], [70, 52, 40])
            .with_surface(0.4, 0.85)
            .with_texture(K::Metal, (12.0, 12.0))
            .with_cost(0.0, "08-7100"),
        // Trim and doors.
        m("Black Paint", &["Paint", "Color"], [28, 28, 30])
            .with_surface(0.5, 0.0)
            .with_cost(0.50, "09-9100"),
        m("Painted White Trim", &["Paint", "Trim"], [250, 250, 247])
            .with_surface(0.4, 0.0)
            .with_cost(0.80, "06-2200"),
        m("Lincoln Door", &["Doors", "Painted Panel"], [246, 245, 240])
            .with_surface(0.4, 0.0)
            .with_cost(0.0, "08-1400"),
        // Site.
        m("Grass", &["Site", "Landscape"], [88, 130, 56])
            .with_pattern(Pattern::Grass)
            .with_texture(K::Grass, (48.0, 48.0))
            .with_cost(0.90, "32-9200"),
        m("Dirt", &["Site", "Landscape"], [120, 92, 66])
            .with_pattern(Pattern::Earth)
            .with_texture(K::Concrete, (48.0, 48.0))
            .with_cost(0.0, "31-2300"),
        m("Gravel", &["Site", "Paving"], [156, 150, 142])
            .with_pattern(Pattern::Concrete)
            .with_texture(K::Stone, (24.0, 24.0))
            .with_cost(1.20, "32-1400"),
        m("Asphalt", &["Site", "Paving"], [64, 64, 66])
            .with_surface(0.95, 0.0)
            .with_texture(K::Concrete, (48.0, 48.0))
            .with_cost(3.00, "32-1200"),
        m("Water", &["Site", "Landscape"], [70, 120, 150])
            .with_surface(0.02, 0.0)
            .with_transparency(0.6)
            .with_texture(K::Glass, (96.0, 96.0))
            .with_cost(0.0, "13-1100"),
    ];
    MaterialLibrary { materials }
}
