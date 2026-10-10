//! Spell checking (TXT-21): the word list, the tokenizer, suggestions and the
//! rich text helpers behind Tools > Spell Check and the Check Spelling button
//! of the text dialog (`dialogs::spell_check`).
//!
//! The word list is read at run time from the operating system
//! (`/usr/share/dict/words` and friends on macOS and Linux; Windows has none,
//! so it relies on the user dictionary and the built-in design vocabulary).
//! Nothing is bundled. The user dictionary is `~/.plan-studio/dictionary.txt`,
//! one word per line; Add appends to it.
//!
//! The big system lists (Webster's 2nd on macOS) hold base forms only, so a
//! word is also accepted when stripping a common ending (plural, -ed, -ing,
//! -er, -ly, ...) or a common prefix leaves a listed word.
//!
//! Suggestions are the listed words within an edit distance of two
//! (Damerau-Levenshtein, adjacent swaps count as one), nearest first.

use plan_core::text_styles::RichRun;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// Largest edit distance a suggestion may have from the misspelling.
pub const MAX_EDIT: usize = 2;
/// How many suggestions the dialog lists.
pub const MAX_SUGGESTIONS: usize = 8;
/// Words longer than this are not given suggestions (the distance buffers are
/// fixed size).
const MAX_WORD: usize = 32;

/// Words an architect writes that the system list lacks: construction and
/// design terms, common plan abbreviations and contractions.
const BUILTIN: &str = "\
abbr alcove ampere amps ansi arch architectural awning backsplash baluster balusters balustrade \
batten battens bldg bifold bullnose bypass cabinetry cantilever cantilevered casing casings \
cfm clerestory clg cmu colonial corbel corbels cornice countertop countertops crawlspace \
crawlspaces cripple culdesac dbl dia dimensioned dishwasher doorway doorways dormer dormers \
downspout downspouts drywall egress elev ensuite entryway entryways eave eaves facia fascia \
fenestration fieldstone fireplace fireplaces flashing footing footings foyer framing furring \
gable gables gfci afci glazing gutter gutters gypsum header headers hvac insulated jamb jambs \
joist joists kitchenette lintel lintels loggia mantel mantels masonry millwork mudroom mudrooms \
mullion mullions muntin muntins newel nosing ogee oriented ovens pantry parapet pergola \
pergolas plenum plumbing pocket powder purlin purlins quoin quoins rafter rafters rebar \
recessed rec reinforced residential rowlock sconce sconces sheathing shiplap shingles \
sidelight sidelights sill sills skylight skylights soffit soffits stairwell stairwells stucco \
studs subfloor subfloors thermostat threshold thresholds tongue trusses truss unfinished \
vanity vaulted veneer vestibule wainscot wainscoting wic wardrobe waterproofing weatherstrip \
windowsill wiring typ min max sim opng rm br ba ft sf lf oc nts tbd tbc tbd ie eg vs etc \
don't doesn't didn't isn't aren't wasn't weren't can't couldn't won't wouldn't shouldn't \
haven't hasn't hadn't it's that's there's here's what's let's we'll you'll they'll i'll \
i'm i've you're we're they're";

// ----- the word list -----

/// A set of lowercase words, also indexed by length for suggestions.
#[derive(Default)]
pub struct WordList {
    words: HashSet<String>,
    by_len: HashMap<usize, Vec<String>>,
}

/// Does the line look like one word of a dictionary? Letters, with inner
/// apostrophes.
fn plausible_word(w: &str) -> bool {
    !w.is_empty()
        && w.chars().all(|c| c.is_alphabetic() || c == '\'')
        && w.chars().any(|c| c.is_alphabetic())
}

impl WordList {
    /// A list of `words` (lowercased; lines that are not a word are dropped).
    pub fn from_words<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let words: HashSet<String> = words
            .into_iter()
            .map(|w| w.as_ref().trim().to_lowercase())
            .filter(|w| plausible_word(w))
            .collect();
        let mut by_len: HashMap<usize, Vec<String>> = HashMap::new();
        for w in &words {
            by_len.entry(w.chars().count()).or_default().push(w.clone());
        }
        Self { words, by_len }
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn contains(&self, lower: &str) -> bool {
        self.words.contains(lower)
    }
}

/// Where the operating system keeps a word list.
#[cfg_attr(test, allow(dead_code))]
pub fn system_word_files() -> Vec<PathBuf> {
    [
        "/usr/share/dict/words",
        "/usr/share/dict/american-english",
        "/usr/share/dict/british-english",
        "/usr/dict/words",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

/// The lines of a word file (lossy UTF-8; an unreadable file is empty).
pub fn read_word_file(path: &Path) -> Vec<String> {
    std::fs::read(path)
        .map(|b| {
            String::from_utf8_lossy(&b)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The first system word list that exists.
#[cfg_attr(test, allow(dead_code))]
pub fn load_system_list() -> WordList {
    system_word_files()
        .iter()
        .find(|p| p.is_file())
        .map(|p| WordList::from_words(read_word_file(p)))
        .unwrap_or_default()
}

#[cfg_attr(test, allow(dead_code))]
static SYSTEM_LIST: OnceLock<Arc<WordList>> = OnceLock::new();
#[cfg_attr(test, allow(dead_code))]
static LOADING: AtomicBool = AtomicBool::new(false);

/// Starts reading the system list on another thread (once).
#[cfg_attr(test, allow(dead_code))]
pub fn start_loading() {
    if SYSTEM_LIST.get().is_none() && !LOADING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(|| {
            let _ = SYSTEM_LIST.set(Arc::new(load_system_list()));
        });
    }
}

/// The system list once it has been read.
#[cfg_attr(test, allow(dead_code))]
pub fn system_list_ready() -> Option<Arc<WordList>> {
    SYSTEM_LIST.get().cloned()
}

// ----- the user dictionary -----

#[cfg(test)]
thread_local! {
    static TEST_CUSTOM: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// `~/.plan-studio/dictionary.txt`. Tests have no path unless one is set.
pub fn custom_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        TEST_CUSTOM.with(|p| p.borrow().clone())
    }
    #[cfg(not(test))]
    {
        crate::paths::user_file("dictionary.txt")
    }
}

/// Appends `word` to the user dictionary file at `path`.
pub fn append_word(path: &Path, word: &str) -> Result<(), String> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{word}").map_err(|e| e.to_string())
}

// ----- the speller -----

/// One misspelled word: its byte range in the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misspelling {
    pub start: usize,
    pub end: usize,
    pub word: String,
}

pub struct Speller {
    list: Arc<WordList>,
    builtin: HashSet<String>,
    custom: HashSet<String>,
    /// Ignore All: words skipped until the check is started again.
    ignored: HashSet<String>,
    /// Skip short words written in capitals (acronyms: HVAC, GFCI).
    pub ignore_upper: bool,
    /// Skip words with digits (2x4, 36in).
    pub ignore_numbers: bool,
}

impl Speller {
    /// A speller over `list`, the built-in vocabulary and the words of the
    /// user dictionary at `custom_file`.
    pub fn new(list: Arc<WordList>, custom_file: Option<&Path>) -> Self {
        let custom = custom_file
            .map(|p| {
                read_word_file(p)
                    .into_iter()
                    .map(|w| w.to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            list,
            builtin: BUILTIN.split_whitespace().map(str::to_string).collect(),
            custom,
            ignored: HashSet::new(),
            ignore_upper: true,
            ignore_numbers: true,
        }
    }

    /// How many words the system list holds (0: no list was found).
    pub fn system_words(&self) -> usize {
        self.list.len()
    }

    pub fn custom_words(&self) -> usize {
        self.custom.len()
    }

    fn listed(&self, lower: &str) -> bool {
        self.list.contains(lower)
            || self.builtin.contains(lower)
            || self.custom.contains(lower)
            || self.ignored.contains(lower)
    }

    /// Ignore All: `word` is no longer reported.
    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    /// Starts a fresh check: Ignore All is forgotten.
    pub fn reset_ignored(&mut self) {
        self.ignored.clear();
    }

    /// Add: `word` joins the user dictionary (and its file, when there is
    /// one).
    pub fn add_custom(&mut self, word: &str) -> Result<(), String> {
        let lower = word.to_lowercase();
        if !plausible_word(&lower) {
            return Err(format!("\"{word}\" is not a word"));
        }
        let fresh = self.custom.insert(lower.clone());
        if fresh {
            if let Some(p) = custom_path() {
                append_word(&p, &lower)?;
            }
        }
        Ok(())
    }

    /// Is the word spelled right (or one the check does not judge)?
    pub fn is_correct(&self, word: &str) -> bool {
        let w = word.replace('\u{2019}', "'");
        let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() < 2 {
            return true;
        }
        if self.ignore_numbers && w.chars().any(|c| c.is_ascii_digit()) {
            return true;
        }
        let all_upper = letters.iter().all(|c| c.is_uppercase());
        // Short capitals are acronyms; a long one is checked like any word
        // (plan notes are often written in capitals).
        if all_upper && self.ignore_upper && letters.len() <= 4 {
            return true;
        }
        // McDonald, iPhone, camelCase: not judged.
        if !all_upper && w.chars().skip(1).any(char::is_uppercase) {
            return true;
        }
        let lower = w.to_lowercase();
        self.known_word(&lower)
    }

    fn known_word(&self, lower: &str) -> bool {
        if self.listed(lower) {
            return true;
        }
        // Possessives.
        if let Some(base) = lower
            .strip_suffix("'s")
            .or_else(|| lower.strip_suffix("s'"))
        {
            if !base.is_empty() && self.known_word(base) {
                return true;
            }
        }
        if lower.contains('\'') {
            return false;
        }
        if stems(lower).iter().any(|s| self.listed(s)) {
            return true;
        }
        // One prefix, then the endings again.
        for p in PREFIXES {
            if let Some(rest) = lower.strip_prefix(p) {
                if rest.chars().count() >= 3
                    && (self.listed(rest) || stems(rest).iter().any(|s| self.listed(s)))
                {
                    return true;
                }
            }
        }
        false
    }

    /// The misspelled words of `text`. With `markup`, `<b>`-style tags are
    /// skipped (the rich text of the text dialog).
    pub fn misspellings(&self, text: &str, markup: bool) -> Vec<Misspelling> {
        tokens(text, markup)
            .into_iter()
            .filter_map(|(s, e)| {
                let w = &text[s..e];
                (!self.is_correct(w)).then(|| Misspelling {
                    start: s,
                    end: e,
                    word: w.to_string(),
                })
            })
            .collect()
    }

    /// Listed words within [`MAX_EDIT`] of `word`, nearest first, spelled in
    /// the case of `word`.
    pub fn suggest(&self, word: &str, limit: usize) -> Vec<String> {
        let lower = word.replace('\u{2019}', "'").to_lowercase();
        let target: Vec<char> = lower.chars().collect();
        if target.len() < 2 || target.len() > MAX_WORD {
            return Vec::new();
        }
        let mut found: Vec<(usize, String)> = Vec::new();
        self.collect_near(&target, &mut found);
        // "walss" -> "walls": a near stem plus the ending the word has.
        for suffix in ["s", "es", "ed", "ing", "ly", "er"] {
            if let Some(stem) = lower.strip_suffix(suffix) {
                let stem_chars: Vec<char> = stem.chars().collect();
                if stem_chars.len() < 2 {
                    continue;
                }
                let mut near: Vec<(usize, String)> = Vec::new();
                self.collect_near(&stem_chars, &mut near);
                for (_, s) in near.into_iter().filter(|(d, _)| *d <= 1) {
                    let cand = format!("{s}{suffix}");
                    let cc: Vec<char> = cand.chars().collect();
                    if let Some(d) = bounded_distance(&target, &cc, MAX_EDIT) {
                        if self.known_word(&cand) {
                            found.push((d, cand));
                        }
                    }
                }
            }
        }
        let first = target[0];
        found.sort_by(|a, b| {
            let fa = !a.1.starts_with(first);
            let fb = !b.1.starts_with(first);
            a.0.cmp(&b.0)
                .then(fa.cmp(&fb))
                .then(
                    a.1.chars()
                        .count()
                        .abs_diff(target.len())
                        .cmp(&b.1.chars().count().abs_diff(target.len())),
                )
                .then(a.1.cmp(&b.1))
        });
        let mut seen = HashSet::new();
        found
            .into_iter()
            .filter(|(_, w)| w != &lower && seen.insert(w.clone()))
            .map(|(_, w)| match_case(word, &w))
            .take(limit)
            .collect()
    }

    fn collect_near(&self, target: &[char], out: &mut Vec<(usize, String)>) {
        let n = target.len();
        let lo = n.saturating_sub(MAX_EDIT).max(1);
        let mut check = |w: &String| {
            let cs: Vec<char> = w.chars().collect();
            if let Some(d) = bounded_distance(target, &cs, MAX_EDIT) {
                out.push((d, w.clone()));
            }
        };
        for len in lo..=n + MAX_EDIT {
            if let Some(bucket) = self.list.by_len.get(&len) {
                bucket.iter().for_each(&mut check);
            }
        }
        self.builtin
            .iter()
            .chain(self.custom.iter())
            .filter(|w| w.chars().count().abs_diff(n) <= MAX_EDIT)
            .for_each(&mut check);
    }
}

const PREFIXES: [&str; 18] = [
    "un", "re", "non", "pre", "over", "under", "semi", "multi", "anti", "inter", "sub", "out",
    "mis", "dis", "post", "self", "cross", "half",
];

/// The base forms `lower` could be an inflection of (plural, past, -ing,
/// comparative, -ly, ...). Callers check whether any is a listed word.
fn stems(lower: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        if s.chars().count() >= 3 {
            out.push(s.to_string());
        }
    };
    // Ending, then what to append to the rest to get the base form.
    let cut: [(&str, &[&str]); 21] = [
        ("ies", &["y"]),
        ("ves", &["f", "fe"]),
        ("es", &[""]),
        ("s", &[""]),
        ("ied", &["y"]),
        ("ed", &["", "e"]),
        ("ing", &["", "e"]),
        ("ier", &["y"]),
        ("iest", &["y"]),
        ("ers", &["", "e"]),
        ("er", &["", "e"]),
        ("est", &["", "e"]),
        ("ily", &["y"]),
        ("ally", &[""]),
        ("ly", &[""]),
        ("ness", &[""]),
        ("ment", &[""]),
        ("ments", &[""]),
        ("less", &[""]),
        ("ful", &[""]),
        ("ings", &["", "e"]),
    ];
    for (ending, appends) in cut {
        if let Some(rest) = lower.strip_suffix(ending) {
            for a in appends {
                push(&format!("{rest}{a}"));
            }
            // stopped -> stop, running -> run
            let mut chars = rest.chars().rev();
            if let (Some(a), Some(b)) = (chars.next(), chars.next()) {
                if a == b && a.is_alphabetic() {
                    push(&rest[..rest.len() - a.len_utf8()]);
                }
            }
        }
    }
    out
}

/// Byte ranges of the words of `text`. Words are runs of letters and digits
/// with inner apostrophes. Addresses (`www.`, `://`, `@`) and, with
/// `markup`, `<b>`-style tags are skipped.
pub fn tokens(text: &str, markup: bool) -> Vec<(usize, usize)> {
    // Chunks (between white space) that are web or mail addresses.
    let mut skip: Vec<(usize, usize)> = Vec::new();
    let mut at = 0;
    for chunk in text.split_inclusive(char::is_whitespace) {
        let c = chunk.trim_end();
        if c.contains("://") || c.contains('@') || c.starts_with("www.") {
            skip.push((at, at + c.len()));
        }
        at += chunk.len();
    }
    let in_skip = |s: usize| skip.iter().any(|(a, b)| s >= *a && s < *b);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let word_char = |c: char| c.is_alphanumeric() || c == '_';
    let apostrophe = |c: char| c == '\'' || c == '\u{2019}';
    while i < chars.len() {
        let (pos, c) = chars[i];
        if markup && c == '<' {
            // A tag: no white space up to the closing bracket.
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != '>' && !chars[j].1.is_whitespace() && j - i < 40
            {
                j += 1;
            }
            if j < chars.len() && chars[j].1 == '>' {
                i = j + 1;
                continue;
            }
        }
        if word_char(c) {
            let mut j = i + 1;
            while j < chars.len() {
                let cj = chars[j].1;
                if word_char(cj)
                    || (apostrophe(cj) && j + 1 < chars.len() && word_char(chars[j + 1].1))
                {
                    j += 1;
                } else {
                    break;
                }
            }
            let end = chars.get(j).map_or(text.len(), |(p, _)| *p);
            if !in_skip(pos) {
                out.push((pos, end));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

// ----- edit distance -----

/// Damerau-Levenshtein distance (optimal string alignment) of `a` and `b` when
/// it is at most `max`.
pub fn bounded_distance(a: &[char], b: &[char], max: usize) -> Option<usize> {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max || n > MAX_WORD || m > MAX_WORD {
        return None;
    }
    // Rows i-2, i-1 and i of the table.
    let mut prev2 = [0usize; MAX_WORD + 1];
    let mut prev = [0usize; MAX_WORD + 1];
    let mut cur = [0usize; MAX_WORD + 1];
    for (j, p) in prev.iter_mut().enumerate().take(m + 1) {
        *p = j;
    }
    for i in 1..=n {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut d = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d = d.min(prev2[j - 2] + 1);
            }
            cur[j] = d;
            row_min = row_min.min(d);
        }
        if row_min > max {
            return None;
        }
        prev2 = prev;
        prev = cur;
    }
    (prev[m] <= max).then_some(prev[m])
}

/// `candidate` written in the case of `original`: ALL CAPS stays capitals, a
/// capitalized word stays capitalized.
pub fn match_case(original: &str, candidate: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return candidate.to_uppercase();
    }
    if original.chars().next().is_some_and(char::is_uppercase) {
        let mut cs = candidate.chars();
        return cs
            .next()
            .map(|f| f.to_uppercase().collect::<String>() + cs.as_str())
            .unwrap_or_default();
    }
    candidate.to_string()
}

/// The replacement for a word found at another place (Change All): the
/// replacement as typed when it has capitals, else in the case of the word
/// it replaces.
pub fn adapt_case(found: &str, replacement: &str) -> String {
    if replacement.chars().any(char::is_uppercase) {
        replacement.to_string()
    } else {
        match_case(found, replacement)
    }
}

// ----- editing text -----

/// `text` with the bytes `start..end` replaced by `new`.
pub fn splice(text: &str, start: usize, end: usize, new: &str) -> String {
    let mut s = String::with_capacity(text.len() + new.len());
    s.push_str(&text[..start]);
    s.push_str(new);
    s.push_str(&text[end..]);
    s
}

/// The rich text runs after the bytes `start..end` of their joined text were
/// replaced by `new`. The new text takes the format of the run the range
/// starts in.
pub fn replace_range_in_runs(
    runs: &[RichRun],
    start: usize,
    end: usize,
    new: &str,
) -> Vec<RichRun> {
    let mut out: Vec<RichRun> = Vec::with_capacity(runs.len());
    let mut at = 0;
    let mut done = false;
    for r in runs {
        let (rs, re) = (at, at + r.text.len());
        at = re;
        if re <= start || rs >= end {
            out.push(r.clone());
            continue;
        }
        let head = &r.text[..start.saturating_sub(rs)];
        let tail = &r.text[end.saturating_sub(rs).min(r.text.len())..];
        let mut run = r.clone();
        run.text = if done {
            tail.to_string()
        } else {
            done = true;
            format!("{head}{new}{tail}")
        };
        if !run.text.is_empty() {
            out.push(run);
        }
    }
    if !done {
        out.push(RichRun::plain(new));
    }
    out
}

// ----- the shared speller -----

thread_local! {
    static SPELLER: RefCell<Option<Speller>> = const { RefCell::new(None) };
}

fn fresh_speller() -> Option<Speller> {
    #[cfg(test)]
    let list = Some(Arc::new(WordList::default()));
    #[cfg(not(test))]
    let list = {
        start_loading();
        system_list_ready()
    };
    list.map(|l| Speller::new(l, custom_path().as_deref()))
}

/// Runs `f` on the application's speller, or returns `None` while the system
/// word list is still being read (the text dialog underlines nothing until
/// then).
pub fn with_ready<R>(f: impl FnOnce(&mut Speller) -> R) -> Option<R> {
    SPELLER.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_none() {
            *s = fresh_speller();
        }
        s.as_mut().map(f)
    })
}

/// Like [`with_ready`] but waits for the word list.
pub fn with<R>(f: impl FnOnce(&mut Speller) -> R) -> R {
    let mut f = Some(f);
    loop {
        if let Some(r) = with_ready(|s| (f.take().expect("called once"))(s)) {
            return r;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Makes `speller` the application's speller (tests, with a synthetic word
/// list), and `custom_file` where Add writes.
#[cfg(test)]
pub fn install_for_test(speller: Speller, custom_file: Option<PathBuf>) {
    TEST_CUSTOM.with(|p| *p.borrow_mut() = custom_file);
    SPELLER.with(|s| *s.borrow_mut() = Some(speller));
}

/// A speller over `words` and nothing else but the built-in vocabulary.
#[cfg(test)]
pub fn test_speller(words: &[&str]) -> Speller {
    Speller::new(Arc::new(WordList::from_words(words.iter())), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speller() -> Speller {
        test_speller(&[
            "the", "kitchen", "wall", "room", "master", "bedroom", "bath", "bathroom", "frame",
            "stop", "run", "close", "happy", "door", "window", "shelf", "knife", "city", "carry",
            "house", "measure", "walk", "closet", "is", "in", "see", "or",
        ])
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn flags_unlisted_words_and_accepts_listed_ones_in_any_case() {
        let sp = speller();
        let found: Vec<String> = sp
            .misspellings("The Kitchn wall is in the MASTER bedrom", false)
            .into_iter()
            .map(|m| m.word)
            .collect();
        assert_eq!(found, vec!["Kitchn", "bedrom"]);
        assert!(sp.is_correct("KITCHEN"));
        assert!(sp.misspellings("kitchen", false).is_empty());
    }

    #[test]
    fn misspelling_ranges_are_byte_ranges_of_the_word() {
        let sp = speller();
        let text = "caf\u{e9} kitchn here";
        let m = sp.misspellings(text, false);
        let kitchn = m.iter().find(|m| m.word == "kitchn").unwrap();
        assert_eq!(&text[kitchn.start..kitchn.end], "kitchn");
    }

    #[test]
    fn endings_and_prefixes_of_listed_words_are_accepted() {
        let sp = speller();
        for w in [
            "walls",
            "doors",
            "windows",
            "shelves",
            "knives",
            "cities",
            "carried",
            "framed",
            "framing",
            "stopped",
            "running",
            "closed",
            "happier",
            "happily",
            "bathrooms",
            "unframed",
            "rewalk",
            "measurement",
            "closets",
        ] {
            assert!(sp.is_correct(w), "{w}");
        }
        for w in ["wallx", "dorrs", "frameding"] {
            assert!(!sp.is_correct(w), "{w}");
        }
        // Possessives follow their word.
        assert!(sp.is_correct("kitchen's"));
        assert!(sp.is_correct("walls'"));
    }

    #[test]
    fn numbers_acronyms_internal_capitals_and_single_letters_are_not_judged() {
        let sp = speller();
        for w in [
            "2x4", "36in", "HVAC", "GFCI", "a", "x", "McDonald", "iPhone",
        ] {
            assert!(sp.is_correct(w), "{w}");
        }
        // A long word in capitals is checked: plan notes are often written so.
        assert!(!sp.is_correct("KITCHN"));
        assert!(sp.is_correct("BATHROOM"));
        // The built-in design vocabulary and contractions.
        for w in ["soffit", "wainscot", "joists", "don't", "can't"] {
            assert!(sp.is_correct(w), "{w}");
        }
    }

    #[test]
    fn tokens_skip_markup_tags_and_addresses() {
        let sp = speller();
        let text = "<b>kitchn</b> <size=1.5>room</size> see www.exmple.com or a@exmple.com";
        let words: Vec<&str> = sp
            .misspellings(text, true)
            .into_iter()
            .map(|m| &text[m.start..m.end])
            .collect();
        assert_eq!(words, vec!["kitchn"]);
        // Without markup mode the tag names are words too.
        let plain: Vec<String> = sp
            .misspellings("<size=1.5>room</size>", false)
            .into_iter()
            .map(|m| m.word)
            .collect();
        assert_eq!(plain, vec!["size", "size"]);
        // Inner apostrophes belong to the word; the curly one too.
        let t = tokens("don\u{2019}t stop 'quoted'", false);
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn edit_distance_counts_swaps_as_one_and_stops_at_the_limit() {
        let d = |a: &str, b: &str, max| bounded_distance(&chars(a), &chars(b), max);
        assert_eq!(d("kitchen", "kitchen", 2), Some(0));
        assert_eq!(d("kitchn", "kitchen", 2), Some(1));
        assert_eq!(d("kicthen", "kitchen", 2), Some(1));
        assert_eq!(d("bedrom", "bedroom", 2), Some(1));
        assert_eq!(d("wall", "wail", 2), Some(1));
        assert_eq!(d("wall", "wxyz", 2), None);
        assert_eq!(d("abc", "abcdef", 2), None);
        assert_eq!(d("abcd", "abxy", 2), Some(2));
    }

    #[test]
    fn suggestions_are_within_two_edits_nearest_first_and_keep_the_case() {
        let sp = speller();
        let s = sp.suggest("kitchn", 8);
        assert_eq!(s.first().map(String::as_str), Some("kitchen"));
        assert!(s.iter().all(|w| {
            bounded_distance(&chars("kitchn"), &chars(&w.to_lowercase()), MAX_EDIT).is_some()
        }));
        assert_eq!(sp.suggest("Kitchn", 1), vec!["Kitchen"]);
        assert_eq!(sp.suggest("KITCHN", 1), vec!["KITCHEN"]);
        // Nearest first: one edit before two.
        let near = sp.suggest("wll", 8);
        assert_eq!(near.first().map(String::as_str), Some("wall"));
        // A swap is one edit.
        assert_eq!(sp.suggest("rooom", 1), vec!["room"]);
        // Nothing within two edits.
        assert!(sp.suggest("zzzzzzzz", 8).is_empty());
        // The limit holds.
        assert!(sp.suggest("bat", 2).len() <= 2);
    }

    #[test]
    fn suggestions_can_end_like_the_misspelling() {
        let sp = speller();
        assert!(sp.suggest("walss", 8).contains(&"walls".to_string()));
        assert!(sp.suggest("framin", 8).contains(&"framing".to_string()));
    }

    #[test]
    fn ignore_all_hides_a_word_until_reset() {
        let mut sp = speller();
        assert!(!sp.is_correct("Zorb"));
        sp.ignore("zorb");
        assert!(sp.is_correct("ZORB") && sp.is_correct("zorb"));
        sp.reset_ignored();
        assert!(!sp.is_correct("zorb"));
    }

    #[test]
    fn add_writes_the_user_dictionary_and_the_next_speller_reads_it() {
        let dir = std::env::temp_dir().join(format!("plan-studio-spell-{}", std::process::id()));
        let file = dir.join("dictionary.txt");
        let _ = std::fs::remove_dir_all(&dir);
        install_for_test(speller(), Some(file.clone()));
        with(|s| {
            assert!(!s.is_correct("zorbify"));
            s.add_custom("Zorbify").unwrap();
            assert!(s.is_correct("zorbify"));
            assert!(s.add_custom("not a word!").is_err());
        });
        assert_eq!(read_word_file(&file), vec!["zorbify"]);
        let again = Speller::new(Arc::new(WordList::default()), Some(&file));
        assert!(again.is_correct("Zorbify"));
        assert_eq!(again.custom_words(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn word_lists_keep_only_words() {
        let l = WordList::from_words(["Apple", "it's", "x-ray", "", "3d", "pear "]);
        assert_eq!(l.len(), 3);
        assert!(l.contains("apple") && l.contains("pear") && l.contains("it's"));
    }

    #[test]
    fn case_matching_for_suggestions_and_change_all() {
        assert_eq!(match_case("Kitchn", "kitchen"), "Kitchen");
        assert_eq!(match_case("KITCHN", "kitchen"), "KITCHEN");
        assert_eq!(match_case("kitchn", "kitchen"), "kitchen");
        assert_eq!(adapt_case("KITCHN", "kitchen"), "KITCHEN");
        assert_eq!(adapt_case("kitchn", "Kitchen"), "Kitchen");
    }

    #[test]
    fn replacing_in_rich_runs_keeps_the_format_of_the_run_it_starts_in() {
        let bold = RichRun {
            bold: true,
            ..RichRun::plain("big kitchn")
        };
        let runs = vec![RichRun::plain("a "), bold, RichRun::plain(" here")];
        // "kitchn" is bytes 6..12 of "a big kitchn here".
        let out = replace_range_in_runs(&runs, 6, 12, "kitchen");
        let joined: String = out.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(joined, "a big kitchen here");
        assert!(out[1].bold && out[1].text == "big kitchen");
        // A word that spans two runs ends up in the first one.
        let two = vec![RichRun::plain("kit"), RichRun::plain("chn ok")];
        let out = replace_range_in_runs(&two, 0, 6, "kitchen");
        let joined: String = out.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(joined, "kitchen ok");
        assert_eq!(out.len(), 2);
        assert_eq!(splice("a kitchn b", 2, 8, "kitchen"), "a kitchen b");
    }
}
