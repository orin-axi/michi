//! Runs napi-build's codegen setup so the compiled cdylib gets the
//! platform-specific NAPI linker flags it needs.

extern crate napi_build;

fn main() {
    napi_build::setup();
}
