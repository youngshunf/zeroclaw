import { cn } from '../../lib/utils';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '../ui/Collapsible';

interface ProgressPanelProps {
  lines: string[];
  className?: string;
  isFinished?: boolean;
}

/**
 * Displays real-time progress from the agent loop:
 * - 🤔 Thinking...
 * - 🔧 shell ⏳ ls -la
 * - 🔧 shell ✅ (2s)
 * - 💬 Got 2 tool call(s) (3s)
 */
export function ProgressPanel({ lines, className, isFinished = false }: ProgressPanelProps) {
  const [open, setOpen] = useState(!isFinished);

  if (lines.length === 0) return null;

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className={cn('hx-progress-panel', className)}
    >
      <CollapsibleTrigger asChild>
        <button className="hx-progress-header">
          {open ? (
            <ChevronDown className="h-3.5 w-3.5 text-gray-400" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-gray-400" />
          )}
          <span className="hx-progress-label">
            {isFinished ? `思考与执行过程 (${lines.length} 步)` : '处理中...'}
          </span>
          {!isFinished && (
            <div className="hx-progress-dots">
              <span /><span /><span />
            </div>
          )}
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="hx-progress-lines">
          {lines.map((line, i) => (
            <div key={i} className="hx-progress-line">{line}</div>
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
