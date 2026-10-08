//! Prints `docs/chief-hotkeys-resolved.md`:
//! `cargo run -p plan-config --example gen_hotkeys_md > docs/chief-hotkeys-resolved.md`

fn main() {
    let cfg = plan_config::load_daniel_config();
    print!("{}", plan_config::resolved_markdown(&cfg.hotkeys));
}
