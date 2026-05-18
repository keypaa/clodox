use std::collections::HashMap;

use cc_core::messages::Message;
use serde::{Deserialize, Serialize};

/// A suggested prompt with context about why it's suggested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSuggestion {
    pub text: String,
    pub category: SuggestionCategory,
    pub confidence: f64,
    pub context: String,
}

/// Category of prompt suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuggestionCategory {
    /// Follow up on the last assistant response.
    FollowUp,
    /// Ask for clarification.
    Clarification,
    /// Suggest a next action.
    NextAction,
    /// Explore related topic.
    Explore,
    /// General helpful prompt.
    General,
}

impl std::fmt::Display for SuggestionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuggestionCategory::FollowUp => write!(f, "Follow-up"),
            SuggestionCategory::Clarification => write!(f, "Clarification"),
            SuggestionCategory::NextAction => write!(f, "Next Action"),
            SuggestionCategory::Explore => write!(f, "Explore"),
            SuggestionCategory::General => write!(f, "General"),
        }
    }
}

/// Prompt suggestion service — context-aware prompt suggestions.
pub struct PromptSuggestionService {
    /// Pre-defined suggestion templates keyed by trigger keywords.
    templates: HashMap<String, Vec<(String, SuggestionCategory)>>,
}

impl PromptSuggestionService {
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        // Code-related triggers
        templates.insert(
            "error".to_string(),
            vec![
                ("Can you explain what's causing this error?".to_string(), SuggestionCategory::Clarification),
                ("How can I fix this?".to_string(), SuggestionCategory::NextAction),
                ("Are there any similar issues I should check for?".to_string(), SuggestionCategory::FollowUp),
            ],
        );

        templates.insert(
            "test".to_string(),
            vec![
                ("Can you write tests for this?".to_string(), SuggestionCategory::NextAction),
                ("What edge cases should we cover?".to_string(), SuggestionCategory::Explore),
                ("Run the tests and show me the results.".to_string(), SuggestionCategory::NextAction),
            ],
        );

        templates.insert(
            "refactor".to_string(),
            vec![
                ("What improvements can you suggest?".to_string(), SuggestionCategory::Explore),
                ("Can you refactor this to be more readable?".to_string(), SuggestionCategory::NextAction),
                ("Are there any design patterns that would fit here?".to_string(), SuggestionCategory::Explore),
            ],
        );

        templates.insert(
            "bug".to_string(),
            vec![
                ("Can you help me find the bug?".to_string(), SuggestionCategory::NextAction),
                ("What's the root cause?".to_string(), SuggestionCategory::Clarification),
                ("How can we prevent this in the future?".to_string(), SuggestionCategory::FollowUp),
            ],
        );

        templates.insert(
            "performance".to_string(),
            vec![
                ("Can you optimize this?".to_string(), SuggestionCategory::NextAction),
                ("Where are the bottlenecks?".to_string(), SuggestionCategory::Explore),
                ("What's the time complexity?".to_string(), SuggestionCategory::Clarification),
            ],
        );

        templates.insert(
            "document".to_string(),
            vec![
                ("Can you add documentation for this?".to_string(), SuggestionCategory::NextAction),
                ("What should the docstring include?".to_string(), SuggestionCategory::Clarification),
                ("Generate a README for this module.".to_string(), SuggestionCategory::NextAction),
            ],
        );

        templates.insert(
            "implement".to_string(),
            vec![
                ("What's the best approach for this?".to_string(), SuggestionCategory::Explore),
                ("Can you show me an example?".to_string(), SuggestionCategory::Clarification),
                ("Let's break this down into steps.".to_string(), SuggestionCategory::NextAction),
            ],
        );

        Self { templates }
    }

    /// Generate suggestions based on the current conversation context.
    pub fn generate_suggestions(&self, messages: &[Message]) -> Vec<PromptSuggestion> {
        if messages.is_empty() {
            return self.get_general_suggestions();
        }

        let mut suggestions = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Analyze recent messages for triggers
        let recent_messages: Vec<&Message> = messages.iter().rev().take(5).collect();

        for msg in recent_messages {
            let text = match msg {
                Message::User(u) => {
                    u.content.iter().filter_map(|b| {
                        if let cc_core::messages::ContentBlockParam::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>().join(" ")
                }
                Message::Assistant(a) => {
                    a.content.iter().filter_map(|b| {
                        if let cc_core::messages::ContentBlockParam::Text { text } = b {
                            Some(text.clone())
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>().join(" ")
                }
                _ => String::new(),
            };

            let lower = text.to_lowercase();

            // Check each template trigger
            for (trigger, template_suggestions) in &self.templates {
                if lower.contains(trigger.as_str()) {
                    for (template_text, category) in template_suggestions {
                        if !seen.contains(template_text) {
                            seen.insert(template_text.clone());
                            suggestions.push(PromptSuggestion {
                                text: template_text.clone(),
                                category: *category,
                                confidence: self.calculate_confidence(trigger, &lower),
                                context: format!("Based on mention of '{trigger}'"),
                            });
                        }
                    }
                }
            }
        }

        // If no specific suggestions, return general ones
        if suggestions.is_empty() {
            suggestions = self.get_general_suggestions();
        }

        // Sort by confidence descending
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        // Return top 5
        suggestions.into_iter().take(5).collect()
    }

    /// Get general suggestions when no context is available.
    pub fn get_general_suggestions(&self) -> Vec<PromptSuggestion> {
        vec![
            PromptSuggestion {
                text: "What can you help me with?".to_string(),
                category: SuggestionCategory::General,
                confidence: 0.5,
                context: "General".to_string(),
            },
            PromptSuggestion {
                text: "Show me the project structure.".to_string(),
                category: SuggestionCategory::Explore,
                confidence: 0.4,
                context: "General".to_string(),
            },
            PromptSuggestion {
                text: "What files have been changed recently?".to_string(),
                category: SuggestionCategory::NextAction,
                confidence: 0.3,
                context: "General".to_string(),
            },
        ]
    }

    /// Generate suggestions based on a specific keyword.
    pub fn get_suggestions_for_keyword(&self, keyword: &str) -> Vec<PromptSuggestion> {
        let lower = keyword.to_lowercase();

        if let Some(templates) = self.templates.get(&lower) {
            templates
                .iter()
                .map(|(text, category)| PromptSuggestion {
                    text: text.clone(),
                    category: *category,
                    confidence: 0.7,
                    context: format!("Keyword: {keyword}"),
                })
                .collect()
        } else {
            // Try partial matching
            let mut results = Vec::new();
            for (trigger, templates) in &self.templates {
                if lower.contains(trigger.as_str()) || trigger.contains(&lower) {
                    for (text, category) in templates {
                        results.push(PromptSuggestion {
                            text: text.clone(),
                            category: *category,
                            confidence: 0.5,
                            context: format!("Keyword: {keyword}"),
                        });
                    }
                }
            }
            results
        }
    }

    /// Calculate confidence score for a suggestion based on trigger match strength.
    fn calculate_confidence(&self, trigger: &str, text: &str) -> f64 {
        let trigger_count = text.matches(trigger).count();
        let base = 0.6;
        let bonus = (trigger_count as f64 * 0.1).min(0.3);
        base + bonus
    }

    /// Get all available suggestion categories.
    pub fn get_available_categories(&self) -> Vec<SuggestionCategory> {
        vec![
            SuggestionCategory::FollowUp,
            SuggestionCategory::Clarification,
            SuggestionCategory::NextAction,
            SuggestionCategory::Explore,
            SuggestionCategory::General,
        ]
    }

    /// Get suggestion count by category.
    pub fn count_by_category(&self, suggestions: &[PromptSuggestion]) -> HashMap<SuggestionCategory, usize> {
        let mut counts = HashMap::new();
        for suggestion in suggestions {
            *counts.entry(suggestion.category).or_insert(0) += 1;
        }
        counts
    }
}

impl Default for PromptSuggestionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_category_display() {
        assert_eq!(SuggestionCategory::FollowUp.to_string(), "Follow-up");
        assert_eq!(SuggestionCategory::Clarification.to_string(), "Clarification");
        assert_eq!(SuggestionCategory::NextAction.to_string(), "Next Action");
        assert_eq!(SuggestionCategory::Explore.to_string(), "Explore");
        assert_eq!(SuggestionCategory::General.to_string(), "General");
    }

    #[test]
    fn test_general_suggestions() {
        let service = PromptSuggestionService::new();
        let suggestions = service.get_general_suggestions();
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.category == SuggestionCategory::General));
    }

    #[test]
    fn test_keyword_suggestions() {
        let service = PromptSuggestionService::new();
        let suggestions = service.get_suggestions_for_keyword("error");
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().all(|s| s.context.contains("error")));
    }

    #[test]
    fn test_keyword_partial_match() {
        let service = PromptSuggestionService::new();
        let suggestions = service.get_suggestions_for_keyword("testing");
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_suggestions_sorted_by_confidence() {
        let service = PromptSuggestionService::new();
        let messages: Vec<Message> = vec![];
        let suggestions = service.generate_suggestions(&messages);
        assert!(!suggestions.is_empty());
        // Check they are sorted by confidence descending
        for i in 1..suggestions.len() {
            assert!(suggestions[i - 1].confidence >= suggestions[i].confidence);
        }
    }

    #[test]
    fn test_available_categories() {
        let service = PromptSuggestionService::new();
        let categories = service.get_available_categories();
        assert_eq!(categories.len(), 5);
    }

    #[test]
    fn test_count_by_category() {
        let service = PromptSuggestionService::new();
        let suggestions = vec![
            PromptSuggestion {
                text: "Test 1".to_string(),
                category: SuggestionCategory::FollowUp,
                confidence: 0.5,
                context: "test".to_string(),
            },
            PromptSuggestion {
                text: "Test 2".to_string(),
                category: SuggestionCategory::FollowUp,
                confidence: 0.5,
                context: "test".to_string(),
            },
            PromptSuggestion {
                text: "Test 3".to_string(),
                category: SuggestionCategory::Explore,
                confidence: 0.5,
                context: "test".to_string(),
            },
        ];
        let counts = service.count_by_category(&suggestions);
        assert_eq!(counts[&SuggestionCategory::FollowUp], 2);
        assert_eq!(counts[&SuggestionCategory::Explore], 1);
    }

    #[test]
    fn test_empty_messages_returns_general() {
        let service = PromptSuggestionService::new();
        let suggestions = service.generate_suggestions(&[]);
        assert!(!suggestions.is_empty());
    }
}
