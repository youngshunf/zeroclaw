import React, { useEffect } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import { Markdown } from 'tiptap-markdown';
import { Bold, Italic, Heading1, Heading2, List, ListOrdered, Code, Quote } from 'lucide-react';

export interface TipTapEditorProps {
  value: string;
  onChange: (value: string) => void;
  editable?: boolean;
  /** Called when pasted markdown's first line is a heading — useful to auto-fill title */
  onPasteTitle?: (title: string) => void;
}

export default function TipTapEditor({ value, onChange, editable = true, onPasteTitle }: TipTapEditorProps) {
  // Keep a ref so the handlePaste closure always sees the latest callback
  const onPasteTitleRef = React.useRef(onPasteTitle);
  React.useEffect(() => { onPasteTitleRef.current = onPasteTitle; }, [onPasteTitle]);
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: {
          levels: [1, 2, 3, 4, 5, 6],
        },
      }),
      Markdown.configure({
        html: true,
        transformPastedText: true,
        transformCopiedText: true,
      }),
    ],
    content: value,
    editable,
    onUpdate: ({ editor }) => {
      const markdown = (editor.storage as any).markdown.getMarkdown();
      onChange(markdown);
    },
    editorProps: {
      attributes: {
        class: 'hx-tiptap-editor h-full',
      },
      handlePaste: (view, event) => {
        const clipboardData = event.clipboardData;
        if (!clipboardData) return false;

        const text = clipboardData.getData('text/plain');
        if (!text) return false;

        const html = clipboardData.getData('text/html');

        // If clipboard has rich HTML with semantic structure (real headings/lists),
        // let TipTap's default HTML handler deal with it
        if (html) {
          const parsed = new DOMParser().parseFromString(html, 'text/html');
          const hasSemanticHtml = !!(
            parsed.querySelector('h1, h2, h3, h4, h5, h6') ||
            parsed.querySelector('table, ul, ol, li') ||
            parsed.querySelector('blockquote, pre')
          );
          if (hasSemanticHtml) return false;
        }

        // Try to parse as markdown using tiptap-markdown's text parser
        try {
          const mdParser = (view as any).someProp('clipboardTextParser');
          if (mdParser) {
            const slice = mdParser(text, view.state.selection.$from, false, view);
            if (slice && slice.content.childCount > 0) {
              event.preventDefault();
              view.dispatch(view.state.tr.replaceSelection(slice));

              // Extract first-line heading as document title
              const headingMatch = text.match(/^#{1,6}\s+(.+)/);
              if (headingMatch && onPasteTitleRef.current) {
                onPasteTitleRef.current(headingMatch[1].trim());
              }
              return true;
            }
          }
        } catch (e) {
          console.warn('[TipTap] Markdown paste fallback:', e);
        }

        return false;
      },
    },
  });

  // Keep content synced if value changes externally
  useEffect(() => {
    if (editor && value !== (editor.storage as any).markdown.getMarkdown()) {
      editor.commands.setContent(value);
    }
  }, [value, editor]);

  if (!editor) {
    return null;
  }

  return (
    <div className="w-full flex flex-col h-full bg-hx-bg-main border border-hx-border rounded-hx-radius-sm overflow-hidden text-hx-text-primary">
      {/* 工具栏 */}
      {editable && (
        <div className="flex px-2 py-1.5 border-b border-hx-border gap-1 bg-hx-bg-panel items-center flex-wrap shrink-0">
          <MenuButton
            isActive={editor.isActive('bold')}
            onClick={() => editor.chain().focus().toggleBold().run()}
            icon={<Bold size={14} />}
            title="加粗"
          />
          <MenuButton
            isActive={editor.isActive('italic')}
            onClick={() => editor.chain().focus().toggleItalic().run()}
            icon={<Italic size={14} />}
            title="斜体"
          />
          <div className="w-[1px] h-4 bg-hx-border mx-1" />
          <MenuButton
            isActive={editor.isActive('heading', { level: 1 })}
            onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
            icon={<Heading1 size={14} />}
            title="标题1"
          />
          <MenuButton
            isActive={editor.isActive('heading', { level: 2 })}
            onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
            icon={<Heading2 size={14} />}
            title="标题2"
          />
          <div className="w-[1px] h-4 bg-hx-border mx-1" />
          <MenuButton
            isActive={editor.isActive('bulletList')}
            onClick={() => editor.chain().focus().toggleBulletList().run()}
            icon={<List size={14} />}
            title="无序列表"
          />
          <MenuButton
            isActive={editor.isActive('orderedList')}
            onClick={() => editor.chain().focus().toggleOrderedList().run()}
            icon={<ListOrdered size={14} />}
            title="有序列表"
          />
          <div className="w-[1px] h-4 bg-hx-border mx-1" />
          <MenuButton
            isActive={editor.isActive('blockquote')}
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
            icon={<Quote size={14} />}
            title="引用"
          />
          <MenuButton
            isActive={editor.isActive('codeBlock')}
            onClick={() => editor.chain().focus().toggleCodeBlock().run()}
            icon={<Code size={14} />}
            title="代码块"
          />
        </div>
      )}

      {/* 编辑区域 */}
      <div className="flex-1 overflow-y-auto p-4">
        <EditorContent editor={editor} className="h-full" />
      </div>
    </div>
  );
}

// 提取工具栏按钮组件
function MenuButton({
  isActive,
  onClick,
  icon,
  title,
}: {
  isActive: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`p-1.5 rounded-hx-radius-xs flex items-center justify-center transition-colors ${
        isActive
          ? 'bg-hx-purple/20 text-hx-purple'
          : 'text-hx-text-secondary hover:bg-hx-bg-hover hover:text-hx-text-primary'
      }`}
    >
      {icon}
    </button>
  );
}
