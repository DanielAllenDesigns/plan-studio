//! Tests against Daniel's real files (embedded) plus literal samples.

use crate::*;

const HOTKEYS_XML: &str = include_str!("../../../docs/chief-config-raw/UserHotkeys.xml");
const DEFAULT_TOOLBAR: &str =
    include_str!("../../../docs/chief-config-raw/Default Configuration.toolbar");
const TOOLBARS_DOC: &str = include_str!("../../../docs/chief-x18-toolbars.md");

fn binding<'a>(f: &'a HotkeyFile, id: &str) -> &'a HotkeyBinding {
    f.bindings
        .iter()
        .find(|b| b.command_id == id)
        .unwrap_or_else(|| panic!("no binding for id {id}"))
}

#[test]
fn xml_has_208_bindings_out_of_2284_commands() {
    let f = parse_hotkeys_xml(HOTKEYS_XML).unwrap();
    assert_eq!(f.bindings.len(), 208);
    assert_eq!(f.total_commands, 2284);
    assert_eq!(f.product.as_deref(), Some("Chief Architect Premier X18"));
    assert_eq!(f.product_version.as_deref(), Some("28.1.1.6"));
    // The file itself names nothing.
    assert_eq!(f.named_count(), 0);
}

#[test]
fn sequences_parse_as_several_chords() {
    let f = parse_hotkeys_xml(HOTKEYS_XML).unwrap();
    let hinged = binding(&f, "335");
    assert!(hinged.sequence);
    assert_eq!(hinged.keys.len(), 2);
    assert_eq!(hinged.chord_text(), "D, H");
    let tape = binding(&f, "20015");
    assert_eq!(tape.keys.len(), 3);
    assert_eq!(tape.chord_text(), "D, T, M");
    // 26 sequences in Daniel's file (setup inventory, "Notable customizations" #4).
    assert_eq!(f.bindings.iter().filter(|b| b.sequence).count(), 26);
}

#[test]
fn chord_display_uses_chief_style_text() {
    let f = parse_hotkeys_xml(HOTKEYS_XML).unwrap();
    assert_eq!(binding(&f, "172").chord_text(), "Ctrl+Alt+Cmd+4");
    assert_eq!(binding(&f, "224").chord_text(), "Ctrl+Alt+Shift+Cmd+P");
    assert_eq!(binding(&f, "101").chord_text(), "Cmd+N");
    assert_eq!(binding(&f, "106").chord_text(), "Shift+F4");
    assert_eq!(binding(&f, "359").chord_text(), "Space");
    assert_eq!(binding(&f, "590").chord_text(), "-");
    assert_eq!(binding(&f, "240").chord_text(), "Ctrl+Z");
    // The 60 Meta+Ctrl+Alt chords from the inventory.
    let four = f
        .bindings
        .iter()
        .filter(|b| b.keys.iter().any(|k| k.ctrl && k.meta && k.alt))
        .count();
    assert_eq!(four, 60);
    // Every chord round-trips through the file notation.
    for b in &f.bindings {
        let text = b
            .keys
            .iter()
            .map(KeyChord::to_file_string)
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(KeyChord::parse_file_sequence(&text).unwrap(), b.keys);
    }
}

#[test]
fn names_resolve_for_at_least_the_achieved_count_minus_five() {
    let (cfg, stats) = load_daniel_config_with_stats().unwrap();
    println!("{stats:?}");
    assert_eq!(stats.total, 208);
    assert_eq!(stats.named, cfg.hotkeys.named_count());
    // The spec asks for 120; see the README for the measured number.
    assert!(stats.named >= 120, "only {} named", stats.named);
    assert!(
        stats.named >= ACHIEVED_NAMES - 5,
        "regressed: {}",
        stats.named
    );
    assert_eq!(stats.from_toolbar, 42);
}

/// Names resolved when this test was written (see README).
const ACHIEVED_NAMES: usize = 143;

#[test]
fn spot_checks_of_resolved_names() {
    let cfg = load_daniel_config();
    let f = &cfg.hotkeys;
    let name = |id: &str| binding(f, id).command_name.clone();
    // Toolbar name table.
    assert_eq!(name("101").as_deref(), Some("New Plan"));
    assert_eq!(name("241").as_deref(), Some("Up One Floor"));
    assert_eq!(binding(f, "241").name_source, NameSource::ToolbarButton);
    // The chord for 590 is Chief's default for Zoom Out, but the toolbar table
    // says the id is Zoom In, and the id wins.
    assert_eq!(name("590").as_deref(), Some("Zoom In"));
    // Matched from documented default hotkeys.
    assert_eq!(name("335").as_deref(), Some("Hinged Door"));
    assert_eq!(
        binding(f, "335").name_source,
        NameSource::MatchedDefaultHotkey
    );
    assert_eq!(name("360").as_deref(), Some("Straight Exterior Wall"));
    assert_eq!(name("791").as_deref(), Some("Straight Interior Wall"));
    assert_eq!(name("20015").as_deref(), Some("Tape Measure"));
    // Ctrl+N is bound twice, so the chord alone cannot name either; the
    // toolbar table still does.
    assert_eq!(name("101").as_deref(), Some("New Plan"));
    assert_eq!(name("23958").as_deref(), Some("New Project"));
    // No name may be used by two commands.
    let mut seen = std::collections::HashSet::new();
    for b in &f.bindings {
        if let Some(n) = &b.command_name {
            assert!(seen.insert(n.clone()), "duplicate name {n}");
        }
    }
}

#[test]
fn plan_studio_bindings_are_the_named_ones() {
    let cfg = load_daniel_config();
    let out = to_plan_studio_bindings(&cfg.hotkeys);
    assert_eq!(out.len(), cfg.hotkeys.named_count());
    let hinged = out
        .iter()
        .find(|b| b.command_name == "Hinged Door")
        .unwrap();
    assert_eq!(hinged.chord_text, "D, H");
    assert_eq!(hinged.keys.len(), 2);
    let wall = out
        .iter()
        .find(|b| b.command_name == "Straight Interior Wall")
        .unwrap();
    assert_eq!(wall.chord_text, "Ctrl+Alt+Cmd+6");
}

#[test]
fn four_toolbar_sets_parse() {
    let cfg = load_daniel_config();
    let names: Vec<&str> = cfg.toolbars.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Default Configuration",
            "Extended Tool Configuration",
            "Space Planning Configuration",
            "Terrain Configuration"
        ]
    );
    assert_eq!(cfg.toolbars[0].toolbars.len(), 22);
    for set in &cfg.toolbars {
        assert_eq!(set.product, "Chief Architect Premier");
        assert_eq!(set.file_revision, 30);
        // Every button id in every toolbar has a label.
        for tb in &set.toolbars {
            for it in &tb.items {
                assert!(
                    it.command_name.is_some(),
                    "{}: {} has no name for id {}",
                    set.name,
                    tb.name,
                    it.command_id
                );
            }
        }
    }
}

#[test]
fn set_names_are_inferred_without_a_file_name() {
    let set = parse_toolbar(DEFAULT_TOOLBAR).unwrap();
    assert_eq!(set.name, "Default Configuration");
    assert!(parse_toolbar("not a toolbar").is_err());
}

#[test]
fn default_build_toolbar_matches_the_captured_order() {
    let cfg = load_daniel_config();
    let default = &cfg.toolbars[0];
    let build = default.find("Architectural Features").unwrap();
    assert_eq!(build, default.build_toolbar().unwrap());
    let names: Vec<&str> = build
        .items
        .iter()
        .map(|i| i.command_name.as_deref().unwrap())
        .collect();
    assert_eq!(
        &names[..10],
        [
            "Select Objects",
            "Straight Wall Tools",
            "Railing and Deck Tools",
            "Curved Wall Tools",
            "Door Tools",
            "Window Tools",
            "Cabinet Tools",
            "Electrical Tools",
            "Stair Tools",
            "Floor Tools",
        ]
    );
    assert_eq!(names.len(), 17);
    assert_eq!(names[16], "3D Solid Tools");

    // docs/chief-x18-toolbars.md "Row 2" lists the tooltip of each button's
    // current variant in the same order. For the first ten buttons, that
    // variant must be one of the flyout's members.
    let doc_names: Vec<String> = TOOLBARS_DOC
        .lines()
        .skip_while(|l| !l.starts_with("## Row 2"))
        .skip(1)
        .take_while(|l| !l.starts_with("## "))
        .filter(|l| l.starts_with("| ") && !l.starts_with("| #") && !l.starts_with("|---"))
        .map(|l| l.split('|').nth(3).unwrap().trim().to_string())
        .collect();
    assert_eq!(doc_names.len(), 30);
    assert_eq!(doc_names[0], "Select Objects");
    for (i, item) in build.items.iter().take(10).enumerate().skip(1) {
        let members: Vec<&str> = item
            .flyout
            .iter()
            .filter_map(|m| m.command_name.as_deref())
            .collect();
        assert!(
            members.contains(&doc_names[i].as_str()),
            "button {} ({}) flyout {:?} lacks '{}'",
            i + 1,
            item.command_name.as_deref().unwrap(),
            members,
            doc_names[i]
        );
    }
    // Flyout members whose default hotkey Daniel kept carry their command id.
    let door = &build.items[4];
    let hinged = door
        .flyout
        .iter()
        .find(|m| m.command_name.as_deref() == Some("Hinged Door"))
        .unwrap();
    assert_eq!(hinged.command_id, "335");
}

#[test]
fn qt_layout_gives_docks_rows_and_positions() {
    let set = parse_toolbar(DEFAULT_TOOLBAR).unwrap();
    let get = |n: &str| set.find(n).unwrap();
    assert_eq!(get("Architectural Features").placement, Placement::Top);
    assert_eq!(get("Architectural Features").row, 2);
    assert_eq!(get("Annotate").pos, 734);
    assert_eq!(get("Zoom").placement, Placement::Right);
    assert_eq!(get("Edit").placement, Placement::Bottom);
    assert!(get("File Management").visible);
    assert!(!get("Layout").visible);
    assert_eq!(get("Layout").view_types, vec![6]);
    assert!(get("Snap Toggles").view_types.is_empty());
    let order: Vec<&str> = set
        .layout_order()
        .iter()
        .take(3)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(order[0], "File Management");
    // Names in the Buttons table lose mnemonics and ellipses.
    assert_eq!(set.button_names["102"], "Open Plan");
    assert_eq!(set.button_names["20113"], "Default");
    assert_eq!(set.button_names["20183"], "3D Solid Tools");
}

#[test]
fn ini_sample_round_trips_into_preferences() {
    let ini = parse_ini(
        "[%General]\nselected color theme=Smoke 2 -Daniel Allen Design\nBackground Color=(245, 241, 239)\nCross Hair On=false\nBumping On=true\nBump Distance=5\nAutosave=1\n",
    );
    assert_eq!(preferences_from_ini(&ini), daniel_x18());
}

#[test]
fn json_round_trip() {
    let cfg = load_daniel_config();
    let text = to_json(&cfg);
    let back: DanielConfig = from_json(&text).unwrap();
    assert_eq!(back, cfg);
    let bindings = to_plan_studio_bindings(&cfg.hotkeys);
    let back: Vec<PlanBinding> = from_json(&to_json(&bindings)).unwrap();
    assert_eq!(back, bindings);
    assert!(from_json::<DanielConfig>("{").is_err());
}

#[test]
fn resolved_markdown_file_is_current() {
    let cfg = load_daniel_config();
    let generated = resolved_markdown(&cfg.hotkeys);
    let on_disk = include_str!("../../../docs/chief-hotkeys-resolved.md");
    assert!(
        on_disk == generated,
        "docs/chief-hotkeys-resolved.md is stale; regenerate with: cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md"
    );
}
