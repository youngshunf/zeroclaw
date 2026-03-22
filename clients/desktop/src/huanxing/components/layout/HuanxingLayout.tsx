import React from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useState, useEffect, Suspense } from 'react';
import NavRail, { type TabKey } from './NavRail';
import { useHasnConversations, useHasnContacts } from '@/huanxing/hooks/useHasn';

// Settings-related paths
const settingsPaths = ['/dashboard', '/config', '/cost', '/logs', '/doctor', '/devices', '/integrations', '/tools', '/cron', '/memory', '/about'];

// Routes that don't belong to any tab (profile, etc.) — return null to keep current tab
const independentPaths = ['/profile'];

// Map routes to tabs
function routeToTab(pathname: string): TabKey | null {
  if (pathname.startsWith('/hasn-chat')) return 'hasn';
  if (pathname.startsWith('/contacts')) return 'contacts';
  if (pathname.startsWith('/agents')) return 'agents';
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
  settings: '/dashboard',
};

export default function HuanxingLayout() {
  const location = useLocation();
  const navigate = useNavigate();
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

  return (
    <div className="hx-app">
      {/* 全局拖拽条 — 覆盖顶部 32px，z-index 最高 */}
      <div
        className="fixed left-0 right-0 top-0 h-8 z-[9999] cursor-move select-none"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      />
      <NavRail
        activeTab={activeTab}
        onTabChange={handleTabChange}
        badges={{ hasn: totalUnread, contacts: friendRequests.filter((r) => r.status === 'pending').length }}
      />
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
    </div>
  );
}
