use serde::{Deserialize, Serialize};

/// Sidecar 运行状态（返回给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub memory_backend: Option<String>,
    pub restart_count: u32,
    pub version: Option<String>,
}

/// 健康检查 API 响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HealthResponse {
    pub status: Option<String>,
    #[serde(default)]
    pub paired: bool,
    #[serde(default)]
    pub runtime: Option<HealthRuntime>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HealthRuntime {
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub uptime_seconds: Option<u64>,
}

/// 状态 API 响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct StatusResponse {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub memory_backend: Option<String>,
    #[serde(default)]
    pub gateway_port: Option<u16>,
    #[serde(default)]
    pub pid: Option<u32>,
}

/// 日志条目
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Sidecar 事件（emit 到前端）
#[derive(Debug, Clone, Serialize)]
pub struct SidecarEvent {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrashEvent {
    pub exit_code: Option<i32>,
    pub restart_count: u32,
    pub will_restart: bool,
}

/// Onboard 请求（前端登录成功后发送）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardRequest {
    pub llm_token: String,
    pub user_nickname: Option<String>,
    pub user_uuid: Option<String>,
    pub user_phone: Option<String>,
    pub agent_key: Option<String>,
    pub api_base_url: Option<String>,
    /// LLM 网关地址（含 /v1 后缀），如 http://127.0.0.1:3180/v1
    pub llm_gateway_url: Option<String>,
    pub hasn_api_key: Option<String>,
    /// 默认 LLM provider (e.g. "custom:http://127.0.0.1:3180/v1")
    pub default_provider: Option<String>,
    /// 降级 fallback provider (e.g. "custom:https://llm.dcfuture.cn/v1")
    pub fallback_provider: Option<String>,
    /// 嵌入向量 provider (e.g. "custom:https://llm.dcfuture.cn/v1")
    pub embedding_provider: Option<String>,
}

/// Onboard 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardResult {
    pub success: bool,
    pub config_created: bool,
    pub agent_created: bool,
    pub sidecar_started: bool,
    pub tenant_dir: Option<String>,
    pub agent_id: Option<String>,
    pub config_path: Option<String>,
    pub workspace_path: Option<String>,
    pub agent_create_stdout: Option<String>,
    pub agent_create_stderr: Option<String>,
    pub error: Option<String>,
}

/// 快捷配置项（前端可修改的字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickConfig {
    pub default_model: Option<String>,
    pub default_temperature: Option<f64>,
    pub autonomy_level: Option<String>,
    pub gateway_port: Option<u16>,
}
