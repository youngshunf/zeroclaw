/**
 * TrustLevelSelector — 信任等级下拉选择器
 *
 * 点击 TrustBadge 弹出，可修改联系人信任等级 (0-5)
 */
import { useState, useRef, useCallback } from 'react';
import { TRUST_LEVEL_LABELS, TRUST_LEVEL_COLORS, type TrustLevel } from '@/lib/hasn-api';

const TRUST_OPTIONS: Array<{ level: TrustLevel; label: string; color: string; emoji: string }> = [
  { level: 0, label: '已拉黑', color: '#9ca3af', emoji: '🚫' },
  { level: 1, label: '陌生人', color: '#94a3b8', emoji: '❓' },
  { level: 2, label: '普通联系人', color: '#60a5fa', emoji: '👤' },
  { level: 3, label: '朋友', color: '#34d399', emoji: '🤝' },
  { level: 4, label: '密友', color: '#f59e0b', emoji: '⭐' },
  { level: 5, label: '所有者', color: '#a78bfa', emoji: '👑' },
];

interface TrustBadgeProps {
  level: number;
  label?: string;
  editable?: boolean;
  onSelect?: (level: TrustLevel) => void;
  /** 如果为 true，不允许选择 level 5（仅限 Agent） */
  disableOwner?: boolean;
}

export function TrustBadge({ level, label, editable = false, onSelect, disableOwner = true }: TrustBadgeProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const colorMap: Record<number, string> = {
    0: '#9ca3af',
    1: '#94a3b8',
    2: '#60a5fa',
    3: '#34d399',
    4: '#f59e0b',
    5: '#a78bfa',
  };
  const color = colorMap[level] ?? colorMap[1];
  const displayLabel = label ?? TRUST_LEVEL_LABELS[level] ?? `L${level}`;

  const handleSelect = useCallback((newLevel: TrustLevel) => {
    onSelect?.(newLevel);
    setOpen(false);
  }, [onSelect]);

  return (
    <div className="relative inline-block" ref={ref}>
      <span
        className={`text-[10px] px-1.5 py-[1px] rounded-lg font-medium ${editable ? 'cursor-pointer hover:opacity-80 transition-opacity' : ''}`}
        style={{ background: `${color}20`, color }}
        onClick={editable ? () => setOpen(!open) : undefined}
      >
        {displayLabel}
        {editable && <span className="ml-0.5 text-[8px]">▼</span>}
      </span>

      {open && (
        <>
          <div
            className="hx-trust-selector-backdrop"
            onClick={() => setOpen(false)}
          />
          <div className="hx-trust-selector" style={{ top: '100%', left: '50%', transform: 'translateX(-50%)', marginTop: 4 }}>
            {TRUST_OPTIONS.map((opt) => {
              if (disableOwner && opt.level === 5) return null;
              return (
                <button
                  key={opt.level}
                  className={`hx-trust-selector-item ${level === opt.level ? 'active' : ''}`}
                  onClick={() => handleSelect(opt.level)}
                >
                  <span className="hx-trust-selector-dot" style={{ background: opt.color }} />
                  <span>{opt.emoji} {opt.label}</span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

export default TrustBadge;
