//! Imperial helpers. The model stores inches; these convert for display/input.

pub fn feet(f: f64) -> f64 {
    f * 12.0
}

pub fn sq_in_to_sq_ft(a: f64) -> f64 {
    a / 144.0
}

/// Format inches as architectural feet-inches, e.g. `12'-6 1/2"`, rounded to 1/16".
pub fn fmt_ft_in(inches: f64) -> String {
    fmt_ft_in_frac(inches, 16)
}

/// Like [`fmt_ft_in`] but rounded to `1/denom"` (e.g. 8 or 16). A `denom` of 0 is treated as 1.
pub fn fmt_ft_in_frac(inches: f64, denom: u32) -> String {
    let denom = i64::from(denom.max(1));
    let sign = if inches < 0.0 { "-" } else { "" };
    let total = (inches.abs() * denom as f64).round() as i64;
    let feet = total / (12 * denom);
    let rem = total % (12 * denom);
    let whole_in = rem / denom;
    let frac = rem % denom;
    let mut s = format!("{sign}{feet}'-{whole_in}");
    if frac != 0 {
        let mut n = frac;
        let mut d = denom;
        while n % 2 == 0 && d % 2 == 0 {
            n /= 2;
            d /= 2;
        }
        s.push_str(&format!(" {n}/{d}"));
    }
    s.push('"');
    s
}

/// Parse `12'`, `12' 6"`, `12'-6 1/2"`, `6"`, `1/2"`, `12.5'` or a bare number (inches).
pub fn parse_ft_in(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (feet_part, inch_part) = match s.find('\'') {
        Some(i) => (Some(&s[..i]), &s[i + 1..]),
        None => (None, s),
    };
    let mut total = 0.0;
    if let Some(f) = feet_part {
        total += f.trim().parse::<f64>().ok()? * 12.0;
    }
    let inch_part = inch_part
        .trim()
        .trim_start_matches('-')
        .trim()
        .trim_end_matches('"')
        .trim();
    if inch_part.is_empty() {
        return Some(total);
    }
    let mut inches = 0.0;
    for tok in inch_part.split_whitespace() {
        if let Some((n, d)) = tok.split_once('/') {
            let n: f64 = n.parse().ok()?;
            let d: f64 = d.parse().ok()?;
            if d == 0.0 {
                return None;
            }
            inches += n / d;
        } else {
            inches += tok.parse::<f64>().ok()?;
        }
    }
    Some(total + inches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_common_values() {
        assert_eq!(fmt_ft_in(0.0), "0'-0\"");
        assert_eq!(fmt_ft_in(150.0), "12'-6\"");
        assert_eq!(fmt_ft_in(150.5), "12'-6 1/2\"");
        assert_eq!(fmt_ft_in(4.5), "0'-4 1/2\"");
        assert_eq!(fmt_ft_in(109.125), "9'-1 1/8\"");
    }

    #[test]
    fn parses_round_trip() {
        assert_eq!(parse_ft_in("12'-6 1/2\""), Some(150.5));
        assert_eq!(parse_ft_in("12' 6\""), Some(150.0));
        assert_eq!(parse_ft_in("12'"), Some(144.0));
        assert_eq!(parse_ft_in("6\""), Some(6.0));
        assert_eq!(parse_ft_in("36"), Some(36.0));
        assert_eq!(parse_ft_in("12.5'"), Some(150.0));
        assert_eq!(parse_ft_in("abc"), None);
    }
}
