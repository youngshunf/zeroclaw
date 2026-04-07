import { useState } from 'react';
import { addCronJob } from '@/lib/cron-api';
import { X } from 'lucide-react';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Textarea } from '@/components/ui/Textarea';

interface TaskEditorModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSuccess: () => void;
}

export function TaskEditorModal({ open, onOpenChange, onSuccess }: TaskEditorModalProps) {
  const [loading, setLoading] = useState(false);
  const [name, setName] = useState('');
  const [expression, setExpression] = useState('0 * * * * *');
  const [prompt, setPrompt] = useState('');

  if (!open) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    try {
      await addCronJob(expression, prompt, name || undefined);
      onSuccess();
      setName('');
      setExpression('0 * * * * *');
      setPrompt('');
    } catch (err) {
      console.error(err);
      alert(`保存失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div className="w-[500px] h-[550px] bg-hx-bg-main border border-hx-border rounded-xl shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-200">
        <form onSubmit={handleSubmit} className="flex flex-col h-full">
          {/* Header */}
          <div className="flex items-center justify-between px-6 p-4 border-b border-hx-border">
            <div>
              <h2 className="text-lg font-bold text-hx-text-primary">新建定时任务</h2>
              <p className="text-xs text-hx-text-secondary mt-0.5">
                设置基于时间的自动执行计划，由底层的多模态 Agent 代为处理跑腿任务。
              </p>
            </div>
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="text-hx-text-tertiary hover:text-hx-text-primary transition-colors bg-transparent border-none cursor-pointer p-1"
            >
              <X size={18} />
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 p-6 space-y-5 overflow-auto">
            <div className="space-y-2">
              <label className="text-sm font-medium text-hx-text-primary">任务名称</label>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="例如：每日简报总结"
              />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium text-hx-text-primary flex justify-between">
                <span>执行频率 (Cron 表达式) <span className="text-red-500">*</span></span>
              </label>
              <div className="flex gap-2">
                <Select value={expression} onValueChange={setExpression}>
                  <SelectTrigger className="w-[180px] bg-hx-bg-input">
                    <SelectValue placeholder="选择频率" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="0 * * * * *">每分钟</SelectItem>
                    <SelectItem value="0 0 * * * *">每小时</SelectItem>
                    <SelectItem value="0 0 8 * * *">每天早 8 点</SelectItem>
                    <SelectItem value="0 0 9 * * 1">每周一早 9 点</SelectItem>
                  </SelectContent>
                </Select>
                <Input
                  value={expression}
                  onChange={(e) => setExpression(e.target.value)}
                  placeholder="0 * * * * *"
                  className="flex-1"
                />
              </div>
            </div>

            <div className="space-y-2 flex-1 flex flex-col h-[180px]">
              <label className="text-sm font-medium text-hx-text-primary">
                Agent 任务指令 (Prompt) <span className="text-red-500">*</span>
              </label>
              <Textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder="任务触发时，告诉 Agent 要做什么..."
                className="flex-1 resize-none"
                required
              />
            </div>
          </div>

          {/* Footer */}
          <div className="p-4 border-t border-hx-border flex justify-end gap-3 bg-hx-bg-panel">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="px-4 py-2 text-sm font-medium text-hx-text-secondary hover:text-hx-text-primary border border-hx-border hover:bg-hx-bg-input rounded-lg transition-colors bg-transparent cursor-pointer"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={loading || !prompt || !expression}
              className="px-4 py-2 text-sm font-medium text-white bg-hx-purple hover:bg-hx-purple-hover rounded-lg border-none shadow-sm cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loading ? '保存中...' : '保存任务'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
