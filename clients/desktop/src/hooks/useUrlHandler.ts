/**
 * useUrlHandler — 聊天消息中的链接点击拦截
 *
 * 规则：
 *   1. 文档分享链接 → 跳转到桌面端文档详情页
 *   2. 其他普通链接 → 打开系统默认浏览器
 */
import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { HUANXING_CONFIG } from '@/config';

/**
 * 从 URL 中提取文档分享 token（如有）。
 *
 * 支持的格式：
 *   - https://huanxing.dcfuture.cn/s/{token}
 *   - https://huanxing.dcfuture.cn/doc/share/{token}
 *   - /s/{token}  （相对路径）
 *   - /doc/share/{token}
 */
function extractDocShareToken(url: string): string | null {
  const siteUrl = HUANXING_CONFIG.siteUrl.replace(/\/$/, '');

  // 绝对路径：移除 siteUrl 前缀得到 pathname
  let pathname = url;
  if (url.startsWith(siteUrl)) {
    pathname = url.slice(siteUrl.length);
  } else if (url.startsWith('http://') || url.startsWith('https://')) {
    // 外部域名链接，不是文档链接
    try {
      const parsed = new URL(url);
      const siteHost = new URL(siteUrl).hostname;
      if (parsed.hostname !== siteHost) return null;
      pathname = parsed.pathname;
    } catch {
      return null;
    }
  }

  // /s/{token}
  const shortMatch = pathname.match(/^\/s\/([a-zA-Z0-9_\-]+)/);
  if (shortMatch) return shortMatch[1];

  // /doc/share/{token}
  const longMatch = pathname.match(/^\/doc\/share\/([a-zA-Z0-9_\-]+)/);
  if (longMatch) return longMatch[1];

  return null;
}

/**
 * 打开系统默认浏览器。
 * Tauri 桌面端使用 @tauri-apps/plugin-shell，
 * 非 Tauri 环境（开发调试）直接 window.open。
 */
async function openExternal(url: string) {
  try {
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(url);
  } catch {
    // 非 Tauri 环境 fallback
    window.open(url, '_blank', 'noopener');
  }
}

/**
 * Hook：返回一个链接点击处理函数，供 <Markdown onUrlClick={handler}> 使用。
 */
export function useUrlHandler() {
  const navigate = useNavigate();

  return useCallback((url: string) => {
    if (!url) return;

    const token = extractDocShareToken(url);
    if (token) {
      // 文档链接 → 跳转到文档页面并传递 share token
      navigate(`/docs?share=${encodeURIComponent(token)}`);
      return;
    }

    // 普通链接 → 打开系统默认浏览器
    openExternal(url);
  }, [navigate]);
}

export default useUrlHandler;
