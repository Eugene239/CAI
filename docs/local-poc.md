# Local Deployment and First Vertical POC

## First deployment shape

The first CAI deployment runs locally in a Docker Compose stack:

```text
Docker Compose
├─ cai: Rust control-plane service
├─ cloudflared: Cloudflare Tunnel connector
└─ cai-state: named Docker volume containing SQLite state
```

SQLite is the only persistent store in the first POC. It records App installations, connected repositories, bootstrap state, run ledger records, and OIDC replay-protection data. There is no PostgreSQL, Redis, or automated backup in the first POC.

## Secrets

Docker Compose secrets are mounted read-only into the CAI service. The first POC does not put the GitHub App private key, webhook secret, OAuth credentials, or other credentials in environment variables, command lines, logs, artifacts, or SQLite.

## Public ingress

One Cloudflare Tunnel exposes only:

```text
POST /webhooks/github
POST /v1/actions/token-exchange
GET  /health
```

The initial Axum router implements `GET /health` as a stateless, credential-free JSON response. Its listener factory accepts loopback addresses only and rejects public bind addresses before opening a socket. The local server starts with:

```bash
cai serve --listen 127.0.0.1:8080
```

It writes one JSON document with the bound listener address, then serves until stopped. Webhook and OIDC exchange handlers remain unimplemented. CAI UI and administrative APIs stay private. The local CAI machine does not need a public listener, public IP address, or inbound port forwarding.

## First vertical proof

The first end-to-end proof runs against `Eugene239/CAI` itself:

1. Install the CAI GitHub App and explicitly confirm bootstrap.
2. Review and merge the bootstrap workflow pull request.
3. Add the `cai` label to a controlled issue.
4. Run the deterministic mock provider on a GitHub-hosted runner.
5. Validate the GitHub OIDC token exchange, fresh App installation token, final comment, and seven-day artifact.
6. Confirm that the run is plan-only and creates no code change or pull request.

The POC does not connect a real OAuth provider, execute on an operator-owned self-hosted runner, or create a delivery pull request.

## Current deterministic evidence primitive

The Rust core can write a self-contained, plan-only mock evidence directory named `cai-run-<run-id>` under a caller-selected output root. It contains:

```text
manifest.json
result.json
plan.md
changed-files.json
```

The manifest records the format version, run ID, and file names. The result includes the resolved policy snapshot and SHA-256. Run IDs are restricted to ASCII letters, digits, hyphens, and underscores so evidence output cannot create nested paths. The current CLI entry point is:

```bash
cai mock evidence --config /path/to/cai.yaml --repository owner/repository --output-root /path/to/evidence --run-id mock-run-001
```

It writes evidence only under the supplied output root and reports one JSON document containing the evidence directory plus the plan-only result. The future GitHub Action will upload this directory as part of the repository-native run artifact and add redacted prompt, event, and execution-log evidence.
