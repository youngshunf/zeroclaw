use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

/// Compact-mode helper for loading a skill's source file on demand.
pub struct ReadSkillTool {
    workspace_dir: PathBuf,
    open_skills_enabled: bool,
    open_skills_dir: Option<String>,
    /// 额外技能目录（如多租户模式下的 common_skills_dir）
    extra_skills_dirs: Vec<PathBuf>,
}

impl ReadSkillTool {
    pub fn new(
        workspace_dir: PathBuf,
        open_skills_enabled: bool,
        open_skills_dir: Option<String>,
    ) -> Self {
        Self {
            workspace_dir,
            open_skills_enabled,
            open_skills_dir,
            extra_skills_dirs: Vec::new(),
        }
    }

    /// 添加额外的技能搜索目录（如 common_skills_dir）
    pub fn with_extra_skills_dir(mut self, dir: PathBuf) -> Self {
        self.extra_skills_dirs.push(dir);
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

        let skills = crate::skills::load_skills_with_open_skills_settings(
            &self.workspace_dir,
            self.open_skills_enabled,
            self.open_skills_dir.as_deref(),
        );

        // 合并额外目录（如 common_skills_dir）中的技能，workspace 同名技能优先
        let ws_names: std::collections::HashSet<String> =
            skills.iter().map(|s| s.name.clone()).collect();
        let mut all_skills = skills;
        for extra_dir in &self.extra_skills_dirs {
            for skill in crate::skills::load_skills_with_open_skills_settings(
                extra_dir,
                self.open_skills_enabled,
                self.open_skills_dir.as_deref(),
            ) {
                if !ws_names.contains(&skill.name) {
                    all_skills.push(skill);
                }
            }
        }

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
        ReadSkillTool::new(tmp.path().join("workspace"), false, None)
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
    async fn reads_skill_from_extra_dir() {
        let tmp = TempDir::new().unwrap();
        // workspace 没有任何技能
        let workspace = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        // common-skills 目录有 newsnow
        let common_dir = tmp.path().join("common-skills");
        let skill_dir = common_dir.join("skills/newsnow");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Newsnow\n\nFetch hot news.\n").unwrap();

        let tool = ReadSkillTool::new(workspace, false, None).with_extra_skills_dir(common_dir);

        let result = tool.execute(json!({ "name": "newsnow" })).await.unwrap();

        assert!(result.success, "error: {:?}", result.error);
        assert!(result.output.contains("# Newsnow"));
        assert!(result.output.contains("hot news"));
    }

    #[tokio::test]
    async fn workspace_skill_takes_priority_over_extra_dir() {
        let tmp = TempDir::new().unwrap();
        // workspace 有 newsnow（用户自定义版本）
        let workspace = tmp.path().join("workspace");
        let ws_skill = workspace.join("skills/newsnow");
        std::fs::create_dir_all(&ws_skill).unwrap();
        std::fs::write(ws_skill.join("SKILL.md"), "# Custom Newsnow\n").unwrap();
        // common-skills 也有 newsnow
        let common_dir = tmp.path().join("common-skills");
        let common_skill = common_dir.join("skills/newsnow");
        std::fs::create_dir_all(&common_skill).unwrap();
        std::fs::write(common_skill.join("SKILL.md"), "# Default Newsnow\n").unwrap();

        let tool = ReadSkillTool::new(workspace, false, None).with_extra_skills_dir(common_dir);

        let result = tool.execute(json!({ "name": "newsnow" })).await.unwrap();

        assert!(result.success);
        assert!(
            result.output.contains("Custom Newsnow"),
            "workspace version should win"
        );
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
