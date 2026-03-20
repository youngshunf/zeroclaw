/**
 * session-manager.ts — 多会话状态管理器
 *
 * 不再持有 WebSocket 连接。所有收发通过全局 wsMultiplexer。
 * 职责：管理会话列表、消息历史、未读计数、流式状态（思考/工具调用）。
 */

import { wsMultiplexer } from './ws';
import type { WsMessage } from '../types/api';

// ---------------------------------------------------------------------------
// 公共类型
// ---------------------------------------------------------------------------

export interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp?: string;
}

export type SessionStatus = 'connecting' | 'connected' | 'disconnected' | 'thinking';

/** 单条工具调用的状态 */
export interface ToolCallEntry {
  call_id: string;
  name: string;
  display_name: string;
  args_preview: string;
  status: 'pending' | 'success' | 'error' | 'cancelled';
  output?: string;
  output_preview?: string;
  duration_ms?: number;
}

/** 当前 turn 的流式状态（思考过程 + 工具调用列表） */
export interface StreamingState {
  /** 思考内容（累积） */
  thinkingContent: string;
  /** 思考是否完成（done=true 后锁定） */
  thinkingDone: boolean;
  /** 按插入顺序的工具调用列表 */
  toolCalls: ToolCallEntry[];
  /** 当前流式文本 chunk 累积 */
  streamingText: string;
}

export type SessionChangeHandler = (sessionId: string | null) => void;
export type MessageHandler = (sessionId: string, messages: ChatMessage[]) => void;
export type StatusHandler = (sessionId: string, status: SessionStatus) => void;
export type StreamingUpdateHandler = (sessionId: string, state: StreamingState) => void;

// ---------------------------------------------------------------------------
// SessionManager
// ---------------------------------------------------------------------------

export class SessionManager {
  /** 消息历史，每个 session 独立 */
  private histories = new Map<string, ChatMessage[]>();
  /** 连接状态 */
  private statuses = new Map<string, SessionStatus>();
  /** 未读消息计数 */
  private unreadCounts = new Map<string, number>();
  /** multiplexer 取消订阅函数 */
  private unsubscribers = new Map<string, () => void>();
  /** 当前流式状态（每个 turn 用完即清） */
  private streamingStates = new Map<string, StreamingState>();
  /** 当前活跃 session */
  private _activeSessionId: string | null = null;
  /** 当前活跃 Agent 名称 */
  private _activeAgent: string | undefined;

  // 事件回调
  public onSessionChange: SessionChangeHandler | null = null;
  public onMessage: MessageHandler | null = null;
  public onStatusChange: StatusHandler | null = null;
  public onStreamingUpdate: StreamingUpdateHandler | null = null;

  get activeSessionId(): string | null {
    return this._activeSessionId;
  }

  // ---------------------------------------------------------------------------
  // 公共方法
  // ---------------------------------------------------------------------------

  /** 注册 session，开始接收其消息（幂等） */
  connectSession(sessionId: string, agentName?: string): void {
    if (this.unsubscribers.has(sessionId)) return;

    this.statuses.set(sessionId, 'connecting');
    this.notifyStatus(sessionId);

    const unsub = wsMultiplexer.subscribe(sessionId, (msg) => {
      this.handleMsg(sessionId, msg);
    });
    this.unsubscribers.set(sessionId, unsub);

    // 如果 WS 已连接，立即请求历史
    if (wsMultiplexer.connected) {
      wsMultiplexer.requestHistory(sessionId);
      this.statuses.set(sessionId, 'connected');
      this.notifyStatus(sessionId);
    }
  }

  /** 切换活跃 session */
  switchTo(sessionId: string, agentName?: string): void {
    this._activeSessionId = sessionId;
    this._activeAgent = agentName;
    this.unreadCounts.set(sessionId, 0);

    if (!this.unsubscribers.has(sessionId)) {
      this.connectSession(sessionId, agentName);
    }
    this.onSessionChange?.(sessionId);
  }

  /** 发送聊天消息 */
  sendMessage(content: string, sessionId?: string): void {
    const sid = sessionId ?? this._activeSessionId;
    if (!sid) throw new Error('No active session');
    if (!wsMultiplexer.connected) throw new Error('WebSocket 未连接');

    // 本地立即追加用户消息
    this.appendMessage(sid, { role: 'user', content, timestamp: new Date().toISOString() });

    // 重置流式状态
    this.streamingStates.set(sid, {
      thinkingContent: '',
      thinkingDone: false,
      toolCalls: [],
      streamingText: '',
    });

    this.statuses.set(sid, 'thinking');
    this.notifyStatus(sid);

    wsMultiplexer.send(sid, content, this._activeAgent);
  }

  /** 移除 session（取消订阅 + 清空历史） */
  removeSession(sessionId: string): void {
    this.unsubscribers.get(sessionId)?.();
    this.unsubscribers.delete(sessionId);
    this.histories.delete(sessionId);
    this.unreadCounts.delete(sessionId);
    this.statuses.delete(sessionId);
    this.streamingStates.delete(sessionId);

    if (this._activeSessionId === sessionId) {
      this._activeSessionId = null;
      this.onSessionChange?.(null);
    }
  }

  /** 清空所有 session（切换 Agent 时使用） */
  clearAll(): void {
    for (const unsub of this.unsubscribers.values()) {
      unsub();
    }
    this.unsubscribers.clear();
    this.histories.clear();
    this.unreadCounts.clear();
    this.statuses.clear();
    this.streamingStates.clear();
    this._activeSessionId = null;
    this.onSessionChange?.(null);
  }

  /** 向后兼容别名 */
  disconnectAll(): void {
    this.clearAll();
  }

  /** 向后兼容别名 */
  disconnectSession(sessionId: string): void {
    this.removeSession(sessionId);
  }

  getHistory(sessionId: string): ChatMessage[] {
    return this.histories.get(sessionId) ?? [];
  }

  getStatus(sessionId: string): SessionStatus {
    return this.statuses.get(sessionId) ?? 'disconnected';
  }

  getUnreadCount(sessionId: string): number {
    return this.unreadCounts.get(sessionId) ?? 0;
  }

  getStreamingState(sessionId: string): StreamingState {
    return (
      this.streamingStates.get(sessionId) ?? {
        thinkingContent: '',
        thinkingDone: false,
        toolCalls: [],
        streamingText: '',
      }
    );
  }

  // ---------------------------------------------------------------------------
  // 内部消息处理
  // ---------------------------------------------------------------------------

  private handleMsg(sessionId: string, msg: WsMessage): void {
    switch (msg.type) {
      case 'session_start':
        // 后端通知 session 初始化完成
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        break;

      case 'history':
        if (Array.isArray(msg.messages)) {
          const restored: ChatMessage[] = msg.messages.map((m) => ({
            role: m.role,
            content: m.content,
            timestamp: m.timestamp,
          }));
          this.histories.set(sessionId, restored);
          this.notifyMessages(sessionId);
        }
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        break;

      case 'chunk': {
        // 流式文本 chunk
        const state = this.getOrInitStreaming(sessionId);
        state.streamingText += msg.content ?? '';
        this.streamingStates.set(sessionId, state);
        this.notifyStreaming(sessionId);
        break;
      }

      case 'thinking': {
        const state = this.getOrInitStreaming(sessionId);
        if (!msg.done) {
          state.thinkingContent += msg.content ?? '';
        } else {
          // done=true：如果有完整内容则替换，否则保留累积内容
          if (msg.content) state.thinkingContent = msg.content;
          state.thinkingDone = true;
        }
        this.streamingStates.set(sessionId, state);
        this.notifyStreaming(sessionId);
        break;
      }

      case 'tool_call': {
        const state = this.getOrInitStreaming(sessionId);
        const existing = state.toolCalls.findIndex((t) => t.call_id === msg.call_id);
        const entry: ToolCallEntry = {
          call_id: msg.call_id ?? '',
          name: msg.name ?? '',
          display_name: msg.display_name ?? msg.name ?? '',
          args_preview: msg.args_preview ?? JSON.stringify(msg.args ?? {}),
          status: 'pending',
        };
        if (existing >= 0) {
          state.toolCalls[existing] = entry;
        } else {
          state.toolCalls.push(entry);
        }
        this.streamingStates.set(sessionId, state);
        this.notifyStreaming(sessionId);
        break;
      }

      case 'tool_result': {
        const state = this.getOrInitStreaming(sessionId);
        const idx = state.toolCalls.findIndex((t) => t.call_id === msg.call_id);
        if (idx >= 0) {
          state.toolCalls[idx] = {
            ...state.toolCalls[idx],
            status: msg.status ?? 'success',
            output: msg.output,
            output_preview: msg.output_preview,
            duration_ms: msg.duration_ms,
          };
          this.streamingStates.set(sessionId, state);
          this.notifyStreaming(sessionId);
        }
        break;
      }

      case 'done': {
        // Turn 结束，将完整回复写入历史
        const response = msg.full_response ?? '';
        if (response) {
          this.appendMessage(sessionId, {
            role: 'assistant',
            content: response,
            timestamp: new Date().toISOString(),
          });
        }
        // 清空流式状态
        this.streamingStates.delete(sessionId);
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        this.notifyStreaming(sessionId);

        if (sessionId !== this._activeSessionId) {
          this.unreadCounts.set(sessionId, (this.unreadCounts.get(sessionId) ?? 0) + 1);
        }
        break;
      }

      case 'error': {
        const errorText = msg.message ?? '未知错误';
        this.appendMessage(sessionId, {
          role: 'system',
          content: `错误: ${errorText}`,
          timestamp: new Date().toISOString(),
        });
        // 清空流式状态
        this.streamingStates.delete(sessionId);
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        this.notifyStreaming(sessionId);
        break;
      }
    }
  }

  private getOrInitStreaming(sessionId: string): StreamingState {
    if (!this.streamingStates.has(sessionId)) {
      this.streamingStates.set(sessionId, {
        thinkingContent: '',
        thinkingDone: false,
        toolCalls: [],
        streamingText: '',
      });
    }
    return this.streamingStates.get(sessionId)!;
  }

  private appendMessage(sessionId: string, msg: ChatMessage): void {
    const history = this.histories.get(sessionId) ?? [];
    this.histories.set(sessionId, [...history, msg]);
    this.notifyMessages(sessionId);
  }

  private notifyMessages(sessionId: string): void {
    this.onMessage?.(sessionId, this.histories.get(sessionId) ?? []);
  }

  private notifyStatus(sessionId: string): void {
    this.onStatusChange?.(sessionId, this.statuses.get(sessionId) ?? 'disconnected');
  }

  private notifyStreaming(sessionId: string): void {
    this.onStreamingUpdate?.(sessionId, this.getStreamingState(sessionId));
  }
}

/** 全局单例 */
export const sessionManager = new SessionManager();
