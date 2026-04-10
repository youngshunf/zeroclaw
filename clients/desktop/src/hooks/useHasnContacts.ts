/**
 * useHasnContacts — 从 HASN API 获取完整联系人数据
 *
 * 阶段四增强：
 * - 返回 rawContacts (ContactFull[])，含 owned_agents / custom_permissions
 * - 按 trust_level 分组的 contactsByTrust
 * - 我的 Agent 列表（rawAgents），含 avatar_url / role / description
 * - MentionSection[] 向后兼容
 * - 新增 myAgentIds Set 用于前端实体类型推断
 */
import { useState, useEffect, useCallback, useMemo } from 'react';
import * as hasnApi from '@/lib/hasn-api';
import type {
  ContactFull,
  AgentInfo,
  AgentPeer,
  TrustLevel,
} from '@/lib/hasn-api';
import { TRUST_LEVELS, TRUST_LEVEL_LABELS } from '@/lib/hasn-api';
import type { MentionItem, MentionSection } from '@/components/chat/input/HxMentionMenu';

// ── 工具函数 ──────────────────────────────────────

/** 将 ContactFull 转为 MentionItem（用于 @提及输入） */
function contactToMention(c: ContactFull): MentionItem {
  return {
    type: c.peer.type === 'agent' ? 'agent' : 'contact',
    id: c.peer.hasn_id,
    label: c.nickname || c.peer.name,
    description: TRUST_LEVEL_LABELS[c.trust_level] ?? c.relation_type,
    avatar: c.peer.avatar_url,
  };
}

/** 将 AgentInfo 转为 MentionItem */
function agentToMention(a: AgentInfo): MentionItem {
  return {
    type: 'agent',
    id: a.hasn_id,
    label: a.name,
    description: a.online ? '在线' : '离线',
    avatar: a.avatar_url,
  };
}

/** 联系人实体类型推断 */
export function inferConversationType(
  peerType: string,
  peerHasnId: string,
  myAgentIds: Set<string>,
): 'human' | 'my-agent' | 'friend-agent' {
  if (peerType === 'human') return 'human';
  if (peerType === 'agent' && myAgentIds.has(peerHasnId)) return 'my-agent';
  return 'friend-agent';
}

// ── 分组配置 ────────────────────────────────────
export interface TrustGroup {
  level: TrustLevel;
  label: string;
  emoji: string;
  contacts: ContactFull[];
}

const TRUST_GROUPS_CONFIG: Array<{ level: TrustLevel; label: string; emoji: string }> = [
  { level: TRUST_LEVELS.OWNER,   label: '我的 Agent', emoji: '🤖' },  // level 5（owner）
  { level: TRUST_LEVELS.TRUSTED, label: '密友',       emoji: '⭐' },  // level 4
  { level: TRUST_LEVELS.FRIEND,  label: '朋友',       emoji: '🤝' },  // level 3
  { level: TRUST_LEVELS.NORMAL,  label: '联系人',     emoji: '👤' },  // level 2
  { level: TRUST_LEVELS.STRANGER,label: '陌生人',     emoji: '❓' },  // level 1（通常隐藏）
];

// ── Hook 返回类型 ────────────────────────────────
export interface UseHasnContactsReturn {
  /** MentionSection[]（@提及输入使用） */
  sections: MentionSection[];
  /** 我的 Agent 原始列表（含 avatar_url / role） */
  rawAgents: AgentInfo[];
  /** 联系人原始列表（完整 ContactFull，含 owned_agents） */
  rawContacts: ContactFull[];
  /** 按信任等级分组 */
  contactsByTrust: TrustGroup[];
  /** 好友请求列表 */
  friendRequests: any[];
  /** 我的 Agent hasn_id 集合（用于推断实体类型） */
  myAgentIds: Set<string>;
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

// ── Hook 实现 ────────────────────────────────────
export function useHasnContacts(): UseHasnContactsReturn {
  const [rawContacts, setRawContacts]   = useState<ContactFull[]>([]);
  const [mentionContacts, setMentionContacts] = useState<MentionItem[]>([]);
  const [rawAgents, setRawAgents]       = useState<AgentInfo[]>([]);
  const [mentionAgents, setMentionAgents]  = useState<MentionItem[]>([]);
  const [friendRequests, setFriendRequests] = useState<any[]>([]);
  const [loading, setLoading]           = useState(false);
  const [error, setError]               = useState<string | null>(null);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [contactRes, agentRes, frRes] = await Promise.allSettled([
        hasnApi.getContacts(),
        hasnApi.getMyAgents(),
        hasnApi.getFriendRequests(),
      ]);

      if (contactRes.status === 'fulfilled') {
        const contacts = contactRes.value;
        setRawContacts(contacts);
        setMentionContacts(contacts.map(contactToMention));
      } else {
        console.warn('[useHasnContacts] contacts fetch failed:', contactRes.reason);
      }

      if (agentRes.status === 'fulfilled') {
        const agents = agentRes.value;
        setRawAgents(agents);
        setMentionAgents(agents.map(agentToMention));
      } else {
        console.warn('[useHasnContacts] agents fetch failed:', agentRes.reason);
      }

      if (frRes.status === 'fulfilled') {
        const fr = Array.isArray(frRes.value) ? frRes.value : ((frRes.value as any).requests || []);
        setFriendRequests(fr);
      } else {
        console.warn('[useHasnContacts] friend requests fetch failed:', frRes.reason);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  // ── 派生数据 ──────────────────────────────────

  /** 我的 Agent hasn_id 集合 */
  const myAgentIds = useMemo(
    () => new Set(rawAgents.map((a) => a.hasn_id)),
    [rawAgents],
  );

  /** 按信任等级分组联系人（仅显示 2/3/4/5，排除 blocked） */
  const contactsByTrust = useMemo<TrustGroup[]>(() => {
    return TRUST_GROUPS_CONFIG
      .map(({ level, label, emoji }) => ({
        level,
        label,
        emoji,
        contacts: rawContacts.filter(
          (c) => c.trust_level === level && c.status !== 'blocked',
        ),
      }))
      .filter((g) => g.contacts.length > 0);
  }, [rawContacts]);

  /** MentionSection[] 保持向后兼容 */
  const sections: MentionSection[] = useMemo(() => {
    const result: MentionSection[] = [];
    if (mentionContacts.length > 0) {
      result.push({ id: 'contacts', label: '联系人', items: mentionContacts });
    }
    if (mentionAgents.length > 0) {
      result.push({ id: 'agents', label: 'Agents', items: mentionAgents });
    }
    if (result.length === 0 && !loading) {
      result.push({
        id: 'empty',
        label: '联系人',
        items: [{
          type: 'contact',
          id: '_empty',
          label: '暂无联系人',
          description: '在 HASN 中添加好友后将会显示',
        }],
      });
    }
    return result;
  }, [mentionContacts, mentionAgents, loading]);

  return {
    sections,
    rawAgents,
    rawContacts,
    contactsByTrust,
    friendRequests,
    myAgentIds,
    loading,
    error,
    refresh: fetchAll,
  };
}
