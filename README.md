# cowork

Focus repo question in, deterministic JSON out.

`cowork` is local CLI for coding agents. It reads only files you point at, sends one narrow prompt to local Ollama-compatible `/api/generate` endpoint, then prints schema-checked JSON on stdout.

Main win: cut expensive cloud-model context use. Instead of pasting raw repo files into hosted model, agent can ask `cowork` for narrow local analysis and get back compact JSON.

## About

`cowork` is for mixed local-plus-cloud workflows.

- local model does narrow repo read
- hosted model gets small structured result
- cloud token use drops because raw file dumps stay out of hosted context
- output stays deterministic enough for scripts and agents

It does not remove token use entirely. It shifts repo-context work from expensive hosted context toward cheaper local inference.

## Why it exists

When agent or script needs one repo-grounded answer, whole-repo scans and raw file paste are waste. `cowork` keeps hosted context small, config simple, output stable.

## Quickstart

Need:

- Rust toolchain
- local model endpoint that supports `POST /api/generate`
- one or more `--paths`
- one `--question`
- `--model`, unless config already sets it

Run from repo root:

```bash
cargo run -- ask \
  --paths src/cli.rs src/config.rs \
  --question "How does ask config precedence work?" \
  --model your-model
```

If you use same model often, put it in config and drop `--model` from command:

```toml
[ask]
model = "your-model"
host = "http://localhost:11434"
```

Save that as `./cowork.toml` for one repo, or `$HOME/.cowork/config.toml` for user-wide defaults.
See [config.example.toml](config.example.toml) for minimal starter config.

## Config summary

`cowork` reads `[ask]` from:

- `./cowork.toml`
- `$HOME/.cowork/config.toml`

Supported keys:

| Key | Meaning | Default | Precedence |
| --- | --- | --- | --- |
| `model` | model name sent to endpoint | none | CLI, project, user |
| `host` | base URL for model endpoint | `http://localhost:11434` | CLI, project, user, built-in |

## File loading

- file paths load directly
- directory paths need `--recursive`
- `--include` and `--exclude` filter discovered files
- explicit file args bypass `--include`, but still respect `--exclude`
- `--max-bytes` fails hard when loaded input grows past limit
- symlinks, binary files, and non-UTF-8 files are skipped
- recursive walks prune `.git`, `target`, `node_modules`, `dist`, `build`, `.next`, `.cache`, and `coverage`

## Example output

Example success shape:

```json
{
  "schema_version": "1.0",
  "command": "ask",
  "status": "ok",
  "question": "How does ask config precedence work?",
  "answer": {
    "summary": "CLI flags win, then project config, then user config.",
    "confidence": "high",
    "not_found": false
  },
  "files": [
    {
      "path": "src/config.rs",
      "included": true,
      "reason": "Loads and merges ask config.",
      "bytes": 5293
    }
  ],
  "symbols": [],
  "evidence": [],
  "risks": [],
  "next_reads": [],
  "metadata": {
    "input_bytes": 5293,
    "duration_ms": 12
  }
}
```

## Scope today

- one subcommand: `ask`
- JSON stdout only, even on errors
- one Ollama-style model client
- no cache, daemon, or index layer

## Help and contributing

- read [SUPPORT.md](SUPPORT.md) for usage questions and bug reports
- read [CONTRIBUTING.md](CONTRIBUTING.md) before opening PR
- use `cargo run -- ask --help` for current flag surface
