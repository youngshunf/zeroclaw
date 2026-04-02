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

export interface EntityRef {
  hasn_id: string;
  owner_id?: string;
  entity_type: "human" | "agent" | "system";
}

export interface MessageContent {
  content_type: string; // "text", "tool_call", "image", etc.
  body: any; // E.g. { text: string } or { tool_name: string, ... }
}

export interface MessageContext {
  conversation_id: string;
  thread_id?: string;
  relation_type?: string;
  scope?: string;
  trade_session_id?: string;
  reply_to?: string;
  capability_id?: string;
}

export interface MessageMetadata {
  priority?: "critical" | "high" | "normal" | "low";
  created_at: string;
  server_received_at?: string;
}

export interface HasnEnvelope {
  id: string;
  version: "1.0";
  from: EntityRef;
  to: EntityRef;
  type: string; // "message", "capability_request", etc.
  content: MessageContent;
  context: MessageContext;
  metadata: MessageMetadata;
  // Legacy fields for backward compatibility during transition
  local_id?: string;
  send_status?: string;
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

// ---------- 环境检测与路径解析 ----------

import { HUANXING_CONFIG, getHuanxingSession } from '../config';

const CLOUD_API_BASE = `${HUANXING_CONFIG.backendBaseUrl}/api/v1/hasn/app/hasn`;
const SIDECAR_API_BASE = `${HUANXING_CONFIG.sidecarBaseUrl}/api/v1/hasn`;

async function cloudGet<T>(path: string): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function cloudPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
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

async function sidecarGet<T>(path: string): Promise<T> {
  const resp = await fetch(`${SIDECAR_API_BASE}${path}`);
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function sidecarPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const resp = await fetch(`${SIDECAR_API_BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

// ---------- 连接管理 (呼叫 Sidecar) ----------

export async function hasnConnect(platformToken: string, hasnId: string, starId: string): Promise<any> {
  localStorage.setItem("hasn:platform_token", platformToken);
  localStorage.setItem("hasn:hasn_id", hasnId);
  return sidecarPost("/connect", { platform_token: platformToken, hasn_id: hasnId, star_id: starId });
}

export async function hasnDisconnect(): Promise<void> {
  await sidecarPost("/disconnect", {});
}

export async function hasnStatus(): Promise<string> {
  try {
    const res = await sidecarGet<any>("/status");
    return res.status;
  } catch {
    return getHuanxingSession()?.accessToken ? "connected" : "disconnected";
  }
}

// ---------- 会话 API (呼叫 Cloud) ----------

export async function getConversations(): Promise<Conversation[]> {
  return cloudGet<Conversation[]>("/conversations");
}

// 适配器：将后端的旧版 Message 转换为 v4.0 HasnEnvelope
function mapLegacyMessageToEnvelope(msg: any): HasnEnvelope {
  return {
    id: msg.id ? String(msg.id) : `msg_${Date.now()}`,
    version: "1.0",
    from: {
      hasn_id: msg.from_id || "",
      entity_type: msg.from_type === 1 ? "human" : "agent"
    },
    to: {
      hasn_id: msg.to_id || "",
      entity_type: "human" // Defaulting to human for legacy
    },
    type: "message",
    content: {
      content_type: msg.content_type === 6 ? "tool_call" : "text",
      body: { text: msg.content || "" }
    },
    context: {
      conversation_id: msg.conversation_id || ""
    },
    metadata: {
      created_at: msg.created_at || new Date().toISOString()
    },
    local_id: msg.local_id,
    send_status: msg.send_status || "delivered"
  };
}

export async function getMessages(
  conversationId: string,
  limit = 50,
  beforeId?: number | string,
): Promise<HasnEnvelope[]> {
  const params = new URLSearchParams({ limit: String(limit) });
  if (beforeId) params.set("before_id", String(beforeId));
  const legacyMessages = await cloudGet<any[]>(`/conversations/${conversationId}/messages?${params}`);
  return legacyMessages.map(mapLegacyMessageToEnvelope);
}

export async function sendMessage(to: string, content: string, replyToId?: number): Promise<HasnEnvelope> {
  // 发送消息通过 Sidecar 代理发出，实现双端一致性
  const hasnId = localStorage.getItem("hasn:hasn_id") || "";
  await sidecarPost("/send", { hasn_id: hasnId, target: to, message: content });
  
  // 乐观构建一个 v4.0 HasnEnvelope 返回给前端
  return {
    id: `temp_${Date.now()}`,
    version: "1.0",
    from: { hasn_id: hasnId, entity_type: "human" },
    to: { hasn_id: to, entity_type: "human" },
    type: "message",
    content: { content_type: "text", body: { text: content } },
    context: { conversation_id: "" }, // Will be filled by WS return
    metadata: { created_at: new Date().toISOString() },
    local_id: `temp_${Date.now()}`,
    send_status: "sent"
  };
}

export async function markConversationRead(conversationId: string, lastMsgId?: number): Promise<void> {
  await cloudPost(`/conversations/${conversationId}/read`, { last_msg_id: lastMsgId ?? 0 });
}

// ---------- 联系人 API (呼叫 Cloud) ----------

export async function getContacts(relationType?: string): Promise<Contact[]> {
  const params = relationType ? `?relation_type=${relationType}` : "";
  return cloudGet<Contact[]>(`/contacts${params}`);
}

export async function sendFriendRequest(starId: string, message?: string): Promise<void> {
  await cloudPost("/contacts/request", { target_star_id: starId, message });
}

export async function getFriendRequests(): Promise<FriendRequest[]> {
  return cloudGet<FriendRequest[]>("/contacts/requests");
}

export async function respondFriendRequest(requestId: number, accept: boolean): Promise<void> {
  await cloudPost(`/contacts/requests/${requestId}/respond`, { action: accept ? "accept" : "reject" });
}

// ---------- Agent API (呼叫 Cloud) ----------

export async function getMyAgents(): Promise<AgentInfo[]> {
  return cloudGet<AgentInfo[]>("/agents");
}

// ---------- Client ID 读取 ----------

/**
 * 读取当前 HASN 客户端 ID
 * 统一 Node 架构下不需要前端绑定 client_id。这仅用于兼容老逻辑。
 */
export async function loadClientId(): Promise<string | undefined> {
  return undefined;
}
