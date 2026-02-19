# ContextSmith v1 Technical Specification

Status: Draft
Owners: ContextSmith maintainers
Last Updated: 2026-02-19

## Problem Statement
AI agents and CI automation need context that is small, relevant, reproducible, and explainable. Existing packers often optimize for convenience, but not for deterministic inclusion behavior and auditable selection logic. ContextSmith v1 must be the deterministic pre-model compiler in those workflows.

## Goals
- Produce deterministic context bundles for diff and collect workflows.
- Maximize useful task signal under strict token budgets.
- Emit machine-readable manifests that explain inclusion/exclusion decisions.
- Provide benchmark evidence of quality-per-token uplift versus baselines.

## Non-Goals
- Embedding-based semantic retrieval in v1.
- IDE plugin implementation in v1.
- Broad graph platform features without benchmark-proven ROI.

## System Context
Inputs:
- Git repo state (working tree, staged, rev ranges).
- CLI command + flags.
- `contextsmith.toml` config.

Outputs:
- Context bundle (`markdown`, `json`, `plain`, `xml`).
- Manifest (`*.manifest.json`) with summary and entry-level decisions.
- Explain output (human-readable and optional JSON contract).
- Stats output for repo and bundle introspection.

## Architecture
Core components:
- Scanner: file discovery and filtering (gitignore + config ignore).
- Slicer: extract deterministic spans around reasons (diff, grep, symbol, file).
- Ranker: deterministic weighted scoring and stable tie-breaking.
- Packer: deterministic budgeted selection with must/drop constraints.
- Manifest writer: schema-stable, versioned metadata emission.
- Explain reader: deterministic rendering of reason and score vectors.
- Stats reader: aggregate repo and bundle measurements.

## Determinism Contract
Given identical:
- repository content and git metadata,
- command and flag set,
- config file,

ContextSmith must produce byte-identical:
- primary bundle output,
- manifest JSON,
- determinism fingerprint.

Required deterministic controls:
- Canonical path normalization.
- Stable sort keys for candidates and selected sections.
- Explicit tie-break order (score, reason priority, file path, range start, range end).
- Stable floating-point score serialization format.

## Data Contracts
Manifest summary additions:
- `schema_version: string`
- `selection_strategy: string`
- `determinism_fingerprint: string`

Manifest entry additions:
- `reason_codes: string[]`
- `rank_signals: object`
- `selected_by: string`

`selected_by` enum values:
- `must`
- `ranked`
- `first-snippet-guard`
- `manual`

Reason code shape:
- Stable, normalized enum-like strings (e.g., `diff_hunk`, `explicit_file`, `grep_match`, `symbol_definition`, `budget_excluded`, `drop_filter`).

## CLI and Output Compatibility
- Keep existing flags; no breaking flag renames/removals in v1.
- Add `--determinism-check`:
  - Executes twice in-memory on same input.
  - Fails with non-zero exit if fingerprints or outputs diverge.
- Keep existing output formats and sibling manifest naming convention.

## Execution Flows
Primary flow A:
1. `diff` (or `collect`) generates candidate sections.
2. Ranker computes deterministic scores.
3. Packer applies budget/must/drop policy.
4. Bundle + manifest are written.
5. `explain` and `stats` consume manifest.

Primary flow B:
1. `diff --format json --out bundle.json`
2. `pack bundle.json --budget N`
3. `explain packed.manifest.json`
4. `stats packed.manifest.json`

## Failure Modes and Edge Cases
- Clean repo: explicit no-change message; no synthetic bundle unless requested by caller behavior.
- No matches in collect: successful exit with no-match message.
- Missing files/bundle/manifest: non-zero with actionable path-specific errors.
- Invalid JSON input: non-zero parse failure with file path context.
- Tiny budget: still include first required snippet guard.
- Must/drop conflict: must precedence documented and reflected in `selected_by`.

## Observability and Metrics
Per run:
- command metadata
- model family
- budget and reserve
- candidate count
- included count
- total estimated tokens
- determinism fingerprint

Benchmark metrics:
- task success rate
- tokens sent
- estimated cost
- quality-per-token
- determinism pass rate

## Benchmark Harness Specification
Task sets:
- PR review
- bug fix localization
- targeted refactor

Baselines:
- Raw diff baseline.
- Repo packing baseline.

Matrix dimensions:
- budgets (small/medium/large)
- model families (`gpt-4` heuristic, `claude` heuristic)
- repository size classes

Output artifact:
- machine-readable benchmark report JSON
- human summary markdown

## Security and Privacy
- Local/offline operation by default.
- No external API calls required for core behavior.
- Respect ignore boundaries to reduce accidental secret inclusion.

## Acceptance Criteria
- Determinism tests pass across repeated invocations.
- Manifest schema versioned and backward-readable.
- Explain output maps to stable reason code taxonomy.
- Benchmark report shows non-regression and quality-per-token target attainment.

## Compatibility and Migration
- Version manifest with `schema_version`.
- Read older manifest versions with compatibility shims.
- Keep pre-existing fields stable in v1.
