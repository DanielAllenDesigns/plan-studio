//! Scenario 35: the tool and dialog sweep (round 14).
//!
//! Daniel's standing requirement: every tool works like Chief's, and every
//! object a tool makes has a specification dialog with Chief's tabs. For every
//! tool id the toolbars, flyouts and menus can pick, this sweep
//!
//! 1. activates the tool and performs its canonical headless gesture (a click,
//!    a click-drag, or two or three clicks and Enter for polylines and
//!    multi-point tools), then asserts that an object appeared, that exactly
//!    one undo step was pushed, and that undo and redo take it away and bring
//!    it back;
//! 2. selects the object and runs **Open Object** from the Edit toolbar,
//!    reads the dialog's title and tab list from the frame it paints, and
//!    compares them with Chief's names (tables below; docs/chief-x18-dialogs.md
//!    and docs/parity/*.md);
//! 3. edits one field through the dialog (typed into the first field, or the
//!    draft where the dialog offers test access) and asserts the plan changed
//!    in one undo step and that undo restores it;
//! 4. runs **Delete Objects** from the Edit toolbar and asserts the object is
//!    gone (one undo step; undo brings it back).
//!
//! A second test runs every Edit toolbar action of every object kind, and a
//! third checks that every toolbar and menu command is live. Each tool runs on
//! its own thread so the thread-local dialog windows of one case never leak
//! into the next.
//!
//! Findings (tools that create nothing, kinds with no dialog, tabs Chief has
//! that Plan Studio lacks) are printed as a table; `S35_WRITE_DOC=1` writes it
//! to `docs/tool-dialog-sweep.md`. Rows that are known gaps are listed in
//! [`KNOWN_GAPS`] with the QA entry in `docs/qa-findings.md`; a new failure
//! outside that list fails the test.

use super::{draw_shell, Sim};
use crate::editor::actions::EditActionKind;
use crate::editor::{roof_view, selection, site_view, EditorRequest, ObjectRef};
use crate::shell::hotkeys::collect_commands;
use crate::toolbar::Action;
use crate::tools::cad::CadMode;
use crate::tools::details::DetailsVariant as Dt;
use crate::tools::dimension::DimMode;
use crate::tools::electrical::ElecVariant;
use crate::tools::foundation::FoundationVariant;
use crate::tools::images::ImageMode;
use crate::tools::roof::RoofMode;
use crate::tools::terrain::TerrainVariant as Tv;
use crate::tools::text::TextMode;
use crate::tools::{registry, KeyEvent, ToolId};
use eframe::egui::{self, Key};
use plan_cabinets::CabinetKind;
use plan_core::geometry::Point;
use plan_core::WallKind;
use std::collections::BTreeMap;

const W: f64 = 480.0;
const H: f64 = 360.0;

// ---------------------------------------------------------------------------
// Every tool id
// ---------------------------------------------------------------------------

/// Every tool id the registry, the toolbars, the flyouts and the menus can pick.
fn all_tool_ids() -> Vec<ToolId> {
    use crate::tools::camera::CameraVariant as Cv;
    let mut ids: Vec<ToolId> = Vec::new();
    let mut add = |id: ToolId| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    for t in registry() {
        add(t.id());
    }
    for c in collect_commands() {
        if let Action::SetTool(id) = c.action {
            add(id);
        }
    }
    for m in DimMode::ALL {
        add(ToolId::DimensionVariant(m));
    }
    for m in CadMode::ALL {
        add(ToolId::CadVariant(m));
    }
    for m in TextMode::ALL {
        add(ToolId::TextVariant(m));
    }
    for k in crate::editor::stairs_view::StairKind::ALL {
        add(ToolId::StairsVariant(k));
    }
    for v in FoundationVariant::ALL {
        add(ToolId::FoundationVariant(v));
    }
    for k in crate::tools::cabinet::KINDS {
        add(ToolId::CabinetVariant(k));
    }
    for v in ElecVariant::ALL {
        add(ToolId::ElectricalVariant(v));
    }
    for v in Tv::ALL {
        add(ToolId::TerrainVariant(v));
    }
    for v in Dt::ALL {
        add(ToolId::DetailsVariant(v));
    }
    for v in crate::tools::framing::FramingVariant::ALL {
        add(ToolId::FramingVariant(v));
    }
    for v in ImageMode::ALL {
        add(ToolId::ImagesVariant(v));
    }
    for k in crate::tools::schedule::FLYOUT_KINDS {
        add(ToolId::ScheduleVariant(k));
    }
    for m in crate::tools::fireplace::FireplaceMode::ALL {
        add(ToolId::FireplaceVariant(m));
    }
    for m in crate::tools::painters::PainterMode::ALL {
        add(ToolId::PainterVariant(m));
    }
    for v in [
        Cv::FullCamera,
        Cv::FloorCamera,
        Cv::GlassHouse,
        Cv::FullOverview,
        Cv::FloorOverview,
        Cv::DollHouse,
        Cv::CrossSection,
        Cv::BackClippedSection,
        Cv::WallElevation,
        Cv::AutoElevation,
        Cv::AutoBackclipped,
        Cv::AutoInterior,
        Cv::Walkthrough,
        Cv::AddLights,
    ] {
        add(ToolId::CameraVariant(v));
    }
    ids
}

// ---------------------------------------------------------------------------
// Fixtures and gestures
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    None,
    Enter,
    /// Enter to finish the shape, Enter again to accept the typed default.
    EnterEnter,
}

#[derive(Clone, Copy, Debug)]
enum G {
    Click(f64, f64),
    Drag((f64, f64), (f64, f64)),
    Clicks(&'static [(f64, f64)], End),
    /// Click, type "Den", Enter.
    Text(f64, f64),
    /// Press-drag the baseline, then click the side it rises to.
    DragClick((f64, f64), (f64, f64), (f64, f64)),
    /// Click, then OK in the dialog the click opened.
    ClickOk(f64, f64),
    /// Click, type "Den", Tab (Rich Text finishes with Tab).
    RichText(f64, f64),
    /// Click a point offset from the centroid of the south roof plane.
    PlaneClick(f64, f64),
    /// Drag between two points offset from that centroid.
    PlaneDrag((f64, f64), (f64, f64)),
    /// Click on the south roof plane, then OK in the dialog.
    PlaneClickOk,
    /// Click inside the first dormer.
    OnDormer,
    /// Click the start of the south plane's eave.
    EaveCorner,
    /// Click the curved wall's apex, then a point 30 inches off it.
    OnArc,
}

const TWO: &[(f64, f64)] = &[(100.0, 100.0), (300.0, 100.0)];
const THREE: &[(f64, f64)] = &[(100.0, 100.0), (300.0, 100.0), (300.0, 250.0)];
const DRAG: G = G::Drag((100.0, 100.0), (300.0, 200.0));
const ON_WALL: G = G::Click(200.0, 2.0);
const IN_ROOM: G = G::Click(240.0, 180.0);
const PLANE: G = G::DragClick((100.0, 100.0), (300.0, 100.0), (200.0, 200.0));

/// The plan a tool is tried on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fx {
    /// Four exterior walls.
    Shell,
    /// The shell with a door, a window and a partition.
    Furnished,
    /// Furnished, with a built roof.
    Roofed,
    /// Roofed, with a dormer.
    Dormered,
    /// The shell with a Library item armed.
    Library,
    /// The shell with a custom countertop.
    Countertop,
    /// The shell with two collinear partitions (End to End).
    Split,
    /// The shell with a run of base cabinets along the north wall.
    Cabinets,
    /// The shell with a curved partition (Radius and Arc Length dimensions).
    Curved,
    /// The shell with a terrain perimeter.
    Perimeter,
    /// The perimeter and a terrain elevation reference point.
    RefPoint,
    /// The shell with a road (two clicks and Enter, Enter).
    Road,
}

fn fixture(f: Fx) -> Sim {
    let mut sim = Sim::new();
    draw_shell(&mut sim, W, H);
    if matches!(f, Fx::Furnished | Fx::Roofed | Fx::Dormered) {
        sim.tool(ToolId::Door);
        sim.click(120.0, 0.0);
        sim.tool(ToolId::Window);
        sim.click(300.0, 0.0);
        sim.tool(ToolId::Wall {
            kind: WallKind::Interior,
        });
        sim.drag((240.0, 0.0), (240.0, H + 1.0));
    }
    if matches!(f, Fx::Roofed | Fx::Dormered) {
        sim.tool(ToolId::RoofVariant(RoofMode::Build));
        sim.click(240.0, 180.0);
        sim.ok();
    }
    if f == Fx::Dormered {
        sim.tool(ToolId::RoofVariant(RoofMode::Dormer));
        perform(&mut sim, G::PlaneClickOk);
    }
    if f == Fx::Countertop {
        sim.tool(ToolId::CabinetVariant(CabinetKind::CustomCountertop));
        sim.drag((100.0, 100.0), (300.0, 200.0));
    }
    if f == Fx::Split {
        sim.tool(ToolId::Wall {
            kind: WallKind::Interior,
        });
        sim.drag((240.0, 0.0), (240.0, 150.0));
        sim.drag((240.0, 150.0), (240.0, H + 1.0));
    }
    if f == Fx::Cabinets {
        sim.tool(ToolId::CabinetVariant(CabinetKind::Base));
        for x in [60.0, 120.0, 180.0] {
            sim.click(x, 20.0);
        }
    }
    if matches!(f, Fx::Perimeter | Fx::RefPoint) {
        sim.tool(ToolId::TerrainVariant(Tv::Perimeter));
        perform(&mut sim, G::Clicks(THREE, End::Enter));
    }
    if f == Fx::RefPoint {
        sim.tool(ToolId::TerrainVariant(Tv::ReferencePoint));
        sim.click(240.0, 180.0);
    }
    if f == Fx::Road {
        sim.tool(ToolId::TerrainVariant(Tv::Road));
        perform(&mut sim, G::Clicks(TWO, End::EnterEnter));
    }
    if f == Fx::Curved {
        let cx = &mut sim.app.cx;
        let id = cx.project.add_wall(
            0,
            Point::new(100.0, 150.0),
            Point::new(340.0, 150.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        cx.project.floors[0]
            .wall_mut(id)
            .expect("the curved wall")
            .curve = Some(plan_core::walls::WallCurve { bulge: 60.0 });
    }
    if f == Fx::Library {
        let id = crate::tools::library::library_catalog()
            .all_items()
            .next()
            .map(|i| i.id.clone())
            .expect("the core library has items");
        crate::tools::library::set_active_item(&mut sim.app.cx, &id);
    }
    sim.tool(ToolId::Select);
    sim.app.cx.selection.clear();
    sim.app.cx.refresh();
    sim
}

fn perform(sim: &mut Sim, g: G) {
    match g {
        G::Click(x, y) => {
            sim.click(x, y);
        }
        G::Drag(a, b) => {
            sim.drag(a, b);
        }
        G::Clicks(pts, end) => {
            for (x, y) in pts {
                sim.click(*x, *y);
            }
            match end {
                End::None => {}
                End::Enter => {
                    sim.key(KeyEvent::key(Key::Enter));
                }
                End::EnterEnter => {
                    sim.key(KeyEvent::key(Key::Enter));
                    sim.key(KeyEvent::key(Key::Enter));
                }
            }
        }
        G::Text(x, y) => {
            sim.click(x, y);
            sim.key(KeyEvent::text("Den"));
            sim.key(KeyEvent::key(Key::Enter));
        }
        G::DragClick(a, b, c) => {
            sim.drag(a, b);
            sim.click(c.0, c.1);
        }
        G::ClickOk(x, y) => {
            sim.click(x, y);
            sim.ok();
        }
        G::RichText(x, y) => {
            sim.click(x, y);
            sim.key(KeyEvent::text("Den"));
            sim.key(KeyEvent::key(Key::Tab));
        }
        G::PlaneClick(dx, dy) => {
            let c = south_plane(sim).centroid();
            sim.click(c.x + dx, c.y + dy);
        }
        G::PlaneDrag(a, b) => {
            let c = south_plane(sim).centroid();
            sim.drag((c.x + a.0, c.y + a.1), (c.x + b.0, c.y + b.1));
        }
        G::PlaneClickOk => {
            let c = south_plane(sim).centroid();
            sim.click(c.x, c.y);
            sim.ok();
            sim.dialog_frame(false);
        }
        G::OnDormer => {
            let at = dormer_point(sim);
            sim.click(at.x, at.y);
        }
        G::EaveCorner => {
            let a = south_plane(sim).baseline.0;
            sim.click(a.x, a.y);
        }
        G::OnArc => {
            let w = sim
                .app
                .cx
                .floor()
                .walls
                .iter()
                .find(|w| w.curve.is_some())
                .expect("the fixture has a curved wall")
                .clone();
            let mid = w.path_length() * 0.5;
            let apex = w.point_along(mid);
            let out = apex.add(w.normal_along(mid).scale(30.0));
            sim.click(apex.x, apex.y);
            sim.click(out.x, out.y);
        }
    }
}

/// The roof plane that faces south: its baseline is the lowest horizontal one.
fn south_plane(sim: &Sim) -> roof_view::RoofPlaneRecord {
    roof_view::load(sim.app.cx.floor())
        .planes
        .into_iter()
        .filter(|p| (p.baseline.0.y - p.baseline.1.y).abs() < 1e-6)
        .min_by(|a, b| a.baseline.0.y.total_cmp(&b.baseline.0.y))
        .expect("the fixture has a south roof plane")
}

/// A point inside the first dormer of the floor.
fn dormer_point(sim: &Sim) -> Point {
    let set = roof_view::load(sim.app.cx.floor());
    let d = &set.dormers[0];
    let main = set.plane(d.main).expect("the dormer's plane");
    let (a, b) = main.baseline;
    a + (b - a).normalized() * d.spec.position_along_eave
        + main.up_slope() * d.spec.setback_from_eave
}

fn gesture_text(g: G) -> String {
    match g {
        G::Click(x, y) => format!("click ({x:.0}, {y:.0})"),
        G::Drag(a, b) => format!("drag ({:.0}, {:.0}) to ({:.0}, {:.0})", a.0, a.1, b.0, b.1),
        G::Clicks(pts, end) => {
            let tail = match end {
                End::None => "",
                End::Enter => ", Enter",
                End::EnterEnter => ", Enter, Enter",
            };
            format!("{} clicks{tail}", pts.len())
        }
        G::Text(..) => "click, type, Enter".into(),
        G::DragClick(..) => "drag the baseline, click the side".into(),
        G::ClickOk(x, y) => format!("click ({x:.0}, {y:.0}), OK"),
        G::RichText(..) => "click, type, Tab".into(),
        G::PlaneClick(..) => "click on the south roof plane".into(),
        G::PlaneDrag(..) => "drag on the south roof plane".into(),
        G::PlaneClickOk => "click on the south roof plane, OK".into(),
        G::OnDormer => "click on the dormer".into(),
        G::EaveCorner => "click the eave corner".into(),
        G::OnArc => "click the curved wall, click to place the line".into(),
    }
}

// ---------------------------------------------------------------------------
// What each tool is expected to do
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Role {
    /// Makes an object with this gesture on this plan.
    Creates(Fx, G),
    /// A mode, a modifier, a command or a dialog opener: no object of its own
    /// (the reason is printed).
    NoObject(&'static str),
    /// Changes an existing object (a roof hole, a counter hole, a return):
    /// one undo step, and undo and redo restore the plan.
    Modifies(Fx, G),
    /// Should make an object but no headless gesture does it yet.
    Unknown(&'static str),
}

use Role::{Creates, Modifies, NoObject};

fn role(id: ToolId) -> Role {
    use crate::editor::stairs_view::StairKind as Sk;
    match id {
        ToolId::Select => NoObject("mode: picks and edits"),
        ToolId::Pan => NoObject("mode: pans the view"),
        ToolId::Underlay => NoObject("mode: moves and calibrates a plan underlay"),
        ToolId::Fireplace | ToolId::FireplaceVariant(_) => Creates(Fx::Shell, IN_ROOM),
        ToolId::Painter | ToolId::PainterVariant(_) => {
            NoObject("mode: paints a layer or one object's attributes onto others (s45)")
        }
        ToolId::MaterialsPolyline => NoObject(
            "draws a Materials List Polyline whose own specification the Materials List hosts (s58)",
        ),
        ToolId::ConstructionLine => NoObject(
            "draws a construction line (a CAD line with a record) whose gestures s66 drives",
        ),
        ToolId::TrayCeiling => NoObject(
            "draws a tray ceiling polyline (a CAD polyline with a record) whose gestures s75 drives",
        ),
        ToolId::RoofBaseline => NoObject(
            "draws a roof baseline polyline (a CAD polyline with a record) whose gestures s78 drives",
        ),
        ToolId::ReferenceOffset => NoObject(
            "mode: moves and turns another plan file in the Reference Display (s66)",
        ),
        ToolId::Library => Creates(Fx::Library, IN_ROOM),
        ToolId::Images => NoObject("base id: the Images flyout entries are the tools"),
        ToolId::Wall { .. } => Creates(Fx::Shell, DRAG),
        ToolId::WallVariant(v) => {
            if v.curved {
                Creates(Fx::Shell, G::Clicks(THREE, End::Enter))
            } else {
                Creates(Fx::Shell, DRAG)
            }
        }
        ToolId::Door | ToolId::Window | ToolId::OpeningVariant(_) => Creates(Fx::Shell, ON_WALL),
        ToolId::Dimension => Creates(Fx::Shell, G::Clicks(THREE, End::Enter)),
        ToolId::DimensionVariant(m) => match m {
            DimMode::Manual | DimMode::PointToPoint | DimMode::Centerline | DimMode::Angular => {
                Creates(Fx::Shell, G::Clicks(THREE, End::Enter))
            }
            DimMode::Interior | DimMode::Baseline => Creates(Fx::Shell, G::Clicks(TWO, End::None)),
            DimMode::Running => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            DimMode::AutoExterior
            | DimMode::AutoInterior
            | DimMode::AutoElevation
            | DimMode::AutoStoryPole => Creates(Fx::Shell, IN_ROOM),
            DimMode::EndToEnd => Creates(
                Fx::Split,
                G::Clicks(&[(240.0, 75.0), (240.0, 255.0), (200.0, 200.0)], End::None),
            ),
            DimMode::TapeMeasure => NoObject("measures only"),
            DimMode::Radius | DimMode::ArcLength => {
                NoObject("needs a curved wall: s54_dimensions_r15 drives it")
            }
            DimMode::AutoNkba => Creates(Fx::Cabinets, IN_ROOM),
            DimMode::Radius | DimMode::ArcLength => Creates(Fx::Curved, G::OnArc),
            DimMode::ExtensionAdd | DimMode::ExtensionDelete => {
                NoObject("modifier: edits a dimension's extension lines")
            }
        },
        ToolId::Text => Creates(Fx::Shell, G::Text(100.0, 200.0)),
        ToolId::TextVariant(m) => match m {
            TextMode::Text => Creates(Fx::Shell, G::Text(100.0, 200.0)),
            // Round 15: a click opens the Note Specification; OK places it.
            TextMode::Note => Creates(Fx::Shell, G::ClickOk(100.0, 200.0)),
            TextMode::RichText => Creates(Fx::Shell, G::RichText(100.0, 200.0)),
            TextMode::LeaderLine => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            TextMode::ArrowLine => Creates(Fx::Shell, G::Clicks(TWO, End::EnterEnter)),
            TextMode::Marker => Creates(Fx::Shell, G::ClickOk(240.0, 180.0)),
            TextMode::Callout => Creates(Fx::Shell, G::ClickOk(100.0, 200.0)),
            TextMode::NoteTypes | TextMode::Macros | TextMode::TextStyles => {
                NoObject("opens a management dialog")
            }
        },
        ToolId::Cad => Creates(Fx::Shell, DRAG),
        ToolId::CadVariant(m) => match m {
            CadMode::Line
            | CadMode::InputLine
            | CadMode::LineArrow
            | CadMode::Circle
            | CadMode::CircleAboutCenter
            | CadMode::Oval
            | CadMode::RectPolyline
            | CadMode::Polygon => Creates(Fx::Shell, DRAG),
            CadMode::Polyline | CadMode::Spline => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            CadMode::Arc
            | CadMode::ArcArrow
            | CadMode::Ellipse
            | CadMode::Box
            | CadMode::CrossBox
            | CadMode::BlockingBox
            | CadMode::Insulation
            | CadMode::RevisionCloud => Creates(Fx::Shell, G::Clicks(THREE, End::Enter)),
            CadMode::PlacePoint | CadMode::PointMarker => Creates(Fx::Shell, IN_ROOM),
            CadMode::DetailFromView => Creates(Fx::Shell, G::ClickOk(240.0, 180.0)),
            CadMode::InputPoint | CadMode::InputArc => {
                NoObject("typed-coordinate entry: needs typed values")
            }
            CadMode::DeleteTempPoints => NoObject("command: removes points"),
            CadMode::MakeBlock
            | CadMode::ExplodeBlock
            | CadMode::EditBlock
            | CadMode::BlockManagement
            | CadMode::InsertBlock
            | CadMode::AddInsertionPoint
            | CadMode::AddBackoffPoint => NoObject("CAD block tool: acts on CAD objects"),
            CadMode::Fillet
            | CadMode::Chamfer
            | CadMode::Offset
            | CadMode::Trim
            | CadMode::Extend
            | CadMode::BreakLine
            | CadMode::ChangeLineArc
            | CadMode::DeleteBreak
            | CadMode::MakeArcTangent
            | CadMode::ReverseDirection
            | CadMode::MakeParallel
            | CadMode::MakePerpendicular
            | CadMode::ConvertToPolyline
            | CadMode::ConvertToSpline
            | CadMode::PolylineToLines
            | CadMode::Hatch => NoObject("modifier: edits existing CAD objects"),
        },
        ToolId::Cabinet => Creates(Fx::Shell, IN_ROOM),
        ToolId::CabinetVariant(k) => match k {
            CabinetKind::CustomCountertop | CabinetKind::SoffitPolygon => Creates(Fx::Shell, DRAG),
            CabinetKind::CustomBacksplash => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            CabinetKind::CounterHole => {
                Modifies(Fx::Countertop, G::Drag((150.0, 130.0), (220.0, 170.0)))
            }
            _ => Creates(Fx::Shell, IN_ROOM),
        },
        ToolId::Stairs => Creates(Fx::Shell, IN_ROOM),
        ToolId::StairsVariant(k) => match k {
            Sk::Landing => Creates(Fx::Shell, DRAG),
            _ => Creates(Fx::Shell, IN_ROOM),
        },
        ToolId::Roof => Creates(Fx::Shell, PLANE),
        ToolId::RoofVariant(m) => match m {
            RoofMode::Plane | RoofMode::Ceiling => Creates(Fx::Shell, PLANE),
            RoofMode::Build => Creates(Fx::Shell, G::ClickOk(240.0, 180.0)),
            RoofMode::GableLine => Creates(Fx::Roofed, ON_WALL),
            RoofMode::Hole => Modifies(Fx::Roofed, G::PlaneDrag((-80.0, -20.0), (-40.0, 20.0))),
            RoofMode::Skylight => Modifies(Fx::Roofed, G::PlaneDrag((40.0, -24.0), (88.0, 24.0))),
            RoofMode::Dormer | RoofMode::FloatingDormer => Creates(Fx::Roofed, G::PlaneClickOk),
            RoofMode::Explode => Modifies(Fx::Dormered, G::OnDormer),
            RoofMode::Return => Modifies(Fx::Roofed, G::EaveCorner),
            RoofMode::Edit | RoofMode::EditAll => NoObject("modifier: edits roof planes"),
            RoofMode::Join => NoObject("modifier: joins two roof planes"),
        },
        ToolId::Electrical => Creates(Fx::Shell, ON_WALL),
        ToolId::ElectricalVariant(v) => match v {
            ElecVariant::OutletFloor
            | ElecVariant::Light
            | ElecVariant::RecessedLight
            | ElecVariant::PendantLight
            | ElecVariant::RopeLight
            | ElecVariant::CeilingFan
            | ElecVariant::SmokeDetector
            | ElecVariant::AutoOutlets => Creates(Fx::Shell, IN_ROOM),
            ElecVariant::AutoSwitches => Creates(Fx::Furnished, IN_ROOM),
            ElecVariant::Connection => NoObject("connects two devices"),
            _ => Creates(Fx::Shell, ON_WALL),
        },
        ToolId::Camera => Creates(Fx::Shell, IN_ROOM),
        ToolId::CameraVariant(v) => {
            use crate::tools::camera::CameraVariant as Cv;
            match v {
                Cv::FullCamera | Cv::FloorCamera => Creates(Fx::Shell, IN_ROOM),
                Cv::CrossSection | Cv::BackClippedSection => Creates(Fx::Shell, DRAG),
                Cv::WallElevation => Creates(Fx::Shell, ON_WALL),
                Cv::AutoElevation | Cv::AutoBackclipped | Cv::AutoInterior => {
                    Creates(Fx::Shell, IN_ROOM)
                }
                Cv::Walkthrough => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
                Cv::GlassHouse | Cv::FullOverview | Cv::FloorOverview | Cv::DollHouse => {
                    NoObject("opens an overview of the 3D view")
                }
                Cv::AddLights => NoObject("opens the lights dialog"),
            }
        }
        ToolId::Terrain => Creates(Fx::Shell, G::Clicks(THREE, End::Enter)),
        ToolId::TerrainVariant(v) => match v {
            Tv::RectFeature | Tv::RoundFeature | Tv::NorthPointer | Tv::ScaleBar => {
                Creates(Fx::Shell, DRAG)
            }
            Tv::BuildingPad | Tv::CulDeSac => Creates(Fx::Shell, IN_ROOM),
            Tv::ReferencePoint => Modifies(Fx::Perimeter, IN_ROOM),
            Tv::RemoveReferencePoint => Modifies(Fx::RefPoint, G::Click(0.0, 0.0)),
            Tv::TerrainLabels => Modifies(Fx::Road, G::Click(200.0, 100.0)),
            Tv::AutoSidewalk => Modifies(Fx::Road, G::Clicks(&[(200.0, 100.0)], End::EnterEnter)),
            Tv::ImportGps | Tv::GrowPlants => NoObject("command: opens an assistant dialog"),
            Tv::ElevationPoint
            | Tv::StonePolyline
            | Tv::StoneSpline
            | Tv::StraightWall
            | Tv::StraightCurb
            | Tv::PlantPolyline
            | Tv::PlantSpline
            | Tv::SprinklerPolyline
            | Tv::SprinklerSpline => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            Tv::ElevationLine
            | Tv::ElevationSpline
            | Tv::Break
            | Tv::Road
            | Tv::Driveway
            | Tv::Sidewalk
            | Tv::RoadMarking
            | Tv::SplineRoad
            | Tv::SplineDriveway
            | Tv::SplineSidewalk
            | Tv::SplineRoadMarking => Creates(Fx::Shell, G::Clicks(TWO, End::EnterEnter)),
            Tv::ElevationRegion | Tv::Hill | Tv::Valley | Tv::Raised | Tv::Lowered => {
                Creates(Fx::Shell, G::Clicks(THREE, End::EnterEnter))
            }
            Tv::Build | Tv::ImportData | Tv::CutFillReport => {
                NoObject("command: builds terrain or opens a dialog")
            }
            _ => Creates(Fx::Shell, G::Clicks(THREE, End::Enter)),
        },
        ToolId::Foundation => Creates(Fx::Shell, DRAG),
        ToolId::FoundationVariant(v) => match v {
            FoundationVariant::SquarePad | FoundationVariant::RoundPier => {
                Creates(Fx::Shell, IN_ROOM)
            }
            _ => Creates(Fx::Shell, DRAG),
        },
        ToolId::Details => Creates(Fx::Shell, DRAG),
        ToolId::DetailsVariant(v) => match v {
            Dt::CornerBoards | Dt::Quoins => Creates(Fx::Shell, G::Click(5.0, 5.0)),
            Dt::AutoCornerBoards | Dt::AutoQuoins => Creates(Fx::Shell, IN_ROOM),
            Dt::WallHatching | Dt::WallMaterialRegion => Creates(Fx::Shell, ON_WALL),
            Dt::MoldingPolyline => Creates(Fx::Shell, G::Clicks(TWO, End::Enter)),
            Dt::Component => NoObject("places the Detail Components window's block"),
            Dt::ReplaceMoldings => NoObject("swaps the profile of a molding that is already placed"),
            _ => Creates(Fx::Shell, DRAG),
        },
        ToolId::Framing => Creates(Fx::Shell, DRAG),
        ToolId::FramingVariant(v) => {
            use crate::tools::framing::FramingVariant as Fv;
            match v {
                Fv::Post | Fv::PostWithFooting | Fv::ReferenceMarker => Creates(Fx::Shell, IN_ROOM),
                Fv::TrussBase => Creates(Fx::Shell, G::Clicks(THREE, End::Enter)),
                _ => Creates(Fx::Shell, DRAG),
            }
        }
        ToolId::Schedule | ToolId::ScheduleVariant(_) => Creates(Fx::Shell, IN_ROOM),
        ToolId::ImagesVariant(m) => match m {
            ImageMode::CreateImage | ImageMode::BillboardImage | ImageMode::ImageLibrary => {
                NoObject("opens a native file picker")
            }
            ImageMode::PointToPointResize | ImageMode::RotateToAlign => {
                NoObject("traces a picture that is already placed")
            }
            ImageMode::SolidFeature => Creates(Fx::Library, IN_ROOM),
            ImageMode::PolylinePath | ImageMode::SplinePath => {
                Creates(Fx::Library, G::Clicks(TWO, End::Enter))
            }
            ImageMode::PolylineRegion | ImageMode::SplineRegion => {
                Creates(Fx::Library, G::Clicks(THREE, End::Enter))
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Objects
// ---------------------------------------------------------------------------

/// Everything on the active floor a click could select, plus the terrain.
fn objects(sim: &Sim) -> Vec<ObjectRef> {
    let cx = &sim.app.cx;
    let mut v = selection::all_selectable(cx);
    if site_view::load_terrain(&cx.project).is_some() {
        v.push(ObjectRef::Terrain);
    }
    v
}

fn undo_steps(sim: &Sim) -> usize {
    sim.app.cx.action_history().0.len()
}

fn project_text(sim: &Sim) -> String {
    format!("{:?}", sim.app.cx.project)
}

// ---------------------------------------------------------------------------
// Reading a dialog off the frame it paints
// ---------------------------------------------------------------------------

/// What a dialog shows: the window title and the tab list down its left edge.
#[derive(Clone, Debug, Default)]
struct Dlg {
    title: String,
    tabs: Vec<String>,
}

type Texts = Vec<(f32, f32, String)>;

fn flat(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(v) => v.into_iter().for_each(|s| flat(s, out)),
        other => out.push(other),
    }
}

/// Every string a frame paints, with its position, top to bottom.
fn texts_of(out: egui::FullOutput) -> Texts {
    let mut leaves = Vec::new();
    for c in out.shapes {
        flat(c.shape, &mut leaves);
    }
    let mut texts: Texts = leaves
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some((t.pos.x, t.pos.y, t.galley.text().to_string())),
            _ => None,
        })
        .collect();
    texts.sort_by(|a, b| (a.1, a.0).partial_cmp(&(b.1, b.0)).unwrap());
    texts.dedup();
    texts
}

fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    }
}

fn key_event(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// One headless frame of every dialog host (the shell's dialogs, the
/// build-tools windows, the exchange windows). The canvas overlay is not
/// drawn, so its readouts do not mix with the dialog's text.
fn dialog_texts(sim: &mut Sim, events: Vec<egui::Event>) -> Texts {
    let ctx = sim.ctx.clone();
    let mut cam = sim.app.camera;
    let out = ctx.run(raw_input(events), |ctx| {
        sim.app.dialogs(ctx);
        crate::dialogs::build_tools::show_all(ctx, &mut sim.app.cx, &mut cam);
        crate::dialogs::exchange::show_all(ctx, &mut sim.app.cx);
    });
    sim.app.camera = cam;
    sim.app.tools.frame(&mut sim.app.cx, &ctx);
    sim.app.process_requests();
    sim.app.cx.refresh();
    texts_of(out)
}

const BUTTONS: &[&str] = &["Help", "Cancel", "OK", "Apply"];

/// The title and tabs in a frame's text: the title is the topmost string; the
/// tabs are the strings in the leftmost column below it (the page is further
/// right; Help / Cancel / OK are the bottom row).
fn dialog_of(texts: &Texts) -> Dlg {
    let Some(first) = texts.first() else {
        return Dlg::default();
    };
    let below: Vec<&(f32, f32, String)> = texts
        .iter()
        .skip(1)
        .filter(|t| !BUTTONS.contains(&t.2.as_str()))
        .collect();
    let x0 = below.iter().map(|t| t.0).fold(f32::MAX, f32::min);
    Dlg {
        title: first.2.clone(),
        tabs: below
            .iter()
            .filter(|t| t.0 < x0 + 12.0)
            .map(|t| t.2.clone())
            .collect(),
    }
}

/// Lays the open dialog out (three frames) and reads its title and tabs.
fn read_dialog(sim: &mut Sim) -> Dlg {
    let mut texts = Vec::new();
    for _ in 0..3 {
        texts = dialog_texts(sim, Vec::new());
    }
    dialog_of(&texts)
}

/// The Camera Specification is hosted by the 3D panel, which the headless
/// application does not run: draw the dialog the panel would open.
fn read_camera_dialog(sim: &mut Sim, id: plan_core::Id) -> Dlg {
    use crate::dialogs::camera::{CameraDialog, CameraExtras};
    let cam = sim.app.cx.project.camera(id).cloned().expect("camera");
    let floor = sim
        .app
        .cx
        .project
        .floors
        .get(cam.floor)
        .map_or(String::new(), |f| f.name.clone());
    let mut dialog = CameraDialog::new(&cam, &floor, CameraExtras::default());
    let ctx = egui::Context::default();
    let mut texts = Vec::new();
    for _ in 0..3 {
        let out = ctx.run(raw_input(Vec::new()), |ctx| {
            let _ = dialog.show(ctx);
        });
        texts = texts_of(out);
    }
    dialog_of(&texts)
}

// ---------------------------------------------------------------------------
// Chief's names
// ---------------------------------------------------------------------------

/// What Chief's dialog is called and which tabs it has. `title` is `None`
/// where the title was not captured.
#[derive(Clone, Copy, Debug)]
struct Want {
    title: Option<&'static str>,
    tabs: &'static [&'static str],
    /// Where the list comes from.
    source: &'static str,
}

const CAPTURED: &str = "captured (docs/chief-x18-dialogs.md)";
const PATTERN: &str = "pattern: General, object tabs, Layer, Materials, Label, Components, Object Information, Schedule (docs/chief-x18-dialogs.md 'Still to capture'); verify in Chief";
const MINIMUM: &str = "minimum: General and Layer (Chief's common frame); verify in Chief";

const WALL_TABS: &[&str] = &[
    "General",
    "Structure",
    "Roof",
    "Foundation",
    "Wall Types",
    "Wall Cap",
    "Wall Covering",
    "Rail Style",
    "Newels/Balusters",
    "Rails",
    "Layer",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Schedule",
];
const DOOR_TABS: &[&str] = &[
    "General",
    "Options",
    "Casing",
    "Lintel",
    "Sill/Threshold",
    "Lites",
    "Jamb",
    "Arch",
    "Hardware",
    "Shutters",
    "Opening Indicators",
    "Rough Opening",
    "Framing",
    "Energy Values",
    "Layer",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Schedule",
];
const WINDOW_TABS: &[&str] = &[
    "General",
    "Options",
    "Casing",
    "Lintel",
    "Sill/Threshold",
    "Sash",
    "Frame",
    "Lites",
    "Shape",
    "Arch",
    "Treatments",
    "Shutters",
    "Opening Indicators",
    "Rough Opening",
    "Framing",
    "Energy Values",
    "Layer",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Schedule",
];
const CABINET_TABS: &[&str] = &[
    "General",
    "Box Construction",
    "Front/Sides/Back",
    "Door/Drawer",
    "Accessories",
    "Opening Indicators",
    "Moldings",
    "Layer",
    "Fill Style",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Schedule",
];
const ROOM_TABS: &[&str] = &[
    "General",
    "Structure",
    "Moldings",
    "Layer",
    "Fill Style",
    "Materials",
    "Label",
    "Components",
];
const DIMENSION_TABS: &[&str] = &[
    "General",
    "Primary Format",
    "Secondary Format",
    "Extensions",
    "Layer",
    "Arrow",
    "Text Style",
];
const PATTERN_TABS: &[&str] = &[
    "General",
    "Layer",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Schedule",
];
const MINIMUM_TABS: &[&str] = &["General", "Layer"];
const CALLOUT_TABS: &[&str] = &[
    "Callout",
    "Attributes",
    "Line Style",
    "Section Arrow",
    "Main Text Style",
    "Link",
];
const FIREPLACE_TABS: &[&str] = &[
    "General",
    "Hearth",
    "Mantel",
    "Chimney",
    "Materials",
    "Label",
    "Components",
    "Object Information",
    "Layer",
];
const MARKER_TABS: &[&str] = &["Marker", "Line Style", "Text Style"];
const NOTE_TABS: &[&str] = &[
    "Note",
    "Line Style",
    "Text Style",
    "Object Information",
    "Schedule",
];

fn want_for(o: ObjectRef, id: ToolId) -> Want {
    let w = |title: Option<&'static str>, tabs, source| Want {
        title,
        tabs,
        source,
    };
    match o {
        ObjectRef::Wall(_) => w(Some("Wall Specification"), WALL_TABS, CAPTURED),
        ObjectRef::Opening(_) => {
            let window = matches!(id, ToolId::Window)
                || matches!(id, ToolId::OpeningVariant(v) if v.kind == plan_core::OpeningKind::Window);
            if window {
                w(Some("Window Specification"), WINDOW_TABS, CAPTURED)
            } else {
                w(Some("Door Specification"), DOOR_TABS, CAPTURED)
            }
        }
        ObjectRef::Cabinet(_) => w(Some("Cabinet Specification"), CABINET_TABS, CAPTURED),
        ObjectRef::Room(_) => w(Some("Room Specification"), ROOM_TABS, CAPTURED),
        ObjectRef::Dimension(_) => w(
            Some("Dimension Specification"),
            DIMENSION_TABS,
            "captured Dimension Defaults tabs minus Setup and Locate (docs/chief-x18-dialogs.md, DIM-39); verify in Chief",
        ),
        ObjectRef::Stair(_) => w(Some("Staircase Specification"), PATTERN_TABS, PATTERN),
        ObjectRef::RoofPlane(_) | ObjectRef::Framing(_) | ObjectRef::Foundation(_) => {
            w(None, PATTERN_TABS, PATTERN)
        }
        ObjectRef::Device(_) => w(
            Some("Electrical Service Specification"),
            PATTERN_TABS,
            PATTERN,
        ),
        ObjectRef::Terrain => w(Some("Terrain Specification"), PATTERN_TABS, PATTERN),
        ObjectRef::Camera(_) => w(Some("Camera Specification"), PATTERN_TABS, PATTERN),
        ObjectRef::Text(_) => w(None, PATTERN_TABS, PATTERN),
        // Manual pp. 759 to 760: Layer, Materials and Components panels.
        ObjectRef::Symbol(_)
            if matches!(id, ToolId::Fireplace | ToolId::FireplaceVariant(_)) =>
        {
            w(
                Some("Fireplace Specification"),
                FIREPLACE_TABS,
                "manual pp. 759 to 760 (docs/parity/cabinets-stairs-framing-terrain-library.md CB-501 to CB-503); verify in Chief",
            )
        }
        // Callouts, markers and notes (manual pp. 552, 556, 561).
        ObjectRef::Cad(_) if matches!(id, ToolId::TextVariant(TextMode::Callout)) => w(
            Some("Callout Specification"),
            CALLOUT_TABS,
            "manual p. 552: Callout, Attributes, Line Style, Section Arrow, Main Text Style, Link",
        ),
        ObjectRef::Cad(_) if matches!(id, ToolId::TextVariant(TextMode::Marker)) => w(
            Some("Marker Specification"),
            MARKER_TABS,
            "manual p. 556: Marker, Line Style, Text Style",
        ),
        ObjectRef::Cad(_) if matches!(id, ToolId::TextVariant(TextMode::Note)) => w(
            Some("Note Specification"),
            NOTE_TABS,
            "manual p. 561: Note, Line Style, Text Style, Object Information, Schedule",
        ),
        _ => w(None, MINIMUM_TABS, MINIMUM),
    }
}

// ---------------------------------------------------------------------------
// One tool, end to end
// ---------------------------------------------------------------------------

/// What one case found. Plain data so it crosses the thread boundary.
#[derive(Clone, Debug, Default)]
struct Report {
    id: String,
    name: String,
    role: String,
    gesture: String,
    made: Vec<String>,
    undo_steps: usize,
    title: String,
    tabs: Vec<String>,
    want_title: String,
    want_tabs: Vec<String>,
    source: String,
    missing: Vec<String>,
    extra: Vec<String>,
    edit_path: String,
    /// Defects: the test fails on these unless the row is in [`KNOWN_GAPS`].
    problems: Vec<String>,
    /// Parity differences (title, missing tabs): reported, never fail.
    notes: Vec<String>,
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "panic".into()
    }
}

/// Runs `f` on a fresh thread with a big stack: its thread-local dialog
/// windows and room selection start empty and die with it.
fn in_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .map_err(|e| e.to_string())?
        .join()
        .map_err(panic_text)
}

/// Selects `o` with the Select tool, runs Open Object from the Edit toolbar
/// and reports whether the toolbar offered it.
fn open_object(sim: &mut Sim, o: ObjectRef) -> bool {
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(o);
    let bar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    let offered = bar
        .iter()
        .any(|a| a.kind == EditActionKind::OpenObject && a.enabled);
    if offered {
        sim.app.cx.apply_edit_action(EditActionKind::OpenObject);
    } else {
        sim.app.cx.requests.push(EditorRequest::OpenSpec(o));
    }
    sim.app.process_requests();
    offered
}

/// The object whose dialog represents what a tool made.
fn main_object(made: &[ObjectRef]) -> ObjectRef {
    made.iter()
        .copied()
        .find(|o| !matches!(o, ObjectRef::Terrain | ObjectRef::Cad(_)))
        .unwrap_or(made[0])
}

/// Tabs to the `tabs`-th focusable widget of the open dialog, types a digit
/// and presses OK; returns whether the plan changed. The caller reopens the
/// dialog between attempts.
fn type_and_ok(sim: &mut Sim, tabs: usize) -> bool {
    let before = project_text(sim);
    dialog_texts(sim, Vec::new());
    dialog_texts(sim, Vec::new());
    for _ in 0..tabs {
        dialog_texts(sim, vec![key_event(Key::Tab)]);
    }
    dialog_texts(sim, vec![egui::Event::Text("7".into())]);
    dialog_texts(sim, vec![key_event(Key::Enter)]);
    dialog_texts(sim, Vec::new());
    flush_tool(sim);
    project_text(sim) != before
}

/// Dialogs a tool hosts (electrical, terrain, roof) apply their OK at the
/// tool's next event; a key it has no use for delivers that event.
fn flush_tool(sim: &mut Sim) {
    sim.key(KeyEvent::key(Key::F24));
}

/// How many Tab presses to try before giving up on the keyboard path.
const TAB_TRIES: &[usize] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32,
];

/// Edits one field through the dialog's draft where the dialog offers test
/// access (walls, openings, dimensions, text, CAD) and presses OK; returns
/// whether it could.
fn edit_draft(sim: &mut Sim, o: ObjectRef) -> bool {
    match o {
        ObjectRef::Wall(_) => {
            let Some(crate::ActiveDialog::Wall(d)) = sim.app.dialog.as_mut() else {
                return false;
            };
            d.draft_mut().height += 6.0;
        }
        ObjectRef::Opening(_) => {
            let Some(crate::ActiveDialog::Opening(d)) = sim.app.dialog.as_mut() else {
                return false;
            };
            d.draft_mut().width += 2.0;
        }
        ObjectRef::Dimension(_) => {
            let Some(d) = sim.app.spec.dimension_draft_mut() else {
                return false;
            };
            d.offset += 6.0;
        }
        ObjectRef::Text(_) => {
            let Some(d) = sim.app.spec.text_draft_mut() else {
                return false;
            };
            if let plan_core::cad::CadItem::Text { text, .. } = &mut d.item {
                text.push('x');
            }
        }
        ObjectRef::Cad(_) if sim.app.spec.annot_dialog_mut().is_some() => {
            if let Some(d) = sim.app.spec.annot_dialog_mut() {
                d.edit_label(|l| l.push('x'));
            }
        }
        ObjectRef::Cad(_) if sim.app.spec.text_draft_mut().is_some() => {
            let Some(d) = sim.app.spec.text_draft_mut() else {
                return false;
            };
            if let plan_core::cad::CadItem::Text { text, .. } = &mut d.item {
                text.push('x');
            } else {
                return false;
            }
        }
        ObjectRef::Cad(_) => {
            use plan_core::cad::CadItem as Ci;
            let Some(d) = sim.app.spec.cad_draft_mut() else {
                return false;
            };
            match &mut d.item {
                Ci::Line { b, .. } => b.x += 6.0,
                Ci::Circle { radius, .. } | Ci::Arc { radius, .. } => *radius += 6.0,
                Ci::Polyline { points, .. } => match points.last_mut() {
                    Some(p) => p.x += 6.0,
                    None => return false,
                },
                Ci::Text { text, .. } => text.push('x'),
            }
        }
        _ => return false,
    }
    dialog_texts(sim, Vec::new());
    dialog_texts(sim, vec![key_event(Key::Enter)]);
    dialog_texts(sim, Vec::new());
    flush_tool(sim);
    true
}

fn run_case(id: ToolId) -> Report {
    let mut r = Report {
        id: format!("{id:?}"),
        ..Report::default()
    };
    let (fx, g) = match role(id) {
        Creates(fx, g) => (fx, g),
        NoObject(why) => {
            r.role = format!("no object: {why}");
            return r;
        }
        Modifies(fx, g) => {
            r.role = "modifies".into();
            r.gesture = format!("{} on {fx:?}", gesture_text(g));
            let mut sim = fixture(fx);
            sim.tool(id);
            r.name = sim.app.tools.active().name().to_string();
            let before = project_text(&sim);
            let steps0 = undo_steps(&sim);
            perform(&mut sim, g);
            r.undo_steps = undo_steps(&sim) - steps0;
            let after = project_text(&sim);
            if after == before {
                r.problems.push(format!(
                    "the gesture changed nothing ({})",
                    sim.app.cx.status
                ));
                return r;
            }
            if r.undo_steps != 1 {
                r.problems
                    .push(format!("{} undo steps for one gesture", r.undo_steps));
            }
            for _ in 0..r.undo_steps {
                sim.undo();
            }
            if project_text(&sim) != before {
                r.problems.push("undo does not restore the plan".into());
            }
            for _ in 0..r.undo_steps {
                sim.redo();
            }
            if project_text(&sim) != after {
                r.problems.push("redo does not restore the change".into());
            }
            return r;
        }
        Role::Unknown(why) => {
            r.role = format!("unknown gesture: {why}");
            r.problems.push(format!("no headless gesture yet ({why})"));
            return r;
        }
    };
    r.role = "creates".into();
    r.gesture = format!("{} on {fx:?}", gesture_text(g));
    let mut sim = fixture(fx);
    sim.tool(id);
    r.name = sim.app.tools.active().name().to_string();
    if r.name.is_empty() || sim.app.tools.active().hint().trim().is_empty() {
        r.problems.push("the tool has no name or hint".into());
    }
    let before = objects(&sim);
    let steps0 = undo_steps(&sim);
    perform(&mut sim, g);
    let after = objects(&sim);
    let made: Vec<ObjectRef> = after
        .iter()
        .filter(|o| !before.contains(o))
        .copied()
        .collect();
    r.undo_steps = undo_steps(&sim) - steps0;
    r.made = made.iter().map(|o| o.type_name().to_string()).collect();
    if made.is_empty() {
        r.problems.push("the gesture created no object".into());
        return r;
    }
    if r.undo_steps != 1 {
        r.problems
            .push(format!("{} undo steps for one gesture", r.undo_steps));
    }
    // Undo takes the object away, redo brings it back.
    let with = project_text(&sim);
    sim.undo();
    let undone = objects(&sim);
    if made.iter().any(|o| undone.contains(o)) {
        r.problems.push("undo leaves the new object".into());
    }
    sim.redo();
    if project_text(&sim) != with {
        r.problems.push("redo does not restore the plan".into());
    }
    sim.app.cx.refresh();

    // ----- the dialog -----
    let o = main_object(&made);
    let steps_before_open = undo_steps(&sim);
    let offered = open_object(&mut sim, o);
    if !offered {
        r.problems
            .push("Open Object is not on the Edit toolbar".into());
    }
    let dlg = match o {
        ObjectRef::Camera(cid) => {
            if !crate::shell::view3d_panel::Outbox::global()
                .take()
                .contains(&crate::shell::view3d_panel::ViewRequest::OpenCameraSpec(
                    cid,
                ))
            {
                r.problems
                    .push("Open Object posts no Camera Specification request".into());
            }
            read_camera_dialog(&mut sim, cid)
        }
        _ => read_dialog(&mut sim),
    };
    let want = want_for(o, id);
    r.title = dlg.title.clone();
    r.tabs = dlg.tabs.clone();
    r.want_title = want.title.unwrap_or("(not captured)").to_string();
    r.want_tabs = want.tabs.iter().map(|s| s.to_string()).collect();
    r.source = want.source.to_string();
    if dlg.title.is_empty() {
        r.problems.push(format!(
            "no specification dialog opens for {}",
            o.type_name()
        ));
        return r;
    }
    if !dlg.title.contains("Specification") {
        r.problems.push(format!(
            "the dialog title '{}' is not a Specification",
            dlg.title
        ));
    }
    if dlg.tabs.is_empty() {
        r.problems.push("the dialog has no tab list".into());
    }
    if let Some(t) = want.title {
        if t != dlg.title {
            r.notes
                .push(format!("title '{}' (Chief: '{t}')", dlg.title));
        }
    }
    r.missing = want
        .tabs
        .iter()
        .filter(|t| !dlg.tabs.iter().any(|x| x == *t))
        .map(|t| t.to_string())
        .collect();
    r.extra = dlg
        .tabs
        .iter()
        .filter(|t| !want.tabs.contains(&t.as_str()))
        .cloned()
        .collect();
    if !r.missing.is_empty() {
        // TODO parity: the dialog lacks these tabs of Chief's list.
        r.notes
            .push(format!("missing tabs: {}", r.missing.join(", ")));
    }
    if undo_steps(&sim) != steps_before_open {
        r.problems
            .push("opening the dialog pushed an undo step".into());
    }

    // ----- edit one field and undo it -----
    let before_edit = project_text(&sim);
    let mut steps_edit0 = undo_steps(&sim);
    let mut changed = false;
    if matches!(o, ObjectRef::Camera(_)) {
        // The Camera Specification is hosted by the 3D panel, which the
        // headless application does not run (the dialog is drawn above).
        r.edit_path = "hosted by the 3D panel (not exercised)".into();
        sim.app.dialog = None;
        sim.app.spec = Default::default();
        return finish_delete(sim, o, r);
    }
    for (n, tabs) in TAB_TRIES.iter().enumerate() {
        if n > 0 {
            sim.app.dialog = None;
            sim.app.spec = Default::default();
            open_object(&mut sim, o);
        }
        steps_edit0 = undo_steps(&sim);
        if type_and_ok(&mut sim, *tabs) {
            changed = true;
            r.edit_path = if *tabs == 0 {
                "typed into the first field".into()
            } else {
                format!("Tab x{tabs}, typed")
            };
            break;
        }
    }
    if !changed {
        // The keyboard did not change anything: edit the draft where the
        // dialog offers test access.
        sim.app.dialog = None;
        sim.app.spec = Default::default();
        open_object(&mut sim, o);
        dialog_texts(&mut sim, Vec::new());
        steps_edit0 = undo_steps(&sim);
        if edit_draft(&mut sim, o) {
            changed = project_text(&sim) != before_edit;
            r.edit_path = "draft field".into();
        }
    }
    if !changed {
        r.edit_path = "none".into();
        r.problems
            .push("editing a field and pressing OK changed nothing".into());
    } else {
        let steps = undo_steps(&sim) - steps_edit0;
        if steps != 1 {
            r.problems
                .push(format!("a dialog edit pushed {steps} undo steps"));
        }
        if sim.app.has_dialog() {
            r.problems.push("OK did not close the dialog".into());
        }
        for _ in 0..steps {
            sim.undo();
        }
        if project_text(&sim) != before_edit {
            r.problems
                .push("undo does not restore the edited field".into());
        }
    }
    // Close anything still open.
    sim.app.dialog = None;
    sim.app.spec = Default::default();
    finish_delete(sim, o, r)
}

/// Delete Objects from the Edit toolbar: gone in one undo step, and undo
/// brings it back.
fn finish_delete(mut sim: Sim, o: ObjectRef, mut r: Report) -> Report {
    // ----- delete -----
    if matches!(o, ObjectRef::Terrain) {
        return r;
    }
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(o);
    let bar = sim.app.tools.active().edit_toolbar(&sim.app.cx);
    if !bar
        .iter()
        .any(|a| a.kind == EditActionKind::Delete && a.enabled)
    {
        r.problems
            .push("Delete Objects is not on the Edit toolbar".into());
        return r;
    }
    let with = project_text(&sim);
    let steps0 = undo_steps(&sim);
    sim.app.cx.apply_edit_action(EditActionKind::Delete);
    sim.app.process_requests();
    sim.app.cx.refresh();
    if objects(&sim).contains(&o) {
        r.problems.push("Delete Objects leaves the object".into());
    } else {
        let steps = undo_steps(&sim) - steps0;
        if steps != 1 {
            r.problems
                .push(format!("Delete Objects pushed {steps} undo steps"));
        }
        for _ in 0..steps {
            sim.undo();
        }
        if project_text(&sim) != with {
            r.problems
                .push("undo does not bring the deleted object back".into());
        }
    }
    r
}

// ---------------------------------------------------------------------------
// Known gaps (each has a QA entry in docs/qa-findings.md)
// ---------------------------------------------------------------------------

/// A written-up defect: the tool ids (prefixes of their `Debug` text) it
/// affects, the problem text it covers, and its entry in docs/qa-findings.md.
struct Gap {
    ids: &'static [&'static str],
    problem: &'static str,
    qa: &'static str,
}

const NOT_DRIVEN: &str = "editing a field and pressing OK changed nothing";

/// Known problems; anything else a tool shows fails the sweep.
const KNOWN_GAPS: &[Gap] = &[];

fn known_qa(id: &str, problem: &str) -> Option<&'static str> {
    KNOWN_GAPS
        .iter()
        .find(|g| problem.contains(g.problem) && g.ids.iter().any(|p| id.starts_with(p)))
        .map(|g| g.qa)
}

/// The QA entries covering a report's problems, `NEW` where one is uncovered.
fn qa_column(r: &Report) -> String {
    let mut out: Vec<&str> = Vec::new();
    for p in &r.problems {
        // A report joins several problems with "; " in the doc, not here.
        let q = known_qa(&r.id, p).unwrap_or("NEW");
        if !out.contains(&q) {
            out.push(q);
        }
    }
    out.join(", ")
}

fn has_new_problem(r: &Report) -> bool {
    r.problems.iter().any(|p| known_qa(&r.id, p).is_none())
}

// ---------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------

fn sweep() -> Vec<Report> {
    let mut out = Vec::new();
    // `S35_ONLY=Cabinet` runs just the tool ids whose name contains the text.
    let only = std::env::var("S35_ONLY").ok();
    for id in all_tool_ids() {
        if only
            .as_deref()
            .is_some_and(|f| !format!("{id:?}").contains(f))
        {
            continue;
        }
        let rep = match in_thread(move || run_case(id)) {
            Ok(r) => r,
            Err(e) => Report {
                id: format!("{id:?}"),
                role: "panicked".into(),
                problems: vec![format!("panicked: {e}")],
                ..Report::default()
            },
        };
        out.push(rep);
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "/")
}

/// The sweep as the markdown of docs/tool-dialog-sweep.md.
fn markdown(reports: &[Report]) -> String {
    let mut s = String::new();
    s.push_str("# Tool and dialog sweep\n\n");
    s.push_str("Generated by `crates/plan-app/src/scenarios/s35_tool_dialog_sweep.rs` (run the test with `S35_WRITE_DOC=1` to rewrite this file). Every tool id the toolbars, flyouts and menus can pick is activated and given its canonical headless gesture; the object it makes is selected, Open Object runs from the Edit toolbar, the dialog's title and tab list are read off the frame it paints and compared with Chief's names, one field is typed into the first field and OK pressed, and Delete Objects removes the object. Undo is checked at every step.\n\n");
    let creators: Vec<&Report> = reports.iter().filter(|r| r.role == "creates").collect();
    let modifiers = reports.iter().filter(|r| r.role == "modifies").count();
    let no_obj: Vec<&Report> = reports
        .iter()
        .filter(|r| r.role.starts_with("no object"))
        .collect();
    let bad: Vec<&Report> = reports.iter().filter(|r| !r.problems.is_empty()).collect();
    s.push_str(&format!(
        "## Headline\n\n* {} tool ids swept: {} create an object, {} change an existing object in one undo step (roof holes, counter holes, returns, explode), {} make no object of their own (modes, commands, dialog openers), {} have defects listed below.\n",
        reports.len(),
        creators.len(),
        modifiers,
        no_obj.len(),
        bad.len()
    ));
    let no_dialog = creators
        .iter()
        .filter(|r| {
            r.problems
                .iter()
                .any(|p| p.contains("no specification dialog"))
        })
        .count();
    s.push_str(&format!(
        "* Tools that create an object whose Open Object opens no dialog: {no_dialog}.\n"
    ));
    let with_missing = creators.iter().filter(|r| !r.missing.is_empty()).count();
    s.push_str(&format!(
        "* Creating tools whose dialog lacks tabs of Chief's list: {with_missing} (table below).\n\n"
    ));

    s.push_str("## Defects\n\n");
    if bad.is_empty() {
        s.push_str("None.\n\n");
    } else {
        s.push_str("| Tool id | Gesture | Problem | Known |\n|---|---|---|---|\n");
        for r in &bad {
            s.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                md_escape(&r.id),
                md_escape(&r.gesture),
                md_escape(&r.problems.join("; ")),
                qa_column(r)
            ));
        }
        s.push('\n');
    }

    s.push_str("## Missing tabs, by dialog\n\nOne row per distinct dialog title with the tabs Chief's list has and Plan Studio's dialog lacks. `Source` says where the Chief list comes from; rows marked pattern were never captured and are the least certain.\n\n");
    s.push_str("| Dialog (actual title) | Chief title | Tabs shown | Missing | Extra | Source |\n|---|---|---|---|---|---|\n");
    let mut seen: BTreeMap<String, &Report> = BTreeMap::new();
    for r in &creators {
        if !r.title.is_empty() {
            seen.entry(format!("{}|{}", r.title, r.tabs.join(",")))
                .or_insert(r);
        }
    }
    for r in seen.values() {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            md_escape(&r.title),
            md_escape(&r.want_title),
            md_escape(&r.tabs.join(", ")),
            md_escape(&if r.missing.is_empty() {
                "-".to_string()
            } else {
                r.missing.join(", ")
            }),
            md_escape(&if r.extra.is_empty() {
                "-".to_string()
            } else {
                r.extra.join(", ")
            }),
            md_escape(&r.source),
        ));
    }

    s.push_str("\n## Every tool\n\n| Tool id | Tool name | Result | Gesture | Object made | Undo steps | Dialog | Edit path |\n|---|---|---|---|---|---|---|---|\n");
    for r in reports {
        let made = if r.made.is_empty() {
            "-".to_string()
        } else {
            let mut kinds: Vec<String> = Vec::new();
            for k in &r.made {
                if !kinds.contains(k) {
                    kinds.push(k.clone());
                }
            }
            let n = r.made.len();
            format!(
                "{}{}",
                kinds.join(", "),
                if n > 1 {
                    format!(" ({n})")
                } else {
                    String::new()
                }
            )
        };
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            md_escape(&r.id),
            md_escape(&r.name),
            md_escape(&r.role),
            md_escape(&r.gesture),
            md_escape(&made),
            if r.role == "creates" || r.role == "modifies" {
                r.undo_steps.to_string()
            } else {
                "-".into()
            },
            md_escape(&if r.title.is_empty() {
                "-".to_string()
            } else {
                r.title.clone()
            }),
            md_escape(&r.edit_path),
        ));
    }
    s
}

#[test]
fn every_creating_tool_makes_an_object_with_a_chief_dialog_that_edits_and_deletes() {
    let reports = sweep();
    let md = markdown(&reports);
    eprintln!("{md}");
    if std::env::var("S35_WRITE_DOC").is_ok() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/tool-dialog-sweep.md"
        );
        if let Err(e) = std::fs::write(path, &md) {
            eprintln!("could not write docs/tool-dialog-sweep.md: {e}");
        }
    }
    let creators = reports.iter().filter(|r| r.role == "creates").count();
    if std::env::var("S35_ONLY").is_err() {
        assert!(creators >= 150, "only {creators} creating tools");
    }
    let new_defects: Vec<String> = reports
        .iter()
        .filter(|r| has_new_problem(r))
        .map(|r| format!("{}: {}", r.id, r.problems.join("; ")))
        .collect();
    assert!(
        new_defects.is_empty(),
        "tools with defects outside KNOWN_GAPS:\n{}",
        new_defects.join("\n")
    );
}

// ---------------------------------------------------------------------------
// The Edit toolbar of every object kind
// ---------------------------------------------------------------------------

/// One creating tool per kind of object (and per dialog family).
const REPRESENTATIVES: &[ToolId] = &[
    ToolId::Wall {
        kind: WallKind::Exterior,
    },
    ToolId::Wall {
        kind: WallKind::Interior,
    },
    ToolId::Door,
    ToolId::Window,
    ToolId::CabinetVariant(CabinetKind::Base),
    ToolId::CabinetVariant(CabinetKind::CustomCountertop),
    ToolId::StairsVariant(crate::editor::stairs_view::StairKind::Straight),
    ToolId::StairsVariant(crate::editor::stairs_view::StairKind::Landing),
    ToolId::RoofVariant(RoofMode::Plane),
    ToolId::RoofVariant(RoofMode::Ceiling),
    ToolId::DimensionVariant(DimMode::Manual),
    ToolId::TextVariant(TextMode::Text),
    ToolId::CadVariant(CadMode::Line),
    ToolId::CadVariant(CadMode::Circle),
    ToolId::CadVariant(CadMode::Polyline),
    ToolId::ElectricalVariant(ElecVariant::Outlet110),
    ToolId::ElectricalVariant(ElecVariant::Light),
    ToolId::TerrainVariant(Tv::Perimeter),
    ToolId::TerrainVariant(Tv::ElevationPoint),
    ToolId::TerrainVariant(Tv::RectFeature),
    ToolId::TerrainVariant(Tv::Road),
    ToolId::FoundationVariant(FoundationVariant::Slab),
    ToolId::FoundationVariant(FoundationVariant::SquarePad),
    ToolId::FramingVariant(crate::tools::framing::FramingVariant::General),
    ToolId::DetailsVariant(Dt::MoldingLine),
    ToolId::DetailsVariant(Dt::Solid3d),
    ToolId::ScheduleVariant(plan_core::schedules::ScheduleKind::Door),
    ToolId::CameraVariant(crate::tools::camera::CameraVariant::FullCamera),
    ToolId::Library,
];

/// Makes what `id`'s gesture makes and selects the main object.
fn make_and_select(id: ToolId) -> Option<(Sim, ObjectRef)> {
    let Creates(fx, g) = role(id) else {
        return None;
    };
    let mut sim = fixture(fx);
    sim.tool(id);
    let before = objects(&sim);
    perform(&mut sim, g);
    let made: Vec<ObjectRef> = objects(&sim)
        .into_iter()
        .filter(|o| !before.contains(o))
        .collect();
    if made.is_empty() {
        return None;
    }
    let o = main_object(&made);
    sim.tool(ToolId::Select);
    sim.app.cx.selection.set(o);
    Some((sim, o))
}

/// `(label, kind)` of every enabled Edit toolbar button for the object `id` makes.
fn edit_buttons(id: ToolId) -> Result<Vec<(String, EditActionKind)>, String> {
    in_thread(move || {
        let Some((sim, _)) = make_and_select(id) else {
            return Vec::new();
        };
        sim.app
            .tools
            .active()
            .edit_toolbar(&sim.app.cx)
            .into_iter()
            .filter(|a| a.enabled)
            .map(|a| (a.label.to_string(), a.kind))
            .collect()
    })
}

/// Runs one Edit toolbar button on a fresh object and says what it did.
fn run_edit_button(id: ToolId, kind: EditActionKind) -> Result<String, String> {
    in_thread(move || {
        let Some((mut sim, _)) = make_and_select(id) else {
            return "no object".to_string();
        };
        let project = project_text(&sim);
        let status = sim.app.cx.status.clone();
        sim.app.cx.apply_edit_action(kind);
        sim.app.process_requests();
        let dlg = read_dialog(&mut sim);
        sim.app.cx.refresh();
        if !dlg.title.is_empty() {
            format!("opened {}", dlg.title)
        } else if project_text(&sim) != project {
            "changed the plan".to_string()
        } else if sim.app.cx.status != status {
            format!("status: {}", sim.app.cx.status)
        } else {
            "no visible effect".to_string()
        }
    })
}

#[test]
fn every_edit_toolbar_action_of_every_object_kind_runs_without_panicking() {
    let mut rows: Vec<String> = Vec::new();
    let mut panics: Vec<String> = Vec::new();
    let mut ran = 0;
    for id in REPRESENTATIVES.iter().copied() {
        let buttons = match edit_buttons(id) {
            Ok(b) => b,
            Err(e) => {
                panics.push(format!("{id:?}: building the Edit toolbar panicked: {e}"));
                continue;
            }
        };
        if buttons.is_empty() {
            panics.push(format!("{id:?}: no Edit toolbar for its object"));
            continue;
        }
        let mut open = false;
        let mut delete = false;
        for (label, kind) in buttons {
            open |= kind == EditActionKind::OpenObject;
            delete |= kind == EditActionKind::Delete;
            match run_edit_button(id, kind) {
                Ok(effect) => {
                    ran += 1;
                    rows.push(format!("{id:?} / {label}: {effect}"));
                }
                Err(e) => panics.push(format!("{id:?} / {label}: panicked: {e}")),
            }
        }
        if !open || !delete {
            panics.push(format!("{id:?}: Edit toolbar lacks Open Object or Delete"));
        }
    }
    eprintln!("{} Edit toolbar actions run:\n  {}", ran, rows.join("\n  "));
    assert!(ran >= 150, "only {ran} Edit toolbar actions ran");
    assert!(panics.is_empty(), "{panics:#?}");
}

// ---------------------------------------------------------------------------
// Every command is live
// ---------------------------------------------------------------------------

/// The rows of the Edit menu that are drawn dimmed (`inert(..)`), read from
/// the source of `menus.rs`: the existing check there skips the Edit menu.
fn dimmed_edit_rows() -> Vec<String> {
    let src = include_str!("../menus.rs");
    let start = src.find("\nfn edit_menu").expect("edit_menu");
    let body = &src[start + 1..];
    let end = body[1..].find("\nfn ").map_or(body.len(), |e| e + 1);
    let body = &body[..end];
    let mut rows = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("inert(") {
        rest = &rest[i + "inert(".len()..];
        let call = &rest[..rest.find(");").unwrap_or(rest.len())];
        let mut parts = call.split('"');
        parts.next();
        while let Some(s) = parts.next() {
            rows.push(s.replace("\\u{2026}", "\u{2026}"));
            parts.next();
        }
    }
    rows.retain(|r| r != "-");
    rows
}

/// Toolbar buttons that are drawn but not built yet (QA-17); a new dead
/// button fails the test.
const KNOWN_STUBS: &[&str] = &[
    "File and Edit / Display Options",
    "File and Edit / Plan Database",
    "File and Edit / Default Configuration",
    "File and Edit / Space Planning Configuration",
    "File and Edit / Extended Tool Configuration",
];

#[test]
fn every_toolbar_and_menu_command_is_live_and_names_a_tool_that_activates() {
    let commands = collect_commands();
    assert!(commands.len() >= 300, "{} commands", commands.len());
    let dead: Vec<String> = commands
        .iter()
        .filter(|c| !c.is_live())
        .map(|c| format!("{} / {}", c.group, c.name))
        .collect();
    eprintln!(
        "{} commands, {} not live: {dead:?}",
        commands.len(),
        dead.len()
    );
    let new_dead: Vec<&String> = dead
        .iter()
        .filter(|d| !KNOWN_STUBS.contains(&d.as_str()))
        .collect();
    assert!(
        new_dead.is_empty(),
        "commands that are still stubs outside KNOWN_STUBS: {new_dead:?}"
    );
    // Every command that picks a tool leaves exactly that tool active.
    let mut wrong: Vec<String> = Vec::new();
    for c in &commands {
        if let Action::SetTool(id) = c.action {
            let name = c.name.clone();
            let ok = in_thread(move || {
                let mut sim = Sim::new();
                sim.action(Action::SetTool(id));
                sim.app.tools.active_id().same_tool(id)
                    && !sim.app.tools.active().name().is_empty()
                    && sim.app.cx.status != "Tool not implemented yet"
            });
            if ok != Ok(true) {
                wrong.push(format!("{name} ({id:?}): {ok:?}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "commands that do not activate their tool: {wrong:?}"
    );
}

/// The Edit menu's dimmed rows: the existing source check in `menus.rs`
/// skips the Edit menu, so its dimmed rows are pinned here. The three macOS
/// system rows are placeholders for what macOS adds to an Edit menu; the other
/// two are Chief commands without a handler (QA-12.. in docs/qa-findings.md).
#[test]
fn the_dimmed_rows_of_the_edit_menu_are_the_known_ones() {
    let rows = dimmed_edit_rows();
    eprintln!("dimmed Edit rows: {rows:?}");
    let known = [
        "Replace Fonts\u{2026}",
        "Reset to Defaults\u{2026}",
        "AutoFill>",
        "Start Dictation",
        "Emoji & Symbols",
    ];
    let new: Vec<&String> = rows
        .iter()
        .filter(|r| !known.contains(&r.as_str()))
        .collect();
    assert!(new.is_empty(), "new dimmed rows in the Edit menu: {new:?}");
}

#[test]
fn zz_debug_one() {
    let Ok(name) = std::env::var("S35_DEBUG") else { return };
    let id = all_tool_ids().into_iter().find(|i| format!("{i:?}") == name).unwrap();
    let Creates(fx, g) = role(id) else { panic!() };
    let mut sim = fixture(fx);
    sim.tool(id);
    perform(&mut sim, g);
    let made: Vec<ObjectRef> = objects(&sim);
    eprintln!("objects {made:?}");
    let o = main_object(&made);
    eprintln!("main {o:?}");
    let offered = open_object(&mut sim, o);
    eprintln!("offered {offered}, tool {:?} dialog? {} spec_open {}", sim.app.tools.active_id(), sim.app.dialog.is_some(), sim.app.spec.is_open());
    for n in 0..3 {
        let t = dialog_texts(&mut sim, Vec::new());
        if n == 2 { for x in &t { eprintln!("  {:?}", x); } }
    }
    for _ in 0..3 { dialog_texts(&mut sim, vec![key_event(Key::Tab)]); }
    let t = dialog_texts(&mut sim, vec![egui::Event::Text("7".into())]);
    eprintln!("after typing:");
    for x in &t { eprintln!("  {:?}", x); }
    let t = dialog_texts(&mut sim, vec![key_event(Key::Enter)]);
    eprintln!("after enter: spec_open {} n={}", sim.app.spec.is_open(), t.len());
}
