# Provider Sessions and Executor Pools

## Status

This is an accepted architecture plan. It is not implemented. The current vertical proof remains the deterministic `mock` provider and does not materialize provider credentials.

## Decision

CAI uses subscription-backed native provider sessions rather than pay-as-you-go API keys. A CAI host owns its provider-session vault and control-plane policy. GitHub Actions remains the queue and execution scheduler.

A CAI host does not select a physical worker. It submits an encrypted task envelope to GitHub, and GitHub schedules the job onto any available runner in that host's trusted executor pool.

```text
CAI host                       GitHub Actions                 executor pool
--------                       --------------                 -------------
select subscription slot  ->  repository_dispatch       ->    any free runner
create signed envelope         queues a CAI task               verifies and decrypts it
encrypt to fleet key           chooses an eligible runner      materializes in tmpfs
poll encrypted result     <-  retained result artifact   <-    runs provider agent
```

This avoids a direct CAI-host-to-runner connection and does not duplicate GitHub's runner scheduling, queueing, or availability behavior.

## Tenant isolation

Every CAI owner is an independent tenant with separate:

- provider-session vault and provider identity;
- signing keys;
- executor-fleet encryption key and key version;
- trusted GitHub runner group and repository installation scope.

A task envelope includes the tenant and executor-key identifiers, an expiry, a one-time task identifier, the allowed repository, and the expected workflow identity. An executor rejects an envelope whose signature, tenant, key identifier, expiry, or workflow binding is invalid. An executor pool from another tenant cannot decrypt or accept it.

A shared physical machine is out of scope for the first implementation. A machine that serves more than one tenant must use separately isolated executor profiles and keys.

## Provider selection and session leasing

Explicit `cai:provider:*` and `cai:model:*` labels are overrides. Without either label, CAI selects the next healthy subscription slot in a round-robin ring before it dispatches the task.

A healthy slot is present, enabled, not exclusively leased, not quota-cooled, and not marked `auth-dead` or `unknown`. Selection and lease acquisition are atomic and durable. Evidence records the resolved provider, model, selected slot identifier, and non-sensitive reasons that slots were skipped before execution begins.

- An explicit provider or model override wins over round-robin selection.
- An unavailable explicit override fails before dispatch. CAI does not substitute another provider or model.
- Quota exhaustion or authentication failure after execution starts ends that run. CAI does not retry it on another slot.
- A subscription quota failure does not create a pay-as-you-go charge path.

A future automated review is a separate, read-only task. It must use a different provider family from implementation, run on the exact pull-request head SHA, and never receive a GitHub write token.

## Task envelope

The CAI host creates a signed task manifest and encrypts the payload to the tenant's executor-fleet public key. The payload contains only the minimum task context required by the selected provider adapter:

- task ID, expiry, repository, revision, provider, model, and allowed execution mode;
- prompt and policy snapshot reference;
- a minimal provider-session copy, when a real provider requires one;
- CAI host result-encryption public key;
- lease identifier and session-state version for a provider that may refresh its local state.

The host submits ciphertext through `repository_dispatch` when it fits GitHub's event-payload limits. Larger ciphertext is stored in a short-lived encrypted object and the dispatch event contains only its immutable reference, digest, expiry, and signed metadata. Plaintext provider credentials, prompts, and session files must never enter workflow inputs, repository files, logs, outputs, caches, or unencrypted artifacts.

The workflow uses a generic trusted selector such as `runs-on: [self-hosted, cai-executor]`. GitHub selects and waits for an available runner in that pool. CAI does not target a runner ID or operate its own worker queue.

## Executor requirements

Each executor machine runs a local privileged `cai-executor` service alongside its GitHub self-hosted runner. The service holds the tenant executor decryption capability outside the workspace and accepts only valid CAI-signed envelopes from the approved workflow.

For every task it:

1. verifies manifest signature, digest, tenant, expiry, replay state, repository, and workflow binding;
2. decrypts the envelope;
3. creates a private per-task directory on tmpfs;
4. materializes only the selected provider's minimal session state;
5. starts the agent in an isolated task container with scoped mounts;
6. removes session material and temporary state after the task;
7. encrypts the result to the CAI host's result key and uploads only ciphertext as a GitHub artifact.

The executor must never expose its fleet key, provider files, host home, SSH agent, Docker socket, or arbitrary host paths to the workflow workspace or task container.

A provider agent that receives a session is necessarily inside that session's trust boundary for the duration of the task. Encryption in transit does not make an untrusted executor safe. Therefore executor pools must be dedicated to CAI tasks and must not accept arbitrary repository workflows.

## Refreshable session state

Some native CLI sessions may update a refresh token or auth file. The executor returns an updated state only as an encrypted result addressed to the CAI host and bound to the active lease. The CAI host validates the lease and state version, validates the returned format, and atomically replaces its vault copy.

If a task crashes or its lease expires, the CAI host preserves the prior vault state and marks the slot for a health check. It does not automatically reuse the task on another subscription slot.

## GitHub integration modes

CAI supports two future integration modes with the same executor protocol:

- **Self-hosted integration:** the owner creates and installs a GitHub App for their CAI host. The App private key stays on that host, which mints installation tokens and dispatches its own encrypted tasks.
- **Managed integration:** a shared CAI GitHub App maps installations to tenants and relays opaque encrypted dispatches. The integration service never receives provider sessions, executor private keys, or plaintext task envelopes.

GitHub App installation and GitHub OAuth sign-in are distinct. Installation authorizes a repository integration; OAuth sign-in is only for human account linking and management. A GitHub App is not a provider credential vault and is not required for the mock proof.

## Provider credentials

The operating model does not use `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CODEX_API_KEY`, `XAI_API_KEY`, or `GEMINI_API_KEY` as the normal path. Provider-specific session handling must be implemented only after validating the provider's supported native authentication flow and applicable terms.

Provider session material remains encrypted at rest on the CAI host and exists on an executor only in its private, per-task tmpfs scope. It is never stored in GitHub secrets as a shared provider credential.
