//! Landscape objects of the terrain in the plan: drawing on their own layers,
//! the 3D meshes, and the helpers the Terrain tool and its specification dialog
//! use to read, replace and move one element (breaks, walls and curbs, terrain
//! features, garden beds, grass, water, stepping stones, plants, sprinklers,
//! roads and elevation lines).

use super::{draw_label, sc, terrain_view, TerrainHit};
use crate::editor::{Camera, EditorContext};
use eframe::egui::{self, Color32, Shape};
use plan_core::{Layer, Point, Project};
use plan_terrain::{
    landscape::{
        LAYER_BEDS, LAYER_BREAKS, LAYER_FEATURES, LAYER_GRASS, LAYER_PLANTS, LAYER_SPRINKLERS,
        LAYER_STONES, LAYER_WALLS, LAYER_WATER,
    },
    landscape_meshes, ElevationLine, Feature, Landscape, PlanItem, PlanShape, RoadStrip, Terrain,
    TerrainBreak, TerrainWall,
};

/// An element of the terrain that has its own specification dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainObject {
    Feature(Feature),
    Break(TerrainBreak),
    Wall(TerrainWall),
    Landscape(Landscape),
    Road(RoadStrip),
    Line(ElevationLine),
}

impl TerrainObject {
    /// The title of its specification.
    pub fn title(&self) -> &'static str {
        match self {
            TerrainObject::Feature(_) => "Terrain Feature Specification",
            TerrainObject::Break(_) => "Terrain Break Specification",
            TerrainObject::Wall(w) if w.kind == plan_terrain::WallKind::Curb => {
                "Terrain Curb Specification"
            }
            TerrainObject::Wall(_) => "Terrain Wall Specification",
            TerrainObject::Landscape(l) => match l.kind {
                plan_terrain::LandscapeKind::GardenBed => "Garden Bed Specification",
                plan_terrain::LandscapeKind::GrassRegion => "Grass Region Specification",
                plan_terrain::LandscapeKind::WaterFeature => "Water Feature Specification",
                plan_terrain::LandscapeKind::SteppingStones => "Stepping Stone Specification",
                plan_terrain::LandscapeKind::Plants => "Plant Specification",
                plan_terrain::LandscapeKind::Sprinklers => "Sprinkler Specification",
            },
            TerrainObject::Road(_) => "Road Specification",
            TerrainObject::Line(_) => "Elevation Line Specification",
        }
    }
}

/// The element a hit names, as an editable copy (`None` for elements without
/// a dialog of their own: the perimeter, points, regions and modifiers).
pub fn object_at(t: &Terrain, hit: TerrainHit) -> Option<TerrainObject> {
    match hit {
        TerrainHit::Feature(i) => t
            .features
            .get(i)
            .filter(|f| f.kind != plan_terrain::FeatureKind::Hole)
            .cloned()
            .map(TerrainObject::Feature),
        TerrainHit::Break(i) => t.breaks.get(i).cloned().map(TerrainObject::Break),
        TerrainHit::Wall(i) => t.walls.get(i).cloned().map(TerrainObject::Wall),
        TerrainHit::Landscape(i) => t.landscape.get(i).cloned().map(TerrainObject::Landscape),
        TerrainHit::Road(i) => t.roads.get(i).cloned().map(TerrainObject::Road),
        TerrainHit::Line(i) => t.elevation_lines.get(i).cloned().map(TerrainObject::Line),
        _ => None,
    }
}

/// Stores `obj` over the element `hit` names; false when the two do not match.
pub fn replace_object(t: &mut Terrain, hit: TerrainHit, obj: TerrainObject) -> bool {
    fn put<T>(v: &mut [T], i: usize, x: T) -> bool {
        v.get_mut(i).map(|slot| *slot = x).is_some()
    }
    match (hit, obj) {
        (TerrainHit::Feature(i), TerrainObject::Feature(x)) => put(&mut t.features, i, x),
        (TerrainHit::Break(i), TerrainObject::Break(x)) => put(&mut t.breaks, i, x),
        (TerrainHit::Wall(i), TerrainObject::Wall(x)) => put(&mut t.walls, i, x),
        (TerrainHit::Landscape(i), TerrainObject::Landscape(x)) => put(&mut t.landscape, i, x),
        (TerrainHit::Road(i), TerrainObject::Road(x)) => put(&mut t.roads, i, x),
        (TerrainHit::Line(i), TerrainObject::Line(x)) => put(&mut t.elevation_lines, i, x),
        _ => false,
    }
}

/// Moves the element `hit` names by `delta` inches; false when it is gone.
pub fn move_terrain_element(t: &mut Terrain, hit: TerrainHit, delta: Point) -> bool {
    fn shift(pts: &mut [Point], d: Point) {
        for p in pts {
            *p = *p + d;
        }
    }
    match hit {
        TerrainHit::Perimeter => {
            shift(&mut t.perimeter, delta);
            !t.perimeter.is_empty()
        }
        TerrainHit::Point(i) => t
            .elevation_points
            .get_mut(i)
            .map(|e| e.pos = e.pos + delta)
            .is_some(),
        TerrainHit::Line(i) => t
            .elevation_lines
            .get_mut(i)
            .map(|l| shift(&mut l.points, delta))
            .is_some(),
        TerrainHit::Region(i) => t
            .elevation_regions
            .get_mut(i)
            .map(|r| shift(&mut r.polygon, delta))
            .is_some(),
        TerrainHit::Modifier(i) => t
            .modifiers
            .get_mut(i)
            .map(|m| shift(&mut m.polygon, delta))
            .is_some(),
        TerrainHit::Feature(i) => t
            .features
            .get_mut(i)
            .map(|f| shift(&mut f.polygon, delta))
            .is_some(),
        TerrainHit::Road(i) => t
            .roads
            .get_mut(i)
            .map(|r| shift(&mut r.centerline, delta))
            .is_some(),
        TerrainHit::Break(i) => t
            .breaks
            .get_mut(i)
            .map(|b| shift(&mut b.points, delta))
            .is_some(),
        TerrainHit::Wall(i) => t
            .walls
            .get_mut(i)
            .map(|w| shift(&mut w.points, delta))
            .is_some(),
        TerrainHit::Landscape(i) => t
            .landscape
            .get_mut(i)
            .map(|l| shift(&mut l.points, delta))
            .is_some(),
    }
}

/// Adds the Chief layers of the landscape objects to the plan's layer list
/// when they are missing (no undo step of their own: call it inside the edit).
pub fn ensure_landscape_layers(project: &mut Project) {
    const LAYERS: [(&str, [u8; 3], u32); 9] = [
        (LAYER_FEATURES, [60, 127, 168], 18),
        (LAYER_BREAKS, [176, 64, 48], 13),
        (LAYER_WALLS, [96, 96, 102], 25),
        (LAYER_BEDS, [139, 90, 43], 18),
        (LAYER_GRASS, [79, 143, 58], 13),
        (LAYER_WATER, [47, 127, 192], 18),
        (LAYER_STONES, [122, 119, 112], 18),
        (LAYER_PLANTS, [46, 125, 50], 18),
        (LAYER_SPRINKLERS, [30, 136, 229], 13),
    ];
    for (name, color, weight) in LAYERS {
        project.layers.add(Layer::new(name, color, weight));
    }
}

/// The landscape meshes of the project's terrain, draped on its built surface
/// (flat ground when the terrain is not built). Walls, curbs, feature slabs,
/// garden beds, grass, water, stepping stones, plants and sprinkler heads; the
/// 3D scene adds them next to the terrain surface and roads.
pub fn terrain_feature_meshes(project: &Project) -> Vec<plan_3d::Mesh> {
    let Some(view) = terrain_view(project) else {
        return Vec::new();
    };
    landscape_meshes(&view.record.terrain, view.surface.as_ref())
}

fn color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Draws the landscape plan items, each on its own layer.
pub fn draw_landscape(
    cx: &EditorContext,
    painter: &egui::Painter,
    cam: &Camera,
    items: &[PlanItem],
) {
    let layers = cx.layers();
    for item in items {
        if !layers.is_visible(&item.layer) {
            continue;
        }
        match &item.shape {
            PlanShape::Polyline {
                points,
                closed,
                color: c,
                weight,
                dashed,
            } => {
                let stroke = egui::Stroke::new(((*weight as f32) * 1.6).max(0.8), color(*c));
                super::draw_polyline(painter, cam, points, *closed, stroke, *dashed);
            }
            PlanShape::Fill {
                triangles,
                color: c,
            } => {
                let fill = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                let mut mesh = egui::Mesh::default();
                for tri in triangles {
                    let base = mesh.vertices.len() as u32;
                    for p in tri {
                        mesh.colored_vertex(sc(cam, *p), fill);
                    }
                    mesh.add_triangle(base, base + 1, base + 2);
                }
                painter.add(Shape::mesh(mesh));
            }
            PlanShape::Text {
                at,
                text,
                height,
                color: c,
            } => draw_label(painter, cam, *at, text, *height, color(*c)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::site_view::{
        edit_terrain, hit_terrain, load_terrain, remove_terrain_element, save_terrain,
        TerrainRecord,
    };
    use crate::plan_defaults;
    use plan_terrain::{LandscapeKind, ShapeKind, WallKind};

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![pt(x, y), pt(x + s, y), pt(x + s, y + s), pt(x, y + s)]
    }

    fn lot() -> Terrain {
        let mut t = Terrain::default();
        t.walls.push(TerrainWall::new(
            WallKind::Wall,
            vec![pt(0.0, 50.0), pt(300.0, 50.0)],
            false,
        ));
        t.breaks.push(TerrainBreak {
            points: vec![pt(0.0, 400.0), pt(300.0, 400.0)],
            z: 24.0,
            ..TerrainBreak::default()
        });
        t.features.push(Feature {
            polygon: square(500.0, 500.0, 100.0),
            ..Feature::default()
        });
        for (kind, pts) in [
            (LandscapeKind::GardenBed, square(100.0, 100.0, 100.0)),
            (
                LandscapeKind::SteppingStones,
                vec![pt(0.0, 700.0), pt(300.0, 700.0)],
            ),
        ] {
            t.landscape
                .push(Landscape::new(kind, ShapeKind::Polyline, pts));
        }
        t
    }

    #[test]
    fn hits_find_walls_breaks_beds_and_paths() {
        let t = lot();
        let tol = 4.0;
        assert_eq!(
            hit_terrain(&t, pt(150.0, 52.0), tol),
            Some(TerrainHit::Wall(0))
        );
        assert_eq!(
            hit_terrain(&t, pt(150.0, 401.0), tol),
            Some(TerrainHit::Break(0))
        );
        // A bed is picked by its edge and by its inside.
        assert_eq!(
            hit_terrain(&t, pt(100.0, 150.0), tol),
            Some(TerrainHit::Landscape(0))
        );
        assert_eq!(
            hit_terrain(&t, pt(150.0, 150.0), tol),
            Some(TerrainHit::Landscape(0))
        );
        assert_eq!(
            hit_terrain(&t, pt(150.0, 702.0), tol),
            Some(TerrainHit::Landscape(1))
        );
        // A feature by its inside; the empty lot is not a hit.
        assert_eq!(
            hit_terrain(&t, pt(550.0, 550.0), tol),
            Some(TerrainHit::Feature(0))
        );
        assert_eq!(hit_terrain(&t, pt(900.0, 900.0), tol), None);
    }

    #[test]
    fn elements_move_and_go() {
        let mut t = lot();
        for hit in [
            TerrainHit::Wall(0),
            TerrainHit::Break(0),
            TerrainHit::Feature(0),
            TerrainHit::Landscape(0),
            TerrainHit::Landscape(1),
        ] {
            assert!(move_terrain_element(&mut t, hit, pt(10.0, -5.0)), "{hit:?}");
        }
        assert_eq!(t.walls[0].points[0], pt(10.0, 45.0));
        assert_eq!(t.breaks[0].points[1], pt(310.0, 395.0));
        assert_eq!(t.features[0].polygon[0], pt(510.0, 495.0));
        assert_eq!(t.landscape[0].points[0], pt(110.0, 95.0));
        assert!(!move_terrain_element(
            &mut t,
            TerrainHit::Wall(4),
            pt(1.0, 1.0)
        ));
        for hit in [
            TerrainHit::Landscape(1),
            TerrainHit::Landscape(0),
            TerrainHit::Feature(0),
            TerrainHit::Break(0),
            TerrainHit::Wall(0),
        ] {
            assert!(remove_terrain_element(&mut t, hit), "{hit:?}");
        }
        assert!(t.walls.is_empty() && t.landscape.is_empty() && t.breaks.is_empty());
    }

    #[test]
    fn objects_are_read_and_replaced_by_hit() {
        let mut t = lot();
        let TerrainObject::Wall(mut w) = object_at(&t, TerrainHit::Wall(0)).unwrap() else {
            panic!("a wall")
        };
        w.height = 60.0;
        assert!(replace_object(
            &mut t,
            TerrainHit::Wall(0),
            TerrainObject::Wall(w)
        ));
        assert_eq!(t.walls[0].height, 60.0);
        // A mismatch changes nothing; holes and perimeter have no dialog.
        let b = object_at(&t, TerrainHit::Break(0)).unwrap();
        assert!(!replace_object(&mut t, TerrainHit::Wall(0), b));
        assert!(object_at(&t, TerrainHit::Perimeter).is_none());
        assert!(object_at(&t, TerrainHit::Wall(9)).is_none());
        t.features[0].kind = plan_terrain::FeatureKind::Hole;
        assert!(object_at(&t, TerrainHit::Feature(0)).is_none());
        assert!(TerrainObject::Break(TerrainBreak::default())
            .title()
            .contains("Break"));
    }

    #[test]
    fn layers_are_added_once() {
        let mut cx = cx();
        ensure_landscape_layers(&mut cx.project);
        let n = cx.project.layers.layers.len();
        ensure_landscape_layers(&mut cx.project);
        assert_eq!(cx.project.layers.layers.len(), n);
        for name in [
            "Terrain, Features",
            "Landscaping, Garden Beds",
            "Plants",
            "Sprinklers",
        ] {
            assert!(cx.project.layers.get(name).is_some(), "{name}");
        }
    }

    fn paint(cx: &EditorContext) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, painter) =
                    ui.allocate_painter(egui::Vec2::new(800.0, 600.0), egui::Sense::hover());
                let mut cam = Camera::default_view();
                cam.rect = painter.clip_rect();
                for scale in [0.05, 0.5, 2.0] {
                    cam.px_per_in = scale;
                    crate::editor::site_view::draw_site(cx, &painter, &cam);
                }
            });
        });
    }

    #[test]
    fn the_landscape_draws_and_respects_its_layers() {
        let mut cx = cx();
        let mut rec = TerrainRecord::new();
        rec.terrain = lot();
        rec.terrain.landscape.push(Landscape::new(
            LandscapeKind::Sprinklers,
            ShapeKind::Polyline,
            vec![pt(0.0, 800.0), pt(400.0, 800.0)],
        ));
        save_terrain(&mut cx.project, &rec);
        let view = terrain_view(&cx.project).unwrap();
        let on = |layer: &str| view.landscape.iter().filter(|i| i.layer == layer).count();
        assert!(
            on("Landscaping, Garden Beds") > 0 && on("Sprinklers") > 0 && on("Terrain, Walls") > 0
        );
        paint(&cx);
        // Hidden layers draw nothing (and nothing panics).
        ensure_landscape_layers(&mut cx.project);
        for name in ["Sprinklers", "Landscaping, Garden Beds", "Terrain, Walls"] {
            cx.project.layers.set_display(name, false);
        }
        paint(&cx);
    }

    #[test]
    fn feature_meshes_follow_the_edits_and_undo() {
        let mut cx = cx();
        assert!(terrain_feature_meshes(&cx.project).is_empty());
        edit_terrain(&mut cx, "Wall", |r| {
            r.terrain.walls.push(TerrainWall::default());
            r.terrain.walls[0].points = vec![pt(0.0, 0.0), pt(200.0, 0.0)];
        });
        let walls = terrain_feature_meshes(&cx.project);
        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].material, plan_3d::Material::Concrete);
        cx.undo();
        assert!(load_terrain(&cx.project).is_none_or(|r| r.terrain.walls.is_empty()));
        assert!(terrain_feature_meshes(&cx.project).is_empty());
    }
}
