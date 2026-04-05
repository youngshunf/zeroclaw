/**
 * HASN WebSocket 事件适配层
 *
 * Tauri 桌面端：监听 Tauri emit 事件（hasn:message, hasn:ack, hasn:typing 等）
 * Web 浏览器：连接 sidecar 的 /ws/hasn-events 推流
 *
 * 对齐 Tauri hasn.rs 的 handle_ws_event 中 emit 的事件名。
 */

export type HasnEventType =
  | "connected"
  | "message"
  | "ack"
  | "typing"
  | "presence"
  | "message_recalled"
  | "owner_bound"
  | "owner_removed"
  | "owner_renewed"
  | "owners_list"
  | "agents_reported"
  | "error"
  | "disconnected";

export interface HasnWsEvent {
  type: HasnEventType;
  data: any;
}

export type HasnEventHandler = (event: HasnWsEvent) => void;

class HasnWebSocket {
  private handlers: Set<HasnEventHandler> = new Set();
  private ws: WebSocket | null = null;
  private tauriUnlisteners: (() => void)[] = [];
  private _connected = false;

  get connected(): boolean {
    return this._connected;
  }

  /** 外部通知连接已建立（如 REST API hasnConnect 成功后） */
  emitConnected(): void {
    this._connected = true;
    this.emit({ type: "connected", data: {} });
  }

  /** 外部通知连接断开 */
  emitDisconnected(): void {
    this._connected = false;
    this.emit({ type: "disconnected", data: {} });
  }

  subscribe(handler: HasnEventHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  async connect(): Promise<void> {
    this.connectWeb();
  }

  disconnect(): void {
    this._connected = false;
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private connectWeb(): void {
    const token = localStorage.getItem("hasn:platform_token") || "";
    const hasnId = localStorage.getItem("hasn:hasn_id") || "";
    
    // 连接到 Sidecar 的 hasn-events 推流接口
    // HTTP URL 形式如 http://localhost:42620
    import('../config').then(({ HUANXING_CONFIG }) => {
      const urlObj = new URL(HUANXING_CONFIG.sidecarBaseUrl);
      const protocol = urlObj.protocol === "https:" ? "wss:" : "ws:";
      const searchParams = new URLSearchParams({ 
        token, 
        hasn_id: hasnId, 
      });
      const wsUrl = `${protocol}//${urlObj.host}/api/v1/hasn/ws/hasn-events?${searchParams.toString()}`;

      this.ws = new WebSocket(wsUrl);

    this.ws.onopen = () => {
      this._connected = true;
      this.emit({ type: "connected", data: {} });
    };

    this.ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        const eventType = msg.type;

        if (eventType === "connected") {
          this.emit({ type: "connected", data: msg });
        } else if (eventType === "message") {
          this.emit({ type: "message", data: msg.payload || msg.message || msg });
        } else if (eventType === "ack") {
          this.emit({ type: "ack", data: msg });
        } else if (eventType === "typing") {
          this.emit({ type: "typing", data: msg });
        } else if (eventType === "presence") {
          this.emit({ type: "presence", data: msg });
        } else if (eventType === "message_recalled") {
          this.emit({ type: "message_recalled", data: msg });
        } else if (eventType === "owner_bound") {
          this.emit({ type: "owner_bound", data: msg });
        } else if (eventType === "owner_removed") {
          this.emit({ type: "owner_removed", data: msg });
        } else if (eventType === "owner_renewed") {
          this.emit({ type: "owner_renewed", data: msg });
        } else if (eventType === "owners_list") {
          this.emit({ type: "owners_list", data: msg });
        } else if (eventType === "offline_messages") {
          // 逐条分发
          for (const m of msg.messages || msg.data?.messages || []) {
            this.emit({ type: "message", data: { ...m, offline: true } });
          }
        } else if (eventType === "error") {
          this.emit({ type: "error", data: msg });
        } else if (eventType === "pong") {
          // 静默
        }
      } catch {
        // 忽略非 JSON
      }
    };

      this.ws.onclose = () => {
        this._connected = false;
        this.emit({ type: "disconnected", data: {} });
        // 自动重连（指数退避简化版）
        setTimeout(() => this.connectWeb(), 3000);
      };

      this.ws.onerror = () => {
        this.emit({ type: "error", data: { message: "WebSocket 连接错误" } });
      };
    });
  }

  /** 通过 Web WebSocket 发送命令 */
  sendCommand(cmd: Record<string, any>): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(cmd));
    }
  }

  private emit(event: HasnWsEvent): void {
    this.handlers.forEach((h) => {
      try { h(event); } catch { /* 忽略 handler 错误 */ }
    });
  }
}

export const hasnWs = new HasnWebSocket();
