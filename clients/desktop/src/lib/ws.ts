/**
 * WebSocket 多路复用客户端
 *
 * 全局唯一 WS 连接，通过帧中的 session_id 将消息路由到各自的处理器。
 * 不再为每个会话单独建立 WebSocket 连接。
 */

import type { WsMessage } from '../types/api';
import { getToken } from './auth';

// ---------------------------------------------------------------------------
// 类型定义
// ---------------------------------------------------------------------------

export type SessionMessageHandler = (msg: WsMessage) => void;
export type ConnectionStatusHandler = (status: 'connected' | 'disconnected' | 'connecting') => void;

// ---------------------------------------------------------------------------
// WsMultiplexer — 单连接多路复用
// ---------------------------------------------------------------------------

const DEFAULT_RECONNECT_DELAY = 1000;
const MAX_RECONNECT_DELAY = 30000;

export class WsMultiplexer {
  private ws: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionallyClosed = false;
  private currentDelay = DEFAULT_RECONNECT_DELAY;

  /** session_id → 处理器集合 */
  private sessionHandlers = new Map<string, Set<SessionMessageHandler>>();

  /** 全局连接状态变更回调 */
  public onStatusChange: ConnectionStatusHandler | null = null;

  /**
   * 订阅某个 session 的消息
   * @returns 取消订阅函数
   */
  subscribe(sessionId: string, handler: SessionMessageHandler): () => void {
    if (!this.sessionHandlers.has(sessionId)) {
      this.sessionHandlers.set(sessionId, new Set());
    }
    this.sessionHandlers.get(sessionId)!.add(handler);
    return () => {
      const handlers = this.sessionHandlers.get(sessionId);
      handlers?.delete(handler);
      if (handlers?.size === 0) {
        this.sessionHandlers.delete(sessionId);
      }
    };
  }

  /** 向指定 session 发送聊天消息 */
  send(sessionId: string, content: string, agent?: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket 未连接');
    }
    this.ws.send(
      JSON.stringify({
        type: 'message',
        session_id: sessionId,
        agent,
        content,
      }),
    );
  }

  /** 请求指定 session 的历史记录 */
  requestHistory(sessionId: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify({ type: 'history_request', session_id: sessionId }));
  }

  /**
   * 建立连接（幂等，已连接则跳过）
   * @param baseUrl 可选，默认使用当前 host
   */
  connect(baseUrl?: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) return;
    this.intentionallyClosed = false;
    this._doConnect(baseUrl);
  }

  /** 主动断开连接，不再自动重连 */
  disconnect(): void {
    this.intentionallyClosed = true;
    this._clearTimer();
    this.ws?.close();
    this.ws = null;
    this.onStatusChange?.('disconnected');
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  // ---------------------------------------------------------------------------
  // 内部方法
  // ---------------------------------------------------------------------------

  private _resolvedBaseUrl: string | undefined;

  private _doConnect(baseUrl?: string): void {
    this._clearTimer();

    // 记住 baseUrl 供重连时复用
    if (baseUrl) this._resolvedBaseUrl = baseUrl;
    const base = this._resolvedBaseUrl ?? this._defaultBaseUrl();

    const token = getToken();
    const params = token ? `?token=${encodeURIComponent(token)}` : '';
    const url = `${base}/ws/chat${params}`;

    this.onStatusChange?.('connecting');
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.currentDelay = DEFAULT_RECONNECT_DELAY;
      this.onStatusChange?.('connected');
    };

    this.ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = JSON.parse(ev.data as string) as WsMessage;
        this._dispatch(msg);
      } catch {
        // 忽略非 JSON 帧
      }
    };

    this.ws.onclose = () => {
      this.onStatusChange?.('disconnected');
      if (!this.intentionallyClosed) this._scheduleReconnect();
    };

    this.ws.onerror = () => {
      // onerror 后紧跟 onclose，重连在 onclose 中处理
    };
  }

  private _dispatch(msg: WsMessage): void {
    const sessionId = msg.session_id;
    if (!sessionId) {
      // 无 session_id 的帧（如 connected）无需路由
      return;
    }
    const handlers = this.sessionHandlers.get(sessionId);
    if (!handlers) return;
    for (const h of handlers) {
      try {
        h(msg);
      } catch {
        // 忽略 handler 错误，避免影响其他 handler
      }
    }
  }

  private _scheduleReconnect(): void {
    this.reconnectTimer = setTimeout(() => {
      this.currentDelay = Math.min(this.currentDelay * 2, MAX_RECONNECT_DELAY);
      this._doConnect();
    }, this.currentDelay);
  }

  private _clearTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private _defaultBaseUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}`;
  }
}

/** 全局单例 WsMultiplexer */
export const wsMultiplexer = new WsMultiplexer();

// ---------------------------------------------------------------------------
// WebSocketClient — 保留旧类供过渡期兼容（deprecated）
// ---------------------------------------------------------------------------

export type WsMessageHandler = (msg: WsMessage) => void;
export type WsOpenHandler = () => void;
export type WsCloseHandler = (ev: CloseEvent) => void;
export type WsErrorHandler = (ev: Event) => void;

export interface WebSocketClientOptions {
  baseUrl?: string;
  reconnectDelay?: number;
  maxReconnectDelay?: number;
  autoReconnect?: boolean;
}

const WS_SESSION_STORAGE_KEY = 'zeroclaw.ws.session_id';

/**
 * @deprecated 请使用全局单例 `wsMultiplexer` + `SessionManager`。
 * 此类将在下一个版本中移除。
 */
export class WebSocketClient {
  private ws: WebSocket | null = null;
  private currentDelay: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionallyClosed = false;

  public onMessage: WsMessageHandler | null = null;
  public onOpen: WsOpenHandler | null = null;
  public onClose: WsCloseHandler | null = null;
  public onError: WsErrorHandler | null = null;

  private readonly baseUrl: string;
  private readonly reconnectDelay: number;
  private readonly maxReconnectDelay: number;
  private readonly autoReconnect: boolean;
  private readonly sessionId: string;

  constructor(options: WebSocketClientOptions = {}) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.baseUrl = options.baseUrl ?? `${protocol}//${window.location.host}`;
    this.reconnectDelay = options.reconnectDelay ?? DEFAULT_RECONNECT_DELAY;
    this.maxReconnectDelay = options.maxReconnectDelay ?? MAX_RECONNECT_DELAY;
    this.autoReconnect = options.autoReconnect ?? true;
    this.currentDelay = this.reconnectDelay;
    this.sessionId = this._resolveSessionId();
  }

  connect(): void {
    this.intentionallyClosed = false;
    this._clearReconnectTimer();

    const token = getToken();
    const params = new URLSearchParams();
    if (token) params.set('token', token);
    params.set('session_id', this.sessionId);
    const url = `${this.baseUrl}/ws/chat?${params.toString()}`;

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.currentDelay = this.reconnectDelay;
      this.onOpen?.();
    };

    this.ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = JSON.parse(ev.data as string) as WsMessage;
        this.onMessage?.(msg);
      } catch {
        // 忽略非 JSON 帧
      }
    };

    this.ws.onclose = (ev: CloseEvent) => {
      this.onClose?.(ev);
      this._scheduleReconnect();
    };

    this.ws.onerror = (ev: Event) => {
      this.onError?.(ev);
    };
  }

  sendMessage(content: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }
    this.ws.send(JSON.stringify({ type: 'message', content }));
  }

  disconnect(): void {
    this.intentionallyClosed = true;
    this._clearReconnectTimer();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  private _scheduleReconnect(): void {
    if (this.intentionallyClosed || !this.autoReconnect) return;
    this.reconnectTimer = setTimeout(() => {
      this.currentDelay = Math.min(this.currentDelay * 2, this.maxReconnectDelay);
      this.connect();
    }, this.currentDelay);
  }

  private _clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private _resolveSessionId(): string {
    const existing = window.localStorage.getItem(WS_SESSION_STORAGE_KEY);
    if (existing && /^[A-Za-z0-9_-]{1,128}$/.test(existing)) {
      return existing;
    }
    const generated =
      globalThis.crypto?.randomUUID?.().replace(/-/g, '_') ??
      `sess_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
    window.localStorage.setItem(WS_SESSION_STORAGE_KEY, generated);
    return generated;
  }
}
