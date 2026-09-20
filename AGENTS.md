# CAI Agent Instructions

## Scope

CAI is an open-source GitHub-native control plane for AI coding agents. It coordinates authorization, policy, runner dispatch, provider adapters, evidence, and pull-request delivery. It does not replace GitHub or make merge and deployment decisions.

## Language

All repository-facing artifacts must be written in English:

- source code and comments;
- documentation and architecture records;
- issues, pull requests, commit messages, and release notes;
- user-facing CLI and UI text.

Discussion outside the repository may use another language.

## Project invariants

1. GitHub remains the source of truth for repository collaboration.
2. A GitHub App and OAuth/OIDC flows are preferred over personal access tokens and long-lived secrets.
3. Repository access is explicit, scoped, and revocable.
4. Every agent run has a durable, append-only evidence record.
5. Agents create draft pull requests by default; they must not merge, deploy, force-push, modify protected branches, or widen permissions.
6. Runners are isolated by task and restricted to their approved repository pool.
7. Provider adapters must not leak credentials into workspaces, logs, prompts, commits, or pull-request text.
8. Policy is enforced by the control plane, never delegated solely to an agent prompt.

## Change discipline

- Read the applicable documentation before changing architecture, authorization, runner behavior, or provider integrations.
- Preserve the distinction between user identity, GitHub App identity, workload identity, and provider identity.
- Prefer small, reviewable changes with an explicit acceptance criterion.
- Update documentation in the same change when behavior, policy, or public interfaces change.
- Do not add provider credentials, sample tokens, real repository data, or sensitive execution output to this repository.
- Do not introduce a new runtime dependency or service without documenting its role, operational cost, and failure mode.

## Required evidence for implementation changes

A pull request that changes behavior must state:

- the affected workflow and policy boundary;
- the commands or checks run and their result;
- any test coverage added or explicitly not applicable;
- any security, migration, or operational consequence.

## Current phase

This repository is in planning. Do not add production code until the project charter, architecture, identity model, and initial implementation language are accepted.
