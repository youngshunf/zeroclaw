/**
 * ChatLayout.tsx — 唤星多会话聊天布局（适配新三栏 UI）
 *
 * 左侧会话列表（hx-panel）+ 右侧聊天区（hx-chat）
 *
 * 多会话实时在线：所有会话在页面加载时自动建立 WS 连接，
 * 后台会话持续接收消息并产生未读提醒。
 */

import { useState, useEffect, useCallback, useRef, lazy, Suspense } from 'react';
import { getToken } from '@/lib/auth';
import type { WsMessage } from '@/types/api';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { listAgents, switchAgent, type AgentInfo } from '@/huanxing/lib/agent-api';
import { listSessions, getSessionMessages, generateSessionTitle, type SessionInfo } from '@/lib/session-api';
import { Search, Plus, Bot, Send, Paperclip, Smile } from 'lucide-react';
import { Markdown } from '@/huanxing/components/markdown';
import { StreamingBubble } from '@/huanxing/components/chat/StreamingBubble';

const SessionList = lazy(() => import('@/huanxing/components/chat/SessionList'));

interface ChatMessage {
  id: string;
  role: 'user' | 'agent';
  content: string;
  timestamp: Date;
}

let msgCounter = 0;
function makeId(): string {
  return globalThis.crypto?.randomUUID?.() ??
    `msg_${Date.now().toString(36)}_${(++msgCounter).toString(36)}`;
}

// ── 常量 ──────────────────────────────────────────────────────────
/** 页面加载时最多自动连接的会话数（按最近活跃排序） */
const MAX_AUTO_CONNECT = 20;
/** 心跳间隔（毫秒） */
const HEARTBEAT_INTERVAL_MS = 30_000;
/** 重连初始延迟（毫秒） */
const RECONNECT_BASE_MS = 1_000;
/** 重连最大延迟（毫秒） */
const RECONNECT_MAX_MS = 30_000;

// ── WS 连接状态 ──────────────────────────────────────────────────
interface SessionWsState {
  ws: WebSocket;
  connected: boolean;
  agentId?: string;
  /** 调用此函数主动关闭（不触发自动重连） */
  intentionalClose: () => void;
}

export default function ChatLayout() {
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [activeAgent, setActiveAgentLocal] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [, setGlobalAgent] = useActiveAgent();
  const [reloadKey, setReloadKey] = useState(0);
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const setActiveAgent = useCallback((name: string | null) => {
    setActiveAgentLocal(name);
    setGlobalAgent(name);
  }, [setGlobalAgent]);

  // 加载 Agent 列表（带重试，等待 sidecar 就绪）
  const loadAgents = useCallback(async (retries = 5) => {
    try {
      const result = await listAgents();
      setAgents(result.agents);
      setActiveAgent(result.current);
    } catch {
      if (retries > 0) {
        setTimeout(() => loadAgents(retries - 1), 2000);
      }
    }
  }, [setActiveAgent]);

  useEffect(() => { loadAgents(); }, [loadAgents]);

  // 切换 Agent
  const handleSwitchAgent = useCallback(async (name: string) => {
    try {
      await switchAgent(name);
      setActiveAgent(name);
    } catch (err) {
      console.error('Failed to switch agent:', err);
    }
  }, [setActiveAgent]);

  // WS state
  const wsMapRef = useRef(new Map<string, SessionWsState>());
  /** 跟踪各会话的重连定时器，组件卸载时清理 */
  const reconnectTimersRef = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const [histories, setHistories] = useState(new Map<string, ChatMessage[]>());
  const [connectedMap, setConnectedMap] = useState(new Map<string, boolean>());
  const [unreadCounts, setUnreadCounts] = useState(new Map<string, number>());
  const [typingMap, setTypingMap] = useState(new Map<string, boolean>());
  /** 最后一条消息预览（用于会话列表） */
  const [lastMessages, setLastMessages] = useState(new Map<string, string>());
  const pendingContentRef = useRef(new Map<string, string>());
  const [streamingContent, setStreamingContent] = useState(new Map<string, string>());
  const [progressLines, setProgressLines] = useState(new Map<string, string[]>());
  /** 各会话标题缓存 */
  const [sessionTitles, setSessionTitles] = useState(new Map<string, string>());
  /** 各会话的 has_more 状态（是否还有更早的历史） */
  const hasMoreRef = useRef(new Map<string, boolean>());
  /** 各会话的最小消息 ID（用于分页 cursor） */
  const oldestIdRef = useRef(new Map<string, number>());
  /** 正在加载历史的会话（防重入） */
  const loadingHistoryRef = useRef(new Set<string>());
  /** 已经通过 REST 加载过初始历史的会话 */
  const loadedSessionsRef = useRef(new Set<string>());
  /** 已经触发过标题生成的会话 */
  const titleGeneratedRef = useRef(new Set<string>());

  const activeSessionIdRef = useRef<string | null>(null);
  activeSessionIdRef.current = activeSessionId;
  /** 组件是否已卸载 */
  const unmountedRef = useRef(false);

  // Auto scroll
  const currentMessages = activeSessionId ? (histories.get(activeSessionId) ?? []) : [];
  const currentConnected = activeSessionId ? (connectedMap.get(activeSessionId) ?? false) : false;
  const currentTyping = activeSessionId ? (typingMap.get(activeSessionId) ?? false) : false;
  const currentStreamingContent = activeSessionId ? (streamingContent.get(activeSessionId) ?? '') : '';
  const currentProgressLines = activeSessionId ? (progressLines.get(activeSessionId) ?? []) : [];

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [currentMessages, currentTyping, currentStreamingContent, currentProgressLines]);

  // ── 通过 REST API 加载会话历史（首次加载 + 上拉加载更多）──
  const loadSessionHistory = useCallback(async (sessionId: string, agentId?: string, loadMore = false) => {
    if (loadingHistoryRef.current.has(sessionId)) return;
    if (!loadMore && loadedSessionsRef.current.has(sessionId)) return;

    loadingHistoryRef.current.add(sessionId);
    try {
      const before = loadMore ? oldestIdRef.current.get(sessionId) : undefined;
      const result = await getSessionMessages(sessionId, {
        limit: 30,
        before,
        agentId,
      });

      // 更新标题
      if (result.title && result.title !== '新对话') {
        setSessionTitles(prev => new Map(prev).set(sessionId, result.title));
      }

      const newMessages: ChatMessage[] = result.messages.map(m => ({
        id: `db_${m.id}`,
        role: m.role === 'user' ? 'user' as const : 'agent' as const,
        content: m.content,
        timestamp: new Date(m.timestamp),
      }));

      if (loadMore) {
        // 上拉加载更多：插入到列表顶部
        setHistories(prev => {
          const existing = prev.get(sessionId) ?? [];
          // 去重：过滤掉已有的 db_ 开头的 id
          const existingIds = new Set(existing.map(m => m.id));
          const unique = newMessages.filter(m => !existingIds.has(m.id));
          return new Map(prev).set(sessionId, [...unique, ...existing]);
        });
      } else {
        // 首次加载：设置历史（如果已有来自 WS 的消息则合并）
        setHistories(prev => {
          const existing = prev.get(sessionId) ?? [];
          // 保留非 db_ 开头的消息（来自 WS 实时推送的）
          const wsOnly = existing.filter(m => !m.id.startsWith('db_'));
          return new Map(prev).set(sessionId, [...newMessages, ...wsOnly]);
        });
        loadedSessionsRef.current.add(sessionId);
      }

      hasMoreRef.current.set(sessionId, result.has_more);
      if (result.oldest_id != null) {
        oldestIdRef.current.set(sessionId, result.oldest_id);
      }

      // 更新最后消息预览
      if (newMessages.length > 0) {
        const last = newMessages[newMessages.length - 1];
        setLastMessages(prev => {
          if (!prev.has(sessionId)) {
            return new Map(prev).set(sessionId, last.content);
          }
          return prev;
        });
      }
    } catch (err) {
      console.warn('[ChatLayout] Failed to load session history:', err);
    } finally {
      loadingHistoryRef.current.delete(sessionId);
    }
  }, []);

  // ── handleMessage（提前声明，connectSession 和 effect 都要用） ──
  const handleMessage = useCallback((sessionId: string, msg: WsMessage) => {
    switch (msg.type) {
      case 'history': {
        // WS history 仅作为 fallback（如果 REST 已加载过则跳过）
        if (loadedSessionsRef.current.has(sessionId)) break;
        const restored: ChatMessage[] = (msg.messages ?? [])
          .filter((e: any) => e.content?.trim())
          .map((e: any): ChatMessage => ({
            id: makeId(),
            role: e.role === 'user' ? 'user' : 'agent',
            content: e.content.trim(),
            timestamp: new Date(),
          }));
        setHistories(prev => new Map(prev).set(sessionId, restored));
        setTypingMap(prev => new Map(prev).set(sessionId, false));
        pendingContentRef.current.set(sessionId, '');
        setStreamingContent(prev => new Map(prev).set(sessionId, ''));
        setProgressLines(prev => new Map(prev).set(sessionId, []));
        // 更新最后消息预览
        if (restored.length > 0) {
          const last = restored[restored.length - 1];
          setLastMessages(prev => new Map(prev).set(sessionId, last.content));
        }
        break;
      }
      case 'chunk': {
        setTypingMap(prev => new Map(prev).set(sessionId, true));
        const cur = pendingContentRef.current.get(sessionId) ?? '';
        const newContent = cur + (msg.content ?? '');
        pendingContentRef.current.set(sessionId, newContent);
        setStreamingContent(prev => new Map(prev).set(sessionId, newContent));
        break;
      }
      case 'progress': {
        setTypingMap(prev => new Map(prev).set(sessionId, true));
        setProgressLines(prev => {
          const lines = [...(prev.get(sessionId) ?? []), msg.content ?? ''];
          return new Map(prev).set(sessionId, lines);
        });
        break;
      }
      case 'progress_block': {
        setTypingMap(prev => new Map(prev).set(sessionId, true));
        setProgressLines(prev => {
          const blockContent = msg.content ?? '';
          const lines = blockContent.split('\n').filter((l: string) => l.trim());
          return new Map(prev).set(sessionId, lines);
        });
        break;
      }
      case 'progress_clear': {
        setProgressLines(prev => new Map(prev).set(sessionId, []));
        break;
      }
      case 'message':
      case 'done': {
        const pending = pendingContentRef.current.get(sessionId) ?? '';
        const content = (msg.full_response ?? msg.content ?? pending).trim();
        const finalContent = content || '(工具执行完成，无文本输出)';
        setHistories(prev => {
          const h = [...(prev.get(sessionId) ?? [])];
          h.push({ id: makeId(), role: 'agent', content: finalContent, timestamp: new Date() });
          return new Map(prev).set(sessionId, h);
        });
        pendingContentRef.current.set(sessionId, '');
        setTypingMap(prev => new Map(prev).set(sessionId, false));
        setStreamingContent(prev => new Map(prev).set(sessionId, ''));
        setProgressLines(prev => new Map(prev).set(sessionId, []));
        // 未读计数：非当前活跃会话 +1
        if (activeSessionIdRef.current !== sessionId) {
          setUnreadCounts(prev => {
            const n = new Map(prev);
            n.set(sessionId, (n.get(sessionId) ?? 0) + 1);
            return n;
          });
        }
        // 更新最后消息预览
        setLastMessages(prev => new Map(prev).set(sessionId, finalContent));
        // ── 自动生成标题 ──
        // 条件：标题是"新对话"且至少有 2 条消息，且未触发过
        if (!titleGeneratedRef.current.has(sessionId)) {
          setHistories(prev => {
            const h = prev.get(sessionId) ?? [];
            setSessionTitles(prevTitles => {
              const currentTitle = prevTitles.get(sessionId);
              if ((!currentTitle || currentTitle === '新对话') && h.length >= 2) {
                titleGeneratedRef.current.add(sessionId);
                // 异步调用，不阻塞
                generateSessionTitle(sessionId).then(res => {
                  if (res.title) {
                    setSessionTitles(pt => new Map(pt).set(sessionId, res.title));
                  }
                }).catch(err => console.warn('[ChatLayout] Title generation failed:', err));
              }
              return prevTitles;
            });
            return prev;
          });
        }
        break;
      }
      case 'error': {
        const errContent = `[错误] ${msg.message ?? '未知错误'}`;
        setHistories(prev => {
          const h = [...(prev.get(sessionId) ?? [])];
          h.push({ id: makeId(), role: 'agent', content: errContent, timestamp: new Date() });
          return new Map(prev).set(sessionId, h);
        });
        setTypingMap(prev => new Map(prev).set(sessionId, false));
        pendingContentRef.current.set(sessionId, '');
        setStreamingContent(prev => new Map(prev).set(sessionId, ''));
        setProgressLines(prev => new Map(prev).set(sessionId, []));
        break;
      }
    }
  }, []);

  // ── WS 连接（含自动重连） ──────────────────────────────────────
  const connectSession = useCallback((sessionId: string, agentId?: string) => {
    // 已有连接则跳过
    if (wsMapRef.current.has(sessionId)) return;
    if (unmountedRef.current) return;

    let reconnectDelay = RECONNECT_BASE_MS;
    let intentionallyClosed = false;

    const doConnect = () => {
      if (unmountedRef.current || intentionallyClosed) return;
      // 清理可能存在的旧定时器
      const oldTimer = reconnectTimersRef.current.get(sessionId);
      if (oldTimer) { clearTimeout(oldTimer); reconnectTimersRef.current.delete(sessionId); }

      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const token = getToken() ?? '';
      const params = new URLSearchParams();
      params.set('session_id', sessionId);
      if (agentId) params.set('agent_id', agentId);
      if (token) params.set('token', token);
      const url = `${protocol}//${window.location.host}/ws/chat?${params.toString()}`;
      const ws = new WebSocket(url);

      const entry: SessionWsState = {
        ws,
        connected: false,
        agentId,
        intentionalClose: () => {
          intentionallyClosed = true;
          const timer = reconnectTimersRef.current.get(sessionId);
          if (timer) { clearTimeout(timer); reconnectTimersRef.current.delete(sessionId); }
          ws.onopen = null;
          ws.onclose = null;
          ws.onerror = null;
          ws.onmessage = null;
          ws.close();
          wsMapRef.current.delete(sessionId);
        },
      };
      wsMapRef.current.set(sessionId, entry);

      ws.onopen = () => {
        reconnectDelay = RECONNECT_BASE_MS; // 重置退避
        const e = wsMapRef.current.get(sessionId);
        if (e) e.connected = true;
        setConnectedMap(prev => new Map(prev).set(sessionId, true));
      };

      ws.onclose = () => {
        wsMapRef.current.delete(sessionId);
        setConnectedMap(prev => new Map(prev).set(sessionId, false));
        // 自动重连（除非主动关闭或组件已卸载）
        if (!intentionallyClosed && !unmountedRef.current) {
          const timer = setTimeout(() => {
            reconnectTimersRef.current.delete(sessionId);
            doConnect();
          }, reconnectDelay);
          reconnectTimersRef.current.set(sessionId, timer);
          reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_MAX_MS);
        }
      };

      ws.onerror = () => { /* onclose 会紧随其后触发，在那里处理重连 */ };

      ws.onmessage = (ev: MessageEvent) => {
        try {
          const msg = JSON.parse(ev.data) as WsMessage;
          if ((msg as any).type === 'pong') return; // 心跳响应，忽略
          handleMessage(sessionId, msg);
        } catch { /* ignore */ }
      };
    };

    doConnect();
  }, [handleMessage]);

  // ── 主动断开单个会话 ──────────────────────────────────────────
  const disconnectSession = useCallback((sessionId: string) => {
    const entry = wsMapRef.current.get(sessionId);
    if (entry) {
      entry.intentionalClose();
    }
    // 也清理可能残留的重连定时器
    const timer = reconnectTimersRef.current.get(sessionId);
    if (timer) { clearTimeout(timer); reconnectTimersRef.current.delete(sessionId); }
  }, []);

  // ── 断开全部（仅在组件卸载时使用） ────────────────────────────
  const disconnectAll = useCallback(() => {
    for (const [sid] of wsMapRef.current) disconnectSession(sid);
    for (const [, timer] of reconnectTimersRef.current) clearTimeout(timer);
    reconnectTimersRef.current.clear();
    setHistories(new Map());
    setConnectedMap(new Map());
    setUnreadCounts(new Map());
    setTypingMap(new Map());
    setLastMessages(new Map());
    pendingContentRef.current.clear();
  }, [disconnectSession]);

  // ── 页面加载：等待 sidecar 就绪后自动连接所有已有会话 ────────
  useEffect(() => {
    let cancelled = false;
    const MAX_RETRIES = 10;
    const RETRY_DELAY_MS = 2000;

    const attemptConnect = async (attempt: number) => {
      if (cancelled) return;
      try {
        const sessions = await listSessions();
        if (cancelled) return;
        // 按最近更新排序，取前 MAX_AUTO_CONNECT 个
        const sorted = [...sessions].sort((a, b) =>
          new Date(b.updated_at ?? b.created_at ?? 0).getTime() -
          new Date(a.updated_at ?? a.created_at ?? 0).getTime()
        );
        for (const session of sorted.slice(0, MAX_AUTO_CONNECT)) {
          connectSession(session.id, session.agent_id);
        }
        // 也刷新一下 Agent 列表
        loadAgents();
      } catch (err) {
        if (cancelled) return;
        if (attempt < MAX_RETRIES) {
          console.warn(`[ChatLayout] Sidecar not ready (attempt ${attempt}/${MAX_RETRIES}), retrying in ${RETRY_DELAY_MS}ms...`);
          setTimeout(() => attemptConnect(attempt + 1), RETRY_DELAY_MS);
        } else {
          console.error('[ChatLayout] Failed to connect after max retries:', err);
        }
      }
    };

    attemptConnect(1);
    return () => { cancelled = true; };
  }, [connectSession, loadAgents]);

  // ── 心跳保活：每 30 秒向所有活跃 WS 发送 ping ────────────────
  useEffect(() => {
    const interval = setInterval(() => {
      for (const [, entry] of wsMapRef.current) {
        if (entry.ws.readyState === WebSocket.OPEN) {
          entry.ws.send(JSON.stringify({ type: 'ping' }));
        }
      }
    }, HEARTBEAT_INTERVAL_MS);
    return () => clearInterval(interval);
  }, []);

  const handleSelectSession = useCallback((sessionId: string, agentId?: string) => {
    setActiveSessionId(sessionId);
    if (agentId) setActiveAgent(agentId);
    // 清除该会话的未读计数
    setUnreadCounts(prev => { const n = new Map(prev); n.delete(sessionId); return n; });
    // 确保连接（可能还没自动连接到，比如新加载的会话）
    connectSession(sessionId, agentId);
    // 通过 REST API 加载历史消息（首次）
    loadSessionHistory(sessionId, agentId);
  }, [connectSession, setActiveAgent, loadSessionHistory]);

  const handleCreateSession = useCallback((sessionId: string, agentId?: string) => {
    connectSession(sessionId, agentId);
    setActiveSessionId(sessionId);
    if (agentId) setActiveAgent(agentId);
  }, [connectSession, setActiveAgent]);

  const handleDeleteSession = useCallback((sessionId: string) => {
    disconnectSession(sessionId);
    setHistories(prev => { const n = new Map(prev); n.delete(sessionId); return n; });
    setUnreadCounts(prev => { const n = new Map(prev); n.delete(sessionId); return n; });
    setLastMessages(prev => { const n = new Map(prev); n.delete(sessionId); return n; });
    if (activeSessionIdRef.current === sessionId) setActiveSessionId(null);
  }, [disconnectSession]);

  const handleSendMessage = useCallback(() => {
    const trimmed = input.trim();
    const sid = activeSessionIdRef.current;
    if (!trimmed || !sid) return;
    const entry = wsMapRef.current.get(sid);
    if (!entry || !entry.connected) return;

    setHistories(prev => {
      const h = [...(prev.get(sid) ?? [])];
      h.push({ id: makeId(), role: 'user', content: trimmed, timestamp: new Date() });
      return new Map(prev).set(sid, h);
    });
    setTypingMap(prev => new Map(prev).set(sid, true));
    pendingContentRef.current.set(sid, '');
    setStreamingContent(prev => new Map(prev).set(sid, ''));
    setProgressLines(prev => new Map(prev).set(sid, []));
    // 更新最后消息预览
    setLastMessages(prev => new Map(prev).set(sid, trimmed));
    entry.ws.send(JSON.stringify({ type: 'message', content: trimmed }));
    setInput('');
    textareaRef.current?.focus();
  }, [input]);

  // ── SSE: Agent 切换 + 会话标题更新 ─────────────────────────
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    import('@/huanxing/lib/sse-events').then(({ connectSseEvents }) => {
      cleanup = connectSseEvents({
        onAgentSwitched: ({ agent }) => {
          // ✅ 不再 disconnectAll，只切换前台显示
          setActiveAgent(agent);
          setReloadKey(k => k + 1);
          loadAgents();
        },
        onSessionUpdated: ({ session_id, title }) => {
          if (title) {
            setSessionTitles(prev => new Map(prev).set(session_id, title));
          }
        },
      });
    });
    return () => cleanup?.();
  }, [setActiveAgent, loadAgents]);

  // 组件卸载时断开全部
  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
      disconnectAll();
    };
  }, [disconnectAll]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  // ── 上拉加载更多历史（ref callback for scroll container） ──
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const handleMessagesScroll = useCallback(() => {
    const el = messagesContainerRef.current;
    if (!el || !activeSessionId) return;
    // 滚动到顶部附近（距离 < 100px）时触发加载更多
    if (el.scrollTop < 100 && hasMoreRef.current.get(activeSessionId)) {
      const prevScrollHeight = el.scrollHeight;
      loadSessionHistory(activeSessionId, activeAgent ?? undefined, true).then(() => {
        // 加载后保持滚动位置
        requestAnimationFrame(() => {
          if (el) {
            el.scrollTop = el.scrollHeight - prevScrollHeight;
          }
        });
      });
    }
  }, [activeSessionId, activeAgent, loadSessionHistory]);

  // 当前会话标题
  const activeSessionTitle = activeSessionId ? (sessionTitles.get(activeSessionId) ?? '新对话') : '';

  // Resolve display name for the active agent
  const activeAgentDisplayName = (() => {
    if (!activeAgent) return 'AI';
    const found = agents.find(a => a.name === activeAgent);
    if (found?.display_name) return found.display_name;
    return activeAgent === 'default' ? '默认 Agent' : activeAgent;
  })();

  return (
    <>
      {/* 会话列表 Panel */}
      <Suspense fallback={<div className="hx-panel" />}>
        <SessionList
          activeSessionId={activeSessionId}
          onSelectSession={handleSelectSession}
          onCreateSession={handleCreateSession}
          onDeleteSession={handleDeleteSession}
          reloadKey={reloadKey}
          unreadCounts={unreadCounts}
          typingMap={typingMap}
          connectedMap={connectedMap}
          agents={agents}
          lastMessages={lastMessages}
          sessionTitles={sessionTitles}
        />
      </Suspense>

      {/* 聊天区 */}
      <div className="hx-chat">
        {/* Chat header */}
        {activeSessionId && activeAgent && (
          <div className="hx-chat-header">
            <div className="hx-chat-header-left">
              <div className="hx-chat-header-avatar">
                <Bot size={18} />
              </div>
              <div className="hx-chat-header-info">
                <h3>{activeAgentDisplayName}</h3>
                <span className="hx-chat-header-subtitle">{activeSessionTitle}</span>
                <div className="hx-chat-header-status">
                  <span className="dot" />
                  {currentConnected ? '在线' : '连接中...'}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Messages */}
        <div className="hx-messages" ref={messagesContainerRef} onScroll={handleMessagesScroll}>
          {/* 上拉加载提示 */}
          {activeSessionId && hasMoreRef.current.get(activeSessionId) && (
            <div className="hx-load-more-hint" style={{ textAlign: 'center', padding: '8px', opacity: 0.5, fontSize: '12px' }}>
              ↑ 上拉加载更多历史消息
            </div>
          )}
          {!activeSessionId ? (
            <div className="hx-empty-state">
              <div className="icon">💬</div>
              <h3>选择或创建一个对话</h3>
              <p>点击左侧 "+" 开始新对话，与 AI Agent 进行交互</p>
            </div>
          ) : currentMessages.length === 0 && !currentTyping ? (
            <div className="hx-empty-state">
              <div className="icon">✨</div>
              <h3>新对话</h3>
              <p>发送消息开始与 {activeAgentDisplayName} 聊天</p>
            </div>
          ) : null}

          {currentMessages.map((msg) => (
            <div key={msg.id} className={`hx-msg ${msg.role === 'user' ? 'user' : 'agent'}`}>
              <div className="hx-msg-avatar">
                {msg.role === 'user' ? '杨' : <Bot size={16} />}
              </div>
              <div className="hx-msg-content">
                <div className="hx-msg-bubble">
                  {msg.role === 'user' ? (
                    <p style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{msg.content}</p>
                  ) : (
                    <Markdown mode="minimal">{msg.content}</Markdown>
                  )}
                </div>
                <span className="hx-msg-time">{msg.timestamp.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}</span>
              </div>
            </div>
          ))}

          {currentTyping && (currentStreamingContent || currentProgressLines.length > 0) ? (
            <StreamingBubble
              content={currentStreamingContent}
              progressLines={currentProgressLines}
              isStreaming={true}
              agentName={activeAgent ?? undefined}
            />
          ) : currentTyping ? (
            <div className="hx-msg agent">
              <div className="hx-msg-avatar"><Bot size={16} /></div>
              <div className="hx-msg-content">
                <div className="hx-typing-dots">
                  <span /><span /><span />
                </div>
              </div>
            </div>
          ) : null}

          <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="hx-chat-input-area">
          <div className="hx-chat-input-wrapper">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={
                !activeSessionId ? '请先选择或创建一个对话' :
                currentConnected ? '输入消息... (Enter 发送)' : '连接中...'
              }
              disabled={!currentConnected || !activeSessionId}
              rows={1}
            />
            <div className="hx-chat-input-actions">
              <button title="附件"><Paperclip size={18} /></button>
              <button title="表情"><Smile size={18} /></button>
              <button
                className="hx-send-btn"
                onClick={handleSendMessage}
                disabled={!currentConnected || !input.trim() || !activeSessionId}
                title="发送"
              >
                <Send size={18} />
              </button>
            </div>
          </div>
          <div className="hx-input-hint">
            <span className="dot" style={{ background: currentConnected ? 'var(--hx-green)' : activeSessionId ? 'var(--hx-amber)' : 'var(--hx-red)' }} />
            {currentConnected ? `${activeAgentDisplayName} · 已连接` : activeSessionId ? '连接中...' : '未选择对话'}
          </div>
        </div>
      </div>
    </>
  );
}
