//! LLM-driven knowledge capture from conversations.
//!
//! Runs alongside memory consolidation after each conversation turn.
//! Extracts domain knowledge (patterns, decisions, lessons) rather than
//! personal facts about the user.
//!
//! Called when `[knowledge] auto_capture = true` and knowledge_graph is
//! available in the tenant context.

use crate::memory::knowledge_graph::{KnowledgeGraph, NodeType};
use crate::providers::traits::Provider;

/// Output of knowledge capture extraction.
#[derive(Debug, serde::Deserialize)]
pub struct KnowledgeCaptureResult {
    /// Extracted knowledge items (0-3 per turn).
    #[serde(default)]
    pub items: Vec<CapturedKnowledgeItem>,
}

/// A single knowledge item extracted from a conversation turn.
#[derive(Debug, serde::Deserialize)]
pub struct CapturedKnowledgeItem {
    /// One of: "pattern", "decision", "lesson", "technology"
    pub node_type: String,
    /// Concise title (under 80 chars).
    pub title: String,
    /// Detailed explanation.
    pub content: String,
    /// Relevant keywords.
    #[serde(default)]
    pub tags: Vec<String>,
    /// 0.0-1.0 — how valuable this knowledge is.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}

/// Minimum confidence threshold for auto-captured knowledge.
const MIN_CONFIDENCE: f64 = 0.6;

/// Maximum length of turn text sent to the LLM for knowledge extraction.
const MAX_TURN_CHARS: usize = 3000;

const KNOWLEDGE_CAPTURE_SYSTEM_PROMPT: &str = r#"You are a knowledge extraction engine. Given a conversation turn, identify any DOMAIN KNOWLEDGE worth capturing for future reuse.

IMPORTANT: This is NOT about the user's personal facts, preferences, or identity. This is about:
- Technical patterns and architecture decisions
- Problem-solving approaches and methodologies
- Lessons learned from debugging or design choices
- Technology evaluations and trade-offs
- Best practices and anti-patterns

Extract 0-3 items. For each item, provide:
- "node_type": one of "pattern", "decision", "lesson", "technology"
- "title": concise title (under 80 chars)
- "content": detailed explanation (2-4 sentences)
- "tags": relevant keywords (1-5 tags)
- "confidence": 0.0-1.0 (how valuable and reusable this knowledge is)

Return ONLY valid JSON: {"items": [...]} or {"items": []} if nothing worth capturing.
Do not include any text outside the JSON object.
Respond in the same language as the conversation."#;

/// Extract domain knowledge from a conversation turn and store in the graph.
///
/// Returns the number of newly added knowledge nodes.
///
/// This function is designed to be called fire-and-forget via `tokio::spawn`,
/// alongside `consolidation::consolidate_turn`.
pub async fn capture_knowledge_from_turn(
    provider: &dyn Provider,
    model: &str,
    graph: &KnowledgeGraph,
    user_message: &str,
    assistant_response: &str,
    source_agent: Option<&str>,
) -> anyhow::Result<usize> {
    let turn_text = format!(
        "User: {}\nAssistant: {}",
        strip_media_markers(user_message),
        strip_media_markers(assistant_response),
    );

    // Truncate very long turns to avoid wasting tokens.
    let truncated = safe_truncate(&turn_text, MAX_TURN_CHARS);

    let raw = provider
        .chat_with_system(Some(KNOWLEDGE_CAPTURE_SYSTEM_PROMPT), &truncated, model, 0.1)
        .await?;

    let result = parse_capture_response(&raw);

    let mut count = 0;
    for item in result.items {
        // Skip low-confidence items
        if item.confidence < MIN_CONFIDENCE {
            continue;
        }

        // Parse node_type, defaulting to Lesson
        let node_type = NodeType::parse(&item.node_type).unwrap_or(NodeType::Lesson);

        // Check for duplicate titles via FTS5 search to avoid storing
        // "Circuit Breaker Pattern" twice.
        if let Ok(existing) = graph.query_by_similarity(&item.title, 1) {
            if let Some(first) = existing.first() {
                if titles_are_similar(&first.node.title, &item.title) {
                    tracing::debug!(
                        existing_title = %first.node.title,
                        new_title = %item.title,
                        "Knowledge capture: skipping duplicate title"
                    );
                    continue;
                }
            }
        }

        // Store the knowledge node
        match graph.add_node(
            node_type,
            &item.title,
            &item.content,
            &item.tags,
            source_agent,
        ) {
            Ok(id) => {
                tracing::info!(
                    node_id = %id,
                    title = %item.title,
                    confidence = item.confidence,
                    "Auto-captured knowledge node"
                );
                count += 1;
            }
            Err(e) => {
                tracing::debug!("Failed to store knowledge node: {e}");
            }
        }
    }

    Ok(count)
}

/// Check if two titles are similar enough to be considered duplicates.
fn titles_are_similar(a: &str, b: &str) -> bool {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    // Exact match
    if a_lower == b_lower {
        return true;
    }
    // One contains the other
    if a_lower.contains(&b_lower) || b_lower.contains(&a_lower) {
        return true;
    }
    false
}

/// Parse the LLM's knowledge capture response, with fallback for malformed JSON.
fn parse_capture_response(raw: &str) -> KnowledgeCaptureResult {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(cleaned).unwrap_or_else(|_| KnowledgeCaptureResult {
        items: Vec::new(),
    })
}

/// Strip channel media markers that contain local filesystem paths.
fn strip_media_markers(text: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\[(?:IMAGE|DOCUMENT|FILE|VIDEO|VOICE|AUDIO):[^\]]*\]").unwrap()
    });
    RE.replace_all(text, "[media attachment]").into_owned()
}

/// Safe truncation that respects UTF-8 character boundaries.
fn safe_truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_chars)
        .last()
        .unwrap_or(0);
    format!("{}…", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json_response() {
        let raw = r#"{"items": [{"node_type": "pattern", "title": "Circuit Breaker", "content": "Use circuit breaker for external API calls.", "tags": ["resilience"], "confidence": 0.8}]}"#;
        let result = parse_capture_response(raw);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Circuit Breaker");
        assert_eq!(result.items[0].confidence, 0.8);
    }

    #[test]
    fn parse_empty_items() {
        let raw = r#"{"items": []}"#;
        let result = parse_capture_response(raw);
        assert!(result.items.is_empty());
    }

    #[test]
    fn parse_malformed_response() {
        let raw = "I'm sorry, I can't extract knowledge from this.";
        let result = parse_capture_response(raw);
        assert!(result.items.is_empty());
    }

    #[test]
    fn parse_json_in_code_block() {
        let raw = "```json\n{\"items\": [{\"node_type\": \"lesson\", \"title\": \"Test\", \"content\": \"Details\", \"tags\": [], \"confidence\": 0.9}]}\n```";
        let result = parse_capture_response(raw);
        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn titles_similarity_check() {
        assert!(titles_are_similar("Circuit Breaker", "circuit breaker"));
        assert!(titles_are_similar(
            "Circuit Breaker Pattern",
            "Circuit Breaker"
        ));
        assert!(!titles_are_similar("Circuit Breaker", "Rate Limiter"));
    }

    #[test]
    fn safe_truncate_cjk() {
        let text = "你好世界".repeat(100);
        let truncated = safe_truncate(&text, 10);
        assert!(truncated.ends_with('…'));
        // Must not panic on CJK
    }

    #[test]
    fn strip_media_markers_works() {
        let text = "Hello [IMAGE:/path/to/image.jpg] world [DOCUMENT:file.pdf]";
        let stripped = strip_media_markers(text);
        assert_eq!(
            stripped,
            "Hello [media attachment] world [media attachment]"
        );
    }
}
