import { useState, useEffect, useRef, useCallback } from 'react';
import { Activity, Pause, Play, ArrowDown, Filter } from 'lucide-react';
import type { SSEEvent } from '@/types/api';
import { SSEClient } from '@/lib/sse';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

function formatTimestamp(ts?: string): string {
  if (!ts) return new Date().toLocaleTimeString('zh-CN');
  return new Date(ts).toLocaleTimeString('zh-CN');
}

function eventBadgeStyle(type: string): React.CSSProperties {
  const base: React.CSSProperties = { display: 'inline-flex', alignItems: 'center', padding: '2px 8px', borderRadius: 4, fontSize: 11, fontWeight: 500, border: '1px solid', textTransform: 'capitalize' as const, flexShrink: 0 };
  switch (type.toLowerCase()) {
    case 'error': return { ...base, background: '#FEF2F2', color: '#DC2626', borderColor: '#FECACA' };
    case 'warn': case 'warning': return { ...base, background: '#FFFBEB', color: '#D97706', borderColor: '#FDE68A' };
    case 'tool_call': case 'tool_result': return { ...base, background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', borderColor: 'rgba(124,58,237,0.2)' };
    case 'message': case 'chat': return { ...base, background: '#EFF6FF', color: '#2563EB', borderColor: '#BFDBFE' };
    case 'health': case 'status': return { ...base, background: '#F0FDF4', color: '#16A34A', borderColor: '#BBF7D0' };
    default: return { ...base, background: 'var(--hx-bg-panel)', color: 'var(--hx-text-tertiary)', borderColor: 'var(--hx-border)' };
  }
}

interface LogEntry { id: string; event: SSEEvent; }

export default function Logs() {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [paused, setPaused] = useState(false);
  const [connected, setConnected] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [typeFilters, setTypeFilters] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const sseRef = useRef<SSEClient | null>(null);
  const pausedRef = useRef(false);
  const entryIdRef = useRef(0);
  const { locale } = useLocaleContext();

  useEffect(() => { pausedRef.current = paused; }, [paused]);

  useEffect(() => {
    const client = new SSEClient();
    client.onConnect = () => setConnected(true);
    client.onError = () => setConnected(false);
    client.onEvent = (event: SSEEvent) => {
      if (pausedRef.current) return;
      entryIdRef.current += 1;
      const entry: LogEntry = { id: `log-${entryIdRef.current}`, event };
      setEntries(prev => { const next = [...prev, entry]; return next.length > 500 ? next.slice(-500) : next; });
    };
    client.connect();
    sseRef.current = client;
    return () => client.disconnect();
  }, []);

  useEffect(() => { if (autoScroll && containerRef.current) containerRef.current.scrollTop = containerRef.current.scrollHeight; }, [entries, autoScroll]);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 50);
  }, []);

  const jumpToBottom = () => { if (containerRef.current) containerRef.current.scrollTop = containerRef.current.scrollHeight; setAutoScroll(true); };

  const allTypes = Array.from(new Set(entries.map(e => e.event.type))).sort();
  const toggleTypeFilter = (type: string) => setTypeFilters(prev => { const next = new Set(prev); next.has(type) ? next.delete(type) : next.add(type); return next; });
  const filteredEntries = typeFilters.size === 0 ? entries : entries.filter(e => typeFilters.has(e.event.type));

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: 400, height: 'calc(100dvh - 8.5rem)' }}>
      {/* Toolbar */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 0', borderBottom: '1px solid var(--hx-border)', marginBottom: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <Activity size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{t('logs_extra.title')}</h2>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, marginLeft: 8 }}>
            <span style={{ width: 8, height: 8, borderRadius: '50%', background: connected ? 'var(--hx-green)' : '#DC2626' }} />
            <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>{connected ? t('logs_extra.connected') : t('logs_extra.disconnected')}</span>
          </span>
          <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginLeft: 8 }}>{t('logs_extra.events', { count: filteredEntries.length })}</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button onClick={() => setPaused(!paused)} style={{
            display: 'flex', alignItems: 'center', gap: 4,
            padding: '6px 12px', borderRadius: 8, fontSize: 13, fontWeight: 500, border: 'none', cursor: 'pointer',
            background: paused ? 'var(--hx-green)' : '#D97706', color: 'white',
          }}>
            {paused ? <><Play size={14} />{t('logs_extra.resume')}</> : <><Pause size={14} />{t('logs_extra.pause')}</>}
          </button>
          {!autoScroll && (
            <button onClick={jumpToBottom} style={{
              display: 'flex', alignItems: 'center', gap: 4,
              padding: '6px 12px', borderRadius: 8, fontSize: 13, fontWeight: 500, border: 'none', cursor: 'pointer',
              background: 'var(--hx-purple)', color: 'white',
            }}>
              <ArrowDown size={14} />{t('logs_extra.jump_bottom')}
            </button>
          )}
        </div>
      </div>

      {/* Filters */}
      {allTypes.length > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 0', borderBottom: '1px solid var(--hx-border)', overflowX: 'auto' }}>
          <Filter size={14} style={{ color: 'var(--hx-text-tertiary)', flexShrink: 0 }} />
          <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', flexShrink: 0 }}>{t('logs_extra.filter')}</span>
          {allTypes.map(type => (
            <label key={type} style={{ display: 'flex', alignItems: 'center', gap: 4, cursor: 'pointer', flexShrink: 0 }}>
              <input type="checkbox" checked={typeFilters.has(type)} onChange={() => toggleTypeFilter(type)} style={{ width: 14, height: 14, accentColor: 'var(--hx-purple)' }} />
              <span style={{ fontSize: 12, color: 'var(--hx-text-secondary)', textTransform: 'capitalize' }}>{type}</span>
            </label>
          ))}
          {typeFilters.size > 0 && (
            <button onClick={() => setTypeFilters(new Set())} style={{ fontSize: 12, color: 'var(--hx-purple)', background: 'none', border: 'none', cursor: 'pointer', flexShrink: 0 }}>
              {t('logs_extra.clear')}
            </button>
          )}
        </div>
      )}

      {/* Log entries */}
      <div ref={containerRef} onScroll={handleScroll} style={{ flex: 1, overflowY: 'auto', padding: '12px 0', display: 'flex', flexDirection: 'column', gap: 6 }}>
        {filteredEntries.length === 0 ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--hx-text-tertiary)' }}>
            <Activity size={40} style={{ marginBottom: 12, opacity: 0.4 }} />
            <p style={{ fontSize: 13 }}>{paused ? t('logs_extra.paused_msg') : t('logs_extra.waiting')}</p>
          </div>
        ) : (
          filteredEntries.map(entry => {
            const { event } = entry;
            const detail = event.message ?? event.content ?? event.data ?? JSON.stringify(
              Object.fromEntries(Object.entries(event).filter(([k]) => k !== 'type' && k !== 'timestamp'))
            );
            return (
              <div key={entry.id} style={{
                background: 'var(--hx-bg-main)', border: '1px solid var(--hx-border-light)',
                borderRadius: 10, padding: 12,
              }}>
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
                  <span style={{ fontSize: 11, color: 'var(--hx-text-tertiary)', fontFamily: 'monospace', whiteSpace: 'nowrap', marginTop: 2 }}>
                    {formatTimestamp(event.timestamp)}
                  </span>
                  <span style={eventBadgeStyle(event.type)}>{event.type}</span>
                  <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', wordBreak: 'break-all', minWidth: 0 }}>
                    {typeof detail === 'string' ? detail : JSON.stringify(detail)}
                  </p>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
