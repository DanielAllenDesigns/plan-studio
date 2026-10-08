//! plan-config: reads Chief Architect's user configuration into Plan Studio data.
//!
//! * [`parse_hotkeys_xml`] reads `UserHotkeys.xml` into [`HotkeyFile`];
//!   [`resolve_names`] recovers command names (Chief's file stores only ids).
//! * [`parse_toolbar`] reads `.toolbar` files into [`ToolbarSet`].
//! * [`parse_ini`] and [`preferences_from_ini`] read the preferences INI;
//!   [`daniel_x18`] holds Daniel's captured values.
//! * [`load_daniel_config`] returns all of Daniel's configuration from files
//!   embedded at compile time.
//!
//! No dependencies beyond serde; the XML and INI readers are hand-written.

#![forbid(unsafe_code)]

mod catalog;
mod chord;
mod daniel;
mod error;
mod hotkeys;
mod import;
mod prefs;
mod templates;
mod toolbar;
mod xml;

pub use catalog::{parse_hotkey_doc, CommandCatalog, DocEntry, DocGroup, KNOWN_IDS};
pub use chord::{format_sequence, normalize_key, KeyChord};
pub use daniel::{
    attach_flyouts, daniel_catalog, from_json, load_daniel_config, load_daniel_config_with_stats,
    to_json, DanielConfig,
};
pub use error::ConfigError;
pub use hotkeys::{
    parse_hotkeys_xml, resolve_names, resolved_markdown, to_plan_studio_bindings,
    write_hotkeys_xml, ExportRow, HotkeyBinding, HotkeyFile, NameSource, PlanBinding, ResolveStats,
};
pub use import::{all_view_buttons, buttons_for_view, import_hotkeys_xml, ChiefView, ViewToolbar};
pub use prefs::{
    daniel_x18, parse_ini, preferences_from_ini, ChiefPreferences, IniFile, IniSection, SnapPrefs,
};
pub use templates::{
    detect_chief_templates, preferences_file, resolve_template, templates_folder, ChiefTemplates,
    FALLBACK_LAYOUT_TEMPLATE, FALLBACK_PLAN_TEMPLATE, LAYOUT_TEMPLATE_KEY, PLAN_TEMPLATE_KEY,
};
pub use toolbar::{
    clean_label, parse_toolbar, parse_toolbar_named, Placement, ToolbarDef, ToolbarItemDef,
    ToolbarSet,
};

/// The text with every line ending as `\n`. A checkout that converts line
/// endings (Windows, `core.autocrlf`) hands the parsers `\r\n`, and they split
/// on `\n`; every parser runs its input through this first.
pub(crate) fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests;
