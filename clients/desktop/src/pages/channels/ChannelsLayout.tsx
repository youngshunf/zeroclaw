import { useState } from 'react';
import WeixinAuthModal from './WeixinAuthModal';
import { MessageSquare, QrCode } from 'lucide-react';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

export default function ChannelsLayout() {
  const [weixinOpen, setWeixinOpen] = useState(false);
  const { locale: _ } = useLocaleContext();

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <MessageSquare size={18} style={{ color: 'var(--hx-purple)' }} />
          <h2 style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)' }}>
            {t('nav.channels') || '联络接管'}
          </h2>
        </div>
      </div>

      <div className="hx-card" style={{ textAlign: 'center', padding: '48px 24px' }}>
        <MessageSquare size={40} style={{ color: 'var(--hx-text-tertiary)', margin: '0 auto 16px' }} />
        <p style={{ fontSize: 14, color: 'var(--hx-text-secondary)', maxWidth: 400, margin: '0 auto 24px', lineHeight: 1.6 }}>
          将您的聊天软件（如微信）绑定到唤星助手，让 AI 代理全权接管您的消息回复，实现 24 小时零死角服务。
        </p>
        <button
          onClick={() => setWeixinOpen(true)}
          style={{
            display: 'inline-flex', alignItems: 'center', gap: 6,
            background: 'var(--hx-purple)', color: 'white',
            fontSize: 13, fontWeight: 500, padding: '10px 20px',
            borderRadius: 8, border: 'none', cursor: 'pointer',
          }}
        >
          <QrCode size={15} />
          添加微信授权
        </button>
      </div>

      <WeixinAuthModal open={weixinOpen} onOpenChange={setWeixinOpen} />
    </div>
  );
}
