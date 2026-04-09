//! Centralized HuanXing tool registration.
//!
//! All HuanXing tool instantiation lives here, keeping `src/tools/mod.rs`
//! free of `#[cfg(feature = "huanxing")]` blocks (per 唤星开发规范 Rule 1).

use std::sync::Arc;

use crate::config::Config;
use crate::security::SecurityPolicy;
use crate::tools::Tool;

/// Build and return all HuanXing tools.
///
/// Called once from `src/tools/mod.rs` behind a single `#[cfg(feature = "huanxing")]` gate.
/// Internally handles DB init, TenantRouter creation, Hub Registry loading,
/// and every tool group's instantiation.
pub fn huanxing_all_tools(
    root_config: &Config,
    security: Arc<SecurityPolicy>,
    workspace_dir: &std::path::Path,
) -> Vec<Arc<dyn Tool>> {
    let mut tool_arcs: Vec<Arc<dyn Tool>> = Vec::new();

    let config_dir = root_config
        .config_path
        .parent()
        .unwrap_or(&root_config.workspace_dir);
    let hx_db_path = root_config.huanxing.resolve_db_path(config_dir);

    let hx_db = match super::TenantDb::open(&hx_db_path) {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!(
                "HuanXing enabled but failed to open DB at {}: {e}",
                hx_db_path.display()
            );
            return tool_arcs;
        }
    };

    // ── DB-only tools (no API/router dependency) ─────────────────
    tool_arcs.push(Arc::new(super::tools::HxLookupSender::new(hx_db.clone())));
    tool_arcs.push(Arc::new(super::tools::HxGetUser::new(hx_db.clone())));
    tool_arcs.push(Arc::new(super::tools::HxLocalFindUser::new(hx_db.clone())));
    tool_arcs.push(Arc::new(super::tools::HxLocalStats::new(hx_db.clone())));

    // ── Image generation tool ────────────────────────────────────
    if root_config.huanxing.hx_image_gen.enabled {
        let api_key = root_config
            .huanxing
            .hx_image_gen
            .api_key
            .clone()
            .or_else(|| root_config.api_key.clone())
            .unwrap_or_default();

        let api_url = root_config
            .huanxing
            .hx_image_gen
            .api_url
            .clone()
            .unwrap_or_else(|| {
                root_config
                    .api_url
                    .clone()
                    .map(|u| format!("{}/images/generations", u.trim_end_matches('/')))
                    .unwrap_or_else(|| "https://api.openai.com/v1/images/generations".to_string())
            });

        tool_arcs.push(Arc::new(super::hx_image_gen::HxImageGenTool::new(
            security.clone(),
            workspace_dir.to_path_buf(),
            root_config.huanxing.hx_image_gen.models.clone(),
            api_url,
            api_key,
        )));
    }

    // ── API-dependent tools (SMS, quota, subscription, etc.) ─────
    let hx_api = if let Some(ref key) = root_config.huanxing.agent_key {
        let api = super::ApiClient::new(
            root_config.huanxing.api_url(),
            key,
            &root_config.huanxing.node_id_or_hostname(),
        );
        tool_arcs.extend(super::tools::huanxing_api_tools(
            api.clone(),
            hx_db.clone(),
            workspace_dir.to_path_buf(),
            root_config.huanxing.owner_key.clone().unwrap_or_default(),
        ));
        tracing::info!(
            "HuanXing API tools registered (sms, quota, subscription, usage, file_upload, website_deploy)"
        );
        Some(api)
    } else {
        tracing::info!("HuanXing agent_key not configured, API tools skipped");
        None
    };

    let hx_api_for_register = hx_api.clone();

    // ── TenantRouter + router-dependent tools ────────────────────
    let hx_config = root_config.huanxing.clone();
    let ws_dir: std::path::PathBuf = root_config.workspace_dir.clone();
    let router_result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
            tokio::task::block_in_place(|| {
                handle.block_on(super::TenantRouter::new(
                    hx_config.clone(),
                    ws_dir.clone(),
                    Arc::new(root_config.clone()),
                ))
            })
        } else {
            let hx = hx_config.clone();
            let wd = ws_dir.clone();
            let rc = Arc::new(root_config.clone());
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(super::TenantRouter::new(hx, wd, rc))
            })
            .join()
            .unwrap()
        }
    } else {
        let hx = hx_config.clone();
        let wd = ws_dir.clone();
        let rc = Arc::new(root_config.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(super::TenantRouter::new(hx, wd, rc))
        })
        .join()
        .unwrap()
    };

    let router = match router_result {
        Ok(r) => Arc::new(r),
        Err(e) => {
            tracing::warn!("HuanXing router init failed, only lookup tools available: {e}");
            return tool_arcs;
        }
    };

    // Resolve common paths
    let common_skills_dir = root_config
        .huanxing
        .resolve_common_skills_dir(&root_config.workspace_dir);
    let templates_dir = root_config
        .huanxing
        .resolve_templates_dir(&root_config.workspace_dir);
    let default_template = root_config
        .huanxing
        .default_template
        .clone()
        .unwrap_or_else(|| "finance".to_string());
    let default_provider = root_config.huanxing.default_provider.clone();
    let llm_base_url = root_config.huanxing.llm_base_url.clone();
    let node_id = root_config
        .huanxing
        .node_id
        .clone()
        .unwrap_or_else(|| "local-dev".to_string());

    // ── Hub Registry ─────────────────────────────────────────────
    let hub_registry: Option<Arc<super::registry::RegistryLoader>> =
        root_config.huanxing.resolve_hub_dir().and_then(|hub_dir| {
            if hub_dir.exists() && hub_dir.join("registry.json").exists() {
                let registry = Arc::new(super::registry::RegistryLoader::new(hub_dir));
                let reg_clone = registry.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                        let _ = tokio::task::block_in_place(|| {
                            handle.block_on(reg_clone.ensure_loaded())
                        });
                    } else {
                        let _ = std::thread::spawn(move || {
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build()
                                .unwrap();
                            rt.block_on(reg_clone.ensure_loaded())
                        })
                        .join();
                    }
                } else {
                    let _ = std::thread::spawn(move || {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap();
                        rt.block_on(reg_clone.ensure_loaded())
                    })
                    .join();
                }
                Some(registry)
            } else {
                None
            }
        });

    // ── Register user tool (with optional hub registry) ──────────
    if let Some(ref api) = hx_api_for_register {
        let register_tool = if let Some(ref registry) = hub_registry {
            super::tools::HxRegisterUser::with_registry(
                hx_db.clone(),
                api.clone(),
                root_config
                    .config_path
                    .parent()
                    .unwrap_or(&root_config.workspace_dir)
                    .to_path_buf(),
                hx_config.clone(),
                common_skills_dir.clone(),
                templates_dir.clone(),
                default_template.clone(),
                default_provider.clone(),
                llm_base_url.clone(),
                node_id.clone(),
                router.clone(),
                registry.clone(),
            )
        } else {
            super::tools::HxRegisterUser::new(
                hx_db.clone(),
                api.clone(),
                root_config
                    .config_path
                    .parent()
                    .unwrap_or(&root_config.workspace_dir)
                    .to_path_buf(),
                hx_config.clone(),
                common_skills_dir.clone(),
                templates_dir.clone(),
                default_template.clone(),
                default_provider.clone(),
                llm_base_url.clone(),
                node_id.clone(),
                router.clone(),
            )
        };
        tool_arcs.push(Arc::new(register_tool));
    }

    // ── Tenant management tools ──────────────────────────────────
    tool_arcs.push(Arc::new(super::tools::HxLocalBindChannel::new(
        hx_db.clone(),
        router.clone(),
    )));
    tool_arcs.push(Arc::new(super::tools::HxLocalUpdateUser::new(
        hx_db.clone(),
        router.clone(),
    )));
    tool_arcs.push(Arc::new(super::tools::HxLocalListUsers::new(hx_db.clone())));
    tool_arcs.push(Arc::new(super::tools::HxInvalidateCache::new(
        router.clone(),
    )));
    tracing::info!("HuanXing tools registered (all P0+P1: 14 tools)");

    // Server lifecycle: register + heartbeat
    router.start_server_lifecycle();

    // ── Document tools (11) ──────────────────────────────────────
    // Requires Owner Key (hasn_ok_xxx) for user-level API authentication.
    // Cascade read:
    //   1. [huanxing].owner_key in global config.toml (cloud deployments)
    //   2. top-level owner_key in Owner config.toml (~/.huanxing/users/{tenant}/config.toml)
    let effective_owner_key = root_config
        .huanxing
        .owner_key
        .clone()
        .or_else(|| {
            // Desktop fallback: read from first tenant's Owner config.toml
            let config_dir = root_config
                .config_path
                .parent()
                .unwrap_or(&root_config.workspace_dir);
            // Look up the first tenant directory from users.db
            let td: Option<String> = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async {
                        hx_db.get_first_tenant_dir().await.ok().flatten()
                    })
            });
            if let Some(tenant_dir) = td.as_deref() {
                let owner_config_path = hx_config
                    .resolve_tenant_root(config_dir, Some(tenant_dir))
                    .join("config.toml");
                if let Ok(content) = std::fs::read_to_string(&owner_config_path) {
                    if let Ok(table) = content.parse::<toml::Table>() {
                        if let Some(toml::Value::String(ok)) = table.get("owner_key") {
                            if !ok.is_empty() {
                                tracing::info!(
                                    "Found owner_key in Owner config: {}",
                                    owner_config_path.display()
                                );
                                return Some(ok.clone());
                            }
                        }
                    }
                }
            }
            None
        });
    if let (Some(api), Some(owner_key)) = (&hx_api, &effective_owner_key) {
        let ok = owner_key.clone();
        tool_arcs.push(Arc::new(super::doc_tools::HxFolderTree::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxFolderCreate::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxFolderDelete::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxFolderMove::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocList::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocGet::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocCreate::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocUpdate::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocDelete::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocMove::new(
            api.clone(),
            ok.clone(),
        )));
        tool_arcs.push(Arc::new(super::doc_tools::HxDocShare::new(
            api.clone(),
            ok,
        )));
        tracing::info!("HuanXing document tools registered (11 tools, Owner Key auth)");
    } else if hx_api.is_some() {
        tracing::info!("HuanXing owner_key not configured, document tools skipped (set owner_key in Owner config.toml or huanxing.owner_key globally)");
    }

    // ── HASN social tools (5) ────────────────────────────────────
    {
        let hasn_url = root_config.huanxing.hasn_url().to_string();
        let fallback_workspace = root_config.workspace_dir.clone();
        tool_arcs.push(Arc::new(super::hasn_tools::HasnSend::new(
            hx_api.clone().unwrap_or_else(|| {
                super::ApiClient::new(
                    root_config.huanxing.api_url(),
                    "",
                    &root_config.huanxing.node_id_or_hostname(),
                )
            }),
            fallback_workspace.clone(),
            hasn_url.clone(),
        )));
        tool_arcs.push(Arc::new(super::hasn_tools::HasnContacts::new(
            fallback_workspace.clone(),
            hasn_url.clone(),
        )));
        tool_arcs.push(Arc::new(super::hasn_tools::HasnAddFriend::new(
            fallback_workspace.clone(),
            hasn_url.clone(),
        )));
        tool_arcs.push(Arc::new(super::hasn_tools::HasnInbox::new(
            fallback_workspace.clone(),
            hasn_url.clone(),
        )));
        tool_arcs.push(Arc::new(super::hasn_tools::HasnRespondRequest::new(
            fallback_workspace,
            hasn_url,
        )));
        tracing::info!("HuanXing HASN social tools registered (5 tools)");
    }

    // ── Skill marketplace tools (6) ──────────────────────────────
    if let Some(ref registry) = hub_registry {
        let fallback_workspace = root_config.workspace_dir.clone();
        let router_slot = super::skill_market_tools::new_router_slot();
        // Inject TenantRouter into slot for cache invalidation after skill install/uninstall
        let _ = router_slot.set(Arc::clone(&router));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillSearch {
            registry: registry.clone(),
            workspace_dir: fallback_workspace.clone(),
        }));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillInfo {
            registry: registry.clone(),
            workspace_dir: fallback_workspace.clone(),
        }));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillInstall {
            registry: registry.clone(),
            workspace_dir: fallback_workspace.clone(),
            router_slot: router_slot.clone(),
        }));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillUninstall {
            workspace_dir: fallback_workspace.clone(),
            router_slot: router_slot.clone(),
        }));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillList {
            registry: registry.clone(),
            workspace_dir: fallback_workspace.clone(),
            common_skills_dir: if common_skills_dir.exists() {
                Some(common_skills_dir.clone())
            } else {
                None
            },
        }));
        tool_arcs.push(Arc::new(super::skill_market_tools::HxSkillUpdate {
            registry: registry.clone(),
            workspace_dir: fallback_workspace,
            router_slot: router_slot.clone(),
        }));
        tracing::info!("HuanXing skill marketplace tools registered (6 tools)");
    }

    // ── Secret management tools (3) ──────────────────────────────
    {
        let fallback_workspace = root_config.workspace_dir.clone();
        tool_arcs.push(Arc::new(super::secret_tools::HxSetSecret {
            workspace_dir: fallback_workspace.clone(),
        }));
        tool_arcs.push(Arc::new(super::secret_tools::HxListSecrets {
            workspace_dir: fallback_workspace.clone(),
        }));
        tool_arcs.push(Arc::new(super::secret_tools::HxDeleteSecret {
            workspace_dir: fallback_workspace,
        }));
        tracing::info!("HuanXing secret management tools registered (3 tools)");
    }

    // ── TTS voice tool (1) ───────────────────────────────────────
    if root_config.tts.enabled {
        let mut tts_config = root_config.tts.clone();

        // 多租户模式下，如果 TTS 未配置独立的 api_key，默认使用用户的 LLM 主 api_key
        if let Some(ref mut generic) = tts_config.generic_openai {
            if generic.api_key.as_deref().unwrap_or("").trim().is_empty() {
                generic.api_key = root_config.api_key.clone();
            }
        }

        tool_arcs.push(Arc::new(super::tools::HxTts::new(
            tts_config,
            root_config.workspace_dir.clone(),
        )));
        tracing::info!("HuanXing TTS tool registered (hx_tts)");
    }

    // ── Enhanced web search tool (replaces upstream WebSearchTool) ──
    if root_config.web_search.enabled {
        tool_arcs.push(Arc::new(
            super::hx_web_search::HxWebSearchTool::new_with_options(
                security.clone(),
                root_config.web_search.provider.clone(),
                root_config.web_search.api_key.clone(),
                root_config.web_search.brave_api_key.clone(),
                root_config.web_search.perplexity_api_key.clone(),
                root_config.web_search.exa_api_key.clone(),
                root_config.web_search.jina_api_key.clone(),
                root_config.web_search.searxng_instance_url.clone(),
                root_config.web_search.api_url.clone(),
                root_config.web_search.max_results,
                root_config.web_search.timeout_secs,
                root_config.web_search.user_agent.clone(),
                root_config.web_search.fallback_providers.clone(),
                root_config.web_search.retries_per_provider,
                root_config.web_search.retry_backoff_ms,
                root_config.web_search.domain_filter.clone(),
                root_config.web_search.language_filter.clone(),
                root_config.web_search.country.clone(),
                root_config.web_search.recency_filter.clone(),
                root_config.web_search.max_tokens,
                root_config.web_search.max_tokens_per_page,
                root_config.web_search.exa_search_type.clone(),
                root_config.web_search.exa_include_text,
                root_config.web_search.jina_site_filters.clone(),
            ),
        ));
        tracing::info!(
            "HuanXing enhanced web search tool registered (firecrawl/tavily/multi-provider)"
        );
    }

    tool_arcs
}
