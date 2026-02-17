//! Manifest types for tracking what was included in a context bundle.
//!
//! A manifest records every candidate snippet, its token estimate, score,
//! and whether it was included in the final output. This enables the
//! `explain` command and budget introspection.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ContextSmithError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Complete manifest describing a context bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    /// Aggregate statistics.
    pub summary: ManifestSummary,
    /// Per-snippet entries (both included and excluded).
    pub entries: Vec<ManifestEntry>,
}

/// Summary statistics for the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestSummary {
    /// Total estimated tokens across all included entries.
    pub total_tokens: usize,
    /// Token budget (if one was set).
    pub budget: Option<usize>,
    /// Tokens reserved for model response.
    pub reserve_tokens: usize,
    /// Total number of candidate snippets.
    pub snippet_count: usize,
    /// Number of snippets included in the output.
    pub included_count: usize,
    /// Model name used for estimation.
    pub model: String,
    /// Ranking weights used (if applicable).
    pub weights_used: Option<WeightsUsed>,
}

/// Ranking weights applied during snippet selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeightsUsed {
    pub text: f64,
    pub diff: f64,
    pub recency: f64,
    pub proximity: f64,
    pub test: f64,
}

/// A single snippet entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestEntry {
    /// File path relative to project root.
    pub file_path: String,
    /// First line number (1-based).
    pub start_line: usize,
    /// Last line number (1-based, inclusive).
    pub end_line: usize,
    /// Estimated token count.
    pub token_estimate: usize,
    /// Character count.
    pub char_count: usize,
    /// Why this snippet was considered.
    pub reason: String,
    /// Ranking score (higher = more relevant).
    pub score: f64,
    /// Whether this snippet was included in the final output.
    pub included: bool,
    /// Programming language identifier.
    pub language: String,
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Build a manifest from a list of entries and metadata.
pub fn build_manifest(
    entries: Vec<ManifestEntry>,
    model: &str,
    budget: Option<usize>,
    reserve: usize,
) -> Manifest {
    let total_tokens: usize = entries
        .iter()
        .filter(|e| e.included)
        .map(|e| e.token_estimate)
        .sum();
    let included_count = entries.iter().filter(|e| e.included).count();

    Manifest {
        summary: ManifestSummary {
            total_tokens,
            budget,
            reserve_tokens: reserve,
            snippet_count: entries.len(),
            included_count,
            model: model.to_string(),
            weights_used: None,
        },
        entries,
    }
}

/// Write a manifest to a JSON file.
pub fn write_manifest(manifest: &Manifest, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| {
        ContextSmithError::config_with_source("failed to serialize manifest as JSON", e)
    })?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            ContextSmithError::io(format!("creating directory '{}'", parent.display()), e)
        })?;
    }

    std::fs::write(path, json)
        .map_err(|e| ContextSmithError::io(format!("writing manifest to '{}'", path.display()), e))
}

/// Read a manifest from a JSON file.
pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ContextSmithError::io(format!("reading manifest '{}'", path.display()), e))?;

    serde_json::from_str(&content).map_err(|e| {
        ContextSmithError::config_with_source(
            format!("failed to parse manifest '{}'", path.display()),
            e,
        )
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<ManifestEntry> {
        vec![
            ManifestEntry {
                file_path: "src/main.rs".to_string(),
                start_line: 1,
                end_line: 10,
                token_estimate: 50,
                char_count: 200,
                reason: "modified in diff".to_string(),
                score: 1.0,
                included: true,
                language: "rust".to_string(),
            },
            ManifestEntry {
                file_path: "src/lib.rs".to_string(),
                start_line: 5,
                end_line: 20,
                token_estimate: 80,
                char_count: 320,
                reason: "modified in diff".to_string(),
                score: 0.8,
                included: false,
                language: "rust".to_string(),
            },
        ]
    }

    #[test]
    fn build_manifest_computes_summary() {
        let manifest = build_manifest(sample_entries(), "gpt-4", Some(100), 0);
        assert_eq!(manifest.summary.total_tokens, 50); // only included
        assert_eq!(manifest.summary.included_count, 1);
        assert_eq!(manifest.summary.snippet_count, 2);
        assert_eq!(manifest.summary.model, "gpt-4");
        assert_eq!(manifest.summary.budget, Some(100));
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let manifest = build_manifest(sample_entries(), "claude", Some(500), 100);
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
    }

    #[test]
    fn write_and_read_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");

        let manifest = build_manifest(sample_entries(), "gpt-4", None, 0);
        write_manifest(&manifest, &path).unwrap();
        let loaded = read_manifest(&path).unwrap();
        assert_eq!(manifest, loaded);
    }

    #[test]
    fn empty_entries() {
        let manifest = build_manifest(vec![], "gpt-4", Some(1000), 0);
        assert_eq!(manifest.summary.total_tokens, 0);
        assert_eq!(manifest.summary.included_count, 0);
        assert_eq!(manifest.summary.snippet_count, 0);
    }

    #[test]
    fn read_nonexistent_manifest_errors() {
        let result = read_manifest(Path::new("/tmp/does_not_exist_manifest.json"));
        assert!(result.is_err());
    }
}
