/**
 * useHasn — HASN IM 状态管理 Hook
 *
 * 管理会话列表、联系人、未读计数、实时事件。
 * Phase 2 逐步填充实际逻辑。
 */
import { useState, useEffect, useCallback } from "react";
import type { Conversation, Contact, FriendRequest } from "../lib/hasn-api";
import * as hasnApi from "../lib/hasn-api";

interface HasnState {
  conversations: Conversation[];
  contacts: Contact[];
  friendRequests: FriendRequest[];
  totalUnread: number;
  loading: boolean;
  error: string | null;
}

export function useHasnConversations() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await hasnApi.getConversations();
      setConversations(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const totalUnread = conversations.reduce((sum, c) => sum + c.unread_count, 0);

  return { conversations, totalUnread, loading, error, refresh };
}

export function useHasnContacts() {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [friendRequests, setFriendRequests] = useState<FriendRequest[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [c, fr] = await Promise.all([
        hasnApi.getContacts(),
        hasnApi.getFriendRequests(),
      ]);
      setContacts(c);
      setFriendRequests(fr);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  return { contacts, friendRequests, loading, error, refresh };
}

export function useHasnMessages(conversationId: string | null) {
  const [messages, setMessages] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);

  const loadMessages = useCallback(async () => {
    if (!conversationId) return;
    setLoading(true);
    try {
      const data = await hasnApi.getMessages(conversationId);
      setMessages(data);
    } catch {
      // silent
    } finally {
      setLoading(false);
    }
  }, [conversationId]);

  useEffect(() => { loadMessages(); }, [loadMessages]);

  const send = useCallback(async (content: string) => {
    if (!conversationId) return;
    try {
      const msg = await hasnApi.sendMessage(conversationId, content);
      setMessages((prev) => [...prev, msg]);
    } catch {
      // TODO: retry queue
    }
  }, [conversationId]);

  return { messages, loading, send, refresh: loadMessages };
}
