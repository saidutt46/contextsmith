//! Snippet extraction and context slicing.
//!
//! Given parsed diff data, the slicer reads source files and extracts
//! minimal spans around each changed region. It handles:
//!
//! - **Context expansion**: adding configurable lines above/below hunks
//! - **Overlap merging**: combining adjacent snippets from the same file
//! - **Hunks-only mode**: emitting raw hunk content without reading files
//!
//! The output is a vector of [`Snippet`] values that downstream code
//! (the diff command, output formatter) can consume directly.

use std::path::{Path, PathBuf};

use crate::error::{ContextSmithError, Result};
use crate::git::{DiffFile, DiffHunk, FileStatus, LineKind};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Controls how snippets are extracted from diff hunks.
#[derive(Debug, Clone)]
pub struct SliceOptions {
    /// Number of context lines to include above and below each hunk.
    pub context_lines: usize,
    /// If true, emit only the raw hunk lines without reading the full file.
    pub hunks_only: bool,
    /// Repository root — source files are resolved relative to this.
    pub root: PathBuf,
}

/// A single extracted code snippet with metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Snippet {
    /// File path relative to the repository root.
    pub file_path: String,
    /// First line number in the snippet (1-based).
    pub start_line: usize,
    /// Last line number in the snippet (1-based, inclusive).
    pub end_line: usize,
    /// The extracted source text.
    pub content: String,
    /// Human-readable reason for inclusion.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Core slicing logic
// ---------------------------------------------------------------------------

/// Extract snippets from parsed diff files.
///
/// For each hunk in each file, either reads the source file and extracts
/// the hunk region plus context lines, or (in hunks-only mode) returns
/// the raw hunk content directly. Overlapping regions within the same
/// file are merged into a single snippet.
pub fn slice_diff_hunks(diff_files: &[DiffFile], options: &SliceOptions) -> Result<Vec<Snippet>> {
    let mut snippets = Vec::new();

    for file in diff_files {
        let file_snippets = if options.hunks_only {
            slice_hunks_only(file)
        } else {
            slice_with_context(file, options)?
        };
        snippets.extend(file_snippets);
    }

    Ok(snippets)
}

/// Extract snippets using only the raw hunk content (no file reading).
///
/// Each hunk becomes one snippet containing the added/removed/context
/// lines exactly as they appear in the diff.
fn slice_hunks_only(file: &DiffFile) -> Vec<Snippet> {
    file.hunks
        .iter()
        .enumerate()
        .map(|(i, hunk)| {
            let content = hunk
                .lines
                .iter()
                .map(|l| {
                    let prefix = match l.kind {
                        LineKind::Added => "+",
                        LineKind::Removed => "-",
                        LineKind::Context => " ",
                    };
                    format!("{prefix}{}", l.content)
                })
                .collect::<Vec<_>>()
                .join("\n");

            Snippet {
                file_path: file.path.clone(),
                start_line: hunk.new_start,
                end_line: hunk.new_start.saturating_add(hunk.new_count).max(1),
                content,
                reason: format!(
                    "{} (hunk {}/{})",
                    status_reason(file.status),
                    i + 1,
                    file.hunks.len()
                ),
            }
        })
        .collect()
}

/// Extract snippets by reading the source file and including context lines.
///
/// Computes the line ranges touched by each hunk, expands them by
/// `context_lines`, merges overlapping ranges, then reads those ranges
/// from the file on disk.
fn slice_with_context(file: &DiffFile, options: &SliceOptions) -> Result<Vec<Snippet>> {
    // For deleted files, there's no file on disk to read — fall back to hunks-only.
    if file.status == FileStatus::Deleted {
        return Ok(slice_hunks_only(file));
    }

    let source_path = options.root.join(&file.path);
    let file_lines = read_file_lines(&source_path)?;
    let total_lines = file_lines.len();

    if total_lines == 0 {
        return Ok(Vec::new());
    }

    // Compute expanded ranges from all hunks, then merge overlaps.
    let ranges = compute_merged_ranges(&file.hunks, options.context_lines, total_lines);

    let snippets = ranges
        .into_iter()
        .filter_map(|(start, end)| {
            // Clamp to valid file bounds (1-based → 0-based indexing).
            let clamped_start = start.max(1);
            let clamped_end = end.min(total_lines);
            if clamped_start > clamped_end {
                return None;
            }
            let content = file_lines[clamped_start.saturating_sub(1)..clamped_end].join("\n");

            Some(Snippet {
                file_path: file.path.clone(),
                start_line: clamped_start,
                end_line: clamped_end,
                content,
                reason: status_reason(file.status),
            })
        })
        .collect();

    Ok(snippets)
}

/// Compute line ranges for all hunks, expand by context, and merge overlaps.
///
/// Uses the actual changed (added/removed) line numbers within each hunk
/// rather than the hunk header's range, which includes git's default 3
/// context lines. This ensures `--context N` accurately controls the
/// number of surrounding lines shown.
///
/// Returns a sorted, non-overlapping list of `(start_line, end_line)` tuples
/// (1-based, inclusive).
fn compute_merged_ranges(
    hunks: &[DiffHunk],
    context_lines: usize,
    total_lines: usize,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = hunks
        .iter()
        .map(|h| {
            // Find the actual changed line range by inspecting DiffLines.
            // Only added lines have new_lineno (they exist in the new file).
            let changed_lines: Vec<usize> = h
                .lines
                .iter()
                .filter(|l| l.kind == LineKind::Added || l.kind == LineKind::Removed)
                .filter_map(|l| l.new_lineno.or(l.old_lineno))
                .collect();

            // Fall back to hunk header if no changed lines found.
            let (change_start, change_end) = if changed_lines.is_empty() {
                (h.new_start, h.new_start + h.new_count.saturating_sub(1))
            } else {
                let min = *changed_lines.iter().min().unwrap();
                let max = *changed_lines.iter().max().unwrap();
                (min, max)
            };

            let start = change_start.saturating_sub(context_lines).max(1);
            let end = (change_end + context_lines).min(total_lines);
            (start, end)
        })
        .collect();

    ranges.sort_by_key(|&(s, _)| s);
    merge_overlapping_ranges(ranges)
}

/// Merge a sorted list of ranges, combining any that overlap or are adjacent.
fn merge_overlapping_ranges(sorted: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();

    for (start, end) in sorted {
        if let Some(last) = merged.last_mut() {
            // +1 to merge adjacent ranges (e.g. [1,5] and [6,10] → [1,10]).
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    merged
}

/// Read all lines from a file, returning them as a vector of strings.
fn read_file_lines(path: &Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ContextSmithError::io(format!("reading file '{}'", path.display()), e))?;
    Ok(content.lines().map(String::from).collect())
}

/// Map a [`FileStatus`] to a human-readable reason string.
fn status_reason(status: FileStatus) -> String {
    match status {
        FileStatus::Added => "added".to_string(),
        FileStatus::Modified => "modified in diff".to_string(),
        FileStatus::Deleted => "deleted".to_string(),
        FileStatus::Renamed => "renamed".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{DiffHunk, DiffLine, LineKind};
    use std::io::Write;

    /// Helper: create a temp directory with a source file and return its path.
    fn setup_source_file(name: &str, content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        (dir, root)
    }

    /// Helper: build a minimal DiffFile with one hunk.
    fn make_diff_file(path: &str, new_start: usize, new_count: usize) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            old_path: None,
            status: FileStatus::Modified,
            hunks: vec![DiffHunk {
                old_start: new_start,
                old_count: new_count,
                new_start,
                new_count,
                header: format!("@@ -{new_start},{new_count} +{new_start},{new_count} @@"),
                lines: vec![DiffLine {
                    kind: LineKind::Added,
                    content: "changed line".to_string(),
                    old_lineno: None,
                    new_lineno: Some(new_start),
                }],
            }],
        }
    }

    #[test]
    fn single_hunk_with_context() {
        let source = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
        let (dir, root) = setup_source_file("test.rs", source);

        let diff = make_diff_file("test.rs", 5, 1);
        let options = SliceOptions {
            context_lines: 2,
            hunks_only: false,
            root,
        };

        let snippets = slice_diff_hunks(&[diff], &options).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].start_line, 3);
        assert_eq!(snippets[0].end_line, 7);
        assert!(snippets[0].content.contains("line3"));
        assert!(snippets[0].content.contains("line7"));

        drop(dir);
    }

    #[test]
    fn overlapping_hunks_are_merged() {
        let source = (1..=20)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (dir, root) = setup_source_file("merge.rs", &source);

        // Two hunks close enough to overlap with 3 lines of context.
        let diff = DiffFile {
            path: "merge.rs".to_string(),
            old_path: None,
            status: FileStatus::Modified,
            hunks: vec![
                DiffHunk {
                    old_start: 5,
                    old_count: 1,
                    new_start: 5,
                    new_count: 1,
                    header: "@@ -5,1 +5,1 @@".to_string(),
                    lines: vec![DiffLine {
                        kind: LineKind::Added,
                        content: "a".to_string(),
                        old_lineno: None,
                        new_lineno: Some(5),
                    }],
                },
                DiffHunk {
                    old_start: 9,
                    old_count: 1,
                    new_start: 9,
                    new_count: 1,
                    header: "@@ -9,1 +9,1 @@".to_string(),
                    lines: vec![DiffLine {
                        kind: LineKind::Added,
                        content: "b".to_string(),
                        old_lineno: None,
                        new_lineno: Some(9),
                    }],
                },
            ],
        };

        let options = SliceOptions {
            context_lines: 3,
            hunks_only: false,
            root,
        };

        let snippets = slice_diff_hunks(&[diff], &options).unwrap();
        // Ranges [2,8] and [6,12] should merge into [2,12].
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].start_line, 2);
        assert_eq!(snippets[0].end_line, 12);

        drop(dir);
    }

    #[test]
    fn hunks_only_mode() {
        let diff = make_diff_file("test.rs", 5, 1);
        let options = SliceOptions {
            context_lines: 3,
            hunks_only: true,
            root: PathBuf::from("/unused"),
        };

        let snippets = slice_diff_hunks(&[diff], &options).unwrap();
        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].content.starts_with('+'));
    }

    #[test]
    fn deleted_file_falls_back_to_hunks_only() {
        let diff = DiffFile {
            path: "gone.rs".to_string(),
            old_path: None,
            status: FileStatus::Deleted,
            hunks: vec![DiffHunk {
                old_start: 1,
                old_count: 2,
                new_start: 0,
                new_count: 0,
                header: "@@ -1,2 +0,0 @@".to_string(),
                lines: vec![
                    DiffLine {
                        kind: LineKind::Removed,
                        content: "old line 1".to_string(),
                        old_lineno: Some(1),
                        new_lineno: None,
                    },
                    DiffLine {
                        kind: LineKind::Removed,
                        content: "old line 2".to_string(),
                        old_lineno: Some(2),
                        new_lineno: None,
                    },
                ],
            }],
        };

        let options = SliceOptions {
            context_lines: 3,
            hunks_only: false,
            root: PathBuf::from("/unused"),
        };

        let snippets = slice_diff_hunks(&[diff], &options).unwrap();
        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].reason.starts_with("deleted"));
        assert!(snippets[0].content.contains("-old line 1"));
    }

    #[test]
    fn file_not_found_returns_error() {
        let diff = make_diff_file("nonexistent.rs", 1, 1);
        let options = SliceOptions {
            context_lines: 3,
            hunks_only: false,
            root: PathBuf::from("/tmp/empty_dir_that_should_not_exist"),
        };

        let result = slice_diff_hunks(&[diff], &options);
        assert!(result.is_err());
    }

    #[test]
    fn merge_overlapping_ranges_basic() {
        let ranges = vec![(1, 5), (3, 8), (10, 15)];
        let merged = merge_overlapping_ranges(ranges);
        assert_eq!(merged, vec![(1, 8), (10, 15)]);
    }

    #[test]
    fn merge_adjacent_ranges() {
        let ranges = vec![(1, 5), (6, 10)];
        let merged = merge_overlapping_ranges(ranges);
        assert_eq!(merged, vec![(1, 10)]);
    }

    #[test]
    fn context_clamped_to_file_boundaries() {
        let source = "a\nb\nc\n";
        let (dir, root) = setup_source_file("small.rs", source);

        // Hunk at line 1 with 5 lines of context — should clamp to [1, 3].
        let diff = make_diff_file("small.rs", 1, 1);
        let options = SliceOptions {
            context_lines: 5,
            hunks_only: false,
            root,
        };

        let snippets = slice_diff_hunks(&[diff], &options).unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].start_line, 1);
        assert_eq!(snippets[0].end_line, 3);

        drop(dir);
    }
}
