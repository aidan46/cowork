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

Current release path is source checkout only. No crates.io package or prebuilt binaries yet.

Examples below assume `cowork` binary already exists from local source build.

Need:

- Rust toolchain
- local model endpoint that supports `POST /api/generate`
- one or more `--paths`
- one `--question`
- `--model`, unless config already sets it

Verify setup:

```bash
cowork doctor --model your-model
```

Print agent rules:

```bash
cowork init codex --print
cowork init claude --print
```

Write managed agent blocks:

```bash
cowork init codex --write
cowork init claude --write
```

If `doctor` passes, ask next:

```bash
cowork ask \
  --paths src/cli.rs src/config.rs \
  --question "How does ask config precedence work?" \
  --model your-model
```

Or locate first, then decide what to read:

```bash
cowork locate \
  --paths src \
  --recursive \
  --thing "CLI parser" \
  --model your-model
```

Path handling defaults to strict mode:

- missing provided path returns JSON error code `MISSING_PATH`
- `--no-fail-on-missing` skips only missing paths, then continues with surviving inputs
- if load ends with zero readable UTF-8 text files, command returns `NO_INPUT_FILES`

## Token-saving workflow

Use `locate` to find likely files first. Use `brief` next when cloud agent needs compact repo context instead of raw file dumps.

Find likely files:

```bash
cowork locate \
  --paths src \
  --recursive \
  --thing "where config precedence is resolved" \
  --model your-model
```

Check `files`, `symbols`, and `evidence`, then build compact handoff:

```bash
cowork brief \
  --paths src/config.rs src/commands/ask.rs \
  --goal "change config precedence safely" \
  --model your-model
```

Command output is JSON on stdout only. Redirect if you want file:

```bash
cowork brief \
  --paths src/config.rs src/commands/ask.rs \
  --goal "change config precedence safely" \
  --model your-model > brief.json
```

Errors stay JSON on stdout too. Top-level `command` tells scripts whether failure came from
`ask`, `brief`, `locate`, or fallback `cli`.

Cloud-agent handoff block:

```txt
Use attached `brief.json` as advisory repo context.
Inspect cited `evidence` paths before edits.
Treat local-model `summary`, `risks`, and conclusions as advisory.
If context is thin or stale, read source files directly before changing code.
```

Notes:

- `metadata.input_bytes` is loaded input byte count, not exact token count
- `metadata.output_bytes` is final JSON byte count after CLI normalization
- `risks` can include CLI-added `unknown` notices when output caps or string truncation fire
- inspect cited evidence before edits, not only model summary
- `cowork` is for narrow repo questions, not whole-repo architecture sweeps
- skip `cowork` when you already know exact files and need direct source reading or broad refactor validation

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
- missing paths fail by default with `MISSING_PATH`
- `--no-fail-on-missing` skips only missing paths
- `--max-bytes` fails hard when loaded input grows past limit
- symlinks, binary files, and non-UTF-8 files are skipped
- if no readable UTF-8 text files survive load, commands fail with `NO_INPUT_FILES`
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
    "output_bytes": 612,
    "duration_ms": 12
  }
}
```

Notes:

- `metadata.output_bytes` is final JSON byte count after normalization
- `risks` may include CLI-owned `unknown` notices when output caps or truncation fire
- error responses keep same top-level `schema_version`, `command`, and `status` framing

## Scope today

- five subcommands: `doctor`, `ask`, `locate`, `brief`, `init`
- `init --print` prints short agent rules
- `init --write` writes bounded managed blocks to `AGENTS.md` or `CLAUDE.md`
- JSON stdout only, even on errors
- local-model summaries stay advisory, and bounded outputs may add CLI-owned risk notices
- one Ollama-style model client
- no cache, daemon, or index layer

## Help and contributing

- read [SUPPORT.md](SUPPORT.md) for usage questions and bug reports
- read [CONTRIBUTING.md](CONTRIBUTING.md) before opening PR
- use `cowork doctor --help`, `cowork ask --help`, `cowork locate --help`, `cowork brief --help`, and `cowork init --help` for current flag surface
