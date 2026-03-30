/**
 * HxSlashMenu — 斜杠命令菜单（行内浮窗）
 *
 * 用户在输入框中键入 `/` 时弹出，根据光标位置定位。
 * 支持键盘导航（↑↓回车）和鼠标选择。
 *
 * 命令列表映射自 ZeroClaw 后端实际支持的指令。
 */
import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Terminal, HelpCircle, Trash2, RotateCcw, StopCircle, Cpu, Settings, Brain, Sparkles } from 'lucide-react';

// ── 类型 ───────────────────────────────────────────────────────
export interface SlashCommandItem {
  id: string;
  label: string;
  description: string;
  hasArgs: boolean;
  icon?: React.ReactNode;
}

export interface SlashCommandSection {
  id: string;
  label: string;
  items: SlashCommandItem[];
}

// ── 默认命令列表（映射 ZeroClaw 后端命令） ────────────────────
const ICON_CLASS = 'h-3.5 w-3.5';

export const HUANXING_SLASH_COMMANDS: SlashCommandItem[] = [
  { id: 'help',   label: '帮助',       description: '显示可用命令列表',                   hasArgs: false, icon: <HelpCircle className={ICON_CLASS} /> },
  { id: 'new',    label: '新对话',     description: '清除当前对话，开始新会话',             hasArgs: false, icon: <RotateCcw className={ICON_CLASS} /> },
  { id: 'clear',  label: '清除对话',   description: '清除对话历史和会话记忆',               hasArgs: false, icon: <Trash2 className={ICON_CLASS} /> },
  { id: 'stop',   label: '停止',       description: '中断当前正在执行的任务',               hasArgs: false, icon: <StopCircle className={ICON_CLASS} /> },
  { id: 'model',  label: '切换模型',   description: '查看或切换当前使用的 AI 模型',         hasArgs: true,  icon: <Cpu className={ICON_CLASS} /> },
  { id: 'config', label: '查看配置',   description: '显示当前 Agent 的运行配置',           hasArgs: false, icon: <Settings className={ICON_CLASS} /> },
  { id: 'think',  label: '思考深度',   description: '设置推理深度 (off|low|medium|high|max)', hasArgs: true, icon: <Brain className={ICON_CLASS} /> },
  { id: 'skill',  label: '使用技能',   description: '指定 Agent 使用特定技能来处理任务',     hasArgs: true,  icon: <Sparkles className={ICON_CLASS} /> },
];

export const HUANXING_SLASH_SECTIONS: SlashCommandSection[] = [
  { id: 'session', label: '会话', items: HUANXING_SLASH_COMMANDS.filter(c => ['new', 'clear', 'stop'].includes(c.id)) },
  { id: 'config',  label: '配置', items: HUANXING_SLASH_COMMANDS.filter(c => ['model', 'config', 'think'].includes(c.id)) },
  { id: 'other',   label: '其他', items: HUANXING_SLASH_COMMANDS.filter(c => ['help', 'skill'].includes(c.id)) },
];

// ── 过滤 ───────────────────────────────────────────────────────
function filterSections(sections: SlashCommandSection[], filter: string): SlashCommandSection[] {
  if (!filter) return sections;
  const q = filter.toLowerCase();
  return sections
    .map(s => ({
      ...s,
      items: s.items.filter(
        item =>
          item.label.toLowerCase().includes(q) ||
          item.id.toLowerCase().includes(q) ||
          item.description.toLowerCase().includes(q),
      ),
    }))
    .filter(s => s.items.length > 0);
}

function flattenSections(sections: SlashCommandSection[]): SlashCommandItem[] {
  return sections.flatMap(s => s.items);
}

// ── 菜单组件 ──────────────────────────────────────────────────
export interface HxSlashMenuProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sections?: SlashCommandSection[];
  filter?: string;
  position: { x: number; y: number };
  onSelectCommand: (commandId: string, hasArgs: boolean) => void;
}

export function HxSlashMenu({
  open,
  onOpenChange,
  sections = HUANXING_SLASH_SECTIONS,
  filter = '',
  position,
  onSelectCommand,
}: HxSlashMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filteredSections = useMemo(() => filterSections(sections, filter), [sections, filter]);
  const flatItems = useMemo(() => flattenSections(filteredSections), [filteredSections]);

  // Reset selection on filter change
  useEffect(() => { setSelectedIndex(0); }, [filter]);

  // Scroll selected into view
  useEffect(() => {
    if (!listRef.current) return;
    const el = listRef.current.querySelector('[data-selected="true"]');
    if (el) el.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  // Handle selection
  const handleSelect = useCallback(
    (item: SlashCommandItem) => {
      onSelectCommand(item.id, item.hasArgs);
      onOpenChange(false);
    },
    [onSelectCommand, onOpenChange],
  );

  // Keyboard navigation
  useEffect(() => {
    if (!open || flatItems.length === 0) return;
    const handler = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          e.stopPropagation();
          setSelectedIndex(prev => (prev < flatItems.length - 1 ? prev + 1 : 0));
          break;
        case 'ArrowUp':
          e.preventDefault();
          e.stopPropagation();
          setSelectedIndex(prev => (prev > 0 ? prev - 1 : flatItems.length - 1));
          break;
        case 'Enter':
        case 'Tab':
          e.preventDefault();
          e.stopPropagation();
          if (flatItems[selectedIndex]) handleSelect(flatItems[selectedIndex]);
          break;
        case 'Escape':
          e.preventDefault();
          onOpenChange(false);
          break;
      }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, [open, flatItems, selectedIndex, handleSelect, onOpenChange]);

  // Click outside
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onOpenChange(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open, onOpenChange]);

  if (!open || flatItems.length === 0) return null;

  const bottomPos = typeof window !== 'undefined' ? window.innerHeight - Math.round(position.y) + 8 : 0;
  let currentIdx = 0;

  return (
    <div
      ref={menuRef}
      className="hx-slash-menu"
      style={{ left: Math.round(position.x) - 10, bottom: bottomPos }}
    >
      <div className="hx-slash-menu-header">斜杠命令</div>
      <div ref={listRef} className="hx-slash-menu-list">
        {filteredSections.map(section => (
          <React.Fragment key={section.id}>
            {filteredSections.length > 1 && (
              <div className="hx-slash-menu-section-label">{section.label}</div>
            )}
            {section.items.map(item => {
              const idx = currentIdx++;
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={item.id}
                  data-selected={isSelected}
                  onClick={() => handleSelect(item)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  className={`hx-slash-menu-item ${isSelected ? 'selected' : ''}`}
                >
                  <div className="hx-slash-menu-item-icon">
                    {item.icon || <Terminal className={ICON_CLASS} />}
                  </div>
                  <div className="hx-slash-menu-item-content">
                    <span className="hx-slash-menu-item-label">{item.label}</span>
                    {item.description && (
                      <span className="hx-slash-menu-item-desc">{item.description}</span>
                    )}
                  </div>
                  <span className="hx-slash-menu-item-id">/{item.id}</span>
                </div>
              );
            })}
          </React.Fragment>
        ))}
      </div>
    </div>
  );
}

// ── Hook: 行内斜杠命令管理 ────────────────────────────────────
export interface SlashInputElement {
  getBoundingClientRect: () => DOMRect;
  getCaretRect?: () => DOMRect | null;
}

export interface UseHxSlashCommandOptions {
  inputRef: React.RefObject<SlashInputElement | null>;
  sections?: SlashCommandSection[];
}

export interface UseHxSlashCommandReturn {
  isOpen: boolean;
  filter: string;
  position: { x: number; y: number };
  sections: SlashCommandSection[];
  handleInputChange: (value: string, cursorPosition: number) => void;
  close: () => void;
  handleSelectCommand: (commandId: string, hasArgs: boolean) => { value: string; cursorPosition: number };
}

export function useHxSlashCommand({
  inputRef,
  sections = HUANXING_SLASH_SECTIONS,
}: UseHxSlashCommandOptions): UseHxSlashCommandReturn {
  const [isOpen, setIsOpen] = useState(false);
  const [filter, setFilter] = useState('');
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [slashStart, setSlashStart] = useState(-1);
  const currentInputRef = useRef({ value: '', cursorPosition: 0 });

  const handleInputChange = useCallback(
    (value: string, cursorPosition: number) => {
      currentInputRef.current = { value, cursorPosition };
      const textBefore = value.slice(0, cursorPosition);
      const slashMatch = textBefore.match(/(?:^|\s)\/(\w[\w\-:]*)?$/);

      const hasItems = sections.some(s => s.items.length > 0);

      if (slashMatch && hasItems) {
        const filterText = slashMatch[1] || '';
        const filtered = filterSections(sections, filterText);
        const hasFiltered = filtered.some(s => s.items.length > 0);

        if (!hasFiltered) {
          setIsOpen(false);
          setFilter('');
          setSlashStart(-1);
          return;
        }

        const matchStart = textBefore.lastIndexOf('/');
        setSlashStart(matchStart);
        setFilter(filterText);

        if (inputRef.current) {
          const caretRect = inputRef.current.getCaretRect?.();
          if (caretRect && caretRect.x > 0) {
            setPosition({ x: caretRect.x, y: caretRect.y });
          } else {
            const rect = inputRef.current.getBoundingClientRect();
            const lineHeight = 20;
            const linesBeforeCursor = textBefore.split('\n').length - 1;
            setPosition({
              x: rect.left,
              y: rect.top + (linesBeforeCursor + 1) * lineHeight,
            });
          }
        }

        setIsOpen(true);
      } else {
        setIsOpen(false);
        setFilter('');
        setSlashStart(-1);
      }
    },
    [inputRef, sections],
  );

  const handleSelectCommand = useCallback(
    (commandId: string, hasArgs: boolean): { value: string; cursorPosition: number } => {
      let result = '';
      let newCursor = 0;

      if (slashStart >= 0) {
        const { value: currentValue, cursorPosition } = currentInputRef.current;
        const before = currentValue.slice(0, slashStart);
        const after = currentValue.slice(cursorPosition);

        if (hasArgs) {
          const cmdText = `/${commandId} `;
          result = before + cmdText + after;
          newCursor = before.length + cmdText.length;
        } else {
          result = `/${commandId}`;
          newCursor = result.length;
        }
      }

      setIsOpen(false);
      return { value: result, cursorPosition: newCursor };
    },
    [slashStart],
  );

  const close = useCallback(() => {
    setIsOpen(false);
    setFilter('');
    setSlashStart(-1);
  }, []);

  return { isOpen, filter, position, sections, handleInputChange, close, handleSelectCommand };
}
