//! Detail Components: a small built-in catalogue of the pieces a wall detail
//! is drawn from (framing sections, insulation, flashing, sheathing, siding
//! profiles, anchor bolts), placed as CAD blocks.
//!
//! A component is a few [`Part`]s in detail inches (lower-left corner of the
//! component at the origin). [`place`] puts the parts on the active floor's
//! detail layers, adds the hatch lines of a part that has a pattern, makes
//! the lot one CAD block named after the component, and centres it on the
//! point given; the whole placement is one undo step. The Detail Components
//! window (`dialogs::details`) lists the catalogue and arms the placement
//! tool.

use crate::editor::EditorContext;
use plan_core::cad::{insulation_items, CadItem};
use plan_core::details::{
    DETAIL_COMPONENTS_LAYER, DETAIL_FRAMING_LAYER, DETAIL_HATCH_LAYER, DETAIL_INSULATION_LAYER,
};
use plan_core::geometry::Point;
use plan_core::Id;
use std::cell::Cell;

/// The kind of a component (the chooser's groups).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Framing,
    Insulation,
    Flashing,
    Sheathing,
    Siding,
    Anchors,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Framing,
        Category::Insulation,
        Category::Flashing,
        Category::Sheathing,
        Category::Siding,
        Category::Anchors,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Framing => "Framing",
            Category::Insulation => "Insulation",
            Category::Flashing => "Flashing",
            Category::Sheathing => "Sheathing",
            Category::Siding => "Siding",
            Category::Anchors => "Anchors",
        }
    }
}

/// A hatch pattern a part is filled with (a `details_view::PATTERN_NAMES`
/// name).
#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub pattern: &'static str,
    pub scale: f64,
    pub angle: f64,
}

/// One drawn piece of a component.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub layer: &'static str,
    pub item: CadItem,
    pub fill: Option<Fill>,
    /// Plotted weight (1/100 mm) when the part is heavier than its layer.
    pub weight: Option<u32>,
}

/// An entry of the catalogue.
#[derive(Clone, Debug)]
pub struct Component {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub parts: Vec<Part>,
}

impl Component {
    /// The bounds of the parts.
    pub fn bounds(&self) -> (Point, Point) {
        let mut lo = Point::new(f64::MAX, f64::MAX);
        let mut hi = Point::new(f64::MIN, f64::MIN);
        for p in &self.parts {
            let (a, b) = p.item.bounds();
            lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
            hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
        }
        if self.parts.is_empty() {
            return (Point::ZERO, Point::ZERO);
        }
        (lo, hi)
    }

    /// Width and height of the component, inches.
    pub fn size(&self) -> (f64, f64) {
        let (lo, hi) = self.bounds();
        (hi.x - lo.x, hi.y - lo.y)
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> CadItem {
    CadItem::Polyline {
        points: vec![
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x, y + h),
        ],
        closed: true,
    }
}

fn line(a: (f64, f64), b: (f64, f64)) -> CadItem {
    CadItem::Line {
        a: Point::new(a.0, a.1),
        b: Point::new(b.0, b.1),
    }
}

fn part(layer: &'static str, item: CadItem) -> Part {
    Part {
        layer,
        item,
        fill: None,
        weight: None,
    }
}

/// A lumber section: the rectangle and the X that marks cut wood.
fn lumber(w: f64, h: f64) -> Vec<Part> {
    vec![
        part(DETAIL_FRAMING_LAYER, rect(0.0, 0.0, w, h)),
        part(DETAIL_FRAMING_LAYER, line((0.0, 0.0), (w, h))),
        part(DETAIL_FRAMING_LAYER, line((0.0, h), (w, 0.0))),
    ]
}

/// A batt standing in a cavity `w` wide and `h` high.
fn batt(w: f64, h: f64) -> Vec<Part> {
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(0.0, h),
        Point::new(w, h),
        Point::new(w, 0.0),
    ];
    insulation_items(&corners)
        .into_iter()
        .map(|i| part(DETAIL_INSULATION_LAYER, i))
        .collect()
}

fn hatched(layer: &'static str, item: CadItem, pattern: &'static str, scale: f64) -> Part {
    Part {
        layer,
        item,
        fill: Some(Fill {
            pattern,
            scale,
            angle: 45.0,
        }),
        weight: None,
    }
}

/// Sheet metal: the profile as a heavy line with a thin second line 1/16"
/// away, so it reads as a thickness.
fn sheet(layer: &'static str, points: &[(f64, f64)]) -> Vec<Part> {
    let pts: Vec<Point> = points.iter().map(|p| Point::new(p.0, p.1)).collect();
    let inner = plan_core::cad::offset_polyline(&pts, false, 0.0625);
    vec![
        Part {
            layer,
            item: CadItem::Polyline {
                points: pts,
                closed: false,
            },
            fill: None,
            weight: Some(50),
        },
        part(
            layer,
            CadItem::Polyline {
                points: inner,
                closed: false,
            },
        ),
    ]
}

/// The whole catalogue.
pub fn catalogue() -> Vec<Component> {
    let c = |id, name, category, parts| Component {
        id,
        name,
        category,
        parts,
    };
    let comp = DETAIL_COMPONENTS_LAYER;
    let mut out = vec![
        c(
            "frame.2x4",
            "2x4 Section",
            Category::Framing,
            lumber(1.5, 3.5),
        ),
        c(
            "frame.2x6",
            "2x6 Section",
            Category::Framing,
            lumber(1.5, 5.5),
        ),
        c(
            "frame.2x8",
            "2x8 Section",
            Category::Framing,
            lumber(1.5, 7.25),
        ),
        c(
            "frame.2x10",
            "2x10 Section",
            Category::Framing,
            lumber(1.5, 9.25),
        ),
        c(
            "frame.2x12",
            "2x12 Section",
            Category::Framing,
            lumber(1.5, 11.25),
        ),
        c(
            "frame.plates",
            "2x6 Wall Plates (bottom and double top)",
            Category::Framing,
            [
                lumber(5.5, 1.5),
                lumber(5.5, 1.5)
                    .into_iter()
                    .map(|mut p| {
                        p.item = plan_core::cad::translated(&p.item, Point::new(0.0, 93.0));
                        p
                    })
                    .collect(),
                lumber(5.5, 1.5)
                    .into_iter()
                    .map(|mut p| {
                        p.item = plan_core::cad::translated(&p.item, Point::new(0.0, 94.5));
                        p
                    })
                    .collect(),
            ]
            .concat(),
        ),
        c(
            "insul.batt35",
            "Batt Insulation, 3 1/2\" Cavity",
            Category::Insulation,
            batt(3.5, 24.0),
        ),
        c(
            "insul.batt55",
            "Batt Insulation, 5 1/2\" Cavity",
            Category::Insulation,
            batt(5.5, 24.0),
        ),
        c(
            "insul.batt75",
            "Batt Insulation, 7 1/4\" Cavity",
            Category::Insulation,
            batt(7.25, 24.0),
        ),
        c(
            "insul.rigid2",
            "Rigid Insulation, 2\"",
            Category::Insulation,
            vec![hatched(
                DETAIL_INSULATION_LAYER,
                rect(0.0, 0.0, 2.0, 24.0),
                "Lines",
                0.5,
            )],
        ),
        c(
            "flash.z",
            "Z Flashing",
            Category::Flashing,
            sheet(comp, &[(0.0, 2.0), (1.5, 2.0), (1.5, 0.0), (3.0, 0.0)]),
        ),
        c(
            "flash.drip",
            "Drip Edge",
            Category::Flashing,
            sheet(comp, &[(0.0, 2.5), (0.0, 0.0), (2.0, 0.0), (2.0, 0.5)]),
        ),
        c(
            "flash.sill",
            "Sloped Sill Flashing",
            Category::Flashing,
            sheet(comp, &[(0.0, 2.0), (1.0, 2.0), (4.0, 0.5), (4.0, 0.0)]),
        ),
        c(
            "sheath.osb",
            "OSB Sheathing, 7/16\"",
            Category::Sheathing,
            vec![hatched(comp, rect(0.0, 0.0, 0.4375, 24.0), "Lines", 0.25)],
        ),
        c(
            "sheath.ply",
            "Plywood Sheathing, 5/8\"",
            Category::Sheathing,
            vec![
                part(comp, rect(0.0, 0.0, 0.625, 24.0)),
                part(comp, line((0.0, 0.0), (0.625, 24.0))),
            ],
        ),
        c(
            "siding.lap",
            "Lap Siding, 6\" Exposure",
            Category::Siding,
            siding_lap(),
        ),
        c(
            "siding.batten",
            "Board and Batten",
            Category::Siding,
            siding_batten(),
        ),
        c(
            "siding.shiplap",
            "Shiplap, 6\" Boards",
            Category::Siding,
            siding_shiplap(),
        ),
        c(
            "anchor.half",
            "Anchor Bolt, 1/2\" x 10\"",
            Category::Anchors,
            anchor(0.5, 10.0),
        ),
        c(
            "anchor.fiveeighths",
            "Anchor Bolt, 5/8\" x 12\"",
            Category::Anchors,
            anchor(0.625, 12.0),
        ),
    ];
    // Normalise: every component starts at the origin.
    for comp in &mut out {
        let (lo, _) = comp.bounds();
        let d = Point::new(-lo.x, -lo.y);
        for p in &mut comp.parts {
            p.item = plan_core::cad::translated(&p.item, d);
        }
    }
    out
}

/// Four courses of bevel siding seen from the side, 6" exposure.
fn siding_lap() -> Vec<Part> {
    let mut parts = Vec::new();
    for i in 0..4 {
        let y = f64::from(i) * 6.0;
        // A tapered board: 3/4" at the butt, 3/16" at the head.
        parts.push(part(
            DETAIL_COMPONENTS_LAYER,
            CadItem::Polyline {
                points: vec![
                    Point::new(0.0, y),
                    Point::new(0.75, y),
                    Point::new(0.1875, y + 7.0),
                    Point::new(0.0, y + 7.0),
                ],
                closed: true,
            },
        ));
    }
    parts
}

/// Three bays of board and batten seen from above.
fn siding_batten() -> Vec<Part> {
    let mut parts = vec![part(DETAIL_COMPONENTS_LAYER, rect(0.0, 0.0, 36.0, 0.75))];
    for i in 0..=3 {
        parts.push(part(
            DETAIL_COMPONENTS_LAYER,
            rect(f64::from(i) * 12.0 - 0.75, 0.75, 1.5, 0.5),
        ));
    }
    parts
}

/// A run of shiplap boards seen from above: each board steps over the next.
fn siding_shiplap() -> Vec<Part> {
    let mut parts = Vec::new();
    for i in 0..4 {
        let x = f64::from(i) * 5.5;
        parts.push(part(
            DETAIL_COMPONENTS_LAYER,
            CadItem::Polyline {
                points: vec![
                    Point::new(x, 0.0),
                    Point::new(x + 6.0, 0.0),
                    Point::new(x + 6.0, 0.375),
                    Point::new(x + 5.5, 0.375),
                    Point::new(x + 5.5, 0.75),
                    Point::new(x, 0.75),
                ],
                closed: true,
            },
        ));
    }
    parts
}

/// An anchor bolt standing in a sill plate: the shank, the washer and the nut.
fn anchor(dia: f64, len: f64) -> Vec<Part> {
    let c = DETAIL_COMPONENTS_LAYER;
    let washer = dia * 3.0;
    vec![
        part(c, rect(-dia / 2.0, 0.0, dia, len)),
        part(c, rect(-washer / 2.0, len, washer, 0.125)),
        part(c, rect(-dia * 0.9, len + 0.125, dia * 1.8, dia * 0.9)),
    ]
}

/// The catalogue entry with this id.
pub fn find(id: &str) -> Option<Component> {
    catalogue().into_iter().find(|c| c.id == id)
}

thread_local! {
    static ARMED: Cell<Option<&'static str>> = const { Cell::new(None) };
}

/// The component the placement tool places, if one is chosen.
pub fn armed() -> Option<Component> {
    ARMED.with(Cell::get).and_then(find)
}

/// Chooses the component the placement tool places (`None` clears it).
pub fn arm(id: Option<&str>) {
    let id = id.and_then(|i| catalogue().into_iter().find(|c| c.id == i).map(|c| c.id));
    ARMED.with(|a| a.set(id));
}

/// Places `component` with the middle of its bounds on `at` as one CAD block
/// on the active floor (one undo step, "Place Detail Component"). Returns the
/// block's group id, or `None` when a detail layer is locked.
pub fn place(cx: &mut EditorContext, component: &Component, at: Point) -> Option<Id> {
    for p in &component.parts {
        if cx.layers().is_locked(p.layer) {
            cx.status = format!("The layer \"{}\" is locked", p.layer);
            return None;
        }
    }
    let (lo, hi) = component.bounds();
    let d = at.sub(Point::lerp(lo, hi, 0.5));
    let fl = cx.floor;
    cx.begin_change("Place Detail Component");
    plan_core::details::ensure_detail_layers(&mut cx.project.layers);
    let mut ids = Vec::new();
    for p in &component.parts {
        let item = plan_core::cad::translated(&p.item, d);
        let id = cx.project.add_cad(fl, p.layer, item.clone());
        if let Some(w) = p.weight {
            cx.project.edit_cad_attrs(fl, id, |a| a.weight = Some(w));
        }
        ids.push(id);
        // The hatch of a filled part: lines clipped to its outline.
        if let (
            Some(fill),
            CadItem::Polyline {
                points,
                closed: true,
            },
        ) = (&p.fill, &item)
        {
            let pattern =
                crate::editor::details_view::pattern_named(fill.pattern, fill.scale, fill.angle);
            let strokes = crate::editor::details_view::strokes_in(&pattern, points, 1.5);
            for (a, b) in strokes {
                ids.push(
                    cx.project
                        .add_cad(fl, DETAIL_HATCH_LAYER, CadItem::Line { a, b }),
                );
            }
        }
    }
    let Some(group) = cx.project.make_cad_block(fl, &ids, Some(component.name)) else {
        cx.cancel_change();
        return None;
    };
    cx.selection.items = ids
        .iter()
        .map(|i| crate::editor::ObjectRef::Cad(*i))
        .collect();
    cx.mark_dirty();
    cx.status = format!("Placed {}", component.name);
    Some(group)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn the_catalogue_covers_every_category_with_unique_ids() {
        let all = catalogue();
        for cat in Category::ALL {
            assert!(
                all.iter().any(|c| c.category == cat),
                "{} has no component",
                cat.name()
            );
        }
        let mut ids: Vec<_> = all.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "ids repeat");
        for c in &all {
            assert!(!c.parts.is_empty(), "{}", c.name);
            let (lo, _) = c.bounds();
            assert!(
                lo.x.abs() < 1e-9 && lo.y.abs() < 1e-9,
                "{} starts at 0,0",
                c.name
            );
            let (w, h) = c.size();
            assert!(w > 0.0 && h > 0.0, "{}", c.name);
        }
    }

    #[test]
    fn framing_sections_are_real_lumber_sizes() {
        let two_by_six = find("frame.2x6").unwrap();
        let (w, h) = two_by_six.size();
        assert_eq!((w, h), (1.5, 5.5));
        // The rectangle and the X.
        assert_eq!(two_by_six.parts.len(), 3);
        assert_eq!(find("frame.2x12").unwrap().size(), (1.5, 11.25));
        let batt = find("insul.batt55").unwrap();
        assert_eq!(batt.size().0, 5.5);
    }

    #[test]
    fn placing_makes_one_block_on_the_detail_layers_in_one_undo_step() {
        let mut cx = new_cx();
        let comp = find("insul.batt35").unwrap();
        let group = place(&mut cx, &comp, Point::new(100.0, 100.0)).unwrap();
        let f = cx.floor();
        assert!(f.cad.iter().all(|o| o.layer == DETAIL_INSULATION_LAYER));
        assert!(f.cad_block(group).is_some());
        assert_eq!(f.cad_block(group).unwrap().name, comp.name);
        // Centred on the click.
        let (lo, hi) = f.block_bounds(group);
        let mid = Point::lerp(lo, hi, 0.5);
        assert!(mid.dist(Point::new(100.0, 100.0)) < 1e-9, "{mid:?}");
        assert!(cx.project.layers.get(DETAIL_INSULATION_LAYER).is_some());
        assert_eq!(cx.undo().as_deref(), Some("Place Detail Component"));
        assert!(cx.floor().cad.is_empty());
        assert!(cx.floor().cad_blocks.is_empty());
    }

    #[test]
    fn a_filled_part_gets_hatch_lines_and_flashing_is_heavy() {
        let mut cx = new_cx();
        place(&mut cx, &find("insul.rigid2").unwrap(), Point::ZERO).unwrap();
        let hatch = cx
            .floor()
            .cad
            .iter()
            .filter(|o| o.layer == DETAIL_HATCH_LAYER)
            .count();
        assert!(hatch > 4, "{hatch} hatch lines");
        let mut flash = new_cx();
        place(&mut flash, &find("flash.z").unwrap(), Point::ZERO).unwrap();
        assert!(flash.floor().cad_attrs.iter().all(|a| a.weight == Some(50)));
        assert_eq!(flash.floor().cad_attrs.len(), 1);
        assert_eq!(flash.floor().cad.len(), 2, "the line and its thickness");
    }

    #[test]
    fn a_locked_layer_refuses_the_placement() {
        let mut cx = new_cx();
        plan_core::details::ensure_detail_layers(&mut cx.project.layers);
        cx.project
            .layers
            .get_mut(DETAIL_FRAMING_LAYER)
            .unwrap()
            .locked = true;
        assert!(place(&mut cx, &find("frame.2x4").unwrap(), Point::ZERO).is_none());
        assert!(cx.floor().cad.is_empty());
        assert!(cx.status.contains("locked"));
    }

    #[test]
    fn arming_remembers_a_known_component_only() {
        arm(Some("frame.2x4"));
        assert_eq!(armed().unwrap().name, "2x4 Section");
        arm(Some("nope"));
        assert!(armed().is_none());
        arm(None);
        assert!(armed().is_none());
    }
}
