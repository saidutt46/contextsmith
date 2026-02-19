# Group A Progress Report

Date: 2026-02-19
Branch: `chore/group-a-cleanup-reliability`

## Scope in Progress
Focus: Group A (cleanup + reliability), with user-facing doc alignment and output-contract stability checks.

## Changes Completed
- Updated `README.md`:
  - Added `collect` command section.
  - Added `stats` command section.
  - Clarified which `collect` flags are wired vs accepted-but-currently-ignored.
- Added output-contract integration tests in `tests/cli_tests.rs`:
  - `diff_out_prints_manifest_and_summary_to_stderr`
  - `collect_files_output_prints_manifest_and_summary_to_stderr`
  - `pack_with_output_prints_manifest_and_summary_to_stderr`

## Measurable Verification

### 1) Command behavior parity check
Commands:
```bash
./target/debug/contextsmith collect --grep "Config" --root . --stdout
./target/debug/contextsmith collect --grep "Config" --root . --span "1:2" --stdout
```
Observed:
- `--span` appears in CLI help.
- Outputs are currently identical for these inputs, confirming docs accurately state `--span` is accepted but not wired.

### 2) Output contract stability tests
Added assertions that for `--out` workflows:
- stderr includes `manifest written to`
- stderr includes command summary prefix (`diff:`, `collect:`, `pack:`)
- budgeted commands include `(budget: N)` in summary output

### 3) Full validation gate
Command:
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
Result: PASS

Observed test totals:
- Unit tests: `105 passed`
- Integration tests: `47 passed` (was 44; +3 new A3 contract tests)
- Doctests: `1 passed`
- Total: `153 passed`, `0 failed`

## Current Status
- Group A A3 is in progress with concrete guardrails now added.
- Internal `test-docs/` not modified per instruction.

## Next Group A Steps
1. Review remaining user-facing docs for any command flag drift in tracked files only.
2. Decide whether to codify a small "command output contract" section in `README.md`.
3. Prepare a focused PR with these Group A changes.
