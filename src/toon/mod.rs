mod escape;
pub(crate) mod render;

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
    pub hints: Vec<String>,
}

/// Render a TOON document to a string.
///
/// Row lengths differing from `fields.len()` produce misaligned output —
/// the caller is responsible for ensuring each row has the same number of
/// values as `fields`.
///
/// Does not panic.
#[must_use]
pub fn render_toon(opts: &ToonOptions) -> String {
    render::render(
        &opts.type_name,
        &opts.fields,
        &opts.rows,
        opts.total_count,
        &opts.hints,
    )
}
