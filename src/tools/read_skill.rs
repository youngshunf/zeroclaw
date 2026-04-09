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

        // Resolve effective global/user dirs: prefer struct fields, then
        // fall back to task-local variables injected by HuanXing dispatcher.
        let global_dir = self
            .global_skills_dir
            .clone()
            .or_else(crate::skills::get_active_global_skills_dir);
        let user_dir = self
            .user_skills_dir
            .clone()
            .or_else(crate::skills::get_active_user_skills_dir);

        let all_skills = crate::skills::load_skills_cascaded(
            global_dir.as_deref(),
            user_dir.as_deref(),
            &self.workspace_dir,
            &self.config,
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
