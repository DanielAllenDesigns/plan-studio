//! 3D mesh decoding against the user's real Chief Architect X18 install.
//!
//! `#[ignore]`: CI has no Chief install. Run with
//!
//! ```text
//! cargo test -p plan-calib --release --test real_mesh3d -- --ignored --nocapture
//! ```
//!
//! glTF files go to `PLAN_CALIB_GLTF_DIR` (default: the system temp directory
//! under `plan-calib-gltf`). Nothing read here is copied into the repository.

use plan_3d::gltf::write_gltf_files;
use plan_3d::Scene;
use plan_calib::decode::{decode_object, extents_agree};
use plan_calib::mesh3d::{parts_extent, place_triangles, MeshCache, Triangles};
use plan_calib::ChiefCatalog;
use plan_core::geometry::Point;
use plan_core::PlacedSymbol;
use std::path::{Path, PathBuf};

const CORE: &str = "/Library/Application Support/Chief Architect Premier X18/Core Libraries";
const CATALOGS: [&str; 3] = [
    "CoreInteriors.calib",
    "CoreArchitectural.calib",
    "CoreMEP.calib",
];

fn gltf_dir() -> PathBuf {
    let d = std::env::var_os("PLAN_CALIB_GLTF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("plan-calib-gltf"));
    std::fs::create_dir_all(&d).expect("create gltf dir");
    d
}

/// Objects whose name equals `name` (or, when `exact` is false, contains it).
fn find_all(name: &str, exact: bool) -> Vec<(ChiefCatalog, i64, String)> {
    let mut out = Vec::new();
    for file in CATALOGS {
        let p = Path::new(CORE).join(file);
        if !p.is_file() {
            eprintln!("skipping: {} not found", p.display());
            continue;
        }
        let cat = ChiefCatalog::open(&p).expect("open catalog");
        let hits: Vec<_> = cat
            .objects()
            .expect("objects")
            .filter(|o| {
                if exact {
                    o.name == name
                } else {
                    o.name.contains(name)
                }
            })
            .map(|o| (o.library_object_id, o.name))
            .collect();
        for (id, n) in hits {
            out.push((ChiefCatalog::open(&p).expect("open catalog"), id, n));
        }
        if exact && !out.is_empty() {
            break;
        }
    }
    out
}

type Loaded = (
    std::sync::Arc<Vec<Triangles>>,
    plan_calib::decode::Size3,
    bool,
);

/// Decoded parts, the size, and whether the mesh bounds agree with it;
/// `None` when the object has no size.
fn load(cat: &ChiefCatalog, id: i64, cache: &mut MeshCache) -> Option<Loaded> {
    let parts = cache.get_or_decode(cat, id).expect("decode meshes");
    let size = decode_object(&cat.object_blobs(id).expect("blobs")).size?;
    let [ex, ez, ey] = parts_extent(&parts);
    let fits = extents_agree(ex as f64, size.width)
        && extents_agree(ez as f64, size.depth)
        && extents_agree(ey as f64, size.height);
    Some((parts, size, fits))
}

fn check(
    (cat, id, found): (ChiefCatalog, i64, String),
    cache: &mut MeshCache,
    scene: &mut Scene,
    x: f64,
    strict_depth: bool,
) {
    let (parts, size, _) = load(&cat, id, cache).expect("object size");
    let tris: usize = parts.iter().map(Triangles::triangle_count).sum();
    let [ex, ez, ey] = parts_extent(&parts);
    eprintln!(
        "{found} (#{id}): {} parts, {tris} triangles, mesh X{ex:.2} x depth{ez:.2} x H{ey:.2} in; size record {:.2} x {:.2} x {:.2} ({:?})",
        parts.len(), size.width, size.depth, size.height, size.source
    );
    assert!(tris > 0, "{found}: no triangles");
    let agree = |a: f32, b: f64| extents_agree(a as f64, b);
    assert!(
        agree(ex, size.width) && agree(ey, size.height),
        "{found}: mesh width/height differ from the decoded size by more than 10%"
    );
    if strict_depth {
        assert!(
            agree(ez, size.depth),
            "{found}: mesh depth differs from the decoded size by more than 10%"
        );
    } else {
        // Parametric doors: the Data record depth is the jamb, the leaf mesh is thinner.
        assert!(
            ez as f64 <= size.depth * 1.1,
            "{found}: mesh deeper than the record"
        );
        eprintln!(
            "   (depth {ez:.2} vs record {:.2}: leaf only, not compared)",
            size.depth
        );
    }

    // Place it two ways: plain, and rotated 90 degrees + flipped.
    let mut s = PlacedSymbol::new(
        "chief",
        Point::new(x, 0.0),
        size.width,
        size.depth,
        size.height,
    );
    s.id = 1;
    let plain = place_triangles(&parts, &s, 0.0);
    let (lo, hi) = bounds(&plain);
    eprintln!("   placed at ({x}, 0): lo {lo:.2?} hi {hi:.2?}");
    assert!((lo[1]).abs() < 1e-2 && (hi[1] - size.height as f32).abs() < 1e-2);
    assert!((hi[2] - 0.0).abs() < 1e-2 && (lo[2] + size.depth as f32).abs() < 1e-2);
    scene.meshes.extend(plain);
    s.position = Point::new(x, 120.0);
    s.angle = 90.0;
    s.flip = true;
    scene.meshes.extend(place_triangles(&parts, &s, 0.0));
}

fn bounds(ms: &[plan_3d::Mesh]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for m in ms {
        let (l, h) = m.bounds().expect("non-empty mesh");
        for k in 0..3 {
            lo[k] = lo[k].min(l[k]);
            hi[k] = hi[k].max(h[k]);
        }
    }
    (lo, hi)
}

#[test]
#[ignore = "needs a Chief Architect X18 install"]
fn library_objects_decode_and_place() {
    let mut cache = MeshCache::default();
    let mut scene = Scene::default();
    let mut x = 0.0;
    let mut checked = 0;

    for name in ["Round Tank Toilet", "Door E29"] {
        match find_all(name, true).into_iter().next() {
            Some(hit) => {
                check(hit, &mut cache, &mut scene, x, name != "Door E29");
                checked += 1;
                x += 100.0;
            }
            None => eprintln!("skipping {name}: not found"),
        }
    }

    // Sofas: some have partial meshes (undecoded record kinds); survey them and
    // check the first whose mesh bounds match its size record.
    let sofas = find_all("Sofa", false);
    let (mut with_geometry, mut agreeing, mut pick) = (0, 0, None);
    for (cat, id, name) in sofas {
        let Some((parts, size, fits)) = load(&cat, id, &mut cache) else {
            continue;
        };
        if parts.is_empty() {
            continue;
        }
        with_geometry += 1;
        let [ex, ez, ey] = parts_extent(&parts);
        eprintln!(
            "   sofa #{id} {name:?}: mesh {ex:.1} x {ez:.1} x {ey:.1} vs size {:.1} x {:.1} x {:.1}: {}",
            size.width, size.depth, size.height,
            if fits { "agrees" } else { "partial" }
        );
        if fits {
            agreeing += 1;
            if pick.is_none() {
                pick = Some((cat, id, name));
            }
        }
    }
    eprintln!("sofas with triangle geometry: {with_geometry}, agreeing with size: {agreeing}");
    if let Some(hit) = pick {
        check(hit, &mut cache, &mut scene, x, true);
        checked += 1;
    }

    if scene.meshes.is_empty() {
        return;
    }
    assert!(checked >= 2, "expected toilet and door at least");
    let (lo, hi) = scene.bounds().expect("scene bounds");
    eprintln!(
        "scene: {} meshes, {} triangles, bounds {lo:.1?} .. {hi:.1?}",
        scene.meshes.len(),
        scene.triangle_count()
    );
    let base = gltf_dir().join("chief-objects");
    write_gltf_files(&scene, &base).expect("write glTF");
    eprintln!("wrote {}.gltf", base.display());
    assert!(!cache.is_empty());
}
