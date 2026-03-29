import React, { useState, useEffect } from 'react';
import { Bot, Wrench, Search, Download, Loader2, RefreshCw } from 'lucide-react';
import {
  getMarketApps,
  getMarketSkills,
  installMarketAgent,
  installMarketSkill,
  type MarketApp,
  type MarketSkill,
} from '../lib/marketplace-api';
import { listAgents, type AgentInfo } from '../lib/agent-api';

function AgentPlaza() {
  const [apps, setApps] = useState<MarketApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    getMarketApps()
      .then((res) => setApps(res.items || []))
      .catch((err) => console.error('Failed to get market apps', err))
      .finally(() => setLoading(false));
  }, []);

  const handleInstall = async (app: MarketApp) => {
    const defaultName = `${app.id}-${Math.floor(Math.random() * 1000)}`;
    const displayName = prompt('为新 Agent 起个名字:', app.name);
    if (!displayName) return;
    
    // 我们假设云端 /apps 返回的数据中包含了 latest_version 和对应元数据
    // 要么我们请求一次云端，要么先用占位
    const pkgUrl = app.package_url || `http://127.0.0.1:8000/api/v1/marketplace/client/download/app/${app.id}/latest`;
    
    setInstalling(app.id);
    try {
      await installMarketAgent(app.id, defaultName, displayName, pkgUrl);
      alert('Agent 安装成功！你可以去左侧 Agent 管理查看。');
    } catch (err: any) {
      alert(`安装失败: ${err.message || err}`);
    } finally {
      setInstalling(null);
    }
  };

  if (loading) return <div className="p-8 text-gray-400">Loading...</div>;
  if (!apps.length) return <div className="p-8 text-gray-400">广场空空如也。</div>;

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {apps.map((app) => (
        <div key={app.id} className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-semibold text-gray-900">{app.name}</h3>
            <span className="text-xs text-gray-500 bg-gray-100 px-2 py-1 rounded-full">{app.category || 'App'}</span>
          </div>
          <p className="text-sm text-gray-500 mb-4 line-clamp-2 h-10">{app.description}</p>
          <div className="flex justify-between items-center mt-auto">
            <span className="text-xs text-gray-400">版本: {app.latest_version || app.version}</span>
            <button
              onClick={() => handleInstall(app)}
              disabled={!!installing}
              className="px-3 py-1.5 bg-[#7c3aed] text-white text-xs font-medium rounded-lg hover:bg-[#6d28d9] disabled:opacity-50 flex items-center gap-1"
            >
              {installing === app.id ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
              下载安装
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

function SkillMarket() {
  const [skills, setSkills] = useState<MarketSkill[]>([]);
  const [localAgents, setLocalAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getMarketSkills(), listAgents()])
      .then(([skillRes, agentsRes]) => {
        setSkills(skillRes.items || []);
        setLocalAgents(agentsRes.agents || []);
        if (agentsRes.agents?.length > 0) {
          setSelectedAgent(agentsRes.agents[0].name);
        }
      })
      .catch((err) => console.error('Failed to init skills', err))
      .finally(() => setLoading(false));
  }, []);

  const handleInstall = async (skill: MarketSkill) => {
    if (!selectedAgent) {
      alert('请先选择一个本地 Agent');
      return;
    }
    const pkgUrl = skill.package_url || `http://127.0.0.1:8000/api/v1/marketplace/client/download/skill/${skill.id}/latest`;
    
    setInstalling(skill.id);
    try {
      await installMarketSkill(selectedAgent, skill.id, pkgUrl);
      alert('技能安装成功！');
    } catch (err: any) {
      alert(`安装失败: ${err.message || err}`);
    } finally {
      setInstalling(null);
    }
  };

  if (loading) return <div className="p-8 text-gray-400">Loading...</div>;
  if (!skills.length) return <div className="p-8 text-gray-400">技能市场尚未上架内容。</div>;

  return (
    <div className="space-y-6">
      <div className="bg-white p-4 rounded-xl border border-gray-200">
        <label className="block text-sm font-medium text-gray-700 mb-2">选择目标 Agent (以安装技能):</label>
        <select
          value={selectedAgent}
          onChange={(e) => setSelectedAgent(e.target.value)}
          className="w-full sm:w-64 px-3 py-2 bg-gray-50 border border-gray-300 text-gray-900 rounded-lg focus:ring-[#7c3aed] focus:border-[#7c3aed]"
        >
          {localAgents.map(a => (
            <option key={a.name} value={a.name}>{a.display_name || a.name}</option>
          ))}
        </select>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {skills.map((skill) => (
          <div key={skill.id} className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm flex flex-col">
            <div className="flex items-center justify-between mb-2">
              <h3 className="font-semibold text-gray-900 flex items-center gap-1.5">
                <Wrench className="w-4 h-4 text-gray-400" /> {skill.name}
              </h3>
              <span className="text-[10px] text-gray-500 bg-gray-100 px-2 py-1 rounded-full">{skill.category || 'Skill'}</span>
            </div>
            <p className="text-sm text-gray-500 mb-4 flex-1">{skill.description}</p>
            <div className="flex justify-between items-center mt-auto">
              <span className="text-xs text-gray-400">v{skill.latest_version || skill.version}</span>
              <button
                onClick={() => handleInstall(skill)}
                disabled={!!installing || !selectedAgent}
                className="px-3 py-1.5 bg-[#10b981] text-white text-xs font-medium rounded-lg hover:bg-[#059669] disabled:opacity-50 flex items-center gap-1"
              >
                {installing === skill.id ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                赋能 Agent
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function Marketplace() {
  const [tab, setTab] = useState<'agents' | 'skills'>('agents');

  return (
    <div className="flex h-full w-full flex-col bg-[#F9FAFB] min-w-0">
      <div 
        className="shrink-0 border-b border-gray-200 bg-white px-6 py-4 pt-10 relative z-10"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      >
        <div style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties} className="flex items-center justify-between">
          <h1 className="text-xl font-bold text-gray-900">应用生态市场</h1>
          <div className="flex space-x-2 bg-gray-100 p-1 rounded-lg">
            <button
              onClick={() => setTab('agents')}
              className={`px-4 py-1.5 text-sm font-medium rounded-md transition-colors ${tab === 'agents' ? 'bg-white shadow text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
            >
              🚀 Agent 广场
            </button>
            <button
              onClick={() => setTab('skills')}
              className={`px-4 py-1.5 text-sm font-medium rounded-md transition-colors ${tab === 'skills' ? 'bg-white shadow text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
            >
              🛠️ 技能资源
            </button>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6 min-h-0">
        {tab === 'agents' ? <AgentPlaza /> : <SkillMarket />}
      </div>
    </div>
  );
}
