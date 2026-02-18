//! Handler for the `contextsmith pack` command.
//!
//! Reads a JSON bundle (output of `diff --format json`) and repacks it
//! into a token-budgeted output. Supports `--must` and `--drop` filters,
//! and writes a manifest alongside file output.

use std::path::PathBuf;

use colored::Colorize;

use crate::cli::OutputFormat;
use crate::error::{ContextSmithError, Result};
use crate::manifest::{self, ManifestEntry};
use crate::output::{self, Bundle, BundleSection, FormatOptions};
use crate::tokens::{self, TokenEstimator};
use crate::utils;

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// All inputs needed to run the pack command.
#[derive(Debug)]
pub struct PackCommandOptions {
    /// Input JSON bundle file.
    pub bundle: Option<PathBuf>,
    /// Token budget.
    pub budget: Option<usize>,
    /// Character budget (alternative to token budget).
    pub chars: Option<usize>,
    /// Model name for token estimation.
    pub model: Option<String>,
    /// Reserve tokens for model response.
    pub reserve: Option<usize>,
    /// Packing strategy (only "greedy" for now).
    pub strategy: Option<String>,
    /// Must-include file paths.
    pub must: Vec<PathBuf>,
    /// File paths to exclude.
    pub drop: Vec<PathBuf>,
    /// Output format.
    pub format: OutputFormat,
    /// Write to stdout.
    pub stdout: bool,
    /// Write output to file.
    pub out: Option<PathBuf>,
    /// Suppress non-essential output.
    pub quiet: bool,
}

/// Run the pack command end-to-end.
pub fn run(options: PackCommandOptions) -> Result<()> {
    // Step 1: Read input bundle.
    let bundle_path = options
        .bundle
        .ok_or_else(|| ContextSmithError::validation("bundle", "input bundle file is required"))?;

    let content = std::fs::read_to_string(&bundle_path).map_err(|e| {
        ContextSmithError::io(format!("reading bundle '{}'", bundle_path.display()), e)
    })?;

    let input_bundle: Bundle = serde_json::from_str(&content).map_err(|e| {
        ContextSmithError::config_with_source(
            format!("failed to parse bundle '{}'", bundle_path.display()),
            e,
        )
    })?;

    if input_bundle.sections.is_empty() {
        if !options.quiet {
            eprintln!("{}", "No sections in bundle.".dimmed());
        }
        return Ok(());
    }

    // Step 2: Determine estimator and effective budget.
    let model = options
        .model
        .as_deref()
        .map(tokens::parse_model)
        .unwrap_or(tokens::ModelFamily::Gpt4);
    let estimator = tokens::CharEstimator::new(model);
    let reserve = options.reserve.unwrap_or(0);

    let effective_budget = options
        .budget
        .map(|b| b.saturating_sub(reserve))
        .or_else(|| {
            options
                .chars
                .map(|c| estimator.estimate(&"x".repeat(c)).saturating_sub(reserve))
        });

    // Step 3: Filter sections by --drop and --must.
    let drop_set: Vec<String> = options
        .drop
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let must_set: Vec<String> = options
        .must
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let filtered: Vec<&BundleSection> = input_bundle
        .sections
        .iter()
        .filter(|s| !drop_set.iter().any(|d| s.file_path.contains(d.as_str())))
        .collect();

    // Step 4: Greedy packing.
    let (included, entries) = greedy_pack(&filtered, &estimator, effective_budget, &must_set);

    // Step 5: Build output bundle.
    let output_bundle = Bundle {
        summary: format!(
            "{} section{} (packed from {})",
            included.len(),
            if included.len() == 1 { "" } else { "s" },
            input_bundle.sections.len(),
        ),
        sections: included,
    };

    // Step 6: Format and write.
    let format = utils::cli_format_to_output_format(&options.format);
    let formatted = output::format_bundle(&output_bundle, format)?;
    output::write_output(
        &formatted,
        &FormatOptions {
            format,
            stdout: options.stdout,
            out: options.out.clone(),
        },
    )?;

    // Step 7: Write manifest alongside output.
    if let Some(ref out_path) = options.out {
        let m = manifest::build_manifest(
            entries.clone(),
            estimator.model_name(),
            options.budget,
            reserve,
        );
        let manifest_path = utils::manifest_sibling_path(out_path);
        manifest::write_manifest(&m, &manifest_path)?;
        if !options.quiet {
            eprintln!(
                "{} manifest written to {}",
                "ok:".green().bold(),
                manifest_path.display()
            );
        }
    }

    // Step 8: Print summary.
    if !options.quiet && !options.stdout {
        let total_tokens: usize = entries
            .iter()
            .filter(|e| e.included)
            .map(|e| e.token_estimate)
            .sum();
        let budget_info = match effective_budget {
            Some(b) => format!(" (budget: {b})"),
            None => String::new(),
        };
        eprintln!(
            "{} {} of {} section{} included, ~{} tokens{}",
            "pack:".green().bold(),
            entries.iter().filter(|e| e.included).count(),
            entries.len(),
            if entries.len() == 1 { "" } else { "s" },
            total_tokens,
            budget_info,
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Greedy pack sections into a budget.
///
/// Must-include sections go first (always included), then remaining
/// sections in order until budget is exhausted. Always includes at
/// least one section.
fn greedy_pack(
    sections: &[&BundleSection],
    estimator: &dyn TokenEstimator,
    budget: Option<usize>,
    must_paths: &[String],
) -> (Vec<BundleSection>, Vec<ManifestEntry>) {
    let mut included = Vec::new();
    let mut entries = Vec::new();
    let mut tokens_used: usize = 0;

    // Separate must-include and optional sections.
    let (must_sections, optional_sections): (Vec<&&BundleSection>, Vec<&&BundleSection>) = sections
        .iter()
        .partition(|s| must_paths.iter().any(|m| s.file_path.contains(m.as_str())));

    // Process must-include first.
    for section in &must_sections {
        let token_est = estimator.estimate(&section.content);
        tokens_used += token_est;
        included.push((**section).clone());
        entries.push(make_entry(section, token_est, true, "must-include"));
    }

    // Then optional sections with budget enforcement.
    for section in &optional_sections {
        let token_est = estimator.estimate(&section.content);

        let is_included = match budget {
            None => true,
            Some(b) => {
                if included.is_empty() {
                    true // Always include at least one section.
                } else {
                    tokens_used + token_est <= b
                }
            }
        };

        if is_included {
            tokens_used += token_est;
            included.push((**section).clone());
        }

        entries.push(make_entry(section, token_est, is_included, &section.reason));
    }

    (included, entries)
}

/// Build a manifest entry from a bundle section.
fn make_entry(
    section: &BundleSection,
    token_estimate: usize,
    included: bool,
    reason: &str,
) -> ManifestEntry {
    ManifestEntry {
        file_path: section.file_path.clone(),
        start_line: 0,
        end_line: 0,
        token_estimate,
        char_count: section.content.len(),
        reason: reason.to_string(),
        score: 0.0,
        included,
        language: section.language.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sections() -> Vec<BundleSection> {
        vec![
            BundleSection {
                file_path: "src/main.rs".to_string(),
                language: "rust".to_string(),
                content: "fn main() { println!(\"hello\"); }".to_string(), // 33 chars
                reason: "modified".to_string(),
            },
            BundleSection {
                file_path: "src/lib.rs".to_string(),
                language: "rust".to_string(),
                content: "pub mod config;".to_string(), // 15 chars
                reason: "modified".to_string(),
            },
            BundleSection {
                file_path: "tests/test.rs".to_string(),
                language: "rust".to_string(),
                content: "#[test] fn it_works() { assert!(true); }".to_string(), // 41 chars
                reason: "added".to_string(),
            },
        ]
    }

    #[test]
    fn greedy_pack_no_budget_includes_all() {
        let sections = sample_sections();
        let refs: Vec<&BundleSection> = sections.iter().collect();
        let estimator = tokens::default_estimator();
        let (included, entries) = greedy_pack(&refs, &estimator, None, &[]);
        assert_eq!(included.len(), 3);
        assert!(entries.iter().all(|e| e.included));
    }

    #[test]
    fn greedy_pack_tight_budget_limits() {
        let sections = sample_sections();
        let refs: Vec<&BundleSection> = sections.iter().collect();
        let estimator = tokens::default_estimator();
        // Budget of 10 tokens (~40 chars with GPT-4). First section is 33 chars = 9 tokens.
        // Second is 15 chars = 4 tokens. 9 + 4 = 13 > 10, so only first included.
        let (included, entries) = greedy_pack(&refs, &estimator, Some(10), &[]);
        assert_eq!(included.len(), 1);
        assert_eq!(included[0].file_path, "src/main.rs");
        assert_eq!(entries.iter().filter(|e| e.included).count(), 1);
    }

    #[test]
    fn greedy_pack_always_includes_one() {
        let sections = sample_sections();
        let refs: Vec<&BundleSection> = sections.iter().collect();
        let estimator = tokens::default_estimator();
        // Budget of 1 — still includes at least one.
        let (included, _) = greedy_pack(&refs, &estimator, Some(1), &[]);
        assert!(!included.is_empty());
    }

    #[test]
    fn greedy_pack_must_include() {
        let sections = sample_sections();
        let refs: Vec<&BundleSection> = sections.iter().collect();
        let estimator = tokens::default_estimator();
        let must = vec!["tests/test.rs".to_string()];
        // Tight budget: must-include goes first, then greedy.
        let (included, entries) = greedy_pack(&refs, &estimator, Some(12), &must);
        // test.rs is must-include (11 tokens), then main.rs (9 tokens) would exceed 12.
        assert!(included.iter().any(|s| s.file_path == "tests/test.rs"));
        assert!(
            entries
                .iter()
                .find(|e| e.file_path == "tests/test.rs")
                .unwrap()
                .included
        );
    }

    #[test]
    fn greedy_pack_drop_filters() {
        let sections = sample_sections();
        // Filter out tests before packing.
        let refs: Vec<&BundleSection> = sections
            .iter()
            .filter(|s| !s.file_path.contains("tests/"))
            .collect();
        let estimator = tokens::default_estimator();
        let (included, _) = greedy_pack(&refs, &estimator, None, &[]);
        assert_eq!(included.len(), 2);
        assert!(!included.iter().any(|s| s.file_path.contains("tests/")));
    }
}
