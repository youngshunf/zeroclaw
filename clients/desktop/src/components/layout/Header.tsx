import { useState, useEffect, useRef } from 'react';
import { useLocation } from 'react-router-dom';
import { LogOut, Menu, PanelLeftClose, PanelLeftOpen, ChevronDown, Bot } from 'lucide-react';
import { t, LANGUAGE_BUTTON_LABELS, LANGUAGE_SWITCH_ORDER } from '@/lib/i18n';
import { useLocaleContext } from '@/App';
import { useAuth } from '@/hooks/useAuth';
import { useActiveAgent } from '@/hooks/useActiveAgent';

function isHuanxingDesktop(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__HUANXING_DESKTOP__;
}

const routeTitles: Record<string, string> = {
  '/': 'nav.dashboard',
  '/agent': 'nav.agent',
  '/tools': 'nav.tools',
  '/cron': 'nav.cron',
  '/integrations': 'nav.integrations',
  '/memory': 'nav.memory',
  '/devices': 'nav.devices',
  '/config': 'nav.config',
  '/cost': 'nav.cost',
  '/logs': 'nav.logs',
  '/doctor': 'nav.doctor',
};

const languageSummary = 'English · 简体中文 · 日本語 · Русский · Français · Tiếng Việt · Ελληνικά';

interface AgentOption {
  name: string;
  active: boolean;
}

interface HeaderProps {
  isSidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  onToggleSidebarCollapse: () => void;
}

export default function Header({
  isSidebarCollapsed,
  onToggleSidebar,
  onToggleSidebarCollapse,
}: HeaderProps) {
  const location = useLocation();
  const { logout } = useAuth();
  const { locale, setAppLocale } = useLocaleContext();
  const [activeAgent] = useActiveAgent();

  // Agent dropdown state
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [agents, setAgents] = useState<AgentOption[]>([]);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const isAgentPage = location.pathname === '/agent';
  const isHuanxing = isHuanxingDesktop();

  // Page title: on /agent page with an active agent, show agent name
  const titleKey = routeTitles[location.pathname] ?? 'nav.dashboard';
  const pageTitle = (isHuanxing && isAgentPage && activeAgent)
    ? activeAgent
    : t(titleKey);

  const toggleLanguage = () => {
    const currentIndex = LANGUAGE_SWITCH_ORDER.indexOf(locale);
    const nextLocale =
      LANGUAGE_SWITCH_ORDER[(currentIndex + 1) % LANGUAGE_SWITCH_ORDER.length] ?? 'en';
    setAppLocale(nextLocale);
  };

  // Load agent list when dropdown opens
  const handleToggleDropdown = async () => {
    if (!dropdownOpen) {
      try {
        const { listAgents } = await import('@/huanxing/lib/agent-api');
        const data = await listAgents();
        setAgents(data.agents.map(a => ({ name: a.name, active: a.active })));
      } catch {
        setAgents([]);
      }
    }
    setDropdownOpen(!dropdownOpen);
  };

  // Switch agent
  const handleSwitchAgent = async (name: string) => {
    setDropdownOpen(false);
    try {
      const { switchAgent } = await import('@/huanxing/lib/agent-api');
      await switchAgent(name);
      // SSE event will handle the rest (ChatLayout listens)
    } catch (err) {
      console.error('Failed to switch agent:', err);
    }
  };

  // Close dropdown on outside click
  useEffect(() => {
    if (!dropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [dropdownOpen]);

  return (
    <header className="glass-header relative flex min-h-[4.5rem] flex-wrap items-center justify-between gap-2 rounded-2xl border border-[#1a3670] px-4 py-3 sm:px-5 sm:py-3.5 md:flex-nowrap md:px-8 md:py-4">
      <div className="absolute inset-0 pointer-events-none opacity-70 bg-[radial-gradient(circle_at_15%_30%,rgba(41,148,255,0.22),transparent_45%),radial-gradient(circle_at_85%_75%,rgba(0,209,255,0.14),transparent_40%)]" />

      <div className="relative flex min-w-0 items-center gap-2.5 sm:gap-3">
        <button
          type="button"
          onClick={onToggleSidebar}
          aria-label="Open navigation"
          className="rounded-lg border border-[#294a8f] bg-[#081637]/70 p-1.5 text-[#9ec2ff] transition hover:border-[#4f83ff] hover:text-white md:hidden"
        >
          <Menu className="h-5 w-5" />
        </button>

        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="truncate text-base font-semibold tracking-wide text-white sm:text-lg">
              {pageTitle}
            </h1>

            {/* Agent switch dropdown (唤星 + /agent page only) */}
            {isHuanxing && isAgentPage && (
              <div className="relative" ref={dropdownRef}>
                <button
                  onClick={handleToggleDropdown}
                  className="flex items-center gap-0.5 rounded-lg border border-[#2c4e97] bg-[#0a1b3f]/60 px-1.5 py-1 text-[#8bb9ff] transition hover:border-[#7c3aed] hover:text-white"
                  title="切换智能体"
                >
                  <ChevronDown className={`h-4 w-4 transition-transform ${dropdownOpen ? 'rotate-180' : ''}`} />
                </button>

                {dropdownOpen && (
                  <div className="absolute left-0 top-full mt-1 z-50 min-w-[200px] rounded-xl border border-[#1e2f5d] bg-[#0a1b3f] shadow-[0_8px_32px_rgba(0,0,0,0.5)] overflow-hidden">
                    <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-[#5f84cc] border-b border-[#1e2f5d]">
                      切换智能体
                    </div>
                    {agents.length === 0 ? (
                      <div className="px-3 py-3 text-xs text-[#5f84cc]">加载中...</div>
                    ) : (
                      agents.map((agent) => (
                        <button
                          key={agent.name}
                          onClick={() => handleSwitchAgent(agent.name)}
                          className={[
                            'w-full flex items-center gap-2 px-3 py-2.5 text-sm text-left transition-colors',
                            agent.active
                              ? 'bg-[#7c3aed]/15 text-white'
                              : 'text-[#9bb7eb] hover:bg-[#07132f] hover:text-white',
                          ].join(' ')}
                        >
                          <Bot className="h-4 w-4 shrink-0 opacity-70" />
                          <span className="truncate">{agent.name}</span>
                          {agent.active && (
                            <span className="ml-auto shrink-0 h-2 w-2 rounded-full bg-green-400" />
                          )}
                        </button>
                      ))
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
          <p className="hidden text-[10px] uppercase tracking-[0.16em] text-[#7ea5eb] sm:block">
            {isHuanxing && isAgentPage ? '唤星 AI Agent' : 'Electric dashboard'}
          </p>
        </div>
      </div>

      <div className="relative flex w-full items-center justify-end gap-1.5 sm:gap-2 md:w-auto md:gap-3">
        <button
          type="button"
          onClick={onToggleSidebarCollapse}
          className="hidden items-center gap-1 rounded-lg border border-[#2b4f97] bg-[#091937]/75 px-2.5 py-1.5 text-xs text-[#c4d8ff] transition hover:border-[#4f83ff] hover:text-white md:flex md:text-sm"
          title={isSidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {isSidebarCollapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          <span>{isSidebarCollapsed ? 'Expand' : 'Collapse'}</span>
        </button>

        <button
          type="button"
          onClick={toggleLanguage}
          title={`🌐 Languages: ${languageSummary}`}
          className="rounded-lg border border-[#2b4f97] bg-[#091937]/75 px-2.5 py-1 text-xs font-medium text-[#c4d8ff] transition hover:border-[#4f83ff] hover:text-white sm:px-3 sm:text-sm"
        >
          {LANGUAGE_BUTTON_LABELS[locale] ?? 'EN'}
        </button>

        <button
          type="button"
          onClick={logout}
          className="flex items-center gap-1 rounded-lg border border-[#2b4f97] bg-[#091937]/75 px-2.5 py-1.5 text-xs text-[#c4d8ff] transition hover:border-[#4f83ff] hover:text-white sm:gap-1.5 sm:px-3 sm:text-sm"
        >
          <LogOut className="h-4 w-4" />
          <span className="hidden sm:inline">{t('auth.logout')}</span>
        </button>
      </div>
    </header>
  );
}
