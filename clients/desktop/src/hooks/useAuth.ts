import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  type ReactNode,
} from 'react';
import React from 'react';
import {
  getToken as readToken,
  setToken as writeToken,
  clearToken as removeToken,
  isAuthenticated as checkAuth,
  TOKEN_STORAGE_KEY,
} from '../lib/auth';
import { pair as apiPair, getPublicHealth } from '../lib/api';

/**
 * 唤星桌面端：如果 sessionStorage 无 token 但 localStorage 有 huanxing_session，
 * 则从中恢复 accessToken 到 sessionStorage，确保全页刷新后不丢失登录态。
 */
function restoreHuanxingToken(): void {
  if (readToken()) return; // 已有 token，不需要恢复
  try {
    const raw = localStorage.getItem('huanxing_session');
    if (!raw) return;
    const session = JSON.parse(raw);
    if (session?.accessToken) {
      writeToken(session.accessToken);
      console.log('[useAuth] 从 huanxing_session 恢复 token');
    }
  } catch {
    // ignore
  }
}

// ---------------------------------------------------------------------------
// Context shape
// ---------------------------------------------------------------------------

export interface AuthState {
  /** The current bearer token, or null if not authenticated. */
  token: string | null;
  /** Whether the user is currently authenticated. */
  isAuthenticated: boolean;
  /** Whether the server requires pairing. Defaults to true (safe fallback). */
  requiresPairing: boolean;
  /** True while the initial auth check is in progress. */
  loading: boolean;
  /** Pair with the agent using a pairing code. Stores the token on success. */
  pair: (code: string) => Promise<void>;
  /** Clear the stored token and sign out. */
  logout: () => void;
  /** Login with an existing token (e.g., huanxing access_token). Updates both storage and React state. */
  loginWithToken: (token: string) => void;
}

const AuthContext = createContext<AuthState | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export interface AuthProviderProps {
  children: ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  // 唤星桌面端：在 React 状态初始化前从 localStorage 恢复 token
  // 必须在 useState(readToken) 之前执行，确保首次读取就有值
  if (typeof window !== 'undefined' && !!(window as any).__HUANXING_DESKTOP__) {
    restoreHuanxingToken();
  }

  const [token, setTokenState] = useState<string | null>(readToken);
  const [authenticated, setAuthenticated] = useState<boolean>(checkAuth);
  const [requiresPairing, setRequiresPairing] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(!checkAuth());

  // On mount: check if server requires pairing at all
  useEffect(() => {
    if (checkAuth()) return; // already have a token, no need to check

    // 唤星桌面端：不走 sidecar pairing 流程，让 App.tsx 的 HuanxingLogin 接管
    const isHuanxing = typeof window !== 'undefined' && !!(window as any).__HUANXING_DESKTOP__;
    if (isHuanxing) {
      setLoading(false);
      return;
    }

    let cancelled = false;
    getPublicHealth()
      .then((health) => {
        if (cancelled) return;
        if (!health.require_pairing) {
          setRequiresPairing(false);
          setAuthenticated(true);
        }
      })
      .catch(() => {
        // health endpoint unreachable — fall back to showing pairing dialog
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Keep state in sync if token storage is changed from another browser context.
  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === TOKEN_STORAGE_KEY) {
        const t = readToken();
        setTokenState(t);
        setAuthenticated(t !== null && t.length > 0);
      }
    };
    window.addEventListener('storage', handler);
    return () => window.removeEventListener('storage', handler);
  }, []);

  const pair = useCallback(async (code: string): Promise<void> => {
    const { token: newToken } = await apiPair(code);
    writeToken(newToken);
    setTokenState(newToken);
    setAuthenticated(true);
  }, []);

  const logout = useCallback((): void => {
    removeToken();
    setTokenState(null);
    setAuthenticated(false);
  }, []);

  const loginWithToken = useCallback((newToken: string): void => {
    writeToken(newToken);
    setTokenState(newToken);
    setAuthenticated(true);
  }, []);

  const value: AuthState = {
    token,
    isAuthenticated: authenticated,
    requiresPairing,
    loading,
    pair,
    logout,
    loginWithToken,
  };

  return React.createElement(AuthContext.Provider, { value }, children);
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Access the authentication state from any component inside `<AuthProvider>`.
 * Throws if used outside the provider.
 */
export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth must be used within an <AuthProvider>');
  }
  return ctx;
}
