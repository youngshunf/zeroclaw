import { useEffect, useState } from 'react';
import {
  CronJob,
  listCronJobs,
  toggleCronJob,
  deleteCronJob,
} from '@/lib/cron-api';
import { PlusIcon, TrashIcon, ListIcon, Clock } from 'lucide-react';
import { TaskEditorModal } from './components/TaskEditorModal';
import { TaskRunsDrawer } from './components/TaskRunsDrawer';

export default function TasksPage() {
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [loading, setLoading] = useState(true);
  const [editorOpen, setEditorOpen] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selectedJob, setSelectedJob] = useState<{id: string, name: string | null} | null>(null);

  const fetchJobs = async () => {
    setLoading(true);
    try {
      const data = await listCronJobs();
      setJobs(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchJobs();
  }, []);

  const handleToggle = async (id: string, enabled: boolean) => {
    await toggleCronJob(id, enabled);
    await fetchJobs();
  };

  const handleDelete = async (id: string) => {
    if (confirm('确定要删除这个定时任务吗？')) {
      await deleteCronJob(id);
      await fetchJobs();
    }
  };

  function formatDate(iso: string | null): string {
    if (!iso) return '-';
    return new Date(iso).toLocaleString('zh-CN');
  }

  return (
    <div className="flex h-full w-full flex-col bg-hx-bg-main min-w-0 text-hx-text-primary">
      <div className="shrink-0 border-b border-hx-border bg-hx-bg-panel pt-5 px-6 pb-3" style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}>
        <div className="flex justify-between items-center" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <div>
            <h1 className="text-xl font-bold tracking-tight flex items-center gap-2">
              <Clock className="w-5 h-5 text-indigo-500" />
              定时调度中心
            </h1>
            <p className="text-hx-text-secondary text-sm mt-1">
              配置并管理由 Guardian 代理自动执行的定时调度计划
            </p>
          </div>
          <button
            onClick={() => setEditorOpen(true)}
            className="flex items-center gap-1.5 px-4 py-2 bg-hx-purple hover:bg-hx-purple-hover text-white text-sm font-medium rounded-lg transition-colors border-none cursor-pointer"
          >
            <PlusIcon strokeWidth={2.5} size={15} />
            新建任务
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
        <div className="rounded-xl border border-hx-border bg-hx-bg-panel shadow-sm overflow-hidden">
          <table className="w-full text-sm text-left">
            <thead className="text-xs text-hx-text-secondary bg-hx-bg-input/50 border-b border-hx-border">
              <tr>
                <th className="px-6 py-4 font-medium">任务名称</th>
                <th className="px-6 py-4 font-medium">执行周期</th>
                <th className="px-6 py-4 font-medium">下次运行时间</th>
                <th className="px-6 py-4 font-medium">状态</th>
                <th className="px-6 py-4 font-medium text-right">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-hx-border">
              {loading ? (
                <tr>
                  <td colSpan={5} className="text-center py-10 text-hx-text-tertiary">
                    加载中...
                  </td>
                </tr>
              ) : jobs.length === 0 ? (
                <tr>
                  <td colSpan={5} className="text-center py-16 text-hx-text-tertiary flex flex-col items-center justify-center">
                    <Clock size={32} className="opacity-30 mb-2 mt-4" />
                    <span>暂无定时任务，点击右上角新建。</span>
                  </td>
                </tr>
              ) : (
                jobs.map((job) => (
                  <tr key={job.id} className="hover:bg-hx-bg-input/30 transition-colors">
                    <td className="px-6 py-4 font-medium text-hx-text-primary w-48">
                      {job.name || '未命名任务'}
                    </td>
                    <td className="px-6 py-4 w-48">
                      <code className="bg-hx-bg-input text-hx-text-secondary px-2 py-1 rounded text-xs font-mono border border-hx-border">
                        {job.expression}
                      </code>
                    </td>
                    <td className="px-6 py-4 text-hx-text-secondary">
                      {formatDate(job.next_run)}
                    </td>
                    <td className="px-6 py-4">
                      {/* CSS 样式的 Switch */}
                      <label className="relative inline-flex items-center cursor-pointer">
                        <input 
                          type="checkbox" 
                          className="sr-only peer" 
                          checked={job.enabled}
                          onChange={(e) => handleToggle(job.id, e.target.checked)}
                        />
                        <div className="w-9 h-5 bg-hx-border rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-emerald-500"></div>
                      </label>
                    </td>
                    <td className="px-6 py-4 text-right space-x-2">
                      <button
                        title="运行日志"
                        onClick={() => {
                          setSelectedJob({ id: job.id, name: job.name });
                          setDrawerOpen(true);
                        }}
                        className="p-1.5 text-hx-text-tertiary hover:text-hx-text-primary rounded-md hover:bg-hx-bg-input transition-colors border-none bg-transparent cursor-pointer"
                      >
                        <ListIcon size={16} />
                      </button>
                      <button
                        onClick={() => handleDelete(job.id)}
                        className="p-1.5 text-hx-text-tertiary hover:text-red-500 rounded-md hover:bg-red-500/10 transition-colors border-none bg-transparent cursor-pointer"
                      >
                        <TrashIcon size={16} />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      <TaskEditorModal
        open={editorOpen}
        onOpenChange={setEditorOpen}
        onSuccess={() => {
          setEditorOpen(false);
          fetchJobs();
        }}
      />

      <TaskRunsDrawer
        jobId={selectedJob?.id || null}
        jobName={selectedJob?.name || null}
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
      />
    </div>
  );
}
