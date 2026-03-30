/**
 * useHasnContacts — 从 HASN API 获取真实联系人并转为 MentionItem[]
 *
 * 合并：
 * - 通讯录好友 (contacts)
 * - 我的 Agent 列表 (agents)
 */
import { useState, useEffect, useCallback } from 'react';
import * as hasnApi from '@/huanxing/lib/hasn-api';
import type { Contact, AgentInfo } from '@/huanxing/lib/hasn-api';
import type { MentionItem, MentionSection } from '@/huanxing/components/chat/input/HxMentionMenu';

/** 将 HASN Contact 转为 MentionItem */
function contactToMention(c: Contact): MentionItem {
  return {
    type: c.peer_type === 'agent' ? 'agent' : 'contact',
    id: c.hasn_id,
    label: c.name,
    description: c.relation_type === 'friend' ? '好友' : c.relation_type,
  };
}

/** 将 HASN AgentInfo 转为 MentionItem */
function agentToMention(a: AgentInfo): MentionItem {
  return {
    type: 'agent',
    id: a.hasn_id,
    label: a.name,
    description: a.online ? '在线' : '离线',
  };
}

export interface UseHasnContactsReturn {
  sections: MentionSection[];
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

export function useHasnContacts(): UseHasnContactsReturn {
  const [contacts, setContacts] = useState<MentionItem[]>([]);
  const [agents, setAgents] = useState<MentionItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [contactList, agentList] = await Promise.allSettled([
        hasnApi.getContacts(),
        hasnApi.getMyAgents(),
      ]);

      if (contactList.status === 'fulfilled') {
        const val = contactList.value as any;
        const arr = Array.isArray(val) ? val : (val.contacts || []);
        setContacts(arr.map(contactToMention));
      } else {
        console.warn('[useHasnContacts] Failed to fetch contacts:', contactList.reason);
      }

      if (agentList.status === 'fulfilled') {
        const val = agentList.value as any;
        const arr = Array.isArray(val) ? val : (val.agents || val.data || []);
        setAgents(arr.map(agentToMention));
      } else {
        console.warn('[useHasnContacts] Failed to fetch agents:', agentList.reason);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  // Fetch on mount
  useEffect(() => { fetch(); }, [fetch]);

  const sections: MentionSection[] = [];

  if (contacts.length > 0) {
    sections.push({ id: 'contacts', label: '联系人', items: contacts });
  }
  if (agents.length > 0) {
    sections.push({ id: 'agents', label: 'Agents', items: agents });
  }

  // 如果都没数据，显示占位
  if (sections.length === 0 && !loading) {
    sections.push({
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

  return { sections, loading, error, refresh: fetch };
}
