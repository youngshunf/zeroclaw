import { useEffect, useState } from 'react';
import { X, CheckCircle2, XCircle, AlertCircle, Clock } from 'lucide-react';
import { CronRun, getCronRuns } from '@/lib/cron-api';

interface TaskRunsDrawerProps {
  jobId: string | null;
  jobName: string | null;
  open: boolean;
  onClose: () => void;
}

export function TaskRunsDrawer({ jobId, jobName, open, onClose }: TaskRunsDrawerProps) {
  const [runs, setRuns] = useState<CronRun[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open && jobId) {
      setLoading(true);
      getCronRuns(jobId)
        .then(setRuns)
        .catch(console.error)
        .finally(() => setLoading(false));
    }
  }, [open, jobId]);

  if (!open) return null;

  return (
    <>
      <div 
        className="fixed inset-0 bg-black/20 backdrop-blur-sm z-40 animate-in fade-in duration-200"
        onClick={onClose}
      />
      
      <div className="fixed top-0 right-0 bottom-0 w-[450px] max-w-[90vw] bg-hx-bg-main border-l border-hx-border shadow-2xl z-50 flex flex-col animate-in slide-in-from-right duration-300">
        <div className="flex items-center justify-between px-6 py-4 border-b border-hx-border bg-hx-bg-panel/50">
          <div>
            <h2 className="text-base font-bold text-hx-text-primary">运行日志</h2>
            <p className="text-xs text-hx-text-secondary mt-0.5 max-w-[300px] truncate">
              {jobName || '未命名任务'}
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-hx-text-tertiary hover:text-hx-text-primary hover:bg-hx-bg-input rounded-md transition-colors bg-transparent border-none cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 bg-hx-bg-main">
          {loading ? (
            <div className="text-center py-10 text-hx-text-tertiary">
              加载中...
            </div>
          ) : runs.length === 0 ? (
            <div className="text-center py-12 flex flex-col items-center">
              <Clock className="w-10 h-10 text-hx-text-tertiary opacity-30 mb-3" />
              <p className="text-sm text-hx-text-tertiary">暂无运行记录</p>
            </div>
          ) : (
            <div className="space-y-4">
              {runs.map((run) => (
                <div key={run.id} className="bg-hx-bg-panel border border-hx-border rounded-lg overflow-hidden shadow-sm">
                  <div className="px-4 py-2 border-b border-hx-border bg-hx-bg-input/30 flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      {run.status === 'success' ? (
                        <CheckCircle2 size={14} className="text-emerald-500" />
                      ) : run.status === 'failed' ? (
                        <XCircle size={14} className="text-red-500" />
                      ) : (
                        <AlertCircle size={14} className="text-amber-500" />
                      )}
                      <span className="text-xs font-medium text-hx-text-primary capitalize">
                        {run.status}
                      </span>
                    </div>
                    <span className="text-xs text-hx-text-tertiary">
                      {new Date(run.started_at).toLocaleString('zh-CN')}
                    </span>
                  </div>
                  
                  <div className="p-4">
                    <div className="text-xs text-hx-text-secondary font-mono bg-black/5 dark:bg-black/20 p-2.5 rounded border border-hx-border/50 max-h-40 overflow-y-auto whitespace-pre-wrap leading-relaxed">
                      {run.output || '无输出日志'}
                    </div>
                    {run.duration_ms !== null && (
                      <div className="mt-2 text-[10px] text-hx-text-tertiary text-right">
                        耗时: {run.duration_ms} ms
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </>
  );
}
