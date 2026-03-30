import { useState } from 'react';
import { DollarSign, Star, Coins } from 'lucide-react';
import UsageStats from '@/components/billing/UsageStats';
import SubscriptionTab from '@/components/billing/SubscriptionTab';
import CreditsTab from '@/components/billing/CreditsTab';

type Tab = 'usage' | 'subscription' | 'credits';

export default function Cost() {
  const [activeTab, setActiveTab] = useState<Tab>('subscription');

  return (
    <div style={{ padding: '0 0 40px 0', maxWidth: 960, margin: '0 auto' }}>
      <div style={{ marginBottom: 32 }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, margin: '0 0 8px 0', color: 'var(--hx-text-primary)' }}>账户与计费</h1>
        <p style={{ margin: 0, color: 'var(--hx-text-secondary)', fontSize: 14 }}>在这里管理您的唤星会员订阅、查看代币消耗记录并随时给数字钱包充值。</p>
      </div>

      <div style={{ display: 'flex', gap: 12, marginBottom: 24, borderBottom: '1px solid var(--hx-border)', paddingBottom: 16 }}>
        <button
          onClick={() => setActiveTab('subscription')}
          style={{
            display: 'flex', alignItems: 'center', gap: 8, padding: '8px 16px', borderRadius: 8, cursor: 'pointer',
            background: activeTab === 'subscription' ? 'var(--hx-secondary)' : 'transparent',
            border: `1px solid ${activeTab === 'subscription' ? 'var(--hx-border)' : 'transparent'}`,
            color: activeTab === 'subscription' ? 'var(--hx-text-primary)' : 'var(--hx-text-secondary)',
            fontWeight: activeTab === 'subscription' ? 600 : 500
          }}
        >
          <Star size={18} />
          会员订阅
        </button>
        <button
          onClick={() => setActiveTab('credits')}
          style={{
            display: 'flex', alignItems: 'center', gap: 8, padding: '8px 16px', borderRadius: 8, cursor: 'pointer',
            background: activeTab === 'credits' ? 'var(--hx-secondary)' : 'transparent',
            border: `1px solid ${activeTab === 'credits' ? 'var(--hx-border)' : 'transparent'}`,
            color: activeTab === 'credits' ? 'var(--hx-text-primary)' : 'var(--hx-text-secondary)',
            fontWeight: activeTab === 'credits' ? 600 : 500
          }}
        >
          <Coins size={18} />
          积分与充值
        </button>
        <button
          onClick={() => setActiveTab('usage')}
          style={{
            display: 'flex', alignItems: 'center', gap: 8, padding: '8px 16px', borderRadius: 8, cursor: 'pointer',
            background: activeTab === 'usage' ? 'var(--hx-secondary)' : 'transparent',
            border: `1px solid ${activeTab === 'usage' ? 'var(--hx-border)' : 'transparent'}`,
            color: activeTab === 'usage' ? 'var(--hx-text-primary)' : 'var(--hx-text-secondary)',
            fontWeight: activeTab === 'usage' ? 600 : 500
          }}
        >
          <DollarSign size={18} />
          模型消耗
        </button>
      </div>

      <div style={{ position: 'relative' }}>
        {activeTab === 'usage' && <UsageStats />}
        {activeTab === 'subscription' && <SubscriptionTab />}
        {activeTab === 'credits' && <CreditsTab />}
      </div>
    </div>
  );
}
