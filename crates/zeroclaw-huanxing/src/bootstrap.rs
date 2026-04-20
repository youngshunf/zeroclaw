use std::sync::Arc;
use tracing::{error, info, warn};

use crate::context_resolver::MessageContextResolver;
use crate::db::{TenantDb, UserFilter};
use crate::hasn_chat_db::HasnChatDb;
use crate::MultiTenantResolver;
use crate::TenantRouter;
use zeroclaw_config::schema::Config;

/// Initialize all HuanXing multi-tenant systems, skills sync, and context resolver.
/// Returns an overriding ContextResolver if successful, otherwise None (fallback to default resolver).
pub async fn init_tenant_systems(config: &Config) -> Option<Arc<dyn MessageContextResolver>> {
    if !config.huanxing.enabled {
        return None;
    }

    // ── 设备指纹初始化（必须在其他系统之前） ──────────────────────────
    // 如果 config.toml 中尚无 node_id，派生并写入文件。
    // 注意：Config 在此是不可变引用，直接修改文件，内存中的值在下次启动才生效。
    // 对于首次启动场景，我们在内存里也修补一下（通过全局 OnceLock）。
    {
        let fp = crate::device_fingerprint::generate_device_fingerprint();
        let needs_write = config
            .huanxing
            .node_id
            .as_deref()
            .map(|s| s.is_empty())
            .unwrap_or(true);

        if needs_write {
            // 写回文件，供下次启动直接读取
            if let Err(e) = crate::device_fingerprint::persist_node_id_to_config(
                &config.config_path,
                &fp.node_id,
            ) {
                warn!("[DeviceFingerprint] 写入 config.toml 失败（非致命）: {e}");
            } else {
                info!(
                    "[DeviceFingerprint] node_id={} 已写入 config.toml",
                    fp.node_id
                );
            }
        } else {
            info!(
                "[DeviceFingerprint] 已有 node_id={}, 保留不覆盖",
                config.huanxing.node_id.as_deref().unwrap_or("")
            );
        }

        info!(
            "[DeviceFingerprint] fingerprint={} node_id={} node_name={} platform={}",
            fp.fingerprint, fp.node_id, fp.node_name, fp.device_platform
        );

        // 将指纹写入全局（供 hasn_connector 和 api 调用时上报）
        crate::device_fingerprint::set_global_fingerprint(fp);
    }

    // Sync common skills from hub before loading tenant contexts
    if let Some(ref hub_dir) = config.huanxing.hub_dir {
        let common_skills_dir = config
            .huanxing
            .resolve_common_skills_dir(&config.workspace_dir);
        match crate::sync::sync_common_skills(hub_dir, &common_skills_dir).await {
            Ok((added, updated, removed, skipped)) => {
                if added + updated + removed > 0 {
                    info!(
                        added,
                        updated, removed, skipped, "Common skills synced from hub"
                    );
                }
            }
            Err(e) => {
                warn!("Common skills sync failed (non-fatal): {e}");
            }
        }
    }

    // ── Phase 05-05 Task 5b — per-tenant peer_id 历史数据修正 ──
    // 对所有活跃 tenant 尝试调用 run_migration_phase_05_05_peer_id。
    // 迁移幂等，打标 sync_state(key=phase_05_05_peer_id_migration)；失败不阻塞启动。
    if let Err(e) = run_phase_05_05_migrations(config).await {
        warn!("[Phase 05-05] peer_id 迁移扫描失败（非致命）: {e}");
    }

    // Initialize Global Tenant Router
    match TenantRouter::new(
        config.huanxing.clone(),
        config.workspace_dir.clone(),
        Arc::new(config.clone()),
    )
    .await
    {
        Ok(router) => {
            info!("HuanXing multi-tenant routing enabled");
            let router = Arc::new(router);
            // 注册全局 router 供 skill_market_tools 失效缓存使用
            crate::skill_market_tools::register_global_router(Arc::clone(&router));

            // Return MultiTenantResolver to override DefaultContextResolver
            Some(Arc::new(MultiTenantResolver::new(router)) as Arc<dyn MessageContextResolver>)
        }
        Err(e) => {
            error!(
                "Failed to initialize HuanXing tenant router: {e}; falling back to single-tenant"
            );
            None
        }
    }
}

/// Phase 05-05 Task 5b — 扫描所有活跃 tenant 的 hasn_chat.db，
/// 幂等修正 sessions.peer_id 从 Owner 的 h_* 变为对端 Agent 的 a_*。
///
/// 错误处理策略：
/// - 单个 tenant 打开失败或 migration 失败 → warn!，继续扫描下一个
/// - 全局 TenantDb 打开失败 → 向上返错，由调用方降级为 warn（非阻塞）
async fn run_phase_05_05_migrations(config: &Config) -> anyhow::Result<()> {
    let config_dir = config
        .config_path
        .parent()
        .unwrap_or(&config.workspace_dir)
        .to_path_buf();
    let users_db_path = config.huanxing.resolve_db_path(&config_dir);

    // 租户总库不存在就没有迁移目标（全新部署）
    if !users_db_path.exists() {
        info!("[Phase 05-05] users DB 不存在，跳过 peer_id 迁移");
        return Ok(());
    }

    let tenant_db = TenantDb::open(&users_db_path)?;
    let filter = UserFilter {
        limit: Some(500),
        ..Default::default()
    };
    let (tenants, total) = tenant_db.list_users(&filter).await?;
    info!(
        "[Phase 05-05] 开始扫描 peer_id 迁移：tenants={} total={}",
        tenants.len(),
        total
    );

    let mut migrated = 0u64;
    let mut scanned = 0u64;
    for tenant in tenants {
        let Some(ref tenant_dir) = tenant.tenant_dir else {
            continue;
        };
        let tenant_root = config
            .huanxing
            .resolve_tenant_root(&config_dir, Some(tenant_dir.as_str()));
        let chat_db_path = tenant_root.join("data").join("hasn_chat.db");
        if !chat_db_path.exists() {
            continue;
        }
        match HasnChatDb::open(&chat_db_path) {
            Ok(db) => match db.run_migration_phase_05_05_peer_id().await {
                Ok(n) => {
                    scanned += 1;
                    migrated += n;
                    if n > 0 {
                        info!(
                            "[Phase 05-05] tenant={} peer_id 修正 {} 行",
                            tenant_dir, n
                        );
                    }
                }
                Err(e) => warn!(
                    "[Phase 05-05] tenant={} 迁移失败（非致命）: {e}",
                    tenant_dir
                ),
            },
            Err(e) => warn!(
                "[Phase 05-05] tenant={} 打开 hasn_chat.db 失败（非致命）: {e}",
                tenant_dir
            ),
        }
    }

    info!(
        "[Phase 05-05] peer_id 迁移完成：扫描 {} 个 tenant，合计修正 {} 行",
        scanned, migrated
    );
    Ok(())
}
