//! Redaction of client-specific text before an inventory is written out.
//!
//! Templates saved from real projects carry leftovers: the project plan's
//! file name, network-volume logo paths, street addresses. The inventory keeps
//! the template vocabulary and drops those.

use crate::classify::{Entry, TemplateInventory};

/// Placeholder written instead of a redacted string.
pub const REDACTED: &str = "[redacted]";

/// Whether a string looks like it names a client, project, address or a
/// local/network path.
pub fn looks_client_specific(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if lower.contains(".cmvolumes") || lower.contains("/users/") || lower.contains("/volumes/") {
        return true;
    }
    if lower.ends_with(".plan") || lower.ends_with(".layout") || lower.contains(".plan ") {
        return true;
    }
    // `_DD_2023-02-16` style project suffixes and ISO dates.
    if has_iso_date(s) {
        return true;
    }
    // "2836 Parkridge": 3-5 digits, a space, a capitalised word (not "2100 - ...").
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if (3..=5).contains(&digits) {
        let rest = &s[digits..];
        if let Some(word) = rest.strip_prefix(' ') {
            return word.starts_with(|c: char| c.is_ascii_uppercase())
                && word.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase());
        }
    }
    false
}

fn has_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(10).any(|w| {
        w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..].iter().all(u8::is_ascii_digit)
    })
}

/// Keeps a resource path only when it is a stock Chief texture or backdrop
/// reference (reduced to its file name) or a bare file name.
pub fn redact_resource(path: &str) -> Option<String> {
    let stock = path.contains("Chief Architect Premier")
        && path.contains(" Data/")
        && (path.contains("/Textures/") || path.contains("/Backdrops/"));
    if stock {
        return path.rsplit('/').next().map(str::to_string);
    }
    if path.contains('/') || looks_client_specific(path) {
        return None;
    }
    Some(path.to_string())
}

/// Replaces client-specific names in place; returns how many were replaced.
/// Redacted entries keep their counts but get the name `[redacted]`.
pub fn redact_inventory(inv: &mut TemplateInventory) -> usize {
    let mut n = 0;
    for cat in TemplateInventory::CATEGORIES {
        n += redact_entries(inv.entries_mut(cat));
    }
    let before = inv.resources.len();
    inv.resources = inv
        .resources
        .iter()
        .filter_map(|r| redact_resource(r))
        .collect();
    n += before - inv.resources.len();
    n += redact_summary(inv);
    inv.redacted = n;
    n
}

/// Blanks client-looking names in the Phase C summary (wall types, styles,
/// materials, paper sizes, layout macros and fields) and drops the printer
/// name. Returns how many strings changed.
fn redact_summary(inv: &mut TemplateInventory) -> usize {
    let mut n = 0;
    let mut fix = |name: &mut String| {
        if looks_client_specific(name) {
            *name = REDACTED.to_string();
            n += 1;
        }
    };
    let s = &mut inv.summary;
    s.wall_types.iter_mut().for_each(|w| fix(&mut w.name));
    s.text_styles.iter_mut().for_each(|t| fix(&mut t.name));
    s.rich_text_defaults
        .iter_mut()
        .for_each(|t| fix(&mut t.name));
    s.dimension_defaults
        .iter_mut()
        .for_each(|d| fix(&mut d.name));
    s.materials.iter_mut().for_each(|m| fix(&mut m.name));
    s.paper_sizes.iter_mut().for_each(|p| fix(&mut p.name));
    if let Some(l) = s.layout.as_mut() {
        l.title_block_macros.iter_mut().for_each(|(m, _)| fix(m));
        l.project_info_fields.iter_mut().for_each(|(m, _)| fix(m));
    }
    n
}

fn redact_entries(list: &mut [Entry]) -> usize {
    let mut n = 0;
    for e in list {
        if looks_client_specific(&e.name) {
            e.name = REDACTED.to_string();
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::classify_strings;

    #[test]
    fn detects_client_strings() {
        for s in [
            "/Liam - 2836 Parkridge_DD_2023-02-16.plan",
            "/Users/someone/.CMVolumes/Studio/Logo.png",
            "2836 Parkridge",
            "Main House 2023-02-16",
            "Smith Residence.plan",
        ] {
            assert!(looks_client_specific(s), "{s}");
        }
        for s in [
            "Siding-6",
            "1/4\" Scale Dimension Defaults",
            "2100 - Footings and foundation",
            "ARCH C (18\" x 24\")",
            "Walls, Default Fill Color",
            "10\" CMU (block) Stem Wall",
        ] {
            assert!(!looks_client_specific(s), "{s}");
        }
    }

    #[test]
    fn resources_keep_stock_textures_only() {
        let stock = "/Users/x/Documents/Chief Architect Premier X18 Data/Textures/Wood.jpg";
        assert_eq!(redact_resource(stock).as_deref(), Some("Wood.jpg"));
        assert_eq!(redact_resource("Ash.png").as_deref(), Some("Ash.png"));
        assert_eq!(redact_resource("/Liam - 2836 Parkridge_DD.plan"), None);
        assert_eq!(redact_resource("/Users/x/.CMVolumes/Studio/logo.png"), None);
    }

    #[test]
    fn inventory_redaction_counts() {
        let strings = vec![
            (1u64, "Smith Residence 2024-01-02 Plan View".to_string()),
            (2, "Working Plan View".to_string()),
        ];
        let mut inv = classify_strings(&strings);
        inv.resources = vec!["/Users/x/.CMVolumes/a.png".into(), "Ash.png".into()];
        let n = redact_inventory(&mut inv);
        assert_eq!(n, 2);
        assert_eq!(inv.redacted, 2);
        assert_eq!(inv.plan_views[0].name, REDACTED);
        assert_eq!(inv.plan_views[1].name, "Working Plan View");
        assert_eq!(inv.resources, vec!["Ash.png"]);
    }

    #[test]
    fn summary_names_are_redacted() {
        use crate::decode::{TemplateMaterial, TemplateWallType};
        let mut inv = classify_strings(&[]);
        let wall = |name: &str| TemplateWallType {
            name: name.into(),
            layers: Vec::new(),
            layer_details: Vec::new(),
            total_thickness_in: 0.0,
            offset: 0,
        };
        inv.summary.wall_types = vec![wall("Siding-6"), wall("2836 Parkridge")];
        inv.summary.materials = vec![TemplateMaterial {
            id: 1,
            name: "Smith Residence.plan".into(),
            color: [0, 0, 0],
            offset: 0,
        }];
        assert_eq!(redact_inventory(&mut inv), 2);
        assert_eq!(inv.summary.wall_types[0].name, "Siding-6");
        assert_eq!(inv.summary.wall_types[1].name, REDACTED);
        assert_eq!(inv.summary.materials[0].name, REDACTED);
    }
}
