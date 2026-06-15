# cowork

Focused repo questions in, schema-checked JSON out through a local model endpoint.

[![CI](https://github.com/aidan46/cowork/actions/workflows/ci-rust.yml/badge.svg)](https://github.com/aidan46/cowork/actions/workflows/ci-rust.yml)
[![Release: v0.6.0-rc.1](https://img.shields.io/badge/release-v0.6.0--rc.1-orange.svg)](https://github.com/aidan46/cowork/releases/tag/v0.6.0-rc.1)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

> `v0.6.0-rc.1` is a prerelease. No stable `v0.6.0` installer is published.

## Install and run

Prerequisites:

- supported macOS or Linux platform listed below
- [Ollama](https://ollama.com/download) installed and running

Install exact prerelease asset:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/aidan46/cowork/releases/download/v0.6.0-rc.1/cowork-installer.sh | sh
```

Installer places only `cowork` in `$CARGO_HOME/bin` or `$HOME/.cargo/bin`. It may
update shell profiles for `PATH`. It never installs or starts Ollama.

Confirm installed version:

```bash
cowork --version
```

```text
cowork 0.6.0-rc.1
```

Choose a model, pull it when missing, write user config, then probe it:

```bash
cowork setup --write-config --pull
```

Ask one repo-grounded question:

```bash
cowork ask \
  --paths src/cli.rs src/config.rs \
  --question "How does ask config precedence work?"
```

## Output

Commands print JSON on stdout, including errors. Exact output from a current
local `ask` fixture:

```json
{
  "schema_version": "1.0",
  "command": "ask",
  "status": "ok",
  "question": "What does this function return?",
  "answer": {
    "summary": "It returns 42.",
    "confidence": "high",
    "not_found": false
  },
  "files": [
    {
      "path": "input.rs",
      "included": true,
      "reason": "Defines the function.",
      "bytes": 25
    }
  ],
  "symbols": [],
  "evidence": [],
  "risks": [],
  "next_reads": [],
  "metadata": {
    "input_bytes": 25,
    "output_bytes": 384,
    "duration_ms": 2
  }
}
```

Model text varies. Top-level framing, normalized fields, and JSON-only stdout are
CLI-owned.

## Use cases

- `locate`: find likely files and symbols before loading more context
- `brief`: build compact handoff context with evidence and risks
- `ask`: answer a narrow question over selected files
- `doctor`: run read-only local setup diagnostics
- `setup`: choose and probe a model, with explicit mutation flags
- `init`: print or write bounded agent instruction blocks

Use `cowork` when a coding agent needs focused local analysis without pasting raw
repo files into hosted context. Skip it for known-file edits, broad architecture
reviews, or final validation that requires direct source inspection.

## Safety boundary

- Default model host is local: `http://localhost:11434`.
- Overriding `--host` or `[ask].host` sends selected content to that endpoint.
- Only explicit file paths, plus files discovered under explicit directories with
  `--recursive`, can be loaded. `cowork` does not scan a repo by itself.
- Include, exclude, byte-limit, UTF-8, binary, symlink, and missing-path rules run
  before model requests.
- Bare `cowork setup` does not pull models or write config.
- `--pull` permits one Ollama model pull. `--write-config` permits config writes.
- Local-model summaries and conclusions are advisory. Inspect cited evidence and
  source before edits.

## Supported platforms

Published `v0.6.0-rc.1` artifacts:

| Platform | Target | Asset | Status |
| --- | --- | --- | --- |
| macOS Apple Silicon | `aarch64-apple-darwin` | `cowork-aarch64-apple-darwin.tar.xz` | supported |
| macOS Intel | `x86_64-apple-darwin` | `cowork-x86_64-apple-darwin.tar.xz` | supported |
| Linux x86_64, glibc 2.31+ | `x86_64-unknown-linux-gnu` | `cowork-x86_64-unknown-linux-gnu.tar.xz` | supported |

Windows, Linux ARM64, Linux musl, and other targets have no published RC artifact.
They are unsupported and untested for this release candidate.

## Verify or build

### Manual archive verification

Select target, then download exact archive and checksum:

```bash
version=v0.6.0-rc.1
target=aarch64-apple-darwin
# target=x86_64-apple-darwin
# target=x86_64-unknown-linux-gnu
archive="cowork-$target.tar.xz"
base="https://github.com/aidan46/cowork/releases/download/$version"

curl -fLO "$base/$archive"
curl -fLO "$base/$archive.sha256"
```

Verify checksum on Linux or macOS:

```bash
if command -v sha256sum >/dev/null 2>&1; then
  sed '/^[[:space:]]*$/d' "$archive.sha256" | sha256sum -c -
else
  sed '/^[[:space:]]*$/d' "$archive.sha256" | shasum -a 256 -c -
fi
```

Verify GitHub artifact attestation:

```bash
gh attestation verify "$archive" --repo aidan46/cowork
```

Extract and install manually:

```bash
tar -xJf "$archive"
mkdir -p "$HOME/.local/bin"
install -m 0755 "cowork-$target/cowork" "$HOME/.local/bin/cowork"
```

Installer also has a published attestation:

```bash
curl -fL \
  https://github.com/aidan46/cowork/releases/download/v0.6.0-rc.1/cowork-installer.sh \
  -o cowork-installer.sh
gh attestation verify cowork-installer.sh --repo aidan46/cowork
```

### Install from source

Rust toolchain required:

```bash
git clone --branch v0.6.0-rc.1 --depth 1 https://github.com/aidan46/cowork.git
cd cowork
cargo install --path . --locked
```

## Workflows

Locate likely files first:

```bash
cowork locate \
  --paths src \
  --recursive \
  --thing "where config precedence is resolved"
```

Build compact handoff context:

```bash
cowork brief \
  --paths src/config.rs src/commands/ask.rs \
  --goal "change config precedence safely" > brief.json
```

Ask directly when file set is already known:

```bash
cowork ask \
  --paths src/config.rs src/commands/ask.rs \
  --question "Which config source wins?"
```

Add `--model your-model` when config has no model. Use `--no-fail-on-missing`
only when skipping missing inputs is intended.

## Config and files

`cowork` reads `[ask]` from `./cowork.toml`, then
`$HOME/.cowork/config.toml`:

```toml
[ask]
model = "your-model"
host = "http://localhost:11434"
```

Precedence:

| Key | Order | Default |
| --- | --- | --- |
| `model` | CLI, project, user | none |
| `host` | CLI, project, user, built-in | `http://localhost:11434` |

File rules:

- files load directly; directories require `--recursive`
- explicit files bypass `--include`, but still respect `--exclude`
- missing paths fail with `MISSING_PATH` unless `--no-fail-on-missing` is set
- zero surviving readable UTF-8 files fails with `NO_INPUT_FILES`
- `--max-bytes` fails when loaded input exceeds limit
- symlinks, binary files, and non-UTF-8 files are skipped
- recursive walks prune common generated and dependency directories

See [config.example.toml](config.example.toml) for starter config.

## Limits

- one Ollama-compatible model client
- no cache, daemon, or index layer
- narrow selected-file questions, not automatic whole-repo analysis
- model output can be incomplete or wrong
- `metadata.input_bytes` and `metadata.output_bytes` are bytes, not token counts

## Project links

- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Support](SUPPORT.md)
- [Release candidate](https://github.com/aidan46/cowork/releases/tag/v0.6.0-rc.1)
- Licenses: [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
