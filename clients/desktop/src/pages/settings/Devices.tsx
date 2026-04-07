import { useEffect, useState } from 'react';
import { Smartphone, RefreshCw, ShieldX, KeyRound, Copy, RotateCcw } from 'lucide-react';
import type { PairedDevice } from '@/types/api';
import { getPairedDevices, revokePairedDevice } from '@/lib/api';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';
import {
  getMyNodes,
  reissueMyNodeKey,
  getOwnerApiKeys,
  createOwnerApiKey,
  deleteOwnerApiKey,
  type HasnNodeInfo,
  type OwnerApiKeyInfo,
} from '@/lib/hasn-api';

function formatDate(value: string | null): string {
  if (!value) return t('devices.unknown');
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export default function Devices() {
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const [nodes, setNodes] = useState<HasnNodeInfo[]>([]);
  const [ownerKeys, setOwnerKeys] = useState<OwnerApiKeyInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingRevoke, setPendingRevoke] = useState<string | null>(null);
  const [creatingOwnerKey, setCreatingOwnerKey] = useState(false);
  const [newOwnerKeyName, setNewOwnerKeyName] = useState('');
  const [latestNodeKey, setLatestNodeKey] = useState<{ nodeId: string; nodeKey: string } | null>(null);
  const [latestOwnerKey, setLatestOwnerKey] = useState<{ keyId: string; key: string } | null>(null);
  const { locale } = useLocaleContext();

  const loadDevices = async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true); else setLoading(true);
    setError(null);
    try {
      const [paired, myNodes, myOwnerKeys] = await Promise.all([
        getPairedDevices(),
        getMyNodes(),
        getOwnerApiKeys(),
      ]);
      setDevices(paired);
      setNodes(myNodes);
      setOwnerKeys(myOwnerKeys);
    }
    catch (err: unknown) { setError(err instanceof Error ? err.message : t('devices.load_failed')); }
    finally { if (isRefresh) setRefreshing(false); else setLoading(false); }
  };

  useEffect(() => { void loadDevices(false); }, []);

  const handleRevoke = async (id: string) => {
    try { await revokePairedDevice(id); setDevices(prev => prev.filter(d => d.id !== id)); setPendingRevoke(null); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : t('devices.revoke_failed')); setPendingRevoke(null); }
  };

  const handleCopy = async (text: string, successMessage: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setError(successMessage);
      setTimeout(() => setError(null), 1800);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '复制失败');
    }
  };

  const handleReissueNodeKey = async (nodeId: string) => {
    try {
      const result = await reissueMyNodeKey(nodeId);
      setLatestNodeKey({ nodeId: result.node_id, nodeKey: result.node_key });
      await loadDevices(true);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '重新签发 Node Key 失败');
    }
  };

  const handleCreateOwnerKey = async () => {
    if (!newOwnerKeyName.trim()) {
      setError('请输入 API Key 名称');
      return;
    }
    setCreatingOwnerKey(true);
    try {
      const result = await createOwnerApiKey({
        name: newOwnerKeyName.trim(),
      });
      setLatestOwnerKey({ keyId: result.key_id, key: result.owner_api_key });
      setNewOwnerKeyName('');
      await loadDevices(true);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '创建 Owner API Key 失败');
    } finally {
      setCreatingOwnerKey(false);
    }
  };

  const handleDeleteOwnerKey = async (keyId: string) => {
    try {
      await deleteOwnerApiKey(keyId);
      setOwnerKeys(prev => prev.filter(k => k.key_id !== keyId));
      if (latestOwnerKey?.keyId === keyId) setLatestOwnerKey(null);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : '吊销 Owner API Key 失败');
    }
  };

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Smartphone size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
            {t('devices.title')} ({devices.length})
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
          {t('devices.refresh')}
        </button>
      </div>

      {error && (
        <div className="hx-error-card" style={{ marginBottom: 16 }}><p>{error}</p></div>
      )}

      {loading ? (
        <div className="hx-loading-center"><div className="hx-spinner" /></div>
      ) : (
        <div style={{ display: 'grid', gap: 16 }}>
          <section className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
            <div style={{ padding: '14px 16px', borderBottom: '1px solid var(--hx-border)', fontWeight: 600 }}>已配对设备</div>
            {devices.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '32px 16px', color: 'var(--hx-text-tertiary)', fontSize: 13 }}>
                暂无已配对设备
              </div>
            ) : (
              <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
                <thead>
                  <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                    {[
                      { key: 'id', label: t('devices.th_id') },
                      { key: 'paired_by', label: t('devices.th_paired_by') },
                      { key: 'created', label: t('devices.th_created') },
                      { key: 'last_seen', label: t('devices.th_last_seen') },
                      { key: 'actions', label: t('devices.th_actions'), align: 'right' as const },
                    ].map(h => (
                      <th key={h.key} style={{ textAlign: h.align || 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{h.label}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {devices.map(device => (
                    <tr key={device.id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                      <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11, color: 'var(--hx-text-primary)' }}>{device.token_fingerprint}</td>
                      <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)' }}>{device.paired_by ?? t('devices.unknown')}</td>
                      <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)', whiteSpace: 'nowrap' }}>{formatDate(device.created_at)}</td>
                      <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)', whiteSpace: 'nowrap' }}>{formatDate(device.last_seen_at)}</td>
                      <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                        {pendingRevoke === device.id ? (
                          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                            <span style={{ fontSize: 12, color: '#DC2626' }}>{t('devices.confirm_revoke')}</span>
                            <button onClick={() => void handleRevoke(device.id)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>{t('devices.yes')}</button>
                            <button onClick={() => setPendingRevoke(null)} style={{ fontSize: 12, fontWeight: 500, color: 'var(--hx-text-tertiary)', background: 'none', border: 'none', cursor: 'pointer' }}>{t('devices.no')}</button>
                          </span>
                        ) : (
                          <button onClick={() => setPendingRevoke(device.id)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>
                            {t('devices.revoke')}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
            <div style={{ padding: '14px 16px', borderBottom: '1px solid var(--hx-border)', fontWeight: 600, display: 'flex', alignItems: 'center', gap: 8 }}>
              <Smartphone size={16} /> HASN 节点
            </div>
            {latestNodeKey && (
              <div style={{ margin: 16, padding: 12, borderRadius: 10, background: 'rgba(99,102,241,0.08)', border: '1px solid rgba(99,102,241,0.2)' }}>
                <div style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginBottom: 6 }}>最新签发的 Node Key，仅展示一次</div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <code style={{ flex: 1, fontSize: 12, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{latestNodeKey.nodeKey}</code>
                  <button onClick={() => void handleCopy(latestNodeKey.nodeKey, 'Node Key 已复制')} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-purple)' }}>
                    <Copy size={16} />
                  </button>
                </div>
              </div>
            )}
            {nodes.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '32px 16px', color: 'var(--hx-text-tertiary)', fontSize: 13 }}>
                暂无 HASN 节点
              </div>
            ) : (
              <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
                <thead>
                  <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>Node ID</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>类型/平台</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>允许绑定 Owner</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>最近活跃</th>
                    <th style={{ textAlign: 'right', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {nodes.map(node => (
                    <tr key={node.node_id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                      <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11 }}>{node.node_id}</td>
                      <td style={{ padding: '10px 16px' }}>
                        <div>{node.node_type}</div>
                        <div style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>{node.device_platform || 'unknown'}</div>
                      </td>
                      <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)' }}>
                        {node.allowed_owner_hasn_ids?.length ? node.allowed_owner_hasn_ids.join(', ') : '不限'}
                      </td>
                      <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)' }}>{formatDate(node.last_seen_at ?? null)}</td>
                      <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                        <button
                          onClick={() => void handleReissueNodeKey(node.node_id)}
                          style={{ display: 'inline-flex', alignItems: 'center', gap: 6, background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-purple)', fontSize: 12, fontWeight: 500 }}
                        >
                          <RotateCcw size={14} />
                          重签发 Node Key
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
            <div style={{ padding: '14px 16px', borderBottom: '1px solid var(--hx-border)', fontWeight: 600, display: 'flex', alignItems: 'center', gap: 8 }}>
              <KeyRound size={16} /> Owner API Keys
            </div>
            <div style={{ padding: 16, borderBottom: '1px solid var(--hx-border-light)', display: 'flex', gap: 8, alignItems: 'center' }}>
              <input
                value={newOwnerKeyName}
                onChange={(e) => setNewOwnerKeyName(e.target.value)}
                placeholder="例如：OpenClaw 插件 / 办公室电脑"
                style={{
                  flex: 1,
                  background: 'var(--hx-surface-elevated)',
                  border: '1px solid var(--hx-border)',
                  borderRadius: 8,
                  padding: '10px 12px',
                  color: 'var(--hx-text-primary)',
                  fontSize: 13,
                }}
              />
              <button
                onClick={() => void handleCreateOwnerKey()}
                disabled={creatingOwnerKey}
                style={{
                  background: 'var(--hx-purple)',
                  color: 'white',
                  border: 'none',
                  borderRadius: 8,
                  padding: '10px 14px',
                  fontSize: 13,
                  fontWeight: 600,
                  cursor: creatingOwnerKey ? 'not-allowed' : 'pointer',
                  opacity: creatingOwnerKey ? 0.6 : 1,
                }}
              >
                {creatingOwnerKey ? '创建中...' : '创建 Key'}
              </button>
            </div>
            {latestOwnerKey && (
              <div style={{ margin: 16, padding: 12, borderRadius: 10, background: 'rgba(16,185,129,0.08)', border: '1px solid rgba(16,185,129,0.2)' }}>
                <div style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginBottom: 6 }}>新创建的 Owner API Key，仅展示一次</div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                  <code style={{ flex: 1, fontSize: 12, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{latestOwnerKey.key}</code>
                  <button onClick={() => void handleCopy(latestOwnerKey.key, 'Owner API Key 已复制')} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-purple)' }}>
                    <Copy size={16} />
                  </button>
                </div>
              </div>
            )}
            {ownerKeys.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '32px 16px', color: 'var(--hx-text-tertiary)', fontSize: 13 }}>
                暂无 Owner API Key
              </div>
            ) : (
              <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
                <thead>
                  <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>Key ID</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>名称</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>绑定 Node</th>
                    <th style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>状态</th>
                    <th style={{ textAlign: 'right', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {ownerKeys.map(key => (
                    <tr key={key.key_id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                      <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11 }}>{key.key_id}</td>
                      <td style={{ padding: '10px 16px' }}>{key.key_name || '-'}</td>
                      <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)' }}>{key.bound_node_id || '不限'}</td>
                      <td style={{ padding: '10px 16px', color: key.status === 'active' ? '#10B981' : '#DC2626' }}>{key.status}</td>
                      <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                        <button
                          onClick={() => void handleDeleteOwnerKey(key.key_id)}
                          style={{ background: 'none', border: 'none', cursor: 'pointer', color: '#DC2626', fontSize: 12, fontWeight: 500 }}
                        >
                          吊销
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
