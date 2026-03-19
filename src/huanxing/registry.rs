//! Registry-based skill index loader.
//!
//! Reads `registry.json` from the hub repository and provides O(1) lookups
//! by skill ID, category-based filtering, and search functionality.
//!
//! This replaces directory traversal with index-driven loading for the
//! huanxing skill marketplace.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global registry index — loaded from `registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: String,
    pub generated_at: String,
    #[serde(default)]
    pub engine_version: String,
    pub stats: RegistryStats,
    pub skills: Vec<SkillEntry>,
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_skills: usize,
    pub total_templates: usize,
    #[serde(default)]
    pub categories: usize,
}

/// A skill entry in the registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub subcategory: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Relative path from hub root, e.g. "skills/search/tavily-search"
    pub path: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub risk_level: String,
    #[serde(default)]
    pub review_status: String,
    #[serde(default)]
    pub pricing_tier: String,
    #[serde(default)]
    pub requires_api_keys: bool,
    #[serde(default)]
    pub requires_cli: bool,
    #[serde(default)]
    pub requires_permissions: Vec<String>,
    #[serde(default)]
    pub has_scripts: bool,
    #[serde(default)]
    pub has_wasm: bool,
    #[serde(default)]
    pub file_count: usize,
    #[serde(default)]
    pub size_bytes: usize,
}

/// A template entry in the registry index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    pub path: String,
    #[serde(default)]
    pub pricing_tier: String,
}

/// Registry loader — reads and caches registry.json, provides lookups.
pub struct RegistryLoader {
    /// Path to the hub repository root (where registry.json lives).
    hub_dir: PathBuf,
    /// Cached registry (behind RwLock for hot-reload).
    cache: Arc<RwLock<Option<RegistryCache>>>,
}

struct RegistryCache {
    registry: Registry,
    /// Skill ID → index in registry.skills vec
    skill_index: HashMap<String, usize>,
    /// Category → list of skill indices
    category_index: HashMap<String, Vec<usize>>,
    /// File modification time of registry.json when loaded
    loaded_mtime: std::time::SystemTime,
}

impl RegistryLoader {
    /// Create a new registry loader pointed at a hub directory.
    pub fn new(hub_dir: PathBuf) -> Self {
        Self {
            hub_dir,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Path to registry.json
    fn registry_path(&self) -> PathBuf {
        self.hub_dir.join("registry.json")
    }

    /// Load or reload registry.json if it has changed.
    pub async fn ensure_loaded(&self) -> Result<()> {
        let path = self.registry_path();
        if !path.exists() {
            anyhow::bail!("registry.json not found at {}", path.display());
        }

        let meta = std::fs::metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        let mtime = meta
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        // Check if reload needed
        {
            let cache = self.cache.read().await;
            if let Some(ref c) = *cache {
                if c.loaded_mtime == mtime {
                    return Ok(());
                }
            }
        }

        // Load and parse
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        let registry: Registry = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;

        // Build indices
        let mut skill_index = HashMap::new();
        let mut category_index: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, skill) in registry.skills.iter().enumerate() {
            skill_index.insert(skill.id.clone(), i);
            category_index
                .entry(skill.category.clone())
                .or_default()
                .push(i);
        }

        tracing::info!(
            skills = registry.stats.total_skills,
            templates = registry.stats.total_templates,
            "Registry loaded from {}",
            path.display()
        );

        // Store
        let mut cache = self.cache.write().await;
        *cache = Some(RegistryCache {
            registry,
            skill_index,
            category_index,
            loaded_mtime: mtime,
        });

        Ok(())
    }

    /// Get the hub directory path.
    pub fn hub_dir(&self) -> &Path {
        &self.hub_dir
    }

    /// Look up a skill by ID.
    pub async fn find_skill(&self, skill_id: &str) -> Option<SkillEntry> {
        let cache = self.cache.read().await;
        let cache = cache.as_ref()?;
        let idx = cache.skill_index.get(skill_id)?;
        Some(cache.registry.skills[*idx].clone())
    }

    /// Get the absolute path to a skill's directory in the hub.
    pub async fn skill_dir(&self, skill_id: &str) -> Option<PathBuf> {
        let entry = self.find_skill(skill_id).await?;
        let dir = self.hub_dir.join(&entry.path);
        if dir.exists() {
            Some(dir)
        } else {
            None
        }
    }

    /// Search skills by query string (matches name, description, tags).
    pub async fn search(&self, query: &str, category: Option<&str>, limit: usize) -> Vec<SkillEntry> {
        let cache = self.cache.read().await;
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return vec![],
        };

        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        let candidates: Box<dyn Iterator<Item = &SkillEntry>> = if let Some(cat) = category {
            if let Some(indices) = cache.category_index.get(cat) {
                Box::new(indices.iter().map(|i| &cache.registry.skills[*i]))
            } else {
                return vec![];
            }
        } else {
            Box::new(cache.registry.skills.iter())
        };

        candidates
            .filter(|s| {
                if query_terms.is_empty() {
                    return true;
                }
                let haystack = format!(
                    "{} {} {} {}",
                    s.name.to_lowercase(),
                    s.description.to_lowercase(),
                    s.id.to_lowercase(),
                    s.tags.join(" ").to_lowercase()
                );
                query_terms.iter().any(|term| haystack.contains(term))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// List skills by category.
    pub async fn list_by_category(&self, category: &str) -> Vec<SkillEntry> {
        let cache = self.cache.read().await;
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return vec![],
        };

        cache
            .category_index
            .get(category)
            .map(|indices| {
                indices
                    .iter()
                    .map(|i| cache.registry.skills[*i].clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List all available categories.
    pub async fn categories(&self) -> Vec<String> {
        let cache = self.cache.read().await;
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return vec![],
        };
        let mut cats: Vec<String> = cache.category_index.keys().cloned().collect();
        cats.sort();
        cats
    }

    /// Get a template by ID.
    pub async fn find_template(&self, template_id: &str) -> Option<TemplateEntry> {
        let cache = self.cache.read().await;
        let cache = cache.as_ref()?;
        cache
            .registry
            .templates
            .iter()
            .find(|t| t.id == template_id)
            .cloned()
    }

    /// Get the list of skill IDs for a given template.
    pub async fn template_skills(&self, template_id: &str) -> Vec<String> {
        self.find_template(template_id)
            .await
            .map(|t| t.skills)
            .unwrap_or_default()
    }

    /// Get all skills metadata (for search tools).
    pub async fn all_skills(&self) -> Vec<SkillEntry> {
        let cache = self.cache.read().await;
        match cache.as_ref() {
            Some(c) => c.registry.skills.clone(),
            None => vec![],
        }
    }

    /// Get all templates metadata.
    pub async fn all_templates(&self) -> Vec<TemplateEntry> {
        let cache = self.cache.read().await;
        match cache.as_ref() {
            Some(c) => c.registry.templates.clone(),
            None => vec![],
        }
    }

    /// Check if registry is loaded.
    pub async fn is_loaded(&self) -> bool {
        self.cache.read().await.is_some()
    }
}

/// Try to load a registry from a hub directory. Returns None if registry.json doesn't exist.
pub fn try_load_registry_sync(hub_dir: &Path) -> Option<Registry> {
    let path = hub_dir.join("registry.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

// NOTE: InstalledSkills has been removed as part of the skills loading optimization.
// Runtime skill loading is purely directory-based — no installed.json needed.
// Version info is read from each skill's manifest.yaml at query time.
// See: docs/开发文档/唤星云服务-zeroclaw/公共技能管理与技能加载优化方案.md

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn sample_registry_json() -> &'static str {
        r#"{
            "version": "1.0.0",
            "generated_at": "2026-03-15T12:00:00Z",
            "engine_version": "0.1.0",
            "stats": { "total_skills": 2, "total_templates": 1, "categories": 2 },
            "skills": [
                {
                    "id": "calculator",
                    "name": "计算器",
                    "version": "1.0.0",
                    "author": "huanxing",
                    "description": "数学计算",
                    "category": "utility",
                    "subcategory": "calculator",
                    "tags": [],
                    "path": "skills/utility/calculator",
                    "platforms": ["cloud", "desktop"],
                    "risk_level": "moderate",
                    "review_status": "official",
                    "pricing_tier": "free",
                    "requires_api_keys": false,
                    "requires_cli": false,
                    "requires_permissions": ["shell"],
                    "has_scripts": true,
                    "has_wasm": false,
                    "file_count": 3,
                    "size_bytes": 2000
                },
                {
                    "id": "tavily-search",
                    "name": "Tavily 搜索",
                    "version": "1.0.0",
                    "author": "huanxing",
                    "description": "AI 搜索引擎",
                    "category": "search",
                    "subcategory": "web-search",
                    "tags": ["搜索"],
                    "path": "skills/search/tavily-search",
                    "platforms": ["cloud", "desktop"],
                    "risk_level": "moderate",
                    "review_status": "official",
                    "pricing_tier": "free",
                    "requires_api_keys": true,
                    "requires_cli": false,
                    "requires_permissions": ["network", "shell"],
                    "has_scripts": true,
                    "has_wasm": false,
                    "file_count": 4,
                    "size_bytes": 3000
                }
            ],
            "templates": [
                {
                    "id": "assistant",
                    "name": "全能助理",
                    "emoji": "🤖",
                    "description": "全面的 AI 助理",
                    "tags": ["助理"],
                    "skills": ["calculator", "tavily-search"],
                    "path": "templates/assistant",
                    "pricing_tier": "free"
                }
            ]
        }"#
    }

    #[tokio::test]
    async fn test_registry_load_and_search() {
        let tmp = TempDir::new().unwrap();
        let registry_path = tmp.path().join("registry.json");
        std::fs::write(&registry_path, sample_registry_json()).unwrap();

        let loader = RegistryLoader::new(tmp.path().to_path_buf());
        loader.ensure_loaded().await.unwrap();

        // Find by ID
        let calc = loader.find_skill("calculator").await.unwrap();
        assert_eq!(calc.name, "计算器");
        assert_eq!(calc.category, "utility");

        // Search
        let results = loader.search("搜索", None, 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "tavily-search");

        // Category list
        let cats = loader.categories().await;
        assert!(cats.contains(&"utility".to_string()));
        assert!(cats.contains(&"search".to_string()));

        // Template skills
        let skills = loader.template_skills("assistant").await;
        assert_eq!(skills, vec!["calculator", "tavily-search"]);

        // Not found
        assert!(loader.find_skill("nonexistent").await.is_none());
    }
}
