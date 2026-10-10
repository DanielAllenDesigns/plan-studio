//! The Anthropic Messages API: request body, streaming (SSE) assembly, error
//! mapping and retries. The HTTP transport sits behind the [`Transport`]
//! trait so tests never touch the network.

use crate::session::{AgentConfig, Usage};
use crate::{prompt, tools};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const API_VERSION: &str = "2023-06-01";
/// Beta for the `fallbacks: "default"` request field.
pub const BETA_FALLBACK: &str = "server-side-fallback-2026-07-01";
/// Beta that lets a Bearer OAuth token authenticate.
pub const BETA_OAUTH: &str = "oauth-2025-04-20";
pub const MAX_TOKENS: u64 = 32_000;

// ----- auth -----

/// How a request authenticates.
#[derive(Clone, PartialEq, Eq)]
pub enum Auth {
    /// `x-api-key: <key>`
    ApiKey(String),
    /// `authorization: Bearer <token>` plus the OAuth beta.
    Bearer(String),
}

impl std::fmt::Debug for Auth {
    // Never print the secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::ApiKey(_) => f.write_str("Auth::ApiKey(<redacted>)"),
            Auth::Bearer(_) => f.write_str("Auth::Bearer(<redacted>)"),
        }
    }
}

impl Auth {
    pub fn secret(&self) -> &str {
        match self {
            Auth::ApiKey(s) | Auth::Bearer(s) => s,
        }
    }
}

/// The config key, else `ANTHROPIC_API_KEY`, else `ANTHROPIC_AUTH_TOKEN`
/// (looked up with `env`). Empty values count as unset.
pub fn resolve_auth(cfg_key: Option<&str>, env: &dyn Fn(&str) -> Option<String>) -> Option<Auth> {
    let nonempty = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(k) = nonempty(cfg_key.map(str::to_string)) {
        return Some(Auth::ApiKey(k));
    }
    if let Some(k) = nonempty(env("ANTHROPIC_API_KEY")) {
        return Some(Auth::ApiKey(k));
    }
    nonempty(env("ANTHROPIC_AUTH_TOKEN")).map(Auth::Bearer)
}

/// [`resolve_auth`] against the process environment.
pub fn resolve_auth_from_env(cfg_key: Option<&str>) -> Option<Auth> {
    resolve_auth(cfg_key, &|k| std::env::var(k).ok())
}

/// The request headers for `auth`.
pub fn build_headers(auth: &Auth) -> Vec<(String, String)> {
    let mut betas = vec![BETA_FALLBACK];
    let mut h = vec![("content-type".to_string(), "application/json".to_string())];
    match auth {
        Auth::ApiKey(k) => h.push(("x-api-key".to_string(), k.clone())),
        Auth::Bearer(t) => {
            h.push(("authorization".to_string(), format!("Bearer {t}")));
            betas.push(BETA_OAUTH);
        }
    }
    h.push(("anthropic-version".to_string(), API_VERSION.to_string()));
    h.push(("anthropic-beta".to_string(), betas.join(",")));
    h
}

/// `msg` with every secret replaced by `[redacted]`.
pub fn redact(msg: &str, secrets: &[String]) -> String {
    let mut out = msg.to_string();
    for s in secrets.iter().filter(|s| s.len() >= 6) {
        out = out.replace(s.as_str(), "[redacted]");
    }
    out
}

// ----- the request -----

/// One `POST /v1/messages` body.
#[derive(Clone, Debug)]
pub struct Request {
    pub body: Value,
}

impl Request {
    /// The streaming request for `messages`. `system` and `tools` come from
    /// [`prompt::SYSTEM_PROMPT`] and [`tools::catalog_json`] and are
    /// byte-identical between calls (prompt caching is prefix based).
    pub fn new(cfg: &AgentConfig, messages: &[Value]) -> Request {
        let body = json!({
            "model": cfg.model,
            "max_tokens": MAX_TOKENS,
            "stream": true,
            "thinking": {"type": "adaptive", "display": "summarized"},
            "output_config": {"effort": cfg.effort.as_str()},
            "fallbacks": "default",
            "system": [{
                "type": "text",
                "text": prompt::SYSTEM_PROMPT,
                "cache_control": {"type": "ephemeral"},
            }],
            "tools": tools::catalog_json(),
            "messages": messages,
        });
        Request { body }
    }

    pub fn to_json_string(&self) -> String {
        self.body.to_string()
    }
}

// ----- transport -----

#[derive(Clone, Debug)]
pub enum TransportError {
    /// Could not connect, DNS, TLS, timeout, dropped connection.
    Connection(String),
    Other(String),
}

/// Posts a streaming request and feeds the response body line by line.
pub trait Transport: Send {
    /// POSTs `body`; calls `on_line` with each raw response line (SSE lines
    /// for a 2xx response, the JSON error body otherwise) until it returns
    /// `false` (cancel) or the body ends. Returns the HTTP status.
    fn post_stream(
        &mut self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<u16, TransportError>;

    /// The `retry-after` seconds of the last response, if it had one.
    fn retry_after_secs(&self) -> Option<u64> {
        None
    }
}

/// The real transport (ureq 3, rustls).
pub struct UreqTransport {
    agent: ureq::Agent,
    retry_after: Option<u64>,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(20)))
            .timeout_recv_response(Some(Duration::from_secs(180)))
            .timeout_recv_body(Some(Duration::from_secs(15 * 60)))
            .build();
        Self {
            agent: config.into(),
            retry_after: None,
        }
    }
}

impl Transport for UreqTransport {
    fn post_stream(
        &mut self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        on_line: &mut dyn FnMut(&str) -> bool,
    ) -> Result<u16, TransportError> {
        self.retry_after = None;
        let mut req = self.agent.post(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req.send(body).map_err(|e| match e {
            ureq::Error::Http(_) | ureq::Error::BadUri(_) => TransportError::Other(e.to_string()),
            e => TransportError::Connection(e.to_string()),
        })?;
        let status = resp.status().as_u16();
        self.retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut reader = std::io::BufReader::new(resp.into_body().into_reader());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = reader
                .read_until(b'\n', &mut buf)
                .map_err(|e| TransportError::Connection(e.to_string()))?;
            if n == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&buf);
            if !on_line(line.trim_end_matches(['\r', '\n'])) {
                break;
            }
        }
        Ok(status)
    }

    fn retry_after_secs(&self) -> Option<u64> {
        self.retry_after
    }
}

// ----- errors -----

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    Cancelled,
    /// 401 (and 403).
    Auth,
    /// 400: the server's message.
    BadRequest(String),
    /// Any other HTTP failure, after the retries.
    Http {
        status: u16,
        message: String,
    },
    Connection(String),
    /// An `error` event or a truncated stream.
    Stream(String),
}

impl ApiError {
    /// The message to show the user. Never contains request headers.
    pub fn user_message(&self) -> String {
        match self {
            ApiError::Cancelled => "Cancelled.".to_string(),
            ApiError::Auth => "API key rejected".to_string(),
            ApiError::BadRequest(m) => format!("The API rejected the request: {m}"),
            ApiError::Http { status, message } => {
                format!("The API returned HTTP {status}: {message}")
            }
            ApiError::Connection(m) => format!("Could not reach the API: {m}"),
            ApiError::Stream(m) => format!("The response stream failed: {m}"),
        }
    }
}

// ----- SSE parsing and assembly -----

/// One line of an SSE stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SseLine {
    Event(String),
    Data(String),
    Blank,
    /// A comment (`: ping`) or an unknown field.
    Ignored,
}

pub fn parse_sse_line(line: &str) -> SseLine {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        SseLine::Blank
    } else if let Some(r) = line.strip_prefix("event:") {
        SseLine::Event(r.strip_prefix(' ').unwrap_or(r).to_string())
    } else if let Some(r) = line.strip_prefix("data:") {
        SseLine::Data(r.strip_prefix(' ').unwrap_or(r).to_string())
    } else {
        SseLine::Ignored
    }
}

/// Something worth showing while the response streams in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    ToolStart { name: String },
}

/// A fully assembled response message.
#[derive(Clone, Debug, Default)]
pub struct AssembledMessage {
    /// The content blocks, ready to echo back as the assistant turn.
    pub content: Vec<Value>,
    pub stop_reason: Option<String>,
    /// `stop_details.explanation` of a refusal.
    pub stop_explanation: Option<String>,
    pub usage: Usage,
    /// `tool_use` id to why its input could not be used (invalid or truncated JSON).
    pub input_errors: BTreeMap<String, String>,
}

struct Block {
    value: Value,
    json_buf: String,
    stopped: bool,
}

/// Rebuilds a response from its SSE lines.
#[derive(Default)]
pub struct StreamAssembler {
    blocks: BTreeMap<usize, Block>,
    stop_reason: Option<String>,
    stop_explanation: Option<String>,
    usage: Usage,
    stream_error: Option<String>,
    finished: bool,
    started: bool,
}

fn u64_of(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

impl StreamAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one raw line; returns what to show right away.
    pub fn push_line(&mut self, line: &str) -> Vec<Delta> {
        match parse_sse_line(line) {
            SseLine::Data(d) => match serde_json::from_str::<Value>(&d) {
                Ok(v) => self.handle(&v),
                Err(_) => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn handle(&mut self, v: &Value) -> Vec<Delta> {
        let mut out = Vec::new();
        match v["type"].as_str().unwrap_or_default() {
            "message_start" => {
                self.started = true;
                let u = &v["message"]["usage"];
                self.usage.input_tokens = u64_of(u, "input_tokens").unwrap_or(0);
                self.usage.output_tokens = u64_of(u, "output_tokens").unwrap_or(0);
                self.usage.cache_read_tokens = u64_of(u, "cache_read_input_tokens").unwrap_or(0);
                self.usage.cache_write_tokens =
                    u64_of(u, "cache_creation_input_tokens").unwrap_or(0);
                self.usage.turns = 1;
            }
            "content_block_start" => {
                let Some(idx) = v["index"].as_u64() else {
                    return out;
                };
                let mut block = v["content_block"].clone();
                match block["type"].as_str().unwrap_or_default() {
                    "text" => {
                        if let Some(t) = block["text"].as_str().filter(|t| !t.is_empty()) {
                            out.push(Delta::Text(t.to_string()));
                        }
                    }
                    "thinking" => {
                        if let Some(t) = block["thinking"].as_str().filter(|t| !t.is_empty()) {
                            out.push(Delta::Thinking(t.to_string()));
                        }
                    }
                    "tool_use" => {
                        block["input"] = json!({});
                        out.push(Delta::ToolStart {
                            name: block["name"].as_str().unwrap_or_default().to_string(),
                        });
                    }
                    _ => {}
                }
                self.blocks.insert(
                    idx as usize,
                    Block {
                        value: block,
                        json_buf: String::new(),
                        stopped: false,
                    },
                );
            }
            "content_block_delta" => {
                let Some(b) = v["index"]
                    .as_u64()
                    .and_then(|i| self.blocks.get_mut(&(i as usize)))
                else {
                    return out;
                };
                let d = &v["delta"];
                match d["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        if let Some(t) = d["text"].as_str() {
                            append_str(&mut b.value, "text", t);
                            out.push(Delta::Text(t.to_string()));
                        }
                    }
                    "thinking_delta" => {
                        if let Some(t) = d["thinking"].as_str() {
                            append_str(&mut b.value, "thinking", t);
                            out.push(Delta::Thinking(t.to_string()));
                        }
                    }
                    "signature_delta" => {
                        if let Some(s) = d["signature"].as_str() {
                            append_str(&mut b.value, "signature", s);
                        }
                    }
                    "input_json_delta" => {
                        if let Some(p) = d["partial_json"].as_str() {
                            b.json_buf.push_str(p);
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                if let Some(b) = v["index"]
                    .as_u64()
                    .and_then(|i| self.blocks.get_mut(&(i as usize)))
                {
                    b.stopped = true;
                }
            }
            "message_delta" => {
                let d = &v["delta"];
                if let Some(r) = d["stop_reason"].as_str() {
                    self.stop_reason = Some(r.to_string());
                }
                let details = if d["stop_details"].is_object() {
                    &d["stop_details"]
                } else {
                    &v["stop_details"]
                };
                if let Some(e) = details["explanation"].as_str() {
                    self.stop_explanation = Some(e.to_string());
                }
                let u = &v["usage"];
                if let Some(o) = u64_of(u, "output_tokens") {
                    self.usage.output_tokens = o;
                }
                // Some responses repeat the input counts at the end; keep the larger.
                if let Some(i) = u64_of(u, "input_tokens") {
                    self.usage.input_tokens = self.usage.input_tokens.max(i);
                }
                if let Some(c) = u64_of(u, "cache_read_input_tokens") {
                    self.usage.cache_read_tokens = self.usage.cache_read_tokens.max(c);
                }
                if let Some(c) = u64_of(u, "cache_creation_input_tokens") {
                    self.usage.cache_write_tokens = self.usage.cache_write_tokens.max(c);
                }
            }
            "message_stop" => self.finished = true,
            "error" => {
                let m = v["error"]["message"]
                    .as_str()
                    .or_else(|| v["error"]["type"].as_str())
                    .unwrap_or("unknown stream error");
                self.stream_error = Some(m.to_string());
            }
            _ => {}
        }
        out
    }

    /// The assembled message, or why the stream is unusable.
    pub fn finish(self) -> Result<AssembledMessage, ApiError> {
        if let Some(e) = self.stream_error {
            return Err(ApiError::Stream(e));
        }
        if !self.started || (!self.finished && self.stop_reason.is_none()) {
            return Err(ApiError::Stream(
                "the response ended before it was complete".to_string(),
            ));
        }
        let mut content = Vec::new();
        let mut input_errors = BTreeMap::new();
        for (_, b) in self.blocks {
            let mut v = b.value;
            match v["type"].as_str().unwrap_or_default() {
                "text" => {
                    if v["text"].as_str().is_none_or(str::is_empty) {
                        continue; // the API rejects empty text blocks in history
                    }
                }
                "tool_use" => {
                    let id = v["id"].as_str().unwrap_or_default().to_string();
                    let buf = b.json_buf.trim();
                    if buf.is_empty() {
                        v["input"] = json!({});
                    } else {
                        match serde_json::from_str::<Value>(buf) {
                            Ok(i) if i.is_object() => v["input"] = i,
                            Ok(_) => {
                                v["input"] = json!({});
                                input_errors.insert(
                                    id,
                                    "The tool input must be a JSON object.".to_string(),
                                );
                            }
                            Err(e) => {
                                v["input"] = json!({});
                                let cut = if self.stop_reason.as_deref() == Some("max_tokens") {
                                    " The response was cut off at the output limit; make the call smaller."
                                } else {
                                    ""
                                };
                                input_errors.insert(
                                    id,
                                    format!("The tool input was not valid JSON ({e}).{cut}"),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
            content.push(v);
        }
        Ok(AssembledMessage {
            content,
            stop_reason: self.stop_reason,
            stop_explanation: self.stop_explanation,
            usage: self.usage,
            input_errors,
        })
    }
}

fn append_str(block: &mut Value, key: &str, s: &str) {
    let cur = block[key].as_str().unwrap_or_default().to_string();
    block[key] = Value::String(cur + s);
}

// ----- the client -----

/// Sleeps for the given time in small slices, returning early when `cancel` is set.
pub type Sleeper = Box<dyn FnMut(Duration, &AtomicBool) + Send>;

fn real_sleep(d: Duration, cancel: &AtomicBool) {
    let end = std::time::Instant::now() + d;
    while std::time::Instant::now() < end && !cancel.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Sends requests through a [`Transport`] with retries.
pub struct ApiClient {
    transport: Box<dyn Transport>,
    sleeper: Sleeper,
}

const MAX_RAW_BODY: usize = 64 * 1024;
const MAX_SERVER_RETRIES: usize = 3;
const MAX_RETRY_AFTER_SECS: u64 = 60;

impl ApiClient {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            sleeper: Box::new(real_sleep),
        }
    }

    /// A client on the real HTTP transport.
    pub fn ureq() -> Self {
        Self::new(Box::new(UreqTransport::new()))
    }

    /// Replaces the backoff sleeper (tests use a no-op).
    pub fn with_sleeper(mut self, sleeper: Sleeper) -> Self {
        self.sleeper = sleeper;
        self
    }

    /// Posts `request` and assembles the streamed response, calling
    /// `on_delta` as text, thinking and tool starts arrive.
    ///
    /// 401 maps to [`ApiError::Auth`]; 429 and 5xx retry up to 3 times after
    /// 2, 4 and 8 seconds (or the server's `retry-after`); 400 maps to the
    /// server's message; a connection error retries once.
    pub fn send(
        &mut self,
        cfg: &AgentConfig,
        auth: &Auth,
        request: &Request,
        cancel: &AtomicBool,
        on_delta: &mut dyn FnMut(Delta),
    ) -> Result<AssembledMessage, ApiError> {
        let url = format!("{}/v1/messages", cfg.base_url.trim_end_matches('/'));
        let headers = build_headers(auth);
        let body = request.to_json_string();
        let mut server_retries = 0usize;
        let mut connection_retries = 0usize;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(ApiError::Cancelled);
            }
            let mut asm = StreamAssembler::new();
            let mut raw = String::new();
            let result = self
                .transport
                .post_stream(&url, &headers, &body, &mut |line| {
                    if cancel.load(Ordering::SeqCst) {
                        return false;
                    }
                    if raw.len() < MAX_RAW_BODY {
                        raw.push_str(line);
                        raw.push('\n');
                    }
                    for d in asm.push_line(line) {
                        on_delta(d);
                    }
                    true
                });
            if cancel.load(Ordering::SeqCst) {
                return Err(ApiError::Cancelled);
            }
            let status = match result {
                Ok(s) => s,
                Err(TransportError::Connection(m)) => {
                    if connection_retries < 1 {
                        connection_retries += 1;
                        (self.sleeper)(Duration::from_secs(1), cancel);
                        continue;
                    }
                    return Err(ApiError::Connection(m));
                }
                Err(TransportError::Other(m)) => return Err(ApiError::Connection(m)),
            };
            match status {
                200..=299 => return asm.finish(),
                401 => return Err(ApiError::Auth),
                429 | 500..=599 => {
                    if server_retries < MAX_SERVER_RETRIES {
                        let backoff = 2u64 << server_retries;
                        let wait = self
                            .transport
                            .retry_after_secs()
                            .map(|s| s.min(MAX_RETRY_AFTER_SECS))
                            .unwrap_or(backoff);
                        server_retries += 1;
                        (self.sleeper)(Duration::from_secs(wait), cancel);
                        continue;
                    }
                    return Err(ApiError::Http {
                        status,
                        message: error_message(&raw, status),
                    });
                }
                400 => return Err(ApiError::BadRequest(error_message(&raw, status))),
                _ => {
                    return Err(ApiError::Http {
                        status,
                        message: error_message(&raw, status),
                    })
                }
            }
        }
    }
}

/// `error.message` of a JSON error body, else a generic text.
fn error_message(raw: &str, status: u16) -> String {
    let parsed = serde_json::from_str::<Value>(raw.trim()).ok().or_else(|| {
        raw.lines()
            .find_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
    });
    parsed
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {status}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(v: Value) -> String {
        format!("event: x\ndata: {v}\n\n")
    }

    #[test]
    fn sse_lines_parse() {
        assert_eq!(parse_sse_line("event: ping"), SseLine::Event("ping".into()));
        assert_eq!(
            parse_sse_line("data: {\"a\":1}"),
            SseLine::Data("{\"a\":1}".into())
        );
        assert_eq!(parse_sse_line("data:{}"), SseLine::Data("{}".into()));
        assert_eq!(parse_sse_line(""), SseLine::Blank);
        assert_eq!(parse_sse_line(": comment"), SseLine::Ignored);
    }

    #[test]
    fn a_stream_with_thinking_text_and_two_tool_calls_assembles() {
        let events = vec![
            json!({"type":"message_start","message":{"id":"m1","usage":{"input_tokens":100,"output_tokens":1,"cache_read_input_tokens":50,"cache_creation_input_tokens":7}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me "}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"plan."}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG=="}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Building "}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"now."}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"toolu_1","name":"add_wall_rectangle","input":{}}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"floor\": 1, \"x\": 0,"}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":" \"y\": 0, \"width\": 30,"}}),
            json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":" \"depth\": 40, \"kind\": \"exterior\"}"}}),
            json!({"type":"content_block_stop","index":2}),
            json!({"type":"content_block_start","index":3,"content_block":{"type":"tool_use","id":"toolu_2","name":"read_parameters","input":{}}}),
            json!({"type":"content_block_stop","index":3}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":321}}),
            json!({"type":"message_stop"}),
        ];
        let mut asm = StreamAssembler::new();
        let mut deltas = Vec::new();
        for e in &events {
            let text = data(e.clone());
            for line in text.lines() {
                deltas.extend(asm.push_line(line));
            }
        }
        // pings and comments are harmless
        assert!(asm.push_line("event: ping").is_empty());
        assert!(asm.push_line(": keep-alive").is_empty());
        let msg = asm.finish().expect("complete");
        assert_eq!(msg.stop_reason.as_deref(), Some("tool_use"));
        assert_eq!(msg.usage.input_tokens, 100);
        assert_eq!(msg.usage.output_tokens, 321);
        assert_eq!(msg.usage.cache_read_tokens, 50);
        assert_eq!(msg.usage.cache_write_tokens, 7);
        assert_eq!(msg.usage.turns, 1);
        assert_eq!(msg.content.len(), 4);
        assert_eq!(
            msg.content[0],
            json!({"type":"thinking","thinking":"Let me plan.","signature":"SIG=="})
        );
        assert_eq!(
            msg.content[1],
            json!({"type":"text","text":"Building now."})
        );
        assert_eq!(
            msg.content[2],
            json!({"type":"tool_use","id":"toolu_1","name":"add_wall_rectangle",
                   "input":{"floor":1,"x":0,"y":0,"width":30,"depth":40,"kind":"exterior"}})
        );
        assert_eq!(
            msg.content[3],
            json!({"type":"tool_use","id":"toolu_2","name":"read_parameters","input":{}})
        );
        assert!(msg.input_errors.is_empty());
        assert_eq!(
            deltas,
            vec![
                Delta::Thinking("Let me ".into()),
                Delta::Thinking("plan.".into()),
                Delta::Text("Building ".into()),
                Delta::Text("now.".into()),
                Delta::ToolStart {
                    name: "add_wall_rectangle".into()
                },
                Delta::ToolStart {
                    name: "read_parameters".into()
                },
            ]
        );
    }

    #[test]
    fn truncated_tool_input_is_reported_not_executed() {
        let mut asm = StreamAssembler::new();
        let evs = [
            json!({"type":"message_start","message":{"usage":{"input_tokens":1,"output_tokens":1}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t9","name":"add_wall","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"floor\": 1, \"start\": [0,"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"},"usage":{"output_tokens":9}}),
            json!({"type":"message_stop"}),
        ];
        for e in &evs {
            asm.push_line(&format!("data: {e}"));
        }
        let msg = asm.finish().unwrap();
        assert_eq!(msg.content[0]["input"], json!({}));
        assert!(msg.input_errors["t9"].contains("cut off"));
    }

    #[test]
    fn a_stream_error_event_and_a_cut_stream_fail() {
        let mut asm = StreamAssembler::new();
        asm.push_line(
            r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        );
        assert_eq!(
            asm.finish().unwrap_err(),
            ApiError::Stream("Overloaded".into())
        );

        let mut asm = StreamAssembler::new();
        asm.push_line(r#"data: {"type":"message_start","message":{"usage":{}}}"#);
        assert!(matches!(asm.finish(), Err(ApiError::Stream(_))));
    }

    #[test]
    fn auth_resolution_prefers_config_then_key_then_token() {
        let env = |k: &str| match k {
            "ANTHROPIC_API_KEY" => Some("env-key".to_string()),
            "ANTHROPIC_AUTH_TOKEN" => Some("env-token".to_string()),
            _ => None,
        };
        assert_eq!(
            resolve_auth(Some("cfg"), &env),
            Some(Auth::ApiKey("cfg".into()))
        );
        assert_eq!(
            resolve_auth(None, &env),
            Some(Auth::ApiKey("env-key".into()))
        );
        assert_eq!(
            resolve_auth(Some("  "), &env),
            Some(Auth::ApiKey("env-key".into()))
        );
        let only_token = |k: &str| (k == "ANTHROPIC_AUTH_TOKEN").then(|| "tok".to_string());
        let auth = resolve_auth(None, &only_token).unwrap();
        assert_eq!(auth, Auth::Bearer("tok".into()));
        let h = build_headers(&auth);
        assert!(h.contains(&("authorization".into(), "Bearer tok".into())));
        assert!(h.iter().any(|(k, v)| k == "anthropic-beta"
            && v == "server-side-fallback-2026-07-01,oauth-2025-04-20"));
        assert!(!h.iter().any(|(k, _)| k == "x-api-key"));
        assert_eq!(resolve_auth(None, &|_| None), None);
    }

    #[test]
    fn api_key_headers() {
        let h = build_headers(&Auth::ApiKey("sk-1".into()));
        assert!(h.contains(&("x-api-key".into(), "sk-1".into())));
        assert!(h.contains(&("anthropic-version".into(), "2023-06-01".into())));
        assert!(h.contains(&("anthropic-beta".into(), BETA_FALLBACK.into())));
        assert_eq!(
            format!("{:?}", Auth::ApiKey("sk-secret".into())),
            "Auth::ApiKey(<redacted>)"
        );
    }

    #[test]
    fn redaction_removes_the_key() {
        let s = redact("bad key sk-ant-12345 here", &["sk-ant-12345".to_string()]);
        assert_eq!(s, "bad key [redacted] here");
    }
}
