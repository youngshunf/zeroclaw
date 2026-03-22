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
import { useHasnContacts } from '@/huanxing/hooks/useHasn';
import * as hasnApi from '@/huanxing/lib/hasn-api';
import type { Contact, FriendRequest } from '@/huanxing/lib/hasn-api';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

function TrustBadge({ level }: { level: number }) {
  const colors = ['var(--hx-text-tertiary)', '#94a3b8', '#60a5fa', '#34d399', '#a78bfa', '#f59e0b'];
  return (
    <span
      style={{
        fontSize: 10,
        padding: '1px 6px',
        borderRadius: 8,
        background: `${colors[level] || colors[0]}20`,
        color: colors[level] || colors[0],
        fontWeight: 500,
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
  const { contacts, friendRequests, loading, error, refresh } = useHasnContacts();

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
    <div style={{ display: 'flex', flex: 1, height: '100%' }}>
      {/* 左侧面板 */}
      <div className="hx-panel">
        <div className="hx-panel-header">
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <h2 className="hx-panel-title">通讯录</h2>
            <button
              className="hx-nav-item"
              style={{ width: 32, height: 32, flexShrink: 0 }}
              title="添加好友"
              onClick={() => setShowAddDialog(true)}
            >
              <UserPlus size={18} />
            </button>
          </div>
          {/* Tab 切换 */}
          <div style={{ display: 'flex', gap: 4, padding: '0 12px 8px' }}>
            <button
              onClick={() => setTab('friends')}
              className={`hx-nav-item ${tab === 'friends' ? 'active' : ''}`}
              style={{ width: 'auto', height: 'auto', padding: '6px 12px', borderRadius: 'var(--hx-radius-sm)', gap: 6, display: 'flex', alignItems: 'center', fontSize: 13, fontWeight: 500 }}
            >
              <Users size={15} />
              好友 ({contacts.length})
            </button>
            <button
              onClick={() => setTab('requests')}
              className={`hx-nav-item ${tab === 'requests' ? 'active' : ''}`}
              style={{ width: 'auto', height: 'auto', padding: '6px 12px', borderRadius: 'var(--hx-radius-sm)', gap: 6, display: 'flex', alignItems: 'center', fontSize: 13, fontWeight: 500, position: 'relative' }}
            >
              <UserPlus size={15} />
              请求
              {friendRequests.length > 0 && (
                <span className="hx-conv-badge" style={{ position: 'static', marginLeft: 4 }}>
                  {friendRequests.length}
                </span>
              )}
            </button>
          </div>
          {/* 搜索 */}
          {tab === 'friends' && (
            <div className="hx-panel-search">
              <Search size={16} />
              <input
                type="text"
                placeholder="搜索联系人..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          )}
        </div>

        <div className="hx-conv-list">
          {loading && contacts.length === 0 ? (
            <div className="hx-empty-state" style={{ padding: '60px 0' }}>
              <Loader2 size={24} className="animate-spin" style={{ opacity: 0.5 }} />
              <p style={{ fontSize: 13 }}>加载中...</p>
            </div>
          ) : tab === 'friends' ? (
            /* 好友列表 */
            filteredContacts.length === 0 ? (
              <div className="hx-empty-state" style={{ padding: '60px 0' }}>
                <Users size={40} style={{ opacity: 0.3 }} />
                <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>
                  {searchQuery ? '未找到匹配的联系人' : '暂无好友'}
                </p>
              </div>
            ) : (
              filteredContacts.map((contact) => (
                <div
                  key={contact.hasn_id}
                  className={`hx-conv-item${selectedContact?.hasn_id === contact.hasn_id ? ' active' : ''}`}
                  onClick={() => setSelectedContact(contact)}
                >
                  <div
                    className="hx-conv-avatar"
                    style={{
                      background: contact.peer_type === 'agent'
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
              <div className="hx-empty-state" style={{ padding: '60px 0' }}>
                <UserPlus size={40} style={{ opacity: 0.3 }} />
                <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>暂无好友请求</p>
              </div>
            ) : (
              friendRequests.map((req) => (
                <div key={req.id} className="hx-conv-item" style={{ cursor: 'default' }}>
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
                    }}
                  >
                    {getInitial(req.from_name)}
                  </div>
                  <div className="hx-conv-info">
                    <div className="hx-conv-name-row">
                      <span className="hx-conv-name">{req.from_name}</span>
                    </div>
                    <div className="hx-conv-preview">
                      {req.message || `@${req.from_star_id} 请求添加好友`}
                    </div>
                  </div>
                  {req.status === 'pending' && (
                    <div style={{ display: 'flex', gap: 4, flexShrink: 0 }}>
                      <button
                        onClick={() => handleRespondRequest(req.id, true)}
                        style={{
                          width: 28,
                          height: 28,
                          borderRadius: '50%',
                          border: 'none',
                          background: 'var(--hx-green)',
                          color: 'white',
                          cursor: 'pointer',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                        }}
                        title="接受"
                      >
                        <Check size={14} />
                      </button>
                      <button
                        onClick={() => handleRespondRequest(req.id, false)}
                        style={{
                          width: 28,
                          height: 28,
                          borderRadius: '50%',
                          border: '1px solid var(--hx-border)',
                          background: 'transparent',
                          color: 'var(--hx-text-secondary)',
                          cursor: 'pointer',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                        }}
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
      <div className="hx-chat">
        {selectedContact ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: 16 }}>
            <div
              style={{
                width: 72,
                height: 72,
                borderRadius: '50%',
                background: selectedContact.peer_type === 'agent'
                  ? 'linear-gradient(135deg, var(--hx-blue), var(--hx-purple))'
                  : 'linear-gradient(135deg, var(--hx-purple), var(--hx-blue))',
                color: 'white',
                fontWeight: 700,
                fontSize: 28,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              {getInitial(selectedContact.name)}
            </div>
            <div style={{ textAlign: 'center' }}>
              <h3 style={{ fontSize: 18, fontWeight: 600, color: 'var(--hx-text-primary)', margin: 0 }}>
                {selectedContact.name}
              </h3>
              <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', margin: '4px 0' }}>
                @{selectedContact.star_id}
              </p>
              <div style={{ display: 'flex', gap: 8, justifyContent: 'center', marginTop: 4 }}>
                <TrustBadge level={selectedContact.trust_level} />
                <span style={{ fontSize: 11, color: 'var(--hx-text-tertiary)' }}>
                  {selectedContact.relation_type} · {selectedContact.peer_type}
                </span>
              </div>
            </div>
            <button
              onClick={() => handleStartChat(selectedContact)}
              style={{
                marginTop: 8,
                padding: '8px 24px',
                borderRadius: 'var(--hx-radius-md)',
                border: 'none',
                background: 'var(--hx-purple)',
                color: 'white',
                fontSize: 13,
                fontWeight: 500,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                gap: 6,
              }}
            >
              <MessageSquare size={16} />
              发消息
            </button>
          </div>
        ) : (
          <div className="hx-empty-state">
            <div className="icon">👥</div>
            <h3>通讯录</h3>
            <p>选择好友查看详情，或点击 + 添加新好友</p>
          </div>
        )}
      </div>

      {/* 添加好友弹窗 */}
      {showAddDialog && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.4)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={() => setShowAddDialog(false)}
        >
          <div
            style={{
              background: 'var(--hx-bg-panel)',
              borderRadius: 'var(--hx-radius-lg)',
              padding: 24,
              width: 360,
              boxShadow: '0 20px 60px rgba(0,0,0,0.2)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ fontSize: 16, fontWeight: 600, margin: '0 0 16px', color: 'var(--hx-text-primary)' }}>
              添加好友
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <input
                type="text"
                placeholder="输入对方 Star ID"
                value={addStarId}
                onChange={(e) => setAddStarId(e.target.value)}
                style={{
                  padding: '8px 12px',
                  borderRadius: 'var(--hx-radius-sm)',
                  border: '1px solid var(--hx-border)',
                  background: 'var(--hx-bg-main)',
                  color: 'var(--hx-text-primary)',
                  fontSize: 13,
                  outline: 'none',
                }}
              />
              <textarea
                placeholder="附言（可选）"
                value={addMessage}
                onChange={(e) => setAddMessage(e.target.value)}
                rows={2}
                style={{
                  padding: '8px 12px',
                  borderRadius: 'var(--hx-radius-sm)',
                  border: '1px solid var(--hx-border)',
                  background: 'var(--hx-bg-main)',
                  color: 'var(--hx-text-primary)',
                  fontSize: 13,
                  outline: 'none',
                  resize: 'none',
                }}
              />
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                <button
                  onClick={() => setShowAddDialog(false)}
                  style={{
                    padding: '6px 16px',
                    borderRadius: 'var(--hx-radius-sm)',
                    border: '1px solid var(--hx-border)',
                    background: 'transparent',
                    color: 'var(--hx-text-secondary)',
                    fontSize: 13,
                    cursor: 'pointer',
                  }}
                >
                  取消
                </button>
                <button
                  onClick={handleAddFriend}
                  disabled={!addStarId.trim() || addLoading}
                  style={{
                    padding: '6px 16px',
                    borderRadius: 'var(--hx-radius-sm)',
                    border: 'none',
                    background: 'var(--hx-purple)',
                    color: 'white',
                    fontSize: 13,
                    cursor: 'pointer',
                    opacity: !addStarId.trim() || addLoading ? 0.5 : 1,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                  }}
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
