# locate

## Start

1. Archive start prompt first:
   - move `prompts/locate.md` to `prompts/archive/locate.md`
2. Read `AGENTS.md`.
3. Read this task block from archived prompt.
4. Read live `HANDOFF.md` if it has content.
5. Read only `IMPLEMENTATION_PLAN.md` Phase 6 and only locate-relevant `SPEC.md` sections.
6. Read `CONTRIBUTING.md`.
7. Run `git status --short`.
8. Run `git branch --show-current`.
9. If on `main`, branch before tracked edits.

## Goal

Ship first `cowork locate` command so agent can get likely files and symbols before deeper reads.

## Scope

Do in order:

1. Add command surface:
   - `cowork locate --paths <PATHS>... --thing "<THING>"`
   - wire help text and parsing
2. Add narrow locate run path:
   - reuse current file discovery and load flow where it keeps behavior stable
   - ask model for likely files and symbols only
   - no long explanation
   - no edit plan
3. Add fixed JSON stdout:
   - top-level `schema_version`
   - top-level `command = "locate"`
   - top-level `status = "ok"`
   - `matches[]` with:
     - `path`
     - optional `symbol`
     - optional `kind`
     - `reason`
     - `confidence`
   - `next_reads[]`
   - `risks[]`
4. Add narrow tests:
   - help and parse path
   - typed output parse or serialize path
   - at least one success path with fixed JSON fields
5. Update docs only where command surface or verify flow changed.

## Out Of Scope

- `review` command
- `plan` command
- `risk` command
- package or release work
- provider abstraction
- daemon, cache, index, or background service
- broad prompt system reshuffle
- path filtering rewrite
- new human-only output mode

## Constraints

- Follow `AGENTS.md`.
- Keep stdout JSON only.
- Keep scope narrow.
- Prefer current module seams:
  - `src/commands.rs`
  - `src/files.rs`
  - `src/model.rs`
  - `src/output.rs`
  - `src/prompt.rs`
- Do not add crate unless direct need proven.
- Do not add new exit codes unless current codes cannot express failure.
- Do not build symbol index or parser layer.
- If extra path flags help reuse current ask flow, keep them minimal and deterministic.

## Files

- `src/cli.rs`
- `src/commands.rs`
- `src/commands/locate.rs`
- `src/output.rs`
- `src/output/locate.rs`
- `src/prompt.rs`
- tests touched by command add, only if needed
- `HANDOFF.md`

## Accept

- `cowork locate` help shows required flags
- command returns fixed JSON top-level fields
- output can return likely files and symbols
- output keeps `matches[]`, `next_reads[]`, and `risks[]` deterministic
- no long explanation or edit-plan text added
- `taplo fmt --check Cargo.toml taplo.toml` pass
- `cargo fmt --all -- --check` pass
- `cargo nextest run` pass
- `cargo clippy --all-targets -- -D warnings` pass
- `cargo doc --no-deps --quiet` pass

## PR Target

- Branch: `feat/locate`
- PR title: `feat(locate): add symbol and file locator command`

## Hygiene

- Update `HANDOFF.md`
- Update checklist if touched
- Open PR
- Wait: `gh pr checks <PR-number> --watch`
- Report ready for review
