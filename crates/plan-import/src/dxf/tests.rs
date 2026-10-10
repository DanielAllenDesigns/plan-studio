//! Reader tests: synthetic DXF text per entity, each read as ASCII and as
//! the same file converted to binary.

use super::*;
use plan_core::Point;

/// "code value" lines to DXF text.
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

/// Entities wrapped in an ENTITIES section.
fn ents(body: &str) -> String {
    dxf(&format!("0 SECTION\n2 ENTITIES\n{body}\n0 ENDSEC\n0 EOF"))
}

/// Parse as ASCII and as binary; the two must agree.
fn both(text: &str) -> DxfDrawing {
    let a = parse_dxf(text).expect("ascii");
    let bin = tokens::ascii_to_binary(text);
    let mut b = parse_dxf_bytes(&bin).expect("binary");
    assert_eq!(b.format, DxfFormat::Binary);
    b.format = DxfFormat::Ascii;
    assert_eq!(a, b, "binary and ASCII readings differ");
    a
}

fn only(d: &DxfDrawing) -> &DxfEntity {
    assert_eq!(d.entities.len(), 1, "{:?}", d.entities);
    &d.entities[0]
}

fn near(a: Point, x: f64, y: f64) -> bool {
    a.dist(Point::new(x, y)) < 1e-9
}

#[test]
fn a_whole_file_reads_header_tables_blocks_and_entities() {
    let src = dxf("0 SECTION
2 HEADER
9 $ACADVER
1 AC1027
9 $INSUNITS
70 4
9 $MEASUREMENT
70 1
9 $DIMSCALE
40 50
9 $EXTMIN
10 0
20 0
30 0
9 $EXTMAX
10 5000
20 3000
30 0
0 ENDSEC
0 SECTION
2 TABLES
0 TABLE
2 LAYER
70 3
0 LAYER
2 A-WALL
70 0
62 1
6 CONTINUOUS
370 50
0 LAYER
2 A-HIDDEN
70 0
62 -3
6 DASHED
0 LAYER
2 A-FROZEN
70 1
62 5
420 16711680
290 0
0 ENDTAB
0 TABLE
2 LTYPE
0 LTYPE
2 DASHED
3 Dashed __ __
73 2
40 0.75
49 0.5
49 -0.25
0 LTYPE
2 DOTS
73 2
49 0
49 -0.25
0 ENDTAB
0 TABLE
2 STYLE
0 STYLE
2 NOTES
3 arial.ttf
41 0.8
50 15
40 0
0 ENDTAB
0 TABLE
2 DIMSTYLE
0 DIMSTYLE
2 PLAN
40 48
41 0.125
140 0.1
0 ENDTAB
0 ENDSEC
0 SECTION
2 BLOCKS
0 BLOCK
8 0
2 CHAIR
70 0
10 1
20 1
0 LINE
8 0
10 1
20 1
11 11
21 1
0 ENDBLK
0 ENDSEC
0 SECTION
2 ENTITIES
0 LINE
8 A-WALL
10 0
20 0
11 1000
21 0
0 ENDSEC
0 EOF");
    let d = both(&src);
    assert_eq!(d.version, "AC1027");
    assert_eq!(d.units, DxfUnits::Millimeters);
    assert!(d.metric);
    assert_eq!(d.dim_scale, 50.0);
    assert_eq!(
        d.extents,
        Some((Point::new(0.0, 0.0), Point::new(5000.0, 3000.0)))
    );
    assert_eq!(d.layers.len(), 3);
    let wall = d.layer("a-wall").unwrap();
    assert_eq!(
        (wall.color, wall.weight, wall.visible),
        (DxfColor::Aci(1), 50, true)
    );
    let hidden = d.layer("A-HIDDEN").unwrap();
    assert_eq!(
        (hidden.color, hidden.visible, hidden.linetype.as_str()),
        (DxfColor::Aci(3), false, "DASHED")
    );
    let frozen = d.layer("A-FROZEN").unwrap();
    assert!(frozen.frozen && !frozen.visible && !frozen.plot);
    assert_eq!(frozen.color, DxfColor::Rgb([255, 0, 0]));
    assert_eq!(d.linetypes.len(), 2);
    assert_eq!(d.linetype("dashed").unwrap().pattern, vec![0.5, -0.25]);
    let st = d.text_style("NOTES").unwrap();
    assert_eq!(
        (st.font.as_str(), st.width_factor, st.oblique_deg),
        ("arial.ttf", 0.8, 15.0)
    );
    let ds = d.dim_style("PLAN");
    assert_eq!((ds.scale, ds.arrow, ds.text_height), (48.0, 0.125, 0.1));
    // A style the table lacks gets the header's values.
    assert_eq!(d.dim_style("nope").scale, 50.0);
    let b = d.block("chair").unwrap();
    assert_eq!(
        (b.base, b.entities.len(), b.anonymous),
        (Point::new(1.0, 1.0), 1, false)
    );
    assert_eq!(d.entities.len(), 1);
}

#[test]
fn lines_circles_arcs_and_ellipses() {
    let d = both(&ents("0 LINE\n8 A\n10 1\n20 2\n30 9\n11 3\n21 4\n31 9"));
    assert!(
        matches!(only(&d).kind, DxfKind::Line { a, b } if near(a, 1.0, 2.0) && near(b, 3.0, 4.0))
    );
    assert_eq!(only(&d).props.layer, "A");

    let d = both(&ents("0 CIRCLE\n8 0\n10 5\n20 6\n40 7"));
    assert!(
        matches!(only(&d).kind, DxfKind::Circle { center, radius } if near(center, 5.0, 6.0) && radius == 7.0)
    );

    let d = both(&ents("0 ARC\n8 0\n10 0\n20 0\n40 30\n50 0\n51 90"));
    assert!(
        matches!(only(&d).kind, DxfKind::Arc { radius, start_deg, end_deg, .. } if (radius, start_deg, end_deg) == (30.0, 0.0, 90.0))
    );

    // An ellipse: major axis 10 along x, ratio 0.5, a half sweep.
    let d = both(&ents(
        "0 ELLIPSE\n8 0\n10 1\n20 1\n30 0\n11 10\n21 0\n31 0\n40 0.5\n41 0\n42 3.141592653589793",
    ));
    match &only(&d).kind {
        DxfKind::Ellipse {
            center,
            u,
            v,
            t0,
            t1,
        } => {
            assert!(near(*center, 1.0, 1.0) && near(*u, 10.0, 0.0) && near(*v, 0.0, 5.0));
            assert_eq!(
                (*t0, (*t1 - std::f64::consts::PI).abs() < 1e-12),
                (0.0, true)
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_down_extrusion_mirrors_the_x_axis() {
    let d = both(&ents(
        "0 ARC\n8 0\n10 5\n20 0\n40 3\n50 0\n51 90\n210 0\n220 0\n230 -1",
    ));
    match &only(&d).kind {
        DxfKind::Arc {
            center,
            start_deg,
            end_deg,
            ..
        } => {
            assert!(near(*center, -5.0, 0.0));
            assert_eq!((*start_deg, *end_deg), (90.0, 180.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn polylines_keep_bulges_closed_flags_and_old_vertex_lists() {
    let d = both(&ents(
        "0 LWPOLYLINE\n8 W\n90 3\n70 1\n10 0\n20 0\n42 1\n10 100\n20 0\n10 100\n20 100",
    ));
    match &only(&d).kind {
        DxfKind::Polyline {
            points,
            closed,
            bulges,
        } => {
            assert_eq!(points.len(), 3);
            assert!(*closed);
            assert_eq!(bulges, &vec![1.0, 0.0, 0.0]);
        }
        other => panic!("{other:?}"),
    }
    let d = both(&ents(
        "0 POLYLINE\n8 P\n66 1\n70 1\n0 VERTEX\n8 P\n10 0\n20 0\n0 VERTEX\n8 P\n10 10\n20 0\n42 0.5\n0 VERTEX\n8 P\n10 10\n20 10\n0 SEQEND\n8 P",
    ));
    match &only(&d).kind {
        DxfKind::Polyline {
            points,
            closed,
            bulges,
        } => {
            assert_eq!(points.len(), 3);
            assert!(*closed);
            assert_eq!(bulges, &vec![0.0, 0.5, 0.0]);
        }
        other => panic!("{other:?}"),
    }
    // Spline frame points (flag 16) are not on the curve.
    let d = both(&ents(
        "0 POLYLINE\n8 P\n66 1\n70 4\n0 VERTEX\n8 P\n70 16\n10 0\n20 0\n0 VERTEX\n8 P\n70 8\n10 1\n20 1\n0 VERTEX\n8 P\n70 8\n10 2\n20 1\n0 SEQEND",
    ));
    assert!(matches!(&only(&d).kind, DxfKind::Polyline { points, .. } if points.len() == 2));
}

#[test]
fn a_polyface_mesh_gives_a_face_per_record_and_a_3d_mesh_is_skipped() {
    let d = both(&ents(
        "0 POLYLINE\n8 M\n66 1\n70 64\n71 4\n72 2\n\
         0 VERTEX\n8 M\n70 192\n10 0\n20 0\n\
         0 VERTEX\n8 M\n70 192\n10 10\n20 0\n\
         0 VERTEX\n8 M\n70 192\n10 10\n20 10\n\
         0 VERTEX\n8 M\n70 192\n10 0\n20 10\n\
         0 VERTEX\n8 M\n70 128\n71 1\n72 2\n73 3\n74 4\n\
         0 VERTEX\n8 M\n70 128\n71 1\n72 3\n73 -4\n0 SEQEND",
    ));
    assert_eq!(d.entities.len(), 2);
    assert!(matches!(&d.entities[0].kind, DxfKind::Face { points, .. } if points.len() == 4));
    assert!(matches!(&d.entities[1].kind, DxfKind::Face { points, .. } if points.len() == 3));
    let d = both(&ents(
        "0 POLYLINE\n8 M\n66 1\n70 16\n0 VERTEX\n10 0\n20 0\n0 SEQEND",
    ));
    assert!(d.entities.is_empty());
    assert_eq!(d.skipped, vec![("POLYLINE (3D mesh)".to_string(), 1)]);
}

#[test]
fn splines_keep_knots_weights_and_control_or_fit_points() {
    let d = both(&ents(
        "0 SPLINE\n8 S\n70 8\n71 2\n72 6\n73 3\n74 0\n40 0\n40 0\n40 0\n40 1\n40 1\n40 1\n\
         10 0\n20 0\n10 10\n20 20\n10 20\n20 0",
    ));
    match &only(&d).kind {
        DxfKind::Spline {
            degree,
            closed,
            knots,
            control,
            fit,
            weights,
        } => {
            assert_eq!(
                (
                    *degree,
                    *closed,
                    knots.len(),
                    control.len(),
                    fit.len(),
                    weights.len()
                ),
                (2, false, 6, 3, 0, 0)
            );
        }
        other => panic!("{other:?}"),
    }
    let d = both(&ents(
        "0 SPLINE\n8 S\n70 1\n71 3\n74 3\n11 0\n21 0\n11 5\n21 5\n11 10\n21 0",
    ));
    assert!(matches!(&only(&d).kind, DxfKind::Spline { closed: true, fit, .. } if fit.len() == 3));
    let d = both(&ents("0 SPLINE\n8 S\n70 0\n71 3"));
    assert!(d.entities.is_empty());
    assert_eq!(d.skipped, vec![("SPLINE".to_string(), 1)]);
}

#[test]
fn solids_and_three_d_faces_come_out_in_outline_order() {
    let d = both(&ents(
        "0 SOLID\n8 0\n10 0\n20 0\n11 10\n21 0\n12 0\n22 10\n13 10\n23 10",
    ));
    match &only(&d).kind {
        DxfKind::Face { points, filled } => {
            assert!(*filled);
            // SOLID corners run 1, 2, 4, 3.
            assert!(near(points[2], 10.0, 10.0) && near(points[3], 0.0, 10.0));
        }
        other => panic!("{other:?}"),
    }
    let d = both(&ents("0 3DFACE\n8 0\n10 0\n20 0\n30 5\n11 10\n21 0\n31 5\n12 10\n22 10\n32 5\n13 10\n23 10\n33 5"));
    assert!(matches!(&only(&d).kind, DxfKind::Face { points, filled: false } if points.len() == 3));
}

#[test]
fn a_hatch_reads_polyline_and_edge_boundaries_and_the_pattern() {
    // A polyline loop with a bulge, an ANSI31 pattern at 45 degrees.
    let d = both(&ents(
        "0 HATCH\n8 H\n10 0\n20 0\n30 0\n210 0\n220 0\n230 1\n2 ANSI31\n70 0\n71 0\n91 1\n\
         92 3\n72 1\n73 1\n93 4\n10 0\n20 0\n10 100\n20 0\n10 100\n20 100\n10 0\n20 100\n97 0\n\
         75 0\n76 1\n52 45\n41 2\n77 0\n78 1\n53 45\n43 0\n44 0\n45 -0.0883883\n46 0.0883883\n79 0\n98 1\n10 5\n20 5",
    ));
    match &only(&d).kind {
        DxfKind::Hatch(h) => {
            assert_eq!(h.pattern, "ANSI31");
            assert!(!h.solid);
            assert_eq!((h.angle_deg, h.scale), (45.0, 2.0));
            assert_eq!(h.loops.len(), 1);
            assert!(h.loops[0].external);
            assert_eq!(h.loops[0].points.len(), 4);
            // The offset (-0.088, 0.088) is 0.125 across the 45 degree lines; times scale 2.
            assert!((h.spacing.unwrap() - 0.25).abs() < 1e-6, "{:?}", h.spacing);
        }
        other => panic!("{other:?}"),
    }
    // Edge boundary: a line, an arc and a line close a half disc; SOLID fill.
    let d = both(&ents(
        "0 HATCH\n8 H\n10 0\n20 0\n30 0\n2 SOLID\n70 1\n71 0\n91 1\n92 1\n93 3\n\
         72 1\n10 -10\n20 0\n11 10\n21 0\n\
         72 2\n10 0\n20 0\n40 10\n50 0\n51 180\n73 1\n\
         72 1\n10 -10\n20 0\n11 -10\n21 0\n97 0\n75 0\n76 1\n98 0",
    ));
    match &only(&d).kind {
        DxfKind::Hatch(h) => {
            assert!(h.solid);
            let pts = &h.loops[0].points;
            assert!(pts.len() > 10, "{}", pts.len());
            // The arc's samples sit on the circle of radius 10.
            let on_arc = pts
                .iter()
                .filter(|p| (p.length() - 10.0).abs() < 1e-6 && p.y > 0.1)
                .count();
            assert!(on_arc > 5);
        }
        other => panic!("{other:?}"),
    }
    // A hatch with no readable boundary is skipped.
    let d = both(&ents("0 HATCH\n8 H\n2 SOLID\n70 1\n91 0"));
    assert!(d.entities.is_empty());
    assert_eq!(d.skipped, vec![("HATCH".to_string(), 1)]);
}

#[test]
fn text_alignment_styles_and_codes() {
    let d = both(&ents(
        "0 TEXT\n8 T\n10 5\n20 6\n40 2.5\n1 Kitchen %%d\n50 30\n7 NOTES\n41 0.8",
    ));
    match &only(&d).kind {
        DxfKind::Text(t) => {
            assert_eq!(t.text, "Kitchen \u{b0}");
            assert!(near(t.pos, 5.0, 6.0));
            assert_eq!(
                (t.h, t.v, t.height, t.angle_deg),
                (HJust::Left, VJust::Baseline, 2.5, 30.0)
            );
            assert_eq!((t.style.as_str(), t.width_factor), ("NOTES", 0.8));
        }
        other => panic!("{other:?}"),
    }
    // Centered text is anchored on the second point.
    let d = both(&ents(
        "0 TEXT\n8 T\n10 0\n20 0\n11 50\n21 60\n40 4\n1 Hi\n72 1\n73 2",
    ));
    assert!(
        matches!(&only(&d).kind, DxfKind::Text(t) if t.h == HJust::Center && t.v == VJust::Middle && near(t.pos, 50.0, 60.0))
    );
    // Aligned text runs along its two points.
    let d = both(&ents(
        "0 TEXT\n8 T\n10 0\n20 0\n11 10\n21 10\n40 4\n1 Hi\n72 3",
    ));
    assert!(
        matches!(&only(&d).kind, DxfKind::Text(t) if (t.angle_deg - 45.0).abs() < 1e-9 && near(t.pos, 0.0, 0.0))
    );
    // MTEXT: attachment, formatting, a reference width, split into chunks.
    let d = both(&ents(
        "0 MTEXT\n8 T\n10 7\n20 8\n40 3\n41 60\n71 5\n3 {\\fArial|b1;Line one}\n1 \\Pline two\n7 NOTES",
    ));
    match &only(&d).kind {
        DxfKind::Text(t) => {
            assert!(t.mtext);
            assert_eq!(t.text, "Line one\nline two");
            assert_eq!(
                (t.h, t.v, t.wrap_width),
                (HJust::Center, VJust::Middle, 60.0)
            );
            assert!(t.runs[0].bold && !t.runs[1].bold);
        }
        other => panic!("{other:?}"),
    }
    // The x-axis direction beats the rotation angle.
    let d = both(&ents(
        "0 MTEXT\n8 T\n10 0\n20 0\n40 3\n1 Up\n11 0\n21 1\n50 0",
    ));
    assert!(matches!(&only(&d).kind, DxfKind::Text(t) if (t.angle_deg - 90.0).abs() < 1e-9));
}

#[test]
fn dimensions_keep_their_definition_points_and_block() {
    let d = both(&ents(
        "0 DIMENSION\n8 D\n2 *D1\n10 50\n20 20\n11 50\n21 25\n70 32\n1 <>\n3 PLAN\n13 0\n23 0\n14 100\n24 0\n50 0\n42 100",
    ));
    match &only(&d).kind {
        DxfKind::Dimension(m) => {
            assert_eq!(
                (m.dtype, m.block.as_str(), m.style.as_str(), m.measurement),
                (0, "*D1", "PLAN", Some(100.0))
            );
            assert!(near(m.def_pt, 50.0, 20.0) && near(m.p13, 0.0, 0.0) && near(m.p14, 100.0, 0.0));
        }
        other => panic!("{other:?}"),
    }
    // Type bits above the low three are flags.
    let d = both(&ents("0 DIMENSION\n8 D\n70 161\n13 0\n23 0\n14 5\n24 0"));
    assert!(matches!(&only(&d).kind, DxfKind::Dimension(m) if m.dtype == 1));
}

#[test]
fn inserts_carry_attributes_arrays_and_flips() {
    let d = both(&ents(
        "0 INSERT\n8 F\n66 1\n2 chair\n10 200\n20 300\n41 2\n42 2\n50 90\n\
         0 ATTRIB\n8 F\n10 1\n20 2\n40 3\n1 A-12\n2 TAG\n70 0\n\
         0 ATTRIB\n8 F\n10 1\n20 2\n40 3\n1 hidden\n2 SECRET\n70 1\n0 SEQEND",
    ));
    match &only(&d).kind {
        DxfKind::Insert(i) => {
            assert_eq!(
                (i.block.as_str(), i.scale, i.rotation_deg),
                ("chair", (2.0, 2.0), 90.0)
            );
            assert_eq!(i.attribs.len(), 2);
            assert!(
                matches!(&i.attribs[0].kind, DxfKind::Text(t) if t.text == "A-12" && t.tag == "TAG" && !t.invisible)
            );
            assert!(matches!(&i.attribs[1].kind, DxfKind::Text(t) if t.invisible));
        }
        other => panic!("{other:?}"),
    }
    let d = both(&ents(
        "0 INSERT\n8 F\n2 B\n10 0\n20 0\n70 3\n71 2\n44 10\n45 20",
    ));
    assert!(
        matches!(&only(&d).kind, DxfKind::Insert(i) if (i.columns, i.rows, i.col_spacing, i.row_spacing) == (3, 2, 10.0, 20.0))
    );
    // A down extrusion turns into a mirror: rotation 180 - a, y scale flipped.
    let d = both(&ents(
        "0 INSERT\n8 F\n2 B\n10 4\n20 5\n50 30\n210 0\n220 0\n230 -1",
    ));
    assert!(
        matches!(&only(&d).kind, DxfKind::Insert(i) if near(i.pos, -4.0, 5.0) && i.rotation_deg == 150.0 && i.scale == (1.0, -1.0))
    );
}

#[test]
fn attdef_points_and_leaders() {
    let d = both(&dxf(
        "0 SECTION\n2 BLOCKS\n0 BLOCK\n2 TAGGED\n10 0\n20 0\n0 ATTDEF\n8 0\n10 0\n20 0\n40 2\n1 default\n2 NAME\n3 Name?\n70 0\n0 ENDBLK\n0 ENDSEC\n0 EOF",
    ));
    let b = d.block("tagged").unwrap();
    assert!(
        matches!(&b.entities[0].kind, DxfKind::Text(t) if t.attdef && t.tag == "NAME" && t.text == "default")
    );
    let d = both(&ents("0 POINT\n8 P\n10 3\n20 4\n30 5"));
    assert!(matches!(only(&d).kind, DxfKind::Marker { pos } if near(pos, 3.0, 4.0)));
    let d = both(&ents(
        "0 LEADER\n8 L\n71 1\n76 3\n10 0\n20 0\n10 10\n20 10\n10 20\n20 10",
    ));
    assert!(matches!(&only(&d).kind, DxfKind::Leader { points, arrow: true } if points.len() == 3));
    // MULTILEADER: leader lines and the text inside the context data.
    let d = both(&ents(
        "0 MULTILEADER\n8 L\n300 CONTEXT_DATA{\n40 1\n10 30\n20 10\n41 2.5\n302 LEADER{\n10 20\n20 10\n304 LEADER_LINE{\n10 0\n20 0\n10 15\n20 10\n305 }\n303 }\n290 1\n304 Drain here\n12 30\n22 10\n301 }",
    ));
    match &only(&d).kind {
        DxfKind::MLeader(m) => {
            assert_eq!(m.lines.len(), 1);
            assert_eq!(m.lines[0].len(), 2);
            assert_eq!(m.text, "Drain here");
            assert!(near(m.text_pos.unwrap(), 30.0, 10.0));
            assert_eq!(m.height, 2.5);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn entity_colours_weights_and_visibility() {
    let d = both(&ents(
        "0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n62 5\n370 35\n6 HIDDEN\n\
         0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n62 0\n\
         0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n420 255\n\
         0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n60 1\n\
         0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n67 1",
    ));
    // The invisible line is dropped; the paper space line is kept apart.
    assert_eq!(d.entities.len(), 3);
    assert_eq!(d.paper_entities.len(), 1);
    let p = &d.entities[0].props;
    assert_eq!(
        (p.color, p.weight, p.linetype.as_str()),
        (DxfColor::Aci(5), 35, "HIDDEN")
    );
    assert_eq!(d.entities[1].props.color, DxfColor::ByBlock);
    assert_eq!(d.entities[2].props.color, DxfColor::Rgb([0, 0, 255]));
    assert_eq!(d.entities[2].props.weight, WEIGHT_BY_LAYER);
}

#[test]
fn blocks_xrefs_and_paper_space() {
    let src = dxf("0 SECTION
2 BLOCKS
0 BLOCK
8 0
2 *Model_Space
0 ENDBLK
0 BLOCK
8 0
2 *Paper_Space
0 LINE
8 0
10 0
20 0
11 1
21 1
0 ENDBLK
0 BLOCK
8 0
2 *Paper_Space1
0 LINE
8 0
10 0
20 0
11 2
21 2
0 ENDBLK
0 BLOCK
8 0
2 SITE
70 4
1 C:\\refs\\site.dwg
0 ENDBLK
0 BLOCK
8 0
2 *D1
70 1
0 LINE
8 0
10 0
20 0
11 5
21 0
0 ENDBLK
0 ENDSEC
0 EOF");
    let d = both(&src);
    assert_eq!(
        d.paper_entities.len(),
        1,
        "only the first page of paper space"
    );
    assert!(d.paper_entities[0].props.paper);
    assert_eq!(d.xrefs, vec!["C:\\refs\\site.dwg".to_string()]);
    assert!(d.block("SITE").unwrap().entities.is_empty());
    assert!(d.block("*D1").unwrap().anonymous);
    assert!(d.block("*Paper_Space").is_none());
}

#[test]
fn files_the_old_reader_tolerated_still_read() {
    let src = "\u{feff}999\ncomment\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nX\n10\n1\n20\n2\n11\n3\n21\n4\n0\nENDSEC\n0\nEOF\n";
    let d = parse_dxf(src).unwrap();
    assert_eq!(d.entities.len(), 1);
    assert_eq!(d.units, DxfUnits::Unitless);
    // Padded codes and CRLF.
    let crlf = [
        "  0", "SECTION", "  2", "ENTITIES", "  0", "CIRCLE", "  8", "Round", " 10", "1.5", " 20",
        "2.5", " 40", "3.0", "  0", "ENDSEC", "  0", "EOF", "",
    ]
    .join("\r\n");
    let d = parse_dxf(&crlf).unwrap();
    assert!(matches!(&d.entities[0].kind, DxfKind::Circle { radius, .. } if *radius == 3.0));
    // A section that lost its ENDSEC.
    let d = parse_dxf("0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n0\nENDBLK\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n1\n0\nEOF\n").unwrap();
    assert_eq!((d.blocks.len(), d.entities.len()), (1, 1));
}

#[test]
fn what_is_not_dxf_is_refused_and_dwg_gets_guidance() {
    assert_eq!(parse_dxf("hello world"), Err(crate::ImportError::NotDxf));
    assert_eq!(
        parse_dxf("AutoCAD Binary DXF\r\n"),
        Err(crate::ImportError::BinaryDxf)
    );
    assert_eq!(
        parse_dxf_bytes(b"hello world"),
        Err(crate::ImportError::NotDxf)
    );
    let mut dwg = b"AC1027".to_vec();
    dwg.extend_from_slice(&[0, 0, 0, 0, 0, 0x0f, 1, 2, 3]);
    let e = parse_dxf_bytes(&dwg).unwrap_err();
    assert_eq!(e, crate::ImportError::Dwg("AC1027".into()));
    let msg = e.to_string();
    assert!(
        msg.contains("DWG")
            && msg.contains("2013")
            && msg.contains("Save As")
            && msg.contains("DXF"),
        "{msg}"
    );
    assert_eq!(dwg_version(b"AC1009\0\0\0"), Some("AC1009".to_string()));
    assert_eq!(dwg_version(b"  0\nSECTION"), None);
    assert_eq!(release_name("AC1032"), "2018");
}

#[test]
fn a_windows_1252_file_decodes_its_text() {
    let mut bytes = ents("0 TEXT\n8 0\n10 0\n20 0\n40 1\n1 CAFE").into_bytes();
    let at = bytes.windows(4).position(|w| w == b"CAFE").unwrap();
    bytes[at + 3] = 0xe9;
    let d = parse_dxf_bytes(&bytes).unwrap();
    assert!(matches!(&only(&d).kind, DxfKind::Text(t) if t.text == "CAF\u{e9}"));
}

#[test]
fn a_cut_off_binary_file_keeps_the_entities_before_the_cut() {
    let src = ents("0 LINE\n8 A\n10 0\n20 0\n11 1\n21 1\n0 LINE\n8 B\n10 0\n20 0\n11 2\n21 2");
    let bin = tokens::ascii_to_binary(&src);
    let d = parse_dxf_bytes(&bin[..bin.len() - 40]).unwrap();
    assert!(!d.entities.is_empty());
}

#[test]
fn units_and_labels() {
    assert_eq!(DxfUnits::from_code(1), DxfUnits::Inches);
    assert_eq!(DxfUnits::from_code(2), DxfUnits::Feet);
    assert_eq!(DxfUnits::from_code(21), DxfUnits::Feet);
    assert_eq!(DxfUnits::from_code(6), DxfUnits::Meters);
    assert_eq!(DxfUnits::from_code(10), DxfUnits::Yards);
    assert_eq!(DxfUnits::from_code(0), DxfUnits::Unitless);
    assert_eq!(DxfUnits::from_code(99), DxfUnits::Unitless);
    assert_eq!(DxfUnits::Millimeters.label(), "Millimeters");
}
