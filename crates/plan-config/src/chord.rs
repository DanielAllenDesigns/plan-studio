//! Key chords and sequences.
//!
//! Chief on macOS stores hotkeys with Qt's modifier names, and Qt swaps the
//! two Mac modifiers: in `UserHotkeys.xml` the token `Ctrl` is the Command key
//! and the token `Meta` is the Control key. (Check: Chief's default Straight
//! Railing is Command-Q and the file stores it as `Ctrl+Q`; Straight Interior
//! Wall is Control-Option-Command-6 and the file stores `Meta+Ctrl+Alt+6`.)
//!
//! [`KeyChord`] uses the physical Mac meaning, so the swap happens once, here:
//! `ctrl` is the Control key, `meta` is the Command key.

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// One key press with modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyChord {
    /// Physical Control key (stored as `Meta` in Chief's file).
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Physical Command key (stored as `Ctrl` in Chief's file).
    pub meta: bool,
    /// Normalized key name: `Q`, `6`, `F8`, `Space`, `Esc`, `Del`, `Tab`,
    /// `Num+`, or a single punctuation character such as `-`.
    pub key: String,
}

impl KeyChord {
    /// A chord with no modifiers.
    pub fn plain(key: &str) -> KeyChord {
        KeyChord {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key: normalize_key(key),
        }
    }

    /// Parses one chord in the notation of `UserHotkeys.xml`
    /// (`Meta+Ctrl+Alt+Shift+Q`, `F8`, `Ctrl+-`, `Space`).
    pub fn parse_file(text: &str) -> Result<KeyChord, ConfigError> {
        let mut rest = text.trim();
        let mut chord = KeyChord::plain("?");
        loop {
            let mut matched = false;
            for name in ["Ctrl", "Alt", "Shift", "Meta"] {
                let n = name.len();
                if rest.len() > n + 1
                    && rest.is_char_boundary(n)
                    && rest[..n].eq_ignore_ascii_case(name)
                    && rest.as_bytes()[n] == b'+'
                {
                    match name {
                        "Ctrl" => chord.meta = true,
                        "Meta" => chord.ctrl = true,
                        "Alt" => chord.alt = true,
                        _ => chord.shift = true,
                    }
                    rest = &rest[n + 1..];
                    matched = true;
                    break;
                }
            }
            if !matched {
                break;
            }
        }
        if rest.is_empty() {
            return Err(ConfigError::Format(format!("empty key in chord '{text}'")));
        }
        chord.key = normalize_key(rest);
        Ok(chord)
    }

    /// Parses a sequence such as `D, H` or `Meta+Ctrl+Alt+4`.
    pub fn parse_file_sequence(text: &str) -> Result<Vec<KeyChord>, ConfigError> {
        let mut parts: Vec<&str> = Vec::new();
        let mut start = 0;
        let bytes = text.as_bytes();
        let mut i = 0;
        while i + 1 < bytes.len() {
            // A separator is ", " not directly after '+' (so "Ctrl+," stays one chord).
            if bytes[i] == b',' && bytes[i + 1] == b' ' && (i == 0 || bytes[i - 1] != b'+') {
                parts.push(&text[start..i]);
                start = i + 2;
                i += 2;
            } else {
                i += 1;
            }
        }
        parts.push(&text[start..]);
        parts.into_iter().map(KeyChord::parse_file).collect()
    }

    /// Parses a chord written with Mac symbols (`⌃⌥⇧⌘Q`, `⇧F6`, `⌦`).
    /// Returns `None` when the text is not a chord.
    pub fn parse_symbols(text: &str) -> Option<KeyChord> {
        let mut chord = KeyChord::plain("?");
        let mut rest = text.trim();
        while let Some(c) = rest.chars().next() {
            match c {
                '\u{2303}' => chord.ctrl = true,
                '\u{2325}' => chord.alt = true,
                '\u{21E7}' => chord.shift = true,
                '\u{2318}' => chord.meta = true,
                _ => break,
            }
            rest = &rest[c.len_utf8()..];
        }
        let key = match rest {
            "\u{2326}" => "Del".to_string(),
            "\u{21E5}" => "Tab".to_string(),
            "\u{232B}" => "Backspace".to_string(),
            "\u{238B}" => "Esc".to_string(),
            "\u{21A9}" => "Enter".to_string(),
            "\u{2212}" => "-".to_string(),
            k if k.chars().count() == 1 && !k.chars().all(char::is_whitespace) => normalize_key(k),
            k if is_named_key(k) => normalize_key(k),
            _ => return None,
        };
        chord.key = key;
        Some(chord)
    }

    /// Mac-symbol form, e.g. `⌃⌥⇧⌘Q` (the form Plan Studio tooltips use).
    pub fn symbols(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('\u{2303}');
        }
        if self.alt {
            s.push('\u{2325}');
        }
        if self.shift {
            s.push('\u{21E7}');
        }
        if self.meta {
            s.push('\u{2318}');
        }
        s.push_str(&self.key);
        s
    }

    /// Back to the notation of `UserHotkeys.xml`.
    pub fn to_file_string(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str("Meta+");
        }
        if self.meta {
            s.push_str("Ctrl+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        s.push_str(&self.key);
        s
    }
}

/// Chief-style text: `Ctrl+Alt+Shift+Cmd+Key`, with `Ctrl` the Control key
/// and `Cmd` the Command key.
impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.meta {
            f.write_str("Cmd+")?;
        }
        f.write_str(&self.key)
    }
}

/// `D, H` style text for a sequence of chords.
pub fn format_sequence(keys: &[KeyChord]) -> String {
    keys.iter()
        .map(KeyChord::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_named_key(k: &str) -> bool {
    let l = k.to_ascii_lowercase();
    matches!(
        l.as_str(),
        "space" | "esc" | "escape" | "del" | "delete" | "tab" | "enter" | "return" | "backspace"
    ) || (l.len() >= 2 && l.starts_with('f') && l[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Canonical key spelling used across the crate.
pub fn normalize_key(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() || s == " " {
        return "Space".to_string();
    }
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.clone().next()) {
        return c.to_uppercase().collect();
    }
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "space" | "spacebar" => "Space".into(),
        "esc" | "escape" => "Esc".into(),
        "del" | "delete" => "Del".into(),
        "tab" => "Tab".into(),
        "enter" | "return" => "Enter".into(),
        "backspace" | "bksp" => "Backspace".into(),
        _ if lower.len() >= 2
            && lower.starts_with('f')
            && lower[1..].chars().all(|c| c.is_ascii_digit()) =>
        {
            s.to_ascii_uppercase()
        }
        _ if lower.starts_with("num") && lower.len() > 3 => format!("Num{}", &s[3..]),
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_modifier_swap_and_display() {
        let c = KeyChord::parse_file("Meta+Ctrl+Alt+6").unwrap();
        assert!(c.ctrl && c.meta && c.alt && !c.shift);
        assert_eq!(c.to_string(), "Ctrl+Alt+Cmd+6");
        assert_eq!(c.symbols(), "\u{2303}\u{2325}\u{2318}6");
        assert_eq!(c.to_file_string(), "Meta+Ctrl+Alt+6");
        assert_eq!(KeyChord::parse_file("Ctrl+Q").unwrap().to_string(), "Cmd+Q");
        assert_eq!(
            KeyChord::parse_file("Shift+F4").unwrap().to_string(),
            "Shift+F4"
        );
    }

    #[test]
    fn sequences_and_odd_keys() {
        let s = KeyChord::parse_file_sequence("D, H").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(format_sequence(&s), "D, H");
        assert_eq!(KeyChord::parse_file("-").unwrap().key, "-");
        assert_eq!(KeyChord::parse_file("Ctrl++").unwrap().key, "+");
        assert_eq!(KeyChord::parse_file("space").unwrap().key, "Space");
        assert_eq!(KeyChord::parse_file("f8").unwrap().key, "F8");
        assert_eq!(KeyChord::parse_file("num+").unwrap().key, "Num+");
        assert!(KeyChord::parse_file("").is_err());
    }

    #[test]
    fn symbol_chords_match_file_chords() {
        let a = KeyChord::parse_symbols("\u{2303}\u{2325}\u{2318}6").unwrap();
        assert_eq!(a, KeyChord::parse_file("Meta+Ctrl+Alt+6").unwrap());
        let b = KeyChord::parse_symbols("\u{21E7}F6").unwrap();
        assert_eq!(b, KeyChord::parse_file("Shift+F6").unwrap());
        assert!(KeyChord::parse_symbols("Related").is_none());
    }
}
