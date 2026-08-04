//! Structured DomainError rendering and GitHub Actions annotation example.
//! Run with: `cargo run --example domain_errors`

use michi::error::{DomainError, ErrorCode};
use michi::recovery::RecoveryHint;

fn main() {
    let err = DomainError::new(ErrorCode::NotFound, "Package '@orin-axi/michi' not found in workspace")
        .hint("Run `just check-all` to inspect workspace packages")
        .recovery(RecoveryHint::new("list_packages"));

    println!("=== Agent Error Block ===");
    println!("{}", err.render());

    println!("=== GitHub Workflow Annotation ===");
    println!("{}", err.render_github_annotation());
}
