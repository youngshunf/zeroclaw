/**
 * SessionList.tsx — 唤星会话列表（按 Agent 分组）
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import {
  Plus,
  MessageSquare,
  Trash2,
  Pencil,
  Check,
  X,
  Loader2,
  Search,
  Bot,
  ChevronDown,
  ChevronRight,
  Cloud,
  Monitor,
} from 'lucide-react';
import {
  listSessions,
  createSession,
  deleteSession,
  renameSession,
  type SessionInfo,
} from '@/lib/session-api';
import type { AgentInfo } from '@/huanxing/lib/agent-api';

// ── Agent color palette ─────────────────────────────────────────────
const AGENT_COLORS = [
  '#7C3AED', // purple (default agent)
  '#06B6D4', // cyan
  '#F59E0B', // amber
  '#10B981', // emerald
  '#EF4444', // red
  '#8B5CF6', // violet
  '#EC4899', // pink
  '#3B82F6', // blue
  '#14B8A6', // teal
  '#F97316', // orange
];

/** Get a stable color for an agent based on its ID */
function getAgentColor(agentId: string): string {
  if (!agentId || agentId === 'default') return AGENT_COLORS[0];
  let hash = 0;
  for (let i = 0; i < agentId.length; i++) {
    hash = ((hash << 5) - hash + agentId.charCodeAt(i)) | 0;
  }
  return AGENT_COLORS[Math.abs(hash) % AGENT_COLORS.length];
}

interface SessionListProps {
  activeSessionId: string | null;
  onSelectSession: (sessionId: string, agentId?: string) => void;
  onCreateSession: (sessionId: string, agentId?: string) => void;
  onDeleteSession: (sessionId: string) => void;
  reloadKey?: number;
  unreadCounts?: Map<string, number>;
  /** Per-session typing indicator (true = agent is thinking/streaming) */
  typingMap?: Map<string, boolean>;
  /** Per-session connection status */
  connectedMap?: Map<string, boolean>;
  /** 所有可用 Agent 列表 */
  agents?: AgentInfo[];
  /** 每个会话最后一条消息预览 */
  lastMessages?: Map<string, string>;
  /** 实时更新的会话标题（LLM 自动生成后通过 SSE 推送） */
  sessionTitles?: Map<string, string>;
}

export default function SessionList({
  activeSessionId,
  onSelectSession,
  onCreateSession,
  onDeleteSession,
  reloadKey = 0,
  unreadCounts,
  typingMap,
  connectedMap,
  agents,
  lastMessages,
  sessionTitles,
}: SessionListProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [collapsedAgents, setCollapsedAgents] = useState<Set<string>>(new Set());
  const editInputRef = useRef<HTMLInputElement>(null);

  const loadSessions = useCallback(async () => {
    setLoading(true);
    try {
      const list = await listSessions();
      setSessions(list);
    } catch (err) {
      console.error('Failed to load sessions:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadSessions(); }, [loadSessions, reloadKey]);
  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  const handleCreate = async (forAgentId?: string) => {
    try {
      const agentId = forAgentId ?? 'default';
      const result = await createSession(undefined, agentId);
      await loadSessions();
      onCreateSession(result.session_id, result.agent_id);
    } catch (err) {
      console.error('Failed to create session:', err);
    }
  };

  const handleDelete = async (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    if (!confirm('确定删除这个对话吗？')) return;
    try {
      await deleteSession(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
      onDeleteSession(sessionId);
    } catch (err) {
      console.error('Failed to delete session:', err);
    }
  };

  const handleStartRename = (e: React.MouseEvent, session: SessionInfo) => {
    e.stopPropagation();
    setEditingId(session.id);
    setEditTitle(session.title);
  };

  const handleSaveRename = async () => {
    if (!editingId || !editTitle.trim()) { setEditingId(null); return; }
    try {
      await renameSession(editingId, editTitle.trim());
      setSessions((prev) =>
        prev.map((s) => s.id === editingId ? { ...s, title: editTitle.trim() } : s)
      );
    } catch (err) {
      console.error('Failed to rename session:', err);
    }
    setEditingId(null);
  };

  const toggleAgentCollapse = (agentId: string) => {
    setCollapsedAgents(prev => {
      const next = new Set(prev);
      if (next.has(agentId)) next.delete(agentId);
      else next.add(agentId);
      return next;
    });
  };

  /** Get display name for an agent */
  const getAgentDisplayName = (agentNameOrId: string) => {
    if (!agentNameOrId || agentNameOrId === 'default') return '默认 Agent';
    const found = agents?.find(a => a.name === agentNameOrId);
    if (found) return found.display_name || found.name;
    return agentNameOrId;
  };

  /** Get icon url for an agent */
  const getAgentIcon = (agentNameOrId: string) => {
    if (!agentNameOrId || agentNameOrId === 'default') return null;
    const found = agents?.find(a => a.name === agentNameOrId);
    return found?.icon_url || null;
  };

  // Filter sessions by search query
  const filteredSessions = searchQuery
    ? sessions.filter(s => s.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : sessions;

  // Group sessions by agent_id — always start from the agents list so every
  // agent appears even when it has zero sessions.
  const groupedSessions = useMemo(() => {
    // Map from agent name → sessions belonging to that agent
    const sessionsByAgent = new Map<string, SessionInfo[]>();
    for (const session of filteredSessions) {
      const aid = session.agent_id || 'default';
      if (!sessionsByAgent.has(aid)) sessionsByAgent.set(aid, []);
      sessionsByAgent.get(aid)!.push(session);
    }

    // Build groups from the known agents list first
    const knownAgentNames = new Set<string>();
    const groups: Array<{
      agentId: string;
      displayName: string;
      location: 'local' | 'remote' | null;
      sessions: SessionInfo[];
    }> = [];

    // Default agent always first
    const defaultEntry = agents?.find(a => a.name === 'default' || a.is_default);
    const defaultId = defaultEntry?.name ?? 'default';
    groups.push({
      agentId: defaultId,
      displayName: defaultEntry?.display_name || defaultEntry?.name || '默认 Agent',
      location: (defaultEntry?.location as 'local' | 'remote' | null) ?? null,
      sessions: sessionsByAgent.get(defaultId) ?? [],
    });
    knownAgentNames.add(defaultId);

    // Remaining agents sorted alphabetically
    const others = (agents ?? [])
      .filter(a => !knownAgentNames.has(a.name))
      .sort((a, b) => (a.display_name || a.name).localeCompare(b.display_name || b.name));
    for (const agent of others) {
      groups.push({
        agentId: agent.name,
        displayName: agent.display_name || agent.name,
        location: (agent.location as 'local' | 'remote' | null) ?? null,
        sessions: sessionsByAgent.get(agent.name) ?? [],
      });
      knownAgentNames.add(agent.name);
    }

    // Append any sessions whose agent_id is not in the known agents list
    // (stale/deleted agents) — show them at the end
    for (const [aid, slist] of sessionsByAgent) {
      if (!knownAgentNames.has(aid)) {
        groups.push({
          agentId: aid,
          displayName: aid,
          location: null,
          sessions: slist,
        });
      }
    }

    return groups;
  }, [filteredSessions, agents]);

  // Always show agent group headers when we have an agents list
  const hasMultipleAgents = (agents?.length ?? 0) > 0;

  return (
    <div className="hx-panel">
      {/* Header — 简洁标题 + 新建对话 */}
      <div className="hx-panel-header">
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            padding: '4px 8px',
            fontSize: 15,
            fontWeight: 600,
            color: 'var(--hx-text-primary)',
          }}>
            <MessageSquare size={18} style={{ color: 'var(--hx-purple)', flexShrink: 0 }} />
            <span>会话</span>
          </div>

          <button
            onClick={() => handleCreate()}
            className="hx-nav-item"
            style={{ width: 32, height: 32, flexShrink: 0 }}
            title="新建对话"
          >
            <Plus size={18} />
          </button>
        </div>
        <div className="hx-panel-search">
          <Search size={16} />
          <input
            type="text"
            placeholder="搜索对话..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
        </div>
      </div>

      {/* Session list — grouped by Agent */}
      <div className="hx-conv-list">
        {loading && sessions.length === 0 && (agents ?? []).length === 0 ? (
          <div className="hx-empty-state" style={{ padding: '40px 0' }}>
            <Loader2 size={24} className="animate-spin" style={{ color: 'var(--hx-purple)' }} />
          </div>
        ) : searchQuery && filteredSessions.length === 0 ? (
          <div className="hx-empty-state" style={{ padding: '40px 0' }}>
            <MessageSquare size={32} style={{ opacity: 0.4 }} />
            <p style={{ fontSize: 13 }}>未找到匹配的对话</p>
          </div>
        ) : (
          groupedSessions.map(({ agentId, displayName, location, sessions: agentSessions }) => {
            const isCollapsed = collapsedAgents.has(agentId);
            const agentUnreadTotal = agentSessions.reduce(
              (sum, s) => sum + (unreadCounts?.get(s.id) ?? 0), 0
            );
            const agentTypingCount = agentSessions.filter(
              s => typingMap?.get(s.id)
            ).length;

            return (
              <div key={agentId}>
                {/* Agent group header — only show when multiple agents exist */}
                {hasMultipleAgents && (
                  <div
                    onClick={() => toggleAgentCollapse(agentId)}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 6,
                      padding: '8px 12px 4px',
                      cursor: 'pointer',
                      userSelect: 'none',
                    }}
                  >
                    {isCollapsed ? (
                      <ChevronRight size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
                    ) : (
                      <ChevronDown size={14} style={{ opacity: 0.5, flexShrink: 0 }} />
                    )}
                    {getAgentIcon(agentId) ? (
                      <img src={getAgentIcon(agentId)!} alt={displayName} style={{ width: 14, height: 14, borderRadius: 4, flexShrink: 0, opacity: 0.8 }} />
                    ) : (
                      <Bot size={14} style={{ color: getAgentColor(agentId), opacity: 0.8, flexShrink: 0 }} />
                    )}
                    <span style={{
                      fontSize: 12,
                      fontWeight: 600,
                      color: 'var(--hx-text-secondary)',
                      flex: 1,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                      textTransform: 'uppercase',
                      letterSpacing: '0.5px',
                    }}>
                      {displayName}
                    </span>
                    {location === 'remote' && (
                      <span title="云端" style={{ flexShrink: 0, lineHeight: 0 }}>
                        <Cloud size={12} style={{ opacity: 0.4 }} />
                      </span>
                    )}
                    {location === 'local' && (
                      <span title="本地" style={{ flexShrink: 0, lineHeight: 0 }}>
                        <Monitor size={12} style={{ opacity: 0.4 }} />
                      </span>
                    )}
                    <span style={{
                      fontSize: 11,
                      color: 'var(--hx-text-tertiary)',
                      flexShrink: 0,
                    }}>
                      {agentSessions.length}
                    </span>
                    {isCollapsed && agentTypingCount > 0 && (
                      <Loader2 size={12} className="animate-spin" style={{ color: 'var(--hx-yellow, #FFD93D)', flexShrink: 0 }} />
                    )}
                    {isCollapsed && agentUnreadTotal > 0 && (
                      <span className="hx-conv-badge" style={{ fontSize: 10, padding: '1px 5px' }}>
                        {agentUnreadTotal > 99 ? '99+' : agentUnreadTotal}
                      </span>
                    )}
                    {/* Quick-add button for this agent group */}
                    <button
                      onClick={(e) => { e.stopPropagation(); handleCreate(agentId); }}
                      style={{
                        background: 'none',
                        border: 'none',
                        padding: 2,
                        cursor: 'pointer',
                        color: 'var(--hx-text-secondary)',
                        opacity: 0.5,
                        flexShrink: 0,
                      }}
                      title={`在 ${displayName} 下新建对话`}
                    >
                      <Plus size={14} />
                    </button>
                  </div>
                )}

                {/* Session items */}
                {!isCollapsed && agentSessions.map((session) => {
                  const isActive = session.id === activeSessionId;
                  const isEditing = session.id === editingId;
                  const unread = unreadCounts?.get(session.id) ?? 0;
                  const isTyping = typingMap?.get(session.id) ?? false;
                  const isConnected = connectedMap?.get(session.id) ?? false;

                  return (
                    <div
                      key={session.id}
                      onClick={() => !isEditing && onSelectSession(session.id, session.agent_id)}
                      className={`hx-conv-item${isActive ? ' active' : ''}`}
                    >
                      <div className="hx-conv-avatar" style={{
                        position: 'relative',
                        background: getAgentIcon(session.agent_id) ? 'transparent' : `${getAgentColor(session.agent_id)}20`,
                        color: getAgentColor(session.agent_id),
                      }}>
                        {getAgentIcon(session.agent_id) ? (
                          <img src={getAgentIcon(session.agent_id)!} alt="agent" style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: 'inherit' }} />
                        ) : (
                          <Bot size={18} />
                        )}
                        {/* Connection status dot */}
                        <span style={{
                          position: 'absolute',
                          bottom: 0,
                          right: 0,
                          width: 8,
                          height: 8,
                          borderRadius: '50%',
                          background: isTyping
                            ? 'var(--hx-yellow, #FFD93D)'
                            : isConnected
                              ? 'var(--hx-green, #22C55E)'
                              : 'var(--hx-text-tertiary, #6B7280)',
                          border: '2px solid var(--hx-bg-panel, #1A1E2E)',
                          transition: 'background 0.3s',
                        }} title={isTyping ? '思考中...' : isConnected ? '在线' : '离线'} />
                      </div>

                      {isEditing ? (
                        <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 4, minWidth: 0 }}>
                          <input
                            ref={editInputRef}
                            value={editTitle}
                            onChange={(e) => setEditTitle(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') handleSaveRename();
                              if (e.key === 'Escape') setEditingId(null);
                            }}
                            style={{
                              flex: 1, minWidth: 0, border: '1px solid var(--hx-purple)',
                              borderRadius: 6, padding: '4px 8px', fontSize: 13,
                              background: 'var(--hx-bg-main)', color: 'var(--hx-text-primary)',
                              outline: 'none',
                            }}
                          />
                          <button onClick={handleSaveRename} style={{ color: 'var(--hx-green)', padding: 2, background: 'none', border: 'none', cursor: 'pointer' }}>
                            <Check size={14} />
                          </button>
                          <button onClick={() => setEditingId(null)} style={{ color: 'var(--hx-red)', padding: 2, background: 'none', border: 'none', cursor: 'pointer' }}>
                            <X size={14} />
                          </button>
                        </div>
                      ) : (
                        <>
                          <div className="hx-conv-info">
                            <div className="hx-conv-name-row">
                              <span className="hx-conv-name">{sessionTitles?.get(session.id) || session.title}</span>
                            </div>
                            <div className="hx-conv-preview" style={isTyping ? { color: 'var(--hx-yellow, #FFD93D)', fontStyle: 'italic' } : undefined}>
                              {(() => {
                                if (isTyping) return '正在思考...';
                                let msg = lastMessages?.get(session.id);
                                if (!msg) return '点击开始对话';
                                msg = msg.replace(/\[IMAGE:[^\]]+\]/g, '[图片]');
                                return msg.length > 30 ? msg.slice(0, 30) + '...' : msg;
                              })()}
                            </div>
                          </div>
                          {unread > 0 && !isActive && (
                            <span className="hx-conv-badge">{unread > 99 ? '99+' : unread}</span>
                          )}
                          {/* Hover actions */}
                          <div className="hx-conv-actions" onClick={(e) => e.stopPropagation()}>
                            <button onClick={(e) => handleStartRename(e, session)} title="重命名">
                              <Pencil size={13} />
                            </button>
                            <button onClick={(e) => handleDelete(e, session.id)} title="删除">
                              <Trash2 size={13} />
                            </button>
                          </div>
                        </>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })
        )}
      </div>

    </div>
  );
}
