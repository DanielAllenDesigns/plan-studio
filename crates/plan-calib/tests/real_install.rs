//! Integration tests against the user's real Chief Architect X18 installation.
//!
//! All tests are `#[ignore]`: CI has no Chief install, and some of them touch
//! gigabytes. Run them locally with
//!
//! ```text
//! cargo test -p plan-calib --release -- --ignored --nocapture
//! ```
//!
//! Each test skips (passes with a note) when the files it needs are missing.
//! Set `PLAN_CALIB_TEST_CALIBZ` to point the `.calibz` test at another pack.
//! Nothing read here is copied into the repository.

use plan_calib::{ChiefCatalog, ChiefLibrary};
use std::path::{Path, PathBuf};
use std::time::Instant;

const CORE: &str = "/Library/Application Support/Chief Architect Premier X18/Core Libraries";

fn core(name: &str) -> Option<PathBuf> {
    let p = Path::new(CORE).join(name);
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("skipping: {} not found", p.display());
        None
    }
}

#[test]
#[ignore = "needs a Chief Architect X18 install (900 MB CoreArchitectural.calib)"]
fn core_architectural_browses() {
    let Some(path) = core("CoreArchitectural.calib") else {
        return;
    };
    let t = Instant::now();
    let cat = ChiefCatalog::open(&path).unwrap();
    eprintln!("open: {:?}  id={}", t.elapsed(), cat.id());

    let t = Instant::now();
    let count = cat.count().unwrap();
    eprintln!("count = {count}: {:?}", t.elapsed());
    assert!(count > 1000, "expected > 1000 objects, got {count}");

    let t = Instant::now();
    let mut objects = cat.objects().unwrap();
    let all: Vec<_> = objects.by_ref().collect();
    assert!(objects.error().is_none(), "{:?}", objects.error());
    eprintln!(
        "objects() full pass ({} items): {:?}",
        all.len(),
        t.elapsed()
    );
    assert_eq!(all.len(), count);

    let door = all
        .iter()
        .find(|o| o.name.contains("Door"))
        .expect("an object whose name contains 'Door'");
    eprintln!(
        "door: #{} {:?} {:?} kw={}",
        door.library_object_id,
        door.name,
        door.category_path,
        door.keywords.len()
    );
    assert!(!door.category_path.is_empty());

    let with_thumb = all.iter().find(|o| o.has_thumbnail).expect("a thumbnail");
    let t = Instant::now();
    let png = cat
        .thumbnail(with_thumb.library_object_id)
        .unwrap()
        .unwrap();
    eprintln!("thumbnail {} B: {:?}", png.len(), t.elapsed());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    let t = Instant::now();
    let tree = cat.category_tree().unwrap();
    eprintln!(
        "tree root {:?}, {} children, total {}: {:?}",
        tree.name,
        tree.children.len(),
        tree.total_count(),
        t.elapsed()
    );
    assert!(
        tree.name.contains("Architectural"),
        "root was {:?}",
        tree.name
    );
    assert!(!tree.children.is_empty());

    // AssociatedData JSON: report how many of the first 300 parse.
    let t = Instant::now();
    let mut parsed = 0;
    for o in all.iter().take(300) {
        if cat.associated_json(o.library_object_id).unwrap().is_some() {
            parsed += 1;
        }
    }
    eprintln!("associated_json parsed {parsed}/300: {:?}", t.elapsed());

    // Bridge a page of items.
    let t = Instant::now();
    let r = plan_calib::to_plan_library(&cat, Some(100)).unwrap();
    eprintln!(
        "bridge 100 items, {} thumbnails: {:?}",
        r.thumbnails.len(),
        t.elapsed()
    );
    assert_eq!(r.catalog.items.len(), 100);
}

#[test]
#[ignore = "needs a Chief Architect X18 install"]
fn find_door_across_core_catalogs() {
    let Some(p) = core("CoreCAD.calib") else {
        return;
    };
    let t = Instant::now();
    let cat = ChiefCatalog::open(&p).unwrap();
    let n = cat.objects().unwrap().count();
    eprintln!("CoreCAD: {n} objects in {:?}", t.elapsed());
    assert!(n > 100);
}

#[test]
#[ignore = "needs a .calibz pack (default: the Rabbitt Pro Plan Tools pack, ~1 GB)"]
fn calibz_extracts_and_opens() {
    let path = std::env::var_os("PLAN_CALIB_TEST_CALIBZ")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs_home().join(
                "Documents/Rabbitt x16 Instructions/Step 2-Drag the containing file into Chief/Pro Plan Tools-240523.calibz",
            )
        });
    if !path.is_file() {
        eprintln!("skipping: {} not found", path.display());
        return;
    }
    let t = Instant::now();
    let cz = plan_calib::CalibZ::open(&path).unwrap();
    eprintln!("list {} entries: {:?}", cz.entries().len(), t.elapsed());
    assert!(cz.entries().len() > 1);

    let t = Instant::now();
    let tmp = cz.extract_calib_to_temp().unwrap();
    let size = std::fs::metadata(&tmp).unwrap().len();
    eprintln!("extracted .calib {size} B to temp: {:?}", t.elapsed());
    assert_eq!(size, cz.calib_entry().uncompressed_size);
    let mut magic = [0u8; 16];
    std::io::Read::read_exact(&mut std::fs::File::open(&tmp).unwrap(), &mut magic).unwrap();
    assert_eq!(&magic, b"SQLite format 3\0");

    let t = Instant::now();
    let cat = ChiefCatalog::open(&path).unwrap();
    let objs: Vec<_> = cat.objects().unwrap().collect();
    eprintln!(
        "opened via ChiefCatalog: {} objects: {:?}",
        objs.len(),
        t.elapsed()
    );

    // A texture by basename, if the pack has any image.
    if let Some(img) = cz
        .entries()
        .iter()
        .find(|e| matches!(e.name.rsplit('.').next(), Some("jpg" | "png")))
    {
        let t = Instant::now();
        let bytes = cz.texture(img.basename()).unwrap();
        eprintln!(
            "texture {:?}: {} B in {:?}",
            img.basename(),
            bytes.len(),
            t.elapsed()
        );
        assert_eq!(bytes.len() as u64, img.uncompressed_size);
    }
    let _ = std::fs::remove_file(tmp);
}

#[test]
#[ignore = "needs a Chief Architect X18 install"]
fn discover_lists_the_registry() {
    let t = Instant::now();
    let lib = ChiefLibrary::discover();
    let st = lib.stats();
    eprintln!("discover: {:?}\n{st:?}", t.elapsed());
    if lib.catalogs().is_empty() {
        eprintln!("skipping: no Chief registry on this machine");
        return;
    }
    assert!(lib.catalogs().len() >= 400, "got {}", lib.catalogs().len());
    assert!(st.core >= 11);
    // Second call uses the on-disk uuid cache.
    let t = Instant::now();
    let _ = ChiefLibrary::discover();
    eprintln!("discover (cached): {:?}", t.elapsed());
}

#[test]
#[ignore = "opens every installed catalog (~27 GB, read lazily); takes a while"]
fn every_installed_catalog_opens_and_lists() {
    let lib = ChiefLibrary::discover();
    if lib.catalogs().is_empty() {
        eprintln!("skipping: no Chief registry on this machine");
        return;
    }
    let t0 = Instant::now();
    let (mut ok, mut objects, mut failures) = (0usize, 0usize, Vec::new());
    for (i, e) in lib.catalogs().iter().enumerate() {
        if e.path.is_none() {
            continue;
        }
        let t = Instant::now();
        let res = lib.open(i).and_then(|c| {
            let mut it = c.objects()?;
            let n = it.by_ref().count();
            match it.take_error() {
                Some(err) => Err(err),
                None => {
                    c.category_tree()?;
                    Ok(n)
                }
            }
        });
        match res {
            Ok(n) => {
                ok += 1;
                objects += n;
                if t.elapsed().as_secs_f64() > 2.0 {
                    eprintln!("slow: {} ({} objects) {:?}", e.name, n, t.elapsed());
                }
            }
            Err(err) => failures.push(format!("{}: {err}", e.name)),
        }
    }
    eprintln!(
        "{ok} catalogs ok, {objects} objects, {} failures in {:?}",
        failures.len(),
        t0.elapsed()
    );
    for f in &failures {
        eprintln!("FAILED {f}");
    }
    assert!(failures.is_empty(), "{} catalogs failed", failures.len());
}

#[test]
#[ignore = "needs a Chief Architect X18 install"]
fn search_finds_doors() {
    let lib = ChiefLibrary::discover();
    if lib.catalogs().is_empty() {
        return;
    }
    let t = Instant::now();
    let hits = lib.search("door", 25);
    eprintln!("search 'door' -> {} hits in {:?}", hits.len(), t.elapsed());
    assert_eq!(hits.len(), 25);
}

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}
