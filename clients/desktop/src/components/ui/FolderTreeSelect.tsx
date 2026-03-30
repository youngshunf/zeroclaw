import * as React from 'react';
import { Folder, ChevronRight, ChevronDown, Check } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export interface FolderTreeNode {
  id: number;
  name: string;
  children: FolderTreeNode[];
}

interface FolderTreeSelectProps {
  tree: FolderTreeNode[];
  selectedId: number | null;
  onSelect: (id: number | null) => void;
}

const FolderTreeItem = ({ 
  node, 
  depth, 
  selectedId, 
  onSelect 
}: { 
  node: FolderTreeNode; 
  depth: number; 
  selectedId: number | null; 
  onSelect: (id: number | null) => void;
}) => {
  const [isOpen, setIsOpen] = React.useState(true);
  const isSelected = selectedId === node.id;
  const hasChildren = node.children && node.children.length > 0;

  return (
    <div className="flex flex-col">
      <div
        onClick={() => onSelect(node.id)}
        className={cn(
          'group flex items-center gap-2 px-2 py-1.5 rounded-md cursor-pointer transition-colors select-none',
          isSelected 
            ? 'bg-hx-purple/10 text-hx-purple' 
            : 'hover:bg-hx-bg-hover text-hx-text-secondary hover:text-hx-text-primary'
        )}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        <button
          onClick={(e) => {
            e.stopPropagation();
            setIsOpen(!isOpen);
          }}
          className={cn(
            'p-0.5 rounded hover:bg-black/5 dark:hover:bg-white/10 transition-transform bg-transparent border-none cursor-pointer',
            !hasChildren && 'invisible'
          )}
        >
          {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
        <Folder size={14} className={cn('shrink-0', isSelected ? 'text-hx-purple' : 'text-hx-text-tertiary')} />
        <span className="truncate text-[13px] flex-1">{node.name}</span>
        {isSelected && <Check size={14} className="shrink-0" />}
      </div>
      {isOpen && hasChildren && (
        <div className="flex flex-col">
          {node.children.map((child) => (
            <FolderTreeItem
              key={child.id}
              node={child}
              depth={depth + 1}
              selectedId={selectedId}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export function FolderTreeSelect({ tree, selectedId, onSelect }: FolderTreeSelectProps) {
  return (
    <div className="flex flex-col gap-1 max-h-[300px] overflow-y-auto pr-1 hx-scrollbar">
      {/* Root Option */}
      <div
        onClick={() => onSelect(null)}
        className={cn(
          'flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer transition-colors border border-dashed text-[13px] font-medium',
          selectedId === null 
            ? 'bg-hx-purple/10 border-hx-purple text-hx-purple' 
            : 'border-hx-border text-hx-text-secondary hover:border-hx-purple/50 hover:text-hx-text-primary'
        )}
      >
        <Folder size={14} className="opacity-60" />
        <span>（根目录 / 移出当前目录）</span>
        {selectedId === null && <Check size={14} className="ml-auto" />}
      </div>

      <div className="mt-2 flex flex-col">
        {tree.map((node) => (
          <FolderTreeItem
            key={node.id}
            node={node}
            depth={0}
            selectedId={selectedId}
            onSelect={onSelect}
          />
        ))}
        {tree.length === 0 && (
          <div className="py-8 text-center text-[12px] text-hx-text-tertiary italic">
            暂无子目录
          </div>
        )}
      </div>
    </div>
  );
}
