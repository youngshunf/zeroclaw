import { invoke } from '@tauri-apps/api/core';

export interface CronJob {
  id: string;
  expression: string;
  name: string | null;
  prompt: string | null;
  enabled: boolean;
  next_run: string;
  last_run: string | null;
  last_status: string | null;
  last_output: string | null;
}

export interface CronRun {
  id: number;
  job_id: string;
  started_at: string;
  finished_at: string;
  status: string;
  output: string | null;
  duration_ms: number | null;
}

export async function listCronJobs(): Promise<CronJob[]> {
  try {
    return await invoke<CronJob[]>('list_cron_jobs');
  } catch (error) {
    console.error('Failed to list cron jobs:', error);
    return [];
  }
}

export async function addCronJob(
  expression: string,
  prompt: string,
  name?: string
): Promise<string> {
  return await invoke<string>('add_cron_job', {
    expression,
    prompt,
    name: name || null,
  });
}

export async function toggleCronJob(id: string, enabled: boolean): Promise<void> {
  await invoke('toggle_cron_job', { id, enabled });
}

export async function deleteCronJob(id: string): Promise<void> {
  await invoke('delete_cron_job', { id });
}

export async function getCronRuns(jobId: string): Promise<CronRun[]> {
  try {
    return await invoke<CronRun[]>('get_cron_runs', { jobId });
  } catch (error) {
    console.error('Failed to get cron runs:', error);
    return [];
  }
}
