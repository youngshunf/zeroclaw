/**
 * connection.ts — WebSocket connection URL builder for multi-agent support.
 *
 * Desktop (Sidecar): always connect to localhost:42620
 * Web/Mobile: connect to current origin
 */

/**
 * Get the WebSocket base URL for connecting to an agent.
 * - Desktop app with Sidecar → `ws://localhost:42620`
 * - Web browser → derives from current page origin
 */
export function getWsBaseUrl(): string {
  // Check if running inside Tauri (desktop app with local Sidecar)
  if (typeof window !== 'undefined' && '__TAURI__' in window) {
    return 'ws://localhost:42620';
  }

  // Web: derive from current origin
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}`;
}

/**
 * Build a full WebSocket URL for a chat session with an agent.
 */
export function buildWsChatUrl(params: {
  sessionId: string;
  agentId?: string;
  token?: string;
}): string {
  const base = getWsBaseUrl();
  const searchParams = new URLSearchParams();
  searchParams.set('session_id', params.sessionId);

  if (params.agentId) {
    searchParams.set('agent_id', params.agentId);
  }
  if (params.token) {
    searchParams.set('token', params.token);
  }

  return `${base}/ws/chat?${searchParams.toString()}`;
}
