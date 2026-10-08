//! Electrical devices (class 21): receptacles, switches, lights, fans.
//!
//! A device is a class 21 object directly on a floor (534 of them on the four
//! floors of one job). Its strings give the kind
//! (`Duplex`, `110V`, `Outlets`, `Wall Mounted`; `Recessed Down Light 6`,
//! `Ceiling Mounted`, `Lighting`; `Single Pole`, `Switches`) and its position is
//! a pair of `f64`s whose offset depends on the layout variant (751, 755 or
//! 1181 from the `CD` byte for the plain devices; 1624 for light fixtures that
//! start with material components), so it is found by shape:
//!
//! ```text
//! +0   f64 x   plan inches, on the wall face for a wall device
//! +8   f64 y
//! +16  zeros
//! +28  f64 a   constant 4.71239 (3 pi / 2) in every device read: no rotation
//! ```
//!
//! The pair is accepted when `a` is a plausible angle and the pair lies in the
//! walls' bounding box of the floor (plus 100"). All 296 first-floor devices of
//! one job give exactly one such pair at the first match (some lights give two
//! or more matches in their later records; the first is the device).
//!
//! No height is stored per device (the bytes of 78 duplex outlets differ only in
//! GUID, id and position): the height comes from the kind. The facing of a wall
//! device is not stored either (the angle field is the constant above); the
//! importer computes it from the wall it sits on (`walls::host_of`).
//!
//! Not decoded: circuit and switch-to-load connections (the objects hold lists
//! of object ids that were not matched to devices).

use super::tree::{f64_at, strings_in, ObjectTree};

/// Electrical device class id.
pub const DEVICE: u8 = 21;
/// Where the position search starts and ends, from the `CD` byte.
const SEARCH_FROM: usize = 0x100;
const SEARCH_TO: usize = 0x1800;

/// Where a device is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mount {
    Wall,
    Ceiling,
    Other,
}

/// One decoded device.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefDevice {
    pub node: usize,
    pub x: f64,
    pub y: f64,
    /// Chief's name of the fixture (`Duplex`, `Recessed Down Light 6`).
    pub name: String,
    /// The strings after the name (`110V`, `Outlets`, `Wall Mounted`).
    pub tags: Vec<String>,
    pub mount: Mount,
}

/// The variant name of `plan_electrical::DeviceKind` for a Chief device, if
/// the name is known.
pub fn kind_of(name: &str, tags: &[String], mount: Mount) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let has = |t: &str| tags.iter().any(|x| x.eq_ignore_ascii_case(t));
    Some(if n.contains("gfci") {
        "Gfci"
    } else if n.contains("quad") {
        "Outlet110Quad"
    } else if n == "220v" || has("220v") {
        "Outlet220"
    } else if n.contains("duplex") || n.contains("outlet") || n.contains("receptacle") {
        if n.contains("floor") {
            "OutletFloor"
        } else {
            "Outlet110"
        }
    } else if n.contains("four way") || n.contains("4-way") || n.contains("four-way") {
        "Switch4Way"
    } else if n.contains("three way") || n.contains("three-way") || n.contains("3-way") {
        "Switch3Way"
    } else if n.contains("dimmer") {
        "SwitchDimmer"
    } else if n.contains("single pole") || n.contains("switch") {
        "Switch"
    } else if n.contains("recessed") {
        "RecessedCan"
    } else if n.contains("fan") && !n.contains("exhaust") {
        "CeilingFan"
    } else if n.contains("smoke") || n.contains("co/") {
        "SmokeDetector"
    } else if n.contains("carbon") {
        "CoDetector"
    } else if n.contains("thermostat") {
        "Thermostat"
    } else if n.contains("doorbell") {
        "Doorbell"
    } else if n.contains("pendant") || n.contains("chandel") {
        "PendantLight"
    } else if n.contains("sconce") {
        "WallSconce"
    } else if n.contains("data") {
        "DataJack"
    } else if n.contains("phone") {
        "PhoneJack"
    } else if n.contains("tv") || n.contains("coax") {
        "TvJack"
    } else if n.contains("panel") {
        "Panel"
    } else if n.contains("flush")
        || n.contains("exhaust")
        || n.contains("light")
        || n.contains("lantern")
        || n.contains("fixture")
        || has("lighting")
    {
        if mount == Mount::Wall {
            "WallSconce"
        } else {
            "CeilingLight"
        }
    } else {
        return None;
    })
}

/// Whether the `plan_electrical` kind is mounted on a wall.
pub fn kind_is_wall_mounted(kind: &str) -> bool {
    matches!(
        kind,
        "Outlet110"
            | "Outlet110Quad"
            | "Outlet220"
            | "Gfci"
            | "Switch"
            | "Switch3Way"
            | "Switch4Way"
            | "SwitchDimmer"
            | "WallSconce"
            | "CoDetector"
            | "Thermostat"
            | "Doorbell"
            | "DataJack"
            | "PhoneJack"
            | "TvJack"
    )
}

/// Default height above the floor of a kind, inches (the values of
/// `DeviceKind::default_height`); `ceiling` is the floor's ceiling height.
pub fn default_height(kind: &str, ceiling: f64) -> f64 {
    match kind {
        "Outlet110" | "Outlet110Quad" | "Outlet220" | "Gfci" | "DataJack" | "PhoneJack" => 12.0,
        "OutletFloor" => 0.0,
        "Switch" | "Switch3Way" | "Switch4Way" | "SwitchDimmer" | "Doorbell" | "TvJack" => 48.0,
        "Thermostat" => 52.0,
        "WallSconce" => 66.0,
        "Panel" | "CoDetector" => 60.0,
        "CeilingLight" | "RecessedCan" | "CeilingFan" | "SmokeDetector" => ceiling,
        "PendantLight" => 84.0,
        _ => 48.0,
    }
}

fn is_junk(s: &str) -> bool {
    s == "description"
        || s == "%automatic_description%"
        || s.starts_with('=')
        || s.starts_with('%')
        || s.len() < 2
        || (s.chars().next().is_some_and(|c| c.is_ascii_digit()) && s.contains(" - "))
        || s == "Main"
        || s == "Accent"
        || s == "d"
}

/// Decodes the class 21 object at tree index `node`. `bounds` is the walls'
/// bounding box of the floor `(min x, min y, max x, max y)`.
pub fn decode_device(
    bytes: &[u8],
    tree: &ObjectTree,
    node: usize,
    bounds: (f64, f64, f64, f64),
) -> Option<ChiefDevice> {
    let n = tree.node(node);
    let pad = 100.0;
    let hi = n.len().saturating_sub(40).min(SEARCH_TO);
    let at = (SEARCH_FROM..hi).find(|&o| {
        let (Some(x), Some(y), Some(a)) = (
            f64_at(bytes, n.marker + o),
            f64_at(bytes, n.marker + o + 8),
            f64_at(bytes, n.marker + o + 28),
        ) else {
            return false;
        };
        x.is_finite()
            && y.is_finite()
            && x.abs() > 1.0
            && y.abs() > 1.0
            && (bounds.0 - pad..=bounds.2 + pad).contains(&x)
            && (bounds.1 - pad..=bounds.3 + pad).contains(&y)
            && a.is_finite()
            && (-6.3..=6.3).contains(&a)
    })?;
    let x = f64_at(bytes, n.marker + at)?;
    let y = f64_at(bytes, n.marker + at + 8)?;
    let strings: Vec<String> = strings_in(bytes, n.marker, n.end.min(n.marker + 4000), 120)
        .into_iter()
        .map(|(_, s)| s)
        .filter(|s| !is_junk(s))
        .collect();
    let name = strings.first()?.clone();
    let tags: Vec<String> = strings.into_iter().skip(1).take(4).collect();
    let all = |t: &str| tags.iter().any(|x| x.eq_ignore_ascii_case(t));
    let mount = if all("Wall Mounted") {
        Mount::Wall
    } else if all("Ceiling Mounted") {
        Mount::Ceiling
    } else {
        Mount::Other
    };
    Some(ChiefDevice {
        node,
        x,
        y,
        name,
        tags,
        mount,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::import::tree::testutil::*;

    /// A device object: strings first, then the position pair at `at`.
    pub fn device_obj(at: usize, x: f64, y: f64, strs: &[&str]) -> Vec<u8> {
        sized(DEVICE, 0, 0x900, |b| {
            let mut p = 0x150;
            for s in strs {
                let c = cstr(s);
                b[p..p + c.len()].copy_from_slice(&c);
                p += c.len() + 2;
            }
            put_f64(b, at, x);
            put_f64(b, at + 8, y);
            put_f64(b, at + 28, 4.71238898038469);
        })
    }

    const BOX: (f64, f64, f64, f64) = (0.0, 0.0, 1000.0, 800.0);

    #[test]
    fn decodes_position_name_tags_and_mount() {
        let obj = device_obj(
            751,
            940.0,
            528.0,
            &[
                "description",
                "%automatic_description%",
                "Duplex",
                "110V",
                "Outlets",
                "Wall Mounted",
            ],
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(DEVICE, 0).next().unwrap();
        let d = decode_device(&obj, &tree, i, BOX).unwrap();
        assert_eq!((d.x, d.y), (940.0, 528.0));
        assert_eq!(d.name, "Duplex");
        assert_eq!(d.tags, vec!["110V", "Outlets", "Wall Mounted"]);
        assert_eq!(d.mount, Mount::Wall);
        assert_eq!(kind_of(&d.name, &d.tags, d.mount), Some("Outlet110"));
        // A second layout variant (offset 755) and a ceiling light.
        let obj = device_obj(
            755,
            123.5,
            658.25,
            &["Recessed Down Light 6", "Ceiling Mounted", "Lighting"],
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(DEVICE, 0).next().unwrap();
        let d = decode_device(&obj, &tree, i, BOX).unwrap();
        assert_eq!((d.x, d.y), (123.5, 658.25));
        assert_eq!(d.mount, Mount::Ceiling);
        assert_eq!(kind_of(&d.name, &d.tags, d.mount), Some("RecessedCan"));
    }

    #[test]
    fn rejects_positions_outside_the_walls_box_and_junk_names() {
        let obj = device_obj(751, 5000.0, 528.0, &["Duplex", "110V"]);
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(DEVICE, 0).next().unwrap();
        assert!(decode_device(&obj, &tree, i, BOX).is_none());
        let obj = device_obj(
            751,
            100.0,
            100.0,
            &["=description", "5720 - Electrical fixtures"],
        );
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(DEVICE, 0).next().unwrap();
        assert!(decode_device(&obj, &tree, i, BOX).is_none());
    }

    #[test]
    fn names_map_to_kinds_and_heights() {
        let t = |s: &str| vec![s.to_string()];
        assert_eq!(kind_of("GFCI WP", &t("110V"), Mount::Wall), Some("Gfci"));
        assert_eq!(
            kind_of("Single Pole", &t("Switches"), Mount::Wall),
            Some("Switch")
        );
        assert_eq!(
            kind_of("Three Way", &t("Switches"), Mount::Wall),
            Some("Switch3Way")
        );
        assert_eq!(
            kind_of("Four Way", &t("Switches"), Mount::Wall),
            Some("Switch4Way")
        );
        assert_eq!(
            kind_of("Three-Way Switch", &t("Switches"), Mount::Wall),
            Some("Switch3Way")
        );
        assert_eq!(
            kind_of("flush mount", &t("Lighting"), Mount::Ceiling),
            Some("CeilingLight")
        );
        assert_eq!(
            kind_of("Exhaust (light)", &t("Fans & Exhaust"), Mount::Ceiling),
            Some("CeilingLight")
        );
        assert_eq!(
            kind_of("Ceiling Fan (lights)", &t("Ceiling Fans"), Mount::Ceiling),
            Some("CeilingFan")
        );
        assert_eq!(
            kind_of("CO/Smoke Detector", &t("Fire"), Mount::Ceiling),
            Some("SmokeDetector")
        );
        assert_eq!(
            kind_of("Glass Jar Pendant 02", &t("lights"), Mount::Ceiling),
            Some("PendantLight")
        );
        assert_eq!(
            kind_of("Bryant Sconce 3", &t("bathroom"), Mount::Wall),
            Some("WallSconce")
        );
        assert_eq!(kind_of("220V", &t("220V"), Mount::Wall), Some("Outlet220"));
        assert_eq!(kind_of("Mystery", &[], Mount::Other), None);
        assert!(kind_is_wall_mounted("Gfci") && !kind_is_wall_mounted("RecessedCan"));
        assert_eq!(default_height("Outlet110", 109.0), 12.0);
        assert_eq!(default_height("RecessedCan", 109.125), 109.125);
        assert_eq!(default_height("Switch", 109.0), 48.0);
    }
}
