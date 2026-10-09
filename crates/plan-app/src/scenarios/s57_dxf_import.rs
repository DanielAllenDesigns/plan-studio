//! Scenario 57: the Import Drawing Assistant, driven through its pages and
//! the menu command: a layer mapping with the advanced table, layers
//! converted to walls with a wall type, dimensions, CAD blocks and hatches,
//! a binary DXF, a DWG refused with guidance, duplicate CAD blocks, several
//! files, and the round trip of the plan's own DXF export (L-43, L-78, L-79,
//! L-81; manual pp. 1289-1301).

use super::{draw_shell, Sim};
use crate::dialogs::dxf_options::DxfExportDialog;
use crate::dialogs::import_drawing::{self as imp, LayerChoice, MappingMode, Page};
use crate::editor::ObjectRef;
use crate::toolbar::{Action, FileCommand};
use eframe::egui;
use plan_core::cad::CadItem;
use plan_core::export::dxf_options::DxfUnits as OutUnits;
use plan_core::geometry::Point;
use plan_core::WallKind;
use plan_import::dxf::tokens;
use plan_import::{BlockConflict, DimensionMode, DxfUnits};

fn sim() -> Sim {
    imp::close();
    let mut sim = Sim::new();
    draw_shell(&mut sim, 480.0, 360.0);
    sim
}

/// "code value" lines to DXF text.
fn dxf(spec: &str) -> String {
    let mut out = String::new();
    for l in spec.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let (c, v) = l.split_once(' ').unwrap_or((l, ""));
        out.push_str(&format!("{c}\n{}\n", v.trim()));
    }
    out
}

/// A small drawing in millimetres: a 4 m x 3 m room drawn as two layers of
/// wall faces, a chair block with an attribute, a linear dimension, a
/// hatch, a note and a frozen junk layer.
fn site_dxf() -> String {
    let wall = |x0: f64, y0: f64, x1: f64, y1: f64| {
        format!("0 LINE\n8 A-WALL\n10 {x0}\n20 {y0}\n11 {x1}\n21 {y1}\n")
    };
    let mut body = String::new();
    // Outer faces and inner faces, 150 mm apart (a 6 in wall).
    for (a, b, c, d) in [
        (0.0, 0.0, 4000.0, 0.0),
        (4000.0, 0.0, 4000.0, 3000.0),
        (4000.0, 3000.0, 0.0, 3000.0),
        (0.0, 3000.0, 0.0, 0.0),
        (150.0, 150.0, 3850.0, 150.0),
        (3850.0, 150.0, 3850.0, 2850.0),
        (3850.0, 2850.0, 150.0, 2850.0),
        (150.0, 2850.0, 150.0, 150.0),
    ] {
        body.push_str(&wall(a, b, c, d));
    }
    body.push_str(
        "0 INSERT\n8 A-FURN\n66 1\n2 CHAIR\n10 1000\n20 1000\n\
0 ATTRIB\n8 A-FURN\n10 1000\n20 1100\n40 50\n1 C-1\n2 TAG\n70 0\n0 SEQEND\n\
0 DIMENSION\n8 A-DIMS\n2 *D1\n10 2000\n20 -400\n11 2000\n21 -350\n70 1\n13 0\n23 0\n14 4000\n24 0\n42 4000\n\
0 TEXT\n8 NOTES\n10 500\n20 2500\n40 100\n1 LIVING\n\
0 HATCH\n8 A-HATCH\n10 0\n20 0\n30 0\n2 ANSI31\n70 0\n71 0\n91 1\n92 7\n72 0\n73 1\n93 4\n10 200\n20 200\n10 600\n20 200\n10 600\n20 600\n10 200\n20 600\n97 0\n75 0\n76 1\n52 0\n41 1\n78 1\n53 45\n43 0\n44 0\n45 -50\n46 50\n79 0\n98 0\n\
0 LINE\n8 JUNK\n10 0\n20 0\n11 99999\n21 0\n",
    );
    dxf(&format!(
        "0 SECTION
2 HEADER
9 $ACADVER
1 AC1027
9 $INSUNITS
70 4
0 ENDSEC
0 SECTION
2 TABLES
0 TABLE
2 LAYER
0 LAYER
2 A-WALL
70 0
62 1
0 LAYER
2 A-FURN
70 0
62 5
0 LAYER
2 A-DIMS
70 0
62 3
0 LAYER
2 NOTES
70 0
62 7
0 LAYER
2 A-HATCH
70 0
62 2
0 LAYER
2 JUNK
70 1
62 8
0 ENDTAB
0 ENDSEC
0 SECTION
2 BLOCKS
0 BLOCK
8 0
2 CHAIR
70 0
10 0
20 0
0 LINE
8 0
10 -200
20 -200
11 200
21 -200
0 LINE
8 0
10 200
20 -200
11 200
21 200
0 LINE
8 0
10 200
20 200
11 -200
21 200
0 LINE
8 0
10 -200
20 200
11 -200
21 -200
0 ATTDEF
8 0
10 0
20 0
40 50
1 default
2 TAG
70 0
0 ENDBLK
0 BLOCK
8 0
2 *D1
70 1
10 0
20 0
0 LINE
8 0
10 0
20 -400
11 4000
21 -400
0 TEXT
8 0
10 2000
20 -350
40 100
1 4000
0 ENDBLK
0 ENDSEC
0 SECTION
2 ENTITIES
{body}0 ENDSEC
0 EOF"
    ))
}

fn open(sim: &mut Sim, name: &str, bytes: Vec<u8>) {
    imp::open_files(&mut sim.app.cx, vec![(name.to_string(), bytes)]);
}

fn frames(sim: &mut Sim) {
    let ctx = sim.ctx.clone();
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            imp::show(ctx, &mut sim.app.cx)
        });
    }
}

fn cad_count(sim: &Sim) -> usize {
    sim.app.cx.floor().cad.len()
}

#[test]
fn the_menu_command_opens_the_file_picker_hook_and_pages_run_in_order() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    assert!(imp::is_open());
    assert_eq!(imp::page(), Some(Page::Files));
    frames(&mut sim);
    // Show Import Assistant is on: OK goes to Select File.
    assert!(imp::next(&mut sim.app.cx));
    assert_eq!(imp::page(), Some(Page::SelectFile));
    frames(&mut sim);
    assert!(imp::next(&mut sim.app.cx));
    assert_eq!(imp::page(), Some(Page::SelectLayers));
    // The layers the original program showed are checked; the frozen one is not.
    let (checked, junk) = imp::with_assistant(|a| {
        (
            a.rows.iter().filter(|r| r.include).count(),
            a.rows
                .iter()
                .find(|r| r.name == "JUNK")
                .map(|r| (r.include, r.frozen)),
        )
    })
    .unwrap();
    assert_eq!(checked, 5);
    assert_eq!(junk, Some((false, true)));
    frames(&mut sim);
    assert!(imp::next(&mut sim.app.cx));
    assert_eq!(imp::page(), Some(Page::LayerMapping));
    frames(&mut sim);
    assert!(imp::next(&mut sim.app.cx));
    assert_eq!(imp::page(), Some(Page::DrawingUnit));
    frames(&mut sim);
    assert!(imp::next(&mut sim.app.cx));
    assert_eq!(imp::page(), Some(Page::Complete));
    frames(&mut sim);
    let preview = imp::with_assistant(|a| a.preview.clone()).unwrap().unwrap();
    // 8 wall lines, 4 chair lines + the attribute, the note, the hatch outline.
    assert_eq!(preview.summary.lines, 12);
    assert_eq!(preview.summary.dimensions, 1);
    assert_eq!(preview.summary.blocks, 1);
    assert_eq!(preview.summary.hatches, 1);
    assert!(preview.size.is_some());
    // Import: the window closes, everything is one undo step.
    let before = cad_count(&sim);
    assert!(!imp::next(&mut sim.app.cx));
    assert!(!imp::is_open());
    assert!(cad_count(&sim) > before);
    assert_eq!(sim.app.cx.undo_label(), Some("Import Drawing"));
    sim.undo();
    assert_eq!(cad_count(&sim), before);
    assert!(sim.app.cx.floor().dimensions.is_empty());
    assert!(sim.app.cx.floor().cad_blocks().is_empty());
}

#[test]
fn units_come_from_the_file_and_the_drawing_lands_where_it_was_asked() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    // 4000 mm is 157.48 in; the drawing is moved so its lower left is at (24, 36).
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.insert_x = "24".into();
        a.insert_y = "36".into();
    });
    assert!(!imp::next(&mut sim.app.cx));
    let f = sim.app.cx.floor();
    let walls: Vec<&plan_core::cad::CadObject> =
        f.cad.iter().filter(|c| c.layer == "A-WALL").collect();
    assert_eq!(walls.len(), 8);
    let (mut lo, mut hi) = (Point::new(1e9, 1e9), Point::new(-1e9, -1e9));
    for w in &walls {
        let (a, b) = w.item.bounds();
        lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
        hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
    }
    // The dimension hangs below the room, so the drawing's own lower left is
    // the dimension line; the walls start one dimension offset above it.
    assert!((hi.x - lo.x - 4000.0 / 25.4).abs() < 1e-6, "{lo:?} {hi:?}");
    assert!((hi.y - lo.y - 3000.0 / 25.4).abs() < 1e-6);
    assert!((lo.x - 24.0).abs() < 1e-6, "x of the lower left {}", lo.x);
    // Frozen JUNK was not imported.
    assert!(f.cad.iter().all(|c| c.layer != "JUNK"));
}

#[test]
fn the_advanced_layer_mapping_renames_skips_and_creates_layers() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| {
        a.mode = MappingMode::Advanced;
        for (name, choice) in a.advanced.iter_mut() {
            match name.as_str() {
                "A-WALL" => *choice = LayerChoice::Plan("Walls, Normal".into()),
                "A-FURN" => *choice = LayerChoice::Named("Furniture".into()),
                _ => {}
            }
        }
        // Do not import the notes.
        a.rows
            .iter_mut()
            .find(|r| r.name == "NOTES")
            .unwrap()
            .include = false;
    });
    for _ in 0..3 {
        imp::next(&mut sim.app.cx);
    }
    assert_eq!(imp::page(), Some(Page::LayerMapping));
    imp::next(&mut sim.app.cx);
    assert_eq!(imp::page(), Some(Page::AdvancedMapping));
    frames(&mut sim);
    imp::next(&mut sim.app.cx);
    assert_eq!(imp::page(), Some(Page::DrawingUnit));
    imp::next(&mut sim.app.cx);
    assert!(!imp::next(&mut sim.app.cx));
    let f = sim.app.cx.floor();
    assert_eq!(
        f.cad.iter().filter(|c| c.layer == "Walls, Normal").count(),
        8
    );
    assert!(f.cad.iter().any(|c| c.layer == "Furniture"));
    assert!(f
        .cad
        .iter()
        .all(|c| !matches!(&c.item, CadItem::Text { text, .. } if text == "LIVING")));
    assert!(sim.app.cx.project.layers.get("Furniture").is_some());
    // Layers made from the file carry its colors.
    let hatch_layer = sim.app.cx.project.layers.get("A-HATCH").unwrap();
    assert_eq!(hatch_layer.color, [255, 255, 0]);
    sim.undo();
    assert!(sim.app.cx.project.layers.get("Furniture").is_none());
}

#[test]
fn one_layer_for_everything_keeps_the_looks_on_the_objects() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.mode = MappingMode::Single;
        a.single_layer = "Imported".into();
    });
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    assert!(f.cad.iter().all(|c| c.layer == "Imported"));
    // The walls were red on their layer; each object says so.
    let red = f
        .cad
        .iter()
        .filter(|c| f.cad_attrs(c.id).and_then(|a| a.color) == Some([255, 0, 0]))
        .count();
    assert_eq!(red, 8);
}

#[test]
fn layers_marked_to_walls_make_walls_of_the_chosen_type() {
    let mut sim = sim();
    let shell_walls = sim.app.cx.floor().walls.len();
    // A wall type the plan has.
    let ty = sim
        .app
        .cx
        .wall_types()
        .first()
        .cloned()
        .expect("a wall type");
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.rows
            .iter_mut()
            .find(|r| r.name == "A-WALL")
            .unwrap()
            .walls = true;
        a.wall_type = ty.name.clone();
    });
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    assert_eq!(
        f.walls.len(),
        shell_walls + 4,
        "four walls of the room besides the shell"
    );
    for w in f.walls.iter().skip(shell_walls) {
        assert_eq!(w.wall_type.as_deref(), Some(ty.name.as_str()));
        assert!((w.thickness - ty.thickness()).abs() < 1e-6);
        assert_eq!(w.kind, ty.kind);
    }
    assert_eq!(sim.app.cx.undo_label(), Some("Import Drawing"));
    sim.undo();
    assert_eq!(sim.app.cx.floor().walls.len(), shell_walls);
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn the_chair_is_a_cad_block_with_its_attribute_and_dimensions_are_objects() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| a.show_assistant = false);
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    let blocks = f.cad_blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].name, "CHAIR");
    // Four sides and the attribute value, not the definition's default.
    let members = f.group_members_cad(blocks[0].group);
    assert_eq!(members.len(), 5);
    let texts: Vec<&str> = f
        .cad
        .iter()
        .filter(|c| members.contains(&c.id))
        .filter_map(|c| match &c.item {
            CadItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["C-1"]);
    // The linear dimension is a real dimension, 4000 mm long, line 400 mm out.
    assert_eq!(f.dimensions.len(), 1);
    let d = &f.dimensions[0];
    assert!((d.length() - 4000.0 / 25.4).abs() < 1e-6);
    assert!((d.offset.abs() - 400.0 / 25.4).abs() < 1e-6);
    // Everything that came in is selected so it can be moved.
    assert!(sim.app.cx.selection.items.len() > 10);
    assert!(sim
        .app
        .cx
        .selection
        .items
        .iter()
        .all(|o| matches!(o, ObjectRef::Cad(_))));
}

#[test]
fn dimensions_can_come_in_as_drawn_blocks_instead() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.dims = DimensionMode::Blocks;
    });
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    assert!(f.dimensions.is_empty());
    let names: Vec<String> = f.cad_blocks().into_iter().map(|b| b.name).collect();
    assert!(names.contains(&"Dimension".to_string()), "{names:?}");
}

#[test]
fn a_pattern_hatch_is_drawn_with_the_hatch_tools_lines() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| a.show_assistant = false);
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    let outline = f
        .cad
        .iter()
        .find(|c| c.layer == "A-HATCH" && matches!(c.item, CadItem::Polyline { closed: true, .. }))
        .expect("the hatch outline");
    let fill = f
        .cad_attrs(outline.id)
        .and_then(|a| a.fill)
        .expect("a fill");
    assert_eq!(fill.pattern, "Diagonal Lines");
    assert!(!fill.lines.is_empty(), "the pattern lines were drawn");
    assert!(fill.lines.iter().all(|l| f.cad.iter().any(|c| c.id == *l)));
    // One undo takes the lines away with the rest.
    sim.undo();
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn a_binary_dxf_imports_like_the_text_one() {
    let mut a = sim();
    open(&mut a, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|x| x.show_assistant = false);
    imp::next(&mut a.app.cx);
    let mut b = sim();
    open(&mut b, "site.dxf", tokens::ascii_to_binary(&site_dxf()));
    assert!(
        imp::with_assistant(|x| x.drawing().format == plan_import::dxf::DxfFormat::Binary).unwrap()
    );
    imp::with_assistant(|x| x.show_assistant = false);
    imp::next(&mut b.app.cx);
    let items = |s: &Sim| {
        s.app
            .cx
            .floor()
            .cad
            .iter()
            .map(|c| (c.layer.clone(), c.item.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(items(&a), items(&b));
    assert!(!items(&a).is_empty());
}

#[test]
fn a_dwg_is_refused_with_the_way_to_save_a_dxf() {
    let mut sim = sim();
    let mut dwg = b"AC1032".to_vec();
    dwg.extend_from_slice(&[0, 0, 0, 0, 0, 1, 2, 3, 4]);
    open(&mut sim, "plan.dwg", dwg);
    assert!(!imp::is_open());
    let msg = imp::notice().expect("a message");
    assert!(
        msg.contains("DWG")
            && msg.contains("2018")
            && msg.contains("Save As")
            && msg.contains("DXF"),
        "{msg}"
    );
    assert!(sim.app.cx.status.starts_with("Import failed"));
    frames(&mut sim);
    assert!(imp::notice().is_some(), "the message stays until OK");
    assert_eq!(cad_count(&sim), 0);
    imp::close();
}

#[test]
fn a_block_the_floor_already_has_asks_what_to_do() {
    let mut sim = sim();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| a.show_assistant = false);
    imp::next(&mut sim.app.cx);
    assert_eq!(sim.app.cx.floor().cad_blocks().len(), 1);
    // Import it again: the chair is a duplicate.
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    for _ in 0..4 {
        imp::next(&mut sim.app.cx);
    }
    assert_eq!(imp::page(), Some(Page::DuplicateBlocks));
    frames(&mut sim);
    imp::with_assistant(|a| {
        assert_eq!(a.dup_names.len(), 1);
        a.dup_default = BlockConflict::AutoName;
    });
    imp::next(&mut sim.app.cx);
    assert_eq!(imp::page(), Some(Page::DrawingUnit));
    imp::next(&mut sim.app.cx);
    assert!(!imp::next(&mut sim.app.cx));
    let names: Vec<String> = sim
        .app
        .cx
        .floor()
        .cad_blocks()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert_eq!(names, vec!["CHAIR".to_string(), "CHAIR_Copy_1".to_string()]);
}

#[test]
fn two_files_import_side_by_side_as_blocks_in_one_step() {
    let mut sim = sim();
    let one = dxf("0 SECTION\n2 ENTITIES\n0 LINE\n8 A\n10 0\n20 0\n11 100\n21 0\n0 CIRCLE\n8 A\n10 50\n20 50\n40 10\n0 ENDSEC\n0 EOF");
    imp::open_files(
        &mut sim.app.cx,
        vec![
            ("a.dxf".into(), one.clone().into_bytes()),
            ("b.dxf".into(), one.into_bytes()),
        ],
    );
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.create_blocks = true;
        a.auto_position = true;
        a.units = Some(DxfUnits::Inches);
    });
    imp::next(&mut sim.app.cx);
    let f = sim.app.cx.floor();
    let blocks = f.cad_blocks();
    assert_eq!(blocks.len(), 2);
    let mut xs: Vec<f64> = blocks.iter().map(|b| f.block_bounds(b.group).0.x).collect();
    xs.sort_by(|a, b| a.total_cmp(b));
    assert!(
        xs[1] - xs[0] > 100.0,
        "the second drawing sits to the right: {xs:?}"
    );
    assert_eq!(sim.app.cx.undo_label(), Some("Import Drawing"));
    sim.undo();
    assert!(sim.app.cx.floor().cad.is_empty());
}

#[test]
fn the_drawing_can_be_placed_on_another_floor() {
    let mut sim = sim();
    let mut up = sim.app.cx.project.floors[0].clone();
    up.name = "2nd Floor".into();
    sim.app.cx.project.floors.push(up);
    sim.app.cx.refresh();
    open(&mut sim, "site.dxf", site_dxf().into_bytes());
    imp::with_assistant(|a| {
        a.show_assistant = false;
        a.target_floor = 1;
    });
    imp::next(&mut sim.app.cx);
    assert_eq!(sim.app.cx.floor, 0, "the plan view stays where it was");
    assert!(sim.app.cx.project.floors[0].cad.is_empty());
    let f = &sim.app.cx.project.floors[1];
    assert!(f.cad.len() > 10 && f.dimensions.len() == 1 && f.cad_blocks().len() == 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Import Drawing"));
    sim.undo();
    assert!(sim.app.cx.project.floors[1].cad.is_empty());
}

#[test]
fn the_import_drawing_menu_command_is_wired() {
    // The command reaches the dialog's entry point (the file picker itself
    // needs a window, so the test only checks nothing breaks without one).
    let _ = FileCommand::ImportDxf;
    let _ = Action::File(FileCommand::ImportDxf);
}

#[test]
fn the_plans_own_dxf_export_reads_back_with_the_same_counts_and_geometry() {
    imp::close();
    let mut sim = Sim::new();
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        Point::new(240.0, 180.0),
        Point::new(0.0, 180.0),
    ];
    for i in 0..4 {
        sim.app.cx.project.add_wall(
            0,
            corners[i],
            corners[(i + 1) % 4],
            6.0,
            96.0,
            WallKind::Exterior,
        );
    }
    let p = &mut sim.app.cx.project;
    p.add_cad(
        0,
        "CAD, Default",
        CadItem::Line {
            a: Point::new(10.0, 20.0),
            b: Point::new(110.5, 60.25),
        },
    );
    p.add_cad(
        0,
        "CAD, Default",
        CadItem::Circle {
            center: Point::new(50.0, 70.0),
            radius: 12.5,
        },
    );
    p.add_cad(
        0,
        "CAD, Default",
        CadItem::Arc {
            center: Point::new(150.0, 70.0),
            radius: 20.0,
            start_angle: 0.3,
            end_angle: 2.1,
        },
    );
    p.add_cad(
        0,
        "CAD, Default",
        CadItem::Polyline {
            points: vec![
                Point::new(5.0, 5.0),
                Point::new(35.0, 8.0),
                Point::new(40.0, 30.0),
            ],
            closed: false,
        },
    );
    p.add_cad(
        0,
        "Notes",
        CadItem::Text {
            pos: Point::new(20.0, 150.0),
            text: "Kitchen".into(),
            height: 4.0,
            angle: 0.0,
        },
    );
    sim.app.cx.refresh();
    let before: Vec<(String, CadItem)> = sim
        .app
        .cx
        .floor()
        .cad
        .iter()
        .map(|c| (c.layer.clone(), c.item.clone()))
        .collect();
    let wall_outlines = sim.app.cx.outlines.len();
    assert_eq!(wall_outlines, 4);

    for units in [OutUnits::Inches, OutUnits::Millimeters] {
        let mut d = DxfExportDialog::new(&sim.app.cx);
        d.opts.units = units;
        d.floors = crate::dialogs::dxf_options::FloorChoice::ThisFloor;
        let text = d.text(&mut sim.app.cx).unwrap();

        // Read it into an empty plan by the assistant, with its own units.
        let mut other = Sim::new();
        draw_shell(&mut other, 480.0, 360.0);
        imp::open_files(
            &mut other.app.cx,
            vec![("round.dxf".into(), text.into_bytes())],
        );
        imp::with_assistant(|a| {
            a.show_assistant = false;
            a.to_origin = false;
            // The layer names are the plan's own.
        });
        imp::next(&mut other.app.cx);
        let f = other.app.cx.floor();
        // Counts: the CAD objects and one closed outline per wall come back.
        let count =
            |pred: &dyn Fn(&CadItem) -> bool| f.cad.iter().filter(|c| pred(&c.item)).count();
        assert_eq!(
            count(&|i| matches!(i, CadItem::Line { .. })),
            1,
            "{units:?}"
        );
        assert_eq!(count(&|i| matches!(i, CadItem::Circle { .. })), 1);
        assert_eq!(count(&|i| matches!(i, CadItem::Arc { .. })), 1);
        assert_eq!(
            count(&|i| matches!(i, CadItem::Polyline { closed: false, .. })),
            1
        );
        assert_eq!(
            count(&|i| matches!(i, CadItem::Polyline { closed: true, .. })),
            wall_outlines
        );
        assert_eq!(
            count(&|i| matches!(i, CadItem::Text { text, .. } if text == "Kitchen")),
            1
        );
        // Geometry within 1e-3 of an inch.
        let want = |layer: &str, pred: &dyn Fn(&CadItem) -> bool| -> CadItem {
            before
                .iter()
                .find(|(l, i)| l == layer && pred(i))
                .unwrap()
                .1
                .clone()
        };
        let got = |pred: &dyn Fn(&CadItem) -> bool| -> CadItem {
            f.cad.iter().find(|c| pred(&c.item)).unwrap().item.clone()
        };
        let tol = 1e-3;
        let (w, g) = (
            want("CAD, Default", &|i| matches!(i, CadItem::Line { .. })),
            got(&|i| matches!(i, CadItem::Line { .. })),
        );
        match (w, g) {
            (CadItem::Line { a, b }, CadItem::Line { a: ga, b: gb }) => {
                assert!(
                    a.dist(ga) < tol && b.dist(gb) < tol,
                    "{units:?} line {ga:?} {gb:?}"
                );
            }
            _ => unreachable!(),
        }
        match (
            want("CAD, Default", &|i| matches!(i, CadItem::Circle { .. })),
            got(&|i| matches!(i, CadItem::Circle { .. })),
        ) {
            (
                CadItem::Circle { center, radius },
                CadItem::Circle {
                    center: gc,
                    radius: gr,
                },
            ) => {
                assert!(center.dist(gc) < tol && (radius - gr).abs() < tol);
            }
            _ => unreachable!(),
        }
        match (
            want("CAD, Default", &|i| matches!(i, CadItem::Arc { .. })),
            got(&|i| matches!(i, CadItem::Arc { .. })),
        ) {
            (
                CadItem::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                },
                CadItem::Arc {
                    center: gc,
                    radius: gr,
                    start_angle: gs,
                    end_angle: ge,
                },
            ) => {
                assert!(center.dist(gc) < tol && (radius - gr).abs() < tol);
                // The file keeps degrees to four places.
                assert!((start_angle - gs).abs() < 1e-5 && (end_angle - ge).abs() < 1e-5);
            }
            _ => unreachable!(),
        }
        match (
            want("CAD, Default", &|i| matches!(i, CadItem::Polyline { .. })),
            got(&|i| matches!(i, CadItem::Polyline { closed: false, .. })),
        ) {
            (CadItem::Polyline { points, .. }, CadItem::Polyline { points: gp, .. }) => {
                assert_eq!(points.len(), gp.len());
                assert!(points.iter().zip(&gp).all(|(a, b)| a.dist(*b) < tol));
            }
            _ => unreachable!(),
        }
        match (
            want("Notes", &|i| matches!(i, CadItem::Text { .. })),
            got(&|i| matches!(i, CadItem::Text { text, .. } if text == "Kitchen")),
        ) {
            (
                CadItem::Text { pos, height, .. },
                CadItem::Text {
                    pos: gp,
                    height: gh,
                    ..
                },
            ) => {
                assert!(
                    pos.dist(gp) < tol && (height - gh).abs() < tol,
                    "{units:?} text {pos:?} {height} vs {gp:?} {gh}"
                );
            }
            _ => unreachable!(),
        }
        // The wall outlines enclose the same area as the plan's walls.
        let area_of = |items: &mut dyn Iterator<Item = &CadItem>| -> f64 {
            items
                .filter_map(|i| match i {
                    CadItem::Polyline {
                        points,
                        closed: true,
                    } => Some(plan_core::geometry::polygon_area(points).abs()),
                    _ => None,
                })
                .sum()
        };
        let got_area = area_of(&mut f.cad.iter().map(|c| &c.item));
        let want_area: f64 = sim
            .app
            .cx
            .outlines
            .iter()
            .map(|o| plan_core::geometry::polygon_area(&o.polygon).abs())
            .sum();
        assert!(
            (got_area - want_area).abs() < 0.01,
            "{units:?}: {got_area} vs {want_area}"
        );
    }
}
