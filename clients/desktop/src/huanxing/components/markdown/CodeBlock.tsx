import * as React from 'react'
import { codeToHtml, bundledLanguages, type BundledLanguage } from 'shiki'
import { cn } from '../../lib/utils'

export interface CodeBlockProps {
  code: string
  language?: string
  className?: string
  mode?: 'terminal' | 'minimal' | 'full'
}

const LANGUAGE_ALIASES: Record<string, BundledLanguage> = {
  'js': 'javascript',
  'ts': 'typescript',
  'py': 'python',
  'sh': 'bash',
  'zsh': 'bash',
  'yml': 'yaml',
  'rb': 'ruby',
  'rs': 'rust',
  'kt': 'kotlin',
}

const highlightCache = new Map<string, string>()
const CACHE_MAX_SIZE = 200

function isValidLanguage(lang: string): lang is BundledLanguage {
  const normalized = LANGUAGE_ALIASES[lang] || lang
  return normalized in bundledLanguages
}

export function CodeBlock({ code, language = 'text', className, mode = 'full' }: CodeBlockProps) {
  const [highlighted, setHighlighted] = React.useState<string | null>(null)
  const [isLoading, setIsLoading] = React.useState(true)
  const [copied, setCopied] = React.useState(false)

  const langLower = language.toLowerCase()
  const resolvedLang: string = LANGUAGE_ALIASES[langLower] || langLower

  React.useEffect(() => {
    let cancelled = false

    async function highlight() {
      // Detect theme from documentElement[data-theme]
      const root = document.documentElement
      const isDark = root?.getAttribute('data-theme') === 'dark'
      const theme = isDark ? 'github-dark' : 'github-light'
      const cacheKey = `${theme}:${resolvedLang}:${code}`

      const cached = highlightCache.get(cacheKey)
      if (cached) {
        if (!cancelled) { setHighlighted(cached); setIsLoading(false) }
        return
      }

      try {
        const lang = isValidLanguage(resolvedLang) ? resolvedLang : 'text'
        const html = await codeToHtml(code, { lang, theme })

        if (highlightCache.size >= CACHE_MAX_SIZE) {
          const firstKey = highlightCache.keys().next().value
          if (firstKey) highlightCache.delete(firstKey)
        }
        highlightCache.set(cacheKey, html)

        if (!cancelled) { setHighlighted(html); setIsLoading(false) }
      } catch (error) {
        console.warn(`Shiki highlighting failed for "${resolvedLang}":`, error)
        if (!cancelled) { setHighlighted(null); setIsLoading(false) }
      }
    }

    highlight()
    return () => { cancelled = true }
  }, [code, resolvedLang])

  const handleCopy = React.useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy:', err)
    }
  }, [code])

  if (mode === 'terminal') {
    return (
      <pre className={cn('font-mono text-sm whitespace-pre-wrap', className)}>
        <code>{code}</code>
      </pre>
    )
  }

  if (mode === 'minimal') {
    if (isLoading || !highlighted) {
      return (
        <pre className={cn('font-mono text-sm whitespace-pre-wrap', className)}>
          <code>{code}</code>
        </pre>
      )
    }
    return (
      <div
        className={cn('font-mono text-sm [&_pre]:!bg-transparent [&_pre]:!p-0 [&_pre]:whitespace-pre-wrap [&_pre]:break-all [&_code]:!bg-transparent', className)}
        dangerouslySetInnerHTML={{ __html: highlighted }}
      />
    )
  }

  // Full mode
  return (
    <div className={cn('relative group rounded-lg overflow-hidden border border-gray-200 bg-gray-50', className)}>
      <div className="flex items-center justify-between px-3 py-1.5 bg-gray-100 border-b border-gray-200 text-xs">
        <span className="text-gray-500 font-medium uppercase tracking-wide">
          {resolvedLang !== 'text' ? resolvedLang : 'plain text'}
        </span>
        <button
          onClick={handleCopy}
          className="opacity-0 group-hover:opacity-100 transition-opacity text-gray-400 hover:text-gray-700"
          aria-label="复制代码"
        >
          {copied ? (
            <svg className="w-4 h-4 text-green-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          ) : (
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
          )}
        </button>
      </div>
      <div className="p-3 overflow-x-auto">
        {isLoading || !highlighted ? (
          <pre className="font-mono text-sm whitespace-pre-wrap break-all">
            <code>{code}</code>
          </pre>
        ) : (
          <div
            className="font-mono text-sm [&_pre]:!bg-transparent [&_pre]:!m-0 [&_pre]:!p-0 [&_pre]:whitespace-pre-wrap [&_pre]:break-all [&_code]:!bg-transparent"
            dangerouslySetInnerHTML={{ __html: highlighted }}
          />
        )}
      </div>
    </div>
  )
}

export function InlineCode({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <code className={cn(
      'px-1.5 py-0.5 rounded bg-gray-100 border border-gray-200 font-mono text-sm text-gray-700',
      className
    )}>
      {children}
    </code>
  )
}
