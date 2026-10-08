# Architecture

## Purpose

CAI is an open-source, GitHub-native harness for AI coding agents. GitHub is the source of truth for repository collaboration, pull requests, checks, artifacts, and review. CAI does not introduce a custom worker queue in the MVP. GitHub Actions remains responsible for queueing, waiting for, and selecting an eligible runner.

## Implementation shape

The core harness and `cai` CLI are Rust binaries. Rust provides a portable, resource-efficient, memory-safe executable for GitHub Actions and self-hosted runners. The local control-plane HTTP boundary uses Axum on Tokio. A TypeScript wrapper may provide the GitHub Action interface where JavaScript action tooling is required.

CAI is not a sandbox runtime: GitHub Actions and the task container remain the MVP execution boundary. The Rust core orchestrates policy enforcement, provider adapters, process lifecycle, evidence collection, and GitHub-facing outputs.

## Local deployment state

The first CAI deployment is Docker Compose with a local SQLite database in a named Docker volume. SQLite records installations, connected repositories, bootstrap state, run ledger records, and OIDC replay-protection data. The first POC has no PostgreSQL, Redis, or automated backup.

## MVP task scope

The MVP write-capable provider task is intentionally **pull-request creation only**. A provider task may inspect and edit its scoped workspace, produce a diff, and return evidence needed for CAI delivery. It does not run repository build, test, lint, packaging, or deployment commands, and MVP task images need only the provider runtime, CAI runtime, Git, and basic workspace tools—not project SDK matrices such as Rust, JDK/Gradle, or Android SDK.

The CAI delivery step creates or updates a ready-for-review pull request. Repository-native GitHub CI is the only build-and-test authority for that MVP pull request, and ordinary human review is required. CAI does not wait for, parse, retry, or fix that CI. A future read-only agent review remains separate work and must not be treated as an MVP merge gate.

## Execution model

A GitHub Actions workflow in each connected repository is the execution plane. CAI installs the standard workflow through an explicit, ready-for-review bootstrap pull request; it never commits the workflow directly to a default branch.

The bootstrap process chooses the initial runner selector from repository visibility:

- public repository: GitHub-hosted runner;
- private repository: `self-hosted` runner.

The repository owner controls eligible self-hosted runners and runner groups in GitHub. If no eligible runner is available, GitHub leaves the job queued; CAI does not fall back to an operator-owned runner. A repository owner may later change the standard workflow to a different supported runner selector. For a real subscription-provider task, CAI encrypts an opaque task envelope to the owning tenant's trusted executor-fleet key and dispatches it to a generic CAI executor selector. GitHub chooses any available executor in that tenant pool; CAI does not select a physical runner or maintain a parallel queue.

The CAI Action creates one task container on the selected GitHub runner. The CAI deployment never executes task code itself. A task container receives only a per-run writable workspace and temporary directory: it must not receive host-home mounts, a Docker socket, SSH agent, host credentials, or arbitrary bind mounts. A task has a 60-minute wall-clock limit by default; repository policy may lower that limit. Resource limits and outbound-network policy are not standardized in the MVP.

```text
GitHub issue or pull request event in a connected repository
  -> CAI workflow trigger in that repository
  -> workflow policy resolution
  -> clean task container
  -> provider adapter and agent process
  -> evidence collection
  -> ready-for-review pull request and final GitHub comment
```

## Invocation

CAI recognizes two ways to start a run:

- A CAI label on an issue or pull request.
- A comment whose first non-empty line starts with `@cai-agent <instruction>`.

Labels and the exact command syntax are the invocation authority. The initiator must have GitHub `write`, `maintain`, or `admin` access. A parser must ignore mentions that do not exactly match the command form.

## Policy resolution

One local `cai.yaml` file in the CAI deployment holds global defaults and repository-specific policy. It is validated and reloaded before every new run. Repository policy resolves the task mode, provider ring, model, execution permissions, and review behavior.

- CAI selects the next healthy provider-session slot through an atomic round-robin lease unless an authorized label overrides provider or model.
- Authorized provider/model labels may override the automatic selection.
- The resolved provider, model, adapter version, task revision, policy snapshot, and policy hash are recorded in the run evidence.
- Policy is enforced by the workflow and harness, not delegated to the agent prompt.

## Provider adapters

Provider adapters implement a common CAI run contract. An adapter owns provider-specific invocation, streamed events, cancellation, authentication handoff, and normalized output. It does not own GitHub authorization or repository policy. The CAI host holds provider session state and creates a minimal encrypted per-task session bundle for a trusted executor; it never transfers plaintext provider credentials through GitHub workflow inputs, logs, outputs, caches, or artifacts. See [Provider sessions and executor pools](provider-sessions.md).

The first implementation validates this contract with a deterministic mock adapter. It does not require an external provider connection. The mock reports fixed token counters and a controlled quota-exhausted outcome. The first end-to-end proof is a plan-only run; later enabled repositories may create ready-for-review pull requests.

## GitHub outputs and evidence

A run publishes its state through the source repository's GitHub Actions checks and workflow logs. The workflow uploads one run artifact to that repository containing the effective prompt, redacted execution logs, structured events, verification output, and result metadata.

Each completed or failed run posts one final comment to its source issue or pull request. The comment states the outcome, resolved provider and model, input/output/total tokens, quota outcome, and links to the Actions run and artifact.

Artifacts and workflow logs are retained for seven days by default. Repository policy may reduce retention to one through six days.

The task container never receives a GitHub write token. After it exits, the CAI Action validates the result and performs branch, commit, and ready-for-review pull-request delivery outside the container with the per-run App installation token.

For an issue, CAI creates `cai/<issue-number>` from the current default branch. If an open CAI pull request already exists for that issue, CAI continues from its exact head and updates that same pull request. For an existing pull request, CAI may push only to a writable head branch in the same repository, never a fork, and refuses delivery if that head SHA changed after checkout.

CAI creates or updates the ready-for-review pull request and stops. Repository-native GitHub CI workflows run independently; CAI does not wait for, parse, retry, or fix them in the MVP. CAI does not merge pull requests, deploy software, force-push branches, modify protected branches, or widen its own permissions.

## Review is deferred beyond the MVP

The MVP does not run automated independent review, publish GitHub review verdicts, or request permissions for review automation. Every CAI pull request remains subject to ordinary human review.

A later review design must establish and validate all of the following before it becomes a merge gate:

- an independent GitHub identity;
- separation of implementation and reviewer provider/model/run;
- read-only reviewer execution on the exact pull-request head revision;
- GitHub review API and ruleset behavior for private repositories.

## Trust boundaries

- Issue text, pull-request text, repository files, diffs, test output, and agent output are untrusted input.
- A task container may access only its scoped workspace and temporary directory.
- Provider credentials must not be committed to repositories or included in logs or artifacts. A provider session may be materialized only in a dedicated executor's private per-task tmpfs directory.
- GitHub App installation tokens, `GITHUB_TOKEN`, OAuth access tokens, and private keys must not be passed as command-line arguments, workflow outputs, or artifact content.
- The CAI Action receives an App installation token only through a GitHub OIDC exchange with the CAI deployment. The exchange accepts only the exact connected repository, generated workflow, default branch, valid run ID, and current App installation scope.
- A reviewer must not write repository contents, push commits, create branches, or merge pull requests.
- GitHub review identity, task identity, provider identity, and runner identity are separate concepts and must remain traceable in run evidence.
