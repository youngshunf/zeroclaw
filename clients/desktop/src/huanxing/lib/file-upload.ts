/**
 * file-upload — 文件上传工具
 *
 * 桌面端文件上传策略：
 * Agent 运行在本地，因此用户拖入的文件会被复制到 Agent 工作区的 files/ 目录下，
 * 然后在消息中附上文件路径，Agent 可以直接通过 file_read 等工具来处理。
 *
 * 目录结构：
 *   ~/.huanxing/agents/{agent}/files/
 *   └── 2026-03-29_abc123_image.png
 */

/** 上传后的文件信息 */
export interface UploadedFile {
  /** 工作区内的相对路径 */
  relativePath: string;
  /** 绝对路径 */
  absolutePath: string;
  /** 原始文件名 */
  originalName: string;
  /** MIME 类型 */
  mimeType: string;
  /** 文件大小（字节） */
  size: number;
  /** 是否是图片 */
  isImage: boolean;
  /** 浏览器内存中的 Blob URL（用于图片预览，仅粘贴/选择的文件有值） */
  blobUrl?: string;
}

// 支持的图片 MIME
const IMAGE_MIMES = new Set([
  'image/png', 'image/jpeg', 'image/gif', 'image/webp',
  'image/svg+xml', 'image/bmp', 'image/tiff',
]);

// 支持的文件扩展名 → MIME 映射
const EXT_MIME_MAP: Record<string, string> = {
  '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
  '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml',
  '.bmp': 'image/bmp', '.tiff': 'image/tiff', '.tif': 'image/tiff',
  '.pdf': 'application/pdf',
  '.txt': 'text/plain', '.md': 'text/markdown',
  '.json': 'application/json', '.toml': 'application/toml',
  '.yaml': 'text/yaml', '.yml': 'text/yaml',
  '.csv': 'text/csv', '.tsv': 'text/tab-separated-values',
  '.py': 'text/x-python', '.rs': 'text/x-rust', '.ts': 'text/typescript',
  '.tsx': 'text/typescript', '.js': 'text/javascript', '.jsx': 'text/javascript',
  '.html': 'text/html', '.css': 'text/css',
  '.zip': 'application/zip', '.tar': 'application/x-tar',
  '.gz': 'application/gzip',
};

function getMimeType(filename: string): string {
  const ext = filename.substring(filename.lastIndexOf('.')).toLowerCase();
  return EXT_MIME_MAP[ext] || 'application/octet-stream';
}

function isImageMime(mime: string): boolean {
  return IMAGE_MIMES.has(mime);
}

/** 生成唯一文件名（带时间戳前缀） */
function generateUniqueFilename(originalName: string): string {
  const date = new Date().toISOString().slice(0, 10); // 2026-03-29
  const rand = Math.random().toString(36).slice(2, 8);
  // 清理文件名中的特殊字符
  const safeName = originalName.replace(/[^a-zA-Z0-9._\u4e00-\u9fff-]/g, '_');
  return `${date}_${rand}_${safeName}`;
}

// ---------------------------------------------------------------------------
// Tauri Backend Integration
// ---------------------------------------------------------------------------

/** 获取 Tauri invoke 函数 */
function getTauriInvoke(): ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null {
  const internals = (window as any).__TAURI_INTERNALS__;
  return internals?.invoke ?? null;
}

/** 获取当前 Agent 的 workspace 目录 */
export async function getWorkspaceDir(): Promise<string | null> {
  const invoke = getTauriInvoke();
  if (!invoke) {
    // 开发模式 fallback
    return null;
  }
  try {
    return await invoke('get_workspace_dir') as string;
  } catch {
    // 如果 Tauri 命令不存在，用 config_dir 推导
    try {
      const configDir = await invoke('get_config_dir') as string;
      return `${configDir}/agents/default`;
    } catch {
      return null;
    }
  }
}

/**
 * 复制文件到 Agent 工作区 files/ 目录
 *
 * 使用 Tauri 后端的 invoke 来完成文件系统操作。
 * 如果 Tauri 不可用（开发模式），返回原始文件路径。
 */
export async function copyFileToWorkspace(
  file: File,
  workspaceDir?: string,
): Promise<UploadedFile> {
  const mime = file.type || getMimeType(file.name);
  const isImage = isImageMime(mime);
  const uniqueName = generateUniqueFilename(file.name);

  const invoke = getTauriInvoke();

  if (invoke && workspaceDir) {
    // Tauri 环境：通过后端复制文件
    // 读取 File 为 ArrayBuffer → base64
    const buffer = await file.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    const base64 = uint8ArrayToBase64(bytes);

    const filesDir = `${workspaceDir}/files`;
    const destPath = `${filesDir}/${uniqueName}`;

    try {
      await invoke('copy_file_to_workspace', {
        base64Data: base64,
        destPath: destPath,
      });
    } catch (err) {
      console.warn('[file-upload] copy_file_to_workspace failed, trying REST fallback:', err);
      // REST fallback — 通过 sidecar API 上传
      await uploadViaRest(file, uniqueName);
    }

    return {
      relativePath: `files/${uniqueName}`,
      absolutePath: destPath,
      originalName: file.name,
      mimeType: mime,
      size: file.size,
      isImage,
      blobUrl: isImage ? URL.createObjectURL(file) : undefined,
    };
  }

  // 非 Tauri 环境（开发模式）：通过 sidecar REST API 上传
  const destPath = await uploadViaRest(file, uniqueName);
  return {
    relativePath: `files/${uniqueName}`,
    absolutePath: destPath,
    originalName: file.name,
    mimeType: mime,
    size: file.size,
    isImage,
    blobUrl: isImage ? URL.createObjectURL(file) : undefined,
  };
}

/**
 * 通过 sidecar REST API 上传文件
 * POST /api/upload (multipart/form-data)
 * 返回服务端保存的绝对路径
 */
async function uploadViaRest(file: File, uniqueName: string): Promise<string> {
  const formData = new FormData();
  formData.append('file', file, uniqueName);

  const { getToken } = await import('@/lib/auth');
  const token = getToken();

  const resp = await fetch('/api/upload', {
    method: 'POST',
    headers: token ? { 'Authorization': `Bearer ${token}` } : {},
    body: formData,
  });

  if (!resp.ok) {
    throw new Error(`Upload failed (${resp.status}): ${await resp.text().catch(() => '')}`);
  }

  const data = await resp.json();
  return data.path || `/tmp/huanxing-dev/files/${uniqueName}`;
}

/**
 * 从拖拽事件中提取文件路径（macOS Tauri 特有）
 *
 * 在 Tauri 桌面端，拖拽文件可以直接获取文件系统路径。
 * 此时不需要复制文件，直接使用原始路径即可。
 */
export function extractDroppedFilePaths(dataTransfer: DataTransfer): string[] {
  const paths: string[] = [];

  // 尝试读取 Tauri 提供的文件路径
  for (let i = 0; i < dataTransfer.files.length; i++) {
    const file = dataTransfer.files[i];
    // Tauri 会在 File 对象上附加 path 属性
    const filePath = (file as any).path;
    if (filePath && typeof filePath === 'string') {
      paths.push(filePath);
    }
  }

  return paths;
}

/**
 * 处理拖拽或选择的文件
 *
 * 策略：
 * 1. 如果文件有 Tauri path → 直接使用原始路径（不复制）
 * 2. 否则 → 复制到 workspace/files/ 目录
 *
 * @returns 文件信息列表
 */
export async function handleFiles(
  files: FileList | File[],
  workspaceDir?: string,
): Promise<UploadedFile[]> {
  const results: UploadedFile[] = [];
  const fileArray = Array.from(files);

  for (const file of fileArray) {
    const tauriPath = (file as any).path;

    if (tauriPath && typeof tauriPath === 'string') {
      // Tauri 环境下拖拽文件，直接使用原始路径
      const mime = file.type || getMimeType(file.name);
      results.push({
        relativePath: tauriPath, // 对于本地路径就是绝对路径
        absolutePath: tauriPath,
        originalName: file.name,
        mimeType: mime,
        size: file.size,
        isImage: isImageMime(mime),
      });
    } else {
      // 浏览器 File API，需要复制到 workspace
      const uploaded = await copyFileToWorkspace(file, workspaceDir);
      results.push(uploaded);
    }
  }

  return results;
}

/**
 * 将上传的文件信息格式化为消息前缀
 *
 * 图片使用 ZeroClaw multimodal 标记格式：[IMAGE:/path/to/image.png]
 * 非图片文件提示 Agent 使用 file_read 工具读取
 */
export function formatFileReferences(files: UploadedFile[]): string {
  if (files.length === 0) return '';

  return files
    .map(f => {
      if (f.isImage) {
        // ZeroClaw multimodal.rs 解析 [IMAGE:path] 标记
        // 支持本地路径、HTTP URL、data: URI
        return `[IMAGE:${f.absolutePath}]`;
      }
      // 非图片文件：提示 Agent 使用工具读取
      return `[附件: ${f.absolutePath}] (请使用 file_read 工具读取此文件)`;
    })
    .join('\n') + '\n';
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function uint8ArrayToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/**
 * 打开原生文件选择对话框
 */
export async function openFileDialog(options?: {
  multiple?: boolean;
  accept?: string;
}): Promise<File[]> {
  const invoke = getTauriInvoke();

  if (invoke) {
    // Tauri 环境：尝试使用原生文件对话框
    // 通过 __TAURI_INTERNALS__ 直接调用，避免 Rollup 解析 @tauri-apps/plugin-dialog
    try {
      const internals = (window as any).__TAURI_INTERNALS__;
      if (internals?.invoke) {
        // 调用 Tauri dialog plugin 的 open 命令
        const selected = await internals.invoke('plugin:dialog|open', {
          multiple: options?.multiple ?? true,
          directory: false,
        });

        if (!selected) return [];

        const paths: string[] = Array.isArray(selected) ? selected : [selected];

        // 将路径转为伪 File 对象（Tauri 特有，带 path 属性）
        return paths.map(p => {
          const name = p.split('/').pop() || 'file';
          const mime = getMimeType(name);
          const f = new File([], name, { type: mime });
          (f as any).path = p;
          return f;
        });
      }
    } catch {
      // Tauri dialog plugin 不可用，降级到 HTML input
    }
  }

  // 降级：使用隐藏的 <input type="file">
  return new Promise(resolve => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = options?.multiple ?? true;
    if (options?.accept) input.accept = options.accept;
    input.style.display = 'none';
    document.body.appendChild(input);

    input.addEventListener('change', () => {
      const files = input.files ? Array.from(input.files) : [];
      document.body.removeChild(input);
      resolve(files);
    });

    input.addEventListener('cancel', () => {
      document.body.removeChild(input);
      resolve([]);
    });

    input.click();
  });
}
