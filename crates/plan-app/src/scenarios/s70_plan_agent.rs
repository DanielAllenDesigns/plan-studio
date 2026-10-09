//! The Plan Agent dock, its Preferences page and the apply-as-one-undo-step
//! rule. Headless and offline: the agent's events are fed in by hand, no run
//! is started.

use super::Sim;
use crate::dialogs::preferences::agent_page::{self, AgentPrefs};
use crate::dialogs::preferences::pages::{self, PagePrefs, PrefsFile};
use crate::shell::agent_panel::{self, AgentPanelState};
use crate::toolbar::{Action, Dock};
use eframe::egui;
use plan_agent::{AgentEvent, Effort, Parameter};
use plan_core::geometry::Point;
use plan_core::model::WallKind;
use std::sync::Mutex;

/// The tests that read or write the process environment run one at a time.
static ENV: Mutex<()> = Mutex::new(());

/// Every string a frame painted.
fn painted_text(full: &egui::FullOutput) -> String {
    fn walk(shape: &egui::epaint::Shape, out: &mut String) {
        match shape {
            egui::epaint::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for c in &full.shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn screen() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    }
}

/// One frame of the right-hand dock; the text it painted.
fn dock_frame(sim: &mut Sim) -> String {
    let full = sim.ctx.run(screen(), |ctx| {
        crate::shell::docks::panel(
            ctx,
            Dock::Agent,
            &mut sim.app.cx,
            &mut sim.app.docks,
            &mut sim.app.settings,
        );
    });
    painted_text(&full)
}

fn with_one_more_wall(sim: &Sim) -> plan_core::model::Project {
    let mut p = sim.app.cx.project.clone();
    p.add_wall(
        0,
        Point::new(0.0, 0.0),
        Point::new(240.0, 0.0),
        6.0,
        108.0,
        WallKind::Exterior,
    );
    p
}

fn param(name: &str, value: f64) -> Parameter {
    Parameter {
        name: name.into(),
        label: "Garage width".into(),
        value,
        min: 10.0,
        max: 40.0,
        unit: "ft".into(),
        description: "Width of the garage".into(),
    }
}

#[test]
fn the_action_opens_the_dock_and_asks_for_a_key() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("ANTHROPIC_API_KEY", "");
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "");
    let mut sim = Sim::new();
    assert_eq!(sim.app.dock, None);
    sim.action(Action::ToggleDock(Dock::Agent));
    assert_eq!(sim.app.dock, Some(Dock::Agent));
    assert_eq!(Dock::Agent.title(), "Plan Agent");
    assert!(!agent_page::config().credentials_available());
    let text = dock_frame(&mut sim);
    assert!(
        text.contains("Ask to make changes") || text.contains("Plan Agent"),
        "{text}"
    );
    assert!(text.contains(agent_panel::NO_KEY_MESSAGE), "{text}");
    // Nothing starts without a key.
    assert!(!agent_panel::start_run(
        &mut sim.app.docks.agent,
        &sim.app.cx,
        "add a garage"
    ));
    assert!(sim.app.docks.agent.handle.is_none());
    // A second click closes it.
    sim.action(Action::ToggleDock(Dock::Agent));
    assert_eq!(sim.app.dock, None);
}

#[test]
fn a_changed_result_is_one_undo_step() {
    let mut sim = Sim::new();
    let before = sim.floor_walls();
    let project = with_one_more_wall(&sim);
    assert!(!sim.app.cx.can_undo());

    let mut st = AgentPanelState::default();
    let applied = agent_panel::process_event(
        &mut st,
        &mut sim.app.cx,
        AgentEvent::Finished {
            project,
            changed: true,
            assistant_text: "Added a wall. It is eight feet long.".into(),
        },
    );
    assert!(applied);
    assert_eq!(sim.floor_walls(), before + 1);
    assert_eq!(sim.app.cx.undo_label(), Some("Plan Agent"));
    assert_eq!(sim.app.cx.status, "Plan Agent: Added a wall.");
    assert!(st.pending_project.is_none());

    assert_eq!(sim.undo().as_deref(), Some("Plan Agent"));
    assert_eq!(sim.floor_walls(), before);
    assert!(!sim.app.cx.can_undo(), "exactly one undo step");
    sim.redo();
    assert_eq!(sim.floor_walls(), before + 1);
}

#[test]
fn an_unchanged_result_leaves_no_undo_step() {
    let mut sim = Sim::new();
    let before = sim.floor_walls();
    let project = with_one_more_wall(&sim);
    let mut st = AgentPanelState::default();
    let applied = agent_panel::process_event(
        &mut st,
        &mut sim.app.cx,
        AgentEvent::Finished {
            project,
            changed: false,
            assistant_text: "Nothing to do.".into(),
        },
    );
    assert!(!applied);
    assert_eq!(sim.floor_walls(), before);
    assert!(!sim.app.cx.can_undo());
}

#[test]
fn failures_and_refusals_only_leave_a_notice() {
    let mut sim = Sim::new();
    let before = sim.floor_walls();
    let mut st = AgentPanelState::default();
    for ev in [
        AgentEvent::Failed("network down".into()),
        AgentEvent::Refused("no".into()),
        AgentEvent::Cancelled,
    ] {
        assert!(!agent_panel::process_event(&mut st, &mut sim.app.cx, ev));
        assert!(st.notice.is_some());
    }
    assert_eq!(sim.floor_walls(), before);
    assert!(!sim.app.cx.can_undo());
}

#[test]
fn moving_a_slider_marks_the_tweaks_dirty() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("ANTHROPIC_API_KEY", "");
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "");
    let mut sim = Sim::new();
    let mut st = AgentPanelState::default();
    agent_panel::handle_event(
        &mut st,
        AgentEvent::ParametersChanged(vec![param("garage_width_ft", 24.0)]),
    );
    assert_eq!(st.params.len(), 1);
    assert!(!st.sliders_dirty);
    // An unknown name changes nothing.
    assert!(!agent_panel::set_slider(&mut st, "nope", 1.0));
    assert!(!st.sliders_dirty);

    assert!(agent_panel::set_slider(&mut st, "garage_width_ft", 28.0));
    assert!(st.sliders_dirty);
    assert_eq!(st.params[0].value, 28.0);
    // Values are held inside min..max.
    agent_panel::set_slider(&mut st, "garage_width_ft", 99.0);
    assert_eq!(st.params[0].value, 40.0);

    // "Apply changes" needs a key; without one nothing runs and the sliders
    // stay dirty.
    assert!(!agent_panel::apply_sliders(&mut st, &sim.app.cx));
    assert!(st.sliders_dirty);
    assert!(st.handle.is_none());

    // The dock draws the sliders.
    sim.app.docks.agent = st;
    sim.action(Action::ToggleDock(Dock::Agent));
    let text = dock_frame(&mut sim);
    assert!(text.contains("Tweaks"), "{text}");
    assert!(text.contains("Apply changes"), "{text}");
}

#[test]
fn the_effort_choice_is_kept_in_the_preferences() {
    pages::set(PagePrefs::default());
    assert_eq!(agent_page::config().effort, Effort::Medium);
    agent_page::set_effort(&Effort::XHigh);
    assert_eq!(agent_page::prefs().agent_effort, "xhigh");
    assert_eq!(agent_page::config().effort, Effort::XHigh);
    pages::set(PagePrefs::default());
}

#[test]
fn the_key_and_model_reach_the_agent_config() {
    pages::set(PagePrefs::default());
    assert_eq!(agent_page::config().model, "claude-opus-5-5");
    assert!(agent_page::config().api_key.is_none());
    pages::update(|p| {
        p.agent.agent_api_key = "  sk-test  ".into();
        p.agent.agent_model = "claude-test".into();
    });
    let cfg = agent_page::config();
    assert_eq!(cfg.api_key.as_deref(), Some("sk-test"));
    assert_eq!(cfg.model, "claude-test");
    assert!(cfg.credentials_available());
    pages::set(PagePrefs::default());
}

#[test]
fn the_three_settings_round_trip_and_default_when_missing() {
    // Defaults.
    let d = PrefsFile::default();
    assert_eq!(d.pages.agent, AgentPrefs::default());
    assert_eq!(d.pages.agent.agent_api_key, "");
    assert_eq!(d.pages.agent.agent_model, "claude-opus-5-5");
    assert_eq!(d.pages.agent.agent_effort, "medium");

    // A file from before the agent existed loads with the defaults.
    let old = pages::parse_file(r#"{"version":1,"general":{"icon_halo":true}}"#).unwrap();
    assert_eq!(old.pages.agent, AgentPrefs::default());
    assert!(old.general.icon_halo);

    // The three keys sit at the top level of the file and survive a round
    // trip.
    let mut f = PrefsFile::default();
    f.pages.agent = AgentPrefs {
        agent_api_key: "sk-test".into(),
        agent_model: "claude-test".into(),
        agent_effort: "high".into(),
    };
    let text = serde_json::to_string(&f).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["agent_api_key"], "sk-test");
    assert_eq!(v["agent_model"], "claude-test");
    assert_eq!(v["agent_effort"], "high");
    assert_eq!(pages::parse_file(&text).unwrap(), f);

    // A partial file fills the rest.
    let part = pages::parse_file(r#"{"agent_effort":"low"}"#).unwrap();
    assert_eq!(part.pages.agent.agent_effort, "low");
    assert_eq!(part.pages.agent.agent_model, "claude-opus-5-5");
    // The key never shows in debug output.
    assert!(!format!("{f:?}").contains("sk-test"));
}

#[test]
fn the_preferences_page_draws_headlessly() {
    let ctx = egui::Context::default();
    let mut cx = crate::editor::EditorContext::new(crate::plan_defaults::embedded());
    let mut settings = crate::theme::AppSettings::default();
    let mut actions = Vec::new();
    crate::dialogs::preferences::open(crate::dialogs::preferences::Page::Agent);
    let full = ctx.run(screen(), |ctx| {
        crate::dialogs::preferences::show_all(ctx, &mut cx, &mut settings, &mut actions);
    });
    let text = painted_text(&full);
    assert!(text.contains("Anthropic API key"), "{text}");
    assert!(text.contains("ANTHROPIC_API_KEY"), "{text}");
}
