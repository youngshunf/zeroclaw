import { Bot, MessageSquare, Users, Store, Settings } from 'lucide-react';

export type MobileTabKey = 'agent' | 'hasn' | 'contacts' | 'market' | 'settings';

interface BottomTabBarProps {
  activeTab: MobileTabKey;
  onTabChange: (tab: MobileTabKey) => void;
  badges?: Partial<Record<MobileTabKey, number>>;
}

const tabs: { key: MobileTabKey; icon: typeof Bot; label: string }[] = [
  { key: 'agent', icon: Bot, label: 'AI 助手' },
  { key: 'hasn', icon: MessageSquare, label: '消息' },
  { key: 'contacts', icon: Users, label: '通讯录' },
  { key: 'market', icon: Store, label: '市场' },
  { key: 'settings', icon: Settings, label: '设置' },
];

/**
 * 移动端底部 Tab 导航栏
 *
 * 替代桌面端的左侧 NavRail，遵循 iOS/Android 底部导航规范：
 * - 5 个核心 Tab
 * - 红点徽章
 * - Safe Area 底部适配
 * - Active 状态高亮
 */
export default function BottomTabBar({ activeTab, onTabChange, badges = {} }: BottomTabBarProps) {
  return (
    <nav className="hx-bottom-tab">
      {tabs.map(({ key, icon: Icon, label }) => (
        <button
          key={key}
          className={`hx-bottom-tab-item${activeTab === key ? ' active' : ''}`}
          onClick={() => onTabChange(key)}
        >
          <div className="hx-bottom-tab-icon-wrap">
            <Icon size={22} />
            {(badges[key] ?? 0) > 0 && (
              <span className="hx-bottom-tab-badge">
                {(badges[key] ?? 0) > 99 ? '99+' : badges[key]}
              </span>
            )}
          </div>
          <span className="hx-bottom-tab-label">{label}</span>
        </button>
      ))}
    </nav>
  );
}
