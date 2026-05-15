# Cowork CLI Specification

## Overview

`cowork` is a local AI coworker CLI designed for coding agents.

The primary goal is to reduce expensive context usage by allowing coding agents to delegate targeted repository inspection and file summarization tasks to a smaller local model.

`cowork` is:

* local-first
* deterministic
* machine-readable
* optimized for coding-agent workflows
* composable with existing coding agents

The first implemented subcommand is:

```bash
cowork ask
```

---

# Design Goals

## Primary goals

1. Minimize context usage for large coding agents
2. Return deterministic structured output
3. Optimize for machine parsing over human readability
4. Keep prompts narrow and task-focused
5. Support local models via Ollama
6. Avoid speculative analysis

## Non-goals

* Full autonomous agent framework
* Repository indexing platform
* IDE replacement
* Long-running daemon system
* Generic chatbot interface

---

# Core Command

```bash
cowork ask \
  --paths <PATHS>... \
  --question "<QUESTION>"
```

Example:

```bash
cowork ask \
  --paths src/auth src/server \
  --question "How does request authentication work?"
```

---

# Command Philosophy

`cowork ask` should answer:

* What matters?
* Which files matter?
* Which symbols matter?
* What should the main coding agent inspect next?

It should not:

* explain the whole repository
* generate architecture essays
* speculate beyond provided files
* replace direct inspection

---

# CLI Interface

```bash
cowork ask \
  --paths <PATHS>... \
  --question <QUESTION> \
  [--model <MODEL>] \
  [--host <HOST>] \
  [--max-bytes <BYTES>] \
  [--recursive] \
  [--include <GLOB>] \
  [--exclude <GLOB>] \
  [--fail-on-missing]
```

---

# Default Configuration

```txt
model            = default local model
host             = http://localhost:11434
max-bytes        = 250000
recursive        = false
fail-on-missing  = true
```

---

# Arguments

## `--paths <PATHS>...`

Required.

Accepts files or directories.

Rules:

* files are read directly
* directories require `--recursive`
* binary files are skipped
* unreadable files are reported
* symbolic links are ignored by default
* explicit file paths bypass `--include` filters

Default excluded directories:

```txt
.git
target
node_modules
dist
build
.next
.cache
coverage
```

---

## `--question <QUESTION>`

Required.

The question must be narrow and task-oriented.

Good examples:

```txt
How does request authentication work?
```

```txt
Which files are responsible for retry logic?
```

```txt
Where is pagination state managed?
```

Bad examples:

```txt
Explain this repository.
```

```txt
How does the whole backend work?
```

---

## `--model <MODEL>`

Override the configured local model.

Example:

```bash
--model gemma3:12b
```

---

## `--host <HOST>`

Override the Ollama endpoint.

Example:

```bash
--host http://192.168.1.50:11434
```

---

## `--max-bytes <BYTES>`

Maximum total file content sent to the model.

If exceeded:

* return structured error
* do not partially truncate in MVP

Future versions may implement chunking.

---

## `--recursive`

Allow recursive directory traversal.

---

## `--include <GLOB>`

Only include matching files.

Example:

```bash
--include "*.rs"
```

---

## `--exclude <GLOB>`

Exclude matching files.

Example:

```bash
--exclude "*.min.js"
```

---

## `--fail-on-missing`

Fail if any provided path does not exist.

Default:

```txt
true
```

---

# Output Philosophy

MVP output format is strict JSON.

Goals:

* deterministic parsing
* stable schema
* low ambiguity
* easy integration with coding agents
* compatibility with jq and scripting pipelines

The output should:

* avoid unnecessary prose
* avoid Markdown formatting
* avoid conversational language
* avoid speculative reasoning
* emit fixed top-level fields

---

# JSON Output Schema

## Success Response

```json
{
  "schema_version": "1.0",
  "command": "ask",
  "status": "ok",
  "question": "How does request authentication work?",
  "answer": {
    "summary": "Authentication is enforced in middleware before route handlers execute.",
    "confidence": "high",
    "not_found": false
  },
  "files": [
    {
      "path": "src/auth/middleware.rs",
      "included": true,
      "reason": "Contains authentication middleware.",
      "bytes": 12420
    }
  ],
  "symbols": [
    {
      "name": "authenticate_request",
      "kind": "function",
      "path": "src/auth/middleware.rs",
      "relevance": "Validates credentials and attaches user context."
    }
  ],
  "evidence": [
    {
      "path": "src/auth/middleware.rs",
      "symbol": "authenticate_request",
      "note": "Requests without valid credentials return an unauthorized response before reaching handlers."
    }
  ],
  "risks": [
    {
      "kind": "missing_context",
      "message": "Tests were not provided."
    }
  ],
  "next_reads": [
    {
      "path": "src/auth/tests.rs",
      "reason": "Likely contains authentication edge cases."
    }
  ],
  "metadata": {
    "input_bytes": 12420,
    "duration_ms": 980
  }
}
```

---

# Schema Rules

## `status`

Allowed values:

```txt
ok
error
```

---

## `confidence`

Allowed values:

```txt
high
medium
low
unknown
```

---

## `symbols[].kind`

Allowed values:

```txt
function
type
trait
impl
module
constant
variable
route
component
test
unknown
```

---

## `risks[].kind`

Allowed values:

```txt
missing_context
model_uncertainty
parse_error
skipped_file
unsupported_file
unknown
```

---

# Error Output Schema

Errors also return JSON.

Example:

```json
{
  "schema_version": "1.0",
  "command": "ask",
  "status": "error",
  "error": {
    "code": "OLLAMA_REQUEST_FAILED",
    "message": "Failed to connect to local model endpoint.",
    "hint": "Ensure the local model server is running."
  }
}
```

---

# Exit Codes

```txt
0  success
1  invalid arguments or config error
2  file read or path error
3  max bytes exceeded
4  model request failed
5  response parse failed
6  no usable input files
```

---

# Prompt Contract

The model prompt must enforce strict structured output with low token overhead.

Example structure:

```txt
You are a local coding coworker helping another coding agent inspect files.

Answer the question using only the provided files.

Return valid JSON only.
Do not use Markdown.
Do not include comments.
Do not include explanatory text outside the JSON object.
Do not speculate beyond the provided files.
Keep output concise.

If the answer is not present in the files, set answer.not_found to true.

Question:
{question}

Expected schema:
{schema}

Files:
{files}
```

Each file is wrapped as:

```xml
<file path="src/foo.rs">
...
</file>
```

---

# File Processing Rules

## Binary files

Skipped automatically.

---

## Large files

If the total input exceeds `--max-bytes`:

* return structured error
* do not silently truncate in MVP
* do not chunk in MVP

---

## Encodings

MVP assumes UTF-8 text files.

Unsupported encodings are skipped.

---

## Symlinks

Ignored in MVP.

---

# Suggested Repository Integration

Example `AGENTS.md` policy:

```md
Before reading large files directly, use:

cowork ask --paths <paths> --question "<specific question>"

Rules:
- Use narrow questions
- Use cowork before reading more than 3 files
- Prefer cowork summaries before consuming large contexts
- Manually inspect suggested symbols and files afterward
```

---

# Future Subcommands

Planned future commands:

```bash
cowork plan
cowork review
cowork trace
cowork index
cowork handoff
```

Suggested meanings:

```txt
cowork ask      answer targeted repository questions
cowork plan     generate implementation plans
cowork review   review diffs or patches
cowork trace    trace symbols or flows through files
cowork index    build local summaries/cache
cowork handoff  generate structured agent handoffs
```

---

# Recommended Architecture

Suggested internal layout:

```txt
src/
  main.rs
  lib.rs
  cli.rs
  config.rs
  error.rs
  commands/
    mod.rs
    ask.rs
  files.rs
  prompt.rs
  model.rs
  output.rs
```

Notes:

* no cache module in MVP
* blocking HTTP is acceptable for MVP
* provider abstraction is out of scope for MVP

---

# MVP Success Criteria

`cowork ask` is successful if a coding agent can:

1. Ask a narrow repository question
2. Receive deterministic structured output
3. Identify relevant files and symbols
4. Reduce unnecessary large-context reads
5. Decide what to inspect manually next

---

# MVP Boundaries

Not in MVP:

* cache layer
* markdown output
* response chunking
* provider abstraction beyond Ollama-compatible endpoint
* `.gitignore`-aware traversal
* Windows support
