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
//! # Gang boxes and connections (stage 3)
//!
//! **Multi-gang boxes.** A class 150 object directly on a floor holds two or
//! three class 21 devices (a switch pair, a `Duplex` with a `GFCI`, a
//! `Single Pole` with a `Four Way`): 36 of them in a job with 354 loose
//! devices, 3.3" to 5.25" apart. They decode like loose devices; stage 2 left
//! them out because their parent is not a floor (34 of 36 have a position).
//!
//! **Connections.** Class 34 version 1, directly on a floor, is the dashed arc
//! between two devices (`Connect Devices`; 128 on a floor set of 354 devices).
//! Its first line record starts on a device and the object holds the arc twice:
//! two line records, start to the arc's middle point to the end (the legs of a
//! three-point arc), then the same arc sampled in 10" chords. The end of the
//! last chord lies on the other device. Checked on six projects: for 251 of 295
//! arc ends of one job the nearest device is within 0.5" (lights end on the
//! light's own point; a wall switch's arc starts 10" to 18" off its point, on
//! the symbol's edge), and the pairs are light to light (`Recessed Down Light 6`
//! to `Recessed Down Light 6`, `flush mount` to `flush mount`: the daisy chain
//! of a lighting run) and switch to light (`Single Pole`, `Three Way`, `Four Way`
//! to a light), 3-way to 3-way (travelers). The bend is the middle point's
//! distance from the chord, as in `ElectricalLayer::bend_connection`.
//! Confidence: High for the two ends, Medium for which end is the switch (the
//! importer takes the switch end as `from`).
//!
//! Not decoded: circuit numbers (nothing in a device or a connection holds one;
//! the objects that could, classes 36, 38 and 41, hold bounding boxes and pen
//! values) and device facing (the angle at +28 is axis-aligned for a third of
//! the devices and unrelated to the wall they sit on, so the importer still
//! derives it from the host wall).

use super::lines::find_edges;
use super::tree::{f64_at, strings_in, ObjectTree};

/// Electrical device class id.
pub const DEVICE: u8 = 21;
/// A group of devices sharing one box (a gang box).
pub const GANG_BOX: u8 = 150;
/// The dashed arc between two devices.
pub const CONNECTION: u8 = 34;
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

/// One decoded connection arc.
#[derive(Debug, Clone, PartialEq)]
pub struct ChiefConnection {
    pub node: usize,
    pub start: (f64, f64),
    /// The middle point of the three-point arc.
    pub mid: (f64, f64),
    pub end: (f64, f64),
}

impl ChiefConnection {
    /// Signed distance of the middle point from the chord `start -> end`,
    /// positive to the left (the `arc_bulge` of `plan_electrical::Connection`),
    /// clamped to half the chord.
    pub fn bulge(&self) -> f64 {
        let (dx, dy) = (self.end.0 - self.start.0, self.end.1 - self.start.1);
        let chord = dx.hypot(dy);
        if chord < 1e-6 {
            return 0.0;
        }
        let (nx, ny) = (-dy / chord, dx / chord);
        let m = (
            (self.start.0 + self.end.0) / 2.0,
            (self.start.1 + self.end.1) / 2.0,
        );
        ((self.mid.0 - m.0) * nx + (self.mid.1 - m.1) * ny).clamp(-chord / 2.0, chord / 2.0)
    }
}

/// Decodes the class 34 (version 1) object at `node`: the start of the first
/// line record, the end of the second (the arc's middle point) and the end of
/// the last one. `None` for an object with fewer than two line records or a
/// zero-length chord.
pub fn decode_connection(bytes: &[u8], tree: &ObjectTree, node: usize) -> Option<ChiefConnection> {
    let n = tree.node(node);
    let edges = find_edges(bytes, n.marker, n.marker + 0x40, n.end);
    let first = edges.first()?;
    let second = edges.get(1)?;
    let last = edges.last()?;
    let (start, mid, end) = (first.start(), second.start(), last.end());
    // The second record starts where the first ended (the arc's middle point).
    let joint = first.end();
    if (joint.0 - mid.0).hypot(joint.1 - mid.1) > 0.1 {
        return None;
    }
    if (start.0 - end.0).hypot(start.1 - end.1) < 1.0 {
        return None;
    }
    Some(ChiefConnection {
        node,
        start,
        mid,
        end,
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

    /// A connection arc: the legs start -> mid -> end as the first two line
    /// records, then the arc again as 10" chords ending on `end`.
    pub fn connection_obj(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> Vec<u8> {
        use crate::import::lines::tests::put_edge;
        let leg = |a: (f64, f64), b: (f64, f64)| {
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            (a.0, a.1, (b.0 - a.0) / len, (b.1 - a.1) / len, len)
        };
        sized(CONNECTION, 1, 3000, |b| {
            put_edge(b, 671, leg(start, mid));
            put_edge(b, 671 + 388, leg(mid, end));
            // Sampled chords: the arc's mid point and the last chord.
            put_edge(b, 2448, leg(start, mid));
            put_edge(b, 2448 + 388, leg(mid, end));
        })
    }

    /// A gang box holding the given device objects.
    pub fn gang_obj(children: &[Vec<u8>]) -> Vec<u8> {
        let kids: usize = children.iter().map(|c| c.len() - 1).sum();
        sized(GANG_BOX, 0, 0x100 + kids, |b| {
            let mut at = 0x100;
            for c in children {
                b[at..at + c.len() - 1].copy_from_slice(&c[1..]);
                at += c.len() - 1;
            }
        })
    }

    #[test]
    fn decodes_a_connection_arc_and_its_bulge() {
        // Switch side at (213, 100), light at (300, 150), the arc passes
        // through (250, 140): left of the chord, so the bulge is positive.
        let obj = connection_obj((213.0, 100.0), (250.0, 140.0), (300.0, 150.0));
        let tree = ObjectTree::build(&obj);
        let i = tree.of_kind(CONNECTION, 1).next().unwrap();
        let c = decode_connection(&obj, &tree, i).unwrap();
        assert_eq!(c.start, (213.0, 100.0));
        assert_eq!(c.mid, (250.0, 140.0));
        assert!((c.end.0 - 300.0).abs() < 1e-9 && (c.end.1 - 150.0).abs() < 1e-9);
        assert!(c.bulge() > 15.0 && c.bulge() < 17.0, "{}", c.bulge());
        // Reversed, the same arc bulges to the other side.
        let rev = ChiefConnection {
            node: c.node,
            start: c.end,
            mid: c.mid,
            end: c.start,
        };
        assert!((rev.bulge() + c.bulge()).abs() < 1e-9);
        // The bulge is limited to half the chord.
        let far = ChiefConnection {
            node: 0,
            start: (0.0, 0.0),
            mid: (5.0, 900.0),
            end: (10.0, 0.0),
        };
        assert_eq!(far.bulge(), 5.0);
    }

    #[test]
    fn connections_need_two_joined_records_and_a_real_chord() {
        use crate::import::lines::tests::put_edge;
        let one = sized(CONNECTION, 1, 3000, |b| {
            put_edge(b, 671, (0.0, 0.0, 1.0, 0.0, 50.0))
        });
        let tree = ObjectTree::build(&one);
        let i = tree.of_kind(CONNECTION, 1).next().unwrap();
        assert!(decode_connection(&one, &tree, i).is_none());
        // The second record does not start where the first ends.
        let gap = sized(CONNECTION, 1, 3000, |b| {
            put_edge(b, 671, (0.0, 0.0, 1.0, 0.0, 50.0));
            put_edge(b, 671 + 388, (80.0, 0.0, 1.0, 0.0, 50.0));
        });
        let tree = ObjectTree::build(&gap);
        let i = tree.of_kind(CONNECTION, 1).next().unwrap();
        assert!(decode_connection(&gap, &tree, i).is_none());
        // Start and end on the same spot.
        let loopy = connection_obj((10.0, 10.0), (30.0, 20.0), (10.0, 10.5));
        let tree = ObjectTree::build(&loopy);
        let i = tree.of_kind(CONNECTION, 1).next().unwrap();
        assert!(decode_connection(&loopy, &tree, i).is_none());
    }

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
