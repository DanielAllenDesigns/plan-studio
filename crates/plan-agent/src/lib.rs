//! plan-agent: the native "Ask to make changes" agent of Plan Studio.
//!
//! The user types a request, the agent calls Claude (Anthropic Messages API)
//! with tools that read and edit the plan model, and the result comes back
//! as an edited clone of the project for the editor to swap in as ONE undo
//! step. This crate has no GUI code.
//!
//! * [`api`]: the Messages API transport (streaming SSE) behind a trait.
//! * [`tools`]: the tool catalog and the executor against a `&mut Project`.
//! * [`session`]: conversation state and the run loop.
//! * [`prompt`]: the system prompt and the plan summary text.
//!
//! Units: tools use FEET; plan-core stores inches.

pub mod api;
pub mod prompt;
pub mod session;
pub mod tools;

pub use session::{
    AgentConfig, AgentEvent, AgentSession, Effort, Parameter, RunHandle, TranscriptEntry, Usage,
};
