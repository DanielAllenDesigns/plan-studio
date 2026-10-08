//! Turning plan objects and 3D files into library items: the "Add to
//! Library" side of the User Catalog.
//!
//! Every maker returns a [`CatalogItem`] (with its 2D symbol) and, when the
//! object has one, its 3D [`Model3d`] in the symbol-local import frame (x
//! centered, bottom at `y = 0`, back at `z = 0`, front at `+z`). Registering
//! them is [`super::user::add`].

use crate::editor::placed::placed_symbol_strokes;
use plan_cabinets::Cabinet;
use plan_core::cad::{CadItem, TEXT_WIDTH_FACTOR};
use plan_core::geometry::Point;
use plan_core::PlacedSymbol;
use plan_import::{ImportedModel, ModelOptions};
use plan_library::{
    rules, CatalogItem, ItemKind, Model3d, ModelPart, Placement, Stroke, Symbol2d,
};
use std::f64::consts::PI;

// ----- 3D conversions -----

/// sRGB byte of a linear color channel.
fn srgb(linear: f32) -> u8 {
    let l = linear.clamp(0.0, 1.0);
    let s = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// A model from plan-3d meshes (scene axes), parts colored by material.
pub fn model_from_meshes(meshes: &[plan_3d::Mesh]) -> Model3d {
    let parts = meshes
        .iter()
        .map(|m| {
            let c = m.material.color();
            ModelPart {
                name: m.material.name().to_string(),
                color: Some([srgb(c[0]), srgb(c[1]), srgb(c[2])]),
                positions: m.vertices.iter().map(|v| v.position).collect(),
                indices: m.indices.clone(),
            }
        })
        .collect();
    Model3d { parts }.cleaned()
}

/// A model from an imported file (already converted to inches and the import
/// frame), moved to the symbol-local frame.
pub fn model_from_import(imported: &ImportedModel) -> Model3d {
    let parts = imported
        .parts
        .iter()
        .map(|p| ModelPart {
            name: p.name.clone(),
            color: p.color,
            positions: p.positions.clone(),
            indices: p.indices.clone(),
        })
        .collect();
    Model3d { parts }.cleaned().normalized()
}

/// Parses a 3D file's `bytes` (`obj`, `gltf` or `glb`, by `ext`) with `opts`.
pub fn parse_model(
    ext: &str,
    bytes: &[u8],
    mtl: Option<&str>,
    resolve: Option<plan_import::gltf::Resolver>,
    opts: &ModelOptions,
) -> Result<ImportedModel, String> {
    let r = match ext.to_ascii_lowercase().as_str() {
        "obj" => {
            let text = String::from_utf8_lossy(bytes);
            plan_import::obj::parse_obj(&text, mtl, opts)
        }
        "gltf" | "glb" => plan_import::gltf::parse_gltf(bytes, resolve, opts),
        other => {
            return Err(format!(
                "Cannot import .{other} files; use OBJ, glTF (.gltf) or binary glTF (.glb)"
            ))
        }
    };
    r.map_err(|e| e.0)
}

// ----- 2D symbols -----

fn rectangle(w: f64, d: f64, y0: f64) -> Stroke {
    let (hw, y1) = (w * 0.5, y0 + d);
    Stroke::Polyline {
        points: vec![
            Point::new(-hw, y0),
            Point::new(hw, y0),
            Point::new(hw, y1),
            Point::new(-hw, y1),
        ],
        closed: true,
    }
}

/// The plan symbol of a 3D model, projected from above with the
/// hidden-line projection of `plan-calib` (outline, creases and silhouette
/// edges). Wall-mounted items are drawn from their back-center, the others
/// about their center. An empty projection falls back to the footprint
/// rectangle, so the symbol is never empty for a model with area.
pub fn silhouette_symbol(model: &Model3d, placement: Placement) -> Symbol2d {
    let Some(ext) = model.extent() else {
        return Symbol2d::default();
    };
    let (w, d) = (ext[0] as f64, ext[2] as f64);
    // Scene (x, y up, z front) -> Chief (x, y back, z up) = (x, -z, y).
    let meshes: Vec<plan_calib::decode::Mesh> = model
        .parts
        .iter()
        .map(|p| plan_calib::decode::Mesh {
            vertices: p
                .positions
                .iter()
                .map(|q| [q[0] as f64, -(q[2] as f64), q[1] as f64])
                .collect(),
            triangles: p.indices.as_chunks::<3>().0.to_vec(),
        })
        .collect();
    let refs: Vec<&plan_calib::decode::Mesh> = meshes.iter().collect();
    let view = plan_calib::decode::plan_view(&refs, plan_calib::decode::PlanOptions::default());
    let mut sym = match view {
        Some(v) if !v.strokes.is_empty() => Symbol2d::new(v.strokes),
        _ => Symbol2d::new(vec![rectangle(w.max(1.0), d.max(1.0), -d.max(1.0) * 0.5)]),
    };
    if placement == Placement::WallMounted {
        sym = sym.transformed(Point::new(0.0, d * 0.5), 0.0, 1.0);
    }
    sym
}

fn cad_stroke(item: &CadItem) -> Option<Stroke> {
    Some(match item {
        CadItem::Line { a, b } => Stroke::Polyline {
            points: vec![*a, *b],
            closed: false,
        },
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Stroke::Arc {
            center: *center,
            radius: *radius,
            start_deg: start_angle.to_degrees(),
            end_deg: start_angle.to_degrees() + (end_angle - start_angle).rem_euclid(2.0 * PI).to_degrees(),
        },
        CadItem::Circle { center, radius } => Stroke::Circle {
            center: *center,
            radius: *radius,
        },
        CadItem::Polyline { points, closed } => Stroke::Polyline {
            points: points.clone(),
            closed: *closed,
        },
        CadItem::Text { .. } => return None,
    })
}

/// The box a text note occupies, as a closed outline with an underline.
fn text_strokes(pos: Point, text: &str, height: f64, angle: f64) -> Vec<Stroke> {
    let w = text.chars().count().max(1) as f64 * height * TEXT_WIDTH_FACTOR;
    let (c, s) = (angle.cos(), angle.sin());
    let at = |dx: f64, dy: f64| Point::new(pos.x + dx * c - dy * s, pos.y + dx * s + dy * c);
    vec![
        Stroke::Polyline {
            points: vec![at(0.0, 0.0), at(w, 0.0), at(w, height), at(0.0, height)],
            closed: true,
        },
        Stroke::Polyline {
            points: vec![at(0.0, height * 0.2), at(w, height * 0.2)],
            closed: false,
        },
    ]
}

// ----- the item makers -----

fn finish(
    mut item: CatalogItem,
    id: &str,
    name: &str,
    folder: &[String],
) -> CatalogItem {
    item.id = id.to_string();
    item.name = name.to_string();
    item.category = folder.to_vec();
    item
}

/// A placed symbol saved as a library item: its size, elevation, flip and
/// (when the library item has one) its 3D model. Chief Architect objects
/// are licensed content and cannot be saved; pictures are saved with
/// Create Image Library.
pub fn item_from_symbol(
    sym: &PlacedSymbol,
    id: &str,
    folder: &[String],
) -> Result<(CatalogItem, Option<Model3d>), String> {
    if crate::tools::library::chief::is_chief_id(&sym.catalog_id) {
        return Err(
            "Chief Architect objects stay in Chief's catalog and cannot be saved in the User \
             Catalog"
                .into(),
        );
    }
    if sym.image.is_some() || sym.distribution.is_some() {
        return Err("Pictures and distributions are saved with Create Image Library".into());
    }
    let src = crate::tools::library::find_item(&sym.catalog_id)
        .ok_or_else(|| format!("The library item {} is not available", sym.catalog_id))?;
    let mut local = sym.clone();
    local.position = Point::ZERO;
    local.angle = 0.0;
    let strokes = placed_symbol_strokes(&local)
        .ok_or_else(|| "The symbol has no drawing".to_string())?;
    // Free-standing symbols are drawn about their center.
    let strokes = if src.placement == Placement::WallMounted {
        strokes
    } else {
        strokes.transformed(Point::new(0.0, -sym.depth * 0.5), 0.0, 1.0)
    };
    let model = crate::tools::library::user::model_of(&src);
    let name = if sym.label.trim().is_empty() {
        src.name.clone()
    } else {
        sym.label.trim().to_string()
    };
    let mut item = (*src).clone();
    item.symbol = strokes;
    item.width = sym.width;
    item.depth = sym.depth;
    item.height = sym.height;
    item.elevation = sym.elevation;
    if item.model3d.is_some() && model.is_none() {
        item.model3d = None;
    }
    Ok((finish(item, id, &name, folder), model.map(|m| (*m).clone())))
}

/// A library item copied into the user catalog (customize a built-in
/// item). Chief Architect objects cannot be copied.
pub fn item_from_item(
    src: &CatalogItem,
    id: &str,
    folder: &[String],
) -> Result<(CatalogItem, Option<Model3d>), String> {
    if crate::tools::library::chief::is_chief_id(&src.id) {
        return Err(
            "Chief Architect objects stay in Chief's catalog and cannot be saved in the User \
             Catalog"
                .into(),
        );
    }
    let model = crate::tools::library::user::model_of(src).map(|m| (*m).clone());
    let mut item = src.clone();
    if model.is_none() {
        item.model3d = None;
    }
    let name = src.name.clone();
    Ok((finish(item, id, &name, folder), model))
}

/// A cabinet saved as a library item: its outline, its 3D model and the
/// cabinet itself (position and angle cleared) as the payload, so placing the
/// item makes a real cabinet again.
pub fn item_from_cabinet(
    cab: &Cabinet,
    id: &str,
    folder: &[String],
) -> Result<(CatalogItem, Model3d), String> {
    let mut saved = cab.clone();
    saved.id = 0;
    saved.position = Point::ZERO;
    saved.angle = 0.0;
    let payload = serde_json::to_value(&saved).map_err(|e| e.to_string())?;
    // The mesh at the origin with the elevation left to the item.
    let mut flat = saved.clone();
    flat.elevation = 0.0;
    let meshes = plan_cabinets::meshes(&flat);
    // Scene frame: back-left corner at the origin, front towards -z. Turn it
    // to front +z and center it on x.
    let model = model_from_meshes(&meshes)
        .rotated_y(180.0)
        .translated([(cab.width * 0.5) as f32, 0.0, 0.0]);
    let outline: Vec<Point> = saved
        .footprint_local()
        .into_iter()
        .map(|p| Point::new(p.x - cab.width * 0.5, p.y))
        .collect();
    let symbol = Symbol2d::new(vec![Stroke::Polyline {
        points: outline,
        closed: true,
    }]);
    let kind = format!("{:?}", cab.kind).to_lowercase();
    let mut item = CatalogItem::new(id, "Cabinet", Placement::WallMounted, symbol)
        .with_size(cab.width, cab.depth, cab.height)
        .with_elevation(cab.elevation)
        .with_tags(&["cabinet", &kind]);
    item.kind = ItemKind::Cabinet;
    item.payload = Some(payload);
    let name = if cab.label.trim().is_empty() {
        cab.display_label()
    } else {
        cab.label.clone()
    };
    Ok((finish(item, id, &name, folder), model))
}

fn shifted(item: &CadItem, d: Point) -> CadItem {
    let mv = |p: &Point| Point::new(p.x + d.x, p.y + d.y);
    match item {
        CadItem::Line { a, b } => CadItem::Line { a: mv(a), b: mv(b) },
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => CadItem::Arc {
            center: mv(center),
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        CadItem::Circle { center, radius } => CadItem::Circle {
            center: mv(center),
            radius: *radius,
        },
        CadItem::Polyline { points, closed } => CadItem::Polyline {
            points: points.iter().map(mv).collect(),
            closed: *closed,
        },
        CadItem::Text {
            pos,
            text,
            height,
            angle,
        } => CadItem::Text {
            pos: mv(pos),
            text: text.clone(),
            height: *height,
            angle: *angle,
        },
    }
}

/// `item` turned by `angle` (radians) about the origin and moved by `to`.
pub fn placed_cad(item: &CadItem, angle: f64, to: Point) -> CadItem {
    let (s, c) = angle.sin_cos();
    let rot = |p: &Point| Point::new(p.x * c - p.y * s + to.x, p.x * s + p.y * c + to.y);
    match item {
        CadItem::Line { a, b } => CadItem::Line { a: rot(a), b: rot(b) },
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => CadItem::Arc {
            center: rot(center),
            radius: *radius,
            start_angle: start_angle + angle,
            end_angle: end_angle + angle,
        },
        CadItem::Circle { center, radius } => CadItem::Circle {
            center: rot(center),
            radius: *radius,
        },
        CadItem::Polyline { points, closed } => CadItem::Polyline {
            points: points.iter().map(rot).collect(),
            closed: *closed,
        },
        CadItem::Text {
            pos,
            text,
            height,
            angle: a,
        } => CadItem::Text {
            pos: rot(pos),
            text: text.clone(),
            height: *height,
            angle: a + angle,
        },
    }
}

/// CAD objects saved as a block (lines, arcs, circles, polylines) or, when
/// they are all text, as a text item. The items are re-centered on the
/// middle of their bounds and kept as the payload.
pub fn item_from_cad(
    items: &[CadItem],
    id: &str,
    name: &str,
    folder: &[String],
) -> Result<CatalogItem, String> {
    if items.is_empty() {
        return Err("There is nothing to save".into());
    }
    let pts = items.iter().flat_map(|i| {
        let (lo, hi) = i.bounds();
        [lo, hi]
    });
    let bounds = plan_library::Bounds::from_points(pts).ok_or("There is nothing to save")?;
    let c = bounds.center();
    let local: Vec<CadItem> = items.iter().map(|i| shifted(i, Point::new(-c.x, -c.y))).collect();
    let all_text = local.iter().all(|i| matches!(i, CadItem::Text { .. }));
    let mut strokes = Vec::new();
    for i in &local {
        match i {
            CadItem::Text {
                pos,
                text,
                height,
                angle,
            } => strokes.extend(text_strokes(*pos, text, *height, *angle)),
            other => strokes.extend(cad_stroke(other)),
        }
    }
    let kind = if all_text { ItemKind::Text } else { ItemKind::CadBlock };
    let tag = if all_text { "text" } else { "cad" };
    let payload = serde_json::to_value(&local).map_err(|e| e.to_string())?;
    let mut item = CatalogItem::new(id, name, Placement::FreeStanding, Symbol2d::new(strokes))
        .with_size(bounds.width(), bounds.height(), 0.0)
        .with_tags(&[tag, "annotation"]);
    item.kind = kind;
    item.payload = Some(payload);
    Ok(finish(item, id, name, folder))
}

/// A material saved as a swatch item (a 12 x 12 inch tile).
pub fn item_from_material(
    name: &str,
    color: [u8; 3],
    id: &str,
    folder: &[String],
) -> (CatalogItem, Model3d) {
    let sq = 12.0;
    let h = sq * 0.5;
    let symbol = Symbol2d::new(vec![
        rectangle(sq, sq, -h),
        Stroke::Polyline {
            points: vec![Point::new(-h, -h), Point::new(h, h)],
            closed: false,
        },
        Stroke::Polyline {
            points: vec![Point::new(h, -h), Point::new(-h, h)],
            closed: false,
        },
    ]);
    let mut item = CatalogItem::new(id, name, Placement::FreeStanding, symbol)
        .with_size(sq, sq, 1.0)
        .with_tags(&["material", "swatch", &format!("material:{name}")]);
    item.kind = ItemKind::Material;
    item.payload = Some(serde_json::json!({ "material": name, "color": color }));
    let model = Model3d::box_model(sq as f32, sq as f32, 1.0, Some(color));
    (finish(item, id, name, folder), model)
}

/// An imported 3D model as a library item with a generated plan symbol.
/// `placement` defaults to what the category and name suggest.
pub fn item_from_model(
    model: &Model3d,
    id: &str,
    name: &str,
    folder: &[String],
    placement: Option<Placement>,
) -> Result<CatalogItem, String> {
    let ext = model.extent().ok_or("The model has no triangles")?;
    let placement = placement.unwrap_or_else(|| rules::placement_for_category(folder, name));
    let symbol = silhouette_symbol(model, placement);
    let mut item = CatalogItem::new(id, name, placement, symbol)
        .with_size(ext[0] as f64, ext[2] as f64, ext[1] as f64)
        .with_tags(&["3d", "imported"]);
    item.kind = ItemKind::Model;
    Ok(finish(item, id, name, folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_box_model_gets_a_nonempty_silhouette_that_matches_its_footprint() {
        let m = Model3d::box_model(36.0, 24.0, 30.0, None);
        for placement in [Placement::FreeStanding, Placement::WallMounted] {
            let s = silhouette_symbol(&m, placement);
            assert!(!s.is_empty(), "{placement:?}");
            let b = s.bounds().unwrap();
            assert!((b.width() - 36.0).abs() < 1.0, "{}", b.width());
            assert!((b.height() - 24.0).abs() < 1.0, "{}", b.height());
            let center_y = (b.min.y + b.max.y) * 0.5;
            if placement == Placement::WallMounted {
                assert!((b.min.y).abs() < 1.0, "back edge on y = 0: {}", b.min.y);
            } else {
                assert!(center_y.abs() < 1.0);
            }
        }
        assert!(silhouette_symbol(&Model3d::default(), Placement::FreeStanding).is_empty());
    }

    #[test]
    fn a_flat_model_falls_back_to_the_footprint_rectangle() {
        // A single triangle standing on edge has no plan area.
        let tri = Model3d::single(ModelPart {
            name: String::new(),
            color: None,
            positions: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]],
            indices: vec![0, 1, 2],
        });
        let s = silhouette_symbol(&tri, Placement::FreeStanding);
        assert!(!s.is_empty());
    }

    #[test]
    fn cad_blocks_and_text_keep_their_items() {
        let items = vec![
            CadItem::Line {
                a: Point::new(100.0, 100.0),
                b: Point::new(140.0, 100.0),
            },
            CadItem::Circle {
                center: Point::new(120.0, 120.0),
                radius: 10.0,
            },
        ];
        let it = item_from_cad(&items, "user.cad.1", "Mark", &folder(&["User", "Blocks"])).unwrap();
        assert_eq!(it.kind, ItemKind::CadBlock);
        assert_eq!(it.category, folder(&["User", "Blocks"]));
        assert!(!it.symbol.is_empty());
        let back: Vec<CadItem> = serde_json::from_value(it.payload.clone().unwrap()).unwrap();
        assert_eq!(back.len(), 2);
        // Re-centered on the middle of the bounds (120, 115).
        match &back[0] {
            CadItem::Line { a, .. } => assert_eq!((a.x, a.y), (-20.0, -15.0)),
            _ => unreachable!(),
        }
        let t = item_from_cad(
            &[CadItem::Text {
                pos: Point::new(0.0, 0.0),
                text: "NOTE".into(),
                height: 4.0,
                angle: 0.0,
            }],
            "user.text.1",
            "Note",
            &folder(&["User", "Text"]),
        )
        .unwrap();
        assert_eq!(t.kind, ItemKind::Text);
        assert!(t.width > 0.0 && !t.symbol.is_empty());
        assert!(item_from_cad(&[], "x", "x", &folder(&["User", "x"])).is_err());
    }

    #[test]
    fn placed_cad_turns_and_moves() {
        let l = CadItem::Line {
            a: Point::new(1.0, 0.0),
            b: Point::new(2.0, 0.0),
        };
        match placed_cad(&l, std::f64::consts::FRAC_PI_2, Point::new(10.0, 10.0)) {
            CadItem::Line { a, b } => {
                assert!((a.x - 10.0).abs() < 1e-9 && (a.y - 11.0).abs() < 1e-9);
                assert!((b.x - 10.0).abs() < 1e-9 && (b.y - 12.0).abs() < 1e-9);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn cabinets_save_their_outline_model_and_payload() {
        let mut cab = Cabinet::base(36.0);
        cab.position = Point::new(50.0, 80.0);
        cab.angle = 0.5;
        cab.id = 9;
        let (item, model) = item_from_cabinet(&cab, "user.cabinet.1", &folder(&["User", "Cabinets"]))
            .unwrap();
        assert_eq!(item.kind, ItemKind::Cabinet);
        assert_eq!((item.width, item.depth), (36.0, cab.depth));
        let saved: Cabinet = serde_json::from_value(item.payload.clone().unwrap()).unwrap();
        assert_eq!((saved.id, saved.angle, saved.position), (0, 0.0, Point::ZERO));
        // The model is centered on x and its front is +z.
        let (lo, hi) = model.bounds().unwrap();
        assert!(((lo[0] + hi[0]) * 0.5).abs() < 0.5, "{lo:?} {hi:?}");
        assert!(lo[2] > -1.0 && hi[2] > cab.depth as f32 - 1.0, "{lo:?} {hi:?}");
        let b = item.symbol.bounds().unwrap();
        assert!((b.min.x + 18.0).abs() < 1e-9 && b.min.y.abs() < 1e-9);
    }

    #[test]
    fn materials_are_swatches() {
        let (it, m) = item_from_material("Slate", [60, 70, 80], "user.material.1", &folder(&["User", "Materials"]));
        assert_eq!(it.kind, ItemKind::Material);
        assert!(it.tags.contains(&"material:Slate".to_string()));
        assert_eq!(m.triangle_count(), 12);
    }

    #[test]
    fn model_items_take_their_size_and_a_placement_from_the_category() {
        let m = Model3d::box_model(20.0, 10.0, 60.0, None);
        let it = item_from_model(&m, "user.model.1", "Pendant", &folder(&["User", "Lighting"]), None).unwrap();
        assert_eq!((it.width, it.depth, it.height), (20.0, 10.0, 60.0));
        assert_eq!(it.kind, ItemKind::Model);
        assert_eq!(it.placement, Placement::Ceiling);
        let fixed = item_from_model(&m, "user.model.2", "Thing", &folder(&["User"]), Some(Placement::WallMounted)).unwrap();
        assert_eq!(fixed.placement, Placement::WallMounted);
        assert!(item_from_model(&Model3d::default(), "x", "x", &folder(&["User"]), None).is_err());
    }

    #[test]
    fn unknown_extensions_are_refused() {
        let e = parse_model("stl", b"", None, None, &ModelOptions::default()).unwrap_err();
        assert!(e.contains(".stl"));
    }
}
