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
import { resolveApiUrl } from '../../config';
import { Input } from '../../../components/ui/Input';
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

  useEffect(() => {
    // 只有当 agents 已经被加载（或至少 reloadKey 更新）时才拉取
    if (agents && agents.length > 0) {
      loadSessions();
    }
  }, [loadSessions, reloadKey, agents]);
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
    return resolveApiUrl(found?.icon_url) || null;
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
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5 px-2 py-1 text-[15px] font-semibold text-hx-text-primary">
            <MessageSquare className="w-[18px] h-[18px] text-hx-purple shrink-0" />
            <span>会话</span>
          </div>

          <button
            onClick={() => handleCreate()}
            className="hx-nav-item w-8 h-8 shrink-0 flex items-center justify-center p-0"
            title="新建对话"
          >
            <Plus className="w-[18px] h-[18px]" />
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
          <div className="hx-empty-state py-10">
            <Loader2 className="w-6 h-6 animate-spin text-hx-purple" />
          </div>
        ) : searchQuery && filteredSessions.length === 0 ? (
          <div className="hx-empty-state py-10">
            <MessageSquare className="w-8 h-8 opacity-40" />
            <p className="text-[13px] mt-2">未找到匹配的对话</p>
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
                    className="flex items-center gap-1.5 px-3 pt-2 pb-1 cursor-pointer select-none"
                  >
                    {isCollapsed ? (
                      <ChevronRight className="w-3.5 h-3.5 opacity-50 shrink-0" />
                    ) : (
                      <ChevronDown className="w-3.5 h-3.5 opacity-50 shrink-0" />
                    )}
                    {getAgentIcon(agentId) ? (
                      <img src={getAgentIcon(agentId)!} alt={displayName} className="w-3.5 h-3.5 rounded shrink-0 opacity-80 object-cover" />
                    ) : (
                      <Bot className="w-3.5 h-3.5 opacity-80 shrink-0" style={{ color: getAgentColor(agentId) }} />
                    )}
                    <span className="text-xs font-semibold text-hx-text-secondary flex-1 overflow-hidden text-ellipsis whitespace-nowrap uppercase tracking-wider">
                      {displayName}
                    </span>
                    {location === 'remote' && (
                      <span title="云端" className="shrink-0 leading-none">
                        <Cloud className="w-3 h-3 opacity-40" />
                      </span>
                    )}
                    {location === 'local' && (
                      <span title="本地" className="shrink-0 leading-none">
                        <Monitor className="w-3 h-3 opacity-40" />
                      </span>
                    )}
                    <span className="text-[11px] text-hx-text-tertiary shrink-0">
                      {agentSessions.length}
                    </span>
                    {isCollapsed && agentTypingCount > 0 && (
                      <Loader2 className="w-3 h-3 animate-spin text-[#FFD93D] shrink-0" />
                    )}
                    {isCollapsed && agentUnreadTotal > 0 && (
                      <span className="hx-conv-badge text-[10px] px-1.5 py-[1px]">
                        {agentUnreadTotal > 99 ? '99+' : agentUnreadTotal}
                      </span>
                    )}
                    {/* Quick-add button for this agent group */}
                    <button
                      onClick={(e) => { e.stopPropagation(); handleCreate(agentId); }}
                      className="bg-transparent border-none p-0.5 cursor-pointer text-hx-text-secondary hover:text-hx-text-primary opacity-50 hover:opacity-100 shrink-0 ml-auto"
                      title={`在 ${displayName} 下新建对话`}
                    >
                      <Plus className="w-3.5 h-3.5" />
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
                      <div className="hx-conv-avatar relative" style={{
                        background: getAgentIcon(session.agent_id) ? 'transparent' : `${getAgentColor(session.agent_id)}20`,
                        color: getAgentColor(session.agent_id),
                      }}>
                        {getAgentIcon(session.agent_id) ? (
                          <img src={getAgentIcon(session.agent_id)!} alt="agent" className="w-full h-full object-cover rounded-inherit" />
                        ) : (
                          <Bot className="w-[18px] h-[18px]" />
                        )}
                        {/* Connection status dot */}
                        <span 
                          className="absolute bottom-0 right-0 w-2 h-2 rounded-full border-2 border-[var(--hx-bg-panel,#1A1E2E)] transition-colors duration-300"
                          style={{
                            background: isTyping
                              ? 'var(--hx-yellow, #FFD93D)'
                              : isConnected
                                ? 'var(--hx-green, #22C55E)'
                                : 'var(--hx-text-tertiary, #6B7280)',
                          }} 
                          title={isTyping ? '思考中...' : isConnected ? '在线' : '离线'} 
                        />
                      </div>

                      {isEditing ? (
                        <div className="flex-1 flex items-center gap-1 min-w-0">
                          <Input
                            ref={editInputRef}
                            value={editTitle}
                            onChange={(e) => setEditTitle(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') handleSaveRename();
                              if (e.key === 'Escape') setEditingId(null);
                            }}
                            className="flex-1 px-2 py-1 !h-auto border-hx-purple"
                            autoFocus
                          />
                          <button onClick={handleSaveRename} className="text-hx-green p-0.5 bg-transparent border-none cursor-pointer">
                            <Check className="w-3.5 h-3.5" />
                          </button>
                          <button onClick={() => setEditingId(null)} className="text-hx-red p-0.5 bg-transparent border-none cursor-pointer">
                            <X className="w-3.5 h-3.5" />
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
