/**
 * HASN API 双模适配层
 *
 * Tauri 桌面端：通过 invoke() 调用 Rust 后端
 * Web 浏览器：通过 fetch() 调用 HTTP API
 */

// ══════════════════════════════════════════════
// 类型定义 — 阶段二对齐 (0-5 六级信任 / 三维权限矩阵)
// ══════════════════════════════════════════════

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
  entity_type: 'human' | 'agent' | 'system';
}

export interface MessageContent {
  content_type: string; // "text", "tool_call", "image", etc.
  body: any;
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
  priority?: 'critical' | 'high' | 'normal' | 'low';
  created_at: string;
  server_received_at?: string;
}

export interface HasnEnvelope {
  id: string;
  version: '1.0';
  from: EntityRef;
  to: EntityRef;
  type: string;
  content: MessageContent;
  context: MessageContext;
  metadata: MessageMetadata;
  local_id?: string;
  send_status?: string;
}

// ── 工具函数 ────────────────────────────────
export function generateUUID(): string {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}

// ── 信任等级常量 ────────────────────────────────
export const TRUST_LEVELS = {
  BLOCKED:  0,
  STRANGER: 1,
  NORMAL:   2,
  FRIEND:   3,
  TRUSTED:  4,
  OWNER:    5,
} as const;

export type TrustLevel = typeof TRUST_LEVELS[keyof typeof TRUST_LEVELS];

export const TRUST_LEVEL_LABELS: Record<number, string> = {
  0: '已拉黑',
  1: '陌生人',
  2: '普通联系人',
  3: '朋友',
  4: '密友',
  5: '所有者',
};

export const TRUST_LEVEL_COLORS: Record<number, string> = {
  0: 'red',
  1: 'gray',
  2: 'blue',
  3: 'green',
  4: 'orange',
  5: 'purple',
};

// ── 关系类型 ────────────────────────────────────
export type RelationType = 'social' | 'commerce' | 'service' | 'professional' | 'platform';

// ── 四态权限编码 ────────────────────────────────
export type PermissionState = 'allow' | 'deny' | 'confirm_required' | 'scope_limited';

// ── 联系人名下 Agent 摘要 ───────────────────────
export interface AgentPeer {
  hasn_id: string;
  star_id: string;
  name: string;
  agent_name: string;
  avatar_url?: string;
  type: string;
  role: string;
}

// ── 联系人 ─────────────────────────────────────
/** 旧版简单 Contact，向后兼容保留 */
export interface Contact {
  hasn_id: string;
  star_id: string;
  name: string;
  peer_type: string;
  relation_type: string;
  trust_level: number;
  status: string;
}

/** 阶段二完整联系人（/contacts 接口实际返回体） */
export interface ContactFull {
  id: number;
  peer: {
    hasn_id: string;
    star_id: string;
    name: string;
    type: string;       // 'human' | 'agent'
    avatar_url?: string;
    status?: string;
  };
  relation_type: RelationType;
  trust_level: TrustLevel;
  trust_level_label: string;
  nickname?: string;
  tags?: string[];
  subscription: boolean;
  status: string;
  owned_agents: AgentPeer[];          // human 联系人名下 Agent 列表
  custom_permissions: Record<string, PermissionState>;
  scope?: Record<string, any>;
  connected_at?: string;
  last_interaction_at?: string;
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

// ── Agent 信息 ─────────────────────────────────
/** 阶段三增强版 AgentInfo（/me/agents 返回体） */
export interface AgentInfo {
  hasn_id: string;
  star_id: string;
  name: string;
  agent_name: string;
  type: string;
  node_id?: string;
  avatar_url?: string;
  role: string;
  description?: string;
  capabilities?: any[];
  online: boolean;
  created_via: string;
  created_time?: string;
}

// ── 节点 / API Key ────────────────────────────
export interface HasnNodeInfo {
  node_id: string;
  user_id?: number | null;
  allowed_owner_hasn_ids?: string[] | null;
  node_type: string;
  node_name?: string | null;
  device_fingerprint?: string | null;
  device_platform?: string | null;
  app_version?: string | null;
  node_info: Record<string, any>;
  capacity?: number;
  last_seen_at?: string | null;
  created_time?: string | null;
}

export interface OwnerApiKeyInfo {
  key_id: string;
  key_name?: string | null;
  owner_id: string;
  status: string;
  scopes?: Record<string, any> | null;
  bound_node_id?: string | null;
  expires_at?: string | null;
  created_time?: string | null;
  last_seen_at?: string | null;
}

export interface CreateOwnerApiKeyPayload {
  name: string;
  scopes?: Record<string, any>;
  bound_node_id?: string | null;
  expires_at?: string | null;
}

export interface CreateOwnerApiKeyResult extends OwnerApiKeyInfo {
  owner_api_key: string;
}

// ── 权限矩阵查询结果 ────────────────────────────
export interface EffectivePermissions {
  contact_id: number;
  relation_type: RelationType;
  trust_level: TrustLevel;
  trust_level_label: string;
  effective_permissions: Record<string, PermissionState>;
}

// ══════════════════════════════════════════════
// 环境检测与路径解析
// ══════════════════════════════════════════════

import { HUANXING_CONFIG, getHuanxingSession } from '../config';
import { hasnWs } from './hasn-ws';

const isDesktop = typeof window !== 'undefined' && (!!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__);
const CLOUD_API_BASE = `${import.meta.env.DEV ? '' : (isDesktop ? HUANXING_CONFIG.backendBaseUrl : '')}/api/v1/hasn/app`;
const SIDECAR_API_BASE = import.meta.env.DEV
  ? `/api/v1/hasn`
  : `${HUANXING_CONFIG.sidecarBaseUrl}/api/v1/hasn`;

// ── 通用请求工具 ────────────────────────────────

async function cloudGet<T>(path: string): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  let data = json.data ?? json;

  // Extract paginated arrays automatically
  if (data && typeof data === 'object' && !Array.isArray(data)) {
    if (Array.isArray(data.items)) data = data.items;
    else if (Array.isArray(data.list)) data = data.list;
    else if (Array.isArray(data.records)) data = data.records;
    else if (Array.isArray(data.contacts)) data = data.contacts;
    else if (Array.isArray(data.agents)) data = data.agents;
    else if (Array.isArray(data.requests)) data = data.requests;
  }

  return data;
}

async function cloudPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function cloudPut<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function cloudDelete<T>(path: string): Promise<T> {
  const token = getHuanxingSession()?.accessToken;
  const resp = await fetch(`${CLOUD_API_BASE}${path}`, {
    method: 'DELETE',
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

async function sidecarGet<T>(path: string): Promise<T> {
  const url = `${SIDECAR_API_BASE}${path}`;
  try {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
    
    // Check Content-Type to avoid parse errors on HTML or empty responses
    const cType = resp.headers.get('content-type') || '';
    const text = await resp.text();
    if (!text.trim()) return {} as T;
    
    try {
      const json = JSON.parse(text);
      return json.data ?? json;
    } catch (err: any) {
      console.error(`[sidecarGet] JSON parse failed for ${url}. Body:`, text);
      throw err;
    }
  } catch (err: any) {
    console.error(`[sidecarGet] Network/fetch failed for ${url}:`, err);
    throw err;
  }
}

async function sidecarPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const url = `${SIDECAR_API_BASE}${path}`;
  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
    const text = await resp.text();
    if (!text.trim()) return {} as T;
    
    const json = JSON.parse(text);
    return json.data ?? json;
  } catch (err: any) {
    console.error(`[sidecarPost] failed for ${url}:`, err);
    throw err;
  }
}

// ══════════════════════════════════════════════
// 连接管理 (Sidecar)
// ══════════════════════════════════════════════

export async function hasnConnect(nodeKey: string, hasnId: string, starId: string): Promise<any> {
  localStorage.setItem('hasn:hasn_id', hasnId);
  localStorage.setItem('hasn:star_id', starId);
  const result = await sidecarPost('/connect', { token: nodeKey });
  hasnWs.emitConnected();
  return result;
}

export async function hasnDisconnect(): Promise<void> {
  await sidecarPost('/disconnect', {});
  hasnWs.emitDisconnected();
}

export async function hasnStatus(): Promise<string> {
  try {
    const res = await sidecarGet<any>('/status');
    if (res.connected === true) return 'connected';
    if (res.status) return res.status;
    return 'disconnected';
  } catch {
    return 'disconnected';
  }
}

export async function hasnAddOwner(ownerId: string, bearerToken: string): Promise<any> {
  return sidecarPost('/node/owners', {
    owner_id: ownerId,
    owner_proof: { type: 'bearer_token', credential: bearerToken },
  });
}

export async function hasnRenewOwner(ownerId: string, bearerToken: string): Promise<any> {
  const connStatus = await hasnStatus();
  if (connStatus !== 'connected') return;
  return sidecarPost(`/node/owners/${encodeURIComponent(ownerId)}/renew`, {
    type: 'bearer_token',
    credential: bearerToken,
  });
}

export async function hasnRemoveOwner(ownerId: string): Promise<any> {
  const resp = await fetch(`${SIDECAR_API_BASE}/node/owners/${encodeURIComponent(ownerId)}`, {
    method: 'DELETE',
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

export async function hasnAddAgent(agentId: string, ownerId: string): Promise<any> {
  return sidecarPost('/node/agents', { agent_id: agentId, owner_id: ownerId });
}

export async function hasnRemoveAgent(agentId: string): Promise<any> {
  const resp = await fetch(`${SIDECAR_API_BASE}/node/agents/${encodeURIComponent(agentId)}`, {
    method: 'DELETE',
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}: ${resp.statusText}`);
  const json = await resp.json();
  return json.data ?? json;
}

// ══════════════════════════════════════════════
// 会话 API (Cloud)
// ══════════════════════════════════════════════

export async function getConversations(): Promise<Conversation[]> {
  const hasnId = (localStorage.getItem('hasn:hasn_id') || '').trim();
  const result = await sidecarGet<{ sessions: any[] }>(`/chat/sessions?hasn_id=${encodeURIComponent(hasnId)}`);
  return result.sessions.map(s => ({
    id: s.conversation_id,
    peer_id: s.peer_id,
    peer_name: s.title || s.peer_id,
    peer_type: s.session_type === 'group' ? 'group' : (s.peer_id?.startsWith('a_') ? 'agent' : 'human'),
    last_message: s.last_message_preview || undefined,
    last_message_at: s.updated_at,
    unread_count: s.unread_count || 0
  }));
}

function mapLegacyMessageToEnvelope(msg: any): HasnEnvelope {
  return {
    id: msg.id ? String(msg.id) : `msg_${Date.now()}`,
    version: '1.0',
    from: {
      hasn_id: msg.from_id || '',
      entity_type: msg.from_type === 1 ? 'human' : 'agent',
    },
    to: {
      hasn_id: msg.to_id || '',
      entity_type: 'human',
    },
    type: 'message',
    content: {
      content_type: msg.content_type === 6 ? 'tool_call' : 'text',
      body: { text: msg.content || '' },
    },
    context: { conversation_id: msg.conversation_id || '' },
    metadata: { created_at: msg.created_at || new Date().toISOString() },
    local_id: msg.local_id,
    send_status: msg.send_status || 'delivered',
  };
}

export async function getMessages(
  conversationId: string,
  limit = 50,
  beforeId?: number | string,
): Promise<HasnEnvelope[]> {
  const hasnId = (localStorage.getItem('hasn:hasn_id') || '').trim();
  const params = new URLSearchParams({ 
    hasn_id: hasnId,
    conversation_id: conversationId,
    limit: String(limit) 
  });
  
  try {
    const raw = await sidecarGet<{ messages: any[] }>(`/chat/messages?${params}`);
    return raw.messages.map((msg: any) => {
      let bodyContent = { text: '' };
      try {
         bodyContent = JSON.parse(msg.content);
      } catch (e) {
         bodyContent.text = msg.content;
      }
      return {
        id: msg.message_id,
        version: '1.0',
        from: { hasn_id: msg.sender_id, entity_type: msg.sender_id?.startsWith('a_') ? 'agent' : 'human' },
        to: { hasn_id: msg.receiver_id, entity_type: msg.receiver_id?.startsWith('a_') ? 'agent' : 'human' },
        type: 'message',
        content: { content_type: msg.content_type || 'text', body: bodyContent },
        context: { conversation_id: msg.conversation_id },
        metadata: { created_at: msg.created_at },
        send_status: msg.status,
      } as HasnEnvelope;
    });
  } catch (err: any) {
    if (err?.message?.includes('404')) return [];
    throw err;
  }
}

export async function sendMessage(to: string, content: string, replyToId?: number): Promise<HasnEnvelope> {
  const status = await hasnStatus();
  if (status !== 'connected') throw new Error('HASN 未连接，无法发送消息');

  const hasnId = localStorage.getItem('hasn:hasn_id') || '';
  const localId = generateUUID();
  await sidecarPost('/send', {
    from_id: hasnId,
    to,
    content: { text: content },
    local_id: localId,
  });

  return {
    id: localId,
    version: '1.0',
    from: { hasn_id: hasnId, entity_type: 'human' },
    to: { hasn_id: to, entity_type: 'human' },
    type: 'message',
    content: { content_type: 'text', body: { text: content } },
    context: { conversation_id: '' },
    metadata: { created_at: new Date().toISOString() },
    local_id: localId,
    send_status: 'sent',
  };
}

export async function markConversationRead(conversationId: string, lastMsgId?: number): Promise<void> {
  const hasnId = (localStorage.getItem('hasn:hasn_id') || '').trim();
  await sidecarPost(`/chat/read?hasn_id=${encodeURIComponent(hasnId)}&conversation_id=${encodeURIComponent(conversationId)}`, {});
}

// ══════════════════════════════════════════════
// 联系人 API (Cloud) — 阶段二增强
// ══════════════════════════════════════════════

/** 获取联系人列表（完整 ContactFull 格式，从本地数据库返回同步的记录） */
export async function getContacts(relationType?: string): Promise<ContactFull[]> {
  const hasnId = (localStorage.getItem('hasn:hasn_id') || '').trim();
  const raw = await sidecarGet<{ contacts: any[] }>(`/chat/contacts?hasn_id=${encodeURIComponent(hasnId)}`);
  
  let contacts = raw.contacts.map((c: any) => ({
    id: 0,
    peer: {
      hasn_id: c.hasn_id,
      star_id: '',
      name: c.nickname || c.hasn_id,
      type: c.contact_type || 'human',
      avatar_url: c.avatar_url,
      status: c.status
    },
    relation_type: c.relation_type,
    trust_level: c.trust_level,
    trust_level_label: TRUST_LEVEL_LABELS[c.trust_level as TrustLevel] || '未知',
    subscription: false,
    status: c.status,
    owned_agents: [],
    custom_permissions: {},
    connected_at: c.created_at,
  } as ContactFull));

  if (relationType) {
    contacts = contacts.filter((c: ContactFull) => c.relation_type === relationType);
  }
  
  return contacts;
}

export async function hasnChatSyncStatus(): Promise<any> {
  const hasnId = (localStorage.getItem('hasn:hasn_id') || '').trim();
  return sidecarGet(`/chat/sync/status?hasn_id=${encodeURIComponent(hasnId)}`);
}

export async function sendFriendRequest(starId: string, message?: string): Promise<void> {
  await cloudPost('/contacts/request', { target_star_id: starId, message });
}

export async function getFriendRequests(): Promise<FriendRequest[]> {
  return cloudGet<FriendRequest[]>('/contacts/requests');
}

export async function respondFriendRequest(requestId: number, accept: boolean): Promise<void> {
  await cloudPost(`/contacts/requests/${requestId}/respond`, { action: accept ? 'accept' : 'reject' });
}

/** 修改联系人信任等级 */
export async function updateTrustLevel(
  contactId: number,
  trustLevel: TrustLevel,
  relationType: RelationType = 'social',
): Promise<void> {
  await cloudPut(`/contacts/${contactId}/trust-level`, {
    trust_level: trustLevel,
    relation_type: relationType,
  });
}

/** 覆盖联系人权限（铁律校验由后端执行） */
export async function updateContactPermissions(
  contactId: number,
  permissions: Record<string, PermissionState>,
): Promise<void> {
  await cloudPut(`/contacts/${contactId}/permissions`, { permissions });
}

/** 查询联系人的合并有效权限 */
export async function getEffectivePermissions(contactId: number): Promise<EffectivePermissions> {
  return cloudGet<EffectivePermissions>(`/contacts/${contactId}/effective-permissions`);
}

// ══════════════════════════════════════════════
// Agent API (Cloud) — 阶段三增强
// ══════════════════════════════════════════════

/** 获取我的 Agent 列表（含 avatar_url / role / description / capabilities） */
export async function getMyAgents(): Promise<AgentInfo[]> {
  return cloudGet<AgentInfo[]>('/me/agents');
}

export async function getMyNodes(): Promise<HasnNodeInfo[]> {
  return cloudGet<HasnNodeInfo[]>('/me/nodes');
}

export async function reissueMyNodeKey(nodeId: string): Promise<{ node_id: string; node_key: string }> {
  return cloudPost<{ node_id: string; node_key: string }>(`/me/nodes/${encodeURIComponent(nodeId)}/reissue-key`, {});
}

export async function getOwnerApiKeys(): Promise<OwnerApiKeyInfo[]> {
  return cloudGet<OwnerApiKeyInfo[]>('/api-keys');
}

export async function createOwnerApiKey(payload: CreateOwnerApiKeyPayload): Promise<CreateOwnerApiKeyResult> {
  return cloudPost<CreateOwnerApiKeyResult>('/api-keys', {
    name: payload.name,
    scopes: payload.scopes,
    bound_node_id: payload.bound_node_id,
    expires_at: payload.expires_at,
  });
}

export async function deleteOwnerApiKey(keyId: string): Promise<void> {
  await cloudDelete(`/api-keys/${encodeURIComponent(keyId)}`);
}
