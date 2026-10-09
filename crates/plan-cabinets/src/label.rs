//! Automatic cabinet labels in Chief's four-part format (reference manual
//! pp. 655 to 657): Key, Code, Size and Door Swing.
//!
//! * **Key** says what the box is for: `B` base, `W` wall, `U` full height
//!   (utility).
//! * **Code** says what shape or fittings it has: `SB` sink base, `RB` range
//!   base, `OB` oven base, `3DB` a bank of three drawers, `FHB` a base with
//!   one full-height door, `DC`/`LC`/`LS`/`LSD`/`BC` in front of the key for
//!   diagonal, left, lazy susan, lazy susan diagonal and blind corners, `P`
//!   and `F` after it for a peninsula and a filler, `XL`/`XR`/`XLR` for
//!   extended face frames, and `OTC`/`RTC` for the tall oven and tall
//!   refrigerator cabinets. A wall cabinet with drawers carries `2D` style
//!   drawer counts after its key.
//! * **Size** is the width, then depth and height as far as they differ from
//!   the standard (base 24 deep and 34 1/2 high; wall 12 deep). Base and full
//!   height labels read width, depth, height; wall labels read width, height,
//!   depth. A wall cabinet always shows its height and a full height cabinet
//!   everything.
//! * **Door Swing** is `L` or `R`, only when every door of the cabinet swings
//!   the same way.
//!
//! Shelves, partitions, countertops, backsplashes and holes have a blank
//! automatic label, and so do the fillers the program makes itself.

use crate::cabinet::{Cabinet, CabinetKind, CabinetPreset, CornerStyle, FaceSide, SideKind};
use crate::face::{FaceItem, FaceLayout};

/// The standard depth of a base cabinet, inches (left out of its label).
const BASE_DEPTH: f64 = 24.0;
/// The standard height of a base cabinet box under its top, inches.
const BASE_HEIGHT: f64 = 34.5;
/// The standard depth of a wall cabinet, inches.
const WALL_DEPTH: f64 = 12.0;

/// What a face layout holds, as far as a label cares.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct FaceFacts {
    pub drawers: usize,
    pub left_doors: usize,
    pub right_doors: usize,
    pub auto_doors: usize,
    pub double_doors: usize,
    /// Everything else that takes up the front (openings, panels).
    pub others: usize,
    pub appliances: Vec<String>,
}

impl FaceFacts {
    pub(crate) fn of(face: &FaceLayout) -> Self {
        let mut f = FaceFacts::default();
        for item in &face.items {
            f.add(item);
        }
        f
    }

    fn add(&mut self, item: &FaceItem) {
        match item {
            FaceItem::Separation { .. } => {}
            FaceItem::Drawer { .. } => self.drawers += 1,
            FaceItem::DoubleDrawer { .. } => self.drawers += 2,
            FaceItem::DoorLeft { .. } => self.left_doors += 1,
            FaceItem::DoorRight { .. } => self.right_doors += 1,
            FaceItem::DoorAuto { .. } | FaceItem::DoorAutoLeft { .. } => self.auto_doors += 1,
            FaceItem::DoubleDoor { .. } => self.double_doors += 1,
            FaceItem::Appliance { name, .. } => self.appliances.push(name.clone()),
            FaceItem::HorizontalLayout { cells, .. } => {
                for c in cells {
                    self.add(&c.item);
                }
            }
            FaceItem::VerticalLayout { items, .. } => {
                for i in items {
                    self.add(i);
                }
            }
            FaceItem::Custom { item, .. } => self.add(item),
            _ => self.others += 1,
        }
    }

    pub(crate) fn doors(&self) -> usize {
        self.left_doors + self.right_doors + self.auto_doors + self.double_doors
    }

    fn has_appliance(&self, names: &[&str]) -> bool {
        self.appliances
            .iter()
            .any(|a| names.iter().any(|n| a.eq_ignore_ascii_case(n)))
    }

    /// `L` or `R` when every door swings the same way (an Auto door picks its
    /// side by position and a pair swings both ways, so neither counts).
    fn swing(&self) -> Option<char> {
        if self.auto_doors > 0 || self.double_doors > 0 {
            return None;
        }
        match (self.left_doors, self.right_doors) {
            (l, 0) if l > 0 => Some('L'),
            (0, r) if r > 0 => Some('R'),
            _ => None,
        }
    }
}

/// A dimension as whole inches, or with up to two trimmed decimals.
fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/// Does the label of this kind stay blank unless the user types one?
pub fn label_is_blank(cabinet: &Cabinet) -> bool {
    cabinet.auto_filler
        || matches!(
            cabinet.kind,
            CabinetKind::Shelf
                | CabinetKind::Partition
                | CabinetKind::CustomCountertop
                | CabinetKind::CustomBacksplash
                | CabinetKind::CounterHole
        )
}

/// The key letter: `B`, `W` or `U`.
pub fn key_of(kind: CabinetKind) -> &'static str {
    if kind.is_wall_like() {
        "W"
    } else if matches!(
        kind,
        CabinetKind::FullHeight | CabinetKind::FullHeightFiller
    ) {
        "U"
    } else {
        "B"
    }
}

/// The extended-stile code: `XL`, `XR`, `XLR` or nothing.
fn stile_code(c: &Cabinet) -> &'static str {
    match (c.stile_ext_left > 0.0, c.stile_ext_right > 0.0) {
        (true, true) => "XLR",
        (true, false) => "XL",
        (false, true) => "XR",
        (false, false) => "",
    }
}

/// The box height without the countertop, which is what "standard" means.
fn box_height(c: &Cabinet) -> f64 {
    c.height - c.countertop.map_or(0.0, |t| t.thickness)
}

/// Key plus code, in the order Chief writes them: corner shape letters before
/// the key, then the key with its fittings, then `P`, `F` and the stile codes.
fn code_of(c: &Cabinet, facts: &FaceFacts) -> String {
    let key = key_of(c.kind);
    let corner = c.corner.unwrap_or_default();
    let shape = match c.kind {
        CabinetKind::CornerBase | CabinetKind::CornerWall => {
            match (corner.style, corner.lazy_susan) {
                (CornerStyle::Diagonal, true) => "LSD",
                (CornerStyle::Diagonal, false) => "DC",
                (CornerStyle::PieCut, true) => "LS",
                (CornerStyle::PieCut, false) => "LC",
            }
        }
        CabinetKind::BlindBase | CabinetKind::BlindWall => "BC",
        _ => "",
    };
    // The key with what is built into it.
    let core = match c.kind {
        CabinetKind::Base => {
            if facts.has_appliance(&["Sink"]) {
                "SB".to_string()
            } else if facts.has_appliance(&["Range", "Cooktop"]) {
                "RB".to_string()
            } else if facts.has_appliance(&["Oven"]) {
                "OB".to_string()
            } else if facts.drawers > 0 && facts.doors() == 0 && facts.others == 0 {
                format!("{}DB", facts.drawers)
            } else if facts.drawers == 0
                && facts.doors() == 1
                && facts.double_doors == 0
                && facts.others == 0
                && facts.appliances.is_empty()
            {
                "FHB".to_string()
            } else {
                key.to_string()
            }
        }
        CabinetKind::Wall if facts.drawers > 0 => format!("{key}{}D", facts.drawers),
        CabinetKind::FullHeight => {
            if facts.has_appliance(&["Oven", "Range", "Wall Oven"]) {
                "OTC".to_string()
            } else if facts.has_appliance(&["Refrigerator"]) {
                "RTC".to_string()
            } else {
                key.to_string()
            }
        }
        _ => key.to_string(),
    };
    let peninsula = matches!(c.kind, CabinetKind::Base)
        && c.side_kind(FaceSide::Back) == SideKind::CustomFace
        && c.side_face(FaceSide::Back)
            .is_some_and(|s| FaceFacts::of(&s.layout).doors() > 0);
    let tail = if c.kind.is_filler() {
        "F"
    } else if peninsula {
        "P"
    } else {
        ""
    };
    format!("{shape}{core}{tail}{}", stile_code(c))
}

/// The size part: see the module docs.
fn size_of(c: &Cabinet) -> String {
    let (w, d, h) = (c.width, c.depth, box_height(c));
    let corner_like = c.kind.is_corner();
    let key = key_of(c.kind);
    match key {
        "W" => {
            // Width, height (always), depth when it is not the standard one.
            let std_depth = if corner_like { w } else { WALL_DEPTH };
            let mut s = format!("{}{}", num(w), num(c.height));
            if !near(d, std_depth) {
                s.push_str(&num(d));
            }
            s
        }
        "U" => format!("{}{}{}", num(w), num(d), num(c.height)),
        _ => {
            // Base: width, depth, height; depth and height only when they
            // are not the standard ones (the depth rides along when only the
            // height differs, because the order is fixed).
            let std_depth = if corner_like { w } else { BASE_DEPTH };
            let h_odd = !near(h, BASE_HEIGHT);
            let mut s = num(w);
            if !near(d, std_depth) || h_odd {
                s.push_str(&num(d));
            }
            if h_odd {
                s.push_str(&num(h));
            }
            s
        }
    }
}

/// Chief's automatic label for `cabinet`. A cabinet made from a library type
/// keeps the type's own letters (`VB30`, `PN2484`).
pub fn auto_label(cabinet: &Cabinet) -> String {
    if label_is_blank(cabinet) {
        return String::new();
    }
    if let Some(p) = cabinet.preset {
        return preset_label(cabinet, p);
    }
    let c = cabinet;
    let facts = FaceFacts::of(&c.face);
    match c.kind {
        CabinetKind::Soffit | CabinetKind::SoffitPolygon => {
            return format!("SO{}", num(c.width));
        }
        CabinetKind::Base => {
            // An appliance bay (dishwasher, range, ...) is labelled as the
            // appliance; a range bay is Chief's range base.
            if let Some(name) = c.appliance.as_deref() {
                let w = num(c.width);
                return match name.to_ascii_lowercase().as_str() {
                    "range" => format!("RB{w}"),
                    "oven" => format!("OB{w}"),
                    "dishwasher" => format!("DW{w}"),
                    "refrigerator" | "fridge" => format!("REF{w}"),
                    "microwave" => format!("MW{w}"),
                    other => {
                        format!(
                            "{}{w}",
                            other.chars().take(3).collect::<String>().to_uppercase()
                        )
                    }
                };
            }
        }
        _ => {}
    }
    let mut s = format!("{}{}", code_of(c, &facts), size_of(c));
    // The shape letters of a corner or blind cabinet come before the key; for
    // those the key sits right after them, which `code_of` already did.
    if !c.kind.is_filler() {
        if let Some(l) = facts.swing() {
            s.push(l);
        }
    }
    s
}

/// The label of a cabinet made from a library type.
fn preset_label(c: &Cabinet, p: CabinetPreset) -> String {
    let (w, h) = (num(c.width), num(c.height));
    match c.kind {
        k if k.is_wall_like() || k == CabinetKind::FullHeight => format!("{}{w}{h}", p.code()),
        _ => format!("{}{w}", p.code()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cabinet::{BlindSide, CornerSpec};
    use crate::face::FaceCell;

    fn with_face(mut c: Cabinet, items: Vec<FaceItem>) -> Cabinet {
        c.face = FaceLayout {
            items,
            frame_width: 0.75,
        };
        c
    }

    fn door_only(c: Cabinet, door: FaceItem) -> Cabinet {
        with_face(c, vec![door])
    }

    #[test]
    fn the_label_table_matches_the_manual() {
        let sep = FaceItem::Separation { height: 0.75 };
        let mut diag = Cabinet::corner_base(36.0);
        diag.corner = Some(CornerSpec {
            style: CornerStyle::Diagonal,
            ..CornerSpec::default()
        });
        let mut lazy = Cabinet::corner_base(36.0);
        lazy.corner = Some(CornerSpec {
            style: CornerStyle::PieCut,
            lazy_susan: true,
            ..CornerSpec::default()
        });
        let mut lc = Cabinet::corner_base(36.0);
        lc.corner = Some(CornerSpec {
            style: CornerStyle::PieCut,
            lazy_susan: false,
            ..CornerSpec::default()
        });
        let mut dcw = Cabinet::corner_wall(24.0);
        dcw.corner = Some(CornerSpec {
            style: CornerStyle::Diagonal,
            ..CornerSpec::default()
        });
        dcw.height = 36.0;
        dcw.face = FaceLayout {
            items: vec![FaceItem::DoorLeft { height: 0.0 }],
            frame_width: 0.75,
        };
        let mut bcw = Cabinet::blind_wall(24.0, 12.0, BlindSide::Left);
        bcw.height = 36.0;
        bcw.face = FaceLayout {
            items: vec![FaceItem::DoorRight { height: 0.0 }],
            frame_width: 0.75,
        };
        let mut otc = Cabinet::full_height(36.0);
        otc.height = 90.0;
        otc.face = FaceLayout {
            items: vec![FaceItem::Appliance {
                height: 0.0,
                name: "Oven".into(),
            }],
            frame_width: 0.75,
        };
        let mut rtc = Cabinet::full_height(36.0);
        rtc.face = FaceLayout {
            items: vec![FaceItem::Appliance {
                height: 0.0,
                name: "Refrigerator".into(),
            }],
            frame_width: 0.75,
        };
        let mut xl = Cabinet::base(24.0);
        xl.stile_ext_left = 1.5;
        let mut xlr = Cabinet::base(24.0);
        xlr.stile_ext_left = 1.5;
        xlr.stile_ext_right = 1.5;
        let mut tall_base = Cabinet::base(24.0);
        tall_base.height = 42.0;
        let mut deep_base = Cabinet::base(30.0);
        deep_base.depth = 30.0;
        let mut wall_drawers = Cabinet::wall(30.0);
        wall_drawers.face = FaceLayout {
            items: vec![
                FaceItem::Drawer { height: 6.0 },
                sep.clone(),
                FaceItem::Drawer { height: 6.0 },
            ],
            frame_width: 0.75,
        };
        let mut peninsula = Cabinet::base(24.0);
        peninsula.set_side_face(
            FaceSide::Back,
            SideKind::CustomFace,
            FaceLayout::single_door(),
        );
        let mut range = Cabinet::base(30.0);
        range.face = FaceLayout {
            items: vec![FaceItem::Appliance {
                height: 0.0,
                name: "Range".into(),
            }],
            frame_width: 0.75,
        };
        let cases: Vec<(&str, Cabinet, &str)> = vec![
            ("default base", Cabinet::base(24.0), "B24"),
            (
                "three drawers",
                {
                    let mut c = Cabinet::base(24.0);
                    c.face = FaceLayout::drawer_bank(3);
                    c
                },
                "3DB24",
            ),
            (
                "sink base, right door",
                {
                    let mut c = Cabinet::sink_base(24.0);
                    c.face = FaceLayout {
                        items: vec![
                            FaceItem::Appliance {
                                height: 6.0,
                                name: "Sink".into(),
                            },
                            FaceItem::DoorRight { height: 0.0 },
                        ],
                        frame_width: 0.75,
                    };
                    c
                },
                "SB24R",
            ),
            ("sink base, double door", Cabinet::sink_base(36.0), "SB36"),
            ("range base", range, "RB30"),
            ("wall 30 x 30", Cabinet::wall(30.0), "W3030"),
            (
                "wall 36 high",
                {
                    let mut c = Cabinet::wall(24.0);
                    c.height = 36.0;
                    c
                },
                "W2436",
            ),
            ("wall with two drawers", wall_drawers, "W2D3030"),
            ("blind wall right door", bcw, "BCW2436R"),
            ("diagonal corner wall left door", dcw, "DCW2436L"),
            ("tall oven", otc, "OTC362490"),
            ("tall refrigerator", rtc, "RTC362484"),
            ("full height", Cabinet::full_height(24.0), "U242484"),
            (
                "base filler",
                Cabinet::filler(CabinetKind::BaseFiller, 3.0),
                "BF3",
            ),
            (
                "wall filler",
                Cabinet::filler(CabinetKind::WallFiller, 3.0),
                "WF330",
            ),
            (
                "full height filler",
                Cabinet::filler(CabinetKind::FullHeightFiller, 3.0),
                "UF32484",
            ),
            ("extended left", xl, "BXL24"),
            ("extended both", xlr, "BXLR24"),
            ("tall base box", tall_base, "B242440.5"),
            ("deep base", deep_base, "B3030"),
            (
                "one door, no drawer",
                door_only(Cabinet::base(18.0), FaceItem::DoorLeft { height: 0.0 }),
                "FHB18L",
            ),
            ("peninsula", peninsula, "BP24"),
            ("diagonal corner base", diag, "DCB36"),
            ("lazy susan corner base", lazy, "LSB36"),
            ("left corner base", lc, "LCB36"),
        ];
        assert!(cases.len() >= 20);
        for (what, cab, want) in cases {
            assert_eq!(auto_label(&cab), want, "{what}");
        }
    }

    #[test]
    fn shelves_partitions_tops_and_auto_fillers_are_blank() {
        for kind in [
            CabinetKind::Shelf,
            CabinetKind::Partition,
            CabinetKind::CustomCountertop,
            CabinetKind::CustomBacksplash,
        ] {
            assert_eq!(auto_label(&Cabinet::new(kind, 24.0)), "", "{kind:?}");
        }
        let mut f = Cabinet::filler(CabinetKind::BaseFiller, 2.0);
        f.auto_filler = true;
        assert_eq!(auto_label(&f), "");
        assert_eq!(f.display_label(), "");
    }

    #[test]
    fn a_mixed_swing_or_an_auto_door_has_no_letter() {
        let mixed = with_face(
            Cabinet::base(36.0),
            vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: None,
                    },
                    FaceCell {
                        item: FaceItem::DoorRight { height: 0.0 },
                        width: None,
                    },
                ],
            }],
        );
        // Two doors, one each way, and no drawer: not a single full-height door.
        assert_eq!(auto_label(&mixed), "B36");
        let both_left = with_face(
            Cabinet::base(36.0),
            vec![FaceItem::HorizontalLayout {
                height: 0.0,
                cells: vec![
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: None,
                    },
                    FaceCell {
                        item: FaceItem::DoorLeft { height: 0.0 },
                        width: None,
                    },
                ],
            }],
        );
        assert_eq!(auto_label(&both_left), "B36L");
        // The default base face has an Auto door: no letter.
        assert_eq!(auto_label(&Cabinet::base(24.0)), "B24");
    }

    #[test]
    fn library_types_keep_their_own_letters() {
        assert_eq!(
            auto_label(&Cabinet::from_preset(CabinetPreset::Vanity, 30.0)),
            "VB30"
        );
        assert_eq!(
            auto_label(&Cabinet::from_preset(CabinetPreset::Pantry, 24.0)),
            "PN2484"
        );
    }

    #[test]
    fn suppress_label_hides_the_displayed_label_only() {
        let mut c = Cabinet::base(24.0);
        assert_eq!(c.display_label(), "B24");
        c.suppress_label = true;
        assert_eq!(c.display_label(), "");
        assert_eq!(auto_label(&c), "B24");
    }
}
