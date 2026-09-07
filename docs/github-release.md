# GitHub release snapshots

Development takes place on the private University of Lille GitLab repository.
The GitHub repository at <https://github.com/2xs/build-fae> receives
history-free release snapshots. Its `main` branch is therefore replaced at
each release rather than merged with the development history.

## Prerequisites

Configure Git to sign tags with an SSH key that is also registered as a GitHub
signing key:

```sh
git config gpg.format ssh
git config user.signingkey ~/.ssh/id_ed25519.pub
```

The repository must have a clean worktree. Run the complete validation before
preparing a release:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo embedded-check
```

## Prepare and publish

The script creates an orphan `github` branch from the selected source commit,
adds the public-repository notice to its README, and creates a signed tag:

```sh
scripts/publish-github-snapshot.sh v0.1.0-alpha.0
```

Inspect the resulting branch and verify its signature before publishing:

```sh
git diff --stat origin/github github
git tag --verify v0.1.0-alpha.0
```

To create and publish in one operation, including a GitHub prerelease through
the authenticated `gh` command, run:

```sh
scripts/publish-github-snapshot.sh --release v0.1.0-alpha.0
```

The script replaces `origin/github` and GitHub's `main` with leases, then
pushes the signed tag. A prepared candidate can instead be published manually
after review with ordinary `git push --force-with-lease` commands.
