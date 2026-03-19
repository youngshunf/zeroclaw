/**
 * session-manager.ts — Multi-session WebSocket manager
 *
 * Manages multiple concurrent WS connections (one per session).
 * Supports switching the "active" session for display while keeping
 * background sessions alive for real-time updates.
 */

import { WebSocketClient, type WsMessageHandler } from './ws';
import type { WsMessage } from '../types/api';

export interface ChatMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp?: string;
}

export type SessionChangeHandler = (sessionId: string | null) => void;
export type MessageHandler = (sessionId: string, messages: ChatMessage[]) => void;
export type StatusHandler = (sessionId: string, status: 'connecting' | 'connected' | 'disconnected' | 'thinking') => void;

export class SessionManager {
  /** All active WS connections: session_id → client */
  private connections = new Map<string, WebSocketClient>();
  /** Message history per session */
  private histories = new Map<string, ChatMessage[]>();
  /** Connection status per session */
  private statuses = new Map<string, 'connecting' | 'connected' | 'disconnected' | 'thinking'>();
  /** Currently active (displayed) session */
  private _activeSessionId: string | null = null;
  /** Unread message count per session */
  private _unreadCounts = new Map<string, number>();

  // Event handlers
  public onSessionChange: SessionChangeHandler | null = null;
  public onMessage: MessageHandler | null = null;
  public onStatusChange: StatusHandler | null = null;

  get activeSessionId(): string | null {
    return this._activeSessionId;
  }

  /** Get message history for a session */
  getHistory(sessionId: string): ChatMessage[] {
    return this.histories.get(sessionId) ?? [];
  }

  /** Get connection status for a session */
  getStatus(sessionId: string): string {
    return this.statuses.get(sessionId) ?? 'disconnected';
  }

  /** Get unread count for a session */
  getUnreadCount(sessionId: string): number {
    return this._unreadCounts.get(sessionId) ?? 0;
  }

  /** Connect to a session (creates WS if not exists) */
  connectSession(sessionId: string): void {
    if (this.connections.has(sessionId)) return;

    const client = new WebSocketClient();
    this.statuses.set(sessionId, 'connecting');
    this.notifyStatus(sessionId);

    client.onOpen = () => {
      this.statuses.set(sessionId, 'connected');
      this.notifyStatus(sessionId);
    };

    client.onClose = () => {
      this.statuses.set(sessionId, 'disconnected');
      this.notifyStatus(sessionId);
    };

    client.onMessage = (msg: WsMessage) => {
      this.handleWsMessage(sessionId, msg);
    };

    // Override the session ID used by the WS client
    // We need to create the WS with the specific session_id
    this.connections.set(sessionId, client);

    // Connect with session-specific URL
    this.connectWithSessionId(client, sessionId);
  }

  /** Switch the active (displayed) session */
  switchTo(sessionId: string): void {
    this._activeSessionId = sessionId;
    this._unreadCounts.set(sessionId, 0);

    // Auto-connect if not connected
    if (!this.connections.has(sessionId)) {
      this.connectSession(sessionId);
    }

    this.onSessionChange?.(sessionId);
  }

  /** Send a message in the active session */
  sendMessage(content: string, sessionId?: string): void {
    const sid = sessionId ?? this._activeSessionId;
    if (!sid) throw new Error('No active session');

    const client = this.connections.get(sid);
    if (!client?.connected) throw new Error('Session not connected');

    // Add user message to history immediately
    const history = this.histories.get(sid) ?? [];
    history.push({ role: 'user', content, timestamp: new Date().toISOString() });
    this.histories.set(sid, history);
    this.notifyMessages(sid);

    // Update status to thinking
    this.statuses.set(sid, 'thinking');
    this.notifyStatus(sid);

    client.sendMessage(content);
  }

  /** Disconnect a specific session */
  disconnectSession(sessionId: string): void {
    const client = this.connections.get(sessionId);
    if (client) {
      client.disconnect();
      this.connections.delete(sessionId);
    }
    this.statuses.set(sessionId, 'disconnected');
    this.notifyStatus(sessionId);
  }

  /** Disconnect ALL sessions (used when switching Agent) */
  disconnectAll(): void {
    for (const [sid, client] of this.connections) {
      client.disconnect();
      this.statuses.set(sid, 'disconnected');
    }
    this.connections.clear();
    this.histories.clear();
    this._unreadCounts.clear();
    this._activeSessionId = null;
    this.onSessionChange?.(null);
  }

  /** Remove a session (disconnect + clear history) */
  removeSession(sessionId: string): void {
    this.disconnectSession(sessionId);
    this.histories.delete(sessionId);
    this._unreadCounts.delete(sessionId);

    if (this._activeSessionId === sessionId) {
      this._activeSessionId = null;
      this.onSessionChange?.(null);
    }
  }

  // ---------------------------------------------------------------------------
  // Internal
  // ---------------------------------------------------------------------------

  private connectWithSessionId(client: WebSocketClient, sessionId: string): void {
    // Override the connect method to use specific session_id
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const baseUrl = `${protocol}//${window.location.host}`;
    const token = sessionStorage.getItem('zeroclaw_token') ?? '';
    const url = `${baseUrl}/ws/chat?session_id=${encodeURIComponent(sessionId)}${token ? `&token=${encodeURIComponent(token)}` : ''}`;

    // Directly create a WebSocket with the correct session_id
    const ws = new WebSocket(url);

    ws.onopen = () => {
      this.statuses.set(sessionId, 'connected');
      this.notifyStatus(sessionId);
    };

    ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = JSON.parse(ev.data) as WsMessage;
        this.handleWsMessage(sessionId, msg);
      } catch {
        // Ignore
      }
    };

    ws.onclose = () => {
      this.statuses.set(sessionId, 'disconnected');
      this.notifyStatus(sessionId);
    };

    // Store the raw WebSocket reference for sending
    (client as any)._rawWs = ws;

    // Override sendMessage to use our WS
    const origSend = client.sendMessage.bind(client);
    client.sendMessage = (content: string) => {
      if (ws.readyState !== WebSocket.OPEN) {
        throw new Error('WebSocket is not connected');
      }
      ws.send(JSON.stringify({ type: 'message', content }));
    };

    // Override connected getter
    Object.defineProperty(client, 'connected', {
      get: () => ws.readyState === WebSocket.OPEN,
      configurable: true,
    });

    // Override disconnect
    client.disconnect = () => {
      ws.close();
    };
  }

  private handleWsMessage(sessionId: string, msg: WsMessage): void {
    const history = this.histories.get(sessionId) ?? [];

    switch (msg.type) {
      case 'history':
        // Server sends persisted history on connect
        if (Array.isArray(msg.messages)) {
          const restored: ChatMessage[] = msg.messages
            .filter((m: any) => m.role === 'user' || m.role === 'assistant')
            .map((m: any) => ({
              role: m.role as 'user' | 'assistant',
              content: m.content,
              timestamp: m.timestamp,
            }));
          this.histories.set(sessionId, restored);
          this.notifyMessages(sessionId);
        }
        break;

      case 'done':
        // Assistant reply complete
        history.push({
          role: 'assistant',
          content: msg.full_response ?? '',
          timestamp: new Date().toISOString(),
        });
        this.histories.set(sessionId, history);
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        this.notifyMessages(sessionId);

        // Increment unread if not active session
        if (sessionId !== this._activeSessionId) {
          this._unreadCounts.set(sessionId, (this._unreadCounts.get(sessionId) ?? 0) + 1);
        }
        break;

      case 'error':
        this.statuses.set(sessionId, 'connected');
        this.notifyStatus(sessionId);
        // Add error as system message
        history.push({
          role: 'system',
          content: `错误: ${msg.message ?? '未知错误'}`,
          timestamp: new Date().toISOString(),
        });
        this.histories.set(sessionId, history);
        this.notifyMessages(sessionId);
        break;
    }
  }

  private notifyMessages(sessionId: string): void {
    this.onMessage?.(sessionId, this.histories.get(sessionId) ?? []);
  }

  private notifyStatus(sessionId: string): void {
    this.onStatusChange?.(sessionId, this.statuses.get(sessionId) ?? 'disconnected');
  }
}

/** Global singleton */
export const sessionManager = new SessionManager();
