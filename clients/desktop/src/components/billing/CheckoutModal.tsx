import { useEffect, useState, useRef, useCallback } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { Loader2, CheckCircle2, XCircle, RefreshCw, CreditCard, X } from 'lucide-react';
import { createOrder, getOrderStatus, cancelOrder, getPayChannels } from '../../lib/subscription-api';
import type { HxCreateOrderResponse, HxPayChannel } from '../../lib/subscription-api';
import { useSubscriptionStore } from '../../stores/useSubscriptionStore';

function isSubscriptionChannel(code: string) {
  return code.includes('papay') || code.includes('cycle') || code.includes('contract') || code.includes('sub');
}
function getChannelGroup(code: string): 'subscribe' | 'onetime' {
  return isSubscriptionChannel(code) ? 'subscribe' : 'onetime';
}

interface CheckoutModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** 当支付类型为 subscription 时使用 */
  tier?: string;
  cycle?: 'monthly' | 'yearly';
  /** 是否隐藏周期选择器（例如充值积分时） */
  hideCycleSelector?: boolean;
  itemTitle?: string;
  itemPrice?: number;
}

type Step = 'select' | 'paying' | 'success' | 'failed';

export default function CheckoutModal({
  isOpen, onClose, tier, cycle: defaultCycle = 'monthly', hideCycleSelector, itemTitle, itemPrice
}: CheckoutModalProps) {
  const [channels, setChannels] = useState<HxPayChannel[]>([]);
  const [selectedCode, setSelectedCode] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [step, setStep] = useState<Step>('select');
  const [orderData, setOrderData] = useState<HxCreateOrderResponse | null>(null);
  const [errorMsg, setErrorMsg] = useState('');
  const [agreeContract, setAgreeContract] = useState(false);
  const [cycle, setCycle] = useState<'monthly' | 'yearly'>(defaultCycle);
  
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const fetchInfo = useSubscriptionStore(s => s.fetchInfo);
  const fetchCreditHistory = useSubscriptionStore(s => s.fetchCreditHistory);

  // Initialize channels
  useEffect(() => {
    if (isOpen && channels.length === 0) {
      getPayChannels().then(list => {
        setChannels(list);
        if (list.length > 0) setSelectedCode(list[0].code);
      }).catch(err => console.error('Failed to get channels', err));
    }
  }, [isOpen]);

  // Handle polling
  const startPolling = useCallback((orderNo: string) => {
    if (pollingRef.current) clearInterval(pollingRef.current);
    pollingRef.current = setInterval(async () => {
      try {
        const res = await getOrderStatus(orderNo);
        if (res.status === 1) { // 支付成功
          clearInterval(pollingRef.current!);
          pollingRef.current = null;
          setStep('success');
          fetchInfo();
          fetchCreditHistory();
        } else if (res.status >= 3) { // 订单关闭
          clearInterval(pollingRef.current!);
          pollingRef.current = null;
          setStep('failed');
          setErrorMsg('订单已过期或关闭');
        }
      } catch {
        // ...
      }
    }, 3000);
  }, [fetchInfo, fetchCreditHistory]);

  useEffect(() => {
    return () => {
      if (pollingRef.current) clearInterval(pollingRef.current);
    };
  }, []);

  const reset = () => {
    setStep('select');
    setOrderData(null);
    setErrorMsg('');
    if (pollingRef.current) clearInterval(pollingRef.current);
  };

  const internalOnClose = () => {
    if (orderData && step === 'paying') {
      try { cancelOrder(orderData.order_no); } catch { /* ignore */ }
    }
    reset();
    onClose();
  };

  const handlePay = async () => {
    if (!selectedCode || !tier) return;
    if (isSubscriptionChannel(selectedCode) && !agreeContract) {
      setErrorMsg('请先阅读并同意自动续费服务协议');
      return;
    }
    setLoading(true);
    setErrorMsg('');
    try {
      const res = await createOrder({
        tier,
        billing_cycle: cycle,
        channel_code: selectedCode,
        auto_renew: isSubscriptionChannel(selectedCode)
      });
      setOrderData(res);
      setStep('paying');
      
      // 打开桌面浏览器
      if (res.pay_url) {
        try {
          // 尝试使用 tauri 壳打开系统浏览器
          const { open } = await import('@tauri-apps/plugin-shell');
          open(res.pay_url);
        } catch {
          window.open(res.pay_url, '_blank');
        }
      }
      startPolling(res.order_no);
    } catch (e) {
      setErrorMsg(e instanceof Error ? e.message : '创建订单失败');
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  const onetimeChannels = channels.filter(ch => getChannelGroup(ch.code) === 'onetime');
  const subscribeChannels = channels.filter(ch => getChannelGroup(ch.code) === 'subscribe');
  const isSub = isSubscriptionChannel(selectedCode);

  return (
    <div className="hx-modal-overlay" style={{
      position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
      backgroundColor: 'rgba(0,0,0,0.6)', backdropFilter: 'blur(4px)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 999
    }}>
      <div className="hx-card" style={{ width: 420, maxWidth: '90vw', position: 'relative', overflow: 'hidden' }}>
        <button onClick={internalOnClose} style={{ position: 'absolute', right: 16, top: 16, background: 'none', border: 'none', color: 'var(--hx-text-secondary)', cursor: 'pointer' }}>
          <X size={20} />
        </button>

        <h2 style={{ fontSize: 18, fontWeight: 600, margin: '0 0 24px 0', color: 'var(--hx-text-primary)' }}>收银台</h2>

        <div style={{ padding: '16px', background: 'var(--hx-secondary)', borderRadius: 8, marginBottom: 24, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div>
            <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)' }}>商品详情</div>
            <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--hx-text-primary)', marginTop: 4 }}>
              {itemTitle || `唤星AI · ${tier} 会员`}
            </div>
            {!hideCycleSelector && <div style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginTop: 2 }}>{cycle === 'monthly' ? '月付' : '年付'}</div>}
          </div>
          <div style={{ textAlign: 'right' }}>
            <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)' }}>需支付</div>
            <div style={{ fontSize: 24, fontWeight: 'bold', color: '#6C5CE7', marginTop: 4 }}>
              ¥{orderData ? (orderData.pay_amount / 100).toFixed(2) : itemPrice?.toFixed(2) || '--'}
            </div>
          </div>
        </div>

        {step === 'select' && (
          <div>
            {!hideCycleSelector && (
              <div style={{ display: 'flex', gap: 8, marginBottom: 20 }}>
                <button onClick={() => setCycle('monthly')} style={{ flex: 1, padding: 10, borderRadius: 8, border: `1px solid ${cycle === 'monthly' ? '#6C5CE7' : 'var(--hx-border)'}`, background: cycle === 'monthly' ? 'rgba(108, 92, 231, 0.1)' : 'transparent', color: 'var(--hx-text-primary)' }}>
                  月付
                </button>
                <button onClick={() => setCycle('yearly')} style={{ flex: 1, padding: 10, borderRadius: 8, border: `1px solid ${cycle === 'yearly' ? '#6C5CE7' : 'var(--hx-border)'}`, background: cycle === 'yearly' ? 'rgba(108, 92, 231, 0.1)' : 'transparent', color: 'var(--hx-text-primary)' }}>
                  年付 (最高省25%)
                </button>
              </div>
            )}

            <div style={{ fontSize: 14, fontWeight: 500, marginBottom: 12, color: 'var(--hx-text-primary)' }}>支付方式</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 20 }}>
              {channels.map(ch => (
                <button
                  key={ch.code}
                  onClick={() => { setSelectedCode(ch.code); setErrorMsg(''); }}
                  style={{
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px',
                    borderRadius: 8, border: `1px solid ${selectedCode === ch.code ? '#6C5CE7' : 'var(--hx-border)'}`,
                    background: selectedCode === ch.code ? 'rgba(108, 92, 231, 0.05)' : 'var(--hx-bg)'
                  }}
                >
                  <span style={{ fontSize: 14, color: 'var(--hx-text-primary)' }}>{ch.name}</span>
                  <div style={{ width: 18, height: 18, borderRadius: '50%', border: `2px solid ${selectedCode === ch.code ? '#6C5CE7' : 'var(--hx-border)'}`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                    {selectedCode === ch.code && <div style={{ width: 8, height: 8, borderRadius: '50%', background: '#6C5CE7' }} />}
                  </div>
                </button>
              ))}
            </div>

            {isSub && (
              <label style={{ display: 'flex', alignItems: 'start', gap: 8, fontSize: 12, color: 'var(--hx-text-secondary)', marginBottom: 20, cursor: 'pointer' }}>
                <input type="checkbox" checked={agreeContract} onChange={(e) => { setAgreeContract(e.target.checked); setErrorMsg(''); }} />
                <span>我已阅读并同意 <a href="#" style={{ color: '#6C5CE7' }}>《自动续费服务协议》</a>，授权在订阅到期时自动从我的账户中扣款续费，可随时在这取消。</span>
              </label>
            )}

            {errorMsg && <div style={{ color: '#ef4444', fontSize: 13, marginBottom: 12 }}>{errorMsg}</div>}

            <button
              onClick={handlePay}
              disabled={loading || !selectedCode}
              style={{
                width: '100%', padding: '12px', borderRadius: 8, background: 'linear-gradient(to bottom right, #6C5CE7, #00D2FF)',
                color: '#fff', border: 'none', fontWeight: 600, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                cursor: (loading || !selectedCode) ? 'not-allowed' : 'pointer', opacity: (loading || !selectedCode) ? 0.7 : 1
              }}
            >
              {loading ? <Loader2 className="animate-spin" size={18} /> : <CreditCard size={18} />}
              确认{isSub ? '订阅' : '支付'}
            </button>
          </div>
        )}

        {step === 'paying' && orderData && (
          <div style={{ textAlign: 'center' }}>
            {orderData.qr_code_url ? (
              <div style={{ marginBottom: 20 }}>
                <div style={{ fontSize: 14, color: 'var(--hx-text-secondary)', marginBottom: 12 }}>
                  请使用 {orderData.channel_code.startsWith('wx') ? '微信' : '支付宝'} 扫码支付
                </div>
                <div style={{ background: '#fff', padding: 16, borderRadius: 12, display: 'inline-block' }}>
                  <QRCodeSVG value={orderData.qr_code_url} size={160} />
                </div>
              </div>
            ) : (
              <div style={{ marginBottom: 20 }}>
                <div style={{ fontSize: 15, fontWeight: 500, color: 'var(--hx-text-primary)', marginBottom: 8 }}>已尝试在浏览器打开支付网关</div>
                <div style={{ fontSize: 13, color: 'var(--hx-text-secondary)' }}>如果在外部浏览器中无法拉起支付，请点击下方刷新。</div>
              </div>
            )}
            <div style={{ display: 'flex', justifyContent: 'center', gap: 12 }}>
              <button 
                onClick={() => { setStep('success'); if (pollingRef.current) clearInterval(pollingRef.current); fetchInfo(); }}
                style={{ padding: '8px 16px', borderRadius: 8, border: '1px solid var(--hx-border)', background: 'var(--hx-bg)', color: 'var(--hx-text-primary)' }}
              >
                我已支付
              </button>
              <button 
                onClick={reset}
                style={{ padding: '8px 16px', borderRadius: 8, border: 'none', background: 'transparent', color: '#ef4444' }}
              >
                取消
              </button>
            </div>
          </div>
        )}

        {step === 'success' && (
          <div style={{ textAlign: 'center', padding: '20px 0' }}>
            <CheckCircle2 size={64} style={{ color: '#10b981', margin: '0 auto 16px auto' }} />
            <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 8 }}>支付成功！</div>
            <div style={{ fontSize: 14, color: 'var(--hx-text-secondary)', marginBottom: 24 }}>您的账户权益已更新。</div>
            <button 
              onClick={internalOnClose}
              style={{ padding: '10px 24px', borderRadius: 8, background: '#6C5CE7', color: '#fff', border: 'none', fontWeight: 600 }}
            >
              完成
            </button>
          </div>
        )}

        {step === 'failed' && (
          <div style={{ textAlign: 'center', padding: '20px 0' }}>
            <XCircle size={64} style={{ color: '#ef4444', margin: '0 auto 16px auto' }} />
            <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--hx-text-primary)', marginBottom: 8 }}>支付未完成</div>
            <div style={{ fontSize: 14, color: 'var(--hx-text-secondary)', marginBottom: 24 }}>{errorMsg}</div>
            <button 
              onClick={reset}
              style={{ padding: '10px 24px', borderRadius: 8, background: '#6C5CE7', color: '#fff', border: 'none', fontWeight: 600 }}
            >
              重新尝试
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
