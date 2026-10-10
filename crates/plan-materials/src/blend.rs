//! Blend Colors (Material Painter): a material made on the fly from two
//! library materials. The result is named after its parts
//! (`Blend: Oak Flooring | Color – White | 40%`), so an object painted with it
//! stores nothing but that name and any library can rebuild the material.

use crate::library::MaterialLibrary;
use crate::material::MaterialDef;

/// Every blend name starts with this.
pub const BLEND_PREFIX: &str = "Blend: ";

/// The name of the blend of `base` and `with`, `percent` of the way to `with`
/// (clamped to 0..=100).
pub fn blend_name(base: &str, with: &str, percent: u8) -> String {
    format!("{BLEND_PREFIX}{base} | {with} | {}%", percent.min(100))
}

/// Is `s` a whole material name: not a blend, or a complete blend name?
fn whole_name(s: &str) -> bool {
    !s.is_empty() && (!s.starts_with(BLEND_PREFIX) || parse_blend_name(s).is_some())
}

/// `(base, with, percent)` of a name made by [`blend_name`]. The parts may be
/// blends themselves; the split is the one that leaves two whole names.
pub fn parse_blend_name(name: &str) -> Option<(String, String, u8)> {
    let rest = name.strip_prefix(BLEND_PREFIX)?;
    let (both, pct) = rest.rsplit_once(" | ")?;
    let pct = pct
        .strip_suffix('%')?
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|p| *p <= 100)?;
    both.match_indices(" | ")
        .map(|(i, _)| (&both[..i], &both[i + 3..]))
        .find(|(a, b)| whole_name(a) && whole_name(b))
        .map(|(a, b)| (a.to_string(), b.to_string(), pct))
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The blend of `base` and `with`, `t` (0..1) of the way to `with`: the colour
/// and the surface sliders mix; the pattern, texture, size and prices stay
/// the base's, and a textured base takes `with`'s colour as its blend colour
/// so the bitmap is tinted too. Named `name`.
pub fn blend_materials(base: &MaterialDef, with: &MaterialDef, t: f32, name: &str) -> MaterialDef {
    let t = t.clamp(0.0, 1.0);
    let mut out = base.clone();
    out.name = name.to_string();
    out.category = vec!["Blends".to_string()];
    for k in 0..3 {
        out.color[k] = lerp_u8(base.color[k], with.color[k], t);
    }
    let (a, b) = (base.surface(), with.surface());
    out.roughness = lerp(a.roughness, b.roughness, t);
    out.metallic = lerp(a.metallic, b.metallic, t);
    out.transparency = lerp(a.transparency, b.transparency, t);
    out.emissive = lerp(a.emissive, b.emissive, t);
    out.class = crate::MaterialClass::General;
    if base.texture_path.is_some() || !matches!(base.texture, crate::Texture::Solid) {
        out.blend_color = Some(with.color);
        out.blend_amount = t.max(base.blend_amount);
    }
    out
}

impl MaterialLibrary {
    /// The material called `name`: a library entry, or the blend a blend name
    /// stands for (its parts are resolved the same way, up to four deep).
    pub fn resolve(&self, name: &str) -> Option<MaterialDef> {
        self.resolve_at(name, 0)
    }

    fn resolve_at(&self, name: &str, depth: u8) -> Option<MaterialDef> {
        if let Some(m) = self.find(name) {
            return Some(m.clone());
        }
        if depth >= 4 {
            return None;
        }
        let (base, with, pct) = parse_blend_name(name)?;
        let a = self.resolve_at(&base, depth + 1)?;
        let b = self.resolve_at(&with, depth + 1)?;
        Some(blend_materials(&a, &b, f32::from(pct) / 100.0, name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_library;

    #[test]
    fn blend_names_round_trip_and_reject_other_names() {
        let n = blend_name("Oak Flooring", "Color – White", 40);
        assert_eq!(n, "Blend: Oak Flooring | Color – White | 40%");
        assert_eq!(
            parse_blend_name(&n),
            Some(("Oak Flooring".into(), "Color – White".into(), 40))
        );
        assert_eq!(parse_blend_name("Oak Flooring"), None);
        assert_eq!(parse_blend_name("Blend: A | 40%"), None);
        assert_eq!(parse_blend_name("Blend: A | B | 140%"), None);
        assert!(blend_name("A", "B", 250).ends_with("| 100%"));
    }

    #[test]
    fn a_blend_mixes_colour_and_keeps_the_base_texture() {
        let lib = core_library();
        let a = lib.find("Drywall").unwrap();
        let b = lib.find("Color – Bone").unwrap();
        let name = blend_name(&a.name, &b.name, 50);
        let m = lib.resolve(&name).expect("resolves");
        for k in 0..3 {
            let want = (f32::from(a.color[k]) + f32::from(b.color[k])) / 2.0;
            assert!((f32::from(m.color[k]) - want).abs() <= 1.0);
        }
        assert_eq!(m.name, name);
        assert_eq!(m.pattern, a.pattern);
        // 0% is the base colour, 100% the other one.
        assert_eq!(
            lib.resolve(&blend_name(&a.name, &b.name, 0)).unwrap().color,
            a.color
        );
        assert_eq!(
            lib.resolve(&blend_name(&a.name, &b.name, 100))
                .unwrap()
                .color,
            b.color
        );
        // A textured base is tinted through its blend colour.
        let wood = lib.find("Oak Flooring").unwrap();
        let t = lib
            .resolve(&blend_name(&wood.name, &b.name, 30))
            .expect("resolves");
        assert_eq!(t.blend_color, Some(b.color));
        assert!(t.blend_amount > 0.29);
        // Blends of blends resolve; a missing part does not.
        let nested = blend_name(&name, "Drywall", 10);
        assert!(lib.resolve(&nested).is_some());
        assert!(lib.resolve(&blend_name("Nope", "Drywall", 10)).is_none());
        assert!(lib.resolve("Not a material").is_none());
    }
}
