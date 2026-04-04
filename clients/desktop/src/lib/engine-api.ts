/**
 * 引擎 API 统一层
 *
 * 对业务代码完全透明的通信抽象：
 * - 桌面端：通过 HTTP 调用 localhost:42620（sidecar 进程）
 * - 移动端：通过 Tauri invoke 调用 in-process engine（FFI）
 *
 * 业务代码只需调用 engineFetch / engineWebSocket，无需关心底层通信方式。
 */

import { isTauriMobile } from '@/lib/platform';

const ENGINE_PORT = 42620;
const ENGINE_BASE = `http://127.0.0.1:${ENGINE_PORT}`;

/**
 * 向引擎发送 HTTP-like 请求
 *
 * @param path  - API 路径，如 "/health", "/api/status", "/pair"
 * @param options - 类似 fetch 的选项
 * @returns 响应数据（JSON parsed）
 */
export async function engineFetch<T = any>(
  path: string,
  options?: {
    method?: string;
    body?: string | Record<string, unknown>;
    headers?: Record<string, string>;
  },
): Promise<T> {
  if (isTauriMobile()) {
    // 移动端：通过 Tauri command 转发到 in-process engine
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>('engine_request', {
      path,
      method: options?.method ?? 'GET',
      body: typeof options?.body === 'string' ? options.body : JSON.stringify(options?.body),
    });
  }

  // 桌面端：直接 HTTP 到 sidecar
  const resp = await fetch(`${ENGINE_BASE}${path}`, {
    method: options?.method ?? 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
    body: typeof options?.body === 'string'
      ? options.body
      : options?.body
        ? JSON.stringify(options.body)
        : undefined,
  });

  if (!resp.ok) {
    throw new Error(`Engine request failed: ${resp.status} ${resp.statusText}`);
  }

  const text = await resp.text();
  try {
    return JSON.parse(text);
  } catch {
    return text as unknown as T;
  }
}

/**
 * 引擎健康检查
 */
export async function engineHealthCheck(): Promise<boolean> {
  try {
    await engineFetch('/health');
    return true;
  } catch {
    return false;
  }
}

/**
 * 引擎状态查询
 */
export async function engineStatus() {
  return engineFetch<{
    model?: string;
    provider?: string;
    uptime_seconds?: number;
    memory_backend?: string;
    pid?: number;
  }>('/api/status');
}

/** WebSocket 事件监听器类型 */
export interface EngineWSListener {
  onMessage: (data: string) => void;
  onClose?: () => void;
  onError?: (error: Event | string) => void;
}

/**
 * 创建到引擎的 WebSocket 连接
 *
 * 桌面端：标准 WebSocket 连接
 * 移动端：通过 Tauri event 系统桥接
 *
 * @returns 一个带 send/close 方法的连接对象
 */
export function engineWebSocket(path: string, listener: EngineWSListener) {
  if (isTauriMobile()) {
    // 移动端：通过 Tauri event bridge
    return createTauriWSBridge(path, listener);
  }

  // 桌面端：标准 WebSocket
  const ws = new WebSocket(`ws://127.0.0.1:${ENGINE_PORT}${path}`);
  ws.onmessage = (e) => listener.onMessage(e.data);
  ws.onclose = () => listener.onClose?.();
  ws.onerror = (e) => listener.onError?.(e);

  return {
    send: (data: string) => ws.send(data),
    close: () => ws.close(),
  };
}

/**
 * 移动端 WebSocket 桥接
 * 通过 Tauri event 系统模拟 WebSocket 通信
 */
async function createTauriWSBridge(path: string, listener: EngineWSListener) {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');

  // 生成唯一连接 ID
  const connId = `ws-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  // 监听引擎发来的消息
  const unlisten = await listen<string>(`engine-ws://${connId}`, (event) => {
    listener.onMessage(event.payload);
  });

  // 通知引擎建立 WS 连接
  await invoke('engine_ws_connect', { path, connId });

  return {
    send: (data: string) => {
      invoke('engine_ws_send', { connId, data }).catch((e) =>
        listener.onError?.(String(e)),
      );
    },
    close: () => {
      invoke('engine_ws_close', { connId }).catch(() => {});
      unlisten();
      listener.onClose?.();
    },
  };
}
