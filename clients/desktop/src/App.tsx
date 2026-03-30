import { Routes, Route, Navigate } from 'react-router-dom';
import { useState, useEffect, createContext, useContext, lazy, Suspense } from 'react';
import Dashboard from './pages/settings/Dashboard';
const ChatLayout = lazy(() => import('./pages/agent/ChatLayout'));
import Tools from './pages/settings/Tools';
import Cron from './pages/settings/Cron';
import Integrations from './pages/settings/Integrations';
import Memory from './pages/settings/Memory';
import Devices from './pages/settings/Devices';
import Config from './pages/settings/Config';
import Cost from './pages/settings/Cost';
import Logs from './pages/settings/Logs';
import Doctor from './pages/settings/Doctor';
const ImageViewer = lazy(() => import('./pages/ImageViewer'));
import { AuthProvider, useAuth } from './hooks/useAuth';
import { coerceLocale, setLocale, type Locale } from './lib/i18n';
import { startTokenRefresh, stopTokenRefresh } from './lib/token-refresh';

// --- 唤星页面 ---
const HuanxingLogin = lazy(() => import('./pages/auth/Login'));
const HasnChat = lazy(() => import('./pages/hasn/HasnChat'));
const Contacts = lazy(() => import('./pages/contacts/Contacts'));
const AgentManager = lazy(() => import('./pages/agents/AgentManager'));
const Marketplace = lazy(() => import('./pages/market/Marketplace'));
const Documents = lazy(() => import('./pages/docs/Documents'));
const HuanxingLayout = lazy(() => import('./components/layout/HuanxingLayout'));
const SettingsPanel = lazy(() => import('./components/layout/SettingsPanel'));
const Engine = lazy(() => import('./pages/engine/Engine'));
const ProfilePage = lazy(() => import('./pages/profile/ProfilePage'));
const SopWorkbench = lazy(() => import('./pages/sop/SopWorkbench'));

const LOCALE_STORAGE_KEY = 'zeroclaw:locale';

// Locale context
interface LocaleContextType {
  locale: Locale;
  setAppLocale: (locale: Locale) => void;
}

export const LocaleContext = createContext<LocaleContextType>({
  locale: 'en',
  setAppLocale: (_locale: Locale) => {},
});

export const useLocaleContext = () => useContext(LocaleContext);

function AppContent() {
  const { isAuthenticated, loading, logout, loginWithToken } = useAuth();
  const [locale, setLocaleState] = useState<Locale>(() => {
    if (typeof window === 'undefined') return 'en';
    const saved = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    if (saved) return coerceLocale(saved);
    return coerceLocale(window.navigator.language);
  });

  useEffect(() => {
    setLocale(locale);
    if (typeof window !== 'undefined') {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    }
  }, [locale]);

  const setAppLocale = (newLocale: Locale) => {
    setLocale(newLocale); // 同步更新模块级 currentLocale，确保 t() 在同一次渲染中生效
    setLocaleState(newLocale);
  };

  // Listen for 401 events to force logout
  useEffect(() => {
    const handler = () => {
      stopTokenRefresh();
      logout();
    };
    window.addEventListener('zeroclaw-unauthorized', handler);
    return () => window.removeEventListener('zeroclaw-unauthorized', handler);
  }, [logout]);

  // 已登录用户：启动 token 自动刷新
  useEffect(() => {
    if (isAuthenticated) {
      startTokenRefresh();
      return () => stopTokenRefresh();
    }
  }, [isAuthenticated]);

  // HASN 自动连接（由 Tauri 层管理，前端负责提供 token）
  useEffect(() => {
    if (!isAuthenticated) return;

    const internals = (window as any).__TAURI_INTERNALS__;
    if (!internals?.invoke) return;

    let cancelled = false;

    // 提供 token 给 Tauri 的共用逻辑
    const provideToken = async () => {
      if (cancelled) return;
      try {
        const { getHuanxingSession } = await import('./config');
        const session = getHuanxingSession();
        if (!session?.accessToken) return;

        // 确保 HASN 身份已注册（幂等）
        const { registerHasnIdentity, registerHasnAgent } = await import('./onboard');
        const identity = await registerHasnIdentity(session);
        if (!identity?.hasn_id || cancelled) return;

        // 调用 hasn_connect（内部会保存 client.json）
        console.log('[App] HASN 提供 token，hasn_id:', identity.hasn_id);
        await internals.invoke('hasn_connect', {
          platformToken: session.accessToken,
          hasnId: identity.hasn_id,
          starId: identity.star_id,
        });
        console.log('[App] HASN 已连接');

        // 确保本地 Agent 的 HASN 身份已注册且 hasn_id 已写入 config.toml（幂等）
        try {
          const nickname = session.user?.nickname || '唤星用户';
          await registerHasnAgent(session, 'default', `${nickname}的星灵`, 'local');
        } catch (agentErr) {
          console.warn('[App] Agent HASN 注册（非致命）:', agentErr);
        }
      } catch (err) {
        console.warn('[App] HASN 连接失败（非致命）:', err);
      }
    };

    // 1. 主动检查：如果未连接，立即提供 token
    internals.invoke('hasn_status').then((s: string) => {
      if (s !== 'connected' && !cancelled) {
        provideToken();
      }
    }).catch(() => {
      provideToken();
    });

    // 2. 被动监听：Tauri 断线重连时也可能再次请求 token
    let unlisten: (() => void) | null = null;
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('hasn:request-token', provideToken)
        .then(fn => { unlisten = fn; });
    }).catch(() => {});

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [isAuthenticated]);

  // 唤星桌面端：主动检查配置有效性
  // config.toml 存在且有效 → 放行（sidecar 由 setup hook 管理）
  // config.toml 不存在或无效 → 强制退出登录
  const [configChecked, setConfigChecked] = useState(false);

  useEffect(() => {
    // 非 Tauri 环境或未登录：跳过检查
    const internals = (window as any).__TAURI_INTERNALS__;
    if (!internals?.invoke || !isAuthenticated) {
      setConfigChecked(true);
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        // 返回 { config_exists: boolean, config_valid: boolean }
        const result = await internals.invoke('check_huanxing_config');
        if (cancelled) return;

        if (result?.config_valid) {
          console.log('[huanxing] 配置有效');
          setConfigChecked(true);
          return;
        }

        // 配置无效 → 强制退出登录
        console.log('[huanxing] 配置无效，强制退出登录',
          `(exists=${result?.config_exists}, valid=${result?.config_valid})`);
        localStorage.removeItem('huanxing_session');
        logout();
      } catch (err) {
        console.warn('[huanxing] 配置检查失败:', err);
        // 检查失败不阻塞，让用户继续使用
      }
      if (!cancelled) setConfigChecked(true);
    })();

    return () => { cancelled = true; };
  }, [isAuthenticated, logout]);

  // 监听 Tauri setup hook 发来的配置无效事件（二重保险）
  useEffect(() => {
    const internals = (window as any).__TAURI_INTERNALS__;
    if (!internals) return;

    let unlisten: (() => void) | null = null;

    // Tauri v2 listen API
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('huanxing:config-invalid', () => {
        console.log('[huanxing] Received config-invalid event, forcing logout');
        localStorage.removeItem('huanxing_session');
        logout();
      }).then(fn => { unlisten = fn; });
    }).catch(() => {});

    return () => { unlisten?.(); };
  }, [logout]);

  if (loading || (isAuthenticated && !configChecked)) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#050b1a]">
        <div className="flex flex-col items-center gap-3">
          <div className="electric-loader h-10 w-10 rounded-full" />
          <p className="text-[#a7c4f3]">加载中...</p>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) {
    return (
      <Suspense fallback={<div className="min-h-screen flex items-center justify-center bg-[#050b1a]"><div className="electric-loader h-10 w-10 rounded-full" /></div>}>
        <HuanxingLogin onLoginSuccess={(token) => {
          loginWithToken(token);
        }} />
      </Suspense>
    );
  }

  return (
    <LocaleContext.Provider value={{ locale, setAppLocale }}>
      <Routes>
        {/* 单独窗口: 图片预览 */}
        <Route path="/image-viewer" element={<Suspense fallback={null}><ImageViewer /></Suspense>} />

        {/* ===== 唤星三栏布局 ===== */}
        <Route element={<Suspense fallback={null}><HuanxingLayout /></Suspense>}>
          {/* Tab 1: AI Agent */}
          <Route path="/agent" element={<Suspense fallback={null}><ChatLayout /></Suspense>} />
          <Route path="/" element={<Navigate to="/agent" replace />} />

          {/* Tab 2: HASN 社交 */}
          <Route path="/hasn-chat" element={<Suspense fallback={null}><HasnChat /></Suspense>} />

          {/* Tab 3: 联系人 */}
          <Route path="/contacts" element={<Suspense fallback={null}><Contacts /></Suspense>} />

          {/* Tab 4: Agent 管理 */}
          <Route path="/agents" element={<Suspense fallback={null}><AgentManager /></Suspense>} />

          {/* 新增: 市场 */}
          <Route path="/market" element={<Suspense fallback={null}><Marketplace /></Suspense>} />

          {/* 新增: 文档 */}
          <Route path="/docs" element={<Suspense fallback={null}><Documents /></Suspense>} />

          {/* 新增: SOP 工作台 */}
          <Route path="/sop" element={<Suspense fallback={null}><SopWorkbench /></Suspense>} />

          {/* 个人资料 */}
          <Route path="/profile" element={<Suspense fallback={null}><ProfilePage /></Suspense>} />

          {/* Tab 5: 设置中心 — 嵌套路由 */}
          <Route element={<Suspense fallback={null}><SettingsPanel /></Suspense>}>
            <Route path="/dashboard" element={<Dashboard />} />
            <Route path="/integrations" element={<Integrations />} />
            <Route path="/memory" element={<Memory />} />
            <Route path="/tools" element={<Tools />} />
            <Route path="/cron" element={<Cron />} />
            <Route path="/config" element={<Config />} />
            <Route path="/cost" element={<Cost />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/doctor" element={<Doctor />} />
            <Route path="/devices" element={<Devices />} />
            <Route path="/engine" element={<Suspense fallback={null}><Engine /></Suspense>} />
          </Route>

          <Route path="*" element={<Navigate to="/agent" replace />} />
        </Route>
      </Routes>
    </LocaleContext.Provider>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
}
