//! plan-chiefplan: read-only reader for Chief Architect `.plan` / `.layout`
//! template files.
//!
//! Phase A ([`scan`], [`classify`]) lists the names stored in a template:
//! layer sets, layers, text styles, dimension defaults, wall types, saved plan
//! views, sheet sizes, layout pages. Phase B ([`values`]) decodes per-layer
//! colour, line weight and display/lock flags where the record frame is
//! understood. Phase C ([`decode`]) reads the typed objects: wall type layer
//! stacks, text styles, dimension defaults, materials and default heights,
//! collected in [`TemplateSummary`]. [`bridge`] turns an inventory into Plan
//! Studio defaults. Phase D ([`import`]) reads a project's geometry (floors,
//! walls, doors, windows, room names, dimensions, text) into a
//! [`plan_core::Project`]; its format notes are in `docs/chief-plan-format.md`.
//!
//! Nothing here writes to a template or copies one: files are read in place.
//! `std` + `serde` only.
//!
//! ```no_run
//! let inv = plan_chiefplan::build_inventory("Residential Template.plan").unwrap();
//! println!("{} layer sets", inv.layer_sets.len());
//! ```

pub mod bridge;
pub mod classify;
pub mod decode;
pub mod error;
pub mod import;
pub mod redact;
pub mod scan;
pub mod values;

pub use bridge::{
    apply_seed, seed_defaults, seed_dimension_sets, seed_layer_sets, seed_plan_defaults,
    seed_text_styles, ApplySeed, LayerSetSeed, LayoutSeed, TemplateSeed,
};
pub use classify::{classify, classify_strings, Category, Entry, TemplateInventory};
pub use decode::{
    DefaultHeights, DefaultMaterial, LayoutInfo, PaperSize, TemplateDimensionDefaults,
    TemplateMaterial, TemplateRichText, TemplateSummary, TemplateTextStyle, TemplateWallLayer,
    TemplateWallType,
};
pub use error::{Error, Result};
pub use scan::{scan, scan_bytes, TemplateKind, TemplateScan};
pub use values::{calibrate, decode, Calibration, Confidence, ValueReport};

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Daniel's default plan template, relative to [`templates_dir`].
pub const DEFAULT_PLAN_TEMPLATE: &str = "x17 Working Template 2025-08-20.plan";
/// Daniel's default layout template, relative to [`templates_dir`].
pub const DEFAULT_LAYOUT_TEMPLATE: &str = "18x24 PRESENTATION LAYOUT TEMPLATE.layout";

/// `~/Documents/Chief Architect Premier X18 Data/Templates`, if `HOME` is set.
pub fn templates_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Documents")
            .join("Chief Architect Premier X18 Data")
            .join("Templates"),
    )
}

/// Every `.plan` / `.layout` file under `dir` (recursive), sorted by path.
pub fn find_template_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if TemplateKind::from_extension(&p).is_some() {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

/// Scan + classify + value decoding for one file, in one pass over its bytes.
/// Layers found by their record frame are merged into `layers`.
pub fn build_inventory(path: impl AsRef<Path>) -> Result<TemplateInventory> {
    Ok(build_inventory_with_values(path)?.0)
}

/// Like [`build_inventory`], also returning the raw [`ValueReport`] (for
/// cross-file calibration).
pub fn build_inventory_with_values(
    path: impl AsRef<Path>,
) -> Result<(TemplateInventory, ValueReport)> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    build_inventory_from_bytes(&bytes, &file_name, TemplateKind::from_extension(path))
}

/// [`build_inventory_with_values`] over a file already in memory.
/// `extension_kind` is the kind the file name implies, if any.
pub fn build_inventory_from_bytes(
    bytes: &[u8],
    file_name: &str,
    extension_kind: Option<TemplateKind>,
) -> Result<(TemplateInventory, ValueReport)> {
    let scan = scan_bytes(bytes, extension_kind)?;
    let mut inv = classify(&scan);
    inv.file_name = file_name.to_string();
    let report = decode(bytes, &scan);
    for set in &report.layer_sets {
        for l in &set.layers {
            inv.add_if_missing(Category::Layer, &l.name, l.offset);
        }
    }
    inv.layer_set_data = report.layer_sets.clone();
    inv.wall_stacks = report.wall_stacks.clone();
    inv.summary = decode::summarize_bytes(
        bytes,
        &scan.strings,
        scan.kind,
        &inv.file_name,
        scan.resource_table_start as usize,
    );
    Ok((inv, report))
}

/// The Phase C decoders alone: wall types, text styles, dimension defaults,
/// materials, default heights, paper sizes and layout info of one template.
pub fn summarize(path: impl AsRef<Path>) -> Result<TemplateSummary> {
    Ok(build_inventory(path)?.summary)
}

/// Inventories of every template in Daniel's Templates folder. Files that
/// fail to parse are skipped.
pub fn scan_daniel_templates() -> Vec<(PathBuf, TemplateInventory)> {
    templates_dir().map_or_else(Vec::new, |d| scan_dir(&d))
}

/// Inventories of every template under `dir`.
pub fn scan_dir(dir: &Path) -> Vec<(PathBuf, TemplateInventory)> {
    find_template_files(dir)
        .into_iter()
        .filter_map(|p| build_inventory(&p).ok().map(|inv| (p, inv)))
        .collect()
}

#[derive(Serialize)]
struct InventoryFile<'a> {
    /// Path relative to the scanned folder.
    file: String,
    inventory: &'a TemplateInventory,
}

#[derive(Serialize)]
struct InventoryDocument<'a> {
    generator: &'static str,
    note: &'static str,
    files: Vec<InventoryFile<'a>>,
    calibration: Calibration,
}

/// Scans every template under `dir`, redacts client-specific strings, and
/// writes one JSON document (inventories plus the cross-file calibration) to
/// `out_path`. Returns the number of templates written.
pub fn write_inventory_json(dir: &Path, out_path: &Path) -> Result<usize> {
    let mut invs = Vec::new();
    let mut reports = Vec::new();
    for p in find_template_files(dir) {
        let Ok((mut inv, rep)) = build_inventory_with_values(&p) else {
            continue;
        };
        redact::redact_inventory(&mut inv);
        let is_default = [DEFAULT_PLAN_TEMPLATE, DEFAULT_LAYOUT_TEMPLATE]
            .iter()
            .any(|d| p.file_name().is_some_and(|n| n == *d));
        if !is_default {
            // Keep the file small: layer tables only for the two default
            // templates; the calibration summary covers all files.
            inv.layer_set_data.clear();
            inv.summary.materials.clear();
        }
        let rel = p
            .strip_prefix(dir)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        invs.push((rel, inv));
        reports.push(rep);
    }
    let doc = InventoryDocument {
        generator: "plan-chiefplan",
        note: "Names only; client-specific strings and non-stock paths are redacted. Per-layer records and the material table are kept for the two default templates only.",
        files: invs
            .iter()
            .map(|(file, inventory)| InventoryFile {
                file: file.clone(),
                inventory,
            })
            .collect(),
        calibration: calibrate(&reports),
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&doc)?)?;
    Ok(invs.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::testutil::{build_template, put_str};

    #[test]
    fn end_to_end_on_a_synthetic_template() {
        let mut body = Vec::new();
        for s in [
            "Elevation View Layer Set",
            "Siding-6",
            "Stucco-6",
            "1/4\" Scale Dimension Defaults",
            "Floor Plan View Dimensioned",
            "Walls, Normal",
        ] {
            body.extend_from_slice(&[0, 0, 0, 0]);
            put_str(&mut body, s);
        }
        let dir = std::env::temp_dir().join(format!("plan-chiefplan-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let bytes = build_template(
            &body,
            &["/x/Chief Architect Premier X18 Data/Textures/A.jpg"],
        );
        std::fs::write(dir.join("a.plan"), &bytes).unwrap();
        std::fs::write(dir.join("sub").join("b.layout"), &bytes).unwrap();
        std::fs::write(dir.join("ignore.txt"), b"x").unwrap();

        assert_eq!(find_template_files(&dir).len(), 2);
        let inv = build_inventory(dir.join("a.plan")).unwrap();
        assert_eq!(inv.file_name, "a.plan");
        assert!(inv.contains(Category::LayerSet, "Elevation View Layer Set"));
        assert!(inv.contains(Category::WallType, "Siding-6"));
        assert!(inv.contains(Category::PlanView, "Floor Plan View Dimensioned"));
        assert_eq!(
            inv.resources,
            vec!["/x/Chief Architect Premier X18 Data/Textures/A.jpg"]
        );
        assert!(inv.thumbnail_bytes > 0);

        let out = dir.join("out").join("inv.json");
        assert_eq!(write_inventory_json(&dir, &out).unwrap(), 2);
        let json = std::fs::read_to_string(&out).unwrap();
        assert!(json.contains("\"Siding-6\"") && json.contains("sub/b.layout"));
        // Stock texture reduced to its file name.
        assert!(json.contains("\"A.jpg\"") && !json.contains("/x/Chief"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
