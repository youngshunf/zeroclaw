import { useEffect, useRef, useState, useCallback } from 'react';
import { X } from 'lucide-react';

interface BottomSheetProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  /** 最大高度占屏幕百分比 (0-100)，默认 85 */
  maxHeight?: number;
}

/**
 * 移动端底部抽屉组件
 *
 * 用于移动端替代桌面端的弹窗/面板：
 * - 从底部滑入，支持手势下拉关闭
 * - 背景遮罩点击关闭
 * - Safe Area 底部适配
 * - Radix UI 对话框的移动端替代
 */
export default function BottomSheet({
  isOpen,
  onClose,
  title,
  children,
  maxHeight = 85,
}: BottomSheetProps) {
  const [visible, setVisible] = useState(false);
  const [animate, setAnimate] = useState(false);
  const sheetRef = useRef<HTMLDivElement>(null);
  const startY = useRef(0);
  const currentY = useRef(0);

  // Open/close animation
  useEffect(() => {
    if (isOpen) {
      setVisible(true);
      requestAnimationFrame(() => {
        requestAnimationFrame(() => setAnimate(true));
      });
    } else {
      setAnimate(false);
      const timer = setTimeout(() => setVisible(false), 300);
      return () => clearTimeout(timer);
    }
  }, [isOpen]);

  // Touch gestures for swipe-to-close
  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    startY.current = e.touches[0].clientY;
    currentY.current = 0;
  }, []);

  const handleTouchMove = useCallback((e: React.TouchEvent) => {
    const delta = e.touches[0].clientY - startY.current;
    if (delta > 0) {
      currentY.current = delta;
      if (sheetRef.current) {
        sheetRef.current.style.transform = `translateY(${delta}px)`;
      }
    }
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (currentY.current > 100) {
      // Swiped down enough → close
      onClose();
    } else if (sheetRef.current) {
      // Snap back
      sheetRef.current.style.transform = '';
    }
    currentY.current = 0;
  }, [onClose]);

  if (!visible) return null;

  return (
    <div
      className={`hx-bottom-sheet-overlay${animate ? ' open' : ''}`}
      onClick={onClose}
    >
      <div
        ref={sheetRef}
        className={`hx-bottom-sheet${animate ? ' open' : ''}`}
        style={{ maxHeight: `${maxHeight}vh` }}
        onClick={(e) => e.stopPropagation()}
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
      >
        {/* Drag handle */}
        <div className="hx-bottom-sheet-handle">
          <div className="hx-bottom-sheet-handle-bar" />
        </div>

        {/* Header */}
        {title && (
          <div className="hx-bottom-sheet-header">
            <h3>{title}</h3>
            <button onClick={onClose} className="hx-bottom-sheet-close">
              <X size={20} />
            </button>
          </div>
        )}

        {/* Content */}
        <div className="hx-bottom-sheet-content">
          {children}
        </div>
      </div>
    </div>
  );
}
