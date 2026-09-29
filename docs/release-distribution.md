# Release Distribution

## Initial scope

CAI publishes one verified Rust binary asset for the initial GitHub-hosted-runner POC:

```text
cai-x86_64-unknown-linux-gnu.tar.gz
cai-x86_64-unknown-linux-gnu.tar.gz.sha256
```

The release workflow runs only when a `v*` Git tag is pushed. It builds the `cai` binary from the exact tagged source with the stable Rust toolchain and `Cargo.lock`, packages the Linux x86_64 binary, generates its SHA-256 checksum, and creates a GitHub Release with both files.

The TypeScript GitHub Action wrapper must download the exact asset selected by its pinned Action revision and verify the published checksum before invoking the binary. It must not fetch a moving `latest` release or accept a workflow-controlled binary version.

## Deferred platforms

Linux ARM64 assets are required before CAI supports ARM64 self-hosted runners. They are deliberately excluded from the initial GitHub-hosted-runner proof and must be added through a separate reviewed release-workflow change.

## Release authority

The workflow has `contents: write` only because GitHub Release creation requires it. It does not receive provider credentials, GitHub App keys, OAuth data, or repository-scoped installation tokens. A release occurs only from an explicit tag push; ordinary pull requests and pushes to `main` cannot trigger it.
