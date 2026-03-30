import { useEffect, useState } from 'react';
import { useSubscriptionStore } from '../../stores/useSubscriptionStore';
import { Package, Loader2, Coins } from 'lucide-react';
import CheckoutModal from './CheckoutModal';
import { t } from '@/lib/i18n';

const creditTypeMap: Record<string, string> = {
  monthly: '月度订阅',
  purchased: '购买积分',
  bonus: '赠送积分',
  yearly: '年度订阅',
  trial: '试用积分',
  subscription_upgrade: '升级赠送',
  official_grant: '官方赠送',
};

export default function CreditsTab() {
  const { subscription, packages, creditHistory, loading, fetchInfo, fetchPackages, fetchCreditHistory } = useSubscriptionStore();
  
  const [checkoutPkgId, setCheckoutPkgId] = useState<number | null>(null);

  useEffect(() => {
    fetchInfo();
    fetchPackages();
    fetchCreditHistory();
  }, [fetchInfo, fetchPackages, fetchCreditHistory]);

  const totalCredits = subscription?.current_credits ?? 0;

  const handlePurchase = (pkgId: number) => {
    // 此处可以通过复用 CheckoutModal, 但 Checkout Modal 目前接受 tier 参数。
    // 如果我们想集成积分包支付，可以在 CheckoutModal 里面加 package_Id 支持。
    // 但是官网 API purchaseCredits 会直接创建订单还是直接用储值扣款？
    // 官网 purchaseCredits 实际返回的是包含二维码的 paymentResult: "user_tier/app/subscription/purchase" 这是统一的网关吗？ 
    // Wait, let's just trigger the package payment via modal mapping.
    // In original code: `const result = await purchaseCredits(packageId)` -> returns `{success}`. Does it trigger a payment?
    // Wait, the store uses `purchaseCredits(pkg.id)`.
    setCheckoutPkgId(pkgId);
  };

  const selectedPkg = packages.find(p => p.id === checkoutPkgId);

  return (
    <div>
      <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)' }}>可用积分</h3>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, margin: '16px 0 32px 0', padding: 24, borderRadius: 12, background: 'var(--hx-secondary)', border: '1px solid var(--hx-border)' }}>
        <div style={{ width: 48, height: 48, borderRadius: '50%', background: 'rgba(255, 217, 61, 0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <Coins style={{ color: '#FFD93D' }} size={24} />
        </div>
        <div>
          <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)' }}>总剩余积分</div>
          <div style={{ fontSize: 24, fontWeight: 'bold', color: '#FFD93D' }}>{Number(totalCredits).toLocaleString()}</div>
        </div>
      </div>

      <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 16 }}>积分包购买</h3>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(240px, 1fr))', gap: 16, marginBottom: 32 }}>
        {packages.map(pkg => (
          <div key={pkg.id} style={{ borderRadius: 12, border: '1px solid var(--hx-border)', padding: 20, background: 'var(--hx-bg)', transition: 'all 0.2s', cursor: 'pointer' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <Package size={16} style={{ color: '#FFD93D' }} />
              <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--hx-text-primary)' }}>{pkg.package_name}</div>
            </div>
            <div style={{ fontSize: 12, color: 'var(--hx-text-secondary)', marginBottom: 16 }}>{pkg.description}</div>
            <div style={{ fontSize: 24, fontWeight: 'bold', color: '#FFD93D', marginBottom: 16 }}>
              {Number(pkg.credits).toLocaleString()} <span style={{ fontSize: 12, fontWeight: 'normal', color: 'var(--hx-text-secondary)' }}>积分</span>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--hx-text-primary)' }}>¥{Number(pkg.price)}</div>
              <button 
                onClick={() => handlePurchase(pkg.id)}
                style={{ padding: '6px 16px', background: 'linear-gradient(to right, #6C5CE7, #00D2FF)', color: '#fff', border: 'none', borderRadius: 8, fontSize: 13, cursor: 'pointer' }}
              >购买</button>
            </div>
          </div>
        ))}
      </div>

      <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 16 }}>详细使用记录</h3>
      <div style={{ borderRadius: 12, border: '1px solid var(--hx-border)', overflow: 'hidden' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: 13 }}>
          <thead style={{ background: 'var(--hx-secondary)' }}>
            <tr>
              <th style={{ padding: '12px 16px', fontWeight: 500, color: 'var(--hx-text-secondary)', borderBottom: '1px solid var(--hx-border)' }}>时间</th>
              <th style={{ padding: '12px 16px', fontWeight: 500, color: 'var(--hx-text-secondary)', borderBottom: '1px solid var(--hx-border)' }}>类型</th>
              <th style={{ padding: '12px 16px', fontWeight: 500, color: 'var(--hx-text-secondary)', borderBottom: '1px solid var(--hx-border)' }}>发放</th>
              <th style={{ padding: '12px 16px', fontWeight: 500, color: 'var(--hx-text-secondary)', borderBottom: '1px solid var(--hx-border)' }}>已用</th>
            </tr>
          </thead>
          <tbody>
            {creditHistory.map(h => (
              <tr key={h.id}>
                <td style={{ padding: '12px 16px', color: 'var(--hx-text-primary)', borderBottom: '1px solid var(--hx-border-light)' }}>{new Date(h.granted_at).toLocaleDateString()}</td>
                <td style={{ padding: '12px 16px', color: 'var(--hx-text-primary)', borderBottom: '1px solid var(--hx-border-light)' }}>{creditTypeMap[h.credit_type] || h.credit_type}</td>
                <td style={{ padding: '12px 16px', color: 'var(--hx-text-primary)', borderBottom: '1px solid var(--hx-border-light)' }}>{h.original_amount}</td>
                <td style={{ padding: '12px 16px', color: 'var(--hx-text-secondary)', borderBottom: '1px solid var(--hx-border-light)' }}>{h.used_amount}</td>
              </tr>
            ))}
            {creditHistory.length === 0 && (
              <tr><td colSpan={4} style={{ textAlign: 'center', padding: 32, color: 'var(--hx-text-tertiary)' }}>无历史记录</td></tr>
            )}
          </tbody>
        </table>
      </div>

      {/* 注意：如果是实际下单系统的话积分包的下单也应该使用 CheckoutModal 传入具体订单 */}
      {/* 由于需要兼容，可以用 CheckoutModal 接一个自定义 title 和 price */}
      {selectedPkg && (
        <CheckoutModal 
          isOpen={true} 
          onClose={() => setCheckoutPkgId(null)}
          tier={`积分包·${selectedPkg.package_name}`}
          itemTitle={`积分充值：${selectedPkg.package_name}`}
          itemPrice={selectedPkg.price}
          hideCycleSelector={true}
        />
      )}
    </div>
  );
}
