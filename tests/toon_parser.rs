//! Test-only TOON parser — NOT part of the public API. Exists purely to
//! support round-trip property tests (render → parse → compare) per
//! docs/01-spec.md's testing strategy. Parses only what render_toon actually
//! produces; not a general-purpose TOON parser for untrusted input.

use michi::toon::Value;

#[derive(Debug, PartialEq)]
pub struct ParsedToon {
    pub type_name: String,
    pub fields: Vec<String>,
    pub rows: Vec<Vec<String>>, // parsed as raw strings; comparing rendered text, not typed values
    pub total_count: Option<usize>,
    pub hints: Vec<String>,
}

pub fn parse(input: &str) -> Option<ParsedToon> {
    let mut lines = input.lines();
    let header = lines.next()?;
    let (type_name, rest) = header.split_once('[')?;
    let (count_str, rest) = rest.split_once(']')?;
    let row_count: usize = count_str.parse().ok()?;
    let fields_str = rest.strip_prefix('{')?.strip_suffix(":")?.strip_suffix('}')?;
    let fields: Vec<String> =
        if fields_str.is_empty() { Vec::new() } else { fields_str.split(',').map(str::to_string).collect() };

    let mut rows = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let line = lines.next()?.strip_prefix("  ")?;
        rows.push(split_toon_row(line));
    }

    let mut total_count = None;
    let mut hints = Vec::new();
    for line in lines {
        if let Some(n) = line.strip_prefix("totalCount: ") {
            total_count = n.parse().ok();
        } else if line.starts_with("help[") {
            // hint lines follow, each prefixed "  "
        } else if let Some(h) = line.strip_prefix("  ") {
            hints.push(h.to_string());
        }
    }

    Some(ParsedToon { type_name: type_name.to_string(), fields, rows, total_count, hints })
}

fn split_toon_row(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current = String::new();
    let mut in_quotes = false;
    while let Some(c) = chars.next() {
        match c {
            '"' if !in_quotes => in_quotes = true,
            '"' if in_quotes => in_quotes = false,
            '\\' if in_quotes => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ',' if !in_quotes => {
                values.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    values.push(current);
    values
}

pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
        Value::Null => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_document() {
        let input = "issues[2]{number,title,state}:\n  42,Fix login redirect,open\n  43,Add dark mode,open\ntotalCount: 47\nhelp[1]:\n  Call get_issue\n";
        let parsed = parse(input).unwrap();
        assert_eq!(parsed.type_name, "issues");
        assert_eq!(parsed.fields, vec!["number", "title", "state"]);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0], vec!["42", "Fix login redirect", "open"]);
        assert_eq!(parsed.total_count, Some(47));
        assert_eq!(parsed.hints, vec!["Call get_issue"]);
    }

    #[test]
    fn parses_quoted_comma_value() {
        let input = "t[1]{a}:\n  \"x,y\"\n";
        let parsed = parse(input).unwrap();
        assert_eq!(parsed.rows[0], vec!["x,y"]);
    }
}
