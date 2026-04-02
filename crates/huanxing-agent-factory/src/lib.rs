use std::path::PathBuf;

pub mod engine;
pub mod types;
pub mod market_api;
pub mod scaffold;

/// 进度回调 trait
pub trait ProgressSink: Send + Sync {
    fn on_progress(&self, step: &str, detail: &str);
    fn on_error(&self, step: &str, error: &str) {
        let _ = step;
        let _ = error;
    }
}

pub struct DummyProgress;
impl ProgressSink for DummyProgress {
    fn on_progress(&self, _step: &str, _detail: &str) {}
}

/// Agent 创建参数
#[derive(Debug, Clone)]
pub struct CreateAgentParams {
    /// 租户身份（手机号/UUID等），决定归属：users/<tenant_id>
    pub tenant_id: String,
    /// 模板内部的 ID，比如 `assistant`
    pub template_id: String,
    /// Agent 在沙盒中的目录名
    pub agent_name: String,
    /// 显示给用户看的 Agent 昵称
    pub display_name: String,
    /// 标志是否为 Desktop 环境（决定配置覆盖策略）
    pub is_desktop: bool,
    /// 用户姓名/昵称 (用于 {{nickname}} 替换)
    pub user_nickname: String,
    
    // 以下为给大模型的覆写选项
    pub provider: Option<String>,
    pub api_key: Option<String>,
    
    /// Agent 绑定的 HASN Identity ID
    pub hasn_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentCreated {
    pub tenant_id: String,
    pub agent_id: String,
    pub workspace_dir: PathBuf,
}

pub struct AgentFactory {
    /// ~/.huanxing 或 /opt/huanxing/config
    pub config_dir: PathBuf,
    /// 是否有远程 api 连线供下载（给 desktop 用的）
    pub market_api_base: Option<String>,
}

impl AgentFactory {
    pub fn new(config_dir: PathBuf, market_api_base: Option<String>) -> Self {
        Self {
            config_dir,
            market_api_base,
        }
    }

    /// 根据租户获得根目录
    pub fn resolve_tenant_root(&self, tenant_id: &str) -> PathBuf {
        let name = tenant_id.trim();
        if name == "admin" || name == "guardian" || name.is_empty() {
            self.config_dir.join("admin") // Fail safe fallback if no tenant
        } else {
            self.config_dir.join("users").join(name)
        }
    }
}
