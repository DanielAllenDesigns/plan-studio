//! Conversion tests: units, placement, layer mapping, every entity kind,
//! blocks, dimensions, looks, joining lines and adding to a project.

use super::*;
use crate::dxf::parse_dxf;
use plan_core::cad::CadItem;
use plan_core::Project;
use std::collections::BTreeMap;
use std::f64::consts::FRAC_PI_2;

fn dxf(spec: &str) -> String {
    let mut out = String::new();
    for l in spec.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let (c, v) = l.split_once(' ').unwrap_or((l, ""));
        out.push_str(c);
        out.push('\n');
        out.push_str(v.trim());
        out.push('\n');
    }
    out
}

fn drawing(blocks: &str, body: &str) -> DxfDrawing {
    parse_dxf(&dxf(&format!(
        "0 SECTION\n2 BLOCKS\n{blocks}\n0 ENDSEC\n0 SECTION\n2 ENTITIES\n{body}\n0 ENDSEC\n0 EOF"
    )))
    .unwrap()
}

fn conv(d: &DxfDrawing) -> Converted {
    convert(d, &ImportOptions::new(1.0, ""))
}

fn near(a: Point, x: f64, y: f64) -> bool {
    a.dist(Point::new(x, y)) < 1e-9
}

fn two_layer_drawing() -> DxfDrawing {
    drawing(
        "",
        "0 LINE\n8 A-WALL\n10 10\n20 0\n11 20\n21 0\n\
0 CIRCLE\n8 A-FURN\n10 10\n20 0\n40 2\n\
0 ARC\n8 A-WALL\n10 0\n20 0\n40 5\n50 0\n51 90\n\
0 TEXT\n8 NOTES\n10 10\n20 0\n40 3\n1 Hi\n50 0",
    )
}

#[test]
fn unit_factors() {
    assert_eq!(to_inches_factor(DxfUnits::Millimeters, None), 1.0 / 25.4);
    assert_eq!(to_inches_factor(DxfUnits::Feet, None), 12.0);
    assert_eq!(to_inches_factor(DxfUnits::Inches, None), 1.0);
    assert_eq!(to_inches_factor(DxfUnits::Unitless, None), 1.0);
    assert_eq!(to_inches_factor(DxfUnits::Yards, None), 36.0);
    assert!((to_inches_factor(DxfUnits::Meters, None) - 39.370_078_74).abs() < 1e-6);
    assert!((to_inches_factor(DxfUnits::Centimeters, None) - 1.0 / 2.54).abs() < 1e-12);
    assert_eq!(to_inches_factor(DxfUnits::Millimeters, Some(DxfUnits::Feet)), 12.0);
}

#[test]
fn a_file_without_units_is_inches_or_millimeters_by_its_measurement() {
    let imperial = parse_dxf("0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n").unwrap();
    assert_eq!(default_units(&imperial), DxfUnits::Inches);
    let metric = parse_dxf("0\nSECTION\n2\nHEADER\n9\n$MEASUREMENT\n70\n1\n0\nENDSEC\n0\nEOF\n").unwrap();
    assert_eq!(default_units(&metric), DxfUnits::Millimeters);
    let declared = parse_dxf("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n2\n9\n$MEASUREMENT\n70\n1\n0\nENDSEC\n0\nEOF\n").unwrap();
    assert_eq!(default_units(&declared), DxfUnits::Feet);
}

#[test]
fn layer_mapping_renames_skips_and_keeps() {
    let d = two_layer_drawing();
    let mut o = ImportOptions::new(1.0, "DXF: ");
    o.layers = vec![
        LayerMapping { source: "A-WALL".into(), target: LayerTarget::Rename("Walls, Normal".into()) },
        LayerMapping { source: "NOTES".into(), target: LayerTarget::Skip },
    ];
    let objs = to_cad_objects_with(&d, &o);
    let layers: Vec<&str> = objs.iter().map(|c| c.layer.as_str()).collect();
    assert_eq!(layers, ["Walls, Normal", "DXF: A-FURN", "Walls, Normal"]);
    assert_eq!(o.target_layer("NOTES"), None);
    assert_eq!(o.target_layer("OTHER").as_deref(), Some("DXF: OTHER"));
    // One layer for everything, except what is skipped.
    o.single_layer = Some("All".into());
    assert_eq!(o.target_layer("OTHER").as_deref(), Some("All"));
    assert_eq!(o.target_layer("A-WALL").as_deref(), Some("Walls, Normal"));
    assert_eq!(o.target_layer("NOTES"), None);
}

#[test]
fn scale_rotation_and_insertion_place_every_entity() {
    let d = two_layer_drawing();
    let mut o = ImportOptions::new(2.0, "");
    o.scale = 0.5;
    o.rotation_deg = 90.0;
    o.base = Point::new(10.0, 0.0);
    o.insertion = Point::new(100.0, 50.0);
    let c = convert(&d, &o);
    match &c.objects[0].item {
        CadItem::Line { a, b } => {
            assert!(near(*a, 100.0, 50.0));
            assert!(near(*b, 100.0, 60.0));
        }
        other => panic!("{other:?}"),
    }
    match &c.objects[1].item {
        CadItem::Circle { center, radius } => {
            assert!(near(*center, 100.0, 50.0));
            assert!((radius - 2.0).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    match &c.objects[2].item {
        CadItem::Arc { radius, start_angle, end_angle, .. } => {
            assert!((radius - 5.0).abs() < 1e-9);
            assert!((start_angle - FRAC_PI_2).abs() < 1e-9);
            assert!((end_angle - 2.0 * FRAC_PI_2).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    match &c.objects[3].item {
        CadItem::Text { height, angle, .. } => {
            assert!((height - 3.0).abs() < 1e-9);
            assert!((angle - FRAC_PI_2).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn plain_options_equal_the_old_conversion() {
    let d = two_layer_drawing();
    assert_eq!(
        to_cad_objects(&d, 0.5, "P: "),
        to_cad_objects_with(&d, &ImportOptions::new(0.5, "P: "))
    );
}

#[test]
fn a_bulged_polyline_samples_an_arc_and_remembers_the_edge() {
    let d = drawing("", "0 LWPOLYLINE\n8 W\n90 2\n70 0\n10 0\n20 0\n42 1\n10 10\n20 0");
    let c = conv(&d);
    let o = &c.objects[0];
    match &o.item {
        CadItem::Polyline { points, closed } => {
            assert_eq!(points.len(), 17);
            assert!(!closed);
            for p in points {
                assert!((p.dist(Point::new(5.0, 0.0)) - 5.0).abs() < 1e-9);
            }
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(o.attrs.arc_edges.len(), 1);
    let e = o.attrs.arc_edges[0];
    assert_eq!((e.from, e.to, e.bulge), (0, 16, 1.0));
    // A closed polyline whose last edge is the arc points past the end.
    let d = drawing("", "0 LWPOLYLINE\n8 W\n90 3\n70 1\n10 0\n20 0\n10 10\n20 0\n10 10\n20 10\n42 1");
    let o = &conv(&d).objects[0];
    let n = match &o.item {
        CadItem::Polyline { points, closed: true } => points.len(),
        other => panic!("{other:?}"),
    };
    assert_eq!(o.attrs.arc_edges.len(), 1);
    assert_eq!(o.attrs.arc_edges[0].to, n, "the closing arc ends at the first point");
}

#[test]
fn ellipses_splines_solids_and_points_become_plan_shapes() {
    let d = drawing(
        "",
        "0 ELLIPSE\n8 E\n10 0\n20 0\n11 10\n21 0\n40 0.5\n41 0\n42 6.283185307179586\n\
0 SPLINE\n8 S\n70 8\n71 2\n72 6\n73 3\n40 0\n40 0\n40 0\n40 1\n40 1\n40 1\n10 0\n20 0\n10 10\n20 20\n10 20\n20 0\n\
0 SPLINE\n8 S\n70 0\n71 3\n74 3\n11 0\n21 0\n11 5\n21 5\n11 10\n21 0\n\
0 SOLID\n8 F\n10 0\n20 0\n11 10\n21 0\n12 0\n22 10\n13 10\n23 10\n62 1\n\
0 3DFACE\n8 F\n10 0\n20 0\n30 3\n11 10\n21 0\n31 3\n12 10\n22 10\n32 3\n13 10\n23 10\n33 3\n\
0 POINT\n8 P\n10 3\n20 4",
    );
    let c = conv(&d);
    assert_eq!(c.objects.len(), 6);
    // The full ellipse is a closed polyline reaching 10 along x and 5 along y.
    match &c.objects[0].item {
        CadItem::Polyline { points, closed } => {
            assert!(*closed);
            let (lo, hi) = c.objects[0].item.bounds();
            assert!((hi.x - 10.0).abs() < 0.02 && (lo.y + 5.0).abs() < 0.02, "{lo:?} {hi:?} {}", points.len());
        }
        other => panic!("{other:?}"),
    }
    // The NURBS ends where its control points do; the fit-point spline passes its points.
    match &c.objects[1].item {
        CadItem::Polyline { points, closed: false } => {
            assert!(near(points[0], 0.0, 0.0) && near(*points.last().unwrap(), 20.0, 0.0));
            assert!(points.len() > 8);
        }
        other => panic!("{other:?}"),
    }
    match &c.objects[2].item {
        CadItem::Polyline { points, .. } => assert!(points.iter().any(|p| p.dist(Point::new(5.0, 5.0)) < 1e-9)),
        other => panic!("{other:?}"),
    }
    // The solid is a filled outline in its colour; the 3D face is an outline.
    assert!(matches!(&c.objects[3].item, CadItem::Polyline { closed: true, points } if points.len() == 4));
    assert_eq!(c.objects[3].attrs.fill.as_ref().map(|f| f.color), Some([255, 0, 0]));
    assert!(matches!(&c.objects[4].item, CadItem::Polyline { closed: true, points } if points.len() == 3));
    assert!(c.objects[4].attrs.fill.is_none());
    assert!(matches!(c.objects[5].item, CadItem::Circle { radius, .. } if radius == POINT_RADIUS));
    let mut o = ImportOptions::new(1.0, "");
    o.import_points = false;
    assert_eq!(convert(&d, &o).objects.len(), 5);
}

#[test]
fn hatches_are_filled_outlines_or_pattern_requests() {
    let loop_ = "92 7\n72 0\n73 1\n93 4\n10 0\n20 0\n10 100\n20 0\n10 100\n20 100\n10 0\n20 100\n97 0\n";
    let solid = drawing("", &format!("0 HATCH\n8 H\n62 3\n2 SOLID\n70 1\n71 0\n91 1\n{loop_}75 0\n76 1\n98 0"));
    let c = conv(&solid);
    assert_eq!(c.objects.len(), 1);
    assert_eq!(c.objects[0].attrs.fill.as_ref().map(|f| (f.color, f.pattern.clone())), Some(([0, 255, 0], String::new())));
    assert!(c.objects[0].hatch.is_none());

    let pat = drawing(
        "",
        &format!("0 HATCH\n8 H\n2 ANSI31\n70 0\n71 0\n91 1\n{loop_}75 0\n76 1\n52 0\n41 1\n78 1\n53 45\n43 0\n44 0\n45 -2\n46 2\n79 0\n98 0"),
    );
    let c = conv(&pat);
    let h = c.objects[0].hatch.as_ref().unwrap();
    assert_eq!(h.style, "Diagonal Lines");
    assert!((h.spacing - 2.0 * 2f64.sqrt()).abs() < 1e-6 || h.spacing > 0.5);
    assert!(c.objects[0].attrs.fill.is_none());
    // Hatches can be left out.
    let mut o = ImportOptions::new(1.0, "");
    o.import_hatch = false;
    assert!(convert(&pat, &o).objects.is_empty());
    assert_eq!(hatch_style("brick", 0.0), ("Brick", false));
    assert_eq!(hatch_style("AR-CONC", 0.0), ("Concrete", false));
    assert_eq!(hatch_style("MYSTERY", 90.0), ("Vertical Lines", false));
    assert_eq!(hatch_style("SOLID", 0.0), ("Solid", true));
}

#[test]
fn text_is_anchored_by_its_justification_and_keeps_its_formatting() {
    // Left, baseline: the position is the lower left.
    let d = drawing("", "0 TEXT\n8 T\n10 5\n20 6\n40 2\n1 Kitchen\n7 Notes");
    let mut o = ImportOptions::new(1.0, "");
    o.text_styles = vec!["NOTES".into()];
    let c = convert(&d, &o);
    match &c.objects[0].item {
        CadItem::Text { pos, text, height, angle } => {
            assert!(near(*pos, 5.0, 6.0));
            assert_eq!((text.as_str(), *height, *angle), ("Kitchen", 2.0, 0.0));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(c.objects[0].attrs.text_style.as_deref(), Some("NOTES"));
    // Centered and middle: moved left by half the width and down by half the height.
    let d = drawing("", "0 TEXT\n8 T\n10 0\n20 0\n11 50\n21 60\n40 4\n1 Hello\n72 1\n73 2");
    match &conv(&d).objects[0].item {
        CadItem::Text { pos, .. } => {
            let w = 5.0 * 4.0 * TEXT_WIDTH_FACTOR;
            assert!(near(*pos, 50.0 - w / 2.0, 60.0 - 2.0), "{pos:?}");
        }
        other => panic!("{other:?}"),
    }
    // MTEXT top-left: the lower left is a block height below the point.
    let d = drawing("", "0 MTEXT\n8 T\n10 0\n20 100\n40 3\n71 1\n1 one\\Ptwo");
    match &conv(&d).objects[0].item {
        CadItem::Text { pos, text, .. } => {
            assert_eq!(text, "one\ntwo");
            let block = 2.0 * 3.0 * plan_core::text_box::LINE_SPACING;
            assert!(near(*pos, 0.0, 100.0 - block), "{pos:?}");
        }
        other => panic!("{other:?}"),
    }
    // Formatting and a wrap width make rich runs and a text box.
    let d = drawing("", "0 MTEXT\n8 T\n10 0\n20 0\n40 3\n41 90\n71 2\n1 {\\fArial|b1;Bold} and plain");
    let c = conv(&d);
    let a = &c.objects[0].attrs;
    assert_eq!(a.runs.len(), 2);
    assert!(a.runs[0].bold && a.runs[0].font.as_deref() == Some("Arial"));
    assert_eq!(a.text_box.width, 90.0);
    assert_eq!(a.text_box.halign, HAlign::Center);
}

#[test]
fn inserts_become_cad_blocks_with_nesting_scale_and_rotation() {
    let blocks = "0 BLOCK\n2 LEG\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 0\n21 10\n0 ENDBLK\n\
0 BLOCK\n2 TABLE\n10 0\n20 0\n\
0 LINE\n8 0\n10 0\n20 0\n11 40\n21 0\n\
0 INSERT\n8 0\n2 LEG\n10 0\n20 0\n\
0 INSERT\n8 0\n2 LEG\n10 40\n20 0\n0 ENDBLK";
    let d = drawing(blocks, "0 INSERT\n8 FURN\n2 table\n10 100\n20 200\n41 2\n42 2\n50 90");
    let c = conv(&d);
    // 1 top + 2 legs, all on the INSERT's layer, one CAD block.
    assert_eq!(c.objects.len(), 3);
    assert!(c.objects.iter().all(|o| o.layer == "FURN" && o.block == Some(0)));
    assert_eq!(c.blocks, vec![ImportedBlock { name: "table".into(), insertion: Point::new(100.0, 200.0) }]);
    // The top: (0,0)-(40,0) x2 rotated 90 deg at (100,200) -> (100,200)-(100,280).
    match &c.objects[0].item {
        CadItem::Line { a, b } => assert!(near(*a, 100.0, 200.0) && near(*b, 100.0, 280.0), "{a:?} {b:?}"),
        other => panic!("{other:?}"),
    }
    // A leg of the nested block: its INSERT sits at (40,0) in the table,
    // which is (100, 280) in the plan; the leg runs 10*2 along the rotated y (-x).
    match &c.objects[2].item {
        CadItem::Line { a, b } => assert!(near(*a, 100.0, 280.0) && near(*b, 80.0, 280.0), "{a:?} {b:?}"),
        other => panic!("{other:?}"),
    }
    // Loose objects when blocks are off.
    let mut o = ImportOptions::new(1.0, "");
    o.blocks = BlockMode::None;
    let c = convert(&d, &o);
    assert!(c.blocks.is_empty() && c.objects.iter().all(|o| o.block.is_none()));
    // The whole drawing as one block.
    o.blocks = BlockMode::WholeDrawing;
    o.drawing_name = "Site".into();
    assert_eq!(convert(&d, &o).drawing_block.as_deref(), Some("Site"));
}

#[test]
fn byblock_colors_and_layer_zero_follow_the_insert() {
    let blocks = "0 BLOCK\n2 B\n10 0\n20 0\n\
0 LINE\n8 0\n62 0\n10 0\n20 0\n11 5\n21 0\n\
0 LINE\n8 KEEP\n62 3\n10 0\n20 5\n11 5\n21 5\n0 ENDBLK";
    let d = drawing(blocks, "0 INSERT\n8 FURN\n62 1\n2 B\n10 0\n20 0");
    let c = conv(&d);
    assert_eq!(c.objects[0].layer, "FURN");
    assert_eq!(c.objects[0].attrs.color, Some([255, 0, 0]), "BYBLOCK takes the insert's colour");
    assert_eq!(c.objects[1].layer, "KEEP");
    assert_eq!(c.objects[1].attrs.color, Some([0, 255, 0]));
}

#[test]
fn attributes_replace_the_definitions_and_arrays_repeat() {
    let blocks = "0 BLOCK\n2 TAGGED\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 5\n21 0\n\
0 ATTDEF\n8 0\n10 0\n20 1\n40 2\n1 default\n2 NAME\n70 0\n0 ENDBLK";
    let with = drawing(blocks, "0 INSERT\n8 F\n66 1\n2 TAGGED\n10 0\n20 0\n0 ATTRIB\n8 F\n10 1\n20 1\n40 2\n1 A-12\n2 NAME\n70 0\n0 SEQEND");
    let c = conv(&with);
    let texts: Vec<&str> = c.objects.iter().filter_map(|o| match &o.item { CadItem::Text { text, .. } => Some(text.as_str()), _ => None }).collect();
    assert_eq!(texts, vec!["A-12"]);
    // Without attributes the definition's default shows.
    let without = drawing(blocks, "0 INSERT\n8 F\n2 TAGGED\n10 0\n20 0");
    let texts: Vec<String> = conv(&without).objects.iter().filter_map(|o| match &o.item { CadItem::Text { text, .. } => Some(text.clone()), _ => None }).collect();
    assert_eq!(texts, vec!["default".to_string()]);
    // A 3x2 array of the block.
    let arr = drawing(blocks, "0 INSERT\n8 F\n2 TAGGED\n10 0\n20 0\n70 3\n71 2\n44 10\n45 20");
    let lines = conv(&arr).objects.iter().filter(|o| matches!(o.item, CadItem::Line { .. })).count();
    assert_eq!(lines, 6);
}

#[test]
fn a_missing_block_and_an_xref_are_reported() {
    let d = drawing("0 BLOCK\n2 SITE\n70 4\n1 site.dwg\n0 ENDBLK", "0 INSERT\n8 F\n2 GONE\n10 0\n20 0\n0 INSERT\n8 F\n2 SITE\n10 0\n20 0");
    let c = conv(&d);
    assert!(c.objects.is_empty());
    let notes = c.notes.join("\n");
    assert!(notes.contains("GONE"), "{notes}");
    assert!(notes.contains("site.dwg") && notes.contains("External references"), "{notes}");
}

#[test]
fn a_self_referencing_block_does_not_loop() {
    let d = drawing("0 BLOCK\n2 LOOP\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 1\n21 0\n0 INSERT\n8 0\n2 LOOP\n10 1\n20 0\n0 ENDBLK", "0 INSERT\n8 F\n2 LOOP\n10 0\n20 0");
    let c = conv(&d);
    assert!(!c.objects.is_empty() && c.objects.len() < 40);
}

#[test]
fn dimensions_become_objects_or_drawn_blocks() {
    let body = "0 DIMENSION\n8 D\n2 *D1\n10 50\n20 20\n11 50\n21 25\n70 1\n1 <>\n13 0\n23 0\n14 100\n24 0\n42 100";
    let blocks = "0 BLOCK\n2 *D1\n70 1\n10 0\n20 0\n\
0 LINE\n8 0\n10 0\n20 20\n11 100\n21 20\n\
0 LINE\n8 0\n10 0\n20 2\n11 0\n21 22\n\
0 LINE\n8 0\n10 100\n20 2\n11 100\n21 22\n\
0 TEXT\n8 0\n10 50\n20 22\n40 2\n1 100\n0 ENDBLK";
    let d = drawing(blocks, body);
    let c = conv(&d);
    assert_eq!(c.dimensions.len(), 1);
    assert!(c.objects.is_empty());
    let dim = &c.dimensions[0].dim;
    assert!(near(dim.start, 0.0, 0.0) && near(dim.end, 100.0, 0.0));
    assert!((dim.offset - 20.0).abs() < 1e-9);
    assert!(dim.text_override.is_none());
    // As CAD blocks: the drawing of the dimension, grouped, with the layer of the dimension.
    let mut o = ImportOptions::new(1.0, "");
    o.dimensions = DimensionMode::Blocks;
    let c = convert(&d, &o);
    assert!(c.dimensions.is_empty());
    assert_eq!(c.objects.len(), 4);
    assert!(c.objects.iter().all(|x| x.layer == "D" && x.block == Some(0)));
    assert_eq!(c.blocks[0].name, "Dimension");
    // A dimension scaled by its drawing's units: 1 unit = 2 inches.
    let mut o = ImportOptions::new(2.0, "");
    o.dimensions = DimensionMode::Objects;
    let dim = &convert(&d, &o).dimensions[0].dim;
    assert!(near(dim.end, 200.0, 0.0) && (dim.offset - 40.0).abs() < 1e-9);
    // An overriding text is kept.
    let d = drawing("", "0 DIMENSION\n8 D\n10 5\n20 5\n70 0\n1 EQ\n13 0\n23 0\n14 10\n24 0\n50 0");
    assert_eq!(conv(&d).dimensions[0].dim.text_override.as_deref(), Some("EQ"));
}

#[test]
fn angular_and_radius_dimensions_are_drawn_without_a_block() {
    let d = drawing("", "0 DIMENSION\n8 D\n10 7\n20 7\n11 8\n21 8\n70 5\n13 10\n23 0\n14 0\n24 10\n15 0\n25 0\n3 STANDARD");
    let c = conv(&d);
    assert!(c.dimensions.is_empty());
    assert!(c.objects.iter().any(|o| matches!(o.item, CadItem::Arc { .. })));
    assert!(c.objects.iter().any(|o| matches!(&o.item, CadItem::Text { text, .. } if text.contains("90"))));
}

#[test]
fn leaders_get_an_arrow_and_multileaders_their_text() {
    let d = drawing(
        "",
        "0 LEADER\n8 L\n71 1\n76 3\n10 0\n20 0\n10 10\n20 10\n10 20\n20 10\n\
0 MULTILEADER\n8 L\n300 CONTEXT_DATA{\n41 2.5\n302 LEADER{\n304 LEADER_LINE{\n10 0\n20 0\n10 15\n20 10\n305 }\n303 }\n304 Drain\n12 20\n22 10\n301 }",
    );
    let c = conv(&d);
    assert_eq!(c.objects.len(), 3);
    assert_eq!(c.objects[0].attrs.arrow_start, ArrowStyle::Filled);
    assert!(matches!(&c.objects[0].item, CadItem::Polyline { points, closed: false } if points.len() == 3));
    assert!(matches!(&c.objects[2].item, CadItem::Text { text, .. } if text == "Drain"));
}

#[test]
fn looks_follow_the_object_or_the_layer() {
    let src = dxf(
        "0 SECTION
2 TABLES
0 TABLE
2 LAYER
0 LAYER
2 A-WALL
70 0
62 1
6 DASHED
370 50
0 LAYER
2 PLAIN
70 0
62 7
6 CONTINUOUS
0 ENDTAB
0 TABLE
2 LTYPE
0 LTYPE
2 DASHED
73 2
49 0.5
49 -0.25
0 ENDTAB
0 ENDSEC
0 SECTION
2 ENTITIES
0 LINE
8 A-WALL
10 0
20 0
11 1
21 0
0 LINE
8 A-WALL
62 5
370 25
6 DOTTED
10 0
20 0
11 1
21 0
0 LINE
8 PLAIN
10 0
20 0
11 1
21 0
0 ENDSEC
0 EOF",
    );
    let d = parse_dxf(&src).unwrap();
    // Layer attributes are imported: only the object's own look is on it.
    let mut o = ImportOptions::new(1.0, "");
    o.layer_attrs = true;
    let c = convert(&d, &o);
    assert!(c.objects[0].attrs.is_default());
    let a = &c.objects[1].attrs;
    assert_eq!((a.color, a.weight, a.dash), (Some([0, 0, 255]), Some(25), Some(LineStyle::Dotted)));
    let wall = c.layers.iter().find(|l| l.name == "A-WALL").unwrap();
    assert_eq!((wall.color, wall.weight, wall.line_style), ([255, 0, 0], 50, LineStyle::Dashed));
    let plain = c.layers.iter().find(|l| l.name == "PLAIN").unwrap();
    assert_eq!(plain.color, [0, 0, 0], "index 7 prints black");
    // Without them (one plan layer) each object carries the look of its layer.
    let mut o = ImportOptions::new(1.0, "");
    o.single_layer = Some("All".into());
    o.object_attrs = true;
    let c = convert(&d, &o);
    let a = &c.objects[0].attrs;
    assert_eq!((a.color, a.weight, a.dash), (Some([255, 0, 0]), Some(50), Some(LineStyle::Dashed)));
    assert!(c.objects[2].attrs.is_default() || c.objects[2].attrs.color == Some([0, 0, 0]));
    assert_eq!(c.layers.len(), 1);
    assert_eq!(c.layers[0].name, "All");
}

#[test]
fn line_types_map_to_the_nearest_style() {
    assert_eq!(linetype_style(None, "CONTINUOUS"), LineStyle::Solid);
    assert_eq!(linetype_style(None, "HIDDEN2"), LineStyle::Dashed);
    assert_eq!(linetype_style(None, "CENTER"), LineStyle::DashDot);
    assert_eq!(linetype_style(None, "DOT"), LineStyle::Dotted);
    assert_eq!(linetype_style(None, "ACAD_ISO02W100"), LineStyle::Solid);
    let custom = DxfLinetype { name: "MINE".into(), description: String::new(), pattern: vec![1.0, -0.5, 0.0, -0.5] };
    assert_eq!(linetype_style(Some(&custom), "MINE"), LineStyle::DashDot);
    let dashes = DxfLinetype { pattern: vec![1.0, -0.5], ..custom.clone() };
    assert_eq!(linetype_style(Some(&dashes), "MINE"), LineStyle::Dashed);
    let dots = DxfLinetype { pattern: vec![0.0, -0.5], ..custom };
    assert_eq!(linetype_style(Some(&dots), "MINE"), LineStyle::Dotted);
}

#[test]
fn lines_join_into_polylines_and_boxes() {
    let d = drawing(
        "",
        "0 LINE\n8 A\n10 0\n20 0\n11 10\n21 0\n\
0 LINE\n8 A\n10 10\n20 0\n11 10\n21 5\n\
0 LINE\n8 A\n10 10\n20 5\n11 0\n21 5\n\
0 LINE\n8 A\n10 0\n20 5\n11 0\n21 0\n\
0 LINE\n8 A\n10 50\n20 0\n11 60\n21 0\n\
0 LINE\n8 A\n10 60\n20 0\n11 60\n21 10",
    );
    let mut o = ImportOptions::new(1.0, "");
    o.boxes = true;
    let c = convert(&d, &o);
    // The rectangle is a box; the open pair stays two lines.
    let polys = c.objects.iter().filter(|x| matches!(x.item, CadItem::Polyline { closed: true, .. })).count();
    let lines = c.objects.iter().filter(|x| matches!(x.item, CadItem::Line { .. })).count();
    assert_eq!((polys, lines), (1, 2));
    o.boxes = false;
    o.join_lines = true;
    let c = convert(&d, &o);
    assert_eq!(c.objects.len(), 2);
    assert!(c.objects.iter().all(|x| matches!(x.item, CadItem::Polyline { .. })));
}

#[test]
fn paper_space_comes_in_as_one_block_only_when_asked() {
    let d = drawing("", "0 LINE\n8 A\n10 0\n20 0\n11 1\n21 0\n0 LINE\n8 A\n67 1\n10 0\n20 0\n11 5\n21 0\n0 LINE\n8 A\n67 1\n10 0\n20 1\n11 5\n21 1");
    let c = conv(&d);
    assert_eq!(c.objects.len(), 1);
    assert!(c.notes.iter().any(|n| n.contains("paper space")));
    let mut o = ImportOptions::new(1.0, "");
    o.include_paper_space = true;
    let c = convert(&d, &o);
    assert_eq!(c.objects.len(), 3);
    assert_eq!(c.blocks[0].name, "Paper Space");
    assert_eq!(c.objects.iter().filter(|x| x.block == Some(0)).count(), 2);
}

#[test]
fn counts_unused_blocks_and_bounds() {
    let d = drawing(
        "0 BLOCK\n2 USED\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 1\n21 1\n0 ENDBLK\n0 BLOCK\n2 SPARE\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 1\n21 1\n0 ENDBLK",
        "0 INSERT\n8 F\n2 USED\n10 100\n20 100\n0 LINE\n8 A\n10 0\n20 0\n11 5\n21 5",
    );
    assert_eq!(unused_blocks(&d), vec!["SPARE".to_string()]);
    let (lo, hi) = drawing_bounds(&d, false).unwrap();
    assert!(near(lo, 0.0, 0.0) && near(hi, 101.0, 101.0), "{lo:?} {hi:?}");
    let counts = layer_counts(&d, false);
    assert_eq!(counts, vec![("A".to_string(), 1), ("F".to_string(), 1)]);
}

#[test]
fn adding_to_a_project_makes_layers_objects_attrs_dimensions_and_blocks() {
    let blocks = "0 BLOCK\n2 CHAIR\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 10\n21 0\n0 LINE\n8 0\n62 1\n10 10\n20 0\n11 10\n21 10\n0 ENDBLK";
    let body = "0 INSERT\n8 FURN\n2 CHAIR\n10 100\n20 100\n\
0 DIMENSION\n8 D\n10 5\n20 5\n70 1\n13 0\n23 0\n14 10\n24 0\n\
0 LINE\n8 FURN\n10 0\n20 0\n11 1\n21 0";
    let d = drawing(blocks, body);
    let mut p = Project::new("t");
    let c = conv(&d);
    let rep = apply_converted(&mut p, 0, &c, BlockConflict::AutoName, &BTreeMap::new());
    assert_eq!(rep.ids.len(), 3);
    assert!(rep.ids.iter().all(|i| *i != 0));
    assert_eq!(rep.dimension_ids.len(), 1);
    assert_eq!(rep.new_layers, 1);
    assert!(p.layers.get("FURN").is_some());
    let f = &p.floors[0];
    assert_eq!(f.cad.len(), 3);
    assert_eq!(f.dimensions.len(), 1);
    // The second chair line keeps its red.
    assert_eq!(f.cad_attrs(rep.ids[1]).unwrap().color, Some([255, 0, 0]));
    // The chair is a CAD block named for the INSERT, its insertion point at the INSERT.
    let blocks = f.cad_blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].name, "CHAIR");
    assert!(blocks[0].insertion.unwrap().dist(Point::new(100.0, 100.0)) < 1e-9);
    assert_eq!(f.group_members_cad(blocks[0].group).len(), 2);
}

#[test]
fn a_block_the_floor_already_has_is_renamed_replaced_or_reused() {
    let blocks = "0 BLOCK\n2 CHAIR\n10 0\n20 0\n0 LINE\n8 0\n10 0\n20 0\n11 10\n21 0\n0 LINE\n8 0\n10 10\n20 0\n11 10\n21 10\n0 ENDBLK";
    let d = drawing(blocks, "0 INSERT\n8 FURN\n2 CHAIR\n10 100\n20 100");
    let c = conv(&d);
    let run = |policy: BlockConflict| {
        let mut p = Project::new("t");
        apply_converted(&mut p, 0, &c, BlockConflict::AutoName, &BTreeMap::new());
        let first = p.floors[0].cad_blocks()[0].group;
        let rep = apply_converted(&mut p, 0, &c, policy, &BTreeMap::new());
        (p, first, rep)
    };
    let (p, _, rep) = run(BlockConflict::AutoName);
    let names: Vec<String> = p.floors[0].cad_blocks().into_iter().map(|b| b.name).collect();
    assert_eq!(names, vec!["CHAIR".to_string(), "CHAIR_Copy_1".to_string()]);
    assert_eq!(rep.renamed_blocks, vec![("CHAIR".to_string(), "CHAIR_Copy_1".to_string())]);
    let (p, first, rep) = run(BlockConflict::Replace);
    let blocks = p.floors[0].cad_blocks();
    assert_eq!(blocks.len(), 1);
    assert_ne!(blocks[0].group, first, "the old block is gone");
    assert_eq!((rep.replaced_blocks, p.floors[0].cad.len()), (1, 2));
    let (p, first, rep) = run(BlockConflict::UseExisting);
    // The imported objects are discarded; a copy of the block on the floor goes at the INSERT.
    assert_eq!(rep.reused_blocks, 1);
    assert_eq!(p.floors[0].cad.len(), 4);
    assert!(p.floors[0].cad_blocks().iter().any(|b| b.group == first));
    // One name decided on its own.
    let mut p = Project::new("t");
    apply_converted(&mut p, 0, &c, BlockConflict::AutoName, &BTreeMap::new());
    let per: BTreeMap<String, BlockConflict> = [("CHAIR".to_string(), BlockConflict::Replace)].into_iter().collect();
    let rep = apply_converted(&mut p, 0, &c, BlockConflict::AutoName, &per);
    assert_eq!(rep.replaced_blocks, 1);
}

#[test]
fn the_whole_drawing_can_be_one_block() {
    let d = drawing("", "0 LINE\n8 A\n10 0\n20 0\n11 1\n21 0\n0 LINE\n8 B\n10 0\n20 0\n11 2\n21 0\n0 CIRCLE\n8 B\n10 0\n20 0\n40 2");
    let mut o = ImportOptions::new(1.0, "");
    o.blocks = BlockMode::WholeDrawing;
    o.drawing_name = "Survey".into();
    let c = convert(&d, &o);
    let mut p = Project::new("t");
    let rep = apply_converted(&mut p, 0, &c, BlockConflict::AutoName, &BTreeMap::new());
    assert_eq!(rep.blocks_made, 1);
    let b = p.floors[0].cad_blocks();
    assert_eq!(b[0].name, "Survey");
    assert_eq!(p.floors[0].group_members_cad(b[0].group).len(), 3);
}
