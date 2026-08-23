---
michi-node: major
---

NAPI `renderToon()` rejections are now prefixed `toon_validation_failed: ` (e.g. `toon_validation_failed: row 0 has 1 values but 2 fields declared`). Previously the rejection reason was the bare validation message. Any caller matching on exact error text needs to update for the new prefix.
