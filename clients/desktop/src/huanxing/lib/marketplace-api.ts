import { invoke } from '@tauri-apps/api/core';

export interface MarketApp {
  id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  pricing_type: string;
  tags?: string[];
  latest_version?: string;
  package_url?: string;
}

export interface MarketSkill {
  id: string;
  name: string;
  version: string;
  description: string;
  category: string;
  pricing_type: string;
  tags?: string[];
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
