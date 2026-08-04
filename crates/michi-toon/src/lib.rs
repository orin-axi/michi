#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # michi-toon
//!
//! Token-Optimized Object Notation (TOON) format renderer and parser for michi and AXI.

mod escape;
pub(crate) mod render;

pub use escape::{escape_value, escape_value_quoted};
pub use render::Value;

/// Error returned by [`ToonOptions::validate()`] when structural invariants are violated.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ToonError {
    /// `type_name` contains a structural character (`[`, `]`, `{`, `}`, `\n`, `\r`).
    InvalidTypeName {
        /// The offending name.
        name: String,
    },
    /// A field name contains a structural character (`,`, `{`, `}`, `\n`, `\r`).
    InvalidFieldName {
        /// The offending field name.
        name: String,
    },
    /// Row `row_index` has `actual` values but `fields` declares `expected`.
    RowLengthMismatch {
        /// Zero-based index of the mismatched row.
        row_index: usize,
        /// Number of fields declared in the header.
        expected: usize,
        /// Actual number of values in the row.
        actual: usize,
    },
    /// An item in [`list()`] could not be rendered as a TOON row.
    ///
    /// Items must serialize to a JSON object (`serde_json::Value::Object`).
    /// Scalars, arrays, and `null` values cannot be represented as TOON rows.
    InvalidItem {
        /// Zero-based index of the invalid item.
        row_index: usize,
        /// Reason why the item is invalid.
        reason: String,
    },
}

impl std::fmt::Display for ToonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTypeName { name } => {
                write!(f, "type_name {name:?} contains a structural character")
            }
            Self::InvalidFieldName { name } => {
                write!(f, "field {name:?} contains a structural character")
            }
            Self::RowLengthMismatch { row_index, expected, actual } => {
                write!(f, "row {row_index} has {actual} values but {expected} fields declared")
            }
            Self::InvalidItem { row_index, reason } => {
                write!(f, "item at index {row_index} cannot be rendered as a TOON row: {reason}")
            }
        }
    }
}

impl std::error::Error for ToonError {}

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

    /// Validate structural invariants before rendering.
    ///
    /// [`render_toon()`] sanitizes bad input gracefully; call this when you
    /// need explicit error signals. Caller opt-in for direct
    /// [`render_toon()`] usage — not called automatically there. See
    /// `PRINCIPLES.md` §1 (invariant policy).
    ///
    /// # Errors
    ///
    /// Returns [`ToonError::InvalidTypeName`] if `type_name` contains a
    /// structural character, [`ToonError::InvalidFieldName`] if any field name
    /// contains a structural character, or [`ToonError::RowLengthMismatch`] if
    /// any row's length differs from `fields.len()`.
    pub fn validate(&self) -> Result<(), ToonError> {
        // type_name: all STRUCTURAL chars are invalid (brackets/braces/comma/newlines)
        if self.type_name.chars().any(|c| escape::STRUCTURAL.contains(&c)) {
            return Err(ToonError::InvalidTypeName { name: self.type_name.clone() });
        }
        for field in &self.fields {
            // field names: comma and braces break the `{a,b,c}` header grammar;
            // brackets (`[`, `]`) are also replaced by sanitize_header_token so
            // we reject them here too for consistency with the sanitizer.
            if field.chars().any(|c| escape::STRUCTURAL.contains(&c)) {
                return Err(ToonError::InvalidFieldName { name: field.clone() });
            }
        }
        for (i, row) in self.rows.iter().enumerate() {
            if row.len() != self.fields.len() {
                return Err(ToonError::RowLengthMismatch {
                    row_index: i,
                    expected: self.fields.len(),
                    actual: row.len(),
                });
            }
        }
        Ok(())
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

/// Build and render a TOON document from a slice of `Serialize`-able items.
///
/// Returns the rendered TOON string on success. Returns `Err` if any item
/// does not serialize to a JSON object (e.g. a bare integer or string slice)
/// — use only with homogeneous struct slices.
///
/// Requires the `serde` feature.
///
/// # Errors
///
/// Returns [`ToonError::InvalidItem`] if an item fails to serialize or does
/// not produce a JSON object (struct or map required). Returns
/// [`ToonError::InvalidTypeName`] or [`ToonError::InvalidFieldName`] if
/// `type_name` or a serialized field name contains a structural character
/// (`[`, `]`, `{`, `}`, `,`, `\n`, `\r`).
#[cfg(feature = "serde")]
pub fn list<T: serde::Serialize>(type_name: impl Into<String>, items: &[T]) -> Result<String, ToonError> {
    let type_name = type_name.into();
    let mut fields: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::with_capacity(items.len());
    for (row_index, item) in items.iter().enumerate() {
        let obj = match serde_json::to_value(item) {
            Ok(serde_json::Value::Object(map)) => map,
            Ok(_) => {
                return Err(ToonError::InvalidItem {
                    row_index,
                    reason: "item is not a JSON object (struct or map required)".into(),
                });
            }
            Err(e) => {
                return Err(ToonError::InvalidItem { row_index, reason: e.to_string() });
            }
        };
        if fields.is_empty() && !obj.is_empty() {
            fields = obj.keys().cloned().collect();
        }
        rows.push(fields.iter().map(|f| json_value_to_toon_value(obj.get(f))).collect());
    }
    let opts = ToonOptions { type_name, fields, rows, total_count: None, hints: Vec::new(), max_cell_len: 200 };
    opts.validate()?;
    Ok(render_toon(&opts))
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

#[cfg(test)]
mod validate_tests {
    use super::*;

    #[test]
    fn validate_ok_for_valid_options() {
        let opts = ToonOptions {
            type_name: "file".into(),
            fields: vec!["path".into(), "size".into()],
            rows: vec![vec![Value::from("a.rs"), Value::from(100i64)]],
            hints: vec![],
            max_cell_len: 200,
            total_count: None,
        };
        assert!(opts.validate().is_ok());
    }

    #[test]
    fn validate_rejects_type_name_with_bracket() {
        let opts = ToonOptions {
            type_name: "foo[bar".into(),
            fields: vec![],
            rows: vec![],
            hints: vec![],
            max_cell_len: 200,
            total_count: None,
        };
        assert!(matches!(opts.validate(), Err(ToonError::InvalidTypeName { .. })));
    }

    #[test]
    fn validate_rejects_field_with_comma() {
        let opts = ToonOptions {
            type_name: "t".into(),
            fields: vec!["a,b".into()],
            rows: vec![],
            hints: vec![],
            max_cell_len: 200,
            total_count: None,
        };
        assert!(matches!(opts.validate(), Err(ToonError::InvalidFieldName { .. })));
    }

    #[test]
    fn validate_rejects_row_length_mismatch() {
        let opts = ToonOptions {
            type_name: "t".into(),
            fields: vec!["a".into()],
            rows: vec![vec![Value::from("x"), Value::from("y")]],
            hints: vec![],
            max_cell_len: 200,
            total_count: None,
        };
        assert!(matches!(opts.validate(), Err(ToonError::RowLengthMismatch { row_index: 0, expected: 1, actual: 2 })));
    }
}

#[cfg(all(test, feature = "serde"))]
mod list_tests {
    use super::*;

    #[test]
    fn list_rejects_non_object_items() {
        let result = list("t", &[1u32, 2, 3]);
        assert!(
            matches!(result, Err(ToonError::InvalidItem { row_index: 0, .. })),
            "non-object items must return Err(InvalidItem), got: {result:?}"
        );
    }
}
