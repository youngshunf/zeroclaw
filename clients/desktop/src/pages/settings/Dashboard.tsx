import { useEffect, useState } from 'react';
import {
  Activity,
  ChevronDown,
  Clock3,
  Cpu,
  Database,
  DollarSign,
  Globe2,
  Radio,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import type { CostSummary, StatusResponse } from '@/types/api';
import { getCost, getStatus } from '@/lib/api';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

type SectionKey = 'cost' | 'channels' | 'health';

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  // Optional: Could be moved to i18n but 'd', 'h', 'm' works globally well. Using basic ones for now.
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatCNY(value: number): string {
  return `¥${value.toFixed(4)}`;
}

function healthDotClass(status: string): string {
  switch (status.toLowerCase()) {
    case 'ok':
    case 'healthy':
      return 'active';
    default:
      return 'inactive';
  }
}

export default function Dashboard() {
  const { locale } = useLocaleContext(); // For re-render
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [cost, setCost] = useState<CostSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sectionsOpen, setSectionsOpen] = useState<Record<SectionKey, boolean>>({
    cost: true,
    channels: true,
    health: true,
  });

  useEffect(() => {
    Promise.all([getStatus(), getCost()])
      .then(([s, c]) => {
        setStatus(s);
        setCost(c);
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : t('dash.load_fail'));
      });
  }, []);

  const toggle = (key: SectionKey) =>
    setSectionsOpen((prev) => ({ ...prev, [key]: !prev[key] }));

  if (error) {
    return (
      <div className="hx-error-card">
        <h2>{t('dash.fail_title')}</h2>
        <p>{error}</p>
      </div>
    );
  }

  if (!status || !cost) {
    return (
      <div className="hx-loading-center">
        <div className="hx-spinner" />
      </div>
    );
  }

  const maxCost = Math.max(cost.session_cost_usd, cost.daily_cost_usd, cost.monthly_cost_usd, 0.001);

  return (
    <div>
      {/* Hero header */}
      <div className="hx-page-hero">
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'wrap', gap: 12 }}>
          <div>
            <p style={{ fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.15em', opacity: 0.8 }}>
              {t('dash.subtitle')}
            </p>
            <h1>{t('dashboard.title') || '系统仪表盘'}</h1>
            <p>{t('dash.desc')}</p>
          </div>
          <div className="hx-hero-badges">
            <span className="hx-badge">
              <Sparkles size={14} />
              {t('dash.running')}
            </span>
            <span className="hx-badge">
              <ShieldCheck size={14} />
              {status.paired ? t('dash.paired') || t('dashboard.paired') || '已配对' : t('dash.unpaired')}
            </span>
          </div>
        </div>
      </div>

      {/* Metrics row */}
      <div className="hx-metrics-grid">
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Cpu />
            <span>{t('dashboard.provider') || 'Model / Provider'}</span>
          </div>
          <div className="hx-metric-value">{status.provider ?? t('dash.unknown')}</div>
          <div className="hx-metric-sub">{status.model}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Clock3 />
            <span>{t('dashboard.uptime') || 'Uptime'}</span>
          </div>
          <div className="hx-metric-value">{formatUptime(status.uptime_seconds)}</div>
          <div className="hx-metric-sub">{t('dash.since_restart')}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Globe2 />
            <span>{t('dashboard.gateway_port') || 'Gateway Port'}</span>
          </div>
          <div className="hx-metric-value">:{status.gateway_port}</div>
          <div className="hx-metric-sub">{status.locale}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Database />
            <span>{t('dashboard.memory_backend') || 'Memory Backend'}</span>
          </div>
          <div className="hx-metric-value" style={{ textTransform: 'capitalize' }}>
            {status.memory_backend}
          </div>
          <div className="hx-metric-sub">{status.paired ? t('dash.device_paired') : t('dash.no_paired_device')}</div>
        </div>
      </div>

      {/* Cost section */}
      <div className="hx-card">
        <div className="hx-card-header" onClick={() => toggle('cost')}>
          <div className="hx-card-title">
            <div className="hx-card-icon"><DollarSign size={18} /></div>
            <div>
              <h2>{t('dash.cost_title')}</h2>
              <div className="hx-card-subtitle">{t('dash.cost_subtitle')}</div>
            </div>
          </div>
          <ChevronDown
            size={18}
            style={{
              color: 'var(--hx-text-tertiary)',
              transition: 'transform 0.2s',
              transform: sectionsOpen.cost ? 'rotate(180deg)' : 'rotate(0)',
            }}
          />
        </div>
        {sectionsOpen.cost && (
          <div>
            {[
              { label: t('dash.session'), value: cost.session_cost_usd },
              { label: t('dash.today'), value: cost.daily_cost_usd },
              { label: t('dash.this_month'), value: cost.monthly_cost_usd },
            ].map(({ label, value }) => (
              <div key={label} style={{ marginBottom: 14 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13, marginBottom: 6 }}>
                  <span style={{ color: 'var(--hx-text-secondary)' }}>{label}</span>
                  <span style={{ fontWeight: 600, color: 'var(--hx-text-primary)' }}>{formatCNY(value)}</span>
                </div>
                <div className="hx-progress-bar">
                  <div className="hx-progress-fill" style={{ width: `${Math.max((value / maxCost) * 100, 3)}%` }} />
                </div>
              </div>
            ))}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginTop: 8 }}>
              <div className="hx-stat-pill">
                <span>{t('dash.total_tokens')}</span>
                <strong>{cost.total_tokens.toLocaleString()}</strong>
              </div>
              <div className="hx-stat-pill">
                <span>{t('dash.request_count')}</span>
                <strong>{cost.request_count.toLocaleString()}</strong>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Channels section */}
      <div className="hx-card">
        <div className="hx-card-header" onClick={() => toggle('channels')}>
          <div className="hx-card-title">
            <div className="hx-card-icon"><Radio size={18} /></div>
            <div>
              <h2>{t('dash.channels_title')}</h2>
              <div className="hx-card-subtitle">{t('dash.channels_subtitle')}</div>
            </div>
          </div>
          <ChevronDown
            size={18}
            style={{
              color: 'var(--hx-text-tertiary)',
              transition: 'transform 0.2s',
              transform: sectionsOpen.channels ? 'rotate(180deg)' : 'rotate(0)',
            }}
          />
        </div>
        {sectionsOpen.channels && (
          Object.entries(status.channels).length === 0 ? (
            <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>{t('dash.no_channels')}</p>
          ) : (
            <div className="hx-item-grid">
              {Object.entries(status.channels).map(([name, active]) => (
                <div key={name} className="hx-item-cell">
                  <span className="hx-item-name">{name}</span>
                  <span className="hx-status-dot">
                    <span className={`dot ${active ? 'active' : 'inactive'}`} />
                    {active ? t('dash.connected') : t('dash.disconnected')}
                  </span>
                </div>
              ))}
            </div>
          )
        )}
      </div>

      {/* Health section */}
      <div className="hx-card">
        <div className="hx-card-header" onClick={() => toggle('health')}>
          <div className="hx-card-title">
            <div className="hx-card-icon"><Activity size={18} /></div>
            <div>
              <h2>{t('dash.health_title')}</h2>
              <div className="hx-card-subtitle">{t('dash.health_subtitle')}</div>
            </div>
          </div>
          <ChevronDown
            size={18}
            style={{
              color: 'var(--hx-text-tertiary)',
              transition: 'transform 0.2s',
              transform: sectionsOpen.health ? 'rotate(180deg)' : 'rotate(0)',
            }}
          />
        </div>
        {sectionsOpen.health && (
          Object.entries(status.health.components).length === 0 ? (
            <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>{t('dash.no_health_data')}</p>
          ) : (
            <div className="hx-health-grid">
              {Object.entries(status.health.components).map(([name, component]) => (
                <div key={name} className="hx-health-item">
                  <div className="hx-health-top">
                    <span className="hx-health-name">{name}</span>
                    <span className={`dot ${healthDotClass(component.status)}`}
                      style={{ width: 8, height: 8, borderRadius: '50%', display: 'inline-block',
                        background: healthDotClass(component.status) === 'active' ? 'var(--hx-green)' : 'var(--hx-text-tertiary)',
                      }}
                    />
                  </div>
                  <div className="hx-health-status">{component.status}</div>
                  {component.restart_count > 0 && (
                    <div style={{ marginTop: 6, fontSize: 12, color: '#D97706' }}>
                      {t('health.restart_count') || 'Restarts'}: {component.restart_count}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )
        )}
      </div>
    </div>
  );
}
