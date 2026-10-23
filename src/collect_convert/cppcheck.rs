//! Cppcheck's XML version 2 report (`--xml --xml-version=2`): every
//! `<error>` of a reported rule, at its first `<location>`.
//!
//! The report is read by a small reader for exactly what Cppcheck's writer
//! (tinyxml2's printer) produces: an XML declaration, comments, elements
//! whose attributes are double-quoted, the five predefined entities and
//! character references. Text between elements is ignored. Anything else
//! is an error, never an empty report.
//!
//! Standards: SCF TDA-12 (Input Data Validation: malformed output is an
//! error), MON-09 (deterministic records).

use std::collections::BTreeSet;

use super::{Finding, Rule, relative};

/// Whether a tag opens, closes, or is an empty element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    Empty,
}

/// One tag: its name, its attributes (values unescaped) and its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tag<'x> {
    name: &'x str,
    attributes: Vec<(&'x str, String)>,
    kind: Kind,
}

impl Tag<'_> {
    fn attribute(&self, name: &str) -> &str {
        self.attributes
            .iter()
            .find(|(key, _)| *key == name)
            .map_or("", |(_, value)| value.as_str())
    }
}

/// The findings of the reported `rules` in `xml`, Cppcheck's report.
pub(super) fn findings(xml: &str, rules: &[Rule]) -> Result<BTreeSet<Finding>, String> {
    let tags = tags(xml)?;
    let root = tags
        .iter()
        .find(|tag| tag.name == "results" && tag.kind != Kind::Close)
        .ok_or_else(|| "Cppcheck printed no XML report".to_owned())?;
    if root.attribute("version") != "2" {
        return Err(format!(
            "Cppcheck XML: version {:?}, not 2",
            root.attribute("version")
        ));
    }
    let mut found = BTreeSet::new();
    let mut open: Option<Finding> = None;
    for tag in &tags {
        match (tag.name, tag.kind) {
            ("error", Kind::Open) => open = Some(error(tag)),
            ("error", Kind::Empty) => found.extend(Some(error(tag))),
            ("error", Kind::Close) => found.extend(open.take()),
            ("location", Kind::Open | Kind::Empty) => {
                if let Some(finding) = open.as_mut().filter(|finding| finding.uri.is_empty()) {
                    finding.uri = relative(tag.attribute("file"));
                    finding.line = tag.attribute("line").parse().unwrap_or_default();
                    finding.column = tag.attribute("column").parse().unwrap_or_default();
                }
            }
            _ => {}
        }
    }
    Ok(found
        .into_iter()
        .filter(|finding| Rule::reported(rules, &finding.rule).is_some())
        .collect())
}

/// The finding an `<error>` element starts, without a location yet.
fn error(tag: &Tag<'_>) -> Finding {
    Finding {
        uri: String::new(),
        line: 0,
        column: 0,
        rule: tag.attribute("id").to_owned(),
        level: match tag.attribute("severity") {
            "error" => "error",
            "warning" | "portability" | "performance" => "warning",
            _ => "note",
        },
        message: tag.attribute("msg").to_owned(),
        fixes: Vec::new(),
    }
}

/// Every tag of `xml`, in order.
fn tags(xml: &str) -> Result<Vec<Tag<'_>>, String> {
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some((_, after)) = rest.split_once('<') {
        let (end, skip) = if after.starts_with('?') {
            ("?>", true)
        } else if after.starts_with("!--") {
            ("-->", true)
        } else {
            (">", false)
        };
        let (inside, next) = after
            .split_once(end)
            .ok_or_else(|| "Cppcheck XML: an unterminated tag".to_owned())?;
        rest = next;
        if !skip {
            found.push(tag(inside)?);
        }
    }
    Ok(found)
}

/// The tag whose text between `<` and `>` is `inside`.
fn tag(inside: &str) -> Result<Tag<'_>, String> {
    let (kind, body) = if let Some(name) = inside.strip_prefix('/') {
        (Kind::Close, name)
    } else if let Some(body) = inside.strip_suffix('/') {
        (Kind::Empty, body)
    } else {
        (Kind::Open, inside)
    };
    let (name, mut rest) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    if name.is_empty() {
        return Err(format!("Cppcheck XML: a tag without a name: <{inside}>"));
    }
    let mut attributes = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let (key, value) = rest
            .split_once("=\"")
            .ok_or_else(|| format!("Cppcheck XML: a malformed attribute in <{name}>"))?;
        let (value, next) = value
            .split_once('"')
            .ok_or_else(|| format!("Cppcheck XML: an unterminated attribute in <{name}>"))?;
        attributes.push((key.trim(), unescape(value)?));
        rest = next;
    }
    Ok(Tag {
        name,
        attributes,
        kind,
    })
}

/// `raw` with its entity and character references replaced.
fn unescape(raw: &str) -> Result<String, String> {
    let mut parts = raw.split('&');
    let mut text = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        let (entity, tail) = part
            .split_once(';')
            .ok_or_else(|| format!("Cppcheck XML: an unterminated reference in {raw:?}"))?;
        let character = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        text.push(character.ok_or_else(|| format!("Cppcheck XML: unknown reference &{entity};"))?);
        text.push_str(tail);
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
