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
- **Config system** — `contextsmith.toml` with ignore patterns, generated file patterns, token budgets, ranking weights, language definitions, cache settings
- **Error handling** — structured error types with semantic helpers (`is_user_error()`, `is_retryable()`)
- **CLI skeleton** — all 8 subcommands defined with full argument parsing: init, diff, collect, pack, trim, map, stats, explain
- **Output formatters** — markdown (LLM-ready), JSON (machine-readable), plain text, XML

### Not Yet Implemented

- `collect`, `pack`, `trim`, `map`, `stats`, `explain` commands (return "not yet implemented" error)
- Token counting and budget enforcement
- AST parsing and symbol expansion
- Ranking and scoring
