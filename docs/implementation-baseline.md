# Implementation Baseline

## Repository layout

The first executable CAI implementation uses one Cargo workspace and one Rust binary crate:

```text
Cargo.toml
crates/
  cai/                 Rust CLI and harness binary
actions/
  cai/                 thin TypeScript GitHub Action wrapper
```

Rust owns all CAI behavior. The TypeScript wrapper is limited to GitHub Actions inputs, contexts, output mapping, binary acquisition, and process invocation. Rust crates are split only after a tested boundary appears.

## Toolchain and tests

- Rust tests use `cargo test`.
- The Action wrapper uses npm with a committed `package-lock.json`.
- Wrapper tests use the built-in Node test runner: `node --test`.
- New production behavior follows RED → GREEN → REFACTOR. A test must fail for the missing behavior before implementation begins.

## CLI contract

The Rust binary writes exactly one JSON result document to standard output. Diagnostics go to standard error. A non-zero exit code signals failure.

## First TDD slice

The first behavior loads and validates local `cai.yaml`, resolves a repository default provider and model, and emits the exact policy hash and snapshot for a deterministic mock run. It is available through:

```bash
cargo run -p cai -- policy resolve --config /path/to/cai.yaml --repository owner/repository
```

Repository overrides use canonical `owner/repository` identifiers. The first schema ignores unknown fields for forward compatibility. Invalid YAML or missing required resolved values fail the run explicitly.

## GitHub Action distribution

The thin TypeScript Action wrapper downloads a platform-specific Rust binary from an immutable GitHub Release asset and verifies its published SHA-256 checksum.

The Action commit pinned by bootstrap contains the exact binary release URL and checksum. The generated `cai.yml` does not accept an arbitrary runtime binary-version input and the wrapper never fetches a moving latest release.
