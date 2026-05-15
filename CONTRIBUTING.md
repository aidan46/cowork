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
- Use `.github/pull_request_template.md`.
- Wait for checks with `gh pr checks <PR-number> --watch`.
- Report to human only after checks pass or a real blocker is clear.
- Do not self-merge unless human explicitly asks.

## Commits

Use Conventional Commits:

```txt
<type>(<scope>): <subject>
```

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

- `pre-commit`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`
- `pre-push`: `cargo test`, `cargo doc --no-deps --quiet`

Manual run:

```bash
bash .githooks/pre-commit
bash .githooks/pre-push
```
