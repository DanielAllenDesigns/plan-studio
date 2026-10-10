//! The "save changes?" prompt that guards New, Open, Close and Quit, the
//! revert confirmation, and the recovery prompt shown after a crash or when an
//! autosave is newer than the plan on disk.
//!
//! Each prompt is a modal window that returns its answer in the frame the user
//! gives it. Keys: Enter is the default button (Save, Revert, Recover), Cmd+D
//! (Ctrl+D elsewhere) is Don't Save, Escape is Cancel (Discard on the recovery
//! prompt has no key so it is never hit by accident).

use eframe::egui::{self, Key, Modifiers};

/// What the user chose in the "save changes?" prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Save the plan, then carry on.
    Save,
    /// Carry on and throw the changes away.
    DontSave,
    /// Stay where we are.
    Cancel,
}

/// What the prompt guards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Unsaved changes would be lost by `verb` ("create a new plan", "quit").
    Unsaved { verb: String },
    /// File > Revert to Saved: the changes would be thrown away.
    Revert,
}

/// An open prompt for the plan called `name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub name: String,
    pub kind: Kind,
}

impl Prompt {
    pub fn unsaved(name: &str, verb: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: Kind::Unsaved {
                verb: verb.to_string(),
            },
        }
    }

    pub fn revert(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: Kind::Revert,
        }
    }

    /// The question the window asks.
    pub fn message(&self) -> String {
        match &self.kind {
            Kind::Unsaved { verb } => {
                format!("Do you want to save the changes you made to \u{201C}{}\u{201D} before you {verb}?", self.name)
            }
            Kind::Revert => format!(
                "Revert \u{201C}{}\u{201D} to the last saved version? Changes made since then are lost.",
                self.name
            ),
        }
    }
}

/// The "don't ask again" key of the revert confirmation (Preferences >
/// Reset Options shows it again).
pub const REVERT_KEY: &str = "revert_to_saved";

/// Shows `prompt`; the answer when the user gives one. For
/// [`Kind::Revert`] the proceed button answers [`Outcome::DontSave`] (carry on
/// without saving) and there is no Save.
pub fn show(ctx: &egui::Context, prompt: &Prompt) -> Option<Outcome> {
    let mut answer = None;
    let revert = prompt.kind == Kind::Revert;
    egui::Modal::new(egui::Id::new("unsaved_changes_prompt")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.heading(if revert {
            "Revert to Saved"
        } else {
            "Unsaved Changes"
        });
        ui.add_space(6.0);
        ui.label(prompt.message());
        ui.add_space(10.0);
        let dont_ask = egui::Id::new("revert_dont_ask_again");
        if revert {
            let mut on = ui.data(|d| d.get_temp::<bool>(dont_ask)).unwrap_or(false);
            if ui.checkbox(&mut on, "Don\u{2019}t ask again").changed() {
                ui.data_mut(|d| d.insert_temp(dont_ask, on));
            }
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            if revert {
                if ui.button("Revert").clicked() {
                    answer = Some(Outcome::DontSave);
                }
            } else {
                if ui.button("Save").clicked() {
                    answer = Some(Outcome::Save);
                }
                if ui.button("Don\u{2019}t Save").clicked() {
                    answer = Some(Outcome::DontSave);
                }
            }
            if ui.button("Cancel").clicked() {
                answer = Some(Outcome::Cancel);
            }
        });
    });
    if answer.is_none() {
        answer = ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                Some(if revert {
                    Outcome::DontSave
                } else {
                    Outcome::Save
                })
            } else if !revert && i.consume_key(Modifiers::COMMAND, Key::D) {
                Some(Outcome::DontSave)
            } else if i.consume_key(Modifiers::NONE, Key::Escape) {
                Some(Outcome::Cancel)
            } else {
                None
            }
        });
    }
    // Reverting with "Don't ask again" ticked hides the question from now on.
    if revert && answer == Some(Outcome::DontSave) {
        let ticked = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("revert_dont_ask_again")));
        if ticked == Some(true) {
            crate::dialogs::preferences::pages::set_dont_ask(REVERT_KEY);
        }
    }
    answer
}

/// The choice on the recovery prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    Recover,
    Discard,
}

/// What recovered work the prompt describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryPrompt {
    /// The plan the work belongs to, or "an untitled plan".
    pub what: String,
    /// When the copy was written ("2026-10-08 14:03 UTC").
    pub when: String,
    /// True after a crash, false for a newer autosave.
    pub crash: bool,
}

impl RecoveryPrompt {
    pub fn message(&self) -> String {
        if self.crash {
            format!(
                "Plan Studio did not close normally. A copy of {} from {} was kept.",
                self.what, self.when
            )
        } else {
            format!(
                "An autosave of {} from {} is newer than the saved file.",
                self.what, self.when
            )
        }
    }
}

/// Shows the recovery prompt; Recover (Enter) opens the kept copy, Discard
/// deletes it.
pub fn show_recovery(ctx: &egui::Context, prompt: &RecoveryPrompt) -> Option<Recovery> {
    let mut answer = None;
    egui::Modal::new(egui::Id::new("recovery_prompt")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.heading("Recover Unsaved Work");
        ui.add_space(6.0);
        ui.label(prompt.message());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("Recover").clicked() {
                answer = Some(Recovery::Recover);
            }
            if ui.button("Discard").clicked() {
                answer = Some(Recovery::Discard);
            }
        });
    });
    if answer.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        answer = Some(Recovery::Recover);
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// Runs one frame of the prompt with `events` and returns its answer.
    fn run(prompt: &Prompt, events: Vec<egui::Event>) -> Option<Outcome> {
        let ctx = egui::Context::default();
        let mut out = None;
        // The first frame lays the window out; the keys arrive on the second.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = show(ctx, prompt);
        });
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            out = show(ctx, prompt);
        });
        out
    }

    #[test]
    fn the_prompt_returns_save_dont_save_and_cancel() {
        let p = Prompt::unsaved("house.psplan", "quit");
        assert_eq!(
            run(&p, vec![key(Key::Enter, Modifiers::NONE)]),
            Some(Outcome::Save)
        );
        assert_eq!(
            run(&p, vec![key(Key::D, Modifiers::COMMAND)]),
            Some(Outcome::DontSave)
        );
        assert_eq!(
            run(&p, vec![key(Key::Escape, Modifiers::NONE)]),
            Some(Outcome::Cancel)
        );
        assert_eq!(run(&p, vec![]), None);
    }

    #[test]
    fn the_revert_prompt_proceeds_or_cancels_and_never_saves() {
        let p = Prompt::revert("house.psplan");
        assert_eq!(
            run(&p, vec![key(Key::Enter, Modifiers::NONE)]),
            Some(Outcome::DontSave)
        );
        assert_eq!(run(&p, vec![key(Key::D, Modifiers::COMMAND)]), None);
        assert_eq!(
            run(&p, vec![key(Key::Escape, Modifiers::NONE)]),
            Some(Outcome::Cancel)
        );
    }

    #[test]
    fn the_messages_name_the_plan_and_the_action() {
        let m = Prompt::unsaved("house.psplan", "open another plan").message();
        assert!(m.contains("house.psplan") && m.contains("open another plan"));
        assert!(Prompt::revert("a.psplan").message().contains("last saved"));
        let r = RecoveryPrompt {
            what: "a.psplan".into(),
            when: "now".into(),
            crash: false,
        };
        assert!(r.message().contains("newer than the saved file"));
    }

    #[test]
    fn the_recovery_prompt_recovers_on_enter() {
        let ctx = egui::Context::default();
        let p = RecoveryPrompt {
            what: "a plan".into(),
            when: "then".into(),
            crash: true,
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let _ = show_recovery(ctx, &p);
        });
        let mut out = None;
        let input = egui::RawInput {
            events: vec![key(Key::Enter, Modifiers::NONE)],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| out = show_recovery(ctx, &p));
        assert_eq!(out, Some(Recovery::Recover));
    }
}
