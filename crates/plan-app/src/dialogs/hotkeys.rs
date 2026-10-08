//! Customize Hotkeys, modeled on Chief's dialog: a "Show Commands/Hotkeys
//! Containing" filter, a Command Name | Hotkey table, and an "Assign a
//! sequence of up to 4 hotkeys" field with Assign and Remove, then Reset
//! Hotkeys and Help / Cancel / OK.
//!
//! The dialog edits a copy of the [`HotkeyMap`]; OK writes it back and saves
//! `~/.plan-studio/hotkeys.json`, Cancel drops it.

use super::Outcome;
use crate::shell::hotkeys::{sequence_label, Chord, Command, HotkeyMap, MAX_SEQUENCE};
use eframe::egui::{self, Align, Align2, Color32, Event, Key, Layout, RichText, Vec2};

const WARN: Color32 = Color32::from_rgb(0xE0, 0xA0, 0x30);

/// What pressing Assign did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignResult {
    Assigned,
    /// The sequence clashes with these commands; nothing changed.
    Conflict(Vec<String>),
    /// No command selected or no keys recorded.
    Nothing,
}

pub struct HotkeyDialog {
    draft: HotkeyMap,
    filter: String,
    selected: Option<String>,
    /// The sequence being recorded (up to [`MAX_SEQUENCE`] chords).
    seq: Vec<Chord>,
    recording: bool,
    /// Commands the recorded sequence clashes with, once Assign was pressed.
    conflict: Option<Vec<String>>,
    /// The assigned sequence picked for Remove.
    chosen: Option<usize>,
    message: String,
}

impl HotkeyDialog {
    pub fn new(map: &HotkeyMap) -> Self {
        HotkeyDialog {
            draft: map.clone(),
            filter: String::new(),
            selected: None,
            seq: Vec::new(),
            recording: false,
            conflict: None,
            chosen: None,
            message: String::new(),
        }
    }

    /// The commands whose name or hotkey contains the filter, by name.
    pub fn visible_commands(&self) -> Vec<&Command> {
        let needle = self.filter.trim().to_lowercase();
        let mut v: Vec<&Command> = self
            .draft
            .commands()
            .iter()
            .filter(|c| {
                needle.is_empty()
                    || c.name.to_lowercase().contains(&needle)
                    || self
                        .draft
                        .hotkey_text(&c.name)
                        .to_lowercase()
                        .contains(&needle)
            })
            .collect();
        v.sort_by_key(|c| c.name.to_lowercase());
        v
    }

    pub fn select(&mut self, name: &str) {
        self.selected = Some(name.to_string());
        self.chosen = None;
        self.conflict = None;
    }

    /// Adds a pressed chord to the sequence being recorded. Returns false
    /// when the sequence is already [`MAX_SEQUENCE`] long.
    pub fn record(&mut self, chord: Chord) -> bool {
        self.conflict = None;
        if self.seq.len() >= MAX_SEQUENCE {
            return false;
        }
        self.seq.push(chord);
        true
    }

    pub fn clear_sequence(&mut self) {
        self.seq.clear();
        self.conflict = None;
    }

    /// Assign: adds the recorded sequence to the selected command. With
    /// `steal` a clash is resolved by taking the sequence from the others.
    pub fn assign(&mut self, steal: bool) -> AssignResult {
        let Some(cmd) = self.selected.clone() else {
            return AssignResult::Nothing;
        };
        if self.seq.is_empty() {
            return AssignResult::Nothing;
        }
        match self.draft.assign(&cmd, self.seq.clone(), steal) {
            Ok(()) => {
                self.message = format!("{cmd}: {}", sequence_label(&self.seq));
                self.seq.clear();
                self.conflict = None;
                AssignResult::Assigned
            }
            Err(names) if names.is_empty() => AssignResult::Nothing,
            Err(names) => {
                self.conflict = Some(names.clone());
                AssignResult::Conflict(names)
            }
        }
    }

    /// Remove: drops the picked sequence of the selected command.
    pub fn remove_chosen(&mut self) -> bool {
        let (Some(cmd), Some(i)) = (self.selected.clone(), self.chosen) else {
            return false;
        };
        let Some(seq) = self.draft.sequences(&cmd).get(i).cloned() else {
            return false;
        };
        self.draft.remove(&cmd, &seq);
        self.chosen = None;
        true
    }

    pub fn choose(&mut self, index: usize) {
        self.chosen = Some(index);
    }

    /// Reset Hotkeys: Chief's and Daniel's defaults again.
    pub fn reset(&mut self) {
        self.draft.reset();
        self.chosen = None;
        self.conflict = None;
        self.message = "Hotkeys reset to the defaults".into();
    }

    /// OK: copies the edited map into `target` and saves it to
    /// `~/.plan-studio/hotkeys.json`.
    pub fn commit(&self, target: &mut HotkeyMap) -> Result<(), String> {
        *target = self.draft.clone();
        target.save()
    }

    #[cfg(test)]
    pub fn commit_to(&self, target: &mut HotkeyMap, path: &std::path::Path) -> Result<(), String> {
        *target = self.draft.clone();
        target.save_to(path)
    }

    fn capture_keys(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = e
            {
                if key == Key::Escape {
                    self.recording = false;
                } else {
                    self.record(Chord::from_event(key, &modifiers));
                }
            }
        }
    }

    /// Draws the window and reports OK / Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        if self.recording {
            self.capture_keys(ctx);
        }
        let mut outcome = Outcome::Open;
        let mut open = true;
        egui::Window::new("Customize Hotkeys")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(560.0, 580.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                self.body(ui, &mut outcome);
            });
        if !open && outcome == Outcome::Open {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn body(&mut self, ui: &mut egui::Ui, outcome: &mut Outcome) {
        ui.horizontal(|ui| {
            ui.label("Show Commands/Hotkeys Containing:");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter).desired_width(ui.available_width()),
            );
        });
        let s = self.draft.daniel_stats();
        ui.weak(format!(
            "Daniel's Chief hotkeys: {} named, {} have a Plan Studio command ({} work today).",
            s.named, s.mapped, s.live
        ));
        ui.add_space(4.0);

        let mut picked: Option<String> = None;
        egui::ScrollArea::vertical()
            .id_salt("hotkey_table")
            .max_height((ui.available_height() - 230.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("hotkey_grid")
                    .num_columns(2)
                    .striped(true)
                    .spacing(Vec2::new(16.0, 3.0))
                    .show(ui, |ui| {
                        ui.strong("Command Name");
                        ui.strong("Hotkey");
                        ui.end_row();
                        for c in self.visible_commands() {
                            let selected = self.selected.as_deref() == Some(c.name.as_str());
                            let text = if c.is_live() {
                                RichText::new(&c.name)
                            } else {
                                RichText::new(&c.name).weak()
                            };
                            let resp = ui.selectable_label(selected, text);
                            if resp.clicked() {
                                picked = Some(c.name.clone());
                            }
                            if !c.is_live() {
                                resp.on_hover_text("Not built yet; the key is kept for later");
                            }
                            ui.label(self.draft.hotkey_text(&c.name));
                            ui.end_row();
                        }
                    });
                let unmapped = self.draft.unmapped();
                if !unmapped.is_empty() {
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(format!(
                        "Chief bindings with no action in Plan Studio yet ({})",
                        unmapped.len()
                    ))
                    .id_salt("hotkey_unmapped")
                    .default_open(false)
                    .show(ui, |ui| {
                        egui::Grid::new("hotkey_unmapped_grid")
                            .num_columns(2)
                            .striped(true)
                            .spacing(Vec2::new(16.0, 3.0))
                            .show(ui, |ui| {
                                for u in unmapped {
                                    ui.weak(&u.name);
                                    ui.weak(&u.chord_text);
                                    ui.end_row();
                                }
                            });
                    });
                }
            });
        if let Some(name) = picked {
            self.select(&name);
        }

        ui.separator();
        self.assign_area(ui);

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Reset Hotkeys").clicked() {
                self.reset();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("OK").clicked() {
                    *outcome = Outcome::Ok;
                }
                if ui.button("Cancel").clicked() {
                    *outcome = Outcome::Cancel;
                }
                if ui.button("Help").clicked() {
                    self.message =
                        "Pick a command, click the field, press up to 4 keys, then Assign.".into();
                }
            });
        });
    }

    fn assign_area(&mut self, ui: &mut egui::Ui) {
        let Some(cmd) = self.selected.clone() else {
            ui.weak("Select a command to assign hotkeys.");
            if !self.message.is_empty() {
                ui.weak(&self.message);
            }
            return;
        };
        ui.label(format!(
            "Assign a sequence of up to {MAX_SEQUENCE} hotkeys to {cmd}:"
        ));
        let text = if self.seq.is_empty() {
            if self.recording {
                "Press keys...".to_string()
            } else {
                "Click here, then press keys".to_string()
            }
        } else {
            sequence_label(&self.seq)
        };
        let mut do_assign = false;
        let mut steal = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::Button::new(text)
                    .selected(self.recording)
                    .min_size(Vec2::new(260.0, 24.0)),
            );
            if field.clicked() {
                self.recording = true;
            } else if self.recording
                && ui.input(|i| i.pointer.any_pressed())
                && !field.contains_pointer()
            {
                self.recording = false;
            }
            if ui.button("Clear").clicked() {
                self.clear_sequence();
            }
            let can = !self.seq.is_empty();
            if ui.add_enabled(can, egui::Button::new("Assign")).clicked() {
                do_assign = true;
            }
        });
        if let Some(names) = &self.conflict {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Already used by: {}.", names.join(", "))).color(WARN),
                );
                if ui.button("Reassign").clicked() {
                    steal = true;
                    do_assign = true;
                }
            });
        }
        if do_assign {
            self.assign(steal);
        }

        let current = self.draft.sequences(&cmd).to_vec();
        ui.horizontal_wrapped(|ui| {
            ui.label("Current hotkeys:");
            if current.is_empty() {
                ui.weak("none");
            }
            for (i, seq) in current.iter().enumerate() {
                if ui
                    .selectable_label(self.chosen == Some(i), sequence_label(seq))
                    .clicked()
                {
                    self.choose(i);
                }
            }
            if ui
                .add_enabled(self.chosen.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                self.remove_chosen();
            }
        });
        if !self.message.is_empty() {
            ui.weak(&self.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar::Action;

    fn chord(key: Key) -> Chord {
        Chord {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        }
    }

    fn dialog() -> HotkeyDialog {
        HotkeyDialog::new(&HotkeyMap::defaults())
    }

    #[test]
    fn filter_matches_command_names_and_hotkeys() {
        let mut d = dialog();
        let all = d.visible_commands().len();
        assert!(all > 100, "{all} commands");
        d.filter = "hinged".into();
        let hits = d.visible_commands();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Hinged Door");
        // By hotkey text.
        d.filter = "d, h".into();
        assert!(d.visible_commands().iter().any(|c| c.name == "Hinged Door"));
        d.filter = "zzzz".into();
        assert!(d.visible_commands().is_empty());
        d.filter = "".into();
        let names: Vec<_> = d
            .visible_commands()
            .iter()
            .map(|c| c.name.to_lowercase())
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn recording_stops_at_four_chords() {
        let mut d = dialog();
        for k in [Key::A, Key::B, Key::C, Key::D] {
            assert!(d.record(chord(k)));
        }
        assert!(!d.record(chord(Key::E)));
        assert_eq!(d.seq.len(), 4);
        d.clear_sequence();
        assert!(d.seq.is_empty());
    }

    #[test]
    fn assign_warns_on_conflict_and_reassign_moves_the_key() {
        let mut d = dialog();
        d.select("Zoom Out");
        d.record(chord(Key::Minus));
        assert_eq!(
            d.assign(false),
            AssignResult::Conflict(vec!["Zoom In".to_string()])
        );
        assert!(d.draft.sequences("Zoom Out").is_empty());
        assert_eq!(d.assign(true), AssignResult::Assigned);
        assert_eq!(d.draft.lookup(&[chord(Key::Minus)]), Some(Action::ZoomOut));
        assert!(d.draft.sequences("Zoom In").is_empty());
        // Nothing selected or nothing recorded.
        d.record(chord(Key::J));
        d.selected = None;
        assert_eq!(d.assign(false), AssignResult::Nothing);
    }

    #[test]
    fn remove_drops_the_chosen_sequence() {
        let mut d = dialog();
        d.select("Hinged Door");
        assert!(!d.remove_chosen(), "nothing chosen yet");
        d.choose(0);
        assert!(d.remove_chosen());
        // Only the number-key alias is left.
        assert_eq!(d.draft.hotkey_text("Hinged Door"), "3");
        d.reset();
        assert_eq!(d.draft.hotkey_text("Hinged Door"), "D, H; 3");
    }

    #[test]
    fn ok_writes_the_map_and_the_file() {
        let dir = std::env::temp_dir().join(format!("plan-studio-hkdlg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("hotkeys.json");
        let mut live = HotkeyMap::defaults();
        let mut d = HotkeyDialog::new(&live);
        d.select("Zoom Out");
        d.record(chord(Key::Equals));
        assert_eq!(d.assign(false), AssignResult::Assigned);
        // Not applied until OK.
        assert_eq!(live.lookup(&[chord(Key::Equals)]), None);
        d.commit_to(&mut live, &path).unwrap();
        assert_eq!(live.lookup(&[chord(Key::Equals)]), Some(Action::ZoomOut));
        let mut again = HotkeyMap::defaults();
        again.load_overrides(&path);
        assert_eq!(again.lookup(&[chord(Key::Equals)]), Some(Action::ZoomOut));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
