//! Prints what the Phase C decoders read from one template (names and
//! numbers only).
//!
//! `cargo run -p plan-chiefplan --example summary -- <file.plan|file.layout> [--json]`
//! Without a path, Daniel's default plan template is used.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(std::path::PathBuf::from)
        .or_else(|| {
            plan_chiefplan::templates_dir().map(|d| d.join(plan_chiefplan::DEFAULT_PLAN_TEMPLATE))
        })
        .expect("no path (set HOME or pass a file)");
    let summary = match plan_chiefplan::summarize(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&summary).expect("json"));
        return;
    }
    println!(
        "{}: {} objects, {} materials ({} named), {} wall types, {} text styles, {} rich text, {} dimension sets, {} paper sizes",
        summary.file_name,
        summary.object_count,
        summary.materials.len(),
        summary.materials.iter().filter(|m| !m.name.is_empty()).count(),
        summary.wall_types.len(),
        summary.text_styles.len(),
        summary.rich_text_defaults.len(),
        summary.dimension_defaults.len(),
        summary.paper_sizes.len()
    );
    for w in &summary.wall_types {
        let layers: Vec<String> = w
            .layers
            .iter()
            .map(|(m, t, main)| format!("{m} {t}{}", if *main { " [main]" } else { "" }))
            .collect();
        println!(
            "  wall {} = {:.4}: {}",
            w.name,
            w.total_thickness_in,
            layers.join(" | ")
        );
    }
    for t in &summary.text_styles {
        println!(
            "  text {} : {} {} {} bold={} guid={}",
            t.name, t.font, t.font_style, t.height_in, t.bold, t.guid
        );
    }
    for d in &summary.dimension_defaults {
        println!("  dim {d:?}");
    }
    for r in &summary.rich_text_defaults {
        println!("  rich {r:?}");
    }
    println!("  heights {:?}", summary.default_heights);
    for p in &summary.paper_sizes {
        println!("  paper {p:?}");
    }
    for m in &summary.default_materials {
        println!("  default material {m:?}");
    }
    if let Some(l) = &summary.layout {
        println!("  layout {l:?}");
    }
}
