import { ChevronLeft } from 'lucide-react';

interface MobileHeaderProps {
  /** 页面标题 */
  title: string;
  /** 副标题（可选） */
  subtitle?: string;
  /** 是否显示返回按钮 */
  showBack?: boolean;
  /** 返回按钮回调 */
  onBack?: () => void;
  /** 右侧操作区域 */
  actions?: React.ReactNode;
  /** 是否透明背景 */
  transparent?: boolean;
}

/**
 * 移动端顶部导航栏
 *
 * 替代桌面端的 Header 组件，提供：
 * - 返回按钮（用于导航栈内的子页面）
 * - 居中标题
 * - 右侧操作按钮区
 * - Safe Area 顶部适配
 */
export default function MobileHeader({
  title,
  subtitle,
  showBack = false,
  onBack,
  actions,
  transparent = false,
}: MobileHeaderProps) {
  return (
    <header className={`hx-mobile-header${transparent ? ' transparent' : ''}`}>
      <div className="hx-mobile-header-left">
        {showBack && (
          <button className="hx-mobile-header-back" onClick={onBack}>
            <ChevronLeft size={24} />
          </button>
        )}
      </div>

      <div className="hx-mobile-header-center">
        <h1 className="hx-mobile-header-title">{title}</h1>
        {subtitle && <p className="hx-mobile-header-subtitle">{subtitle}</p>}
      </div>

      <div className="hx-mobile-header-right">
        {actions}
      </div>
    </header>
  );
}
