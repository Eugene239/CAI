# Identity and Access Model

## Identity types

CAI keeps these identities separate:

| Identity | Purpose | Example actions |
| --- | --- | --- |
| Human user | Initiates or approves work | Connect a repository, start a run, approve a plan |
| GitHub App | Repository-scoped integration identity | Receive webhooks, post status, create a draft pull request |
| Runner workload | Executes one isolated task | Fetch a repository revision, invoke an approved provider adapter |
| Provider identity | Authorizes a model or coding-agent provider | Invoke Claude, Codex, Cursor, or another provider |
| CAI service identity | Operates the control plane | Queue dispatch, ledger writes, policy evaluation |

## Authentication principles

- Use GitHub App installation access for repository operations.
- Use GitHub OAuth for human sign-in and account linking.
- Use OIDC or workload identity federation for runners and provider calls where supported.
- Treat all credentials as short-lived and scoped to a single responsibility.
- Never use a human personal access token as CAI's normal service credential.

## Authorization order

1. Verify the GitHub webhook and deduplicate the event.
2. Resolve the GitHub App installation and repository.
3. Resolve the initiating user, when the trigger has a user.
4. Load repository policy and evaluate the requested action.
5. Select an eligible runner pool and provider adapter.
6. Mint only the credentials required for the approved run.
7. Record the decision before starting the workspace.

## Repository policy baseline

A newly connected repository starts with:

- explicit trigger allowlist;
- read-only planning or draft-PR write mode;
- no merge or deployment capability;
- no cross-repository access;
- no persistent workspace;
- bounded execution time and provider budget;
- repository-defined verification commands recorded before execution.

## Revocation

Repository owners must be able to revoke a GitHub App installation, runner-pool assignment, provider connection, or individual run. Revocation must prevent future privileged actions and be recorded in the run ledger.
