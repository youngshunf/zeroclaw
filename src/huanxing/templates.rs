//! Template engine for creating per-user agent workspaces.
//!
//! Reads template definitions from `templates/{name}/template.yaml` (preferred)
//! or `templates/{name}/template.json` (legacy) and copies workspace files with
//! placeholder substitution.
//!
//! Skills are installed from the hub repository (via RegistryLoader) rather than
//! being embedded in template directories.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::registry::RegistryLoader;

// ═══════════════════════════════════════════════════════
// Template definition (supports both YAML and JSON)
// ═══════════════════════════════════════════════════════

/// Parsed template definition — works with both template.yaml and template.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub temperature: Option<f64>,

    /// Skills — supports both formats:
    /// - Legacy (flat list): `"skills": ["calc", "search"]` → all treated as exclusive
    /// - New (structured): `"skills": { "exclusive": [...], "common": [...] }`
    #[serde(default, deserialize_with = "deserialize_skills")]
    pub skills: SkillsConfig,

    #[serde(default)]
    pub tools_allow: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,

    #[serde(default)]
    pub pricing: Option<PricingConfig>,
    #[serde(default)]
    pub onboarding: Option<OnboardingConfig>,
}

/// Skills configuration — template-exclusive skills copied to user workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    /// Template-exclusive skills: copied to user workspace on creation.
    /// Can also be a flat list in template.yaml.
    #[serde(default)]
    pub exclusive: Vec<String>,
}

impl SkillsConfig {
    /// All skill IDs.
    pub fn all(&self) -> Vec<&str> {
        self.exclusive.iter().map(|s| s.as_str()).collect()
    }
}

/// Custom deserializer: accepts either a flat list or structured {exclusive: [...]}.
fn deserialize_skills<'de, D>(deserializer: D) -> std::result::Result<SkillsConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SkillsRaw {
        /// Flat list: all skills → exclusive
        Flat(Vec<String>),
        /// Structured: {exclusive: [...]}
        Structured(SkillsConfig),
    }

    match SkillsRaw::deserialize(deserializer) {
        Ok(SkillsRaw::Flat(list)) => Ok(SkillsConfig { exclusive: list }),
        Ok(SkillsRaw::Structured(cfg)) => Ok(cfg),
        Err(e) => {
            tracing::debug!("Skills deserialization fallback: {e}");
            Ok(SkillsConfig::default())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    #[serde(default)]
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingConfig {
    #[serde(default)]
    pub welcome_message: String,
    #[serde(default)]
    pub suggested_questions: Vec<String>,
}

// ═══════════════════════════════════════════════════════
// Template engine
// ═══════════════════════════════════════════════════════

/// Template engine for workspace creation.
pub struct TemplateEngine {
    templates_dir: PathBuf,
    /// Optional: hub registry for installing skills from the marketplace.
    registry: Option<Arc<RegistryLoader>>,
}

/// User info for placeholder substitution.
pub struct UserInfo<'a> {
    pub nickname: &'a str,
    pub phone: &'a str,
    pub star_name: &'a str,
    pub user_id: &'a str,
    pub agent_id: &'a str,
    pub template: &'a str,
}

impl TemplateEngine {
    /// Create a new template engine (without hub registry — legacy mode).
    pub fn new(templates_dir: PathBuf) -> Self {
        Self {
            templates_dir,
            registry: None,
        }
    }

    /// Create a new template engine with hub registry support.
    pub fn with_registry(templates_dir: PathBuf, registry: Arc<RegistryLoader>) -> Self {
        Self {
            templates_dir,
            registry: Some(registry),
        }
    }

    /// Load a template definition from template.yaml (preferred) or template.json (legacy).
    pub fn load_definition(&self, template_name: &str) -> Result<TemplateDefinition> {
        let template_dir = self.templates_dir.join(template_name);

        // Try YAML first (new format)
        let yaml_path = template_dir.join("template.yaml");
        if yaml_path.exists() {
            let content = std::fs::read_to_string(&yaml_path)
                .with_context(|| format!("Failed to read {}", yaml_path.display()))?;
            let def: TemplateDefinition = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", yaml_path.display()))?;
            tracing::debug!(template = template_name, "Loaded template.yaml");
            return Ok(def);
        }

        // Fall back to JSON (legacy format)
        let json_path = template_dir.join("template.json");
        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)
                .with_context(|| format!("Failed to read {}", json_path.display()))?;
            let def: TemplateDefinition = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", json_path.display()))?;
            tracing::debug!(template = template_name, "Loaded template.json (legacy)");
            return Ok(def);
        }

        // Return a minimal default template
        tracing::warn!(
            template = template_name,
            "No template.yaml or template.json found, using defaults"
        );
        Ok(TemplateDefinition {
            id: template_name.to_string(),
            name: template_name.to_string(),
            version: String::new(),
            emoji: "⭐".to_string(),
            description: String::new(),
            preview: String::new(),
            tags: Vec::new(),
            model: "claude-sonnet-4-6".to_string(),
            temperature: None,
            skills: SkillsConfig::default(),
            tools_allow: Vec::new(),
            files: vec![
                "SOUL.md".to_string(),
                "IDENTITY.md".to_string(),
                "USER.md.template".to_string(),
                "MEMORY.md".to_string(),
            ],
            pricing: None,
            onboarding: None,
        })
    }

    /// List available template names.
    pub fn list_templates(&self) -> Result<Vec<String>> {
        let mut templates = Vec::new();
        if !self.templates_dir.exists() {
            return Ok(templates);
        }
        for entry in std::fs::read_dir(&self.templates_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip _base, _reference, hidden dirs
            if name.starts_with('_') || name.starts_with('.') {
                continue;
            }
            if entry.path().is_dir() {
                templates.push(name);
            }
        }
        templates.sort();
        Ok(templates)
    }

    /// Create a full agent workspace from a template.
    ///
    /// Steps:
    /// 1. Create workspace directory structure
    /// 2. Copy ALL _base files → overlay with template-specific files → then def.files extras
    /// 3. Install skills from hub registry (or fallback to template dir scanning)
    /// 4. Generate per-agent config.toml
    pub async fn create_workspace(
        &self,
        workspace_dir: &Path,
        user_info: &UserInfo<'_>,
        provider: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<Vec<String>> {
        let template_name = user_info.template;
        let def = self.load_definition(template_name)?;

        // 1. Create directory structure
        tokio::fs::create_dir_all(workspace_dir).await?;
        tokio::fs::create_dir_all(workspace_dir.join("memory")).await?;
        tokio::fs::create_dir_all(workspace_dir.join("files")).await?;
        tokio::fs::create_dir_all(workspace_dir.join("files/ideas")).await?;
        tokio::fs::create_dir_all(workspace_dir.join("files/drafts")).await?;
        tokio::fs::create_dir_all(workspace_dir.join("files/published")).await?;

        let mut created_files = Vec::new();

        // 2. Copy ALL _base files first (foundation), then overlay template-specific files.
        let template_dir = self.templates_dir.join(template_name);
        let base_dir = self.templates_dir.join("_base");

        // 2a. Copy all files from _base/ directory
        if base_dir.exists() {
            let mut entries = tokio::fs::read_dir(&base_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden files
                if file_name.starts_with('.') {
                    continue;
                }

                let dest_name = if file_name.ends_with(".template") {
                    file_name.trim_end_matches(".template").to_string()
                } else {
                    file_name.clone()
                };
                let dest = workspace_dir.join(&dest_name);

                let content = tokio::fs::read_to_string(&path).await?;
                let content = self.substitute_placeholders(&content, user_info);
                tokio::fs::write(&dest, &content).await?;
                created_files.push(file_name.clone());
                tracing::debug!(file = %file_name, "Copied from _base");
            }
        }

        // 2b. Overlay template-specific files (overwrite _base versions)
        if template_dir.exists() {
            let mut entries = tokio::fs::read_dir(&template_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let file_name = entry.file_name().to_string_lossy().to_string();
                // Skip hidden files and template definition files
                if file_name.starts_with('.')
                    || file_name == "template.yaml"
                    || file_name == "template.json"
                {
                    continue;
                }

                let dest_name = if file_name.ends_with(".template") {
                    file_name.trim_end_matches(".template").to_string()
                } else {
                    file_name.clone()
                };
                let dest = workspace_dir.join(&dest_name);

                let content = tokio::fs::read_to_string(&path).await?;
                let content = self.substitute_placeholders(&content, user_info);
                tokio::fs::write(&dest, &content).await?;
                if !created_files.contains(&file_name) {
                    created_files.push(file_name.clone());
                }
                tracing::debug!(file = %file_name, template = template_name, "Overlaid from template");
            }
        }

        // 2c. Also copy any explicitly listed files from def.files (in case they
        //     are not in _base or template dir as loose files)
        for file_name in &def.files {
            let dest_name = if file_name.ends_with(".template") {
                file_name.trim_end_matches(".template").to_string()
            } else {
                file_name.clone()
            };
            let dest = workspace_dir.join(&dest_name);
            if dest.exists() {
                continue; // already copied above
            }

            let source = if template_dir.join(file_name).exists() {
                template_dir.join(file_name)
            } else if base_dir.join(file_name).exists() {
                base_dir.join(file_name)
            } else {
                tracing::debug!("Template file not found, skipping: {file_name}");
                continue;
            };

            let content = tokio::fs::read_to_string(&source).await?;
            let content = self.substitute_placeholders(&content, user_info);
            tokio::fs::write(&dest, &content).await?;
            created_files.push(file_name.clone());
        }

        // 3. Install skills
        let installed_skills = self.install_skills(workspace_dir, &def).await?;

        // 5. Generate per-agent config.toml
        self.generate_agent_config(workspace_dir, &def, provider, api_key)
            .await?;
        created_files.push("config.toml".to_string());

        tracing::info!(
            agent_id = user_info.agent_id,
            template = template_name,
            files = created_files.len(),
            skills = installed_skills.len(),
            "Agent workspace created"
        );

        Ok(created_files)
    }

    /// Install skills for a workspace.
    ///
    /// **New behavior** (with registry): installs from hub repository via RegistryLoader.
    /// **Legacy fallback** (no registry): scans template's `skills/` subdirectory.
    ///
    /// - Exclusive skills → copied to `workspace/skills/`
    /// - Common skills → loaded at runtime from common_skills_dir (not handled here)
    async fn install_skills(
        &self,
        workspace_dir: &Path,
        def: &TemplateDefinition,
    ) -> Result<Vec<String>> {
        let mut installed = Vec::new();

        // Try registry-based installation first
        if let Some(ref registry) = self.registry {
            let _ = registry.ensure_loaded().await;

            // Install exclusive skills from hub
            for skill_id in &def.skills.exclusive {
                match self
                    .install_skill_from_hub(workspace_dir, registry, skill_id, "exclusive")
                    .await
                {
                    Ok(_version) => {
                        installed.push(skill_id.clone());
                        tracing::debug!(
                            skill = %skill_id,
                            template = &def.id,
                            "Installed exclusive skill from hub"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            skill = %skill_id,
                            error = %e,
                            "Failed to install exclusive skill from hub, trying template dir"
                        );
                        // Fallback: try template's skills/ directory
                        if self
                            .install_skill_from_template_dir(workspace_dir, &def.id, skill_id)
                            .await
                        {
                            installed.push(skill_id.clone());
                        }
                    }
                }
            }
        } else {
            // Legacy mode: scan template's skills/ directory
            tracing::debug!(
                template = &def.id,
                "No registry available, using legacy template dir scanning"
            );
            let template_dir = self.templates_dir.join(&def.id);
            let skills_src = template_dir.join("skills");

            if skills_src.exists() && skills_src.is_dir() {
                let skills_dest = workspace_dir.join("skills");

                for entry in std::fs::read_dir(&skills_src)? {
                    let entry = entry?;
                    if entry.path().is_dir() {
                        let skill_name = entry.file_name().to_string_lossy().to_string();
                        tokio::fs::create_dir_all(&skills_dest).await?;
                        let dest = skills_dest.join(&skill_name);
                        copy_dir_recursive(&entry.path(), &dest).await?;
                        installed.push(skill_name.clone());
                        tracing::debug!(
                            skill = %skill_name,
                            template = &def.id,
                            "Installed template skill (legacy)"
                        );
                    }
                }
            }
        }

        Ok(installed)
    }

    /// Install a single skill from the hub repository.
    async fn install_skill_from_hub(
        &self,
        workspace_dir: &Path,
        registry: &RegistryLoader,
        skill_id: &str,
        _source: &str,
    ) -> Result<String> {
        let entry = registry
            .find_skill(skill_id)
            .await
            .with_context(|| format!("Skill '{}' not found in registry", skill_id))?;

        let src_dir = registry
            .skill_dir(skill_id)
            .await
            .with_context(|| format!("Skill '{}' directory not found in hub", skill_id))?;

        let dest_dir = workspace_dir.join("skills").join(skill_id);
        tokio::fs::create_dir_all(dest_dir.parent().unwrap()).await?;

        if dest_dir.exists() {
            // Already exists, skip
            return Ok(entry.version);
        }

        copy_dir_recursive(&src_dir, &dest_dir).await?;
        Ok(entry.version)
    }

    /// Fallback: install skill from template's embedded skills/ directory.
    async fn install_skill_from_template_dir(
        &self,
        workspace_dir: &Path,
        template_name: &str,
        skill_id: &str,
    ) -> bool {
        let src = self
            .templates_dir
            .join(template_name)
            .join("skills")
            .join(skill_id);
        if !src.exists() {
            return false;
        }

        let dest = workspace_dir.join("skills").join(skill_id);
        if let Err(e) = tokio::fs::create_dir_all(dest.parent().unwrap()).await {
            tracing::warn!("Failed to create skills dir: {e}");
            return false;
        }
        match copy_dir_recursive(&src, &dest).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("Failed to copy skill from template dir: {e}");
                false
            }
        }
    }

    /// Generate per-agent config.toml from template.
    async fn generate_agent_config(
        &self,
        workspace_dir: &Path,
        def: &TemplateDefinition,
        provider: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<()> {
        let model = if def.model.is_empty() {
            "claude-sonnet-4-6"
        } else {
            // Strip "anthropic/" prefix if present
            def.model.strip_prefix("anthropic/").unwrap_or(&def.model)
        };

        let provider_str = provider.ok_or_else(|| {
            anyhow::anyhow!("default_provider not configured in [huanxing] section of config.toml")
        })?;
        let api_key_str = api_key.unwrap_or("");
        let temperature = def
            .temperature
            .map(|t| t.to_string())
            .unwrap_or_else(|| "0.7".to_string());

        // Try to read the config template
        let template_path = self.templates_dir.join("_base/config.toml.template");
        let config = if template_path.exists() {
            let template_content = tokio::fs::read_to_string(&template_path).await?;
            template_content
                .replace("{{default_provider}}", provider_str)
                .replace("{{default_model}}", model)
                .replace("{{default_temperature}}", &temperature)
                .replace("{{api_key}}", api_key_str)
        } else {
            tracing::warn!(
                "Config template not found at {}, using minimal fallback",
                template_path.display()
            );
            format!(
                r#"default_provider = "{provider_str}"
default_model = "{model}"
default_temperature = {temperature}
api_key = "{api_key_str}"

[agent.session]
backend = "sqlite"
strategy = "per-sender"
ttl_seconds = 86400
max_messages = 100

[memory]
auto_save = true
"#
            )
        };

        tokio::fs::write(workspace_dir.join("config.toml"), config).await?;
        Ok(())
    }

    /// Substitute placeholders in content.
    fn substitute_placeholders(&self, content: &str, info: &UserInfo<'_>) -> String {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        content
            .replace("{{nickname}}", info.nickname)
            .replace("{{phone}}", info.phone)
            .replace("{{star_name}}", info.star_name)
            .replace("{{user_id}}", info.user_id)
            .replace("{{agent_id}}", info.agent_id)
            .replace("{{template}}", info.template)
            .replace("{{createdAt}}", &now)
            .replace("{{created_at}}", &now)
    }
}

/// Recursively copy a directory.
async fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dest).await?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dest_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dest_path).await?;
        }
    }
    Ok(())
}
