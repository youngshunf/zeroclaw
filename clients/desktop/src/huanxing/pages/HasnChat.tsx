/**
 * HasnChat.tsx — HASN 社交聊天页面
 *
 * 接入真实 HASN API + WS 实时事件
 */
import { useState, useCallback, useRef, useEffect } from 'react';
import {
  Search,
  Plus,
  Send,
  Paperclip,
  Smile,
  MessageSquare,
  Users,
  Loader2,
  ChevronUp,
} from 'lucide-react';
import { Markdown } from '@/huanxing/components/markdown';
import { getHuanxingSession } from '@/huanxing/config';
import {
  useHasnConnection,
  useHasnConversations,
  useHasnMessages,
} from '@/huanxing/hooks/useHasn';
import * as hasnApi from '@/huanxing/lib/hasn-api';

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

  // HASN 连接
  const { connected, status, connect } = useHasnConnection();

  // 自动连接
  useEffect(() => {
    if (!connected && session?.accessToken && myId) {
      connect(session.accessToken, myId, '').catch(() => {});
    }
  }, [connected, session?.accessToken, myId, connect]);

  // 会话列表
  const { conversations, totalUnread, loading: convsLoading, refresh: refreshConvs, setConversations } = useHasnConversations();
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // 消息
  const { messages, loading: msgsLoading, send, loadMore } = useHasnMessages(activeConvId);
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const activeConv = conversations.find((c) => c.id === activeConvId);

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

  // 发送消息
  const handleSendMessage = useCallback(() => {
    const trimmed = input.trim();
    if (!trimmed || !activeConvId) return;
    send(trimmed);
    setInput('');
    textareaRef.current?.focus();
  }, [input, activeConvId, send]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

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
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px' }}>
              <Users size={18} style={{ color: 'var(--hx-purple)', flexShrink: 0 }} />
              <span style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
                HASN 社交
              </span>
              {/* 连接状态指示 */}
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: '50%',
                  background: connected ? 'var(--hx-green)' : 'var(--hx-text-tertiary)',
                  flexShrink: 0,
                }}
              />
            </div>
            <button
              className="hx-nav-item"
              style={{ width: 32, height: 32, flexShrink: 0 }}
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
            <div className="hx-empty-state" style={{ padding: '40px 0' }}>
              <Loader2 size={24} className="animate-spin" style={{ opacity: 0.5 }} />
              <p style={{ fontSize: 13 }}>加载中...</p>
            </div>
          ) : filteredConversations.length === 0 ? (
            <div className="hx-empty-state" style={{ padding: '40px 0' }}>
              <MessageSquare size={32} style={{ opacity: 0.4 }} />
              <p style={{ fontSize: 13 }}>
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
                    className="hx-conv-avatar"
                    style={{
                      background: conv.peer_type === 'agent'
                        ? 'linear-gradient(135deg, var(--hx-blue), var(--hx-purple))'
                        : 'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                      color: 'white',
                      fontWeight: 600,
                      fontSize: 14,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                    }}
                  >
                    {getInitial(conv.peer_name)}
                  </div>

                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{conv.peer_name}</span>
                      <span style={{ fontSize: 11, color: 'var(--hx-text-tertiary)', flexShrink: 0 }}>
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
                className="hx-chat-header-avatar"
                style={{
                  background: 'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                  color: 'white',
                  fontWeight: 600,
                  fontSize: 14,
                }}
              >
                {getInitial(activeConv.peer_name)}
              </div>
              <div className="hx-chat-header-info">
                <h3>{activeConv.peer_name}</h3>
                <div className="hx-chat-header-status">
                  <span
                    className="dot"
                    style={{
                      background: connected ? 'var(--hx-green)' : 'var(--hx-text-tertiary)',
                    }}
                  />
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
              <Loader2 size={24} className="animate-spin" style={{ opacity: 0.5 }} />
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
                <div style={{ textAlign: 'center', padding: '8px 0' }}>
                  <button
                    onClick={loadMore}
                    disabled={msgsLoading}
                    style={{
                      fontSize: 12,
                      color: 'var(--hx-purple)',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 4,
                    }}
                  >
                    {msgsLoading ? <Loader2 size={12} className="animate-spin" /> : <ChevronUp size={12} />}
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
                      className="hx-msg-avatar"
                      style={
                        !isOutgoing
                          ? {
                              background: 'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                              color: 'white',
                              fontWeight: 600,
                              fontSize: 12,
                            }
                          : undefined
                      }
                    >
                      {isOutgoing ? getInitial(myName) : getInitial(activeConv?.peer_name || '?')}
                    </div>
                    <div className="hx-msg-content">
                      <div className="hx-msg-bubble">
                        <Markdown mode="minimal">{msg.content}</Markdown>
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

        {/* 输入区 */}
        <div className="hx-chat-input-area">
          <div className="hx-chat-input-wrapper">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={
                !activeConvId
                  ? '请先选择一个聊天'
                  : '输入消息... (Enter 发送)'
              }
              disabled={!activeConvId}
              rows={1}
            />
            <div className="hx-chat-input-actions">
              <button title="附件">
                <Paperclip size={18} />
              </button>
              <button title="表情">
                <Smile size={18} />
              </button>
              <button
                className="hx-send-btn"
                onClick={handleSendMessage}
                disabled={!input.trim() || !activeConvId}
                title="发送"
              >
                <Send size={18} />
              </button>
            </div>
          </div>
          <div className="hx-input-hint">
            <span
              className="dot"
              style={{
                background: connected ? 'var(--hx-green)' : 'var(--hx-text-tertiary)',
              }}
            />
            {activeConvId && activeConv
              ? `${activeConv.peer_name} · HASN`
              : status === 'connected' ? 'HASN 已连接' : 'HASN 未连接'}
          </div>
        </div>
      </div>
    </>
  );
}
