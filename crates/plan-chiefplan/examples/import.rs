//! Imports one Chief project `.plan` and prints the report.
//!
//! ```text
//! cargo run --release -p plan-chiefplan --example import -- House.plan [--json] [--no-seed] [--project out.json]
//! ```
//!
//! The file is read in place; nothing is copied or written.

use plan_chiefplan::import::{import_plan, ImportOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: import <file.plan> [--json] [--no-seed] [--project out.json]");
        std::process::exit(2);
    };
    let opts = ImportOptions {
        seed_from_file: !args.iter().any(|a| a == "--no-seed"),
        ..ImportOptions::default()
    };
    let out_project = args
        .iter()
        .position(|a| a == "--project")
        .and_then(|i| args.get(i + 1));
    let t = std::time::Instant::now();
    match import_plan(path, &opts) {
        Ok(r) => {
            if let Some(out) = out_project {
                std::fs::write(out, r.project.to_json().unwrap()).unwrap();
            }
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&r.report).unwrap());
            } else {
                println!("{}", r.report.summary());
                for f in &r.report.floors {
                    println!(
                        "  {:<12} elev {:>8.3}  ceil {:>8.3}  walls {:>3}  openings {:>3}  rooms {:>2}  dims {:>3}",
                        f.name, f.elevation, f.ceiling_height, f.walls, f.openings, f.rooms, f.dimensions
                    );
                }
                for s in r.report.skipped_classes.iter().take(12) {
                    println!(
                        "  skipped class {:>3}.{} x{:<5} {} ({})",
                        s.class, s.version, s.count, s.label, s.confidence
                    );
                }
            }
            eprintln!("{:.1}s", t.elapsed().as_secs_f64());
        }
        Err(e) => {
            eprintln!("import failed: {e}");
            std::process::exit(1);
        }
    }
}
