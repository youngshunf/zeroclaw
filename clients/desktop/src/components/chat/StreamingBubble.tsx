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
  /** Agent icon URL */
  agentIconUrl?: string;
  /** Link click handler for Markdown */
  onUrlClick?: (url: string) => void;
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
  agentIconUrl,
  onUrlClick,
}: StreamingBubbleProps) {
  const hasContent = content.length > 0;
  const hasProgress = progressLines.length > 0;

  if (!hasContent && !hasProgress && !isStreaming) return null;

  return (
    <div className="hx-msg agent">
      <div className="hx-msg-avatar" style={{ overflow: 'hidden', padding: agentIconUrl ? 0 : undefined }}>
        {agentIconUrl ? (
          <img src={agentIconUrl} alt={agentName ?? 'agent'} style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: 'inherit' }} />
        ) : (
          <Bot size={16} />
        )}
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
            <Markdown mode="minimal" onUrlClick={onUrlClick}>{content}</Markdown>
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
