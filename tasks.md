# ContextSmith v1 Task Backlog

Status: Draft
Ordering: Sequential by dependency

## Group A — Cleanup and Reliability (First Execution Priority)

### A1. Documentation Truth Alignment
Type: Documentation
Dependencies: none
Definition of Done:
- README, CHANGELOG, and test docs match actual implemented commands and behavior.
- No stale "not implemented" references for implemented commands.
Validation Commands:
- `rg -n "not yet implemented|stubbed" README.md CHANGELOG.md test-docs/*.md`
- manual verification of command status tables.

### A2. Test-Doc Expectation Correction
Type: Test Infrastructure
Dependencies: A1
Definition of Done:
- Phase test docs use current expected outputs.
- No false negatives due to stale assumptions.
Validation Commands:
- run documented smoke scripts and confirm expected labels.

### A3. Output and Error Contract Normalization
Type: CLI Contract
Dependencies: A1
Definition of Done:
- Summary/error lines consistently formatted across diff/collect/pack/explain/stats.
- Test assertions use stable phrases.
Validation Commands:
- targeted CLI integration tests for each command.

## Group B — Determinism and Schema Stabilization

### B1. Stable Ordering Contract
Type: Core Logic
Dependencies: A3
Definition of Done:
- Candidate and output ordering deterministic and documented.
- Tie-break logic explicit in code and tests.
Validation Commands:
- repeated command runs with output hash comparison.

### B2. Determinism Fingerprint
Type: Core Logic
Dependencies: B1
Definition of Done:
- Manifest includes `determinism_fingerprint`.
- Fingerprint is stable for identical inputs.
Validation Commands:
- repeat-run fingerprint equality test.

### B3. Manifest Schema v1 Fields
Type: Data Contract
Dependencies: B2
Definition of Done:
- Add `schema_version`, `selection_strategy`, `reason_codes`, `rank_signals`, `selected_by`.
- Backward read compatibility preserved.
Validation Commands:
- schema serialization/deserialization tests.
- compatibility tests with old manifest fixtures.

### B4. Explain Contract Upgrade
Type: UX/CLI Contract
Dependencies: B3
Definition of Done:
- Explain maps stable reason labels from reason codes.
- JSON explain mode supports downstream tooling.
Validation Commands:
- explain command tests with detailed + weights + json outputs.

### B5. Determinism Check Mode
Type: CLI Feature
Dependencies: B2
Definition of Done:
- New `--determinism-check` mode added and documented.
- Non-zero exit on divergence.
Validation Commands:
- deterministic and forced-divergence tests.

## Group C — Benchmark Harness and Competitive Tracking

### C1. Benchmark Fixture Corpus
Type: Evaluation
Dependencies: B3
Definition of Done:
- Curated fixture repos/tasks for PR review, bug fix, refactor.
Validation Commands:
- fixture integrity check scripts.

### C2. Baseline Runner
Type: Evaluation Tooling
Dependencies: C1
Definition of Done:
- Automated runner executes ContextSmith + baseline methods.
- Captures tokens, section counts, and output artifacts.
Validation Commands:
- benchmark runner end-to-end execution.

### C3. Quality-per-Token Scoring Pipeline
Type: Evaluation
Dependencies: C2
Definition of Done:
- Scoring output includes quality-per-token and deltas vs baseline.
- Report formats: JSON + markdown summary.
Validation Commands:
- sample benchmark report generation.

### C4. KPI Regression Gate
Type: CI/Release
Dependencies: C3
Definition of Done:
- CI/pre-release gate blocks regressions beyond configured threshold.
Validation Commands:
- simulated regression run triggers failure.

## Group D — Adoption Integrations

### D1. GitHub Action Workflow Templates
Type: Integration Docs/Tooling
Dependencies: C2
Definition of Done:
- Example workflows for diff->pack->explain and collect->pack pipelines.
Validation Commands:
- workflow lint and dry-run verification.

### D2. Agent Integration Recipe
Type: Integration Docs
Dependencies: B4
Definition of Done:
- Minimal machine-readable integration guide with manifest/JSON contracts.
Validation Commands:
- sample integration run reproduces documented output.

### D3. 30-Minute Onboarding Path
Type: Product Docs
Dependencies: D1, D2
Definition of Done:
- End-to-end quickstart validated on clean environment.
Validation Commands:
- follow quickstart exactly; expected artifacts created.

## Milestone Mapping
- M1: A1-A3
- M2: B1-B5
- M3: C1-C4
- M4: D1-D3

## Execution Notes
- Treat currently implemented functionality as baseline.
- New feature work is blocked until Group A cleanup is complete.
- Ranking/selection changes must include benchmark evidence and determinism proof.
