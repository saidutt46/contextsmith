//! Token estimation for context budgeting.
//!
//! Provides a trait-based architecture for token counting, with a built-in
//! character heuristic as the default implementation. Real tokenizers
//! (tiktoken-rs, custom BPE, etc.) can be plugged in by implementing
//! the [`TokenEstimator`] trait.

// ---------------------------------------------------------------------------
// Trait (extensibility point)
// ---------------------------------------------------------------------------

/// Estimates token counts for a given text.
///
/// Implementations range from fast character heuristics to exact BPE
/// tokenizers. All implementations must be thread-safe.
pub trait TokenEstimator: Send + Sync {
    /// Estimate the number of tokens in `text`.
    fn estimate(&self, text: &str) -> usize;

    /// Human-readable name of the model this estimator targets.
    fn model_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Model families
// ---------------------------------------------------------------------------

/// Known model families with tuned character-per-token ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFamily {
    /// GPT-4 and GPT-4 Turbo (~4 chars/token).
    Gpt4,
    /// GPT-3.5 Turbo (~4 chars/token).
    Gpt35,
    /// Anthropic Claude models (~3.5 chars/token).
    Claude,
    /// Unknown model — uses conservative 4 chars/token.
    Unknown,
}

impl ModelFamily {
    /// Default characters-per-token ratio for this model family.
    fn chars_per_token(self) -> f64 {
        match self {
            Self::Gpt4 | Self::Gpt35 | Self::Unknown => 4.0,
            Self::Claude => 3.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Character-based estimator
// ---------------------------------------------------------------------------

/// Token estimator that uses a character-count heuristic.
///
/// Fast and dependency-free. Accuracy is ±15-20% compared to real BPE
/// tokenizers, which is sufficient for budget planning.
#[derive(Debug, Clone)]
pub struct CharEstimator {
    model: ModelFamily,
    chars_per_token: f64,
}

impl CharEstimator {
    /// Create a new estimator for the given model family.
    pub fn new(model: ModelFamily) -> Self {
        Self {
            chars_per_token: model.chars_per_token(),
            model,
        }
    }
}

impl TokenEstimator for CharEstimator {
    fn estimate(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let chars = text.len() as f64;
        (chars / self.chars_per_token).ceil() as usize
    }

    fn model_name(&self) -> &str {
        match self.model {
            ModelFamily::Gpt4 => "gpt-4",
            ModelFamily::Gpt35 => "gpt-3.5-turbo",
            ModelFamily::Claude => "claude",
            ModelFamily::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Parse a model name string into a [`ModelFamily`].
///
/// Recognises common model name prefixes and substrings. Unrecognised
/// names map to [`ModelFamily::Unknown`].
pub fn parse_model(name: &str) -> ModelFamily {
    let lower = name.to_lowercase();
    if lower.contains("claude") {
        ModelFamily::Claude
    } else if lower.contains("gpt-4") || lower.contains("gpt4") {
        ModelFamily::Gpt4
    } else if lower.contains("gpt-3") || lower.contains("gpt3") {
        ModelFamily::Gpt35
    } else {
        ModelFamily::Unknown
    }
}

/// Create a default estimator targeting GPT-4 (most conservative).
pub fn default_estimator() -> CharEstimator {
    CharEstimator::new(ModelFamily::Gpt4)
}

/// Create an estimator for the named model.
pub fn estimator_for_model(name: &str) -> CharEstimator {
    CharEstimator::new(parse_model(name))
}

/// Estimate token count using the given model family's heuristic.
pub fn estimate_tokens(text: &str, model: ModelFamily) -> usize {
    CharEstimator::new(model).estimate(text)
}

/// Estimate token count using the default (GPT-4) heuristic.
pub fn estimate_tokens_default(text: &str) -> usize {
    default_estimator().estimate(text)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_is_zero_tokens() {
        assert_eq!(estimate_tokens_default(""), 0);
        assert_eq!(estimate_tokens("", ModelFamily::Claude), 0);
    }

    #[test]
    fn single_character() {
        // 1 char / 4.0 chars_per_token = 0.25, ceil = 1
        assert_eq!(estimate_tokens_default("x"), 1);
    }

    #[test]
    fn known_string_gpt4() {
        // 20 chars / 4.0 = 5.0 tokens
        let text = "abcdefghijklmnopqrst"; // 20 chars
        assert_eq!(estimate_tokens(text, ModelFamily::Gpt4), 5);
    }

    #[test]
    fn known_string_claude() {
        // 21 chars / 3.5 = 6.0 tokens
        let text = "abcdefghijklmnopqrstu"; // 21 chars
        assert_eq!(estimate_tokens(text, ModelFamily::Claude), 6);
    }

    #[test]
    fn known_string_rounds_up() {
        // 10 chars / 4.0 = 2.5, ceil = 3
        let text = "0123456789"; // 10 chars
        assert_eq!(estimate_tokens(text, ModelFamily::Gpt4), 3);
    }

    #[test]
    fn unknown_model_uses_conservative_ratio() {
        // Same as GPT-4: 4 chars/token
        let text = "abcdefgh"; // 8 chars / 4.0 = 2
        assert_eq!(estimate_tokens(text, ModelFamily::Unknown), 2);
    }

    #[test]
    fn parse_model_recognises_variants() {
        assert_eq!(parse_model("gpt-4"), ModelFamily::Gpt4);
        assert_eq!(parse_model("gpt-4-turbo"), ModelFamily::Gpt4);
        assert_eq!(parse_model("gpt4o"), ModelFamily::Gpt4);
        assert_eq!(parse_model("gpt-3.5-turbo"), ModelFamily::Gpt35);
        assert_eq!(parse_model("gpt3.5"), ModelFamily::Gpt35);
        assert_eq!(parse_model("claude-3-opus"), ModelFamily::Claude);
        assert_eq!(parse_model("claude-sonnet"), ModelFamily::Claude);
        assert_eq!(parse_model("llama-70b"), ModelFamily::Unknown);
        assert_eq!(parse_model("mistral"), ModelFamily::Unknown);
    }

    #[test]
    fn estimator_model_name() {
        assert_eq!(default_estimator().model_name(), "gpt-4");
        assert_eq!(estimator_for_model("claude-3-opus").model_name(), "claude");
    }

    #[test]
    fn trait_object_works() {
        let estimator: Box<dyn TokenEstimator> = Box::new(default_estimator());
        assert_eq!(estimator.estimate("abcd"), 1);
        assert_eq!(estimator.model_name(), "gpt-4");
    }
}
