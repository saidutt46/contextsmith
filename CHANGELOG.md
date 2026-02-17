# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`contextsmith init`** — bootstrap a project with `contextsmith.toml` config and `.contextsmith/cache/` directory
  - `--root` to specify project directory
  - `--force` to overwrite existing config
  - `--no-cache` to skip cache directory creation
- **`contextsmith diff`** — gather context from git changes with smart slicing
  - Supports `--staged`, `--untracked`, `--since`, revision ranges (e.g. `HEAD~3..HEAD`)
  - `--context N` controls lines of surrounding context (default 3)
  - `--hunks-only` for raw diff output without file context
  - `--format markdown|json|plain|xml` output formats
  - `--out <file>` and `--stdout` output destinations
  - `--include-related` accepted but not yet functional
- **`contextsmith diff --budget`** — token-aware budget enforcement on diff output
  - Greedily includes snippets until budget is reached (always includes at least one)
  - Writes `manifest.json` sibling file alongside `--out` output
  - Token count shown in summary output
- **Token estimation** — trait-based architecture with character heuristic default
  - `TokenEstimator` trait for pluggable tokenizer backends
  - Built-in `CharEstimator` with per-model-family ratios (GPT-4: ~4, Claude: ~3.5 chars/token)
  - `ModelFamily` enum: Gpt4, Gpt35, Claude, Unknown
- **Manifest system** — structured metadata for context bundles
  - `Manifest`, `ManifestSummary`, `ManifestEntry` types with full JSON serialization
  - Tracks token estimates, inclusion status, scores, and reasons for every snippet
- **`contextsmith pack`** — repack a JSON bundle into a token-budgeted output
  - Reads JSON bundle (from `diff --format json --out`)
  - `--budget`, `--chars`, `--model`, `--reserve` for budget control
  - `--must` to force-include files, `--drop` to exclude files
  - Greedy packing strategy with manifest output
- **`contextsmith explain`** — manifest introspection and debugging
  - Reads manifest JSON and prints human-readable inclusion/exclusion report
  - `--top N` to limit output, `--detailed` for scoring info
  - `--show-weights` to display ranking weights
  - Supports file path or directory input (auto-discovers `manifest.json`)
- **Config system** — `contextsmith.toml` with ignore patterns, generated file patterns, token budgets, ranking weights, language definitions, cache settings
- **Error handling** — structured error types with semantic helpers (`is_user_error()`, `is_retryable()`)
- **CLI skeleton** — all 8 subcommands defined with full argument parsing: init, diff, collect, pack, trim, map, stats, explain
- **Output formatters** — markdown (LLM-ready), JSON (machine-readable), plain text, XML

### Not Yet Implemented

- `collect`, `trim`, `map`, `stats` commands (return "not yet implemented" error)
- AST parsing and symbol expansion
- Ranking and scoring (currently order-based)
