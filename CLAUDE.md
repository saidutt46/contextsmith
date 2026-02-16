# ContextSmith — Project Conventions

## Architecture
- Single crate: `lib.rs` (library) + `main.rs` (binary)
- CLI: clap derive, global flags, command handlers in `src/commands/`
- Config: `contextsmith.toml`, serde Serialize+Deserialize, validate on load/save

## Error Handling
- `thiserror` for all error types in library code (`src/error.rs`)
- `anyhow` only in `main.rs` for top-level error handling
- No `.unwrap()` in library code

## Code Style
- `cargo clippy` clean, `cargo fmt` always
- Unit tests inline (`#[cfg(test)] mod tests`), integration tests in `tests/`
- Keep modules focused — one responsibility per file

## Commands
All subcommands live in `src/commands/`. Each has its own file.
Unimplemented commands return `ContextSmithError::NotImplemented`.

## Strict Rules

- **Never commit or push directly to `main`**. Always create a feature branch (e.g., `feature/*`, `fix/*`, `chore/*`) and open a PR.
- **Never include Claude's name, email, or any AI attribution** in commits, co-author lines, PR descriptions, or anywhere in the repo. No `Co-Authored-By` lines referencing Claude or Anthropic.
- **Never bump versions manually**. Use `cargo release` for all version management.
- **Never commit or push without explicit user permission**.

### Git Workflow

- **Use `gh` for all git-related operations** (PRs, issues, checks).
- **Do not squash commits** — always preserve full commit history.
- **Do not commit unless user finishes testing or explicitly asks**.
- **When starting to code** — always check if on `main`; if so, suggest creating a new branch before making any changes.
