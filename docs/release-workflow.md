# Release Workflow

Tag `vX.Y.Z` starts release. Tag must match `Cargo.toml`, root `Cargo.lock`, and
top-level CHANGELOG heading. Ordinary `main` push cannot release. PR runs plan,
config, fixture, permission, and pin checks only.

## Targets

- `aarch64-apple-darwin`, runner `macos-14`
- `x86_64-apple-darwin`, runner `macos-15-intel`
- `x86_64-unknown-linux-gnu`, runner `ubuntu-22.04`, pinned Rust 1.96.0 Debian
  11 container

GNU chosen over musl. Debian 11 build floor is glibc 2.31. Build job records
`ldd`, linked libraries, and highest required GLIBC symbol. Separate smoke job
verifies checksum, unpacks archive, and runs packaged `cowork --help`. Musl
would need cross tools on glibc CI or Alpine compatibility for JavaScript
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

## Trust

- cargo-dist pinned to `0.32.0`; host archives use hard-coded SHA-256
- `allow-dirty = ["ci"]` keeps cargo-dist from owning hand-written workflow
- Rust pinned to `1.96.0`; Linux image pinned by digest
- every action pinned by commit
- archive checksums verified after build, during packaged smoke, and before
  release upload
- attestation job alone gets OIDC and attestation writes
- release job alone gets `contents: write`
- matching CHANGELOG section is sole GitHub release body
