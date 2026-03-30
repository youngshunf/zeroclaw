import { useState, useCallback, useEffect, useMemo } from 'react';
import { FileText, Plus, Search, Loader2, Save, Trash2, Globe, Lock, Clock, Edit3, Share2, BookOpen, ExternalLink, X, Copy, ChevronRight, ChevronDown, Folder, FolderPlus, FilePlus, ArrowRightLeft } from 'lucide-react';
import TipTapEditor from '@/components/TipTapEditor';
import MarkdownPreview from '@/components/MarkdownPreview';
import { getHuanxingSession } from '@/huanxing/config';
import {
  getHuanxingDocumentListApi,
  createHuanxingDocumentApi,
  updateHuanxingDocumentApi,
  deleteHuanxingDocumentApi,
  getHuanxingFolderTreeApi,
  createHuanxingFolderApi,
  deleteHuanxingFolderApi,
  moveHuanxingDocumentApi,
  type HuanxingDocumentResult,
  type HuanxingFolderTreeNode,
} from '@/huanxing/lib/document-api';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from '@/components/ui/Dialog';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/AlertDialog';
import { FolderTreeSelect } from '@/components/ui/FolderTreeSelect';
import { Input } from '@/components/ui/Input';

export default function Documents() {
  const [documents, setDocuments] = useState<HuanxingDocumentResult[]>([]);
  const [folderTree, setFolderTree] = useState<HuanxingFolderTreeNode[]>([]);
  const [expandedFolders, setExpandedFolders] = useState<Set<number>>(new Set());
  
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [selectedDoc, setSelectedDoc] = useState<HuanxingDocumentResult | null>(null);
  const [editorContent, setEditorContent] = useState('');
  const [editorTitle, setEditorTitle] = useState('');
  const [isSaving, setIsSaving] = useState(false);
  const [newDocFolderId, setNewDocFolderId] = useState<number | null>(null);

  // 双态切换
  const [isEditing, setIsEditing] = useState(false);
  // 分享弹窗
  const [showShareModal, setShowShareModal] = useState(false);

  // 对话框状态控制
  const [deleteDocTarget, setDeleteDocTarget] = useState<HuanxingDocumentResult | null>(null);
  const [deleteFolderTarget, setDeleteFolderTarget] = useState<{ id: number; name: string } | null>(null);
  const [folderCreateTarget, setFolderCreateTarget] = useState<{ parentId: number | null } | null>(null);
  const [moveDocTarget, setMoveDocTarget] = useState<HuanxingDocumentResult | null>(null);
  const [newFolderName, setNewFolderName] = useState('');
  const [moveTargetFolderId, setMoveTargetFolderId] = useState<number | null>(null);

  const fetchFolders = useCallback(async () => {
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      const res = await getHuanxingFolderTreeApi(session.accessToken);
      setFolderTree(res.data || []);
    } catch (err) {
      console.error('Fetch folders failed:', err);
    }
  }, []);

  const fetchDocuments = useCallback(async () => {
    try {
      setLoading(true);
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      
      const res = await getHuanxingDocumentListApi(session.accessToken, {
        size: 100, // 避免超长引起 FastAPI 422 Unprocessable Error
        title: searchQuery || undefined,
      });
      const responseData = res as any;
      let items: HuanxingDocumentResult[] = [];
      if (Array.isArray(responseData)) {
        items = responseData;
      } else if (responseData?.data) {
        if (Array.isArray(responseData.data)) items = responseData.data;
        else if (Array.isArray(responseData.data.items)) items = responseData.data.items;
        else if (Array.isArray(responseData.data.list)) items = responseData.data.list;
        else if (Array.isArray(responseData.data.records)) items = responseData.data.records;
      }
      setDocuments(items);
    } catch (err) {
      console.error('Fetch docs failed:', err);
    } finally {
      setLoading(false);
    }
  }, [searchQuery]);

  useEffect(() => {
    fetchFolders();
    fetchDocuments();
  }, [fetchFolders, fetchDocuments]);

  const toggleFolder = (folderId: number) => {
    setExpandedFolders(prev => {
      const next = new Set(prev);
      if (next.has(folderId)) next.delete(folderId);
      else next.add(folderId);
      return next;
    });
  };

  const confirmCreateFolder = async () => {
    if (!newFolderName.trim() || !folderCreateTarget) return;
    const { parentId } = folderCreateTarget;
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      await createHuanxingFolderApi(session.accessToken, { name: newFolderName.trim(), parent_id: parentId });
      fetchFolders();
      if (parentId) {
        setExpandedFolders(prev => new Set(prev).add(parentId));
      }
      setFolderCreateTarget(null);
      setNewFolderName('');
    } catch (err) {
      console.error('Create folder failed:', err);
    }
  };

  const confirmDeleteFolder = async () => {
    if (!deleteFolderTarget) return;
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      await deleteHuanxingFolderApi(session.accessToken, deleteFolderTarget.id);
      fetchFolders();
      fetchDocuments();
      setDeleteFolderTarget(null);
    } catch (err) {
      console.error('Delete folder failed:', err);
    }
  };

  const handleSelectDoc = (doc: HuanxingDocumentResult) => {
    setSelectedDoc(doc);
    setEditorTitle(doc.title);
    setEditorContent(doc.content || '');
    setIsEditing(false); // 点击列表时默认切入预览态
  };

  const handleCreateNew = (folderId: number | null = null) => {
    setSelectedDoc(null);
    setNewDocFolderId(folderId);
    setEditorTitle('未命名文档');
    setEditorContent('');
    setIsEditing(true); // 新建文档时直接进入编辑态
    if (folderId) {
      setExpandedFolders(prev => new Set(prev).add(folderId));
    }
  };

  const handleSave = async () => {
    if (!editorTitle.trim()) return;
    try {
      setIsSaving(true);
      const session = getHuanxingSession();
      if (!session?.accessToken) return;

      if (selectedDoc) {
        // 更新，仅提交后端 API 所需验证通过的参数阻止 422 Error
        await updateHuanxingDocumentApi(session.accessToken, selectedDoc.id, {
          title: editorTitle,
          content: editorContent,
        });
        const updatedDoc = {
          ...selectedDoc,
          title: editorTitle,
          content: editorContent
        };
        setSelectedDoc(updatedDoc);
        setDocuments(docs => docs.map(d => (d.id === updatedDoc.id ? updatedDoc : d)));
      } else {
        // 创建
        const res = await createHuanxingDocumentApi(session.accessToken, {
          title: editorTitle,
          content: editorContent,
          folder_id: newDocFolderId,
        });
        const newDoc = (res as any).data || res;
        setSelectedDoc(newDoc);
        setDocuments([newDoc, ...documents]);
        setNewDocFolderId(null);
      }
    } catch (err) {
      console.error('Save doc failed:', err);
    } finally {
      setIsSaving(false);
    }
  };

  const confirmDeleteDoc = async () => {
    if (!deleteDocTarget) return;
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      await deleteHuanxingDocumentApi(session.accessToken, deleteDocTarget.id);
      setDocuments(docs => docs.filter(d => d.id !== deleteDocTarget.id));
      if (selectedDoc?.id === deleteDocTarget.id) {
        setSelectedDoc(null);
        setEditorTitle('');
        setEditorContent('');
        setIsEditing(false);
      }
      setDeleteDocTarget(null);
    } catch (err) {
      console.error('Delete doc failed:', err);
    }
  };

  // 生成分享并更改可见性
  const handleToggleShare = async () => {
    if (!selectedDoc) return;
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      const isPublic = !selectedDoc.is_public;
      const shareToken = isPublic ? (selectedDoc.share_token || Math.random().toString(36).substring(2, 10)) : '';
      
      const res = await updateHuanxingDocumentApi(session.accessToken, selectedDoc.id, {
        is_public: isPublic,
        share_token: shareToken,
      });
      const updatedDoc = (res as any).data || res;
      setSelectedDoc(updatedDoc);
      setDocuments(docs => docs.map(d => (d.id === updatedDoc.id ? updatedDoc : d)));
    } catch (e) {
      console.error('Share update failed', e);
    }
  };

  const confirmMoveDocument = async () => {
    if (!moveDocTarget) return;
    
    try {
      const session = getHuanxingSession();
      if (!session?.accessToken) return;
      await moveHuanxingDocumentApi(session.accessToken, moveDocTarget.id, moveTargetFolderId);
      setDocuments(docs => docs.map(d => d.id === moveDocTarget.id ? { ...d, folder_id: moveTargetFolderId } : d));
      setMoveDocTarget(null);
      setMoveTargetFolderId(null);
    } catch(err) {
      console.error('Move doc failed:', err);
    }
  };

  const filteredDocs = documents.filter(doc =>
    doc && doc.title && doc.title.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // 解析 TOC 行目录
  const tocNodes = useMemo(() => {
    const nodes: { id: string; text: string; level: number }[] = [];
    if (!editorContent) return nodes;
    const lines = editorContent.split('\n');
    let idCounter = 0;
    for (const line of lines) {
      const match = line.match(/^(#{1,6})\s+(.*)$/);
      if (match) {
        nodes.push({
          id: `toc-${idCounter++}`,
          level: match[1].length,
          text: match[2].trim(),
        });
      }
    }
    return nodes;
  }, [editorContent]);

  // 大纲滚动跳转
  const scrollToHeading = (text: string) => {
    const cleanText = text.replace(/[\\#*`_]+/g, '').trim();
    const headings = Array.from(document.querySelectorAll('.hx-markdown h1, .hx-markdown h2, .hx-markdown h3, .hx-markdown h4, .hx-markdown h5, .hx-markdown h6, .hx-tiptap-editor h1, .hx-tiptap-editor h2, .hx-tiptap-editor h3, .hx-tiptap-editor h4, .hx-tiptap-editor h5, .hx-tiptap-editor h6'));
    const target = headings.find(h => h.textContent === cleanText);
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  };

  // 面板树渲染
  const renderDocNode = (doc: HuanxingDocumentResult, depth: number) => {
    return (
      <div
        key={`doc-${doc.id}`}
        onClick={() => handleSelectDoc(doc)}
        style={{ paddingLeft: `${depth * 14 + 12}px` }}
        className={`group flex items-center justify-between pr-2 py-1.5 rounded-hx-radius-sm cursor-pointer transition-colors select-none ${
          selectedDoc?.id === doc.id
            ? 'bg-hx-purple/10 text-hx-text-primary font-medium'
            : 'hover:bg-hx-bg-hover text-hx-text-secondary hover:text-hx-text-primary'
        }`}
      >
        <div className="flex items-center gap-2 truncate">
          <FileText size={14} className={selectedDoc?.id === doc.id ? 'text-hx-purple' : 'opacity-60'} />
          <span className="truncate text-[13px]">{doc.title || '未命名'}</span>
        </div>
        <div className="flex gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
          {doc.is_public && <Globe size={12} className="text-hx-blue mr-1" />}
          <button
            onClick={(e) => {
              e.stopPropagation();
              setMoveDocTarget(doc);
              setMoveTargetFolderId(doc.folder_id || null);
            }}
            className="p-1 -mr-1 rounded bg-transparent text-hx-text-tertiary hover:bg-hx-purple/10 hover:text-hx-purple transition-colors border-none cursor-pointer"
            title="移动到目录"
          >
            <ArrowRightLeft size={12} />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setDeleteDocTarget(doc);
            }}
            className="p-1 -mr-1 rounded bg-transparent text-hx-text-tertiary hover:bg-red-500 hover:text-white transition-colors border-none cursor-pointer"
            title="删除文档"
          >
            <Trash2 size={12} />
          </button>
        </div>
      </div>
    );
  };

  const renderTree = (folders: HuanxingFolderTreeNode[], depth: number): any => {
    return folders.map(folder => {
      const isExpanded = expandedFolders.has(folder.id) || searchQuery !== '';
      const folderDocs = filteredDocs.filter(d => d.folder_id === folder.id);
      
      // 在搜索的时候，只展示有内容的文件夹
      if (searchQuery && folderDocs.length === 0 && (!folder.children || folder.children.length === 0)) return null;

      return (
        <div key={`folder-${folder.id}`} className="flex flex-col">
          <div 
             className="group flex items-center justify-between pr-2 py-1.5 hover:bg-hx-bg-hover rounded-hx-radius-sm cursor-pointer select-none"
             style={{ paddingLeft: `${depth * 14 + 2}px` }}
             onClick={() => toggleFolder(folder.id)}
          >
             <div className="flex items-center gap-1 text-hx-text-primary text-[13px] font-medium truncate">
               <div className="w-5 flex items-center justify-center shrink-0">
                 {isExpanded ? <ChevronDown size={14} className="opacity-60" /> : <ChevronRight size={14} className="opacity-60" />}
               </div>
               <Folder size={14} className="text-hx-text-tertiary fill-hx-text-tertiary/20" />
               <span className="truncate ml-1">{folder.name}</span>
             </div>
             
             <div className="opacity-0 group-hover:opacity-100 flex items-center shrink-0">
               <button onClick={(e) => { e.stopPropagation(); setFolderCreateTarget({ parentId: folder.id }); }} className="p-1 bg-transparent border-none text-hx-text-tertiary hover:text-hx-text-primary cursor-pointer rounded hover:bg-black/5 dark:hover:bg-white/10" title="新建子目录">
                 <FolderPlus size={14} />
               </button>
               <button onClick={(e) => { e.stopPropagation(); handleCreateNew(folder.id); }}
                 className="p-1 bg-transparent border-none text-hx-text-tertiary hover:text-hx-text-primary cursor-pointer rounded hover:bg-black/5 dark:hover:bg-white/10" title="在此目录新建文档">
                 <FilePlus size={14} />
               </button>
               <button onClick={(e) => { e.stopPropagation(); setDeleteFolderTarget({ id: folder.id, name: folder.name }); }} className="p-1 bg-transparent border-none text-hx-text-tertiary hover:text-red-500 cursor-pointer rounded hover:bg-red-500/10" title="删除当前目录及子内容">
                 <Trash2 size={14} />
               </button>
             </div>
          </div>
          
          {isExpanded && (
             <div className="flex flex-col mt-[2px]">
               {folder.children && folder.children.length > 0 && renderTree(folder.children, depth + 1)}
               {folderDocs.map(doc => renderDocNode(doc, depth + 1))}
               {!searchQuery && (!folder.children || folder.children.length === 0) && folderDocs.length === 0 && (
                 <div className="px-2 py-1 text-[11px] text-hx-text-tertiary opacity-50 select-none" style={{ paddingLeft: `${(depth + 1) * 14 + 28}px` }}>此目录为空</div>
               )}
             </div>
          )}
        </div>
      );
    });
  };

  return (
    <div className="flex flex-1 h-full bg-hx-bg-main overflow-hidden text-hx-text-primary">
      {/* 左侧面板 */}
      <div className="hx-panel w-[300px] shrink-0 border-r border-hx-border flex flex-col bg-hx-bg-panel z-10">
        <div className="hx-panel-header shrink-0 p-4 border-b border-hx-border" data-tauri-drag-region="true">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-[16px] font-semibold m-0 flex items-center gap-2">
              <BookOpen size={18} className="text-hx-purple" />
              知识百科
            </h2>
            <div className="flex items-center gap-1">
              <button
                className="w-7 h-7 flex items-center justify-center rounded-hx-radius-sm bg-transparent text-hx-text-secondary hover:bg-hx-bg-hover hover:text-hx-text-primary transition-colors cursor-pointer border-none"
                title="新建主目录"
                onClick={() => setFolderCreateTarget({ parentId: null })}
              >
                <FolderPlus size={16} />
              </button>
              <button
                className="w-7 h-7 flex items-center justify-center rounded-hx-radius-sm bg-hx-purple/10 text-hx-purple hover:bg-hx-purple hover:text-white transition-colors cursor-pointer border-none"
                title="新建独立文档"
                onClick={() => handleCreateNew(null)}
              >
                <Plus size={16} />
              </button>
            </div>
          </div>
          <div className="hx-panel-search">
            <Search size={16} />
            <Input
              type="text"
              placeholder="搜索文档快照..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto overflow-x-hidden p-2 flex flex-col gap-[2px]">
          {loading && folderTree.length === 0 && documents.length === 0 ? (
            <div className="py-[60px] flex flex-col items-center justify-center text-hx-text-tertiary">
              <Loader2 size={24} className="animate-spin opacity-50 mb-2" />
              <p className="text-[13px] m-0">组织目录中...</p>
            </div>
          ) : (
            <>
              {renderTree(folderTree, 0)}
              {filteredDocs.filter(d => !d.folder_id).map(doc => renderDocNode(doc, 0))}
              {folderTree.length === 0 && filteredDocs.length === 0 && !loading && (
                 <div className="py-[60px] flex flex-col items-center justify-center text-hx-text-tertiary">
                   <FileText size={40} className="opacity-30 mb-2" />
                   <p className="text-[13px] m-0">{searchQuery ? '未检索到内容' : '一纸空白，点右上角新建吧'}</p>
                 </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* 右侧主区 */}
      <div className="flex-1 flex flex-col min-w-0 bg-hx-bg-main relative">
        {(selectedDoc || (!selectedDoc && editorTitle === '未命名文档')) ? (
          <div className="flex flex-col h-full w-full">
            {/* Header 控制栏 */}
            <div data-tauri-drag-region="true" className="h-14 shrink-0 px-6 border-b border-hx-border flex items-center justify-between bg-hx-bg-panel/50 z-10 w-full">
              {isEditing ? (
                 <Input
                 type="text"
                 value={editorTitle}
                 onChange={(e) => setEditorTitle(e.target.value)}
                 id="title-input-box"
                 placeholder="在此键入文档标题"
                 className="flex-1 text-lg font-bold !h-auto py-1.5"
                 autoFocus
               />
              ) : (
                 <div data-tauri-drag-region="true" className="flex items-center h-full text-lg font-bold text-hx-text-primary flex-1 truncate mr-4">
                   {editorTitle}
                 </div>
              )}
             
              <div className="flex items-center gap-3 shrink-0">
                {selectedDoc && (
                   <button
                     onClick={() => setShowShareModal(true)}
                     className="px-3 py-1.5 rounded-hx-radius-sm border border-hx-border bg-transparent text-hx-text-secondary text-[13px] font-medium cursor-pointer hover:bg-hx-bg-hover hover:text-hx-text-primary transition-colors flex items-center gap-1.5"
                   >
                     <Share2 size={14} /> 分享
                   </button>
                )}
                
                <button
                  onClick={() => setIsEditing(!isEditing)}
                  className={`px-3 py-1.5 rounded-hx-radius-sm border-none text-[13px] font-medium cursor-pointer transition-colors flex items-center gap-1.5 ${isEditing ? 'bg-hx-bg-hover text-hx-text-secondary' : 'bg-hx-purple/10 text-hx-purple'}`}
                >
                  {isEditing ? <><BookOpen size={14} /> 预览模式</> : <><Edit3 size={14} /> 进入编辑</>}
                </button>
                
                {isEditing && (
                  <button
                    onClick={handleSave}
                    disabled={isSaving || !editorTitle.trim()}
                    className="px-4 py-1.5 rounded-hx-radius-sm border-none bg-hx-purple text-white text-[13px] font-medium cursor-pointer shadow-sm disabled:opacity-50 disabled:cursor-not-allowed hover:opacity-90 transition-opacity flex items-center gap-1.5"
                  >
                    {isSaving ? <Loader2 size={16} className="animate-spin" /> : <Save size={16} />} 
                    保存内容
                  </button>
                )}
              </div>
            </div>
            
            {/* 布局区：正文 + TOC */}
            <div className="flex-1 overflow-hidden flex flex-row">
               {/* 左：正文预览或编辑 */}
               <div className="flex-1 min-w-0 flex flex-col h-full border-r border-hx-border">
                  {isEditing ? (
                    <TipTapEditor
                      value={editorContent}
                      onChange={setEditorContent}
                    />
                  ) : (
                    <MarkdownPreview content={editorContent} />
                  )}
               </div>

               {/* 右：TOC 大纲 */}
               {tocNodes.length > 0 && (
                 <div className="w-[240px] shrink-0 bg-hx-bg-panel flex flex-col p-4 shadow-[-4px_0_12px_rgba(0,0,0,0.02)] z-0">
                    <span className="text-[12px] font-bold text-hx-text-tertiary tracking-wider mb-4 uppercase">此页目录大纲</span>
                    <div className="flex flex-col gap-2 overflow-y-auto pr-2">
                       {tocNodes.map(node => (
                         <button 
                           key={node.id}
                           onClick={() => scrollToHeading(node.text)}
                           title={node.text}
                           className={`text-left text-[13px] truncate transition-colors bg-transparent border-none outline-none cursor-pointer text-hx-text-secondary hover:text-hx-purple
                            ${node.level === 1 ? 'font-semibold text-hx-text-primary mt-1' : ''}
                            ${node.level === 2 ? 'pl-3' : ''}
                            ${node.level === 3 ? 'pl-6 text-[12px]' : ''}
                           ${node.level > 3 ? 'pl-8 text-[11px] text-hx-text-tertiary' : ''}
                           `}
                         >
                           {node.text.replace(/[#*`_]+/g, '')}
                         </button>
                       ))}
                    </div>
                 </div>
               )}
            </div>
          </div>
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-hx-text-tertiary">
            <div className="w-16 h-16 rounded-full bg-hx-purple/10 flex items-center justify-center mb-4">
               <BookOpen size={32} className="text-hx-purple" />
            </div>
            <h3 className="text-[15px] font-semibold text-hx-text-primary mt-0 mb-2">欢迎来到知识百科</h3>
            <p className="text-[13px] text-hx-text-secondary m-0">在左侧构筑结构体系，让沉淀的思想生根发芽</p>
          </div>
        )}
      </div>

      {/* Share Dialog */}
      <Dialog open={showShareModal} onOpenChange={setShowShareModal}>
        <DialogContent className="sm:max-w-[480px]">
          <DialogHeader>
            <DialogTitle>文档共享设置</DialogTitle>
            <DialogDescription>获取文档外部链接，分享给团队、互联网的任何用户，或者 Agent 大模型代理引擎使用。</DialogDescription>
          </DialogHeader>

          {selectedDoc && (
            <div className="bg-hx-bg-main p-4 rounded-hx-radius-md border border-hx-border flex flex-col gap-4 mt-2">
              <div className="flex items-center justify-between border-b border-hx-border pb-4">
                <div className="flex items-center gap-3">
                  <div className={`w-10 h-10 rounded-full flex items-center justify-center ${selectedDoc.is_public ? 'bg-hx-blue/20 text-hx-blue' : 'bg-hx-bg-hover text-hx-text-secondary'}`}>
                    {selectedDoc.is_public ? <Globe size={18}/> : <Lock size={18}/>}
                  </div>
                  <div>
                    <div className="font-semibold text-[14px] text-hx-text-primary">{selectedDoc.is_public ? '已开启公开链接' : '当前为私密状态'}</div>
                    <div className="text-[12px] text-hx-text-tertiary">{selectedDoc.is_public ? '任何获得此链接的人/机器均可查看' : '仅在您的桌面客户端可见'}</div>
                  </div>
                </div>
                <button 
                  onClick={handleToggleShare}
                  className={`px-4 py-1.5 rounded-full text-[13px] font-semibold border-none cursor-pointer transition-colors ${selectedDoc.is_public ? 'bg-hx-bg-hover text-hx-text-secondary' : 'bg-hx-blue text-white'}`}
                >
                  {selectedDoc.is_public ? '关闭共享' : '开启共享'}
                </button>
              </div>

              {selectedDoc.is_public && selectedDoc.share_token && (
                <div className="flex flex-col gap-2">
                  <span className="text-[12px] font-medium text-hx-text-secondary">外部访问链接</span>
                  <div className="flex border border-hx-border rounded-hx-radius-sm bg-hx-bg-panel overflow-hidden">
                    <Input 
                      type="text" 
                      readOnly 
                      value={`https://huanxing.cloud/docs/share/${selectedDoc.share_token}`} 
                      className="flex-1 bg-transparent border-none shadow-none focus:ring-0"
                    />
                    <button 
                      className="px-3 border-l border-hx-border bg-hx-bg-hover hover:bg-hx-purple/10 text-hx-text-secondary hover:text-hx-purple cursor-pointer transition-colors"
                      onClick={() => {
                        navigator.clipboard.writeText(`https://huanxing.cloud/docs/share/${selectedDoc.share_token}`);
                        alert('已复制分享链接！');
                      }}
                    >
                      <Copy size={16} />
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}
        </DialogContent>
      </Dialog>

      {/* Delete Doc Confirmation */}
      <AlertDialog open={!!deleteDocTarget} onOpenChange={(open) => !open && setDeleteDocTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确定要删除此文档吗？</AlertDialogTitle>
            <AlertDialogDescription>
              文档 "{deleteDocTarget?.title}" 将会被永久删除。此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={confirmDeleteDoc} className="bg-red-500 hover:bg-red-600 text-white border-none">
              确定删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Delete Folder Confirmation */}
      <AlertDialog open={!!deleteFolderTarget} onOpenChange={(open) => !open && setDeleteFolderTarget(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确定要删除整个目录吗？</AlertDialogTitle>
            <AlertDialogDescription>
              目录 "{deleteFolderTarget?.name}" 及其下属的所有文档和子目录都将被永久移除。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={confirmDeleteFolder} className="bg-red-500 hover:bg-red-600 text-white border-none">
              确认强力删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Create Folder Dialog */}
      <Dialog open={!!folderCreateTarget} onOpenChange={(open) => !open && setFolderCreateTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{folderCreateTarget?.parentId ? '新建子目录' : '新建根目录'}</DialogTitle>
            <DialogDescription>为您的知识库添加一个新的分类层级。</DialogDescription>
          </DialogHeader>
          <div className="py-4">
            <Input
              type="text"
              autoFocus
              placeholder="请输入目录名称..."
              value={newFolderName}
              onChange={(e) => setNewFolderName(e.target.value)}
              className="w-full"
              onKeyDown={(e) => e.key === 'Enter' && confirmCreateFolder()}
            />
          </div>
          <DialogFooter>
            <button onClick={() => setFolderCreateTarget(null)} className="hx-btn hx-btn-outline">取消</button>
            <button onClick={confirmCreateFolder} disabled={!newFolderName.trim()} className="hx-btn hx-btn-primary">创建目录</button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Move Document Dialog */}
      <Dialog open={!!moveDocTarget} onOpenChange={(open) => !open && setMoveDocTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>移动文档</DialogTitle>
            <DialogDescription>将文档 "{moveDocTarget?.title}" 移动到指定的目录中。</DialogDescription>
          </DialogHeader>
          <div className="py-4">
             <div className="flex flex-col gap-3">
                <label className="text-[12px] font-medium text-hx-text-secondary">请选择目标目录</label>
                <FolderTreeSelect 
                  tree={folderTree}
                  selectedId={moveTargetFolderId}
                  onSelect={setMoveTargetFolderId}
                />
             </div>
          </div>
          <DialogFooter>
            <button onClick={() => setMoveDocTarget(null)} className="hx-btn hx-btn-outline">取消</button>
            <button onClick={confirmMoveDocument} className="hx-btn hx-btn-primary">确认移动</button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

    </div>
  );
}
