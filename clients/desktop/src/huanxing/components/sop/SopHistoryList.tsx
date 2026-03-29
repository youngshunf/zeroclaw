import React, { useEffect, useState } from 'react';
import { listRuns, type SopRun } from '../../lib/sop-api';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { RefreshCw, CheckCircle2, XCircle, Clock, Zap } from 'lucide-react';

export function SopHistoryList() {
  const [activeAgentName] = useActiveAgent();
  const [runs, setRuns] = useState<SopRun[]>([]);
  const [loading, setLoading] = useState(false);

  const loadRuns = async () => {
    if (!activeAgentName) return;
    try {
      setLoading(true);
      const res = await listRuns(activeAgentName, 'completed');
      const resActive = await listRuns(activeAgentName, 'active');
      const allRuns = [...resActive.runs, ...res.runs];
      
      const uniqueRunsMap = new Map();
      allRuns.forEach(r => uniqueRunsMap.set(r.run_id, r));
      
      const combined = Array.from(uniqueRunsMap.values());
      combined.sort((a, b) => new Date(b.started_at).getTime() - new Date(a.started_at).getTime());
      
      setRuns(combined);
    } catch (e) {
      console.error('Failed to load runs', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadRuns();
  }, [activeAgentName]);

  if (!activeAgentName) {
    return <div style={{ padding: 32, textAlign: 'center', color: 'var(--hx-text-tertiary)' }}>此模块需要选中一个 Agent</div>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--hx-bg-main)', color: 'var(--hx-text-primary)' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: 16, borderBottom: '1px solid var(--hx-border)' }}>
        <h2 style={{ fontSize: 13, fontWeight: 600, color: 'var(--hx-text-primary)', display: 'flex', alignItems: 'center', gap: 8, margin: 0 }}>
          <Clock style={{ width: 16, height: 16, color: 'var(--hx-purple)' }} /> 
          SOP 执行历史 ({runs.length})
        </h2>
        <button 
          onClick={loadRuns}
          disabled={loading}
          style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--hx-text-tertiary)', padding: 4 }}
        >
          <RefreshCw style={{ width: 16, height: 16, animation: loading ? 'hx-spin 1s linear infinite' : 'none' }} />
        </button>
      </div>
      
      <div style={{ flex: 1, overflowY: 'auto', padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
        {runs.length === 0 && !loading ? (
          <div style={{ textAlign: 'center', color: 'var(--hx-text-tertiary)', fontSize: 13, marginTop: 48 }}>
            暂无历史执行记录
          </div>
        ) : (
          runs.map(run => {
            const isCompleted = run.status === 'completed';
            const isFailed = run.status === 'failed' || run.status === 'cancelled';
            const isActive = run.status === 'pending' || run.status === 'running' || run.status === 'waiting_approval';
            
            return (
              <div 
                key={run.run_id} 
                style={{
                  padding: 12,
                  borderRadius: 'var(--hx-radius-md)',
                  border: `1px solid ${isActive ? 'var(--hx-purple)' : 'var(--hx-border)'}`,
                  background: isActive ? 'var(--hx-purple-bg)' : 'var(--hx-bg-panel)',
                  transition: 'all 0.15s',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    {isCompleted ? <CheckCircle2 style={{ width: 16, height: 16, color: 'var(--hx-green)' }} /> :
                     isFailed ? <XCircle style={{ width: 16, height: 16, color: 'var(--hx-red)' }} /> :
                     <RefreshCw style={{ width: 16, height: 16, color: 'var(--hx-purple)', animation: 'hx-spin 1s linear infinite' }} />}
                    <span style={{ fontWeight: 500, fontSize: 13, color: 'var(--hx-text-primary)' }}>{run.sop_name}</span>
                  </div>
                  <span style={{ fontSize: 10, color: 'var(--hx-text-tertiary)', fontFamily: 'monospace' }}>
                    {run.run_id.slice(0, 8)}
                  </span>
                </div>
                
                <div style={{ display: 'flex', alignItems: 'center', gap: 16, fontSize: 12, color: 'var(--hx-text-secondary)' }}>
                  <span>状态: 
                    <span style={{ marginLeft: 4, color: isActive ? 'var(--hx-purple)' : isCompleted ? 'var(--hx-green)' : 'var(--hx-red)' }}>
                      {run.status}
                    </span>
                  </span>
                  <span>进度: {Math.min(run.current_step, run.total_steps)}/{run.total_steps}</span>
                  {run.llm_calls_saved > 0 && (
                    <span style={{ display: 'flex', alignItems: 'center', gap: 4, color: 'var(--hx-amber)' }} title="节省了的 LLM 推理次数">
                      <Zap style={{ width: 12, height: 12 }} />
                      节约: {run.llm_calls_saved}
                    </span>
                  )}
                </div>
                
                <div style={{ marginTop: 8, fontSize: 10, color: 'var(--hx-text-tertiary)', display: 'flex', justifyContent: 'space-between' }}>
                  <span>开始于 {new Date(run.started_at).toLocaleString()}</span>
                  {run.completed_at && <span>结束于 {new Date(run.completed_at).toLocaleString()}</span>}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
