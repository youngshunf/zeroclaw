/**
 * AgentNegotiationView — Agent↔Agent 上帝视角交涉监控组件
 *
 * 当选中交涉类型会话时，替代普通聊天气泡区显示。
 * 对齐线框图 §三 后台交涉"上帝视角"
 */
import { useMemo } from 'react';
import { AlertTriangle, Radio } from 'lucide-react';
import { Markdown } from '@/components/markdown';
import type { HasnEnvelope } from '@/lib/hasn-api';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

interface AgentNegotiationViewProps {
  /** 我的 Agent 名称 */
  myAgentName: string;
  /** 我的 Agent 头像 */
  myAgentAvatar?: string;
  /** 对方 Agent 名称 */
  peerAgentName: string;
  /** 对方 Agent 归属人名称 */
  peerOwnerName: string;
  /** 对方 Agent 头像 */
  peerAgentAvatar?: string;
  /** 消息列表 */
  messages: HasnEnvelope[];
  /** 我的 hasn_id */
  myId: string;
  /** 介入回调 */
  onIntervene?: () => void;
  /** 消息列表底部 ref */
  messagesEndRef?: React.RefObject<HTMLDivElement>;
}

export default function AgentNegotiationView({
  myAgentName,
  myAgentAvatar,
  peerAgentName,
  peerOwnerName,
  peerAgentAvatar,
  messages,
  myId,
  onIntervene,
  messagesEndRef,
}: AgentNegotiationViewProps) {
  // 计算交涉轮次
  const roundCount = useMemo(() => {
    return Math.ceil(messages.length / 2);
  }, [messages]);

  // 检测交涉是否已完成 (如果最后一条消息包含特定标记)
  const isCompleted = useMemo(() => {
    if (messages.length === 0) return false;
    const last = messages[messages.length - 1];
    const text = last.content?.body?.text || '';
    return text.includes('交涉已完成') || text.includes('已确认') || text.includes('已完成');
  }, [messages]);

  return (
    <>
      {/* Header */}
      <div className="hx-chat-header">
        <div className="hx-chat-header-left">
          <div className="flex items-center gap-2">
            {/* My Agent avatar */}
            {myAgentAvatar ? (
              <img src={myAgentAvatar} alt={myAgentName} className="w-[32px] h-[32px] rounded-full object-cover ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel" />
            ) : (
              <div className="w-[32px] h-[32px] rounded-full bg-gradient-to-br from-hx-blue to-hx-purple text-white text-xs font-semibold flex items-center justify-center ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel">
                {getInitial(myAgentName)}
              </div>
            )}
            <span className="text-hx-text-tertiary text-[12px]">↔</span>
            {/* Peer Agent avatar */}
            {peerAgentAvatar ? (
              <img src={peerAgentAvatar} alt={peerAgentName} className="w-[32px] h-[32px] rounded-full object-cover ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel" />
            ) : (
              <div className="w-[32px] h-[32px] rounded-full bg-gradient-to-br from-purple-400 to-hx-blue text-white text-xs font-semibold flex items-center justify-center ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel">
                {getInitial(peerAgentName)}
              </div>
            )}
          </div>
          <div className="hx-chat-header-info">
            <h3 className="flex items-center gap-1.5">
              🤖 {myAgentName} <span className="text-hx-text-tertiary text-[12px]">↔</span> 🟡 {peerAgentName}
              <span className="text-[11px] text-hx-text-tertiary font-normal">({peerOwnerName}的)</span>
            </h3>
            <div className="hx-chat-header-status">
              <Radio size={12} className="text-amber-500" />
              <span className="text-amber-500">
                {isCompleted ? '交涉已完成' : `自动交涉中 (已进行 ${roundCount} 轮)`}
              </span>
            </div>
          </div>
        </div>
        <div className="hx-chat-header-actions">
          <button
            className="hx-intervene-btn"
            onClick={onIntervene}
          >
            介入
          </button>
        </div>
      </div>

      {/* Messages area */}
      <div className="hx-messages">
        {messages.length === 0 ? (
          <div className="hx-empty-state">
            <div className="icon">🤖</div>
            <h3>Agent 交涉频道</h3>
            <p>您的 Agent 正在与对方 Agent 进行自动交涉</p>
          </div>
        ) : (
          <>
            {messages.map((msg) => {
              const isFromMyAgent = msg.from.hasn_id === myId;
              const senderName = isFromMyAgent ? myAgentName : peerAgentName;
              const senderAvatar = isFromMyAgent ? myAgentAvatar : peerAgentAvatar;
              const senderLabel = isFromMyAgent ? '(代表我)' : `(代表${peerOwnerName})`;
              const senderEmoji = isFromMyAgent ? '🤖' : '🟡';

              return (
                <div
                  key={msg.local_id || msg.id}
                  className={`hx-msg ${isFromMyAgent ? 'user' : 'agent'}`}
                >
                  {senderAvatar ? (
                    <img
                      src={senderAvatar}
                      alt={senderName}
                      className={`hx-msg-avatar object-cover ${
                        isFromMyAgent
                          ? 'ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel'
                          : 'ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel'
                      }`}
                    />
                  ) : (
                    <div
                      className={`hx-msg-avatar flex items-center justify-center text-white font-semibold text-xs ${
                        isFromMyAgent
                          ? 'bg-gradient-to-br from-hx-blue to-hx-purple ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel'
                          : 'bg-gradient-to-br from-purple-400 to-hx-blue ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel'
                      }`}
                    >
                      {getInitial(senderName)}
                    </div>
                  )}
                  <div className="hx-msg-content">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-[11px] font-medium text-hx-text-secondary">
                        {senderEmoji} {senderName}
                      </span>
                      <span className="text-[9px] text-hx-text-tertiary">{senderLabel}</span>
                    </div>
                    <div className="hx-msg-bubble">
                      <Markdown mode="minimal">{msg.content.body?.text || ''}</Markdown>
                    </div>
                    <span className="hx-msg-time">
                      {msg.metadata?.created_at
                        ? new Date(msg.metadata.created_at).toLocaleTimeString('zh-CN', {
                            hour: '2-digit',
                            minute: '2-digit',
                          })
                        : ''}
                    </span>
                  </div>
                </div>
              );
            })}

            {/* Completion divider */}
            {isCompleted && (
              <div className="hx-negotiation-divider">
                🤝 交涉已完成
              </div>
            )}
          </>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Status banner */}
      <div className="hx-negotiation-banner">
        <AlertTriangle size={14} />
        <span>当前处于 Agent 代管频道。点击右上角「介入」中断交涉并直接对话。</span>
      </div>
    </>
  );
}
