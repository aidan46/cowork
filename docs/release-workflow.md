# Release Workflow

Tag `vX.Y.Z` starts stable release. Tag `vX.Y.Z-rc.N` starts prerelease, where
`N` is positive decimal without leading zero. Tag must match `Cargo.toml`, root
`Cargo.lock`, and top-level CHANGELOG heading. Other suffixes fail validation.
Ordinary `main` push cannot release. PR runs plan, config, fixture, permission,
and pin checks only.

## Targets

- `aarch64-apple-darwin`, runner `macos-14`
- `x86_64-apple-darwin`, runner `macos-15-intel`
- `x86_64-unknown-linux-gnu`, runner `ubuntu-22.04`, pinned Rust 1.96.0 Debian
  11 container

GNU chosen over musl. Debian 11 build floor is glibc 2.31. Build job records
`ldd`, linked libraries, and highest required GLIBC symbol. Separate smoke job
verifies checksum before extraction into empty temp dir. It requires exact
package files, native executable format and architecture, exact version output
with empty stderr, working help, and Markdown rules from
`cowork init codex --print` with empty stderr.
Musl would need cross tools on glibc CI or Alpine compatibility for JavaScript
actions. GNU keeps native build and explicit runtime floor.

## Assets

- `cowork-aarch64-apple-darwin.tar.xz`
- `cowork-aarch64-apple-darwin.tar.xz.sha256`
- `cowork-x86_64-apple-darwin.tar.xz`
- `cowork-x86_64-apple-darwin.tar.xz.sha256`
- `cowork-x86_64-unknown-linux-gnu.tar.xz`
- `cowork-x86_64-unknown-linux-gnu.tar.xz.sha256`
- `cowork-installer.sh`

Archives contain `cowork`, README, CHANGELOG, MIT license, and Apache license.
Installer selects only listed archives and installs only `cowork` under Cargo
home. `release-manifest.txt` is upload allowlist.

Linux installer smoke runs generated script before upload. `COWORK_DOWNLOAD_URL`
points it at checked local workflow assets through `file://`; unpublished GitHub
assets are never fetched. Smoke uses isolated `HOME` and `CARGO_HOME`, checks
installed version and receipt, allowlists all writes, verifies PATH files and
`GITHUB_PATH`, rejects network, Ollama, or service commands, and proves corrupt
archive rejection. Internal per-target cargo-dist manifests carry archive
checksums into generated installer; manifests are never release assets. PR
validation also builds and executes real generated Linux installer.

## Trust

- cargo-dist pinned to `0.32.0`; host archives use hard-coded SHA-256
- `allow-dirty = ["ci"]` keeps cargo-dist from owning hand-written workflow
- Rust pinned to `1.96.0`; Linux image pinned by digest
- every action pinned by commit
- archive checksums verified after build, during packaged smoke, and before
  release upload
- installer upload waits for isolated local-asset smoke
- attestation job alone gets OIDC and attestation writes
- release job alone gets `contents: write`
- matching CHANGELOG section is sole GitHub release body
- validated RC tags publish non-draft GitHub prereleases
- validated stable tags publish non-draft stable GitHub releases
