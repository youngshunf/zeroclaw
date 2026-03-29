import React, { useState, useEffect } from 'react';
import { Bot, Wrench, Workflow, Download, Loader2 } from 'lucide-react';
import {
  getMarketApps,
  getMarketSkills,
  getMarketSops,
  installMarketAgent,
  installMarketSkill,
  installMarketSop,
  type MarketApp,
  type MarketSkill,
  type MarketSop,
} from '../lib/marketplace-api';
import { listAgents, type AgentInfo } from '../lib/agent-api';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../components/ui/Select';

// ── Icon 渲染组件 ────────────────────────────────────────────
function ItemIcon({ iconUrl, emoji, fallback, size = 'md' }: { iconUrl?: string; emoji?: string; fallback: React.ReactNode; size?: 'md' | 'lg' }) {
  const cls = size === 'lg' ? 'w-10 h-10 rounded-xl' : 'w-7 h-7 rounded-lg';
  if (iconUrl) {
    return <img src={iconUrl} alt="" className={`${cls} object-cover border border-gray-100 shadow-sm`} />;
  }
  if (emoji) {
    return (
      <div className={`${cls} flex items-center justify-center bg-indigo-50/50 border border-indigo-100/50 shadow-sm ${size === 'lg' ? 'text-2xl' : 'text-lg'}`}>
        {emoji}
      </div>
    );
  }
  return <>{fallback}</>;
}

// ── Agent 广场 ──────────────────────────────────────────────

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
    const appId = app.app_id || app.id;
    const defaultName = `${appId.toString().replace(/[^a-zA-Z0-9-]/g, "")}-${Math.floor(Math.random() * 1000)}`;
    
    // In Tauri v2, window.prompt might be blocked or absent in some environments.
    // If it returns null/empty immediately (or user cancels), we'll fallback to auto naming instead of aborting.
    let displayName = app.name;
    try {
      const userRes = window.prompt('为新 Agent 起个名字 (留空默认使用应用名):', app.name);
      if (userRes && userRes.trim() !== '') {
        displayName = userRes.trim();
      }
    } catch {
      // Ignored if prompt throws
    }

    const pkgUrl = app.package_url || ""; // Let Rust resolve it via backend

    setInstalling(String(app.id));
    try {
      await installMarketAgent(appId, defaultName, displayName, pkgUrl);
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
        <div key={app.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>
          <div className="flex items-start justify-between mb-3">
            <div className="flex items-center gap-3">
              <ItemIcon iconUrl={app.icon_url} emoji={app.emoji} fallback={<Bot className="w-10 h-10 text-indigo-400 p-2 bg-indigo-50 rounded-xl" />} size="lg" />
              <div>
                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', fontSize: 15, lineHeight: 1.3 }}>{app.name}</h3>
                {app.skill_dependencies && (
                  <span style={{ fontSize: 10, color: 'var(--hx-text-tertiary)' }}>{app.skill_dependencies.split(',').length} 项技能</span>
                )}
              </div>
            </div>
              <span style={{ fontSize: 11, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '4px 10px', borderRadius: 9999 }}>{app.category || 'App'}</span>
          </div>
          <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 16, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden', height: 40 }}>{app.description}</p>
          <div className="flex justify-between items-center mt-auto">
            <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{app.latest_version || '1.0.0'}</span>
            <button
              onClick={() => handleInstall(app)}
              disabled={!!installing}
              className="px-3 py-1.5 bg-[#7c3aed] text-white text-xs font-medium rounded-lg hover:bg-[#6d28d9] disabled:opacity-50 flex items-center gap-1 transition-colors"
            >
              {installing === String(app.id) ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
              下载安装
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── 技能市场 ────────────────────────────────────────────────

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
    if (!selectedAgent) { alert('请先选择一个本地 Agent'); return; }
    const skillId = skill.skill_id || skill.id;
    const pkgUrl = skill.package_url || ""; // Let Rust resolve it via backend

    setInstalling(String(skill.id));
    try {
      await installMarketSkill(selectedAgent, String(skillId), pkgUrl);
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
      <AgentSelector agents={localAgents} selected={selectedAgent} onChange={setSelectedAgent} label="选择目标 Agent（安装技能）" />
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {skills.map((skill) => (
          <div key={skill.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                <ItemIcon iconUrl={skill.icon_url} emoji={skill.emoji} fallback={<Wrench className="w-5 h-5 text-gray-400" />} />
                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', lineHeight: 1.3 }}>{skill.name}</h3>
              </div>
              <span style={{ fontSize: 10, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '4px 8px', borderRadius: 9999 }}>{skill.category || 'Skill'}</span>
            </div>
            <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 16, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>{skill.description}</p>
            <div className="flex justify-between items-center mt-auto">
              <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{skill.latest_version || '1.0.0'}</span>
              <button
                onClick={() => handleInstall(skill)}
                disabled={!!installing || !selectedAgent}
                className="px-3 py-1.5 bg-[#10b981] text-white text-xs font-medium rounded-lg hover:bg-[#059669] disabled:opacity-50 flex items-center gap-1 transition-colors"
              >
                {installing === String(skill.id) ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                赋能 Agent
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── SOP 工作流市场 ──────────────────────────────────────────

function SopMarket() {
  const [sops, setSops] = useState<MarketSop[]>([]);
  const [localAgents, setLocalAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  const [installing, setInstalling] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getMarketSops(), listAgents()])
      .then(([sopRes, agentsRes]) => {
        setSops(sopRes.items || []);
        setLocalAgents(agentsRes.agents || []);
        if (agentsRes.agents?.length > 0) {
          setSelectedAgent(agentsRes.agents[0].name);
        }
      })
      .catch((err) => console.error('Failed to init sops', err))
      .finally(() => setLoading(false));
  }, []);

  const handleInstall = async (sop: MarketSop) => {
    if (!selectedAgent) { alert('请先选择一个本地 Agent'); return; }
    const sopId = sop.sop_id || sop.id;
    const pkgUrl = sop.package_url || ""; // Let Rust resolve it via backend

    setInstalling(String(sop.id));
    try {
      await installMarketSop(selectedAgent, String(sopId), pkgUrl);
      alert('SOP 工作流安装成功！依赖的技能已自动安装。');
    } catch (err: any) {
      alert(`安装失败: ${err.message || err}`);
    } finally {
      setInstalling(null);
    }
  };

  const modeLabel = (mode?: string) => {
    switch (mode) {
      case 'auto': return '全自动';
      case 'supervised': return '监督式';
      case 'step_by_step': return '逐步';
      case 'deterministic': return '确定性';
      default: return mode || '监督式';
    }
  };

  if (loading) return <div className="p-8 text-gray-400">Loading...</div>;
  if (!sops.length) return <div className="p-8 text-gray-400">工作流市场尚未上架内容。</div>;

  return (
    <div className="space-y-6">
      <AgentSelector agents={localAgents} selected={selectedAgent} onChange={setSelectedAgent} label="选择目标 Agent（安装工作流）" />
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {sops.map((sop) => (
          <div key={sop.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                <ItemIcon iconUrl={sop.icon_url} emoji={sop.emoji} fallback={<Workflow className="w-5 h-5 text-blue-400" />} />
                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', lineHeight: 1.3 }}>{sop.name}</h3>
              </div>
              <div className="flex gap-1.5">
                <span style={{ fontSize: 10, color: 'var(--hx-blue)', background: 'var(--hx-purple-bg)', padding: '2px 8px', borderRadius: 9999 }}>{modeLabel(sop.execution_mode)}</span>
                <span style={{ fontSize: 10, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '2px 8px', borderRadius: 9999 }}>{sop.category || 'SOP'}</span>
              </div>
            </div>
            <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 12, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>{sop.description}</p>
            {sop.skill_dependencies && (
              <div className="mb-3">
                <span className="text-[10px] text-gray-400">依赖技能: </span>
                {sop.skill_dependencies.split(',').map((dep) => (
                  <span key={dep.trim()} className="inline-block text-[10px] bg-amber-50 text-amber-700 px-1.5 py-0.5 rounded mr-1 mb-1">
                    {dep.trim()}
                  </span>
                ))}
              </div>
            )}
            <div className="flex justify-between items-center mt-auto">
              <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{sop.latest_version || '1.0.0'}</span>
              <button
                onClick={() => handleInstall(sop)}
                disabled={!!installing || !selectedAgent}
                className="px-3 py-1.5 bg-[#3b82f6] text-white text-xs font-medium rounded-lg hover:bg-[#2563eb] disabled:opacity-50 flex items-center gap-1 transition-colors"
              >
                {installing === String(sop.id) ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Download className="w-3.5 h-3.5" />}
                安装工作流
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Agent 选择器复用组件 ──────────────────────────────────

function AgentSelector({ agents, selected, onChange, label }: { agents: AgentInfo[]; selected: string; onChange: (v: string) => void; label: string }) {
  return (
    <div style={{ background: 'var(--hx-bg-panel)', padding: 16, borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)' }}>
      <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 8 }}>{label}</label>
      <Select value={selected} onValueChange={onChange}>
        <SelectTrigger style={{ width: '100%', maxWidth: 256, background: 'var(--hx-bg-input)', color: 'var(--hx-text-primary)', borderColor: 'var(--hx-border)' }}>
          <SelectValue placeholder="选择目标 Agent" />
        </SelectTrigger>
        <SelectContent>
          {agents.map(a => (
            <SelectItem key={a.name} value={a.name}>
              {a.display_name || a.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

// ── 主页面 ──────────────────────────────────────────────────

export default function Marketplace() {
  const [tab, setTab] = useState<'agents' | 'skills' | 'sops'>('agents');

  const tabs = [
    { key: 'agents' as const, label: 'Agent 广场', icon: Bot, color: '#7c3aed' },
    { key: 'skills' as const, label: '技能资源', icon: Wrench, color: '#10b981' },
    { key: 'sops' as const, label: '工作流市场', icon: Workflow, color: '#3b82f6' },
  ];

  return (
    <div style={{ display: 'flex', height: '100%', width: '100%', flexDirection: 'column', background: 'var(--hx-bg-main)', minWidth: 0, color: 'var(--hx-text-primary)' }}>
      <div 
        style={{ flexShrink: 0, borderBottom: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: '20px 24px 12px', position: 'relative', zIndex: 10, WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      >
        <div style={{ WebkitAppRegion: 'no-drag', display: 'flex', alignItems: 'center', justifyContent: 'center', width: '100%' } as React.CSSProperties}>
          <div style={{ display: 'flex', gap: 6, background: 'var(--hx-bg-input)', padding: 6, borderRadius: 'var(--hx-radius-md)' }}>
            {tabs.map(t => (
              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                style={{
                  display: 'flex', alignItems: 'center', padding: '6px 16px', fontSize: 13, fontWeight: 500,
                  borderRadius: 'var(--hx-radius-sm)', transition: 'all 0.15s', border: 'none', cursor: 'pointer',
                  background: tab === t.key ? 'var(--hx-bg-main)' : 'transparent',
                  color: tab === t.key ? 'var(--hx-text-primary)' : 'var(--hx-text-tertiary)',
                  boxShadow: tab === t.key ? 'var(--hx-shadow-sm)' : 'none',
                }}
              >
                <t.icon style={{ width: 16, height: 16, marginRight: 6 }} /> {t.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6 min-h-0">
        {tab === 'agents' && <AgentPlaza />}
        {tab === 'skills' && <SkillMarket />}
        {tab === 'sops' && <SopMarket />}
      </div>
    </div>
  );
}
