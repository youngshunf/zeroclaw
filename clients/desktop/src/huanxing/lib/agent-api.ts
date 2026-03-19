/**
 * Agent API client — calls Sidecar `/api/agents` endpoints.
 *
 * Uses `apiFetch` for consistent auth token handling.
 */
import { getHuanxingSession, HUANXING_CONFIG } from '../config';
import { apiFetch } from '@/lib/api';
import { getToken } from '@/lib/auth';

export interface AgentInfo {
  name: string;
  display_name: string | null;
  config_dir: string;
  model: string | null;
  active: boolean;
  is_default: boolean;
  location?: 'local' | 'remote';
}

export interface AgentListResponse {
  agents: AgentInfo[];
  current: string;
}

export interface CreateAgentParams {
  name: string;
  display_name?: string;
  model?: string;
  temperature?: number;
  template?: string;
  soul_md?: string;
  identity_md?: string;
  agents_md?: string;
  user_md?: string;
  tools_md?: string;
}

/** List all agents */
export async function listAgents(): Promise<AgentListResponse> {
  return apiFetch<AgentListResponse>('/api/agents');
}

/** Create a new agent — injects user's LLM token from login session */
export async function createAgent(params: CreateAgentParams): Promise<{ status: string; name: string; config_dir: string }> {
  const session = getHuanxingSession();
  const body: Record<string, unknown> = { ...params };
  if (session?.llmToken) {
    body.api_key = session.llmToken;
    body.base_url = HUANXING_CONFIG.llmGatewayV1;
  }

  return apiFetch<{ status: string; name: string; config_dir: string }>('/api/agents', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/** Delete an agent */
export async function deleteAgent(name: string): Promise<void> {
  await apiFetch(`/api/agents/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  });
}

/** Switch active agent */
export async function switchAgent(name: string): Promise<{ status: string; agent: string; model: string }> {
  return apiFetch<{ status: string; agent: string; model: string }>('/api/agent/switch', {
    method: 'POST',
    body: JSON.stringify({ name }),
  });
}

/** List workspace files for an agent */
export async function listFiles(name: string): Promise<string[]> {
  const data = await apiFetch<{ files: string[] }>(`/api/agents/${encodeURIComponent(name)}/files`);
  return data.files;
}

/** Read a workspace file */
export async function readFile(agentName: string, filename: string): Promise<string> {
  const data = await apiFetch<{ content: string }>(
    `/api/agents/${encodeURIComponent(agentName)}/files/${encodeURIComponent(filename)}`,
  );
  return data.content;
}

/** Write a workspace file */
export async function writeFile(agentName: string, filename: string, content: string): Promise<void> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'text/plain' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await fetch(
    `/api/agents/${encodeURIComponent(agentName)}/files/${encodeURIComponent(filename)}`,
    {
      method: 'PUT',
      headers,
      body: content,
    },
  );
  if (!res.ok) throw new Error(`Failed to write file: ${res.status}`);
}
