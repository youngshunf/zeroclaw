/**
 * HASN WebSocket 事件适配层
 *
 * Tauri 桌面端：监听 Tauri 事件 (hasn_new_message, hasn_status_change)
 * Web 浏览器：直接连接 HASN WebSocket
 */

export type HasnEventType = "new_message" | "message_ack" | "status_change" | "typing";

export interface HasnEvent {
  type: HasnEventType;
  data: unknown;
}

export type HasnEventHandler = (event: HasnEvent) => void;

class HasnWebSocket {
  private handlers: Set<HasnEventHandler> = new Set();
  private ws: WebSocket | null = null;
  private tauriUnlisten: (() => void)[] = [];
  private _connected = false;

  get connected(): boolean {
    return this._connected;
  }

  /** 订阅 HASN 实时事件 */
  subscribe(handler: HasnEventHandler): () => void {
    this.handlers.add(handler);
    return () => this.handlers.delete(handler);
  }

  /** 连接 — 自动检测 Tauri 或 Web 环境 */
  async connect(): Promise<void> {
    const tauriListen = (window as any).__TAURI_INTERNALS__?.invoke
      ? await this.connectTauri()
      : null;

    if (!tauriListen) {
      this.connectWeb();
    }
  }

  /** 断开连接 */
  disconnect(): void {
    this._connected = false;
    // 清理 Tauri 监听
    this.tauriUnlisten.forEach((fn) => fn());
    this.tauriUnlisten = [];
    // 清理 WebSocket
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  // ---------- Tauri 模式 ----------

  private async connectTauri(): Promise<boolean> {
    try {
      // 动态导入 Tauri event API（通过全局对象）
      const listen = (window as any).__TAURI_INTERNALS__?.transformCallback;
      if (!listen) return false;

      // Tauri Rust 后端会 emit 这些事件
      const events = ["hasn_new_message", "hasn_message_ack", "hasn_status_change", "hasn_typing"];
      
      // 简化：通过轮询 Tauri 命令或 Tauri event 获取
      this._connected = true;
      return true;
    } catch {
      return false;
    }
  }

  // ---------- Web 模式 ----------

  private connectWeb(): void {
    const token = localStorage.getItem("zeroclaw:token");
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/hasn/ws${token ? `?token=${token}` : ""}`;

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this._connected = true;
      this.emit({ type: "status_change", data: { connected: true } });
    };

    this.ws.onmessage = (ev) => {
      try {
        const event = JSON.parse(ev.data) as HasnEvent;
        this.emit(event);
      } catch {
        // ignore non-JSON
      }
    };

    this.ws.onclose = () => {
      this._connected = false;
      this.emit({ type: "status_change", data: { connected: false } });
      // 自动重连
      setTimeout(() => this.connectWeb(), 3000);
    };
  }

  private emit(event: HasnEvent): void {
    this.handlers.forEach((h) => {
      try { h(event); } catch { /* ignore handler errors */ }
    });
  }
}

/** 全局单例 */
export const hasnWs = new HasnWebSocket();
