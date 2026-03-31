import { useEffect, useRef, useState, useMemo } from 'react';
import mermaid from 'mermaid';
import { Download, Maximize2, Loader2, AlertCircle } from 'lucide-react';
import { HxPhotoProvider, PhotoView } from '../chat/HxImageLightbox';
import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';

export interface MermaidViewerProps {
  code: string;
}

export default function MermaidViewer({ code }: MermaidViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [svgStr, setSvgStr] = useState<string>('');
  const [error, setError] = useState<string>('');
  const [isRendering, setIsRendering] = useState(true);

  // 初始化 Mermaid 配置，针对深浅模式可扩展
  useEffect(() => {
    mermaid.initialize({
      startOnLoad: false,
      theme: 'neutral', // 使用精致的 neutral 主题
      securityLevel: 'loose',
      fontFamily: 'inherit',
    });
  }, []);

  // 渲染 Mermaid
  useEffect(() => {
    let isMounted = true;
    const renderDiagram = async () => {
      if (!code || !containerRef.current) return;
      try {
        setIsRendering(true);
        setError('');
        // 生成随机ID防止多次渲染冲突
        const id = `mermaid-render-${Math.random().toString(36).substring(2, 9)}`;
        const { svg } = await mermaid.render(id, code);
        if (isMounted) {
          setSvgStr(svg);
        }
      } catch (e: any) {
        if (isMounted) {
          setError(e.message || '渲染 Mermaid 图表失败');
          console.error('Mermaid render error:', e);
        }
      } finally {
        if (isMounted) setIsRendering(false);
      }
    };
    renderDiagram();
    return () => { isMounted = false; };
  }, [code]);

  // 将 SVG 原生字符串转为 Base64 图片 (以便支持下载和放大)
  const svgDataUrl = useMemo(() => {
    if (!svgStr) return '';
    try {
      // 解决中文由于 btoa 报错的问题
      return `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(svgStr)))}`;
    } catch {
      return '';
    }
  }, [svgStr]);

  // 下载为 JPG
  const handleDownloadJPG = () => {
    if (!svgDataUrl) return;
    
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement('canvas');
      // 图像四周留出 20px padding，更美观
      const padding = 20;
      canvas.width = img.width + padding * 2;
      canvas.height = img.height + padding * 2;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      
      // 必须填充纯白背景，否则透明部分会变成黑色
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      
      // 绘制图像（带 padding 偏移）
      ctx.drawImage(img, padding, padding);
      
      // 导出为 JPEG 格式
      canvas.toBlob(async (blob) => {
        if (!blob) return;
        
        try {
          // 调起 Tauri 原生路径选择界面保存文件
          const savePath = await save({
            title: '保存图表为 JPG',
            defaultPath: `mermaid-diagram-${new Date().getTime()}.jpg`,
            filters: [{ name: 'Image', extensions: ['jpg', 'jpeg'] }]
          });
          
          if (savePath) {
            const buffer = await blob.arrayBuffer();
            await writeFile(savePath, new Uint8Array(buffer));
          }
        } catch (e) {
          console.error('Save dialog or write failed:', e);
          // Fallback 至浏览器下载
          const blobUrl = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = blobUrl;
          a.download = `mermaid-diagram-${new Date().getTime()}.jpg`;
          document.body.appendChild(a);
          a.click();
          document.body.removeChild(a);
          setTimeout(() => URL.revokeObjectURL(blobUrl), 1000);
        }
      }, 'image/jpeg', 1.0);
    };
    img.src = svgDataUrl;
  };

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center p-6 bg-red-50 dark:bg-red-900/10 rounded-hx-radius-md border border-red-200 dark:border-red-900/30 text-red-500 my-4">
        <AlertCircle className="w-8 h-8 mb-2" />
        <p className="text-[13px] font-medium text-center">无法解析或渲染此 Mermaid 图表</p>
        <pre className="mt-2 text-[11px] p-2 bg-red-100 dark:bg-red-900/20 rounded max-w-full overflow-auto text-red-700 dark:text-red-400 whitespace-pre-wrap text-left">
          {error}
        </pre>
      </div>
    );
  }

  return (
    <div className="relative group my-4 rounded-hx-radius-md border border-hx-border bg-white overflow-hidden shadow-sm">
      {/* 操作条 - 悬浮显示 */}
      <div className="absolute top-2 right-2 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity z-10 bg-hx-bg-main/90 backdrop-blur-sm p-1 rounded-hx-radius-sm border border-hx-border shadow-sm">
        {svgDataUrl && (
          <>
            <HxPhotoProvider>
              <PhotoView src={svgDataUrl}>
                <button
                  className="w-8 h-8 flex items-center justify-center rounded hover:bg-hx-bg-hover text-hx-text-secondary hover:text-hx-text-primary transition-colors cursor-pointer"
                  title="放大查看 (支持缩放拖拽)"
                >
                  <Maximize2 size={15} />
                </button>
              </PhotoView>
            </HxPhotoProvider>
            
            <div className="w-[1px] h-4 bg-hx-border mx-1"></div>
            
            <button
              onClick={handleDownloadJPG}
              className="w-8 h-8 flex items-center justify-center rounded hover:bg-hx-bg-hover text-hx-text-secondary hover:text-hx-text-primary transition-colors cursor-pointer"
              title="下载为 JPG"
            >
              <Download size={15} />
            </button>
          </>
        )}
      </div>

      {/* SVG 渲染区 */}
      <div 
        className="w-full flex items-center justify-center p-6 min-h-[120px] overflow-x-auto"
        ref={containerRef}
      >
        {isRendering ? (
          <div className="flex items-center gap-2 text-hx-text-tertiary">
            <Loader2 className="animate-spin w-5 h-5" />
            <span className="text-[13px]">正在渲染图表...</span>
          </div>
        ) : (
          <div 
            className="hx-mermaid-render scale-100 origin-center"
            dangerouslySetInnerHTML={{ __html: svgStr }} 
          />
        )}
      </div>
    </div>
  );
}
