//! Ranking and scoring for context snippets.
//!
//! Provides a TF-IDF–style scoring system with configurable weights for
//! multiple signals. In Phase 2, only the `text` signal is active; other
//! signals (diff, recency, proximity, test) are stubbed at 0.0 and will
//! be populated in later phases.

use crate::config::RankingWeights;
use crate::output::BundleSection;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Individual signal scores for a snippet.
///
/// Each field represents a normalised score in [0.0, 1.0] for a
/// particular relevance signal.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalScores {
    /// Text relevance (TF-IDF style match score).
    pub text: f64,
    /// Diff relevance (recently changed code). Stub = 0.0 in Phase 2.
    pub diff: f64,
    /// Recency (how recently the file was modified). Stub = 0.0.
    pub recency: f64,
    /// Proximity (closeness to other relevant snippets). Stub = 0.0.
    pub proximity: f64,
    /// Test relevance (is this a test file or tests affected code). Stub = 0.0.
    pub test: f64,
}

impl Default for SignalScores {
    fn default() -> Self {
        Self {
            text: 0.0,
            diff: 0.0,
            recency: 0.0,
            proximity: 0.0,
            test: 0.0,
        }
    }
}

/// A snippet annotated with a composite score and signal breakdown.
#[derive(Debug, Clone)]
pub struct ScoredSnippet {
    /// The original bundle section.
    pub section: BundleSection,
    /// Composite weighted score.
    pub score: f64,
    /// Individual signal breakdown.
    pub signals: SignalScores,
}

// ---------------------------------------------------------------------------
// Scoring functions
// ---------------------------------------------------------------------------

/// Rank snippets by weighted signal scores.
///
/// Computes a composite score for each snippet using the configured
/// weights, sorts by score descending, and breaks ties deterministically
/// by file path then line position.
pub fn rank_snippets(
    sections: &[BundleSection],
    match_counts: &[usize],
    weights: &RankingWeights,
) -> Vec<ScoredSnippet> {
    let total_matches: usize = match_counts.iter().sum();

    let mut scored: Vec<ScoredSnippet> = sections
        .iter()
        .zip(match_counts.iter())
        .map(|(section, &count)| {
            let signals = SignalScores {
                text: text_score(count, total_matches, sections.len()),
                diff: 0.0,
                recency: 0.0,
                proximity: 0.0,
                test: 0.0,
            };
            let score = weighted_score(&signals, weights);
            ScoredSnippet {
                section: section.clone(),
                score,
                signals,
            }
        })
        .collect();

    // Sort by score descending, tie-break on file path then reason (which
    // encodes position info for grep matches).
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.section.file_path.cmp(&b.section.file_path))
            .then_with(|| a.section.reason.cmp(&b.section.reason))
    });

    scored
}

/// Compute the text relevance score for a snippet.
///
/// Uses a TF-IDF–inspired formula: the snippet's match count divided by
/// total matches, weighted by inverse document frequency (log of total
/// sections / sections with matches).
pub fn text_score(match_count: usize, total_matches: usize, total_sections: usize) -> f64 {
    if total_matches == 0 || total_sections == 0 {
        return 0.0;
    }

    // Term frequency: proportion of matches in this snippet.
    let tf = match_count as f64 / total_matches as f64;

    // Inverse document frequency: log(total / matched).
    // Since we know this snippet has matches, idf is at least log(1) = 0.
    // We add 1 to avoid log(0) and to give non-zero score to single-match
    // scenarios.
    let idf = ((total_sections as f64) / (total_sections as f64).max(1.0)).ln() + 1.0;

    tf * idf
}

/// Compute the weighted composite score from signal scores.
pub fn weighted_score(signals: &SignalScores, weights: &RankingWeights) -> f64 {
    signals.text * weights.text
        + signals.diff * weights.diff
        + signals.recency * weights.recency
        + signals.proximity * weights.proximity
        + signals.test * weights.test
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sections() -> Vec<BundleSection> {
        vec![
            BundleSection {
                file_path: "src/main.rs".to_string(),
                language: "rust".to_string(),
                content: "fn main() {}".to_string(),
                reason: "grep match for 'fn'".to_string(),
            },
            BundleSection {
                file_path: "src/lib.rs".to_string(),
                language: "rust".to_string(),
                content: "pub mod config;".to_string(),
                reason: "grep match for 'fn'".to_string(),
            },
            BundleSection {
                file_path: "tests/test.rs".to_string(),
                language: "rust".to_string(),
                content: "#[test] fn it_works() {}".to_string(),
                reason: "grep match for 'fn'".to_string(),
            },
        ]
    }

    #[test]
    fn text_score_proportional_to_matches() {
        // 3 matches out of 10 total, 5 sections.
        let score_high = text_score(3, 10, 5);
        // 1 match out of 10 total, 5 sections.
        let score_low = text_score(1, 10, 5);
        assert!(score_high > score_low);
    }

    #[test]
    fn text_score_zero_on_no_matches() {
        assert_eq!(text_score(0, 0, 5), 0.0);
        assert_eq!(text_score(0, 10, 0), 0.0);
    }

    #[test]
    fn weighted_score_uses_weights() {
        let signals = SignalScores {
            text: 0.5,
            diff: 0.0,
            recency: 0.0,
            proximity: 0.0,
            test: 0.0,
        };
        let weights = RankingWeights {
            text: 2.0,
            diff: 1.0,
            recency: 0.5,
            proximity: 1.0,
            test: 0.5,
        };
        let score = weighted_score(&signals, &weights);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rank_snippets_sorts_by_score() {
        let sections = sample_sections();
        // Different match counts → different scores.
        let match_counts = vec![5, 1, 3];
        let weights = RankingWeights::default();
        let ranked = rank_snippets(&sections, &match_counts, &weights);

        assert_eq!(ranked.len(), 3);
        // Highest match count should be first.
        assert_eq!(ranked[0].section.file_path, "src/main.rs");
        // Scores should be in descending order.
        assert!(ranked[0].score >= ranked[1].score);
        assert!(ranked[1].score >= ranked[2].score);
    }

    #[test]
    fn rank_snippets_deterministic_tiebreak() {
        let sections = vec![
            BundleSection {
                file_path: "b.rs".to_string(),
                language: "rust".to_string(),
                content: "fn b() {}".to_string(),
                reason: "match".to_string(),
            },
            BundleSection {
                file_path: "a.rs".to_string(),
                language: "rust".to_string(),
                content: "fn a() {}".to_string(),
                reason: "match".to_string(),
            },
        ];
        // Equal match counts → tie.
        let match_counts = vec![1, 1];
        let weights = RankingWeights::default();
        let ranked = rank_snippets(&sections, &match_counts, &weights);

        // Should tie-break on file path (alphabetical).
        assert_eq!(ranked[0].section.file_path, "a.rs");
        assert_eq!(ranked[1].section.file_path, "b.rs");
    }

    #[test]
    fn rank_snippets_empty_input() {
        let ranked = rank_snippets(&[], &[], &RankingWeights::default());
        assert!(ranked.is_empty());
    }

    #[test]
    fn signal_scores_default_is_zero() {
        let s = SignalScores::default();
        assert_eq!(s.text, 0.0);
        assert_eq!(s.diff, 0.0);
        assert_eq!(s.recency, 0.0);
        assert_eq!(s.proximity, 0.0);
        assert_eq!(s.test, 0.0);
    }
}
