# Roadmap

## Phase 0 — Foundation

- Project charter and architecture accepted.
- Implementation language and license selected.
- GitHub App permission matrix defined.
- Threat model and repository policy schema drafted.

## Phase 1 — GitHub and runner control plane

- GitHub App webhook verification and idempotency.
- OAuth login and repository onboarding.
- Runner-pool enrollment and health reporting.
- Durable run ledger and policy decision record.
- Read-only repository inspection run.

## Phase 2 — First provider and draft PR delivery

- Claude adapter using workload identity federation where available.
- Isolated execution contract.
- Verification evidence collector.
- Issue or comment to evidence-backed draft pull request.
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

- Autonomous merge and deployment.
- General-purpose hosted IDE.
- Broad multi-repository agent permissions.
- Shared persistent workspaces.
- Provider-specific features that require permanent credentials in repositories.
