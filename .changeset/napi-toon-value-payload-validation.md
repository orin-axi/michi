---
michi-node: minor
---

`renderToon`, `renderKv`, `renderStatus`, and `JsAgentResponse#items`/`#kvItems` now reject a `JsToonValue` whose declared `type` doesn't match its payload, instead of silently defaulting. Previously, `{ type: 'int' }` with `intVal` omitted rendered as `0`, and an unrecognized `type` (e.g. a typo like `'integer'`) silently rendered as `null`/dropped the value. Both cases now throw a descriptive error (e.g. `JsToonValue: type is "int" but intVal is missing`, or `JsToonValue: unknown type "integer": expected "str", "int", "float", "bool", or "null"`) before any rendering happens.
