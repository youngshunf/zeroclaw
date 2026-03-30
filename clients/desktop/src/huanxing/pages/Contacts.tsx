/**
 * 联系人页面 — 接入 HASN 联系人 API
 */
import { useState, useCallback } from 'react';
import {
  Users,
  UserPlus,
  Search,
  Check,
  X,
  Loader2,
  MessageSquare,
} from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { Input } from '@/components/ui/Input';
import { Textarea } from '@/components/ui/Textarea';
import { useHasnContacts } from '@/huanxing/hooks/useHasn';
import * as hasnApi from '@/huanxing/lib/hasn-api';
import type { Contact } from '@/huanxing/lib/hasn-api';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

function TrustBadge({ level }: { level: number }) {
  const colors = ['#9ca3af', '#94a3b8', '#60a5fa', '#34d399', '#a78bfa', '#f59e0b'];
  const color = colors[level] || colors[0];
  return (
    <span
      className="text-[10px] px-1.5 py-[1px] rounded-lg font-medium"
      style={{
        background: `${color}20`,
        color: color,
      }}
    >
      L{level}
    </span>
  );
}

export default function Contacts() {
  const navigate = useNavigate();
  const [tab, setTab] = useState<'friends' | 'requests'>('friends');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedContact, setSelectedContact] = useState<Contact | null>(null);
  const { contacts, friendRequests, loading, refresh } = useHasnContacts();

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
  const handleStartChat = useCallback((contact: Contact) => {
    navigate('/hasn-chat', { state: { peerId: contact.hasn_id } });
  }, [navigate]);

  // 过滤联系人
  const filteredContacts = searchQuery
    ? contacts.filter((c) =>
        c.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        c.star_id.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : contacts;

  return (
    <div className="flex flex-1 h-full">
      {/* 左侧面板 */}
      <div className="hx-panel">
        <div className="hx-panel-header">
          <div className="flex items-center justify-between">
            <h2 className="hx-panel-title">通讯录</h2>
            <button
              className="hx-nav-item !w-8 !h-8 shrink-0"
              title="添加好友"
              onClick={() => setShowAddDialog(true)}
            >
              <UserPlus size={18} />
            </button>
          </div>
          {/* Tab 切换 */}
          <div className="flex gap-1 px-3 pb-2">
            <button
              onClick={() => setTab('friends')}
              className={`hx-nav-item !w-auto !h-auto px-3 py-1.5 rounded-hx-radius-sm gap-1.5 flex items-center text-[13px] font-medium transition-colors ${
                tab === 'friends' ? 'active' : ''
              }`}
            >
              <Users size={15} />
              好友 ({contacts.length})
            </button>
            <button
              onClick={() => setTab('requests')}
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
          {tab === 'friends' && (
            <div className="hx-panel-search">
              <Search size={16} className="text-hx-text-tertiary" />
              <Input
                type="text"
                placeholder="搜索联系人..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9"
              />
            </div>
          )}
        </div>

        <div className="hx-conv-list">
          {loading && contacts.length === 0 ? (
            <div className="hx-empty-state py-[60px]">
              <Loader2 size={24} className="animate-spin opacity-50" />
              <p className="text-[13px] mt-2">加载中...</p>
            </div>
          ) : tab === 'friends' ? (
            /* 好友列表 */
            filteredContacts.length === 0 ? (
              <div className="hx-empty-state py-[60px]">
                <Users size={40} className="opacity-30 mb-2" />
                <p className="text-[13px] text-hx-text-tertiary m-0">
                  {searchQuery ? '未找到匹配的联系人' : '暂无好友'}
                </p>
              </div>
            ) : (
              filteredContacts.map((contact) => (
                <div
                  key={contact.hasn_id}
                  className={`hx-conv-item ${selectedContact?.hasn_id === contact.hasn_id ? 'active' : ''}`}
                  onClick={() => setSelectedContact(contact)}
                >
                  <div
                    className={`hx-conv-avatar !text-white !text-sm ${
                      contact.peer_type === 'agent'
                        ? 'bg-gradient-to-br from-[#6366F1] to-[#7C3AED]'
                        : 'bg-gradient-to-br from-[#7C3AED] to-[#6366F1]'
                    }`}
                  >
                    {getInitial(contact.name)}
                  </div>
                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{contact.name}</span>
                      <TrustBadge level={contact.trust_level} />
                    </div>
                    <div className="hx-conv-preview">
                      @{contact.star_id} · {contact.relation_type}
                    </div>
                  </div>
                </div>
              ))
            )
          ) : (
            /* 好友请求列表 */
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

      {/* 右侧详情 */}
      <div className="hx-chat flex-1 bg-hx-bg-main relative">
        {selectedContact ? (
          <div className="flex flex-col items-center justify-center h-full gap-4">
            <div
              className={`w-[72px] h-[72px] rounded-full text-white font-bold text-[28px] flex items-center justify-center ${
                selectedContact.peer_type === 'agent'
                  ? 'bg-gradient-to-br from-[#6366F1] to-[#7C3AED]'
                  : 'bg-gradient-to-br from-[#7C3AED] to-[#6366F1]'
              }`}
            >
              {getInitial(selectedContact.name)}
            </div>
            <div className="text-center">
              <h3 className="text-lg font-semibold text-hx-text-primary m-0">
                {selectedContact.name}
              </h3>
              <p className="text-[13px] text-hx-text-secondary my-1">
                @{selectedContact.star_id}
              </p>
              <div className="flex gap-2 justify-center mt-1 items-center">
                <TrustBadge level={selectedContact.trust_level} />
                <span className="text-[11px] text-hx-text-tertiary">
                  {selectedContact.relation_type} · {selectedContact.peer_type}
                </span>
              </div>
            </div>
            <button
              onClick={() => handleStartChat(selectedContact)}
              className="mt-2 px-6 py-2 rounded-hx-radius-md border-none bg-hx-purple text-white text-[13px] font-medium cursor-pointer flex items-center gap-1.5 transition-opacity hover:opacity-90"
            >
              <MessageSquare size={16} />
              发消息
            </button>
          </div>
        ) : (
          <div className="hx-empty-state h-full">
            <div className="icon">👥</div>
            <h3 className="text-[15px] font-semibold text-hx-text-primary mt-0 mb-1">通讯录</h3>
            <p className="text-[13px] text-hx-text-secondary">选择好友查看详情，或点击 + 添加新好友</p>
          </div>
        )}
      </div>

      {/* 添加好友弹窗 */}
      {showAddDialog && (
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
          </div>
        </div>
      )}
    </div>
  );
}
