//! Symbol search abstraction for finding definitions in source code.
//!
//! Provides a trait-based design so regex-based search (Phase 2) can be
//! swapped for tree-sitter–based search (Phase 3) without changing
//! downstream code.

use regex::Regex;

use crate::error::{ContextSmithError, Result};
use crate::indexer::{self, TextMatch};
use crate::scanner::ScannedFile;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Finds symbol definitions across source files.
///
/// Implementations search for definitions of a named symbol (function,
/// struct, class, type, etc.) using different strategies:
/// - [`RegexSymbolFinder`]: regex-based heuristic (Phase 2)
/// - Future: `TreeSitterSymbolFinder` using AST parsing (Phase 3)
pub trait SymbolFinder: Send + Sync {
    /// Find definitions of the given symbol name across files.
    fn find_definitions(&self, files: &[ScannedFile], symbol: &str) -> Result<Vec<TextMatch>>;
}

// ---------------------------------------------------------------------------
// Regex-based implementation
// ---------------------------------------------------------------------------

/// Regex-based symbol finder.
///
/// Uses language-agnostic regex patterns to find common definition forms
/// like `fn name`, `struct Name`, `class Name`, `def name`, etc.
/// Accuracy is lower than AST-based search but works across all languages
/// without additional dependencies.
pub struct RegexSymbolFinder;

impl SymbolFinder for RegexSymbolFinder {
    fn find_definitions(&self, files: &[ScannedFile], symbol: &str) -> Result<Vec<TextMatch>> {
        let pattern = build_symbol_pattern(symbol);
        let re = Regex::new(&pattern)
            .map_err(|e| ContextSmithError::pattern(&pattern, e.to_string()))?;

        let mut all_matches = Vec::new();

        for file in files {
            let content = match std::fs::read_to_string(&file.abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let matches = indexer::search_content(&re, &content, &file.rel_path);
            all_matches.extend(matches);
        }

        Ok(all_matches)
    }
}

/// Build a regex pattern that matches common definition forms for a symbol.
///
/// Covers:
/// - Rust: `fn name`, `struct Name`, `enum Name`, `trait Name`, `type Name`,
///   `const NAME`, `static NAME`, `mod name`, `impl Name`
/// - Python: `def name`, `class Name`
/// - JavaScript/TypeScript: `function name`, `class Name`, `const name`,
///   `let name`, `var name`, `interface Name`, `type Name`
/// - Go: `func name`, `type Name`
/// - Ruby: `def name`, `class Name`, `module Name`
/// - Java/Kotlin: `class Name`, `interface Name`, `enum Name`
/// - General: `Name =` (assignment)
pub fn build_symbol_pattern(symbol: &str) -> String {
    // Escape the symbol name for use in regex.
    let escaped = regex::escape(symbol);

    // Build alternation of common definition keywords.
    format!(
        r"(?:^|\s)(?:pub\s+(?:(?:unsafe\s+)?(?:async\s+)?)?|export\s+(?:default\s+)?|(?:async\s+)?)?(?:fn|struct|enum|trait|type|const|static|mod|impl|def|class|function|func|interface|module|let|var)\s+{escaped}\b"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_symbol_pattern_matches_rust_fn() {
        let pattern = build_symbol_pattern("run");
        let re = Regex::new(&pattern).unwrap();
        assert!(re.is_match("fn run() {"));
        assert!(re.is_match("pub fn run() {"));
        assert!(re.is_match("pub async fn run() {"));
        assert!(!re.is_match("fn running() {"));
    }

    #[test]
    fn build_symbol_pattern_matches_struct() {
        let pattern = build_symbol_pattern("Config");
        let re = Regex::new(&pattern).unwrap();
        assert!(re.is_match("struct Config {"));
        assert!(re.is_match("pub struct Config {"));
        assert!(re.is_match("class Config:"));
    }

    #[test]
    fn build_symbol_pattern_matches_python_def() {
        let pattern = build_symbol_pattern("process");
        let re = Regex::new(&pattern).unwrap();
        assert!(re.is_match("def process(data):"));
        assert!(re.is_match("async def process(data):"));
    }

    #[test]
    fn regex_symbol_finder_across_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.rs"),
            "pub fn run() {\n    println!(\"hello\");\n}\n\nfn helper() {}",
        )
        .unwrap();
        std::fs::write(dir.path().join("b.rs"), "fn other() {}\nfn run_tests() {}").unwrap();

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
        ];

        let finder = RegexSymbolFinder;
        let matches = finder.find_definitions(&files, "run").unwrap();
        // Should find "pub fn run()" in a.rs but not "fn run_tests()" in b.rs
        // (word boundary prevents partial match).
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, "a.rs");
    }
}
