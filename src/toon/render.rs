use super::escape::escape_value;

/// Cell value variants for a TOON row.
/// Defined here for use by render; re-exported from mod.rs in Task 3.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

/// Render a TOON document string from its parts.
///
/// Pre-allocates output capacity based on row count × estimated row width.
pub(crate) fn render(
    type_name: &str,
    fields: &[String],
    rows: &[Vec<Value>],
    total_count: Option<usize>,
    hints: &[String],
) -> String {
    let row_count = rows.len();
    let field_count = fields.len();
    let capacity = 60 + row_count * (field_count * 12 + 10) + hints.len() * 60;
    let mut out = String::with_capacity(capacity);

    // type_name[count]{field,field,...}:
    out.push_str(type_name);
    out.push('[');
    out.push_str(&row_count.to_string());
    out.push_str("]{");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(field);
    }
    out.push_str("}:\n");

    // rows
    for row in rows {
        out.push_str("  ");
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let s = match val {
                Value::Str(s) => escape_value(s),
                Value::Int(n) => std::borrow::Cow::Owned(n.to_string()),
                Value::Float(f) => std::borrow::Cow::Owned(f.to_string()),
                Value::Bool(b) => std::borrow::Cow::Borrowed(if *b { "true" } else { "false" }),
                Value::Null => std::borrow::Cow::Borrowed(""),
            };
            out.push_str(&s);
        }
        out.push('\n');
    }

    // totalCount
    if let Some(total) = total_count {
        out.push_str("totalCount: ");
        out.push_str(&total.to_string());
        out.push('\n');
    }

    // help[N]: hints
    if !hints.is_empty() {
        out.push_str("help[");
        out.push_str(&hints.len().to_string());
        out.push_str("]:\n");
        for hint in hints {
            out.push_str("  ");
            out.push_str(hint);
            out.push('\n');
        }
    }

    out
}
