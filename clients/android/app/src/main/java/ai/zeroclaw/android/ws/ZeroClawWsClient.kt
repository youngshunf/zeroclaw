package ai.zeroclaw.android.ws

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.net.URLEncoder
import java.util.concurrent.TimeUnit

/**
 * OkHttp WebSocket 客户端，与 zeroclaw gateway 通信。
 *
 * 端点：ws://127.0.0.1:{port}/ws/chat?token=xxx
 * 协议：zeroclaw WS v2（多 session 复用）
 *
 * @see src/gateway/ws.rs
 */
class ZeroClawWsClient(
    private val port: Int,
    private val token: String,
    private val agentName: String
) {
    companion object {
        private const val TAG = "WsClient"
    }

    private val client = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .pingInterval(30, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    private val json = Json { ignoreUnknownKeys = true }

    private val _messages = MutableSharedFlow<WsInboundMessage>(extraBufferCapacity = 64)
    val messages: SharedFlow<WsInboundMessage> = _messages.asSharedFlow()

    private val _connectionStatus = MutableStateFlow(ConnectionStatus.DISCONNECTED)
    val connectionStatus: StateFlow<ConnectionStatus> = _connectionStatus.asStateFlow()

    /** 建立 WS 连接 */
    fun connect() {
        if (_connectionStatus.value == ConnectionStatus.CONNECTED) return
        _connectionStatus.value = ConnectionStatus.CONNECTING

        val encodedToken = URLEncoder.encode(token, "UTF-8")
        val url = "ws://127.0.0.1:$port/ws/chat?token=$encodedToken"
        val request = Request.Builder().url(url).build()

        webSocket = client.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                _connectionStatus.value = ConnectionStatus.CONNECTED
                Log.i(TAG, "WS 已连接")
            }

            override fun onMessage(ws: WebSocket, text: String) {
                try {
                    val msg = json.decodeFromString<WsInboundMessage>(text)
                    scope.launch { _messages.emit(msg) }
                } catch (e: Exception) {
                    Log.w(TAG, "WS 消息解析失败: $text", e)
                }
            }

            override fun onClosing(ws: WebSocket, code: Int, reason: String) {
                ws.close(1000, null)
                _connectionStatus.value = ConnectionStatus.DISCONNECTED
                Log.i(TAG, "WS 关闭中: $code $reason")
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                Log.e(TAG, "WS 连接失败", t)
                _connectionStatus.value = ConnectionStatus.DISCONNECTED
            }
        })
    }

    /** 发送聊天消息 */
    fun sendMessage(content: String, sessionId: String? = null) {
        val frame = WsOutboundMessage(
            content = content,
            agent = agentName,
            sessionId = sessionId
        )
        webSocket?.send(json.encodeToString(WsOutboundMessage.serializer(), frame))
    }

    /** 请求历史记录 */
    fun requestHistory(sessionId: String) {
        val frame = WsHistoryRequest(sessionId = sessionId, agent = agentName)
        webSocket?.send(json.encodeToString(WsHistoryRequest.serializer(), frame))
    }

    /** 断开连接 */
    fun disconnect() {
        webSocket?.close(1000, "Client disconnect")
        webSocket = null
        _connectionStatus.value = ConnectionStatus.DISCONNECTED
    }
}
