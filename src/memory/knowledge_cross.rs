//! Cross-workspace knowledge search for same-user agent isolation on cloud.
//!
//! On **desktop**, all agents share a single knowledge.db via `owner_dir`,
//! so cross-workspace search is inherently enabled — no extra logic needed.
//!
//! On **cloud**, each agent has its own knowledge.db under its workspace.
//! When `cross_workspace_search = true`, this module discovers and queries
//! all knowledge.db files under the user's agents directory, aggregating
//! results and deduplicating by title.

use crate::memory::knowledge_graph::{KnowledgeGraph, KnowledgeNode};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Aggregated search result from cross-workspace search.
#[derive(Debug)]
pub struct CrossSearchResult {
    /// The knowledge node.
    pub node: KnowledgeNode,
    /// FTS5 relevance score.
    pub score: f64,
    /// Name of the source agent.
    pub source_agent: String,
}

/// Cross-workspace knowledge index that discovers and queries multiple
/// knowledge.db files under a given agents directory.
pub struct CrossWorkspaceKnowledgeIndex {
    /// List of (agent_name, KnowledgeGraph) pairs.
    graphs: Vec<(String, Arc<KnowledgeGraph>)>,
}

impl CrossWorkspaceKnowledgeIndex {
    /// Discover and open all knowledge.db files under the given `agents_dir`.
    ///
    /// Scans `agents_dir/<agent_name>/knowledge/knowledge.db` for each
    /// subdirectory that contains a knowledge database.
    pub fn discover(agents_dir: &Path, max_nodes: usize) -> Self {
        let mut graphs = Vec::new();

        let entries = match std::fs::read_dir(agents_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(
                    dir = %agents_dir.display(),
                    "Cross-workspace knowledge: failed to read agents dir: {e}"
                );
                return Self { graphs };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let agent_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Look for knowledge.db in common locations
            let db_candidates = [
                path.join("knowledge/knowledge.db"),
                path.join("knowledge.db"),
            ];

            for db_path in &db_candidates {
                if db_path.exists() {
                    match KnowledgeGraph::new(db_path, max_nodes) {
                        Ok(g) => {
                            tracing::debug!(
                                agent = %agent_name,
                                db = %db_path.display(),
                                "Cross-workspace: discovered knowledge DB"
                            );
                            graphs.push((agent_name.clone(), Arc::new(g)));
                        }
                        Err(e) => {
                            tracing::debug!(
                                agent = %agent_name,
                                db = %db_path.display(),
                                "Cross-workspace: failed to open knowledge DB: {e}"
                            );
                        }
                    }
                    break; // Use first found
                }
            }
        }

        tracing::info!(
            count = graphs.len(),
            "Cross-workspace knowledge index: discovered {} agent knowledge DBs",
            graphs.len()
        );

        Self { graphs }
    }

    /// Create index from a pre-built list of (agent_name, graph) pairs.
    pub fn from_graphs(graphs: Vec<(String, Arc<KnowledgeGraph>)>) -> Self {
        Self { graphs }
    }

    /// Number of indexed agent databases.
    pub fn agent_count(&self) -> usize {
        self.graphs.len()
    }

    /// Search across all knowledge graphs, deduplicating by title.
    ///
    /// Results are sorted by score descending and capped at `limit`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<CrossSearchResult> {
        let mut all_results: Vec<CrossSearchResult> = Vec::new();
        let mut seen_titles: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (agent_name, graph) in &self.graphs {
            match graph.query_by_similarity(query, limit) {
                Ok(results) => {
                    for result in results {
                        let title_lower = result.node.title.to_lowercase();
                        if seen_titles.contains(&title_lower) {
                            continue; // Skip duplicates
                        }
                        seen_titles.insert(title_lower);
                        all_results.push(CrossSearchResult {
                            score: result.score,
                            node: result.node,
                            source_agent: agent_name.clone(),
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        agent = %agent_name,
                        "Cross-workspace search failed for agent: {e}"
                    );
                }
            }
        }

        // Sort by score descending
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all_results.truncate(limit);
        all_results
    }

    /// Get the directory paths being indexed (for diagnostics).
    pub fn indexed_agents(&self) -> Vec<&str> {
        self.graphs.iter().map(|(name, _)| name.as_str()).collect()
    }
}

/// Resolve the agents directory from a workspace path.
///
/// On cloud, the structure is typically:
/// `/opt/huanxing/agents/<agent_id>/` — each agent is a subdirectory.
/// So `agents_dir` = parent of `workspace_dir`.
pub fn agents_dir_from_workspace(workspace_dir: &Path) -> Option<PathBuf> {
    workspace_dir.parent().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_dir_yields_empty_index() {
        let tmp = TempDir::new().unwrap();
        let index = CrossWorkspaceKnowledgeIndex::discover(tmp.path(), 1000);
        assert_eq!(index.agent_count(), 0);
    }

    #[test]
    fn discovers_knowledge_dbs() {
        let tmp = TempDir::new().unwrap();

        // Create agent1/knowledge/knowledge.db
        let agent1_dir = tmp.path().join("agent1/knowledge");
        std::fs::create_dir_all(&agent1_dir).unwrap();
        let graph1 = KnowledgeGraph::new(&agent1_dir.join("knowledge.db"), 1000).unwrap();
        graph1
            .add_node(
                crate::memory::knowledge_graph::NodeType::Pattern,
                "Test Pattern",
                "A test pattern",
                &["test".to_string()],
                Some("agent1"),
            )
            .unwrap();

        // Create agent2/knowledge/knowledge.db
        let agent2_dir = tmp.path().join("agent2/knowledge");
        std::fs::create_dir_all(&agent2_dir).unwrap();
        let graph2 = KnowledgeGraph::new(&agent2_dir.join("knowledge.db"), 1000).unwrap();
        graph2
            .add_node(
                crate::memory::knowledge_graph::NodeType::Decision,
                "Test Decision",
                "A test decision",
                &["test".to_string()],
                Some("agent2"),
            )
            .unwrap();

        let index = CrossWorkspaceKnowledgeIndex::discover(tmp.path(), 1000);
        assert_eq!(index.agent_count(), 2);

        let results = index.search("test", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn deduplicates_by_title() {
        let tmp = TempDir::new().unwrap();

        // Create two agents with the same knowledge item
        for agent_name in &["agent1", "agent2"] {
            let dir = tmp.path().join(format!("{agent_name}/knowledge"));
            std::fs::create_dir_all(&dir).unwrap();
            let graph = KnowledgeGraph::new(&dir.join("knowledge.db"), 1000).unwrap();
            graph
                .add_node(
                    crate::memory::knowledge_graph::NodeType::Pattern,
                    "Same Title",
                    "Same content",
                    &["dup".to_string()],
                    Some(agent_name),
                )
                .unwrap();
        }

        let index = CrossWorkspaceKnowledgeIndex::discover(tmp.path(), 1000);
        let results = index.search("Same Title", 10);
        // Should only return 1 (deduplicated)
        assert_eq!(results.len(), 1);
    }
}
