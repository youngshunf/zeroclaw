use thiserror::Error;

/// hasn-client-core 统一错误类型
#[derive(Debug, Error)]
pub enum HasnError {
    #[error("HTTP 请求错误: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API 错误 [{status}]: {message}")]
    Api { status: u16, message: String },

    #[error("WebSocket 错误: {0}")]
    Ws(String),

    #[error("数据库错误: {0}")]
    Db(String),

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("认证错误: {0}")]
    Auth(String),
}

// 方便序列化给前端
impl serde::Serialize for HasnError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
