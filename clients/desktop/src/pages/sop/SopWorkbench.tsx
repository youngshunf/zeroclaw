import React, { useState, useEffect } from 'react';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { listAgents, type AgentInfo } from '@/lib/agent-api';
import { listSops, getSopDetail, executeSop, type SopInfo, type SopDetailResponse } from '@/lib/sop-api';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { Workflow, Play, CheckCircle, AlertTriangle, FileText, Bot, History } from 'lucide-react';
import { SopRunPanel } from '@/components/sop/SopRunPanel';
import { SopHistoryList } from '@/components/sop/SopHistoryList';
import { resolveApiUrl } from '@/config';

export default function SopWorkbench() {
  const [activeAgentName, setActiveAgentName] = useActiveAgent();
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [sops, setSops] = useState<SopInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedSopName, setSelectedSopName] = useState<string | null>(null);
  const [sopDetail, setSopDetail] = useState<SopDetailResponse | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [executing, setExecuting] = useState(false);
  const [rightTab, setRightTab] = useState<'detail' | 'history'>('detail');

  useEffect(() => {
    listAgents().then(res => setAgents(res.agents || [])).catch(console.error);
  }, []);

  useEffect(() => {
    if (!activeAgentName) {
      setSops([]);
      setSelectedSopName(null);
      return;
    }
    setLoading(true);
    listSops(activeAgentName)
      .then(res => {
        setSops(res.sops);
        if (res.sops.length > 0 && !selectedSopName) {
          setSelectedSopName(res.sops[0].name);
        }
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [activeAgentName]);

  useEffect(() => {
    if (!activeAgentName || !selectedSopName) {
      setSopDetail(null);
      return;
    }
    setLoadingDetail(true);
    getSopDetail(activeAgentName, selectedSopName)
      .then(setSopDetail)
      .catch(console.error)
      .finally(() => setLoadingDetail(false));
  }, [activeAgentName, selectedSopName]);

  const handleExecute = async () => {
    if (!sopDetail || !activeAgentName) return;
    try {
      setExecuting(true);
      const res = await executeSop(activeAgentName, sopDetail.name);
      setActiveSessionId(res.session_id);
    } catch (e: any) {
      alert(`启动失败: ${e.message}`);
    } finally {
      setExecuting(false);
    }
  };

  return (
    <div className="flex h-full w-full overflow-hidden bg-hx-bg-main text-hx-text-primary">
      {/* ── 左侧侧边栏 ── */}
      <div className="w-[320px] min-w-[320px] border-r border-hx-border flex flex-col">
        <div className="p-4 border-b border-hx-border">
          <label className="block text-[11px] font-semibold text-hx-text-tertiary uppercase tracking-wider mb-2">
            当前智能体
          </label>
          <Select value={activeAgentName || ''} onValueChange={(v) => { setActiveAgentName(v); setSelectedSopName(null); }}>
            <SelectTrigger className="w-full bg-hx-bg-input border-hx-border text-hx-text-primary">
              <SelectValue placeholder="选择 Agent" />
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

        <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-2">
          {loading && <div className="text-center text-[13px] py-8 text-hx-text-tertiary">加载中...</div>}
          {!loading && sops.length === 0 && (
            <div className="text-center text-[13px] py-8 text-hx-text-tertiary">
              无可用工作流。<br />
              请前往应用市场安装。
            </div>
          )}
          {sops.map(sop => (
            <button
              key={sop.name}
              onClick={() => setSelectedSopName(sop.name)}
              className={`w-full flex flex-col text-left p-3 px-4 rounded-hx-radius-md transition-all duration-150 border cursor-pointer ${
                selectedSopName === sop.name 
                  ? 'border-hx-purple bg-hx-purple-bg'
                  : 'border-hx-border bg-transparent hover:bg-hx-bg-panel'
              }`}
            >
              <div className="flex items-center justify-between mb-1 text-hx-text-primary w-full">
                <div className="flex items-center gap-2 overflow-hidden mb-1">
                  <Workflow className="w-4 h-4 shrink-0 opacity-70" />
                  <span className="font-medium text-sm overflow-hidden text-ellipsis whitespace-nowrap">{sop.display_name || sop.name}</span>
                </div>
                {sop.active_runs > 0 && (
                  <span className="w-2 h-2 rounded-full bg-hx-green shadow-[0_0_8px_rgba(16,185,129,0.8)] shrink-0" title={`运行中: ${sop.active_runs}`} />
                )}
              </div>
              <p className="text-[12px] text-hx-text-secondary overflow-hidden text-ellipsis whitespace-nowrap m-0 w-full">{sop.description}</p>
            </button>
          ))}
        </div>
      </div>

      {/* ── 右侧主内容 ── */}
      <div className="flex-1 flex flex-col min-w-0 relative">
        {activeSessionId && activeAgentName && sopDetail ? (
          <div className="absolute inset-0 z-20 bg-hx-bg-main p-6">
             <SopRunPanel
               sessionId={activeSessionId}
               agentName={activeAgentName}
               sopName={sopDetail.name}
               onClose={() => setActiveSessionId(null)}
             />
          </div>
        ) : null}

        {/* Tab bar */}
        {activeAgentName && (
          <div 
            className="flex items-center border-b border-hx-border bg-hx-bg-panel shrink-0"
            style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
          >
            <button
              onClick={() => setRightTab('detail')}
              className={`flex items-center gap-2 px-5 py-3 text-sm font-medium bg-transparent border-none cursor-pointer border-b-2 transition-all duration-150 ${
                rightTab === 'detail' 
                  ? 'border-b-hx-purple text-hx-purple' 
                  : 'border-b-transparent text-hx-text-tertiary hover:text-hx-text-primary'
              }`}
            >
              <Workflow className="w-4 h-4" />
              工作流详情
            </button>
            <button
              onClick={() => setRightTab('history')}
              className={`flex items-center gap-2 px-5 py-3 text-sm font-medium bg-transparent border-none cursor-pointer border-b-2 transition-all duration-150 ${
                rightTab === 'history' 
                  ? 'border-b-hx-purple text-hx-purple' 
                  : 'border-b-transparent text-hx-text-tertiary hover:text-hx-text-primary'
              }`}
            >
              <History className="w-4 h-4" />
              执行历史
            </button>
          </div>
        )}

        {!activeAgentName ? (
          <div className="flex-1 flex items-center justify-center text-hx-text-secondary">
            ← 请先在左侧选择 Agent
          </div>
        ) : rightTab === 'history' ? (
          <SopHistoryList />
        ) : !selectedSopName ? (
          <div className="flex-1 flex items-center justify-center text-hx-text-secondary">
            ← 请选择一个工作流查看详情
          </div>
        ) : loadingDetail ? (
          <div className="flex-1 flex items-center justify-center text-hx-text-secondary">
            加载工作流详情中...
          </div>
        ) : sopDetail ? (
          <div className="flex-1 overflow-y-auto p-8 max-w-[960px] mx-auto w-full">
            
            <div className="flex items-start justify-between mb-8">
              <div>
                <div className="flex items-center gap-3 mb-2">
                  <Workflow className="w-8 h-8 text-hx-purple shrink-0" />
                  <h1 className="text-2xl font-bold text-hx-text-primary m-0">{sopDetail.name}</h1>
                  <span className="px-2.5 py-1 rounded-hx-radius-sm bg-hx-purple-bg text-[12px] font-mono border border-hx-border text-hx-text-secondary">
                    v{sopDetail.version}
                  </span>
                </div>
                <p className="text-hx-text-secondary text-base mt-3">{sopDetail.description}</p>
              </div>
              
              <button
                onClick={handleExecute}
                disabled={executing}
                className={`flex items-center gap-2 px-6 py-3 bg-hx-purple text-white font-medium rounded-hx-radius-md border-none shrink-0 shadow-[0_4px_14px_rgba(124,58,237,0.3)] transition-all duration-200 ${
                  executing ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:bg-hx-purple-hover'
                }`}
              >
                <Play className="w-5 h-5 fill-current" />
                {executing ? '启动中...' : '启动工作流'}
              </button>
            </div>

            {/* Tags / Metadata */}
            <div className="flex flex-wrap gap-3 mb-8">
              {[
                { label: '执行模式', value: sopDetail.execution_mode },
                { label: '并发限制', value: String(sopDetail.max_concurrent) },
                { label: '优先级', value: sopDetail.priority },
              ].map(tag => (
                <div key={tag.label} className="flex items-center gap-1.5 px-3 py-1.5 rounded-hx-radius-sm bg-hx-bg-panel border border-hx-border text-[13px]">
                  <span className="text-hx-text-tertiary">{tag.label}:</span>
                  <span className="text-hx-text-primary capitalize">{tag.value}</span>
                </div>
              ))}
            </div>

            {/* Requirements Check */}
            {sopDetail.requirements && (
              <div className="mb-8 bg-hx-bg-panel border border-hx-border rounded-hx-radius-md overflow-hidden">
                <div className="px-5 py-3 border-b border-hx-border flex justify-between items-center bg-hx-bg-main/50">
                  <h3 className="font-semibold text-hx-text-primary flex items-center gap-2 m-0 text-sm">
                    <CheckCircle className="w-4 h-4 text-hx-green" />
                    能力依赖项 (Requirements)
                  </h3>
                </div>
                <div className="p-5">
                  {(sopDetail.requirements.skills.length === 0 && sopDetail.requirements.optional_skills.length === 0) ? (
                    <p className="text-hx-text-tertiary text-[13px] m-0">无特殊技能依赖。</p>
                  ) : (
                    <div className="grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-4">
                      {sopDetail.requirements.skills.length > 0 && (
                        <div>
                          <h4 className="text-[11px] font-semibold text-hx-text-tertiary uppercase tracking-wider mb-2">必备技能 (Required)</h4>
                          <div className="flex flex-wrap gap-2">
                            {sopDetail.requirements.skills.map(s => (
                              <span key={s} className="px-2 py-1 rounded-hx-radius-sm bg-orange-500/10 text-orange-500 border border-orange-500/25 text-[12px] flex items-center gap-1.5">
                                <AlertTriangle className="w-3 h-3" />
                                {s}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                      {sopDetail.requirements.optional_skills.length > 0 && (
                        <div>
                          <h4 className="text-[11px] font-semibold text-hx-text-tertiary uppercase tracking-wider mb-2">可选增强 (Optional)</h4>
                          <div className="flex flex-wrap gap-2">
                            {sopDetail.requirements.optional_skills.map(s => (
                              <span key={s} className="px-2 py-1 rounded-hx-radius-sm bg-hx-purple-bg text-hx-purple border border-hx-border text-[12px]">
                                {s}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Triggers */}
            {sopDetail.triggers?.length > 0 && (
              <div className="mb-8">
                <h3 className="text-base font-semibold text-hx-text-primary mb-3 flex items-center gap-2">
                  <Bot className="w-5 h-5 text-hx-blue" />
                  意图触发词 (Triggers)
                </h3>
                <div className="flex flex-wrap gap-2">
                  {sopDetail.triggers.map(t => (
                    <span key={t} className="px-3 py-1.5 bg-hx-purple-bg text-hx-purple border border-hx-border rounded-hx-radius-sm text-[13px]">
                      "{t}"
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Steps Workflow */}
            <div>
              <h3 className="text-base font-semibold text-hx-text-primary mb-4 flex items-center gap-2">
                <FileText className="w-5 h-5 text-hx-text-tertiary" />
                执行步骤 ({sopDetail.steps?.length || 0})
              </h3>
              <div className="flex flex-col gap-4">
                {sopDetail.steps?.map((step, idx) => (
                  <div key={idx} className="flex gap-4">
                    <div className="flex flex-col items-center">
                      <div className="w-8 h-8 rounded-full bg-hx-bg-panel border border-hx-border flex items-center justify-center text-[13px] font-bold text-hx-text-secondary shrink-0 shadow-sm">
                        {step.number}
                      </div>
                      {idx < sopDetail.steps.length - 1 && (
                        <div className="w-[2px] flex-1 bg-hx-border my-1" />
                      )}
                    </div>
                    <div className="pb-6 flex-1">
                      <p className="text-[15px] font-medium text-hx-text-primary mb-1 mt-1">{step.title}</p>
                      
                      {step.requires_confirmation && (
                        <span className="inline-block px-2 py-0.5 text-[10px] bg-red-500/10 text-red-500 border border-red-500/25 rounded mb-2 font-medium">需要人工审批</span>
                      )}

                      {step.suggested_tools && step.suggested_tools.length > 0 && (
                        <div className="flex flex-wrap gap-1.5 mt-2">
                          <span className="text-[12px] text-hx-text-tertiary mr-1">🔧</span>
                          {step.suggested_tools.map(tool => (
                            <span key={tool} className="px-1.5 py-0.5 rounded bg-hx-bg-panel text-[11px] font-mono border border-hx-border text-hx-text-secondary">
                              {tool}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>

          </div>
        ) : null}
      </div>
    </div>
  );
}
