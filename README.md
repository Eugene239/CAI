# CAI

**CAI is an open-source, GitHub-native control plane for AI coding agents.**

It connects private repositories to approved agent providers and isolated runners, turns repository work into reviewable pull requests, and keeps authorization, policy, evidence, and audit history under the repository owner's control.

CAI is not a model provider, an IDE, or a replacement for GitHub. GitHub remains the source of truth for repositories, issues, pull requests, and review.

## Product principles

- **GitHub-native:** start work from issues, pull requests, labels, assignments, and comments.
- **OAuth and workload identity first:** no personal access tokens or permanent provider keys as the normal operating model.
- **Private-repository ready:** repositories are explicitly selected through a GitHub App installation.
- **Isolated execution:** every task runs in an ephemeral, policy-bound environment.
- **Draft PR by default:** agents may prepare work; humans retain merge and deployment authority.
- **Evidence before claims:** test output, changed files, commit SHA, agent events, and policy decisions are recorded for every run.
- **Provider-neutral:** Claude, Codex, Cursor, AGY, and future providers are adapters rather than dependencies of the control plane.
- **Dogfood in the open:** CAI is built and used through its own GitHub workflow.

## Initial scope

The first release targets this flow:

```text
GitHub issue, comment, or label
  -> CAI policy decision
  -> isolated self-hosted or GitHub-hosted runner
  -> agent execution and verification
  -> evidence-backed draft pull request
  -> human review
```

The initial release does not merge pull requests, deploy software, force-push branches, or grant cross-repository access.

## Repository map

- [Project charter](PROJECT_CHARTER.md)
- [Architecture](docs/architecture.md)
- [Identity and access model](docs/identity-and-access.md)
- [Repository onboarding](docs/repository-onboarding.md)
- [Roadmap](docs/roadmap.md)
- [Contribution guide](CONTRIBUTING.md)
- [Agent instructions](AGENTS.md)

## Status

Planning and project skeleton. No production code has been added yet.
