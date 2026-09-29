# GitHub App and Workflow Bootstrap

## Purpose

The CAI GitHub App connects repositories, receives GitHub events, mints short-lived installation credentials, and opens an explicit bootstrap pull request. It is publicly installable, but it never expands its own installation scope or commits directly to a repository's default branch.

## Installation scope

The installer chooses `All repositories` or `Only select repositories` in GitHub. Every selected repository is connected to CAI immediately. The installer later adds or removes repositories in GitHub App settings; CAI only observes the resulting scope.

## MVP permissions

| Permission | Access | MVP use |
| --- | --- | --- |
| Metadata | Read | Identify installations and repositories. |
| Contents | Write | Create the bootstrap branch and commit the standard workflow. |
| Workflows | Write | Add or update the workflow under `.github/workflows/`. |
| Pull requests | Write | Open the normal, ready-for-review bootstrap pull request. |
| Issues | Write | Post bootstrap and final run comments. |

The MVP does not request Actions, Checks, Administration, or review-specific permissions. Users cancel runs through the native GitHub Actions UI. GitHub Actions creates its own workflow status and check output.

## Bootstrap contract

Bootstrap requires an explicit confirmation for a repository already selected in the App installation. CAI then:

1. Reads the repository visibility and default branch.
2. Creates a dedicated `cai/setup` branch.
3. Adds the standard `cai.yml` workflow.
4. Pins the CAI Action to an immutable release commit SHA.
5. Chooses `ubuntu-latest` for a public repository and `self-hosted` for a private repository.
6. Opens a normal ready-for-review pull request.

The workflow becomes active only after a human merges that pull request.

The repository owner controls runner groups and eligible self-hosted runners in GitHub. If no runner matches the generated selector, the workflow remains queued. CAI must not redirect that job to an operator-owned runner.

## Credentials

The CAI deployment stores the GitHub App private key and webhook secret outside repositories, workflow files, and run artifacts. For each privileged operation, it generates an App JWT and mints a fresh installation access token scoped to the installation.

Installation tokens, `GITHUB_TOKEN`, OAuth access tokens, provider credentials, and private keys must be redacted from logs and excluded from artifacts. They must not be passed through command-line arguments or workflow outputs.

## Actions OIDC exchange

The generated workflow does not receive a stored App private key or a repository secret containing an installation token. It requests a GitHub OIDC token and sends it to the CAI deployment's token-exchange endpoint.

CAI validates the token's repository, generated workflow, default-branch reference, GitHub run ID, and current App installation scope before minting a fresh installation token for that run. The task container never receives that token; only the delivery layer outside the container may use it.

CAI exposes its GitHub webhook endpoint and OIDC token-exchange endpoint through one Cloudflare Tunnel. A public health endpoint is permitted. All UI and administrative APIs remain private.
