//! Chief files the user picks at run time (Customize Hotkeys > Import,
//! Customize Toolbars > Import), as opposed to Daniel's embedded copies.
//!
//! * [`import_hotkeys_xml`] reads any `UserHotkeys.xml` and recovers command
//!   names with the same catalog [`crate::load_daniel_config`] uses (the file
//!   itself stores ids only).
//! * [`buttons_for_view`] lists the buttons a `.toolbar` file shows in one view
//!   type, toolbar by toolbar in on-screen order, with Chief's own labels.
//!
//! Chief stores view types as small integers. The files give no names for
//! them; the roles below are inferred from which toolbars carry each code (the
//! Layout and Page Management bars carry 6, Ray Trace 9, Materials List 4, the
//! build bars 0/1/3/14, and so on).

use crate::daniel::{daniel_catalog, load_daniel_config};
use crate::error::ConfigError;
use crate::hotkeys::{parse_hotkeys_xml, resolve_names, HotkeyFile, ResolveStats};
use crate::toolbar::ToolbarSet;

/// The view a Chief view-type code stands for, as far as the files show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChiefView {
    /// Code 0: the floor plan.
    Plan,
    /// Code 1: camera (3D) views.
    Camera3d,
    /// Code 3: elevations, sections and other vector views.
    Elevation,
    /// Code 6: the layout page.
    Layout,
}

impl ChiefView {
    /// The Chief code this role is stored as.
    pub fn code(self) -> u32 {
        match self {
            ChiefView::Plan => 0,
            ChiefView::Camera3d => 1,
            ChiefView::Elevation => 3,
            ChiefView::Layout => 6,
        }
    }

    /// The role of a code, `None` for codes Plan Studio has no view for
    /// (materials list 4, ray trace 9, the resource panes 12/13, 14).
    pub fn from_code(code: u32) -> Option<ChiefView> {
        match code {
            0 => Some(ChiefView::Plan),
            1 => Some(ChiefView::Camera3d),
            3 => Some(ChiefView::Elevation),
            6 => Some(ChiefView::Layout),
            _ => None,
        }
    }
}

/// One toolbar's buttons within a view type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewToolbar {
    pub toolbar: String,
    /// `(command id, label)` left to right. The label is Chief's, cleaned of
    /// mnemonics and ellipses; an id with no label in the file gets its number.
    pub buttons: Vec<(String, String)>,
}

/// The toolbars of `set` that show in view type `code` and have buttons, in
/// on-screen order (dock, row, position).
pub fn buttons_for_view(set: &ToolbarSet, code: u32) -> Vec<ViewToolbar> {
    set.layout_order()
        .into_iter()
        .filter(|t| t.view_types.contains(&code) && !t.items.is_empty())
        .map(|t| ViewToolbar {
            toolbar: t.name.clone(),
            buttons: t
                .items
                .iter()
                .map(|i| {
                    (
                        i.command_id.clone(),
                        i.command_name
                            .clone()
                            .unwrap_or_else(|| i.command_id.clone()),
                    )
                })
                .collect(),
        })
        .collect()
}

/// Every button of `set` that appears in some view type (contextual toolbars,
/// which have no view type, are not counted), as `(command id, label)` with
/// repeats removed.
pub fn all_view_buttons(set: &ToolbarSet) -> Vec<(String, String)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for t in set.toolbars.iter().filter(|t| !t.view_types.is_empty()) {
        for i in &t.items {
            if seen.insert(i.command_id.clone()) {
                out.push((
                    i.command_id.clone(),
                    i.command_name
                        .clone()
                        .unwrap_or_else(|| i.command_id.clone()),
                ));
            }
        }
    }
    out
}

/// Reads a `UserHotkeys.xml` and recovers the command names: the XML's own,
/// then Daniel's toolbar name tables, the known-id table and unique default
/// chords.
pub fn import_hotkeys_xml(text: &str) -> Result<(HotkeyFile, ResolveStats), ConfigError> {
    let mut file = parse_hotkeys_xml(text)?;
    let cfg = load_daniel_config();
    let mut catalog = daniel_catalog(&cfg.toolbars);
    catalog.add_xml_names(&file);
    let stats = resolve_names(&mut file, &catalog);
    Ok((file, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar::parse_toolbar_named;

    const XML: &str = include_str!("../../../docs/chief-config-raw/UserHotkeys.xml");
    const DEFAULT_TOOLBAR: &str =
        include_str!("../../../docs/chief-config-raw/Default Configuration.toolbar");

    #[test]
    fn the_hotkey_import_names_what_the_embedded_copy_names() {
        let (file, stats) = import_hotkeys_xml(XML).unwrap();
        let embedded = load_daniel_config().hotkeys;
        assert_eq!(file.bindings.len(), 208);
        assert_eq!(file.named_count(), embedded.named_count());
        assert_eq!(stats.named, embedded.named_count());
    }

    #[test]
    fn a_broken_hotkey_file_is_an_error_not_a_panic() {
        assert!(import_hotkeys_xml("not xml at all <").is_err());
    }

    #[test]
    fn view_codes_have_roles() {
        assert_eq!(ChiefView::from_code(0), Some(ChiefView::Plan));
        assert_eq!(ChiefView::from_code(6), Some(ChiefView::Layout));
        assert_eq!(ChiefView::from_code(9), None);
        for v in [
            ChiefView::Plan,
            ChiefView::Camera3d,
            ChiefView::Elevation,
            ChiefView::Layout,
        ] {
            assert_eq!(ChiefView::from_code(v.code()), Some(v));
        }
    }

    #[test]
    fn plan_view_buttons_come_in_screen_order_with_labels() {
        let set = parse_toolbar_named("Default Configuration", DEFAULT_TOOLBAR).unwrap();
        let plan = buttons_for_view(&set, ChiefView::Plan.code());
        assert!(plan.len() >= 8, "{} toolbars", plan.len());
        let build = plan
            .iter()
            .find(|t| t.toolbar == "Architectural Features")
            .unwrap();
        assert_eq!(build.buttons[0].1, "Select Objects");
        // The layout toolbars are not in the plan.
        assert!(plan.iter().all(|t| t.toolbar != "Page Management"));
        let layout = buttons_for_view(&set, ChiefView::Layout.code());
        assert!(layout.iter().any(|t| t.toolbar == "Page Management"));
        let all = all_view_buttons(&set);
        assert!(all.len() > 80, "{}", all.len());
    }
}
