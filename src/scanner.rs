//! File discovery with .gitignore and config-based filtering.
//!
//! Walks a project directory using the `ignore` crate (which respects
//! `.gitignore`, `.ignore`, and similar files), then applies additional
//! filters from `contextsmith.toml` (ignore patterns, generated file
//! patterns, language filters).

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{ContextSmithError, Result};
use crate::utils;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A discovered file with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Path relative to the scan root.
    pub rel_path: String,
    /// Absolute path on disk.
    pub abs_path: PathBuf,
    /// Inferred programming language identifier.
    pub language: String,
    /// Whether this file matches generated-code patterns.
    pub is_generated: bool,
    /// File size in bytes.
    pub size: u64,
}

/// Options controlling file discovery.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Root directory to scan.
    pub root: PathBuf,
    /// Glob patterns for files to ignore (from config).
    pub ignore_patterns: Vec<String>,
    /// Glob patterns for generated files (from config).
    pub generated_patterns: Vec<String>,
    /// If set, only include files matching this language.
    pub lang_filter: Option<String>,
    /// If set, only include files matching this path glob.
    pub path_filter: Option<String>,
    /// Additional exclude patterns (from CLI --exclude).
    pub exclude_patterns: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core scanning
// ---------------------------------------------------------------------------

/// Walk the project directory and return all discoverable source files.
///
/// Respects `.gitignore` (via the `ignore` crate), then applies config
/// ignore patterns, generated file detection, and optional filters.
pub fn scan(options: &ScanOptions) -> Result<Vec<ScannedFile>> {
    let root = options.root.canonicalize().map_err(|e| {
        ContextSmithError::io(
            format!("canonicalizing root '{}'", options.root.display()),
            e,
        )
    })?;

    let mut builder = ignore::WalkBuilder::new(&root);
    builder.hidden(false).git_ignore(true).git_global(true);

    // Add config ignore patterns as custom globs.
    for pattern in &options.ignore_patterns {
        builder.add_ignore(create_ignore_file(pattern));
    }

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Only process files.
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let abs_path = entry.path().to_path_buf();
        let rel_path = abs_path
            .strip_prefix(&root)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .to_string();

        // Apply exclude patterns.
        if matches_any_pattern(&rel_path, &options.exclude_patterns) {
            continue;
        }

        // Apply config ignore patterns (simple substring/glob matching).
        if matches_any_pattern(&rel_path, &options.ignore_patterns) {
            continue;
        }

        let language = utils::infer_language(&rel_path);

        // Apply language filter.
        if let Some(ref lang) = options.lang_filter {
            if !language.eq_ignore_ascii_case(lang) {
                continue;
            }
        }

        // Apply path filter (simple glob matching).
        if let Some(ref path_glob) = options.path_filter {
            if !simple_glob_match(path_glob, &rel_path) {
                continue;
            }
        }

        let is_generated = is_generated_file(&rel_path, &options.generated_patterns);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        files.push(ScannedFile {
            rel_path,
            abs_path,
            language,
            is_generated,
            size,
        });
    }

    // Sort by relative path for deterministic output.
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    Ok(files)
}

/// Build `ScanOptions` from a config and root path.
pub fn scan_options_from_config(config: &Config, root: &Path) -> ScanOptions {
    ScanOptions {
        root: root.to_path_buf(),
        ignore_patterns: config.ignore.clone(),
        generated_patterns: config.generated.clone(),
        lang_filter: None,
        path_filter: None,
        exclude_patterns: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Generated file detection
// ---------------------------------------------------------------------------

/// Check whether a file path matches any generated-code pattern.
pub fn is_generated_file(rel_path: &str, patterns: &[String]) -> bool {
    matches_any_pattern(rel_path, patterns)
}

/// Check whether file content contains a generated-code marker.
///
/// Looks for common markers like `@generated`, `DO NOT EDIT`, etc.
/// in the first few lines of the file.
pub fn has_generated_marker(content: &str) -> bool {
    let header = content.lines().take(10).collect::<Vec<_>>().join("\n");
    let lower = header.to_lowercase();
    lower.contains("@generated")
        || lower.contains("do not edit")
        || lower.contains("auto-generated")
        || lower.contains("automatically generated")
}

// ---------------------------------------------------------------------------
// Pattern matching helpers
// ---------------------------------------------------------------------------

/// Check if a path matches any of the given patterns.
///
/// Supports simple glob-style matching: `*` matches any sequence of
/// non-separator characters, `**` is not yet supported but patterns
/// are also checked as simple substring contains.
fn matches_any_pattern(path: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| simple_glob_match(p, path) || path.contains(p.trim_start_matches('*')))
}

/// Minimal glob matching for ignore patterns.
///
/// Handles `*.ext` prefix wildcards, `dir/` directory patterns,
/// and patterns with `*` in the middle (e.g. `*.generated.*`).
/// Falls back to substring matching for other patterns.
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    if pattern.contains('*') {
        // Split on '*' and check that all parts appear in order.
        let parts: Vec<&str> = pattern.split('*').collect();
        let filename = path.rsplit('/').next().unwrap_or(path);
        let mut remaining = filename;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                // First part must be a prefix.
                if let Some(rest) = remaining.strip_prefix(part) {
                    remaining = rest;
                } else {
                    return false;
                }
            } else if i == parts.len() - 1 {
                // Last part must be a suffix.
                if !remaining.ends_with(part) {
                    return false;
                }
                remaining = "";
            } else if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return false;
            }
        }
        true
    } else if pattern.ends_with('/') {
        // Match directory prefix.
        let dir = pattern.trim_end_matches('/');
        path.starts_with(dir) || path.contains(&format!("/{dir}/"))
    } else {
        // Exact match or component match.
        path == pattern
            || path.ends_with(&format!("/{pattern}"))
            || path.starts_with(&format!("{pattern}/"))
            || path.contains(&format!("/{pattern}/"))
    }
}

/// Create a temporary ignore file from a single pattern.
///
/// The `ignore` crate's `WalkBuilder` accepts paths to ignore files,
/// but we need to add patterns programmatically. This is a workaround
/// that writes the pattern to a temp location.
fn create_ignore_file(pattern: &str) -> PathBuf {
    let _ = pattern; // Pattern is used via the matches_any_pattern helper instead.
                     // We handle custom patterns in our own filtering logic rather than
                     // through the ignore crate's file-based mechanism. Return a
                     // non-existent path which the builder will silently skip.
    PathBuf::from("/dev/null/.contextsmith-ignore-placeholder")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_glob_match_extension() {
        assert!(simple_glob_match("*.rs", "src/main.rs"));
        assert!(simple_glob_match("*.py", "scripts/run.py"));
        assert!(!simple_glob_match("*.rs", "src/main.py"));
    }

    #[test]
    fn simple_glob_match_directory() {
        assert!(simple_glob_match("node_modules", "node_modules/foo.js"));
        assert!(simple_glob_match("target", "target/debug/binary"));
        assert!(!simple_glob_match("target", "src/target_utils.rs"));
    }

    #[test]
    fn is_generated_file_matches_patterns() {
        let patterns = vec!["*.pb.rs".to_string(), "*.generated.*".to_string()];
        assert!(is_generated_file("proto/message.pb.rs", &patterns));
        assert!(is_generated_file("src/schema.generated.ts", &patterns));
        assert!(!is_generated_file("src/main.rs", &patterns));
    }

    #[test]
    fn has_generated_marker_detects_markers() {
        assert!(has_generated_marker("// @generated\nfn foo() {}"));
        assert!(has_generated_marker("# DO NOT EDIT\nimport foo"));
        assert!(has_generated_marker(
            "/* Auto-generated by protoc */\nmessage Foo {}"
        ));
        assert!(!has_generated_marker(
            "fn main() {\n    println!(\"hello\");\n}"
        ));
    }

    #[test]
    fn scan_finds_files_in_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub mod foo;").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/helper.rs"), "fn help() {}").unwrap();

        let options = ScanOptions {
            root: dir.path().to_path_buf(),
            ignore_patterns: vec![],
            generated_patterns: vec![],
            lang_filter: None,
            path_filter: None,
            exclude_patterns: vec![],
        };

        let files = scan(&options).unwrap();
        assert!(files.len() >= 3);
        assert!(files.iter().any(|f| f.rel_path == "main.rs"));
        assert!(files.iter().any(|f| f.rel_path.contains("helper.rs")));
    }

    #[test]
    fn scan_respects_language_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("script.py"), "print('hi')").unwrap();

        let options = ScanOptions {
            root: dir.path().to_path_buf(),
            ignore_patterns: vec![],
            generated_patterns: vec![],
            lang_filter: Some("rust".to_string()),
            path_filter: None,
            exclude_patterns: vec![],
        };

        let files = scan(&options).unwrap();
        assert!(files.iter().all(|f| f.language == "rust"));
    }

    #[test]
    fn scan_respects_exclude_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
        std::fs::write(dir.path().join("vendor/dep.rs"), "fn dep() {}").unwrap();

        let options = ScanOptions {
            root: dir.path().to_path_buf(),
            ignore_patterns: vec![],
            generated_patterns: vec![],
            lang_filter: None,
            path_filter: None,
            exclude_patterns: vec!["vendor".to_string()],
        };

        let files = scan(&options).unwrap();
        assert!(!files.iter().any(|f| f.rel_path.contains("vendor")));
    }

    #[test]
    fn scan_marks_generated_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("schema.pb.rs"), "// generated").unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let options = ScanOptions {
            root: dir.path().to_path_buf(),
            ignore_patterns: vec![],
            generated_patterns: vec!["*.pb.rs".to_string()],
            lang_filter: None,
            path_filter: None,
            exclude_patterns: vec![],
        };

        let files = scan(&options).unwrap();
        let generated = files.iter().find(|f| f.rel_path == "schema.pb.rs");
        assert!(generated.is_some());
        assert!(generated.unwrap().is_generated);

        let regular = files.iter().find(|f| f.rel_path == "main.rs");
        assert!(regular.is_some());
        assert!(!regular.unwrap().is_generated);
    }

    #[test]
    fn scan_options_from_config_uses_defaults() {
        let config = Config::default();
        let root = PathBuf::from("/tmp/test");
        let options = scan_options_from_config(&config, &root);
        assert_eq!(options.root, root);
        assert!(!options.ignore_patterns.is_empty());
        assert!(!options.generated_patterns.is_empty());
        assert!(options.lang_filter.is_none());
    }
}
