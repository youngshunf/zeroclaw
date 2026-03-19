import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Bot, MessageSquare, Users, Grid2X2, Settings, User, LogOut } from 'lucide-react';
import logoLight from '../../assets/logo-icon-light.svg';
import { getHuanxingSession, clearHuanxingSession } from '../../config';
import { useAuth } from '../../../hooks/useAuth';

export type TabKey = 'agent' | 'hasn' | 'contacts' | 'agents' | 'settings';

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
];

export default function NavRail({ activeTab, onTabChange, badges = {} }: NavRailProps) {
  const { logout } = useAuth();
  const navigate = useNavigate();
  const [showMenu, setShowMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // 从 session 获取用户信息
  const session = getHuanxingSession();
  const userName = session?.user?.nickname || session?.user?.username || '用户';
  const userPhone = session?.user?.phone || '';
  const userAvatar = session?.user?.avatar || '';
  const avatarChar = userName.charAt(0);

  // 点击外部关闭菜单
  useEffect(() => {
    if (!showMenu) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [showMenu]);

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

  return (
    <nav className="hx-nav-rail">
      {/* Logo */}
      <div className="hx-nav-logo" onClick={() => onTabChange('agent')}>
        <img src={logoLight} alt="唤星" width="32" height="32" />
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
