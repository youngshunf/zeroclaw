import React from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import MermaidViewer from './mermaid/MermaidViewer';
import { CodeBlock, InlineCode } from './markdown/CodeBlock';

export interface MarkdownPreviewProps {
  content: string;
  onUrlClick?: (url: string) => void;
}

const MarkdownPreview = React.memo(({ content, onUrlClick }: MarkdownPreviewProps) => {
  return (
    <div className="h-full overflow-y-auto px-4 py-4 scroll-smooth bg-hx-bg-main" data-tauri-drag-region="true">
      <div className="hx-markdown w-full min-h-full pb-[20vh]">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          // @ts-ignore
          rehypePlugins={[rehypeRaw]}
          components={{
            a(props) {
              const { href, children, ...rest } = props;
              return (
                <a 
                  href={href} 
                  {...rest} 
                  target="_blank" 
                  rel="noopener noreferrer"
                  onClick={(e) => {
                    if (onUrlClick && href) {
                      e.preventDefault();
                      onUrlClick(href);
                    }
                  }}
                >
                  {children}
                </a>
              );
            },
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
