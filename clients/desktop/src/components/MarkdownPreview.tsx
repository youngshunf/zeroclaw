import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import MermaidViewer from './mermaid/MermaidViewer';

export interface MarkdownPreviewProps {
  content: string;
}

export default function MarkdownPreview({ content }: MarkdownPreviewProps) {
  return (
    <div className="h-full overflow-y-auto px-4 py-4 scroll-smooth bg-hx-bg-main" data-tauri-drag-region="true">
      <div className="hx-markdown w-full min-h-full pb-[20vh]">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          // @ts-ignore
          rehypePlugins={[rehypeRaw]}
          components={{
            code(props) {
              const { children, className, node, ...rest } = props;
              const match = /language-(\w+)/.exec(className || '');
              
              if (match && match[1] === 'mermaid') {
                return <MermaidViewer code={String(children).replace(/\n$/, '')} />;
              }
              
              return <code {...rest} className={className}>{children}</code>;
            }
          }}
        >
          {content || '*暂无内容*'}
        </ReactMarkdown>
      </div>
    </div>
  );
}
