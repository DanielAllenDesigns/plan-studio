//! Run-loop, request-shape, retry and cancel tests against a scripted
//! transport. No network: the transport replays canned SSE text.

use plan_agent::api::{ApiClient, Request, Transport, TransportError};
use plan_agent::{AgentConfig, AgentEvent, AgentSession, RunHandle, TranscriptEntry};
use plan_core::defaults::PlanDefaults;
use plan_core::model::Project;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const KEY: &str = "sk-ant-test-secret-12345";

// ----- scripted transport -----

enum Reply {
    /// A 200 response with this SSE text.
    Sse(String),
    /// A non-2xx response with this body.
    Status(u16, String),
    /// A 200 response that never produces a line until the run is cancelled.
    Block,
}

#[derive(Clone, Default)]
struct Log {
    /// The parsed JSON body of every request.
    bodies: Arc<Mutex<Vec<Value>>>,
    /// The headers of every request.
    headers: Arc<Mutex<Vec<Vec<(String, String)>>>>,
    /// The backoff sleeps.
    sleeps: Arc<Mutex<Vec<Duration>>>,
}

struct Scripted {
    replies: VecDeque<Reply>,
    log: Log,
}

impl Transport for Scripted {
    fn post_stream(
        &mut self,
        _url: &str,
        headers: &[(String, String)],
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<u16, TransportError> {
        self.log
            .bodies
            .lock()
            .unwrap()
            .push(serde_json::from_str(body).expect("the body is JSON"));
        self.log.headers.lock().unwrap().push(headers.to_vec());
        match self.replies.pop_front().expect("no scripted reply left") {
            Reply::Sse(text) => {
                for line in text.lines() {
                    if !on_line(line) {
                        break;
                    }
                }
                Ok(200)
            }
            Reply::Status(code, body) => {
                for line in body.lines() {
                    on_line(line);
                }
                Ok(code)
            }
            Reply::Block => loop {
                if !on_line("") {
                    return Ok(200);
                }
                std::thread::sleep(Duration::from_millis(5));
            },
        }
    }
}

fn client(replies: Vec<Reply>) -> (ApiClient, Log) {
    let log = Log::default();
    let sleeps = log.sleeps.clone();
    let c = ApiClient::new(Box::new(Scripted {
        replies: replies.into(),
        log: log.clone(),
    }))
    .with_sleeper(Box::new(move |d, _| sleeps.lock().unwrap().push(d)));
    (c, log)
}

// ----- SSE builders -----

fn sse(events: Vec<Value>) -> String {
    events
        .into_iter()
        .map(|e| format!("event: {}\ndata: {}\n\n", e["type"].as_str().unwrap(), e))
        .collect()
}

fn start(input: u64) -> Value {
    json!({"type":"message_start","message":{"id":"msg","usage":{"input_tokens":input,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}})
}

fn end(stop: &str, out: u64) -> Vec<Value> {
    vec![
        json!({"type":"message_delta","delta":{"stop_reason":stop},"usage":{"output_tokens":out}}),
        json!({"type":"message_stop"}),
    ]
}

fn tool_block(idx: u64, id: &str, name: &str, input_parts: &[&str]) -> Vec<Value> {
    let mut v = vec![json!({"type":"content_block_start","index":idx,
        "content_block":{"type":"tool_use","id":id,"name":name,"input":{}}})];
    for p in input_parts {
        v.push(json!({"type":"content_block_delta","index":idx,
            "delta":{"type":"input_json_delta","partial_json":p}}));
    }
    v.push(json!({"type":"content_block_stop","index":idx}));
    v
}

fn text_block(idx: u64, text: &str) -> Vec<Value> {
    vec![
        json!({"type":"content_block_start","index":idx,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":idx,"delta":{"type":"text_delta","text":text}}),
        json!({"type":"content_block_stop","index":idx}),
    ]
}

/// thinking + add_wall_rectangle + define_parameter, stop_reason tool_use.
fn first_response() -> String {
    let mut ev = vec![
        start(500),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"A 30 by 40 box."}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG-ONE=="}}),
        json!({"type":"content_block_stop","index":0}),
    ];
    ev.extend(tool_block(
        1,
        "toolu_rect",
        "add_wall_rectangle",
        &[
            "{\"floor\":1,\"x\":0,",
            "\"y\":0,\"width\":30,",
            "\"depth\":40,\"kind\":\"exterior\"}",
        ],
    ));
    ev.extend(tool_block(
        2,
        "toolu_param",
        "define_parameter",
        &["{\"name\":\"house_width_ft\",\"label\":\"House width\",\"value\":30,\"min\":20,\"max\":50,\"unit\":\"ft\",\"description\":\"Width of the house.\"}"],
    ));
    ev.extend(end("tool_use", 200));
    sse(ev)
}

fn final_response(text: &str) -> String {
    let mut ev = vec![start(800)];
    ev.extend(text_block(0, text));
    ev.extend(end("end_turn", 60));
    sse(ev)
}

fn config() -> AgentConfig {
    AgentConfig {
        api_key: Some(KEY.to_string()),
        ..AgentConfig::default()
    }
}

/// Polls the handle until the terminal event; returns every event.
fn finish(h: &mut RunHandle, s: &mut AgentSession) -> Vec<AgentEvent> {
    let t0 = Instant::now();
    let mut all = Vec::new();
    while h.is_running() {
        all.extend(h.drain(s));
        assert!(t0.elapsed() < Duration::from_secs(20), "the run hangs");
        std::thread::sleep(Duration::from_millis(2));
    }
    all.extend(h.drain(s));
    all
}

fn new_project(d: &PlanDefaults) -> Project {
    Project::from_defaults("Test", d)
}

// ----- request shape -----

#[test]
fn the_request_body_has_the_documented_keys_and_nothing_forbidden() {
    let cfg = config();
    let msgs = vec![json!({"role":"user","content":[{"type":"text","text":"hi"}]})];
    let req = Request::new(&cfg, &msgs);
    let b = req.body.as_object().unwrap();
    let mut keys: Vec<&str> = b.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "fallbacks",
            "max_tokens",
            "messages",
            "model",
            "output_config",
            "stream",
            "system",
            "thinking",
            "tools"
        ]
    );
    assert_eq!(b["model"], "claude-opus-5-5");
    assert_eq!(b["max_tokens"], 32000);
    assert_eq!(b["stream"], true);
    assert_eq!(
        b["thinking"],
        json!({"type":"adaptive","display":"summarized"})
    );
    assert_eq!(b["output_config"], json!({"effort":"medium"}));
    assert_eq!(b["fallbacks"], "default");
    assert_eq!(b["system"][0]["type"], "text");
    assert_eq!(b["system"][0]["cache_control"], json!({"type":"ephemeral"}));
    let text = req.to_json_string();
    for forbidden in [
        "temperature",
        "budget_tokens",
        "tool_choice",
        "\"disabled\"",
    ] {
        assert!(!text.contains(forbidden), "{forbidden} must not be sent");
    }
    let tools = b["tools"].as_array().unwrap();
    assert!(!tools.is_empty());
    for t in tools {
        assert_eq!(t["strict"], true);
        assert_eq!(t["eager_input_streaming"], true);
        assert_eq!(t["input_schema"]["additionalProperties"], false);
    }
}

#[test]
fn system_and_tools_are_byte_identical_between_requests() {
    let cfg = config();
    let a = Request::new(&cfg, &[json!({"role":"user","content":"one"})]);
    let b = Request::new(
        &cfg,
        &[
            json!({"role":"user","content":"one"}),
            json!({"role":"assistant","content":[{"type":"text","text":"two"}]}),
            json!({"role":"user","content":"three"}),
        ],
    );
    assert_eq!(a.body["system"].to_string(), b.body["system"].to_string());
    assert_eq!(a.body["tools"].to_string(), b.body["tools"].to_string());
}

// ----- the loop -----

#[test]
fn a_two_round_run_edits_the_plan_and_echoes_the_assistant_turn_verbatim() {
    let (c, log) = client(vec![
        Reply::Sse(first_response()),
        Reply::Sse(final_response("Built a 30 by 40 foot house.")),
    ]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(
        &config(),
        new_project(&defaults),
        &defaults,
        "build a 30x40 house",
        c,
    );
    let events = finish(&mut h, &mut session);

    // Events.
    let started: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolStarted { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(started, vec!["add_wall_rectangle", "define_parameter"]);
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ParametersChanged(p) if p.len() == 1)));
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::ThinkingDelta(t) if t == "A 30 by 40 box.")));
    let Some(AgentEvent::Finished {
        project,
        changed,
        assistant_text,
    }) = events.last()
    else {
        panic!("the last event is Finished: {events:?}");
    };
    assert!(*changed);
    assert_eq!(project.floors[0].walls.len(), 4);
    assert_eq!(assistant_text, "Built a 30 by 40 foot house.");
    assert!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolFinished { ok: true, .. }))
            .count()
            == 2
    );

    // The second request echoes the assistant turn and answers both tools in order.
    let bodies = log.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 2);
    let msgs = bodies[1]["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0]["role"], "user");
    assert_eq!(msgs[1]["role"], "assistant");
    assert_eq!(
        msgs[1]["content"][0],
        json!({"type":"thinking","thinking":"A 30 by 40 box.","signature":"SIG-ONE=="})
    );
    assert_eq!(msgs[1]["content"][1]["type"], "tool_use");
    assert_eq!(msgs[1]["content"][1]["id"], "toolu_rect");
    assert_eq!(
        msgs[1]["content"][1]["input"],
        json!({"floor":1,"x":0,"y":0,"width":30,"depth":40,"kind":"exterior"})
    );
    assert_eq!(msgs[1]["content"][2]["id"], "toolu_param");
    assert_eq!(msgs[2]["role"], "user");
    let results = msgs[2]["content"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .all(|r| r["type"] == "tool_result" && r["is_error"] == false));
    assert_eq!(results[0]["tool_use_id"], "toolu_rect");
    assert_eq!(results[1]["tool_use_id"], "toolu_param");
    // The first user message: plan summary block, then the request.
    let first = msgs[0]["content"].as_array().unwrap();
    assert!(first[0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Current plan state:"));
    assert_eq!(first.last().unwrap()["text"], "build a 30x40 house");
    // Both requests send the same system prompt and tools.
    assert_eq!(bodies[0]["system"], bodies[1]["system"]);
    assert_eq!(bodies[0]["tools"], bodies[1]["tools"]);
    // The key travels in the header only.
    let headers = log.headers.lock().unwrap();
    assert!(headers[0].contains(&("x-api-key".to_string(), KEY.to_string())));
    assert!(!bodies[0].to_string().contains(KEY));
    drop((bodies, headers));

    // The session.
    assert_eq!(session.parameters().len(), 1);
    assert_eq!(session.parameters()[0].name, "house_width_ft");
    assert_eq!(session.messages().len(), 4);
    assert_eq!(session.usage().turns, 2);
    assert_eq!(session.usage().input_tokens, 1300);
    assert_eq!(session.usage().output_tokens, 260);
    let kinds: Vec<&str> = session
        .transcript()
        .iter()
        .map(|t| match t {
            TranscriptEntry::User(_) => "user",
            TranscriptEntry::Assistant(_) => "assistant",
            TranscriptEntry::Thinking(_) => "thinking",
            TranscriptEntry::ToolCall { ok: true, .. } => "tool",
            TranscriptEntry::ToolCall { ok: false, .. } => "tool-failed",
            TranscriptEntry::Error(_) => "error",
        })
        .collect();
    assert_eq!(kinds, vec!["user", "thinking", "tool", "tool", "assistant"]);
    assert!(
        matches!(&session.transcript()[4], TranscriptEntry::Assistant(t) if t == "Built a 30 by 40 foot house.")
    );
}

#[test]
fn slider_changes_reach_the_next_run_and_history_carries_over() {
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let (c, _) = client(vec![
        Reply::Sse(first_response()),
        Reply::Sse(final_response("Done.")),
    ]);
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "build", c);
    let events = finish(&mut h, &mut session);
    let Some(AgentEvent::Finished { project, .. }) = events.last() else {
        panic!("finished")
    };
    let project = project.clone();

    assert!(session.set_parameter_value("house_width_ft", 36.0));
    let (c, log) = client(vec![Reply::Sse(final_response("Widened."))]);
    let mut h = session.run_with_client(&config(), project, &defaults, "apply the new width", c);
    let events = finish(&mut h, &mut session);
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { changed: false, .. })
    ));
    let bodies = log.bodies.lock().unwrap();
    let msgs = bodies[0]["messages"].as_array().unwrap();
    // 4 committed messages, then the new user turn.
    assert_eq!(msgs.len(), 5);
    assert_eq!(msgs[1]["content"][0]["signature"], "SIG-ONE==");
    let blocks = msgs[4]["content"].as_array().unwrap();
    assert_eq!(blocks.len(), 3);
    assert!(blocks[0]["text"]
        .as_str()
        .unwrap()
        .contains("house_width_ft = 36"));
    assert_eq!(
        blocks[1]["text"],
        "Parameter changes from the user: house_width_ft=36"
    );
    assert_eq!(blocks[2]["text"], "apply the new width");
}

#[test]
fn the_tool_round_cap_answers_the_tools_without_running_them_and_asks_for_a_summary() {
    let measure = |id: &str| {
        let mut ev = vec![start(10)];
        ev.extend(tool_block(
            0,
            id,
            "measure",
            &["{\"from\":[0,0],\"to\":[3,4]}"],
        ));
        ev.extend(end("tool_use", 5));
        sse(ev)
    };
    let (c, log) = client(vec![
        Reply::Sse(measure("t1")),
        Reply::Sse(measure("t2")),
        Reply::Sse(final_response("Stopped early.")),
    ]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let cfg = AgentConfig {
        max_tool_rounds: 1,
        ..config()
    };
    let mut h = session.run_with_client(&cfg, new_project(&defaults), &defaults, "measure", c);
    let events = finish(&mut h, &mut session);
    assert!(
        matches!(events.last(), Some(AgentEvent::Finished { assistant_text, .. }) if assistant_text == "Stopped early.")
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolFinished { .. }))
            .count(),
        1,
        "only the first round ran tools"
    );
    let bodies = log.bodies.lock().unwrap();
    let last_user = bodies[2]["messages"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let content = last_user["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "tool_result");
    assert_eq!(content[0]["is_error"], true);
    assert_eq!(content[0]["tool_use_id"], "t2");
    assert_eq!(
        content[1],
        json!({"type":"text","text":"Stop and summarize."})
    );
}

#[test]
fn truncated_tool_input_comes_back_as_an_error_result() {
    let mut ev = vec![start(10)];
    ev.extend(tool_block(
        0,
        "t_cut",
        "add_wall",
        &["{\"floor\":1,\"start\":[0,"],
    ));
    ev.extend(end("max_tokens", 32000));
    let (c, log) = client(vec![
        Reply::Sse(sse(ev)),
        Reply::Sse(final_response("Retrying smaller.")),
    ]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Finished { changed: false, .. })
    ));
    let bodies = log.bodies.lock().unwrap();
    let r = &bodies[1]["messages"][2]["content"][0];
    assert_eq!(r["is_error"], true);
    assert!(r["content"].as_str().unwrap().contains("not valid JSON"));
    // The echoed tool_use carries an empty (valid) input.
    assert_eq!(bodies[1]["messages"][1]["content"][0]["input"], json!({}));
}

#[test]
fn a_refusal_is_reported_and_leaves_the_history_alone() {
    let mut ev = vec![start(10)];
    ev.extend(text_block(0, "I can't help with that."));
    ev.push(json!({"type":"message_delta","delta":{"stop_reason":"refusal","stop_details":{"explanation":"Declined by policy."}},"usage":{"output_tokens":5}}));
    ev.push(json!({"type":"message_stop"}));
    let (c, _) = client(vec![Reply::Sse(sse(ev))]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(matches!(events.last(), Some(AgentEvent::Refused(m)) if m == "Declined by policy."));
    assert!(session.messages().is_empty());
    assert!(matches!(
        session.transcript().last(),
        Some(TranscriptEntry::Error(_))
    ));
}

// ----- cancel and errors -----

#[test]
fn cancel_stops_a_blocked_request() {
    let (c, _) = client(vec![Reply::Block]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    std::thread::sleep(Duration::from_millis(50));
    assert!(h.is_running());
    h.cancel();
    let events = finish(&mut h, &mut session);
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled)));
    assert!(session.messages().is_empty());
    assert!(!h.is_running());
}

#[test]
fn a_429_is_retried_with_backoff_and_then_succeeds() {
    let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
    let (c, log) = client(vec![
        Reply::Status(429, body.to_string()),
        Reply::Status(529, body.to_string()),
        Reply::Sse(final_response("Fine.")),
    ]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(
        matches!(events.last(), Some(AgentEvent::Finished { .. })),
        "{events:?}"
    );
    assert_eq!(log.bodies.lock().unwrap().len(), 3);
    assert_eq!(
        *log.sleeps.lock().unwrap(),
        vec![Duration::from_secs(2), Duration::from_secs(4)]
    );
}

#[test]
fn server_errors_give_up_after_three_retries() {
    let body = r#"{"type":"error","error":{"type":"api_error","message":"boom"}}"#;
    let (c, log) = client(
        (0..4)
            .map(|_| Reply::Status(500, body.to_string()))
            .collect(),
    );
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(
        matches!(events.last(), Some(AgentEvent::Failed(m)) if m.contains("500") && m.contains("boom"))
    );
    assert_eq!(
        *log.sleeps.lock().unwrap(),
        vec![
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8)
        ]
    );
}

#[test]
fn a_401_fails_without_the_key_anywhere_in_the_message() {
    // Even a server that echoes the key back must not leak it.
    let body = format!(
        r#"{{"type":"error","error":{{"type":"authentication_error","message":"invalid x-api-key {KEY}"}}}}"#
    );
    let (c, log) = client(vec![Reply::Status(401, body)]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    let Some(AgentEvent::Failed(m)) = events.last() else {
        panic!("failed: {events:?}")
    };
    assert_eq!(m, "API key rejected");
    assert!(!m.contains(KEY));
    assert!(log.sleeps.lock().unwrap().is_empty(), "401 is not retried");
    for e in session.transcript() {
        if let TranscriptEntry::Error(t) = e {
            assert!(!t.contains(KEY));
        }
    }
}

#[test]
fn a_400_shows_the_server_message_and_redacts_the_key() {
    let body = format!(
        r#"{{"type":"error","error":{{"type":"invalid_request_error","message":"bad field near {KEY}"}}}}"#
    );
    let (c, _) = client(vec![Reply::Status(400, body)]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    let Some(AgentEvent::Failed(m)) = events.last() else {
        panic!("failed")
    };
    assert!(m.contains("bad field near"));
    assert!(!m.contains(KEY));
}

#[test]
fn a_dropped_connection_is_retried_once() {
    struct Flaky {
        calls: usize,
        ok: String,
    }
    impl Transport for Flaky {
        fn post_stream(
            &mut self,
            _: &str,
            _: &[(String, String)],
            _: &str,
            on_line: &mut dyn FnMut(&str) -> bool,
        ) -> Result<u16, TransportError> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(TransportError::Connection("connection reset".into()));
            }
            for l in self.ok.lines() {
                on_line(l);
            }
            Ok(200)
        }
    }
    let c = ApiClient::new(Box::new(Flaky {
        calls: 0,
        ok: final_response("Back."),
    }))
    .with_sleeper(Box::new(|_, _| {}));
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let mut h = session.run_with_client(&config(), new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(matches!(events.last(), Some(AgentEvent::Finished { .. })));
}

#[test]
fn missing_credentials_fail_cleanly() {
    // Not an environment-dependent assertion: an explicit empty key plus no
    // env falls through, so only check when nothing is set.
    if std::env::var("ANTHROPIC_API_KEY").is_ok() || std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
        return;
    }
    let (c, _) = client(vec![]);
    let defaults = PlanDefaults::default();
    let mut session = AgentSession::new();
    let cfg = AgentConfig::default();
    assert!(!cfg.credentials_available());
    let mut h = session.run_with_client(&cfg, new_project(&defaults), &defaults, "x", c);
    let events = finish(&mut h, &mut session);
    assert!(matches!(events.last(), Some(AgentEvent::Failed(m)) if m.contains("No API key")));
}

// ----- the real transport, against a local socket -----

#[test]
fn the_ureq_transport_streams_lines_and_reads_status_and_retry_after() {
    use plan_agent::api::UreqTransport;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        // Read the request head and body.
        let mut buf = Vec::new();
        let mut chunk = [0u8; 2048];
        let (head_len, content_len) = loop {
            let n = sock.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                break (i + 4, len);
            }
        };
        while buf.len() < head_len + content_len {
            let n = sock.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
        }
        let request = String::from_utf8_lossy(&buf).to_string();
        let body = "event: ping\ndata: {\"type\":\"ping\"}\n\nevent: error\ndata: {\"type\":\"error\"}\n\n";
        write!(
            sock,
            "HTTP/1.1 429 Too Many Requests\r\nretry-after: 7\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{body}"
        )
        .unwrap();
        request
    });

    let mut t = UreqTransport::new();
    let mut lines = Vec::new();
    let status = t
        .post_stream(
            &format!("http://127.0.0.1:{port}/v1/messages"),
            &[
                ("content-type".to_string(), "application/json".to_string()),
                ("x-api-key".to_string(), "k-123456".to_string()),
            ],
            "{\"hello\":1}",
            &mut |l| {
                lines.push(l.to_string());
                true
            },
        )
        .expect("the request goes through");
    assert_eq!(status, 429);
    assert_eq!(t.retry_after_secs(), Some(7));
    assert_eq!(lines[0], "event: ping");
    assert!(lines.contains(&"data: {\"type\":\"ping\"}".to_string()));
    let request = server.join().unwrap().to_lowercase();
    assert!(request.starts_with("post /v1/messages http/1.1"));
    assert!(request.contains("x-api-key: k-123456"));
    assert!(request.ends_with("{\"hello\":1}"));
}
