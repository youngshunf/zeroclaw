import { invoke } from '@tauri-apps/api/core';

export interface MarketApp {
  id: string;
  app_id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  pricing_type: string;
  tags?: string;
  emoji?: string;
  icon_url?: string;
  latest_version?: string;
  package_url?: string;
  skill_dependencies?: string;
  sop_dependencies?: string;
}

export interface MarketSkill {
  id: string;
  skill_id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  pricing_type: string;
  tags?: string;
  emoji?: string;
  icon_url?: string;
  latest_version?: string;
  package_url?: string;
}

export interface MarketSop {
  id: string;
  sop_id: string;
  name: string;
  description: string;
  category: string;
  pricing_type: string;
  tags?: string;
  emoji?: string;
  icon_url?: string;
  execution_mode?: string;
  skill_dependencies?: string;
  latest_version?: string;
  package_url?: string;
}

export interface MarketResponse<T> {
  items: T[];
  total: number;
}

export async function getMarketApps(): Promise<MarketResponse<MarketApp>> {
  return invoke('get_market_apps');
}

export async function getMarketSkills(): Promise<MarketResponse<MarketSkill>> {
  return invoke('get_market_skills');
}

export async function getMarketSops(): Promise<MarketResponse<MarketSop>> {
  return invoke('get_market_sops');
}

export async function installMarketAgent(
  appId: string, 
  agentName: string, 
  displayName: string, 
  packageUrl: string
): Promise<void> {
  await invoke('download_and_install_agent', {
    appId,
    agentName,
    displayName,
    packageUrl,
  });
}

export async function installMarketSkill(
  agentName: string, 
  skillId: string, 
  packageUrl: string
): Promise<void> {
  await invoke('download_and_install_skill', {
    agentName,
    skillId,
    packageUrl,
  });
}

export async function installMarketSop(
  agentName: string,
  sopId: string,
  packageUrl: string
): Promise<void> {
  await invoke('download_and_install_sop', {
    agentName,
    sopId,
    packageUrl,
  });
}
