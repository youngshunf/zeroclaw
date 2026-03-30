import { useSearchParams } from 'react-router-dom';
import { PhotoProvider, PhotoSlider } from 'react-photo-view';
import 'react-photo-view/dist/react-photo-view.css';
import { RotateCw, FlipHorizontal, ZoomIn, ZoomOut, Download } from 'lucide-react';
import { useEffect } from 'react';

// Custom toolbar
function Toolbar({ onRotate, onFlip, onZoomIn, onZoomOut, onDownload }: any) {
  return (
    <div className="hx-photoview-toolbar" style={{ display: 'flex', gap: '16px', color: 'white' }}>
      <button onClick={onZoomIn} title="放大"><ZoomIn size={20} /></button>
      <button onClick={onZoomOut} title="缩小"><ZoomOut size={20} /></button>
      <button onClick={onRotate} title="旋转"><RotateCw size={20} /></button>
      <button onClick={onFlip} title="翻转"><FlipHorizontal size={20} /></button>
      <button onClick={onDownload} title="下载"><Download size={20} /></button>
    </div>
  );
}

export default function ImageViewer() {
  const [params] = useSearchParams();
  const src = params.get('src');

  // Tauri drag region over the whole background
  useEffect(() => {
    // Add drag region style to the photo-view backdrop so users can drag the window
    const style = document.createElement('style');
    style.innerHTML = `
      .PhotoView-Slider__Backdrop { -webkit-app-region: drag; }
      .PhotoView-Slider__BannerWrap, .hx-photoview-toolbar { -webkit-app-region: no-drag; }
    `;
    document.head.appendChild(style);
    return () => style.remove();
  }, []);

  const handleClose = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      getCurrentWindow().close();
    } catch {
      window.close();
    }
  };

  if (!src) return null;

  return (
    <div style={{ width: '100vw', height: '100vh', background: 'transparent' }} data-tauri-drag-region>
      <PhotoProvider
        maskOpacity={1}
        speed={() => 300}
        toolbarRender={({ rotate, onRotate, onScale, scale }) => {
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
        <PhotoSlider
          images={[{ src, key: src }]}
          visible={true}
          onClose={handleClose}
          index={0}
          onIndexChange={() => {}}
        />
      </PhotoProvider>
    </div>
  );
}
