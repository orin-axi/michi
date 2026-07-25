# Philosophy

This document exists so "should this go in michi?" and "how should this work get done?" don't have to be re-derived from scratch each time — by a human contributor or an agent. It's the shared reasoning behind `CONTRIBUTING.md`'s and `CLAUDE.md`'s rules, not a replacement for either.

---

## What belongs in michi

A primitive earns a place here only if it satisfies all of the following. Treat these as gates, not vibes — each one should be answerable with a yes/no and evidence, not a feeling.

**1. Rendering, not domain logic.** It turns already-decided data into agent-readable strings/types. It doesn't decide _what_ data survives — truncation at the char/byte level is the one established exception (`truncate`, `ToonOptions::max_cell_len`), treated as a rendering-safety concern, not business logic.

This has a ceiling, and it's worth naming explicitly: when a tool's real need is _more_ sophisticated than a generic primitive could serve — a domain-specific ranking algorithm over a tool's own graph, a statistical sampling heuristic tuned to a specific data shape — that's a signal the primitive doesn't belong in michi generalized further, not a signal to make michi's primitive more complex. Michi should own the parts that are genuinely the same everywhere, and get out of the way of the parts that aren't. See "Split the decision from the signal," below, for how this plays out in practice.

**2. Zero-dep by default, no exceptions.** New dependencies are opt-in behind a Cargo feature, never required in the default build. If the _correct_ implementation of something needs a mandatory dependency, the answer isn't "make an exception" — it's either redesign around caller-supplied injection (a closure, a trait) so michi stays dependency-free, or the primitive doesn't belong here.

**3. Genuinely cross-tool — verified, not assumed.** Earns inclusion when independent, real tools would otherwise hand-roll the identical pattern. "Independent" and "real" are load-bearing: a pattern one tool uses heavily is one data point, not proof. A survey across several real agent-facing dev tools (a code-search engine, a CLI proxy, a context-compression tool, a code-graph server) found `help[]`-shaped "here's a note for the agent" hints independently reinvented, inconsistently, in every single one — that's what clearing this bar actually looks like. A pattern found in only one tool, however dramatically repeated _within_ that tool, hasn't cleared it yet — it's a candidate to watch for a second independent confirmation, not something to build yet.

**4. Doesn't duplicate an existing primitive.** Check `hints`/`recovery`/`idempotency`/`status`/`error`/`mcp` first, and verify the distinction concretely before claiming novelty. "This looks similar to X" is a reason to go read X's source, not a reason to assume it's covered — a proposed "diagnostics" primitive and the existing `idempotency::PartialSuccess` looked adjacent until actually reading `PartialSuccess`'s doc comment showed it's scoped to multi-step _mutating_ operations, not read-path degradation signals. They turned out to be genuinely distinct — but that had to be checked, not assumed either way.

**5. Deviations get tracked, not silently absorbed.** Where the final shape diverges from the original ask or an earlier draft, it goes in a Design Decisions table. The spec and the code should never quietly drift apart.

### Split the decision from the signal

The cleanest resolution to tension between "stay generic" (#1, #3) and "real tools need real sophistication" (the ceiling in #1) is architectural, not a compromise: separate the _domain-specific decision_ from the _universal signal_.

Truncation is the worked example. Deciding _which_ items survive a budget is inherently tool-specific — a personalized-PageRank ranking over a call graph, a statistical outlier-preserving sampler, a simple first-N cutoff — and none of that belongs in michi. But _reporting_ what got cut, in a consistent agent-readable shape, is exactly the kind of thing every tool needs regardless of how it decided:

```rust
/// What survived some truncation/budget decision, however it was made.
pub struct TruncationOutcome<T> {
    pub kept: Vec<T>,
    pub total_before: usize,
    pub truncated: bool,
}

impl<T> TruncationOutcome<T> {
    /// Michi's standard help[]-shaped signal for what got cut — same
    /// contract regardless of which strategy produced this outcome.
    pub fn hint(&self, guidance: impl Into<String>) -> Option<Hint> { /* ... */ }
}

// Zero-dep "best practice" strategies michi ships, for the common case:
pub fn truncate_by_count<T>(items: Vec<T>, max: usize) -> TruncationOutcome<T>;
pub fn truncate_by_bytes<T>(items: Vec<T>, max_bytes: usize, size_of: impl Fn(&T) -> usize) -> TruncationOutcome<T>;
```

A caller with a genuinely novel heuristic never has to fit their algorithm into michi's shape. They wrap whatever they decided in `TruncationOutcome` and get the same consistent hint/report format everyone else gets. Michi owns the contract, not the heuristic. When evaluating a new primitive, ask whether it can be split this way before concluding it's "too domain-specific to generalize" — often only half of it is.

---

## How work gets done

Heuristics, not hard rules — process needs contextual judgment the content checklist doesn't.

**1. TDD, always.** Failing test first, verified red for the right reason, then implementation, then green. No implementation code without a preceding red test.

**2. Independent verification before "done."** A separate pass re-checks against the spec — compiling and running the actual output, not trusting the implementer's self-report. Per-task review alone has missed real bugs that a separately-scoped validation pass, checking the actual wire output rather than re-reading the diff, caught. "Every task's reviewer said pass" and "the feature actually works" are different questions; both need answering.

**3. Verify before asserting.** Claims about external state — a registry's current contents, another project's actual spec, a dependency's real behavior — get checked directly, not recalled from memory or assumed from a plausible-sounding claim. If something can't be quickly re-verified, say so rather than asserting confidence.

**4. History is append-only.** Superseded plans/specs get a notice pointing at what replaced them, not a rewrite pretending they always said the current thing. A plan document that was correct when it was written stays as written — corrections happen in new commits, not retroactive edits to old ones.

**5. Publish/release gates are earned.** Passing a component's own tests isn't "ready to ship." A release gate (e.g. crates.io publish being conditioned on a real consumer integration, not just internal completeness) exists for a reason — treat it as binding until the condition it names is actually met, not as a formality to route around once everything else looks finished.

---

## Using this document

Content questions ("should X be a michi primitive?") go through the five-point checklist above, in order — stop at the first "no." Process questions ("is this ready to ship / merge / call done?") go through the five heuristics, weighed together, not mechanically checked off.

When the two pull in different directions — a genuinely useful pattern that's only been seen once, a process shortcut that would ship something faster — that's not a bug in the framework, it's the signal to slow down and think it through explicitly rather than pick the convenient answer.
