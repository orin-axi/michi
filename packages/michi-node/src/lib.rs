#![deny(clippy::all)]

use napi_derive::napi;

/// Value type for a TOON row cell (JavaScript-friendly).
///
/// Use the `type` field to discriminate: `"str"`, `"int"`, `"float"`, `"bool"`, `"null"`.
#[napi(object)]
pub struct JsToonValue {
    #[napi(js_name = "type")]
    pub r#type: String,
    #[napi(js_name = "strVal")]
    pub str_val: Option<String>,
    #[napi(js_name = "intVal")]
    pub int_val: Option<i32>,
    #[napi(js_name = "floatVal")]
    pub float_val: Option<f64>,
    #[napi(js_name = "boolVal")]
    pub bool_val: Option<bool>,
}

/// Options for rendering a TOON document (JavaScript-friendly).
#[napi(object)]
pub struct JsToonOptions {
    #[napi(js_name = "typeName")]
    pub type_name: String,
    pub fields: Vec<String>,
    pub rows: Vec<Vec<JsToonValue>>,
    #[napi(js_name = "totalCount")]
    pub total_count: Option<i32>,
    pub hints: Vec<String>,
}

fn js_value_to_rust(v: JsToonValue) -> michi::toon::Value {
    match v.r#type.as_str() {
        "str" => michi::toon::Value::Str(v.str_val.unwrap_or_default()),
        "int" => michi::toon::Value::Int(v.int_val.unwrap_or(0) as i64),
        "float" => michi::toon::Value::Float(v.float_val.unwrap_or(0.0)),
        "bool" => michi::toon::Value::Bool(v.bool_val.unwrap_or(false)),
        _ => michi::toon::Value::Null,
    }
}

/// Render a TOON list document from options.
#[napi(catch_unwind)]
pub fn render_toon(opts: JsToonOptions) -> String {
    let rust_opts = michi::toon::ToonOptions {
        type_name: opts.type_name,
        fields: opts.fields,
        rows: opts
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(js_value_to_rust).collect())
            .collect(),
        total_count: opts.total_count.map(|n| n as usize),
        hints: opts.hints,
    };
    michi::toon::render_toon(&rust_opts)
}

/// Render a definitive empty state block: `type_name[0]{}:\ntotalCount: 0\n`.
#[napi(catch_unwind)]
pub fn empty_state(type_name: String) -> String {
    michi::empty::empty_state(&type_name)
}

/// Render a `help[N]:` hint block.
#[napi(catch_unwind)]
pub fn render_hints(hints: Vec<String>) -> String {
    let h: Vec<michi::hints::Hint> = hints.into_iter().map(Into::into).collect();
    michi::hints::render_hints(&h)
}

/// Truncate content to `max_chars` Unicode scalar values with an agent-readable suffix.
#[napi(catch_unwind)]
pub fn truncate(content: String, max_chars: i32, hint: String) -> String {
    michi::truncate::truncate_inline(&content, max_chars.max(0) as usize, &hint)
}
