# Local Deployment and First Vertical POC

## First deployment shape

The first CAI deployment runs locally with CAI and SQLite state. Docker Compose is an optional packaging choice, not a required public control plane:

```text
Local CAI host
├─ cai: Rust control-plane service
└─ cai-state: SQLite state
```

SQLite is the only persistent store in the first POC. SQLite records App installations, connected repositories, bootstrap state, run ledger records, and OIDC replay-protection data. There is no PostgreSQL, Redis, or automated backup in the first POC.

## Current SQLite run ledger

The Rust core opens a SQLite database through `rusqlite` with a bundled SQLite build, so the first POC does not depend on an operating-system SQLite package. The implemented `runs` table persists run ID, repository, outcome, execution mode, provider/model, token counters, quota state, and policy SHA-256. It deliberately does not persist the full policy snapshot or any credential material. Installation, repository, bootstrap, and OIDC replay records are not implemented yet.

## Secrets

Docker Compose secrets are mounted read-only into the CAI service. The first POC does not put the GitHub App private key, webhook secret, OAuth credentials, or other credentials in environment variables, command lines, logs, artifacts, or SQLite.

## Local listener and GitHub connectivity

The initial Axum router implements `GET /health` as a stateless, credential-free JSON response. Its listener factory accepts loopback addresses only and rejects public bind addresses before opening a socket. The local server starts with:

```bash
cai serve --listen 127.0.0.1:8080
```

It writes one JSON document with the bound listener address, then serves until stopped. GitHub integration and executor dispatch use outbound GitHub API calls. The local CAI machine does not need a public listener, public IP address, inbound port forwarding, or a direct network connection to an executor runner.

## First vertical proof

The completed first end-to-end proof ran against `Eugene239/CAI` itself:

1. Publish the immutable CAI release and verify its published SHA-256 sidecar.
2. Merge the manual plan-only workflow and the authorized `cai` label trigger.
3. Add the `cai` label to a controlled issue.
4. Run the deterministic mock provider on a GitHub-hosted runner.
5. Validate the retained seven-day evidence artifact.
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

It writes evidence only under the supplied output root and reports one JSON document containing the evidence directory plus the plan-only result. The current local vertical command combines the deterministic mock, evidence directory, and durable run ledger:

```bash
cai mock run --config /path/to/cai.yaml --repository owner/repository --output-root /path/to/evidence --state-db /path/to/cai.sqlite --run-id mock-run-001
```

It makes no GitHub or repository writes. The implemented GitHub Action uploads the evidence directory as part of the repository-native run artifact. Real-provider prompt, event, and execution-log handling remain future work.
