//! A minimal, tolerant XML reader for the COLLADA importer.
//!
//! Handles elements, attributes (single or double quoted), text, CDATA,
//! comments, processing instructions, `<!DOCTYPE ...>` and the five
//! predefined entities plus numeric character references. Namespaces are not
//! interpreted: a `prefix:name` tag keeps its local name only. It is not a
//! validating parser and builds the whole tree in memory.

use crate::model::ModelError;

/// One element.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Node {
    /// Local tag name (any `prefix:` removed).
    pub name: String,
    /// Attributes in file order.
    pub attrs: Vec<(String, String)>,
    /// Child elements in file order.
    pub children: Vec<Node>,
    /// The element's own text (all text pieces joined).
    pub text: String,
}

impl Node {
    /// The value of attribute `key`.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The first child called `name`.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every child called `name`.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The first descendant called `name` (depth first, document order).
    pub fn find(&self, name: &str) -> Option<&Node> {
        for c in &self.children {
            if c.name == name {
                return Some(c);
            }
            if let Some(f) = c.find(name) {
                return Some(f);
            }
        }
        None
    }

    /// Every descendant called `name`, in document order.
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.find_all(name, out);
        }
    }

    /// The text as numbers; tokens that are not numbers are skipped.
    pub fn floats(&self) -> Vec<f64> {
        self.text
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect()
    }

    /// The text as non-negative integers; other tokens are skipped.
    pub fn ints(&self) -> Vec<usize> {
        self.text
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect()
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

struct Reader<'a> {
    s: &'a str,
    at: usize,
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.at..]
    }

    fn skip_past(&mut self, pat: &str) -> bool {
        match self.rest().find(pat) {
            Some(i) => {
                self.at += i + pat.len();
                true
            }
            None => {
                self.at = self.s.len();
                false
            }
        }
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.at += r.len() - r.trim_start().len();
    }

    fn name(&mut self) -> &'a str {
        let r = self.rest();
        let end = r
            .find(|c: char| c.is_whitespace() || matches!(c, '>' | '/' | '='))
            .unwrap_or(r.len());
        self.at += end;
        &r[..end]
    }
}

/// Parses `text` and returns the root element.
pub fn parse(text: &str) -> Result<Node, ModelError> {
    let text = text.trim_start_matches('\u{feff}');
    let mut r = Reader { s: text, at: 0 };
    // Open elements; the last is being filled.
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    while r.at < r.s.len() {
        let rest = r.rest();
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&unescape(&rest[..end]));
            }
            r.at += end;
            continue;
        }
        if rest.starts_with("<!--") {
            r.skip_past("-->");
        } else if rest.starts_with("<![CDATA[") {
            r.at += 9;
            let end = r.rest().find("]]>").unwrap_or(r.rest().len());
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&r.rest()[..end]);
            }
            r.at += end;
            r.skip_past("]]>");
        } else if rest.starts_with("<?") {
            r.skip_past("?>");
        } else if rest.starts_with("<!") {
            // DOCTYPE, possibly with an internal subset in [ ].
            let end = rest.find('>').unwrap_or(rest.len());
            if rest[..end].contains('[') {
                r.skip_past("]>");
            } else {
                r.skip_past(">");
            }
        } else if rest.starts_with("</") {
            r.at += 2;
            r.skip_past(">");
            if let Some(done) = stack.pop() {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(done),
                    None => root = Some(done),
                }
            }
        } else {
            r.at += 1;
            let name = local(r.name()).to_string();
            let mut node = Node {
                name,
                ..Node::default()
            };
            let mut self_closing = false;
            loop {
                r.skip_ws();
                let rest = r.rest();
                if rest.starts_with("/>") {
                    r.at += 2;
                    self_closing = true;
                    break;
                }
                if rest.starts_with('>') {
                    r.at += 1;
                    break;
                }
                if rest.is_empty() {
                    return Err(ModelError("The XML ends inside a tag".into()));
                }
                let key = local(r.name()).to_string();
                if key.is_empty() {
                    // Stray character: skip it so the loop always advances.
                    r.at += rest.chars().next().map_or(1, char::len_utf8);
                    continue;
                }
                r.skip_ws();
                if r.rest().starts_with('=') {
                    r.at += 1;
                    r.skip_ws();
                    let q = r.rest().chars().next().unwrap_or('"');
                    if q == '"' || q == '\'' {
                        r.at += 1;
                        let end = r.rest().find(q).unwrap_or(r.rest().len());
                        node.attrs.push((key, unescape(&r.rest()[..end])));
                        r.at += end + 1;
                    } else {
                        let v = r.name();
                        node.attrs.push((key, unescape(v)));
                    }
                } else {
                    node.attrs.push((key, String::new()));
                }
            }
            if self_closing {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            } else {
                stack.push(node);
            }
        }
        if root.is_some() && stack.is_empty() {
            break;
        }
    }
    // A truncated file: close what is open so the part before it survives.
    while let Some(done) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(done),
            None => root = Some(done),
        }
    }
    root.ok_or_else(|| ModelError("Not an XML file".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_attributes_text_and_entities() {
        let n = parse(
            "<?xml version=\"1.0\"?><!-- c --><a x='1' y=\"&lt;2&gt;\"><b>1 2\n3</b><c/><d><![CDATA[<raw>]]></d><e:f>t&amp;&#65;&#x42;</e:f></a>",
        )
        .unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attr("x"), Some("1"));
        assert_eq!(n.attr("y"), Some("<2>"));
        assert_eq!(n.child("b").unwrap().ints(), vec![1, 2, 3]);
        assert!(n.child("c").is_some());
        assert_eq!(n.child("d").unwrap().text, "<raw>");
        assert_eq!(n.child("f").unwrap().text, "t&AB");
    }

    #[test]
    fn search_helpers_and_floats() {
        let n = parse("<r><a><b>1.5 -2e1 x 3</b></a><a><b/></a></r>").unwrap();
        assert_eq!(n.find("b").unwrap().floats(), vec![1.5, -20.0, 3.0]);
        let mut all = Vec::new();
        n.find_all("b", &mut all);
        assert_eq!(all.len(), 2);
        assert_eq!(n.children_named("a").count(), 2);
    }

    #[test]
    fn doctype_and_truncation_and_junk() {
        let n = parse("<!DOCTYPE r [<!ELEMENT r ANY>]><r><a>1</a>").unwrap();
        assert_eq!(n.child("a").unwrap().text, "1");
        assert!(parse("just text").is_err());
        assert!(parse("").is_err());
        assert!(parse("<a b").is_err());
    }
}
