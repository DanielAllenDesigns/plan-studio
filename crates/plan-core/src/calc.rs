//! Arithmetic in number boxes (manual part 7, "Arithmetic in dialog number
//! fields"): a length field accepts one or more `+ - * /` operations between
//! lengths, fractions and plain numbers, e.g. `0 + 3"` or `92 5/8 - 3"`.
//!
//! Reading rules, chosen so ordinary architectural entry keeps its meaning:
//!
//! * `+` and `*` (or the multiplication sign) are always operators; so is the
//!   division sign.
//! * `-` is an operator when spaces stand on both sides of it, or when it
//!   follows an inch mark; `12'-6` and `12-6` stay feet-inches.
//! * `/` is an operator when spaces stand on both sides of it, or when it
//!   follows a foot or inch mark; `5/8` stays a fraction.
//! * `*` and `/` bind tighter than `+` and `-`; a plain number next to a
//!   length scales it, and a length divided by a length is a plain ratio.
//! * A plain number standing alone, or added to a length, is read in the
//!   field's default unit, exactly like the same text without an operator.
//!
//! Anything that does not parse (a stray operator, a length times a length,
//! a division by zero) gives `None`, as a bad single value does.

/// One piece of an expression: a plain number, or a length in inches.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Val {
    Scalar(f64),
    Len(f64),
}

/// Evaluates `text` as an expression whose operands `atom` reads as lengths
/// in inches. `unit_len` is what the number 1 stands for in the field's
/// default unit (so a bare product like `6 * 2` stays in that unit).
/// Returns `None` when `text` is not a well-formed expression.
pub fn eval_with(text: &str, atom: &dyn Fn(&str) -> Option<f64>, unit_len: f64) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let terms = split_terms(t)?;
    let mut total = 0.0;
    for (sign, term) in terms {
        let v = eval_term(&term, atom, unit_len)?;
        total += sign * v;
    }
    total.is_finite().then_some(total)
}

/// True when `text` holds a binary operator by the reading rules above.
pub fn has_operator(text: &str) -> bool {
    let t = text.trim();
    let b: Vec<char> = t.chars().collect();
    (0..b.len()).any(|i| is_binary_op(&b, i).is_some())
}

/// Evaluates `text` as plain numbers (angles, counts, percentages).
pub fn eval_number(text: &str) -> Option<f64> {
    let atom = |s: &str| s.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    eval_with(text, &atom, 1.0)
}

/// The operator at `i`, when the character there is a binary operator.
fn is_binary_op(b: &[char], i: usize) -> Option<char> {
    let c = b[i];
    // The first thing in the text, or what follows another operator, is a
    // sign or part of an operand.
    let pi = b[..i].iter().rposition(|p| !p.is_whitespace())?;
    let next_nonspace = b[i + 1..].iter().find(|p| !p.is_whitespace());
    next_nonspace?;
    let before_ws = b[i - 1].is_whitespace();
    let after_ws = b.get(i + 1).is_some_and(|n| n.is_whitespace());
    let prev_c = b[pi];
    let after_operator = matches!(prev_c, '+' | '*' | '\u{d7}' | '\u{f7}')
        || (matches!(prev_c, '-' | '/') && pi != i && is_spaced(b, pi));
    match c {
        '+' | '*' | '\u{d7}' | '\u{f7}' => (!after_operator).then_some(c),
        '-' => {
            if after_operator {
                return None;
            }
            ((before_ws && after_ws) || prev_c == '"').then_some(c)
        }
        '/' => {
            if after_operator {
                return None;
            }
            ((before_ws && after_ws) || matches!(prev_c, '\'' | '"')).then_some(c)
        }
        _ => None,
    }
}

/// Is the `-` or `/` at `i` spaced on both sides (so an operator)?
fn is_spaced(b: &[char], i: usize) -> bool {
    i > 0 && b[i - 1].is_whitespace() && b.get(i + 1).is_some_and(|n| n.is_whitespace())
}

/// Splits at top-level `+` and `-` into signed terms.
fn split_terms(t: &str) -> Option<Vec<(f64, String)>> {
    let b: Vec<char> = t.chars().collect();
    let mut out = Vec::new();
    let mut sign = 1.0;
    let mut cur = String::new();
    for i in 0..b.len() {
        match is_binary_op(&b, i) {
            Some('+') => {
                out.push((sign, std::mem::take(&mut cur)));
                sign = 1.0;
            }
            Some('-') => {
                out.push((sign, std::mem::take(&mut cur)));
                sign = -1.0;
            }
            _ => cur.push(b[i]),
        }
    }
    out.push((sign, cur));
    out.iter().all(|(_, s)| !s.trim().is_empty()).then_some(out)
}

/// Splits a term at `*` and `/` into its factors and the operators between.
fn eval_term(term: &str, atom: &dyn Fn(&str) -> Option<f64>, unit_len: f64) -> Option<f64> {
    let b: Vec<char> = term.chars().collect();
    let mut factors: Vec<(char, String)> = Vec::new();
    let mut op = '*';
    let mut cur = String::new();
    for i in 0..b.len() {
        match is_binary_op(&b, i) {
            Some(c @ ('*' | '\u{d7}' | '/' | '\u{f7}')) => {
                factors.push((op, std::mem::take(&mut cur)));
                op = if c == '/' || c == '\u{f7}' { '/' } else { '*' };
            }
            _ => cur.push(b[i]),
        }
    }
    factors.push((op, cur));
    let mut acc: Option<Val> = None;
    for (op, f) in &factors {
        let f = f.trim();
        if f.is_empty() {
            return None;
        }
        let v = match f.parse::<f64>() {
            Ok(n) if n.is_finite() => Val::Scalar(n),
            _ => Val::Len(atom(f)?),
        };
        acc = Some(match acc {
            None => v,
            Some(a) => combine(a, *op, v)?,
        });
    }
    match acc? {
        Val::Len(l) => Some(l),
        Val::Scalar(s) => Some(s * unit_len),
    }
}

fn combine(a: Val, op: char, b: Val) -> Option<Val> {
    use Val::{Len, Scalar};
    match (a, op, b) {
        (Scalar(x), '*', Scalar(y)) => Some(Scalar(x * y)),
        (Scalar(x), '*', Len(y)) | (Len(y), '*', Scalar(x)) => Some(Len(x * y)),
        (Scalar(x), '/', Scalar(y)) if y != 0.0 => Some(Scalar(x / y)),
        (Len(x), '/', Scalar(y)) if y != 0.0 => Some(Len(x / y)),
        (Len(x), '/', Len(y)) if y != 0.0 => Some(Scalar(x / y)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{parse_length, LengthUnit};

    fn ev(s: &str) -> Option<f64> {
        parse_length(s, LengthUnit::FeetInches)
    }

    fn close(a: Option<f64>, b: f64) {
        let a = a.unwrap_or_else(|| panic!("no value, wanted {b}"));
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn adds_and_subtracts_lengths_with_marks() {
        close(ev("0 + 3\""), 3.0);
        close(ev("12' + 6\""), 150.0);
        close(ev("12'6\" - 1'"), 138.0);
    }

    #[test]
    fn fractions_and_mixed_numbers_are_operands() {
        close(ev("92 5/8 - 3\""), 89.625);
        close(ev("1/2\" + 1/4\""), 0.75);
        close(ev("92 5/8"), 92.625);
    }

    #[test]
    fn dashes_and_fractions_without_spaces_keep_their_meaning() {
        close(ev("12'-6"), 150.0);
        close(ev("12-6"), 150.0);
        close(ev("12-6 1/2"), 150.5);
        close(ev("3/4"), 0.75);
        assert!(!has_operator("12'-6"));
        assert!(!has_operator("5/8"));
        assert!(has_operator("5 - 3"));
        assert!(has_operator("6\"-2\""));
    }

    #[test]
    fn multiplication_and_division_bind_tighter() {
        close(ev("2' + 3 * 4\""), 36.0);
        close(ev("12' / 2"), 72.0);
        close(ev("12'/2 + 6\""), 78.0);
        close(ev("2 * 3' - 1'"), 60.0);
    }

    #[test]
    fn chains_run_left_to_right_within_a_precedence_level() {
        close(ev("10' - 1' - 6\""), 102.0);
        close(ev("100 / 4 / 5"), 5.0);
        close(ev("1 + 2 + 3"), 6.0);
    }

    #[test]
    fn a_length_ratio_is_a_plain_number() {
        close(ev("12' / 3' * 6\""), 24.0);
    }

    #[test]
    fn metric_and_default_units() {
        close(
            parse_length("1m + 50cm", LengthUnit::Millimeters),
            1.5 / 0.0254,
        );
        close(parse_length("6 * 2", LengthUnit::DecimalFeet), 144.0);
        close(parse_length("6 + 2", LengthUnit::Inches), 8.0);
        close(parse_length("6 + 2", LengthUnit::FeetInches), 8.0);
    }

    #[test]
    fn errors_give_none() {
        assert!(ev("3 +").is_none());
        assert!(ev("+ 3 +").is_none());
        assert!(ev("12' * 12'").is_none());
        assert!(ev("12' / 0").is_none());
        assert!(ev("3 * * 4").is_none());
        assert!(ev("abc + 3").is_none());
    }

    #[test]
    fn plain_numbers_evaluate() {
        assert_eq!(eval_number("45 + 45"), Some(90.0));
        assert_eq!(eval_number("360 / 8"), Some(45.0));
        assert_eq!(eval_number("2 * 3 + 1"), Some(7.0));
        assert_eq!(eval_number("-15"), Some(-15.0));
        assert_eq!(eval_number("x"), None);
    }
}
