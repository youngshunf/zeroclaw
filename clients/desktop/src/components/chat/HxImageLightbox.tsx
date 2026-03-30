/**
 * HxImageLightbox — 图片预览组件
 *
 * 基于 react-photo-view 提供完整的图片查看功能：
 * - 鼠标滚轮 / 手势缩放
 * - 拖拽平移
 * - 旋转
 * - 翻转
 * - 下载
 * - 工具栏操作
 */
import React, { useCallback } from 'react';
import { PhotoProvider, PhotoView } from 'react-photo-view';
import 'react-photo-view/dist/react-photo-view.css';
import { RotateCw, FlipHorizontal, ZoomIn, ZoomOut, Download } from 'lucide-react';

// ── 辅助：将本地文件路径转为可在 webview 中显示的 URL ──────────

import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * 将本地文件路径转为 webview 可显示的 URL
 *
 * - Tauri 环境: 使用 convertFileSrc → https://asset.localhost/...
 * - 已是 URL (blob:, data:, http:) 的直接返回
 * - 非 Tauri fallback: 通过 gateway 代理
 */
export function localPathToSrc(filePath: string): string {
  // 已经是可用 URL
  if (filePath.startsWith('blob:') || filePath.startsWith('data:') || filePath.startsWith('http')) {
    return filePath;
  }

  // Tauri v2：使用 convertFileSrc
  try {
    return convertFileSrc(filePath);
  } catch {
    // 非 Tauri 环境
  }

  // 开发模式 fallback — 通过 sidecar 代理
  return `/api/file?path=${encodeURIComponent(filePath)}`;
}

/**
 * 检测 filePath 是否是图片
 */
export function isImagePath(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() || '';
  return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'tiff', 'tif'].includes(ext);
}

// ── 自定义工具栏 ──────────────────────────────────────────────
interface ToolbarProps {
  onRotate: () => void;
  onFlip: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onDownload: () => void;
}

function Toolbar({ onRotate, onFlip, onZoomIn, onZoomOut, onDownload }: ToolbarProps) {
  return (
    <div className="hx-photoview-toolbar">
      <button onClick={onZoomIn} title="放大"><ZoomIn size={18} /></button>
      <button onClick={onZoomOut} title="缩小"><ZoomOut size={18} /></button>
      <button onClick={onRotate} title="旋转"><RotateCw size={18} /></button>
      <button onClick={onFlip} title="翻转"><FlipHorizontal size={18} /></button>
      <button onClick={onDownload} title="下载"><Download size={18} /></button>
    </div>
  );
}

// ── PhotoProvider wrapper ──────────────────────────────────────
/**
 * 图片查看器 Provider — 包裹子组件中的所有 PhotoView
 *
 * 使用方式：
 * ```tsx
 * <HxPhotoProvider>
 *   <PhotoView src={imageUrl}>
 *     <img src={imageUrl} style={{ cursor: 'pointer' }} />
 *   </PhotoView>
 * </HxPhotoProvider>
 * ```
 */
export function HxPhotoProvider({ children }: { children: React.ReactNode }) {
  return (
    <PhotoProvider
      speed={() => 300}
      maskOpacity={0.85}
      toolbarRender={({ rotate, onRotate, onScale, scale, index, images }) => {
        const currentImage = images[index];
        const src = currentImage?.src || '';

        const handleDownload = () => {
          if (!src) return;
          const link = document.createElement('a');
          link.href = src;
          link.download = src.split('/').pop() || 'image';
          link.target = '_blank';
          document.body.appendChild(link);
          link.click();
          document.body.removeChild(link);
        };

        return (
          <Toolbar
            onZoomIn={() => onScale(scale + 0.5)}
            onZoomOut={() => onScale(scale > 0.5 ? scale - 0.5 : scale)}
            onRotate={() => onRotate(rotate + 90)}
            onFlip={() => onScale(-scale)}
            onDownload={handleDownload}
          />
        );
      }}
    >
      {children}
    </PhotoProvider>
  );
}

// re-export PhotoView for convenience
export { PhotoView } from 'react-photo-view';
