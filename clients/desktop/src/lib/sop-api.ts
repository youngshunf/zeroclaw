import { apiFetch } from '@/lib/api';

export interface SopRequirementsInfo {
  skills: string[];
  optional_skills: string[];
}

export interface SopStepInfo {
  number: number;
  title: string;
  requires_confirmation: boolean;
  suggested_tools: string[];
}

export interface SopInfo {
  name: string;
  display_name: string | null;
  description: string;
  version: string;
  priority: string;
  execution_mode: string;
  max_concurrent: number;
  active_runs: number;
  requirements: SopRequirementsInfo | null;
}

export interface SopDetailResponse extends SopInfo {
  steps: SopStepInfo[];
  triggers: string[];
}

export interface SopListResponse {
  sops: SopInfo[];
}

/** Lists all available SOPs for the specified agent. */
export async function listSops(agentName: string): Promise<SopListResponse> {
  return apiFetch<SopListResponse>(`/api/sop/list?agent=${encodeURIComponent(agentName)}`);
}

/** Gets full details including steps for a specific SOP. */
export async function getSopDetail(agentName: string, sopName: string): Promise<SopDetailResponse> {
  return apiFetch<SopDetailResponse>(
    `/api/sop/${encodeURIComponent(sopName)}/detail?agent=${encodeURIComponent(agentName)}`
  );
}

export interface ExecuteSopResponse {
  session_id: string;
  run_id: string;
  title: string;
}

export async function executeSop(agentName: string, sopName: string, payload?: string): Promise<ExecuteSopResponse> {
  return apiFetch<ExecuteSopResponse>(
    `/api/sop/${encodeURIComponent(sopName)}/execute?agent=${encodeURIComponent(agentName)}`,
    {
      method: 'POST',
      body: JSON.stringify({ payload: payload || null }),
    }
  );
}

export interface SopRun {
  run_id: string;
  sop_name: string;
  status: string;
  current_step: number;
  total_steps: number;
  started_at: string;
  completed_at: string | null;
  llm_calls_saved: number;
}

export interface RunsListResponse {
  runs: SopRun[];
}

export async function listRuns(agentName: string, status?: string): Promise<RunsListResponse> {
  const params = new URLSearchParams([['agent', agentName]]);
  if (status) params.append('status', status);
  return apiFetch<RunsListResponse>(`/api/sop/runs?${params.toString()}`);
}
