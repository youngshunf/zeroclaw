/**
 * HasnChat.tsx — HASN 社交聊天页面
 *
 * 复用 Agent Chat 的 hx-panel + hx-chat 布局和样式
 * 数据来源：HASN 网络 API（Phase 2 接入真实数据）
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
  Trash2,
  Check,
  X,
  Loader2,
} from 'lucide-react';
import { Markdown } from '@/huanxing/components/markdown';

// ---- 类型 ----
interface HasnConversation {
  id: string;
  name: string;
  avatar: string;
  lastMessage: string;
  lastTime: string;
  unread: number;
  online: boolean;
}

interface HasnMessage {
  id: string;
  content: string;
  sender: string;
  direction: 'incoming' | 'outgoing';
  timestamp: Date;
}

// ---- Mock 数据（Phase 2 替换为真实 HASN API）----
const MOCK_CONVERSATIONS: HasnConversation[] = [
  {
    id: '1',
    name: '小明',
    avatar: '',
    lastMessage: '你好，在吗？',
    lastTime: '刚刚',
    unread: 2,
    online: true,
  },
  {
    id: '2',
    name: '产品群',
    avatar: '',
    lastMessage: '明天的会议改到下午3点',
    lastTime: '10分钟前',
    unread: 0,
    online: false,
  },
  {
    id: '3',
    name: 'AI助手',
    avatar: '',
    lastMessage: '已为您生成摘要',
    lastTime: '1小时前',
    unread: 1,
    online: true,
  },
];

const MOCK_MESSAGES: Record<string, HasnMessage[]> = {
  '1': [
    { id: 'm1', content: '嗨，最近怎么样？', sender: '小明', direction: 'incoming', timestamp: new Date('2026-03-13T14:20:00') },
    { id: 'm2', content: '还不错！你呢？', sender: '我', direction: 'outgoing', timestamp: new Date('2026-03-13T14:21:00') },
    { id: 'm3', content: '你好，在吗？', sender: '小明', direction: 'incoming', timestamp: new Date('2026-03-13T14:30:00') },
  ],
  '2': [
    { id: 'm4', content: '明天的会议改到下午3点', sender: '张经理', direction: 'incoming', timestamp: new Date('2026-03-13T13:00:00') },
  ],
  '3': [
    { id: 'm5', content: '你好！我是你的AI助手，有什么可以帮你的？', sender: 'AI助手', direction: 'incoming', timestamp: new Date('2026-03-13T12:00:00') },
    { id: 'm6', content: '帮我总结一下今天的新闻', sender: '我', direction: 'outgoing', timestamp: new Date('2026-03-13T12:01:00') },
    { id: 'm7', content: '已为您生成摘要', sender: 'AI助手', direction: 'incoming', timestamp: new Date('2026-03-13T12:02:00') },
  ],
};

// ---- 头像首字母 ----
function getInitial(name: string): string {
  return name.charAt(0);
}

export default function HasnChat() {
  const [conversations, setConversations] = useState<HasnConversation[]>(MOCK_CONVERSATIONS);
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, HasnMessage[]>>(MOCK_MESSAGES);
  const [searchQuery, setSearchQuery] = useState('');
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const activeConv = conversations.find((c) => c.id === activeConvId);
  const currentMessages = activeConvId ? (messages[activeConvId] ?? []) : [];

  // Auto scroll
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [currentMessages]);

  // 选中会话
  const handleSelectConversation = useCallback((id: string) => {
    setActiveConvId(id);
    // 清除未读
    setConversations((prev) =>
      prev.map((c) => (c.id === id ? { ...c, unread: 0 } : c))
    );
  }, []);

  // 发送消息
  const handleSendMessage = useCallback(() => {
    const trimmed = input.trim();
    if (!trimmed || !activeConvId) return;

    const newMsg: HasnMessage = {
      id: `m_${Date.now()}`,
      content: trimmed,
      sender: '我',
      direction: 'outgoing',
      timestamp: new Date(),
    };

    setMessages((prev) => ({
      ...prev,
      [activeConvId]: [...(prev[activeConvId] || []), newMsg],
    }));

    // 更新会话列表最后消息
    setConversations((prev) =>
      prev.map((c) =>
        c.id === activeConvId
          ? { ...c, lastMessage: trimmed, lastTime: '刚刚' }
          : c
      )
    );

    setInput('');
    textareaRef.current?.focus();

    // TODO Phase 2: 调用 hasnApi.sendMessage()
  }, [input, activeConvId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  // 过滤会话
  const filteredConversations = searchQuery
    ? conversations.filter((c) =>
        c.name.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : conversations;

  return (
    <>
      {/* ===== 左侧会话列表（hx-panel） ===== */}
      <div className="hx-panel">
        {/* Header */}
        <div className="hx-panel-header">
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px' }}>
              <Users size={18} style={{ color: 'var(--hx-purple)', flexShrink: 0 }} />
              <span style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
                HASN 社交
              </span>
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

        {/* 会话列表 */}
        <div className="hx-conv-list">
          {filteredConversations.length === 0 ? (
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
                  {/* 头像 */}
                  <div
                    className="hx-conv-avatar"
                    style={{
                      background: 'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                      color: 'white',
                      fontWeight: 600,
                      fontSize: 14,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      position: 'relative',
                    }}
                  >
                    {getInitial(conv.name)}
                    {/* 在线状态小点 */}
                    {conv.online && (
                      <span
                        style={{
                          position: 'absolute',
                          bottom: 0,
                          right: 0,
                          width: 8,
                          height: 8,
                          borderRadius: '50%',
                          background: 'var(--hx-green)',
                          border: '2px solid var(--hx-bg-panel)',
                        }}
                      />
                    )}
                  </div>

                  {/* 会话信息 */}
                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{conv.name}</span>
                      <span
                        style={{
                          fontSize: 11,
                          color: 'var(--hx-text-tertiary)',
                          flexShrink: 0,
                        }}
                      >
                        {conv.lastTime}
                      </span>
                    </div>
                    <div className="hx-conv-preview">{conv.lastMessage}</div>
                  </div>

                  {/* 未读角标 */}
                  {conv.unread > 0 && !isActive && (
                    <span className="hx-conv-badge">
                      {conv.unread > 99 ? '99+' : conv.unread}
                    </span>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* ===== 右侧聊天区（hx-chat） ===== */}
      <div className="hx-chat">
        {/* Chat header */}
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
                {getInitial(activeConv.name)}
              </div>
              <div className="hx-chat-header-info">
                <h3>{activeConv.name}</h3>
                <div className="hx-chat-header-status">
                  <span
                    className="dot"
                    style={{
                      background: activeConv.online
                        ? 'var(--hx-green)'
                        : 'var(--hx-text-tertiary)',
                    }}
                  />
                  {activeConv.online ? '在线' : '离线'}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Messages */}
        <div className="hx-messages">
          {!activeConvId ? (
            <div className="hx-empty-state">
              <div className="icon">💬</div>
              <h3>HASN 社交聊天</h3>
              <p>选择一个会话开始聊天，或点击 "+" 发起新对话</p>
            </div>
          ) : currentMessages.length === 0 ? (
            <div className="hx-empty-state">
              <div className="icon">✨</div>
              <h3>新对话</h3>
              <p>发送消息开始与 {activeConv?.name || '对方'} 聊天</p>
            </div>
          ) : null}

          {currentMessages.map((msg) => (
            <div
              key={msg.id}
              className={`hx-msg ${msg.direction === 'outgoing' ? 'user' : 'agent'}`}
            >
              <div
                className="hx-msg-avatar"
                style={
                  msg.direction === 'incoming'
                    ? {
                        background:
                          'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                        color: 'white',
                        fontWeight: 600,
                        fontSize: 12,
                      }
                    : undefined
                }
              >
                {msg.direction === 'outgoing'
                  ? '杨'
                  : getInitial(msg.sender)}
              </div>
              <div className="hx-msg-content">
                {msg.direction === 'incoming' && (
                  <span className="hx-msg-sender">{msg.sender}</span>
                )}
                <div className="hx-msg-bubble">
                  <Markdown mode="minimal">{msg.content}</Markdown>
                </div>
                <span className="hx-msg-time">
                  {msg.timestamp.toLocaleTimeString('zh-CN', {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
              </div>
            </div>
          ))}

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
                background: activeConvId
                  ? 'var(--hx-green)'
                  : 'var(--hx-text-tertiary)',
              }}
            />
            {activeConvId && activeConv
              ? `${activeConv.name} · HASN`
              : '未选择聊天'}
          </div>
        </div>
      </div>
    </>
  );
}
