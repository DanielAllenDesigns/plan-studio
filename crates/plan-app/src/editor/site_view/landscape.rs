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
    landscape_meshes, terrain_object_id, terrain_object_of, ElevationLine, ElevationPoint,
    ElevationRegion, Feature, Landscape, Modifier, ModifierKind, ObjectExtras, ObjectKey, PlanItem,
    PlanShape, RoadStrip, Terrain, TerrainBreak, TerrainPart, TerrainWall,
};

/// An element of the terrain that has its own specification dialog.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum TerrainObject {
    Feature(Feature),
    Break(TerrainBreak),
    Wall(TerrainWall),
    Landscape(Landscape),
    Road(RoadStrip),
    Line(ElevationLine),
    /// Points, regions and modifiers keep their label, schedule and
    /// information panels in the terrain's side table; the dialog edits them
    /// with the object.
    Point(ElevationPoint, ObjectExtras),
    Region(ElevationRegion, ObjectExtras),
    Modifier(Modifier, ObjectExtras),
}

impl TerrainObject {
    /// The label, schedule and object information panels' data of the object.
    pub fn extras_mut(&mut self) -> &mut ObjectExtras {
        match self {
            TerrainObject::Feature(o) => &mut o.extras,
            TerrainObject::Break(o) => &mut o.extras,
            TerrainObject::Wall(o) => &mut o.extras,
            TerrainObject::Landscape(o) => &mut o.extras,
            TerrainObject::Road(o) => &mut o.extras,
            TerrainObject::Line(o) => &mut o.extras,
            TerrainObject::Point(_, x)
            | TerrainObject::Region(_, x)
            | TerrainObject::Modifier(_, x) => x,
        }
    }

    pub fn extras(&self) -> &ObjectExtras {
        match self {
            TerrainObject::Feature(o) => &o.extras,
            TerrainObject::Break(o) => &o.extras,
            TerrainObject::Wall(o) => &o.extras,
            TerrainObject::Landscape(o) => &o.extras,
            TerrainObject::Road(o) => &o.extras,
            TerrainObject::Line(o) => &o.extras,
            TerrainObject::Point(_, x)
            | TerrainObject::Region(_, x)
            | TerrainObject::Modifier(_, x) => x,
        }
    }

    /// The title of its specification.
    pub fn title(&self) -> &'static str {
        match self {
            TerrainObject::Feature(f) if f.kind == plan_terrain::FeatureKind::Hole => {
                "Terrain Hole Specification"
            }
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
                plan_terrain::LandscapeKind::SprinklerLine => "Sprinkler Line Specification",
            },
            TerrainObject::Road(r) => match r.kind {
                plan_terrain::RoadKind::Road => "Road Specification",
                plan_terrain::RoadKind::Driveway => "Driveway Specification",
                plan_terrain::RoadKind::Sidewalk => "Sidewalk Specification",
                plan_terrain::RoadKind::Marking => "Road Marking Specification",
                plan_terrain::RoadKind::Median => "Median Specification",
                plan_terrain::RoadKind::CulDeSac => "Cul-de-sac Specification",
            },
            TerrainObject::Line(_) => "Elevation Line Specification",
            TerrainObject::Point(..) => "Elevation Point Specification",
            TerrainObject::Region(..) => "Elevation Region Specification",
            TerrainObject::Modifier(m, _) => match m.kind {
                ModifierKind::Hill => "Hill Specification",
                ModifierKind::Valley => "Valley Specification",
                ModifierKind::RaisedRegion => "Raised Region Specification",
                ModifierKind::LoweredRegion => "Lowered Region Specification",
                ModifierKind::FlatRegion => "Flat Region Specification",
            },
        }
    }
}

/// The element a hit names, as an editable copy (`None` for the perimeter,
/// whose specification is the Terrain Specification itself).
pub fn object_at(t: &Terrain, hit: TerrainHit) -> Option<TerrainObject> {
    match hit {
        TerrainHit::Feature(i) => t.features.get(i).cloned().map(TerrainObject::Feature),
        TerrainHit::Point(i) => t
            .elevation_points
            .get(i)
            .cloned()
            .map(|p| TerrainObject::Point(p, t.extras(ObjectKey::Point(i)))),
        TerrainHit::Region(i) => t
            .elevation_regions
            .get(i)
            .cloned()
            .map(|r| TerrainObject::Region(r, t.extras(ObjectKey::Region(i)))),
        TerrainHit::Modifier(i) => t
            .modifiers
            .get(i)
            .cloned()
            .map(|m| TerrainObject::Modifier(m, t.extras(ObjectKey::Modifier(i)))),
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
        (TerrainHit::Point(i), TerrainObject::Point(x, ex)) => {
            let ok = put(&mut t.elevation_points, i, x);
            t.set_extras(ObjectKey::Point(i), ex);
            ok
        }
        (TerrainHit::Region(i), TerrainObject::Region(x, ex)) => {
            let ok = put(&mut t.elevation_regions, i, x);
            t.set_extras(ObjectKey::Region(i), ex);
            ok
        }
        (TerrainHit::Modifier(i), TerrainObject::Modifier(x, ex)) => {
            let ok = put(&mut t.modifiers, i, x);
            t.set_extras(ObjectKey::Modifier(i), ex);
            ok
        }
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
            .map(|l| {
                shift(&mut l.points, delta);
                shift(&mut l.control, delta);
            })
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
            .map(|f| {
                shift(&mut f.polygon, delta);
                shift(&mut f.control, delta);
                f.center = f.center + delta;
            })
            .is_some(),
        TerrainHit::Road(i) => t
            .roads
            .get_mut(i)
            .map(|r| {
                shift(&mut r.centerline, delta);
                shift(&mut r.outline, delta);
                r.center = r.center + delta;
            })
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
            .map(|l| {
                shift(&mut l.points, delta);
                shift(&mut l.control, delta);
            })
            .is_some(),
    }
}

// ----- terrain elements as selectable objects -----

/// Does the element `hit` names exist?
pub fn hit_exists(t: &Terrain, hit: TerrainHit) -> bool {
    match hit {
        TerrainHit::Perimeter => !t.perimeter.is_empty(),
        TerrainHit::Point(i) => i < t.elevation_points.len(),
        TerrainHit::Line(i) => i < t.elevation_lines.len(),
        TerrainHit::Region(i) => i < t.elevation_regions.len(),
        TerrainHit::Modifier(i) => i < t.modifiers.len(),
        TerrainHit::Feature(i) => i < t.features.len(),
        TerrainHit::Road(i) => i < t.roads.len(),
        TerrainHit::Break(i) => i < t.breaks.len(),
        TerrainHit::Wall(i) => i < t.walls.len(),
        TerrainHit::Landscape(i) => i < t.landscape.len(),
    }
}

/// Every element the Select tool can pick, in drawing order: everything but
/// the perimeter (that is the whole terrain, `ObjectRef::Terrain`).
pub fn all_hits(t: &Terrain) -> Vec<TerrainHit> {
    let mut out = Vec::new();
    out.extend((0..t.elevation_points.len()).map(TerrainHit::Point));
    out.extend((0..t.elevation_lines.len()).map(TerrainHit::Line));
    out.extend((0..t.elevation_regions.len()).map(TerrainHit::Region));
    out.extend((0..t.modifiers.len()).map(TerrainHit::Modifier));
    out.extend((0..t.features.len()).map(TerrainHit::Feature));
    out.extend((0..t.roads.len()).map(TerrainHit::Road));
    out.extend((0..t.breaks.len()).map(TerrainHit::Break));
    out.extend((0..t.walls.len()).map(TerrainHit::Wall));
    out.extend((0..t.landscape.len()).map(TerrainHit::Landscape));
    out
}

/// The plan points that outline the element: the vertices its handles sit on
/// (empty when it is gone).
pub fn hit_points(t: &Terrain, hit: TerrainHit) -> Vec<Point> {
    match hit {
        TerrainHit::Perimeter => t.perimeter.clone(),
        TerrainHit::Point(i) => t
            .elevation_points
            .get(i)
            .map(|e| e.pos)
            .into_iter()
            .collect(),
        TerrainHit::Line(i) => t
            .elevation_lines
            .get(i)
            .map(|l| {
                if l.control.is_empty() {
                    l.points.clone()
                } else {
                    l.control.clone()
                }
            })
            .unwrap_or_default(),
        TerrainHit::Region(i) => t
            .elevation_regions
            .get(i)
            .map(|r| r.polygon.clone())
            .unwrap_or_default(),
        TerrainHit::Modifier(i) => t
            .modifiers
            .get(i)
            .map(|m| m.polygon.clone())
            .unwrap_or_default(),
        TerrainHit::Feature(i) => t
            .features
            .get(i)
            .map(|f| {
                if f.control.is_empty() {
                    f.polygon.clone()
                } else {
                    f.control.clone()
                }
            })
            .unwrap_or_default(),
        TerrainHit::Road(i) => t
            .roads
            .get(i)
            .map(|r| {
                if r.outline.len() >= 3 {
                    r.outline.clone()
                } else {
                    r.centerline.clone()
                }
            })
            .unwrap_or_default(),
        TerrainHit::Break(i) => t
            .breaks
            .get(i)
            .map(|b| b.points.clone())
            .unwrap_or_default(),
        TerrainHit::Wall(i) => t.walls.get(i).map(|w| w.points.clone()).unwrap_or_default(),
        TerrainHit::Landscape(i) => t
            .landscape
            .get(i)
            .map(|l| {
                if l.control.is_empty() {
                    l.points.clone()
                } else {
                    l.control.clone()
                }
            })
            .unwrap_or_default(),
    }
}

/// Is the element's outline closed (a region) rather than a path or a point?
pub fn hit_is_closed(t: &Terrain, hit: TerrainHit) -> bool {
    match hit {
        TerrainHit::Perimeter
        | TerrainHit::Region(_)
        | TerrainHit::Modifier(_)
        | TerrainHit::Feature(_) => true,
        TerrainHit::Landscape(i) => t.landscape.get(i).is_some_and(Landscape::is_region),
        TerrainHit::Road(i) => t.roads.get(i).is_some_and(|r| r.outline.len() >= 3),
        _ => false,
    }
}

/// Moves vertex `n` of the element to `to`; false when either is gone.
pub fn move_terrain_vertex(t: &mut Terrain, hit: TerrainHit, n: usize, to: Point) -> bool {
    fn put(v: &mut [Point], n: usize, to: Point) -> bool {
        v.get_mut(n).map(|p| *p = to).is_some()
    }
    match hit {
        TerrainHit::Perimeter => put(&mut t.perimeter, n, to),
        TerrainHit::Point(i) => {
            n == 0 && t.elevation_points.get_mut(i).map(|e| e.pos = to).is_some()
        }
        TerrainHit::Line(i) => t.elevation_lines.get_mut(i).is_some_and(|l| {
            if l.control.is_empty() {
                put(&mut l.points, n, to)
            } else {
                // A spline: the handles are its control points.
                let moved = put(&mut l.control, n, to);
                l.reflatten(crate::tools::terrain::SPLINE_SAMPLES);
                moved
            }
        }),
        TerrainHit::Region(i) => t
            .elevation_regions
            .get_mut(i)
            .is_some_and(|r| put(&mut r.polygon, n, to)),
        TerrainHit::Modifier(i) => t
            .modifiers
            .get_mut(i)
            .is_some_and(|m| put(&mut m.polygon, n, to)),
        TerrainHit::Feature(i) => t.features.get_mut(i).is_some_and(|f| {
            if f.kind == plan_terrain::FeatureKind::Round {
                // A round feature: a handle drags the edge, which sets the radius.
                f.set_radius_through(to);
                n < f.polygon.len()
            } else if f.control.is_empty() {
                put(&mut f.polygon, n, to)
            } else {
                let moved = put(&mut f.control, n, to);
                f.reflatten();
                moved
            }
        }),
        TerrainHit::Road(i) => t.roads.get_mut(i).is_some_and(|r| {
            if r.kind == plan_terrain::RoadKind::CulDeSac {
                // A handle drags the edge of the circle, which sets the radius.
                r.radius = r.center.dist(to).max(12.0);
                r.outline =
                    plan_terrain::road_polygon(&plan_terrain::cul_de_sac(r.center, r.radius));
                n < r.outline.len()
            } else if r.outline.len() >= 3 {
                put(&mut r.outline, n, to)
            } else {
                put(&mut r.centerline, n, to)
            }
        }),
        TerrainHit::Break(i) => t
            .breaks
            .get_mut(i)
            .is_some_and(|b| put(&mut b.points, n, to)),
        TerrainHit::Wall(i) => t
            .walls
            .get_mut(i)
            .is_some_and(|w| put(&mut w.points, n, to)),
        TerrainHit::Landscape(i) => t.landscape.get_mut(i).is_some_and(|l| {
            if l.control.is_empty() {
                put(&mut l.points, n, to)
            } else {
                // A kidney or spline: the handles are its control points.
                let moved = put(&mut l.control, n, to);
                l.reflatten();
                moved
            }
        }),
    }
}

/// The layer the element is drawn on when it has one of its own (features,
/// breaks, walls and landscape objects); the others share the terrain's.
pub fn hit_layer(t: &Terrain, hit: TerrainHit) -> Option<String> {
    match hit {
        TerrainHit::Feature(i) => t
            .features
            .get(i)
            .map(|f| f.style.layer_or(LAYER_FEATURES).to_string()),
        TerrainHit::Break(i) => t
            .breaks
            .get(i)
            .map(|b| b.style.layer_or(LAYER_BREAKS).to_string()),
        TerrainHit::Wall(i) => t
            .walls
            .get(i)
            .map(|w| w.style.layer_or(w.default_layer()).to_string()),
        TerrainHit::Landscape(i) => t.landscape.get(i).map(|l| l.layer().to_string()),
        TerrainHit::Road(i) => t
            .roads
            .get(i)
            .and_then(|r| r.own_layer())
            .map(str::to_string),
        _ => None,
    }
}

/// Chief's name for the kind of element.
pub fn hit_type_name(t: Option<&Terrain>, hit: TerrainHit) -> &'static str {
    match hit {
        TerrainHit::Perimeter => "Terrain Perimeter",
        TerrainHit::Point(_) => "Elevation Point",
        TerrainHit::Line(_) => "Elevation Line",
        TerrainHit::Region(_) => "Elevation Region",
        TerrainHit::Modifier(_) => "Terrain Modifier",
        TerrainHit::Feature(_) => "Terrain Feature",
        TerrainHit::Road(i) => t
            .and_then(|t| t.roads.get(i))
            .map_or("Road", |r| r.kind.name()),
        TerrainHit::Break(_) => "Terrain Break",
        TerrainHit::Wall(i) => match t.and_then(|t| t.walls.get(i)) {
            Some(w) if w.kind == plan_terrain::WallKind::Curb => "Terrain Curb",
            _ => "Terrain Wall",
        },
        TerrainHit::Landscape(i) => t
            .and_then(|t| t.landscape.get(i))
            .map_or("Landscape Object", Landscape::name),
    }
}

/// Removes several elements; deleting from the highest index down keeps the
/// others addressable. Returns how many went.
pub fn remove_terrain_elements(t: &mut Terrain, hits: &[TerrainHit]) -> usize {
    fn rank(h: &TerrainHit) -> (u8, usize) {
        match *h {
            TerrainHit::Perimeter => (0, 0),
            TerrainHit::Point(i) => (1, i),
            TerrainHit::Line(i) => (2, i),
            TerrainHit::Region(i) => (3, i),
            TerrainHit::Modifier(i) => (4, i),
            TerrainHit::Feature(i) => (5, i),
            TerrainHit::Road(i) => (6, i),
            TerrainHit::Break(i) => (7, i),
            TerrainHit::Wall(i) => (8, i),
            TerrainHit::Landscape(i) => (9, i),
        }
    }
    let mut sorted = hits.to_vec();
    sorted.sort_by_key(|h| std::cmp::Reverse(rank(h)));
    sorted.dedup();
    sorted
        .into_iter()
        .filter(|h| super::remove_terrain_element(t, *h))
        .count()
}

/// Moves several elements by `delta`; returns how many moved.
pub fn move_terrain_elements(t: &mut Terrain, hits: &[TerrainHit], delta: Point) -> usize {
    hits.iter()
        .filter(|h| move_terrain_element(t, **h, delta))
        .count()
}

/// The `object_id` of the 3D meshes of the element (0 when it has none: only
/// features, roads, walls and landscape objects are meshed).
pub fn hit_mesh_id(hit: TerrainHit) -> u64 {
    match hit {
        TerrainHit::Feature(i) => terrain_object_id(TerrainPart::Feature, i),
        TerrainHit::Road(i) => terrain_object_id(TerrainPart::Road, i),
        TerrainHit::Wall(i) => terrain_object_id(TerrainPart::Wall, i),
        TerrainHit::Landscape(i) => terrain_object_id(TerrainPart::Landscape, i),
        _ => 0,
    }
}

/// The element a 3D mesh `object_id` belongs to, if it is a terrain object's.
pub fn hit_for_mesh_id(id: u64) -> Option<TerrainHit> {
    terrain_object_of(id).map(|(part, i)| match part {
        TerrainPart::Feature => TerrainHit::Feature(i),
        TerrainPart::Wall => TerrainHit::Wall(i),
        TerrainPart::Landscape => TerrainHit::Landscape(i),
        TerrainPart::Road => TerrainHit::Road(i),
    })
}

/// Adds the Chief layers of the landscape objects to the plan's layer list
/// when they are missing (no undo step of their own: call it inside the edit).
pub fn ensure_landscape_layers(project: &mut Project) {
    const LAYERS: [(&str, [u8; 3], u32); 12] = [
        (plan_terrain::LAYER_PRIMARY_CONTOURS, [122, 85, 43], 18),
        (plan_terrain::LAYER_SECONDARY_CONTOURS, [168, 139, 99], 13),
        (plan_terrain::LAYER_TERRAIN_LABELS, [176, 64, 48], 13),
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
    let mut meshes = landscape_meshes(&view.record.terrain, view.surface.as_ref());
    // The skirt around the terrain edge (Terrain Specification > Skirt).
    if let Some(surface) = view.surface.as_ref() {
        meshes.extend(plan_terrain::skirt_mesh(&view.record.terrain, surface));
    }
    meshes
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
        // A mismatch changes nothing; the perimeter has no object dialog.
        let b = object_at(&t, TerrainHit::Break(0)).unwrap();
        assert!(!replace_object(&mut t, TerrainHit::Wall(0), b));
        assert!(object_at(&t, TerrainHit::Perimeter).is_none());
        assert!(object_at(&t, TerrainHit::Wall(9)).is_none());
        // A hole opens the Terrain Feature dialog with its general page only.
        t.features[0].kind = plan_terrain::FeatureKind::Hole;
        assert!(object_at(&t, TerrainHit::Feature(0)).is_some());
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

#[cfg(test)]
mod element_tests {
    use super::*;
    use plan_terrain::{Landscape, LandscapeKind, ShapeKind, WallKind};

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn terrain() -> Terrain {
        let mut t = Terrain::default();
        for i in 0..3 {
            t.walls.push(TerrainWall::new(
                WallKind::Wall,
                vec![
                    pt(0.0, f64::from(i) * 100.0),
                    pt(100.0, f64::from(i) * 100.0),
                ],
                false,
            ));
        }
        t.landscape.push(Landscape::new(
            LandscapeKind::GardenBed,
            ShapeKind::Polyline,
            vec![pt(0.0, 500.0), pt(100.0, 500.0), pt(100.0, 600.0)],
        ));
        t
    }

    #[test]
    fn several_elements_are_removed_without_disturbing_each_other() {
        let mut t = terrain();
        // Wall 0 and wall 2 together: removing 0 first would shift 2 onto 1.
        let n = remove_terrain_elements(&mut t, &[TerrainHit::Wall(0), TerrainHit::Wall(2)]);
        assert_eq!(n, 2);
        assert_eq!(t.walls.len(), 1);
        assert_eq!(t.walls[0].points[0].y, 100.0, "wall 1 is the one left");
        // Duplicates and elements that are gone count once or not at all.
        let n = remove_terrain_elements(
            &mut t,
            &[
                TerrainHit::Wall(0),
                TerrainHit::Wall(0),
                TerrainHit::Wall(7),
            ],
        );
        assert_eq!(n, 1);
        assert!(t.walls.is_empty() && t.landscape.len() == 1);
    }

    #[test]
    fn vertices_move_and_elements_describe_themselves() {
        let mut t = terrain();
        let bed = TerrainHit::Landscape(0);
        assert!(hit_exists(&t, bed) && !hit_exists(&t, TerrainHit::Landscape(1)));
        assert_eq!(hit_points(&t, bed).len(), 3);
        assert!(hit_is_closed(&t, bed));
        assert!(!hit_is_closed(&t, TerrainHit::Wall(0)));
        assert!(move_terrain_vertex(&mut t, bed, 1, pt(120.0, 520.0)));
        assert_eq!(hit_points(&t, bed)[1], pt(120.0, 520.0));
        assert!(!move_terrain_vertex(&mut t, bed, 9, pt(0.0, 0.0)));
        assert!(!move_terrain_vertex(
            &mut t,
            TerrainHit::Wall(9),
            0,
            pt(0.0, 0.0)
        ));
        assert_eq!(
            hit_layer(&t, bed).as_deref(),
            Some("Landscaping, Garden Beds")
        );
        assert_eq!(
            hit_layer(&t, TerrainHit::Wall(1)).as_deref(),
            Some("Terrain, Walls")
        );
        assert_eq!(hit_layer(&t, TerrainHit::Point(0)), None);
        assert_eq!(hit_type_name(Some(&t), bed), "Garden Bed");
        assert_eq!(hit_type_name(None, TerrainHit::Wall(0)), "Terrain Wall");
        let curb = {
            let mut c = TerrainWall::new(WallKind::Curb, vec![pt(0.0, 0.0), pt(10.0, 0.0)], false);
            c.height = 6.0;
            c
        };
        t.walls.push(curb);
        assert_eq!(hit_type_name(Some(&t), TerrainHit::Wall(3)), "Terrain Curb");
        // Every pickable element but the perimeter.
        assert_eq!(all_hits(&t).len(), 5);
        assert!(!all_hits(&t).contains(&TerrainHit::Perimeter));
    }

    #[test]
    fn mesh_ids_name_the_element_they_came_from() {
        for hit in [
            TerrainHit::Feature(2),
            TerrainHit::Road(0),
            TerrainHit::Wall(5),
            TerrainHit::Landscape(11),
        ] {
            assert_eq!(hit_for_mesh_id(hit_mesh_id(hit)), Some(hit));
        }
        assert_eq!(hit_mesh_id(TerrainHit::Point(1)), 0);
        assert_eq!(hit_for_mesh_id(17), None);
    }

    #[test]
    fn kidney_and_spline_handles_are_their_control_points() {
        let (a, b, c) = (pt(0.0, 0.0), pt(300.0, 0.0), pt(150.0, 100.0));
        let control = plan_terrain::kidney_control_points(a, b, c).unwrap();
        let mut t = Terrain::default();
        let mut bed = Landscape::new(
            LandscapeKind::GardenBed,
            ShapeKind::Kidney,
            plan_terrain::closed_spline(&control),
        );
        bed.control = control.clone();
        t.landscape.push(bed);
        let hit = TerrainHit::Landscape(0);
        // Handles sit on the control points, not the 80 flattened ones.
        assert_eq!(hit_points(&t, hit), control);
        let outline = t.landscape[0].points.clone();
        assert!(move_terrain_vertex(&mut t, hit, 2, pt(200.0, -80.0)));
        assert_eq!(t.landscape[0].control[2], pt(200.0, -80.0));
        assert_ne!(t.landscape[0].points, outline, "the curve followed");
        assert_eq!(t.landscape[0].points.len(), outline.len());
        assert!(t.landscape[0].points[2 * 8].dist(pt(200.0, -80.0)) < 1e-9);
        // The whole element moves with its controls.
        assert!(move_terrain_element(&mut t, hit, pt(10.0, 10.0)));
        assert_eq!(t.landscape[0].control[2], pt(210.0, -70.0));
        assert!(t.landscape[0].points[2 * 8].dist(pt(210.0, -70.0)) < 1e-9);

        // A kidney feature and an elevation spline work the same way.
        t.features.push(Feature {
            kind: plan_terrain::FeatureKind::Kidney,
            polygon: plan_terrain::closed_spline(&control),
            control: control.clone(),
            ..Feature::default()
        });
        assert!(move_terrain_vertex(
            &mut t,
            TerrainHit::Feature(0),
            0,
            pt(310.0, 5.0)
        ));
        assert!(t.features[0].polygon[0].dist(pt(310.0, 5.0)) < 1e-9);
        t.elevation_lines.push(plan_terrain::ElevationLine::spline(
            vec![pt(0.0, 0.0), pt(100.0, 50.0), pt(200.0, 0.0)],
            24.0,
            0.5,
            8,
        ));
        let line = TerrainHit::Line(0);
        assert_eq!(hit_points(&t, line).len(), 3);
        assert!(move_terrain_vertex(&mut t, line, 1, pt(100.0, 90.0)));
        assert!(t.elevation_lines[0].points[8].dist(pt(100.0, 90.0)) < 1e-9);
    }

    #[test]
    fn auto_rebuild_off_keeps_the_built_surface_until_build_terrain_runs_again() {
        use crate::editor::site_view::{edit_terrain, terrain_key, terrain_view};
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let elevation = |cx: &EditorContext| {
            crate::editor::site_view::terrain_elevation_at(&cx.project, pt(600.0, 480.0))
        };
        edit_terrain(&mut cx, "Build", |r| {
            r.terrain.perimeter = Terrain::default().perimeter;
            r.built = true;
            r.terrain
                .elevation_points
                .push(plan_terrain::ElevationPoint {
                    pos: pt(600.0, 480.0),
                    z: 24.0,
                });
            r.built_key = terrain_key(r);
        });
        assert_eq!(elevation(&cx).map(f64::round), Some(24.0));

        // With auto rebuild on, an edit shows at once.
        edit_terrain(&mut cx, "Edit", |r| r.terrain.elevation_points[0].z = 48.0);
        assert_eq!(elevation(&cx).map(f64::round), Some(48.0));
        assert!(!terrain_view(&cx.project).unwrap().stale);

        // Off: the edit leaves the surface as built and marks the view stale.
        edit_terrain(&mut cx, "Spec", |r| {
            let mut draft = r.clone();
            draft.auto_rebuild = false;
            r.apply_spec(&draft);
        });
        assert_eq!(elevation(&cx).map(f64::round), Some(48.0));
        edit_terrain(&mut cx, "Edit", |r| r.terrain.elevation_points[0].z = 96.0);
        assert_eq!(
            elevation(&cx).map(f64::round),
            Some(48.0),
            "still the built surface"
        );
        assert!(terrain_view(&cx.project).unwrap().stale);
        // Build Terrain brings it up to date.
        crate::tools::terrain::build_terrain_now(&mut cx).unwrap();
        assert_eq!(elevation(&cx).map(f64::round), Some(96.0));
        assert!(!terrain_view(&cx.project).unwrap().stale);
        // The switch survives a save.
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert!(
            !crate::editor::site_view::load_terrain(&back)
                .unwrap()
                .auto_rebuild
        );
    }

    #[test]
    fn the_building_pad_comes_from_the_walls_and_the_first_floor() {
        use crate::editor::site_view::auto_building_pad;
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        assert!(!auto_building_pad(&mut cx), "no building yet");
        assert!(cx.status.contains("walls"));
        for (a, b) in [
            (pt(500.0, 400.0), pt(700.0, 400.0)),
            (pt(700.0, 400.0), pt(700.0, 560.0)),
            (pt(700.0, 560.0), pt(500.0, 560.0)),
            (pt(500.0, 560.0), pt(500.0, 400.0)),
        ] {
            cx.project
                .add_wall(0, a, b, 6.0, 109.0, plan_core::WallKind::Exterior);
        }
        assert!(auto_building_pad(&mut cx));
        let rec = crate::editor::site_view::load_terrain(&cx.project).unwrap();
        let pad = rec.terrain.building_pad.as_ref().unwrap();
        assert!(pad.footprint.len() >= 4);
        assert_eq!(pad.first_floor, Some(cx.project.floors[0].elevation));
        assert_eq!(
            rec.terrain.building_pad_elevation,
            cx.project.floors[0].elevation - rec.terrain.subfloor_height_above_terrain
        );
        // Same walls, same pad: nothing changes and there is no new undo step.
        let undo = cx.undo_label().map(str::to_string);
        assert!(!auto_building_pad(&mut cx));
        assert_eq!(cx.undo_label().map(str::to_string), undo);
        assert_eq!(cx.undo().as_deref(), Some("Building Pad"));
    }
}
