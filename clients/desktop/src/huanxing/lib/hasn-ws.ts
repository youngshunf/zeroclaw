/**
 * HASN WebSocket 事件适配层
 *
 * Tauri 桌面端：监听 Tauri emit 事件（hasn:message, hasn:ack, hasn:typing 等）
 * Web 浏览器：直接连接 /ws/client WebSocket
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

  subscribe(handler: HasnEventHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  async connect(): Promise<void> {
    if (this.isTauri()) {
      await this.connectTauri();
    } else {
      this.connectWeb();
    }
  }

  disconnect(): void {
    this._connected = false;
    this.tauriUnlisteners.forEach((fn) => fn());
    this.tauriUnlisteners = [];
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private isTauri(): boolean {
    return !!(window as any).__TAURI_INTERNALS__?.invoke;
  }

  // ---------- Tauri 模式 ----------

  private async connectTauri(): Promise<void> {
    try {
      // 动态导入 Tauri event API
      const { listen } = await import("@tauri-apps/api/event");

      // 监听 Tauri Rust 后端 emit 的事件（对齐 hasn.rs handle_ws_event）
      const eventMap: Record<string, HasnEventType> = {
        "hasn:connected": "connected",
        "hasn:message": "message",
        "hasn:ack": "ack",
        "hasn:typing": "typing",
        "hasn:presence": "presence",
        "hasn:message_recalled": "message_recalled",
        "hasn:agents_reported": "agents_reported",
        "hasn:error": "error",
      };

      for (const [tauriEvent, hasnType] of Object.entries(eventMap)) {
        const unlisten = await listen(tauriEvent, (event: any) => {
          this.emit({ type: hasnType, data: event.payload });
        });
        this.tauriUnlisteners.push(unlisten);
      }

      this._connected = true;
      this.emit({ type: "connected", data: {} });
    } catch (e) {
      console.error("[HasnWS] Tauri 事件监听失败:", e);
    }
  }

  // ---------- Web 模式 ----------

  private connectWeb(): void {
    const token = localStorage.getItem("hasn:client_jwt");
    if (!token) {
      console.warn("[HasnWS] 无 client_jwt，无法连接 WebSocket");
      return;
    }

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/api/v1/hasn/ws/client?token=${token}`;

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this._connected = true;
      this.emit({ type: "connected", data: {} });
    };

    this.ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        const cmd = msg.cmd?.toLowerCase();

        if (cmd === "connected") {
          this.emit({ type: "connected", data: msg });
        } else if (cmd === "message") {
          this.emit({ type: "message", data: msg.message || msg });
        } else if (cmd === "ack") {
          this.emit({ type: "ack", data: msg });
        } else if (cmd === "typing") {
          this.emit({ type: "typing", data: msg });
        } else if (cmd === "presence") {
          this.emit({ type: "presence", data: msg });
        } else if (cmd === "message_recalled") {
          this.emit({ type: "message_recalled", data: msg });
        } else if (cmd === "offline_messages") {
          // 逐条分发
          for (const m of msg.messages || []) {
            this.emit({ type: "message", data: { ...m, offline: true } });
          }
        } else if (cmd === "error") {
          this.emit({ type: "error", data: msg });
        } else if (cmd === "pong") {
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
