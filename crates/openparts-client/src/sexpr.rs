//! A small s-expression reader for KiCad files (`.kicad_sch`,
//! `.kicad_pcb`, `.kicad_mod`), keeping each node's byte span so a
//! caller can rewrite one string in place without re-serialising the
//! file.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum Sexpr {
    List {
        items: Vec<Sexpr>,
        span: Range<usize>,
    },
    Atom {
        /// The atom's text, unescaped when it was quoted.
        text: String,
        quoted: bool,
        /// The atom as written, quotes included.
        span: Range<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("s-expression syntax error at byte {offset}: {message}")]
pub struct SexprError {
    pub offset: usize,
    pub message: &'static str,
}

impl Sexpr {
    pub fn span(&self) -> Range<usize> {
        match self {
            Sexpr::List { span, .. } | Sexpr::Atom { span, .. } => span.clone(),
        }
    }

    pub fn items(&self) -> &[Sexpr] {
        match self {
            Sexpr::List { items, .. } => items,
            Sexpr::Atom { .. } => &[],
        }
    }

    pub fn atom(&self) -> Option<&str> {
        match self {
            Sexpr::Atom { text, .. } => Some(text),
            Sexpr::List { .. } => None,
        }
    }

    /// The list's head atom (`footprint` in `(footprint "X" ...)`).
    pub fn head(&self) -> Option<&str> {
        self.items().first().and_then(Sexpr::atom)
    }

    /// The `i`-th item's atom text (0 is the head).
    pub fn arg(&self, i: usize) -> Option<&str> {
        self.items().get(i).and_then(Sexpr::atom)
    }

    /// Direct child lists with head `name`.
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Sexpr> + 'a {
        self.items().iter().filter(move |c| c.head() == Some(name))
    }

    pub fn child(&self, name: &str) -> Option<&Sexpr> {
        self.items().iter().find(|c| c.head() == Some(name))
    }

    /// `(property "name" "value" ...)` children as (name, value node).
    pub fn properties(&self) -> impl Iterator<Item = (&str, &Sexpr)> + '_ {
        self.children("property")
            .filter_map(|p| Some((p.arg(1)?, p.items().get(2)?)))
    }

    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties()
            .find(|(k, _)| *k == name)
            .and_then(|(_, v)| v.atom())
    }
}

/// Parses the first top-level expression of `text`.
pub fn parse(text: &str) -> Result<Sexpr, SexprError> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        text,
        pos: 0,
    };
    p.skip_ws();
    p.expr()
}

/// A KiCad quoted string for `value` (quotes included).
pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, message: &'static str) -> SexprError {
        SexprError {
            offset: self.pos,
            message,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn expr(&mut self) -> Result<Sexpr, SexprError> {
        match self.bytes.get(self.pos) {
            None => Err(self.err("unexpected end of input")),
            Some(b'(') => {
                let start = self.pos;
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.bytes.get(self.pos) {
                        None => return Err(self.err("unclosed list")),
                        Some(b')') => {
                            self.pos += 1;
                            return Ok(Sexpr::List {
                                items,
                                span: start..self.pos,
                            });
                        }
                        Some(_) => items.push(self.expr()?),
                    }
                }
            }
            Some(b')') => Err(self.err("unexpected ')'")),
            Some(b'"') => {
                let start = self.pos;
                self.pos += 1;
                let mut text = String::new();
                let mut run = self.pos;
                loop {
                    match self.bytes.get(self.pos) {
                        None => return Err(self.err("unterminated string")),
                        Some(b'"') => {
                            text.push_str(&self.text[run..self.pos]);
                            self.pos += 1;
                            return Ok(Sexpr::Atom {
                                text,
                                quoted: true,
                                span: start..self.pos,
                            });
                        }
                        Some(b'\\') => {
                            text.push_str(&self.text[run..self.pos]);
                            match self.bytes.get(self.pos + 1) {
                                Some(b'n') => text.push('\n'),
                                Some(b't') => text.push('\t'),
                                Some(&c) if c.is_ascii() => text.push(c as char),
                                // A backslash before a multi-byte char: keep both.
                                Some(_) => {
                                    text.push('\\');
                                    self.pos += 1;
                                    run = self.pos;
                                    continue;
                                }
                                None => return Err(self.err("unterminated string")),
                            }
                            self.pos += 2;
                            run = self.pos;
                        }
                        Some(_) => self.pos += 1,
                    }
                }
            }
            Some(_) => {
                let start = self.pos;
                while let Some(&b) = self.bytes.get(self.pos) {
                    if b.is_ascii_whitespace() || b == b'(' || b == b')' || b == b'"' {
                        break;
                    }
                    self.pos += 1;
                }
                Ok(Sexpr::Atom {
                    text: self.text[start..self.pos].to_string(),
                    quoted: false,
                    span: start..self.pos,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lists_atoms_and_strings_with_spans() {
        let text = r#"(footprint "A\"B" (layer F.Cu) (property "Value" "x y"))"#;
        let e = parse(text).unwrap();
        assert_eq!(e.head(), Some("footprint"));
        assert_eq!(e.arg(1), Some("A\"B"));
        assert_eq!(e.child("layer").unwrap().arg(1), Some("F.Cu"));
        assert_eq!(e.property("Value"), Some("x y"));
        let (_, v) = e.properties().next().unwrap();
        assert_eq!(&text[v.span()], "\"x y\"");
        assert_eq!(e.span(), 0..text.len());
    }

    #[test]
    fn quote_round_trips() {
        let q = quote("a\"b\\c");
        let e = parse(&format!("(x {q})")).unwrap();
        assert_eq!(e.arg(1), Some("a\"b\\c"));
    }

    #[test]
    fn errors_on_unbalanced_input() {
        assert!(parse("(a (b)").is_err());
        assert!(parse("(a \"b)").is_err());
        assert!(parse(")").is_err());
    }
}
