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
 * - Tauri 桌面端：直连 backendBaseUrl（无跨域限制）
 * - 浏览器开发环境：返回空字符串，走 Vite proxy（避免跨域）
 */
function baseUrl(): string {
  const isDesktop =
    typeof window !== 'undefined' && !!(window as any).__TAURI__;
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
async function authRequest<T = unknown>(
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
  const resp = await request<{ data: HuanxingLoginData }>(
    '/api/v1/auth/phone-login',
    {
      method: 'POST',
      body: JSON.stringify({ phone, code }),
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
