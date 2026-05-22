# Changelog

All notable changes to `cowork` land here.

## Unreleased

## v0.2.0, 2026-05-21

### Added

- `cowork doctor` for local setup diagnostics and probe checks
- `cowork init codex --print` and `cowork init claude --print`
- `cowork init codex --write` and `cowork init claude --write`
- `cowork locate` for symbol and file lookup

### Changed

- split output helpers and command runner into narrower modules
- lint policy now blocks production `expect` and `unwrap`

## v0.1.0, 2026-05-20

### Added

- `cowork ask` for narrow repo questions against local Ollama-compatible `/api/generate`.
- config precedence: CLI flags, then `./cowork.toml`, then `$HOME/.cowork/config.toml`
- recursive file loading with `--recursive`, `--include`, `--exclude`, and `--max-bytes`
- deterministic JSON success and error output with fixed top-level fields

### Notes

- first public GitHub release is source-only
- supported run path today is from source repo with Cargo
- no crates.io install path or packaged binaries in this release
