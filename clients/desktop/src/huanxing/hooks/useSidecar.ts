/**
 * useSidecar — ZeroClaw sidecar 进程管理 Hook
 *
 * 开发模式: 通过 HTTP 直连 sidecar API
 * 生产模式 (Tauri): 通过 invoke() IPC 调用 SidecarManager
 */

import { useState, useEffect, useCallback, useRef } from 'react';

// ── 类型定义 ──────────────────────────────────────────────

export interface SidecarStatus {
  running: boolean;
  pid: number | null;
  port: number;
  model: string | null;
  provider: string | null;
  uptime_seconds: number | null;
  memory_backend: string | null;
  restart_count: number;
  version: string | null;
}

export interface QuickConfig {
  default_model: string | null;
  default_temperature: number | null;
  autonomy_level: string | null;
  gateway_port: number | null;
}

export interface SidecarState {
  status: SidecarStatus | null;
  loading: boolean;
  starting: boolean;
  stopping: boolean;
  logs: string[];
  error: string | null;
  config: QuickConfig | null;
  configLoading: boolean;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  restart: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  refreshLogs: () => Promise<void>;
  clearLogs: () => void;
  loadConfig: () => Promise<void>;
  saveConfig: (config: QuickConfig) => Promise<void>;
}

// ── Tauri IPC 检测 ──────────────────────────────────────

function isTauri(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = (window as any).__TAURI_INTERNALS__;
  if (!internals?.invoke) throw new Error('Tauri not available');
  return internals.invoke(cmd, args) as Promise<T>;
}

async function tauriListen(_event: string, _handler: (payload: any) => void): Promise<() => void> {
  // TODO: 接入 Tauri v2 事件监听（需要 @tauri-apps/api）
  // 目前依靠轮询机制获取状态更新
  return () => {};
}

// ── HTTP fallback (开发模式) ─────────────────────────────

async function httpGetStatus(): Promise<SidecarStatus> {
  try {
    const resp = await fetch('/api/status');
    if (!resp.ok) throw new Error(`${resp.status}`);
    const data = await resp.json();
    return {
      running: true,
      pid: null,
      port: data.gateway_port ?? 42620,
      model: data.model ?? null,
      provider: data.provider ?? null,
      uptime_seconds: data.uptime_seconds ?? null,
      memory_backend: data.memory_backend ?? null,
      restart_count: 0,
      version: null,
    };
  } catch {
    return {
      running: false,
      pid: null,
      port: 42620,
      model: null,
      provider: null,
      uptime_seconds: null,
      memory_backend: null,
      restart_count: 0,
      version: null,
    };
  }
}

// ── Hook 实现 ────────────────────────────────────────────

export function useSidecar(): SidecarState {
  const [status, setStatus] = useState<SidecarStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState<QuickConfig | null>(null);
  const [configLoading, setConfigLoading] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // ── 状态刷新 ──
  const refreshStatus = useCallback(async () => {
    try {
      if (isTauri()) {
        const s = await tauriInvoke<SidecarStatus>('get_zeroclaw_status');
        setStatus(s);
      } else {
        const s = await httpGetStatus();
        setStatus(s);
      }
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : '获取状态失败');
    }
  }, []);

  // ── 日志刷新 ──
  const refreshLogs = useCallback(async () => {
    if (!isTauri()) return; // 开发模式暂无日志
    try {
      const lines = await tauriInvoke<string[]>('get_zeroclaw_logs', { lines: 200 });
      setLogs(lines);
    } catch {
      // ignore
    }
  }, []);

  // ── 启动 ──
  const start = useCallback(async () => {
    setStarting(true);
    setError(null);
    try {
      if (isTauri()) {
        const s = await tauriInvoke<SidecarStatus>('start_zeroclaw');
        setStatus(s);
      } else {
        setError('开发模式下请手动启动 sidecar');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '启动失败');
    } finally {
      setStarting(false);
    }
  }, []);

  // ── 停止 ──
  const stop = useCallback(async () => {
    setStopping(true);
    setError(null);
    try {
      if (isTauri()) {
        await tauriInvoke<void>('stop_zeroclaw');
        setStatus((prev) => prev ? { ...prev, running: false, pid: null } : null);
      } else {
        setError('开发模式下请手动停止 sidecar');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '停止失败');
    } finally {
      setStopping(false);
    }
  }, []);

  // ── 重启 ──
  const restart = useCallback(async () => {
    setStarting(true);
    setError(null);
    try {
      if (isTauri()) {
        const s = await tauriInvoke<SidecarStatus>('restart_zeroclaw');
        setStatus(s);
      } else {
        setError('开发模式下请手动重启 sidecar');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : '重启失败');
    } finally {
      setStarting(false);
    }
  }, []);

  // ── 清空日志 ──
  const clearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  // ── 初始化 + 轮询 ──
  useEffect(() => {
    let mounted = true;

    const init = async () => {
      await refreshStatus();
      if (mounted) setLoading(false);
    };
    init();

    // 每 10 秒刷新状态
    pollRef.current = setInterval(refreshStatus, 10_000);

    return () => {
      mounted = false;
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [refreshStatus]);

  // ── Tauri 事件监听 ──
  useEffect(() => {
    if (!isTauri()) return;

    const unlisteners: (() => void)[] = [];

    // 状态变化
    tauriListen('sidecar://status-changed', (payload: any) => {
      setStatus((prev) => ({
        ...(prev ?? {
          port: 42620,
          restart_count: 0,
          version: null,
          provider: null,
          uptime_seconds: null,
          memory_backend: null,
        }),
        running: payload.running,
        pid: payload.pid ?? null,
        port: payload.port ?? 42620,
        model: payload.model ?? prev?.model ?? null,
      }));
    }).then((u) => unlisteners.push(u));

    // 日志
    tauriListen('sidecar://log', (line: string) => {
      setLogs((prev) => {
        const next = [...prev, line];
        // 前端也限制 500 行
        return next.length > 500 ? next.slice(next.length - 500) : next;
      });
    }).then((u) => unlisteners.push(u));

    // 崩溃
    tauriListen('sidecar://crash', (payload: any) => {
      if (!payload.will_restart) {
        setError(`引擎异常退出 (code: ${payload.exit_code})，已达最大重启次数`);
      }
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  // ── 配置管理 ──
  const loadConfig = useCallback(async () => {
    if (!isTauri()) return;
    setConfigLoading(true);
    try {
      const c = await tauriInvoke<QuickConfig>('get_zeroclaw_config');
      setConfig(c);
    } catch (e) {
      setError(e instanceof Error ? e.message : '读取配置失败');
    } finally {
      setConfigLoading(false);
    }
  }, []);

  const saveConfig = useCallback(async (newConfig: QuickConfig) => {
    if (!isTauri()) {
      setError('开发模式下请手动编辑 ~/.zeroclaw/config.toml');
      return;
    }
    setConfigLoading(true);
    setError(null);
    try {
      const s = await tauriInvoke<SidecarStatus>('update_zeroclaw_config', { config: newConfig });
      setStatus(s);
      setConfig(newConfig);
    } catch (e) {
      setError(e instanceof Error ? e.message : '保存配置失败');
    } finally {
      setConfigLoading(false);
    }
  }, []);

  return {
    status,
    loading,
    starting,
    stopping,
    logs,
    error,
    config,
    configLoading,
    start,
    stop,
    restart,
    refreshStatus,
    refreshLogs,
    clearLogs,
    loadConfig,
    saveConfig,
  };
}
