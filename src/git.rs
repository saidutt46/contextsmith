//! Git integration for ContextSmith.
//!
//! Provides safe wrappers around `git` CLI commands and a parser for
//! unified diff output. This module is the sole interface to git —
//! all other modules work with the parsed [`DiffFile`] and [`DiffHunk`]
//! types rather than raw git output.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{ContextSmithError, Result};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options controlling which diff to produce.
#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// Repository root directory.
    pub root: PathBuf,
    /// Optional revision range (e.g. "HEAD~3..HEAD").
    pub rev_range: Option<String>,
    /// If true, diff staged (index) changes only.
    pub staged: bool,
    /// If true, include untracked files in the diff.
    pub untracked: bool,
    /// Optional base reference or duration (e.g. "2h", "2024-01-01").
    pub since: Option<String>,
}

/// A single file affected by the diff.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffFile {
    /// Path to the file (relative to repo root).
    pub path: String,
    /// Previous path if the file was renamed.
    pub old_path: Option<String>,
    /// How the file was changed.
    pub status: FileStatus,
    /// Individual change regions within the file.
    pub hunks: Vec<DiffHunk>,
}

/// The kind of change applied to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// A contiguous region of changes within a file.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    /// Starting line number in the old file.
    pub old_start: usize,
    /// Number of lines from the old file.
    pub old_count: usize,
    /// Starting line number in the new file.
    pub new_start: usize,
    /// Number of lines in the new file.
    pub new_count: usize,
    /// The `@@` header line (e.g. `@@ -10,7 +10,8 @@ fn main()`).
    pub header: String,
    /// Individual lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// A single line within a [`DiffHunk`].
#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    /// Whether this line was added, removed, or is context.
    pub kind: LineKind,
    /// The text content of the line (without the leading +/-/space).
    pub content: String,
    /// Line number in the old file, if applicable.
    pub old_lineno: Option<usize>,
    /// Line number in the new file, if applicable.
    pub new_lineno: Option<usize>,
}

/// Classification of a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

// ---------------------------------------------------------------------------
// Git command execution
// ---------------------------------------------------------------------------

/// Run a git command in the given directory and return its stdout.
///
/// Returns a [`ContextSmithError::Git`] if the command fails or if git
/// is not installed.
fn run_git(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| ContextSmithError::Git {
            message: format!("failed to execute git: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ContextSmithError::Git {
            message: if stderr.is_empty() {
                format!("git exited with status {}", output.status)
            } else {
                stderr
            },
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Verify that the given path is inside a git repository.
pub fn verify_git_repo(root: &Path) -> Result<()> {
    run_git(&["rev-parse", "--git-dir"], root).map(|_| ())
}

// ---------------------------------------------------------------------------
// Diff retrieval
// ---------------------------------------------------------------------------

/// Obtain a parsed diff from the repository according to the given options.
///
/// This builds the appropriate `git diff` invocation, runs it, and parses
/// the unified diff output into structured [`DiffFile`] values.
pub fn get_diff(options: &DiffOptions) -> Result<Vec<DiffFile>> {
    verify_git_repo(&options.root)?;

    let mut args = vec!["diff", "--no-color", "-u"];

    if options.staged {
        args.push("--cached");
    }

    // Build the revision range or --merge-base for `--since`.
    let since_rev;
    if let Some(ref range) = options.rev_range {
        args.push(range);
    } else if let Some(ref since) = options.since {
        // `git diff $(git rev-list -1 --before=<since> HEAD)..HEAD`
        // We resolve the base commit first, then pass it as a range.
        since_rev = resolve_since_rev(&options.root, since)?;
        args.push(&since_rev);
    }

    let raw = run_git(&args, &options.root)?;
    let mut files = parse_unified_diff(&raw);

    // If --untracked is set, append untracked files as "added" diffs.
    if options.untracked {
        let untracked = get_untracked_files(&options.root)?;
        for path in untracked {
            let content = std::fs::read_to_string(options.root.join(&path)).unwrap_or_default();
            let lines: Vec<DiffLine> = content
                .lines()
                .enumerate()
                .map(|(i, line)| DiffLine {
                    kind: LineKind::Added,
                    content: line.to_string(),
                    old_lineno: None,
                    new_lineno: Some(i + 1),
                })
                .collect();

            let line_count = lines.len();
            if line_count == 0 {
                continue;
            }

            files.push(DiffFile {
                path: path.clone(),
                old_path: None,
                status: FileStatus::Added,
                hunks: vec![DiffHunk {
                    old_start: 0,
                    old_count: 0,
                    new_start: 1,
                    new_count: line_count,
                    header: format!("@@ -0,0 +1,{line_count} @@"),
                    lines,
                }],
            });
        }
    }

    Ok(files)
}

/// Resolve a `--since` value to a revision range string (e.g. "abc123..HEAD").
fn resolve_since_rev(root: &Path, since: &str) -> Result<String> {
    let output = run_git(
        &["rev-list", "-1", &format!("--before={since}"), "HEAD"],
        root,
    )?;
    let base = output.trim();
    if base.is_empty() {
        return Err(ContextSmithError::Git {
            message: format!("no commits found before '{since}'"),
        });
    }
    Ok(format!("{base}..HEAD"))
}

/// List untracked files in the repository.
fn get_untracked_files(root: &Path) -> Result<Vec<String>> {
    let output = run_git(&["ls-files", "--others", "--exclude-standard"], root)?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

// ---------------------------------------------------------------------------
// Unified diff parser
// ---------------------------------------------------------------------------

/// Parse the full output of `git diff -u` into structured [`DiffFile`] values.
///
/// Handles standard unified diff headers (`diff --git`, `---`, `+++`, `@@`),
/// rename detection, and file status inference.
pub fn parse_unified_diff(input: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current_file: Option<DiffFile> = None;
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_lineno: usize = 0;
    let mut new_lineno: usize = 0;

    for line in input.lines() {
        // --- New file header ---
        if line.starts_with("diff --git ") {
            // Flush any in-progress hunk and file.
            flush_hunk(&mut current_file, &mut current_hunk);
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            let (a_path, b_path) = parse_diff_header(line);
            let status = if a_path != b_path {
                FileStatus::Renamed
            } else {
                FileStatus::Modified // refined later by --- / +++ lines
            };

            current_file = Some(DiffFile {
                path: b_path.clone(),
                old_path: if a_path != b_path { Some(a_path) } else { None },
                status,
                hunks: Vec::new(),
            });
            continue;
        }

        // --- Detect new / deleted files ---
        if line.starts_with("--- /dev/null") {
            if let Some(ref mut f) = current_file {
                f.status = FileStatus::Added;
            }
            continue;
        }
        if line.starts_with("+++ /dev/null") {
            if let Some(ref mut f) = current_file {
                f.status = FileStatus::Deleted;
            }
            continue;
        }

        // Skip the --- and +++ path lines (already captured from header).
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }

        // --- Hunk header ---
        if line.starts_with("@@ ") {
            flush_hunk(&mut current_file, &mut current_hunk);
            if let Some(hunk) = parse_hunk_header(line) {
                old_lineno = hunk.old_start;
                new_lineno = hunk.new_start;
                current_hunk = Some(hunk);
            }
            continue;
        }

        // --- Diff content lines ---
        if let Some(ref mut hunk) = current_hunk {
            if let Some(stripped) = line.strip_prefix('+') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Added,
                    content: stripped.to_string(),
                    old_lineno: None,
                    new_lineno: Some(new_lineno),
                });
                new_lineno += 1;
            } else if let Some(stripped) = line.strip_prefix('-') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Removed,
                    content: stripped.to_string(),
                    old_lineno: Some(old_lineno),
                    new_lineno: None,
                });
                old_lineno += 1;
            } else if let Some(stripped) = line.strip_prefix(' ') {
                hunk.lines.push(DiffLine {
                    kind: LineKind::Context,
                    content: stripped.to_string(),
                    old_lineno: Some(old_lineno),
                    new_lineno: Some(new_lineno),
                });
                old_lineno += 1;
                new_lineno += 1;
            } else if line == "\\ No newline at end of file" {
                // Git marker — skip silently.
            } else {
                // Treat bare context lines (no leading space) as context.
                hunk.lines.push(DiffLine {
                    kind: LineKind::Context,
                    content: line.to_string(),
                    old_lineno: Some(old_lineno),
                    new_lineno: Some(new_lineno),
                });
                old_lineno += 1;
                new_lineno += 1;
            }
        }
    }

    // Flush trailing hunk and file.
    flush_hunk(&mut current_file, &mut current_hunk);
    if let Some(file) = current_file.take() {
        files.push(file);
    }

    files
}

/// Push the current hunk (if any) into the current file.
fn flush_hunk(file: &mut Option<DiffFile>, hunk: &mut Option<DiffHunk>) {
    if let (Some(ref mut f), Some(h)) = (file, hunk.take()) {
        f.hunks.push(h);
    }
}

/// Extract (a_path, b_path) from a `diff --git a/path b/path` line.
fn parse_diff_header(line: &str) -> (String, String) {
    // Format: "diff --git a/<path> b/<path>"
    let rest = line.strip_prefix("diff --git ").unwrap_or(line);
    let parts: Vec<&str> = rest.splitn(2, " b/").collect();
    let a_path = parts
        .first()
        .unwrap_or(&"")
        .strip_prefix("a/")
        .unwrap_or(parts.first().unwrap_or(&""))
        .to_string();
    let b_path = parts.get(1).unwrap_or(&"").to_string();
    (a_path, b_path)
}

/// Parse a hunk header line like `@@ -10,7 +10,8 @@ fn main()`.
fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // Extract the range portion between @@ markers.
    let trimmed = line.strip_prefix("@@ ")?;
    let end = trimmed.find(" @@")?;
    let range_str = &trimmed[..end];

    let parts: Vec<&str> = range_str.split(' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let (old_start, old_count) = parse_range(parts[0].strip_prefix('-')?)?;
    let (new_start, new_count) = parse_range(parts[1].strip_prefix('+')?)?;

    Some(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
        header: line.to_string(),
        lines: Vec::new(),
    })
}

/// Parse a range like "10,7" or "10" into (start, count).
fn parse_range(s: &str) -> Option<(usize, usize)> {
    if let Some((start, count)) = s.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical unified diff output for a single modified file.
    const SAMPLE_DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 fn main() {
-    println!(\"Hello\");
+    println!(\"Hello, world!\");
+    println!(\"Welcome\");
     let x = 1;
     let y = 2;
 }";

    /// Diff output for a newly added file.
    const NEW_FILE_DIFF: &str = "\
diff --git a/new.txt b/new.txt
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,3 @@
+line one
+line two
+line three";

    /// Diff output for a deleted file.
    const DELETED_FILE_DIFF: &str = "\
diff --git a/old.txt b/old.txt
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-goodbye
-world";

    /// Diff output with a renamed file.
    const RENAMED_FILE_DIFF: &str = "\
diff --git a/old_name.rs b/new_name.rs
--- a/old_name.rs
+++ b/new_name.rs
@@ -1,3 +1,3 @@
 fn example() {
-    old_code();
+    new_code();
 }";

    #[test]
    fn parse_single_modified_file() {
        let files = parse_unified_diff(SAMPLE_DIFF);
        assert_eq!(files.len(), 1);

        let file = &files[0];
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.status, FileStatus::Modified);
        assert!(file.old_path.is_none());
        assert_eq!(file.hunks.len(), 1);

        let hunk = &file.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 5);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 6);
        assert_eq!(hunk.lines.len(), 7);
    }

    #[test]
    fn parse_added_file() {
        let files = parse_unified_diff(NEW_FILE_DIFF);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, FileStatus::Added);
        assert_eq!(files[0].hunks[0].lines.len(), 3);
        assert!(files[0].hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == LineKind::Added));
    }

    #[test]
    fn parse_deleted_file() {
        let files = parse_unified_diff(DELETED_FILE_DIFF);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, FileStatus::Deleted);
        assert!(files[0].hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == LineKind::Removed));
    }

    #[test]
    fn parse_renamed_file() {
        let files = parse_unified_diff(RENAMED_FILE_DIFF);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "new_name.rs");
        assert_eq!(files[0].old_path.as_deref(), Some("old_name.rs"));
        assert_eq!(files[0].status, FileStatus::Renamed);
    }

    #[test]
    fn parse_multiple_files() {
        let combined = format!("{SAMPLE_DIFF}\n{NEW_FILE_DIFF}");
        let files = parse_unified_diff(&combined);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[1].path, "new.txt");
    }

    #[test]
    fn parse_empty_diff() {
        let files = parse_unified_diff("");
        assert!(files.is_empty());
    }

    #[test]
    fn line_numbers_are_tracked() {
        let files = parse_unified_diff(SAMPLE_DIFF);
        let lines = &files[0].hunks[0].lines;

        // First line is context: "fn main() {"
        assert_eq!(lines[0].kind, LineKind::Context);
        assert_eq!(lines[0].old_lineno, Some(1));
        assert_eq!(lines[0].new_lineno, Some(1));

        // Second line is removed: `    println!("Hello");`
        assert_eq!(lines[1].kind, LineKind::Removed);
        assert_eq!(lines[1].old_lineno, Some(2));
        assert_eq!(lines[1].new_lineno, None);

        // Third line is added: `    println!("Hello, world!");`
        assert_eq!(lines[2].kind, LineKind::Added);
        assert_eq!(lines[2].old_lineno, None);
        assert_eq!(lines[2].new_lineno, Some(2));
    }

    #[test]
    fn hunk_header_parsing() {
        let hunk = parse_hunk_header("@@ -10,7 +10,8 @@ fn main()").unwrap();
        assert_eq!(hunk.old_start, 10);
        assert_eq!(hunk.old_count, 7);
        assert_eq!(hunk.new_start, 10);
        assert_eq!(hunk.new_count, 8);
    }

    #[test]
    fn hunk_header_single_line() {
        let hunk = parse_hunk_header("@@ -1 +1 @@").unwrap();
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 1);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 1);
    }

    #[test]
    fn diff_header_parsing() {
        let (a, b) = parse_diff_header("diff --git a/src/lib.rs b/src/lib.rs");
        assert_eq!(a, "src/lib.rs");
        assert_eq!(b, "src/lib.rs");
    }
}
