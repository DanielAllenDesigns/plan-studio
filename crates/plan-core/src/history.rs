//! Snapshot-based undo/redo.
//!
//! The editor calls [`History::push`] with the project state *before* each
//! mutation. [`History::undo`] / [`History::redo`] swap the current state with
//! the stored snapshots. Whole-project snapshots are simple and robust; the
//! plan data is small enough that this is cheap.

use crate::model::Project;

#[derive(Debug, Clone)]
pub struct History {
    past: Vec<Project>,
    future: Vec<Project>,
    cap: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(100)
    }
}

impl History {
    /// `cap` is the maximum number of undo steps kept (minimum 1).
    pub fn new(cap: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            cap: cap.max(1),
        }
    }

    /// Record the state before a change. Clears the redo stack.
    pub fn push(&mut self, snapshot_before: &Project) {
        self.past.push(snapshot_before.clone());
        self.future.clear();
        if self.past.len() > self.cap {
            let excess = self.past.len() - self.cap;
            self.past.drain(..excess);
        }
    }

    /// Step back. `current` is stored for redo; returns the state to restore.
    pub fn undo(&mut self, current: &Project) -> Option<Project> {
        let prev = self.past.pop()?;
        self.future.push(current.clone());
        Some(prev)
    }

    /// Step forward. `current` is stored for undo; returns the state to restore.
    pub fn redo(&mut self, current: &Project) -> Option<Project> {
        let next = self.future.pop()?;
        self.past.push(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(n: &str) -> Project {
        Project::new(n)
    }

    #[test]
    fn push_undo_redo_round_trip() {
        let mut h = History::new(10);
        let a = named("a");
        h.push(&a);
        let b = named("b");
        assert!(h.can_undo() && !h.can_redo());
        let restored = h.undo(&b).unwrap();
        assert_eq!(restored.name, "a");
        assert!(!h.can_undo() && h.can_redo());
        let again = h.redo(&restored).unwrap();
        assert_eq!(again.name, "b");
        assert!(h.can_undo() && !h.can_redo());
        assert!(h.redo(&again).is_none());
    }

    #[test]
    fn cap_trims_oldest() {
        let mut h = History::new(2);
        for n in ["a", "b", "c"] {
            h.push(&named(n));
        }
        let cur = named("d");
        let p1 = h.undo(&cur).unwrap();
        assert_eq!(p1.name, "c");
        let p2 = h.undo(&p1).unwrap();
        assert_eq!(p2.name, "b");
        assert!(h.undo(&p2).is_none());
    }

    #[test]
    fn new_push_clears_redo() {
        let mut h = History::new(5);
        h.push(&named("a"));
        let prev = h.undo(&named("b")).unwrap();
        assert!(h.can_redo());
        h.push(&prev);
        assert!(!h.can_redo());
    }
}
