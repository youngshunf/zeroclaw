/**
 * 唤星 Token 自动刷新机制
 *
 * - access_token 即将过期前自动刷新
 * - 401 响应时立即尝试刷新
 * - refresh_token 也过期则跳转登录
 * - 并发请求时只刷新一次（防重入）
 */

import { getHuanxingSession, updateAccessToken, clearHuanxingSession, HUANXING_CONFIG } from '../config';
import { setToken, clearToken } from '@/lib/auth';

/** 提前多少毫秒刷新（默认 2 分钟） */
const REFRESH_BEFORE_MS = 2 * 60 * 1000;

/** 定时器 ID */
let refreshTimer: ReturnType<typeof setTimeout> | null = null;

/** 刷新锁：防止并发刷新 */
let refreshPromise: Promise<boolean> | null = null;

/**
 * 启动自动刷新定时器
 * 登录成功后调用
 */
export function startTokenRefresh(): void {
  stopTokenRefresh();
  scheduleRefresh();
}

/**
 * 停止自动刷新
 * 登出时调用
 */
export function stopTokenRefresh(): void {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
}

/**
 * 手动触发 token 刷新（如收到 401 时调用）
 * 返回 true 表示刷新成功，false 表示需要重新登录
 */
export async function refreshTokenNow(): Promise<boolean> {
  // 防止并发刷新
  if (refreshPromise) return refreshPromise;

  refreshPromise = doRefresh();
  try {
    return await refreshPromise;
  } finally {
    refreshPromise = null;
  }
}

// ── 内部实现 ──

function scheduleRefresh(): void {
  const session = getHuanxingSession();
  if (!session?.accessTokenExpireTime) return;

  const expireTime = new Date(session.accessTokenExpireTime).getTime();
  const now = Date.now();
  const delay = Math.max(expireTime - now - REFRESH_BEFORE_MS, 1000); // 最少 1 秒

  console.log(`[token-refresh] 下次刷新: ${Math.round(delay / 1000)}s 后`);

  refreshTimer = setTimeout(async () => {
    const success = await refreshTokenNow();
    if (success) {
      scheduleRefresh(); // 刷新成功，安排下一次
    } else {
      console.warn('[token-refresh] 刷新失败，需要重新登录');
      handleLogout();
    }
  }, delay);
}

async function doRefresh(): Promise<boolean> {
  const session = getHuanxingSession();
  if (!session?.refreshToken) {
    console.warn('[token-refresh] 无 refresh token');
    return false;
  }

  // 检查 refresh token 是否也过期
  if (session.refreshTokenExpireTime) {
    const refreshExpire = new Date(session.refreshTokenExpireTime).getTime();
    if (Date.now() > refreshExpire) {
      console.warn('[token-refresh] refresh token 已过期');
      return false;
    }
  }

  try {
    const resp = await fetch(`${HUANXING_CONFIG.backendBaseUrl}/api/v1/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: session.refreshToken }),
    });

    if (!resp.ok) {
      console.warn(`[token-refresh] 刷新请求失败: ${resp.status}`);
      if (resp.status === 401 || resp.status === 403) {
        return false; // refresh token 无效，需要重新登录
      }
      return false;
    }

    const result = await resp.json();
    const data = result.data;

    if (!data?.access_token) {
      console.warn('[token-refresh] 响应缺少 access_token');
      return false;
    }

    // 更新本地存储
    updateAccessToken(
      data.access_token,
      data.access_token_expire_time,
      data.new_refresh_token || undefined,
      data.new_refresh_token_expire_time || undefined,
    );

    // 同步更新 ZeroClaw auth 模块的 token
    setToken(data.access_token);

    console.log('[token-refresh] ✅ token 刷新成功');
    return true;
  } catch (err) {
    console.warn('[token-refresh] 网络错误:', err);
    return false;
  }
}

function handleLogout(): void {
  stopTokenRefresh();
  clearHuanxingSession();
  clearToken();
  // 触发 ZeroClaw 的 401 事件，App.tsx 会跳回登录页
  window.dispatchEvent(new Event('zeroclaw-unauthorized'));
}
