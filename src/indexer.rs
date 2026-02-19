//! Lightweight regex-based search across files.
//!
//! Provides text search functionality for the `collect --grep` command.
//! Searches across a set of [`ScannedFile`]s using regex patterns and
//! returns structured match results with file/line/column information.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

use crate::error::{ContextSmithError, Result};
use crate::scanner::ScannedFile;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single text match within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMatch {
    /// File path relative to the project root.
    pub file_path: String,
    /// Line number (1-based).
    pub line_number: usize,
    /// The full content of the matching line.
    pub line_content: String,
    /// Column (0-based byte offset) where the match starts.
    pub column: usize,
    /// Length of the match in bytes.
    pub match_length: usize,
}

/// Aggregated search results.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// All matches found.
    pub matches: Vec<TextMatch>,
    /// Number of files searched.
    pub files_searched: usize,
    /// Number of files with at least one match.
    pub files_matched: usize,
}

// ---------------------------------------------------------------------------
// Search functions
// ---------------------------------------------------------------------------

/// Search across multiple files for a regex pattern.
///
/// Reads each file, applies the pattern, and collects all matches.
/// Files that cannot be read (binary, permission errors) are silently
/// skipped.
pub fn search_files(files: &[ScannedFile], pattern: &str) -> Result<SearchResult> {
    let re = Regex::new(pattern).map_err(|e| ContextSmithError::pattern(pattern, e.to_string()))?;

    let mut all_matches = Vec::new();
    let mut files_matched = 0;

    for file in files {
        let content = match std::fs::read_to_string(&file.abs_path) {
            Ok(c) => c,
            Err(_) => continue, // Skip unreadable files (binary, permissions, etc.)
        };

        let file_matches = search_content(&re, &content, &file.rel_path);
        if !file_matches.is_empty() {
            files_matched += 1;
            all_matches.extend(file_matches);
        }
    }

    Ok(SearchResult {
        matches: all_matches,
        files_searched: files.len(),
        files_matched,
    })
}

/// Search within a single file's content for regex matches.
///
/// Returns a [`TextMatch`] for each line containing at least one match.
/// Multiple matches on the same line produce multiple entries.
pub fn search_content(re: &Regex, content: &str, file_path: &str) -> Vec<TextMatch> {
    let mut matches = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        for mat in re.find_iter(line) {
            matches.push(TextMatch {
                file_path: file_path.to_string(),
                line_number: line_idx + 1,
                line_content: line.to_string(),
                column: mat.start(),
                match_length: mat.len(),
            });
        }
    }

    matches
}

/// Group matches by file path.
///
/// Returns a map from file path to the list of matches in that file,
/// preserving the order of first appearance.
pub fn group_by_file(matches: &[TextMatch]) -> HashMap<String, Vec<&TextMatch>> {
    let mut grouped: HashMap<String, Vec<&TextMatch>> = HashMap::new();
    for m in matches {
        grouped.entry(m.file_path.clone()).or_default().push(m);
    }
    grouped
}

/// Compile a regex pattern, returning a descriptive error on failure.
pub fn compile_pattern(pattern: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|e| ContextSmithError::pattern(pattern, e.to_string()))
}

/// Read the content of a file, returning an error with context.
pub fn read_file_content(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| ContextSmithError::io(format!("reading '{}'", path.display()), e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_content_finds_matches() {
        let re = Regex::new("fn \\w+").unwrap();
        let content = "fn main() {\n    println!(\"hello\");\n}\nfn helper() {}";
        let matches = search_content(&re, content, "test.rs");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[0].line_content, "fn main() {");
        assert_eq!(matches[1].line_number, 4);
    }

    #[test]
    fn search_content_no_matches() {
        let re = Regex::new("class \\w+").unwrap();
        let content = "fn main() {}\nfn helper() {}";
        let matches = search_content(&re, content, "test.rs");
        assert!(matches.is_empty());
    }

    #[test]
    fn search_content_multiple_matches_per_line() {
        let re = Regex::new("\\bfoo\\b").unwrap();
        let content = "let foo = foo + foo;";
        let matches = search_content(&re, content, "test.rs");
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().all(|m| m.line_number == 1));
    }

    #[test]
    fn search_content_captures_column() {
        let re = Regex::new("hello").unwrap();
        let content = "say hello world";
        let matches = search_content(&re, content, "test.rs");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].column, 4);
        assert_eq!(matches[0].match_length, 5);
    }

    #[test]
    fn search_files_across_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn alpha() {}\nfn beta() {}").unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn gamma() {}").unwrap();
        std::fs::write(dir.path().join("c.txt"), "no functions here").unwrap();

        let files = vec![
            ScannedFile {
                rel_path: "a.rs".to_string(),
                abs_path: dir.path().join("a.rs"),
                language: "rust".to_string(),
                is_generated: false,
                size: 0,
            },
            ScannedFile {
                rel_path: "b.rs".to_string(),
                abs_path: dir.path().join("b.rs"),
                language: "rust".to_string(),
                is_generated: false,
                size: 0,
            },
            ScannedFile {
                rel_path: "c.txt".to_string(),
                abs_path: dir.path().join("c.txt"),
                language: "".to_string(),
                is_generated: false,
                size: 0,
            },
        ];

        let result = search_files(&files, "fn \\w+").unwrap();
        assert_eq!(result.files_searched, 3);
        assert_eq!(result.files_matched, 2);
        assert_eq!(result.matches.len(), 3);
    }

    #[test]
    fn search_files_invalid_pattern_errors() {
        let result = search_files(&[], "[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn group_by_file_groups_correctly() {
        let matches = vec![
            TextMatch {
                file_path: "a.rs".to_string(),
                line_number: 1,
                line_content: "fn a()".to_string(),
                column: 0,
                match_length: 4,
            },
            TextMatch {
                file_path: "b.rs".to_string(),
                line_number: 1,
                line_content: "fn b()".to_string(),
                column: 0,
                match_length: 4,
            },
            TextMatch {
                file_path: "a.rs".to_string(),
                line_number: 5,
                line_content: "fn c()".to_string(),
                column: 0,
                match_length: 4,
            },
        ];

        let grouped = group_by_file(&matches);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["a.rs"].len(), 2);
        assert_eq!(grouped["b.rs"].len(), 1);
    }

    #[test]
    fn compile_pattern_valid() {
        assert!(compile_pattern("fn \\w+").is_ok());
    }
}
