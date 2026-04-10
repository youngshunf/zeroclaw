/**
 * 联系人页面 — 接入 HASN 联系人 API
 *
 * 对齐线框图 §四 通讯录主界面：
 * - 左侧 hx-panel: 我的星组顶置 + 好友按信任等级分组(含Agent从属) + Agent Tab 按归属分组
 * - 右侧 hx-chat: 完整 ContactProfile 详情面板
 */
import { useState, useCallback, useMemo } from 'react';
import {
  Users,
  UserPlus,
  Search,
  Check,
  X,
  Loader2,
  ChevronLeft,
  Bot,
} from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { Input } from '@/components/ui/Input';
import { Textarea } from '@/components/ui/Textarea';
import { useHasnContacts } from '@/hooks/useHasnContacts';
import ContactProfile from '@/components/hasn/ContactProfile';
import * as hasnApi from '@/lib/hasn-api';
import type { ContactFull, AgentPeer } from '@/lib/hasn-api';
import { usePlatform } from '@/hooks/usePlatform';
import BottomSheet from '@/components/ui/BottomSheet';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

function TrustBadge({ level, label }: { level: number; label?: string }) {
  const colorMap: Record<number, string> = {
    0: '#9ca3af',
    1: '#94a3b8',
    2: '#60a5fa',
    3: '#34d399',
    4: '#f59e0b',
    5: '#a78bfa',
  };
  const color = colorMap[level] ?? colorMap[1];
  const displayLabel = label ?? `L${level}`;
  return (
    <span
      className="text-[10px] px-1.5 py-[1px] rounded-lg font-medium"
      style={{ background: `${color}20`, color }}
    >
      {displayLabel}
    </span>
  );
}

export default function Contacts() {
  const navigate = useNavigate();
  const [tab, setTab] = useState<'friends' | 'agents' | 'requests'>('friends');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedContact, setSelectedContact] = useState<ContactFull | null>(null);
  const { rawContacts, rawAgents, contactsByTrust, friendRequests, loading, refresh } = useHasnContacts();

  // 添加好友
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [addStarId, setAddStarId] = useState('');
  const [addMessage, setAddMessage] = useState('');
  const [addLoading, setAddLoading] = useState(false);

  const handleAddFriend = useCallback(async () => {
    if (!addStarId.trim()) return;
    setAddLoading(true);
    try {
      await hasnApi.sendFriendRequest(addStarId.trim(), addMessage || undefined);
      setShowAddDialog(false);
      setAddStarId('');
      setAddMessage('');
      refresh();
    } catch {
      // 静默
    } finally {
      setAddLoading(false);
    }
  }, [addStarId, addMessage, refresh]);

  // 处理好友请求
  const handleRespondRequest = useCallback(async (requestId: number, accept: boolean) => {
    try {
      await hasnApi.respondFriendRequest(requestId, accept);
      refresh();
    } catch {
      // 静默
    }
  }, [refresh]);

  // 跳转聊天
  const handleStartChat = useCallback((contact: ContactFull) => {
    navigate('/hasn-chat', {
      state: {
        peerId: contact.peer.hasn_id,
        peerName: contact.nickname || contact.peer.name,
        peerType: contact.peer.type,
      },
    });
  }, [navigate]);

  // 选择 Agent 从属（将其包装为 ContactFull 以复用详情面板）
  const handleSelectAgentPeer = useCallback((agent: AgentPeer) => {
    const contactMock: ContactFull = {
      id: 0,
      peer: {
        hasn_id: agent.hasn_id,
        star_id: agent.star_id || '',
        name: agent.name,
        type: 'agent',
        avatar_url: agent.avatar_url,
      },
      relation_type: 'social',
      trust_level: 2,
      trust_level_label: '好友Agent',
      subscription: false,
      status: 'connected',
      owned_agents: [],
      custom_permissions: {},
    };
    setSelectedContact(contactMock);
  }, []);

  // ── 过滤分组的联系人 ──
  const filteredGroups = useMemo(() => {
    if (!searchQuery) return contactsByTrust;
    const lowerQuery = searchQuery.toLowerCase();
    return contactsByTrust.map((g: any) => ({
      ...g,
      contacts: g.contacts.filter((c: any) => 
        (c.nickname || c.peer.name).toLowerCase().includes(lowerQuery) ||
        c.peer.star_id.toLowerCase().includes(lowerQuery)
      )
    })).filter((g: any) => g.contacts.length > 0);
  }, [contactsByTrust, searchQuery]);

  // ── 过滤 Agent（按归属人分组） ──
  const agentGroups = useMemo(() => {
    // 收集所有 peer_type === 'agent' 的联系人
    const agentContacts = rawContacts.filter(c => c.peer.type === 'agent');
    
    // 我的 Agent
    const myAgentGroup = {
      owner: '我的 Agent',
      emoji: '🤖',
      agents: rawAgents.map(a => ({
        hasn_id: a.hasn_id,
        star_id: a.node_id || 'LOCAL',
        name: a.name,
        agent_name: a.agent_name,
        avatar_url: a.avatar_url,
        type: a.type,
        role: a.role,
      })),
    };

    // 其他人的 Agent (从联系人中提取)
    const otherAgentGroups: Record<string, { owner: string; emoji: string; agents: AgentPeer[] }> = {};
    for (const c of agentContacts) {
      // 简单用 peer.name 做分组 key
      const ownerKey = c.nickname || c.peer.name;
      if (!otherAgentGroups[ownerKey]) {
        otherAgentGroups[ownerKey] = {
          owner: `${ownerKey} 的 Agent`,
          emoji: '🟡',
          agents: [],
        };
      }
      otherAgentGroups[ownerKey].agents.push({
        hasn_id: c.peer.hasn_id,
        star_id: c.peer.star_id,
        name: c.peer.name,
        agent_name: c.peer.name,
        avatar_url: c.peer.avatar_url,
        type: c.peer.type,
        role: c.relation_type,
      });
    }

    const allGroups = [myAgentGroup, ...Object.values(otherAgentGroups)];
    
    if (!searchQuery) return allGroups;
    const lowerQuery = searchQuery.toLowerCase();
    return allGroups.map(g => ({
      ...g,
      agents: g.agents.filter(a =>
        a.name.toLowerCase().includes(lowerQuery) ||
        (a.agent_name && a.agent_name.toLowerCase().includes(lowerQuery))
      )
    })).filter(g => g.agents.length > 0);
  }, [rawContacts, rawAgents, searchQuery]);

  // ── 移动端导航栈 ──
  const { isMobile } = usePlatform();
  const mobileView = isMobile ? (selectedContact ? 'detail' : 'list') : 'both';

  // 添加好友表单内容（BottomSheet 和 Dialog 复用）
  const addFriendForm = (
    <div className="flex flex-col gap-3">
      <Input
        type="text"
        placeholder="输入对方 Star ID"
        value={addStarId}
        onChange={(e) => setAddStarId(e.target.value)}
        className="w-full"
      />
      <Textarea
        placeholder="附言（可选）"
        value={addMessage}
        onChange={(e) => setAddMessage(e.target.value)}
        rows={2}
        className="w-full resize-none"
      />
      <div className="flex gap-2 justify-end mt-1">
        <button
          onClick={() => setShowAddDialog(false)}
          className="px-4 py-1.5 rounded-hx-radius-sm border border-hx-border bg-transparent text-hx-text-secondary text-[13px] cursor-pointer hover:bg-hx-bg-hover transition-colors"
        >
          取消
        </button>
        <button
          onClick={handleAddFriend}
          disabled={!addStarId.trim() || addLoading}
          className={`px-4 py-1.5 rounded-hx-radius-sm border-none bg-hx-purple text-white text-[13px] cursor-pointer flex items-center gap-1.5 hover:bg-hx-purple-hover transition-colors ${
            !addStarId.trim() || addLoading ? 'opacity-50 !cursor-not-allowed' : ''
          }`}
        >
          {addLoading && <Loader2 size={14} className="animate-spin" />}
          发送请求
        </button>
      </div>
    </div>
  );

  return (
    <div
      className={isMobile ? (mobileView === 'list' ? 'hx-mobile-show-panel' : '') : ''}
      style={{ display: 'flex', flex: 1, minWidth: 0, height: '100%' }}
    >
      {/* ===== 左侧面板 ===== */}
      <div className="hx-panel">
        <div className="hx-panel-header">
          <div className="flex items-center justify-between">
            <h2 className="hx-panel-title">👥 通讯录</h2>
            <button
              className="hx-nav-item !w-8 !h-8 shrink-0"
              title="添加好友"
              onClick={() => setShowAddDialog(true)}
            >
              <UserPlus size={18} />
            </button>
          </div>
          {/* Tab 切换 */}
          <div className="flex gap-1 px-3 pb-2 flex-wrap">
            <button
              onClick={() => { setTab('friends'); setSearchQuery(''); }}
              className={`hx-nav-item !w-auto !h-auto px-3 py-1.5 rounded-hx-radius-sm gap-1.5 flex items-center text-[13px] font-medium transition-colors ${
                tab === 'friends' ? 'active' : ''
              }`}
            >
              <Users size={15} />
              好友 ({rawContacts.filter(c => c.peer.type === 'human').length})
            </button>
            <button
              onClick={() => { setTab('agents'); setSearchQuery(''); }}
              className={`hx-nav-item !w-auto !h-auto px-3 py-1.5 rounded-hx-radius-sm gap-1.5 flex items-center text-[13px] font-medium transition-colors ${
                tab === 'agents' ? 'active' : ''
              }`}
            >
              <Bot size={15} />
              Agent ({rawAgents.length})
            </button>
            <button
              onClick={() => { setTab('requests'); setSearchQuery(''); }}
              className={`hx-nav-item !w-auto !h-auto px-3 py-1.5 rounded-hx-radius-sm gap-1.5 flex items-center text-[13px] font-medium transition-colors relative ${
                tab === 'requests' ? 'active' : ''
              }`}
            >
              <UserPlus size={15} />
              请求
              {friendRequests.length > 0 && (
                <span className="hx-conv-badge static ml-1">
                  {friendRequests.length}
                </span>
              )}
            </button>
          </div>
          {/* 搜索 */}
          {(tab === 'friends' || tab === 'agents') && (
            <div className="hx-panel-search">
              <Search size={16} className="text-hx-text-tertiary" />
              <Input
                type="text"
                placeholder={tab === 'friends' ? '搜索姓名、唤星号...' : '搜索 Agent...'}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9"
              />
            </div>
          )}
        </div>

        <div className="hx-conv-list">
          {loading && rawContacts.length === 0 && rawAgents.length === 0 ? (
            <div className="hx-empty-state py-[60px]">
              <Loader2 size={24} className="animate-spin opacity-50" />
              <p className="text-[13px] mt-2">加载中...</p>
            </div>
          ) : tab === 'friends' ? (
            <>
              {/* ── 📂 我的星组 (顶部固定) ── */}
              {rawAgents.length > 0 && (
                <div className="hx-my-stars-section">
                  <div className="hx-my-stars-header">
                    <span>📂</span>
                    <span>我的星组 (My Entities)</span>
                  </div>
                  {rawAgents.map((agent: any) => {
                    const contactMock: ContactFull = {
                      id: 0,
                      peer: {
                        hasn_id: agent.hasn_id,
                        star_id: agent.node_id || 'LOCAL',
                        name: agent.name,
                        type: 'agent',
                        avatar_url: agent.avatar_url,
                      },
                      relation_type: 'social',
                      trust_level: 5,
                      trust_level_label: '所有者',
                      subscription: true,
                      status: 'connected',
                      owned_agents: [],
                      custom_permissions: {},
                    };
                    const isActive = selectedContact?.peer.hasn_id === agent.hasn_id;
                    return (
                      <div
                        key={`myagent-${agent.hasn_id}`}
                        className={`hx-conv-item ${isActive ? 'active' : ''}`}
                        onClick={() => setSelectedContact(contactMock)}
                      >
                        {agent.avatar_url ? (
                          <div className="relative shrink-0">
                            <img
                              src={agent.avatar_url}
                              alt={agent.name}
                              className="hx-conv-avatar object-cover ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel"
                            />
                            <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10 font-mono">✨</div>
                          </div>
                        ) : (
                          <div className="relative shrink-0">
                            <div className="hx-conv-avatar !text-white !text-sm bg-gradient-to-br from-hx-blue to-hx-purple ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel">
                              {getInitial(agent.name)}
                            </div>
                            <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10 font-mono">✨</div>
                          </div>
                        )}
                        <div className="hx-conv-info">
                          <div className="hx-conv-name-row">
                            <span className="hx-conv-name">✨ {agent.name}</span>
                            <span className={`w-2 h-2 rounded-full shrink-0 ${agent.online !== false ? 'bg-hx-green' : 'bg-amber-400'}`} />
                          </div>
                          <div className="hx-conv-preview">
                            #{agent.star_id || agent.node_id || 'LOCAL'}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              {/* ── 好友列表（按信任等级分组 + Agent 从属子列表） ── */}
              {filteredGroups.length === 0 ? (
                <div className="hx-empty-state py-[60px]">
                  <Users size={40} className="opacity-30 mb-2" />
                  <p className="text-[13px] text-hx-text-tertiary m-0">
                    {searchQuery ? '未找到匹配的联系人' : '暂无好友'}
                  </p>
                </div>
              ) : (
                filteredGroups.map((group: any) => {
                  // 跳过 owner 等级（已在我的星组中显示）
                  if (group.level === 5) return null;
                  return (
                    <div key={`group-${group.level}`} className="mb-2">
                      <div className="hx-entity-section">
                        <span>{group.emoji}</span>
                        <span>{group.label}</span>
                        <span className="count">({group.contacts.length})</span>
                      </div>
                      {group.contacts.map((contact: any) => (
                        <div key={contact.peer.hasn_id}>
                          {/* 联系人主行 */}
                          <div
                            className={`hx-conv-item ${selectedContact?.peer.hasn_id === contact.peer.hasn_id ? 'active' : ''}`}
                            onClick={() => setSelectedContact(contact)}
                          >
                            {contact.peer.avatar_url ? (
                              <div className="relative shrink-0">
                                <img
                                  src={contact.peer.avatar_url}
                                  alt={contact.peer.name}
                                  className="hx-conv-avatar object-cover ring-2 ring-hx-green ring-offset-1 ring-offset-hx-bg-panel"
                                />
                              </div>
                            ) : (
                              <div
                                className="hx-conv-avatar shrink-0 !text-white !text-sm bg-gradient-to-br from-[#7C3AED] to-[#6366F1] ring-2 ring-hx-green ring-offset-1 ring-offset-hx-bg-panel"
                              >
                                {getInitial(contact.nickname || contact.peer.name)}
                              </div>
                            )}
                            <div className="hx-conv-info">
                              <div className="hx-conv-name-row">
                                <span className="hx-conv-name">
                                  👥 {contact.nickname || contact.peer.name}
                                </span>
                                <span className={`w-2 h-2 rounded-full shrink-0 ${contact.status === 'connected' ? 'bg-hx-green' : 'bg-hx-red'}`} />
                              </div>
                              <div className="hx-conv-preview">
                                @{contact.peer.star_id} · {contact.trust_level_label || contact.relation_type}
                              </div>
                            </div>
                          </div>

                          {/* Agent 从属子列表 */}
                          {contact.owned_agents && contact.owned_agents.length > 0 && (
                            contact.owned_agents.map((agent: AgentPeer) => (
                              <div
                                key={`child-${agent.hasn_id}`}
                                className="hx-agent-child-row"
                                onClick={() => handleSelectAgentPeer(agent)}
                              >
                                <span className="hx-agent-child-prefix">└›</span>
                                {agent.avatar_url ? (
                                  <img
                                    src={agent.avatar_url}
                                    alt={agent.name}
                                    className="hx-agent-child-avatar object-cover"
                                  />
                                ) : (
                                  <div className="hx-agent-child-avatar bg-gradient-to-br from-[#6366F1] to-[#7C3AED]">
                                    {getInitial(agent.name)}
                                  </div>
                                )}
                                <div className="hx-agent-child-info">
                                  <span className="hx-agent-child-name">🟡 {agent.name}</span>
                                  <span className="hx-agent-child-role">{agent.role || 'Agent'}</span>
                                </div>
                              </div>
                            ))
                          )}
                        </div>
                      ))}
                    </div>
                  );
                })
              )}
            </>
          ) : tab === 'agents' ? (
            /* ── Agent Tab — 按归属人分组 ── */
            agentGroups.every(g => g.agents.length === 0) ? (
              <div className="hx-empty-state py-[60px]">
                <div className="text-[40px] opacity-30 mb-2">🤖</div>
                <p className="text-[13px] text-hx-text-tertiary m-0">
                  {searchQuery ? '未找到匹配的 Agent' : '暂无 Agent'}
                </p>
              </div>
            ) : (
              agentGroups.map((group) => {
                if (group.agents.length === 0) return null;
                return (
                  <div key={group.owner} className="mb-2">
                    <div className="hx-entity-section">
                      <span>{group.emoji}</span>
                      <span>{group.owner}</span>
                      <span className="count">({group.agents.length})</span>
                    </div>
                    {group.agents.map((agent) => {
                      // 伪造 ContactFull 以在详情页复用
                      const contactMock: ContactFull = {
                        id: 0,
                        peer: {
                          hasn_id: agent.hasn_id,
                          star_id: agent.star_id || 'LOCAL',
                          name: agent.name,
                          type: 'agent',
                          avatar_url: agent.avatar_url,
                        },
                        relation_type: 'social',
                        trust_level: group.emoji === '🤖' ? 5 : 2,
                        trust_level_label: group.emoji === '🤖' ? '所有者' : '好友Agent',
                        subscription: group.emoji === '🤖',
                        status: 'connected',
                        owned_agents: [],
                        custom_permissions: {},
                      };
                      const isActive = selectedContact?.peer.hasn_id === agent.hasn_id;
                      const isMyAgent = group.emoji === '🤖';
                      return (
                        <div
                          key={`agent-${agent.hasn_id}`}
                          className={`hx-conv-item ${isActive ? 'active' : ''}`}
                          onClick={() => setSelectedContact(contactMock)}
                        >
                          {agent.avatar_url ? (
                            <div className="relative shrink-0">
                              <img
                                src={agent.avatar_url}
                                alt={agent.name}
                                className={`hx-conv-avatar object-cover ${isMyAgent ? 'ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel' : 'ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel'}`}
                              />
                              {isMyAgent && <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10 font-mono">✨</div>}
                            </div>
                          ) : (
                            <div className="relative shrink-0">
                              <div className={`hx-conv-avatar !text-white !text-sm ${isMyAgent ? 'bg-gradient-to-br from-hx-blue to-hx-purple ring-2 ring-hx-blue ring-offset-1 ring-offset-hx-bg-panel' : 'bg-gradient-to-br from-[#6366F1] to-[#7C3AED] ring-2 ring-purple-400 ring-offset-1 ring-offset-hx-bg-panel'}`}>
                                {getInitial(agent.name)}
                              </div>
                              {isMyAgent && <div className="absolute -bottom-0.5 -right-0.5 text-[10px] bg-hx-bg-panel rounded-full z-10 font-mono">✨</div>}
                            </div>
                          )}
                          <div className="hx-conv-info">
                            <div className="hx-conv-name-row">
                              <span className="hx-conv-name">
                                {isMyAgent ? '✨' : '🟡'} {agent.name}
                              </span>
                              <TrustBadge level={isMyAgent ? 5 : 2} label={isMyAgent ? '我的Agent' : '好友Agent'} />
                            </div>
                            <div className="hx-conv-preview">
                              #{agent.star_id || agent.hasn_id.substring(0, 8)} · {agent.role || agent.type}
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                );
              })
            )
          ) : (
            /* ── 好友请求列表 ── */
            friendRequests.length === 0 ? (
              <div className="hx-empty-state py-[60px]">
                <UserPlus size={40} className="opacity-30 mb-2" />
                <p className="text-[13px] text-hx-text-tertiary">暂无好友请求</p>
              </div>
            ) : (
              friendRequests.map((req) => (
                <div key={req.id} className="hx-conv-item !cursor-default">
                  <div
                    className="hx-conv-avatar !text-white !text-sm bg-gradient-to-br from-[#7C3AED] to-[#6366F1]"
                  >
                    {getInitial(req.from_name)}
                  </div>
                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{req.from_name}</span>
                    </div>
                    <div className="hx-conv-preview text-hx-text-secondary">
                      {req.message || `@${req.from_star_id} 请求添加好友`}
                    </div>
                  </div>
                  {req.status === 'pending' && (
                    <div className="flex gap-1 shrink-0">
                      <button
                        onClick={() => handleRespondRequest(req.id, true)}
                        className="w-7 h-7 rounded-full border-none bg-hx-green text-white cursor-pointer flex items-center justify-center transition-opacity hover:opacity-80"
                        title="接受"
                      >
                        <Check size={14} />
                      </button>
                      <button
                        onClick={() => handleRespondRequest(req.id, false)}
                        className="w-7 h-7 rounded-full border border-hx-border bg-transparent text-hx-text-secondary cursor-pointer flex items-center justify-center transition-colors hover:bg-hx-bg-hover"
                        title="拒绝"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  )}
                </div>
              ))
            )
          )}
        </div>
      </div>

      {/* ===== 右侧详情 ===== */}
      {(!isMobile || mobileView === 'detail') && (
        <div className="hx-chat flex-1 bg-hx-bg-main relative">
          {selectedContact ? (
            <>
              {/* 移动端返回按钮 */}
              {isMobile && (
                <button
                  className="hx-mobile-header-back"
                  onClick={() => setSelectedContact(null)}
                  style={{ position: 'absolute', top: 8, left: 8, zIndex: 10 }}
                >
                  <ChevronLeft size={22} />
                </button>
              )}
              <ContactProfile
                contact={selectedContact}
                onStartChat={handleStartChat}
                onSelectAgent={handleSelectAgentPeer}
                onRefresh={refresh}
              />
            </>
          ) : (
            <div className="hx-empty-state h-full">
              <div className="icon">👥</div>
              <h3 className="text-[15px] font-semibold text-hx-text-primary mt-0 mb-1">通讯录</h3>
              <p className="text-[13px] text-hx-text-secondary">选择好友查看详情，或点击 + 添加新好友</p>
            </div>
          )}
        </div>
      )}

      {/* 添加好友 — 移动端用 BottomSheet，桌面端用 Dialog */}
      {isMobile ? (
        <BottomSheet
          isOpen={showAddDialog}
          onClose={() => setShowAddDialog(false)}
          title="添加好友"
        >
          {addFriendForm}
        </BottomSheet>
      ) : (
        showAddDialog && (
          <div
            className="fixed inset-0 bg-black/40 flex items-center justify-center z-[1000]"
            onClick={() => setShowAddDialog(false)}
          >
            <div
              className="bg-hx-bg-panel rounded-hx-radius-lg p-6 w-[360px] shadow-[0_20px_60px_rgba(0,0,0,0.2)]"
              onClick={(e) => e.stopPropagation()}
            >
              <h3 className="text-base font-semibold text-hx-text-primary mb-4 mt-0">
                添加好友
              </h3>
              {addFriendForm}
            </div>
          </div>
        )
      )}
    </div>
  );
}
