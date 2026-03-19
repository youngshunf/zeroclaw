import { useState } from 'react';
import { Stethoscope, Play, CheckCircle, AlertTriangle, XCircle, Loader2 } from 'lucide-react';
import type { DiagResult } from '@/types/api';
import { runDoctor } from '@/lib/api';

function severityStyle(severity: DiagResult['severity']): { icon: typeof CheckCircle; color: string; bg: string; border: string } {
  switch (severity) {
    case 'ok': return { icon: CheckCircle, color: 'var(--hx-green)', bg: '#F0FDF4', border: '#BBF7D0' };
    case 'warn': return { icon: AlertTriangle, color: '#D97706', bg: '#FFFBEB', border: '#FDE68A' };
    case 'error': return { icon: XCircle, color: '#DC2626', bg: '#FEF2F2', border: '#FECACA' };
  }
}

export default function Doctor() {
  const [results, setResults] = useState<DiagResult[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRun = async () => {
    setLoading(true); setError(null); setResults(null);
    try { setResults(await runDoctor()); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : '诊断失败'); }
    finally { setLoading(false); }
  };

  const okCount = results?.filter(r => r.severity === 'ok').length ?? 0;
  const warnCount = results?.filter(r => r.severity === 'warn').length ?? 0;
  const errorCount = results?.filter(r => r.severity === 'error').length ?? 0;

  const grouped = results?.reduce<Record<string, DiagResult[]>>((acc, item) => {
    if (!acc[item.category]) acc[item.category] = [];
    acc[item.category].push(item);
    return acc;
  }, {}) ?? {};

  return (
    <div>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Stethoscope size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>系统诊断</h2>
        </div>
        <button
          onClick={handleRun} disabled={loading}
          style={{
            display: 'flex', alignItems: 'center', gap: 6,
            background: 'var(--hx-purple)', color: 'white',
            fontSize: 13, fontWeight: 500, padding: '8px 16px',
            borderRadius: 8, border: 'none', cursor: loading ? 'not-allowed' : 'pointer',
            opacity: loading ? 0.5 : 1,
          }}
        >
          {loading ? <><Loader2 size={14} style={{ animation: 'hx-spin 0.8s linear infinite' }} />诊断中...</> : <><Play size={14} />运行诊断</>}
        </button>
      </div>

      {error && <div className="hx-error-card" style={{ marginBottom: 16 }}><p>{error}</p></div>}

      {loading && (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '60px 0' }}>
          <div className="hx-spinner" style={{ marginBottom: 16 }} />
          <p style={{ color: 'var(--hx-text-secondary)', fontSize: 14 }}>正在运行诊断...</p>
          <p style={{ color: 'var(--hx-text-tertiary)', fontSize: 12, marginTop: 4 }}>可能需要几秒钟</p>
        </div>
      )}

      {results && !loading && (
        <>
          {/* Summary */}
          <div className="hx-card" style={{ display: 'flex', alignItems: 'center', gap: 16, flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <CheckCircle size={18} style={{ color: 'var(--hx-green)' }} />
              <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--hx-text-primary)' }}>{okCount} <span style={{ fontWeight: 400, color: 'var(--hx-text-tertiary)' }}>正常</span></span>
            </div>
            <div style={{ width: 1, height: 20, background: 'var(--hx-border)' }} />
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <AlertTriangle size={18} style={{ color: '#D97706' }} />
              <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--hx-text-primary)' }}>{warnCount} <span style={{ fontWeight: 400, color: 'var(--hx-text-tertiary)' }}>警告</span></span>
            </div>
            <div style={{ width: 1, height: 20, background: 'var(--hx-border)' }} />
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <XCircle size={18} style={{ color: '#DC2626' }} />
              <span style={{ fontSize: 13, fontWeight: 500, color: 'var(--hx-text-primary)' }}>{errorCount} <span style={{ fontWeight: 400, color: 'var(--hx-text-tertiary)' }}>错误</span></span>
            </div>
            <div style={{ marginLeft: 'auto' }}>
              {errorCount > 0 ? (
                <span style={{ padding: '4px 12px', borderRadius: 99, fontSize: 12, fontWeight: 500, background: '#FEF2F2', color: '#DC2626', border: '1px solid #FECACA' }}>发现问题</span>
              ) : warnCount > 0 ? (
                <span style={{ padding: '4px 12px', borderRadius: 99, fontSize: 12, fontWeight: 500, background: '#FFFBEB', color: '#D97706', border: '1px solid #FDE68A' }}>有警告</span>
              ) : (
                <span style={{ padding: '4px 12px', borderRadius: 99, fontSize: 12, fontWeight: 500, background: '#F0FDF4', color: '#16A34A', border: '1px solid #BBF7D0' }}>全部正常</span>
              )}
            </div>
          </div>

          {/* Grouped results */}
          {Object.entries(grouped).sort(([a], [b]) => a.localeCompare(b)).map(([category, items]) => (
            <div key={category} style={{ marginBottom: 20 }}>
              <h3 style={{ fontSize: 12, fontWeight: 600, color: 'var(--hx-text-tertiary)', textTransform: 'capitalize', letterSpacing: '0.05em', marginBottom: 10 }}>{category}</h3>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {items.map((result, idx) => {
                  const s = severityStyle(result.severity);
                  const Icon = s.icon;
                  return (
                    <div key={`${category}-${idx}`} style={{ display: 'flex', alignItems: 'flex-start', gap: 10, borderRadius: 10, border: `1px solid ${s.border}`, background: s.bg, padding: 12 }}>
                      <Icon size={16} style={{ color: s.color, flexShrink: 0, marginTop: 1 }} />
                      <div>
                        <p style={{ fontSize: 13, color: 'var(--hx-text-primary)' }}>{result.message}</p>
                        <p style={{ fontSize: 11, color: 'var(--hx-text-tertiary)', marginTop: 2, textTransform: 'capitalize' }}>{result.severity}</p>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </>
      )}

      {!results && !loading && !error && (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '60px 0' }}>
          <Stethoscope size={48} style={{ color: 'var(--hx-text-tertiary)', marginBottom: 16 }} />
          <p style={{ fontSize: 16, fontWeight: 500, color: 'var(--hx-text-secondary)' }}>系统诊断</p>
          <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)', marginTop: 4 }}>点击"运行诊断"检查系统状态</p>
        </div>
      )}
    </div>
  );
}
