//! Live smoke test for the Plan Agent (spends real API credit).
//!
//! ```text
//! ANTHROPIC_API_KEY=... cargo run -p plan-agent --example smoke -- "Draw a 30 by 40 foot house"
//! ```
//! Runs one turn against a fresh project and prints the streamed text, the
//! tool calls, the wall count before and after, and the token usage.

use plan_agent::{AgentConfig, AgentEvent, AgentSession, Effort};
use plan_core::defaults::PlanDefaults;
use plan_core::model::Project;
use std::time::Duration;

fn main() {
    let text = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let text = if text.is_empty() {
        "Draw a 30 by 40 foot rectangular house with a front door on the long south side."
            .to_string()
    } else {
        text
    };
    let cfg = AgentConfig {
        effort: Effort::Low,
        ..AgentConfig::default()
    };
    if !cfg.credentials_available() {
        eprintln!("No credentials: set ANTHROPIC_API_KEY.");
        std::process::exit(2);
    }
    let defaults = PlanDefaults::default();
    let project = Project::from_defaults("Smoke", &defaults);
    let walls_before: usize = project.floors.iter().map(|f| f.walls.len()).sum();
    let mut session = AgentSession::new();
    let mut handle = session.run(&cfg, project, &defaults, &text);
    loop {
        let events = handle.drain(&mut session);
        for ev in events {
            match ev {
                AgentEvent::TextDelta(t) => print!("{t}"),
                AgentEvent::ToolStarted {
                    name,
                    input_summary,
                } => println!("\n[tool] {name} {input_summary}"),
                AgentEvent::ToolFinished {
                    name,
                    ok,
                    result_summary,
                } => {
                    println!(
                        "[tool] {name} -> {} {result_summary}",
                        if ok { "ok" } else { "ERR" }
                    )
                }
                AgentEvent::ParametersChanged(p) => println!("[params] {p:?}"),
                AgentEvent::Finished {
                    project, changed, ..
                } => {
                    let walls_after: usize = project.floors.iter().map(|f| f.walls.len()).sum();
                    println!("\n[done] changed={changed} walls {walls_before} -> {walls_after}");
                }
                AgentEvent::Refused(m) => println!("\n[refused] {m}"),
                AgentEvent::Failed(m) => println!("\n[failed] {m}"),
                AgentEvent::Cancelled => println!("\n[cancelled]"),
                _ => {}
            }
        }
        if !handle.is_running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let u = session.usage();
    println!(
        "[usage] in {} out {} cache_read {} turns {} est ${:.3}",
        u.input_tokens,
        u.output_tokens,
        u.cache_read_tokens,
        u.turns,
        u.estimated_cost_usd(&cfg.model)
    );
}
