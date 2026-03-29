/**
 * HxImageMessage — 解析消息中的 [IMAGE:] 标记并渲染为图片
 *
 * ZeroClaw multimodal 格式：[IMAGE:/path/to/file.png]
 * 本组件将其替换为可点击的图片（原始比例，限制最大宽度），
 * 点击使用 react-photo-view 提供完整预览（缩放/旋转/下载）。
 */
import React, { useMemo } from 'react';
import { HxPhotoProvider, PhotoView, localPathToSrc } from './HxImageLightbox';

const IMAGE_MARKER_RE = /\[IMAGE:([^\]]+)\]/g;

export interface ParsedContentPart {
  type: 'text' | 'image';
  content: string; // text content or image path
}

/**
 * 将消息内容解析为 text + image 部分
 */
export function parseImageMarkers(content: string): ParsedContentPart[] {
  const parts: ParsedContentPart[] = [];
  let lastIndex = 0;

  // Reset regex state
  IMAGE_MARKER_RE.lastIndex = 0;

  let match: RegExpExecArray | null;
  while ((match = IMAGE_MARKER_RE.exec(content)) !== null) {
    // Text before marker
    if (match.index > lastIndex) {
      const text = content.slice(lastIndex, match.index).trim();
      if (text) {
        parts.push({ type: 'text', content: text });
      }
    }

    // Image marker
    parts.push({ type: 'image', content: match[1].trim() });
    lastIndex = match.index + match[0].length;
  }

  // Remaining text
  if (lastIndex < content.length) {
    const text = content.slice(lastIndex).trim();
    if (text) {
      parts.push({ type: 'text', content: text });
    }
  }

  return parts;
}

/**
 * 检查消息是否包含 [IMAGE:] 标记
 */
export function containsImageMarkers(content: string): boolean {
  IMAGE_MARKER_RE.lastIndex = 0;
  return IMAGE_MARKER_RE.test(content);
}

/**
 * 渲染包含图片标记的消息
 */
export interface HxImageMessageProps {
  /** 原始消息内容 */
  content: string;
  /** 文字部分的渲染器（通常为 Markdown） */
  renderText?: (text: string) => React.ReactNode;
}

export function HxImageMessage({ content, renderText }: HxImageMessageProps) {
  const parts = useMemo(() => parseImageMarkers(content), [content]);

  // 如果没有图片标记，不做特殊处理
  if (parts.length === 0 || !parts.some(p => p.type === 'image')) {
    return renderText ? <>{renderText(content)}</> : <span>{content}</span>;
  }

  // 收集所有图片 src 用于 PhotoProvider 画廊
  const imageSrcs = parts
    .filter(p => p.type === 'image')
    .map(p => localPathToSrc(p.content));

  return (
    <HxPhotoProvider>
      {parts.map((part, i) => {
        if (part.type === 'text') {
          return renderText ? (
            <React.Fragment key={i}>{renderText(part.content)}</React.Fragment>
          ) : (
            <span key={i}>{part.content}</span>
          );
        }

        // Image part — 原始比例，限制最大宽度
        const src = localPathToSrc(part.content);

        return (
          <div key={i} className="hx-msg-image-container">
            <PhotoView src={src}>
              <img
                src={src}
                className="hx-msg-image"
                loading="lazy"
                onError={(e) => {
                  // 图片加载失败时显示 fallback
                  const target = e.currentTarget;
                  target.style.display = 'none';
                  const fallback = target.parentElement?.querySelector('.hx-msg-image-fallback') as HTMLElement;
                  if (fallback) fallback.style.display = 'flex';
                }}
              />
            </PhotoView>
            <div className="hx-msg-image-fallback" style={{ display: 'none' }}>
              <span>📷 图片无法加载</span>
            </div>
          </div>
        );
      })}
    </HxPhotoProvider>
  );
}
