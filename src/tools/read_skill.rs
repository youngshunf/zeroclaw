use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Compact-mode helper for loading a skill's source file on demand.
pub struct ReadSkillTool {
    workspace_dir: PathBuf,
    config: Arc<crate::config::Config>,
    /// Global skills directory (Level 1 — platform-wide shared skills)
    global_skills_dir: Option<PathBuf>,
    /// User/tenant skills directory (Level 2 — user-shared skills)
    user_skills_dir: Option<PathBuf>,
}

impl ReadSkillTool {
    pub fn new(
        workspace_dir: PathBuf,
        config: Arc<crate::config::Config>,
    ) -> Self {
        Self {
            workspace_dir,
            config,
            global_skills_dir: None,
            user_skills_dir: None,
        }
    }

    /// Set extra skill directories for three-level cascade loading.
    pub fn with_extra_dirs(
        mut self,
        global_skills_dir: Option<PathBuf>,
        user_skills_dir: Option<PathBuf>,
    ) -> Self {
        self.global_skills_dir = global_skills_dir;
        self.user_skills_dir = user_skills_dir;
        self
    }

    /// Derive global skills directory from Config when task-local is unavailable.
    ///
    /// Uses HuanXing's `resolve_common_skills_dir()` when the feature is active,
    /// falling back to `{config_dir}/skills/` for vanilla ZeroClaw.
    fn derive_global_skills_dir(&self) -> Option<PathBuf> {
        let config_dir = self.config.config_path.parent()?;
        #[cfg(feature = "huanxing")]
        {
            if self.config.huanxing.enabled {
                let common_dir = self.config.huanxing.resolve_common_skills_dir(config_dir);
                // resolve_common_skills_dir returns {config_dir}/skills/ by default.
                // The actual skills may be directly in this dir or in a `skills/` subdirectory.
                let nested = common_dir.join("skills");
                if nested.exists() {
                    return Some(nested);
                }
                if common_dir.exists() {
                    return Some(common_dir);
                }
            }
        }
        // Vanilla ZeroClaw: check {config_dir}/skills/
        let dir = config_dir.join("skills");
        if dir.exists() { Some(dir) } else { None }
    }

    /// Derive user/tenant skills directory from workspace_dir when task-local is unavailable.
    ///
    /// Workspace layout: `{config_dir}/users/{td}/agents/{id}/workspace/`
    /// User skills dir:  `{config_dir}/users/{td}/workspace/skills/`
    fn derive_user_skills_dir(&self) -> Option<PathBuf> {
        // Walk up: workspace/ → agents/{id} → agents/ → users/{td}
        let tenant_root = self.workspace_dir
            .parent()  // agents/{id}
            .and_then(|p| p.parent())  // agents/
            .and_then(|p| p.parent())?;  // users/{td}
        let user_skills = tenant_root.join("workspace").join("skills");
        if user_skills.exists() { Some(user_skills) } else { None }
    }
}

#[async_trait]
impl Tool for ReadSkillTool {
    fn name(&self) -> &str {
        "read_skill"
    }

    fn description(&self) -> &str {
        "Read the full source file for an available skill by name. Use this in compact skills mode when you need the complete skill instructions without remembering file paths."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The skill name exactly as listed in <available_skills>."
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let requested = args
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;

        // Resolve effective global/user dirs with three-level fallback:
        //   1. Struct fields (set at construction time)
        //   2. Task-local variables (injected by channels/mod.rs pipeline)
        //   3. Dynamic derivation from Config (covers WS/desktop path where
        //      task-locals are never injected)
        let global_dir = self
            .global_skills_dir
            .clone()
            .or_else(crate::skills::get_active_global_skills_dir)
            .or_else(|| self.derive_global_skills_dir());
        let user_dir = self
            .user_skills_dir
            .clone()
            .or_else(crate::skills::get_active_user_skills_dir)
            .or_else(|| self.derive_user_skills_dir());

        tracing::info!(
            requested_skill = requested,
            workspace_dir = %self.workspace_dir.display(),
            global_dir = ?global_dir,
            user_dir = ?user_dir,
            config_path = %self.config.config_path.display(),
            huanxing_enabled = cfg!(feature = "huanxing"),
            "【read_skill 调试】开始加载技能"
        );

        let all_skills = crate::skills::load_skills_cascaded(
            global_dir.as_deref(),
            user_dir.as_deref(),
            &self.workspace_dir,
            &self.config,
        );

        let skill_names: Vec<&str> = all_skills.iter().map(|s| s.name.as_str()).collect();
        tracing::info!(
            total = all_skills.len(),
            names = ?skill_names,
            "【read_skill 调试】技能加载完成"
        );

        let Some(skill) = all_skills
            .iter()
            .find(|skill| skill.name.eq_ignore_ascii_case(requested))
        else {
            let mut names: Vec<&str> = all_skills.iter().map(|skill| skill.name.as_str()).collect();
            names.sort_unstable();
            let available = if names.is_empty() {
                "none".to_string()
            } else {
                names.join(", ")
            };

            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Unknown skill '{requested}'. Available skills: {available}"
                )),
            });
        };

        let Some(location) = skill.location.as_ref() else {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Skill '{}' has no readable source location.",
                    skill.name
                )),
            });
        };

        match tokio::fs::read_to_string(location).await {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(err) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Failed to read skill '{}' from {}: {err}",
                    skill.name,
                    location.display()
                )),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tool(tmp: &TempDir) -> ReadSkillTool {
        ReadSkillTool::new(tmp.path().join("workspace"), std::sync::Arc::new(crate::config::Config::default()))
    }

    #[tokio::test]
    async fn reads_markdown_skill_by_name() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/weather");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Weather\n\nUse this skill for forecast lookups.\n",
        )
        .unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "weather" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("# Weather"));
        assert!(result.output.contains("forecast lookups"));
    }

    #[tokio::test]
    async fn reads_toml_skill_manifest_by_name() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/deploy");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.toml"),
            r#"[skill]
name = "deploy"
description = "Ship safely"
"#,
        )
        .unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "deploy" }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("[skill]"));
        assert!(result.output.contains("Ship safely"));
    }

    #[tokio::test]
    async fn unknown_skill_lists_available_names() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("workspace/skills/weather");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Weather\n").unwrap();

        let result = make_tool(&tmp)
            .execute(json!({ "name": "calendar" }))
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Unknown skill 'calendar'. Available skills: weather")
        );
    }
}
