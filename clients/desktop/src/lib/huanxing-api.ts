/**
 * 唤星后端 API 封装
 *
 * 所有唤星后端接口统一通过此模块调用，不要在业务代码中直接写 URL。
 * 基地址从 HUANXING_CONFIG.backendBaseUrl 读取。
 */

import { HUANXING_CONFIG, type HuanxingLoginData } from '../config';

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

/**
 * 获取后端基地址
 * - 开发模式（import.meta.env.DEV）: 返回空字符串，走 Vite proxy
 *   这对 iOS 模拟器至关重要，因为模拟器中 127.0.0.1 指向自身而不是 Mac 宿主
 * - 生产构建: Tauri 桌面端直连 backendBaseUrl
 */
function baseUrl(): string {
  // 开发模式统一走 Vite proxy
  if (import.meta.env.DEV) return '';
  // 生产构建的 Tauri 桌面端直连
  const isDesktop =
    typeof window !== 'undefined' &&
    (!!((window as any).__TAURI_INTERNALS__) || !!((window as any).__TAURI__));
  return isDesktop ? HUANXING_CONFIG.backendBaseUrl : '';
}

/** 统一请求封装 */
async function request<T = unknown>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const url = `${baseUrl()}${path}`;
  const resp = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
  });

  // 401 处理：尝试刷新 token，失败则触发全局登出
  if (resp.status === 401) {
    let refreshed = false;
    try {
      const { refreshTokenNow } = await import('./token-refresh');
      refreshed = await refreshTokenNow();
    } catch {
      // token-refresh 模块不可用
    }

    if (refreshed) {
      // 用新 token 重试原请求
      const { getHuanxingSession } = await import('../config');
      const session = getHuanxingSession();
      if (session?.accessToken) {
        const retryHeaders = {
          'Content-Type': 'application/json',
          ...options.headers,
          'Authorization': `Bearer ${session.accessToken}`,
        };
        const retryResp = await fetch(url, { ...options, headers: retryHeaders });
        if (retryResp.ok) {
          return retryResp.json() as Promise<T>;
        }
      }
    }

    // 刷新失败或重试失败 → 触发全局登出
    const { clearHuanxingSession } = await import('../config');
    const { clearToken } = await import('./auth');
    clearHuanxingSession();
    clearToken();
    window.dispatchEvent(new Event('zeroclaw-unauthorized'));

    const body = await resp.json().catch(() => ({}));
    const msg = (body as any)?.msg || (body as any)?.detail || 'Token 已过期';
    throw new Error(msg);
  }

  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    const msg =
      (body as any)?.msg ||
      (body as any)?.detail ||
      (body as any)?.message ||
      `请求失败 (${resp.status})`;
    throw new Error(msg);
  }

  return resp.json() as Promise<T>;
}

/** 带 access_token 的请求 */
export async function authRequest<T = unknown>(
  path: string,
  token: string,
  options: RequestInit = {},
): Promise<T> {
  return request<T>(path, {
    ...options,
    headers: {
      Authorization: `Bearer ${token}`,
      ...options.headers,
    },
  });
}

// ---------------------------------------------------------------------------
// Auth 模块
// ---------------------------------------------------------------------------

/** 发送手机验证码 */
export async function sendVerifyCode(phone: string): Promise<void> {
  await request('/api/v1/auth/send-code', {
    method: 'POST',
    body: JSON.stringify({ phone }),
  });
}

/** 手机号 + 验证码登录 */
export async function phoneLogin(
  phone: string,
  code: string,
): Promise<HuanxingLoginData> {
  // 从本地 sidecar 获取设备指纹（启动时由 ZeroClaw 生成）
  let deviceFingerprint: string | undefined;
  let deviceName: string | undefined;
  try {
    const statusResp = await fetch(
      `${HUANXING_CONFIG.sidecarBaseUrl}/api/v1/hasn/status`,
    );
    if (statusResp.ok) {
      const statusData = await statusResp.json();
      deviceFingerprint = statusData.device_fingerprint || undefined;
      deviceName = statusData.node_name || undefined;
    }
  } catch {
    // sidecar 不可达，降级为不传指纹（后端仍可正常工作）
  }

  const resp = await request<{ data: HuanxingLoginData }>(
    '/api/v1/auth/phone-login',
    {
      method: 'POST',
      body: JSON.stringify({
        phone,
        code,
        device_fingerprint: deviceFingerprint,
        device_name: deviceName,
      }),
    },
  );
  return resp.data;
}

// ---------------------------------------------------------------------------
// User 模块（预留）
// ---------------------------------------------------------------------------

/** 获取当前用户信息 */
export async function getUserProfile(token: string) {
  return authRequest<{ data: any }>('/api/v1/sys/users/me', token);
}

/** 更新用户昵称 */
export async function updateNickname(token: string, nickname: string) {
  return authRequest('/api/v1/sys/users/me/nickname', token, {
    method: 'PUT',
    body: JSON.stringify({ nickname }),
  });
}

/** 更新用户头像 URL */
export async function updateAvatar(token: string, avatar: string) {
  return authRequest('/api/v1/sys/users/me/avatar', token, {
    method: 'PUT',
    body: JSON.stringify({ avatar }),
  });
}

/** 上传用户头像文件，返回 URL */
export async function uploadAvatar(token: string, file: Blob, filename: string = 'avatar.png'): Promise<string> {
  const formData = new FormData();
  formData.append('file', file, filename);
  const url = `${baseUrl()}/api/v1/sys/users/me/avatar/upload`;
  const resp = await fetch(url, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}` },
    body: formData,
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as any)?.msg || `上传失败 (${resp.status})`);
  }
  const result = await resp.json();
  return result.data?.url || '';
}

/** 更新用户资料（nickname/avatar/gender/birthday/bio 等） */
export async function updateProfile(token: string, profile: {
  nickname?: string;
  avatar?: string;
  gender?: string;
  birthday?: string;
  province?: string;
  city?: string;
  district?: string;
  industry?: string;
  bio?: string;
}) {
  return authRequest('/api/v1/sys/users/me/profile', token, {
    method: 'PUT',
    body: JSON.stringify(profile),
  });
}

// ---------------------------------------------------------------------------
// LLM 模块（预留）
// ---------------------------------------------------------------------------

/** LLM proxy 基地址 */
export function getLlmProxyUrl(): string {
  return HUANXING_CONFIG.llmGatewayUrl;
}

/** LLM proxy v1 地址（OpenAI 兼容） */
export function getLlmProxyV1Url(): string {
  return HUANXING_CONFIG.llmGatewayV1;
}
