# Default recipe: run full CI pipeline via moon & just
default: ci

# ── Build ──────────────────────────────────────────────────────────────────
build:
    moon run :build

build-release:
    cargo build --release --workspace

build-node:
    cd packages/michi-node && pnpm build --platform

build-node-release:
    cd packages/michi-node && pnpm build --platform --release

# ── Test ───────────────────────────────────────────────────────────────────
test:
    moon run :test

test-rust:
    cargo nextest run --workspace

test-rust-all:
    cargo nextest run --workspace --all-features

test-node: build-node
    cd packages/michi-node && pnpm test

# ── Lint & Format ───────────────────────────────────────────────────────────
check: fmt-check clippy deny typos fmt-md-check

lint:
    moon run :lint

clippy:
    cargo clippy --workspace --all-features --all-targets -- -D warnings

fmt:
    moon run :format

format: fmt

fmt-check:
    moon run :format-check

fmt-md:
    CI=true pnpm exec oxfmt --ignore-path=.oxfmtignore --write "**/*.md"

fmt-md-check:
    CI=true pnpm exec oxfmt --ignore-path=.oxfmtignore --check "**/*.md"

typos:
    typos

deny:
    cargo deny check

audit:
    moon run :audit

# ── Documentation & WASM Checks ───────────────────────────────────────────
doc: doc-check

doc-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

check-wasm:
    moon run :check-wasm

# ── Examples & Benchmarks ──────────────────────────────────────────────────
examples:
    moon run :examples

example name:
    cargo run --example {{name}}

snapshots:
    cargo insta review

snapshots-accept:
    cargo insta test --accept

bench:
    cargo bench --workspace

bench-baseline:
    cargo bench --workspace -- --save-baseline main

# ── Continuous Development ──────────────────────────────────────────────────
watch:
    cargo watch -x "check --workspace --all-features"

# ── Coverage & Cleanup ─────────────────────────────────────────────────────
coverage:
    cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info

clean:
    moon clean 2>/dev/null || true
    cargo clean
    rm -f lcov.info

# ── Git Hooks & Pre-push ───────────────────────────────────────────────────
pre-commit: fmt-check

pre-push: fmt-check lint

hooks:
    @echo '#!/bin/sh\njust pre-commit' > .git/hooks/pre-commit
    @chmod +x .git/hooks/pre-commit
    @echo '#!/bin/sh\njust pre-push' > .git/hooks/pre-push
    @chmod +x .git/hooks/pre-push
    @echo "Git pre-commit and pre-push hooks installed successfully."

# ── CI Verification Pipeline ───────────────────────────────────────────────
check-all: ci

ci: fmt-check lint test audit doc-check check-wasm
