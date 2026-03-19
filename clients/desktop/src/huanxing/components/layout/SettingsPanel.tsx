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

const settingsGroups = [
  {
    label: '常规',
    items: [
      { to: '/dashboard', icon: LayoutDashboard, label: '仪表盘' },
      { to: '/integrations', icon: Puzzle, label: 'AI 接入' },
      { to: '/memory', icon: Brain, label: '记忆管理' },
      { to: '/tools', icon: Wrench, label: '工具列表' },
      { to: '/cron', icon: Clock, label: '定时任务' },
    ],
  },
  {
    label: '系统',
    items: [
      { to: '/config', icon: Settings, label: '配置编辑' },
      { to: '/engine', icon: Cpu, label: 'AI 引擎' },
      { to: '/cost', icon: DollarSign, label: '费用追踪' },
      { to: '/logs', icon: Activity, label: '系统日志' },
      { to: '/doctor', icon: Stethoscope, label: '系统诊断' },
      { to: '/devices', icon: Smartphone, label: '设备管理' },
    ],
  },
  {
    label: '关于',
    items: [{ to: '/about', icon: Info, label: '关于唤星' }],
  },
];

export default function SettingsPanel() {
  return (
    <div className="hx-settings">
      <aside className="hx-settings-nav">
        <div className="hx-panel-header">
          <h2 className="hx-panel-title">设置中心</h2>
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
