/**
 * HxMentionMenu — @提及菜单（行内浮窗）
 *
 * 用户在输入框中键入 `@` 时弹出，支持选择：
 * - HASN 联系人（当前使用 mock 数据）
 * - 技能（从 Agent 配置中获取）
 *
 * 选中后在输入框中插入 @标记。
 */
import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { User, Sparkles, Bot } from 'lucide-react';

// ── 类型 ───────────────────────────────────────────────────────
export type MentionType = 'contact' | 'skill' | 'agent';

export interface MentionItem {
  type: MentionType;
  id: string;
  label: string;
  avatar?: string;
  description?: string;
}

export interface MentionSection {
  id: string;
  label: string;
  items: MentionItem[];
}

// ── Mock 数据已移除 — 使用 useHasnContacts + useAgentSkills 接入真实数据 ──
// 消费者通过 props 或 hook 参数传入 sections。

/** 空占位分组（当没有数据时显示提示） */
export const EMPTY_MENTION_SECTIONS: MentionSection[] = [
  {
    id: 'empty',
    label: '提及',
    items: [
      { type: 'contact', id: '_hint', label: '暂无可提及的对象', description: '连接 HASN 后将显示联系人和 Agent' },
    ],
  },
];

// ── 过滤 ───────────────────────────────────────────────────────
function filterSections(sections: MentionSection[], filter: string): MentionSection[] {
  if (!filter) return sections;
  const q = filter.toLowerCase();
  return sections
    .map(s => ({
      ...s,
      items: s.items.filter(
        item =>
          item.label.toLowerCase().includes(q) ||
          item.id.toLowerCase().includes(q) ||
          (item.description?.toLowerCase().includes(q) ?? false),
      ),
    }))
    .filter(s => s.items.length > 0);
}

function flattenSections(sections: MentionSection[]): MentionItem[] {
  return sections.flatMap(s => s.items);
}

// ── 类型图标 ──────────────────────────────────────────────────
function MentionIcon({ type }: { type: MentionType }) {
  const cls = 'h-3.5 w-3.5';
  switch (type) {
    case 'contact': return <User className={cls} />;
    case 'skill': return <Sparkles className={cls} />;
    case 'agent': return <Bot className={cls} />;
  }
}

// ── 菜单组件 ──────────────────────────────────────────────────
export interface HxMentionMenuProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sections?: MentionSection[];
  filter?: string;
  position: { x: number; y: number };
  onSelectMention: (item: MentionItem) => void;
}

export function HxMentionMenu({
  open,
  onOpenChange,
  sections: customSections,
  filter = '',
  position,
  onSelectMention,
}: HxMentionMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const defaultSections: MentionSection[] = EMPTY_MENTION_SECTIONS;

  const sections = customSections ?? defaultSections;
  const filteredSections = useMemo(() => filterSections(sections, filter), [sections, filter]);
  const flatItems = useMemo(() => flattenSections(filteredSections), [filteredSections]);

  useEffect(() => { setSelectedIndex(0); }, [filter]);

  useEffect(() => {
    if (!listRef.current) return;
    const el = listRef.current.querySelector('[data-selected="true"]');
    if (el) el.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  const handleSelect = useCallback(
    (item: MentionItem) => {
      onSelectMention(item);
      onOpenChange(false);
    },
    [onSelectMention, onOpenChange],
  );

  // Keyboard nav
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
      className="hx-mention-menu"
      style={{ left: Math.round(position.x) - 10, bottom: bottomPos }}
    >
      <div className="hx-mention-menu-header">提及</div>
      <div ref={listRef} className="hx-mention-menu-list">
        {filteredSections.map(section => (
          <React.Fragment key={section.id}>
            {filteredSections.length > 1 && (
              <div className="hx-mention-menu-section-label">{section.label}</div>
            )}
            {section.items.map(item => {
              const idx = currentIdx++;
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={`${item.type}-${item.id}`}
                  data-selected={isSelected}
                  onClick={() => handleSelect(item)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  className={`hx-mention-menu-item ${isSelected ? 'selected' : ''}`}
                >
                  <div className="hx-mention-menu-item-icon">
                    <MentionIcon type={item.type} />
                  </div>
                  <div className="hx-mention-menu-item-content">
                    <span className="hx-mention-menu-item-label">{item.label}</span>
                    {item.description && (
                      <span className="hx-mention-menu-item-desc">{item.description}</span>
                    )}
                  </div>
                  <span className="hx-mention-menu-item-type">
                    {item.type === 'contact' ? '联系人' : item.type === 'skill' ? '技能' : 'Agent'}
                  </span>
                </div>
              );
            })}
          </React.Fragment>
        ))}
      </div>
    </div>
  );
}

// ── Hook: 行内 @ 提及管理 ─────────────────────────────────────
export interface UseMentionInputElement {
  getBoundingClientRect: () => DOMRect;
  getCaretRect?: () => DOMRect | null;
}

export interface UseHxMentionOptions {
  inputRef: React.RefObject<UseMentionInputElement | null>;
  sections?: MentionSection[];
}

export interface UseHxMentionReturn {
  isOpen: boolean;
  filter: string;
  position: { x: number; y: number };
  sections: MentionSection[];
  handleInputChange: (value: string, cursorPosition: number) => void;
  close: () => void;
  handleSelectMention: (item: MentionItem) => { value: string; cursorPosition: number };
}

export function useHxMention({
  inputRef,
  sections: customSections,
}: UseHxMentionOptions): UseHxMentionReturn {
  const [isOpen, setIsOpen] = useState(false);
  const [filter, setFilter] = useState('');
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [atStart, setAtStart] = useState(-1);
  const currentInputRef = useRef({ value: '', cursorPosition: 0 });

  const defaultSections: MentionSection[] = EMPTY_MENTION_SECTIONS;

  const sections = customSections ?? defaultSections;

  const handleInputChange = useCallback(
    (value: string, cursorPosition: number) => {
      currentInputRef.current = { value, cursorPosition };
      const textBefore = value.slice(0, cursorPosition);
      // Match @xxx but not @@, not email-like (foo@bar)
      const atMatch = textBefore.match(/(?:^|\s)@(\w*)$/);

      const hasItems = sections.some(s => s.items.length > 0);

      if (atMatch && hasItems) {
        const filterText = atMatch[1] || '';
        const filtered = filterSections(sections, filterText);
        const hasFiltered = filtered.some(s => s.items.length > 0);

        if (!hasFiltered) {
          setIsOpen(false);
          setFilter('');
          setAtStart(-1);
          return;
        }

        const matchStart = textBefore.lastIndexOf('@');
        setAtStart(matchStart);
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
        setAtStart(-1);
      }
    },
    [inputRef, sections],
  );

  const handleSelectMention = useCallback(
    (item: MentionItem): { value: string; cursorPosition: number } => {
      if (atStart < 0) return { value: '', cursorPosition: 0 };

      const { value: currentValue, cursorPosition } = currentInputRef.current;
      const before = currentValue.slice(0, atStart);
      const after = currentValue.slice(cursorPosition);

      const mentionTag = `@${item.label} `;
      const result = before + mentionTag + after;
      const newCursor = before.length + mentionTag.length;

      setIsOpen(false);
      return { value: result, cursorPosition: newCursor };
    },
    [atStart],
  );

  const close = useCallback(() => {
    setIsOpen(false);
    setFilter('');
    setAtStart(-1);
  }, []);

  return { isOpen, filter, position, sections, handleInputChange, close, handleSelectMention };
}
