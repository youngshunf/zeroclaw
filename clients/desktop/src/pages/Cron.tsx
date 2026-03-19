import { useState, useEffect } from 'react';
import { Clock, Plus, Trash2, X, CheckCircle, XCircle, AlertCircle } from 'lucide-react';
import type { CronJob } from '@/types/api';
import { getCronJobs, addCronJob, deleteCronJob } from '@/lib/api';

function formatDate(iso: string | null): string {
  if (!iso) return '-';
  return new Date(iso).toLocaleString('zh-CN');
}

const inputStyle: React.CSSProperties = {
  width: '100%', background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)',
  borderRadius: 8, padding: '8px 12px', fontSize: 13, color: 'var(--hx-text-primary)', outline: 'none',
};

function StatusIcon({ status }: { status: string | null }) {
  if (!status) return null;
  switch (status.toLowerCase()) {
    case 'ok': case 'success': return <CheckCircle size={14} style={{ color: 'var(--hx-green)' }} />;
    case 'error': case 'failed': return <XCircle size={14} style={{ color: '#DC2626' }} />;
    default: return <AlertCircle size={14} style={{ color: '#D97706' }} />;
  }
}

export default function Cron() {
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [formName, setFormName] = useState('');
  const [formSchedule, setFormSchedule] = useState('');
  const [formCommand, setFormCommand] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const fetchJobs = () => { setLoading(true); getCronJobs().then(setJobs).catch(err => setError(err.message)).finally(() => setLoading(false)); };
  useEffect(() => { fetchJobs(); }, []);

  const handleAdd = async () => {
    if (!formSchedule.trim() || !formCommand.trim()) { setFormError('调度表达式和命令为必填项'); return; }
    setSubmitting(true); setFormError(null);
    try { const job = await addCronJob({ name: formName.trim() || undefined, schedule: formSchedule.trim(), command: formCommand.trim() }); setJobs(prev => [...prev, job]); setShowForm(false); setFormName(''); setFormSchedule(''); setFormCommand(''); }
    catch (err: unknown) { setFormError(err instanceof Error ? err.message : '添加任务失败'); }
    finally { setSubmitting(false); }
  };

  const handleDelete = async (id: string) => {
    try { await deleteCronJob(id); setJobs(prev => prev.filter(j => j.id !== id)); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : '删除失败'); }
    finally { setConfirmDelete(null); }
  };

  if (error && jobs.length === 0) return <div className="hx-error-card"><h2>加载失败</h2><p>{error}</p></div>;
  if (loading) return <div className="hx-loading-center"><div className="hx-spinner" /></div>;

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Clock size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>定时任务 ({jobs.length})</h2>
        </div>
        <button onClick={() => setShowForm(true)} style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'var(--hx-purple)', color: 'white',
          fontSize: 13, fontWeight: 500, padding: '8px 16px', borderRadius: 8, border: 'none', cursor: 'pointer',
        }}>
          <Plus size={14} />添加任务
        </button>
      </div>

      {showForm && (
        <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 50 }}>
          <div style={{ background: 'var(--hx-bg-main)', border: '1px solid var(--hx-border)', borderRadius: 16, padding: 24, width: '100%', maxWidth: 440, margin: '0 16px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
              <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)' }}>添加定时任务</h3>
              <button onClick={() => { setShowForm(false); setFormError(null); }} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-text-tertiary)' }}><X size={18} /></button>
            </div>
            {formError && <div className="hx-error-card" style={{ marginBottom: 12, padding: 10 }}><p style={{ fontSize: 12 }}>{formError}</p></div>}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>名称（可选）</label>
                <input type="text" value={formName} onChange={e => setFormName(e.target.value)} placeholder="如 每日清理" style={inputStyle} />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>调度表达式 <span style={{ color: '#DC2626' }}>*</span></label>
                <input type="text" value={formSchedule} onChange={e => setFormSchedule(e.target.value)} placeholder="如 0 0 * * * (cron 表达式)" style={inputStyle} />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>命令 <span style={{ color: '#DC2626' }}>*</span></label>
                <input type="text" value={formCommand} onChange={e => setFormCommand(e.target.value)} placeholder="如 cleanup --older-than 7d" style={inputStyle} />
              </div>
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, marginTop: 20 }}>
              <button onClick={() => { setShowForm(false); setFormError(null); }} style={{ padding: '8px 16px', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', border: '1px solid var(--hx-border)', borderRadius: 8, background: 'transparent', cursor: 'pointer' }}>取消</button>
              <button onClick={handleAdd} disabled={submitting} style={{ padding: '8px 16px', fontSize: 13, fontWeight: 500, color: 'white', background: 'var(--hx-purple)', borderRadius: 8, border: 'none', cursor: submitting ? 'not-allowed' : 'pointer', opacity: submitting ? 0.5 : 1 }}>{submitting ? '添加中...' : '添加'}</button>
            </div>
          </div>
        </div>
      )}

      {jobs.length === 0 ? (
        <div className="hx-card" style={{ textAlign: 'center', padding: '40px 20px' }}>
          <Clock size={40} style={{ color: 'var(--hx-text-tertiary)', margin: '0 auto 12px' }} />
          <p style={{ color: 'var(--hx-text-tertiary)', fontSize: 13 }}>暂无定时任务</p>
        </div>
      ) : (
        <div className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
          <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                {['ID', '名称', '命令', '下次运行', '状态', '启用', '操作'].map(h => (
                  <th key={h} style={{ textAlign: h === '操作' ? 'right' : 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {jobs.map(job => (
                <tr key={job.id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                  <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11, color: 'var(--hx-text-tertiary)' }}>{job.id.slice(0, 8)}</td>
                  <td style={{ padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-primary)' }}>{job.name ?? '-'}</td>
                  <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11, color: 'var(--hx-text-secondary)', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis' }}>{job.command}</td>
                  <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)' }}>{formatDate(job.next_run)}</td>
                  <td style={{ padding: '10px 16px' }}>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                      <StatusIcon status={job.last_status} />
                      <span style={{ fontSize: 12, color: 'var(--hx-text-secondary)', textTransform: 'capitalize' }}>{job.last_status ?? '-'}</span>
                    </span>
                  </td>
                  <td style={{ padding: '10px 16px' }}>
                    <span style={{
                      display: 'inline-flex', padding: '2px 10px', borderRadius: 99, fontSize: 11, fontWeight: 500,
                      background: job.enabled ? '#F0FDF4' : 'var(--hx-bg-panel)',
                      color: job.enabled ? '#16A34A' : 'var(--hx-text-tertiary)',
                      border: `1px solid ${job.enabled ? '#BBF7D0' : 'var(--hx-border)'}`,
                    }}>{job.enabled ? '已启用' : '已禁用'}</span>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                    {confirmDelete === job.id ? (
                      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                        <span style={{ fontSize: 12, color: '#DC2626' }}>删除？</span>
                        <button onClick={() => handleDelete(job.id)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>是</button>
                        <button onClick={() => setConfirmDelete(null)} style={{ fontSize: 12, fontWeight: 500, color: 'var(--hx-text-tertiary)', background: 'none', border: 'none', cursor: 'pointer' }}>否</button>
                      </span>
                    ) : (
                      <button onClick={() => setConfirmDelete(job.id)} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-text-tertiary)' }}>
                        <Trash2 size={14} />
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
