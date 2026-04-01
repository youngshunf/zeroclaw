/**
 * 唤星桌面端 onboard 流程
 *
 * 登录成功后，为用户初始化 AI 引擎环境：
 *
 * Tauri 桌面端模式:
 *  1. 调用 Tauri IPC `onboard_zeroclaw`
 *  2. 后端创建 ~/.huanxing/ 配置目录
 *  3. 从模板生成 config.toml（注入 LLM token）
 *  4. 创建默认 agent（小星）
 *  5. 启动 sidecar 进程
 *
 * Web 开发模式 (fallback):
 *  1. 读取 sidecar 当前配置
 *  2. 注入 LLM provider（唤星 LLM 网关 + llm_token）
 *  3. 写回配置
 */

import { HUANXING_CONFIG, type HuanxingSession } from './config';

/** Onboard 结果 */
export interface OnboardResult {
  success: boolean;
  error?: string;
  configUpdated?: boolean;
  workspaceReady?: boolean;
  config_created?: boolean;
  agent_created?: boolean;
  sidecar_started?: boolean;
}

/** HASN 注册结果 */
export interface HasnIdentity {
  hasn_id: string;
  star_id: string;
  name: string;
  agent_hasn_id?: string;
  agent_star_id?: string;
  already_exists: boolean;
}

// ── Tauri IPC 检测 ──────────────────────────────────────

function isTauri(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;
}

/**
 * 获取 HASN API 基地址
 * - Tauri 生产模式（tauri:// 协议）：直接访问后端
 * - Dev 模式 / Web 模式：走 Vite 代理（相对路径）
 */
function hasnApiUrl(path: string): string {
  const protocol = window.location.protocol;
  // Tauri 生产打包：协议是 tauri:// 或 https://tauri.localhost
  if (protocol === 'tauri:' || protocol === 'https:' && window.location.hostname === 'tauri.localhost') {
    return `${HUANXING_CONFIG.backendBaseUrl}${path}`;
  }
  // Dev 模式 (http://localhost:1420) 和 Web 模式：走代理
  return path;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // 使用 Tauri 内部 API 直接调用，避免 @tauri-apps/api 依赖问题
  const internals = (window as any).__TAURI_INTERNALS__;
  if (!internals?.invoke) {
    throw new Error('Tauri internals not available');
  }
  return internals.invoke(cmd, args) as Promise<T>;
}

/**
 * 执行自动 onboard
 *
 * @param session 登录后的唤星会话数据
 * @returns OnboardResult
 */
export async function autoOnboard(session: HuanxingSession): Promise<OnboardResult> {
  // Tauri 模式：通过 IPC 让后端完成全部流程
  if (isTauri()) {
    return tauriOnboard(session);
  }

  // Web 开发模式 fallback：通过 HTTP 直接配置已有 sidecar
  return webOnboard(session);
}

/**
 * Tauri 桌面端 onboard — 创建配置 + Agent + 启动 sidecar
 */
async function tauriOnboard(session: HuanxingSession): Promise<OnboardResult> {
  try {
    const result = await tauriInvoke<OnboardResult>('onboard_zeroclaw', {
      request: {
        llm_token: session.llmToken,
        user_nickname: session.user.nickname || null,
        user_uuid: session.user.uuid || null,
        user_phone: session.user.phone || null,
        agent_key: session.agentKey || null,
        api_base_url: HUANXING_CONFIG.backendBaseUrl,
        llm_gateway_url: HUANXING_CONFIG.llmGatewayV1,
      },
    });

    console.log('[huanxing-onboard] Tauri onboard result:', result);
    return {
      success: result.success,
      configUpdated: result.config_created,
      workspaceReady: result.agent_created,
      config_created: result.config_created,
      agent_created: result.agent_created,
      sidecar_started: result.sidecar_started,
      error: result.error ?? undefined,
    };
  } catch (err) {
    console.warn('[huanxing-onboard] Tauri onboard error:', err);
    return { success: false, error: String(err) };
  }
}

/**
 * Web 开发模式 onboard — 配置已有 sidecar
 */
async function webOnboard(session: HuanxingSession): Promise<OnboardResult> {
  try {
    // Step 1: 读取当前 sidecar 配置
    const configResp = await fetch('/api/config');
    if (!configResp.ok) {
      // sidecar 可能还没配置过，那就发一个完整的初始配置
      return await initializeConfig(session);
    }

    const currentConfig = await configResp.text();

    // Step 2: 检查是否已经配置了唤星 LLM
    if (currentConfig.includes(HUANXING_CONFIG.llmGatewayV1) &&
        currentConfig.includes(session.llmToken)) {
      // 已经配置过了，跳过
      return { success: true, configUpdated: false, workspaceReady: true };
    }

    // Step 3: 更新配置 — 替换 LLM provider
    const updatedConfig = patchConfig(currentConfig, session);

    // Step 4: 写回配置
    const putResp = await fetch('/api/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'text/plain' },
      body: updatedConfig,
    });

    if (!putResp.ok) {
      const err = await putResp.text();
      console.warn('[huanxing-onboard] config PUT failed:', err);
      return { success: true, configUpdated: false, error: err };
    }

    return { success: true, configUpdated: true, workspaceReady: true };
  } catch (err) {
    console.warn('[huanxing-onboard] onboard error:', err);
    return { success: true, configUpdated: false, error: String(err) };
  }
}

/**
 * TOML 配置补丁 — 替换 LLM provider 部分
 */
function patchConfig(toml: string, session: HuanxingSession): string {
  let result = toml;

  result = result.replace(
    /^default_provider\s*=\s*".*"$/m,
    `default_provider = "${HUANXING_CONFIG.defaultProvider}"`
  );

  result = result.replace(
    /^default_model\s*=\s*".*"$/m,
    `default_model = "${HUANXING_CONFIG.defaultModel}"`
  );

  const providerBlock = `[model_providers]\nopenai_compat = { api_key = "${session.llmToken}", base_url = "${HUANXING_CONFIG.llmGatewayV1}" }`;

  const providerRegex = /\[model_providers\][\s\S]*?(?=\n\[(?!model_providers))/;
  if (providerRegex.test(result)) {
    result = result.replace(providerRegex, providerBlock + '\n\n');
  }

  return result;
}

/**
 * 首次初始化 — sidecar 还没有配置时
 */
async function initializeConfig(session: HuanxingSession): Promise<OnboardResult> {
  const minimalConfig = generateMinimalConfig(session);

  try {
    const resp = await fetch('/api/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'text/plain' },
      body: minimalConfig,
    });

    if (!resp.ok) {
      return { success: true, configUpdated: false, error: await resp.text() };
    }

    return { success: true, configUpdated: true, workspaceReady: true };
  } catch (err) {
    return { success: true, configUpdated: false, error: String(err) };
  }
}

/**
 * 生成最小可用 TOML 配置（Web 开发模式 fallback）
 */
function generateMinimalConfig(session: HuanxingSession): string {
  const agentName = session.user.nickname || HUANXING_CONFIG.defaultAgentName;

  return `# 唤星桌面端 — 自动生成配置
# 云服务: ${HUANXING_CONFIG.cloudBaseUrl}
# LLM 网关: ${HUANXING_CONFIG.llmGatewayUrl}
# 生成时间: ${new Date().toISOString()}

default_provider = "${HUANXING_CONFIG.defaultProvider}"
default_model = "${HUANXING_CONFIG.defaultModel}"
default_temperature = ${HUANXING_CONFIG.defaultTemperature}
model_routes = []
embedding_routes = []

[model_providers]
openai_compat = { api_key = "${session.llmToken}", base_url = "${HUANXING_CONFIG.llmGatewayV1}" }

[provider]

[observability]
backend = "none"

[autonomy]
level = "supervised"
workspace_only = true

[agent]
compact_context = true
max_tool_iterations = 20
max_history_messages = 50

[agent.session]
backend = "none"
strategy = "per-sender"
ttl_seconds = 3600
max_messages = 50

[memory]
backend = "sqlite"
auto_save = true
hygiene_enabled = true
embedding_provider = "none"

[gateway]
port = 42620
host = "127.0.0.1"
require_pairing = false

[huanxing]
enabled = true
api_base_url = "${HUANXING_CONFIG.backendBaseUrl}"

[huanxing.templates]

[security]
canary_tokens = true

[security.otp]
enabled = false

[identity]
format = "openclaw"

[scheduler]
enabled = true

[cron]
enabled = true

[plugins]
enabled = true

[plugins.entries]

[skills]
open_skills_enabled = false

[reliability]
provider_retries = 2

[runtime]
kind = "native"
`;
}

// ── HASN 身份注册（登录后调用）──────────────────────────

/**
 * 注册 HASN 身份（Human + 默认 Agent），幂等
 *
 * @returns HasnIdentity — 包含 hasn_id, star_id
 */
export async function registerHasnIdentity(session: HuanxingSession): Promise<HasnIdentity> {
  const resp = await fetch(hasnApiUrl('/api/v1/hasn/app/auth/register'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${session.accessToken}`,
    },
    body: JSON.stringify({
      name: session.user.nickname || '唤星用户',
      avatar_url: session.user.avatar || null,
    }),
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`HASN 注册失败 (${resp.status}): ${text}`);
  }

  const json = await resp.json();
  const data = json.data ?? json;

  return {
    hasn_id: data.human?.hasn_id,
    star_id: data.human?.star_id,
    name: data.human?.name,
    agent_hasn_id: data.agent?.hasn_id,
    agent_star_id: data.agent?.star_id,
    already_exists: data.already_exists ?? false,
  };
}


/** Agent HASN 注册结果 */
export interface AgentHasnIdentity {
  hasn_id: string;
  star_id: string;
  name: string;
  agent_name: string;
  api_key?: string;
  already_exists: boolean;
}

/**
 * 注册 Agent 的 HASN 身份（幂等）
 *
 * 用于桌面端本地 Agent（如 "default"）。
 * 后端默认 Agent（"star"）由 registerHasnIdentity 自动创建。
 */
export async function registerHasnAgent(
  session: HuanxingSession,
  agentName: string,
  displayName: string,
  agentType: string = 'local',
  clientId?: string,
): Promise<AgentHasnIdentity> {
  // 如果是本地 Agent 且未传 clientId，尝试从 Tauri state 获取
  let resolvedClientId = clientId;
  if (agentType === 'local' && !resolvedClientId) {
    try {
      const internals = (window as any).__TAURI_INTERNALS__;
      if (internals?.invoke) {
        const statusJson = await internals.invoke('hasn_status');
        // hasn_status 返回 "connected" / "disconnected" 等字符串
        // 从 hasn client.json 读取 client_id
        const { loadClientId } = await import('./lib/hasn-api');
        resolvedClientId = await loadClientId();
      }
    } catch {
      // 忽略，clientId 可选
    }
  }

  const body: Record<string, unknown> = {
    agent_name: agentName,
    display_name: displayName,
    agent_type: agentType,
  };
  if (resolvedClientId) body.client_id = resolvedClientId;

  const resp = await fetch(hasnApiUrl('/api/v1/hasn/app/auth/register-agent'), {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${session.accessToken}`,
    },
    body: JSON.stringify(body),
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(`Agent HASN 注册失败 (${resp.status}): ${text}`);
  }

  const json = await resp.json();
  const data = json.data ?? json;

  const result: AgentHasnIdentity = {
    hasn_id: data.hasn_id,
    star_id: data.star_id,
    name: data.name,
    agent_name: data.agent_name,
    api_key: data.api_key,
    already_exists: data.already_exists ?? false,
  };

  // 注册成功后，将 hasn_id 写回 Agent 的 config.toml
  if (result.hasn_id && agentType === 'local') {
    try {
      await writeAgentHasnId(agentName, result.hasn_id);
      console.log(`[onboard] Agent '${agentName}' hasn_id 已写入 config.toml:`, result.hasn_id);
    } catch (err) {
      console.warn(`[onboard] Agent '${agentName}' hasn_id 写入 config.toml 失败（非致命）:`, err);
    }
  }

  return result;
}

/**
 * 将 hasn_id 写回 Agent 的 config.toml
 *
 * 通过 Sidecar REST API 读取 → 替换/插入 hasn_id → 写回。
 * 不依赖 Tauri IPC，Sidecar 运行在 localhost:42620。
 */
async function writeAgentHasnId(agentName: string, hasnId: string): Promise<void> {
  const sidecarPort = 42620; // TODO: 从配置读取
  const baseUrl = `http://127.0.0.1:${sidecarPort}`;
  const filePath = `/api/agents/${encodeURIComponent(agentName)}/files/config.toml`;

  // 1. 读取现有 config
  const readResp = await fetch(`${baseUrl}${filePath}`);
  if (!readResp.ok) {
    throw new Error(`读取 agent config 失败: ${readResp.status}`);
  }
  const json = await readResp.json();
  let content: string = json.content ?? '';

  // 2. 替换或插入 hasn_id
  if (content.includes('hasn_id =')) {
    // 替换已有的 hasn_id 行
    content = content.replace(/hasn_id\s*=\s*"[^"]*"/, `hasn_id = "${hasnId}"`);
  } else if (content.includes('[agent]')) {
    // 在 [agent] 段落下插入
    content = content.replace('[agent]', `[agent]\nhasn_id = "${hasnId}"`);
  } else {
    // 追加
    content += `\nhasn_id = "${hasnId}"\n`;
  }

  // 3. 写回
  const writeResp = await fetch(`${baseUrl}${filePath}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'text/plain' },
    body: content,
  });
  if (!writeResp.ok) {
    throw new Error(`写回 agent config 失败: ${writeResp.status}`);
  }
}
