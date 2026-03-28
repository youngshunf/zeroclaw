import { Routes, Route, Navigate } from 'react-router-dom';
import { useState, useEffect, createContext, useContext, lazy, Suspense } from 'react';
import Dashboard from './pages/Dashboard';
const ChatLayout = lazy(() => import('./pages/ChatLayout'));
import Tools from './pages/Tools';
import Cron from './pages/Cron';
import Integrations from './pages/Integrations';
import Memory from './pages/Memory';
import Devices from './pages/Devices';
import Config from './pages/Config';
import Cost from './pages/Cost';
import Logs from './pages/Logs';
import Doctor from './pages/Doctor';
import { AuthProvider, useAuth } from './hooks/useAuth';
import { coerceLocale, setLocale, type Locale } from './lib/i18n';
import { startTokenRefresh, stopTokenRefresh } from './huanxing/lib/token-refresh';

// --- 唤星页面 ---
const HuanxingLogin = lazy(() => import('./huanxing/pages/Login'));
const HasnChat = lazy(() => import('./huanxing/pages/HasnChat'));
const Contacts = lazy(() => import('./huanxing/pages/Contacts'));
const AgentManager = lazy(() => import('./huanxing/pages/AgentManager'));
const HuanxingLayout = lazy(() => import('./huanxing/components/layout/HuanxingLayout'));
const SettingsPanel = lazy(() => import('./huanxing/components/layout/SettingsPanel'));
const Engine = lazy(() => import('./huanxing/pages/Engine'));
const ProfilePage = lazy(() => import('./huanxing/components/profile/ProfilePage'));

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

  // 唤星桌面端：主动检查配置有效性
  // config 有效 → 自动修复（启动 sidecar）
  // config 不存在 / 无效 → 强制退出登录
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
        const status: { config_exists: boolean; config_valid: boolean } =
          await internals.invoke('check_huanxing_config');
        if (cancelled) return;

        if (status.config_valid) {
          // 配置有效 → 自动修复（确保 sidecar 启动）
          console.log('[huanxing] 配置有效，检查 sidecar 状态...');
          try {
            const sidecarStatus = await internals.invoke('get_zeroclaw_status');
            if (!sidecarStatus?.running) {
              console.log('[huanxing] sidecar 未运行，自动启动...');
              await internals.invoke('start_zeroclaw');
            }
          } catch (e) {
            console.warn('[huanxing] sidecar 自动修复失败:', e);
          }
          setConfigChecked(true);
          return;
        }

        // 配置不存在或无效 → 强制退出登录
        console.log('[huanxing] 配置无效或不存在，强制退出登录',
          `(exists=${status.config_exists}, valid=${status.config_valid})`);
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
