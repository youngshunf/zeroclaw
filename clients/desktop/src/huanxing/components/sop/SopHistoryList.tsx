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
    return <div className="p-8 text-center text-hx-text-tertiary">此模块需要选中一个 Agent</div>;
  }

  return (
    <div className="flex flex-col h-full bg-hx-bg-main text-hx-text-primary">
      <div className="flex items-center justify-between p-4 border-b border-hx-border">
        <h2 className="text-[13px] font-semibold text-hx-text-primary flex items-center gap-2 m-0">
          <Clock className="w-4 h-4 text-hx-purple" /> 
          SOP 执行历史 ({runs.length})
        </h2>
        <button 
          onClick={loadRuns}
          disabled={loading}
          className="bg-transparent border-none cursor-pointer text-hx-text-tertiary p-1 disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
        </button>
      </div>
      
      <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
        {runs.length === 0 && !loading ? (
          <div className="text-center text-hx-text-tertiary text-[13px] mt-12">
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
                className={`p-3 rounded-hx-radius-md border transition-all duration-150 ${
                  isActive ? 'border-hx-purple bg-hx-purple-bg' : 'border-hx-border bg-hx-bg-panel'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    {isCompleted ? <CheckCircle2 className="w-4 h-4 text-hx-green" /> :
                     isFailed ? <XCircle className="w-4 h-4 text-hx-red" /> :
                     <RefreshCw className="w-4 h-4 text-hx-purple animate-spin" />}
                    <span className="font-medium text-[13px] text-hx-text-primary">{run.sop_name}</span>
                  </div>
                  <span className="text-[10px] text-hx-text-tertiary font-mono">
                    {run.run_id.slice(0, 8)}
                  </span>
                </div>
                
                <div className="flex items-center gap-4 text-xs text-hx-text-secondary">
                  <span>状态: 
                    <span className={`ml-1 ${isActive ? 'text-hx-purple' : isCompleted ? 'text-hx-green' : 'text-hx-red'}`}>
                      {run.status}
                    </span>
                  </span>
                  <span>进度: {Math.min(run.current_step, run.total_steps)}/{run.total_steps}</span>
                  {run.llm_calls_saved > 0 && (
                    <span className="flex items-center gap-1 text-hx-amber" title="节省了的 LLM 推理次数">
                      <Zap className="w-3 h-3" />
                      节约: {run.llm_calls_saved}
                    </span>
                  )}
                </div>
                
                <div className="mt-2 text-[10px] text-hx-text-tertiary flex justify-between">
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
