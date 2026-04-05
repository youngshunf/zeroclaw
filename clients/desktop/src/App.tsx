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
const ChannelsLayout = lazy(() => import('./pages/channels/ChannelsLayout'));
const ImageViewer = lazy(() => import('./pages/ImageViewer'));
import { AuthProvider, useAuth } from './hooks/useAuth';
import { coerceLocale, setLocale, type Locale } from './lib/i18n';
import { startTokenRefresh, stopTokenRefresh } from './lib/token-refresh';
import { isTauriMobile } from './lib/platform';
import { getHuanxingSession } from './config';

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
      console.error('[huanxing-debug] ⚠️ zeroclaw-unauthorized event → logout');
      console.trace('[huanxing-debug] stack trace');
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

  // HASN 身份注册 + 连接建立
  useEffect(() => {
    if (!isAuthenticated) return;

    let cancelled = false;

    const registerAndConnect = async () => {
      if (cancelled) return;
      try {
        const { getHuanxingSession } = await import('./config');
        const session = getHuanxingSession();
        if (!session?.accessToken) return;

        // 确保 HASN 身份已注册（幂等）
        const { registerHasnIdentity, registerHasnAgent } = await import('./onboard');
        const identity = await registerHasnIdentity(session);
        if (!identity?.hasn_id || cancelled) return;

        // 确保本地 Agent 的 HASN 身份已注册且 hasn_id 已写入 config.toml（幂等）
        try {
          const nickname = session.user?.nickname || '唤星用户';
          await registerHasnAgent(session, 'default', `${nickname}的星灵`, 'local');
        } catch (agentErr) {
          console.warn('[App] Agent HASN 注册（非致命）:', agentErr);
        }

        // 建立 HASN WebSocket 连接
        if (!cancelled && identity.hasn_id) {
          try {
            const { hasnConnect, hasnAddOwner, hasnAddAgent } = await import('./lib/hasn-api');
            // 优先用 session.hasnNodeKey，其次用 identity 注册时返回的 node_key
            const nodeKey = session.hasnNodeKey || identity.node_key;
            if (nodeKey) {
              await hasnConnect(nodeKey, identity.hasn_id, identity.star_id || '');
              await hasnAddOwner(identity.hasn_id, session.accessToken);

              // 本地默认 Agent 已完成 HASN 注册时，自动上线 Presence
              const agentDisplayName = `${session.user.nickname || '唤星用户'}的星灵`;
              const agentIdentity = await registerHasnAgent(session, 'default', agentDisplayName, 'local');
              if (agentIdentity?.hasn_id) {
                await hasnAddAgent(agentIdentity.hasn_id, identity.hasn_id);
              }

              console.log('[App] HASN 连接与 Owner/Agent 绑定已建立, hasn_id:', identity.hasn_id);
            } else {
              console.warn('[App] 缺少 hasn_node_key，无法建立 HASN 连接');
            }
          } catch (wsErr) {
            console.warn('[App] HASN 连接失败（非致命）:', wsErr);
          }
        }
      } catch (err) {
        console.warn('[App] HASN 注册阶段失败:', err);
      }
    };

    registerAndConnect();

    return () => {
      cancelled = true;
    };
  }, [isAuthenticated]);

  // Owner Binding 自动续期（轻量定时续租）
  useEffect(() => {
    if (!isAuthenticated) return;

    const interval = window.setInterval(() => {
      const session = getHuanxingSession();
      const hasnId = localStorage.getItem('hasn:hasn_id');
      if (!session?.accessToken || !hasnId) return;

      import('./lib/hasn-api')
        .then(({ hasnStatus, hasnRenewOwner }) => hasnStatus().then((status) => ({ status, hasnRenewOwner })))
        .then(async ({ status, hasnRenewOwner }) => {
          if (status === 'connected') {
            await hasnRenewOwner(hasnId, session.accessToken);
          }
        })
        .catch((err) => {
          console.warn('[App] HASN owner renew failed:', err);
        });
    }, 5 * 60 * 1000);

    return () => window.clearInterval(interval);
  }, [isAuthenticated]);

  // 唤星桌面端：主动检查配置有效性
  // config.toml 存在且有效 → 放行（sidecar 由 setup hook 管理）
  // config.toml 不存在或无效 → 强制退出登录
  const [configChecked, setConfigChecked] = useState(false);

  useEffect(() => {
    // 非 Tauri 环境、未登录、或移动端：跳过配置检查
    // 移动端没有注册 check_huanxing_config 命令（它依赖 SidecarManager）
    const internals = (window as any).__TAURI_INTERNALS__;
    if (!internals?.invoke || !isAuthenticated || isTauriMobile()) {
      setConfigChecked(true);
      return;
    }

    let cancelled = false;

    const doCheck = async (): Promise<boolean> => {
      try {
        const result = await internals.invoke('check_huanxing_config');
        return result?.config_valid === true;
      } catch {
        return true; // 检查失败不阻塞
      }
    };

    (async () => {
      // 延迟 3 秒，等 onboard 有机会完成（首次登录时 config.toml 需要 onboard 创建）
      await new Promise(r => setTimeout(r, 3000));
      if (cancelled) return;

      const firstCheck = await doCheck();
      if (cancelled) return;

      if (firstCheck) {
        console.log('[huanxing] 配置有效');
        setConfigChecked(true);
        return;
      }

      // 首次检查失败（可能 onboard 仍在进行中），再等 3 秒重试一次
      console.warn('[huanxing] 配置检查首次未通过，等待重试...');
      await new Promise(r => setTimeout(r, 3000));
      if (cancelled) return;

      const retryCheck = await doCheck();
      if (cancelled) return;

      if (retryCheck) {
        console.log('[huanxing] 配置重试通过');
        setConfigChecked(true);
        return;
      }

      // 两次检查都失败：配置确实无效
      console.error('[huanxing-debug] ⚠️ check_huanxing_config → config invalid → logout (after retry)');
      localStorage.removeItem('huanxing_session');
      logout();
      setConfigChecked(true);
    })();

    return () => { cancelled = true; };
  }, [isAuthenticated, logout]);

  // 监听 Tauri setup hook 发来的配置无效事件（二重保险）
  useEffect(() => {
    // 移动端跳过 config-invalid 事件监听（嵌入式引擎自己管理配置）
    const internals = (window as any).__TAURI_INTERNALS__;
    if (!internals || isTauriMobile()) return;

    let unlisten: (() => void) | null = null;

    // Tauri v2 listen API
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen('huanxing:config-invalid', () => {
        console.error('[huanxing-debug] ⚠️ config-invalid event from Tauri → logout');
        localStorage.removeItem('huanxing_session');
        logout();
      }).then(fn => { unlisten = fn; });
    }).catch(() => {});

    return () => { unlisten?.(); };
  }, [logout]);

  // 主题管理：登录页强制 dark，登录后默认切换回 light (或尊重已存偏好)
  useEffect(() => {
    const root = document.documentElement;
    if (!isAuthenticated) {
      // 登录页：必须是暗色
      root.setAttribute('data-theme', 'dark');
      root.classList.add('dark');
    } else {
      // 登录成功后：如果没存过偏好，强制设为 light (符合用户“登录成功切换为 light”的需求)
      const savedTheme = localStorage.getItem('huanxing_theme');
      if (!savedTheme) {
        root.setAttribute('data-theme', 'light');
        root.classList.remove('dark');
        localStorage.setItem('huanxing_theme', 'light');
      } else {
        // 已有偏好，按照偏好应用
        root.setAttribute('data-theme', savedTheme);
        if (savedTheme === 'dark') root.classList.add('dark');
        else root.classList.remove('dark');
      }
    }
  }, [isAuthenticated]);

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
            <Route path="/channels" element={<Suspense fallback={null}><ChannelsLayout /></Suspense>} />
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
