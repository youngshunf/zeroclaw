/**
 * HxChatInput — 唤星聊天输入容器
 *
 * 布局：
 * ┌──────────────────────────────────────────────┐
 * │ [附件预览条]                                   │
 * │  多行输入区域 (textarea / auto-grow)          │
 * │  支持 @  和 / 触发弹窗                        │
 * ├──────────────────────────────────────────────┤
 * │ [📎 附件] [/ 命令] [@ 提及]     [🤖 状态] [➤] │
 * └──────────────────────────────────────────────┘
 *
 * 功能：
 * - Enter 发送，Shift+Enter 换行
 * - 键入 / 弹出斜杠命令菜单
 * - 键入 @ 弹出提及菜单
 * - 自动伸缩高度（min 44px, max 300px）
 * - 拖拽文件到输入框自动上传
 * - 点击附件按钮打开文件选择器
 * - 粘贴图片自动添加为附件
 */
import React, { useRef, useState, useCallback, useEffect } from 'react';
import { Paperclip, Send, Slash, AtSign, Square, X, Image, FileText } from 'lucide-react';
import { HxVoiceButton } from './HxVoiceButton';
import { HxPhotoProvider, PhotoView, localPathToSrc } from '@/huanxing/components/chat/HxImageLightbox';
import { HxSlashMenu, useHxSlashCommand, type SlashInputElement, type SlashCommandSection } from './HxSlashMenu';
import { HxMentionMenu, useHxMention, type MentionItem, type UseMentionInputElement, type MentionSection } from './HxMentionMenu';
import {
  handleFiles,
  openFileDialog,
  formatFileReferences,
  getWorkspaceDir,
  type UploadedFile,
} from '@/huanxing/lib/file-upload';

export interface HxChatInputProps {
  /** 发送消息回调 */
  onSend: (content: string) => void;
  /** 是否禁用输入 */
  disabled?: boolean;
  /** 是否正在生成中（显示停止按钮） */
  isGenerating?: boolean;
  /** 停止生成回调 */
  onStop?: () => void;
  /** 是否已连接 */
  connected?: boolean;
  /** Agent 名称（用于状态提示） */
  agentName?: string;
  /** 输入框占位文字 */
  placeholder?: string;
  /** 聚焦控制 */
  autoFocus?: boolean;
  /** @ 提及菜单分组（传入 HASN 联系人数据） */
  mentionSections?: MentionSection[];
  /** / 斜杠命令菜单分组（传入额外命令如技能列表） */
  slashSections?: SlashCommandSection[];
}

export function HxChatInput({
  onSend,
  disabled = false,
  isGenerating = false,
  onStop,
  connected = true,
  agentName = 'AI 助手',
  placeholder,
  autoFocus = true,
  mentionSections,
  slashSections,
}: HxChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDragOver, setIsDragOver] = useState(false);

  // ── File attachments ──────────────────────────────────────
  const [attachedFiles, setAttachedFiles] = useState<UploadedFile[]>([]);
  const [isUploading, setIsUploading] = useState(false);
  const workspaceDirRef = useRef<string | null>(null);

  // Load workspace dir once
  useEffect(() => {
    getWorkspaceDir().then(dir => { workspaceDirRef.current = dir; });
  }, []);

  const addFiles = useCallback(async (files: FileList | File[]) => {
    if (!files || (files instanceof FileList && files.length === 0)) return;
    setIsUploading(true);
    try {
      const uploaded = await handleFiles(files, workspaceDirRef.current ?? undefined);
      setAttachedFiles(prev => [...prev, ...uploaded]);
    } catch (err) {
      console.error('[HxChatInput] File upload error:', err);
    } finally {
      setIsUploading(false);
    }
  }, []);

  const removeFile = useCallback((index: number) => {
    setAttachedFiles(prev => prev.filter((_, i) => i !== index));
  }, []);

  // ── Auto-resize textarea ──────────────────────────────────
  const adjustHeight = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    ta.style.height = 'auto';
    const scrollH = ta.scrollHeight;
    ta.style.height = `${Math.min(scrollH, 300)}px`;
  }, []);

  useEffect(() => { adjustHeight(); }, [value, adjustHeight]);

  // Auto-focus
  useEffect(() => {
    if (autoFocus && textareaRef.current && !disabled) {
      textareaRef.current.focus();
    }
  }, [autoFocus, disabled]);

  // ── Slash command hook ────────────────────────────────────
  const slashInputAdapter = useRef<SlashInputElement | null>(null);
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    slashInputAdapter.current = {
      getBoundingClientRect: () => ta.getBoundingClientRect(),
      getCaretRect: () => {
        const rect = ta.getBoundingClientRect();
        const style = getComputedStyle(ta);
        const lineHeight = parseFloat(style.lineHeight) || 20;
        const paddingTop = parseFloat(style.paddingTop) || 0;
        const paddingLeft = parseFloat(style.paddingLeft) || 0;
        const text = ta.value.slice(0, ta.selectionStart);
        const lines = text.split('\n');
        const currentLine = lines.length - 1;
        return new DOMRect(
          rect.left + paddingLeft + Math.min(lines[currentLine].length * 8, rect.width - paddingLeft * 2),
          rect.top + paddingTop + currentLine * lineHeight,
          0,
          lineHeight,
        );
      },
    };
  });

  const slashInputRef = useRef<SlashInputElement | null>(null);
  slashInputRef.current = slashInputAdapter.current;
  const slash = useHxSlashCommand({ inputRef: slashInputRef, sections: slashSections });

  // ── Mention hook ──────────────────────────────────────────
  const mentionInputAdapter = useRef<UseMentionInputElement | null>(null);
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    mentionInputAdapter.current = {
      getBoundingClientRect: () => ta.getBoundingClientRect(),
      getCaretRect: () => {
        const rect = ta.getBoundingClientRect();
        const style = getComputedStyle(ta);
        const lineHeight = parseFloat(style.lineHeight) || 20;
        const paddingTop = parseFloat(style.paddingTop) || 0;
        const paddingLeft = parseFloat(style.paddingLeft) || 0;
        const text = ta.value.slice(0, ta.selectionStart);
        const lines = text.split('\n');
        const currentLine = lines.length - 1;
        return new DOMRect(
          rect.left + paddingLeft + Math.min(lines[currentLine].length * 8, rect.width - paddingLeft * 2),
          rect.top + paddingTop + currentLine * lineHeight,
          0,
          lineHeight,
        );
      },
    };
  });

  const mentionInputRef = useRef<UseMentionInputElement | null>(null);
  mentionInputRef.current = mentionInputAdapter.current;
  const mention = useHxMention({ inputRef: mentionInputRef, sections: mentionSections });

  // ── Input change handler ──────────────────────────────────
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      const cursorPos = e.target.selectionStart;
      setValue(newValue);
      slash.handleInputChange(newValue, cursorPos);
      mention.handleInputChange(newValue, cursorPos);
    },
    [slash, mention],
  );

  // ── Send ──────────────────────────────────────────────────
  const handleSend = useCallback(() => {
    const trimmedText = value.trim();
    if ((!trimmedText && attachedFiles.length === 0) || disabled) return;

    // Build final message: file references + user text
    let finalContent = '';
    if (attachedFiles.length > 0) {
      finalContent += formatFileReferences(attachedFiles);
    }
    if (trimmedText) {
      finalContent += trimmedText;
    }

    onSend(finalContent.trim());
    setValue('');
    setAttachedFiles([]);
    slash.close();
    mention.close();
    requestAnimationFrame(() => {
      const ta = textareaRef.current;
      if (ta) {
        ta.style.height = 'auto';
        ta.focus();
      }
    });
  }, [value, attachedFiles, disabled, onSend, slash, mention]);

  // ── Key handler ───────────────────────────────────────────
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (slash.isOpen || mention.isOpen) return;
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [slash.isOpen, mention.isOpen, handleSend],
  );

  // ── Paste handler (image paste) ───────────────────────────
  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const items = e.clipboardData?.items;
      if (!items) return;

      const imageFiles: File[] = [];
      for (let i = 0; i < items.length; i++) {
        if (items[i].type.startsWith('image/')) {
          const file = items[i].getAsFile();
          if (file) imageFiles.push(file);
        }
      }

      if (imageFiles.length > 0) {
        e.preventDefault();
        addFiles(imageFiles);
      }
    },
    [addFiles],
  );

  // ── Slash command selection ───────────────────────────────
  const handleSlashSelect = useCallback(
    (commandId: string, hasArgs: boolean) => {
      const { value: newValue, cursorPosition } = slash.handleSelectCommand(commandId, hasArgs);
      setValue(newValue);
      if (!hasArgs && newValue.trim()) {
        requestAnimationFrame(() => {
          onSend(newValue.trim());
          setValue('');
          const ta = textareaRef.current;
          if (ta) { ta.style.height = 'auto'; ta.focus(); }
        });
      } else {
        requestAnimationFrame(() => {
          const ta = textareaRef.current;
          if (ta) { ta.focus(); ta.setSelectionRange(cursorPosition, cursorPosition); }
        });
      }
    },
    [slash, onSend],
  );

  // ── Mention selection ─────────────────────────────────────
  const handleMentionSelect = useCallback(
    (item: MentionItem) => {
      const { value: newValue, cursorPosition } = mention.handleSelectMention(item);
      setValue(newValue);
      requestAnimationFrame(() => {
        const ta = textareaRef.current;
        if (ta) { ta.focus(); ta.setSelectionRange(cursorPosition, cursorPosition); }
      });
    },
    [mention],
  );

  // ── Toolbar button handlers ───────────────────────────────
  const handleSlashButton = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const start = ta.selectionStart;
    const before = value.slice(0, start);
    const after = value.slice(start);
    const needsSpace = before.length > 0 && !before.endsWith(' ') && !before.endsWith('\n');
    const inserted = (needsSpace ? ' ' : '') + '/';
    const newValue = before + inserted + after;
    const newCursor = start + inserted.length;
    setValue(newValue);
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(newCursor, newCursor);
      slash.handleInputChange(newValue, newCursor);
    });
  }, [value, slash]);

  const handleAtButton = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    const start = ta.selectionStart;
    const before = value.slice(0, start);
    const after = value.slice(start);
    const needsSpace = before.length > 0 && !before.endsWith(' ') && !before.endsWith('\n');
    const inserted = (needsSpace ? ' ' : '') + '@';
    const newValue = before + inserted + after;
    const newCursor = start + inserted.length;
    setValue(newValue);
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(newCursor, newCursor);
      mention.handleInputChange(newValue, newCursor);
    });
  }, [value, mention]);

  // ── File attach button ────────────────────────────────────
  const handleAttachClick = useCallback(async () => {
    const files = await openFileDialog({ multiple: true });
    if (files.length > 0) {
      await addFiles(files);
    }
  }, [addFiles]);

  // ── Drag & Drop ───────────────────────────────────────────
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      await addFiles(e.dataTransfer.files);
    }
  }, [addFiles]);

  // ── Derived state ─────────────────────────────────────────
  const canSend = (value.trim().length > 0 || attachedFiles.length > 0) && !disabled;

  // Voice transcription callback — fill input box with transcribed text
  const handleVoiceTranscribed = useCallback((text: string) => {
    setValue(prev => {
      const combined = prev.trim() ? `${prev.trim()} ${text}` : text;
      return combined;
    });
    // Focus textarea after transcription
    requestAnimationFrame(() => {
      textareaRef.current?.focus();
    });
  }, []);
  const effectivePlaceholder = placeholder ?? (
    !connected ? '连接中...' :
    disabled ? '请先选择或创建一个对话' :
    '输入消息... (Enter 发送，Shift+Enter 换行)'
  );

  return (
    <div className="hx-chat-input-area">
      {/* Floating menus */}
      <HxSlashMenu
        open={slash.isOpen}
        onOpenChange={(open) => { if (!open) slash.close(); }}
        filter={slash.filter}
        position={slash.position}
        onSelectCommand={handleSlashSelect}
      />
      <HxMentionMenu
        open={mention.isOpen}
        onOpenChange={(open) => { if (!open) mention.close(); }}
        filter={mention.filter}
        position={mention.position}
        onSelectMention={handleMentionSelect}
      />

      {/* Input container */}
      <div
        ref={containerRef}
        className={`hx-chat-input-container ${isDragOver ? 'drag-over' : ''}`}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        {attachedFiles.length > 0 && (
          <HxPhotoProvider>
            <div className="hx-attached-files">
              {attachedFiles.map((file, index) => (
                <div key={`${file.originalName}-${index}`} className={`hx-attached-file-chip${file.isImage ? ' is-image' : ''}`}>
                  {file.isImage ? (
                    <PhotoView src={file.blobUrl || localPathToSrc(file.absolutePath)}>
                      <img
                        src={file.blobUrl || localPathToSrc(file.absolutePath)}
                        alt={file.originalName}
                        className="hx-attached-file-thumb"
                      />
                    </PhotoView>
                  ) : (
                    <>
                      <span className="hx-attached-file-icon">
                        <FileText size={14} />
                      </span>
                      <span className="hx-attached-file-name" title={file.absolutePath}>
                        {file.originalName}
                      </span>
                      <span className="hx-attached-file-size">
                        {formatSize(file.size)}
                      </span>
                    </>
                  )}
                  <button
                    type="button"
                    className="hx-attached-file-remove"
                    onClick={() => removeFile(index)}
                    title="移除"
                  >
                    <X size={12} />
                  </button>
                </div>
              ))}
            </div>
          </HxPhotoProvider>
        )}

        {/* Drag overlay */}
        {isDragOver && (
          <div className="hx-drop-overlay">
            <Paperclip size={24} />
            <span>拖放文件到此处</span>
          </div>
        )}

        {/* Textarea */}
        <div className="hx-chat-input-editor">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            placeholder={effectivePlaceholder}
            disabled={disabled}
            rows={1}
            className="hx-chat-input-textarea"
          />
        </div>

        {/* Bottom toolbar */}
        <div className="hx-chat-input-toolbar">
          <div className="hx-chat-input-toolbar-left">
            <button
              type="button"
              onClick={handleAttachClick}
              className="hx-input-tool-btn"
              title="添加附件"
              disabled={disabled || isUploading}
            >
              <Paperclip size={16} />
            </button>
            <button
              type="button"
              onClick={handleSlashButton}
              className="hx-input-tool-btn"
              title="斜杠命令"
              disabled={disabled}
            >
              <Slash size={16} />
            </button>
            <button
              type="button"
              onClick={handleAtButton}
              className="hx-input-tool-btn"
              title="@ 提及"
              disabled={disabled}
            >
              <AtSign size={16} />
            </button>

            {/* Voice input button */}
            <HxVoiceButton
              onTranscribed={handleVoiceTranscribed}
              disabled={disabled || isGenerating}
            />

            {isUploading && (
              <span className="hx-upload-indicator">上传中...</span>
            )}
          </div>

          <div className="hx-chat-input-toolbar-right">
            {/* Status indicator */}
            <div className="hx-input-status">
              <span
                className={`hx-input-status-dot ${connected ? 'bg-[var(--hx-green)]' : 'bg-[var(--hx-amber)]'}`}
              />
              <span className="hx-input-status-text">
                {connected ? agentName : '连接中...'}
              </span>
            </div>

            {/* Send / Stop button */}
            {isGenerating ? (
              <button
                type="button"
                onClick={onStop}
                className="hx-input-stop-btn"
                title="停止生成"
              >
                <Square size={16} />
              </button>
            ) : (
              <button
                type="button"
                onClick={handleSend}
                disabled={!canSend}
                className="hx-input-send-btn"
                title="发送"
              >
                <Send size={16} />
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Hint */}
      <div className="hx-input-hint-v2">
        <kbd>/</kbd> 命令 · <kbd>@</kbd> 提及 · <kbd>Shift+Enter</kbd> 换行
      </div>

    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────────
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
