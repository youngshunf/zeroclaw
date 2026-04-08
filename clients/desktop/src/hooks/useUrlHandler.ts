/**
 * useUrlHandler — 聊天消息中的链接点击拦截
 *
 * 规则：
 *   1. 文档分享链接 → 跳转到桌面端文档详情页
 *   2. 其他普通链接 → 打开系统默认浏览器
 */
import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { getHuanxingSession, HUANXING_CONFIG } from '@/config';
import { getHuanxingSharedDocumentApi } from '@/lib/document-api';

/**
 * 从 URL 中提取文档分享 token（如有）。
 *
 * 支持的格式：
 *   - https://huanxing.dcfuture.cn/s/{token}
 *   - https://huanxing.dcfuture.cn/doc/share/{token}
 *   - /s/{token}  （相对路径）
 *   - /doc/share/{token}
 */
function extractDocUrlInfo(url: string): { type: 'share', token: string } | { type: 'id', id: string } | null {
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
  if (shortMatch) return { type: 'share', token: shortMatch[1] };

  // /doc/share/{token}
  const longMatch = pathname.match(/^\/doc\/share\/([a-zA-Z0-9_\-]+)/);
  if (longMatch) return { type: 'share', token: longMatch[1] };

  // /d/{id} or /docs?id={id}
  const idMatch = pathname.match(/^\/d\/(\d+)/) || pathname.match(/^\/docs\?id=(\d+)/);
  if (idMatch) return { type: 'id', id: idMatch[1] };

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

  return useCallback(async (url: string) => {
    if (!url) return;

    const info = extractDocUrlInfo(url);
    if (info) {
      if (info.type === 'id') {
        navigate(`/docs?id=${info.id}`);
        return;
      }
      
      if (info.type === 'share') {
        const token = info.token;
        try {
          const session = getHuanxingSession();
          // 解码分享链接：获取真实的文档 ID
          const res = await getHuanxingSharedDocumentApi(session?.accessToken || '', token);
          if (res?.data?.id) {
             navigate(`/docs?id=${res.data.id}&share=${encodeURIComponent(token)}`);
             return;
          }
        } catch {
          // 解码失败时退化为纯分享形态
        }
        navigate(`/docs?share=${encodeURIComponent(token)}`);
        return;
      }
    }

    // 普通链接 → 打开系统默认浏览器
    openExternal(url);
  }, [navigate]);
}

export default useUrlHandler;
