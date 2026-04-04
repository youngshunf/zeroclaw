use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::error::HasnError;
use crate::model::{HasnFrame, build_ping};

/// WebSocket 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum WsStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

/// HASN 原生 WebSocket 客户端 (v2.0)
///
/// 帧格式: { "hasn": "hasn/2.0", "method": "hasn.xxx.yyy", "params": {...} }
pub struct HasnWsClient {
    status: Arc<RwLock<WsStatus>>,
    /// 发送通道: 往这里写会通过WS发给服务端
    sender: Arc<Mutex<Option<mpsc::Sender<String>>>>,
    /// 控制关闭
    cancel: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl HasnWsClient {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(WsStatus::Disconnected)),
            sender: Arc::new(Mutex::new(None)),
            cancel: Arc::new(Mutex::new(None)),
        }
    }

    /// 获取当前状态
    pub async fn status(&self) -> WsStatus {
        self.status.read().await.clone()
    }

    /// 连接 WebSocket
    ///
    /// `url`: wss://{server}/api/v1/hasn/ws/node?node_key=hasn_nk_xxx
    /// `on_event`: 收到服务端帧时回调（HasnFrame）
    pub async fn connect<F>(&self, url: &str, on_event: F) -> Result<(), HasnError>
    where
        F: Fn(HasnFrame) + Send + Sync + 'static,
    {
        // 先断开旧连接
        self.disconnect().await;

        *self.status.write().await = WsStatus::Connecting;
        info!(
            "[HasnWS] 连接: {}",
            &url[..url.find('?').unwrap_or(url.len())]
        );

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| HasnError::Ws(format!("WS连接失败: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        *self.status.write().await = WsStatus::Connected;
        info!("[HasnWS] 已连接");

        // 创建发送通道
        let (tx, mut rx) = mpsc::channel::<String>(64);
        *self.sender.lock().await = Some(tx);

        // 创建取消信号
        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        *self.cancel.lock().await = Some(cancel_tx);

        let status = self.status.clone();

        // 主循环
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // 收到服务端消息
                    Some(msg) = read.next() => {
                        match msg {
                            Ok(Message::Text(text)) => {
                                match serde_json::from_str::<HasnFrame>(&text) {
                                    Ok(frame) => on_event(frame),
                                    Err(e) => {
                                        warn!("[HasnWS] 帧解析失败: {} text={}", e, &text[..text.len().min(200)]);
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                info!("[HasnWS] 服务端关闭连接");
                                break;
                            }
                            Err(e) => {
                                error!("[HasnWS] 接收错误: {}", e);
                                break;
                            }
                            _ => {} // Binary, Ping, Pong 由 tungstenite 自动处理
                        }
                    }

                    // 有消息需要发送
                    Some(text) = rx.recv() => {
                        if let Err(e) = write.send(Message::Text(text.into())).await {
                            error!("[HasnWS] 发送失败: {}", e);
                            break;
                        }
                    }

                    // 取消信号
                    _ = &mut cancel_rx => {
                        info!("[HasnWS] 收到断开信号");
                        let _ = write.close().await;
                        break;
                    }
                }
            }

            *status.write().await = WsStatus::Disconnected;
            info!("[HasnWS] 连接已关闭");
        });

        // 启动心跳定时器（协议 §3.11: 30s 间隔）
        let heartbeat_sender = self.sender.clone();
        let heartbeat_status = self.status.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.tick().await; // 跳过第一次立即触发
            loop {
                interval.tick().await;
                // 如果不再 Connected，停止心跳
                if *heartbeat_status.read().await != WsStatus::Connected {
                    break;
                }
                let ping = build_ping(chrono::Utc::now().timestamp_millis());
                if let Ok(json) = serde_json::to_string(&ping) {
                    let sender = heartbeat_sender.lock().await;
                    if let Some(tx) = sender.as_ref() {
                        if tx.send(json).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&self) {
        if let Some(cancel) = self.cancel.lock().await.take() {
            let _ = cancel.send(());
        }
        *self.sender.lock().await = None;
        *self.status.write().await = WsStatus::Disconnected;
    }

    /// 发送上行帧
    pub async fn send_frame(&self, frame: &HasnFrame) -> Result<(), HasnError> {
        let json = serde_json::to_string(frame).map_err(|e| HasnError::Parse(e.to_string()))?;

        let sender = self.sender.lock().await;
        if let Some(tx) = sender.as_ref() {
            tx.send(json)
                .await
                .map_err(|_| HasnError::Ws("发送通道已关闭".to_string()))?;
            Ok(())
        } else {
            Err(HasnError::Ws("未连接".to_string()))
        }
    }

    /// 自动重连 (指数退避)
    pub async fn connect_with_retry<F>(
        &self,
        url: &str,
        on_event: F,
        max_retries: u32,
    ) -> Result<(), HasnError>
    where
        F: Fn(HasnFrame) + Send + Sync + Clone + 'static,
    {
        let mut delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(30);

        for attempt in 0..=max_retries {
            if attempt > 0 {
                *self.status.write().await = WsStatus::Reconnecting { attempt };
                info!(
                    "[HasnWS] 重连尝试 {}/{}, 等待 {:?}",
                    attempt, max_retries, delay
                );
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, max_delay);
            }

            match self.connect(url, on_event.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!("[HasnWS] 连接失败 (尝试{}): {}", attempt, e);
                    if attempt == max_retries {
                        return Err(e);
                    }
                }
            }
        }

        Err(HasnError::Ws("重连次数已用尽".to_string()))
    }
}
