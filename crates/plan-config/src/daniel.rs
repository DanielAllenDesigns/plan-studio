//! Daniel's own configuration, embedded at compile time.

use crate::catalog::CommandCatalog;
use crate::error::ConfigError;
use crate::hotkeys::{parse_hotkeys_xml, resolve_names, HotkeyFile, ResolveStats};
use crate::prefs::{daniel_x18, ChiefPreferences};
use crate::toolbar::{parse_toolbar_named, ToolbarItemDef, ToolbarSet};
use serde::{Deserialize, Serialize};

const HOTKEYS_XML: &str = include_str!("../../../docs/chief-config-raw/UserHotkeys.xml");

const TOOLBAR_FILES: [(&str, &str); 4] = [
    (
        "Default Configuration",
        include_str!("../../../docs/chief-config-raw/Default Configuration.toolbar"),
    ),
    (
        "Extended Tool Configuration",
        include_str!("../../../docs/chief-config-raw/Extended Tool Configuration.toolbar"),
    ),
    (
        "Space Planning Configuration",
        include_str!("../../../docs/chief-config-raw/Space Planning Configuration.toolbar"),
    ),
    (
        "Terrain Configuration",
        include_str!("../../../docs/chief-config-raw/Terrain Configuration.toolbar"),
    ),
];

/// Inventory documents that carry `Name <hotkey>` pairs and flyout groups.
const INVENTORY_DOCS: [&str; 3] = [
    include_str!("../../../docs/chief-x18-subtools.md"),
    include_str!("../../../docs/chief-x18-menus.md"),
    include_str!("../../../docs/chief-x18-toolbars.md"),
];

/// Everything parsed from Daniel's Chief installation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DanielConfig {
    /// Hotkeys with command names resolved.
    pub hotkeys: HotkeyFile,
    /// The four toolbar sets, Default first.
    pub toolbars: Vec<ToolbarSet>,
    pub preferences: ChiefPreferences,
}

/// The catalog [`load_daniel_config`] resolves names with.
pub fn daniel_catalog(sets: &[ToolbarSet]) -> CommandCatalog {
    CommandCatalog::from_sources(sets, &INVENTORY_DOCS)
}

/// Fills each flyout button's `flyout` from the catalog's groups. A member
/// gets the id of the hotkey binding with the same name when there is one.
pub fn attach_flyouts(set: &mut ToolbarSet, catalog: &CommandCatalog, hotkeys: &HotkeyFile) {
    for tb in &mut set.toolbars {
        for item in &mut tb.items {
            let Some(group) = item.command_name.as_deref().and_then(|n| catalog.group(n)) else {
                continue;
            };
            item.flyout = group
                .members
                .iter()
                .map(|m| ToolbarItemDef {
                    command_id: hotkeys
                        .bindings
                        .iter()
                        .find(|b| b.command_name.as_deref() == Some(m.name.as_str()))
                        .map(|b| b.command_id.clone())
                        .unwrap_or_default(),
                    command_name: Some(m.name.clone()),
                    flyout: Vec::new(),
                    separator_before: false,
                })
                .collect();
        }
    }
}

/// Parses the embedded files. Also returns how name resolution went.
pub fn load_daniel_config_with_stats() -> Result<(DanielConfig, ResolveStats), ConfigError> {
    let mut hotkeys = parse_hotkeys_xml(HOTKEYS_XML)?;
    let mut toolbars = Vec::with_capacity(TOOLBAR_FILES.len());
    for (name, text) in TOOLBAR_FILES {
        toolbars.push(parse_toolbar_named(name, text)?);
    }
    let mut catalog = daniel_catalog(&toolbars);
    catalog.add_xml_names(&hotkeys);
    let stats = resolve_names(&mut hotkeys, &catalog);
    for set in &mut toolbars {
        attach_flyouts(set, &catalog, &hotkeys);
    }
    Ok((
        DanielConfig {
            hotkeys,
            toolbars,
            preferences: daniel_x18(),
        },
        stats,
    ))
}

/// Daniel's hotkeys (names resolved), toolbar sets and preferences.
///
/// The inputs are embedded and covered by this crate's tests, so a failure
/// here is a bug in the crate, not a runtime condition.
pub fn load_daniel_config() -> DanielConfig {
    load_daniel_config_with_stats()
        .expect("embedded Chief configuration must parse")
        .0
}

/// Pretty JSON for any of the crate's serializable types.
pub fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("plain data always serializes")
}

/// Reads JSON written by [`to_json`].
pub fn from_json<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, ConfigError> {
    serde_json::from_str(text).map_err(|e| ConfigError::Json(e.to_string()))
}
