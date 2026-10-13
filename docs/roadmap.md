# Roadmap

## Phase 0 — Foundation

- Project charter and architecture accepted.
- Rust core and TypeScript GitHub Action wrapper selected.
- GitHub App permission matrix and ready-for-review workflow bootstrap defined.
- Threat model and repository policy schema drafted.
- Apache-2.0 license, Cargo workspace baseline, Action-wrapper contract, and TDD baseline selected.

## Phase 1 — GitHub and runner control plane

- GitHub App webhook verification and idempotency.
- OAuth login and repository onboarding.
- GitHub App installation lifecycle or self-hosted App manifest onboarding.
- SQLite state in a named Docker volume; no PostgreSQL, Redis, or automated backup in the first POC.
- Tenant-scoped executor-pool enrollment and health reporting.
- Durable run ledger and policy decision record.
- Read-only repository inspection run.

## First vertical POC

- Bootstrap `Eugene239/CAI` through the CAI App.
- Run a plan-only deterministic mock task on a GitHub-hosted runner.
- Prove OIDC token exchange, final usage comment, and run artifact.

## Phase 2 — ACP agent boundary and provider sessions

- Implement the small Rust ACP client boundary: initialize, authentication negotiation, session creation, prompt, streamed updates, permission response, cancellation, and terminal result handling over stdio.
- Initial Codex ACP spike with a supported ChatGPT subscription path in a disposable scoped workspace; pin and verify the CLI and ACP adapter, and collect structured completion and changed-file evidence.
- Add Claude Code and Cursor only after their own supported subscription login, refresh, expiry/quota, materialization, cleanup, and non-interactive execution proofs.
- Add AGY only through a pinned, integrity-verified, independently reviewed third-party ACP bridge; do not give an unreviewed bridge subscription material.
- ACP task-adapter containment proof: private per-task credential state, cleanup, process-tree cancellation, deadline enforcement, no GitHub write authority in the task container, and no silent agent/provider/model/slot fallback.
- Subscription-session adapter contract and deterministic mock usage reporting after a provider-specific supported authentication flow is accepted.
- Host-held provider session vault, atomic session leases, and round-robin slot selection.
- Signed encrypted task envelopes dispatched through GitHub Actions to any eligible executor in a tenant-scoped pool.
- Per-task tmpfs session materialization, encrypted result return, and validated refresh-state handback.
- Explicit provider/model overrides with no silent fallback; quota and authentication failures fail closed.
- Isolated execution contract: the MVP provider task only creates a ready-for-review pull request and does not run repository build/test/lint commands or require project SDK matrices.
- Evidence collector and final GitHub run comment.
- Issue or comment to evidence-backed ready-for-review pull request.
- GitHub status and cancellation flow.

## Phase 3 — Operator surfaces

- `cai` CLI for login, repository onboarding, provider connection, runner enrollment, run inspection, and cancellation.
- Minimal web control panel for organizations, repositories, runners, policies, provider connections, and run history.
- Streamable HTTP MCP server with OAuth for CAI tools and run inspection.

## Phase 4 — Additional ACP agents and provider paths

- Additional approved native agents through the stable ACP boundary.
- Non-ACP native adapters only where a provider has no supported ACP implementation and a separately approved structured non-interactive contract.

## Explicitly deferred

- Automated independent review and use of the GitHub review API as a merge gate.
- Autonomous merge and deployment.
- General-purpose hosted IDE.
- Broad multi-repository agent permissions.
- Shared persistent workspaces.
- Provider-specific features that require permanent credentials in repositories.
