# ContextSmith

A deterministic, token-aware context bundler for LLMs — "grep for LLMs."

Fast, offline, no LLM calls. Builds optimized context bundles from your codebase so you can feed the right code to the right model.

## Features

- **Diff-aware context** — extract minimal, relevant snippets from git changes
- **Token budgeting** — fit context into model token limits with greedy packing
- **Manifest tracking** — know exactly what was included, excluded, and why
- **Multiple output formats** — Markdown (LLM-ready), JSON, plain text, XML
- **Smart slicing** — configurable context lines, overlapping hunk merging, hunks-only mode
- **Model-aware** — different token estimation ratios for GPT-4, Claude, etc.
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

- Rust 1.85+
- Git (for `diff` command)

## Quick Start

```bash
# Initialize a project
contextsmith init

# See what changed (unstaged working tree diff)
contextsmith diff --stdout

# Diff with a token budget (fits ~500 tokens of context)
contextsmith diff --budget 500 --stdout

# Diff to file (also creates a manifest.json sibling)
contextsmith diff --budget 2000 --out context.md

# Full pipeline: capture → pack → explain
contextsmith diff --format json --out bundle.json
contextsmith pack bundle.json --budget 1000 --model claude --out packed.md
contextsmith explain packed.manifest.json
```

## Commands

| Command     | Alias | Status          | Description                                      |
|-------------|-------|-----------------|--------------------------------------------------|
| `init`      |       | Implemented     | Bootstrap config and cache directory              |
| `diff`      | `d`   | Implemented     | Gather context from git changes with budget support |
| `pack`      | `p`   | Implemented     | Repack a bundle into a token-budgeted output      |
| `explain`   | `e`   | Implemented     | Show why each snippet was included or excluded    |
| `collect`   | `c`   | Implemented     | Collect context by query, symbols, or patterns    |
| `trim`      |       | Not yet         | Trim existing content to fit a budget             |
| `map`       |       | Not yet         | Generate project map (file tree, symbols, graph)  |
| `stats`     |       | Implemented     | Show statistics for a context bundle              |

## `contextsmith diff`

Gathers context from git changes with optional token budgeting.

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
| `--budget <N>`      | Token budget — greedily include snippets to fit  |
| `--include-related` | Pull in related symbols (not yet implemented)   |
| `--format <fmt>`    | `markdown` / `json` / `plain` / `xml`           |
| `--out <path>`      | Write output to file (also creates manifest.json) |
| `--stdout`          | Write to stdout                                 |

```bash
# Unstaged changes with budget
contextsmith diff --budget 2000 --stdout

# Staged changes only
contextsmith diff --staged --stdout

# Last 3 commits to file
contextsmith diff HEAD~3..HEAD --budget 4000 --out context.md

# Raw hunks, JSON format
contextsmith diff --hunks-only --format json --stdout
```

## `contextsmith pack`

Repacks a JSON bundle (from `diff --format json --out`) into a token-budgeted output. Useful for iterating on budget/filters without re-running git.

```
contextsmith pack <BUNDLE> [OPTIONS]
```

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--budget <N>`      | Token budget                                    |
| `--chars <N>`       | Character budget (converted to tokens)          |
| `--model <name>`    | Model for token estimation (`gpt-4`, `claude`)  |
| `--reserve <N>`     | Reserve tokens for model response               |
| `--must <path>`     | Force-include files matching this path           |
| `--drop <path>`     | Exclude files matching this path                |
| `--format <fmt>`    | `markdown` / `json` / `plain` / `xml`           |
| `--out <path>`      | Write output to file (also creates manifest.json) |
| `--stdout`          | Write to stdout                                 |

```bash
# Create a JSON bundle first
contextsmith diff --format json --out bundle.json

# Pack for GPT-4 with 4000 token budget
contextsmith pack bundle.json --budget 4000 --out context.md

# Pack for Claude with reserved response tokens
contextsmith pack bundle.json --budget 2000 --model claude --reserve 500 --stdout

# Force-include tests, exclude docs
contextsmith pack bundle.json --budget 3000 --must tests/ --drop docs/ --stdout
```

## `contextsmith explain`

Reads a manifest.json and prints a human-readable report of what was included/excluded and why.

```
contextsmith explain [MANIFEST] [OPTIONS]
```

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--top <N>`         | Show only the top N entries                     |
| `--detailed`        | Show char count, score, and language per entry   |
| `--show-weights`    | Print ranking weights used for selection         |

```bash
# Explain a manifest
contextsmith explain context.manifest.json

# Top 5 entries with full details
contextsmith explain context.manifest.json --detailed --top 5

# Pass a directory (auto-discovers manifest.json inside)
contextsmith explain ./output-dir/
```

## `contextsmith collect`

Collects context from explicit files, content patterns, or symbol definitions.

```
contextsmith collect [QUERY] [OPTIONS]
```

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--files <path>`    | Include explicit file(s) (repeatable)           |
| `--grep <pattern>`  | Search file content by pattern                  |
| `--symbol <name>`   | Search for symbol definitions                   |
| `--exclude <path>`  | Exclude matching paths (repeatable)             |
| `--lang <name>`     | Filter by language                              |
| `--path <pattern>`  | Filter by file path pattern                     |
| `--max-files <N>`   | Cap number of files considered                  |
| `--budget <N>`      | Token budget                                    |
| `--format <fmt>`    | `markdown` / `json` / `plain` / `xml`           |
| `--out <path>`      | Write output to file (also creates manifest.json) |
| `--stdout`          | Write to stdout                                 |

Accepted by CLI but currently not wired in command execution:

| Flag                | Current status                                  |
|---------------------|-------------------------------------------------|
| `--scope`           | Accepted; currently ignored                     |
| `--diff`            | Accepted; currently ignored                     |
| `--span`            | Accepted; currently ignored (`context_lines` fixed at 3) |
| `--max-snippets`    | Accepted; currently ignored                     |
| `--include-defs`    | Accepted; currently ignored                     |
| `--include-refs`    | Accepted; currently ignored                     |
| `--include-imports` | Accepted; currently ignored                     |
| `--tests`           | Accepted; currently ignored                     |
| `--rank`            | Accepted; currently ignored                     |

```bash
# Positional query (same as --grep)
contextsmith collect "Config" --stdout

# Symbol search with budget
contextsmith collect --symbol TokenEstimator --budget 500 --stdout

# Explicit files to JSON
contextsmith collect --files src/main.rs --files src/lib.rs --format json --stdout
```

## `contextsmith stats`

Shows repository or bundle statistics for tuning context budgets.

```
contextsmith stats [BUNDLE] [OPTIONS]
```

| Flag                | Description                                     |
|---------------------|-------------------------------------------------|
| `--top-files <N>`   | Show top N files/snippets by token count        |
| `--by-lang`         | Group stats by language                         |
| `--by-type`         | Group stats by file type                        |
| `--tokens`          | Estimate and display token counts               |

```bash
# Repository mode
contextsmith stats --root . --tokens --by-lang

# Bundle mode (manifest path)
contextsmith stats ./context.manifest.json --top-files 5
```

### Token Estimation

ContextSmith estimates tokens using character-count heuristics (no external tokenizer dependency):

| Model family | Chars/token | Notes |
|---|---|---|
| GPT-4 / GPT-3.5 | ~4.0 | Default when no `--model` given |
| Claude | ~3.5 | Slightly more tokens per character |
| Unknown | ~4.0 | Conservative fallback |

Accuracy is ±15-20% vs real BPE tokenizers — sufficient for budget planning. The trait-based design (`TokenEstimator`) supports plugging in real tokenizers later.

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

### Output Contract

For budgeted commands with `--out`, ContextSmith writes:

- main bundle to the requested output path,
- sibling manifest at `<stem>.manifest.json`,
- non-essential status lines to stderr (for example `ok: manifest written ...` and command summaries like `diff:`, `collect:`, `pack:`).

`--quiet` suppresses these non-essential stderr status lines.

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

ContextSmith is in active development. `init`, `diff`, `collect`, `pack`, `stats`, and `explain` are functional with 150 automated tests (105 unit + 44 integration + 1 doctest). Remaining planned commands are `trim` and `map`. See the [CHANGELOG](CHANGELOG.md) for details.

## License

Licensed under either of:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
