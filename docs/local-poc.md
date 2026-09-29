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

CAI UI and administrative APIs stay private. The local CAI machine does not need a public listener, public IP address, or inbound port forwarding.

## First vertical proof

The first end-to-end proof runs against `Eugene239/CAI` itself:

1. Install the CAI GitHub App and explicitly confirm bootstrap.
2. Review and merge the bootstrap workflow pull request.
3. Add the `cai` label to a controlled issue.
4. Run the deterministic mock provider on a GitHub-hosted runner.
5. Validate the GitHub OIDC token exchange, fresh App installation token, final comment, and seven-day artifact.
6. Confirm that the run is plan-only and creates no code change or pull request.

The POC does not connect a real OAuth provider, execute on an operator-owned self-hosted runner, or create a delivery pull request.
