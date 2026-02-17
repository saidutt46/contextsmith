//! Handler for the `contextsmith explain` command.
//!
//! Reads a manifest JSON file and prints a human-readable explanation
//! of what was included/excluded and why. Useful for debugging budget
//! decisions and understanding context assembly.

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
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
}
