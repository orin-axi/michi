//! No-op: `michi`'s napi exports don't need napi-build codegen at this
//! crate's build step (that happens in `packages/michi-node`, the actual
//! cdylib). Kept as a real build script so a future napi-feature build
//! need has a place to land without introducing a new file.

fn main() {}
