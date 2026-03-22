package ai.zeroclaw.android.onboard

import android.content.Context
import android.util.Log
import ai.zeroclaw.android.data.HuanxingSession
import ai.zeroclaw.android.network.HuanxingApi
import ai.zeroclaw.android.process.ZeroClawProcessManager
import java.io.File

/**
 * Onboard 管理器：登录后初始化 zeroclaw 运行环境。
 *
 * 流程：
 * 1. 生成 config.toml（注入 llm_token + llm_base_url）
 * 2. 创建默认 agent 目录
 * 3. 启动 zeroclaw 子进程
 *
 * 对齐桌面端 onboard.ts + sidecar.rs 的逻辑。
 */
class OnboardManager(private val context: Context) {

    companion object {
        private const val TAG = "OnboardManager"
    }

    private val processManager = ZeroClawProcessManager(context)

    /** 执行完整 onboard 流程 */
    suspend fun onboard(session: HuanxingSession): Result<Unit> {
        return runCatching {
            val configDir = processManager.configDir

            // 1. 生成 config.toml
            val configFile = File(configDir, "config.toml")
            val configContent = generateConfigToml(session)
            configFile.writeText(configContent)
            Log.i(TAG, "config.toml 已生成: ${configFile.absolutePath}")

            // 2. 创建默认 agent 目录
            val agentDir = File(configDir, "agents/default")
            agentDir.mkdirs()
            Log.i(TAG, "默认 agent 目录已创建: ${agentDir.absolutePath}")

            // 3. 启动 zeroclaw 子进程
            processManager.start().getOrThrow()
            Log.i(TAG, "Onboard 完成")
        }
    }

    /** 更新 config.toml 中的 LLM 配置（不重启进程） */
    fun updateLlmConfig(llmToken: String, llmBaseUrl: String) {
        val configFile = File(processManager.configDir, "config.toml")
        if (!configFile.exists()) {
            Log.w(TAG, "config.toml 不存在，跳过更新")
            return
        }

        var content = configFile.readText()

        // 替换 api_key
        content = content.replace(
            Regex("""api_key\s*=\s*"[^"]*""""),
            """api_key = "$llmToken""""
        )

        // 替换 base_url（model_providers 中的）
        content = content.replace(
            Regex("""base_url\s*=\s*"[^"]*""""),
            """base_url = "$llmBaseUrl""""
        )

        // 替换 default_provider
        val baseUrlWithoutV1 = llmBaseUrl.removeSuffix("/v1").removeSuffix("/")
        content = content.replace(
            Regex("""default_provider\s*=\s*"[^"]*""""),
            """default_provider = "custom:$baseUrlWithoutV1""""
        )

        configFile.writeText(content)
        Log.i(TAG, "config.toml LLM 配置已更新")
    }

    /** config.toml 是否已存在 */
    fun isConfigured(): Boolean {
        return File(processManager.configDir, "config.toml").exists()
    }

    /** 获取 ProcessManager 实例（供 Service 使用） */
    fun getProcessManager(): ZeroClawProcessManager = processManager

    /**
     * 生成 config.toml 内容。
     *
     * 模板对齐桌面端 onboard.ts:generateMinimalConfig()。
     * llm_base_url 和 llm_token 从登录响应动态注入。
     */
    private fun generateConfigToml(session: HuanxingSession): String {
        val llmBaseUrl = session.llmBaseUrl.ifBlank { "https://llm.dcfuture.cn/v1" }
        val baseUrlWithoutV1 = llmBaseUrl.removeSuffix("/v1").removeSuffix("/")
        val agentName = session.userNickname.ifBlank { HuanxingApi.DEFAULT_AGENT_NAME }

        return """# 唤星 Android 端 — 自动生成配置
# API: ${HuanxingApi.API_BASE_URL}
# LLM: $llmBaseUrl

display_name = "$agentName"
default_provider = "custom:$baseUrlWithoutV1"
default_model = "${HuanxingApi.DEFAULT_MODEL}"
title_model = "${HuanxingApi.TITLE_MODEL}"
default_temperature = ${HuanxingApi.DEFAULT_TEMPERATURE}
model_routes = []
embedding_routes = []

[model_providers]
openai_compat = { api_key = "${session.llmToken}", base_url = "$llmBaseUrl" }

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
port = ${ZeroClawProcessManager.DEFAULT_PORT}
host = "127.0.0.1"
require_pairing = false

[huanxing]
enabled = true
api_base_url = "${HuanxingApi.API_BASE_URL}"

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
"""
    }
}
