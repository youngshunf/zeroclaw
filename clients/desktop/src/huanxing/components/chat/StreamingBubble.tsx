import { ProgressPanel } from './ProgressPanel';
import { Markdown } from '../markdown';
import { Bot } from 'lucide-react';

interface StreamingBubbleProps {
  /** Accumulated text content being streamed */
  content: string;
  /** Progress lines from the agent loop */
  progressLines: string[];
  /** Whether currently streaming */
  isStreaming: boolean;
  /** Agent name for avatar */
  agentName?: string;
}

/**
 * A chat bubble that displays streaming content + progress information.
 * Shows during the agent loop execution before the final `done` message.
 */
export function StreamingBubble({
  content,
  progressLines,
  isStreaming,
  agentName,
}: StreamingBubbleProps) {
  const hasContent = content.length > 0;
  const hasProgress = progressLines.length > 0;

  if (!hasContent && !hasProgress && !isStreaming) return null;

  return (
    <div className="hx-msg agent">
      <div className="hx-msg-avatar">
        <Bot size={18} />
      </div>
      <div className="hx-msg-content">
        {agentName && (
          <span className="hx-msg-sender">{agentName}</span>
        )}
        <div className="hx-msg-bubble">
          {hasProgress && (
            <ProgressPanel lines={progressLines} />
          )}
          {hasContent && (
            <Markdown mode="minimal">{content}</Markdown>
          )}
          {isStreaming && !hasContent && !hasProgress && (
            <div className="hx-typing">
              <span /><span /><span />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
