/**
 * HasnChat.tsx — HASN 社交聊天页面
 *
 * 接入真实 HASN API + WS 实时事件
 * 使用 HxChatInput 组件（含 /命令、@提及、文件上传）
 */
import { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import {
  Search,
  Plus,
  MessageSquare,
  Users,
  Loader2,
  ChevronUp,
} from 'lucide-react';
import { Markdown } from '@/components/markdown';
import { HxImageMessage, containsImageMarkers } from '@/components/chat/HxImageMessage';
import { getHuanxingSession } from '@/config';
import {
  useHasnConnection,
  useHasnConversations,
  useHasnMessages,
} from '@/hooks/useHasn';
import { useHasnContacts } from '@/hooks/useHasnContacts';
import { useAgentSkills } from '@/hooks/useAgentSkills';
import { HxChatInput } from '@/components/chat/input';
import { HUANXING_SLASH_SECTIONS } from '@/components/chat/input/HxSlashMenu';
import * as hasnApi from '@/lib/hasn-api';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

function formatTime(iso?: string): string {
  if (!iso) return '';
  try {
    const d = new Date(iso);
    const now = new Date();
    const diff = now.getTime() - d.getTime();
    if (diff < 60_000) return '刚刚';
    if (diff < 3600_000) return `${Math.floor(diff / 60_000)}分钟前`;
    if (d.toDateString() === now.toDateString()) {
      return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
    }
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (d.toDateString() === yesterday.toDateString()) return '昨天';
    return d.toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' });
  } catch {
    return '';
  }
}

export default function HasnChat() {
  const session = getHuanxingSession();
  const myId = session?.user?.uuid || '';
  const myName = session?.user?.nickname || '我';

  // HASN 连接（由 Tauri 层管理，前端只读状态）
  const { connected, status } = useHasnConnection();

  // 会话列表
  const { conversations, totalUnread, loading: convsLoading, refresh: refreshConvs, setConversations } = useHasnConversations();
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // 消息
  const { messages, loading: msgsLoading, send, loadMore } = useHasnMessages(activeConvId);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const activeConv = conversations.find((c) => c.id === activeConvId);

  // ── HASN 联系人 + Agent 技能（提供给 HxChatInput） ──────────
  const hasnContacts = useHasnContacts();
  const agentSkills = useAgentSkills();

  const mentionSections = useMemo(() => {
    const sections = [...hasnContacts.sections];
    if (agentSkills.asMentionItems.length > 0) {
      sections.push({ id: 'skills', label: '技能', items: agentSkills.asMentionItems });
    }
    return sections;
  }, [hasnContacts.sections, agentSkills.asMentionItems]);

  const slashSections = useMemo(() => {
    const sections = [...HUANXING_SLASH_SECTIONS];
    if (agentSkills.asSlashItems.length > 0) {
      sections.push({ id: 'skills', label: '可用技能', items: agentSkills.asSlashItems });
    }
    return sections;
  }, [agentSkills.asSlashItems]);

  // 自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // 选中会话
  const handleSelectConversation = useCallback((id: string) => {
    setActiveConvId(id);
    // 清除未读
    setConversations((prev) =>
      prev.map((c) => (c.id === id ? { ...c, unread_count: 0 } : c))
    );
    // 通知服务端已读
    hasnApi.markConversationRead(id).catch(() => {});
  }, [setConversations]);

  // 发送消息（通过 HxChatInput）
  const handleSendMessage = useCallback((content: string) => {
    if (!content || !activeConvId) return;
    send(content);
  }, [activeConvId, send]);

  // 过滤会话
  const filteredConversations = searchQuery
    ? conversations.filter((c) =>
        c.peer_name.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : conversations;

  return (
    <>
      {/* ===== 左侧会话列表 ===== */}
      <div className="hx-panel">
        <div className="hx-panel-header">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5 px-2 py-1">
              <Users className="w-[18px] h-[18px] text-hx-purple shrink-0" />
              <span className="text-[15px] font-semibold text-hx-text-primary">
                HASN 社交
              </span>
              {/* 连接状态指示 */}
              <span
                className={`w-1.5 h-1.5 rounded-full shrink-0 ${connected ? 'bg-hx-green' : 'bg-hx-text-tertiary'}`}
              />
            </div>
            <button
              className="hx-nav-item w-8 h-8 shrink-0"
              title="新建聊天"
            >
              <Plus size={18} />
            </button>
          </div>
          <div className="hx-panel-search">
            <Search size={16} />
            <input
              type="text"
              placeholder="搜索聊天..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
        </div>

        <div className="hx-conv-list">
          {convsLoading && conversations.length === 0 ? (
            <div className="hx-empty-state py-10">
              <Loader2 className="w-6 h-6 animate-spin opacity-50" />
              <p className="text-[13px]">加载中...</p>
            </div>
          ) : filteredConversations.length === 0 ? (
            <div className="hx-empty-state py-10">
              <MessageSquare className="w-8 h-8 opacity-40" />
              <p className="text-[13px]">
                {searchQuery ? '未找到匹配的聊天' : '暂无聊天'}
              </p>
            </div>
          ) : (
            filteredConversations.map((conv) => {
              const isActive = conv.id === activeConvId;
              return (
                <div
                  key={conv.id}
                  onClick={() => handleSelectConversation(conv.id)}
                  className={`hx-conv-item${isActive ? ' active' : ''}`}
                >
                  <div
                    className={`hx-conv-avatar flex items-center justify-center text-white font-semibold text-sm ${conv.peer_type === 'agent' ? 'bg-gradient-to-br from-hx-blue to-hx-purple' : 'bg-gradient-to-br from-hx-purple to-hx-blue'}`}
                  >
                    {getInitial(conv.peer_name)}
                  </div>

                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{conv.peer_name}</span>
                      <span className="text-[11px] text-hx-text-tertiary shrink-0">
                        {formatTime(conv.last_message_at)}
                      </span>
                    </div>
                    <div className="hx-conv-preview">{conv.last_message || ''}</div>
                  </div>

                  {conv.unread_count > 0 && !isActive && (
                    <span className="hx-conv-badge">
                      {conv.unread_count > 99 ? '99+' : conv.unread_count}
                    </span>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* ===== 右侧聊天区 ===== */}
      <div className="hx-chat">
        {activeConvId && activeConv && (
          <div className="hx-chat-header">
            <div className="hx-chat-header-left">
              <div
                className="hx-chat-header-avatar bg-gradient-to-br from-hx-purple to-hx-blue text-white font-semibold text-sm"
              >
                {getInitial(activeConv.peer_name)}
              </div>
              <div className="hx-chat-header-info">
                <h3>{activeConv.peer_name}</h3>
                <div className="hx-chat-header-status">
                  <span className={`dot ${connected ? 'bg-hx-green' : 'bg-hx-text-tertiary'}`} />
                  {activeConv.peer_type === 'agent' ? 'Agent' : 'HASN'}
                </div>
              </div>
            </div>
          </div>
        )}

        <div className="hx-messages">
          {!activeConvId ? (
            <div className="hx-empty-state">
              <div className="icon">💬</div>
              <h3>HASN 社交聊天</h3>
              <p>选择一个会话开始聊天，或点击 "+" 发起新对话</p>
            </div>
          ) : msgsLoading && messages.length === 0 ? (
            <div className="hx-empty-state">
              <Loader2 className="w-6 h-6 animate-spin opacity-50" />
            </div>
          ) : messages.length === 0 ? (
            <div className="hx-empty-state">
              <div className="icon">✨</div>
              <h3>新对话</h3>
              <p>发送消息开始与 {activeConv?.peer_name || '对方'} 聊天</p>
            </div>
          ) : (
            <>
              {/* 加载更多 */}
              {messages.length >= 50 && (
                <div className="text-center py-2">
                  <button
                    onClick={loadMore}
                    disabled={msgsLoading}
                    className="text-[12px] text-hx-purple bg-transparent border-none cursor-pointer inline-flex items-center gap-1"
                  >
                    {msgsLoading ? <Loader2 className="w-3 h-3 animate-spin" /> : <ChevronUp className="w-3 h-3" />}
                    加载更早消息
                  </button>
                </div>
              )}

              {messages.map((msg) => {
                const isOutgoing = msg.from_id === myId;
                return (
                  <div
                    key={msg.local_id || msg.id}
                    className={`hx-msg ${isOutgoing ? 'user' : 'agent'}`}
                  >
                    <div
                      className={`hx-msg-avatar flex items-center justify-center shrink-0 ${!isOutgoing ? 'bg-gradient-to-br from-hx-purple to-hx-blue text-white font-semibold text-xs' : ''}`}
                    >
                      {isOutgoing ? getInitial(myName) : getInitial(activeConv?.peer_name || '?')}
                    </div>
                    <div className="hx-msg-content">
                      <div className="hx-msg-bubble">
                        {containsImageMarkers(msg.content) ? (
                          <HxImageMessage
                            content={msg.content}
                            renderText={(text) => <Markdown mode="minimal">{text}</Markdown>}
                          />
                        ) : (
                          <Markdown mode="minimal">{msg.content}</Markdown>
                        )}
                      </div>
                      <span className="hx-msg-time">
                        {msg.created_at
                          ? new Date(msg.created_at).toLocaleTimeString('zh-CN', {
                              hour: '2-digit',
                              minute: '2-digit',
                            })
                          : ''}
                        {isOutgoing && msg.send_status === 'sending' && ' · 发送中'}
                        {isOutgoing && msg.send_status === 'failed' && ' · 发送失败'}
                      </span>
                    </div>
                  </div>
                );
              })}
            </>
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* 输入区 — 使用 HxChatInput（含 /命令、@提及、文件上传） */}
        <HxChatInput
          onSend={handleSendMessage}
          disabled={!activeConvId}
          connected={connected}
          agentName={activeConv?.peer_name || 'HASN'}
          placeholder={
            !activeConvId
              ? '请先选择一个聊天'
              : '输入消息... (Enter 发送，Shift+Enter 换行)'
          }
          mentionSections={mentionSections}
          slashSections={slashSections}
        />
      </div>
    </>
  );
}
