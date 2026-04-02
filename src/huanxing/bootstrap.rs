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
