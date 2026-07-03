use crate::hints::Hint;

/// Overall health classification of a service or component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// All systems operational.
    Ok,
    /// Degraded but still serving requests.
    Degraded,
    /// Not serving requests.
    Down,
}

impl Health {
    /// The short label string used in rendered output.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::Down => "down",
        }
    }
}

impl std::fmt::Display for Health {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A single named component with a health status and optional detail message.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusItem {
    /// Component name.
    pub name: String,
    /// Health of this component.
    pub health: Health,
    /// Optional human-readable detail (latency, error count, etc.).
    pub detail: Option<String>,
}

impl StatusItem {
    /// Create a status item with no detail message.
    pub fn new(name: impl Into<String>, health: Health) -> Self {
        Self { name: name.into(), health, detail: None }
    }

    /// Create a status item with a detail message.
    pub fn with_detail(name: impl Into<String>, health: Health, detail: impl Into<String>) -> Self {
        Self { name: name.into(), health, detail: Some(detail.into()) }
    }
}

/// A collection of status items with an overall health summary.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusResponse {
    /// Aggregate health across all items.
    pub overall: Health,
    /// Individual component statuses.
    pub items: Vec<StatusItem>,
    /// Optional contextual hints for the agent.
    pub hints: Vec<Hint>,
}

impl StatusResponse {
    /// Create a status response. `overall` is the caller-supplied aggregate — michi
    /// does not recompute it from `items` so the caller controls the roll-up logic.
    pub fn new(overall: Health, items: Vec<StatusItem>) -> Self {
        Self { overall, items, hints: Vec::new() }
    }

    /// Attach contextual hints.
    pub fn with_hints(mut self, hints: Vec<Hint>) -> Self {
        self.hints = hints;
        self
    }

    /// Render this status response as an agent-readable string.
    ///
    /// Format:
    /// ```text
    /// status: ok
    /// db: ok
    /// cache: degraded (high latency)
    /// queue: down
    /// help[1]:
    ///   check queue logs
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        let mut capacity = 16 + self.items.len() * 32;
        if !self.hints.is_empty() {
            capacity += 8 + self.hints.len() * 40;
        }
        let mut out = String::with_capacity(capacity);
        out.push_str("status: ");
        out.push_str(self.overall.label());
        out.push('\n');
        for item in &self.items {
            out.push_str(&item.name);
            out.push_str(": ");
            out.push_str(item.health.label());
            if let Some(detail) = &item.detail {
                out.push_str(" (");
                out.push_str(detail);
                out.push(')');
            }
            out.push('\n');
        }
        crate::hints::append_hints(&mut out, &self.hints);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_labels() {
        assert_eq!(Health::Ok.label(), "ok");
        assert_eq!(Health::Degraded.label(), "degraded");
        assert_eq!(Health::Down.label(), "down");
    }

    #[test]
    fn health_display() {
        assert_eq!(format!("{}", Health::Ok), "ok");
        assert_eq!(format!("{}", Health::Down), "down");
    }

    #[test]
    fn basic_status_renders() {
        let resp = StatusResponse::new(Health::Ok, vec![StatusItem::new("db", Health::Ok)]);
        assert_eq!(resp.render(), "status: ok\ndb: ok\n");
    }

    #[test]
    fn degraded_with_detail_renders() {
        let resp = StatusResponse::new(
            Health::Degraded,
            vec![StatusItem::new("db", Health::Ok), StatusItem::with_detail("cache", Health::Degraded, "high latency")],
        );
        let out = resp.render();
        assert!(out.starts_with("status: degraded\n"));
        assert!(out.contains("cache: degraded (high latency)"));
    }

    #[test]
    fn status_with_hints() {
        let resp = StatusResponse::new(Health::Down, vec![StatusItem::new("queue", Health::Down)])
            .with_hints(vec![Hint::new("check queue logs")]);
        let out = resp.render();
        assert!(out.contains("status: down\n"));
        assert!(out.contains("help[1]:\n  check queue logs\n"));
    }

    #[test]
    fn empty_items_renders_only_status_line() {
        let resp = StatusResponse::new(Health::Ok, vec![]);
        assert_eq!(resp.render(), "status: ok\n");
    }

    #[test]
    fn no_hints_no_help_block() {
        let resp = StatusResponse::new(Health::Ok, vec![StatusItem::new("svc", Health::Ok)]);
        assert!(!resp.render().contains("help["));
    }
}
