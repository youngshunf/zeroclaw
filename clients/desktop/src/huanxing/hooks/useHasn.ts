/**
 * useHasn — HASN IM 状态管理 Hooks
 *
 * 管理连接状态、会话列表、消息、联系人、实时事件。
 * 对齐 hasn-api.ts 类型和 hasn-ws.ts 事件。
 */
import { useState, useEffect, useCallback, useRef } from "react";
import type { Conversation, Message, Contact, FriendRequest } from "../lib/hasn-api";
import * as hasnApi from "../lib/hasn-api";
import { hasnWs, type HasnWsEvent } from "../lib/hasn-ws";

// ---------- 连接状态 ----------

export function useHasnConnection() {
  const [connected, setConnected] = useState(false);
  const [status, setStatus] = useState<string>("disconnected");

  useEffect(() => {
    const unsub = hasnWs.subscribe((event: HasnWsEvent) => {
      if (event.type === "connected") {
        setConnected(true);
        setStatus("connected");
      } else if (event.type === "disconnected" || event.type === "error") {
        setConnected(false);
        setStatus("disconnected");
      }
    });
    return unsub;
  }, []);

  const connect = useCallback(async (platformToken: string, hasnId: string, starId: string) => {
    setStatus("connecting");
    try {
      await hasnApi.hasnConnect(platformToken, hasnId, starId);
      await hasnWs.connect();
      setConnected(true);
      setStatus("connected");
    } catch (err) {
      setStatus("disconnected");
      throw err;
    }
  }, []);

  const disconnect = useCallback(async () => {
    hasnWs.disconnect();
    await hasnApi.hasnDisconnect();
    setConnected(false);
    setStatus("disconnected");
  }, []);

  return { connected, status, connect, disconnect };
}

// ---------- 会话列表 ----------

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

  // 实时消息到达时更新会话列表
  useEffect(() => {
    const unsub = hasnWs.subscribe((event: HasnWsEvent) => {
      if (event.type === "message") {
        const msg = event.data;
        setConversations((prev) => {
          const idx = prev.findIndex((c) => c.id === msg.conversation_id);
          if (idx >= 0) {
            const updated = [...prev];
            updated[idx] = {
              ...updated[idx],
              last_message: typeof msg.content === "string" ? msg.content : "[消息]",
              last_message_at: msg.created_at || new Date().toISOString(),
              unread_count: updated[idx].unread_count + 1,
            };
            // 将有新消息的会话移到顶部
            const [item] = updated.splice(idx, 1);
            updated.unshift(item);
            return updated;
          }
          // 新会话 — 刷新整个列表
          refresh();
          return prev;
        });
      }
    });
    return unsub;
  }, [refresh]);

  const totalUnread = conversations.reduce((sum, c) => sum + c.unread_count, 0);

  return { conversations, totalUnread, loading, error, refresh, setConversations };
}

// ---------- 消息列表 ----------

export function useHasnMessages(conversationId: string | null) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const convIdRef = useRef(conversationId);
  convIdRef.current = conversationId;

  const loadMessages = useCallback(async (beforeId?: number) => {
    if (!conversationId) return;
    setLoading(true);
    try {
      const data = await hasnApi.getMessages(conversationId, 50, beforeId);
      if (beforeId) {
        setMessages((prev) => [...data, ...prev]);
      } else {
        setMessages(data);
      }
    } catch {
      // 静默
    } finally {
      setLoading(false);
    }
  }, [conversationId]);

  // 切换会话时重新加载
  useEffect(() => {
    setMessages([]);
    if (conversationId) loadMessages();
  }, [conversationId, loadMessages]);

  // 实时消息推送
  useEffect(() => {
    const unsub = hasnWs.subscribe((event: HasnWsEvent) => {
      if (event.type === "message" && event.data.conversation_id === convIdRef.current) {
        setMessages((prev) => [...prev, event.data]);
      } else if (event.type === "ack" && event.data.conversation_id === convIdRef.current) {
        // 更新消息发送状态
        setMessages((prev) =>
          prev.map((m) =>
            m.local_id === event.data.local_id
              ? { ...m, id: event.data.server_id ?? m.id, send_status: "sent" }
              : m,
          ),
        );
      } else if (event.type === "message_recalled") {
        setMessages((prev) =>
          prev.filter((m) => m.id !== event.data.message_id),
        );
      }
    });
    return unsub;
  }, []);

  const send = useCallback(async (content: string, replyToId?: number) => {
    if (!conversationId) return;
    // 乐观插入
    const tempMsg: Message = {
      id: 0,
      local_id: `local_${Date.now()}`,
      conversation_id: conversationId,
      from_id: "",
      from_type: 1,
      content,
      content_type: 1,
      status: 0,
      send_status: "sending",
      created_at: new Date().toISOString(),
      reply_to_id: replyToId,
    };
    setMessages((prev) => [...prev, tempMsg]);

    try {
      const sent = await hasnApi.sendMessage(conversationId, content, replyToId);
      setMessages((prev) =>
        prev.map((m) => (m.local_id === tempMsg.local_id ? { ...sent, send_status: "sent" } : m)),
      );
    } catch {
      setMessages((prev) =>
        prev.map((m) => (m.local_id === tempMsg.local_id ? { ...m, send_status: "failed" } : m)),
      );
    }
  }, [conversationId]);

  const loadMore = useCallback(() => {
    if (messages.length > 0) {
      loadMessages(messages[0].id);
    }
  }, [messages, loadMessages]);

  return { messages, loading, send, loadMore, refresh: loadMessages };
}

// ---------- 联系人 ----------

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
