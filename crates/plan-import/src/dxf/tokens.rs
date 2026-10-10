//! Group-code / value tokenizers for ASCII and binary DXF, plus the small
//! converter that turns ASCII text into binary (for tests and for writers).

use std::borrow::Cow;

/// One group-code / value pair.
pub type Pair<'a> = (i32, Cow<'a, str>);

/// The first bytes of a binary DXF file.
pub const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// Does `bytes` start like a binary DXF?
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(b"AutoCAD Binary DXF")
}

/// Text from file bytes: UTF-8 when valid, otherwise Windows-1252 (what
/// AutoCAD before 2007 wrote).
pub fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(bytes.iter().map(|b| cp1252(*b)).collect()),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}',
        '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}',
        '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}',
        '\u{178}',
    ];
    match b {
        0x80..=0x9f => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

/// Split ASCII text into pairs, skipping comments (999) and resynchronising
/// past lines that are not group codes.
pub fn tokenize_ascii(text: &str) -> Vec<Pair<'_>> {
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let Some(code_line) = lines.next() {
        let Ok(code) = code_line.trim().parse::<i32>() else {
            continue;
        };
        let Some(value) = lines.next() else {
            break;
        };
        if code != 999 {
            out.push((code, Cow::Borrowed(value)));
        }
    }
    out
}

/// How a group code's value is stored in a binary file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    Str,
    I16,
    I32,
    I64,
    F64,
    Bool,
    Chunk,
}

fn wire(code: i32) -> Wire {
    match code {
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => Wire::F64,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => Wire::I16,
        90..=99 | 420..=429 | 440..=459 | 1071 => Wire::I32,
        160..=169 => Wire::I64,
        290..=299 => Wire::Bool,
        310..=319 => Wire::Chunk,
        _ => Wire::Str,
    }
}

/// Split a binary DXF into pairs. A file that ends in the middle of a pair
/// gives the pairs read so far.
///
/// # Errors
/// The sentinel is missing.
pub fn tokenize_binary(bytes: &[u8]) -> Result<Vec<Pair<'static>>, String> {
    if !is_binary(bytes) {
        return Err("not a binary DXF".into());
    }
    // 18 characters, CR LF, then 0x1a 0x00; tolerate a file that lost the tail.
    let mut at = if bytes.len() >= 22 && bytes[18..22] == [13, 10, 26, 0] {
        22
    } else {
        bytes
            .iter()
            .position(|b| *b == 0x1a)
            .map_or(18, |p| (p + 2).min(bytes.len()))
    };
    let mut out = Vec::new();
    let take = |at: &mut usize, n: usize| -> Option<&[u8]> {
        let s = bytes.get(*at..*at + n)?;
        *at += n;
        Some(s)
    };
    while at < bytes.len() {
        let Some(&c) = bytes.get(at) else { break };
        at += 1;
        let code = if c == 255 {
            let Some(b) = take(&mut at, 2) else { break };
            i32::from(u16::from_le_bytes([b[0], b[1]]))
        } else {
            i32::from(c)
        };
        let value: String = match wire(code) {
            Wire::Str => {
                let end = bytes[at..].iter().position(|b| *b == 0);
                let (s, next) = match end {
                    Some(n) => (&bytes[at..at + n], at + n + 1),
                    None => (&bytes[at..], bytes.len()),
                };
                at = next;
                decode_text(s).into_owned()
            }
            Wire::I16 => {
                let Some(b) = take(&mut at, 2) else { break };
                i16::from_le_bytes([b[0], b[1]]).to_string()
            }
            Wire::I32 => {
                let Some(b) = take(&mut at, 4) else { break };
                i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string()
            }
            Wire::I64 => {
                let Some(b) = take(&mut at, 8) else { break };
                i64::from_le_bytes(b.try_into().unwrap_or([0; 8])).to_string()
            }
            Wire::F64 => {
                let Some(b) = take(&mut at, 8) else { break };
                f64::from_le_bytes(b.try_into().unwrap_or([0; 8])).to_string()
            }
            Wire::Bool => {
                let Some(b) = take(&mut at, 1) else { break };
                b[0].to_string()
            }
            Wire::Chunk => {
                let Some(n) = take(&mut at, 1).map(|b| usize::from(b[0])) else {
                    break;
                };
                let Some(b) = take(&mut at, n) else { break };
                b.iter().map(|x| format!("{x:02X}")).collect()
            }
        };
        if code != 999 {
            out.push((code, Cow::Owned(value)));
        }
    }
    Ok(out)
}

/// Write pairs as a binary DXF (R13 layout: codes of 255 and up take the
/// 255 escape).
pub fn encode_binary(pairs: &[Pair<'_>]) -> Vec<u8> {
    let mut out = BINARY_SENTINEL.to_vec();
    for (code, value) in pairs {
        if *code >= 255 {
            out.push(255);
            out.extend_from_slice(&(*code as u16).to_le_bytes());
        } else {
            out.push(*code as u8);
        }
        let v = value.trim();
        match wire(*code) {
            Wire::Str => {
                out.extend_from_slice(value.as_bytes());
                out.push(0);
            }
            Wire::I16 => {
                out.extend_from_slice(&(v.parse::<i64>().unwrap_or(0) as i16).to_le_bytes())
            }
            Wire::I32 => {
                out.extend_from_slice(&(v.parse::<i64>().unwrap_or(0) as i32).to_le_bytes())
            }
            Wire::I64 => out.extend_from_slice(&v.parse::<i64>().unwrap_or(0).to_le_bytes()),
            Wire::F64 => out.extend_from_slice(&v.parse::<f64>().unwrap_or(0.0).to_le_bytes()),
            Wire::Bool => out.push(v.parse::<i64>().unwrap_or(0) as u8),
            Wire::Chunk => {
                let bytes: Vec<u8> = (0..v.len() / 2)
                    .filter_map(|i| u8::from_str_radix(v.get(2 * i..2 * i + 2)?, 16).ok())
                    .collect();
                out.push(bytes.len().min(255) as u8);
                out.extend_from_slice(&bytes[..bytes.len().min(255)]);
            }
        }
    }
    out
}

/// An ASCII DXF as a binary one (same content).
pub fn ascii_to_binary(text: &str) -> Vec<u8> {
    encode_binary(&tokenize_ascii(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_pairs_skip_comments_and_noise() {
        let t = tokenize_ascii("999\nhi\n  0\r\nSECTION\r\nnoise\n  2\nHEADER\n");
        let t: Vec<(i32, &str)> = t.iter().map(|(c, v)| (*c, v.as_ref())).collect();
        assert_eq!(t, vec![(0, "SECTION"), (2, "HEADER")]);
    }

    #[test]
    fn binary_round_trips_every_value_type() {
        let pairs: Vec<Pair> = vec![
            (0, "LINE".into()),
            (5, "2A".into()),
            (8, "Walls".into()),
            (10, "1.5".into()),
            (70, "-3".into()),
            (90, "70000".into()),
            (160, "5000000000".into()),
            (290, "1".into()),
            (310, "0A0B".into()),
            (370, "25".into()),
            (420, "16711680".into()),
            (1000, "xdata".into()),
            (1071, "123456".into()),
            (0, "EOF".into()),
        ];
        let bytes = encode_binary(&pairs);
        assert!(is_binary(&bytes));
        let back = tokenize_binary(&bytes).unwrap();
        assert_eq!(back, pairs);
        // A cut-off file keeps what was complete.
        let cut = tokenize_binary(&bytes[..bytes.len() - 5]).unwrap();
        assert!(cut.len() < pairs.len() && cut.len() >= pairs.len() - 3);
        assert!(tokenize_binary(b"hello").is_err());
    }

    #[test]
    fn old_files_decode_as_windows_1252() {
        let t = decode_text(b"Caf\xe9 \x80 100");
        assert_eq!(t, "Caf\u{e9} \u{20ac} 100");
        assert_eq!(decode_text("fine".as_bytes()), "fine");
    }
}
