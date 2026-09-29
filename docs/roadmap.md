# Roadmap

## Phase 0 — Foundation

- Project charter and architecture accepted.
- Rust core and TypeScript GitHub Action wrapper selected.
- GitHub App permission matrix and ready-for-review workflow bootstrap defined.
- Threat model and repository policy schema drafted.

## Phase 1 — GitHub and runner control plane

- GitHub App webhook verification and idempotency.
- OAuth login and repository onboarding.
- Docker Compose deployment for CAI and Cloudflare Tunnel ingress.
- SQLite state in a named Docker volume; no PostgreSQL, Redis, or automated backup in the first POC.
- Docker Compose secrets for App and OAuth credentials.
- Runner-pool enrollment and health reporting.
- Durable run ledger and policy decision record.
- Read-only repository inspection run.

## First vertical POC

- Bootstrap `Eugene239/CAI` through the CAI App.
- Run a plan-only deterministic mock task on a GitHub-hosted runner.
- Prove OIDC token exchange, final usage comment, and run artifact.

## Phase 2 — First provider and pull-request delivery

- OAuth adapter contract and deterministic mock usage reporting.
- Isolated execution contract.
- Evidence collector and final GitHub run comment.
- Issue or comment to evidence-backed ready-for-review pull request.
- GitHub status and cancellation flow.

## Phase 3 — Operator surfaces

- `cai` CLI for login, repository onboarding, provider connection, runner enrollment, run inspection, and cancellation.
- Minimal web control panel for organizations, repositories, runners, policies, provider connections, and run history.
- Streamable HTTP MCP server with OAuth for CAI tools and run inspection.

## Phase 4 — Additional provider adapters

- Codex local/self-hosted adapter.
- Cursor external-provider adapter where its supported authentication model fits CAI policy.
- Additional agents through a stable adapter contract.
- ACP compatibility assessment for local agent sessions.

## Explicitly deferred

- Automated independent review and use of the GitHub review API as a merge gate.
- Autonomous merge and deployment.
- General-purpose hosted IDE.
- Broad multi-repository agent permissions.
- Shared persistent workspaces.
- Provider-specific features that require permanent credentials in repositories.
