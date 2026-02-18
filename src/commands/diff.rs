//! Handler for the `contextsmith diff` command.
//!
//! Orchestrates the diff pipeline: runs git to obtain changes, slices
//! context around each hunk, builds an output bundle, and writes the
//! result in the user's chosen format.

use std::path::PathBuf;

use colored::Colorize;
use tracing::warn;

use crate::cli::OutputFormat;
use crate::error::Result;
use crate::git::{self, DiffOptions, FileStatus};
use crate::manifest::{self, ManifestEntry};
use crate::output::{self, Bundle, BundleSection, FormatOptions};
use crate::slicer::{self, SliceOptions, Snippet};
use crate::tokens::{self, TokenEstimator};
use crate::utils;

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// All inputs needed to run the diff command.
#[derive(Debug)]
pub struct DiffCommandOptions {
    /// Repository root directory.
    pub root: PathBuf,
    /// Optional revision range (e.g. "HEAD~3..HEAD").
    pub rev_range: Option<String>,
    /// Diff only staged changes.
    pub staged: bool,
    /// Include untracked files.
    pub untracked: bool,
    /// Base reference or duration for filtering.
    pub since: Option<String>,
    /// Only include raw hunk content, no file context.
    pub hunks_only: bool,
    /// Number of context lines around each hunk.
    pub context_lines: usize,
    /// Pull in related symbols (currently stubbed).
    pub include_related: bool,
    /// Output format.
    pub format: OutputFormat,
    /// Write output to this file path.
    pub out: Option<PathBuf>,
    /// Write output to stdout.
    pub stdout: bool,
    /// Suppress non-essential output.
    pub quiet: bool,
    /// Token budget — if set, greedily include snippets until budget fills.
    pub budget: Option<usize>,
    /// Model name for token estimation.
    pub model: Option<String>,
}

/// Run the diff command end-to-end.
pub fn run(options: DiffCommandOptions) -> Result<()> {
    // Warn about stubbed functionality.
    if options.include_related {
        warn!("--include-related is not yet implemented; ignoring");
    }

    // Step 1: Get parsed diff from git.
    let diff_files = git::get_diff(&DiffOptions {
        root: options.root.clone(),
        rev_range: options.rev_range,
        staged: options.staged,
        untracked: options.untracked,
        since: options.since,
    })?;

    if diff_files.is_empty() {
        if !options.quiet {
            println!("{}", "No changes found.".dimmed());
        }
        return Ok(());
    }

    // Step 2: Slice context around hunks.
    let snippets = slicer::slice_diff_hunks(
        &diff_files,
        &SliceOptions {
            context_lines: options.context_lines,
            hunks_only: options.hunks_only,
            root: options.root,
        },
    )?;

    // Step 3: Apply budget if set.
    let model = options
        .model
        .as_deref()
        .map(tokens::parse_model)
        .unwrap_or(tokens::ModelFamily::Gpt4);
    let estimator = tokens::CharEstimator::new(model);

    let (included_snippets, manifest_entries) =
        apply_budget_and_build_entries(&snippets, &estimator, options.budget);

    // Step 4: Build a bundle from included snippets.
    let bundle = build_bundle(&diff_files, included_snippets);

    // Step 5: Format and write output.
    let format = utils::cli_format_to_output_format(&options.format);
    let formatted = output::format_bundle(&bundle, format)?;
    output::write_output(
        &formatted,
        &FormatOptions {
            format,
            stdout: options.stdout,
            out: options.out.clone(),
        },
    )?;

    // Step 6: Write manifest as sibling file when --out is specified.
    if let Some(ref out_path) = options.out {
        let manifest =
            manifest::build_manifest(manifest_entries, estimator.model_name(), options.budget, 0);
        let manifest_path = utils::manifest_sibling_path(out_path);
        manifest::write_manifest(&manifest, &manifest_path)?;
        if !options.quiet {
            eprintln!(
                "{} manifest written to {}",
                "ok:".green().bold(),
                manifest_path.display()
            );
        }
    }

    // Step 7: Print summary to stderr (unless writing to stdout or quiet).
    if !options.quiet && !options.stdout {
        let total_tokens: usize = manifest_entries_total_tokens(&snippets, &estimator);
        print_summary(&diff_files, total_tokens, options.budget);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply budget constraints and build manifest entries for all snippets.
///
/// Returns the included snippets and manifest entries for every snippet.
/// If no budget is set, all snippets are included.
/// Always includes at least one snippet even if it exceeds the budget.
fn apply_budget_and_build_entries(
    snippets: &[Snippet],
    estimator: &dyn tokens::TokenEstimator,
    budget: Option<usize>,
) -> (Vec<Snippet>, Vec<ManifestEntry>) {
    let mut included = Vec::new();
    let mut entries = Vec::new();
    let mut tokens_used: usize = 0;

    for (i, snippet) in snippets.iter().enumerate() {
        let token_est = estimator.estimate(&snippet.content);
        let char_count = snippet.content.len();

        let is_included = match budget {
            None => true,
            Some(b) => {
                // Always include at least one snippet.
                if included.is_empty() {
                    true
                } else {
                    tokens_used + token_est <= b
                }
            }
        };

        if is_included {
            tokens_used += token_est;
            included.push(snippet.clone());
        }

        entries.push(ManifestEntry {
            file_path: snippet.file_path.clone(),
            start_line: snippet.start_line,
            end_line: snippet.end_line,
            token_estimate: token_est,
            char_count,
            reason: snippet.reason.clone(),
            score: (snippets.len() - i) as f64, // order-based score
            included: is_included,
            language: utils::infer_language(&snippet.file_path),
        });
    }

    (included, entries)
}

/// Total tokens across all snippets (used for summary display).
fn manifest_entries_total_tokens(
    snippets: &[Snippet],
    estimator: &dyn tokens::TokenEstimator,
) -> usize {
    snippets
        .iter()
        .map(|s| estimator.estimate(&s.content))
        .sum()
}

/// Build an output [`Bundle`] from diff files and extracted snippets.
fn build_bundle(diff_files: &[git::DiffFile], snippets: Vec<Snippet>) -> Bundle {
    let file_count = diff_files.len();
    let hunk_count: usize = diff_files.iter().map(|f| f.hunks.len()).sum();

    let sections: Vec<BundleSection> = snippets
        .into_iter()
        .map(|s| BundleSection {
            language: utils::infer_language(&s.file_path),
            file_path: s.file_path,
            content: s.content,
            reason: s.reason,
        })
        .collect();

    Bundle {
        summary: format!(
            "{} file{} changed, {} hunk{}, {} snippet{}",
            file_count,
            if file_count == 1 { "" } else { "s" },
            hunk_count,
            if hunk_count == 1 { "" } else { "s" },
            sections.len(),
            if sections.len() == 1 { "" } else { "s" },
        ),
        sections,
    }
}

/// Print a coloured summary of the diff to stderr.
fn print_summary(diff_files: &[git::DiffFile], total_tokens: usize, budget: Option<usize>) {
    let added = diff_files
        .iter()
        .filter(|f| f.status == FileStatus::Added)
        .count();
    let modified = diff_files
        .iter()
        .filter(|f| f.status == FileStatus::Modified)
        .count();
    let deleted = diff_files
        .iter()
        .filter(|f| f.status == FileStatus::Deleted)
        .count();
    let renamed = diff_files
        .iter()
        .filter(|f| f.status == FileStatus::Renamed)
        .count();
    let total_hunks: usize = diff_files.iter().map(|f| f.hunks.len()).sum();

    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("{added} added"));
    }
    if modified > 0 {
        parts.push(format!("{modified} modified"));
    }
    if deleted > 0 {
        parts.push(format!("{deleted} deleted"));
    }
    if renamed > 0 {
        parts.push(format!("{renamed} renamed"));
    }

    let budget_info = match budget {
        Some(b) => format!(", ~{total_tokens} tokens (budget: {b})"),
        None => format!(", ~{total_tokens} tokens"),
    };

    eprintln!(
        "{} {} file{} ({}), {} hunk{}{}",
        "diff:".green().bold(),
        diff_files.len(),
        if diff_files.len() == 1 { "" } else { "s" },
        parts.join(", "),
        total_hunks,
        if total_hunks == 1 { "" } else { "s" },
        budget_info,
    );
}
