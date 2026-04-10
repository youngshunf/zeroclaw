/**
 * ContactProfile — 通讯录右侧详情 Profile 组件
 *
 * 包含：头像区 + 操作按钮 + 简介 + 能力声明 + 隐私控制 + 名下 Agent
 * 对齐线框图 §四 通讯录主界面右侧设计
 */
import { useState, useCallback } from 'react';
import {
  MessageSquare,
  Edit3,
  Shield,
  ChevronDown,
  ChevronUp,
  Zap,
  Lock,
  User,
  Bot,
} from 'lucide-react';
import { TrustBadge } from './TrustLevelSelector';
import type { ContactFull, AgentPeer, TrustLevel, PermissionState } from '@/lib/hasn-api';
import { TRUST_LEVEL_LABELS } from '@/lib/hasn-api';
import * as hasnApi from '@/lib/hasn-api';

function getInitial(name: string): string {
  return name.charAt(0) || '?';
}

/** 默认社交权限行为标签 */
const SOCIAL_ACTION_LABELS: Record<string, string> = {
  send_message: '发送消息',
  view_public_info: '查看公开信息',
  view_schedule: '查看日程',
  view_preferences: '查看偏好详情',
  view_location: '查看位置',
  make_appointment: '帮预约',
  make_commitment: '代做承诺',
  view_sensitive: '查看敏感信息',
};

/** 铁律锁定的 action (不可修改) */
const IRON_LAW_ACTIONS = new Set(['view_sensitive', 'make_commitment']);

interface ContactProfileProps {
  contact: ContactFull;
  onStartChat: (contact: ContactFull) => void;
  onSelectAgent?: (agentPeer: AgentPeer) => void;
  onRefresh?: () => void;
}

export default function ContactProfile({
  contact,
  onStartChat,
  onSelectAgent,
  onRefresh,
}: ContactProfileProps) {
  const [privacyExpanded, setPrivacyExpanded] = useState(false);
  const [trustUpdating, setTrustUpdating] = useState(false);

  const isAgent = contact.peer.type === 'agent';
  const isOwner = contact.trust_level === 5;

  // ── 修改信任等级 ──────────────────
  const handleTrustChange = useCallback(async (newLevel: TrustLevel) => {
    setTrustUpdating(true);
    try {
      await hasnApi.updateTrustLevel(contact.id, newLevel, contact.relation_type);
      onRefresh?.();
    } catch (err) {
      console.warn('[ContactProfile] Failed to update trust level:', err);
    } finally {
      setTrustUpdating(false);
    }
  }, [contact.id, contact.relation_type, onRefresh]);

  return (
    <div className="hx-contact-profile">
      {/* ── Profile Header ── */}
      <div className="hx-profile-header">
        {/* Avatar */}
        <div style={{ width: 80, height: 80, minWidth: 80, minHeight: 80, borderRadius: '50%', overflow: 'hidden' }} className="mb-4 relative shrink-0">
          {contact.peer.avatar_url ? (
            <img src={contact.peer.avatar_url} alt={contact.peer.name} style={{ width: '100%', height: '100%', objectFit: 'cover' }} />
          ) : (
            <div
              style={{ width: '100%', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
              className={`text-white font-bold text-[32px] ${
                isAgent
                  ? 'bg-gradient-to-br from-[#6366F1] to-[#7C3AED]'
                  : 'bg-gradient-to-br from-[#7C3AED] to-[#6366F1]'
              }`}
            >
              {getInitial(contact.nickname || contact.peer.name)}
            </div>
          )}
        </div>

        {/* Name & Star ID */}
        <h3 className="hx-profile-name">
          {isAgent && '🤖 '}{contact.nickname || contact.peer.name}
        </h3>
        <p className="hx-profile-star-id">@{contact.peer.star_id}</p>

        {/* Badges */}
        <div className="hx-profile-badges">
          <TrustBadge
            level={contact.trust_level}
            label={contact.trust_level_label}
            editable={!isOwner}
            onSelect={handleTrustChange}
            disableOwner={!isAgent}
          />
          <span className="text-[11px] text-hx-text-tertiary">
            {contact.relation_type}
          </span>
        </div>

        {/* Owner info for Agent */}
        {isAgent && contact.peer.status && (
          <div className="hx-profile-owner-info mb-3">
            <User size={12} />
            <span>归属: {contact.peer.status}</span>
          </div>
        )}

        {/* Action buttons */}
        <div className="hx-profile-actions">
          <button
            className="hx-profile-action-btn primary"
            onClick={() => onStartChat(contact)}
          >
            <MessageSquare size={15} />
            发消息
          </button>
          {isAgent ? (
            <button
              className="hx-profile-action-btn secondary"
              onClick={() => {
                // TODO: 委派星灵对接 API
              }}
            >
              <Bot size={15} />
              委派星灵对接
            </button>
          ) : (
            <button
              className="hx-profile-action-btn secondary"
              onClick={() => {
                // TODO: 修改备注
              }}
            >
              <Edit3 size={15} />
              修改备注
            </button>
          )}
        </div>
      </div>

      {/* ── 📝 简介 (Profile) ── */}
      <div className="hx-profile-section">
        <h4 className="hx-profile-section-title">
          📝 简介
        </h4>
        <div className="hx-profile-section-content">
          {isAgent ? (
            <p className="m-0">
              {/* Agent description — mock data for now */}
              {contact.peer.name} 是一个专业的 AI Agent 数字分身。
            </p>
          ) : (
            <div className="flex flex-col gap-1">
              <span>关系类型: {contact.relation_type}</span>
              <span>信任等级: {TRUST_LEVEL_LABELS[contact.trust_level]}</span>
              {contact.connected_at && (
                <span>建立时间: {new Date(contact.connected_at).toLocaleDateString('zh-CN')}</span>
              )}
            </div>
          )}
        </div>
      </div>

      {/* ── ⚡ 能力声明 (Capabilities) — 仅 Agent ── */}
      {isAgent && (
        <div className="hx-profile-section">
          <h4 className="hx-profile-section-title">
            <Zap size={14} className="text-hx-purple" />
            能力声明
          </h4>
          <div className="hx-profile-section-content">
            {/* Capabilities from data or placeholders */}
            <div className="hx-profile-capability-item">
              <span>读取/对齐 Excel 账单</span>
            </div>
            <div className="hx-profile-capability-item">
              <span>提供授权公开的日历预约排期</span>
            </div>
            <div className="hx-profile-capability-item">
              <span>基础数据分析与报告</span>
            </div>
          </div>
        </div>
      )}

      {/* ── 名下 Agent ── 仅 Human 联系人 */}
      {!isAgent && contact.owned_agents.length > 0 && (
        <div className="hx-profile-section">
          <h4 className="hx-profile-section-title">
            <Bot size={14} className="text-hx-purple" />
            名下 Agent ({contact.owned_agents.length})
          </h4>
          <div className="flex flex-col gap-1">
            {contact.owned_agents.map((agent) => (
              <div
                key={agent.hasn_id}
                className="hx-agent-child-row !ml-0"
                onClick={() => onSelectAgent?.(agent)}
              >
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
                  <span className="hx-agent-child-role">{agent.role || agent.type}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── ⚠️ 隐私控制 (Privacy Control) ── */}
      <div className="hx-profile-section">
        <h4
          className="hx-profile-section-title cursor-pointer select-none"
          onClick={() => setPrivacyExpanded(!privacyExpanded)}
        >
          <Shield size={14} className="text-hx-purple" />
          隐私控制
          {privacyExpanded
            ? <ChevronUp size={14} className="ml-auto text-hx-text-tertiary" />
            : <ChevronDown size={14} className="ml-auto text-hx-text-tertiary" />}
        </h4>

        {privacyExpanded && (
          <div className="mt-2">
            {Object.entries(SOCIAL_ACTION_LABELS).map(([action, label]) => {
              const isLocked = IRON_LAW_ACTIONS.has(action);
              const currentState: PermissionState =
                (contact.custom_permissions as any)?.[action] || 'deny';
              const isAllowed = currentState === 'allow';

              return (
                <div key={action} className="hx-permission-row">
                  <span className="hx-permission-label">
                    {label}
                    {isLocked && <span className="iron-law">🔒 铁律</span>}
                  </span>
                  <button
                    className={`hx-permission-toggle ${isAllowed ? 'active' : ''} ${isLocked ? 'locked' : ''}`}
                    disabled={isLocked}
                    onClick={async () => {
                      if (isLocked) return;
                      try {
                        const newPerms = {
                          ...contact.custom_permissions,
                          [action]: isAllowed ? 'deny' : 'allow',
                        };
                        await hasnApi.updateContactPermissions(contact.id, newPerms as any);
                        onRefresh?.();
                      } catch (err) {
                        console.warn('[ContactProfile] Permission update failed:', err);
                      }
                    }}
                  />
                </div>
              );
            })}
            <div className="text-[10px] text-hx-text-tertiary mt-2 opacity-60">
              🔒 标记项受 HASN 协议铁律约束，不可覆盖
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
