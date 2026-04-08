use anyhow::Result;
use std::path::Path;

use crate::market_api::{download_bytes, get_download_info, unzip_buffer};
use crate::types::TemplateDefinition;
use crate::{AgentFactory, CreateAgentParams, ProgressSink};

pub fn fallback_template(id: &str) -> TemplateDefinition {
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
    let final_model = params.model.as_deref().unwrap_or(&def.model);
    let final_provider = params.provider.as_deref().unwrap_or("");
    let final_fallback = params.fallback_provider.as_deref().unwrap_or("");
    let final_embedding = params
        .embedding_provider
        .as_deref()
        .unwrap_or(final_provider);
    let final_llm_gw = params.llm_gateway.as_deref().unwrap_or("");

    content
        .replace("{{star_name}}", &params.display_name)
        .replace("{{nickname}}", &params.user_nickname)
        .replace("{{phone}}", &params.user_phone)
        .replace("{{owner_dir}}", &params.owner_dir)
        .replace("{{default_model}}", final_model)
        .replace("{{default_provider}}", final_provider)
        .replace("{{fallback_provider}}", final_fallback)
        .replace("{{embedding_provider}}", final_embedding)
        .replace("{{llm_gateway}}", final_llm_gw)
        .replace("{{api_key}}", params.api_key.as_deref().unwrap_or(""))
        .replace(
            "{{default_temperature}}",
            &format!("{}", def.temperature.unwrap_or(0.7)),
        )
        .replace("{{user_id}}", &params.tenant_id)
        .replace("{{agent_id}}", &params.agent_name)
        .replace("{{hasn_id}}", params.hasn_id.as_deref().unwrap_or(""))
        .replace("{{template}}", &params.template_id)
        .replace("{{created_at}}", now)
        .replace("{{createdAt}}", now)
}

/// Public wrapper for substitute_placeholders — used by main.rs config repair logic
pub fn substitute_placeholders_pub(
    content: &str,
    params: &CreateAgentParams,
    def: &TemplateDefinition,
    now: &str,
) -> String {
    substitute_placeholders(content, params, def, now)
}

#[cfg(test)]
mod tests {
    use super::{fallback_template, substitute_placeholders};
    use crate::CreateAgentParams;

    #[test]
    fn substitute_placeholders_keeps_model_and_provider_independent() {
        let def = fallback_template("assistant");
        let params = CreateAgentParams {
            tenant_id: "001-tenant-a".to_string(),
            template_id: "assistant".to_string(),
            agent_name: "default".to_string(),
            display_name: "Star".to_string(),
            is_desktop: true,
            user_nickname: "Nick".to_string(),
            user_phone: "18611348367".to_string(),
            owner_dir: "/tmp/test/workspace".to_string(),
            provider: Some("custom:https://llm.example.com/v1".to_string()),
            model: Some("qwen3-32b".to_string()),
            api_key: None,
            hasn_id: None,
            fallback_provider: Some("custom:https://fallback.example.com/v1".to_string()),
            embedding_provider: Some("custom:https://embed.example.com/v1".to_string()),
            llm_gateway: Some("http://127.0.0.1:3180/v1".to_string()),
        };

        let rendered = substitute_placeholders(
            "provider={{default_provider}}\nmodel={{default_model}}\napi_key={{api_key}}\nhasn_id={{hasn_id}}\nfallback={{fallback_provider}}\nembedding={{embedding_provider}}\ngw={{llm_gateway}}\n",
            &params,
            &def,
            "2026-04-02 12:00",
        );

        assert!(rendered.contains("provider=custom:https://llm.example.com/v1"));
        assert!(rendered.contains("model=qwen3-32b"));
        assert!(rendered.contains("api_key="));
        assert!(rendered.contains("hasn_id="));
        assert!(rendered.contains("fallback=custom:https://fallback.example.com/v1"));
        assert!(rendered.contains("embedding=custom:https://embed.example.com/v1"));
        assert!(rendered.contains("gw=http://127.0.0.1:3180/v1"));
    }
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
                let dest_path = workspace.join(dest_name);
                match scaffold.content {
                    crate::scaffold::EmbeddedContent::Text(t) => {
                        std::fs::write(
                            dest_path,
                            substitute_placeholders(t, params, def, &now),
                        )?;
                    }
                    crate::scaffold::EmbeddedContent::Binary(b) => {
                        std::fs::write(dest_path, b)?;
                    }
                }
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
                    // .template 文件已由 Layer 1/2 的 process_file 处理（渲染+去后缀），
                    // Layer 3 不应再将原始 .template 文件复制进 workspace
                    if name.ends_with(".template") {
                        continue;
                    }
                    // template.yaml 是模板元数据，不复制到 workspace
                    if name == "template.yaml" {
                        continue;
                    }
                    if path.is_file() {
                        // 二进制文件（图片等）直接复制，不做文本替换
                        let is_binary = name.ends_with(".png")
                            || name.ends_with(".jpg")
                            || name.ends_with(".jpeg")
                            || name.ends_with(".gif")
                            || name.ends_with(".webp")
                            || name.ends_with(".ico");
                        if is_binary {
                            std::fs::copy(&path, dest.join(&name))?;
                        } else {
                            let content = std::fs::read_to_string(&path)?;
                            std::fs::write(
                                dest.join(&name),
                                substitute_placeholders(&content, params, def, now),
                            )?;
                        }
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
        //    重要：如果 owner 配置已存在（桌面端 onboard 登录时已创建），则跳过覆写。
        //    市场安装额外 Agent 时不应修改已有的租户级配置。
        let tenant_root = self.resolve_tenant_root(&params.tenant_id);
        let owner_config_exists = tenant_root.join("config.toml").exists();

        if !owner_config_exists {
            let owner_ws = tenant_root.join("workspace");
            std::fs::create_dir_all(&owner_ws)?;

            let mut owner_dirs_to_process = vec![base_dir.join("owner")];
            if params.is_desktop {
                owner_dirs_to_process.push(base_desktop_dir.join("owner"));
            }

            let mut owner_processed_flag = false;
            for owner_dir in owner_dirs_to_process {
                if owner_dir.is_dir() {
                    owner_processed_flag = true;
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

                            let content = std::fs::read_to_string(&entry.path())?;
                            std::fs::write(
                                target_path,
                                substitute_placeholders(&content, params, def, &now),
                            )?;
                        }
                    }
                }
            }

            if !owner_processed_flag && used_embedded {
                // Fallback to embedded owner scaffold
                for scaffold in crate::scaffold::owner_scaffold() {
                    let dest_name = scaffold.name.trim_end_matches(".template");
                    let target_path = if dest_name == "config.toml" {
                        tenant_root.join(dest_name)
                    } else {
                        owner_ws.join(dest_name)
                    };
                    if !target_path.exists() {
                        match scaffold.content {
                            crate::scaffold::EmbeddedContent::Text(t) => {
                                std::fs::write(
                                    &target_path,
                                    substitute_placeholders(t, params, def, &now),
                                )?;
                            }
                            crate::scaffold::EmbeddedContent::Binary(b) => {
                                std::fs::write(&target_path, b)?;
                            }
                        }
                    }
                }
            }
        } // end if !owner_config_exists

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

        // Promote config.toml from workspace/ to wrapper layer (canonical position)
        // ZeroClaw 运行时期望 config 在 agents/{name}/config.toml，而非 workspace/ 内
        let ws_config = workspace.join("config.toml");
        let wrapper_config = target_dir.join("config.toml");
        if ws_config.exists() && !wrapper_config.exists() {
            tokio::fs::rename(&ws_config, &wrapper_config).await?;
        }

        // Promote icon.svg/icon.png to wrapper layer
        // 优先查看是否有新图标，如果有，覆盖旧图标并清理不同格式的残留
        for icon_name in &["icon.png", "icon.svg"] {
            let ws_icon = workspace.join(icon_name);
            let wrapper_icon = target_dir.join(icon_name);
            if ws_icon.exists() {
                // 如果是提升 PNG，顺便清理旧的 SVG；反之亦然
                let alt_icon = if *icon_name == "icon.png" { "icon.svg" } else { "icon.png" };
                let alt_wrapper_icon = target_dir.join(alt_icon);
                if alt_wrapper_icon.exists() {
                    let _ = tokio::fs::remove_file(&alt_wrapper_icon).await;
                }
                
                // 强制移动(覆盖)
                let _ = tokio::fs::rename(&ws_icon, &wrapper_icon).await;
            }
        }

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
    /// 幂等：如果 target_dir 已存在，只补全缺失文件（config.toml 等），不覆盖已有文件。
    pub async fn create_local_agent(
        &self,
        templates_base_path: &Path, // e.g. `<workspace_dir>/hub/templates/`
        params: &CreateAgentParams,
        progress: &dyn ProgressSink,
    ) -> Result<crate::AgentCreated> {
        let tenant_root = self.resolve_tenant_root(&params.tenant_id);
        let target_dir = tenant_root.join("agents").join(&params.agent_name);
        tracing::info!(
            agent_name = %params.agent_name,
            target_dir = %target_dir.display(),
            tenant_root = %tenant_root.display(),
            "create_local_agent: resolved paths"
        );
        let already_exists = target_dir.exists();
        if already_exists {
            tracing::info!(
                "Agent 目录已存在，将补全缺失文件: {}",
                target_dir.display()
            );
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

        // Promote config.toml from workspace/ to wrapper layer (canonical position)
        // ZeroClaw 运行时期望 config 在 agents/{name}/config.toml，而非 workspace/ 内
        let ws_config = workspace.join("config.toml");
        let wrapper_config = target_dir.join("config.toml");
        tracing::warn!(
            ws_config_exists = ws_config.exists(),
            wrapper_config_exists = wrapper_config.exists(),
            ws_config_path = %ws_config.display(),
            wrapper_config_path = %wrapper_config.display(),
            target_dir = %target_dir.display(),
            workspace = %workspace.display(),
            "PROMOTE DIAGNOSTIC: about to promote config.toml"
        );
        if ws_config.exists() && !wrapper_config.exists() {
            tokio::fs::rename(&ws_config, &wrapper_config).await?;
            tracing::info!("Promoted config.toml to {}", wrapper_config.display());
        }

        // Promote icon.svg/icon.png to wrapper layer
        for icon_name in &["icon.png", "icon.svg"] {
            let ws_icon = workspace.join(icon_name);
            let wrapper_icon = target_dir.join(icon_name);
            if ws_icon.exists() {
                let alt_icon = if *icon_name == "icon.png" { "icon.svg" } else { "icon.png" };
                let alt_wrapper_icon = target_dir.join(alt_icon);
                if alt_wrapper_icon.exists() {
                    let _ = tokio::fs::remove_file(&alt_wrapper_icon).await;
                }
                let _ = tokio::fs::rename(&ws_icon, &wrapper_icon).await;
            }
        }

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
