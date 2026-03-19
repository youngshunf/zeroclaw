/**
 * SessionList.tsx — 会话列表侧边栏
 *
 * 显示当前 Agent 的所有会话，支持：
 * - 创建新会话
 * - 切换会话
 * - 重命名会话（双击标题）
 * - 删除会话
 * - 未读消息角标
 */

import { useState, useEffect, useRef, useCallback } from 'react';
import {
  Plus,
  MessageSquare,
  Trash2,
  Pencil,
  Check,
  X,
  Loader2,
} from 'lucide-react';
import {
  listSessions,
  createSession,
  deleteSession,
  renameSession,
  type SessionInfo,
} from '@/lib/session-api';

interface SessionListProps {
  activeSessionId: string | null;
  onSelectSession: (sessionId: string) => void;
  onCreateSession: (sessionId: string) => void;
  onDeleteSession: (sessionId: string) => void;
  /** Trigger reload from outside (e.g., agent switch) */
  reloadKey?: number;
  /** Unread counts per session */
  unreadCounts?: Map<string, number>;
}

export default function SessionList({
  activeSessionId,
  onSelectSession,
  onCreateSession,
  onDeleteSession,
  reloadKey = 0,
  unreadCounts,
}: SessionListProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState('');
  const editInputRef = useRef<HTMLInputElement>(null);

  // Load sessions
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
    loadSessions();
  }, [loadSessions, reloadKey]);

  // Focus edit input
  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  // Create new session
  const handleCreate = async () => {
    try {
      const result = await createSession();
      await loadSessions();
      onCreateSession(result.session_id);
    } catch (err) {
      console.error('Failed to create session:', err);
    }
  };

  // Delete session
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

  // Start rename
  const handleStartRename = (e: React.MouseEvent, session: SessionInfo) => {
    e.stopPropagation();
    setEditingId(session.id);
    setEditTitle(session.title);
  };

  // Save rename
  const handleSaveRename = async () => {
    if (!editingId || !editTitle.trim()) {
      setEditingId(null);
      return;
    }
    try {
      await renameSession(editingId, editTitle.trim());
      setSessions((prev) =>
        prev.map((s) =>
          s.id === editingId ? { ...s, title: editTitle.trim() } : s
        )
      );
    } catch (err) {
      console.error('Failed to rename session:', err);
    }
    setEditingId(null);
  };

  // Cancel rename
  const handleCancelRename = () => {
    setEditingId(null);
  };

  return (
    <div className="flex flex-col h-full border-r border-[#1e2f5d] bg-[#050b1a]/90 w-[240px]">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-3 border-b border-[#1e2f5d]">
        <span className="text-sm font-medium text-[#9bb7eb]">对话列表</span>
        <button
          onClick={handleCreate}
          className="p-1.5 rounded-lg text-[#8bb9ff] hover:bg-[#7c3aed]/20 hover:text-white transition-colors"
          title="新建对话"
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto py-1">
        {loading && sessions.length === 0 ? (
          <div className="flex items-center justify-center py-8 text-[#5f84cc]">
            <Loader2 className="h-5 w-5 animate-spin" />
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-8 text-[#5f84cc]">
            <MessageSquare className="h-8 w-8 mb-2 opacity-50" />
            <p className="text-xs">暂无对话</p>
            <button
              onClick={handleCreate}
              className="mt-2 text-xs text-[#7c3aed] hover:text-[#9b5de5] transition-colors"
            >
              创建第一个对话
            </button>
          </div>
        ) : (
          sessions.map((session) => {
            const isActive = session.id === activeSessionId;
            const isEditing = session.id === editingId;
            const unread = unreadCounts?.get(session.id) ?? 0;

            return (
              <div
                key={session.id}
                onClick={() => !isEditing && onSelectSession(session.id)}
                className={[
                  'group flex items-center gap-2 mx-1.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all duration-200',
                  isActive
                    ? 'bg-[#7c3aed]/15 border border-[#7c3aed]/40 text-white'
                    : 'border border-transparent text-[#9bb7eb] hover:bg-[#07132f] hover:text-white',
                ].join(' ')}
              >
                <MessageSquare className="h-4 w-4 shrink-0 opacity-60" />

                {isEditing ? (
                  <div className="flex-1 flex items-center gap-1 min-w-0">
                    <input
                      ref={editInputRef}
                      value={editTitle}
                      onChange={(e) => setEditTitle(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') handleSaveRename();
                        if (e.key === 'Escape') handleCancelRename();
                      }}
                      className="flex-1 min-w-0 bg-[#0a1b3f] border border-[#2c4e97] rounded px-1.5 py-0.5 text-xs text-white focus:outline-none focus:border-[#7c3aed]"
                    />
                    <button onClick={handleSaveRename} className="p-0.5 text-green-400 hover:text-green-300">
                      <Check className="h-3.5 w-3.5" />
                    </button>
                    <button onClick={handleCancelRename} className="p-0.5 text-red-400 hover:text-red-300">
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ) : (
                  <>
                    <span className="flex-1 text-xs truncate min-w-0">
                      {session.title}
                    </span>

                    {/* Unread badge */}
                    {unread > 0 && !isActive && (
                      <span className="shrink-0 bg-[#7c3aed] text-white text-[10px] font-bold rounded-full min-w-[18px] h-[18px] flex items-center justify-center px-1">
                        {unread > 99 ? '99+' : unread}
                      </span>
                    )}

                    {/* Action buttons (visible on hover) */}
                    <div className="shrink-0 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button
                        onClick={(e) => handleStartRename(e, session)}
                        className="p-0.5 text-[#5f84cc] hover:text-white transition-colors"
                        title="重命名"
                      >
                        <Pencil className="h-3 w-3" />
                      </button>
                      <button
                        onClick={(e) => handleDelete(e, session.id)}
                        className="p-0.5 text-[#5f84cc] hover:text-red-400 transition-colors"
                        title="删除"
                      >
                        <Trash2 className="h-3 w-3" />
                      </button>
                    </div>
                  </>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Session count */}
      <div className="px-3 py-2 border-t border-[#1e2f5d] text-[10px] text-[#5f84cc]">
        {sessions.length} 个对话
      </div>
    </div>
  );
}
