# Unreleased

# v0.5.0, 2026-06-09

## Added

- `cowork setup` recommends local model, optionally pulls it, writes ask config
  only when requested, and probes available chosen model

## Changed

- `cowork doctor` now reports installed Ollama model state, requires exact
  effective-model match, and skips probe when model is missing

# v0.4.0, 2026-06-08

## Added

- command-aware JSON errors for `ask`, `brief`, `locate`, and fallback `cli`
- `NO_INPUT_FILES` when zero readable UTF-8 text files survive path loading

## Changed

- missing paths fail by default, `--no-fail-on-missing` skips only missing
  paths
- bounded output notices now apply to `ask`, `brief`, and `locate`
- `metadata.output_bytes` now reports final JSON byte count after normalization
- README now documents v0.4.0 reliability behavior for users and scripts

# v0.3.0, 2026-06-07

## Added

- `cowork brief` for compact, goal-focused context packs
- documented `locate` then `brief` workflow for cloud-agent handoff
- generated Codex and Claude init rules teaching when to use `locate` and `brief`

## Changed

- CLI now owns byte counts, duration, output size, and meaningful compression ratios
- `cowork ask` normalizes model rows and caps response collections for stable output

# v0.2.0, 2026-05-21

## Added

- `cowork doctor` for local setup diagnostics and probe checks
- `cowork init codex --print` and `cowork init claude --print`
- `cowork init codex --write` and `cowork init claude --write`
- `cowork locate` for symbol and file lookup

## Changed

- split output helpers and command runner into narrower modules
- lint policy now blocks production `expect` and `unwrap`

# v0.1.0, 2026-05-20

## Added

- `cowork ask` for narrow repo questions against local Ollama-compatible `/api/generate`.
- config precedence: CLI flags, then `./cowork.toml`, then `$HOME/.cowork/config.toml`
- recursive file loading with `--recursive`, `--include`, `--exclude`, and `--max-bytes`
- deterministic JSON success and error output with fixed top-level fields

## Notes

- first public GitHub release is source-only
- supported run path today is from source repo with Cargo
- no crates.io install path or packaged binaries in this release
