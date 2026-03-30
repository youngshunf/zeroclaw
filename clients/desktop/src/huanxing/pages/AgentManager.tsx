/**
 * Agent 管理页面 — 创建、切换、删除 Agent，编辑工作区文件。
 *
 * 布局：
 *  左侧：Agent 列表，每个 Agent 下方可折叠展示文件列表
 *  右侧：文件编辑器（撑满整个右侧区域）
 */
import { useCallback, useEffect, useState } from 'react';
import {
  Plus,
  Trash2,
  Bot,
  FileText,
  ChevronRight,
  ChevronDown,
  Loader2,
  Star,
  Save,
  X,
  AlertCircle,
  FolderOpen,
} from 'lucide-react';
import {
  listAgents,
  createAgent,
  deleteAgent,
  listFiles,
  readFile,
  writeFile,
  type AgentInfo,
  type CreateAgentParams,
} from '../lib/agent-api';
import { AGENT_TEMPLATES, listAvailableTemplates } from '../lib/agent-templates';

// ── 模板类型（兼容 hub 模板和本地模板）──────────────────────────

interface AvailableTemplate {
  id: string;
  name: string;
  description: string;
  icon: string;
  fromHub: boolean;
  // 本地模板专有字段
  model?: string;
  temperature?: number;
  soulMd?: string;
  identityMd?: string;
}

// ── Create Agent Dialog ────────────────────────────────────────

interface CreateDialogProps {
  open: boolean;
  onClose: () => void;
  onCreate: (params: CreateAgentParams) => Promise<void>;
}

function CreateAgentDialog({ open, onClose, onCreate }: CreateDialogProps) {
  const [displayName, setDisplayName] = useState('');
  const [selectedId, setSelectedId] = useState('assistant');
  const [templates, setTemplates] = useState<AvailableTemplate[]>([]);
  const [templatesLoading, setTemplatesLoading] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  // 每次打开时加载模板（优先 hub，失败回退本地）
  useEffect(() => {
    if (!open) return;
    setTemplatesLoading(true);
    listAvailableTemplates().then((list) => {
      const enriched: AvailableTemplate[] = list.map((t) => {
        if (!t.fromHub) {
          const local = AGENT_TEMPLATES.find((l) => l.id === t.id);
          return { ...t, model: local?.model, temperature: local?.temperature, soulMd: local?.soulMd, identityMd: local?.identityMd };
        }
        return t;
      });
      setTemplates(enriched);
      if (enriched.length > 0) setSelectedId(enriched[0].id);
    }).finally(() => setTemplatesLoading(false));
  }, [open]);

  if (!open) return null;

  const selectedTemplate = templates.find((t) => t.id === selectedId) ?? templates[0];

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      // name 是目录名（仅 [a-zA-Z0-9_-]），自动生成；display_name 是显示名，支持中文
      const autoName = `${selectedTemplate?.id ?? 'agent'}-${Date.now().toString(36).slice(-4)}`;
      const params: CreateAgentParams = {
        name: autoName,
        display_name: displayName.trim() || selectedTemplate?.name,
        model: selectedTemplate?.model,
        temperature: selectedTemplate?.temperature,
      };

      if (selectedTemplate?.fromHub) {
        params.template = selectedTemplate.id;
      } else {
        params.soul_md = selectedTemplate?.soulMd;
        params.identity_md = selectedTemplate?.identityMd;
      }

      await onCreate(params);
      setDisplayName('');
      onClose();
    } catch (err: any) {
      setError(err.message || '创建失败');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20 backdrop-blur-sm">
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-lg rounded-2xl border border-[#7c3aed] bg-white p-6 shadow-2xl"
      >
        <h3 className="mb-4 text-lg font-semibold text-gray-900 flex items-center gap-2">
          <Plus className="h-5 w-5 text-[#7c3aed]" />
          创建新 Agent
        </h3>

        {error && (
          <div className="mb-3 flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-600">
            <AlertCircle className="h-4 w-4 shrink-0" />
            {error}
          </div>
        )}

        <label className="mb-2 block text-sm text-gray-600">选择模板</label>
        {templatesLoading ? (
          <div className="mb-4 flex items-center justify-center py-6">
            <Loader2 className="h-5 w-5 animate-spin text-[#7c3aed]" />
          </div>
        ) : (
          <div className="mb-4 grid grid-cols-3 gap-2">
            {templates.map((tpl) => (
              <button
                key={tpl.id}
                type="button"
                onClick={() => setSelectedId(tpl.id)}
                className={[
                  'flex flex-col items-center gap-1 rounded-xl border p-3 text-center transition-all',
                  selectedId === tpl.id
                    ? 'border-[#7c3aed] bg-[#7c3aed]/15 text-[#7c3aed]'
                    : 'border-gray-200 bg-[#FAFAFA]/60 text-gray-600 hover:border-[#7c3aed]',
                ].join(' ')}
              >
                <span className="text-xl">{tpl.icon}</span>
                <span className="text-xs font-medium">{tpl.name}</span>
              </button>
            ))}
          </div>
        )}

        {selectedTemplate && (
          <p className="mb-4 text-xs text-gray-400">{selectedTemplate.description}</p>
        )}

        <label className="mb-1 block text-sm text-gray-600">名称（可选，支持中文）</label>
        <input
          type="text"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder={selectedTemplate?.name ?? '给你的 Agent 起个名字'}
          className="mb-4 w-full rounded-lg border border-gray-300 bg-[#F9FAFB] px-3 py-2 text-gray-900 placeholder:text-gray-400 focus:border-[#7c3aed] focus:outline-none"
          autoFocus
        />

        {selectedTemplate?.model && (
          <div className="mb-4 flex items-center gap-2 text-xs text-gray-400">
            <span>模型: {selectedTemplate.model}</span>
            <span>·</span>
            <span>温度: {selectedTemplate.temperature}</span>
          </div>
        )}

        <div className="flex justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-[#7c3aed] px-4 py-2 text-sm text-gray-600 hover:bg-[#FAFAFA]"
          >
            取消
          </button>
          <button
            type="submit"
            disabled={loading || templatesLoading}
            className="flex items-center gap-2 rounded-lg bg-[#7c3aed] px-4 py-2 text-sm font-medium text-white hover:bg-[#6d28d9] disabled:opacity-50"
          >
            {loading && <Loader2 className="h-4 w-4 animate-spin" />}
            创建
          </button>
        </div>
      </form>
    </div>
  );
}

// ── File Editor (full height) ──────────────────────────────────

interface FileEditorProps {
  agentName: string;
  filename: string;
  onClose: () => void;
}

function FileEditor({ agentName, filename, onClose }: FileEditorProps) {
  const [content, setContent] = useState('');
  const [original, setOriginal] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    setLoading(true);
    readFile(agentName, filename)
      .then((c) => {
        setContent(c);
        setOriginal(c);
      })
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [agentName, filename]);

  const handleSave = async () => {
    setSaving(true);
    setError('');
    try {
      await writeFile(agentName, filename, content);
      setOriginal(content);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSaving(false);
    }
  };

  // Ctrl/Cmd+S to save
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault();
        if (content !== original) handleSave();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [content, original]);

  const dirty = content !== original;

  return (
    <div className="flex h-full w-full flex-col min-h-0 min-w-0 overflow-hidden bg-white">
      {/* Header */}
      <div 
        className="relative z-10 bg-white flex items-center justify-between border-b border-gray-200 px-5 py-3 shrink-0"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      >
        <div className="flex items-center gap-2 text-sm text-gray-900 min-w-0" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <FileText className="h-4 w-4 text-[#7c3aed] shrink-0" />
          <span className="font-semibold truncate">{agentName}</span>
          <span className="text-gray-400">/</span>
          <span className="font-medium text-gray-500">{filename}</span>
          {dirty && <span className="text-xs text-amber-600 shrink-0">• 未保存</span>}
        </div>
        <div className="flex items-center gap-2 shrink-0" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <button
            onClick={handleSave}
            disabled={saving || !dirty}
            className="flex items-center gap-1.5 rounded-lg bg-[#7c3aed] px-3 py-1.5 text-xs font-medium text-white hover:bg-[#6d28d9] disabled:opacity-40 transition-opacity"
          >
            {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
            保存
          </button>
          <button
            onClick={onClose}
            className="rounded-lg p-1.5 text-gray-600 hover:bg-[#FAFAFA] hover:text-gray-900 transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="mx-4 mt-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-xs text-red-600 shrink-0">
          {error}
        </div>
      )}

      {/* Editor — fills all remaining space */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="h-6 w-6 animate-spin text-[#7c3aed]" />
        </div>
      ) : (
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          className="flex-1 min-h-0 w-full resize-none bg-white px-5 py-4 font-mono text-sm leading-relaxed text-gray-800 placeholder:text-gray-400 focus:outline-none overflow-auto"
          spellCheck={false}
        />
      )}
    </div>
  );
}

// ── Agent Card (with collapsible file list) ────────────────────

interface AgentCardProps {
  agent: AgentInfo;
  expanded: boolean;
  files: string[];
  filesLoading: boolean;
  editingFile: string | null;
  onToggleExpand: () => void;
  onDelete: () => void;
  onSelectFile: (filename: string) => void;
}

function AgentCard({
  agent, expanded, files, filesLoading, editingFile,
  onToggleExpand, onDelete, onSelectFile,
}: AgentCardProps) {
  const ChevronIcon = expanded ? ChevronDown : ChevronRight;

  return (
    <div className="rounded-2xl border border-gray-200 bg-[#FAFAFA]/60 overflow-hidden transition-all duration-200">
      {/* Agent header */}
      <div
        className={[
          'flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors',
          agent.active
            ? 'bg-[#7c3aed]/10 border-b border-[#7c3aed]/20'
            : 'hover:bg-white/80',
          expanded && !agent.active ? 'border-b border-gray-200' : '',
        ].join(' ')}
        onClick={onToggleExpand}
      >
        {/* Icon */}
        <div className={[
          'flex h-9 w-9 items-center justify-center shrink-0 overflow-hidden',
          agent.icon_url ? '' : 'rounded-xl',
          !agent.icon_url && agent.active ? 'bg-[#7c3aed]/30' : '',
          !agent.icon_url && !agent.active ? 'bg-gray-200' : '',
        ].join(' ')}>
          {agent.icon_url ? (
            <img src={agent.icon_url} alt={agent.name} className="h-full w-full object-cover rounded-xl" />
          ) : agent.active ? (
            <Star className="h-4 w-4 text-[#7c3aed]" />
          ) : (
            <Bot className="h-4 w-4 text-gray-400" />
          )}
        </div>

        {/* Name + model */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-gray-900 truncate">{agent.display_name || agent.name}</h3>
            {agent.active && (
              <span className="flex items-center gap-1 shrink-0">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-400 animate-pulse" />
                <span className="text-[10px] text-emerald-600">活跃</span>
              </span>
            )}
          </div>
          <p className="text-xs text-gray-400 truncate">{agent.model || '未配置模型'}</p>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          {!agent.active && (
            <button
              onClick={(e) => { e.stopPropagation(); onDelete(); }}
              title="删除"
              className="rounded-lg p-1.5 text-gray-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          )}
          <ChevronIcon className="h-4 w-4 text-gray-400 transition-transform" />
        </div>
      </div>

      {/* Collapsible file list */}
      {expanded && (
        <div className="py-1">
          {filesLoading ? (
            <div className="flex items-center justify-center py-4">
              <Loader2 className="h-4 w-4 animate-spin text-gray-400" />
            </div>
          ) : files.length === 0 ? (
            <p className="px-4 py-3 text-xs text-gray-400">暂无工作区文件</p>
          ) : (
            files.map((f) => (
              <button
                key={f}
                onClick={() => onSelectFile(f)}
                className={[
                  'flex w-full items-center gap-2 px-4 py-2 text-left text-xs transition-colors',
                  editingFile === f
                    ? 'bg-[#7c3aed]/15 text-[#7c3aed]'
                    : 'text-gray-600 hover:bg-white hover:text-gray-900',
                ].join(' ')}
              >
                <FileText className="h-3.5 w-3.5 shrink-0 text-gray-400" />
                <span className="truncate">{f}</span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}

// ── Main Page ──────────────────────────────────────────────────

export default function AgentManager() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [showCreate, setShowCreate] = useState(false);

  // Expanded agent & files
  const [expandedAgent, setExpandedAgent] = useState<string | null>(null);
  const [files, setFiles] = useState<string[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);

  // Editor state
  const [editingAgent, setEditingAgent] = useState<string | null>(null);
  const [editingFile, setEditingFile] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await listAgents();
      setAgents(data.agents);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // Load files when expanded agent changes
  useEffect(() => {
    if (!expandedAgent) {
      setFiles([]);
      return;
    }
    setFilesLoading(true);
    listFiles(expandedAgent)
      .then(setFiles)
      .catch(() => setFiles([]))
      .finally(() => setFilesLoading(false));
  }, [expandedAgent]);

  const handleCreate = async (params: CreateAgentParams) => {
    await createAgent(params);
    await refresh();
  };

  const handleDelete = async (name: string) => {
    if (!confirm(`确定要删除 Agent「${name}」吗？此操作不可撤销。`)) return;
    try {
      await deleteAgent(name);
      if (editingAgent === name) {
        setEditingAgent(null);
        setEditingFile(null);
      }
      if (expandedAgent === name) setExpandedAgent(null);
      await refresh();
    } catch (e: any) {
      setError(e.message);
    }
  };

  const toggleExpand = (name: string) => {
    setExpandedAgent(expandedAgent === name ? null : name);
  };

  const selectFile = (agentName: string, filename: string) => {
    setEditingAgent(agentName);
    setEditingFile(filename);
  };

  return (
    <div className="flex flex-row h-full min-h-0 w-full min-w-0 overflow-hidden text-gray-900 bg-white">
      {/* ─── Left: Agent List ─── */}
      <div className="flex w-72 shrink-0 flex-col border-r border-gray-200 min-h-0 bg-white">
        <div 
          className="relative z-10 bg-white flex items-center justify-between border-b border-gray-200 px-4 py-3 shrink-0"
          style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
          data-tauri-drag-region
        >
          <h2 className="text-sm font-semibold text-gray-900" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>Agent 管理</h2>
          <button
            onClick={() => setShowCreate(true)}
            style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
            className="flex items-center gap-1.5 rounded-lg bg-[#7c3aed] px-2.5 py-1.5 text-xs font-medium text-white hover:bg-[#6d28d9] transition-colors"
          >
            <Plus className="h-3.5 w-3.5" />
            新建
          </button>
        </div>

        {error && (
          <div className="mx-3 mt-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-600 shrink-0">
            {error}
            <button onClick={() => setError('')} className="ml-2 text-red-400 hover:text-red-200">×</button>
          </div>
        )}

        <div className="flex-1 space-y-2 overflow-y-auto p-3 min-h-0">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-6 w-6 animate-spin text-[#7c3aed]" />
            </div>
          ) : agents.length === 0 ? (
            <div className="py-12 text-center">
              <Bot className="mx-auto mb-3 h-10 w-10 text-gray-300" />
              <p className="text-sm text-gray-400">还没有 Agent</p>
              <p className="mt-1 text-xs text-gray-400">点击"新建"创建你的第一个 AI 分身</p>
            </div>
          ) : (
            agents.map((a) => (
              <AgentCard
                key={a.name}
                agent={a}
                expanded={expandedAgent === a.name}
                files={expandedAgent === a.name ? files : []}
                filesLoading={expandedAgent === a.name && filesLoading}
                editingFile={editingAgent === a.name ? editingFile : null}
                onToggleExpand={() => toggleExpand(a.name)}
                onDelete={() => handleDelete(a.name)}
                onSelectFile={(f) => selectFile(a.name, f)}
              />
            ))
          )}
        </div>
      </div>

      {/* ─── Right: Editor (full size) ─── */}
      <div className="flex flex-1 flex-col min-w-0 min-h-0 overflow-hidden">
        {editingFile && editingAgent ? (
          <FileEditor
            agentName={editingAgent}
            filename={editingFile}
            onClose={() => setEditingFile(null)}
          />
        ) : (
          <div className="flex flex-1 items-center justify-center">
            <div className="text-center">
              <FolderOpen className="mx-auto mb-3 h-12 w-12 text-[#1e2f5d]" />
              <p className="text-sm text-gray-400">展开 Agent 选择文件编辑</p>
              <p className="mt-1 text-xs text-gray-400">支持 Cmd+S 保存</p>
            </div>
          </div>
        )}
      </div>

      {/* Create Dialog */}
      <CreateAgentDialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onCreate={handleCreate}
      />
    </div>
  );
}
