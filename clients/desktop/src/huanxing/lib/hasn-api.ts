/**
 * HASN API 双模适配层
 *
 * 自动检测运行环境：
 * - Tauri 桌面端：通过 invoke() 调用 Rust 后端
 * - Web 浏览器：通过 fetch() 调用 HTTP API
 *
 * 所有 HASN 相关的前端组件统一通过此模块通信。
 */

// ---------- 类型定义 ----------

export interface Conversation {
  id: string;
  peer_hasn_uuid: string;
  peer_name: string;
  peer_star_id: string;
  last_message?: string;
  last_message_at?: string;
  unread_count: number;
  is_pinned: boolean;
  is_muted: boolean;
}

export interface Message {
  id: string;
  conversation_id: string;
  sender_id: string;
  content: string;
  message_type: "text" | "image" | "file";
  status: "sending" | "sent" | "delivered" | "read" | "failed";
  sent_at: string;
  read_at?: string;
}

export interface Contact {
  hasn_uuid: string;
  star_id: string;
  nickname: string;
  avatar_url?: string;
  relation_type: "social" | "commerce" | "service" | "professional";
  is_online: boolean;
  last_seen?: string;
}

export interface FriendRequest {
  id: number;
  from_uuid: string;
  from_star_id: string;
  from_nickname: string;
  message?: string;
  created_at: string;
  status: "pending" | "accepted" | "rejected";
}

// ---------- 环境检测 ----------

function getTauriInvoke(): ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null {
  const internals = (window as any).__TAURI_INTERNALS__;
  return internals?.invoke ?? null;
}

const HASN_API_BASE = "/api/v1/hasn";

async function httpGet<T>(path: string): Promise<T> {
  const token = localStorage.getItem("zeroclaw:token");
  const resp = await fetch(`${HASN_API_BASE}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  return resp.json();
}

async function httpPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const token = localStorage.getItem("zeroclaw:token");
  const resp = await fetch(`${HASN_API_BASE}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  return resp.json();
}

// ---------- 会话 API ----------

export async function getConversations(): Promise<Conversation[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_conversations") as Promise<Conversation[]>;
  return httpGet<Conversation[]>("/conversations");
}

export async function getMessages(conversationId: string, limit = 50, before?: string): Promise<Message[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_messages", { conversationId, limit, before }) as Promise<Message[]>;
  const params = new URLSearchParams({ limit: String(limit) });
  if (before) params.set("before", before);
  return httpGet<Message[]>(`/conversations/${conversationId}/messages?${params}`);
}

export async function sendMessage(to: string, content: string, messageType = "text"): Promise<Message> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("send_message", { to, content, messageType }) as Promise<Message>;
  return httpPost<Message>("/messages/send", { to, content, message_type: messageType });
}

export async function markConversationRead(conversationId: string): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("mark_conversation_read", { conversationId }); return; }
  await httpPost(`/conversations/${conversationId}/read`, {});
}

// ---------- 联系人 API ----------

export async function getContacts(relationType?: string): Promise<Contact[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_contacts", { relationType }) as Promise<Contact[]>;
  const params = relationType ? `?relation_type=${relationType}` : "";
  return httpGet<Contact[]>(`/contacts${params}`);
}

export async function sendFriendRequest(starId: string, message?: string): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("send_friend_request", { starId, message }); return; }
  await httpPost("/friends/request", { star_id: starId, message });
}

export async function getFriendRequests(): Promise<FriendRequest[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_friend_requests") as Promise<FriendRequest[]>;
  return httpGet<FriendRequest[]>("/friends/requests");
}

export async function respondFriendRequest(requestId: string, accept: boolean): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("respond_friend_request", { requestId, accept }); return; }
  await httpPost(`/friends/requests/${requestId}/respond`, { action: accept ? "accept" : "reject" });
}
