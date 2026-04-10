//! LLM-driven memory consolidation.
//!
//! After each conversation turn, extracts structured information:
//! - `history_entry`: A timestamped summary for the daily conversation log.
//! - `memory_update`: New facts, preferences, or decisions worth remembering
//!   long-term (or `null` if nothing new was learned).
//! - `knowledge_items`: Domain knowledge (patterns, decisions, lessons) to
//!   capture in the knowledge graph (when `auto_capture` is enabled).
//!
//! This two-phase approach replaces the naive raw-message auto-save with
//! semantic extraction, similar to Nanobot's `save_memory` tool call pattern.

use crate::memory::conflict;
use crate::memory::importance;
use crate::memory::knowledge_graph::{KnowledgeGraph, NodeType};
use crate::memory::traits::{Memory, MemoryCategory};
use crate::providers::traits::Provider;
use std::path::Path;


/// Output of consolidation extraction.
#[derive(Debug, serde::Deserialize)]
pub struct ConsolidationResult {
    /// Brief timestamped summary for the conversation history log.
    pub history_entry: String,
    /// New facts/preferences/decisions to store long-term, or None.
    pub memory_update: Option<String>,
    /// Atomic facts extracted from the turn (when consolidation_extract_facts is enabled).
    #[serde(default)]
    pub facts: Vec<String>,
    /// Observed trend or pattern (when consolidation_extract_facts is enabled).
    #[serde(default)]
    pub trend: Option<String>,
    /// Domain knowledge items extracted when auto_capture is enabled.
    #[serde(default)]
    pub knowledge_items: Vec<KnowledgeItem>,
}

/// A knowledge item extracted from the conversation turn.
#[derive(Debug, serde::Deserialize)]
pub struct KnowledgeItem {
    /// One of: pattern, decision, lesson, technology
    pub node_type: String,
    /// Concise title (under 80 chars)
    pub title: String,
    /// Detailed explanation (2-4 sentences)
    pub content: String,
    /// Relevant keywords
    #[serde(default)]
    pub tags: Vec<String>,
    /// 0.0-1.0 confidence score
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_confidence() -> f64 {
    0.5
}

/// Minimum confidence threshold for auto-captured knowledge.
const MIN_KNOWLEDGE_CONFIDENCE: f64 = 0.6;

const CONSOLIDATION_SYSTEM_PROMPT: &str = r#"You are a memory consolidation engine. Given a conversation turn, extract:
1. "history_entry": A brief summary of what happened in this turn (1-2 sentences). Include the key topic or action.
2. "memory_update": Any NEW facts, preferences, decisions, or commitments worth remembering long-term. Return null if nothing new was learned.

Respond ONLY with valid JSON: {"history_entry": "...", "memory_update": "..." or null}
Do not include any text outside the JSON object."#;

/// Combined prompt that extracts both memory and knowledge in one LLM call.
const CONSOLIDATION_WITH_KNOWLEDGE_PROMPT: &str = r#"You are a memory AND knowledge extraction engine. Given a conversation turn, extract TWO categories of information:

## Memory (personal)
1. "history_entry": A brief summary of what happened in this turn (1-2 sentences).
2. "memory_update": Any NEW facts, preferences, decisions, or commitments about the USER worth remembering long-term. Return null if nothing new was learned.

## Knowledge (domain)
3. "knowledge_items": An array of 0-3 domain knowledge items worth capturing for reuse. These are NOT about the user — they are about:
   - Technical patterns and architecture decisions
   - Problem-solving approaches and methodologies
   - Lessons learned from debugging or design choices
   - Technology evaluations and trade-offs

For each knowledge item, provide:
- "node_type": one of "pattern", "decision", "lesson", "technology"
- "title": concise title (under 80 chars)
- "content": detailed explanation (2-4 sentences)
- "tags": relevant keywords (1-5 tags)
- "confidence": 0.0-1.0 (how valuable and reusable this knowledge is)

Respond ONLY with valid JSON:
{"history_entry": "...", "memory_update": "..." or null, "knowledge_items": [...] or []}
Do not include any text outside the JSON object.
Respond in the same language as the conversation."#;

/// Run two-phase LLM-driven consolidation on a conversation turn.
///
/// Phase 1: Write a history entry to the Daily memory category.
/// Phase 2: Write a memory update to the Core category (if the LLM identified new facts).
///
/// This function is designed to be called fire-and-forget via `tokio::spawn`.
/// Strip channel media markers (e.g. `[IMAGE:/local/path]`, `[DOCUMENT:...]`)
/// that contain local filesystem paths.  These must never be forwarded to
/// upstream provider APIs — they would leak local paths and cause API errors.
fn strip_media_markers(text: &str) -> String {
    // Matches [IMAGE:...], [DOCUMENT:...], [FILE:...], [VIDEO:...], [VOICE:...], [AUDIO:...]
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"\[(?:IMAGE|DOCUMENT|FILE|VIDEO|VOICE|AUDIO):[^\]]*\]").unwrap()
    });
    RE.replace_all(text, "[media attachment]").into_owned()
}

pub async fn consolidate_turn(
    provider: &dyn Provider,
    model: &str,
    memory: &dyn Memory,
    workspace_dir: Option<&Path>,
    user_message: &str,
    assistant_response: &str,
) -> anyhow::Result<()> {
    let turn_text = format!(
        "User: {}\nAssistant: {}",
        strip_media_markers(user_message),
        strip_media_markers(assistant_response),
    );

    // Truncate very long turns to avoid wasting tokens on consolidation.
    // Use char-boundary-safe slicing to prevent panic on multi-byte UTF-8 (e.g. CJK text).
    let truncated = if turn_text.len() > 4000 {
        let end = turn_text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= 4000)
            .last()
            .unwrap_or(0);
        format!("{}…", &turn_text[..end])
    } else {
        turn_text.clone()
    };

    let raw = provider
        .chat_with_system(Some(CONSOLIDATION_SYSTEM_PROMPT), &truncated, model, 0.1)
        .await?;

    let result: ConsolidationResult = parse_consolidation_response(&raw, &turn_text);

    // Phase 1: Write history entry to Daily category.
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let history_key = format!("daily_{date}_{}", uuid::Uuid::new_v4());
    memory
        .store(
            &history_key,
            &result.history_entry,
            MemoryCategory::Daily,
            None,
        )
        .await?;

    // Phase 2: Write memory update to Core category (if present).
    if let Some(ref update) = result.memory_update {
        if !update.trim().is_empty() {
            let mem_key = format!("core_{}", uuid::Uuid::new_v4());

            // Compute importance score heuristically.
            let imp = importance::compute_importance(update, &MemoryCategory::Core);

            // Check for conflicts with existing Core memories.
            if let Err(e) = conflict::check_and_resolve_conflicts(
                memory,
                &mem_key,
                update,
                &MemoryCategory::Core,
                0.85,
            )
            .await
            {
                tracing::debug!("conflict check skipped: {e}");
            }

            // Store with importance metadata.
            memory
                .store_with_metadata(
                    &mem_key,
                    update,
                    MemoryCategory::Core,
                    None,
                    None,
                    Some(imp),
                )
                .await?;

            if let Some(wd) = workspace_dir {
                distill_core_memory_to_markdown(provider, model, wd, update).await;
            }
        }
    }

    Ok(())
}

/// Run consolidated memory + knowledge extraction in a single LLM call.
///
/// When `auto_capture` is enabled, this replaces the basic `consolidate_turn`
/// with a combined prompt that extracts both memory updates AND domain knowledge
/// in one round-trip, saving tokens and latency.
///
/// Knowledge items meeting the confidence threshold are stored in the graph.
pub async fn consolidate_turn_with_knowledge(
    provider: &dyn Provider,
    model: &str,
    memory: &dyn Memory,
    graph: &KnowledgeGraph,
    workspace_dir: Option<&Path>,
    user_message: &str,
    assistant_response: &str,
    source_agent: Option<&str>,
) -> anyhow::Result<usize> {
    let turn_text = format!(
        "User: {}\nAssistant: {}",
        strip_media_markers(user_message),
        strip_media_markers(assistant_response),
    );

    let truncated = if turn_text.len() > 4000 {
        let end = turn_text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= 4000)
            .last()
            .unwrap_or(0);
        format!("{}…", &turn_text[..end])
    } else {
        turn_text.clone()
    };

    let raw = provider
        .chat_with_system(
            Some(CONSOLIDATION_WITH_KNOWLEDGE_PROMPT),
            &truncated,
            model,
            0.1,
        )
        .await?;

    let result: ConsolidationResult = parse_consolidation_response(&raw, &turn_text);

    // Phase 1+2: Standard memory consolidation (history + core updates)
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let history_key = format!("daily_{date}_{}", uuid::Uuid::new_v4());
    memory
        .store(
            &history_key,
            &result.history_entry,
            MemoryCategory::Daily,
            None,
        )
        .await?;

    if let Some(ref update) = result.memory_update {
        if !update.trim().is_empty() {
            let mem_key = format!("core_{}", uuid::Uuid::new_v4());
            let imp = importance::compute_importance(update, &MemoryCategory::Core);
            if let Err(e) = conflict::check_and_resolve_conflicts(
                memory,
                &mem_key,
                update,
                &MemoryCategory::Core,
                0.85,
            )
            .await
            {
                tracing::debug!("conflict check skipped: {e}");
            }
            memory
                .store_with_metadata(
                    &mem_key,
                    update,
                    MemoryCategory::Core,
                    None,
                    None,
                    Some(imp),
                )
                .await?;

            if let Some(wd) = workspace_dir {
                distill_core_memory_to_markdown(provider, model, wd, update).await;
            }
        }
    }

    // Phase 3: Knowledge capture — store extracted items in the graph
    let mut knowledge_count = 0;
    for item in &result.knowledge_items {
        if item.confidence < MIN_KNOWLEDGE_CONFIDENCE {
            continue;
        }

        let node_type = NodeType::parse(&item.node_type).unwrap_or(NodeType::Lesson);

        // Dedup: check if a similar title already exists
        if let Ok(existing) = graph.query_by_similarity(&item.title, 1) {
            if let Some(first) = existing.first() {
                let a = first.node.title.to_lowercase();
                let b = item.title.to_lowercase();
                if a == b || a.contains(&b) || b.contains(&a) {
                    tracing::debug!(
                        existing = %first.node.title,
                        new = %item.title,
                        "Knowledge capture: skipping duplicate title"
                    );
                    continue;
                }
            }
        }

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
                knowledge_count += 1;
            }
            Err(e) => {
                tracing::debug!("Failed to store knowledge node: {e}");
            }
        }
    }

    Ok(knowledge_count)
}

/// Parse the LLM's consolidation response, with fallback for malformed JSON.
fn parse_consolidation_response(raw: &str, fallback_text: &str) -> ConsolidationResult {
    // Try to extract JSON from the response (LLM may wrap in markdown code blocks).
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str(cleaned).unwrap_or_else(|_| {
        // Fallback: use truncated turn text as history entry.
        // Use char-boundary-safe slicing to prevent panic on multi-byte UTF-8.
        let summary = if fallback_text.len() > 200 {
            let end = fallback_text
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 200)
                .last()
                .unwrap_or(0);
            format!("{}…", &fallback_text[..end])
        } else {
            fallback_text.to_string()
        };
        ConsolidationResult {
            history_entry: summary,
            memory_update: None,
            facts: Vec::new(),
            trend: None,
            knowledge_items: Vec::new(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json_response() {
        let raw = r#"{"history_entry": "User asked about Rust.", "memory_update": "User prefers Rust over Go."}"#;
        let result = parse_consolidation_response(raw, "fallback");
        assert_eq!(result.history_entry, "User asked about Rust.");
        assert_eq!(
            result.memory_update.as_deref(),
            Some("User prefers Rust over Go.")
        );
    }

    #[test]
    fn parse_json_with_null_memory() {
        let raw = r#"{"history_entry": "Routine greeting.", "memory_update": null}"#;
        let result = parse_consolidation_response(raw, "fallback");
        assert_eq!(result.history_entry, "Routine greeting.");
        assert!(result.memory_update.is_none());
    }

    #[test]
    fn parse_json_wrapped_in_code_block() {
        let raw =
            "```json\n{\"history_entry\": \"Discussed deployment.\", \"memory_update\": null}\n```";
        let result = parse_consolidation_response(raw, "fallback");
        assert_eq!(result.history_entry, "Discussed deployment.");
    }

    #[test]
    fn fallback_on_malformed_response() {
        let raw = "I'm sorry, I can't do that.";
        let result = parse_consolidation_response(raw, "User: hello\nAssistant: hi");
        assert_eq!(result.history_entry, "User: hello\nAssistant: hi");
        assert!(result.memory_update.is_none());
    }

    #[test]
    fn fallback_truncates_long_text() {
        let long_text = "x".repeat(500);
        let result = parse_consolidation_response("invalid", &long_text);
        // 200 bytes + "…" (3 bytes in UTF-8) = 203
        assert!(result.history_entry.len() <= 203);
    }

    #[test]
    fn fallback_truncates_cjk_text_without_panic() {
        let cjk_text = "二手书项目".repeat(50); // 250 chars = 750 bytes
        let result = parse_consolidation_response("invalid", &cjk_text);
        assert!(
            result
                .history_entry
                .is_char_boundary(result.history_entry.len())
        );
        assert!(result.history_entry.ends_with('…'));
    }
}

/// Asynchronously rewrite MEMORY.md using the LLM provider based on existing MEMORY.md and the new core update
async fn distill_core_memory_to_markdown(
    provider: &dyn Provider,
    model: &str,
    workspace_dir: &std::path::Path,
    new_memory_update: &str,
) {
    let memory_md_path = workspace_dir.join("MEMORY.md");
    let current_content = if memory_md_path.exists() {
        tokio::fs::read_to_string(&memory_md_path).await.unwrap_or_default()
    } else {
        String::new()
    };
    
    let prompt = format!(
        "You are tasked with dynamically updating a long-term memory file (MEMORY.md). \n\
         The existing file content is provided below. You have just learned a NEW core memory fact. \n\
         Please seamlessly integrate the new fact into the existing file. \n\
         If there is a relevant section (e.g. user preferences, milestones), add it there. \n\
         If not, create an appropriate section. \n\
         Preserve the original formatting, tone, and any structural directives (like headings, comments). \n\n\
         NEW FACT TO INTEGRATE:\n\
         {new_memory_update}\n\n\
         CURRENT MEMORY.md:\n\
         {current_content}"
    );

    let system_prompt = "You are a memory distillation engine. Output ONLY the raw markdown content for the updated MEMORY.md. Do not wrap it in markdown code blocks unless the file itself should have them, do not explain your changes.";

    match provider.chat_with_system(Some(system_prompt), &prompt, model, 0.1).await {
        Ok(new_content) => {
            let cleaned = new_content
                .trim()
                .strip_prefix("```markdown")
                .or_else(|| new_content.trim().strip_prefix("```"))
                .and_then(|s| s.strip_suffix("```"))
                .unwrap_or(&new_content)
                .trim();
                
            if let Err(e) = tokio::fs::write(&memory_md_path, cleaned).await {
                tracing::warn!("Failed to overwrite MEMORY.md during distillation: {}", e);
            } else {
                tracing::info!("Successfully distilled and updated MEMORY.md with new core fact");
            }
        }
        Err(e) => {
            tracing::warn!("LLM provider failed during MEMORY.md distillation: {}", e);
        }
    }
}
