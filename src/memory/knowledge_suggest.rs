//! Knowledge suggestion injection for conversation context.
//!
//! When `[knowledge] suggest_on_query = true`, queries the knowledge graph
//! with the user's message and injects matching items as a `[context]` block
//! before the user message.

use crate::memory::knowledge_graph::{KnowledgeGraph, KnowledgeNode};

/// Query the knowledge graph for items relevant to the user's message.
pub fn suggest_knowledge(graph: &KnowledgeGraph, query: &str, max_items: usize) -> Vec<KnowledgeNode> {
    if query.trim().is_empty() || max_items == 0 {
        return Vec::new();
    }

    graph
        .query_by_similarity(query, max_items)
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.node)
        .collect()
}

/// Format knowledge suggestions as a `[context]` block to prepend before user message.
///
/// The block uses a structured format that LLMs understand as background knowledge
/// without confusing it with the user's actual question.
pub fn format_knowledge_context(nodes: &[KnowledgeNode]) -> String {
    if nodes.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(512);
    out.push_str("[context: relevant knowledge from your knowledge graph]\n");
    for node in nodes {
        let truncated_content = safe_truncate(&node.content, 200);
        let tags_str = if node.tags.is_empty() {
            String::new()
        } else {
            format!(" ({})", node.tags.join(", "))
        };
        out.push_str(&format!(
            "- [{}] {}{}: {}\n",
            node.node_type.as_str(),
            node.title,
            tags_str,
            truncated_content,
        ));
    }
    out.push_str("[/context]\n\n");
    out
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
    use crate::memory::knowledge_graph::NodeType;
    use chrono::Utc;

    fn make_node(title: &str, content: &str, node_type: NodeType) -> KnowledgeNode {
        KnowledgeNode {
            id: uuid::Uuid::new_v4().to_string(),
            node_type,
            title: title.to_string(),
            content: content.to_string(),
            tags: vec!["test".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source_project: None,
        }
    }

    #[test]
    fn format_empty_nodes() {
        assert_eq!(format_knowledge_context(&[]), "");
    }

    #[test]
    fn format_single_node() {
        let nodes = vec![make_node(
            "Circuit Breaker",
            "Use circuit breaker for external API calls to prevent cascade failures.",
            NodeType::Pattern,
        )];
        let result = format_knowledge_context(&nodes);
        assert!(result.contains("[context:"));
        assert!(result.contains("[pattern] Circuit Breaker"));
        assert!(result.contains("(test)"));
        assert!(result.contains("[/context]"));
    }

    #[test]
    fn format_multiple_nodes() {
        let nodes = vec![
            make_node("CB", "Pattern 1", NodeType::Pattern),
            make_node("CQRS", "Pattern 2", NodeType::Decision),
        ];
        let result = format_knowledge_context(&nodes);
        assert!(result.contains("[pattern] CB"));
        assert!(result.contains("[decision] CQRS"));
    }

    #[test]
    fn safe_truncate_long_content() {
        let long = "x".repeat(500);
        let truncated = safe_truncate(&long, 200);
        assert!(truncated.len() <= 204); // 200 + "…" (3 bytes UTF-8)
    }
}
