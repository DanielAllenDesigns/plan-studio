//! Text of TEXT/MTEXT entities: AutoCAD control sequences and MTEXT
//! formatting codes.

use super::model::DxfRun;

/// Replace AutoCAD `%%` control sequences and `\U+XXXX` escapes with their
/// characters.
pub fn clean_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'U') {
            // \U+00B0
            let rest: String = chars.clone().take(6).collect();
            if rest.len() == 6 && rest.starts_with("U+") {
                if let Some(ch) = u32::from_str_radix(&rest[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(ch);
                    for _ in 0..6 {
                        chars.next();
                    }
                    continue;
                }
            }
            out.push(c);
            continue;
        }
        if c != '%' || chars.peek() != Some(&'%') {
            out.push(c);
            continue;
        }
        chars.next();
        match chars.peek().copied() {
            Some('d' | 'D') => {
                chars.next();
                out.push('°');
            }
            Some('p' | 'P') => {
                chars.next();
                out.push('±');
            }
            Some('c' | 'C') => {
                chars.next();
                out.push('Ø');
            }
            Some('%') => {
                chars.next();
                out.push('%');
            }
            // Underline / overline / strike toggles carry no text.
            Some('u' | 'U' | 'o' | 'O' | 'k' | 'K') => {
                chars.next();
            }
            Some(d) if d.is_ascii_digit() => {
                let mut n = String::new();
                while n.len() < 3 && chars.peek().is_some_and(char::is_ascii_digit) {
                    n.extend(chars.next());
                }
                match n.parse::<u32>().ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => {
                        out.push_str("%%");
                        out.push_str(&n);
                    }
                }
            }
            Some(other) => {
                chars.next();
                out.push_str("%%");
                out.push(other);
            }
            None => out.push_str("%%"),
        }
    }
    out
}

#[derive(Clone)]
struct Fmt {
    bold: bool,
    italic: bool,
    underline: bool,
    scale: f64,
    color: Option<[u8; 3]>,
}

impl Fmt {
    fn plain() -> Self {
        Fmt {
            bold: false,
            italic: false,
            underline: false,
            scale: 1.0,
            color: None,
        }
    }
}

/// The runs of MTEXT `raw` (`base_height` turns an absolute `\H` into a
/// scale) and its plain text. The runs are empty when the text carries no
/// formatting.
pub fn mtext_runs(raw: &str, base_height: f64) -> (Vec<DxfRun>, String) {
    let mut runs: Vec<DxfRun> = Vec::new();
    let mut first_font: Option<String> = None;
    let mut stack: Vec<Fmt> = Vec::new();
    let mut cur = Fmt::plain();
    let mut buf = String::new();
    let flush = |buf: &mut String, cur: &Fmt, runs: &mut Vec<DxfRun>| {
        if buf.is_empty() {
            return;
        }
        let run = DxfRun {
            text: clean_text(buf),
            bold: cur.bold,
            italic: cur.italic,
            underline: cur.underline,
            scale: cur.scale,
            color: cur.color,
            font: None,
        };
        buf.clear();
        if let Some(last) = runs.last_mut() {
            if last.bold == run.bold
                && last.italic == run.italic
                && last.underline == run.underline
                && (last.scale - run.scale).abs() < 1e-9
                && last.color == run.color
            {
                last.text.push_str(&run.text);
                return;
            }
        }
        runs.push(run);
    };
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                flush(&mut buf, &cur, &mut runs);
                stack.push(cur.clone());
            }
            '}' => {
                flush(&mut buf, &cur, &mut runs);
                if let Some(s) = stack.pop() {
                    cur = s;
                }
            }
            '\\' => {
                let Some(code) = chars.next() else { break };
                match code {
                    'P' | 'N' | 'X' => buf.push('\n'),
                    '~' => buf.push(' '),
                    '\\' => buf.push('\\'),
                    '{' => buf.push('{'),
                    '}' => buf.push('}'),
                    'L' => {
                        flush(&mut buf, &cur, &mut runs);
                        cur.underline = true;
                    }
                    'l' => {
                        flush(&mut buf, &cur, &mut runs);
                        cur.underline = false;
                    }
                    'O' | 'o' | 'K' | 'k' => {}
                    'f' | 'F' => {
                        let arg = until_semicolon(&mut chars);
                        flush(&mut buf, &cur, &mut runs);
                        if first_font.is_none() {
                            let name = arg.split('|').next().unwrap_or("").trim();
                            if !name.is_empty() {
                                first_font = Some(name.to_string());
                            }
                        }
                        let low = arg.to_ascii_lowercase();
                        cur.bold = low.contains("|b1");
                        cur.italic = low.contains("|i1");
                    }
                    'H' => {
                        let arg = until_semicolon(&mut chars);
                        flush(&mut buf, &cur, &mut runs);
                        if let Some(rel) = arg.strip_suffix('x') {
                            if let Ok(k) = rel.trim().parse::<f64>() {
                                cur.scale *= k;
                            }
                        } else if let Ok(h) = arg.trim().parse::<f64>() {
                            if base_height > 0.0 {
                                cur.scale = h / base_height;
                            }
                        }
                    }
                    'C' => {
                        let arg = until_semicolon(&mut chars);
                        flush(&mut buf, &cur, &mut runs);
                        cur.color = arg
                            .trim()
                            .parse::<i32>()
                            .ok()
                            .and_then(crate::dxf::aci::aci_rgb);
                    }
                    'c' => {
                        let arg = until_semicolon(&mut chars);
                        flush(&mut buf, &cur, &mut runs);
                        cur.color = arg.trim().parse::<u32>().ok().map(|v| {
                            [
                                (v & 255) as u8,
                                ((v >> 8) & 255) as u8,
                                ((v >> 16) & 255) as u8,
                            ]
                        });
                    }
                    'S' => {
                        // Stacked fraction "a/b", "a#b" or "a^b".
                        let arg = until_semicolon(&mut chars);
                        let mut out = String::new();
                        for ch in arg.chars() {
                            match ch {
                                '^' | '#' => out.push('/'),
                                '\\' => {}
                                _ => out.push(ch),
                            }
                        }
                        buf.push_str(&out);
                    }
                    'A' | 'W' | 'Q' | 'T' | 'p' => {
                        until_semicolon(&mut chars);
                    }
                    'U' => {
                        // \U+XXXX
                        let rest: String = chars.clone().take(5).collect();
                        if rest.len() == 5 && rest.starts_with('+') {
                            if let Some(ch) = u32::from_str_radix(&rest[1..], 16)
                                .ok()
                                .and_then(char::from_u32)
                            {
                                buf.push(ch);
                                for _ in 0..5 {
                                    chars.next();
                                }
                                continue;
                            }
                        }
                        buf.push('U');
                    }
                    other => buf.push(other),
                }
            }
            _ => buf.push(c),
        }
    }
    flush(&mut buf, &cur, &mut runs);
    let plain: String = runs.iter().map(|r| r.text.as_str()).collect();
    if runs.iter().all(DxfRun::is_plain) {
        runs.clear();
    } else {
        // The first font of the text is used for all of it.
        for r in &mut runs {
            r.font = first_font.clone();
        }
    }
    (runs, plain)
}

fn until_semicolon(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    for n in chars.by_ref() {
        if n == ';' {
            break;
        }
        s.push(n);
    }
    s
}

/// MTEXT with its formatting removed.
pub fn strip_mtext(s: &str) -> String {
    mtext_runs(s, 0.0).1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_sequences_become_characters() {
        assert_eq!(clean_text("45%%d %%p1 %%c10"), "45° ±1 Ø10");
        assert_eq!(clean_text("%%u under%%u"), " under");
        assert_eq!(clean_text("deg \\U+00B0 and \\U+20AC"), "deg ° and €");
        assert_eq!(clean_text("%%176"), "°");
        assert_eq!(clean_text("100%%% sure"), "100% sure");
        assert_eq!(
            clean_text("100%% sure"),
            "100%% sure",
            "a lone %% is not a code"
        );
    }

    #[test]
    fn mtext_formatting_is_stripped_and_kept_as_runs() {
        assert_eq!(
            strip_mtext("{\\fArial|b0;Hi}\\Pthere\\~now"),
            "Hi\nthere now"
        );
        assert_eq!(strip_mtext("\\H2.5;Big \\C1;red"), "Big red");
        assert_eq!(strip_mtext("\\S1/2;in"), "1/2in");
        let (runs, plain) = mtext_runs("plain {\\fArial|b1|i1;bold}{\\L under\\l} \\H2x;big", 2.0);
        assert_eq!(plain, "plain bold under big");
        assert_eq!(runs.len(), 5, "{runs:?}");
        assert!(runs[1].bold && runs[1].italic && !runs[1].underline);
        assert!(runs[2].underline && !runs[2].bold);
        assert_eq!(runs[4].scale, 2.0);
        // An absolute height is relative to the base height.
        let (runs, _) = mtext_runs("a\\H6;b", 3.0);
        assert_eq!(runs[1].scale, 2.0);
        // Colour by index.
        let (runs, _) = mtext_runs("\\C1;red", 1.0);
        assert_eq!(runs[0].color, Some([255, 0, 0]));
        // Plain text has no runs.
        assert!(mtext_runs("just text", 1.0).0.is_empty());
    }
}
