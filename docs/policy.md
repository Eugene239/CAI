# Local Policy Configuration

## Location and reload model

Each CAI deployment uses one local `cai.yaml` file. It contains global defaults and repository-specific overrides. The file is not stored in connected repositories and is not a GitHub comment interface.

CAI validates and reloads `cai.yaml` before every new run. An invalid update blocks new runs without changing an already-started run.

CAI does not maintain a separate audit history of policy edits until a management UI exists. Every run artifact includes the exact loaded policy snapshot and its content hash.

## Repository policy

A repository policy defines:

- default provider and model;
- allowed `cai:provider:*` and `cai:model:*` label overrides;
- enabled execution mode;
- task time limit and other local execution constraints.

New repositories may create ready-for-review pull requests immediately. CAI does not set a monetary spend budget: provider access is OAuth-based. Every completed or failed run records the resolved provider, model, input tokens, output tokens, total tokens, and quota outcome.

CAI never silently falls back to another provider or model. If a provider reports a quota or rate limit, CAI stops the run and records the error. A human may start a new run with an explicit allowed override.

CAI does not add its own secret scan before pull-request delivery in the MVP. Repository-native secret scanning remains responsible for that protection.

## Invocation authorization

Only a GitHub user with `write`, `maintain`, or `admin` access to a connected repository may start a CAI task through the configured label or command. GitHub Actions handles normal delivery behavior; CAI does not add an additional idempotency or concurrency layer in the MVP.

## Deterministic provider proof

Before a real OAuth provider is connected, CAI uses a deterministic `mock` provider. The mock reports fixed provider, model, and token counters and supports a controlled quota-exhausted scenario. This validates usage reporting and failure handling without a provider connection.
