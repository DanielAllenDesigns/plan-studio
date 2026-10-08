//! Edit > Find/Replace Text: finds and replaces text in the plan's text
//! objects (notes, callouts and labels: the `Text` items of the CAD layer),
//! including their rich-text runs.

use crate::cad::CadItem;
use crate::model::Project;
use crate::text_styles::RichRun;
use crate::Id;

/// What to look for and what replaces it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextSearch {
    pub find: String,
    pub replace: String,
    pub match_case: bool,
    /// Only whole words match (neighbours are not letters, digits or `_`).
    pub whole_word: bool,
}

/// A text object that contains the search text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMatch {
    pub floor: usize,
    pub id: Id,
    pub text: String,
    /// How many times the search text occurs in it.
    pub count: usize,
}

fn same_char(a: char, b: char, match_case: bool) -> bool {
    a == b || (!match_case && a.to_lowercase().eq(b.to_lowercase()))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Start indices (in chars) of the matches of `q` in `chars`, left to right
/// and not overlapping.
fn match_starts(chars: &[char], q: &TextSearch) -> Vec<usize> {
    let needle: Vec<char> = q.find.chars().collect();
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > chars.len() {
        return out;
    }
    let mut i = 0;
    while i + needle.len() <= chars.len() {
        let hit = needle
            .iter()
            .enumerate()
            .all(|(k, n)| same_char(chars[i + k], *n, q.match_case));
        let bounded = !q.whole_word
            || ((i == 0 || !is_word(chars[i - 1]))
                && (i + needle.len() == chars.len() || !is_word(chars[i + needle.len()])));
        if hit && bounded {
            out.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// How many times `q.find` occurs in `text`.
pub fn count_matches(text: &str, q: &TextSearch) -> usize {
    match_starts(&text.chars().collect::<Vec<_>>(), q).len()
}

/// `text` with every match of `q.find` replaced by `q.replace`, and how many
/// were replaced.
pub fn replace_in(text: &str, q: &TextSearch) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let starts = match_starts(&chars, q);
    if starts.is_empty() {
        return (text.to_string(), 0);
    }
    let len = q.find.chars().count();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for &s in &starts {
        out.extend(&chars[at..s]);
        out.push_str(&q.replace);
        at = s + len;
    }
    out.extend(&chars[at..]);
    (out, starts.len())
}

/// The runs after replacing in each one, when no match spans two runs;
/// otherwise one run with the first run's style and the replaced text.
fn replace_in_runs(runs: &[RichRun], new_text: &str, q: &TextSearch) -> Vec<RichRun> {
    let per_run: Vec<RichRun> = runs
        .iter()
        .map(|r| RichRun {
            text: replace_in(&r.text, q).0,
            ..r.clone()
        })
        .collect();
    let joined: String = per_run.iter().map(|r| r.text.as_str()).collect();
    if joined == new_text {
        per_run
    } else {
        vec![RichRun {
            text: new_text.to_string(),
            ..runs.first().cloned().unwrap_or_default()
        }]
    }
}

impl Project {
    fn floors_in_scope(&self, all_floors: bool, floor: usize) -> std::ops::Range<usize> {
        if all_floors {
            0..self.floors.len()
        } else {
            floor..(floor + 1).min(self.floors.len())
        }
    }

    /// The text objects containing `q.find`, on every floor or on `floor`.
    pub fn find_text(&self, q: &TextSearch, all_floors: bool, floor: usize) -> Vec<TextMatch> {
        let mut out = Vec::new();
        for fi in self.floors_in_scope(all_floors, floor) {
            for c in &self.floors[fi].cad {
                if let CadItem::Text { text, .. } = &c.item {
                    let count = count_matches(text, q);
                    if count > 0 {
                        out.push(TextMatch {
                            floor: fi,
                            id: c.id,
                            text: text.clone(),
                            count,
                        });
                    }
                }
            }
        }
        out
    }

    /// Replaces `q.find` by `q.replace` in every text object in scope.
    /// Returns `(objects changed, occurrences replaced)`. An empty search
    /// text changes nothing.
    pub fn replace_text(
        &mut self,
        q: &TextSearch,
        all_floors: bool,
        floor: usize,
    ) -> (usize, usize) {
        if q.find.is_empty() {
            return (0, 0);
        }
        let (mut objects, mut occurrences) = (0, 0);
        for fi in self.floors_in_scope(all_floors, floor) {
            let mut changed: Vec<(Id, String)> = Vec::new();
            for c in &mut self.floors[fi].cad {
                if let CadItem::Text { text, .. } = &mut c.item {
                    let (new, n) = replace_in(text, q);
                    if n > 0 {
                        *text = new.clone();
                        changed.push((c.id, new));
                        occurrences += n;
                    }
                }
            }
            objects += changed.len();
            for (id, new_text) in changed {
                let runs = self.floors[fi]
                    .cad_attrs(id)
                    .map(|a| a.runs)
                    .filter(|r| !r.is_empty());
                if let Some(runs) = runs {
                    let replaced = replace_in_runs(&runs, &new_text, q);
                    self.edit_cad_attrs(fi, id, |a| a.runs = replaced);
                }
            }
        }
        (objects, occurrences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::CadItem;
    use crate::geometry::Point;

    fn q(find: &str, replace: &str) -> TextSearch {
        TextSearch {
            find: find.into(),
            replace: replace.into(),
            ..TextSearch::default()
        }
    }

    fn text(p: &mut Project, floor: usize, s: &str) -> Id {
        p.add_cad(
            floor,
            "CAD, Default",
            CadItem::Text {
                pos: Point::ZERO,
                text: s.into(),
                height: 3.0,
                angle: 0.0,
            },
        )
    }

    #[test]
    fn replace_in_honours_case_and_whole_words() {
        let s = "Kitchen kitchenette KITCHEN";
        assert_eq!(
            replace_in(s, &q("kitchen", "Den")),
            ("Den Denette Den".into(), 3)
        );
        let mut cs = q("kitchen", "Den");
        cs.match_case = true;
        assert_eq!(replace_in(s, &cs), ("Kitchen Denette KITCHEN".into(), 1));
        let mut ww = q("kitchen", "Den");
        ww.whole_word = true;
        assert_eq!(replace_in(s, &ww), ("Den kitchenette Den".into(), 2));
        // Non-overlapping, left to right; the replacement is not searched again.
        assert_eq!(replace_in("aaa", &q("aa", "a")), ("aa".into(), 1));
        assert_eq!(replace_in("abc", &q("", "x")), ("abc".into(), 0));
        assert_eq!(count_matches("x x x", &q("x", "")), 3);
        // Multi-byte text keeps its characters.
        assert_eq!(
            replace_in("Küche Küche", &q("küche", "Den")),
            ("Den Den".into(), 2)
        );
    }

    #[test]
    fn find_and_replace_text_objects_on_one_floor_or_all() {
        let mut p = Project::new("t");
        p.floors.push(crate::model::Floor::new("2nd", 109.0));
        let a = text(&mut p, 0, "Master Bath");
        let _b = text(&mut p, 0, "Garage");
        let c = text(&mut p, 1, "Bath 2");
        let hits = p.find_text(&q("bath", ""), true, 0);
        assert_eq!(hits.iter().map(|m| m.id).collect::<Vec<_>>(), vec![a, c]);
        assert_eq!(p.find_text(&q("bath", ""), false, 0).len(), 1);
        assert_eq!(p.replace_text(&q("Bath", "Suite"), false, 0), (1, 1));
        assert!(
            matches!(&p.floors[0].cad[0].item, CadItem::Text { text, .. } if text == "Master Suite")
        );
        // The other floor is untouched until asked.
        assert!(matches!(&p.floors[1].cad[0].item, CadItem::Text { text, .. } if text == "Bath 2"));
        assert_eq!(p.replace_text(&q("Bath", "Suite"), true, 0), (1, 1));
        assert_eq!(p.replace_text(&q("", "x"), true, 0), (0, 0));
    }

    #[test]
    fn rich_text_runs_follow_the_replacement() {
        let mut p = Project::new("t");
        let id = text(&mut p, 0, "Main Bath");
        p.edit_cad_attrs(0, id, |a| {
            a.runs = vec![RichRun::bold("Main "), RichRun::plain("Bath")];
        });
        p.replace_text(&q("Bath", "Suite"), false, 0);
        let runs = p.floors[0].cad_attrs(id).unwrap().runs;
        assert_eq!(runs.len(), 2);
        assert!(runs[0].bold && runs[0].text == "Main ");
        assert_eq!(runs[1].text, "Suite");
        // A match that spans runs collapses them into one with the first style.
        p.replace_text(&q("n Suite", "n Bath"), false, 0);
        let runs = p.floors[0].cad_attrs(id).unwrap().runs;
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "Main Bath");
        assert!(runs[0].bold);
    }
}
