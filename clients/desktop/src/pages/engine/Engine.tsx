/**
 * 引擎管理页面 — ZeroClaw Sidecar 控制台
 *
 * 使用 hx 设计系统 CSS 类，与其他设置页面风格统一。
 */

import { useState, useRef, useEffect } from 'react';
import {
  Power, Square, RotateCw, Activity,
  Terminal, ChevronDown, ChevronUp,
  Cpu, Clock, Database, Zap, AlertTriangle,
  Trash2, Download, Settings,
} from 'lucide-react';
import { useSidecar, type QuickConfig } from '@/hooks/useSidecar';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

export default function Engine() {
  const {
    status, loading, starting, stopping,
    logs, error, config, configLoading,
    start, stop, restart,
    refreshStatus, clearLogs,
    loadConfig, saveConfig,
  } = useSidecar();

  const { locale } = useLocaleContext();
  const [logsExpanded, setLogsExpanded] = useState(true);
  const [configExpanded, setConfigExpanded] = useState(false);
  const logEndRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    if (autoScroll && logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const formatUptime = (seconds: number | null): string => {
    if (!seconds) return '-';
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    return `${h}h ${m}m`;
  };

  const exportLogs = () => {
    const blob = new Blob([logs.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `zeroclaw-logs-${new Date().toISOString().slice(0, 19)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (loading) {
    return (
      <div className="hx-loading-center">
        <div className="hx-spinner" />
      </div>
    );
  }

  const isRunning = status?.running ?? false;

  return (
    <div className="hx-engine">
      {/* Header */}
      <div className="hx-engine-header">
        <div className="hx-engine-title">
          <div className="hx-engine-icon">
            <Cpu size={16} />
          </div>
          <h2>{t('engine.title')}</h2>
        </div>
        <button className="hx-engine-refresh" onClick={refreshStatus}>
          <RotateCw size={12} />
          {t('engine.refresh')}
        </button>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="hx-engine-error">
          <AlertTriangle size={14} />
          <span>{error}</span>
        </div>
      )}

      {/* Status Card */}
      <div className="hx-card">
        <div className="hx-engine-status-row">
          <div className="hx-engine-status-info">
            <span className={`hx-engine-dot ${isRunning ? 'active' : ''}`} />
            <span className="hx-engine-status-label">
              {isRunning ? t('engine.running') : t('engine.stopped')}
            </span>
            {status?.pid && (
              <span className="hx-engine-pid">PID: {status.pid}</span>
            )}
          </div>

          <div className="hx-engine-actions">
            {!isRunning ? (
              <button className="hx-btn hx-btn-primary" onClick={start} disabled={starting}>
                <Power size={14} />
                {starting ? t('engine.starting') : t('engine.start')}
              </button>
            ) : (
              <>
                <button className="hx-btn hx-btn-outline" onClick={restart} disabled={starting}>
                  <RotateCw size={14} />
                  {starting ? t('engine.restarting') : t('engine.restart')}
                </button>
                <button className="hx-btn hx-btn-danger" onClick={stop} disabled={stopping}>
                  <Square size={14} />
                  {stopping ? t('engine.stopping') : t('engine.stop')}
                </button>
              </>
            )}
          </div>
        </div>

        {/* Metrics Grid */}
        {isRunning && status && (
          <div className="hx-engine-metrics">
            <div className="hx-engine-metric">
              <div className="hx-engine-metric-label"><Zap size={13} /> {t('engine.model')}</div>
              <div className="hx-engine-metric-value">{status.model ?? '-'}</div>
            </div>
            <div className="hx-engine-metric">
              <div className="hx-engine-metric-label"><Activity size={13} /> Provider</div>
              <div className="hx-engine-metric-value truncate">{status.provider?.replace('custom:', '') ?? '-'}</div>
            </div>
            <div className="hx-engine-metric">
              <div className="hx-engine-metric-label"><Clock size={13} /> {t('engine.uptime')}</div>
              <div className="hx-engine-metric-value">{formatUptime(status.uptime_seconds)}</div>
            </div>
            <div className="hx-engine-metric">
              <div className="hx-engine-metric-label"><Database size={13} /> {t('engine.memory_backend')}</div>
              <div className="hx-engine-metric-value">{status.memory_backend ?? '-'}</div>
            </div>
          </div>
        )}

        {/* Restart warning */}
        {status && status.restart_count > 0 && (
          <div className="hx-engine-warning">
            <AlertTriangle size={12} />
            {t('engine.restarted_count', { count: status.restart_count })}
          </div>
        )}
      </div>

      {/* Quick Config */}
      <div className="hx-card hx-card-collapse">
        <div
          className="hx-card-header"
          onClick={() => {
            setConfigExpanded(!configExpanded);
            if (!configExpanded && !config) loadConfig();
          }}
        >
          <div className="hx-card-title">
            <div className="hx-card-icon"><Settings size={16} /></div>
            <div>
              <h2>{t('engine.quick_config')}</h2>
            </div>
          </div>
          {configExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
        </div>

        {configExpanded && (
          <div className="hx-card-body">
            {configLoading ? (
              <div className="hx-engine-empty">{t('engine.loading_config')}</div>
            ) : config ? (
              <ConfigEditor config={config} onSave={saveConfig} saving={configLoading} />
            ) : (
              <div className="hx-engine-empty">
                {t('engine.config_helper')}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Logs */}
      <div className="hx-card hx-card-collapse">
        <div className="hx-card-header" onClick={() => setLogsExpanded(!logsExpanded)}>
          <div className="hx-card-title">
            <div className="hx-card-icon"><Terminal size={16} /></div>
            <div>
              <h2>{t('engine.logs_title')}</h2>
              <span className="hx-card-subtitle">{t('engine.lines', { count: logs.length })}</span>
            </div>
          </div>
          <div className="hx-engine-log-controls">
            {logsExpanded && (
              <>
                <button
                  className={`hx-engine-log-btn ${autoScroll ? 'active' : ''}`}
                  onClick={(e) => { e.stopPropagation(); setAutoScroll(!autoScroll); }}
                >
                  {t('engine.auto_scroll')}
                </button>
                <button
                  className="hx-engine-log-icon-btn"
                  title={t('engine.clear_logs') || 'Clear Logs'}
                  onClick={(e) => { e.stopPropagation(); clearLogs(); }}
                >
                  <Trash2 size={13} />
                </button>
                <button
                  className="hx-engine-log-icon-btn"
                  title={t('engine.export_logs') || 'Export Logs'}
                  onClick={(e) => { e.stopPropagation(); exportLogs(); }}
                >
                  <Download size={13} />
                </button>
              </>
            )}
            {logsExpanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
          </div>
        </div>

        {logsExpanded && (
          <div className="hx-engine-log-panel">
            {logs.length === 0 ? (
              <div className="hx-engine-log-empty">
                {isRunning ? t('engine.waiting_logs') : t('engine.not_running')}
              </div>
            ) : (
              logs.map((line, i) => (
                <div
                  key={i}
                  className={`hx-engine-log-line ${
                    line.includes('ERROR') || line.includes('[stderr]')
                      ? 'error'
                      : line.includes('WARN')
                      ? 'warn'
                      : line.includes('INFO')
                      ? 'info'
                      : ''
                  }`}
                >
                  {line}
                </div>
              ))
            )}
            <div ref={logEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}

// ── 快捷配置编辑器 ────────────────────────────────────────

const MODEL_OPTIONS = [
  'claude-sonnet-4-6',
  'claude-sonnet-4-20250514',
  'gpt-4o',
  'gpt-4o-mini',
  'deepseek-chat',
  'deepseek-reasoner',
];

const AUTONOMY_OPTIONS = [
  { value: 'supervised', label: '监督模式 — 高风险操作需确认' },
  { value: 'semi', label: '半自主 — 仅文件删除需确认' },
  { value: 'full', label: '全自主 — 所有操作自动执行' },
];

function ConfigEditor({
  config,
  onSave,
  saving,
}: {
  config: QuickConfig;
  onSave: (config: QuickConfig) => Promise<void>;
  saving: boolean;
}) {
  const [draft, setDraft] = useState<QuickConfig>({ ...config });
  const hasChanges = JSON.stringify(draft) !== JSON.stringify(config);

  return (
    <div className="hx-engine-config-form">
      {/* 模型 */}
      <div className="hx-engine-field">
        <label>{t('engine.default_model')}</label>
        <Select
          value={draft.default_model ?? 'none'}
          onValueChange={(val) => setDraft({ ...draft, default_model: val === 'none' ? null : val })}
        >
          <SelectTrigger>
            <SelectValue placeholder={t('engine.not_set')} />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="none">{t('engine.not_set')}</SelectItem>
            {MODEL_OPTIONS.map((m) => (
              <SelectItem key={m} value={m}>{m}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* 温度 */}
      <div className="hx-engine-field">
        <label>{t('engine.temperature')}: {draft.default_temperature ?? 0.7}</label>
        <input
          type="range"
          min="0" max="2" step="0.1"
          value={draft.default_temperature ?? 0.7}
          onChange={(e) => setDraft({ ...draft, default_temperature: parseFloat(e.target.value) })}
        />
        <div className="hx-engine-range-labels">
          <span>{t('engine.precise')}</span>
          <span>{t('engine.balanced')}</span>
          <span>{t('engine.creative')}</span>
        </div>
      </div>

      {/* 自主级别 */}
      <div className="hx-engine-field">
        <label>{t('engine.autonomy')}</label>
        <Select
          value={draft.autonomy_level ?? 'supervised'}
          onValueChange={(val) => setDraft({ ...draft, autonomy_level: val })}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="supervised">{t('engine.supervised')}</SelectItem>
            <SelectItem value="semi">{t('engine.semi')}</SelectItem>
            <SelectItem value="full">{t('engine.full')}</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* 按钮 */}
      <div className="hx-engine-config-actions">
        {hasChanges && (
          <button className="hx-btn hx-btn-outline" onClick={() => setDraft({ ...config })}>
            {t('engine.reset')}
          </button>
        )}
        <button
          className={`hx-btn ${hasChanges ? 'hx-btn-primary' : 'hx-btn-disabled'}`}
          onClick={() => onSave(draft)}
          disabled={!hasChanges || saving}
        >
          {saving ? t('config.saving') : t('engine.save_restart')}
        </button>
      </div>
    </div>
  );
}
