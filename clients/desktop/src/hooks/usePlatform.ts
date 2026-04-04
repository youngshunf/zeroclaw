import { useState, useEffect, useMemo } from 'react';
import {
  isTauri,
  isTauriMobile,
  isTauriDesktop,
  isTouchDevice,
  isIOS,
  isAndroid,
  type ScreenSize,
} from '@/lib/platform';

/**
 * 平台检测 React Hook
 *
 * 所有组件通过此 Hook 统一判断当前平台，实现响应式渲染。
 * 自动监听窗口尺寸变化，实时更新 screenSize。
 *
 * @example
 * ```tsx
 * function MyComponent() {
 *   const { isMobile, isDesktop } = usePlatform();
 *   return isMobile ? <MobileView /> : <DesktopView />;
 * }
 * ```
 */
export function usePlatform() {
  const [screenSize, setScreenSize] = useState<ScreenSize>(() => {
    if (typeof window === 'undefined') return 'desktop';
    const w = window.innerWidth;
    if (w < 768) return 'mobile';
    if (w < 1024) return 'tablet';
    return 'desktop';
  });

  useEffect(() => {
    const update = () => {
      const w = window.innerWidth;
      if (w < 768) setScreenSize('mobile');
      else if (w < 1024) setScreenSize('tablet');
      else setScreenSize('desktop');
    };

    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, []);

  return useMemo(() => ({
    /** 当前屏幕尺寸类别 */
    screenSize,
    /** 是否为手机尺寸 (<768px) 或移动 OS */
    isMobile: screenSize === 'mobile' || isTauriMobile(),
    /** 是否为平板尺寸 (768-1024px) */
    isTablet: screenSize === 'tablet',
    /** 是否为桌面尺寸 (>1024px) 且非移动 OS */
    isDesktop: screenSize === 'desktop' && !isTauriMobile(),
    /** 是否运行在 Tauri 环境中 */
    isTauri: isTauri(),
    /** 是否为 Tauri 移动端 */
    isTauriMobile: isTauriMobile(),
    /** 是否为 Tauri 桌面端 */
    isTauriDesktop: isTauriDesktop(),
    /** 是否为触控设备 */
    isTouchDevice: isTouchDevice(),
    /** 是否为 iOS */
    isIOS: isIOS(),
    /** 是否为 Android */
    isAndroid: isAndroid(),
  }), [screenSize]);
}
