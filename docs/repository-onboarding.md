# Repository Onboarding

## Goal

Connect a GitHub repository to CAI without personal access tokens or permanent provider keys. CAI installs one standard workflow through a ready-for-review bootstrap pull request.

## Installation and bootstrap flow

```text
Install CAI GitHub App
  -> choose All repositories or Only select repositories in GitHub
  -> explicitly confirm bootstrap for a selected repository
  -> CAI opens a ready-for-review bootstrap pull request
  -> owner merges the pull request
  -> repository can accept CAI runs
```

CAI must:

1. Treat every repository selected by the installer as connected, without expanding the installation scope itself.
2. Require an explicit bootstrap confirmation before writing to a selected repository.
3. Open a normal, ready-for-review pull request on a dedicated bootstrap branch; never write the workflow directly to the default branch.
4. Discover repository instructions and verification configuration in read-only mode.
5. Select `ubuntu-latest` for a public repository and `self-hosted` for a private repository in the initial workflow.
6. Record the installation, repository visibility, bootstrap pull request, and initial policy.

## Default behavior

- CAI sees only repositories selected in the GitHub App installation.
- The first runnable mode is a `cai` label to a mock plan-only run.
- No merge, deployment, force-push, protected-branch write, or cross-repository access is enabled.
- Provider authentication is configured at the CAI deployment or runner level and is never copied into repository secrets.
- For a private repository, the owner configures an eligible self-hosted runner or runner group in GitHub. CAI never falls back to an operator-owned runner.

## Preflight record

Onboarding records:

- GitHub owner, repository, installation, visibility, and default branch;
- bootstrap pull request and pinned CAI Action revision;
- discovered instructions such as `AGENTS.md`, `CLAUDE.md`, and `CONTRIBUTING.md`;
- discovered build, lint, and test entry points;
- generated runner selector and provider adapters;
- initial policy version and onboarding principal.

Preflight discovery does not start an agent run. Bootstrap only changes repository content through its explicit pull request.
