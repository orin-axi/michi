default:
    @just --list

# ── Build ──────────────────────────────────────────────────────────────────
build:
    cargo build --workspace

build-release:
    cargo build --release --workspace

build-node:
    cd packages/michi-node && pnpm build --platform

build-node-release:
    cd packages/michi-node && pnpm build --platform --release

# ── Test ───────────────────────────────────────────────────────────────────
test: test-rust test-node

test-rust:
    cargo nextest run --workspace

test-rust-all:
    cargo nextest run --workspace --all-features

test-node: build-node
    cd packages/michi-node && pnpm test

# ── Lint ───────────────────────────────────────────────────────────────────
check: fmt-check clippy deny typos

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-features -- -D warnings

deny:
    cargo deny check

typos:
    typos

# ── Snapshots ──────────────────────────────────────────────────────────────
snapshots:
    cargo insta review

# ── Benchmarks ─────────────────────────────────────────────────────────────
bench:
    cargo bench --workspace

# ── Coverage ───────────────────────────────────────────────────────────────
coverage:
    cargo llvm-cov nextest --workspace --all-features --lcov --output-path lcov.info

# ── Pre-push ───────────────────────────────────────────────────────────────
ci: check test
