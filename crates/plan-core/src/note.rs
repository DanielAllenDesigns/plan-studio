//! Notes, Note Types and Note Schedules (TXT-54, TXT-55; manual pp. 559 to
//! 563), and the creation and removal of callout, marker and note records.
//!
//! A note is a [`Note`] record (see [`crate::callout`]); the Note Schedule
//! reads them back with [`Project::note_rows`], numbered per Note Type in
//! draw order, so deleting a note renumbers the ones after it.

use crate::cad::CadItem;
use crate::callout::{
    callout_with_caution, marker_items, note_items, AnnotRef, Callout, Marker, Note, Vars,
};
use crate::geometry::Point;
use crate::model::{Id, Project};
use crate::schedules::{Schedule, ScheduleKind, ScheduleLayer};

/// One line of a Note Schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteRow {
    pub floor: usize,
    /// The note's first CAD object (what a schedule row points at).
    pub id: Id,
    pub pos: Point,
    pub note_type: String,
    pub number: u32,
    /// The schedule text.
    pub text: String,
    /// `"{prefix} {number}"`, e.g. `E 2`.
    pub mark: String,
}

impl Project {
    /// The notes of the plan as schedule rows: ordered by Note Type (in the
    /// order of Note Type Management), then number.
    pub fn note_rows(&self) -> Vec<NoteRow> {
        let numbers = self.note_numbering();
        let types = self.note_types();
        let mut rows: Vec<NoteRow> = Vec::new();
        for (fi, f) in self.floors.iter().enumerate() {
            for (i, n) in f.annots.notes.iter().enumerate() {
                if !n.include_in_schedule {
                    continue;
                }
                let number = numbers.get(fi).and_then(|v| v.get(i)).copied().unwrap_or(0);
                let prefix = types
                    .get(&n.note_type)
                    .or(types.types.first())
                    .map_or("Note", |t| t.prefix.as_str());
                rows.push(NoteRow {
                    floor: fi,
                    id: n.items.first().copied().unwrap_or(0),
                    pos: n.center,
                    note_type: n.note_type.clone(),
                    number,
                    text: n.text.clone(),
                    mark: format!("{prefix} {number}"),
                });
            }
        }
        let order = |name: &str| {
            types
                .types
                .iter()
                .position(|t| t.name == name)
                .unwrap_or(usize::MAX)
        };
        rows.sort_by_key(|r| (order(&r.note_type), r.number, r.floor));
        rows
    }

    /// Adds a callout to floor `fi`; returns the id of its first CAD object.
    pub fn add_callout(&mut self, fi: usize, mut c: Callout) -> Id {
        let v = Vars {
            link: c.link.as_ref().map(|l| self.resolve_view_link(l)),
            ..Vars::default()
        };
        let broken = c.link.is_some() && !v.link.as_ref().is_some_and(|l| l.valid);
        let g = callout_with_caution(&c, &v, broken && !c.ignore_link);
        let layer = c.layer.clone();
        c.items.clear();
        self.write_gen(fi, &mut c.items, &mut c.pose_idx, &mut c.pose, &layer, g);
        let id = c.items[0];
        self.floors[fi].annots.callouts.push(c);
        id
    }

    /// Adds a marker to floor `fi`; returns the id of its first CAD object.
    pub fn add_marker(&mut self, fi: usize, mut m: Marker) -> Id {
        let g = marker_items(&m, &Vars::default());
        let layer = m.layer.clone();
        m.items.clear();
        self.write_gen(fi, &mut m.items, &mut m.pose_idx, &mut m.pose, &layer, g);
        let id = m.items[0];
        self.floors[fi].annots.markers.push(m);
        id
    }

    /// Adds a note to floor `fi`; returns the id of its first CAD object.
    /// The note goes last in draw order, so it is the next of its type.
    pub fn add_note(&mut self, fi: usize, mut n: Note) -> Id {
        let numbers = self.note_numbering();
        let next = self
            .floors
            .iter()
            .flat_map(|f| f.annots.notes.iter())
            .filter(|o| o.note_type == n.note_type)
            .count() as u32
            + 1;
        let _ = numbers;
        let v = Vars {
            number: Some(next),
            ..Vars::default()
        };
        let caution = !self.note_schedule_exists(&n.note_type);
        let g = note_items(&n, &v, caution);
        let layer = n.layer.clone();
        n.items.clear();
        self.write_gen(fi, &mut n.items, &mut n.pose_idx, &mut n.pose, &layer, g);
        let id = n.items[0];
        self.floors[fi].annots.notes.push(n);
        id
    }

    /// Removes the record `r` of floor `fi` and its CAD objects.
    pub fn remove_annot(&mut self, fi: usize, r: AnnotRef) {
        let items: Vec<Id> = self.floors[fi].annot_items(r).to_vec();
        match r {
            AnnotRef::Callout(i) => {
                self.floors[fi].annots.callouts.remove(i);
            }
            AnnotRef::Marker(i) => {
                self.floors[fi].annots.markers.remove(i);
            }
            AnnotRef::Note(i) => {
                self.floors[fi].annots.notes.remove(i);
            }
        }
        for id in items {
            self.remove_cad(fi, id);
        }
        self.prune_cad_data(fi);
    }

    /// Convert Text to Note: the text object `text_id` of floor `fi` becomes
    /// a note of `note_type` (other settings from `base`, the Saved Note
    /// Defaults). Returns the new note's first CAD object.
    pub fn convert_text_to_note(
        &mut self,
        fi: usize,
        text_id: Id,
        note_type: &str,
        base: &Note,
    ) -> Option<Id> {
        let obj = self.floors[fi].cad.iter().find(|c| c.id == text_id)?;
        let CadItem::Text { pos, text, .. } = &obj.item else {
            return None;
        };
        if self.floors[fi].annot_of(text_id).is_some() {
            return None;
        }
        let (pos, text) = (*pos, text.clone());
        let mut n = base.clone();
        n.text = text;
        n.note_type = note_type.to_string();
        n.center = pos;
        self.remove_cad(fi, text_id);
        self.prune_cad_data(fi);
        Some(self.add_note(fi, n))
    }

    /// Create Note Schedule from Note(s): a Note Schedule in floor `fi` that
    /// lists the Note Types of `notes` (all types if they differ in number).
    /// Returns the schedule's id.
    pub fn create_note_schedule(&mut self, fi: usize, types: &[String], at: Point) -> Option<Id> {
        let f = self.floors.get_mut(fi)?;
        let mut layer = ScheduleLayer::load(f);
        let mut s = Schedule::new(ScheduleKind::Note, at);
        if let [one] = types {
            s.title = format!("{one} Schedule");
            s.filter = one.clone();
        }
        let id = layer.add(s);
        layer.store(f);
        Some(id)
    }

    /// Ignore Note With No Schedule: drops the Caution symbol of the given
    /// notes (all notes of floor `fi` when `only` is empty).
    pub fn ignore_notes_without_schedule(&mut self, fi: usize, only: &[usize]) -> usize {
        let mut n = 0;
        for (i, note) in self.floors[fi].annots.notes.iter_mut().enumerate() {
            if (only.is_empty() || only.contains(&i)) && !note.ignore_no_schedule {
                note.ignore_no_schedule = true;
                n += 1;
            }
        }
        n
    }
}
