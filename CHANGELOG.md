# Changelog

All notable changes to `cowork` land here.

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
