import { authRequest } from './huanxing-api';

// ==================== 类型定义 ====================

export interface HuanxingDocumentResult {
  id: number;
  uuid: string;
  user_id: number;
  title: string;
  content?: string;
  summary?: string;
  tags?: string;
  folder_id?: number | null;
  word_count: number;
  status: string;
  is_public: boolean;
  created_by: string;
  agent_id?: string;
  share_token?: string;
  share_password?: string;
  share_permission?: string;
  share_expires_at?: string;
  current_version: number;
  created_at: string;
  updated_at?: string;
  deleted_at?: string;
  is_shared?: boolean;
}

export interface HuanxingDocumentParams {
  uuid?: string;
  user_id?: number;
  title?: string;
  status?: string;
  agent_id?: string;
  page?: number;
  size?: number;
}

export interface HuanxingDocumentCreateParams {
  uuid?: string;
  title: string;
  content?: string;
  summary?: string;
  tags?: string;
  folder_id?: number | null;
  word_count?: number;
  status?: string;
  is_public?: boolean;
  created_by?: string;
  agent_id?: string;
  share_token?: string;
  share_password?: string;
  share_permission?: string;
  share_expires_at?: string;
  current_version?: number;
}

export interface HuanxingDocumentUpdateParams {
  uuid?: string;
  user_id?: number;
  title?: string;
  content?: string;
  summary?: string;
  tags?: string;
  word_count?: number;
  status?: string;
  is_public?: boolean;
  created_by?: string;
  agent_id?: string;
  share_token?: string;
  share_password?: string;
  share_permission?: string;
  share_expires_at?: string;
  current_version?: number;
}

export interface HuanxingFolderTreeNode {
  id: number;
  uuid: string;
  name: string;
  icon?: string;
  parent_id?: number | null;
  sort_order: number;
  doc_count: number;
  children: HuanxingFolderTreeNode[];
}

// ==================== API ====================

/** 获取文档列表 */
export async function getHuanxingDocumentListApi(
  token: string,
  params?: HuanxingDocumentParams,
): Promise<{ data: HuanxingDocumentResult[]; total: number }> {
  // 转换 params 为 querystring
  const url = new URL('/api/v1/huanxing/app/docs', 'http://localhost');
  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        url.searchParams.append(key, String(value));
      }
    });
  }
  return authRequest(url.pathname + url.search, token);
}

/** 获取文档详情 */
export async function getHuanxingDocumentApi(token: string, pk: number): Promise<{ data: HuanxingDocumentResult }> {
  return authRequest(`/api/v1/huanxing/app/docs/${pk}`, token);
}

/** 创建文档 */
export async function createHuanxingDocumentApi(
  token: string,
  data: HuanxingDocumentCreateParams,
): Promise<{ data: HuanxingDocumentResult }> {
  return authRequest('/api/v1/huanxing/app/docs', token, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

/** 更新文档 */
export async function updateHuanxingDocumentApi(
  token: string,
  pk: number,
  data: Partial<HuanxingDocumentUpdateParams>,
): Promise<{ data: HuanxingDocumentResult }> {
  return authRequest(`/api/v1/huanxing/app/docs/${pk}`, token, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

/** 删除文档 */
export async function deleteHuanxingDocumentApi(token: string, pk: number): Promise<void> {
  return authRequest(`/api/v1/huanxing/app/docs/${pk}`, token, {
    method: 'DELETE',
  });
}

/** 获取目录树 */
export async function getHuanxingFolderTreeApi(token: string): Promise<{ data: HuanxingFolderTreeNode[] }> {
  return authRequest('/api/v1/huanxing/app/folders', token);
}

/** 创建目录 */
export async function createHuanxingFolderApi(
  token: string,
  data: { name: string; parent_id?: number | null }
): Promise<{ data: HuanxingFolderTreeNode }> {
  return authRequest('/api/v1/huanxing/app/folders', token, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

/** 删除目录 */
export async function deleteHuanxingFolderApi(token: string, pk: number): Promise<void> {
  return authRequest(`/api/v1/huanxing/app/folders/${pk}?recursive=true`, token, {
    method: 'DELETE',
  });
}

/** 移动文档到其他目录 */
export async function moveHuanxingDocumentApi(
  token: string,
  pk: number,
  targetFolderId: number | null
): Promise<{ data: any }> {
  return authRequest(`/api/v1/huanxing/app/docs/${pk}/move`, token, {
    method: 'POST',
    body: JSON.stringify({ target_folder_id: targetFolderId }),
  });
}

/** 生成/更新分享链接 */
export async function createShareLinkApi(
  token: string,
  pk: number,
  options?: {
    permission?: 'view' | 'edit';
    expires_hours?: number;
    password?: string;
  },
): Promise<{ data: { share_url: string } }> {
  const url = new URL(`/api/v1/huanxing/app/docs/${pk}/share`, 'http://localhost');
  url.searchParams.set('permission', options?.permission || 'view');
  url.searchParams.set('expires_hours', String(options?.expires_hours || 72));
  if (options?.password) {
    url.searchParams.set('password', options.password);
  }
  return authRequest(url.pathname + url.search, token, {
    method: 'POST',
  });
}

/** 取消分享 */
export async function cancelShareLinkApi(
  token: string,
  pk: number,
): Promise<void> {
  return authRequest(`/api/v1/huanxing/app/docs/${pk}/share`, token, {
    method: 'DELETE',
  });
}

/** 导出文档 (返回 Blob 和 filename) */
export async function exportHuanxingDocumentApi(
  token: string,
  pk: number,
  format: 'markdown' | 'pdf' | 'docx' = 'markdown'
): Promise<{ blob: Blob; filename: string }> {
  // baseUrl() 的获取逻辑
  const isDesktop =
    typeof window !== 'undefined' &&
    (!!((window as any).__TAURI_INTERNALS__) || !!((window as any).__TAURI__));
  const { HUANXING_CONFIG } = await import('../config');
  const base = isDesktop ? HUANXING_CONFIG.backendBaseUrl : '';
  
  const response = await fetch(`${base}/api/v1/huanxing/app/docs/${pk}/export?format=${format}`, {
    method: 'GET',
    headers: {
      Authorization: `Bearer ${token}`
    }
  });

  if (!response.ok) {
    throw new Error(`导出失败 (${response.status})`);
  }

  const blob = await response.blob();
  
  // 尝试从 Content-Disposition 提取文件名
  let filename = `huanxing_document.${format === 'markdown' ? 'md' : format}`;
  const disposition = response.headers.get('Content-Disposition');
  if (disposition && disposition.includes('filename*=')) {
    // 处理 filename*=UTF-8''...
    const match = disposition.match(/filename\*=UTF-8''([^;]+)/i);
    if (match && match[1]) {
      try {
        filename = decodeURIComponent(match[1]);
      } catch (e) {
        // 解码失败用默认
      }
    }
  } else if (disposition && disposition.includes('filename=')) {
    const match = disposition.match(/filename="?([^";]+)"?/i);
    if (match && match[1]) {
      filename = match[1];
    }
  }

  return { blob, filename };
}

/** 获取分享文档预览元数据 (公开接口) */
export async function getHuanxingSharedDocumentApi(
  token: string,
  shareToken: string,
  password?: string
): Promise<{ data: HuanxingDocumentResult }> {
  // 即使是 open 接口，也可携带 token (不强求)
  const url = new URL(`/api/v1/huanxing/open/share/${shareToken}`, 'http://localhost');
  if (password) {
    url.searchParams.set('password', password);
  }
  return authRequest(url.pathname + url.search, token, {
    method: 'GET',
  });
}

/** 入库/保存他人的分享链接到自己的空间 */
export async function saveHuanxingSharedDocumentApi(
  token: string,
  shareToken: string,
  folderId?: number
): Promise<{ data: any }> {
  return authRequest(`/api/v1/huanxing/app/docs/share/save`, token, {
    method: 'POST',
    body: JSON.stringify({
      share_token: shareToken,
      folder_id: folderId ?? null
    }),
  });
}
