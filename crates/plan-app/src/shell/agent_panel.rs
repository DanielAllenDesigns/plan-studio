//! The Plan Agent dock: "Ask to make changes". The user types a request, the
//! agent (`plan-agent`, Claude with tools that edit the plan model) works on
//! a CLONE of the project on a background thread, and the result lands in the
//! editor as ONE undo step named "Plan Agent".
//!
//! Top to bottom: the transcript (user, assistant, thinking and tool rows),
//! the "Tweaks" sliders for the parameters the agent exposed, the request box
//! with the effort choice, Send and Stop, and the usage footer.
//!
//! The panel never touches `cx.project` while a run is in progress; [`pump`]
//! (called once a frame by the shell, whether the dock is open or not) drains
//! the run's events and, for a finished run that changed the plan, calls
//! [`apply_finished`].

use super::docks::DockState;
use crate::dialogs::preferences::{self, agent_page};
use crate::editor::EditorContext;
use eframe::egui::{self, Key, Modifiers, RichText};
use plan_agent::{AgentEvent, AgentSession, Effort, Parameter, RunHandle, TranscriptEntry, Usage};
use plan_core::model::Project;
use std::time::Duration;

/// The request text the "Apply changes" button sends.
pub const APPLY_TEXT: &str = "Apply the parameter changes";
/// The undo step's name.
pub const UNDO_LABEL: &str = "Plan Agent";
/// The message shown when no key is available.
pub const NO_KEY_MESSAGE: &str = "Set your Anthropic API key in Preferences \u{25b8} Plan Agent";

/// What the dock keeps between frames.
pub struct AgentPanelState {
    pub session: AgentSession,
    pub handle: Option<RunHandle>,
    pub input: String,
    pub effort: Effort,
    /// A finished project waiting to be applied (set by [`handle_event`],
    /// taken by [`process_event`]).
    pub pending_project: Option<Project>,
    /// The assistant's closing text for `pending_project`.
    pending_text: String,
    /// A slider moved since the last run.
    pub sliders_dirty: bool,
    /// The tweaks as the agent last published them (with the user's moves).
    pub params: Vec<Parameter>,
    /// The saved effort key last adopted from the Preferences page.
    effort_seen: String,
    /// The last error or refusal, shown above the request box.
    pub notice: Option<String>,
    /// Put the keyboard focus in the request box on the next frame.
    focus_input: bool,
}

impl Default for AgentPanelState {
    fn default() -> Self {
        Self {
            session: AgentSession::new(),
            handle: None,
            input: String::new(),
            effort: Effort::Medium,
            pending_project: None,
            pending_text: String::new(),
            sliders_dirty: false,
            params: Vec::new(),
            effort_seen: String::new(),
            notice: None,
            focus_input: false,
        }
    }
}

impl AgentPanelState {
    /// Is a run in progress?
    pub fn running(&self) -> bool {
        self.handle.as_ref().is_some_and(RunHandle::is_running)
    }
}

// ----- the frame loop -----

/// Drains the running agent's events and applies a finished result to the
/// editor. The shell calls it every frame (an agent keeps working while the
/// dock is closed). Returns true when the plan was replaced, so the shell can
/// restart the active tool.
pub fn pump(st: &mut AgentPanelState, cx: &mut EditorContext, ctx: &egui::Context) -> bool {
    let Some(handle) = st.handle.as_mut() else {
        return false;
    };
    // Drain first, then look: `is_running` stays true until the terminal
    // event has been drained, so the last events are never lost.
    let events = handle.drain(&mut st.session);
    if !handle.is_running() {
        st.handle = None;
    }
    let mut applied = false;
    for ev in events {
        applied |= process_event(st, cx, ev);
    }
    if st.handle.is_some() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    applied
}

/// [`handle_event`], then applies a finished project to the editor. True
/// when the plan was replaced.
pub fn process_event(st: &mut AgentPanelState, cx: &mut EditorContext, ev: AgentEvent) -> bool {
    handle_event(st, ev);
    match st.pending_project.take() {
        Some(project) => {
            let mut text = std::mem::take(&mut st.pending_text);
            if text.trim().is_empty() {
                text = last_assistant_text(&st.session);
            }
            apply_finished(cx, project, &text);
            true
        }
        None => false,
    }
}

/// Applies the bookkeeping of one event (the session itself was updated by
/// `drain`). A finished run that changed the plan leaves its project in
/// `pending_project`.
pub fn handle_event(st: &mut AgentPanelState, ev: AgentEvent) {
    match ev {
        AgentEvent::ParametersChanged(p) => st.params = p,
        AgentEvent::Finished {
            project,
            changed,
            assistant_text,
        } => {
            st.notice = None;
            st.sliders_dirty = false;
            if changed {
                st.pending_project = Some(project);
                st.pending_text = assistant_text;
            }
        }
        AgentEvent::Refused(m) => st.notice = Some(format!("The model declined: {m}")),
        AgentEvent::Failed(m) => st.notice = Some(m),
        AgentEvent::Cancelled => st.notice = Some("Stopped.".into()),
        AgentEvent::TextDelta(_)
        | AgentEvent::ThinkingDelta(_)
        | AgentEvent::ToolStarted { .. }
        | AgentEvent::ToolFinished { .. }
        | AgentEvent::Usage(_) => {}
    }
}

/// Swaps the agent's project into the editor as ONE undo step and writes the
/// status line. The run edited a clone; nothing else touched the plan.
pub fn apply_finished(cx: &mut EditorContext, project: Project, assistant_text: &str) {
    cx.begin_change(UNDO_LABEL);
    cx.project = project;
    cx.floor = cx.floor.min(cx.project.floors.len().saturating_sub(1));
    // The agent joins walls by position only (DECISIONS AG5): run Fix Wall
    // Connections on every floor, inside the same undo step.
    let opts = crate::editor::connect::ConnectOptions::from_defaults(&cx.defaults);
    for floor in 0..cx.project.floors.len() {
        crate::editor::connect::fix_all_connections_project(&mut cx.project, floor, &opts);
    }
    cx.mark_dirty();
    // Ids in the old selection may be gone.
    cx.reset_view_state();
    cx.refresh();
    let first = first_sentence(assistant_text);
    cx.status = if first.is_empty() {
        "Plan Agent: plan updated".to_string()
    } else {
        format!("Plan Agent: {first}")
    };
}

/// The first sentence of `text`, at most 140 characters.
pub fn first_sentence(text: &str) -> String {
    let text = text.trim();
    let mut end = text.len();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (n, (i, c)) in chars.iter().enumerate() {
        let next_is_gap = chars.get(n + 1).is_none_or(|(_, d)| d.is_whitespace());
        if matches!(c, '.' | '!' | '?') && next_is_gap {
            end = i + c.len_utf8();
            break;
        }
        if *c == '\n' {
            end = *i;
            break;
        }
    }
    let s = text[..end].trim();
    if s.chars().count() > 140 {
        let cut: String = s.chars().take(137).collect();
        format!("{}...", cut.trim_end())
    } else {
        s.to_string()
    }
}

fn last_assistant_text(session: &AgentSession) -> String {
    session
        .transcript()
        .iter()
        .rev()
        .find_map(|e| match e {
            TranscriptEntry::Assistant(t) if !t.trim().is_empty() => Some(t.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

// ----- actions -----

/// Starts a run with `text` on a clone of the plan. False when a run is
/// already going, the text is empty or no key is available.
pub fn start_run(st: &mut AgentPanelState, cx: &EditorContext, text: &str) -> bool {
    let text = text.trim();
    if st.running() || text.is_empty() {
        return false;
    }
    let cfg = agent_page::config_with(st.effort);
    if !cfg.credentials_available() {
        return false;
    }
    st.notice = None;
    let handle = st.session.run(&cfg, cx.project.clone(), &cx.defaults, text);
    st.handle = Some(handle);
    true
}

/// The user moved the slider `name` to `value`. Marks the sliders dirty; the
/// next run tells the model about the new value.
pub fn set_slider(st: &mut AgentPanelState, name: &str, value: f64) -> bool {
    let Some(p) = st.params.iter_mut().find(|p| p.name == name) else {
        return false;
    };
    p.value = value.clamp(p.min.min(p.max), p.max.max(p.min));
    let v = p.value;
    st.sliders_dirty = true;
    st.session.set_parameter_value(name, v);
    true
}

/// "Apply changes": runs the agent on the moved sliders.
pub fn apply_sliders(st: &mut AgentPanelState, cx: &EditorContext) -> bool {
    if !st.sliders_dirty {
        return false;
    }
    let started = start_run(st, cx, APPLY_TEXT);
    if started {
        st.sliders_dirty = false;
    }
    started
}

/// Adopts the effort saved on the Preferences page when it changed there.
fn sync_effort(st: &mut AgentPanelState) {
    let saved = agent_page::prefs().agent_effort;
    if saved != st.effort_seen {
        st.effort = agent_page::effort_from_key(&saved);
        st.effort_seen = saved;
    }
}

// ----- drawing -----

/// The dock body.
pub fn show(ui: &mut egui::Ui, st: &mut DockState, cx: &mut EditorContext) {
    let st = &mut st.agent;
    sync_effort(st);
    let credentials = agent_page::config().credentials_available();
    let running = st.running();

    ui.horizontal(|ui| {
        ui.weak(agent_page::config().model);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!running, egui::Button::new("Clear").small())
                .on_hover_text("Start a new conversation")
                .clicked()
            {
                st.session.clear();
                st.params.clear();
                st.sliders_dirty = false;
                st.notice = None;
            }
        });
    });

    egui::TopBottomPanel::bottom("agent_panel_bottom")
        .resizable(false)
        .show_separator_line(true)
        .show_inside(ui, |ui| bottom(ui, st, cx, credentials, running));

    egui::ScrollArea::vertical()
        .id_salt("agent_transcript")
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| transcript(ui, st, running));
}

fn bottom(
    ui: &mut egui::Ui,
    st: &mut AgentPanelState,
    cx: &mut EditorContext,
    credentials: bool,
    running: bool,
) {
    ui.add_space(4.0);
    if !st.params.is_empty() {
        tweaks(ui, st, cx);
    }
    if let Some(n) = st.notice.clone() {
        ui.colored_label(egui::Color32::from_rgb(0xE5, 0x73, 0x73), n);
    }
    if !credentials {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(egui::Color32::from_rgb(0xE5, 0x73, 0x73), NO_KEY_MESSAGE);
            if ui.link("Open Preferences").clicked() {
                preferences::open(preferences::Page::Agent);
            }
        });
    }

    // Ctrl/Cmd+Enter sends. Take the key before the text box sees it.
    let input_id = egui::Id::new("agent_input");
    let mut send = false;
    if ui.memory(|m| m.has_focus(input_id))
        && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter))
    {
        send = true;
    }
    let edit = egui::TextEdit::multiline(&mut st.input)
        .id(input_id)
        .hint_text("Ask to make changes")
        .desired_rows(3)
        .desired_width(f32::INFINITY);
    let resp = ui.add_enabled(credentials && !running, edit);
    if st.focus_input {
        resp.request_focus();
        st.focus_input = false;
    }

    ui.horizontal(|ui| {
        let before = st.effort;
        egui::ComboBox::from_id_salt("agent_effort")
            .selected_text(agent_page::effort_label(&st.effort))
            .width(90.0)
            .show_ui(ui, |ui| {
                for (key, label) in agent_page::EFFORTS {
                    let e = agent_page::effort_from_key(key);
                    let cur = e == st.effort;
                    if ui.selectable_label(cur, label).clicked() {
                        st.effort = e;
                    }
                }
            })
            .response
            .on_hover_text("How hard the model thinks");
        if st.effort != before {
            agent_page::set_effort(&st.effort);
            st.effort_seen = agent_page::effort_key(&st.effort).to_string();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if running {
                if ui.button("Stop").clicked() {
                    if let Some(h) = &st.handle {
                        h.cancel();
                    }
                }
            } else {
                let can = credentials && !st.input.trim().is_empty();
                if ui
                    .add_enabled(can, egui::Button::new("Send"))
                    .on_hover_text("Ctrl/\u{2318}+Enter")
                    .clicked()
                {
                    send = true;
                }
            }
        });
    });
    if send && credentials && !running && !st.input.trim().is_empty() {
        let text = std::mem::take(&mut st.input);
        if start_run(st, cx, &text) {
            st.focus_input = true;
        } else {
            st.input = text;
        }
    }

    ui.add_space(2.0);
    ui.weak(usage_line(st.session.usage(), &agent_page::config().model));
    ui.add_space(2.0);
}

fn tweaks(ui: &mut egui::Ui, st: &mut AgentPanelState, cx: &EditorContext) {
    ui.strong("Tweaks");
    let mut moved: Vec<(String, f64)> = Vec::new();
    for p in &st.params {
        let mut v = p.value;
        let label = if p.label.is_empty() {
            &p.name
        } else {
            &p.label
        };
        let mut slider = egui::Slider::new(&mut v, p.min.min(p.max)..=p.max.max(p.min)).text(label);
        if !p.unit.is_empty() {
            slider = slider.suffix(format!(" {}", p.unit));
        }
        let resp = ui.add_enabled(!st.running(), slider);
        let resp = if p.description.is_empty() {
            resp
        } else {
            resp.on_hover_text(&p.description)
        };
        if resp.changed() {
            moved.push((p.name.clone(), v));
        }
    }
    for (name, v) in moved {
        set_slider(st, &name, v);
    }
    let can = st.sliders_dirty && !st.running() && agent_page::config().credentials_available();
    if ui
        .add_enabled(can, egui::Button::new("Apply changes"))
        .clicked()
    {
        apply_sliders(st, cx);
    }
    ui.separator();
}

fn transcript(ui: &mut egui::Ui, st: &AgentPanelState, running: bool) {
    let entries = st.session.transcript();
    if entries.is_empty() && !running {
        ui.add_space(8.0);
        ui.weak("Describe a change to the plan, for example:");
        ui.weak(
            "\"Add a 24' x 30' two-car garage on the right side with a door into the mudroom.\"",
        );
        ui.weak("The whole request is one undo step.");
        return;
    }
    let mut i = 0;
    while i < entries.len() {
        match &entries[i] {
            TranscriptEntry::User(t) => {
                user_bubble(ui, t);
                i += 1;
            }
            TranscriptEntry::Assistant(t) => {
                ui.add_space(4.0);
                ui.add(egui::Label::new(t.as_str()).selectable(true).wrap());
                i += 1;
            }
            TranscriptEntry::Thinking(t) => {
                if !t.trim().is_empty() {
                    egui::CollapsingHeader::new(RichText::new("Thinking\u{2026}").weak())
                        .id_salt(("agent_thinking", i))
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(t.as_str()).weak().small())
                                    .selectable(true)
                                    .wrap(),
                            );
                        });
                }
                i += 1;
            }
            TranscriptEntry::ToolCall { .. } => {
                let start = i;
                while i < entries.len() && matches!(entries[i], TranscriptEntry::ToolCall { .. }) {
                    i += 1;
                }
                let trailing = i == entries.len();
                tool_group(ui, &entries[start..i], start, trailing && running);
            }
            TranscriptEntry::Error(t) => {
                ui.colored_label(egui::Color32::from_rgb(0xE5, 0x73, 0x73), t.as_str());
                i += 1;
            }
        }
    }
    if running {
        let last_is_tools = matches!(entries.last(), Some(TranscriptEntry::ToolCall { .. }));
        if !last_is_tools {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Working\u{2026}");
            });
        }
    }
}

fn user_bubble(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(6)
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(text).selectable(true).wrap());
        });
}

fn tool_group(ui: &mut egui::Ui, rows: &[TranscriptEntry], start: usize, live: bool) {
    let title = if live {
        "Working\u{2026}".to_string()
    } else {
        format!(
            "{} step{}",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        )
    };
    ui.horizontal(|ui| {
        if live {
            ui.spinner();
        }
        egui::CollapsingHeader::new(RichText::new(title).weak())
            .id_salt(("agent_tools", start))
            .default_open(live)
            .open(live.then_some(true))
            .show(ui, |ui| {
                for r in rows {
                    if let TranscriptEntry::ToolCall { name, summary, ok } = r {
                        ui.label(RichText::new(tool_line(name, summary, *ok)).weak().small());
                    }
                }
            });
    });
}

/// `\u{2713} add_wall_rectangle  30' x 40' exterior` or `\u{2717} name  (error)`.
pub fn tool_line(name: &str, summary: &str, ok: bool) -> String {
    if ok {
        format!("\u{2713} {name}  {summary}")
    } else {
        format!("\u{2717} {name}  ({summary})")
    }
}

/// `12.3k` for 12 345, `850` for 850.
pub fn compact(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// `Usage: 12.3k in / 2.1k out \u{b7} est. $0.08`.
pub fn usage_line(u: &Usage, model: &str) -> String {
    format!(
        "Usage: {} in / {} out \u{b7} est. ${:.2}",
        compact(u.input_tokens),
        compact(u.output_tokens),
        u.estimated_cost_usd(model)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sentence_stops_at_the_period() {
        assert_eq!(
            first_sentence("Added the garage. Then more."),
            "Added the garage."
        );
        assert_eq!(first_sentence("Done"), "Done");
        assert_eq!(
            first_sentence("  Wide 3.5 inch wall. x"),
            "Wide 3.5 inch wall."
        );
        assert_eq!(first_sentence("Line one\nLine two"), "Line one");
        assert_eq!(first_sentence(""), "");
        assert!(first_sentence(&"x".repeat(300)).chars().count() <= 140);
    }

    #[test]
    fn tool_rows_and_usage_read_like_the_brief() {
        assert_eq!(
            tool_line("add_wall_rectangle", "30' x 40' exterior", true),
            "\u{2713} add_wall_rectangle  30' x 40' exterior"
        );
        assert_eq!(
            tool_line("add_opening", "no such wall", false),
            "\u{2717} add_opening  (no such wall)"
        );
        assert_eq!(compact(850), "850");
        assert_eq!(compact(12_345), "12.3k");
        let u = Usage {
            input_tokens: 12_300,
            output_tokens: 2_100,
            ..Usage::default()
        };
        assert!(usage_line(&u, "claude-opus-5-5").starts_with("Usage: 12.3k in / 2.1k out"));
    }
}
