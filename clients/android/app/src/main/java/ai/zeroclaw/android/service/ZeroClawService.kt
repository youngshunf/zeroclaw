package ai.zeroclaw.android.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import ai.zeroclaw.android.MainActivity
import ai.zeroclaw.android.ZeroClawApp
import ai.zeroclaw.android.data.SessionManager
import ai.zeroclaw.android.onboard.OnboardManager
import ai.zeroclaw.android.process.ZeroClawProcessManager
import ai.zeroclaw.android.ws.ConnectionStatus
import ai.zeroclaw.android.ws.WsInboundMessage
import ai.zeroclaw.android.ws.ZeroClawWsClient
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow

import java.util.UUID

/**
 * 前台服务：管理 zeroclaw 子进程 + WS 通信。
 *
 * 生命周期：
 * 1. ACTION_START → 启动子进程 → 健康检查 → WS 连接 → Running
 * 2. ACTION_SEND → 通过 WS 发送消息
 * 3. ACTION_STOP → 断开 WS → 停止子进程 → Stopped
 */
class ZeroClawService : Service() {

    companion object {
        private const val TAG = "ZeroClawService"
        private const val NOTIFICATION_ID = 1001
        const val ACTION_START = "ai.zeroclaw.action.START"
        const val ACTION_STOP = "ai.zeroclaw.action.STOP"
        const val ACTION_SEND = "ai.zeroclaw.action.SEND"
        const val EXTRA_MESSAGE = "message"
    }

    private val binder = LocalBinder()
    private val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())

    private lateinit var processManager: ZeroClawProcessManager
    private lateinit var sessionManager: SessionManager
    private lateinit var onboardManager: OnboardManager
    private var wsClient: ZeroClawWsClient? = null

    /** 当前会话 ID（每次启动生成新的） */
    private var sessionId: String = UUID.randomUUID().toString()

    private val _status = MutableStateFlow<Status>(Status.Stopped)
    val status: StateFlow<Status> = _status

    private val _lastMessage = MutableStateFlow<String?>(null)
    val lastMessage: StateFlow<String?> = _lastMessage

    /** UI 层订阅的完整消息流 */
    private val _chatMessages = MutableSharedFlow<WsInboundMessage>(extraBufferCapacity = 64)
    val chatMessages: SharedFlow<WsInboundMessage> = _chatMessages.asSharedFlow()

    inner class LocalBinder : Binder() {
        fun getService(): ZeroClawService = this@ZeroClawService
    }

    override fun onBind(intent: Intent): IBinder = binder

    override fun onCreate() {
        super.onCreate()
        processManager = ZeroClawProcessManager(this)
        sessionManager = SessionManager(this)
        onboardManager = OnboardManager(this)
        startForeground(NOTIFICATION_ID, createNotification())
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startAgent()
            ACTION_STOP -> stopAgent()
            ACTION_SEND -> intent.getStringExtra(EXTRA_MESSAGE)?.let { sendMessage(it) }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        stopAgent()
        scope.cancel()
        super.onDestroy()
    }

    private fun startAgent() {
        if (_status.value == Status.Running) return
        _status.value = Status.Starting

        scope.launch {
            try {
                // 1. 检查登录状态
                val session = sessionManager.getSession()
                if (session == null || session.llmToken.isBlank()) {
                    _status.value = Status.Error("请先登录")
                    return@launch
                }

                // 2. 如果 config.toml 不存在，先执行 onboard
                if (!onboardManager.isConfigured()) {
                    onboardManager.onboard(session).getOrThrow()
                } else {
                    // config 已存在，直接启动子进程
                    processManager.start().getOrThrow()
                }

                // 3. 建立 WS 连接（使用 gateway_token 认证）
                sessionId = UUID.randomUUID().toString()
                val wsToken = session.gatewayToken.ifBlank { session.llmToken }
                wsClient = ZeroClawWsClient(
                    port = processManager.port,
                    token = wsToken,
                    agentName = session.userNickname.ifBlank { "default" }
                )
                wsClient!!.connect()

                // 4. 订阅 WS 消息
                launch {
                    wsClient!!.messages.collect { msg ->
                        handleInboundMessage(msg)
                    }
                }

                // 5. 等待连接建立（最多 5 秒）
                var waited = 0
                while (wsClient!!.connectionStatus.value != ConnectionStatus.CONNECTED && waited < 5000) {
                    delay(100)
                    waited += 100
                }

                if (wsClient!!.connectionStatus.value == ConnectionStatus.CONNECTED) {
                    _status.value = Status.Running
                    Log.i(TAG, "Agent 启动成功")
                } else {
                    _status.value = Status.Error("WS 连接超时")
                }

            } catch (e: Exception) {
                Log.e(TAG, "Agent 启动失败", e)
                _status.value = Status.Error(e.message ?: "启动失败")
            }
        }
    }

    private fun stopAgent() {
        scope.launch {
            wsClient?.disconnect()
            wsClient = null
            processManager.stop()
            _status.value = Status.Stopped
            Log.i(TAG, "Agent 已停止")
        }
    }

    fun sendMessage(message: String) {
        if (_status.value != Status.Running) return
        wsClient?.sendMessage(message, sessionId)
    }

    private suspend fun handleInboundMessage(msg: WsInboundMessage) {
        // 转发到 UI 层
        _chatMessages.emit(msg)

        when (msg.type) {
            "done" -> {
                _lastMessage.value = msg.fullResponse
            }
            "error" -> {
                Log.w(TAG, "Agent 错误: ${msg.message}")
            }
            "connected" -> {
                Log.i(TAG, "WS 连接确认: ${msg.message}")
            }
            "session_start" -> {
                Log.i(TAG, "Session 开始: resumed=${msg.resumed}, messages=${msg.messageCount}")
            }
        }
    }

    private fun createNotification(): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, ZeroClawApp.CHANNEL_ID)
            .setContentTitle("ZeroClaw is running")
            .setContentText("Your AI assistant is active")
            .setSmallIcon(android.R.drawable.ic_menu_manage)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setSilent(true)
            .build()
    }

    sealed class Status {
        object Stopped : Status()
        object Starting : Status()
        object Running : Status()
        data class Error(val message: String) : Status()
    }
}
