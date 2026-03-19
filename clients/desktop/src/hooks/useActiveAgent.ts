/**
 * useActiveAgent.ts — 全局 Agent 名称状态
 *
 * 用 sessionStorage + 自定义事件实现跨组件同步，
 * 比 Context 轻量，不需要改 App 层级。
 */

import { useState, useEffect, useCallback } from 'react';

const STORAGE_KEY = 'huanxing:active_agent';
const EVENT_NAME = 'huanxing:agent-changed';

/** Get current active agent name */
export function getActiveAgentName(): string | null {
  return sessionStorage.getItem(STORAGE_KEY);
}

/** Set active agent name (broadcasts to all hooks) */
export function setActiveAgentName(name: string | null): void {
  if (name) {
    sessionStorage.setItem(STORAGE_KEY, name);
  } else {
    sessionStorage.removeItem(STORAGE_KEY);
  }
  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: name }));
}

/** React hook: subscribe to active agent name changes */
export function useActiveAgent(): [string | null, (name: string | null) => void] {
  const [name, setName] = useState<string | null>(() => getActiveAgentName());

  useEffect(() => {
    const handler = (e: Event) => {
      setName((e as CustomEvent).detail ?? null);
    };
    window.addEventListener(EVENT_NAME, handler);
    return () => window.removeEventListener(EVENT_NAME, handler);
  }, []);

  const update = useCallback((newName: string | null) => {
    setActiveAgentName(newName);
    setName(newName);
  }, []);

  return [name, update];
}
