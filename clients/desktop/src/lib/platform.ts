/**
 * 平台检测工具 — 纯函数版本（非 React 场景使用）
 *
 * 提供统一的平台判断能力，用于：
 * - CSS/布局条件渲染
 * - 引擎 API 路由（HTTP vs FFI）
 * - 功能降级（如桌面端独有的 Sidecar 管理）
 */

/** 是否运行在 Tauri 环境中（桌面或移动） */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__?.invoke;
}

/** 是否为触控设备（移动端或触屏笔记本） */
export function isTouchDevice(): boolean {
  return typeof window !== 'undefined' && ('ontouchstart' in window || navigator.maxTouchPoints > 0);
}

// Vite 编译时注入的平台标识（来自 TAURI_ENV_PLATFORM）
declare const __TAURI_PLATFORM__: string;
const _tauriPlatform = typeof __TAURI_PLATFORM__ !== 'undefined' ? __TAURI_PLATFORM__ : '';

/** 是否为 iOS 平台 */
export function isIOS(): boolean {
  if (typeof navigator === 'undefined') return false;
  // 1. Vite 编译时常量（最可靠）
  if (_tauriPlatform === 'ios') return true;
  // 2. UA 直接匹配
  if (/iPad|iPhone|iPod/.test(navigator.userAgent)) return true;
  // 3. iPad 桌面 UA 模式
  if (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1) return true;
  return false;
}

/** 是否为 Android 平台 */
export function isAndroid(): boolean {
  if (typeof navigator === 'undefined') return false;
  if (_tauriPlatform === 'android') return true;
  return /Android/.test(navigator.userAgent);
}

/** 是否为移动端（iOS 或 Android） */
export function isMobileOS(): boolean {
  if (_tauriPlatform === 'ios' || _tauriPlatform === 'android') return true;
  return isIOS() || isAndroid();
}

/** 是否为 Tauri 移动端（Tauri + 移动 OS） */
export function isTauriMobile(): boolean {
  return isTauri() && isMobileOS();
}

/** 是否为 Tauri 桌面端 */
export function isTauriDesktop(): boolean {
  return isTauri() && !isMobileOS();
}

/** 是否为唤星桌面端（带有桌面端标识） */
export function isHuanxingDesktop(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__HUANXING_DESKTOP__;
}

/** 获取屏幕尺寸类别 */
export type ScreenSize = 'mobile' | 'tablet' | 'desktop';

export function getScreenSize(): ScreenSize {
  if (typeof window === 'undefined') return 'desktop';
  const w = window.innerWidth;
  if (w < 768) return 'mobile';
  if (w < 1024) return 'tablet';
  return 'desktop';
}

/** CSS Safe Area 值（用于 JS 计算场景） */
export function getSafeAreaInsets() {
  if (typeof getComputedStyle === 'undefined') {
    return { top: 0, bottom: 0, left: 0, right: 0 };
  }
  const style = getComputedStyle(document.documentElement);
  return {
    top: parseInt(style.getPropertyValue('--sat') || '0', 10),
    bottom: parseInt(style.getPropertyValue('--sab') || '0', 10),
    left: parseInt(style.getPropertyValue('--sal') || '0', 10),
    right: parseInt(style.getPropertyValue('--sar') || '0', 10),
  };
}
