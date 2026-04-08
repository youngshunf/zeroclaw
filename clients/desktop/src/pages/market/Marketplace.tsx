import React, { useState, useEffect, useRef } from 'react';
import { Bot, Wrench, Workflow, Download, Loader2, CheckCircle, XCircle, RefreshCw } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { usePlatform } from '@/hooks/usePlatform';
import {
  getMarketApps,
  getMarketSkills,
  getMarketSops,
  installMarketAgent,
  installMarketSkill,
  installMarketSop,
  forceRefreshMarketCache,
  type MarketApp,
  type MarketSkill,
  type MarketSop,
} from '@/lib/marketplace-api';
import { listAgents, type AgentInfo } from '@/lib/agent-api';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { resolveApiUrl } from '@/config';
import { Input } from '@/components/ui/Input';

// ── Icon 渲染组件 ────────────────────────────────────────────
function ItemIcon({ iconUrl, emoji, fallback, size = 'md' }: { iconUrl?: string; emoji?: string; fallback: React.ReactNode; size?: 'md' | 'lg' }) {
  const cls = size === 'lg' ? 'w-10 h-10 rounded-xl' : 'w-7 h-7 rounded-lg';
  if (iconUrl) {
    return <img src={resolveApiUrl(iconUrl)} alt="" className={`${cls} object-cover border border-hx-border shadow-hx-shadow-sm`} />;
  }
  if (emoji) {
    return (
      <div className={`${cls} flex items-center justify-center bg-indigo-50/50 dark:bg-indigo-500/10 border border-indigo-100/50 dark:border-indigo-500/20 shadow-sm ${size === 'lg' ? 'text-2xl' : 'text-lg'}`}>
        {emoji}
      </div>
    );
  }
  return <>{fallback}</>;
}

export function useInstallManager() {
  const [showModal, setShowModal] = useState(false);
  const [installStatus, setInstallStatus] = useState<'idle' | 'installing' | 'success' | 'error'>('idle');
  const [installSteps, setInstallSteps] = useState<string[]>([]);
  const [installError, setInstallError] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [installSteps]);

  useEffect(() => {
    const unlisten = listen<{ message: string }>('agent-install-progress', (event) => {
      setInstallSteps(prev => [...prev, event.payload.message]);
    });
    return () => { unlisten.then(f => { try { f(); } catch { /* HMR safe */ } }); };
  }, []);

  return {
    showModal, setShowModal, installStatus, setInstallStatus,
    installSteps, setInstallSteps, installError, setInstallError, scrollRef
  };
}

export function InstallModal({
  isOpen, onClose, targetName, iconUrl, emoji, iconFallback,
  type, // 'agent'|'skill'|'sop'
  agentNameInput, setAgentNameInput,
  installStatus, installSteps, installError,
  onConfirm,
  scrollRef
}: {
  isOpen: boolean; onClose: () => void; targetName: string; iconUrl?: string; emoji?: string; iconFallback: React.ReactNode;
  type: 'agent'|'skill'|'sop';
  agentNameInput?: string; setAgentNameInput?: (v: string) => void;
  installStatus: 'idle' | 'installing' | 'success' | 'error';
  installSteps: string[]; installError: string;
  onConfirm: () => void;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}) {
  if (!isOpen) return null;

  const titlePrefix = type === 'agent' ? '配置 Agent' : type === 'skill' ? '技能赋能' : '安装工作流';
  const desc = type === 'agent' 
    ? '系统将自动执行：下载模板、解压环境、安装所需的技能和 SOP 依赖。'
    : type === 'skill' 
    ? '正在提取技能代码和环境配置，应用到目标 Agent。'
    : '正在装载 SOP 工作流配置，并自动补全局部技能依赖。';

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="bg-hx-bg-panel rounded-2xl w-[460px] max-w-[90vw] shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in duration-200 border border-hx-border">
        {/* Header */}
        <div className="border-hx-border px-6 py-4 border-b flex items-center gap-3">
          <ItemIcon iconUrl={iconUrl} emoji={emoji} fallback={iconFallback} />
          <div>
            <h2 className="text-hx-text-primary text-base font-bold leading-tight">{titlePrefix}：{targetName}</h2>
            <p className="text-hx-text-secondary text-xs">智能自动化部署</p>
          </div>
        </div>

        {/* Body */}
        <div className="p-6">
          {installStatus === 'idle' && (
            <div className="space-y-4">
              {type === 'agent' && (
                <div>
                  <label className="text-hx-text-primary block text-sm font-medium mb-1">为您的新 Agent 命名</label>
                  <Input 
                    type="text" 
                    value={agentNameInput || ''}
                    onChange={(e) => setAgentNameInput && setAgentNameInput(e.target.value)}
                    placeholder={targetName}
                    autoFocus
                    className="w-full"
                  />
                </div>
              )}
              <p className="bg-hx-purple-bg text-hx-text-secondary border-hx-border text-xs p-2.5 rounded-md border text-center">
                {desc}
              </p>
            </div>
          )}

          {installStatus === 'installing' && (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-indigo-500 dark:text-indigo-400 mb-2">
                <Loader2 className="w-4 h-4 animate-spin" />
                <span className="text-sm font-medium text-hx-text-primary">正在拉取与配置资源...</span>
              </div>
              <div 
                ref={scrollRef}
                className="bg-hx-bg-input text-hx-text-secondary border border-hx-border rounded-lg p-3 h-48 overflow-y-auto font-mono text-xs shadow-inner whitespace-pre-wrap"
              >
                {installSteps.map((step, idx) => {
                  const isError = step.toLowerCase().includes('error') || step.includes('失败') || step.includes('中止');
                  const isSuccess = step.includes('完成') || step.includes('成功');
                  const colorClass = isError ? 'text-red-500' : isSuccess ? 'text-emerald-500' : 'text-hx-text-primary';
                  return (
                    <div key={idx} className={`mb-1.5 flex items-start gap-1.5 leading-tight ${colorClass}`}>
                      <span className="text-hx-text-tertiary select-none shrink-0 font-medium">[{idx + 1 < 10 ? `0${idx+1}` : idx+1}]</span>
                      <span>{step}</span>
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {installStatus === 'success' && (
            <div className="py-6 flex flex-col items-center justify-center text-center">
              <div className="w-12 h-12 bg-green-500/10 text-green-500 border border-green-500/20 rounded-full flex items-center justify-center mb-3">
                <CheckCircle className="w-7 h-7" />
              </div>
              <h3 className="text-hx-text-primary text-lg font-bold mb-1">安装完成！</h3>
              <p className="text-hx-text-secondary text-sm max-w-[80%]">组件已赋能成功，现在可以前往工作台查看与使用。</p>
            </div>
          )}

          {installStatus === 'error' && (
            <div className="py-4 flex flex-col items-center justify-center text-center">
              <div className="w-12 h-12 bg-red-500/10 text-red-500 border border-red-500/20 rounded-full flex items-center justify-center mb-3">
                <XCircle className="w-7 h-7" />
              </div>
              <h3 className="text-hx-text-primary text-lg font-bold mb-2">安装意外中止</h3>
              <p className="text-xs text-red-600 bg-red-50/10 p-3 rounded-md border border-red-500/20 max-w-full overflow-hidden text-ellipsis text-left whitespace-pre-wrap">
                {installError}
              </p>
            </div>
          )}
        </div>

        {/* Footer Buttons */}
        <div className="bg-hx-bg-main border-hx-border px-6 py-4 border-t flex justify-end gap-2">
          {(installStatus === 'idle' || installStatus === 'error') && (
            <button 
              onClick={onClose}
              className="text-hx-text-secondary px-4 py-2 text-sm font-medium hover:text-hx-text-primary hover:bg-hx-bg-input rounded-lg transition-colors"
            >
              取消
            </button>
          )}
          {installStatus === 'success' && (
            <button 
              onClick={onClose}
              className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
            >
              关闭
            </button>
          )}
          {installStatus === 'idle' && (
            <button 
              onClick={onConfirm}
              className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
            >
              确认并安装
            </button>
          )}
          {installStatus === 'error' && (
            <button 
              onClick={onConfirm}
              className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
            >
              重试安装
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Agent 广场 ──────────────────────────────────────────────

function AgentPlaza() {
  const [apps, setApps] = useState<MarketApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);

  const fetchApps = () => {
    getMarketApps()
      .then((res) => setApps(res.items || []))
      .catch((err) => console.error('Failed to get market apps', err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchApps();
    // 监听后端同步完成事件，解决首次启动竞态问题
    const unlisten = listen('marketplace-synced', () => { fetchApps(); });
    return () => { unlisten.then(f => { try { f(); } catch { /* HMR safe */ } }); };
  }, []);

  // Modal States
  const { showModal, setShowModal, installStatus, setInstallStatus, installSteps, setInstallSteps, installError, setInstallError, scrollRef } = useInstallManager();
  const [targetApp, setTargetApp] = useState<MarketApp | null>(null);
  const [agentNameInput, setAgentNameInput] = useState('');

  const openInstallModal = (app: MarketApp) => {
    setTargetApp(app);
    setAgentNameInput(app.name);
    setInstallStatus('idle');
    setInstallSteps([]);
    setInstallError('');
    setShowModal(true);
  };

  const closeModal = () => {
    if (installStatus === 'installing') return;
    setShowModal(false);
    setTargetApp(null);
  };

  const confirmInstall = async () => {
    if (!targetApp) return;
    const appId = targetApp.app_id || targetApp.id;
    const defaultName = `${appId.toString().replace(/[^a-zA-Z0-9-]/g, "")}-${Math.floor(Math.random() * 1000)}`;
    const displayName = agentNameInput.trim() || targetApp.name;
    const pkgUrl = targetApp.package_url || ""; // Let Rust resolve it via backend

    setInstallStatus('installing');
    setInstallSteps(['🚀 开始配置 Agent...']);
    try {
      await installMarketAgent(appId, defaultName, displayName, pkgUrl);
      setInstallStatus('success');
    } catch (err: any) {
      setInstallError(err.message || String(err));
      setInstallStatus('error');
    }
  };

  if (loading) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Loader2 className="w-6 h-6 animate-spin mb-3 text-indigo-400" />
      <span className="text-sm">正在加载广场数据...</span>
    </div>
  );
  if (!apps.length) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Bot className="w-10 h-10 mb-3 opacity-30" />
      <span className="text-sm">暂无上架内容，请稍后再试</span>
    </div>
  );

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {apps.map((app) => (
        <div key={app.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">
          <div className="flex items-start justify-between mb-3">
            <div className="flex items-center gap-3">
              <ItemIcon iconUrl={app.icon_url} emoji={app.emoji} fallback={<Bot className="w-10 h-10 text-indigo-500 dark:text-indigo-400 p-2 bg-indigo-50 dark:bg-indigo-500/10 rounded-xl" />} size="lg" />
              <div>
                <h3 className="font-semibold text-hx-text-primary text-[15px] leading-tight">{app.name}</h3>
                {app.skill_dependencies && (
                  <span className="text-[10px] text-hx-text-tertiary">{app.skill_dependencies.split(',').length} 项技能</span>
                )}
              </div>
            </div>
              <span className="text-[11px] text-hx-text-secondary bg-hx-bg-input px-2.5 py-1 rounded-full">{app.category || 'App'}</span>
          </div>
          <p className="text-[13px] text-hx-text-secondary mb-4 flex-1 line-clamp-2 overflow-hidden h-10">{app.description}</p>
          <div className="flex justify-between items-center mt-auto">
            <span className="text-xs text-hx-text-tertiary">v{app.latest_version || '1.0.0'}</span>
            <button
              onClick={() => openInstallModal(app)}
              className="px-3 py-1.5 bg-[#7c3aed] text-white text-xs font-medium rounded-lg hover:bg-[#6d28d9] flex items-center gap-1 transition-colors"
            >
              <Download className="w-3.5 h-3.5" />
              下载安装
            </button>
          </div>
        </div>
      ))}

      {/* ── Install Modal ── */}
      {targetApp && (
        <InstallModal
          isOpen={showModal}
          onClose={closeModal}
          type="agent"
          targetName={targetApp.name}
          iconUrl={targetApp.icon_url}
          emoji={targetApp.emoji}
          iconFallback={<Bot className="w-8 h-8 text-indigo-500 dark:text-indigo-400 p-1.5 bg-indigo-50 dark:bg-indigo-500/10 rounded-lg" />}
          agentNameInput={agentNameInput}
          setAgentNameInput={setAgentNameInput}
          installStatus={installStatus}
          installSteps={installSteps}
          installError={installError}
          onConfirm={confirmInstall}
          scrollRef={scrollRef}
        />
      )}
    </div>
  );
}

// ── 技能市场 ────────────────────────────────────────────────

function SkillMarket() {
  const [skills, setSkills] = useState<MarketSkill[]>([]);
  const [localAgents, setLocalAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  const [installScope, setInstallScope] = useState<'agent' | 'user'>('agent');
  
  const { showModal, setShowModal, installStatus, setInstallStatus, installSteps, setInstallSteps, installError, setInstallError, scrollRef } = useInstallManager();
  const [targetSkill, setTargetSkill] = useState<MarketSkill | null>(null);

  const fetchSkills = () => {
    Promise.all([getMarketSkills(), listAgents()])
      .then(([skillRes, agentsRes]) => {
        setSkills(skillRes.items || []);
        setLocalAgents(agentsRes.agents || []);
        if (agentsRes.agents?.length > 0) {
          setSelectedAgent(prev => prev || agentsRes.agents[0].name);
        }
      })
      .catch((err) => console.error('Failed to init skills', err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchSkills();
    const unlisten = listen('marketplace-synced', () => { fetchSkills(); });
    return () => { unlisten.then(f => { try { f(); } catch { /* HMR safe */ } }); };
  }, []);

  const openInstallModal = (skill: MarketSkill) => {
    if (!selectedAgent) { alert('请先选择一个本地 Agent'); return; }
    setTargetSkill(skill);
    setInstallScope('agent');
    setInstallStatus('idle');
    setInstallSteps([]);
    setInstallError('');
    setShowModal(true);
  };

  const closeModal = () => {
    if (installStatus === 'installing') return;
    setShowModal(false);
    setTargetSkill(null);
  };

  const confirmInstall = async () => {
    if (!targetSkill || !selectedAgent) return;
    const skillId = targetSkill.skill_id || targetSkill.id;
    const pkgUrl = targetSkill.package_url || ""; // Let Rust resolve it via backend
    const scopeLabel = installScope === 'user' ? '用户公共' : 'Agent';

    setInstallStatus('installing');
    setInstallSteps([`🚀 开始将技能 ${targetSkill.name} 安装到${scopeLabel}目录...`]);
    try {
      await installMarketSkill(selectedAgent, String(skillId), pkgUrl, installScope);
      setInstallStatus('success');
    } catch (err: any) {
      setInstallError(err.message || String(err));
      setInstallStatus('error');
    }
  };

  if (loading) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Loader2 className="w-6 h-6 animate-spin mb-3 text-emerald-400" />
      <span className="text-sm">正在加载技能数据...</span>
    </div>
  );
  if (!skills.length) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Wrench className="w-10 h-10 mb-3 opacity-30" />
      <span className="text-sm">暂无上架技能，请稍后再试</span>
    </div>
  );

  return (
    <div className="space-y-6">
      <AgentSelector agents={localAgents} selected={selectedAgent} onChange={setSelectedAgent} label="选择目标 Agent（安装技能）" />
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {skills.map((skill) => (
          <div key={skill.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                <ItemIcon iconUrl={skill.icon_url} emoji={skill.emoji} fallback={<Wrench className="w-5 h-5 text-gray-400" />} />
                <h3 className="font-semibold text-hx-text-primary leading-tight">{skill.name}</h3>
              </div>
              <span className="text-[10px] text-hx-text-secondary bg-hx-bg-input px-2 py-1 rounded-full">{skill.category || 'Skill'}</span>
            </div>
            <p className="text-[13px] text-hx-text-secondary mb-4 flex-1 line-clamp-2 overflow-hidden">{skill.description}</p>
            <div className="flex justify-between items-center mt-auto">
              <span className="text-xs text-hx-text-tertiary">v{skill.latest_version || '1.0.0'}</span>
              <button
                onClick={() => openInstallModal(skill)}
                disabled={!selectedAgent}
                className="px-3 py-1.5 bg-[#10b981] dark:bg-emerald-600/90 text-white text-xs font-medium rounded-lg hover:bg-[#059669] dark:hover:bg-emerald-500 disabled:opacity-50 flex items-center gap-1 transition-colors"
              >
                <Download className="w-3.5 h-3.5" />
                赋能 Agent
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* ── Install Modal ── */}
      {targetSkill && showModal && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm">
          <div className="bg-hx-bg-panel rounded-2xl w-[460px] max-w-[90vw] shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in duration-200 border border-hx-border">
            {/* Header */}
            <div className="border-hx-border px-6 py-4 border-b flex items-center gap-3">
              <ItemIcon iconUrl={targetSkill.icon_url} emoji={targetSkill.emoji} fallback={<Wrench className="w-8 h-8 text-indigo-500 dark:text-indigo-400 p-1.5 bg-indigo-50 dark:bg-indigo-500/10 rounded-lg" />} />
              <div>
                <h2 className="text-hx-text-primary text-base font-bold leading-tight">技能赋能：{targetSkill.name}</h2>
                <p className="text-hx-text-secondary text-xs">选择安装位置</p>
              </div>
            </div>

            {/* Body */}
            <div className="p-6">
              {installStatus === 'idle' && (
                <div className="space-y-4">
                  {/* ── Install Scope Selector ── */}
                  <div className="space-y-2.5">
                    <label className="text-hx-text-primary block text-sm font-medium">安装位置</label>
                    <div
                      onClick={() => setInstallScope('agent')}
                      className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all ${
                        installScope === 'agent'
                          ? 'border-emerald-500/50 bg-emerald-500/5 shadow-sm'
                          : 'border-hx-border hover:border-hx-text-tertiary'
                      }`}
                    >
                      <div className={`mt-0.5 w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0 ${
                        installScope === 'agent' ? 'border-emerald-500' : 'border-hx-text-tertiary'
                      }`}>
                        {installScope === 'agent' && <div className="w-2 h-2 rounded-full bg-emerald-500" />}
                      </div>
                      <div>
                        <span className="text-hx-text-primary text-sm font-medium">安装到当前 Agent</span>
                        <p className="text-hx-text-tertiary text-[11px] mt-0.5">仅 <strong>{selectedAgent}</strong> 可使用此技能</p>
                      </div>
                    </div>
                    <div
                      onClick={() => setInstallScope('user')}
                      className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all ${
                        installScope === 'user'
                          ? 'border-indigo-500/50 bg-indigo-500/5 shadow-sm'
                          : 'border-hx-border hover:border-hx-text-tertiary'
                      }`}
                    >
                      <div className={`mt-0.5 w-4 h-4 rounded-full border-2 flex items-center justify-center shrink-0 ${
                        installScope === 'user' ? 'border-indigo-500' : 'border-hx-text-tertiary'
                      }`}>
                        {installScope === 'user' && <div className="w-2 h-2 rounded-full bg-indigo-500" />}
                      </div>
                      <div>
                        <span className="text-hx-text-primary text-sm font-medium">安装为公共技能</span>
                        <p className="text-hx-text-tertiary text-[11px] mt-0.5">您的<strong>所有 Agent</strong> 均可使用此技能</p>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {installStatus === 'installing' && (
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-indigo-500 dark:text-indigo-400 mb-2">
                    <Loader2 className="w-4 h-4 animate-spin" />
                    <span className="text-sm font-medium text-hx-text-primary">正在拉取与配置资源...</span>
                  </div>
                  <div 
                    ref={scrollRef}
                    className="bg-hx-bg-input text-hx-text-secondary border border-hx-border rounded-lg p-3 h-48 overflow-y-auto font-mono text-xs shadow-inner whitespace-pre-wrap"
                  >
                    {installSteps.map((step, idx) => {
                      const isError = step.toLowerCase().includes('error') || step.includes('失败');
                      const isSuccess = step.includes('完成') || step.includes('成功');
                      const colorClass = isError ? 'text-red-500' : isSuccess ? 'text-emerald-500' : 'text-hx-text-primary';
                      return (
                        <div key={idx} className={`mb-1.5 flex items-start gap-1.5 leading-tight ${colorClass}`}>
                          <span className="text-hx-text-tertiary select-none shrink-0 font-medium">[{idx + 1 < 10 ? `0${idx+1}` : idx+1}]</span>
                          <span>{step}</span>
                        </div>
                      )
                    })}
                  </div>
                </div>
              )}

              {installStatus === 'success' && (
                <div className="py-6 flex flex-col items-center justify-center text-center">
                  <div className="w-12 h-12 bg-green-500/10 text-green-500 border border-green-500/20 rounded-full flex items-center justify-center mb-3">
                    <CheckCircle className="w-7 h-7" />
                  </div>
                  <h3 className="text-hx-text-primary text-lg font-bold mb-1">安装完成！</h3>
                  <p className="text-hx-text-secondary text-sm max-w-[80%]">
                    {installScope === 'user'
                      ? '公共技能已就绪，所有 Agent 均可在下次对话中使用。'
                      : '技能已赋能成功，现在可以前往工作台查看与使用。'}
                  </p>
                </div>
              )}

              {installStatus === 'error' && (
                <div className="py-4 flex flex-col items-center justify-center text-center">
                  <div className="w-12 h-12 bg-red-500/10 text-red-500 border border-red-500/20 rounded-full flex items-center justify-center mb-3">
                    <XCircle className="w-7 h-7" />
                  </div>
                  <h3 className="text-hx-text-primary text-lg font-bold mb-2">安装意外中止</h3>
                  <p className="text-xs text-red-600 bg-red-50/10 p-3 rounded-md border border-red-500/20 max-w-full overflow-hidden text-ellipsis text-left whitespace-pre-wrap">
                    {installError}
                  </p>
                </div>
              )}
            </div>

            {/* Footer Buttons */}
            <div className="bg-hx-bg-main border-hx-border px-6 py-4 border-t flex justify-end gap-2">
              {(installStatus === 'idle' || installStatus === 'error') && (
                <button 
                  onClick={closeModal}
                  className="text-hx-text-secondary px-4 py-2 text-sm font-medium hover:text-hx-text-primary hover:bg-hx-bg-input rounded-lg transition-colors"
                >
                  取消
                </button>
              )}
              {installStatus === 'success' && (
                <button 
                  onClick={closeModal}
                  className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
                >
                  关闭
                </button>
              )}
              {installStatus === 'idle' && (
                <button 
                  onClick={confirmInstall}
                  className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
                >
                  确认并安装
                </button>
              )}
              {installStatus === 'error' && (
                <button 
                  onClick={confirmInstall}
                  className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors"
                >
                  重试安装
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── SOP 工作流市场 ──────────────────────────────────────────

function SopMarket() {
  const [sops, setSops] = useState<MarketSop[]>([]);
  const [localAgents, setLocalAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  
  const { showModal, setShowModal, installStatus, setInstallStatus, installSteps, setInstallSteps, installError, setInstallError, scrollRef } = useInstallManager();
  const [targetSop, setTargetSop] = useState<MarketSop | null>(null);

  const fetchSops = () => {
    Promise.all([getMarketSops(), listAgents()])
      .then(([sopRes, agentsRes]) => {
        setSops(sopRes.items || []);
        setLocalAgents(agentsRes.agents || []);
        if (agentsRes.agents?.length > 0) {
          setSelectedAgent(prev => prev || agentsRes.agents[0].name);
        }
      })
      .catch((err) => console.error('Failed to init sops', err))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchSops();
    const unlisten = listen('marketplace-synced', () => { fetchSops(); });
    return () => { unlisten.then(f => { try { f(); } catch { /* HMR safe */ } }); };
  }, []);

  const openInstallModal = (sop: MarketSop) => {
    if (!selectedAgent) { alert('请先选择一个本地 Agent'); return; }
    setTargetSop(sop);
    setInstallStatus('idle');
    setInstallSteps([]);
    setInstallError('');
    setShowModal(true);
  };

  const closeModal = () => {
    if (installStatus === 'installing') return;
    setShowModal(false);
    setTargetSop(null);
  };

  const confirmInstall = async () => {
    if (!targetSop || !selectedAgent) return;
    const sopId = targetSop.sop_id || targetSop.id;
    const pkgUrl = targetSop.package_url || ""; // Let Rust resolve it via backend

    setInstallStatus('installing');
    setInstallSteps([`🚀 开始将工作流 ${targetSop.name} 配置给 Agent...`]);
    try {
      await installMarketSop(selectedAgent, String(sopId), pkgUrl);
      setInstallStatus('success');
    } catch (err: any) {
      setInstallError(err.message || String(err));
      setInstallStatus('error');
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

  if (loading) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Loader2 className="w-6 h-6 animate-spin mb-3 text-blue-400" />
      <span className="text-sm">正在加载工作流数据...</span>
    </div>
  );
  if (!sops.length) return (
    <div className="flex flex-col items-center justify-center py-20 text-hx-text-tertiary">
      <Workflow className="w-10 h-10 mb-3 opacity-30" />
      <span className="text-sm">暂无上架工作流，请稍后再试</span>
    </div>
  );

  return (
    <div className="space-y-6">
      <AgentSelector agents={localAgents} selected={selectedAgent} onChange={setSelectedAgent} label="选择目标 Agent（安装工作流）" />
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {sops.map((sop) => (
          <div key={sop.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">
            <div className="flex items-start justify-between mb-2">
              <div className="flex items-center gap-2">
                <ItemIcon iconUrl={sop.icon_url} emoji={sop.emoji} fallback={<Workflow className="w-5 h-5 text-blue-400" />} />
                <h3 className="font-semibold text-hx-text-primary leading-tight">{sop.name}</h3>
              </div>
              <div className="flex gap-1.5">
                <span className="text-[10px] text-hx-blue bg-hx-purple-bg px-2 py-0.5 rounded-full">{modeLabel(sop.execution_mode)}</span>
                <span className="text-[10px] text-hx-text-secondary bg-hx-bg-input px-2 py-0.5 rounded-full">{sop.category || 'SOP'}</span>
              </div>
            </div>
            <p className="text-[13px] text-hx-text-secondary mb-3 flex-1 line-clamp-2 overflow-hidden">{sop.description}</p>
            {sop.skill_dependencies && (
              <div className="mb-3">
                <span className="text-[10px] text-gray-400">依赖技能: </span>
                {sop.skill_dependencies.split(',').map((dep) => (
                  <span key={dep.trim()} className="inline-block text-[10px] bg-amber-50 dark:bg-amber-500/10 text-amber-700 dark:text-amber-400 px-1.5 py-0.5 rounded mr-1 mb-1">
                    {dep.trim()}
                  </span>
                ))}
              </div>
            )}
            <div className="flex justify-between items-center mt-auto">
              <span className="text-xs text-hx-text-tertiary">v{sop.latest_version || '1.0.0'}</span>
              <button
                onClick={() => openInstallModal(sop)}
                disabled={!selectedAgent}
                className="px-3 py-1.5 bg-[#3b82f6] text-white text-xs font-medium rounded-lg hover:bg-[#2563eb] disabled:opacity-50 flex items-center gap-1 transition-colors"
              >
                <Download className="w-3.5 h-3.5" />
                安装工作流
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* ── Install Modal ── */}
      {targetSop && (
        <InstallModal
          isOpen={showModal}
          onClose={closeModal}
          type="sop"
          targetName={targetSop.name}
          iconUrl={targetSop.icon_url}
          emoji={targetSop.emoji}
          iconFallback={<Workflow className="w-8 h-8 text-indigo-500 dark:text-indigo-400 p-1.5 bg-indigo-50 dark:bg-indigo-500/10 rounded-lg" />}
          installStatus={installStatus}
          installSteps={installSteps}
          installError={installError}
          onConfirm={confirmInstall}
          scrollRef={scrollRef}
        />
      )}
    </div>
  );
}

// ── Agent 选择器复用组件 ──────────────────────────────────

function AgentSelector({ agents, selected, onChange, label }: { agents: AgentInfo[]; selected: string; onChange: (v: string) => void; label: string }) {
  return (
    <div className="bg-hx-bg-panel p-4 rounded-hx-radius-md border border-hx-border">
      <label className="block text-[13px] font-medium text-hx-text-secondary mb-2">{label}</label>
      <Select value={selected} onValueChange={onChange}>
        <SelectTrigger className="w-full max-w-64 bg-hx-bg-input text-hx-text-primary border-hx-border">
          <SelectValue placeholder="选择目标 Agent" />
        </SelectTrigger>
        <SelectContent>
          {agents.map(a => (
            <SelectItem key={a.name} value={a.name}>
              <div className="flex items-center gap-2">
                {a.icon_url ? (
                  <img src={resolveApiUrl(a.icon_url)} alt={a.name} className="w-4 h-4 rounded object-cover" />
                ) : (
                  <Bot className="w-4 h-4" />
                )}
                <span>{a.display_name || a.name}</span>
              </div>
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
  const [isRefreshing, setIsRefreshing] = useState(false);
  const { isMobile } = usePlatform();

  const handleForceRefresh = async () => {
    setIsRefreshing(true);
    try {
      await forceRefreshMarketCache();
      const { emit } = await import('@tauri-apps/api/event');
      await emit('marketplace-synced');
    } catch (err) {
      console.error('Refresh failed', err);
    } finally {
      setIsRefreshing(false);
    }
  };

  const tabs = [
    { key: 'agents' as const, label: isMobile ? 'Agent' : 'Agent 广场', icon: Bot, color: '#7c3aed' },
    { key: 'skills' as const, label: isMobile ? '技能' : '技能资源', icon: Wrench, color: '#10b981' },
    { key: 'sops' as const, label: isMobile ? '工作流' : '工作流市场', icon: Workflow, color: '#3b82f6' },
  ];

  return (
    <div className="flex h-full w-full flex-col bg-hx-bg-main min-w-0 text-hx-text-primary">
      <div 
        className={`shrink-0 border-b border-hx-border bg-hx-bg-panel ${isMobile ? 'pt-2 px-3 pb-2' : 'pt-5 px-6 pb-3'} relative z-10`}
        style={isMobile ? undefined : { WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region={!isMobile}
      >
        <div className="flex items-center justify-center w-full" style={isMobile ? undefined : { WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <div className={`flex gap-1.5 bg-hx-bg-input p-1.5 rounded-hx-radius-md ${isMobile ? 'w-full overflow-x-auto' : ''}`}>
            {tabs.map(t => (
              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                className={`flex items-center ${isMobile ? 'flex-1 justify-center px-2 py-2' : 'px-4 py-1.5'} text-[13px] font-medium rounded-hx-radius-sm transition-all duration-150 border-none cursor-pointer ${
                  tab === t.key 
                    ? 'bg-hx-bg-main text-hx-text-primary shadow-hx-shadow-sm' 
                    : 'bg-transparent text-hx-text-tertiary hover:text-hx-text-secondary'
                }`}
              >
                <t.icon className="w-4 h-4 mr-1.5" /> {t.label}
              </button>
            ))}
          </div>

          <button 
            onClick={handleForceRefresh}
            disabled={isRefreshing}
            className={`absolute right-6 top-1/2 -translate-y-1/2 p-2 text-hx-text-tertiary hover:text-hx-text-primary hover:bg-hx-bg-input rounded-lg transition-colors cursor-pointer ${isRefreshing ? 'opacity-50' : ''}`}
            title="强制刷新资源"
            style={isMobile ? undefined : { WebkitAppRegion: 'no-drag' } as React.CSSProperties}
          >
            <RefreshCw className={`w-4 h-4 ${isRefreshing ? 'animate-spin text-hx-purple' : ''}`} />
          </button>
        </div>
      </div>

      <div className={`flex-1 overflow-y-auto ${isMobile ? 'p-3' : 'p-6'} min-h-0`}>
        {tab === 'agents' && <AgentPlaza />}
        {tab === 'skills' && <SkillMarket />}
        {tab === 'sops' && <SopMarket />}
      </div>
    </div>
  );
}
