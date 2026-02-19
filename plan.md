# ContextSmith v1 Delivery Plan

Status: Draft
Last Updated: 2026-02-19

## Milestone Overview
- M1: Reliability cleanup and deterministic hardening foundations.
- M2: Determinism guarantees and schema stabilization.
- M3: Benchmark harness and baseline comparisons.
- M4: Adoption integrations and KPI-driven ranking improvements.

## M1 — Reliability Cleanup and Truth Alignment
Objectives:
- Align docs, tests, and runtime behavior.
- Remove stale expectations (especially command status drift).
- Ensure command contracts are explicit and reproducible.

Exit Gates:
- README/CHANGELOG/test-docs reflect current command behavior.
- No contradictory claims (e.g., implemented vs stubbed).
- CI checks pass (`fmt`, `clippy`, `test`).

## M2 — Determinism Guarantees and Manifest Schema v1
Objectives:
- Implement deterministic fingerprint and stable serialization rules.
- Add schema/version fields and rank signal metadata.
- Add deterministic golden/snapshot tests.

Exit Gates:
- Repeat-run byte-equality tests pass on fixture repos.
- `--determinism-check` behavior defined and tested.
- Manifest schema compatibility tests pass.

## M3 — Benchmark Harness and Comparative Reporting
Objectives:
- Build benchmark corpus and reproducible runner.
- Compare against raw diff and one repo-packer baseline.
- Track quality-per-token uplift and non-regression.

Exit Gates:
- Benchmark runner reproducible in CI and locally.
- Baseline comparison report generated automatically.
- Hard KPI gate enforced for merges affecting ranking/packing.

## M4 — Adoption Integrations + KPI-Driven Improvements
Objectives:
- Publish CI and agent integration templates.
- Improve ranking signals only when benchmark-positive.
- Keep deterministic guarantees unchanged.

Exit Gates:
- 30-minute integration path documented and validated.
- At least 3 external repos can run canonical workflow.
- Ranking changes merge only with KPI improvement evidence.

## Measurement Cadence
Measure at these intervals:
- End of each milestone.
- Every ranking logic change.
- Every manifest schema change.

Required metrics per checkpoint:
- Determinism pass rate.
- Candidate vs included section counts.
- Total token estimate.
- Task success proxy score.
- Quality-per-token delta vs baselines.

## Benchmark Matrix
Dimensions:
- Task classes: PR review, bug fix, refactor.
- Budgets: low, medium, high.
- Model profiles: gpt-4 heuristic, claude heuristic.
- Repo classes: small, medium, large.

Baselines:
- Raw diff baseline.
- Repo packer baseline.

## Risk Register
- Scope creep risk: adding broad features without KPI proof.
  - Mitigation: KPI gate required before expansion.
- Determinism regression risk from ranking changes.
  - Mitigation: mandatory determinism suite on every PR touching selection logic.
- Doc drift risk.
  - Mitigation: docs and test-doc updates required in same PR.

## Rollback and Containment Rules
- If determinism fails: block merge and revert change set.
- If quality-per-token regresses beyond threshold: feature flag off or rollback.
- If schema compatibility breaks: hold release until migration shim lands.

## Release Readiness Criteria
Release v1 only if all are true:
- Determinism suite green.
- Benchmark report meets/non-regresses KPI thresholds.
- Schema versioning and compatibility tests green.
- Docs and examples reflect current behavior.
