# Identity and Access Model

## Identity types

CAI keeps these identities separate:

| Identity | Purpose | Example actions |
| --- | --- | --- |
| Human user | Installs the App, confirms bootstrap, or starts work | Select repositories, merge bootstrap PR, add a CAI label |
| GitHub App | Repository-scoped integration identity | Receive webhooks, create the bootstrap branch and pull request |
| Runner workload | Executes one isolated task | Fetch a repository revision, invoke an approved provider adapter |
| Provider identity | Authorizes a model or coding-agent provider | Invoke Claude, Codex, Cursor, or another provider |
| CAI deployment | Hosts the harness and GitHub App credentials | Verify webhooks, mint credentials, route a workflow run |

## Authentication principles

- Use GitHub App installation access for repository operations.
- Use GitHub OAuth for human sign-in and account linking.
- Use OIDC or workload identity federation for runners and provider calls where supported.
- Treat all credentials as short-lived and scoped to a single responsibility.
- Never use a human personal access token as CAI's normal service credential.
- The App private key and webhook secret remain in the CAI deployment's protected secret store, never in a connected repository.

## Installation scope and credentials

The installer, not CAI, chooses `All repositories` or `Only select repositories` in GitHub App settings. CAI records the resulting installation scope and never adds repositories itself.

For a privileged repository operation, CAI generates an App JWT and mints a fresh installation access token. It does not persist an installation token as a repository credential. If a user changes the installation's repository list, CAI must mint a fresh token and re-check scope before a privileged action.

An Actions workflow obtains its installation token through a GitHub OIDC exchange with the CAI deployment. CAI validates the exact connected repository, generated workflow, default-branch reference, GitHub run ID, and installation scope. The task container never receives the token.

## Authorization order

1. Verify the GitHub webhook and deduplicate the event.
2. Resolve the GitHub App installation and repository.
3. Confirm that the repository is in the current installation scope.
4. Resolve the initiating user, when the trigger has a user.
5. Load repository policy and evaluate the requested action.
6. Resolve the workflow runner selector and provider adapter.
7. Mint only the credentials required for the approved run.
8. Record the decision before starting the workspace.

## Repository policy baseline

A newly connected repository starts with:

- explicit trigger allowlist;
- read-only planning or ready-for-review pull-request write mode;
- no merge or deployment capability;
- no cross-repository access;
- no persistent workspace;
- bounded execution time and provider quota handling;
- repository-native CI remains independent from CAI delivery.

## Revocation

Repository owners must be able to revoke a GitHub App installation, change the installation repository list, remove runner access, provider connection, or individual run. Revocation must prevent future privileged actions and be recorded in the run ledger.
