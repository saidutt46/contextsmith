//! Handler for the `contextsmith explain` command.
//!
//! Reads a manifest JSON file and prints a human-readable explanation
//! of what was included/excluded and why. Useful for debugging budget
//! decisions and understanding context assembly.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::error::{ContextSmithError, Result};
use crate::manifest::{self, Manifest};

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// All inputs needed to run the explain command.
#[derive(Debug)]
pub struct ExplainCommandOptions {
    /// Path to manifest.json or directory containing it.
    pub bundle: Option<PathBuf>,
    /// Show detailed scoring information.
    pub detailed: bool,
    /// Limit to top N entries.
    pub top: Option<usize>,
    /// Print ranking weights used.
    pub show_weights: bool,
    /// Suppress non-essential output.
    pub quiet: bool,
}

/// Run the explain command.
pub fn run(options: ExplainCommandOptions) -> Result<()> {
    // Step 1: Resolve manifest path.
    let manifest_path = resolve_manifest_path(options.bundle.as_deref())?;
    let manifest = manifest::read_manifest(&manifest_path)?;

    // Step 2: Show weights if requested.
    if options.show_weights {
        print_weights(&manifest);
    }

    // Step 3: Sort entries by score descending.
    let mut entries = manifest.entries.clone();
    sort_entries_for_display(&mut entries);

    // Limit to top N if requested.
    if let Some(top) = options.top {
        entries.truncate(top);
    }

    // Step 4: Print entries.
    for entry in &entries {
        let status = if entry.included {
            "included".green().to_string()
        } else {
            "excluded".dimmed().to_string()
        };

        let location = if entry.start_line > 0 {
            format!(
                "{}:{}-{}",
                entry.file_path, entry.start_line, entry.end_line
            )
        } else {
            entry.file_path.clone()
        };

        println!(
            "  {} ({} tokens, {})  {}",
            location.bold(),
            entry.token_estimate,
            status,
            entry.reason.dimmed(),
        );

        if options.detailed {
            println!(
                "    chars: {}, score: {:.2}, lang: {}",
                entry.char_count, entry.score, entry.language,
            );
        }
    }

    // Step 5: Print footer.
    println!();
    let summary = &manifest.summary;
    let budget_info = match summary.budget {
        Some(b) => format!(" / {b} budget"),
        None => String::new(),
    };
    println!(
        "{} ~{} tokens{}, {} of {} snippet{} included",
        "summary:".green().bold(),
        summary.total_tokens,
        budget_info,
        summary.included_count,
        summary.snippet_count,
        if summary.snippet_count == 1 { "" } else { "s" },
    );

    if summary.reserve_tokens > 0 {
        println!("  reserve: {} tokens", summary.reserve_tokens);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sort entries for deterministic explain output.
///
/// Primary key is score descending. Ties are broken by file path,
/// start/end line, reason, token estimate, and language so repeated runs
/// produce identical output ordering.
fn sort_entries_for_display(entries: &mut [crate::manifest::ManifestEntry]) {
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.start_line.cmp(&b.start_line))
            .then_with(|| a.end_line.cmp(&b.end_line))
            .then_with(|| a.reason.cmp(&b.reason))
            .then_with(|| a.token_estimate.cmp(&b.token_estimate))
            .then_with(|| a.language.cmp(&b.language))
    });
}

/// Resolve the manifest path from user input.
///
/// - `Some(file.json)` → use directly
/// - `Some(directory)` → look for `manifest.json` in it
/// - `None` → `./manifest.json`
fn resolve_manifest_path(input: Option<&Path>) -> Result<PathBuf> {
    match input {
        Some(p) => {
            if p.is_dir() {
                let candidate = p.join("manifest.json");
                if candidate.exists() {
                    Ok(candidate)
                } else {
                    Err(ContextSmithError::invalid_path(
                        p.to_string_lossy(),
                        "no manifest.json found in directory",
                    ))
                }
            } else {
                Ok(p.to_path_buf())
            }
        }
        None => {
            let default = PathBuf::from("manifest.json");
            if default.exists() {
                Ok(default)
            } else {
                Err(ContextSmithError::invalid_path(
                    "manifest.json",
                    "no manifest.json found in current directory; specify a path",
                ))
            }
        }
    }
}

/// Print ranking weights from the manifest.
fn print_weights(manifest: &Manifest) {
    match &manifest.summary.weights_used {
        Some(w) => {
            println!("{}", "Ranking weights:".bold());
            println!("  text:      {:.2}", w.text);
            println!("  diff:      {:.2}", w.diff);
            println!("  recency:   {:.2}", w.recency);
            println!("  proximity: {:.2}", w.proximity);
            println!("  test:      {:.2}", w.test);
            println!();
        }
        None => {
            println!(
                "{}",
                "No ranking weights recorded (order-based selection).".dimmed()
            );
            println!();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ManifestEntry;

    #[test]
    fn resolve_manifest_path_with_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my-manifest.json");
        std::fs::write(&path, "{}").unwrap();

        let resolved = resolve_manifest_path(Some(&path)).unwrap();
        assert_eq!(resolved, path);
    }

    #[test]
    fn resolve_manifest_path_with_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("manifest.json"), "{}").unwrap();

        let resolved = resolve_manifest_path(Some(dir.path())).unwrap();
        assert_eq!(resolved, dir.path().join("manifest.json"));
    }

    #[test]
    fn resolve_manifest_path_directory_without_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_manifest_path(Some(dir.path()));
        assert!(result.is_err());
    }

    #[test]
    fn sort_entries_for_display_is_deterministic_on_ties() {
        let mut entries = vec![
            ManifestEntry {
                file_path: "b.rs".to_string(),
                start_line: 10,
                end_line: 20,
                token_estimate: 5,
                char_count: 20,
                reason: "r".to_string(),
                score: 1.0,
                included: true,
                language: "rust".to_string(),
            },
            ManifestEntry {
                file_path: "a.rs".to_string(),
                start_line: 10,
                end_line: 20,
                token_estimate: 5,
                char_count: 20,
                reason: "r".to_string(),
                score: 1.0,
                included: true,
                language: "rust".to_string(),
            },
        ];

        sort_entries_for_display(&mut entries);
        assert_eq!(entries[0].file_path, "a.rs");
        assert_eq!(entries[1].file_path, "b.rs");
    }

    #[test]
    fn sort_entries_for_display_prefers_higher_score() {
        let mut entries = vec![
            ManifestEntry {
                file_path: "a.rs".to_string(),
                start_line: 1,
                end_line: 1,
                token_estimate: 1,
                char_count: 1,
                reason: "r".to_string(),
                score: 0.1,
                included: true,
                language: "rust".to_string(),
            },
            ManifestEntry {
                file_path: "z.rs".to_string(),
                start_line: 1,
                end_line: 1,
                token_estimate: 1,
                char_count: 1,
                reason: "r".to_string(),
                score: 0.9,
                included: true,
                language: "rust".to_string(),
            },
        ];

        sort_entries_for_display(&mut entries);
        assert_eq!(entries[0].score, 0.9);
        assert_eq!(entries[0].file_path, "z.rs");
    }
}
