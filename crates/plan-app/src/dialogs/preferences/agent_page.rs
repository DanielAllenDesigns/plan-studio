//! Preferences > Plan Agent: the Anthropic API key, the model and the effort
//! the Plan Agent dock starts with.
//!
//! The three choices ([`AgentPrefs`]) are flattened into `preferences.json`
//! (`agent_api_key`, `agent_model`, `agent_effort`) through
//! [`super::pages::PagePrefs`]. The key lives in that user file only; it is
//! never written to a plan file, never logged and never shown in clear (the
//! field is masked and `Debug` redacts it).

use super::pages;
use eframe::egui;
use plan_agent::{AgentConfig, Effort};
use serde::{Deserialize, Serialize};

/// The model the agent uses when the field is empty.
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// The effort key the agent uses when the field is empty or unknown.
pub const DEFAULT_EFFORT: &str = "medium";

/// The effort choices in menu order, with their file key and menu label.
pub const EFFORTS: [(&str, &str); 4] = [
    ("low", "Low"),
    ("medium", "Medium"),
    ("high", "High"),
    ("xhigh", "Extra high"),
];

fn model() -> String {
    DEFAULT_MODEL.to_string()
}

fn effort() -> String {
    DEFAULT_EFFORT.to_string()
}

/// What the Plan Agent page keeps.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPrefs {
    /// The Anthropic API key. Empty: use `ANTHROPIC_API_KEY` from the
    /// environment.
    #[serde(default)]
    pub agent_api_key: String,
    /// The model id.
    #[serde(default = "model")]
    pub agent_model: String,
    /// "low", "medium", "high" or "xhigh".
    #[serde(default = "effort")]
    pub agent_effort: String,
}

impl Default for AgentPrefs {
    fn default() -> Self {
        Self {
            agent_api_key: String::new(),
            agent_model: model(),
            agent_effort: effort(),
        }
    }
}

// The key must not reach a log through `{:?}` of the preferences.
impl std::fmt::Debug for AgentPrefs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPrefs")
            .field(
                "agent_api_key",
                &if self.agent_api_key.is_empty() {
                    "<empty>"
                } else {
                    "<hidden>"
                },
            )
            .field("agent_model", &self.agent_model)
            .field("agent_effort", &self.agent_effort)
            .finish()
    }
}

/// The effort a file key stands for (unknown keys: Medium).
pub fn effort_from_key(key: &str) -> Effort {
    match key.trim().to_ascii_lowercase().as_str() {
        "low" => Effort::Low,
        "high" => Effort::High,
        "xhigh" => Effort::XHigh,
        _ => Effort::Medium,
    }
}

/// The file key of an effort.
pub fn effort_key(e: &Effort) -> &'static str {
    match e {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::XHigh => "xhigh",
    }
}

/// The menu label of an effort.
pub fn effort_label(e: &Effort) -> &'static str {
    EFFORTS
        .iter()
        .find(|(k, _)| *k == effort_key(e))
        .map_or("Medium", |(_, l)| l)
}

/// The saved choices.
pub fn prefs() -> AgentPrefs {
    pages::current().agent
}

/// The agent's configuration from the saved choices; `effort` is the dock's
/// own choice (the dock keeps the page's effort in step with it).
pub fn config_with(effort: Effort) -> AgentConfig {
    let p = prefs();
    let key = p.agent_api_key.trim();
    let model = p.agent_model.trim();
    let dflt = AgentConfig::default();
    AgentConfig {
        api_key: (!key.is_empty()).then(|| key.to_string()),
        model: if model.is_empty() {
            dflt.model.clone()
        } else {
            model.to_string()
        },
        effort,
        ..dflt
    }
}

/// The agent's configuration with the saved effort.
pub fn config() -> AgentConfig {
    config_with(effort_from_key(&prefs().agent_effort))
}

/// Saves the effort choice (the dock's combo).
pub fn set_effort(e: &Effort) {
    let key = effort_key(e);
    if prefs().agent_effort != key {
        pages::update(|p| p.agent.agent_effort = key.to_string());
    }
}

/// The page body. Every change applies at once (the dialog saves the file
/// when the mouse is released).
pub fn show(ui: &mut egui::Ui) {
    let before = prefs();
    let mut p = before.clone();

    ui.label("The Plan Agent dock (View > Plan Agent) asks Claude to edit the plan for you.");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Anthropic API key");
        ui.add(
            egui::TextEdit::singleline(&mut p.agent_api_key)
                .password(true)
                .hint_text("sk-ant-...")
                .desired_width(260.0),
        );
        if ui
            .add_enabled(!p.agent_api_key.is_empty(), egui::Button::new("Forget key"))
            .clicked()
        {
            p.agent_api_key.clear();
        }
    });
    ui.weak("Use ANTHROPIC_API_KEY from the environment when empty.");
    ui.weak(
        "The key is saved in your user preferences file only. It is never stored in a plan \
         file and is never shown or logged.",
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Model");
        ui.add(
            egui::TextEdit::singleline(&mut p.agent_model)
                .hint_text(DEFAULT_MODEL)
                .desired_width(220.0),
        );
        if ui.button("Default").clicked() {
            p.agent_model = DEFAULT_MODEL.to_string();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Effort");
        let cur = effort_from_key(&p.agent_effort);
        egui::ComboBox::from_id_salt("prefs_agent_effort")
            .selected_text(effort_label(&cur))
            .show_ui(ui, |ui| {
                for (key, label) in EFFORTS {
                    if ui
                        .selectable_label(effort_key(&cur) == key, label)
                        .clicked()
                    {
                        p.agent_effort = key.to_string();
                    }
                }
            });
    });
    ui.add_space(6.0);
    if config().credentials_available() {
        ui.colored_label(
            egui::Color32::from_rgb(0x4C, 0xAF, 0x50),
            "A key is available.",
        );
    } else {
        ui.colored_label(
            egui::Color32::from_rgb(0xE5, 0x73, 0x73),
            "No key yet: enter one above or set ANTHROPIC_API_KEY.",
        );
    }
    ui.weak(
        "A \"Test connection\" button is not offered: plan-agent has no one-line ping call yet.",
    );

    if p != before {
        pages::update(|pp| pp.agent = p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_keys_round_trip() {
        for (key, _) in EFFORTS {
            assert_eq!(effort_key(&effort_from_key(key)), key);
        }
        assert_eq!(effort_from_key("nonsense"), Effort::Medium);
        assert_eq!(effort_from_key(" HIGH "), Effort::High);
    }

    #[test]
    fn debug_hides_the_key() {
        let p = AgentPrefs {
            agent_api_key: "sk-ant-secret".into(),
            ..AgentPrefs::default()
        };
        let text = format!("{p:?}");
        assert!(!text.contains("secret"), "{text}");
    }
}
