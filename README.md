# cowork

`cowork` asks focused questions about repo files, sends one narrow prompt to local model endpoint, then prints deterministic JSON.

## Quickstart

`cowork ask` needs:

- one or more `--paths`
- one `--question`
- `--model`, unless set in config

Minimal example:

```bash
cowork ask \
  --paths src/config.rs \
  --question "How does ask config precedence work?" \
  --model your-model
```

`host` defaults to `http://localhost:11434`. `model` has no built-in default.

## Config

`cowork` reads `[ask]` from:

- `./cowork.toml`
- `$HOME/.cowork/config.toml`

Supported keys:

- `model`
- `host`

Precedence:

- `host`: built-in, user, project, CLI
- `model`: user, project, CLI

If `model` is unset in both config files, pass `--model`.

Example:

```toml
[ask]
model = "your-model"
host = "http://localhost:11434"
```
