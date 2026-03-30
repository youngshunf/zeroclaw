import { authRequest } from './huanxing-api';
import { getHuanxingSession, HUANXING_CONFIG } from '../config';

// 我们不直接 import 外部项目类型，最好是在这里声明或者直接引用（如果 TS 配置允许跨 workspace 引用）。
// 为确保无类型报错，部分返回类型可以直接在当前应用内复用或定义。
// 直接 copy 一份基础类型定义
export interface HxSubscriptionInfo {
  tier_name: string;
  tier_display_name?: string;
  subscription_end_date?: string | null;
  subscription_status: number;
  current_credits: number;
  total_credits: number;
  used_credits: number;
  balances?: Array<{
    id: number;
    credit_type: string;
    original_amount: string | number;
    used_amount: string | number;
    remaining_amount: string | number;
    expires_at: string | null;
  }>;
}

export interface HxSubscriptionTier {
  id: number;
  tier_name: string;
  display_name: string;
  monthly_price: number;
  yearly_price?: number;
  yearly_discount?: number;
  monthly_credits: number;
}

export interface HxCreditPackage {
  id: number;
  package_name: string;
  description: string;
  price: number;
  credits: number;
  bonus_credits: number;
}

export interface HxPaymentResult {
  success: boolean;
  message?: string;
}

export interface HxUpgradeCalculation {
  original_price: number;
  final_price: number;
  deduction_amount: number;
  message: string;
}

export interface HxCreditHistory {
  id: number;
  credit_type: string;
  original_amount: string | number;
  used_amount: string | number;
  description?: string;
  granted_at: string;
}

export interface HxPayChannel {
  code: string;
  name: string;
  type: string;
  icon?: string;
}

export interface HxCreateOrderResponse {
  order_no: string;
  pay_url?: string;
  qr_code_url?: string;
  pay_amount: number;
  channel_code: string;
}

export interface HxOrderStatusResponse {
  status: number;
  paid_at?: string;
  fail_reason?: string;
}

/** 包装使用 Session token 的请求 */
async function req<T>(path: string, options: RequestInit = {}): Promise<T> {
  const session = getHuanxingSession();
  if (!session?.accessToken) {
    throw new Error('用户未登录');
  }
  // authRequest expects absolute path from the domain if we don't pass full url, but huanxing-api prefixes backendBaseUrl
  const resp = await authRequest<{ data: T }>(path, session.accessToken, options);
  return resp as any; // huanxing-api.ts -> `request` -> `return resp.json()` which is { code, msg, data } or unwrapped? 
  // Let's check huanxing-api: return resp.json() directly. Usually returns `{ code, msg, data }`.
}

/** 由于后端的响应包裹通常是 { code, msg, data }，这里做一个通用的解包函数 */
async function fetchApi<T>(path: string, options: RequestInit = {}): Promise<T> {
  const session = getHuanxingSession();
  if (!session?.accessToken) {
    throw new Error('未登录或会话已过期');
  }
  const isDesktop = typeof window !== 'undefined' && 
    (!!((window as any).__TAURI_INTERNALS__) || !!((window as any).__TAURI__));
  const baseUrl = isDesktop ? HUANXING_CONFIG.backendBaseUrl : '';
  const url = `${baseUrl}${path}`;

  const resp = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${session.accessToken}`,
      'X-App-Code': 'huanxing',
      ...options.headers,
    },
  });

  if (!resp.ok) {
    throw new Error(`请求失败 (${resp.status})`);
  }

  const json = await resp.json();
  if (json.code !== 0 && json.code !== 200) {
    throw new Error(json.msg || json.message || '请求失败');
  }
  return json.data as T;
}

// ===== 需要登录（app/）=====

export function getMyInfo() {
  return fetchApi<HxSubscriptionInfo>('/api/v1/user_tier/app/subscription/info', { method: 'GET' });
}

export function getBalanceHistory() {
  return fetchApi<HxCreditHistory[]>('/api/v1/user_tier/app/subscription/balances/history', { method: 'GET' });
}

export function calculateUpgrade(tierName: string, subscriptionType: string) {
  return fetchApi<HxUpgradeCalculation>('/api/v1/user_tier/app/subscription/upgrade/calculate', {
    method: 'POST',
    body: JSON.stringify({ tier_name: tierName, subscription_type: subscriptionType }),
  });
}

export function purchaseCredits(packageId: number) {
  return fetchApi<HxPaymentResult>('/api/v1/user_tier/app/subscription/purchase', {
    method: 'POST',
    body: JSON.stringify({ package_id: packageId }),
  });
}

// ===== 支付通道与订单 =====

export function getPayChannels() {
  return fetchApi<HxPayChannel[]>('/api/v1/pay/channels', { method: 'GET' });
}

export function createOrder(params: {
  tier: string;
  billing_cycle: string;
  channel_code: string;
  auto_renew: boolean;
}) {
  return fetchApi<HxCreateOrderResponse>('/api/v1/pay/create_order', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}

export function getOrderStatus(orderNo: string) {
  return fetchApi<HxOrderStatusResponse>(`/api/v1/pay/order/${encodeURIComponent(orderNo)}/status`, { method: 'GET' });
}

export function cancelOrder(orderNo: string) {
  return fetchApi<void>(`/api/v1/pay/order/${encodeURIComponent(orderNo)}/cancel`, { method: 'POST' });
}

// ===== 不需要登录（open/）这里为了简化，依然使用带有 authorization 的 fetchApi =====

export function getTiers() {
  return fetchApi<HxSubscriptionTier[]>('/api/v1/user_tier/open/tiers', { method: 'GET' });
}

export function getPackages() {
  return fetchApi<HxCreditPackage[]>('/api/v1/user_tier/open/packages', { method: 'GET' });
}
