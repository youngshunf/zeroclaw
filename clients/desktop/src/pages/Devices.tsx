import { useEffect, useState } from 'react';
import { Smartphone, RefreshCw, ShieldX } from 'lucide-react';
import type { PairedDevice } from '@/types/api';
import { getPairedDevices, revokePairedDevice } from '@/lib/api';

function formatDate(value: string | null): string {
  if (!value) return '未知';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN');
}

export default function Devices() {
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingRevoke, setPendingRevoke] = useState<string | null>(null);

  const loadDevices = async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true); else setLoading(true);
    setError(null);
    try { setDevices(await getPairedDevices()); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : '加载设备列表失败'); }
    finally { if (isRefresh) setRefreshing(false); else setLoading(false); }
  };

  useEffect(() => { void loadDevices(false); }, []);

  const handleRevoke = async (id: string) => {
    try { await revokePairedDevice(id); setDevices(prev => prev.filter(d => d.id !== id)); setPendingRevoke(null); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : '撤销设备失败'); setPendingRevoke(null); }
  };

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Smartphone size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
            已配对设备 ({devices.length})
          </h2>
        </div>
        <button
          onClick={() => void loadDevices(true)}
          disabled={refreshing}
          style={{
            display: 'flex', alignItems: 'center', gap: 6,
            background: 'var(--hx-purple)', color: 'white',
            fontSize: 13, fontWeight: 500, padding: '8px 16px',
            borderRadius: 8, border: 'none', cursor: refreshing ? 'not-allowed' : 'pointer',
            opacity: refreshing ? 0.6 : 1,
          }}
        >
          <RefreshCw size={14} style={{ animation: refreshing ? 'hx-spin 0.8s linear infinite' : 'none' }} />
          刷新
        </button>
      </div>

      {error && (
        <div className="hx-error-card" style={{ marginBottom: 16 }}><p>{error}</p></div>
      )}

      {loading ? (
        <div className="hx-loading-center"><div className="hx-spinner" /></div>
      ) : devices.length === 0 ? (
        <div className="hx-card" style={{ textAlign: 'center', padding: '40px 20px' }}>
          <ShieldX size={40} style={{ color: 'var(--hx-text-tertiary)', margin: '0 auto 12px' }} />
          <p style={{ color: 'var(--hx-text-tertiary)', fontSize: 13 }}>暂无配对设备</p>
        </div>
      ) : (
        <div className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
          <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                {['设备 ID', '配对方式', '创建时间', '最后在线', '操作'].map(h => (
                  <th key={h} style={{ textAlign: h === '操作' ? 'right' : 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {devices.map(device => (
                <tr key={device.id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                  <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11, color: 'var(--hx-text-primary)' }}>{device.token_fingerprint}</td>
                  <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)' }}>{device.paired_by ?? '未知'}</td>
                  <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)', whiteSpace: 'nowrap' }}>{formatDate(device.created_at)}</td>
                  <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)', whiteSpace: 'nowrap' }}>{formatDate(device.last_seen_at)}</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                    {pendingRevoke === device.id ? (
                      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                        <span style={{ fontSize: 12, color: '#DC2626' }}>确认撤销？</span>
                        <button onClick={() => void handleRevoke(device.id)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>是</button>
                        <button onClick={() => setPendingRevoke(null)} style={{ fontSize: 12, fontWeight: 500, color: 'var(--hx-text-tertiary)', background: 'none', border: 'none', cursor: 'pointer' }}>否</button>
                      </span>
                    ) : (
                      <button onClick={() => setPendingRevoke(device.id)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>
                        撤销
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
