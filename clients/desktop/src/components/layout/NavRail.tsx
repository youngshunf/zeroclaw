import { useState, useRef, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Bot, MessageSquare, Users, Grid2X2, Settings, User, LogOut, Sun, Moon, Globe, Check, Store, Workflow, FileText, Clock } from 'lucide-react';
import logoDark from '@/assets/logo-v2-dark.svg';
import logoLight from '@/assets/logo-v2-light.svg';
import { getHuanxingSession, clearHuanxingSession } from '@/config';
import { useAuth } from '@/hooks/useAuth';
import { LANGUAGE_BUTTON_LABELS, LANGUAGE_SWITCH_ORDER, type Locale } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

export type TabKey = 'agent' | 'hasn' | 'contacts' | 'agents' | 'market' | 'docs' | 'sop' | 'tasks' | 'settings';

interface NavRailProps {
  activeTab: TabKey;
  onTabChange: (tab: TabKey) => void;
  badges?: Partial<Record<TabKey, number>>;
}

const tabs: { key: TabKey; icon: typeof Bot; label: string }[] = [
  { key: 'agent', icon: Bot, label: 'AI 助手' },
  { key: 'hasn', icon: MessageSquare, label: '消息' },
  { key: 'contacts', icon: Users, label: '通讯录' },
  { key: 'agents', icon: Grid2X2, label: 'Agent 管理' },
  { key: 'market', icon: Store, label: '应用生态市场' },
  { key: 'docs', icon: FileText, label: '知识文档' },
  { key: 'sop', icon: Workflow, label: 'SOP 工作台' },
  { key: 'tasks', icon: Clock, label: '定时调度' },
];

/** Full display names for the language picker */
const LANGUAGE_NAMES: Record<Locale, string> = {
  en: 'English',
  tr: 'Türkçe',
  'zh-CN': '简体中文',
  ja: '日本語',
  ru: 'Русский',
  fr: 'Français',
  vi: 'Tiếng Việt',
  el: 'Ελληνικά',
};

export default function NavRail({ activeTab, onTabChange, badges = {} }: NavRailProps) {
  const { logout } = useAuth();
  const navigate = useNavigate();
  const { locale, setAppLocale } = useLocaleContext();
  const [showMenu, setShowMenu] = useState(false);
  const [showLangMenu, setShowLangMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const langRef = useRef<HTMLDivElement>(null);

  // Theme state
  const [isDark, setIsDark] = useState(() => {
    if (typeof window === 'undefined') return false;
    const saved = localStorage.getItem('huanxing_theme');
    return saved === 'dark';
  });

  // Apply theme attribute
  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute('data-theme', isDark ? 'dark' : 'light');
    if (isDark) root.classList.add('dark');
    else root.classList.remove('dark');
    localStorage.setItem('huanxing_theme', isDark ? 'dark' : 'light');
  }, [isDark]);

  const toggleTheme = useCallback(() => {
    setIsDark(prev => !prev);
  }, []);

  // 从 session 获取用户信息
  const session = getHuanxingSession();
  const userName = session?.user?.nickname || session?.user?.username || '用户';
  const userPhone = session?.user?.phone || '';
  const userAvatar = session?.user?.avatar || '';
  const avatarChar = userName.charAt(0);

  // 点击外部关闭菜单
  useEffect(() => {
    if (!showMenu && !showLangMenu) return;
    const handler = (e: MouseEvent) => {
      if (showMenu && menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
      if (showLangMenu && langRef.current && !langRef.current.contains(e.target as Node)) {
        setShowLangMenu(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [showMenu, showLangMenu]);

  const handleLogout = () => {
    setShowMenu(false);
    clearHuanxingSession();
    try { sessionStorage.removeItem('zeroclaw_token'); } catch {}
    logout();
  };

  const handleProfile = () => {
    setShowMenu(false);
    navigate('/profile');
  };

  const handleSelectLanguage = (lang: Locale) => {
    setAppLocale(lang);
    setShowLangMenu(false);
  };

  return (
    <nav className="hx-nav-rail">
      {/* Logo */}
      <div className="hx-nav-logo" onClick={() => onTabChange('agent')}>
        <img 
          src={isDark ? logoDark : logoLight} 
          alt="唤星" 
          width="36" 
          height="36" 
          className="object-cover rounded-[7.5px]" 
        />
      </div>

      <div className="hx-nav-divider" />

      {/* Main tabs */}
      {tabs.map(({ key, icon: Icon, label }) => (
        <button
          key={key}
          className={`hx-nav-item${activeTab === key ? ' active' : ''}`}
          onClick={() => onTabChange(key)}
          title={label}
        >
          <Icon size={22} />
          {(badges[key] ?? 0) > 0 && (
            <span className="hx-nav-badge">{badges[key]}</span>
          )}
        </button>
      ))}

      <div className="hx-nav-spacer" />

      {/* Theme toggle */}
      <button
        className="hx-nav-item"
        onClick={toggleTheme}
        title={isDark ? '切换亮色模式' : '切换暗色模式'}
      >
        {isDark ? <Sun size={20} /> : <Moon size={20} />}
      </button>

      {/* Language picker */}
      <div className="hx-nav-avatar-wrap" ref={langRef}>
        <button
          className="hx-nav-item hx-nav-lang-btn"
          onClick={() => setShowLangMenu(!showLangMenu)}
          title="切换语言"
        >
          <Globe size={22} />
          <span className="hx-nav-lang-label">{LANGUAGE_BUTTON_LABELS[locale] ?? 'EN'}</span>
        </button>

        {showLangMenu && (
          <div className="hx-lang-menu">
            <div className="hx-lang-menu-title">语言 / Language</div>
            {LANGUAGE_SWITCH_ORDER.map((lang) => (
              <button
                key={lang}
                className={`hx-lang-menu-item${locale === lang ? ' active' : ''}`}
                onClick={() => handleSelectLanguage(lang)}
              >
                <span>{LANGUAGE_NAMES[lang] ?? lang}</span>
                {locale === lang && <Check size={14} />}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Settings */}
      <button
        className={`hx-nav-item${activeTab === 'settings' ? ' active' : ''}`}
        onClick={() => onTabChange('settings')}
        title="设置"
      >
        <Settings size={22} />
      </button>

      {/* User avatar with popup menu */}
      <div className="hx-nav-avatar-wrap" ref={menuRef}>
        {userAvatar ? (
          <img
            className="hx-nav-avatar hx-nav-avatar-img"
            src={userAvatar}
            alt={userName}
            onClick={() => setShowMenu(!showMenu)}
          />
        ) : (
          <div
            className="hx-nav-avatar"
            onClick={() => setShowMenu(!showMenu)}
            title={userName}
          >
            {avatarChar}
          </div>
        )}

        {showMenu && (
          <div className="hx-avatar-menu">
            <div className="hx-avatar-menu-header">
              {userAvatar ? (
                <img className="hx-avatar-menu-avatar hx-avatar-menu-avatar-img" src={userAvatar} alt={userName} />
              ) : (
                <div className="hx-avatar-menu-avatar">{avatarChar}</div>
              )}
              <div className="hx-avatar-menu-info">
                <div className="hx-avatar-menu-name">{userName}</div>
                {userPhone && (
                  <div className="hx-avatar-menu-phone">{userPhone}</div>
                )}
              </div>
            </div>
            <div className="hx-avatar-menu-divider" />
            <button className="hx-avatar-menu-item" onClick={handleProfile}>
              <User size={16} />
              <span>个人资料</span>
            </button>
            <button className="hx-avatar-menu-item hx-avatar-menu-logout" onClick={handleLogout}>
              <LogOut size={16} />
              <span>退出登录</span>
            </button>
          </div>
        )}
      </div>
    </nav>
  );
}
