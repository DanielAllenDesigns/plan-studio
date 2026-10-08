//! Prints the object tree summary of a Chief `.plan`: how many objects of
//! each class, which classes the importer knows, and how the floors nest.
//!
//! ```text
//! cargo run --release -p plan-chiefplan --example object_survey -- House.plan
//! ```

use plan_chiefplan::import::floors::{class_label, floor_nodes};
use plan_chiefplan::import::tree::ObjectTree;
use std::collections::BTreeMap;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: object_survey <file.plan>");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).expect("read");
    let tree = ObjectTree::build(&bytes);
    println!(
        "{} objects accepted, {} rejected as chance matches",
        tree.len(),
        tree.rejected
    );
    let mut hist: BTreeMap<(u8, u8), (usize, usize)> = BTreeMap::new();
    for n in &tree.nodes {
        let e = hist.entry((n.class, n.version)).or_insert((0, 0));
        e.0 += 1;
        e.1 += n.len();
    }
    let mut rows: Vec<_> = hist.into_iter().collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1 .0));
    println!("class.ver     count      bytes  label");
    for ((c, v), (count, size)) in rows.iter().take(40) {
        let label = class_label(*c, *v)
            .map(|(l, conf)| format!("{l} ({conf})"))
            .unwrap_or_default();
        println!("{c:>5}.{v:<3} {count:>9} {size:>10}  {label}");
    }
    for (i, f) in floor_nodes(&tree).into_iter().enumerate() {
        let n = tree.node(f);
        let inside = tree
            .nodes
            .iter()
            .filter(|m| m.marker > n.marker && m.end <= n.end)
            .count();
        println!(
            "floor object {i}: {} bytes, {inside} objects inside",
            n.len()
        );
    }
}
