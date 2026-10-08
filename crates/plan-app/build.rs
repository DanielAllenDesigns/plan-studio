//! Lists the manual's chapters (`docs/manual/*.md`) for the in-app Help viewer.
//!
//! The Markdown is embedded with `include_str!` so the viewer works from a
//! bare binary; this script writes the list (`manual_chapters.rs` in
//! `OUT_DIR`) so a new chapter appears in Help without any code change.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets this"));
    let dir = manifest.join("..").join("..").join("docs").join("manual");
    println!("cargo:rerun-if-changed={}", dir.display());
    println!("cargo:rerun-if-changed=build.rs");

    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut out = String::from("/// (file name, Markdown text) of every manual chapter.\n");
    out.push_str("pub const CHAPTERS: &[(&str, &str)] = &[\n");
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned());
        let (Some(name), Some(path)) = (name, f.to_str()) else {
            continue;
        };
        // `{:?}` escapes backslashes in Windows paths.
        out.push_str(&format!("    ({name:?}, include_str!({path:?})),\n"));
    }
    out.push_str("];\n");

    let target = PathBuf::from(env::var("OUT_DIR").expect("cargo sets this"));
    fs::write(target.join("manual_chapters.rs"), out).expect("write manual_chapters.rs");
}
