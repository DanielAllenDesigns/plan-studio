//! A tiny tolerant XML reader: elements, attributes and text.
//!
//! It understands the prolog, comments, CDATA, DOCTYPE, the five predefined
//! entities plus numeric character references, and self-closing tags. It does
//! not do namespaces (a prefix such as `xsi:` is simply part of the name) or
//! validation. Stray or mismatched closing tags are ignored rather than fatal,
//! and elements left open at the end of the input are closed implicitly.

use crate::error::ConfigError;

#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Concatenated direct text (not trimmed).
    pub text: String,
}

impl Element {
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn text_trimmed(&self) -> &str {
        self.text.trim()
    }
}

fn line_of(src: &str, pos: usize) -> usize {
    src.as_bytes()[..pos.min(src.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

fn err(src: &str, pos: usize, msg: &str) -> ConfigError {
    ConfigError::Xml {
        line: line_of(src, pos),
        msg: msg.to_string(),
    }
}

/// Decodes `&amp;`-style entities; unknown ones are kept verbatim.
pub(crate) fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest.find(';').filter(|&j| j <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent.strip_prefix('#').and_then(|n| {
                let code = match n.strip_prefix(['x', 'X']) {
                    Some(h) => u32::from_str_radix(h, 16).ok(),
                    None => n.parse::<u32>().ok(),
                };
                code.and_then(char::from_u32)
            }),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, done: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(done),
        None => {
            if root.is_none() {
                *root = Some(done);
            }
        }
    }
}

/// Parses `src` and returns the root element.
pub(crate) fn parse(src: &str) -> Result<Element, ConfigError> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let b = src.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    while i < b.len() {
        if b[i] != b'<' {
            let end = src[i..].find('<').map_or(b.len(), |j| i + j);
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&decode_entities(&src[i..end]));
            }
            i = end;
            continue;
        }
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            let j = rest
                .find("-->")
                .ok_or_else(|| err(src, i, "unterminated comment"))?;
            i += j + 3;
        } else if rest.starts_with("<![CDATA[") {
            let j = rest
                .find("]]>")
                .ok_or_else(|| err(src, i, "unterminated CDATA"))?;
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&rest[9..j]);
            }
            i += j + 3;
        } else if rest.starts_with("<?") {
            let j = rest
                .find("?>")
                .ok_or_else(|| err(src, i, "unterminated processing instruction"))?;
            i += j + 2;
        } else if rest.starts_with("<!") {
            let j = rest
                .find('>')
                .ok_or_else(|| err(src, i, "unterminated declaration"))?;
            i += j + 1;
        } else if rest.starts_with("</") {
            let j = rest
                .find('>')
                .ok_or_else(|| err(src, i, "unterminated closing tag"))?;
            let name = rest[2..j].trim();
            if let Some(pos) = stack.iter().rposition(|e| e.name == name) {
                while stack.len() > pos {
                    let done = stack.pop().expect("stack is non-empty");
                    attach(&mut stack, &mut root, done);
                }
            }
            i += j + 1;
        } else {
            // Start tag.
            let start = i;
            i += 1;
            let name_end = src[i..]
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .map_or(b.len(), |j| i + j);
            let mut el = Element {
                name: src[i..name_end].to_string(),
                ..Element::default()
            };
            if el.name.is_empty() {
                return Err(err(src, start, "empty element name"));
            }
            i = name_end;
            let self_closing;
            loop {
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= b.len() {
                    return Err(err(src, start, "unterminated start tag"));
                }
                if b[i] == b'>' {
                    i += 1;
                    self_closing = false;
                    break;
                }
                if b[i] == b'/' {
                    i += 1;
                    if i < b.len() && b[i] == b'>' {
                        i += 1;
                    }
                    self_closing = true;
                    break;
                }
                let an_end = src[i..]
                    .find(|c: char| c.is_whitespace() || c == '=' || c == '/' || c == '>')
                    .map_or(b.len(), |j| i + j);
                let an_end = an_end.max(i + 1);
                let aname = src[i..an_end].to_string();
                i = an_end;
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
                let mut value = String::new();
                if i < b.len() && b[i] == b'=' {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                        let q = b[i] as char;
                        let close = src[i + 1..]
                            .find(q)
                            .ok_or_else(|| err(src, i, "unterminated attribute value"))?;
                        value = decode_entities(&src[i + 1..i + 1 + close]);
                        i += close + 2;
                    } else {
                        let ve = src[i..]
                            .find(|c: char| c.is_whitespace() || c == '>')
                            .map_or(b.len(), |j| i + j);
                        value = decode_entities(&src[i..ve]);
                        i = ve;
                    }
                }
                el.attrs.push((aname, value));
            }
            if self_closing {
                attach(&mut stack, &mut root, el);
            } else {
                stack.push(el);
            }
        }
    }
    while let Some(done) = stack.pop() {
        attach(&mut stack, &mut root, done);
    }
    root.ok_or_else(|| err(src, 0, "no root element"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_elements_attrs_and_entities() {
        let x = parse(
            "<?xml version=\"1.0\"?><!-- c --><a x='1' y=\"&lt;2&gt;\"><b>t&amp;u</b><c/><d></d></a>",
        )
        .unwrap();
        assert_eq!(x.name, "a");
        assert_eq!(x.attr("x"), Some("1"));
        assert_eq!(x.attr("y"), Some("<2>"));
        assert_eq!(x.children.len(), 3);
        assert_eq!(x.child("b").unwrap().text_trimmed(), "t&u");
    }

    #[test]
    fn tolerates_unclosed_and_stray_tags() {
        let x = parse("<a><b>x</zzz><c>").unwrap();
        // The stray </zzz> is ignored, so <c> ends up inside the still-open <b>.
        assert_eq!(x.children.len(), 1);
        assert_eq!(x.children[0].children.len(), 1);
    }

    #[test]
    fn rejects_empty_input() {
        assert!(parse("  <!-- nothing -->").is_err());
    }
}
