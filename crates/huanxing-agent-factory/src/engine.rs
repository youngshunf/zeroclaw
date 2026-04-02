use anyhow::Result;
use std::path::Path;

use crate::market_api::{download_bytes, get_download_info, unzip_buffer};
use crate::types::TemplateDefinition;
use crate::{AgentFactory, CreateAgentParams, ProgressSink};

fn fallback_template(id: &str) -> TemplateDefinition {
    TemplateDefinition {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        emoji: "🤖".to_string(),
        description: "Default fallback template".to_string(),
        model: "MiniMax-M2.7".to_string(),
        temperature: Some(0.7),
        skills: Default::default(),
        sops: vec![],
    }
}

// 辅助闭包：对文件内容做占位符替换
fn substitute_placeholders(
    content: &str,
    params: &CreateAgentParams,
    def: &TemplateDefinition,
    now: &str,
) -> String {
    let _final_model = params.provider.as_deref().unwrap_or("anthropic"); // Just a default fallback if missing
                                                                          // In actual implementation params.provider comes from CLI or API.
                                                                          // The main logic is that standard template defaults are used if not provided.

    content
        .replace("{{star_name}}", &params.display_name)
        .replace("{{nickname}}", &params.user_nickname)
        // Note: For backend cloud, the CLI can pass api_key and provider.
        // In desktop mode, owner/agent placeholders may intentionally stay empty.
        // Runtime falls back to `.huanxing/config.toml` for shared LLM credentials,
        // and `hasn_id` is backfilled later by the post-login HASN registration flow.
        .replace("{{default_model}}", params.provider.as_deref().unwrap_or(&def.model))
        .replace("{{default_provider}}", params.provider.as_deref().unwrap_or("anthropic"))
        .replace("{{api_key}}", params.api_key.as_deref().unwrap_or(""))
        .replace("{{default_temperature}}", &format!("{}", def.temperature.unwrap_or(0.7)))
        .replace("{{user_id}}", &params.tenant_id)
        .replace("{{agent_id}}", &params.agent_name)
        .replace("{{hasn_id}}", params.hasn_id.as_deref().unwrap_or(""))
        .replace("{{template}}", &params.template_id)
        .replace("{{created_at}}", now)
        .replace("{{createdAt}}", now)
}

impl AgentFactory {
    /// 执行实际的工作区骨架复制和替换
    pub async fn process_workspace(
        &self,
        base_dir: &Path,         // _base 的路径
        base_desktop_dir: &Path, // _base_desktop 的路径
        template_dir: &Path,     // 具体模板如 assistant/ 的路径
        target_dir: &Path,       // users/{phone}/agents/{agent_name}/
        params: &CreateAgentParams,
        def: &TemplateDefinition,
        _progress: &dyn ProgressSink,
    ) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string();

        // 核心工作区
        let workspace = target_dir.join("workspace");

        let process_file = |src_path: &Path, file_name: &str, dest_dir: &Path| -> Result<()> {
            if file_name.ends_with(".template") {
                let dest_name = file_name.trim_end_matches(".template");
                let content = std::fs::read_to_string(src_path)?;
                std::fs::write(
                    dest_dir.join(dest_name),
                    substitute_placeholders(&content, params, def, &now),
                )?;
            } else if file_name.ends_with(".md") || file_name == "template.yaml" {
                let content = std::fs::read_to_string(src_path)?;
                std::fs::write(
                    dest_dir.join(file_name),
                    substitute_placeholders(&content, params, def, &now),
                )?;
            } else {
                std::fs::copy(src_path, dest_dir.join(file_name))?;
            }
            Ok(())
        };

        // 1. Layer 1: _base/agent/
        let layer1 = base_dir.join("agent");
        let mut used_embedded = false;
        if layer1.is_dir() {
            for entry in std::fs::read_dir(&layer1)? {
                let entry = entry?;
                let file_name = entry.file_name().to_string_lossy().to_string();

                // 如果是云端但名字起头是 _，则跳过隐藏文件
                if file_name.starts_with('.') {
                    continue;
                }

                if entry.path().is_file() {
                    process_file(&entry.path(), &file_name, &workspace)?;
                }
            }
        } else {
            // Fallback to embedded agent scaffold
            used_embedded = true;
            for scaffold in crate::scaffold::agent_scaffold() {
                let dest_name = scaffold.name.trim_end_matches(".template");
                std::fs::write(
                    workspace.join(dest_name),
                    substitute_placeholders(scaffold.content, params, def, &now),
                )?;
            }
        }

        // 2. Layer 2: 如果是桌面版，加载 _base_desktop/agent/
        if params.is_desktop {
            let layer2 = base_desktop_dir.join("agent");
            if layer2.is_dir() {
                for entry in std::fs::read_dir(&layer2)? {
                    let entry = entry?;
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with('.') {
                        continue;
                    }
                    if entry.path().is_file() {
                        process_file(&entry.path(), &file_name, &workspace)?;
                    }
                }
            }
        }

        // 3. Layer 3: template_dir (assistant 模板独有: SOUL.md / IDENTITY.md 等等，将覆盖前面的基础骨架)
        if template_dir.is_dir() {
            fn process_recursive(
                dir: &Path,
                dest: &Path,
                params: &CreateAgentParams,
                def: &TemplateDefinition,
                now: &str,
            ) -> Result<()> {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') || name.starts_with('_') {
                        continue;
                    }
                    if path.is_file() {
                        let content = std::fs::read_to_string(&path)?;
                        std::fs::write(
                            dest.join(&name),
                            substitute_placeholders(&content, params, def, now),
                        )?;
                    } else if path.is_dir() {
                        let new_dest = dest.join(&name);
                        std::fs::create_dir_all(&new_dest)?;
                        process_recursive(&path, &new_dest, params, def, now)?;
                    }
                }
                Ok(())
            }
            process_recursive(template_dir, &workspace, params, def, &now)?;
        }

        // 4. 初始化 Tenant 的一些全局基础（比如租户的 BOOTSTRAP.md 和用户级 config.toml）
        let tenant_root = self.resolve_tenant_root(&params.tenant_id);
        let owner_ws = tenant_root.join("workspace");
        std::fs::create_dir_all(&owner_ws)?;

        let mut owner_dirs_to_process = vec![base_dir.join("owner")];
        if params.is_desktop {
            owner_dirs_to_process.push(base_desktop_dir.join("owner"));
        }

        let mut owner_processed = false;
        for owner_dir in owner_dirs_to_process {
            if owner_dir.is_dir() {
                owner_processed = true;
                for entry in std::fs::read_dir(&owner_dir)? {
                    let entry = entry?;
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if file_name.starts_with('.') {
                        continue;
                    }
                    if entry.path().is_file() {
                        let mut dest_name = file_name.clone();
                        if file_name.ends_with(".template") {
                            dest_name = file_name.trim_end_matches(".template").to_string();
                        }

                        // config.toml 放根目录 (users/<tenant_id>/config.toml)
                        // 其他的基础文件放 workspace/
                        let target_path = if dest_name == "config.toml" {
                            tenant_root.join(&dest_name)
                        } else {
                            owner_ws.join(&dest_name)
                        };

                        // 由于从 base -> base_desktop 顺序推进，这会产生真实的“覆盖”(override)
                        // 桌面端专属的 owner/config.toml 会直接盖掉云端的配置
                        let content = std::fs::read_to_string(&entry.path())?;
                        std::fs::write(
                            target_path,
                            substitute_placeholders(&content, params, def, &now),
                        )?;
                    }
                }
            }
        }

        if !owner_processed && used_embedded {
            // Fallback to embedded owner scaffold
            for scaffold in crate::scaffold::owner_scaffold() {
                let dest_name = scaffold.name.trim_end_matches(".template");
                let target_path = if dest_name == "config.toml" {
                    tenant_root.join(dest_name)
                } else {
                    owner_ws.join(dest_name)
                };
                if !target_path.exists() {
                    std::fs::write(
                        &target_path,
                        substitute_placeholders(scaffold.content, params, def, &now),
                    )?;
                }
            }
        }

        Ok(())
    }

    /// 下载并安装特定的 Skill / SOP (对应 marketplace 的 Step 4&5)
    pub async fn install_dependencies(
        &self,
        base_api: &str,
        resource_type: &str, // "skill" or "sop"
        id_list: &[String],
        target_dir: &Path, // targets to target_dir/workspace/skills/ 或 sops/
        progress: &dyn ProgressSink,
    ) -> Result<()> {
        let dest_subdir = target_dir
            .join("workspace")
            .join(format!("{}s", resource_type));
        tokio::fs::create_dir_all(&dest_subdir).await?;

        for rid in id_list {
            progress.on_progress(&format!("正在下发依赖 {}", resource_type), rid);
            match get_download_info(base_api, resource_type, rid).await {
                Ok(info) => {
                    if let Some(pkg_url) = info.get("package_url").and_then(|v| v.as_str()) {
                        match download_bytes(pkg_url).await {
                            Ok(bytes) => {
                                let rid_dir = dest_subdir.join(rid);
                                let _ = tokio::fs::create_dir_all(&rid_dir).await;
                                if let Err(e) = unzip_buffer(&bytes, &rid_dir) {
                                    progress.on_error(&format!("解压 {}", rid), &e.to_string());
                                }
                            }
                            Err(e) => progress.on_error(&format!("下载 {}", rid), &e.to_string()),
                        }
                    }
                }
                Err(e) => progress.on_error(&format!("获取信息 {}", rid), &e.to_string()),
            }
        }
        Ok(())
    }

    /// 从市场下载打包好的模板 zip 包，并创建 Agent
    pub async fn install_from_market(
        &self,
        params: &CreateAgentParams,
        package_url: &str,
        progress: &dyn ProgressSink,
    ) -> Result<crate::AgentCreated> {
        progress.on_progress("正在获取模板依赖包...", &params.template_id);

        let tenant_root = self.resolve_tenant_root(&params.tenant_id);
        let target_dir = tenant_root.join("agents").join(&params.agent_name);
        if target_dir.exists() {
            anyhow::bail!("Agent 目录已存在: {}", params.agent_name);
        }

        let workspace = target_dir.join("workspace");
        tokio::fs::create_dir_all(&workspace).await?;
        tokio::fs::create_dir_all(workspace.join("memory")).await?;
        tokio::fs::create_dir_all(workspace.join("files/ideas")).await?;
        tokio::fs::create_dir_all(workspace.join("files/drafts")).await?;
        tokio::fs::create_dir_all(workspace.join("files/published")).await?;

        progress.on_progress("正在下载 Agent 模板包...", &package_url);
        let zip_bytes = download_bytes(package_url).await?;

        let tmpdir = tempfile::tempdir()?;
        unzip_buffer(&zip_bytes, tmpdir.path())?;

        progress.on_progress("正在解析配置...", "配置 Agent 工作区");

        // We assume the ZIP contains template files at the root, AND contains `_base` and `_base_desktop`
        // as directories embedded inside it by the cloud API.
        // Let's parse `template.yaml` from tmpdir root.
        let yaml_path = tmpdir.path().join("template.yaml");
        let def = if yaml_path.exists() {
            let content = std::fs::read_to_string(&yaml_path)?;
            serde_yaml::from_str::<TemplateDefinition>(&content)?
        } else {
            fallback_template(&params.template_id)
        };

        let base_dir = tmpdir.path().join("_base");
        let base_desktop_dir = tmpdir.path().join("_base_desktop");

        // The template files are IN the tmpdir.path()
        self.process_workspace(
            &base_dir,
            &base_desktop_dir,
            tmpdir.path(),
            &target_dir,
            params,
            &def,
            progress,
        )
        .await?;

        // Download skills & sops
        if let Some(api) = &self.market_api_base {
            self.install_dependencies(api, "skill", &def.skills.exclusive, &target_dir, progress)
                .await?;
            self.install_dependencies(api, "sop", &def.sops, &target_dir, progress)
                .await?;
        }

        progress.on_progress("Agent 创建完成!", &params.agent_name);
        Ok(crate::AgentCreated {
            tenant_id: params.tenant_id.clone(),
            agent_id: params.agent_name.clone(),
            workspace_dir: workspace,
        })
    }

    /// 从本地 hub 目录读取并创建 Agent (主要由 Cloud Backend API 或 CLI 调用)
    pub async fn create_local_agent(
        &self,
        templates_base_path: &Path, // e.g. `<workspace_dir>/hub/templates/`
        params: &CreateAgentParams,
        progress: &dyn ProgressSink,
    ) -> Result<crate::AgentCreated> {
        let tenant_root = self.resolve_tenant_root(&params.tenant_id);
        let target_dir = tenant_root.join("agents").join(&params.agent_name);
        if target_dir.exists() {
            anyhow::bail!("Agent 目录已存在: {}", params.agent_name);
        }

        let workspace = target_dir.join("workspace");
        tokio::fs::create_dir_all(&workspace).await?;
        tokio::fs::create_dir_all(workspace.join("memory")).await?;
        tokio::fs::create_dir_all(workspace.join("files/ideas")).await?;
        tokio::fs::create_dir_all(workspace.join("files/drafts")).await?;
        tokio::fs::create_dir_all(workspace.join("files/published")).await?;

        let base_dir = templates_base_path.join("_base");
        let base_desktop_dir = templates_base_path.join("_base_desktop");
        let template_dir = templates_base_path.join(&params.template_id);

        let yaml_path = template_dir.join("template.yaml");
        let def = if yaml_path.exists() {
            let content = std::fs::read_to_string(&yaml_path)?;
            serde_yaml::from_str::<TemplateDefinition>(&content)?
        } else {
            fallback_template(&params.template_id)
        };

        self.process_workspace(
            &base_dir,
            &base_desktop_dir,
            &template_dir,
            &target_dir,
            params,
            &def,
            progress,
        )
        .await?;

        // 默认尝试把本地的 skills 复制过去 (本地 hub 理论上包含所有基础技能)
        let local_hub_skills = templates_base_path.parent().unwrap().join("skills");
        if local_hub_skills.exists() {
            let dest_skills_dir = target_dir.join("workspace").join("skills");
            for skill_id in &def.skills.exclusive {
                let src_skill = local_hub_skills.join(skill_id);
                if src_skill.exists() {
                    let _ = tokio::fs::create_dir_all(&dest_skills_dir.join(skill_id)).await;
                    let mut opts = fs_extra::dir::CopyOptions::new();
                    opts.content_only = true;
                    opts.overwrite = true;
                    let _ = fs_extra::dir::copy(&src_skill, dest_skills_dir.join(skill_id), &opts);
                }
            }
        }

        progress.on_progress("Agent 工作区创建完毕", &params.agent_name);
        Ok(crate::AgentCreated {
            tenant_id: params.tenant_id.clone(),
            agent_id: params.agent_name.clone(),
            workspace_dir: workspace,
        })
    }
}
