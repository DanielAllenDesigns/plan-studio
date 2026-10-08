//! Writes the redacted template inventory JSON.
//!
//! `cargo run -p plan-chiefplan --example inventory -- [templates dir] <out.json>`
//! Without a directory argument Daniel's Chief X18 Templates folder is used.

use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out) = args.pop() else {
        eprintln!("usage: inventory [templates dir] <out.json>");
        std::process::exit(2);
    };
    let dir = args
        .pop()
        .map(PathBuf::from)
        .or_else(plan_chiefplan::templates_dir)
        .expect("no templates dir (set HOME or pass one)");
    match plan_chiefplan::write_inventory_json(&dir, &PathBuf::from(&out)) {
        Ok(n) => println!("wrote {n} template inventories to {out}"),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
