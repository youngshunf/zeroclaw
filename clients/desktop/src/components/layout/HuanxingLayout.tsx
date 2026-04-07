import React from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useState, useEffect, Suspense } from 'react';
import NavRail, { type TabKey } from './NavRail';
import BottomTabBar, { type MobileTabKey } from './BottomTabBar';
import { useHasnConversations, useHasnContacts } from '@/hooks/useHasn';
import { usePlatform } from '@/hooks/usePlatform';

// Settings-related paths
const settingsPaths = ['/dashboard', '/config', '/cost', '/logs', '/doctor', '/devices', '/integrations', '/tools', '/cron', '/memory', '/about'];

// Routes that don't belong to any tab (profile, etc.) — return null to keep current tab
const independentPaths = ['/profile'];

// Map routes to tabs
function routeToTab(pathname: string): TabKey | null {
  if (pathname.startsWith('/hasn-chat')) return 'hasn';
  if (pathname.startsWith('/contacts')) return 'contacts';
  if (pathname.startsWith('/agents')) return 'agents';
  if (pathname.startsWith('/market')) return 'market';
  if (pathname.startsWith('/docs')) return 'docs';
  if (pathname.startsWith('/sop')) return 'sop';
  if (pathname.startsWith('/tasks')) return 'tasks';
  // Settings pages
  if (settingsPaths.some((p) => pathname.startsWith(p))) return 'settings';
  // Independent pages — don't change active tab
  if (independentPaths.some((p) => pathname.startsWith(p))) return null;
  // Default: agent
  return 'agent';
}

// Map tabs to default routes
const tabRoutes: Record<TabKey, string> = {
  agent: '/agent',
  hasn: '/hasn-chat',
  contacts: '/contacts',
  agents: '/agents',
  market: '/market',
  docs: '/docs',
  sop: '/sop',
  tasks: '/tasks',
  settings: '/dashboard',
};

// Map mobile tabs to desktop tabs
const mobileToDesktop: Record<MobileTabKey, TabKey> = {
  agent: 'agent',
  hasn: 'hasn',
  contacts: 'contacts',
  market: 'market',
  settings: 'settings',
};

const desktopToMobile: Partial<Record<TabKey, MobileTabKey>> = {
  agent: 'agent',
  hasn: 'hasn',
  contacts: 'contacts',
  market: 'market',
  settings: 'settings',
};

export default function HuanxingLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const { isMobile } = usePlatform();
  const [activeTab, setActiveTab] = useState<TabKey>(() => routeToTab(location.pathname) ?? 'agent');
  const { totalUnread } = useHasnConversations();
  const { friendRequests } = useHasnContacts();

  // Sync tab with route changes (skip for independent pages like /profile)
  useEffect(() => {
    const tab = routeToTab(location.pathname);
    if (tab !== null) {
      setActiveTab(tab);
    }
  }, [location.pathname]);

  const handleTabChange = (tab: TabKey) => {
    setActiveTab(tab);
    navigate(tabRoutes[tab]);
  };

  const handleMobileTabChange = (tab: MobileTabKey) => {
    const desktopTab = mobileToDesktop[tab];
    setActiveTab(desktopTab);
    navigate(tabRoutes[desktopTab]);
  };

  const activeMobileTab: MobileTabKey = desktopToMobile[activeTab] ?? 'agent';
  const pendingRequests = friendRequests.filter((r) => r.status === 'pending').length;

  return (
    <div className={`hx-app${isMobile ? ' hx-app-mobile' : ''}`}>
      {/* 全局拖拽条 — 仅桌面端 */}
      {!isMobile && (
        <div
          className="fixed left-0 right-0 top-0 h-8 z-[1] cursor-move select-none"
          style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
          data-tauri-drag-region
        />
      )}

      {/* 桌面端：左侧导航栏 */}
      {!isMobile && (
        <NavRail
          activeTab={activeTab}
          onTabChange={handleTabChange}
          badges={{ hasn: totalUnread, contacts: pendingRequests }}
        />
      )}

      <div className="hx-content">
        <Suspense
          fallback={
            <div className="hx-loading">
              <div className="hx-loader" />
            </div>
          }
        >
          <Outlet />
        </Suspense>
      </div>

      {/* 移动端：底部 Tab 栏 */}
      {isMobile && (
        <BottomTabBar
          activeTab={activeMobileTab}
          onTabChange={handleMobileTabChange}
          badges={{ hasn: totalUnread, contacts: pendingRequests }}
        />
      )}
    </div>
  );
}
