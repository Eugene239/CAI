# Architecture

## Purpose

CAI is an open-source, GitHub-native harness for AI coding agents. GitHub is the source of truth for repository collaboration, pull requests, checks, artifacts, and review. CAI does not introduce a custom worker, queue, or persistent execution service in the MVP.

## Execution model

A GitHub Actions workflow is the execution plane. It may run on GitHub-hosted runners or operator-managed self-hosted runners. The operator chooses the host platform and provisioning method.

Every task executes in one container with a clean workspace and scoped mounts. A task container has a 60-minute wall-clock limit by default; repository policy may lower that limit. Resource limits and outbound-network policy are not standardized in the MVP.

```text
GitHub issue or pull request event
  -> CAI workflow trigger
  -> workflow policy resolution
  -> clean task container
  -> provider adapter and agent process
  -> verification and evidence collection
  -> GitHub check, artifact, comment, or draft pull request
```

## Invocation

CAI recognizes two ways to start a run:

- A CAI label on an issue or pull request.
- A comment whose first non-empty line starts with `@cai-agent <instruction>`.

Labels and the exact command syntax are the invocation authority. The MVP does not add a separate CAI actor allowlist. A parser must ignore mentions that do not exactly match the command form.

## Policy resolution

Repository policy resolves the task mode, provider, model, execution permissions, and review behavior.

- CAI selects a provider and model automatically from repository policy and required capabilities.
- Authorized provider/model labels may override the automatic selection.
- The resolved provider, model, adapter version, and task revision are recorded in the run evidence.
- Policy is enforced by the workflow and harness, not delegated to the agent prompt.

## Provider adapters

Provider adapters implement a common CAI run contract. An adapter owns provider-specific invocation, streamed events, cancellation, authentication handoff, and normalized output. It does not own GitHub authorization or repository policy.

The first implementation validates this contract with a deterministic mock adapter. It does not require an external provider connection. The first end-to-end proof is a plan-only run: a `cai` label starts the mock adapter, uploads evidence, and makes no repository changes.

## GitHub outputs and evidence

A run publishes its state through GitHub checks and workflow logs. The workflow uploads one run artifact containing the effective prompt, raw execution logs, structured events, verification output, and result metadata.

Artifacts and workflow logs are retained for seven days by default. Repository policy may reduce retention to one through six days.

A write-capable implementation run may create or update a draft pull request. CAI does not merge pull requests, deploy software, force-push branches, modify protected branches, or widen its own permissions.

## Optional independent review

Independent review is configured per repository and supports two trigger modes:

- `manual` — the default; a review command starts the review.
- `on_pr_update` — review runs for every new pull-request head revision.

The default review result is a GitHub `COMMENT` plus a `CAI / independent-review` status check. A repository may opt into formal `APPROVE` or `REQUEST_CHANGES` behavior.

A formal gate requires a reviewer run that is independent from the implementation run:

```text
implementation provider != reviewer provider
implementation model != reviewer model
implementation run != reviewer run
reviewer execution mode == read-only
```

The reviewer evaluates the exact pull-request head revision, the diff, repository review rules, and verification evidence. If a new commit changes the pull request, the previous verdict does not apply to the new revision.

## Trust boundaries

- Issue text, pull-request text, repository files, diffs, test output, and agent output are untrusted input.
- A task container may access only its scoped workspace and mounts.
- Provider credentials must not be committed to repositories or included in artifacts.
- A reviewer must not write repository contents, push commits, create branches, or merge pull requests.
- GitHub review identity, task identity, provider identity, and runner identity are separate concepts and must remain traceable in run evidence.
