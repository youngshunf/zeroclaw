import { useState, useEffect } from 'react';
import { DollarSign, TrendingUp, Hash, Layers } from 'lucide-react';
import type { CostSummary } from '@/types/api';
import { getCost } from '@/lib/api';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

function formatCNY(value: number): string {
  return `¥${value.toFixed(4)}`;
}

export default function Cost() {
  const [cost, setCost] = useState<CostSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { locale } = useLocaleContext();

  useEffect(() => {
    getCost()
      .then(setCost)
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  if (error) {
    return <div className="hx-error-card"><h2>{t('tools.load_failed')}</h2><p>{error}</p></div>;
  }
  if (loading || !cost) {
    return <div className="hx-loading-center"><div className="hx-spinner" /></div>;
  }

  const models = Object.values(cost.by_model);

  return (
    <div>
      {/* Summary Cards */}
      <div className="hx-metrics-grid">
        <div className="hx-metric-card">
          <div className="hx-metric-head"><DollarSign /><span>{t('cost_extra.session')}</span></div>
          <div className="hx-metric-value">{formatCNY(cost.session_cost_usd)}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head"><TrendingUp /><span>{t('cost_extra.daily')}</span></div>
          <div className="hx-metric-value">{formatCNY(cost.daily_cost_usd)}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head"><Layers /><span>{t('cost_extra.monthly')}</span></div>
          <div className="hx-metric-value">{formatCNY(cost.monthly_cost_usd)}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head"><Hash /><span>{t('cost_extra.requests')}</span></div>
          <div className="hx-metric-value">{cost.request_count.toLocaleString()}</div>
        </div>
      </div>

      {/* Token Statistics */}
      <div className="hx-card">
        <h3 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 16 }}>
          {t('cost_extra.token_stats')}
        </h3>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 10 }}>
          <div className="hx-stat-pill">
            <span>{t('cost_extra.total_tokens')}</span>
            <strong>{cost.total_tokens.toLocaleString()}</strong>
          </div>
          <div className="hx-stat-pill">
            <span>{t('cost_extra.avg_tokens')}</span>
            <strong>{cost.request_count > 0 ? Math.round(cost.total_tokens / cost.request_count).toLocaleString() : '0'}</strong>
          </div>
          <div className="hx-stat-pill">
            <span>{t('cost_extra.cost_per_1k')}</span>
            <strong>{cost.total_tokens > 0 ? formatCNY((cost.monthly_cost_usd / cost.total_tokens) * 1000) : '¥0.0000'}</strong>
          </div>
        </div>
      </div>

      {/* Model Breakdown */}
      <div className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
        <div style={{ padding: '16px 20px', borderBottom: '1px solid var(--hx-border)' }}>
          <h3 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{t('cost_extra.model_breakdown')}</h3>
        </div>
        {models.length === 0 ? (
          <div style={{ padding: 32, textAlign: 'center', color: 'var(--hx-text-tertiary)', fontSize: 13 }}>
            {t('cost_extra.no_data')}
          </div>
        ) : (
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                  <th style={{ textAlign: 'left', padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{t('cost_extra.th_model')}</th>
                  <th style={{ textAlign: 'right', padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{t('cost_extra.th_cost')}</th>
                  <th style={{ textAlign: 'right', padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{t('cost_extra.th_tokens')}</th>
                  <th style={{ textAlign: 'right', padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{t('cost_extra.th_requests')}</th>
                  <th style={{ textAlign: 'left', padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{t('cost_extra.th_share')}</th>
                </tr>
              </thead>
              <tbody>
                {models.sort((a, b) => b.cost_usd - a.cost_usd).map((m) => {
                  const share = cost.monthly_cost_usd > 0 ? (m.cost_usd / cost.monthly_cost_usd) * 100 : 0;
                  return (
                    <tr key={m.model} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                      <td style={{ padding: '10px 20px', fontWeight: 500, color: 'var(--hx-text-primary)' }}>{m.model}</td>
                      <td style={{ padding: '10px 20px', textAlign: 'right', color: 'var(--hx-text-secondary)', fontFamily: 'monospace' }}>{formatCNY(m.cost_usd)}</td>
                      <td style={{ padding: '10px 20px', textAlign: 'right', color: 'var(--hx-text-secondary)' }}>{m.total_tokens.toLocaleString()}</td>
                      <td style={{ padding: '10px 20px', textAlign: 'right', color: 'var(--hx-text-secondary)' }}>{m.request_count.toLocaleString()}</td>
                      <td style={{ padding: '10px 20px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div className="hx-progress-bar" style={{ width: 80 }}>
                            <div className="hx-progress-fill" style={{ width: `${Math.max(share, 2)}%` }} />
                          </div>
                          <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', width: 40, textAlign: 'right' }}>
                            {share.toFixed(1)}%
                          </span>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
