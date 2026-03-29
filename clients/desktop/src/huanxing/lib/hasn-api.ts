/**
 * HASN API 双模适配层
 *
 * Tauri 桌面端：通过 invoke() 调用 Rust 后端
 * Web 浏览器：通过 fetch() 调用 HTTP API
 */

// ---------- 类型定义（对齐 Tauri hasn.rs 响应类型）----------

export interface Conversation {
  id: string;
  peer_id: string;
  peer_name: string;
  peer_type: string;
  last_message?: string;
  last_message_at?: string;
  unread_count: number;
}

export interface Message {
  id: number;
  local_id: string;
  conversation_id: string;
  from_id: string;
  from_type: number;
  content: string;
  content_type: number;
  status: number;
  send_status: string;
  created_at?: string;
  reply_to_id?: number;
}

export interface Contact {
  hasn_id: string;
  star_id: string;
  name: string;
  peer_type: string;
  relation_type: string;
  trust_level: number;
  status: string;
}

export interface FriendRequest {
  id: number;
  from_hasn_id: string;
  from_star_id: string;
  from_name: string;
  message?: string;
  status: string;
  created_at?: string;
}

export interface AgentInfo {
  hasn_id: string;
  star_id: string;
  name: string;
  agent_name: string;
  type: string;
  server_id?: string;
  online: boolean;
  created_via: string;
  created_time?: string;
}

// ---------- 环境检测 ----------

function getTauriInvoke(): ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null {
  const internals = (window as any).__TAURI_INTERNALS__;
  return internals?.invoke ?? null;
}

const HASN_API_BASE = "/api/v1/hasn_core/app/hasn";

async function httpGet<T>(path: string): Promise<T> {
  const token = localStorage.getItem("hasn:platform_token");
  const resp = await fetch(`${HASN_API_BASE}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function httpPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const token = localStorage.getItem("hasn:platform_token");
  const resp = await fetch(`${HASN_API_BASE}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

// ---------- 连接管理 ----------

export async function hasnConnect(platformToken: string, hasnId: string, starId: string): Promise<any> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("hasn_connect", { platformToken, hasnId, starId });
  // Web 模式：存储 token 供后续 API 使用
  localStorage.setItem("hasn:platform_token", platformToken);
  localStorage.setItem("hasn:hasn_id", hasnId);
  return { connected: true, hasn_id: hasnId };
}

export async function hasnDisconnect(): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("hasn_disconnect"); return; }
  localStorage.removeItem("hasn:platform_token");
}

export async function hasnStatus(): Promise<string> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("hasn_status") as Promise<string>;
  return localStorage.getItem("hasn:platform_token") ? "connected" : "disconnected";
}

// ---------- 会话 API ----------

export async function getConversations(): Promise<Conversation[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_conversations") as Promise<Conversation[]>;
  return httpGet<Conversation[]>("/conversations");
}

export async function getMessages(
  conversationId: string,
  limit = 50,
  beforeId?: number,
): Promise<Message[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_messages", { conversationId, beforeId, limit }) as Promise<Message[]>;
  const params = new URLSearchParams({ limit: String(limit) });
  if (beforeId) params.set("before_id", String(beforeId));
  return httpGet<Message[]>(`/conversations/${conversationId}/messages?${params}`);
}

export async function sendMessage(to: string, content: string, replyToId?: number): Promise<Message> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("send_message", { to, content, replyToId: replyToId ?? null }) as Promise<Message>;
  return httpPost<Message>("/messages/send", { to, content: { text: content }, content_type: 1, reply_to_id: replyToId });
}

export async function markConversationRead(conversationId: string, lastMsgId?: number): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("mark_conversation_read", { conversationId, lastMsgId }); return; }
  await httpPost(`/conversations/${conversationId}/read`, { last_msg_id: lastMsgId ?? 0 });
}

// ---------- 联系人 API ----------

export async function getContacts(relationType?: string): Promise<Contact[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_contacts", { relationType }) as Promise<Contact[]>;
  const params = relationType ? `?relation_type=${relationType}` : "";
  return httpGet<Contact[]>(`/social/contacts${params}`);
}

export async function sendFriendRequest(starId: string, message?: string): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("send_friend_request", { starId, message }); return; }
  await httpPost("/social/contacts/request", { target_star_id: starId, message });
}

export async function getFriendRequests(): Promise<FriendRequest[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_friend_requests") as Promise<FriendRequest[]>;
  return httpGet<FriendRequest[]>("/social/contacts/requests");
}

export async function respondFriendRequest(requestId: number, accept: boolean): Promise<void> {
  const invoke = getTauriInvoke();
  if (invoke) { await invoke("respond_friend_request", { requestId, accept }); return; }
  await httpPost(`/social/contacts/requests/${requestId}/respond`, { action: accept ? "accept" : "reject" });
}

// ---------- Agent API ----------

export async function getMyAgents(): Promise<AgentInfo[]> {
  const invoke = getTauriInvoke();
  if (invoke) return invoke("get_my_agents") as Promise<AgentInfo[]>;
  return httpGet<AgentInfo[]>("/me/agents");
}

// ---------- Client ID 读取 ----------

/**
 * 读取当前 HASN 客户端 ID（从 ~/.huanxing/hasn/client.json）
 * 用于 Agent 注册时绑定 client_id
 */
export async function loadClientId(): Promise<string | undefined> {
  const invoke = getTauriInvoke();
  if (!invoke) return undefined;
  try {
    // 调用 Rust 侧读取 client.json
    const result = await invoke("hasn_get_client_id") as string | null;
    return result ?? undefined;
  } catch {
    return undefined;
  }
}
