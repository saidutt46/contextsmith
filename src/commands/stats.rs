//! Handler for the `contextsmith stats` command.
//!
//! Two modes:
//! - **Bundle mode** (positional arg): read a manifest, show token/snippet stats.
//! - **Repo scan mode** (no arg, requires --root): walk repo with scanner,
//!   count files, estimate tokens.

use std::collections::HashMap;
use std::path::PathBuf;

use colored::Colorize;

use crate::config::Config;
use crate::error::Result;
use crate::manifest;
use crate::scanner;
use crate::tokens::{self, TokenEstimator};

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/// All inputs needed to run the stats command.
#[derive(Debug)]
pub struct StatsCommandOptions {
    /// Input bundle/manifest file (if provided, runs bundle mode).
    pub bundle: Option<PathBuf>,
    /// Repository root (for repo scan mode).
    pub root: PathBuf,
    /// Show top N files by token count.
    pub top_files: Option<usize>,
    /// Group stats by language.
    pub by_lang: bool,
    /// Group stats by file type.
    pub by_type: bool,
    /// Show token counts.
    pub tokens: bool,
    /// Suppress non-essential output.
    pub quiet: bool,
    /// Path to config file.
    pub config_path: Option<PathBuf>,
}

/// Run the stats command.
pub fn run(options: StatsCommandOptions) -> Result<()> {
    if let Some(ref bundle_path) = options.bundle {
        run_bundle_mode(bundle_path, &options)
    } else {
        run_repo_mode(&options)
    }
}

// ---------------------------------------------------------------------------
// Bundle mode
// ---------------------------------------------------------------------------

/// Show stats from an existing manifest file.
fn run_bundle_mode(path: &std::path::Path, options: &StatsCommandOptions) -> Result<()> {
    let manifest = manifest::read_manifest(path)?;
    let summary = &manifest.summary;

    println!("{}", "Bundle Statistics".bold());
    println!("  model:           {}", summary.model);
    println!("  total tokens:    {}", summary.total_tokens);
    println!(
        "  budget:          {}",
        summary.budget.map_or("none".to_string(), |b| b.to_string())
    );
    println!("  reserve tokens:  {}", summary.reserve_tokens);
    println!("  snippets:        {}", summary.snippet_count);
    println!("  included:        {}", summary.included_count);

    if manifest.entries.is_empty() {
        return Ok(());
    }

    // Top files by token count.
    let top_n = options.top_files.unwrap_or(10);
    let mut entries = manifest.entries.clone();
    entries.sort_by(|a, b| b.token_estimate.cmp(&a.token_estimate));
    entries.truncate(top_n);

    println!();
    println!("{}", format!("Top {} files by tokens:", top_n).bold());
    for entry in &entries {
        let status = if entry.included { "+" } else { "-" };
        let location = if entry.start_line > 0 {
            format!(
                "{}:{}-{}",
                entry.file_path, entry.start_line, entry.end_line
            )
        } else {
            entry.file_path.clone()
        };
        println!(
            "  {} {:>6} tokens  {}",
            status.dimmed(),
            entry.token_estimate,
            location,
        );
    }

    // By-language breakdown.
    if options.by_lang {
        println!();
        println!("{}", "By language:".bold());
        let mut lang_stats: HashMap<String, (usize, usize)> = HashMap::new();
        for entry in &manifest.entries {
            let lang = if entry.language.is_empty() {
                "unknown".to_string()
            } else {
                entry.language.clone()
            };
            let (count, tokens) = lang_stats.entry(lang).or_insert((0, 0));
            *count += 1;
            *tokens += entry.token_estimate;
        }
        let mut langs: Vec<_> = lang_stats.into_iter().collect();
        langs.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
        for (lang, (count, tokens)) in &langs {
            println!("  {:<15} {:>4} snippets  {:>6} tokens", lang, count, tokens);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Repo scan mode
// ---------------------------------------------------------------------------

/// Walk the repo and show file/token statistics.
fn run_repo_mode(options: &StatsCommandOptions) -> Result<()> {
    let config = load_config(options)?;
    let scan_options = scanner::scan_options_from_config(&config, &options.root);
    let files = scanner::scan(&scan_options)?;

    if files.is_empty() {
        println!("{}", "No files found.".dimmed());
        return Ok(());
    }

    let estimator = tokens::default_estimator();
    let mut total_tokens: usize = 0;
    let mut total_bytes: u64 = 0;
    let mut lang_stats: HashMap<String, (usize, u64, usize)> = HashMap::new(); // (count, bytes, tokens)
    let mut file_tokens: Vec<(String, usize, u64)> = Vec::new();

    for file in &files {
        let file_size = file.size;
        total_bytes += file_size;

        let tokens = if options.tokens {
            match std::fs::read_to_string(&file.abs_path) {
                Ok(content) => estimator.estimate(&content),
                Err(_) => 0,
            }
        } else {
            0
        };
        total_tokens += tokens;

        let lang = if file.language.is_empty() {
            "unknown".to_string()
        } else {
            file.language.clone()
        };

        let entry = lang_stats.entry(lang).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += file_size;
        entry.2 += tokens;

        file_tokens.push((file.rel_path.clone(), tokens, file_size));
    }

    println!("{}", "Repository Statistics".bold());
    println!("  files:           {}", files.len());
    println!("  total bytes:     {}", format_bytes(total_bytes));
    if options.tokens {
        println!("  total tokens:    ~{}", total_tokens);
    }
    let generated_count = files.iter().filter(|f| f.is_generated).count();
    if generated_count > 0 {
        println!("  generated files: {}", generated_count);
    }

    // Top files.
    if options.tokens {
        let top_n = options.top_files.unwrap_or(10);
        file_tokens.sort_by(|a, b| b.1.cmp(&a.1));
        file_tokens.truncate(top_n);

        println!();
        println!("{}", format!("Top {} files by tokens:", top_n).bold());
        for (path, tokens, _) in &file_tokens {
            println!("  {:>6} tokens  {}", tokens, path);
        }
    }

    // By-language breakdown.
    if options.by_lang {
        println!();
        println!("{}", "By language:".bold());
        let mut langs: Vec<_> = lang_stats.into_iter().collect();
        langs.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        for (lang, (count, bytes, tokens)) in &langs {
            if options.tokens {
                println!(
                    "  {:<15} {:>4} files  {:>8}  ~{:>6} tokens",
                    lang,
                    count,
                    format_bytes(*bytes),
                    tokens,
                );
            } else {
                println!(
                    "  {:<15} {:>4} files  {:>8}",
                    lang,
                    count,
                    format_bytes(*bytes),
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format bytes as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Load config from explicit path or discovery.
fn load_config(options: &StatsCommandOptions) -> Result<Config> {
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
    fn format_bytes_displays_correctly() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
    }

    #[test]
    fn bundle_mode_with_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest::build_manifest(
            vec![crate::manifest::ManifestEntry {
                file_path: "test.rs".to_string(),
                start_line: 1,
                end_line: 10,
                token_estimate: 50,
                char_count: 200,
                reason: "test".to_string(),
                score: 1.0,
                included: true,
                language: "rust".to_string(),
            }],
            "gpt-4",
            Some(1000),
            0,
        );
        let path = dir.path().join("test.manifest.json");
        manifest::write_manifest(&manifest, &path).unwrap();

        // Should succeed without panicking.
        let options = StatsCommandOptions {
            bundle: Some(path),
            root: dir.path().to_path_buf(),
            top_files: Some(5),
            by_lang: true,
            by_type: false,
            tokens: true,
            quiet: false,
            config_path: None,
        };
        run(options).unwrap();
    }
}
