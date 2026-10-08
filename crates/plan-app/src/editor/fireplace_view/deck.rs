//! Decks and split-level floors in the editor (CB-86, R-86).
//!
//! * [`build_framing`] is Build Framing > Deck: it makes the joists, rim
//!   joists, ledger, beams and posts with footings of every deck room of the
//!   active floor (`plan_framing::deck`) as manual framing members, replaces
//!   the ones a previous build made, and builds the stairs to grade if the
//!   Deck Specification asks. [`clear_framing`] takes them away again.
//! * [`draw`] adds the decking boards to the plan and marks every level
//!   change between rooms of one floor.
//! * [`add_steps`] puts stairs at the level changes (split-level floors).

use super::super::actions::{EditAction, EditActionKind};
use super::super::framing_view::{self, Record};
use super::super::selection::ObjectRef;
use super::super::stairs_view;
use super::super::{Camera, EditorContext};
use super::cmd;
use eframe::egui::{self, Align2, FontId, Pos2, Shape, Stroke};
use plan_core::deck::{deck_rooms, ledger_edges, plank_layout, DeckRoom};
use plan_core::geometry::Point;
use plan_core::split_level::{level_steps, room_level, LevelStep};
use plan_framing::deck::{build_deck_framing, is_deck_member, DeckInput};
use plan_stairs::{solve, Stair, StairParams};

/// Shortest level change that gets stairs, inches.
const MIN_STEP_LENGTH: f64 = 30.0;

/// What a deck build made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildReport {
    pub decks: usize,
    pub joists: usize,
    pub rims: usize,
    pub ledgers: usize,
    pub beams: usize,
    pub posts: usize,
    pub stairs: usize,
    pub warnings: Vec<String>,
}

impl BuildReport {
    pub fn summary(&self) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut s = format!(
            "Built deck framing: {}, {}, {}, {}, {}",
            plural(self.joists, "joist", "joists"),
            plural(self.rims, "rim joist", "rim joists"),
            plural(self.ledgers, "ledger", "ledgers"),
            plural(self.beams, "beam", "beams"),
            plural(self.posts, "post with footing", "posts with footings"),
        );
        if self.stairs > 0 {
            s.push_str(&format!(", {}", plural(self.stairs, "stair", "stairs")));
        }
        if let Some(w) = self.warnings.first() {
            s.push_str(&format!(". {w}"));
        }
        s
    }
}

/// The deck rooms of the active floor (the rooms of `cx.rooms` that carry a
/// Deck Specification).
pub fn decks(cx: &EditorContext) -> Vec<DeckRoom> {
    if cx.floor().room_names.iter().all(|n| n.deck.is_none()) {
        return Vec::new();
    }
    deck_rooms(cx.floor(), &cx.rooms)
}

/// The deck the selection is: a selected room with a Deck Specification.
pub fn selected_deck(cx: &EditorContext) -> Option<DeckRoom> {
    let Some(ObjectRef::Room(i)) = cx.selection.single() else {
        return None;
    };
    let room = cx.rooms.get(i)?;
    decks(cx).into_iter().find(|d| room.contains(d.name.anchor))
}

/// Edit toolbar buttons of the selection: Build and Delete Deck Framing for
/// a deck room, Add Steps for a room with a level change.
pub fn edit_actions(cx: &EditorContext) -> Vec<EditAction> {
    let mut v = Vec::new();
    if selected_deck(cx).is_some() {
        v.push(EditAction::new(EditActionKind::Custom {
            id: cmd::BUILD_DECK,
            label: "Build Deck Framing",
            icon: "",
        }));
        v.push(EditAction::new(EditActionKind::Custom {
            id: cmd::CLEAR_DECK,
            label: "Delete Deck Framing",
            icon: "",
        }));
    }
    if let Some(ObjectRef::Room(i)) = cx.selection.single() {
        if steps_of(cx, Some(i)).iter().any(|s| s.rise() >= 0.5) {
            v.push(EditAction::new(EditActionKind::Custom {
                id: cmd::ADD_STEPS,
                label: "Add Steps at Level Change",
                icon: "",
            }));
        }
    }
    v
}

/// The level changes of the active floor, or only those that touch room `i`.
fn steps_of(cx: &EditorContext, room: Option<usize>) -> Vec<LevelStep> {
    if cx
        .floor()
        .room_names
        .iter()
        .all(|n| n.floor_height_offset.abs() < 0.5 && n.misc.is_none())
    {
        return Vec::new();
    }
    level_steps(cx.floor(), &cx.rooms)
        .into_iter()
        .filter(|s| room.is_none_or(|i| s.low_room == i || s.high_room == i))
        .collect()
}

/// The level changes of the active floor (the plan marks them).
pub fn level_changes(cx: &EditorContext) -> Vec<LevelStep> {
    steps_of(cx, None)
}

// ----- build and clear -----

/// Build Framing > Deck for every deck of the active floor, as one undo
/// step. Returns the report, or `None` when there is no deck.
pub fn build_framing(cx: &mut EditorContext) -> Option<BuildReport> {
    let all = decks(cx);
    if all.is_empty() {
        cx.status = "There is no deck on this floor: give a room the type Deck first".into();
        return None;
    }
    cx.begin_change("Build Deck Framing");
    let fl = cx.floor;
    let mut records = framing_view::load_records(&cx.project.floors[fl]);
    records.retain(|r| !r.member().is_some_and(is_deck_member));
    let mut report = BuildReport::default();
    let floor_elevation = cx.project.floors[fl].elevation;
    let walls = cx.project.floors[fl].walls.clone();
    let mut layers: Vec<String> = Vec::new();
    for d in &all {
        let Some(room) = cx.rooms.iter().find(|r| r.contains(d.name.anchor)).cloned() else {
            continue;
        };
        let top = floor_elevation + room_level(&cx.project.floors[fl], &room);
        let ledger = ledger_edges(&d.outline, &walls);
        let built = {
            let project = &mut cx.project;
            let mut next = || project.alloc_id();
            build_deck_framing(
                &DeckInput {
                    outline: &d.outline,
                    ledger_edges: &ledger,
                    spec: &d.spec,
                    deck_top: top,
                },
                &mut next,
            )
        };
        report.decks += 1;
        report.joists += built.joists();
        report.rims += built.rims();
        report.ledgers += built.ledgers();
        report.beams += built.beams();
        report.posts += built.posts();
        report.warnings.extend(built.warnings.iter().cloned());
        for mut m in built.members {
            m.layer_name = framing_view::layer_for(m.kind).to_string();
            if !layers.contains(&m.layer_name) {
                layers.push(m.layer_name.clone());
            }
            records.push(Record::Manual(m));
        }
        // Stairs to grade replace the ones an earlier build made.
        let mut stair_id = d.spec.stairs.stair_id;
        if let Some(old) = stair_id.take() {
            stairs_view::remove(&mut cx.project, fl, old);
        }
        if d.spec.stairs.to_grade {
            if let Some(obj) = stairs_to_grade(d, top) {
                stair_id = Some(stairs_view::add(&mut cx.project, fl, obj));
                report.stairs += 1;
            }
        }
        if let Some(n) = cx.project.floors[fl]
            .room_names
            .iter_mut()
            .find(|n| n.anchor == d.name.anchor)
        {
            if let Some(spec) = n.deck.as_mut() {
                spec.framing.built = true;
                spec.stairs.stair_id = stair_id;
            }
        }
    }
    framing_view::store_records(&mut cx.project.floors[fl], &records);
    framing_view::ensure_manual_layers(&mut cx.project);
    for l in &layers {
        cx.project.layers.set_display(l, true);
    }
    cx.mark_dirty();
    cx.refresh();
    cx.status = report.summary();
    Some(report)
}

/// The stair from the ground up to the edge of a deck.
fn stairs_to_grade(d: &DeckRoom, deck_top: f64) -> Option<stairs_view::StairObj> {
    let outline = &d.outline;
    let n = outline.len();
    let i = d.spec.stairs.edge.min(n.saturating_sub(1));
    let (p, q) = (outline[i], outline[(i + 1) % n]);
    if p.dist(q) < 12.0 {
        return None;
    }
    let along = (q - p).normalized();
    // The outline is counter-clockwise: the right side is outward.
    let out = Point::new(along.y, -along.x);
    let rise = d.spec.framing.height_above_grade.max(1.0);
    let width = d.spec.stairs.width.min(p.dist(q) - 6.0).max(24.0);
    let params = StairParams {
        total_rise: rise,
        width,
        tread_depth: d.spec.stairs.tread.max(8.0),
        ..StairParams::default()
    };
    let sol = solve(&params);
    let run = sol.total_run;
    // The top of the run meets the deck edge; the stair climbs toward it.
    let top_center = Point::lerp(p, q, 0.5);
    let bottom_center = top_center + out * run;
    let travel = -out;
    let right = Point::new(travel.y, -travel.x);
    let mut stair = Stair::new(
        0,
        bottom_center - right * (width * 0.5),
        travel.angle(),
        params,
    );
    stair.floor_elevation = deck_top - rise;
    let x = stairs_view::StairExtras {
        story_rise: rise,
        ..stairs_view::StairExtras::default()
    };
    Some(stairs_view::StairObj { stair, x })
}

/// Deletes the members of the last deck build (one undo step). Returns how
/// many went.
pub fn clear_framing(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    let mut records = framing_view::load_records(&cx.project.floors[fl]);
    let before = records.len();
    records.retain(|r| !r.member().is_some_and(is_deck_member));
    let n = before - records.len();
    if n == 0 {
        cx.status = "There is no deck framing to delete".into();
        return 0;
    }
    cx.begin_change("Delete Deck Framing");
    framing_view::store_records(&mut cx.project.floors[fl], &records);
    for name in &mut cx.project.floors[fl].room_names {
        if let Some(spec) = name.deck.as_mut() {
            spec.framing.built = false;
        }
    }
    cx.selection.retain_existing(&cx.project, fl);
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Deleted {n} deck framing members");
    n
}

// ----- steps at level changes -----

/// Puts a stair at each level change of the selected room, or of the floor,
/// as one undo step. Returns how many stairs.
pub fn add_steps(cx: &mut EditorContext) -> usize {
    let room = match cx.selection.single() {
        Some(ObjectRef::Room(i)) => Some(i),
        _ => None,
    };
    let steps: Vec<LevelStep> = steps_of(cx, room)
        .into_iter()
        .filter(|s| s.rise() >= 0.5 && s.length() >= MIN_STEP_LENGTH)
        .collect();
    if steps.is_empty() {
        cx.status = "There is no level change long enough for stairs".into();
        return 0;
    }
    cx.begin_change("Add Steps");
    let fl = cx.floor;
    let elevation = cx.project.floors[fl].elevation;
    let mut made = 0;
    for step in &steps {
        let Some(high) = cx.rooms.get(step.high_room) else {
            continue;
        };
        let toward = step.toward_high(high.interior_point());
        let rise = step.rise();
        let width = (step.length() - 6.0).clamp(24.0, 48.0);
        let params = StairParams {
            total_rise: rise,
            width,
            ..StairParams::default()
        };
        let run = solve(&params).total_run;
        let bottom_center = step.mid() - toward * run;
        let right = Point::new(toward.y, -toward.x);
        let mut stair = Stair::new(
            0,
            bottom_center - right * (width * 0.5),
            toward.angle(),
            params,
        );
        stair.floor_elevation = elevation + step.low;
        let x = stairs_view::StairExtras {
            story_rise: rise,
            ..stairs_view::StairExtras::default()
        };
        stairs_view::add(&mut cx.project, fl, stairs_view::StairObj { stair, x });
        made += 1;
    }
    cx.mark_dirty();
    cx.refresh();
    cx.status = format!("Added {made} stair{}", if made == 1 { "" } else { "s" });
    made
}

// ----- drawing -----

fn sc(cam: &Camera, p: Point) -> Pos2 {
    cam.world_to_screen(p)
}

/// Draws the decking boards and the level changes of the active floor.
pub fn draw(cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
    let pal = &cx.palette;
    let board = Stroke::new(0.7_f32, pal.text.gamma_multiply(0.35));
    let border = Stroke::new(0.9_f32, pal.text.gamma_multiply(0.6));
    let shown = cx
        .layers()
        .get(plan_core::deck::DECK_LAYER)
        .is_none_or(|l| l.display);
    if shown {
        for d in decks(cx) {
            if !d.spec.planking.enabled {
                continue;
            }
            for plank in &plank_layout(&d.outline, &d.spec.planking).planks {
                let pts: Vec<Pos2> = plank.corners.iter().map(|p| sc(cam, *p)).collect();
                painter.add(Shape::closed_line(
                    pts,
                    if plank.border { border } else { board },
                ));
            }
        }
    }
    let marker = Stroke::new(1.4_f32, pal.text.gamma_multiply(0.8));
    for step in level_changes(cx) {
        let (a, b) = (sc(cam, step.a), sc(cam, step.b));
        painter.extend(Shape::dashed_line(&[a, b], marker, 8.0, 5.0));
        let Some(high) = cx.rooms.get(step.high_room) else {
            continue;
        };
        let toward = step.toward_high(high.interior_point());
        // Chevrons point up the step, every two feet.
        let len = step.length();
        let dir = (step.b - step.a).normalized();
        let mut s = 12.0;
        while s < len {
            let c = step.a + dir * s;
            let tip = c + toward * 4.0;
            let l = c - dir * 3.0;
            let r = c + dir * 3.0;
            painter.add(Shape::line(
                vec![sc(cam, l), sc(cam, tip), sc(cam, r)],
                marker,
            ));
            s += 24.0;
        }
        let label_at = step.mid() + toward * 14.0;
        painter.text(
            sc(cam, label_at),
            Align2::CENTER_CENTER,
            format!("+{}", crate::dialogs::fmt_short(step.rise())),
            FontId::proportional(11.0),
            pal.text,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::deck::{DeckSpec, DECK_ROOM_TYPE};
    use plan_core::{RoomName, Wall, WallClass, WallKind};

    /// A 16 x 8 ft deck with the house wall on the bottom edge.
    fn deck_cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let pts = [(0.0, 0.0), (192.0, 0.0), (192.0, 96.0), (0.0, 96.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let mut w = if i == 0 {
                Wall::new(
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    6.5,
                    109.0,
                    WallKind::Exterior,
                )
            } else {
                let mut w = Wall::new(
                    Point::new(a.0, a.1),
                    Point::new(b.0, b.1),
                    1.5,
                    36.0,
                    WallKind::Exterior,
                );
                w.class = WallClass::DeckEdge;
                w.is_deck_edge = true;
                w
            };
            w.id = cx.project.alloc_id();
            cx.floor_mut().walls.push(w);
        }
        let mut name = RoomName::new(Point::new(96.0, 48.0), "Deck", DECK_ROOM_TYPE);
        name.has_ceiling = false;
        name.deck = Some(DeckSpec::default());
        cx.floor_mut().room_names.push(name);
        cx.refresh();
        cx
    }

    #[test]
    fn building_the_framing_makes_the_members_as_one_undo_step() {
        let mut cx = deck_cx();
        assert_eq!(cx.rooms.len(), 1);
        let report = build_framing(&mut cx).unwrap();
        assert_eq!(report.decks, 1);
        assert_eq!(report.ledgers, 1);
        assert_eq!(report.rims, 3);
        assert_eq!(report.beams, 1);
        assert_eq!(report.posts, 3);
        assert!(report.joists >= 10);
        let members = framing_view::manual_members(cx.floor());
        assert_eq!(
            members.len(),
            report.joists + report.rims + report.ledgers + report.beams + report.posts
        );
        assert!(members.iter().all(is_deck_member));
        // The layers the members sit on are on.
        assert!(
            cx.project
                .layers
                .get(framing_view::LAYER_JOISTS)
                .unwrap()
                .display
        );
        assert!(
            cx.project
                .layers
                .get(framing_view::LAYER_POSTS)
                .unwrap()
                .display
        );
        // The room knows its framing exists, so 3D skips the skirt.
        assert!(
            cx.floor().room_names[0]
                .deck
                .as_ref()
                .unwrap()
                .framing
                .built
        );
        assert!(cx.status.starts_with("Built deck framing"));
        cx.undo();
        assert!(framing_view::manual_members(cx.floor()).is_empty());
        assert!(
            !cx.floor().room_names[0]
                .deck
                .as_ref()
                .unwrap()
                .framing
                .built
        );
    }

    #[test]
    fn a_rebuild_replaces_the_deck_members_and_keeps_others() {
        let mut cx = deck_cx();
        // A hand-placed post stays.
        let mut post = framing_view::new_member(
            cx.floor(),
            plan_framing::ManualMemberKind::Post,
            Point::new(10.0, 10.0),
            Point::new(10.0, 10.0),
        );
        post.id = cx.project.alloc_id();
        let mut records = framing_view::load_records(cx.floor());
        records.push(Record::Manual(post));
        framing_view::store_records(cx.floor_mut(), &records);
        build_framing(&mut cx).unwrap();
        let first = framing_view::manual_members(cx.floor()).len();
        // Change the spacing and build again.
        cx.floor_mut().room_names[0]
            .deck
            .as_mut()
            .unwrap()
            .framing
            .joist_spacing = 24.0;
        build_framing(&mut cx).unwrap();
        let second = framing_view::manual_members(cx.floor());
        assert!(second.len() < first, "{} vs {first}", second.len());
        assert_eq!(second.iter().filter(|m| !is_deck_member(m)).count(), 1);
        assert_eq!(clear_framing(&mut cx), second.len() - 1);
        assert_eq!(framing_view::manual_members(cx.floor()).len(), 1);
        assert_eq!(clear_framing(&mut cx), 0);
    }

    #[test]
    fn building_without_a_deck_says_so() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert!(build_framing(&mut cx).is_none());
        assert!(cx.status.contains("no deck"));
    }

    #[test]
    fn stairs_to_grade_leave_the_chosen_edge_and_are_replaced_on_rebuild() {
        let mut cx = deck_cx();
        // The edge along the right side of the deck (x near 192).
        let outline = decks(&cx)[0].outline.clone();
        let right = (0..outline.len())
            .find(|&i| {
                let (p, q) = (outline[i], outline[(i + 1) % outline.len()]);
                p.x > 150.0 && q.x > 150.0
            })
            .unwrap();
        {
            let spec = cx.floor_mut().room_names[0].deck.as_mut().unwrap();
            spec.stairs.to_grade = true;
            spec.stairs.edge = right;
        }
        let report = build_framing(&mut cx).unwrap();
        assert_eq!(report.stairs, 1);
        let stairs = stairs_view::load(cx.floor());
        assert_eq!(stairs.len(), 1);
        let s = &stairs[0].stair;
        // 36" of rise at the deck edge; the stair climbs toward the deck (-x).
        assert!((s.params.total_rise - 36.0).abs() < 1e-9);
        assert!((s.direction.cos() + 1.0).abs() < 1e-6, "{}", s.direction);
        // Its bottom is at grade: 36" below the deck top.
        let top = cx.floor().elevation + room_level(cx.floor(), &cx.rooms[0]);
        assert!((s.bottom_elevation() - (top - 36.0)).abs() < 1e-6);
        let first_id = stairs[0].id();
        build_framing(&mut cx).unwrap();
        let again = stairs_view::load(cx.floor());
        assert_eq!(again.len(), 1);
        assert_ne!(again[0].id(), first_id);
    }

    /// Two rooms side by side, the right one 24" up, divided by a Room Divider.
    fn split_cx() -> EditorContext {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let corners = [(0.0, 0.0), (240.0, 0.0), (240.0, 120.0), (0.0, 120.0)];
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let mut w = Wall::new(
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                4.5,
                109.0,
                WallKind::Exterior,
            );
            w.id = cx.project.alloc_id();
            cx.floor_mut().walls.push(w);
        }
        let mut mid = Wall::new(
            Point::new(120.0, 0.0),
            Point::new(120.0, 120.0),
            4.5,
            109.0,
            WallKind::Interior,
        );
        mid.id = cx.project.alloc_id();
        mid.class = WallClass::RoomDivider;
        mid.flags.room_divider = true;
        cx.floor_mut().walls.push(mid);
        cx.floor_mut()
            .room_names
            .push(RoomName::new(Point::new(60.0, 60.0), "Low", "Living"));
        let mut high = RoomName::new(Point::new(180.0, 60.0), "High", "Living");
        high.floor_height_offset = 24.0;
        cx.floor_mut().room_names.push(high);
        cx.refresh();
        cx
    }

    #[test]
    fn a_level_change_is_found_and_gets_stairs_climbing_into_the_high_room() {
        let mut cx = split_cx();
        let changes = level_changes(&cx);
        assert_eq!(changes.len(), 1);
        assert!((changes[0].rise() - 24.0).abs() < 1e-9);
        let low = changes[0].low;
        assert_eq!(add_steps(&mut cx), 1);
        let stairs = stairs_view::load(cx.floor());
        assert_eq!(stairs.len(), 1);
        let s = &stairs[0].stair;
        // 24" of rise in 4 risers, climbing toward +x.
        assert!((s.params.total_rise - 24.0).abs() < 1e-9);
        assert!(s.direction.cos() > 0.99);
        assert_eq!(solve(&s.params).risers, 4);
        // The stair stands in the low room: its origin is left of x = 120.
        assert!(s.origin.x < 120.0, "{:?}", s.origin);
        // The stair starts at the low room's floor.
        assert!((s.bottom_elevation() - (cx.floor().elevation + low)).abs() < 1e-9);
        cx.undo();
        assert!(stairs_view::load(cx.floor()).is_empty());
    }

    #[test]
    fn no_level_change_means_no_stairs() {
        let mut cx = split_cx();
        cx.floor_mut().room_names[1].floor_height_offset = 0.0;
        cx.refresh();
        assert!(level_changes(&cx).is_empty());
        assert_eq!(add_steps(&mut cx), 0);
        assert!(cx.status.contains("no level change"));
    }

    #[test]
    fn the_edit_toolbar_offers_deck_and_step_commands_for_the_selected_room() {
        let mut cx = deck_cx();
        assert!(edit_actions(&cx).is_empty());
        cx.selection.set(ObjectRef::Room(0));
        let ids: Vec<&str> = edit_actions(&cx)
            .iter()
            .filter_map(|a| match a.kind {
                EditActionKind::Custom { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, [cmd::BUILD_DECK, cmd::CLEAR_DECK]);
        assert!(selected_deck(&cx).is_some());
        let mut cx = split_cx();
        cx.selection.set(ObjectRef::Room(0));
        let ids: Vec<&str> = edit_actions(&cx)
            .iter()
            .filter_map(|a| match a.kind {
                EditActionKind::Custom { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, [cmd::ADD_STEPS]);
    }

    #[test]
    fn the_report_reads_in_words() {
        let r = BuildReport {
            joists: 1,
            rims: 3,
            posts: 3,
            beams: 1,
            ledgers: 1,
            stairs: 1,
            warnings: vec!["Check the posts.".into()],
            ..BuildReport::default()
        };
        let s = r.summary();
        assert!(s.contains("1 joist,"));
        assert!(s.contains("3 posts with footings"));
        assert!(s.contains("1 stair"));
        assert!(s.ends_with("Check the posts."));
    }
}
