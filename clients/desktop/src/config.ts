/**
 * 唤星桌面端配置
 *
 * 所有环境相关的地址、默认值集中在此文件管理。
 * 后期修改只需改这一个文件。
 */

export const HUANXING_CONFIG = {
  /** 唤星后端服务基地址 */
  backendBaseUrl: 'http://127.0.0.1:8020',

  /** 唤星云服务基地址（线上） */
  cloudBaseUrl: 'https://huanxing.dcfuture.cn',

  /** LLM 网关基地址 */
  llmGatewayUrl: 'http://127.0.0.1:3180',

  /** LLM 网关 v1 路径（用于 ZeroClaw openai_compat provider） */
  llmGatewayV1: 'http://127.0.0.1:3180/v1',

  /** 默认 LLM 模型 */
  defaultModel: 'claude-sonnet-4-6',

  /** 默认 LLM provider 名称 */
  defaultProvider: 'custom:http://127.0.0.1:3180/v1',

  /** 降级 fallback provider (云端直连) */
  fallbackProvider: 'custom:https://llm.dcfuture.cn/v1',

  /** 嵌入向量 provider (云端直连) */
  embeddingProvider: 'custom:https://llm.dcfuture.cn/v1',

  /** 默认温度 */
  defaultTemperature: 0.7,

  /** ZeroClaw sidecar 本地地址（唤星专属端口） */
  sidecarBaseUrl: 'http://localhost:42620',

  /** 产品名称 */
  productName: '唤星',

  /** 唤星官网域名（用于分享链接、外部跳转等） */
  siteUrl: 'https://huanxing.dcfuture.cn',

  /** Agent 默认名称 */
  defaultAgentName: '唤星AI助手',
} as const;

/** 登录成功后后端返回的完整数据 */
export interface HuanxingLoginData {
  access_token: string;
  access_token_expire_time: string;
  refresh_token: string;
  refresh_token_expire_time: string;
  llm_token: string;
  hasn_node_key?: string;
  owner_key?: string;
  agent_key: string;
  gateway_token: string;
  is_new_user: boolean;
  user: {
    uuid: string;
    username: string;
    nickname: string;
    phone: string;
    email?: string;
    avatar?: string;
    is_new_user: boolean;
  };
}

/** 本地存储的唤星会话信息 */
export interface HuanxingSession {
  accessToken: string;
  accessTokenExpireTime: string;   // ISO datetime
  refreshToken: string;
  refreshTokenExpireTime: string;  // ISO datetime
  llmToken: string;
  hasnNodeKey?: string;
  ownerKey?: string;
  agentKey: string;
  gatewayToken: string;
  user: HuanxingLoginData['user'];
  isNewUser: boolean;
  loginAt: string;  // ISO timestamp
}

const SESSION_KEY = 'huanxing_session';

/** 保存唤星会话（用 localStorage 持久化，关标签页不丢） */
export function saveHuanxingSession(data: HuanxingLoginData): HuanxingSession {
  const session: HuanxingSession = {
    accessToken: data.access_token,
    accessTokenExpireTime: data.access_token_expire_time,
    refreshToken: data.refresh_token,
    refreshTokenExpireTime: data.refresh_token_expire_time,
    llmToken: data.llm_token,
    hasnNodeKey: data.hasn_node_key,
    ownerKey: data.owner_key,
    agentKey: data.agent_key,
    gatewayToken: data.gateway_token,
    user: data.user,
    isNewUser: data.is_new_user,
    loginAt: new Date().toISOString(),
  };
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // ignore
  }
  return session;
}

/** 读取唤星会话 */
export function getHuanxingSession(): HuanxingSession | null {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as HuanxingSession;
  } catch {
    return null;
  }
}

/** 更新 access token（refresh 后调用） */
export function updateAccessToken(newToken: string, expireTime: string, newRefreshToken?: string, newRefreshExpireTime?: string): void {
  const session = getHuanxingSession();
  if (!session) return;
  session.accessToken = newToken;
  session.accessTokenExpireTime = expireTime;
  if (newRefreshToken) {
    session.refreshToken = newRefreshToken;
  }
  if (newRefreshExpireTime) {
    session.refreshTokenExpireTime = newRefreshExpireTime;
  }
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // ignore
  }
}

/** 清除唤星会话 */
export function clearHuanxingSession(): void {
  try {
    localStorage.removeItem(SESSION_KEY);
  } catch {
    // ignore
  }
}

/**
 * 确保 API URL 是绝对路径
 * Tauri 环境中，`<img src="/api/...">` 会请求 `tauri://localhost/api/...` 导致未能加载正确图片。
 * 此函数用于将此类相对路径拼接上实际的后端 baseUrl。
 */
export function resolveApiUrl(url: string | null | undefined): string {
  if (!url) return '';
  if (url.startsWith('http://qncdn.dcfuture.cn')) {
    url = url.replace('http://', 'https://');
  }
  if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('data:') || url.startsWith('blob:')) {
    return url;
  }

  // 判断是否为 Tauri 桌面端
  const isDesktop = typeof window !== 'undefined' &&
    (!!((window as any).__TAURI_INTERNALS__) || !!((window as any).__TAURI__));

  if (isDesktop) {
    // 区分本地 sidecar 接口与云端 backend 接口
    // Agent 内部文件、会话状态等来自 sidecar
    const isLocalSidecarUrl = url.startsWith('/api/agents') || url.startsWith('/api/sessions') || url.startsWith('/api/hub/');
    const base = isLocalSidecarUrl ? HUANXING_CONFIG.sidecarBaseUrl : HUANXING_CONFIG.backendBaseUrl;

    const baseClean = base.replace(/\/$/, '');
    const path = url.startsWith('/') ? url : `/${url}`;
    return `${baseClean}${path}`;
  }

  return url;
}
