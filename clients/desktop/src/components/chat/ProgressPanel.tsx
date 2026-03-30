import { cn } from '../../lib/utils';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { useState } from 'react';

interface ProgressPanelProps {
  lines: string[];
  className?: string;
}

/**
 * Displays real-time progress from the agent loop:
 * - 🤔 Thinking...
 * - 🔧 shell ⏳ ls -la
 * - 🔧 shell ✅ (2s)
 * - 💬 Got 2 tool call(s) (3s)
 */
export function ProgressPanel({ lines, className }: ProgressPanelProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (lines.length === 0) return null;

  return (
    <div className={cn('hx-progress-panel', className)}>
      <button
        className="hx-progress-header"
        onClick={() => setCollapsed(!collapsed)}
      >
        {collapsed ? (
          <ChevronRight className="h-3.5 w-3.5 text-gray-400" />
        ) : (
          <ChevronDown className="h-3.5 w-3.5 text-gray-400" />
        )}
        <span className="hx-progress-label">
          {collapsed ? `处理中 (${lines.length} 步)` : '处理中...'}
        </span>
        <div className="hx-progress-dots">
          <span /><span /><span />
        </div>
      </button>
      {!collapsed && (
        <div className="hx-progress-lines">
          {lines.map((line, i) => (
            <div key={i} className="hx-progress-line">{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
