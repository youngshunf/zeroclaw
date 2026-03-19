/**
 * session-api.ts — REST API client for session CRUD
 */

import { apiFetch } from './api';

export interface SessionInfo {
  id: string;
  title: string;
  agent_id: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

export interface SessionMessage {
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
}

export interface SessionDetail extends SessionInfo {
  messages: SessionMessage[];
}

/** List all sessions, optionally filtered by agent_id */
export async function listSessions(agentId?: string): Promise<SessionInfo[]> {
  const params = agentId ? `?agent_id=${encodeURIComponent(agentId)}` : '';
  const data = await apiFetch<{ sessions: SessionInfo[] }>(`/api/sessions${params}`);
  return data.sessions;
}

/** Create a new session */
export async function createSession(
  title?: string,
  agentId?: string,
): Promise<{ session_id: string; title: string; agent_id: string }> {
  return apiFetch<{ session_id: string; title: string; agent_id: string }>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify({ title, agent_id: agentId }),
  });
}

/** Get session detail with message history */
export async function getSession(sessionId: string): Promise<SessionDetail> {
  return apiFetch<SessionDetail>(`/api/sessions/${encodeURIComponent(sessionId)}`);
}

/** Paginated message for history loading */
export interface PaginatedMessage {
  id: number;
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
}

export interface PaginatedSessionDetail {
  id: string;
  title: string;
  agent_id: string;
  created_at: string;
  updated_at: string;
  messages: PaginatedMessage[];
  has_more: boolean;
  oldest_id: number | null;
  total_count: number;
}

/** Get session messages with pagination (newest first, paged by `before` cursor) */
export async function getSessionMessages(
  sessionId: string,
  options?: { limit?: number; before?: number; agentId?: string },
): Promise<PaginatedSessionDetail> {
  const params = new URLSearchParams();
  if (options?.limit) params.set('limit', String(options.limit));
  if (options?.before) params.set('before', String(options.before));
  if (options?.agentId) params.set('agent_id', options.agentId);
  const qs = params.toString();
  return apiFetch<PaginatedSessionDetail>(
    `/api/sessions/${encodeURIComponent(sessionId)}${qs ? `?${qs}` : ''}`,
  );
}

/** Auto-generate session title via LLM */
export async function generateSessionTitle(
  sessionId: string,
): Promise<{ title: string }> {
  return apiFetch<{ title: string }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/generate-title`,
    { method: 'POST' },
  );
}

/** Rename a session */
export async function renameSession(sessionId: string, title: string): Promise<void> {
  await apiFetch(`/api/sessions/${encodeURIComponent(sessionId)}`, {
    method: 'PUT',
    body: JSON.stringify({ title }),
  });
}

/** Delete a session */
export async function deleteSession(sessionId: string): Promise<void> {
  await apiFetch(`/api/sessions/${encodeURIComponent(sessionId)}`, {
    method: 'DELETE',
  });
}

/** Clear all messages in a session */
export async function clearSession(sessionId: string): Promise<void> {
  await apiFetch(`/api/sessions/${encodeURIComponent(sessionId)}/messages`, {
    method: 'DELETE',
  });
}
