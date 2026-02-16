# ContextSmith

A deterministic, token-aware context bundler for LLMs — "grep for LLMs."

Fast, offline, no LLM calls. Builds optimized context bundles from your codebase so you can feed the right code to the right model.

## Features

- **Diff-aware context** — extract minimal, relevant snippets from git changes
- **Multiple output formats** — Markdown (LLM-ready), JSON, plain text, XML
- **Smart slicing** — configurable context lines, overlapping hunk merging, hunks-only mode
- **Deterministic** — same repo state + same query = same output, every time
- **Offline** — no network calls, no LLM APIs, runs entirely locally

## Installation

### From source

```bash
git clone https://github.com/saidutt46/contextsmith.git
cd contextsmith
cargo install --path .
```

### Requirements

- Rust 1.70+
- Git (for `diff` command)

## Quick Start

```bash
# Initialize a project
contextsmith init

# See what changed (unstaged working tree diff)
contextsmith diff --stdout

# Diff staged changes in markdown format
contextsmith diff --staged --stdout

# Diff last 3 commits, write to file
contextsmith diff HEAD~3..HEAD --out context.md

# Tight context (1 line around each change)
contextsmith diff --context 1 --stdout

# Raw hunks only (no file reading)
contextsmith diff --hunks-only --stdout

# JSON output for tooling
contextsmith diff --format json --stdout
```

## Commands

| Command     | Alias | Status          | Description                                      |
|-------------|-------|-----------------|--------------------------------------------------|
| `init`      |       | Implemented     | Bootstrap config and cache directory              |
| `diff`      | `d`   | Implemented     | Gather context from git changes                   |
| `collect`   | `c`   | Not yet         | Collect context by query, symbols, or patterns    |
| `pack`      | `p`   | Not yet         | Pack snippets into a token-budgeted bundle        |
| `trim`      |       | Not yet         | Trim existing content to fit a budget             |
| `map`       |       | Not yet         | Generate project map (file tree, symbols, graph)  |
| `stats`     |       | Not yet         | Show statistics for a context bundle              |
| `explain`   | `e`   | Not yet         | Explain why each snippet was included             |

## `contextsmith diff`

```
contextsmith diff [REV_RANGE] [OPTIONS]
```

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--staged`          | Diff staged (index) changes only                |
| `--untracked`       | Include untracked files                         |
| `--since <ref>`     | Changes since a timestamp or ref                |
| `--hunks-only`      | Raw hunk content, no file context               |
| `--context <N>`     | Lines of context around changes (default: 3)    |
| `--include-related` | Pull in related symbols (not yet implemented)   |
| `--format <fmt>`    | `markdown` / `json` / `plain` / `xml`           |
| `--out <path>`      | Write output to file                            |
| `--stdout`          | Write to stdout                                 |

### Global Flags

| Flag               | Description                              |
|--------------------|------------------------------------------|
| `--root <path>`    | Project root directory                   |
| `--config <path>`  | Path to config file                      |
| `--no-cache`       | Disable caching                          |
| `--quiet`          | Suppress non-essential output            |
| `-v`, `-vv`, `-vvv`| Increase verbosity                       |
| `--color <mode>`   | `auto` / `always` / `never`             |
| `--json`           | Output as JSON                           |
| `--time`           | Show timing information                  |

## Configuration

Running `contextsmith init` creates a `contextsmith.toml` with sensible defaults:

```toml
ignore = ["node_modules", "target", "DerivedData", ".next", "dist", "build", ".contextsmith", "*.min.js", "*.map"]
generated = ["*.pb.rs", "*.pb.go", "*_pb2.py", "*.generated.*"]
default_budget = 12000
reserve_tokens = 500

[ranking_weights]
text = 1.0
diff = 2.0
recency = 0.5
proximity = 1.5
test = 0.8

[languages.rust]
extensions = ["rs"]

[languages.typescript]
extensions = ["ts", "tsx"]

[languages.python]
extensions = ["py"]

[cache]
enabled = true
```

## Project Status

ContextSmith is in early development. The `init` and `diff` commands are functional. Remaining commands will be implemented incrementally — see the [CHANGELOG](CHANGELOG.md) for details.

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
