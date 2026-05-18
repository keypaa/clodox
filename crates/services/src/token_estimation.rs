use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Per-model pricing information (per million tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: f64,
    pub cache_creation_per_million: f64,
}

impl ModelPricing {
    /// Calculate cost for a given token usage.
    pub fn calculate_cost(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.output_per_million;
        let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * self.cache_read_per_million;
        let cache_creation_cost =
            (cache_creation_tokens as f64 / 1_000_000.0) * self.cache_creation_per_million;
        input_cost + output_cost + cache_read_cost + cache_creation_cost
    }
}

/// Token estimation result.
#[derive(Debug, Clone, Default)]
pub struct TokenEstimate {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost: f64,
}

impl TokenEstimate {
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost = cost;
        self
    }
}

/// Token estimation service — cost prediction, context size calculation, per-model pricing.
pub struct TokenEstimationService {
    pricing: HashMap<String, ModelPricing>,
    /// Average characters per token (rough estimate).
    chars_per_token: f64,
}

impl TokenEstimationService {
    pub fn new() -> Self {
        let mut service = Self {
            pricing: HashMap::new(),
            chars_per_token: 4.0, // Rough average for English text
        };

        // Register default model pricing (Anthropic, as of 2025)
        service.register_model_pricing(
            "claude-sonnet-4-20250514",
            ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.30,
                cache_creation_per_million: 3.75,
            },
        );

        service.register_model_pricing(
            "claude-opus-4-20250514",
            ModelPricing {
                input_per_million: 15.0,
                output_per_million: 75.0,
                cache_read_per_million: 1.50,
                cache_creation_per_million: 18.75,
            },
        );

        service.register_model_pricing(
            "claude-sonnet-4-5-20250929",
            ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.30,
                cache_creation_per_million: 3.75,
            },
        );

        service.register_model_pricing(
            "claude-3-5-sonnet-20241022",
            ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.30,
                cache_creation_per_million: 3.75,
            },
        );

        service.register_model_pricing(
            "claude-3-5-haiku-20241022",
            ModelPricing {
                input_per_million: 0.80,
                output_per_million: 4.0,
                cache_read_per_million: 0.08,
                cache_creation_per_million: 1.0,
            },
        );

        service
    }

    /// Register pricing for a model.
    pub fn register_model_pricing(&mut self, model: &str, pricing: ModelPricing) {
        self.pricing.insert(model.to_string(), pricing);
    }

    /// Get pricing for a model.
    pub fn get_pricing(&self, model: &str) -> Option<&ModelPricing> {
        self.pricing.get(model)
    }

    /// Set the characters-per-token ratio for estimation.
    pub fn set_chars_per_token(&mut self, ratio: f64) {
        self.chars_per_token = ratio;
    }

    /// Estimate tokens from a text string.
    pub fn estimate_tokens_from_text(&self, text: &str) -> u64 {
        (text.len() as f64 / self.chars_per_token).ceil() as u64
    }

    /// Estimate tokens for a message list.
    pub fn estimate_tokens_for_messages(&self, messages: &[cc_core::messages::Message]) -> u64 {
        let mut total_chars = 0usize;

        for msg in messages {
            match msg {
                cc_core::messages::Message::User(u) => {
                    for block in &u.content {
                        if let cc_core::messages::ContentBlockParam::Text { text } = block {
                            total_chars += text.len();
                        }
                    }
                }
                cc_core::messages::Message::Assistant(a) => {
                    for block in &a.content {
                        if let cc_core::messages::ContentBlockParam::Text { text } = block {
                            total_chars += text.len();
                        }
                    }
                }
                _ => {}
            }
        }

        (total_chars as f64 / self.chars_per_token).ceil() as u64
    }

    /// Estimate cost for a given token usage and model.
    pub fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
    ) -> f64 {
        if let Some(pricing) = self.pricing.get(model) {
            pricing.calculate_cost(input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens)
        } else {
            // Default fallback pricing (Sonnet 4 rates)
            let default_pricing = ModelPricing {
                input_per_million: 3.0,
                output_per_million: 15.0,
                cache_read_per_million: 0.30,
                cache_creation_per_million: 3.75,
            };
            default_pricing.calculate_cost(input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens)
        }
    }

    /// Full estimation: tokens + cost for a message list.
    pub fn estimate(
        &self,
        model: &str,
        messages: &[cc_core::messages::Message],
        expected_output_tokens: u64,
    ) -> TokenEstimate {
        let input_tokens = self.estimate_tokens_for_messages(messages);
        let cache_read_tokens = (input_tokens as f64 * 0.3) as u64; // Rough estimate: 30% cache hit
        let cache_creation_tokens = (input_tokens as f64 * 0.7) as u64; // Rough estimate: 70% cache write

        let total_tokens = input_tokens + expected_output_tokens + cache_read_tokens + cache_creation_tokens;

        let estimated_cost = self.estimate_cost(
            model,
            input_tokens,
            expected_output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
        );

        TokenEstimate {
            input_tokens,
            output_tokens: expected_output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            total_tokens,
            estimated_cost,
        }
    }

    /// Estimate remaining context window.
    pub fn estimate_remaining(
        &self,
        model: &str,
        messages: &[cc_core::messages::Message],
    ) -> (u64, u64) {
        let max_tokens = self.max_context_tokens(model);
        let used = self.estimate_tokens_for_messages(messages);
        let remaining = max_tokens.saturating_sub(used);
        (remaining, max_tokens)
    }

    /// Get max context tokens for a model.
    pub fn max_context_tokens(&self, model: &str) -> u64 {
        match model {
            m if m.contains("claude-sonnet-4") || m.contains("claude-opus-4") || m.contains("claude-haiku-4") => {
                200_000
            }
            m if m.contains("claude-3-5") => 200_000,
            _ => 200_000, // Default
        }
    }

    /// Get all registered model names.
    pub fn registered_models(&self) -> Vec<String> {
        self.pricing.keys().cloned().collect()
    }

    /// Format a cost for display.
    pub fn format_cost(cost: f64) -> String {
        if cost < 0.01 {
            format!("${:.4}", cost)
        } else if cost < 1.0 {
            format!("${:.3}", cost)
        } else {
            format!("${:.2}", cost)
        }
    }
}

impl Default for TokenEstimationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_read_per_million: 0.30,
            cache_creation_per_million: 3.75,
        };
        let cost = pricing.calculate_cost(1_000_000, 500_000, 200_000, 800_000);
        assert!((cost - 13.56).abs() < 0.01);
    }

    #[test]
    fn test_token_estimate_with_cost() {
        let estimate = TokenEstimate::default().with_cost(1.5);
        assert!((estimate.estimated_cost - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_estimate_tokens_from_text() {
        let service = TokenEstimationService::new();
        let text = "Hello, world!";
        let tokens = service.estimate_tokens_from_text(text);
        assert!(tokens > 0);
        assert!(tokens <= 4);
    }

    #[test]
    fn test_format_cost_small() {
        assert_eq!(TokenEstimationService::format_cost(0.0012), "$0.0012");
    }

    #[test]
    fn test_format_cost_medium() {
        assert_eq!(TokenEstimationService::format_cost(0.123), "$0.123");
    }

    #[test]
    fn test_format_cost_large() {
        assert_eq!(TokenEstimationService::format_cost(12.34), "$12.34");
    }

    #[test]
    fn test_registered_models() {
        let service = TokenEstimationService::new();
        let models = service.registered_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("claude")));
    }

    #[test]
    fn test_max_context_tokens() {
        let service = TokenEstimationService::new();
        assert_eq!(service.max_context_tokens("claude-sonnet-4-20250514"), 200_000);
        assert_eq!(service.max_context_tokens("unknown-model"), 200_000);
    }

    #[test]
    fn test_estimate_cost_known_model() {
        let service = TokenEstimationService::new();
        let cost = service.estimate_cost("claude-sonnet-4-20250514", 1_000_000, 500_000, 0, 0);
        assert!((cost - 10.5).abs() < 0.01);
    }

    #[test]
    fn test_estimate_cost_unknown_model_fallback() {
        let service = TokenEstimationService::new();
        let cost = service.estimate_cost("unknown-model", 1_000_000, 500_000, 0, 0);
        assert!((cost - 10.5).abs() < 0.01);
    }

    #[test]
    fn test_estimate_remaining() {
        let service = TokenEstimationService::new();
        let messages: Vec<cc_core::messages::Message> = vec![];
        let (remaining, max) = service.estimate_remaining("claude-sonnet-4-20250514", &messages);
        assert_eq!(max, 200_000);
        assert_eq!(remaining, 200_000);
    }

    #[test]
    fn test_register_custom_pricing() {
        let mut service = TokenEstimationService::new();
        service.register_model_pricing(
            "custom-model",
            ModelPricing {
                input_per_million: 1.0,
                output_per_million: 5.0,
                cache_read_per_million: 0.10,
                cache_creation_per_million: 1.25,
            },
        );
        assert!(service.get_pricing("custom-model").is_some());
    }
}
