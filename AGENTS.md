# AGENTS

## Core Rules
- Use caveman skill for all agent-facing files: `AGENTS.md`, `HANDOFF.md`, `README.md`, `SPEC.md`, `docs/*`, `prompts/*`, plan docs, review docs, any file written mainly for agent consumption.
- Use caveman skill for code comments too. Keep comments short, direct, low-token.
- Never push directly to `main`.
- Always land work through PR.
- After opening PR, wait for checks with `gh pr checks <PR-number> --watch`.
- Report to human when PR is good to review, or blocked by concrete failing checks.
- Keep scope narrow. Prefer small patches over broad rewrites.
- MVP first. Do not add cache, provider abstraction, daemon flow, indexing system, or multi-command architecture unless task requires it.
- Read less. Search first. Inspect exact symbols or lines next. Avoid large file reads without reason.
- Update `HANDOFF.md` after meaningful progress.
- Update checklist progress when work lands.
- Preserve deterministic JSON CLI output. No conversational text in command responses.

## Session Start
- Read `AGENTS.md` first.
- Read active prompt next.
- Read `HANDOFF.md` if it has live content.
- Read only relevant `SPEC.md` sections for current task.
- Read `CONTRIBUTING.md` when task touches git, PR, CI, release flow.
- Run `git status --short`.
- Run `git branch --show-current`.
- If `.github/` setup is missing, do GitHub bootstrap before feature work.
- If current branch is `main`, create feature branch before edits intended for PR.
- Confirm current task scope before editing unrelated modules.
- Prefer one module or one flow per session when possible.
- Record decisions that change schema, flags, config, or exit codes.

## Prompt Rules
- Prompts stay short.
- Prompts hold task-local scope, accept, out-of-scope, branch target, PR target.
- Prompts do not restate stable repo rules already in `AGENTS.md` or `CONTRIBUTING.md`.
- Use caveman ultra skill in prompts too.

## Implementation Bias
- Prefer manual config merge over heavy config frameworks.
- Prefer blocking HTTP over async runtime for MVP.
- Prefer typed errors with stable exit codes over generic error plumbing.
- Prefer explicit tests for schema, config precedence, and file filtering.

## Context Control
- Do not broad-scan repo without reason.
- Do not rewrite module layout during MVP unless current layout blocks progress.
- Do not add crates without direct need.
- Do not keep stale history in `HANDOFF.md`.

## Done Check
- Run `cargo fmt`
- Run `cargo nextest run`
- Run `cargo clippy -- -D warnings`
- Run `cargo doc --no-deps --quiet`
- Update `HANDOFF.md`
- Update checklist
