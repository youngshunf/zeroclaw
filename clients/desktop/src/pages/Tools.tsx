import { useState, useEffect } from 'react';
import { Wrench, Search, ChevronDown, ChevronRight, Terminal, Package } from 'lucide-react';
import type { ToolSpec, CliTool } from '@/types/api';
import { getTools, getCliTools } from '@/lib/api';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

export default function Tools() {
  const [tools, setTools] = useState<ToolSpec[]>([]);
  const [cliTools, setCliTools] = useState<CliTool[]>([]);
  const [search, setSearch] = useState('');
  const [expandedTool, setExpandedTool] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { locale } = useLocaleContext();

  useEffect(() => {
    Promise.all([getTools(), getCliTools()])
      .then(([t, c]) => { setTools(t); setCliTools(c); })
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  const filtered = tools.filter(t =>
    t.name.toLowerCase().includes(search.toLowerCase()) ||
    t.description.toLowerCase().includes(search.toLowerCase())
  );
  const filteredCli = cliTools.filter(t =>
    t.name.toLowerCase().includes(search.toLowerCase()) ||
    t.category.toLowerCase().includes(search.toLowerCase())
  );

  if (error) return <div className="hx-error-card"><h2>{t('tools.load_failed')}</h2><p>{error}</p></div>;
  if (loading) return <div className="hx-loading-center"><div className="hx-spinner" /></div>;

  return (
    <div>
      {/* Search */}
      <div className="hx-panel-search" style={{ maxWidth: 400, marginBottom: 20 }}>
        <Search size={16} />
        <input type="text" value={search} onChange={e => setSearch(e.target.value)} placeholder={t('tools.search_pl')} />
      </div>

      {/* Agent Tools */}
      <div style={{ marginBottom: 24 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 14 }}>
          <Wrench size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
            {t('tools.agent_tools')} ({filtered.length})
          </h2>
        </div>

        {filtered.length === 0 ? (
          <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>{t('tools.no_match')}</p>
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 12 }}>
            {filtered.map(tool => {
              const isExpanded = expandedTool === tool.name;
              return (
                <div key={tool.name} className="hx-card" style={{ padding: 0, overflow: 'hidden', marginBottom: 0 }}>
                  <button
                    onClick={() => setExpandedTool(isExpanded ? null : tool.name)}
                    style={{
                      width: '100%', textAlign: 'left', padding: 16, border: 'none',
                      background: 'transparent', cursor: 'pointer',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 8 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
                        <Package size={14} style={{ color: 'var(--hx-purple)', flexShrink: 0 }} />
                        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--hx-text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {tool.name}
                        </span>
                      </div>
                      {isExpanded ? <ChevronDown size={14} style={{ color: 'var(--hx-text-tertiary)' }} /> : <ChevronRight size={14} style={{ color: 'var(--hx-text-tertiary)' }} />}
                    </div>
                    <p style={{ fontSize: 12, color: 'var(--hx-text-secondary)', marginTop: 8, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>
                      {tool.description}
                    </p>
                  </button>
                  {isExpanded && tool.parameters && (
                    <div style={{ borderTop: '1px solid var(--hx-border)', padding: 16 }}>
                      <p style={{ fontSize: 11, color: 'var(--hx-text-tertiary)', marginBottom: 8, fontWeight: 500, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                        {t('tools.schema')}
                      </p>
                      <pre style={{ fontSize: 11, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-panel)', borderRadius: 8, padding: 12, overflowX: 'auto', maxHeight: 200, overflowY: 'auto' }}>
                        {JSON.stringify(tool.parameters, null, 2)}
                      </pre>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* CLI Tools */}
      {filteredCli.length > 0 && (
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 14 }}>
            <Terminal size={18} style={{ color: 'var(--hx-green)' }} />
            <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
              {t('tools.cli_tools')} ({filteredCli.length})
            </h2>
          </div>
          <div className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
            <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                  {[
                    { key: 'name', label: t('tools.th_name') },
                    { key: 'path', label: t('tools.th_path') },
                    { key: 'version', label: t('tools.th_version') },
                    { key: 'category', label: t('tools.th_category') },
                  ].map(h => (
                    <th key={h.key} style={{ textAlign: 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{h.label}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {filteredCli.map(tool => (
                  <tr key={tool.name} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                    <td style={{ padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-primary)' }}>{tool.name}</td>
                    <td style={{ padding: '10px 16px', color: 'var(--hx-text-tertiary)', fontFamily: 'monospace', fontSize: 11, maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis' }}>{tool.path}</td>
                    <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)' }}>{tool.version ?? '-'}</td>
                    <td style={{ padding: '10px 16px' }}>
                      <span style={{ display: 'inline-flex', padding: '2px 10px', borderRadius: 99, fontSize: 11, fontWeight: 500, background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', textTransform: 'capitalize' }}>
                        {tool.category}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
