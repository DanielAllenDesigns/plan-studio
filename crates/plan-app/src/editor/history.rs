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
    /// An open undo group (see [`begin_group`](Self::begin_group)).
    group: Group,
}

/// Everything done between `begin_group` and `end_group` is one undo step
/// (QA-24), and none when it changed nothing (QA-26).
#[derive(Debug, Default)]
struct Group {
    /// How many `begin_group` calls are open (groups nest).
    depth: usize,
    /// The plan before the first change of the group.
    before: Option<Project>,
    label: Option<String>,
}

impl ChangeHistory {
    const CAP: usize = 100;

    pub fn new() -> Self {
        Self {
            history: History::new(Self::CAP),
            past: Vec::new(),
            future: Vec::new(),
            merge_open: false,
            group: Group::default(),
        }
    }

    /// Opens an undo group: every `begin` until the matching
    /// [`end_group`](Self::end_group) is folded into one step.
    pub fn begin_group(&mut self) {
        self.group.depth += 1;
    }

    /// Closes the group opened by [`begin_group`](Self::begin_group). When the
    /// outermost group ends, one step (named after the first change in it) is
    /// recorded if `current` differs from the plan before the group's first
    /// change; a group that changed nothing leaves no step and keeps the redo
    /// steps. Returns whether a step was recorded.
    pub fn end_group(&mut self, current: &Project) -> bool {
        self.group.depth = self.group.depth.saturating_sub(1);
        if self.group.depth > 0 {
            return false;
        }
        let (Some(before), label) = (self.group.before.take(), self.group.label.take()) else {
            return false;
        };
        if before.to_json().ok() == current.to_json().ok() {
            return false;
        }
        self.begin(&before, label.as_deref().unwrap_or("Edit"));
        true
    }

    /// Records `project` (the state before a change) as the step `label`.
    pub fn begin(&mut self, project: &Project, label: &str) {
        if self.group.depth > 0 {
            if self.group.before.is_none() {
                self.group.before = Some(project.clone());
                self.group.label = Some(label.to_string());
            }
            return;
        }
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
        if self.group.depth > 0 {
            self.begin(project, label);
            return;
        }
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
        // A group decides at its end by comparing the plan.
        if self.group.depth > 0 {
            return;
        }
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

    /// How many steps undo can go back (cancelled steps do not count).
    pub fn depth(&self) -> usize {
        self.past.iter().flatten().count()
    }

    /// The names of the steps undo goes through, oldest first (Action History).
    pub fn past_labels(&self) -> Vec<String> {
        self.past.iter().flatten().cloned().collect()
    }

    /// The names of the steps redo goes through, next first.
    pub fn future_labels(&self) -> Vec<String> {
        self.future.iter().rev().flatten().cloned().collect()
    }

    /// Renames the newest step (a command that ran several steps and merged
    /// them calls this).
    pub fn relabel_last(&mut self, label: &str) {
        if self.group.depth > 0 && self.group.before.is_some() {
            self.group.label = Some(label.to_string());
            return;
        }
        if let Some(l) = self.past.iter_mut().rev().find(|l| l.is_some()) {
            *l = Some(label.to_string());
        }
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
        self.group = Group::default();
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
    fn a_group_is_one_step_named_after_its_first_change() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        h.begin_group();
        h.begin(&p, "Delete");
        p.name = "b".into();
        h.begin(&p, "Delete Cabinet");
        p.name = "c".into();
        h.begin_merged(&p, "Delete Stair");
        p.name = "d".into();
        assert!(h.end_group(&p));
        assert_eq!(h.depth(), 1);
        assert_eq!(h.undo_label(), Some("Delete"));
        assert_eq!(h.undo(&mut p).as_deref(), Some("Delete"));
        assert_eq!(p.name, "a");
        assert!(!h.can_undo());
    }

    #[test]
    fn a_group_that_changed_nothing_leaves_no_step_and_keeps_redo() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        h.begin(&p, "Real");
        p.name = "b".into();
        h.undo(&mut p);
        assert!(h.can_redo());
        h.begin_group();
        h.begin(&p, "Unlock");
        h.cancel();
        assert!(!h.end_group(&p));
        assert_eq!(h.depth(), 0);
        assert!(h.can_redo(), "a no-op must not drop the redo steps");
    }

    #[test]
    fn nested_groups_close_with_the_outermost() {
        let mut h = ChangeHistory::new();
        let mut p = Project::new("a");
        h.begin_group();
        h.begin_group();
        h.begin(&p, "Inner");
        p.name = "b".into();
        assert!(!h.end_group(&p));
        assert_eq!(h.depth(), 0);
        assert!(h.end_group(&p));
        assert_eq!(h.depth(), 1);
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
