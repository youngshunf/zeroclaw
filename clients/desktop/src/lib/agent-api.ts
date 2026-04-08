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
  hasn_id?: string | null;
  config_dir: string;
  model: string | null;
  active: boolean;
  is_default: boolean;
  location?: 'local' | 'remote';
  icon_url?: string;
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
  const data = await apiFetch<AgentListResponse>('/api/agents');
  data.agents = data.agents.map(a => {
    let url = a.icon_url;
    if (url && !url.includes('raw=')) {
      url = url.includes('?') ? `${url}&raw=true` : `${url}?raw=true`;
    }
    return {
      ...a,
      icon_url: url && !url.startsWith('http') ? `${HUANXING_CONFIG.sidecarBaseUrl}${url}` : url
    };
  });
  return data;
}

/** Create a new agent — injects user's LLM token from login session + registers HASN identity */
export async function createAgent(params: CreateAgentParams): Promise<{ status: string; name: string; config_dir: string }> {
  const session = getHuanxingSession();
  const body: Record<string, unknown> = { ...params };
  if (session?.llmToken) {
    body.api_key = session.llmToken;
    body.base_url = HUANXING_CONFIG.llmGatewayV1;
  }

  // 1. 在 Sidecar 创建工作区
  const result = await apiFetch<{ status: string; name: string; config_dir: string }>('/api/agents', {
    method: 'POST',
    body: JSON.stringify(body),
  });

  // 注意：不再此处手动调用 API 注册 HASN。
  // App.tsx 在检测到 Agent 目录生成并在 `listAgents()` 返回空的 hasn_id 时，会自动读取 agent workspace 配置里的 display_name 注册并写回。

  return result;
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
