use std::time::Duration;

/// 唤星专属端口（不与 ZeroClaw 默认 42617 冲突）
pub const HUANXING_PORT: u16 = 42620;
/// 唤星配置目录名
pub const HUANXING_DIR_NAME: &str = ".huanxing";
/// 最大自动重启次数
pub const MAX_AUTO_RESTARTS: u32 = 3;
/// 自动重启计数重置窗口
pub const RESTART_WINDOW: Duration = Duration::from_secs(300); // 5 分钟
/// 健康检查超时
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);
/// 启动后等待健康检查最长时间
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
/// 日志缓冲区最大行数
pub const LOG_BUFFER_SIZE: usize = 500;
/// SIGTERM 后等待退出的时间
pub const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
