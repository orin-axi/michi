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

/// Maximum row count accepted for a single call's `rows`: [`render_toon`]'s
/// `opts.rows` and [`JsAgentResponse::items`]'s `rows`.
const MAX_ROWS: usize = 100_000;
/// Maximum count accepted per call for anything shaped like "one entry per
/// field": [`render_toon`]'s `opts.fields` and each row's value count,
/// [`JsAgentResponse::items`]'s `fields` and each row's value count, and
/// [`JsAgentResponse::kv_items`]'s `items` (one KV entry is one field).
const MAX_FIELDS: usize = 1_000;
/// Maximum hint count accepted per call: [`render_hints`]'s `hints`,
/// [`append_hints`]'s `hints`, [`render_recovery`]'s `hints`, and the
/// cumulative count enforced across calls by [`JsAgentResponse::hint`] and
/// [`JsAgentResponse::recovery_hint`].
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
    /// Decimal places when `type` is `"float"` (KV render only). Clamped to [0, 20]. Defaults to 6.
    #[napi(js_name = "decimalsVal")]
    pub decimals_val: Option<i32>,
}

/// Options for rendering a TOON document (JavaScript-friendly).
#[napi(object)]
pub struct JsToonOptions {
    /// Snake_case type name, e.g. `"issue"`, `"component"`.
    #[napi(js_name = "typeName")]
    pub type_name: String,
    /// Ordered field names for the header.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`. Capped at `MAX_ROWS`
    /// rows and `MAX_FIELDS` values per row.
    pub rows: Vec<Vec<JsToonValue>>,
    /// Total available count (may exceed `rows.len()` when paginated).
    #[napi(js_name = "totalCount")]
    pub total_count: Option<i32>,
    /// Agent-facing usage hints. Capped at `MAX_HINTS` entries.
    pub hints: Vec<String>,
}

fn js_value_to_rust(v: JsToonValue) -> michi_toon::Value {
    match v.r#type.as_str() {
        "str" => michi_toon::Value::Str(v.str_val.unwrap_or_default().into()),
        "int" => michi_toon::Value::Int(i64::from(v.int_val.unwrap_or(0))),
        "float" => michi_toon::Value::Float(v.float_val.unwrap_or(0.0)),
        "bool" => michi_toon::Value::Bool(v.bool_val.unwrap_or(false)),
        _ => michi_toon::Value::Null,
    }
}

/// Render a TOON list document from options.
///
/// # Errors
///
/// Returns an error if `rows`, `fields`, `hints`, or any row's value count
/// exceeds this module's hard size limits (`MAX_ROWS`, `MAX_FIELDS`,
/// `MAX_HINTS`) — an unbounded caller-supplied collection here would let a
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

    let mut rows: Vec<Vec<michi_toon::Value>> = Vec::with_capacity(opts.rows.len());
    for row in opts.rows {
        if row.len() > MAX_FIELDS {
            return Err(napi::Error::from_reason(format!(
                "row length {} exceeds maximum fields per row of {MAX_FIELDS}",
                row.len()
            )));
        }
        rows.push(row.into_iter().map(js_value_to_rust).collect());
    }

    let toon_opts = michi_toon::ToonOptions::new(opts.type_name, opts.fields, rows)
        .total_count(opts.total_count.map(usize::try_from).and_then(Result::ok))
        .hints(opts.hints);

    Ok(michi_toon::render_toon(&toon_opts))
}

/// Render an explicit empty-state response.
#[napi(catch_unwind)]
pub fn empty_state(type_name: String) -> String {
    michi_core::empty::empty_state(&type_name)
}

/// Render a `help[N]:` hint block.
///
/// # Errors
///
/// Returns an error if `hints` exceeds `MAX_HINTS` entries.
#[napi(catch_unwind)]
pub fn render_hints(hints: Vec<String>) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let h: Vec<crate::hints::Hint> = hints.into_iter().map(Into::into).collect();
    Ok(crate::hints::render_hints(&h))
}

/// Append a `help[N]:` block to an existing body string.
///
/// # Errors
///
/// Returns an error if `hints` exceeds `MAX_HINTS` entries.
#[napi(catch_unwind)]
#[allow(clippy::needless_pass_by_value)] // napi-derive requires owned String for JS string params
pub fn append_hints(body: String, hints: Vec<String>) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let h: Vec<crate::hints::Hint> = hints.into_iter().map(Into::into).collect();
    let mut out = body;
    crate::hints::append_hints(&mut out, &h);
    Ok(out)
}

/// A recovery hint for [`render_recovery`] (JavaScript-friendly — no
/// structured params over this boundary; use `AgentResponse.recoveryHint`
/// for the common case, or the Rust API directly for typed params).
#[napi(object)]
pub struct JsRecoveryHint {
    /// The tool/operation name the agent should call to recover.
    pub tool: String,
    /// Optional human-readable reason.
    pub reason: Option<String>,
}

/// Render recovery hints as a `recovery[N]:` block.
///
/// # Errors
///
/// Returns an error if `hints` exceeds `MAX_HINTS` entries.
#[napi(catch_unwind)]
pub fn render_recovery(hints: Vec<JsRecoveryHint>) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let converted: Vec<crate::recovery::RecoveryHint> = hints
        .into_iter()
        .map(|h| {
            let mut hint = crate::recovery::RecoveryHint::new(h.tool);
            if let Some(reason) = h.reason {
                hint = hint.reason(reason);
            }
            hint
        })
        .collect();
    Ok(crate::recovery::render_recovery(&converted))
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
        "float" => {
            let decimals = v.decimals_val.unwrap_or(6).clamp(0, 20) as u8;
            crate::kv::KvValue::Float(v.float_val.unwrap_or(0.0), decimals)
        }
        "bool" => crate::kv::KvValue::Bool(v.bool_val.unwrap_or(false)),
        _ => crate::kv::KvValue::Missing,
    }
}

/// Render key-value pairs with aligned columns.
#[napi(catch_unwind)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render_kv(items: Vec<JsKvItem>, total_count: Option<i32>, hints: Vec<String>) -> napi::Result<String> {
    if items.len() > MAX_FIELDS {
        return Err(napi::Error::from_reason(format!("items length {} exceeds maximum of {MAX_FIELDS}", items.len())));
    }
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let converted: Vec<crate::kv::KvItem> =
        items.into_iter().map(|i| crate::kv::KvItem { key: i.key, value: js_kv_value_to_rust(i.value) }).collect();
    let hint_objs: Vec<crate::hints::Hint> = hints.into_iter().map(Into::into).collect();
    Ok(crate::kv::render_kv(&converted, total_count.map(|n| n.max(0) as usize), &hint_objs))
}

/// Render an explicit `already_done` status block.
#[napi(catch_unwind)]
pub fn render_already_done(operation: String, summary: String, hints: Vec<String>) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    Ok(michi_resilience::render_already_done(&operation, &summary, &hints))
}

/// Parse an RFC 7231 `Retry-After` header value into seconds.
#[napi(catch_unwind)]
pub fn parse_retry_after(header_value: String) -> Option<f64> {
    michi_resilience::parse_retry_after(&header_value).map(|d| d.as_secs_f64())
}

/// Calculate the next retry delay in milliseconds.
///
/// `jitter_seed` must be in `[0.0, 1.0]`; pass a per-call random value from
/// your preferred RNG to avoid thundering-herd retries across concurrent callers.
/// Delay inputs must be finite and non-negative; `retry_after_ms` may be `null`.
#[napi(catch_unwind)]
#[allow(clippy::too_many_arguments)] // NAPI boundary: all params are scalar and directly map to the JS API
pub fn next_retry_delay(
    max_retries: u32,
    base_delay_ms: f64,
    max_delay_ms: f64,
    jitter_factor: f64,
    jitter_seed: f64,
    attempt: u32,
    retry_after_ms: Option<f64>,
) -> napi::Result<Option<f64>> {
    for (name, val) in [("base_delay_ms", base_delay_ms), ("max_delay_ms", max_delay_ms)] {
        if !val.is_finite() || val < 0.0 {
            return Err(napi::Error::from_reason(format!("{name} must be a finite non-negative number, got {val}")));
        }
        // Duration::from_secs_f64 panics on values > ~1.8e19s; reject here so
        // catch_unwind never has to catch a panic from untrusted JS input.
        if val / 1000.0 > u64::MAX as f64 {
            return Err(napi::Error::from_reason(format!("{name} is too large to convert to a Duration, got {val}")));
        }
    }
    for (name, val) in [("jitter_factor", jitter_factor), ("jitter_seed", jitter_seed)] {
        if !val.is_finite() {
            return Err(napi::Error::from_reason(format!("{name} must be a finite number in [0.0, 1.0], got {val}")));
        }
    }
    if let Some(ms) = retry_after_ms {
        if !ms.is_finite() || ms < 0.0 {
            return Err(napi::Error::from_reason(format!("retry_after_ms must be finite and non-negative, got {ms}")));
        }
    }
    let jitter_seed = jitter_seed.clamp(0.0, 1.0);
    let config = michi_resilience::RetryConfig::new(
        max_retries,
        std::time::Duration::from_secs_f64(base_delay_ms / 1000.0),
        std::time::Duration::from_secs_f64(max_delay_ms / 1000.0),
        jitter_factor,
    );
    let retry_after = retry_after_ms.map(|ms| std::time::Duration::from_secs_f64(ms / 1000.0));
    Ok(michi_resilience::next_retry_delay(&config, attempt, jitter_seed, retry_after).map(|d| d.as_secs_f64() * 1000.0))
}

/// Return `true` if the HTTP status code is conventionally retryable (429, 502, 503, 504).
#[napi(catch_unwind)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn is_retryable_status(status: u32) -> bool {
    let code = u16::try_from(status).unwrap_or(0);
    michi_resilience::is_retryable_status(code)
}

/// Render a classified `DomainError` card or GitHub annotation.
#[napi(catch_unwind)]
pub fn render_domain_error(
    code: String,
    message: String,
    hints: Vec<String>,
    github_annotation: Option<bool>,
) -> napi::Result<String> {
    if hints.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!("hints length {} exceeds maximum of {MAX_HINTS}", hints.len())));
    }
    let error_code = match code.as_str() {
        "invalid_input" => crate::error::ErrorCode::InvalidInput,
        "unauthorized" => crate::error::ErrorCode::Unauthorized,
        "forbidden" => crate::error::ErrorCode::Forbidden,
        "conflict" => crate::error::ErrorCode::Conflict,
        "rate_limited" => crate::error::ErrorCode::RateLimited,
        "unavailable" => crate::error::ErrorCode::Unavailable,
        "timeout" => crate::error::ErrorCode::Timeout,
        "external_failure" => crate::error::ErrorCode::ExternalFailure,
        "not_found" => crate::error::ErrorCode::NotFound,
        other => {
            return Err(napi::Error::from_reason(format!(
                "unknown error code {other:?}; expected one of: invalid_input, not_found, \
                 unauthorized, forbidden, conflict, rate_limited, unavailable, timeout, external_failure"
            )));
        }
    };
    let mut err = crate::error::DomainError::new(error_code, message);
    for h in hints {
        err = err.hint(h);
    }
    if github_annotation.unwrap_or(false) {
        Ok(err.render_github_annotation())
    } else {
        Ok(err.render())
    }
}

/// A component status item for [`render_status`].
#[napi(object)]
pub struct JsStatusItem {
    /// Component key.
    pub key: String,
    /// Component value.
    pub value: JsToonValue,
    /// Health state: `"ok"`, `"degraded: <reason>"`, or `"error: <reason>"`.
    pub health: Option<String>,
}

/// Render a P8 content-first orientation response.
#[napi(catch_unwind)]
pub fn render_status(
    tool_name: String,
    description: String,
    items: Vec<JsStatusItem>,
    hints: Option<Vec<String>>,
) -> napi::Result<String> {
    let hints_vec = hints.unwrap_or_default();
    if items.len() > MAX_FIELDS {
        return Err(napi::Error::from_reason(format!("items length {} exceeds maximum of {MAX_FIELDS}", items.len())));
    }
    if hints_vec.len() > MAX_HINTS {
        return Err(napi::Error::from_reason(format!(
            "hints length {} exceeds maximum of {MAX_HINTS}",
            hints_vec.len()
        )));
    }
    let status_items: Vec<crate::status::StatusItem> = items
        .into_iter()
        .map(|i| {
            let health = match i.health.as_deref() {
                Some("ok") | None => Some(crate::status::Health::Ok),
                Some(h) if h.starts_with("degraded:") => {
                    Some(crate::status::Health::Degraded(h.trim_start_matches("degraded:").trim().to_string()))
                }
                Some(h) if h.starts_with("error:") => {
                    Some(crate::status::Health::Error(h.trim_start_matches("error:").trim().to_string()))
                }
                Some(other) => {
                    return Err(napi::Error::from_reason(format!(
                        "unknown health value {other:?}: expected \"ok\", \"degraded: <reason>\", or \"error: <reason>\""
                    )));
                }
            };
            Ok(crate::status::StatusItem { key: i.key, value: js_kv_value_to_rust(i.value), health })
        })
        .collect::<napi::Result<Vec<_>>>()?;

    let hint_objs: Vec<crate::hints::Hint> = hints_vec.into_iter().map(Into::into).collect();
    let resp = crate::status::StatusResponse::new(tool_name, description, status_items).with_hints(hint_objs);
    Ok(resp.render())
}

/// MCP content-block annotations (JavaScript-friendly). Currently carries
/// only `audience` — michi has no concept of MCP's optional `priority`.
#[napi(object)]
pub struct JsAnnotations {
    /// `["assistant"]` or `["user"]` — which surface(s) this block targets.
    pub audience: Vec<String>,
}

/// One MCP content block (JavaScript-friendly). Wire-conformant with MCP's
/// text content shape: `{type: "text", text, annotations: {audience: [...]}}`.
#[napi(object)]
pub struct JsContentBlock {
    /// Always `"text"` — michi only ever produces text content blocks.
    #[napi(js_name = "type")]
    pub content_type: String,
    /// The block's text content.
    pub text: String,
    /// Which surface(s) this block is meant for.
    pub annotations: JsAnnotations,
}

/// The MCP `CallToolResult` shape, returned by [`JsAgentResponse::to_call_tool_result`].
#[napi(object)]
pub struct JsCallToolResult {
    /// Text content blocks.
    pub content: Vec<JsContentBlock>,
    /// Whether this is a tool execution error.
    #[napi(js_name = "isError")]
    pub is_error: bool,
    /// The same data as `content[0]`, as a real parsed JSON value — not a
    /// string the caller has to `JSON.parse()` themselves.
    #[napi(js_name = "structuredContent")]
    pub structured_content: serde_json::Value,
}

/// NAPI wrapper around [`crate::response::AgentResponse`].
#[napi(js_name = "AgentResponse")]
#[derive(Debug)]
pub struct JsAgentResponse {
    inner: Option<michi_core::response::AgentResponse>,
    /// Cumulative count of [`Self::hint`] calls, capped at [`MAX_HINTS`].
    hint_count: usize,
    /// Cumulative count of [`Self::recovery_hint`] calls, capped at [`MAX_HINTS`].
    recovery_count: usize,
}

#[napi]
impl JsAgentResponse {
    /// Create a new response builder for the given type name.
    #[napi(constructor, catch_unwind)]
    #[must_use]
    pub fn new(type_name: String) -> Self {
        Self { inner: Some(michi_core::response::AgentResponse::new(type_name)), hint_count: 0, recovery_count: 0 }
    }

    fn take(&mut self) -> napi::Result<michi_core::response::AgentResponse> {
        self.inner.take().ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
    }

    /// Populate the TOON list path.
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
        let converted: Vec<Vec<michi_toon::Value>> =
            rows.into_iter().map(|row| row.into_iter().map(js_value_to_rust).collect()).collect();
        self.inner = Some(b.items(converted, &field_refs));
        Ok(())
    }

    /// Set the total available count (TOON path only).
    #[napi(catch_unwind)]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // n clamped non-negative first
    pub fn total_count(&mut self, n: i32) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.total_count(n.max(0) as usize));
        Ok(())
    }

    /// Populate the KV single-item path.
    #[napi(catch_unwind)]
    pub fn kv_items(&mut self, items: Vec<JsKvItem>) -> napi::Result<()> {
        if items.len() > MAX_FIELDS {
            return Err(napi::Error::from_reason(format!(
                "items length {} exceeds maximum of {MAX_FIELDS}",
                items.len()
            )));
        }
        let b = self.take()?;
        let converted = items
            .into_iter()
            .map(|i| michi_core::kv::KvItem { key: i.key, value: js_kv_value_to_rust(i.value) })
            .collect();
        self.inner = Some(b.kv_items(converted));
        Ok(())
    }

    /// Append a contextual hint.
    #[napi(catch_unwind)]
    pub fn hint(&mut self, hint: String) -> napi::Result<()> {
        if self.hint_count >= MAX_HINTS {
            return Err(napi::Error::from_reason(format!("cumulative hint count exceeds maximum of {MAX_HINTS}")));
        }
        let b = self.take()?;
        self.inner = Some(b.hint(hint));
        self.hint_count += 1;
        Ok(())
    }

    /// Append a recovery hint naming a tool.
    #[napi(catch_unwind)]
    pub fn recovery_hint(&mut self, tool: String, reason: Option<String>) -> napi::Result<()> {
        if self.recovery_count >= MAX_HINTS {
            return Err(napi::Error::from_reason(format!(
                "cumulative recovery hint count exceeds maximum of {MAX_HINTS}"
            )));
        }
        let b = self.take()?;
        let mut hint = michi_core::recovery::RecoveryHint::new(tool);
        if let Some(reason) = reason {
            hint = hint.reason(reason);
        }
        self.inner = Some(b.recovery_hint(hint));
        self.recovery_count += 1;
        Ok(())
    }

    /// Mark this response as an error state.
    #[napi(catch_unwind)]
    pub fn as_error(&mut self) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.as_error());
        Ok(())
    }

    /// Attach a human-facing companion block (`audience: user`) for MCP callers.
    #[napi(catch_unwind)]
    pub fn human_content(&mut self, text: String) -> napi::Result<()> {
        let b = self.take()?;
        self.inner = Some(b.human_content(text));
        Ok(())
    }

    /// Render via the TOON path (`items`/`fields`/`totalCount`), reading only
    /// that slot regardless of which of `.items()`/`.kvItems()` was called
    /// last — see [`crate::response::AgentResponse::render_toon`].
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

    /// Render via the KV path (`kvItems`/`totalCount`), reading only that
    /// slot regardless of which of `.items()`/`.kvItems()` was called last —
    /// see [`crate::response::AgentResponse::render_kv`].
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

    /// Render for the given audience — `"assistant"` or `"user"`. `"user"`
    /// returns the `humanContent()` block if one was set, falling back to
    /// the same agent-oriented rendering `"assistant"` would produce
    /// otherwise — never an empty string. See [`Self::has_human_content`]
    /// to check for the fallback case ahead of time.
    ///
    /// # Errors
    ///
    /// Returns an error if `audience` is anything other than `"assistant"`
    /// or `"user"`, or if an internal invariant is violated (should not
    /// happen in normal use).
    #[napi(catch_unwind)]
    #[allow(clippy::needless_pass_by_value)] // napi-derive requires owned String for JS string params
    pub fn render_for(&self, audience: String) -> napi::Result<String> {
        let audience = match audience.as_str() {
            "assistant" => crate::audience::Audience::Assistant,
            "user" => crate::audience::Audience::User,
            other => {
                return Err(napi::Error::from_reason(format!(
                    "unknown audience {other:?}: expected \"assistant\" or \"user\""
                )))
            }
        };
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(|b| b.render_for(audience))
    }

    /// Whether `.humanContent()` was set on this builder.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn has_human_content(&self) -> napi::Result<bool> {
        self.inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))
            .map(crate::response::AgentResponse::has_human_content)
    }

    /// Build the MCP `CallToolResult` for this response. Returns a real
    /// object, not a JSON string — `structuredContent` is a parsed
    /// `serde_json::Value`, so TypeScript callers don't need to
    /// `JSON.parse()` it themselves.
    ///
    /// # Errors
    ///
    /// Returns an error only if an internal invariant is violated (should not happen in normal use).
    #[napi(catch_unwind)]
    pub fn to_call_tool_result(&self) -> napi::Result<JsCallToolResult> {
        let inner = self.inner.as_ref().ok_or_else(|| napi::Error::from_reason("AgentResponse already consumed"))?;
        let result = inner.to_call_tool_result();
        let structured_content = serde_json::from_str(&result.structured_content)
            .map_err(|e| napi::Error::from_reason(format!("structured_content was not valid JSON: {e}")))?;
        Ok(JsCallToolResult {
            content: result
                .content
                .into_iter()
                .map(|c| JsContentBlock {
                    content_type: "text".to_string(),
                    text: c.text,
                    annotations: JsAnnotations {
                        audience: c
                            .audience
                            .into_iter()
                            .map(|a| match a {
                                michi_core::audience::Audience::Assistant => "assistant".to_string(),
                                michi_core::audience::Audience::User => "user".to_string(),
                                _ => "assistant".to_string(),
                            })
                            .collect(),
                    },
                })
                .collect(),
            is_error: result.is_error,
            structured_content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(t: &str) -> JsToonValue {
        JsToonValue {
            r#type: t.to_string(),
            str_val: None,
            int_val: None,
            float_val: None,
            bool_val: None,
            decimals_val: None,
        }
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
    fn js_agent_response_render_toon_is_slot_specific_even_after_kv_items_called_last() {
        let mut r = JsAgentResponse::new("issues".to_string());
        r.items(vec![vec![JsToonValue { int_val: Some(1), ..value("int") }]], vec!["id".to_string()]).unwrap();
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: JsToonValue { int_val: Some(99), ..value("int") } }])
            .unwrap();
        let toon = r.render_toon().unwrap();
        let kv = r.render_kv().unwrap();
        assert!(toon.starts_with("issues[1]{id}:\n  1\n"), "got: {toon}");
        assert_ne!(toon, kv, "render_toon() must not follow the last-called .kvItems()");
    }

    #[test]
    fn js_agent_response_render_kv_is_slot_specific_even_after_items_called_last() {
        let mut r = JsAgentResponse::new("issue".to_string());
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: JsToonValue { int_val: Some(1), ..value("int") } }])
            .unwrap();
        r.items(vec![vec![JsToonValue { int_val: Some(99), ..value("int") }]], vec!["id".to_string()]).unwrap();
        let kv = r.render_kv().unwrap();
        let toon = r.render_toon().unwrap();
        assert!(kv.contains("id: 1"), "got: {kv}");
        assert_ne!(kv, toon, "render_kv() must not follow the last-called .items()");
    }

    #[test]
    fn js_agent_response_hint_and_render_hints_only() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.hint("do this".to_string()).unwrap();
        assert_eq!(r.render_hints_only().unwrap(), "help[1]:\n  do this\n");
    }

    #[test]
    fn render_for_assistant_matches_render_toon() {
        let mut r = JsAgentResponse::new("issue".to_string());
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: value("int") }]).unwrap();
        assert_eq!(r.render_for("assistant".to_string()).unwrap(), r.render_kv().unwrap());
    }

    #[test]
    fn render_for_user_returns_human_content_when_set() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.human_content("hi there".to_string()).unwrap();
        assert_eq!(r.render_for("user".to_string()).unwrap(), "hi there");
    }

    #[test]
    fn render_for_user_falls_back_when_unset() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        assert_eq!(r.render_for("user".to_string()).unwrap(), r.render_kv().unwrap());
    }

    #[test]
    fn render_for_rejects_unknown_audience() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        let err = r.render_for("nonsense".to_string()).expect_err("should reject");
        assert!(err.reason.contains("nonsense"), "got: {}", err.reason);
    }

    #[test]
    fn has_human_content_reflects_whether_it_was_set() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        assert!(!r.has_human_content().unwrap());
        r.human_content("hi".to_string()).unwrap();
        assert!(r.has_human_content().unwrap());
    }

    #[test]
    fn js_agent_response_render_json_reflects_is_error() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.as_error().unwrap();
        assert!(r.render_json().unwrap().contains("\"isError\":true"));
    }

    #[test]
    fn append_hints_appends_to_existing_body() {
        let out = append_hints("body\n".to_string(), vec!["do this".to_string()]).expect("valid input");
        assert_eq!(out, "body\nhelp[1]:\n  do this\n");
    }

    #[test]
    fn append_hints_rejects_too_many() {
        assert!(append_hints("body\n".to_string(), vec!["h".to_string(); MAX_HINTS + 1]).is_err());
    }

    #[test]
    fn render_recovery_basic() {
        let hints = vec![JsRecoveryHint { tool: "retry".to_string(), reason: None }];
        let out = render_recovery(hints).expect("valid input");
        assert!(out.starts_with("recovery[1]:\n  retry"), "got: {out}");
    }

    #[test]
    fn render_recovery_includes_reason() {
        let hints = vec![JsRecoveryHint { tool: "retry".to_string(), reason: Some("rate limited".to_string()) }];
        let out = render_recovery(hints).expect("valid input");
        assert!(out.contains("rate limited"), "got: {out}");
    }

    #[test]
    fn render_recovery_rejects_too_many() {
        let hints: Vec<JsRecoveryHint> =
            (0..=MAX_HINTS).map(|_| JsRecoveryHint { tool: "retry".to_string(), reason: None }).collect();
        assert!(render_recovery(hints).is_err());
    }

    #[test]
    fn js_agent_response_hint_rejects_after_cumulative_limit_exceeded() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        for i in 0..MAX_HINTS {
            r.hint(format!("hint {i}")).expect("under limit should succeed");
        }
        let result = r.hint("one too many".to_string());
        assert!(result.is_err(), "expected error after exceeding MAX_HINTS cumulative hints");
    }

    #[test]
    fn js_agent_response_recovery_hint_rejects_after_cumulative_limit_exceeded() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        for _ in 0..MAX_HINTS {
            r.recovery_hint("retry".to_string(), None).expect("under limit should succeed");
        }
        let result = r.recovery_hint("retry".to_string(), None);
        assert!(result.is_err(), "expected error after exceeding MAX_HINTS cumulative recovery hints");
    }

    #[test]
    fn js_agent_response_to_call_tool_result_basic() {
        let mut r = JsAgentResponse::new("issue".to_string());
        r.kv_items(vec![JsKvItem { key: "id".to_string(), value: value("int") }]).unwrap();
        let result = r.to_call_tool_result().unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
        assert_eq!(result.content[0].annotations.audience, vec!["assistant".to_string()]);
        assert!(!result.is_error);
    }

    #[test]
    fn js_agent_response_to_call_tool_result_reflects_is_error() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.as_error().unwrap();
        let result = r.to_call_tool_result().unwrap();
        assert!(result.is_error);
    }

    #[test]
    fn js_agent_response_to_call_tool_result_structured_content_is_parsed_json() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        let result = r.to_call_tool_result().unwrap();
        assert!(result.structured_content.get("isError").is_some(), "got: {:?}", result.structured_content);
    }

    #[test]
    fn js_agent_response_to_call_tool_result_includes_user_block_with_correct_annotations() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.human_content("friendly summary".to_string()).unwrap();
        let result = r.to_call_tool_result().unwrap();
        assert_eq!(result.content.len(), 2);
        assert_eq!(result.content[1].content_type, "text");
        assert_eq!(result.content[1].annotations.audience, vec!["user".to_string()]);
    }

    #[test]
    fn js_agent_response_human_content_adds_user_audience_block() {
        let mut r = JsAgentResponse::new("t".to_string());
        r.kv_items(vec![]).unwrap();
        r.human_content("friendly summary".to_string()).unwrap();
        let result = r.to_call_tool_result().unwrap();
        assert_eq!(result.content.len(), 2);
        assert_eq!(result.content[1].text, "friendly summary");
        assert_eq!(result.content[1].annotations.audience, vec!["user".to_string()]);
    }

    #[test]
    fn render_already_done_napi_basic() {
        let out = render_already_done("delete_item".into(), "already deleted".into(), vec![]).unwrap();
        assert!(out.contains("operation: delete_item"), "got: {out}");
        assert!(out.contains("status:    already_done"), "got: {out}");
        assert!(out.contains("summary:   already deleted"), "got: {out}");
    }

    #[test]
    fn render_already_done_napi_with_hints() {
        let out = render_already_done("sync".into(), "done".into(), vec!["use get_status to verify".into()]).unwrap();
        assert!(out.contains("help[1]:\n  use get_status to verify\n"), "got: {out}");
    }

    #[test]
    fn render_kv_napi_float_default_decimals() {
        let item = JsKvItem { key: "score".into(), value: JsToonValue { float_val: Some(3.14159), ..value("float") } };
        let out = render_kv(vec![item], None, vec![]).unwrap();
        assert!(out.contains("score:"), "got: {out}");
        assert!(out.contains("3.141590"), "expected 6 decimal places by default, got: {out}");
    }

    #[test]
    fn render_kv_napi_float_custom_decimals() {
        let item = JsKvItem {
            key: "score".into(),
            value: JsToonValue { float_val: Some(3.14159), decimals_val: Some(2), ..value("float") },
        };
        let out = render_kv(vec![item], None, vec![]).unwrap();
        assert!(out.contains("3.14"), "expected 2 decimal places, got: {out}");
        assert!(!out.contains("3.141"), "must not have more than 2 decimal places, got: {out}");
    }

    #[test]
    fn render_status_napi_ok_health() {
        let items = vec![JsStatusItem {
            key: "index".into(),
            value: JsToonValue { str_val: Some("ready".into()), ..value("str") },
            health: Some("ok".into()),
        }];
        let out = render_status("my-tool".into(), "desc".into(), items, None).unwrap();
        assert!(out.contains("tool:"), "got: {out}");
        assert!(!out.contains("DEGRADED") && !out.contains("ERROR"), "ok should have no annotation, got: {out}");
    }

    #[test]
    fn render_status_napi_degraded_health() {
        let items = vec![JsStatusItem {
            key: "cache".into(),
            value: JsToonValue { str_val: Some("warm".into()), ..value("str") },
            health: Some("degraded: high latency".into()),
        }];
        let out = render_status("my-tool".into(), "desc".into(), items, None).unwrap();
        assert!(out.contains("[DEGRADED: high latency]"), "got: {out}");
    }

    #[test]
    fn render_status_napi_error_health() {
        let items = vec![JsStatusItem {
            key: "db".into(),
            value: JsToonValue { str_val: Some("down".into()), ..value("str") },
            health: Some("error: connection refused".into()),
        }];
        let out = render_status("my-tool".into(), "desc".into(), items, None).unwrap();
        assert!(out.contains("[ERROR: connection refused]"), "got: {out}");
    }

    #[test]
    fn render_status_napi_unknown_health_returns_error() {
        let items = vec![JsStatusItem {
            key: "x".into(),
            value: JsToonValue { str_val: Some("v".into()), ..value("str") },
            health: Some("degraded".into()), // missing colon and reason
        }];
        assert!(render_status("t".into(), "d".into(), items, None).is_err());
    }

    #[test]
    fn next_retry_delay_rejects_nan_jitter_factor() {
        let err = next_retry_delay(3, 100.0, 30_000.0, f64::NAN, 0.5, 0, None)
            .expect_err("NaN jitter_factor must be rejected");
        assert!(err.reason.contains("jitter_factor"), "got: {}", err.reason);
        assert!(err.reason.contains("finite"), "got: {}", err.reason);
    }

    #[test]
    fn next_retry_delay_rejects_nan_jitter_seed() {
        let err =
            next_retry_delay(3, 100.0, 30_000.0, 0.5, f64::NAN, 0, None).expect_err("NaN jitter_seed must be rejected");
        assert!(err.reason.contains("jitter_seed"), "got: {}", err.reason);
        assert!(err.reason.contains("finite"), "got: {}", err.reason);
    }
}
