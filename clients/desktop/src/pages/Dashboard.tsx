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

type SectionKey = 'cost' | 'channels' | 'health';

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}天 ${h}小时 ${m}分钟`;
  if (h > 0) return `${h}小时 ${m}分钟`;
  return `${m}分钟`;
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
        setError(err instanceof Error ? err.message : '加载仪表盘失败');
      });
  }, []);

  const toggle = (key: SectionKey) =>
    setSectionsOpen((prev) => ({ ...prev, [key]: !prev[key] }));

  if (error) {
    return (
      <div className="hx-error-card">
        <h2>加载失败</h2>
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
              唤星运行控制台
            </p>
            <h1>系统仪表盘</h1>
            <p>实时运行状态、费用统计、渠道连接状态一览</p>
          </div>
          <div className="hx-hero-badges">
            <span className="hx-badge">
              <Sparkles size={14} />
              运行中
            </span>
            <span className="hx-badge">
              <ShieldCheck size={14} />
              {status.paired ? '已配对' : '未配对'}
            </span>
          </div>
        </div>
      </div>

      {/* Metrics row */}
      <div className="hx-metrics-grid">
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Cpu />
            <span>模型 / 供应商</span>
          </div>
          <div className="hx-metric-value">{status.provider ?? '未知'}</div>
          <div className="hx-metric-sub">{status.model}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Clock3 />
            <span>运行时长</span>
          </div>
          <div className="hx-metric-value">{formatUptime(status.uptime_seconds)}</div>
          <div className="hx-metric-sub">自上次重启</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Globe2 />
            <span>网关端口</span>
          </div>
          <div className="hx-metric-value">:{status.gateway_port}</div>
          <div className="hx-metric-sub">{status.locale}</div>
        </div>
        <div className="hx-metric-card">
          <div className="hx-metric-head">
            <Database />
            <span>记忆后端</span>
          </div>
          <div className="hx-metric-value" style={{ textTransform: 'capitalize' }}>
            {status.memory_backend}
          </div>
          <div className="hx-metric-sub">{status.paired ? '设备已配对' : '无配对设备'}</div>
        </div>
      </div>

      {/* Cost section */}
      <div className="hx-card">
        <div className="hx-card-header" onClick={() => toggle('cost')}>
          <div className="hx-card-title">
            <div className="hx-card-icon"><DollarSign size={18} /></div>
            <div>
              <h2>费用统计</h2>
              <div className="hx-card-subtitle">会话、日、月度运行费用</div>
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
              { label: '本次会话', value: cost.session_cost_usd },
              { label: '今日', value: cost.daily_cost_usd },
              { label: '本月', value: cost.monthly_cost_usd },
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
                <span>总 Token 数</span>
                <strong>{cost.total_tokens.toLocaleString()}</strong>
              </div>
              <div className="hx-stat-pill">
                <span>请求数</span>
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
              <h2>渠道状态</h2>
              <div className="hx-card-subtitle">接入渠道和连接状态</div>
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
            <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>暂无接入渠道</p>
          ) : (
            <div className="hx-item-grid">
              {Object.entries(status.channels).map(([name, active]) => (
                <div key={name} className="hx-item-cell">
                  <span className="hx-item-name">{name}</span>
                  <span className="hx-status-dot">
                    <span className={`dot ${active ? 'active' : 'inactive'}`} />
                    {active ? '已连接' : '未连接'}
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
              <h2>组件健康</h2>
              <div className="hx-card-subtitle">运行时心跳和组件状态</div>
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
            <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>暂无组件健康数据</p>
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
                      重启次数: {component.restart_count}
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
