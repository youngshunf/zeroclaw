import * as React from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import rehypeRaw from 'rehype-raw'
import remarkGfm from 'remark-gfm'
import { cn } from '../../lib/utils'
import { CodeBlock, InlineCode } from './CodeBlock'
import { preprocessLinks } from './linkify'
import remarkCollapsibleSections from './remarkCollapsibleSections'
import { CollapsibleSection } from './CollapsibleSection'
import { useCollapsibleMarkdown } from './CollapsibleMarkdownContext'
import { wrapWithSafeProxy } from './safe-components'
import MermaidViewer from '../mermaid/MermaidViewer'

export type RenderMode = 'terminal' | 'minimal' | 'full'

export interface MarkdownProps {
  children: string
  mode?: RenderMode
  className?: string
  id?: string
  onUrlClick?: (url: string) => void
  collapsible?: boolean
}

interface CollapsibleContext {
  collapsedSections: Set<string>
  toggleSection: (id: string) => void
}

function createComponents(
  mode: RenderMode,
  onUrlClick?: (url: string) => void,
  collapsibleContext?: CollapsibleContext | null,
): Partial<Components> {
  const baseComponents: Record<string, any> & Partial<Components> = {
    // Handle custom XML tags from AI responses
    example: ({ children }: { children?: React.ReactNode }) => <span className="block my-2 pl-3 border-l-2 border-gray-300 text-gray-500">{children}</span>,
    // Section wrapper for collapsible headings
    div: ({ node, children, ...props }) => {
      const sectionId = (props as Record<string, unknown>)['data-section-id'] as string | undefined
      const headingLevel = (props as Record<string, unknown>)['data-heading-level'] as number | undefined
      if (sectionId && headingLevel && collapsibleContext) {
        return (
          <CollapsibleSection
            sectionId={sectionId}
            headingLevel={headingLevel}
            isCollapsed={collapsibleContext.collapsedSections.has(sectionId)}
            onToggle={collapsibleContext.toggleSection}
          >
            {children}
          </CollapsibleSection>
        )
      }
      return <div {...props}>{children}</div>
    },
    a: ({ href, children }) => {
      const handleClick = (e: React.MouseEvent) => {
        e.preventDefault()
        if (href && onUrlClick) {
          onUrlClick(href)
        } else if (href) {
          window.open(href, '_blank', 'noopener')
        }
      }
      return (
        <a href={href} onClick={handleClick} className="text-[#7c3aed] hover:underline cursor-pointer">
          {children}
        </a>
      )
    },
  }

  if (mode === 'terminal') {
    return {
      ...baseComponents,
      code: ({ children }) => <code className="font-mono">{children}</code>,
      pre: ({ children }) => <pre className="font-mono whitespace-pre-wrap my-2">{children}</pre>,
      p: ({ children }) => <p className="my-1">{children}</p>,
      ul: ({ children }) => <ul className="list-disc list-inside my-1">{children}</ul>,
      ol: ({ children }) => <ol className="list-decimal list-inside my-1">{children}</ol>,
      li: ({ children }) => <li className="my-0.5">{children}</li>,
    }
  }

  if (mode === 'minimal') {
    return {
      ...baseComponents,
      code: ({ className, children, ...props }) => {
        const match = /language-([\w-]+)/.exec(className || '')
        const isBlock = 'node' in props && props.node?.position?.start.line !== props.node?.position?.end.line
        if (match || isBlock) {
          const code = String(children).replace(/\n$/, '')
          if (match?.[1] === 'mermaid') {
            return <MermaidViewer code={code} />
          }
          return <CodeBlock code={code} language={match?.[1]} mode="full" className="my-2" />
        }
        return <InlineCode>{children}</InlineCode>
      },
      pre: ({ children }) => <>{children}</>,
      p: ({ children }) => <p className="my-2 leading-relaxed">{children}</p>,
      ul: ({ children }) => <ul className="my-2 space-y-1 pl-4 list-disc marker:text-gray-400">{children}</ul>,
      ol: ({ children }) => <ol className="my-2 space-y-1 pl-6 list-decimal">{children}</ol>,
      li: ({ children }) => <li>{children}</li>,
      table: ({ children }) => (
        <div className="my-3 overflow-x-auto">
          <table className="min-w-full text-sm">{children}</table>
        </div>
      ),
      thead: ({ children }) => <thead className="border-b border-gray-200">{children}</thead>,
      th: ({ children }) => <th className="text-left py-2 px-3 font-semibold text-gray-600">{children}</th>,
      td: ({ children }) => <td className="py-2 px-3 border-b border-gray-100">{children}</td>,
      h1: ({ children }) => <h1 className="text-base font-bold mt-5 mb-3 text-gray-900">{children}</h1>,
      h2: ({ children }) => <h2 className="text-base font-semibold mt-4 mb-3 text-gray-900">{children}</h2>,
      h3: ({ children }) => <h3 className="text-sm font-semibold mt-4 mb-2 text-gray-800">{children}</h3>,
      blockquote: ({ children }) => (
        <blockquote className="border-l-2 border-gray-300 pl-3 my-2 text-gray-500 italic">{children}</blockquote>
      ),
      hr: () => <hr className="my-4 border-gray-200" />,
      strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
      em: ({ children }) => <em className="italic">{children}</em>,
    }
  }

  // Full mode
  return {
    ...baseComponents,
    code: ({ className, children, ...props }) => {
      const match = /language-([\w-]+)/.exec(className || '')
      const isBlock = 'node' in props && props.node?.position?.start.line !== props.node?.position?.end.line
      if (match || isBlock) {
        const code = String(children).replace(/\n$/, '')
        if (match?.[1] === 'mermaid') {
          return <MermaidViewer code={code} />
        }
        return <CodeBlock code={code} language={match?.[1]} mode="full" className="my-2" />
      }
      return <InlineCode>{children}</InlineCode>
    },
    pre: ({ children }) => <>{children}</>,
    p: ({ children }) => <p className="my-3 leading-relaxed">{children}</p>,
    ul: ({ children }) => <ul className="my-3 space-y-1.5 pl-4 list-disc marker:text-gray-400">{children}</ul>,
    ol: ({ children }) => <ol className="my-3 space-y-1.5 pl-6 list-decimal">{children}</ol>,
    li: ({ children }) => <li className="leading-relaxed">{children}</li>,
    table: ({ children }) => (
      <div className="my-4 overflow-x-auto rounded-md border border-gray-200">
        <table className="min-w-full divide-y divide-gray-200">{children}</table>
      </div>
    ),
    thead: ({ children }) => <thead className="bg-gray-50">{children}</thead>,
    tbody: ({ children }) => <tbody className="divide-y divide-gray-100">{children}</tbody>,
    th: ({ children }) => <th className="text-left py-3 px-4 font-semibold text-sm text-gray-700">{children}</th>,
    td: ({ children }) => <td className="py-3 px-4 text-sm">{children}</td>,
    tr: ({ children }) => <tr className="hover:bg-gray-50 transition-colors">{children}</tr>,
    h1: ({ children }) => <h1 className="text-base font-bold mt-7 mb-4 text-gray-900">{children}</h1>,
    h2: ({ children }) => <h2 className="text-base font-semibold mt-6 mb-3 text-gray-900">{children}</h2>,
    h3: ({ children }) => <h3 className="text-sm font-semibold mt-5 mb-3 text-gray-800">{children}</h3>,
    h4: ({ children }) => <h4 className="text-sm font-semibold mt-3 mb-1 text-gray-800">{children}</h4>,
    blockquote: ({ children }) => (
      <blockquote className="border-l-4 border-gray-300 bg-gray-50 pl-4 pr-3 py-2 my-3 rounded-r-md">{children}</blockquote>
    ),
    input: ({ type, checked }) => {
      if (type === 'checkbox') {
        return <input type="checkbox" checked={checked} readOnly className="mr-2 rounded border-gray-300" />
      }
      return <input type={type} />
    },
    hr: () => <hr className="my-6 border-gray-200" />,
    strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
    em: ({ children }) => <em className="italic">{children}</em>,
    del: ({ children }) => <del className="line-through text-gray-400">{children}</del>,
  } as Partial<Components>
}

export function Markdown({
  children,
  mode = 'minimal',
  className,
  id,
  onUrlClick,
  collapsible = false,
}: MarkdownProps) {
  const collapsibleContext = useCollapsibleMarkdown()

  const components = React.useMemo(
    () => wrapWithSafeProxy(createComponents(mode, onUrlClick, collapsible ? collapsibleContext : null)),
    [mode, onUrlClick, collapsible, collapsibleContext]
  )

  const processedContent = React.useMemo(
    () => preprocessLinks(children),
    [children]
  )

  const remarkPlugins = React.useMemo(
    () => collapsible ? [remarkGfm, remarkCollapsibleSections] : [remarkGfm],
    [collapsible]
  )

  return (
    <div className={cn('markdown-content', className)}>
      <ReactMarkdown
        remarkPlugins={remarkPlugins}
        rehypePlugins={[rehypeRaw]}
        components={components}
      >
        {processedContent}
      </ReactMarkdown>
    </div>
  )
}

export const MemoizedMarkdown = React.memo(
  Markdown,
  (prevProps, nextProps) => {
    if (prevProps.id && nextProps.id) {
      return prevProps.id === nextProps.id && prevProps.children === nextProps.children && prevProps.mode === nextProps.mode
    }
    return prevProps.children === nextProps.children && prevProps.mode === nextProps.mode
  }
)
MemoizedMarkdown.displayName = 'MemoizedMarkdown'

export { CodeBlock, InlineCode } from './CodeBlock'
export { CollapsibleMarkdownProvider } from './CollapsibleMarkdownContext'
