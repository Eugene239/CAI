# CAI Project Charter

## Mission

Build an open-source control plane that lets teams safely delegate bounded software-engineering work to AI coding agents in GitHub repositories.

## Problem

Coding agents can edit code, run tests, and open pull requests, but teams need a reliable outer loop around them: repository-scoped authorization, isolated execution, clear policy, repeatable onboarding, verifiable output, and human control over irreversible actions.

## Product statement

CAI receives an authorized GitHub event, evaluates repository policy, dispatches an approved agent into an isolated runner, captures the run ledger, and returns a ready-for-review pull request with evidence.

## Users

- Repository owners connecting private repositories.
- Developers delegating bounded work from GitHub.
- Maintainers reviewing agent-created pull requests.
- Operators administering runners, provider connections, and repository policies.

## Non-goals for the first release

- Building a foundation model or a general-purpose agent framework.
- Replacing GitHub Issues, Pull Requests, Actions, or code review.
- Providing an IDE, hosted source-code forge, or autonomous merge/deploy system.
- Requiring a proprietary agent provider.
- Granting broad permanent credentials to repositories, runners, or agents.

## Success criteria

1. A private repository can be connected through a GitHub App installation in minutes.
2. An authorized issue or pull-request comment can create an isolated agent run.
3. Every run has an immutable ledger containing its initiator, policy decision, repository revision, provider, runner, execution events, evidence, and final outcome.
4. A successful write-capable run produces a ready-for-review pull request only.
5. Repository policy can deny a run before any workspace or provider access is created.
6. A self-hosted runner can be enrolled, health-checked, and restricted to named repository pools.

## Operating principles

- Default deny for repositories, actions, tools, and egress.
- Short-lived credentials and explicit trust boundaries.
- GitHub is the collaboration surface and source of truth.
- Human review is a product guarantee, not an agent prompt.
- Provider adapters are replaceable.
- Documentation, code, issues, and pull requests are written in English.

## Implementation direction

CAI's core harness and CLI will be implemented in Rust. Rust is selected for a portable, resource-efficient, memory-safe executable suitable for GitHub Actions and self-hosted runners. The GitHub Action wrapper will use TypeScript where an action integration requires JavaScript tooling.

This does not make CAI a sandbox runtime: GitHub Actions and the task container remain the MVP execution boundary.

## Open decisions

- Deployment footprint.
- Durable queue and ledger storage beyond the Actions-first MVP.
- First supported runner implementation.
- First provider adapters after Claude.
- Open-source license.
