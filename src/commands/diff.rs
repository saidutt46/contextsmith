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
use crate::output::{self, Bundle, BundleSection, Format, FormatOptions};
use crate::slicer::{self, SliceOptions};

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

    // Step 3: Build a bundle from the snippets.
    let bundle = build_bundle(&diff_files, snippets);

    // Step 4: Format and write output.
    let format = cli_format_to_output_format(&options.format);
    let formatted = output::format_bundle(&bundle, format)?;
    output::write_output(
        &formatted,
        &FormatOptions {
            format,
            stdout: options.stdout,
            out: options.out,
        },
    )?;

    // Step 5: Print summary to stderr (unless writing to stdout or quiet).
    if !options.quiet && !options.stdout {
        print_summary(&diff_files);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an output [`Bundle`] from diff files and extracted snippets.
fn build_bundle(diff_files: &[git::DiffFile], snippets: Vec<slicer::Snippet>) -> Bundle {
    let file_count = diff_files.len();
    let hunk_count: usize = diff_files.iter().map(|f| f.hunks.len()).sum();

    let sections: Vec<BundleSection> = snippets
        .into_iter()
        .map(|s| BundleSection {
            language: infer_language(&s.file_path),
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

/// Infer a syntax-highlighting language identifier from a file extension.
fn infer_language(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "rb" => "ruby",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "sh" | "bash" | "zsh" => "bash",
        "md" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        _ => "",
    }
    .to_string()
}

/// Map the clap [`OutputFormat`] to the library [`Format`].
fn cli_format_to_output_format(fmt: &OutputFormat) -> Format {
    match fmt {
        OutputFormat::Markdown => Format::Markdown,
        OutputFormat::Json => Format::Json,
        OutputFormat::Plain => Format::Plain,
        OutputFormat::Xml => Format::Xml,
    }
}

/// Print a coloured summary of the diff to stderr.
fn print_summary(diff_files: &[git::DiffFile]) {
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

    eprintln!(
        "{} {} file{} ({}), {} hunk{}",
        "diff:".green().bold(),
        diff_files.len(),
        if diff_files.len() == 1 { "" } else { "s" },
        parts.join(", "),
        total_hunks,
        if total_hunks == 1 { "" } else { "s" },
    );
}
