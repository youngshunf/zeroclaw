/**
 * HasnChat.tsx — HASN 社交聊天页面
 *
 * 对齐线框图：
 * - §二 消息 / 聊天主界面
 * - §三 后台交涉"上帝视角"
 *
 * 接入真实 HASN API + WS 实时事件
 * 使用 HxChatInput 组件（含 /命令、@提及、文件上传）
 */
import { useState, useCallback, useRef, useEffect, useMemo } from 'react';
import { useLocation } from 'react-router-dom';
import {
  Search,
  Plus,
  MessageSquare,
  Users,
  Loader2,
  ChevronUp,
  ChevronLeft,
  Radio,
} from 'lucide-react';
import { Markdown } from '@/components/markdown';
import { HxImageMessage, containsImageMarkers } from '@/components/chat/HxImageMessage';
import { getHuanxingSession } from '@/config';
import {
  useHasnConnection,
  useHasnConversations,
  useHasnMessages,
} from '@/hooks/useHasn';
import { useHasnContacts, inferConversationType } from '@/hooks/useHasnContacts';
import { useAgentSkills } from '@/hooks/useAgentSkills';
import { HxChatInput } from '@/components/chat/input';
import { HUANXING_SLASH_SECTIONS } from '@/components/chat/input/HxSlashMenu';
import AgentNegotiationView from '@/components/hasn/AgentNegotiationView';
import * as hasnApi from '@/lib/hasn-api';
import { usePlatform } from '@/hooks/usePlatform';

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

/**
 * 扩展实体类型推断：新增 negotiation 类型
 * negotiation = 我的 Agent 与 好友的 Agent 之间的自动交涉
 */
function inferEntityType(
  peerType: string,
  peerHasnId: string,
  myAgentIds: Set<string>,
  convMetadata?: any,
): 'human' | 'my-agent' | 'friend-agent' | 'negotiation' {
  // 如果会话标记为交涉类型
  if (convMetadata?.negotiation || convMetadata?.type === 'agent_negotiation') {
    return 'negotiation';
  }
  return inferConversationType(peerType, peerHasnId, myAgentIds);
}

/**
 * 获取实体类型的显示配置
 */
function getEntityStyle(type: ReturnType<typeof inferEntityType>) {
  switch (type) {
    case 'human':
      return {
        avatarBorder: 'ring-2 ring-hx-green ring-offset-1 ring-offset-hx-bg-panel',
        emoji: '👥',
        nameSuffix: '',
        gradient: 'from-hx-green/80 to-hx-green',
      };
    case 'my-agent':
      return {
        avatarBorder: 'ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel',
        emoji: '🤖',
        nameSuffix: '',
        gradient: 'from-hx-blue to-hx-purple',
      };
    case 'friend-agent':
      return {
        avatarBorder: 'ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel',
        emoji: '🟡',
        nameSuffix: ' (Agent)',
        gradient: 'from-purple-400 to-hx-blue',
      };
    case 'negotiation':
      return {
        avatarBorder: 'ring-2 ring-amber-400 ring-offset-1 ring-offset-hx-bg-panel',
        emoji: '🤖↔🟡',
        nameSuffix: '',
        gradient: 'from-amber-400 to-purple-400',
      };
  }
}

export default function HasnChat() {
  const session = getHuanxingSession();
  // 使用 HASN ID（非 UUID）来判断消息发送方
  const myHasnId = typeof window !== 'undefined' ? localStorage.getItem('hasn:hasn_id') || '' : '';
  const myId = myHasnId;
  const myName = session?.user?.nickname || '我';
  const myAvatarUrl = session?.user?.avatar || '';

  // HASN 连接（由 Tauri 层管理，前端只读状态）
  const { connected, status } = useHasnConnection();

  // 会话列表
  const { conversations, totalUnread, loading: convsLoading, refresh: refreshConvs, setConversations } = useHasnConversations();
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // 消息
  const { messages, loading: msgsLoading, send, loadMore } = useHasnMessages(activeConvId);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // activeConv: 先按 id 查，再按 peer_id 查（兼容临时会话 id === peerId 的情况）
  const activeConv = conversations.find((c) => c.id === activeConvId)
    || conversations.find((c) => c.peer_id === activeConvId);

  // ── HASN 联系人 + Agent 技能（提供给 HxChatInput） ──────────
  const hasnContacts = useHasnContacts();
  const agentSkills = useAgentSkills();

  // Router location（用于从通讯录「发消息」跳转）
  const location = useLocation();

  // 聊天对象信息兜底（从 location.state 或 contacts 中获取）
  const activePeerInfo = useMemo(() => {
    if (activeConv) {
      return {
        name: activeConv.peer_name,
        type: activeConv.peer_type,
        peerId: activeConv.peer_id,
      };
    }
    // 从 location.state 兜底
    const state = (location?.state || {}) as any;
    if (state.peerId && state.peerId === activeConvId) {
      return {
        name: state.peerName || activeConvId?.substring(0, 8) || '',
        type: state.peerType || 'agent',
        peerId: state.peerId,
      };
    }
    // 从联系人中查找
    const cp = hasnContacts.rawContacts.find(c => c.peer.hasn_id === activeConvId);
    if (cp) {
      return {
        name: cp.nickname || cp.peer.name,
        type: cp.peer.type,
        peerId: cp.peer.hasn_id,
      };
    }
    // 从 Agent 列表中查找
    const ag = hasnContacts.rawAgents?.find((a: any) => a.hasn_id === activeConvId);
    if (ag) {
      return {
        name: ag.name,
        type: 'agent',
        peerId: ag.hasn_id,
      };
    }
    return null;
  }, [activeConv, activeConvId, location?.state, hasnContacts.rawContacts, hasnContacts.rawAgents]);

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

  // ── 从通讯录「发消息」跳转过来时，自动打开/创建会话 ──────────
  const locationPeerIdHandled = useRef<string | null>(null);
  useEffect(() => {
    const state = location.state as { peerId?: string; peerName?: string; peerType?: string } | null;
    if (!state?.peerId) return;
    // 防止重复处理同一个 peerId
    if (locationPeerIdHandled.current === state.peerId) return;
    locationPeerIdHandled.current = state.peerId;

    const peerId = state.peerId;
    const peerName = state.peerName || '';
    const peerType = state.peerType || 'agent';

    // 查找已有会话
    const existing = conversations.find((c) => c.peer_id === peerId);
    if (existing) {
      handleSelectConversation(existing.id);
    } else {
      // 没有已有会话 → 创建临时会话条目（发送第一条消息时后端会自动建会话）
      const tempConv: any = {
        id: peerId,
        peer_id: peerId,
        peer_name: peerName || peerId.substring(0, 8),
        peer_type: peerType,
        unread_count: 0,
        last_message: '',
      };
      setConversations((prev: any) => {
        // 避免重复添加
        if (prev.some((c: any) => c.peer_id === peerId)) return prev;
        return [tempConv, ...prev];
      });
      setActiveConvId(peerId);
    }
  }, [location.state, conversations, handleSelectConversation, setConversations]);

  // 过滤会话
  const filteredConversations = searchQuery
    ? conversations.filter((c) =>
        c.peer_name.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : conversations;

  // ── 移动端导航栈 ────────────────────────────────────────────
  const { isMobile } = usePlatform();
  const mobileView = isMobile ? (activeConvId ? 'chat' : 'panel') : 'both';

  // 当前活动会话的实体类型
  const activeEntityType = (activeConv || activePeerInfo)
    ? inferEntityType(
        activeConv?.peer_type || activePeerInfo?.type || 'human',
        activeConv?.peer_id || activePeerInfo?.peerId || '',
        hasnContacts.myAgentIds,
      )
    : null;

  return (
    <div className={isMobile ? (mobileView === 'panel' ? 'hx-mobile-show-panel' : 'hx-mobile-show-chat') : ''}
         style={{ display: 'flex', flex: 1, minWidth: 0, height: '100%' }}>
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
            <>
              {/* ── 最近聊天 ── */}
              {filteredConversations.length > 0 && (
                <div>
                  {filteredConversations.map((conv) => {
                    const isActive = conv.id === activeConvId;
                    const entityType = inferEntityType(conv.peer_type, conv.peer_id, hasnContacts.myAgentIds);
                    const style = getEntityStyle(entityType);
                    
                    // 查找 ContactFull 以获取 avatar_url
                    const cp = hasnContacts.rawContacts.find(c => c.peer.hasn_id === conv.peer_id);
                    const avatarUrl = cp?.peer.avatar_url;

                    // 交涉类型的特殊渲染
                    if (entityType === 'negotiation') {
                      return (
                        <div
                          key={conv.id}
                          onClick={() => handleSelectConversation(conv.id)}
                          className={`hx-conv-item${isActive ? ' active' : ''}`}
                        >
                          <div className="relative flex items-center">
                            <div className="hx-conv-avatar flex items-center justify-center text-white font-semibold text-xs bg-gradient-to-br from-amber-400 to-purple-400 ring-2 ring-amber-400 ring-offset-1 ring-offset-hx-bg-panel -mr-2 z-[1]" style={{ width: 32, height: 32, minWidth: 32 }}>
                              🤖
                            </div>
                            <div className="hx-conv-avatar flex items-center justify-center text-white font-semibold text-xs bg-gradient-to-br from-purple-400 to-hx-blue ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel" style={{ width: 32, height: 32, minWidth: 32 }}>
                              🟡
                            </div>
                          </div>
                          <div className="hx-conv-info">
                            <div className="hx-conv-name-row">
                              <span className="hx-conv-name text-[12px]">
                                🤖 ↔ 🟡 {conv.peer_name}
                              </span>
                              <span className="text-[11px] text-hx-text-tertiary shrink-0">
                                {formatTime(conv.last_message_at)}
                              </span>
                            </div>
                            <div className="hx-conv-preview flex items-center gap-1">
                              <span className="text-[9px] px-1 bg-amber-500/10 text-amber-500 rounded-sm font-medium">[自动交涉]</span>
                              {conv.last_message || ''}
                            </div>
                          </div>
                        </div>
                      );
                    }

                    return (
                      <div
                        key={conv.id}
                        onClick={() => handleSelectConversation(conv.id)}
                        className={`hx-conv-item${isActive ? ' active' : ''}`}
                      >
                        {avatarUrl ? (
                          <div className="relative">
                            <img src={avatarUrl} alt={conv.peer_name} className={`hx-conv-avatar object-cover ${style.avatarBorder}`} />
                            {entityType === 'my-agent' && <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10">✨</div>}
                          </div>
                        ) : (
                          <div className="relative">
                            <div
                              className={`hx-conv-avatar flex items-center justify-center text-white font-semibold text-sm bg-gradient-to-br ${style.gradient} ${style.avatarBorder}`}
                            >
                              {getInitial(conv.peer_name)}
                            </div>
                            {entityType === 'my-agent' && <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10">✨</div>}
                          </div>
                        )}

                        <div className="hx-conv-info">
                          <div className="hx-conv-name-row">
                            <span className="hx-conv-name">
                              {style.emoji !== '👥' && <span className="mr-0.5">{style.emoji}</span>}
                              {conv.peer_name}
                              {style.nameSuffix && <span className="text-hx-text-tertiary font-normal ml-0.5 text-[11px]">{style.nameSuffix}</span>}
                            </span>
                            <span className="text-[11px] text-hx-text-tertiary shrink-0">
                              {formatTime(conv.last_message_at)}
                            </span>
                          </div>
                          <div className="hx-conv-preview">
                            {entityType === 'friend-agent' && (
                              <span className="text-[9px] px-1 bg-purple-500/10 text-purple-400 rounded-sm font-medium mr-1">Agent</span>
                            )}
                            {conv.last_message || ''}
                          </div>
                        </div>

                        {conv.unread_count > 0 && !isActive && (
                          <span className="hx-conv-badge">
                            {conv.unread_count > 99 ? '99+' : conv.unread_count}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* ===== 右侧聊天区 ===== */}
      <div className="hx-chat">
        {/* 交涉类型会话 → 上帝视角 */}
        {activeConvId && activeConv && activeEntityType === 'negotiation' ? (
          <AgentNegotiationView
            myAgentName="星灵"
            peerAgentName={activeConv.peer_name}
            peerOwnerName="好友"
            messages={messages}
            myId={myId}
            onIntervene={() => {
              // TODO: 介入交涉 API
              console.log('[HasnChat] Intervene negotiation');
            }}
            messagesEndRef={messagesEndRef as any}
          />
        ) : (
          <>
            {/* Chat Header — 只要有 activeConvId 就显示 */}
            {activeConvId && activePeerInfo && (
              <div className="hx-chat-header">
                <div className="hx-chat-header-left">
                  {isMobile && (
                    <button
                      className="hx-mobile-header-back"
                      onClick={() => setActiveConvId(null)}
                      style={{ marginRight: 4, flexShrink: 0 }}
                    >
                      <ChevronLeft size={22} />
                    </button>
                  )}
                  
                  {(() => {
                    const entityType = activeEntityType || 'human';
                    const style = getEntityStyle(entityType);
                    const peerName = activePeerInfo.name;
                    const peerId = activePeerInfo.peerId;
                    const cp = hasnContacts.rawContacts.find(c => c.peer.hasn_id === peerId);
                    const ag = hasnContacts.rawAgents?.find((a: any) => a.hasn_id === peerId);
                    const avatarUrl = cp?.peer.avatar_url || ag?.avatar_url;
                    const starId = cp?.peer.star_id || ag?.star_id || ag?.node_id || peerId?.substring(0, 12);
                    
                    let HeaderIcon = null;
                    if (avatarUrl) {
                      HeaderIcon = <img src={avatarUrl} alt={peerName} className={`w-[36px] h-[36px] rounded-full object-cover shrink-0 ${style.avatarBorder}`} />;
                    } else {
                      HeaderIcon = (
                        <div className={`w-[36px] h-[36px] rounded-full bg-gradient-to-br ${style.gradient} text-white font-semibold text-sm flex items-center justify-center shrink-0 ${style.avatarBorder}`}>
                          {getInitial(peerName)}
                        </div>
                      );
                    }

                    return (
                      <>
                        {HeaderIcon}
                        <div className="hx-chat-header-info">
                          <h3 className="flex items-center gap-1.5">
                            {entityType !== 'human' && <span>{style.emoji}</span>}
                            {peerName}
                            {entityType === 'friend-agent' && (
                              <span className="text-[11px] text-hx-text-tertiary font-normal">
                                ({cp?.nickname || cp?.peer.name || '好友'}的Agent)
                              </span>
                            )}
                          </h3>
                          <div className="hx-chat-header-status">
                            <span className={`dot ${connected ? 'bg-hx-green' : 'bg-hx-text-tertiary'}`} />
                            {entityType === 'human' && (
                              <span>
                                🟢 在线
                                {cp?.owned_agents && cp.owned_agents.length > 0 && (
                                  <span className="text-hx-text-tertiary ml-1">| 对方 Agent 正在待机</span>
                                )}
                              </span>
                            )}
                            {entityType === 'my-agent' && <span>我的 Agent · 就绪</span>}
                            {entityType === 'friend-agent' && (
                              <span>归属: {cp?.nickname || cp?.peer.name || '好友'}</span>
                            )}
                            {starId && (
                              <span className="text-hx-text-tertiary ml-1.5 text-[11px]">#{starId}</span>
                            )}
                          </div>
                        </div>
                      </>
                    );
                  })()}
                </div>
                {/* Header actions */}
                <div className="hx-chat-header-actions">
                  {activeEntityType === 'human' && (
                    <button className="text-[11px] px-2 py-1 rounded-hx-radius-sm bg-hx-bg-hover text-hx-text-secondary border border-hx-border hover:text-hx-purple hover:border-hx-purple transition-colors cursor-pointer" title="切换到对方 Agent">
                      🤖 切换到Agent
                    </button>
                  )}
                </div>
              </div>
            )}

            {/* Messages */}
            <div className="hx-messages">
              {!activeConvId ? (
                <div className="hx-empty-state">
                  <div className="icon">💬</div>
                  <h3>HASN 社交聊天</h3>
                  <p>{isMobile ? '选择一个会话开始聊天' : '选择一个会话开始聊天，或点击 "+" 发起新对话'}</p>
                </div>
              ) : msgsLoading && messages.length === 0 ? (
                <div className="hx-empty-state">
                  <Loader2 className="w-6 h-6 animate-spin opacity-50" />
                </div>
              ) : messages.length === 0 ? (
                <div className="hx-empty-state">
                  <div className="icon">✨</div>
                  <h3>新对话</h3>
                  <p>发送消息开始与 {activePeerInfo?.name || '对方'} 聊天</p>
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
                    const isOutgoing = msg.from.hasn_id === myId;
                    const senderEntityType = isOutgoing
                      ? 'human'
                      : inferEntityType(msg.from.entity_type, msg.from.hasn_id, hasnContacts.myAgentIds);
                    const senderStyle = getEntityStyle(senderEntityType === 'negotiation' ? 'friend-agent' : senderEntityType);
                    const cp = !isOutgoing ? hasnContacts.rawContacts.find(c => c.peer.hasn_id === msg.from.hasn_id) : null;
                    const senderAvatarUrl = cp?.peer.avatar_url;
                    
                    // 判断是否为 Agent 代管消息
                    const isDelegated = senderEntityType === 'friend-agent' && activeEntityType === 'human';

                    return (
                      <div
                        key={msg.local_id || msg.id}
                        className={`hx-msg ${isOutgoing ? 'user' : 'agent'}`}
                      >
                        {/* Avatar */}
                        {isOutgoing ? (
                          <div className="hx-msg-avatar shrink-0" style={{ overflow: 'hidden' }}>
                            {myAvatarUrl ? (
                              <img src={myAvatarUrl} alt={myName} style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: 'inherit' }} />
                            ) : (
                              <div className="w-full h-full flex items-center justify-center bg-gradient-to-br from-hx-purple to-hx-blue text-white font-semibold text-xs">
                                {getInitial(myName)}
                              </div>
                            )}
                          </div>
                        ) : senderAvatarUrl ? (
                          <div className="relative shrink-0">
                            <img src={senderAvatarUrl} alt={activePeerInfo?.name || '?'} className={`hx-msg-avatar flex items-center justify-center shrink-0 object-cover ${senderStyle.avatarBorder}`} />
                            {senderEntityType === 'my-agent' && <div className="absolute -bottom-0.5 -right-0.5 text-[8px] bg-hx-bg-panel rounded-full z-10 font-mono">✨</div>}
                          </div>
                        ) : (
                          <div
                            className={`hx-msg-avatar flex items-center justify-center shrink-0 bg-gradient-to-br ${senderStyle.gradient} text-white font-semibold text-xs ${senderStyle.avatarBorder}`}
                          >
                            {getInitial(activePeerInfo?.name || '?')}
                          </div>
                        )}

                        <div className="hx-msg-content">
                          {/* Sender identity label */}
                          <div className="flex items-center gap-2 mb-1">
                            <span className="text-[11px] font-medium text-hx-text-secondary flex items-center gap-1">
                              {isOutgoing ? (
                                <>👤 {myName} <span className="text-[9px] text-hx-text-tertiary">(我)</span></>
                              ) : (
                                <>
                                  {senderStyle.emoji} {activePeerInfo?.name || '对方'}
                                </>
                              )}
                            </span>
                            {!isOutgoing && senderEntityType === 'my-agent' && (
                              <span className="text-[9px] px-1 bg-hx-blue/10 text-hx-blue rounded-sm">我的 Agent</span>
                            )}
                            {!isOutgoing && senderEntityType === 'friend-agent' && (
                              <span className="text-[9px] px-1 bg-purple-500/10 text-purple-500 rounded-sm">好友 Agent</span>
                            )}
                            {isDelegated && (
                              <span className="text-[9px] px-1 bg-amber-500/10 text-amber-600 rounded-sm">代为回复</span>
                            )}
                          </div>

                          {/* Message bubble — special style for Agent delegation */}
                          {isDelegated ? (
                            <div className="hx-agent-delegation-bubble">
                              <div className="hx-agent-delegation-label">
                                🟡 {activePeerInfo?.name}的Agent (代为回复)
                              </div>
                              {containsImageMarkers(msg.content.body?.text || '') ? (
                                <HxImageMessage
                                  content={msg.content.body?.text || ''}
                                  renderText={(text) => <Markdown mode="minimal">{text}</Markdown>}
                                />
                              ) : (
                                <Markdown mode="minimal">{msg.content.body?.text || ''}</Markdown>
                              )}
                            </div>
                          ) : (
                            <div className="hx-msg-bubble">
                              {msg.content.content_type === 'tool_call' ? (
                                <div className="font-mono text-[11px] opacity-70 p-1 bg-black/5 rounded">
                                  {`> 工具调用: ${msg.content.body?.text || msg.content.body?.display_text || '执行中...'}`}
                                </div>
                              ) : containsImageMarkers(msg.content.body?.text || '') ? (
                                <HxImageMessage
                                  content={msg.content.body?.text || ''}
                                  renderText={(text) => <Markdown mode="minimal">{text}</Markdown>}
                                />
                              ) : (
                                <Markdown mode="minimal">{msg.content.body?.text || ''}</Markdown>
                              )}
                            </div>
                          )}

                          <span className="hx-msg-time">
                            {msg.metadata?.created_at
                              ? new Date(msg.metadata.created_at).toLocaleTimeString('zh-CN', {
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
              agentName={activePeerInfo?.name || 'HASN'}
              placeholder={
                !activeConvId
                  ? '请先选择一个聊天'
                  : activeEntityType === 'negotiation'
                    ? '⚠️ Agent 代管频道 — 点击「介入」直接对话'
                    : '输入消息... (Enter 发送，Shift+Enter 换行)'
              }
              mentionSections={mentionSections}
              slashSections={slashSections}
            />
          </>
        )}
      </div>
    </div>
  );
}
