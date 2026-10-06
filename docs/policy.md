# Local Policy Configuration

## Location and reload model

Each CAI deployment uses one local `cai.yaml` file. It contains global defaults and repository-specific overrides. The file is not stored in connected repositories and is not a GitHub comment interface.

CAI validates and reloads `cai.yaml` before every new run. An invalid update blocks new runs without changing an already-started run.

CAI does not maintain a separate audit history of policy edits until a management UI exists. Every run artifact includes the exact loaded policy snapshot and its content hash.

## Repository policy

A repository policy defines:

- round-robin provider-session selection and approved models;
- allowed `cai:provider:*` and `cai:model:*` label overrides;
- enabled execution mode;
- task time limit and other local execution constraints.

Without a `cai:provider:*` or `cai:model:*` label, CAI selects the next healthy subscription slot from the repository's round-robin ring before it dispatches the task. A healthy slot is present, enabled, not leased, not quota-cooled, and not marked `auth-dead` or `unknown`. The selection, lease, resolved provider, resolved model, and non-sensitive skipped-slot reasons are recorded in evidence.

An authorized provider or model label wins over round-robin selection. CAI never silently substitutes another provider or model: an unavailable explicit override fails before dispatch, and a quota, rate-limit, or authentication failure after execution begins ends the run. A subscription quota failure does not create a pay-as-you-go fallback or charge path. Every completed or failed run records the resolved provider, model, input tokens, output tokens, total tokens, and quota outcome.

CAI does not add its own secret scan before pull-request delivery in the MVP. Repository-native secret scanning remains responsible for that protection.

## Invocation authorization

Only a GitHub user with `write`, `maintain`, or `admin` access to a connected repository may start a CAI task through the configured label or command. GitHub Actions handles normal delivery behavior; CAI does not add an additional idempotency or concurrency layer in the MVP.

## Deterministic provider proof

Before a real OAuth provider is connected, CAI uses a deterministic `mock` provider. The CLI exposes it through `cai mock plan --config /path/to/cai.yaml --repository owner/repository`. Every mock result includes the exact resolved policy snapshot and its SHA-256 hash. A successful mock plan reports fixed usage of 128 input tokens and 64 output tokens (192 total), emits no changed files, and never writes to a repository. `--quota-exhausted` deterministically reports 128 input tokens, zero output tokens, an exhausted quota state, and no plan. The command rejects a policy whose resolved provider is not `mock` rather than silently falling back.
