# Repository Onboarding

## Goal

Connect a private GitHub repository to CAI quickly without adding personal access tokens, permanent provider keys, or bespoke workflow files.

## Target CLI flow

```text
cai repo connect owner/private-repository
```

The command should:

1. Open the GitHub App installation flow when the repository is not yet authorized.
2. Confirm that the organization owner selected the repository explicitly.
3. Discover repository instructions and verification configuration in read-only mode.
4. Select an approved runner pool.
5. Create a repository record with the baseline policy.
6. Confirm the supported GitHub triggers and default delivery mode.

## Target UI flow

```text
Connect repository
  -> GitHub repository picker
  -> runner-pool selection
  -> policy summary
  -> connect
```

## Default behavior

- The repository is private or public according to its existing GitHub visibility.
- CAI sees only repositories selected in the GitHub App installation.
- The first runnable mode is issue/comment to plan or draft pull request.
- No merge, deployment, force-push, protected-branch write, or cross-repository access is enabled.
- Provider authentication is configured at the CAI workspace or runner-pool level and is never copied into repository secrets.

## Preflight record

Onboarding records:

- GitHub owner, repository, installation, and default branch;
- discovered instructions such as `AGENTS.md`, `CLAUDE.md`, and `CONTRIBUTING.md`;
- discovered build, lint, and test entry points;
- allowed runner pool and provider adapters;
- initial policy version and onboarding principal.

Preflight discovery does not start an agent run or modify repository content.
