mod escape;
pub(crate) mod render;

pub(crate) use escape::escape_value;
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
