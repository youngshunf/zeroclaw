import { useState, useEffect } from 'react';
import { Brain, Search, Plus, Trash2, X } from 'lucide-react';
import type { MemoryEntry } from '@/types/api';
import { getMemory, storeMemory, deleteMemory } from '@/lib/api';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

function truncate(text: string, max: number): string {
  return text.length <= max ? text : text.slice(0, max) + '...';
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString('zh-CN');
}

const inputStyle: React.CSSProperties = {
  width: '100%', background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)',
  borderRadius: 8, padding: '8px 12px', fontSize: 13, color: 'var(--hx-text-primary)',
  outline: 'none',
};

export default function Memory() {
  const { locale } = useLocaleContext();
  const [entries, setEntries] = useState<MemoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('all');
  const [showForm, setShowForm] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [formKey, setFormKey] = useState('');
  const [formContent, setFormContent] = useState('');
  const [formCategory, setFormCategory] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const fetchEntries = (q?: string, cat?: string) => {
    setLoading(true);
    getMemory(q || undefined, cat || undefined).then(setEntries).catch(err => setError(err.message)).finally(() => setLoading(false));
  };

  useEffect(() => { fetchEntries(); }, []);
  const actualFilter = categoryFilter === 'all' ? '' : categoryFilter;
  const handleSearch = () => fetchEntries(search, actualFilter);
  const handleKeyDown = (e: React.KeyboardEvent) => { if (e.key === 'Enter') handleSearch(); };
  const categories = Array.from(new Set(entries.map(e => e.category))).sort();

  const handleAdd = async () => {
    if (!formKey.trim() || !formContent.trim()) { setFormError(t('mem.required') || 'Key 和内容为必填项'); return; }
    setSubmitting(true); setFormError(null);
    try { await storeMemory(formKey.trim(), formContent.trim(), formCategory.trim() || undefined); fetchEntries(search, actualFilter); setShowForm(false); setFormKey(''); setFormContent(''); setFormCategory(''); }
    catch (err: unknown) { setFormError(err instanceof Error ? err.message : (t('mem.save_fail') || '保存记忆失败')); }
    finally { setSubmitting(false); }
  };

  const handleDelete = async (key: string) => {
    try { await deleteMemory(key); setEntries(prev => prev.filter(e => e.key !== key)); }
    catch (err: unknown) { setError(err instanceof Error ? err.message : (t('mem.del_fail') || '删除失败')); }
    finally { setConfirmDelete(null); }
  };

  if (error && entries.length === 0) return <div className="hx-error-card"><h2>{t('mem.load_fail')}</h2><p>{error}</p></div>;

  return (
    <div>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Brain size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{t('mem.title') || 'Memory Management'} ({entries.length})</h2>
        </div>
        <button onClick={() => setShowForm(true)} style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'var(--hx-purple)', color: 'white',
          fontSize: 13, fontWeight: 500, padding: '8px 16px', borderRadius: 8, border: 'none', cursor: 'pointer',
        }}>
          <Plus size={14} />{t('mem.add') || 'Add Memory'}
        </button>
      </div>

      {/* Search and Filter */}
      <div style={{ display: 'flex', gap: 10, marginBottom: 16, flexWrap: 'wrap' }}>
        <div className="hx-panel-search" style={{ flex: 1, minWidth: 200 }}>
          <Search size={16} />
          <input type="text" value={search} onChange={e => setSearch(e.target.value)} onKeyDown={handleKeyDown} placeholder={t('mem.search_pl') || 'Search memory...'} />
        </div>
        <div style={{ minWidth: 120 }}>
          <Select value={categoryFilter} onValueChange={setCategoryFilter}>
            <SelectTrigger>
              <SelectValue placeholder={t('mem.all_cat')} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t('mem.all_cat')}</SelectItem>
              {categories.map(cat => <SelectItem key={cat} value={cat}>{cat}</SelectItem>)}
            </SelectContent>
          </Select>
        </div>
        <button onClick={handleSearch} style={{
          padding: '8px 16px', background: 'var(--hx-purple)', color: 'white', fontSize: 13, fontWeight: 500,
          borderRadius: 8, border: 'none', cursor: 'pointer',
        }}>{t('mem.search')}</button>
      </div>

      {error && <div className="hx-error-card" style={{ marginBottom: 16 }}><p>{error}</p></div>}

      {/* Add Memory Modal */}
      {showForm && (
        <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 50 }}>
          <div style={{ background: 'var(--hx-bg-main)', border: '1px solid var(--hx-border)', borderRadius: 16, padding: 24, width: '100%', maxWidth: 440, margin: '0 16px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16 }}>
              <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{t('mem.add')}</h3>
              <button onClick={() => { setShowForm(false); setFormError(null); }} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-text-tertiary)' }}>
                <X size={18} />
              </button>
            </div>
            {formError && <div className="hx-error-card" style={{ marginBottom: 12, padding: 10 }}><p style={{ fontSize: 12 }}>{formError}</p></div>}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>Key <span style={{ color: '#DC2626' }}>*</span></label>
                <input type="text" value={formKey} onChange={e => setFormKey(e.target.value)} placeholder={t('mem.key_pl') || 'e.g. user_preferences'} style={inputStyle} />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>{t('mem.content')} <span style={{ color: '#DC2626' }}>*</span></label>
                <textarea value={formContent} onChange={e => setFormContent(e.target.value)} placeholder={t('mem.content_pl') || 'Memory content...'} rows={4} style={{ ...inputStyle, resize: 'none' }} />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 4 }}>{t('mem.cat_opt')}</label>
                <input type="text" value={formCategory} onChange={e => setFormCategory(e.target.value)} placeholder={t('mem.cat_pl') || 'e.g. preferences, context'} style={inputStyle} />
              </div>
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 10, marginTop: 20 }}>
              <button onClick={() => { setShowForm(false); setFormError(null); }} style={{ padding: '8px 16px', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', border: '1px solid var(--hx-border)', borderRadius: 8, background: 'transparent', cursor: 'pointer' }}>{t('mem.cancel')}</button>
              <button onClick={handleAdd} disabled={submitting} style={{ padding: '8px 16px', fontSize: 13, fontWeight: 500, color: 'white', background: 'var(--hx-purple)', borderRadius: 8, border: 'none', cursor: submitting ? 'not-allowed' : 'pointer', opacity: submitting ? 0.5 : 1 }}>{submitting ? t('mem.saving') : t('mem.save')}</button>
            </div>
          </div>
        </div>
      )}

      {/* Memory Table */}
      {loading ? (
        <div className="hx-loading-center"><div className="hx-spinner" /></div>
      ) : entries.length === 0 ? (
        <div className="hx-card" style={{ textAlign: 'center', padding: '40px 20px' }}>
          <Brain size={40} style={{ color: 'var(--hx-text-tertiary)', margin: '0 auto 12px' }} />
          <p style={{ color: 'var(--hx-text-tertiary)', fontSize: 13 }}>{t('mem.no_entries')}</p>
        </div>
      ) : (
        <div className="hx-card" style={{ padding: 0, overflow: 'hidden' }}>
          <table style={{ width: '100%', fontSize: 13, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--hx-border)' }}>
                {[{ label: t('mem.th_key'), key: 'Key' }, { label: t('mem.th_content'), key: 'Content' }, { label: t('mem.th_cat'), key: 'Category' }, { label: t('mem.th_time'), key: 'Time' }, { label: t('mem.th_actions'), key: 'Actions' }].map(h => (
                  <th key={h.key} style={{ textAlign: h.key === 'Actions' ? 'right' : 'left', padding: '10px 16px', fontWeight: 500, color: 'var(--hx-text-tertiary)' }}>{h.label}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {entries.map(entry => (
                <tr key={entry.id} style={{ borderBottom: '1px solid var(--hx-border-light)' }}>
                  <td style={{ padding: '10px 16px', fontFamily: 'monospace', fontSize: 11, fontWeight: 500, color: 'var(--hx-text-primary)' }}>{entry.key}</td>
                  <td style={{ padding: '10px 16px', color: 'var(--hx-text-secondary)', maxWidth: 300 }} title={entry.content}>{truncate(entry.content, 80)}</td>
                  <td style={{ padding: '10px 16px' }}>
                    <span style={{ display: 'inline-flex', padding: '2px 10px', borderRadius: 99, fontSize: 11, fontWeight: 500, background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', textTransform: 'capitalize' }}>{entry.category}</span>
                  </td>
                  <td style={{ padding: '10px 16px', fontSize: 12, color: 'var(--hx-text-tertiary)', whiteSpace: 'nowrap' }}>{formatDate(entry.timestamp)}</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right' }}>
                    {confirmDelete === entry.key ? (
                      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
                        <span style={{ fontSize: 12, color: '#DC2626' }}>{t('mem.del_confirm')}</span>
                        <button onClick={() => handleDelete(entry.key)} style={{ fontSize: 12, fontWeight: 500, color: '#DC2626', background: 'none', border: 'none', cursor: 'pointer' }}>{t('mem.yes')}</button>
                        <button onClick={() => setConfirmDelete(null)} style={{ fontSize: 12, fontWeight: 500, color: 'var(--hx-text-tertiary)', background: 'none', border: 'none', cursor: 'pointer' }}>{t('mem.no')}</button>
                      </span>
                    ) : (
                      <button onClick={() => setConfirmDelete(entry.key)} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--hx-text-tertiary)' }}>
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
