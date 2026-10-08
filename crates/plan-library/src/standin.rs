//! Stand-in names for placed symbols whose catalog item is not known.
//!
//! The Chief `.plan` importer names a library object `chief-plan.<slug>`
//! because the plan stores no catalog link (`plan-chiefplan` `symbols.rs`).
//! Such a symbol, and any other whose id no catalog resolves, is drawn as a
//! labelled box in plan and as a labelled block in 3D; this module turns the
//! id into the text on the label.

/// The id prefix of the Chief importer's stand-in symbols.
pub const IMPORT_PREFIX: &str = "chief-plan.";

/// Is `catalog_id` one of the importer's `chief-plan.<slug>` stand-ins?
pub fn is_import_stand_in(catalog_id: &str) -> bool {
    catalog_id.starts_with(IMPORT_PREFIX)
}

/// The words on the label of a symbol with this catalog id: the slug of a
/// `chief-plan.` id with its dashes as spaces and each word capitalized
/// (`chief-plan.dining-chair` gives `Dining Chair`), else the last dotted
/// part of the id the same way; `Symbol` when nothing is left.
pub fn stand_in_label(catalog_id: &str) -> String {
    let tail = catalog_id
        .strip_prefix(IMPORT_PREFIX)
        .unwrap_or_else(|| catalog_id.rsplit('.').next().unwrap_or(catalog_id));
    let words: Vec<String> = tail
        .split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(c) => c.to_uppercase().chain(cs).collect(),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        "Symbol".to_string()
    } else {
        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importer_ids_become_title_case_words() {
        assert!(is_import_stand_in("chief-plan.dining-chair"));
        assert!(!is_import_stand_in("core.plumbing.toilet"));
        assert_eq!(stand_in_label("chief-plan.dining-chair"), "Dining Chair");
        assert_eq!(stand_in_label("chief-plan.object"), "Object");
        assert_eq!(stand_in_label("chief-plan."), "Symbol");
    }

    #[test]
    fn other_unknown_ids_use_their_last_part() {
        assert_eq!(stand_in_label("acme.lighting.wall_sconce"), "Wall Sconce");
        assert_eq!(stand_in_label(""), "Symbol");
    }
}
