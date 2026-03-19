import * as React from 'react'
import { ChevronRight } from 'lucide-react'
import { cn } from '../../lib/utils'

interface CollapsibleSectionProps {
  sectionId: string
  headingLevel: number
  isCollapsed: boolean
  onToggle: (sectionId: string) => void
  children: React.ReactNode
}

export function CollapsibleSection({
  sectionId,
  headingLevel,
  isCollapsed,
  onToggle,
  children,
}: CollapsibleSectionProps) {
  const childArray = React.Children.toArray(children)
  const heading = childArray[0]
  const content = childArray.slice(1)

  if (headingLevel > 4) {
    return <>{children}</>
  }

  const isExpanded = !isCollapsed
  const hasContent = content.length > 0

  return (
    <div className="markdown-collapsible-section" data-section-id={sectionId}>
      <div
        className={cn('relative group', hasContent && 'cursor-pointer')}
        onClick={() => hasContent && onToggle(sectionId)}
      >
        <div
          className={cn(
            'absolute -left-4 top-[5px] select-none transition-all duration-200',
            !hasContent && 'opacity-0',
            hasContent && isCollapsed && 'opacity-100',
            hasContent && isExpanded && 'opacity-0 group-hover:opacity-100'
          )}
          style={{ transform: isExpanded ? 'rotate(90deg)' : 'rotate(0deg)' }}
        >
          <ChevronRight className="h-3 w-3 text-gray-400" />
        </div>
        {heading}
      </div>
      {hasContent && isExpanded && (
        <div className="collapsible-section-content">
          {content}
        </div>
      )}
    </div>
  )
}
