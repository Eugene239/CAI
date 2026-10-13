# OpenCode Execution Runtime

## Status

Accepted implementation decision. OpenCode is CAI's selected unified multi-provider coding-agent runtime. This document records the target CAI integration; it does not claim that an OpenCode task adapter, provider credential flow, or GitHub executor workflow is implemented.

## Decision

CAI will invoke a pinned OpenCode release through its documented non-interactive interface inside the isolated task container. OpenCode is the common agent runtime across approved provider configurations: it owns the coding-agent loop and provider/tool protocol handling, while CAI remains the control and security plane. This decision does not characterize OpenCode as the first coding agent ever used by the project.

- policy resolves the exact approved runtime, provider, model, and credential slot before a task starts;
- the host creates an encrypted tenant-bound task envelope and keeps long-lived state outside GitHub;
- a trusted executor performs durable admission before decrypting or materializing session state;
- the task container receives only a scoped workspace, private temporary state, and the minimum per-task credential material;
- CAI enforces the task deadline, cancellation, evidence collection, diff validation, and delivery of a ready-for-review pull request;
- the task container never receives GitHub write authority.

```text
CAI encrypted task envelope
  -> trusted executor admission
  -> isolated task container with scoped workspace and private task state
  -> pinned OpenCode non-interactive run
  -> scoped workspace diff and structured execution evidence
  -> CAI validation and separate ready-for-review PR delivery
```

CAI must not reimplement coding-agent loops or provider-specific tool-event protocols that OpenCode already owns. It must also not delegate CAI policy, authorization, credential isolation, runner selection, replay protection, or PR delivery to OpenCode.

## Provider and fallback policy

OpenCode is an execution runtime, not a policy authority. CAI supplies an immutable task configuration that pins the approved provider and model chosen before dispatch. The OpenCode configuration and task adapter must fail closed when that provider or model is unavailable; they must not silently select another provider, model, subscription slot, or credential.

The initial CAI integration proof uses one approved API-key-backed provider/model in a disposable task. It validates the unified runtime boundary before enabling additional approved provider configurations. It does not use unofficial subscription plugins, credential-sharing schemes, or a provider OAuth/session flow. A native OAuth or subscription session integration requires a separate documented provider-specific decision and proof of its supported authentication model.

## Required spike evidence

The first OpenCode executor spike must prove all of the following before it becomes a production task adapter:

1. The exact OpenCode version and distribution digest are pinned and verified before execution.
2. A non-interactive invocation edits only the supplied disposable scoped workspace.
3. The adapter records structured completion and changed-file evidence, and rejects changes outside the scoped workspace.
4. Per-task API-key or session material is private to the task, absent from repository files, logs, workflow inputs, outputs, caches, artifacts, and container layers, and removed after the task.
5. CAI cancellation and wall-clock timeout terminate the OpenCode process tree and leave no task credentials or workspace state behind.
6. The runtime cannot obtain a GitHub write token, host home, SSH agent, Docker socket, or provider vault.

ACP remains a future compatibility boundary, not the prerequisite for the first OpenCode proof. CAI may add a local stdio ACP client only after the OpenCode non-interactive contract is proven and ACP offers a stable advantage for approved runtimes.
