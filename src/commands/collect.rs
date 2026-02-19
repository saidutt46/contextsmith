//! Handler for the `contextsmith collect` command.
//!
//! Collects context from the codebase using explicit file paths (`--files`),
//! content search (`--grep`), or symbol search (`--symbol`). Outputs a
//! token-budgeted bundle with manifest.

use std::path::PathBuf;

use colored::Colorize;

use crate::cli::OutputFormat;
use crate::config::Config;
use crate::error::{ContextSmithError, Result};
use crate::indexer;
use crate::manifest::{self, ManifestEntry};
use crate::output::{self, Bundle, BundleSection, FormatOptions};
use crate::ranker;
use crate::scanner;
use crate::symbols::{RegexSymbolFinder, SymbolFinder};
use crate::tokens::{self, TokenEstimator};
use crate::utils;

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// All inputs needed to run the collect command.
#[derive(Debug)]
pub struct CollectCommandOptions {
    /// Repository root directory.
    pub root: PathBuf,
    /// Specific files to include.
    pub files: Vec<PathBuf>,
    /// Search by content pattern (grep).
    pub grep: Option<String>,
    /// Search for symbol definitions.
    pub symbol: Option<String>,
    /// Exclude patterns.
    pub exclude: Vec<String>,
    /// Filter by language.
    pub lang: Option<String>,
    /// Filter by file path pattern.
    pub path: Option<String>,
    /// Lines of context around grep matches.
    pub context_lines: usize,
    /// Max files to include.
    pub max_files: Option<usize>,
    /// Output format.
    pub format: OutputFormat,
    /// Write output to file.
    pub out: Option<PathBuf>,
    /// Write to stdout.
    pub stdout: bool,
    /// Suppress non-essential output.
    pub quiet: bool,
    /// Token budget.
    pub budget: Option<usize>,
    /// Model name for token estimation.
    pub model: Option<String>,
    /// Path to config file.
    pub config_path: Option<PathBuf>,
}

/// Collect mode — at least one must be specified.
#[derive(Debug)]
enum CollectMode {
    Files,
    Grep,
    Symbol,
}

/// Run the collect command.
pub fn run(options: CollectCommandOptions) -> Result<()> {
    // Step 1: Validate that at least one mode is specified.
    let mode = validate_mode(&options)?;

    // Step 2: Load config (for scanner options).
    let config = load_config(&options)?;

    // Step 3: Dispatch to the appropriate handler.
    let (sections, summary) = match mode {
        CollectMode::Files => collect_files(&options)?,
        CollectMode::Grep => collect_grep(&options, &config)?,
        CollectMode::Symbol => collect_symbol(&options, &config)?,
    };

    if sections.is_empty() {
        if !options.quiet {
            println!("{}", "No matching content found.".dimmed());
        }
        return Ok(());
    }

    // Step 4: Apply budget and build manifest entries.
    let model = options
        .model
        .as_deref()
        .map(tokens::parse_model)
        .unwrap_or(tokens::ModelFamily::Gpt4);
    let estimator = tokens::CharEstimator::new(model);

    let (included_sections, manifest_entries) = apply_budget(&sections, &estimator, options.budget);

    // Step 5: Build bundle.
    let bundle = Bundle {
        summary: format!(
            "{} ({} section{})",
            summary,
            included_sections.len(),
            if included_sections.len() == 1 {
                ""
            } else {
                "s"
            },
        ),
        sections: included_sections,
    };

    // Step 6: Format and write.
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

    // Step 7: Write manifest sibling.
    if let Some(ref out_path) = options.out {
        let m = manifest::build_manifest(
            manifest_entries.clone(),
            estimator.model_name(),
            options.budget,
            0,
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

    // Step 8: Print summary to stderr.
    if !options.quiet && !options.stdout {
        let total_tokens: usize = manifest_entries
            .iter()
            .filter(|e| e.included)
            .map(|e| e.token_estimate)
            .sum();
        let budget_info = match options.budget {
            Some(b) => format!(", ~{total_tokens} tokens (budget: {b})"),
            None => format!(", ~{total_tokens} tokens"),
        };
        eprintln!(
            "{} {} of {} section{} included{}",
            "collect:".green().bold(),
            manifest_entries.iter().filter(|e| e.included).count(),
            manifest_entries.len(),
            if manifest_entries.len() == 1 { "" } else { "s" },
            budget_info,
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Mode validation
// ---------------------------------------------------------------------------

/// Ensure at least one collect mode is specified.
fn validate_mode(options: &CollectCommandOptions) -> Result<CollectMode> {
    if !options.files.is_empty() {
        return Ok(CollectMode::Files);
    }
    if options.grep.is_some() {
        return Ok(CollectMode::Grep);
    }
    if options.symbol.is_some() {
        return Ok(CollectMode::Symbol);
    }
    Err(ContextSmithError::validation(
        "mode",
        "at least one of <query>, --files, --grep, or --symbol must be specified",
    ))
}

// ---------------------------------------------------------------------------
// collect --files
// ---------------------------------------------------------------------------

/// Collect context from explicitly specified files.
///
/// Reads each file in full and creates one section per file.
fn collect_files(options: &CollectCommandOptions) -> Result<(Vec<BundleSection>, String)> {
    let mut sections = Vec::new();

    for file_path in &options.files {
        let abs_path = if file_path.is_absolute() {
            file_path.clone()
        } else {
            options.root.join(file_path)
        };

        let content = std::fs::read_to_string(&abs_path).map_err(|e| {
            ContextSmithError::io(format!("reading file '{}'", abs_path.display()), e)
        })?;

        let rel_path = file_path.to_string_lossy().to_string();
        let language = utils::infer_language(&rel_path);

        sections.push(BundleSection {
            file_path: rel_path,
            language,
            content,
            reason: "explicit file".to_string(),
        });
    }

    let summary = format!(
        "collected {} file{}",
        sections.len(),
        if sections.len() == 1 { "" } else { "s" },
    );

    Ok((sections, summary))
}

// ---------------------------------------------------------------------------
// collect --grep
// ---------------------------------------------------------------------------

/// Collect context by searching the codebase for a pattern.
///
/// Scans the repo for files, searches for the pattern, then extracts
/// context around each match to create sections.
fn collect_grep(
    options: &CollectCommandOptions,
    config: &Config,
) -> Result<(Vec<BundleSection>, String)> {
    let pattern = options.grep.as_deref().unwrap_or("");

    // Scan the repo for files.
    let mut scan_options = scanner::scan_options_from_config(config, &options.root);
    scan_options.lang_filter = options.lang.clone();
    scan_options.path_filter = options.path.clone();
    scan_options.exclude_patterns = options.exclude.clone();

    let files = scanner::scan(&scan_options)?;

    // Search across files.
    let result = indexer::search_files(&files, pattern)?;

    if result.matches.is_empty() {
        return Ok((Vec::new(), "no matches found".to_string()));
    }

    // Group matches by file and build sections with context.
    let grouped = indexer::group_by_file(&result.matches);
    let mut sections = Vec::new();
    let mut match_counts = Vec::new();

    // Sort file paths for deterministic output.
    let mut file_paths: Vec<&String> = grouped.keys().collect();
    file_paths.sort();

    // Apply max_files limit.
    if let Some(max) = options.max_files {
        file_paths.truncate(max);
    }

    for file_path in file_paths {
        let file_matches = &grouped[file_path];

        // Find the file to read its content with context.
        let scanned = files.iter().find(|f| &f.rel_path == file_path);
        let content = match scanned {
            Some(f) => match std::fs::read_to_string(&f.abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            },
            None => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Build context snippets around each match.
        let ranges = compute_match_ranges(file_matches, options.context_lines, total_lines);

        for (start, end) in ranges {
            let snippet_content = lines[start.saturating_sub(1)..end.min(total_lines)].join("\n");

            let match_count = file_matches
                .iter()
                .filter(|m| m.line_number >= start && m.line_number <= end)
                .count();

            sections.push(BundleSection {
                file_path: file_path.clone(),
                language: utils::infer_language(file_path),
                content: snippet_content,
                reason: format!(
                    "grep match{} for '{}'",
                    if match_count == 1 { "" } else { "es" },
                    pattern,
                ),
            });
            match_counts.push(match_count);
        }
    }

    // Rank sections using TF-IDF scoring.
    let weights = config.ranking_weights.clone();
    let ranked = ranker::rank_snippets(&sections, &match_counts, &weights);
    let sections: Vec<BundleSection> = ranked.iter().map(|r| r.section.clone()).collect();

    let summary = format!(
        "grep '{}': {} match{} in {} file{}",
        pattern,
        result.matches.len(),
        if result.matches.len() == 1 { "" } else { "es" },
        result.files_matched,
        if result.files_matched == 1 { "" } else { "s" },
    );

    Ok((sections, summary))
}

// ---------------------------------------------------------------------------
// collect --symbol
// ---------------------------------------------------------------------------

/// Collect context by finding symbol definitions in the codebase.
///
/// Uses the `SymbolFinder` trait (regex-based in Phase 2) to locate
/// definitions, then extracts context around each definition.
fn collect_symbol(
    options: &CollectCommandOptions,
    config: &Config,
) -> Result<(Vec<BundleSection>, String)> {
    let symbol = options.symbol.as_deref().unwrap_or("");

    // Scan the repo for files.
    let mut scan_options = scanner::scan_options_from_config(config, &options.root);
    scan_options.lang_filter = options.lang.clone();
    scan_options.path_filter = options.path.clone();
    scan_options.exclude_patterns = options.exclude.clone();

    let files = scanner::scan(&scan_options)?;

    // Find symbol definitions.
    let finder = RegexSymbolFinder;
    let matches = finder.find_definitions(&files, symbol)?;

    if matches.is_empty() {
        return Ok((Vec::new(), format!("no definitions found for '{symbol}'")));
    }

    // Group matches by file and build sections with context.
    let grouped = indexer::group_by_file(&matches);
    let mut sections = Vec::new();
    let mut match_counts = Vec::new();

    let mut file_paths: Vec<&String> = grouped.keys().collect();
    file_paths.sort();

    if let Some(max) = options.max_files {
        file_paths.truncate(max);
    }

    for file_path in file_paths {
        let file_matches = &grouped[file_path];

        let scanned = files.iter().find(|f| &f.rel_path == file_path);
        let content = match scanned {
            Some(f) => match std::fs::read_to_string(&f.abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            },
            None => continue,
        };

        let file_match_refs: Vec<&indexer::TextMatch> = file_matches.to_vec();
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let ranges = compute_match_ranges(&file_match_refs, options.context_lines, total_lines);

        for (start, end) in ranges {
            let snippet_content = lines[start.saturating_sub(1)..end.min(total_lines)].join("\n");

            let match_count = file_matches
                .iter()
                .filter(|m| m.line_number >= start && m.line_number <= end)
                .count();

            sections.push(BundleSection {
                file_path: file_path.clone(),
                language: utils::infer_language(file_path),
                content: snippet_content,
                reason: format!("definition of '{symbol}'"),
            });
            match_counts.push(match_count);
        }
    }

    // Rank sections using the ranker.
    let weights = config.ranking_weights.clone();
    let ranked = ranker::rank_snippets(&sections, &match_counts, &weights);

    let ranked_sections: Vec<BundleSection> = ranked.iter().map(|r| r.section.clone()).collect();

    let summary = format!(
        "symbol '{}': {} definition{} in {} file{}",
        symbol,
        matches.len(),
        if matches.len() == 1 { "" } else { "s" },
        grouped.len(),
        if grouped.len() == 1 { "" } else { "s" },
    );

    Ok((ranked_sections, summary))
}

// ---------------------------------------------------------------------------
// Range computation
// ---------------------------------------------------------------------------

/// Compute merged line ranges around grep matches with context.
///
/// Each match expands by `context_lines` above and below, then
/// overlapping ranges are merged. Returns 1-based inclusive ranges.
fn compute_match_ranges(
    matches: &[&indexer::TextMatch],
    context_lines: usize,
    total_lines: usize,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = matches
        .iter()
        .map(|m| {
            let start = m.line_number.saturating_sub(context_lines).max(1);
            let end = (m.line_number + context_lines).min(total_lines);
            (start, end)
        })
        .collect();

    ranges.sort_by_key(|&(s, _)| s);
    merge_overlapping_ranges(ranges)
}

/// Merge sorted ranges, combining overlapping or adjacent ones.
fn merge_overlapping_ranges(sorted: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in sorted {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

// ---------------------------------------------------------------------------
// Budget enforcement
// ---------------------------------------------------------------------------

/// Apply budget constraints and build manifest entries.
///
/// Same greedy algorithm as diff: always include at least one section,
/// then greedily include sections until budget is exhausted.
fn apply_budget(
    sections: &[BundleSection],
    estimator: &dyn TokenEstimator,
    budget: Option<usize>,
) -> (Vec<BundleSection>, Vec<ManifestEntry>) {
    let mut included = Vec::new();
    let mut entries = Vec::new();
    let mut tokens_used: usize = 0;

    for (i, section) in sections.iter().enumerate() {
        let token_est = estimator.estimate(&section.content);
        let char_count = section.content.len();

        let is_included = match budget {
            None => true,
            Some(b) => {
                if included.is_empty() {
                    true
                } else {
                    tokens_used + token_est <= b
                }
            }
        };

        if is_included {
            tokens_used += token_est;
            included.push(section.clone());
        }

        entries.push(ManifestEntry {
            file_path: section.file_path.clone(),
            start_line: 0,
            end_line: 0,
            token_estimate: token_est,
            char_count,
            reason: section.reason.clone(),
            score: (sections.len() - i) as f64,
            included: is_included,
            language: section.language.clone(),
        });
    }

    (included, entries)
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load config from explicit path or discovery.
fn load_config(options: &CollectCommandOptions) -> Result<Config> {
    let config_path = crate::config::find_config_file(options.config_path.as_deref());
    match config_path {
        Some(p) => Config::load(&p),
        None => Ok(Config::default()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_mode_requires_at_least_one() {
        let options = CollectCommandOptions {
            root: PathBuf::from("/tmp"),
            files: vec![],
            grep: None,
            symbol: None,
            exclude: vec![],
            lang: None,
            path: None,
            context_lines: 3,
            max_files: None,
            format: OutputFormat::Markdown,
            out: None,
            stdout: true,
            quiet: true,
            budget: None,
            model: None,
            config_path: None,
        };
        assert!(validate_mode(&options).is_err());
    }

    #[test]
    fn validate_mode_files() {
        let options = CollectCommandOptions {
            root: PathBuf::from("/tmp"),
            files: vec![PathBuf::from("main.rs")],
            grep: None,
            symbol: None,
            exclude: vec![],
            lang: None,
            path: None,
            context_lines: 3,
            max_files: None,
            format: OutputFormat::Markdown,
            out: None,
            stdout: true,
            quiet: true,
            budget: None,
            model: None,
            config_path: None,
        };
        assert!(matches!(
            validate_mode(&options).unwrap(),
            CollectMode::Files
        ));
    }

    #[test]
    fn validate_mode_grep() {
        let options = CollectCommandOptions {
            root: PathBuf::from("/tmp"),
            files: vec![],
            grep: Some("pattern".to_string()),
            symbol: None,
            exclude: vec![],
            lang: None,
            path: None,
            context_lines: 3,
            max_files: None,
            format: OutputFormat::Markdown,
            out: None,
            stdout: true,
            quiet: true,
            budget: None,
            model: None,
            config_path: None,
        };
        assert!(matches!(
            validate_mode(&options).unwrap(),
            CollectMode::Grep
        ));
    }

    #[test]
    fn merge_ranges_basic() {
        let ranges = vec![(1, 5), (4, 8), (15, 20)];
        let merged = merge_overlapping_ranges(ranges);
        assert_eq!(merged, vec![(1, 8), (15, 20)]);
    }

    #[test]
    fn merge_ranges_adjacent() {
        let ranges = vec![(1, 5), (6, 10)];
        let merged = merge_overlapping_ranges(ranges);
        assert_eq!(merged, vec![(1, 10)]);
    }

    #[test]
    fn apply_budget_no_budget_includes_all() {
        let sections = vec![
            BundleSection {
                file_path: "a.rs".to_string(),
                language: "rust".to_string(),
                content: "fn a() {}".to_string(),
                reason: "test".to_string(),
            },
            BundleSection {
                file_path: "b.rs".to_string(),
                language: "rust".to_string(),
                content: "fn b() {}".to_string(),
                reason: "test".to_string(),
            },
        ];
        let estimator = tokens::default_estimator();
        let (included, entries) = apply_budget(&sections, &estimator, None);
        assert_eq!(included.len(), 2);
        assert!(entries.iter().all(|e| e.included));
    }

    #[test]
    fn apply_budget_tight_budget_limits() {
        let sections = vec![
            BundleSection {
                file_path: "a.rs".to_string(),
                language: "rust".to_string(),
                content: "fn alpha() { do_something(); }".to_string(), // 30 chars = 8 tokens
                reason: "test".to_string(),
            },
            BundleSection {
                file_path: "b.rs".to_string(),
                language: "rust".to_string(),
                content: "fn beta() { do_another_thing(); }".to_string(), // 33 chars = 9 tokens
                reason: "test".to_string(),
            },
        ];
        let estimator = tokens::default_estimator();
        // Budget 8: first section fits (8 tokens), second exceeds (8+9=17 > 8).
        let (included, _) = apply_budget(&sections, &estimator, Some(8));
        assert_eq!(included.len(), 1);
        assert_eq!(included[0].file_path, "a.rs");
    }
}
