import { useEffect, useState } from 'react';
import { useSubscriptionStore } from '../../stores/useSubscriptionStore';
import { ArrowUp, Loader2 } from 'lucide-react';
import { calculateUpgrade } from '../../lib/subscription-api';
import type { HxUpgradeCalculation } from '../../lib/subscription-api';
import CheckoutModal from './CheckoutModal';
import { t } from '@/lib/i18n';

export default function SubscriptionTab() {
  const { subscription, tiers, loading, fetchInfo, fetchTiers } = useSubscriptionStore();
  const [calculating, setCalculating] = useState(false);
  const [calcResult, setCalcResult] = useState<HxUpgradeCalculation | null>(null);
  const [selectedTier, setSelectedTier] = useState<string | null>(null);
  const [message, setMessage] = useState('');
  const [checkoutTier, setCheckoutTier] = useState<string | null>(null);
  const [checkoutPrice, setCheckoutPrice] = useState<number | null>(null);

  useEffect(() => {
    fetchInfo();
    fetchTiers();
  }, [fetchInfo, fetchTiers]);

  const currentTierName = subscription?.tier_display_name || '微星';

  const tierColorMap: Record<string, string> = {
    '微星': '#6E7681',
    '明星': '#6C5CE7',
    '恒星': '#00D2FF',
    '超新星': '#FFD93D',
  };

  const currentLevelColor = tierColorMap[currentTierName] || '#6E7681';

  const handleCalculate = async (tierName: string) => {
    setSelectedTier(tierName);
    setCalcResult(null);
    setMessage('');
    setCalculating(true);
    try {
      const result = await calculateUpgrade(tierName, 'monthly'); // 预计算默认先用月付测试
      setCalcResult(result);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : '计算差补失败');
    } finally {
      setCalculating(false);
    }
  };

  const handleUpgrade = () => {
    if (!selectedTier || !calcResult) return;
    setCheckoutTier(selectedTier);
    setCheckoutPrice(calcResult.final_price);
  };

  if (loading) {
    return <div style={{ display: 'flex', justifyContent: 'center', padding: '40px 0' }}><Loader2 className="animate-spin" size={24} color="var(--hx-text-secondary)" /></div>;
  }

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16, padding: 24, borderRadius: 12, border: '1px solid var(--hx-border)', background: 'var(--hx-secondary)', marginBottom: 32 }}>
        <div style={{ width: 48, height: 48, borderRadius: '50%', background: `radial-gradient(circle, ${currentLevelColor} 0%, transparent 70%)`, boxShadow: `0 0 20px ${currentLevelColor}40`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ width: 12, height: 12, borderRadius: '50%', background: currentLevelColor, boxShadow: `0 0 8px ${currentLevelColor}` }} />
        </div>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)' }}>当前等级</div>
          <div style={{ fontSize: 20, fontWeight: 'bold', color: currentLevelColor, marginTop: 4 }}>{currentTierName}</div>
        </div>
        {subscription?.subscription_end_date && (
          <div style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>
            到期时间：{new Date(subscription.subscription_end_date).toLocaleDateString()}
          </div>
        )}
      </div>

      <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 16 }}>升级解锁更多能力</h3>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 16, marginBottom: 32 }}>
        {tiers.map(tier => {
          const isCurrent = tier.display_name === currentTierName;
          const isSelected = selectedTier === tier.tier_name;
          const color = tierColorMap[tier.display_name] || '#6E7681';

          return (
            <div key={tier.id} style={{ position: 'relative', borderRadius: 12, border: `1px solid ${isSelected ? color : isCurrent ? 'var(--hx-border)' : 'var(--hx-border)'}`, background: isSelected ? 'var(--hx-bg)' : 'var(--hx-secondary)', padding: 20, opacity: isCurrent ? 0.8 : 1 }}>
              {isCurrent && <div style={{ position: 'absolute', top: -10, right: 16, background: color, color: '#fff', fontSize: 12, fontWeight: 500, padding: '2px 8px', borderRadius: 12 }}>当前</div>}
              <div style={{ fontSize: 18, fontWeight: 'bold', color, marginBottom: 8 }}>{tier.display_name}</div>
              <div style={{ fontSize: 24, fontWeight: 'bold', color: 'var(--hx-text-primary)' }}>
                {tier.monthly_price === 0 ? '免费' : `¥${tier.monthly_price}`} <span style={{ fontSize: 13, fontWeight: 'normal', color: 'var(--hx-text-secondary)' }}>/月</span>
              </div>
              <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginTop: 16, marginBottom: 24 }}>
                每月 {tier.monthly_credits} 积分
              </div>
              {!isCurrent && tier.monthly_price > 0 && (
                <button
                  onClick={() => handleCalculate(tier.tier_name)}
                  disabled={calculating && isSelected}
                  style={{ width: '100%', padding: '8px 16px', borderRadius: 8, border: `1px solid ${color}`, background: 'transparent', color: 'var(--hx-text-primary)', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, cursor: 'pointer' }}
                >
                  {(calculating && isSelected) ? <Loader2 size={16} className="animate-spin" /> : <ArrowUp size={16} />}
                  升级到 {tier.display_name}
                </button>
              )}
            </div>
          );
        })}
      </div>

      {calcResult && (
        <div style={{ padding: 24, borderRadius: 12, border: '1px solid #6C5CE7', background: 'rgba(108, 92, 231, 0.05)', marginBottom: 24 }}>
          <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 8 }}>差补金额确认</h3>
          <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 16 }}>{calcResult.message}</p>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8, marginBottom: 16 }}>
            {calcResult.original_price !== calcResult.final_price && <span style={{ textDecoration: 'line-through', color: 'var(--hx-text-tertiary)', fontSize: 14 }}>¥{calcResult.original_price}</span>}
            <span style={{ fontSize: 24, fontWeight: 'bold', color: '#6C5CE7' }}>¥{calcResult.final_price}</span>
          </div>
          <div style={{ display: 'flex', gap: 12 }}>
            <button
              onClick={handleUpgrade}
              style={{ padding: '8px 24px', borderRadius: 8, background: '#6C5CE7', color: '#fff', border: 'none', fontWeight: 600, cursor: 'pointer' }}
            >
              去支付
            </button>
            <button
              onClick={() => { setCalcResult(null); setSelectedTier(null); }}
              style={{ padding: '8px 24px', borderRadius: 8, background: 'transparent', color: 'var(--hx-text-secondary)', border: '1px solid var(--hx-border)', cursor: 'pointer' }}
            >
              取消
            </button>
          </div>
        </div>
      )}

      {message && <div style={{ color: '#ef4444', fontSize: 13 }}>{message}</div>}

      <CheckoutModal
        isOpen={!!checkoutTier}
        onClose={() => setCheckoutTier(null)}
        tier={checkoutTier || undefined}
        itemPrice={checkoutPrice !== null ? checkoutPrice : undefined}
      />
    </div>
  );
}
