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
#[derive(Default)]
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
        hints: opts.hints.into_iter().map(Into::into).collect(),
        max_cell_len: 200,
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

/// A key-value item for [`JsAgentResponse::kv_items`] (JavaScript-friendly).
#[napi(object)]
pub struct JsKvItem {
    /// The field name.
    pub key: String,
    /// The field value.
    pub value: JsToonValue,
}

fn js_kv_value_to_rust(v: JsToonValue) -> crate::kv::KvValue {
    match v.r#type.as_str() {
        "str" => crate::kv::KvValue::Text(v.str_val.unwrap_or_default()),
        "int" => crate::kv::KvValue::Int(i64::from(v.int_val.unwrap_or(0))),
        "float" => crate::kv::KvValue::Float(v.float_val.unwrap_or(0.0), 6),
        "bool" => crate::kv::KvValue::Bool(v.bool_val.unwrap_or(false)),
        _ => crate::kv::KvValue::Missing,
    }
}

/// NAPI wrapper around [`crate::response::AgentResponse`].
///
/// `AgentResponse`'s Rust methods consume `self` and return `Self` — that
/// idiom can't cross the NAPI boundary directly, since a `#[napi]` class
/// instance is owned by the JS garbage collector and Rust only ever sees
/// `&mut self`. Each mutating method here `take()`s the inner builder out of
/// its `Option` slot, applies the consuming method, and puts the result back.
#[napi(js_name = "AgentResponse")]
pub struct JsAgentResponse {
    inner: Option<crate::response::AgentResponse>,
}

#[napi]
impl JsAgentResponse {
    /// Create a new response builder for the given type name.
    #[napi(constructor, catch_unwind)]
    #[must_use]
    pub fn new(type_name: String) -> Self {
        Self { inner: Some(crate::response::AgentResponse::new(type_name)) }
    }

    fn take(&mut self) -> napi::Result<crate::response::AgentResponse> {
        self.inner.take().ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
    }

    /// Populate the TOON list path.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not
    /// happen in normal use), or if `rows`/`fields`/any row's value count
    /// exceed this crate's NAPI-boundary size limits.
    #[napi(catch_unwind)]
    #[allow(clippy::needless_pass_by_value)] // napi-derive requires owned Vec<String> for JS array params
    pub fn items(&mut self, rows: Vec<Vec<JsToonValue>>, fields: Vec<String>) -> napi::Result<()> {
        if rows.len() > MAX_ROWS {
            return Err(napi::Error::from_reason(format!("rows length {} exceeds maximum of {MAX_ROWS}", rows.len())));
        }
        if fields.len() > MAX_FIELDS {
            return Err(napi::Error::from_reason(format!(
                "fields length {} exceeds maximum of {MAX_FIELDS}",
                fields.len()
            )));
        }
        for row in &rows {
            if row.len() > MAX_FIELDS {
                return Err(napi::Error::from_reason(format!(
                    "row length {} exceeds maximum of {MAX_FIELDS}",
                    row.len()
                )));
            }
        }
        let b = self.take()?;
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        let converted: Vec<Vec<crate::toon::Value>> =
            rows.into_iter().map(|row| row.into_iter().map(js_value_to_rust).collect()).collect();
        self.inner = Some(b.items(converted, &field_refs));
        Ok(())
    }

    /// Set the total available count (TOON path only).
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // n clamped non-negative first
    pub fn total_count(&mut self, n: i32) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.total_count(n.max(0) as usize));
        Ok(())
    }

    /// Populate the KV single-item path.
    ///
    /// # Errors
    ///
    /// Returns an error if `items` exceeds [`MAX_FIELDS`] entries, or if an
    /// internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn kv_items(&mut self, items: Vec<JsKvItem>) -> napi::Result<()> {
        if items.len() > MAX_FIELDS {
            return Err(napi::Error::from_reason(format!(
                "items length {} exceeds maximum of {MAX_FIELDS}",
                items.len()
            )));
        }
        let b = self.take()?;
        let converted =
            items.into_iter().map(|i| crate::kv::KvItem { key: i.key, value: js_kv_value_to_rust(i.value) }).collect();
        self.inner = Some(b.kv_items(converted));
        Ok(())
    }

    /// Append a contextual hint.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn hint(&mut self, hint: String) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.hint(hint));
        Ok(())
    }

    /// Append a recovery hint naming a tool (no structured params — use
    /// `AgentResponse` from Rust directly for typed params; the NAPI surface
    /// keeps this to the common case of "here's what to call next").
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn recovery_hint(&mut self, tool: String, reason: Option<String>) -> napi::Result<()> {
        let b = self.take()?;
        let mut hint = crate::recovery::RecoveryHint::new(tool);
        if let Some(reason) = reason {
            hint = hint.reason(reason);
        }
        self.inner = Some(b.recovery_hint(hint));
        Ok(())
    }

    /// Mark this response as an error state.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn as_error(&mut self) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.as_error());
        Ok(())
    }

    /// Render via the TOON or KV path (whichever was populated).
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn render_toon(&self) -> napi::Result<String> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(crate::response::AgentResponse::render_toon)
    }

    /// Render via the KV path.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn render_kv(&self) -> napi::Result<String> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(crate::response::AgentResponse::render_kv)
    }

    /// Render as a compact JSON string (`{"body":...,"hints":[...],"recovery":[...],"isError":bool}`).
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn render_json(&self) -> napi::Result<String> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(|b| b.render(crate::response::OutputFormat::Json))
    }

    /// Render just the `help[N]:` block — the three-surface seam for MCP
    /// frameworks assembling their own display body separately.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn render_hints_only(&self) -> napi::Result<String> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(crate::response::AgentResponse::render_hints_only)
    }
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

    #[test]
    fn js_agent_response_items_then_render_toon() {
        let mut r = JsAgentResponse::new("issues".to_string());
        r.items(vec![vec![JsToonValue { int_val: Some(1), ..value("int") }]], vec!["id".to_string()]).unwrap();
        let out = r.render_toon().unwrap();
        assert!(out.starts_with("issues[1]{id}:"), "got: {out}");
    }

    #[test]
    fn js_agent_response_items_rejects_oversized_row() {
        let mut r = JsAgentResponse::new("issue".to_string());
        let err = r
            .items(vec![(0..=MAX_FIELDS).map(|_| value("null")).collect()], vec!["a".to_string()])
            .expect_err("oversized row must be rejected before consuming the builder");
        assert!(err.reason.contains("row length"), "got: {}", err.reason);

        // The rejected call must not have consumed the builder via `take()`:
        // a subsequent valid call should still succeed.
        r.items(vec![vec![value("null")]], vec!["a".to_string()]).expect("builder still usable after rejection");
    }

    #[test]
    fn js_agent_response_kv_items_rejects_oversized_input() {
        let mut r = JsAgentResponse::new("issue".to_string());
        let oversized: Vec<JsKvItem> =
            (0..=MAX_FIELDS).map(|i| JsKvItem { key: format!("k{i}"), value: value("null") }).collect();
        let err = r.kv_items(oversized).expect_err("oversized items must be rejected before consuming the builder");
        assert!(err.reason.contains("items length"), "got: {}", err.reason);

        // The rejected call must not have consumed the builder via `take()`:
        // a subsequent valid call should still succeed.
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: value("null") }])
            .expect("builder still usable after rejection");
    }

    #[test]
    fn js_agent_response_kv_items_then_render_kv() {
        let mut r = JsAgentResponse::new("issue".to_string());
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: value("null") }]).unwrap();
        let out = r.render_kv().unwrap();
        assert!(out.contains("id:"), "got: {out}");
    }

    #[test]
    fn js_agent_response_hint_and_render_hints_only() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.hint("do this".to_string()).unwrap();
        assert_eq!(r.render_hints_only().unwrap(), "help[1]:\n  do this\n");
    }

    #[test]
    fn js_agent_response_render_json_reflects_is_error() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.as_error().unwrap();
        assert!(r.render_json().unwrap().contains("\"isError\":true"));
    }
}
