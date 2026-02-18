//! Shared utility functions used across multiple commands.
//!
//! Centralises helpers that were previously duplicated in `diff.rs` and
//! `pack.rs`: language inference, CLI format mapping, and manifest path
//! computation.

use std::path::Path;

use crate::cli::OutputFormat;
use crate::output::Format;

// ---------------------------------------------------------------------------
// Language inference
// ---------------------------------------------------------------------------

/// Infer a syntax-highlighting language identifier from a file path.
///
/// Checks the file extension first, then falls back to well-known
/// filenames (e.g. `Dockerfile`, `.gitignore`).
pub fn infer_language(path: &str) -> String {
    // Try extension first.
    let ext = path.rsplit('.').next().unwrap_or("");
    let from_ext = match ext {
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
        "graphql" | "gql" => "graphql",
        "proto" => "protobuf",
        "tf" => "hcl",
        "lock" => "toml",
        _ => "",
    };

    if !from_ext.is_empty() {
        return from_ext.to_string();
    }

    // Fall back to well-known filenames.
    let filename = path.rsplit('/').next().unwrap_or(path);
    match filename {
        "Dockerfile" | "Containerfile" => "dockerfile",
        "Makefile" | "GNUmakefile" => "makefile",
        "Justfile" | "justfile" => "makefile",
        "CMakeLists.txt" => "cmake",
        ".gitignore" | ".dockerignore" | ".prettierignore" | ".eslintignore" => "gitignore",
        ".env" | ".env.local" | ".env.example" => "dotenv",
        "Gemfile" => "ruby",
        "Rakefile" => "ruby",
        "Vagrantfile" => "ruby",
        _ => "",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Format mapping
// ---------------------------------------------------------------------------

/// Map the clap [`OutputFormat`] to the library [`Format`].
pub fn cli_format_to_output_format(fmt: &OutputFormat) -> Format {
    match fmt {
        OutputFormat::Markdown => Format::Markdown,
        OutputFormat::Json => Format::Json,
        OutputFormat::Plain => Format::Plain,
        OutputFormat::Xml => Format::Xml,
    }
}

// ---------------------------------------------------------------------------
// Manifest path
// ---------------------------------------------------------------------------

/// Compute the manifest sibling path for a given output file.
///
/// `output.md` -> `output.manifest.json`
pub fn manifest_sibling_path(out_path: &Path) -> std::path::PathBuf {
    let stem = out_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let parent = out_path.parent().unwrap_or(Path::new("."));
    parent.join(format!("{stem}.manifest.json"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_language_from_extension() {
        assert_eq!(infer_language("src/main.rs"), "rust");
        assert_eq!(infer_language("app.ts"), "typescript");
        assert_eq!(infer_language("index.js"), "javascript");
        assert_eq!(infer_language("script.py"), "python");
        assert_eq!(infer_language("config.toml"), "toml");
    }

    #[test]
    fn infer_language_from_filename() {
        assert_eq!(infer_language("Dockerfile"), "dockerfile");
        assert_eq!(infer_language("Makefile"), "makefile");
        assert_eq!(infer_language(".gitignore"), "gitignore");
    }

    #[test]
    fn infer_language_unknown() {
        assert_eq!(infer_language("README"), "");
        assert_eq!(infer_language("data.bin"), "");
    }

    #[test]
    fn cli_format_mapping() {
        assert_eq!(
            cli_format_to_output_format(&OutputFormat::Markdown),
            Format::Markdown
        );
        assert_eq!(
            cli_format_to_output_format(&OutputFormat::Json),
            Format::Json
        );
        assert_eq!(
            cli_format_to_output_format(&OutputFormat::Plain),
            Format::Plain
        );
        assert_eq!(cli_format_to_output_format(&OutputFormat::Xml), Format::Xml);
    }

    #[test]
    fn manifest_sibling_path_basic() {
        use std::path::PathBuf;

        let path = PathBuf::from("/tmp/output.md");
        assert_eq!(
            manifest_sibling_path(&path),
            PathBuf::from("/tmp/output.manifest.json")
        );

        let path2 = PathBuf::from("bundle.json");
        assert_eq!(
            manifest_sibling_path(&path2),
            PathBuf::from("bundle.manifest.json")
        );
    }
}
