use std::sync::Arc;
use tracing::{error, info, warn};

use crate::channels::context_resolver::MessageContextResolver;
use crate::config::Config;
use crate::huanxing::MultiTenantResolver;
use crate::huanxing::TenantRouter;

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
        let fp = crate::huanxing::device_fingerprint::generate_device_fingerprint();
        let needs_write = config
            .huanxing
            .node_id
            .as_deref()
            .map(|s| s.is_empty())
            .unwrap_or(true);

        if needs_write {
            // 写回文件，供下次启动直接读取
            if let Err(e) = crate::huanxing::device_fingerprint::persist_node_id_to_config(
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
        crate::huanxing::device_fingerprint::set_global_fingerprint(fp);
    }

    // Sync common skills from hub before loading tenant contexts
    if let Some(ref hub_dir) = config.huanxing.hub_dir {
        let common_skills_dir = config
            .huanxing
            .resolve_common_skills_dir(&config.workspace_dir);
        match crate::huanxing::sync::sync_common_skills(hub_dir, &common_skills_dir).await {
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
            crate::huanxing::skill_market_tools::register_global_router(Arc::clone(&router));

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
