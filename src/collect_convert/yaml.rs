//! The YAML that LLVM's YAML writer produces (clang-tidy's
//! `--export-fixes`), read into JSON values: block mappings and block
//! sequences by indentation, plain, single-quoted and double-quoted
//! scalars (folded across lines as YAML 1.2 §7.3 says, with the escapes of
//! §5.7), and the empty flow collections `[]` and `{}`. Every scalar is a
//! string. Anything else (anchors, tags, block scalars, non-empty flow
//! collections, duplicate keys) is refused, so a document is either read
//! exactly or not at all.
//!
//! Standards: SCF TDA-12 (Input Data Validation: a fixed grammar; whatever
//! it does not cover is an error, never a guess).

use std::collections::VecDeque;

use serde_json::{Map, Value};

/// Every document of `text` (`---` starts one, `...` ends one), in order.
///
/// Every loop below runs at most once per line left and every step takes
/// a line or a character off the front, so the reader ends on any input.
pub(super) fn documents(text: &str) -> Result<Vec<Value>, String> {
    let mut parser = Parser::new(text);
    let mut found = Vec::new();
    for _ in 0..parser.total {
        parser.skip_blank();
        let Some(line) = parser.line() else {
            break;
        };
        match line.trim_end() {
            "---" | "..." => parser.advance(),
            _ => found.push(parser.block(0)?),
        }
    }
    Ok(found)
}

/// The width of `line`'s leading spaces.
fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Whether `content` (a line without its indentation) starts a sequence
/// item.
fn is_item(content: &str) -> bool {
    content == "-" || content.starts_with("- ")
}

/// Whether `content` ends or starts a document.
fn is_marker(content: &str) -> bool {
    matches!(content.trim_end(), "---" | "...")
}

/// The lines not read yet.
struct Parser {
    lines: VecDeque<String>,
    total: usize,
}

impl Parser {
    fn new(text: &str) -> Self {
        let lines: VecDeque<String> = text.split('\n').map(str::to_owned).collect();
        Self {
            total: lines.len(),
            lines,
        }
    }

    /// The current line.
    fn line(&self) -> Option<&str> {
        self.lines.front().map(String::as_str)
    }

    /// Moves past the current line.
    fn advance(&mut self) {
        self.lines.pop_front();
    }

    /// How many lines are left: the bound of every loop over them.
    fn left(&self) -> usize {
        self.lines.len()
    }

    fn error(&self, what: &str) -> String {
        format!("YAML line {}: {what}", self.total - self.left() + 1)
    }

    /// Skips lines holding only whitespace.
    fn skip_blank(&mut self) {
        for _ in 0..self.left() {
            if !self.line().is_some_and(|line| line.trim().is_empty()) {
                break;
            }
            self.advance();
        }
    }

    /// The node starting on the current line, indented at least `min`.
    fn block(&mut self, min: usize) -> Result<Value, String> {
        self.skip_blank();
        let line = self.line().unwrap_or_default();
        let width = indent(line);
        if width < min {
            return Err(self.error("expected a nested node"));
        }
        if is_item(line.trim_start_matches(' ')) {
            self.sequence(width)
        } else {
            self.mapping(width)
        }
    }

    /// The block sequence whose items start at column `width`.
    fn sequence(&mut self, width: usize) -> Result<Value, String> {
        let mut items = Vec::new();
        for _ in 0..self.left() {
            self.skip_blank();
            let Some(line) = self.line() else { break };
            let content = line.trim_start_matches(' ');
            if indent(line) != width || !is_item(content) {
                break;
            }
            let rest = content.get(1..).unwrap_or_default();
            let inner = rest.trim_start_matches(' ');
            if inner.is_empty() {
                self.advance();
                items.push(self.block(width + 1)?);
                continue;
            }
            let column = width + 1 + (rest.len() - inner.len());
            if key_of(inner).is_some() {
                // `- key: value`: a mapping whose first key is on this line.
                let virtual_line = format!("{}{inner}", " ".repeat(column));
                self.advance();
                self.lines.push_front(virtual_line);
                items.push(self.mapping(column)?);
            } else {
                items.push(self.scalar(column)?);
            }
        }
        Ok(Value::Array(items))
    }

    /// The block mapping whose keys start at column `width`.
    fn mapping(&mut self, width: usize) -> Result<Value, String> {
        let mut map = Map::new();
        for _ in 0..self.left() {
            self.skip_blank();
            let Some(line) = self.line() else { break };
            let content = line.trim_start_matches(' ');
            let here = indent(line);
            if here < width || is_marker(content) {
                break;
            }
            if here > width || is_item(content) {
                return Err(self.error("unexpected indentation"));
            }
            let (key, rest) = key_of(content).ok_or_else(|| self.error("expected `key:`"))?;
            if map.contains_key(key) {
                return Err(self.error("duplicate key"));
            }
            let key = key.to_owned();
            let inner = rest.trim_start_matches(' ');
            let value = if inner.is_empty() {
                self.advance();
                self.skip_blank();
                let next = self.line().unwrap_or_default();
                let nested = indent(next);
                if nested > width {
                    self.block(nested)?
                } else if nested == width && is_item(next.trim_start_matches(' ')) {
                    self.sequence(width)?
                } else {
                    Value::Null
                }
            } else {
                let column = here + (content.len() - inner.len());
                self.scalar(column)?
            };
            map.insert(key, value);
        }
        if map.is_empty() {
            return Err(self.error("expected `key:`"));
        }
        Ok(Value::Object(map))
    }

    /// The scalar (or empty flow collection) at `column` of the current
    /// line; moves past its last line.
    fn scalar(&mut self, column: usize) -> Result<Value, String> {
        let line = self.line().unwrap_or_default();
        let text = line.get(column..).unwrap_or_default();
        let value = match text.chars().next() {
            Some(quote @ ('\'' | '"')) => return self.quoted(column, quote),
            _ => text.trim_end(),
        };
        let value = match value {
            "[]" => Value::Array(Vec::new()),
            "{}" => Value::Object(Map::new()),
            _ if value.starts_with(['&', '*', '!', '|', '>', '[', '{', '%', '@', '`'])
                || value.contains(" #") =>
            {
                return Err(self.error("unsupported scalar"));
            }
            plain => Value::String(plain.to_owned()),
        };
        self.advance();
        Ok(value)
    }

    /// The characters of the next line that is not blank, without its
    /// indentation, and how many blank lines came before it; `None` at the
    /// end of the text.
    fn continuation(&mut self) -> Option<(usize, Vec<char>)> {
        let mut breaks = 0;
        for _ in 0..self.left() {
            self.advance();
            match self.line() {
                Some(next) if next.trim().is_empty() => breaks += 1,
                Some(next) => {
                    return Some((
                        breaks,
                        next.trim_start_matches([' ', '\t']).chars().collect(),
                    ));
                }
                None => break,
            }
        }
        None
    }

    /// The quoted scalar whose opening `quote` is at `column`.
    fn quoted(&mut self, column: usize, quote: char) -> Result<Value, String> {
        let mut out = String::new();
        let first: Vec<char> = self
            .line()
            .unwrap_or_default()
            .get(column + 1..)
            .unwrap_or_default()
            .chars()
            .collect();
        let mut chars = first.into_iter().peekable();
        for _ in 0..self.left() {
            let mut joined = false;
            while let Some(c) = chars.next() {
                match (quote, c) {
                    ('\'', '\'') if chars.peek() == Some(&'\'') => {
                        out.push('\'');
                        chars.next();
                    }
                    ('\'', '\'') | ('"', '"') => {
                        let rest: String = chars.collect();
                        if !rest.trim().is_empty() {
                            return Err(self.error("text after a quoted scalar"));
                        }
                        self.advance();
                        return Ok(Value::String(out));
                    }
                    ('"', '\\') => match chars.next() {
                        // An escaped line break: the lines join, no space.
                        None => joined = true,
                        Some(escape) => {
                            let decoded = match escape {
                                'x' => hex(&mut chars, 2),
                                'u' => hex(&mut chars, 4),
                                'U' => hex(&mut chars, 8),
                                _ => simple(escape),
                            };
                            out.push(decoded.ok_or_else(|| self.error("bad escape"))?);
                        }
                    },
                    (_, c) => out.push(c),
                }
            }
            // A line break inside the scalar: folded (§7.3.1, §7.3.2), the
            // white space before it dropped unless it was escaped.
            if !joined {
                let trimmed = out.trim_end_matches([' ', '\t']).len();
                out.truncate(trimmed);
            }
            let Some((breaks, next)) = self.continuation() else {
                break;
            };
            if breaks == 0 && !joined {
                out.push(' ');
            }
            out.extend(std::iter::repeat_n('\n', breaks));
            chars = next.into_iter().peekable();
        }
        Err(self.error("unterminated quoted scalar"))
    }
}

/// The character a one-character escape (YAML 1.2 §5.7) stands for.
fn simple(escape: char) -> Option<char> {
    Some(match escape {
        '0' => '\0',
        'a' => '\u{7}',
        'b' => '\u{8}',
        't' | '\t' => '\t',
        'n' => '\n',
        'v' => '\u{b}',
        'f' => '\u{c}',
        'r' => '\r',
        'e' => '\u{1b}',
        ' ' => ' ',
        '"' => '"',
        '/' => '/',
        '\\' => '\\',
        'N' => '\u{85}',
        '_' => '\u{a0}',
        'L' => '\u{2028}',
        'P' => '\u{2029}',
        _ => return None,
    })
}

/// The character of the next `digits` hex digits of `chars`.
fn hex(chars: &mut impl Iterator<Item = char>, digits: usize) -> Option<char> {
    let code: String = chars.take(digits).collect();
    if code.chars().count() != digits {
        return None;
    }
    u32::from_str_radix(&code, 16).ok().and_then(char::from_u32)
}

/// `key: rest` (or `key:` at the end of the line) with a plain key.
fn key_of(content: &str) -> Option<(&str, &str)> {
    let (key, rest) = content
        .split_once(": ")
        .or_else(|| content.strip_suffix(':').map(|key| (key, "")))?;
    let plain = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    plain.then_some((key, rest))
}

#[cfg(test)]
mod tests;
