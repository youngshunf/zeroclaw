import {
  Settings,
  Save,
  CheckCircle,
  AlertTriangle,
  ShieldAlert,
  FileText,
  SlidersHorizontal,
} from 'lucide-react';
import { useConfigForm, type EditorMode } from '@/components/config/useConfigForm';
import ConfigFormEditor from '@/components/config/ConfigFormEditor';
import ConfigRawEditor from '@/components/config/ConfigRawEditor';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

function ModeTab({
  active,
  icon: Icon,
  label,
  onClick,
}: {
  mode: EditorMode;
  active: boolean;
  icon: React.ComponentType<{ className?: string; size?: number }>;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 6,
        padding: '6px 12px',
        borderRadius: 8,
        fontSize: 13,
        fontWeight: 500,
        border: 'none',
        cursor: 'pointer',
        transition: 'all 0.2s',
        background: active ? 'var(--hx-purple)' : 'transparent',
        color: active ? 'white' : 'var(--hx-text-secondary)',
      }}
    >
      <Icon size={14} />
      {label}
    </button>
  );
}

export default function Config() {
  const {
    loading,
    saving,
    error,
    success,
    mode,
    rawToml,
    setMode,
    getFieldValue,
    setFieldValue,
    isFieldMasked,
    setRawToml,
    save,
  } = useConfigForm();

  const { locale } = useLocaleContext();

  if (loading) {
    return <div className="hx-loading-center"><div className="hx-spinner" /></div>;
  }

  return (
    <div>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Settings size={20} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{t('config.editor_title')}</h2>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 2, background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)', borderRadius: 10, padding: 2 }}>
            <ModeTab mode="form" active={mode === 'form'} icon={SlidersHorizontal} label={t('config.form')} onClick={() => setMode('form')} />
            <ModeTab mode="raw" active={mode === 'raw'} icon={FileText} label={t('config.raw')} onClick={() => setMode('raw')} />
          </div>
          <button
            onClick={save}
            disabled={saving}
            style={{
              display: 'flex', alignItems: 'center', gap: 6,
              background: 'var(--hx-purple)', color: 'white',
              fontSize: 13, fontWeight: 500, padding: '8px 16px',
              borderRadius: 8, border: 'none', cursor: saving ? 'not-allowed' : 'pointer',
              opacity: saving ? 0.5 : 1,
            }}
          >
            <Save size={14} />
            {saving ? t('config.saving') : t('config.save')}
          </button>
        </div>
      </div>

      {/* Sensitive fields note */}
      <div style={{
        display: 'flex', alignItems: 'flex-start', gap: 12,
        background: '#FFFBEB', border: '1px solid #FDE68A', borderRadius: 10, padding: 14, marginBottom: 16,
      }}>
        <ShieldAlert size={18} style={{ color: '#D97706', flexShrink: 0, marginTop: 2 }} />
        <div>
          <p style={{ fontSize: 13, fontWeight: 500, color: '#92400E' }}>{t('config.hidden_title')}</p>
          <p style={{ fontSize: 12, color: '#B45309', marginTop: 2 }}>
            {mode === 'form'
              ? t('config.hidden_desc_form')
              : t('config.hidden_desc_raw')}
          </p>
        </div>
      </div>

      {/* Success message */}
      {success && (
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          background: '#F0FDF4', border: '1px solid #BBF7D0', borderRadius: 10, padding: 12, marginBottom: 16,
        }}>
          <CheckCircle size={16} style={{ color: '#16A34A', flexShrink: 0 }} />
          <span style={{ fontSize: 13, color: '#15803D' }}>{success}</span>
        </div>
      )}

      {/* Error message */}
      {error && (
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8,
          background: '#FEF2F2', border: '1px solid #FECACA', borderRadius: 10, padding: 12, marginBottom: 16,
        }}>
          <AlertTriangle size={16} style={{ color: '#DC2626', flexShrink: 0 }} />
          <span style={{ fontSize: 13, color: '#991B1B' }}>{error}</span>
        </div>
      )}

      {/* Editor */}
      {mode === 'form' ? (
        <ConfigFormEditor
          getFieldValue={getFieldValue}
          setFieldValue={setFieldValue}
          isFieldMasked={isFieldMasked}
        />
      ) : (
        <ConfigRawEditor
          rawToml={rawToml}
          onChange={setRawToml}
          disabled={saving}
        />
      )}
    </div>
  );
}
