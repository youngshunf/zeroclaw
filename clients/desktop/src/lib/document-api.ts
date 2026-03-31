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
