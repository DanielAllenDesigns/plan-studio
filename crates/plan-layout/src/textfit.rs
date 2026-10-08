//! Text in a text box: wrapping at the box width, clipping what overflows,
//! and shrinking the type until it fits.

use crate::canvas::text_w;
use crate::extent::LINE_SPACING;
use serde::{Deserialize, Serialize};

/// Side padding of the text inside its box, points.
pub(crate) const PAD_PT: f64 = 3.0;
/// The smallest type shrink-to-fit goes down to, points.
pub const MIN_SHRINK_PT: f64 = 4.0;

/// How a text box fits its text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFit {
    /// Wrap at the box width; what does not fit the height is clipped.
    #[default]
    Wrap,
    /// Wrap, and shrink the type (down to 4 pt) until all of it fits.
    Shrink,
    /// The text as typed: one line per line break, no wrapping (it can run
    /// past the box; a clipping box cuts it off).
    Off,
}

/// The lines a text box shows and the type size they are set in.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedText {
    pub size_pt: f64,
    pub lines: Vec<String>,
}

/// Breaks `word` into pieces no wider than `max_w`, at least one character each.
fn break_word(word: &str, size: f64, bold: bool, max_w: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in word.chars() {
        let mut next = cur.clone();
        next.push(ch);
        if !cur.is_empty() && text_w(&next, size, bold) > max_w {
            out.push(std::mem::take(&mut cur));
            cur.push(ch);
        } else {
            cur = next;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Greedy word wrap of `text` to `max_w` points. Line breaks stay (an empty
/// line is kept); a word wider than the box is broken across lines.
pub fn wrap_lines(text: &str, size: f64, bold: bool, max_w: f64) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.lines() {
        if para.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            if text_w(word, size, bold) <= max_w {
                let joined = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if text_w(&joined, size, bold) <= max_w {
                    line = joined;
                } else {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            } else {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                let mut pieces = break_word(word, size, bold, max_w);
                line = pieces.pop().unwrap_or_default();
                out.extend(pieces);
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// Height the `n` lines of `size` points take, points.
pub fn lines_height(n: usize, size: f64) -> f64 {
    n as f64 * size * LINE_SPACING
}

/// Lays `text` out for a box `w_pt` x `h_pt` points. `size_pt` is the type
/// size asked for; with [`TextFit::Shrink`] the result may be smaller.
pub fn fit_text_box(
    text: &str,
    size_pt: f64,
    bold: bool,
    fit: TextFit,
    w_pt: f64,
    h_pt: f64,
) -> FittedText {
    let avail_w = (w_pt - 2.0 * PAD_PT).max(1.0);
    match fit {
        TextFit::Off => FittedText {
            size_pt,
            lines: text.lines().map(str::to_string).collect(),
        },
        TextFit::Wrap => FittedText {
            size_pt,
            lines: wrap_lines(text, size_pt, bold, avail_w),
        },
        TextFit::Shrink => {
            let mut size = size_pt;
            loop {
                let lines = wrap_lines(text, size, bold, avail_w);
                let fits = lines_height(lines.len(), size) <= h_pt + 1e-6;
                if fits || size <= MIN_SHRINK_PT + 1e-9 {
                    return FittedText {
                        size_pt: size,
                        lines,
                    };
                }
                size = (size - 0.5).max(MIN_SHRINK_PT);
            }
        }
    }
}

/// How many lines of `n` start inside a box `h_pt` tall at `size` points. A
/// line is shown when its top edge is above the box bottom; whether its lower
/// part is cut off is up to the box's clip.
pub fn visible_line_count(n: usize, size: f64, h_pt: f64) -> usize {
    (0..n)
        .take_while(|i| (*i as f64) * size * LINE_SPACING < h_pt - 1e-6)
        .count()
}

/// How many lines fit completely inside `h_pt`.
pub fn whole_line_count(n: usize, size: f64, h_pt: f64) -> usize {
    n.min((h_pt / (size * LINE_SPACING) + 1e-9).floor() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_wraps_inside_the_box_width() {
        let text = "the quick brown fox jumps over the lazy dog and keeps running";
        let l = wrap_lines(text, 10.0, false, 100.0);
        assert!(l.len() > 3);
        for line in &l {
            assert!(text_w(line, 10.0, false) <= 100.0 + 1e-9, "{line}");
        }
        assert_eq!(l.join(" "), text);
    }

    #[test]
    fn line_breaks_and_blank_lines_are_kept() {
        let l = wrap_lines("one\n\ntwo", 10.0, false, 200.0);
        assert_eq!(l, vec!["one", "", "two"]);
    }

    #[test]
    fn a_word_wider_than_the_box_is_broken_not_lost() {
        let l = wrap_lines("ABCDEFGHIJKLMNOPQRSTUVWXYZ", 12.0, true, 40.0);
        assert!(l.len() > 2);
        assert_eq!(l.concat(), "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        for line in &l {
            assert!(text_w(line, 12.0, true) <= 40.0 + 1e-9);
        }
    }

    #[test]
    fn off_keeps_the_lines_as_typed() {
        let f = fit_text_box(
            "a very long single line indeed",
            12.0,
            false,
            TextFit::Off,
            30.0,
            30.0,
        );
        assert_eq!(f.lines, vec!["a very long single line indeed"]);
        assert_eq!(f.size_pt, 12.0);
    }

    #[test]
    fn shrink_reduces_the_type_until_the_text_fits_the_height() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let (w, h) = (120.0, 60.0);
        let wrapped = fit_text_box(text, 14.0, false, TextFit::Wrap, w, h);
        assert!(
            lines_height(wrapped.lines.len(), 14.0) > h,
            "needs to overflow first"
        );
        let shrunk = fit_text_box(text, 14.0, false, TextFit::Shrink, w, h);
        assert!(shrunk.size_pt < 14.0 && shrunk.size_pt >= MIN_SHRINK_PT);
        assert!(lines_height(shrunk.lines.len(), shrunk.size_pt) <= h + 1e-6);
        // Text that already fits is left alone.
        let ok = fit_text_box("short", 10.0, false, TextFit::Shrink, w, h);
        assert_eq!(ok.size_pt, 10.0);
        // Hopeless text bottoms out at the minimum.
        let tiny = fit_text_box(&text.repeat(20), 14.0, false, TextFit::Shrink, 40.0, 10.0);
        assert_eq!(tiny.size_pt, MIN_SHRINK_PT);
    }

    #[test]
    fn lines_below_the_box_are_not_shown() {
        assert_eq!(visible_line_count(10, 10.0, 30.0), 3);
        assert_eq!(whole_line_count(10, 10.0, 30.0), 2);
        assert_eq!(whole_line_count(1, 10.0, 30.0), 1);
        assert_eq!(visible_line_count(0, 10.0, 30.0), 0);
    }
}
