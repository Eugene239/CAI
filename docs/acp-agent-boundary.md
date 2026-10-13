# ACP Agent Boundary

## Status

Accepted architecture decision. CAI uses a small Rust Agent Client Protocol (ACP) client as its primary coding-agent integration boundary. This document records the intended boundary; it does not claim that an ACP client, provider session flow, or GitHub executor workflow is implemented.

## Decision

CAI launches approved coding-agent CLIs locally inside the isolated task container and communicates with them over stdio ACP. The agent CLI owns its coding loop, provider-specific tool protocol, and native subscription authentication semantics. CAI remains the control and security plane:

- policy resolves the exact approved agent, provider, model, and credential slot before a task starts;
- the host creates an encrypted tenant-bound task envelope and keeps long-lived state outside GitHub;
- a trusted executor performs durable admission before decrypting or materializing session state;
- the task container receives only a scoped workspace, private temporary state, and the minimum per-task credential material;
- CAI enforces the task deadline, cancellation, evidence collection, diff validation, and separate ready-for-review pull-request delivery;
- the task container never receives GitHub write authority.

```text
CAI encrypted task envelope
  -> trusted executor admission
  -> isolated task container with scoped workspace and private task state
  -> CAI Rust ACP client over stdio
  -> approved native coding-agent CLI
  -> scoped workspace diff and normalized ACP evidence
  -> CAI validation and separate ready-for-review PR delivery
```

CAI must not reimplement agent loops or provider-specific tool-event protocols. It must not delegate policy, authorization, credential isolation, runner selection, replay protection, or PR delivery to an ACP agent.

## Approved integration paths

The intended paths are native subscription-capable CLIs behind ACP:

- Codex through its ACP adapter;
- Claude Code through an ACP adapter;
- Cursor through its native ACP server;
- AGY through a pinned third-party ACP bridge only after source, integrity, license, and subscription-authentication behavior are independently reviewed.

A provider path is not enabled merely because an ACP process starts. CAI must prove that its native subscription login, refresh, expiry/quota behavior, credential materialization, and cleanup satisfy the requirements below. API-key substitution, credential extraction, client impersonation, and undocumented authentication workarounds do not satisfy the proof.

## Subscription and fallback policy

ACP is a transport and lifecycle protocol, not a policy authority. CAI supplies immutable agent, provider, model, and credential-slot selections. The CAI client must fail closed when the approved selection is unavailable; it must not silently select another agent, provider, model, or subscription slot.

The first ACP proof uses one approved, supported subscription-backed agent/provider/model in a disposable task. It must verify login, refresh, expiry/quota handling, private per-task materialization, cleanup, and non-interactive execution. Each additional provider path requires its own proof before it is enabled.

## Required implementation evidence

Before an ACP path becomes a production task adapter, it must prove all of the following:

1. The agent CLI, ACP adapter or bridge, and any distribution digest are pinned and verified before execution.
2. The CAI client completes `initialize`, authentication negotiation when required, `session/new`, `session/prompt`, streamed update handling, permission handling, cancellation, and terminal result handling.
3. The agent edits only the supplied disposable scoped workspace, and CAI records normalized completion and changed-file evidence.
4. Per-task subscription session material is private to the task, absent from repository files, logs, workflow inputs, outputs, caches, artifacts, and container layers, and removed after the task.
5. CAI cancellation and wall-clock timeout terminate the agent process tree and leave no task credentials or workspace state behind.
6. The agent cannot obtain a GitHub write token, host home, SSH agent, Docker socket, or provider vault.
7. A third-party ACP bridge is source-pinned, integrity-verified, reviewed for credential handling, and separately approved before it receives any subscription session material.
