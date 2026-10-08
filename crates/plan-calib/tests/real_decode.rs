//! Decode validation against the user's real Chief Architect X18 install.
//!
//! All tests are `#[ignore]` (CI has no Chief install; they read gigabytes).
//! Run with
//!
//! ```text
//! cargo test -p plan-calib --release --test real_decode -- --ignored --nocapture --test-threads=1
//! ```
//!
//! SVG renders are written to `PLAN_CALIB_SVG_DIR` (default: the system temp
//! directory under `plan-calib-svg`). Nothing read here is copied into the
//! repository.

use plan_calib::decode::{self, extents_agree, SizeSource, SymbolSource};
use plan_calib::ChiefCatalog;
use plan_library::{Stroke, Symbol2d};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

const CORE: &str = "/Library/Application Support/Chief Architect Premier X18/Core Libraries";

fn open(name: &str) -> Option<ChiefCatalog> {
    let p = Path::new(CORE).join(name);
    if !p.is_file() {
        eprintln!("skipping: {} not found", p.display());
        return None;
    }
    Some(ChiefCatalog::open(p).expect("open catalog"))
}

fn svg_dir() -> PathBuf {
    let d = std::env::var_os("PLAN_CALIB_SVG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("plan-calib-svg"));
    std::fs::create_dir_all(&d).expect("create svg dir");
    d
}

/// Draws `strokes` (Y up) as an SVG with a footprint box of `w` x `d`.
fn svg(strokes: &[Stroke], w: f64, d: f64) -> String {
    let b = Symbol2d::new(strokes.to_vec()).bounds().expect("bounds");
    let pad = 0.06 * b.width().max(b.height()).max(1.0);
    let (x0, y0) = (b.min.x.min(-w / 2.0) - pad, b.min.y.min(-d / 2.0) - pad);
    let (x1, y1) = (b.max.x.max(w / 2.0) + pad, b.max.y.max(d / 2.0) + pad);
    let (vw, vh) = (x1 - x0, y1 - y0);
    let scale = 600.0 / vw.max(vh);
    let sw = 1.2 / scale;
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{:.0}' height='{:.0}' viewBox='{} {} {} {}'>\n<rect x='{}' y='{}' width='{}' height='{}' fill='white'/>\n",
        vw * scale, vh * scale, x0, -y1, vw, vh, x0, -y1, vw, vh
    );
    s += &format!(
        "<rect x='{}' y='{}' width='{w}' height='{d}' fill='none' stroke='#c33' stroke-width='{}' stroke-dasharray='{} {}'/>\n",
        -w / 2.0, -d / 2.0, sw, 4.0 * sw, 3.0 * sw
    );
    for st in strokes {
        match st {
            Stroke::Polyline { points, closed } => {
                let pts: Vec<String> = points
                    .iter()
                    .map(|p| format!("{:.3},{:.3}", p.x, -p.y))
                    .collect();
                let tag = if *closed { "polygon" } else { "polyline" };
                s += &format!(
                    "<{tag} points='{}' fill='none' stroke='black' stroke-width='{sw}'/>\n",
                    pts.join(" ")
                );
            }
            Stroke::Circle { center, radius } => {
                s += &format!("<circle cx='{}' cy='{}' r='{}' fill='none' stroke='black' stroke-width='{sw}'/>\n", center.x, -center.y, radius);
            }
            Stroke::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let p = |a: f64| {
                    (
                        center.x + radius * a.to_radians().cos(),
                        -(center.y + radius * a.to_radians().sin()),
                    )
                };
                let (a, bb) = (p(*start_deg), p(*end_deg));
                let large = ((end_deg - start_deg).rem_euclid(360.0) > 180.0) as u8;
                s += &format!("<path d='M{} {} A{} {} 0 {} 0 {} {}' fill='none' stroke='black' stroke-width='{sw}'/>\n", a.0, a.1, radius, radius, large, bb.0, bb.1);
            }
        }
    }
    s + "</svg>\n"
}

#[derive(Default)]
struct Tally {
    total: usize,
    sizes: BTreeMap<&'static str, usize>,
    symbols: usize,
    symbols_by_source: BTreeMap<&'static str, usize>,
    both: usize,
    fits: usize,
    examples: Vec<String>,
}

fn survey(cat: &ChiefCatalog, t: &mut Tally, names: &mut Vec<(String, i64, String)>) {
    let mut objects = cat.objects().expect("objects");
    for obj in objects.by_ref() {
        t.total += 1;
        let blobs = cat.object_blobs(obj.library_object_id).expect("blobs");
        let dec = decode::decode_object(&blobs);
        if let Some(s) = dec.size {
            let k = match s.source {
                SizeSource::DataRecord => "data record",
                SizeSource::PlantRecord => "plant record",
                SizeSource::MeshBounds => "mesh bounds",
            };
            *t.sizes.entry(k).or_default() += 1;
        }
        if let Some(sym) = &dec.symbol {
            t.symbols += 1;
            let k = match sym.source {
                SymbolSource::FaceStream => "symDxf faces",
                SymbolSource::TriangleMesh => "triangle mesh",
                SymbolSource::PlantCanopy => "plant canopy",
            };
            *t.symbols_by_source.entry(k).or_default() += 1;
            if let Some(s) = dec.size {
                t.both += 1;
                // Symbol bounds (strokes) vs decoded size, the metric the
                // spec asks for.
                let (bw, bd) = (sym.bounds.width(), sym.bounds.height());
                if extents_agree(bw, s.width) && extents_agree(bd, s.depth) {
                    t.fits += 1;
                    if t.examples.len() < 10 && s.source == SizeSource::DataRecord {
                        t.examples.push(format!(
                            "{:<38} {:>7.2} x {:>6.2} x {:>6.2} in   symbol {:>7.2} x {:>6.2}",
                            obj.name, s.width, s.depth, s.height, bw, bd
                        ));
                    }
                }
            }
        }
        names.push((
            obj.name.clone(),
            obj.library_object_id,
            cat.name().to_owned(),
        ));
    }
}

fn report(label: &str, t: &Tally) {
    let pct = |n: usize, d: usize| 100.0 * n as f64 / d.max(1) as f64;
    let sized: usize = t.sizes.values().sum();
    eprintln!("== {label}: {} objects", t.total);
    eprintln!(
        "   sizes decoded : {sized} ({:.1}%)  {:?}",
        pct(sized, t.total),
        t.sizes
    );
    eprintln!(
        "   symbols       : {} ({:.1}%)  {:?}",
        t.symbols,
        pct(t.symbols, t.total),
        t.symbols_by_source
    );
    eprintln!(
        "   symbol bounds within 10% of decoded size: {} of {} with both ({:.1}%)",
        t.fits,
        t.both,
        pct(t.fits, t.both)
    );
}

#[test]
#[ignore = "needs a Chief Architect X18 install (reads ~1.5 GB)"]
fn decode_coverage_core_catalogs() {
    let mut all = Tally::default();
    let mut names = Vec::new();
    let t0 = Instant::now();
    for file in [
        "CoreArchitectural.calib",
        "CoreInteriors.calib",
        "CoreMEP.calib",
    ] {
        let Some(cat) = open(file) else { continue };
        let t = Instant::now();
        let mut tally = Tally::default();
        survey(&cat, &mut tally, &mut names);
        report(file, &tally);
        eprintln!("   ({:?})", t.elapsed());
        all.total += tally.total;
        all.symbols += tally.symbols;
        all.both += tally.both;
        all.fits += tally.fits;
        for (k, v) in tally.sizes {
            *all.sizes.entry(k).or_default() += v;
        }
        for (k, v) in tally.symbols_by_source {
            *all.symbols_by_source.entry(k).or_default() += v;
        }
        all.examples.extend(tally.examples);
    }
    if all.total == 0 {
        return;
    }
    report("Architectural + Interiors + MEP", &all);
    eprintln!("   10 examples (size from the Data record, symbol within 10%):");
    for e in all.examples.iter().take(10) {
        eprintln!("     {e}");
    }
    eprintln!("   total {:?}", t0.elapsed());
    let sized: usize = all.sizes.values().sum();
    assert!(sized * 10 >= all.total * 7, "expected >= 70% sizes");
    assert!(all.symbols * 2 >= all.total, "expected >= 50% symbols");
}

fn find_object(cat: &ChiefCatalog, name: &str) -> Option<i64> {
    cat.objects()
        .ok()?
        .find(|o| o.name == name)
        .map(|o| o.library_object_id)
}

#[test]
#[ignore = "needs a Chief Architect X18 install"]
fn render_sample_symbols() {
    let out = svg_dir();
    let samples: [(&str, &str, &str); 8] = [
        ("CoreArchitectural.calib", "Round Tank Toilet", "toilet"),
        ("CoreInteriors.calib", "Manta Sofa", "sofa"),
        ("CoreInteriors.calib", "Vanity", "cabinet"),
        ("CoreArchitectural.calib", "Tub-Shower 3", "tub"),
        ("CoreArchitectural.calib", "Door E29", "door"),
        ("CorePlants.calib", "Pinus pinea (adult)", "tree"),
        ("CoreArchitectural.calib", "Pedestal Sink 01", "sink"),
        ("CoreInteriors.calib", "Writing Cabinet", "cabinet2"),
    ];
    for (file, name, tag) in samples {
        let Some(cat) = open(file) else { continue };
        let Some(id) = find_object(&cat, name) else {
            eprintln!("{name}: not found in {file}");
            continue;
        };
        let dec = decode::decode_object(&cat.object_blobs(id).expect("blobs"));
        let Some(sym) = dec.symbol else {
            eprintln!("{name}: no symbol decoded (size {:?})", dec.size);
            continue;
        };
        let (w, d) = dec
            .size
            .map_or((sym.extent[0], sym.extent[1]), |s| (s.width, s.depth));
        let path = out.join(format!("{tag}.svg"));
        std::fs::write(&path, svg(&sym.strokes, w, d)).expect("write svg");
        eprintln!(
            "{tag:<9} {name:<22} size {:?} fits={:?} strokes={} -> {}",
            dec.size.map(|s| (s.width, s.depth, s.height)),
            dec.symbol_fits_size,
            sym.strokes.len(),
            path.display()
        );
    }
}
