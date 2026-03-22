package ai.zeroclaw.android

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.os.Build
import androidx.work.Configuration
import androidx.work.WorkManager
import ai.zeroclaw.android.data.SessionManager
import ai.zeroclaw.android.data.SettingsRepository
import ai.zeroclaw.android.network.HuanxingApi
import ai.zeroclaw.android.onboard.OnboardManager
import ai.zeroclaw.android.worker.HeartbeatWorker
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

class ZeroClawApp : Application(), Configuration.Provider {

    companion object {
        const val CHANNEL_ID = "zeroclaw_service"
        const val CHANNEL_NAME = "ZeroClaw Agent"
        const val AGENT_CHANNEL_ID = "zeroclaw_agent"
        const val AGENT_CHANNEL_NAME = "Agent Messages"

        // Singleton instance for easy access
        lateinit var instance: ZeroClawApp
            private set
    }

    // Application scope for coroutines
    private val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    // Lazy initialized repositories
    val settingsRepository by lazy { SettingsRepository(this) }
    val sessionManager by lazy { SessionManager(this) }

    override fun onCreate() {
        super.onCreate()
        instance = this

        createNotificationChannels()
        initializeWorkManager()

        // Schedule heartbeat if auto-start is enabled
        applicationScope.launch {
            val settings = settingsRepository.settings.first()
            if (settings.autoStart && settings.isConfigured()) {
                HeartbeatWorker.scheduleHeartbeat(
                    this@ZeroClawApp,
                    settings.heartbeatIntervalMinutes.toLong()
                )
            }
        }

        // Listen for settings changes and update heartbeat schedule
        applicationScope.launch {
            settingsRepository.settings
                .map { Triple(it.autoStart, it.isConfigured(), it.heartbeatIntervalMinutes) }
                .distinctUntilChanged()
                .collect { (autoStart, isConfigured, intervalMinutes) ->
                    if (autoStart && isConfigured) {
                        HeartbeatWorker.scheduleHeartbeat(this@ZeroClawApp, intervalMinutes.toLong())
                    } else {
                        HeartbeatWorker.cancelHeartbeat(this@ZeroClawApp)
                    }
                }
        }

        // Token 自动刷新 + LLM 配置更新
        startTokenRefreshLoop()
    }

    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = getSystemService(NotificationManager::class.java)

            // Service channel (foreground service - low priority, silent)
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                CHANNEL_NAME,
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "ZeroClaw background service notification"
                setShowBadge(false)
                enableVibration(false)
                setSound(null, null)
            }

            // Agent messages channel (high priority for important messages)
            val agentChannel = NotificationChannel(
                AGENT_CHANNEL_ID,
                AGENT_CHANNEL_NAME,
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Messages and alerts from your AI agent"
                enableVibration(true)
                setShowBadge(true)
            }

            manager.createNotificationChannel(serviceChannel)
            manager.createNotificationChannel(agentChannel)
        }
    }

    private fun initializeWorkManager() {
        // WorkManager is initialized via Configuration.Provider
        // This ensures it's ready before any work is scheduled
    }

    // Configuration.Provider implementation for custom WorkManager setup
    override val workManagerConfiguration: Configuration
        get() = Configuration.Builder()
            .setMinimumLoggingLevel(android.util.Log.INFO)
            .build()

    /**
     * Token 自动刷新循环。
     *
     * 每 60 秒检查一次：
     * - access_token 即将过期 → 调用 /api/v1/auth/refresh
     * - 刷新成功后 → 同步调用 /api/v1/auth/llm-config 更新 LLM 配置
     * - LLM 配置有变化 → 更新 config.toml
     */
    private fun startTokenRefreshLoop() {
        applicationScope.launch(Dispatchers.IO) {
            while (true) {
                delay(60_000) // 每 60 秒检查一次

                if (!sessionManager.isLoggedIn()) continue
                if (!sessionManager.needsRefresh()) continue

                val session = sessionManager.getSession() ?: continue

                android.util.Log.d("ZeroClawApp", "Token 即将过期，开始刷新")

                // 刷新 access_token
                val refreshResult = HuanxingApi.refreshToken(session.refreshToken)
                refreshResult.fold(
                    onSuccess = { resp ->
                        sessionManager.updateAccessToken(
                            newToken = resp.access_token,
                            expireTime = resp.access_token_expire_time,
                            newRefreshToken = resp.new_refresh_token,
                            newRefreshExpireTime = resp.new_refresh_token_expire_time
                        )
                        android.util.Log.i("ZeroClawApp", "Token 刷新成功")

                        // 同步更新 LLM 配置
                        refreshLlmConfig(resp.access_token)
                    },
                    onFailure = { e ->
                        android.util.Log.w("ZeroClawApp", "Token 刷新失败: ${e.message}")
                    }
                )
            }
        }
    }

    /**
     * 获取最新 LLM 配置并更新本地。
     * 供 token 刷新循环和 Settings 手动刷新调用。
     */
    suspend fun refreshLlmConfig(accessToken: String? = null): Result<Unit> {
        val token = accessToken ?: sessionManager.getSession()?.accessToken ?: return Result.failure(
            RuntimeException("未登录")
        )

        return HuanxingApi.getLlmConfig(token).map { llmConfig ->
            val currentSession = sessionManager.getSession()
            val changed = currentSession?.llmToken != llmConfig.api_token ||
                    currentSession.llmBaseUrl != llmConfig.llm_base_url

            if (changed) {
                // 更新 SessionManager
                sessionManager.updateLlmConfig(llmConfig.api_token, llmConfig.llm_base_url)

                // 更新 config.toml
                val onboardManager = OnboardManager(this@ZeroClawApp)
                onboardManager.updateLlmConfig(llmConfig.api_token, llmConfig.llm_base_url)

                android.util.Log.i("ZeroClawApp", "LLM 配置已更新: ${llmConfig.llm_base_url}")
            }
        }
    }
}
