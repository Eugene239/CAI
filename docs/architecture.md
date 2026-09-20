# Architecture

## Intended shape

CAI is a control plane, not an in-workspace coding agent. It decides whether work may begin, creates a bounded execution context, records evidence, and delivers results through GitHub.

```text
GitHub App webhook
  -> event verifier and idempotency gate
  -> policy engine
  -> durable run ledger and queue
  -> runner dispatcher
  -> isolated runner workspace
  -> provider adapter and agent process
  -> verification collector
  -> GitHub draft pull request and status updates
```

## Components

### GitHub integration

Receives verified GitHub App events, resolves the installation and repository, posts status updates, and creates draft pull requests. It must not use a human personal access token for normal operation.

### Policy engine

Evaluates the repository policy before dispatch. It decides whether an event is eligible, which runner pool and provider adapter are allowed, whether planning approval is required, which tools and network destinations are allowed, and whether the run is read-only or write-capable.

### Run ledger

Stores the immutable lifecycle of a run: event identity, initiator, repository revision, policy decision, credentials issued, runner identity, provider adapter, tool events, test evidence, cost or usage metadata, and terminal outcome.

### Dispatcher and runner pools

Dispatches approved runs only to eligible pools. A runner pool is explicitly bound to repositories and has a declared trust level, platform, capacity, and egress policy. Each task receives an isolated workspace.

### Provider adapters

Translate CAI's internal run contract into a provider-specific agent invocation. Adapters are responsible for capability discovery, streamed events, cancellation, provider authentication handoff, and normalized results. They must not own repository authorization.

### Verification collector

Runs or captures the repository-defined checks, packages their output as evidence, and makes the final outcome available to GitHub and the run ledger.

## Trust boundaries

- The GitHub event is untrusted until webhook verification and installation resolution succeed.
- Issue text, pull-request text, repository files, test output, and agent output are untrusted input.
- An agent can propose actions but cannot bypass control-plane policy.
- A runner can access only the workspace, credentials, network destinations, and repository granted to its run.
- Provider credentials stay in a broker or provider-approved workload-identity flow, never in repository files.

## Default outcome

A successful write-capable run creates or updates a draft pull request. Merge, deployment, force-push, protected-branch writes, and privilege escalation are outside the initial architecture.
