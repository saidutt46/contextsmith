//! Output formatting for ContextSmith bundles.
//!
//! Transforms a [`Bundle`] of collected snippets into the user's chosen
//! format (Markdown, JSON, plain text, or XML) and writes the result
//! to a file or stdout.
//!
//! All commands that produce output should build a [`Bundle`], pick a
//! formatter, and call [`write_output`] — this keeps presentation logic
//! centralised and consistent across the CLI.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{ContextSmithError, Result};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Controls where and how output is written.
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Desired output format.
    pub format: Format,
    /// If true, write to stdout instead of a file.
    pub stdout: bool,
    /// File path to write to (ignored when `stdout` is true).
    pub out: Option<PathBuf>,
}

/// Supported output formats.
///
/// Mirrors [`crate::cli::OutputFormat`] but decoupled from clap so that
/// library code can use it without pulling in CLI dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Json,
    Plain,
    Xml,
}

/// A complete output bundle ready for formatting.
///
/// This is the universal intermediate representation that every command
/// builds before handing off to a formatter.
#[derive(Debug, Clone, Serialize)]
pub struct Bundle {
    /// Human-readable summary (e.g. "3 files changed, 5 hunks").
    pub summary: String,
    /// Ordered list of content sections.
    pub sections: Vec<BundleSection>,
}

/// A single section within a [`Bundle`], typically one per file.
#[derive(Debug, Clone, Serialize)]
pub struct BundleSection {
    /// File path relative to the project root.
    pub file_path: String,
    /// Programming language identifier for syntax highlighting.
    pub language: String,
    /// The content to display.
    pub content: String,
    /// Why this section was included (e.g. "modified in diff").
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Render a bundle to a string in the given format.
pub fn format_bundle(bundle: &Bundle, format: Format) -> Result<String> {
    match format {
        Format::Markdown => Ok(format_markdown(bundle)),
        Format::Json => format_json(bundle),
        Format::Plain => Ok(format_plain(bundle)),
        Format::Xml => Ok(format_xml(bundle)),
    }
}

/// Markdown: fenced code blocks with file-path headers.
///
/// Produces output suitable for direct pasting into LLM prompts:
/// ```text
/// # Context Bundle
/// > 3 files changed, 5 hunks
///
/// ## `src/main.rs`
/// *modified in diff*
/// ```rust
/// fn main() { ... }
/// ```
/// ```
fn format_markdown(bundle: &Bundle) -> String {
    let mut out = String::new();
    out.push_str("# Context Bundle\n\n");
    if !bundle.summary.is_empty() {
        out.push_str(&format!("> {}\n\n", bundle.summary));
    }

    for section in &bundle.sections {
        out.push_str(&format!("## `{}`\n", section.file_path));
        if !section.reason.is_empty() {
            out.push_str(&format!("*{}*\n", section.reason));
        }
        out.push_str(&format!("```{}\n", section.language));
        out.push_str(&section.content);
        if !section.content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    out
}

/// JSON: serialise the full bundle using serde.
fn format_json(bundle: &Bundle) -> Result<String> {
    serde_json::to_string_pretty(bundle)
        .map_err(|e| ContextSmithError::config_with_source("failed to serialize bundle as JSON", e))
}

/// Plain text: file paths followed by raw content, no decoration.
fn format_plain(bundle: &Bundle) -> String {
    let mut out = String::new();
    if !bundle.summary.is_empty() {
        out.push_str(&bundle.summary);
        out.push_str("\n\n");
    }

    for section in &bundle.sections {
        out.push_str(&format!("--- {} ---\n", section.file_path));
        out.push_str(&section.content);
        if !section.content.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// XML: structured tags for machine consumption.
fn format_xml(bundle: &Bundle) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<bundle>\n");
    out.push_str(&format!(
        "  <summary>{}</summary>\n",
        escape_xml(&bundle.summary)
    ));

    for section in &bundle.sections {
        out.push_str("  <section>\n");
        out.push_str(&format!(
            "    <file_path>{}</file_path>\n",
            escape_xml(&section.file_path)
        ));
        out.push_str(&format!(
            "    <language>{}</language>\n",
            escape_xml(&section.language)
        ));
        out.push_str(&format!(
            "    <reason>{}</reason>\n",
            escape_xml(&section.reason)
        ));
        out.push_str("    <content><![CDATA[");
        out.push_str(&section.content);
        out.push_str("]]></content>\n");
        out.push_str("  </section>\n");
    }

    out.push_str("</bundle>\n");
    out
}

/// Minimal XML escaping for attribute/text content.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Output writing
// ---------------------------------------------------------------------------

/// Write a formatted string to the appropriate destination.
///
/// If `options.stdout` is true, writes to stdout. Otherwise writes to the
/// file at `options.out` (creating parent directories as needed).
pub fn write_output(content: &str, options: &FormatOptions) -> Result<()> {
    if options.stdout {
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(content.as_bytes())
            .map_err(|e| ContextSmithError::io("writing to stdout", e))?;
        return Ok(());
    }

    if let Some(ref path) = options.out {
        write_to_file(content, path)
    } else {
        // Default to stdout when no file path is given.
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(content.as_bytes())
            .map_err(|e| ContextSmithError::io("writing to stdout", e))
    }
}

/// Write content to a file, creating parent directories if needed.
fn write_to_file(content: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            ContextSmithError::io(format!("creating directory '{}'", parent.display()), e)
        })?;
    }
    std::fs::write(path, content)
        .map_err(|e| ContextSmithError::io(format!("writing output to '{}'", path.display()), e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bundle() -> Bundle {
        Bundle {
            summary: "2 files changed".to_string(),
            sections: vec![
                BundleSection {
                    file_path: "src/main.rs".to_string(),
                    language: "rust".to_string(),
                    content: "fn main() {}\n".to_string(),
                    reason: "modified in diff".to_string(),
                },
                BundleSection {
                    file_path: "README.md".to_string(),
                    language: "markdown".to_string(),
                    content: "# Hello\n".to_string(),
                    reason: "added".to_string(),
                },
            ],
        }
    }

    #[test]
    fn markdown_contains_file_headers() {
        let output = format_markdown(&sample_bundle());
        assert!(output.contains("## `src/main.rs`"));
        assert!(output.contains("## `README.md`"));
        assert!(output.contains("```rust"));
        assert!(output.contains("```markdown"));
    }

    #[test]
    fn markdown_contains_summary() {
        let output = format_markdown(&sample_bundle());
        assert!(output.contains("> 2 files changed"));
    }

    #[test]
    fn json_is_valid() {
        let output = format_json(&sample_bundle()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["summary"], "2 files changed");
        assert_eq!(parsed["sections"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn plain_contains_file_separators() {
        let output = format_plain(&sample_bundle());
        assert!(output.contains("--- src/main.rs ---"));
        assert!(output.contains("--- README.md ---"));
    }

    #[test]
    fn xml_is_well_formed() {
        let output = format_xml(&sample_bundle());
        assert!(output.starts_with("<?xml"));
        assert!(output.contains("<file_path>src/main.rs</file_path>"));
        assert!(output.contains("<![CDATA[fn main() {}\n]]>"));
        assert!(output.contains("</bundle>"));
    }

    #[test]
    fn xml_escapes_special_characters() {
        let escaped = escape_xml("x < y & z > w");
        assert_eq!(escaped, "x &lt; y &amp; z &gt; w");
    }

    #[test]
    fn write_to_file_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("dir").join("output.md");
        write_to_file("hello", &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
    }
}
