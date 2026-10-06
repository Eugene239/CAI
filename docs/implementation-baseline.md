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

The first behavior loads and validates local `cai.yaml`, resolves a repository default provider and model, and emits the exact policy hash and snapshot for a deterministic mock run. The deterministic mock plan reports fixed usage, can simulate quota exhaustion, emits no changed files, and makes no repository writes. It is available through:

```bash
cargo run -p cai -- policy resolve --config /path/to/cai.yaml --repository owner/repository
cargo run -p cai -- mock plan --config /path/to/cai.yaml --repository owner/repository
cargo run -p cai -- mock plan --config /path/to/cai.yaml --repository owner/repository --quota-exhausted
```

Repository overrides use canonical `owner/repository` identifiers. The first schema ignores unknown fields for forward compatibility. Invalid YAML or missing required resolved values fail the run explicitly.

## Encrypted executor primitive

The Rust core implements the transport primitive for a future trusted self-hosted executor pool. A CAI host signs a versioned task envelope with Ed25519 and encrypts its payload to the tenant executor-fleet X25519 public key using ChaCha20-Poly1305. The executor verifies the signature, tenant, and expiry before it decrypts or materializes a session file. The current replay guard is in-memory and process-local; durable replay protection remains future work.

The deterministic executor CLI test path creates a new Unix mode-`0700` task directory, materializes only validated session-file names, writes an Ed25519-signed encrypted result for the CAI host only after cleanup succeeds, and removes the task directory before it exits. Its caller must provide an executor-controlled tmpfs parent; the primitive does not itself verify the filesystem type. It is not a provider adapter and does not accept real provider credentials. GitHub dispatch delivery and a dedicated self-hosted executor service remain future work.

## GitHub Action distribution

The thin TypeScript Action wrapper downloads the Linux x86_64 Rust binary from an immutable URL and verifies its published SHA-256 checksum before execution. It invokes `cai mock run` with explicit policy, repository, evidence-root, SQLite-ledger, and run-ID inputs; then it validates the one-document JSON result and publishes the plan-only outcome, provider/model, evidence directory, and run ID as Action outputs.

The Action commit pinned by bootstrap contains the exact binary release URL and checksum. The generated `cai.yml` does not accept an arbitrary runtime binary-version input and the wrapper never fetches a moving latest release. The wrapper receives no GitHub App, OAuth/OIDC, provider, or repository-write credential in this first slice.
