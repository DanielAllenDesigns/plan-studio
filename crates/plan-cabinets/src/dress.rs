//! What a cabinet carries beyond its box: the plan fill (Fill Style tab), the
//! object information, the accessories (front pilasters and feet) and the
//! component list (Components tab).

use serde::{Deserialize, Serialize};

use crate::cabinet::{Cabinet, CabinetKind, HandleStyle, SideKind};
use crate::face::FaceItem;

/// Spacing of the adjustable shelves behind a door or in an opening, inches.
pub const SHELF_SPACING: f64 = 13.0;
/// Most adjustable shelves counted (and drawn) in one bay.
pub const MAX_SHELVES: usize = 6;

/// How the plan view fills a cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FillPattern {
    /// No fill: the outline only.
    #[default]
    None,
    Solid,
    /// Parallel lines at 45 degrees.
    Hatch,
    CrossHatch,
}

impl FillPattern {
    pub const ALL: [FillPattern; 4] = [
        FillPattern::None,
        FillPattern::Solid,
        FillPattern::Hatch,
        FillPattern::CrossHatch,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FillPattern::None => "None",
            FillPattern::Solid => "Solid",
            FillPattern::Hatch => "Hatch",
            FillPattern::CrossHatch => "Cross Hatch",
        }
    }
}

/// The Fill Style tab: a pattern, its colour and how opaque it is. The fill
/// is drawn in the plan view only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanFill {
    pub pattern: FillPattern,
    pub color: [u8; 3],
    /// 0.1 (faint) to 1.0 (solid).
    pub alpha: f32,
    /// Distance between hatch lines, inches.
    pub spacing: f64,
}

impl Default for PlanFill {
    fn default() -> Self {
        Self {
            pattern: FillPattern::None,
            color: [180, 180, 180],
            alpha: 0.5,
            spacing: 3.0,
        }
    }
}

impl PlanFill {
    /// Whether the fill draws anything.
    pub fn is_visible(&self) -> bool {
        self.pattern != FillPattern::None
    }

    /// The hatch lines of the pattern across `ring` (a plan polygon), as
    /// segments: parallel lines at 45 degrees, plus the 135 degree set for a
    /// cross hatch. Solid and None give no lines.
    pub fn hatch_lines(
        &self,
        ring: &[plan_core::geometry::Point],
    ) -> Vec<[plan_core::geometry::Point; 2]> {
        let angles: &[f64] = match self.pattern {
            FillPattern::Hatch => &[std::f64::consts::FRAC_PI_4],
            FillPattern::CrossHatch => &[
                std::f64::consts::FRAC_PI_4,
                3.0 * std::f64::consts::FRAC_PI_4,
            ],
            _ => &[],
        };
        let mut out = Vec::new();
        for a in angles {
            out.extend(scan_lines(ring, *a, self.spacing.max(0.5)));
        }
        out
    }
}

/// Lines at angle `a` spaced `gap` apart, clipped to the polygon `ring`
/// (even-odd, so concave outlines work).
fn scan_lines(
    ring: &[plan_core::geometry::Point],
    a: f64,
    gap: f64,
) -> Vec<[plan_core::geometry::Point; 2]> {
    use plan_core::geometry::Point;
    if ring.len() < 3 {
        return Vec::new();
    }
    let u = Point::new(a.cos(), a.sin());
    let v = Point::new(-a.sin(), a.cos());
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in ring {
        let t = p.dot(v);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let mut out = Vec::new();
    let mut t = (lo / gap).ceil() * gap;
    while t < hi {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..ring.len() {
            let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
            let (tp, tq) = (p.dot(v), q.dot(v));
            if (tp <= t) != (tq <= t) {
                let f = (t - tp) / (tq - tp);
                xs.push((p + (q - p) * f).dot(u));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.windows(2).step_by(2) {
            out.push([u * pair[0] + v * t, u * pair[1] + v * t]);
        }
        t += gap;
    }
    out
}

/// The Object Information tab: what the cabinet is as a product.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectInfo {
    pub manufacturer: String,
    pub model: String,
    pub description: String,
    pub notes: String,
}

impl ObjectInfo {
    pub fn is_empty(&self) -> bool {
        self.manufacturer.is_empty()
            && self.model.is_empty()
            && self.description.is_empty()
            && self.notes.is_empty()
    }
}

/// A front pilaster: a vertical strip at the front edge of the cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PilasterStyle {
    #[default]
    None,
    Plain,
    Fluted,
}

impl PilasterStyle {
    pub const ALL: [PilasterStyle; 3] = [
        PilasterStyle::None,
        PilasterStyle::Plain,
        PilasterStyle::Fluted,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PilasterStyle::None => "None",
            PilasterStyle::Plain => "Plain Pilaster",
            PilasterStyle::Fluted => "Fluted Pilaster",
        }
    }
}

/// Feet under a base cabinet (they stand in place of the toe kick board).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootStyle {
    #[default]
    None,
    Block,
    Bun,
    Bracket,
}

impl FootStyle {
    pub const ALL: [FootStyle; 4] = [
        FootStyle::None,
        FootStyle::Block,
        FootStyle::Bun,
        FootStyle::Bracket,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FootStyle::None => "None",
            FootStyle::Block => "Block Foot",
            FootStyle::Bun => "Bun Foot",
            FootStyle::Bracket => "Bracket Foot",
        }
    }
}

/// The Accessories tab.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accessories {
    pub pilaster: PilasterStyle,
    pub pilaster_left: bool,
    pub pilaster_right: bool,
    pub pilaster_width: f64,
    pub feet: FootStyle,
    /// Side of a square foot, inches.
    pub foot_size: f64,
}

impl Default for Accessories {
    fn default() -> Self {
        Self {
            pilaster: PilasterStyle::None,
            pilaster_left: true,
            pilaster_right: true,
            pilaster_width: 2.5,
            feet: FootStyle::None,
            foot_size: 3.0,
        }
    }
}

impl Accessories {
    /// Whether anything is added to the box.
    pub fn is_default(&self) -> bool {
        *self == Accessories::default()
    }

    /// The pilasters the cabinet carries: (left, right).
    pub fn pilasters(&self) -> (bool, bool) {
        if self.pilaster == PilasterStyle::None {
            (false, false)
        } else {
            (self.pilaster_left, self.pilaster_right)
        }
    }
}

/// One line of the Components tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub count: usize,
    /// Width x height x thickness (or the nearest equivalent), as text.
    pub size: String,
    pub material: String,
}

/// Inches to feet-less eighths text, `34 1/2`.
pub fn fmt_in(v: f64) -> String {
    let eighths = (v * 8.0).round() as i64;
    let (whole, rest) = (eighths / 8, eighths % 8);
    if rest == 0 {
        return whole.to_string();
    }
    let g = match rest {
        2 | 6 => 2,
        4 => 4,
        _ => 1,
    };
    let (n, d) = (rest / g, 8 / g);
    if whole == 0 {
        format!("{n}/{d}")
    } else {
        format!("{whole} {n}/{d}")
    }
}

fn dims(w: f64, h: f64, t: f64) -> String {
    format!("{} x {} x {}", fmt_in(w), fmt_in(h), fmt_in(t))
}

/// Adjustable shelves a bay `h` inches tall carries.
pub fn shelf_count(h: f64) -> usize {
    ((h / SHELF_SPACING).floor() as usize).min(MAX_SHELVES)
}

/// What the cabinet is made of: the parts, how many of each, their sizes and
/// materials (Components tab). Custom tops, fillers and the odd kinds list
/// what they have.
pub fn components(c: &Cabinet) -> Vec<Component> {
    let mut out = Vec::new();
    let m = &c.materials;
    let mut add = |name: &str, count: usize, size: String, material: &str| {
        if count > 0 {
            out.push(Component {
                name: name.to_string(),
                count,
                size,
                material: material.to_string(),
            });
        }
    };
    let panel = 0.75;
    match c.kind {
        CabinetKind::Soffit | CabinetKind::SoffitPolygon => {
            add(
                "Soffit",
                1,
                dims(c.width, c.height, c.depth),
                m.carcass.name(),
            );
            return out;
        }
        CabinetKind::Shelf => {
            add("Shelf", 1, dims(c.width, c.depth, panel), m.carcass.name());
            return out;
        }
        CabinetKind::Partition => {
            add(
                "Partition",
                1,
                dims(c.depth, c.height, panel),
                m.carcass.name(),
            );
            return out;
        }
        CabinetKind::CustomCountertop | CabinetKind::CustomBacksplash => {
            let n = c.custom.as_ref().map_or(0, |t| t.outline.len());
            add(
                if c.kind == CabinetKind::CustomCountertop {
                    "Custom countertop"
                } else {
                    "Custom backsplash"
                },
                1,
                format!("{n} corners x {}", fmt_in(c.height)),
                if c.kind == CabinetKind::CustomCountertop {
                    m.countertop.name()
                } else {
                    m.backsplash.name()
                },
            );
            return out;
        }
        CabinetKind::CounterHole => return out,
        _ => {}
    }
    let toe = c.toe_kick.map_or(0.0, |t| t.height);
    let top = c.countertop.map_or(0.0, |t| t.thickness);
    let box_h = (c.height - toe - top).max(0.0);
    let carcass = m.carcass.name();
    if !c.kind.is_filler() && c.appliance.is_none() {
        let plain = |s| c.side_kind(s) == SideKind::Plain;
        let sides = usize::from(plain(crate::cabinet::FaceSide::Left))
            + usize::from(plain(crate::cabinet::FaceSide::Right));
        add("Box side", sides, dims(c.depth, box_h, panel), carcass);
        let finished = 2 - sides;
        add(
            "Finished end panel",
            finished,
            dims(c.depth, box_h, panel),
            m.door.name(),
        );
        add(
            "Box bottom",
            1,
            dims((c.width - 2.0 * panel).max(0.0), c.depth, panel),
            carcass,
        );
        if !c.kind.is_base_like() {
            add(
                "Box top",
                1,
                dims((c.width - 2.0 * panel).max(0.0), c.depth, panel),
                carcass,
            );
        }
        if plain(crate::cabinet::FaceSide::Back) {
            add("Back", 1, dims(c.width, box_h, panel), carcass);
        }
    }
    if let Ok(leaves) = c.face.resolve(c.face_height(), c.face_width()) {
        let (mut doors, mut drawers, mut panels, mut separations, mut apps, mut shelves) =
            (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        let mut door_sizes: Vec<(f64, f64)> = Vec::new();
        let mut drawer_sizes: Vec<(f64, f64)> = Vec::new();
        for leaf in &leaves {
            let (_, _, w, h) = leaf.rect;
            match &leaf.item {
                FaceItem::DoorAuto { .. }
                | FaceItem::DoorLeft { .. }
                | FaceItem::DoorRight { .. } => {
                    doors += 1;
                    door_sizes.push((w, h));
                    shelves += shelf_count(h);
                }
                FaceItem::DoubleDoor { .. } => {
                    doors += 2;
                    door_sizes.push((w / 2.0, h));
                    door_sizes.push((w / 2.0, h));
                    shelves += shelf_count(h);
                }
                FaceItem::Drawer { .. } => {
                    drawers += 1;
                    drawer_sizes.push((w, h));
                }
                FaceItem::Panel { .. } => panels += 1,
                FaceItem::Separation { .. } => separations += 1,
                FaceItem::Appliance { .. } => apps += 1,
                FaceItem::Opening { .. } => shelves += shelf_count(h),
                FaceItem::HorizontalLayout { .. } => {}
            }
        }
        let first =
            |v: &[(f64, f64)], t: f64| v.first().map_or(String::new(), |(w, h)| dims(*w, *h, t));
        let style = &c.door_style;
        add(
            &format!("Door ({} - {})", style.name, style.profile.name()),
            doors,
            first(&door_sizes, style.thickness),
            m.door.name(),
        );
        add(
            &format!(
                "Drawer front ({} - {})",
                c.drawer_style.name,
                c.drawer_style.profile.name()
            ),
            drawers,
            first(&drawer_sizes, c.drawer_style.thickness),
            m.drawer.name(),
        );
        add("Panel", panels, String::new(), m.door.name());
        add("Face frame rail", separations, String::new(), m.door.name());
        add("Appliance", apps, String::new(), "Metal");
        add(
            "Adjustable shelf",
            shelves,
            dims(
                (c.width - 2.0 * panel).max(0.0),
                (c.depth - 1.5).max(0.0),
                panel,
            ),
            carcass,
        );
        let door_pulls = if style.handle == HandleStyle::None {
            0
        } else {
            doors
        };
        let drawer_pulls = if c.drawer_style.handle == HandleStyle::None {
            0
        } else {
            drawers
        };
        add(
            &format!("Door handle ({})", style.handle.name()),
            door_pulls,
            String::new(),
            "Metal",
        );
        add(
            &format!("Drawer handle ({})", c.drawer_style.handle.name()),
            drawer_pulls,
            String::new(),
            "Metal",
        );
    }
    if let Some(t) = c.toe_kick.filter(|t| t.height > 0.0) {
        if c.accessories.feet == FootStyle::None {
            add(
                "Toe kick",
                1,
                dims(c.width, t.height, panel),
                m.toe_kick.name(),
            );
        }
    }
    let feet = c.accessories.feet;
    if feet != FootStyle::None {
        let s = c.accessories.foot_size;
        add(
            feet.name(),
            4,
            dims(s, c.toe_kick.map_or(4.0, |t| t.height), s),
            m.toe_kick.name(),
        );
    }
    let (pl, pr) = c.accessories.pilasters();
    add(
        c.accessories.pilaster.name(),
        usize::from(pl) + usize::from(pr),
        dims(c.accessories.pilaster_width, box_h, panel),
        m.door.name(),
    );
    if let Some(t) = c.countertop {
        add(
            "Countertop",
            1,
            dims(
                c.width + t.overhang_sides * 2.0,
                c.depth + t.overhang_front + t.overhang_back,
                t.thickness,
            ),
            m.countertop.name(),
        );
    }
    if let Some(b) = c.backsplash {
        add(
            "Backsplash",
            1,
            dims(c.width, b.height, b.thickness),
            m.backsplash.name(),
        );
    }
    for mo in &c.moldings {
        add(
            mo.name(),
            1,
            dims(c.width, mo.height, mo.projection),
            m.molding.name(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    #[test]
    fn eighths_read_like_chief() {
        assert_eq!(fmt_in(34.5), "34 1/2");
        assert_eq!(fmt_in(0.75), "3/4");
        assert_eq!(fmt_in(24.0), "24");
        assert_eq!(fmt_in(1.625), "1 5/8");
    }

    #[test]
    fn hatch_lines_stay_inside_the_outline() {
        let ring = vec![
            Point::new(0.0, 0.0),
            Point::new(24.0, 0.0),
            Point::new(24.0, 24.0),
            Point::new(0.0, 24.0),
        ];
        let f = PlanFill {
            pattern: FillPattern::Hatch,
            ..PlanFill::default()
        };
        let lines = f.hatch_lines(&ring);
        assert!(lines.len() > 10);
        for [a, b] in &lines {
            for p in [a, b] {
                assert!(p.x > -1e-6 && p.x < 24.0 + 1e-6 && p.y > -1e-6 && p.y < 24.0 + 1e-6);
            }
        }
        let cross = PlanFill {
            pattern: FillPattern::CrossHatch,
            ..f
        };
        assert!(cross.hatch_lines(&ring).len() > lines.len());
        let none = PlanFill::default();
        assert!(none.hatch_lines(&ring).is_empty());
        let solid = PlanFill {
            pattern: FillPattern::Solid,
            ..f
        };
        assert!(solid.hatch_lines(&ring).is_empty());
    }

    #[test]
    fn components_count_the_parts_of_a_base_cabinet() {
        let c = Cabinet::base(24.0);
        let parts = components(&c);
        let named = |n: &str| parts.iter().find(|p| p.name.starts_with(n));
        assert_eq!(named("Box side").unwrap().count, 2);
        assert_eq!(named("Back").unwrap().count, 1);
        assert!(named("Countertop").is_some());
        assert!(named("Toe kick").is_some());
        let doors = named("Door (").map(|d| d.count).unwrap_or(0);
        let drawers = named("Drawer front").map(|d| d.count).unwrap_or(0);
        assert!(doors + drawers > 0);
        // Feet replace the toe kick board.
        let mut f = c.clone();
        f.accessories.feet = FootStyle::Bun;
        let parts = components(&f);
        assert!(parts.iter().all(|p| !p.name.starts_with("Toe kick")));
        assert_eq!(
            parts.iter().find(|p| p.name == "Bun Foot").unwrap().count,
            4
        );
    }

    #[test]
    fn soffits_shelves_and_fillers_list_what_they_have() {
        let s = components(&Cabinet::new(CabinetKind::Soffit, 36.0));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "Soffit");
        let p = components(&Cabinet::new(CabinetKind::Partition, 12.0));
        assert_eq!(p[0].name, "Partition");
        let f = components(&Cabinet::filler(CabinetKind::BaseFiller, 3.0));
        assert!(f.iter().all(|c| c.name != "Box side"));
    }

    fn triangles(c: &Cabinet) -> usize {
        crate::meshes(c).iter().map(|m| m.indices.len() / 3).sum()
    }

    #[test]
    fn feet_replace_the_toe_kick_and_pilasters_add_boards() {
        let base = Cabinet::base(30.0);
        let plain = triangles(&base);
        let mut feet = base.clone();
        feet.accessories.feet = FootStyle::Block;
        let with_feet = triangles(&feet);
        // Four blocks instead of one board.
        assert!(with_feet > plain, "{with_feet} vs {plain}");
        for style in [FootStyle::Bun, FootStyle::Bracket] {
            let mut f = base.clone();
            f.accessories.feet = style;
            assert!(triangles(&f) > plain, "{style:?}");
        }
        let mut pil = base.clone();
        pil.accessories.pilaster = PilasterStyle::Plain;
        let plain_p = triangles(&pil);
        assert_eq!(plain_p, plain + 24, "two boxes of 12 triangles");
        pil.accessories.pilaster = PilasterStyle::Fluted;
        assert!(triangles(&pil) > plain_p);
        pil.accessories.pilaster_right = false;
        pil.accessories.pilaster = PilasterStyle::Plain;
        assert_eq!(triangles(&pil), plain + 12);
    }

    #[test]
    fn the_new_fields_round_trip_and_old_plans_still_load() {
        let mut c = Cabinet::base(24.0);
        c.fill = PlanFill {
            pattern: FillPattern::CrossHatch,
            ..PlanFill::default()
        };
        c.info.manufacturer = "Acme".into();
        c.accessories.feet = FootStyle::Bun;
        c.in_schedule = false;
        c.door_style.library = "chief.x.1".into();
        let json = serde_json::to_string(&c).unwrap();
        let back: Cabinet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        // Defaults stay out of the file.
        let plain = serde_json::to_string(&Cabinet::base(24.0)).unwrap();
        for key in [
            "\"fill\"",
            "\"info\"",
            "\"accessories\"",
            "\"in_schedule\"",
            "\"library\"",
        ] {
            assert!(!plain.contains(key), "{key} in {plain}");
        }
        // A plan saved before these fields: they take their defaults.
        let old: Cabinet = serde_json::from_str(&plain).unwrap();
        assert!(old.in_schedule);
        assert!(!old.fill.is_visible());
        assert!(old.info.is_empty());
        assert!(old.accessories.is_default());
    }
}
