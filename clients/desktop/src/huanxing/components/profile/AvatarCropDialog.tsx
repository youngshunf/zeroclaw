/**
 * 头像裁剪弹窗组件
 * 纯 CSS/JS 实现，无外部依赖
 */
import { useState, useRef, useCallback, useEffect } from 'react';
import { getCroppedImg } from '../../lib/cropImage';

interface AvatarCropDialogProps {
  imageSrc: string;
  onCropComplete: (blob: Blob) => void;
  onClose: () => void;
}

export default function AvatarCropDialog({ imageSrc, onCropComplete, onClose }: AvatarCropDialogProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [imgSize, setImgSize] = useState({ w: 0, h: 0 });
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [scale, setScale] = useState(1);
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef({ x: 0, y: 0, ox: 0, oy: 0 });

  const CROP_SIZE = 256; // 裁剪框尺寸
  const CONTAINER_SIZE = 300;

  // 图片加载后计算初始缩放
  const handleImgLoad = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    setImgSize({ w, h });
    // 让短边 = CROP_SIZE
    const minScale = CROP_SIZE / Math.min(w, h);
    setScale(minScale);
    // 居中
    setOffset({
      x: (CONTAINER_SIZE - w * minScale) / 2,
      y: (CONTAINER_SIZE - h * minScale) / 2,
    });
  }, []);

  // 拖拽
  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setDragging(true);
    dragStart.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
  };

  useEffect(() => {
    if (!dragging) return;
    const handleMove = (e: MouseEvent) => {
      const dx = e.clientX - dragStart.current.x;
      const dy = e.clientY - dragStart.current.y;
      setOffset({ x: dragStart.current.ox + dx, y: dragStart.current.oy + dy });
    };
    const handleUp = () => setDragging(false);
    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [dragging]);

  // 缩放
  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    setScale((s) => Math.max(0.1, Math.min(5, s - e.deltaY * 0.001)));
  };

  const handleSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
    setScale(Number(e.target.value));
  };

  // 确认裁剪
  const handleConfirm = async () => {
    if (!imgSize.w) return;
    // 计算裁剪框在原图上的位置
    const cropLeft = (CONTAINER_SIZE - CROP_SIZE) / 2;
    const cropTop = (CONTAINER_SIZE - CROP_SIZE) / 2;
    const pixelCrop = {
      x: (cropLeft - offset.x) / scale,
      y: (cropTop - offset.y) / scale,
      width: CROP_SIZE / scale,
      height: CROP_SIZE / scale,
    };
    try {
      const blob = await getCroppedImg(imageSrc, pixelCrop, 256);
      onCropComplete(blob);
    } catch (err) {
      console.error('裁剪失败:', err);
    }
  };

  const minScale = imgSize.w ? CROP_SIZE / Math.min(imgSize.w, imgSize.h) : 0.5;

  return (
    <div className="hx-crop-overlay" onClick={onClose}>
      <div className="hx-crop-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="hx-crop-header">
          <h3>裁剪头像</h3>
          <button className="hx-crop-close" onClick={onClose}>✕</button>
        </div>
        <div
          className="hx-crop-container"
          ref={containerRef}
          onMouseDown={handleMouseDown}
          onWheel={handleWheel}
          style={{ width: CONTAINER_SIZE, height: CONTAINER_SIZE, cursor: dragging ? 'grabbing' : 'grab' }}
        >
          <img
            src={imageSrc}
            onLoad={handleImgLoad}
            className="absolute max-w-none select-none pointer-events-none"
            style={{
              left: offset.x,
              top: offset.y,
              width: imgSize.w * scale,
              height: imgSize.h * scale,
            }}
            draggable={false}
            alt=""
          />
          {/* 裁剪框遮罩 */}
          <div className="hx-crop-mask">
            <div className="hx-crop-circle" style={{ width: CROP_SIZE, height: CROP_SIZE }} />
          </div>
        </div>
        <div className="hx-crop-controls">
          <label className="hx-crop-zoom-label">
            <span>缩放</span>
            <input
              type="range"
              min={minScale}
              max={minScale * 4}
              step={0.01}
              value={scale}
              onChange={handleSlider}
              className="hx-crop-slider"
            />
          </label>
        </div>
        <div className="hx-crop-actions">
          <button className="hx-btn-secondary" onClick={onClose}>取消</button>
          <button className="hx-btn-primary" onClick={handleConfirm}>确认</button>
        </div>
      </div>
    </div>
  );
}
