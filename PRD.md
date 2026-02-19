# ContextSmith PRD v1.0 — Deterministic Context Compiler for Agents

## Summary
ContextSmith will be positioned as a deterministic, token-efficient context compiler for AI agents and CI pipelines.
Current implemented work is treated as done baseline.
Roadmap is narrowed to features that improve answer quality per token and reproducibility, with hard cuts on broad platform ambitions in near-term phases.

## Product Thesis
- Teams and agents need reproducible, explainable context assembly from code changes and targeted retrieval.
- Existing tools are strong at repo packing, but weak at deterministic selection contracts and auditable inclusion logic.
- ContextSmith wins by being the trusted build step before model calls, not by becoming a general IDE assistant.

## Target Users
- Primary: Agent builders, CI/PR automation pipelines, internal developer platforms.
- Secondary: Power users running terminal-first workflows.
- Not primary in v1: IDE plugin-first users, semantic-search-first users.

## Success Metrics (Hard Gates)
- Quality-per-token uplift: On benchmark tasks, equal or better task success at lower token budgets vs baseline packers.
- Determinism: Same git state + same command + same config => byte-identical output and manifest.
- Operational reliability: Stable outputs and explain reasons in CI without flaky ordering.
- Adoption proof: At least 3 external repos using ContextSmith in automated workflows.

## Baseline Treated as Done
- init, diff, pack, explain.
- Budgeting, manifest generation, multi-format output.
- Phase 2 core: collect, stats, initial ranking integration (per current project state).

## Keep / Cut / Defer

| Category | Decision | Scope |
|---|---|---|
| Diff-first deterministic bundling | Keep | Core daily workflow and CI entrypoint |
| Collect/pack/explain/stats loop | Keep | Must be production-grade and stable |
| Manifest + explainability | Keep | First-class trust and debugging surface |
| Token budgeting by model family | Keep | Improve precision and reporting |
| CLI + machine-readable JSON | Keep | Primary interface for agents/tools |
| Tree-sitter multi-language expansion | Defer | Only after benchmark wins with lexical+diff signals |
| Graph/call-map heavy infrastructure | Defer | Keep lightweight until clear ROI |
| Embeddings/semantic search | Cut (v1) | Explicitly out-of-scope for v1 |
| IDE plugin development | Cut (v1) | Integrate via external adapters instead |
| Do-everything command sprawl | Cut | No new major commands before KPI wins |

## v1 Scope (Decision Complete)
- Focus commands: diff, collect, pack, explain, stats.
- Guarantee deterministic ordering and deterministic tie-breaking everywhere.
- Stabilize manifest schema and explain reason taxonomy.
- Add benchmark/eval harness and publish comparative results.
- Add integration surfaces that drive adoption without expanding core complexity.

## Public Interfaces and Types (Required Changes)
- Manifest schema versioning:
  - Add schema_version.
  - Add selection_strategy.
  - Add determinism_fingerprint (repo state + config hash + command hash).
- Manifest entry enrichments:
  - reason_codes as normalized enums.
  - rank_signals object with per-signal numeric values.
  - selected_by (must, ranked, first-snippet-guard, manual).
- Explain output contract:
  - Stable reason labels mapped from reason_codes.
  - Optional strict JSON mode for downstream tooling.
- CLI compatibility:
  - Keep existing flags.
  - Add no breaking renames in v1.
  - Add --determinism-check mode (re-run compare hash, non-zero on mismatch).
- Output contract:
  - Document canonical sorting keys for sections and entries.

## v1 Roadmap

## Phase A — Product Reliability Hardening
- Lock command semantics and deterministic ordering rules.
- Eliminate doc drift; align README/CHANGELOG/test docs with actual behavior.
- Add golden tests for output determinism and manifest schema snapshots.
- Exit criteria:
  - Determinism tests pass across repeated runs.
  - Docs match runtime behavior.

## Phase B — Evaluation and Proof
- Build benchmark corpus and task set:
  - PR review tasks,
  - bug-fix tasks,
  - refactor tasks.
- Define baselines:
  - repo packer baseline,
  - raw diff baseline.
- Report metrics:
  - task success rate,
  - tokens sent,
  - cost estimate,
  - re-run consistency.
- Exit criteria:
  - Demonstrated quality-per-token gain in benchmark report.

## Phase C — Integration for Adoption
- Ship official GitHub Action example workflows.
- Ship agent integration recipes:
  - one minimal MCP/tool wrapper,
  - one JSON pipeline recipe for CI bots.
- Publish drop-in adoption templates.
- Exit criteria:
  - External users can integrate in <30 minutes.

## Phase D — Ranking Improvements (Still Deterministic)
- Improve rank signals only if benchmark-proven:
  - lexical density normalization,
  - diff proximity,
  - path intent heuristics,
  - lightweight test proximity.
- No embeddings.
- Exit criteria:
  - Measured KPI improvement over prior version.

## Testing and Acceptance Criteria
- Unit:
  - deterministic sort and tie-break logic,
  - reason code mapping,
  - manifest schema version compatibility.
- Integration:
  - command-to-command pipeline stability (diff -> pack -> explain -> stats),
  - collect modes and filters with expected inclusion/exclusion.
- Golden:
  - byte-level output snapshots for fixed fixture repos.
- Property tests:
  - repeated identical invocations produce identical fingerprints.
- Acceptance:
  - all gates pass in CI,
  - benchmark report updated and shows non-regression on KPI.

## Distribution Plan
- Positioning statement:
  - Deterministic context compiler for agent and CI workflows.
- Publish 3 canonical workflows:
  - PR review context,
  - incident hotfix context,
  - regression debugging context.
- Offer migration guide:
  - From repo packer to deterministic compiler.

## Risks and Mitigations
- Risk: just another context tool perception.
- Mitigation: Publish objective benchmark deltas and determinism guarantees.
- Risk: scope creep from PRD breadth.
- Mitigation: KPI-gated roadmap; no new major surface without measured lift.
- Risk: adoption friction.
- Mitigation: Simple integration templates and strict JSON contracts.

## Assumptions and Defaults
- Assumption: Primary buyers/users are agent/CI workflow owners.
- Assumption: Offline deterministic behavior is a differentiator worth paying for.
- Default ranking strategy: deterministic lexical+diff signals only.
- Default output objective: maximize useful context under budget while preserving explainability.
- Default roadmap rule: if feature does not improve KPI or reliability, defer/cut.

## Explicit Non-Goals for v1
- Semantic embedding retrieval.
- IDE plugin implementation.
- Broad graph platform features beyond minimal deterministic ranking support.
- Expanding command count beyond current core set.
