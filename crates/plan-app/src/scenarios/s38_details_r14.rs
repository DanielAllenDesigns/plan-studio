//! Scenario 38 (round 14): pictures, underlays and CAD details. Import
//! Picture and its Image Specification, Point to Point Resize and Rotate to
//! Align for pictures and underlays, scanned PDFs as underlays, Auto Detail
//! from a section with insulation, CAD Detail Management, Detail Components
//! and Send to Layout (documentation-layout L-39..L-41, L-46).

use super::{draw_shell, Sim};
use crate::dialogs::{details as dlg, images as img_dlg, underlay as win};
use crate::editor::{framing_view, ObjectRef};
use crate::shell::docks::{browser_nodes, BrowserItem, BrowserNode};
use crate::toolbar::Action;
use crate::tools::details::{self as td, AutoDetailOptions, DetailsVariant};
use crate::tools::images::{self, ImageMode};
use crate::tools::underlay::{self, pdf, trace};
use crate::tools::ToolId;
use plan_core::details::{
    DETAIL_CUT_LAYER, DETAIL_FRAMING_LAYER, DETAIL_INSULATION_LAYER, DETAIL_NOTES_LAYER,
};
use plan_core::geometry::Point;
use plan_core::{CameraKind, CameraObject, Id};
use std::path::PathBuf;

const W: f64 = 480.0;
const H: f64 = 360.0;

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("plan-studio-s38-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A PNG written by the renderer's encoder: `w` x `h`, left half red, right
/// half white.
fn png_file(name: &str, w: u32, h: u32) -> String {
    let mut rgba = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(if x < w / 2 {
                &[200, 20, 20, 255]
            } else {
                &[255, 255, 255, 255]
            });
        }
    }
    let image = plan_render::Image {
        width: w,
        height: h,
        rgba,
        hdr: vec![[0.0; 3]; (w * h) as usize],
    };
    let path = scratch().join(name);
    plan_render::write_png(&path, &image).unwrap();
    path.to_string_lossy().into_owned()
}

fn pictures(sim: &Sim) -> Vec<plan_core::PlacedSymbol> {
    sim.app
        .cx
        .floor()
        .symbols
        .iter()
        .filter(|s| s.image.is_some())
        .cloned()
        .collect()
}

#[test]
fn import_picture_places_a_picture_opens_its_specification_and_undoes() {
    let mut sim = Sim::new();
    let path = png_file("survey.png", 40, 20);
    images::inject_pick(Some(&path));
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    let pics = pictures(&sim);
    assert_eq!(pics.len(), 1);
    let spec = pics[0].image.as_ref().unwrap();
    assert_eq!((spec.px_w, spec.px_h), (40, 20));
    // Longest side 36", the picture's proportions, lying flat.
    assert!((pics[0].width - 36.0).abs() < 1e-9 && (pics[0].depth - 18.0).abs() < 1e-9);
    assert!(!spec.billboard);
    // The Image Specification is open on it; Cancel closes it unchanged.
    assert!(sim.app.has_dialog(), "the specification opened");
    sim.cancel();
    assert!(!sim.app.has_dialog());
    assert_eq!(pictures(&sim)[0].width, pics[0].width);
    assert!(matches!(
        sim.app.cx.selection.items.first(),
        Some(ObjectRef::Symbol(id)) if *id == pics[0].id
    ));
    // One undo step takes it away; a cancelled box places nothing.
    let n = sim.app.cx.floor().symbols.len();
    assert_eq!(sim.undo().as_deref(), Some("Import Picture"));
    assert_eq!(sim.app.cx.floor().symbols.len(), n - 1);
    images::inject_pick(None);
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    assert!(pictures(&sim).is_empty());
    assert!(sim.app.cx.status.contains("No picture"));
    // A file that is not a picture says so.
    let junk = scratch().join("junk.png");
    std::fs::write(&junk, b"not a picture").unwrap();
    images::inject_pick(Some(&junk.to_string_lossy()));
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    assert!(pictures(&sim).is_empty());
}

#[test]
fn a_jpeg_picture_decodes_for_the_plan_and_the_transparent_colour_applies() {
    let mut sim = Sim::new();
    let jpg = scratch().join("photo.jpg");
    std::fs::write(
        &jpg,
        include_bytes!("../tools/underlay/testdata/rgb420.jpg"),
    )
    .unwrap();
    let spec = images::load_spec(&jpg.to_string_lossy()).unwrap();
    assert_eq!((spec.px_w, spec.px_h), (37, 29));
    assert!(spec.note.is_empty(), "decoded, not a frame: {}", spec.note);
    assert_ne!(spec.color, [180, 180, 180], "the average colour was read");
    // The corner colour is there for the transparent colour.
    assert!(images::corner_color(&jpg.to_string_lossy()).is_some());
    // It can be placed like any picture.
    images::inject_pick(Some(&jpg.to_string_lossy()));
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    assert_eq!(pictures(&sim).len(), 1);
}

/// Where the picture's local point (u across from its back centre, v out from
/// its back edge) lies in the plan.
fn on_picture(s: &plan_core::PlacedSymbol, u: f64, v: f64) -> Point {
    let a = s.angle.to_radians();
    let dir = Point::new(a.cos(), a.sin());
    s.position + dir * u + dir.perp() * v
}

/// The point the pointer reports for a click at `p` (snapped to the grid).
fn snapped(sim: &mut Sim, p: Point) -> Point {
    crate::tools::PointerEvent::at(sim.cx(), p).snapped
}

/// `p` in the picture's own frame: across from its back centre, out from
/// its back edge.
fn local(s: &plan_core::PlacedSymbol, p: Point) -> (f64, f64) {
    let a = s.angle.to_radians();
    let dir = Point::new(a.cos(), a.sin());
    let d = p - s.position;
    (d.dot(dir), d.dot(dir.perp()))
}

#[test]
fn point_to_point_resize_scales_a_picture_about_the_first_point_from_a_typed_distance() {
    let mut sim = Sim::new();
    images::inject_pick(Some(&png_file("scale.png", 40, 20)));
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    sim.ok();
    let before = pictures(&sim)[0].clone();
    // Two points on the picture, about 30" apart on the drawing.
    let a = snapped(&mut sim, on_picture(&before, -12.0, 4.0));
    let b = snapped(&mut sim, on_picture(&before, 18.0, 4.0));
    let drawn = a.dist(b);
    assert!(drawn > 25.0);
    // The picture is selected, so the tool starts with it; the clicks are
    // the two points.
    sim.app.cx.selection.set(ObjectRef::Symbol(before.id));
    sim.tool(ToolId::ImagesVariant(ImageMode::PointToPointResize));
    sim.click(a.x, a.y);
    sim.click(b.x, b.y);
    assert!(img_dlg::resize_prompt_open(), "the distance is asked for");
    // An unreadable distance keeps the prompt open and changes nothing.
    assert!(!img_dlg::submit_resize_prompt(sim.cx(), "abc"));
    assert!(img_dlg::resize_prompt_open());
    assert_eq!(pictures(&sim)[0].width, before.width);
    // The real distance is 15'-0".
    assert!(img_dlg::submit_resize_prompt(sim.cx(), "15'-0\""));
    assert!(!img_dlg::resize_prompt_open());
    let after = pictures(&sim)[0].clone();
    let k = 180.0 / drawn;
    assert!((after.width / before.width - k).abs() < 1e-9);
    assert!(
        (after.depth / before.depth - k).abs() < 1e-9,
        "proportions kept"
    );
    // The first point stays where it is on the picture.
    let (u, v) = local(&before, a);
    assert!(on_picture(&after, u * k, v * k).dist(a) < 1e-6);
    // The second point is now exactly 15' from it.
    let (ub, vb) = local(&before, b);
    assert!((on_picture(&after, ub * k, vb * k).dist(a) - 180.0).abs() < 1e-6);
    assert_eq!(sim.undo().as_deref(), Some("Point to Point Resize"));
    assert_eq!(pictures(&sim)[0].width, before.width);
}

#[test]
fn rotate_to_align_turns_a_tilted_picture_level() {
    let mut sim = Sim::new();
    images::inject_pick(Some(&png_file("tilt.png", 40, 20)));
    sim.action(Action::Custom(images::IMPORT_PICTURE));
    sim.ok();
    // Scanned crooked by 4 degrees.
    let id = pictures(&sim)[0].id;
    let fl = sim.app.cx.floor;
    sim.app.cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == id)
        .unwrap()
        .angle = 4.0;
    let tilted = pictures(&sim)[0].clone();
    // Click along the picture's top edge, which should be level.
    let a = snapped(&mut sim, on_picture(&tilted, -15.0, 16.0));
    let b = snapped(&mut sim, on_picture(&tilted, 15.0, 16.0));
    let turn = trace::level_angle(a, b);
    assert!(turn < -0.5 && turn > -8.0, "{turn}");
    sim.app.cx.selection.set(ObjectRef::Symbol(id));
    sim.tool(ToolId::ImagesVariant(ImageMode::RotateToAlign));
    sim.click(a.x, a.y);
    let r = sim.click(b.x, b.y);
    assert_eq!(r.commit.as_deref(), Some("Rotate to Align"));
    let after = pictures(&sim)[0].clone();
    assert!(
        (after.angle - (4.0 + turn)).abs() < 1e-9,
        "{} vs {turn}",
        after.angle
    );
    // The first point did not move on the picture.
    let (u, v) = local(&tilted, a);
    assert!(on_picture(&after, u, v).dist(a) < 1e-6);
    assert_eq!(sim.undo().as_deref(), Some("Rotate to Align"));
    assert_eq!(pictures(&sim)[0].angle, 4.0);
    // Esc forgets a half-finished trace: the next click starts over.
    sim.tool(ToolId::ImagesVariant(ImageMode::RotateToAlign));
    sim.click(a.x, a.y);
    sim.esc();
    sim.app.cx.selection.set(ObjectRef::Symbol(id));
    sim.tool(ToolId::ImagesVariant(ImageMode::RotateToAlign));
    let r = sim.click(b.x, b.y);
    assert!(r.commit.is_none(), "one click is only the first point");
}

#[test]
fn a_scanned_pdf_becomes_a_calibrated_underlay_and_a_vector_pdf_says_to_export_a_png() {
    let mut sim = Sim::new();
    let (bytes, _) = pdf::tests::paged_pdf();
    let path = scratch().join("survey-scan.pdf");
    std::fs::write(&path, &bytes).unwrap();
    // Two pages: a Flate/PNG-predictor picture and a JPEG.
    assert_eq!(underlay::pdf_page_count(&path), 2);
    let first = underlay::import_pdf_page(sim.cx(), &path, 1).unwrap();
    let second = underlay::import_pdf_page(sim.cx(), &path, 2).unwrap();
    let (a, b) = {
        let f = sim.app.cx.floor();
        (
            f.underlay(first).unwrap().clone(),
            f.underlay(second).unwrap().clone(),
        )
    };
    assert!(a.path.ends_with(".png"), "{}", a.path);
    assert!(b.path.ends_with(".jpg"), "{}", b.path);
    assert_eq!((a.pixel_width, a.pixel_height), (6, 4));
    assert_eq!((b.pixel_width, b.pixel_height), (40, 24));
    assert!(underlay::load_picture(&a.path).is_ok(), "the PNG decodes");
    assert!(underlay::load_picture(&b.path).is_ok(), "the JPEG decodes");
    // Point to Point Resize: two points on the picture are really 20'-0".
    let pa = a.pixel_to_plan(0.0, 0.0);
    let pb = a.pixel_to_plan(6.0, 0.0);
    assert!(underlay::apply_calibration(sim.cx(), first, pa, pb, 240.0));
    let scaled = sim.app.cx.floor().underlay(first).unwrap().clone();
    assert!(
        (scaled.size().0 - 240.0).abs() < 1e-6,
        "{:?}",
        scaled.size()
    );
    // Rotate to Align through the tool: the picture is turned 2 degrees.
    let id = first;
    {
        let fl = sim.app.cx.floor;
        sim.app.cx.project.floors[fl]
            .underlay_mut(id)
            .unwrap()
            .rotation = 2f64.to_radians();
    }
    let tilted = sim.app.cx.floor().underlay(id).unwrap().clone();
    let (p, q) = (
        tilted.pixel_to_plan(0.0, 4.0),
        tilted.pixel_to_plan(6.0, 4.0),
    );
    win::begin_alignment(id);
    sim.tool(ToolId::Underlay);
    sim.click(p.x, p.y);
    let r = sim.click(q.x, q.y);
    assert_eq!(r.commit.as_deref(), Some("Rotate Underlay to Align"));
    let level = sim.app.cx.floor().underlay(id).unwrap().clone();
    assert!(level.rotation.abs() < 0.02, "{}", level.rotation);
    assert!(win::calibration().is_none(), "the trace is over");
    assert_eq!(sim.undo().as_deref(), Some("Rotate Underlay to Align"));
    // A PDF of vector drawing and text has no picture.
    let vector = scratch().join("vector.pdf");
    std::fs::write(
        &vector,
        b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n%%EOF\n",
    )
    .unwrap();
    let err = underlay::import_pdf_page(sim.cx(), &vector, 1).unwrap_err();
    assert!(err.contains("export the page as a PNG"), "{err}");
    let n = sim.app.cx.floor().underlays.len();
    assert!(underlay::import_image(sim.cx(), &vector).is_err());
    assert_eq!(sim.app.cx.floor().underlays.len(), n);
    // Trace helpers on their own.
    assert!(trace::level_angle(Point::ZERO, Point::new(100.0, 5.0)) < 0.0);
}

/// A house of the real wall tool plus a section crossing its south wall.
fn house_with_section(sim: &mut Sim) -> Id {
    draw_shell(sim, W, H);
    let mut c = CameraObject::new(
        CameraKind::CrossSection { back_clip: None },
        Point::new(W / 2.0, 0.0),
        0.0,
        "Wall Section",
        0,
    );
    crate::tools::camera::set_section_geometry(&mut c, Point::new(W / 2.0, 0.0), 0.0, 120.0, None);
    let id = sim.app.cx.project.add_camera(c);
    sim.app.cx.refresh();
    id
}

fn layer_count(sim: &Sim, floor: usize, layer: &str) -> usize {
    sim.app.cx.project.floors[floor]
        .cad
        .iter()
        .filter(|o| o.layer == layer)
        .count()
}

#[test]
fn auto_detail_makes_a_detail_from_a_section_with_insulation_and_framing() {
    let mut h = House::new();
    let sim = &mut h.sim;
    // Built framing: the plates are cut by the plane; the cavity is insulated.
    framing_view::build(sim.cx(), false);
    // Move the cut to a place between two studs (a stud in the way is drawn
    // instead of a batt).
    let cam = h.cam;
    let mut x = W / 2.0 - 8.0;
    loop {
        crate::tools::camera::set_section_geometry(
            sim.app
                .cx
                .project
                .cameras
                .iter_mut()
                .find(|c| c.id == cam)
                .unwrap(),
            Point::new(x, 0.0),
            0.0,
            120.0,
            None,
        );
        let c = sim.app.cx.project.camera(cam).unwrap().clone();
        let b = td::build_auto_detail(
            &sim.app.cx.project,
            &sim.app.cx.defaults,
            &c,
            &AutoDetailOptions::default(),
        );
        if b.batts == 1 {
            break;
        }
        x += 1.0;
        assert!(x < W / 2.0 + 24.0, "a cut between studs exists");
    }
    // Auto Detail on the selected section (the toolbar button).
    sim.app.cx.selection.items = vec![ObjectRef::Camera(cam)];
    let floors = sim.app.cx.project.floors.len();
    sim.action(Action::Custom(td::AUTO_DETAIL));
    assert_eq!(sim.app.cx.project.floors.len(), floors + 1);
    let d = floors;
    let f = &sim.app.cx.project.floors[d];
    assert!(f.is_cad_detail());
    assert_eq!(
        f.detail.as_ref().unwrap().source,
        plan_core::details::DetailSource::Camera { camera: cam }
    );
    // Lines on the right layers: the cut, the notes, the insulation batt and
    // the framing members from plan-framing (the plates the plane cuts).
    assert!(layer_count(sim, d, DETAIL_CUT_LAYER) > 0);
    assert!(layer_count(sim, d, DETAIL_NOTES_LAYER) > 0);
    assert!(
        layer_count(sim, d, DETAIL_INSULATION_LAYER) >= 2,
        "box and wave"
    );
    assert!(layer_count(sim, d, DETAIL_FRAMING_LAYER) >= 9);
    // The detail is named for the view.
    let n = sim.app.cx.project.callout_number(cam).unwrap();
    assert_eq!(
        sim.app.cx.project.floors[d].name,
        format!("{n} - Wall Section")
    );
    // The Project Browser lists it under CAD Details, not under Floors.
    let nodes = browser_nodes(sim.cx());
    let details = &nodes
        .iter()
        .find(|(n, _)| *n == BrowserNode::CadDetails)
        .unwrap()
        .1;
    assert!(details
        .iter()
        .any(|e| e.item == BrowserItem::DetailFloor(d)));
    let floors_rows = &nodes
        .iter()
        .find(|(n, _)| *n == BrowserNode::Floors)
        .unwrap()
        .1;
    assert!(floors_rows
        .iter()
        .all(|e| e.item != BrowserItem::DetailFloor(d)));
    // One undo step removes it.
    assert_eq!(sim.undo().as_deref(), Some("Auto Detail"));
    assert_eq!(sim.app.cx.project.floors.len(), floors);
}

/// The house and its section, with the camera's id.
struct House {
    sim: Sim,
    cam: Id,
}

impl House {
    fn new() -> House {
        let mut sim = Sim::new();
        let cam = house_with_section(&mut sim);
        House { sim, cam }
    }
}

#[test]
fn an_unframed_section_still_gets_plates_and_a_batt_and_follows_a_callout_rename() {
    let mut h = House::new();
    let (cam, sim) = (h.cam, &mut h.sim);
    let i = td::auto_detail(sim.cx(), cam, &AutoDetailOptions::default()).unwrap();
    assert!(
        layer_count(sim, i, DETAIL_INSULATION_LAYER) >= 2,
        "the batt"
    );
    assert!(
        layer_count(sim, i, DETAIL_FRAMING_LAYER) >= 9,
        "plates, cut"
    );
    // The camera is renamed: the next frame of the windows renames the
    // detail (it is the camera's section until the user types a name).
    sim.app
        .cx
        .project
        .cameras
        .iter_mut()
        .find(|c| c.id == cam)
        .unwrap()
        .name = "Eave Section".into();
    let ctx = eframe::egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        crate::dialogs::underlay::show_all(ctx, &mut sim.app.cx);
    });
    let n = sim.app.cx.project.callout_number(cam).unwrap();
    assert_eq!(
        sim.app.cx.project.floors[i].name,
        format!("{n} - Eave Section")
    );
}

#[test]
fn cad_detail_management_lists_opens_renames_duplicates_and_deletes() {
    let mut h = House::new();
    let (cam, sim) = (h.cam, &mut h.sim);
    let i = td::auto_detail(sim.cx(), cam, &AutoDetailOptions::default()).unwrap();
    // The window opens from the menu command and draws.
    sim.action(Action::Custom(td::MANAGEMENT));
    assert!(dlg::management_open());
    dlg::select_detail(Some(&sim.app.cx.project.floors[i].name.clone()));
    let ctx = eframe::egui::Context::default();
    for _ in 0..2 {
        let _ = ctx.run(Default::default(), |ctx| {
            crate::dialogs::underlay::show_all(ctx, &mut sim.app.cx);
        });
    }
    // Open: the detail's tab.
    assert!(td::open_detail(sim.cx(), i));
    assert_eq!(sim.app.cx.floor, i);
    // Rename, duplicate, delete.
    assert!(td::rename_detail(sim.cx(), i, "Typical Exterior Wall"));
    let j = td::duplicate_detail(sim.cx(), i).unwrap();
    assert_eq!(
        sim.app.cx.project.floors[j].cad.len(),
        sim.app.cx.project.floors[i].cad.len()
    );
    assert!(td::delete_detail(sim.cx(), i));
    assert_eq!(sim.app.cx.project.cad_detail_floors().len(), 1);
    assert_eq!(sim.app.cx.floor, 0, "the editor left the deleted detail");
    // Send to Layout, with a layout started.
    let d = sim.app.cx.project.cad_detail_floors()[0];
    let mut layout = plan_layout::Layout::new("L", plan_docs::SheetSize::ArchC);
    layout.add_page(4, "Details");
    crate::shell::layout_window::store(&mut sim.app.cx.project, &layout);
    let page = td::send_to_layout(sim.cx(), d).unwrap();
    let layout = crate::shell::layout_window::load(&sim.app.cx.project).unwrap();
    assert!(layout
        .page(page)
        .unwrap()
        .boxes
        .iter()
        .any(|b| matches!(&b.source, plan_layout::BoxSource::CadDetail { .. })));
}

#[test]
fn detail_components_place_as_named_cad_blocks_by_click() {
    let mut sim = Sim::new();
    let d = td::new_detail(sim.cx());
    assert!(td::open_detail(sim.cx(), d));
    // The chooser arms a component and asks for the placement tool.
    td::arm(Some("frame.2x6"));
    sim.tool(ToolId::DetailsVariant(DetailsVariant::Component));
    assert!(sim.app.tools.active().hint().contains("2x6 Section"));
    let r = sim.click(100.0, 100.0);
    assert_eq!(r.commit.as_deref(), Some("Place Detail Component"));
    let f = &sim.app.cx.project.floors[d];
    assert_eq!(f.cad.len(), 3, "the section and its X");
    assert_eq!(f.cad_blocks.len(), 1);
    assert_eq!(f.cad_blocks[0].name, "2x6 Section");
    assert!(f.cad.iter().all(|o| o.layer == DETAIL_FRAMING_LAYER));
    // Another click places another; undo takes the last away.
    sim.click(160.0, 100.0);
    assert_eq!(sim.app.cx.project.floors[d].cad_blocks.len(), 2);
    assert_eq!(sim.undo().as_deref(), Some("Place Detail Component"));
    assert_eq!(sim.app.cx.project.floors[d].cad_blocks.len(), 1);
    // Nothing armed: the click only explains.
    td::arm(None);
    sim.click(200.0, 100.0);
    assert!(
        sim.app.cx.status.contains("Detail Components"),
        "{}",
        sim.app.cx.status
    );
    assert_eq!(sim.app.cx.project.floors[d].cad_blocks.len(), 1);
    // The window command opens the chooser.
    sim.action(Action::Custom(td::COMPONENTS));
    assert!(dlg::components_open());
}
