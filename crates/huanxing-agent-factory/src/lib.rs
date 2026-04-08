use std::path::PathBuf;

pub mod engine;
pub mod market_api;
pub mod scaffold;
pub mod types;

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
    /// 用户手机号 (用于 {{phone}} 替换)
    pub user_phone: String,
    /// 用户全局目录的绝对路径 (用于 {{owner_dir}} 替换，让 BOOTSTRAP 中的指令指向正确位置)
    pub owner_dir: String,

    // 以下为给大模型的覆写选项
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,

    /// Agent 绑定的 HASN Identity ID
    pub hasn_id: Option<String>,

    /// 降级 fallback provider (e.g. "custom:https://llm.dcfuture.cn/v1")
    pub fallback_provider: Option<String>,
    /// 嵌入向量 provider (e.g. "custom:https://llm.dcfuture.cn/v1")
    pub embedding_provider: Option<String>,
    /// LLM 网关 V1 URL (e.g. "http://127.0.0.1:3180/v1")，用于 TTS/STT api_url
    pub llm_gateway: Option<String>,
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

    /// 生成全局配置文件 (`~/.huanxing/config.toml`)
    /// 使用嵌入的 scaffold/global/config.toml.template，替换占位符。
    /// 此配置是进程级的，与用户/租户无关。
    pub fn generate_global_config(&self, vars: &GlobalConfigVars) -> String {
        let template = scaffold::global_scaffold()
            .into_iter()
            .find(|s| s.name == "config.toml.template")
            .and_then(|s| match s.content {
                scaffold::EmbeddedContent::Text(t) => Some(t.to_string()),
                _ => None,
            })
            .unwrap_or_default();

        template
            .replace("{{star_name}}", &vars.display_name)
            .replace("{{default_provider}}", &vars.default_provider)
            .replace("{{default_model}}", &vars.default_model)
            .replace("{{title_model}}", &vars.title_model)
            .replace("{{gateway_port}}", &vars.gateway_port.to_string())
            .replace("{{llm_gateway}}", &vars.llm_gateway)
            .replace("{{api_base_url}}", &vars.api_base_url)
            .replace("{{agent_key}}", &vars.agent_key)
            .replace("{{node_id}}", &vars.node_id)
            .replace("{{hasn_api_key}}", &vars.hasn_api_key)
    }
}

/// 全局配置生成所需的变量
#[derive(Debug, Clone)]
pub struct GlobalConfigVars {
    pub display_name: String,
    pub default_provider: String,
    pub default_model: String,
    pub title_model: String,
    pub gateway_port: u16,
    pub llm_gateway: String,
    pub api_base_url: String,
    pub agent_key: String,
    pub node_id: String,
    pub hasn_api_key: String,
}
