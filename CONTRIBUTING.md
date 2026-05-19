# Contributing

## Branches

- Never push directly to `main`.
- Start work on branch, open PR, wait for checks, then hand off to human for review.
- Use linear history. Rebase when needed. No merge commits.

Suggested branch prefixes:

- `feat/`
- `fix/`
- `refactor/`
- `perf/`
- `test/`
- `docs/`
- `chore/`
- `ci/`

## Pull requests

- Open PR for every session change meant to land.
- PRs merge by squash only.
- Merge with `gh pr merge <PR-number> --squash --delete-branch --subject ... --body-file ...`.
- Pass explicit squash commit subject and `--body-file` when merging. Do not blindly use default combined commit text.
- Wrap squash commit body lines to 72 cols max. No long one-line paragraphs.
- Prefer `scripts/gh-pr-merge-squash` for merge step.
- Use `.github/pull_request_template.md`.
- PR title must be a good Conventional Commit subject, because squash merge uses it as the commit message.
- In `## Summary`, state what changed, where it changed, and visible behavior impact.
- In `## How to verify`, wrap each test command in backticks so reviewers can copy it directly.
- In `## How to verify`, include expected result after each relevant command, especially for CLI output or exit-code changes.
- For behavior checks, call out exact JSON codes, key messages, or stub path reached when that is the point of the PR.
- Use `cargo nextest run`, not `cargo test`, for Rust test execution in PR verification, hooks, and local checks.
- Wait for checks with `gh pr checks <PR-number> --watch`.
- Report to human only after checks pass or a real blocker is clear.
- Do not self-merge unless human explicitly asks.

## Commits

Use Conventional Commits:

```txt
<type>(<scope>): <subject>
```

PR titles should use the same format, because squash merge uses the PR title as the final commit subject.

Types:

- `feat`
- `fix`
- `docs`
- `refactor`
- `perf`
- `test`
- `build`
- `ci`
- `chore`
- `revert`

## Git hooks

Hooks live in `.githooks/`.

Activate once per clone:

```bash
git config core.hooksPath .githooks
```

Checks:

- `pre-commit`: `taplo fmt --check Cargo.toml taplo.toml`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `taplo fmt --check Cargo.toml taplo.toml`, `cargo nextest run`, `cargo doc --no-deps --quiet`

Manual run:

```bash
bash .githooks/pre-commit
bash .githooks/pre-push
```

## Merge command

Preferred merge command:

```bash
scripts/gh-pr-merge-squash <PR-number> "<type>(<scope>): <subject>" path/to/merge-body.txt
```
