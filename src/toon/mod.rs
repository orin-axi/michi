mod escape;
pub(crate) mod render;

pub(crate) use escape::{escape_value, escape_value_quoted};
pub use render::Value;

/// Options for rendering a TOON document.
///
/// TOON (Token-Optimized Object Notation) is the canonical agent-facing list
/// format. Field names appear once in the header; rows are compact
/// comma-separated values. See `docs/01-spec.md` for the grammar.
#[derive(Debug, Clone)]
pub struct ToonOptions {
    /// Snake_case type name, e.g. `"issue"`, `"component"`.
    pub type_name: String,
    /// Ordered field names for the header, e.g. `["number", "title", "state"]`.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`.
    pub rows: Vec<Vec<Value>>,
    /// Total available count (may exceed `rows.len()` when paginated). Emitted
    /// as `totalCount: N` when `Some`.
    pub total_count: Option<usize>,
    /// Agent-facing usage hints. Emitted as `help[N]:` block when non-empty.
    pub hints: Vec<crate::hints::Hint>,
    /// Max `Value::Str` cell length in Unicode scalar values before inline
    /// truncation via [`crate::truncate::truncate_inline`]. Non-string cells
    /// are never truncated.
    pub max_cell_len: usize,
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
///
/// # Panics
///
/// In debug builds, panics if any row's length doesn't match `fields.len()`.
/// Release builds render the mismatched row as-is (this is a development-time
/// correctness signal, not an input-validation guarantee — validate untrusted
/// input before calling this in release builds).
#[must_use]
pub fn render_toon(opts: &ToonOptions) -> String {
    render::render(&opts.type_name, &opts.fields, &opts.rows, opts.total_count, &opts.hints, opts.max_cell_len)
}

/// Convert one field's JSON value into a TOON [`Value`]. Scalars map
/// directly; nested objects/arrays fall back to a compact JSON string —
/// lossless, never a hard error.
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
                Value::Str(n.to_string())
            }
        }
        Some(serde_json::Value::String(s)) => Value::Str(s.clone()),
        Some(other) => Value::Str(other.to_string()),
    }
}

/// Build [`ToonOptions`] from a slice of `Serialize`-able items sharing the
/// same shape. Field order follows the first item's serialized key order;
/// scalar values (string/number/bool/null) map directly to [`Value`],
/// anything else (a nested object or array) is serialized to a compact JSON
/// string and carried as `Value::Str` — lossless, never a hard error. Items
/// that don't serialize to a JSON object (e.g. a bare number or string slice)
/// produce an empty row rather than panicking.
///
/// Requires the `serde` feature.
///
/// ```rust
/// # #[cfg(feature = "serde")] {
/// use michi::toon;
///
/// #[derive(serde::Serialize)]
/// struct Issue { number: u64, title: String, state: String }
///
/// let issues = vec![Issue { number: 51815, title: "Bug".to_string(), state: "open".to_string() }];
/// let opts = toon::list("issues", &issues);
/// let out = toon::render_toon(&opts);
/// assert!(out.starts_with("issues[1]{number,title,state}:\n"));
/// # }
/// ```
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
