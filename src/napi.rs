//! NAPI export surface for the `michi` npm package (`packages/michi-node`).
//!
//! `packages/michi-node/src/lib.rs` re-exports everything from this module
//! (`pub use michi::napi::*;`) rather than defining `#[napi]` items directly,
//! so the export logic can be unit-tested here alongside the rest of the
//! crate. `packages/michi-node` remains a separate Cargo crate only because
//! `crate-type = ["cdylib"]` cannot coexist with a regular `[lib]` in one
//! `Cargo.toml` — see `docs/superpowers/specs/2026-07-03-michi-design.md` §1.
//!
//! Every export uses `#[napi(catch_unwind)]` so a Rust panic cannot crash the
//! host Node.js process, and every export that accepts caller-controlled
//! collection sizes validates them against hard limits before doing any
//! work, since a single crafted call from untrusted JS is otherwise able to
//! force unbounded synchronous allocation on Node's single-threaded event
//! loop.
//!
//! This is the one module in the crate exempted from `#![deny(unsafe_code)]`
//! (src/lib.rs): `#[napi(catch_unwind)]` expands to macro-generated FFI glue
//! that needs `unsafe` to call into the N-API C ABI. No unsafe is hand-written
//! here — only the module-level allow below permits the macro expansion.
#![allow(unsafe_code)]

use napi_derive::napi;

/// Maximum row count accepted per [`render_toon`] call.
const MAX_ROWS: usize = 100_000;
/// Maximum field/column count accepted per header or row.
const MAX_FIELDS: usize = 1_000;
/// Maximum hint count accepted per call.
const MAX_HINTS: usize = 10_000;

/// Value type for a TOON row cell (JavaScript-friendly).
///
/// Use the `type` field to discriminate: `"str"`, `"int"`, `"float"`, `"bool"`, `"null"`.
#[napi(object)]
pub struct JsToonValue {
    /// Discriminant: `"str"`, `"int"`, `"float"`, `"bool"`, or `"null"`.
    #[napi(js_name = "type")]
    pub r#type: String,
    /// The value when `type` is `"str"`.
    #[napi(js_name = "strVal")]
    pub str_val: Option<String>,
    /// The value when `type` is `"int"`.
    #[napi(js_name = "intVal")]
    pub int_val: Option<i32>,
    /// The value when `type` is `"float"`.
    #[napi(js_name = "floatVal")]
    pub float_val: Option<f64>,
    /// The value when `type` is `"bool"`.
    #[napi(js_name = "boolVal")]
    pub bool_val: Option<bool>,
}

/// Options for rendering a TOON document (JavaScript-friendly).
#[napi(object)]
pub struct JsToonOptions {
    /// Snake_case type name, e.g. `"issue"`, `"component"`.
    #[napi(js_name = "typeName")]
    pub type_name: String,
    /// Ordered field names for the header.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`. Capped at [`MAX_ROWS`]
    /// rows and [`MAX_FIELDS`] values per row.
    pub rows: Vec<Vec<JsToonValue>>,
    /// Total available count (may exceed `rows.len()` when paginated).
    #[napi(js_name = "totalCount")]
    pub total_count: Option<i32>,
    /// Agent-facing usage hints. Capped at [`MAX_HINTS`] entries.
    pub hints: Vec<String>,
}

fn js_value_to_rust(v: JsToonValue) -> crate::toon::Value {
    match v.r#type.as_str() {
        "str" => crate::toon::Value::Str(v.str_val.unwrap_or_default()),
        "int" => crate::toon::Value::Int(i64::from(v.int_val.unwrap_or(0))),
        "float" => crate::toon::Value::Float(v.float_val.unwrap_or(0.0)),
        "bool" => crate::toon::Value::Bool(v.bool_val.unwrap_or(false)),
        _ => crate::toon::Value::Null,
    }
}

/// Render a TOON list document from options.
///
/// # Errors
///
/// Returns an error if `rows`, `fields`, `hints`, or any row's value count
/// exceeds this module's hard size limits ([`MAX_ROWS`], [`MAX_FIELDS`],
/// [`MAX_HINTS`]) — an unbounded caller-supplied collection here would let a
/// single JS call force unbounded synchronous allocation on Node's event
/// loop, which `#[napi(catch_unwind)]` alone does not guard against.
#[napi(catch_unwind)]
pub fn render_toon(opts: JsToonOptions) -> napi::Result<String> {
    if opts.rows.len() > MAX_ROWS {
        return Err(napi::Error::from_reason(format!("rows length {} exceeds maximum of {MAX_ROWS}", opts.rows.len())));
    }
    if opts.fields.len() > MAX_FIELDS {
        return Err(napi::Error::from_reason(format!(
            "fields length {} exceeds maximum of {MAX_FIELDS}",
            opts.fields.len()
        )));
    }
    if opts.hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!(
            "hints length {} exceeds maximum of {MAX_HINTS}",
            opts.hints.len()
        )));
    }
    for row in &opts.rows {
        if row.len() > MAX_FIELDS {
            return Err(napi::Error::from_reason(format!("row length {} exceeds maximum of {MAX_FIELDS}", row.len())));
        }
    }

    // Casts are safe: total_count/max_chars are clamped non-negative first,
    // and usize is at least as wide as i32 on every platform this crate targets.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rust_opts = crate::toon::ToonOptions {
        type_name: opts.type_name,
        fields: opts.fields,
        rows: opts.rows.into_iter().map(|row| row.into_iter().map(js_value_to_rust).collect()).collect(),
        total_count: opts.total_count.map(|n| n.max(0) as usize),
        hints: opts.hints,
    };
    Ok(crate::toon::render_toon(&rust_opts))
}

/// Render a definitive empty state block: `type_name[0]{}:\ntotalCount: 0\n`.
#[napi(catch_unwind)]
#[allow(clippy::needless_pass_by_value)] // napi-derive requires owned String for JS string params
pub fn empty_state(type_name: String) -> String {
    crate::empty::empty_state(&type_name)
}

/// Render a `help[N]:` hint block.
///
/// # Errors
///
/// Returns an error if `hints` exceeds [`MAX_HINTS`] entries.
#[napi(catch_unwind)]
pub fn render_hints(hints: Vec<String>) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let h: Vec<crate::hints::Hint> = hints.into_iter().map(Into::into).collect();
    Ok(crate::hints::render_hints(&h))
}

/// Truncate content to `max_chars` Unicode scalar values with an agent-readable suffix.
#[napi(catch_unwind)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // max_chars clamped non-negative first
#[allow(clippy::needless_pass_by_value)] // napi-derive requires owned String for JS string params
pub fn truncate(content: String, max_chars: i32, hint: String) -> String {
    crate::truncate::truncate_inline(&content, max_chars.max(0) as usize, &hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(t: &str) -> JsToonValue {
        JsToonValue { r#type: t.to_string(), str_val: None, int_val: None, float_val: None, bool_val: None }
    }

    #[test]
    fn render_toon_basic() {
        let opts = JsToonOptions {
            type_name: "issue".to_string(),
            fields: vec!["id".to_string()],
            rows: vec![vec![JsToonValue { int_val: Some(1), ..value("int") }]],
            total_count: Some(1),
            hints: vec![],
        };
        let out = render_toon(opts).expect("valid input renders");
        assert!(out.starts_with("issue[1]{id}:"));
    }

    #[test]
    fn render_toon_rejects_too_many_rows() {
        let opts = JsToonOptions {
            type_name: "issue".to_string(),
            fields: vec!["id".to_string()],
            rows: (0..=MAX_ROWS).map(|_| vec![value("null")]).collect(),
            total_count: None,
            hints: vec![],
        };
        assert!(render_toon(opts).is_err());
    }

    #[test]
    fn render_toon_rejects_too_many_fields_in_header() {
        let opts = JsToonOptions {
            type_name: "issue".to_string(),
            fields: vec!["f".to_string(); MAX_FIELDS + 1],
            rows: vec![],
            total_count: None,
            hints: vec![],
        };
        assert!(render_toon(opts).is_err());
    }

    #[test]
    fn render_toon_rejects_too_many_hints() {
        let opts = JsToonOptions {
            type_name: "issue".to_string(),
            fields: vec![],
            rows: vec![],
            total_count: None,
            hints: vec!["h".to_string(); MAX_HINTS + 1],
        };
        assert!(render_toon(opts).is_err());
    }

    #[test]
    fn render_toon_rejects_oversized_row() {
        let opts = JsToonOptions {
            type_name: "issue".to_string(),
            fields: vec!["a".to_string()],
            rows: vec![(0..=MAX_FIELDS).map(|_| value("null")).collect()],
            total_count: None,
            hints: vec![],
        };
        assert!(render_toon(opts).is_err());
    }

    #[test]
    fn empty_state_renders() {
        assert_eq!(empty_state("issue".to_string()), "issue[0]{}:\ntotalCount: 0\n");
    }

    #[test]
    fn render_hints_basic() {
        let out = render_hints(vec!["do this".to_string()]).expect("valid input renders");
        assert!(out.contains("help[1]:"));
    }

    #[test]
    fn render_hints_rejects_too_many() {
        assert!(render_hints(vec!["h".to_string(); MAX_HINTS + 1]).is_err());
    }

    #[test]
    fn truncate_basic() {
        assert_eq!(truncate("hello".to_string(), 100, "full=true".to_string()), "hello");
    }

    #[test]
    fn truncate_clamps_negative_max_chars() {
        // max_chars.max(0) clamps to 0 rather than wrapping/panicking on a negative input.
        let out = truncate("hello".to_string(), -5, "full=true".to_string());
        assert!(out.chars().count() <= 1 || out.contains("chars truncated"));
    }
}
