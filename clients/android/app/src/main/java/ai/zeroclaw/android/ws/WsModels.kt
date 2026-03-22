package ai.zeroclaw.android.ws

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** WS 连接状态 */
enum class ConnectionStatus {
    DISCONNECTED, CONNECTING, CONNECTED
}

/**
 * 服务端 → 客户端的 WS 帧。
 *
 * 协议 v2（多 session），按 type 区分帧类型：
 * - connected: 连接建立确认
 * - session_start: session 初始化/恢复
 * - chunk: 流式文本片段
 * - done: 完整回复
 * - error: 错误
 * - tool_call: 工具调用开始
 * - tool_result: 工具调用结果
 * - history: 历史消息
 *
 * @see src/gateway/ws.rs 协议定义
 */
@Serializable
data class WsInboundMessage(
    val type: String,
    @SerialName("session_id")
    val sessionId: String? = null,
    // connected / error
    val message: String? = null,
    // chunk
    val content: String? = null,
    // done
    @SerialName("full_response")
    val fullResponse: String? = null,
    // session_start
    val resumed: Boolean? = null,
    @SerialName("message_count")
    val messageCount: Int? = null,
    // history
    val messages: List<HistoryMessage>? = null,
    // tool_call
    @SerialName("call_id")
    val callId: String? = null,
    val name: String? = null,
    @SerialName("display_name")
    val displayName: String? = null,
    @SerialName("args_preview")
    val argsPreview: String? = null,
    // tool_result
    val status: String? = null,
    @SerialName("output_preview")
    val outputPreview: String? = null
)

@Serializable
data class HistoryMessage(
    val role: String,
    val content: String
)

/** 客户端 → 服务端：聊天消息帧 */
@Serializable
data class WsOutboundMessage(
    val type: String = "message",
    val content: String,
    val agent: String,
    @SerialName("session_id")
    val sessionId: String? = null
)

/** 客户端 → 服务端：历史请求帧 */
@Serializable
data class WsHistoryRequest(
    val type: String = "history_request",
    @SerialName("session_id")
    val sessionId: String,
    val agent: String
)
