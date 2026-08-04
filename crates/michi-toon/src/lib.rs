#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # michi-toon
//!
//! Token-Optimized Object Notation (TOON) format renderer and parser for michi and AXI.

mod escape;
pub(crate) mod render;

pub use escape::{escape_value, escape_value_quoted};
pub use render::Value;

#[cfg(feature = "serde")]
pub mod serializer;

/// Options for rendering a TOON document.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ToonOptions {
    /// Snake_case type name, e.g. `"issue"`, `"component"`.
    pub type_name: String,
    /// Ordered field names for the header, e.g. `["number", "title", "state"]`.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`.
    pub rows: Vec<Vec<Value>>,
    /// Total available count (emitted as `totalCount: N` when `Some`).
    pub total_count: Option<usize>,
    /// Agent-facing usage hints. Emitted as `help[N]:` block when non-empty.
    pub hints: Vec<String>,
    /// Max string cell length before inline truncation. Default: 200.
    pub max_cell_len: usize,
}

impl ToonOptions {
    /// Create a new ToonOptions struct with type_name and fields.
    #[must_use]
    pub fn new(type_name: impl Into<String>, fields: Vec<String>, rows: Vec<Vec<Value>>) -> Self {
        Self { type_name: type_name.into(), fields, rows, total_count: None, hints: Vec::new(), max_cell_len: 200 }
    }

    /// Set total count option.
    #[must_use]
    pub fn total_count(mut self, total: Option<usize>) -> Self {
        self.total_count = total;
        self
    }

    /// Set hints option.
    #[must_use]
    pub fn hints(mut self, hints: Vec<String>) -> Self {
        self.hints = hints;
        self
    }

    /// Set max cell length option.
    #[must_use]
    pub fn max_cell_len(mut self, len: usize) -> Self {
        self.max_cell_len = len;
        self
    }
}

impl Default for ToonOptions {
    fn default() -> Self {
        Self {
            type_name: String::new(),
            fields: Vec::new(),
            rows: Vec::new(),
            total_count: None,
            hints: Vec::new(),
            max_cell_len: 200,
        }
    }
}

/// Render a TOON document to a string.
#[must_use]
pub fn render_toon(opts: &ToonOptions) -> String {
    render::render(&opts.type_name, &opts.fields, &opts.rows, opts.total_count, &opts.hints, opts.max_cell_len)
}

#[cfg(feature = "serde")]
fn json_value_to_toon_value(v: Option<&serde_json::Value>) -> Value {
    match v {
        None | Some(serde_json::Value::Null) => Value::Null,
        Some(serde_json::Value::Bool(b)) => Value::Bool(*b),
        Some(serde_json::Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Str(compact_str::CompactString::new(n.to_string()))
            }
        }
        Some(serde_json::Value::String(s)) => Value::Str(compact_str::CompactString::new(s)),
        Some(other) => Value::Str(compact_str::CompactString::new(other.to_string())),
    }
}

/// Build [`ToonOptions`] from a slice of `Serialize`-able items.
#[cfg(feature = "serde")]
#[must_use]
pub fn list<T: serde::Serialize>(type_name: impl Into<String>, items: &[T]) -> ToonOptions {
    let mut fields: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::with_capacity(items.len());
    for item in items {
        let obj = match serde_json::to_value(item) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        };
        if fields.is_empty() && !obj.is_empty() {
            fields = obj.keys().cloned().collect();
        }
        rows.push(fields.iter().map(|f| json_value_to_toon_value(obj.get(f))).collect());
    }
    ToonOptions { type_name: type_name.into(), fields, rows, total_count: None, hints: Vec::new(), max_cell_len: 200 }
}

#[cfg(test)]
mod auto_trait_tests {
    use super::*;

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn test_auto_traits() {
        assert_send_sync_static::<ToonOptions>();
        assert_send_sync_static::<Value>();
    }
}
