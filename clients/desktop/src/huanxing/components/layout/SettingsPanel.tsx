import { NavLink, Outlet } from 'react-router-dom';
import {
  LayoutDashboard,
  Puzzle,
  Brain,
  Wrench,
  Clock,
  Settings,
  DollarSign,
  Activity,
  Stethoscope,
  Smartphone,
  Info,
  Cpu,
} from 'lucide-react';
import { t } from '@/lib/i18n';
import { useLocaleContext } from '@/App';

function useSettingsGroups() {
  // Re-read locale so labels re-render on language switch
  const { locale: _ } = useLocaleContext();

  return [
    {
      label: t('settings.general') || '常规',
      items: [
        { to: '/dashboard', icon: LayoutDashboard, label: t('nav.dashboard') },
        { to: '/integrations', icon: Puzzle, label: t('nav.integrations') },
        { to: '/memory', icon: Brain, label: t('nav.memory') },
        { to: '/tools', icon: Wrench, label: t('nav.tools') },
        { to: '/cron', icon: Clock, label: t('nav.cron') },
      ],
    },
    {
      label: t('settings.system') || '系统',
      items: [
        { to: '/config', icon: Settings, label: t('nav.config') },
        { to: '/engine', icon: Cpu, label: t('settings.engine') || 'AI 引擎' },
        { to: '/cost', icon: DollarSign, label: t('nav.cost') },
        { to: '/logs', icon: Activity, label: t('nav.logs') },
        { to: '/doctor', icon: Stethoscope, label: t('nav.doctor') },
        { to: '/devices', icon: Smartphone, label: t('nav.devices') },
      ],
    },
    {
      label: t('settings.about') || '关于',
      items: [{ to: '/about', icon: Info, label: t('settings.about_app') || '关于唤星' }],
    },
  ];
}

export default function SettingsPanel() {
  const settingsGroups = useSettingsGroups();

  return (
    <div className="hx-settings">
      <aside className="hx-settings-nav">
        <div className="hx-panel-header">
          <h2 className="hx-panel-title">{t('settings.title') || '设置中心'}</h2>
        </div>
        <nav className="hx-settings-menu">
          {settingsGroups.map((group) => (
            <div key={group.label} className="hx-settings-group">
              <div className="hx-settings-group-label">{group.label}</div>
              {group.items.map(({ to, icon: Icon, label }) => (
                <NavLink
                  key={to}
                  to={to}
                  end={to === '/dashboard'}
                  className={({ isActive }) =>
                    `hx-settings-item${isActive ? ' active' : ''}`
                  }
                >
                  <Icon size={18} />
                  <span>{label}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
      </aside>
      <div className="hx-settings-content">
        <Outlet />
      </div>
    </div>
  );
}
