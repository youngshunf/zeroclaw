//! Skill marketplace tools for user agents.
//!
//! These tools allow users to search, browse, install, uninstall, and manage skills
//! through their Agent's conversation interface. The data source is the local
//! huanxing-hub registry (registry.json + skill directories).

use super::registry::RegistryLoader;
use crate::security::SecurityPolicy;
use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Lazy slot for TenantRouter — set after router is created.
/// Tools are created before the router, so we use OnceLock for deferred init.
pub type RouterSlot = Arc<std::sync::OnceLock<Arc<crate::huanxing::TenantRouter>>>;

/// Create an empty router slot. Call `slot.set(router)` after TenantRouter is created.
pub fn new_router_slot() -> RouterSlot {
    Arc::new(std::sync::OnceLock::new())
}

/// 进程级全局 TenantRouter 引用，由 channels/mod.rs 在初始化后注入。
/// skill 工具通过此引用失效缓存，无需依赖 RouterSlot 的传递链。
static GLOBAL_TENANT_ROUTER: std::sync::OnceLock<Arc<crate::huanxing::TenantRouter>> =
    std::sync::OnceLock::new();

/// 注册全局 TenantRouter（由 channels/mod.rs 在 tenant_router 创建后调用）。
pub fn register_global_router(router: Arc<crate::huanxing::TenantRouter>) {
    let _ = GLOBAL_TENANT_ROUTER.set(router);
}

/// 获取当前 task 的 tenant 安全策略（若已注入）。
/// shell/file 工具调用此函数，优先使用 tenant policy 覆盖全局 policy。
pub fn tenant_security() -> Option<Arc<SecurityPolicy>> {
    crate::tools::get_active_security()
}

/// Invalidate the tenant cache for the current user (best-effort).
fn invalidate_current_user(slot: &RouterSlot, fallback_ws: &Path) {
    // 优先使用全局 router（channels 里的那个，有实际消息缓存）
    let router = GLOBAL_TENANT_ROUTER
        .get()
        .cloned()
        .or_else(|| slot.get().cloned());
    if let Some(router) = router {
        let ws = tenant_workspace(fallback_ws);
        if let Some(agent_id) = ws.file_name().and_then(|n| n.to_str()) {
            router.invalidate_agent(agent_id);
            tracing::debug!(agent_id, "Tenant cache invalidated after skill change");
        }
    }
}

/// Get the effective workspace dir for the current tenant.
/// 多租户模式下从 task-local ACTIVE_WORKSPACE 读取 per-tenant 目录；
/// 非多租户模式下（CLI/单机）回退到工具构建时传入的 workspace_dir（agents_dir）。
fn tenant_workspace(fallback: &Path) -> PathBuf {
    crate::tools::get_active_workspace().unwrap_or_else(|| fallback.to_path_buf())
}

/// Format a risk level with emoji.
fn risk_emoji(level: &str) -> &str {
    match level {
        "safe" => "🟢 safe",
        "moderate" => "🟡 moderate",
        "elevated" => "🟠 elevated",
        "dangerous" => "🔴 dangerous",
        _ => level,
    }
}

/// Format a review status with emoji.
fn review_emoji(status: &str) -> &str {
    match status {
        "official" => "✅ official",
        "verified" => "🟢 verified",
        "community" => "🟡 community",
        "unreviewed" => "⚠️ unreviewed",
        _ => status,
    }
}

/// Copy a directory recursively.
async fn copy_dir_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(dest).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dest_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dest_path).await?;
        }
    }
    Ok(())
}

/// Read version from a skill's manifest.yaml.
async fn read_manifest_version(skill_dir: &Path) -> Option<String> {
    let manifest = skill_dir.join("manifest.yaml");
    let content = tokio::fs::read_to_string(&manifest).await.ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("version:") {
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Read name from a skill's manifest.yaml.
async fn read_manifest_name(skill_dir: &Path) -> Option<String> {
    let manifest = skill_dir.join("manifest.yaml");
    let content = tokio::fs::read_to_string(&manifest).await.ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let n = rest.trim().trim_matches('"').trim_matches('\'');
            if !n.is_empty() {
                return Some(n.to_string());
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════
// hx_skill_search — Search the skill marketplace
// ═══════════════════════════════════════════════════════

pub struct HxSkillSearch {
    pub registry: Arc<RegistryLoader>,
    pub workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for HxSkillSearch {
    fn name(&self) -> &str {
        "hx_skill_search"
    }

    fn description(&self) -> &str {
        "搜索技能市场，寻找当前 Agent 尚未拥有的新技能。仅在用户明确要求获取新能力、且该技能不在当前可用技能列表中时才调用。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词（匹配名称、描述、标签）"
                },
                "category": {
                    "type": "string",
                    "description": "按分类过滤: productivity, finance, health, creative, developer, data, social, search, utility"
                },
                "limit": {
                    "type": "integer",
                    "description": "返回数量上限，默认 10"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let ws = tenant_workspace(&self.workspace_dir);
        eprintln!(
            "  🔍 SKILL-SEARCH args={args} fallback_ws={} effective_ws={}",
            self.workspace_dir.display(),
            ws.display()
        );

        if let Err(e) = self.registry.ensure_loaded().await {
            eprintln!("  🔍 SKILL-SEARCH registry load failed: {e}");
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("无法加载技能索引: {e}")),
            });
        }

        let query = args["query"].as_str().unwrap_or("");
        let category = args.get("category").and_then(|v| v.as_str());
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.registry.search(query, category, limit).await;
        eprintln!(
            "  🔍 SKILL-SEARCH query={query:?} category={category:?} results={}",
            results.len()
        );
        let skills_dir = ws.join("skills");

        let items: Vec<Value> = results
            .iter()
            .map(|s| {
                let is_installed = skills_dir.join(&s.id).exists();
                json!({
                    "id": s.id,
                    "name": s.name,
                    "version": s.version,
                    "description": s.description,
                    "category": s.category,
                    "risk_level": risk_emoji(&s.risk_level),
                    "review_status": review_emoji(&s.review_status),
                    "pricing": s.pricing_tier,
                    "has_scripts": s.has_scripts,
                    "installed": is_installed,
                })
            })
            .collect();

        Ok(ToolResult {
            success: true,
            output: json!({
                "total": items.len(),
                "results": items,
            })
            .to_string(),
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_skill_info — Get skill details
// ═══════════════════════════════════════════════════════

pub struct HxSkillInfo {
    pub registry: Arc<RegistryLoader>,
    pub workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for HxSkillInfo {
    fn name(&self) -> &str {
        "hx_skill_info"
    }

    fn description(&self) -> &str {
        "查看技能详细信息，包含描述、依赖、权限、工具列表等。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": {
                    "type": "string",
                    "description": "技能 ID"
                }
            },
            "required": ["skill_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if let Err(e) = self.registry.ensure_loaded().await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("无法加载技能索引: {e}")),
            });
        }

        let skill_id = match args["skill_id"].as_str() {
            Some(id) => id,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("缺少 skill_id 参数".to_string()),
                })
            }
        };

        let entry = match self.registry.find_skill(skill_id).await {
            Some(e) => e,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("技能 '{skill_id}' 不存在")),
                })
            }
        };

        let ws = tenant_workspace(&self.workspace_dir);
        let skill_dir = ws.join("skills").join(skill_id);
        let installed_version = if skill_dir.exists() {
            read_manifest_version(&skill_dir).await
        } else {
            None
        };

        // Try to read SKILL.md for full description
        let full_desc = if let Some(dir) = self.registry.skill_dir(skill_id).await {
            let skill_md = dir.join("SKILL.md");
            if skill_md.exists() {
                tokio::fs::read_to_string(&skill_md).await.ok()
            } else {
                None
            }
        } else {
            None
        };

        Ok(ToolResult {
            success: true,
            output: json!({
                "id": entry.id,
                "name": entry.name,
                "version": entry.version,
                "author": entry.author,
                "description": entry.description,
                "full_description": full_desc,
                "category": entry.category,
                "subcategory": entry.subcategory,
                "tags": entry.tags,
                "platforms": entry.platforms,
                "risk_level": risk_emoji(&entry.risk_level),
                "review_status": review_emoji(&entry.review_status),
                "pricing": entry.pricing_tier,
                "requires_api_keys": entry.requires_api_keys,
                "requires_permissions": entry.requires_permissions,
                "has_scripts": entry.has_scripts,
                "file_count": entry.file_count,
                "size_bytes": entry.size_bytes,
                "installed": installed_version.is_some(),
                "installed_version": installed_version,
            })
            .to_string(),
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_skill_install — Install a skill
// ═══════════════════════════════════════════════════════

pub struct HxSkillInstall {
    pub registry: Arc<RegistryLoader>,
    pub workspace_dir: PathBuf,
    pub router_slot: RouterSlot,
}

#[async_trait]
impl Tool for HxSkillInstall {
    fn name(&self) -> &str {
        "hx_skill_install"
    }

    fn description(&self) -> &str {
        "安装一个新技能到当前 Agent。仅在用户明确要求获取新能力、且该技能不在当前可用技能列表中时才调用。技能将在下次对话中生效。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": {
                    "type": "string",
                    "description": "要安装的技能 ID"
                },
                "accept_risk": {
                    "type": "boolean",
                    "description": "是否确认接受风险（elevated/dangerous 等级需要）"
                }
            },
            "required": ["skill_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if let Err(e) = self.registry.ensure_loaded().await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("无法加载技能索引: {e}")),
            });
        }

        let skill_id = match args["skill_id"].as_str() {
            Some(id) => id,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("缺少 skill_id 参数".to_string()),
                })
            }
        };
        let accept_risk = args
            .get("accept_risk")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 1. Find skill in registry
        let entry = match self.registry.find_skill(skill_id).await {
            Some(e) => e,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("技能 '{skill_id}' 不存在")),
                })
            }
        };

        // 2. Check if already installed (by checking directory + manifest version)
        let ws = tenant_workspace(&self.workspace_dir);
        let dest_dir = ws.join("skills").join(skill_id);
        if dest_dir.exists() {
            let current_ver = read_manifest_version(&dest_dir).await.unwrap_or_default();
            if current_ver == entry.version {
                return Ok(ToolResult {
                    success: true,
                    output: format!(
                        "技能 '{}' v{} 已安装，无需重复安装。",
                        entry.name, entry.version
                    ),
                    error: None,
                });
            }
        }

        // 3. Risk check — elevated/dangerous require explicit accept_risk
        if (entry.risk_level == "elevated" || entry.risk_level == "dangerous") && !accept_risk {
            return Ok(ToolResult {
                success: true,
                output: json!({
                    "requires_confirmation": true,
                    "message": format!(
                        "⚠️ 技能 '{}' 风险等级: {}\n审核状态: {}\n需要权限: {:?}\n\n请确认是否安装？（设置 accept_risk=true）",
                        entry.name,
                        risk_emoji(&entry.risk_level),
                        review_emoji(&entry.review_status),
                        entry.requires_permissions
                    )
                })
                .to_string(),
                error: None,
            });
        }

        // 3b. API key requirement hint (non-blocking)
        let mut api_key_hint: Option<String> = None;
        if entry.requires_api_keys {
            // Try to read manifest.yaml for detailed key info
            if let Some(ref src) = self.registry.skill_dir(skill_id).await {
                let manifest_path = src.join("manifest.yaml");
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    // Simple extraction: find "key:" lines under requires.api_keys
                    let key_names: Vec<String> = content
                        .lines()
                        .filter(|l| l.trim().starts_with("- key:"))
                        .filter_map(|l| l.split("\"").nth(1).map(|s| s.to_string()))
                        .collect();
                    if !key_names.is_empty() {
                        let ws = tenant_workspace(&self.workspace_dir);
                        let env_content =
                            std::fs::read_to_string(ws.join(".env")).unwrap_or_default();
                        let missing: Vec<&str> = key_names
                            .iter()
                            .filter(|k| !env_content.contains(k.as_str()))
                            .map(|k| k.as_str())
                            .collect();
                        if !missing.is_empty() {
                            api_key_hint = Some(format!(
                                "\n\n🔑 需要配置 API Key:\n{}\n\n💡 告诉我：\"帮我设置 {} 为 xxx\"",
                                missing
                                    .iter()
                                    .map(|k| format!("  • {}", k))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                missing[0]
                            ));
                        }
                    }
                }
            }
        }

        // 4. Find skill directory in hub
        let src_dir = match self.registry.skill_dir(skill_id).await {
            Some(dir) => dir,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("技能 '{}' 的文件在 hub 仓库中未找到", skill_id)),
                })
            }
        };

        // 5. Copy to workspace/skills/
        let dest_dir = tenant_workspace(&self.workspace_dir)
            .join("skills")
            .join(skill_id);
        if dest_dir.exists() {
            // Backup old version
            let backup = tenant_workspace(&self.workspace_dir)
                .join(".trash")
                .join(format!(
                    "{}-{}",
                    skill_id,
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                ));
            if let Err(e) = tokio::fs::create_dir_all(backup.parent().unwrap()).await {
                tracing::warn!("Failed to create backup dir: {e}");
            }
            if let Err(e) = tokio::fs::rename(&dest_dir, &backup).await {
                tracing::warn!("Failed to backup old skill: {e}");
                let _ = tokio::fs::remove_dir_all(&dest_dir).await;
            }
        }

        if let Err(e) = copy_dir_recursive(&src_dir, &dest_dir).await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("安装失败: {e}")),
            });
        }

        // 6. Security audit
        match crate::skills::audit::audit_skill_directory(&dest_dir) {
            Ok(warnings) => {
                if !warnings.findings.is_empty() {
                    tracing::warn!(
                        skill = skill_id,
                        "Skill audit warnings: {:?}",
                        warnings.findings
                    );
                }
            }
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&dest_dir).await;
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("安全审计未通过，安装已取消: {e}")),
                });
            }
        }

        let mut msg = format!(
            "✅ {} v{} 安装成功！\n📋 风险等级: {} | 审核: {}",
            entry.name,
            entry.version,
            risk_emoji(&entry.risk_level),
            review_emoji(&entry.review_status),
        );

        // Invalidate tenant cache so the new skill loads immediately
        invalidate_current_user(&self.router_slot, &self.workspace_dir);

        // Append API key hint if any
        if let Some(hint) = api_key_hint {
            msg.push_str(&hint);
        }

        Ok(ToolResult {
            success: true,
            output: msg,
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_skill_uninstall — Uninstall a skill
// ═══════════════════════════════════════════════════════

pub struct HxSkillUninstall {
    pub workspace_dir: PathBuf,
    pub router_slot: RouterSlot,
}

#[async_trait]
impl Tool for HxSkillUninstall {
    fn name(&self) -> &str {
        "hx_skill_uninstall"
    }

    fn description(&self) -> &str {
        "卸载一个已安装的技能。技能文件将移到回收站（可恢复）。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": {
                    "type": "string",
                    "description": "要卸载的技能 ID"
                }
            },
            "required": ["skill_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let skill_id = match args["skill_id"].as_str() {
            Some(id) => id,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("缺少 skill_id 参数".to_string()),
                })
            }
        };

        let skill_dir = tenant_workspace(&self.workspace_dir)
            .join("skills")
            .join(skill_id);
        if !skill_dir.exists() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("技能 '{}' 未安装", skill_id)),
            });
        }

        // Move to .trash (recoverable)
        let trash_dir = tenant_workspace(&self.workspace_dir).join(".trash");
        let trash_dest = trash_dir.join(format!(
            "{}-{}",
            skill_id,
            chrono::Utc::now().format("%Y%m%d%H%M%S")
        ));
        if let Err(e) = tokio::fs::create_dir_all(&trash_dir).await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("创建回收站失败: {e}")),
            });
        }
        if let Err(e) = tokio::fs::rename(&skill_dir, &trash_dest).await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("卸载失败: {e}")),
            });
        }

        // Invalidate tenant cache so skill unloads immediately
        invalidate_current_user(&self.router_slot, &self.workspace_dir);

        Ok(ToolResult {
            success: true,
            output: format!("✅ {} 已卸载（文件已移到回收站，可恢复）。", skill_id),
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_skill_list — List installed skills
// ═══════════════════════════════════════════════════════

pub struct HxSkillList {
    pub registry: Arc<RegistryLoader>,
    pub workspace_dir: PathBuf,
    /// 公共技能目录（可选），用于在列表中显示公共技能
    pub common_skills_dir: Option<PathBuf>,
}

#[async_trait]
impl Tool for HxSkillList {
    fn name(&self) -> &str {
        "hx_skill_list"
    }

    fn description(&self) -> &str {
        "列出当前 Agent 已安装的私有技能及平台公共技能。注意：系统提示词中 <available_skills> 列出的技能均已可用，通常无需调用此工具。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        let ws = tenant_workspace(&self.workspace_dir);
        eprintln!(
            "  🔍 SKILL-LIST fallback_ws={} effective_ws={}",
            self.workspace_dir.display(),
            ws.display()
        );

        let _ = self.registry.ensure_loaded().await;

        // 收集私有技能 ID（来自 agent workspace）
        let private_skills_dir = ws.join("skills");
        let mut private_skill_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        if private_skills_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&private_skills_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.path().is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if !name.starts_with('.') {
                            private_skill_ids.insert(name);
                        }
                    }
                }
            }
        }

        // 收集公共技能 ID（来自 common_skills_dir，私有技能同名时跳过）
        let mut common_skill_ids: Vec<String> = Vec::new();
        if let Some(ref common_dir) = self.common_skills_dir {
            let common_skills_dir = common_dir.join("skills");
            if common_skills_dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&common_skills_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry.path().is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !name.starts_with('.') && !private_skill_ids.contains(&name) {
                                common_skill_ids.push(name);
                            }
                        }
                    }
                }
            }
        }

        let mut items: Vec<Value> = Vec::new();

        // 私有技能
        for skill_id in &private_skill_ids {
            let skill_path = private_skills_dir.join(skill_id);
            let installed_ver = read_manifest_version(&skill_path)
                .await
                .unwrap_or_else(|| "unknown".to_string());

            let (registry_name, has_update, latest_ver) =
                if let Some(entry) = self.registry.find_skill(skill_id).await {
                    let has_update = entry.version != installed_ver;
                    (
                        entry.name.clone(),
                        has_update,
                        if has_update {
                            Some(entry.version.clone())
                        } else {
                            None
                        },
                    )
                } else {
                    let name = read_manifest_name(&skill_path)
                        .await
                        .unwrap_or_else(|| skill_id.clone());
                    (name, false, None)
                };

            items.push(json!({
                "id": skill_id,
                "name": registry_name,
                "version": installed_ver,
                "has_update": has_update,
                "latest_version": latest_ver,
                "source": "private",
            }));
        }

        // 公共技能（标记 source=common，已可直接使用，无需安装）
        for skill_id in &common_skill_ids {
            let skill_path = self.common_skills_dir.as_ref().unwrap().join("skills").join(skill_id);
            let installed_ver = read_manifest_version(&skill_path)
                .await
                .unwrap_or_else(|| "unknown".to_string());

            let registry_name = if let Some(entry) = self.registry.find_skill(skill_id).await {
                entry.name.clone()
            } else {
                read_manifest_name(&skill_path)
                    .await
                    .unwrap_or_else(|| skill_id.clone())
            };

            items.push(json!({
                "id": skill_id,
                "name": registry_name,
                "version": installed_ver,
                "has_update": false,
                "latest_version": null,
                "source": "common",
            }));
        }

        items.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));

        Ok(ToolResult {
            success: true,
            output: json!({
                "total": items.len(),
                "skills": items,
            })
            .to_string(),
            error: None,
        })
    }
}

// ═══════════════════════════════════════════════════════
// hx_skill_update — Update installed skills
// ═══════════════════════════════════════════════════════

pub struct HxSkillUpdate {
    pub registry: Arc<RegistryLoader>,
    pub workspace_dir: PathBuf,
    pub router_slot: RouterSlot,
}

#[async_trait]
impl Tool for HxSkillUpdate {
    fn name(&self) -> &str {
        "hx_skill_update"
    }

    fn description(&self) -> &str {
        "检查并更新已安装的技能。可指定单个技能或检查全部。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "skill_id": {
                    "type": "string",
                    "description": "指定更新的技能 ID（不传则检查全部）"
                },
                "update_all": {
                    "type": "boolean",
                    "description": "是否更新所有可更新的技能"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if let Err(e) = self.registry.ensure_loaded().await {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("无法加载技能索引: {e}")),
            });
        }

        let ws = tenant_workspace(&self.workspace_dir);
        let skills_dir = ws.join("skills");
        let skill_id = args.get("skill_id").and_then(|v| v.as_str());
        let update_all = args
            .get("update_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut updates: Vec<Value> = Vec::new();
        let mut up_to_date: Vec<String> = Vec::new();

        // Collect installed skill IDs from directory
        let skill_ids: Vec<String> = if let Some(id) = skill_id {
            vec![id.to_string()]
        } else {
            let mut ids = Vec::new();
            if skills_dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(&skills_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry.path().is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !name.starts_with('.') {
                                ids.push(name);
                            }
                        }
                    }
                }
            }
            ids
        };

        for sid in &skill_ids {
            let skill_path = skills_dir.join(sid);
            let current_ver = read_manifest_version(&skill_path)
                .await
                .unwrap_or_else(|| "0.0.0".to_string());
            if let Some(entry) = self.registry.find_skill(sid).await {
                if entry.version != current_ver {
                    updates.push(json!({
                        "id": sid,
                        "name": entry.name,
                        "current_version": current_ver,
                        "latest_version": entry.version,
                        "size_bytes": entry.size_bytes,
                    }));
                } else {
                    up_to_date.push(sid.clone());
                }
            }
        }

        if updates.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "所有技能都已是最新版本 ✅".to_string(),
                error: None,
            });
        }

        if !update_all && skill_id.is_none() {
            return Ok(ToolResult {
                success: true,
                output: json!({
                    "updates_available": updates.len(),
                    "updates": updates,
                    "up_to_date": up_to_date.len(),
                    "message": format!("有 {} 个技能可更新。使用 update_all=true 或指定 skill_id 来更新。", updates.len()),
                })
                .to_string(),
                error: None,
            });
        }

        // Perform updates
        let mut updated = Vec::new();
        let mut failed = Vec::new();

        for update in &updates {
            let sid = update["id"].as_str().unwrap();
            if let Some(src_dir) = self.registry.skill_dir(sid).await {
                let dest_dir = tenant_workspace(&self.workspace_dir)
                    .join("skills")
                    .join(sid);

                // Backup old
                let backup = tenant_workspace(&self.workspace_dir)
                    .join(".trash")
                    .join(format!(
                        "{}-{}",
                        sid,
                        chrono::Utc::now().format("%Y%m%d%H%M%S")
                    ));
                let _ = tokio::fs::create_dir_all(backup.parent().unwrap()).await;
                let _ = tokio::fs::rename(&dest_dir, &backup).await;

                match copy_dir_recursive(&src_dir, &dest_dir).await {
                    Ok(()) => {
                        updated.push(sid.to_string());
                    }
                    Err(e) => {
                        let _ = tokio::fs::rename(&backup, &dest_dir).await;
                        failed.push(format!("{}: {}", sid, e));
                    }
                }
            }
        }

        // Invalidate tenant cache if any updates were applied
        if !updated.is_empty() {
            invalidate_current_user(&self.router_slot, &self.workspace_dir);
        }

        let msg = if failed.is_empty() {
            format!(
                "✅ 已更新 {} 个技能: {}。",
                updated.len(),
                updated.join(", ")
            )
        } else {
            format!(
                "更新完成: {} 成功, {} 失败\n成功: {}\n失败: {}",
                updated.len(),
                failed.len(),
                updated.join(", "),
                failed.join(", ")
            )
        };

        Ok(ToolResult {
            success: true,
            output: msg,
            error: None,
        })
    }
}
