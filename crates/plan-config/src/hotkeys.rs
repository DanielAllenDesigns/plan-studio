//! `UserHotkeys.xml`: parsing, name recovery and Plan Studio output.

use crate::catalog::CommandCatalog;
use crate::chord::{format_sequence, KeyChord};
use crate::error::ConfigError;
use crate::normalize_newlines;
use crate::xml;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Where a binding's command name came from, strongest first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NameSource {
    #[default]
    Unknown,
    /// The XML itself carried a name (Chief's own files do not).
    Xml,
    /// The id appears in a `.toolbar` file's `Buttons` name table.
    ToolbarButton,
    /// The id is in the small hardcoded table of ids inferred from the
    /// toolbar files and Daniel's setup inventory.
    KnownId,
    /// The binding's chord equals the chord of exactly one documented Chief
    /// default hotkey (a weaker inference: Daniel may have rebound it).
    MatchedDefaultHotkey,
}

impl NameSource {
    pub fn label(self) -> &'static str {
        match self {
            NameSource::Unknown => "unknown",
            NameSource::Xml => "xml",
            NameSource::ToolbarButton => "toolbar name table",
            NameSource::KnownId => "known id table",
            NameSource::MatchedDefaultHotkey => "matched default hotkey",
        }
    }
}

/// One command that has a key assigned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub command_id: String,
    pub command_name: Option<String>,
    /// One chord, or several for a sequence such as `D, H`.
    pub keys: Vec<KeyChord>,
    /// True when `keys` has more than one chord.
    pub sequence: bool,
    #[serde(default)]
    pub name_source: NameSource,
}

impl HotkeyBinding {
    /// Chief-style text, e.g. `Ctrl+Alt+Cmd+6` or `D, H`.
    pub fn chord_text(&self) -> String {
        format_sequence(&self.keys)
    }
}

/// The whole hotkey file (only commands that carry a key become bindings).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HotkeyFile {
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub product_version: Option<String>,
    #[serde(default)]
    pub file_version: Option<String>,
    /// Number of `<command>` records in the file, bound or not (2,284 for Daniel).
    #[serde(default)]
    pub total_commands: usize,
    pub bindings: Vec<HotkeyBinding>,
}

impl HotkeyFile {
    pub fn named_count(&self) -> usize {
        self.bindings
            .iter()
            .filter(|b| b.command_name.is_some())
            .count()
    }

    pub fn unnamed_count(&self) -> usize {
        self.bindings.len() - self.named_count()
    }
}

/// Parses `UserHotkeys.xml`. A command with an empty `<keyCodes/>` is counted
/// in `total_commands` but does not become a binding.
pub fn parse_hotkeys_xml(text: &str) -> Result<HotkeyFile, ConfigError> {
    let root = xml::parse(&normalize_newlines(text))?;
    if !root.name.eq_ignore_ascii_case("UserHotkeys") && root.child("command").is_none() {
        return Err(ConfigError::Format(format!(
            "root element is <{}>, expected <UserHotkeys>",
            root.name
        )));
    }
    let mut file = HotkeyFile::default();
    if let Some(p) = root.child("product") {
        let get = |n: &str| p.child(n).map(|e| e.text_trimmed().to_string());
        file.product = get("name");
        file.product_version = get("productVersion");
        file.file_version = get("fileVersion");
    }
    for cmd in root.children.iter().filter(|c| c.name == "command") {
        file.total_commands += 1;
        let id = cmd
            .child("id")
            .map(|e| e.text_trimmed().to_string())
            .or_else(|| cmd.attr("id").map(str::to_string))
            .unwrap_or_default();
        let codes = cmd
            .child("keyCodes")
            .map(|e| e.text_trimmed().to_string())
            .or_else(|| cmd.attr("keyCodes").map(str::to_string))
            .unwrap_or_default();
        if codes.is_empty() {
            continue;
        }
        if id.is_empty() {
            return Err(ConfigError::Format(format!(
                "command with keys '{codes}' has no <id>"
            )));
        }
        let keys = KeyChord::parse_file_sequence(&codes)
            .map_err(|e| ConfigError::Format(format!("command {id}: {e}")))?;
        let name = ["name", "commandName"]
            .iter()
            .find_map(|n| cmd.child(n).map(|e| e.text_trimmed().to_string()))
            .or_else(|| cmd.attr("name").map(str::to_string))
            .filter(|n| !n.is_empty());
        file.bindings.push(HotkeyBinding {
            command_id: id,
            name_source: if name.is_some() {
                NameSource::Xml
            } else {
                NameSource::Unknown
            },
            command_name: name,
            sequence: keys.len() > 1,
            keys,
        });
    }
    Ok(file)
}

/// How many bindings each name source supplied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveStats {
    pub total: usize,
    pub named: usize,
    pub from_xml: usize,
    pub from_toolbar: usize,
    pub from_known_ids: usize,
    pub from_default_hotkey: usize,
    pub unresolved: usize,
}

/// Fills in missing command names, in this order:
///
/// 1. names already present in the file;
/// 2. the catalog's id table (toolbar `Buttons` tables, then the hardcoded ids);
/// 3. a documented Chief default hotkey with the same chord, but only when
///    exactly one binding in the file uses that chord, exactly one default
///    command owns it, and no other binding already has that name.
pub fn resolve_names(file: &mut HotkeyFile, known: &CommandCatalog) -> ResolveStats {
    let mut taken: HashSet<String> = HashSet::new();
    for b in &mut file.bindings {
        if b.command_name.is_some() {
            if b.name_source == NameSource::Unknown {
                b.name_source = NameSource::Xml;
            }
        } else if let Some((name, src)) = known.name_for_id(&b.command_id) {
            b.command_name = Some(name.to_string());
            b.name_source = src;
        }
        if let Some(n) = &b.command_name {
            taken.insert(n.clone());
        }
    }

    let mut uses: HashMap<Vec<KeyChord>, usize> = HashMap::new();
    for b in &file.bindings {
        *uses.entry(b.keys.clone()).or_insert(0) += 1;
    }
    for b in &mut file.bindings {
        if b.command_name.is_some() || uses.get(&b.keys) != Some(&1) {
            continue;
        }
        let names = known.default_names_for(&b.keys);
        if let [name] = names.as_slice() {
            if !taken.contains(*name) {
                taken.insert((*name).to_string());
                b.command_name = Some((*name).to_string());
                b.name_source = NameSource::MatchedDefaultHotkey;
            }
        }
    }

    let mut st = ResolveStats {
        total: file.bindings.len(),
        ..ResolveStats::default()
    };
    for b in &file.bindings {
        match (&b.command_name, b.name_source) {
            (None, _) => st.unresolved += 1,
            (Some(_), NameSource::Xml) | (Some(_), NameSource::Unknown) => st.from_xml += 1,
            (Some(_), NameSource::ToolbarButton) => st.from_toolbar += 1,
            (Some(_), NameSource::KnownId) => st.from_known_ids += 1,
            (Some(_), NameSource::MatchedDefaultHotkey) => st.from_default_hotkey += 1,
        }
    }
    st.named = st.total - st.unresolved;
    st
}

/// A named binding in the shape Plan Studio's hotkey table wants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanBinding {
    pub command_id: String,
    pub command_name: String,
    /// Chief-style text (`Ctrl+Alt+Cmd+6`, `D, H`).
    pub chord_text: String,
    pub keys: Vec<KeyChord>,
}

/// Only the bindings that have a name, in file order.
pub fn to_plan_studio_bindings(file: &HotkeyFile) -> Vec<PlanBinding> {
    file.bindings
        .iter()
        .filter_map(|b| {
            b.command_name.as_ref().map(|n| PlanBinding {
                command_id: b.command_id.clone(),
                command_name: n.clone(),
                chord_text: b.chord_text(),
                keys: b.keys.clone(),
            })
        })
        .collect()
}

/// `docs/chief-hotkeys-resolved.md`: every binding, sorted by name.
pub fn resolved_markdown(file: &HotkeyFile) -> String {
    let named = file.named_count();
    let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
    for b in &file.bindings {
        if b.command_name.is_some() {
            *by_source.entry(b.name_source.label()).or_insert(0) += 1;
        }
    }
    let mut rows: Vec<&HotkeyBinding> = file.bindings.iter().collect();
    rows.sort_by(|a, b| {
        let ka = (
            a.command_name.is_none(),
            a.command_name.clone().unwrap_or_default().to_lowercase(),
            a.command_id.parse::<u64>().unwrap_or(u64::MAX),
        );
        let kb = (
            b.command_name.is_none(),
            b.command_name.clone().unwrap_or_default().to_lowercase(),
            b.command_id.parse::<u64>().unwrap_or(u64::MAX),
        );
        ka.cmp(&kb)
    });

    let mut s = String::new();
    s.push_str("# Daniel's Chief hotkeys, with recovered command names\n\n");
    s.push_str("Generated by `cargo run -p plan-config --example gen_hotkeys_md` from `docs/chief-config-raw/UserHotkeys.xml`; do not edit by hand.\n\n");
    let by = |label: &str| by_source.get(label).copied().unwrap_or(0);
    s.push_str(&format!(
        "- Bindings: {}\n- Resolved to a command name: {} ({} from the toolbar name table, {} from the known-id table, {} matched to a documented default hotkey, {} named in the XML)\n- Unresolved (id only): {}\n",
        file.bindings.len(),
        named,
        by(NameSource::ToolbarButton.label()),
        by(NameSource::KnownId.label()),
        by(NameSource::MatchedDefaultHotkey.label()),
        by(NameSource::Xml.label()),
        file.bindings.len() - named
    ));
    s.push_str(
        "\nChord text: `Ctrl` is the Control key, `Cmd` the Command key (the XML stores them as `Meta` and `Ctrl`). \
Names from the toolbar name table are exact. Names marked \"matched default hotkey\" are inferred: Daniel's chord equals the documented Chief default of exactly one command, so treat them as likely, not certain.\n\n",
    );
    s.push_str("| Chord | Command | Name source | Id |\n|---|---|---|---|\n");
    for b in rows {
        let name = b
            .command_name
            .clone()
            .unwrap_or_else(|| format!("unknown id {}", b.command_id));
        let src = if b.command_name.is_some() {
            b.name_source.label()
        } else {
            "-"
        };
        s.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            b.chord_text().replace('|', "\\|"),
            name.replace('|', "\\|"),
            src,
            b.command_id
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<UserHotkeys>
  <product><name>X</name><productVersion>1.2</productVersion><fileVersion>7</fileVersion></product>
  <command><id>1</id><keyCodes/></command>
  <command><id>2</id><keyCodes>D, H</keyCodes></command>
  <command><id>3</id><keyCodes>Meta+Ctrl+Alt+6</keyCodes><name>Wall</name></command>
</UserHotkeys>"#;

    #[test]
    fn parses_sample() {
        let f = parse_hotkeys_xml(SAMPLE).unwrap();
        assert_eq!(f.total_commands, 3);
        assert_eq!(f.bindings.len(), 2);
        assert_eq!(f.bindings[0].keys.len(), 2);
        assert!(f.bindings[0].sequence);
        assert_eq!(f.bindings[1].command_name.as_deref(), Some("Wall"));
        assert_eq!(f.bindings[1].name_source, NameSource::Xml);
        assert_eq!(f.bindings[1].chord_text(), "Ctrl+Alt+Cmd+6");
        assert_eq!(f.product_version.as_deref(), Some("1.2"));
    }

    #[test]
    fn rejects_wrong_root() {
        assert!(parse_hotkeys_xml("<Other><x/></Other>").is_err());
    }
}
