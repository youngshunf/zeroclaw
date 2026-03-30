import React, { useState, useEffect } from 'react';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { listAgents, type AgentInfo } from '../lib/agent-api';
import { listSops, getSopDetail, executeSop, type SopInfo, type SopDetailResponse } from '../lib/sop-api';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../components/ui/Select';
import { Workflow, Play, CheckCircle, AlertTriangle, FileText, Bot, History } from 'lucide-react';
import { SopRunPanel } from '../components/sop/SopRunPanel';
import { SopHistoryList } from '../components/sop/SopHistoryList';

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
    <div className="hx-sop-workbench" style={{ display: 'flex', height: '100%', width: '100%', overflow: 'hidden', background: 'var(--hx-bg-main)', color: 'var(--hx-text-primary)' }}>
      {/* ── 左侧侧边栏 ── */}
      <div style={{ width: 320, minWidth: 320, borderRight: '1px solid var(--hx-border)', display: 'flex', flexDirection: 'column' }}>
        <div style={{ padding: 16, borderBottom: '1px solid var(--hx-border)' }}>
          <label style={{ display: 'block', fontSize: 11, fontWeight: 600, color: 'var(--hx-text-tertiary)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 8 }}>
            当前智能体
          </label>
          <Select value={activeAgentName || ''} onValueChange={(v) => { setActiveAgentName(v); setSelectedSopName(null); }}>
            <SelectTrigger style={{ width: '100%', background: 'var(--hx-bg-input)', borderColor: 'var(--hx-border)', color: 'var(--hx-text-primary)' }}>
              <SelectValue placeholder="选择 Agent" />
            </SelectTrigger>
            <SelectContent>
              {agents.map(a => (
                <SelectItem key={a.name} value={a.name}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    {a.icon_url ? (
                      <img src={a.icon_url} alt={a.name} style={{ width: 16, height: 16, borderRadius: 4, objectFit: 'cover' }} />
                    ) : (
                      <Bot size={16} />
                    )}
                    <span>{a.display_name || a.name}</span>
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div style={{ flex: 1, overflowY: 'auto', padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
          {loading && <div style={{ textAlign: 'center', fontSize: 13, padding: '32px 0', color: 'var(--hx-text-tertiary)' }}>加载中...</div>}
          {!loading && sops.length === 0 && (
            <div style={{ textAlign: 'center', fontSize: 13, padding: '32px 0', color: 'var(--hx-text-tertiary)' }}>
              无可用工作流。<br />
              请前往应用市场安装。
            </div>
          )}
          {sops.map(sop => (
            <button
              key={sop.name}
              onClick={() => setSelectedSopName(sop.name)}
              style={{
                width: '100%',
                display: 'flex',
                flexDirection: 'column',
                textAlign: 'left',
                padding: '12px 16px',
                borderRadius: 'var(--hx-radius-md)',
                transition: 'all 0.15s',
                border: selectedSopName === sop.name ? '1px solid var(--hx-purple)' : '1px solid var(--hx-border)',
                background: selectedSopName === sop.name ? 'var(--hx-purple-bg)' : 'transparent',
                cursor: 'pointer',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 4, color: 'var(--hx-text-primary)' }}>
                <span style={{ fontWeight: 500, fontSize: 14, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{sop.name}</span>
                {sop.active_runs > 0 && (
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--hx-green)', boxShadow: '0 0 8px rgba(16,185,129,0.8)' }} title={`运行中: ${sop.active_runs}`} />
                )}
              </div>
              <p style={{ fontSize: 12, color: 'var(--hx-text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', margin: 0 }}>{sop.description}</p>
            </button>
          ))}
        </div>
      </div>

      {/* ── 右侧主内容 ── */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, position: 'relative' }}>
        {activeSessionId && activeAgentName && sopDetail ? (
          <div style={{ position: 'absolute', inset: 0, zIndex: 20, background: 'var(--hx-bg-main)', padding: 24 }}>
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
          <div style={{ display: 'flex', alignItems: 'center', borderBottom: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', flexShrink: 0, WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
            <button
              onClick={() => setRightTab('detail')}
              style={{
                display: 'flex', alignItems: 'center', gap: 8, padding: '12px 20px', fontSize: 14, fontWeight: 500,
                borderBottom: rightTab === 'detail' ? '2px solid var(--hx-purple)' : '2px solid transparent',
                color: rightTab === 'detail' ? 'var(--hx-purple)' : 'var(--hx-text-tertiary)',
                background: 'transparent', border: 'none', cursor: 'pointer',
                borderBottomStyle: 'solid', borderBottomWidth: 2,
                borderBottomColor: rightTab === 'detail' ? 'var(--hx-purple)' : 'transparent',
                transition: 'all 0.15s',
              }}
            >
              <Workflow className="w-4 h-4" />
              工作流详情
            </button>
            <button
              onClick={() => setRightTab('history')}
              style={{
                display: 'flex', alignItems: 'center', gap: 8, padding: '12px 20px', fontSize: 14, fontWeight: 500,
                color: rightTab === 'history' ? 'var(--hx-purple)' : 'var(--hx-text-tertiary)',
                background: 'transparent', border: 'none', cursor: 'pointer',
                borderBottomStyle: 'solid', borderBottomWidth: 2,
                borderBottomColor: rightTab === 'history' ? 'var(--hx-purple)' : 'transparent',
                transition: 'all 0.15s',
              }}
            >
              <History className="w-4 h-4" />
              执行历史
            </button>
          </div>
        )}

        {!activeAgentName ? (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--hx-text-secondary)' }}>
            ← 请先在左侧选择 Agent
          </div>
        ) : rightTab === 'history' ? (
          <SopHistoryList />
        ) : !selectedSopName ? (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--hx-text-secondary)' }}>
            ← 请选择一个工作流查看详情
          </div>
        ) : loadingDetail ? (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--hx-text-secondary)' }}>
            加载工作流详情中...
          </div>
        ) : sopDetail ? (
          <div style={{ flex: 1, overflowY: 'auto', padding: 32, maxWidth: 960, margin: '0 auto', width: '100%' }}>
            
            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 32 }}>
              <div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 8 }}>
                  <Workflow style={{ width: 32, height: 32, color: 'var(--hx-purple)' }} />
                  <h1 style={{ fontSize: 28, fontWeight: 700, color: 'var(--hx-text-primary)', margin: 0 }}>{sopDetail.name}</h1>
                  <span style={{ padding: '4px 10px', borderRadius: 'var(--hx-radius-sm)', background: 'var(--hx-purple-bg)', fontSize: 12, fontFamily: 'monospace', border: '1px solid var(--hx-border)', color: 'var(--hx-text-secondary)' }}>
                    v{sopDetail.version}
                  </span>
                </div>
                <p style={{ color: 'var(--hx-text-secondary)', fontSize: 16, marginTop: 12 }}>{sopDetail.description}</p>
              </div>
              
              <button
                onClick={handleExecute}
                disabled={executing}
                style={{
                  display: 'flex', alignItems: 'center', gap: 8, padding: '12px 24px',
                  background: 'var(--hx-purple)', color: '#fff', fontWeight: 500,
                  borderRadius: 'var(--hx-radius-md)', border: 'none', cursor: executing ? 'not-allowed' : 'pointer',
                  opacity: executing ? 0.5 : 1, boxShadow: '0 4px 14px rgba(124,58,237,0.3)', transition: 'all 0.2s',
                  flexShrink: 0,
                }}
              >
                <Play style={{ width: 20, height: 20, fill: 'currentColor' }} />
                {executing ? '启动中...' : '启动工作流'}
              </button>
            </div>

            {/* Tags / Metadata */}
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, marginBottom: 32 }}>
              {[
                { label: '执行模式', value: sopDetail.execution_mode },
                { label: '并发限制', value: String(sopDetail.max_concurrent) },
                { label: '优先级', value: sopDetail.priority },
              ].map(tag => (
                <div key={tag.label} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px', borderRadius: 'var(--hx-radius-sm)', background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)', fontSize: 13 }}>
                  <span style={{ color: 'var(--hx-text-tertiary)' }}>{tag.label}:</span>
                  <span style={{ color: 'var(--hx-text-primary)', textTransform: 'capitalize' }}>{tag.value}</span>
                </div>
              ))}
            </div>

            {/* Requirements Check */}
            {sopDetail.requirements && (
              <div style={{ marginBottom: 32, background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)', borderRadius: 'var(--hx-radius-md)', overflow: 'hidden' }}>
                <div style={{ padding: '12px 20px', borderBottom: '1px solid var(--hx-border)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', display: 'flex', alignItems: 'center', gap: 8, margin: 0, fontSize: 14 }}>
                    <CheckCircle style={{ width: 16, height: 16, color: 'var(--hx-green)' }} />
                    能力依赖项 (Requirements)
                  </h3>
                </div>
                <div style={{ padding: 20 }}>
                  {(sopDetail.requirements.skills.length === 0 && sopDetail.requirements.optional_skills.length === 0) ? (
                    <p style={{ color: 'var(--hx-text-tertiary)', fontSize: 13, margin: 0 }}>无特殊技能依赖。</p>
                  ) : (
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 16 }}>
                      {sopDetail.requirements.skills.length > 0 && (
                        <div>
                          <h4 style={{ fontSize: 11, fontWeight: 600, color: 'var(--hx-text-tertiary)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 8 }}>必备技能 (Required)</h4>
                          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                            {sopDetail.requirements.skills.map(s => (
                              <span key={s} style={{ padding: '4px 8px', borderRadius: 'var(--hx-radius-sm)', background: 'rgba(245,158,11,0.1)', color: 'var(--hx-amber)', border: '1px solid rgba(245,158,11,0.25)', fontSize: 12, display: 'flex', alignItems: 'center', gap: 6 }}>
                                <AlertTriangle style={{ width: 12, height: 12 }} />
                                {s}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}
                      {sopDetail.requirements.optional_skills.length > 0 && (
                        <div>
                          <h4 style={{ fontSize: 11, fontWeight: 600, color: 'var(--hx-text-tertiary)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 8 }}>可选增强 (Optional)</h4>
                          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                            {sopDetail.requirements.optional_skills.map(s => (
                              <span key={s} style={{ padding: '4px 8px', borderRadius: 'var(--hx-radius-sm)', background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', border: '1px solid var(--hx-border)', fontSize: 12 }}>
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
              <div style={{ marginBottom: 32 }}>
                <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
                  <Bot style={{ width: 20, height: 20, color: 'var(--hx-blue)' }} />
                  意图触发词 (Triggers)
                </h3>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {sopDetail.triggers.map(t => (
                    <span key={t} style={{ padding: '6px 12px', background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', border: '1px solid var(--hx-border)', borderRadius: 'var(--hx-radius-sm)', fontSize: 13 }}>
                      "{t}"
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Steps Workflow */}
            <div>
              <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 16, display: 'flex', alignItems: 'center', gap: 8 }}>
                <FileText style={{ width: 20, height: 20, color: 'var(--hx-text-tertiary)' }} />
                执行步骤 ({sopDetail.steps?.length || 0})
              </h3>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                {sopDetail.steps?.map((step, idx) => (
                  <div key={idx} style={{ display: 'flex', gap: 16 }}>
                    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
                      <div style={{
                        width: 32, height: 32, borderRadius: '50%', background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 13, fontWeight: 700, color: 'var(--hx-text-secondary)', flexShrink: 0
                      }}>
                        {step.number}
                      </div>
                      {idx < sopDetail.steps.length - 1 && (
                        <div style={{ width: 2, flex: 1, background: 'var(--hx-border)', margin: '4px 0' }} />
                      )}
                    </div>
                    <div style={{ paddingBottom: 24, flex: 1 }}>
                      <p style={{ fontSize: 15, fontWeight: 500, color: 'var(--hx-text-primary)', marginBottom: 4, marginTop: 4 }}>{step.title}</p>
                      
                      {step.requires_confirmation && (
                        <span style={{ display: 'inline-block', padding: '2px 8px', fontSize: 10, background: 'rgba(239,68,68,0.1)', color: 'var(--hx-red)', border: '1px solid rgba(239,68,68,0.25)', borderRadius: 4, marginBottom: 8 }}>需要人工审批</span>
                      )}

                      {step.suggested_tools && step.suggested_tools.length > 0 && (
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
                          <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginRight: 4 }}>🔧</span>
                          {step.suggested_tools.map(tool => (
                            <span key={tool} style={{ padding: '2px 6px', borderRadius: 4, background: 'var(--hx-bg-panel)', fontSize: 11, fontFamily: 'monospace', border: '1px solid var(--hx-border)', color: 'var(--hx-text-secondary)' }}>
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
