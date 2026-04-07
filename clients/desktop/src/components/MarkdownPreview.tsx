import React, { useCallback, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import MermaidViewer from './mermaid/MermaidViewer';
import { CodeBlock, InlineCode } from './markdown/CodeBlock';

export interface MarkdownPreviewProps {
  content: string;
}

const MarkdownPreview = React.memo(({ content }: MarkdownPreviewProps) => {
  const [copied, setCopied] = useState(false);
  const contentRef = React.useRef<HTMLDivElement>(null);

  // 复制整篇文档为富文本（保留格式）
  const handleCopyRichText = useCallback(async () => {
    const el = contentRef.current;
    if (!el) return;
    try {
      // 获取渲染后的 HTML 作为富文本
      const html = el.innerHTML;
      const plainText = el.innerText;
      
      const blob = new Blob([html], { type: 'text/html' });
      const textBlob = new Blob([plainText], { type: 'text/plain' });
      
      await navigator.clipboard.write([
        new ClipboardItem({
          'text/html': blob,
          'text/plain': textBlob,
        })
      ]);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy rich text:', err);
      // 降级：复制纯文本
      try {
        await navigator.clipboard.writeText(content);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch {}
    }
  }, [content]);

  return (
    <div className="h-full overflow-y-auto px-4 py-4 scroll-smooth bg-hx-bg-main relative" data-tauri-drag-region="true">
      {/* 浮动复制按钮 */}
      <button
        onClick={handleCopyRichText}
        className="absolute top-3 right-5 z-20 px-2.5 py-1.5 rounded-md border border-hx-border bg-hx-bg-panel/80 backdrop-blur-sm text-hx-text-tertiary text-xs font-medium cursor-pointer hover:bg-hx-bg-hover hover:text-hx-text-primary transition-all flex items-center gap-1.5 opacity-0 hover:opacity-100 focus:opacity-100 group-hover:opacity-70"
        style={{ opacity: copied ? 1 : undefined }}
        title="复制为富文本"
      >
        {copied ? (
          <>
            <svg className="w-3.5 h-3.5 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
            已复制
          </>
        ) : (
          <>
            <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            复制文档
          </>
        )}
      </button>

      <div className="hx-markdown w-full min-h-full pb-[20vh] group" ref={contentRef}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          // @ts-ignore
          rehypePlugins={[rehypeRaw]}
          components={{
            code(props) {
              const { children, className, node, ...rest } = props;
              const match = /language-(\w+)/.exec(className || '');
              const codeStr = String(children).replace(/\n$/, '');
              
              // Mermaid 流程图
              if (match && match[1] === 'mermaid') {
                return <MermaidViewer code={codeStr} />;
              }
              
              // 围栏代码块（有语言标记或被 <pre> 包裹）
              const isBlock = node && (node as any).position && 
                (node as any).position.start.line !== (node as any).position.end.line;
              
              if (match || isBlock) {
                return (
                  <CodeBlock 
                    code={codeStr} 
                    language={match ? match[1] : 'text'} 
                    mode="full" 
                  />
                );
              }
              
              // 行内代码
              return <InlineCode {...rest} className={className}>{children}</InlineCode>;
            },
            // pre 标签需要透传，让 code 组件自行处理
            pre({ children }) {
              return <>{children}</>;
            }
          }}
        >
          {content || '*暂无内容*'}
        </ReactMarkdown>
      </div>
    </div>
  );
});

export default MarkdownPreview;
