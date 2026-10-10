//! Conversation state and the run loop.
//!
//! [`AgentSession`] keeps the append-only `messages` array (assistant content
//! echoed back verbatim, thinking blocks included), the transcript the UI
//! shows, the parameters ("tweaks") and the usage totals. [`AgentSession::run`]
//! starts one turn on a background thread; the editor polls the returned
//! [`RunHandle`] every frame with [`RunHandle::drain`].
//!
//! A run is atomic with respect to the history: the messages of a run are
//! committed to the session only when the run finishes normally, so a failed,
//! refused or cancelled run leaves the history valid and the plan untouched.

use crate::api::{self, ApiClient, ApiError, Auth, Delta, Request};
use crate::{prompt, tools};
use plan_core::defaults::PlanDefaults;
use plan_core::model::Project;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

/// How hard the model thinks (`output_config.effort`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
}

impl Effort {
    /// The API value: `low`, `medium`, `high` or `xhigh`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
        }
    }
}

#[derive(Clone)]
pub struct AgentConfig {
    /// `x-api-key`. `None` tries `ANTHROPIC_API_KEY`, then `ANTHROPIC_AUTH_TOKEN` (Bearer).
    pub api_key: Option<String>,
    pub model: String,
    pub effort: Effort,
    pub max_tool_rounds: usize,
    pub base_url: String,
}

impl std::fmt::Debug for AgentConfig {
    /// Never prints the key: `api_key` shows only whether one is set.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .field("effort", &self.effort)
            .field("max_tool_rounds", &self.max_tool_rounds)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "claude-opus-5-5".to_string(),
            effort: Effort::Medium,
            max_tool_rounds: 24,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }
}

impl AgentConfig {
    /// A key or token is available, from the config or the environment.
    pub fn credentials_available(&self) -> bool {
        api::resolve_auth_from_env(self.api_key.as_deref()).is_some()
    }
}

/// A "tweak" the agent exposed with the `define_parameter` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// snake_case id, e.g. `garage_width_ft`.
    pub name: String,
    /// Label for the slider, e.g. "Garage width".
    pub label: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    /// `ft`, `in`, `sqft`, `count` or empty.
    pub unit: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub enum TranscriptEntry {
    User(String),
    /// Final or partial text of one assistant turn.
    Assistant(String),
    /// Summarized thinking (may be empty).
    Thinking(String),
    /// One row per executed tool.
    ToolCall {
        name: String,
        summary: String,
        ok: bool,
    },
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub turns: u32,
}

impl Usage {
    /// Adds `o` to the totals.
    pub fn add(&mut self, o: &Usage) {
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cache_read_tokens += o.cache_read_tokens;
        self.cache_write_tokens += o.cache_write_tokens;
        self.turns += o.turns;
    }

    /// Estimated cost in US dollars. Claude Opus 5.5 pricing: $4 per million
    /// input tokens, $20 per million output tokens, cache reads $0.20 per
    /// million and cache writes 1.25 times the input price. Other models use
    /// the same rates (an estimate, not a bill).
    pub fn estimated_cost_usd(&self, _model: &str) -> f64 {
        const PER_M: f64 = 1_000_000.0;
        let input = 4.0;
        self.input_tokens as f64 * input / PER_M
            + self.output_tokens as f64 * 20.0 / PER_M
            + self.cache_read_tokens as f64 * 0.20 / PER_M
            + self.cache_write_tokens as f64 * input * 1.25 / PER_M
    }
}

// `Finished` carries the whole project (fixed by the shared contract) and is sent once per run.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum AgentEvent {
    /// Streamed assistant text.
    TextDelta(String),
    ThinkingDelta(String),
    ToolStarted {
        name: String,
        input_summary: String,
    },
    ToolFinished {
        name: String,
        ok: bool,
        result_summary: String,
    },
    ParametersChanged(Vec<Parameter>),
    /// The usage of ONE API response (an increment); [`RunHandle::drain`]
    /// adds it to [`AgentSession::usage`].
    Usage(Usage),
    Finished {
        project: Project,
        changed: bool,
        assistant_text: String,
    },
    /// The model declined (`stop_reason` refusal); the message to show.
    Refused(String),
    /// Transport or API error. Never contains the key.
    Failed(String),
    Cancelled,
}

impl AgentEvent {
    /// The last event of a run.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentEvent::Finished { .. }
                | AgentEvent::Refused(_)
                | AgentEvent::Failed(_)
                | AgentEvent::Cancelled
        )
    }
}

/// Internal channel traffic: events for the UI and history commits.
#[allow(clippy::large_enum_variant)]
enum Msg {
    Event(AgentEvent),
    /// New messages to append to the session history (sent once, right before `Finished`).
    Commit(Vec<Value>),
}

#[derive(Clone, Debug, Default)]
pub struct AgentSession {
    messages: Vec<Value>,
    transcript: Vec<TranscriptEntry>,
    parameters: Vec<Parameter>,
    usage: Usage,
    /// Slider changes since the last run, told to the model with the next request.
    pending_changes: Vec<(String, f64)>,
    /// Input summary of the tool being run, joined with its `ToolFinished`.
    tool_slot: Option<String>,
}

impl AgentSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transcript(&self) -> &[TranscriptEntry] {
        &self.transcript
    }

    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    pub fn usage(&self) -> &Usage {
        &self.usage
    }

    /// The raw message history (what the next request sends).
    pub fn messages(&self) -> &[Value] {
        &self.messages
    }

    /// A slider moved. The value is clamped to the parameter's range and the
    /// next run tells the model "parameter X is now Y". `false` for an
    /// unknown parameter.
    pub fn set_parameter_value(&mut self, name: &str, value: f64) -> bool {
        let Some(p) = self.parameters.iter_mut().find(|p| p.name == name) else {
            return false;
        };
        if !value.is_finite() {
            return false;
        }
        let v = value.clamp(p.min.min(p.max), p.max.max(p.min));
        if (p.value - v).abs() > f64::EPSILON {
            p.value = v;
            self.pending_changes.retain(|(n, _)| n != name);
            self.pending_changes.push((name.to_string(), v));
        }
        true
    }

    /// Forgets the conversation, parameters and usage.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Starts a turn on a background thread. `project` is a clone of the
    /// editor's project; the thread edits only that clone and hands it back in
    /// [`AgentEvent::Finished`]. Call [`RunHandle::drain`] every frame.
    pub fn run(
        &mut self,
        cfg: &AgentConfig,
        project: Project,
        defaults: &PlanDefaults,
        user_text: &str,
    ) -> RunHandle {
        self.run_with_client(cfg, project, defaults, user_text, ApiClient::ureq())
    }

    /// [`AgentSession::run`] with an explicit API client (tests pass a
    /// scripted transport).
    pub fn run_with_client(
        &mut self,
        cfg: &AgentConfig,
        project: Project,
        defaults: &PlanDefaults,
        user_text: &str,
        client: ApiClient,
    ) -> RunHandle {
        let mut blocks = vec![json!({
            "type": "text",
            "text": format!(
                "Current plan state:\n{}",
                prompt::plan_summary_with_params(&project, defaults, &self.parameters)
            ),
        })];
        if !self.pending_changes.is_empty() {
            let list: Vec<String> = self
                .pending_changes
                .iter()
                .map(|(n, v)| format!("{n}={v}"))
                .collect();
            blocks.push(json!({
                "type": "text",
                "text": format!("Parameter changes from the user: {}", list.join(", ")),
            }));
            self.pending_changes.clear();
        }
        blocks.push(json!({"type": "text", "text": user_text}));
        self.transcript
            .push(TranscriptEntry::User(user_text.to_string()));

        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let job = Job {
            cfg: cfg.clone(),
            auth: api::resolve_auth_from_env(cfg.api_key.as_deref()),
            client,
            history: self.messages.clone(),
            user_content: blocks,
            project,
            defaults: defaults.clone(),
            parameters: self.parameters.clone(),
            cancel: cancel.clone(),
        };
        thread::spawn(move || {
            let secrets = job.secrets();
            let tx2 = tx.clone();
            let outcome = catch_unwind(AssertUnwindSafe(move || run_job(job, &tx2)));
            if outcome.is_err() {
                let _ = tx.send(Msg::Event(AgentEvent::Failed(api::redact(
                    "The agent stopped because of an internal error.",
                    &secrets,
                ))));
            }
        });
        RunHandle {
            rx,
            cancel,
            done: false,
        }
    }

    /// Applies one event to the transcript, usage and parameters.
    fn apply(&mut self, ev: &AgentEvent) {
        match ev {
            AgentEvent::TextDelta(t) => match self.transcript.last_mut() {
                Some(TranscriptEntry::Assistant(s)) => s.push_str(t),
                _ => self.transcript.push(TranscriptEntry::Assistant(t.clone())),
            },
            AgentEvent::ThinkingDelta(t) => match self.transcript.last_mut() {
                Some(TranscriptEntry::Thinking(s)) => s.push_str(t),
                _ => self.transcript.push(TranscriptEntry::Thinking(t.clone())),
            },
            AgentEvent::ToolStarted { input_summary, .. } => {
                self.tool_slot = Some(input_summary.clone());
            }
            AgentEvent::ToolFinished {
                name,
                ok,
                result_summary,
            } => {
                let input = self.tool_slot.take().unwrap_or_default();
                let summary = if *ok {
                    input
                } else if input.is_empty() {
                    result_summary.clone()
                } else {
                    format!("{input}: {result_summary}")
                };
                self.transcript.push(TranscriptEntry::ToolCall {
                    name: name.clone(),
                    summary,
                    ok: *ok,
                });
            }
            AgentEvent::ParametersChanged(p) => self.parameters = p.clone(),
            AgentEvent::Usage(u) => self.usage.add(u),
            AgentEvent::Finished { assistant_text, .. } => {
                // Text normally arrived as deltas; make sure it is there.
                let answered = self
                    .transcript
                    .iter()
                    .rev()
                    .take_while(|e| !matches!(e, TranscriptEntry::User(_)))
                    .any(|e| matches!(e, TranscriptEntry::Assistant(_)));
                if !answered && !assistant_text.is_empty() {
                    self.transcript
                        .push(TranscriptEntry::Assistant(assistant_text.clone()));
                }
            }
            AgentEvent::Refused(m) | AgentEvent::Failed(m) => {
                self.transcript.push(TranscriptEntry::Error(m.clone()))
            }
            AgentEvent::Cancelled => self
                .transcript
                .push(TranscriptEntry::Error("Cancelled.".to_string())),
        }
    }
}

/// The poll side of a running turn.
pub struct RunHandle {
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    done: bool,
}

impl RunHandle {
    /// Applies everything the run produced since the last call to `session`
    /// (transcript, usage, parameters, history) and returns the events for
    /// the UI. Call it every frame while [`RunHandle::is_running`].
    pub fn drain(&mut self, session: &mut AgentSession) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(Msg::Commit(msgs)) => session.messages.extend(msgs),
                Ok(Msg::Event(ev)) => {
                    session.apply(&ev);
                    if ev.is_terminal() {
                        self.done = true;
                    }
                    out.push(ev);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.done {
                        self.done = true;
                        let ev = AgentEvent::Failed("The agent stopped unexpectedly.".to_string());
                        session.apply(&ev);
                        out.push(ev);
                    }
                    break;
                }
            }
        }
        out
    }

    /// True until the terminal event (finished, refused, failed or cancelled)
    /// has been drained.
    pub fn is_running(&self) -> bool {
        !self.done
    }

    /// Asks the run to stop; it ends after the current HTTP call (or sooner)
    /// with [`AgentEvent::Cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

// ----- the run loop -----

struct Job {
    cfg: AgentConfig,
    auth: Option<Auth>,
    client: ApiClient,
    history: Vec<Value>,
    user_content: Vec<Value>,
    project: Project,
    defaults: PlanDefaults,
    parameters: Vec<Parameter>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn secrets(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(k) = &self.cfg.api_key {
            v.push(k.clone());
        }
        if let Some(a) = &self.auth {
            v.push(a.secret().to_string());
        }
        v
    }
}

const MAX_RESULT_CHARS: usize = 40_000;

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push_str("...");
    t
}

fn run_job(job: Job, tx: &Sender<Msg>) {
    let send = |ev: AgentEvent| {
        let _ = tx.send(Msg::Event(ev));
    };
    let secrets = job.secrets();
    let Job {
        cfg,
        auth,
        mut client,
        history,
        user_content,
        mut project,
        defaults,
        parameters,
        cancel,
    } = job;
    let Some(auth) = auth else {
        send(AgentEvent::Failed(
            "No API key. Add one in Preferences > Plan Agent or set ANTHROPIC_API_KEY.".to_string(),
        ));
        return;
    };
    let before = serde_json::to_string(&project).unwrap_or_default();
    let mut params = parameters;

    let mut work = history;
    let base_len = work.len();
    work.push(json!({"role": "user", "content": user_content}));

    let mut tool_rounds = 0usize;
    let mut continued = false;
    let mut final_call = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            send(AgentEvent::Cancelled);
            return;
        }
        let request = Request::new(&cfg, &work);
        let mut on_delta = |d: Delta| match d {
            Delta::Text(t) => send(AgentEvent::TextDelta(t)),
            Delta::Thinking(t) => send(AgentEvent::ThinkingDelta(t)),
            Delta::ToolStart { .. } => {}
        };
        let msg = match client.send(&cfg, &auth, &request, &cancel, &mut on_delta) {
            Ok(m) => m,
            Err(ApiError::Cancelled) => {
                send(AgentEvent::Cancelled);
                return;
            }
            Err(e) => {
                send(AgentEvent::Failed(api::redact(&e.user_message(), &secrets)));
                return;
            }
        };
        send(AgentEvent::Usage(msg.usage.clone()));
        if !msg.content.is_empty() {
            work.push(json!({"role": "assistant", "content": msg.content.clone()}));
        }
        let stop = msg.stop_reason.clone().unwrap_or_default();
        if stop == "refusal" {
            send(AgentEvent::Refused(api::redact(
                msg.stop_explanation
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("The model declined to help with this request."),
                &secrets,
            )));
            return;
        }
        let tool_uses: Vec<&Value> = msg
            .content
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .collect();

        if !tool_uses.is_empty() && matches!(stop.as_str(), "tool_use" | "max_tokens") {
            if final_call || tool_rounds >= cfg.max_tool_rounds {
                // The cap: answer every tool_use without running it, ask for a summary.
                let mut content: Vec<Value> = tool_uses
                    .iter()
                    .map(|tu| {
                        json!({
                            "type": "tool_result",
                            "tool_use_id": tu["id"],
                            "content": "Not executed: the tool round limit was reached.",
                            "is_error": true,
                        })
                    })
                    .collect();
                if final_call {
                    work.push(json!({"role": "user", "content": content}));
                    break;
                }
                content.push(json!({"type": "text", "text": "Stop and summarize."}));
                work.push(json!({"role": "user", "content": content}));
                final_call = true;
                continue;
            }
            let results = run_tools(
                &tool_uses,
                &msg.input_errors,
                &mut project,
                &defaults,
                &mut params,
                &send,
            );
            work.push(json!({"role": "user", "content": results}));
            tool_rounds += 1;
            continue;
        }
        if stop == "max_tokens" && !continued {
            continued = true;
            work.push(json!({"role": "user", "content": [{"type": "text", "text": "Continue."}]}));
            continue;
        }
        break;
    }

    let after = serde_json::to_string(&project).unwrap_or_default();
    let assistant_text = final_assistant_text(&work[base_len..]);
    let _ = tx.send(Msg::Commit(work[base_len..].to_vec()));
    send(AgentEvent::Finished {
        project,
        changed: before != after,
        assistant_text,
    });
}

/// The text of the last assistant message of the run; every assistant text if that one has none.
fn final_assistant_text(msgs: &[Value]) -> String {
    let texts = |m: &Value| -> String {
        m["content"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    };
    let assistant: Vec<&Value> = msgs.iter().filter(|m| m["role"] == "assistant").collect();
    match assistant.last().map(|m| texts(m)) {
        Some(t) if !t.trim().is_empty() => t,
        _ => assistant
            .iter()
            .map(|m| texts(m))
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Executes every `tool_use` block in order and returns the `tool_result` blocks.
fn run_tools(
    tool_uses: &[&Value],
    input_errors: &BTreeMap<String, String>,
    project: &mut Project,
    defaults: &PlanDefaults,
    params: &mut Vec<Parameter>,
    send: &dyn Fn(AgentEvent),
) -> Vec<Value> {
    let mut results = Vec::new();
    for tu in tool_uses {
        let id = tu["id"].as_str().unwrap_or_default().to_string();
        let name = tu["name"].as_str().unwrap_or_default().to_string();
        let input = tu["input"].clone();
        send(AgentEvent::ToolStarted {
            name: name.clone(),
            input_summary: tools::input_summary(&name, &input),
        });
        let params_before = params.clone();
        let res: Result<Value, String> = match input_errors.get(&id) {
            Some(e) => Err(e.clone()),
            None => catch_unwind(AssertUnwindSafe(|| {
                tools::execute(&name, &input, project, defaults, params)
            }))
            .unwrap_or_else(|_| Err(format!("internal error while running {name}"))),
        };
        let (ok, text) = match &res {
            Ok(v) => (true, v.to_string()),
            Err(e) => (false, e.clone()),
        };
        send(AgentEvent::ToolFinished {
            name: name.clone(),
            ok,
            result_summary: clip(&text, 200),
        });
        if *params != params_before {
            send(AgentEvent::ParametersChanged(params.clone()));
        }
        results.push(json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": clip(&text, MAX_RESULT_CHARS),
            "is_error": !ok,
        }));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_debug_never_shows_the_key() {
        let cfg = AgentConfig {
            api_key: Some("sk-ant-secret-value".to_string()),
            ..AgentConfig::default()
        };
        let text = format!("{cfg:?}");
        assert!(!text.contains("secret-value"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
    }

    fn param(name: &str, value: f64) -> Parameter {
        Parameter {
            name: name.into(),
            label: "L".into(),
            value,
            min: 10.0,
            max: 30.0,
            unit: "ft".into(),
            description: String::new(),
        }
    }

    #[test]
    fn config_defaults_follow_the_contract() {
        let c = AgentConfig::default();
        assert_eq!(c.model, "claude-opus-5-5");
        assert_eq!(c.effort, Effort::Medium);
        assert_eq!(c.max_tool_rounds, 24);
        assert_eq!(c.base_url, "https://api.anthropic.com");
        assert_eq!(Effort::XHigh.as_str(), "xhigh");
    }

    #[test]
    fn cost_estimate_uses_opus_rates() {
        let u = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_read_tokens: 1_000_000,
            cache_write_tokens: 1_000_000,
            turns: 1,
        };
        let c = u.estimated_cost_usd("claude-opus-5-5");
        assert!((c - (4.0 + 20.0 + 0.2 + 5.0)).abs() < 1e-9, "{c}");
    }

    #[test]
    fn slider_changes_clamp_and_queue() {
        let mut s = AgentSession::new();
        s.parameters.push(param("garage_width_ft", 24.0));
        assert!(!s.set_parameter_value("nope", 1.0));
        assert!(s.set_parameter_value("garage_width_ft", 99.0));
        assert_eq!(s.parameters()[0].value, 30.0);
        assert!(s.set_parameter_value("garage_width_ft", 20.0));
        assert_eq!(
            s.pending_changes,
            vec![("garage_width_ft".to_string(), 20.0)]
        );
    }

    #[test]
    fn events_build_the_transcript() {
        let mut s = AgentSession::new();
        s.apply(&AgentEvent::TextDelta("Hel".into()));
        s.apply(&AgentEvent::TextDelta("lo".into()));
        s.apply(&AgentEvent::ToolStarted {
            name: "measure".into(),
            input_summary: "a to b".into(),
        });
        s.apply(&AgentEvent::ToolFinished {
            name: "measure".into(),
            ok: false,
            result_summary: "bad".into(),
        });
        assert!(matches!(&s.transcript()[0], TranscriptEntry::Assistant(t) if t == "Hello"));
        assert!(
            matches!(&s.transcript()[1], TranscriptEntry::ToolCall { summary, ok: false, .. } if summary == "a to b: bad")
        );
    }
}
