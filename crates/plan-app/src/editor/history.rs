//! Undo/redo with names. Wraps `plan_core::History` (whole-project snapshots)
//! and keeps a label per step ("Move Wall") for the Edit menu.

use plan_core::{History, Project};

#[derive(Debug, Default)]
pub struct ChangeHistory {
    history: History,
    /// One label per entry of the undo stack; `None` marks a cancelled step.
    past: Vec<Option<String>>,
    future: Vec<Option<String>>,
    /// The newest step may absorb further `begin_change_merged` calls.
    merge_open: bool,
}

impl ChangeHistory {
    const CAP: usize = 100;

    pub fn new() -> Self {
        Self {
            history: History::new(Self::CAP),
            past: Vec::new(),
            future: Vec::new(),
            merge_open: false,
        }
    }

    /// Records `project` (the state before a change) as the step `label`.
    pub fn begin(&mut self, project: &Project, label: &str) {
        self.history.push(project);
        self.past.push(Some(label.to_string()));
        self.future.clear();
        if self.past.len() > Self::CAP {
            let excess = self.past.len() - Self::CAP;
            self.past.drain(..excess);
        }
        self.merge_open = false;
    }

    /// Like [`begin`](Self::begin), but a run of calls with the same label
    /// (a slider drag) is one step. Call [`end_merge`](Self::end_merge) when
    /// the gesture is over.
    pub fn begin_merged(&mut self, project: &Project, label: &str) {
        let same = self.merge_open
            && matches!(self.past.last(), Some(Some(l)) if l == label)
            && self.future.is_empty();
        if !same {
            self.begin(project, label);
            self.merge_open = true;
        }
    }

    pub fn end_merge(&mut self) {
        self.merge_open = false;
    }

    /// Marks the newest step as a no-op (the change turned out to be refused).
    /// The step is skipped by undo and redo.
    pub fn cancel(&mut self) {
        if let Some(last) = self.past.last_mut() {
            *last = None;
        }
        self.merge_open = false;
    }

    pub fn can_undo(&self) -> bool {
        self.past.iter().any(Option::is_some)
    }

    pub fn can_redo(&self) -> bool {
        self.future.iter().any(Option::is_some)
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.past.iter().rev().find_map(|l| l.as_deref())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.iter().rev().find_map(|l| l.as_deref())
    }

    /// Restores the previous state into `project`; returns the step's label.
    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        self.merge_open = false;
        while self.history.can_undo() {
            let prev = self.history.undo(project)?;
            *project = prev;
            let label = self.past.pop().flatten();
            self.future.push(label.clone());
            if label.is_some() {
                return label;
            }
        }
        None
    }

    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        self.merge_open = false;
        while self.history.can_redo() {
            let next = self.history.redo(project)?;
            *project = next;
            let label = self.future.pop().flatten();
            self.past.push(label.clone());
            if label.is_some() {
                return label;
            }
        }
        None
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.past.clear();
        self.future.clear();
        self.merge_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_undo_and_redo() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        h.begin(&p, "First");
        p.name = "b".into();
        h.begin(&p, "Second");
        p.name = "c".into();
        assert_eq!(h.undo_label(), Some("Second"));
        assert_eq!(h.undo(&mut p).as_deref(), Some("Second"));
        assert_eq!(p.name, "b");
        assert_eq!(h.redo_label(), Some("Second"));
        assert_eq!(h.redo(&mut p).as_deref(), Some("Second"));
        assert_eq!(p.name, "c");
    }

    #[test]
    fn cancelled_steps_are_skipped() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        h.begin(&p, "Real");
        p.name = "b".into();
        h.begin(&p, "Refused");
        h.cancel();
        assert_eq!(h.undo_label(), Some("Real"));
        assert_eq!(h.undo(&mut p).as_deref(), Some("Real"));
        assert_eq!(p.name, "a");
        assert!(!h.can_undo());
        assert_eq!(h.redo(&mut p).as_deref(), Some("Real"));
        assert_eq!(p.name, "b");
        assert!(!h.can_redo());
    }

    #[test]
    fn merged_steps_collapse() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        for n in ["b", "c", "d"] {
            h.begin_merged(&p, "Thickness");
            p.name = n.into();
        }
        h.end_merge();
        assert_eq!(h.undo(&mut p).as_deref(), Some("Thickness"));
        assert_eq!(p.name, "a");
        assert!(!h.can_undo());
    }
}
